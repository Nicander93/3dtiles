//! Temp dir → final output rename/commit (plan §7.4).

use crate::protocol::{Emitter, Stage};
use std::fs;
use std::path::{Path, PathBuf};

/// `<output-parent>/.geoforge-task-<task-id>/`
pub fn temp_work_dir(final_output: &Path, task_id: &str) -> PathBuf {
    let parent = final_output.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(".geoforge-task-{task_id}"))
}

pub fn prepare_temp(final_output: &Path, task_id: &str) -> Result<PathBuf, String> {
    let temp = temp_work_dir(final_output, task_id);
    if temp.exists() {
        fs::remove_dir_all(&temp).map_err(|e| format!("clean temp: {e}"))?;
    }
    fs::create_dir_all(&temp).map_err(|e| format!("create temp: {e}"))?;
    Ok(temp)
}

/// Commit temp directory to final output path.
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

    if final_output.exists() {
        remove_path(final_output)?;
    }

    match fs::rename(temp_dir, final_output) {
        Ok(()) => {}
        Err(_) => {
            copy_dir_recursive(temp_dir, final_output)?;
            let _ = fs::remove_dir_all(temp_dir);
        }
    }

    emitter.log(&format!("[commit] {}", final_output.display()));
    Ok(())
}

pub fn cleanup_temp(temp_dir: &Path) {
    if temp_dir.exists() {
        let _ = fs::remove_dir_all(temp_dir);
    }
}

fn remove_path(p: &Path) -> Result<(), String> {
    if p.is_dir() {
        fs::remove_dir_all(p).map_err(|e| e.to_string())
    } else {
        fs::remove_file(p).map_err(|e| e.to_string())
    }
}

fn copy_dir_recursive(src: &Path, dst: &Path) -> Result<(), String> {
    fs::create_dir_all(dst).map_err(|e| e.to_string())?;
    for entry in fs::read_dir(src).map_err(|e| e.to_string())? {
        let entry = entry.map_err(|e| e.to_string())?;
        let ty = entry.file_type().map_err(|e| e.to_string())?;
        let to = dst.join(entry.file_name());
        if ty.is_dir() {
            copy_dir_recursive(&entry.path(), &to)?;
        } else {
            fs::copy(entry.path(), &to).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}
