//! Layer A — built-in fast recursive 3D Tiles validator (Phase 12).
//!
//! Checks tileset JSON structure, URI resolution / cycle prevention, content
//! existence, and basic B3DM / GLB headers. Failures use stable error codes;
//! never silently pass a broken tree.

use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Stable Layer A error codes (processor validate / pre-commit).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ValidationCode {
    TilesetMissing,
    TilesetJsonInvalid,
    AssetMissing,
    RootMissing,
    GeometricErrorInvalid,
    RefineInvalid,
    BoundingVolumeMissing,
    BoundingVolumeInvalid,
    TransformInvalid,
    ContentUriMissing,
    ContentUriInvalid,
    ContentUriEscape,
    ContentMissing,
    ContentInvalid,
    ContentTooSmall,
    ExternalTilesetInvalid,
    CycleDetected,
    GeometricErrorMonotonicity,
}

impl ValidationCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::TilesetMissing => "TILESET_MISSING",
            Self::TilesetJsonInvalid => "TILESET_JSON_INVALID",
            Self::AssetMissing => "ASSET_MISSING",
            Self::RootMissing => "ROOT_MISSING",
            Self::GeometricErrorInvalid => "GEOMETRIC_ERROR_INVALID",
            Self::RefineInvalid => "REFINE_INVALID",
            Self::BoundingVolumeMissing => "BOUNDING_VOLUME_MISSING",
            Self::BoundingVolumeInvalid => "BOUNDING_VOLUME_INVALID",
            Self::TransformInvalid => "TRANSFORM_INVALID",
            Self::ContentUriMissing => "CONTENT_URI_MISSING",
            Self::ContentUriInvalid => "CONTENT_URI_INVALID",
            Self::ContentUriEscape => "CONTENT_URI_ESCAPE",
            Self::ContentMissing => "CONTENT_MISSING",
            Self::ContentInvalid => "CONTENT_INVALID",
            Self::ContentTooSmall => "CONTENT_TOO_SMALL",
            Self::ExternalTilesetInvalid => "EXTERNAL_TILESET_INVALID",
            Self::CycleDetected => "CYCLE_DETECTED",
            Self::GeometricErrorMonotonicity => "GEOMETRIC_ERROR_MONOTONICITY",
        }
    }
}

impl std::fmt::Display for ValidationCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationIssue {
    pub code: String,
    pub path: String,
    pub message: String,
    pub severity: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ValidationReport {
    pub ok: bool,
    pub layer: String,
    pub root: String,
    pub tileset_count: u64,
    pub content_count: u64,
    pub external_tileset_count: u64,
    pub error_count: u64,
    pub warning_count: u64,
    pub issues: Vec<ValidationIssue>,
}

impl ValidationReport {
    fn new(root: &Path) -> Self {
        Self {
            ok: true,
            layer: "A".into(),
            root: root.display().to_string(),
            tileset_count: 0,
            content_count: 0,
            external_tileset_count: 0,
            error_count: 0,
            warning_count: 0,
            issues: Vec::new(),
        }
    }

    fn push_error(&mut self, code: ValidationCode, path: &str, message: impl Into<String>) {
        self.ok = false;
        self.error_count += 1;
        self.issues.push(ValidationIssue {
            code: code.as_str().into(),
            path: path.into(),
            message: message.into(),
            severity: "error".into(),
        });
    }

    fn push_warning(&mut self, code: ValidationCode, path: &str, message: impl Into<String>) {
        self.warning_count += 1;
        self.issues.push(ValidationIssue {
            code: code.as_str().into(),
            path: path.into(),
            message: message.into(),
            severity: "warning".into(),
        });
    }

