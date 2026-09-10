//! Sibling staging + atomic rename commit (plan §6 / Phase 13).
//!
//! Layout (same parent as final):
//! ```text
//! /output-parent/
//!     final-output/
//!     .geoforge-stage-<task-id>/
//!     .geoforge-backup-<task-id>/
//! ```
//!
//! Commit never deletes `final` first and never copy-fills into `final`
//! (partial copy would leave a corrupt success surface). Rename only;
//! on failure → `COMMIT_RENAME_FAILED` and restore from backup.

use crate::protocol::{Emitter, Stage};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, MutexGuard};
use std::cell::Cell;

/// Test-only: force the stage→final rename to fail (after backup moved).
static INJECT_FAIL_STAGE_TO_FINAL: AtomicBool = AtomicBool::new(false);
/// Serializes commit_transaction in-process (avoids inject-flag races under cargo test -j).
static COMMIT_SERIAL: Mutex<()> = Mutex::new(());

thread_local! {
    static COMMIT_LOCK_DEPTH: Cell<u32> = const { Cell::new(0) };
}

/// Enable/disable injected stage→final rename failure.
pub fn inject_fail_stage_to_final(fail: bool) {
    INJECT_FAIL_STAGE_TO_FINAL.store(fail, Ordering::SeqCst);
}

/// Run `f` with inject-fail enabled under the commit serial lock.
pub fn with_inject_fail_stage_to_final<R>(f: impl FnOnce() -> R) -> R {
    let _guard = acquire_commit_serial();
    INJECT_FAIL_STAGE_TO_FINAL.store(true, Ordering::SeqCst);
    let out = f();
    INJECT_FAIL_STAGE_TO_FINAL.store(false, Ordering::SeqCst);
    out
}

fn stage_rename_injected_fail() -> bool {
    INJECT_FAIL_STAGE_TO_FINAL.load(Ordering::SeqCst)
}

#[allow(dead_code)]
struct CommitSerialGuard(Option<MutexGuard<'static, ()>>);
impl Drop for CommitSerialGuard {
    fn drop(&mut self) {
        COMMIT_LOCK_DEPTH.with(|d| {
            let v = d.get().saturating_sub(1);
            d.set(v);
        });
    }
}

fn acquire_commit_serial() -> CommitSerialGuard {
    COMMIT_LOCK_DEPTH.with(|d| {
        if d.get() > 0 {
            d.set(d.get() + 1);
            CommitSerialGuard(None)
        } else {
            let g = COMMIT_SERIAL.lock().unwrap_or_else(|e| e.into_inner());
            d.set(1);
            CommitSerialGuard(Some(g))
        }
    })
}

/// `<output-parent>/.geoforge-stage-<task-id>/`
pub fn staging_dir(final_output: &Path, task_id: &str) -> PathBuf {
    let parent = final_output.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(".geoforge-stage-{task_id}"))
}

/// `<output-parent>/.geoforge-backup-<task-id>/`
pub fn backup_dir(final_output: &Path, task_id: &str) -> PathBuf {
    let parent = final_output.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(".geoforge-backup-{task_id}"))
}

/// Marker left beside final when a run is interrupted / cancelled before commit.
pub fn interrupted_marker(final_output: &Path, task_id: &str) -> PathBuf {
    let parent = final_output.parent().unwrap_or_else(|| Path::new("."));
    parent.join(format!(".geoforge-interrupted-{task_id}"))
}

/// Backward-compatible alias — staging is the task work root.
pub fn temp_work_dir(final_output: &Path, task_id: &str) -> PathBuf {
    staging_dir(final_output, task_id)
}

/// Recover a half-finished prior commit (backup present, final missing), then
/// prepare a clean sibling staging directory for this task.
pub fn prepare_temp(final_output: &Path, task_id: &str) -> Result<PathBuf, String> {
    prepare_staging(final_output, task_id)
}

