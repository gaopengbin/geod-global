//! Finite unchanged JAXA Earth GSMaP precipitation-rate distribution.
//! The publisher calls this a monthly product, but this release's original
//! Item/TIFF interval covers only September 1. Keep that conflict visible.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

const ID: &str = "jaxa-gsmap-rainfall";
const PRODUCT: &str = "JAXA.EORC_GSMaP_standard.Gauge.00Z-23Z.v6_monthly";
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
    serde_json::from_str(include_str!("jaxa-rainfall-reviewed-items.json"))
        .expect("actual JAXA September 2026 GSMaP precipitation-rate Items")
});
static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("jaxa-rainfall-reviewed-collections.json"))
        .expect("actual JAXA Earth GSMaP monthly Collection")
});

pub(super) fn definitions() -> Vec<Dataset> {
    let mut dataset = base(
        ID,
        "JAXA GSMaP · Precipitation rate",
        "climate",
        "https://earth.jaxa.jp/en/data/products/precipitation/index.html",
    );
    dataset.description = "GSMaP standard Gauge Version 6 precipitation rate from JAXA Earth's September 2026 monthly release. The publisher describes GSMaP as multi-satellite precipitation mapping under the GPM mission, combining GPM and meteorological-satellite information. This adapter retains complete numerical COG-level-1 files. It is neither optical satellite imagery nor CHIRPS accumulated monthly precipitation.".into();
    dataset.version =
        "GSMaP standard Gauge v6 · JAXA Earth COG v1 · reviewed September 2026 release · level 1"
            .into();
    dataset.nominal_resolution = "0.1° WGS84 publisher distribution grid · 900 × 900 per 90° tile; about 11 km spacing at the equator, not local rainfall accuracy".into();
    dataset.unit = "mm/hr precipitation rate".into();
    dataset.value_interpretation = "Original Float32 values are already precipitation rates in mm/hr: publisher slope 1, offset 0. Zero is a valid zero rate. Publisher scalar NoData -999.9 is stored as Float32 -999.9000244140625 in the reviewed TIFF. Additional publisher error codes -4, -8 and -99 remain distinct from that scalar NoData and are not rainfall. No automatic integration to mm/month, unit conversion, masking, interpolation, clipping or reprojection is applied. The original Item and TIFF interval says 2026-09-01 00:00–23:00 UTC despite the monthly product/path; complete September temporal coverage is not verified.".into();
    dataset.layers = vec![scaled(
        layer(
            "precip",
            "Precipitation rate · original Float32 values",
            "mm/hr",
            &dataset.value_interpretation,
        ),
        1.,
        0.,
        -999.9,
    )];
    dataset.default_layer = Some("precip".into());
    dataset.years = vec![YEAR];
    dataset.default_year = Some(YEAR);
    dataset.months = vec![MONTH];
    dataset.default_month = Some(MONTH);
    dataset.discovery = Some("stac-items".into());
    dataset.license = "proprietary".into();
    dataset.license_url = "https://earth.jaxa.jp/en/data/policy/".into();
    dataset.attribution = "GSMaP standard Gauge Version 6, Japan Aerospace Exploration Agency (JAXA), Earth Observation Research Center (EORC), GSMaP Project. This data was obtained through JAXA Earth API. Retain the original product release, exact Item/TIFF acquisition interval, unit, metadata conflicts and access date.".into();
    dataset.warnings.extend([
        "Only the actual September 2026 release of the GSMaP standard Gauge v6 monthly product, COG level 1, is indexed. Daily/hourly/NRT products, monthly normals, GPM DPR swaths and other months or levels are not substitutes. The selected area chooses intersecting 90° published tiles; no demonstration area is inserted.".into(),
        "The publisher Collection title and YYYY-MM path identify a monthly precipitation-rate product, and the JAXA Earth overview describes GSMaP monthly averages. However, all eight archived Items and the independently decoded TIFF specify only 2026-09-01T00:00:00Z through 2026-09-01T23:00:00Z. This unchanged conflict is exposed rather than extending the interval to a full month. This file does not prove complete monthly temporal coverage or monthly rainfall totals.".into(),
        "The actual Item je:img_level declares level-2 raster and null levels 0/1, although its URL, parent catalog and Collection je:cog_level_max identify the level-1 distribution. Those publisher fields are preserved; they are not repaired or used to claim another resolution. The full reviewed NE TIFF is 900 × 900 Float32 at 0.1° on EPSG:4326.".into(),
        "Collection coverage is 60°S–60°N while its rectangular files extend to 90° latitude. Polar cells can be NoData. Public file availability is not valid observation coverage; satellite estimates are not a rain-gauge observation at every location. Raw grayscale preview is not a calibrated precipitation palette.".into(),
        "Publisher error codes -4, -8 and -99 must be excluded from scientific rainfall interpretation; they are not silently merged into the scalar NoData tag. Valid zero values are retained. Publisher MODE interpolation metadata remains original and does not prove area-averaged rainfall.".into(),
        "Publisher Items declare proprietary research-data terms with JAXA/GSMaP credit and advance notice for commercial use under the linked policy. Public fee-free delivery is not a Creative Commons license.".into(),
        "The exact publisher Collection uses generic ID collection while Items name the full GSMaP product ID. This conflict is archived without a fabricated alias or matching-collection assertion. Items declare no file checksum: a local SHA-256 receipt and matching observed HTTP ETag/MD5 are not publisher checksum verification.".into(),
        "Standalone Items have no self link. Revalidation uses the exact independently retrieved Item URL. Relative TIFF paths resolve on public HTTPS without a private-bucket credential or S3 transport exception.".into(),
    ]);
    dataset.limits = vec![
        "One finite September 2026 publisher release, up to eight intersecting 90° geographic tiles. Unsupported year, month, layer, country, named region or season selections fail explicitly.".into(),
        "Complete original transfers retain the 512 MiB limit and current native TIFF, pixel-count and per-axis decoding limits. Only the NE original has been independently decoded in this acceptance; other indexed tiles retain exact publisher documents, not a claim of per-tile transfer validation.".into(),
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
        .ok_or("JAXA GSMaP Item is outside its exact reviewed release and level")?;
    let title = format!("GSMaP 2026-09 · {}", region.replace('/', " · "));
    Ok(DatasetSource {
        id: format!("precip-2026-09-{}", region.replace('/', "-")),
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
        || request.layer.as_deref() != Some("precip")
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the reviewed JAXA GSMaP precipitation-rate release for September 2026; no alternate product, date, region or season is substituted".into());
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
        return Err("No reviewed JAXA GSMaP tile intersects this area or the area exceeds the eight-tile limit".into());
    }
    Ok(selected)
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    if proof.dataset.id != ID || proof.schema_version != 2 || proof.catalog_raster.is_some() {
        return Err("Unsupported JAXA GSMaP publisher-Item provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let selected = plan
        .sources
        .iter()
        .find(|source| {
            source.id == proof.source_id
                && source.stac.as_ref().is_some_and(|source| source.url == url)
        })
        .ok_or("JAXA GSMaP source does not match the exact selected release and geographic tile")?;
    if super::provenance(&plan, selected) != *proof {
        return Err(
            "JAXA GSMaP product, layer, request or declaration differs from its immutable preset"
                .into(),
        );
    }
    let reviewed = ITEMS
        .iter()
        .find(|item| item.dataset_id == ID && item.url == url)
        .ok_or("JAXA GSMaP Item is outside the finite reviewed index")?;
    if reviewed.collection_url != format!("{HTTPS}{PRODUCT}/collection.json") {
        return Err(
            "JAXA GSMaP collection endpoint differs from the exact reviewed product".into(),
        );
    }
    if raw.is_some_and(|document| document != &reviewed.document) {
        return Err("JAXA GSMaP publisher Item changed: original interval, rate unit, error codes, metadata conflicts, relative asset, roles, license or tile identity differs from review".into());
    }
    if reviewed.document["assets"]
        .as_object()
        .is_none_or(|assets| assets.len() != 1 || !assets.contains_key("PRECIP"))
        || reviewed.document["collection"] != PRODUCT
    {
        return Err(
            "JAXA GSMaP Item has an unexpected measurement asset or product identity".into(),
        );
    }
    Ok(())
}

pub(crate) fn validate_collection(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<()> {
    let plan = super::resolve(proof.request.clone())?;
    let selected_url = plan
        .sources
        .iter()
        .find(|source| source.id == proof.source_id)
        .and_then(|source| source.stac.as_ref())
        .map(|source| source.url.as_str())
        .ok_or("JAXA GSMaP collection receipt has no exact selected publisher Item")?;
    validate(proof, selected_url, None)?;
    let reviewed = COLLECTIONS
        .iter()
        .find(|entry| entry.dataset_id == ID && entry.url == url)
        .ok_or("JAXA GSMaP Collection URL is outside the exact reviewed endpoint")?;
    if url != format!("{HTTPS}{PRODUCT}/collection.json") || raw != &reviewed.document {
        return Err("JAXA GSMaP Collection identity, license, scientific metadata or catalog contents changed since review".into());
    }
    if raw["type"] != "Collection" || raw["id"] != "collection" || raw["license"] != "proprietary" {
        return Err("Unexpected JAXA GSMaP publisher Collection conflict declaration".into());
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
        return Err("Reviewed standalone JAXA GSMaP Item unexpectedly has a self link".into());
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(tile: [f64; 4]) -> ResolveRequest {
        serde_json::from_value(
            json!({"datasetId":ID,"year":YEAR,"month":MONTH,"layer":"precip","bounds":tile}),
        )
        .unwrap()
    }

    #[test]
    fn exact_release_retains_real_interval_and_half_open_aoi_without_substitution() {
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
            Some("2026-09-01T23:00:00.000000Z")
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
            json!({"datasetId":"chirps"}),
            json!({"year":2025}),
            json!({"month":8}),
            json!({"layer":"total"}),
            json!({"region":"demo"}),
            json!({"country":"JPN"}),
            json!({"season":"annual"}),
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
    fn original_float_rate_zero_nodata_errors_dates_and_publisher_conflicts_are_bound() {
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
        assert_eq!(proof.layer.scale, Some(1.));
        assert_eq!(proof.layer.offset, Some(0.));
        assert_eq!(proof.layer.nodata, Some(-999.9));
        assert_eq!(proof.dataset.category, "climate");
        assert_eq!(
            raw["assets"]["PRECIP"]["je:rasters"]["dn"]["data_type"],
            "float32"
        );
        assert_eq!(
            raw["assets"]["PRECIP"]["je:rasters"]["dn"]["min"],
            json!(0.)
        );
        assert_eq!(
            raw["assets"]["PRECIP"]["je:rasters"]["value"]["unit"],
            "mm/hr"
        );
        assert_eq!(
            raw["assets"]["PRECIP"]["je:rasters"]["dn"]["error"],
            json!([-4., -8., -99.])
        );
        assert_eq!(raw["properties"]["je:img_level"]["level-1"], Value::Null);
        assert_eq!(raw["properties"]["je:img_level"]["level-2"], "raster");
        assert!(((-999.9_f64 as f32) as f64 + 999.9000244140625).abs() < f64::EPSILON);
        for field in [
            "href",
            "slope",
            "nodata",
            "errors",
            "unit",
            "license",
            "interval",
            "collection",
            "bbox",
            "roles",
            "level",
            "self",
        ] {
            let mut changed = raw.clone();
            match field {
                "href" => {
                    changed["assets"]["PRECIP"]["href"] =
                        json!("https://example.com/replacement.tif")
                }
                "slope" => {
                    changed["assets"]["PRECIP"]["je:rasters"]["dn2value"]["slope"] = json!(720.)
                }
                "nodata" => changed["assets"]["PRECIP"]["je:rasters"]["dn"]["nodata"] = json!(0),
                "errors" => changed["assets"]["PRECIP"]["je:rasters"]["dn"]["error"] = json!([]),
                "unit" => {
                    changed["assets"]["PRECIP"]["je:rasters"]["value"]["unit"] = json!("mm/month")
                }
                "license" => changed["properties"]["license"] = json!("CC-BY-4.0"),
                "interval" => changed["properties"]["end_datetime"] = json!("2026-09-30T23:59:59Z"),
                "collection" => changed["collection"] = json!("monthly-normal"),
                "bbox" => changed["bbox"] = json!([0., 0., 90., 90.]),
                "roles" => changed["assets"]["PRECIP"]["roles"] = json!(["overview"]),
                "level" => changed["properties"]["je:img_level"]["level-1"] = json!("raster"),
                _ => changed["links"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"rel":"self","href":"s3://private/item.json"})),
            }
            assert!(validate(&proof, url, Some(&changed)).is_err(), "{field}");
        }
        let mut changed = proof.clone();
        changed.layer.nodata = Some(0.);
        assert!(validate(&changed, url, Some(&raw)).is_err());
        assert!(validate(&proof, &url.replace("/1/", "/0/"), Some(&raw)).is_err());
    }

    #[test]
    fn exact_collection_conflict_keeps_identity_license_and_metadata_unchanged() {
        let plan = super::super::resolve(request([116., 39., 117., 40.])).unwrap();
        let proof = super::super::provenance(&plan, &plan.sources[0]);
        let reviewed = &COLLECTIONS[0];
        assert!(validate_collection(&proof, &reviewed.url, &reviewed.document).is_ok());
        assert_ne!(ITEMS[0].document["collection"], reviewed.document["id"]);
        assert_eq!(reviewed.document["summaries"]["je:ppu_max"], json!([10]));
        assert!(validate_collection(
            &proof,
            &reviewed.url.replace("_monthly/", "_daily/"),
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
