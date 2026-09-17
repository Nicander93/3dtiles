//! Texture stage: keep = skip; else geoforge-texture / Python texture_ktx2 / basisu.

use crate::cancel::CancelFlag;
use crate::protocol::{Emitter, Stage};
use crate::util::{command_available, run_logged, tool_paths, ToolPaths};
use serde_json::Value;
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::{Arc, OnceLock};

const KEEP: &[&str] = &["keep", "none", "", "passthrough"];
const KTX2_MAGIC: &[u8] = b"\xABKTX 20\r\n\x1A\n";

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

fn postprocess_available(tools: &ToolPaths) -> bool {
    (tools.texture_bin.is_file()
        || (!tools.packaged && tools.texture_py.is_file() && command_available(&tools.python)))
        && tools.basisu.is_file()
}

fn validate_texture_mode_with_tools(
    mode: &str,
    tools: &ToolPaths,
    allow_native: bool,
    native_available: bool,
) -> Result<(), String> {
    if is_keep(mode) {
        return Ok(());
    }
    if allow_native && native_available && mode != "ktx2-uastc" {
        return Ok(());
    }
    if postprocess_available(tools) {
        return Ok(());
    }
    Err(format!(
        "texture mode={mode} is unavailable: the texture encoder or basisu is missing; choose keep or install the texture component"
    ))
}

pub fn validate_texture_mode(mode: &str) -> Result<(), String> {
    let mode = normalize_mode(Some(mode));
    validate_texture_mode_with_tools(&mode, tool_paths(), true, converter_supports_native_ktx2())
}

pub fn validate_existing_tiles_texture_mode(mode: &str) -> Result<(), String> {
    let mode = normalize_mode(Some(mode));
    validate_texture_mode_with_tools(&mode, tool_paths(), false, false)
}

pub fn native_texture_mode_available(mode: &str) -> bool {
    normalize_mode(Some(mode)) == "ktx2-etc1s" && converter_supports_native_ktx2()
}

pub fn converter_supports_native_ktx2() -> bool {
    static SUPPORTED: OnceLock<bool> = OnceLock::new();
    *SUPPORTED.get_or_init(|| {
        if let Ok(value) = std::env::var("GEOFORGE_NATIVE_KTX2") {
            return value == "1" || value.eq_ignore_ascii_case("true");
        }
        let tools = tool_paths();
        if !tools.convert_bin.is_file() {
            return false;
        }
        let mut command = Command::new(&tools.convert_bin);
        command
            .arg("--help")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        crate::util::hide_console_window(&mut command);
        command.output().is_ok_and(|output| {
            output.status.success()
                && (String::from_utf8_lossy(&output.stdout).contains("--enable-texture-compress")
                    || String::from_utf8_lossy(&output.stderr)
                        .contains("--enable-texture-compress"))
        })
    })
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

    let tools = tool_paths();
    if native_texture_mode_available(&mode) && walk_has_ktx2(out_dir) {
        emitter.stage_extra(
            Stage::Texture,
            &format!("KTX2 applied by converter ({mode})"),
            serde_json::json!({ "textureMode": mode, "postprocess": false, "native": true }),
        );
        emitter.log(&format!("[texture] native KTX2 evidence found mode={mode}"));
        return Ok(());
    }
    validate_texture_mode_with_tools(&mode, tools, false, false)?;

    emitter.stage_extra(
        Stage::Texture,
        &format!("post-process basisu mode={mode}"),
        serde_json::json!({ "textureMode": mode, "postprocess": true }),
    );

    let mut cmd: Vec<String> = Vec::new();

    if tools.texture_bin.is_file() {
        cmd.push(tools.texture_bin.to_string_lossy().into_owned());
        cmd.push("-i".into());
        cmd.push(out_dir.to_string_lossy().into_owned());
        cmd.push("--mode".into());
        cmd.push(mode.clone());
        if tools.basisu.is_file() {
            cmd.push("--basisu".into());
            cmd.push(tools.basisu.to_string_lossy().into_owned());
        }
    } else {
        cmd.push(tools.python.to_string_lossy().into_owned());
        cmd.push(tools.texture_py.to_string_lossy().into_owned());
        cmd.push("-i".into());
        cmd.push(out_dir.to_string_lossy().into_owned());
        cmd.push("--mode".into());
        cmd.push(mode.clone());
        if tools.basisu.is_file() {
            cmd.push("--basisu".into());
            cmd.push(tools.basisu.to_string_lossy().into_owned());
        }
    }

    let cwd = Some(tools.repo_root.as_path());
    let rc = run_logged(emitter, cancel, &cmd, cwd)?;
    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    if rc != 0 {
        return Err(format!("texture post-process exited {rc}"));
    }

    let found = walk_has_ktx2(out_dir);
    emitter.log(&format!("[texture] evidence found={found}"));
    if !found {
        return Err(format!(
            "KTX2 mode={mode} was requested, but no KTX2 evidence found under {}",
            out_dir.display()
        ));
    }
    emitter.stage_extra(
        Stage::Texture,
        &format!("KTX2 applied ({mode})"),
        serde_json::json!({ "textureMode": mode, "postprocess": true }),
    );
    emitter.log(&format!("[texture] OK mode={mode}"));
    Ok(())
}

