use super::*;
use std::io::Cursor;

const FIXTURE: &[u8] = include_bytes!("../../fixtures/safe/synthetic-l2a.zip");
const ITEM: &str = "S2C_MSIL2A_20250627T184941_N0511_R113_T10SEG_20250627T234511";
const HREF: &str = "https://download.dataspace.copernicus.eu/odata/v1/Products(0d695b42-4b24-4954-ba09-f2a44303fdd8)/$value";

fn source(root: &Path, data: &[u8]) -> Job {
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let mut job = crate::new_download_job(crate::CreateJobRequest {
        item_id: ITEM.into(),
        asset_key: "product".into(),
        href: HREF.into(),
        media_type: "application/zip".into(),
        title: None,
    });
    let path = root.join("assets").join(format!("{}.zip", job.id));
    std::fs::write(&path, data).unwrap();
    job.status = JobStatus::Succeeded;
    job.bytes_downloaded = data.len() as u64;
    job.total_bytes = Some(data.len() as u64);
    job.sha256 = Some(format!("{:x}", Sha256::digest(data)));
    job.output_path = Some(path.to_string_lossy().into_owned());
    job
}
fn preparation(source: &Job, key: &str) -> Job {
    let mut job = source.clone();
    job.id = Uuid::new_v4().to_string();
    job.kind = "raster_prepare".into();
    job.asset_key = key.into();
    job.parent_id = Some(source.id.clone());
    job.media_type = "image/tiff".into();
    job.safe = Some(SafeSpec {
        source_job_id: source.id.clone(),
        source_sha256: source.sha256.clone().unwrap(),
    });
    job.status = JobStatus::Queued;
    job.sha256 = None;
    job.output_path = None;
    job
}
fn changed(mut change: impl FnMut(&str, &mut Vec<u8>)) -> Vec<u8> {
    let mut archive = zip::ZipArchive::new(Cursor::new(FIXTURE)).unwrap();
    let mut out = zip::ZipWriter::new(Cursor::new(Vec::new()));
    for index in 0..archive.len() {
        let mut entry = archive.by_index(index).unwrap();
        let name = entry.name().to_owned();
        let mut bytes = Vec::new();
        entry.read_to_end(&mut bytes).unwrap();
        change(&name, &mut bytes);
        out.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Deflated),
        )
        .unwrap();
        out.write_all(&bytes).unwrap();
    }
    out.finish().unwrap().into_inner()
}
fn expected(key: &str) -> Vec<u8> {
    if key == "visual" {
        (0..520)
            .flat_map(|y| {
                (0..64)
                    .flat_map(move |x| (0..3).map(move |b| ((x * 7 + y * 3 + b * 31) % 256) as u8))
            })
            .collect()
    } else {
        (0..260)
            .flat_map(|y| (0..32).map(move |x| ((x + y) % 12) as u8))
            .collect()
    }
}

#[test]
fn synthetic_gdal_jp2_preserves_every_tci_and_scl_sample_across_strips() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let source = source(&root, FIXTURE);
    let cancel = CancellationToken::new();
    for key in ["visual", "scl"] {
        let job = preparation(&source, key);
        let (grid, bytes, hash) = convert(&root, &job, &source, &cancel, None).unwrap();
        let path = root.join("assets").join(format!("{}.tif", job.id));
        let mut decoder = Decoder::new(BufReader::new(File::open(&path).unwrap())).unwrap();
        let DecodingResult::U8(pixels) = decoder.read_image().unwrap() else {
            panic!("UInt8 required")
        };
        assert_eq!(pixels, expected(key));
        assert_eq!(
            grid.pixel_size,
            if key == "visual" {
                [10.0; 2]
            } else {
                [20.0; 2]
            }
        );
        assert_eq!(grid.bounds, [500000.0, 4194800.0, 500640.0, 4200000.0]);
        assert_eq!(grid.crs, "EPSG:32610");
        assert_eq!(bytes, std::fs::metadata(path).unwrap().len());
        assert_eq!(hash.len(), 64);
    }
}

