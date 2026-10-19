use super::*;
use std::io::Write;

const TIFF: &[u8] = include_bytes!("../../fixtures/gebco-region/gebco-2026-24x24.tif");
const BOUNDS: [f64; 4] = [-30.1, 40., -30., 40.1];
fn grids() -> Vec<u8> {
    serde_json::to_vec(&json!([{"id":1,"name":"gebco_2026_global","projection":"EPSG:4326","data_sources":[{"id":1,"name":"gebco_2026","display_name":"Bathymetry","x_name":"lon","y_name":"lat","z_name":"elevation"}]}])).unwrap()
}
fn formats() -> Vec<u8> {
    br#"[{"id":2,"name":"geotiff","category":"Data"}]"#.to_vec()
}
fn zip_fixture(method: zip::CompressionMethod, name: &str, tiff: &[u8]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for (name, bytes) in [
        (name, tiff),
        (
            "GEBCO_Grid_documentation.pdf",
            b"%PDF-test-transport-documentation".as_slice(),
        ),
        (
            "GEBCO_Grid_terms_of_use.pdf",
            b"%PDF-test-transport-terms".as_slice(),
        ),
    ] {
        writer
            .start_file(
                name,
                zip::write::SimpleFileOptions::default().compression_method(method),
            )
            .unwrap();
        writer.write_all(bytes).unwrap();
    }
    writer.finish().unwrap().into_inner()
}
#[test]
fn request_rejects_urls_email_credentials_invalid_regions_and_oversize() {
    let valid = SubsetRequest {
        name: "North Atlantic".into(),
        bounds: BOUNDS,
    };
    assert!(validate_request(&valid).is_ok());
    for bounds in [
        [-180., -80., 180., 80.],
        [0., 0., 50., 50.],
        [0., 0., 0., 1.],
        [10., 30., 9., 31.],
        [181., 0., 182., 1.],
    ] {
        assert!(validate_request(&SubsetRequest {
            name: "Region".into(),
            bounds
        })
        .is_err());
    }
    for extra in ["url", "email", "credential", "gridId", "dataSourceId"] {
        let mut v = json!({"name":"Region","bounds":BOUNDS});
        v[extra] = json!("untrusted");
        assert!(serde_json::from_value::<SubsetRequest>(v).is_err());
    }
    for name in [" ", "Region\n"] {
        assert!(validate_request(&SubsetRequest {
            name: name.into(),
            bounds: BOUNDS
        })
        .is_err());
    }
}
#[test]
fn catalog_does_not_substitute_tid_or_other_release_or_image_format() {
    assert!(selected_catalog(&grids(), &formats()).is_ok());
    for (path, value) in [
        ("name", json!("gebco_2025_global")),
        ("id", json!(4)),
        ("projection", json!("EPSG:3996")),
    ] {
        let mut g: Value = serde_json::from_slice(&grids()).unwrap();
        g[0][path] = value;
        assert!(selected_catalog(&serde_json::to_vec(&g).unwrap(), &formats()).is_err());
    }
    let mut g: Value = serde_json::from_slice(&grids()).unwrap();
    g[0]["data_sources"][0]["z_name"] = json!("tid");
    assert!(selected_catalog(&serde_json::to_vec(&g).unwrap(), &formats()).is_err());
    assert!(selected_catalog(
        &grids(),
        br#"[{"id":2,"name":"geotiff","category":"Image"}]"#
    )
    .is_err());
}
#[test]
fn archive_verifies_real_numerical_member_crc_region_and_safe_names() {
    let name = "gebco_2026_n40.1_s40.0_w-30.1_e-30.0_geotiff.tif";
    for method in [
        zip::CompressionMethod::Stored,
        zip::CompressionMethod::Deflated,
    ] {
        let bytes = zip_fixture(method, name, TIFF);
        let (n, b) = unzip(&bytes, BOUNDS).unwrap();
        assert_eq!(n, name);
        assert_eq!(b, TIFF);
        assert_eq!(
            hash(&b),
            "86e3fceacfa0f544c7644d8c54c71acb5c73071db5944346025d381e7e58a6ce"
        );
    }
    for name in [
        "../gebco_2026_n40.1_s40.0_w-30.1_e-30.0_geotiff.tif",
        "x\\subset.tif",
        "gebco_2026_n40.1_s40.0_w-29.1_e-30.0_geotiff.tif",
        "gebco_2026_tid.tif",
    ] {
        assert!(unzip(
            &zip_fixture(zip::CompressionMethod::Stored, name, TIFF),
            BOUNDS
        )
        .is_err());
    }
    let mut bytes = zip_fixture(zip::CompressionMethod::Stored, name, TIFF);
    let p = bytes.windows(8).position(|w| w == &TIFF[..8]).unwrap();
    bytes[p + 7] ^= 1;
    assert!(unzip(&bytes, BOUNDS).is_err());
}
#[test]
fn zip_rejects_encryption_devices_overlaps_and_forged_expansion_before_decoding() {
    let original = zip_fixture(
        zip::CompressionMethod::Stored,
        "gebco_2026_n40.1_s40.0_w-30.1_e-30.0_geotiff.tif",
        TIFF,
    );
    let central = original
        .windows(4)
        .position(|w| w == b"PK\x01\x02")
        .unwrap();
    let mut encrypted = original.clone();
    encrypted[6] |= 1;
    encrypted[central + 8] |= 1;
    assert!(unzip(&encrypted, BOUNDS).is_err());
    for mode in [0o020666u32, 0o060666, 0o120777] {
        let mut changed = original.clone();
        changed[central + 5] = 3;
        changed[central + 38..central + 42].copy_from_slice(&(mode << 16).to_le_bytes());
        assert!(unzip(&changed, BOUNDS).is_err());
    }
    let mut expanded = original.clone();
    expanded[central + 24..central + 28].copy_from_slice(&(MAX_BYTES as u32 + 1).to_le_bytes());
    assert!(unzip(&expanded, BOUNDS).is_err());
    let mut overlap = original.clone();
    overlap[central + 20..central + 24].copy_from_slice(&(TIFF.len() as u32 + 100).to_le_bytes());
    assert!(unzip(&overlap, BOUNDS).is_err());
    for name in ["CON", "COM1.tif", "LPT9.txt", "NUL ", "x:y", "folder\\name"] {
        assert!(!safe_member(name));
    }
}
#[tokio::test]
async fn managed_native_chain_preserves_real_values_dual_hash_reopen_and_detects_tampering() {
    use axum::{
        routing::{get, post},
        Json, Router,
    };
    let archive = zip_fixture(
        zip::CompressionMethod::Stored,
        "gebco_2026_n40.1_s40.0_w-30.1_e-30.0_geotiff.tif",
        TIFF,
    );
    let copied = archive.clone();
    let g = grids();
    let f = formats();
    let router = Router::new()
        .route("/api/grids", get(move || async move { g }))
        .route("/api/formats", get(move || async move { f }))
        .route(
            "/api/queue",
            post(|Json(v): Json<Value>| async move {
                assert_eq!(v["email"], Value::Null);
                assert_eq!(v["items"][0]["data_source_ids"], json!([1]));
                (
                    axum::http::StatusCode::CREATED,
                    Json(json!({"basketId":"GEBCO_14_Oct_2026_test_transport"})),
                )
            }),
        )
        .route(
            "/api/queue/status/{id}",
            get(|| async { Json(json!({"status":"finished"})) }),
        )
        .route(
            "/api/queue/download/{id}",
            get(move || async move { copied }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let temp = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let result = manager
        .gebco_create_with_client(
            SubsetRequest {
                name: "Real GEBCO pixels, simulated transport".into(),
                bounds: BOUNDS,
            },
            reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            &origin,
        )
        .await
        .unwrap();
    assert_eq!((result.raster.width, result.raster.height), (24, 24));
    assert!(!result.record.publisher_static_whole_original);
    let id = result.record.id.clone();
    assert_eq!(manager.gebco_archive(&id).await.unwrap(), archive);
    assert_eq!(manager.gebco_source(&id).await.unwrap(), TIFF);
    assert_eq!(manager.gebco_list().await.unwrap().len(), 1);
    let sample = manager
        .gebco_pixel(PixelRequest {
            id: id.clone(),
            column: 12,
            row: 12,
        })
        .await
        .unwrap();
    assert_eq!(sample.values, vec![json!(-1705)]);
    assert_eq!(sample.no_data, vec![false]);
    assert!(manager
        .gebco_pixel(PixelRequest {
            id: id.clone(),
            column: 24,
            row: 0
        })
        .await
        .is_err());
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(temp.path()).await.unwrap();
    assert_eq!(
        manager.gebco_get(&id).await.unwrap().sha256,
        result.record.sha256
    );
    assert!(manager
        .gebco_inspect(&id)
        .await
        .unwrap()
        .preview_data_url
        .is_some());
    let path = record_path(&manager, &id).unwrap().join("subset.tif");
    let mut changed = TIFF.to_vec();
    changed[100] ^= 1;
    tokio::fs::write(&path, changed).await.unwrap();
    assert!(manager.gebco_source(&id).await.is_err());
    assert!(manager.gebco_archive(&id).await.is_err());
    assert!(manager.gebco_list().await.is_err());
    manager.shutdown().await.unwrap();
    server.abort();
}
#[tokio::test]
#[ignore = "Creates one actual bounded official GEBCO subset and fresh managed research store; set GEOD_GEBCO_ACCEPTANCE"]
async fn official_live_generation_download_pixel_and_reopen_acceptance() {
    let output = std::path::PathBuf::from(std::env::var("GEOD_GEBCO_ACCEPTANCE").unwrap());
    std::fs::create_dir_all(&output).unwrap();
    let store = output.join("store");
    assert!(
        !store.exists(),
        "Live acceptance must use a fresh private store"
    );
    let manager = JobManager::open(&store).await.unwrap();
    if let Ok(proxy) = std::env::var("GEOD_GEBCO_TEST_PROXY") {
        manager
            .save_proxy_settings(crate::proxy::ProxySettings {
                mode: crate::proxy::ProxyMode::Custom,
                url: Some(proxy),
            })
            .await
            .unwrap();
    }
    let result = manager
        .gebco_create_subset(SubsetRequest {
            name: "GEBCO 2026 North Atlantic numerical subset".into(),
            bounds: BOUNDS,
        })
        .await
        .unwrap();
    let id = result.record.id.clone();
    let inspection = manager.gebco_inspect(&id).await.unwrap();
    let source = manager.gebco_source(&id).await.unwrap();
    let archive = manager.gebco_archive(&id).await.unwrap();
    let points = [
        (0, 0, -1755),
        (12, 0, -1751),
        (23, 0, -1554),
        (0, 12, -1867),
        (12, 12, -1705),
        (23, 12, -1705),
        (0, 23, -1890),
        (12, 23, -1678),
        (23, 23, -1673),
    ];
    let mut pixels = Vec::new();
    for (column, row, raw) in points {
        let v = manager
            .gebco_pixel(PixelRequest {
                id: id.clone(),
                column,
                row,
            })
            .await
            .unwrap();
        assert_eq!(v.values, vec![json!(raw)]);
        assert_eq!(v.no_data, vec![false]);
        pixels.push(v);
    }
    // A generated TIFF includes the service's basket/time history; distinct
    // requests legitimately differ in SHA while preserving all grid samples.
    assert_eq!(hash(&source), result.record.sha256);
    assert_eq!(hash(&archive), result.record.archive_sha256);
    let reference: Value =
        serde_json::from_str(include_str!("../../fixtures/gebco-region/reference.json")).unwrap();
    let actual = tiff::decoder::Decoder::new(Cursor::new(&source))
        .unwrap()
        .read_image()
        .unwrap();
    let tiff::decoder::DecodingResult::I16(actual) = actual else {
        panic!("Signed elevation samples required")
    };
    let expected = reference["pixels"].as_array().unwrap();
    assert_eq!(actual.len(), expected.len() * 24);
    for (i, value) in actual.iter().enumerate() {
        assert_eq!(*value as i64, expected[i / 24][i % 24].as_i64().unwrap());
    }
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(&store).await.unwrap();
    let reopened = manager.gebco_get(&id).await.unwrap();
    assert_eq!(reopened.sha256, result.record.sha256);
    assert_eq!(manager.gebco_source(&id).await.unwrap(), source);
    assert_eq!(manager.gebco_archive(&id).await.unwrap(), archive);
    assert_eq!(manager.gebco_list().await.unwrap().len(), 1);
    manager.shutdown().await.unwrap();
    let evidence = json!({"record":result.record,"inspection":inspection,"pixels":pixels,"all576GridSamplesMatchIndependentReference":true,"nativeGeneration":true,"wholeOfficialZip":true,"sourceMemberUnmodified":true,"dualHashVerified":true,"referencePointsMatch":true,"reopenVerified":true,"researchDataCopiedIntoStore":false,"publisherStaticWholeOriginal":false,"originalPlannedIdCompleted":false});
    std::fs::write(
        output.join("acceptance.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!(
        "GEBCO official live acceptance passed; source SHA {}, ZIP SHA {}",
        reopened.sha256, reopened.archive_sha256
    );
}
#[tokio::test]
#[ignore = "Read-only native regression of actual GEBCO software-created research subset after parser changes; set GEOD_GEBCO_EXISTING_ACCEPTANCE"]
async fn official_saved_subset_new_parser_reopen_preserves_whole_files() {
    let output = PathBuf::from(std::env::var("GEOD_GEBCO_EXISTING_ACCEPTANCE").unwrap());
    let original: Value =
        serde_json::from_slice(&std::fs::read(output.join("acceptance.json")).unwrap()).unwrap();
    let manager = JobManager::open(output.join("store")).await.unwrap();
    let id = original["record"]["id"].as_str().unwrap();
    let record = manager.gebco_get(id).await.unwrap();
    assert_eq!(manager.gebco_list().await.unwrap().len(), 1);
    let source = manager.gebco_source(id).await.unwrap();
    let archive = manager.gebco_archive(id).await.unwrap();
    assert_eq!(
        hash(&source),
        original["record"]["sha256"].as_str().unwrap()
    );
    assert_eq!(
        hash(&archive),
        original["record"]["archiveSha256"].as_str().unwrap()
    );
    let inspection = manager.gebco_inspect(id).await.unwrap();
    assert_eq!((inspection.width, inspection.height), (24, 24));
    let pixel = manager
        .gebco_pixel(PixelRequest {
            id: id.into(),
            column: 12,
            row: 12,
        })
        .await
        .unwrap();
    assert_eq!(pixel.values, vec![json!(-1705)]);
    manager.shutdown().await.unwrap();
    std::fs::write(output.join("acceptance-parser-reopen.json"),serde_json::to_vec_pretty(&json!({"record":record,"inspection":inspection,"pixel":pixel,"sameWholeZipSha256":true,"sameWholeTiffSha256":true,"noNewQueue":true,"sourceResearchDataInjected":false})).unwrap()).unwrap();
}

#[tokio::test]
#[ignore = "One official tiny GEBCO queue-only native diagnostic; set GEOD_GEBCO_QUEUE_DIAGNOSTIC and GEOD_GEBCO_TEST_PROXY"]
async fn official_same_proxy_queue_transport_diagnostic() {
    let output = PathBuf::from(std::env::var("GEOD_GEBCO_QUEUE_DIAGNOSTIC").unwrap());
    assert!(
        !output.exists(),
        "Do not repeat a publisher queue diagnostic automatically"
    );
    std::fs::create_dir_all(&output).unwrap();
    let temporary = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temporary.path()).await.unwrap();
    manager
        .save_proxy_settings(crate::proxy::ProxySettings {
            mode: crate::proxy::ProxyMode::Custom,
            url: Some(std::env::var("GEOD_GEBCO_TEST_PROXY").unwrap()),
        })
        .await
        .unwrap();
    let client = manager.gebco_client().await.unwrap();
    let (grids, formats) = catalog_bytes(&client, ORIGIN).await.unwrap();
    selected_catalog(&grids, &formats).unwrap();
    let body=serde_json::to_vec(&json!({"id":"0","email":null,"submission_date":now(),"processing_status":"new","items":[{"id":0,"grid_id":1,"data_source_ids":[1],"formats":[2],"left":-30.1,"right":-30.0,"top":40.1,"bottom":40.0}]})).unwrap();
    std::fs::write(output.join("submission.json"), &body).unwrap();
    let started = std::time::Instant::now();
    let sent = client
        .post(format!("{ORIGIN}/api/queue"))
        .header("Content-Type", "application/json")
        .body(body)
        .send()
        .await;
    let mut receipt = json!({"observedAt":now(),"transport":"same native reqwest client: metadata then queue","proxyKind":"same-custom-socks5","endpoint":"https://download.gebco.net/api/queue","metadataVerified":true,"email":null,"credentials":false,"automaticRetry":false});
    let succeeded = match sent {
        Ok(response) => {
            let status = response.status().as_u16();
            receipt["httpStatus"] = json!(status);
            let raw = response_bytes(response, status, MAX_METADATA)
                .await
                .unwrap();
            std::fs::write(output.join("response.json"), &raw).unwrap();
            if let Ok(v) = serde_json::from_slice::<Value>(&raw) {
                receipt["message"] = v["message"].clone();
                receipt["basketId"] = v["basketId"].clone();
            }
            status == 201
        }
        Err(error) => {
            let mut chain = Vec::new();
            let mut source = std::error::Error::source(&error);
            while let Some(value) = source {
                let text = value.to_string().to_ascii_lowercase();
                chain.push(if text.contains("reset") {
                    "connection-reset"
                } else if text.contains("closed")
                    || text.contains("incomplete")
                    || text.contains("eof")
                {
                    "peer-closed-or-incomplete"
                } else if text.contains("timed out") || text.contains("timeout") {
                    "timeout"
                } else if text.contains("socks") {
                    "socks-transport"
                } else if text.contains("dns") {
                    "dns"
                } else if text.contains("certificate")
                    || text.contains("tls")
                    || text.contains("ssl")
                {
                    "tls"
                } else {
                    "other-transport"
                });
                source = value.source();
            }
            receipt["errorClassification"] = json!({"isTimeout":error.is_timeout(),"isConnect":error.is_connect(),"isRequest":error.is_request(),"isBody":error.is_body(),"safeCauseCategories":chain});
            false
        }
    };
    receipt["elapsedSeconds"] = json!(started.elapsed().as_secs_f64());
    std::fs::write(
        output.join("receipt.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .unwrap();
    manager.shutdown().await.unwrap();
    println!("{}", receipt);
    assert!(
        succeeded,
        "Queue transport diagnostic failed; inspect the safe diagnostic receipt"
    );
}

#[tokio::test]
async fn interrupted_status_and_retryable_reads_continue_same_basket_without_post_retry() {
    use axum::{
        response::IntoResponse,
        routing::{get, post},
        Json, Router,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let grid_calls = Arc::new(AtomicUsize::new(0));
    let format_calls = Arc::new(AtomicUsize::new(0));
    let status_calls = Arc::new(AtomicUsize::new(0));
    let download_calls = Arc::new(AtomicUsize::new(0));
    let post_calls = Arc::new(AtomicUsize::new(0));
    let gc = grid_calls.clone();
    let fc = format_calls.clone();
    let sc = status_calls.clone();
    let dc = download_calls.clone();
    let pc = post_calls.clone();
    let archive = zip_fixture(
        zip::CompressionMethod::Stored,
        "gebco_2026_n40.1_s40.0_w-30.1_e-30.0_geotiff.tif",
        TIFF,
    );
    let expected_sha = hash(&archive);
    let router = Router::new()
        .route(
            "/api/grids",
            get(move || {
                let calls = gc.clone();
                async move {
                    if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                        axum::http::StatusCode::SERVICE_UNAVAILABLE.into_response()
                    } else {
                        grids().into_response()
                    }
                }
            }),
        )
        .route(
            "/api/formats",
            get(move || {
                let calls = fc.clone();
                async move {
                    if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                        axum::http::StatusCode::TOO_MANY_REQUESTS.into_response()
                    } else {
                        formats().into_response()
                    }
                }
            }),
        )
        .route(
            "/api/queue",
            post(move || {
                let calls = pc.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    (
                        axum::http::StatusCode::CREATED,
                        Json(json!({"basketId":"GEBCO_14_Oct_2026_retry_same_basket"})),
                    )
                }
            }),
        )
        .route(
            "/api/queue/status/{id}",
            get(
                move |axum::extract::Path(id): axum::extract::Path<String>| {
                    let calls = sc.clone();
                    async move {
                        assert_eq!(id, "GEBCO_14_Oct_2026_retry_same_basket");
                        if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                            let chunks = vec![
                                Ok::<_, std::io::Error>(vec![b'{']),
                                Err(std::io::Error::new(
                                    std::io::ErrorKind::ConnectionReset,
                                    "controlled GET interruption",
                                )),
                            ];
                            axum::response::Response::builder()
                                .status(200)
                                .header("Content-Length", "100")
                                .body(axum::body::Body::from_stream(futures_util::stream::iter(
                                    chunks,
                                )))
                                .unwrap()
                        } else {
                            Json(json!({"status":"finished"})).into_response()
                        }
                    }
                },
            ),
        )
        .route(
            "/api/queue/download/{id}",
            get(
                move |axum::extract::Path(id): axum::extract::Path<String>| {
                    let calls = dc.clone();
                    let archive = archive.clone();
                    async move {
                        assert_eq!(id, "GEBCO_14_Oct_2026_retry_same_basket");
                        if calls.fetch_add(1, Ordering::SeqCst) == 0 {
                            axum::http::StatusCode::REQUEST_TIMEOUT.into_response()
                        } else {
                            archive.into_response()
                        }
                    }
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let temporary = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temporary.path()).await.unwrap();
    let result = tokio::time::timeout(
        Duration::from_secs(180),
        manager.gebco_create_with_client(
            SubsetRequest {
                name: "Controlled GET retry with real numerical pixels".into(),
                bounds: BOUNDS,
            },
            reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            &origin,
        ),
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(
        result.record.basket_id,
        "GEBCO_14_Oct_2026_retry_same_basket"
    );
    assert_eq!(result.record.archive_sha256, expected_sha);
    assert_eq!(post_calls.load(Ordering::SeqCst), 1);
    for calls in [grid_calls, format_calls, status_calls, download_calls] {
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
    assert_eq!(manager.gebco_list().await.unwrap().len(), 1);
    let pixel = manager
        .gebco_pixel(PixelRequest {
            id: result.record.id,
            column: 12,
            row: 12,
        })
        .await
        .unwrap();
    assert_eq!(pixel.values, vec![json!(-1705)]);
    manager.shutdown().await.unwrap();
    server.abort();
}

#[tokio::test]
async fn permanent_status_failure_stops_after_four_reads_and_never_requeues_or_saves() {
    use axum::{
        routing::{get, post},
        Json, Router,
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let posts = Arc::new(AtomicUsize::new(0));
    let statuses = Arc::new(AtomicUsize::new(0));
    let pc = posts.clone();
    let sc = statuses.clone();
    let router = Router::new()
        .route("/api/grids", get(|| async { grids() }))
        .route("/api/formats", get(|| async { formats() }))
        .route(
            "/api/queue",
            post(move || {
                let calls = pc.clone();
                async move {
                    calls.fetch_add(1, Ordering::SeqCst);
                    (
                        axum::http::StatusCode::CREATED,
                        Json(json!({"basketId":"GEBCO_14_Oct_2026_permanent_failure"})),
                    )
                }
            }),
        )
        .route(
            "/api/queue/status/{id}",
            get(
                move |axum::extract::Path(id): axum::extract::Path<String>| {
                    let calls = sc.clone();
                    async move {
                        assert_eq!(id, "GEBCO_14_Oct_2026_permanent_failure");
                        calls.fetch_add(1, Ordering::SeqCst);
                        axum::http::StatusCode::SERVICE_UNAVAILABLE
                    }
                },
            ),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, router).await.unwrap();
    });
    let temporary = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temporary.path()).await.unwrap();
    let error = manager
        .gebco_create_with_client(
            SubsetRequest {
                name: "Controlled permanent GET failure".into(),
                bounds: BOUNDS,
            },
            reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .build()
                .unwrap(),
            &origin,
        )
        .await
        .unwrap_err();
    assert!(error.contains("four GET attempts"));
    assert!(error.contains("no queue POST was repeated"));
    assert_eq!(posts.load(Ordering::SeqCst), 1);
    assert_eq!(statuses.load(Ordering::SeqCst), GET_ATTEMPTS);
    assert!(manager.gebco_list().await.unwrap().is_empty());
    manager.shutdown().await.unwrap();
    server.abort();
}
