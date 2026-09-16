//! Output / input tileset validation before commit (T02).

use crate::cancel::CancelFlag;
use crate::protocol::{Emitter, Stage};
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

const MAX_DEPTH: u32 = 64;
const MAX_TILESETS: usize = 10_000;

#[derive(Debug)]
pub struct ValidateError {
    pub code: String,
    pub path: String,
    pub message: String,
}

impl ValidateError {
    fn new(code: &str, path: &Path, message: impl Into<String>) -> Self {
        Self {
            code: code.into(),
            path: path.to_string_lossy().into_owned(),
            message: message.into(),
        }
    }

    fn display(&self) -> String {
        format!("{} [{}]: {}", self.code, self.path, self.message)
    }
}

pub fn validate_tileset_dir(emitter: &Emitter, dir: &Path) -> Result<(), String> {
    validate_tileset_dir_cancellable(emitter, dir, None)
}

pub fn validate_tileset_dir_cancellable(
    emitter: &Emitter,
    dir: &Path,
    cancel: Option<&CancelFlag>,
) -> Result<(), String> {
    emitter.stage(Stage::Validate, "Verifying output");
    let root = dir.join("tileset.json");
    if !root.is_file() {
        return Err(format!("tileset.json missing under {}", dir.display()));
    }
    let mut visited = HashSet::new();
    walk_tileset(emitter, dir, &root, &mut visited, 0, cancel)?;
    emitter.log(&format!("[validate] OK {}", root.display()));
    Ok(())
}

fn check_cancel(cancel: Option<&CancelFlag>) -> Result<(), String> {
    if cancel.map(|c| c.is_cancelled()).unwrap_or(false) {
        Err("cancelled".into())
    } else {
        Ok(())
    }
}

fn walk_tileset(
    emitter: &Emitter,
    data_root: &Path,
    tileset_path: &Path,
    visited: &mut HashSet<PathBuf>,
    depth: u32,
    cancel: Option<&CancelFlag>,
) -> Result<(), String> {
    check_cancel(cancel)?;
    if depth > MAX_DEPTH {
        return Err(ValidateError::new(
            "TILESET_DEPTH",
            tileset_path,
            format!("tileset nesting exceeds {MAX_DEPTH}"),
        )
        .display());
    }
    if visited.len() > MAX_TILESETS {
        return Err(ValidateError::new(
            "TILESET_COUNT",
            tileset_path,
            format!("too many tileset files (>{MAX_TILESETS})"),
        )
        .display());
    }
    let key = canonicalize_soft(tileset_path);
    if !visited.insert(key.clone()) {
        return Err(
            ValidateError::new("TILESET_CYCLE", tileset_path, "cyclic tileset reference").display(),
        );
    }

    let text = fs::read_to_string(tileset_path)
        .map_err(|e| ValidateError::new("TILESET_READ", tileset_path, e.to_string()).display())?;
    let v: Value = serde_json::from_str(&text)
        .map_err(|e| ValidateError::new("TILESET_JSON", tileset_path, e.to_string()).display())?;

    if !v.is_object() {
        return Err(ValidateError::new(
            "TILESET_SHAPE",
            tileset_path,
            "root must be a JSON object",
        )
        .display());
    }
    let asset = v.get("asset").ok_or_else(|| {
        ValidateError::new("TILESET_ASSET", tileset_path, "missing asset").display()
    })?;
    let version = asset.get("version").and_then(|x| x.as_str()).unwrap_or("");
    if version.is_empty() {
        return Err(
            ValidateError::new("TILESET_VERSION", tileset_path, "asset.version required").display(),
        );
    }
    let root = v.get("root").ok_or_else(|| {
        ValidateError::new("TILESET_ROOT", tileset_path, "missing root").display()
    })?;

    if v.get("extensionsRequired")
        .and_then(|e| e.as_array())
        .map(|a| !a.is_empty())
        .unwrap_or(false)
    {
        let names: Vec<String> = v["extensionsRequired"]
            .as_array()
            .unwrap()
            .iter()
            .filter_map(|x| x.as_str().map(|s| s.to_string()))
            .collect();
        // Allow known empty / none; reject unknown required extensions for V1.
        for n in &names {
            if n != "3DTILES_content_gltf" && !n.starts_with("KHR_") {
                return Err(ValidateError::new(
                    "UNSUPPORTED_EXTENSION",
                    tileset_path,
                    format!("unsupported extensionsRequired: {n}"),
                )
                .display());
            }
        }
    }

    let base_dir = tileset_path.parent().unwrap_or(data_root);
    visit_tile(emitter, data_root, base_dir, root, visited, depth, cancel)?;
    Ok(())
}

