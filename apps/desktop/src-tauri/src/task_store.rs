//! Task persistence (status, stages, logs path, params snapshot).

use crate::db::{logs_dir, now_secs};
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::io::{Read, Seek, SeekFrom};
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
  write_lock: Arc<Mutex<()>>,
}

impl TaskStore {
  pub fn new(conn: Arc<Mutex<Connection>>, data_dir: PathBuf) -> Self {
    Self {
      conn,
      data_dir,
      write_lock: Arc::new(Mutex::new(())),
    }
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
    let _write_guard = self.write_lock.lock();
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
    let _write_guard = self.write_lock.lock();
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
      task.log = truncate_utf8_tail(&task.log, 200_000);
    }
    let lp = self.log_path_for(id);
    let file_result = (|| -> Result<(), String> {
      if let Some(parent) = lp.parent() {
        std::fs::create_dir_all(parent).map_err(|error| {
          format!("create task log directory {} failed: {error}", parent.display())
        })?;
      }
      let mut file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&lp)
        .map_err(|error| format!("open task log {} failed: {error}", lp.display()))?;
      use std::io::Write;
      file
        .write_all(entry.as_bytes())
        .map_err(|error| format!("write task log {} failed: {error}", lp.display()))
    })();
    task.log_path = lp.to_string_lossy().into_owned();
    task.updated_at = now_secs();
    self.upsert(&task)?;
    if let Err(error) = file_result {
      return Err(error);
    }
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
      if let Ok(file_text) = read_log_tail(&lp, 256 * 1024) {
        text = file_text;
      }
    }
    let mut lines: Vec<&str> = text.lines().collect();
    let max_lines = if tail == 0 { 500 } else { tail.min(5_000) };
    if lines.len() > max_lines {
      lines = lines[lines.len() - max_lines..].to_vec();
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

  /// Oldest queued task that is not cancel_requested.
  pub fn next_queued(&self) -> Result<Option<TaskRecord>, String> {
    let conn = self.conn.lock();
    conn
      .query_row(
        "SELECT * FROM tasks WHERE status = 'queued' AND cancel_requested = 0
         ORDER BY created_at ASC LIMIT 1",
        [],
        |r| Self::row_to_task(r),
      )
      .optional()
      .map_err(|e| e.to_string())
  }
}

fn read_log_tail(path: &PathBuf, max_bytes: u64) -> Result<String, String> {
  let mut file = std::fs::File::open(path).map_err(|e| e.to_string())?;
  let length = file.metadata().map_err(|e| e.to_string())?.len();
  let start = length.saturating_sub(max_bytes);
  file.seek(SeekFrom::Start(start)).map_err(|e| e.to_string())?;
  let mut bytes = Vec::new();
  file.read_to_end(&mut bytes).map_err(|e| e.to_string())?;
  // The byte window may begin in the middle of a multi-byte log character.
  // Drop only continuation bytes so the returned tail starts at a UTF-8
  // boundary while keeping the bounded read.
  let prefix = bytes
    .iter()
    .position(|byte| (byte & 0b1100_0000) != 0b1000_0000)
    .unwrap_or(bytes.len());
  if prefix > 0 {
    bytes.drain(..prefix);
  }
  Ok(String::from_utf8_lossy(&bytes).into_owned())
}

fn truncate_utf8_tail(value: &str, max_bytes: usize) -> String {
  if value.len() <= max_bytes {
    return value.to_string();
  }

  let mut start = value.len() - max_bytes;
  while start < value.len() && !value.is_char_boundary(start) {
    start += 1;
  }
  value[start..].to_string()
}

#[cfg(test)]
mod tests {
  use super::{read_log_tail, truncate_utf8_tail};
  use crate::db::init_schema;
  use parking_lot::Mutex;
  use rusqlite::Connection;
  use std::fs;
  use std::sync::{Arc, Barrier};
  use std::thread;
  use std::time::{SystemTime, UNIX_EPOCH};

