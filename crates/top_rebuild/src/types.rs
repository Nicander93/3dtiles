//! Core SourceBlock / Representation / math types (plan §§9–11).
//!
//! Coordinate frames:
//! - Mesh vertex = content local (glTF node / B3DM RTC already expanded on load)
//! - Representation.world_transform = content local → world
//! - SourceBlock.world_transform = block local → world
//! - SourceBlock / Representation bounds = local to that world_transform
//! - TreeBuilder spatial work uses world-space bounds
//! - ProxyBuilder mesh = parent local

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Column-major 4×4 double matrix (3D Tiles `transform` layout).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Mat4d(pub [f64; 16]);

impl Default for Mat4d {
    fn default() -> Self {
        Self::identity()
    }
}

impl Mat4d {
    pub fn identity() -> Self {
        Self([
            1.0, 0.0, 0.0, 0.0, // col0
            0.0, 1.0, 0.0, 0.0, // col1
            0.0, 0.0, 1.0, 0.0, // col2
            0.0, 0.0, 0.0, 1.0, // col3
        ])
    }

    pub fn translation(tx: f64, ty: f64, tz: f64) -> Self {
        let mut m = Self::identity();
        m.0[12] = tx;
        m.0[13] = ty;
        m.0[14] = tz;
        m
    }

    pub fn from_slice(v: &[f64]) -> Option<Self> {
        if v.len() != 16 {
            return None;
        }
        let mut a = [0.0; 16];
        a.copy_from_slice(v);
        Some(Self(a))
    }

    pub fn mul(&self, other: &Mat4d) -> Mat4d {
        let a = &self.0;
        let b = &other.0;
        let mut o = [0.0; 16];
        for col in 0..4 {
            for row in 0..4 {
                o[col * 4 + row] = a[row] * b[col * 4]
                    + a[4 + row] * b[col * 4 + 1]
                    + a[8 + row] * b[col * 4 + 2]
                    + a[12 + row] * b[col * 4 + 3];
            }
        }
        Mat4d(o)
    }

    pub fn transform_point(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        let m = &self.0;
        let xp = m[0] * x + m[4] * y + m[8] * z + m[12];
        let yp = m[1] * x + m[5] * y + m[9] * z + m[13];
        let zp = m[2] * x + m[6] * y + m[10] * z + m[14];
        let wp = m[3] * x + m[7] * y + m[11] * z + m[15];
        if wp.abs() > 1e-12 {
            (xp / wp, yp / wp, zp / wp)
        } else {
            (xp, yp, zp)
        }
    }
}

/// Cesium-style `boundingVolume.box`: center(3) + half-axis vectors (9).
///
/// Layout: `[cx, cy, cz, axx, axy, axz, ayx, ayy, ayz, azx, azy, azz]`.
/// Frame is the owning node's local space (see crate-level coordinate notes).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundingVolume {
    pub box_values: Option<[f64; 12]>,
}

impl BoundingVolume {
    pub fn from_box(values: [f64; 12]) -> Self {
        Self {
            box_values: Some(values),
        }
    }

    pub fn empty() -> Self {
        Self { box_values: None }
    }

    pub fn center(&self) -> Option<(f64, f64, f64)> {
        self.box_values.map(|b| (b[0], b[1], b[2]))
    }

    pub fn transformed_center(&self, m: &Mat4d) -> Option<(f64, f64, f64)> {
        let (x, y, z) = self.center()?;
        Some(m.transform_point(x, y, z))
    }