    pub fn first_error_summary(&self) -> Option<String> {
        self.issues
            .iter()
            .find(|i| i.severity == "error")
            .map(|i| format!("{}: {} ({})", i.code, i.message, i.path))
    }
}

/// Validate a tileset directory (expects `tileset.json` at root). Writes nothing.
pub fn validate_tileset_tree(dir: &Path) -> ValidationReport {
    let mut report = ValidationReport::new(dir);
    let root_tileset = dir.join("tileset.json");
    if !root_tileset.is_file() {
        report.push_error(
            ValidationCode::TilesetMissing,
            &root_tileset.display().to_string(),
            format!("tileset.json missing under {}", dir.display()),
        );
        return report;
    }
    let mut visited: HashSet<PathBuf> = HashSet::new();
    validate_tileset_file(dir, &root_tileset, &mut visited, &mut report, None);
    report
}

/// Same as [`validate_tileset_tree`] but also writes `validation_internal.json` under `dir`.
pub fn validate_and_write_report(dir: &Path) -> Result<ValidationReport, String> {
    let report = validate_tileset_tree(dir);
    let out = dir.join("validation_internal.json");
    let text = serde_json::to_string_pretty(&report).map_err(|e| e.to_string())?;
    fs::write(&out, text).map_err(|e| e.to_string())?;
    Ok(report)
}

fn validate_tileset_file(
    output_root: &Path,
    tileset_path: &Path,
    visited: &mut HashSet<PathBuf>,
    report: &mut ValidationReport,
    parent_ge: Option<f64>,
) {
    let canon = canonicalize_loose(tileset_path);
    if !visited.insert(canon.clone()) {
        report.push_error(
            ValidationCode::CycleDetected,
            &tileset_path.display().to_string(),
            format!("tileset cycle detected at {}", tileset_path.display()),
        );
        return;
    }
    report.tileset_count += 1;

    let text = match fs::read_to_string(tileset_path) {
        Ok(t) => t,
        Err(e) => {
            report.push_error(
                ValidationCode::TilesetJsonInvalid,
                &tileset_path.display().to_string(),
                format!("cannot read tileset: {e}"),
            );
            return;
        }
    };
    let value: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            report.push_error(
                ValidationCode::TilesetJsonInvalid,
                &tileset_path.display().to_string(),
                format!("tileset.json is not valid JSON: {e}"),
            );
            return;
        }
    };

    let path_s = tileset_path.display().to_string();
    if value.get("asset").is_none() {
        report.push_error(
            ValidationCode::AssetMissing,
            &path_s,
            "tileset missing required `asset`",
        );
    }
    let Some(root) = value.get("root") else {
        report.push_error(
            ValidationCode::RootMissing,
            &path_s,
            "tileset missing required `root`",
        );
        return;
    };

    let tileset_ge = read_geometric_error(value.get("geometricError"), &path_s, report);
    let base = tileset_path
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();

    visit_tile(
        output_root,
        &base,
        root,
        &format!("{path_s}#root"),
        visited,
        report,
        parent_ge.or(tileset_ge),
        /*is_root=*/ true,
    );
}

#[allow(clippy::too_many_arguments)]
fn visit_tile(
    output_root: &Path,
    base_dir: &Path,
    tile: &Value,
    loc: &str,
    visited: &mut HashSet<PathBuf>,
    report: &mut ValidationReport,
    parent_ge: Option<f64>,
    is_root: bool,
) {
    let ge = match read_geometric_error(tile.get("geometricError"), loc, report) {
        Some(g) => Some(g),
        None if is_root => parent_ge,
        None => {
            // Non-root tiles should carry geometricError; warn then continue.
            report.push_warning(
                ValidationCode::GeometricErrorInvalid,
                loc,
                "tile missing geometricError (inherited not validated)",
            );
            parent_ge
        }
    };

    if let (Some(p), Some(c)) = (parent_ge, ge) {
        // REPLACE trees: parent error should be >= child (allow equal for flat leaves).
        if let Some(refine) = tile.get("refine").and_then(|v| v.as_str()) {
            if refine.eq_ignore_ascii_case("REPLACE") && p + 1e-9 < c {
                report.push_warning(
                    ValidationCode::GeometricErrorMonotonicity,
                    loc,
                    format!("REPLACE parent geometricError {p} < child {c}"),
                );
            }
        } else if p + 1e-9 < c {
            // Inherited refine often REPLACE in our outputs.
            report.push_warning(
                ValidationCode::GeometricErrorMonotonicity,
                loc,
                format!("parent geometricError {p} < child {c}"),
            );
        }
    }

    if let Some(refine) = tile.get("refine") {
        match refine.as_str() {
            Some(s) if s.eq_ignore_ascii_case("REPLACE") || s.eq_ignore_ascii_case("ADD") => {}
            Some(s) => report.push_error(
                ValidationCode::RefineInvalid,
                loc,
                format!("refine must be REPLACE or ADD, got `{s}`"),
            ),
            None => report.push_error(
                ValidationCode::RefineInvalid,
                loc,
                "refine must be a string REPLACE or ADD",
            ),
        }
    }

    match tile.get("boundingVolume") {
        None => report.push_error(
            ValidationCode::BoundingVolumeMissing,
            loc,
            "tile missing boundingVolume",
        ),
        Some(bv) => {
            if !bounding_volume_ok(bv) {
                report.push_error(
                    ValidationCode::BoundingVolumeInvalid,
                    loc,
                    "boundingVolume must contain finite box[12], region[6], or sphere[4]",
                );
            }
        }
    }

    if let Some(tf) = tile.get("transform") {
        if !transform_ok(tf) {
            report.push_error(
                ValidationCode::TransformInvalid,
                loc,
                "transform must be length-16 array of finite numbers",
            );
        }
    }

    if let Some(content) = tile.get("content") {
        validate_content(output_root, base_dir, content, loc, visited, report);
    }
    // 3D Tiles 1.1 multiple contents
    if let Some(contents) = tile.get("contents").and_then(|v| v.as_array()) {
        for (i, c) in contents.iter().enumerate() {
            validate_content(
                output_root,
                base_dir,
                c,
                &format!("{loc}/contents[{i}]"),
                visited,
                report,
            );
        }
    }

    if let Some(children) = tile.get("children").and_then(|v| v.as_array()) {
        for (i, child) in children.iter().enumerate() {
            visit_tile(
                output_root,
                base_dir,
                child,
                &format!("{loc}/children[{i}]"),
                visited,
                report,
                ge.or(parent_ge),
                /*is_root=*/ false,
            );
        }
    }
}

