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

fn parse_box(bv: &Value) -> Result<Option<BoundingVolume>> {
    let Some(value) = bv.get("box") else {
        return Ok(None);
    };
    let arr = value.as_array().ok_or_else(|| {
        TopRebuildError::InvalidTileset("boundingVolume.box must be an array".into())
    })?;
    if arr.len() != 12 {
        return Err(TopRebuildError::InvalidTileset(format!(
            "boundingVolume.box must contain 12 values (got {})",
            arr.len()
        )));
    }
    let mut vals = [0.0; 12];
    for (i, v) in arr.iter().take(12).enumerate() {
        vals[i] = v.as_f64().ok_or_else(|| {
            TopRebuildError::InvalidTileset(format!("boundingVolume.box[{i}] must be a number"))
        })?;
        if !vals[i].is_finite() {
            return Err(TopRebuildError::InvalidTileset(format!(
                "boundingVolume.box[{i}] must be finite"
            )));
        }
    }
    Ok(Some(BoundingVolume::from_box(vals)))
}

fn parse_transform(node: &Value) -> Result<Mat4d> {
    let Some(value) = node.get("transform") else {
        return Ok(Mat4d::identity());
    };
    let arr = value
        .as_array()
        .ok_or_else(|| TopRebuildError::InvalidTileset("transform must be an array".into()))?;
    if arr.len() != 16 {
        return Err(TopRebuildError::InvalidTileset(format!(
            "transform must contain 16 values (got {})",
            arr.len()
        )));
    }
    let mut vals = [0.0; 16];
    for (i, v) in arr.iter().enumerate() {
        vals[i] = v.as_f64().ok_or_else(|| {
            TopRebuildError::InvalidTileset(format!("transform[{i}] must be a number"))
        })?;
        if !vals[i].is_finite() {
            return Err(TopRebuildError::InvalidTileset(format!(
                "transform[{i}] must be finite"
            )));
        }
    }
    Ok(Mat4d(vals))
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
) -> Result<()> {
    let local = parse_transform(node)?;
    let world = mul_transform(parent_world, &local);
    let ge = node
        .get("geometricError")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.0);
    let bounds = match node.get("boundingVolume") {
        Some(value) => parse_box(value)?.unwrap_or_else(BoundingVolume::empty),
        None => BoundingVolume::empty(),
    };

    if let Some(uri) = node
        .get("content")
        .and_then(|c| c.get("uri"))
        .and_then(|u| u.as_str())
    {
        let rel = normalize_uri(uri);
        let content_path = block_dir.join(&rel);
        let rep_id = format!("{block_id}#rep{counter}");
        *counter += 1;
        out.push(Representation::single_part(
            rep_id,
            content_path,
            ge,
            bounds.clone(),
            world.clone(),
        ));
    }

    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for child in children {
            collect_representations(child, block_dir, &world, block_id, out, counter)?;
        }
    }
    Ok(())
}

fn load_json(path: &Path) -> Result<Value> {
    let text = fs::read_to_string(path)?;
    Ok(serde_json::from_str(&text)?)
}

fn world_center(block: &SourceBlock) -> Option<(f64, f64, f64)> {
    block.bounds.transformed_center(&block.world_transform)
}

fn grid_xy_in_frame(block: &SourceBlock, frame_inv: &Mat4d) -> Option<(f64, f64)> {
    let (wx, wy, wz) = world_center(block)?;
    let p = frame_inv.transform_point(wx, wy, wz);
    Some((p.0, p.1))
}

