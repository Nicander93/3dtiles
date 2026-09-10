//! App settings persisted in the same SQLite DB.

use parking_lot::Mutex;
use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::sync::Arc;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AppSettings {
  pub default_output_root: String,
  pub default_rebuild_top: bool,
  pub default_rebuild_levels: i32,
  pub default_texture_compress: bool,
  /// Optional override for Python desktop_server base URL (Phase 2 bridge).
  pub python_server_url: String,
  /// Resource server prefer fixed port (0 = ephemeral).
  pub resource_server_port: u16,
}

impl Default for AppSettings {
  fn default() -> Self {
    Self {
      default_output_root: String::new(),
      default_rebuild_top: true,
      default_rebuild_levels: 1,
      default_texture_compress: true,
      python_server_url: "http://127.0.0.1:8787".into(),
      resource_server_port: 0,
    }
  }
}

#[derive(Clone)]
pub struct SettingsStore {
  conn: Arc<Mutex<Connection>>,
}

impl SettingsStore {
  pub fn new(conn: Arc<Mutex<Connection>>) -> Self {
    Self { conn }
  }

  pub fn get(&self) -> Result<AppSettings, String> {
    let conn = self.conn.lock();
    let raw: Option<String> = conn
      .query_row(
        "SELECT value_json FROM settings WHERE key = 'app'",
        [],
        |r| r.get(0),
      )
      .optional()
      .map_err(|e| e.to_string())?;
    match raw {
      Some(s) => serde_json::from_str(&s).map_err(|e| e.to_string()),
      None => Ok(AppSettings::default()),
    }
  }

  pub fn update(&self, settings: AppSettings) -> Result<AppSettings, String> {
    let conn = self.conn.lock();
    let value = serde_json::to_string(&settings).map_err(|e| e.to_string())?;
    conn
      .execute(
        r#"INSERT INTO settings (key, value_json) VALUES ('app', ?1)
           ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json"#,
        params![value],
      )
      .map_err(|e| e.to_string())?;
    // Keep a JSON blob for debugging / future keys
    let _ = conn.execute(
      r#"INSERT INTO settings (key, value_json) VALUES ('app_meta', ?1)
         ON CONFLICT(key) DO UPDATE SET value_json = excluded.value_json"#,
      params![json!({ "updated": true }).to_string()],
    );
    Ok(settings)
  }
}
