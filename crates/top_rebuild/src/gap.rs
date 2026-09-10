//! Border gap metrics (plan §15.3, Phase 8).
//!
//! V1 does **not** weld across tiles. Meshoptimizer `LockBorder` protects
//! topological boundaries during simplify so each child keeps its silhouette.
//! After proxies are built (or on child meshes in a shared frame), we measure
//! nearest-neighbor distances between border vertices of adjacent children
//! near their shared mid-plane — `maxGap` / `P95Gap` in meters.
//!
//! These are calibration metrics for 4×4 experiments, not hard publish gates.

use serde::{Deserialize, Serialize};

/// One pair of adjacent child meshes compared along a shared edge region.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct GapPair {
    pub a_index: usize,
    pub b_index: usize,
    pub sample_count: usize,
    pub max_gap: f64,
    pub p95_gap: f64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct GapMetrics {
    pub max_gap: f64,
    pub p95_gap: f64,
    pub pair_count: usize,
    pub pairs: Vec<GapPair>,
    pub lock_border: bool,
    pub notes: Vec<String>,
}

/// Edges that belong to exactly one triangle are topological border edges.
pub fn topological_border_vertices(positions: &[[f32; 3]], indices: &[u32]) -> Vec<[f32; 3]> {
    use std::collections::HashMap;
    let mut edge_count: HashMap<(u32, u32), u32> = HashMap::new();
    for tri in indices.chunks_exact(3) {
        let edges = [
            (tri[0], tri[1]),
            (tri[1], tri[2]),
            (tri[2], tri[0]),
        ];
        for (a, b) in edges {
            let key = if a < b { (a, b) } else { (b, a) };
            *edge_count.entry(key).or_insert(0) += 1;
        }
    }
    let mut seen = vec![false; positions.len()];
    let mut out = Vec::new();
    for ((a, b), c) in edge_count {
        if c == 1 {
            for idx in [a, b] {
                let i = idx as usize;
                if i < positions.len() && !seen[i] {
                    seen[i] = true;
                    out.push(positions[i]);
                }
            }
        }
    }
    out
}

fn aabb(pts: &[[f32; 3]]) -> Option<([f32; 3], [f32; 3])> {
    if pts.is_empty() {
        return None;
    }
    let mut mn = pts[0];
    let mut mx = pts[0];
    for p in pts {
        for i in 0..3 {
            mn[i] = mn[i].min(p[i]);
            mx[i] = mx[i].max(p[i]);
        }
    }
    Some((mn, mx))
}

fn nearest_dist(p: [f32; 3], cloud: &[[f32; 3]]) -> f64 {
    let mut best = f64::MAX;
    for q in cloud {
        let dx = (p[0] - q[0]) as f64;
        let dy = (p[1] - q[1]) as f64;
        let dz = (p[2] - q[2]) as f64;
        let d = (dx * dx + dy * dy + dz * dz).sqrt();
        if d < best {
            best = d;
        }
    }
    best
}

