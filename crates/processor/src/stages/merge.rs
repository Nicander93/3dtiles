//! Portable tileset aggregation. Source trees remain unchanged beneath external tileset references.

use super::{commit, scan, validate};
use crate::{path_policy, CancelFlag, Emitter, Stage, TaskConfig};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::io::{Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

pub const MAX_INPUTS: usize = 64;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct MergeOptions {
    additional_inputs: Vec<String>,
}

pub fn input_paths(input: &str, options: &Value) -> Result<Vec<PathBuf>, String> {
    let merge: MergeOptions =
        serde_json::from_value(options.get("merge").cloned().unwrap_or(Value::Null))
            .map_err(|e| format!("invalid merge options: {e}"))?;
    let paths: Vec<_> = std::iter::once(input.to_string())
        .chain(merge.additional_inputs)
        .collect();
    if !(2..=MAX_INPUTS).contains(&paths.len()) || paths.iter().any(|p| p.trim().is_empty()) {
        return Err(format!("merge requires 2..={MAX_INPUTS} non-empty inputs"));
    }
    Ok(paths.into_iter().map(PathBuf::from).collect())
}

/// Shared by task submission and execution: protect every input, not just the first one.
pub fn preflight(
    input: &str,
    options: &Value,
    output: &Path,
    task_id: &str,
) -> Result<Vec<PathBuf>, String> {
    let mut seen = HashSet::new();
    let mut tilesets = Vec::new();
    for path in input_paths(input, options)? {
        let tileset = scan::resolve_tileset(&path.to_string_lossy())?;
        if !tileset
            .extension()
            .and_then(|n| n.to_str())
            .is_some_and(|n| n.eq_ignore_ascii_case("json"))
        {
            return Err("merge input must be a directory or tileset JSON file".into());
        }
        let name = tileset
            .file_name()
            .and_then(|n| n.to_str())
            .ok_or("merge tileset filename must be UTF-8")?;
        if name.contains([':', '?', '#', '%', '\\']) {
            return Err("merge tileset filename contains unsupported URI characters".into());
        }
        let canonical = path_policy::normalize_path(&tileset)?;
        if !seen.insert(canonical.clone()) {
            return Err(format!("duplicate merge input: {}", tileset.display()));
        }
        tilesets.push(canonical);
    }
    // Check the complete input set before validate_io_paths may create output parents.
    path_policy::validate_task_id(task_id)?;
    let output = path_policy::normalize_path(output)?;
    let temp = commit::temp_work_dir(&output, task_id);
    for input in &tilesets {
        let root = input.parent().ok_or("merge input has no parent")?;
        for target in [&output, &temp] {
            if target == root
                || path_policy::is_strict_descendant(root, target)
                || path_policy::is_strict_descendant(target, root)
            {
                return Err(
                    "merge output or temporary directory must not overlap any input data root"
                        .into(),
                );
            }
        }
    }
    for input in &tilesets {
        path_policy::validate_io_paths(input, &output, task_id)?;
    }
    Ok(tilesets)
}

pub fn run(config: &TaskConfig, emitter: &Emitter, cancel: &CancelFlag) -> Result<PathBuf, String> {
    emitter.stage(Stage::Scan, "Checking merge inputs");
    let output = path_policy::normalize_path(Path::new(config.output_path()))?;
    let inputs = preflight(
        config.input_path(),
        &config.options,
        &output,
        &config.task_id,
    )?;
    let mut children = Vec::new();
    let mut combined: Option<Bounds> = None;
    let mut error = 0.0_f64;
    let mut version = "1.0";
    for (index, input) in inputs.iter().enumerate() {
        check_cancel(cancel)?;
        let source: Value = serde_json::from_slice(&fs::read(input).map_err(|e| e.to_string())?)
            .map_err(|e| format!("invalid tileset {}: {e}", input.display()))?;
        check_tileset_features(&source)?;
        if source["asset"]["version"] == "1.1" {
            version = "1.1";
        }
        let (bounds, source_error) = root_bounds(&source)?;
        combined = Some(combined.map(|b| b.union(bounds)).unwrap_or(bounds));
        error = error.max(source_error);
        children.push(json!({
            "boundingVolume": {"box": bounds.box_array()},
            "geometricError": source_error,
            "content": {"uri": format!("sources/source-{:03}/{}", index + 1, input.file_name().and_then(|n| n.to_str()).ok_or("merge input filename must be UTF-8")?)}
        }));
    }
    let temp = commit::prepare_temp(&output, &config.task_id)?;
    let mut guard = commit::TempGuard::new(temp.clone());
    let staged = temp.join("staged");
    fs::create_dir(&staged).map_err(|e| e.to_string())?;
    emitter.stage(Stage::Merge, "Copying source datasets");
    emitter.progress(Stage::Merge, 0, inputs.len() as u64);
    for (index, input) in inputs.iter().enumerate() {
        let root = input.parent().ok_or("merge input has no parent")?;
        copy_tree(
            root,
            &staged.join(format!("sources/source-{:03}", index + 1)),
            root,
            cancel,
            0,
        )?;
        emitter.progress(Stage::Merge, (index + 1) as u64, inputs.len() as u64);
    }
    let merged = json!({
        "asset": {"version": version, "generator": "GeoForge tileset merge"},
        "geometricError": error,
        "root": {
            "boundingVolume": {"box": combined.ok_or("merge has no bounds")?.box_array()},
            "geometricError": error, "refine": "ADD", "children": children
        }
    });
    fs::write(
        staged.join("tileset.json"),
        serde_json::to_vec_pretty(&merged).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    check_cancel(cancel)?;
    commit::write_checkpoint(&temp, commit::Checkpoint::Validating)?;
    validate::validate_tileset_dir_cancellable(emitter, &staged, Some(cancel))?;
    commit::write_checkpoint(&temp, commit::Checkpoint::Validated)?;
    check_cancel(cancel)?;
    commit::write_checkpoint(&temp, commit::Checkpoint::Committing)?;
    commit::commit_rename(emitter, &staged, &temp, &output, Some(&mut guard))?;
    commit::cleanup_temp(&temp);
    Ok(output)
}

fn check_cancel(cancel: &CancelFlag) -> Result<(), String> {
    if cancel.is_cancelled() {
        Err("cancelled".into())
    } else {
        Ok(())
    }
}

pub(crate) fn check_tileset_features(value: &Value) -> Result<(), String> {
    if value.get("root").is_some() {
        let asset = value.get("asset").ok_or("merge input missing asset")?;
        if !matches!(
            asset.get("version").and_then(Value::as_str),
            Some("1.0" | "1.1")
        ) {
            return Err("merge supports tileset asset.version 1.0 or 1.1".into());
        }
        if value.get("schema").is_some()
            || value.get("schemaUri").is_some()
            || value.get("groups").is_some()
        {
            return Err("merge does not yet support structural metadata schemas or groups".into());
        }
    }
    if value.get("metadata").and_then(|m| m.get("class")).is_some() {
        return Err("merge does not yet support structural metadata classes".into());
    }
    if value.get("implicitTiling").is_some() || value.get("contents").is_some() {
        return Err("merge does not yet support implicitTiling or multiple contents".into());
    }
    if value
        .get("extensionsRequired")
        .and_then(Value::as_array)
        .is_some_and(|a| !a.is_empty())
    {
        return Err("merge does not yet support required tileset extensions".into());
    }
    match value {
        Value::Object(obj) => {
            for child in obj.values() {
                check_tileset_features(child)?;
            }
        }
        Value::Array(arr) => {
            for child in arr {
                check_tileset_features(child)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn copy_tree(
    src: &Path,
    dst: &Path,
    source_root: &Path,
    cancel: &CancelFlag,
    depth: usize,
) -> Result<(), String> {
    check_cancel(cancel)?;
    if depth > 128 {
        return Err("merge directory nesting exceeds 128".into());
    }
    let metadata = fs::symlink_metadata(src).map_err(|e| e.to_string())?;
    #[cfg(windows)]
    let linked = {
        use std::os::windows::fs::MetadataExt;
        metadata.file_attributes() & 0x400 != 0
    };
    #[cfg(not(windows))]
    let linked = metadata.file_type().is_symlink();
    if linked {
        return Err(format!(
            "merge refuses symbolic links or junctions: {}",
            src.display()
        ));
    }
    if metadata.is_dir() {
        fs::create_dir_all(dst).map_err(|e| e.to_string())?;
        for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
            let entry = entry.map_err(|e| e.to_string())?;
            copy_tree(
                &entry.path(),
                &dst.join(entry.file_name()),
                source_root,
                cancel,
                depth + 1,
            )?;
        }
    } else if metadata.is_file() {
        let extension = src
            .extension()
            .and_then(|s| s.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if matches!(extension.as_str(), "glb" | "b3dm") {
            check_binary_references(src, source_root)?;
        }
        if matches!(extension.as_str(), "json" | "gltf") {
            // Only JSON documents are inspected; unrelated binary resources are streamed.
            if let Ok(value) =
                serde_json::from_slice::<Value>(&fs::read(src).map_err(|e| e.to_string())?)
            {
                check_tileset_features(&value)?;
                check_references(
                    &value,
                    src.parent().ok_or("resource has no parent")?,
                    source_root,
                )?;
            }
        }
        let mut input = fs::File::open(src).map_err(|e| e.to_string())?;
        let mut output = fs::File::create(dst).map_err(|e| e.to_string())?;
        let mut buffer = vec![0_u8; 128 * 1024];
        loop {
            check_cancel(cancel)?;
            let size = input.read(&mut buffer).map_err(|e| e.to_string())?;
            if size == 0 {
                break;
            }
            output
                .write_all(&buffer[..size])
                .map_err(|e| e.to_string())?;
        }
    } else {
        return Err(format!("unsupported merge resource: {}", src.display()));
    }
    Ok(())
}

fn check_references(value: &Value, base: &Path, root: &Path) -> Result<(), String> {
    match value {
        Value::Object(obj) => {
            for (key, child) in obj {
                if key == "uri" || key == "url" {
                    if let Some(uri) = child.as_str() {
                        if uri.starts_with("data:") {
                            continue;
                        }
                        if uri.contains([':', '?', '#', '%', '\\']) || uri.starts_with('/') {
                            return Err(format!(
                                "merge requires plain relative local resource URIs: {uri}"
                            ));
                        }
                        let referenced = base.join(uri);
                        if !referenced.is_file() {
                            return Err(format!("merge resource missing: {uri}"));
                        }
                        let path = path_policy::normalize_path(&referenced)
                            .map_err(|e| format!("merge resource missing: {uri} ({e})"))?;
                        let canonical_root = path_policy::normalize_path(root)?;
                        if !path_policy::is_strict_descendant(&canonical_root, &path) {
                            return Err(format!("merge resource escapes input root: {uri}"));
                        }
                    }
                } else if matches!(
                    key.as_str(),
                    "root"
                        | "children"
                        | "content"
                        | "contents"
                        | "buffers"
                        | "images"
                        | "extensions"
                ) {
                    check_references(child, base, root)?;
                }
            }
        }
        Value::Array(arr) => {
            for child in arr {
                check_references(child, base, root)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_binary_references(path: &Path, root: &Path) -> Result<(), String> {
    let inspect = || -> Result<(), String> {
        let mut file = fs::File::open(path).map_err(|e| e.to_string())?;
        let mut header = [0_u8; 28];
        file.read_exact(&mut header[..12])
            .map_err(|e| e.to_string())?;
        if &header[..4] == b"b3dm" {
            file.read_exact(&mut header[12..28])
                .map_err(|e| e.to_string())?;
            let offset = [12, 16, 20, 24]
                .into_iter()
                .map(|i| u32::from_le_bytes(header[i..i + 4].try_into().unwrap()) as u64)
                .sum::<u64>()
                + 28;
            file.seek(SeekFrom::Start(offset))
                .map_err(|e| e.to_string())?;
            file.read_exact(&mut header[..12])
                .map_err(|e| e.to_string())?;
        }
        if &header[..4] != b"glTF" {
            return Err("expected GLB payload".into());
        }
        let mut chunk = [0_u8; 8];
        file.read_exact(&mut chunk).map_err(|e| e.to_string())?;
        let length = u32::from_le_bytes(chunk[..4].try_into().unwrap()) as usize;
        if &chunk[4..] != b"JSON" || length > 64 * 1024 * 1024 {
            return Err("missing or oversized GLB JSON chunk".into());
        }
        let mut text = vec![0_u8; length];
        file.read_exact(&mut text).map_err(|e| e.to_string())?;
        let value: Value = serde_json::from_slice(&text).map_err(|e| e.to_string())?;
        check_references(&value, path.parent().ok_or("GLB has no parent")?, root)
    };
    inspect().map_err(|e| format!("merge binary resource {}: {e}", path.display()))
}

#[derive(Clone, Copy, Debug)]
struct Bounds {
    min: [f64; 3],
    max: [f64; 3],
}

impl Bounds {
    fn point(point: [f64; 3]) -> Self {
        Self {
            min: point,
            max: point,
        }
    }
    fn union(self, other: Self) -> Self {
        Self {
            min: std::array::from_fn(|i| self.min[i].min(other.min[i])),
            max: std::array::from_fn(|i| self.max[i].max(other.max[i])),
        }
    }
    fn box_array(self) -> [f64; 12] {
        let c: [f64; 3] = std::array::from_fn(|i| self.min[i] * 0.5 + self.max[i] * 0.5);
        let h: [f64; 3] = std::array::from_fn(|i| self.max[i] * 0.5 - self.min[i] * 0.5);
        [c[0], c[1], c[2], h[0], 0., 0., 0., h[1], 0., 0., 0., h[2]]
    }
}

fn numbers<const N: usize>(value: &Value, label: &str) -> Result<[f64; N], String> {
    let values = value
        .as_array()
        .filter(|a| a.len() == N)
        .ok_or_else(|| format!("invalid merge {label}"))?;
    let mut result = [0.0; N];
    for (i, v) in values.iter().enumerate() {
        result[i] = v
            .as_f64()
            .filter(|n| n.is_finite())
            .ok_or_else(|| format!("invalid merge {label}"))?;
    }
    Ok(result)
}

fn root_bounds(source: &Value) -> Result<(Bounds, f64), String> {
    let root = source.get("root").ok_or("merge input missing root")?;
    if !matches!(
        root.get("refine").and_then(Value::as_str),
        Some("ADD" | "REPLACE")
    ) {
        return Err("merge input root requires explicit refine ADD or REPLACE".into());
    }
    let transform = if let Some(t) = root.get("transform") {
        numbers::<16>(t, "transform")?
    } else {
        [
            1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1., 0., 0., 0., 0., 1.,
        ]
    };
    if transform[3] != 0. || transform[7] != 0. || transform[11] != 0. || transform[15] != 1. {
        return Err("merge root transform must be affine".into());
    }
    let point = |p: [f64; 3]| -> [f64; 3] {
        std::array::from_fn(|r| {
            transform[r] * p[0]
                + transform[4 + r] * p[1]
                + transform[8 + r] * p[2]
                + transform[12 + r]
        })
    };
    let bv = root
        .get("boundingVolume")
        .ok_or("merge input root missing boundingVolume")?;
    let bounds = if let Some(b) = bv.get("box") {
        let b = numbers::<12>(b, "box")?;
        let mut bounds = Bounds::point(point([b[0], b[1], b[2]]));
        for x in [-1., 1.] {
            for y in [-1., 1.] {
                for z in [-1., 1.] {
                    bounds = bounds.union(Bounds::point(point(std::array::from_fn(|i| {
                        b[i] + x * b[3 + i] + y * b[6 + i] + z * b[9 + i]
                    }))));
                }
            }
        }
        bounds
    } else if let Some(s) = bv.get("sphere") {
        let s = numbers::<4>(s, "sphere")?;
        if s[3] < 0. {
            return Err("merge sphere radius must be nonnegative".into());
        }
        let center = point([s[0], s[1], s[2]]);
        let extent: [f64; 3] = std::array::from_fn(|r| {
            s[3] * (transform[r].powi(2) + transform[4 + r].powi(2) + transform[8 + r].powi(2))
                .sqrt()
        });
        Bounds {
            min: std::array::from_fn(|i| center[i] - extent[i]),
            max: std::array::from_fn(|i| center[i] + extent[i]),
        }
    } else if let Some(r) = bv.get("region") {
        // Geographic region bounds are already in EPSG:4979; tile transforms do not apply.
        region_bounds(numbers::<6>(r, "region")?)?
    } else {
        return Err("merge supports box, sphere or WGS84 region bounds".into());
    };
    let ge = source
        .get("geometricError")
        .and_then(Value::as_f64)
        .ok_or("merge input missing geometricError")?;
    let root_ge = root
        .get("geometricError")
        .and_then(Value::as_f64)
        .ok_or("merge root missing geometricError")?;
    let norm1 = (0..3)
        .map(|c| (0..3).map(|r| transform[c * 4 + r].abs()).sum::<f64>())
        .fold(0., f64::max);
    let norm_inf = (0..3)
        .map(|r| (0..3).map(|c| transform[c * 4 + r].abs()).sum::<f64>())
        .fold(0., f64::max);
    let error = ge.max(root_ge) * (norm1 * norm_inf).sqrt();
    if ge < 0.
        || root_ge < 0.
        || !error.is_finite()
        || bounds
            .min
            .iter()
            .chain(bounds.max.iter())
            .any(|n| !n.is_finite())
    {
        return Err("merge bounds or geometricError are invalid or overflow".into());
    }
    Ok((bounds, error))
}

fn region_bounds(r: [f64; 6]) -> Result<Bounds, String> {
    use std::f64::consts::{FRAC_PI_2, PI, TAU};
    if r[0].abs() > PI
        || r[2].abs() > PI
        || r[1].abs() > FRAC_PI_2
        || r[3].abs() > FRAC_PI_2
        || r[1] > r[3]
        || r[4] > r[5]
    {
        return Err("invalid merge WGS84 region".into());
    }
    fn sin_range(lo: f64, hi: f64) -> [f64; 2] {
        let mut range = [lo.sin().min(hi.sin()), lo.sin().max(hi.sin())];
        for k in -4..=4 {
            let t = std::f64::consts::FRAC_PI_2 + k as f64 * std::f64::consts::PI;
            if t >= lo && t <= hi {
                range[0] = range[0].min(t.sin());
                range[1] = range[1].max(t.sin());
            }
        }
        range
    }
    fn product(a: [f64; 2], b: [f64; 2]) -> [f64; 2] {
        let values = [a[0] * b[0], a[0] * b[1], a[1] * b[0], a[1] * b[1]];
        [
            values.into_iter().fold(f64::INFINITY, f64::min),
            values.into_iter().fold(f64::NEG_INFINITY, f64::max),
        ]
    }
    let east = if r[2] < r[0] { r[2] + TAU } else { r[2] };
    let sin_lat = sin_range(r[1], r[3]);
    let cos_lat = sin_range(r[1] + FRAC_PI_2, r[3] + FRAC_PI_2);
    const A: f64 = 6_378_137.;
    const E2: f64 = 6.6943799901413165e-3;
    // Interval arithmetic conservatively encloses the ellipsoid, including poles/date-line crossings.
    let min_sin_squared = if sin_lat[0] <= 0. && sin_lat[1] >= 0. {
        0.
    } else {
        sin_lat[0].powi(2).min(sin_lat[1].powi(2))
    };
    let max_sin_squared = sin_lat[0].powi(2).max(sin_lat[1].powi(2));
    let n = [
        A / (1. - E2 * min_sin_squared).sqrt(),
        A / (1. - E2 * max_sin_squared).sqrt(),
    ];
    let radial = product([n[0] + r[4], n[1] + r[5]], cos_lat);
    let x = product(radial, sin_range(r[0] + FRAC_PI_2, east + FRAC_PI_2));
    let y = product(radial, sin_range(r[0], east));
    let z = product([n[0] * (1. - E2) + r[4], n[1] * (1. - E2) + r[5]], sin_lat);
    Ok(Bounds {
        min: [x[0], y[0], z[0]],
        max: [x[1], y[1], z[1]],
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_invalid_bounds_and_unsupported_features() {
        let source = json!({"geometricError": 1, "root":{"refine":"REPLACE", "geometricError":0, "boundingVolume":{"sphere":[0,0,0,-1]}}});
        assert!(root_bounds(&source).is_err());
        for value in [
            json!({"implicitTiling":{}}),
            json!({"contents":[]}),
            json!({"extensionsRequired":["unknown"]}),
            json!({"asset":{"version":"1.1"},"root":{},"schemaUri":"schema.json"}),
        ] {
            assert!(check_tileset_features(&value).is_err());
        }
        assert!(check_tileset_features(&json!({"asset":{"version":"2.0"},"buffers":[]})).is_ok());
        assert!(input_paths("first", &json!({"merge":{"additionalInputs":[]}})).is_err());
        assert!(input_paths(
            "first",
            &json!({"merge":{"additionalInputs":vec!["path";64]}})
        )
        .is_err());
    }

    #[test]
    fn binary_external_texture_dependencies_are_checked_in_glb_and_b3dm() {
        let temp = tempfile::tempdir().unwrap();
        let text = br#"{"asset":{"version":"2.0"},"images":[{"uri":"missing.png"}]}"#;
        let mut glb = Vec::new();
        glb.extend_from_slice(b"glTF");
        glb.extend_from_slice(&2_u32.to_le_bytes());
        glb.extend_from_slice(&((20 + text.len()) as u32).to_le_bytes());
        glb.extend_from_slice(&(text.len() as u32).to_le_bytes());
        glb.extend_from_slice(b"JSON");
        glb.extend_from_slice(text);
        let glb_path = temp.path().join("tile.glb");
        fs::write(&glb_path, &glb).unwrap();
        let mut b3dm = Vec::new();
        b3dm.extend_from_slice(b"b3dm");
        b3dm.extend_from_slice(&1_u32.to_le_bytes());
        b3dm.extend_from_slice(&((28 + glb.len()) as u32).to_le_bytes());
        b3dm.extend_from_slice(&[0_u8; 16]);
        b3dm.extend_from_slice(&glb);
        let b3dm_path = temp.path().join("tile.b3dm");
        fs::write(&b3dm_path, b3dm).unwrap();
        for path in [&glb_path, &b3dm_path] {
            assert!(check_binary_references(path, temp.path())
                .unwrap_err()
                .contains("missing.png"));
        }
        fs::write(temp.path().join("missing.png"), b"dependency").unwrap();
        for path in [&glb_path, &b3dm_path] {
            check_binary_references(path, temp.path()).unwrap();
        }
    }

    #[test]
    fn polar_region_is_finite_and_small_regions_do_not_expand_to_global_bounds() {
        let polar = region_bounds([-3., 1.5, 3., std::f64::consts::FRAC_PI_2, 0., 100.]).unwrap();
        assert!(polar.max[2] >= 6_356_752.314245);
        assert!(polar
            .min
            .iter()
            .chain(polar.max.iter())
            .all(|n| n.is_finite()));
        let small = region_bounds([0., 0., 0.00001, 0.00001, 0., 10.]).unwrap();
        assert!(small.max[0] - small.min[0] < 11.);
        assert!(small.max[1] - small.min[1] < 65.);
    }

    #[test]
    fn bounds_apply_transform_once_and_scale_error() {
        let value = json!({"geometricError": 8, "root": {"geometricError": 4, "refine": "REPLACE",
            "boundingVolume": {"box": [0,0,0,1,0,0,0,2,0,0,0,3]},
            "transform": [0,2,0,0,-3,0,0,0,0,0,4,0,100,200,300,1]}});
        let (b, ge) = root_bounds(&value).unwrap();
        assert_eq!(b.min, [94., 198., 288.]);
        assert_eq!(b.max, [106., 202., 312.]);
        assert_eq!(ge, 32.);
    }

    #[test]
    fn region_contains_ecef_samples_across_date_line_and_ignores_transform() {
        let r = [3.0, -0.2, -3.0, 0.4, -100., 500.];
        let b = region_bounds(r).unwrap();
        for lon in [3.0_f64, 3.14, -3.14, -3.0] {
            for lat in [-0.2_f64, 0., 0.4] {
                for h in [-100., 500.] {
                    let n = 6_378_137. / (1. - 6.6943799901413165e-3 * lat.sin().powi(2)).sqrt();
                    let p = [
                        (n + h) * lat.cos() * lon.cos(),
                        (n + h) * lat.cos() * lon.sin(),
                        (n * (1. - 6.6943799901413165e-3) + h) * lat.sin(),
                    ];
                    for i in 0..3 {
                        assert!(p[i] >= b.min[i] - 1e-8 && p[i] <= b.max[i] + 1e-8);
                    }
                }
            }
        }
        let value = json!({"geometricError": 1, "root": {"geometricError": 0, "refine": "REPLACE", "boundingVolume": {"region": r},
            "transform": [1,0,0,0,0,1,0,0,0,0,1,0,1e8,1e8,1e8,1]}});
        assert_eq!(root_bounds(&value).unwrap().0.min, b.min);
    }

    #[test]
    fn sphere_bounds_cover_nonuniform_scale_and_shear() {
        let value = json!({"geometricError": 1, "root": {"geometricError": 0, "refine": "REPLACE", "boundingVolume": {"sphere": [0,0,0,2]},
            "transform": [2,0,0,0,1,3,0,0,0,0,4,0,5,6,7,1]}});
        let b = root_bounds(&value).unwrap().0;
        assert_eq!(b.min[2], -1.);
        assert!((b.max[0] - (5. + 2. * 5_f64.sqrt())).abs() < 1e-10);
    }

    #[test]
    fn copy_cancel_and_unsafe_uri_are_rejected() {
        let temp = tempfile::tempdir().unwrap();
        let source = temp.path().join("input");
        fs::create_dir(&source).unwrap();
        fs::write(source.join("tile.bin"), b"untouched").unwrap();
        let flag = CancelFlag::new();
        flag.request();
        let target = temp.path().join("output");
        assert_eq!(
            copy_tree(&source, &target, &source, &flag, 0).unwrap_err(),
            "cancelled"
        );
        assert!(!target.exists());
        for uri in [
            "../outside.bin",
            "https://example.com/a.glb",
            "/absolute.glb",
            "a%20b.glb",
        ] {
            assert!(check_references(&json!({"content":{"uri":uri}}), &source, &source).is_err());
        }
        assert_eq!(fs::read(source.join("tile.bin")).unwrap(), b"untouched");
    }
}
