//! convert-osgb / process-tileset pipelines with temp → validate → commit.

use crate::cancel::CancelFlag;
use crate::geo::{build_tile_config_json, missing_crs_message, resolve_effective_geo};
use crate::path_policy;
use crate::protocol::{Emitter, Stage, TaskConfig, EXIT_CANCELLED, EXIT_FAILED, EXIT_OK};
use crate::stages::{commit, convert, rebuild, scan, texture, validate};
use geoforge_protocol::GeoReferenceOptions;
use serde_json::json;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub struct RunOutcome {
    pub exit_code: i32,
    pub final_path: Option<PathBuf>,
}

pub fn run_task(config: TaskConfig, cancel: CancelFlag) -> RunOutcome {
    let emitter = Arc::new(Emitter::new(config.task_id.clone()));
    cancel.install_watchers();

    if let Err(msg) = config.validate_schema() {
        emitter.error("UNSUPPORTED_SCHEMA", &msg);
        return RunOutcome {
            exit_code: EXIT_FAILED,
            final_path: None,
        };
    }

    let result = match config.operation.as_str() {
        "convert-osgb" => run_convert_osgb(&config, &emitter, &cancel),
        "convert-model" => run_convert_model(&config, &emitter, &cancel),
        "process-tileset" => run_process_tileset(&config, &emitter, &cancel),
        other => Err(format!("Unknown operation: {other}")),
    };

    match result {
        Ok(path) => {
            // After successful commit, cancel must not rewrite outcome.
            emitter.stage(Stage::Done, "succeeded");
            emitter.result(&path.to_string_lossy());
            RunOutcome {
                exit_code: EXIT_OK,
                final_path: Some(path),
            }
        }
        Err(msg) => {
            if cancel.is_cancelled() || msg == "cancelled" {
                emitter.error("CANCELLED", "task cancelled");
                RunOutcome {
                    exit_code: EXIT_CANCELLED,
                    final_path: None,
                }
            } else {
                emitter.error(error_code_for_message(&msg), &msg);
                RunOutcome {
                    exit_code: EXIT_FAILED,
                    final_path: None,
                }
            }
        }
    }
}

fn run_convert_model(
    config: &TaskConfig,
    emitter: &Arc<Emitter>,
    cancel: &CancelFlag,
) -> Result<PathBuf, String> {
    let options = config.model_options()?;
    options.validate()?;
    let input_file = Path::new(config.input_path());
    if !input_file.is_file() {
        return Err(format!("model input must be a file: {}", input_file.display()));
    }
    let extension = input_file.extension().and_then(|value| value.to_str()).unwrap_or_default();
    if !extension.eq_ignore_ascii_case(options.model.format.extension()) {
        return Err(format!(
            "model.format={} does not match input file: {}",
            options.model.format.extension(),
            input_file.display()
        ));
    }
    let resource_root = input_file.parent().ok_or_else(|| "model input has no parent directory".to_string())?;
    let validated = path_policy::validate_io_paths(resource_root, Path::new(config.output_path()), &config.task_id)?;
    report_output_space(emitter, validated.output_parent_free_bytes);
    emitter.stage(Stage::Scan, "Validating model input");
    emitter.metric("input.modelBytes", json!(std::fs::metadata(input_file).map_err(|error| error.to_string())?.len()));

    let (longitude, latitude, height) = match options.georeference {
        GeoReferenceOptions::Anchor { longitude_deg, latitude_deg, ellipsoid_height_m, .. } => {
            (Some(longitude_deg), Some(latitude_deg), Some(ellipsoid_height_m))
        }
        GeoReferenceOptions::Local => (None, None, None),
        // The complete projected configuration is serialized in model-config.
        // CLI longitude/latitude flags are only the legacy anchor transport.
        GeoReferenceOptions::Projected { .. } => (None, None, None),
    };
    check_cancel(cancel)?;

    let final_out = validated.output.clone();
    let temp = commit::prepare_temp(&final_out, &config.task_id)?;
    let mut temp_guard = commit::TempGuard::new(temp.clone(), cancel);
    let staged = temp.join("staged");
    let model_config_path = temp.join("model-config.json");
    let mut model_config = serde_json::to_value(&options).map_err(|error| error.to_string())?;
    model_config
        .as_object_mut()
        .ok_or_else(|| "model config serialization did not produce an object".to_string())?
        .insert("version".into(), json!(1));
    std::fs::write(
        &model_config_path,
        serde_json::to_vec_pretty(&model_config).map_err(|error| error.to_string())?,
    )
    .map_err(|error| format!("cannot write model converter config: {error}"))?;
    convert::run_model_convert(
        emitter,
        cancel,
        input_file,
        &staged,
        options.model.format,
        &model_config_path,
        longitude,
        latitude,
        height,
    )?;
    check_cancel(cancel)?;
    validate::validate_tileset_dir_cancellable(emitter, &staged, Some(cancel))?;
    emitter.metric("temp.stagedBytes", json!(directory_size_bytes(&staged)));
    commit::commit_rename(emitter, &staged, &final_out)?;
    emitter.metric("output.bytes", json!(directory_size_bytes(&final_out)));
    temp_guard.mark_committed();
    commit::cleanup_temp(&temp);
    Ok(final_out)
}

