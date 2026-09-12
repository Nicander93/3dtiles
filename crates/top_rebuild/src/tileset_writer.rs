//! TilesetWriter + HLOD rebuild pipeline (plan §§17–19, Phase 7).
//!
//! - Proxy content as B3DM (3D Tiles 1.0) or GLB
//! - boundingVolume + geometricError (plan §17.3)
//! - refine: REPLACE
//! - Attach original child subtrees; world transform invariant
//! - Full 4×4: 16 → 4 → 1

use crate::adapter::load_source_blocks;
use crate::b3dm::pack_glb_as_b3dm;
use crate::error::{Result, TopRebuildError};
use crate::gap::GapMetrics;
use crate::glb::{make_box_primitive, make_textured_box_glb, transform_primitive, write_glb};
use crate::proxy_builder::{build_proxy_to_file, ChildContent, ProxyBudget, ProxyBuildResult};
use crate::texture::TextureMetrics;
use crate::selector::{self, Selection, DEFAULT_SOURCE_ERROR_RATIO};
use crate::tree_builder::{build_tree, TreeBuildOptions};
use crate::types::{BoundingVolume, Mat4d, SourceBlock, TreeNode};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

/// Writer / rebuild options (Phases 7–8).
#[derive(Clone, Debug)]
pub struct WriteOptions {
    /// Pack proxy GLB as B3DM (3D Tiles 1.0). If false, write `.glb`.
    pub pack_as_b3dm: bool,
    pub l1_max_triangles: u64,
    pub l2_max_triangles: u64,
    pub target_error_meters: f64,
    /// When selected content is empty / unreadable, synthesize a box mesh.
    pub synthesize_if_empty: bool,
    pub box_segments: u32,
    pub source_error_ratio: f64,
    /// Plan §16.2 budgets
    pub max_texture_size: u32,
    pub max_texture_bytes: u64,
    pub max_glb_bytes: u64,
    pub enable_ktx2: bool,
    pub lock_border: bool,
    pub strict_budget: bool,
    /// When synthesizing empty leaves, embed a solid test texture (Phase 8 path).
    pub inject_test_textures: bool,
    /// Gap warning threshold in meters (plan §15.3 calibration).
    pub gap_warn_meters: f64,
}

impl Default for WriteOptions {
    fn default() -> Self {
        Self {
            pack_as_b3dm: true,
            l1_max_triangles: 4_000,
            l2_max_triangles: 2_000,
            target_error_meters: 2.0,
            synthesize_if_empty: false,
            box_segments: 6,
            source_error_ratio: DEFAULT_SOURCE_ERROR_RATIO,
            max_texture_size: 1024,
            max_texture_bytes: 8 * 1024 * 1024,
            max_glb_bytes: 16 * 1024 * 1024,
            enable_ktx2: false,
            lock_border: true,
            strict_budget: false,
            inject_test_textures: false,
            gap_warn_meters: 1.0,
        }
    }
}

#[derive(Clone, Debug)]
pub struct ProxyWriteMetrics {
    pub node_id: String,
    pub level: u32,
    pub triangles_before: u64,
    pub triangles_after: u64,
    pub triangles_target: u64,
    pub simplification_error_meters: f64,
    pub geometric_error: f64,
    pub content_uri: String,
    pub texture_bytes: u64,
    pub glb_bytes: u64,
    pub max_gap: f64,
    pub p95_gap: f64,
}

#[derive(Clone, Debug)]
pub struct RebuildReport {
    pub level_counts: Vec<(u32, usize)>,
    pub proxies: Vec<ProxyWriteMetrics>,
    pub tileset_path: PathBuf,
    pub metrics_path: PathBuf,
    pub warnings: Vec<String>,
    pub gap: GapMetrics,
    pub texture: TextureMetrics,
    pub total_triangles_after: u64,
    pub total_texture_bytes: u64,
    pub total_glb_bytes: u64,
}

