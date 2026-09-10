use thiserror::Error;

/// Stable production error codes (Phase 11 / P0-1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErrorCode {
    ContentMissing,
    ContentInvalid,
    GltfInvalid,
    TextureMissing,
    SourceCoverageIncomplete,
    GridSpatialMismatch,
    Other,
}

impl ErrorCode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::ContentMissing => "CONTENT_MISSING",
            Self::ContentInvalid => "CONTENT_INVALID",
            Self::GltfInvalid => "GLTF_INVALID",
            Self::TextureMissing => "TEXTURE_MISSING",
            Self::SourceCoverageIncomplete => "SOURCE_COVERAGE_INCOMPLETE",
            Self::GridSpatialMismatch => "GRID_SPATIAL_MISMATCH",
            Self::Other => "OTHER",
        }
    }
}

impl std::fmt::Display for ErrorCode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Debug, Error)]
pub enum TopRebuildError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("GRID_SPATIAL_MISMATCH: {0}")]
    GridSpatialMismatch(String),

    #[error("NO_FURTHER_REDUCTION: level size did not shrink ({size})")]
    NoFurtherReduction { size: usize },

    #[error("no Tile_* source blocks found in tileset")]
    NoSourceBlocks,

    #[error("source block `{0}` has no representations")]
    NoRepresentations(String),

    #[error("invalid tileset: {0}")]
    InvalidTileset(String),

    #[error("budget exceeded: {0}")]
    BudgetExceeded(String),

    #[error("{code}: {message}")]
    Coded { code: ErrorCode, message: String },

    #[error("{0}")]
    Other(String),
}

impl TopRebuildError {
    pub fn coded(code: ErrorCode, message: impl Into<String>) -> Self {
        Self::Coded {
            code,
            message: message.into(),
        }
    }

    pub fn content_missing(message: impl Into<String>) -> Self {
        Self::coded(ErrorCode::ContentMissing, message)
    }

    pub fn content_invalid(message: impl Into<String>) -> Self {
        Self::coded(ErrorCode::ContentInvalid, message)
    }

    pub fn gltf_invalid(message: impl Into<String>) -> Self {
        Self::coded(ErrorCode::GltfInvalid, message)
    }

    pub fn texture_missing(message: impl Into<String>) -> Self {
        Self::coded(ErrorCode::TextureMissing, message)
    }

    pub fn source_coverage_incomplete(message: impl Into<String>) -> Self {
        Self::coded(ErrorCode::SourceCoverageIncomplete, message)
    }

    pub fn code(&self) -> Option<ErrorCode> {
        match self {
            Self::Coded { code, .. } => Some(*code),
            Self::GridSpatialMismatch(_) => Some(ErrorCode::GridSpatialMismatch),
            _ => None,
        }
    }

    pub fn code_str(&self) -> &str {
        match self {
            Self::Coded { code, .. } => code.as_str(),
            Self::GridSpatialMismatch(_) => "GRID_SPATIAL_MISMATCH",
            _ => "OTHER",
        }
    }
}

pub type Result<T> = std::result::Result<T, TopRebuildError>;
