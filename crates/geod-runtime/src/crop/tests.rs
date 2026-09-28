use super::*;
use crate::raster::tests::{fixture, record};

fn parameters(bounds: [f64; 4]) -> ClipParameters {
    ClipParameters {
        crs: "source".into(),
        bounds,
        geometry: None,
    }
}

fn output_job(source: &Job, output: &CropOutput, id: &str) -> Job {
    let mut job = source.clone();
    job.id = id.into();
    job.output_path = Some(output.output_path.clone());
    job.sha256 = Some(output.sha256.clone());
    job.bytes_downloaded = output.bytes;
    job.total_bytes = Some(output.bytes);
    job.kind = "raster_clip".into();
    job.parent_id = Some(source.id.clone());
    job.crop = Some(output.plan.clone());
    job
}

fn wgs84_ring(points: &[[f64; 2]]) -> Vec<[f64; 2]> {
    let (wgs84, utm) = projections("EPSG:32610").unwrap();
    points
        .iter()
        .map(|[x, y]| {
            let mut point = (*x, *y, 0.0);
            proj4rs::transform::transform(&utm, &wgs84, &mut point).unwrap();
            [point.0.to_degrees(), point.1.to_degrees()]
        })
        .collect()
}

#[test]
fn polygon_clip_masks_holes_and_preserves_original_source() {
    let directory = tempfile::tempdir().unwrap();
    let original = fixture(5, 4, &[4; 20], 32610, false);
    let source = record(directory.path(), &original);
    let outer = wgs84_ring(&[
        [500000.0, 4200000.0],
        [500100.0, 4200000.0],
        [500100.0, 4199920.0],
        [500000.0, 4199920.0],
        [500000.0, 4200000.0],
    ]);
    let hole = wgs84_ring(&[
        [500020.0, 4199980.0],
        [500080.0, 4199980.0],
        [500080.0, 4199940.0],
        [500020.0, 4199940.0],
        [500020.0, 4199980.0],
    ]);
    let geometry = PolygonGeometry::Polygon(vec![outer, hole]);
    let parameters = ClipParameters {
        crs: "EPSG:4326".into(),
        bounds: geometry.bounds().unwrap(),
        geometry: Some(geometry),
    };
    let output_id = uuid::Uuid::new_v4().to_string();
    let output = write_crop(
        directory.path(),
        &source,
        &parameters,
        &output_id,
        &CancellationToken::new(),
    )
    .unwrap();
    assert!(output.plan.masked_pixels.unwrap() > 0);
    let raster = load_verified_raster(
        directory.path(),
        &output_job(&source, &output, &output_id),
        None,
    )
    .unwrap();
    assert!(raster.pixels.contains(&0));
    assert!(raster.pixels.contains(&4));
    assert_eq!(raster.nodata, Some(0));
    assert_eq!(
        std::fs::read(source.output_path.as_ref().unwrap()).unwrap(),
        original
    );
}

#[test]
fn polygon_validation_rejects_unclosed_rings_and_nonoverlapping_windows() {
    let geometry = PolygonGeometry::Polygon(vec![vec![
        [-123.0, 37.0],
        [-122.0, 37.0],
        [-122.0, 38.0],
        [-123.0, 38.0],
    ]]);
    assert!(geometry.bounds().unwrap_err().contains("closed"));
    let geometry = PolygonGeometry::Polygon(vec![vec![
        [-123.0, 37.0],
        [-122.0, 37.0],
        [-122.0, 38.0],
        [-123.0, 37.0],
    ]]);
    let parameters = ClipParameters {
        crs: "EPSG:4326".into(),
        bounds: [-125.0, 35.0, -124.0, 36.0],
        geometry: Some(geometry),
    };
    assert!(validate_parameters(&parameters)
        .unwrap_err()
        .contains("overlap"));
}

#[test]
fn source_window_alignment_clamping_and_edges_are_explicit() {
    let directory = tempfile::tempdir().unwrap();
    let values: Vec<_> = (0..20).map(|i| (i % 12) as u8).collect();
    let source = record(directory.path(), &fixture(5, 4, &values, 32610, false));
    let exact = plan_crop(
        directory.path(),
        &source,
        &parameters([500020.0, 4199940.0, 500080.0, 4199980.0]),
    )
    .unwrap();
    assert_eq!(exact.window, [1, 1, 3, 2]);
    assert_eq!(exact.bounds, [500020.0, 4199940.0, 500080.0, 4199980.0]);
    assert!(exact.warnings.is_empty());
    let aligned = plan_crop(
        directory.path(),
        &source,
        &parameters([500021.0, 4199941.0, 500079.0, 4199979.0]),
    )
    .unwrap();
    assert_eq!(aligned.window, [1, 1, 3, 2]);
    assert!(aligned
        .warnings
        .iter()
        .any(|w| w.contains("pixel boundaries")));
    let clamped = plan_crop(
        directory.path(),
        &source,
        &parameters([499900.0, 4199900.0, 500200.0, 4200100.0]),
    )
    .unwrap();
    assert_eq!(clamped.window, [0, 0, 5, 4]);
    assert_eq!(clamped.bounds, [500000.0, 4199920.0, 500100.0, 4200000.0]);
    assert!(clamped
        .warnings
        .iter()
        .any(|w| w.contains("source raster extent")));
    let corner = plan_crop(
        directory.path(),
        &source,
        &parameters([500080.0, 4199920.0, 500100.0, 4199940.0]),
    )
    .unwrap();
    assert_eq!(corner.window, [4, 3, 1, 1]);
}

