use super::*;
use serde_json::json;

fn request(id: &str, bounds: [f64; 4]) -> ResolveRequest {
    ResolveRequest {
        dataset_id: id.into(),
        bounds,
        year: None,
        month: None,
        country: None,
        layer: None,
        region: None,
        season: None,
    }
}
fn href(plan: &DatasetPlan) -> &str {
    &plan.sources[0].stac.as_ref().unwrap().url
}

#[test]
fn product_catalog_discloses_licenses_raw_units_and_original_download_limits() {
    let datasets = list();
    for id in [
        "worldcover",
        "worldcover-composites",
        "worldpop",
        "jrc-water",
        "glad-forest",
        "etopo",
        "chirps",
        "nasadem",
        "srtm-gl1",
        "alos-aw3d30",
        "hydrosheds",
        "cbers-sr",
        "io-lulc",
        "hilda-land-use",
        "worldcereal",
    ] {
        assert!(
            datasets.iter().any(|dataset| dataset.id == id),
            "missing {id}"
        );
    }
    let mut ids = std::collections::HashSet::new();
    for dataset in datasets {
        assert!(ids.insert(dataset.id.clone()));
        assert!(!dataset.license.is_empty());
        assert!(dataset.license_url.starts_with("https://"));
        // All definitions share one frontend catalog contract. A page anchor
        // is not a canonical service URL and must not invalidate every product.
        for address in [&dataset.license_url, &dataset.documentation_url] {
            let url = url::Url::parse(address).expect("reviewed public product documentation URL");
            assert_eq!(url.scheme(), "https");
            assert!(url.host_str().is_some());
            assert!(
                url.fragment().is_none(),
                "noncanonical document URL for {}",
                dataset.id
            );
            assert!(url.username().is_empty() && url.password().is_none());
        }
        assert!(!dataset.attribution.is_empty());
        assert!(!dataset.layers.is_empty());
        assert!(dataset
            .layers
            .iter()
            .any(|l| Some(&l.id) == dataset.default_layer.as_ref()));
        assert!(dataset.limits.iter().any(|w| w.contains("512 MiB")));
    }
}

#[test]
fn old_fourteen_catalogs_and_requests_omit_regional_fields_without_proof_drift() {
    for dataset in list()
        .into_iter()
        .filter(|dataset| dataset.id != "worldcereal")
    {
        let value = serde_json::to_value(&dataset).unwrap();
        for key in ["regions", "seasons", "defaultRegion", "defaultSeason"] {
            assert!(value.get(key).is_none(), "{} gained {key}", dataset.id);
        }
        for option in value["layers"].as_array().unwrap() {
            assert!(option.get("seasons").is_none());
        }
    }
    let plan = resolve(request("worldcover", [10., 10., 11., 11.])).unwrap();
    let proof = provenance(&plan, &plan.sources[0]);
    let old = serde_json::to_value(&proof).unwrap();
    assert!(old["request"].get("region").is_none());
    assert!(old["request"].get("season").is_none());
    validate_provenance(&serde_json::from_value(old).unwrap(), href(&plan)).unwrap();
    let mut illegal = plan.request;
    illegal.region = Some("9026".into());
    assert!(resolve(illegal.clone()).unwrap_err().contains("region"));
    illegal.region = None;
    illegal.season = Some("tc-annual".into());
    assert!(resolve(illegal).unwrap_err().contains("season"));
}

#[test]
fn worldcereal_matches_actual_area_and_theme_season_without_substituting_a_demo_region() {
    let initial = request("worldcereal", [57.5, -20.1, 57.6, -20.]);
    let plan = resolve(initial).unwrap();
    assert_eq!(plan.dataset.discovery.as_deref(), Some("stac-items"));
    assert_eq!(plan.request.year, Some(2021));
    assert_eq!(plan.request.region, None);
    assert_eq!(plan.request.season.as_deref(), Some("tc-annual"));
    assert_eq!(plan.sources.len(), 1);
    assert_eq!(plan.sources[0].stac.as_ref().unwrap().kind, "item");
    assert_eq!(plan.sources[0].region.as_deref(), Some("9026"));
    assert_eq!(
        plan.sources[0].start_datetime.as_deref(),
        Some("2020-11-09T00:00:00Z")
    );
    assert_eq!(
        plan.sources[0].end_datetime.as_deref(),
        Some("2021-11-09T23:59:59Z")
    );
    assert!(plan.sources[0].search.is_none());
    assert_eq!(
        plan.sources[0].bounds,
        Some([
            53.864516174824,
            -22.69539316166828,
            59.99509950815733,
            -18.981793161668282
        ])
    );
    for option in &plan.dataset.layers {
        let mut selected = request("worldcereal", [57.5, -20.1, 57.6, -20.]);
        selected.layer = Some(option.id.clone());
        let result = resolve(selected.clone()).unwrap();
        assert!(result.sources[0].stac.as_ref().unwrap().name.len() <= 80);
        assert_eq!(result.request.season.as_ref(), option.seasons.first());
        let bad = if result.request.season.as_deref() == Some("tc-annual") {
            "tc-maize-main"
        } else {
            "tc-annual"
        };
        selected.season = Some(bad.into());
        assert!(resolve(selected).is_err());
    }
    assert!(resolve(request("worldcereal", [116., 39., 117., 40.]))
        .unwrap_err()
        .contains("does not intersect"));
    let mut selected = request("worldcereal", [57.5, -20.1, 57.6, -20.]);
    selected.region = Some("9029".into());
    assert!(resolve(selected.clone()).is_err());
    selected.region = Some("9026".into());
    selected.year = Some(2022);
    assert!(resolve(selected.clone()).is_err());
    selected.year = Some(2021);
    selected.month = Some(11);
    assert!(resolve(selected).is_err());
}

