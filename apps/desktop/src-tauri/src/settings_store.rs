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
  #[serde(default = "default_texture_compress")]
  pub default_texture_compress: bool,
  #[serde(default = "default_convert_threads")]
  pub default_convert_threads: i32,
  /// Optional override for Python desktop_server base URL (Phase 2 bridge).
  pub python_server_url: String,
  /// Resource server prefer fixed port (0 = ephemeral).
  pub resource_server_port: u16,
  /// Records missing this field were saved before version 2.
  #[serde(default)]
  pub settings_version: u32,
}

const SETTINGS_VERSION: u32 = 2;

fn default_texture_compress() -> bool {
  false
}

/// 0 lets the processor use half of the CPU cores (at most 8).
fn default_convert_threads() -> i32 {
  0
}

impl Default for AppSettings {
  fn default() -> Self {
    Self {
      default_output_root: String::new(),
      default_rebuild_top: true,
      default_rebuild_levels: 1,
      default_texture_compress: false,
      default_convert_threads: default_convert_threads(),
      python_server_url: "http://127.0.0.1:8787".into(),
      resource_server_port: 0,
      settings_version: SETTINGS_VERSION,
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
      Some(s) => {
        let settings: AppSettings = serde_json::from_str(&s).map_err(|e| e.to_string())?;
        Ok(migrate_settings(settings))
      }
      None => Ok(AppSettings::default()),
    }
  }

  pub fn update(&self, mut settings: AppSettings) -> Result<AppSettings, String> {
    // Values saved from the current UI are explicit choices, including 1.
    settings.settings_version = SETTINGS_VERSION;
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

/// Before version 2 the UI defaulted to one convert worker, so a stored 1 cannot be told
/// apart from an untouched default. Move it to the new automatic default.
fn migrate_settings(mut settings: AppSettings) -> AppSettings {
  if settings.settings_version < SETTINGS_VERSION && settings.default_convert_threads == 1 {
    settings.default_convert_threads = default_convert_threads();
  }
  settings.settings_version = SETTINGS_VERSION;
  settings
}

#[cfg(test)]
mod tests {
  use super::*;

  #[test]
  fn default_settings_are_keep_and_auto_workers() {
    let defaults = AppSettings::default();
    assert_eq!(defaults.default_texture_compress, false);
    assert_eq!(defaults.default_convert_threads, 0);
  }

  #[test]
  fn serialize_and_deserialize_round_trip() {
    let settings = AppSettings {
      default_output_root: "D:\\output".into(),
      default_rebuild_top: true,
      default_rebuild_levels: 2,
      default_texture_compress: false,
      default_convert_threads: 4,
      python_server_url: "http://localhost:8787".into(),
      resource_server_port: 8080,
      settings_version: SETTINGS_VERSION,
    };
    let json = serde_json::to_string(&settings).unwrap();
    let parsed: AppSettings = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.default_output_root, "D:\\output");
    assert_eq!(parsed.default_rebuild_top, true);
    assert_eq!(parsed.default_rebuild_levels, 2);
    assert_eq!(parsed.default_texture_compress, false);
    assert_eq!(parsed.default_convert_threads, 4);
    assert_eq!(parsed.python_server_url, "http://localhost:8787");
    assert_eq!(parsed.resource_server_port, 8080);
  }

  #[test]
  fn old_json_without_convert_threads_uses_default() {
    let old_json = r#"{
      "defaultOutputRoot": "D:\\old",
      "defaultRebuildTop": true,
      "defaultRebuildLevels": 1,
      "defaultTextureCompress": true,
      "pythonServerUrl": "http://127.0.0.1:8787",
      "resourceServerPort": 0
    }"#;
    let parsed: AppSettings = serde_json::from_str(old_json).unwrap();
    assert_eq!(parsed.default_output_root, "D:\\old");
    assert_eq!(parsed.default_texture_compress, true);
    assert_eq!(parsed.default_convert_threads, 0);
  }

