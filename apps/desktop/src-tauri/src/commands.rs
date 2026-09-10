//! Tauri commands — §5.4 desktop API surface.

use crate::process_manager::ProcessManager;
use crate::python_bridge::{cancel_python_task, spawn_python_task_follower};
use crate::settings_store::AppSettings;
use crate::state::AppState;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
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
  let name = config
    .task_name
    .or(config.name)
    .unwrap_or_default();
  let options = config.options.unwrap_or(json!({}));
  let task = state.tasks.create(
    &config.operation,
    &config.input.path(),
    &config.output.path(),
    options,
    &name,
  )?;

  if ProcessManager::processor_available() {
    let _ = state.tasks.append_log(
      &task.id,
      "[desktop] Phase 3: executing via local processor binary (no Python HTTP)",
    );
    state.processes.spawn_task(
      state.tasks.clone(),
      state.artifacts.clone(),
      task.id.clone(),
      state.data_dir.clone(),
    );
  } else {
    let _ = state.tasks.append_log(
      &task.id,
      "[desktop] processor binary unavailable; falling back to Python desktop_server HTTP bridge",
    );
    spawn_python_task_follower(
      state.tasks.clone(),
      state.artifacts.clone(),
      task.id.clone(),
      state.python_base(),
    );
  }
  Ok(json!({ "ok": true, "task": task, "id": task.id }))
}