    /// Eight OBB corners: `center ± axisX ± axisY ± axisZ`.
    pub fn corners(&self) -> Option<[(f64, f64, f64); 8]> {
        let b = self.box_values?;
        let c = (b[0], b[1], b[2]);
        let ax = (b[3], b[4], b[5]);
        let ay = (b[6], b[7], b[8]);
        let az = (b[9], b[10], b[11]);
        let mut out = [(0.0, 0.0, 0.0); 8];
        let mut i = 0;
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    out[i] = (
                        c.0 + sx * ax.0 + sy * ay.0 + sz * az.0,
                        c.1 + sx * ax.1 + sy * ay.1 + sz * az.1,
                        c.2 + sx * ax.2 + sy * ay.2 + sz * az.2,
                    );
                    i += 1;
                }
            }
        }
        Some(out)
    }

    pub fn aabb_min_max(&self) -> Option<((f64, f64, f64), (f64, f64, f64))> {
        let corners = self.corners()?;
        let mut amin = (f64::MAX, f64::MAX, f64::MAX);
        let mut amax = (f64::MIN, f64::MIN, f64::MIN);
        for (x, y, z) in corners {
            amin.0 = amin.0.min(x);
            amin.1 = amin.1.min(y);
            amin.2 = amin.2.min(z);
            amax.0 = amax.0.max(x);
            amax.1 = amax.1.max(y);
            amax.2 = amax.2.max(z);
        }
        Some((amin, amax))
    }

    fn from_aabb(min: (f64, f64, f64), max: (f64, f64, f64)) -> BoundingVolume {
        let cx = (min.0 + max.0) * 0.5;
        let cy = (min.1 + max.1) * 0.5;
        let cz = (min.2 + max.2) * 0.5;
        let hx = (max.0 - min.0) * 0.5;
        let hy = (max.1 - min.1) * 0.5;
        let hz = (max.2 - min.2) * 0.5;
        BoundingVolume::from_box([cx, cy, cz, hx, 0.0, 0.0, 0.0, hy, 0.0, 0.0, 0.0, hz])
    }

    /// Transform this box to another frame via 8 corners, then wrap as world AABB.
    pub fn transform_bounds(&self, m: &Mat4d) -> BoundingVolume {
        let Some(corners) = self.corners() else {
            return BoundingVolume::empty();
        };
        let mut amin = (f64::MAX, f64::MAX, f64::MAX);
        let mut amax = (f64::MIN, f64::MIN, f64::MIN);
        for (x, y, z) in corners {
            let p = m.transform_point(x, y, z);
            amin.0 = amin.0.min(p.0);
            amin.1 = amin.1.min(p.1);
            amin.2 = amin.2.min(p.2);
            amax.0 = amax.0.max(p.0);
            amax.1 = amax.1.max(p.1);
            amax.2 = amax.2.max(p.2);
        }
        BoundingVolume::from_aabb(amin, amax)
    }

    pub fn world_bounds(&self, world_transform: &Mat4d) -> BoundingVolume {
        self.transform_bounds(world_transform)
    }

    pub fn union(a: &BoundingVolume, b: &BoundingVolume) -> BoundingVolume {
        match (a.aabb_min_max(), b.aabb_min_max()) {
            (Some((amin, amax)), Some((bmin, bmax))) => {
                let min_x = amin.0.min(bmin.0);
                let min_y = amin.1.min(bmin.1);
                let min_z = amin.2.min(bmin.2);
                let max_x = amax.0.max(bmax.0);
                let max_y = amax.1.max(bmax.1);
                let max_z = amax.2.max(bmax.2);
                BoundingVolume::from_aabb(
                    (min_x, min_y, min_z),
                    (max_x, max_y, max_z),
                )
            }
            (Some(_), None) => a.clone(),
            (None, Some(_)) => b.clone(),
            (None, None) => BoundingVolume::empty(),
        }
    }

    pub fn union_all(items: &[BoundingVolume]) -> BoundingVolume {
        let mut acc = BoundingVolume::empty();
        for bv in items {
            acc = BoundingVolume::union(&acc, bv);
        }
        acc
    }
}

/// One SourceBlock LOD / content expression (plan §9.3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Representation {
    pub id: String,
    pub content_path: PathBuf,
    pub geometric_error_meters: f64,
    pub triangle_count: u64,
    pub texture_bytes: u64,
    /// Local to `world_transform`.
    pub bounds: BoundingVolume,
    /// Content local → world.
    pub world_transform: Mat4d,
}

/// Spatial block from tileset adapter (plan §9.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceBlock {
    pub id: String,
    pub grid_x: Option<i32>,
    pub grid_y: Option<i32>,
    /// Local to `world_transform`.
    pub bounds: BoundingVolume,
    /// Block local → world.
    pub world_transform: Mat4d,
    pub representations: Vec<Representation>,
    /// Original block `tileset.json`, if loaded from disk.
    pub source_tileset: Option<PathBuf>,
}

/// One node in the bottom-up quadtree (Phase 5: hierarchy only, no mesh merge).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: String,
    /// 0 = source / L0; higher = proxy levels.
    pub level: u32,
    pub grid_x: i32,
    pub grid_y: i32,
    /// World-space AABB.
    pub bounds: BoundingVolume,
    pub world_transform: Mat4d,
    /// Selected / aggregated representation ids contributing to this node.
    pub source_representation_ids: Vec<String>,
    /// Child tree-node ids (empty at leaves of the current build level dump).
    pub child_ids: Vec<String>,
}

/// Full pyramid produced by TreeBuilder.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RebuildTree {
    /// levels[0] = L0 source nodes, levels[1] = L1 proxies, ...
    pub levels: Vec<Vec<TreeNode>>,
}

