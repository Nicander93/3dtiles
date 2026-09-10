//! Tileset Source Adapter: parse tileset.json (+ external refs) → SourceBlocks (plan §9.4).

use crate::error::{Result, TopRebuildError};
use crate::types::{BoundingVolume, Mat4d, Representation, SourceBlock};
use regex::Regex;
use serde_json::Value;
use std::cmp::Ordering;
use std::fs;
use std::path::{Path, PathBuf};

fn tile_re() -> Regex {
    Regex::new(r"(?i)Tile_\+?(-?\d+)_\+?(-?\d+)").expect("tile regex")
}

fn normalize_uri(uri: &str) -> String {
    let mut u = uri.replace('\\', "/");
    while u.starts_with("./") {
        u = u[2..].to_string();
    }
    while u.starts_with(".//") {
        u = u[3..].to_string();
    }
    while u.starts_with('/') {
        u = u[1..].to_string();
    }
    u
}

fn parse_box(bv: &Value) -> Option<BoundingVolume> {
    let arr = bv.get("box")?.as_array()?;
    if arr.len() < 12 {
        return None;
    }
    let mut vals = [0.0; 12];
    for (i, v) in arr.iter().take(12).enumerate() {
        vals[i] = v.as_f64().unwrap_or(0.0);
    }
    Some(BoundingVolume::from_box(vals))
}

fn parse_transform(node: &Value) -> Mat4d {
    if let Some(arr) = node.get("transform").and_then(|t| t.as_array()) {
        if arr.len() == 16 {
            let mut vals = [0.0; 16];
            for (i, v) in arr.iter().enumerate() {
                vals[i] = v.as_f64().unwrap_or(0.0);
            }
            return Mat4d(vals);
        }
    }
    Mat4d::identity()
}

fn mul_transform(parent: &Mat4d, local: &Mat4d) -> Mat4d {
    parent.mul(local)
}

/// Walk a block tileset root collecting content-bearing nodes as Representations.
/// Order: root first (typically coarser / higher geometricError), then depth-first children.
fn collect_representations(
    node: &Value,
    block_dir: &Path,
    parent_world: &Mat4d,
    block_id: &str,
    out: &mut Vec<Representation>,
    counter: &mut usize,
) {
    let local = parse_transform(node);
    let world = mul_transform(parent_world, &local);
    let ge = node
        .get("geometricError")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let bounds = node
        .get("boundingVolume")
        .and_then(parse_box)
        .unwrap_or_else(BoundingVolume::empty);

    if let Some(uri) = node
        .get("content")
        .and_then(|c| c.get("uri"))
        .and_then(|u| u.as_str())
    {
        let rel = normalize_uri(uri);
        let content_path = block_dir.join(&rel);
        let rep_id = format!("{block_id}#rep{counter}");
        *counter += 1;
        out.push(Representation {
            id: rep_id,
            content_path,
            geometric_error_meters: ge,
            triangle_count: 0,
            texture_bytes: 0,
            bounds: bounds.clone(),
            world_transform: world.clone(),
        });
    }

    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            collect_representations(child, block_dir, &world, block_id, out, counter);
        }
    }
}

