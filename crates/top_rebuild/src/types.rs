//! Core SourceBlock / Representation / math types (plan §§9–11).

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

/// Cesium-style `boundingVolume.box`: center(3) + halfAxes(9).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundingVolume {
    /// If present: [cx,cy,cz, hx,0,0, 0,hy,0, 0,0,hz] (or general half-axes).
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

    /// AABB extents assuming (near-)diagonal half-axes, common in converter output.
    pub fn aabb_min_max(&self) -> Option<((f64, f64, f64), (f64, f64, f64))> {
        let b = self.box_values?;
        let (cx, cy, cz) = (b[0], b[1], b[2]);
        let hx = b[3].abs().max(b[4].abs()).max(b[5].abs());
        let hy = b[6].abs().max(b[7].abs()).max(b[8].abs());
        let hz = b[9].abs().max(b[10].abs()).max(b[11].abs());
        // Prefer classic diagonal layout half-axes when present
        let hx = if b[4].abs() < 1e-12 && b[5].abs() < 1e-12 {
            b[3].abs()
        } else {
            hx
        };
        let hy = if b[6].abs() < 1e-12 && b[8].abs() < 1e-12 {
            b[7].abs()
        } else {
            hy
        };
        let hz = if b[9].abs() < 1e-12 && b[10].abs() < 1e-12 {
            b[11].abs()
        } else {
            hz
        };
        Some(((cx - hx, cy - hy, cz - hz), (cx + hx, cy + hy, cz + hz)))
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
                let cx = (min_x + max_x) * 0.5;
                let cy = (min_y + max_y) * 0.5;
                let cz = (min_z + max_z) * 0.5;
                let hx = (max_x - min_x) * 0.5;
                let hy = (max_y - min_y) * 0.5;
                let hz = (max_z - min_z) * 0.5;
                BoundingVolume::from_box([cx, cy, cz, hx, 0.0, 0.0, 0.0, hy, 0.0, 0.0, 0.0, hz])
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
    pub bounds: BoundingVolume,
    pub world_transform: Mat4d,
}

/// Spatial block from tileset adapter (plan §9.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceBlock {
    pub id: String,
    pub grid_x: Option<i32>,
    pub grid_y: Option<i32>,
    pub bounds: BoundingVolume,
    pub world_transform: Mat4d,
    pub representations: Vec<Representation>,
}

/// One node in the bottom-up quadtree (Phase 5: hierarchy only, no mesh merge).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct TreeNode {
    pub id: String,
    /// 0 = source / L0; higher = proxy levels.
    pub level: u32,
    pub grid_x: i32,
    pub grid_y: i32,
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
}
