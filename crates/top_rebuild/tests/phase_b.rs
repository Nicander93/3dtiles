//! Phase B: selector fallback is unit-tested in selector.rs.
//! Here: original LOD tree copy, release refuse to synthesize, opt-in synthesize.

use serde_json::{json, Value};
use std::fs;
use std::path::{Path, PathBuf};
use top_rebuild::b3dm::pack_glb_as_b3dm;
use top_rebuild::glb::{make_box_primitive, write_glb};
use top_rebuild::{rebuild_tileset, Mat4d, TreeBuildOptions, WriteOptions};

fn tmp(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "top_rebuild_phaseb_{}_{}",
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

fn write_box_b3dm(path: &Path) {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    let prim = make_box_primitive(2.0, 2.0, 2.0, 2, "mat0:0.8,0.8,0.8,1");
    let glb = write_glb(&[prim]).unwrap();
    fs::write(path, pack_glb_as_b3dm(&glb).unwrap()).unwrap();
}

fn nested_lod_node(names: &[&str], ges: &[f64], refine: &str) -> Value {
    fn nest(i: usize, names: &[&str], ges: &[f64], refine: &str) -> Value {
        let mut node = json!({
            "boundingVolume": { "box": box12(0.0, 0.0, 0.0, 40.0, 40.0, 10.0) },
            "geometricError": ges[i],
            "refine": refine,
            "content": { "uri": format!("./{}", names[i]) }
        });
        if i + 1 < names.len() {
            node["children"] = json!([nest(i + 1, names, ges, refine)]);
        }
        node
    }
    nest(0, names, ges, refine)
}

fn write_block_lod4(root: &Path, name: &str) {
    let dir = root.join("Data").join(name);
    fs::create_dir_all(&dir).unwrap();
    let files = ["L0.b3dm", "L1.b3dm", "L2.b3dm", "L3.b3dm"];
    let ges = [40.0, 20.0, 10.0, 2.0];
    for f in files {
        write_box_b3dm(&dir.join(f));
    }
    let ts = json!({
        "asset": { "version": "1.0", "gltfUpAxis": "Z" },
        "geometricError": 40.0,
        "root": nested_lod_node(&files, &ges, "REPLACE")
    });
    fs::write(dir.join("tileset.json"), serde_json::to_string_pretty(&ts).unwrap()).unwrap();
}

fn write_two_tile_root(root: &Path) {
    write_block_lod4(root, "Tile_+000_+000");
    write_block_lod4(root, "Tile_+001_+000");
    let ts = json!({
        "asset": { "version": "1.0", "gltfUpAxis": "Z" },
        "geometricError": 200.0,
        "root": {
            "boundingVolume": { "box": box12(50.0, 0.0, 0.0, 90.0, 40.0, 10.0) },
            "geometricError": 200.0,
            "refine": "ADD",
            "transform": json!(Mat4d::identity().0.to_vec()),
            "children": [
                {
                    "transform": json!(Mat4d::translation(0.0, 0.0, 0.0).0.to_vec()),
                    "boundingVolume": { "box": box12(0.0, 0.0, 0.0, 40.0, 40.0, 10.0) },
                    "geometricError": 50.0,
                    "content": { "uri": "./Data/Tile_+000_+000/tileset.json" }
                },
                {
                    "transform": json!(Mat4d::translation(100.0, 0.0, 0.0).0.to_vec()),
                    "boundingVolume": { "box": box12(0.0, 0.0, 0.0, 40.0, 40.0, 10.0) },
                    "geometricError": 50.0,
                    "content": { "uri": "./Data/Tile_+001_+000/tileset.json" }
                }
            ]
        }
    });
    fs::write(root.join("tileset.json"), serde_json::to_string_pretty(&ts).unwrap()).unwrap();
}

fn collect_contents(node: &Value, depth: u32, out: &mut Vec<(u32, String, f64, String)>) {
    let ge = node
        .get("geometricError")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let refine = node
        .get("refine")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
    if let Some(uri) = node
        .get("content")
        .and_then(|c| c.get("uri"))
        .and_then(|u| u.as_str())
    {
        out.push((depth, uri.to_string(), ge, refine));
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for c in children {
            collect_contents(c, depth + 1, out);
        }
    }
}

fn release_opts() -> WriteOptions {
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
fn default_write_options_do_not_synthesize() {
    let d = WriteOptions::default();
    assert!(!d.synthesize_if_empty);
    assert!(!d.inject_test_textures);
}

#[test]
fn multi_level_original_subtree_preserved() {
    let input = tmp("lod4_in");
    write_two_tile_root(&input);
    let out = tmp("lod4_out");
    rebuild_tileset(&input, &out, &TreeBuildOptions::default(), &release_opts()).unwrap();

    for name in ["Tile_+000_+000", "Tile_+001_+000"] {
        let ts_path = out.join("Data").join(name).join("tileset.json");
        let doc: Value = serde_json::from_str(&fs::read_to_string(&ts_path).unwrap()).unwrap();
        let mut contents = Vec::new();
        collect_contents(&doc["root"], 0, &mut contents);
        assert_eq!(contents.len(), 4, "{name} lost LOD layers: {contents:?}");
        let ges: Vec<f64> = contents.iter().map(|c| c.2).collect();
        assert_eq!(ges, vec![40.0, 20.0, 10.0, 2.0], "{name} GE lost");
        assert!(contents.iter().all(|c| c.3 == "REPLACE"), "{name} refine lost");
        assert_eq!(contents[0].1, "./L0.b3dm");
        assert_eq!(contents[3].1, "./L3.b3dm");
        for uri in contents.iter().map(|c| &c.1) {
            let file = out.join("Data").join(name).join(uri.trim_start_matches("./"));
            assert!(file.is_file(), "missing {}", file.display());
            assert!(file.metadata().unwrap().len() > 32);
        }
    }
}

#[test]
fn unreadable_content_release_failure() {
    let input = tmp("bad_in");
    write_two_tile_root(&input);
    fs::write(
        input.join("Data/Tile_+000_+000/L0.b3dm"),
        b"this is not a b3dm file!!!!",
    )
    .unwrap();
    let out = tmp("bad_out");
    let err = rebuild_tileset(&input, &out, &TreeBuildOptions::default(), &release_opts())
        .unwrap_err()
        .to_string();
    assert!(
        err.contains("CONTENT_UNREADABLE") || err.contains("UNSUPPORTED_CONTENT"),
        "{err}"
    );
}

#[test]
fn missing_content_release_failure() {
    let input = tmp("miss_in");
    write_two_tile_root(&input);
    fs::remove_file(input.join("Data/Tile_+000_+000/L2.b3dm")).unwrap();
    let out = tmp("miss_out");
    let err = rebuild_tileset(&input, &out, &TreeBuildOptions::default(), &release_opts())
        .unwrap_err()
        .to_string();
    assert!(err.contains("CONTENT_MISSING"), "{err}");
}

#[test]
fn synthetic_debug_explicit_opt_in() {
    let input = tmp("syn_in");
    write_two_tile_root(&input);
    fs::remove_file(input.join("Data/Tile_+000_+000/L0.b3dm")).unwrap();
    let out = tmp("syn_out");
    let mut opts = release_opts();
    opts.synthesize_if_empty = true;
    rebuild_tileset(&input, &out, &TreeBuildOptions::default(), &opts).unwrap();
    let restored = out.join("Data/Tile_+000_+000/L0.b3dm");
    assert!(restored.is_file());
    assert!(restored.metadata().unwrap().len() > 32);
    let doc: Value = serde_json::from_str(
        &fs::read_to_string(out.join("Data/Tile_+000_+000/tileset.json")).unwrap(),
    )
    .unwrap();
    let mut contents = Vec::new();
    collect_contents(&doc["root"], 0, &mut contents);
    assert_eq!(contents.len(), 4);
}
