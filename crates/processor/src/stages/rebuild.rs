//! Top rebuild stage — default Rust `top_rebuild` core (Phase 9).
//!
//! Python `rebuild_top.py` remains available only when
//! `GEOFORGE_REBUILD_ENGINE=python` (regression / baseline).

use crate::cancel::CancelFlag;
use crate::protocol::{Emitter, Stage};
use crate::util::{run_logged, tool_paths};
use serde_json::{json, Value};
use std::path::Path;
use std::sync::Arc;

fn rebuild_engine() -> String {
    std::env::var("GEOFORGE_REBUILD_ENGINE")
        .unwrap_or_else(|_| "rust".into())
        .to_ascii_lowercase()
}

pub fn run_rebuild(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    input_dir: &Path,
    output_dir: &Path,
    rebuild: &Value,
) -> Result<(), String> {
    precheck_rebuild_input(emitter, input_dir)?;
    let rebuild = apply_quality_preset(rebuild);

    if output_dir.exists() {
        std::fs::remove_dir_all(output_dir).map_err(|e| e.to_string())?;
    }

    let engine = rebuild_engine();
    if engine == "python" || engine == "py" || engine == "baseline" {
        run_rebuild_python(emitter, cancel, input_dir, output_dir, &rebuild)
    } else {
        run_rebuild_rust(emitter, cancel, input_dir, output_dir, &rebuild)
    }
}

fn apply_quality_preset(rebuild: &Value) -> Value {
    let mut obj = rebuild.as_object().cloned().unwrap_or_default();
    let quality = obj
        .get("quality")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if obj.get("l1MaxTriangles").is_none() && obj.get("l1_max_triangles").is_none() {
        let (l1, l2) = match quality.as_str() {
            "quality" => (8000u64, 4000u64),
            "speed" => (2000, 1000),
            "balanced" => (4000, 2000),
            _ => (4000, 2000),
        };
        if !quality.is_empty() {
            obj.insert("l1MaxTriangles".into(), json!(l1));
            obj.insert("l2MaxTriangles".into(), json!(l2));
        }
    }
    Value::Object(obj)
}

