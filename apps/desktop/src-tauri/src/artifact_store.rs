//! Artifact registry — roots served by the local resource HTTP server.

use crate::db::now_secs;
use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::Deserialize;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Debug, Clone, Deserialize)]
pub struct ArtifactRecord {
  pub id: String,
  pub task_id: Option<String>,
  pub path: String,
  pub kind: String,
  pub label: String,
  pub created_at: f64,
  pub available: bool,
  pub has_tileset: bool,
}

impl ArtifactRecord {
  pub fn to_json_value(&self) -> serde_json::Value {
    serde_json::json!({
      "id": self.id,
      "taskId": self.task_id,
      "path": self.path,
      "kind": self.kind,
      "label": self.label,
      "createdAt": self.created_at,
      "created_at": self.created_at,
      "available": self.available,
      "has_tileset": self.has_tileset,
      "hasTileset": self.has_tileset,
    })
  }
}

fn make_artifact(
  id: String,
  task_id: Option<String>,
  path: String,
  kind: String,
  label: String,
  created_at: f64,
) -> ArtifactRecord {
  let root = PathBuf::from(&path);
  let available = root.exists();
  let has_tileset = root.join("tileset.json").is_file();
  ArtifactRecord {
    id,
    task_id,
    path,
    kind,
    label,
    created_at,
    available,
    has_tileset,
  }
}

#[derive(Clone)]
pub struct ArtifactStore {
  conn: Arc<Mutex<Connection>>,
  roots: Arc<parking_lot::RwLock<HashMap<String, PathBuf>>>,
}

impl ArtifactStore {
  pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
    let store = Self {
      conn,
      roots: Arc::new(parking_lot::RwLock::new(HashMap::new())),
    };
    let _ = store.reload_index();
    store
  }

  pub fn roots_handle(&self) -> Arc<parking_lot::RwLock<HashMap<String, PathBuf>>> {
    self.roots.clone()
  }

  pub fn reload_index(&self) -> Result<(), String> {
    let list = self.list_raw()?;
    let mut map = self.roots.write();
    map.clear();
    for a in list {
      if let Ok(canon) = PathBuf::from(&a.path).canonicalize() {
        map.insert(a.id, canon);
      } else {
        map.insert(a.id, PathBuf::from(a.path));
      }
    }
    Ok(())
  }

  pub fn register(
    &self,
    path: &str,
    task_id: Option<&str>,
    kind: &str,
    label: &str,
  ) -> Result<ArtifactRecord, String> {
    let root = PathBuf::from(path);
    let canon = root.canonicalize().unwrap_or(root.clone());
    let path_str = canon.to_string_lossy().into_owned();
    let id = format!("art-{}", &Uuid::new_v4().to_string().replace('-', "")[..12]);
    let now = now_secs();
    let label = if label.is_empty() {
      canon
        .file_name()
        .map(|s| s.to_string_lossy().into_owned())
        .unwrap_or_else(|| id.clone())
    } else {
      label.to_string()
    };
    let kind = if kind.is_empty() { "3dtiles" } else { kind };
    {
      let conn = self.conn.lock();
      conn
        .execute(
          r#"INSERT INTO artifacts (id, task_id, path, kind, label, created_at, available)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)"#,
          params![
            id,
            task_id,
            path_str,
            kind,
            label,
            now,
            if canon.exists() { 1 } else { 0 }
          ],
        )
        .map_err(|e| e.to_string())?;
    }
    self.roots.write().insert(id.clone(), canon);
    Ok(make_artifact(
      id,
      task_id.map(|s| s.to_string()),
      path_str,
      kind.to_string(),
      label,
      now,
    ))
  }

  pub fn get(&self, id: &str) -> Result<Option<ArtifactRecord>, String> {
    let conn = self.conn.lock();
    conn
      .query_row("SELECT * FROM artifacts WHERE id = ?1", params![id], |r| {
        Ok(make_artifact(
          r.get("id")?,
          r.get("task_id")?,
          r.get("path")?,
          r.get::<_, Option<String>>("kind")?.unwrap_or_default(),
          r.get::<_, Option<String>>("label")?.unwrap_or_default(),
          r.get::<_, Option<f64>>("created_at")?.unwrap_or(0.0),
        ))
      })
      .optional()
      .map_err(|e| e.to_string())
  }

  fn list_raw(&self) -> Result<Vec<ArtifactRecord>, String> {
    let conn = self.conn.lock();
    let mut stmt = conn
      .prepare("SELECT * FROM artifacts ORDER BY created_at DESC")
      .map_err(|e| e.to_string())?;
    let rows = stmt
      .query_map([], |r| {
        Ok(make_artifact(
          r.get("id")?,
          r.get("task_id")?,
          r.get("path")?,
          r.get::<_, Option<String>>("kind")?.unwrap_or_default(),
          r.get::<_, Option<String>>("label")?.unwrap_or_default(),
          r.get::<_, Option<f64>>("created_at")?.unwrap_or(0.0),
        ))
      })
      .map_err(|e| e.to_string())?;
    let mut out = Vec::new();
    for r in rows {
      out.push(r.map_err(|e| e.to_string())?);
    }
    Ok(out)
  }

  pub fn list(&self) -> Result<Vec<serde_json::Value>, String> {
    Ok(
      self
        .list_raw()?
        .into_iter()
        .map(|a| a.to_json_value())
        .collect(),
    )
  }

  pub fn resolve_file(root: &Path, rel: &str) -> Result<PathBuf, String> {
    let rel = rel.trim_start_matches('/');
    let rel = if rel.is_empty() { "tileset.json" } else { rel };
    if rel.split('/').any(|p| p == "..") {
      return Err("path traversal rejected".into());
    }
    let root = root
      .canonicalize()
      .map_err(|e| format!("artifact root missing: {e}"))?;
    let target = root.join(rel);
    let target = target
      .canonicalize()
      .map_err(|e| format!("file not found: {rel} ({e})"))?;
    if !target.starts_with(&root) {
      return Err("path traversal rejected".into());
    }
    if !target.is_file() {
      return Err(format!("file not found: {rel}"));
    }
    Ok(target)
  }
}
