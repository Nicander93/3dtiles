//! ProxyBuilder (plan §§12–16, Phases 6–8).
//!
//! - Load child GLB/B3DM (+ textures)
//! - Expand scene transforms → parent local frame
//! - Cross-child primitive grouping (material / layout / texture hash)
//! - meshoptimizer simplify with **LockBorder** (plan §15.2 — no weld)
//! - Texture hash dedup, resize, budget enforcement, optional KTX2
//! - Gap metrics maxGap / P95Gap (plan §15.3)
//! - V1: no weld, no atlas, no remesh

use crate::b3dm::load_content;
use crate::error::{Result, TopRebuildError};
use crate::gap::{compute_gap_metrics, GapMetrics};
use crate::glb::{
    load_mesh_from_glb, transform_primitive, write_glb_with_textures, LoadedPrimitive,
};
use crate::texture::{process_textures, TextureData, TextureMetrics};
use crate::types::Mat4d;
use meshopt::{simplify_decoder, SimplifyOptions};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

/// Proxy geometry + texture budgets (plan §§14.2, 16.2).
#[derive(Clone, Debug)]
pub struct ProxyBudget {
    pub max_triangles: u64,
    pub target_error_meters: f64,
    /// Max edge length in pixels for any texture (plan maxTextureSize).
    pub max_texture_size: u32,
    /// Soft/hard total encoded texture bytes across unique textures.
    pub max_texture_bytes: u64,
    /// Soft/hard max output GLB bytes; warning (or error if strict) when exceeded.
    pub max_glb_bytes: u64,
    /// Attempt KTX2 via basisu when available.
    pub enable_ktx2: bool,
    /// Always true for V1 correctness path — meshoptimizer LockBorder.
    pub lock_border: bool,
    /// If true, over-budget GLB/texture is a hard error instead of warning.
    pub strict_budget: bool,
    /// Inject solid test textures when children have none (fixture synthesize path).
    pub inject_test_textures: bool,
}

