//! Texture path for proxies (plan §16, Phase 8).
//!
//! - Hash dedup (SHA-256 of decoded RGBA)
//! - Resize to `maxTextureSize`
//! - Enforce `maxTextureBytes` (iterative downscale + warnings)
//! - Optional KTX2 encode/decode via `basisu` CLI (honest if unavailable)
//! - Input `image/ktx2` is unpacked with basisu (`GEOFORGE_BASISU` / PATH / vcpkg)
//! - Windows packaging: spawn PATH includes `basisu` dir and sibling `../bin`
//!   (MSVC CRT). Unpack failure → opaque `image/ktx2` pass-through (no fake pixels).
//!
//! V1: no atlas / UV remap / baking. Original UVs preserved on mesh.

use crate::error::{Result, TopRebuildError};
use image::imageops::FilterType;
use image::{DynamicImage, ImageBuffer, ImageFormat, RgbaImage};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::ffi::OsString;
use std::io::Cursor;
use std::path::{Path, PathBuf};
use std::process::Command;

fn hide_console_window(command: &mut Command) {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    #[cfg(not(windows))]
    let _ = command;
}


/// Directories to prepend to `PATH` when spawning `basisu`.
///
/// GeoForge Windows layout keeps `basisu.exe` under `resources/runtime/texture/`
/// while MSVC CRT DLLs (`vcruntime140.dll`, …) live in sibling
/// `resources/runtime/bin/`. Without `bin` on `PATH`, spawn exits
/// `0xc0000135` (STATUS_DLL_NOT_FOUND).
///
/// Always includes the basisu parent directory; adds `../bin` when that
/// directory exists.
pub fn basisu_path_prefixes(basisu: &Path) -> Vec<PathBuf> {
    let mut dirs = Vec::new();
    let Some(parent) = basisu.parent() else {
        return dirs;
    };
    dirs.push(parent.to_path_buf());
    let sibling_bin = parent.join("..").join("bin");
    if sibling_bin.is_dir() {
        dirs.push(
            std::fs::canonicalize(&sibling_bin).unwrap_or_else(|_| sibling_bin),
        );
    }
    dirs
}

/// Build a `PATH` value with [`basisu_path_prefixes`] prepended to the process PATH.
pub fn path_with_basisu_dirs(basisu: &Path) -> OsString {
    let mut entries = basisu_path_prefixes(basisu);
    if let Some(existing) = std::env::var_os("PATH") {
        entries.extend(std::env::split_paths(&existing));
    }
    std::env::join_paths(&entries).unwrap_or_else(|_| {
        std::env::var_os("PATH").unwrap_or_default()
    })
}

/// Configure a `basisu` [`Command`]: PATH for CRT DLLs, cwd, hide console.
///
/// - Encode: pass `cwd = None` so cwd becomes the basisu parent (absolute I/O).
/// - Unpack: pass a writable temp `cwd` — basisu writes PNGs to cwd, and
///   packaged `runtime/texture` may be read-only. PATH still includes basisu
///   parent and sibling `../bin` (Windows CRT layout; fixes `0xc0000135`).
fn configure_basisu_command(command: &mut Command, basisu: &Path, cwd: Option<&Path>) {
    let run_cwd = cwd.or_else(|| basisu.parent());
    if let Some(dir) = run_cwd {
        command.current_dir(dir);
    }
    command.env("PATH", path_with_basisu_dirs(basisu));
    hide_console_window(command);
}

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

