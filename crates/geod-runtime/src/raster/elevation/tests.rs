use super::*;
use tiff::encoder::{colortype, TiffEncoder};

const ID: &str = "Copernicus_DSM_COG_10_N85_00_W123_00_DEM";
fn fixture(nodata: Option<&str>, epsg: u16, point: u16, longitude: f64) -> Vec<u8> {
    product_fixture(
        providers::DemProduct::Glo30Public,
        nodata,
        epsg,
        point,
        longitude,
        None,
    )
}
fn product_fixture(
    product: providers::DemProduct,
    nodata: Option<&str>,
    epsg: u16,
    point: u16,
    longitude: f64,
    vertical: Option<[u16; 2]>,
) -> Vec<u8> {
    let width = *product.widths().last().unwrap();
    let height = product.height();
    let mut data = vec![12.375f32; (width * height) as usize];
    data[0] = 0.0;
    data[1] = -79.079_38;
    data[2] = 657.42194;
    if nodata == Some("nan") {
        data[0] = f32::NAN;
    }
    let mut bytes = Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
        let mut image = encoder
            .new_image::<colortype::Gray32Float>(width, height)
            .unwrap();
        let mut keys = vec![
            1u16, 1, 0, 4, 1024, 0, 1, 2, 1025, 0, 1, point, 2048, 0, 1, epsg, 2054, 0, 1, 9102,
        ];
        if let Some([reference, unit]) = vertical {
            keys[3] += 2;
            keys.extend([4096, 0, 1, reference, 4099, 0, 1, unit]);
        }
        image
            .encoder()
            .write_tag(Tag::GeoKeyDirectoryTag, &keys[..])
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::ModelPixelScaleTag,
                &[1.0 / f64::from(width), 1.0 / f64::from(height), 0.0][..],
            )
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::ModelTiepointTag,
                &[0.0, 0.0, 0.0, longitude, 86.0, 0.0][..],
            )
            .unwrap();
        if let Some(n) = nodata {
            image.encoder().write_tag(Tag::GdalNodata, n).unwrap();
        }
        image.write_data(&data).unwrap();
    }
    bytes.into_inner()
}
fn record(root: &Path, bytes: &[u8]) -> Job {
    let mut job = crate::raster::tests::record(root, bytes);
    job.item_id = ID.into();
    job.asset_key = "elevation".into();
    job.href = format!("https://{}/{ID}/{ID}.tif", providers::DEM_HOST);
    job
}

#[test]
fn float32_height_preserves_negative_fractional_and_zero_values_in_point_grid() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = fixture(None, 4326, 2, -123.0);
    let job = record(dir.path(), &bytes);
    for (col, value) in [(0, 0.0f32), (1, -79.079_38), (2, 657.42194)] {
        let p = sample(dir.path(), &job, -123.0 + col as f64 / 360.0, 86.0).unwrap();
        assert_eq!(p.pixel, [col, 0]);
        assert_eq!(p.value, f64::from(value));
        assert!(!p.is_no_data);
        assert!(p.reflectance.is_none() && p.values.is_none());
    }
    let data = inspect(dir.path(), &job, 160).unwrap();
    assert_eq!(
        (
            data.width,
            data.height,
            data.data_type.as_str(),
            data.crs.as_str()
        ),
        (360, 3600, "Float32", "EPSG:4326")
    );
    assert_eq!(data.pixel_size, [1.0 / 360.0, 1.0 / 3600.0]);
    assert_eq!(data.bounds[0], -123.0 - 0.5 / 360.0);
    assert_eq!(data.bounds[3], 86.0 + 0.5 / 3600.0);
    assert_eq!(data.nodata, None);
    assert!(data.classes.is_empty());
    let info = data.elevation.unwrap();
    assert_eq!(info.vertical_reference, "EPSG:3855");
    assert_eq!(info.height_unit, "metre");
    assert_eq!(info.coordinate_unit, "degree");
    assert_eq!(info.valid_sample_count, info.sample_count);
}
#[test]
fn nan_nodata_is_transparent_and_serializes_as_null_without_inventing_zero_height() {
    let dir = tempfile::tempdir().unwrap();
    let job = record(dir.path(), &fixture(Some("nan"), 4326, 2, -123.0));
    let p = sample(dir.path(), &job, -123.0, 86.0).unwrap();
    assert!(p.is_no_data && p.value.is_nan());
    let json = serde_json::to_value(p).unwrap();
    assert!(json["value"].is_null());
    let data = inspect(dir.path(), &job, 160).unwrap();
    assert!(data.elevation.as_ref().unwrap().nodata_is_nan);
    assert!(
        data.elevation.as_ref().unwrap().valid_sample_count
            < data.elevation.as_ref().unwrap().sample_count
    );
    assert!(serde_json::to_value(data).is_ok());
}
#[test]
fn wrong_crs_raster_interpretation_geocell_and_nodata_are_rejected() {
    for (epsg, point, lon, nodata) in [
        (3857, 2, -123.0, None),
        (4326, 1, -123.0, None),
        (4326, 2, -122.0, None),
        (4326, 2, -123.0, Some("inf")),
    ] {
        let bytes = fixture(nodata, epsg, point, lon);
        assert!(source(&bytes, ID).is_err());
    }
}
#[test]
fn altered_bytes_wrong_asset_key_and_different_product_cannot_be_inspected() {
    let dir = tempfile::tempdir().unwrap();
    let bytes = fixture(None, 4326, 2, -123.0);
    let job = record(dir.path(), &bytes);
    let mut wrong = job.clone();
    wrong.asset_key = "scl".into();
    assert!(inspect(dir.path(), &wrong, 160).is_err());
    wrong = job.clone();
    wrong.item_id = ID.replace("N85", "N84");
    assert!(inspect(dir.path(), &wrong, 160).is_err());
    std::fs::write(
        dir.path().join("assets").join(format!("{}.tif", job.id)),
        [0; 8],
    )
    .unwrap();
    assert!(inspect(dir.path(), &job, 160).is_err());
}
#[test]
fn source_urls_are_exact_unsigned_public_geocells_only() {
    let href = format!("https://{}/{ID}/{ID}.tif", providers::DEM_HOST);
    assert!(providers::asset_url(&href).is_ok());
    assert!(providers::matches_item(
        &providers::asset_url(&href).unwrap(),
        ID,
        "elevation"
    ));
    for bad in [
        format!("{href}?token=private"),
        href.replace(providers::DEM_HOST, "copernicus-dem-30m.evil.example"),
        href.replace("COG_10_", "COG_30_"),
        href.replace("N85", "N90"),
        href.replace("W123", "W181"),
        href.replace(".tif", ".zip"),
    ] {
        assert!(providers::asset_url(&bad).is_err());
    }
    assert!(providers::dem_cell("Copernicus_DSM_COG_10_Nä5_00_W123_00_DEM").is_none());
}