#[test]
fn rejects_changed_archive_pin_and_cancel_leaves_no_committed_output() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let source = source(&root, FIXTURE);
    let mut job = preparation(&source, "scl");
    job.safe.as_mut().unwrap().source_sha256 = "0".repeat(64);
    assert!(
        convert(&root, &job, &source, &CancellationToken::new(), None)
            .unwrap_err()
            .contains("SHA-256")
    );
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(convert(
        &root,
        &preparation(&source, "visual"),
        &source,
        &cancel,
        None
    )
    .is_err());
    assert_eq!(std::fs::read_dir(root.join("assets")).unwrap().count(), 1);
}

#[test]
fn rejects_wrong_product_tile_crs_and_external_entities() {
    for (needle, replacement, expected) in [
        ("PRODUCT_URI>", "PRODUCT_URI>other-", "identity"),
        ("EPSG:32610", "EPSG:32611", "CRS"),
        ("TL_FIXTURE_T10SEG", "TL_FIXTURE_T11SEG", "another MGRS"),
        (
            "<n1:Level-2A_Tile_ID",
            "<!DOCTYPE root [<!ENTITY x SYSTEM 'file:///invalid'>]><n1:Level-2A_Tile_ID",
            "Invalid SAFE XML",
        ),
    ] {
        let data = changed(|name, bytes| {
            if name.ends_with(".xml") {
                *bytes = String::from_utf8(bytes.clone())
                    .unwrap()
                    .replace(needle, replacement)
                    .into_bytes();
            }
        });
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source = source(&root, &data);
        let job = preparation(&source, "scl");
        let error = convert(&root, &job, &source, &CancellationToken::new(), None).unwrap_err();
        assert!(error.contains(expected), "{error}");
        assert!(!root.join("assets").join(format!("{}.tif", job.id)).exists());
    }
}

#[test]
fn rejects_jp2_wrong_bit_depth_inconsistent_size_and_truncation() {
    for mode in 0..3 {
        let data = changed(|name, bytes| {
            if name.ends_with("_SCL_20m.jp2") {
                let start = bytes
                    .windows(4)
                    .position(|v| v == [255, 79, 255, 81])
                    .unwrap();
                match mode {
                    0 => bytes[start + 42] = 15,
                    1 => bytes[start + 11] = 33,
                    _ => {
                        bytes.truncate(bytes.len() - 1);
                    }
                }
            }
        });
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let source = source(&root, &data);
        assert!(convert(
            &root,
            &preparation(&source, "scl"),
            &source,
            &CancellationToken::new(),
            None
        )
        .is_err());
    }
}

#[test]
fn rejects_out_of_range_scl_samples_without_publishing_a_file() {
    let data = changed(|name, bytes| {
        if name.ends_with("_SCL_20m.jp2") {
            *bytes = include_bytes!("../../fixtures/safe/invalid-scl.jp2").to_vec();
        }
    });
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let source = source(&root, &data);
    let job = preparation(&source, "scl");
    assert!(
        convert(&root, &job, &source, &CancellationToken::new(), None)
            .unwrap_err()
            .contains("TCI/SCL range")
    );
    assert_eq!(std::fs::read_dir(root.join("assets")).unwrap().count(), 1);
}

#[test]
fn rejects_zip_crc_failure_even_with_a_matching_archive_sha256() {
    let mut data = FIXTURE.to_vec();
    let mut offset = 0;
    let mut altered = false;
    while offset + 46 < data.len() {
        if data[offset..offset + 4] == [80, 75, 1, 2] {
            let size =
                u16::from_le_bytes(data[offset + 28..offset + 30].try_into().unwrap()) as usize;
            if data[offset + 46..offset + 46 + size].ends_with(b"_SCL_20m.jp2") {
                data[offset + 16] ^= 1;
                altered = true;
                break;
            }
        }
        offset += 1;
    }
    assert!(altered);
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let source = source(&root, &data);
    let job = preparation(&source, "scl");
    assert!(
        convert(&root, &job, &source, &CancellationToken::new(), None)
            .unwrap_err()
            .contains("CRC/decompression")
    );
    assert_eq!(std::fs::read_dir(root.join("assets")).unwrap().count(), 1);
}

