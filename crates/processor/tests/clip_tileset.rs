use serde_json::{json, Value};
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::Path;
use std::process::{Command, Stdio};

fn glb() -> Vec<u8> {
    let positions = [-100_f32, 0., 100., 100., 0., 100., 100., 0., -100.];
    let normals = [0_f32, 1., 0., 0., 1., 0., 0., 1., 0.];
    let uvs = [0_f32, 0., 1., 0., 1., 1.];
    let mut bin = Vec::new();
    for values in [&positions[..], &normals[..], &uvs[..]] {
        for n in values {
            bin.extend_from_slice(&n.to_le_bytes());
        }
    }
    let root = json!({"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],"nodes":[{"mesh":0}],
        "meshes":[{"primitives":[{"attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2},"material":0}]}],
        "materials":[{"doubleSided":true,"pbrMetallicRoughness":{"baseColorFactor":[0.8,0.2,0.1,1.],"metallicFactor":0,"roughnessFactor":1}}],
        "buffers":[{"byteLength":bin.len()}],"bufferViews":[{"buffer":0,"byteOffset":0,"byteLength":36},{"buffer":0,"byteOffset":36,"byteLength":36},{"buffer":0,"byteOffset":72,"byteLength":24}],
        "accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3","min":[-100,0,-100],"max":[100,0,100]},
            {"bufferView":1,"componentType":5126,"count":3,"type":"VEC3"},{"bufferView":2,"componentType":5126,"count":3,"type":"VEC2"}]});
    pack(&root, &bin)
}
fn pack(root: &Value, bin: &[u8]) -> Vec<u8> {
    let mut text = serde_json::to_vec(root).unwrap();
    while text.len() % 4 != 0 {
        text.push(b' ');
    }
    let mut out = b"glTF".to_vec();
    for n in [
        2,
        (28 + text.len() + bin.len()) as u32,
        text.len() as u32,
        0x4e4f534a,
    ] {
        out.extend_from_slice(&n.to_le_bytes());
    }
    out.extend_from_slice(&text);
    out.extend_from_slice(&(bin.len() as u32).to_le_bytes());
    out.extend_from_slice(&0x004e4942_u32.to_le_bytes());
    out.extend_from_slice(bin);
    out
}
fn unpack(bytes: &[u8]) -> (Value, Vec<u8>) {
    let bytes = if bytes.starts_with(b"b3dm") {
        let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
        &bytes[28 + len..]
    } else {
        bytes
    };
    let len = u32::from_le_bytes(bytes[12..16].try_into().unwrap()) as usize;
    (
        serde_json::from_slice(&bytes[20..20 + len]).unwrap(),
        bytes[28 + len..].to_vec(),
    )
}
fn attribute(
    doc: &Value,
    bin: &[u8],
    primitive: &Value,
    name: &str,
    width: usize,
) -> Vec<Vec<f64>> {
    let id = primitive["attributes"][name].as_u64().unwrap() as usize;
    let a = &doc["accessors"][id];
    let v = &doc["bufferViews"][a["bufferView"].as_u64().unwrap() as usize];
    let offset = v["byteOffset"].as_u64().unwrap() as usize;
    (0..a["count"].as_u64().unwrap() as usize)
        .map(|i| {
            (0..width)
                .map(|c| {
                    f32::from_le_bytes(
                        bin[offset + (i * width + c) * 4..offset + (i * width + c + 1) * 4]
                            .try_into()
                            .unwrap(),
                    ) as f64
                })
                .collect()
        })
        .collect()
}
fn fixture(root: &Path) -> std::path::PathBuf {
    let input = root.join("地理模型");
    fs::create_dir(&input).unwrap();
    fs::write(input.join("tile.glb"), glb()).unwrap();
    fs::write(input.join("tileset.json"),serde_json::to_vec(&json!({"asset":{"version":"1.0"},"geometricError":10,
        "root":{"transform":[0,1,0,0,0,0,1,0,1,0,0,0,6378147,0,0,1],"refine":"REPLACE","geometricError":0,"boundingVolume":{"box":[0,0,0,100,0,0,0,100,0,0,0,1]},"content":{"uri":"tile.glb"}}})).unwrap()).unwrap();
    input
}
fn config(input: &Path, output: &Path) -> Value {
    json!({"schemaVersion":1,"taskId":"clip-test","operation":"clip-tileset","input":{"path":input},"output":{"path":output},"options":{"clip":{"region":{"type":"rectangle","bounds":[-0.0005,-0.0005,0.0005,0.0005]}}}})
}
fn run(root: &Path, config: Value) -> std::process::Output {
    let task = root.join("task.json");
    fs::write(&task, serde_json::to_vec(&config).unwrap()).unwrap();
    Command::new(env!("CARGO_BIN_EXE_processor"))
        .args(["run", "--task"])
        .arg(task)
        .output()
        .unwrap()
}
fn json_file(path: &Path) -> Value {
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}

#[test]
fn exports_real_clipped_geometry_and_interpolated_attributes_portably() {
    let temp = tempfile::tempdir().unwrap();
    let input = fixture(temp.path());
    let output = temp.path().join("成果");
    let original = fs::read(input.join("tile.glb")).unwrap();
    let result = run(temp.path(), config(&input, &output));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let tileset = json_file(&output.join("tileset.json"));
    let path = output.join(tileset["root"]["content"]["uri"].as_str().unwrap());
    let (doc, bin) = unpack(&fs::read(path).unwrap());
    let p = &doc["meshes"][0]["primitives"][0];
    let vertices = attribute(&doc, &bin, p, "POSITION", 3);
    let normals = attribute(&doc, &bin, p, "NORMAL", 3);
    let uvs = attribute(&doc, &bin, p, "TEXCOORD_0", 2);
    assert!(vertices.len() > 3);
    assert!(vertices
        .iter()
        .all(|p| p[0].abs() < 55.67 && p[2].abs() < 55.30));
    let mut area = 0.;
    for tri in vertices.chunks_exact(3) {
        area += ((tri[1][0] - tri[0][0]) * (tri[2][2] - tri[0][2])
            - (tri[1][2] - tri[0][2]) * (tri[2][0] - tri[0][0]))
            .abs()
            * 0.5;
    }
    assert!(area > 6000. && area < 6200., "clipped area={area}");
    for i in 0..vertices.len() {
        assert!((uvs[i][0] - (vertices[i][0] + 100.) / 200.).abs() < 1e-6);
        assert!((uvs[i][1] - (100. - vertices[i][2]) / 200.).abs() < 1e-6);
        assert_eq!(normals[i], vec![0., 1., 0.]);
    }
    let (original_doc, _) = unpack(&original);
    assert_eq!(doc["materials"], original_doc["materials"]);
    assert_eq!(fs::read(input.join("tile.glb")).unwrap(), original);
    assert!(!output.join("tile.glb").exists());
    fs::remove_dir_all(&input).unwrap();
    assert!(processor::validate_tileset_tree(&output).ok);
    let report = json_file(&output.join("clip-report.json"));
    assert_eq!(report["trianglesBefore"], 1);
    assert_eq!(report["capsGenerated"], false);
}
#[test]
fn clips_all_lods_external_roots_and_node_transforms() {
    let temp = tempfile::tempdir().unwrap();
    let input = fixture(temp.path());
    let output = temp.path().join("output");
    let mut top = json_file(&input.join("tileset.json"));
    let root = top["root"].clone();
    let mut child = root.clone();
    child.as_object_mut().unwrap().remove("transform");
    child["content"] = json!({"uri":"nested/tileset.json"});
    child["geometricError"] = json!(5);
    top["root"]["children"] = json!([child]);
    top["root"]["geometricError"] = json!(10);
    fs::create_dir(input.join("nested")).unwrap();
    let mut nested = json!({"asset":{"version":"1.0"},"geometricError":5,"root":root});
    nested["root"].as_object_mut().unwrap().remove("transform");
    nested["root"]["content"]["uri"] = json!("../tile.glb");
    fs::write(
        input.join("nested/tileset.json"),
        serde_json::to_vec(&nested).unwrap(),
    )
    .unwrap();
    fs::write(
        input.join("tileset.json"),
        serde_json::to_vec(&top).unwrap(),
    )
    .unwrap();
    let result = run(temp.path(), config(&input, &output));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let root = json_file(&output.join("tileset.json"));
    let nested = json_file(
        &output.join(
            root["root"]["children"][0]["content"]["uri"]
                .as_str()
                .unwrap(),
        ),
    );
    assert!(nested["root"]["content"]["uri"]
        .as_str()
        .unwrap()
        .starts_with("../content/"));
    assert_eq!(
        json_file(&output.join("clip-report.json"))["contentsProcessed"],
        2
    );
    assert!(processor::validate_tileset_tree(&output).ok);
}
#[test]
fn b3dm_rtc_and_external_image_dependencies_survive() {
    let temp = tempfile::tempdir().unwrap();
    let input = fixture(temp.path());
    let output = temp.path().join("output");
    let (mut doc, bin) = unpack(&glb());
    doc["nodes"][0]["translation"] = json!([-10, 0, 0]);
    doc["images"] = json!([{"uri":"image.png"}]);
    doc["textures"] = json!([{"source":0}]);
    doc["materials"][0]["pbrMetallicRoughness"]["baseColorTexture"] = json!({"index":0});
    let image_bytes: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 120, 156, 99, 248, 207, 192, 240,
        31, 0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];
    fs::write(input.join("image.png"), image_bytes).unwrap();
    let payload = pack(&doc, &bin);
    let mut ft = serde_json::to_vec(&json!({"BATCH_LENGTH":0,"RTC_CENTER":[10,0,0]})).unwrap();
    while (28 + ft.len()) % 8 != 0 {
        ft.push(b' ');
    }
    let mut bytes = b"b3dm".to_vec();
    for n in [
        1,
        (28 + ft.len() + payload.len()) as u32,
        ft.len() as u32,
        0,
        0,
        0,
    ] {
        bytes.extend_from_slice(&n.to_le_bytes());
    }
    bytes.extend(ft);
    bytes.extend(payload);
    while bytes.len() % 8 != 0 {
        bytes.push(0);
    }
    let total_length = bytes.len() as u32;
    bytes[8..12].copy_from_slice(&total_length.to_le_bytes());
    fs::write(input.join("tile.b3dm"), bytes).unwrap();
    let mut top = json_file(&input.join("tileset.json"));
    top["root"]["content"]["uri"] = json!("tile.b3dm");
    fs::write(
        input.join("tileset.json"),
        serde_json::to_vec(&top).unwrap(),
    )
    .unwrap();
    let result = run(temp.path(), config(&input, &output));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let top = json_file(&output.join("tileset.json"));
    let model = output.join(top["root"]["content"]["uri"].as_str().unwrap());
    let output_bytes = fs::read(&model).unwrap();
    assert_eq!(output_bytes.len() % 8, 0);
    let (doc, _) = unpack(&output_bytes);
    assert_eq!(doc["nodes"][0]["translation"], json!([-10, 0, 0]));
    let image = model
        .parent()
        .unwrap()
        .join(doc["images"][0]["uri"].as_str().unwrap());
    assert_eq!(fs::read(image).unwrap(), image_bytes);
    assert!(processor::validate_tileset_tree(&output).ok);
}
#[test]
fn failure_and_empty_results_never_modify_inputs_or_existing_output() {
    let temp = tempfile::tempdir().unwrap();
    let input = fixture(temp.path());
    let original = fs::read(input.join("tileset.json")).unwrap();
    let output = temp.path().join("output");
    fs::create_dir(&output).unwrap();
    fs::write(output.join("sentinel"), b"keep").unwrap();
    assert!(!run(temp.path(), config(&input, &output)).status.success());
    assert_eq!(fs::read(output.join("sentinel")).unwrap(), b"keep");
    let mut empty = config(&input, &temp.path().join("empty"));
    empty["options"]["clip"]["region"]["bounds"] = json!([0.005, 0.005, 0.006, 0.006]);
    assert!(!run(temp.path(), empty).status.success());
    assert!(!temp.path().join("empty").exists());
    let nested = input.join("new/output");
    assert!(!run(temp.path(), config(&input, &nested)).status.success());
    assert!(!input.join("new").exists());
    let (mut doc, bin) = unpack(&glb());
    doc["meshes"][0]["primitives"][0]["attributes"]["_BATCHID"] = json!(0);
    fs::write(input.join("tile.glb"), pack(&doc, &bin)).unwrap();
    assert!(!run(
        temp.path(),
        config(&input, &temp.path().join("unsupported"))
    )
    .status
    .success());
    assert_eq!(fs::read(input.join("tileset.json")).unwrap(), original);
    assert!(!temp.path().join(".geoforge-task-clip-test").exists());
}
#[test]
fn indexed_strided_attributes_and_normalized_colors_are_interpolated() {
    let temp = tempfile::tempdir().unwrap();
    let input = fixture(temp.path());
    let output = temp.path().join("indexed");
    let (mut doc, mut bin) = unpack(&glb());
    let index_offset = bin.len();
    for n in [0_u16, 1, 2] {
        bin.extend_from_slice(&n.to_le_bytes());
    }
    while bin.len() % 4 != 0 {
        bin.push(0);
    }
    let color_offset = bin.len();
    bin.extend_from_slice(&[0, 0, 255, 255, 255, 0, 255, 255, 255, 255, 255, 255]);
    doc["bufferViews"].as_array_mut().unwrap().extend([
        json!({"buffer":0,"byteOffset":index_offset,"byteLength":6}),
        json!({"buffer":0,"byteOffset":color_offset,"byteLength":12,"byteStride":4}),
    ]);
    doc["accessors"].as_array_mut().unwrap().extend([
        json!({"bufferView":3,"componentType":5123,"count":3,"type":"SCALAR"}),
        json!({"bufferView":4,"componentType":5121,"count":3,"type":"VEC4","normalized":true}),
    ]);
    doc["meshes"][0]["primitives"][0]["indices"] = json!(3);
    doc["meshes"][0]["primitives"][0]["attributes"]["COLOR_0"] = json!(4);
    doc["buffers"][0]["byteLength"] = json!(bin.len());
    fs::write(input.join("tile.glb"), pack(&doc, &bin)).unwrap();
    let result = run(temp.path(), config(&input, &output));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let top = json_file(&output.join("tileset.json"));
    let (doc, bin) =
        unpack(&fs::read(output.join(top["root"]["content"]["uri"].as_str().unwrap())).unwrap());
    let p = &doc["meshes"][0]["primitives"][0];
    let colors = attribute(&doc, &bin, p, "COLOR_0", 4);
    let uvs = attribute(&doc, &bin, p, "TEXCOORD_0", 2);
    for (color, uv) in colors.iter().zip(uvs) {
        assert!((color[0] - uv[0]).abs() < 1e-6);
        assert!((color[1] - uv[1]).abs() < 1e-6);
        assert_eq!(color[2], 1.);
        assert_eq!(color[3], 1.);
    }
}
#[test]
fn shared_mesh_instances_are_cropped_in_each_node_frame() {
    let temp = tempfile::tempdir().unwrap();
    let input = fixture(temp.path());
    let output = temp.path().join("instances");
    let (mut doc, bin) = unpack(&glb());
    doc["nodes"] = json!([{"children":[1,2],"translation":[10,0,0]},{"mesh":0,"translation":[-10,0,0]},{"mesh":0,"translation":[-30,0,0]}]);
    fs::write(input.join("tile.glb"), pack(&doc, &bin)).unwrap();
    let result = run(temp.path(), config(&input, &output));
    assert!(
        result.status.success(),
        "{}",
        String::from_utf8_lossy(&result.stdout)
    );
    let top = json_file(&output.join("tileset.json"));
    let (doc, bin) =
        unpack(&fs::read(output.join(top["root"]["content"]["uri"].as_str().unwrap())).unwrap());
    assert_eq!(doc["meshes"].as_array().unwrap().len(), 2);
    for (i, shift) in [0., -20.].iter().enumerate() {
        let p = &doc["meshes"][i]["primitives"][0];
        for v in attribute(&doc, &bin, p, "POSITION", 3) {
            assert!((v[0] + shift).abs() < 55.68);
            assert!(v[2].abs() < 55.30);
        }
    }
}
#[test]
fn invalid_indices_cycles_and_missing_images_fail_cleanly() {
    let temp = tempfile::tempdir().unwrap();
    let input = fixture(temp.path());
    let output = temp.path().join("failure");
    let (mut doc, bin) = unpack(&glb());
    doc["accessors"][0]["byteOffset"] = json!(1_000_000);
    fs::write(input.join("tile.glb"), pack(&doc, &bin)).unwrap();
    assert!(!run(temp.path(), config(&input, &output)).status.success());
    let (mut doc, bin) = unpack(&glb());
    doc["images"] = json!([{"uri":"missing.png"}]);
    fs::write(input.join("tile.glb"), pack(&doc, &bin)).unwrap();
    assert!(!run(temp.path(), config(&input, &output)).status.success());
    fs::write(input.join("tile.glb"), glb()).unwrap();
    let mut top = json_file(&input.join("tileset.json"));
    top["root"]["content"] = json!({"uri":"tileset.json"});
    fs::write(
        input.join("tileset.json"),
        serde_json::to_vec(&top).unwrap(),
    )
    .unwrap();
    assert!(!run(temp.path(), config(&input, &output)).status.success());
    assert!(!output.exists());
    assert!(!temp.path().join(".geoforge-task-clip-test").exists());
}
#[test]
fn stdin_cancel_before_geometry_commits_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let input = fixture(temp.path());
    // Keep enough work available so cancellation does not race a one-triangle commit on fast CI.
    let (mut document, binary) = unpack(&glb());
    document["nodes"] = json!((0..1000).map(|_| json!({"mesh":0})).collect::<Vec<_>>());
    document["scenes"][0]["nodes"] = json!((0..1000).collect::<Vec<_>>());
    fs::write(input.join("tile.glb"), pack(&document, &binary)).unwrap();
    let output = temp.path().join("output");
    let task = temp.path().join("task.json");
    fs::write(&task, serde_json::to_vec(&config(&input, &output)).unwrap()).unwrap();
    let mut child = Command::new(env!("CARGO_BIN_EXE_processor"))
        .args(["run", "--task"])
        .arg(&task)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"cancel\n").unwrap();
    let reader = BufReader::new(child.stdout.take().unwrap());
    for line in reader.lines() {
        line.unwrap();
    }
    assert_eq!(child.wait().unwrap().code(), Some(2));
    assert!(!output.exists());
    assert!(!temp.path().join(".geoforge-task-clip-test").exists());
}
