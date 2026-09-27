//! Temp dir → final output rename/commit (plan §7.4 / T01).

use crate::path_policy::{self, validate_task_id};
use crate::protocol::{Emitter, Stage};
use std::fs;
use std::path::{Path, PathBuf};

/// `<output-parent>/.geoforge-task-<task-id>/`
const TEMP_MARKER: &str = ".geoforge-owned";
const CHECKPOINT_FILE: &str = ".geoforge-checkpoint";

/// Pipeline stage checkpoints for crash recovery diagnostics
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Checkpoint {
    Prepared,
    Converting,
    Converted,
    Rebuilding,
    Rebuilt,
    Texturing,
    Textured,
    Validating,
    Validated,
    Committing,
    Committed,
}

impl Checkpoint {
    pub fn as_str(&self) -> &'static str {
        match self {
            Checkpoint::Prepared => "prepared",
            Checkpoint::Converting => "converting",
            Checkpoint::Converted => "converted",
            Checkpoint::Rebuilding => "rebuilding",
            Checkpoint::Rebuilt => "rebuilt",
            Checkpoint::Texturing => "texturing",
            Checkpoint::Textured => "textured",
            Checkpoint::Validating => "validating",
            Checkpoint::Validated => "validated",
            Checkpoint::Committing => "committing",
            Checkpoint::Committed => "committed",
        }
    }
    
    pub fn from_str(s: &str) -> Option<Self> {
        match s {
            "prepared" => Some(Checkpoint::Prepared),
            "converting" => Some(Checkpoint::Converting),
            "converted" => Some(Checkpoint::Converted),
            "rebuilding" => Some(Checkpoint::Rebuilding),
            "rebuilt" => Some(Checkpoint::Rebuilt),
            "texturing" => Some(Checkpoint::Texturing),
            "textured" => Some(Checkpoint::Textured),
            "validating" => Some(Checkpoint::Validating),
            "validated" => Some(Checkpoint::Validated),
            "committing" => Some(Checkpoint::Committing),
            "committed" => Some(Checkpoint::Committed),
            _ => None,
        }
    }
}

/// Write a checkpoint marker to track pipeline progress
pub fn write_checkpoint(temp_dir: &Path, checkpoint: Checkpoint) -> Result<(), String> {
    let checkpoint_path = temp_dir.join(CHECKPOINT_FILE);
    fs::write(&checkpoint_path, format!("{}\n", checkpoint.as_str()))
        .map_err(|e| format!("failed to write checkpoint: {}", e))
}

/// Read the last checkpoint from a temp directory
pub fn read_checkpoint(temp_dir: &Path) -> Option<Checkpoint> {
    let checkpoint_path = temp_dir.join(CHECKPOINT_FILE);
    let content = fs::read_to_string(&checkpoint_path).ok()?;
    Checkpoint::from_str(content.trim())
}

pub fn temp_work_dir(final_output: &Path, task_id: &str) -> PathBuf {
    let parent = final_output.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(".geoforge-task-{task_id}"))
}

pub fn prepare_temp(final_output: &Path, task_id: &str) -> Result<PathBuf, String> {
    validate_task_id(task_id)?;
    let temp = temp_work_dir(final_output, task_id);
    
    // Exclusive create: intentionally no check-then-act race window.
    // If the directory exists, we refuse to proceed rather than risking
    // data loss from deleting unknown content or race with another process.
    match fs::create_dir(&temp) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            // Diagnose existing temp directory
            let checkpoint_info = read_checkpoint(&temp)
                .map(|cp| format!(" Last checkpoint: {}", cp.as_str()))
                .unwrap_or_else(|| " No checkpoint found.".to_string());
            
            return Err(format!(
                "temporary work directory already exists (refusing to overwrite): {}{} \
                 This may indicate: (1) a previous run was interrupted, (2) another process \
                 is using the same task ID, or (3) leftover state from a crash. \
                 If you're certain no other process is using this directory, manually remove it \
                 and retry.",
                temp.display(),
                checkpoint_info
            ));
        }
        Err(error) => {
            return Err(format!(
                "failed to create temporary work directory: {} ({})",
                temp.display(),
                error
            ))
        }
    }
    
    // Write ownership marker
    let marker = temp.join(TEMP_MARKER);
    if let Err(error) = fs::write(&marker, format!("geoforge-task:{task_id}\n")) {
        // Cleanup: remove the directory we just created
        let _ = fs::remove_dir(&temp);
        return Err(format!(
            "failed to write ownership marker: {} ({})",
            marker.display(),
            error
        ));
    }
    
    // Write initial checkpoint
    if let Err(error) = write_checkpoint(&temp, Checkpoint::Prepared) {
        let _ = fs::remove_dir_all(&temp);
        return Err(error);
    }
    
    Ok(temp)
}

