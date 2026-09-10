//! Integration: load tests/fixtures/top_rebuild/grid_16x16 and expect
//! L0 256 / L1 64 / L2 16 / L3 4 / L4 1 (Phase 10 scale ladder).

use std::path::PathBuf;
use top_rebuild::{
    build_from_tileset, format_level_counts, load_source_blocks, TreeBuildOptions,
};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/top_rebuild/grid_16x16")
        .canonicalize()
        .expect("fixture path")
}

#[test]
fn fixture_loads_256_source_blocks() {
    let blocks = load_source_blocks(&fixture_dir()).expect("load");
    assert_eq!(blocks.len(), 256);
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
fn fixture_quadtree_256_64_16_4_1() {
    let tree = build_from_tileset(&fixture_dir(), &TreeBuildOptions::default()).expect("build");
    let counts = format_level_counts(&tree);
    assert_eq!(counts, "L0 256\nL1 64\nL2 16\nL3 4\nL4 1");
    assert_eq!(
        tree.level_counts(),
        vec![(0, 256), (1, 64), (2, 16), (3, 4), (4, 1)]
    );
    let root = &tree.levels[4][0];
    assert_eq!(root.child_ids.len(), 4);
    assert_eq!(root.source_representation_ids.len(), 256);
}
