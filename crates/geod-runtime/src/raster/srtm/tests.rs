use super::*;
use crate::{new_download_job, CreateJobRequest};
use std::io::Write;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

const ID: &str = "N37W123.SRTMGL1.hgt";
fn fixture(name: &str, bytes: &[u8]) -> Vec<u8> {
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            name,
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(bytes).unwrap();
    writer.finish().unwrap().into_inner()
}
fn grid() -> Vec<u8> {
    let mut data = Vec::with_capacity(srtm::HGT_BYTES as usize);
    for y in 0..srtm::EDGE {
        for x in 0..srtm::EDGE {
            let v = match (x, y) {
                (0, 0) => i16::MIN,
                (3600, 0) => -32767,
                (0, 3600) => -27,
                (3600, 3600) => 32767,
                _ => ((x * 3 + y) % 5000) as i16 - 500,
            };
            data.extend_from_slice(&v.to_be_bytes());
        }
    }
    data
}
fn record(root: &Path, bytes: &[u8]) -> Job {
    let mut job = new_download_job(CreateJobRequest {
        item_id: ID.into(),
        asset_key: "srtm".into(),
        media_type: "application/zip".into(),
        href: format!(
            "https://{}{}{ID}/{ID}.zip",
            providers::nasa::HOST,
            srtm::PREFIX
        ),
        title: None,
    });
    let path = root.join("assets").join(format!("{}.zip", job.id));
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(&path, bytes).unwrap();
    job.status = JobStatus::Succeeded;
    job.bytes_downloaded = bytes.len() as u64;
    job.total_bytes = Some(bytes.len() as u64);
    job.sha256 = Some(format!("{:x}", Sha256::digest(bytes)));
    job.output_path = Some(path.to_string_lossy().into_owned());
    job
}

#[test]
fn srtm_identity_is_product_version_cell_and_key_specific() {
    let root = tempfile::tempdir().unwrap();
    let job = record(root.path(), b"unused");
    let url = providers::asset_url(&job.href).unwrap();
    assert!(providers::matches_item(&url, ID, "srtm"));
    assert!(!providers::matches_item(&url, ID, "elevation"));
    assert!(!providers::matches_item(
        &url,
        "N38W123.SRTMGL1.hgt",
        "srtm"
    ));
    for id in [
        "S00E001.SRTMGL1.hgt",
        "N60E010.SRTMGL1.hgt",
        "S57E001.SRTMGL1.hgt",
        "N00W000.SRTMGL1.hgt",
        "N37W123.SRTMGL3.hgt",
        "N00E180.SRTMGL1.hgt",
        "N中W123.SRTMGL1.hgt",
    ] {
        assert!(srtm::cell(id).is_none());
    }
    for href in [
        format!("{}?token=SECRET", job.href),
        job.href.replace(".003/", ".002/"),
        job.href.replace("/lp-prod-protected/", "/lp-prod-public/"),
        job.href.replace(".hgt.zip", ".num.zip"),
    ] {
        assert!(providers::asset_url(&href).is_err());
    }
    let mut request = CreateJobRequest {
        item_id: ID.into(),
        asset_key: "srtm".into(),
        href: job.href,
        media_type: "image/tiff".into(),
        title: None,
    };
    assert!(crate::validate_request(&request, None).is_err());
    request.media_type = "application/zip".into();
    assert!(crate::validate_request(&request, None).is_ok());
}

