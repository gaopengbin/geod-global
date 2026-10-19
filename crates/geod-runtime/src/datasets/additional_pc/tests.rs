use super::*;
use serde_json::json;

fn request(id: &str, key: &str) -> ResolveRequest {
    serde_json::from_value(
        json!({"datasetId":id,"bounds":[116.,39.,117.,40.],"year":2020,"month":9,"layer":key}),
    )
    .unwrap()
}
fn proof(id: &str, key: &str) -> (SourceProvenance, &'static ReviewedItem) {
    let plan = super::super::resolve(request(id, key)).unwrap();
    (
        super::super::provenance(&plan, &plan.sources[0]),
        reviewed(product(id).unwrap()).unwrap(),
    )
}

#[test]
fn finite_recipe_never_substitutes_date_tile_variable_or_selected_asset() {
    for p in PRODUCTS {
        for (key, _, _) in p.layers {
            let (proof, item) = proof(p.id, key);
            validate(&proof, &item.url, Some(&item.document)).unwrap();
            self_url(&proof, &item.url, &item.document).unwrap();
            let map = transports(&proof, &item.url, &item.document).unwrap();
            assert_eq!(map.len(), 1);
            assert_eq!(map[0].asset_key, *key);
            assert_eq!(map[0].href, map[0].original_href);
            assert!(!map[0].href.contains('?'));
            assert_eq!(
                access_scope(&proof, &item.url, &item.document, key, &map[0].href).unwrap(),
                (p.account, p.container)
            );
            assert!(access_scope(
                &proof,
                &item.url,
                &item.document,
                "unselected",
                &map[0].href
            )
            .is_err());
            assert!(access_scope(
                &proof,
                &item.url,
                &item.document,
                key,
                &format!("{}?sig=counterfeit", map[0].href)
            )
            .is_err());
            let c = COLLECTIONS.iter().find(|c| c.dataset_id == p.id).unwrap();
            validate_collection(&proof, &c.url, &c.document).unwrap();
            for wrong in [
                json!({"year":2021}),
                json!({"month":8}),
                json!({"layer":"visual"}),
                json!({"country":"CHN"}),
                json!({"region":"other"}),
                json!({"season":"annual"}),
            ] {
                let mut changed = serde_json::to_value(request(p.id, key)).unwrap();
                for (k, v) in wrong.as_object().unwrap() {
                    changed[k] = v.clone();
                }
                assert!(resolve(&serde_json::from_value(changed).unwrap()).is_err());
            }
        }
    }
    let mut outside = request("modis-more", "Burn_Date");
    outside.bounds = [-120., 35., -119., 36.];
    assert!(resolve(&outside).is_err());
    assert!(!is_product("modis-11A2-061"));
    assert!(!is_product("noaa-ohc"));
}

#[test]
fn modis_aoi_intersects_actual_sinusoidal_grid_without_changing_publisher_footprint() {
    let item = reviewed(product("modis-more").unwrap()).unwrap();
    let original = item.document.clone();
    let bbox: [f64; 4] = serde_json::from_value(original["bbox"].clone()).unwrap();
    // Day-252 original cell (856,1216), independently transformed from its
    // physical centre. Also accept a large AOI whose corners miss the tile.
    for bounds in [
        [
            114.12054327659972,
            34.921249996837574,
            114.14054327659973,
            34.94124999683757,
        ],
        [-180., -90., 180., 90.],
    ] {
        for key in ["Burn_Date", "QA"] {
            let mut r = request("modis-more", key);
            r.bounds = bounds;
            let sources = resolve(&r).unwrap();
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0].bounds, Some(bbox));
            assert_eq!(sources[0].id, original["id"].as_str().unwrap());
        }
    }
    // Each lies inside the author's WGS84 bbox, but wholly outside the actual
    // physical grid: west, east at southern latitudes, and north of its edge.
    for bounds in [
        [104., 39., 104.1, 39.1],
        [130., 30., 130.1, 30.1],
        [120., 40.02, 120.1, 40.04],
    ] {
        assert!(bounds[0] > bbox[0] && bounds[2] < bbox[2]);
        assert!(bounds[1] > bbox[1] && bounds[3] < bbox[3]);
        let mut r = request("modis-more", "Burn_Date");
        r.bounds = bounds;
        assert!(resolve(&r).is_err(), "{bounds:?}");
    }
    let north = (MCD_TRANSFORM[5] / crate::providers::modis::RADIUS).to_degrees();
    let mut edge = request("modis-more", "QA");
    edge.bounds = [120., north, 121., north + 0.001];
    assert!(resolve(&edge).is_err());
    assert_eq!(item.document, original);
}

