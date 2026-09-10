//! B3DM ↔ GLB payload reader/writer (Phase 6 + Phase 12 alignment fix).

use crate::error::{Result, TopRebuildError};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read};
use std::path::Path;

/// Extract the embedded GLB payload from a Batched 3D Model (.b3dm) file.
pub fn extract_glb_from_b3dm_bytes(data: &[u8]) -> Result<Vec<u8>> {
    if data.len() < 28 {
        return Err(TopRebuildError::Other("b3dm too short".into()));
    }
    if &data[0..4] != b"b3dm" {
        return Err(TopRebuildError::Other("not a b3dm file".into()));
    }
    let mut cur = Cursor::new(data);
    let mut magic = [0u8; 4];
    cur.read_exact(&mut magic)?;
    let _version = cur.read_u32::<LittleEndian>()?;
    let byte_length = cur.read_u32::<LittleEndian>()? as usize;
    let ft_json = cur.read_u32::<LittleEndian>()? as usize;
    let ft_bin = cur.read_u32::<LittleEndian>()? as usize;
    let bt_json = cur.read_u32::<LittleEndian>()? as usize;
    let bt_bin = cur.read_u32::<LittleEndian>()? as usize;

    let mut offset = 28 + ft_json + ft_bin + bt_json + bt_bin;
    if offset > data.len() {
        return Err(TopRebuildError::Other(format!(
            "b3dm table sizes exceed file (offset={offset}, len={})",
            data.len()
        )));
    }
    // Align / search for glTF magic if tables leave padding.
    if offset + 4 <= data.len() && &data[offset..offset + 4] != b"glTF" {
        let mut found = None;
        for pad in 0..8 {
            let o = offset + pad;
            if o + 4 <= data.len() && &data[o..o + 4] == b"glTF" {
                found = Some(o);
                break;
            }
        }
        offset = found.ok_or_else(|| {
            TopRebuildError::Other("glb magic not found in b3dm payload".into())
        })?;
    }
    // Prefer GLB header length so trailing b3dm padding is not returned.
    if offset + 12 <= data.len() && &data[offset..offset + 4] == b"glTF" {
        let glb_len = u32::from_le_bytes(data[offset + 8..offset + 12].try_into().unwrap()) as usize;
        if glb_len >= 12 && offset + glb_len <= data.len() {
            return Ok(data[offset..offset + glb_len].to_vec());
        }
    }
    if byte_length > 0 && byte_length <= data.len() && byte_length > offset {
        Ok(data[offset..byte_length].to_vec())
    } else {
        Ok(data[offset..].to_vec())
    }
}

pub fn extract_glb_from_b3dm_path(path: &Path) -> Result<Vec<u8>> {
    let data = std::fs::read(path)?;
    extract_glb_from_b3dm_bytes(&data)
}

/// Pad JSON with trailing spaces so that `start_offset + len` ends on an 8-byte boundary.
fn pad_json_end_aligned(mut json: Vec<u8>, start_offset: usize) -> Vec<u8> {
    while (start_offset + json.len()) % 8 != 0 {
        json.push(b' ');
    }
    json
}

/// Pack a GLB into a minimal valid B3DM for CesiumGS validator.
///
/// Spec (Feature Table / Batch Table padding):
/// - JSON headers must **end** on an 8-byte boundary within the tile
/// - Binary bodies (and embedded GLB) must **start** on an 8-byte boundary
/// - Tile `byteLength` must be a multiple of 8
///
/// Header is 28 bytes, so feature-table JSON length is typically ≡ 4 (mod 8).
pub fn pack_glb_as_b3dm(glb: &[u8]) -> Result<Vec<u8>> {
    let ftj = pad_json_end_aligned(b"{\"BATCH_LENGTH\":1}".to_vec(), 28);
    let after_ft = 28 + ftj.len();
    debug_assert_eq!(after_ft % 8, 0);

    // Empty feature binary (length 0) — next section starts at after_ft (8-aligned).
    let btj = pad_json_end_aligned(b"{}".to_vec(), after_ft);
    let after_bt = after_ft + btj.len();
    debug_assert_eq!(after_bt % 8, 0);

    let mut total = after_bt + glb.len();
    let pad_end = (8 - (total % 8)) % 8;
    total += pad_end;

    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"b3dm");
    out.write_u32::<LittleEndian>(1)?;
    out.write_u32::<LittleEndian>(total as u32)?;
    out.write_u32::<LittleEndian>(ftj.len() as u32)?;
    out.write_u32::<LittleEndian>(0)?; // featureTableBinaryByteLength
    out.write_u32::<LittleEndian>(btj.len() as u32)?;
    out.write_u32::<LittleEndian>(0)?; // batchTableBinaryByteLength
    out.extend_from_slice(&ftj);
    out.extend_from_slice(&btj);
    out.extend_from_slice(glb);
    out.extend(std::iter::repeat(0u8).take(pad_end));
    Ok(out)
}

/// Load content bytes as GLB: accepts raw `.glb` / `.gltf` binary or `.b3dm`.
pub fn load_content_glb(path: &Path) -> Result<Vec<u8>> {
    let data = std::fs::read(path)?;
    if data.len() >= 4 && &data[0..4] == b"glTF" {
        return Ok(data);
    }
    if data.len() >= 4 && &data[0..4] == b"b3dm" {
        return extract_glb_from_b3dm_bytes(&data);
    }
    // Extension hint
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "glb" {
        return Ok(data);
    }
    if ext == "b3dm" {
        return extract_glb_from_b3dm_bytes(&data);
    }
    Err(TopRebuildError::Other(format!(
        "unsupported content format: {}",
        path.display()
    )))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_emptyish_glb() {
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&12u32.to_le_bytes());
        let b3dm = pack_glb_as_b3dm(&glb).unwrap();
        assert_eq!(b3dm.len() % 8, 0);
        let extracted = extract_glb_from_b3dm_bytes(&b3dm).unwrap();
        assert_eq!(extracted, glb);
        let ftj = u32::from_le_bytes(b3dm[12..16].try_into().unwrap()) as usize;
        let ftb = u32::from_le_bytes(b3dm[16..20].try_into().unwrap()) as usize;
        let btj = u32::from_le_bytes(b3dm[20..24].try_into().unwrap()) as usize;
        let btb = u32::from_le_bytes(b3dm[24..28].try_into().unwrap()) as usize;
        assert_eq!((28 + ftj) % 8, 0, "feature JSON must end 8-aligned");
        assert_eq!((28 + ftj + ftb) % 8, 0);
        assert_eq!((28 + ftj + ftb + btj) % 8, 0, "batch JSON must end 8-aligned");
        let offset = 28 + ftj + ftb + btj + btb;
        assert_eq!(offset % 8, 0);
        assert_eq!(&b3dm[offset..offset + 4], b"glTF");
    }
}
