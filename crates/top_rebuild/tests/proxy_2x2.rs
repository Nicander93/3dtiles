//! Phase 6 acceptance: 2×2 adjacent SourceBlocks → one parent Proxy GLB;
//! transform tests; triangle counts before/after.

use std::fs;
use std::path::PathBuf;
use top_rebuild::b3dm::{extract_glb_from_b3dm_bytes, pack_glb_as_b3dm};
use top_rebuild::glb::{load_primitives_from_glb, make_box_primitive, write_glb};
use top_rebuild::{
    build_proxy_to_file, to_parent_local, ChildContent, Mat4d, ProxyBudget,
};

fn work_dir(name: &str) -> PathBuf {
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target/top_rebuild_proxy_2x2_test")
        .join(name);
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

#[test]
fn transform_parent_local_frame() {
    let child = Mat4d::translation(50.0, 50.0, 10.0);
    let parent = Mat4d::translation(100.0, 100.0, 10.0);
    let p = to_parent_local((0.0, 0.0, 0.0), &child, &parent);
    assert!((p.0 + 50.0).abs() < 1e-9);
    assert!((p.1 + 50.0).abs() < 1e-9);
    assert!(p.2.abs() < 1e-9);

    // Nested: ECEF-scale translation must not blow float mesh (we keep double until float store)
    let big_child = Mat4d::translation(6_378_000.0, 100.0, 50.0);
    let big_parent = Mat4d::translation(6_378_050.0, 150.0, 50.0);
    let q = to_parent_local((1.0, 2.0, 3.0), &big_child, &big_parent);
    assert!((q.0 - (1.0 - 50.0)).abs() < 1e-6);
    assert!((q.1 - (2.0 - 50.0)).abs() < 1e-6);
    assert!((q.2 - 3.0).abs() < 1e-6);
}

#[test]
fn two_by_two_b3dm_to_proxy_glb() {
    let dir = work_dir("b3dm");
    let segs = 8;
    let box_prim = make_box_primitive(20.0, 20.0, 5.0, segs, "mat0:0.700,0.700,0.700,1.000");
    let tris_one = box_prim.triangle_count() as u64;
    let glb = write_glb(&[box_prim]).expect("write child glb");
    let b3dm = pack_glb_as_b3dm(&glb).expect("pack b3dm");

    // Round-trip B3DM reader
    let extracted = extract_glb_from_b3dm_bytes(&b3dm).unwrap();
    assert_eq!(&extracted[0..4], b"glTF");

    let centers = [
        (50.0, 50.0, 10.0),
        (150.0, 50.0, 10.0),
        (50.0, 150.0, 10.0),
        (150.0, 150.0, 10.0),
    ];
    let mut children = Vec::new();
    for (i, (cx, cy, cz)) in centers.iter().enumerate() {
        let p = dir.join(format!("child_{i}.b3dm"));
        fs::write(&p, &b3dm).unwrap();
        children.push(ChildContent {
            content_path: p,
            world_transform: Mat4d::translation(*cx, *cy, *cz),
        });
    }

    let parent = Mat4d::translation(100.0, 100.0, 10.0);
    let budget = ProxyBudget {
        max_triangles: (tris_one * 4 / 2).max(48),
        target_error_meters: 2.5,
        ..Default::default()
    };
    let out = dir.join("Proxy_L1_0_0.glb");
    let result = build_proxy_to_file(&children, &parent, &budget, &out).expect("proxy");

    assert_eq!(result.triangles_before, tris_one * 4);
    assert!(
        result.triangles_after < result.triangles_before,
        "before={} after={}",
        result.triangles_before,
        result.triangles_after
    );
    assert!(result.triangles_after > 0);
    assert!(out.exists());
    assert!(out.metadata().unwrap().len() > 100);

    // Inspectable: reload with gltf
    let bytes = fs::read(&out).unwrap();
    let loaded = load_primitives_from_glb(&bytes).expect("reload proxy");
    let after: u64 = loaded.iter().map(|p| p.triangle_count() as u64).sum();
    assert_eq!(after, result.triangles_after);

    // Centroid near parent-local origin
    let mut sx = 0.0f64;
    let mut sy = 0.0f64;
    let mut n = 0.0f64;
    for p in &loaded {
        for v in &p.positions {
            sx += v[0] as f64;
            sy += v[1] as f64;
            n += 1.0;
        }
    }
    assert!((sx / n).abs() < 20.0, "cx={}", sx / n);
    assert!((sy / n).abs() < 20.0, "cy={}", sy / n);

    println!(
        "proxy_2x2 OK tris {} -> {} err={:.4} groups={} path={}",
        result.triangles_before,
        result.triangles_after,
        result.simplification_error_meters,
        result.group_count,
        out.display()
    );
}

#[test]
fn glb_only_path_without_b3dm() {
    let dir = work_dir("glb_only");
    let prim = make_box_primitive(10.0, 10.0, 2.0, 4, "default");
    let tris = prim.triangle_count() as u64;
    let glb = write_glb(&[prim]).unwrap();
    let mut children = Vec::new();
    for i in 0..4 {
        let p = dir.join(format!("child_{i}.glb"));
        fs::write(&p, &glb).unwrap();
        let (cx, cy) = match i {
            0 => (0.0, 0.0),
            1 => (40.0, 0.0),
            2 => (0.0, 40.0),
            _ => (40.0, 40.0),
        };
        children.push(ChildContent {
            content_path: p,
            world_transform: Mat4d::translation(cx, cy, 0.0),
        });
    }
    let parent = Mat4d::translation(20.0, 20.0, 0.0);
    let out = dir.join("proxy.glb");
    let result = build_proxy_to_file(
        &children,
        &parent,
        &ProxyBudget {
            max_triangles: tris * 2,
            target_error_meters: 1.0,
            ..Default::default()
        },
        &out,
    )
    .unwrap();
    assert!(result.triangles_after <= result.triangles_before);
    assert!(out.exists());
}
