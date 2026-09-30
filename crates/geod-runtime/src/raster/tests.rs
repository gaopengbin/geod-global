use super::*;
use std::collections::BTreeMap;
use tiff::encoder::{colortype, TiffEncoder};

#[tokio::test]
async fn interactive_queries_wait_for_previous_worker_instead_of_failing_immediately() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let job = record(&manager.inner.root, &fixture(1, 1, &[4], 32610, false));
    manager
        .inner
        .store
        .lock()
        .await
        .jobs
        .insert(job.id.clone(), job.clone());

    let permit = manager
        .inner
        .raster_permits
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    let (result, ()) = tokio::join!(manager.inspect_raster(&job.id), async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        drop(permit);
    });
    assert_eq!(result.unwrap().width, 1);

    let permit = manager
        .inner
        .raster_permits
        .clone()
        .acquire_owned()
        .await
        .unwrap();
    let (result, ()) = tokio::join!(manager.sample_raster(&job.id, 500001.0, 4199999.0), async {
        tokio::time::sleep(Duration::from_millis(20)).await;
        drop(permit);
    });
    assert_eq!(result.unwrap().value, 4);
}

pub(crate) fn fixture(width: u32, height: u32, pixels: &[u8], epsg: u16, matrix: bool) -> Vec<u8> {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut buffer).unwrap();
        let mut image = encoder
            .new_image::<colortype::Gray8>(width, height)
            .unwrap();
        let keys = [
            1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, epsg, 3076, 0, 1, 9001,
        ];
        image
            .encoder()
            .write_tag(Tag::GeoKeyDirectoryTag, &keys[..])
            .unwrap();
        if matrix {
            let transform = [
                20.0, 0.0, 0.0, 500000.0, 0.0, -20.0, 0.0, 4200000.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
                0.0, 1.0,
            ];
            image
                .encoder()
                .write_tag(Tag::ModelTransformationTag, &transform[..])
                .unwrap();
        } else {
            image
                .encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[20.0f64, 20.0, 0.0][..])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::ModelTiepointTag,
                    &[0.0f64, 0.0, 0.0, 500000.0, 4200000.0, 0.0][..],
                )
                .unwrap();
        }
        image.encoder().write_tag(Tag::GdalNodata, "0").unwrap();
        image.write_data(pixels).unwrap();
    }
    buffer.into_inner()
}

pub(crate) fn record(root: &Path, bytes: &[u8]) -> Job {
    let id = uuid::Uuid::new_v4().to_string();
    let assets = root.join("assets");
    std::fs::create_dir_all(&assets).unwrap();
    let path = assets.join(format!("{id}.tif"));
    std::fs::write(&path, bytes).unwrap();
    Job {
        id,
        item_id: "S2_TEST".into(),
        asset_key: "scl".into(),
        href:
            "https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/S2_TEST/SCL.tif"
                .into(),
        media_type: "image/tiff; application=geotiff".into(),
        title: "Test".into(),
        status: JobStatus::Succeeded,
        bytes_downloaded: bytes.len() as u64,
        total_bytes: Some(bytes.len() as u64),
        sha256: Some(format!("{:x}", Sha256::digest(bytes))),
        output_path: Some(path.to_string_lossy().into_owned()),
        error: None,
        created_at: "2026-09-22T00:00:00Z".into(),
        updated_at: "2026-09-22T00:00:00Z".into(),
        source: "test fixture".into(),
        validation: "test".into(),
        attempts: 1,
        kind: "download".into(),
        parent_id: None,
        recipe: None,
        crop: None,
        mosaic: None,
        mosaic_output: None,
        manifest_path: None,
    }
}

fn png_bytes(inspection: &RasterInspection) -> (Vec<u8>, png::OutputInfo) {
    let encoded = STANDARD
        .decode(
            inspection
                .preview_data_url
                .strip_prefix("data:image/png;base64,")
                .unwrap(),
        )
        .unwrap();
    let mut reader = png::Decoder::new(Cursor::new(encoded)).read_info().unwrap();
    let mut buffer = vec![0; reader.output_buffer_size().unwrap()];
    let info = reader.next_frame(&mut buffer).unwrap();
    (buffer, info)
}