/// Commit temp directory to final output path (no-replace; no copy fallback).
/// 
/// Atomicity guarantee: once `rename_no_replace` succeeds, the output is committed
/// and will not be removed by late cancellation. The temp guard is marked committed
/// before returning, preventing cleanup even if cancellation arrives during manifest write.
pub fn commit_rename(
    emitter: &Emitter,
    staged_dir: &Path,
    owned_temp_dir: &Path,
    final_output: &Path,
    temp_guard: Option<&mut TempGuard>,
) -> Result<(), String> {
    emitter.stage(Stage::Commit, "Committing output");
    ensure_owned_temp(owned_temp_dir)?;
    ensure_staged_output(staged_dir, owned_temp_dir)?;

    if let Some(parent) = final_output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    // Atomic rename: once this succeeds, output is committed
    path_policy::rename_no_replace(staged_dir, final_output)?;
    
    // Mark committed IMMEDIATELY after successful rename to prevent late-cancel cleanup
    if let Some(guard) = temp_guard {
        guard.mark_committed();
    }

    // Best-effort manifest write (non-critical; failure doesn't affect commit)
    let manifest = final_output.join(".geoforge-manifest.json");
    let _ = fs::write(
        &manifest,
        serde_json::json!({
            "committedAt": chrono::Utc::now().to_rfc3339(),
            "finalPath": final_output.to_string_lossy(),
        })
        .to_string(),
    );

    emitter.log(&format!("[commit] {}", final_output.display()));
    Ok(())
}

fn ensure_staged_output(staged_dir: &Path, owned_temp_dir: &Path) -> Result<(), String> {
    let staged_metadata = fs::symlink_metadata(staged_dir).map_err(|error| {
        format!("staged output is missing or inaccessible: {} ({error})", staged_dir.display())
    })?;
    if !staged_metadata.is_dir() || staged_metadata.file_type().is_symlink() {
        return Err(format!("staged output must be a real directory: {}", staged_dir.display()));
    }

    let staged_parent = staged_dir
        .parent()
        .ok_or_else(|| format!("staged output has no parent: {}", staged_dir.display()))?;
    let canonical_parent = fs::canonicalize(staged_parent).map_err(|error| {
        format!("cannot resolve staged output parent {}: {error}", staged_parent.display())
    })?;
    let canonical_temp = fs::canonicalize(owned_temp_dir).map_err(|error| {
        format!("cannot resolve owned temporary directory {}: {error}", owned_temp_dir.display())
    })?;
    if canonical_parent != canonical_temp {
        return Err(format!(
            "staged output must be a direct child of its owned temporary directory: {}",
            staged_dir.display()
        ));
    }
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
    
    // Read and validate marker content (normalize line endings)
    let marker_path = temp_dir.join(TEMP_MARKER);
    let Ok(marker_content) = fs::read_to_string(&marker_path) else {
        return false;
    };
    
    // Compare with expected, normalizing both sides (trim handles \r\n, \n, or no newline)
    marker_content.trim() == format!("geoforge-task:{task_id}")
}

