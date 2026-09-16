//! Temp dir → final output rename/commit (plan §7.4 / T01).

use crate::cancel::CancelFlag;
use crate::path_policy::{self, validate_task_id};
use crate::protocol::{Emitter, Stage};
use std::fs;
use std::path::{Path, PathBuf};

/// `<output-parent>/.geoforge-task-<task-id>/`
const TEMP_MARKER: &str = ".geoforge-owned";

pub fn temp_work_dir(final_output: &Path, task_id: &str) -> PathBuf {
    let parent = final_output.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(".geoforge-task-{task_id}"))
}

pub fn prepare_temp(final_output: &Path, task_id: &str) -> Result<PathBuf, String> {
    validate_task_id(task_id)?;
    let temp = temp_work_dir(final_output, task_id);
    // `create_dir` is intentionally exclusive: a check followed by
    // `create_dir_all` would leave a race where another process can replace
    // the path between the two operations.
    match fs::create_dir(&temp) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(format!(
                "temporary work directory already exists; refusing to delete it: {}",
                temp.display()
            ));
        }
        Err(error) => return Err(format!("create temp: {error}")),
    }
    if let Err(error) = fs::write(temp.join(TEMP_MARKER), format!("geoforge-task:{task_id}\n")) {
        let _ = fs::remove_dir(&temp);
        return Err(format!("write temp ownership marker: {error}"));
    }
    Ok(temp)
}

/// Commit temp directory to final output path (no-replace; no copy fallback).
pub fn commit_rename(
    emitter: &Emitter,
    temp_dir: &Path,
    final_output: &Path,
) -> Result<(), String> {
    emitter.stage(Stage::Commit, "Committing output");
    ensure_owned_temp(temp_dir)?;

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
    if owns_temp_dir(temp_dir) {
        let _ = fs::remove_dir_all(temp_dir);
    }
}

fn owns_temp_dir(temp_dir: &Path) -> bool {
    let Ok(metadata) = fs::symlink_metadata(temp_dir) else {
        return false;
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return false;
    }
    let Some(task_id) = temp_dir
        .file_name()
        .and_then(|value| value.to_str())
        .and_then(|value| value.strip_prefix(".geoforge-task-"))
    else {
        return false;
    };
    fs::read_to_string(temp_dir.join(TEMP_MARKER))
        .map(|value| value.trim() == format!("geoforge-task:{task_id}"))
        .unwrap_or(false)
}

fn ensure_owned_temp(temp_dir: &Path) -> Result<(), String> {
    let metadata = fs::symlink_metadata(temp_dir).map_err(|error| {
        format!(
            "temporary work directory is missing or inaccessible {}: {error}",
            temp_dir.display()
        )
    })?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(format!(
            "temporary work directory is not a real directory: {}",
            temp_dir.display()
        ));
    }
    if owns_temp_dir(temp_dir) {
        return Ok(());
    }
    if temp_dir.parent().map(owns_temp_dir).unwrap_or(false) {
        return Ok(());
    }
    Err(format!(
        "temporary work directory ownership marker missing or unsafe: {}",
        temp_dir.display()
    ))
}

/// Cleans only a work directory created by this run when cancellation interrupts it.
pub struct TempGuard<'a> {
    path: PathBuf,
    cancel: &'a CancelFlag,
    committed: bool,
}

impl<'a> TempGuard<'a> {
    pub fn new(path: PathBuf, cancel: &'a CancelFlag) -> Self {
        Self {
            path,
            cancel,
            committed: false,
        }
    }

    pub fn mark_committed(&mut self) {
        self.committed = true;
    }
}

impl Drop for TempGuard<'_> {
    fn drop(&mut self) {
        if self.cancel.is_cancelled() && !self.committed {
            cleanup_temp(&self.path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{cleanup_temp, prepare_temp, temp_work_dir, TEMP_MARKER};
    use std::fs;
    use std::path::PathBuf;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_root(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system clock before unix epoch")
            .as_nanos();
        let root = std::env::temp_dir().join(format!("geoforge-commit-{name}-{stamp}"));
        fs::create_dir_all(&root).expect("create commit fixture");
        root
    }

    #[test]
    fn existing_temp_is_refused_without_deleting_sentinel() {
        let root = temp_root("existing");
        let output = root.join("output");
        let temp = temp_work_dir(&output, "task-safe");
        fs::create_dir_all(&temp).expect("create existing temp");
        fs::write(temp.join("sentinel"), b"keep").expect("write sentinel");

        let error = prepare_temp(&output, "task-safe").expect_err("existing temp must fail");
        assert!(error.contains("already exists"), "{error}");
        assert!(temp.join("sentinel").is_file());
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cleanup_only_removes_owned_temp_directory() {
        let root = temp_root("ownership");
        let output = root.join("output");
        let owned = prepare_temp(&output, "task-owned").expect("prepare owned temp");
        fs::write(owned.join("payload"), b"data").expect("write payload");
        cleanup_temp(&owned);
        assert!(!owned.exists());

        let unowned = temp_work_dir(&output, "task-unowned");
        fs::create_dir_all(&unowned).expect("create unowned temp");
        fs::write(unowned.join("sentinel"), b"keep").expect("write unowned sentinel");
        cleanup_temp(&unowned);
        assert!(unowned.join("sentinel").is_file());

        let wrong_marker = temp_work_dir(&output, "task-marker");
        fs::create_dir_all(&wrong_marker).expect("create wrong marker temp");
        fs::write(wrong_marker.join(TEMP_MARKER), "geoforge-task:other\n")
            .expect("write wrong marker");
        cleanup_temp(&wrong_marker);
        assert!(wrong_marker.join(TEMP_MARKER).is_file());

        let _ = fs::remove_dir_all(root);
    }
}
