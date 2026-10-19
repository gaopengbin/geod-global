//! Finite, unchanged JAXA Earth JASMES Terra+Aqua MODIS NDVI monthly COGs.
//! These are the publisher's converted numerical distribution, not mission HDF
//! files, monthly averages, or monthly-normal climate products.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

const ID: &str = "jaxa-modis-ndvi";
const PRODUCT: &str = "JAXA.JASMES_Terra.MODIS-Aqua.MODIS_ndvi.v811_global_monthly";
const HTTPS: &str = "https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/";
const YEAR: u16 = 2026;
const MONTH: u8 = 9;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedItem {
    dataset_id: String,
    url: String,
    collection_url: String,
    document: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedCollection {
    dataset_id: String,
    url: String,
    document: Value,
}

static ITEMS: LazyLock<Vec<ReviewedItem>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("jaxa-ndvi-reviewed-items.json"))
        .expect("actual JAXA September 2026 monthly MODIS NDVI items")
});
static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("jaxa-ndvi-reviewed-collections.json"))
        .expect("actual JAXA Earth JASMES monthly MODIS NDVI collection")
});

pub(super) fn definitions() -> Vec<Dataset> {
    let mut dataset = base(
        ID,
        "JAXA MODIS · Monthly vegetation index",
        "vegetation",
        "https://data.earth.jaxa.jp/en/",
    );
    dataset.description = "JAXA JASMES Terra+Aqua MODIS Version 811 monthly normalized difference vegetation index, distributed as complete JAXA Earth numerical COG tiles. The underlying monthly product is a maximum-value composite of daily daytime NDVI during the month; it is not a monthly mean, a single-scene acquisition, or a monthly-normal climate statistic.".into();
    dataset.version = "JASMES MODIS v811 · JAXA Earth COG v1 · reviewed September 2026".into();
    dataset.nominal_resolution = "0.05° WGS84 distribution grid · approximately 5 km source grid · 1,800 × 1,800 per 90° tile; spacing is not local accuracy".into();
    dataset.unit = "dimensionless NDVI for valid stored DN".into();
    dataset.value_interpretation = "Original UInt16 DN are retained. The actual publisher items declare valid DN 2–9966: NDVI = DN × 0.0001, offset zero. Original scalar NoData/error code 65535 is retained and is not an NDVI value. No automatic numerical scaling, masking, reprojection, or conversion to vegetation cover percentage is applied. The original TIFF stores scale 1 and offset 0; its physical interpretation comes from the archived publisher STAC metadata.".into();
    dataset.layers = vec![scaled(
        layer(
            "ndvi",
            "Monthly maximum-value composite NDVI · original DN",
            "dimensionless NDVI (valid DN × 0.0001)",
            &dataset.value_interpretation,
        ),
        0.0001,
        0.,
        65535.,
    )];
    dataset.default_layer = Some("ndvi".into());
    dataset.years = vec![YEAR];
    dataset.default_year = Some(YEAR);
    dataset.months = vec![MONTH];
    dataset.default_month = Some(MONTH);
    dataset.discovery = Some("stac-items".into());
    dataset.license = "proprietary".into();
    dataset.license_url = "https://earth.jaxa.jp/en/data/policy/".into();
    dataset.attribution = "JAXA Satellite Monitoring for Environmental Studies (JASMES), Terra/Aqua MODIS NDVI Version 811, Japan Aerospace Exploration Agency (JAXA) Earth Observation Research Center (EORC), using NASA Terra/Aqua MODIS observations. Credit JAXA/NASA. This data was obtained through JAXA Earth API. Retain the original monthly acquisition interval, product, publisher item and access date.".into();
    dataset.warnings.extend([
        "Only the actual September 2026 COG-level-1 monthly NDVI distribution is indexed. Other dates, half-month products, monthly-normal statistics, SGLI data and pyramid levels are not substitutes. The selected area determines intersecting 90° tiles; no demonstration area is inserted.".into(),
        "These are JAXA Earth converted numerical GeoTIFFs, not original MODIS mission HDF delivery or client-generated subsets. Every original DN, relative asset declaration, acquisition timestamp, license and scientific metadata is retained unchanged. The original acquisition interval is 2026-09-01T00:00:00.000000Z through 2026-09-30T23:59:59.999999Z; creation in October is not an October observation.".into(),
        "Clouds and observation gaps can leave NoData. NDVI is an index, not vegetation area percentage, biomass or a guarantee of healthy vegetation. Embedded overview resampling is MODE; a raw grayscale preview is not a calibrated thematic vegetation map.".into(),
        "Publisher items declare proprietary research-data terms, with source credits and advance notice for commercial use under the JAXA policy. Fee-free public delivery does not create a Creative Commons license.".into(),
        "The exact publisher Collection document uses generic ID collection while Items name the complete MODIS NDVI product ID. The unchanged conflict is archived without fabricating an alias or claiming a matching verified collection. Items declare no file checksum: local SHA-256 receipts and observed HTTP ETags remain distinct from publisher checksum verification.".into(),
        "Standalone Items have no self link. Revalidation uses each exact independently retrieved Item URL. Their relative TIFF paths resolve on public HTTPS; no private-bucket access or S3 transport exception is granted.".into(),
    ]);
    dataset.limits = vec![
        "One finite published monthly measurement, up to eight intersecting 90° geographic tiles. Unsupported year, month, layer, country, named region or season selections fail explicitly.".into(),
        "Complete original transfers retain the 512 MiB limit and current native TIFF, pixel-count and per-axis decoding limits. Published availability is not proof of useful valid-area coverage or local scientific accuracy.".into(),
    ];
    vec![dataset]
}

