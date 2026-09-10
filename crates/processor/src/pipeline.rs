//! convert-osgb / process-tileset pipelines with temp → validate → commit.

use crate::cancel::CancelFlag;
use crate::geo::{build_tile_config_json, missing_crs_message, resolve_effective_geo};
use crate::protocol::{Emitter, Stage, TaskConfig, EXIT_CANCELLED, EXIT_FAILED, EXIT_OK};
use crate::stages::{commit, convert, rebuild, scan, texture, validate};
use serde_json::json;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct RunOutcome {
    pub exit_code: i32,
    pub final_path: Option<PathBuf>,
}

pub fn run_task(config: TaskConfig, cancel: CancelFlag) -> RunOutcome {
    let emitter = Arc::new(Emitter::new(config.task_id.clone()));
    cancel.install_watchers();

    let result = match config.operation.as_str() {
        "convert-osgb" => run_convert_osgb(&config, &emitter, &cancel),
        "process-tileset" => run_process_tileset(&config, &emitter, &cancel),
        other => Err(format!("Unknown operation: {other}")),
    };

    match result {
        Ok(path) => {
            if cancel.is_cancelled() {
                emitter.error("CANCELLED", "task cancelled");
                RunOutcome {
                    exit_code: EXIT_CANCELLED,
                    final_path: None,
                }
            } else {
                emitter.stage(Stage::Done, "succeeded");
                emitter.result(&path.to_string_lossy());
                RunOutcome {
                    exit_code: EXIT_OK,
                    final_path: Some(path),
                }
            }
        }
        Err(msg) => {
            if cancel.is_cancelled() || msg == "cancelled" {
                emitter.error("CANCELLED", "task cancelled");
                // Best-effort cleanup of temp
                let final_out = PathBuf::from(config.output_path());
                commit::cleanup_temp(&commit::temp_work_dir(&final_out, &config.task_id));
                RunOutcome {
                    exit_code: EXIT_CANCELLED,
                    final_path: None,
                }
            } else {
                emitter.error("FAILED", &msg);
                let final_out = PathBuf::from(config.output_path());
                commit::cleanup_temp(&commit::temp_work_dir(&final_out, &config.task_id));
                RunOutcome {
                    exit_code: EXIT_FAILED,
                    final_path: None,
                }
            }
        }
    }
}

fn check_cancel(cancel: &CancelFlag) -> Result<(), String> {
    if cancel.is_cancelled() {
        Err("cancelled".into())
    } else {
        Ok(())
    }
}