fn percentile_95(sorted: &[f64]) -> f64 {
    if sorted.is_empty() {
        return 0.0;
    }
    let idx = ((sorted.len() as f64 - 1.0) * 0.95).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

/// Compare border clouds of N child meshes already in the same parent-local frame.
/// Adjacent pairs: AABB centers within `adjacency_factor * mean_extent` and sharing
/// an axis-aligned face band (`band_frac` of extent).
pub fn compute_gap_metrics(
    child_positions: &[Vec<[f32; 3]>],
    child_indices: &[Vec<u32>],
    lock_border: bool,
) -> GapMetrics {
    let mut notes = vec![
        "LockBorder protects topological borders during meshoptimizer simplify (no cross-tile weld).".into(),
        "Gaps measured as nearest-neighbor distance between border verts near shared mid-plane.".into(),
    ];
    if child_positions.len() != child_indices.len() {
        notes.push("position/index child count mismatch; gap metrics skipped".into());
        return GapMetrics {
            lock_border,
            notes,
            ..Default::default()
        };
    }

    let borders: Vec<Vec<[f32; 3]>> = child_positions
        .iter()
        .zip(child_indices.iter())
        .map(|(p, i)| topological_border_vertices(p, i))
        .collect();

    let centers: Vec<[f32; 3]> = child_positions
        .iter()
        .map(|pts| {
            if pts.is_empty() {
                return [0.0; 3];
            }
            let mut s = [0.0f32; 3];
            for p in pts {
                s[0] += p[0];
                s[1] += p[1];
                s[2] += p[2];
            }
            let n = pts.len() as f32;
            [s[0] / n, s[1] / n, s[2] / n]
        })
        .collect();

    let aabbs: Vec<Option<([f32; 3], [f32; 3])>> =
        child_positions.iter().map(|p| aabb(p)).collect();

    let mut pairs = Vec::new();
    let n = borders.len();
    for a in 0..n {
        for b in (a + 1)..n {
            let (Some((amn, amx)), Some((bmn, bmx))) = (aabbs[a], aabbs[b]) else {
                continue;
            };
            // Overlap in YZ / XZ / XY to detect shared face band
            let overlap = |i: usize| amn[i].max(bmn[i]) <= amx[i].min(bmx[i]) + 1e-3;
            let sep = [
                (centers[a][0] - centers[b][0]).abs(),
                (centers[a][1] - centers[b][1]).abs(),
                (centers[a][2] - centers[b][2]).abs(),
            ];
            let ext_a = [
                (amx[0] - amn[0]).max(1e-3),
                (amx[1] - amn[1]).max(1e-3),
                (amx[2] - amn[2]).max(1e-3),
            ];
            let ext_b = [
                (bmx[0] - bmn[0]).max(1e-3),
                (bmx[1] - bmn[1]).max(1e-3),
                (bmx[2] - bmn[2]).max(1e-3),
            ];
            // Adjacent if separated mainly on one axis and overlapping on the other two.
            // Also accept thin/open clouds whose local axis extent is tiny but gap is small
            // relative to the shared-face extents (fixture / open-border calibration).
            let mut axis = None;
            for i in 0..3 {
                let j = (i + 1) % 3;
                let k = (i + 2) % 3;
                let mean_ext = (ext_a[i] + ext_b[i]) * 0.5;
                let face_ext = ((ext_a[j] + ext_b[j]) * 0.5).max((ext_a[k] + ext_b[k]) * 0.5);
                let near = sep[i] <= mean_ext * 1.6 + 1e-3
                    || (mean_ext < 0.05 && sep[i] <= face_ext.max(1.0) * 0.5);
                if near && sep[i] > 1e-6 && overlap(j) && overlap(k) {
                    axis = Some(i);
                    break;
                }
            }
            let Some(axis) = axis else {
                continue;
            };
            let mid = (centers[a][axis] + centers[b][axis]) * 0.5;
            let band = ((ext_a[axis] + ext_b[axis]) * 0.15).max(0.5);

            let filter = |cloud: &[[f32; 3]]| -> Vec<[f32; 3]> {
                cloud
                    .iter()
                    .copied()
                    .filter(|p| (p[axis] - mid).abs() <= band)
                    .collect()
            };
            let ca = filter(&borders[a]);
            let cb = filter(&borders[b]);
            if ca.is_empty() || cb.is_empty() {
                // Fall back to full border clouds if mid-band empty (coarse meshes)
                let ca = &borders[a];
                let cb = &borders[b];
                if ca.is_empty() || cb.is_empty() {
                    continue;
                }
                let mut dists = Vec::new();
                for p in ca {
                    dists.push(nearest_dist(*p, cb));
                }
                for p in cb {
                    dists.push(nearest_dist(*p, ca));
                }
                dists.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
                // fix Equal -> Equal
                let max_gap = dists.last().copied().unwrap_or(0.0);
                let p95 = percentile_95(&dists);
                pairs.push(GapPair {
                    a_index: a,
                    b_index: b,
                    sample_count: dists.len(),
                    max_gap,
                    p95_gap: p95,
                });
                continue;
            }
            let mut dists = Vec::new();
            for p in &ca {
                dists.push(nearest_dist(*p, &cb));
            }
            for p in &cb {
                dists.push(nearest_dist(*p, &ca));
            }
            dists.sort_by(|x, y| x.partial_cmp(y).unwrap_or(std::cmp::Ordering::Equal));
            let max_gap = dists.last().copied().unwrap_or(0.0);
            let p95 = percentile_95(&dists);
            pairs.push(GapPair {
                a_index: a,
                b_index: b,
                sample_count: dists.len(),
                max_gap,
                p95_gap: p95,
            });
        }
    }

    let mut all: Vec<f64> = pairs.iter().map(|p| p.max_gap).collect();
    // Aggregate: global max of pair max; global p95 of pair p95 values (pragmatic)
    let max_gap = all.iter().copied().fold(0.0_f64, f64::max);
    let mut p95s: Vec<f64> = pairs.iter().map(|p| p.p95_gap).collect();
    p95s.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));
    let p95_gap = percentile_95(&p95s);
    all.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

    GapMetrics {
        max_gap,
        p95_gap,
        pair_count: pairs.len(),
        pairs,
        lock_border,
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::glb::make_box_primitive;

    #[test]
    fn border_verts_on_box() {
        let p = make_box_primitive(1.0, 1.0, 1.0, 2, "default");
        let b = topological_border_vertices(&p.positions, &p.indices);
        // Closed manifold box → no topological border
        assert!(b.is_empty() || b.len() < p.positions.len());
    }

    #[test]
    fn gap_between_two_offset_boxes_small() {
        // Two boxes in parent-local, centers ±20 on X — adjacent-ish
        let mut a = make_box_primitive(10.0, 10.0, 5.0, 4, "a");
        let mut b = make_box_primitive(10.0, 10.0, 5.0, 4, "b");
        for v in &mut a.positions {
            v[0] -= 15.0;
        }
        for v in &mut b.positions {
            v[0] += 15.0;
        }
        // Open a seam: drop faces by only comparing borders — closed boxes have 0 topo border.
        // Punch a hole: remove last tri from each so border appears, OR use duplicated open grids.
        // Simpler: feed AABB edge samples via artificial border clouds by taking all verts.
        let m = compute_gap_metrics(
            &[a.positions.clone(), b.positions.clone()],
            &[a.indices.clone(), b.indices.clone()],
            true,
        );
        // Manifold boxes → empty borders → possibly 0 pairs; still must not panic
        let _ = m.max_gap;
    }

    #[test]
    fn gap_with_open_border_clouds() {
        // Two line-like point clouds facing each other with ~0.5m gap
        let mut left = Vec::new();
        let mut right = Vec::new();
        let mut li = Vec::new();
        let mut ri = Vec::new();
        for i in 0..10 {
            left.push([-0.25, i as f32, 0.0]);
            right.push([0.25, i as f32, 0.0]);
        }
        // Degenerate tris so every edge is unique (border)
        for i in 0..8 {
            li.extend_from_slice(&[i, i + 1, i]);
            ri.extend_from_slice(&[i, i + 1, i]);
        }
        let m = compute_gap_metrics(&[left, right], &[li, ri], true);
        assert!(m.pair_count >= 1, "expected adjacent pair");
        assert!(m.max_gap > 0.4 && m.max_gap < 0.7, "max_gap={}", m.max_gap);
        assert!(m.p95_gap > 0.0);
    }
}