#[derive(Clone)]
struct NodePayload {
    content_path: PathBuf,
    content_uri: String,
    world_transform: Mat4d,
    geometric_error: f64,
    bounds: BoundingVolume,
    node: TreeNode,
    /// For L0: external tileset uri. For proxies: None (inline content).
    external_tileset_uri: Option<String>,
}

/// Space diagonal of the OBB (twice the root-sum-square of half-axis lengths).
pub fn box_diagonal(bv: &BoundingVolume) -> f64 {
    let Some(b) = bv.box_values else {
        return 0.0;
    };
    let hx = (b[3] * b[3] + b[4] * b[4] + b[5] * b[5]).sqrt();
    let hy = (b[6] * b[6] + b[7] * b[7] + b[8] * b[8]).sqrt();
    let hz = (b[9] * b[9] + b[10] * b[10] + b[11] * b[11]).sqrt();
    2.0 * (hx * hx + hy * hy + hz * hz).sqrt()
}

/// Plan §17.3: `proxyError = max(childSourceError) + simplificationError`,
/// with strict parent > child and upward monotonicity.
/// A mild diagonal floor assists when simplification error is ~0
/// (not the Python 0.25 smoke heuristic alone).
pub fn geometric_error_proxy(
    child_errors: &[f64],
    simplification_error: f64,
    bounds: &BoundingVolume,
) -> f64 {
    let max_child = child_errors.iter().copied().fold(0.0_f64, f64::max);
    let from_simp = max_child + simplification_error.max(0.0);
    let diag = box_diagonal(bounds);
    let from_diag = if diag > 0.0 { diag * 0.05 } else { 0.0 };
    let floor = (max_child * 1.01 + 1e-3).max(max_child + 1.0);
    let ge = from_simp.max(floor).max(from_diag);
    if ge <= max_child {
        max_child + 1.0
    } else {
        ge
    }
}

/// `newChildLocal = inv(parentWorld) * childWorld`
/// so `parentWorld * newChildLocal == childWorld`.
pub fn relative_transform(parent_world: &Mat4d, child_world: &Mat4d) -> Result<Mat4d> {
    let inv = parent_world
        .inverse()
        .ok_or_else(|| TopRebuildError::Other("parent world transform not invertible".into()))?;
    Ok(inv.mul(child_world))
}

/// Express a world-space AABB box in the local frame of `world_transform`.
pub fn bv_world_to_local(bv: &BoundingVolume, world_transform: &Mat4d) -> BoundingVolume {
    let Some(((min_x, min_y, min_z), (max_x, max_y, max_z))) = bv.aabb_min_max() else {
        return BoundingVolume::empty();
    };
    let inv = match world_transform.inverse() {
        Some(m) => m,
        None => return bv.clone(),
    };
    let corners = [
        (min_x, min_y, min_z),
        (max_x, min_y, min_z),
        (min_x, max_y, min_z),
        (max_x, max_y, min_z),
        (min_x, min_y, max_z),
        (max_x, min_y, max_z),
        (min_x, max_y, max_z),
        (max_x, max_y, max_z),
    ];
    let mut amin = (f64::MAX, f64::MAX, f64::MAX);
    let mut amax = (f64::MIN, f64::MIN, f64::MIN);
    for (x, y, z) in corners {
        let p = inv.transform_point(x, y, z);
        amin.0 = amin.0.min(p.0);
        amin.1 = amin.1.min(p.1);
        amin.2 = amin.2.min(p.2);
        amax.0 = amax.0.max(p.0);
        amax.1 = amax.1.max(p.1);
        amax.2 = amax.2.max(p.2);
    }
    let cx = (amin.0 + amax.0) * 0.5;
    let cy = (amin.1 + amax.1) * 0.5;
    let cz = (amin.2 + amax.2) * 0.5;
    let hx = (amax.0 - amin.0) * 0.5;
    let hy = (amax.1 - amin.1) * 0.5;
    let hz = (amax.2 - amin.2) * 0.5;
    BoundingVolume::from_box([cx, cy, cz, hx, 0.0, 0.0, 0.0, hy, 0.0, 0.0, 0.0, hz])
}