fn bounds(document: &Value) -> Result<[f64; 4]> {
    let value = serde_json::from_value(document["bbox"].clone()).map_err(crate::io_error)?;
    features::bounds(value)?;
    Ok(value)
}

fn source(item: &ReviewedItem) -> Result<DatasetSource> {
    let region = item
        .url
        .strip_prefix(&format!("{HTTPS}{PRODUCT}/2026-09/1/"))
        .and_then(|suffix| suffix.strip_suffix(".json"))
        .ok_or("JAXA NDVI Item is outside its exact reviewed month and level")?;
    let title = format!("JAXA NDVI 2026-09 · {}", region.replace('/', " · "));
    Ok(DatasetSource {
        id: format!("ndvi-2026-09-{}", region.replace('/', "-")),
        title: title.clone(),
        bounds: Some(bounds(&item.document)?),
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
    if request.dataset_id != ID
        || request.year != Some(YEAR)
        || request.month != Some(MONTH)
        || request.layer.as_deref() != Some("ndvi")
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the reviewed JAXA MODIS monthly NDVI for September 2026; no alternate product, date, region or season is substituted".into());
    }
    let mut selected = Vec::new();
    for item in ITEMS.iter().filter(|item| item.dataset_id == ID) {
        let tile = bounds(&item.document)?;
        if request.bounds[0] < tile[2]
            && request.bounds[2] > tile[0]
            && request.bounds[1] < tile[3]
            && request.bounds[3] > tile[1]
        {
            selected.push(source(item)?);
        }
    }
    if selected.is_empty() || selected.len() > MAX_TILES {
        return Err("No reviewed JAXA NDVI tile intersects this area or the area exceeds the eight-tile limit".into());
    }
    Ok(selected)
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    if proof.dataset.id != ID || proof.schema_version != 2 || proof.catalog_raster.is_some() {
        return Err("Unsupported JAXA MODIS NDVI publisher-Item provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let selected = plan
        .sources
        .iter()
        .find(|source| {
            source.id == proof.source_id
                && source.stac.as_ref().is_some_and(|source| source.url == url)
        })
        .ok_or("JAXA NDVI source does not match the exact selected monthly geographic tile")?;
    if super::provenance(&plan, selected) != *proof {
        return Err(
            "JAXA NDVI product, layer, request or declaration differs from its immutable preset"
                .into(),
        );
    }
    let reviewed = ITEMS
        .iter()
        .find(|item| item.dataset_id == ID && item.url == url)
        .ok_or("JAXA NDVI Item is outside the finite reviewed index")?;
    if reviewed.collection_url != format!("{HTTPS}{PRODUCT}/collection.json") {
        return Err("JAXA NDVI collection endpoint differs from the exact reviewed product".into());
    }
    if raw.is_some_and(|document| document != &reviewed.document) {
        return Err("JAXA NDVI publisher Item changed: original interval, DN scaling, error code, relative asset, roles, license or tile identity differs from review".into());
    }
    if reviewed.document["assets"]
        .as_object()
        .is_none_or(|assets| assets.len() != 1 || !assets.contains_key("ndvi"))
    {
        return Err("JAXA NDVI Item has an unexpected measurement asset".into());
    }
    Ok(())
}

/// Archive only this exact conflicting publisher Collection, without adding
/// an ID alias or a matching-collection verification assertion.
pub(crate) fn validate_collection(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<()> {
    let plan = super::resolve(proof.request.clone())?;
    let selected_url = plan
        .sources
        .iter()
        .find(|source| source.id == proof.source_id)
        .and_then(|source| source.stac.as_ref())
        .map(|source| source.url.as_str())
        .ok_or("JAXA NDVI collection receipt has no exact selected publisher Item")?;
    validate(proof, selected_url, None)?;
    let reviewed = COLLECTIONS
        .iter()
        .find(|entry| entry.dataset_id == ID && entry.url == url)
        .ok_or("JAXA NDVI Collection URL is outside the exact reviewed endpoint")?;
    if url != format!("{HTTPS}{PRODUCT}/collection.json") || raw != &reviewed.document {
        return Err("JAXA NDVI Collection identity, license, scientific metadata or catalog contents changed since review".into());
    }
    if raw["type"] != "Collection" || raw["id"] != "collection" || raw["license"] != "proprietary" {
        return Err("Unexpected JAXA NDVI publisher Collection conflict declaration".into());
    }
    Ok(())
}

pub(crate) fn transports(
    proof: &SourceProvenance,
    url: &str,
    raw: &Value,
) -> Result<Vec<stac::AssetTransport>> {
    validate(proof, url, Some(raw))?;
    Ok(Vec::new())
}

pub(crate) fn self_url(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<String> {
    validate(proof, url, Some(raw))?;
    if raw["links"]
        .as_array()
        .is_none_or(|links| links.iter().any(|link| link["rel"] == "self"))
    {
        return Err("Reviewed standalone JAXA NDVI Item unexpectedly has a self link".into());
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(tile: [f64; 4]) -> ResolveRequest {
        serde_json::from_value(
            json!({"datasetId":ID,"year":YEAR,"month":MONTH,"layer":"ndvi","bounds":tile}),
        )
        .unwrap()
    }

    #[test]
    fn exact_month_half_open_tiles_preserve_aoi_and_reject_product_substitutions() {
        assert_eq!(ITEMS.len(), 8);
        let input = request([116., 39., 117., 40.]);
        let before = input.clone();
        let selected = resolve(&input).unwrap();
        assert_eq!(input, before);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].bounds, Some([90., 0., 180., 90.]));
        assert_eq!(
            selected[0].start_datetime.as_deref(),
            Some("2026-09-01T00:00:00.000000Z")
        );
        assert_eq!(
            selected[0].end_datetime.as_deref(),
            Some("2026-09-30T23:59:59.999999Z")
        );
        assert!(selected[0].stac.as_ref().unwrap().name.len() <= 80);
        assert!(selected[0].catalog_raster.is_none());
        assert_eq!(resolve(&request([90., 0., 180., 90.])).unwrap().len(), 1);
        assert_eq!(resolve(&request([89., -1., 91., 1.])).unwrap().len(), 4);
        assert_eq!(
            resolve(&request([-180., -90., 180., 90.])).unwrap().len(),
            8
        );
        assert!(resolve(&request([170., 0., 190., 10.])).is_err());
        for mutation in [
            json!({"datasetId":"jaxa-sgli-lst"}),
            json!({"year":2025}),
            json!({"month":8}),
            json!({"layer":"mean"}),
            json!({"region":"demo"}),
            json!({"country":"JPN"}),
            json!({"season":"summer"}),
        ] {
            let mut changed = serde_json::to_value(&before).unwrap();
            changed
                .as_object_mut()
                .unwrap()
                .extend(mutation.as_object().unwrap().clone());
            assert!(resolve(&serde_json::from_value(changed).unwrap()).is_err());
        }
    }

    #[test]
    fn precise_scientific_date_license_and_asset_values_are_bound_to_immutable_provenance() {
        let plan = super::super::resolve(request([116., 39., 117., 40.])).unwrap();
        let selected = &plan.sources[0];
        let proof = super::super::provenance(&plan, selected);
        let url = &selected.stac.as_ref().unwrap().url;
        let raw = ITEMS
            .iter()
            .find(|item| &item.url == url)
            .unwrap()
            .document
            .clone();
        assert!(validate(&proof, url, Some(&raw)).is_ok());
        assert_eq!(self_url(&proof, url, &raw).unwrap(), *url);
        assert!(transports(&proof, url, &raw).unwrap().is_empty());
        assert_eq!(raw["properties"]["datetime"], Value::Null);
        assert_eq!(proof.layer.scale, Some(0.0001));
        assert_eq!(proof.layer.nodata, Some(65535.));
        assert_eq!(
            raw["assets"]["ndvi"]["je:rasters"]["dn"]["error"],
            json!([65535.])
        );
        for field in [
            "href",
            "slope",
            "nodata",
            "license",
            "interval",
            "collection",
            "bbox",
            "roles",
            "self",
        ] {
            let mut changed = raw.clone();
            match field {
                "href" => {
                    changed["assets"]["ndvi"]["href"] = json!("https://example.com/replacement.tif")
                }
                "slope" => {
                    changed["assets"]["ndvi"]["je:rasters"]["dn2value"]["slope"] = json!(0.01)
                }
                "nodata" => changed["assets"]["ndvi"]["je:rasters"]["dn"]["nodata"] = json!(0),
                "license" => changed["properties"]["license"] = json!("CC-BY-4.0"),
                "interval" => {
                    changed["properties"]["start_datetime"] = json!("2026-10-01T00:00:00Z")
                }
                "collection" => changed["collection"] = json!("monthly-normal"),
                "bbox" => changed["bbox"] = json!([0., 0., 90., 90.]),
                "roles" => changed["assets"]["ndvi"]["roles"] = json!(["overview"]),
                _ => changed["links"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"rel":"self","href":"s3://private/item.json"})),
            }
            assert!(validate(&proof, url, Some(&changed)).is_err(), "{field}");
        }
        let mut changed = proof.clone();
        changed.layer.scale = Some(1.);
        assert!(validate(&changed, url, Some(&raw)).is_err());
        assert!(validate(&proof, &url.replace("/1/", "/0/"), Some(&raw)).is_err());
    }

    #[test]
    fn exact_collection_conflict_is_archived_without_aliasing_or_relaxing_other_collections() {
        let plan = super::super::resolve(request([116., 39., 117., 40.])).unwrap();
        let proof = super::super::provenance(&plan, &plan.sources[0]);
        let reviewed = &COLLECTIONS[0];
        assert!(validate_collection(&proof, &reviewed.url, &reviewed.document).is_ok());
        assert_ne!(ITEMS[0].document["collection"], reviewed.document["id"]);
        assert!(validate_collection(
            &proof,
            &reviewed.url.replace("_ndvi.", "_taua."),
            &reviewed.document
        )
        .is_err());
        for field in ["id", "license", "description", "summaries", "links"] {
            let mut changed = reviewed.document.clone();
            changed[field] = json!("replacement");
            assert!(
                validate_collection(&proof, &reviewed.url, &changed).is_err(),
                "{field}"
            );
        }
    }
}
