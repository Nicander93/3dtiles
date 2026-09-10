//! TopRebuild core: SourceBlock / Representation / TreeBuilder / ProxyBuilder /
//! TilesetWriter HLOD + Phase 8 boundary/texture/budget metrics + Phase 9 release CLI.
//!
//! Phase 9: `top_rebuild` release binary is the Processor default rebuild engine.
//! Python baseline remains under tools/experiments/rebuild_top_py for regression only
//! (`GEOFORGE_REBUILD_ENGINE=python`). No atlas/weld. No large-area claim (Phase 10).

pub mod adapter;
pub mod b3dm;
pub mod dump;
pub mod error;
pub mod gap;
pub mod glb;
pub mod proxy_builder;
pub mod selector;
pub mod texture;
pub mod tileset_writer;
pub mod tree_builder;
pub mod types;

pub use adapter::load_source_blocks;
pub use dump::{format_level_counts, format_tree_detail, print_acceptance};
pub use error::{Result, TopRebuildError};
pub use gap::{compute_gap_metrics, GapMetrics, GapPair};
pub use proxy_builder::{
    build_proxy, build_proxy_from_paths, build_proxy_to_file, to_parent_local, ChildContent,
    ProxyBudget, ProxyBuildResult,
};
pub use selector::{select_for_blocks, select_one, Selection, DEFAULT_SOURCE_ERROR_RATIO};
pub use texture::{find_basisu, process_textures, ProcessedTexture, TextureData, TextureMetrics};
pub use tileset_writer::{
    assert_world_transform_invariant, box_diagonal, bv_world_to_local, geometric_error_proxy,
    probe_tileset_structure, rebuild_tileset, relative_transform, ProxyWriteMetrics, RebuildReport,
    TilesetProbe, WriteOptions,
};
pub use tree_builder::{build_tree, TreeBuildOptions};
pub use types::{BoundingVolume, Mat4d, RebuildTree, Representation, SourceBlock, TreeNode};

/// Convenience: load fixture/tileset and build the quadtree.
pub fn build_from_tileset(
    tileset_path: &std::path::Path,
    opts: &TreeBuildOptions,
) -> Result<RebuildTree> {
    let blocks = load_source_blocks(tileset_path)?;
    build_tree(&blocks, opts)
}
