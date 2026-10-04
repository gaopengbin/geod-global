//! Synthetic HGT fixtures test processing mechanics, not NASA terrain accuracy.
use super::*;
use crate::{
    new_download_job,
    projects::{CreateProjectRequest, ProjectAsset, ProjectScene},
    CreateJobRequest,
};
use std::io::Cursor;
use zip::{write::SimpleFileOptions, CompressionMethod, ZipWriter};

const STEP: f64 = 1.0 / 3600.0;

fn tile(root: &Path, lon: i32) -> Job {
    tile_at(root, lon, 37)
}

fn tile_at(root: &Path, lon: i32, lat: i32) -> Job {
    let item = format!("N{lat:02}W{:03}.SRTMGL1.hgt", -lon);
    let mut raw = Vec::with_capacity(crate::providers::srtm::HGT_BYTES as usize);
    for y in 0..3601 {
        for x in 0..3601 {
            let height: i16 = if lat == 38 {
                if y == 3600 && x < 4 {
                    [32767, -32768, 0, -32768][x]
                } else {
                    17
                }
            } else if lon == -123 {
                if x == 3600 && y < 4 {
                    [0, 32767, -32767, -32768][y]
                } else {
                    -27
                }
            } else if x == 0 && y < 4 {
                [-32767, -32768, 0, -32768][y]
            } else {
                10
            };
            raw.extend_from_slice(&height.to_be_bytes());
        }
    }
    let mut writer = ZipWriter::new(Cursor::new(Vec::new()));
    writer
        .start_file(
            format!("{}.hgt", &item[..7]),
            SimpleFileOptions::default().compression_method(CompressionMethod::Deflated),
        )
        .unwrap();
    writer.write_all(&raw).unwrap();
    let bytes = writer.finish().unwrap().into_inner();
    let mut job = new_download_job(CreateJobRequest {
        item_id: item.clone(),
        asset_key: "srtm".into(),
        media_type: "application/zip".into(),
        href: format!(
            "https://{}{}{item}/{item}.zip",
            crate::providers::nasa::HOST,
            crate::providers::srtm::PREFIX
        ),
        title: Some("Synthetic SRTM processing fixture, not a NASA download".into()),
    });
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let path = root.join("assets").join(format!("{}.zip", job.id));
    std::fs::write(&path, &bytes).unwrap();
    job.status = JobStatus::Succeeded;
    job.bytes_downloaded = bytes.len() as u64;
    job.total_bytes = Some(job.bytes_downloaded);
    job.sha256 = Some(format!("{:x}", Sha256::digest(&bytes)));
    job.output_path = Some(path.to_string_lossy().into_owned());
    job
}

fn request(
    sources: &[Job],
    bounds: [f64; 4],
    geometry: Option<crop::PolygonGeometry>,
) -> CreateProjectRequest {
    CreateProjectRequest {
        name: "Synthetic SRTM processing fixture".into(),
        bounds,
        geometry,
        scenes: sources
            .iter()
            .map(|job| {
                let [lon, lat] = crate::providers::srtm::cell(&job.item_id).unwrap();
                ProjectScene {
                    item_id: job.item_id.clone(),
                    date: "2000-02-11T00:00:00Z".into(),
                    cloud: None,
                    crs: Some("EPSG:4326".into()),
                    grid_code: Some(job.item_id.clone()),
                    bbox: [lon as f64, lat as f64, (lon + 1) as f64, (lat + 1) as f64],
                    assets: BTreeMap::from([(
                        "srtm".into(),
                        ProjectAsset {
                            href: job.href.clone(),
                            media_type: job.media_type.clone(),
                            raster_band: None,
                        },
                    )]),
                }
            })
            .collect(),
    }
}

fn project(sources: &[Job], bounds: [f64; 4]) -> Project {
    let r = request(sources, bounds, None);
    Project {
        id: Uuid::new_v4().to_string(),
        name: r.name,
        bounds,
        geometry: None,
        scenes: r.scenes,
        stac_items: Vec::new(),
        wcs_items: Vec::new(),
        created_at: now(),
        updated_at: now(),
    }
}

