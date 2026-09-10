//! GLB load (scene transform expand) and writer with optional embedded textures (Phase 8).

use crate::error::{Result, TopRebuildError};
use crate::texture::{extract_textures_from_glb, ProcessedTexture, TextureData};
use crate::types::Mat4d;
use byteorder::{LittleEndian, WriteBytesExt};
use std::collections::BTreeMap;

/// One triangulated primitive in a local (already scene-expanded) frame.
#[derive(Clone, Debug, Default)]
pub struct LoadedPrimitive {
    pub positions: Vec<[f32; 3]>,
    pub normals: Option<Vec<[f32; 3]>>,
    pub uvs: Option<Vec<[f32; 2]>>,
    pub indices: Vec<u32>,
    /// Simple material key (baseColor + optional texture hash).
    pub material_key: String,
    /// SHA-256 of source texture (if any) for dedup / grouping.
    pub texture_hash: Option<String>,
}

impl LoadedPrimitive {
    pub fn triangle_count(&self) -> usize {
        self.indices.len() / 3
    }
}

/// Mesh + textures loaded from one GLB/B3DM payload.
#[derive(Clone, Debug, Default)]
pub struct LoadedMesh {
    pub primitives: Vec<LoadedPrimitive>,
    pub textures: Vec<TextureData>,
}

/// If the glTF crate rejects unknown `extensionsRequired` (e.g. KHR_techniques_webgl
/// from older converters), demote those entries to `extensionsUsed` and retry parse.
pub fn parse_gltf_lenient(glb: &[u8]) -> Result<gltf::Gltf> {
    match gltf::Gltf::from_slice(glb) {
        Ok(g) => Ok(g),
        Err(e) => {
            let msg = e.to_string();
            if !msg.contains("Unsupported extension") && !msg.contains("extensionsRequired") {
                return Err(TopRebuildError::Other(format!("gltf parse: {e}")));
            }
            let demoted = demote_unsupported_extensions_required(glb)
                .map_err(|d| TopRebuildError::Other(format!("gltf parse: {e}; demote failed: {d}")))?;
            gltf::Gltf::from_slice(&demoted)
                .map_err(|e2| TopRebuildError::Other(format!("gltf parse after demote: {e2}")))
        }
    }
}

fn demote_unsupported_extensions_required(glb: &[u8]) -> std::result::Result<Vec<u8>, String> {
    if glb.len() < 20 || &glb[0..4] != b"glTF" {
        return Err("not glb".into());
    }
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap()) as usize;
    if 20 + json_len > glb.len() {
        return Err("json chunk truncated".into());
    }
    let json_bytes = &glb[20..20 + json_len];
    let json_str = std::str::from_utf8(json_bytes).map_err(|e| e.to_string())?;
    let mut root: serde_json::Value =
        serde_json::from_str(json_str.trim_end()).map_err(|e| e.to_string())?;
    let req = root
        .get("extensionsRequired")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if req.is_empty() {
        return Err("no extensionsRequired to demote".into());
    }
    // Keep only extensions the gltf crate is known to accept for required.
    let known = ["KHR_texture_basisu", "KHR_materials_unlit"];
    let (keep, demote): (Vec<_>, Vec<_>) = req.into_iter().partition(|e| {
        e.as_str()
            .map(|s| known.iter().any(|k| *k == s))
            .unwrap_or(false)
    });
    if demote.is_empty() {
        return Err("extensionsRequired already only known".into());
    }
    let mut used = root
        .get("extensionsUsed")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    for d in &demote {
        if !used.iter().any(|u| u == d) {
            used.push(d.clone());
        }
    }
    root["extensionsUsed"] = serde_json::Value::Array(used);
    if keep.is_empty() {
        root.as_object_mut().map(|o| o.remove("extensionsRequired"));
    } else {
        root["extensionsRequired"] = serde_json::Value::Array(keep);
    }
    let mut new_json = serde_json::to_vec(&root).map_err(|e| e.to_string())?;
    while new_json.len() % 4 != 0 {
        new_json.push(b' ');
    }
    let bin_start = 20 + json_len;
    let rest = &glb[bin_start..];
    let total = 12 + 8 + new_json.len() + rest.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(new_json.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&new_json);
    out.extend_from_slice(rest);
    Ok(out)
}

