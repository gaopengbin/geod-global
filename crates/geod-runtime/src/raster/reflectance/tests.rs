use super::*;
use tiff::encoder::{colortype, TiffEncoder};

pub(crate) fn fixture(signed: bool, nodata: &str, scale: f64) -> Vec<u8> {
    fixture_with_type(signed, nodata, scale, 1)
}

pub(crate) fn fixture_with_type(
    signed: bool,
    nodata: &str,
    scale: f64,
    raster_type: u16,
) -> Vec<u8> {
    fixture_samples(
        signed,
        nodata,
        scale,
        raster_type,
        if signed {
            &[-9999, -100, 0, 1000, 10000, 32767]
        } else {
            &[0, 1000, 7273, 10000, 30000, 65535]
        },
    )
}

pub(crate) fn fixture_samples(
    signed: bool,
    nodata: &str,
    scale: f64,
    raster_type: u16,
    values: &[i32],
) -> Vec<u8> {
    fixture_sized(signed, nodata, scale, raster_type, values, 3, 2)
}

pub(crate) fn fixture_sized(
    signed: bool,
    nodata: &str,
    scale: f64,
    raster_type: u16,
    values: &[i32],
    width: u32,
    height: u32,
) -> Vec<u8> {
    fn encode<C: colortype::ColorType>(
        pixels: &[C::Inner],
        nodata: &str,
        scale: f64,
        raster_type: u16,
        dimensions: [u32; 2],
    ) -> Vec<u8>
    where
        [C::Inner]: tiff::encoder::TiffValue,
    {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder
                .new_image::<C>(dimensions[0], dimensions[1])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::GeoKeyDirectoryTag,
                    &[
                        1u16,
                        1,
                        0,
                        4,
                        1024,
                        0,
                        1,
                        1,
                        1025,
                        0,
                        1,
                        raster_type,
                        3072,
                        0,
                        1,
                        32610,
                        3076,
                        0,
                        1,
                        9001,
                    ][..],
                )
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::ModelPixelScaleTag, &[scale, scale, 0.0][..])
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::ModelTiepointTag,
                    &[0.0, 0.0, 0.0, 500000.0, 4200000.0, 0.0][..],
                )
                .unwrap();
            image.encoder().write_tag(Tag::GdalNodata, nodata).unwrap();
            image.write_data(pixels).unwrap();
        }
        bytes.into_inner()
    }
    if signed {
        encode::<colortype::GrayI16>(
            &values.iter().map(|v| *v as i16).collect::<Vec<_>>(),
            nodata,
            scale,
            raster_type,
            [width, height],
        )
    } else {
        encode::<colortype::Gray16>(
            &values.iter().map(|v| *v as u16).collect::<Vec<_>>(),
            nodata,
            scale,
            raster_type,
            [width, height],
        )
    }
}

#[test]
fn pixel_is_point_centres_are_converted_to_outer_edges_before_map_sampling() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = record(&root, false, &fixture_with_type(false, "0", 30.0, 2));
    let metadata = inspect(&root, &job, 160).unwrap();
    assert_eq!(metadata.bounds, [499985.0, 4199955.0, 500075.0, 4200015.0]);
    assert_eq!(
        metadata.reflectance.unwrap().pixel_interpretation,
        "PixelIsPoint"
    );
    let pixel = sample(&root, &job, 500014.99, 4200000.0).unwrap();
    assert_eq!(pixel.pixel, [0, 0]);
    assert_eq!(pixel.center, [500000.0, 4200000.0]);
    assert_eq!(
        sample(&root, &job, 500015.0, 4200000.0).unwrap().value,
        1000.0
    );
    let invalid = record(&root, false, &fixture_with_type(false, "0", 30.0, 3));
    assert!(inspect(&root, &invalid, 160).is_err());
}

pub(crate) fn record(root: &Path, signed: bool, bytes: &[u8]) -> Job {
    let mut job = crate::raster::tests::record(root, bytes);
    job.asset_key = "red".into();
    if signed {
        job.item_id = "HLS.L30.T10SEG.2025179T184546.v2.0".into();
        job.href = format!(
            "https://{}/lp-prod-protected/HLSL30.020/{}/{}.B04.tif",
            providers::nasa::HOST,
            job.item_id,
            job.item_id
        );
    } else {
        job.item_id = "LC09_L2SP_044034_20250628_02_T1".into();
        let product = "LC09_L2SP_044034_20250628_20250629_02_T1";
        job.href = format!("https://{}/landsat-c2/level-2/standard/oli-tirs/2025/044/034/{product}/{product}_SR_B4.TIF", providers::LANDSAT_HOST);
    }
    job
}