/// Detect standalone .ktx2 or embedded KTX2 / KHR_texture_basisu in glTF/GLB/B3DM.
pub fn walk_has_ktx2(dir: &Path) -> bool {
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
                continue;
            }
            let ext = p
                .extension()
                .and_then(|x| x.to_str())
                .unwrap_or("")
                .to_ascii_lowercase();
            if ext == "ktx2" {
                return true;
            }
            if matches!(ext.as_str(), "glb" | "b3dm" | "gltf") && file_has_ktx2_evidence(&p) {
                return true;
            }
        }
        false
    }
    rec(dir, 0)
}

fn file_has_ktx2_evidence(path: &Path) -> bool {
    let Ok(data) = std::fs::read(path) else {
        return false;
    };
    let ext = path
        .extension()
        .and_then(|x| x.to_str())
        .unwrap_or("")
        .to_ascii_lowercase();
    if ext == "gltf" {
        return text_has_ktx2(std::str::from_utf8(&data).unwrap_or(""));
    }
    let glb = if ext == "b3dm" {
        extract_glb_from_b3dm(&data).unwrap_or(&[][..])
    } else {
        data.as_slice()
    };
    if glb.len() < 20 || &glb[0..4] != b"glTF" {
        return data.windows(KTX2_MAGIC.len()).any(|w| w == KTX2_MAGIC);
    }
    let json_len = u32::from_le_bytes(glb[12..16].try_into().unwrap_or([0; 4])) as usize;
    let json_end = 20usize.saturating_add(json_len);
    if json_end <= glb.len() {
        if let Ok(s) = std::str::from_utf8(&glb[20..json_end]) {
            if text_has_ktx2(s) {
                return true;
            }
        }
    }
    glb.windows(KTX2_MAGIC.len()).any(|w| w == KTX2_MAGIC)
}

fn text_has_ktx2(s: &str) -> bool {
    s.contains("KHR_texture_basisu") || s.contains("image/ktx2") || s.contains(".ktx2")
}

fn extract_glb_from_b3dm(data: &[u8]) -> Option<&[u8]> {
    if data.len() < 28 || &data[0..4] != b"b3dm" {
        return None;
    }
    let ft_json = u32::from_le_bytes(data[12..16].try_into().ok()?) as usize;
    let ft_bin = u32::from_le_bytes(data[16..20].try_into().ok()?) as usize;
    let bt_json = u32::from_le_bytes(data[20..24].try_into().ok()?) as usize;
    let bt_bin = u32::from_le_bytes(data[24..28].try_into().ok()?) as usize;
    let mut offset = 28 + ft_json + ft_bin + bt_json + bt_bin;
    if offset >= data.len() {
        return None;
    }
    if offset + 4 <= data.len() && &data[offset..offset + 4] != b"glTF" {
        for pad in 0..8 {
            let o = offset + pad;
            if o + 4 <= data.len() && &data[o..o + 4] == b"glTF" {
                offset = o;
                break;
            }
        }
    }
    Some(&data[offset..])
}

pub fn texture_mode_from_options(options: &Value) -> String {
    normalize_mode(
        options
            .get("texture")
            .and_then(|t| t.get("mode"))
            .and_then(|v| v.as_str()),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn detects_embedded_marker_in_gltf_json() {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let dir = std::env::temp_dir().join(format!("ktx2-ev-{n}"));
        fs::create_dir_all(&dir).unwrap();
        fs::write(
            dir.join("a.gltf"),
            r#"{"extensionsUsed":["KHR_texture_basisu"],"images":[{"mimeType":"image/ktx2"}]}"#,
        )
        .unwrap();
        assert!(walk_has_ktx2(&dir));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn packaged_mode_does_not_use_source_texture_script() {
        let n = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let root = std::env::temp_dir().join(format!("geoforge-texture-packaged-{n}"));
        let source_script = root.join("source/tools/texture_ktx2/run.py");
        fs::create_dir_all(source_script.parent().unwrap()).unwrap();
        fs::write(&source_script, b"print('source-only')").unwrap();
        let tools = ToolPaths {
            repo_root: root.join("runtime"),
            runtime_root: root.join("runtime"),
            convert_bin: PathBuf::from("_3dtile"),
            top_rebuild: PathBuf::from("top_rebuild"),
            rebuild_py: PathBuf::from("missing-rebuild.py"),
            texture_py: source_script,
            texture_bin: PathBuf::from("missing-geoforge-texture"),
            basisu: PathBuf::from("missing-basisu"),
            python: PathBuf::from("python"),
            packaged: true,
        };

        let error =
            validate_texture_mode_with_tools("ktx2-etc1s", &tools, false, false).unwrap_err();
        assert!(error.contains("texture mode=ktx2-etc1s is unavailable"));
        let _ = fs::remove_dir_all(root);
    }
}