/// Load mesh + textures from GLB bytes.
pub fn load_mesh_from_glb(glb: &[u8]) -> Result<LoadedMesh> {
    let (textures, img_to_hash) = extract_textures_from_glb(glb)?;
    let gltf = parse_gltf_lenient(glb)?;
    let blob = gltf
        .blob
        .as_ref()
        .ok_or_else(|| TopRebuildError::Other("GLB missing BIN chunk".into()))?;

    // Map material index → texture hash (baseColorTexture)
    let mut mat_tex: BTreeMap<usize, String> = BTreeMap::new();
    for (mi, mat) in gltf.document.materials().enumerate() {
        if let Some(info) = mat.pbr_metallic_roughness().base_color_texture() {
            let tex = info.texture();
            let img_idx = tex.source().index();
            if let Some(h) = img_to_hash.get(&img_idx) {
                mat_tex.insert(mi, h.clone());
            }
        }
    }

    let mut out = Vec::new();
    let doc = &gltf.document;
    if doc.scenes().len() == 0 {
        for node in doc.nodes() {
            visit_node(&node, &Mat4d::identity(), blob, &mat_tex, &mut out)?;
        }
    } else {
        let roots: Vec<_> = if let Some(scene) = doc.default_scene() {
            scene.nodes().collect()
        } else {
            doc.scenes()
                .next()
                .map(|s| s.nodes().collect())
                .unwrap_or_default()
        };
        for node in roots {
            visit_node(&node, &Mat4d::identity(), blob, &mat_tex, &mut out)?;
        }
    }
    Ok(LoadedMesh {
        primitives: out,
        textures,
    })
}

/// Load all mesh primitives from a GLB byte slice (textures ignored for grouping).
pub fn load_primitives_from_glb(glb: &[u8]) -> Result<Vec<LoadedPrimitive>> {
    Ok(load_mesh_from_glb(glb)?.primitives)
}

fn visit_node(
    node: &gltf::Node<'_>,
    parent: &Mat4d,
    blob: &[u8],
    mat_tex: &BTreeMap<usize, String>,
    out: &mut Vec<LoadedPrimitive>,
) -> Result<()> {
    let local = Mat4d::from_gltf_cols(node.transform().matrix());
    let world = parent.mul(&local);

    if let Some(mesh) = node.mesh() {
        for prim in mesh.primitives() {
            out.push(read_primitive(&prim, blob, &world, mat_tex)?);
        }
    }
    for child in node.children() {
        visit_node(&child, &world, blob, mat_tex, out)?;
    }
    Ok(())
}

fn material_key(prim: &gltf::Primitive<'_>, tex_hash: &Option<String>) -> String {
    let base = match prim.material().index() {
        Some(i) => {
            let m = prim.material();
            let c = m.pbr_metallic_roughness().base_color_factor();
            format!(
                "mat{i}:{:.3},{:.3},{:.3},{:.3}",
                c[0], c[1], c[2], c[3]
            )
        }
        None => "default".into(),
    };
    match tex_hash {
        Some(h) => format!("{base}#tex:{}", &h[..16.min(h.len())]),
        None => base,
    }
}