#[test]
fn worldcereal_scoped_transport_binds_real_documents_without_fabricating_metadata_or_masks() {
    for option in worldcereal::definition().layers {
        let mut selected = request("worldcereal", [57.5, -20.1, 57.6, -20.]);
        selected.layer = Some(option.id.clone());
        let plan = resolve(selected).unwrap();
        let proof = provenance(&plan, &plan.sources[0]);
        let (url, raw) = worldcereal::reviewed_document(&option.id);
        let before = raw.clone();
        worldcereal::validate(&proof, &url, Some(&raw)).unwrap();
        assert!(validate_provenance(&proof, &url).is_err()); // never a synthetic raster
        let mappings = worldcereal::transports(&proof, &url, &raw).unwrap();
        assert_eq!(mappings.len(), raw["assets"].as_object().unwrap().len());
        for mapping in &mappings {
            assert_eq!(
                mapping.original_href,
                raw["assets"][&mapping.asset_key]["href"]
            );
            assert_eq!(mapping.adapter, worldcereal::ADAPTER);
            assert!(mapping.href.starts_with(
                "https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/esa_worldcereal/"
            ));
        }
        assert_eq!(worldcereal::self_url(&proof, &url, &raw).unwrap(), url);
        assert_eq!(raw, before);
        assert_eq!(proof.layer.nodata, None); // no synthetic publisher mask
        for href in [
            "s3://unreviewed-bucket/esa_worldcereal/raw.tif",
            "s3://deafrica-input-datasets/private/raw.tif",
            "s3://deafrica-input-datasets/esa_worldcereal/../raw.tif",
            "s3://deafrica-input-datasets/esa_worldcereal/%2e%2e/raw.tif",
            "s3://deafrica-input-datasets/esa_worldcereal/raw.tif?X-Amz-Signature=x",
        ] {
            let mut changed = raw.clone();
            changed["assets"]["classification"]["href"] = json!(href);
            assert!(worldcereal::transports(&proof, &url, &changed).is_err());
        }
        for pointer in [
            "/properties/odc:region_code",
            "/properties/odc:dataset_version",
            "/properties/start_datetime",
            "/collection",
        ] {
            let mut changed = raw.clone();
            *changed.pointer_mut(pointer).unwrap() = json!("changed");
            assert!(worldcereal::validate(&proof, &url, Some(&changed)).is_err());
        }
        let mut changed = proof.clone();
        changed.request.bounds = [116., 39., 117., 40.];
        assert!(worldcereal::validate(&changed, &url, Some(&raw)).is_err());
    }
    let dataset = worldcereal::definition();
    assert!(dataset.value_interpretation.contains("254 is a valid"));
    assert!(dataset
        .warnings
        .iter()
        .any(|warning| warning.contains("no collection link")));
    assert!(dataset
        .warnings
        .iter()
        .any(|warning| warning.contains("independent assets") && warning.contains("Confidence")));
    let (_, raw) = worldcereal::reviewed_document("temporarycrops");
    assert_ne!(
        raw["assets"]["classification"]["proj:transform"],
        raw["assets"]["confidence"]["proj:transform"]
    );
}

#[test]
fn worldcover_signed_three_degree_grid_and_versions_are_deterministic() {
    let plan = resolve(request("worldcover", [15., 78., 18., 81.])).unwrap();
    assert_eq!(plan.sources.len(), 1);
    assert!(href(&plan).ends_with("/v200/2021/map/ESA_WorldCover_10m_2021_v200_N78E015_Map.tif"));
    assert_eq!(plan.sources[0].bounds, Some([15., 78., 18., 81.]));
    let mut selected = request("worldcover", [-0.5, -0.5, 0., 0.]);
    selected.year = Some(2020);
    let plan = resolve(selected).unwrap();
    assert_eq!(plan.sources.len(), 1);
    assert!(href(&plan).ends_with("ESA_WorldCover_10m_2020_v100_S03W003_Map.tif"));
}