#[test]
fn shared_srtm_column_has_one_position_and_exact_signed_values_across_strips() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let sources = [tile(&root, -123), tile(&root, -122)];
    let bounds = [
        -123.0 - STEP / 2.0,
        37.0 - STEP / 2.0,
        -121.0 + STEP / 2.0,
        38.0 + STEP / 2.0,
    ];
    let output = write_mosaic(
        &root,
        &project(&sources, bounds),
        &sources,
        "srtm",
        &Uuid::new_v4().to_string(),
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    assert_eq!((output.plan.width, output.plan.height), (7201, 3601));
    assert_eq!(output.plan.covered_pixels, 7201 * 3601 - 1);
    let mut decoder = Decoder::new(File::open(&output.path).unwrap()).unwrap();
    crate::raster::elevation::validate_header(&mut decoder, "", Some(&output.plan)).unwrap();
    assert!(decoder.strip_count().unwrap() > 1);
    let DecodingResult::I16(values) = decoder.read_image().unwrap() else {
        panic!("Int16 required");
    };
    for (i, value) in values.iter().enumerate() {
        let (y, x) = (i / 7201, i % 7201);
        let expected = if x < 3600 {
            -27
        } else if x > 3600 {
            10
        } else if y < 4 {
            [-32767, 32767, 0, -32768][y]
        } else {
            10
        };
        assert_eq!(*value, expected, "column {x}, row {y}");
    }
    assert!(std::fs::read_dir(root.join("assets"))
        .unwrap()
        .all(|entry| entry
            .unwrap()
            .path()
            .extension()
            .is_none_or(|e| e != "part")));
}

#[tokio::test]
async fn queued_srtm_polygon_output_inspection_and_cache_survive_restart() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let sources = [
        tile(manager.storage_root(), -123),
        tile(manager.storage_root(), -122),
    ];
    let bounds = [
        -122.0 - 1.5 * STEP,
        38.0 - 3.5 * STEP,
        -122.0 + 1.5 * STEP,
        38.0 + STEP / 2.0,
    ];
    let ring = vec![
        [bounds[0], bounds[1]],
        [-122.0 + 0.4 * STEP, bounds[1]],
        [-122.0 + 0.4 * STEP, bounds[3]],
        [bounds[0], bounds[3]],
        [bounds[0], bounds[1]],
    ];
    let p = manager
        .create_project(request(
            &sources,
            bounds,
            Some(crop::PolygonGeometry::Polygon(vec![ring])),
        ))
        .await
        .unwrap();
    let saved = sources.iter().map(|j| (j.id.clone(), j.clone())).collect();
    manager.inner.store.lock().await.jobs = saved;
    manager
        .persist(&manager.inner.store.lock().await.jobs)
        .await
        .unwrap();
    let queued = manager.run_project_mosaic(&p.id, "srtm").await.unwrap();
    let job = manager.wait(&queued.id).await.unwrap();
    assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
    let plan = job.mosaic_output.as_ref().unwrap();
    assert_eq!(
        (
            plan.width,
            plan.height,
            plan.covered_pixels,
            plan.masked_pixels
        ),
        (3, 4, 7, 4)
    );
    let metadata = manager.inspect_raster(&job.id).await.unwrap();
    assert_eq!(metadata.data_type, "Int16");
    assert_eq!(metadata.nodata, Some(-32768.0));
    assert_eq!(metadata.elevation.unwrap().vertical_reference, "EPSG:5773");
    // Equal reference dates are ordered by item ID: W123 follows W122.
    for (y, expected) in [0, 32767, -32767, -32768].iter().enumerate() {
        let pixel = manager
            .sample_raster(&job.id, -122.0, 38.0 - y as f64 * STEP)
            .await
            .unwrap();
        assert_eq!(pixel.value, f64::from(*expected));
        assert_eq!(pixel.is_no_data, *expected == -32768);
    }
    let masked = manager
        .sample_raster(&job.id, -122.0 + STEP, 38.0)
        .await
        .unwrap();
    assert_eq!(masked.value, -32768.0);
    assert!(masked.is_no_data);
    let preview = manager.file_thumbnail(&job.id).await.unwrap();
    let mut tampered = job.clone();
    tampered.mosaic_output.as_mut().unwrap().elevation = Some(crate::raster::elevation::profile());
    assert!(crate::raster::srtm::validate_job(&tampered).is_err());
    drop(manager);
    // Simulate a force-terminated engine's orphaned HGT staging file. Only the
    // reserved generated name is removed; unrelated partial files survive.
    let staged = dir.path().join("assets/srtm-grid-A1b2C3.part");
    let unrelated = dir.path().join("assets/unrelated.part");
    std::fs::write(&staged, b"orphaned HGT staging").unwrap();
    std::fs::write(&unrelated, b"unrelated file").unwrap();
    let restored = JobManager::open(dir.path()).await.unwrap();
    assert!(!staged.exists());
    assert!(unrelated.exists());
    assert_eq!(
        restored.file_thumbnail(&job.id).await.unwrap().data_url,
        preview.data_url
    );
    assert_eq!(
        restored
            .sample_raster(&job.id, -122.0, 38.0)
            .await
            .unwrap()
            .value,
        0.0
    );
}