  #[test]
  fn tail_truncation_keeps_valid_utf8() {
    let value = "日志🙂".repeat(100);
    let tail = truncate_utf8_tail(&value, 17);

    assert!(tail.len() <= 17);
    assert!(std::str::from_utf8(tail.as_bytes()).is_ok());
    assert!(value.ends_with(&tail));
  }

  #[test]
  fn short_values_are_unchanged() {
    assert_eq!(truncate_utf8_tail("abc", 10), "abc");
  }

  #[test]
  fn append_log_reports_file_write_failure_without_recursive_logging() {
    let conn = Connection::open_in_memory().expect("open sqlite");
    init_schema(&conn).expect("create schema");
    let stamp = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system clock before unix epoch")
      .as_nanos();
    let data_file = std::env::temp_dir().join(format!("geoforge-task-log-file-{stamp}"));
    fs::write(&data_file, b"data").expect("create blocking data file");
    let store = super::TaskStore::new(Arc::new(Mutex::new(conn)), data_file.clone());
    let task = store
      .create("convert-osgb", "input", "output", serde_json::json!({}), "test")
      .expect("create task");

    let error = store
      .append_log(&task.id, "cannot persist")
      .expect_err("file failure must be reported");
    assert!(error.contains("task log"), "{error}");
    let saved = store.get(&task.id).expect("read task").expect("task exists");
    assert!(saved.log.contains("cannot persist"));
    let _ = fs::remove_file(data_file);
  }

  #[test]
  fn log_file_tail_starts_at_utf8_boundary() {
    let stamp = SystemTime::now()
      .duration_since(UNIX_EPOCH)
      .expect("system clock before unix epoch")
      .as_nanos();
    let path = std::env::temp_dir().join(format!("geoforge-log-tail-{stamp}.log"));
    fs::write(&path, "prefix-中文-🚀-tail").expect("write log fixture");

    let tail = read_log_tail(&path, 7).expect("read log tail");
    assert!(std::str::from_utf8(tail.as_bytes()).is_ok());
    assert!(!tail.starts_with('\u{fffd}'));

    let _ = fs::remove_file(path);
  }

  #[test]
  fn concurrent_log_cancel_and_error_updates_keep_all_fields() {
    let conn = Connection::open_in_memory().expect("open sqlite");
    init_schema(&conn).expect("create schema");
    let data_dir = std::env::temp_dir().join(format!(
      "geoforge-task-store-{}",
      SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system clock before unix epoch")
        .as_nanos()
    ));
    fs::create_dir_all(&data_dir).expect("create task data dir");
    let store = super::TaskStore::new(Arc::new(Mutex::new(conn)), data_dir.clone());
    let task = store
      .create("convert-osgb", "input", "output", serde_json::json!({}), "test")
      .expect("create task");

    let barrier = Arc::new(Barrier::new(3));
    let cancel_done = Arc::new(Barrier::new(2));
    let log_store = store.clone();
    let log_task_id = task.id.clone();
    let log_barrier = barrier.clone();
    let logs = thread::spawn(move || {
      log_barrier.wait();
      for index in 0..100 {
        log_store
          .append_log(&log_task_id, &format!("log-{index}"))
          .expect("append log");
      }
    });

    let cancel_store = store.clone();
    let cancel_task_id = task.id.clone();
    let cancel_barrier = barrier.clone();
    let cancel_done_for_thread = cancel_done.clone();
    let cancel = thread::spawn(move || {
      cancel_barrier.wait();
      cancel_store
        .request_cancel(&cancel_task_id)
        .expect("request cancel");
      cancel_done_for_thread.wait();
    });

    barrier.wait();
    cancel_done.wait();
    store
      .update_fields(&task.id, |record| {
        record.error = Some("controlled failure".into());
        record.stage = "convert".into();
      })
      .expect("update error");

    logs.join().expect("log thread");
    cancel.join().expect("cancel thread");
    let final_task = store
      .get(&task.id)
      .expect("read task")
      .expect("task exists");
    assert!(final_task.cancel_requested);
    assert_eq!(final_task.error.as_deref(), Some("controlled failure"));
    assert_eq!(final_task.stage, "convert");
    assert!(final_task.log.contains("log-99"));

    let _ = fs::remove_dir_all(data_dir);
  }
}
