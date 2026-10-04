use super::*;
use std::io::Cursor;
const FIXTURES: [&[u8]; 3] = [
    include_bytes!("../../../../fixtures/viirs/synthetic-vnp09a1.h5.gz"),
    include_bytes!("../../../../fixtures/viirs/synthetic-vj109a1.h5.gz"),
    include_bytes!("../../../../fixtures/viirs/synthetic-vj209a1.h5.gz"),
];
fn source(root: &Path, index: usize) -> (Job, serde_json::Value) {
    let expected: serde_json::Value =
        serde_json::from_str(include_str!("../../../../fixtures/viirs/expected.json")).unwrap();
    let expected = expected["fixtures"][index].clone();
    let mut bytes = Vec::new();
    flate2::read::GzDecoder::new(FIXTURES[index])
        .read_to_end(&mut bytes)
        .unwrap();
    let item = expected["itemId"].as_str().unwrap();
    let product = super::super::identity(item).unwrap();
    let href = format!(
        "https://{}/lp-prod-protected/{}.002/{item}/{item}.h5",
        crate::providers::nasa::HOST,
        product.product
    );
    let mut job = crate::new_download_job(crate::CreateJobRequest {
        item_id: item.into(),
        asset_key: "viirs".into(),
        href,
        media_type: "application/x-hdf5".into(),
        title: Some("SYNTHETIC VIIRS fixture; not NASA observations".into()),
    });
    std::fs::create_dir_all(root.join("assets")).unwrap();
    let path = root.join("assets").join(format!("{}.h5", job.id));
    std::fs::write(&path, &bytes).unwrap();
    job.status = JobStatus::Succeeded;
    job.bytes_downloaded = bytes.len() as u64;
    job.total_bytes = Some(job.bytes_downloaded);
    job.sha256 = Some(format!("{:x}", Sha256::digest(&bytes)));
    job.output_path = Some(path.to_string_lossy().into_owned());
    job.viirs_science = Some(
        hdf::verify(
            &path,
            item,
            job.bytes_downloaded,
            job.sha256.as_deref().unwrap(),
            &CancellationToken::new(),
        )
        .unwrap(),
    );
    (job, expected)
}
fn prepared(source: &Job, key: &str) -> Job {
    let mut job = crate::new_download_job(crate::CreateJobRequest {
        item_id: source.item_id.clone(),
        asset_key: key.into(),
        href: source.href.clone(),
        media_type: "image/tiff".into(),
        title: None,
    });
    job.kind = "raster_prepare".into();
    job.parent_id = Some(source.id.clone());
    job.viirs_prepare = Some(ViirsSpec {
        source_job_id: source.id.clone(),
        source_sha256: source.sha256.clone().unwrap(),
        science: source.viirs_science.clone().unwrap(),
    });
    job
}
#[test]
fn every_prepared_sample_matches_independent_h5py_for_three_platforms() {
    for index in 0..3 {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        let (source, expected) = source(&root, index);
        let original = std::fs::read(source.output_path.as_ref().unwrap()).unwrap();
        for (channel, key) in ["red", "green", "blue"].iter().enumerate() {
            let job = prepared(&source, key);
            let (size, hash) = convert(&root, &job, &source, &CancellationToken::new()).unwrap();
            let path = root.join("assets").join(format!("{}.tif", job.id));
            let bytes = std::fs::read(&path).unwrap();
            assert_eq!(size, bytes.len() as u64);
            assert_eq!(hash, format!("{:x}", Sha256::digest(&bytes)));
            let mut decoder = Decoder::new(Cursor::new(bytes)).unwrap();
            let header =
                crate::raster::reflectance::validate_header(&mut decoder, &profile()).unwrap();
            grid(&header, &job).unwrap();
            let DecodingResult::I16(pixels) = decoder.read_image().unwrap() else {
                panic!("not signed original samples")
            };
            assert_eq!(pixels.len(), 1_440_000);
            assert_eq!(
                sample_hash(&pixels),
                expected["bands"][channel]["samplesSha256"]
            );
            assert_eq!(pixels[0], NODATA);
            assert_eq!(*pixels.last().unwrap(), 16001);
            assert!(decoder
                .get_tag(Tag::Unknown(42112))
                .unwrap()
                .into_string()
                .unwrap()
                .contains("viirs-09a1-v002"));
            if let Ok(path) = std::env::var("GEOD_VIIRS_PREPARE_EVIDENCE") {
                let out = Path::new(&path).join(format!("{index}-{key}.tif"));
                std::fs::create_dir_all(out.parent().unwrap()).unwrap();
                std::fs::copy(root.join("assets").join(format!("{}.tif", job.id)), out).unwrap();
            }
        }
        assert_eq!(
            std::fs::read(source.output_path.as_ref().unwrap()).unwrap(),
            original
        );
    }
}