fn visit_tile(
    emitter: &Emitter,
    data_root: &Path,
    base_dir: &Path,
    tile: &Value,
    visited: &mut HashSet<PathBuf>,
    depth: u32,
    cancel: Option<&CancelFlag>,
) -> Result<(), String> {
    check_cancel(cancel)?;
    if !tile.is_object() {
        return Err(ValidateError::new("TILE_SHAPE", base_dir, "tile must be object").display());
    }
    if tile.get("contents").is_some() {
        return Err(ValidateError::new(
            "UNSUPPORTED_CONTENTS",
            base_dir,
            "multiple contents[] not supported in V1",
        )
        .display());
    }
    if tile.get("implicitTiling").is_some() {
        return Err(ValidateError::new(
            "UNSUPPORTED_IMPLICIT",
            base_dir,
            "implicitTiling not supported in V1",
        )
        .display());
    }
    let geometric_error = tile.get("geometricError").ok_or_else(|| {
        ValidateError::new("BAD_NUMBER", base_dir, "geometricError required").display()
    })?;
    check_finite_number(Some(geometric_error), "geometricError", base_dir)?;
    if let Some(b) = tile.get("boundingVolume") {
        check_bounding_volume(b, base_dir)?;
    }
    if let Some(t) = tile.get("transform") {
        check_transform(t, base_dir)?;
    }

    if let Some(content) = tile.get("content") {
        let uri = content
            .get("uri")
            .or_else(|| content.get("url"))
            .and_then(|u| u.as_str())
            .ok_or_else(|| {
                ValidateError::new("CONTENT_URI", base_dir, "content.uri/url missing").display()
            })?;
        resolve_and_check_content(emitter, data_root, base_dir, uri, visited, depth, cancel)?;
    }

    if let Some(children) = tile.get("children").and_then(|c| c.as_array()) {
        for child in children {
            visit_tile(emitter, data_root, base_dir, child, visited, depth, cancel)?;
        }
    }
    Ok(())
}