#[test]
fn real_tiff_geotags_counts_checksum_and_png_pixels_match() {
    for matrix in [false, true] {
        let directory = tempfile::tempdir().unwrap();
        let bytes = fixture(3, 2, &[0, 4, 4, 6, 9, 11], 32610, matrix);
        let job = record(directory.path(), &bytes);
        let inspection = inspect_download(directory.path(), &job).unwrap();
        assert_eq!(
            (inspection.width, inspection.height, inspection.band_count),
            (3, 2, 1)
        );
        assert_eq!(inspection.data_type, "UInt8");
        assert_eq!(inspection.crs, "EPSG:32610");
        assert_eq!(
            inspection.bounds,
            [500000.0, 4199960.0, 500060.0, 4200000.0]
        );
        assert_eq!(inspection.pixel_size, [20.0, 20.0]);
        assert_eq!(inspection.nodata, Some(0));
        assert_eq!(inspection.sha256, job.sha256.unwrap());
        assert_eq!(inspection.classes.iter().map(|c| c.count).sum::<u64>(), 6);
        assert_eq!(inspection.classes[4].count, 2);
        assert_eq!(inspection.classes[6].color, "#0000ff");
        let (pixels, info) = png_bytes(&inspection);
        assert_eq!(
            (info.width, info.height, info.color_type),
            (3, 2, png::ColorType::Rgba)
        );
        assert_eq!(&pixels[..8], &[0, 0, 0, 0, 0, 160, 0, 255]);
        assert_eq!(&pixels[12..16], &[0, 0, 255, 255]);
    }
}

#[test]
fn pixel_queries_use_full_resolution_grid_not_preview_and_reject_outer_edges() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = fixture(3, 2, &[0, 4, 4, 6, 9, 11], 32610, false);
    let job = record(directory.path(), &bytes);
    for (x, y, expected_pixel, expected_value) in [
        (500000.0, 4200000.0, [0, 0], 0),
        (500020.0, 4199980.0, [1, 1], 9),
        (500059.9, 4199960.1, [2, 1], 11),
    ] {
        let decoded = load_verified_raster(directory.path(), &job, None).unwrap();
        let sample = sample_pixel(decoded, &job.id, x, y).unwrap();
        assert_eq!(sample.pixel, expected_pixel);
        assert_eq!(sample.value, expected_value);
        assert_eq!(sample.is_no_data, expected_value == 0);
        assert_eq!(sample.sha256, job.sha256.clone().unwrap());
        assert_eq!(
            sample.center,
            [
                500010.0 + expected_pixel[0] as f64 * 20.0,
                4199990.0 - expected_pixel[1] as f64 * 20.0
            ]
        );
    }
    for (x, y) in [
        (500060.0, 4199980.0),
        (500000.0, 4199960.0),
        (499999.0, 4200000.0),
        (500010.0, 4200001.0),
        (f64::NAN, 4200000.0),
    ] {
        let decoded = load_verified_raster(directory.path(), &job, None).unwrap();
        assert!(sample_pixel(decoded, &job.id, x, y).is_err());
    }
}

