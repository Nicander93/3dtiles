//! Spawn `processor` sidecar/child, parse JSONL events, update SQLite (Phase 3).

use crate::artifact_store::ArtifactStore;
use crate::db::now_secs;
use crate::task_store::TaskStore;
use parking_lot::Mutex;
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, Command, Stdio};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

/// Registry of running processor children for cancel.
#[derive(Clone, Default)]
pub struct ProcessManager {
  inner: Arc<Mutex<HashMap<String, ActiveProc>>>,
}

struct ActiveProc {
  child: Child,
  stdin: Option<ChildStdin>,
}

impl ProcessManager {
  pub fn new() -> Self {
    Self::default()
  }

  pub fn processor_available() -> bool {
    resolve_processor_bin().is_some()
  }

  pub fn spawn_task(
    &self,
    tasks: TaskStore,
    artifacts: ArtifactStore,
    task_id: String,
    data_dir: PathBuf,
  ) {
    let mgr = self.clone();
    thread::spawn(move || {
      if let Err(e) = run_processor_task(&mgr, &tasks, &artifacts, &task_id, &data_dir) {
        let _ = tasks.append_log(&task_id, &format!("[processor] error: {e}"));
        let _ = tasks.update_fields(&task_id, |t| {
          if t.status != "cancelled" && t.status != "cancelling" {
            t.status = "failed".into();
            t.stage = "failed".into();
            t.error = Some(e);
          } else {
            t.status = "cancelled".into();
            t.stage = "cancelled".into();
          }
          t.finished_at = Some(now_secs());
        });
      }
      mgr.inner.lock().remove(&task_id);
    });
  }

  pub fn cancel(&self, task_id: &str) -> bool {
    let mut map = self.inner.lock();
    if let Some(proc) = map.get_mut(task_id) {
      // Prefer stdin cancel (plan §7)
      if let Some(ref mut stdin) = proc.stdin {
        let _ = writeln!(stdin, "cancel");
        let _ = stdin.flush();
      }
      #[cfg(unix)]
      {
        let pid = proc.child.id() as i32;
        unsafe {
          extern "C" {
            fn kill(pid: i32, sig: i32) -> i32;
          }
          let _ = kill(pid, 15);
        }
      }
      let _ = proc.child.kill();
      true
    } else {
      false
    }
  }
}

fn resolve_processor_bin() -> Option<PathBuf> {
  if let Ok(p) = std::env::var("GEOFORGE_PROCESSOR") {
    let pb = PathBuf::from(&p);
    if pb.is_file() {
      return Some(pb);
    }
  }
  // Sidecar next to current exe / Tauri resource layouts (Phase 14)
  if let Ok(exe) = std::env::current_exe() {
    if let Some(dir) = exe.parent() {
      for name in ["processor", "processor.exe"] {
        for sub in [
          PathBuf::from(name),
          PathBuf::from("resources/bin").join(name),
          PathBuf::from("bin").join(name),
        ] {
          let cand = dir.join(&sub);
          if cand.is_file() {
            return Some(cand);
          }
        }
      }
      // Dev: apps/desktop/src-tauri/target/* → workspace target
      for rel in [
        "../../../target/debug/processor",
        "../../../target/release/processor",
        "../../binaries/processor",
      ] {
        let cand = dir.join(rel);
        if cand.is_file() {
          return Some(cand);
        }
      }
    }
  }
  // Walk up from CWD (developer checkout) — no hardcoded /workspace required
  if let Ok(cwd) = std::env::current_dir() {
    for anc in cwd.ancestors().take(8) {
      for sub in [
        "target/debug/processor",
        "target/release/processor",
        "apps/desktop/src-tauri/binaries/processor",
      ] {
        let cand = anc.join(sub);
        if cand.is_file() {
          return Some(cand);
        }
      }
    }
  }
  None
}