fn ensure_owned_temp(temp_dir: &Path) -> Result<(), String> {
    // Check directory exists and is not a symlink
    let metadata = fs::symlink_metadata(temp_dir).map_err(|error| {
        format!(
            "temporary work directory is missing or inaccessible: {} ({})",
            temp_dir.display(),
            error
        )
    })?;
    
    // Check symlink BEFORE checking is_dir (symlinks can point to dirs)
    if metadata.file_type().is_symlink() {
        return Err(format!(
            "temporary work directory is a symlink (security risk): {}",
            temp_dir.display()
        ));
    }
    
    if !metadata.is_dir() {
        return Err(format!(
            "temporary work directory is not a directory: {}",
            temp_dir.display()
        ));
    }
    
    // Check ownership marker
    if owns_temp_dir(temp_dir) {
        return Ok(());
    }
    
    // Fallback: allow if parent directory is owned (for nested operations)
    if temp_dir.parent().map(owns_temp_dir).unwrap_or(false) {
        return Ok(());
    }
    
    // Detailed error message
    let marker_path = temp_dir.join(TEMP_MARKER);
    if !marker_path.exists() {
        return Err(format!(
            "temporary work directory missing ownership marker: {} (expected {})",
            temp_dir.display(),
            marker_path.display()
        ));
    }
    
    // Marker exists but content is wrong
    let marker_content = fs::read_to_string(&marker_path)
        .unwrap_or_else(|_| "<unreadable>".to_string());
    
    let Some(task_id) = temp_dir
        .file_name()
        .and_then(|n| n.to_str())
        .and_then(|n| n.strip_prefix(".geoforge-task-"))
    else {
        return Err(format!(
            "temporary work directory name does not match pattern '.geoforge-task-<id>': {}",
            temp_dir.display()
        ));
    };
    
    Err(format!(
        "temporary work directory ownership marker mismatch: {} (expected 'geoforge-task:{}', found '{}')",
        temp_dir.display(),
        task_id,
        marker_content.trim()
    ))
}

/// Removes this run's owned work directory whenever processing exits before commit.
///
/// # Semantics
///
/// - **Failure or cancel before commit**: cleanup removes the owned temp directory
/// - **Success after commit**: cleanup is suppressed by `mark_committed()`; output remains
/// - **Late cancel after commit**: the committed output is never removed by this guard
pub struct TempGuard {
    path: PathBuf,
    committed: bool,
}

impl TempGuard {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            committed: false,
        }
    }

    pub fn mark_committed(&mut self) {
        self.committed = true;
    }
}

