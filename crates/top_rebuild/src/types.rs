//! Core SourceBlock / Representation / math types (plan §§9–11, Phase 11 P0-3/P0-4).

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

    /// Rotation about Z (radians), column-major.
    pub fn rotation_z(radians: f64) -> Self {
        let (s, c) = radians.sin_cos();
        Self([
            c, s, 0.0, 0.0, // col0
            -s, c, 0.0, 0.0, // col1
            0.0, 0.0, 1.0, 0.0, // col2
            0.0, 0.0, 0.0, 1.0, // col3
        ])
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

    /// Invert a general 4×4 (Gauss–Jordan). Returns None if singular.
    pub fn inverse(&self) -> Option<Mat4d> {
        let mut a = self.0;
        let mut inv = Self::identity().0;
        for col in 0..4 {
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

    pub fn is_finite(&self) -> bool {
        self.0.iter().all(|v| v.is_finite())
    }

    /// Max absolute element difference vs another matrix.
    pub fn max_abs_diff(&self, other: &Mat4d) -> f64 {
        self.0
            .iter()
            .zip(other.0.iter())
            .map(|(a, b)| (a - b).abs())
            .fold(0.0_f64, f64::max)
    }
}

/// Axis-aligned bounds in world (or any) space.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Aabb3d {
    pub min: [f64; 3],
    pub max: [f64; 3],
}

impl Aabb3d {
    pub fn empty() -> Self {
        Self {
            min: [f64::MAX, f64::MAX, f64::MAX],
            max: [f64::MIN, f64::MIN, f64::MIN],
        }
    }

    pub fn is_valid(&self) -> bool {
        self.min[0] <= self.max[0] && self.min[1] <= self.max[1] && self.min[2] <= self.max[2]
    }

    pub fn include_point(&mut self, x: f64, y: f64, z: f64) {
        self.min[0] = self.min[0].min(x);
        self.min[1] = self.min[1].min(y);
        self.min[2] = self.min[2].min(z);
        self.max[0] = self.max[0].max(x);
        self.max[1] = self.max[1].max(y);
        self.max[2] = self.max[2].max(z);
    }

    pub fn union(a: &Aabb3d, b: &Aabb3d) -> Aabb3d {
        if !a.is_valid() {
            return b.clone();
        }
        if !b.is_valid() {
            return a.clone();
        }
        Aabb3d {
            min: [
                a.min[0].min(b.min[0]),
                a.min[1].min(b.min[1]),
                a.min[2].min(b.min[2]),
            ],
            max: [
                a.max[0].max(b.max[0]),
                a.max[1].max(b.max[1]),
                a.max[2].max(b.max[2]),
            ],
        }
    }

    pub fn to_box_bv(&self) -> BoundingVolume {
        let cx = (self.min[0] + self.max[0]) * 0.5;
        let cy = (self.min[1] + self.max[1]) * 0.5;
        let cz = (self.min[2] + self.max[2]) * 0.5;
        let hx = (self.max[0] - self.min[0]) * 0.5;
        let hy = (self.max[1] - self.min[1]) * 0.5;
        let hz = (self.max[2] - self.min[2]) * 0.5;
        BoundingVolume::from_box([cx, cy, cz, hx, 0.0, 0.0, 0.0, hy, 0.0, 0.0, 0.0, hz])
    }
}

/// Local BV + conservative world AABB (Phase 11 / P0-4).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SpatialBounds {
    pub local: BoundingVolume,
    pub world_aabb: Aabb3d,
}

impl SpatialBounds {
    pub fn from_local_and_world_transform(local: BoundingVolume, world: &Mat4d) -> Self {
        let world_aabb = local
            .world_aabb(world)
            .unwrap_or_else(Aabb3d::empty);
        Self { local, world_aabb }
    }
}

