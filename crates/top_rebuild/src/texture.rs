//! Texture path for proxies (plan §16, Phase 8).
//!
//! - Hash dedup (SHA-256 of decoded RGBA)
//! - Resize to `maxTextureSize`
//! - Enforce `maxTextureBytes` (iterative downscale + warnings)
//! - Optional KTX2 via `basisu` CLI (honest if unavailable)
//!
//! V1: no atlas / UV remap / baking. Original UVs preserved on mesh.

use crate::error::{Result, TopRebuildError};
use image::imageops::FilterType;
use image::{DynamicImage, ImageBuffer, ImageFormat, RgbaImage};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

/// Raw decoded texture carried through the proxy pipeline.
#[derive(Clone, Debug)]
pub struct TextureData {
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
    pub hash: String,
    /// When set, bytes are already encoded (e.g. KTX2) — pass through without re-encode.
    pub opaque_encoded: Option<(String, Vec<u8>)>,
}

impl TextureData {
    pub fn from_rgba(width: u32, height: u32, rgba: Vec<u8>) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(width.to_le_bytes());
        hasher.update(height.to_le_bytes());
        hasher.update(&rgba);
        let hash = hex::encode(hasher.finalize());
        Self {
            width,
            height,
            rgba,
            hash,
            opaque_encoded: None,
        }
    }

    /// Opaque pass-through (KTX2 etc.) — hash of encoded bytes; 1×1 placeholder rgba.
    pub fn from_opaque_encoded(mime: &str, bytes: &[u8]) -> Self {
        let mut hasher = Sha256::new();
        hasher.update(mime.as_bytes());
        hasher.update(bytes);
        let hash = hex::encode(hasher.finalize());
        Self {
            width: 1,
            height: 1,
            rgba: vec![0, 0, 0, 0],
            hash,
            opaque_encoded: Some((mime.to_string(), bytes.to_vec())),
        }
    }

    pub fn from_encoded_bytes(bytes: &[u8]) -> Result<Self> {
        let img = image::load_from_memory(bytes)
            .map_err(|e| TopRebuildError::Other(format!("image decode: {e}")))?;
        let rgba = img.to_rgba8();
        let (w, h) = rgba.dimensions();
        Ok(Self::from_rgba(w, h, rgba.into_raw()))
    }

    pub fn solid(r: u8, g: u8, b: u8, a: u8, size: u32) -> Self {
        let s = size.max(1);
        let mut rgba = Vec::with_capacity((s * s * 4) as usize);
        for _ in 0..(s * s) {
            rgba.extend_from_slice(&[r, g, b, a]);
        }
        Self::from_rgba(s, s, rgba)
    }

    pub fn byte_len_rgba(&self) -> u64 {
        self.rgba.len() as u64
    }
}

/// One texture after budget processing, ready to embed in GLB.
#[derive(Clone, Debug)]
pub struct ProcessedTexture {
    /// Pixel hash after resize/encode pipeline.
    pub hash: String,
    /// Original input hash (before resize) for remapping mesh material refs.
    pub source_hash: String,
    pub width: u32,
    pub height: u32,
    /// Encoded image bytes (PNG or KTX2).
    pub bytes: Vec<u8>,
    pub mime: String,
    pub ktx2: bool,
}

#[derive(Clone, Debug, Default)]
pub struct TextureMetrics {
    pub input_count: usize,
    pub unique_count: usize,
    pub dedup_removed: usize,
    pub total_bytes_before: u64,
    pub total_bytes_after: u64,
    pub max_dimension_after: u32,
    pub ktx2_available: bool,
    pub ktx2_encoded: usize,
    pub basisu_path: Option<String>,
    pub warnings: Vec<String>,
}