/// Validate grid indices against world-space centers, compared in the
/// origin block's local frame (so a shared ECEF root does not distort XY).
pub fn validate_grid_spatial(blocks: &[SourceBlock]) -> Result<()> {
    let with_grid: Vec<_> = blocks
        .iter()
        .filter(|b| b.grid_x.is_some() && b.grid_y.is_some() && world_center(b).is_some())
        .collect();
    if with_grid.len() < 2 {
        return Ok(());
    }

    let origin = with_grid
        .iter()
        .min_by_key(|b| (b.grid_x.unwrap(), b.grid_y.unwrap()))
        .unwrap();
    let frame_inv = origin
        .world_transform
        .inverse()
        .unwrap_or_else(Mat4d::identity);

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
            let (acx, acy) = grid_xy_in_frame(a, &frame_inv).unwrap();
            let (bcx, bcy) = grid_xy_in_frame(b, &frame_inv).unwrap();
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

    let (ox, oy) = grid_xy_in_frame(origin, &frame_inv).unwrap();
    let (ogx, ogy) = (origin.grid_x.unwrap(), origin.grid_y.unwrap());

    let cell_x = if cell_x > 1e-6 { cell_x } else { cell_y };
    let cell_y = if cell_y > 1e-6 { cell_y } else { cell_x };
    let tol = (cell_x.max(cell_y) * 0.35).max(1.0);
    for b in &with_grid {
        let (gx, gy) = (b.grid_x.unwrap(), b.grid_y.unwrap());
        let (cx, cy) = grid_xy_in_frame(b, &frame_inv).unwrap();
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
///
/// # Phase 11 P0-2: External Tileset Preservation
///
/// Records the absolute path to each external tileset (source_tileset_path)
/// and its directory (source_block_dir) for later preservation during rebuild.
/// This supports the Phase 11 requirement to retain original Block external
/// tilesets rather than flattening all content into a single output tileset.
///
/// The recorded paths enable tileset_writer to:
/// - Copy external tilesets to the output directory
/// - Preserve subtree structure and LOD hierarchies
/// - Maintain world-space transform invariants
///
/// See also: tileset_writer::preserve_block_subtree (Phase 11 P0-2)
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
    let root_world = parse_transform(root)?;
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
        let grid_x: i32 = caps
            .get(1)
            .ok_or_else(|| TopRebuildError::InvalidTileset("missing Tile X coordinate".into()))?
            .as_str()
            .parse()
            .map_err(|_| {
                TopRebuildError::InvalidTileset(format!("Tile X coordinate is outside i32: {rel}"))
            })?;
        let grid_y: i32 = caps
            .get(2)
            .ok_or_else(|| TopRebuildError::InvalidTileset("missing Tile Y coordinate".into()))?
            .as_str()
            .parse()
            .map_err(|_| {
                TopRebuildError::InvalidTileset(format!("Tile Y coordinate is outside i32: {rel}"))
            })?;

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

        let child_local = parse_transform(child)?;
        let child_world = mul_transform(&root_world, &child_local);
        let child_bounds = match child.get("boundingVolume") {
            Some(value) => parse_box(value)?.unwrap_or_else(BoundingVolume::empty),
            None => BoundingVolume::empty(),
        };

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
        )?;
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
            source_tileset_path: ext_path,
            source_block_dir: block_dir,
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
    use serde_json::json;

    #[test]
    fn normalize_uri_strips_dot_slash() {
        assert_eq!(
            normalize_uri("./Data/Tile_+000_+000/tileset.json"),
            "Data/Tile_+000_+000/tileset.json"
        );
        assert_eq!(normalize_uri(".//Data/x"), "Data/x");
    }

    #[test]
    fn rejects_non_numeric_bounding_box_values() {
        let value = json!({
            "box": [0, 0, 0, 1, 0, 0, 0, 1, 0, 0, "broken", 1]
        });
        let error = parse_box(&value).expect_err("invalid box must be rejected");
        assert!(error.to_string().contains("boundingVolume.box[10]"));
    }

    #[test]
    fn rejects_malformed_transform_shape() {
        let value = json!({ "transform": [1, 0, 0] });
        let error = parse_transform(&value).expect_err("short transform must be rejected");
        assert!(error
            .to_string()
            .contains("transform must contain 16 values"));
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
            representations: vec![Representation::single_part(
                format!("{id}#r0"),
                "x.b3dm".into(),
                10.0,
                BoundingVolume::empty(),
                Mat4d::identity(),
            )],
            source_tileset_path: PathBuf::new(),
            source_block_dir: PathBuf::new(),
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

    #[test]
    fn grid_spatial_valid_when_centers_come_from_tile_transform() {
        let mk = |id: &str, gx: i32, gy: i32, tx: f64, ty: f64| SourceBlock {
            id: id.into(),
            grid_x: Some(gx),
            grid_y: Some(gy),
            bounds: BoundingVolume::from_box([
                0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 10.0,
            ]),
            world_transform: Mat4d::translation(tx, ty, 0.0),
            representations: vec![Representation::single_part(
                format!("{id}#r0"),
                "x.b3dm".into(),
                10.0,
                BoundingVolume::empty(),
                Mat4d::translation(tx, ty, 0.0),
            )],
            source_tileset_path: PathBuf::new(),
            source_block_dir: PathBuf::new(),
        };
        let blocks = vec![
            mk("Tile_0_0", 0, 0, 0.0, 0.0),
            mk("Tile_1_0", 1, 0, 100.0, 0.0),
        ];
        validate_grid_spatial(&blocks).expect("grid spatial valid");
    }

    #[test]
    fn grid_spatial_valid_under_shared_ecef_rotation() {
        let ecef = Mat4d([
            0.0,
            1.0,
            0.0,
            0.0,
            -1.0,
            0.0,
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            3_900_000.0,
            1_000_000.0,
            4_800_000.0,
            1.0,
        ]);
        let mk = |id: &str, gx: i32, gy: i32, cx: f64, cy: f64| SourceBlock {
            id: id.into(),
            grid_x: Some(gx),
            grid_y: Some(gy),
            bounds: BoundingVolume::from_box([
                cx, cy, 0.0, 50.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 10.0,
            ]),
            world_transform: ecef.clone(),
            representations: vec![Representation::single_part(
                format!("{id}#r0"),
                "x.b3dm".into(),
                10.0,
                BoundingVolume::empty(),
                ecef.clone(),
            )],
            source_tileset_path: PathBuf::new(),
            source_block_dir: PathBuf::new(),
        };
        let blocks = vec![mk("A", 0, 0, 0.0, 0.0), mk("B", 1, 0, 100.0, 0.0)];
        validate_grid_spatial(&blocks).expect("ecef rotation must not collapse grid xy");
    }

    #[test]
    fn external_tileset_paths_recorded() {
        use std::fs;
        use tempfile::TempDir;

        let tmp = TempDir::new().unwrap();
        let root_path = tmp.path().join("tileset.json");
        let tile_dir = tmp.path().join("Data").join("Tile_+000_+000");
        fs::create_dir_all(&tile_dir).unwrap();

        let tile_ts = tile_dir.join("tileset.json");
        let tile_b3dm = tile_dir.join("Tile_+000_+000.b3dm");
        fs::write(&tile_b3dm, b"fake b3dm").unwrap();

        fs::write(
            &tile_ts,
            r#"{
                "root": {
                    "boundingVolume": {
                        "box": [0,0,0,50,0,0,0,50,0,0,0,10]
                    },
                    "geometricError": 100.0,
                    "refine": "REPLACE",
                    "content": {
                        "uri": "./Tile_+000_+000.b3dm"
                    }
                }
            }"#,
        ).unwrap();

        fs::write(
            &root_path,
            r#"{
                "root": {
                    "boundingVolume": {
                        "box": [0,0,0,100,0,0,0,100,0,0,0,50]
                    },
                    "geometricError": 500.0,
                    "refine": "REPLACE",
                    "children": [
                        {
                            "boundingVolume": {
                                "box": [25,25,0,50,0,0,0,50,0,0,0,10]
                            },
                            "geometricError": 100.0,
                            "content": {
                                "uri": "./Data/Tile_+000_+000/tileset.json"
                            }
                        }
                    ]
                }
            }"#,
        ).unwrap();

        let blocks = load_source_blocks(&root_path).unwrap();
        assert_eq!(blocks.len(), 1);
        let block = &blocks[0];

        assert_eq!(block.grid_x, Some(0));
        assert_eq!(block.grid_y, Some(0));
        assert_eq!(block.source_tileset_path, tile_ts);
        assert_eq!(block.source_block_dir, tile_dir);
        assert!(!block.representations.is_empty());
    }
}
