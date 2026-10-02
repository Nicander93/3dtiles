//! Exact convex geographic cropping of explicit static triangle tilesets.
mod content;
mod math;

use super::{commit, merge, scan, validate};
use crate::{path_policy, CancelFlag, Emitter, Stage, TaskConfig};
use content::{union, Content, Counts};
use math::*;
use serde_json::{json, Value};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

pub fn preflight(
    input_path: &str,
    options: &Value,
    output_path: &Path,
    task_id: &str,
) -> Result<PathBuf, String> {
    let clip = options
        .get("clip")
        .and_then(Value::as_object)
        .ok_or("missing clip options")?;
    if clip.len() != 1 || !clip.contains_key("region") {
        return Err("clip options must contain only region".into());
    }
    Region::parse(&clip["region"])?;
    let input = scan::resolve_tileset(input_path)?;
    let input = path_policy::normalize_path(&input)?;
    let output = path_policy::normalize_path(output_path)?;
    let root = input.parent().ok_or("input has no parent")?;
    let temp = commit::temp_work_dir(&output, task_id);
    for target in [&output, &temp] {
        if target == root
            || path_policy::is_strict_descendant(root, target)
            || path_policy::is_strict_descendant(target, root)
        {
            return Err("clip output or temporary directory must not overlap input root".into());
        }
    }
    path_policy::validate_io_paths(&input, &output, task_id)?;
    Ok(input)
}
pub fn run(config: &TaskConfig, emitter: &Emitter, cancel: &CancelFlag) -> Result<PathBuf, String> {
    emitter.stage(Stage::Scan, "Checking clipping region and input");
    let input = preflight(
        config.input_path(),
        &config.options,
        Path::new(config.output_path()),
        &config.task_id,
    )?;
    let output = path_policy::normalize_path(Path::new(config.output_path()))?;
    let region = Region::parse(&config.options["clip"]["region"])?;
    let temp = commit::prepare_temp(&output, &config.task_id)?;
    let mut guard = commit::TempGuard::new(temp.clone());
    let staged = temp.join("staged");
    fs::create_dir(&staged).map_err(|e| e.to_string())?;
    fs::create_dir(staged.join("content")).map_err(|e| e.to_string())?;
    fs::create_dir(staged.join("resources")).map_err(|e| e.to_string())?;
    emitter.stage(Stage::Clip, "Clipping triangles across all LODs");
    let mut context = Context {
        region,
        source_root: input.parent().unwrap().to_path_buf(),
        output: staged.clone(),
        cancel,
        emitter,
        active: HashSet::new(),
        next: 0,
        resources: 0,
        counts: Counts::default(),
        files: 0,
        removed: 0,
    };
    let (mut source, bounds) = context.tileset(&input, IDENTITY, 0)?;
    if bounds.is_none() {
        return Err("clip removed all geometry; no output committed".into());
    }
    source["asset"]["generator"] = json!("GeoForge exact geographic clip");
    fs::write(
        staged.join("tileset.json"),
        serde_json::to_vec_pretty(&source).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())?;
    fs::write(staged.join("clip-report.json"),serde_json::to_vec_pretty(&json!({"region":config.options["clip"]["region"],"projection":"WGS84 to local ENU; vertical half-planes","contentsProcessed":context.files,"contentsRemoved":context.removed,"trianglesBefore":context.counts.before,"trianglesAfter":context.counts.after,"capsGenerated":false})).map_err(|e|e.to_string())?).map_err(|e|e.to_string())?;
    context.check_cancel()?;
    commit::write_checkpoint(&temp, commit::Checkpoint::Validating)?;
    validate::validate_tileset_dir_cancellable(emitter, &staged, Some(cancel))?;
    commit::write_checkpoint(&temp, commit::Checkpoint::Validated)?;
    context.check_cancel()?;
    commit::write_checkpoint(&temp, commit::Checkpoint::Committing)?;
    commit::commit_rename(emitter, &staged, &temp, &output, Some(&mut guard))?;
    commit::cleanup_temp(&temp);
    Ok(output)
}
struct Context<'a> {
    region: Region,
    source_root: PathBuf,
    output: PathBuf,
    cancel: &'a CancelFlag,
    emitter: &'a Emitter,
    active: HashSet<PathBuf>,
    next: usize,
    resources: usize,
    counts: Counts,
    files: u64,
    removed: u64,
}
impl Context<'_> {
    fn check_cancel(&self) -> Result<(), String> {
        if self.cancel.is_cancelled() {
            Err("cancelled".into())
        } else {
            Ok(())
        }
    }
    fn reference(&self, base: &Path, uri: &str) -> Result<PathBuf, String> {
        if uri.is_empty() || uri.starts_with('/') || uri.contains([':', '?', '#', '%', '\\']) {
            return Err(format!("clip requires plain local relative URI: {uri}"));
        }
        let path = path_policy::normalize_path(&base.join(uri))?;
        if !path.is_file() || !path_policy::is_strict_descendant(&self.source_root, &path) {
            return Err(format!(
                "clip resource missing or outside input root: {uri}"
            ));
        }
        Ok(path)
    }
    fn read(&self, path: &Path) -> Result<Vec<u8>, String> {
        self.check_cancel()?;
        if fs::metadata(path).map_err(|e| e.to_string())?.len() > 128 * 1024 * 1024 {
            return Err(format!("clip content exceeds 128 MiB: {}", path.display()));
        }
        fs::read(path).map_err(|e| e.to_string())
    }
    fn tileset(
        &mut self,
        path: &Path,
        parent: Matrix,
        depth: usize,
    ) -> Result<(Value, Option<Bounds>), String> {
        if depth > 128 || !self.active.insert(path.to_path_buf()) {
            return Err("cyclic or deeply nested external tileset".into());
        }
        let mut source: Value =
            serde_json::from_slice(&self.read(path)?).map_err(|e| e.to_string())?;
        merge::check_tileset_features(&source).map_err(|e| e.replace("merge", "clip"))?;
        if source["asset"].get("gltfUpAxis").is_some_and(|v| v != "Y") {
            return Err("clip supports glTF Y-up tilesets only".into());
        }
        let error = source["geometricError"]
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.)
            .ok_or("invalid tileset geometricError")?;
        if !matches!(source["root"]["refine"].as_str(), Some("ADD" | "REPLACE")) {
            return Err("tileset root requires explicit refine".into());
        }
        let prefix = if self.active.len() == 1 { "" } else { "../" };
        let bounds = self.tile(
            &mut source["root"],
            path.parent().ok_or("tileset has no directory")?,
            parent,
            depth + 1,
            prefix,
        )?;
        source["geometricError"] =
            json!(error.max(source["root"]["geometricError"].as_f64().unwrap_or(0.)));
        self.active.remove(path);
        Ok((source, bounds))
    }
    fn tile(
        &mut self,
        tile: &mut Value,
        base: &Path,
        parent: Matrix,
        depth: usize,
        prefix: &str,
    ) -> Result<Option<Bounds>, String> {
        self.check_cancel()?;
        if depth > 128 {
            return Err("tile nesting exceeds 128".into());
        }
        let local = transform(tile.get("transform"))?;
        let world = multiply(parent, local);
        let mut error = tile["geometricError"]
            .as_f64()
            .filter(|n| n.is_finite() && *n >= 0.)
            .ok_or("invalid tile geometricError")?;
        let mut bounds = None;
        if let Some(value) = tile.get("content").cloned() {
            let uri = value
                .get("uri")
                .or_else(|| value.get("url"))
                .and_then(Value::as_str)
                .ok_or("tile content requires URI")?;
            let path = self.reference(base, uri)?;
            self.next += 1;
            let id = self.next;
            let ext = path
                .extension()
                .and_then(|v| v.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            let (new_uri, result) = if ext == "json" {
                let (source, b) = self.tileset(&path, world, depth + 1)?;
                let root_transform = transform(source["root"].get("transform"))?;
                let scale = scale_bound(root_transform);
                error = error.max(source["geometricError"].as_f64().unwrap_or(0.) * scale);
                let relative = format!("content/tileset-{id:06}.json");
                if b.is_some() {
                    fs::write(
                        self.output.join(&relative),
                        serde_json::to_vec_pretty(&source).map_err(|e| e.to_string())?,
                    )
                    .map_err(|e| e.to_string())?;
                }
                (relative, b)
            } else if matches!(ext.as_str(), "glb" | "b3dm") {
                let mut content = Content::parse(&self.read(&path)?)?;
                if content.b3dm != (ext == "b3dm") {
                    return Err("content magic does not match file extension".into());
                }
                if let Some(images) = content.document["images"].as_array() {
                    for image in images {
                        if let Some(uri) = image["uri"].as_str() {
                            if !uri.starts_with("data:") {
                                self.reference(path.parent().unwrap(), uri)?;
                            }
                        }
                    }
                }
                let b = content::crop(
                    &mut content,
                    &self.region,
                    world,
                    self.cancel,
                    &mut self.counts,
                )?;
                let relative = format!("content/model-{id:06}.{ext}");
                if b.is_some() {
                    self.images(&mut content.document, path.parent().unwrap())?;
                    fs::write(self.output.join(&relative), content.bytes()?)
                        .map_err(|e| e.to_string())?;
                }
                self.files += 1;
                self.emitter.progress(Stage::Clip, self.files, 0);
                (relative, b)
            } else {
                return Err(format!(
                    "clip supports explicit GLB/B3DM or external JSON, found {ext}"
                ));
            };
            if let Some(b) = result {
                let mut c = value;
                c.as_object_mut().ok_or("invalid content")?.remove("url");
                // Generated external tilesets live one level below the outer entry.
                c["uri"] = json!(format!("{prefix}{new_uri}"));
                if c.get("boundingVolume").is_some() {
                    c["boundingVolume"] = b.json();
                }
                tile["content"] = c;
                bounds = union(bounds, Some(b));
            } else {
                tile.as_object_mut()
                    .ok_or("invalid tile")?
                    .remove("content");
                self.removed += 1;
            }
        }
        if let Some(children) = tile.get("children").and_then(Value::as_array).cloned() {
            let mut kept = Vec::new();
            for mut child in children {
                let child_transform = transform(child.get("transform"))?;
                if let Some(b) = self.tile(&mut child, base, world, depth + 1, prefix)? {
                    let scale = scale_bound(child_transform);
                    error = error.max(child["geometricError"].as_f64().unwrap_or(0.) * scale);
                    bounds = union(bounds, Some(b));
                    kept.push(child);
                }
            }
            if kept.is_empty() {
                tile.as_object_mut()
                    .ok_or("invalid tile")?
                    .remove("children");
            } else {
                tile["children"] = json!(kept);
            }
        }
        if let Some(b) = bounds {
            tile["boundingVolume"] = b.json();
            tile["geometricError"] = json!(error);
        }
        Ok(bounds.map(|b| b.transformed(local)))
    }
    fn images(&mut self, doc: &mut Value, base: &Path) -> Result<(), String> {
        if let Some(images) = doc["images"].as_array_mut() {
            for image in images {
                if let Some(uri) = image["uri"].as_str() {
                    if uri.starts_with("data:") {
                        continue;
                    }
                    let source = self.reference(base, uri)?;
                    let bytes = self.read(&source)?;
                    self.resources += 1;
                    let extension = source
                        .extension()
                        .and_then(|e| e.to_str())
                        .filter(|e| e.chars().all(|c| c.is_ascii_alphanumeric()))
                        .unwrap_or("bin");
                    let name = format!("image-{:06}.{extension}", self.resources);
                    fs::write(self.output.join("resources").join(&name), bytes)
                        .map_err(|e| e.to_string())?;
                    image["uri"] = json!(format!("../resources/{name}"));
                }
            }
        }
        Ok(())
    }
}
fn scale_bound(m: Matrix) -> f64 {
    let one = (0..3)
        .map(|c| (0..3).map(|r| m[c * 4 + r].abs()).sum::<f64>())
        .fold(0., f64::max);
    let inf = (0..3)
        .map(|r| (0..3).map(|c| m[c * 4 + r].abs()).sum::<f64>())
        .fold(0., f64::max);
    (one * inf).sqrt()
}
