use super::*;
use crate::projects::{CreateProjectRequest, ProjectAsset, ProjectScene};
use std::io::Cursor;
use tiff::encoder::colortype;

fn source(root: &Path, item: usize, left: f64, values: &[f32], width: u32) -> Job {
    let mut buffer = Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut buffer).unwrap();
        let mut image = encoder
            .new_image::<colortype::Gray32Float>(width, values.len() as u32 / width)
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::GeoKeyDirectoryTag,
                &[
                    1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 32610, 3076, 0, 1,
                    9001,
                ][..],
            )
            .unwrap();
        image
            .encoder()
            .write_tag(Tag::ModelPixelScaleTag, &[10.0, 10.0, 0.0][..])
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::ModelTiepointTag,
                &[0.0, 0.0, 0.0, left, 4200000.0, 0.0][..],
            )
            .unwrap();
        image
            .encoder()
            .write_tag(Tag::GdalNodata, "-32768")
            .unwrap();
        image.write_data(values).unwrap();
    }
    let catalogue: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../prototype/qa/sentinel-1-rtc-catalog.json"
    ))
    .unwrap();
    let scene = &catalogue["features"][item];
    let mut job = crate::raster::tests::record(root, &buffer.into_inner());
    job.item_id = scene["id"].as_str().unwrap().into();
    job.href = scene["assets"]["vv"]["href"].as_str().unwrap().into();
    job.asset_key = "vv".into();
    job
}
fn request(sources: &[Job], geometry: Option<crop::PolygonGeometry>) -> CreateProjectRequest {
    CreateProjectRequest {
        name: "Synthetic RTC processing".into(),
        bounds: [-123.001, 37.947, -122.999, 37.949],
        geometry,
        scenes: sources
            .iter()
            .map(|job| ProjectScene {
                footprint: None,
                item_id: job.item_id.clone(),
                date: "2025-06-30T14:06:54Z".into(),
                cloud: None,
                crs: Some("EPSG:32610".into()),
                grid_code: None,
                bbox: [-123.001, 37.947, -122.999, 37.949],
                assets: BTreeMap::from([(
                    "vv".into(),
                    ProjectAsset {
                        href: job.href.clone(),
                        media_type: "image/tiff".into(),
                        raster_band: None,
                    },
                )]),
            })
            .collect(),
    }
}
fn project(sources: &[Job]) -> Project {
    let r = request(sources, None);
    Project {
        id: Uuid::new_v4().to_string(),
        name: r.name,
        bounds: r.bounds,
        geometry: r.geometry,
        scenes: r.scenes,
        stac_items: Vec::new(),
        wcs_items: Vec::new(),
        created_at: now(),
        updated_at: now(),
        agent_approvals: Vec::new(),
    }
}
fn values(output: &MosaicOutput) -> Vec<f32> {
    let mut decoder = Decoder::new(File::open(&output.path).unwrap()).unwrap();
    crate::raster::radar::validate_header(&mut decoder, Some(&output.plan)).unwrap();
    let DecodingResult::F32(samples) = decoder.read_image().unwrap() else {
        panic!("Float32 gamma0 required")
    };
    samples
}

#[test]
fn radar_mosaic_keeps_float_bits_zero_nodata_and_newest_valid_overlap() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().canonicalize().unwrap();
    let a = source(&root, 1, 500000.0, &[0.1, 0.0, 0.3, 0.4, 0.0, -32768.0], 3);
    let b = source(
        &root,
        0,
        500010.0,
        &[-32768.0, 0.0, 0.8, 0.7, -32768.0, 0.9],
        3,
    );
    let sources = [a, b];
    let output = write_mosaic(
        &root,
        &project(&sources),
        &sources,
        "vv",
        &Uuid::new_v4().to_string(),
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    assert_eq!(
        output.plan.radar,
        Some(crate::raster::radar::profile("vv").unwrap())
    );
    assert_eq!(
        (
            output.plan.width,
            output.plan.height,
            output.plan.covered_pixels
        ),
        (4, 2, 7)
    );
    assert!(output.plan.elevation.is_none() && output.plan.calibration.is_none());
    assert_eq!(
        values(&output),
        [0.1f32, 0.0, 0.0, 0.8, 0.4, 0.7, -32768.0, 0.9]
    );
}

