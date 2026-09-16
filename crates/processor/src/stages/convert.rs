//! Invoke prebuilt `_3dtile` / `GEOFORGE_3DTILE` (no Docker fallback).

use crate::cancel::CancelFlag;
use crate::protocol::{Emitter, Stage};
use crate::util::{run_logged_env_result, tool_paths, CommandResult};
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub fn run_convert(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    osgb_root: &str,
    out_dir: &Path,
    options: &Value,
    cfg_json: Option<&str>,
) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let extra = convert_flags(emitter, options, cfg_json);
    let tools = tool_paths();

    emitter.stage(Stage::Convert, "OSGB → 3D Tiles");
    let started = std::time::Instant::now();
    let configured_threads = std::env::var("GEOFORGE_CONVERT_THREADS")
        .ok()
        .and_then(|value| value.parse::<usize>().ok())
        .filter(|value| *value > 0);
    emitter.metric("converter.threads.configured", json!(configured_threads));
    let result = if tools.convert_bin.is_file() {
        run_native(
            emitter,
            cancel,
            &tools.convert_bin,
            osgb_root,
            out_dir,
            &extra,
        )
    } else if tools.packaged {
        return Err(format!(
            "组件缺失，请修复安装（转换器 _3dtile 未找到：{}）",
            tools.convert_bin.display()
        ));
    } else {
        return Err(format!(
            "找不到转换器 _3dtile（查过 {}）。请运行 apps/desktop/scripts/prepare-converter.ps1，或设置 GEOFORGE_3DTILE。",
            tools.convert_bin.display()
        ));
    };

    let result = match result {
        Ok(result) => result,
        Err(error) => {
            emitter.metric("converter.elapsedMs", json!(started.elapsed().as_millis()));
            return Err(error);
        }
    };

    emitter.metric("converter.elapsedMs", json!(started.elapsed().as_millis()));
    if let Some(bytes) = result.peak_memory_bytes {
        emitter.metric("converter.peakMemoryBytes", json!(bytes));
    }

    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    if result.exit_code != 0 {
        let detail = if result.stderr_tail.is_empty() {
            "converter did not provide stderr output".to_string()
        } else {
            format!("last converter output:\n{}", result.stderr_tail)
        };
        return Err(format!("convert exited {}: {detail}", result.exit_code));
    }
    let tileset = out_dir.join("tileset.json");
    if !tileset.is_file() {
        return Err(format!("tileset.json missing under {}", out_dir.display()));
    }
    Ok(())
}

fn convert_flags(emitter: &Arc<Emitter>, options: &Value, cfg_json: Option<&str>) -> Vec<String> {
    let mut extra = Vec::new();
    let conv = options.get("convert").cloned().unwrap_or(Value::Null);
    if conv
        .get("verbose")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        extra.push("-v".into());
    }
    if let Some(cfg) = conv.get("config").and_then(|v| v.as_str()) {
        extra.push("-c".into());
        extra.push(cfg.into());
        emitter.log("[convert] using options.convert.config");
    } else if let Some(cfg) = cfg_json {
        extra.push("-c".into());
        extra.push(cfg.into());
        emitter.log(&format!("[convert] -c {cfg}"));
    }

    let texture = options.get("texture").cloned().unwrap_or(Value::Null);
    let mode = crate::stages::texture::normalize_mode(texture.get("mode").and_then(|v| v.as_str()));
    if mode != "keep" {
        if std::env::var("GEOFORGE_NATIVE_KTX2").ok().as_deref() == Some("1") {
            extra.push("--enable-texture-compress".into());
            emitter.log(&format!(
                "[convert] texture flag: --enable-texture-compress (mode={mode})"
            ));
        } else {
            emitter.log(&format!(
                "[convert] mode={mode}: convert without native KTX2; texture stage may post-process"
            ));
        }
    }
    extra
}

fn run_native(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    bin: &Path,
    osgb_root: &str,
    out_dir: &Path,
    extra: &[String],
) -> Result<CommandResult, String> {
    let mut cmd = vec![
        bin.to_string_lossy().into_owned(),
        "-f".into(),
        "osgb".into(),
        "-i".into(),
        strip_verbatim_str(osgb_root),
        "-o".into(),
        strip_verbatim_str(&out_dir.to_string_lossy()),
    ];
    cmd.extend(extra.iter().cloned());
    let cwd = bin.parent();
    let mut env = Vec::new();
    if let Some(dir) = cwd {
        let plugins = dir.join("osgPlugins-3.6.5");
        if plugins.is_dir() {
            env.push(("OSG_LIBRARY_PATH", plugins));
        }
        let gdal = dir.join("gdal");
        if gdal.is_dir() {
            env.push(("GDAL_DATA", gdal));
        }
        let proj = dir.join("proj");
        if proj.is_dir() {
            env.push(("PROJ_DATA", proj.clone()));
            env.push(("PROJ_LIB", proj));
        }
    }
    let env_refs: Vec<(&str, PathBuf)> = env;
    run_logged_env_result(emitter, cancel, &cmd, cwd, &env_refs)
}

fn strip_verbatim_str(s: &str) -> String {
    #[cfg(windows)]
    {
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            return format!(r"\\{rest}");
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return rest.to_string();
        }
    }
    s.to_string()
}