#[test]
fn srtm_archive_rejects_wrong_name_grid_duplicate_traversal_and_crc() {
    let bytes = grid();
    let read =
        |z: Vec<u8>| srtm::read_hgt(Cursor::new(z), ID, Instant::now() + Duration::from_secs(30));
    assert_eq!(read(fixture("N37W123.hgt", &bytes)).unwrap(), bytes);
    assert_eq!(read(fixture(ID, &bytes)).unwrap(), bytes);
    for name in [
        "../N37W123.hgt",
        "N38W123.hgt",
        "data/N37W123.hgt",
        "N37W123.num",
    ] {
        assert!(read(fixture(name, &bytes)).is_err());
    }
    assert!(read(fixture("N37W123.hgt", &bytes[..bytes.len() - 2])).is_err());
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    for name in ["N37W123.hgt", ID] {
        writer
            .start_file(name, SimpleFileOptions::default())
            .unwrap();
        writer.write_all(&bytes).unwrap();
    }
    assert!(read(writer.finish().unwrap().into_inner()).is_err());
    let mut corrupt = fixture(ID, &bytes);
    let offset = corrupt.windows(4).position(|v| v == b"PK\x01\x02").unwrap();
    corrupt[offset + 16] ^= 0xff; // central directory CRC must also be checked
    assert!(read(corrupt).is_err());
}

#[tokio::test]
async fn srtm_original_signed_heights_point_corners_mask_and_persistent_thumbnail() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let job = record(&manager.inner.root, &fixture("N37W123.hgt", &grid()));
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.insert(job.id.clone(), job.clone());
        manager.persist(&store.jobs).await.unwrap();
    }
    let metadata = manager.inspect_raster(&job.id).await.unwrap();
    assert_eq!(
        (metadata.width, metadata.height, metadata.band_count),
        (3601, 3601, 1)
    );
    assert_eq!(metadata.data_type, "Int16");
    assert_eq!(metadata.nodata, Some(-32768.0));
    let profile = metadata.elevation.unwrap();
    assert_eq!(profile.vertical_reference, "EPSG:5773");
    assert_eq!(profile.product, "srtmgl1-v003");
    assert_eq!(profile.pixel_interpretation, "PixelIsPoint");
    assert_eq!(
        metadata.bounds,
        [
            -123.0 - SPACING / 2.0,
            37.0 - SPACING / 2.0,
            -122.0 + SPACING / 2.0,
            38.0 + SPACING / 2.0
        ]
    );
    for (x, y, want) in [
        (0, 0, -32768),
        (3600, 0, -32767),
        (0, 3600, -27),
        (3600, 3600, 32767),
        (7, 5, -474),
    ] {
        let point = manager
            .sample_raster(
                &job.id,
                -123.0 + x as f64 * SPACING,
                38.0 - y as f64 * SPACING,
            )
            .await
            .unwrap();
        assert_eq!(point.pixel, [x, y]);
        assert_eq!(point.value, f64::from(want));
        assert_eq!(point.is_no_data, want == -32768);
    }
    assert!(manager
        .sample_raster(&job.id, metadata.bounds[2], 38.0)
        .await
        .is_err());
    let thumbnail = manager.file_thumbnail(&job.id).await.unwrap();
    let cached = directory.path().join("cache/thumbnails/v1");
    let paths: Vec<_> = std::fs::read_dir(&cached)
        .unwrap()
        .map(|p| p.unwrap().path())
        .collect();
    assert_eq!(paths.len(), 1);
    let content = std::fs::read(&paths[0]).unwrap();
    let png = STANDARD
        .decode(
            thumbnail
                .data_url
                .strip_prefix("data:image/png;base64,")
                .unwrap(),
        )
        .unwrap();
    let mut decoder = png::Decoder::new(Cursor::new(png)).read_info().unwrap();
    let mut rgba = vec![0; decoder.output_buffer_size().unwrap()];
    decoder.next_frame(&mut rgba).unwrap();
    assert_eq!(&rgba[..4], &[0, 0, 0, 0]);
    assert_eq!(rgba[7], 255);
    drop(manager);
    let reopened = JobManager::open(directory.path()).await.unwrap();
    assert_eq!(
        reopened.file_thumbnail(&job.id).await.unwrap().data_url,
        thumbnail.data_url
    );
    assert_eq!(std::fs::read(&paths[0]).unwrap(), content);
    // A changed original invalidates the cache and fails scientific inspection.
    std::fs::write(job.output_path.unwrap(), b"tampered").unwrap();
    assert!(reopened.file_thumbnail(&job.id).await.is_err());
    assert!(reopened.inspect_raster(&job.id).await.is_err());
}