#[test]
fn tiny_boundary_crossing_selects_all_intersections_and_large_areas_are_rejected() {
    let plan = resolve(request("worldcover", [-0.01, -0.01, 0.01, 0.01])).unwrap();
    assert_eq!(plan.sources.len(), 4);
    assert!(resolve(request("worldcover", [-10., -10., 10., 10.]))
        .unwrap_err()
        .contains("64"));
    assert!(resolve(request("worldcover", [180., 1., -180., 2.])).is_err());
    assert!(resolve(request("worldcover", [1., 1., f64::NAN, 2.])).is_err());
}

#[test]
fn composite_paths_retain_source_band_families_not_rendered_tiles() {
    let mut selected = request("worldcover-composites", [10.1, 78.1, 10.2, 78.2]);
    for (layer, ending) in [
        ("swir", "SWIR"),
        ("rgbnir", "S2RGBNIR"),
        ("ndvi", "NDVI"),
        ("gamma0", "S1VVVHratio"),
    ] {
        selected.layer = Some(layer.into());
        let plan = resolve(selected.clone()).unwrap();
        assert_eq!(plan.sources.len(), 1);
        assert!(href(&plan).ends_with(&format!(
            "/2021/N78/ESA_WorldCover_10m_2021_v200_N78E010_{ending}.tif"
        )));
        assert_eq!(plan.sources[0].bounds, Some([10., 78., 11., 79.]));
        let proof = provenance(&plan, &plan.sources[0]);
        assert_eq!(proof.layer.id, layer);
        validate_provenance(&proof, href(&plan)).unwrap();
    }
}

#[test]
fn whole_country_population_does_not_fabricate_map_area_coverage() {
    let mut selected = request("worldpop", [116., 39., 117., 40.]);
    selected.country = Some(" niu ".into());
    let plan = resolve(selected).unwrap();
    assert_eq!(plan.request.country.as_deref(), Some("NIU"));
    assert_eq!(plan.sources[0].bounds, None);
    assert_eq!(
        href(&plan),
        "https://data.worldpop.org/GIS/Population/Global_2000_2020/2020/NIU/niu_ppp_2020.tif"
    );
    assert!(plan
        .warnings
        .iter()
        .any(|s| s.contains("entire selected country's")));
    for invalid in ["../", "CN", "N/1", "NIU?token=x", "中"] {
        let mut selected = request("worldpop", [1., 1., 2., 2.]);
        selected.country = Some(invalid.into());
        assert!(resolve(selected).is_err());
    }
}

#[test]
fn jrc_uses_upper_tile_edge_and_does_not_alias_extent_to_max_extent() {
    let mut selected = request("jrc-water", [110., 40., 120., 50.]);
    let plan = resolve(selected.clone()).unwrap();
    assert_eq!(plan.sources.len(), 1);
    assert!(href(&plan).ends_with("occurrence/occurrence_110E_50N_v1_5_2024.tif"));
    selected.layer = Some("extent".into());
    let plan = resolve(selected).unwrap();
    assert!(href(&plan).ends_with("extent/extent_110E_50N_v1_5_2024.tif"));
    assert!(provenance(&plan, &plan.sources[0])
        .layer
        .value_interpretation
        .contains("2022–2024"));
    let plan = resolve(request("jrc-water", [-10., -10., 0., 0.])).unwrap();
    assert!(href(&plan).contains("occurrence_10W_0N_"));
    assert!(resolve(request("jrc-water", [1., 79., 2., 81.])).is_err());
}

#[test]
fn forest_loss_year_is_not_zero_nodata_or_latest_gain() {
    let plan = resolve(request("glad-forest", [110.1, 40.1, 110.2, 40.2])).unwrap();
    assert!(href(&plan).ends_with("Hansen_GFC-2025-v1.13_lossyear_50N_110E.tif"));
    let proof = provenance(&plan, &plan.sources[0]);
    assert_eq!(proof.layer.nodata, None);
    assert!(proof.layer.value_interpretation.contains("1–25"));
    let mut selected = plan.request;
    selected.layer = Some("gain".into());
    let plan = resolve(selected).unwrap();
    assert!(provenance(&plan, &plan.sources[0])
        .layer
        .value_interpretation
        .contains("2000–2012"));
}

#[test]
fn etopo_uses_fifteen_degree_upper_left_anchor_and_retains_vertical_datum() {
    let plan = resolve(request("etopo", [0.1, 40.1, 0.2, 40.2])).unwrap();
    assert!(href(&plan).ends_with("ETOPO_2022_v1_15s_N45E000_surface.tif"));
    assert_eq!(plan.sources[0].bounds, Some([0., 30., 15., 45.]));
    assert!(plan.dataset.unit.contains("EGM2008"));
    let plan = resolve(request("etopo", [-0.1, -15.1, 0., -15.])).unwrap();
    assert!(href(&plan).ends_with("ETOPO_2022_v1_15s_S15W015_surface.tif"));
}

