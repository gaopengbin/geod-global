//! Finite, unchanged JAXA Earth SGLI daytime sea-surface-temperature COGs.
//! These are publisher-converted level-1 numerical overviews, not original
//! mission HDF, optical RGB, or a guarantee of useful observations over land.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

const ID: &str = "jaxa-sgli-sst";
const PRODUCT: &str = "JAXA.G-Portal_GCOM-C.SGLI_standard.L3-SST.daytime.v3_global_monthly";
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
    serde_json::from_str(include_str!("jaxa-more-reviewed-items.json"))
        .expect("actual JAXA September 2026 monthly SGLI sea-surface temperature items")
});
static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("jaxa-more-reviewed-collections.json"))
        .expect("actual JAXA Earth SGLI monthly sea-surface temperature collection")
});

pub(super) fn definitions() -> Vec<Dataset> {
    let mut dataset = base(
        ID,
        "JAXA SGLI · Sea-surface temperature",
        "ocean",
        "https://earth.jaxa.jp/en/data/products/sea-surface-temperature/index.html",
    );
    dataset.description = "GCOM-C SGLI Version 3 daytime Level-3 sea-surface temperature from the September 2026 monthly release, distributed as complete JAXA Earth COG-level-1 numerical overview tiles. These thermal surface-water observations are not two-metre air temperature, land-surface temperature, water-column temperature, optical RGB or the original mission HDF delivery.".into();
    dataset.version =
        "SGLI standard SST v3 · September 2026 release · JAXA Earth COG v1 · level-1 overview"
            .into();
    dataset.nominal_resolution = "0.025° WGS84 publisher overview distribution grid · 3,600 × 3,600 per 90° tile; about 2.8 km spacing at the equator, not the underlying 250 m SGLI product or local scientific accuracy".into();
    dataset.unit = "°C sea-surface temperature for valid stored DN".into();
    dataset.value_interpretation = "Original UInt16 DN are retained. Exact publisher calibration: sea-surface temperature [degreeC] = DN × 0.0012000000569969416 − 10. Scalar NoData/error 65535 is retained and is not temperature; zero is not scalar NoData. The source mission documentation reserves 65532 retrieval error, 65533 cloud error and 65534 land, outside its 0–65531 valid-DN range. None of those three codes occur in the fully decoded NE original; this COG Item declares only 65535 and no separate QA asset. The NE Item's min/max 5726/36505 match the actual original's observed valid extrema, rather than universal SST quality limits. The TIFF embeds scale 1 and offset 0; physical calibration comes from unchanged STAC metadata. No automatic scaling, averaging, extra quality masking, clipping or reprojection is applied.".into();
    dataset.layers = vec![scaled(
        layer(
            "sst",
            "Daytime sea-surface temperature · original DN",
            "°C (valid DN × publisher slope − 10)",
            &dataset.value_interpretation,
        ),
        0.0012000000569969416,
        -10.,
        65535.,
    )];
    dataset.default_layer = Some("sst".into());
    dataset.years = vec![YEAR];
    dataset.default_year = Some(YEAR);
    dataset.months = vec![MONTH];
    dataset.default_month = Some(MONTH);
    dataset.discovery = Some("stac-items".into());
    dataset.license = "proprietary".into();
    dataset.license_url = "https://earth.jaxa.jp/en/data/policy/".into();
    dataset.attribution = "GCOM-C SGLI Level-3 Standard Sea Surface Temperature (Daytime/Monthly) Version 3; Japan Aerospace Exploration Agency (JAXA), Earth Observation Research Center (EORC). This data was obtained through JAXA Earth API. Credit JAXA and the SGLI SST research name; retain the exact original acquisition interval, calibration, source product, publisher Item and access date.".into();
    dataset.warnings = vec![
        "Only the actual September 2026 daytime SGLI SST COG-level-1 release is indexed. Nighttime, AMSR2, daily or half-monthly products, monthly normals, other months and pyramid levels are not substitutes. The selected area filters intersecting complete 90° tiles; no ocean demonstration area is inserted.".into(),
        "This is a monthly-labelled release, not a single acquisition or a guaranteed observation at every pixel for the whole month. The exact NE Item and TIFF interval is 2026-09-01T00:25:58Z through 2026-09-30T11:03:17Z; intervals differ by tile and remain unchanged. October creation dates are not October observations. No unverified temporal mean, maximum composite or midnight-to-month-end interval is asserted.".into(),
        "The official source mission can provide approximately 250 m SST; this adapter intentionally selects the much coarser level-1 0.025° numerical overview. Publisher je:img_level identifies overview and the TIFF embeds MODE resampling. Original stored values remain intact; grayscale preview is not a calibrated temperature palette or evidence of 250 m detail.".into(),
        "Clouds, land and observation gaps can be NoData. A non-NoData stored value does not prove an independently valid marine observation or useful coastal coverage; the complete original even contains non-NoData values in inland coordinates. No separate QA layer or ocean mask is supplied in these Items. Numeric range and geographic selection are not a scientific quality guarantee.".into(),
        "The exact publisher Collection uses generic ID collection while Items name the full SGLI SST product ID. The unchanged conflict is archived without inventing an alias or claiming a matching verified Collection. Standalone Items have no self link; exact independently retrieved URLs are used for revalidation.".into(),
        "Publisher Items declare proprietary research-data terms. The linked JAXA policy requires source and named third-party credits and advance notification for commercial use; public fee-free delivery is not a Creative Commons license. No commercial-use notification is sent by this adapter.".into(),
        "Items declare no file checksum. Local SHA-256 receipts and observed HTTP ETags are not publisher checksum verification. Relative TIFF paths resolve on public HTTPS without private-bucket access or an S3 transport exception.".into(),
    ];
    dataset.limits = vec![
        "One finite September 2026 publisher release, up to eight intersecting 90° geographic tiles. Unsupported year, month, layer, country, named region and season selections fail explicitly.".into(),
        "Complete original transfers retain the 512 MiB limit and current TIFF pixel-count/per-axis limits. Only the actual NE original has been independently transferred and fully decoded; the other seven indexed tiles retain exact publisher metadata without a per-file download acceptance claim. The AOI never crops or creates new temperature values.".into(),
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
        .ok_or("JAXA SST Item is outside its exact reviewed month and level")?;
    let title = format!("JAXA SST 2026-09 · {}", region.replace('/', " · "));
    Ok(DatasetSource {
        id: format!("sst-2026-09-{}", region.replace('/', "-")),
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
        || request.layer.as_deref() != Some("sst")
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the reviewed JAXA SGLI monthly sea-surface temperature for September 2026; no alternate product, date, region or season is substituted".into());
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
        return Err("No reviewed JAXA SST tile intersects this area or the area exceeds the eight-tile limit".into());
    }
    Ok(selected)
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    if proof.dataset.id != ID || proof.schema_version != 2 || proof.catalog_raster.is_some() {
        return Err("Unsupported JAXA SGLI SST publisher-Item provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let selected = plan
        .sources
        .iter()
        .find(|source| {
            source.id == proof.source_id
                && source.stac.as_ref().is_some_and(|source| source.url == url)
        })
        .ok_or("JAXA SST source does not match the exact selected monthly geographic tile")?;
    if super::provenance(&plan, selected) != *proof {
        return Err(
            "JAXA SST product, layer, request or declaration differs from its immutable preset"
                .into(),
        );
    }
    let reviewed = ITEMS
        .iter()
        .find(|item| item.dataset_id == ID && item.url == url)
        .ok_or("JAXA SST Item is outside the finite reviewed index")?;
    if reviewed.collection_url != format!("{HTTPS}{PRODUCT}/collection.json") {
        return Err("JAXA SST collection endpoint differs from the exact reviewed product".into());
    }
    if raw.is_some_and(|document| document != &reviewed.document) {
        return Err("JAXA SST publisher Item changed: original interval, DN scaling, error code, relative asset, roles, license or tile identity differs from review".into());
    }
    if reviewed.document["assets"]
        .as_object()
        .is_none_or(|assets| assets.len() != 1 || !assets.contains_key("SST"))
    {
        return Err("JAXA SST Item has an unexpected measurement asset".into());
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
        .ok_or("JAXA SST collection receipt has no exact selected publisher Item")?;
    validate(proof, selected_url, None)?;
    let reviewed = COLLECTIONS
        .iter()
        .find(|entry| entry.dataset_id == ID && entry.url == url)
        .ok_or("JAXA SST Collection URL is outside the exact reviewed endpoint")?;
    if url != format!("{HTTPS}{PRODUCT}/collection.json") || raw != &reviewed.document {
        return Err("JAXA SST Collection identity, license, scientific metadata or catalog contents changed since review".into());
    }
    if raw["type"] != "Collection" || raw["id"] != "collection" || raw["license"] != "proprietary" {
        return Err("Unexpected JAXA SST publisher Collection conflict declaration".into());
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
        return Err("Reviewed standalone JAXA SST Item unexpectedly has a self link".into());
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(tile: [f64; 4]) -> ResolveRequest {
        serde_json::from_value(
            json!({"datasetId":ID,"year":YEAR,"month":MONTH,"layer":"sst","bounds":tile}),
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
            Some("2026-09-01T00:25:58Z")
        );
        assert_eq!(
            selected[0].end_datetime.as_deref(),
            Some("2026-09-30T11:03:17Z")
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
        assert_eq!(proof.layer.scale, Some(0.0012000000569969416));
        assert_eq!(proof.layer.offset, Some(-10.));
        assert_eq!(proof.layer.nodata, Some(65535.));
        assert_eq!(proof.dataset.category, "ocean");
        assert_eq!(
            raw["assets"]["SST"]["je:rasters"]["dn"]["min"],
            json!(5726.)
        );
        assert_eq!(
            raw["assets"]["SST"]["je:rasters"]["dn"]["max"],
            json!(36505.)
        );
        assert_eq!(
            raw["assets"]["SST"]["je:rasters"]["value"]["unit"],
            "degreeC"
        );
        assert_eq!(raw["properties"]["je:img_level"]["level-1"], "overview");
        assert_eq!(
            raw["assets"]["SST"]["je:rasters"]["dn"]["error"],
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
                    changed["assets"]["SST"]["href"] = json!("https://example.com/replacement.tif")
                }
                "slope" => {
                    changed["assets"]["SST"]["je:rasters"]["dn2value"]["slope"] = json!(0.01)
                }
                "nodata" => changed["assets"]["SST"]["je:rasters"]["dn"]["nodata"] = json!(0),
                "license" => changed["properties"]["license"] = json!("CC-BY-4.0"),
                "interval" => {
                    changed["properties"]["start_datetime"] = json!("2026-10-01T00:00:00Z")
                }
                "collection" => changed["collection"] = json!("monthly-normal"),
                "bbox" => changed["bbox"] = json!([0., 0., 90., 90.]),
                "roles" => changed["assets"]["SST"]["roles"] = json!(["overview"]),
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
            &reviewed.url.replace("L3-SST.", "L3-CHLA."),
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
