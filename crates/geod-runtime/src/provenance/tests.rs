use super::*;

fn original(root: &Path) -> Job {
    let mut job=crate::new_download_job(crate::CreateJobRequest{item_id:"S2A_10SEG_20250627_0_L2A".into(),asset_key:"scl".into(),href:"https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/10/S/EG/2025/6/S2A_10SEG_20250627_0_L2A/SCL.tif".into(),media_type:"image/tiff".into(),title:None});
    job.status = JobStatus::Succeeded;
    let bytes = b"II*\0provenance-transport-fixture";
    job.sha256 = Some(digest(bytes));
    job.bytes_downloaded = bytes.len() as u64;
    job.total_bytes = Some(job.bytes_downloaded);
    let path = root.join("assets").join(format!("{}.tif", job.id));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    job.output_path = Some(path.to_string_lossy().into());
    job
}

#[test]
fn private_fields_and_signed_urls_do_not_leave_the_workstation_in_context() {
    let mut value = json!({"assets":{"data":{"href":"https://source.example/data.tif?sv=x&sig=secret&sp=r"}},"authorization":"Bearer private","outputPath":"C:/private/name.tif","properties":{"proj:epsg":32610,"bands":["red"],"note":"public metadata"}});
    let mut n = 0;
    sanitize(&mut value, &mut n);
    let encoded = value.to_string();
    assert!(!encoded.contains("secret") && !encoded.contains("private"));
    assert_eq!(value["properties"]["proj:epsg"], 32610);
    assert_eq!(
        value["assets"]["data"]["href"],
        "https://source.example/data.tif"
    );
    assert_eq!(n, 3);
}

#[test]
fn processing_graph_preserves_exact_source_checksums_and_rejects_cycles_or_rebinding() {
    let dir = tempfile::tempdir().unwrap();
    let source = original(dir.path());
    let mut clip = source.clone();
    clip.id = uuid::Uuid::new_v4().to_string();
    clip.kind = "raster_clip".into();
    clip.parent_id = Some(source.id.clone());
    clip.recipe = Some(crate::RasterRecipe {
        schema_version: crate::processing::RECIPE_SCHEMA_VERSION.into(),
        name: "clip".into(),
        source: crate::processing::RecipeSource {
            job_id: source.id.clone(),
            sha256: source.sha256.clone().unwrap(),
        },
        operation: crate::processing::ClipOperation {
            operation_type: "clip".into(),
            crs: "EPSG:32610".into(),
            bounds: [1., 2., 3., 4.],
            geometry: None,
        },
        output: crate::processing::RecipeOutput {
            format: "GeoTIFF".into(),
        },
    });
    let mut jobs = BTreeMap::from([
        (source.id.clone(), source.clone()),
        (clip.id.clone(), clip.clone()),
    ]);
    let graph = ordered_graph(&jobs, &clip.id).unwrap();
    assert_eq!(
        graph.iter().map(|j| j.id.clone()).collect::<Vec<_>>(),
        vec![source.id.clone(), clip.id.clone()]
    );
    jobs.get_mut(&source.id).unwrap().sha256 = Some("a".repeat(64));
    assert!(ordered_graph(&jobs, &clip.id).is_err());
    jobs.insert(source.id.clone(), source.clone());
    jobs.get_mut(&source.id).unwrap().parent_id = Some(clip.id.clone());
    assert!(ordered_graph(&jobs, &clip.id).is_err());
}

#[test]
fn renamed_raster_and_json_export_is_byte_exact_and_preserves_existing_files() {
    let dir = tempfile::tempdir().unwrap();
    let job = original(dir.path());
    let export = tempfile::tempdir().unwrap();
    let target = export.path().join("北京 Sentinel-2.tif");
    let metadata = json!({"schemaVersion":"geod-raster-provenance/v1","output":{"file":format!("{}.tif",job.id),"sha256":job.sha256,"bytes":job.bytes_downloaded},"sources":[{"itemId":job.item_id,"href":job.href}],"processingSteps":[{"operation":"clip","localResampling":{"applied":false}}]});
    let bytes = serde_json::to_vec(&metadata).unwrap();
    export_pair(
        Path::new(job.output_path.as_ref().unwrap()),
        &job,
        bytes.clone(),
        &target,
    )
    .unwrap();
    assert_eq!(
        std::fs::read(&target).unwrap(),
        std::fs::read(job.output_path.as_ref().unwrap()).unwrap()
    );
    let sidecar = export.path().join("北京 Sentinel-2.tif.provenance.json");
    let copied: Value = serde_json::from_slice(&std::fs::read(&sidecar).unwrap()).unwrap();
    assert_eq!(copied["output"]["file"], "北京 Sentinel-2.tif");
    assert_eq!(copied["sources"], metadata["sources"]);
    assert!(!copied.to_string().contains(dir.path().to_str().unwrap()));
    assert!(export_pair(
        Path::new(job.output_path.as_ref().unwrap()),
        &job,
        bytes,
        &target
    )
    .is_err());
    std::fs::write(job.output_path.as_ref().unwrap(), b"changed").unwrap();
    let second = export.path().join("different.tif");
    assert!(export_pair(
        Path::new(job.output_path.as_ref().unwrap()),
        &job,
        serde_json::to_vec(&metadata).unwrap(),
        &second
    )
    .is_err());
    assert!(!second.exists());
}