/// Cesium-style `boundingVolume.box`: center(3) + halfAxes(9).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BoundingVolume {
    /// [cx,cy,cz, axx,axy,axz, ayx,ayy,ayz, azx,azy,azz] — half-axis vectors (may be oriented).
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

    /// Eight corners of the (possibly oriented) box in local space.
    pub fn local_corners(&self) -> Option<[(f64, f64, f64); 8]> {
        let b = self.box_values?;
        let (cx, cy, cz) = (b[0], b[1], b[2]);
        let ax = (b[3], b[4], b[5]);
        let ay = (b[6], b[7], b[8]);
        let az = (b[9], b[10], b[11]);
        let mut out = [(0.0, 0.0, 0.0); 8];
        let mut i = 0;
        for sx in [-1.0, 1.0] {
            for sy in [-1.0, 1.0] {
                for sz in [-1.0, 1.0] {
                    out[i] = (
                        cx + sx * ax.0 + sy * ay.0 + sz * az.0,
                        cy + sx * ax.1 + sy * ay.1 + sz * az.1,
                        cz + sx * ax.2 + sy * ay.2 + sz * az.2,
                    );
                    i += 1;
                }
            }
        }
        Some(out)
    }

    /// Conservative world-space AABB of this local box under `world` transform.
    pub fn world_aabb(&self, world: &Mat4d) -> Option<Aabb3d> {
        let corners = self.local_corners()?;
        let mut aabb = Aabb3d::empty();
        for (x, y, z) in corners {
            let p = world.transform_point(x, y, z);
            if !p.0.is_finite() || !p.1.is_finite() || !p.2.is_finite() {
                return None;
            }
            aabb.include_point(p.0, p.1, p.2);
        }
        Some(aabb)
    }

    /// AABB extents from oriented (or axis-aligned) half-axes — never assumes hx=b[3], hy=b[7], hz=b[11] alone.
    pub fn aabb_min_max(&self) -> Option<((f64, f64, f64), (f64, f64, f64))> {
        let corners = self.local_corners()?;
        let mut min_x = f64::MAX;
        let mut min_y = f64::MAX;
        let mut min_z = f64::MAX;
        let mut max_x = f64::MIN;
        let mut max_y = f64::MIN;
        let mut max_z = f64::MIN;
        for (x, y, z) in corners {
            min_x = min_x.min(x);
            min_y = min_y.min(y);
            min_z = min_z.min(z);
            max_x = max_x.max(x);
            max_y = max_y.max(y);
            max_z = max_z.max(z);
        }
        Some(((min_x, min_y, min_z), (max_x, max_y, max_z)))
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
                Aabb3d {
                    min: [min_x, min_y, min_z],
                    max: [max_x, max_y, max_z],
                }
                .to_box_bv()
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

/// One content part of a coverage frontier (Phase 11 / P0-3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct RepresentationPart {
    pub content_path: PathBuf,
    pub world_transform: Mat4d,
    pub bounds: BoundingVolume,
}

/// Complete coverage frontier at one geometric-error band (plan §9.3 / P0-3).
///
/// `parts` is a non-overlapping set that together cover the SourceBlock.
/// Never treat a multi-tile block as a single content file when coverage needs several parts.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Representation {
    pub id: String,
    pub geometric_error_meters: f64,
    pub parts: Vec<RepresentationPart>,
    pub triangle_count: u64,
    pub texture_bytes: u64,
    pub bounds: BoundingVolume,
}

impl Representation {
    /// Convenience for single-content representations (chain LOD / fixtures).
    pub fn single_part(
        id: impl Into<String>,
        content_path: PathBuf,
        geometric_error_meters: f64,
        bounds: BoundingVolume,
        world_transform: Mat4d,
    ) -> Self {
        let part = RepresentationPart {
            content_path,
            world_transform,
            bounds: bounds.clone(),
        };
        Self {
            id: id.into(),
            geometric_error_meters,
            parts: vec![part],
            triangle_count: 0,
            texture_bytes: 0,
            bounds,
        }
    }

    pub fn primary_content_path(&self) -> Option<&PathBuf> {
        self.parts.first().map(|p| &p.content_path)
    }