#[test]
fn original_signed_and_unsigned_values_calibration_and_display_remain_distinct() {
    for signed in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let job = record(
            &root,
            signed,
            &fixture(signed, if signed { "-9999" } else { "0" }, 30.0),
        );
        let metadata = inspect(&root, &job, 768).unwrap();
        assert_eq!(
            (metadata.width, metadata.height, metadata.band_count),
            (3, 2, 1)
        );
        assert_eq!(metadata.bounds, [500000.0, 4199940.0, 500090.0, 4200000.0]);
        assert_eq!(metadata.data_type, if signed { "Int16" } else { "UInt16" });
        assert!(metadata.classes.is_empty());
        let display = metadata.reflectance.as_ref().unwrap();
        assert_eq!((display.sample_count, display.valid_sample_count), (6, 5));
        let nodata = sample(&root, &job, 500015.0, 4199985.0).unwrap();
        assert!(nodata.is_no_data);
        assert!(nodata.reflectance.is_none());
        let original = sample(&root, &job, 500045.0, 4199985.0).unwrap();
        assert_eq!(original.value, if signed { -100.0 } else { 1000.0 });
        assert!(
            (original.reflectance.unwrap() - if signed { -0.01 } else { -0.1725 }).abs() < 1e-12
        );
        let high = sample(&root, &job, 500075.0, 4199955.0).unwrap();
        assert_eq!(high.value, if signed { 32767.0 } else { 65535.0 });
        assert!(high.reflectance.unwrap() > 1.0); // Never clamp science values for display.
        assert!(sample(&root, &job, 500090.0, 4199985.0).is_err());
        assert!(sample(&root, &job, 500000.0, 4199940.0).is_err());
        let png = STANDARD
            .decode(metadata.preview_data_url.split(',').nth(1).unwrap())
            .unwrap();
        let mut reader = png::Decoder::new(Cursor::new(png)).read_info().unwrap();
        let mut rgba = vec![0; reader.output_buffer_size().unwrap()];
        reader.next_frame(&mut rgba).unwrap();
        assert_eq!(rgba[3], 0);
        assert!(rgba[4..]
            .chunks_exact(4)
            .all(|p| p[0] == p[1] && p[1] == p[2] && p[3] == 255));
    }
}

#[test]
fn incompatible_product_metadata_wrong_samples_hash_and_paths_are_rejected() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    for (signed, nodata, scale) in [
        (true, "0", 30.0),
        (false, "-9999", 30.0),
        (false, "0", 20.0),
    ] {
        let job = record(&root, signed, &fixture(signed, nodata, scale));
        assert!(inspect(&root, &job, 160).is_err());
    }
    let mut job = record(&root, false, &fixture(true, "-9999", 30.0));
    assert!(inspect(&root, &job, 160).unwrap_err().contains("16-bit"));
    job = record(&root, false, &fixture(false, "0", 30.0));
    job.sha256 = Some("0".repeat(64));
    assert!(sample(&root, &job, 500015.0, 4199985.0)
        .unwrap_err()
        .contains("SHA-256"));
    job = record(&root, false, &fixture(false, "0", 30.0));
    job.asset_key = "green".into();
    assert!(inspect(&root, &job, 160).unwrap_err().contains("channel"));
    job.asset_key = "red".into();
    job.href = job.href.replace(providers::LANDSAT_HOST, "unknown.example");
    assert!(inspect(&root, &job, 160).is_err());
    job = record(&root, false, &fixture(false, "0", 30.0));
    let outside = root.join("outside.tif");
    std::fs::copy(job.output_path.as_ref().unwrap(), &outside).unwrap();
    job.output_path = Some(outside.to_string_lossy().into_owned());
    assert!(inspect(&root, &job, 160).is_err());
}

#[tokio::test]
async fn reflectance_thumbnails_survive_restart_in_the_persistent_cache() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let job = record(manager.storage_root(), true, &fixture(true, "-9999", 30.0));
    let records = BTreeMap::from([(job.id.clone(), job.clone())]);
    manager.inner.store.lock().await.jobs = records.clone();
    manager.persist(&records).await.unwrap();
    let first = manager.file_thumbnail(&job.id).await.unwrap();
    assert_eq!((first.width, first.height), (3, 2));
    drop(manager);
    let restarted = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(
        restarted.file_thumbnail(&job.id).await.unwrap().data_url,
        first.data_url
    );
    assert!(restarted
        .inspect_raster(&job.id)
        .await
        .unwrap()
        .reflectance
        .is_some());
    assert_eq!(
        restarted
            .sample_raster(&job.id, 500045.0, 4199985.0)
            .await
            .unwrap()
            .value,
        -100.0
    );
}
