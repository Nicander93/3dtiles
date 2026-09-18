//! Phase 11 / P0-1..P0-4 acceptance tests.

use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use top_rebuild::b3dm::pack_glb_as_b3dm;
use top_rebuild::glb::{make_box_primitive, write_glb};
use top_rebuild::{
    rebuild_tileset, structure_digest, validate_release_content, BoundingVolume, Mat4d,
    TreeBuildOptions, WriteOptions,
};

static SCRATCH_SEQ: AtomicU64 = AtomicU64::new(0);

fn fixture_4x4() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/top_rebuild/grid_4x4")
        .canonicalize()
        .expect("fixture")
}

fn scratch(name: &str) -> PathBuf {
    let n = SCRATCH_SEQ.fetch_add(1, Ordering::SeqCst);
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("{name}_{n}"));
    let _ = fs::remove_dir_all(&d);
    d
}

fn release_opts() -> WriteOptions {
    WriteOptions {
        synthesize_if_empty: false,
        inject_test_textures: false,
        l1_max_triangles: 3_000,
        l2_max_triangles: 1_500,
        box_segments: 4,
        ..Default::default()
    }
}

fn debug_opts() -> WriteOptions {
    WriteOptions {
        synthesize_if_empty: true,
        inject_test_textures: true,
        l1_max_triangles: 3_000,
        l2_max_triangles: 1_500,
        box_segments: 4,
        ..Default::default()
    }
}

fn copy_dir(src: &Path, dst: &Path) {
    fs::create_dir_all(dst).unwrap();
    for entry in fs::read_dir(src).unwrap() {
        let entry = entry.unwrap();
        let to = dst.join(entry.file_name());
        if entry.file_type().unwrap().is_dir() {
            copy_dir(&entry.path(), &to);
        } else {
            fs::copy(entry.path(), &to).unwrap();
        }
    }
}

fn make_tiny_b3dm() -> Vec<u8> {
    let prim = make_box_primitive(5.0, 5.0, 2.0, 2, "phase11");
    let glb = write_glb(&[prim]).expect("glb");
    pack_glb_as_b3dm(&glb).expect("b3dm")
}

/// Copy grid_4x4 and fill every empty .b3dm with a tiny valid mesh (release-path ready).
fn fixture_with_real_content() -> PathBuf {
    let src = fixture_4x4();
    let dst = scratch("phase11_real_content");
    copy_dir(&src, &dst);
    let bytes = make_tiny_b3dm();
    for entry in walkdir_b3dm(&dst) {
        fs::write(&entry, &bytes).unwrap();
    }
    dst
}

fn walkdir_b3dm(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    fn rec(dir: &Path, out: &mut Vec<PathBuf>) {
        for entry in fs::read_dir(dir).unwrap() {
            let entry = entry.unwrap();
            let p = entry.path();
            if entry.file_type().unwrap().is_dir() {
                rec(&p, out);
            } else if p.extension().and_then(|e| e.to_str()) == Some("b3dm") {
                out.push(p);
            }
        }
    }
    rec(root, &mut out);
    out
}

fn fixture_with_missing_content() -> PathBuf {
    let dst = fixture_with_real_content();
    let victim = dst.join("Data/Tile_+000_+000/Tile_+000_+000.b3dm");
    fs::remove_file(&victim).expect("remove content");
    dst
}

fn fixture_with_corrupt_b3dm() -> PathBuf {
    let dst = fixture_with_real_content();
    let victim = dst.join("Data/Tile_+000_+000/Tile_+000_+000.b3dm");
    fs::write(&victim, b"b3dm\x01\x00\x00\x00\x40\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00NOT_A_VALID_GLB_PAYLOAD!!!!!!!!!").unwrap();
    dst
}

fn fixture_empty_placeholders() -> PathBuf {
    let src = fixture_4x4();
    let dst = scratch("phase11_empty_placeholders");
    copy_dir(&src, &dst);
    dst
}

#[test]
fn release_missing_content_must_fail() {
    let input = fixture_with_missing_content();
    let out = scratch("phase11_release_missing_out");
    let err = rebuild_tileset(&input, &out, &TreeBuildOptions::default(), &release_opts())
        .expect_err("must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("CONTENT_MISSING") || err.code_str() == "CONTENT_MISSING",
        "got {msg}"
    );
}

#[test]
fn release_corrupt_b3dm_must_fail() {
    let input = fixture_with_corrupt_b3dm();
    let out = scratch("phase11_release_corrupt_out");
    let err = rebuild_tileset(&input, &out, &TreeBuildOptions::default(), &release_opts())
        .expect_err("must fail");
    let msg = err.to_string();
    assert!(
        msg.contains("CONTENT_INVALID")
            || msg.contains("GLTF_INVALID")
            || err.code_str() == "CONTENT_INVALID"
            || err.code_str() == "GLTF_INVALID",
        "got {msg}"
    );
}

