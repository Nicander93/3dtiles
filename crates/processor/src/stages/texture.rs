//! Texture stage: keep = skip; KTX2 via Rust+basisu (Phase 14).
//! Python texture_ktx2 only when `GEOFORGE_TEXTURE_ENGINE=python`.

use crate::cancel::CancelFlag;
use crate::protocol::{Emitter, Stage};
use crate::stages::texture_ktx2;
use crate::util::{run_logged, tool_paths};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

const KEEP: &[&str] = &["keep", "none", "", "passthrough"];

pub fn normalize_mode(mode: Option<&str>) -> String {
    let m = mode.unwrap_or("keep").to_lowercase();
    let m = m.trim();
    if KEEP.contains(&m) {
        return "keep".into();
    }
    if matches!(m, "ktx2" | "ktx2-etc1s" | "etc1s") {
        return "ktx2-etc1s".into();
    }
    if matches!(m, "ktx2-uastc" | "uastc") {
        return "ktx2-uastc".into();
    }
    m.to_string()
}

pub fn is_keep(mode: &str) -> bool {
    normalize_mode(Some(mode)) == "keep"
}

fn texture_engine() -> String {
    std::env::var("GEOFORGE_TEXTURE_ENGINE")
        .unwrap_or_else(|_| "rust".into())
        .to_ascii_lowercase()
}

pub fn finish_texture(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    out_dir: &Path,
    tex_mode: &str,
) -> Result<(), String> {
    let mode = normalize_mode(Some(tex_mode));
    if is_keep(&mode) {
        emitter.stage_extra(
            Stage::Texture,
            "skipped (keep)",
            serde_json::json!({ "skipped": true, "textureMode": "keep" }),
        );
        emitter.log("[texture] mode=keep — skipped (no Python)");
        return Ok(());
    }

    emitter.stage_extra(
        Stage::Texture,
        &format!("post-process basisu mode={mode}"),
        serde_json::json!({ "textureMode": mode, "postprocess": true }),
    );

    let engine = texture_engine();
    if engine == "python" || engine == "py" || engine == "baseline" {
        finish_texture_python(emitter, cancel, out_dir, &mode)?;
    } else {
        finish_texture_rust(emitter, cancel, out_dir, &mode)?;
    }

    let found = walk_has_ktx2(out_dir);
    emitter.log(&format!("[texture] evidence found={found}"));
    if !found {
        return Err(format!(
            "KTX2 mode={mode} was requested, but no .ktx2 evidence found under {}",
            out_dir.display()
        ));
    }
    emitter.stage_extra(
        Stage::Texture,
        &format!("KTX2 applied ({mode})"),
        serde_json::json!({ "textureMode": mode, "postprocess": true, "engine": engine }),
    );
    emitter.log(&format!("[texture] OK mode={mode} engine={engine}"));
    Ok(())
}

fn finish_texture_rust(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    out_dir: &Path,
    mode: &str,
) -> Result<(), String> {
    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    let tools = tool_paths();
    let basisu = if tools.basisu.is_file() {
        tools.basisu.clone()
    } else if let Ok(p) = std::env::var("GEOFORGE_BASISU") {
        let pb = std::path::PathBuf::from(p);
        if pb.is_file() {
            pb
        } else {
            return Err(format!(
                "KTX2 mode={mode} requested but basisu not found (GEOFORGE_BASISU={})",
                pb.display()
            ));
        }
    } else {
        return Err(format!(
            "KTX2 mode={mode} requested but basisu sidecar not found (set GEOFORGE_BASISU or bundle binaries/basisu). \
             Python fallback only with GEOFORGE_TEXTURE_ENGINE=python"
        ));
    };

    emitter.log(&format!(
        "[texture] engine=rust basisu={} mode={mode}",
        basisu.display()
    ));
    let mut logs: Vec<String> = Vec::new();
    let stats = texture_ktx2::process_tileset_dir(out_dir, mode, &basisu, 128, |m| {
        logs.push(m.to_string());
    })?;
    for line in logs {
        emitter.log(&line);
        if cancel.is_cancelled() {
            return Err("cancelled".into());
        }
    }
    emitter.log(&format!(
        "[texture] rust ktx2 stats filesSeen={} converted={} textures={} errors={}",
        stats.files_seen,
        stats.files_converted,
        stats.textures_converted,
        stats.errors.len()
    ));
    if !stats.errors.is_empty() && stats.textures_converted == 0 {
        return Err(format!(
            "texture post-process failed: {}",
            stats.errors.join("; ")
        ));
    }
    Ok(())
}

