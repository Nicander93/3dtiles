//! Phase 13 — sibling staging, atomic commit, cancel/crash safety.
//!
//! Plan §6: never delete-final-then-copy; backup rename + rollback;
//! cancel/crash leaves prior final intact; no success on partial.

use processor::{
    backup_dir, cleanup_staging, commit_transaction, dir_fingerprint, with_inject_fail_stage_to_final,
    interrupted_marker, mark_interrupted, prepare_staging, recover_interrupted_commit, staging_dir,
    CancelFlag, Emitter, TaskConfig, EXIT_CANCELLED, EXIT_OK,
};
use serde_json::json;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("phase13_{name}_{n}"));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn write_tree(dir: &Path, marker: &str) {
    fs::create_dir_all(dir).unwrap();
    fs::write(
        dir.join("tileset.json"),
        format!(
            r#"{{"asset":{{"version":"1.0"}},"marker":"{marker}","geometricError":1.0,"root":{{"boundingVolume":{{"box":[0,0,0,1,0,0,0,1,0,0,0,1]}},"geometricError":0.0}}}}"#
        ),
    )
    .unwrap();
    fs::write(dir.join("payload.bin"), marker.as_bytes()).unwrap();
}

#[test]
fn sibling_staging_not_inside_final() {
    let root = scratch("sibling");
    let final_out = root.join("outdir");
    fs::create_dir_all(&final_out).unwrap();
    let stage = prepare_staging(&final_out, "taskA").unwrap();
    assert_eq!(stage, staging_dir(&final_out, "taskA"));
    assert_eq!(stage.parent(), Some(root.as_path()));
    assert!(!stage.starts_with(&final_out));
}

#[test]
fn commit_fresh_atomic_rename() {
    let root = scratch("fresh");
    let final_out = root.join("outdir");
    let stage = prepare_staging(&final_out, "taskB").unwrap();
    let content = stage.join("staged");
    write_tree(&content, "v1");
    let em = Emitter::new("taskB");
    commit_transaction(&em, &content, &final_out, "taskB", None).unwrap();
    assert!(final_out.join("tileset.json").is_file());
    assert!(!backup_dir(&final_out, "taskB").exists());
}

#[test]
fn commit_replace_keeps_old_until_success() {
    let root = scratch("replace");
    let final_out = root.join("outdir");
    write_tree(&final_out, "old");
    let before = dir_fingerprint(&final_out).unwrap();

    let stage = prepare_staging(&final_out, "taskC").unwrap();
    let content = stage.join("staged");
    write_tree(&content, "new");

    // Before commit: final still old.
    assert_eq!(dir_fingerprint(&final_out).unwrap(), before);

    let em = Emitter::new("taskC");
    commit_transaction(&em, &content, &final_out, "taskC", None).unwrap();
    assert_eq!(
        fs::read_to_string(final_out.join("payload.bin")).unwrap(),
        "new"
    );
    assert!(!backup_dir(&final_out, "taskC").exists());
}

#[test]
fn second_rename_failure_rolls_back_final() {
    let root = scratch("rollback");
    let final_out = root.join("outdir");
    write_tree(&final_out, "precious");
    let before = dir_fingerprint(&final_out).unwrap();

    let stage = prepare_staging(&final_out, "taskD").unwrap();
    let content = stage.join("staged");
    write_tree(&content, "corrupt-candidate");

    let em = Emitter::new("taskD");
    let err = with_inject_fail_stage_to_final(|| {
        commit_transaction(&em, &content, &final_out, "taskD", None).unwrap_err()
    });

    assert!(
        err.contains("COMMIT_RENAME_FAILED"),
        "expected COMMIT_RENAME_FAILED, got {err}"
    );
    assert_eq!(dir_fingerprint(&final_out).unwrap(), before);
    assert_eq!(
        fs::read_to_string(final_out.join("payload.bin")).unwrap(),
        "precious"
    );
}

#[test]
fn kill_before_commit_leaves_final_uncorrupted() {
    // Scripted crash: work written to staging only; process "dies" before commit.
    let root = scratch("kill_pre_commit");
    let final_out = root.join("outdir");
    write_tree(&final_out, "live");
    let before = dir_fingerprint(&final_out).unwrap();

    let stage = prepare_staging(&final_out, "taskE").unwrap();
    write_tree(&stage.join("work"), "half-written");
    // Crash: no commit_transaction call.
    assert_eq!(dir_fingerprint(&final_out).unwrap(), before);
    assert!(stage.join("work/payload.bin").is_file());

    // Next start cleans staging; final unchanged.
    let _ = prepare_staging(&final_out, "taskE").unwrap();
    assert_eq!(dir_fingerprint(&final_out).unwrap(), before);
}

