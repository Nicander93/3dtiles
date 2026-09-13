//! Invoke existing _3dtile / GEOFORGE_3DTILE wrapper, or Docker if missing.

use crate::cancel::CancelFlag;
use crate::protocol::{Emitter, Stage};
use crate::util::{
    docker_available, docker_image, docker_volume_path, run_logged, tool_paths,
};
use serde_json::Value;
use std::path::Path;
use std::sync::Arc;

pub fn run_convert(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    osgb_root: &str,
    out_dir: &Path,
    options: &Value,
    cfg_json: Option<&str>,
) -> Result<(), String> {
    std::fs::create_dir_all(out_dir).map_err(|e| e.to_string())?;
    let extra = convert_flags(emitter, options, cfg_json);
    let tools = tool_paths();

    emitter.stage(Stage::Convert, "OSGB → 3D Tiles");
    let rc = if tools.convert_bin.is_file() {
        run_native(emitter, cancel, &tools.convert_bin, osgb_root, out_dir, &extra)?
    } else if !tools.packaged && docker_available() {
        let image = docker_image();
        emitter.log(&format!(
            "[convert] 本机无 _3dtile（{}），改用 Docker {image}",
            tools.convert_bin.display()
        ));
        run_docker(emitter, cancel, osgb_root, out_dir, &extra, &image)?
    } else if tools.packaged {
        return Err(format!(
            "组件缺失，请修复安装（转换器 _3dtile 未找到：{}）",
            tools.convert_bin.display()
        ));
    } else {
        return Err(format!(
            "找不到转换器 _3dtile（查过 {}）。开发环境可设置 GEOFORGE_3DTILE，或安装 Docker（镜像 {}）。",
            tools.convert_bin.display(),
            docker_image()
        ));
    };

    if cancel.is_cancelled() {
        return Err("cancelled".into());
    }
    if rc != 0 {
        return Err(format!("convert exited {rc}"));
    }
    let tileset = out_dir.join("tileset.json");
    if !tileset.is_file() {
        return Err(format!("tileset.json missing under {}", out_dir.display()));
    }
    Ok(())
}

fn convert_flags(emitter: &Arc<Emitter>, options: &Value, cfg_json: Option<&str>) -> Vec<String> {
    let mut extra = Vec::new();
    let conv = options.get("convert").cloned().unwrap_or(Value::Null);
    if conv.get("verbose").and_then(|v| v.as_bool()).unwrap_or(false) {
        extra.push("-v".into());
    }
    if let Some(cfg) = conv.get("config").and_then(|v| v.as_str()) {
        extra.push("-c".into());
        extra.push(cfg.into());
        emitter.log("[convert] using options.convert.config");
    } else if let Some(cfg) = cfg_json {
        extra.push("-c".into());
        extra.push(cfg.into());
        emitter.log(&format!("[convert] -c {cfg}"));
    }

    let texture = options.get("texture").cloned().unwrap_or(Value::Null);
    let mode = crate::stages::texture::normalize_mode(texture.get("mode").and_then(|v| v.as_str()));
    if mode != "keep" {
        if std::env::var("GEOFORGE_NATIVE_KTX2").ok().as_deref() == Some("1") {
            extra.push("--enable-texture-compress".into());
            emitter.log(&format!(
                "[convert] texture flag: --enable-texture-compress (mode={mode})"
            ));
        } else {
            emitter.log(&format!(
                "[convert] mode={mode}: convert without native KTX2; texture stage may post-process"
            ));
        }
    }
    extra
}

fn run_native(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    bin: &Path,
    osgb_root: &str,
    out_dir: &Path,
    extra: &[String],
) -> Result<i32, String> {
    let mut cmd = vec![
        bin.to_string_lossy().into_owned(),
        "-f".into(),
        "osgb".into(),
        "-i".into(),
        osgb_root.into(),
        "-o".into(),
        out_dir.to_string_lossy().into_owned(),
    ];
    cmd.extend(extra.iter().cloned());
    run_logged(emitter, cancel, &cmd, None)
}

fn run_docker(
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
    osgb_root: &str,
    out_dir: &Path,
    extra: &[String],
    image: &str,
) -> Result<i32, String> {
    let input = docker_volume_path(Path::new(osgb_root))?;
    let output = docker_volume_path(out_dir)?;
    let name = format!(
        "geoforge-convert-{}",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0)
    );
    let mut cmd = vec![
        "docker".into(),
        "run".into(),
        "--rm".into(),
        "--name".into(),
        name.clone(),
        "-e".into(),
        "LD_LIBRARY_PATH=/3dtiles/lib".into(),
        "-w".into(),
        "/3dtiles".into(),
        "-v".into(),
        format!("{input}:/input"),
        "-v".into(),
        format!("{output}:/output"),
    ];

    let mut tile_args = vec![
        "./target/release/_3dtile".into(),
        "-f".into(),
        "osgb".into(),
        "-i".into(),
        "/input".into(),
        "-o".into(),
        "/output".into(),
    ];
    let mut i = 0;
    while i < extra.len() {
        if extra[i] == "-c" {
            if let Some(cfg) = extra.get(i + 1) {
                let cfg_path = Path::new(cfg);
                if cfg_path.is_file() {
                    let host = docker_volume_path(cfg_path)?;
                    cmd.push("-v".into());
                    cmd.push(format!("{host}:/cfg.json"));
                    tile_args.push("-c".into());
                    tile_args.push("/cfg.json".into());
                } else {
                    tile_args.push("-c".into());
                    tile_args.push(cfg.clone());
                }
                i += 2;
                continue;
            }
        }
        tile_args.push(extra[i].clone());
        i += 1;
    }

    cmd.push(image.into());
    cmd.extend(tile_args);
    // Wrap run_logged: on cancel, also docker stop the named container
    let result = run_logged(emitter, cancel, &cmd, None);
    if cancel.is_cancelled() {
        let _ = std::process::Command::new("docker")
            .args(["stop", "-t", "2", &name])
            .status();
    }
    result
}
