use super::*;
use std::io::Cursor;
use tiff::encoder::{colortype, TiffEncoder};
const ITEM: &str = "LC09_L2SP_044034_20250628_02_T1";
fn fixture(nodata: Option<&str>, spacing: f64) -> Vec<u8> {
    fixture_grid(nodata, spacing, false)
}
fn fixture_grid(nodata: Option<&str>, spacing: f64, point: bool) -> Vec<u8> {
    let mut out = Cursor::new(Vec::new());
    let mut encoder = TiffEncoder::new(&mut out).unwrap();
    let mut image = encoder.new_image::<colortype::Gray16>(10, 2).unwrap();
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
                if point { 2 } else { 1 },
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
        .write_tag(Tag::ModelPixelScaleTag, &[spacing, spacing, 0.0][..])
        .unwrap();
    image
        .encoder()
        .write_tag(
            Tag::ModelTiepointTag,
            &[0.0, 0.0, 0.0, 500000.0, 4200000.0, 0.0][..],
        )
        .unwrap();
    if let Some(v) = nodata {
        image.encoder().write_tag(Tag::GdalNodata, v).unwrap();
    }
    image
        .write_data(&[
            0u16, 1, 65, 65535, 64, 8, 2, 4, 16, 32, 128, 256, 512, 1024, 2048, 4096, 8192, 16384,
            32768, 14,
        ])
        .unwrap();
    out.into_inner()
}
fn record(root: &Path, key: &str, bytes: &[u8]) -> Job {
    let mut job = crate::raster::tests::record(root, bytes);
    job.item_id = ITEM.into();
    job.asset_key = key.into();
    let product = "LC09_L2SP_044034_20250628_20250629_02_T1";
    job.href = format!(
        "https://{}/landsat-c2/level-2/standard/oli-tirs/2025/044/034/{product}/{product}_{}.TIF",
        providers::LANDSAT_HOST,
        key.to_uppercase()
    );
    job
}
#[test]
fn landsat_quality_preserves_bit_fifteen_and_reserved_confidence_codes() {
    let q = decode("qa_pixel", 0x8000 | 64).unwrap();
    assert_eq!(q.hex, "0x8040");
    assert_eq!(q.fields[11].value, 2);
    assert_eq!(q.fields[11].label, "Reserved code");
    assert!(!q.fields[11].defined);
    assert_eq!(q.fields[6].value, 1);
    let r = decode("qa_radsat", u16::MAX).unwrap();
    assert_eq!(r.fields[12].value, 15);
    assert!(!r.fields[12].defined);
    assert_eq!(class("qa_radsat", 8), 1);
    assert_eq!(class("qa_radsat", 1), 2);
    assert_eq!(class("qa_radsat", 1 << 11), 3);
    assert_eq!(class("qa_radsat", 1 << 15), 4);
}
#[test]
fn landsat_fill_is_a_bit_and_radsat_zero_is_never_nodata() {
    for (key, nodata) in [
        ("qa_pixel", Some("1")),
        ("qa_radsat", None),
        ("qa_radsat", Some("0")),
    ] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let job = record(&root, key, &fixture(nodata, 30.0));
        let m = inspect(&root, &job, 160).unwrap();
        assert_eq!(m.nodata, None);
        assert!(m.reflectance.is_none());
        assert_eq!(m.data_type, "UInt16");
        let q = m.quality.unwrap();
        assert_eq!(q.sample_count, 20);
        assert_eq!(
            q.valid_sample_count,
            if key == "qa_pixel" { 17 } else { 20 }
        );
        for f in q.flags.unwrap().fields {
            assert_eq!(f.counts.iter().sum::<u64>(), 20);
        }
        let zero = sample(&root, &job, 500015.0, 4199985.0).unwrap();
        assert_eq!(zero.value, 0.0);
        assert!(!zero.is_no_data);
        let high = sample(&root, &job, 500105.0, 4199985.0).unwrap();
        assert_eq!(high.value, 65535.0);
        assert_eq!(high.is_no_data, key == "qa_pixel");
        assert!(!high.quality.unwrap().fields.is_empty());
        let fill = sample(&root, &job, 500075.0, 4199985.0).unwrap();
        assert_eq!(fill.is_no_data, key == "qa_pixel");
        assert!(sample(&root, &job, 500300.0, 4199985.0).is_err());
    }
}
#[test]
fn landsat_quality_rejects_wrong_identity_grid_tag_and_changed_source() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = record(&root, "qa_pixel", &fixture(Some("1"), 30.0));
    let mut bad = job.clone();
    bad.item_id = ITEM.replace("20250628", "20250629");
    assert!(inspect(&root, &bad, 160).is_err());
    bad = job.clone();
    bad.asset_key = "qa_radsat".into();
    assert!(inspect(&root, &bad, 160).is_err());
    bad = record(&root, "qa_pixel", &fixture(Some("0"), 30.0));
    assert!(inspect(&root, &bad, 160).is_err());
    bad = record(&root, "qa_pixel", &fixture(Some("1"), 20.0));
    assert!(inspect(&root, &bad, 160).is_err());
    std::fs::write(job.output_path.as_ref().unwrap(), fixture(Some("1"), 20.0)).unwrap();
    assert!(inspect(&root, &job, 160).unwrap_err().contains("SHA-256"));
}
#[test]
fn landsat_point_grid_preserves_centres_and_converts_only_outer_bounds() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    let job = record(&root, "qa_pixel", &fixture_grid(None, 30.0, true));
    let m = inspect(&root, &job, 160).unwrap();
    assert_eq!(m.bounds, [499985.0, 4199955.0, 500285.0, 4200015.0]);
    assert_eq!(m.quality.unwrap().pixel_interpretation, "PixelIsPoint");
    let p = sample(&root, &job, 500000.0, 4200000.0).unwrap();
    assert_eq!(p.pixel, [0, 0]);
    assert_eq!(p.center, [500000.0, 4200000.0]);
    assert!(sample(&root, &job, 500285.0, 4200000.0).is_err());
}