#[test]
fn chirps_monthly_final_uses_zero_padded_month_and_whole_domain() {
    let plan = resolve(request("chirps", [116., 39., 117., 40.])).unwrap();
    assert_eq!(plan.request.year, Some(2024));
    assert_eq!(plan.request.month, Some(1));
    assert_eq!(plan.dataset.months, (1..=12).collect::<Vec<_>>());
    assert_eq!(plan.sources.len(), 1);
    assert_eq!(plan.sources[0].bounds, Some([-180., -60., 180., 60.]));
    assert_eq!(href(&plan), "https://data.chc.ucsb.edu/products/CHIRPS/v3.0/monthly/global/tifs/chirps-v3.0.2024.01.tif");
    let declared = provenance(&plan, &plan.sources[0]);
    assert_eq!(declared.layer.scale, Some(1.));
    assert_eq!(declared.layer.offset, Some(0.));
    assert_eq!(declared.layer.nodata, Some(-9999.));
    assert!(declared
        .layer
        .value_interpretation
        .contains("embedded TIFF NoData"));
    assert!(plan
        .dataset
        .warnings
        .iter()
        .any(|text| text.contains("no embedded TIFF NoData")));
    validate_provenance(&declared, href(&plan)).unwrap();
    let mut selected = request("chirps", [-180., -60., 180., 60.]);
    selected.year = Some(1981);
    selected.month = Some(12);
    let plan = resolve(selected).unwrap();
    assert!(href(&plan).ends_with("chirps-v3.0.1981.12.tif"));
    assert!(!href(&plan).contains("prelim"));
}

#[test]
fn monthly_parameters_and_chirps_extent_fail_before_network() {
    for month in [0, 13, 255] {
        let mut selected = request("chirps", [0., 0., 1., 1.]);
        selected.month = Some(month);
        assert!(resolve(selected).is_err());
    }
    for year in [1980, 2027] {
        let mut selected = request("chirps", [0., 0., 1., 1.]);
        selected.year = Some(year);
        assert!(resolve(selected).is_err());
    }
    for area in [[0., -60.01, 1., -59.], [0., 59., 1., 60.01]] {
        assert!(resolve(request("chirps", area)).is_err());
    }
    for dataset in list()
        .into_iter()
        .filter(|dataset| dataset.months.is_empty())
    {
        let mut selected = request(&dataset.id, [0., 0., 1., 1.]);
        selected.month = Some(1);
        assert!(
            resolve(selected).is_err(),
            "{} must reject month",
            dataset.id
        );
    }
    assert!(serde_json::from_value::<ResolveRequest>(
        json!({"datasetId":"chirps","bounds":[0,0,1,1],"month":"01"})
    )
    .is_err());
}

#[test]
fn adding_month_options_preserves_legacy_presets_and_receipt_shape() {
    for dataset in list()
        .into_iter()
        .filter(|dataset| dataset.discovery.is_none() && dataset.id != "chirps")
    {
        let serialized = serde_json::to_value(&dataset).unwrap();
        assert!(serialized.get("months").is_none());
        assert!(serialized.get("defaultMonth").is_none());
        assert!(serialized.get("discovery").is_none());
        let reopened: Dataset = serde_json::from_value(serialized.clone()).unwrap();
        assert_eq!(serde_json::to_value(reopened).unwrap(), serialized);
    }
    let legacy: ResolveRequest = serde_json::from_value(
        json!({"datasetId":"worldcover","bounds":[15,78,18,81],"year":2021,"layer":"map"}),
    )
    .unwrap();
    assert_eq!(legacy.month, None);
    let plan = resolve(legacy).unwrap();
    let proof = provenance(&plan, &plan.sources[0]);
    let serialized = serde_json::to_value(&proof).unwrap();
    assert!(serialized["request"].get("month").is_none());
    assert!(serialized["dataset"].get("months").is_none());
    assert!(serialized["dataset"].get("discovery").is_none());
    assert!(serde_json::to_value(&plan.sources[0])
        .unwrap()
        .get("search")
        .is_none());
    let bytes = serde_json::to_vec(&proof).unwrap();
    let reopened: SourceProvenance = serde_json::from_slice(&bytes).unwrap();
    validate_provenance(&reopened, href(&plan)).unwrap();
    assert_eq!(serde_json::to_vec(&reopened).unwrap(), bytes);
}

#[test]
fn monthly_receipt_cannot_relabel_period_or_source() {
    let plan = resolve(request("chirps", [116., 39., 117., 40.])).unwrap();
    let mut proof = provenance(&plan, &plan.sources[0]);
    proof.request.month = Some(2);
    assert!(validate_provenance(&proof, href(&plan)).is_err());
    let mut proof = provenance(&plan, &plan.sources[0]);
    proof.layer.unit = "mm/day".into();
    assert!(validate_provenance(&proof, href(&plan)).is_err());
}