fn resolve_and_check_content(
    emitter: &Emitter,
    data_root: &Path,
    base_dir: &Path,
    uri: &str,
    visited: &mut HashSet<PathBuf>,
    depth: u32,
    cancel: Option<&CancelFlag>,
) -> Result<(), String> {
    if uri.starts_with("http://") || uri.starts_with("https://") || uri.starts_with("//") {
        return Err(ValidateError::new(
            "REMOTE_URI",
            base_dir,
            format!("remote content URI not supported: {uri}"),
        )
        .display());
    }
    let joined = if Path::new(uri).is_absolute() {
        PathBuf::from(uri)
    } else {
        base_dir.join(uri)
    };
    let resolved = normalize_join(&joined);
    if !is_under_root(data_root, &resolved) {
        return Err(
            ValidateError::new("PATH_ESCAPE", &resolved, "content URI escapes data root").display(),
        );
    }
    if !resolved.is_file() {
        return Err(
            ValidateError::new("MISSING_CONTENT", &resolved, "referenced file missing").display(),
        );
    }

    let lower = resolved
        .file_name()
        .and_then(|s| s.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if lower == "tileset.json" || lower.ends_with(".json") && looks_like_external_tileset(&resolved)
    {
        walk_tileset(emitter, data_root, &resolved, visited, depth + 1, cancel)?;
        return Ok(());
    }
    let ext = resolved
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    match ext.as_str() {
        "b3dm" => check_b3dm(&resolved)?,
        "glb" => {
            let data = fs::read(&resolved).map_err(|e| e.to_string())?;
            check_glb_bytes(&resolved, &data)?;
        }
        "gltf" => check_gltf_external(data_root, &resolved)?,
        "i3dm" | "pnts" | "cmpt" => {
            emitter.log(&format!(
                "[validate] skip deep check for .{ext}: {}",
                resolved.display()
            ));
        }
        _ => {
            emitter.log(&format!(
                "[validate] content ok (untyped): {}",
                resolved.display()
            ));
        }
    }
    Ok(())
}

fn looks_like_external_tileset(path: &Path) -> bool {
    let Ok(text) = fs::read_to_string(path) else {
        return false;
    };
    let Ok(v) = serde_json::from_str::<Value>(&text) else {
        return false;
    };
    v.get("root").is_some() && v.get("asset").is_some()
}

fn check_finite_number(v: Option<&Value>, name: &str, ctx: &Path) -> Result<(), String> {
    if let Some(value) = v {
        let n = value.as_f64().ok_or_else(|| {
            ValidateError::new("BAD_NUMBER", ctx, format!("{name} must be a number")).display()
        })?;
        if !n.is_finite() {
            return Err(
                ValidateError::new("BAD_NUMBER", ctx, format!("{name} must be finite")).display(),
            );
        }
    }
    Ok(())
}

fn check_bounding_volume(b: &Value, ctx: &Path) -> Result<(), String> {
    let object = b.as_object().ok_or_else(|| {
        ValidateError::new("BAD_BOUNDS", ctx, "boundingVolume must be an object").display()
    })?;
    let variants = ["box", "region", "sphere"]
        .iter()
        .filter(|key| object.contains_key(**key))
        .count();
    if variants != 1 {
        return Err(ValidateError::new(
            "BAD_BOUNDS",
            ctx,
            "boundingVolume must contain exactly one of box, region, sphere",
        )
        .display());
    }
    for (key, expected) in [("box", 12usize), ("region", 6), ("sphere", 4)] {
        let Some(value) = object.get(key) else {
            continue;
        };
        let values = value.as_array().ok_or_else(|| {
            ValidateError::new("BAD_BOUNDS", ctx, format!("{key} must be an array")).display()
        })?;
        if values.len() != expected {
            return Err(ValidateError::new(
                "BAD_BOUNDS",
                ctx,
                format!("{key} must have {expected} values"),
            )
            .display());
        }
        for value in values {
            let number = value.as_f64().ok_or_else(|| {
                ValidateError::new("BAD_BOUNDS", ctx, format!("{key} values must be numbers"))
                    .display()
            })?;
            if !number.is_finite() {
                return Err(ValidateError::new(
                    "BAD_BOUNDS",
                    ctx,
                    format!("{key} has non-finite value"),
                )
                .display());
            }
        }
    }
    Ok(())
}

fn check_transform(t: &Value, ctx: &Path) -> Result<(), String> {
    let arr = t.as_array().ok_or_else(|| {
        ValidateError::new("BAD_TRANSFORM", ctx, "transform must be array").display()
    })?;
    if arr.len() != 16 {
        return Err(
            ValidateError::new("BAD_TRANSFORM", ctx, "transform must have 16 values").display(),
        );
    }
    for n in arr {
        let f = n.as_f64().ok_or_else(|| {
            ValidateError::new("BAD_TRANSFORM", ctx, "transform values must be numbers").display()
        })?;
        if !f.is_finite() {
            return Err(
                ValidateError::new("BAD_TRANSFORM", ctx, "transform has non-finite").display(),
            );
        }
    }
    Ok(())
}

fn check_b3dm(path: &Path) -> Result<(), String> {
    let mut f = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut header = [0u8; 28];
    f.read_exact(&mut header).map_err(|e| {
        ValidateError::new("B3DM_HEADER", path, format!("truncated: {e}")).display()
    })?;
    if &header[0..4] != b"b3dm" {
        return Err(ValidateError::new("B3DM_MAGIC", path, "not b3dm").display());
    }
    let version = u32::from_le_bytes(header[4..8].try_into().unwrap());
    if version != 1 {
        return Err(ValidateError::new(
            "B3DM_VERSION",
            path,
            format!("unsupported version {version}"),
        )
        .display());
    }
    let byte_length = u32::from_le_bytes(header[8..12].try_into().unwrap()) as usize;
    let ft_json = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;
    let ft_bin = u32::from_le_bytes(header[16..20].try_into().unwrap()) as usize;
    let bt_json = u32::from_le_bytes(header[20..24].try_into().unwrap()) as usize;
    let bt_bin = u32::from_le_bytes(header[24..28].try_into().unwrap()) as usize;
    let meta = fs::metadata(path).map_err(|e| e.to_string())?;
    let file_len = meta.len() as usize;
    if byte_length > 0 && byte_length > file_len {
        return Err(ValidateError::new(
            "B3DM_LENGTH",
            path,
            format!("byteLength {byte_length} > file {file_len}"),
        )
        .display());
    }
    let tables = 28usize
        .checked_add(ft_json)
        .and_then(|x| x.checked_add(ft_bin))
        .and_then(|x| x.checked_add(bt_json))
        .and_then(|x| x.checked_add(bt_bin))
        .ok_or_else(|| ValidateError::new("B3DM_TABLES", path, "table size overflow").display())?;
    if tables > file_len {
        return Err(
            ValidateError::new("B3DM_TABLES", path, "feature/batch tables exceed file").display(),
        );
    }
    // Read embedded GLB if present
    let data = fs::read(path).map_err(|e| e.to_string())?;
    if tables + 12 <= data.len() && &data[tables..tables + 4] == b"glTF" {
        check_glb_bytes(path, &data[tables..])?;
    } else if let Some(off) = find_gltf_magic(&data[tables..]) {
        check_glb_bytes(path, &data[tables + off..])?;
    }
    Ok(())
}

fn find_gltf_magic(data: &[u8]) -> Option<usize> {
    for pad in 0..8 {
        if pad + 4 <= data.len() && &data[pad..pad + 4] == b"glTF" {
            return Some(pad);
        }
    }
    None
}

fn check_glb_bytes(path: &Path, glb: &[u8]) -> Result<(), String> {
    if glb.len() < 20 {
        return Err(ValidateError::new("GLB_SHORT", path, "GLB too short").display());
    }
    if &glb[0..4] != b"glTF" {
        return Err(ValidateError::new("GLB_MAGIC", path, "not glTF").display());
    }
    let version = u32::from_le_bytes(glb[4..8].try_into().unwrap());
    if version != 2 {
        return Err(ValidateError::new(
            "GLB_VERSION",
            path,
            format!("unsupported GLB version {version}"),
        )
        .display());
    }
    let total = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    if total > glb.len() {
        return Err(ValidateError::new(
            "GLB_LENGTH",
            path,
            format!("declared length {total} > buffer {}", glb.len()),
        )
        .display());
    }
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    if &glb[16..20] != b"JSON" {
        return Err(ValidateError::new("GLB_CHUNK", path, "missing JSON chunk").display());
    }
    let json_end = 20usize
        .checked_add(json_len)
        .filter(|e| *e <= glb.len())
        .ok_or_else(|| ValidateError::new("GLB_JSON", path, "JSON chunk OOB").display())?;
    let root: Value = serde_json::from_slice(&glb[20..json_end])
        .map_err(|e| ValidateError::new("GLB_JSON", path, e.to_string()).display())?;
    let bin_start = if json_end < glb.len() {
        // optional BIN chunk
        if json_end + 8 <= glb.len() && &glb[json_end + 4..json_end + 8] == b"BIN\0" {
            let bin_len =
                u32::from_le_bytes(glb[json_end..json_end + 4].try_into().unwrap()) as usize;
            let bin_data_start = json_end + 8;
            let bin_data_end = bin_data_start
                .checked_add(bin_len)
                .filter(|e| *e <= glb.len())
                .ok_or_else(|| ValidateError::new("GLB_BIN", path, "BIN chunk OOB").display())?;
            Some((bin_data_start, bin_data_end))
        } else {
            None
        }
    } else {
        None
    };

    if let Some(views) = root.get("bufferViews").and_then(|v| v.as_array()) {
        for (i, view) in views.iter().enumerate() {
            let offset = view.get("byteOffset").and_then(|x| x.as_u64()).unwrap_or(0) as usize;
            let length = view
                .get("byteLength")
                .and_then(|x| x.as_u64())
                .ok_or_else(|| {
                    ValidateError::new(
                        "BUFFER_VIEW",
                        path,
                        format!("bufferView[{i}] missing byteLength"),
                    )
                    .display()
                })? as usize;
            if let Some((bs, be)) = bin_start {
                let end = offset.checked_add(length).ok_or_else(|| {
                    ValidateError::new("BUFFER_VIEW", path, format!("bufferView[{i}] overflow"))
                        .display()
                })?;
                let bin_len = be - bs;
                if end > bin_len {
                    return Err(ValidateError::new(
                        "BUFFER_VIEW",
                        path,
                        format!("bufferView[{i}] exceeds BIN ({end} > {bin_len})"),
                    )
                    .display());
                }
            }
        }
    }
    Ok(())
}

fn check_gltf_external(data_root: &Path, path: &Path) -> Result<(), String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let root: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let base = path.parent().unwrap_or_else(|| Path::new("."));
    if let Some(buffers) = root.get("buffers").and_then(|b| b.as_array()) {
        for (i, buf) in buffers.iter().enumerate() {
            if let Some(uri) = buf.get("uri").and_then(|u| u.as_str()) {
                if uri.starts_with("data:") {
                    continue;
                }
                if uri.starts_with("http") {
                    return Err(ValidateError::new(
                        "REMOTE_URI",
                        path,
                        format!("remote buffer[{i}]"),
                    )
                    .display());
                }
                let p = normalize_join(&base.join(uri));
                if !is_under_root(data_root, &p) {
                    return Err(ValidateError::new(
                        "PATH_ESCAPE",
                        &p,
                        format!("gltf buffer[{i}] escapes data root"),
                    )
                    .display());
                }
                if !p.is_file() {
                    return Err(ValidateError::new(
                        "MISSING_BUFFER",
                        &p,
                        format!("gltf buffer[{i}] missing"),
                    )
                    .display());
                }
            }
        }
    }
    if let Some(images) = root.get("images").and_then(|b| b.as_array()) {
        for (i, img) in images.iter().enumerate() {
            if let Some(uri) = img.get("uri").and_then(|u| u.as_str()) {
                if uri.starts_with("data:") {
                    continue;
                }
                if uri.starts_with("http") {
                    return Err(ValidateError::new(
                        "REMOTE_URI",
                        path,
                        format!("remote image[{i}]"),
                    )
                    .display());
                }
                let p = normalize_join(&base.join(uri));
                if !is_under_root(data_root, &p) {
                    return Err(ValidateError::new(
                        "PATH_ESCAPE",
                        &p,
                        format!("gltf image[{i}] escapes data root"),
                    )
                    .display());
                }
                if !p.is_file() {
                    return Err(ValidateError::new(
                        "MISSING_IMAGE",
                        &p,
                        format!("gltf image[{i}] missing"),
                    )
                    .display());
                }
            }
        }
    }
    if let Some(req) = root.get("extensionsRequired").and_then(|e| e.as_array()) {
        for ext in req {
            let name = ext.as_str().unwrap_or("");
            if name == "KHR_draco_mesh_compression" {
                return Err(ValidateError::new(
                    "UNSUPPORTED_EXTENSION",
                    path,
                    "KHR_draco_mesh_compression required but not supported for process path",
                )
                .display());
            }
        }
    }
    Ok(())
}

