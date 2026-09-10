//! Representation selector (plan §10, Phase 11 P0-3).
//!
//! Rule: among complete coverage frontiers with
//!   sourceError <= targetProxyError * sourceErrorRatio
//! pick the **coarsest** (largest geometricError). Fallback: coarsest overall.
//! Incomplete frontiers must never be selected (adapter rejects them).

use crate::error::{Result, TopRebuildError};
use crate::types::{Representation, SourceBlock};

pub const DEFAULT_SOURCE_ERROR_RATIO: f64 = 0.5;

#[derive(Clone, Debug)]
pub struct Selection {
    pub source_block_id: String,
    pub representation_index: usize,
    pub representation_id: String,
    pub source_error: f64,
    pub triangle_count: u64,
    pub texture_bytes: u64,
    pub frontier_parts: usize,
}

/// Select one Representation (complete coverage frontier) per SourceBlock.
pub fn select_for_blocks(
    blocks: &[SourceBlock],
    target_proxy_error: f64,
    source_error_ratio: f64,
) -> Result<Vec<(usize, Selection)>> {
    let mut out = Vec::with_capacity(blocks.len());
    for (bi, block) in blocks.iter().enumerate() {
        let sel = select_one(block, target_proxy_error, source_error_ratio)?;
        out.push((bi, sel));
    }
    Ok(out)
}

pub fn select_one(
    block: &SourceBlock,
    target_proxy_error: f64,
    source_error_ratio: f64,
) -> Result<Selection> {
    if block.representations.is_empty() {
        return Err(TopRebuildError::NoRepresentations(block.id.clone()));
    }
    // Reject empty-part reps as incomplete coverage.
    for rep in &block.representations {
        if rep.parts.is_empty() {
            return Err(TopRebuildError::source_coverage_incomplete(format!(
                "block {} representation {} has zero parts",
                block.id, rep.id
            )));
        }
    }
    let threshold = target_proxy_error * source_error_ratio;

    let mut best_ok: Option<(usize, &Representation)> = None;
    for (i, rep) in block.representations.iter().enumerate() {
        if rep.geometric_error_meters <= threshold {
            match best_ok {
                None => best_ok = Some((i, rep)),
                Some((_, cur)) if rep.geometric_error_meters > cur.geometric_error_meters => {
                    best_ok = Some((i, rep));
                }
                _ => {}
            }
        }
    }

    let (index, rep) = if let Some(v) = best_ok {
        v
    } else {
        block
            .representations
            .iter()
            .enumerate()
            .max_by(|(_, a), (_, b)| {
                a.geometric_error_meters
                    .partial_cmp(&b.geometric_error_meters)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .unwrap()
    };

    Ok(Selection {
        source_block_id: block.id.clone(),
        representation_index: index,
        representation_id: rep.id.clone(),
        source_error: rep.geometric_error_meters,
        triangle_count: rep.triangle_count,
        texture_bytes: rep.texture_bytes,
        frontier_parts: rep.parts.len(),
    })
}

/// Phase 5 default: use max source GE across blocks as a stand-in target proxy error scale.
pub fn default_target_proxy_error(blocks: &[SourceBlock]) -> f64 {
    blocks
        .iter()
        .flat_map(|b| b.representations.iter())
        .map(|r| r.geometric_error_meters)
        .fold(0.0_f64, f64::max)
        * 2.0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{BoundingVolume, Mat4d};
    use std::path::PathBuf;

    fn block_with(ges: &[f64]) -> SourceBlock {
        let reps: Vec<_> = ges
            .iter()
            .enumerate()
            .map(|(i, &ge)| {
                Representation::single_part(
                    format!("r{i}"),
                    format!("c{i}.b3dm").into(),
                    ge,
                    BoundingVolume::empty(),
                    Mat4d::identity(),
                )
            })
            .collect();
        SourceBlock {
            id: "B".into(),
            grid_x: Some(0),
            grid_y: Some(0),
            bounds: BoundingVolume::empty(),
            world_transform: Mat4d::identity(),
            source_tileset_path: PathBuf::from("tileset.json"),
            source_block_dir: PathBuf::from("."),
            representations: reps,
        }
    }

    #[test]
    fn picks_coarsest_satisfying_ratio() {
        let b = block_with(&[50.0, 10.0, 2.0]);
        let s = select_one(&b, 100.0, 0.5).unwrap();
        assert_eq!(s.representation_index, 0);
        assert_eq!(s.source_error, 50.0);
        assert_eq!(s.frontier_parts, 1);
    }

    #[test]
    fn fallback_when_none_satisfy() {
        let b = block_with(&[50.0, 20.0, 10.0]);
        let s = select_one(&b, 5.0, 0.5).unwrap();
        assert_eq!(s.source_error, 50.0);
    }
}
