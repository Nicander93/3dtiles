//! Rust KTX2 post-process for 3D Tiles (Phase 14).
//!
//! Walks B3DM/GLB/glTF under a tileset directory, encodes JPEG/PNG textures with
//! the bundled `basisu` CLI, and rewrites glTF to `KHR_texture_basisu`.
//! No Python on the release path.

use serde_json::{json, Map, Value};
use std::collections::HashMap;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const KTX2_MIME: &str = "image/ktx2";
const EXT_NAME: &str = "KHR_texture_basisu";

#[derive(Debug, Default, Clone)]
pub struct Ktx2Stats {
    pub root: String,
    pub mode: String,
    pub basisu: String,
    pub files_seen: usize,
    pub files_converted: usize,
    pub textures_converted: usize,
    pub errors: Vec<String>,
}

impl Ktx2Stats {
    pub fn to_json(&self) -> Value {
        json!({
            "root": self.root,
            "mode": self.mode,
            "basisu": self.basisu,
            "filesSeen": self.files_seen,
            "filesConverted": self.files_converted,
            "texturesConverted": self.textures_converted,
            "errors": self.errors,
        })
    }
}

fn pad4(n: usize) -> usize {
    (4 - (n % 4)) % 4
}

fn align4(buf: &mut Vec<u8>) {
    let pad = pad4(buf.len());
    if pad > 0 {
        buf.extend(std::iter::repeat(0u8).take(pad));
    }
}

fn parse_b3dm(data: &[u8]) -> Result<(Vec<u8>, Vec<u8>), String> {
    if data.len() < 28 || &data[0..4] != b"b3dm" {
        return Err("not b3dm".into());
    }
    let ftj = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
    let ftb = u32::from_le_bytes(data[16..20].try_into().unwrap()) as usize;
    let btj = u32::from_le_bytes(data[20..24].try_into().unwrap()) as usize;
    let btb = u32::from_le_bytes(data[24..28].try_into().unwrap()) as usize;
    let mut offset = 28 + ftj + ftb + btj + btb;
    if offset + 4 > data.len() {
        return Err("b3dm truncated".into());
    }
    if &data[offset..offset + 4] != b"glTF" {
        let mut found = None;
        for pad in 0..8 {
            let o = offset + pad;
            if o + 4 <= data.len() && &data[o..o + 4] == b"glTF" {
                found = Some(o);
                break;
            }
        }
        offset = found.ok_or_else(|| "glb not found in b3dm".to_string())?;
    }
    Ok((data[..offset].to_vec(), data[offset..].to_vec()))
}

fn write_b3dm_preserving_header(path: &Path, header: &[u8], glb: &[u8]) -> Result<(), String> {
    let mut out = Vec::with_capacity(header.len() + glb.len());
    out.extend_from_slice(header);
    out.extend_from_slice(glb);
    let len = out.len() as u32;
    out[8..12].copy_from_slice(&len.to_le_bytes());
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    fs::write(path, out).map_err(|e| e.to_string())
}

fn parse_glb(glb: &[u8]) -> Result<(Value, Vec<u8>), String> {
    if glb.len() < 12 || &glb[0..4] != b"glTF" {
        return Err("not glb".into());
    }
    let version = u32::from_le_bytes(glb[4..8].try_into().unwrap());
    if version != 2 {
        return Err(format!("unsupported glTF version {version}"));
    }
    let length = u32::from_le_bytes(glb[8..12].try_into().unwrap()) as usize;
    let mut pos = 12usize;
    let mut json_data: Option<Value> = None;
    let mut bin_data = Vec::new();
    while pos + 8 <= glb.len() && pos < length {
        let clen = u32::from_le_bytes(glb[pos..pos + 4].try_into().unwrap()) as usize;
        let ctype = &glb[pos + 4..pos + 8];
        pos += 8;
        if pos + clen > glb.len() {
            break;
        }
        let chunk = &glb[pos..pos + clen];
        pos += clen;
        if ctype == b"JSON" {
            let s = std::str::from_utf8(chunk).map_err(|e| e.to_string())?;
            json_data = Some(serde_json::from_str(s.trim_end()).map_err(|e| e.to_string())?);
        } else if ctype.starts_with(b"BIN") {
            bin_data = chunk.to_vec();
        }
    }
    let json_data = json_data.ok_or_else(|| "GLB missing JSON chunk".to_string())?;
    Ok((json_data, bin_data))
}

