use super::*;
use tiff::encoder::{colortype, TiffEncoder};

const ITEM: &str = "MYD09A1.A2025177.h08v05.061.2025189031924";

fn fixture(key: &str, fill: &str, shift: f64) -> Vec<u8> {
    let tile = std::f64::consts::PI * providers::modis::RADIUS / 18.0;
    let mut bytes = Cursor::new(Vec::new());
    let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
    macro_rules! encode {
        ($ty:ty, $sample:ty) => {{
            let mut image = encoder.new_image::<$ty>(2400, 2400).unwrap();
            reflectance::modis::write_crs(image.encoder()).unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::ModelPixelScaleTag,
                    &[providers::modis::PIXEL, providers::modis::PIXEL, 0.0][..],
                )
                .unwrap();
            image
                .encoder()
                .write_tag(
                    Tag::ModelTiepointTag,
                    &[0.0, 0.0, 0.0, -10.0 * tile + shift, 4.0 * tile, 0.0][..],
                )
                .unwrap();
            image.encoder().write_tag(Tag::GdalNodata, fill).unwrap();
            let mut values = vec![0 as $sample; 2400 * 2400];
            values[0] = 0;
            values[1] = 1;
            values[2] = 2;
            values[3] = 3;
            values[4] = if key == "modis_qc" {
                0x8000_0000u32 as $sample
            } else {
                0x8000 as $sample
            };
            values[2399] = <$sample>::MAX;
            values[2400 * 2400 - 1] = <$sample>::MAX;
            image.write_data(&values).unwrap();
        }};
    }
    if key == "modis_qc" {
        encode!(colortype::Gray32, u32);
    } else {
        encode!(colortype::Gray16, u16);
    }
    bytes.into_inner()
}
fn record(root: &Path, key: &str, bytes: &[u8]) -> Job {
    let mut job = crate::raster::tests::record(root, bytes);
    job.asset_key = key.into();
    job.item_id = ITEM.into();
    job.href = format!(
        "https://{}/modis-061-cogs/MYD09A1/08/05/2025177/{ITEM}{}",
        providers::modis::HOST,
        providers::modis::suffix(key).unwrap()
    );
    job
}
#[test]
fn quality_fields_retain_all_unsigned_bits_and_do_not_label_unknown_codes_as_good() {
    let raw = 0xC000_0000u32 | (7 << 2) | (8 << 6) | (1 << 18) | 2;
    let pixel = decode("modis_qc", raw).unwrap();
    assert_eq!(pixel.fields.len(), 10);
    assert_eq!(pixel.fields[0].value, 2);
    assert_eq!(pixel.fields[1].value, 7);
    assert_eq!(pixel.fields[2].value, 8);
    assert_eq!(pixel.fields[5].value, 1);
    assert!(!pixel.fields[5].defined);
    assert_eq!(pixel.fields[8].value, 1);
    assert_eq!(pixel.fields[9].value, 1);
    assert_eq!(pixel.binary.len(), 32);
    assert!(pixel.hex.starts_with("0xC"));
    assert!(decode("modis_qc", u32::MAX).unwrap().fields.is_empty());
}
#[test]
fn state_uses_its_own_cloud_aerosol_salt_pan_and_snow_layout() {
    let pixel = decode("modis_state", 0xFFFF - 4).unwrap();
    assert_eq!(pixel.fields.len(), 11);
    let values: Vec<_> = pixel.fields.iter().map(|f| f.value).collect();
    assert_eq!(values, [3, 0, 7, 3, 3, 1, 1, 1, 1, 1, 1]);
    assert_eq!(pixel.fields[0].label, "Unset; product assumes clear");
    assert_eq!(pixel.fields[9].name, "Salt pan");
    assert_eq!(pixel.fields[10].start_bit, 15);
    assert!(decode("modis_state", 65535).unwrap().fields.is_empty());
}
#[test]
fn unsigned_quality_files_count_original_samples_and_sample_exact_values() {
    for (key, fill) in [("modis_qc", "4294967295"), ("modis_state", "65535")] {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().canonicalize().unwrap();
        let job = record(&root, key, &fixture(key, fill, 0.0));
        let metadata = inspect(&root, &job, 160).unwrap();
        let quality = metadata.quality.as_ref().unwrap();
        assert_eq!(quality.sample_count, 5_760_000);
        assert_eq!(quality.valid_sample_count, 5_759_998);
        assert_eq!(
            metadata.classes.iter().map(|c| c.count).sum::<u64>(),
            5_759_998
        );
        assert_eq!(
            metadata.classes.iter().map(|c| c.count).collect::<Vec<_>>(),
            [5_759_995, 1, 1, 1]
        );
        assert!(metadata.reflectance.is_none());
        let x = metadata.bounds[0] + 4.5 * providers::modis::PIXEL;
        let y = metadata.bounds[3] - 0.5 * providers::modis::PIXEL;
        let pixel = sample(&root, &job, x, y).unwrap();
        assert_eq!(
            pixel.value,
            if key == "modis_qc" {
                2147483648.0
            } else {
                32768.0
            }
        );
        assert_eq!(pixel.pixel, [4, 0]);
        assert!(!pixel.is_no_data);
        assert!(pixel.reflectance.is_none());
        let flags = pixel.quality.unwrap();
        assert_eq!(
            flags.hex,
            if key == "modis_qc" {
                "0x80000000"
            } else {
                "0x8000"
            }
        );
        assert_eq!(flags.fields.last().unwrap().value, 1);
        let missing = sample(
            &root,
            &job,
            metadata.bounds[2] - 0.5 * providers::modis::PIXEL,
            y,
        )
        .unwrap();
        assert!(missing.is_no_data);
        assert_eq!(missing.value, metadata.nodata.unwrap());
        assert!(missing.quality.unwrap().fields.is_empty());
        let mut changed = job.clone();
        changed.sha256 = Some("0".repeat(64));
        assert!(inspect(&root, &changed, 160).is_err());
        changed = job.clone();
        changed.item_id = ITEM.replace("h08", "h09");
        assert!(inspect(&root, &changed, 160).is_err());
        assert!(sample(&root, &job, metadata.bounds[2], y).is_err());
        assert!(sample(&root, &job, x, metadata.bounds[1]).is_err());
    }
}
#[test]
fn original_quality_grid_fill_and_channel_type_are_not_inferred_or_silently_repaired() {
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().canonicalize().unwrap();
    for (key, fill, shift) in [
        ("modis_qc", "0", 0.0),
        ("modis_state", "65535", 463.312716527778),
    ] {
        let job = record(&root, key, &fixture(key, fill, shift));
        assert!(inspect(&root, &job, 160).is_err());
    }
    let bytes = fixture("modis_state", "65535", 0.0);
    let wrong_type = record(&root, "modis_qc", &bytes);
    assert!(inspect(&root, &wrong_type, 160).is_err());
}
