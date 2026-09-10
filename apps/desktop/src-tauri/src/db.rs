//! Shared SQLite schema (tasks, artifacts, settings) under the app data directory.

use rusqlite::{Connection, Result as SqlResult};
use std::path::{Path, PathBuf};

pub fn default_data_dir() -> PathBuf {
  if let Ok(p) = std::env::var("GEOFORGE_DATA_DIR") {
    return PathBuf::from(p);
  }
  // Prefer repo-local path for smoke / CI, then home.
  let candidates = [
    PathBuf::from("/workspace/repos/3dtiles/.geoforge"),
    dirs_fallback(),
  ];
  for c in candidates {
    if std::fs::create_dir_all(&c).is_ok() {
      return c;
    }
  }
  let tmp = std::env::temp_dir().join("geoforge");
  let _ = std::fs::create_dir_all(&tmp);
  tmp
}

fn dirs_fallback() -> PathBuf {
  if let Ok(home) = std::env::var("HOME") {
    return PathBuf::from(home).join(".geoforge");
  }
  std::env::temp_dir().join("geoforge")
}

pub fn open_db(data_dir: &Path) -> SqlResult<Connection> {
  std::fs::create_dir_all(data_dir).ok();
  let db_path = data_dir.join("tasks.db");
  let conn = Connection::open(db_path)?;
  conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA journal_mode = WAL;")?;
  init_schema(&conn)?;
  Ok(conn)
}

pub fn init_schema(conn: &Connection) -> SqlResult<()> {
  conn.execute_batch(
    r#"
    CREATE TABLE IF NOT EXISTS tasks (
      id TEXT PRIMARY KEY,
      operation TEXT NOT NULL,
      status TEXT NOT NULL,
      task_name TEXT,
      input_path TEXT,
      output_path TEXT,
      options_json TEXT,
      stage TEXT,
      progress_json TEXT,
      log_text TEXT,
      log_path TEXT,
      error TEXT,
      created_at REAL,
      updated_at REAL,
      started_at REAL,
      finished_at REAL,
      pid INTEGER,
      cancel_requested INTEGER DEFAULT 0,
      python_task_id TEXT
    );

    CREATE TABLE IF NOT EXISTS artifacts (
      id TEXT PRIMARY KEY,
      task_id TEXT,
      path TEXT NOT NULL,
      kind TEXT,
      label TEXT,
      created_at REAL,
      available INTEGER DEFAULT 1
    );

    CREATE TABLE IF NOT EXISTS settings (
      key TEXT PRIMARY KEY,
      value_json TEXT NOT NULL
    );
    "#,
  )?;
  Ok(())
}

pub fn logs_dir(data_dir: &Path) -> PathBuf {
  let p = data_dir.join("logs");
  let _ = std::fs::create_dir_all(&p);
  p
}

pub fn now_secs() -> f64 {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .map(|d| d.as_secs_f64())
    .unwrap_or(0.0)
}