fn run_processor_task(
  mgr: &ProcessManager,
  tasks: &TaskStore,
  artifacts: &ArtifactStore,
  task_id: &str,
  data_dir: &Path,
) -> Result<(), String> {
  let task = tasks
    .get(task_id)?
    .ok_or_else(|| "task missing".to_string())?;

  let bin = resolve_processor_bin().ok_or_else(|| {
    "processor binary not found. Build with: cargo build -p processor \
(set GEOFORGE_PROCESSOR to override)."
      .to_string()
  })?;

  let _ = tasks.append_log(
    task_id,
    &format!(
      "[processor] Phase 3: spawning {} (no Python HTTP)",
      bin.display()
    ),
  );

  // Write TaskConfig JSON
  let tasks_dir = data_dir.join("processor-tasks");
  std::fs::create_dir_all(&tasks_dir).map_err(|e| e.to_string())?;
  let task_json_path = tasks_dir.join(format!("{task_id}.json"));
  let config = json!({
    "schemaVersion": 1,
    "taskId": task_id,
    "operation": task.operation,
    "input": { "path": task.input.path },
    "output": { "path": task.output.path },
    "options": task.options,
  });
  std::fs::write(
    &task_json_path,
    serde_json::to_string_pretty(&config).map_err(|e| e.to_string())?,
  )
  .map_err(|e| e.to_string())?;

  let mut child = Command::new(&bin)
    .arg("run")
    .arg("--task")
    .arg(&task_json_path)
    .stdin(Stdio::piped())
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .spawn()
    .map_err(|e| format!("spawn processor failed: {e}"))?;

  let pid = child.id() as i64;
  let stdin = child.stdin.take();
  let stdout = child.stdout.take().ok_or_else(|| "no stdout".to_string())?;
  let stderr = child.stderr.take();

  // Mirror stderr in background
  if let Some(err) = stderr {
    let tasks_c = tasks.clone();
    let tid = task_id.to_string();
    thread::spawn(move || {
      for line in BufReader::new(err).lines().flatten() {
        let _ = tasks_c.append_log(&tid, &format!("[processor:stderr] {line}"));
      }
    });
  }

  {
    let mut map = mgr.inner.lock();
    map.insert(
      task_id.to_string(),
      ActiveProc {
        child,
        stdin,
      },
    );
  }

  let _ = tasks.update_fields(task_id, |t| {
    t.status = "running".into();
    t.stage = "scan".into();
    t.started_at = Some(now_secs());
    t.pid = Some(pid);
    let mut prog = t.progress.as_object().cloned().unwrap_or_default();
    prog.insert("executor".into(), json!("processor"));
    t.progress = Value::Object(prog);
  });

  let mut result_path: Option<String> = None;
  let reader = BufReader::new(stdout);
  for line in reader.lines().flatten() {
    let line = line.trim().to_string();
    if line.is_empty() {
      continue;
    }
    // Honour cancel mid-stream
    if let Ok(Some(t)) = tasks.get(task_id) {
      if t.cancel_requested {
        mgr.cancel(task_id);
      }
    }
    match serde_json::from_str::<Value>(&line) {
      Ok(ev) => apply_event(tasks, task_id, &ev, &mut result_path),
      Err(_) => {
        let _ = tasks.append_log(task_id, &format!("[processor:raw] {line}"));
      }
    }
  }

  // Wait for exit
  let exit_code = {
    let mut map = mgr.inner.lock();
    if let Some(mut proc) = map.remove(task_id) {
      match proc.child.wait() {
        Ok(st) => st.code().unwrap_or(1),
        Err(_) => 1,
      }
    } else {
      // Already removed / wait via drop
      1
    }
  };

  // Small settle
  thread::sleep(Duration::from_millis(50));

  let cancel_requested = tasks
    .get(task_id)?
    .map(|t| t.cancel_requested)
    .unwrap_or(false);

  if cancel_requested || exit_code == 2 {
    let _ = tasks.update_fields(task_id, |t| {
      t.status = "cancelled".into();
      t.stage = "cancelled".into();
      t.finished_at = Some(now_secs());
    });
    return Ok(());
  }

  if exit_code != 0 {
    return Err(format!("processor exited {exit_code}"));
  }

  let out_path = result_path.unwrap_or_else(|| {
    tasks
      .get(task_id)
      .ok()
      .flatten()
      .map(|t| t.output.path)
      .unwrap_or_default()
  });

  let _ = tasks.update_fields(task_id, |t| {
    t.status = "succeeded".into();
    t.stage = "done".into();
    t.finished_at = Some(now_secs());
    let mut prog = t.progress.as_object().cloned().unwrap_or_default();
    prog.insert("path".into(), json!(out_path));
    t.progress = Value::Object(prog);
  });

  if !out_path.is_empty() {
    let root = PathBuf::from(&out_path);
    if root.join("tileset.json").is_file() {
      match artifacts.register(&out_path, Some(task_id), "3dtiles", "") {
        Ok(art) => {
          let _ = tasks.update_fields(task_id, |t| {
            let mut prog = t.progress.as_object().cloned().unwrap_or_default();
            prog.insert("artifactId".into(), json!(art.id));
            prog.insert("path".into(), json!(art.path));
            t.progress = Value::Object(prog);
          });
          let _ = tasks.append_log(
            task_id,
            &format!("[processor] registered artifact {} for Rust preview", art.id),
          );
        }
        Err(e) => {
          let _ = tasks.append_log(task_id, &format!("[processor] artifact register failed: {e}"));
        }
      }
    }
  }

  Ok(())
}

