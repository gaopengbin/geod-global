//! Two finite JAXA Earth December 2025 monthly COG distributions.
//! Original publisher JSON, relative asset hrefs, DN and quality codes remain
//! unchanged; these are JAXA's converted COGs, not original mission HDF files.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

const HTTPS: &str = "https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/";
const POLICY: &str = "https://earth.jaxa.jp/en/data/policy/";
const YEAR: u16 = 2025;
const MONTH: u8 = 12;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedItem {
    dataset_id: String,
    url: String,
    collection_url: String,
    document: Value,
}

static ITEMS: LazyLock<Vec<ReviewedItem>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("jaxa-reviewed-items.json"))
        .expect("compile-time actual JAXA December 2025 publisher items")
});

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedCollection {
    dataset_id: String,
    url: String,
    document: Value,
}

static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("jaxa-reviewed-collections.json"))
        .expect("compile-time actual JAXA Earth publisher collection documents")
});

fn product(id: &str) -> Result<(&'static str, &'static str, &'static str)> {
    match id {
        "jaxa-amsr2-soil-moisture" => Ok((
            "smc",
            "SMC",
            "JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SMC.nighttime.v3_global_monthly",
        )),
        "jaxa-sgli-lst" => Ok((
            "lst",
            "LST",
            "JAXA.G-Portal_GCOM-C.SGLI_standard.L3-LST.daytime.v3_global_monthly",
        )),
        _ => Err("Unknown reviewed JAXA Earth monthly product".into()),
    }
}