    pub fn primary_world_transform(&self) -> Mat4d {
        self.parts
            .first()
            .map(|p| p.world_transform.clone())
            .unwrap_or_default()
    }

    pub fn frontier_parts(&self) -> usize {
        self.parts.len()
    }
}

/// Spatial block from tileset adapter (plan §9.2).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SourceBlock {
    pub id: String,
    pub grid_x: Option<i32>,
    pub grid_y: Option<i32>,
    pub bounds: BoundingVolume,
    pub world_transform: Mat4d,
    /// Absolute path to the original Block external tileset.json (P0-2).
    pub source_tileset_path: PathBuf,
    /// Directory containing the Block external tileset and its content.
    pub source_block_dir: PathBuf,
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
    fn oriented_box_aabb_not_axis_assumption() {
        // 45° rotated box in XY: half-axes are not diagonal layout.
        let s = std::f64::consts::FRAC_1_SQRT_2 * 10.0;
        let bv = BoundingVolume::from_box([
            0.0, 0.0, 0.0, // center
            s, s, 0.0, // half X (rotated)
            -s, s, 0.0, // half Y
            0.0, 0.0, 5.0, // half Z
        ]);
        let ((min_x, min_y, min_z), (max_x, max_y, max_z)) = bv.aabb_min_max().unwrap();
        // Extent along X/Y should be ~20 (full diagonal of 10√2 * 2 axes)
        assert!((max_x - min_x) > 19.0, "dx={}", max_x - min_x);
        assert!((max_y - min_y) > 19.0, "dy={}", max_y - min_y);
        assert!((max_z - min_z - 10.0).abs() < 1e-9);
        // Must not wrongly use b[3], b[7], b[11] as axis lengths alone
        let naive_hx = 10.0 * std::f64::consts::FRAC_1_SQRT_2;
        assert!((max_x - min_x) > naive_hx * 2.0 + 1.0);
    }

    #[test]
    fn world_aabb_under_rotation_translation() {
        let local = BoundingVolume::from_box([
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 2.0, 0.0, 0.0, 0.0, 3.0,
        ]);
        let world = Mat4d::translation(100.0, 50.0, 0.0).mul(&Mat4d::rotation_z(
            std::f64::consts::FRAC_PI_2,
        ));
        let aabb = local.world_aabb(&world).unwrap();
        // Local (±1,±2,±3) after rotZ 90°: (x,y)->(-y,x) then + (100,50,0)
        assert!((aabb.min[0] - (100.0 - 2.0)).abs() < 1e-9);
        assert!((aabb.max[0] - (100.0 + 2.0)).abs() < 1e-9);
        assert!((aabb.min[1] - (50.0 - 1.0)).abs() < 1e-9);
        assert!((aabb.max[1] - (50.0 + 1.0)).abs() < 1e-9);
        assert!((aabb.min[2] + 3.0).abs() < 1e-9);
        assert!((aabb.max[2] - 3.0).abs() < 1e-9);
    }

    #[test]
    fn nested_transform_invariant_1e8() {
        let a = Mat4d::translation(1e6, 2e6, 100.0).mul(&Mat4d::rotation_z(0.3));
        let b = Mat4d::translation(10.0, -5.0, 1.0);
        let child_world = a.mul(&b);
        let inv = a.inverse().unwrap();
        let local = inv.mul(&child_world);
        let composed = a.mul(&local);
        assert!(
            composed.max_abs_diff(&child_world) <= 1e-8,
            "err={}",
            composed.max_abs_diff(&child_world)
        );
        assert!(composed.is_finite());
    }

    #[test]
    fn negative_grid_parent_floor_via_translation() {
        // Spot-check ECEF-scale translation stays finite under inverse.
        let t = Mat4d::translation(-2_500_000.0, 4_800_000.0, 3_400_000.0);
        let inv = t.inverse().unwrap();
        let p = inv.transform_point(-2_500_000.0, 4_800_000.0, 3_400_000.0);
        assert!(p.0.abs() < 1e-6 && p.1.abs() < 1e-6 && p.2.abs() < 1e-6);
    }
}