/// Keep a small, stable error vocabulary in the JSONL/UI layer while
/// preserving the original message for diagnostics. This is intentionally a
/// classifier rather than a new error hierarchy so stage implementations can
/// continue returning useful context without changing the protocol schema.
fn error_code_for_message(message: &str) -> &'static str {
    let lower = message.to_ascii_lowercase();
    if lower.contains("input does not exist") {
        "PATH_INPUT_NOT_FOUND"
    } else if lower.contains("output already exists") {
        "PATH_OUTPUT_EXISTS"
    } else if lower.contains("not writable")
        || lower.contains("cannot create output parent")
        || lower.contains("cannot write output directory")
        || lower.contains("access denied")
        || lower.contains("permission denied")
        || lower.contains("拒绝访问")
        || lower.contains("os error 5")
    {
        "PATH_OUTPUT_NOT_WRITABLE"
    } else if lower.contains("must not") && lower.contains("input") {
        "PATH_OVERLAP"
    } else if lower.contains("invalid convert-model options")
        || lower.contains("model config")
        || lower.contains("model.format")
    {
        "MODEL_CONFIG_INVALID"
    } else if lower.contains("model input must be a file")
        || lower.contains("only .fbx and .obj")
        || lower.contains("requires an explicit model")
    {
        "MODEL_INPUT_INVALID"
    } else if lower.contains("projected model georeference") {
        "MODEL_GEOREFERENCE_UNSUPPORTED"
    // A converter failure often echoes the input metadata path in stderr.  Use
    // the explicit process-exit marker first so that this cannot be mistaken
    // for a scan-time metadata error.
    } else if lower.contains("convert exited") || lower.contains("converter") {
        "CONVERTER_EXIT_NONZERO"
    } else if lower.contains("metadata.xml") || lower.contains("srs") {
        "OSGB_METADATA_INVALID"
    } else if lower.contains("rebuild") {
        "REBUILD_FAILED"
    } else if lower.contains("texture") {
        "TEXTURE_FAILED"
    } else if lower.contains("validate") || lower.contains("tileset") {
        "VALIDATE_FAILED"
    } else if lower.contains("disk") || lower.contains("space") {
        "DISK_FULL"
    } else {
        "TASK_FAILED"
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
    let tex_mode = texture::texture_mode_from_options(options);
    texture::validate_texture_mode(&tex_mode)?;
    let validated = path_policy::validate_io_paths(
        Path::new(config.input_path()),
        Path::new(config.output_path()),
        &config.task_id,
    )?;
    report_output_space(emitter, validated.output_parent_free_bytes);
    let final_out = validated.output.clone();
    let temp = commit::prepare_temp(&final_out, &config.task_id)?;
    let mut temp_guard = commit::TempGuard::new(temp.clone(), cancel);
    // Work subdirs inside temp
    let convert_dir = temp.join("convert");
    std::fs::create_dir_all(&convert_dir).map_err(|e| e.to_string())?;

    emitter.stage(Stage::Scan, "Validating OSGB root");
    let scan_result = scan::scan_osgb(validated.input_root.to_string_lossy().as_ref());
    let tile_count = scan_result
        .get("summary")
        .and_then(|s| s.get("tileCount"))
        .cloned()
        .unwrap_or(json!(0));
    emitter.log(&format!(
        "[scan] valid={} tiles={}",
        scan_result
            .get("valid")
            .and_then(|v| v.as_bool())
            .unwrap_or(false),
        tile_count
    ));
    if let Some(bytes) = scan_result
        .get("summary")
        .and_then(|summary| summary.get("totalBytes"))
        .and_then(|value| value.as_u64())
    {
        emitter.metric("input.osgbBytes", json!(bytes));
    }
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
    emitter.stage_extra(Stage::Scan, "OSGB validated", json!({ "geo": effective }));

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
            copy_dir(&work, &staged, Some(cancel))?;
            let _ = std::fs::remove_dir_all(&work);
            Ok::<(), String>(())
        })?;
    }

    validate::validate_tileset_dir_cancellable(emitter, &staged, Some(cancel))?;
    emitter.metric("temp.stagedBytes", json!(directory_size_bytes(&staged)));
    check_cancel(cancel)?;

    // Brief non-cancellable publish window
    commit::commit_rename(emitter, &staged, &final_out)?;
    emitter.metric("output.bytes", json!(directory_size_bytes(&final_out)));
    // Cleanup leftover temp shell
    temp_guard.mark_committed();
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
    texture::validate_existing_tiles_texture_mode(&tex_mode)?;

    let validated = path_policy::validate_io_paths(
        Path::new(config.input_path()),
        Path::new(config.output_path()),
        &config.task_id,
    )?;
    report_output_space(emitter, validated.output_parent_free_bytes);
    emitter.stage(Stage::Scan, "Checking tileset input");
    let tileset = scan::resolve_tileset(validated.input_root.to_string_lossy().as_ref())?;
    let in_dir = tileset
        .parent()
        .unwrap_or_else(|| Path::new("."))
        .to_path_buf();
    check_cancel(cancel)?;

    let final_out = validated.output.clone();
    let temp = commit::prepare_temp(&final_out, &config.task_id)?;
    let mut temp_guard = commit::TempGuard::new(temp.clone(), cancel);
    let work = temp.join("work");

    if want_rebuild {
        rebuild::run_rebuild(emitter, cancel, &in_dir, &work, &rebuild_opts)?;
    } else {
        // texture-only: copy input tree (work is outside input_root by path_policy)
        emitter.log(&format!(
            "[texture] copy {} -> {}",
            in_dir.display(),
            work.display()
        ));
        copy_dir(&in_dir, &work, Some(cancel))?;
    }
    check_cancel(cancel)?;

    if want_texture {
        texture::finish_texture(emitter, cancel, &work, &tex_mode)?;
        check_cancel(cancel)?;
    } else {
        texture::finish_texture(emitter, cancel, &work, "keep")?;
    }

    validate::validate_tileset_dir_cancellable(emitter, &work, Some(cancel))?;
    emitter.metric("temp.stagedBytes", json!(directory_size_bytes(&work)));
    check_cancel(cancel)?;
    commit::commit_rename(emitter, &work, &final_out)?;
    emitter.metric("output.bytes", json!(directory_size_bytes(&final_out)));
    temp_guard.mark_committed();
    commit::cleanup_temp(&temp);
    Ok(final_out)
}

