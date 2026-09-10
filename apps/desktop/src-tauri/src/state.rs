//! Shared application state for Tauri commands.

use crate::artifact_store::ArtifactStore;
use crate::db::{default_data_dir, open_db};
use crate::process_manager::ProcessManager;
use crate::resource_server::{start_resource_server, ResourceServerHandle};
use crate::settings_store::{AppSettings, SettingsStore};
use crate::task_store::TaskStore;
use parking_lot::Mutex;
use rusqlite::Connection;
use std::path::PathBuf;
use std::sync::Arc;

pub struct AppState {
  pub data_dir: PathBuf,
  pub tasks: TaskStore,
  pub artifacts: ArtifactStore,
  pub settings: SettingsStore,
  pub resource: ResourceServerHandle,
  pub processes: ProcessManager,
}

impl AppState {
  pub async fn init() -> Result<Self, String> {
    let data_dir = if let Ok(p) = std::env::var("GEOFORGE_DATA_DIR") {
      PathBuf::from(p)
    } else {
      default_data_dir()
    };
    std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
    let conn = open_db(&data_dir).map_err(|e| e.to_string())?;
    let conn = Arc::new(Mutex::new(conn));
    let tasks = TaskStore::new(conn.clone(), data_dir.clone());
    tasks.mark_stale_interrupted()?;
    let artifacts = ArtifactStore::new(conn.clone());
    let settings = SettingsStore::new(conn);
    let app_settings = settings.get().unwrap_or_default();
    let preferred = if app_settings.resource_server_port > 0 {
      app_settings.resource_server_port
    } else {
      std::env::var("GEOFORGE_RESOURCE_PORT")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(0)
    };
    let resource = start_resource_server(artifacts.roots_handle(), preferred).await?;
    Ok(Self {
      data_dir,
      tasks,
      artifacts,
      settings,
      resource,
      processes: ProcessManager::new(),
    })
  }

  pub fn python_base(&self) -> String {
    self
      .settings
      .get()
      .map(|s| {
        if s.python_server_url.is_empty() {
          crate::python_bridge::python_base_default()
        } else {
          s.python_server_url
        }
      })
      .unwrap_or_else(|_| crate::python_bridge::python_base_default())
  }
}

/// Headless init for smoke tests / examples (no Tauri).
pub async fn init_headless(
  data_dir: PathBuf,
  port: u16,
) -> Result<(AppState, Arc<Mutex<Connection>>), String> {
  std::fs::create_dir_all(&data_dir).map_err(|e| e.to_string())?;
  let conn = open_db(&data_dir).map_err(|e| e.to_string())?;
  let conn = Arc::new(Mutex::new(conn));
  let tasks = TaskStore::new(conn.clone(), data_dir.clone());
  let _ = tasks.mark_stale_interrupted();
  let artifacts = ArtifactStore::new(conn.clone());
  let settings = SettingsStore::new(conn.clone());
  let mut s = settings.get().unwrap_or_else(|_| AppSettings::default());
  if port > 0 {
    s.resource_server_port = port;
    let _ = settings.update(s.clone());
  }
  let resource = start_resource_server(artifacts.roots_handle(), port).await?;
  Ok((
    AppState {
      data_dir,
      tasks,
      artifacts,
      settings,
      resource,
      processes: ProcessManager::new(),
    },
    conn,
  ))
}