#[tauri::command]
pub fn cancel_task(state: State<'_, AppState>, task_id: String) -> Result<Value, String> {
  let task = state
    .tasks
    .request_cancel(&task_id)?
    .ok_or_else(|| "task not found".to_string())?;
  // Prefer local processor cancel
  let _ = state.processes.cancel(&task_id);
  if let Some(ref py_id) = task.python_task_id {
    let _ = cancel_python_task(&state.python_base(), py_id);
  }
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


/// OSGB scan in-process (Rust port of desktop_server osgb_scan — no Python).
#[tauri::command]
pub fn scan_osgb(path: String) -> Result<Value, String> {
  Ok(scan_osgb_impl(&path))
}

fn scan_osgb_impl(path: &str) -> Value {
  // Inline lightweight port matching processor::stages::scan (keep desktop crate independent).
  use std::fs;
  use std::path::PathBuf;

  let tile_re = regex_lite_tile();
  let expanded = PathBuf::from(path);
  if !expanded.exists() {
    return json!({
      "ok": false,
      "valid": false,
      "path": expanded.to_string_lossy(),
      "errors": [format!("Path does not exist: {}", expanded.display())],
      "warnings": [],
      "summary": {},
    });
  }

  let mut root = expanded.canonicalize().unwrap_or(expanded.clone());
  let mut warnings: Vec<String> = Vec::new();
  if root.file_name().and_then(|n| n.to_str()) == Some("Data")
    && root.parent().map(|p| p.join("metadata.xml").is_file()).unwrap_or(false)
  {
    warnings.push("Selected Data/ directory; normalized to dataset root.".into());
    root = root.parent().unwrap().to_path_buf();
  }

  let mut errors: Vec<String> = Vec::new();
  let meta_path = root.join("metadata.xml");
  let data_dir = root.join("Data");
  if !meta_path.is_file() {
    errors.push(format!("Missing metadata.xml under {}", root.display()));
  }
  if !data_dir.is_dir() {
    errors.push(format!("Missing Data/ directory under {}", root.display()));
  }

  let mut metadata = json!({});
  if meta_path.is_file() {
    if let Ok(text) = fs::read_to_string(&meta_path) {
      let srs = extract_xml_tag(&text, "SRS");
      let origin = extract_xml_tag(&text, "SRSOrigin");
      metadata = json!({
        "path": meta_path.to_string_lossy(),
        "srs": srs,
        "srsOrigin": origin,
      });
      if srs.is_none() {
        warnings.push("metadata.xml has no SRS; local preview ok, geographic export needs CRS.".into());
      }
    }
  }

  let mut tiles = Vec::new();
  let mut total_osgb: u64 = 0;
  let mut total_bytes: u64 = 0;
  if data_dir.is_dir() {
    let mut children: Vec<_> = fs::read_dir(&data_dir)
      .into_iter()
      .flatten()
      .flatten()
      .filter(|e| e.path().is_dir())
      .collect();
    children.sort_by_key(|e| e.file_name());
    for child in children {
      let name = child.file_name().to_string_lossy().into_owned();
      if !tile_re(&name) {
        warnings.push(format!("Non-standard directory under Data/: {name}"));
        continue;
      }
      let entry = child.path().join(format!("{name}.osgb"));
      let entry_ok = entry.is_file();
      let osgb_files: Vec<_> = fs::read_dir(child.path())
        .into_iter()
        .flatten()
        .flatten()
        .filter(|e| {
          e.path()
            .extension()
            .and_then(|x| x.to_str())
            .map(|x| x.eq_ignore_ascii_case("osgb"))
            .unwrap_or(false)
        })
        .collect();
      let file_count = osgb_files.len() as u64;
      let size: u64 = osgb_files
        .iter()
        .filter_map(|e| e.metadata().ok().map(|m| m.len()))
        .sum();
      total_osgb += file_count;
      total_bytes += size;
      if !entry_ok {
        errors.push(format!("Missing entry OSGB (must match folder name): {}", entry.display()));
      }
      tiles.push(json!({
        "name": name,
        "entryExists": entry_ok,
        "osgbCount": file_count,
        "bytes": size,
      }));
    }
  }
  if data_dir.is_dir() && tiles.is_empty() {
    errors.push("No Tile_* directories found under Data/".into());
  }

  let valid = errors.is_empty();
  let srs = metadata.get("srs").cloned().unwrap_or(Value::Null);
  let unit_hint = match srs.as_str() {
    Some(s) if s.to_uppercase().starts_with("ENU:") => {
      "ENU 局部坐标：米（东/北/天）；地理原点为经纬度（度）"
    }
    Some(_) => "自定义 CRS：请确认单位与轴序；高程基准独立",
    None => "未知单位（缺 SRS）",
  };
  let geo = json!({
    "scanSrs": srs,
    "scanOrigin": metadata.get("srsOrigin"),
    "effectiveCrs": srs,
    "effectiveOrigin": metadata.get("srsOrigin").and_then(|o| o.as_str()).map(|text| {
      let parts: Vec<_> = text.split(',').collect();
      json!({
        "x": parts.get(0).and_then(|p| p.parse::<f64>().ok()),
        "y": parts.get(1).and_then(|p| p.parse::<f64>().ok()),
        "z": parts.get(2).and_then(|p| p.parse::<f64>().ok()),
        "source": "metadata",
        "text": text,
      })
    }),
    "unitHint": unit_hint,
    "hasCrs": srs.as_str().map(|s| !s.is_empty()).unwrap_or(false),
    "geographicExport": false,
  });

  json!({
    "ok": valid,
    "valid": valid,
    "path": root.to_string_lossy(),
    "requestedPath": path,
    "errors": errors,
    "warnings": warnings,
    "metadata": metadata,
    "summary": {
      "root": root.to_string_lossy(),
      "tileCount": tiles.len(),
      "osgbFileCount": total_osgb,
      "totalBytes": total_bytes,
      "srs": srs,
      "srsOrigin": metadata.get("srsOrigin"),
    },
    "tiles": tiles,
    "hasMetadata": meta_path.is_file(),
    "hasDataDir": data_dir.is_dir(),
    "tileCount": tiles.len(),
    "unitHint": unit_hint,
    "geo": geo,
    "message": if valid { "OSGB root OK" } else { "OSGB validation failed" },
  })
}

fn extract_xml_tag(xml: &str, tag: &str) -> Option<String> {
  let open = format!("<{tag}>");
  let close = format!("</{tag}>");
  let start = xml.find(&open)? + open.len();
  let end = xml[start..].find(&close)? + start;
  let val = xml[start..end].trim();
  if val.is_empty() { None } else { Some(val.to_string()) }
}

fn regex_lite_tile() -> impl Fn(&str) -> bool {
  |name: &str| {
    let n = name.as_bytes();
    // Tile_+digits_+digits (case-insensitive Tile_)
    let lower = name.to_ascii_lowercase();
    if !lower.starts_with("tile_") {
      return false;
    }
    let rest = &name[5..];
    let parts: Vec<&str> = rest.split('_').collect();
    if parts.len() != 2 {
      return false;
    }
    parts.iter().all(|p| {
      let p = p.strip_prefix('+').or_else(|| p.strip_prefix('-')).unwrap_or(p);
      !p.is_empty() && p.chars().all(|c| c.is_ascii_digit())
    }) && !n.is_empty()
  }
}

#[tauri::command]
pub fn health(state: State<'_, AppState>) -> Result<Value, String> {
  let convert = std::env::var("GEOFORGE_3DTILE")
    .unwrap_or_else(|_| "/workspace/runtime/3dtile-bin/run.sh".into());
  let processor = ProcessManager::processor_available();
  Ok(json!({
    "ok": true,
    "status": "ok",
    "product": "geoforge-desktop",
    "version": "0.1.0-phase3",
    "message": if processor {
      "Tauri + processor (Python HTTP not required for convert/process)"
    } else {
      "Tauri ready; processor binary missing — convert falls back to Python HTTP"
    },
    "convertBin": convert,
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
      "postprocessBasisu": PathBuf::from(
        "/workspace/repos/3dtiles/vcpkg_installed/x64-linux/tools/basisu/basisu"
      ).is_file(),
      "basisuPath": "/workspace/repos/3dtiles/vcpkg_installed/x64-linux/tools/basisu/basisu",
      "notes": [
        "Phase 3: KTX2 still via Python texture_ktx2 / basisu when requested; keep mode needs no Python."
      ],
    },
  }))
}

