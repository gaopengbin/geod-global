use super::*;

fn request() -> ResolveRequest {
    ResolveRequest {
        dataset_id: "soilgrids".into(),
        bounds: [-66., -23., -65., -22.],
        year: None,
        month: None,
        country: None,
        layer: Some("most-probable".into()),
        region: None,
        season: None,
    }
}

#[test]
fn corrected_2017_identity_categorical_zero_and_raw_scores_remain_explicit() {
    let d = definitions().remove(0);
    assert_eq!(d.id, "soilgrids");
    assert!(d.title.contains("2017 corrected"));
    assert!(
        d.years.is_empty() && d.months.is_empty() && d.regions.is_empty() && d.seasons.is_empty()
    );
    assert_eq!(d.default_layer.as_deref(), Some("most-probable"));
    assert!(d.license.contains("ODbL") && !d.license.contains("CC BY"));
    assert!(d.layers[0]
        .value_interpretation
        .contains("Class 0 is valid Acrisols"));
    assert_eq!(d.layers[1].unit, "raw score");
    assert!(d.layers[1]
        .value_interpretation
        .contains("without dividing by 100"));
    for l in d.layers {
        assert_eq!(
            (l.scale, l.offset, l.nodata),
            (Some(1.), Some(0.), Some(255.))
        );
    }
}

#[test]
fn exact_originals_are_selected_by_their_actual_layer_and_footprint() {
    for t in &INDEX.originals {
        let mut r = request();
        r.layer = Some(t.layer.clone());
        r.bounds = [
            (t.bounds[0] + t.bounds[2]) * 0.5,
            -23.,
            (t.bounds[0] + t.bounds[2]) * 0.5 + 0.1,
            -22.,
        ];
        let s = resolve(&r).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].id, t.id);
        assert_eq!(s[0].bounds, Some(t.bounds));
        assert_eq!(s[0].stac.as_ref().unwrap().url, t.url);
        assert!(
            s[0].search.is_none() && s[0].start_datetime.is_none() && s[0].end_datetime.is_none()
        );
    }
    let mut r = request();
    r.bounds = [-61., -23., -60., -22.];
    assert_eq!(resolve(&r).unwrap().len(), 2);
}

#[test]
fn unsupported_product_filters_and_outside_areas_do_not_fall_back() {
    for n in 0..9 {
        let mut r = request();
        match n {
            0 => r.year = Some(2017),
            1 => r.month = Some(1),
            2 => r.country = Some("BRA".into()),
            3 => r.region = Some("global".into()),
            4 => r.season = Some("summer".into()),
            5 => r.layer = Some("phh2o".into()),
            6 => r.layer = None,
            7 => r.bounds = [116., 39., 117., 40.],
            _ => r.dataset_id = "soilgrids-2020".into(),
        };
        assert!(resolve(&r).is_err(), "Invalid selection {n}");
    }
}

#[test]
fn source_receipt_accepts_only_the_complete_pinned_original() {
    let plan = super::super::resolve(request()).unwrap();
    let s = &plan.sources[0];
    let p = super::super::provenance(&plan, s);
    let t = INDEX.originals.iter().find(|t| t.id == s.id).unwrap();
    validate_original(&p, &t.url, t.bytes, &t.sha256).unwrap();
    assert!(validate_original(&p, &t.url, t.bytes - 1, &t.sha256).is_err());
    assert!(validate_original(&p, &t.url, t.bytes, &"0".repeat(64)).is_err());
    assert!(validate_original(
        &p,
        &t.url.replace("MostProbable", "Acrisols"),
        t.bytes,
        &t.sha256
    )
    .is_err());
    let mut forged = p.clone();
    forged.source_bounds = Some([-180., -90., 180., 90.]);
    assert!(validate_original(&forged, &t.url, t.bytes, &t.sha256).is_err());
    let mut forged = p;
    forged.dataset.license = "CC BY 4.0".into();
    assert!(validate_original(&forged, &t.url, t.bytes, &t.sha256).is_err());
}

#[test]
#[ignore = "requires GEOD_SOILGRIDS_ORIGINAL_DIR containing four complete official originals"]
fn whole_official_tiffs_match_full_hash_georeferencing_and_every_reference_pixel() {
    use sha2::{Digest, Sha256};
    use std::{io::Cursor, path::PathBuf};
    use tiff::{
        decoder::{Decoder, DecodingResult},
        tags::Tag,
    };
    let dir = PathBuf::from(std::env::var("GEOD_SOILGRIDS_ORIGINAL_DIR").unwrap());
    for t in &INDEX.originals {
        let raw = std::fs::read(dir.join(format!("{}-{}.tif", t.publisher_layer, t.tile))).unwrap();
        assert_eq!(raw.len() as u64, t.bytes);
        assert_eq!(format!("{:x}", Sha256::digest(&raw)), t.sha256);
        let mut d = Decoder::new(Cursor::new(raw)).unwrap();
        assert_eq!(d.dimensions().unwrap(), (t.width, t.height));
        assert_eq!(d.get_tag_u16_vec(Tag::BitsPerSample).unwrap(), vec![8]);
        assert_eq!(
            d.get_tag_unsigned::<u32>(Tag::RowsPerStrip).unwrap(),
            t.rows_per_strip
        );
        assert_eq!(
            d.get_tag_unsigned::<u16>(Tag::Compression).unwrap(),
            t.compression
        );
        assert_eq!(
            d.get_tag_f64_vec(Tag::ModelPixelScaleTag).unwrap(),
            t.pixel_scale
        );
        assert_eq!(
            d.get_tag_f64_vec(Tag::ModelTiepointTag).unwrap(),
            t.tiepoint
        );
        let keys = d.get_tag_u16_vec(Tag::GeoKeyDirectoryTag).unwrap();
        assert!(keys[4..].chunks_exact(4).any(|v| v == [2048, 0, 1, 4326]));
        assert!(keys[4..].chunks_exact(4).any(|v| v == [1025, 0, 1, 1]));
        assert_eq!(
            d.get_tag_ascii_string(Tag::GdalNodata)
                .unwrap()
                .trim_end_matches('\0'),
            "255"
        );
        let DecodingResult::U8(values) = d.read_image().unwrap() else {
            panic!("Original WRB sample type changed")
        };
        assert_eq!(values.len(), (t.width * t.height) as usize);
        for p in &t.samples {
            let value = values[(p.row * t.width + p.column) as usize];
            assert_eq!(vec![value], p.values);
            assert_eq!(vec![value == 255], p.no_data);
        }
        assert_eq!(
            values.iter().filter(|v| **v == 0).count() as u64,
            t.valid_zero_count
        );
        assert_eq!(
            values.iter().filter(|v| **v == 255).count() as u64,
            t.no_data_count
        );
        let valid = values
            .iter()
            .copied()
            .filter(|v| *v != 255)
            .collect::<Vec<_>>();
        assert_eq!(*valid.iter().min().unwrap(), t.minimum);
        assert_eq!(*valid.iter().max().unwrap(), t.maximum);
        if t.layer == "most-probable" {
            assert!(valid.iter().all(|v| *v < 30));
        }
        println!(
            "{}: {} complete bytes, centre {}, valid zero {}, NoData {}, full SHA {}",
            t.id,
            t.bytes,
            values[(2048 * t.width + 2048) as usize],
            t.valid_zero_count,
            t.no_data_count,
            t.sha256
        );
    }
}