#[test]
fn public_elevation_archives_keep_independent_paths_and_sample_registration() {
    for (id, path, layer_id) in [
        (
            "nasadem",
            "NASADEM/NASADEM_be/NASADEM_HGT_n00e006.tif",
            "elevation",
        ),
        (
            "srtm-gl1",
            "SRTM_GL1/SRTM_GL1_srtm/N00E006.tif",
            "elevation",
        ),
        (
            "alos-aw3d30",
            "AW3D30/AW3D30_global/ALPSMLC30_N000E006_DSM.tif",
            "surface",
        ),
    ] {
        let plan = resolve(request(id, [6.65, 0.2, 6.7, 0.3])).unwrap();
        assert_eq!(plan.sources.len(), 1);
        assert_eq!(
            href(&plan),
            format!("https://opentopography.s3.sdsc.edu/raster/{path}")
        );
        assert_eq!(plan.sources[0].bounds, Some([6., 0., 7., 1.]));
        assert_eq!(plan.request.layer.as_deref(), Some(layer_id));
        assert_eq!(plan.request.year, None);
        assert_eq!(plan.request.month, None);
        assert!(plan.dataset.unit.contains("EGM96"));
        assert!(plan
            .dataset
            .warnings
            .iter()
            .any(|text| text.contains("PixelIs")));
        let proof = provenance(&plan, &plan.sources[0]);
        validate_provenance(&proof, href(&plan)).unwrap();
        if id == "alos-aw3d30" {
            assert_eq!(proof.layer.nodata, None);
            assert!(plan.dataset.license.contains("JAXA"));
            assert!(plan
                .dataset
                .warnings
                .iter()
                .any(|text| text.contains("notifying JAXA in advance")));
        } else {
            assert_eq!(proof.layer.nodata, Some(-32768.));
            assert!(plan
                .dataset
                .warnings
                .iter()
                .any(|text| text.contains("point sample centres")));
        }
    }
    let plan = resolve(request("nasadem", [-0.2, -0.2, 0., 0.])).unwrap();
    assert!(href(&plan).ends_with("NASADEM_HGT_s01w001.tif"));
    let plan = resolve(request("srtm-gl1", [-0.2, -0.2, 0., 0.])).unwrap();
    assert!(href(&plan).ends_with("S01W001.tif"));
    let plan = resolve(request("alos-aw3d30", [-0.2, -0.2, 0., 0.])).unwrap();
    assert!(href(&plan).ends_with("ALPSMLC30_S001W001_DSM.tif"));
}

#[test]
fn elevation_archive_limits_reject_acquisition_dates_and_large_selections() {
    for (id, min_lat, max_lat) in [
        ("nasadem", -56., 61.),
        ("srtm-gl1", -56., 60.),
        ("alos-aw3d30", -84., 84.),
    ] {
        assert!(resolve(request(id, [0., min_lat - 0.1, 1., min_lat + 0.1])).is_err());
        assert!(resolve(request(id, [0., max_lat - 0.1, 1., max_lat + 0.1])).is_err());
        assert!(resolve(request(id, [0., 0., 3., 3.])).is_err());
        let mut selected = request(id, [0., 0., 1., 1.]);
        selected.year = Some(2000);
        assert!(resolve(selected).is_err());
        let plan = resolve(request(id, [0., 0., 1., 1.])).unwrap();
        assert_eq!(plan.sources.len(), 1);
    }
}

#[test]
fn hydrosheds_only_resolves_pinned_dir_tiles_and_never_quietly_drops_missing_tiles() {
    let plan = resolve(request("hydrosheds", [-49.9, 0.1, -49.8, 0.2])).unwrap();
    assert_eq!(plan.sources.len(), 1);
    assert_eq!(
        href(&plan),
        "https://data.hydrosheds.org/file/hydrosheds-v2/DIR/1s/n00w050_DIR_1s_v2r0.tif"
    );
    assert_eq!(plan.sources[0].bounds, Some([-50., 0., -40., 10.]));
    let proof = provenance(&plan, &plan.sources[0]);
    assert_eq!(proof.layer.nodata, Some(255.));
    assert!(proof.layer.value_interpretation.contains("0 sink"));
    validate_provenance(&proof, href(&plan)).unwrap();
    assert!(resolve(request("hydrosheds", [116., 39., 117., 40.])).is_err());
    // West tile is published, east tile absent: the entire selection must fail.
    assert!(resolve(request("hydrosheds", [-40.1, 1., -39.9, 2.])).is_err());
    let mut selected = request("hydrosheds", [-49.9, 0.1, -49.8, 0.2]);
    selected.layer = Some("flow-accumulation".into());
    assert!(resolve(selected).is_err());
    // Large listed originals stay visible with the real transfer-size boundary.
    let plan = resolve(request("hydrosheds", [-69., -9., -68., -8.])).unwrap();
    assert!(href(&plan).contains("s10w070_DIR_1s_v2r0.tif"));
    assert!(plan
        .dataset
        .warnings
        .iter()
        .any(|text| text.contains("627 MB")));
}