fn mat4_to_json(m: &Mat4d) -> Value {
    json!(m.0.to_vec())
}

fn bv_to_json(bv: &BoundingVolume) -> Value {
    match bv.box_values {
        Some(b) => json!({ "box": b.to_vec() }),
        None => json!({}),
    }
}

fn synthesize_box_glb_at(
    cx: f64,
    cy: f64,
    cz: f64,
    hx: f32,
    hy: f32,
    hz: f32,
    segments: u32,
    material_key: &str,
) -> Result<Vec<u8>> {
    let mut prim = make_box_primitive(hx, hy, hz, segments, material_key);
    let place = Mat4d::translation(cx, cy, cz);
    transform_primitive(&mut prim, &place);
    write_glb(&[prim])
}

fn copy_dir_all(src: &Path, dst: &Path) -> Result<()> {
    fs::create_dir_all(dst)?;
    for entry in fs::read_dir(src)? {
        let entry = entry?;
        let from = entry.path();
        let to = dst.join(entry.file_name());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&from, &to)?;
        } else {
            fs::copy(&from, &to)?;
        }
    }
    Ok(())
}

fn content_rel(block: &SourceBlock, content_path: &Path) -> PathBuf {
    if let Some(ts) = &block.source_tileset {
        if let Some(dir) = ts.parent() {
            if let Ok(rel) = content_path.strip_prefix(dir) {
                return rel.to_path_buf();
            }
        }
    }
    content_path
        .file_name()
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from(format!("{}.b3dm", block.id)))
}

fn probe_mesh_content(path: &Path) -> Result<()> {
    if !path.exists() {
        return Err(TopRebuildError::ContentMissing(path.display().to_string()));
    }
    let len = fs::metadata(path)?.len();
    if len == 0 {
        return Err(TopRebuildError::ContentMissing(path.display().to_string()));
    }
    let loaded = crate::b3dm::load_content(path).map_err(|e| {
        TopRebuildError::ContentUnreadable(format!("{}: {e}", path.display()))
    })?;
    crate::glb::load_mesh_from_glb(&loaded.glb).map_err(|e| {
        let msg = e.to_string();
        if msg.contains("unsupported") {
            TopRebuildError::UnsupportedContent(format!("{}: {msg}", path.display()))
        } else {
            TopRebuildError::ContentUnreadable(format!("{}: {msg}", path.display()))
        }
    })?;
    Ok(())
}

fn write_synthesized_content(
    dest: &Path,
    block: &SourceBlock,
    i: usize,
    opts: &WriteOptions,
) -> Result<()> {
    if let Some(parent) = dest.parent() {
        fs::create_dir_all(parent)?;
    }
    let (cx, cy, cz) = block.bounds.center().unwrap_or((0.0, 0.0, 0.0));
    let ((min_x, min_y, min_z), (max_x, max_y, max_z)) = block
        .bounds
        .aabb_min_max()
        .unwrap_or(((-40.0, -40.0, 0.0), (40.0, 40.0, 20.0)));
    let hx = ((max_x - min_x) * 0.4).max(5.0) as f32;
    let hy = ((max_y - min_y) * 0.4).max(5.0) as f32;
    let hz = ((max_z - min_z) * 0.4).max(2.0) as f32;
    let segs = if i == 0 {
        opts.box_segments
    } else {
        opts.box_segments.saturating_add(2)
    };
    let scale = if i == 0 { 1.0 } else { 0.85 };
    let glb = if opts.inject_test_textures {
        let rgb = (
            (40 + (block.grid_x.unwrap_or(0) as u8).wrapping_mul(30)),
            (80 + (block.grid_y.unwrap_or(0) as u8).wrapping_mul(20)),
            160u8,
        );
        let local = make_textured_box_glb(
            hx * scale,
            hy * scale,
            hz * scale,
            segs,
            &format!("leaf:{}", block.id),
            rgb,
            64,
        )?;
        let mesh = crate::glb::load_mesh_from_glb(&local)?;
        let mut prims = mesh.primitives;
        let place = Mat4d::translation(cx, cy, cz);
        for p in &mut prims {
            transform_primitive(p, &place);
        }
        let (proc, _) = crate::texture::process_textures(&mesh.textures, 64, 0, false, None)?;
        crate::glb::write_glb_with_textures(&prims, &proc)?
    } else {
        synthesize_box_glb_at(
            cx,
            cy,
            cz,
            hx * scale,
            hy * scale,
            hz * scale,
            segs,
            &format!("leaf:{}", block.id),
        )?
    };
    let bytes = if dest
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.eq_ignore_ascii_case("glb"))
        .unwrap_or(false)
    {
        glb
    } else {
        pack_glb_as_b3dm(&glb)?
    };
    fs::write(dest, bytes)?;
    Ok(())
}

