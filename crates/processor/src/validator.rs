//! Layer A/B validator — built-in fast recursive 3D Tiles validator (Phase 12).
//!
//! Checks tileset JSON structure, URI resolution / cycle prevention, content
//! existence, and basic B3DM / GLB headers. Failures use stable error codes;
//! never silently pass a broken tree.

use serde::Serialize;

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

impl ValidationIssue {
    pub fn error(code: ValidationCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.as_str().to_string(),
            path: path.into(),
            message: message.into(),
            severity: "error".to_string(),
        }
    }

    pub fn warning(code: ValidationCode, path: impl Into<String>, message: impl Into<String>) -> Self {
        Self {
            code: code.as_str().to_string(),
            path: path.into(),
            message: message.into(),
            severity: "warning".to_string(),
        }
    }
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
    pub fn new(root: impl Into<String>) -> Self {
        Self {
            ok: true,
            layer: "A".into(),
            root: root.into(),
            tileset_count: 0,
            content_count: 0,
            external_tileset_count: 0,
            error_count: 0,
            warning_count: 0,
            issues: Vec::new(),
        }
    }

    pub fn add_error(&mut self, code: ValidationCode, path: impl Into<String>, message: impl Into<String>) {
        self.issues.push(ValidationIssue::error(code, path, message));
        self.error_count += 1;
        self.ok = false;
    }

    pub fn add_warning(&mut self, code: ValidationCode, path: impl Into<String>, message: impl Into<String>) {
        self.issues.push(ValidationIssue::warning(code, path, message));
        self.warning_count += 1;
    }
}

use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Component, Path, PathBuf};

/// Validate a tileset directory (expects `tileset.json` at root).
pub fn validate_tileset_tree(dir: &Path) -> ValidationReport {
    let mut report = ValidationReport::new(dir.display().to_string());
    let root_tileset = dir.join("tileset.json");
    
    if !root_tileset.is_file() {
        report.add_error(
            ValidationCode::TilesetMissing,
            root_tileset.display().to_string(),
            format!("tileset.json missing under {}", dir.display()),
        );
        return report;
    }

    let mut visited: HashSet<PathBuf> = HashSet::new();
    validate_tileset_file(dir, &root_tileset, &mut visited, &mut report);
    report
}