#[test]
fn debug_fixture_may_synthesize_only_when_explicit() {
    let input = fixture_empty_placeholders();
    let out = scratch("phase11_debug_synth_out");
    let report = rebuild_tileset(&input, &out, &TreeBuildOptions::default(), &debug_opts())
        .expect("debug synthesize allowed");
    assert_eq!(report.level_counts, vec![(0, 16), (1, 4), (2, 1)]);
    assert!(report.tileset_path.exists());

    let release = WriteOptions::default();
    assert!(!release.synthesize_if_empty);
    assert!(!release.inject_test_textures);

    // Same empty placeholders must fail on release path.
    let out2 = scratch("phase11_debug_synth_release_fail");
    let err = rebuild_tileset(&input, &out2, &TreeBuildOptions::default(), &release_opts())
        .expect_err("release must not synthesize");
    assert!(err.code_str() == "CONTENT_MISSING" || err.code_str() == "CONTENT_INVALID", "{}", err);
}

#[test]
fn subtree_preservation_grid_4x4() {
    let input = fixture_with_real_content();
    let out = scratch("phase11_subtree_preserve_out");
    let report = rebuild_tileset(&input, &out, &TreeBuildOptions::default(), &release_opts())
        .expect("rebuild");
    assert!(report.subtree_preservation_path.exists());
    assert_eq!(report.subtree_preservation.len(), 16);
    for e in &report.subtree_preservation {
        assert!(
            e.structure_preserved,
            "{} not preserved nodes {}->{} content {}->{} missing={}",
            e.block,
            e.source_node_count,
            e.output_node_count,
            e.source_content_count,
            e.output_content_count,
            e.missing_uri
        );
        assert_eq!(e.missing_uri, 0);
        assert!(e.source_node_count >= 2, "{}", e.block);
        assert_eq!(e.output_node_count, e.source_node_count);
    }

    let src = input.join("Data/Tile_+000_+000/tileset.json");
    let dst = out.join("Data/Tile_+000_+000/tileset.json");
    let (sn, sc, ss) = structure_digest(&src).unwrap();
    let (on, oc, os) = structure_digest(&dst).unwrap();
    assert_eq!(sn, on);
    assert_eq!(sc, oc);
    assert_eq!(ss, os);

    let doc: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&dst).unwrap()).unwrap();
    let child_uri = doc["root"]["children"][0]["content"]["uri"]
        .as_str()
        .unwrap();
    assert!(
        child_uri.contains("_L1.b3dm"),
        "expected preserved L1 uri, got {child_uri}"
    );
}

#[test]
fn release_grid_4x4_no_synthetic() {
    let input = fixture_with_real_content();
    let out = scratch("phase11_release_4x4_out");
    let report = rebuild_tileset(&input, &out, &TreeBuildOptions::default(), &release_opts())
        .expect("rebuild with real content");
    assert_eq!(report.level_counts, vec![(0, 16), (1, 4), (2, 1)]);
    let sample = out.join("Data/Tile_+001_+001/Tile_+001_+001.b3dm");
    validate_release_content(&sample).expect("valid content");
}

#[test]
fn transform_identity_translation_rotation_nested() {
    let id = Mat4d::identity();
    assert!(id.max_abs_diff(&Mat4d::identity()) == 0.0);

    let t = Mat4d::translation(3.0, -4.0, 5.0);
    let p = t.transform_point(1.0, 2.0, 3.0);
    assert!((p.0 - 4.0).abs() < 1e-12);
    assert!((p.1 + 2.0).abs() < 1e-12);
    assert!((p.2 - 8.0).abs() < 1e-12);

    let r = Mat4d::rotation_z(std::f64::consts::FRAC_PI_2);
    let q = r.transform_point(1.0, 0.0, 0.0);
    assert!(q.0.abs() < 1e-12 && (q.1 - 1.0).abs() < 1e-12);

    let parent = Mat4d::translation(1e5, 2e5, 50.0).mul(&Mat4d::rotation_z(0.1));
    let child_local = Mat4d::translation(2.0, 3.0, 0.5);
    let child_world = parent.mul(&child_local);
    let recovered = parent.inverse().unwrap().mul(&child_world);
    assert!(recovered.max_abs_diff(&child_local) <= 1e-8);

    let ecef = Mat4d::translation(-2_556_000.0, 4_780_000.0, 3_350_000.0);
    assert!(ecef.is_finite());
    let back = ecef.inverse().unwrap().transform_point(
        -2_556_000.0,
        4_780_000.0,
        3_350_000.0,
    );
    assert!(back.0.abs() < 1e-4 && back.1.abs() < 1e-4 && back.2.abs() < 1e-4);
}

#[test]
fn oriented_bounding_box_corners() {
    let s = std::f64::consts::FRAC_1_SQRT_2 * 5.0;
    let bv = BoundingVolume::from_box([
        10.0, 20.0, 0.0, s, s, 0.0, -s, s, 0.0, 0.0, 0.0, 2.0,
    ]);
    let corners = bv.local_corners().unwrap();
    assert_eq!(corners.len(), 8);
    let ((min_x, min_y, _), (max_x, max_y, _)) = bv.aabb_min_max().unwrap();
    assert!((max_x - min_x) > 9.0);
    assert!((max_y - min_y) > 9.0);
    let world = Mat4d::translation(1000.0, 0.0, 0.0);
    let aabb = bv.world_aabb(&world).unwrap();
    assert!((aabb.min[0] - (1000.0 + min_x)).abs() < 1e-9);
}
