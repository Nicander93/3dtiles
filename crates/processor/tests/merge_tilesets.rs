use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

fn fixture(root: &Path, name: &str, offset: f64) -> PathBuf {
    let dir = root.join(name);
    fs::create_dir(&dir).unwrap();
    let gltf = json!({"asset":{"version":"2.0"}, "scenes":[{"nodes":[0]}], "scene":0,
        "nodes":[{"mesh":0}], "meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],
        "buffers":[{"byteLength":36}], "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}]});
    let mut text = serde_json::to_vec(&gltf).unwrap();
    while text.len() % 4 != 0 {
        text.push(b' ');
    }
    let mut glb = Vec::new();
    glb.extend_from_slice(b"glTF");
    glb.extend_from_slice(&2_u32.to_le_bytes());
    glb.extend_from_slice(&((12 + 8 + text.len() + 8 + 36) as u32).to_le_bytes());
    glb.extend_from_slice(&(text.len() as u32).to_le_bytes());
    glb.extend_from_slice(b"JSON");
    glb.extend(text);
    glb.extend_from_slice(&36_u32.to_le_bytes());
    glb.extend_from_slice(&0x004e4942_u32.to_le_bytes());
    for n in [0_f32, 0., 0., 1., 0., 0., 0., 1., 0.] {
        glb.extend_from_slice(&n.to_le_bytes());
    }
    fs::write(dir.join("tile.glb"), glb).unwrap();
    fs::write(dir.join("tileset.json"), serde_json::to_vec_pretty(&json!({"asset":{"version":"1.0"},
        "geometricError": 8, "extras":{"keep":"source metadata"},
        "root":{"boundingVolume":{"box":[0,0,0,1,0,0,0,1,0,0,0,1]},"geometricError":0,"refine":"REPLACE",
        "transform":[1,0,0,0,0,1,0,0,0,0,1,0,offset,0,0,1],"content":{"uri":"tile.glb"}}
    })).unwrap()).unwrap();
    dir
}

fn command(a: &Path, b: &Path, output: &Path, id: &str) -> Command {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_processor"));
    cmd.arg("merge-tilesets")
        .arg("-i")
        .arg(a)
        .arg(b)
        .arg("-o")
        .arg(output)
        .arg("--task-id")
        .arg(id);
    cmd
}

#[test]
fn cli_merges_portable_sources_preserving_bytes_transforms_and_external_subtrees() {
    let temp = tempfile::tempdir().unwrap();
    let a = fixture(temp.path(), "中文 input A", 100.);
    let b = fixture(temp.path(), "input B", -200.);
    // A nested tileset points back to a resource inside the source root.
    fs::create_dir(a.join("nested")).unwrap();
    let mut nested: Value =
        serde_json::from_slice(&fs::read(a.join("tileset.json")).unwrap()).unwrap();
    nested["root"]["content"]["uri"] = json!("../tile.glb");
    nested["root"].as_object_mut().unwrap().remove("transform");
    fs::write(
        a.join("nested/child.json"),
        serde_json::to_vec(&nested).unwrap(),
    )
    .unwrap();
    let mut source: Value =
        serde_json::from_slice(&fs::read(a.join("tileset.json")).unwrap()).unwrap();
    source["root"]["content"]["uri"] = json!("nested/child.json");
    fs::write(a.join("tileset.json"), serde_json::to_vec(&source).unwrap()).unwrap();
    let source_bytes = fs::read(a.join("tileset.json")).unwrap();
    let glb_bytes = fs::read(a.join("tile.glb")).unwrap();
    let output = temp.path().join("merged");
    let result = command(&a, &b.join("tileset.json"), &output, "merge-ok")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let merged: Value =
        serde_json::from_slice(&fs::read(output.join("tileset.json")).unwrap()).unwrap();
    assert_eq!(merged["root"]["children"].as_array().unwrap().len(), 2);
    assert!(merged["root"].get("transform").is_none());
    assert!(merged["root"]["children"][0].get("transform").is_none());
    assert_eq!(
        merged["root"]["children"][0]["boundingVolume"]["box"][0],
        100.
    );
    assert_eq!(merged["root"]["boundingVolume"]["box"][3], 151.);
    assert_eq!(
        fs::read(output.join("sources/source-001/tileset.json")).unwrap(),
        source_bytes
    );
    assert_eq!(
        fs::read(output.join("sources/source-001/tile.glb")).unwrap(),
        glb_bytes
    );
    assert_eq!(fs::read(a.join("tileset.json")).unwrap(), source_bytes);
    // The copied dataset must survive removal of the original input directories.
    fs::remove_dir_all(&a).unwrap();
    fs::remove_dir_all(&b).unwrap();
    let report = processor::validate_tileset_tree(&output);
    assert!(report.ok, "{report:?}");
    assert_eq!(report.tileset_count, 4);
    assert!(!temp.path().join(".geoforge-task-merge-ok").exists());
    let events: Vec<Value> = String::from_utf8(result.stdout)
        .unwrap()
        .lines()
        .map(|l| serde_json::from_str(l).unwrap())
        .collect();
    assert!(events
        .iter()
        .any(|e| e["stage"] == "merge" && e["type"] == "progress" && e["completed"] == 2));
    assert!(events.iter().any(|e| e["type"] == "result"));
}

#[test]
fn cli_rejects_duplicate_overlap_existing_output_and_missing_resources_without_changing_inputs() {
    let temp = tempfile::tempdir().unwrap();
    let a = fixture(temp.path(), "a", 0.);
    let b = fixture(temp.path(), "b", 0.);
    let bytes = fs::read(b.join("tileset.json")).unwrap();
    let output = temp.path().join("output");
    assert!(!command(&a, &a.join("tileset.json"), &output, "duplicate")
        .output()
        .unwrap()
        .status
        .success());
    assert!(!command(&a, &b, &b.join("new/deep/output"), "overlap")
        .output()
        .unwrap()
        .status
        .success());
    assert!(
        !b.join("new").exists(),
        "preflight must not modify a later input"
    );
    fs::create_dir(&output).unwrap();
    fs::write(output.join("sentinel"), b"keep").unwrap();
    assert!(!command(&a, &b, &output, "existing")
        .output()
        .unwrap()
        .status
        .success());
    assert_eq!(fs::read(output.join("sentinel")).unwrap(), b"keep");
    fs::remove_file(b.join("tile.glb")).unwrap();
    let failed = temp.path().join("failed");
    assert!(!command(&a, &b, &failed, "missing")
        .output()
        .unwrap()
        .status
        .success());
    assert!(!failed.exists());
    assert!(!temp.path().join(".geoforge-task-missing").exists());
    assert_eq!(fs::read(b.join("tileset.json")).unwrap(), bytes);
}

#[test]
fn cli_cancel_during_copy_cleans_only_owned_work() {
    let temp = tempfile::tempdir().unwrap();
    let a = fixture(temp.path(), "a", 0.);
    let b = fixture(temp.path(), "b", 0.);
    let source = fs::File::create(a.join("large-resource.bin")).unwrap();
    source.set_len(256 * 1024 * 1024).unwrap();
    let output = temp.path().join("output");
    let mut child = command(&a, &b, &output, "cancel-copy")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut stdout = BufReader::new(child.stdout.take().unwrap());
    let mut line = String::new();
    loop {
        line.clear();
        assert!(
            stdout.read_line(&mut line).unwrap() > 0,
            "processor exited before merge"
        );
        let event: Value = serde_json::from_str(&line).unwrap();
        if event["type"] == "stage" && event["stage"] == "merge" {
            break;
        }
    }
    child.stdin.take().unwrap().write_all(b"cancel\n").unwrap();
    // Drain JSONL so the process cannot block on a full stdout pipe.
    for line in stdout.lines() {
        line.unwrap();
    }
    assert_eq!(child.wait().unwrap().code(), Some(2));
    assert!(!output.exists());
    assert!(!temp.path().join(".geoforge-task-cancel-copy").exists());
    assert_eq!(
        fs::metadata(a.join("large-resource.bin")).unwrap().len(),
        256 * 1024 * 1024
    );
}

#[cfg(unix)]
#[test]
fn cli_rejects_symlink_resources_without_reading_outside_input() {
    let temp = tempfile::tempdir().unwrap();
    let a = fixture(temp.path(), "a", 0.);
    let b = fixture(temp.path(), "b", 0.);
    fs::write(temp.path().join("outside"), b"private").unwrap();
    std::os::unix::fs::symlink(temp.path().join("outside"), a.join("link")).unwrap();
    let output = temp.path().join("output");
    assert!(!command(&a, &b, &output, "symlink")
        .output()
        .unwrap()
        .status
        .success());
    assert!(!output.exists());
}

#[test]
fn cli_preserves_nonstandard_entry_names_and_explicit_11_version() {
    let temp = tempfile::tempdir().unwrap();
    let a = fixture(temp.path(), "a", 0.);
    let b = fixture(temp.path(), "b", 10.);
    let named = a.join("entry.json");
    fs::rename(a.join("tileset.json"), &named).unwrap();
    let mut value: Value = serde_json::from_slice(&fs::read(&named).unwrap()).unwrap();
    value["asset"]["version"] = json!("1.1");
    fs::write(&named, serde_json::to_vec(&value).unwrap()).unwrap();
    let output = temp.path().join("merged");
    let result = command(&named, &b, &output, "named-entry")
        .output()
        .unwrap();
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let merged: Value =
        serde_json::from_slice(&fs::read(output.join("tileset.json")).unwrap()).unwrap();
    assert_eq!(merged["asset"]["version"], "1.1");
    assert_eq!(
        merged["root"]["children"][0]["content"]["uri"],
        "sources/source-001/entry.json"
    );
    assert!(processor::validate_tileset_tree(&output).ok);
}