fn read_primitive(
    prim: &gltf::Primitive<'_>,
    blob: &[u8],
    node_world: &Mat4d,
    mat_tex: &BTreeMap<usize, String>,
) -> Result<LoadedPrimitive> {
    let reader = prim.reader(|buf| {
        if buf.index() == 0 {
            Some(blob)
        } else {
            None
        }
    });

    let positions_iter = reader
        .read_positions()
        .ok_or_else(|| TopRebuildError::Other("primitive missing POSITION".into()))?;
    let mut positions = Vec::new();
    for p in positions_iter {
        let (x, y, z) = node_world.transform_point(p[0] as f64, p[1] as f64, p[2] as f64);
        positions.push([x as f32, y as f32, z as f32]);
    }

    let normals = reader.read_normals().map(|iter| {
        iter.map(|n| {
            let (x, y, z) =
                node_world.transform_direction(n[0] as f64, n[1] as f64, n[2] as f64);
            let len = (x * x + y * y + z * z).sqrt();
            if len > 1e-12 {
                [(x / len) as f32, (y / len) as f32, (z / len) as f32]
            } else {
                [n[0], n[1], n[2]]
            }
        })
        .collect::<Vec<_>>()
    });

    let uvs = reader
        .read_tex_coords(0)
        .map(|tc| tc.into_f32().collect::<Vec<_>>());

    let indices: Vec<u32> = if let Some(idx) = reader.read_indices() {
        idx.into_u32().collect()
    } else {
        (0..positions.len() as u32).collect()
    };

    if prim.mode() != gltf::mesh::Mode::Triangles {
        return Err(TopRebuildError::Other(format!(
            "unsupported primitive mode {:?}",
            prim.mode()
        )));
    }

    let texture_hash = prim
        .material()
        .index()
        .and_then(|i| mat_tex.get(&i).cloned());

    Ok(LoadedPrimitive {
        positions,
        normals,
        uvs,
        indices,
        material_key: material_key(prim, &texture_hash),
        texture_hash,
    })
}

/// Apply an additional world/local Mat4d to all vertices of a primitive (in place).
pub fn transform_primitive(prim: &mut LoadedPrimitive, m: &Mat4d) {
    for p in &mut prim.positions {
        let (x, y, z) = m.transform_point(p[0] as f64, p[1] as f64, p[2] as f64);
        *p = [x as f32, y as f32, z as f32];
    }
    if let Some(ref mut normals) = prim.normals {
        for n in normals.iter_mut() {
            let (x, y, z) = m.transform_direction(n[0] as f64, n[1] as f64, n[2] as f64);
            let len = (x * x + y * y + z * z).sqrt();
            if len > 1e-12 {
                *n = [(x / len) as f32, (y / len) as f32, (z / len) as f32];
            }
        }
    }
}

/// Write GLB without textures (legacy).
pub fn write_glb(primitives: &[LoadedPrimitive]) -> Result<Vec<u8>> {
    write_glb_with_textures(primitives, &[])
}