fn ensure_leaf_content(
    block: &SourceBlock,
    sel: &Selection,
    out_block_dir: &Path,
    opts: &WriteOptions,
) -> Result<(PathBuf, f64)> {
    if let Some(ts) = &block.source_tileset {
        let src_dir = ts.parent().ok_or_else(|| {
            TopRebuildError::InvalidTileset(format!("block tileset has no parent: {}", ts.display()))
        })?;
        copy_dir_all(src_dir, out_block_dir)?;
        if !out_block_dir.join("tileset.json").is_file() {
            return Err(TopRebuildError::InvalidTileset(format!(
                "copied block {} missing tileset.json",
                block.id
            )));
        }
    } else {
        fs::create_dir_all(out_block_dir)?;
        return Err(TopRebuildError::InvalidTileset(format!(
            "block {} has no original tileset to preserve",
            block.id
        )));
    }

    for (i, r) in block.representations.iter().enumerate() {
        let dest = out_block_dir.join(content_rel(block, &r.content_path));
        match probe_mesh_content(&r.content_path) {
            Ok(()) => {
                if !dest.is_file() {
                    if let Some(parent) = dest.parent() {
                        fs::create_dir_all(parent)?;
                    }
                    fs::copy(&r.content_path, &dest)?;
                }
            }
            Err(_) if opts.synthesize_if_empty => {
                write_synthesized_content(&dest, block, i, opts)?;
            }
            Err(e) => return Err(e),
        }
    }

    let sel_rep = &block.representations[sel.representation_index];
    let sel_path = out_block_dir.join(content_rel(block, &sel_rep.content_path));
    Ok((sel_path, sel.source_error))
}

fn write_proxy_bytes(
    out_dir: &Path,
    node: &TreeNode,
    glb_bytes: &[u8],
    pack_as_b3dm: bool,
) -> Result<(PathBuf, String)> {
    let dir = out_dir.join("Data").join(&node.id);
    fs::create_dir_all(&dir)?;
    let (name, bytes) = if pack_as_b3dm {
        (format!("{}.b3dm", node.id), pack_glb_as_b3dm(glb_bytes)?)
    } else {
        (format!("{}.glb", node.id), glb_bytes.to_vec())
    };
    let path = dir.join(&name);
    fs::write(&path, &bytes)?;
    let uri = format!("./Data/{}/{}", node.id, name);
    Ok((path, uri))
}

fn make_tile_node(
    payload: &NodePayload,
    parent_world: &Mat4d,
    children_json: Vec<Value>,
    is_root: bool,
) -> Result<Value> {
    let local_x = if is_root {
        payload.world_transform.clone()
    } else {
        relative_transform(parent_world, &payload.world_transform)?
    };
    let local_bv = bv_world_to_local(&payload.bounds, &payload.world_transform);

    let mut node = json!({
        "boundingVolume": bv_to_json(&local_bv),
        "geometricError": payload.geometric_error,
        "refine": "REPLACE",
        "transform": mat4_to_json(&local_x),
    });

    if let Some(ext) = &payload.external_tileset_uri {
        node["content"] = json!({ "uri": ext });
        // Leaves keep external subtree; do not nest synthetic children here.
    } else {
        node["content"] = json!({ "uri": payload.content_uri });
        if !children_json.is_empty() {
            node["children"] = Value::Array(children_json);
        }
    }
    Ok(node)
}