pub fn prepare_staging(final_output: &Path, task_id: &str) -> Result<PathBuf, String> {
    recover_interrupted_commit(final_output, task_id)?;

    let stage = staging_dir(final_output, task_id);
    if stage.exists() {
        fs::remove_dir_all(&stage).map_err(|e| format!("clean staging: {e}"))?;
    }
    // Drop stale interrupt marker from a previous run once we start fresh.
    let marker = interrupted_marker(final_output, task_id);
    if marker.exists() {
        let _ = fs::remove_file(&marker);
    }
    fs::create_dir_all(&stage).map_err(|e| format!("create staging: {e}"))?;
    Ok(stage)
}

/// If a previous process died after `final → backup` but before `stage → final`,
/// restore backup so the user's last good result remains available.
pub fn recover_interrupted_commit(final_output: &Path, task_id: &str) -> Result<(), String> {
    let backup = backup_dir(final_output, task_id);
    if backup.exists() && !final_output.exists() {
        rename_dir(&backup, final_output).map_err(|e| {
            format!("recover backup → final failed: {e}")
        })?;
    } else if backup.exists() && final_output.exists() {
        // Both exist: prefer final, drop leftover backup.
        let _ = fs::remove_dir_all(&backup);
    }
    Ok(())
}

/// Refuse paths that would overwrite source / escape into final mid-write.
pub fn assert_safe_commit_paths(
    stage_content: &Path,
    final_output: &Path,
    source: Option<&Path>,
) -> Result<(), String> {
    if path_eq_loose(stage_content, final_output) {
        return Err(
            "COMMIT_REFUSED: stage path must not equal final (no in-place overwrite)".into(),
        );
    }
    if is_within(stage_content, final_output) {
        return Err(
            "COMMIT_REFUSED: staging must be a sibling of final, not inside it".into(),
        );
    }
    if let Some(src) = source {
        if paths_same_existing(final_output, src) || path_eq_loose(final_output, src) {
            return Err(
                "COMMIT_REFUSED: output must not overwrite source input in place".into(),
            );
        }
        if paths_same_existing(stage_content, src) || path_eq_loose(stage_content, src) {
            return Err("COMMIT_REFUSED: staging must not be the source input".into());
        }
    }
    Ok(())
}

/// Commit validated staging content to final via rename transaction (plan §6.2).
///
/// `stage_content` is the directory whose tree becomes `final_output` (often a
/// subfolder of `.geoforge-stage-<id>/` that already holds `tileset.json`).
pub fn commit_rename(
    emitter: &Emitter,
    stage_content: &Path,
    final_output: &Path,
) -> Result<(), String> {
    // task_id inferred from staging parent name when possible; backup uses a
    // stable sibling name derived from final + "commit" when unknown.
    let task_id = infer_task_id_from_stage(stage_content).unwrap_or_else(|| "commit".into());
    commit_transaction(emitter, stage_content, final_output, &task_id, None)
}

pub fn commit_transaction(
    emitter: &Emitter,
    stage_content: &Path,
    final_output: &Path,
    task_id: &str,
    source: Option<&Path>,
) -> Result<(), String> {
    let _commit_serial = acquire_commit_serial();

    emitter.stage(Stage::Commit, "Committing output (atomic rename)");

    assert_safe_commit_paths(stage_content, final_output, source)?;

    if !stage_content.is_dir() {
        return Err("COMMIT_REFUSED: stage content is not a directory".into());
    }
    // Never claim success on an empty / incomplete stage.
    if dir_is_effectively_empty(stage_content) {
        return Err("COMMIT_REFUSED: stage content is empty (incomplete result)".into());
    }

    if let Some(parent) = final_output.parent() {
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }

    let manifest = stage_content.join(".geoforge-manifest.json");
    let _ = fs::write(
        &manifest,
        serde_json::json!({
            "committedAt": chrono::Utc::now().to_rfc3339(),
            "finalPath": final_output.to_string_lossy(),
            "taskId": task_id,
            "commitProtocol": "phase13-sibling-rename",
        })
        .to_string(),
    );

    // Best-effort durability before rename.
    sync_dir_best_effort(stage_content);

    let backup = backup_dir(final_output, task_id);
    // Ensure no stale backup from a prior attempt.
    if backup.exists() {
        fs::remove_dir_all(&backup).map_err(|e| format!("clean stale backup: {e}"))?;
    }

    if !final_output.exists() {
        rename_dir(stage_content, final_output).map_err(|e| {
            format!("COMMIT_RENAME_FAILED: stage → final: {e}")
        })?;
        emitter.log(&format!("[commit] renamed stage → {}", final_output.display()));
        return Ok(());
    }

    // final exists: rename(final, backup) → rename(stage, final) → drop backup
    // failure after backup → restore backup → final
    rename_dir(final_output, &backup).map_err(|e| {
        format!("COMMIT_RENAME_FAILED: final → backup: {e}")
    })?;

    match rename_stage_to_final(stage_content, final_output) {
        Ok(()) => {
            let _ = fs::remove_dir_all(&backup);
            emitter.log(&format!(
                "[commit] replaced {} (backup removed)",
                final_output.display()
            ));
            Ok(())
        }
        Err(e) => {
            // Rollback: restore previous final.
            if final_output.exists() {
                let _ = fs::remove_dir_all(final_output);
            }
            if let Err(re) = rename_dir(&backup, final_output) {
                return Err(format!(
                    "COMMIT_RENAME_FAILED: stage → final failed ({e}); rollback also failed ({re})"
                ));
            }
            Err(format!(
                "COMMIT_RENAME_FAILED: stage → final failed ({e}); previous final restored from backup"
            ))
        }
    }
}

