use super::math::*;
use crate::CancelFlag;
use serde_json::{json, Value};
use std::collections::{BTreeMap, HashSet};

pub struct Content {
    pub document: Value,
    pub binary: Vec<u8>,
    pub rtc: Point,
    pub b3dm: bool,
}
fn u32_at(data: &[u8], offset: usize) -> Result<u32, String> {
    Ok(u32::from_le_bytes(
        data.get(offset..offset + 4)
            .ok_or("truncated binary header")?
            .try_into()
            .unwrap(),
    ))
}
impl Content {
    pub fn parse(bytes: &[u8]) -> Result<Self, String> {
        let b3dm = bytes.starts_with(b"b3dm");
        let mut rtc = [0.; 3];
        let mut glb = bytes;
        if b3dm {
            if u32_at(bytes, 4)? != 1 || u32_at(bytes, 8)? as usize != bytes.len() {
                return Err("invalid B3DM header".into());
            }
            let fj = u32_at(bytes, 12)? as usize;
            let fb = u32_at(bytes, 16)? as usize;
            let bj = u32_at(bytes, 20)? as usize;
            let bb = u32_at(bytes, 24)? as usize;
            if fb != 0 || bj != 0 || bb != 0 {
                return Err(
                    "clip does not support B3DM feature/batch binary tables or batch metadata"
                        .into(),
                );
            }
            let table: Value =
                serde_json::from_slice(bytes.get(28..28 + fj).ok_or("invalid B3DM feature table")?)
                    .map_err(|e| e.to_string())?;
            if table["BATCH_LENGTH"].as_u64() != Some(0) {
                return Err("clip requires unbatched B3DM (BATCH_LENGTH=0)".into());
            }
            if table
                .as_object()
                .ok_or("invalid feature table")?
                .keys()
                .any(|k| k != "BATCH_LENGTH" && k != "RTC_CENTER")
            {
                return Err("unsupported B3DM feature table property".into());
            }
            if let Some(v) = table.get("RTC_CENTER") {
                rtc = array::<3>(v)?;
            }
            glb = bytes.get(28 + fj..).ok_or("invalid B3DM GLB offset")?;
            let length = u32_at(glb, 8)? as usize;
            if length < 12
                || length > glb.len()
                || glb.len() - length > 7
                || glb[length..].iter().any(|b| *b != 0)
            {
                return Err("invalid B3DM GLB length or trailing padding".into());
            }
            glb = &glb[..length];
        }
        if !glb.starts_with(b"glTF")
            || u32_at(glb, 4)? != 2
            || u32_at(glb, 8)? as usize != glb.len()
        {
            return Err("clip requires a valid glTF 2 GLB".into());
        }
        let len = u32_at(glb, 12)? as usize;
        if glb.get(16..20) != Some(b"JSON") || len > 16 * 1024 * 1024 || len % 4 != 0 {
            return Err("invalid GLB JSON chunk".into());
        }
        let document: Value =
            serde_json::from_slice(glb.get(20..20 + len).ok_or("truncated GLB JSON")?)
                .map_err(|e| e.to_string())?;
        let bin_start = 20 + len;
        if u32_at(glb, bin_start + 4)? != 0x004e4942 {
            return Err("GLB must contain one BIN chunk".into());
        }
        let bin_len = u32_at(glb, bin_start)? as usize;
        if bin_start + 8 + bin_len != glb.len() {
            return Err("invalid GLB BIN length or extra chunks".into());
        }
        let binary = glb[bin_start + 8..].to_vec();
        if document["asset"]["version"] != "2.0"
            || document["buffers"].as_array().map(Vec::len) != Some(1)
            || document["buffers"][0].get("uri").is_some()
        {
            return Err("clip requires one embedded glTF 2 buffer".into());
        }
        let declared = document["buffers"][0]["byteLength"]
            .as_u64()
            .ok_or("missing buffer length")? as usize;
        if declared > binary.len() || binary.len() - declared > 3 {
            return Err("invalid glTF buffer length".into());
        }
        reject_features(&document)?;
        Ok(Self {
            document,
            binary,
            rtc,
            b3dm,
        })
    }
    pub fn bytes(&self) -> Result<Vec<u8>, String> {
        let mut text = serde_json::to_vec(&self.document).map_err(|e| e.to_string())?;
        while text.len() % 4 != 0 {
            text.push(b' ');
        }
        let mut bin = self.binary.clone();
        while bin.len() % 4 != 0 {
            bin.push(0);
        }
        if self.b3dm && (28 + text.len() + bin.len()) % 8 != 0 {
            text.extend_from_slice(b"    ");
        }
        let length = 28 + text.len() + bin.len();
        let mut out = Vec::with_capacity(length);
        out.extend_from_slice(b"glTF");
        for n in [2, length as u32, text.len() as u32, 0x4e4f534a] {
            out.extend_from_slice(&n.to_le_bytes());
        }
        out.extend_from_slice(&text);
        out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
        out.extend_from_slice(&0x004e4942_u32.to_le_bytes());
        out.extend_from_slice(&bin);
        if self.b3dm {
            let mut ft = serde_json::to_vec(&json!({"BATCH_LENGTH":0,"RTC_CENTER":self.rtc}))
                .map_err(|e| e.to_string())?;
            while (28 + ft.len()) % 8 != 0 {
                ft.push(b' ');
            }
            let mut wrapper = Vec::new();
            wrapper.extend_from_slice(b"b3dm");
            for n in [
                1,
                (28 + ft.len() + out.len()) as u32,
                ft.len() as u32,
                0,
                0,
                0,
            ] {
                wrapper.extend_from_slice(&n.to_le_bytes());
            }
            wrapper.extend_from_slice(&ft);
            wrapper.extend_from_slice(&out);
            out = wrapper;
        }
        Ok(out)
    }
}
fn reject_features(v: &Value) -> Result<(), String> {
    match v {
        Value::Object(o) => {
            for k in [
                "skins",
                "animations",
                "targets",
                "weights",
                "skin",
                "sparse",
            ] {
                if o.contains_key(k) {
                    return Err(format!("clip does not support glTF {k}"));
                }
            }
            if let Some(e) = o.get("extensions").and_then(Value::as_object) {
                for k in e.keys() {
                    if !matches!(k.as_str(), "KHR_materials_unlit" | "KHR_texture_transform") {
                        return Err(format!("clip does not support glTF extension {k}"));
                    }
                }
            }
            for key in ["extensionsRequired", "extensionsUsed"] {
                if let Some(a) = o.get(key).and_then(Value::as_array) {
                    for e in a {
                        if !matches!(
                            e.as_str(),
                            Some("KHR_materials_unlit" | "KHR_texture_transform")
                        ) {
                            return Err(format!("unsupported glTF extension: {e}"));
                        }
                    }
                }
            }
            for (k, c) in o {
                if k != "extras" {
                    reject_features(c)?;
                }
            }
        }
        Value::Array(a) => {
            for c in a {
                reject_features(c)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn index(v: &Value, label: &str) -> Result<usize, String> {
    v.as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| format!("invalid {label} index"))
}
fn view<'a>(doc: &Value, bin: &'a [u8], idx: usize) -> Result<(&'a [u8], usize), String> {
    let v = doc["bufferViews"].get(idx).ok_or("missing bufferView")?;
    if v["buffer"].as_u64() != Some(0) {
        return Err("bufferView must use embedded buffer".into());
    }
    let offset = v
        .get("byteOffset")
        .map(|v| index(v, "byteOffset"))
        .transpose()?
        .unwrap_or(0);
    let length = index(&v["byteLength"], "byteLength")?;
    let declared = index(&doc["buffers"][0]["byteLength"], "buffer length")?;
    let end = offset
        .checked_add(length)
        .filter(|n| *n <= declared)
        .ok_or("bufferView out of bounds")?;
    let stride = v
        .get("byteStride")
        .map(|v| index(v, "byteStride"))
        .transpose()?
        .unwrap_or(0);
    Ok((
        bin.get(offset..end).ok_or("bufferView out of bounds")?,
        stride,
    ))
}
fn accessor(doc: &Value, bin: &[u8], idx: usize) -> Result<Vec<Vec<f64>>, String> {
    let a = doc["accessors"].get(idx).ok_or("missing accessor")?;
    let count = index(&a["count"], "count")?;
    if count == 0 || count > 1_000_000 {
        return Err("accessor count must be 1..1000000".into());
    }
    let components = match a["type"].as_str() {
        Some("SCALAR") => 1,
        Some("VEC2") => 2,
        Some("VEC3") => 3,
        Some("VEC4") => 4,
        _ => return Err("unsupported accessor shape".into()),
    };
    let component = a["componentType"].as_u64().ok_or("invalid componentType")?;
    let width = match component {
        5120 | 5121 => 1,
        5122 | 5123 => 2,
        5125 | 5126 => 4,
        _ => return Err("unsupported accessor component type".into()),
    };
    let normalized = a["normalized"].as_bool().unwrap_or(false);
    if normalized && matches!(component, 5125 | 5126) {
        return Err("invalid normalized accessor".into());
    }
    let (bytes, stride) = view(doc, bin, index(&a["bufferView"], "bufferView")?)?;
    let stride = if stride == 0 {
        components * width
    } else {
        stride
    };
    if stride < components * width || stride > 252 || stride % width != 0 {
        return Err("invalid accessor stride".into());
    }
    let offset = a
        .get("byteOffset")
        .map(|v| index(v, "byteOffset"))
        .transpose()?
        .unwrap_or(0);
    let end = offset
        .checked_add((count - 1) * stride + components * width)
        .filter(|n| *n <= bytes.len())
        .ok_or("accessor outside bufferView")?;
    let _ = end;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let mut values = Vec::with_capacity(components);
        for c in 0..components {
            let start = offset + i * stride + c * width;
            let b = &bytes[start..start + width];
            let value = match component {
                5120 => {
                    let n = b[0] as i8 as f64;
                    if normalized {
                        (n / 127.).max(-1.)
                    } else {
                        n
                    }
                }
                5121 => {
                    let n = b[0] as f64;
                    if normalized {
                        n / 255.
                    } else {
                        n
                    }
                }
                5122 => {
                    let n = i16::from_le_bytes(b.try_into().unwrap()) as f64;
                    if normalized {
                        (n / 32767.).max(-1.)
                    } else {
                        n
                    }
                }
                5123 => {
                    let n = u16::from_le_bytes(b.try_into().unwrap()) as f64;
                    if normalized {
                        n / 65535.
                    } else {
                        n
                    }
                }
                5125 => u32::from_le_bytes(b.try_into().unwrap()) as f64,
                _ => f32::from_le_bytes(b.try_into().unwrap()) as f64,
            };
            if !value.is_finite() {
                return Err("non-finite mesh attribute".into());
            }
            values.push(value);
        }
        out.push(values);
    }
    Ok(out)
}
fn append_view(doc: &mut Value, bin: &mut Vec<u8>, data: &[u8], target: Option<u32>) -> usize {
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let offset = bin.len();
    bin.extend_from_slice(data);
    let views = doc["bufferViews"].as_array_mut().unwrap();
    let id = views.len();
    let mut v = json!({"buffer":0,"byteOffset":offset,"byteLength":data.len()});
    if let Some(t) = target {
        v["target"] = json!(t);
    }
    views.push(v);
    id
}
fn write_attribute(
    doc: &mut Value,
    bin: &mut Vec<u8>,
    vertices: &[Vertex],
    offset: usize,
    width: usize,
    kind: &str,
) -> Result<usize, String> {
    let mut bytes = Vec::with_capacity(vertices.len() * width * 4);
    let mut min = vec![f64::INFINITY; width];
    let mut max = vec![f64::NEG_INFINITY; width];
    for v in vertices {
        for i in 0..width {
            let n = v.values[offset + i] as f32;
            if !n.is_finite() {
                return Err("mesh attribute overflows f32".into());
            }
            min[i] = min[i].min(n as f64);
            max[i] = max[i].max(n as f64);
            bytes.extend_from_slice(&n.to_le_bytes());
        }
    }
    let view = append_view(doc, bin, &bytes, Some(34962));
    let a = doc["accessors"].as_array_mut().unwrap();
    let id = a.len();
    let mut value =
        json!({"bufferView":view,"componentType":5126,"count":vertices.len(),"type":kind});
    value["min"] = json!(min);
    value["max"] = json!(max);
    a.push(value);
    Ok(id)
}
#[derive(Default)]
pub struct Counts {
    pub before: u64,
    pub after: u64,
    pub flattened: u64,
    pub walls: u64,
}
/// Rebuild only geometry accessors. Materials, samplers and image bytes keep their original meaning.
pub fn crop(
    content: &mut Content,
    region: &Region,
    tile_world: Matrix,
    cancel: &CancelFlag,
    counts: &mut Counts,
    flatten_height: Option<f64>,
) -> Result<Option<Bounds>, String> {
    let original = content.document.clone();
    let source_binary = std::mem::take(&mut content.binary);
    content.document["bufferViews"] = json!([]);
    content.document["accessors"] = json!([]);
    content.document["meshes"] = json!([]);
    if let Some(images) = original["images"].as_array() {
        for (i, image) in images.iter().enumerate() {
            if let Some(v) = image.get("bufferView") {
                let (data, stride) =
                    view(&original, &source_binary, index(v, "image bufferView")?)?;
                if stride != 0 {
                    return Err("image bufferView must not have a stride".into());
                }
                let id = append_view(&mut content.document, &mut content.binary, data, None);
                content.document["images"][i]["bufferView"] = json!(id);
            } else if image["uri"].as_str().is_none() {
                return Err("image has no resource".into());
            }
        }
    }
    let nodes = original["nodes"].as_array().ok_or("glTF requires nodes")?;
    if nodes.len() > 100_000 {
        return Err("too many glTF nodes".into());
    }
    for node in content.document["nodes"].as_array_mut().unwrap() {
        node.as_object_mut()
            .ok_or("invalid glTF node")?
            .remove("mesh");
    }
    let scenes = original["scenes"]
        .as_array()
        .filter(|s| s.len() == 1)
        .ok_or("clip requires exactly one glTF scene")?;
    if original.get("scene").is_some_and(|s| s.as_u64() != Some(0)) {
        return Err("invalid default glTF scene".into());
    }
    let scene_nodes = scenes[0]["nodes"]
        .as_array()
        .ok_or("glTF scene requires nodes")?;
    let mut visited = HashSet::new();
    let mut bounds = None;
    let mut rtc = IDENTITY;
    rtc[12] = content.rtc[0];
    rtc[13] = content.rtc[1];
    rtc[14] = content.rtc[2];
    let base = multiply(rtc, Y_UP);
    let mut context = CropContext {
        original: &original,
        binary: &source_binary,
        region,
        tile_world,
        base,
        cancel,
        counts,
        visited: &mut visited,
        flatten_height,
    };
    for node in scene_nodes {
        let b = context.node(index(node, "scene node")?, IDENTITY, 0, content)?;
        bounds = union(bounds, b);
    }
    if visited.len() != nodes.len() {
        return Err("clip does not support unreachable glTF nodes".into());
    }
    content.document["buffers"] = json!([{"byteLength":content.binary.len()}]);
    Ok(bounds)
}
struct CropContext<'a> {
    original: &'a Value,
    binary: &'a [u8],
    region: &'a Region,
    tile_world: Matrix,
    base: Matrix,
    cancel: &'a CancelFlag,
    counts: &'a mut Counts,
    visited: &'a mut HashSet<usize>,
    flatten_height: Option<f64>,
}
impl CropContext<'_> {
    fn node(
        &mut self,
        id: usize,
        parent: Matrix,
        depth: usize,
        out: &mut Content,
    ) -> Result<Option<Bounds>, String> {
        if depth > 128 || !self.visited.insert(id) {
            return Err("cyclic/shared or deeply nested glTF node".into());
        }
        if self.cancel.is_cancelled() {
            return Err("cancelled".into());
        }
        let node = self.original["nodes"].get(id).ok_or("missing glTF node")?;
        let local = multiply(parent, node_transform(node)?);
        let tile = multiply(self.base, local);
        let world = multiply(self.tile_world, tile);
        let projected = multiply(self.region.frame, world);
        let mut bounds = None;
        if let Some(mesh) = node.get("mesh") {
            let mesh = self.original["meshes"]
                .get(index(mesh, "mesh")?)
                .ok_or("missing glTF mesh")?;
            let primitives = mesh["primitives"]
                .as_array()
                .ok_or("mesh requires primitives")?;
            let mut result = Vec::new();
            for primitive in primitives {
                if let Some((p, b)) = self.primitive(primitive, tile, world, projected, out)? {
                    result.push(p);
                    bounds = union(bounds, Some(b));
                }
            }
            if !result.is_empty() {
                let id_mesh = out.document["meshes"].as_array().unwrap().len();
                let mut m = mesh.clone();
                m["primitives"] = json!(result);
                out.document["meshes"].as_array_mut().unwrap().push(m);
                out.document["nodes"][id]["mesh"] = json!(id_mesh);
            }
        }
        if let Some(children) = node["children"].as_array() {
            for c in children {
                bounds = union(
                    bounds,
                    self.node(index(c, "node child")?, local, depth + 1, out)?,
                );
            }
        }
        Ok(bounds)
    }
    fn primitive(
        &mut self,
        p: &Value,
        tile: Matrix,
        world: Matrix,
        projected: Matrix,
        out: &mut Content,
    ) -> Result<Option<(Value, Bounds)>, String> {
        if p.get("mode").is_some_and(|m| m.as_u64() != Some(4)) {
            return Err("clip supports TRIANGLES primitives only".into());
        }
        if let Some(m) = p.get("material") {
            let id = index(m, "material")?;
            if self.original["materials"].get(id).is_none() {
                return Err("missing glTF material".into());
            }
        }
        let attrs = p["attributes"]
            .as_object()
            .ok_or("primitive needs attributes")?;
        let mut layout = BTreeMap::new();
        let mut data = BTreeMap::new();
        let mut width = 0;
        for (name, id) in attrs {
            let expected = match name.as_str() {
                "POSITION" | "NORMAL" => 3,
                "TANGENT" => 4,
                "TEXCOORD_0" | "TEXCOORD_1" => 2,
                "COLOR_0" => 0,
                _ => return Err(format!("clip does not support attribute {name}")),
            };
            let idx = index(id, "attribute")?;
            let a = &self.original["accessors"][idx];
            if matches!(name.as_str(), "POSITION" | "NORMAL" | "TANGENT")
                && a["componentType"].as_u64() != Some(5126)
            {
                return Err("position/normal/tangent must use float accessors".into());
            }
            let values = accessor(self.original, self.binary, idx)?;
            let n = values[0].len();
            if (expected != 0 && n != expected) || (expected == 0 && n != 3 && n != 4) {
                return Err("invalid attribute width".into());
            }
            layout.insert(name.clone(), (width, n));
            width += n;
            data.insert(name.clone(), values);
        }
        let position = data.get("POSITION").ok_or("primitive needs POSITION")?;
        let count = position.len();
        if data.values().any(|v| v.len() != count) {
            return Err("attribute counts differ".into());
        }
        let indices: Vec<usize> = if let Some(i) = p.get("indices") {
            let idx = index(i, "indices")?;
            let a = &self.original["accessors"][idx];
            if a["type"] != "SCALAR"
                || !matches!(a["componentType"].as_u64(), Some(5121 | 5123 | 5125))
                || a["normalized"].as_bool() == Some(true)
            {
                return Err("invalid index accessor".into());
            }
            accessor(self.original, self.binary, idx)?
                .iter()
                .map(|v| v[0] as usize)
                .collect()
        } else {
            (0..count).collect()
        };
        if indices.len() % 3 != 0 || indices.iter().any(|i| *i >= count) {
            return Err("invalid triangle indices".into());
        }
        self.counts.before += indices.len() as u64 / 3;
        let (pos_offset, _) = layout["POSITION"];
        let mut shading = super::flatten::Shading::default();
        if self.flatten_height.is_some() {
            let texture = &self.original["materials"]
                [p["material"].as_u64().unwrap_or(usize::MAX as u64) as usize]["normalTexture"];
            let transform = &texture["extensions"]["KHR_texture_transform"];
            let tex_coord = transform
                .get("texCoord")
                .or_else(|| texture.get("texCoord"))
                .map(|v| v.as_u64().ok_or("invalid normal-map texture coordinate"))
                .transpose()?
                .unwrap_or(0);
            if tex_coord > 1 {
                return Err("flatten supports normal maps on TEXCOORD_0 or TEXCOORD_1".into());
            }
            shading.uv = format!("TEXCOORD_{tex_coord}");
            let scale = transform
                .get("scale")
                .map(array::<2>)
                .transpose()?
                .unwrap_or([1., 1.]);
            let rotation = transform
                .get("rotation")
                .map(|v| {
                    v.as_f64()
                        .filter(|r| r.is_finite())
                        .ok_or("invalid texture rotation")
                })
                .transpose()?
                .unwrap_or(0.);
            let (s, c) = rotation.sin_cos();
            shading.transform = [c * scale[0], -s * scale[1], s * scale[0], c * scale[1]];
        }
        let mut vertices = Vec::new();
        let mut bounds = None;
        let mut projected_bounds = None;
        for p in position {
            let p: Point = p.as_slice().try_into().unwrap();
            let w = point(world, p);
            let radius = dot(w, w).sqrt();
            if !(6_000_000.0..=7_000_000.0).contains(&radius) {
                return Err("clip input geometry must already be georeferenced in ECEF".into());
            }
            projected_bounds = union(projected_bounds, Some(Bounds::new(point(projected, p))));
        }
        let projected_bounds = projected_bounds.unwrap();
        let corners: Vec<Point> = (0..4)
            .map(|mask| {
                [
                    if mask & 1 == 0 {
                        projected_bounds.min[0]
                    } else {
                        projected_bounds.max[0]
                    },
                    if mask & 2 == 0 {
                        projected_bounds.min[1]
                    } else {
                        projected_bounds.max[1]
                    },
                    0.,
                ]
            })
            .collect();
        if self.flatten_height.is_none() && self.region.excludes_bounds(&corners) {
            return Ok(None);
        }
        // A concave ring is not the intersection of its perimeter half-planes.
        let fully_inside =
            self.region.pieces().len() == 1 && corners.iter().all(|p| self.region.contains(*p, 0.));
        for (triangle, ids) in indices.chunks_exact(3).enumerate() {
            if triangle % 256 == 0 && self.cancel.is_cancelled() {
                return Err("cancelled".into());
            }
            if layout.contains_key("TANGENT") {
                let tangents = &data["TANGENT"];
                if ids.iter().any(|i| tangents[*i][3].abs() != 1.)
                    || tangents[ids[0]][3] != tangents[ids[1]][3]
                    || tangents[ids[0]][3] != tangents[ids[2]][3]
                {
                    return Err("triangle tangent handedness must agree".into());
                }
            }
            let mut input = Vec::new();
            for id in ids {
                let v: Point = position[*id].as_slice().try_into().unwrap();
                let w = point(world, v);
                let radius = dot(w, w).sqrt();
                if !(6_000_000.0..=7_000_000.0).contains(&radius) {
                    return Err("clip input geometry must already be georeferenced in ECEF".into());
                }
                let mut values = Vec::with_capacity(width);
                for value in data.values() {
                    values.extend_from_slice(&value[*id]);
                }
                input.push(Vertex {
                    values,
                    clip: point(projected, v),
                });
            }
            let triangles = if let Some(height) = self.flatten_height {
                super::flatten::flatten_triangle(
                    self.region,
                    input.try_into().unwrap(),
                    height,
                    inverse(projected)?,
                    pos_offset,
                    &layout,
                    &shading,
                    self.counts,
                )?
            } else {
                let clipped = if fully_inside {
                    vec![input]
                } else {
                    clip_region(self.region, input.try_into().unwrap())
                };
                clipped
                    .iter()
                    .flat_map(|clipped| {
                        (1..clipped.len().saturating_sub(1)).map(|i| {
                            [
                                clipped[0].clone(),
                                clipped[i].clone(),
                                clipped[i + 1].clone(),
                            ]
                        })
                    })
                    .collect()
            };
            for mut tri in triangles {
                let a = point(
                    tile,
                    tri[0].values[pos_offset..pos_offset + 3]
                        .try_into()
                        .unwrap(),
                );
                let b = point(
                    tile,
                    tri[1].values[pos_offset..pos_offset + 3]
                        .try_into()
                        .unwrap(),
                );
                let c = point(
                    tile,
                    tri[2].values[pos_offset..pos_offset + 3]
                        .try_into()
                        .unwrap(),
                );
                let u: Point = std::array::from_fn(|i| b[i] - a[i]);
                let v: Point = std::array::from_fn(|i| c[i] - a[i]);
                let cross = [
                    u[1] * v[2] - u[2] * v[1],
                    u[2] * v[0] - u[0] * v[2],
                    u[0] * v[1] - u[1] * v[0],
                ];
                if dot(cross, cross) < 1e-20 {
                    continue;
                }
                for vertex in &mut tri {
                    if let (Some((normal, _)), Some((tangent, _))) =
                        (layout.get("NORMAL"), layout.get("TANGENT"))
                    {
                        let n: Point = vertex.values[*normal..*normal + 3].try_into().unwrap();
                        let t: Point = vertex.values[*tangent..*tangent + 3].try_into().unwrap();
                        let length = dot(n, n);
                        if length < 1e-24 {
                            return Err("clipped normal is degenerate".into());
                        }
                        let projection = dot(n, t) / length;
                        for i in 0..3 {
                            vertex.values[*tangent + i] -= n[i] * projection;
                        }
                    }
                    for name in ["NORMAL", "TANGENT"] {
                        if let Some((offset, _)) = layout.get(name) {
                            let norm = vertex.values[*offset..*offset + 3]
                                .iter()
                                .map(|v| v * v)
                                .sum::<f64>()
                                .sqrt();
                            if norm < 1e-12 {
                                return Err("clipped normal/tangent is degenerate".into());
                            }
                            for v in &mut vertex.values[*offset..*offset + 3] {
                                *v /= norm;
                            }
                        }
                    }
                    // Bounds use the f32 values that will actually be written to the output.
                    let p = std::array::from_fn(|i| vertex.values[pos_offset + i] as f32 as f64);
                    let q = point(projected, p);
                    if let Some(height) = self.flatten_height {
                        if (vertex.clip[2] - height).abs() < 1e-6 && (q[2] - height).abs() > 0.02 {
                            return Err("f32 flatten height exceeds 2 cm tolerance; use localized mesh coordinates".into());
                        }
                    }
                    if self.flatten_height.is_none() && !self.region.contains(q, 0.02) {
                        return Err("f32 output exceeds 2 cm clip boundary tolerance; use localized mesh coordinates".into());
                    }
                    bounds = union(bounds, Some(Bounds::new(point(tile, p))));
                }
                vertices.extend(tri);
                if vertices.len() > 1_000_000 {
                    return Err("clipped primitive exceeds 1000000 vertices".into());
                }
            }
        }
        if vertices.is_empty() {
            return Ok(None);
        }
        self.counts.after += vertices.len() as u64 / 3;
        let mut attributes = serde_json::Map::new();
        for (name, (offset, width)) in layout {
            let kind = match width {
                2 => "VEC2",
                3 => "VEC3",
                _ => "VEC4",
            };
            let id = write_attribute(
                &mut out.document,
                &mut out.binary,
                &vertices,
                offset,
                width,
                kind,
            )?;
            attributes.insert(name, json!(id));
        }
        if out.binary.len() > 128 * 1024 * 1024 {
            return Err("clipped content exceeds 128 MiB".into());
        }
        let mut result = p.clone();
        result.as_object_mut().unwrap().remove("indices");
        result["attributes"] = Value::Object(attributes);
        result["mode"] = json!(4);
        Ok(Some((result, bounds.unwrap())))
    }
}
pub fn union(a: Option<Bounds>, b: Option<Bounds>) -> Option<Bounds> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (a, b) => a.or(b),
    }
}
