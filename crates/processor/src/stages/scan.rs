//! OSGB scan / tileset input precheck (ported from desktop_server osgb_scan.py).

use regex::Regex;
use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

fn tile_dir_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)^Tile_[+\-]?\d+_[+\-]?\d+$").unwrap())
}

fn normalize_root(path: &Path) -> (PathBuf, Option<String>) {
    let p = path
        .canonicalize()
        .unwrap_or_else(|_| path.to_path_buf());
    if p.file_name().and_then(|n| n.to_str()) == Some("Data")
        && p.parent().map(|par| par.join("metadata.xml").is_file()).unwrap_or(false)
    {
        return (
            p.parent().unwrap().to_path_buf(),
            Some("Selected Data/ directory; normalized to dataset root.".into()),
        );
    }
    (p, None)
}

fn parse_metadata(meta_path: &Path) -> Value {
    let mut info = json!({
        "path": meta_path.to_string_lossy(),
        "srs": Value::Null,
        "srsOrigin": Value::Null,
        "rawHead": "",
    });
    match fs::read_to_string(meta_path) {
        Ok(text) => {
            let head: String = text.chars().take(1500).collect();
            info["rawHead"] = json!(head);
            // Lightweight XML tag extract (same fields Python ElementTree uses).
            if let Some(srs) = extract_tag(&text, "SRS") {
                info["srs"] = json!(srs);
            }
            if let Some(origin) = extract_tag(&text, "SRSOrigin") {
                info["srsOrigin"] = json!(origin);
            }
        }
        Err(e) => {
            info["parseError"] = json!(e.to_string());
        }
    }
    info
}

fn extract_tag(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    let val = xml[start..end].trim();
    if val.is_empty() {
        None
    } else {
        Some(val.to_string())
    }
}

/// Validate OSGB dataset root (metadata.xml + Data/Tile_*/same-name.osgb).
pub fn scan_osgb(path: &str) -> Value {
    let raw = PathBuf::from(path);
    let expanded = if path.starts_with('~') {
        // best-effort; expanduser not always available
        raw
    } else {
        raw
    };

    if !expanded.exists() {
        return json!({
            "ok": false,
            "valid": false,
            "path": expanded.to_string_lossy(),
            "errors": [format!("Path does not exist: {}", expanded.display())],
            "warnings": [],
            "summary": {},
        });
    }

    let (root, note) = normalize_root(&expanded);
    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();
    if let Some(n) = note {
        warnings.push(n);
    }

    let meta_path = root.join("metadata.xml");
    let data_dir = root.join("Data");
    if !meta_path.is_file() {
        errors.push(format!("Missing metadata.xml under {}", root.display()));
    }
    if !data_dir.is_dir() {
        errors.push(format!("Missing Data/ directory under {}", root.display()));
    }

    let mut metadata = json!({});
    if meta_path.is_file() {
        metadata = parse_metadata(&meta_path);
        if let Some(pe) = metadata.get("parseError").and_then(|v| v.as_str()) {
            warnings.push(format!("metadata.xml parse issue: {pe}"));
        }
        if metadata.get("srs").and_then(|v| v.as_str()).is_none() {
            warnings.push(
                "metadata.xml has no SRS; local preview ok, geographic export needs CRS.".into(),
            );
        }
    }

    let mut tiles: Vec<Value> = Vec::new();
    let mut missing_entry: Vec<String> = Vec::new();
    let mut total_osgb: u64 = 0;
    let mut total_bytes: u64 = 0;

    if data_dir.is_dir() {
        let mut children: Vec<_> = fs::read_dir(&data_dir)
            .into_iter()
            .flatten()
            .flatten()
            .filter(|e| e.path().is_dir())
            .collect();
        children.sort_by_key(|e| e.file_name());

        for child in children {
            let name = child.file_name().to_string_lossy().into_owned();
            if !tile_dir_re().is_match(&name) {
                warnings.push(format!("Non-standard directory under Data/: {name}"));
                continue;
            }
            let entry = child.path().join(format!("{name}.osgb"));
            let entry_ok = entry.is_file();
            let osgb_files: Vec<_> = fs::read_dir(child.path())
                .into_iter()
                .flatten()
                .flatten()
                .filter(|e| {
                    e.path()
                        .extension()
                        .and_then(|x| x.to_str())
                        .map(|x| x.eq_ignore_ascii_case("osgb"))
                        .unwrap_or(false)
                })
                .collect();
            let file_count = osgb_files.len() as u64;
            let size: u64 = osgb_files
                .iter()
                .filter_map(|e| e.metadata().ok().map(|m| m.len()))
                .sum();
            total_osgb += file_count;
            total_bytes += size;
            if !entry_ok {
                missing_entry.push(entry.to_string_lossy().into_owned());
            }
            tiles.push(json!({
                "name": name,
                "path": child.path().to_string_lossy(),
                "entryOsgb": entry.to_string_lossy(),
                "entryExists": entry_ok,
                "osgbCount": file_count,
                "bytes": size,
            }));
        }
    }

    for m in missing_entry.iter().take(20) {
        errors.push(format!("Missing entry OSGB (must match folder name): {m}"));
    }
    if missing_entry.len() > 20 {
        errors.push(format!(
            "... and {} more missing entry files",
            missing_entry.len() - 20
        ));
    }
    if data_dir.is_dir() && tiles.is_empty() {
        errors.push("No Tile_* directories found under Data/".into());
    }

    let valid = errors.is_empty();
    let summary = json!({
        "root": root.to_string_lossy(),
        "tileCount": tiles.len(),
        "osgbFileCount": total_osgb,
        "totalBytes": total_bytes,
        "srs": metadata.get("srs").cloned().unwrap_or(Value::Null),
        "srsOrigin": metadata.get("srsOrigin").cloned().unwrap_or(Value::Null),
        "tiles": tiles.iter().map(|t| json!({
            "name": t.get("name"),
            "osgbCount": t.get("osgbCount"),
            "entryExists": t.get("entryExists"),
        })).collect::<Vec<_>>(),
    });

    json!({
        "ok": valid,
        "valid": valid,
        "path": root.to_string_lossy(),
        "requestedPath": expanded.to_string_lossy(),
        "errors": errors,
        "warnings": warnings,
        "metadata": metadata,
        "summary": summary,
        "tiles": tiles,
        "hasMetadata": meta_path.is_file(),
        "hasDataDir": data_dir.is_dir(),
        "tileCount": summary.get("tileCount"),
        "unitHint": crate::geo::unit_hint(
            metadata.get("srs").and_then(|v| v.as_str())
        ),
    })
}

/// Resolve tileset.json path from a directory or file path.
pub fn resolve_tileset(input_path: &str) -> Result<PathBuf, String> {
    let inp = PathBuf::from(input_path);
    let tileset = if inp.is_dir() {
        inp.join("tileset.json")
    } else {
        inp
    };
    if !tileset.is_file() {
        return Err(format!("tileset.json not found at {}", tileset.display()));
    }
    Ok(tileset)
}