async fn setup(path: &Path) -> (JobManager, crate::Project, Job) {
    let manager = JobManager::open(path).await.unwrap();
    let (source, _) = source(&manager.inner.root, 1);
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.insert(source.id.clone(), source.clone());
        manager.persist(&store.jobs).await.unwrap();
    }
    let scene = crate::projects::ProjectScene {
        item_id: source.item_id.clone(),
        date: "2025-06-26T00:00:00Z".into(),
        cloud: None,
        crs: Some(CRS.into()),
        grid_code: Some("h08v05".into()),
        bbox: [-130.0, 30.0, -110.0, 40.0],
        assets: BTreeMap::from([(
            "viirs".into(),
            crate::projects::ProjectAsset {
                href: source.href.clone(),
                media_type: source.media_type.clone(),
                raster_band: None,
            },
        )]),
    };
    let project = manager
        .create_project(crate::CreateProjectRequest {
            name: "SYNTHETIC VIIRS processing fixture".into(),
            bounds: [-123.0, 37.7, -122.9, 37.8],
            geometry: None,
            scenes: vec![scene],
        })
        .await
        .unwrap();
    (manager, project, source)
}
#[tokio::test]
async fn managed_preparation_inspection_rgb_thumbnail_clip_and_restart() {
    let directory = tempfile::tempdir().unwrap();
    let (manager, project, source) = setup(directory.path()).await;
    let mut outputs = Vec::new();
    let mut inspections = Vec::new();
    let mut thumbnails = Vec::new();
    for key in ["red", "green", "blue"] {
        let queued = manager
            .prepare_project(&project.id, key)
            .await
            .unwrap()
            .jobs
            .remove(0);
        assert_eq!(
            manager
                .prepare_project(&project.id, key)
                .await
                .unwrap()
                .jobs[0]
                .id,
            queued.id
        );
        let job = manager.wait(&queued.id).await.unwrap();
        assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
        let inspection = manager.inspect_raster(&job.id).await.unwrap();
        assert_eq!(inspection.crs, CRS);
        assert_eq!(inspection.width, 1200);
        let s = &job.viirs_prepare.as_ref().unwrap().science;
        let pixel = manager
            .sample_raster(
                &job.id,
                s.bounds[0] + s.pixel_size[0] / 2.0,
                s.bounds[3] - s.pixel_size[1] / 2.0,
            )
            .await
            .unwrap();
        assert!(pixel.is_no_data);
        assert_eq!(pixel.value, f64::from(NODATA));
        let thumbnail = manager.file_thumbnail(&job.id).await.unwrap();
        assert_eq!(thumbnail.sha256, job.sha256.as_ref().unwrap().as_str());
        thumbnails.push(thumbnail);
        inspections.push(inspection);
        outputs.push(job);
    }
    let ids: [String; 3] = outputs
        .iter()
        .map(|j| j.id.clone())
        .collect::<Vec<_>>()
        .try_into()
        .unwrap();
    let composite = manager
        .inspect_composite(crate::CompositeRequest {
            job_ids: ids.clone(),
        })
        .await
        .unwrap();
    assert_eq!(composite.grid.crs, CRS);
    assert_eq!(composite.composite.product, "viirs-09a1-v002");
    let s = &source.viirs_science.as_ref().unwrap();
    let x = s.bounds[0] + 10.5 * s.pixel_size[0];
    let y = s.bounds[3] - 20.5 * s.pixel_size[1];
    let pixel = manager
        .sample_composite(crate::CompositePixelRequest { job_ids: ids, x, y })
        .await
        .unwrap();
    assert_eq!(pixel.values, [-30, 107, 244]);
    for (actual, expected) in pixel.reflectances.iter().zip([-0.003, 0.0107, 0.0244]) {
        assert!((actual.unwrap() - expected).abs() < 1e-12);
    }
    let queued = manager
        .run_project_mosaic(&project.id, "red")
        .await
        .unwrap();
    let clip = manager.wait(&queued.id).await.unwrap();
    assert_eq!(clip.status, JobStatus::Succeeded, "{:?}", clip.error);
    assert_eq!(clip.mosaic_output.as_ref().unwrap().crs, CRS);
    let manifest: serde_json::Value =
        serde_json::from_slice(&std::fs::read(clip.manifest_path.as_ref().unwrap()).unwrap())
            .unwrap();
    let provenance = &manifest["sources"][0];
    assert_eq!(provenance["compositePeriod"]["start"], "2025-06-26");
    assert_eq!(provenance["compositePeriod"]["end"], "2025-07-03");
    assert!(provenance.get("acquiredAt").is_none());
    assert_eq!(provenance["originalHdf5"]["jobId"], source.id);
    assert_eq!(
        provenance["originalHdf5"]["sha256"],
        source.sha256.as_ref().unwrap().as_str()
    );
    assert_eq!(
        provenance["originalHdf5"]["samplesSha256"],
        source.viirs_science.as_ref().unwrap().bands[0].samples_sha256
    );
    assert_eq!(provenance["qualityMaskApplied"], false);
    assert_eq!(
        manager
            .inspect_raster(&clip.id)
            .await
            .unwrap()
            .reflectance
            .unwrap()
            .product,
        "viirs-09a1-v002"
    );
    if let Ok(path) = std::env::var("GEOD_VIIRS_PREPARE_EVIDENCE") {
        let out = Path::new(&path);
        std::fs::create_dir_all(out).unwrap();
        std::fs::copy(
            clip.output_path.as_ref().unwrap(),
            out.join("project-red-clip.tif"),
        )
        .unwrap();
        std::fs::write(out.join("ui-fixture.json"),serde_json::to_vec_pretty(&serde_json::json!({"provenance":"Synthetic HDF5 fixture outputs from native processing; not NASA observations or production authorization","project":project,"source":source,"jobs":outputs,"inspections":inspections,"thumbnails":thumbnails,"composite":composite,"pixel":pixel,"clip":clip})).unwrap()).unwrap();
    }
    manager.shutdown().await.unwrap();
    drop(manager);
    let manager = JobManager::open(directory.path()).await.unwrap();
    for job in outputs {
        let restored = manager.get(&job.id).await.unwrap();
        assert_eq!(restored.viirs_prepare, job.viirs_prepare);
        assert_eq!(
            manager.file_thumbnail(&job.id).await.unwrap().sha256,
            job.sha256.unwrap()
        );
    }
    assert_eq!(
        manager.get(&clip.id).await.unwrap().status,
        JobStatus::Succeeded
    );
    manager.shutdown().await.unwrap();
}
#[test]
fn rejects_changed_pins_bad_band_geometry_source_bytes_and_cancel() {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path().canonicalize().unwrap();
    let (source, _) = source(&root, 0);
    let job = prepared(&source, "red");
    let jobs = BTreeMap::from([(source.id.clone(), source.clone())]);
    validate_source(&job, &jobs).unwrap();
    let mut changed = job.clone();
    changed.viirs_prepare.as_mut().unwrap().source_sha256 = "0".repeat(64);
    assert!(validate_source(&changed, &jobs).is_err());
    changed = job.clone();
    changed.asset_key = "visual".into();
    assert!(validate_stored(&changed).is_err());
    changed = job.clone();
    changed.viirs_prepare.as_mut().unwrap().science.bounds[0] += 1.0;
    assert!(validate_stored(&changed).is_err());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(convert(&root, &job, &source, &cancel).is_err());
    std::fs::write(source.output_path.as_ref().unwrap(), b"changed source").unwrap();
    assert!(convert(&root, &job, &source, &CancellationToken::new()).is_err());
    assert_eq!(std::fs::read_dir(root.join("assets")).unwrap().count(), 1);
}

