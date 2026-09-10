//! Integration: load tests/fixtures/top_rebuild/grid_4x4 and expect L0 16 / L1 4 / L2 1.

use std::path::PathBuf;
use top_rebuild::{
    build_from_tileset, format_level_counts, load_source_blocks, TreeBuildOptions,
};

fn fixture_dir() -> PathBuf {
    // CARGO_MANIFEST_DIR = crates/top_rebuild
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/top_rebuild/grid_4x4")
        .canonicalize()
        .expect("fixture path")
}

#[test]
fn fixture_loads_16_source_blocks() {
    let blocks = load_source_blocks(&fixture_dir()).expect("load");
    assert_eq!(blocks.len(), 16);
    for b in &blocks {
        assert!(b.grid_x.is_some() && b.grid_y.is_some());
        assert!(
            b.representations.len() >= 2,
            "{} should have coarse+fine reps",
            b.id
        );
    }
}

#[test]
fn fixture_quadtree_16_4_1() {
    let tree = build_from_tileset(&fixture_dir(), &TreeBuildOptions::default()).expect("build");
    let counts = format_level_counts(&tree);
    assert_eq!(counts, "L0 16\nL1 4\nL2 1");
    assert_eq!(tree.level_counts(), vec![(0, 16), (1, 4), (2, 1)]);

    // L2 root covers all 16 representation ids from L0
    let root = &tree.levels[2][0];
    assert_eq!(root.child_ids.len(), 4);
    assert_eq!(root.source_representation_ids.len(), 16);
    assert!(root.bounds.box_values.is_some());
}