#[tokio::test]
async fn radar_polygon_clip_metadata_pixels_and_thumbnail_survive_restart() {
    let d = tempfile::tempdir().unwrap();
    let manager = JobManager::open(d.path()).await.unwrap();
    let original = source(
        manager.storage_root(),
        0,
        499980.0,
        &[0.0, 0.11, 0.2, 0.3, 0.4, -32768.0, 0.6, 0.7],
        4,
    );
    let geometry = crop::PolygonGeometry::Polygon(vec![vec![
        [-123.001, 37.947],
        [-123.0, 37.947],
        [-123.0, 37.949],
        [-123.001, 37.949],
        [-123.001, 37.947],
    ]]);
    let p = manager
        .create_project(request(std::slice::from_ref(&original), Some(geometry)))
        .await
        .unwrap();
    let records = BTreeMap::from([(original.id.clone(), original.clone())]);
    manager.inner.store.lock().await.jobs = records.clone();
    manager.persist(&records).await.unwrap();
    let job = manager.run_project_mosaic(&p.id, "vv").await.unwrap();
    let job = manager.wait(&job.id).await.unwrap();
    assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
    let plan = job.mosaic_output.as_ref().unwrap();
    assert_eq!(
        (
            plan.width,
            plan.height,
            plan.masked_pixels,
            plan.covered_pixels
        ),
        (4, 2, 4, 3)
    );
    let data = manager.inspect_raster(&job.id).await.unwrap();
    assert!(data.elevation.is_none() && data.reflectance.is_none());
    assert_eq!(data.radar.unwrap().unit, "linear");
    let zero = manager
        .sample_raster(&job.id, 499985.0, 4199995.0)
        .await
        .unwrap();
    assert_eq!(zero.value, 0.0);
    assert!(!zero.is_no_data && zero.decibels.is_none());
    assert!(
        manager
            .sample_raster(&job.id, 500005.0, 4199995.0)
            .await
            .unwrap()
            .is_no_data
    );
    let sidecar: serde_json::Value =
        serde_json::from_reader(File::open(job.manifest_path.as_ref().unwrap()).unwrap()).unwrap();
    assert_eq!(sidecar["sources"][0]["radar"]["polarization"], "VV");
    assert_eq!(sidecar["sources"][0]["additionalCalibrationApplied"], false);
    let thumb = manager.file_thumbnail(&job.id).await.unwrap();
    drop(manager);
    let restored = JobManager::open(d.path()).await.unwrap();
    assert_eq!(
        restored.file_thumbnail(&job.id).await.unwrap().data_url,
        thumb.data_url
    );
    let value = restored
        .sample_raster(&job.id, 499995.0, 4199995.0)
        .await
        .unwrap();
    assert_eq!(value.value, f64::from(0.11f32));
    let mut changed = job.clone();
    changed
        .mosaic_output
        .as_mut()
        .unwrap()
        .radar
        .as_mut()
        .unwrap()
        .polarization = "VH".into();
    assert!(crate::raster::radar::inspect(restored.storage_root(), &changed, 160).is_err());
    let mut foreign = job.clone();
    foreign.href = "https://example.com/iw-vv.rtc.tiff".into();
    assert!(crate::raster::radar::inspect(restored.storage_root(), &foreign, 160).is_err());
    let mut swapped = job.clone();
    swapped.href = swapped.href.replace("iw-vv", "iw-vh");
    assert!(crate::raster::radar::inspect(restored.storage_root(), &swapped, 160).is_err());
    let mut shifted = job;
    shifted.mosaic_output.as_mut().unwrap().bounds[0] += 10.0;
    assert!(crate::raster::radar::inspect(restored.storage_root(), &shifted, 160).is_err());
}

#[test]
fn radar_rejects_invalid_samples_changed_hash_polarization_grid_and_cancel() {
    for invalid in [-1.0f32, f32::NAN, f32::INFINITY] {
        let d = tempfile::tempdir().unwrap();
        let root = d.path().canonicalize().unwrap();
        let sources = [source(&root, 0, 500000.0, &[invalid, 0.0, 0.1, 0.2], 2)];
        let id = Uuid::new_v4().to_string();
        assert!(write_mosaic(
            &root,
            &project(&sources),
            &sources,
            "vv",
            &id,
            &CancellationToken::new(),
            None
        )
        .is_err());
        assert!(!root.join("assets").join(format!("{id}.tif")).exists());
    }
    let d = tempfile::tempdir().unwrap();
    let root = d.path().canonicalize().unwrap();
    let a = source(&root, 0, 500000.0, &[0.1; 4], 2);
    let b = source(&root, 1, 500005.0, &[0.2; 4], 2);
    let sources = [a.clone(), b];
    assert!(write_mosaic(
        &root,
        &project(&sources),
        &sources,
        "vv",
        &Uuid::new_v4().to_string(),
        &CancellationToken::new(),
        None
    )
    .is_err());
    let mut changed = a.clone();
    changed.sha256 = Some("0".repeat(64));
    assert!(source_raster(&root, &changed, "vv", &CancellationToken::new()).is_err());
    changed = a.clone();
    changed.asset_key = "vh".into();
    changed.href = changed.href.replace("iw-vv", "iw-vh");
    assert!(source_raster(&root, &changed, "vv", &CancellationToken::new()).is_err());
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(write_mosaic(
        &root,
        &project(std::slice::from_ref(&a)),
        &[a],
        "vv",
        &Uuid::new_v4().to_string(),
        &cancel,
        None
    )
    .is_err());
}

#[test]
fn radar_streams_more_than_eight_million_float32_samples_without_changing_bits() {
    let d = tempfile::tempdir().unwrap();
    let root = d.path().canonicalize().unwrap();
    let pattern = [0.0f32, 0.1, 0.000000001, 10000.0, -32768.0];
    let expected = (0..9_000_000)
        .map(|i| pattern[i % pattern.len()])
        .collect::<Vec<_>>();
    let original = source(&root, 0, 500000.0, &expected, 10000);
    let mut p = project(std::slice::from_ref(&original));
    p.bounds = [-123.1, 37.8, -121.7, 38.0];
    let output = write_mosaic(
        &root,
        &p,
        &[original],
        "vv",
        &Uuid::new_v4().to_string(),
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    assert_eq!((output.plan.width, output.plan.height), (10000, 900));
    assert_eq!(output.plan.covered_pixels, 7_200_000);
    assert_eq!(values(&output), expected);
}
