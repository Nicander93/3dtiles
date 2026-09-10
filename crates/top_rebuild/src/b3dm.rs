//! B3DM ↔ GLB payload reader/writer (Phase 6).

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

/// Pack a GLB into a minimal fanvanzh-style B3DM (feature/batch JSON only).
pub fn pack_glb_as_b3dm(glb: &[u8]) -> Result<Vec<u8>> {
    fn pad4(mut b: Vec<u8>) -> Vec<u8> {
        let rem = (4 - (b.len() % 4)) % 4;
        b.extend(std::iter::repeat(b' ').take(rem));
        b
    }
    let ftj = pad4(b"{\"BATCH_LENGTH\":1}".to_vec());
    let btj = pad4(b"{\"batchId\":{\"byteOffset\":0}}".to_vec());
    let total = 28 + ftj.len() + btj.len() + glb.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"b3dm");
    out.write_u32::<LittleEndian>(1)?;
    out.write_u32::<LittleEndian>(total as u32)?;
    out.write_u32::<LittleEndian>(ftj.len() as u32)?;
    out.write_u32::<LittleEndian>(0)?;
    out.write_u32::<LittleEndian>(btj.len() as u32)?;
    out.write_u32::<LittleEndian>(0)?;
    out.extend_from_slice(&ftj);
    out.extend_from_slice(&btj);
    out.extend_from_slice(glb);
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
        // Minimal valid-looking glTF magic + version + length header only (not a full GLB)
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&12u32.to_le_bytes());
        let b3dm = pack_glb_as_b3dm(&glb).unwrap();
        let extracted = extract_glb_from_b3dm_bytes(&b3dm).unwrap();
        assert_eq!(extracted, glb);
    }
}