#[test]
fn cancel_exit_cleans_staging_marks_interrupted() {
    let root = scratch("cancel");
    let final_out = root.join("outdir");
    write_tree(&final_out, "stable");
    let before = dir_fingerprint(&final_out).unwrap();

    let stage = prepare_staging(&final_out, "taskF").unwrap();
    write_tree(&stage.join("work"), "partial");
    mark_interrupted(&final_out, "taskF");
    cleanup_staging(&stage, Some(&final_out));

    assert!(!stage.exists());
    assert!(interrupted_marker(&final_out, "taskF").is_file());
    assert_eq!(dir_fingerprint(&final_out).unwrap(), before);

    // CancelFlag path: cancelled task must not report success.
    let cancel = CancelFlag::new();
    cancel.request();
    assert!(cancel.is_cancelled());
    // Simulated outcome code used by pipeline.
    let code = if cancel.is_cancelled() {
        EXIT_CANCELLED
    } else {
        EXIT_OK
    };
    assert_eq!(code, EXIT_CANCELLED);
    assert_ne!(code, EXIT_OK);
}

#[test]
fn recover_backup_after_crash_mid_commit() {
    let root = scratch("recover");
    let final_out = root.join("outdir");
    let backup = backup_dir(&final_out, "taskG");
    // Simulate: final→backup done, stage→final never happened, process died.
    write_tree(&backup, "last-good");
    assert!(!final_out.exists());
    recover_interrupted_commit(&final_out, "taskG").unwrap();
    assert_eq!(
        fs::read_to_string(final_out.join("payload.bin")).unwrap(),
        "last-good"
    );
}

#[test]
fn refuse_empty_partial_stage() {
    let root = scratch("empty");
    let final_out = root.join("outdir");
    write_tree(&final_out, "keep");
    let before = dir_fingerprint(&final_out).unwrap();
    let stage = prepare_staging(&final_out, "taskH").unwrap();
    let content = stage.join("staged");
    fs::create_dir_all(&content).unwrap();
    let em = Emitter::new("taskH");
    let err = commit_transaction(&em, &content, &final_out, "taskH", None).unwrap_err();
    assert!(err.contains("COMMIT_REFUSED"));
    assert_eq!(dir_fingerprint(&final_out).unwrap(), before);
}

#[test]
fn refuse_source_in_place_overwrite() {
    let root = scratch("src");
    let source = root.join("tiles");
    write_tree(&source, "input");
    let stage = prepare_staging(&source, "taskI").unwrap();
    let content = stage.join("staged");
    write_tree(&content, "attacker");
    let em = Emitter::new("taskI");
    let err = commit_transaction(&em, &content, &source, "taskI", Some(&source)).unwrap_err();
    assert!(err.contains("COMMIT_REFUSED"), "{err}");
    assert_eq!(
        fs::read_to_string(source.join("payload.bin")).unwrap(),
        "input"
    );
}

#[test]
fn source_untouched_when_writing_elsewhere() {
    let root = scratch("src_safe");
    let source = root.join("input");
    let final_out = root.join("outdir");
    write_tree(&source, "src-data");
    let src_fp = dir_fingerprint(&source).unwrap();

    let stage = prepare_staging(&final_out, "taskJ").unwrap();
    let content = stage.join("staged");
    write_tree(&content, "out-data");
    let em = Emitter::new("taskJ");
    commit_transaction(&em, &content, &final_out, "taskJ", Some(&source)).unwrap();

    assert_eq!(dir_fingerprint(&source).unwrap(), src_fp);
    assert_eq!(
        fs::read_to_string(final_out.join("payload.bin")).unwrap(),
        "out-data"
    );
}

#[test]
fn task_config_smoke_paths_use_sibling_stage_name() {
    // Ensure TaskConfig output parent would host `.geoforge-stage-*` siblings.
    let root = scratch("cfg");
    let out = root.join("outdir");
    let cfg = TaskConfig {
        schema_version: Some(1),
        task_id: "smoke1".into(),
        operation: "process-tileset".into(),
        input: processor::protocol::PathRef {
            path: root.join("in").to_string_lossy().into(),
        },
        output: processor::protocol::PathRef {
            path: out.to_string_lossy().into(),
        },
        options: json!({}),
    };
    let stage = staging_dir(Path::new(cfg.output_path()), &cfg.task_id);
    assert!(stage
        .file_name()
        .unwrap()
        .to_string_lossy()
        .starts_with(".geoforge-stage-"));
}