#[test]
fn prepared_json_tampering_and_output_rebinding_are_not_silently_exported() {
    let dir = tempfile::tempdir().unwrap();
    let job = original(dir.path());
    let graph = vec![job.clone()];
    let hash = graph_hash(&graph).unwrap();
    let doc = json!({"graphSha256":hash,"output":{"sha256":job.sha256,"bytes":job.bytes_downloaded},"sources":[]});
    let bytes = serde_json::to_vec(&doc).unwrap();
    std::fs::create_dir_all(dir.path().join("exports")).unwrap();
    let (file, pin) = paths(dir.path(), &job.id).unwrap();
    std::fs::write(&file, &bytes).unwrap();
    std::fs::write(&pin, digest(&bytes)).unwrap();
    assert!(read_saved(dir.path(), &job, &hash).is_ok());
    std::fs::write(&file, b"{}").unwrap();
    assert!(read_saved(dir.path(), &job, &hash).is_err());
}

#[tokio::test]
async fn missing_catalog_context_is_explicit_and_an_explicit_retry_can_complete_it() {
    use axum::{routing::get, Router};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let dir = tempfile::tempdir().unwrap();
    let source = original(dir.path());
    let status = Arc::new(AtomicUsize::new(503));
    let calls = Arc::new(AtomicUsize::new(0));
    let value = json!({"type":"Feature","stac_version":"1.0.0","id":source.item_id,"collection":"sentinel-2-l2a","geometry":null,
        "properties":{"datetime":"2025-06-27T00:00:00Z","proj:epsg":32610},"assets":{"scl":{"href":source.href,"type":"image/tiff"}},"links":[]});
    let service = Router::new().fallback(get({
        let status = status.clone();
        let calls = calls.clone();
        move || {
            let status = status.clone();
            let calls = calls.clone();
            let value = value.clone();
            async move {
                calls.fetch_add(1, Ordering::SeqCst);
                (
                    axum::http::StatusCode::from_u16(status.load(Ordering::SeqCst) as u16).unwrap(),
                    axum::Json(value),
                )
            }
        }
    }));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, service).await.unwrap();
    });
    let manager = JobManager::open_inner(dir.path(), Some(origin))
        .await
        .unwrap();
    manager
        .inner
        .store
        .lock()
        .await
        .jobs
        .insert(source.id.clone(), source.clone());
    assert!(manager.raster_provenance_bytes(&source.id).await.is_err());
    assert!(!dir.path().join("exports").exists());
    let first = manager.prepare_raster_provenance(&source.id).await.unwrap();
    assert_eq!(first.captured_stac_items, 0);
    assert_eq!(first.missing_stac_items, 1);
    let (_, failed_bytes) = manager.raster_provenance_bytes(&source.id).await.unwrap();
    let failed: Value = serde_json::from_slice(&failed_bytes).unwrap();
    assert_eq!(
        failed["sources"][0]["catalog"]["reason"],
        "catalog_http_error"
    );
    assert_eq!(failed["sources"][0]["catalog"]["httpStatus"], 503);
    assert_eq!(failed["sources"][0]["catalog"]["retries"], 2);
    status.store(200, Ordering::SeqCst);
    let completed = manager.prepare_raster_provenance(&source.id).await.unwrap();
    assert_eq!(completed.captured_stac_items, 1);
    assert_eq!(completed.missing_stac_items, 0);
    let (_, bytes) = manager.raster_provenance_bytes(&source.id).await.unwrap();
    let doc: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        doc["sources"][0]["catalog"]["item"]["properties"]["proj:epsg"],
        32610
    );
    assert_eq!(doc["sources"][0]["catalog"]["captureStage"], "export");
    assert!(!String::from_utf8(bytes)
        .unwrap()
        .contains(dir.path().to_str().unwrap()));
    manager.prepare_raster_provenance(&source.id).await.unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 4);
    server.abort();
}

#[tokio::test]
async fn transient_context_failure_recovers_but_authorization_and_identity_failures_do_not_retry() {
    use axum::{routing::get, Router};
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    for behavior in ["transient", "forbidden", "identity"] {
        let dir = tempfile::tempdir().unwrap();
        let source = original(dir.path());
        let calls = Arc::new(AtomicUsize::new(0));
        let value = json!({"type":"Feature","id":if behavior=="identity"{"unrelated-item"}else{source.item_id.as_str()},"properties":{},"assets":{"scl":{"href":source.href}}});
        let app = Router::new().fallback(get({
            let calls = calls.clone();
            move || {
                let calls = calls.clone();
                let value = value.clone();
                async move {
                    let attempt = calls.fetch_add(1, Ordering::SeqCst);
                    let status = if behavior == "forbidden" {
                        403
                    } else if behavior == "transient" && attempt == 0 {
                        503
                    } else {
                        200
                    };
                    (
                        axum::http::StatusCode::from_u16(status).unwrap(),
                        axum::Json(value),
                    )
                }
            }
        }));
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let server = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let manager = JobManager::open_inner(dir.path(), Some(origin))
            .await
            .unwrap();
        let result = source_context(
            manager,
            source,
            tokio::time::Instant::now() + Duration::from_secs(5),
        )
        .await;
        if behavior == "transient" {
            assert_eq!(result["catalog"]["status"], "captured");
            assert_eq!(result["catalog"]["retries"], 1);
            assert_eq!(calls.load(Ordering::SeqCst), 2);
        } else {
            assert_eq!(result["catalog"]["status"], "unavailable");
            assert_eq!(result["catalog"]["retries"], 0);
            assert_eq!(calls.load(Ordering::SeqCst), 1);
            if behavior == "forbidden" {
                assert_eq!(result["catalog"]["httpStatus"], 403);
            } else {
                assert_eq!(result["catalog"]["reason"], "catalog_identity_mismatch");
            }
        }
        server.abort();
    }
}