fn validate_content(
    output_root: &Path,
    base_dir: &Path,
    content: &Value,
    loc: &str,
    visited: &mut HashSet<PathBuf>,
    report: &mut ValidationReport,
) {
    let Some(uri_v) = content.get("uri") else {
        report.push_error(
            ValidationCode::ContentUriMissing,
            loc,
            "content missing uri",
        );
        return;
    };
    let Some(uri) = uri_v.as_str() else {
        report.push_error(
            ValidationCode::ContentUriInvalid,
            loc,
            "content.uri must be a string",
        );
        return;
    };
    if uri.is_empty() {
        report.push_error(
            ValidationCode::ContentUriInvalid,
            loc,
            "content.uri is empty",
        );
        return;
    }
    // Skip absolute http(s) remote URIs for local Layer A.
    if uri.starts_with("http://") || uri.starts_with("https://") || uri.starts_with("data:") {
        report.push_warning(
            ValidationCode::ContentUriInvalid,
            loc,
            format!("skipping non-local content uri `{uri}`"),
        );
        return;
    }

    let resolved = match resolve_uri(base_dir, output_root, uri) {
        Ok(p) => p,
        Err(code) => {
            report.push_error(
                code,
                loc,
                format!("content uri `{uri}` rejected ({code})"),
            );
            return;
        }
    };

    let path_s = resolved.display().to_string();
    if !resolved.exists() {
        report.push_error(
            ValidationCode::ContentMissing,
            &path_s,
            format!("content file missing (uri=`{uri}`)"),
        );
        return;
    }

    let ext = resolved
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();

    if ext == "json" || looks_like_external_tileset(&resolved) {
        report.external_tileset_count += 1;
        validate_tileset_file(output_root, &resolved, visited, report, None);
        return;
    }

    report.content_count += 1;
    validate_content_bytes(&resolved, &path_s, report);
}

fn looks_like_external_tileset(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("tileset.json"))
        .unwrap_or(false)
}

fn validate_content_bytes(path: &Path, path_s: &str, report: &mut ValidationReport) {
    let meta = match fs::metadata(path) {
        Ok(m) => m,
        Err(e) => {
            report.push_error(
                ValidationCode::ContentMissing,
                path_s,
                format!("cannot stat content: {e}"),
            );
            return;
        }
    };
    // Minimum: B3DM header 28 bytes or GLB header 12 bytes.
    if meta.len() < 12 {
        report.push_error(
            ValidationCode::ContentTooSmall,
            path_s,
            format!("content too small ({} bytes)", meta.len()),
        );
        return;
    }

    let data = match fs::read(path) {
        Ok(d) => d,
        Err(e) => {
            report.push_error(
                ValidationCode::ContentInvalid,
                path_s,
                format!("cannot read content: {e}"),
            );
            return;
        }
    };

    if data.len() >= 4 && &data[0..4] == b"b3dm" {
        validate_b3dm_header(&data, path_s, report);
    } else if data.len() >= 4 && &data[0..4] == b"glTF" {
        validate_glb_header(&data, path_s, report);
    } else if data.len() >= 4 && &data[0..4] == b"i3dm" {
        // Acknowledge but only basic size check for V1.
        if data.len() < 32 {
            report.push_error(
                ValidationCode::ContentTooSmall,
                path_s,
                "i3dm header truncated",
            );
        }
    } else if data.len() >= 4 && (&data[0..4] == b"pnts" || &data[0..4] == b"cmpt") {
        // Allowed formats; no deep check in Layer A.
    } else {
        let ext = path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "glb" || ext == "b3dm" {
            report.push_error(
                ValidationCode::ContentInvalid,
                path_s,
                format!("expected {ext} magic, got {:?}", &data.get(0..4)),
            );
        } else {
            report.push_warning(
                ValidationCode::ContentInvalid,
                path_s,
                format!("unrecognized content magic {:?}; extension={ext}", &data.get(0..4)),
            );
        }
    }
}

