//! Phase 2 smoke: register a known tileset, serve via Rust localhost, curl 200.
//!
//!   cd apps/desktop/src-tauri && cargo run --example phase2_smoke

use geoforge_desktop_lib::init_headless;
use std::path::PathBuf;
use std::time::Duration;

#[tokio::main]
async fn main() {
  let tiles = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
    .join("../../../examples/preview/tiles");
  assert!(
    tiles.join("tileset.json").is_file(),
    "missing fixture tileset at {:?}",
    tiles
  );

  let data = std::env::temp_dir().join(format!(
    "geoforge-phase2-smoke-{}",
    std::process::id()
  ));
  let _ = std::fs::remove_dir_all(&data);
  std::fs::create_dir_all(&data).unwrap();

  let (state, _conn) = init_headless(data.clone(), 0).await.expect("init");
  println!("resource_base={}", state.resource.base_url);
  println!("data_dir={}", data.display());

  let art = state
    .artifacts
    .register(
      &tiles.to_string_lossy(),
      None,
      "3dtiles",
      "phase2-smoke",
    )
    .expect("register");
  println!("artifact_id={}", art.id);
  println!("has_tileset={}", art.has_tileset);

  let url = state.resource.preview_url(&art.id);
  println!("preview_url={url}");

  // Persist check: reopen DB
  {
    let art2 = state.artifacts.get(&art.id).unwrap().expect("persisted");
    assert_eq!(art2.path, art.path);
    println!("sqlite_persist=ok");
  }

  // HTTP fetch
  let client = reqwest::Client::new();
  for _ in 0..20 {
    match client.get(&url).send().await {
      Ok(res) => {
        println!("http_status={}", res.status());
        assert!(res.status().is_success(), "expected 200");
        let body = res.text().await.unwrap();
        assert!(body.contains("geometricError") || body.contains("root"), "tileset body");
        println!("tileset_bytes={}", body.len());
        break;
      }
      Err(e) => {
        eprintln!("retry: {e}");
        tokio::time::sleep(Duration::from_millis(100)).await;
      }
    }
  }

  // Path traversal must fail
  let bad = format!(
    "{}/artifacts/{}/../../etc/passwd",
    state.resource.base_url, art.id
  );
  let res = client.get(&bad).send().await.expect("bad req");
  println!("traversal_status={}", res.status());
  assert!(
    res.status().is_client_error(),
    "traversal should be rejected"
  );

  // Settings persist
  let mut s = state.settings.get().unwrap();
  s.default_output_root = "/tmp/geoforge-out".into();
  state.settings.update(s).unwrap();
  let s2 = state.settings.get().unwrap();
  assert_eq!(s2.default_output_root, "/tmp/geoforge-out");
  println!("settings_persist=ok");

  println!("PHASE2_SMOKE_OK");
}
