//! `processor capabilities --json` — single source for tool probe (T06).

use crate::util::tool_paths;
use serde_json::{json, Value};
use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

pub fn capabilities_json() -> Value {
    let tools = tool_paths();
    let convert = probe_bin(&tools.convert_bin, &["_3dtile", "--help"]);
    let top = probe_bin(&tools.top_rebuild, &["top_rebuild", "--help"]);
    let texture = if tools.texture_bin.is_file() {
        probe_bin(&tools.texture_bin, &["geoforge-texture", "--help"])
    } else if tools.texture_py.is_file() {
        json!({
            "path": tools.texture_py,
            "exists": true,
            "launchOk": true,
            "kind": "python-script",
            "error": Value::Null,
        })
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
        "textureModes": texture_modes(basisu.get("exists").and_then(|v| v.as_bool()).unwrap_or(false)
            || texture.get("launchOk").and_then(|v| v.as_bool()).unwrap_or(false)),
    })
}

fn texture_modes(ok: bool) -> Value {
    json!([
        { "mode": "keep", "supported": true, "postprocess": false },
        {
            "mode": "ktx2-etc1s",
            "supported": ok,
            "postprocess": true,
            "reason": if ok { Value::Null } else { json!("纹理组件缺失，请修复安装") },
        },
        {
            "mode": "ktx2-uastc",
            "supported": ok,
            "postprocess": true,
            "reason": if ok { Value::Null } else { json!("纹理组件缺失，请修复安装") },
        },
        {
            "mode": "ktx2",
            "supported": ok,
            "postprocess": true,
            "reason": if ok { Value::Null } else { json!("纹理组件缺失，请修复安装") },
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
    let mut child = match Command::new(path)
        .arg("--help")
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
    {
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
    let deadline = std::time::Instant::now() + Duration::from_secs(3);
    let mut ok = false;
    while std::time::Instant::now() < deadline {
        match child.try_wait() {
            Ok(Some(status)) => {
                ok = status.success() || status.code().is_some();
                break;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(50)),
            Err(e) => {
                return json!({
                    "path": path,
                    "exists": true,
                    "launchOk": false,
                    "error": e.to_string(),
                });
            }
        }
    }
    if !ok {
        let _ = child.kill();
        let _ = child.wait();
        ok = true;
    }
    json!({
        "path": path,
        "exists": true,
        "launchOk": ok,
        "error": Value::Null,
    })
}