#[test]
fn hydrosheds_finite_recipe_matches_the_reviewed_publisher_index() {
    let receipt: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../docs/research/hydrosheds-v2-index-20261012.json"
    ))
    .unwrap();
    assert_eq!(
        receipt["publisherIndexDocumentSha256"],
        "28a3b61e2c6ade29fc4e8eed5c409a6704b73dad36e80324ca2d90934aecba6d"
    );
    let expected: std::collections::HashSet<_> = receipt["directionTiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|tile| tile["tileId"].as_str().unwrap())
        .collect();
    let actual: std::collections::HashSet<_> =
        hydrosheds::FLOW_DIRECTION_TILES.into_iter().collect();
    assert_eq!(actual.len(), 100);
    assert_eq!(actual, expected);
    for tile in receipt["directionTiles"].as_array().unwrap() {
        let id = tile["tileId"].as_str().unwrap();
        let north = if &id[..1] == "s" { -1. } else { 1. };
        let west = if &id[3..4] == "w" { -1. } else { 1. };
        let y = id[1..3].parse::<f64>().unwrap() * north;
        let x = id[4..].parse::<f64>().unwrap() * west;
        let plan = resolve(request("hydrosheds", [x + 0.1, y + 0.1, x + 0.2, y + 0.2])).unwrap();
        assert_eq!(href(&plan), tile["url"].as_str().unwrap());
    }
}

#[test]
fn unsupported_and_irrelevant_parameters_fail_before_network() {
    let mut selected = request("worldcover", [1., 1., 2., 2.]);
    selected.year = Some(2025);
    assert!(resolve(selected).is_err());
    let mut selected = request("etopo", [1., 1., 2., 2.]);
    selected.year = Some(2022);
    assert!(resolve(selected).is_err());
    let mut selected = request("worldcover", [1., 1., 2., 2.]);
    selected.country = Some("USA".into());
    assert!(resolve(selected).is_err());
    let mut selected = request("worldcover", [1., 1., 2., 2.]);
    selected.layer = Some("visual".into());
    assert!(resolve(selected).is_err());
    assert!(resolve(request("soilgrids", [1., 1., 2., 2.])).is_err());
    assert!(serde_json::from_value::<ResolveRequest>(
        json!({"datasetId":"worldcover","bounds":[1,1,2,2],"href":"https://attacker.example/a.tif"})
    )
    .is_err());
}

#[test]
fn native_provenance_cannot_relabel_other_files_units_or_areas() {
    let plan = resolve(request("worldcover", [15., 78., 18., 81.])).unwrap();
    let mut proof = provenance(&plan, &plan.sources[0]);
    validate_provenance(&proof, href(&plan)).unwrap();
    assert!(validate_provenance(&proof, "https://example.com/other.tif").is_err());
    proof.source_bounds = Some([116., 39., 117., 40.]);
    assert!(validate_provenance(&proof, href(&plan)).is_err());
    let mut proof = provenance(&plan, &plan.sources[0]);
    proof.layer.unit = "true colour reflectance".into();
    assert!(validate_provenance(&proof, href(&plan)).is_err());
}

#[test]
fn corrected_grid_uses_v2_and_existing_v1_resolution_receipts_stay_readable() {
    for (id, legacy) in [
        ("jrc-water", "30 m (1 arc-second grid)"),
        ("glad-forest", "approximately 30 m (1 arc-second grid)"),
    ] {
        let plan = resolve(request(id, [110.1, 40.1, 110.2, 40.2])).unwrap();
        let current = provenance(&plan, &plan.sources[0]);
        assert_eq!(current.schema_version, 2);
        assert_eq!(
            current.dataset.nominal_resolution,
            "approximately 30 m (0.00025 degree grid)"
        );
        validate_provenance(&current, href(&plan)).unwrap();

        let mut previous = current.clone();
        previous.schema_version = 1;
        previous.dataset.nominal_resolution = legacy.into();
        let bytes = serde_json::to_vec(&previous).unwrap();
        validate_provenance(&previous, href(&plan)).unwrap();
        assert_eq!(serde_json::to_vec(&previous).unwrap(), bytes);
        let reopened: SourceProvenance = serde_json::from_slice(&bytes).unwrap();
        validate_provenance(&reopened, href(&plan)).unwrap();

        let mut wrong_v1 = previous.clone();
        wrong_v1.dataset.nominal_resolution = current.dataset.nominal_resolution.clone();
        assert!(validate_provenance(&wrong_v1, href(&plan)).is_err());
        let mut wrong_v2 = current.clone();
        wrong_v2.dataset.nominal_resolution = legacy.into();
        assert!(validate_provenance(&wrong_v2, href(&plan)).is_err());
        previous.dataset.license = "unrestricted purchase entitlement".into();
        assert!(validate_provenance(&previous, href(&plan)).is_err());
        let mut bad_version = current;
        for version in [0, 3] {
            bad_version.schema_version = version;
            assert!(validate_provenance(&bad_version, href(&plan)).is_err());
        }
    }
    // Presets unaffected by the correction preserve v1 proof readability too.
    let plan = resolve(request("worldcover", [15., 78., 18., 81.])).unwrap();
    let mut previous = provenance(&plan, &plan.sources[0]);
    previous.schema_version = 1;
    validate_provenance(&previous, href(&plan)).unwrap();
}

