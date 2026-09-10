//! Invoke existing _3dtile / GEOFORGE_3DTILE wrapper.

use crate::cancel::CancelFlag;
use crate::protocol::{Emitter, Stage};
use std::sync::Arc;
use crate::util::{run_logged, tool_paths};
use serde_json::Value;
use std::path::Path;

pub fn run_convert(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    osgb_root: &str,
    out_dir: &Path,
    options: &Value,
    cfg_json: Option<&str>,
) -> Result<(), String> {
    let tools = tool_paths();
    if !tools.convert_bin.is_file() {
        return Err(format!("Converter not found: {}", tools.convert_bin.display()));
    }
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;

    let mut cmd: Vec<String> = vec![
        tools.convert_bin.to_string_lossy().into_owned(),
        "-f".into(),
        "osgb".into(),
        "-i".into(),
        osgb_root.into(),
        "-o".into(),
        out_dir.to_string_lossy().into_owned(),
    ];

    let conv = options.get("convert").cloned().unwrap_or(Value::Null);
    if conv.get("verbose").and_then(|v| v.as_bool()).unwrap_or(false) {
        cmd.push("-v".into());
    }
    if let Some(cfg) = conv.get("config").and_then(|v| v.as_str()) {
        cmd.push("-c".into());
        cmd.push(cfg.into());
        emitter.log("[convert] using options.convert.config");
    } else if let Some(cfg) = cfg_json {
        cmd.push("-c".into());
        cmd.push(cfg.into());
        emitter.log(&format!("[convert] -c {cfg}"));
    }

    // Native KTX2 only when explicitly requested and not using postprocess path.
    // Phase 3 happy-path smoke uses texture.mode=keep; KTX2 still via Python postprocess stage.
    let texture = options.get("texture").cloned().unwrap_or(Value::Null);
    let mode = crate::stages::texture::normalize_mode(
        texture.get("mode").and_then(|v| v.as_str()),
    );
    if mode != "keep" {
        // Prefer postprocess (basisu) — do not pass native flag unless GEOFORGE_NATIVE_KTX2=1
        if std::env::var("GEOFORGE_NATIVE_KTX2").ok().as_deref() == Some("1") {
            cmd.push("--enable-texture-compress".into());
            emitter.log(&format!("[convert] texture flag: --enable-texture-compress (mode={mode})"));
        } else {
            emitter.log(&format!(
                "[convert] mode={mode}: convert without native KTX2; texture stage may post-process"
            ));
        }
    }

    emitter.stage(Stage::Convert, "OSGB → 3D Tiles");
    let rc = run_logged(emitter, cancel, &cmd, None)?;
    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    if rc != 0 {
        return Err(format!("convert exited {rc}"));
    }
    let tileset = out_dir.join("tileset.json");
    if !tileset.is_file() {
        return Err(format!("tileset.json missing under {}", out_dir.display()));
    }
    Ok(())
}