#[tokio::test]
async fn interrupted_preparation_retries_and_changed_parent_cannot_be_reused() {
    let directory = tempfile::tempdir().unwrap();
    let (manager, project, source) = setup(directory.path()).await;
    let mut job = prepared(&source, "green");
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
    let mut older = source.clone();
    older.id = Uuid::nil().to_string();
    older.viirs_science = None;
    let old_path = manager
        .inner
        .root
        .join("assets")
        .join(format!("{}.h5", older.id));
    std::fs::copy(source.output_path.as_ref().unwrap(), &old_path).unwrap();
    older.output_path = Some(old_path.to_string_lossy().into_owned());
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.insert(older.id.clone(), older);
    }
    let prepared = manager
        .prepare_project(&project.id, "blue")
        .await
        .unwrap()
        .jobs
        .remove(0);
    assert_eq!(prepared.parent_id.as_deref(), Some(source.id.as_str()));
    assert_eq!(
        manager.wait(&prepared.id).await.unwrap().status,
        JobStatus::Succeeded
    );
    {
        let mut store = manager.inner.store.lock().await;
        store.jobs.get_mut(&source.id).unwrap().sha256 = Some("0".repeat(64));
    }
    assert!(manager.prepare_project(&project.id, "blue").await.is_err());
    assert!(manager
        .run_project_mosaic(&project.id, "green")
        .await
        .is_err());
    manager.shutdown().await.unwrap();
}