  #[test]
  fn old_json_without_both_new_fields_uses_defaults() {
    let old_json = r#"{
      "defaultOutputRoot": "",
      "defaultRebuildTop": true,
      "defaultRebuildLevels": 1,
      "pythonServerUrl": "http://127.0.0.1:8787",
      "resourceServerPort": 0
    }"#;
    let parsed: AppSettings = serde_json::from_str(old_json).unwrap();
    assert_eq!(parsed.default_texture_compress, false);
    assert_eq!(parsed.default_convert_threads, 0);
  }

  #[test]
  fn explicit_false_texture_compress_is_preserved() {
    let json = r#"{
      "defaultOutputRoot": "",
      "defaultRebuildTop": true,
      "defaultRebuildLevels": 1,
      "defaultTextureCompress": false,
      "defaultConvertThreads": 2,
      "pythonServerUrl": "http://127.0.0.1:8787",
      "resourceServerPort": 0
    }"#;
    let parsed: AppSettings = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.default_texture_compress, false);
    assert_eq!(parsed.default_convert_threads, 2);
  }

  #[test]
  fn explicit_true_texture_compress_is_preserved() {
    let json = r#"{
      "defaultOutputRoot": "",
      "defaultRebuildTop": true,
      "defaultRebuildLevels": 1,
      "defaultTextureCompress": true,
      "defaultConvertThreads": 4,
      "pythonServerUrl": "http://127.0.0.1:8787",
      "resourceServerPort": 0
    }"#;
    let parsed: AppSettings = serde_json::from_str(json).unwrap();
    assert_eq!(parsed.default_texture_compress, true);
    assert_eq!(parsed.default_convert_threads, 4);
  }

  #[test]
  fn sqlite_round_trip() {
    use rusqlite::Connection;
    use std::sync::Arc;
    
    let conn = Connection::open_in_memory().unwrap();
    conn.execute(
      "CREATE TABLE settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL)",
      [],
    ).unwrap();
    
    let store = SettingsStore::new(Arc::new(Mutex::new(conn)));
    
    let settings = AppSettings {
      default_output_root: "D:\\test".into(),
      default_rebuild_top: false,
      default_rebuild_levels: 2,
      default_texture_compress: false,
      default_convert_threads: 2,
      python_server_url: "http://test:9999".into(),
      resource_server_port: 9999,
      settings_version: SETTINGS_VERSION,
    };
    
    store.update(settings.clone()).unwrap();
    let loaded = store.get().unwrap();
    
    assert_eq!(loaded.default_output_root, settings.default_output_root);
    assert_eq!(loaded.default_texture_compress, settings.default_texture_compress);
    assert_eq!(loaded.default_convert_threads, settings.default_convert_threads);
  }

  fn store_with(raw: Option<&str>) -> SettingsStore {
    let conn = Connection::open_in_memory().unwrap();
    conn
      .execute(
        "CREATE TABLE settings (key TEXT PRIMARY KEY, value_json TEXT NOT NULL)",
        [],
      )
      .unwrap();
    if let Some(raw) = raw {
      conn
        .execute(
          "INSERT INTO settings (key, value_json) VALUES ('app', ?1)",
          params![raw],
        )
        .unwrap();
    }
    SettingsStore::new(Arc::new(Mutex::new(conn)))
  }

  #[test]
  fn old_saved_single_worker_migrates_to_auto() {
    let store = store_with(Some(
      r#"{"defaultOutputRoot":"","defaultRebuildTop":true,"defaultRebuildLevels":1,
          "defaultConvertThreads":1,"pythonServerUrl":"","resourceServerPort":0}"#,
    ));
    let loaded = store.get().unwrap();
    assert_eq!(loaded.default_convert_threads, 0);
    assert_eq!(loaded.settings_version, SETTINGS_VERSION);
  }

  #[test]
  fn old_saved_non_default_workers_are_kept() {
    let store = store_with(Some(
      r#"{"defaultOutputRoot":"","defaultRebuildTop":true,"defaultRebuildLevels":1,
          "defaultConvertThreads":2,"pythonServerUrl":"","resourceServerPort":0}"#,
    ));
    assert_eq!(store.get().unwrap().default_convert_threads, 2);
  }

  #[test]
  fn single_worker_saved_after_migration_is_kept() {
    let store = store_with(None);
    let mut settings = store.get().unwrap();
    settings.default_convert_threads = 1;
    // The frontend may omit the version; saving through the store still marks it current.
    settings.settings_version = 0;
    store.update(settings).unwrap();
    assert_eq!(store.get().unwrap().default_convert_threads, 1);
  }
}