#[tauri::command]
pub fn capabilities() -> Result<Value, String> {
  let basisu = PathBuf::from(
    "/workspace/repos/3dtiles/vcpkg_installed/x64-linux/tools/basisu/basisu",
  );
  let basisu_ok = basisu.is_file()
    || std::env::var("GEOFORGE_BASISU").map(|p| PathBuf::from(p).is_file()).unwrap_or(false);
  let modes = vec![
    json!({ "mode": "keep", "supported": true, "cliFlags": [], "postprocess": false }),
    json!({
      "mode": "ktx2-etc1s",
      "supported": basisu_ok,
      "cliFlags": [],
      "postprocess": true,
      "reason": if basisu_ok { Value::Null } else { json!("basisu not found") },
      "processTileset": { "mode": "ktx2-etc1s", "supported": basisu_ok },
    }),
    json!({
      "mode": "ktx2-uastc",
      "supported": basisu_ok,
      "cliFlags": [],
      "postprocess": true,
      "reason": if basisu_ok { Value::Null } else { json!("basisu not found") },
      "processTileset": { "mode": "ktx2-uastc", "supported": basisu_ok },
    }),
    json!({
      "mode": "ktx2",
      "supported": basisu_ok,
      "cliFlags": [],
      "postprocess": true,
      "reason": if basisu_ok { Value::Null } else { json!("basisu not found") },
      "processTileset": { "mode": "ktx2", "supported": basisu_ok },
    }),
  ];
  Ok(json!({
    "ok": true,
    "convert": {
      "bin": std::env::var("GEOFORGE_3DTILE").unwrap_or_else(|_| "/workspace/runtime/3dtile-bin/run.sh".into()),
      "exists": PathBuf::from(
        std::env::var("GEOFORGE_3DTILE").unwrap_or_else(|_| "/workspace/runtime/3dtile-bin/run.sh".into())
      ).is_file(),
      "processor": ProcessManager::processor_available(),
    },
    "textureModes": modes,
    "aliases": { "ktx2": "ktx2-etc1s" },
    "postprocessBasisu": {
      "available": basisu_ok,
      "path": if basisu_ok { json!(basisu.to_string_lossy()) } else { Value::Null },
    },
  }))
}
