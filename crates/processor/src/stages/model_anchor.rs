use crate::cancel::CancelFlag;
use crate::geo::enu_to_ecef;
use crate::stages::clip::math::{
    array, multiply, node_transform, transform, Bounds, Matrix, IDENTITY, Y_UP,
};
use geoforge_protocol::{GeoReferenceOptions, ModelPivot};
use serde_json::{json, Value};
use std::{
    fs,
    io::{Read, Seek, SeekFrom},
    path::Path,
};

/// Converter v0.2.4 adds its bbox centre to the anchor and ignores pivot/HPR.
/// Replace that placement using the normalized tile-space bounds, before validation/commit.
pub fn apply(
    output: &Path,
    reference: &GeoReferenceOptions,
    cancel: &CancelFlag,
) -> Result<(), String> {
    let GeoReferenceOptions::Anchor {
        longitude_deg,
        latitude_deg,
        ellipsoid_height_m,
        pivot,
        heading_deg,
        pitch_deg,
        roll_deg,
    } = reference
    else {
        return Ok(());
    };
    let path = output.join("tileset.json");
    let mut doc: Value = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    let offset = match pivot {
        ModelPivot::Original => [0.; 3],
        ModelPivot::BoundingBoxCenter | ModelPivot::BottomCenter => {
            // Tile bounds include conservative padding. Use POSITION accessor bounds so
            // bottom-centre placement lands on the geometry rather than below its floor.
            let axis = match doc["asset"]["gltfUpAxis"].as_str() {
                Some("Z") => IDENTITY,
                None | Some("Y") => Y_UP,
                _ => return Err("unsupported model glTF up axis".into()),
            };
            let mut root = doc["root"].clone();
            root.as_object_mut()
                .ok_or("invalid model root")?
                .remove("transform");
            let b = tile_bounds(&root, output, IDENTITY, axis, 0, cancel)?
                .ok_or("model has no geometry bounds")?;
            let mut centre = std::array::from_fn(|i| (b.min[i] + b.max[i]) * 0.5);
            if *pivot == ModelPivot::BottomCenter {
                centre[2] = b.min[2];
            }
            centre
        }
    };
    doc["root"]["transform"] = json!(placement(
        *longitude_deg,
        *latitude_deg,
        *ellipsoid_height_m,
        *heading_deg,
        *pitch_deg,
        *roll_deg,
        offset
    ));
    fs::write(
        path,
        serde_json::to_vec_pretty(&doc).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}
fn union(a: Option<Bounds>, b: Option<Bounds>) -> Option<Bounds> {
    match (a, b) {
        (Some(a), Some(b)) => Some(a.union(b)),
        (a, None) => a,
        (None, b) => b,
    }
}
fn tile_bounds(
    tile: &Value,
    base: &Path,
    parent: Matrix,
    axis: Matrix,
    depth: usize,
    cancel: &CancelFlag,
) -> Result<Option<Bounds>, String> {
    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    if depth > 128 {
        return Err("model bounds nesting exceeds 128".into());
    }
    let world = multiply(parent, transform(tile.get("transform"))?);
    let mut bounds = None;
    if let Some(uri) = tile["content"]["uri"]
        .as_str()
        .or_else(|| tile["content"]["url"].as_str())
    {
        let path = base.join(uri);
        if uri.ends_with(".json") {
            let doc: Value = serde_json::from_slice(&fs::read(&path).map_err(|e| e.to_string())?)
                .map_err(|e| e.to_string())?;
            bounds = tile_bounds(
                &doc["root"],
                path.parent().unwrap(),
                world,
                axis,
                depth + 1,
                cancel,
            )?;
        } else {
            bounds = content_bounds(&path, world, axis)?;
        }
    }
    if let Some(children) = tile["children"].as_array() {
        for child in children {
            bounds = union(
                bounds,
                tile_bounds(child, base, world, axis, depth + 1, cancel)?,
            );
        }
    }
    Ok(bounds)
}
fn read_json(file: &mut fs::File, size: usize) -> Result<Value, String> {
    if size > 16 * 1024 * 1024 {
        return Err("model metadata exceeds 16 MiB".into());
    }
    let mut bytes = vec![0; size];
    file.read_exact(&mut bytes).map_err(|e| e.to_string())?;
    if bytes.is_empty() {
        return Ok(json!({}));
    }
    serde_json::from_slice(&bytes).map_err(|e| e.to_string())
}
fn content_bounds(path: &Path, world: Matrix, axis: Matrix) -> Result<Option<Bounds>, String> {
    // Read metadata only: textures and geometry buffers may be hundreds of MiB.
    let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
    let mut header = [0u8; 28];
    file.read_exact(&mut header[..4])
        .map_err(|e| e.to_string())?;
    let mut rtc = IDENTITY;
    if &header[..4] == b"b3dm" {
        file.read_exact(&mut header[4..])
            .map_err(|e| e.to_string())?;
        let sizes: Vec<u64> = [12, 16, 20, 24]
            .iter()
            .map(|i| u32::from_le_bytes(header[*i..*i + 4].try_into().unwrap()) as u64)
            .collect();
        let table = read_json(&mut file, sizes[0] as usize)?;
        if let Some(p) = table.get("RTC_CENTER") {
            rtc[12..15].copy_from_slice(&array::<3>(p)?);
        }
        file.seek(SeekFrom::Start(28 + sizes.iter().sum::<u64>()))
            .map_err(|e| e.to_string())?;
        file.read_exact(&mut header[..4])
            .map_err(|e| e.to_string())?;
    }
    if &header[..4] != b"glTF" {
        return Err("model pivot requires GLB/B3DM geometry".into());
    }
    file.read_exact(&mut header[4..20])
        .map_err(|e| e.to_string())?;
    if &header[4..8] != 2u32.to_le_bytes().as_slice() || &header[16..20] != b"JSON" {
        return Err("invalid model GLB header".into());
    }
    let size = u32::from_le_bytes(header[12..16].try_into().unwrap()) as usize;
    let doc = read_json(&mut file, size)?;
    let nodes = doc["scenes"][doc["scene"].as_u64().unwrap_or(0) as usize]["nodes"]
        .as_array()
        .ok_or("model glTF has no scene")?;
    let mut bounds = None;
    let base = multiply(multiply(world, rtc), axis);
    for node in nodes {
        bounds = union(
            bounds,
            node_bounds(
                &doc,
                node.as_u64().ok_or("invalid node index")? as usize,
                base,
                0,
            )?,
        );
    }
    Ok(bounds)
}
fn node_bounds(
    doc: &Value,
    id: usize,
    parent: Matrix,
    depth: usize,
) -> Result<Option<Bounds>, String> {
    if depth > 128 {
        return Err("model node bounds nesting exceeds 128".into());
    }
    let node = doc["nodes"].get(id).ok_or("invalid model node index")?;
    let matrix = multiply(parent, node_transform(node)?);
    let mut bounds = None;
    if let Some(mesh) = node["mesh"].as_u64() {
        let primitives = doc["meshes"][mesh as usize]["primitives"]
            .as_array()
            .ok_or("invalid model mesh")?;
        for primitive in primitives {
            let id = primitive["attributes"]["POSITION"]
                .as_u64()
                .ok_or("model POSITION missing")? as usize;
            let accessor = &doc["accessors"][id];
            let min = array::<3>(&accessor["min"])?;
            let max = array::<3>(&accessor["max"])?;
            if (0..3).any(|i| min[i] > max[i]) {
                return Err("invalid model POSITION bounds".into());
            }
            bounds = union(bounds, Some(Bounds { min, max }.transformed(matrix)));
        }
    }
    if let Some(children) = node["children"].as_array() {
        for child in children {
            bounds = union(
                bounds,
                node_bounds(
                    doc,
                    child.as_u64().ok_or("invalid model child")? as usize,
                    matrix,
                    depth + 1,
                )?,
            );
        }
    }
    Ok(bounds)
}
fn placement(
    lon: f64,
    lat: f64,
    height: f64,
    heading: f64,
    pitch: f64,
    roll: f64,
    pivot: [f64; 3],
) -> Matrix {
    let (sh, ch) = heading.to_radians().sin_cos();
    let (sp, cp) = pitch.to_radians().sin_cos();
    let (sr, cr) = roll.to_radians().sin_cos();
    // Cesium HPR: heading about -Z, pitch about -Y, roll about +X; Rz * Ry * Rx.
    let rz = [
        ch, -sh, 0., 0., sh, ch, 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
    ];
    let ry = [
        cp, 0., sp, 0., 0., 1., 0., 0., -sp, 0., cp, 0., 0., 0., 0., 1.,
    ];
    let rx = [
        1., 0., 0., 0., 0., cr, sr, 0., 0., -sr, cr, 0., 0., 0., 0., 1.,
    ];
    let mut translation = IDENTITY;
    translation[12] = -pivot[0];
    translation[13] = -pivot[1];
    translation[14] = -pivot[2];
    multiply(
        enu_to_ecef(lon, lat, height),
        multiply(multiply(rz, ry), multiply(rx, translation)),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stages::clip::math::point;
    #[test]
    fn bottom_pivot_uses_geometry_not_padded_tile_bounds() {
        let dir = tempfile::tempdir().unwrap();
        let gltf = json!({"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],"accessors":[{"min":[10,20,30],"max":[12,23,34]}]});
        let mut payload = serde_json::to_vec(&gltf).unwrap();
        while payload.len() % 4 != 0 {
            payload.push(b' ');
        }
        let mut bytes = b"glTF".to_vec();
        bytes.extend(2u32.to_le_bytes());
        bytes.extend((20u32 + payload.len() as u32).to_le_bytes());
        bytes.extend((payload.len() as u32).to_le_bytes());
        bytes.extend(b"JSON");
        bytes.extend(payload);
        fs::write(dir.path().join("model.glb"), bytes).unwrap();
        let path = dir.path().join("tileset.json");
        fs::write(&path,serde_json::to_vec(&json!({"asset":{"gltfUpAxis":"Z"},"root":{"transform":IDENTITY,"boundingVolume":{"box":[11,21.5,32,1.5,0,0,0,2,0,0,0,2.5]},"content":{"uri":"model.glb"}}})).unwrap()).unwrap();
        let reference: GeoReferenceOptions = serde_json::from_value(
            json!({"mode":"anchor","longitudeDeg":0,"latitudeDeg":0,"ellipsoidHeightM":0,"pivot":"bottomCenter"}),
        )
        .unwrap();
        apply(dir.path(), &reference, &CancelFlag::new()).unwrap();
        let doc: Value = serde_json::from_slice(&fs::read(path).unwrap()).unwrap();
        let anchor = point(
            transform(doc["root"].get("transform")).unwrap(),
            [11., 21.5, 30.],
        );
        assert!(
            (anchor[0] - 6378137.).abs() < 1e-8 && anchor[1].abs() < 1e-8 && anchor[2].abs() < 1e-8
        );
    }
    #[test]
    fn heading_turns_east_towards_south_and_pivot_maps_to_anchor() {
        let m = placement(0., 0., 0., 90., 0., 0., [10., 20., 30.]);
        let p = point(m, [10., 20., 30.]);
        assert!((p[0] - 6378137.).abs() < 1e-8 && p[1].abs() < 1e-8 && p[2].abs() < 1e-8);
        let p = point(m, [11., 20., 30.]);
        assert!((p[2] + 1.).abs() < 1e-8);
    }
}