fn validate_b3dm_header(data: &[u8], path_s: &str, report: &mut ValidationReport) {
    if data.len() < 28 {
        report.push_error(
            ValidationCode::ContentInvalid,
            path_s,
            "b3dm header truncated (<28 bytes)",
        );
        return;
    }
    let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
    if version != 1 {
        report.push_warning(
            ValidationCode::ContentInvalid,
            path_s,
            format!("unexpected b3dm version {version} (expected 1)"),
        );
    }
    let byte_length = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    if byte_length > 0 && byte_length > data.len() {
        report.push_error(
            ValidationCode::ContentInvalid,
            path_s,
            format!("b3dm byteLength {byte_length} > file length {}", data.len()),
        );
        return;
    }
    let ft_json = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
    let ft_bin = u32::from_le_bytes(data[16..20].try_into().unwrap()) as usize;
    let bt_json = u32::from_le_bytes(data[20..24].try_into().unwrap()) as usize;
    let bt_bin = u32::from_le_bytes(data[24..28].try_into().unwrap()) as usize;
    let offset = 28usize
        .saturating_add(ft_json)
        .saturating_add(ft_bin)
        .saturating_add(bt_json)
        .saturating_add(bt_bin);
    if offset > data.len() {
        report.push_error(
            ValidationCode::ContentInvalid,
            path_s,
            format!("b3dm feature/batch tables exceed file (offset={offset}, len={})", data.len()),
        );
        return;
    }
    // Embedded GLB (optional deep header check).
    if offset + 12 <= data.len() && &data[offset..offset + 4] == b"glTF" {
        validate_glb_header(&data[offset..], path_s, report);
    } else if offset < data.len() {
        // Search small pad window for glTF magic.
        let mut found = None;
        for pad in 0..8 {
            let o = offset + pad;
            if o + 4 <= data.len() && &data[o..o + 4] == b"glTF" {
                found = Some(o);
                break;
            }
        }
        if let Some(o) = found {
            validate_glb_header(&data[o..], path_s, report);
        } else {
            report.push_error(
                ValidationCode::ContentInvalid,
                path_s,
                "b3dm payload missing glTF magic",
            );
        }
    }
}

fn validate_glb_header(data: &[u8], path_s: &str, report: &mut ValidationReport) {
    if data.len() < 12 {
        report.push_error(
            ValidationCode::ContentInvalid,
            path_s,
            "glb header truncated (<12 bytes)",
        );
        return;
    }
    if &data[0..4] != b"glTF" {
        report.push_error(
            ValidationCode::ContentInvalid,
            path_s,
            "glb magic missing",
        );
        return;
    }
    let version = u32::from_le_bytes(data[4..8].try_into().unwrap());
    if version != 2 {
        report.push_error(
            ValidationCode::ContentInvalid,
            path_s,
            format!("glb version {version} (expected 2)"),
        );
        return;
    }
    let length = u32::from_le_bytes(data[8..12].try_into().unwrap()) as usize;
    if length > data.len() {
        report.push_error(
            ValidationCode::ContentInvalid,
            path_s,
            format!("glb length {length} > available {}", data.len()),
        );
        return;
    }
    if length < 12 {
        report.push_error(
            ValidationCode::ContentInvalid,
            path_s,
            "glb length field too small",
        );
        return;
    }
    // Basic first-chunk header if present.
    if data.len() >= 20 {
        let chunk_len = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
        let chunk_type = &data[16..20];
        if chunk_type != b"JSON" {
            report.push_warning(
                ValidationCode::ContentInvalid,
                path_s,
                format!("glb first chunk type {:?} (expected JSON)", chunk_type),
            );
        }
        if 20 + chunk_len > data.len() && 20 + chunk_len > length {
            report.push_error(
                ValidationCode::ContentInvalid,
                path_s,
                format!("glb JSON chunk length {chunk_len} exceeds file"),
            );
        }
    }
}

