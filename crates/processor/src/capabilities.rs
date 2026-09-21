//! `processor capabilities --json` — single source for tool probe (T06).

use crate::stages::texture::converter_supports_native_ktx2;
use crate::util::{hide_console_window, tool_paths};
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

pub fn capabilities_json() -> Value {
    let tools = tool_paths();
    let convert = probe_bin(&tools.convert_bin, &["_3dtile", "--help"]);
    let model = probe_model_capabilities(&tools.convert_bin);
    let top = probe_bin(&tools.top_rebuild, &["top_rebuild", "--help"]);
    let texture = if tools.texture_bin.is_file() {
        probe_bin(&tools.texture_bin, &["geoforge-texture", "--help"])
    } else if !tools.packaged && tools.texture_py.is_file() {
        probe_script(&tools.texture_py, &tools.python)
    } else {
        json!({
            "path": tools.texture_bin,
            "exists": false,
            "launchOk": false,
            "kind": "missing",
            "error": "texture tool missing — 请修复安装",
        })
    };
    let basisu = json!({
        "path": tools.basisu,
        "exists": tools.basisu.is_file(),
    });

    let convert_ready = convert
        .get("launchOk")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let postprocess_ready = texture
        .get("launchOk")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
        && basisu
            .get("exists")
            .and_then(|v| v.as_bool())
            .unwrap_or(false);
    let native_ktx2 = convert_ready && converter_supports_native_ktx2();

    json!({
        "ok": true,
        "packaged": tools.packaged,
        "runtimeRoot": tools.runtime_root,
        "tools": {
            "converter": convert,
            "topRebuild": top,
            "texture": texture,
            "basisu": basisu,
        },
        "convert": {
            "ready": convert_ready,
            "native": convert_ready,
            "dockerFallback": false,
            "message": if convert_ready {
                "ready"
            } else if tools.packaged {
                "组件缺失，请修复安装（转换器 _3dtile）"
            } else {
                "找不到转换器；运行 prepare-converter.ps1 或设置 GEOFORGE_3DTILE"
            },
        },
        "postprocessBasisu": {
            "available": postprocess_ready,
            "path": if postprocess_ready {
                Value::String(tools.basisu.display().to_string())
            } else {
                Value::Null
            },
        },
        "model": model,
        "textureModes": texture_modes(native_ktx2, postprocess_ready),
    })
}

fn probe_model_capabilities(path: &Path) -> Value {
    if !path.is_file() {
        return json!({ "ready": false, "reason": "converter missing" });
    }
    let mut command = Command::new(path);
    command
        .arg("--capabilities-json")
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    hide_console_window(&mut command);
    let output = match command.output() {
        Ok(output) if output.status.success() => output,
        Ok(output) => return json!({
            "ready": false,
            "reason": format!("--capabilities-json exited with {:?}", output.status.code()),
        }),
        Err(error) => return json!({ "ready": false, "reason": error.to_string() }),
    };
    let parsed: Value = match serde_json::from_slice(&output.stdout) {
        Ok(parsed) => parsed,
        Err(error) => return json!({ "ready": false, "reason": format!("invalid capability JSON: {error}") }),
    };
    model_capability_value(&parsed)
}

fn model_capability_value(parsed: &Value) -> Value {
    let formats = parsed.get("formats").and_then(Value::as_array);
    let supports_fbx = formats
        .map(|formats| formats.iter().any(|format| format.as_str() == Some("fbx")))
        .unwrap_or(false);
    let supports_obj = formats
        .map(|formats| formats.iter().any(|format| format.as_str() == Some("obj")))
        .unwrap_or(false);
    let config_version = parsed.get("modelConfigVersion").and_then(Value::as_u64);
    json!({
        "ready": supports_fbx && supports_obj && config_version == Some(1),
        "formats": formats.cloned().unwrap_or_default(),
        "modelConfigVersion": config_version,
        "georeferenceModes": parsed.get("georeferenceModes").cloned().unwrap_or_default(),
        "projectedGeoreference": parsed.get("projectedGeoreference").cloned().unwrap_or(Value::Bool(false)),
        "reason": if supports_fbx && supports_obj && config_version == Some(1) { Value::Null } else { json!("converter does not support FBX, OBJ, and modelConfigVersion=1") },
    })
}

#[cfg(test)]
mod tests {
    use super::model_capability_value;
    use serde_json::json;

