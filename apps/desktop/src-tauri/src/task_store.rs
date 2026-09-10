//! Task persistence (status, stages, logs path, params snapshot).

use crate::db::{logs_dir, now_secs};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskRecord {
  pub id: String,
  pub operation: String,
  pub status: String,
  pub task_name: String,
  pub input: PathRef,
  pub output: PathRef,
  pub options: Value,
  pub stage: String,
  pub progress: Value,
  pub log: String,
  pub log_path: String,
  pub error: Option<String>,
  pub created_at: f64,
  pub updated_at: f64,
  pub started_at: Option<f64>,
  pub finished_at: Option<f64>,
  pub pid: Option<i64>,
  pub cancel_requested: bool,
  #[serde(skip_serializing_if = "Option::is_none")]
  pub python_task_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathRef {
  pub path: String,
}

#[derive(Clone)]
pub struct TaskStore {
  conn: Arc<Mutex<Connection>>,
  data_dir: PathBuf,
}

impl TaskStore {
  pub fn new(conn: Arc<Mutex<Connection>>, data_dir: PathBuf) -> Self {
    Self { conn, data_dir }
  }

  pub fn mark_stale_interrupted(&self) -> Result<(), String> {
    let conn = self.conn.lock();
    conn
      .execute(
        "UPDATE tasks SET status = 'interrupted', stage = COALESCE(NULLIF(stage,''), 'interrupted'),
         updated_at = ?1
         WHERE status IN ('running', 'cancelling', 'queued')",
        params![now_secs()],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  pub fn log_path_for(&self, task_id: &str) -> PathBuf {
    logs_dir(&self.data_dir).join(format!("{task_id}.log"))
  }

  pub fn create(
    &self,
    operation: &str,
    input_path: &str,
    output_path: &str,
    options: Value,
    task_name: &str,
  ) -> Result<TaskRecord, String> {
    let id = format!("task-{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);
    let now = now_secs();
    let name = if task_name.is_empty() {
      id.clone()
    } else {
      task_name.to_string()
    };
    let log_path = self.log_path_for(&id);
    let task = TaskRecord {
      id: id.clone(),
      operation: operation.to_string(),
      status: "queued".into(),
      task_name: name,
      input: PathRef {
        path: input_path.to_string(),
      },
      output: PathRef {
        path: output_path.to_string(),
      },
      options,
      stage: "queued".into(),
      progress: json!({}),
      log: String::new(),
      log_path: log_path.to_string_lossy().into_owned(),
      error: None,
      created_at: now,
      updated_at: now,
      started_at: None,
      finished_at: None,
      pid: None,
      cancel_requested: false,
      python_task_id: None,
    };
    self.upsert(&task)?;
    Ok(task)
  }

  pub fn upsert(&self, task: &TaskRecord) -> Result<(), String> {
    let conn = self.conn.lock();
    conn
      .execute(
        r#"
        INSERT INTO tasks (
          id, operation, status, task_name, input_path, output_path,
          options_json, stage, progress_json, log_text, log_path, error,
          created_at, updated_at, started_at, finished_at, pid, cancel_requested, python_task_id
        ) VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14,?15,?16,?17,?18,?19)
        ON CONFLICT(id) DO UPDATE SET
          operation=excluded.operation,
          status=excluded.status,
          task_name=excluded.task_name,
          input_path=excluded.input_path,
          output_path=excluded.output_path,
          options_json=excluded.options_json,
          stage=excluded.stage,
          progress_json=excluded.progress_json,
          log_text=excluded.log_text,
          log_path=excluded.log_path,
          error=excluded.error,
          created_at=excluded.created_at,
          updated_at=excluded.updated_at,
          started_at=excluded.started_at,
          finished_at=excluded.finished_at,
          pid=excluded.pid,
          cancel_requested=excluded.cancel_requested,
          python_task_id=excluded.python_task_id
        "#,
        params![
          task.id,
          task.operation,
          task.status,
          task.task_name,
          task.input.path,
          task.output.path,
          task.options.to_string(),
          task.stage,
          task.progress.to_string(),
          task.log,
          task.log_path,
          task.error,
          task.created_at,
          task.updated_at,
          task.started_at,
          task.finished_at,
          task.pid,
          if task.cancel_requested { 1 } else { 0 },
          task.python_task_id,
        ],
      )
      .map_err(|e| e.to_string())?;
    Ok(())
  }

  fn row_to_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<TaskRecord> {
    let options_json: String = row.get("options_json")?;
    let progress_json: String = row.get("progress_json")?;
    let options: Value = serde_json::from_str(&options_json).unwrap_or(json!({}));
    let progress: Value = serde_json::from_str(&progress_json).unwrap_or(json!({}));
    let cancel: i64 = row.get("cancel_requested")?;
    Ok(TaskRecord {
      id: row.get("id")?,
      operation: row.get("operation")?,
      status: row.get("status")?,
      task_name: row.get::<_, Option<String>>("task_name")?.unwrap_or_default(),
      input: PathRef {
        path: row.get::<_, Option<String>>("input_path")?.unwrap_or_default(),
      },
      output: PathRef {
        path: row.get::<_, Option<String>>("output_path")?.unwrap_or_default(),
      },
      options,
      stage: row.get::<_, Option<String>>("stage")?.unwrap_or_default(),
      progress,
      log: row.get::<_, Option<String>>("log_text")?.unwrap_or_default(),
      log_path: row.get::<_, Option<String>>("log_path")?.unwrap_or_default(),
      error: row.get("error")?,
      created_at: row.get::<_, Option<f64>>("created_at")?.unwrap_or(0.0),
      updated_at: row.get::<_, Option<f64>>("updated_at")?.unwrap_or(0.0),
      started_at: row.get("started_at")?,
      finished_at: row.get("finished_at")?,
      pid: row.get("pid")?,
      cancel_requested: cancel != 0,
      python_task_id: row.get("python_task_id")?,
    })
  }

  pub fn get(&self, id: &str) -> Result<Option<TaskRecord>, String> {
    let conn = self.conn.lock();
    conn
      .query_row("SELECT * FROM tasks WHERE id = ?1", params![id], |r| {
        Self::row_to_task(r)
      })
      .optional()
      .map_err(|e| e.to_string())
  }

  pub fn list(&self, status_filter: Option<&str>) -> Result<Vec<TaskRecord>, String> {
    let conn = self.conn.lock();
    let mut out = Vec::new();
    if let Some(st) = status_filter {
      let mut stmt = conn
        .prepare("SELECT * FROM tasks WHERE status = ?1 ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
      let rows = stmt
        .query_map(params![st], |r| Self::row_to_task(r))
        .map_err(|e| e.to_string())?;
      for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
      }
    } else {
      let mut stmt = conn
        .prepare("SELECT * FROM tasks ORDER BY created_at DESC")
        .map_err(|e| e.to_string())?;
      let rows = stmt
        .query_map([], |r| Self::row_to_task(r))
        .map_err(|e| e.to_string())?;
      for r in rows {
        out.push(r.map_err(|e| e.to_string())?);
      }
    }
    Ok(out)
  }

  pub fn update_fields<F>(&self, id: &str, mutator: F) -> Result<Option<TaskRecord>, String>
  where
    F: FnOnce(&mut TaskRecord),
  {
    let mut task = match self.get(id)? {
      Some(t) => t,
      None => return Ok(None),
    };
    mutator(&mut task);
    task.updated_at = now_secs();
    if task.log_path.is_empty() {
      task.log_path = self.log_path_for(id).to_string_lossy().into_owned();
    }
    self.upsert(&task)?;
    Ok(Some(task))
  }

  pub fn append_log(&self, id: &str, line: &str) -> Result<(), String> {
    let mut task = match self.get(id)? {
      Some(t) => t,
      None => return Ok(()),
    };
    let entry = if line.ends_with('\n') {
      line.to_string()
    } else {
      format!("{line}\n")
    };
    if !task.log.is_empty() && !task.log.ends_with('\n') {
      task.log.push('\n');
    }
    task.log.push_str(entry.trim_end_matches('\n'));
    if task.log.len() > 200_000 {
      task.log = task.log[task.log.len() - 200_000..].to_string();
    }
    let lp = self.log_path_for(id);
    if let Some(parent) = lp.parent() {
      let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(mut f) = std::fs::OpenOptions::new()
      .create(true)
      .append(true)
      .open(&lp)
    {
      use std::io::Write;
      let _ = f.write_all(entry.as_bytes());
    }
    task.log_path = lp.to_string_lossy().into_owned();
    task.updated_at = now_secs();
    self.upsert(&task)?;
    Ok(())
  }

  pub fn get_logs(&self, id: &str, tail: usize) -> Result<Option<Value>, String> {
    let task = match self.get(id)? {
      Some(t) => t,
      None => return Ok(None),
    };
    let mut text = task.log.clone();
    let lp = PathBuf::from(&task.log_path);
    if lp.is_file() {
      if let Ok(file_text) = std::fs::read_to_string(&lp) {
        text = file_text;
      }
    }
    let mut lines: Vec<&str> = text.lines().collect();
    if tail > 0 && lines.len() > tail {
      lines = lines[lines.len() - tail..].to_vec();
    }
    let joined = lines.join("\n");
    Ok(Some(json!({
      "taskId": id,
      "logPath": task.log_path,
      "lines": lines,
      "log": joined,
    })))
  }

  pub fn request_cancel(&self, id: &str) -> Result<Option<TaskRecord>, String> {
    self.update_fields(id, |task| {
      match task.status.as_str() {
        "queued" => {
          task.status = "cancelled".into();
          task.stage = "cancelled".into();
          task.cancel_requested = true;
          task.finished_at = Some(now_secs());
        }
        "running" => {
          task.status = "cancelling".into();
          task.cancel_requested = true;
        }
        _ => {}
      }
    })
  }
}
