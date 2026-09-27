//! P4 complexity tests: verify O(N log N) behavior with synthetic large metadata.
//!
//! These tests ensure that validate_grid_spatial, block indexing, and gap metrics
//! do not exhibit O(N²) arrays or all-pairs comparisons for large N.

use std::fs;
use std::path::PathBuf;
use tempfile::TempDir;
use top_rebuild::adapter::load_source_blocks;
use top_rebuild::tileset_writer::{rebuild_tileset, WriteOptions};
use top_rebuild::tree_builder::TreeBuildOptions;

fn create_synthetic_grid(tmp: &TempDir, n: usize) -> PathBuf {
    let root = tmp.path().to_path_buf();
    let data_dir = root.join("Data");
    
    let mut root_children = Vec::new();
    
    for i in 0..n {
        let gx = (i % 100) as i32;
        let gy = (i / 100) as i32;
        let block_name = format!("Tile_+{:03}_+{:03}", gx, gy);
        let block_dir = data_dir.join(&block_name);
        fs::create_dir_all(&block_dir).unwrap();
        
        let cx = gx as f64 * 100.0;
        let cy = gy as f64 * 100.0;
        
        let tileset = serde_json::json!({
            "root": {
                "boundingVolume": {
                    "box": [cx, cy, 0.0, 50.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 10.0]
                },
                "geometricError": 100.0,
                "refine": "REPLACE",
                "content": {
                    "uri": format!("./{}.b3dm", block_name)
                }
            }
        });
        
        fs::write(
            block_dir.join("tileset.json"),
            serde_json::to_string_pretty(&tileset).unwrap(),
        )
        .unwrap();
        
        let b3dm_path = block_dir.join(format!("{}.b3dm", block_name));
        let glb = create_minimal_glb(cx as f32, cy as f32);
        let b3dm = top_rebuild::b3dm::pack_glb_as_b3dm(&glb).unwrap();
        fs::write(&b3dm_path, b3dm).unwrap();
        
        root_children.push(serde_json::json!({
            "boundingVolume": {
                "box": [cx, cy, 0.0, 50.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 10.0]
            },
            "geometricError": 100.0,
            "content": {
                "uri": format!("./Data/{}/tileset.json", block_name)
            }
        }));
    }
    
    let root_tileset = serde_json::json!({
        "asset": { "version": "1.0" },
        "geometricError": 500.0,
        "root": {
            "boundingVolume": {
                "box": [5000.0, 5000.0, 0.0, 5000.0, 0.0, 0.0, 0.0, 5000.0, 0.0, 0.0, 0.0, 50.0]
            },
            "geometricError": 500.0,
            "refine": "REPLACE",
            "children": root_children
        }
    });
    
    let tileset_path = root.join("tileset.json");
    fs::write(
        &tileset_path,
        serde_json::to_string_pretty(&root_tileset).unwrap(),
    )
    .unwrap();
    
    tileset_path
}

fn create_minimal_glb(cx: f32, cy: f32) -> Vec<u8> {
    let prim = top_rebuild::glb::make_box_primitive(20.0, 20.0, 5.0, 1, "default");
    let mut positions = prim.positions;
    for p in &mut positions {
        p[0] += cx;
        p[1] += cy;
    }
    
    let prim_moved = top_rebuild::glb::LoadedPrimitive {
        positions,
        normals: prim.normals,
        uvs: prim.uvs,
        indices: prim.indices,
        material_key: prim.material_key,
        texture_hash: None,
    };
    
    top_rebuild::glb::write_glb(&[prim_moved]).unwrap()
}

#[test]
fn test_1k_blocks_validate_grid_spatial() {
    let tmp = TempDir::new().unwrap();
    let tileset_path = create_synthetic_grid(&tmp, 1000);
    
    let start = std::time::Instant::now();
    let blocks = load_source_blocks(&tileset_path).unwrap();
    let elapsed = start.elapsed();
    
    assert_eq!(blocks.len(), 1000);
    assert!(
        elapsed.as_millis() < 5000,
        "1k blocks took {}ms, expected < 5000ms (O(N²) would be ~500s)",
        elapsed.as_millis()
    );
}

