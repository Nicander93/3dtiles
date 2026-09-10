//! Phase 12 Layer A acceptance — processor catches missing / corrupt content.

use processor::{validate_and_write_report, validate_tileset_tree};
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

static SEQ: AtomicU64 = AtomicU64::new(0);

fn scratch(name: &str) -> PathBuf {
    let n = SEQ.fetch_add(1, Ordering::SeqCst);
    let d = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../target")
        .join(format!("phase12_{name}_{n}"));
    let _ = fs::remove_dir_all(&d);
    fs::create_dir_all(&d).unwrap();
    d
}

fn write(path: &Path, body: &str) {
    if let Some(p) = path.parent() {
        fs::create_dir_all(p).unwrap();
    }
    fs::write(path, body).unwrap();
}

fn tiny_glb() -> Vec<u8> {
    let json = br#"{"asset":{"version":"2.0"}}"#;
    let mut json_pad = json.to_vec();
    while json_pad.len() % 4 != 0 {
        json_pad.push(b' ');
    }
    let length = 12 + 8 + json_pad.len();
    let mut out = Vec::new();
    out.extend_from_slice(b"glTF");
    out.extend_from_slice(&2u32.to_le_bytes());
    out.extend_from_slice(&(length as u32).to_le_bytes());
    out.extend_from_slice(&(json_pad.len() as u32).to_le_bytes());
    out.extend_from_slice(b"JSON");
    out.extend_from_slice(&json_pad);
    out
}

fn tiny_b3dm() -> Vec<u8> {
    let glb = tiny_glb();
    let ftj = b"{\"BATCH_LENGTH\":0}  "; // 20 bytes
    let total = 28 + ftj.len() + glb.len();
    let mut out = Vec::new();
    out.extend_from_slice(b"b3dm");
    out.extend_from_slice(&1u32.to_le_bytes());
    out.extend_from_slice(&(total as u32).to_le_bytes());
    out.extend_from_slice(&(ftj.len() as u32).to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(&0u32.to_le_bytes());
    out.extend_from_slice(ftj);
    out.extend_from_slice(&glb);
    out
}

fn good_tileset(content_uri: &str) -> String {
    format!(
        r#"{{
  "asset": {{ "version": "1.0" }},
  "geometricError": 100.0,
  "root": {{
    "boundingVolume": {{ "box": [0,0,0, 10,0,0, 0,10,0, 0,0,5] }},
    "geometricError": 50.0,
    "refine": "REPLACE",
    "content": {{ "uri": "{content_uri}" }}
  }}
}}"#
    )
}

fn tree_with_external() -> PathBuf {
    let dir = scratch("ok_external");
    write(
        &dir.join("tileset.json"),
        r#"{
  "asset": { "version": "1.0" },
  "geometricError": 200.0,
  "root": {
    "boundingVolume": { "box": [0,0,0, 20,0,0, 0,20,0, 0,0,5] },
    "geometricError": 100.0,
    "refine": "REPLACE",
    "children": [
      {
        "boundingVolume": { "box": [0,0,0, 10,0,0, 0,10,0, 0,0,5] },
        "geometricError": 50.0,
        "content": { "uri": "./Data/BlockA/tileset.json" }
      }
    ]
  }
}"#,
    );
    write(
        &dir.join("Data/BlockA/tileset.json"),
        &good_tileset("./BlockA.b3dm"),
    );
    let mut f = fs::File::create(dir.join("Data/BlockA/BlockA.b3dm")).unwrap();
    f.write_all(&tiny_b3dm()).unwrap();
    dir
}

#[test]
fn layer_a_passes_known_good_tree() {
    let dir = tree_with_external();
    let report = validate_and_write_report(&dir).expect("write report");
    assert!(report.ok, "{:?}", report.issues);
    assert!(report.tileset_count >= 2);
    assert_eq!(report.content_count, 1);
    assert!(dir.join("validation_internal.json").is_file());
}

#[test]
fn layer_a_catches_missing_content() {
    let dir = tree_with_external();
    fs::remove_file(dir.join("Data/BlockA/BlockA.b3dm")).unwrap();
    let report = validate_tileset_tree(&dir);
    assert!(!report.ok);
    assert!(
        report.issues.iter().any(|i| i.code == "CONTENT_MISSING"),
        "{:?}",
        report.issues
    );
}

#[test]
fn layer_a_catches_corrupt_b3dm() {
    let dir = tree_with_external();
    fs::write(
        dir.join("Data/BlockA/BlockA.b3dm"),
        b"b3dm\x01\x00\x00\x00\x40\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00NOT_A_VALID_GLB!!!!",
    )
    .unwrap();
    let report = validate_tileset_tree(&dir);
    assert!(!report.ok);
    assert!(
        report.issues.iter().any(|i| i.code == "CONTENT_INVALID"),
        "{:?}",
        report.issues
    );
}

#[test]
fn layer_a_catches_missing_asset_root() {
    let dir = scratch("no_asset");
    write(
        &dir.join("tileset.json"),
        r#"{ "geometricError": 1.0, "root": {
          "boundingVolume": {"box":[0,0,0,1,0,0,0,1,0,0,0,1]},
          "geometricError": 1.0,
          "refine": "REPLACE"
        }}"#,
    );
    let report = validate_tileset_tree(&dir);
    assert!(!report.ok);
    assert!(report.issues.iter().any(|i| i.code == "ASSET_MISSING"));
}