impl Default for ProxyBudget {
    fn default() -> Self {
        Self {
            max_triangles: 50_000,
            target_error_meters: 1.0,
            max_texture_size: 1024,
            max_texture_bytes: 8 * 1024 * 1024,
            max_glb_bytes: 16 * 1024 * 1024,
            enable_ktx2: false,
            lock_border: true,
            strict_budget: false,
            inject_test_textures: false,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ChildContent {
    pub content_path: PathBuf,
    pub world_transform: Mat4d,
}

#[derive(Clone, Debug)]
pub struct ProxyBuildResult {
    pub glb_bytes: Vec<u8>,
    pub triangles_before: u64,
    pub triangles_after: u64,
    pub triangles_target: u64,
    pub simplification_error_meters: f64,
    pub warnings: Vec<String>,
    pub parent_world_transform: Mat4d,
    pub group_count: usize,
    pub gap: GapMetrics,
    pub texture: TextureMetrics,
    pub texture_bytes: u64,
    pub glb_bytes_len: u64,
    pub lock_border: bool,
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
struct GroupKey {
    material_key: String,
    has_normals: bool,
    has_uvs: bool,
    texture_hash: String,
}

pub fn build_proxy(
    children: &[ChildContent],
    parent_world_transform: &Mat4d,
    budget: &ProxyBudget,
) -> Result<ProxyBuildResult> {
    build_proxy_with_work(children, parent_world_transform, budget, None)
}

fn build_proxy_with_work(
    children: &[ChildContent],
    parent_world_transform: &Mat4d,
    budget: &ProxyBudget,
    work_dir: Option<&Path>,
) -> Result<ProxyBuildResult> {
    if children.is_empty() {
        return Err(TopRebuildError::Other("build_proxy: no children".into()));
    }
    if !budget.lock_border {
        // Documented: V1 always locks border; flag kept for honesty / future.
    }
    let parent_inv = parent_world_transform
        .inverse()
        .ok_or_else(|| TopRebuildError::Other("parent world transform not invertible".into()))?;

    let mut locals: Vec<LoadedPrimitive> = Vec::new();
    let mut all_textures: Vec<TextureData> = Vec::new();
    let mut child_positions: Vec<Vec<[f32; 3]>> = Vec::new();
    let mut child_indices: Vec<Vec<u32>> = Vec::new();
    let mut warnings = Vec::new();

    for (ci, child) in children.iter().enumerate() {
        let loaded = load_content(&child.content_path)?;
        let mut mesh = load_mesh_from_glb(&loaded.glb)?;
        if mesh.textures.is_empty() && budget.inject_test_textures {
            let (r, g, b) = (
                (40 + ci * 40) as u8,
                (80 + ci * 20) as u8,
                (120 + ci * 10) as u8,
            );
            let tex = TextureData::solid(r, g, b, 255, 64);
            for p in &mut mesh.primitives {
                if p.uvs.is_none() {
                    p.uvs = Some(
                        p.positions
                            .iter()
                            .map(|v| {
                                [
                                    (v[0] * 0.05 + 0.5).fract().abs(),
                                    (v[1] * 0.05 + 0.5).fract().abs(),
                                ]
                            })
                            .collect(),
                    );
                }
                p.texture_hash = Some(tex.hash.clone());
                p.material_key = format!(
                    "{}#tex:{}",
                    p.material_key,
                    &tex.hash[..16.min(tex.hash.len())]
                );
            }
            mesh.textures.push(tex);
        }
        all_textures.extend(mesh.textures);

        let mut child_world = child.world_transform.clone();
        if let Some([rx, ry, rz]) = loaded.rtc_center {
            child_world = child_world.mul(&Mat4d::translation(rx, ry, rz));
        }
        let to_parent = parent_inv.mul(&child_world);
        let mut cpos = Vec::new();
        let mut cidx = Vec::new();
        let mut base = 0u32;
        for p in &mut mesh.primitives {
            transform_primitive(p, &to_parent);
            cpos.extend_from_slice(&p.positions);
            cidx.extend(p.indices.iter().map(|i| i + base));
            base += p.positions.len() as u32;
            locals.push(p.clone());
        }
        child_positions.push(cpos);
        child_indices.push(cidx);
    }

    let gap = compute_gap_metrics(&child_positions, &child_indices, budget.lock_border);
    if gap.max_gap > 1.0 {
        warnings.push(format!(
            "GAP_WARN: maxGap={:.4}m P95Gap={:.4}m (calibration; not a hard fail)",
            gap.max_gap, gap.p95_gap
        ));
    }

    let triangles_before: u64 = locals.iter().map(|p| p.triangle_count() as u64).sum();

    let mut groups: BTreeMap<GroupKey, LoadedPrimitive> = BTreeMap::new();
    for p in locals {
        let key = GroupKey {
            material_key: p.material_key.clone(),
            has_normals: p.normals.is_some(),
            has_uvs: p.uvs.is_some(),
            texture_hash: p.texture_hash.clone().unwrap_or_default(),
        };
        groups
            .entry(key)
            .and_modify(|acc| append_primitive(acc, &p))
            .or_insert(p);
    }
    let group_count = groups.len();

    let mut simplified = Vec::new();
    let mut max_err: f64 = 0.0;
    let total_tris = triangles_before.max(1);

    for (_key, mut group) in groups {
        let share =
            (group.triangle_count() as f64 / total_tris as f64) * budget.max_triangles as f64;
        let target_tris = (share.round() as usize).max(4);
        let target_indices = target_tris * 3;

        let (prim, err, warn) = simplify_group(
            &mut group,
            target_indices,
            budget.target_error_meters,
            budget.lock_border,
        );
        if let Some(w) = warn {
            warnings.push(w);
        }
        max_err = max_err.max(err as f64);
        simplified.push(prim);
    }

    let triangles_after: u64 = simplified.iter().map(|p| p.triangle_count() as u64).sum();
    if triangles_after > budget.max_triangles {
        warnings.push(format!(
            "BUDGET_NOT_REACHED: after={triangles_after} max={}",
            budget.max_triangles
        ));
    }

    // Texture path: dedup / resize / KTX2
    let needed_hashes: std::collections::BTreeSet<String> = simplified
        .iter()
        .filter_map(|p| p.texture_hash.clone())
        .collect();
    let tex_inputs: Vec<TextureData> = {
        let mut map = BTreeMap::new();
        for t in &all_textures {
            if needed_hashes.contains(&t.hash) {
                map.entry(t.hash.clone()).or_insert_with(|| t.clone());
            }
        }
        // If hashes changed keys somehow, still process all collected
        if map.is_empty() && !all_textures.is_empty() {
            for t in &all_textures {
                map.entry(t.hash.clone()).or_insert_with(|| t.clone());
            }
        }
        map.into_values().collect()
    };

    let (processed, mut tex_metrics) = process_textures(
        &tex_inputs,
        budget.max_texture_size,
        budget.max_texture_bytes,
        budget.enable_ktx2,
        work_dir,
    )?;
    warnings.extend(tex_metrics.warnings.drain(..));

    // Remap primitive texture_hash (source) → processed hash after resize
    let hash_remap: BTreeMap<String, String> = processed
        .iter()
        .map(|t| (t.source_hash.clone(), t.hash.clone()))
        .collect();
    for p in &mut simplified {
        if let Some(ref h) = p.texture_hash {
            if let Some(nh) = hash_remap.get(h) {
                p.texture_hash = Some(nh.clone());
                if let Some((base, _)) = p.material_key.split_once("#tex:") {
                    p.material_key = format!("{base}#tex:{}", &nh[..16.min(nh.len())]);
                }
            }
        }
    }

    let glb_bytes = write_glb_with_textures(&simplified, &processed)?;
    let glb_len = glb_bytes.len() as u64;
    if budget.max_glb_bytes > 0 && glb_len > budget.max_glb_bytes {
        let msg = format!(
            "GLB_BYTES_OVER_BUDGET: after={glb_len} max={}",
            budget.max_glb_bytes
        );
        if budget.strict_budget {
            return Err(TopRebuildError::BudgetExceeded(msg));
        }
        warnings.push(msg);
    }
    for w in &tex_metrics.warnings {
        let _ = w;
    }
    if budget.strict_budget {
        for w in &warnings {
            if w.contains("TEXTURE_BYTES_OVER_BUDGET") {
                return Err(TopRebuildError::BudgetExceeded(w.clone()));
            }
        }
    }

    let texture_bytes = processed.iter().map(|t| t.bytes.len() as u64).sum();

    Ok(ProxyBuildResult {
        glb_bytes,
        triangles_before,
        triangles_after,
        triangles_target: budget.max_triangles,
        simplification_error_meters: max_err,
        warnings,
        parent_world_transform: parent_world_transform.clone(),
        group_count,
        gap,
        texture: TextureMetrics {
            warnings: Vec::new(),
            ..tex_metrics
        },
        texture_bytes,
        glb_bytes_len: glb_len,
        lock_border: budget.lock_border,
    })
}

pub fn build_proxy_from_paths(
    content_paths: &[PathBuf],
    child_world_transforms: &[Mat4d],
    parent_world_transform: &Mat4d,
    budget: &ProxyBudget,
) -> Result<ProxyBuildResult> {
    if content_paths.len() != child_world_transforms.len() {
        return Err(TopRebuildError::Other(
            "content_paths and transforms length mismatch".into(),
        ));
    }
    let children: Vec<ChildContent> = content_paths
        .iter()
        .zip(child_world_transforms.iter())
        .map(|(p, t)| ChildContent {
            content_path: p.clone(),
            world_transform: t.clone(),
        })
        .collect();
    build_proxy(&children, parent_world_transform, budget)
}

fn append_primitive(acc: &mut LoadedPrimitive, other: &LoadedPrimitive) {
    let base = acc.positions.len() as u32;
    acc.positions.extend_from_slice(&other.positions);
    if let (Some(an), Some(bn)) = (acc.normals.as_mut(), other.normals.as_ref()) {
        an.extend_from_slice(bn);
    }
    if let (Some(au), Some(bu)) = (acc.uvs.as_mut(), other.uvs.as_ref()) {
        au.extend_from_slice(bu);
    }
    acc.indices
        .extend(other.indices.iter().map(|i| i + base));
}

/// Simplify with LockBorder | ErrorAbsolute (plan §14–15).
/// LockBorder is the V1 border-protection strategy (no cross-tile weld).
fn simplify_group(
    group: &mut LoadedPrimitive,
    target_index_count: usize,
    target_error_meters: f64,
    lock_border: bool,
) -> (LoadedPrimitive, f32, Option<String>) {
    let index_count = group.indices.len();
    if index_count <= target_index_count || group.positions.is_empty() {
        return (group.clone(), 0.0, None);
    }

    let mut result_error = 0.0f32;
    let mut options = SimplifyOptions::ErrorAbsolute;
    if lock_border {
        options |= SimplifyOptions::LockBorder;
    }
    let new_indices = simplify_decoder(
        &group.indices,
        &group.positions,
        target_index_count,
        target_error_meters as f32,
        options,
        Some(&mut result_error),
    );

    let warn = if new_indices.len() > target_index_count + 3 {
        Some(format!(
            "BUDGET_NOT_REACHED: group tris {} -> {} (target {}) [LockBorder={}]",
            index_count / 3,
            new_indices.len() / 3,
            target_index_count / 3,
            lock_border
        ))
    } else {
        None
    };

    let compacted = compact_primitive(group, &new_indices);
    (compacted, result_error, warn)
}

fn compact_primitive(src: &LoadedPrimitive, indices: &[u32]) -> LoadedPrimitive {
    let mut remap = vec![u32::MAX; src.positions.len()];
    let mut positions = Vec::new();
    let mut normals = src.normals.as_ref().map(|_| Vec::new());
    let mut uvs = src.uvs.as_ref().map(|_| Vec::new());
    let mut out_indices = Vec::with_capacity(indices.len());

    for &i in indices {
        let i = i as usize;
        if remap[i] == u32::MAX {
            remap[i] = positions.len() as u32;
            positions.push(src.positions[i]);
            if let (Some(dn), Some(sn)) = (normals.as_mut(), src.normals.as_ref()) {
                dn.push(sn[i]);
            }
            if let (Some(du), Some(su)) = (uvs.as_mut(), src.uvs.as_ref()) {
                du.push(su[i]);
            }
        }
        out_indices.push(remap[i]);
    }

    LoadedPrimitive {
        positions,
        normals,
        uvs,
        indices: out_indices,
        material_key: src.material_key.clone(),
        texture_hash: src.texture_hash.clone(),
    }
}

pub fn to_parent_local(
    local_vertex: (f64, f64, f64),
    child_world: &Mat4d,
    parent_world: &Mat4d,
) -> (f64, f64, f64) {
    let (wx, wy, wz) = child_world.transform_point(local_vertex.0, local_vertex.1, local_vertex.2);
    let inv = parent_world.inverse().expect("invertible parent");
    inv.transform_point(wx, wy, wz)
}

pub fn build_proxy_to_file(
    children: &[ChildContent],
    parent_world_transform: &Mat4d,
    budget: &ProxyBudget,
    out_path: &Path,
) -> Result<ProxyBuildResult> {
    let work = out_path.parent();
    let result = build_proxy_with_work(children, parent_world_transform, budget, work)?;
    if let Some(parent) = out_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(out_path, &result.glb_bytes)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::b3dm::{pack_glb_as_b3dm, pack_glb_as_b3dm_with_rtc};
    use crate::glb::{load_mesh_from_glb, make_box_primitive, make_textured_box_glb, write_glb};
    use std::fs;

    fn tmp_dir() -> PathBuf {
        let d = std::env::temp_dir().join(format!(
            "top_rebuild_proxy_{}",
            std::process::id()
        ));
        let _ = fs::create_dir_all(&d);
        d
    }

    #[test]
    fn parent_local_frame_2x2() {
        let c0 = Mat4d::translation(50.0, 50.0, 10.0);
        let c1 = Mat4d::translation(150.0, 50.0, 10.0);
        let parent = Mat4d::translation(100.0, 50.0, 10.0);

        let p0 = to_parent_local((0.0, 0.0, 0.0), &c0, &parent);
        assert!((p0.0 + 50.0).abs() < 1e-9);
        assert!(p0.1.abs() < 1e-9 && p0.2.abs() < 1e-9);

        let p1 = to_parent_local((0.0, 0.0, 0.0), &c1, &parent);
        assert!((p1.0 - 50.0).abs() < 1e-9);

        let q = to_parent_local((1.0, 2.0, 3.0), &c0, &parent);
        assert!((q.0 - (-50.0 + 1.0)).abs() < 1e-9);
        assert!((q.1 - 2.0).abs() < 1e-9);
        assert!((q.2 - 3.0).abs() < 1e-9);
    }

    #[test]
    fn proxy_from_four_b3dm_boxes() {
        let dir = tmp_dir().join("four");
        fs::create_dir_all(&dir).unwrap();
        let segs = 8u32;
        let prim = make_box_primitive(20.0, 20.0, 5.0, segs, "mat0:0.800,0.800,0.800,1.000");
        let glb = write_glb(&[prim]).unwrap();
        let b3dm = pack_glb_as_b3dm(&glb).unwrap();

        let centers = [
            (50.0, 50.0, 10.0),
            (150.0, 50.0, 10.0),
            (50.0, 150.0, 10.0),
            (150.0, 150.0, 10.0),
        ];
        let mut paths = Vec::new();
        let mut xforms = Vec::new();
        for (i, (cx, cy, cz)) in centers.iter().enumerate() {
            let p = dir.join(format!("child_{i}.b3dm"));
            fs::write(&p, &b3dm).unwrap();
            paths.push(p);
            xforms.push(Mat4d::translation(*cx, *cy, *cz));
        }
        let parent = Mat4d::translation(100.0, 100.0, 10.0);
        let before_one = {
            let g = load_mesh_from_glb(&glb).unwrap();
            g.primitives[0].triangle_count() as u64
        };
        let budget = ProxyBudget {
            max_triangles: (before_one * 4 / 2).max(64),
            target_error_meters: 2.0,
            ..Default::default()
        };
        let out = dir.join("proxy.glb");
        let children: Vec<_> = paths
            .iter()
            .zip(xforms.iter())
            .map(|(p, t)| ChildContent {
                content_path: p.clone(),
                world_transform: t.clone(),
            })
            .collect();
        let result = build_proxy_to_file(&children, &parent, &budget, &out).unwrap();
        assert!(out.exists());
        assert_eq!(result.triangles_before, before_one * 4);
        assert!(result.triangles_after < result.triangles_before);
        assert!(result.lock_border);
        assert_eq!(&result.glb_bytes[0..4], b"glTF");
    }

    #[test]
    fn proxy_with_textures_dedup_and_budget() {
        let dir = tmp_dir().join("tex");
        fs::create_dir_all(&dir).unwrap();
        // Same solid color → same hash → dedup to 1
        let glb = make_textured_box_glb(10.0, 10.0, 2.0, 4, "mat0:1,1,1,1", (10, 20, 30), 128).unwrap();
        let mut children = Vec::new();
        for i in 0..4 {
            let p = dir.join(format!("c{i}.glb"));
            fs::write(&p, &glb).unwrap();
            let (cx, cy) = match i {
                0 => (0.0, 0.0),
                1 => (40.0, 0.0),
                2 => (0.0, 40.0),
                _ => (40.0, 40.0),
            };
            children.push(ChildContent {
                content_path: p,
                world_transform: Mat4d::translation(cx, cy, 0.0),
            });
        }
        let parent = Mat4d::translation(20.0, 20.0, 0.0);
        let budget = ProxyBudget {
            max_triangles: 2000,
            max_texture_size: 64,
            max_texture_bytes: 2 * 1024 * 1024,
            max_glb_bytes: 8 * 1024 * 1024,
            ..Default::default()
        };
        let out = dir.join("proxy.glb");
        let result = build_proxy_to_file(&children, &parent, &budget, &out).unwrap();
        assert!(result.texture.unique_count <= 1);
        assert!(result.texture.max_dimension_after <= 64);
        assert!(result.glb_bytes_len > 0);
        assert!(result.texture_bytes > 0);
    }

    #[test]
    fn rtc_center_applied_into_parent_local() {
        let dir = tmp_dir().join("rtc");
        fs::create_dir_all(&dir).unwrap();
        let prim = make_box_primitive(1.0, 1.0, 1.0, 2, "mat0:1,1,1,1");
        let glb = write_glb(&[prim]).unwrap();
        let b3dm = pack_glb_as_b3dm_with_rtc(&glb, Some([50.0, 0.0, 0.0])).unwrap();
        let p = dir.join("c.b3dm");
        fs::write(&p, b3dm).unwrap();
        let children = vec![ChildContent {
            content_path: p,
            world_transform: Mat4d::identity(),
        }];
        let out = dir.join("proxy.glb");
        let budget = ProxyBudget {
            max_triangles: 20_000,
            ..Default::default()
        };
        let result = build_proxy_to_file(&children, &Mat4d::identity(), &budget, &out).unwrap();
        let mesh = load_mesh_from_glb(&result.glb_bytes).unwrap();
        let mean_x: f32 = {
            let xs: Vec<f32> = mesh.primitives[0].positions.iter().map(|v| v[0]).collect();
            xs.iter().sum::<f32>() / xs.len() as f32
        };
        assert!(
            (mean_x - 50.0).abs() < 2.0,
            "expected RTC shift ~50, got mean_x={mean_x}"
        );
    }
}
