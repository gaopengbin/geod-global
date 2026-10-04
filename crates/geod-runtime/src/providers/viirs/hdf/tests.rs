use super::*;
use std::io::Write;

const FIXTURES: [&[u8]; 3] = [
    include_bytes!("../../../../fixtures/viirs/synthetic-vnp09a1.h5.gz"),
    include_bytes!("../../../../fixtures/viirs/synthetic-vj109a1.h5.gz"),
    include_bytes!("../../../../fixtures/viirs/synthetic-vj209a1.h5.gz"),
];
fn fixture(index: usize) -> (Vec<u8>, serde_json::Value) {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/viirs/expected.json")).unwrap();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(FIXTURES[index])
        .read_to_end(&mut bytes)
        .unwrap();
    (bytes, expected["fixtures"][index].clone())
}
fn check(bytes: &[u8], item: &str) -> Result<ScienceSummary> {
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(bytes).unwrap();
    let path = file.into_temp_path();
    verify(
        &path,
        item,
        bytes.len() as u64,
        &format!("{:x}", Sha256::digest(bytes)),
        &CancellationToken::new(),
    )
}

#[test]
fn all_original_pixels_match_independent_h5py_across_storage_endian_and_superblocks() {
    let mut summaries = Vec::new();
    for index in 0..3 {
        let (bytes, expected) = fixture(index);
        assert_eq!(bytes.len() as u64, expected["byteCount"].as_u64().unwrap());
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            expected["sourceSha256"]
        );
        let summary = check(&bytes, expected["itemId"].as_str().unwrap()).unwrap();
        for (band, independent) in summary
            .bands
            .iter()
            .zip(expected["bands"].as_array().unwrap())
        {
            assert_eq!(band.samples_sha256, independent["samplesSha256"]);
            assert_eq!(
                u64::from(band.no_data_count),
                independent["noDataCount"].as_u64().unwrap()
            );
            assert_eq!(
                u64::from(band.outside_valid_range_count),
                independent["outsideValidRangeCount"].as_u64().unwrap()
            );
            assert_eq!(
                i64::from(band.minimum.unwrap()),
                independent["minimum"].as_i64().unwrap()
            );
            assert_eq!(
                i64::from(band.maximum.unwrap()),
                independent["maximum"].as_i64().unwrap()
            );
            assert_eq!(band.sample_count, 1_440_000);
        }
        assert!(!summary.quality_mask_applied);
        assert_eq!(summary.crs, CRS);
        summaries.push(summary);
    }
    if let Ok(path) = std::env::var("GEOD_VIIRS_FIXTURE_EVIDENCE") {
        std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({
            "provenance": "Synthetic HDF5 fixture validation only; not NASA observations or authenticated access",
            "summaries": summaries,
        })).unwrap()).unwrap();
    }
}

#[test]
fn checksum_size_cancellation_wrong_product_and_signature_only_never_validate() {
    let (bytes, expected) = fixture(0);
    let item = expected["itemId"].as_str().unwrap();
    let mut file = tempfile::NamedTempFile::new().unwrap();
    file.write_all(&bytes).unwrap();
    let path = file.into_temp_path();
    let token = CancellationToken::new();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    assert!(
        verify(&path, item, bytes.len() as u64, &"0".repeat(64), &token)
            .unwrap_err()
            .contains("checksum")
    );
    assert!(verify(&path, item, bytes.len() as u64 - 1, &hash, &token)
        .unwrap_err()
        .contains("byte count"));
    assert!(check(&bytes, &item.replace("VNP", "VJ1"))
        .unwrap_err()
        .contains("LocalGranuleID"));
    assert!(check(b"\x89HDF\r\n\x1a\nsignature only", item).is_err());
    assert!(check(&bytes[..bytes.len() / 2], item).is_err());
    token.cancel();
    assert!(verify(&path, item, bytes.len() as u64, &hash, &token)
        .unwrap_err()
        .contains("cancelled"));
}

#[test]
fn odl_rejects_duplicate_grid_axis_order_projection_and_tile_spoofing() {
    let (bytes, _) = fixture(0);
    let file = Hdf5File::from_vec(bytes).unwrap();
    let text = file
        .dataset("/HDFEOS INFORMATION/StructMetadata.0")
        .unwrap()
        .read_string()
        .unwrap();
    assert!(odl::grid(&text, 8, 5).is_ok());
    for changed in [
        text.replace("XDim=1200", "XDim=1200\nXDim=1200"),
        text.replace("XDim=1200", "XDim=1201"),
        text.replace("GCTP_SNSOID", "GCTP_GEO"),
        text.replace("6371007.181", "6378137.0"),
        text.replace("(\"YDim\",\"XDim\")", "(\"XDim\",\"YDim\")"),
        text.replace("HDFE_GD_UL", "HDFE_GD_LL"),
        text.replace("END_GROUP=GRID_1", "END_GROUP=GRID_2"),
        format!("{text}\nXDim=1200"),
    ] {
        assert!(odl::grid(&changed, 8, 5).is_err(), "{changed}");
    }
    assert!(odl::grid(&text, 9, 5).is_err());
}

