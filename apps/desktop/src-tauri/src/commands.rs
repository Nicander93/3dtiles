//! Tauri commands — §5.4 desktop API surface.

use crate::process_manager::ProcessManager;
use crate::settings_store::AppSettings;
use crate::state::AppState;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use std::process::{Command, Stdio};
use tauri::AppHandle;
use tauri::State;
use tauri_plugin_dialog::{DialogExt, FilePath};

fn file_path_to_string(path: FilePath) -> Result<String, String> {
  path
    .into_path()
    .map(|p| p.to_string_lossy().into_owned())
    .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn select_input_directory(app: AppHandle) -> Result<Option<String>, String> {
  let picked = app
    .dialog()
    .file()
    .set_title("选择输入目录")
    .blocking_pick_folder();
  match picked {
    Some(path) => Ok(Some(file_path_to_string(path)?)),
    None => Ok(None),
  }
}

#[tauri::command]
pub fn select_output_directory(app: AppHandle) -> Result<Option<String>, String> {
  let picked = app
    .dialog()
    .file()
    .set_title("选择输出目录")
    .blocking_pick_folder();
  match picked {
    Some(path) => Ok(Some(file_path_to_string(path)?)),
    None => Ok(None),
  }
}

#[tauri::command]
pub fn select_tileset_file(app: AppHandle) -> Result<Option<String>, String> {
  let picked = app
    .dialog()
    .file()
    .set_title("选择 tileset.json")
    .add_filter("3D Tiles", &["json"])
    .blocking_pick_file();
  match picked {
    Some(path) => Ok(Some(file_path_to_string(path)?)),
    None => Ok(None),
  }
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SubmitTaskConfig {
  pub operation: String,
  pub input: PathOrObject,
  pub output: PathOrObject,
  pub options: Option<Value>,
  pub task_name: Option<String>,
  pub name: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum PathOrObject {
  Str(String),
  Obj { path: String },
}

impl PathOrObject {
  fn path(&self) -> String {
    match self {
      PathOrObject::Str(s) => s.clone(),
      PathOrObject::Obj { path } => path.clone(),
    }
  }
}

#[tauri::command]
pub fn submit_task(state: State<'_, AppState>, config: SubmitTaskConfig) -> Result<Value, String> {
  if !ProcessManager::processor_available() {
    return Err(
      "找不到 processor 组件，无法创建任务。请修复安装或设置环境变量 GEOFORGE_PROCESSOR。".into(),
    );
  }
  let name = config.task_name.or(config.name).unwrap_or_default();
  let options = config.options.unwrap_or(json!({}));
  // Preflight path policy (same rules as processor); provisional id for validation only
  let provisional_id = "task-preflight0";
  processor::validate_io_paths(
    std::path::Path::new(&config.input.path()),
    std::path::Path::new(&config.output.path()),
    provisional_id,
  )
  .map_err(|e| format!("路径校验失败: {e}"))?;

  let task = state.tasks.create(
    &config.operation,
    &config.input.path(),
    &config.output.path(),
    options,
    &name,
  )?;

  let _ = state
    .tasks
    .append_log(&task.id, "[desktop] queued for local processor (serial)");
  state.processes.spawn_task(
    state.tasks.clone(),
    state.artifacts.clone(),
    task.id.clone(),
    state.data_dir.clone(),
  );
  Ok(json!({ "ok": true, "task": task, "id": task.id }))
}

#[tauri::command]
pub fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<Value, String> {
  let task = state
    .tasks
    .request_cancel(&task_id)?
    .ok_or_else(|| "task not found".to_string())?;
  // Cooperative cancel — do not block UI
  let _ = state.processes.cancel(&task_id);
  // Queued-only tasks never start
  if task.status == "queued" || task.stage == "queued" {
    let _ = state.tasks.update_fields(&task_id, |t| {
      t.status = "cancelled".into();
      t.stage = "cancelled".into();
      t.finished_at = Some(crate::db::now_secs());
    });
    state.processes.notify_idle();
  }
  let task = state.tasks.get(&task_id)?.unwrap_or(task);
  Ok(json!({ "ok": true, "task": task }))
}

#[tauri::command]
pub fn get_task(state: State<'_, AppState>, task_id: String) -> Result<Value, String> {
  let task = state
    .tasks
    .get(&task_id)?
    .ok_or_else(|| "task not found".to_string())?;
  Ok(json!({ "ok": true, "task": task }))
}

#[derive(Debug, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ListTasksFilter {
  pub status: Option<String>,
}

#[tauri::command]
pub fn list_tasks(
  state: State<'_, AppState>,
  filter: Option<ListTasksFilter>,
) -> Result<Value, String> {
  let status = filter.as_ref().and_then(|f| f.status.as_deref());
  let tasks = state.tasks.list(status)?;
  // The task list is polled frequently. Keep its payload small and fetch the
  // bounded log tail only when a task is opened in the details drawer.
  let tasks = tasks
    .into_iter()
    .map(|task| {
      let mut value = serde_json::to_value(task).map_err(|error| error.to_string())?;
      if let Some(object) = value.as_object_mut() {
        object.insert("log".into(), Value::String(String::new()));
      }
      Ok::<Value, String>(value)
    })
    .collect::<Result<Vec<_>, _>>()?;
  Ok(json!({ "ok": true, "tasks": tasks }))
}

#[tauri::command]
pub fn get_task_logs(
  state: State<'_, AppState>,
  task_id: String,
  tail: Option<usize>,
) -> Result<Value, String> {
  let logs = state
    .tasks
    .get_logs(&task_id, tail.unwrap_or(500))?
    .ok_or_else(|| "task not found".to_string())?;
  let mut out = match logs {
    Value::Object(map) => map,
    _ => serde_json::Map::new(),
  };
  out.insert("ok".into(), json!(true));
  Ok(Value::Object(out))
}

#[tauri::command]
pub fn list_artifacts(state: State<'_, AppState>) -> Result<Value, String> {
  let artifacts = state.artifacts.list()?;
  Ok(json!({ "ok": true, "artifacts": artifacts }))
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterArtifactArgs {
  pub path: String,
  pub task_id: Option<String>,
  pub kind: Option<String>,
  pub label: Option<String>,
}

/// Register an existing tileset directory as an artifact (smoke / import).
#[tauri::command]
pub fn register_artifact(
  state: State<'_, AppState>,
  args: RegisterArtifactArgs,
) -> Result<Value, String> {
  let art = state.artifacts.register(
    &args.path,
    args.task_id.as_deref(),
    args.kind.as_deref().unwrap_or("3dtiles"),
    args.label.as_deref().unwrap_or(""),
  )?;
  Ok(json!({ "ok": true, "artifact": art.to_json_value() }))
}

#[tauri::command]
pub fn get_preview_url(state: State<'_, AppState>, artifact_id: String) -> Result<Value, String> {
  let art = state
    .artifacts
    .get(&artifact_id)?
    .ok_or_else(|| "artifact not found".to_string())?;
  if !art.has_tileset {
    return Err(format!("tileset.json missing under {}", art.path));
  }
  let url = state.resource.preview_url(&artifact_id);
  Ok(json!({
    "ok": true,
    "id": artifact_id,
    "url": url,
    "previewUrl": url,
    "path": format!("{}/tileset.json", art.path.trim_end_matches('/')),
    "has_tileset": true,
    "resourcePort": state.resource.port,
    "resourceBase": state.resource.base_url,
  }))
}

#[tauri::command]
pub fn open_artifact_directory(
  state: State<'_, AppState>,
  artifact_id: String,
) -> Result<Value, String> {
  let art = state
    .artifacts
    .get(&artifact_id)?
    .ok_or_else(|| "artifact not found".to_string())?;
  let path = PathBuf::from(&art.path);
  opener::open(&path).map_err(|e| format!("open directory failed: {e}"))?;
  Ok(json!({ "ok": true, "path": art.path }))
}

#[tauri::command]
pub fn get_settings(state: State<'_, AppState>) -> Result<AppSettings, String> {
  state.settings.get()
}

#[tauri::command]
pub fn update_settings(
  state: State<'_, AppState>,
  settings: AppSettings,
) -> Result<AppSettings, String> {
  state.settings.update(settings)
}

#[tauri::command]
pub fn get_resource_server_info(state: State<'_, AppState>) -> Result<Value, String> {
  Ok(json!({
    "ok": true,
    "port": state.resource.port,
    "baseUrl": state.resource.base_url,
    "dataDir": state.data_dir,
  }))
}

/// OSGB scan via processor CLI (single scan implementation).
#[tauri::command]
pub fn scan_osgb(path: String) -> Result<Value, String> {
  let bin = ProcessManager::processor_bin().ok_or_else(|| {
    "找不到 processor 组件，无法扫描。请修复安装或设置 GEOFORGE_PROCESSOR。".to_string()
  })?;
  let mut command = Command::new(&bin);
  ProcessManager::apply_runtime_env(&mut command);
  let output = command
    .args(["scan-osgb", "--path", &path])
    .stdout(Stdio::piped())
    .stderr(Stdio::piped())
    .output()
    .map_err(|e| format!("启动 processor 扫描失败: {e}"))?;
  let stdout = String::from_utf8_lossy(&output.stdout);
  let stderr = String::from_utf8_lossy(&output.stderr);
  if stdout.trim().is_empty() {
    return Err(format!(
      "processor 扫描无输出 (exit {:?}): {stderr}",
      output.status.code()
    ));
  }
  serde_json::from_str(stdout.trim()).map_err(|e| {
    format!(
      "解析扫描结果失败: {e}; stderr={stderr}; stdout={}",
      stdout.chars().take(400).collect::<String>()
    )
  })
}

fn resolve_convert_bin() -> PathBuf {
  if let Ok(p) = std::env::var("GEOFORGE_3DTILE") {
    return PathBuf::from(p);
  }
  if let Ok(root) = std::env::var("GEOFORGE_RUNTIME_ROOT") {
    for name in ["_3dtile.exe", "_3dtile"] {
      let cand = PathBuf::from(&root).join("converter").join(name);
      if cand.is_file() {
        return cand;
      }
    }
  }
  if let Ok(exe) = std::env::current_exe() {
    if let Some(dir) = exe.parent() {
      for name in ["_3dtile", "_3dtile.exe"] {
        let cand = dir.join(name);
        if cand.is_file() {
          return cand;
        }
      }
      let bundled = dir.join("resources").join("runtime").join("converter");
      for name in ["_3dtile.exe", "_3dtile"] {
        let cand = bundled.join(name);
        if cand.is_file() {
          return cand;
        }
      }
    }
  }
  PathBuf::from("_3dtile")
}

fn convert_status() -> Value {
  let bin = resolve_convert_bin();
  let exists = bin.is_file();
  json!({
    "bin": bin.to_string_lossy(),
    "exists": exists,
    "docker": false,
    "processor": ProcessManager::processor_available(),
  })
}

#[tauri::command]
pub fn health(state: State<'_, AppState>) -> Result<Value, String> {
  let convert = convert_status();
  let processor = ProcessManager::processor_available();
  let convert_ok = convert
    .get("exists")
    .and_then(|v| v.as_bool())
    .unwrap_or(false);
  Ok(json!({
    "ok": true,
    "status": "ok",
    "product": "geoforge-desktop",
    "version": "0.1.0-phase3",
    "message": if !processor {
      "找不到 processor 组件。请修复安装或设置 GEOFORGE_PROCESSOR。"
    } else if convert_ok {
      "Tauri + processor"
    } else {
      "Processor 已找到，但没有 _3dtile，OSGB 转换会失败（运行 prepare-converter.ps1）"
    },
    "convertBin": convert.get("bin").and_then(|v| v.as_str()).unwrap_or(""),
    "convert": convert,
    "processorAvailable": processor,
    "resourceServer": {
      "port": state.resource.port,
      "baseUrl": state.resource.base_url,
    },
    "texture": {
      "enableTextureCompress": false,
      "ktx2Etc1s": true,
      "ktx2Uastc": true,
      "processTilesetTexture": true,
      "postprocessBasisu": false,
      "basisuPath": "",
      "notes": [
        "KTX2 via geoforge-texture / basisu when bundled; keep mode needs no texture tool."
      ],
    },
  }))
}

#[tauri::command]
pub fn capabilities() -> Result<Value, String> {
  // Prefer spawning processor capabilities so probe matches task execution env
  if let Some(bin) = ProcessManager::processor_bin() {
    let mut cmd = Command::new(&bin);
    ProcessManager::apply_runtime_env(&mut cmd);
    let out = cmd
      .args(["capabilities", "--json"])
      .stdout(Stdio::piped())
      .stderr(Stdio::piped())
      .output();
    if let Ok(o) = out {
      if o.status.success() {
        if let Ok(v) = serde_json::from_slice::<Value>(&o.stdout) {
          return Ok(v);
        }
      }
    }
  }
  // Fallback: in-process (same crate rules)
  Ok(processor::capabilities_json())
}