fn build_glb(gltf: &Value, bin_data: &[u8]) -> Result<Vec<u8>, String> {
    let mut json_bytes = serde_json::to_vec(gltf).map_err(|e| e.to_string())?;
    let json_pad = pad4(json_bytes.len());
    json_bytes.extend(std::iter::repeat(b' ').take(json_pad));
    let bin_pad = pad4(bin_data.len());
    let mut bin_padded = bin_data.to_vec();
    bin_padded.extend(std::iter::repeat(0u8).take(bin_pad));
    let total = 12 + 8 + json_bytes.len() + 8 + bin_padded.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(json_bytes.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_bytes);
    out.extend_from_slice(&(bin_padded.len() as u32).to_le_bytes());
    out.extend_from_slice(b"BIN\0");
    out.extend_from_slice(&bin_padded);
    Ok(out)
}

fn encode_image_basisu(
    basisu: &Path,
    image_bytes: &[u8],
    mime: &str,
    mode: &str,
    work: &Path,
    idx: usize,
    quality: u32,
) -> Result<Vec<u8>, String> {
    let ext = if mime.contains("jpeg") || mime.contains("jpg") {
        ".jpg"
    } else if mime.contains("webp") {
        ".webp"
    } else {
        ".png"
    };
    let src = work.join(format!("img_{idx}{ext}"));
    let dst = work.join(format!("img_{idx}.ktx2"));
    fs::write(&src, image_bytes).map_err(|e| e.to_string())?;
    let mut cmd = Command::new(basisu);
    cmd.args([
        "-ktx2",
        "-file",
        src.to_str().unwrap_or("in"),
        "-output_file",
        dst.to_str().unwrap_or("out.ktx2"),
    ]);
    if mode == "ktx2-uastc" {
        cmd.args(["-uastc", "-uastc_level", "2"]);
    } else {
        cmd.args(["-etc1s", "-q", &quality.to_string()]);
    }
    let out = cmd.output().map_err(|e| format!("basisu spawn: {e}"))?;
    if !out.status.success() || !dst.is_file() {
        let err = String::from_utf8_lossy(&out.stderr);
        let stdout = String::from_utf8_lossy(&out.stdout);
        return Err(format!(
            "basisu failed rc={:?}: {}",
            out.status.code(),
            err.chars()
                .chain(stdout.chars())
                .rev()
                .take(500)
                .collect::<String>()
                .chars()
                .rev()
                .collect::<String>()
        ));
    }
    fs::read(&dst).map_err(|e| e.to_string())
}

fn sniff_mime(raw: &[u8]) -> &'static str {
    if raw.len() >= 3 && raw[0..3] == [0xff, 0xd8, 0xff] {
        "image/jpeg"
    } else if raw.len() >= 8 && raw[0..8] == *b"\x89PNG\r\n\x1a\n" {
        "image/png"
    } else {
        "image/jpeg"
    }
}

fn is_raster_mime(mime: &str) -> bool {
    let m = mime.to_ascii_lowercase();
    matches!(
        m.as_str(),
        "image/jpeg" | "image/jpg" | "image/png" | "image/webp"
    ) || m.starts_with("image/") && m != KTX2_MIME
}