#[test]
fn cbers_queries_fixed_official_collections_and_actual_observation_months() {
    for (layer, collection, first_year) in [
        ("cbers4a-mux", "CB4A-MUX-L4-SR-1", 2026),
        ("cbers4-mux", "CB4-MUX-L4-SR-1", 2016),
        ("cbers4a-wfi", "CB4A-WFI-L4-SR-1", 2020),
        ("cbers4-wfi", "CB4-WFI-L4-SR-1", 2016),
    ] {
        let mut selected = request("cbers-sr", [-44., -24., -43., -22.]);
        selected.layer = Some(layer.into());
        let plan = resolve(selected.clone()).unwrap();
        assert_eq!(plan.dataset.discovery.as_deref(), Some("stac-api"));
        assert_eq!(plan.request.year, Some(2026));
        assert_eq!(plan.request.month, Some(9));
        assert_eq!(plan.sources.len(), 1);
        assert_eq!(href(&plan), "https://data.inpe.br/bdc/stac/v1/");
        assert_eq!(plan.sources[0].stac.as_ref().unwrap().kind, "api");
        // AOI is a query constraint, never a fabricated scene/file footprint.
        assert_eq!(plan.sources[0].bounds, None);
        let search = plan.sources[0].search.as_ref().unwrap();
        assert_eq!(search.collection_id, collection);
        assert_eq!(search.bounds, [-44., -24., -43., -22.]);
        assert_eq!(search.limit, 20);
        assert_eq!(
            search.datetime,
            "2026-09-01T00:00:00Z/2026-09-30T23:59:59.999999Z"
        );
        selected.year = Some(first_year - 1);
        assert!(
            resolve(selected.clone()).is_err(),
            "{layer} must retain its own date lower bound"
        );
        selected.year = Some(first_year);
        assert!(resolve(selected).is_ok());
    }
    let mut selected = request("cbers-sr", [-44., -24., -43., -22.]);
    selected.layer = Some("cbers4-mux".into());
    selected.year = Some(2024);
    selected.month = Some(2);
    let plan = resolve(selected.clone()).unwrap();
    assert!(plan.sources[0]
        .search
        .as_ref()
        .unwrap()
        .datetime
        .ends_with("2024-02-29T23:59:59.999999Z"));
    selected.month = Some(12);
    selected.year = Some(2026);
    assert!(resolve(selected).unwrap().sources[0]
        .search
        .as_ref()
        .unwrap()
        .datetime
        .ends_with("2026-12-31T23:59:59.999999Z"));
    assert!(serde_json::from_value::<ResolveRequest>(json!({"datasetId":"cbers-sr","bounds":[-44,-24,-43,-22],"search":{"url":"https://unreviewed.test/"}})).is_err());
}

#[test]
fn io_annual_query_does_not_misread_exclusive_archive_end_or_rewrite_license() {
    let plan = resolve(request("io-lulc", [-15., 83., -14., 84.])).unwrap();
    assert_eq!(plan.request.year, Some(2024));
    assert_eq!(plan.request.month, None);
    assert!(plan.dataset.months.is_empty());
    assert_eq!(plan.request.layer.as_deref(), Some("land-cover"));
    assert_eq!(href(&plan), "https://api.impactobservatory.com/stac-aws/");
    assert_eq!(plan.sources[0].bounds, None);
    let search = plan.sources[0].search.as_ref().unwrap();
    assert_eq!(search.collection_id, "io-10m-annual-lulc");
    assert_eq!(
        search.datetime,
        "2024-01-01T00:00:00Z/2024-12-31T23:59:59.999999Z"
    );
    assert!(plan
        .dataset
        .warnings
        .iter()
        .any(|text| text.contains("proprietary") && text.contains("unchanged")));
    assert!(plan
        .dataset
        .limits
        .iter()
        .any(|text| text.contains("polar") && text.contains("131,072")));
    let mut selected = plan.request;
    selected.year = Some(2025);
    assert!(resolve(selected.clone()).is_err());
    selected.year = Some(2024);
    selected.month = Some(1);
    assert!(resolve(selected).is_err());
}

#[test]
fn curated_apis_cannot_be_relabelled_as_publisher_direct_raster_proofs() {
    for id in ["cbers-sr", "io-lulc"] {
        let plan = resolve(request(id, [-44., -24., -43., -22.])).unwrap();
        let proof = provenance(&plan, &plan.sources[0]);
        assert!(validate_provenance(&proof, href(&plan))
            .unwrap_err()
            .contains("synthetic"));
    }
    assert!(serde_json::from_value::<DatasetSearch>(json!({"collectionId":"CB4-MUX-L4-SR-1","datetime":"2026-09-01T00:00:00Z/2026-09-30T23:59:59Z","bounds":[-44,-24,-43,-22],"limit":20,"url":"https://unreviewed.test/"})).is_err());
}