/// Write GLB embedding processed textures (PNG or KTX2). Original UVs preserved.
pub fn write_glb_with_textures(
    primitives: &[LoadedPrimitive],
    textures: &[ProcessedTexture],
) -> Result<Vec<u8>> {
    if primitives.is_empty() {
        return Err(TopRebuildError::Other("write_glb: no primitives".into()));
    }

    let tex_by_hash: BTreeMap<&str, usize> = textures
        .iter()
        .enumerate()
        .map(|(i, t)| (t.hash.as_str(), i))
        .collect();

    let mut bin: Vec<u8> = Vec::new();
    let mut accessors = Vec::new();
    let mut buffer_views = Vec::new();
    let mut gl_primitives = Vec::new();
    let mut materials = Vec::new();
    let mut material_map: BTreeMap<String, usize> = BTreeMap::new();
    let mut images = Vec::new();
    let mut gl_textures = Vec::new();
    let mut samplers = Vec::new();
    let mut extensions_used: Vec<String> = Vec::new();
    let extensions_required: Vec<String> = Vec::new();

    // Embed textures first so bufferViews exist
    let mut tex_index_by_hash: BTreeMap<String, usize> = BTreeMap::new();
    if !textures.is_empty() {
        samplers.push(serde_json::json!({
            "magFilter": 9729,
            "minFilter": 9987,
            "wrapS": 10497,
            "wrapT": 10497
        }));
    }
    for t in textures {
        let off = bin.len();
        bin.extend_from_slice(&t.bytes);
        pad_bin(&mut bin);
        let view = buffer_views.len();
        buffer_views.push(serde_json::json!({
            "buffer": 0,
            "byteOffset": off,
            "byteLength": t.bytes.len()
        }));
        let img_idx = images.len();
        let img = serde_json::json!({
            "mimeType": t.mime,
            "bufferView": view
        });
        if t.ktx2 {
            if !extensions_used.iter().any(|e| e == "KHR_texture_basisu") {
                extensions_used.push("KHR_texture_basisu".into());
            }
            // KHR_texture_basisu: image stays mime image/ktx2; texture object carries extension
        }
        images.push(img);
        let tex_idx = gl_textures.len();
        // Always set `source` so gltf crate / L2 reload can parse; KHR_texture_basisu
        // points at the same KTX2 image when encoded (PNG fallback omitted for size).
        let mut tex_obj = serde_json::json!({
            "sampler": 0,
            "source": img_idx
        });
        if t.ktx2 {
            tex_obj["extensions"] = serde_json::json!({
                "KHR_texture_basisu": { "source": img_idx }
            });
        }
        gl_textures.push(tex_obj);
        tex_index_by_hash.insert(t.hash.clone(), tex_idx);
        let _ = tex_by_hash; // silence if unused path
    }

    for prim in primitives {
        let mat_idx = *material_map
            .entry(prim.material_key.clone())
            .or_insert_with(|| {
                let idx = materials.len();
                let color = parse_mat_color(&prim.material_key);
                let mut mat = serde_json::json!({
                    "pbrMetallicRoughness": {
                        "baseColorFactor": color,
                        "metallicFactor": 0.0,
                        "roughnessFactor": 1.0
                    }
                });
                if let Some(ref h) = prim.texture_hash {
                    if let Some(&ti) = tex_index_by_hash.get(h) {
                        mat["pbrMetallicRoughness"]["baseColorTexture"] =
                            serde_json::json!({ "index": ti });
                    } else {
                        // hash after resize may differ — match by prefix in material_key
                        if let Some((_, rest)) = prim.material_key.split_once("#tex:") {
                            for (full, &ti) in &tex_index_by_hash {
                                if full.starts_with(rest) || rest.starts_with(&full[..16.min(full.len())]) {
                                    mat["pbrMetallicRoughness"]["baseColorTexture"] =
                                        serde_json::json!({ "index": ti });
                                    break;
                                }
                            }
                        }
                    }
                }
                materials.push(mat);
                idx
            });

        let pos_view = push_f32_view(&mut bin, &mut buffer_views, flatten3(&prim.positions), 34962);
        let (min, max) = aabb(&prim.positions);
        let pos_acc = accessors.len();
        accessors.push(serde_json::json!({
            "bufferView": pos_view,
            "componentType": 5126,
            "count": prim.positions.len(),
            "type": "VEC3",
            "min": min,
            "max": max
        }));

        let mut attrs = serde_json::json!({ "POSITION": pos_acc });

        if let Some(ref normals) = prim.normals {
            let n_view = push_f32_view(&mut bin, &mut buffer_views, flatten3(normals), 34962);
            let n_acc = accessors.len();
            accessors.push(serde_json::json!({
                "bufferView": n_view,
                "componentType": 5126,
                "count": normals.len(),
                "type": "VEC3"
            }));
            attrs["NORMAL"] = serde_json::json!(n_acc);
        }

        if let Some(ref uvs) = prim.uvs {
            let u_view = push_f32_view(&mut bin, &mut buffer_views, flatten2(uvs), 34962);
            let u_acc = accessors.len();
            accessors.push(serde_json::json!({
                "bufferView": u_view,
                "componentType": 5126,
                "count": uvs.len(),
                "type": "VEC2"
            }));
            attrs["TEXCOORD_0"] = serde_json::json!(u_acc);
        }

        let i_off = bin.len();
        for i in &prim.indices {
            bin.write_u32::<LittleEndian>(*i)?;
        }
        pad_bin(&mut bin);
        let i_view = buffer_views.len();
        buffer_views.push(serde_json::json!({
            "buffer": 0,
            "byteOffset": i_off,
            "byteLength": prim.indices.len() * 4,
            "target": 34963
        }));
        let i_acc = accessors.len();
        accessors.push(serde_json::json!({
            "bufferView": i_view,
            "componentType": 5125,
            "count": prim.indices.len(),
            "type": "SCALAR"
        }));

        gl_primitives.push(serde_json::json!({
            "attributes": attrs,
            "indices": i_acc,
            "mode": 4,
            "material": mat_idx
        }));
    }

    pad_bin(&mut bin);

    let mut root = serde_json::json!({
        "asset": { "version": "2.0", "generator": "top_rebuild ProxyBuilder" },
        "buffers": [{ "byteLength": bin.len() }],
        "bufferViews": buffer_views,
        "accessors": accessors,
        "materials": materials,
        "meshes": [{ "primitives": gl_primitives }],
        "nodes": [{ "mesh": 0 }],
        "scenes": [{ "nodes": [0] }],
        "scene": 0
    });
    if !images.is_empty() {
        root["images"] = serde_json::json!(images);
        root["textures"] = serde_json::json!(gl_textures);
        root["samplers"] = serde_json::json!(samplers);
    }
    if !extensions_used.is_empty() {
        root["extensionsUsed"] = serde_json::json!(extensions_used);
    }
    if !extensions_required.is_empty() {
        root["extensionsRequired"] = serde_json::json!(extensions_required);
    }

    let mut json = serde_json::to_vec(&root)?;
    while json.len() % 4 != 0 {
        json.push(b' ');
    }

    let total = 12 + 8 + json.len() + 8 + bin.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.write_u32::<LittleEndian>(2)?;
    out.write_u32::<LittleEndian>(total as u32)?;
    out.write_u32::<LittleEndian>(json.len() as u32)?;
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json);
    out.write_u32::<LittleEndian>(bin.len() as u32)?;
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin);
    Ok(out)
}