fn rewrite_gltf_textures(
    mut gltf: Value,
    bin_data: &[u8],
    basisu: &Path,
    mode: &str,
    work: &Path,
    external_base: Option<&Path>,
    quality: u32,
) -> Result<(Value, Vec<u8>, usize), String> {
    let images = gltf
        .get("images")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if images.is_empty() {
        return Ok((gltf, bin_data.to_vec(), 0));
    }

    let buffer_views: Vec<Value> = gltf
        .get("bufferViews")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();

    let mut converted: HashMap<usize, Vec<u8>> = HashMap::new();
    for (i, img) in images.iter().enumerate() {
        let mut mime = img
            .get("mimeType")
            .and_then(|v| v.as_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if mime == KTX2_MIME {
            continue;
        }
        let mut raw: Option<Vec<u8>> = None;
        if let Some(bv_i) = img.get("bufferView").and_then(|v| v.as_u64()) {
            let bv = buffer_views
                .get(bv_i as usize)
                .ok_or_else(|| format!("bad bufferView {bv_i}"))?;
            let off = bv.get("byteOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let length = bv
                .get("byteLength")
                .and_then(|v| v.as_u64())
                .ok_or_else(|| "bufferView missing byteLength".to_string())?
                as usize;
            if off + length > bin_data.len() {
                return Err(format!("bufferView {bv_i} out of range"));
            }
            raw = Some(bin_data[off..off + length].to_vec());
            if mime.is_empty() {
                mime = sniff_mime(raw.as_ref().unwrap()).to_string();
            }
        } else if let (Some(uri), Some(base)) = (img.get("uri").and_then(|v| v.as_str()), external_base)
        {
            if uri.starts_with("data:") {
                continue;
            }
            let p = base.join(uri);
            if !p.is_file() {
                continue;
            }
            let bytes = fs::read(&p).map_err(|e| e.to_string())?;
            if mime.is_empty() {
                mime = match p.extension().and_then(|e| e.to_str()).unwrap_or("") {
                    "png" => "image/png".into(),
                    "jpg" | "jpeg" => "image/jpeg".into(),
                    "webp" => "image/webp".into(),
                    _ => sniff_mime(&bytes).to_string(),
                };
            }
            raw = Some(bytes);
        }
        let Some(raw) = raw else { continue };
        if !is_raster_mime(&mime) {
            continue;
        }
        converted.insert(
            i,
            encode_image_basisu(basisu, &raw, &mime, mode, work, i, quality)?,
        );
    }

    if converted.is_empty() {
        return Ok((gltf, bin_data.to_vec(), 0));
    }

    let mut image_bv_indices: HashMap<usize, usize> = HashMap::new();
    for (i, img) in images.iter().enumerate() {
        if converted.contains_key(&i) {
            if let Some(bv_i) = img.get("bufferView").and_then(|v| v.as_u64()) {
                image_bv_indices.insert(bv_i as usize, i);
            }
        }
    }

    let mut new_bin: Vec<u8> = Vec::new();
    let mut new_bvs: Vec<Value> = Vec::new();
    for (bi, bv) in buffer_views.iter().enumerate() {
        let mut nbv = bv.as_object().cloned().unwrap_or_default();
        if let Some(&img_i) = image_bv_indices.get(&bi) {
            let ktx = &converted[&img_i];
            align4(&mut new_bin);
            nbv.insert("byteOffset".into(), json!(new_bin.len()));
            nbv.insert("byteLength".into(), json!(ktx.len()));
            new_bin.extend_from_slice(ktx);
        } else {
            let off = bv.get("byteOffset").and_then(|v| v.as_u64()).unwrap_or(0) as usize;
            let length = bv
                .get("byteLength")
                .and_then(|v| v.as_u64())
                .unwrap_or(0) as usize;
            let chunk = if off + length <= bin_data.len() {
                &bin_data[off..off + length]
            } else {
                &[]
            };
            align4(&mut new_bin);
            nbv.insert("byteOffset".into(), json!(new_bin.len()));
            nbv.insert("byteLength".into(), json!(chunk.len()));
            new_bin.extend_from_slice(chunk);
        }
        new_bvs.push(Value::Object(nbv));
    }

    let mut new_images = images;
    for (&i, ktx) in &converted {
        let img = &new_images[i];
        if img.get("bufferView").is_some() {
            let bv = img.get("bufferView").cloned().unwrap();
            new_images[i] = json!({ "mimeType": KTX2_MIME, "bufferView": bv });
        } else {
            align4(&mut new_bin);
            let bv_index = new_bvs.len();
            new_bvs.push(json!({
                "buffer": 0,
                "byteOffset": new_bin.len(),
                "byteLength": ktx.len()
            }));
            new_bin.extend_from_slice(ktx);
            new_images[i] = json!({ "mimeType": KTX2_MIME, "bufferView": bv_index });
        }
    }

    gltf["images"] = Value::Array(new_images.clone());
    gltf["bufferViews"] = Value::Array(new_bvs);

    let mut buffers = gltf
        .get("buffers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_else(|| vec![json!({ "byteLength": 0 })]);
    if buffers.is_empty() {
        buffers.push(json!({ "byteLength": 0 }));
    }
    let mut b0 = buffers[0].as_object().cloned().unwrap_or_default();
    b0.insert("byteLength".into(), json!(new_bin.len()));
    b0.remove("uri");
    buffers[0] = Value::Object(b0);
    gltf["buffers"] = Value::Array(buffers);

    // textures: KHR_texture_basisu
    let textures = gltf
        .get("textures")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut new_textures = Vec::new();
    for tex in textures {
        let src = tex.get("source").and_then(|v| v.as_u64()).map(|u| u as usize);
        let Some(src) = src else {
            new_textures.push(tex);
            continue;
        };
        let img_mime = new_images
            .get(src)
            .and_then(|i| i.get("mimeType"))
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if !converted.contains_key(&src) && img_mime != KTX2_MIME {
            new_textures.push(tex);
            continue;
        }
        let mut ntex: Map<String, Value> = tex
            .as_object()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter(|(k, _)| k != "source")
            .collect();
        let mut exts = ntex
            .get("extensions")
            .and_then(|v| v.as_object())
            .cloned()
            .unwrap_or_default();
        exts.insert(EXT_NAME.into(), json!({ "source": src }));
        ntex.insert("extensions".into(), Value::Object(exts));
        // Keep source for broader loader compatibility (matches top_rebuild writer).
        ntex.insert("source".into(), json!(src));
        new_textures.push(Value::Object(ntex));
    }
    gltf["textures"] = Value::Array(new_textures);

    let mut used = gltf
        .get("extensionsUsed")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    let mut req = gltf
        .get("extensionsRequired")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if !used.iter().any(|e| e.as_str() == Some(EXT_NAME)) {
        used.push(json!(EXT_NAME));
    }
    if !req.iter().any(|e| e.as_str() == Some(EXT_NAME)) {
        req.push(json!(EXT_NAME));
    }
    gltf["extensionsUsed"] = Value::Array(used);
    gltf["extensionsRequired"] = Value::Array(req);

    Ok((gltf, new_bin, converted.len()))
}

fn process_glb_bytes(
    glb: &[u8],
    basisu: &Path,
    mode: &str,
    work: &Path,
    quality: u32,
) -> Result<(Vec<u8>, usize), String> {
    let (gltf, bin_data) = parse_glb(glb)?;
    let (gltf2, bin2, n) =
        rewrite_gltf_textures(gltf, &bin_data, basisu, mode, work, None, quality)?;
    if n == 0 {
        return Ok((glb.to_vec(), 0));
    }
    Ok((build_glb(&gltf2, &bin2)?, n))
}

fn process_b3dm_file(
    path: &Path,
    basisu: &Path,
    mode: &str,
    work: &Path,
    quality: u32,
) -> Result<usize, String> {
    let data = fs::read(path).map_err(|e| e.to_string())?;
    let (header, glb) = parse_b3dm(&data)?;
    let (new_glb, n) = process_glb_bytes(&glb, basisu, mode, work, quality)?;
    if n == 0 {
        return Ok(0);
    }
    write_b3dm_preserving_header(path, &header, &new_glb)?;
    Ok(n)
}

fn process_glb_file(
    path: &Path,
    basisu: &Path,
    mode: &str,
    work: &Path,
    quality: u32,
) -> Result<usize, String> {
    let glb = fs::read(path).map_err(|e| e.to_string())?;
    let (new_glb, n) = process_glb_bytes(&glb, basisu, mode, work, quality)?;
    if n == 0 {
        return Ok(0);
    }
    fs::write(path, new_glb).map_err(|e| e.to_string())?;
    Ok(n)
}

fn process_gltf_file(
    path: &Path,
    basisu: &Path,
    mode: &str,
    work: &Path,
    quality: u32,
) -> Result<usize, String> {
    let text = fs::read_to_string(path).map_err(|e| e.to_string())?;
    let gltf: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let mut bin_data = Vec::new();
    let buffers = gltf
        .get("buffers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if let Some(uri) = buffers
        .first()
        .and_then(|b| b.get("uri"))
        .and_then(|v| v.as_str())
    {
        if !uri.starts_with("data:") {
            let bin_path = path.parent().unwrap_or(Path::new(".")).join(uri);
            if bin_path.is_file() {
                bin_data = fs::read(&bin_path).map_err(|e| e.to_string())?;
            }
        }
    }
    let parent = path.parent().map(|p| p.to_path_buf());
    let (gltf2, bin2, n) = rewrite_gltf_textures(
        gltf,
        &bin_data,
        basisu,
        mode,
        work,
        parent.as_deref(),
        quality,
    )?;
    if n == 0 {
        return Ok(0);
    }
    let bin_name = path.with_extension("bin").file_name().unwrap().to_owned();
    let bin_name_str = bin_name.to_string_lossy().into_owned();
    let mut gltf2 = gltf2;
    let buffers = gltf2
        .get("buffers")
        .and_then(|v| v.as_array())
        .cloned()
        .unwrap_or_default();
    if buffers.is_empty() {
        gltf2["buffers"] = json!([{ "byteLength": bin2.len(), "uri": bin_name_str }]);
    } else {
        let mut b0 = buffers[0].as_object().cloned().unwrap_or_default();
        let uri = b0
            .get("uri")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string())
            .unwrap_or(bin_name_str);
        b0.insert("uri".into(), json!(uri.clone()));
        b0.insert("byteLength".into(), json!(bin2.len()));
        let mut bufs = buffers;
        bufs[0] = Value::Object(b0);
        gltf2["buffers"] = Value::Array(bufs);
        fs::write(path.parent().unwrap_or(Path::new(".")).join(&uri), &bin2)
            .map_err(|e| e.to_string())?;
        let pretty = serde_json::to_string_pretty(&gltf2).map_err(|e| e.to_string())?;
        fs::write(path, pretty).map_err(|e| e.to_string())?;
        return Ok(n);
    }
    fs::write(
        path.parent().unwrap_or(Path::new(".")).join(&bin_name),
        &bin2,
    )
    .map_err(|e| e.to_string())?;
    let pretty = serde_json::to_string_pretty(&gltf2).map_err(|e| e.to_string())?;
    fs::write(path, pretty).map_err(|e| e.to_string())?;
    Ok(n)
}

fn collect_content_files(root: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    fn walk(d: &Path, depth: u32, out: &mut Vec<PathBuf>) {
        if depth > 24 {
            return;
        }
        let Ok(rd) = fs::read_dir(d) else {
            return;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, depth + 1, out);
            } else if let Some(ext) = p.extension().and_then(|x| x.to_str()) {
                let e = ext.to_ascii_lowercase();
                if matches!(e.as_str(), "b3dm" | "glb" | "gltf") {
                    out.push(p);
                }
            }
        }
    }
    walk(root, 0, &mut files);
    files.sort();
    files
}

/// In-place KTX2 post-process under a 3D Tiles directory (release path).
pub fn process_tileset_dir(
    root: &Path,
    mode: &str,
    basisu: &Path,
    quality: u32,
    mut log: impl FnMut(&str),
) -> Result<Ktx2Stats, String> {
    if !basisu.is_file() {
        return Err(format!(
            "basisu not found: {} (set GEOFORGE_BASISU or bundle sidecar)",
            basisu.display()
        ));
    }
    let mut mode = mode.to_ascii_lowercase();
    if matches!(mode.as_str(), "ktx2" | "etc1s") {
        mode = "ktx2-etc1s".into();
    }
    let mut stats = Ktx2Stats {
        root: root.display().to_string(),
        mode: mode.clone(),
        basisu: basisu.display().to_string(),
        ..Default::default()
    };
    let files = collect_content_files(root);
    stats.files_seen = files.len();
    let work = tempfile_work()?;
    for fp in &files {
        let rel = fp.strip_prefix(root).unwrap_or(fp);
        let result = match fp.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase().as_str()
        {
            "b3dm" => process_b3dm_file(fp, basisu, &mode, &work, quality),
            "glb" => process_glb_file(fp, basisu, &mode, &work, quality),
            "gltf" => process_gltf_file(fp, basisu, &mode, &work, quality),
            _ => Ok(0),
        };
        match result {
            Ok(n) if n > 0 => {
                stats.files_converted += 1;
                stats.textures_converted += n;
                log(&format!(
                    "[texture_ktx2] {} textures={n}",
                    rel.display()
                ));
            }
            Ok(_) => {}
            Err(e) => {
                stats.errors.push(format!("{}: {e}", fp.display()));
                log(&format!("[texture_ktx2] ERROR {}: {e}", rel.display()));
            }
        }
    }
    let _ = fs::remove_dir_all(&work);
    Ok(stats)
}

fn tempfile_work() -> Result<PathBuf, String> {
    let dir = std::env::temp_dir().join(format!(
        "geoforge_ktx2_{}_{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    ));
    fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    // Touch so empty dir exists for basisu
    let marker = dir.join(".keep");
    let mut f = fs::File::create(&marker).map_err(|e| e.to_string())?;
    let _ = f.write_all(b"ok");
    Ok(dir)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pad_and_align() {
        assert_eq!(pad4(0), 0);
        assert_eq!(pad4(1), 3);
        let mut v = vec![1u8, 2, 3];
        align4(&mut v);
        assert_eq!(v.len() % 4, 0);
    }

    #[test]
    fn build_parse_glb_roundtrip_empty_images() {
        let gltf = json!({
            "asset": {"version": "2.0"},
            "buffers": [{"byteLength": 4}],
            "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 4}],
            "images": []
        });
        let bin = vec![1u8, 2, 3, 4];
        let glb = build_glb(&gltf, &bin).unwrap();
        let (g2, b2) = parse_glb(&glb).unwrap();
        assert_eq!(b2, bin);
        assert_eq!(g2["asset"]["version"], "2.0");
    }
}
