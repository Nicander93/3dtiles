//! Phase A: coordinate / transform regression tests.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use top_rebuild::b3dm::pack_glb_as_b3dm;
use top_rebuild::glb::{load_mesh_from_glb, make_box_primitive, write_glb};
use top_rebuild::{
    assert_world_transform_invariant, load_source_blocks, rebuild_tileset,
    to_parent_local, BoundingVolume, Mat4d, TreeBuildOptions, WriteOptions,
};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "top_rebuild_xform_{}_{}",
        name,
        std::process::id()
    ));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn box12(cx: f64, cy: f64, cz: f64, hx: f64, hy: f64, hz: f64) -> Vec<f64> {
    vec![cx, cy, cz, hx, 0.0, 0.0, 0.0, hy, 0.0, 0.0, 0.0, hz]
}

fn mat_json(m: &Mat4d) -> Value {
    json!(m.0.to_vec())
}

fn parse_mat(node: &Value) -> Mat4d {
    if let Some(arr) = node.get("transform").and_then(|t| t.as_array()) {
        if arr.len() == 16 {
            let vals: Vec<f64> = arr.iter().map(|v| v.as_f64().unwrap_or(0.0)).collect();
            return Mat4d::from_slice(&vals).unwrap();
        }
    }
    Mat4d::identity()
}

fn write_box_b3dm(path: &Path) {
    let prim = make_box_primitive(2.0, 2.0, 2.0, 2, "mat0:0.8,0.8,0.8,1");
    let glb = write_glb(&[prim]).unwrap();
    let b3dm = pack_glb_as_b3dm(&glb).unwrap();
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).unwrap();
    }
    fs::write(path, b3dm).unwrap();
}

fn write_block(
    root: &Path,
    name: &str,
    ext_root_transform: &Mat4d,
    b3dm_name: &str,
) {
    let dir = root.join("Data").join(name);
    fs::create_dir_all(&dir).unwrap();
    write_box_b3dm(&dir.join(b3dm_name));
    let ts = json!({
        "asset": { "version": "1.0", "gltfUpAxis": "Z" },
        "geometricError": 20.0,
        "root": {
            "transform": mat_json(ext_root_transform),
            "boundingVolume": { "box": box12(0.0, 0.0, 0.0, 40.0, 40.0, 10.0) },
            "geometricError": 20.0,
            "refine": "REPLACE",
            "content": { "uri": format!("./{b3dm_name}") }
        }
    });
    fs::write(dir.join("tileset.json"), serde_json::to_string_pretty(&ts).unwrap()).unwrap();
}

fn write_root_two_tiles(
    root: &Path,
    root_transform: &Mat4d,
    t0: &Mat4d,
    t1: &Mat4d,
    ext0: &Mat4d,
    ext1: &Mat4d,
) {
    write_block(root, "Tile_+000_+000", ext0, "Tile_+000_+000.b3dm");
    write_block(root, "Tile_+001_+000", ext1, "Tile_+001_+000.b3dm");
    let ts = json!({
        "asset": { "version": "1.0", "gltfUpAxis": "Z" },
        "geometricError": 200.0,
        "root": {
            "transform": mat_json(root_transform),
            "boundingVolume": { "box": box12(50.0, 0.0, 0.0, 90.0, 40.0, 10.0) },
            "geometricError": 200.0,
            "refine": "ADD",
            "children": [
                {
                    "transform": mat_json(t0),
                    "boundingVolume": { "box": box12(0.0, 0.0, 0.0, 40.0, 40.0, 10.0) },
                    "geometricError": 50.0,
                    "content": { "uri": "./Data/Tile_+000_+000/tileset.json" }
                },
                {
                    "transform": mat_json(t1),
                    "boundingVolume": { "box": box12(0.0, 0.0, 0.0, 40.0, 40.0, 10.0) },
                    "geometricError": 50.0,
                    "content": { "uri": "./Data/Tile_+001_+000/tileset.json" }
                }
            ]
        }
    });
    fs::write(root.join("tileset.json"), serde_json::to_string_pretty(&ts).unwrap()).unwrap();
}

fn accum_leaf_world(node: &Value, parent: &Mat4d, out: &mut Vec<(String, Mat4d)>) {
    let world = parent.mul(&parse_mat(node));
    if let Some(uri) = node
        .get("content")
        .and_then(|c| c.get("uri"))
        .and_then(|u| u.as_str())
    {
        if uri.ends_with("tileset.json") {
            out.push((uri.to_string(), world.clone()));
        }
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for c in children {
            accum_leaf_world(c, &world, out);
        }
    }
}