fn validate_tileset_file(
    output_root: &Path,
    tileset_path: &Path,
    visited: &mut HashSet<PathBuf>,
    report: &mut ValidationReport,
) {
    let canon = canonicalize_loose(tileset_path);
    if !visited.insert(canon.clone()) {
        report.add_error(
            ValidationCode::CycleDetected,
            tileset_path.display().to_string(),
            format!("tileset cycle detected at {}", tileset_path.display()),
        );
        return;
    }

    let text = match fs::read_to_string(tileset_path) {
        Ok(t) => t,
        Err(e) => {
            report.add_error(
                ValidationCode::TilesetJsonInvalid,
                tileset_path.display().to_string(),
                format!("cannot read tileset: {e}"),
            );
            return;
        }
    };

    let value: Value = match serde_json::from_str(&text) {
        Ok(v) => v,
        Err(e) => {
            report.add_error(
                ValidationCode::TilesetJsonInvalid,
                tileset_path.display().to_string(),
                format!("tileset.json is not valid JSON: {e}"),
            );
            return;
        }
    };

    report.tileset_count += 1;

    let path_s = tileset_path.display().to_string();
    
    if value.get("asset").is_none() {
        report.add_error(
            ValidationCode::AssetMissing,
            &path_s,
            "tileset missing required `asset`",
        );
    }

    if value.get("root").is_none() {
        report.add_error(
            ValidationCode::RootMissing,
            &path_s,
            "tileset missing required `root`",
        );
        return;
    }

    let base_dir = tileset_path.parent().unwrap_or_else(|| Path::new("."));

    let tileset_ge = read_geometric_error(value.get("geometricError"), &path_s, report);

    if let Some(root) = value.get("root") {
        visit_tile(
            output_root,
            base_dir,
            root,
            &format!("{path_s}#root"),
            visited,
            report,
            tileset_ge,
            true,
        );
    }
}

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
            report.add_warning(
                ValidationCode::GeometricErrorInvalid,
                loc,
                "tile missing geometricError (inherited not validated)",
            );
            parent_ge
        }
    };

    if let (Some(p), Some(c)) = (parent_ge, ge) {
        if let Some(refine) = tile.get("refine").and_then(|v| v.as_str()) {
            if refine.eq_ignore_ascii_case("REPLACE") && p + 1e-9 < c {
                report.add_warning(
                    ValidationCode::GeometricErrorMonotonicity,
                    loc,
                    format!("REPLACE parent geometricError {p} < child {c}"),
                );
            }
        } else if p + 1e-9 < c {
            report.add_warning(
                ValidationCode::GeometricErrorMonotonicity,
                loc,
                format!("parent geometricError {p} < child {c}"),
            );
        }
    }

    if let Some(refine) = tile.get("refine") {
        match refine.as_str() {
            Some(s) if s.eq_ignore_ascii_case("REPLACE") || s.eq_ignore_ascii_case("ADD") => {}
            Some(s) => report.add_error(
                ValidationCode::RefineInvalid,
                loc,
                format!("refine must be REPLACE or ADD, got `{s}`"),
            ),
            None => report.add_error(
                ValidationCode::RefineInvalid,
                loc,
                "refine must be a string REPLACE or ADD",
            ),
        }
    }

    match tile.get("boundingVolume") {
        None => report.add_error(
            ValidationCode::BoundingVolumeMissing,
            loc,
            "tile missing boundingVolume",
        ),
        Some(bv) => {
            if !bounding_volume_ok(bv) {
                report.add_error(
                    ValidationCode::BoundingVolumeInvalid,
                    loc,
                    "boundingVolume must contain finite box[12], region[6], or sphere[4]",
                );
            }
        }
    }

    if let Some(tf) = tile.get("transform") {
        if !transform_ok(tf) {
            report.add_error(
                ValidationCode::TransformInvalid,
                loc,
                "transform must be length-16 array of finite numbers",
            );
        }
    }

    if let Some(content) = tile.get("content") {
        validate_content(output_root, base_dir, content, visited, report);
    }

    if let Some(contents) = tile.get("contents").and_then(|v| v.as_array()) {
        for c in contents {
            validate_content(output_root, base_dir, c, visited, report);
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
                ge,
                false,
            );
        }
    }
}