fn emit_node(
    node_id: &str,
    parent_world: &Mat4d,
    payloads: &HashMap<String, NodePayload>,
    is_root: bool,
) -> Result<Value> {
    let payload = payloads
        .get(node_id)
        .ok_or_else(|| TopRebuildError::Other(format!("missing payload {node_id}")))?;

    let mut child_json = Vec::new();
    if payload.external_tileset_uri.is_none() {
        for cid in &payload.node.child_ids {
            child_json.push(emit_node(cid, &payload.world_transform, payloads, false)?);
        }
    }
    make_tile_node(payload, parent_world, child_json, is_root)
}

/// Full HLOD rebuild: load → tree → proxies → tileset.json.
pub fn rebuild_tileset(
    input: &Path,
    output: &Path,
    tree_opts: &TreeBuildOptions,
    write_opts: &WriteOptions,
) -> Result<RebuildReport> {
    let blocks = load_source_blocks(input)?;
    let mut tree_opts = tree_opts.clone();
    tree_opts.source_error_ratio = write_opts.source_error_ratio;
    let tree = build_tree(&blocks, &tree_opts)?;
    if tree.levels.is_empty() {
        return Err(TopRebuildError::Other("empty rebuild tree".into()));
    }

    if output.exists() {
        let _ = fs::remove_dir_all(output);
    }
    fs::create_dir_all(output.join("Data"))?;

    let target = tree_opts
        .target_proxy_error
        .unwrap_or_else(|| selector::default_target_proxy_error(&blocks));
    let selections =
        selector::select_for_blocks(&blocks, target, tree_opts.source_error_ratio)?;
    let sel_by_block: HashMap<String, Selection> = selections
        .iter()
        .map(|(_, s)| (s.source_block_id.clone(), s.clone()))
        .collect();

    let mut payloads: HashMap<String, NodePayload> = HashMap::new();
    let mut warnings = Vec::new();
    for (_, s) in &selections {
        if let Some(w) = &s.warning {
            warnings.push(w.clone());
        }
    }

    for node in &tree.levels[0] {
        let block_id = node.id.strip_prefix("L0_").unwrap_or(&node.id);
        let block = blocks
            .iter()
            .find(|b| b.id == block_id)
            .ok_or_else(|| TopRebuildError::Other(format!("missing block {block_id}")))?;
        let sel = sel_by_block.get(&block.id).ok_or_else(|| {
            TopRebuildError::Other(format!("missing selection for {}", block.id))
        })?;
        let out_block = output.join("Data").join(&block.id);
        let (content_path, ge) = ensure_leaf_content(block, sel, &out_block, write_opts)?;
        let external_uri = format!("./Data/{}/tileset.json", block.id);
        payloads.insert(
            node.id.clone(),
            NodePayload {
                content_path,
                content_uri: external_uri.clone(),
                world_transform: block.representations[sel.representation_index]
                    .world_transform
                    .clone(),
                geometric_error: ge,
                bounds: node.bounds.clone(),
                node: node.clone(),
                external_tileset_uri: Some(external_uri),
            },
        );
    }

    let mut proxy_metrics = Vec::new();
    let mut agg_gap = GapMetrics {
        lock_border: write_opts.lock_border,
        ..Default::default()
    };
    let mut agg_texture = TextureMetrics::default();

    for level_idx in 1..tree.levels.len() {
        let max_tris = if level_idx == 1 {
            write_opts.l1_max_triangles
        } else {
            write_opts.l2_max_triangles
        };
        let budget = ProxyBudget {
            max_triangles: max_tris,
            target_error_meters: write_opts.target_error_meters,
            max_texture_size: write_opts.max_texture_size,
            max_texture_bytes: write_opts.max_texture_bytes,
            max_glb_bytes: write_opts.max_glb_bytes,
            enable_ktx2: write_opts.enable_ktx2,
            lock_border: write_opts.lock_border,
            strict_budget: write_opts.strict_budget,
            inject_test_textures: write_opts.inject_test_textures,
        };

        for node in &tree.levels[level_idx] {
            let mut children = Vec::new();
            let mut child_ges = Vec::new();
            for cid in &node.child_ids {
                let p = payloads.get(cid).ok_or_else(|| {
                    TopRebuildError::Other(format!("missing payload for child {cid}"))
                })?;
                children.push(ChildContent {
                    content_path: p.content_path.clone(),
                    world_transform: p.world_transform.clone(),
                });
                child_ges.push(p.geometric_error);
            }

            let tmp_glb = output
                .join("Data")
                .join(&node.id)
                .join(format!("{}_tmp.glb", node.id));
            if let Some(parent) = tmp_glb.parent() {
                fs::create_dir_all(parent)?;
            }
            let built: ProxyBuildResult =
                build_proxy_to_file(&children, &node.world_transform, &budget, &tmp_glb)?;
            for w in &built.warnings {
                warnings.push(format!("{}: {w}", node.id));
            }
            let ge = geometric_error_proxy(
                &child_ges,
                built.simplification_error_meters,
                &node.bounds,
            );
            let (content_path, uri) =
                write_proxy_bytes(output, node, &built.glb_bytes, write_opts.pack_as_b3dm)?;
            let _ = fs::remove_file(&tmp_glb);

            if built.gap.max_gap > write_opts.gap_warn_meters {
                warnings.push(format!(
                    "{}: GAP_WARN maxGap={:.4} P95Gap={:.4} threshold={}",
                    node.id, built.gap.max_gap, built.gap.p95_gap, write_opts.gap_warn_meters
                ));
            }
            proxy_metrics.push(ProxyWriteMetrics {
                node_id: node.id.clone(),
                level: node.level,
                triangles_before: built.triangles_before,
                triangles_after: built.triangles_after,
                triangles_target: built.triangles_target,
                simplification_error_meters: built.simplification_error_meters,
                geometric_error: ge,
                content_uri: uri.clone(),
                texture_bytes: built.texture_bytes,
                glb_bytes: built.glb_bytes_len,
                max_gap: built.gap.max_gap,
                p95_gap: built.gap.p95_gap,
            });
            // Accumulate texture metrics (last wins for ktx2 flags; sums for bytes)
            agg_texture.input_count += built.texture.input_count;
            agg_texture.unique_count += built.texture.unique_count;
            agg_texture.dedup_removed += built.texture.dedup_removed;
            agg_texture.total_bytes_before += built.texture.total_bytes_before;
            agg_texture.total_bytes_after += built.texture.total_bytes_after;
            agg_texture.max_dimension_after = agg_texture
                .max_dimension_after
                .max(built.texture.max_dimension_after);
            agg_texture.ktx2_available = agg_texture.ktx2_available || built.texture.ktx2_available;
            agg_texture.ktx2_encoded += built.texture.ktx2_encoded;
            if agg_texture.basisu_path.is_none() {
                agg_texture.basisu_path = built.texture.basisu_path.clone();
            }
            if built.gap.max_gap > agg_gap.max_gap {
                agg_gap = built.gap.clone();
            } else if agg_gap.pair_count == 0 {
                agg_gap = built.gap.clone();
            }

            payloads.insert(
                node.id.clone(),
                NodePayload {
                    content_path,
                    content_uri: uri,
                    world_transform: node.world_transform.clone(),
                    geometric_error: ge,
                    bounds: node.bounds.clone(),
                    node: node.clone(),
                    external_tileset_uri: None,
                },
            );
        }
    }

    let root_level = tree.levels.len() - 1;
    let root_nodes = &tree.levels[root_level];
    if root_nodes.len() != 1 {
        return Err(TopRebuildError::Other(format!(
            "expected single root proxy, got {} (increase --levels to continue merging until one root, or omit --levels for full pyramid; N×N grid needs ~log2(N) merge levels)",
            root_nodes.len()
        )));
    }
    let root_id = &root_nodes[0].id;
    let root_payload = payloads
        .get(root_id)
        .ok_or_else(|| TopRebuildError::Other("missing root payload".into()))?;

    let root_json = emit_node(root_id, &Mat4d::identity(), &payloads, true)?;
    let tileset = json!({
        "asset": { "version": "1.0", "gltfUpAxis": "Z" },
        "geometricError": root_payload.geometric_error,
        "root": root_json
    });
    let tileset_path = output.join("tileset.json");
    fs::write(&tileset_path, serde_json::to_string_pretty(&tileset)?)?;

    let total_triangles_after: u64 = proxy_metrics.iter().map(|p| p.triangles_after).sum();
    let total_texture_bytes: u64 = proxy_metrics.iter().map(|p| p.texture_bytes).sum();
    let total_glb_bytes: u64 = proxy_metrics.iter().map(|p| p.glb_bytes).sum();

    let metrics_path = output.join("rebuild_metrics.json");
    let metrics_json = json!({
        "phase": 8,
        "level_counts": tree.level_counts().iter().map(|(l,c)| json!({"level": l, "count": c})).collect::<Vec<_>>(),
        "lock_border": write_opts.lock_border,
        "budgets": {
            "l1_max_triangles": write_opts.l1_max_triangles,
            "l2_max_triangles": write_opts.l2_max_triangles,
            "maxTextureSize": write_opts.max_texture_size,
            "maxTextureBytes": write_opts.max_texture_bytes,
            "maxGlbBytes": write_opts.max_glb_bytes,
            "enable_ktx2": write_opts.enable_ktx2,
        },
        "gaps": {
            "maxGap": agg_gap.max_gap,
            "P95Gap": agg_gap.p95_gap,
            "pair_count": agg_gap.pair_count,
            "pairs": agg_gap.pairs,
            "notes": agg_gap.notes,
        },
        "textures": {
            "input_count": agg_texture.input_count,
            "unique_count": agg_texture.unique_count,
            "dedup_removed": agg_texture.dedup_removed,
            "total_bytes_before": agg_texture.total_bytes_before,
            "total_bytes_after": agg_texture.total_bytes_after,
            "max_dimension_after": agg_texture.max_dimension_after,
            "ktx2_available": agg_texture.ktx2_available,
            "ktx2_encoded": agg_texture.ktx2_encoded,
            "basisu_path": agg_texture.basisu_path,
        },
        "totals": {
            "triangles_after": total_triangles_after,
            "texture_bytes": total_texture_bytes,
            "glb_bytes": total_glb_bytes,
        },
        "proxies": proxy_metrics.iter().map(|p| json!({
            "node_id": p.node_id,
            "level": p.level,
            "triangles_before": p.triangles_before,
            "triangles_after": p.triangles_after,
            "triangles_target": p.triangles_target,
            "simplification_error_meters": p.simplification_error_meters,
            "geometric_error": p.geometric_error,
            "content_uri": p.content_uri,
            "texture_bytes": p.texture_bytes,
            "glb_bytes": p.glb_bytes,
            "maxGap": p.max_gap,
            "P95Gap": p.p95_gap,
        })).collect::<Vec<_>>(),
        "warnings": warnings,
    });
    fs::write(&metrics_path, serde_json::to_string_pretty(&metrics_json)?)?;

    Ok(RebuildReport {
        level_counts: tree.level_counts(),
        proxies: proxy_metrics,
        tileset_path,
        metrics_path,
        warnings,
        gap: agg_gap,
        texture: agg_texture,
        total_triangles_after,
        total_texture_bytes,
        total_glb_bytes,
    })
}