impl Drop for TempGuard {
    fn drop(&mut self) {
        if !self.committed {
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
        assert!(error.contains("refusing"), "{error}");
        assert!(temp.join("sentinel").is_file(), "sentinel must be preserved");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn ownership_marker_mismatch_gives_detailed_error() {
        use super::{ensure_owned_temp, TEMP_MARKER};
        
        let root = temp_root("marker-mismatch");
        let output = root.join("output");
        let temp = temp_work_dir(&output, "task-check");
        fs::create_dir_all(&temp).expect("create temp");
        fs::write(temp.join(TEMP_MARKER), "geoforge-task:wrong-id\n")
            .expect("write wrong marker");
        
        let error = ensure_owned_temp(&temp).expect_err("wrong marker must fail");
        assert!(error.contains("mismatch"), "{error}");
        assert!(error.contains("task-check"), "{error}");
        assert!(error.contains("wrong-id"), "{error}");
        
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_ownership_marker_gives_detailed_error() {
        use super::ensure_owned_temp;
        
        let root = temp_root("marker-missing");
        let output = root.join("output");
        let temp = temp_work_dir(&output, "task-nomarker");
        fs::create_dir_all(&temp).expect("create temp");
        
        let error = ensure_owned_temp(&temp).expect_err("missing marker must fail");
        assert!(error.contains("missing ownership marker"), "{error}");
        assert!(error.contains(TEMP_MARKER), "{error}");
        
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    #[cfg(unix)]
    fn symlink_temp_is_rejected() {
        use super::ensure_owned_temp;
        
        let root = temp_root("symlink");
        let output = root.join("output");
        let real_dir = root.join("real");
        fs::create_dir_all(&real_dir).expect("create real dir");
        
        let temp = temp_work_dir(&output, "task-symlink");
        std::os::unix::fs::symlink(&real_dir, &temp).expect("create symlink");
        let error = ensure_owned_temp(&temp).expect_err("symlink must fail");
        assert!(error.contains("symlink"), "{error}");
        assert!(error.contains("security risk"), "{error}");
        
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checkpoint_write_and_read() {
        use super::{read_checkpoint, write_checkpoint, Checkpoint};
        
        let root = temp_root("checkpoint");
        let output = root.join("output");
        let temp = temp_work_dir(&output, "task-cp");
        fs::create_dir_all(&temp).expect("create temp");
        
        // Write and read checkpoint
        write_checkpoint(&temp, Checkpoint::Converting).expect("write checkpoint");
        let cp = read_checkpoint(&temp).expect("read checkpoint");
        assert_eq!(cp, Checkpoint::Converting);
        
        // Overwrite checkpoint
        write_checkpoint(&temp, Checkpoint::Validated).expect("write checkpoint 2");
        let cp2 = read_checkpoint(&temp).expect("read checkpoint 2");
        assert_eq!(cp2, Checkpoint::Validated);
        
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn prepare_temp_includes_checkpoint() {
        use super::{prepare_temp, read_checkpoint, Checkpoint};
        
        let root = temp_root("prepare-cp");
        let output = root.join("output");
        
        let temp = prepare_temp(&output, "task-prep").expect("prepare temp");
        
        // Check checkpoint exists and is Prepared
        let cp = read_checkpoint(&temp).expect("read checkpoint");
        assert_eq!(cp, Checkpoint::Prepared);
        
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn existing_temp_reports_last_checkpoint() {
        use super::{prepare_temp, write_checkpoint, Checkpoint};
        
        let root = temp_root("existing-cp");
        let output = root.join("output");
        
        // Create temp with a checkpoint
        let temp = prepare_temp(&output, "task-exist").expect("prepare temp");
        write_checkpoint(&temp, Checkpoint::Converting).expect("write checkpoint");
        
        // Try to prepare again
        let error = prepare_temp(&output, "task-exist").expect_err("should fail");
        assert!(error.contains("already exists"), "{error}");
        assert!(error.contains("Last checkpoint: converting"), "{error}");
        
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn checkpoint_roundtrip_all_stages() {
        use super::Checkpoint;
        
        let stages = [
            Checkpoint::Prepared,
            Checkpoint::Converting,
            Checkpoint::Converted,
            Checkpoint::Rebuilding,
            Checkpoint::Rebuilt,
            Checkpoint::Texturing,
            Checkpoint::Textured,
            Checkpoint::Validating,
            Checkpoint::Validated,
            Checkpoint::Committing,
            Checkpoint::Committed,
        ];
        
        for stage in &stages {
            let s = stage.as_str();
            let parsed = Checkpoint::from_str(s).expect("parse checkpoint");
            assert_eq!(*stage, parsed, "roundtrip failed for {:?}", stage);
        }
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

    #[test]
    fn late_cancel_after_commit_does_not_remove_output() {
        use super::TempGuard;
        use crate::cancel::CancelFlag;
        
        let root = temp_root("late-cancel");
        let output = root.join("output");
        let temp = prepare_temp(&output, "task-commit").expect("prepare temp");
        fs::write(temp.join("data.txt"), b"committed").expect("write data");
        
        let cancel = CancelFlag::new();
        let mut guard = TempGuard::new(temp.clone());
        
        // Simulate commit: rename succeeds, guard is marked
        fs::rename(&temp, &output).expect("rename temp to output");
        guard.mark_committed();
        
        // Late cancel arrives AFTER commit
        cancel.request();
        
        // Drop guard (simulates end of scope)
        drop(guard);
        
        // Output must still exist (not removed by late cancel)
        assert!(output.exists(), "committed output must not be removed by late cancel");
        assert_eq!(fs::read(output.join("data.txt")).unwrap(), b"committed");
        
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn commits_staged_output_from_owned_temp_directory() {
        use super::{commit_rename, TempGuard};
        use crate::protocol::Emitter;

        let root = temp_root("staged-commit");
        let output = root.join("output");
        let temp = prepare_temp(&output, "task-staged").expect("prepare owned temp");
        let staged = temp.join("staged");
        fs::create_dir(&staged).expect("create staged output");
        fs::write(staged.join("tileset.json"), b"{}").expect("write tileset");

        let mut guard = TempGuard::new(temp.clone());
        let emitter = Emitter::new("task-staged");
        commit_rename(&emitter, &staged, &temp, &output, Some(&mut guard))
            .expect("commit staged model output");
        cleanup_temp(&temp);

        assert!(output.join("tileset.json").is_file());
        assert!(!temp.exists());
        drop(guard);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_to_commit_staged_output_outside_owned_temp_directory() {
        use super::{commit_rename, TempGuard};
        use crate::protocol::Emitter;

        let root = temp_root("staged-outside");
        let output = root.join("output");
        let temp = prepare_temp(&output, "task-staged-outside").expect("prepare owned temp");
        let foreign = root.join("foreign");
        fs::create_dir(&foreign).expect("create foreign output");
        fs::write(foreign.join("tileset.json"), b"{}").expect("write tileset");

        let mut guard = TempGuard::new(temp.clone());
        let emitter = Emitter::new("task-staged-outside");
        let error = commit_rename(&emitter, &foreign, &temp, &output, Some(&mut guard))
            .expect_err("foreign output must not be committed");

        assert!(error.contains("direct child"));
        assert!(!output.exists());
        cleanup_temp(&temp);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn existing_final_output_is_preserved_when_commit_is_refused() {
        use super::{commit_rename, TempGuard};
        use crate::protocol::Emitter;

        let root = temp_root("final-exists");
        let output = root.join("output");
        fs::create_dir(&output).expect("create existing output");
        fs::write(output.join("sentinel.txt"), b"keep existing output")
            .expect("write output sentinel");

        let temp = prepare_temp(&output, "task-final-exists").expect("prepare owned temp");
        let staged = temp.join("staged");
        fs::create_dir(&staged).expect("create staged output");
        fs::write(staged.join("tileset.json"), b"{}")
            .expect("write staged tileset");

        let mut guard = TempGuard::new(temp.clone());
        let emitter = Emitter::new("task-final-exists");
        let error = commit_rename(&emitter, &staged, &temp, &output, Some(&mut guard))
            .expect_err("existing final output must not be replaced");

        assert!(!error.is_empty());
        assert_eq!(fs::read(output.join("sentinel.txt")).unwrap(), b"keep existing output");
        assert!(staged.join("tileset.json").is_file());
        cleanup_temp(&temp);
        drop(guard);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_staged_output_is_not_committed() {
        use super::{commit_rename, TempGuard};
        use crate::protocol::Emitter;

        let root = temp_root("staged-missing");
        let output = root.join("output");
        let temp = prepare_temp(&output, "task-staged-missing").expect("prepare owned temp");
        let staged = temp.join("staged");

        let mut guard = TempGuard::new(temp.clone());
        let emitter = Emitter::new("task-staged-missing");
        let error = commit_rename(&emitter, &staged, &temp, &output, Some(&mut guard))
            .expect_err("missing staged output must not commit");

        assert!(error.contains("staged output is missing"), "{error}");
        assert!(!output.exists());
        cleanup_temp(&temp);
        drop(guard);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn uncommitted_temp_is_removed_on_error_scope_exit() {
        use super::TempGuard;

        let root = temp_root("failed-task-cleanup");
        let output = root.join("output");
        let temp = prepare_temp(&output, "task-failed-cleanup").expect("prepare owned temp");
        fs::write(temp.join("partial-output"), b"incomplete")
            .expect("write partial output");

        {
            let _guard = TempGuard::new(temp.clone());
            // Returning an error from any pipeline stage drops the guard.
        }

        assert!(!temp.exists(), "failed task must not leave its owned temp directory");
        let _ = fs::remove_dir_all(root);
    }
}