/// Locate `basisu` (GEOFORGE_BASISU → sidecar next to exe → PATH → vcpkg_installed).
/// Phase 14: `/workspace/...` is only a last-resort developer probe, not required.
pub fn find_basisu() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GEOFORGE_BASISU") {
        let pb = PathBuf::from(&p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            for name in ["basisu", "basisu.exe"] {
                for sub in [
                    PathBuf::from(name),
                    PathBuf::from("resources/bin").join(name),
                    PathBuf::from("bin").join(name),
                    PathBuf::from("resources").join(name),
                ] {
                    let cand = dir.join(&sub);
                    if cand.is_file() {
                        return Some(cand);
                    }
                }
            }
        }
    }
    if let Ok(out) = Command::new("which").arg("basisu").output() {
        if out.status.success() {
            let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
            if !s.is_empty() {
                let pb = PathBuf::from(s);
                if pb.is_file() {
                    return Some(pb);
                }
            }
        }
    }
    let candidates = [
        "vcpkg_installed/x64-linux/tools/basisu/basisu",
        "../vcpkg_installed/x64-linux/tools/basisu/basisu",
        "../../vcpkg_installed/x64-linux/tools/basisu/basisu",
        // Developer probe only (not a release requirement):
        "/workspace/repos/3dtiles/vcpkg_installed/x64-linux/tools/basisu/basisu",
    ];
    for c in candidates {
        let pb = PathBuf::from(c);
        if pb.is_file() {
            return Some(pb);
        }
    }
    let mut cur = std::env::current_dir().ok()?;
    for _ in 0..6 {
        let p = cur.join("vcpkg_installed/x64-linux/tools/basisu/basisu");
        if p.is_file() {
            return Some(p);
        }
        if !cur.pop() {
            break;
        }
    }
    None
}

fn encode_png(tex: &TextureData) -> Result<Vec<u8>> {
    let img: RgbaImage = ImageBuffer::from_raw(tex.width, tex.height, tex.rgba.clone())
        .ok_or_else(|| TopRebuildError::Other("rgba buffer size mismatch".into()))?;
    let mut buf = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(img)
        .write_to(&mut buf, ImageFormat::Png)
        .map_err(|e| TopRebuildError::Other(format!("png encode: {e}")))?;
    Ok(buf.into_inner())
}

fn resize_to_max(tex: &TextureData, max_size: u32) -> TextureData {
    let max_size = max_size.max(1);
    if tex.width <= max_size && tex.height <= max_size {
        return tex.clone();
    }
    let scale = (max_size as f32 / tex.width.max(tex.height) as f32).min(1.0);
    let nw = ((tex.width as f32 * scale).round() as u32).max(1);
    let nh = ((tex.height as f32 * scale).round() as u32).max(1);
    let img: RgbaImage = ImageBuffer::from_raw(tex.width, tex.height, tex.rgba.clone())
        .expect("rgba");
    let resized = image::imageops::resize(&img, nw, nh, FilterType::Triangle);
    TextureData::from_rgba(nw, nh, resized.into_raw())
}

fn encode_ktx2(basisu: &Path, png_bytes: &[u8], work: &Path) -> Result<Vec<u8>> {
    std::fs::create_dir_all(work)?;
    let png_path = work.join("in.png");
    let out_path = work.join("out.ktx2");
    std::fs::write(&png_path, png_bytes)?;
    let status = Command::new(basisu)
        .args([
            "-ktx2",
            "-etc1s",
            "-file",
            png_path.to_str().unwrap_or("in.png"),
            "-output_file",
            out_path.to_str().unwrap_or("out.ktx2"),
        ])
        .status()
        .map_err(|e| TopRebuildError::Other(format!("basisu spawn: {e}")))?;
    if !status.success() {
        return Err(TopRebuildError::Other(format!(
            "basisu failed with status {status}"
        )));
    }
    let bytes = std::fs::read(&out_path)?;
    if bytes.len() < 12 {
        return Err(TopRebuildError::Other("basisu produced empty ktx2".into()));
    }
    Ok(bytes)
}