#[test]
fn persisted_summary_cannot_change_calibration_identity_counts_or_geometry() {
    let (bytes, expected) = fixture(0);
    let item = expected["itemId"].as_str().unwrap();
    let summary = check(&bytes, item).unwrap();
    let hash = summary.source_sha256.clone();
    let mut changed = summary.clone();
    changed.bands[0].scale = 0.01;
    assert!(changed.validate(item, &hash).is_err());
    let mut changed = summary.clone();
    changed.bounds[0] += 10.0;
    assert!(changed.validate(item, &hash).is_err());
    let mut changed = summary.clone();
    changed.bands[0].no_data_count = u32::MAX;
    assert!(changed.validate(item, &hash).is_err());
    assert!(summary
        .validate(&item.replace("002", "001"), &hash)
        .is_err());
    assert!(summary.validate(item, &"0".repeat(64)).is_err());
}

#[test]
fn malformed_metadata_calibration_unsigned_oversize_external_and_damaged_chunks_are_rejected() {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/viirs/expected.json")).unwrap();
    for record in expected["negatives"].as_array().unwrap() {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("fixtures/viirs")
            .join(record["file"].as_str().unwrap());
        let bytes = std::fs::read(path).unwrap();
        let mut original = Vec::new();
        flate2::read::GzDecoder::new(bytes.as_slice())
            .read_to_end(&mut original)
            .unwrap();
        let result = check(&original, record["itemId"].as_str().unwrap());
        assert!(
            result.is_err(),
            "Accepted negative fixture: {}",
            record["fault"]
        );
    }
}

#[tokio::test]
async fn managed_download_validates_before_commit_and_keeps_summary_after_restart() {
    use crate::{CreateJobRequest, JobManager, JobStatus};
    use axum::{routing::get, Router};
    let (bytes, expected) = fixture(0);
    let good = bytes.clone();
    let bad = b"\x89HDF\r\n\x1a\nsignature-only-not-a-product".to_vec();
    let app = Router::new()
        .route(
            "/good.h5",
            get(move || {
                let bytes = good.clone();
                async move { bytes }
            }),
        )
        .route(
            "/bad.h5",
            get(move || {
                let bytes = bad.clone();
                async move { bytes }
            }),
        );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open_inner(dir.path(), Some(origin.clone()))
        .await
        .unwrap();
    let request = |path: &str| CreateJobRequest {
        item_id: expected["itemId"].as_str().unwrap().into(),
        asset_key: "viirs".into(),
        href: format!("{origin}/{path}.h5"),
        media_type: "application/x-hdf5".into(),
        title: Some("SYNTHETIC VIIRS acceptance fixture".into()),
    };
    let job = manager.create(request("good")).await.unwrap();
    let succeeded = manager.wait(&job.id).await.unwrap();
    assert_eq!(succeeded.status, JobStatus::Succeeded);
    assert_eq!(succeeded.bytes_downloaded, bytes.len() as u64);
    assert_eq!(
        std::fs::read(succeeded.output_path.as_ref().unwrap()).unwrap(),
        bytes
    );
    let summary = succeeded.viirs_science.unwrap();
    assert_eq!(summary.source_sha256, succeeded.sha256.unwrap());
    assert!(succeeded.validation.contains("M5/M4/M3"));
    let bad = manager.create(request("bad")).await.unwrap();
    let failed = manager.wait(&bad.id).await.unwrap();
    assert_eq!(failed.status, JobStatus::Failed);
    assert!(
        failed.viirs_science.is_none() && failed.output_path.is_none() && failed.sha256.is_none()
    );
    assert!(!dir
        .path()
        .join("assets")
        .join(format!("{}.h5", bad.id))
        .exists());
    assert!(!dir
        .path()
        .join("assets")
        .join(format!("{}.part", bad.id))
        .exists());
    drop(manager);
    let reopened = JobManager::open_inner(dir.path(), Some(origin))
        .await
        .unwrap();
    assert_eq!(
        reopened.get(&job.id).await.unwrap().viirs_science,
        Some(summary)
    );
    assert!(reopened.get(&bad.id).await.unwrap().viirs_science.is_none());
    server.abort();
}