#[test]
fn crop_is_real_geotiff_preserving_exact_pixels_crs_nodata_and_original() {
    let directory = tempfile::tempdir().unwrap();
    let values: Vec<_> = (0..20).map(|i| (i % 12) as u8).collect();
    let original = fixture(5, 4, &values, 32610, false);
    let source = record(directory.path(), &original);
    let crop = parameters([500020.0, 4199940.0, 500080.0, 4199980.0]);
    let id = uuid::Uuid::new_v4().to_string();
    let output = write_crop(
        directory.path(),
        &source,
        &crop,
        &id,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(output.plan.format, "GeoTIFF");
    assert_eq!(output.plan.source_sha256, source.sha256.clone().unwrap());
    let raster =
        load_verified_raster(directory.path(), &output_job(&source, &output, &id), None).unwrap();
    assert_eq!(raster.pixels, [6, 7, 8, 11, 0, 1]);
    assert_eq!((raster.width, raster.height), (3, 2));
    assert_eq!(raster.crs, "EPSG:32610");
    assert_eq!(raster.pixel_size, [20.0, 20.0]);
    assert_eq!(raster.nodata, Some(0));
    assert_eq!(raster.bounds, output.plan.bounds);
    assert_eq!(
        std::fs::read(source.output_path.as_ref().unwrap()).unwrap(),
        original
    );
    assert!(std::fs::read_dir(directory.path().join("assets"))
        .unwrap()
        .all(|entry| entry.unwrap().path().extension().unwrap() == "tif"));
    let second_id = uuid::Uuid::new_v4().to_string();
    let second = write_crop(
        directory.path(),
        &source,
        &crop,
        &second_id,
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(
        output.sha256, second.sha256,
        "identical recipe/source must encode identical bytes"
    );
    assert!(write_crop(
        directory.path(),
        &source,
        &crop,
        &id,
        &CancellationToken::new()
    )
    .unwrap_err()
    .contains("already exists"));
    assert_eq!(
        std::fs::read(&output.output_path).unwrap(),
        std::fs::read(&second.output_path).unwrap()
    );
}

#[test]
fn wgs84_utm_projection_matches_known_reference_coordinates() {
    // UTM zone central meridians have easting 500000; WGS84 TM meridional
    // distance at 40 N is 4427757.218738374 m (PROJ reference calculation).
    let (from, to) = projections("EPSG:32610").unwrap();
    let central = project_point(&from, &to, -123.0, 0.0).unwrap();
    assert!((central[0] - 500000.0).abs() < 1e-6);
    assert!(central[1].abs() < 1e-6);
    let north = project_point(&from, &to, -123.0, 40.0).unwrap();
    assert!((north[0] - 500000.0).abs() < 1e-6);
    assert!((north[1] - 4427757.218738374).abs() < 0.01);
    let (from, to) = projections("EPSG:32756").unwrap();
    let south = project_point(&from, &to, 153.0, -40.0).unwrap();
    assert!((south[0] - 500000.0).abs() < 1e-6);
    assert!((south[1] - 5572242.781261626).abs() < 0.01);
}

#[test]
fn wgs84_envelope_samples_edges_not_only_corners() {
    let envelope = projected_envelope([-124.0, 37.0, -122.0, 38.0], "EPSG:32610").unwrap();
    let (from, to) = projections("EPSG:32610").unwrap();
    let center = project_point(&from, &to, -123.0, 37.0).unwrap();
    let corner = project_point(&from, &to, -124.0, 37.0).unwrap();
    assert!((envelope[1] - center[1]).abs() < 1e-7);
    assert!(
        envelope[1] < corner[1] - 100.0,
        "south-edge interior supplies minimum northing"
    );
}

#[test]
fn wgs84_request_produces_a_real_intersecting_source_grid_window() {
    let directory = tempfile::tempdir().unwrap();
    // The fixture's 500000 / 4200000 origin is near -123 / 37.9476 degrees.
    let source = record(
        directory.path(),
        &fixture(100, 100, &vec![4; 10000], 32610, false),
    );
    let params = ClipParameters {
        crs: "EPSG:4326".into(),
        bounds: [-123.0, 37.94, -122.99, 37.947],
        geometry: None,
    };
    let plan = plan_crop(directory.path(), &source, &params).unwrap();
    assert!(plan.width > 0 && plan.width <= 100 && plan.height > 0 && plan.height <= 100);
    assert_eq!(plan.requested_bounds, params.bounds);
    assert_eq!(plan.requested_crs, "EPSG:4326");
    assert!(plan.warnings.iter().any(|w| w.contains("densified")));
    let output = write_crop(
        directory.path(),
        &source,
        &params,
        &uuid::Uuid::new_v4().to_string(),
        &CancellationToken::new(),
    )
    .unwrap();
    assert_eq!(output.plan.window, plan.window);
}

#[test]
fn rejects_empty_outside_invalid_crs_and_antimeridian_requests() {
    let directory = tempfile::tempdir().unwrap();
    let source = record(
        directory.path(),
        &fixture(2, 2, &[4, 4, 6, 6], 32610, false),
    );
    for bounds in [
        [500040.0, 4199960.0, 500060.0, 4200000.0],
        [500000.0, 4200000.0, 500040.0, 4200020.0],
        [500000.0, 4199980.0, 500000.0, 4200000.0],
        [f64::NAN, 0.0, 1.0, 1.0],
    ] {
        assert!(plan_crop(directory.path(), &source, &parameters(bounds)).is_err());
    }
    for params in [
        ClipParameters {
            crs: "EPSG:3857".into(),
            bounds: [0.0, 0.0, 1.0, 1.0],
            geometry: None,
        },
        ClipParameters {
            crs: "EPSG:4326".into(),
            bounds: [179.0, -1.0, -179.0, 1.0],
            geometry: None,
        },
        ClipParameters {
            crs: "EPSG:4326".into(),
            bounds: [-123.0, 85.0, -122.0, 86.0],
            geometry: None,
        },
    ] {
        assert!(plan_crop(directory.path(), &source, &params).is_err());
    }
    assert!(write_crop(
        directory.path(),
        &source,
        &parameters([500000.0, 4199960.0, 500040.0, 4200000.0]),
        "../outside",
        &CancellationToken::new()
    )
    .is_err());
}

#[test]
fn cancellation_never_publishes_an_output_or_mutates_source() {
    let directory = tempfile::tempdir().unwrap();
    let original = fixture(2, 2, &[4, 4, 6, 6], 32610, false);
    let source = record(directory.path(), &original);
    let token = CancellationToken::new();
    token.cancel();
    let id = uuid::Uuid::new_v4().to_string();
    assert!(write_crop(
        directory.path(),
        &source,
        &parameters([500000.0, 4199960.0, 500040.0, 4200000.0]),
        &id,
        &token
    )
    .unwrap_err()
    .contains("cancelled"));
    assert!(!directory
        .path()
        .join("assets")
        .join(format!("{id}.tif"))
        .exists());
    assert_eq!(
        std::fs::read_dir(directory.path().join("assets"))
            .unwrap()
            .count(),
        1
    );
    assert_eq!(
        std::fs::read(source.output_path.unwrap()).unwrap(),
        original
    );
}

#[test]
fn cancelling_during_geotiff_encoding_removes_partial_output() {
    let directory = tempfile::tempdir().unwrap();
    let source = record(
        directory.path(),
        &fixture(6400, 6400, &vec![4; 6400 * 6400], 32610, false),
    );
    let token = CancellationToken::new();
    let monitor_token = token.clone();
    let assets = directory.path().join("assets");
    let monitor = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(10);
        while Instant::now() < deadline {
            if std::fs::read_dir(&assets).unwrap().any(|entry| {
                entry
                    .unwrap()
                    .path()
                    .extension()
                    .is_some_and(|ext| ext == "part")
            }) {
                monitor_token.cancel();
                return true;
            }
            std::thread::sleep(Duration::from_millis(1));
        }
        monitor_token.cancel();
        false
    });
    let id = uuid::Uuid::new_v4().to_string();
    let result = write_crop(
        directory.path(),
        &source,
        &parameters([500000.0, 4072000.0, 628000.0, 4200000.0]),
        &id,
        &token,
    );
    assert!(
        monitor.join().unwrap(),
        "test did not observe the partial encoding stage"
    );
    assert!(result.is_err());
    assert_eq!(
        std::fs::read_dir(directory.path().join("assets"))
            .unwrap()
            .count(),
        1
    );
    assert!(!directory
        .path()
        .join("assets")
        .join(format!("{id}.tif"))
        .exists());
}