    #[test]
    fn accepts_v1_fbx_and_obj_model_capability() {
        let result = model_capability_value(&json!({
            "formats": ["fbx", "obj"],
            "modelConfigVersion": 1,
            "georeferenceModes": ["local", "anchor"],
            "projectedGeoreference": false,
        }));
        assert_eq!(result["ready"], true);
        assert_eq!(result["projectedGeoreference"], false);
    }

    #[test]
    fn rejects_legacy_converter_capability() {
        let result = model_capability_value(&json!({ "formats": ["fbx"] }));
        assert_eq!(result["ready"], false);
        assert!(result["reason"].as_str().unwrap_or_default().contains("modelConfigVersion"));
    }
}

fn texture_modes(native_ktx2: bool, postprocess: bool) -> Value {
    let etc1s = native_ktx2 || postprocess;
    json!([
        { "mode": "keep", "supported": true, "postprocess": false },
        {
            "mode": "ktx2-etc1s",
            "supported": etc1s,
            "postprocess": !native_ktx2 && postprocess,
            "native": native_ktx2,
            "cliFlags": if native_ktx2 { json!(["--enable-texture-compress"]) } else { json!([]) },
            "processTileset": {
                "mode": "ktx2-etc1s",
                "supported": postprocess,
                "reason": if postprocess { Value::Null } else { json!("KTX2 processing for existing tiles requires the texture component") },
            },
            "reason": if etc1s { Value::Null } else { json!("KTX2 encoder unavailable") },
        },
        {
            "mode": "ktx2-uastc",
            "supported": postprocess,
            "postprocess": true,
            "processTileset": {
                "mode": "ktx2-uastc",
                "supported": postprocess,
                "reason": if postprocess { Value::Null } else { json!("UASTC requires the texture component") },
            },
            "reason": if postprocess { Value::Null } else { json!("UASTC requires the texture component") },
        },
        {
            "mode": "ktx2",
            "supported": etc1s,
            "postprocess": !native_ktx2 && postprocess,
            "native": native_ktx2,
            "processTileset": {
                "mode": "ktx2-etc1s",
                "supported": postprocess,
                "reason": if postprocess { Value::Null } else { json!("KTX2 processing for existing tiles requires the texture component") },
            },
            "reason": if etc1s { Value::Null } else { json!("KTX2 encoder unavailable") },
        },
    ])
}

fn probe_bin(path: &Path, _hint: &[&str]) -> Value {
    let exists = path.is_file();
    if !exists {
        return json!({
            "path": path,
            "exists": false,
            "launchOk": false,
            "error": format!("missing: {}", path.display()),
        });
    }
    let mut command = Command::new(path);
    command
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console_window(&mut command);
    let child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            return json!({
                "path": path,
                "exists": true,
                "launchOk": false,
                "error": e.to_string(),
            });
        }
    };
    probe_child(path, child)
}

fn probe_script(script: &Path, interpreter: &Path) -> Value {
    let mut command = Command::new(interpreter);
    command
        .arg(script)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    hide_console_window(&mut command);
    let child = match command.spawn() {
        Ok(c) => c,
        Err(e) => {
            return json!({
                "path": script,
                "exists": true,
                "launchOk": false,
                "kind": "python-script",
                "interpreter": interpreter,
                "error": e.to_string(),
            });
        }
    };
    let mut result = probe_child(script, child);
    if let Some(object) = result.as_object_mut() {
        object.insert("kind".into(), json!("python-script"));
        object.insert("interpreter".into(), json!(interpreter));
    }
    result
}

fn probe_child(path: &Path, mut child: std::process::Child) -> Value {
    let mut status_code = None;
    let mut timed_out = false;
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    while std::time::Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) => {
                status_code = status.code();
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                let _ = child.kill();
                let _ = child.wait();
                return json!({
                    "path": path,
                    "exists": true,
                    "launchOk": false,
                    "error": e.to_string(),
                });
            }
        }
    }
    if status_code.is_none() {
        let _ = child.kill();
        let _ = child.wait();
        timed_out = true;
    }
    let launch_ok = !timed_out && status_code == Some(0);
    json!({
        "path": path,
        "exists": true,
        "launchOk": launch_ok,
        "exitCode": status_code,
        "timedOut": timed_out,
        "error": if timed_out {
            json!("--help timed out after 3 seconds")
        } else if !launch_ok {
            json!(format!("--help exited with {}", status_code.unwrap_or(-1)))
        } else {
            Value::Null
        },
    })
}
