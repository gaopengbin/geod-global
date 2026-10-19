use super::*;
use flate2::{write::GzEncoder, Compression};
use std::io::Write;

fn compressed(text: &str) -> Vec<u8> {
    let mut gzip = GzEncoder::new(Vec::new(), Compression::default());
    gzip.write_all(text.as_bytes()).unwrap();
    gzip.finish().unwrap()
}
#[test]
fn original_export_mime_identifies_retained_gzip_without_changing_other_formats() {
    let gzip = compressed("{\"type\":\"Feature\",\"properties\":{},\"geometry\":null}\n");
    assert_eq!(
        crate::vector::original_content_type(&gzip),
        "application/gzip"
    );
    decode_gzip_jsonl(&gzip).unwrap();
    assert_eq!(
        crate::vector::original_content_type(b"SQLite format 3\0"),
        "application/geopackage+sqlite3"
    );
    assert_eq!(
        crate::vector::original_content_type(b"PK\x03\x04"),
        "application/zip"
    );
    assert_eq!(
        crate::vector::original_content_type(b"{\"type\":\"FeatureCollection\",\"features\":[]}"),
        "application/json"
    );
    assert_eq!(
        crate::vector::original_content_type(&[0x1f, 0x8b, 0x01]),
        "application/json"
    );
}
#[test]
fn official_ms_complete_catalog_uses_original_partition_urls_and_current_aoi() {
    assert_eq!(MS.rows.len(), 30340);
    assert_eq!(
        MS.source_sha256,
        "de61b569b0c23c364d61173632d8c5487ea30bf20912c55b9fa1ab4a89d01148"
    );
    for row in &MS.rows {
        let package = microsoft(row).unwrap();
        features::public_url(&package.url).unwrap();
        features::bounds(package.bounds).unwrap();
    }
    let result = search(SearchRequest {
        dataset_id: "ms-buildings".into(),
        bounds: [1.50, 42.57, 1.53, 42.60],
    })
    .unwrap();
    assert!(result["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["id"] == "Andorra-120222210"));
    let source = package("ms-buildings", "Andorra-120222210").unwrap();
    assert_eq!(source.url, "https://bfppub.z5.web.core.windows.net/2026-08-13/global-buildings.geojsonl/RegionName=Andorra/quadkey=120222210/part-00061-110f5303-ff85-4c71-a2bf-c6070024fec8.c000.csv.gz");
    assert!(search(SearchRequest {
        dataset_id: "ms-buildings".into(),
        bounds: [-180., -85., 180., 85.]
    })
    .unwrap_err()
    .contains("more than 8"));
    assert!(package("ms-buildings", "unknown-120222210").is_err());
    assert!(package("overture-maps", "part-0").is_err());
}
#[test]
fn official_jsonl_complete_conversion_preserves_numbers_identity_geometry_and_missing_sentinels() {
    let a = json!({"type":"Feature","id":"original-a","geometry":{"type":"Polygon","coordinates":[[[1.,2.],[2.,2.],[2.,3.],[1.,2.]]]},"properties":{"height":-1.,"confidence":-1.,"nested":{"source":["original"]}}});
    let b = json!({"type":"Feature","geometry":{"type":"Point","coordinates":[3.,4.]},"properties":{"height":4.201916217803955,"confidence":0.8}});
    let bytes = compressed(&format!("{a}\r\n{b}\n"));
    let data = decode_gzip_jsonl(&bytes).unwrap();
    assert_eq!(data, json!({"type":"FeatureCollection","features":[a,b]}));
    let original = bytes.clone();
    decode_gzip_jsonl(&bytes).unwrap();
    assert_eq!(bytes, original);
}
#[test]
fn official_jsonl_rejects_truncated_crc_trailing_data_bad_rows_and_expansion_bombs() {
    let valid = compressed("{\"type\":\"Feature\",\"properties\":{},\"geometry\":null}\n");
    assert!(decode_gzip_jsonl(&valid[..valid.len() - 1]).is_err());
    let mut corrupted = valid.clone();
    let at = corrupted.len() - 8;
    corrupted[at] ^= 1;
    assert!(decode_gzip_jsonl(&corrupted).is_err());
    let mut appended = valid;
    appended.extend_from_slice(b"not a gzip member");
    assert!(decode_gzip_jsonl(&appended).is_err());
    for invalid in [
        "",
        "{}",
        "{\"type\":\"FeatureCollection\",\"features\":[]}",
        "{\"type\":\"Feature\"}\n\n{\"type\":\"Feature\"}",
    ] {
        assert!(decode_gzip_jsonl(&compressed(invalid)).is_err());
    }
    assert!(decode_gzip_jsonl(&compressed(&"x".repeat(MAX_BYTES + 1)))
        .unwrap_err()
        .contains("Decompressed"));
}
#[test]
fn official_receipts_reject_wrong_source_changed_bytes_and_outside_package_area() {
    let p = geofabrik();
    let mut r = Provenance {
        conversion: 1,
        package: p,
        requested_bounds: [7.41, 43.72, 7.43, 43.74],
        fetched_at: now(),
        bytes: 746384,
        sha256: GEO_HASH.into(),
        selection: "complete publisher original; area selects a package, not a geometry clip"
            .into(),
    };
    r.validate().unwrap();
    r.package.url = "https://example.com/monaco.zip".into();
    assert!(r.validate().is_err());
    r.package = geofabrik();
    r.requested_bounds = [1., 2., 3., 4.];
    assert!(r.validate().is_err());
    r.requested_bounds = [7.41, 43.72, 7.43, 43.74];
    r.sha256 = "0".repeat(64);
    assert!(r.validate().is_err());
    assert!(r.validate_original(b"fake").is_err());
}

#[tokio::test]
#[ignore = "requires exact complete publisher originals in GEOD_OFFICIAL_VECTOR_VERIFICATION_DIR"]
async fn official_complete_originals_preserve_every_geometry_and_property_on_save_and_reopen() {
    let directory = std::path::PathBuf::from(
        std::env::var("GEOD_OFFICIAL_VECTOR_VERIFICATION_DIR")
            .expect("set explicit original-file acceptance directory"),
    );
    let temp = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let mut originals = Vec::new();
    for (dataset, id, file, region, features, sha) in [
        (
            "ms-buildings",
            "Andorra-120222210",
            "ms-Andorra-120222210.csv.gz",
            [1.50, 42.57, 1.53, 42.60],
            1932,
            "6fd8cabe2689532388ed37dfba9f1c5e4e9c326a8d8d01874dde76faae41c604",
        ),
        (
            "geofabrik",
            "monaco-261011-free-shp",
            "monaco-261011-free.shp.zip",
            [7.41, 43.72, 7.43, 43.74],
            8672,
            GEO_HASH,
        ),
    ] {
        let bytes = std::fs::read(directory.join(file)).unwrap();
        assert_eq!(hash(&bytes), sha);
        let package = package(dataset, id).unwrap();
        let mut title = crate::vector::initial(&package.file_name, "managed").unwrap();
        title.official_package = Some(Provenance {
            conversion: 1,
            package,
            requested_bounds: region,
            fetched_at: now(),
            bytes: bytes.len(),
            sha256: hash(&bytes),
            selection: "complete publisher original; area selects a package, not a geometry clip"
                .into(),
        });
        let asset = manager
            .import_vector_bytes(bytes.clone(), title)
            .await
            .unwrap();
        let inspection = manager.inspect_vector(&asset.id).await.unwrap();
        assert_eq!(asset.feature_count, features);
        assert_eq!(asset.source_sha256, sha);
        assert!(asset.official_package.is_some());
        if dataset == "ms-buildings" {
            let exact = decode_gzip_jsonl(&bytes).unwrap();
            assert_eq!(exact, inspection.geojson);
            assert_eq!(
                inspection.geojson["features"][0]["properties"]["height"],
                json!(4.201916217803955)
            );
            assert_eq!(
                inspection.geojson["features"][0]["properties"]["confidence"],
                json!(-1.)
            );
        } else {
            assert_eq!(asset.coordinate_count, 48914);
            assert_eq!(asset.shapefile.as_ref().unwrap().layers.len(), 20);
            assert_eq!(asset.shapefile.as_ref().unwrap().files.len(), 101);
            assert_eq!(
                asset.data_timestamp.as_deref(),
                Some("2026-10-11T20:21:06Z")
            );
        }
        originals.push((inspection, bytes));
    }
    drop(manager);
    let reopened = JobManager::open(temp.path()).await.unwrap();
    for (before, original) in originals {
        let after = reopened.inspect_vector(&before.asset.id).await.unwrap();
        assert_eq!(before.asset, after.asset);
        assert_eq!(before.geojson, after.geojson);
        let source = reopened.inner.vectors.lock().await[&after.asset.id].clone();
        assert_eq!(std::fs::read(&source.path).unwrap(), original);
        assert_eq!(
            reopened
                .vector_original_bytes(&after.asset.id)
                .await
                .unwrap(),
            original
        );
        let outside = tempfile::tempdir().unwrap();
        let export = outside
            .path()
            .join(if after.asset.format == "geojsonl-gzip" {
                "complete-original.csv.gz"
            } else {
                "complete-original.zip"
            });
        reopened
            .export_vector_original_path(&after.asset.id, export.clone())
            .await
            .unwrap();
        let exported = std::fs::read(export).unwrap();
        assert_eq!(exported, original);
        assert_eq!(hash(&exported), after.asset.source_sha256);
        if after.asset.format == "geojsonl-gzip" {
            use axum::{
                body::{to_bytes, Body},
                http::{header, Request, StatusCode},
            };
            use tower::ServiceExt;
            assert_eq!(
                crate::vector::original_content_type(&exported),
                "application/gzip"
            );
            assert_eq!(decode_gzip_jsonl(&exported).unwrap(), after.geojson);
            let response = crate::service::router(reopened.clone())
                .oneshot(
                    Request::builder()
                        .uri(format!("/vectors/{}/source", after.asset.id))
                        .header(header::HOST, "127.0.0.1:4318")
                        .header(header::ORIGIN, crate::service::ALLOWED_ORIGIN)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::OK);
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/gzip");
            let http_original = to_bytes(response.into_body(), MAX_BYTES + 1).await.unwrap();
            assert_eq!(http_original.as_ref(), original.as_slice());
            assert_eq!(hash(&http_original), after.asset.source_sha256);
        }
        println!(
            "{}",
            json!({"datasetId":after.asset.official_package.as_ref().unwrap().package.dataset_id,
            "assetId":after.asset.id,"bytes":after.asset.bytes,"sourceSha256":after.asset.source_sha256,
            "geojsonSha256":after.asset.geojson_sha256,"features":after.asset.feature_count,
            "coordinates":after.asset.coordinate_count,"reopenedEveryGeometryAndProperty":true,
            "originalFileRetainedByteIdentically":true})
        );
    }
}