fn read_geometric_error(v: Option<&Value>, loc: &str, report: &mut ValidationReport) -> Option<f64> {
    let Some(v) = v else {
        return None;
    };
    let Some(n) = v.as_f64() else {
        report.push_error(
            ValidationCode::GeometricErrorInvalid,
            loc,
            "geometricError must be a finite number >= 0",
        );
        return None;
    };
    if !n.is_finite() || n < 0.0 {
        report.push_error(
            ValidationCode::GeometricErrorInvalid,
            loc,
            format!("geometricError must be finite and >= 0, got {n}"),
        );
        return None;
    }
    Some(n)
}

fn bounding_volume_ok(bv: &Value) -> bool {
    if let Some(box_v) = bv.get("box").and_then(|v| v.as_array()) {
        return box_v.len() == 12 && box_v.iter().all(|x| x.as_f64().map(|n| n.is_finite()).unwrap_or(false));
    }
    if let Some(region) = bv.get("region").and_then(|v| v.as_array()) {
        return region.len() == 6
            && region
                .iter()
                .all(|x| x.as_f64().map(|n| n.is_finite()).unwrap_or(false));
    }
    if let Some(sphere) = bv.get("sphere").and_then(|v| v.as_array()) {
        return sphere.len() == 4
            && sphere
                .iter()
                .all(|x| x.as_f64().map(|n| n.is_finite()).unwrap_or(false));
    }
    false
}

fn transform_ok(tf: &Value) -> bool {
    let Some(arr) = tf.as_array() else {
        return false;
    };
    arr.len() == 16 && arr.iter().all(|x| x.as_f64().map(|n| n.is_finite()).unwrap_or(false))
}

/// Resolve a relative content URI against `base_dir`, ensuring it stays under `output_root`.
fn resolve_uri(
    base_dir: &Path,
    output_root: &Path,
    uri: &str,
) -> Result<PathBuf, ValidationCode> {
    // Strip query/fragment if any.
    let clean = uri.split(['?', '#']).next().unwrap_or(uri);
    if Path::new(clean).is_absolute() {
        // Absolute local paths are allowed only if still under output_root.
        let p = PathBuf::from(clean);
        return ensure_under_root(output_root, &p);
    }
    // Normalize `.` / `..` without requiring the file to exist yet.
    let joined = base_dir.join(clean);
    let normalized = normalize_path(&joined);
    ensure_under_root(output_root, &normalized)
}

fn ensure_under_root(output_root: &Path, path: &Path) -> Result<PathBuf, ValidationCode> {
    let root_n = normalize_path(output_root);
    let path_n = normalize_path(path);
    if path_n.starts_with(&root_n) {
        Ok(path_n)
    } else {
        // Also compare canonical when both exist.
        match (fs::canonicalize(output_root), fs::canonicalize(path)) {
            (Ok(r), Ok(p)) if p.starts_with(&r) => Ok(p),
            (Ok(r), Err(_)) => {
                // File may not exist; compare normalized strings under root.
                if path_n.starts_with(normalize_path(&r)) {
                    Ok(path_n)
                } else {
                    Err(ValidationCode::ContentUriEscape)
                }
            }
            _ => Err(ValidationCode::ContentUriEscape),
        }
    }
}

fn normalize_path(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for c in path.components() {
        match c {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            Component::RootDir => out.push(c.as_os_str()),
            Component::Normal(s) => out.push(s),
            Component::Prefix(p) => out.push(p.as_os_str()),
        }
    }
    out
}