fn rename_stage_to_final(stage: &Path, final_output: &Path) -> Result<(), String> {
    if stage_rename_injected_fail() {
        return Err("injected failure".into());
    }
    rename_dir(stage, final_output)
}

fn rename_dir(from: &Path, to: &Path) -> Result<(), String> {
    fs::rename(from, to).map_err(|e| e.to_string())
}

/// Mark interrupted and remove staging (cancel / fail paths). Source untouched.
pub fn cleanup_temp(temp_dir: &Path) {
    cleanup_staging(temp_dir, None);
}

pub fn cleanup_staging(stage_dir: &Path, final_output: Option<&Path>) {
    if let Some(final_out) = final_output {
        if let Some(task_id) = infer_task_id_from_stage(stage_dir) {
            mark_interrupted(final_out, &task_id);
        }
    }
    if stage_dir.exists() {
        let _ = fs::remove_dir_all(stage_dir);
    }
}

pub fn mark_interrupted(final_output: &Path, task_id: &str) {
    let marker = interrupted_marker(final_output, task_id);
    let body = serde_json::json!({
        "taskId": task_id,
        "finalPath": final_output.to_string_lossy(),
        "interruptedAt": chrono::Utc::now().to_rfc3339(),
        "note": "staging cleaned or abandoned; final not committed",
    });
    if let Some(parent) = marker.parent() {
        let _ = fs::create_dir_all(parent);
    }
    let _ = fs::write(marker, body.to_string());
}

fn infer_task_id_from_stage(stage_content: &Path) -> Option<String> {
    // Walk up looking for `.geoforge-stage-<id>`.
    for p in stage_content.ancestors().take(4) {
        if let Some(name) = p.file_name().and_then(|s| s.to_str()) {
            if let Some(id) = name.strip_prefix(".geoforge-stage-") {
                return Some(id.to_string());
            }
            // Legacy name from Phase 3.
            if let Some(id) = name.strip_prefix(".geoforge-task-") {
                return Some(id.to_string());
            }
        }
    }
    None
}

fn dir_is_effectively_empty(dir: &Path) -> bool {
    match fs::read_dir(dir) {
        Ok(mut it) => it.next().is_none(),
        Err(_) => true,
    }
}

fn path_eq_loose(a: &Path, b: &Path) -> bool {
    a == b || a.as_os_str() == b.as_os_str()
}

fn paths_same_existing(a: &Path, b: &Path) -> bool {
    match (fs::canonicalize(a), fs::canonicalize(b)) {
        (Ok(ca), Ok(cb)) => ca == cb,
        _ => false,
    }
}