pub(super) fn definitions() -> Vec<Dataset> {
    let mut soil = base(
        "jaxa-amsr2-soil-moisture",
        "JAXA AMSR2 · Monthly soil moisture",
        "soil",
        "https://data.earth.jaxa.jp/en/",
    );
    soil.description = "GCOM-W / AMSR2 Version 3 nighttime monthly soil-moisture observations, distributed by JAXA Earth as complete numerical COG tiles. This is satellite-derived volumetric water content, not SoilGrids soil chemistry or a soil-depth profile.".into();
    soil.nominal_resolution = "JAXA Earth distributed 0.1° WGS84 grid · 900 × 900 per 90° tile; output spacing is not sensor footprint or local accuracy".into();
    soil.unit = "volumetric percent for valid stored DN".into();
    soil.value_interpretation = "Original Int16 DN are retained. Only published valid DN 0–600 represent volumetric soil moisture: DN × 0.1 gives percent. Embedded NoData is -32768; -32767 through -32761 are distinct original error codes, not moisture values or newly fabricated NoData. No automatic scientific conversion or masking of those error codes is applied. A fixed physical depth interval is not declared by these publisher items.".into();
    soil.layers = vec![scaled(
        layer(
            "smc",
            "Nighttime monthly soil moisture · original DN",
            "volumetric percent (valid DN × 0.1)",
            &soil.value_interpretation,
        ),
        0.1,
        0.,
        -32768.,
    )];
    soil.default_layer = Some("smc".into());
    soil.attribution = "GCOM-W / AMSR2 L3 Soil Moisture Content Version 3, Japan Aerospace Exploration Agency (JAXA), distributed through JAXA Earth API. DOI 10.57746/EO.01gs73b17zekqwyj4hh9xpqgy7. Retain the monthly period, nighttime measurement, publisher item and access date.".into();
    soil.warnings.push("The publisher describes volumetric water content over land, with limitations in vegetation-covered areas. Missing or negative error-coded pixels do not establish dry soil. Original error codes are preserved separately from scalar NoData; a raw grayscale preview can include those codes and does not establish scientific validity.".into());

    let mut lst = base(
        "jaxa-sgli-lst",
        "JAXA SGLI · Monthly land-surface temperature",
        "climate",
        "https://data.earth.jaxa.jp/en/",
    );
    lst.description = "GCOM-C / SGLI Version 3 daytime monthly land-surface temperature, distributed by JAXA Earth as complete numerical COG tiles. This is observed surface temperature, not two-metre air temperature, a climate normal or original mission HDF delivery.".into();
    lst.nominal_resolution = "JAXA Earth distributed 0.025° WGS84 grid · 3,600 × 3,600 per 90° tile; published source map is approximately 5 km".into();
    lst.unit = "Kelvin for valid stored DN".into();
    lst.value_interpretation = "Original UInt16 DN are retained. Published valid DN 11203–16860 × 0.019999999552965164 gives Kelvin, offset zero. Original scalar NoData/error code 65535 is retained and is not a temperature. No automatic scaling, Celsius conversion, reprojection or sensor-resolution claim is applied.".into();
    lst.layers = vec![scaled(
        layer(
            "lst",
            "Daytime monthly land-surface temperature · original DN",
            "Kelvin (valid DN × 0.019999999552965164)",
            &lst.value_interpretation,
        ),
        0.019999999552965164,
        0.,
        65535.,
    )];
    lst.default_layer = Some("lst".into());
    lst.attribution = "GCOM-C / SGLI L3 Map Land Surface Temperature Version 3, Japan Aerospace Exploration Agency (JAXA), distributed through JAXA Earth API. DOI 10.57746/EO.01gs73bfs5sbzhyxg4j9ytajsa. Retain the monthly period, daytime measurement, publisher item and access date.".into();
    lst.warnings.push("Clouds and observation gaps can leave original NoData. Published source observations are around local daytime; the JAXA Earth WGS84 COG is a converted distribution grid, not a promise of 0.025° independent sensor detail. Embedded overview resampling is MODE, and a preview is not a calibrated temperature map.".into());

    for dataset in [&mut soil, &mut lst] {
        dataset.version = "JAXA source v3 · Earth COG v1 · reviewed December 2025".into();
        dataset.license = "proprietary".into();
        dataset.license_url = POLICY.into();
        dataset.years = vec![YEAR];
        dataset.default_year = Some(YEAR);
        dataset.months = vec![MONTH];
        dataset.default_month = Some(MONTH);
        dataset.discovery = Some("stac-items".into());
        dataset.warnings.extend([
            "Only the actual December 2025, COG-level-1 monthly distribution is currently indexed: nighttime AMSR2 or daytime SGLI respectively. Other catalog dates, normal periods, day/night modes and pyramid levels are not substitutes. The selected map area determines intersecting 90° tiles; no demonstration region is inserted.".into(),
            "Files are the complete numerical GeoTIFFs formally converted and distributed by JAXA Earth, not original mission HDF products or client-generated subsets. Original DN, relative asset declarations, publisher JSON, scientific metadata and embedded masks remain unchanged.".into(),
            "Publisher items have datetime null and precise acquisition start/end timestamps. Those original intervals are retained: the SGLI December bin spans 2025-11-30 to 2026-01-01 in UTC. A monthly directory is not a single-scene acquisition or a strictly rounded UTC calendar interval.".into(),
            "The original license declaration is proprietary with JAXA Research Data terms. Those terms permit fee-free use with source credits; commercial use requires advance notice to JAXA. Gratis public delivery is not a Creative Commons license.".into(),
            "The original collection documents use the generic ID collection while items name the full dataset ID. That mismatch remains unchanged; the preset and linked terms are separate client declarations, not a fabricated verified publisher collection. These items declare no file checksum; local SHA-256 receipts and observed HTTP ETags are distinct from publisher file-checksum verification.".into(),
            "Standalone items have no self link. Revalidation uses the exact independently retrieved item URL. Relative TIFF paths already resolve against that public HTTPS directory; no S3 transport mapping or private-bucket access is granted.".into(),
        ]);
        dataset.limits = vec![
            "One finite published month and one fixed measurement per product; up to eight intersecting 90° geographic tiles. Outside, invalid or unsupported year/month/layer selections fail explicitly.".into(),
            "Complete original transfers retain the 512 MiB limit and current native TIFF, 8-billion-pixel and per-axis decoding limits. Published availability does not certify useful valid-area coverage or local scientific accuracy.".into(),
        ];
    }
    vec![soil, lst]
}