/// Reject sparse / non Tile_* grids before invoking the engine (T08).
fn precheck_rebuild_input(emitter: &Arc<Emitter>, input_dir: &Path) -> Result<(), String> {
    let tileset = if input_dir.is_file() {
        input_dir.to_path_buf()
    } else {
        input_dir.join("tileset.json")
    };
    if !tileset.is_file() {
        return Err(format!(
            "rebuild precheck: tileset.json missing at {}",
            tileset.display()
        ));
    }
    emitter.log(&format!("[rebuild] precheck {}", tileset.display()));

    // Collect Tile_+X_+Y style folder names under input
    let root = tileset.parent().unwrap_or(input_dir);
    let re = regex::Regex::new(r"(?i)^Tile_([+\-]?\d+)_([+\-]?\d+)$").unwrap();
    let mut coords: Vec<(i32, i32)> = Vec::new();
    let mut nonstandard_tiles = Vec::new();
    let entries = std::fs::read_dir(root)
        .map_err(|error| format!("rebuild precheck: cannot read {}: {error}", root.display()))?;
    for entry in entries {
        let entry = entry.map_err(|error| {
            format!(
                "rebuild precheck: cannot read an entry under {}: {error}",
                root.display()
            )
        })?;
        if !entry
            .file_type()
            .map_err(|error| {
                format!(
                    "rebuild precheck: cannot inspect {}: {error}",
                    entry.path().display()
                )
            })?
            .is_dir()
        {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        if let Some(c) = re.captures(&name) {
            let x: i32 = c[1].parse().map_err(|_| {
                format!("rebuild precheck: Tile coordinate out of i32 range: {name}")
            })?;
            let y: i32 = c[2].parse().map_err(|_| {
                format!("rebuild precheck: Tile coordinate out of i32 range: {name}")
            })?;
            coords.push((x, y));
        } else if entry.path().join("tileset.json").is_file() {
            nonstandard_tiles.push(name);
        }
    }
    if !nonstandard_tiles.is_empty() {
        return Err(format!(
            "rebuild precheck: unsupported non-standard Tile layout: {}",
            nonstandard_tiles.join(", ")
        ));
    }
    if coords.is_empty() {
        // External tileset refs may use nested structure — let engine fail with its own message
        emitter.log("[rebuild] precheck: no Tile_* siblings; deferring to top_rebuild");
        return Ok(());
    }
    coords.sort();
    coords.dedup();
    let min_x = coords.iter().map(|c| c.0).min().unwrap();
    let max_x = coords.iter().map(|c| c.0).max().unwrap();
    let min_y = coords.iter().map(|c| c.1).min().unwrap();
    let max_y = coords.iter().map(|c| c.1).max().unwrap();
    let w = (max_x - min_x + 1) as usize;
    let h = (max_y - min_y + 1) as usize;
    let expected = w.saturating_mul(h);
    if coords.len() != expected {
        return Err(format!(
            "不支持的数据布局：检测到稀疏或不连续的 Tile 网格（有 {} 块，矩形范围期望 {} = {}×{}）。V1 顶层重建仅支持规则块数据。",
            coords.len(),
            expected,
            w,
            h
        ));
    }
    if w > 16 || h > 16 {
        emitter.log(&format!(
            "[rebuild] warning: grid {w}×{h} exceeds documented 16×16 verification envelope"
        ));
    }
    emitter.log(&format!(
        "[rebuild] precheck OK continuous grid {w}×{h} ({} tiles)",
        coords.len()
    ));
    Ok(())
}

fn run_rebuild_rust(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    input_dir: &Path,
    output_dir: &Path,
    rebuild: &Value,
) -> Result<(), String> {
    let tools = tool_paths();
    let bin = &tools.top_rebuild;
    if !bin.is_file() {
        return Err(format!(
            "top_rebuild binary not found: {} (set GEOFORGE_TOP_REBUILD or cargo build -p top_rebuild --bin top_rebuild). \
             For Python baseline only: GEOFORGE_REBUILD_ENGINE=python",
            bin.display()
        ));
    }

    let mut cmd = vec![
        bin.to_string_lossy().into_owned(),
        "-i".into(),
        input_dir.to_string_lossy().into_owned(),
        "-o".into(),
        output_dir.to_string_lossy().into_owned(),
    ];

    // levels: rust max_levels = merge passes beyond L0.
    // levels <= 0 means "until single root" (omit --levels so top_rebuild builds full pyramid).
    // Positive values clamp at caller; N×N continuous grid needs ~log2(N) merges.
    if let Some(levels) = rebuild.get("levels").and_then(|v| v.as_i64()) {
        if levels > 0 {
            cmd.push("--levels".into());
            cmd.push((levels as u32).to_string());
        }
    }

    if let Some(r) = rebuild
        .get("sourceErrorRatio")
        .or_else(|| rebuild.get("source_error_ratio"))
        .and_then(|v| v.as_f64())
    {
        cmd.push("--source-error-ratio".into());
        cmd.push(r.to_string());
    }

    if let Some(n) = rebuild
        .get("l1MaxTriangles")
        .or_else(|| rebuild.get("l1_max_triangles"))
        .and_then(|v| v.as_u64())
    {
        cmd.push("--l1-max-triangles".into());
        cmd.push(n.to_string());
    }
    if let Some(n) = rebuild
        .get("l2MaxTriangles")
        .or_else(|| rebuild.get("l2_max_triangles"))
        .and_then(|v| v.as_u64())
    {
        cmd.push("--l2-max-triangles".into());
        cmd.push(n.to_string());
    }
    if let Some(e) = rebuild
        .get("targetError")
        .or_else(|| rebuild.get("target_error"))
        .and_then(|v| v.as_f64())
    {
        cmd.push("--target-error".into());
        cmd.push(e.to_string());
    }
    if let Some(n) = rebuild
        .get("maxTextureSize")
        .or_else(|| rebuild.get("max_texture_size"))
        .and_then(|v| v.as_u64())
    {
        cmd.push("--max-texture-size".into());
        cmd.push(n.to_string());
    }
    if let Some(n) = rebuild
        .get("maxTextureBytes")
        .or_else(|| rebuild.get("max_texture_bytes"))
        .and_then(|v| v.as_u64())
    {
        cmd.push("--max-texture-bytes".into());
        cmd.push(n.to_string());
    }
    if let Some(n) = rebuild
        .get("maxGlbBytes")
        .or_else(|| rebuild.get("max_glb_bytes"))
        .and_then(|v| v.as_u64())
    {
        cmd.push("--max-glb-bytes".into());
        cmd.push(n.to_string());
    }

    let ktx2 = rebuild
        .get("ktx2")
        .or_else(|| rebuild.get("enableKtx2"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    if ktx2 {
        cmd.push("--ktx2".into());
    }

    // Map legacy textureScale → maxTextureSize hint when size not set
    if rebuild.get("maxTextureSize").is_none() && rebuild.get("max_texture_size").is_none() {
        if let Some(scale) = rebuild
            .get("textureScale")
            .or_else(|| rebuild.get("texture_scale"))
            .and_then(|v| v.as_f64())
        {
            let size = ((1024.0 * scale).round() as u32).clamp(64, 4096);
            cmd.push("--max-texture-size".into());
            cmd.push(size.to_string());
        }
    }

    // Fixture / empty-content path: optional inject (off by default in release CLI)
    if rebuild
        .get("injectTestTextures")
        .or_else(|| rebuild.get("inject_test_textures"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        cmd.push("--inject-test-textures".into());
    }
    if rebuild
        .get("synthesizeIfEmpty")
        .or_else(|| rebuild.get("synthesize_if_empty"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        cmd.push("--synthesize-if-empty".into());
    }

    emitter.stage(Stage::Rebuild, "Top-level rebuild (Rust top_rebuild core)");
    emitter.stage(Stage::RebuildIndex, "rebuild-index (top_rebuild)");
    emitter.stage(Stage::RebuildProxy, "rebuild-proxy (top_rebuild)");
    emitter.log(&format!("[rebuild] engine=rust binary={}", bin.display()));

    let rc = run_logged(emitter, cancel, &cmd, None)?;
    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    if rc != 0 {
        return Err(format!("top_rebuild exited {rc}"));
    }
    if !output_dir.join("tileset.json").is_file() {
        return Err("rebuild output missing tileset.json".into());
    }
    Ok(())
}

fn run_rebuild_python(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    input_dir: &Path,
    output_dir: &Path,
    rebuild: &Value,
) -> Result<(), String> {
    let tools = tool_paths();
    if !tools.rebuild_py.is_file() {
        return Err(format!(
            "rebuild_top.py not found: {}",
            tools.rebuild_py.display()
        ));
    }

    let levels = rebuild.get("levels").and_then(|v| v.as_i64()).unwrap_or(1);
    let levels = if levels <= 1 { 1 } else { 2 };
    let simplify = rebuild
        .get("simplify")
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);
    let texture_scale = rebuild
        .get("textureScale")
        .or_else(|| rebuild.get("texture_scale"))
        .and_then(|v| v.as_f64())
        .unwrap_or(0.5);

    let py = if tools.python.is_file() {
        tools.python.to_string_lossy().into_owned()
    } else {
        "python3".into()
    };

    let cmd = vec![
        py,
        tools.rebuild_py.to_string_lossy().into_owned(),
        "-i".into(),
        input_dir.to_string_lossy().into_owned(),
        "-o".into(),
        output_dir.to_string_lossy().into_owned(),
        "--levels".into(),
        levels.to_string(),
        "--simplify".into(),
        simplify.to_string(),
        "--texture-scale".into(),
        texture_scale.to_string(),
    ];

    emitter.stage(
        Stage::Rebuild,
        "Top-level rebuild (Python baseline — GEOFORGE_REBUILD_ENGINE=python)",
    );
    emitter.stage(Stage::RebuildIndex, "rebuild-index (baseline script)");
    emitter.stage(Stage::RebuildProxy, "rebuild-proxy (baseline script)");
    emitter.log(&format!(
        "[rebuild] engine=python script={}",
        tools.rebuild_py.display()
    ));

    let rc = run_logged(emitter, cancel, &cmd, None)?;
    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    if rc != 0 {
        return Err(format!("rebuild-top exited {rc}"));
    }
    if !output_dir.join("tileset.json").is_file() {
        return Err("rebuild output missing tileset.json".into());
    }
    Ok(())
}

pub fn rebuild_enabled(options: &Value) -> bool {
    options
        .get("rebuildTop")
        .and_then(|v| v.get("enabled"))
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
}

pub fn rebuild_opts(options: &Value) -> Value {
    options
        .get("rebuildTop")
        .cloned()
        .unwrap_or(serde_json::json!({}))
}

#[cfg(test)]
mod tests {
    use super::precheck_rebuild_input;
    use crate::protocol::Emitter;
    use std::fs;
    use std::path::PathBuf;
    use std::sync::Arc;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("geoforge-rebuild-{name}-{stamp}"));
        fs::create_dir_all(&root).expect("create rebuild fixture");
        fs::write(root.join("tileset.json"), b"{}").expect("write tileset");
        root
    }

    #[test]
    fn rejects_nonstandard_tile_layout_instead_of_deferring() {
        let root = temp_root("unicode");
        let tile = root.join("Tile_甲_乙");
        fs::create_dir_all(&tile).expect("create nonstandard tile");
        fs::write(tile.join("tileset.json"), b"{}").expect("write child tileset");

        let emitter = Arc::new(Emitter::new("test-rebuild-layout"));
        let error = precheck_rebuild_input(&emitter, &root).expect_err("layout must fail");
        assert!(error.contains("non-standard"), "{error}");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn rejects_tile_coordinate_outside_i32() {
        let root = temp_root("range");
        let tile = root.join("Tile_2147483648_0");
        fs::create_dir_all(&tile).expect("create out-of-range tile");
        fs::write(tile.join("tileset.json"), b"{}").expect("write child tileset");

        let emitter = Arc::new(Emitter::new("test-rebuild-range"));
        let error = precheck_rebuild_input(&emitter, &root).expect_err("range must fail");
        assert!(error.contains("i32 range"), "{error}");
        let _ = fs::remove_dir_all(root);
    }
}