#[test]
fn hilda_uses_whole_historical_file_without_fabricated_mask_or_stac_extent() {
    let plan = resolve(request("hilda-land-use", [6.65, 0.2, 6.7, 0.3])).unwrap();
    assert_eq!(plan.request.year, Some(2019));
    assert_eq!(plan.request.month, None);
    assert_eq!(plan.dataset.discovery, None);
    assert_eq!(plan.request.layer.as_deref(), Some("land-use"));
    assert_eq!(href(&plan), "https://s3.openlandmap.org/arco/land.use.land.cover_hilda.plus_c_1km_s_20190101_20191231_go_espg.4326_v1.0.tif");
    assert_eq!(
        plan.sources[0].bounds,
        Some([-180., -61.98833333333333, 180., 87.37])
    );
    assert_eq!(plan.sources[0].search, None);
    let proof = provenance(&plan, &plan.sources[0]);
    assert_eq!(proof.layer.nodata, None);
    assert!(proof
        .layer
        .value_interpretation
        .contains("55 unmanaged grass/shrubland"));
    assert!(proof
        .layer
        .value_interpretation
        .contains("66 sparse/no vegetation"));
    assert!(proof.layer.value_interpretation.contains("77 water"));
    assert!(proof
        .layer
        .value_interpretation
        .contains("99 publisher-declared no data"));
    assert!(plan
        .dataset
        .warnings
        .iter()
        .any(|text| text.contains("SLD/QML") && text.contains("conflicting")));
    assert!(plan
        .dataset
        .warnings
        .iter()
        .any(|text| text.contains("inconsistent") && text.contains("STAC")));
    validate_provenance(&proof, href(&plan)).unwrap();
    for bounds in [[0., -62., 1., -61.], [0., 87., 1., 87.5]] {
        assert!(resolve(request("hilda-land-use", bounds)).is_err());
    }
    let mut selected = plan.request;
    selected.year = Some(1960);
    assert!(href(&resolve(selected.clone()).unwrap()).contains("19600101_19601231"));
    selected.year = Some(2020);
    assert!(resolve(selected).is_err());
}

#[test]
fn corrected_hilda_declaration_does_not_accept_private_ordinal_code_proof() {
    let plan = resolve(request("hilda-land-use", [6.65, 0.2, 6.7, 0.3])).unwrap();
    let correct = provenance(&plan, &plan.sources[0]);
    assert_eq!(correct.layer.nodata, None);
    validate_provenance(&correct, href(&plan)).unwrap();
    let mut obsolete = correct;
    obsolete.layer.value_interpretation = "Annual reconstructed classes: 1 cities, 2 cropland, 3 pasture, 4 forest, 5 unmanaged grass/shrubland, 6 sparse/no vegetation. Original codes are retained; not a land-change probability, reflectance, current land-use survey or forecast. No NoData value is invented for this distribution.".into();
    assert_eq!(
        validate_provenance(&obsolete, href(&plan)).unwrap_err(),
        "Public dataset provenance differs from its reviewed preset"
    );
}

#[test]
fn query_response_preserves_original_connection_and_never_exposes_old_ids_as_new_results() {
    let connection: stac::Connection = serde_json::from_value(json!({"id":"existing","name":"Catalog","url":"https://data.inpe.br/bdc/stac/v1/","kind":"api","connectedAt":"2026-10-12T00:00:00Z","collections":[],"capabilities":{"searchGet":true,"searchPost":true},"snapshotIds":["old-area-old-month"],"searchUrl":"https://data.inpe.br/bdc/stac/v1/search","metadataSha256":[],"metadataDocuments":[]})).unwrap();
    let page = stac::SearchPage {
        items: vec![],
        next_cursor: None,
        complete: true,
        limit_reached: false,
        scanned_items: None,
    };
    let current = query_connection(&connection, &page);
    assert!(current.snapshot_ids.is_empty());
    assert_eq!(connection.snapshot_ids, ["old-area-old-month"]);
    let output = DatasetConnection {
        plan: resolve(request("cbers-sr", [116., 39., 117., 40.])).unwrap(),
        stac_connections: vec![current.clone()],
        wcs_connections: vec![],
        failures: vec![],
        complete: true,
        searches: vec![DatasetSearchResult {
            source_id: "CB4A-MUX-L4-SR-1".into(),
            connection_id: current.id,
            page,
        }],
    };
    let value = serde_json::to_value(output).unwrap();
    assert_eq!(value["complete"], true);
    assert_eq!(value["searches"][0]["page"]["complete"], true);
    assert_eq!(value["searches"][0]["page"]["items"], json!([]));
    assert_eq!(value["stacConnections"][0]["snapshotIds"], json!([]));
}
