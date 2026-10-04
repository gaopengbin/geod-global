use super::*;
use crate::raster::reflectance::{
    self,
    tests::{fixture_samples, fixture_with_type, record},
};

#[test]
fn large_signed_and_unsigned_bands_keep_exact_samples_across_output_strips() {
    for signed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let values: Vec<i32> = (0..9_000_000)
            .map(|index| {
                if signed {
                    if index / 3000 < 512 {
                        -100
                    } else {
                        32767
                    }
                } else if index / 3000 < 512 {
                    1000
                } else {
                    65535
                }
            })
            .collect();
        let source = record(
            &root,
            signed,
            &reflectance::tests::fixture_sized(
                signed,
                if signed { "-9999" } else { "0" },
                30.0,
                1,
                &values,
                3000,
                3000,
            ),
        );
        let mut project = project(std::slice::from_ref(&source));
        project.bounds = [-123.1, 36.0, -121.0, 39.0];
        let output = write_mosaic(
            &root,
            &project,
            &[source],
            "red",
            &Uuid::new_v4().to_string(),
            &CancellationToken::new(),
            None,
        )
        .unwrap();
        assert_eq!(
            (
                output.plan.width,
                output.plan.height,
                output.plan.covered_pixels
            ),
            (3000, 3000, 9_000_000)
        );
        let mut decoder = Decoder::new(File::open(&output.path).unwrap()).unwrap();
        assert!(decoder.strip_count().unwrap() > 1);
        let actual = decoded_bytes(
            decoder.read_image().unwrap(),
            output.plan.calibration.as_ref(),
            None,
            false,
            None,
        )
        .unwrap();
        assert!(actual
            .chunks_exact(2)
            .zip(values)
            .all(|(sample, expected)| if signed {
                i32::from(i16::from_ne_bytes([sample[0], sample[1]])) == expected
            } else {
                i32::from(u16::from_ne_bytes([sample[0], sample[1]])) == expected
            }));
    }
}

fn project(sources: &[Job]) -> Project {
    let mut project = super::tests::project(sources, "red");
    project.bounds = [-123.01, 37.94, -122.99, 37.96];
    project
}

pub(super) fn output_job(
    root: &Path,
    source: &Job,
    project: &Project,
    output: MosaicOutput,
    id: String,
) -> Job {
    let mut job = source.clone();
    job.id = id;
    job.kind = "raster_mosaic".into();
    job.item_id = format!("project:{}", project.id);
    job.mosaic = Some(MosaicSpec {
        project_id: project.id.clone(),
        asset_key: "red".into(),
        sources: vec![MosaicSource {
            job_id: source.id.clone(),
            sha256: source.sha256.clone().unwrap(),
        }],

        coverage_sources: Vec::new(),
        vi_selection: None,
    });
    job.output_path = Some(output.path);
    job.bytes_downloaded = output.bytes;
    job.total_bytes = Some(output.bytes);
    job.sha256 = Some(output.sha256);
    job.mosaic_output = Some(output.plan);
    let manifest = root
        .join("assets")
        .join(format!("{}.metadata.json", job.id));
    std::fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!({
            "schemaVersion":"geod-project-mosaic/v1", "assetKey":"red", "plan":job.mosaic_output,
            "sources":job.mosaic.as_ref().unwrap().sources,
        }))
        .unwrap(),
    )
    .unwrap();
    job.manifest_path = Some(manifest.to_string_lossy().into_owned());
    assert!(Path::new(job.output_path.as_ref().unwrap()).starts_with(root));
    job
}