fn parse_mat_color(key: &str) -> [f64; 4] {
    let key = key.split("#tex:").next().unwrap_or(key);
    if let Some(rest) = key.split_once(':').map(|(_, r)| r) {
        let parts: Vec<f64> = rest
            .split(',')
            .filter_map(|s| s.parse().ok())
            .collect();
        if parts.len() == 4 {
            return [parts[0], parts[1], parts[2], parts[3]];
        }
    }
    [1.0, 1.0, 1.0, 1.0]
}

fn flatten3(v: &[[f32; 3]]) -> Vec<f32> {
    v.iter().flat_map(|p| [p[0], p[1], p[2]]).collect()
}
fn flatten2(v: &[[f32; 2]]) -> Vec<f32> {
    v.iter().flat_map(|p| [p[0], p[1]]).collect()
}

fn aabb(pos: &[[f32; 3]]) -> ([f32; 3], [f32; 3]) {
    let mut min = [f32::MAX; 3];
    let mut max = [f32::MIN; 3];
    for p in pos {
        for i in 0..3 {
            min[i] = min[i].min(p[i]);
            max[i] = max[i].max(p[i]);
        }
    }
    (min, max)
}

fn pad_bin(bin: &mut Vec<u8>) {
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
}

fn push_f32_view(
    bin: &mut Vec<u8>,
    views: &mut Vec<serde_json::Value>,
    data: Vec<f32>,
    target: u32,
) -> usize {
    let off = bin.len();
    for v in data {
        let _ = bin.write_f32::<LittleEndian>(v);
    }
    pad_bin(bin);
    let idx = views.len();
    views.push(serde_json::json!({
        "buffer": 0,
        "byteOffset": off,
        "byteLength": bin.len() - off,
        "target": target
    }));
    idx
}

/// Axis-aligned box centered at origin, half-extents `hx,hy,hz`, subdivided `segments` per edge.
pub fn make_box_primitive(hx: f32, hy: f32, hz: f32, segments: u32, material_key: &str) -> LoadedPrimitive {
    make_box_primitive_uv(hx, hy, hz, segments, material_key, false)
}

