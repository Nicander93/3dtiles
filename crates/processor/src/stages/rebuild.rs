//! Top rebuild stage — default Rust `top_rebuild` core (Phase 9).
//!
//! Python `rebuild_top.py` remains available only when
//! `GEOFORGE_REBUILD_ENGINE=python` (regression / baseline).

use crate::cancel::CancelFlag;
use crate::protocol::{Emitter, Stage};
use crate::util::{run_logged, tool_paths};
use serde_json::Value;
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
    if output_dir.exists() {
        std::fs::remove_dir_all(output_dir).map_err(|e| e.to_string())?;
    }

    let engine = rebuild_engine();
    if engine == "python" || engine == "py" || engine == "baseline" {
        run_rebuild_python(emitter, cancel, input_dir, output_dir, rebuild)
    } else {
        run_rebuild_rust(emitter, cancel, input_dir, output_dir, rebuild)
    }
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

    emitter.stage(
        Stage::Rebuild,
        "Top-level rebuild (Rust top_rebuild core)",
    );
    emitter.stage(Stage::RebuildIndex, "rebuild-index (top_rebuild)");
    emitter.stage(Stage::RebuildProxy, "rebuild-proxy (top_rebuild)");
    emitter.log(&format!(
        "[rebuild] engine=rust binary={}",
        bin.display()
    ));

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

    let levels = rebuild
        .get("levels")
        .and_then(|v| v.as_i64())
        .unwrap_or(1);
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
