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
    
    match realign_b3dm_under(out_dir) {
        Ok(n) if n > 0 => emitter.log(&format!(
            "[convert] realigned {n} b3dm file(s) for 8-byte table/GLB alignment (Layer B)"
        )),
        Ok(_) => {}
        Err(e) => emitter.log(&format!("[convert] b3dm realign warning: {e}")),
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

/// Pad b3dm feature/batch tables so GLB starts on an 8-byte boundary (CesiumGS Layer B).
fn realign_one_b3dm(data: &[u8]) -> Result<Option<Vec<u8>>, String> {
    if data.len() < 28 || &data[0..4] != b"b3dm" {
        return Ok(None);
    }
    let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
    let ft_json = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
    let ft_bin = u32::from_le_bytes(data[16..20].try_into().unwrap()) as usize;
    let bt_json = u32::from_le_bytes(data[20..24].try_into().unwrap()) as usize;
    let bt_bin = u32::from_le_bytes(data[24..28].try_into().unwrap()) as usize;
    let mut offset = 28 + ft_json + ft_bin + bt_json + bt_bin;
    if offset > data.len() {
        return Err(format!("b3dm tables exceed file (offset={offset})"));
    }
    if offset + 4 <= data.len() && &data[offset..offset + 4] != b"glTF" {
        let mut found = None;
        for pad in 0..8 {
            let o = offset + pad;
            if o + 4 <= data.len() && &data[o..o + 4] == b"glTF" {
                found = Some(o);
                break;
            }
        }
        let Some(o) = found else { return Ok(None); };
        offset = o;
    }
    let glb = if offset + 12 <= data.len() && &data[offset..offset + 4] == b"glTF" {
        let glb_len = u32::from_le_bytes(data[offset + 8..offset + 12].try_into().unwrap()) as usize;
        if glb_len >= 12 && offset + glb_len <= data.len() {
            data[offset..offset + glb_len].to_vec()
        } else {
            data[offset..].to_vec()
        }
    } else {
        data[offset..].to_vec()
    };
    let mut ftj = data[28..28 + ft_json].to_vec();
    let mut ftb = data[28 + ft_json..28 + ft_json + ft_bin].to_vec();
    let mut btj = data[28 + ft_json + ft_bin..28 + ft_json + ft_bin + bt_json].to_vec();
    let mut btb = data[28 + ft_json + ft_bin + bt_json..28 + ft_json + ft_bin + bt_json + bt_bin].to_vec();
    while (28 + ftj.len()) % 8 != 0 { ftj.push(b' '); }
    while (28 + ftj.len() + ftb.len()) % 8 != 0 { ftb.push(0); }
    while (28 + ftj.len() + ftb.len() + btj.len()) % 8 != 0 { btj.push(b' '); }
    while (28 + ftj.len() + ftb.len() + btj.len() + btb.len()) % 8 != 0 { btb.push(0); }
    let new_offset = 28 + ftj.len() + ftb.len() + btj.len() + btb.len();
    let already = ftj.len() == ft_json && ftb.len() == ft_bin && btj.len() == bt_json && btb.len() == bt_bin && offset == new_offset;
    if already {
        return Ok(None);
    }
    let total = new_offset + glb.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"b3dm");
    out.extend_from_slice(&version.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(ftj.len() as u32).to_le_bytes());
    out.extend_from_slice(&(ftb.len() as u32).to_le_bytes());
    out.extend_from_slice(&(btj.len() as u32).to_le_bytes());
    out.extend_from_slice(&(btb.len() as u32).to_le_bytes());
    out.extend_from_slice(&ftj);
    out.extend_from_slice(&ftb);
    out.extend_from_slice(&btj);
    out.extend_from_slice(&btb);
    out.extend_from_slice(&glb);
    // 3D Tiles: tile body / byteLength must be 8-byte aligned.
    while out.len() % 8 != 0 {
        out.push(0);
    }
    let total = out.len() as u32;
    out[8..12].copy_from_slice(&total.to_le_bytes());
    Ok(Some(out))
}

fn realign_b3dm_under(out_dir: &Path) -> Result<usize, String> {
    use std::fs;
    let mut n = 0usize;
    let mut stack = vec![out_dir.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let rd = match fs::read_dir(&dir) {
            Ok(r) => r,
            Err(_) => continue,
        };
        for ent in rd.flatten() {
            let p = ent.path();
            if p.is_dir() {
                stack.push(p);
            } else if p.extension().and_then(|e| e.to_str()).map(|e| e.eq_ignore_ascii_case("b3dm")).unwrap_or(false) {
                let data = fs::read(&p).map_err(|e| format!("{}: {e}", p.display()))?;
                if let Some(fixed) = realign_one_b3dm(&data)? {
                    fs::write(&p, fixed).map_err(|e| format!("{}: {e}", p.display()))?;
                    n += 1;
                }
            }
        }
    }
    Ok(n)
}
