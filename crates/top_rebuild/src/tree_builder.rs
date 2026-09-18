//! Regular Quadtree TreeBuilder — bottom-up aggregation (plan §11).
//!
//! Phase 5: hierarchy / bounds / transform / source representation ids only.
//! No mesh simplification or ProxyBuilder geometry merge.
//! Phase 11: TreeBuilder still only decides spatial block composition / bounds / levels.

use crate::error::{Result, TopRebuildError};
use crate::selector::{self, Selection, DEFAULT_SOURCE_ERROR_RATIO};
use crate::types::{BoundingVolume, Mat4d, RebuildTree, SourceBlock, TreeNode};
use std::collections::BTreeMap;

#[derive(Clone, Debug)]
pub struct TreeBuildOptions {
    pub source_error_ratio: f64,
    /// Optional override for representation selection target error.
    pub target_proxy_error: Option<f64>,
    /// Max aggregation levels beyond L0 (None = until single root or no reduction).
    pub max_levels: Option<u32>,
}

impl Default for TreeBuildOptions {
    fn default() -> Self {
        Self {
            source_error_ratio: DEFAULT_SOURCE_ERROR_RATIO,
            target_proxy_error: None,
            max_levels: None,
        }
    }
}

fn parent_grid(x: i32, y: i32) -> (i32, i32) {
    // plan §11.2: parentX = floor(childX / 2); parentY = floor(childY / 2)
    (x.div_euclid(2), y.div_euclid(2))
}

fn stub_proxy_from_children(level: u32, px: i32, py: i32, children: &[TreeNode]) -> TreeNode {
    let bounds = BoundingVolume::union_all(
        &children
            .iter()
            .map(|c| c.bounds.clone())
            .collect::<Vec<_>>(),
    );
    let world_transform = if let Some((cx, cy, cz)) = bounds.center() {
        Mat4d::translation(cx, cy, cz)
    } else {
        Mat4d::identity()
    };
    let mut source_representation_ids = Vec::new();
    let mut child_ids = Vec::new();
    for c in children {
        child_ids.push(c.id.clone());
        for id in &c.source_representation_ids {
            if !source_representation_ids.contains(id) {
                source_representation_ids.push(id.clone());
            }
        }
    }
    TreeNode {
        id: format!("Proxy_L{level}_{px}_{py}"),
        level,
        grid_x: px,
        grid_y: py,
        bounds,
        world_transform,
        source_representation_ids,
        child_ids,
    }
}

fn l0_nodes(blocks: &[SourceBlock], selections: &[(usize, Selection)]) -> Result<Vec<TreeNode>> {
    let mut nodes = Vec::with_capacity(selections.len());
    for (bi, sel) in selections {
        let block = &blocks[*bi];
        let gx = block.grid_x.ok_or_else(|| {
            TopRebuildError::Other(format!("block {} missing gridX", block.id))
        })?;
        let gy = block.grid_y.ok_or_else(|| {
            TopRebuildError::Other(format!("block {} missing gridY", block.id))
        })?;
        let rep = &block.representations[sel.representation_index];
        nodes.push(TreeNode {
            id: format!("L0_{}", block.id),
            level: 0,
            grid_x: gx,
            grid_y: gy,
            bounds: block.bounds.clone(),
            world_transform: block.world_transform.clone(),
            source_representation_ids: vec![rep.id.clone()],
            child_ids: vec![],
        });
    }
    nodes.sort_by_key(|n| (n.grid_y, n.grid_x));
    Ok(nodes)
}

fn aggregate_one_level(level_idx: u32, current: &[TreeNode]) -> Result<Vec<TreeNode>> {
    let mut groups: BTreeMap<(i32, i32), Vec<&TreeNode>> = BTreeMap::new();
    for node in current {
        let key = parent_grid(node.grid_x, node.grid_y);
        groups.entry(key).or_default().push(node);
    }
    let mut next = Vec::with_capacity(groups.len());
    for ((px, py), kids) in groups {
        let owned: Vec<TreeNode> = kids.into_iter().cloned().collect();
        next.push(stub_proxy_from_children(level_idx, px, py, &owned));
    }
    next.sort_by_key(|n| (n.grid_y, n.grid_x));
    Ok(next)
}

