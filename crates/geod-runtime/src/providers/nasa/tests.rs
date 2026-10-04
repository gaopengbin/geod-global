use super::*;
use crate::{new_download_job, CreateJobRequest};
use axum::{http::HeaderMap, routing::get, Json, Router};

const ID: &str = "HLS.L30.T10SEG.2025179T184546.v2.0";

#[tokio::test]
async fn viirs_original_authorization_and_catalogue_period_are_not_bypassed() {
    let id = "VJ209A1.A2025177.h08v05.002.2025333224010";
    let href = format!("https://{HOST}/lp-prod-protected/VJ209A1.002/{id}/{id}.h5");
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let record = manager
        .create(CreateJobRequest {
            item_id: id.into(),
            asset_key: "viirs".into(),
            href: href.clone(),
            media_type: "application/x-hdf5".into(),
            title: None,
        })
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let job = manager.get(&record.id).await.unwrap();
            if job.status == crate::JobStatus::Failed {
                assert!(job.error.unwrap().contains("Settings"));
                assert!(job.output_path.is_none());
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .unwrap();
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(directory.path()).await.unwrap();
    assert_eq!(reopened.get(&record.id).await.unwrap().href, href);
    assert!(std::fs::read_dir(directory.path().join("assets"))
        .unwrap()
        .next()
        .is_none());
    reopened.shutdown().await.unwrap();
    let good = serde_json::json!({"id":id,"collection":"VJ209A1_002","properties":{"start_datetime":"2025-06-26T00:00:00.000Z","end_datetime":"2025-07-03T23:59:59.000Z"},"assets":{"2025333224010":{"href":href}}});
    let mut bad = good.clone();
    bad["properties"]["end_datetime"] = "2025-07-04T23:59:59Z".into();
    let app = Router::new()
        .route(
            "/good",
            get(move || {
                let value = good.clone();
                async move { Json(value) }
            }),
        )
        .route(
            "/bad",
            get(move || {
                let value = bad.clone();
                async move { Json(value) }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = Client::builder().no_proxy().build().unwrap();
    assert!(verify_item(
        &client,
        Url::parse(&format!("{origin}/good")).unwrap(),
        &record
    )
    .await
    .is_ok());
    assert!(verify_item(
        &client,
        Url::parse(&format!("{origin}/bad")).unwrap(),
        &record
    )
    .await
    .is_err());
    task.abort();
    assert!(crate::verify_signature(b"\x89HDF\r\n\x1a\n", "application/x-hdf5").is_ok());
    for header in [
        b"<html>login".as_slice(),
        b"II\x2a\0".as_slice(),
        b"\x89HDF".as_slice(),
    ] {
        assert!(crate::verify_signature(header, "application/x-hdf5").is_err());
    }
}

#[tokio::test]
async fn srtm_authorization_gates_originals_and_retries_the_same_persisted_job() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let id = "N37W123.SRTMGL1.hgt";
    let record = manager
        .create(CreateJobRequest {
            item_id: id.into(),
            asset_key: "srtm".into(),
            href: format!(
                "https://{HOST}{}{id}/{id}.zip",
                crate::providers::srtm::PREFIX
            ),
            media_type: "application/zip".into(),
            title: None,
        })
        .await
        .unwrap();
    let failed = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let job = manager.get(&record.id).await.unwrap();
            if job.status == crate::JobStatus::Failed {
                break job;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(failed.error.unwrap().contains("Settings"));
    assert!(failed.output_path.is_none());
    assert!(std::fs::read_dir(directory.path().join("assets"))
        .unwrap()
        .next()
        .is_none());
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(directory.path()).await.unwrap();
    assert_eq!(reopened.get(&record.id).await.unwrap().href, record.href);
    reopened.retry(&record.id).await.unwrap();
    assert_eq!(reopened.get(&record.id).await.unwrap().attempts, 2);
    reopened.shutdown().await.unwrap();
}

#[tokio::test]
async fn srtm_catalogue_pins_collection_hgt_and_exact_signed_cdn_object() {
    let id = "N37W123.SRTMGL1.hgt";
    let record = new_download_job(CreateJobRequest {
        item_id: id.into(),
        asset_key: "srtm".into(),
        href: format!(
            "https://{HOST}{}{id}/{id}.zip",
            crate::providers::srtm::PREFIX
        ),
        media_type: "application/zip".into(),
        title: None,
    });
    let good = serde_json::json!({"id":id,"collection":"SRTMGL1_003","assets":{"hgt":{"href":record.href}}});
    let mut wrong = good.clone();
    wrong["collection"] = "NASADEM_HGT_001".into();
    let app = Router::new()
        .route(
            "/good",
            get(move || {
                let data = good.clone();
                async move { Json(data) }
            }),
        )
        .route(
            "/wrong",
            get(move || {
                let data = wrong.clone();
                async move { Json(data) }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let client = Client::builder()
        .no_proxy()
        .redirect(reqwest::redirect::Policy::none())
        .build()
        .unwrap();
    assert!(verify_item(
        &client,
        Url::parse(&format!("{origin}/good")).unwrap(),
        &record
    )
    .await
    .is_ok());
    assert!(verify_item(
        &client,
        Url::parse(&format!("{origin}/wrong")).unwrap(),
        &record
    )
    .await
    .is_err());
    let original = Url::parse(&record.href).unwrap();
    let target=format!("https://{CDN}/get-Srtm1/lp-prod-protected.s3.us-west-2.amazonaws.com/SRTMGL1.003/{id}/{id}.zip?Expires=2000000000&Signature=PRIVATE&Key-Pair-Id=PRIVATE");
    assert!(cdn_target(&target, &original, 1999999900).is_ok());
    assert!(cdn_target(&target.replace(".003/", ".002/"), &original, 1999999900).is_err());
    assert!(cdn_target(&target.replace("N37W123", "N38W123"), &original, 1999999900).is_err());
    task.abort();
}
fn job() -> Job {
    new_download_job(CreateJobRequest {
        item_id: ID.into(),
        asset_key: "red".into(),
        href: format!("https://{HOST}{PREFIX}{ID}/{ID}.B04.tif"),
        media_type: "image/tiff; application=geotiff".into(),
        title: None,
    })
}

#[test]
fn hls_band_binding_rejects_wrong_product_channel_unsigned_host_and_version() {
    for (key, band) in [("red", "B04"), ("green", "B03"), ("blue", "B02")] {
        let mut record = job();
        record.asset_key = key.into();
        record.href = record.href.replace("B04", band);
        let url = crate::providers::asset_url(&record.href).unwrap();
        assert!(matches_item(&url, ID, key));
        assert!(!matches_item(&url, ID, "scl"));
        assert!(!matches_item(
            &url,
            "HLS.L30.T10SEG.2025178T184546.v2.0",
            key
        ));
        for bad in [
            record.href.replace("v2.0", "v1.5"),
            record.href.replace("HLSL30.020", "HLSS30.020"),
            format!("{}?token=secret", record.href),
            record.href.replace(HOST, "evil.example"),
            record.href.replace(band, "Fmask"),
        ] {
            assert!(crate::providers::asset_url(&bad).is_err());
        }
    }
    assert!(!valid_item("HLS.L30.T0中中.2025179T184546.v2.0"));
    assert!(!asset_path("/lp-prod-protected/HLSL30.020/../secret.tif"));
}

#[test]
fn only_exact_signed_cdn_object_is_followed_and_errors_do_not_echo_access_details() {
    let original = Url::parse(&job().href).unwrap();
    let good = format!("https://{CDN}/get-abcd1234/lp-prod-protected.s3.us-west-2.amazonaws.com/HLSL30.020/{ID}/{ID}.B04.tif?Expires=2000000000&Signature=PRIVATE-SIGNATURE&Key-Pair-Id=PRIVATE-KEY");
    assert!(cdn_target(&good, &original, 1999999900).is_ok());
    for bad in [
        good.replace(CDN, "unreviewed.cloudfront.net"),
        good.replace("B04", "B03"),
        good.replace("Expires=2000000000", "Expires=1"),
        good.replace("Expires=2000000000", "Expires=2999999900"),
        format!("{good}&Expires=2000000000"),
        good.replace("Signature=PRIVATE-SIGNATURE", "Signature="),
        good.replace("https:", "http:"),
        good.replace("/get-", "/head-"),
        format!("{good}#fragment"),
        "https://urs.earthdata.nasa.gov/oauth/authorize?secret=PRIVATE-SIGNATURE".into(),
    ] {
        let error = cdn_target(&bad, &original, 1999999900).unwrap_err();
        assert!(!error.contains("PRIVATE"));
    }
}

#[tokio::test]
async fn protocol_checks_catalogue_identity_bearer_and_rejects_login_redirects() {
    async fn original(headers: HeaderMap) -> (StatusCode, Vec<u8>) {
        assert_eq!(
            headers.get(header::AUTHORIZATION).unwrap(),
            "Bearer LOCAL-TEST-TOKEN"
        );
        (StatusCode::OK, b"II\x2a\0local-transfer-fixture".to_vec())
    }
    let record = job();
    let metadata = serde_json::json!({ "id": ID, "collection": "HLSL30_2.0", "assets": { "B04": {"href":record.href} } });
    let good = metadata.clone();
    let mut wrong = metadata;
    wrong["assets"]["B04"]["href"] = Value::String("https://evil.example/file.tif".into());
    let app = Router::new()
        .route(
            "/item",
            get(move || {
                let good = good.clone();
                async move { Json(good) }
            }),
        )
        .route(
            "/wrong",
            get(move || {
                let wrong = wrong.clone();
                async move { Json(wrong) }
            }),
        )
        .route("/file", get(original))
        .route(
            "/login",
            get(|| async {
                (
                    StatusCode::FOUND,
                    [(
                        header::LOCATION,
                        "https://urs.earthdata.nasa.gov/oauth/authorize?token=PRIVATE",
                    )],
                )
            }),
        )
        .route(
            "/unreviewed",
            get(|| async {
                (
                    StatusCode::TEMPORARY_REDIRECT,
                    [(
                        header::LOCATION,
                        "http://127.0.0.1:1/token-theft?token=PRIVATE",
                    )],
                )
            }),
        )
        .route(
            "/denied",
            get(|| async { (StatusCode::FORBIDDEN, "SECRET-ERROR-BODY") }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let task = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    // A local protocol fixture must not inherit the workstation's proxy route.
    let client = crate::proxy::download_client(&crate::ProxySettings {
        mode: crate::proxy::ProxyMode::Direct,
        url: None,
    })
    .unwrap();
    assert!(verify_item(
        &client,
        Url::parse(&format!("{origin}/item")).unwrap(),
        &record
    )
    .await
    .is_ok());
    assert!(verify_item(
        &client,
        Url::parse(&format!("{origin}/wrong")).unwrap(),
        &record
    )
    .await
    .is_err());
    let response = protected_response(
        &client,
        Url::parse(&format!("{origin}/file")).unwrap(),
        "LOCAL-TEST-TOKEN",
    )
    .await
    .unwrap();
    let bytes = response.bytes().await.unwrap();
    assert!(crate::verify_signature(&bytes, "image/tiff").is_ok());
    for path in ["login", "unreviewed", "denied"] {
        let error = protected_response(
            &client,
            Url::parse(&format!("{origin}/{path}")).unwrap(),
            "LOCAL-TEST-TOKEN",
        )
        .await
        .unwrap_err();
        assert!(
            !error.contains("PRIVATE")
                && !error.contains("SECRET")
                && !error.contains("LOCAL-TEST-TOKEN")
        );
    }
    task.abort();
}

#[tokio::test]
async fn unconnected_original_fails_cleanly_and_same_job_can_be_retried_after_restart() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let record = job();
    let queued = manager
        .create(CreateJobRequest {
            item_id: record.item_id,
            asset_key: record.asset_key,
            href: record.href.clone(),
            media_type: record.media_type,
            title: None,
        })
        .await
        .unwrap();
    let finished = tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let value = manager.get(&queued.id).await.unwrap();
            if value.status == crate::JobStatus::Failed {
                break value;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    assert!(finished.error.unwrap().contains("Settings"));
    assert!(finished.output_path.is_none());
    assert!(tokio::fs::read_dir(directory.path().join("assets"))
        .await
        .unwrap()
        .next_entry()
        .await
        .unwrap()
        .is_none());
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(directory.path()).await.unwrap();
    let restored = manager.get(&queued.id).await.unwrap();
    assert_eq!(restored.href, record.href);
    manager.retry(&queued.id).await.unwrap();
    assert_eq!(manager.get(&queued.id).await.unwrap().attempts, 2);
    manager.shutdown().await.unwrap();
}