#[tokio::test]
async fn pixel_api_rechecks_source_hash_and_keeps_http_boundary() {
    use axum::{
        body::{to_bytes, Body},
        http::Request,
    };
    use tower::ServiceExt;
    let directory = tempfile::tempdir().unwrap();
    let bytes = fixture(2, 2, &[0, 4, 6, 9], 32610, false);
    let job = record(directory.path(), &bytes);
    std::fs::write(
        directory.path().join("jobs.json"),
        serde_json::to_vec(&BTreeMap::from([(job.id.clone(), job.clone())])).unwrap(),
    )
    .unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let app = crate::service::router(manager.clone());
    let route = format!("/jobs/{}/pixel?x=500030&y=4199970", job.id);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&route)
                .header("host", "127.0.0.1:4318")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), 200);
    let result: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 8192).await.unwrap()).unwrap();
    assert_eq!(result["value"], 9);
    assert_eq!(result["pixel"], serde_json::json!([1, 1]));
    let denied = app
        .oneshot(
            Request::builder()
                .uri(&route)
                .header("host", "127.0.0.1:4318")
                .header("origin", "https://untrusted.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), 403);
    std::fs::write(job.output_path.unwrap(), vec![0u8; bytes.len()]).unwrap();
    assert!(manager
        .sample_raster(&job.id, 500030.0, 4199970.0)
        .await
        .unwrap_err()
        .contains("SHA-256"));
}

#[test]
fn preview_uses_exact_nearest_samples_and_does_not_exceed_768() {
    let directory = tempfile::tempdir().unwrap();
    let values: Vec<_> = (0..1000 * 4).map(|index| (index % 12) as u8).collect();
    let bytes = fixture(1000, 4, &values, 32756, false);
    let job = record(directory.path(), &bytes);
    let inspection = inspect_download(directory.path(), &job).unwrap();
    assert_eq!(
        (inspection.preview_width, inspection.preview_height),
        (768, 3)
    );
    let (pixels, _) = png_bytes(&inspection);
    for y in 0..3usize {
        for x in 0..768usize {
            let value = values[(y * 4 / 3) * 1000 + x * 1000 / 768];
            assert_eq!(
                &pixels[(y * 768 + x) * 4..(y * 768 + x) * 4 + 3],
                &PALETTE[value as usize].1
            );
        }
    }
}

#[test]
fn rejects_changed_missing_corrupt_and_unmanaged_files() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = fixture(2, 1, &[4, 6], 32610, false);
    let mut job = record(directory.path(), &bytes);
    let original = job.clone();
    job.sha256 = Some("0".repeat(64));
    assert!(inspect_download(directory.path(), &job)
        .unwrap_err()
        .contains("SHA-256 mismatch"));
    job = original.clone();
    job.bytes_downloaded += 1;
    assert!(inspect_download(directory.path(), &job)
        .unwrap_err()
        .contains("size"));
    job = original.clone();
    job.asset_key = "visual".into();
    assert!(inspect_download(directory.path(), &job)
        .unwrap_err()
        .contains("supports only"));
    job = original.clone();
    job.media_type = "image/jpeg".into();
    assert!(inspect_download(directory.path(), &job).is_err());
    job = original.clone();
    job.status = JobStatus::Failed;
    assert!(inspect_download(directory.path(), &job).is_err());
    job = original.clone();
    let outside = directory.path().join("outside.tif");
    std::fs::write(&outside, &bytes).unwrap();
    job.output_path = Some(outside.to_string_lossy().into_owned());
    assert!(inspect_download(directory.path(), &job)
        .unwrap_err()
        .contains("managed"));
    std::fs::remove_file(original.output_path.as_ref().unwrap()).unwrap();
    assert!(inspect_download(directory.path(), &original).is_err());
    let corrupt = record(directory.path(), b"II\x2a\0broken");
    assert!(inspect_download(directory.path(), &corrupt)
        .unwrap_err()
        .contains("Cannot inspect"));
}

#[test]
fn rejects_unsupported_crs_class_values_and_missing_geotags() {
    let directory = tempfile::tempdir().unwrap();
    let unsupported = record(directory.path(), &fixture(2, 1, &[4, 6], 4326, false));
    assert!(inspect_download(directory.path(), &unsupported)
        .unwrap_err()
        .contains("WGS84 UTM"));
    let values = record(directory.path(), &fixture(2, 1, &[4, 255], 32610, false));
    assert!(inspect_download(directory.path(), &values)
        .unwrap_err()
        .contains("classes 0-11"));
    let mut buffer = Cursor::new(Vec::new());
    TiffEncoder::new(&mut buffer)
        .unwrap()
        .write_image::<colortype::Gray8>(2, 1, &[4, 6])
        .unwrap();
    let missing = record(directory.path(), &buffer.into_inner());
    assert!(inspect_download(directory.path(), &missing)
        .unwrap_err()
        .contains("projection keys"));
    let mut buffer = Cursor::new(Vec::new());
    TiffEncoder::new(&mut buffer)
        .unwrap()
        .write_image::<colortype::RGB8>(1, 1, &[4, 6, 9])
        .unwrap();
    let rgb = record(directory.path(), &buffer.into_inner());
    assert!(inspect_download(directory.path(), &rgb)
        .unwrap_err()
        .contains("single-band"));
}

