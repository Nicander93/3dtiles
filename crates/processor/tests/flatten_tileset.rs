use serde_json::{json, Value};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};

fn fixture(root: &Path) -> PathBuf {
    let input = root.join("source");
    fs::create_dir(&input).unwrap();
    let mut binary = Vec::new();
    for n in [
        -100_f32, 0., 100., 100., 0., 100., 100., 0., -100., 0., 1., 0., 0., 1., 0., 0., 1., 0.,
        0., 0., 1., 0., 1., 1.,
    ] {
        binary.extend_from_slice(&n.to_le_bytes());
    }
    let doc = json!({"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2},"material":0}]}],
        "materials":[{"doubleSided":true,"pbrMetallicRoughness":{"baseColorFactor":[0.7,0.2,0.1,1]}}],"buffers":[{"byteLength":binary.len()}],
        "bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":36},{"buffer":0,"byteOffset":72,"byteLength":24}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[-100,0,-100],"max":[100,0,100]},{"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"},{"bufferView":2,"componentType":5126,"count":3,"type":"VEC2"}]});
    let mut text = serde_json::to_vec(&doc).unwrap();
    while text.len() % 4 != 0 {
        text.push(b' ');
    }
    let mut glb = b"glTF".to_vec();
    for n in [
        2,
        (28 + text.len() + binary.len()) as u32,
        text.len() as u32,
        0x4e4f534a,
    ] {
        glb.extend_from_slice(&n.to_le_bytes());
    }
    glb.extend(text);
    glb.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    glb.extend_from_slice(&0x004e4942_u32.to_le_bytes());
    glb.extend(binary);
    fs::write(input.join("tile.glb"), glb).unwrap();
    fs::write(input.join("tileset.json"),serde_json::to_vec(&json!({"asset":{"version":"1.1"},"geometricError":10,"root":{"transform":[0,1,0,0,0,0,1,0,1,0,0,0,6378147,0,0,1],"refine":"REPLACE","geometricError":0,"boundingVolume":{"box":[0,0,0,100,0,0,0,100,0,0,0,1]},"content":{"uri":"tile.glb"}}})).unwrap()).unwrap();
    input
}
fn config(input: &Path, output: &Path, height: f64) -> Value {
    json!({"schemaVersion":1,"taskId":"flatten-test","operation":"flatten-tileset","input":{"path":input},"output":{"path":output},"options":{"flatten":{"region":{"type":"rectangle","bounds":[-0.0002,-0.0002,0.0002,0.0002]},"heightMeters":height}}})
}
fn run(root: &Path, config: &Value) -> std::process::Output {
    let file = root.join("task.json");
    fs::write(&file, serde_json::to_vec(config).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_processor"))
        .args(["run", "--task"])
        .arg(file)
        .output()
        .unwrap()
}
fn read(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
fn attributes(output: &Path, uri: &str, name: &str, width: usize) -> Vec<Vec<f64>> {
    let bytes = fs::read(output.join(uri)).unwrap();
    let length = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    let doc: Value = serde_json::from_slice(&bytes[20..20 + length]).unwrap();
    let binary = &bytes[28 + length..];
    let id = doc["meshes"][0]["primitives"][0]["attributes"][name]
        .as_u64()
        .unwrap() as usize;
    let a = &doc["accessors"][id];
    let view = &doc["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
    let offset = view["byteOffset"].as_u64().unwrap() as usize;
    (0..a["count"].as_u64().unwrap() as usize)
        .map(|i| {
            (0..width)
                .map(|j| {
                    f32::from_le_bytes(
                        binary[offset + (i * width + j) * 4..offset + (i * width + j + 1) * 4]
                            .try_into()
                            .unwrap(),
                    ) as f64
                })
                .collect()
        })
        .collect()
}

#[test]
fn flatten_preserves_outside_and_exports_flat_surface_with_boundary_walls_and_normals() {
    for height in [-5., 25.] {
        let root = tempfile::tempdir().unwrap();
        let input = fixture(root.path());
        let output = root.path().join("flattened");
        let original = fs::read(input.join("tile.glb")).unwrap();
        let result = run(root.path(), &config(&input, &output, height));
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stdout)
        );
        let tileset = read(&output.join("tileset.json"));
        let uri = tileset["root"]["content"]["uri"].as_str().unwrap();
        let p = attributes(&output, uri, "POSITION", 3);
        let normals = attributes(&output, uri, "NORMAL", 3);
        assert!(p.iter().any(|p| p[0].abs() > 90. && p[1].abs() < 1e-6));
        assert!(p.iter().any(|p| (p[1] - (height - 10.)).abs() < 0.02));
        assert!(p.iter().all(|p| {
            let outside = p[0].abs() > 22.27 || p[2].abs() > 22.12;
            !outside || p[1].abs() < 1e-5
        }));
        assert!(normals
            .iter()
            .all(|n| (n.iter().map(|v| v * v).sum::<f64>() - 1.).abs() < 1e-5));
        assert!(normals.iter().any(|n| n[1].abs() < 0.01));
        assert_eq!(attributes(&output, uri, "TEXCOORD_0", 2).len(), p.len());
        let report = read(&output.join("flatten-report.json"));
        assert_eq!(report["trianglesBefore"], 1);
        assert!(report["trianglesFlattened"].as_u64().unwrap() > 0);
        assert!(report["wallTriangles"].as_u64().unwrap() > 0);
        assert_eq!(fs::read(input.join("tile.glb")).unwrap(), original);
        assert!(
            tileset["root"]["boundingVolume"]["box"][2]
                .as_f64()
                .unwrap()
                .abs()
                > 1.
        );
    }
}
#[test]
fn flatten_processes_every_lod_and_nested_external_content_portably() {
    let root = tempfile::tempdir().unwrap();
    let input = fixture(root.path());
    let output = root.path().join("out");
    let mut inner = read(&input.join("tileset.json"));
    inner["root"].as_object_mut().unwrap().remove("transform");
    fs::write(
        input.join("nested.json"),
        serde_json::to_vec(&inner).unwrap(),
    )
    .unwrap();
    let mut outer = read(&input.join("tileset.json"));
    outer["root"]["children"] = json!([{"refine":"REPLACE","geometricError":0,"boundingVolume":{"box":[0,0,0,100,0,0,0,100,0,0,0,1]},"content":{"uri":"nested.json"}}]);
    fs::write(
        input.join("tileset.json"),
        serde_json::to_vec(&outer).unwrap(),
    )
    .unwrap();
    let result = run(root.path(), &config(&input, &output, 0.));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    assert_eq!(
        read(&output.join("flatten-report.json"))["contentsProcessed"],
        2
    );
    let entry = read(&output.join("tileset.json"));
    let external = read(
        &output.join(
            entry["root"]["children"][0]["content"]["uri"]
                .as_str()
                .unwrap(),
        ),
    );
    let uri = external["root"]["content"]["uri"].as_str().unwrap();
    assert!(output.join("content").join(uri).is_file());
    fs::remove_dir_all(input).unwrap();
    assert!(output
        .join(entry["root"]["content"]["uri"].as_str().unwrap())
        .is_file());
}
#[test]
fn invalid_or_empty_flatten_region_and_existing_output_leave_no_partial_result() {
    let root = tempfile::tempdir().unwrap();
    let input = fixture(root.path());
    let output = root.path().join("out");
    let mut bad = config(&input, &output, 10001.);
    assert!(!run(root.path(), &bad).status.success());
    assert!(!output.exists());
    bad = config(&input, &output, 0.);
    bad["options"]["flatten"]["region"]["bounds"] = json!([0.005, 0.005, 0.006, 0.006]);
    let result = run(root.path(), &bad);
    assert!(!result.status.success());
    assert!(String::from_utf8_lossy(&result.stdout).contains("does not intersect"));
    assert!(!output.exists());
    fs::create_dir(&output).unwrap();
    fs::write(output.join("sentinel"), b"keep").unwrap();
    assert!(!run(root.path(), &config(&input, &output, 0.))
        .status
        .success());
    assert_eq!(fs::read(output.join("sentinel")).unwrap(), b"keep");
    let nested = input.join("not-created").join("out");
    assert!(!run(root.path(), &config(&input, &nested, 0.))
        .status
        .success());
    assert!(!input.join("not-created").exists());
}

#[test]
fn flatten_in_nonzero_geographic_frame_handles_scaled_rotated_nodes() {
    let root = tempfile::tempdir().unwrap();
    let input = fixture(root.path());
    let output = root.path().join("transformed");
    let (lon, lat) = (117_f64, 35_f64);
    let (sl, cl) = lon.to_radians().sin_cos();
    let (s, c) = lat.to_radians().sin_cos();
    let n = 6378137. / (1. - 6.6943799901413165e-3 * s * s).sqrt();
    let mut entry = read(&input.join("tileset.json"));
    entry["root"]["transform"] = json!([
        -sl,
        cl,
        0.,
        0.,
        -s * cl,
        -s * sl,
        c,
        0.,
        c * cl,
        c * sl,
        s,
        0.,
        (n + 10.) * c * cl,
        (n + 10.) * c * sl,
        (n * (1. - 6.6943799901413165e-3) + 10.) * s,
        1.
    ]);
    fs::write(
        input.join("tileset.json"),
        serde_json::to_vec(&entry).unwrap(),
    )
    .unwrap();
    let original = fs::read(input.join("tile.glb")).unwrap();
    let length = u32::from_le_bytes(original[12..16].try_into().unwrap()) as usize;
    let mut doc: Value = serde_json::from_slice(&original[20..20 + length]).unwrap();
    doc["nodes"][0]["scale"] = json!([2., 3., 0.5]);
    doc["nodes"][0]["translation"] = json!([5., 0., 7.]);
    doc["nodes"][0]["rotation"] = json!([
        0.,
        (std::f64::consts::PI / 8.).sin(),
        0.,
        (std::f64::consts::PI / 8.).cos()
    ]);
    let mut text = serde_json::to_vec(&doc).unwrap();
    while text.len() % 4 != 0 {
        text.push(b' ');
    }
    let binary = &original[28 + length..];
    let mut bytes = b"glTF".to_vec();
    for n in [
        2,
        (28 + text.len() + binary.len()) as u32,
        text.len() as u32,
        0x4e4f534a,
    ] {
        bytes.extend_from_slice(&n.to_le_bytes());
    }
    bytes.extend(text);
    bytes.extend_from_slice(&(binary.len() as u32).to_le_bytes());
    bytes.extend_from_slice(&0x004e4942_u32.to_le_bytes());
    bytes.extend(binary);
    fs::write(input.join("tile.glb"), bytes).unwrap();
    let mut cfg = config(&input, &output, 0.);
    cfg["options"]["flatten"]["region"]["bounds"] =
        json!([lon - 0.0001, lat - 0.0001, lon + 0.0001, lat + 0.0001]);
    let result = run(root.path(), &cfg);
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let entry = read(&output.join("tileset.json"));
    let p = attributes(
        &output,
        entry["root"]["content"]["uri"].as_str().unwrap(),
        "POSITION",
        3,
    );
    assert!(p.iter().any(|p| (p[1] * 3. + 10.).abs() < 0.02));
    assert!(p.iter().any(|p| p[1].abs() < 1e-6));
}
