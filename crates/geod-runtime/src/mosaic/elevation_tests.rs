use super::*;
use crate::{
    projects::{CreateProjectRequest, ProjectAsset, ProjectScene},
    raster::elevation,
};
use std::io::Cursor;
use tiff::encoder::colortype;

fn tile(root: &Path, lon: i32, width: u32, nodata: Option<&str>) -> Job {
    product_tile(
        root,
        lon,
        width,
        nodata,
        crate::providers::DemProduct::Glo30Public,
    )
}
fn product_tile(
    root: &Path,
    lon: i32,
    width: u32,
    nodata: Option<&str>,
    product: crate::providers::DemProduct,
) -> Job {
    let height = product.height();
    let mut samples = (0..height)
        .flat_map(|_| (0..width).map(|col| [0.0f32, -79.079_38, 657.42194][col as usize % 3]))
        .collect::<Vec<_>>();
    if nodata == Some("nan") {
        samples[1] = f32::NAN;
    }
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
        let mut image = encoder
            .new_image::<colortype::Gray32Float>(width, height)
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::GeoKeyDirectoryTag,
                &[
                    1u16, 1, 0, 4, 1024, 0, 1, 2, 1025, 0, 1, 2, 2048, 0, 1, 4326, 2054, 0, 1, 9102,
                ][..],
            )
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::ModelPixelScaleTag,
                &[1.0 / width as f64, 1.0 / f64::from(height), 0.0][..],
            )
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::ModelTiepointTag,
                &[0.0, 0.0, 0.0, lon as f64, 86.0, 0.0][..],
            )
            .unwrap();
        if let Some(nodata) = nodata {
            image.encoder().write_tag(Tag::GdalNodata, nodata).unwrap();
        }
        image.write_data(&samples).unwrap();
    }
    let mut job = crate::raster::tests::record(root, &bytes.into_inner());
    let resolution = if product == crate::providers::DemProduct::Glo90 {
        "30"
    } else {
        "10"
    };
    job.item_id = format!("Copernicus_DSM_COG_{resolution}_N85_00_W{:03}_00_DEM", -lon);
    job.asset_key = "elevation".into();
    job.href = format!(
        "https://{}/{}/{}.tif",
        product.host(),
        job.item_id,
        job.item_id
    );
    job
}
fn request(
    sources: &[Job],
    bounds: [f64; 4],
    geometry: Option<crop::PolygonGeometry>,
) -> CreateProjectRequest {
    CreateProjectRequest {
        name: "Elevation fixture".into(),
        bounds,
        geometry,
        scenes: sources
            .iter()
            .map(|job| {
                let [lon, lat] = crate::providers::dem_cell(&job.item_id).unwrap();
                ProjectScene {
                    item_id: job.item_id.clone(),
                    date: "2021-01-01T00:00:00Z".into(),
                    cloud: None,
                    crs: Some("EPSG:4326".into()),
                    grid_code: Some(job.item_id.clone()),
                    bbox: [lon as f64, lat as f64, (lon + 1) as f64, (lat + 1) as f64],
                    assets: BTreeMap::from([(
                        "elevation".into(),
                        ProjectAsset {
                            href: job.href.clone(),
                            media_type: "image/tiff".into(),
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
        bounds: r.bounds,
        geometry: r.geometry,
        scenes: r.scenes,
        stac_items: Vec::new(),
        wcs_items: Vec::new(),
        created_at: now(),
        updated_at: now(),
    }
}

#[test]
fn adjacent_point_tiles_keep_exact_float32_seam_and_vertical_reference() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let sources = [tile(&root, -123, 360, None), tile(&root, -122, 360, None)];
    let dx = 1.0 / 360.0;
    let dy = 1.0 / 3600.0;
    let project = project(
        &sources,
        [
            -122.0 - 2.1 * dx,
            86.0 - 1.1 * dy,
            -122.0 + 1.1 * dx,
            86.0 + 0.1 * dy,
        ],
    );
    let output = write_mosaic(
        &root,
        &project,
        &sources,
        "elevation",
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
        (4, 2, 8)
    );
    assert_eq!(output.plan.elevation, Some(elevation::profile()));
    let mut decoder = Decoder::new(File::open(output.path).unwrap()).unwrap();
    elevation::validate_header(&mut decoder, "", Some(&output.plan)).unwrap();
    let DecodingResult::F32(values) = decoder.read_image().unwrap() else {
        panic!("Float32 required")
    };
    assert_eq!(
        values,
        [-79.079_38_f32, 657.42194, 0.0, -79.079_38].repeat(2)
    );
}

#[tokio::test]
async fn queued_elevation_polygon_mask_persists_nan_and_original_values_after_restart() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let source = tile(manager.storage_root(), -123, 360, Some("nan"));
    let sources = [source.clone()];
    let dx = 1.0 / 360.0;
    let dy = 1.0 / 3600.0;
    let bounds = [
        -123.0 - 0.1 * dx,
        86.0 - 1.1 * dy,
        -123.0 + 2.1 * dx,
        86.0 + 0.1 * dy,
    ];
    let ring = vec![
        [bounds[0], bounds[1]],
        [-123.0 + 1.4 * dx, bounds[1]],
        [-123.0 + 1.4 * dx, bounds[3]],
        [bounds[0], bounds[3]],
        [bounds[0], bounds[1]],
    ];
    let project = manager
        .create_project(request(
            &sources,
            bounds,
            Some(crop::PolygonGeometry::Polygon(vec![ring])),
        ))
        .await
        .unwrap();
    let saved = BTreeMap::from([(source.id.clone(), source)]);
    manager.inner.store.lock().await.jobs = saved.clone();
    manager.persist(&saved).await.unwrap();
    let job = manager
        .run_project_mosaic(&project.id, "elevation")
        .await
        .unwrap();
    let job = manager.wait(&job.id).await.unwrap();
    assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
    assert!(Path::new(job.manifest_path.as_ref().unwrap()).exists());
    let plan = job.mosaic_output.as_ref().unwrap();
    assert_eq!(
        (
            plan.width,
            plan.height,
            plan.covered_pixels,
            plan.masked_pixels
        ),
        (3, 2, 3, 2)
    );
    let metadata = manager.inspect_raster(&job.id).await.unwrap();
    assert!(metadata.elevation.unwrap().nodata_is_nan);
    let zero = manager.sample_raster(&job.id, -123.0, 86.0).await.unwrap();
    assert_eq!(zero.value, 0.0);
    assert!(!zero.is_no_data);
    let masked = manager
        .sample_raster(&job.id, -123.0 + 2.0 * dx, 86.0)
        .await
        .unwrap();
    assert!(masked.is_no_data && masked.value.is_nan());
    assert!(serde_json::to_value(masked).unwrap()["value"].is_null());
    let thumbnail = manager.file_thumbnail(&job.id).await.unwrap();
    drop(manager);
    let restored = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(
        restored.file_thumbnail(&job.id).await.unwrap().data_url,
        thumbnail.data_url
    );
    let pixel = restored
        .sample_raster(&job.id, -123.0 + dx, 86.0 - dy)
        .await
        .unwrap();
    assert_eq!(pixel.value, f64::from(-79.079_38_f32));
    assert!(!pixel.is_no_data);
    let mut changed = job.clone();
    changed
        .mosaic_output
        .as_mut()
        .unwrap()
        .elevation
        .as_mut()
        .unwrap()
        .height_unit = "foot".into();
    assert!(elevation::inspect(restored.storage_root(), &changed, 160).is_err());
}

#[test]
fn glo90_seam_preserves_product_grid_and_rejects_mixed_copernicus_products() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let sources = [
        product_tile(&root, -123, 120, None, crate::providers::DemProduct::Glo90),
        product_tile(&root, -122, 120, None, crate::providers::DemProduct::Glo90),
    ];
    let dx = 1.0 / 120.0;
    let dy = 1.0 / 1200.0;
    let glo90_project = project(
        &sources,
        [
            -122.0 - 2.1 * dx,
            86.0 - 1.1 * dy,
            -122.0 + 1.1 * dx,
            86.0 + 0.1 * dy,
        ],
    );
    let output = write_mosaic(
        &root,
        &glo90_project,
        &sources,
        "elevation",
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
        (4, 2, 8)
    );
    assert_eq!(
        output.plan.elevation.as_ref().unwrap().product,
        "cop-dem-glo-90"
    );
    let mut decoder = Decoder::new(File::open(output.path).unwrap()).unwrap();
    elevation::validate_header(&mut decoder, "", Some(&output.plan)).unwrap();
    let DecodingResult::F32(values) = decoder.read_image().unwrap() else {
        panic!("Float32 required")
    };
    assert_eq!(
        values,
        [-79.079_38_f32, 657.42194, 0.0, -79.079_38].repeat(2)
    );
    let mixed = [sources[0].clone(), tile(&root, -122, 360, None)];
    let mixed_project = project(&mixed, glo90_project.bounds);
    let id = Uuid::new_v4().to_string();
    assert!(write_mosaic(
        &root,
        &mixed_project,
        &mixed,
        "elevation",
        &id,
        &CancellationToken::new(),
        None
    )
    .is_err());
    assert!(!root.join("assets").join(format!("{id}.tif")).exists());
}

#[test]
fn incompatible_spacing_checksum_and_cancel_never_commit_elevation_output() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let first = tile(&root, -123, 360, None);
    let second = tile(&root, -122, 720, None);
    let sources = [first.clone(), second];
    let project = project(&sources, [-123.0, 85.9, -121.9, 86.0]);
    let id = Uuid::new_v4().to_string();
    let cancel = CancellationToken::new();
    assert!(
        write_mosaic(&root, &project, &sources, "elevation", &id, &cancel, None)
            .unwrap_err()
            .contains("pixel sizes")
    );
    let mut changed = first.clone();
    changed.sha256 = Some("0".repeat(64));
    let mut changed_project = project.clone();
    changed_project.scenes.truncate(1);
    assert!(write_mosaic(
        &root,
        &changed_project,
        &[changed],
        "elevation",
        &id,
        &cancel,
        None
    )
    .unwrap_err()
    .contains("SHA-256"));
    cancel.cancel();
    assert!(write_mosaic(&root, &project, &sources, "elevation", &id, &cancel, None).is_err());
    assert!(!root.join("assets").join(format!("{id}.tif")).exists());
}