#[test]
fn invalid_georeferencing_is_rejected_without_guessed_bounds() {
    assert!(georeference(2, 2, None, Some(&[20.0, 20.0, 0.0]), None).is_err());
    assert!(georeference(2, 2, None, Some(&[f64::NAN, 20.0, 0.0]), Some(&[0.0; 6])).is_err());
    assert!(georeference(2, 2, Some(&[0.0; 16]), None, None).is_err());
    let tiepoint = [2.0, 3.0, 0.0, 500040.0, 4199940.0, 0.0];
    assert_eq!(
        georeference(2, 2, None, Some(&[20.0, 20.0, 0.0]), Some(&tiepoint))
            .unwrap()
            .0,
        [500000.0, 4199960.0, 500040.0, 4200000.0]
    );
    assert!(validate_geokeys(&[1, 1, 0, 100]).is_err());
}

#[tokio::test]
async fn manager_uses_persisted_job_and_rechecks_changed_file_each_time() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = fixture(2, 1, &[4, 6], 32610, false);
    let job = record(directory.path(), &bytes);
    let mut records = BTreeMap::new();
    records.insert(job.id.clone(), job.clone());
    std::fs::write(
        directory.path().join("jobs.json"),
        serde_json::to_vec(&records).unwrap(),
    )
    .unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    assert_eq!(
        manager.inspect_raster(&job.id).await.unwrap().classes[6].count,
        1
    );
    let permit = manager
        .inner
        .raster_permits
        .clone()
        .try_acquire_owned()
        .unwrap();
    assert!(manager
        .inspect_raster(&job.id)
        .await
        .unwrap_err()
        .contains("busy"));
    drop(permit);
    let mut changed = bytes;
    let last = changed.len() - 1;
    changed[last] ^= 1;
    std::fs::write(job.output_path.as_ref().unwrap(), changed).unwrap();
    assert!(manager
        .inspect_raster(&job.id)
        .await
        .unwrap_err()
        .contains("SHA-256 mismatch"));
}

#[test]
fn rejects_oversized_decoded_dimensions_before_reading_pixels() {
    let directory = tempfile::tempdir().unwrap();
    let bytes = fixture(20000, 1, &vec![4; 20000], 32610, false);
    let job = record(directory.path(), &bytes);
    assert!(inspect_download(directory.path(), &job)
        .unwrap_err()
        .contains("dimensions exceed"));
}

#[tokio::test]
async fn raster_api_serializes_metadata_and_retains_origin_guard() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let directory = tempfile::tempdir().unwrap();
    let job = record(directory.path(), &fixture(2, 1, &[4, 6], 32610, false));
    let records = BTreeMap::from([(job.id.clone(), job.clone())]);
    std::fs::write(
        directory.path().join("jobs.json"),
        serde_json::to_vec(&records).unwrap(),
    )
    .unwrap();
    let app = crate::service::router(JobManager::open(directory.path()).await.unwrap());
    let address = format!("/jobs/{}/raster", job.id);
    let blocked = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&address)
                .header("Host", "127.0.0.1:4318")
                .header("Origin", "https://evil.example")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked.status(), StatusCode::FORBIDDEN);
    let response = app
        .oneshot(
            Request::builder()
                .uri(address)
                .header("Host", "127.0.0.1:4318")
                .header("Origin", crate::service::ALLOWED_ORIGIN)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let data: serde_json::Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 10000).await.unwrap()).unwrap();
    assert_eq!(data["bandCount"], 1);
    assert_eq!(data["pixelSize"], serde_json::json!([20.0, 20.0]));
    assert!(data["previewDataUrl"]
        .as_str()
        .unwrap()
        .starts_with("data:image/png;base64,"));
    assert_eq!(data["sha256"], job.sha256.unwrap());
}
