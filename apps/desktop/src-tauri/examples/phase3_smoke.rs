//! Phase 3 smoke: submit convert-osgb via ProcessManager → SQLite → artifact HTTP.
//! Requires: `cargo build -p processor` and GEOFORGE_3DTILE / fixture OSGB.

use geoforge_desktop_lib::init_headless;
use serde_json::json;
use std::path::PathBuf;
use std::time::{Duration, Instant};

#[tokio::main]
async fn main() {
  let fixture = std::env::var("GEOFORGE_SMOKE_OSGB")
    .unwrap_or_else(|_| "/workspace/data/osgb_one_tile".into());
  let out = std::env::var("GEOFORGE_SMOKE_OUT").unwrap_or_else(|_| {
    format!(
      "/workspace/data/geoforge_outputs/phase3_processor_{}",
      chrono_stamp()
    )
  });
  let data_dir = PathBuf::from(format!("/tmp/geoforge-phase3-smoke-{}", chrono_stamp()));

  println!("fixture={fixture}");
  println!("out={out}");
  println!("data_dir={}", data_dir.display());

  if !PathBuf::from(&fixture).join("metadata.xml").is_file() {
    eprintln!("PHASE3_SMOKE_FAIL missing fixture metadata.xml");
    std::process::exit(1);
  }

  let (state, _conn) = init_headless(data_dir.clone(), 0)
    .await
    .expect("init_headless");

  // Ensure processor binary discoverable
  if !geoforge_desktop_lib_processor_available() {
    eprintln!("PHASE3_SMOKE_FAIL processor binary not found; cargo build -p processor");
    std::process::exit(1);
  }

  let task = state
    .tasks
    .create(
      "convert-osgb",
      &fixture,
      &out,
      json!({ "texture": { "mode": "keep" }, "rebuildTop": { "enabled": false } }),
      "phase3-smoke",
    )
    .expect("create task");
  println!("task_id={}", task.id);

  state.processes.spawn_task(
    state.tasks.clone(),
    state.artifacts.clone(),
    task.id.clone(),
    state.data_dir.clone(),
  );

  let deadline = Instant::now() + Duration::from_secs(600);
  loop {
    if Instant::now() > deadline {
      eprintln!("PHASE3_SMOKE_FAIL timeout");
      std::process::exit(1);
    }
    let t = state.tasks.get(&task.id).unwrap().unwrap();
    println!("status={} stage={}", t.status, t.stage);
    if matches!(
      t.status.as_str(),
      "succeeded" | "completed" | "failed" | "cancelled" | "interrupted"
    ) {
      if t.status != "succeeded" && t.status != "completed" {
        eprintln!("PHASE3_SMOKE_FAIL status={} error={:?}", t.status, t.error);
        std::process::exit(1);
      }
      break;
    }
    std::thread::sleep(Duration::from_secs(2));
  }

  let t = state.tasks.get(&task.id).unwrap().unwrap();
  let artifact_id = t
    .progress
    .get("artifactId")
    .and_then(|v| v.as_str())
    .unwrap_or("")
    .to_string();
  if artifact_id.is_empty() {
    eprintln!("PHASE3_SMOKE_FAIL no artifactId in progress");
    std::process::exit(1);
  }
  let url = state.resource.preview_url(&artifact_id);
  println!("preview_url={url}");

  let client = reqwest::Client::new();
  let res = client.get(&url).send().await.expect("http get");
  let status = res.status();
  let bytes = res.bytes().await.expect("bytes").len();
  println!("http_status={status} bytes={bytes}");
  if !status.is_success() || bytes == 0 {
    eprintln!("PHASE3_SMOKE_FAIL preview HTTP");
    std::process::exit(1);
  }

  println!("PHASE3_SMOKE_OK task_id={} artifact_id={} out={out}", task.id, artifact_id);
}

fn chrono_stamp() -> String {
  std::time::SystemTime::now()
    .duration_since(std::time::UNIX_EPOCH)
    .unwrap()
    .as_secs()
    .to_string()
}

fn geoforge_desktop_lib_processor_available() -> bool {
  // Duplicate resolve logic lightly
  if let Ok(p) = std::env::var("GEOFORGE_PROCESSOR") {
    if std::path::Path::new(&p).is_file() {
      return true;
    }
  }
  std::path::Path::new("/workspace/repos/3dtiles/target/debug/processor").is_file()
    || std::path::Path::new("/workspace/repos/3dtiles/target/release/processor").is_file()
}