fn is_within(child: &Path, ancestor: &Path) -> bool {
    match (fs::canonicalize(child), fs::canonicalize(ancestor)) {
        (Ok(c), Ok(a)) => c.starts_with(&a) && c != a,
        _ => {
            // Fallback: prefix check on lexical paths.
            child.starts_with(ancestor) && child != ancestor
        }
    }
}

fn sync_dir_best_effort(dir: &Path) {
    // fsync directory on Unix after writes; ignore errors (tmpfs / permissions).
    #[cfg(unix)]
    {
        if let Ok(f) = fs::File::open(dir) {
            let _ = f.sync_all();
        }
    }
    let _ = dir;
}

/// Directory tree fingerprint for cancel/crash assertions (path + size + bytes hash).
pub fn dir_fingerprint(root: &Path) -> Result<String, String> {
    use std::collections::BTreeMap;
    fn walk(dir: &Path, base: &Path, out: &mut BTreeMap<String, String>) -> Result<(), String> {
        let mut entries: Vec<_> = fs::read_dir(dir)
            .map_err(|e| e.to_string())?
            .collect::<Result<Vec<_>, _>>()
            .map_err(|e| e.to_string())?;
        entries.sort_by_key(|e| e.file_name());
        for ent in entries {
            let path = ent.path();
            let rel = path
                .strip_prefix(base)
                .unwrap_or(&path)
                .to_string_lossy()
                .replace('\\', "/");
            let ty = ent.file_type().map_err(|e| e.to_string())?;
            if ty.is_dir() {
                walk(&path, base, out)?;
            } else if ty.is_file() {
                let bytes = fs::read(&path).map_err(|e| e.to_string())?;
                out.insert(rel, format!("{}:{}", bytes.len(), simple_hash(&bytes)));
            }
        }
        Ok(())
    }
    let mut map = BTreeMap::new();
    walk(root, root, &mut map)?;
    let mut acc = String::new();
    for (k, v) in map {
        acc.push_str(&k);
        acc.push('=');
        acc.push_str(&v);
        acc.push('\n');
    }
    Ok(format!("{:016x}", simple_hash(acc.as_bytes())))
}