fn run_convert_osgb(
    config: &TaskConfig,
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
) -> Result<PathBuf, String> {
    let options = config.options_obj();
    let final_out = PathBuf::from(config.output_path());
    let temp = commit::prepare_temp(&final_out, &config.task_id)?;
    // Work subdirs inside temp
    let convert_dir = temp.join("convert");
    std::fs::create_dir_all(&convert_dir).map_err(|e| e.to_string())?;

    emitter.stage(Stage::Scan, "Validating OSGB root");
    let scan_result = scan::scan_osgb(config.input_path());
    let tile_count = scan_result
        .get("summary")
        .and_then(|s| s.get("tileCount"))
        .cloned()
        .unwrap_or(json!(0));
    emitter.log(&format!(
        "[scan] valid={} tiles={}",
        scan_result.get("valid").and_then(|v| v.as_bool()).unwrap_or(false),
        tile_count
    ));
    if !scan_result
        .get("valid")
        .and_then(|v| v.as_bool())
        .unwrap_or(false)
    {
        let errs = scan_result
            .get("errors")
            .and_then(|v| v.as_array())
            .map(|a| {
                a.iter()
                    .filter_map(|x| x.as_str())
                    .collect::<Vec<_>>()
                    .join("; ")
            })
            .unwrap_or_else(|| "invalid OSGB".into());
        return Err(errs);
    }
    check_cancel(cancel)?;

    let root = scan_result
        .get("path")
        .and_then(|v| v.as_str())
        .unwrap_or(config.input_path())
        .to_string();

    let effective = resolve_effective_geo(&scan_result, options);
    emitter.log(&format!(
        "[geo] effectiveCrs={:?} geographicExport={:?}",
        effective.get("effectiveCrs"),
        effective.get("geographicExport")
    ));
    if let Some(msg) = missing_crs_message(&effective) {
        return Err(msg);
    }
    emitter.stage_extra(
        Stage::Scan,
        "OSGB validated",
        json!({ "geo": effective }),
    );

    let (cfg_json, notes) = build_tile_config_json(&effective);
    for n in notes {
        emitter.log(&format!("[geo] {n}"));
    }

    convert::run_convert(
        emitter,
        cancel,
        &root,
        &convert_dir,
        options,
        cfg_json.as_deref(),
    )?;
    check_cancel(cancel)?;

    let mut work = convert_dir;
    if rebuild::rebuild_enabled(options) {
        let rebuild_out = temp.join("rebuild");
        rebuild::run_rebuild(
            emitter,
            cancel,
            &work,
            &rebuild_out,
            &rebuild::rebuild_opts(options),
        )?;
        check_cancel(cancel)?;
        work = rebuild_out;
    }

    let tex_mode = texture::texture_mode_from_options(options);
    texture::finish_texture(emitter, cancel, &work, &tex_mode)?;
    check_cancel(cancel)?;

    // Stage final content into temp root for commit
    let staged = temp.join("staged");
    if staged.exists() {
        let _ = std::fs::remove_dir_all(&staged);
    }
    // Move work → staged (or rename if already at convert and no rebuild)
    if work != staged {
        std::fs::rename(&work, &staged).or_else(|_| {
            copy_dir(&work, &staged)?;
            let _ = std::fs::remove_dir_all(&work);
            Ok::<(), String>(())
        })?;
    }

    validate::validate_tileset_dir(emitter, &staged)?;
    check_cancel(cancel)?;

    // Move staged up: commit expects temp_dir contents to become final_out
    // So rename temp's staged to be the only content — simplest: commit `staged` as final
    commit::commit_rename(emitter, &staged, &final_out)?;
    // Cleanup leftover temp shell
    commit::cleanup_temp(&temp);

    Ok(final_out)
}

fn run_process_tileset(
    config: &TaskConfig,
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
) -> Result<PathBuf, String> {
    let options = config.options_obj();
    let rebuild_opts = rebuild::rebuild_opts(options);
    let want_rebuild = rebuild::rebuild_enabled(options);
    let tex_mode = texture::texture_mode_from_options(options);
    let want_texture = !texture::is_keep(&tex_mode);

    if !want_rebuild && !want_texture {
        return Err(
            "process-tileset requires rebuildTop.enabled and/or texture.mode != keep".into(),
        );
    }

    emitter.stage(Stage::Scan, "Checking tileset input");
    let tileset = scan::resolve_tileset(config.input_path())?;
    let in_dir = tileset
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    check_cancel(cancel)?;

    let final_out = PathBuf::from(config.output_path());
    let temp = commit::prepare_temp(&final_out, &config.task_id)?;
    let work = temp.join("work");

    if want_rebuild {
        rebuild::run_rebuild(emitter, cancel, &in_dir, &work, &rebuild_opts)?;
    } else {
        // texture-only: copy input tree
        emitter.log(&format!(
            "[texture] copy {} -> {}",
            in_dir.display(),
            work.display()
        ));
        copy_dir(&in_dir, &work)?;
    }
    check_cancel(cancel)?;

    if want_texture {
        texture::finish_texture(emitter, cancel, &work, &tex_mode)?;
        check_cancel(cancel)?;
    } else {
        texture::finish_texture(emitter, cancel, &work, "keep")?;
    }

    validate::validate_tileset_dir(emitter, &work)?;
    check_cancel(cancel)?;
    commit::commit_rename(emitter, &work, &final_out)?;
    commit::cleanup_temp(&temp);
    Ok(final_out)
}

fn copy_dir(src: &Path, dst: &Path) -> Result<(), String> {
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &to)?;
        } else {
            std::fs::copy(entry.path(), &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