/// Verify `parent * child_local ≈ child_world` within `eps`.
pub fn assert_world_transform_invariant(
    parent_world: &Mat4d,
    child_world: &Mat4d,
    child_local: &Mat4d,
    eps: f64,
) -> Result<()> {
    let composed = parent_world.mul(child_local);
    for i in 0..16 {
        let d = (composed.0[i] - child_world.0[i]).abs();
        if d > eps {
            return Err(TopRebuildError::Other(format!(
                "transform invariant failed at [{i}]: composed={} world={} delta={d}",
                composed.0[i], child_world.0[i]
            )));
        }
    }
    Ok(())
}

/// Walk a written tileset.json and collect refine / GE / content counts.
pub fn probe_tileset_structure(tileset_path: &Path) -> Result<TilesetProbe> {
    let text = fs::read_to_string(tileset_path)?;
    let doc: Value = serde_json::from_str(&text)?;
    let root = doc
        .get("root")
        .ok_or_else(|| TopRebuildError::InvalidTileset("missing root".into()))?;
    let mut probe = TilesetProbe::default();
    walk_probe(root, 0, &mut probe);
    Ok(probe)
}

#[derive(Clone, Debug, Default)]
pub struct TilesetProbe {
    pub proxy_nodes: usize,
    pub leaf_external: usize,
    pub replace_count: usize,
    pub max_depth: u32,
    pub geometric_errors: Vec<f64>,
}