fn write_opts() -> WriteOptions {
    WriteOptions {
        pack_as_b3dm: true,
        l1_max_triangles: 8_000,
        l2_max_triangles: 4_000,
        target_error_meters: 2.0,
        synthesize_if_empty: false,
        box_segments: 2,
        inject_test_textures: false,
        ..Default::default()
    }
}

#[test]
fn local_bounds_plus_tile_transform_is_regular_grid() {
    let root = tmp("grid_tf");
    write_root_two_tiles(
        &root,
        &Mat4d::identity(),
        &Mat4d::translation(0.0, 0.0, 0.0),
        &Mat4d::translation(100.0, 0.0, 0.0),
        &Mat4d::identity(),
        &Mat4d::identity(),
    );
    let blocks = load_source_blocks(&root).expect("load");
    assert_eq!(blocks.len(), 2);
    let c0 = blocks[0]
        .bounds
        .transformed_center(&blocks[0].world_transform)
        .unwrap();
    let c1 = blocks[1]
        .bounds
        .transformed_center(&blocks[1].world_transform)
        .unwrap();
    assert!(c0.0.abs() < 1e-6);
    assert!((c1.0 - 100.0).abs() < 1e-6);
}

#[test]
fn world_bounds_union_after_child_transforms() {
    let a = BoundingVolume::from_box([
        0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 5.0,
    ]);
    let b = a.clone();
    let wa = a.world_bounds(&Mat4d::translation(0.0, 0.0, 0.0));
    let wb = b.world_bounds(&Mat4d::translation(100.0, 0.0, 0.0));
    let u = BoundingVolume::union(&wa, &wb);
    let c = u.center().unwrap();
    assert!((c.0 - 50.0).abs() < 1e-9);
    let (amin, amax) = u.aabb_min_max().unwrap();
    assert!((amin.0 + 10.0).abs() < 1e-9);
    assert!((amax.0 - 110.0).abs() < 1e-9);
}

#[test]
fn external_tileset_cumulative_transform() {
    let root = tmp("ext_tf");
    let root_t = Mat4d::translation(1000.0, 0.0, 0.0);
    let child1 = Mat4d::translation(100.0, 0.0, 0.0);
    let ext = Mat4d::translation(0.0, 0.0, 5.0);
    write_root_two_tiles(
        &root,
        &root_t,
        &Mat4d::identity(),
        &child1,
        &ext,
        &ext,
    );
    let blocks = load_source_blocks(&root).expect("load");
    let w0 = &blocks.iter().find(|b| b.id.contains("000")).unwrap().representations[0]
        .world_transform;
    let p = w0.transform_point(0.0, 0.0, 0.0);
    assert!((p.0 - 1000.0).abs() < 1e-6);
    assert!((p.2 - 5.0).abs() < 1e-6);
    let w1 = &blocks.iter().find(|b| b.id.contains("001")).unwrap().representations[0]
        .world_transform;
    let p1 = w1.transform_point(0.0, 0.0, 0.0);
    assert!((p1.0 - 1100.0).abs() < 1e-6);
    assert!((p1.2 - 5.0).abs() < 1e-6);
}

#[test]
fn parent_local_and_world_roundtrip() {
    let child = Mat4d::translation(150.0, 50.0, 10.0);
    let parent = Mat4d::translation(100.0, 50.0, 10.0);
    let local = to_parent_local((1.0, 2.0, 3.0), &child, &parent);
    let world = parent.transform_point(local.0, local.1, local.2);
    assert!((world.0 - 151.0).abs() < 1e-9);
    assert!((world.1 - 52.0).abs() < 1e-9);
    assert!((world.2 - 13.0).abs() < 1e-9);
}

#[test]
fn ecef_large_coordinate_parent_local_proxy() {
    let ecef = Mat4d::translation(3_900_000.0, 1_000_000.0, 4_800_000.0);
    let root = tmp("ecef");
    write_root_two_tiles(
        &root,
        &ecef,
        &Mat4d::identity(),
        &Mat4d::translation(100.0, 0.0, 0.0),
        &Mat4d::identity(),
        &Mat4d::identity(),
    );
    let out = tmp("ecef_out");
    let report = rebuild_tileset(&root, &out, &TreeBuildOptions::default(), &write_opts())
        .expect("rebuild ecef");
    assert_eq!(report.level_counts[0], (0, 2));
    let proxy_uri = &report.proxies[0].content_uri;
    let proxy_path = out.join(proxy_uri.trim_start_matches("./"));
    let glb = fs::read(&proxy_path).unwrap();
    let glb = if glb.starts_with(b"b3dm") {
        top_rebuild::b3dm::extract_glb_from_b3dm_bytes(&glb).unwrap()
    } else {
        glb
    };
    let mesh = load_mesh_from_glb(&glb).unwrap();
    for p in &mesh.primitives {
        for v in &p.positions {
            let r = (v[0] * v[0] + v[1] * v[1] + v[2] * v[2]).sqrt();
            assert!(
                r < 500.0,
                "proxy vertex still in ECEF magnitude: {r} at {v:?}"
            );
        }
    }
}