async fn setup(directory: &Path) -> (JobManager, crate::Project, Job) {
    let manager = JobManager::open(directory).await.unwrap();
    let source = source(&manager.inner.root, FIXTURE);
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.insert(source.id.clone(), source.clone());
        manager.persist(&store.jobs).await.unwrap();
    }
    let scene = crate::projects::ProjectScene {
        item_id: ITEM.into(),
        date: "2025-06-27T18:49:41Z".into(),
        cloud: Some(0.0),
        crs: Some("EPSG:32610".into()),
        grid_code: Some("10SEG".into()),
        bbox: [-123.0, 37.90, -122.99, 37.95],
        assets: BTreeMap::from([(
            "product".into(),
            crate::projects::ProjectAsset {
                href: HREF.into(),
                media_type: "application/zip".into(),
                raster_band: None,
            },
        )]),
    };
    let project = manager
        .create_project(crate::CreateProjectRequest {
            name: "Synthetic SAFE QA".into(),
            bounds: [-122.999, 37.92, -122.995, 37.94],
            geometry: None,
            scenes: vec![scene],
        })
        .await
        .unwrap();
    (manager, project, source)
}

#[tokio::test]
async fn prepare_restart_inspect_pixel_thumbnail_and_project_processing_keep_provenance() {
    let directory = tempfile::tempdir().unwrap();
    let (manager, project, source) = setup(directory.path()).await;
    let mut outputs = Vec::new();
    for key in ["scl", "visual"] {
        let jobs = manager.prepare_project(&project.id, key).await.unwrap();
        let job = manager.wait(&jobs.jobs[0].id).await.unwrap();
        assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
        assert_eq!(job.parent_id.as_deref(), Some(source.id.as_str()));
        let inspection = manager.inspect_raster(&job.id).await.unwrap();
        assert_eq!(
            inspection.pixel_size,
            if key == "scl" { [20.0; 2] } else { [10.0; 2] }
        );
        let pixel = manager
            .sample_raster(&job.id, 500010.0, 4199990.0)
            .await
            .unwrap();
        assert_eq!(pixel.value, if key == "visual" { 10.0 } else { 0.0 });
        let thumbnail = manager.file_thumbnail(&job.id).await.unwrap();
        assert_eq!(thumbnail.sha256, job.sha256.clone().unwrap());
        let again = manager.prepare_project(&project.id, key).await.unwrap();
        assert_eq!(again.jobs[0].id, job.id);
        let mosaic = manager.run_project_mosaic(&project.id, key).await.unwrap();
        assert_eq!(mosaic.mosaic.as_ref().unwrap().sources[0].job_id, job.id);
        let mosaic = manager.wait(&mosaic.id).await.unwrap();
        assert_eq!(mosaic.status, JobStatus::Succeeded, "{:?}", mosaic.error);
        assert_eq!(
            mosaic.mosaic_output.as_ref().unwrap().pixel_size,
            inspection.pixel_size
        );
        outputs.push(job);
    }
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(directory.path()).await.unwrap();
    for job in outputs {
        let restored = manager.get(&job.id).await.unwrap();
        assert_eq!(restored.safe_output, job.safe_output);
        assert_eq!(restored.sha256, job.sha256);
        assert_eq!(
            manager.file_thumbnail(&job.id).await.unwrap().sha256,
            job.sha256.unwrap()
        );
    }
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn preparation_rejects_missing_source_and_interrupted_jobs_can_retry() {
    let directory = tempfile::tempdir().unwrap();
    let (manager, project, source) = setup(directory.path()).await;
    let mut job = preparation(&source, "scl");
    job.status = JobStatus::Running;
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.insert(job.id.clone(), job.clone());
        manager.persist(&store.jobs).await.unwrap();
    }
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(directory.path()).await.unwrap();
    assert_eq!(
        manager.get(&job.id).await.unwrap().status,
        JobStatus::Interrupted
    );
    manager.retry(&job.id).await.unwrap();
    assert_eq!(
        manager.wait(&job.id).await.unwrap().status,
        JobStatus::Succeeded
    );
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.get_mut(&source.id).unwrap().sha256 = Some("0".repeat(64));
    }
    assert!(manager
        .run_project_mosaic(&project.id, "scl")
        .await
        .is_err());
    manager.shutdown().await.unwrap();
}