fn finish_texture_python(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    out_dir: &Path,
    mode: &str,
) -> Result<(), String> {
    let tools = tool_paths();
    let py = if tools.python.is_file() {
        tools.python.to_string_lossy().into_owned()
    } else {
        "python3".into()
    };

    let mut cmd = vec![py];
    let exp_mod = tools
        .repo_root
        .join("tools/experiments/desktop_server_py/app/texture_ktx2.py");
    if tools.texture_py.is_file() {
        cmd.push(tools.texture_py.to_string_lossy().into_owned());
        cmd.push("-i".into());
        cmd.push(out_dir.to_string_lossy().into_owned());
        cmd.push("--mode".into());
        cmd.push(mode.into());
    } else if exp_mod.is_file() {
        cmd.push(exp_mod.to_string_lossy().into_owned());
        cmd.push("-i".into());
        cmd.push(out_dir.to_string_lossy().into_owned());
        cmd.push("--mode".into());
        cmd.push(mode.into());
    } else {
        return Err(format!(
            "GEOFORGE_TEXTURE_ENGINE=python but texture_ktx2 script not found"
        ));
    }

    emitter.log(&format!(
        "[texture] engine=python (experiments only) script={}",
        cmd.get(1).cloned().unwrap_or_default()
    ));
    let cwd = if tools.repo_root.as_os_str().is_empty() {
        None
    } else {
        Some(tools.repo_root.as_path())
    };
    let rc = run_logged(emitter, cancel, &cmd, cwd)?;
    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    if rc != 0 {
        return Err(format!("texture post-process exited {rc}"));
    }
    Ok(())
}

fn walk_has_ktx2(dir: &Path) -> bool {
    fn file_has_ktx2_evidence(p: &Path) -> bool {
        let ext = p
            .extension()
            .and_then(|x| x.to_str())
            .unwrap_or("")
            .to_ascii_lowercase();
        if ext == "ktx2" {
            return true;
        }
        if !matches!(ext.as_str(), "b3dm" | "glb" | "gltf") {
            return false;
        }
        let Ok(bytes) = std::fs::read(p) else {
            return false;
        };
        // Embedded KTX2 magic or glTF mime / extension markers.
        if bytes.windows(4).any(|w| w == b"KTX ") {
            return true;
        }
        if let Ok(s) = std::str::from_utf8(&bytes) {
            if s.contains("image/ktx2") || s.contains("KHR_texture_basisu") {
                return true;
            }
        } else {
            // GLB JSON chunk is ASCII-ish; search raw
            let needle1 = b"image/ktx2";
            let needle2 = b"KHR_texture_basisu";
            if bytes.windows(needle1.len()).any(|w| w == needle1)
                || bytes.windows(needle2.len()).any(|w| w == needle2)
            {
                return true;
            }
        }
        false
    }
    fn rec(d: &Path, depth: u32) -> bool {
        if depth > 12 {
            return false;
        }
        let Ok(rd) = std::fs::read_dir(d) else {
            return false;
        };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                if rec(&p, depth + 1) {
                    return true;
                }
            } else if file_has_ktx2_evidence(&p) {
                return true;
            }
        }
        false
    }
    rec(dir, 0)
}

pub fn texture_mode_from_options(options: &Value) -> String {
    normalize_mode(
        options
            .get("texture")
            .and_then(|t| t.get("mode"))
            .and_then(|v| v.as_str()),
    )
}
