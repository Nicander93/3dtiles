//! B3DM ↔ GLB payload reader/writer (Phase 6).

use crate::error::{Result, TopRebuildError};
use byteorder::{LittleEndian, ReadBytesExt, WriteBytesExt};
use std::io::{Cursor, Read};
use std::path::Path;

#[derive(Clone, Debug)]
pub struct LoadedContent {
    pub glb: Vec<u8>,
    pub rtc_center: Option<[f64; 3]>,
}

pub fn extract_glb_from_b3dm_bytes(data: &[u8]) -> Result<Vec<u8>> {
    Ok(extract_b3dm_bytes(data)?.glb)
}

pub fn extract_glb_from_b3dm_path(path: &Path) -> Result<Vec<u8>> {
    let data = std::fs::read(path)?;
    Ok(extract_b3dm_bytes(&data)?.glb)
}

fn extract_b3dm_bytes(data: &[u8]) -> Result<LoadedContent> {
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
    let glb = if byte_length > 0 && byte_length <= data.len() && byte_length > offset {
        data[offset..byte_length].to_vec()
    } else {
        data[offset..].to_vec()
    };
    let ftj_end = (28 + ft_json).min(data.len());
    let ftj = if ft_json > 0 { &data[28..ftj_end] } else { &[] };
    Ok(LoadedContent {
        glb,
        rtc_center: parse_rtc_center(ftj),
    })
}

fn parse_rtc_center(ft_json: &[u8]) -> Option<[f64; 3]> {
    let s = std::str::from_utf8(ft_json).ok()?;
    let s = s.trim_matches(|c: char| c == '\0' || c.is_whitespace());
    if s.is_empty() {
        return None;
    }
    let v: serde_json::Value = serde_json::from_str(s).ok()?;
    let arr = v.get("RTC_CENTER")?.as_array()?;
    if arr.len() < 3 {
        return None;
    }
    Some([
        arr[0].as_f64()?,
        arr[1].as_f64()?,
        arr[2].as_f64()?,
    ])
}

fn pad_json(mut bytes: Vec<u8>, offset: usize) -> Vec<u8> {
    while (offset + bytes.len()) % 8 != 0 {
        bytes.push(b' ');
    }
    bytes
}

fn feature_table_json(rtc: Option<[f64; 3]>) -> Vec<u8> {
    let json = if let Some([x, y, z]) = rtc {
        format!("{{\"BATCH_LENGTH\":0,\"RTC_CENTER\":[{x},{y},{z}]}}").into_bytes()
    } else {
        b"{\"BATCH_LENGTH\":0}".to_vec()
    };
    pad_json(json, 28)
}

fn write_unbatched_b3dm(glb: &[u8], rtc: Option<[f64; 3]>) -> Result<Vec<u8>> {
    let ftj = feature_table_json(rtc);
    let total = 28 + ftj.len() + glb.len();
    let mut out = Vec::with_capacity(total);
    out.extend_from_slice(b"b3dm");
    out.write_u32::<LittleEndian>(1)?;
    out.write_u32::<LittleEndian>(total as u32)?;
    out.write_u32::<LittleEndian>(ftj.len() as u32)?;
    out.write_u32::<LittleEndian>(0)?;
    out.write_u32::<LittleEndian>(0)?;
    out.write_u32::<LittleEndian>(0)?;
    out.extend_from_slice(&ftj);
    out.extend_from_slice(glb);
    Ok(out)
}

pub fn pack_glb_as_b3dm(glb: &[u8]) -> Result<Vec<u8>> {
    pack_glb_as_b3dm_with_rtc(glb, None)
}

pub fn pack_glb_as_b3dm_with_rtc(glb: &[u8], rtc: Option<[f64; 3]>) -> Result<Vec<u8>> {
    write_unbatched_b3dm(glb, rtc)
}

pub fn load_content(path: &Path) -> Result<LoadedContent> {
    let data = std::fs::read(path)?;
    if data.len() >= 4 && &data[0..4] == b"glTF" {
        return Ok(LoadedContent {
            glb: data,
            rtc_center: None,
        });
    }
    if data.len() >= 4 && &data[0..4] == b"b3dm" {
        return extract_b3dm_bytes(&data);
    }
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "glb" {
        return Ok(LoadedContent {
            glb: data,
            rtc_center: None,
        });
    }
    if ext == "b3dm" {
        return extract_b3dm_bytes(&data);
    }
    Err(TopRebuildError::UnsupportedContent(format!(
        "content format: {}",
        path.display()
    )))
}

pub fn load_content_glb(path: &Path) -> Result<Vec<u8>> {
    Ok(load_content(path)?.glb)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_unbatched_layout(data: &[u8]) {
        let ft_json = u32::from_le_bytes(data[12..16].try_into().unwrap()) as usize;
        let ft_binary = u32::from_le_bytes(data[16..20].try_into().unwrap());
        let bt_json = u32::from_le_bytes(data[20..24].try_into().unwrap());
        let bt_binary = u32::from_le_bytes(data[24..28].try_into().unwrap());
        let glb_offset = 28 + ft_json;
        let feature: serde_json::Value =
            serde_json::from_slice(&data[28..glb_offset]).unwrap();

        assert_eq!(feature["BATCH_LENGTH"], 0);
        assert_eq!(ft_binary, 0);
        assert_eq!(bt_json, 0);
        assert_eq!(bt_binary, 0);
        assert_eq!(glb_offset % 8, 0);
        assert_eq!(&data[glb_offset..glb_offset + 4], b"glTF");
    }

    #[test]
    fn roundtrip_emptyish_glb() {
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&12u32.to_le_bytes());
        let b3dm = pack_glb_as_b3dm(&glb).unwrap();
        assert_unbatched_layout(&b3dm);
        let extracted = extract_glb_from_b3dm_bytes(&b3dm).unwrap();
        assert_eq!(extracted, glb);
    }

    #[test]
    fn rtc_center_roundtrip() {
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2u32.to_le_bytes());
        glb.extend_from_slice(&12u32.to_le_bytes());
        let b3dm = pack_glb_as_b3dm_with_rtc(&glb, Some([10.0, 20.0, 30.0])).unwrap();
        assert_unbatched_layout(&b3dm);
        let loaded = extract_b3dm_bytes(&b3dm).unwrap();
        assert_eq!(loaded.glb, glb);
        let rtc = loaded.rtc_center.expect("rtc");
        assert!((rtc[0] - 10.0).abs() < 1e-9);
        assert!((rtc[1] - 20.0).abs() < 1e-9);
        assert!((rtc[2] - 30.0).abs() < 1e-9);
    }
}
