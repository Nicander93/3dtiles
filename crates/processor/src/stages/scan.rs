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
    let p = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let p = strip_verbatim(p);
    if p.file_name().and_then(|n| n.to_str()) == Some("Data")
        && p.parent()
            .map(|par| par.join("metadata.xml").is_file())
            .unwrap_or(false)
    {
        return (
            p.parent().unwrap().to_path_buf(),
            Some("Selected Data/ directory; normalized to dataset root.".into()),
        );
    }
    (p, None)
}

fn strip_verbatim(path: PathBuf) -> PathBuf {
    #[cfg(windows)]
    {
        let s = path.to_string_lossy();
        if let Some(rest) = s.strip_prefix(r"\\?\UNC\") {
            return PathBuf::from(format!(r"\\{rest}"));
        }
        if let Some(rest) = s.strip_prefix(r"\\?\") {
            return PathBuf::from(rest);
        }
    }
    path
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
            if let Some(error) = xml_parse_error(&text) {
                info["parseError"] = json!(format!("XML parse error: {error}"));
                return info;
            }
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

/// Validate the XML structure needed by the preflight without pulling an XML
/// tree into memory. Metadata files are small, but this still gives users a
/// useful distinction between malformed XML and a valid file with no SRS.
fn xml_parse_error(xml: &str) -> Option<String> {
    let mut stack: Vec<String> = Vec::new();
    let mut cursor = 0usize;
    let mut root_seen = false;
    let mut root_closed = false;

    while let Some(relative) = xml[cursor..].find('<') {
        let start = cursor + relative;
        let outside = &xml[cursor..start];
        if !root_seen && !outside.trim().is_empty() {
            return Some("non-whitespace text before the root element".into());
        }
        if root_closed && !outside.trim().is_empty() {
            return Some("non-whitespace text after the root element".into());
        }

        let (end, kind) = if xml[start..].starts_with("<!--") {
            let Some(offset) = xml[start + 4..].find("-->") else {
                return Some("unclosed XML comment".into());
            };
            let end = start + 4 + offset + 3;
            (end, "special")
        } else if xml[start..].starts_with("<![CDATA[") {
            let Some(offset) = xml[start + 9..].find("]]>") else {
                return Some("unclosed CDATA section".into());
            };
            let end = start + 9 + offset + 3;
            (end, "special")
        } else if xml[start..].starts_with("<?") {
            let Some(offset) = xml[start + 2..].find("?>") else {
                return Some("unclosed processing instruction".into());
            };
            let end = start + 2 + offset + 2;
            (end, "special")
        } else {
            let Some(offset) = find_tag_end(&xml[start + 1..]) else {
                return Some("unclosed XML tag".into());
            };
            let end = start + 1 + offset + 1;
            (end, "tag")
        };

        if kind == "special" {
            cursor = end;
            continue;
        }

        let raw = xml[start + 1..end - 1].trim();
        cursor = end;
        if raw.is_empty() || raw.starts_with('!') {
            continue;
        }
        if let Some(close) = raw.strip_prefix('/') {
            let name = close.split_whitespace().next().unwrap_or_default();
            let Some(actual) = stack.pop() else {
                return Some(format!("unexpected closing tag </{name}>"));
            };
            if actual != name {
                return Some(format!("closing tag </{name}> does not match <{actual}>"));
            }
            root_closed = stack.is_empty();
            continue;
        }

        let self_closing = raw.ends_with('/');
        let body = raw.trim_end_matches('/').trim();
        let name = body.split_whitespace().next().unwrap_or_default();
        if name.is_empty() {
            return Some("tag has no name".into());
        }
        if stack.is_empty() {
            if root_seen {
                return Some(format!("multiple root elements; found <{name}>"));
            }
            root_seen = true;
        }
        if !self_closing {
            stack.push(name.to_string());
        } else if stack.is_empty() {
            root_closed = true;
        }
    }

    if !xml[cursor..].trim().is_empty() {
        return Some("text after the final XML tag".into());
    }
    if let Some(name) = stack.last() {
        return Some(format!("unclosed tag <{name}>"));
    }
    if !root_seen {
        return Some("XML document has no root element".into());
    }
    None
}

fn find_tag_end(text: &str) -> Option<usize> {
    let mut quote = None;
    for (offset, ch) in text.char_indices() {
        match (quote, ch) {
            (Some(current), value) if value == current => quote = None,
            (None, '\'' | '"') => quote = Some(ch),
            (None, '>') => return Some(offset),
            _ => {}
        }
    }
    None
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
            errors.push(format!(
                "Cannot read metadata.xml under {}: {pe}",
                root.display()
            ));
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
        let mut children: Vec<_> = Vec::new();
        match fs::read_dir(&data_dir) {
            Ok(entries) => {
                for entry in entries {
                    match entry {
                        Ok(entry) if entry.path().is_dir() => children.push(entry),
                        Ok(_) => {}
                        Err(error) => errors.push(format!(
                            "Cannot read an entry under {}: {error}",
                            data_dir.display()
                        )),
                    }
                }
            }
            Err(error) => errors.push(format!(
                "Cannot read Data/ directory under {}: {error}",
                root.display()
            )),
        }
        children.sort_by_key(|e| e.file_name());

        for child in children {
            let name = child.file_name().to_string_lossy().into_owned();
            let entry = child.path().join(format!("{name}.osgb"));
            if !tile_dir_re().is_match(&name) && !entry.is_file() {
                warnings.push(format!("Non-standard directory under Data/: {name}"));
                continue;
            }
            if !tile_dir_re().is_match(&name) {
                warnings.push(format!(
                    "Non-standard directory under Data/: {name}; accepted for convert-only, rebuild may reject it"
                ));
            }
            let entry_ok = entry.is_file();
            let mut osgb_files: Vec<_> = Vec::new();
            match fs::read_dir(child.path()) {
                Ok(entries) => {
                    for entry in entries {
                        match entry {
                            Ok(entry)
                                if entry
                                    .path()
                                    .extension()
                                    .and_then(|x| x.to_str())
                                    .map(|x| x.eq_ignore_ascii_case("osgb"))
                                    .unwrap_or(false) =>
                            {
                                osgb_files.push(entry)
                            }
                            Ok(_) => {}
                            Err(error) => errors.push(format!(
                                "Cannot read an entry under tile {}: {error}",
                                child.path().display()
                            )),
                        }
                    }
                }
                Err(error) => errors.push(format!(
                    "Cannot read tile directory {}: {error}",
                    child.path().display()
                )),
            }
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
                "standardLayout": tile_dir_re().is_match(&name),
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

#[cfg(test)]
mod tests {
    use super::scan_osgb;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("geoforge-scan-{name}-{stamp}"));
        fs::create_dir_all(&root).expect("create scan fixture");
        root
    }

    #[test]
    fn reports_missing_tile_entry_and_keeps_nonstandard_warning() {
        let root = temp_root("invalid");
        fs::write(
            root.join("metadata.xml"),
            "<Metadata><SRS>ENU:120,30</SRS><SRSOrigin>0,0,0</SRSOrigin></Metadata>",
        )
        .expect("write metadata");
        fs::create_dir_all(root.join("Data/Tile_0_0")).expect("create tile");
        fs::create_dir_all(root.join("Data/Other")).expect("create nonstandard directory");

        let result = scan_osgb(root.to_string_lossy().as_ref());
        assert!(!result["valid"].as_bool().unwrap_or(true));
        assert!(result["errors"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|error| error.as_str().unwrap_or("").contains("Missing entry OSGB")));
        assert!(result["warnings"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|warning| warning.as_str().unwrap_or("").contains("Non-standard")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn accepts_utf8_dataset_paths_when_tile_layout_matches() {
        let parent = temp_root("unicode");
        let root = parent.join("数据集");
        fs::create_dir_all(&root).expect("create unicode dataset root");
        fs::write(
            root.join("metadata.xml"),
            "<Metadata><SRS>ENU:120,30</SRS><SRSOrigin>0,0,0</SRSOrigin></Metadata>",
        )
        .expect("write metadata");
        let tile = root.join("Data/Tile_1_2");
        fs::create_dir_all(&tile).expect("create tile");
        fs::write(tile.join("Tile_1_2.osgb"), b"fixture").expect("write osgb");

        let result = scan_osgb(root.to_string_lossy().as_ref());
        assert!(result["valid"].as_bool().unwrap_or(false));
        assert_eq!(result["tileCount"].as_u64(), Some(1));

        let _ = fs::remove_dir_all(parent);
    }

    #[test]
    fn includes_converter_compatible_nonstandard_tile_for_convert_only() {
        let root = temp_root("nonstandard-convert");
        fs::write(
            root.join("metadata.xml"),
            "<Metadata><SRS>ENU:120,30</SRS><SRSOrigin>0,0,0</SRSOrigin></Metadata>",
        )
        .expect("write metadata");
        let tile = root.join("Data/Tile_甲_乙");
        fs::create_dir_all(&tile).expect("create nonstandard tile");
        fs::write(tile.join("Tile_甲_乙.osgb"), b"fixture").expect("write osgb");

        let result = scan_osgb(root.to_string_lossy().as_ref());
        assert!(result["valid"].as_bool().unwrap_or(false));
        assert_eq!(result["tileCount"].as_u64(), Some(1));
        assert!(result["warnings"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|warning| warning
                .as_str()
                .unwrap_or("")
                .contains("accepted for convert-only")));

        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_malformed_metadata_xml_before_conversion() {
        let root = temp_root("bad-xml");
        fs::write(
            root.join("metadata.xml"),
            "<Metadata><SRS>ENU:120,30</SRS><SRSOrigin>0,0,0</Metadata>",
        )
        .expect("write malformed metadata");
        let tile = root.join("Data/Tile_1_2");
        fs::create_dir_all(&tile).expect("create tile");
        fs::write(tile.join("Tile_1_2.osgb"), b"fixture").expect("write osgb");

        let result = scan_osgb(root.to_string_lossy().as_ref());
        assert!(!result["valid"].as_bool().unwrap_or(true));
        assert!(result["errors"]
            .as_array()
            .unwrap_or(&Vec::new())
            .iter()
            .any(|error| error.as_str().unwrap_or("").contains("XML parse error")));

        let _ = fs::remove_dir_all(root);
    }
}