fn apply_event(
  tasks: &TaskStore,
  task_id: &str,
  ev: &Value,
  result_path: &mut Option<String>,
) {
  let ty = ev.get("type").and_then(|v| v.as_str()).unwrap_or("");
  match ty {
    "stage" => {
      let stage = ev
        .get("stage")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
      let message = ev
        .get("message")
        .and_then(|v| v.as_str())
        .unwrap_or("")
        .to_string();
      let _ = tasks.update_fields(task_id, |t| {
        if !stage.is_empty() {
          // Map rebuild-index/proxy → rebuild for UI stages
          t.stage = if stage.starts_with("rebuild") {
            "rebuild".into()
          } else if stage == "validate" {
            "check".into()
          } else {
            stage.clone()
          };
        }
        let mut prog = t.progress.as_object().cloned().unwrap_or_default();
        prog.insert("stage".into(), json!(stage));
        if !message.is_empty() {
          prog.insert("message".into(), json!(message));
        }
        // Forward geo if present
        if let Some(geo) = ev.get("geo") {
          prog.insert("geo".into(), geo.clone());
        }
        t.progress = Value::Object(prog);
      });
      if !message.is_empty() {
        let _ = tasks.append_log(task_id, &format!("[{stage}] {message}"));
      }
    }
    "progress" => {
      let _ = tasks.update_fields(task_id, |t| {
        let mut prog = t.progress.as_object().cloned().unwrap_or_default();
        if let Some(c) = ev.get("completed") {
          prog.insert("completed".into(), c.clone());
        }
        if let Some(tot) = ev.get("total") {
          prog.insert("total".into(), tot.clone());
        }
        if let Some(st) = ev.get("stage") {
          prog.insert("stage".into(), st.clone());
        }
        t.progress = Value::Object(prog);
      });
    }
    "log" => {
      if let Some(msg) = ev.get("message").and_then(|v| v.as_str()) {
        let _ = tasks.append_log(task_id, msg);
      }
    }
    "warning" => {
      let code = ev.get("code").and_then(|v| v.as_str()).unwrap_or("WARN");
      let msg = ev.get("message").and_then(|v| v.as_str()).unwrap_or("");
      let _ = tasks.append_log(task_id, &format!("[warning:{code}] {msg}"));
    }
    "error" => {
      let code = ev.get("code").and_then(|v| v.as_str()).unwrap_or("ERROR");
      let msg = ev.get("message").and_then(|v| v.as_str()).unwrap_or("");
      let _ = tasks.append_log(task_id, &format!("[error:{code}] {msg}"));
      let _ = tasks.update_fields(task_id, |t| {
        if code != "CANCELLED" {
          t.error = Some(msg.to_string());
        }
      });
    }
    "result" => {
      if let Some(p) = ev.get("path").and_then(|v| v.as_str()) {
        *result_path = Some(p.to_string());
        let _ = tasks.append_log(task_id, &format!("[result] {p}"));
      }
    }
    "metric" => {
      let name = ev.get("name").and_then(|v| v.as_str()).unwrap_or("metric");
      let val = ev.get("value").cloned().unwrap_or(Value::Null);
      let _ = tasks.append_log(task_id, &format!("[metric] {name}={val}"));
    }
    _ => {
      let _ = tasks.append_log(task_id, &format!("[event] {ev}"));
    }
  }
}