fn simple_hash(bytes: &[u8]) -> u64 {
    // FNV-1a 64 — good enough for test fingerprints, no extra deps.
    let mut h: u64 = 0xcbf29ce484222325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

#[cfg(test)]
mod unit_tests {
    use super::*;
    use crate::protocol::Emitter;
    use std::sync::atomic::{AtomicU64, Ordering as AtmOrd};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn scratch(name: &str) -> PathBuf {
        let n = SEQ.fetch_add(1, AtmOrd::SeqCst);
        let d = std::env::temp_dir().join(format!("gf_p13_{name}_{n}"));
        let _ = fs::remove_dir_all(&d);
        fs::create_dir_all(&d).unwrap();
        d
    }

    fn write_tree(dir: &Path, marker: &str) {
        fs::create_dir_all(dir).unwrap();
        fs::write(dir.join("tileset.json"), format!(r#"{{"marker":"{marker}"}}"#)).unwrap();
        fs::write(dir.join("blob.bin"), marker.as_bytes()).unwrap();
    }

    #[test]
    fn staging_is_sibling_not_inside_final() {
        let root = scratch("sibling");
        let final_out = root.join("out");
        fs::create_dir_all(&final_out).unwrap();
        let stage = prepare_staging(&final_out, "t1").unwrap();
        assert_eq!(stage, root.join(".geoforge-stage-t1"));
        assert!(!stage.starts_with(&final_out) || stage == root.join(".geoforge-stage-t1"));
        assert_eq!(stage.parent(), final_out.parent());
    }

    #[test]
    fn commit_fresh_rename() {
        let root = scratch("fresh");
        let final_out = root.join("out");
        let stage = prepare_staging(&final_out, "t2").unwrap();
        let content = stage.join("staged");
        write_tree(&content, "v1");
        let em = Emitter::new("t2");
        commit_transaction(&em, &content, &final_out, "t2", None).unwrap();
        assert!(final_out.join("tileset.json").is_file());
        assert!(!content.exists());
        assert!(!backup_dir(&final_out, "t2").exists());
    }

    #[test]
    fn commit_replace_removes_backup_on_success() {
        let root = scratch("replace");
        let final_out = root.join("out");
        write_tree(&final_out, "old");
        let old_fp = dir_fingerprint(&final_out).unwrap();

        let stage = prepare_staging(&final_out, "t3").unwrap();
        let content = stage.join("staged");
        write_tree(&content, "new");
        let em = Emitter::new("t3");
        commit_transaction(&em, &content, &final_out, "t3", None).unwrap();

        let body = fs::read_to_string(final_out.join("blob.bin")).unwrap();
        assert_eq!(body, "new");
        assert!(!backup_dir(&final_out, "t3").exists());
        let new_fp = dir_fingerprint(&final_out).unwrap();
        assert_ne!(old_fp, new_fp);
    }

    #[test]
    fn commit_rename_failure_restores_backup() {
        let root = scratch("rollback");
        let final_out = root.join("out");
        write_tree(&final_out, "keep-me");
        let before = dir_fingerprint(&final_out).unwrap();

        let stage = prepare_staging(&final_out, "t4").unwrap();
        let content = stage.join("staged");
        write_tree(&content, "should-not-land");

        let em = Emitter::new("t4");
        let err = with_inject_fail_stage_to_final(|| {
            commit_transaction(&em, &content, &final_out, "t4", None).unwrap_err()
        });

        assert!(err.contains("COMMIT_RENAME_FAILED"), "{err}");
        assert!(final_out.exists(), "final must be restored");
        let after = dir_fingerprint(&final_out).unwrap();
        assert_eq!(before, after, "original final fingerprint must be unchanged");
        let body = fs::read_to_string(final_out.join("blob.bin")).unwrap();
        assert_eq!(body, "keep-me");
    }

    #[test]
    fn refuse_empty_stage() {
        let root = scratch("empty");
        let final_out = root.join("out");
        let stage = prepare_staging(&final_out, "t5").unwrap();
        let content = stage.join("staged");
        fs::create_dir_all(&content).unwrap();
        let em = Emitter::new("t5");
        let err = commit_transaction(&em, &content, &final_out, "t5", None).unwrap_err();
        assert!(err.contains("COMMIT_REFUSED"));
        assert!(!final_out.exists());
    }

    #[test]
    fn refuse_source_overwrite() {
        let root = scratch("srcow");
        let source = root.join("source");
        write_tree(&source, "src");
        let stage = prepare_staging(&source, "t6").unwrap();
        let content = stage.join("staged");
        write_tree(&content, "bad");
        let em = Emitter::new("t6");
        let err =
            commit_transaction(&em, &content, &source, "t6", Some(&source)).unwrap_err();
        assert!(err.contains("COMMIT_REFUSED"), "{err}");
        assert_eq!(
            fs::read_to_string(source.join("blob.bin")).unwrap(),
            "src"
        );
    }

    #[test]
    fn cancel_cleanup_leaves_final_intact() {
        let root = scratch("cancel");
        let final_out = root.join("out");
        write_tree(&final_out, "stable");
        let before = dir_fingerprint(&final_out).unwrap();

        let stage = prepare_staging(&final_out, "t7").unwrap();
        write_tree(&stage.join("work"), "partial");
        // Simulate cancel before commit:
        mark_interrupted(&final_out, "t7");
        cleanup_staging(&stage, Some(&final_out));

        assert!(!stage.exists());
        assert!(interrupted_marker(&final_out, "t7").is_file());
        assert_eq!(dir_fingerprint(&final_out).unwrap(), before);
    }

    #[test]
    fn recover_backup_when_final_missing() {
        let root = scratch("recover");
        let final_out = root.join("out");
        let backup = backup_dir(&final_out, "t8");
        write_tree(&backup, "from-backup");
        assert!(!final_out.exists());
        recover_interrupted_commit(&final_out, "t8").unwrap();
        assert!(final_out.join("blob.bin").is_file());
        assert_eq!(
            fs::read_to_string(final_out.join("blob.bin")).unwrap(),
            "from-backup"
        );
        assert!(!backup.exists());
    }
}