#[test]
fn publisher_and_proof_scientific_changes_cannot_acquire_transport_permission() {
    for (id, key) in [
        ("planetary-more", "sst"),
        ("modis-more", "Burn_Date"),
        ("modis-more", "QA"),
    ] {
        let (original, item) = proof(id, key);
        for pointer in [
            "/properties/start_datetime",
            "/properties/proj:transform/0",
            "/assets",
            "/collection",
            "/bbox/0",
        ] {
            let mut raw = item.document.clone();
            *raw.pointer_mut(pointer).unwrap() = json!("changed");
            assert!(
                validate(&original, &item.url, Some(&raw)).is_err(),
                "{pointer}"
            );
        }
        for field in ["href", "type", "roles", "raster:bands"] {
            let mut raw = item.document.clone();
            raw["assets"][key][field] = json!("changed");
            assert!(transports(&original, &item.url, &raw).is_err(), "{field}");
        }
        let mut changed = original.clone();
        changed.layer.nodata = Some(0.);
        assert!(validate(&changed, &item.url, Some(&item.document)).is_err());
        changed = original.clone();
        changed.layer.scale = Some(100.);
        assert!(validate(&changed, &item.url, Some(&item.document)).is_err());
        changed = original.clone();
        changed.dataset.license = "CC-BY-4.0".into();
        assert!(validate(&changed, &item.url, Some(&item.document)).is_err());
        changed = original.clone();
        changed.schema_version = 1;
        assert!(validate(&changed, &item.url, Some(&item.document)).is_err());
        let c = COLLECTIONS.iter().find(|v| v.dataset_id == id).unwrap();
        let mut raw = c.document.clone();
        raw["license"] = json!("CC-BY-4.0");
        assert!(validate_collection(&original, &c.url, &raw).is_err());
        raw = c.document.clone();
        raw["item_assets"][key]["raster:bands"] = json!([]);
        assert!(validate_collection(&original, &c.url, &raw).is_err());
    }
}

#[test]
fn independent_codes_and_temporal_semantics_remain_distinct() {
    let defs = definitions();
    let s = &defs
        .iter()
        .find(|d| d.id == "planetary-more")
        .unwrap()
        .layers[0];
    assert_eq!(
        (s.scale, s.offset, s.nodata),
        (Some(OISST_SCALE), Some(0.), Some(-999.))
    );
    let m = defs.iter().find(|d| d.id == "modis-more").unwrap();
    let burn = m.layers.iter().find(|l| l.id == "Burn_Date").unwrap();
    let qa = m.layers.iter().find(|l| l.id == "QA").unwrap();
    assert_eq!(burn.nodata, Some(-1.));
    assert_ne!(burn.nodata, Some(-2.));
    assert_ne!(burn.nodata, Some(0.));
    assert_eq!(qa.nodata, None);
    assert_eq!(qa.unit, "bit field");
    let (p, item) = proof("planetary-more", "sst");
    assert_eq!(p.layer.scale, Some(OISST_SCALE));
    assert_eq!(
        item.document["properties"]["start_datetime"],
        "2020-09-01T00:00:00Z"
    );
    assert_eq!(
        item.document["properties"]["end_datetime"],
        "2020-09-01T23:59:59Z"
    );
    assert_eq!(
        item.document["assets"]["sst"]["raster:bands"][0]["scale"],
        OISST_SCALE
    );
    let (burn_proof, item) = proof("modis-more", "Burn_Date");
    let qa_href = item.document["assets"]["QA"]["href"].as_str().unwrap();
    assert!(access_scope(&burn_proof, &item.url, &item.document, "QA", qa_href).is_err());
    assert_eq!(
        item.document["properties"]["start_datetime"],
        "2020-09-01T00:00:00Z"
    );
    assert_eq!(
        item.document["properties"]["end_datetime"],
        "2020-09-30T23:59:59Z"
    );
    assert!(item.document["properties"]["proj:epsg"].is_null());
}

