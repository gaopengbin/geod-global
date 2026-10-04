use super::*;
use tiff::encoder::{colortype, TiffEncoder};

fn file(values: &[f32]) -> Vec<u8> {
    let mut output = Cursor::new(Vec::new());
    {
        let mut e = TiffEncoder::new(&mut output).unwrap();
        let mut image = e.new_image::<colortype::Gray32Float>(2, 2).unwrap();
        let keys = [
            1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, 32610, 3076, 0, 1, 9001,
        ];
        image
            .encoder()
            .write_tag(Tag::GeoKeyDirectoryTag, &keys[..])
            .unwrap();
        image
            .encoder()
            .write_tag(Tag::ModelPixelScaleTag, &[10.0, 10.0, 0.0][..])
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::ModelTiepointTag,
                &[0.0, 0.0, 0.0, 500000.0, 4200000.0, 0.0][..],
            )
            .unwrap();
        image
            .encoder()
            .write_tag(Tag::GdalNodata, "-32768")
            .unwrap();
        image.write_data(values).unwrap();
    }
    output.into_inner()
}
fn job(root: &Path, values: &[f32]) -> Job {
    let mut job = super::super::tests::record(root, &file(values));
    job.asset_key = "vv".into();
    job.item_id = "S1C_IW_GRDH_1SDV_20250630T140654_20250630T140719_003013_rtc".into();
    job.href="https://sentinel1euwestrtc.blob.core.windows.net/sentinel1-grd-rtc/GRD/2025/6/30/IW/DV/S1C_IW_GRDH_1SDV_20250630T140654_20250630T140719_003013_00622B_A30F/measurement/iw-vv.rtc.tiff".into();
    job
}
#[test]
fn original_float_values_zero_and_nodata_keep_their_distinct_meaning() {
    let directory = tempfile::tempdir().unwrap();
    let j = job(directory.path(), &[0.1, 0.0, -32768.0, 2.0]);
    let metadata = inspect(directory.path(), &j, 2).unwrap();
    assert_eq!(metadata.pixel_size, [10.0, 10.0]);
    assert_eq!(metadata.data_type, "Float32");
    assert_eq!(metadata.bounds, [500000.0, 4199980.0, 500020.0, 4200000.0]);
    let display = metadata.radar.unwrap();
    assert_eq!(display.unit, "linear");
    assert_eq!(display.valid_sample_count, 3);
    assert!(!display.overview);
    let p = sample(directory.path(), &j, 500005.0, 4199995.0).unwrap();
    assert_eq!(p.value, f64::from(0.1f32));
    assert_eq!(p.decibels, Some(10.0 * f64::from(0.1f32).log10()));
    let p = sample(directory.path(), &j, 500015.0, 4199995.0).unwrap();
    assert_eq!(p.value, 0.0);
    assert_eq!(p.decibels, None);
    assert!(!p.is_no_data);
    let p = sample(directory.path(), &j, 500005.0, 4199985.0).unwrap();
    assert_eq!(p.value, -32768.0);
    assert!(p.is_no_data);
    assert_eq!(p.decibels, None);
    assert!(sample(directory.path(), &j, 500020.0, 4199995.0).is_err());
    let mut switched = j.clone();
    switched.asset_key = "vh".into();
    assert!(validate_job(&switched).is_err());
    std::fs::write(
        j.output_path.as_ref().unwrap(),
        file(&[0.2, 0.0, -32768.0, 2.0]),
    )
    .unwrap();
    assert!(inspect(directory.path(), &j, 2).is_err());
}
#[test]
fn invalid_gamma0_cannot_enter_the_display_or_pixel_result() {
    for value in [-1.0, f32::NAN, f32::INFINITY] {
        let directory = tempfile::tempdir().unwrap();
        let j = job(directory.path(), &[value, 0.0, -32768.0, 2.0]);
        assert!(inspect(directory.path(), &j, 2).is_err());
        assert!(sample(directory.path(), &j, 500005.0, 4199995.0).is_err());
    }
}