fn canonicalize_soft(p: &Path) -> PathBuf {
    let c = fs::canonicalize(p).unwrap_or_else(|_| p.to_path_buf());
    strip_verbatim(c)
}

fn strip_verbatim(p: PathBuf) -> PathBuf {
    let s = p.to_string_lossy();
    #[cfg(windows)]
    {
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    let _ = s;
    p
}

fn normalize_join(p: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in p.components() {
        match c {
            std::path::Component::CurDir => {}
            std::path::Component::ParentDir => {
                let _ = out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    strip_verbatim(out)
}

fn is_under_root(root: &Path, child: &Path) -> bool {
    let r = canonicalize_soft(root);
    let c = if child.exists() {
        canonicalize_soft(child)
    } else {
        normalize_join(child)
    };
    #[cfg(windows)]
    let eq = |a: &Path, b: &Path| {
        a.to_string_lossy()
            .eq_ignore_ascii_case(&b.to_string_lossy())
    };
    #[cfg(not(windows))]
    let eq = |a: &Path, b: &Path| a == b;

    if eq(&r, &c) {
        return true;
    }
    let mut cur = c.parent();
    while let Some(p) = cur {
        if eq(p, &r) {
            return true;
        }
        cur = p.parent();
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::Emitter;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn tmp() -> PathBuf {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let p = std::env::temp_dir().join(format!("gf-val-{n}"));
        let _ = fs::remove_dir_all(&p);
        fs::create_dir_all(&p).unwrap();
        p
    }

    #[test]
    fn rejects_missing_root() {
        let dir = tmp();
        fs::write(dir.join("tileset.json"), r#"{"asset":{"version":"1.0"}}"#).unwrap();
        let e = Emitter::new("t");
        let err = validate_tileset_dir(&e, &dir).unwrap_err();
        assert!(
            err.contains("TILESET_ROOT") || err.contains("missing root"),
            "{err}"
        );
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn accepts_minimal_ok() {
        let dir = tmp();
        // tiny valid GLB: header + empty JSON {}
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        let json = b"{}".to_vec();
        let mut json_pad = json.clone();
        while json_pad.len() % 4 != 0 {
            json_pad.push(b' ');
        }
        let total = 12 + 8 + json_pad.len();
        glb.extend_from_slice(&(total as u32).to_le_bytes());
        glb.extend_from_slice(&(json_pad.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(&json_pad);
        fs::write(dir.join("tile.glb"), &glb).unwrap();
        let tileset = format!(
            r#"{{"asset":{{"version":"1.0"}},"root":{{"geometricError":1,"boundingVolume":{{"box":[0,0,0,1,0,0,0,1,0,0,0,1]}},"content":{{"uri":"tile.glb"}}}}}}"#
        );
        fs::write(dir.join("tileset.json"), tileset).unwrap();
        let e = Emitter::new("t");
        validate_tileset_dir(&e, &dir).unwrap();
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_missing_content() {
        let dir = tmp();
        let tileset = r#"{"asset":{"version":"1.0"},"root":{"geometricError":1,"content":{"uri":"missing.b3dm"}}}"#;
        fs::write(dir.join("tileset.json"), tileset).unwrap();
        let e = Emitter::new("t");
        let err = validate_tileset_dir(&e, &dir).unwrap_err();
        assert!(err.contains("MISSING_CONTENT"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_invalid_bounding_volume_shape() {
        let dir = tmp();
        let tileset = r#"{"asset":{"version":"1.0"},"root":{"geometricError":1,"boundingVolume":{"box":[0,1]}}}"#;
        fs::write(dir.join("tileset.json"), tileset).unwrap();
        let e = Emitter::new("t");
        let err = validate_tileset_dir(&e, &dir).unwrap_err();
        assert!(err.contains("BAD_BOUNDS"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_gltf_external_path_escape() {
        let dir = tmp();
        fs::write(dir.join("outside.bin"), b"outside").unwrap();
        fs::write(
            dir.join("nested.gltf"),
            r#"{"asset":{"version":"2.0"},"buffers":[{"uri":"../outside.bin","byteLength":7}]}"#,
        )
        .unwrap();
        fs::write(
            dir.join("tileset.json"),
            r#"{"asset":{"version":"1.0"},"root":{"geometricError":1,"content":{"uri":"nested.gltf"}}}"#,
        )
        .unwrap();
        let e = Emitter::new("t");
        let err = validate_tileset_dir(&e, &dir).unwrap_err();
        assert!(err.contains("PATH_ESCAPE"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn rejects_cycle() {
        let dir = tmp();
        let a = dir.join("a.json");
        let b = dir.join("b.json");
        fs::write(
            &a,
            r#"{"asset":{"version":"1.0"},"root":{"geometricError":1,"content":{"uri":"b.json"}}}"#,
        )
        .unwrap();
        fs::write(
            &b,
            r#"{"asset":{"version":"1.0"},"root":{"geometricError":1,"content":{"uri":"a.json"}}}"#,
        )
        .unwrap();
        fs::write(
            dir.join("tileset.json"),
            r#"{"asset":{"version":"1.0"},"root":{"geometricError":1,"content":{"uri":"a.json"}}}"#,
        )
        .unwrap();
        let e = Emitter::new("t");
        let err = validate_tileset_dir(&e, &dir).unwrap_err();
        assert!(err.contains("CYCLE"), "{err}");
        let _ = fs::remove_dir_all(&dir);
    }
}