#[test]
fn typed_overlap_preserves_older_valid_dn_under_newer_nodata_without_clamping() {
    for signed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let nodata = if signed { -9999 } else { 0 };
        let older = if signed {
            vec![nodata, -100, 0, 1000, 10000, 32767]
        } else {
            vec![0, 1000, 7273, 10000, 30000, 65535]
        };
        let first = record(
            &root,
            signed,
            &fixture_samples(signed, &nodata.to_string(), 30.0, 1, &older),
        );
        let newer = vec![nodata, nodata, 5, nodata, 7, nodata];
        let second = record(
            &root,
            signed,
            &fixture_samples(signed, &nodata.to_string(), 30.0, 1, &newer),
        );
        let sources = [first.clone(), second];
        let project = project(&sources);
        let id = Uuid::new_v4().to_string();
        let output = write_mosaic(
            &root,
            &project,
            &sources,
            "red",
            &id,
            &CancellationToken::new(),
            None,
        )
        .unwrap();
        assert_eq!(output.plan.covered_pixels, 5);
        assert_eq!(output.plan.calibration.as_ref().unwrap().signed, signed);
        let mut decoded = Decoder::new(File::open(&output.path).unwrap()).unwrap();
        let bytes = decoded_bytes(
            decoded.read_image().unwrap(),
            output.plan.calibration.as_ref(),
            None,
            false,
            None,
        )
        .unwrap();
        let expected = [nodata, older[1], 5, older[3], 7, older[5]];
        let expected_bytes: Vec<u8> = expected
            .iter()
            .flat_map(|v| (*v as i16).to_le_bytes())
            .collect();
        assert_eq!(bytes, expected_bytes);
        let job = output_job(&root, &first, &project, output, id);
        let inspection = reflectance::inspect(&root, &job, 160).unwrap();
        assert_eq!(
            inspection.data_type,
            if signed { "Int16" } else { "UInt16" }
        );
        assert_eq!(
            inspection.reflectance.unwrap().pixel_interpretation,
            "PixelIsArea"
        );
    }
}

#[test]
fn point_grid_is_preserved_at_identical_pixel_centres_in_area_output() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = record(&root, false, &fixture_with_type(false, "0", 30.0, 2));
    let project = project(std::slice::from_ref(&source));
    let id = Uuid::new_v4().to_string();
    let output = write_mosaic(
        &root,
        &project,
        std::slice::from_ref(&source),
        "red",
        &id,
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    assert_eq!(
        output.plan.bounds,
        [499985.0, 4199955.0, 500075.0, 4200015.0]
    );
    let job = output_job(&root, &source, &project, output, id);
    let pixel = reflectance::sample(&root, &job, 500030.0, 4200000.0).unwrap();
    assert_eq!(pixel.value, 1000.0);
    assert_eq!(pixel.center, [500030.0, 4200000.0]);
    assert!((pixel.reflectance.unwrap() + 0.1725).abs() < 1e-12);
    let mut wrong_band = job.clone();
    wrong_band.asset_key = "green".into();
    wrong_band.mosaic.as_mut().unwrap().asset_key = "green".into();
    assert!(reflectance::inspect(&root, &wrong_band, 160)
        .unwrap_err()
        .contains("channel"));
    let mut wrong_geometry = job;
    wrong_geometry.mosaic_output.as_mut().unwrap().bounds[0] += 15.0;
    assert!(reflectance::inspect(&root, &wrong_geometry, 160)
        .unwrap_err()
        .contains("geometry"));
}

#[test]
fn mixed_calibration_sample_type_changed_hash_and_cancelled_sources_fail_without_output() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let first = record(&root, false, &fixture_samples(false, "0", 30.0, 1, &[1; 6]));
    let second = record(
        &root,
        true,
        &fixture_samples(true, "-9999", 30.0, 1, &[1; 6]),
    );
    let sources = [first.clone(), second];
    let project = project(&sources);
    let output_id = Uuid::new_v4().to_string();
    assert!(write_mosaic(
        &root,
        &project,
        &sources,
        "red",
        &output_id,
        &CancellationToken::new(),
        None
    )
    .is_err());
    let mut changed = first.clone();
    changed.sha256 = Some("0".repeat(64));
    let singleton = super::bands_tests::project(std::slice::from_ref(&changed));
    assert!(write_mosaic(
        &root,
        &singleton,
        &[changed],
        "red",
        &output_id,
        &CancellationToken::new(),
        None
    )
    .unwrap_err()
    .contains("SHA-256"));
    let cancel = CancellationToken::new();
    cancel.cancel();
    assert!(write_mosaic(
        &root,
        &singleton,
        &[first],
        "red",
        &output_id,
        &cancel,
        None
    )
    .is_err());
    assert!(!root
        .join("assets")
        .join(format!("{output_id}.tif"))
        .exists());
}