/// Dedup + resize + optional KTX2. Returns processed textures keyed by original hash
/// (after resize the hash of pixels may change; we keep `source_hash` → processed map
/// via the returned list ordered stably by hash).
pub fn process_textures(
    inputs: &[TextureData],
    max_texture_size: u32,
    max_texture_bytes: u64,
    enable_ktx2: bool,
    work_dir: Option<&Path>,
) -> Result<(Vec<ProcessedTexture>, TextureMetrics)> {
    let mut metrics = TextureMetrics {
        input_count: inputs.len(),
        ..Default::default()
    };

    // Dedup by hash
    let mut unique: BTreeMap<String, TextureData> = BTreeMap::new();
    for t in inputs {
        unique.entry(t.hash.clone()).or_insert_with(|| t.clone());
    }
    metrics.unique_count = unique.len();
    metrics.dedup_removed = metrics.input_count.saturating_sub(metrics.unique_count);
    metrics.total_bytes_before = unique.values().map(|t| t.byte_len_rgba()).sum();

    let basisu = find_basisu();
    metrics.ktx2_available = basisu.is_some();
    metrics.basisu_path = basisu.as_ref().map(|p| p.display().to_string());
    if enable_ktx2 && basisu.is_none() {
        metrics.warnings.push(
            "KTX2 requested but basisu CLI not found (checked PATH, GEOFORGE_BASISU, vcpkg_installed); keeping PNG"
                .into(),
        );
    }

    // First pass: resize to max_texture_size (keep source hash); opaque KTX2 pass-through
    let mut sized: Vec<(String, TextureData)> = unique
        .into_iter()
        .map(|(src_hash, t)| {
            if t.opaque_encoded.is_some() {
                (src_hash, t)
            } else {
                (src_hash, resize_to_max(&t, max_texture_size))
            }
        })
        .collect();

    // Enforce maxTextureBytes on sum of PNG-encoded sizes
    let mut scale_steps = 0u32;
    loop {
        let mut png_total = 0u64;
        for (_src, t) in &sized {
            if let Some((_, ref enc)) = t.opaque_encoded {
                png_total += enc.len() as u64;
            } else {
                let png = encode_png(t)?;
                png_total += png.len() as u64;
            }
        }
        if max_texture_bytes == 0 || png_total <= max_texture_bytes || scale_steps >= 8 {
            if max_texture_bytes > 0 && png_total > max_texture_bytes {
                metrics.warnings.push(format!(
                    "TEXTURE_BYTES_OVER_BUDGET: after={png_total} max={max_texture_bytes} (after {scale_steps} downscales)"
                ));
            }
            break;
        }
        sized = sized
            .iter()
            .map(|(src, t)| {
                if t.opaque_encoded.is_some() {
                    (src.clone(), t.clone())
                } else {
                    let nw = ((t.width as f32 * 0.7).round() as u32).max(1);
                    let nh = ((t.height as f32 * 0.7).round() as u32).max(1);
                    let max_dim = nw.max(nh);
                    (src.clone(), resize_to_max(t, max_dim))
                }
            })
            .collect();
        scale_steps += 1;
        metrics.warnings.push(format!(
            "texture downscale step {scale_steps}: png_total was {png_total} > max {max_texture_bytes}"
        ));
    }

    let tmp = work_dir
        .map(|p| p.join("_ktx2_work"))
        .unwrap_or_else(|| std::env::temp_dir().join(format!("top_rebuild_ktx2_{}", std::process::id())));

    let mut out = Vec::new();
    for (source_hash, t) in &sized {
        if let Some((ref mime, ref enc)) = t.opaque_encoded {
            let processed = ProcessedTexture {
                hash: t.hash.clone(),
                source_hash: source_hash.clone(),
                width: t.width,
                height: t.height,
                bytes: enc.clone(),
                mime: mime.clone(),
                ktx2: mime.contains("ktx2"),
            };
            if processed.ktx2 {
                metrics.ktx2_encoded += 1;
            }
            metrics.max_dimension_after = metrics
                .max_dimension_after
                .max(processed.width.max(processed.height));
            metrics.total_bytes_after += processed.bytes.len() as u64;
            out.push(processed);
            continue;
        }
        let png = encode_png(t)?;
        let mut processed = ProcessedTexture {
            hash: t.hash.clone(),
            source_hash: source_hash.clone(),
            width: t.width,
            height: t.height,
            bytes: png.clone(),
            mime: "image/png".into(),
            ktx2: false,
        };
        if enable_ktx2 {
            if let Some(ref b) = basisu {
                let slot = tmp.join(&t.hash[..16.min(t.hash.len())]);
                match encode_ktx2(b, &png, &slot) {
                    Ok(ktx) => {
                        processed.bytes = ktx;
                        processed.mime = "image/ktx2".into();
                        processed.ktx2 = true;
                        metrics.ktx2_encoded += 1;
                    }
                    Err(e) => {
                        metrics.warnings.push(format!(
                            "KTX2 encode failed for {}: {e}; keeping PNG",
                            &t.hash[..8.min(t.hash.len())]
                        ));
                    }
                }
            }
        }
        metrics.max_dimension_after = metrics
            .max_dimension_after
            .max(processed.width.max(processed.height));
        metrics.total_bytes_after += processed.bytes.len() as u64;
        out.push(processed);
    }

    // Stable order by hash
    out.sort_by(|a, b| a.hash.cmp(&b.hash));
    Ok((out, metrics))
}

