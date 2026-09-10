//! Phase 8 acceptance: metrics report, texture budgets, LockBorder gaps.

use std::fs;
use std::path::PathBuf;
use top_rebuild::{
    find_basisu, process_textures, rebuild_tileset, TextureData, TreeBuildOptions, WriteOptions,
};

fn fixture_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../tests/fixtures/top_rebuild/grid_4x4")
        .canonicalize()
        .expect("fixture path")
}

fn out_dir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(name);
    let _ = fs::remove_dir_all(&d);
    d
}

#[test]
fn phase8_4x4_metrics_report() {
    let out = out_dir("top_rebuild_phase8_4x4");
    let report = rebuild_tileset(
        &fixture_dir(),
        &out,
        &TreeBuildOptions::default(),
        &WriteOptions {
            pack_as_b3dm: true,
            l1_max_triangles: 2_500,
            l2_max_triangles: 1_200,
            target_error_meters: 2.0,
            synthesize_if_empty: true,
            box_segments: 6,
            source_error_ratio: 0.5,
            max_texture_size: 128,
            max_texture_bytes: 4 * 1024 * 1024,
            max_glb_bytes: 32 * 1024 * 1024,
            enable_ktx2: false,
            lock_border: true,
            inject_test_textures: true,
            ..Default::default()
        },
    )
    .expect("rebuild");

    assert_eq!(report.level_counts, vec![(0, 16), (1, 4), (2, 1)]);
    assert!(report.metrics_path.exists());
    let raw = fs::read_to_string(&report.metrics_path).unwrap();
    let m: serde_json::Value = serde_json::from_str(&raw).unwrap();
    assert!(m["phase"].as_u64().unwrap() >= 8);
    assert!(m["lock_border"].as_bool().unwrap());
    assert!(m["gaps"].get("maxGap").is_some());
    assert!(m["gaps"].get("P95Gap").is_some());
    assert_eq!(m["budgets"]["maxTextureSize"], 128);
    assert!(m["totals"]["glb_bytes"].as_u64().unwrap() > 0);
    // With inject_test_textures, proxies should carry some texture bytes
    assert!(
        report.total_texture_bytes > 0,
        "expected textured proxies"
    );
    assert!(report.texture.max_dimension_after <= 128);
    println!(
        "phase8 metrics OK maxGap={:.4} P95={:.4} tex_unique={} glb_b={} path={}",
        report.gap.max_gap,
        report.gap.p95_gap,
        report.texture.unique_count,
        report.total_glb_bytes,
        report.metrics_path.display()
    );
}

#[test]
fn phase8_ktx2_honest_availability() {
    let a = TextureData::solid(255, 128, 0, 255, 32);
    let enable = find_basisu().is_some();
    let work = out_dir("top_rebuild_phase8_ktx2");
    fs::create_dir_all(&work).unwrap();
    let (proc, metrics) = process_textures(&[a], 32, 0, true, Some(&work)).unwrap();
    assert_eq!(metrics.ktx2_available, enable);
    if enable {
        // May still fall back on encode failure — either ktx2 or png is ok
        assert!(!proc.is_empty());
        println!(
            "basisu available path={:?} ktx2_encoded={}",
            metrics.basisu_path, metrics.ktx2_encoded
        );
    } else {
        assert!(metrics.warnings.iter().any(|w| w.contains("basisu")));
        assert_eq!(proc[0].mime, "image/png");
        println!("basisu UNAVAILABLE — honest PNG fallback");
    }
}

#[test]
fn phase8_texture_byte_budget_warns() {
    let big = TextureData::solid(1, 2, 3, 255, 256);
    let (_proc, metrics) = process_textures(&[big], 256, 200, false, None).unwrap();
    assert!(
        metrics
            .warnings
            .iter()
            .any(|w| w.contains("TEXTURE_BYTES_OVER_BUDGET") || w.contains("downscale")),
        "expected budget warning, got {:?}",
        metrics.warnings
    );
}
