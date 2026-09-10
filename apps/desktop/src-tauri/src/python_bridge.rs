//! Phase 2/3 fallback bridge: used only when `processor` binary is unavailable.
//! Happy path (Phase 3) is `process_manager` → local processor JSONL.
//! Task/artifact persistence and Cesium preview remain Rust-owned.

use crate::artifact_store::ArtifactStore;
use crate::db::now_secs;
use crate::task_store::TaskStore;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::thread;
use std::time::Duration;

pub fn python_base_default() -> String {
  std::env::var("GEOFORGE_PYTHON_URL").unwrap_or_else(|_| "http://127.0.0.1:8787".into())
}

/// Create task on Python server and poll until terminal; sync into Rust SQLite.
/// On success, register artifact under output path.
pub fn spawn_python_task_follower(
  tasks: TaskStore,
  artifacts: ArtifactStore,
  rust_task_id: String,
  python_base: String,
) {
  thread::spawn(move || {
    if let Err(e) = run_follower(&tasks, &artifacts, &rust_task_id, &python_base) {
      let _ = tasks.append_log(&rust_task_id, &format!("[bridge] error: {e}"));
      let _ = tasks.update_fields(&rust_task_id, |t| {
        t.status = "failed".into();
        t.stage = "failed".into();
        t.error = Some(e);
        t.finished_at = Some(now_secs());
      });
    }
  });
}

