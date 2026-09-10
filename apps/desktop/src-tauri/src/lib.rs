mod artifact_store;
mod commands;
mod db;
mod process_manager;
mod python_bridge;
mod resource_server;
mod settings_store;
mod state;
mod task_store;

pub use artifact_store::ArtifactStore;
pub use db::{default_data_dir, open_db};
pub use resource_server::start_resource_server;
pub use process_manager::ProcessManager;
pub use state::{init_headless, AppState};

use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
  tauri::Builder::default()
    .plugin(tauri_plugin_dialog::init())
    .setup(|app| {
      if cfg!(debug_assertions) {
        app.handle().plugin(
          tauri_plugin_log::Builder::default()
            .level(log::LevelFilter::Info)
            .build(),
        )?;
      }

      let handle = app.handle().clone();
      // Prefer OS app data dir when available.
      let data_dir = handle
        .path()
        .app_data_dir()
        .map(|p| p.join("geoforge"))
        .unwrap_or_else(|_| default_data_dir());
      if std::env::var_os("GEOFORGE_DATA_DIR").is_none() {
        std::env::set_var("GEOFORGE_DATA_DIR", &data_dir);
      }

      let state = tauri::async_runtime::block_on(async { AppState::init().await })
        .map_err(|e| {
          log::error!("AppState init failed: {e}");
          std::io::Error::new(std::io::ErrorKind::Other, e)
        })?;
      log::info!(
        "GeoForge desktop ready; data_dir={:?}; resource={}",
        state.data_dir,
        state.resource.base_url
      );
      app.manage(state);
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      commands::select_input_directory,
      commands::select_output_directory,
      commands::select_tileset_file,
      commands::submit_task,
      commands::cancel_task,
      commands::get_task,
      commands::list_tasks,
      commands::get_task_logs,
      commands::list_artifacts,
      commands::register_artifact,
      commands::get_preview_url,
      commands::open_artifact_directory,
      commands::get_settings,
      commands::update_settings,
      commands::get_resource_server_info,
      commands::scan_osgb,
      commands::health,
      commands::capabilities,
    ])
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