#[test]
fn shared_srtm_row_is_not_duplicated_and_keeps_older_heights_under_nodata() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let sources = [tile_at(&root, -123, 37), tile_at(&root, -123, 38)];
    let bounds = [
        -123.0 - STEP / 2.0,
        37.0 - STEP / 2.0,
        -122.0 + STEP / 2.0,
        39.0 + STEP / 2.0,
    ];
    let output = write_mosaic(
        &root,
        &project(&sources, bounds),
        &sources,
        "srtm",
        &Uuid::new_v4().to_string(),
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    assert_eq!((output.plan.width, output.plan.height), (3601, 7201));
    let mut decoder = Decoder::new(File::open(&output.path).unwrap()).unwrap();
    let DecodingResult::I16(values) = decoder.read_image().unwrap() else {
        panic!("Int16 required");
    };
    for (i, value) in values.iter().enumerate() {
        let (y, x) = (i / 3601, i % 3601);
        let expected = if y < 3600 {
            17
        } else if y == 3600 {
            if x < 4 {
                [32767, -27, 0, -27][x]
            } else {
                17
            }
        } else if x == 3600 && y < 3604 {
            [0, 32767, -32767, -32768][y - 3600]
        } else {
            -27
        };
        assert_eq!(*value, expected, "column {x}, row {y}");
    }
}

#[test]
fn cancellation_after_hgt_staging_removes_the_temporary_grid_and_output() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = tile(&root, -123);
    let p = project(
        std::slice::from_ref(&source),
        crate::raster::srtm::bounds(&source.item_id).unwrap(),
    );
    let cancel = CancellationToken::new();
    let signal = cancel.clone();
    let (tx, mut rx) = tokio::sync::mpsc::unbounded_channel();
    let watcher = std::thread::spawn(move || {
        rx.blocking_recv().unwrap();
        signal.cancel();
    });
    let output = write_mosaic(
        &root,
        &p,
        &[source],
        "srtm",
        &Uuid::new_v4().to_string(),
        &cancel,
        Some(&tx),
    );
    watcher.join().unwrap();
    assert!(output.unwrap_err().to_lowercase().contains("cancel"));
    assert_eq!(std::fs::read_dir(root.join("assets")).unwrap().count(), 1);
}

#[test]
fn cancelled_or_changed_hgt_cannot_publish_a_result_or_leave_staged_grids() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let mut job = tile(&root, -123);
    let token = CancellationToken::new();
    token.cancel();
    assert!(source_raster(&root, &job, "srtm", &token).is_err());
    job.sha256 = Some("0".repeat(64));
    assert!(source_raster(&root, &job, "srtm", &CancellationToken::new()).is_err());
    assert_eq!(std::fs::read_dir(root.join("assets")).unwrap().count(), 1);
}