fn canonicalize_loose(path: &Path) -> PathBuf {
    fs::canonicalize(path).unwrap_or_else(|_| normalize_path(path))
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use std::io::Write;

    fn write(path: &Path, body: &str) {
        if let Some(p) = path.parent() {
            fs::create_dir_all(p).unwrap();
        }
        fs::write(path, body).unwrap();
    }

    fn minimal_tileset(content_uri: &str) -> String {
        format!(
            r#"{{
  "asset": {{ "version": "1.0" }},
  "geometricError": 100.0,
  "root": {{
    "boundingVolume": {{ "box": [0,0,0, 1,0,0, 0,1,0, 0,0,1] }},
    "geometricError": 50.0,
    "refine": "REPLACE",
    "content": {{ "uri": "{content_uri}" }}
  }}
}}"#
        )
    }

    /// Minimal GLB: header + empty-ish JSON chunk (not a full mesh; Layer A header only).
    fn tiny_glb() -> Vec<u8> {
        let json = br#"{"asset":{"version":"2.0"}}"#;
        let json_pad = {
            let mut v = json.to_vec();
            while v.len() % 4 != 0 {
                v.push(b' ');
            }
            v
        };
        let length = 12 + 8 + json_pad.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"glTF");
        out.extend_from_slice(&2u32.to_le_bytes());
        out.extend_from_slice(&(length as u32).to_le_bytes());
        out.extend_from_slice(&(json_pad.len() as u32).to_le_bytes());
        out.extend_from_slice(b"JSON");
        out.extend_from_slice(&json_pad);
        out
    }

    fn tiny_b3dm() -> Vec<u8> {
        let glb = tiny_glb();
        let ftj = b"{\"BATCH_LENGTH\":0}  "; // 18+2 = 20, %4 == 0
        let total = 28 + ftj.len() + glb.len();
        let mut out = Vec::new();
        out.extend_from_slice(b"b3dm");
        out.extend_from_slice(&1u32.to_le_bytes());
        out.extend_from_slice(&(total as u32).to_le_bytes());
        out.extend_from_slice(&(ftj.len() as u32).to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(&0u32.to_le_bytes());
        out.extend_from_slice(ftj);
        out.extend_from_slice(&glb);
        out
    }

    #[test]
    fn catches_missing_content() {
        let dir = std::env::temp_dir().join("layer_a_missing_content");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        write(&dir.join("tileset.json"), &minimal_tileset("./gone.b3dm"));
        let report = validate_tileset_tree(&dir);
        assert!(!report.ok);
        assert!(
            report.issues.iter().any(|i| i.code == "CONTENT_MISSING"),
            "{:?}",
            report.issues
        );
    }

    #[test]
    fn accepts_valid_b3dm_tree() {
        let dir = std::env::temp_dir().join("layer_a_ok_b3dm");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        write(&dir.join("tileset.json"), &minimal_tileset("./tile.b3dm"));
        let mut f = fs::File::create(dir.join("tile.b3dm")).unwrap();
        f.write_all(&tiny_b3dm()).unwrap();
        let report = validate_tileset_tree(&dir);
        assert!(report.ok, "{:?}", report.issues);
        assert_eq!(report.content_count, 1);
    }

    #[test]
    fn catches_uri_escape() {
        let dir = std::env::temp_dir().join("layer_a_escape");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        write(
            &dir.join("tileset.json"),
            &minimal_tileset("../../etc/passwd"),
        );
        let report = validate_tileset_tree(&dir);
        assert!(!report.ok);
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.code == "CONTENT_URI_ESCAPE"),
            "{:?}",
            report.issues
        );
    }

    #[test]
    fn catches_cycle() {
        let dir = std::env::temp_dir().join("layer_a_cycle");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(dir.join("a")).unwrap();
        fs::create_dir_all(dir.join("b")).unwrap();
        write(
            &dir.join("tileset.json"),
            &minimal_tileset("./a/tileset.json"),
        );
        write(
            &dir.join("a/tileset.json"),
            &minimal_tileset("../b/tileset.json"),
        );
        write(
            &dir.join("b/tileset.json"),
            &minimal_tileset("../a/tileset.json"),
        );
        let report = validate_tileset_tree(&dir);
        assert!(!report.ok);
        assert!(
            report.issues.iter().any(|i| i.code == "CYCLE_DETECTED"),
            "{:?}",
            report.issues
        );
    }

    #[test]
    fn catches_bad_geometric_error() {
        let dir = std::env::temp_dir().join("layer_a_bad_ge");
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        write(
            &dir.join("tileset.json"),
            r#"{
  "asset": {"version":"1.0"},
  "geometricError": -1,
  "root": {
    "boundingVolume": {"box":[0,0,0,1,0,0,0,1,0,0,0,1]},
    "geometricError": 1,
    "refine": "REPLACE"
  }
}"#,
        );
        let report = validate_tileset_tree(&dir);
        assert!(!report.ok);
        assert!(
            report
                .issues
                .iter()
                .any(|i| i.code == "GEOMETRIC_ERROR_INVALID"),
            "{:?}",
            report.issues
        );
    }
}
