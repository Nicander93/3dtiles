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
}