fn run_follower(
  tasks: &TaskStore,
  artifacts: &ArtifactStore,
  rust_task_id: &str,
  python_base: &str,
) -> Result<(), String> {
  let task = tasks
    .get(rust_task_id)?
    .ok_or_else(|| "task missing".to_string())?;

  let _ = tasks.append_log(
    rust_task_id,
    &format!(
      "[bridge] Phase 2: execution still via Python desktop_server at {python_base}"
    ),
  );

  let client = reqwest::blocking::Client::builder()
    .timeout(Duration::from_secs(30))
    .build()
    .map_err(|e| e.to_string())?;

  // Health check
  match client
    .get(format!("{}/api/health", python_base.trim_end_matches('/')))
    .send()
  {
    Ok(r) if r.status().is_success() => {}
    Ok(r) => {
      return Err(format!(
        "Python desktop_server unhealthy: HTTP {}",
        r.status()
      ));
    }
    Err(e) => {
      return Err(format!(
        "Python desktop_server unreachable at {python_base}: {e}. Start with scripts/run_geoforge.sh"
      ));
    }
  }

  let payload = json!({
    "operation": task.operation,
    "input": { "path": task.input.path },
    "output": { "path": task.output.path },
    "options": task.options,
    "taskName": task.task_name,
  });

  let create_url = format!("{}/api/tasks", python_base.trim_end_matches('/'));
  let res = client
    .post(&create_url)
    .json(&payload)
    .send()
    .map_err(|e| format!("submit to Python failed: {e}"))?;
  if !res.status().is_success() {
    let status = res.status();
    let body = res.text().unwrap_or_default();
    return Err(format!("Python create task HTTP {status}: {body}"));
  }
  let body: Value = res.json().map_err(|e| e.to_string())?;
  let py_task = body
    .get("task")
    .cloned()
    .or_else(|| body.get("id").map(|id| json!({ "id": id })))
    .ok_or_else(|| "Python response missing task".to_string())?;
  let py_id = py_task
    .get("id")
    .and_then(|v| v.as_str())
    .ok_or_else(|| "Python task id missing".to_string())?
    .to_string();

  let _ = tasks.update_fields(rust_task_id, |t| {
    t.status = "running".into();
    t.stage = "scan".into();
    t.started_at = Some(now_secs());
    t.python_task_id = Some(py_id.clone());
    let mut prog = t.progress.as_object().cloned().unwrap_or_default();
    prog.insert("pythonTaskId".into(), json!(py_id));
    t.progress = Value::Object(prog);
  });
  let _ = tasks.append_log(
    rust_task_id,
    &format!("[bridge] linked Python task {py_id}"),
  );

  // Poll
  loop {
    // Honour local cancel
    if let Ok(Some(t)) = tasks.get(rust_task_id) {
      if t.cancel_requested {
        let cancel_url = format!(
          "{}/api/tasks/{}/cancel",
          python_base.trim_end_matches('/'),
          py_id
        );
        let _ = client.post(&cancel_url).send();
      }
    }

    let get_url = format!(
      "{}/api/tasks/{}",
      python_base.trim_end_matches('/'),
      py_id
    );
    let res = client
      .get(&get_url)
      .send()
      .map_err(|e| format!("poll failed: {e}"))?;
    if !res.status().is_success() {
      thread::sleep(Duration::from_millis(800));
      continue;
    }
    let body: Value = res.json().map_err(|e| e.to_string())?;
    let py = body.get("task").cloned().unwrap_or(body);
    let status = py
      .get("status")
      .and_then(|v| v.as_str())
      .unwrap_or("running")
      .to_string();
    let stage = py
      .get("stage")
      .and_then(|v| v.as_str())
      .unwrap_or("")
      .to_string();
    let error = py
      .get("error")
      .and_then(|v| v.as_str())
      .map(|s| s.to_string());
    let log = py
      .get("log")
      .and_then(|v| v.as_str())
      .unwrap_or("")
      .to_string();
    let progress = py.get("progress").cloned().unwrap_or(json!({}));

    let _ = tasks.update_fields(rust_task_id, |t| {
      t.status = status.clone();
      if !stage.is_empty() {
        t.stage = stage.clone();
      }
      t.progress = progress.clone();
      if let Some(ref e) = error {
        t.error = Some(e.clone());
      }
      if !log.is_empty() {
        t.log = log.clone();
      }
      if matches!(
        status.as_str(),
        "succeeded" | "completed" | "failed" | "cancelled" | "interrupted"
      ) {
        t.finished_at = Some(now_secs());
      }
    });

    if matches!(
      status.as_str(),
      "succeeded" | "completed" | "failed" | "cancelled" | "interrupted"
    ) {
      if matches!(status.as_str(), "succeeded" | "completed") {
        let out = tasks
          .get(rust_task_id)?
          .map(|t| t.output.path)
          .unwrap_or_default();
        if !out.is_empty() {
          let out_path = PathBuf::from(&out);
          let tileset = if out_path.join("tileset.json").is_file() {
            Some(out_path)
          } else if out_path.is_file()
            && out_path
              .file_name()
              .map(|n| n == "tileset.json")
              .unwrap_or(false)
          {
            out_path.parent().map(|p| p.to_path_buf())
          } else {
            None
          };
          if let Some(root) = tileset {
            match artifacts.register(
              &root.to_string_lossy(),
              Some(rust_task_id),
              "3dtiles",
              "",
            ) {
              Ok(art) => {
                let _ = tasks.update_fields(rust_task_id, |t| {
                  let mut prog = t.progress.as_object().cloned().unwrap_or_default();
                  prog.insert("artifactId".into(), json!(art.id));
                  prog.insert("path".into(), json!(art.path));
                  t.progress = Value::Object(prog);
                });
                let _ = tasks.append_log(
                  rust_task_id,
                  &format!("[bridge] registered artifact {} → Rust resource server", art.id),
                );
              }
              Err(e) => {
                let _ = tasks.append_log(
                  rust_task_id,
                  &format!("[bridge] artifact register failed: {e}"),
                );
              }
            }
          }
        }
      }
      break;
    }
    thread::sleep(Duration::from_millis(900));
  }
  Ok(())
}

pub fn cancel_python_task(python_base: &str, python_task_id: &str) -> Result<(), String> {
  let client = reqwest::blocking::Client::builder()
    .timeout(Duration::from_secs(10))
    .build()
    .map_err(|e| e.to_string())?;
  let url = format!(
    "{}/api/tasks/{}/cancel",
    python_base.trim_end_matches('/'),
    python_task_id
  );
  let res = client
    .post(&url)
    .send()
    .map_err(|e| format!("cancel proxy failed: {e}"))?;
  if res.status().is_success() || res.status().as_u16() == 404 {
    Ok(())
  } else {
    Err(format!("cancel HTTP {}", res.status()))
  }
}
