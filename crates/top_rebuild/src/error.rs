use thiserror::Error;

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

    #[error("{0}")]
    Other(String),
}

pub type Result<T> = std::result::Result<T, TopRebuildError>;
