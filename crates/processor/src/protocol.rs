//! TaskConfig schema + stdout JSONL event protocol (plan §7).

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

pub const SCHEMA_VERSION: u32 = 1;

/// Exit codes: 0 success, 1 failed, 2 cancelled.
pub const EXIT_OK: i32 = 0;
pub const EXIT_FAILED: i32 = 1;
pub const EXIT_CANCELLED: i32 = 2;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskConfig {
    pub schema_version: Option<u32>,
    pub task_id: String,
    pub operation: String,
    pub input: PathRef,
    pub output: PathRef,
    #[serde(default)]
    pub options: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathRef {
    pub path: String,
}

impl TaskConfig {
    pub fn input_path(&self) -> &str {
        &self.input.path
    }
    pub fn output_path(&self) -> &str {
        &self.output.path
    }
    pub fn options_obj(&self) -> &Value {
        &self.options
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Stage {
    Scan,
    Convert,
    RebuildIndex,
    RebuildProxy,
    Rebuild,
    Texture,
    Validate,
    Commit,
    Done,
}

impl Stage {
    pub fn as_str(self) -> &'static str {
        match self {
            Stage::Scan => "scan",
            Stage::Convert => "convert",
            Stage::RebuildIndex => "rebuild-index",
            Stage::RebuildProxy => "rebuild-proxy",
            Stage::Rebuild => "rebuild",
            Stage::Texture => "texture",
            Stage::Validate => "validate",
            Stage::Commit => "commit",
            Stage::Done => "done",
        }
    }
}

/// Thread-safe JSONL event emitter (stdout only).
pub struct Emitter {
    task_id: String,
    seq: AtomicU64,
    out: Mutex<io::Stdout>,
}

impl Emitter {
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            seq: AtomicU64::new(0),
            out: Mutex::new(io::stdout()),
        }
    }

    fn next_seq(&self) -> u64 {
        self.seq.fetch_add(1, Ordering::SeqCst) + 1
    }

    fn emit(&self, mut event: Value) {
        let seq = self.next_seq();
        if let Some(obj) = event.as_object_mut() {
            obj.insert("schemaVersion".into(), json!(SCHEMA_VERSION));
            obj.insert("taskId".into(), json!(self.task_id));
            obj.insert("seq".into(), json!(seq));
        }
        if let Ok(mut guard) = self.out.lock() {
            let _ = writeln!(guard, "{}", event);
            let _ = guard.flush();
        }
    }

    pub fn stage(&self, stage: Stage, message: &str) {
        self.emit(json!({
            "type": "stage",
            "stage": stage.as_str(),
            "message": message,
        }));
    }

    pub fn stage_extra(&self, stage: Stage, message: &str, extra: Value) {
        let mut ev = json!({
            "type": "stage",
            "stage": stage.as_str(),
            "message": message,
        });
        if let (Some(dst), Some(src)) = (ev.as_object_mut(), extra.as_object()) {
            for (k, v) in src {
                dst.insert(k.clone(), v.clone());
            }
        }
        self.emit(ev);
    }

    pub fn progress(&self, stage: Stage, completed: u64, total: u64) {
        self.emit(json!({
            "type": "progress",
            "stage": stage.as_str(),
            "completed": completed,
            "total": total,
        }));
    }

    pub fn log(&self, message: &str) {
        self.emit(json!({
            "type": "log",
            "message": message,
        }));
    }

    pub fn warning(&self, code: &str, message: &str) {
        self.emit(json!({
            "type": "warning",
            "code": code,
            "message": message,
        }));
    }

    pub fn metric(&self, name: &str, value: Value) {
        self.emit(json!({
            "type": "metric",
            "name": name,
            "value": value,
        }));
    }

    pub fn error(&self, code: &str, message: &str) {
        self.emit(json!({
            "type": "error",
            "code": code,
            "message": message,
        }));
    }

    pub fn result(&self, path: &str) {
        self.emit(json!({
            "type": "result",
            "path": path,
        }));
    }
}