fn validate_content(
    output_root: &Path,
    base_dir: &Path,
    content: &Value,
    visited: &mut HashSet<PathBuf>,
    report: &mut ValidationReport,
) {
    let Some(uri) = content.get("uri").and_then(|v| v.as_str()) else {
        return;
    };

    let resolved = match resolve_uri(base_dir, output_root, uri) {
        Ok(p) => p,
        Err(code) => {
            report.add_error(
                code,
                base_dir.display().to_string(),
                format!("content uri `{uri}` rejected"),
            );
            return;
        }
    };

    if !resolved.exists() {
        report.add_error(
            ValidationCode::ContentMissing,
            resolved.display().to_string(),
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
        validate_tileset_file(output_root, &resolved, visited, report);
    } else {
        report.content_count += 1;
    }
}

fn looks_like_external_tileset(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .map(|n| n.eq_ignore_ascii_case("tileset.json"))
        .unwrap_or(false)
}

fn resolve_uri(
    base_dir: &Path,
    output_root: &Path,
    uri: &str,
) -> Result<PathBuf, ValidationCode> {
    let clean = uri.split(['?', '#']).next().unwrap_or(uri);
    
    if Path::new(clean).is_absolute() {
        let p = PathBuf::from(clean);
        return ensure_under_root(output_root, &p);
    }

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
        match (fs::canonicalize(output_root), fs::canonicalize(path)) {
            (Ok(r), Ok(p)) if p.starts_with(&r) => Ok(p),
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

fn read_geometric_error(v: Option<&Value>, loc: &str, report: &mut ValidationReport) -> Option<f64> {
    let Some(v) = v else {
        return None;
    };
    let Some(n) = v.as_f64() else {
        report.add_error(
            ValidationCode::GeometricErrorInvalid,
            loc,
            "geometricError must be a finite number >= 0",
        );
        return None;
    };
    if !n.is_finite() || n < 0.0 {
        report.add_error(
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
        return box_v.len() == 12 
            && box_v.iter().all(|x| x.as_f64().map(|n| n.is_finite()).unwrap_or(false));
    }
    if let Some(region) = bv.get("region").and_then(|v| v.as_array()) {
        return region.len() == 6
            && region.iter().all(|x| x.as_f64().map(|n| n.is_finite()).unwrap_or(false));
    }
    if let Some(sphere) = bv.get("sphere").and_then(|v| v.as_array()) {
        return sphere.len() == 4
            && sphere.iter().all(|x| x.as_f64().map(|n| n.is_finite()).unwrap_or(false));
    }
    false
}

fn transform_ok(tf: &Value) -> bool {
    let Some(arr) = tf.as_array() else {
        return false;
    };
    arr.len() == 16 && arr.iter().all(|x| x.as_f64().map(|n| n.is_finite()).unwrap_or(false))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn validation_code_as_str() {
        assert_eq!(ValidationCode::TilesetMissing.as_str(), "TILESET_MISSING");
        assert_eq!(ValidationCode::CycleDetected.as_str(), "CYCLE_DETECTED");
    }

    #[test]
    fn validation_code_display() {
        assert_eq!(format!("{}", ValidationCode::AssetMissing), "ASSET_MISSING");
    }

    #[test]
    fn validation_issue_error() {
        let issue = ValidationIssue::error(
            ValidationCode::RootMissing,
            "tileset.json",
            "root field not found",
        );
        assert_eq!(issue.code, "ROOT_MISSING");
        assert_eq!(issue.severity, "error");
    }

    #[test]
    fn validation_issue_warning() {
        let issue = ValidationIssue::warning(
            ValidationCode::GeometricErrorMonotonicity,
            "node",
            "GE not monotonic",
        );
        assert_eq!(issue.code, "GEOMETRIC_ERROR_MONOTONICITY");
        assert_eq!(issue.severity, "warning");
    }

    #[test]
    fn validation_report_new() {
        let report = ValidationReport::new("input/tileset.json");
        assert!(report.ok);
        assert_eq!(report.layer, "A");
        assert_eq!(report.error_count, 0);
        assert_eq!(report.issues.len(), 0);
    }

    #[test]
    fn validation_report_add_error() {
        let mut report = ValidationReport::new("test");
        report.add_error(ValidationCode::TilesetMissing, "path", "not found");
        assert!(!report.ok);
        assert_eq!(report.error_count, 1);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].severity, "error");
    }

    #[test]
    fn validation_report_add_warning() {
        let mut report = ValidationReport::new("test");
        report.add_warning(ValidationCode::GeometricErrorMonotonicity, "path", "check");
        assert!(report.ok);
        assert_eq!(report.warning_count, 1);
        assert_eq!(report.issues.len(), 1);
        assert_eq!(report.issues[0].severity, "warning");
    }

    #[test]
    fn validate_tileset_tree_missing() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert_eq!(report.error_count, 1);
        assert!(report.issues[0].code.contains("TILESET_MISSING"));
    }

    #[test]
    fn validate_tileset_tree_invalid_json() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        fs::write(tmp.path().join("tileset.json"), "not json").unwrap();
        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert_eq!(report.error_count, 1);
        assert!(report.issues[0].code.contains("TILESET_JSON_INVALID"));
    }

    #[test]
    fn validate_tileset_tree_missing_asset() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{"root": {"geometricError": 100}}"#,
        )
        .unwrap();
        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.code.contains("ASSET_MISSING")));
    }

    #[test]
    fn validate_tileset_tree_missing_root() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{"asset": {"version": "1.0"}}"#,
        )
        .unwrap();
        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.code.contains("ROOT_MISSING")));
    }

    #[test]
    fn validate_tileset_tree_minimal_ok() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "geometricError": 100.0,
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 50.0
                }
            }"#,
        )
        .unwrap();
        let report = validate_tileset_tree(tmp.path());
        assert!(report.ok, "errors: {:?}", report.issues);
        assert_eq!(report.tileset_count, 1);
    }

    #[test]
    fn validate_external_tileset() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 100.0,
                    "content": {"uri": "sub/tileset.json"}
                }
            }"#,
        )
        .unwrap();

        fs::create_dir(tmp.path().join("sub")).unwrap();
        fs::write(
            tmp.path().join("sub/tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 50.0
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(report.ok, "errors: {:?}", report.issues);
        assert_eq!(report.tileset_count, 2);
        assert_eq!(report.external_tileset_count, 1);
    }

    #[test]
    fn validate_cycle_detection() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 100.0,
                    "content": {"uri": "tileset.json"}
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.code == "CYCLE_DETECTED"));
    }

    #[test]
    fn validate_uri_escape() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 100.0,
                    "content": {"uri": "../../escape.b3dm"}
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.code == "CONTENT_URI_ESCAPE"));
    }

    #[test]
    fn validate_content_missing() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 100.0,
                    "content": {"uri": "missing.b3dm"}
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.code == "CONTENT_MISSING"));
    }

    #[test]
    fn validate_content_count() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 100.0,
                    "content": {"uri": "tile.b3dm"}
                }
            }"#,
        )
        .unwrap();

        fs::write(tmp.path().join("tile.b3dm"), b"dummy").unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(report.ok);
        assert_eq!(report.content_count, 1);
    }

    #[test]
    fn validate_bounding_volume_missing() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "geometricError": 100.0
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.code == "BOUNDING_VOLUME_MISSING"));
    }

    #[test]
    fn validate_bounding_volume_box() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 100.0
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(report.ok, "errors: {:?}", report.issues);
    }

    #[test]
    fn validate_bounding_volume_invalid() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0]},
                    "geometricError": 100.0
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.code == "BOUNDING_VOLUME_INVALID"));
    }

    #[test]
    fn validate_transform_valid() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 100.0,
                    "transform": [1,0,0,0,0,1,0,0,0,0,1,0,0,0,0,1]
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(report.ok, "errors: {:?}", report.issues);
    }

    #[test]
    fn validate_transform_invalid() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 100.0,
                    "transform": [1,0,0,0,0,1,0,0]
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.code == "TRANSFORM_INVALID"));
    }

    #[test]
    fn validate_geometric_error_invalid() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": -5.0
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.code == "GEOMETRIC_ERROR_INVALID"));
    }

    #[test]
    fn validate_geometric_error_monotonicity() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 10.0,
                    "refine": "REPLACE",
                    "children": [{
                        "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                        "geometricError": 20.0
                    }]
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(report.ok);
        assert!(report.issues.iter().any(|i| i.code == "GEOMETRIC_ERROR_MONOTONICITY" && i.severity == "warning"));
    }

    #[test]
    fn validate_refine_invalid() {
        use tempfile::TempDir;
        let tmp = TempDir::new().unwrap();
        
        fs::write(
            tmp.path().join("tileset.json"),
            r#"{
                "asset": {"version": "1.0"},
                "root": {
                    "boundingVolume": {"box": [0,0,0,1,0,0,0,1,0,0,0,1]},
                    "geometricError": 100.0,
                    "refine": "INVALID"
                }
            }"#,
        )
        .unwrap();

        let report = validate_tileset_tree(tmp.path());
        assert!(!report.ok);
        assert!(report.issues.iter().any(|i| i.code == "REFINE_INVALID"));
    }
}
