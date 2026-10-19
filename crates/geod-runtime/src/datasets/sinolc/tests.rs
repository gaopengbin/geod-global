use super::*;
use serde_json::json;

fn request() -> ResolveRequest {
    ResolveRequest {
        dataset_id: "sinolc-1".into(),
        bounds: [121.4, 31.1, 121.6, 31.3],
        year: None,
        month: None,
        country: None,
        layer: Some("classes".into()),
        region: None,
        season: None,
    }
}
fn metadata() -> crate::stac::raster::GenericRasterInspection {
    let t = &INDEX.member;
    crate::stac::raster::GenericRasterInspection {
        spectral_encoding: None,
        sample_encoding: None,
        categorical_encoding: None,
        archive_original: None,
        job_id: "test".into(),
        sha256: t.sha256.clone(),
        width: t.width,
        height: t.height,
        storage_layout: "tile".into(),
        bands: vec![crate::stac::raster::Band {
            index: 1,
            data_type: "UInt8".into(),
            nodata: json!(0),
        }],
        crs: Some("EPSG:4326".into()),
        horizontal_crs: Some("EPSG:4326".into()),
        vertical_crs: None,
        source_model_pixel_scale: Some(t.pixel_scale),
        source_model_tiepoint: Some(t.tiepoint),
        transform: Some([
            t.pixel_scale[0],
            0.,
            t.bounds[0],
            0.,
            -t.pixel_scale[1],
            t.bounds[3],
        ]),
        bounds: Some(t.bounds),
        pixel_interpretation: Some("area".into()),
        preview_data_url: None,
        preview_width: None,
        preview_height: None,
        display_band: 1,
        display_range: None,
        limitations: vec![],
    }
}

#[test]
fn fixed_original_scope_preserves_palette_nodata_and_mixed_observation_dates() {
    checked_index().unwrap();
    let d = definitions().remove(0);
    assert_eq!(d.id, "sinolc-1");
    assert!(d.years.is_empty() && d.default_year.is_none());
    assert_eq!(d.layers[0].nodata, Some(0.));
    assert_eq!(d.layers[0].unit, "class code");
    assert!(d.warnings.iter().any(|v| v.contains("2019, 2020 and 2021")));
    assert!(d.warnings.iter().any(|v| v.contains("exceeds ordinary")));
    assert_eq!(INDEX.classes.len(), 11);
    assert!(!INDEX
        .classes
        .iter()
        .any(|v| v.code == 11 || v.code == 250 || v.code == 255));
    assert_eq!(resolve(&request()).unwrap().len(), 1);
}

#[test]
fn original_selection_rejects_unsupported_filters_outside_area_and_wrong_family() {
    for change in 0..9 {
        let mut r = request();
        match change {
            0 => r.year = Some(2021),
            1 => r.month = Some(1),
            2 => r.country = Some("CHN".into()),
            3 => r.region = Some("nationwide".into()),
            4 => r.season = Some("summer".into()),
            5 => r.layer = Some("image-capture-time".into()),
            6 => r.layer = None,
            7 => r.bounds = [116., 39., 117., 40.],
            _ => r.dataset_id = "sinobf-1".into(),
        };
        assert!(resolve(&r).is_err(), "invalid selection {change}");
    }
}

#[test]
fn archive_recipe_requires_the_whole_original_and_exact_provenance() {
    let plan = crate::datasets::resolve(request()).unwrap();
    let p = crate::datasets::provenance(&plan, &plan.sources[0]);
    let a = archive_selection(&p, &INDEX.archive_url).unwrap();
    assert_eq!(a.archive_bytes, 365818022);
    assert_eq!(a.member_bytes, 427679242);
    assert_eq!(a.member_sha256, ORIGINAL_MEMBER_SHA256);
    assert!(archive_selection(&p, &INDEX.archive_url.replace("7709370", "8214467")).is_err());
    for change in 0..6 {
        let mut forged = p.clone();
        match change {
            0 => forged.source_id = "other-city".into(),
            1 => forged.source_bounds = None,
            2 => forged.layer.nodata = Some(255.),
            3 => forged.dataset.license = "public domain".into(),
            4 => forged.schema_version = 3,
            _ => forged.declaration_kind = "user-source".into(),
        };
        assert!(archive_selection(&forged, &INDEX.archive_url).is_err());
    }
}