fn copy_dir(src: &Path, dst: &Path, cancel: Option<&CancelFlag>) -> Result<(), String> {
    if cancel.map(|flag| flag.is_cancelled()).unwrap_or(false) {
        return Err("cancelled".into());
    }
    std::fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in std::fs::read_dir(src).map_err(|e| e.to_string())? {
        if cancel.map(|flag| flag.is_cancelled()).unwrap_or(false) {
            return Err("cancelled".into());
        }
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir(&entry.path(), &to, cancel)?;
        } else {
            copy_file_cancellable(&entry.path(), &to, cancel)?;
        }
    }
    Ok(())
}

fn copy_file_cancellable(
    source: &Path,
    destination: &Path,
    cancel: Option<&CancelFlag>,
) -> Result<(), String> {
    let mut input = std::fs::File::open(source).map_err(|e| e.to_string())?;
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)
        .map_err(|e| e.to_string())?;
    let mut buffer = [0_u8; 1024 * 1024];
    loop {
        if cancel.map(|flag| flag.is_cancelled()).unwrap_or(false) {
            return Err("cancelled".into());
        }
        let count = input.read(&mut buffer).map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        output
            .write_all(&buffer[..count])
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

fn directory_size_bytes(path: &Path) -> u64 {
    let Ok(entries) = std::fs::read_dir(path) else {
        return 0;
    };
    entries
        .filter_map(Result::ok)
        .map(|entry| {
            let child = entry.path();
            if child.is_dir() {
                directory_size_bytes(&child)
            } else {
                entry.metadata().map(|metadata| metadata.len()).unwrap_or(0)
            }
        })
        .fold(0, u64::saturating_add)
}

fn report_output_space(emitter: &Emitter, free_bytes: Option<u64>) {
    let Some(free_bytes) = free_bytes else {
        return;
    };
    emitter.metric("output.parentFreeBytes", json!(free_bytes));
    let threshold = std::env::var("GEOFORGE_LOW_DISK_BYTES")
        .ok()
        .and_then(|value| value.parse::<u64>().ok())
        .unwrap_or(1024 * 1024 * 1024);
    if free_bytes < threshold {
        emitter.warning(
            "LOW_DISK_SPACE",
            &format!(
                "output parent has {} bytes free; conversion may need more space",
                free_bytes
            ),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{copy_dir, error_code_for_message};
    use crate::cancel::CancelFlag;
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_dir(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let path = std::env::temp_dir().join(format!("geoforge-pipeline-{name}-{stamp}"));
        fs::create_dir_all(&path).expect("create pipeline fixture");
        path
    }

    #[test]
    fn classifies_path_and_converter_failures() {
        assert_eq!(
            error_code_for_message("output already exists: C:/out"),
            "PATH_OUTPUT_EXISTS"
        );
        assert_eq!(
            error_code_for_message("D:/out: 拒绝访问。 (os error 5)"),
            "PATH_OUTPUT_NOT_WRITABLE"
        );
        assert_eq!(
            error_code_for_message("convert exited 1: converter stderr"),
            "CONVERTER_EXIT_NONZERO"
        );
        assert_eq!(
            error_code_for_message("convert exited 1: metadata.xml could not be read by converter"),
            "CONVERTER_EXIT_NONZERO"
        );
    }

    #[test]
    fn keeps_unknown_failures_in_stable_bucket() {
        assert_eq!(
            error_code_for_message("unexpected native failure"),
            "TASK_FAILED"
        );
    }

    #[test]
    fn classifies_model_configuration_and_georeference_errors() {
        assert_eq!(
            error_code_for_message("OBJ requires an explicit model.unit; OBJ does not reliably declare units"),
            "MODEL_INPUT_INVALID"
        );
        assert_eq!(
            error_code_for_message("invalid convert-model options: missing field `model`"),
            "MODEL_CONFIG_INVALID"
        );
        assert_eq!(
            error_code_for_message("projected model georeference requires a converter with model-config support"),
            "MODEL_GEOREFERENCE_UNSUPPORTED"
        );
    }

    #[test]
    fn copy_dir_honours_preexisting_cancel_before_writing() {
        let root = temp_dir("copy-cancel");
        let source = root.join("source");
        let destination = root.join("destination");
        fs::create_dir_all(&source).expect("create source");
        fs::write(source.join("payload.bin"), vec![7_u8; 1024 * 1024]).expect("write source");
        let cancel = CancelFlag::new();
        cancel.request();
        let error = copy_dir(&source, &destination, Some(&cancel)).expect_err("copy cancelled");
        assert_eq!(error, "cancelled");
        assert!(!destination.exists());
        let _ = fs::remove_dir_all(root);
    }
}
