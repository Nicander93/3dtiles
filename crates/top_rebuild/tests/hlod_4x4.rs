//! Phase 7 acceptance: full 4×4 HLOD 16→4→1 tileset with REPLACE + transform invariant.

use std::fs;
use std::path::PathBuf;
use top_rebuild::{
    assert_world_transform_invariant, bv_world_to_local, geometric_error_proxy,
    probe_tileset_structure, rebuild_tileset, relative_transform, BoundingVolume, Mat4d,
    TreeBuildOptions, WriteOptions,
};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/top_rebuild/grid_4x4")
        .canonicalize()
        .expect("fixture path")
}

fn out_dir() -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../target/top_rebuild_hlod_4x4_test");
    let _ = fs::remove_dir_all(&d);
    d
}

#[test]
fn world_transform_invariant_nested() {
    let l2 = Mat4d::translation(200.0, 200.0, 10.0);
    let l1 = Mat4d::translation(100.0, 100.0, 10.0);
    let l0 = Mat4d::identity();

    let l1_local = relative_transform(&l2, &l1).unwrap();
    assert_world_transform_invariant(&l2, &l1, &l1_local, 1e-9).unwrap();

    let l0_local = relative_transform(&l1, &l0).unwrap();
    assert_world_transform_invariant(&l1, &l0, &l0_local, 1e-9).unwrap();

    // Composed: l2 * l1_local * l0_local == l0 world (identity)
    let composed = l2.mul(&l1_local).mul(&l0_local);
    for i in 0..16 {
        let expect = if i % 5 == 0 { 1.0 } else { 0.0 };
        assert!(
            (composed.0[i] - expect).abs() < 1e-9,
            "composed[{i}]={} expect={expect}",
            composed.0[i]
        );
    }
}

#[test]
fn geometric_error_plan_formula() {
    let bv = BoundingVolume::from_box([
        100.0, 100.0, 10.0, 100.0, 0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 10.0,
    ]);
    let ge = geometric_error_proxy(&[50.0, 48.0], 0.25, &bv);
    assert!(ge > 50.0, "parent must exceed max child");
    assert!(ge >= 50.0 + 0.25 - 1e-9);
}

#[test]
fn hlod_4x4_pipeline_16_4_1() {
    let input = fixture_dir();
    let out = out_dir();
    let report = rebuild_tileset(
        &input,
        &out,
        &TreeBuildOptions::default(),
        &WriteOptions {
            pack_as_b3dm: true,
            l1_max_triangles: 3_000,
            l2_max_triangles: 1_500,
            target_error_meters: 2.0,
            synthesize_if_empty: true,
            box_segments: 6,
            source_error_ratio: 0.5,
            ..Default::default()
        },
    )
    .expect("rebuild");

    assert_eq!(report.level_counts, vec![(0, 16), (1, 4), (2, 1)]);
    assert!(report.tileset_path.exists());
    assert_eq!(report.proxies.len(), 5); // 4 L1 + 1 L2

    // Monotonic GE across levels
    let l1_ge: Vec<f64> = report
        .proxies
        .iter()
        .filter(|p| p.level == 1)
        .map(|p| p.geometric_error)
        .collect();
    let l2_ge = report
        .proxies
        .iter()
        .find(|p| p.level == 2)
        .unwrap()
        .geometric_error;
    assert!(l1_ge.iter().all(|g| *g > 50.0));
    assert!(l2_ge > l1_ge.iter().copied().fold(0.0, f64::max));

    // Content files exist
    for p in &report.proxies {
        let path = out.join(p.content_uri.trim_start_matches("./"));
        assert!(path.exists(), "missing {}", path.display());
        assert!(path.metadata().unwrap().len() > 100);
    }
    for y in 0..4 {
        for x in 0..4 {
            let leaf = out.join(format!("Data/Tile_+{x:03}_+{y:03}/tileset.json"));
            assert!(leaf.exists(), "missing leaf {}", leaf.display());
            let b3 = out.join(format!("Data/Tile_+{x:03}_+{y:03}/Tile_+{x:03}_+{y:03}.b3dm"));
            assert!(b3.exists() && b3.metadata().unwrap().len() > 32);
        }
    }

    let probe = probe_tileset_structure(&report.tileset_path).unwrap();
    assert_eq!(probe.proxy_nodes, 5);
    assert_eq!(probe.leaf_external, 16);
    assert!(probe.replace_count >= 5);
    assert!(probe.max_depth >= 2);

    // Root refine REPLACE + transform present
    let doc: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report.tileset_path).unwrap()).unwrap();
    let root = &doc["root"];
    assert_eq!(root["refine"], "REPLACE");
    assert!(root["transform"].as_array().unwrap().len() == 16);
    assert_eq!(root["children"].as_array().unwrap().len(), 4);
    for c in root["children"].as_array().unwrap() {
        assert_eq!(c["refine"], "REPLACE");
        assert_eq!(c["children"].as_array().unwrap().len(), 4);
    }

    // BV local for root should be near origin
    let root_box = root["boundingVolume"]["box"].as_array().unwrap();
    let rcx = root_box[0].as_f64().unwrap();
    let rcy = root_box[1].as_f64().unwrap();
    assert!(rcx.abs() < 1.0 && rcy.abs() < 1.0, "root local BV center ({rcx},{rcy})");

    // Spot-check transform invariant using written matrices
    let l2_t: Vec<f64> = root["transform"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let l2 = Mat4d::from_slice(&l2_t).unwrap();
    let l1_node = &root["children"][0];
    let l1_local_t: Vec<f64> = l1_node["transform"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_f64().unwrap())
        .collect();
    let l1_local = Mat4d::from_slice(&l1_local_t).unwrap();
    // Recover L1 world ≈ l2 * l1_local
    let l1_world = l2.mul(&l1_local);
    let back = relative_transform(&l2, &l1_world).unwrap();
    assert_world_transform_invariant(&l2, &l1_world, &back, 1e-6).unwrap();

    // Local BV helper sanity
    let world_bv = BoundingVolume::from_box([
        100.0, 100.0, 10.0, 100.0, 0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 10.0,
    ]);
    let local = bv_world_to_local(&world_bv, &l1_world);
    let _ = local;

    assert!(report.metrics_path.exists(), "rebuild_metrics.json missing");
    let metrics: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&report.metrics_path).unwrap()).unwrap();
    assert_eq!(metrics["phase"], 8);
    assert!(metrics["gaps"]["maxGap"].as_f64().is_some());
    assert!(metrics["gaps"]["P95Gap"].as_f64().is_some());
    assert!(metrics["budgets"]["maxTextureSize"].as_u64().unwrap() >= 256);
    assert!(report.total_glb_bytes > 0);

    println!(
        "hlod_4x4 OK counts {:?} proxies={} ge_l2={:.3} maxGap={:.4} tex_b={} path={}",
        report.level_counts,
        report.proxies.len(),
        l2_ge,
        report.gap.max_gap,
        report.total_texture_bytes,
        report.tileset_path.display()
    );
}