#[test]
fn reviewed_large_grid_permission_is_sha_bound_and_not_a_generic_dimension_override() {
    assert!(large_grid_identity(
        ORIGINAL_MEMBER_SHA256,
        132000,
        114000,
        true
    ));
    assert!(!large_grid_identity(&"0".repeat(64), 132000, 114000, true));
    assert!(!large_grid_identity(
        ORIGINAL_MEMBER_SHA256,
        132001,
        114000,
        true
    ));
    assert!(!large_grid_identity(
        ORIGINAL_MEMBER_SHA256,
        132000,
        114001,
        true
    ));
    assert!(!large_grid_identity(
        ORIGINAL_MEMBER_SHA256,
        132000,
        114000,
        false
    ));
    assert_eq!(CHUNK_COUNT, 230136);
    assert_eq!(EXPECTED_TILE_DIMS, [256, 256]);
    assert!(budget_preview_profile(&metadata()));
    for change in 0..16 {
        let mut m = metadata();
        match change {
            0 => m.sha256 = "0".repeat(64),
            1 => m.width -= 1,
            2 => m.height -= 1,
            3 => m.storage_layout = "strip".into(),
            4 => m.crs = None,
            5 => m.horizontal_crs = None,
            6 => m.vertical_crs = Some("EPSG:5773".into()),
            7 => m.pixel_interpretation = Some("point".into()),
            8 => m.bands[0].nodata = json!(255),
            9 => m.bands[0].data_type = "UInt16".into(),
            10 => m.bands.push(m.bands[0].clone()),
            11 => m.source_model_pixel_scale.as_mut().unwrap()[0] *= 2.,
            12 => m.source_model_tiepoint.as_mut().unwrap()[3] += 0.1,
            13 => m.transform = None,
            14 => m.bounds.as_mut().unwrap()[2] += 0.1,
            _ => m.bands[0].index = 2,
        };
        assert!(!budget_preview_profile(&m), "profile change {change}");
        assert!(categorical_encoding(&m).is_none());
        assert!(categorical_color(&m, 9.).is_none());
    }
}

#[test]
fn exact_palette_preserves_known_and_unlisted_class_codes_without_percent_or_rgb_claim() {
    let m = metadata();
    assert_eq!(categorical_color(&m, 0.), Some([0, 0, 0, 0]));
    assert_eq!(categorical_color(&m, 9.), Some([0, 100, 200, 255]));
    assert_eq!(categorical_color(&m, 11.), Some([0, 205, 115, 255]));
    assert_eq!(categorical_color(&m, 250.), Some([0, 0, 0, 255]));
    assert_eq!(categorical_color(&m, 255.), Some([0, 0, 0, 255]));
    for v in [-1., 256., 1.5, f64::NAN] {
        assert!(categorical_color(&m, v).is_none());
    }
    let encoding = categorical_encoding(&m).unwrap();
    assert_eq!(encoding["unlistedCodes"], json!([11, 250, 255]));
    assert_eq!(original_color_map().len(), 768);
}

