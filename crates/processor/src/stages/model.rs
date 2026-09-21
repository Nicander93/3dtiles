//! Lightweight FBX/OBJ preflight. It never parses mesh data or returns it to
//! the desktop process; the native converter remains the authoritative parser.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};

pub fn scan_model(path: &str) -> Value {
    let input = PathBuf::from(path);
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    let format = input
        .extension()
        .and_then(|extension| extension.to_str())
        .map(str::to_ascii_lowercase);

    if !input.is_file() {
        errors.push(format!("Model file does not exist: {}", input.display()));
    }
    if !matches!(format.as_deref(), Some("fbx" | "obj")) {
        errors.push("Only .fbx and .obj model files are supported".into());
    }

    let mut materials = Vec::new();
    if format.as_deref() == Some("obj") && input.is_file() {
        match read_obj_material_libraries(&input) {
            Ok(libraries) if libraries.is_empty() => {
                warnings.push("OBJ has no mtllib reference; it will convert without MTL materials".into());
            }
            Ok(libraries) => {
                let root = input.parent().unwrap_or_else(|| Path::new("."));
                for library in libraries {
                    let resolved = root.join(&library);
                    let exists = resolved.is_file();
                    if !exists {
                        errors.push(format!("Referenced OBJ material file is missing: {}", resolved.display()));
                    }
                    let textures = if exists {
                        match read_mtl_texture_references(&resolved) {
                            Ok(references) => references
                                .into_iter()
                                .map(|reference| {
                                    let texture_path = resolved.parent().unwrap_or(root).join(&reference);
                                    let texture_exists = texture_path.is_file();
                                    if !texture_exists {
                                        errors.push(format!("Referenced MTL texture is missing: {}", texture_path.display()));
                                    }
                                    json!({ "reference": reference, "path": texture_path, "exists": texture_exists })
                                })
                                .collect::<Vec<_>>(),
                            Err(error) => {
                                errors.push(error);
                                Vec::new()
                            }
                        }
                    } else {
                        Vec::new()
                    };
                    materials.push(json!({
                        "reference": library,
                        "path": resolved,
                        "exists": exists,
                        "textures": textures,
                    }));
                }
            }
            Err(error) => errors.push(error),
        }
    }

    let bytes = input.metadata().ok().map(|metadata| metadata.len());
    let valid = errors.is_empty();
    json!({
        "ok": valid,
        "valid": valid,
        "path": input,
        "format": format,
        "errors": errors,
        "warnings": warnings,
        "summary": { "bytes": bytes, "materialLibraryCount": materials.len() },
        "materials": materials,
    })
}

fn read_mtl_texture_references(path: &Path) -> Result<Vec<String>, String> {
    const MAX_BYTES: u64 = 8 * 1024 * 1024;
    let metadata = fs::metadata(path).map_err(|error| format!("Cannot inspect MTL file: {error}"))?;
    if metadata.len() > MAX_BYTES {
        return Err(format!("MTL is too large for preflight texture scan (>{MAX_BYTES} bytes)"));
    }
    let text = fs::read_to_string(path).map_err(|error| format!("Cannot read MTL file: {error}"))?;
    Ok(text
        .lines()
        .filter_map(mtl_texture_reference)
        .collect())
}

fn mtl_texture_reference(line: &str) -> Option<String> {
    let line = line.split('#').next()?.trim();
    let (_, value) = line.split_once(char::is_whitespace)?;
    let command = line.split_whitespace().next()?.to_ascii_lowercase();
    if !matches!(command.as_str(), "map_kd" | "map_ks" | "map_d" | "map_bump" | "bump" | "norm" | "disp" | "decal") {
        return None;
    }
    let value = value.trim().trim_matches('"');
    (!value.is_empty()).then(|| value.to_string())
}

fn read_obj_material_libraries(path: &Path) -> Result<Vec<String>, String> {
    const MAX_BYTES: u64 = 8 * 1024 * 1024;
    let metadata = fs::metadata(path).map_err(|error| format!("Cannot inspect OBJ file: {error}"))?;
    if metadata.len() > MAX_BYTES {
        return Err(format!("OBJ is too large for preflight material scan (>{MAX_BYTES} bytes)"));
    }
    let text = fs::read_to_string(path).map_err(|error| format!("Cannot read OBJ file: {error}"))?;
    Ok(text
        .lines()
        .filter_map(|line| line.trim().strip_prefix("mtllib "))
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .collect())
}

#[cfg(test)]
mod tests {
    use super::scan_model;
    use std::fs;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn reports_missing_obj_material_library() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("geoforge-model-scan-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        let obj = root.join("building.obj");
        fs::write(&obj, "mtllib missing.mtl\nv 0 0 0\n").unwrap();
        let result = scan_model(obj.to_string_lossy().as_ref());
        assert!(!result["valid"].as_bool().unwrap_or(true));
        assert!(result["errors"][0].as_str().unwrap_or_default().contains("missing"));
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn reports_missing_mtl_texture() {
        let stamp = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("geoforge-model-texture-{stamp}"));
        fs::create_dir_all(&root).unwrap();
        let obj = root.join("building.obj");
        fs::write(&obj, "mtllib building.mtl\nv 0 0 0\n").unwrap();
        fs::write(root.join("building.mtl"), "newmtl wall\nmap_Kd missing.png\n").unwrap();
        let result = scan_model(obj.to_string_lossy().as_ref());
        assert!(!result["valid"].as_bool().unwrap_or(true));
        assert!(result["errors"]
            .as_array()
            .unwrap()
            .iter()
            .any(|error| error.as_str().unwrap_or_default().contains("MTL texture")));
        let _ = fs::remove_dir_all(root);
    }
}