#[test]
fn signed_polygon_mask_uses_minus_9999_and_keeps_zero_and_negative_values_valid() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let source = record(
        &root,
        true,
        &fixture_samples(true, "-9999", 30.0, 1, &[-100, 0, 100, 200, 300, 400]),
    );
    let mut project = project(std::slice::from_ref(&source));
    let utm = proj4rs::Proj::from_proj_string("+proj=utm +zone=10 +datum=WGS84 +units=m +no_defs")
        .unwrap();
    let wgs = proj4rs::Proj::from_proj_string("+proj=longlat +datum=WGS84 +no_defs").unwrap();
    let ring = [
        [499990.0, 4199930.0],
        [500060.0, 4199930.0],
        [500060.0, 4200010.0],
        [499990.0, 4200010.0],
        [499990.0, 4199930.0],
    ]
    .map(|[x, y]| {
        let mut p = (x, y, 0.0);
        proj4rs::transform::transform(&utm, &wgs, &mut p).unwrap();
        [p.0.to_degrees(), p.1.to_degrees()]
    });
    project.geometry = Some(crop::PolygonGeometry::Polygon(vec![ring.to_vec()]));
    let id = Uuid::new_v4().to_string();
    let output = write_mosaic(
        &root,
        &project,
        std::slice::from_ref(&source),
        "red",
        &id,
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    assert_eq!(
        (output.plan.masked_pixels, output.plan.covered_pixels),
        (2, 4)
    );
    let mut decoder = Decoder::new(File::open(output.path).unwrap()).unwrap();
    match decoder.read_image().unwrap() {
        DecodingResult::I16(data) => assert_eq!(data, [-100, 0, -9999, 200, 300, -9999]),
        _ => panic!("signed output required"),
    }
}

#[tokio::test]
async fn derived_band_calibration_thumbnail_and_pixels_survive_manager_restart() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let root = manager.storage_root();
    let source = record(
        root,
        true,
        &fixture_samples(
            true,
            "-9999",
            30.0,
            1,
            &[-9999, -100, 0, 1000, 10000, 32767],
        ),
    );
    let project = project(std::slice::from_ref(&source));
    let id = Uuid::new_v4().to_string();
    let output = write_mosaic(
        root,
        &project,
        std::slice::from_ref(&source),
        "red",
        &id,
        &CancellationToken::new(),
        None,
    )
    .unwrap();
    let mut job = output_job(root, &source, &project, output, id);
    let saved = BTreeMap::from([(source.id.clone(), source), (job.id.clone(), job.clone())]);
    manager.inner.store.lock().await.jobs = saved.clone();
    manager.persist(&saved).await.unwrap();
    let first = manager.file_thumbnail(&job.id).await.unwrap();
    drop(manager);
    let restored = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(
        restored.file_thumbnail(&job.id).await.unwrap().data_url,
        first.data_url
    );
    assert_eq!(
        restored
            .sample_raster(&job.id, 500045.0, 4199985.0)
            .await
            .unwrap()
            .value,
        -100.0
    );
    job.mosaic_output
        .as_mut()
        .unwrap()
        .calibration
        .as_mut()
        .unwrap()
        .offset = 0.25;
    assert!(reflectance::inspect(restored.storage_root(), &job, 160)
        .unwrap_err()
        .contains("calibration"));
}