#[test]
#[ignore = "requires SINOLC_REVIEWED_ORIGINAL_DIR containing complete publisher ZIP and unchanged TIFF; no CI download"]
fn exact_real_original_supports_bounded_rust_tiles_and_every_independent_gdal_pixel() {
    use sha2::{Digest, Sha256};
    use std::{
        fs::File,
        io::{BufReader, Read},
        path::PathBuf,
        time::Instant,
    };
    use tiff::{
        decoder::{Decoder, DecodingResult, Limits},
        tags::Tag,
    };
    let dir = PathBuf::from(std::env::var("SINOLC_REVIEWED_ORIGINAL_DIR").unwrap());
    for (name, expected_bytes, expected_sha) in [
        (
            "East_Shanghai.zip",
            INDEX.archive_bytes,
            &INDEX.archive_sha256,
        ),
        (
            "East_Shanghai.tif",
            INDEX.member.bytes,
            &INDEX.member.sha256,
        ),
    ] {
        let mut source = File::open(dir.join(name)).unwrap();
        let mut hash = Sha256::new();
        let mut bytes = 0u64;
        let mut block = [0u8; 65536];
        loop {
            let n = source.read(&mut block).unwrap();
            if n == 0 {
                break;
            }
            bytes += n as u64;
            hash.update(&block[..n]);
        }
        assert_eq!(bytes, expected_bytes);
        assert_eq!(format!("{:x}", hash.finalize()), *expected_sha);
    }
    let started = Instant::now();
    let mut limits = Limits::default();
    limits.decoding_buffer_size = 128 * 1024 * 1024;
    limits.intermediate_buffer_size = 32 * 1024 * 1024;
    limits.ifd_value_size = 1024 * 1024;
    let mut d = Decoder::new(BufReader::new(
        File::open(dir.join("East_Shanghai.tif")).unwrap(),
    ))
    .unwrap()
    .with_limits(limits);
    assert_eq!(d.dimensions().unwrap(), (132000, 114000));
    assert_eq!(d.tile_count().unwrap(), CHUNK_COUNT);
    assert_eq!(d.chunk_dimensions(), (256, 256));
    assert!(!d.more_images());
    assert_eq!(d.get_tag_unsigned::<u16>(Tag::Compression).unwrap(), 5);
    assert_eq!(
        d.get_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
            .unwrap(),
        1
    );
    assert_eq!(
        d.get_tag_unsigned::<u16>(Tag::PlanarConfiguration).unwrap(),
        2
    );
    assert_eq!(
        d.find_tag_unsigned::<u16>(Tag::Predictor)
            .unwrap()
            .unwrap_or(1),
        1
    );
    assert_eq!(d.get_tag_u16_vec(Tag::BitsPerSample).unwrap(), [8]);
    assert_eq!(
        d.get_tag_ascii_string(Tag::GdalNodata)
            .unwrap()
            .trim_end_matches('\0'),
        "0"
    );
    assert_eq!(
        d.get_tag_u16_vec(Tag::ColorMap).unwrap(),
        original_color_map()
    );
    assert_eq!(
        d.get_tag_f64_vec(Tag::ModelPixelScaleTag).unwrap(),
        INDEX.member.pixel_scale
    );
    assert_eq!(
        d.get_tag_f64_vec(Tag::ModelTiepointTag).unwrap(),
        INDEX.member.tiepoint
    );
    let geo_keys = d.get_tag_u16_vec(Tag::GeoKeyDirectoryTag).unwrap();
    assert!(geo_keys[4..]
        .chunks_exact(4)
        .any(|v| v == [2048, 0, 1, 4326]));
    assert!(geo_keys[4..].chunks_exact(4).any(|v| v == [1025, 0, 1, 1]));
    let columns = 132000u32.div_ceil(256);
    let mut charged = 0usize;
    for p in INDEX.member.pixels.iter().chain(&INDEX.member.edge_pixels) {
        let index = p.row / 256 * columns + p.column / 256;
        let (w, h) = d.chunk_data_dimensions(index);
        charged += w as usize * h as usize * 8;
        assert!(charged <= 512 * 1024 * 1024);
        let DecodingResult::U8(values) = d.read_chunk(index).unwrap() else {
            panic!("Original class encoding changed")
        };
        assert_eq!(values.len(), w as usize * h as usize);
        let value = values[((p.row % 256) * w + p.column % 256) as usize];
        assert_eq!(vec![value], p.values);
        assert_eq!(vec![value == 0], p.no_data);
    }
    assert!(started.elapsed().as_secs() < 30);
    for p in &INDEX.member.edge_pixels {
        let (w, h) = d.chunk_data_dimensions(p.row / 256 * columns + p.column / 256);
        assert!(
            w == 160 || h == 80,
            "Independent boundary reference must decode a partial original tile"
        );
    }
    println!("SinoLC exact original: 144 independent raw references +8 partial-edge references, all tiles <=65536 B, charged {charged} B, elapsed {:?}, no whole-image allocation",started.elapsed());
    assert!(
        d.read_image().is_err(),
        "Whole-image allocation must remain rejected by the unchanged 128 MiB decode cap"
    );
}