/// Build regular quadtree bottom-up from SourceBlocks (plan §11.3).
pub fn build_tree(blocks: &[SourceBlock], opts: &TreeBuildOptions) -> Result<RebuildTree> {
    if blocks.is_empty() {
        return Err(TopRebuildError::NoSourceBlocks);
    }
    let target = opts
        .target_proxy_error
        .unwrap_or_else(|| selector::default_target_proxy_error(blocks));
    let selections = selector::select_for_blocks(blocks, target, opts.source_error_ratio)?;
    let mut levels: Vec<Vec<TreeNode>> = Vec::new();
    let mut current = l0_nodes(blocks, &selections)?;
    levels.push(current.clone());

    let mut level_idx = 1u32;
    loop {
        if current.len() <= 1 {
            break;
        }
        if let Some(max) = opts.max_levels {
            if level_idx > max {
                break;
            }
        }
        let next = aggregate_one_level(level_idx, &current)?;
        if next.len() >= current.len() {
            return Err(TopRebuildError::NoFurtherReduction {
                size: current.len(),
            });
        }
        levels.push(next.clone());
        current = next;
        level_idx += 1;
    }

    Ok(RebuildTree { levels })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::Representation;
    use std::path::PathBuf;

    fn synth_block(gx: i32, gy: i32) -> SourceBlock {
        let cell = 100.0;
        let half = 50.0;
        let cx = gx as f64 * cell + half;
        let cy = gy as f64 * cell + half;
        let boxv = [cx, cy, 10.0, half, 0.0, 0.0, 0.0, half, 0.0, 0.0, 0.0, 10.0];
        let id = format!("Tile_+{gx:03}_+{gy:03}");
        SourceBlock {
            id: id.clone(),
            grid_x: Some(gx),
            grid_y: Some(gy),
            bounds: BoundingVolume::from_box(boxv),
            world_transform: Mat4d::identity(),
            source_tileset_path: PathBuf::from(format!("{id}/tileset.json")),
            source_block_dir: PathBuf::from(id.clone()),
            representations: vec![Representation::single_part(
                format!("{id}#rep0"),
                format!("{id}.b3dm").into(),
                50.0,
                BoundingVolume::from_box(boxv),
                Mat4d::identity(),
            )],
        }
    }

    #[test]
    fn parent_grid_floor() {
        assert_eq!(parent_grid(0, 0), (0, 0));
        assert_eq!(parent_grid(1, 1), (0, 0));
        assert_eq!(parent_grid(2, 3), (1, 1));
        assert_eq!(parent_grid(-1, -2), (-1, -1));
    }

    #[test]
    fn grid_2x2_to_one() {
        let blocks: Vec<_> = (0..2)
            .flat_map(|y| (0..2).map(move |x| synth_block(x, y)))
            .collect();
        let tree = build_tree(&blocks, &TreeBuildOptions::default()).unwrap();
        assert_eq!(tree.levels.len(), 2);
        assert_eq!(tree.levels[0].len(), 4);
        assert_eq!(tree.levels[1].len(), 1);
    }

    #[test]
    fn grid_4x4_to_4_to_1() {
        let blocks: Vec<_> = (0..4)
            .flat_map(|y| (0..4).map(move |x| synth_block(x, y)))
            .collect();
        let tree = build_tree(&blocks, &TreeBuildOptions::default()).unwrap();
        let counts = tree.level_counts();
        assert_eq!(counts, vec![(0, 16), (1, 4), (2, 1)]);
    }

    #[test]
    fn missing_corner_still_aggregates() {
        let blocks = vec![synth_block(0, 0), synth_block(1, 0), synth_block(0, 1)];
        let tree = build_tree(&blocks, &TreeBuildOptions::default()).unwrap();
        assert_eq!(tree.levels[0].len(), 3);
        assert_eq!(tree.levels[1].len(), 1);
        assert_eq!(tree.levels[1][0].child_ids.len(), 3);
    }

    #[test]
    fn negative_grid_index_parent() {
        assert_eq!(parent_grid(-1, 0), (-1, 0));
        assert_eq!(parent_grid(-2, -3), (-1, -2));
        let blocks = vec![synth_block(-2, -2), synth_block(-1, -2), synth_block(-2, -1), synth_block(-1, -1)];
        let tree = build_tree(&blocks, &TreeBuildOptions::default()).unwrap();
        assert_eq!(tree.levels[0].len(), 4);
        assert_eq!(tree.levels[1].len(), 1);
        assert_eq!(tree.levels[1][0].grid_x, -1);
        assert_eq!(tree.levels[1][0].grid_y, -1);
    }
}