impl RebuildTree {
    pub fn level_counts(&self) -> Vec<(u32, usize)> {
        self.levels
            .iter()
            .enumerate()
            .map(|(i, n)| (i as u32, n.len()))
            .collect()
    }
}

impl Mat4d {
    /// Invert a general 4×4 (Gauss–Jordan). Returns None if singular.
    pub fn inverse(&self) -> Option<Mat4d> {
        let mut a = self.0;
        let mut inv = Self::identity().0;
        for col in 0..4 {
            // Pivot
            let mut pivot = col;
            let mut best = a[col * 4 + col].abs();
            for r in (col + 1)..4 {
                let v = a[col * 4 + r].abs();
                if v > best {
                    best = v;
                    pivot = r;
                }
            }
            if best < 1e-15 {
                return None;
            }
            if pivot != col {
                for c in 0..4 {
                    a.swap(c * 4 + col, c * 4 + pivot);
                    inv.swap(c * 4 + col, c * 4 + pivot);
                }
            }
            let diag = a[col * 4 + col];
            for c in 0..4 {
                a[c * 4 + col] /= diag;
                inv[c * 4 + col] /= diag;
            }
            for row in 0..4 {
                if row == col {
                    continue;
                }
                let f = a[col * 4 + row];
                for c in 0..4 {
                    a[c * 4 + row] -= f * a[c * 4 + col];
                    inv[c * 4 + row] -= f * inv[c * 4 + col];
                }
            }
        }
        Some(Mat4d(inv))
    }

    /// Convert glTF column-major `[[f32;4];4]` (m[col][row]) into Mat4d.
    pub fn from_gltf_cols(m: [[f32; 4]; 4]) -> Self {
        let mut a = [0.0; 16];
        for col in 0..4 {
            for row in 0..4 {
                a[col * 4 + row] = m[col][row] as f64;
            }
        }
        Self(a)
    }

    pub fn transform_direction(&self, x: f64, y: f64, z: f64) -> (f64, f64, f64) {
        let m = &self.0;
        (
            m[0] * x + m[4] * y + m[8] * z,
            m[1] * x + m[5] * y + m[9] * z,
            m[2] * x + m[6] * y + m[10] * z,
        )
    }
}

#[cfg(test)]
mod mat4_tests {
    use super::*;

    #[test]
    fn inverse_translation() {
        let t = Mat4d::translation(10.0, -3.0, 7.5);
        let inv = t.inverse().expect("inv");
        let p = inv.transform_point(10.0, -3.0, 7.5);
        assert!((p.0).abs() < 1e-9 && p.1.abs() < 1e-9 && p.2.abs() < 1e-9);
        let id = t.mul(&inv);
        for i in 0..16 {
            let expect = if i % 5 == 0 { 1.0 } else { 0.0 };
            assert!((id.0[i] - expect).abs() < 1e-9, "i={i} got {}", id.0[i]);
        }
    }

    #[test]
    fn obb_corners_and_translated_world_aabb() {
        let local = BoundingVolume::from_box([
            0.0, 0.0, 0.0, 10.0, 0.0, 0.0, 0.0, 5.0, 0.0, 0.0, 0.0, 2.0,
        ]);
        let corners = local.corners().unwrap();
        assert!(corners.iter().any(|c| (c.0 - 10.0).abs() < 1e-12 && (c.1 - 5.0).abs() < 1e-12));
        let world = local.world_bounds(&Mat4d::translation(100.0, 0.0, 0.0));
        let c = world.center().unwrap();
        assert!((c.0 - 100.0).abs() < 1e-9 && c.1.abs() < 1e-9);
        let (amin, amax) = world.aabb_min_max().unwrap();
        assert!((amin.0 - 90.0).abs() < 1e-9);
        assert!((amax.0 - 110.0).abs() < 1e-9);
    }

    #[test]
    fn rotated_half_axes_aabb_uses_corners() {
        // axisX along (10,10,0), axisY along (-4,4,0)
        let bv = BoundingVolume::from_box([
            0.0, 0.0, 0.0, 10.0, 10.0, 0.0, -4.0, 4.0, 0.0, 0.0, 0.0, 1.0,
        ]);
        let (amin, amax) = bv.aabb_min_max().unwrap();
        assert!((amin.0 + 14.0).abs() < 1e-9);
        assert!((amax.0 - 14.0).abs() < 1e-9);
        assert!((amin.1 + 14.0).abs() < 1e-9);
        assert!((amax.1 - 14.0).abs() < 1e-9);
    }
}
