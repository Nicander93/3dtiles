//! Temp dir → final output rename/commit (plan §7.4 / T01).

use crate::path_policy::{self, validate_task_id};
use crate::protocol::{Emitter, Stage};
use std::fs;
use std::path::{Path, PathBuf};

/// `<output-parent>/.geoforge-task-<task-id>/`
pub fn temp_work_dir(final_output: &Path, task_id: &str) -> PathBuf {
    let parent = final_output.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(".geoforge-task-{task_id}"))
}

pub fn prepare_temp(final_output: &Path, task_id: &str) -> Result<PathBuf, String> {
    validate_task_id(task_id)?;
    let temp = temp_work_dir(final_output, task_id);
    if temp.exists() {
        fs::remove_dir_all(&temp).map_err(|e| format!("clean temp: {e}"))?;
    }
    fs::create_dir_all(&temp).map_err(|e| format!("create temp: {e}"))?;
    Ok(temp)
}

/// Commit temp directory to final output path (no-replace; no copy fallback).
pub fn commit_rename(
    emitter: &Emitter,
    temp_dir: &Path,
    final_output: &Path,
) -> Result<(), String> {
    emitter.stage(Stage::Commit, "Committing output");

    if let Some(parent) = final_output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let manifest = temp_dir.join(".geoforge-manifest.json");
    let _ = fs::write(
        &manifest,
        serde_json::json!({
            "committedAt": chrono::Utc::now().to_rfc3339(),
            "finalPath": final_output.to_string_lossy(),
        })
        .to_string(),
    );

    path_policy::rename_no_replace(temp_dir, final_output)?;

    emitter.log(&format!("[commit] {}", final_output.display()));
    Ok(())
}

pub fn cleanup_temp(temp_dir: &Path) {
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(temp_dir);
    }
}