fn walk_probe(node: &Value, depth: u32, probe: &mut TilesetProbe) {
    probe.max_depth = probe.max_depth.max(depth);
    if let Some(ge) = node.get("geometricError").and_then(|v| v.as_f64()) {
        probe.geometric_errors.push(ge);
    }
    if node
        .get("refine")
        .and_then(|v| v.as_str())
        .map(|s| s.eq_ignore_ascii_case("REPLACE"))
        .unwrap_or(false)
    {
        probe.replace_count += 1;
    }
    if let Some(uri) = node
        .get("content")
        .and_then(|c| c.get("uri"))
        .and_then(|u| u.as_str())
    {
        if uri.contains("Proxy_") {
            probe.proxy_nodes += 1;
        }
        if uri.ends_with("tileset.json") && uri.contains("Tile_") {
            probe.leaf_external += 1;
        }
    }
    if let Some(children) = node.get("children").and_then(|c| c.as_array()) {
        for c in children {
            walk_probe(c, depth + 1, probe);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn relative_transform_roundtrip() {
        let parent = Mat4d::translation(100.0, 100.0, 10.0);
        let child = Mat4d::translation(50.0, 50.0, 10.0);
        let local = relative_transform(&parent, &child).unwrap();
        assert_world_transform_invariant(&parent, &child, &local, 1e-9).unwrap();
        let p = local.transform_point(0.0, 0.0, 0.0);
        assert!((p.0 + 50.0).abs() < 1e-9);
        assert!((p.1 + 50.0).abs() < 1e-9);
    }

    #[test]
    fn geometric_error_monotonic() {
        let bv = BoundingVolume::from_box([0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 100.0, 0.0, 0.0, 0.0, 10.0]);
        let ge = geometric_error_proxy(&[50.0, 40.0], 0.5, &bv);
        assert!(ge > 50.0);
        let ge2 = geometric_error_proxy(&[ge], 1.0, &bv);
        assert!(ge2 > ge);
    }

    #[test]
    fn bv_to_local_centered() {
        let world_bv =
            BoundingVolume::from_box([100.0, 100.0, 10.0, 50.0, 0.0, 0.0, 0.0, 50.0, 0.0, 0.0, 0.0, 10.0]);
        let t = Mat4d::translation(100.0, 100.0, 10.0);
        let local = bv_world_to_local(&world_bv, &t);
        let c = local.center().unwrap();
        assert!(c.0.abs() < 1e-9 && c.1.abs() < 1e-9 && c.2.abs() < 1e-9);
    }
}
