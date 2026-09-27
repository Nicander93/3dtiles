//! Invoke prebuilt `_3dtile` / `GEOFORGE_3DTILE` (no Docker fallback).

use crate::cancel::CancelFlag;
use crate::protocol::{Emitter, Stage};
use crate::util::{run_logged_env_result, tool_paths, CommandResult};
use geoforge_protocol::ModelFormat;
use serde_json::{json, Value};
use std::path::{Path, PathBuf};
use std::sync::Arc;

#[derive(Debug, Clone)]
struct ThreadConfig {
    requested: String,
    override_value: Option<usize>,
}

impl ThreadConfig {
    fn to_override(&self) -> Option<usize> {
        self.override_value
    }
}

fn resolve_thread_config(options: &Value) -> ThreadConfig {
    let threads_value = options
        .get("convert")
        .and_then(|v| v.get("threads"));
    
    match threads_value {
        Some(Value::Number(n)) if n.is_u64() => {
            let val = n.as_u64().unwrap() as usize;
            if val == 0 {
                ThreadConfig {
                    requested: "0 (explicit auto)".to_string(),
                    override_value: Some(0),
                }
            } else if val >= 1 && val <= 16 {
                ThreadConfig {
                    requested: format!("{} (explicit)", val),
                    override_value: Some(val),
                }
            } else {
                ThreadConfig {
                    requested: format!("{} (invalid, > 16)", val),
                    override_value: None,
                }
            }
        }
        Some(v) => {
            ThreadConfig {
                requested: format!("invalid type: {}", v),
                override_value: None,
            }
        }
        None => {
            if let Ok(env_val) = std::env::var("GEOFORGE_CONVERT_THREADS") {
                if let Ok(parsed) = env_val.parse::<usize>() {
                    if parsed > 0 && parsed <= 16 {
                        ThreadConfig {
                            requested: format!("from env: {}", parsed),
                            override_value: Some(parsed),
                        }
                    } else {
                        ThreadConfig {
                            requested: format!("from env: {} (invalid)", parsed),
                            override_value: None,
                        }
                    }
                } else {
                    ThreadConfig {
                        requested: format!("from env: {} (parse error)", env_val),
                        override_value: None,
                    }
                }
            } else {
                ThreadConfig {
                    requested: "default (omitted, no env)".to_string(),
                    override_value: None,
                }
            }
        }
    }
}


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
    
    let thread_config = resolve_thread_config(options);
    emitter.metric("converter.threads.requested", json!(thread_config.requested));
    emitter.log(&format!(
        "[convert] thread config requested: {}",
        thread_config.requested
    ));
    
    let thread_override = thread_config.to_override();
    emitter.metric("converter.threads.override", json!(thread_override));
    emitter.log(&format!(
        "[convert] thread override passed to subprocess: {}",
        thread_override.map_or("none (converter default)".to_string(), |t| {
            if t == 0 {
                "0 (auto)".to_string()
            } else {
                t.to_string()
            }
        })
    ));
    
    let result = if tools.convert_bin.is_file() {
        run_native(
            emitter,
            cancel,
            &tools.convert_bin,
            osgb_root,
            out_dir,
            &extra,
            thread_override,
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

    let mut result = match result {
        Ok(result) => result,
        Err(error) => {
            emitter.metric("converter.elapsedMs", json!(started.elapsed().as_millis()));
            return Err(error);
        }
    };

    let should_retry = result.exit_code != 0
        && thread_override != Some(1)
        && !cancel.is_cancelled()
        && retry_single_thread(&result);
    
    if should_retry {
        emitter.log(&format!(
            "[convert] multi-threaded converter failed (exit_code={}, thread_override={:?}); stderr tail:\n{}",
            result.exit_code,
            thread_override,
            if result.stderr_tail.is_empty() { "(empty)" } else { &result.stderr_tail }
        ));
        emitter.log("[convert] retrying once with one worker thread");
        emitter.metric("converter.retryCount", json!(1));
        emitter.metric("converter.retryThreads", json!(1));
        if out_dir.exists() {
            std::fs::remove_dir_all(out_dir).map_err(|error| {
                format!(
                    "failed to clear partial converter output before retry {}: {error}",
                    out_dir.display()
                )
            })?;
        }
        std::fs::create_dir_all(out_dir).map_err(|error| {
            format!(
                "failed to recreate converter output before retry {}: {error}",
                out_dir.display()
            )
        })?;
        result = run_native(
            emitter,
            cancel,
            &tools.convert_bin,
            osgb_root,
            out_dir,
            &extra,
            Some(1),
        )?;
        if result.exit_code == 0 {
            emitter.log("[convert] single-threaded retry succeeded");
        } else {
            emitter.log(&format!(
                "[convert] single-threaded retry also failed (exit_code={})",
                result.exit_code
            ));
        }
    }

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

/// Invoke the converter's explicit FBX/OBJ route. Model-specific settings are
/// validated by the task pipeline; this boundary deliberately does not reuse
/// the OSGB-only `-c` configuration contract.
pub fn run_model_convert(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    input_file: &Path,
    out_dir: &Path,
    format: ModelFormat,
    model_config: &Path,
    longitude: Option<f64>,
    latitude: Option<f64>,
    height: Option<f64>,
) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|error| error.to_string())?;
    let tools = tool_paths();
    if !tools.convert_bin.is_file() {
        return Err(format!(
            "找不到转换器 _3dtile（查过 {}）。请运行 apps/desktop/scripts/prepare-converter.ps1，或设置 GEOFORGE_3DTILE。",
            tools.convert_bin.display()
        ));
    }

    emitter.stage(Stage::Convert, "Model → 3D Tiles");
    let mut command = vec![
        tools.convert_bin.to_string_lossy().into_owned(),
        "-f".into(),
        format.extension().into(),
        "-i".into(),
        strip_verbatim_str(&input_file.to_string_lossy()),
        "-o".into(),
        strip_verbatim_str(&out_dir.to_string_lossy()),
        "--model-config".into(),
        strip_verbatim_str(&model_config.to_string_lossy()),
    ];
    if let (Some(longitude), Some(latitude), Some(height)) = (longitude, latitude, height) {
        command.extend([
            "--lon".into(),
            longitude.to_string(),
            "--lat".into(),
            latitude.to_string(),
            "--alt".into(),
            height.to_string(),
        ]);
    }

    let cwd = tools.convert_bin.parent();
    let env = converter_environment(cwd);
    let env_refs: Vec<(&str, PathBuf)> = env;
    let started = std::time::Instant::now();
    let result = run_logged_env_result(emitter, cancel, &command, cwd, &env_refs)?;
    emitter.metric("converter.elapsedMs", json!(started.elapsed().as_millis()));
    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    if result.exit_code != 0 {
        return Err(format!("convert exited {}: {}", result.exit_code, result.stderr_tail));
    }
    if !out_dir.join("tileset.json").is_file() {
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
        if crate::stages::texture::native_texture_mode_available(&mode) {
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
    thread_override: Option<usize>,
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
    let mut env = converter_environment(cwd);
    if let Some(threads) = thread_override {
        if threads == 0 {
            emitter.log("[convert] forcing thread mode to auto (0)");
            env.push((
                "GEOFORGE_CONVERT_THREADS",
                PathBuf::from("0"),
            ));
        } else {
            emitter.log(&format!("[convert] forcing thread count to {}", threads));
            env.push((
                "GEOFORGE_CONVERT_THREADS",
                PathBuf::from(threads.to_string()),
            ));
        }
    }
    let env_refs: Vec<(&str, PathBuf)> = env;
    run_logged_env_result(emitter, cancel, &cmd, cwd, &env_refs)
}

fn converter_environment(cwd: Option<&Path>) -> Vec<(&'static str, PathBuf)> {
    let mut env = Vec::new();
    let Some(dir) = cwd else { return env };
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
    env
}

fn retry_single_thread(result: &CommandResult) -> bool {
    const STATUS_ACCESS_VIOLATION: i32 = -1_073_741_819;
    const STATUS_HEAP_CORRUPTION: i32 = -1_073_740_940;
    result
        .stderr_tail
        .contains("converter returned no JSON for tile:")
        || matches!(
            result.exit_code,
            STATUS_ACCESS_VIOLATION | STATUS_HEAP_CORRUPTION
        )
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

#[cfg(test)]
mod tests {
    use super::{resolve_thread_config, retry_single_thread};
    use crate::util::CommandResult;
    use serde_json::json;
    use serial_test::serial;

    fn result(exit_code: i32, stderr_tail: &str) -> CommandResult {
        CommandResult {
            exit_code,
            stderr_tail: stderr_tail.into(),
            peak_memory_bytes: None,
        }
    }

    #[test]
    fn retries_missing_tile_json() {
        assert!(retry_single_thread(&result(
            1,
            "ERROR: converter returned no JSON for tile: D:\\data\\Tile.osgb",
        )));
    }

    #[test]
    fn retries_known_windows_native_crashes() {
        assert!(retry_single_thread(&result(-1_073_740_940, "")));
        assert!(retry_single_thread(&result(-1_073_741_819, "")));
    }

    #[test]
    fn does_not_retry_unrelated_converter_errors() {
        assert!(!retry_single_thread(&result(
            1,
            "failed to read metadata.xml"
        )));
    }

    #[test]
    fn thread_config_explicit_1() {
        let options = json!({"convert": {"threads": 1}});
        let config = resolve_thread_config(&options);
        assert_eq!(config.to_override(), Some(1));
        assert_eq!(config.requested, "1 (explicit)");
    }

    #[test]
    fn thread_config_explicit_2() {
        let options = json!({"convert": {"threads": 2}});
        let config = resolve_thread_config(&options);
        assert_eq!(config.to_override(), Some(2));
        assert_eq!(config.requested, "2 (explicit)");
    }

    #[test]
    fn thread_config_explicit_4() {
        let options = json!({"convert": {"threads": 4}});
        let config = resolve_thread_config(&options);
        assert_eq!(config.to_override(), Some(4));
        assert_eq!(config.requested, "4 (explicit)");
    }

    #[test]
    fn thread_config_explicit_auto() {
        let options = json!({"convert": {"threads": 0}});
        let config = resolve_thread_config(&options);
        assert_eq!(config.to_override(), Some(0));
        assert_eq!(config.requested, "0 (explicit auto)");
    }

    #[test]
    fn thread_config_invalid_high() {
        let options = json!({"convert": {"threads": 32}});
        let config = resolve_thread_config(&options);
        assert_eq!(config.to_override(), None);
        assert!(config.requested.contains("invalid"));
    }

    #[test]
    fn thread_config_invalid_type() {
        let options = json!({"convert": {"threads": "auto"}});
        let config = resolve_thread_config(&options);
        assert_eq!(config.to_override(), None);
        assert!(config.requested.contains("invalid type"));
    }

    #[test]
    #[serial]
    fn thread_config_omitted_no_env() {
        std::env::remove_var("GEOFORGE_CONVERT_THREADS");
        let options = json!({"convert": {}});
        let config = resolve_thread_config(&options);
        assert_eq!(config.to_override(), None);
        assert_eq!(config.requested, "default (omitted, no env)");
    }

    #[test]
    #[serial]
    fn thread_config_from_env() {
        std::env::set_var("GEOFORGE_CONVERT_THREADS", "4");
        let options = json!({"convert": {}});
        let config = resolve_thread_config(&options);
        assert_eq!(config.to_override(), Some(4));
        assert_eq!(config.requested, "from env: 4");
        std::env::remove_var("GEOFORGE_CONVERT_THREADS");
    }

    #[test]
    #[serial]
    fn thread_config_env_invalid() {
        std::env::set_var("GEOFORGE_CONVERT_THREADS", "999");
        let options = json!({"convert": {}});
        let config = resolve_thread_config(&options);
        assert_eq!(config.to_override(), None);
        assert!(config.requested.contains("invalid"));
        std::env::remove_var("GEOFORGE_CONVERT_THREADS");
    }

    #[test]
    #[serial]
    fn thread_config_explicit_wins_over_env() {
        std::env::set_var("GEOFORGE_CONVERT_THREADS", "8");
        let options = json!({"convert": {"threads": 2}});
        let config = resolve_thread_config(&options);
        assert_eq!(config.to_override(), Some(2));
        assert_eq!(config.requested, "2 (explicit)");
        std::env::remove_var("GEOFORGE_CONVERT_THREADS");
    }
}