/// Box with optional UVs (for texture path tests).
pub fn make_box_primitive_uv(
    hx: f32,
    hy: f32,
    hz: f32,
    segments: u32,
    material_key: &str,
    with_uvs: bool,
) -> LoadedPrimitive {
    let s = segments.max(1);
    let mut positions = Vec::new();
    let mut normals = Vec::new();
    let mut uvs = if with_uvs { Some(Vec::new()) } else { None };
    let mut indices = Vec::new();

    let faces: [(usize, f32, [f32; 3]); 6] = [
        (0, hx, [1.0, 0.0, 0.0]),
        (0, -hx, [-1.0, 0.0, 0.0]),
        (1, hy, [0.0, 1.0, 0.0]),
        (1, -hy, [0.0, -1.0, 0.0]),
        (2, hz, [0.0, 0.0, 1.0]),
        (2, -hz, [0.0, 0.0, -1.0]),
    ];

    for (axis, sign_ext, nrm) in faces {
        let base = positions.len() as u32;
        for j in 0..=s {
            for i in 0..=s {
                let u = i as f32 / s as f32 * 2.0 - 1.0;
                let v = j as f32 / s as f32 * 2.0 - 1.0;
                let mut p = [0.0f32; 3];
                p[axis] = sign_ext;
                let (a, b) = match axis {
                    0 => (1, 2),
                    1 => (0, 2),
                    _ => (0, 1),
                };
                let au = if axis == 0 && nrm[0] < 0.0 {
                    -u
                } else if axis == 1 && nrm[1] < 0.0 {
                    -u
                } else {
                    u
                };
                p[a] = au * if a == 0 { hx } else if a == 1 { hy } else { hz };
                p[b] = v * if b == 0 { hx } else if b == 1 { hy } else { hz };
                positions.push(p);
                normals.push(nrm);
                if let Some(ref mut uv) = uvs {
                    uv.push([i as f32 / s as f32, j as f32 / s as f32]);
                }
            }
        }
        let stride = s + 1;
        for j in 0..s {
            for i in 0..s {
                let i0 = base + j * stride + i;
                let i1 = i0 + 1;
                let i2 = i0 + stride;
                let i3 = i2 + 1;
                if nrm[0] + nrm[1] + nrm[2] > 0.0 {
                    indices.extend_from_slice(&[i0, i2, i1, i1, i2, i3]);
                } else {
                    indices.extend_from_slice(&[i0, i1, i2, i1, i3, i2]);
                }
            }
        }
    }

    LoadedPrimitive {
        positions,
        normals: Some(normals),
        uvs,
        indices,
        material_key: material_key.to_string(),
        texture_hash: None,
    }
}

/// Build a textured box GLB (solid color PNG) for Phase 8 fixtures / synthesize.
pub fn make_textured_box_glb(
    hx: f32,
    hy: f32,
    hz: f32,
    segments: u32,
    material_key: &str,
    rgb: (u8, u8, u8),
    tex_size: u32,
) -> Result<Vec<u8>> {
    let tex = TextureData::solid(rgb.0, rgb.1, rgb.2, 255, tex_size);
    let mut prim = make_box_primitive_uv(hx, hy, hz, segments, material_key, true);
    prim.texture_hash = Some(tex.hash.clone());
    prim.material_key = format!(
        "{}#tex:{}",
        material_key,
        &tex.hash[..16.min(tex.hash.len())]
    );
    let (processed, _) = crate::texture::process_textures(&[tex], tex_size.max(64), 0, false, None)?;
    write_glb_with_textures(&[prim], &processed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn box_roundtrip_glb() {
        let prim = make_box_primitive(1.0, 1.0, 1.0, 2, "default");
        assert!(prim.triangle_count() > 10);
        let glb = write_glb(&[prim]).unwrap();
        assert_eq!(&glb[0..4], b"glTF");
        let loaded = load_primitives_from_glb(&glb).unwrap();
        assert_eq!(loaded.len(), 1);
        assert!(loaded[0].triangle_count() > 10);
    }

    #[test]
    fn textured_box_roundtrip() {
        let glb = make_textured_box_glb(2.0, 2.0, 1.0, 2, "mat0:1,0,0,1", (200, 40, 40), 32).unwrap();
        let mesh = load_mesh_from_glb(&glb).unwrap();
        assert!(!mesh.textures.is_empty());
        assert!(mesh.primitives[0].uvs.is_some());
        assert!(mesh.primitives[0].texture_hash.is_some());
    }

    #[test]
    fn scene_node_translation_applied() {
        let mut prim = make_box_primitive(1.0, 1.0, 1.0, 1, "default");
        let m = Mat4d::translation(100.0, 0.0, 0.0);
        transform_primitive(&mut prim, &m);
        let cx: f32 =
            prim.positions.iter().map(|p| p[0]).sum::<f32>() / prim.positions.len() as f32;
        assert!((cx - 100.0).abs() < 1e-3);
    }
}
