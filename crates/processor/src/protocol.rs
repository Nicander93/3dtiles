//! Processor runtime protocol: re-exports geoforge-protocol + stdout JSONL Emitter.

pub use geoforge_protocol::{
    PathRef, Stage, TaskConfig, EXIT_CANCELLED, EXIT_FAILED, EXIT_OK, SCHEMA_VERSION,
};

use serde_json::{json, Value};
use std::io::{self, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;

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