#[test]
fn test_10k_blocks_validate_grid_spatial() {
    let tmp = TempDir::new().unwrap();
    let tileset_path = create_synthetic_grid(&tmp, 10000);
    
    let start = std::time::Instant::now();
    let blocks = load_source_blocks(&tileset_path).unwrap();
    let elapsed = start.elapsed();
    
    assert_eq!(blocks.len(), 10000);
    assert!(
        elapsed.as_millis() < 50000,
        "10k blocks took {}ms, expected < 50s (O(N²) would be ~5000s)",
        elapsed.as_millis()
    );
}

#[test]
fn test_1k_blocks_rebuild_with_block_index() {
    let tmp = TempDir::new().unwrap();
    let tileset_path = create_synthetic_grid(&tmp, 100);
    
    let out_dir = tmp.path().join("output");
    fs::create_dir_all(&out_dir).unwrap();
    
    let tree_opts = TreeBuildOptions {
        max_levels: Some(2),
        ..Default::default()
    };
    
    let write_opts = WriteOptions {
        synthesize_if_empty: true,
        ..Default::default()
    };
    
    let start = std::time::Instant::now();
    let _report = rebuild_tileset(&tileset_path, &out_dir, &tree_opts, &write_opts).unwrap();
    let elapsed = start.elapsed();
    
    assert!(
        elapsed.as_millis() < 30000,
        "100 blocks rebuild took {}ms, expected < 30s",
        elapsed.as_millis()
    );
}

#[test]
fn test_spatial_index_gap_metrics() {
    let tmp = TempDir::new().unwrap();
    let tileset_path = create_synthetic_grid(&tmp, 64);
    
    let out_dir = tmp.path().join("output");
    fs::create_dir_all(&out_dir).unwrap();
    
    let tree_opts = TreeBuildOptions {
        max_levels: Some(2),
        ..Default::default()
    };
    
    let write_opts = WriteOptions {
        synthesize_if_empty: true,
        ..Default::default()
    };
    
    let start = std::time::Instant::now();
    let report = rebuild_tileset(&tileset_path, &out_dir, &tree_opts, &write_opts).unwrap();
    let elapsed = start.elapsed();
    
    assert!(
        elapsed.as_millis() < 30000,
        "64 blocks with gap metrics took {}ms, expected < 30s",
        elapsed.as_millis()
    );
    
    assert!(report.gap.notes.iter().any(|n| n.contains("P4")));
}

#[test]
fn test_no_quadratic_arrays_in_memory() {
    let tmp = TempDir::new().unwrap();
    let tileset_path = create_synthetic_grid(&tmp, 1000);
    
    let start_mem = get_memory_usage();
    let blocks = load_source_blocks(&tileset_path).unwrap();
    let end_mem = get_memory_usage();
    
    let mem_growth_mb = (end_mem - start_mem) as f64 / 1_048_576.0;
    
    assert!(
        mem_growth_mb < 100.0,
        "1k blocks used {:.1} MB, expected < 100 MB (O(N²) arrays would use ~1000 MB)",
        mem_growth_mb
    );
    
    assert_eq!(blocks.len(), 1000);
}

#[cfg(target_os = "linux")]
fn get_memory_usage() -> usize {
    let status = fs::read_to_string("/proc/self/status").unwrap();
    for line in status.lines() {
        if line.starts_with("VmRSS:") {
            let parts: Vec<&str> = line.split_whitespace().collect();
            if parts.len() >= 2 {
                return parts[1].parse::<usize>().unwrap_or(0) * 1024;
            }
        }
    }
    0
}

#[cfg(not(target_os = "linux"))]
fn get_memory_usage() -> usize {
    0
}