#[test]
fn glo90_grid_and_values_are_verified_without_accepting_glo30_or_conflicting_vertical_tags() {
    let product = providers::DemProduct::Glo90;
    let id = ID.replace("COG_10_", "COG_30_");
    let href = format!("https://{}/{id}/{id}.tif", product.host());
    assert!(providers::matches_item(
        &providers::asset_url(&href).unwrap(),
        &id,
        "elevation"
    ));
    assert!(providers::asset_url(&href.replace(product.host(), providers::DEM_HOST)).is_err());
    let dir = tempfile::tempdir().unwrap();
    let bytes = product_fixture(product, None, 4326, 2, -123.0, Some([3855, 9001]));
    let mut job = record(dir.path(), &bytes);
    job.item_id = id.clone();
    job.href = href;
    let data = inspect(dir.path(), &job, 160).unwrap();
    assert_eq!((data.width, data.height), (120, 1200));
    assert_eq!(data.elevation.as_ref().unwrap().product, "cop-dem-glo-90");
    for (col, value) in [(0, 0.0f32), (1, -79.079_38), (2, 657.42194)] {
        let pixel = sample(dir.path(), &job, -123.0 + f64::from(col) / 120.0, 86.0).unwrap();
        assert_eq!(pixel.pixel, [col, 0]);
        assert_eq!(pixel.value, f64::from(value));
    }
    assert!(source(&fixture(None, 4326, 2, -123.0), &id).is_err());
    assert!(source(&bytes, ID).is_err());
    for vertical in [Some([5773, 9001]), Some([3855, 9002])] {
        assert!(source(
            &product_fixture(product, None, 4326, 2, -123.0, vertical),
            &id
        )
        .is_err());
    }
}

#[tokio::test]
#[ignore = "Downloads actual public Copernicus DEM into GEOD_LIVE_DEM_DIR"]
async fn live_dem_download_pixels_thumbnail_and_restart_match_independent_float32_reference() {
    use crate::{CreateJobRequest, JobManager, JobStatus};
    let root = std::env::var("GEOD_LIVE_DEM_DIR").expect("Set an isolated verification directory");
    let manager = JobManager::open(&root).await.unwrap();
    assert!(
        manager.list().await.is_empty(),
        "Use a new isolated verification directory"
    );
    let id = "Copernicus_DSM_COG_10_N37_00_W123_00_DEM";
    let job = manager
        .create(CreateJobRequest {
            item_id: id.into(),
            asset_key: "elevation".into(),
            href: format!("https://{}/{id}/{id}.tif", providers::DEM_HOST),
            media_type: "image/tiff; application=geotiff".into(),
            title: None,
        })
        .await
        .unwrap();
    let job = tokio::time::timeout(Duration::from_secs(180), manager.wait(&job.id))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
    assert_eq!(job.bytes_downloaded, 20_060_161);
    assert_eq!(
        job.sha256.as_deref(),
        Some("82343318fbf7265c32342d9cace0fda4eb48d4b567a1a3335cc37488d4a9a7d5")
    );
    let metadata = manager.inspect_raster(&job.id).await.unwrap();
    assert_eq!((metadata.width, metadata.height), (3600, 3600));
    for (row, col, value) in [
        (0, 0, 101.38304901123047),
        (49, 1963, -79.07937622070312),
        (1000, 1000, 0.0),
        (1800, 1800, 0.0),
        (2500, 3000, 657.4219360351562),
        (3599, 3599, 69.05463409423828),
    ] {
        let pixel = manager
            .sample_raster(
                &job.id,
                -123.0 + col as f64 / 3600.0,
                38.0 - row as f64 / 3600.0,
            )
            .await
            .unwrap();
        assert_eq!(pixel.pixel, [col, row]);
        assert_eq!(pixel.value, value);
        assert!(!pixel.is_no_data);
    }
    let thumb = manager.file_thumbnail(&job.id).await.unwrap();
    assert_eq!((thumb.width, thumb.height), (160, 160));
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(&root).await.unwrap();
    assert_eq!(
        reopened.file_thumbnail(&job.id).await.unwrap().data_url,
        thumb.data_url
    );
    assert_eq!(
        reopened.inspect_raster(&job.id).await.unwrap().sha256,
        metadata.sha256
    );
    reopened.shutdown().await.unwrap();
    println!("Actual DEM: 20060161 bytes, reference SHA-256, six full-resolution Float32 values, 160 px persistent thumbnail, restart verified.");
}