fn bbox(document: &Value) -> Result<[f64; 4]> {
    let bounds = serde_json::from_value(document["bbox"].clone()).map_err(crate::io_error)?;
    features::bounds(bounds)?;
    Ok(bounds)
}

fn source(item: &ReviewedItem) -> Result<DatasetSource> {
    let (layer, _, collection) = product(&item.dataset_id)?;
    let bounds = bbox(&item.document)?;
    let region = item
        .url
        .strip_prefix(&format!("{HTTPS}{collection}/2025-12/1/"))
        .and_then(|suffix| suffix.strip_suffix(".json"))
        .ok_or("JAXA item is outside its exact published December level-1 directory")?;
    let id = format!("{layer}-2025-12-{}", region.replace('/', "-"));
    let title = format!(
        "JAXA {} 2025-12 · {}",
        layer.to_uppercase(),
        region.replace('/', " · ")
    );
    Ok(DatasetSource {
        id,
        title: title.clone(),
        bounds: Some(bounds),
        stac: Some(stac::ConnectRequest {
            name: title,
            url: item.url.clone(),
            kind: "item".into(),
        }),
        wcs: None,
        coverage_id: None,
        search: None,
        region: Some(region.into()),
        season: None,
        start_datetime: item.document["properties"]["start_datetime"]
            .as_str()
            .map(str::to_owned),
        end_datetime: item.document["properties"]["end_datetime"]
            .as_str()
            .map(str::to_owned),
        catalog_raster: None,
    })
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    let (layer, _, _) = product(&request.dataset_id)?;
    if request.year != Some(YEAR)
        || request.month != Some(MONTH)
        || request.layer.as_deref() != Some(layer)
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the reviewed JAXA December 2025 monthly measurement; country, named region, season and other dates or day/night modes are not supported".into());
    }
    let mut selected = Vec::new();
    for item in ITEMS
        .iter()
        .filter(|item| item.dataset_id == request.dataset_id)
    {
        let tile = bbox(&item.document)?;
        // Half-open overlap avoids adding adjacent files at exact 0°/90° edges.
        if request.bounds[0] < tile[2]
            && request.bounds[2] > tile[0]
            && request.bounds[1] < tile[3]
            && request.bounds[3] > tile[1]
        {
            selected.push(source(item)?);
        }
    }
    if selected.is_empty() || selected.len() > MAX_TILES {
        return Err("The requested area has no reviewed JAXA month/level tiles or exceeds the eight-tile limit; no alternate period or region is substituted".into());
    }
    Ok(selected)
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    product(&proof.dataset.id)?;
    if proof.schema_version != 2 || proof.catalog_raster.is_some() {
        return Err("Unsupported JAXA publisher-item provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let source = plan
        .sources
        .iter()
        .find(|source| {
            source.id == proof.source_id
                && source.stac.as_ref().is_some_and(|source| source.url == url)
        })
        .ok_or(
            "JAXA source does not match the exact selected monthly product and geographic tile",
        )?;
    if super::provenance(&plan, source) != *proof {
        return Err(
            "JAXA product, layer, request or declaration differs from the immutable preset".into(),
        );
    }
    let reviewed = ITEMS
        .iter()
        .find(|item| item.dataset_id == proof.dataset.id && item.url == url)
        .ok_or("JAXA publisher item is outside the finite reviewed index")?;
    let (_, asset_key, collection) = product(&proof.dataset.id)?;
    let expected_collection_url = format!("{HTTPS}{collection}/collection.json");
    if reviewed.collection_url != expected_collection_url {
        return Err("JAXA reviewed collection endpoint differs from its fixed product".into());
    }
    if raw.is_some_and(|document| document != &reviewed.document) {
        return Err("JAXA publisher item changed: original interval, DN metadata, scientific unit, relative asset, roles, license or tile identity differs from the reviewed document".into());
    }
    if reviewed.document["assets"]
        .as_object()
        .is_none_or(|assets| assets.len() != 1 || !assets.contains_key(asset_key))
    {
        return Err("JAXA reviewed item has an unexpected measurement asset".into());
    }
    Ok(())
}

/// Recognize only the two independently reviewed publisher collection documents
/// whose generic ID conflicts with their actual item collection identifier.
/// This permits archiving an unchanged declaration, not attaching a verified
/// matching collection or inventing an alias for arbitrary STAC documents.
pub(crate) fn validate_collection(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<()> {
    let plan = super::resolve(proof.request.clone())?;
    let source_url = plan
        .sources
        .iter()
        .find(|source| source.id == proof.source_id)
        .and_then(|source| source.stac.as_ref())
        .map(|source| source.url.as_str())
        .ok_or("JAXA collection receipt has no exact selected publisher item")?;
    validate(proof, source_url, None)?;
    let (_, _, product_name) = product(&proof.dataset.id)?;
    let expected_url = format!("{HTTPS}{product_name}/collection.json");
    let reviewed = COLLECTIONS
        .iter()
        .find(|collection| collection.dataset_id == proof.dataset.id && collection.url == url)
        .ok_or("JAXA collection URL is outside the two exact reviewed publisher endpoints")?;
    if url != expected_url || raw != &reviewed.document {
        return Err("JAXA collection declaration changed: exact publisher identity, license, mode, scientific metadata or catalog contents differ from the archived review".into());
    }
    if raw["type"] != "Collection" || raw["id"] != "collection" || raw["license"] != "proprietary" {
        return Err("Unexpected JAXA publisher collection conflict declaration".into());
    }
    Ok(())
}

pub(crate) fn transports(
    proof: &SourceProvenance,
    url: &str,
    raw: &Value,
) -> Result<Vec<stac::AssetTransport>> {
    validate(proof, url, Some(raw))?;
    // The exact publisher-relative HTTP asset needs no transport exception.
    Ok(Vec::new())
}

pub(crate) fn self_url(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<String> {
    validate(proof, url, Some(raw))?;
    if raw["links"]
        .as_array()
        .is_none_or(|links| links.iter().any(|link| link["rel"] == "self"))
    {
        return Err(
            "JAXA indexed standalone publisher item unexpectedly declares a self link".into(),
        );
    }
    Ok(url.into())
}

#[cfg(test)]
pub(crate) fn reviewed_document(dataset_id: &str, bounds: [f64; 4]) -> (String, Value) {
    let item = ITEMS
        .iter()
        .find(|item| item.dataset_id == dataset_id && bbox(&item.document).unwrap() == bounds)
        .unwrap();
    (item.url.clone(), item.document.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(id: &str, layer: &str, bounds: [f64; 4]) -> ResolveRequest {
        serde_json::from_value(
            json!({"datasetId":id,"year":2025,"month":12,"layer":layer,"bounds":bounds}),
        )
        .unwrap()
    }

    #[test]
    fn exact_publisher_month_and_half_open_regions_resolve_without_replacing_aoi() {
        assert_eq!(ITEMS.len(), 16);
        for (id, layer) in [
            ("jaxa-amsr2-soil-moisture", "smc"),
            ("jaxa-sgli-lst", "lst"),
        ] {
            let input = request(id, layer, [116., 39., 117., 40.]);
            let saved = input.clone();
            let selected = resolve(&input).unwrap();
            assert_eq!(input, saved);
            assert_eq!(selected.len(), 1);
            assert_eq!(selected[0].bounds, Some([90., 0., 180., 90.]));
            assert!(selected[0].stac.as_ref().unwrap().name.len() <= 80);
            assert!(selected[0].catalog_raster.is_none());
            assert_eq!(
                resolve(&request(id, layer, [90., 0., 180., 90.]))
                    .unwrap()
                    .len(),
                1
            );
            assert_eq!(
                resolve(&request(id, layer, [89., -1., 91., 1.]))
                    .unwrap()
                    .len(),
                4
            );
            assert_eq!(
                resolve(&request(id, layer, [-180., -90., 180., 90.]))
                    .unwrap()
                    .len(),
                8
            );
            for input in [
                json!({"year":2024}),
                json!({"month":11}),
                json!({"layer":"other"}),
                json!({"country":"JPN"}),
                json!({"region":"demo"}),
                json!({"season":"summer"}),
            ] {
                let mut changed = serde_json::to_value(&saved).unwrap();
                changed
                    .as_object_mut()
                    .unwrap()
                    .extend(input.as_object().unwrap().clone());
                assert!(resolve(&serde_json::from_value(changed).unwrap()).is_err());
            }
            assert!(resolve(&request(id, layer, [170., 0., 190., 10.])).is_err());
        }
    }

    #[test]
    fn publisher_dn_error_license_dates_and_relative_assets_remain_bound_without_transport_mapping()
    {
        for (id, layer, asset) in [
            ("jaxa-amsr2-soil-moisture", "smc", "SMC"),
            ("jaxa-sgli-lst", "lst", "LST"),
        ] {
            let plan = super::super::resolve(request(id, layer, [116., 39., 117., 40.])).unwrap();
            let source = &plan.sources[0];
            let proof = super::super::provenance(&plan, source);
            let url = &source.stac.as_ref().unwrap().url;
            let raw = ITEMS
                .iter()
                .find(|item| &item.url == url)
                .unwrap()
                .document
                .clone();
            assert!(validate(&proof, url, Some(&raw)).is_ok());
            assert!(transports(&proof, url, &raw).unwrap().is_empty());
            assert_eq!(self_url(&proof, url, &raw).unwrap(), *url);
            assert_eq!(raw["properties"]["datetime"], Value::Null);
            assert_eq!(raw["properties"]["license"], "proprietary");
            let collection = COLLECTIONS
                .iter()
                .find(|entry| entry.dataset_id == id)
                .unwrap();
            assert!(validate_collection(&proof, &collection.url, &collection.document).is_ok());
            assert_ne!(raw["collection"], collection.document["id"]);
            assert!(validate_collection(&proof, url, &collection.document).is_err());
            for field in ["id", "license", "description", "summaries"] {
                let mut changed = collection.document.clone();
                changed[field] = json!("replacement");
                assert!(
                    validate_collection(&proof, &collection.url, &changed).is_err(),
                    "{field}"
                );
            }
            let other = COLLECTIONS
                .iter()
                .find(|entry| entry.dataset_id != id)
                .unwrap();
            assert!(validate_collection(&proof, &other.url, &other.document).is_err());
            for mutation in [
                "href",
                "role",
                "quality",
                "interval",
                "collection",
                "bbox",
                "self",
            ] {
                let mut changed = raw.clone();
                match mutation {
                    "href" => {
                        changed["assets"][asset]["href"] =
                            json!("https://example.com/replacement.tif")
                    }
                    "role" => changed["assets"][asset]["roles"] = json!(["overview"]),
                    "quality" => changed["assets"][asset]["je:rasters"]["dn"]["error"] = json!([]),
                    "interval" => {
                        changed["properties"]["start_datetime"] = json!("2024-12-01T00:00:00Z")
                    }
                    "collection" => changed["collection"] = json!("other-product"),
                    "bbox" => changed["bbox"] = json!([0., 0., 90., 90.]),
                    _ => changed["links"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"rel":"self","href":"s3://private/item.json"})),
                }
                assert!(validate(&proof, url, Some(&changed)).is_err(), "{mutation}");
            }
            let mut changed = proof.clone();
            changed.request.month = Some(11);
            assert!(validate(&changed, url, Some(&raw)).is_err());
            changed = proof.clone();
            changed.layer.scale = Some(1.);
            assert!(validate(&changed, url, Some(&raw)).is_err());
            assert!(validate(&proof, &url.replace("/1/", "/0/"), Some(&raw)).is_err());
        }
    }
}