/// Locate `basisu` for KTX2 encode/decode.
///
/// Search order:
/// 1. `GEOFORGE_BASISU` (absolute path to the binary; preferred on Windows packaging)
/// 2. `basisu` / `basisu.exe` on `PATH` (`which` / bare spawn)
/// 3. `vcpkg_installed/*/tools/basisu/basisu[.exe]` relative to cwd / known roots
pub fn find_basisu() -> Option<PathBuf> {
    if let Ok(p) = std::env::var("GEOFORGE_BASISU") {
        let pb = PathBuf::from(&p);
        if pb.is_file() {
            return Some(pb);
        }
    }
    for cmd in ["which", "where"] {
        let mut lookup = Command::new(cmd);
        lookup.arg("basisu");
        hide_console_window(&mut lookup);
        if let Ok(out) = lookup.output() {
            if out.status.success() {
                let s = String::from_utf8_lossy(&out.stdout)
                    .lines()
                    .next()
                    .unwrap_or("")
                    .trim()
                    .to_string();
                if !s.is_empty() {
                    let pb = PathBuf::from(s);
                    if pb.is_file() {
                        return Some(pb);
                    }
                }
            }
        }
    }
    let candidates = [
        "vcpkg_installed/x64-linux/tools/basisu/basisu",
        "vcpkg_installed/x64-windows/tools/basisu/basisu.exe",
        "vcpkg_installed/x64-windows/tools/basisu/basisu",
        "../vcpkg_installed/x64-linux/tools/basisu/basisu",
        "../../vcpkg_installed/x64-linux/tools/basisu/basisu",
        "/workspace/repos/3dtiles/vcpkg_installed/x64-linux/tools/basisu/basisu",
    ];
    for c in candidates {
        let pb = PathBuf::from(c);
        if pb.is_file() {
            return Some(pb);
        }
    }
    // Walk up from cwd looking for vcpkg-installed basisu
    let mut cur = std::env::current_dir().ok()?;
    for _ in 0..6 {
        for rel in [
            "vcpkg_installed/x64-linux/tools/basisu/basisu",
            "vcpkg_installed/x64-windows/tools/basisu/basisu.exe",
            "vcpkg_installed/x64-windows/tools/basisu/basisu",
        ] {
            let p = cur.join(rel);
            if p.is_file() {
                return Some(p);
            }
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
    let img: RgbaImage =
        ImageBuffer::from_raw(tex.width, tex.height, tex.rgba.clone()).expect("rgba");
    let resized = image::imageops::resize(&img, nw, nh, FilterType::Triangle);
    TextureData::from_rgba(nw, nh, resized.into_raw())
}

fn encode_ktx2(basisu: &Path, png_bytes: &[u8], work: &Path) -> Result<Vec<u8>> {
    std::fs::create_dir_all(work)?;
    let png_path = work.join("in.png");
    let out_path = work.join("out.ktx2");
    std::fs::write(&png_path, png_bytes)?;
    let basisu = std::fs::canonicalize(basisu).unwrap_or_else(|_| basisu.to_path_buf());
    // Absolute I/O paths: configure_basisu_command sets cwd to basisu parent.
    let png_abs = std::fs::canonicalize(&png_path).unwrap_or(png_path);
    let out_abs = out_path.clone();
    let mut command = Command::new(&basisu);
    command.args([
        "-ktx2",
        "-etc1s",
        "-file",
        png_abs.to_str().unwrap_or("in.png"),
        "-output_file",
        out_abs.to_str().unwrap_or("out.ktx2"),
    ]);
    configure_basisu_command(&mut command, &basisu, None);
    let status = command
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

fn is_ktx2_payload(mime: Option<&str>, raw: &[u8]) -> bool {
    mime == Some("image/ktx2")
        || (raw.len() >= 12 && raw[1] == b'K' && raw[2] == b'T' && raw[3] == b'X')
}

/// Try basisu `-unpack` → RGBA. On any failure, returns an error (caller may
/// fall back to opaque pass-through).
fn decode_ktx2_via_basisu(ktx2_bytes: &[u8]) -> Result<TextureData> {
    let basisu = find_basisu().ok_or_else(|| {
        TopRebuildError::ContentUnreadable(
            "TEXTURE_INVALID: image/ktx2 requires basisu CLI (set GEOFORGE_BASISU to the binary, or install basisu on PATH / vcpkg_installed)".into(),
        )
    })?;
    // Absolute path: relative vcpkg paths break once cwd changes.
    let basisu = std::fs::canonicalize(&basisu).unwrap_or(basisu);

    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let id = COUNTER.fetch_add(1, Ordering::Relaxed);
    let work = std::env::temp_dir().join(format!(
        "top_rebuild_ktx2_dec_{}_{}",
        std::process::id(),
        id
    ));
    let _ = std::fs::remove_dir_all(&work);
    std::fs::create_dir_all(&work).map_err(|e| {
        TopRebuildError::ContentUnreadable(format!("TEXTURE_INVALID: temp dir: {e}"))
    })?;
    let in_path = work.join("in.ktx2");
    std::fs::write(&in_path, ktx2_bytes).map_err(|e| {
        TopRebuildError::ContentUnreadable(format!("TEXTURE_INVALID: write ktx2: {e}"))
    })?;

            // Writable temp cwd (basisu writes PNGs here). PATH includes basisu parent
    // + sibling ../bin so Windows CRT DLLs resolve (packaging layout).
    let mut command = Command::new(&basisu);
    command.args(["-unpack", "-file", "in.ktx2", "-no_ktx", "-etc1_only"]);
    configure_basisu_command(&mut command, &basisu, Some(&work));


    let output = command.output().map_err(|e| {
        TopRebuildError::ContentUnreadable(format!("TEXTURE_INVALID: basisu unpack spawn: {e}"))
    })?;
    if !output.status.success() {
        let _ = std::fs::remove_dir_all(&work);
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        return Err(TopRebuildError::ContentUnreadable(format!(
            "TEXTURE_INVALID: basisu unpack failed ({status}): {stderr} {stdout}",
            status = output.status
        )));
    }

    let png_path = std::fs::read_dir(&work)
        .map_err(|e| {
            TopRebuildError::ContentUnreadable(format!("TEXTURE_INVALID: read unpack dir: {e}"))
        })?
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.extension().and_then(|ext| ext.to_str()) == Some("png")
                && p.file_name()
                    .and_then(|n| n.to_str())
                    .map(|n| n.contains("unpacked"))
                    .unwrap_or(false)
        });

    let Some(png_path) = png_path else {
        let _ = std::fs::remove_dir_all(&work);
        return Err(TopRebuildError::ContentUnreadable(
            "TEXTURE_INVALID: basisu unpack produced no PNG".into(),
        ));
    };
    let png_bytes = std::fs::read(&png_path).map_err(|e| {
        TopRebuildError::ContentUnreadable(format!("TEXTURE_INVALID: read unpacked PNG: {e}"))
    })?;
    let _ = std::fs::remove_dir_all(&work);

    TextureData::from_encoded_bytes(&png_bytes).map_err(|e| {
        TopRebuildError::ContentUnreadable(format!("TEXTURE_INVALID: decode unpacked PNG: {e}"))
    })
}

/// Unpack KTX2 → RGBA via basisu, or opaque `image/ktx2` pass-through on failure.
///
/// Does not invent placeholder pixels: either real RGBA from unpack, or the
/// original encoded bytes preserved for later embed.
fn decode_ktx2_to_texture(ktx2_bytes: &[u8]) -> TextureData {
    match decode_ktx2_via_basisu(ktx2_bytes) {
        Ok(tex) => tex,
        Err(e) => {
            eprintln!(
                "warning: KTX2 unpack failed ({e}); opaque pass-through image/ktx2 ({} bytes)",
                ktx2_bytes.len()
            );
            TextureData::from_opaque_encoded("image/ktx2", ktx2_bytes)
        }
    }
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

    let tmp = work_dir.map(|p| p.join("_ktx2_work")).unwrap_or_else(|| {
        std::env::temp_dir().join(format!("top_rebuild_ktx2_{}", std::process::id()))
    });

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

/// Extract embedded images from a GLB (PNG/JPEG/KTX2→RGBA or opaque KTX2).
/// Returns textures + map of glTF image index → hash.
///
/// Applies the same `reader_compatible_glb` preprocess as mesh load so converter
/// B3DMs with `KHR_texture_basisu` (missing `textures[i].source`) are readable.
/// KTX2 payloads are unpacked via basisu when possible; on failure the original
/// `image/ktx2` bytes are kept as opaque pass-through (no invented pixels).
pub fn extract_textures_from_glb(
    glb: &[u8],
) -> Result<(Vec<TextureData>, BTreeMap<usize, String>)> {
    let glb = crate::glb::reader_compatible_glb(glb)?;
    let gltf = gltf::Gltf::from_slice(&glb)
        .map_err(|e| TopRebuildError::Other(format!("gltf parse: {e}")))?;
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
            if is_ktx2_payload(mime.as_deref(), &raw) {
                let tex = decode_ktx2_to_texture(&raw);
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

    #[test]
    fn decode_ktx2_via_basisu_when_available() {
        let Some(basisu) = find_basisu() else {
            eprintln!("skip decode_ktx2_via_basisu_when_available: basisu not found");
            return;
        };
        let png = TextureData::solid(200, 10, 10, 255, 8);
        let png_bytes = encode_png(&png).unwrap();
        let work = std::env::temp_dir().join(format!(
            "top_rebuild_ktx2_roundtrip_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&work);
        let ktx = encode_ktx2(&basisu, &png_bytes, &work).unwrap();
        assert!(is_ktx2_payload(Some("image/ktx2"), &ktx));
        let decoded = decode_ktx2_to_texture(&ktx);
        assert_eq!(decoded.width, 8);
        assert_eq!(decoded.height, 8);
        assert_eq!(decoded.rgba.len(), 8 * 8 * 4);
        assert!(decoded.opaque_encoded.is_none());
        let _ = std::fs::remove_dir_all(&work);
    }

    #[test]
    fn ktx2_unpack_failure_is_opaque_passthrough() {
        // Invalid KTX2 body: basisu present → unpack fails; absent → find fails.
        // Either path must opaque-pass-through, never invent pixels / hard-fail.
        let bytes = b"\xabKTX 20\xbb\r\n\x1a\n????not-a-real-ktx2????";
        let tex = decode_ktx2_to_texture(bytes);
        let (mime, enc) = tex
            .opaque_encoded
            .as_ref()
            .expect("expected opaque image/ktx2 pass-through");
        assert_eq!(mime, "image/ktx2");
        assert_eq!(enc.as_slice(), bytes);
        assert_eq!(tex.width, 1);
        assert_eq!(tex.height, 1);
    }

    #[test]
    fn from_opaque_encoded_preserves_bytes() {
        let raw = b"fake-ktx2-payload";
        let tex = TextureData::from_opaque_encoded("image/ktx2", raw);
        assert_eq!(
            tex.opaque_encoded.as_ref().map(|(m, b)| (m.as_str(), b.as_slice())),
            Some(("image/ktx2", raw.as_slice()))
        );
        let (proc, m) = process_textures(&[tex], 64, 0, false, None).unwrap();
        assert_eq!(proc.len(), 1);
        assert_eq!(proc[0].mime, "image/ktx2");
        assert_eq!(proc[0].bytes, raw);
        assert!(proc[0].ktx2);
        assert_eq!(m.ktx2_encoded, 1);
    }

    #[test]
    fn basisu_path_prefixes_includes_parent_and_sibling_bin() {
        let root = std::env::temp_dir().join(format!(
            "top_rebuild_basisu_path_{}",
            std::process::id()
        ));
        let texture = root.join("texture");
        let bin = root.join("bin");
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&texture).unwrap();
        std::fs::create_dir_all(&bin).unwrap();
        let exe = texture.join("basisu");
        std::fs::write(&exe, b"").unwrap();

        let prefixes = basisu_path_prefixes(&exe);
        assert!(
            prefixes.iter().any(|p| p == &texture || p.ends_with("texture")),
            "expected texture parent in {prefixes:?}"
        );
        assert!(
            prefixes.iter().any(|p| {
                let c = std::fs::canonicalize(&bin).unwrap_or(bin.clone());
                p == &c || p == &bin || p.ends_with("bin")
            }),
            "expected sibling bin in {prefixes:?}"
        );

        let path = path_with_basisu_dirs(&exe);
        let path_s = path.to_string_lossy();
        assert!(
            path_s.contains("texture") || path_s.contains("bin"),
            "PATH should mention layout dirs: {path_s}"
        );

        // Without sibling bin, only parent is prepended.
        let _ = std::fs::remove_dir_all(&bin);
        let prefixes2 = basisu_path_prefixes(&exe);
        assert_eq!(prefixes2.len(), 1);
        assert_eq!(prefixes2[0], texture);

        let _ = std::fs::remove_dir_all(&root);
    }
}
