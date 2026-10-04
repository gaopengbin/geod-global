use super::*;
use serde_json::json;
fn rights() -> Rights {
    Rights {
        license: "CC0 test fixture".into(),
        attribution: "Synthetic unit fixture".into(),
        license_url: None,
        permission_confirmed: true,
    }
}
fn local() -> LocalRequest {
    LocalRequest {
        name: "Fixture scene".into(),
        rights: rights(),
    }
}
fn tile(uri: &str) -> serde_json::Value {
    json!({"boundingVolume":{"sphere":[0,0,0,2]},"geometricError":0,"content":{"uri":uri}})
}
fn scene() -> serde_json::Value {
    json!({"asset":{"version":"1.1"},"geometricError":1,"root":tile("model.gltf")})
}
fn model() -> serde_json::Value {
    json!({"asset":{"version":"2.0"},"buffers":[{"uri":"mesh.bin","byteLength":12}],"bufferViews":[{"buffer":0,"byteLength":12}],"images":[{"uri":"tex.png"}]})
}
fn fixture(p: &Path) {
    std::fs::write(
        p.join("tileset.json"),
        serde_json::to_vec(&scene()).unwrap(),
    )
    .unwrap();
    std::fs::write(p.join("model.gltf"), serde_json::to_vec(&model()).unwrap()).unwrap();
    std::fs::write(p.join("mesh.bin"), [0; 12]).unwrap();
    let png=STANDARD.decode("iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAQAAAC1HAwCAAAAC0lEQVR42mP8/x8AAwMCAO+aRL8AAAAASUVORK5CYII=").unwrap();
    std::fs::write(p.join("tex.png"), png).unwrap();
}
#[tokio::test]
async fn complete_local_dependencies_survive_input_removal_and_restart() {
    let temp = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    fixture(input.path());
    let m = JobManager::open(temp.path()).await.unwrap();
    let p = m
        .open_three_d_path(input.path().join("tileset.json"), local())
        .await
        .unwrap();
    assert_eq!(p.resources.len(), 4);
    assert_eq!(p.tile_count, 1);
    let original: Vec<_> = p
        .resources
        .iter()
        .map(|r| read_verified(&m.inner.root, r).unwrap())
        .collect();
    drop(input);
    let zip = m.three_d_export(&p.id).await.unwrap();
    drop(m);
    let m = JobManager::open(temp.path()).await.unwrap();
    assert_eq!(
        m.inspect_three_d(&p.id).await.unwrap().receipt_sha256,
        p.receipt_sha256
    );
    for (r, b) in p.resources.iter().zip(original) {
        assert_eq!(read_verified(&m.inner.root, r).unwrap(), b);
    }
    let copy = m.import_three_d_archive(zip, local()).await.unwrap();
    assert_eq!(copy.resources.len(), 4);
    assert_eq!(copy.origin, "local-archive");
    assert_eq!(copy.tile_count, 1);
    assert_eq!(
        copy.imported_from.as_ref().unwrap().source_receipt_sha256,
        p.receipt_sha256
    );
    assert_eq!(copy.imported_from.as_ref().unwrap().source, p.source);
    assert_eq!(
        serde_json::to_value(&copy.resources).unwrap(),
        serde_json::to_value(&p.resources).unwrap()
    );
    let second = m
        .import_three_d_archive(m.three_d_export(&copy.id).await.unwrap(), local())
        .await
        .unwrap();
    assert_eq!(
        serde_json::to_value(&second.resources).unwrap(),
        serde_json::to_value(&p.resources).unwrap()
    );
    let history = second.imported_from.as_ref().unwrap();
    assert_eq!(history.source_receipt_sha256, copy.receipt_sha256);
    assert_eq!(
        history.previous.as_ref().unwrap().source_receipt_sha256,
        p.receipt_sha256
    );
    for r in &p.resources {
        assert_eq!(
            read_verified(&m.inner.root, r).unwrap(),
            read_verified(
                &m.inner.root,
                second.resources.iter().find(|x| x.id == r.id).unwrap()
            )
            .unwrap()
        );
    }
}
#[tokio::test]
async fn repeated_imports_keep_rights_history_and_reject_more_than_eight_levels() {
    let temp = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    fixture(input.path());
    let m = JobManager::open(temp.path()).await.unwrap();
    let original = m
        .open_three_d_path(input.path().join("tileset.json"), local())
        .await
        .unwrap();
    let mut p = original.clone();
    for _ in 0..8 {
        p = m
            .import_three_d_archive(m.three_d_export(&p.id).await.unwrap(), local())
            .await
            .unwrap();
    }
    assert!(m
        .import_three_d_archive(m.three_d_export(&p.id).await.unwrap(), local())
        .await
        .unwrap_err()
        .contains("overly deep"));
    assert_eq!(m.list_three_d().await.len(), 9);
    let mut leaf = p.imported_from.as_ref().unwrap();
    let mut count = 1;
    while let Some(previous) = &leaf.previous {
        leaf = previous;
        count += 1;
    }
    assert_eq!(count, 8);
    assert_eq!(leaf.source_receipt_sha256, original.receipt_sha256);
    assert_eq!(leaf.rights.license, original.rights.license);
}
fn altered_archive(bytes: Vec<u8>, edit: impl Fn(&str, Vec<u8>) -> Option<Vec<u8>>) -> Vec<u8> {
    let mut input = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
    let mut output = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for i in 0..input.len() {
        let mut file = input.by_index(i).unwrap();
        let name = file.name().to_owned();
        let mut b = Vec::new();
        file.read_to_end(&mut b).unwrap();
        if let Some(b) = edit(&name, b) {
            output
                .start_file(name, zip::write::SimpleFileOptions::default())
                .unwrap();
            output.write_all(&b).unwrap();
        }
    }
    output.finish().unwrap().into_inner()
}
#[tokio::test]
async fn archive_import_verifies_originals_localized_fields_and_complete_membership() {
    let storage = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    fixture(input.path());
    let m = JobManager::open(storage.path()).await.unwrap();
    let p = m
        .open_three_d_path(input.path().join("tileset.json"), local())
        .await
        .unwrap();
    let bytes = m.three_d_export(&p.id).await.unwrap();
    let bad = altered_archive(bytes.clone(), |name, b| {
        if name.starts_with("originals/") {
            Some(vec![0; b.len()])
        } else {
            Some(b)
        }
    });
    assert!(m
        .import_three_d_archive(bad, local())
        .await
        .unwrap_err()
        .contains("SHA-256"));
    let bad = altered_archive(bytes.clone(), |name, b| {
        if name.starts_with("scene/") && name.ends_with(".json") {
            let mut v: serde_json::Value = serde_json::from_slice(&b).unwrap();
            v["extras"] = json!({"changed":true});
            Some(serde_json::to_vec(&v).unwrap())
        } else {
            Some(b)
        }
    });
    assert!(m
        .import_three_d_archive(bad, local())
        .await
        .unwrap_err()
        .contains("differs"));
    let bad = altered_archive(bytes, |name, b| {
        if name.starts_with("originals/") {
            None
        } else {
            Some(b)
        }
    });
    assert!(m
        .import_three_d_archive(bad, local())
        .await
        .unwrap_err()
        .contains("missing"));
    assert_eq!(m.list_three_d().await.len(), 1);
}
#[tokio::test]
async fn discovery_pin_and_local_path_escapes_never_commit() {
    let storage = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    fixture(input.path());
    let result = collect(
        Input::Folder {
            root: std::fs::canonicalize(input.path()).unwrap(),
        },
        "tileset.json".into(),
        "Fixture".into(),
        rights(),
        "local-files",
        "tileset.json".into(),
        Some(&"a".repeat(64)),
    )
    .await;
    assert!(result.unwrap_err().contains("changed after discovery"));
    let mut v = scene();
    v["root"]["content"]["uri"] = json!("../outside.glb");
    std::fs::write(
        input.path().join("tileset.json"),
        serde_json::to_vec(&v).unwrap(),
    )
    .unwrap();
    let m = JobManager::open(storage.path()).await.unwrap();
    assert!(m
        .open_three_d_path(input.path().join("tileset.json"), local())
        .await
        .unwrap_err()
        .contains("escapes"));
    assert!(m.list_three_d().await.is_empty());
}
#[test]
fn saved_graph_rejects_unrelated_dependency_targets_even_with_a_new_receipt() {
    let storage = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    fixture(input.path());
    let runtime = tokio::runtime::Runtime::new().unwrap();
    runtime.block_on(async {
        let m = JobManager::open(storage.path()).await.unwrap();
        let mut p = m
            .open_three_d_path(input.path().join("tileset.json"), local())
            .await
            .unwrap();
        let entry = p.resources.iter_mut().find(|r| r.id == p.entry).unwrap();
        entry.links[0].reference.uri = "different.gltf".into();
        p.receipt_sha256 = receipt(&p).unwrap();
        assert!(validate(&p).unwrap_err().contains("target changed"));
    });
}
#[test]
fn html_credits_cannot_trigger_untracked_remote_requests() {
    for (mut v, is_gltf) in [(model(), true), (scene(), false)] {
        v["asset"]["copyright"] = json!("<img src=https://external.example.com/a.png>");
        assert!(
            format::analyze(&serde_json::to_vec(&v).unwrap(), Purpose::Content)
                .unwrap_err()
                .contains("plain text")
        );
        v["asset"].as_object_mut().unwrap().remove("copyright");
        if !is_gltf {
            v["asset"]["extras"] = json!({"cesium":{"credits":[{"html":"<img src=https://external.example.com/b.png>"}]}});
            assert!(format::analyze(&serde_json::to_vec(&v).unwrap(), Purpose::Content).is_err());
        }
    }
}
#[tokio::test]
async fn missing_texture_does_not_commit_partial_scene() {
    let temp = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    fixture(input.path());
    std::fs::remove_file(input.path().join("tex.png")).unwrap();
    let m = JobManager::open(temp.path()).await.unwrap();
    assert!(m
        .open_three_d_path(input.path().join("tileset.json"), local())
        .await
        .is_err());
    assert!(m.list_three_d().await.is_empty());
    assert_eq!(
        std::fs::read_dir(temp.path().join("three-d"))
            .unwrap()
            .count(),
        0
    );
}
#[tokio::test]
async fn truncated_external_buffer_and_cycles_are_rejected() {
    let temp = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    fixture(input.path());
    std::fs::write(input.path().join("mesh.bin"), [0; 11]).unwrap();
    let m = JobManager::open(temp.path()).await.unwrap();
    assert!(m
        .open_three_d_path(input.path().join("tileset.json"), local())
        .await
        .unwrap_err()
        .contains("shorter"));
    std::fs::write(
        input.path().join("tileset.json"),
        serde_json::to_vec(
            &json!({"asset":{"version":"1.0"},"geometricError":1,"root":tile("tileset.json")}),
        )
        .unwrap(),
    )
    .unwrap();
    assert!(m
        .open_three_d_path(input.path().join("tileset.json"), local())
        .await
        .unwrap_err()
        .contains("Cyclic"));
    assert!(m.list_three_d().await.is_empty());
}
#[tokio::test]
async fn tampered_resource_is_not_previewed_or_exported() {
    let temp = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    fixture(input.path());
    let m = JobManager::open(temp.path()).await.unwrap();
    let p = m
        .open_three_d_path(input.path().join("tileset.json"), local())
        .await
        .unwrap();
    let r = &p.resources[0];
    std::fs::write(resource_path(&m.inner.root, r).unwrap(), b"changed").unwrap();
    assert!(m.inspect_three_d(&p.id).await.is_err());
    assert!(m
        .read_three_d_resource(ResourceRequest {
            id: p.id.clone(),
            resource_id: r.id.clone()
        })
        .await
        .is_err());
    assert!(m.three_d_export(&p.id).await.is_err());
}
#[test]
fn explicit_transforms_metadata_and_multiple_contents_preserved() {
    let mut v = scene();
    v["root"]["transform"] =
        json!([1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 12.0, 34.0, 56.0, 1.0]);
    v["root"].as_object_mut().unwrap().remove("content");
    v["root"]["contents"] = json!([{"uri":"a.glb"},{"uri":"b.glb"}]);
    v["schemaUri"] = json!("schema.json");
    let b = serde_json::to_vec(&v).unwrap();
    let a = format::analyze(&b, Purpose::Content).unwrap();
    assert_eq!(a.references.len(), 3);
    let rewritten = format::rewrite(
        &b,
        a.kind,
        &[("/root/contents/0/uri".into(), "local.glb".into())],
    )
    .unwrap();
    let result: serde_json::Value = serde_json::from_slice(&rewritten).unwrap();
    assert_eq!(v["root"]["transform"], result["root"]["transform"]);
    assert_eq!(result["root"]["contents"][1]["uri"], "b.glb");
    assert_eq!(result["root"]["contents"][0]["uri"], "local.glb");
}
#[test]
fn reject_unsupported_tiling_extensions_and_fetch_fields() {
    for (key, value) in [
        ("implicitTiling", json!({})),
        ("transform", json!(vec![0; 16])),
    ] {
        let mut v = scene();
        v["root"][key] = value;
        assert!(format::analyze(&serde_json::to_vec(&v).unwrap(), Purpose::Content).is_err());
    }
    let mut v = scene();
    v["extensionsRequired"] = json!(["3DTILES_implicit_tiling"]);
    assert!(format::analyze(&serde_json::to_vec(&v).unwrap(), Purpose::Content).is_err());
    let mut v = model();
    v["extensions"] = json!({"unreviewed":{"uri":"https://evil.example.com/data"}});
    assert!(format::analyze(&serde_json::to_vec(&v).unwrap(), Purpose::Content).is_err());
}
#[test]
fn unsafe_origins_and_paths_are_rejected() {
    let root = safe_url("https://assets.example.com/scene/tileset.json").unwrap();
    for url in [
        "http://assets.example.com/a",
        "https://127.0.0.1/a",
        "https://assets.example.com/a?token=x",
    ] {
        assert!(safe_url(url).is_err());
    }
    assert!(remote_link(&root, "https://other.example.com/a", &root).is_err());
    for uri in [
        "../../outside.glb",
        "/absolute.glb",
        "C:/file.glb",
        "https://host.example.com/a",
        "%2e%2e/file",
        "a\\b.glb",
    ] {
        assert!(local_link("tileset.json", uri).is_err());
    }
    assert_eq!(
        local_link("sub/model.gltf", "../textures/a.png").unwrap(),
        "textures/a.png"
    );
}
fn glb(v: serde_json::Value, bin: &[u8]) -> Vec<u8> {
    let mut json = serde_json::to_vec(&v).unwrap();
    while !json.len().is_multiple_of(4) {
        json.push(b' ');
    }
    let mut b = b"glTF".to_vec();
    b.extend(2u32.to_le_bytes());
    b.extend(((20 + json.len() + 8 + bin.len()) as u32).to_le_bytes());
    b.extend((json.len() as u32).to_le_bytes());
    b.extend(0x4e4f534au32.to_le_bytes());
    b.extend(json);
    b.extend((bin.len() as u32).to_le_bytes());
    b.extend(0x004e4942u32.to_le_bytes());
    b.extend(bin);
    b
}
#[test]
fn glb_and_b3dm_rewrite_preserves_binary_and_tables() {
    let v = json!({"asset":{"version":"2.0"},"buffers":[{"byteLength":12}],"images":[{"uri":"tex.png"}]});
    let binary = [42; 12];
    let glb = glb(v, &binary);
    let mut table = serde_json::to_vec(&json!({"BATCH_LENGTH":0,"RTC_CENTER":[1,2,3]})).unwrap();
    while !(28 + table.len()).is_multiple_of(8) {
        table.push(b' ');
    }
    let mut b = b"b3dm".to_vec();
    b.extend(1u32.to_le_bytes());
    b.extend(((28 + table.len() + glb.len()) as u32).to_le_bytes());
    b.extend((table.len() as u32).to_le_bytes());
    b.extend([0; 12]);
    b.extend(table.clone());
    b.extend(glb.clone());
    for (bytes, kind) in [(&glb, Kind::Glb), (&b, Kind::B3dm)] {
        let a = format::analyze(bytes, Purpose::Content).unwrap();
        assert_eq!(a.kind, kind);
        let out = format::rewrite(
            bytes,
            kind,
            &[("/images/0/uri".into(), "longer/texture.png".into())],
        )
        .unwrap();
        let (v, _, _, n) = format::binary_document(&out, kind).unwrap();
        assert_eq!(n, Some(12));
        assert_eq!(v["images"][0]["uri"], "longer/texture.png");
        assert!(out.ends_with(&binary));
        format::analyze(&out, Purpose::Content).unwrap();
        if kind == Kind::B3dm {
            assert_eq!(&out[28..28 + table.len()], &table);
        }
    }
    let mut bad = glb;
    bad[8..12].copy_from_slice(&1u32.to_le_bytes());
    assert!(format::analyze(&bad, Purpose::Content).is_err());
}
#[test]
fn rights_and_inline_data_are_validated() {
    let mut r = rights();
    r.permission_confirmed = false;
    assert!(r.validate().is_err());
    let v = json!({"asset":{"version":"2.0"},"buffers":[{"uri":"data:application/octet-stream;base64,AA==","byteLength":2}]});
    assert!(format::analyze(&serde_json::to_vec(&v).unwrap(), Purpose::Content).is_err());
}