fn load_json(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

/// Validate grid indices against spatial centers (plan §9.4).
///
/// Phase 10: V1 **stops** on `GRID_SPATIAL_MISMATCH` (no silent wrong tree).
/// Sparse / irregular `Tile_*` sets (e.g. OSGBny 6 tiles) are a known limit of
/// regular-grid Proxy HLOD — not covered by inventing partial coverage.
/// Optional `--allow-partial-grid` was deferred; document refusal instead.
pub fn validate_grid_spatial(blocks: &[SourceBlock]) -> Result<()> {
    let with_grid: Vec<_> = blocks
        .iter()
        .filter(|b| b.grid_x.is_some() && b.grid_y.is_some() && b.bounds.center().is_some())
        .collect();
    if with_grid.len() < 2 {
        return Ok(());
    }

    let mut spacings_x = Vec::new();
    let mut spacings_y = Vec::new();
    let mut collapsed_x = false;
    let mut collapsed_y = false;

    for i in 0..with_grid.len() {
        for j in (i + 1)..with_grid.len() {
            let a = with_grid[i];
            let b = with_grid[j];
            let (ax, ay) = (a.grid_x.unwrap(), a.grid_y.unwrap());
            let (bx, by) = (b.grid_x.unwrap(), b.grid_y.unwrap());
            let (acx, acy, _) = a.bounds.center().unwrap();
            let (bcx, bcy, _) = b.bounds.center().unwrap();
            let dxg = (bx - ax).abs();
            let dyg = (by - ay).abs();
            if dxg > 0 {
                let sx = (bcx - acx).abs() / dxg as f64;
                if sx < 1e-3 {
                    collapsed_x = true;
                } else {
                    spacings_x.push(sx);
                }
            }
            if dyg > 0 {
                let sy = (bcy - acy).abs() / dyg as f64;
                if sy < 1e-3 {
                    collapsed_y = true;
                } else {
                    spacings_y.push(sy);
                }
            }
        }
    }

    // Distinct grid indices but coincident centers ⇒ mismatch.
    if (collapsed_x && !spacings_x.is_empty())
        || (collapsed_y && !spacings_y.is_empty())
        || (collapsed_x && collapsed_y)
        || (collapsed_x && spacings_y.iter().any(|s| *s > 1e-3))
        || (collapsed_y && spacings_x.iter().any(|s| *s > 1e-3))
    {
        return Err(TopRebuildError::GridSpatialMismatch(
            "distinct grid indices map to coincident / collapsed centers".into(),
        ));
    }

    spacings_x.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    spacings_y.sort_by(|a, b| a.partial_cmp(b).unwrap_or(Ordering::Equal));
    let cell_x = spacings_x.get(spacings_x.len() / 2).copied().unwrap_or(0.0);
    let cell_y = spacings_y.get(spacings_y.len() / 2).copied().unwrap_or(0.0);
    if cell_x <= 1e-6 && cell_y <= 1e-6 {
        return Ok(());
    }

    let origin = with_grid
        .iter()
        .min_by_key(|b| (b.grid_x.unwrap(), b.grid_y.unwrap()))
        .unwrap();
    let (ox, oy, _) = origin.bounds.center().unwrap();
    let (ogx, ogy) = (origin.grid_x.unwrap(), origin.grid_y.unwrap());

    let cell_x = if cell_x > 1e-6 { cell_x } else { cell_y };
    let cell_y = if cell_y > 1e-6 { cell_y } else { cell_x };
    let tol = (cell_x.max(cell_y) * 0.35).max(1.0);
    for b in &with_grid {
        let (gx, gy) = (b.grid_x.unwrap(), b.grid_y.unwrap());
        let (cx, cy, _) = b.bounds.center().unwrap();
        let expected_x = ox + (gx - ogx) as f64 * cell_x;
        let expected_y = oy + (gy - ogy) as f64 * cell_y;
        let dist = (cx - expected_x).hypot(cy - expected_y);
        if dist > tol {
            return Err(TopRebuildError::GridSpatialMismatch(format!(
                "block {} grid=({},{}) center=({:.3},{:.3}) expected≈({:.3},{:.3}) dist={:.3} tol={:.3}",
                b.id, gx, gy, cx, cy, expected_x, expected_y, dist, tol
            )));
        }
    }
    Ok(())
}

/// Load root tileset.json and external Tile_* children into SourceBlocks.
pub fn load_source_blocks(tileset_path: &Path) -> Result<Vec<SourceBlock>> {
    let tileset_path = if tileset_path.is_dir() {
        tileset_path.join("tileset.json")
    } else {
        tileset_path.to_path_buf()
    };
    if !tileset_path.exists() {
        return Err(TopRebuildError::InvalidTileset(format!(
            "missing {}",
            tileset_path.display()
        )));
    }
    let input_dir = tileset_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let root_doc = load_json(&tileset_path)?;
    let root = root_doc
        .get("root")
        .ok_or_else(|| TopRebuildError::InvalidTileset("missing root".into()))?;
    let root_world = parse_transform(root);
    let re = tile_re();

    let mut blocks = Vec::new();
    let children = root
        .get("children")
        .and_then(|c| c.as_array())
        .cloned()
        .unwrap_or_default();

    for child in &children {
        let Some(uri) = child
            .get("content")
            .and_then(|c| c.get("uri"))
            .and_then(|u| u.as_str())
        else {
            continue;
        };
        let rel = normalize_uri(uri);
        let Some(caps) = re.captures(&rel) else {
            continue;
        };
        let grid_x: i32 = caps[1].parse().unwrap_or(0);
        let grid_y: i32 = caps[2].parse().unwrap_or(0);

        let ext_path = input_dir.join(&rel);
        if !ext_path.exists() {
            return Err(TopRebuildError::InvalidTileset(format!(
                "missing external tileset {}",
                ext_path.display()
            )));
        }
        let block_dir = ext_path.parent().unwrap_or(&input_dir).to_path_buf();
        let block_id = block_dir
            .file_name()
            .map(|s| s.to_string_lossy().to_string())
            .unwrap_or_else(|| format!("Tile_+{grid_x}_+{grid_y}"));

        let child_local = parse_transform(child);
        let child_world = mul_transform(&root_world, &child_local);
        let child_bounds = child
            .get("boundingVolume")
            .and_then(parse_box)
            .unwrap_or_else(BoundingVolume::empty);

        let ext_doc = load_json(&ext_path)?;
        let ext_root = ext_doc
            .get("root")
            .ok_or_else(|| TopRebuildError::InvalidTileset(format!("{rel} missing root")))?;

        let mut representations = Vec::new();
        let mut counter = 0usize;
        collect_representations(
            ext_root,
            &block_dir,
            &child_world,
            &block_id,
            &mut representations,
            &mut counter,
        );
        if representations.is_empty() {
            return Err(TopRebuildError::NoRepresentations(block_id));
        }

        let bounds = if child_bounds.box_values.is_some() {
            child_bounds
        } else {
            representations[0].bounds.clone()
        };

        blocks.push(SourceBlock {
            id: block_id,
            grid_x: Some(grid_x),
            grid_y: Some(grid_y),
            bounds,
            world_transform: child_world,
            representations,
        });
    }

    if blocks.is_empty() {
        return Err(TopRebuildError::NoSourceBlocks);
    }

    validate_grid_spatial(&blocks)?;
    blocks.sort_by_key(|b| (b.grid_y.unwrap_or(0), b.grid_x.unwrap_or(0)));
    Ok(blocks)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normalize_uri_strips_dot_slash() {
        assert_eq!(
            normalize_uri("./Data/Tile_+000_+000/tileset.json"),
            "Data/Tile_+000_+000/tileset.json"
        );
        assert_eq!(normalize_uri(".//Data/x"), "Data/x");
    }

    #[test]
    fn grid_spatial_mismatch_detected() {
        use crate::types::{BoundingVolume, Mat4d, Representation, SourceBlock};
        let mk = |id: &str, gx: i32, gy: i32, cx: f64, cy: f64| SourceBlock {
            id: id.into(),
            grid_x: Some(gx),
            grid_y: Some(gy),
            bounds: BoundingVolume::from_box([
                cx, cy, 0.0, 50.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 10.0,
            ]),
            world_transform: Mat4d::identity(),
            representations: vec![Representation {
                id: format!("{id}#r0"),
                content_path: "x.b3dm".into(),
                geometric_error_meters: 10.0,
                triangle_count: 0,
                texture_bytes: 0,
                bounds: BoundingVolume::empty(),
                world_transform: Mat4d::identity(),
            }],
        };
        let blocks = vec![
            mk("A", 0, 0, 50.0, 50.0),
            mk("B", 1, 0, 50.0, 50.0),
            mk("C", 0, 1, 50.0, 150.0),
        ];
        let err = validate_grid_spatial(&blocks).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("GRID_SPATIAL_MISMATCH"), "{msg}");
    }
}
