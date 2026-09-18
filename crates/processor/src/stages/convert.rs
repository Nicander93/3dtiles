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

    // Native KTX2 only when explicitly requested. Default release path uses Rust+basisu
    // texture stage (Phase 14); Python is experiments-only.
    let texture = options.get("texture").cloned().unwrap_or(Value::Null);
    let mode = crate::stages::texture::normalize_mode(
        texture.get("mode").and_then(|v| v.as_str()),
    );
    if mode != "keep" {
        // Prefer Rust postprocess (basisu sidecar) — native flag only with GEOFORGE_NATIVE_KTX2=1
        if std::env::var("GEOFORGE_NATIVE_KTX2").ok().as_deref() == Some("1") {
            cmd.push("--enable-texture-compress".into());
            emitter.log(&format!("[convert] texture flag: --enable-texture-compress (mode={mode})"));
        } else {
            emitter.log(&format!(
                "[convert] mode={mode}: convert without native KTX2; texture stage may post-process (rust+basisu)"
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
    // Native _3dtile often omits root `refine`; 3D Tiles requires it when children exist
    // (CesiumGS validator: TILE_REFINE_MISSING_IN_ROOT). Default REPLACE for OSGB mesh trees.
    match ensure_tileset_refine(out_dir) {
        Ok(n) if n > 0 => emitter.log(&format!(
            "[convert] set missing refine=REPLACE on {n} tileset root(s) with children"
        )),
        Ok(_) => {}
        Err(e) => emitter.log(&format!("[convert] refine normalize warning: {e}")),
    }
    Ok(())
}

/// Walk tileset.json tree under `out_dir` and set `root.refine = "REPLACE"` when missing
/// and the root has children (external or inline). Does not invent geometry.
fn ensure_tileset_refine(out_dir: &Path) -> Result<usize, String> {
    use serde_json::{json, Value};
    use std::fs;
    let mut fixed = 0usize;
    let mut stack = vec![out_dir.join("tileset.json")];
    let mut seen = std::collections::HashSet::new();
    while let Some(path) = stack.pop() {
        let canon = path.canonicalize().unwrap_or(path.clone());
        if !seen.insert(canon.clone()) {
            continue;
        }
        if !path.is_file() {
            continue;
        }
        let text = fs::read_to_string(&path).map_err(|e| e.to_string())?;
        let mut doc: Value = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        let Some(root) = doc.get_mut("root") else {
            continue;
        };
        let has_children = root
            .get("children")
            .and_then(|c| c.as_array())
            .map(|a| !a.is_empty())
            .unwrap_or(false);
        if has_children && root.get("refine").and_then(|v| v.as_str()).is_none() {
            root.as_object_mut()
                .ok_or_else(|| format!("root not object in {}", path.display()))?
                .insert("refine".into(), json!("REPLACE"));
            fs::write(&path, serde_json::to_string_pretty(&doc).map_err(|e| e.to_string())? + "
")
                .map_err(|e| e.to_string())?;
            fixed += 1;
        }
        // queue external tilesets referenced by content.uri
        let root_ref = doc.get("root").cloned().unwrap_or(Value::Null);
        let mut nodes = vec![root_ref];
        while let Some(node) = nodes.pop() {
            if let Some(uri) = node.pointer("/content/uri").and_then(|v| v.as_str()) {
                if uri.ends_with("tileset.json") || uri.contains("tileset.json?") {
                    let base = path.parent().unwrap_or(out_dir);
                    let child = base.join(uri.split('?').next().unwrap_or(uri));
                    if child.is_file() {
                        stack.push(child);
                    }
                }
            }
            if let Some(chs) = node.get("children").and_then(|c| c.as_array()) {
                nodes.extend(chs.iter().cloned());
            }
        }
    }
    Ok(fixed)
}