/// Extract embedded images from a GLB (PNG/JPEG). Returns textures + map of
/// glTF image index → hash.
pub fn extract_textures_from_glb(glb: &[u8]) -> Result<(Vec<TextureData>, BTreeMap<usize, String>)> {
    let gltf = crate::glb::parse_gltf_lenient(glb)?;
    let blob = gltf.blob.as_ref();
    let mut textures = Vec::new();
    let mut index_to_hash = BTreeMap::new();

    for (i, image) in gltf.document.images().enumerate() {
        let bytes: Option<Vec<u8>> = match image.source() {
            gltf::image::Source::View { view, .. } => {
                let blob = blob.ok_or_else(|| TopRebuildError::Other("GLB missing BIN".into()))?;
                let start = view.offset();
                let end = start + view.length();
                if end > blob.len() {
                    return Err(TopRebuildError::Other("image bufferView OOB".into()));
                }
                Some(blob[start..end].to_vec())
            }
            gltf::image::Source::Uri { .. } => {
                // External / data-URI images skipped in V1 (fixtures use bufferView).
                None
            }
        };
        if let Some(raw) = bytes {
            let mime: Option<String> = match image.source() {
                gltf::image::Source::View { mime_type, .. } => Some(mime_type.to_string()),
                gltf::image::Source::Uri { mime_type, .. } => mime_type.map(|s| s.to_string()),
            };
            let is_ktx2 = mime.as_deref() == Some("image/ktx2")
                || (raw.len() > 12 && raw[1] == b'K' && raw[2] == b'T' && raw[3] == b'X');
            if is_ktx2 {
                let tex = TextureData::from_opaque_encoded("image/ktx2", &raw);
                index_to_hash.insert(i, tex.hash.clone());
                textures.push(tex);
            } else {
                match TextureData::from_encoded_bytes(&raw) {
                    Ok(tex) => {
                        index_to_hash.insert(i, tex.hash.clone());
                        textures.push(tex);
                    }
                    Err(_) => {}
                }
            }
        }
    }
    Ok((textures, index_to_hash))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hash_dedup_identical_solids() {
        let a = TextureData::solid(255, 0, 0, 255, 16);
        let b = TextureData::solid(255, 0, 0, 255, 16);
        assert_eq!(a.hash, b.hash);
        let (proc, m) = process_textures(&[a, b], 64, 0, false, None).unwrap();
        assert_eq!(m.unique_count, 1);
        assert_eq!(m.dedup_removed, 1);
        assert_eq!(proc.len(), 1);
        assert_eq!(proc[0].mime, "image/png");
    }

    #[test]
    fn resize_respects_max_texture_size() {
        let big = TextureData::solid(0, 128, 255, 255, 256);
        let (proc, m) = process_textures(&[big], 64, 0, false, None).unwrap();
        assert!(proc[0].width <= 64 && proc[0].height <= 64);
        assert!(m.max_dimension_after <= 64);
    }

    #[test]
    fn find_basisu_may_exist() {
        // Honest: either found under vcpkg or not — just shouldn't panic
        let _ = find_basisu();
    }
}
