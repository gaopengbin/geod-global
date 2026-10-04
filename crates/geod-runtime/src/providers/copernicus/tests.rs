use super::*;
use std::io::{Cursor, Write};

const ID: &str = "S2C_MSIL2A_20250627T184941_N0511_R113_T10SEG_20250627T234511";
const UUID: &str = "0d695b42-4b24-4954-ba09-f2a44303fdd8";
fn value() -> Value {
    serde_json::json!({"Id":UUID,"Name":format!("{ID}.SAFE"),"Online":true,"ContentLength":1128466167_u64})
}
#[test]
fn product_binding_retains_processing_baseline_and_rejects_offline_ambiguous_or_oversized_metadata()
{
    assert!(valid_item(ID));
    assert!(!valid_item(&ID.replace("MSIL2A", "MSIL1C")));
    assert!(!valid_item("S2A_MSIL2A_' or Online eq true"));
    let good = product(&value(), ID).unwrap();
    let url = crate::providers::asset_url(&good.href).unwrap();
    assert!(crate::providers::matches_item(&url, ID, "product"));
    assert!(!crate::providers::matches_item(&url, ID, "visual"));
    assert!(product(&value(), &ID.replace("N0511", "N0400")).is_err());
    for (key, replacement) in [
        ("Online", Value::Bool(false)),
        ("Id", Value::String("not-a-uuid".into())),
        ("ContentLength", Value::from(MAX_PRODUCT_BYTES + 1)),
        ("ContentLength", Value::from(0)),
    ] {
        let mut bad = value();
        bad[key] = replacement;
        assert!(product(&bad, ID).is_err());
    }
    for href in [
        format!("{}?access_token=PRIVATE", good.href),
        good.href.replace(HOST, "evil.example"),
        good.href.replace("/$value", "/$zip"),
    ] {
        assert!(crate::providers::asset_url(&href).is_err());
    }
    let request = crate::CreateJobRequest {
        item_id: ID.into(),
        asset_key: "product".into(),
        href: good.href,
        media_type: "application/zip".into(),
        title: None,
    };
    assert!(crate::validate_request(&request, None).is_ok());
    let wrong_type = crate::CreateJobRequest {
        media_type: "image/tiff".into(),
        ..request
    };
    assert!(crate::validate_request(&wrong_type, None).is_err());
}

fn archive(names: &[String]) -> Vec<u8> {
    let mut writer = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for name in names {
        writer
            .start_file(
                name,
                zip::write::SimpleFileOptions::default()
                    .compression_method(zip::CompressionMethod::Stored),
            )
            .unwrap();
        writer
            .write_all(b"unit-test-only-not-a-real-product")
            .unwrap();
    }
    writer.finish().unwrap().into_inner()
}
fn names() -> Vec<String> {
    [
        "manifest.safe",
        "MTD_MSIL2A.xml",
        "GRANULE/unit/IMG_DATA/R10m/T10SEG_TCI_10m.jp2",
        "GRANULE/unit/IMG_DATA/R20m/T10SEG_SCL_20m.jp2",
    ]
    .iter()
    .map(|name| format!("{ID}.SAFE/{name}"))
    .collect()
}
#[test]
fn safe_archive_directory_requires_correct_product_and_rejects_missing_truncated_or_unsafe_entries()
{
    let directory = tempfile::tempdir().unwrap();
    let path = directory.path().join("product.zip");
    let good = archive(&names());
    std::fs::write(&path, &good).unwrap();
    assert!(verify_safe(&path, ID).is_ok());
    assert!(verify_safe(&path, &ID.replace("T10SEG", "T10SEH")).is_err());
    for bad in [
        archive(&names()[..3]),
        archive(&[format!("{ID}.SAFE/../outside")]),
        archive(&["other.SAFE/manifest.safe".into()]),
        good[..good.len() - 20].to_vec(),
        b"<html>login</html>".to_vec(),
    ] {
        std::fs::write(&path, bad).unwrap();
        assert!(verify_safe(&path, ID).is_err());
    }
}

#[tokio::test]
async fn public_resolution_pins_name_and_download_auth_never_follows_foreign_redirects() {
    use axum::{http::HeaderMap, routing::get, Json, Router};
    let metadata = value();
    let app = Router::new()
        .route(
            "/products",
            get(
                move |query: axum::extract::Query<std::collections::HashMap<String, String>>| {
                    let metadata = metadata.clone();
                    async move {
                        assert_eq!(
                            query.get("$filter").unwrap(),
                            &format!("Name eq '{ID}.SAFE'")
                        );
                        Json(serde_json::json!({"value":[metadata]}))
                    }
                },
            ),
        )
        .route(
            "/download",
            get(|headers: HeaderMap| async move {
                assert_eq!(
                    headers.get(header::AUTHORIZATION).unwrap(),
                    "Bearer PRIVATE-ACCESS-TOKEN"
                );
                (
                    StatusCode::FOUND,
                    [(header::LOCATION, "https://evil.example/data?token=SECRET")],
                )
            }),
        )
        .route(
            "/denied",
            get(|| async { (StatusCode::UNAUTHORIZED, "PRIVATE-ERROR-BODY") }),
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
    assert_eq!(
        resolve_one(&client, &format!("{origin}/products"), ID)
            .await
            .unwrap()
            .item_id,
        ID
    );
    for path in ["download", "denied"] {
        let error = protected_response(
            &client,
            Url::parse(&format!("{origin}/{path}")).unwrap(),
            "PRIVATE-ACCESS-TOKEN",
        )
        .await
        .unwrap_err();
        assert!(!error.contains("PRIVATE") && !error.contains("SECRET") && !error.contains("evil"));
    }
    task.abort();
}

#[tokio::test]
async fn resolution_rejects_untrusted_scene_names_before_network_requests() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    for ids in [
        vec![],
        vec![ID.into(), ID.into()],
        vec!["' or Online eq true".into()],
        vec![ID.into(); 33],
    ] {
        assert!(manager
            .resolve_copernicus_products(ResolveProductsRequest { item_ids: ids })
            .await
            .is_err());
    }
}