#[test]
fn gltf_node_transform_baked_on_load() {
    let prim = make_box_primitive(1.0, 1.0, 1.0, 1, "mat0:1,1,1,1");
    let glb = write_glb(&[prim]).unwrap();
    let mesh = load_mesh_from_glb(&glb).unwrap();
    let mean_x: f64 = mesh.primitives[0]
        .positions
        .iter()
        .map(|v| v[0] as f64)
        .sum::<f64>()
        / mesh.primitives[0].positions.len() as f64;
    assert!(mean_x.abs() < 0.5);

    let translated = glb_with_node_translation(&glb, 25.0, 0.0, 0.0);
    let mesh2 = load_mesh_from_glb(&translated).unwrap();
    let mean_x2: f64 = mesh2.primitives[0]
        .positions
        .iter()
        .map(|v| v[0] as f64)
        .sum::<f64>()
        / mesh2.primitives[0].positions.len() as f64;
    assert!(
        (mean_x2 - 25.0).abs() < 0.5,
        "gltf node translation not applied, mean_x={mean_x2}"
    );
}

#[test]
fn remount_accumulated_world_matches_source() {
    let root_t = Mat4d::translation(200.0, 40.0, 8.0);
    let root = tmp("remount");
    write_root_two_tiles(
        &root,
        &root_t,
        &Mat4d::identity(),
        &Mat4d::translation(100.0, 0.0, 0.0),
        &Mat4d::translation(0.0, 0.0, 3.0),
        &Mat4d::translation(0.0, 0.0, 3.0),
    );
    let blocks = load_source_blocks(&root).unwrap();
    let out = tmp("remount_out");
    rebuild_tileset(&root, &out, &TreeBuildOptions::default(), &write_opts()).unwrap();
    let doc: Value =
        serde_json::from_str(&fs::read_to_string(out.join("tileset.json")).unwrap()).unwrap();
    let mut leaves = Vec::new();
    accum_leaf_world(&doc["root"], &Mat4d::identity(), &mut leaves);
    assert_eq!(leaves.len(), 2);
    for (uri, world) in &leaves {
        let id = if uri.contains("Tile_+001_+000") {
            "Tile_+001_+000"
        } else {
            "Tile_+000_+000"
        };
        let block = blocks.iter().find(|b| b.id == id).unwrap();
        let expected = &block.representations[0].world_transform;
        assert_world_transform_invariant(&Mat4d::identity(), expected, world, 1e-4)
            .unwrap_or_else(|_| panic!("{uri} remount world mismatch"));
        let p_old = expected.transform_point(0.0, 0.0, 0.0);
        let p_new = world.transform_point(0.0, 0.0, 0.0);
        assert!((p_old.0 - p_new.0).abs() < 1e-3);
        assert!((p_old.1 - p_new.1).abs() < 1e-3);
        assert!((p_old.2 - p_new.2).abs() < 1e-3);
    }
}

fn glb_with_node_translation(src: &[u8], tx: f64, ty: f64, tz: f64) -> Vec<u8> {
    assert_eq!(&src[0..4], b"glTF");
    let json_len = u32::from_le_bytes(src[12..16].try_into().unwrap()) as usize;
    let json_start = 20;
    let json_end = json_start + json_len;
    let mut json: Value = serde_json::from_slice(&src[json_start..json_end]).unwrap();
    json["nodes"][0]["matrix"] = json!([
        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, tx, ty, tz, 1.0
    ]);
    let mut json_bytes = serde_json::to_vec(&json).unwrap();
    while json_bytes.len() % 4 != 0 {
        json_bytes.push(b' ');
    }
    let bin_chunk_start = json_end;
    let bin = src[bin_chunk_start..].to_vec();
    let total = 12 + 8 + json_bytes.len() + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&bin);
    out
}
