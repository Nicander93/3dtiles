//! Texture stage: keep = skip; else call Python texture_ktx2 / basisu postprocess.

use crate::cancel::CancelFlag;
use crate::protocol::{Emitter, Stage};
use std::sync::Arc;
use crate::util::{run_logged, tool_paths};
use serde_json::Value;
use std::path::Path;

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
        emitter.log("[texture] mode=keep — skipped");
        return Ok(());
    }

    emitter.stage_extra(
        Stage::Texture,
        &format!("post-process basisu mode={mode}"),
        serde_json::json!({ "textureMode": mode, "postprocess": true }),
    );

    let tools = tool_paths();
    // Prefer tools/texture_ktx2/run.py (loads experiments/desktop_server_py module).
    // Legacy fallback: python -m apps.desktop_server.app.texture_ktx2
    let py = if tools.python.is_file() {
        tools.python.to_string_lossy().into_owned()
    } else {
        "python3".into()
    };

    let mut cmd = vec![py];
    let exp_mod = tools
        .repo_root
        .join("tools/experiments/desktop_server_py/app/texture_ktx2.py");
    let legacy_mod = tools
        .repo_root
        .join("apps/desktop_server/app/texture_ktx2.py");
    if tools.texture_py.is_file() {
        cmd.push(tools.texture_py.to_string_lossy().into_owned());
        cmd.push("-i".into());
        cmd.push(out_dir.to_string_lossy().into_owned());
        cmd.push("--mode".into());
        cmd.push(mode.clone());
    } else if exp_mod.is_file() {
        // Direct path execution when wrapper missing
        cmd.push(exp_mod.to_string_lossy().into_owned());
        cmd.push("-i".into());
        cmd.push(out_dir.to_string_lossy().into_owned());
        cmd.push("--mode".into());
        cmd.push(mode.clone());
    } else if legacy_mod.is_file() {
        cmd.push("-m".into());
        cmd.push("apps.desktop_server.app.texture_ktx2".into());
        cmd.push("-i".into());
        cmd.push(out_dir.to_string_lossy().into_owned());
        cmd.push("--mode".into());
        cmd.push(mode.clone());
    } else {
        return Err(format!(
            "KTX2 mode={mode} requested but texture_ktx2 module/script not found"
        ));
    }

    let cwd = Some(tools.repo_root.as_path());
    let rc = run_logged(emitter, cancel, &cmd, cwd)?;
    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    if rc != 0 {
        return Err(format!("texture post-process exited {rc}"));
    }

    // Light evidence check: any .ktx2 under out_dir
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
        &format!("KTX2 applied ({mode} via basisu postprocess)"),
        serde_json::json!({ "textureMode": mode, "postprocess": true }),
    );
    emitter.log(&format!("[texture] OK mode={mode}"));
    Ok(())
}

fn walk_has_ktx2(dir: &Path) -> bool {
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
            } else if p
                .extension()
                .and_then(|x| x.to_str())
                .map(|x| x.eq_ignore_ascii_case("ktx2"))
                .unwrap_or(false)
            {
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