#[test]
fn complete_original_reference_is_bound_to_selected_layer_and_canonical_url() {
    let cases = [
        ("planetary-more", "sst", 1_096_226, OISST_SHA256),
        ("modis-more", "Burn_Date", 112_419, BURN_DATE_SHA256),
        ("modis-more", "QA", 811_891, BURN_QA_SHA256),
    ];
    for (id, key, bytes, sha) in cases {
        let (p, item) = proof(id, key);
        let href = item.document["assets"][key]["href"].as_str().unwrap();
        let original = item.document.clone();
        validate_original(&p, href, bytes, sha).unwrap();
        assert!(validate_original(&p, href, bytes + 1, sha).is_err());
        assert!(validate_original(&p, href, bytes, &"0".repeat(64)).is_err());
        assert!(validate_original(&p, &format!("{href}?sig=not-canonical"), bytes, sha).is_err());
        for (other_id, other_key, other_bytes, other_sha) in cases {
            if (other_id, other_key) == (id, key) {
                continue;
            }
            // A Burn_Date URL returning a complete, otherwise valid QA file
            // must fail; its known grid and SHA are not sufficient permission.
            assert!(validate_original(&p, href, other_bytes, other_sha).is_err());
            let (_, other_item) = proof(other_id, other_key);
            let other_href = other_item.document["assets"][other_key]["href"]
                .as_str()
                .unwrap();
            assert!(validate_original(&p, other_href, bytes, sha).is_err());
        }
        let mut changed = p.clone();
        changed.layer.nodata = Some(0.);
        assert!(validate_original(&changed, href, bytes, sha).is_err());
        assert_eq!(item.document, original);
    }
}

#[test]
#[ignore = "requires the three complete independent originals in GEOD_ADDITIONAL_PC_RESEARCH_DIR"]
fn complete_independent_originals_decode_without_grid_code_or_nodata_changes() {
    use sha2::{Digest, Sha256};
    let dir = std::path::PathBuf::from(
        std::env::var_os("GEOD_ADDITIONAL_PC_RESEARCH_DIR").expect("research directory"),
    );
    let reference: Value =
        serde_json::from_slice(&std::fs::read(dir.join("independent-originals.json")).unwrap())
            .unwrap();
    for (name, sha, width, height, dtype, nodata) in [
        (
            "planetary-more-sst.tif",
            OISST_SHA256,
            1440,
            720,
            "Int16",
            json!(-999),
        ),
        (
            "modis-more-Burn_Date.tif",
            BURN_DATE_SHA256,
            2400,
            2400,
            "Int16",
            json!(-1),
        ),
        (
            "modis-more-QA.tif",
            BURN_QA_SHA256,
            2400,
            2400,
            "UInt8",
            Value::Null,
        ),
    ] {
        let bytes = std::fs::read(dir.join(name)).unwrap();
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), sha);
        let m = crate::stac::raster::inspect_local(&bytes, "independent-research-original", sha)
            .unwrap();
        assert_eq!((m.width, m.height), (width, height));
        assert_eq!(m.bands.len(), 1);
        assert_eq!(m.bands[0].data_type, dtype);
        assert_eq!(m.bands[0].nodata, nodata);
        assert!(m
            .preview_data_url
            .as_ref()
            .is_some_and(|v| v.starts_with("data:image/png;base64,")));
        let original = reference["references"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|v| v["profiles"].as_array().unwrap())
            .find(|p| p["sha256"] == sha)
            .unwrap();
        for point in original["samples"].as_array().unwrap() {
            let pixel = crate::stac::raster::sample_local(
                &bytes,
                "independent-research-original",
                sha,
                point["column"].as_u64().unwrap() as u32,
                point["row"].as_u64().unwrap() as u32,
            )
            .unwrap();
            assert_eq!(serde_json::to_value(pixel.values).unwrap(), point["values"]);
            assert_eq!(
                serde_json::to_value(pixel.no_data).unwrap(),
                point["noData"]
            );
        }
        if is_modis_original_sha(sha) {
            validate_modis_grid(&m).unwrap();
            for field in ["sha", "grid", "type", "nodata", "area", "crs"] {
                let mut changed = m.clone();
                match field {
                    "sha" => changed.sha256 = "unreviewed".into(),
                    "grid" => changed.transform.as_mut().unwrap()[2] += 1.,
                    "type" => changed.bands[0].data_type = "UInt16".into(),
                    "nodata" => changed.bands[0].nodata = json!(0.),
                    "area" => changed.pixel_interpretation = Some("point".into()),
                    _ => changed.crs = Some("EPSG:4326".into()),
                }
                assert!(validate_modis_grid(&changed).is_err(), "{field}");
            }
        } else {
            assert_eq!(m.crs.as_deref(), Some("EPSG:4326"));
            assert_eq!(m.transform, Some([0.25, 0., -180., 0., -0.25, 90.]));
        }
    }
}
