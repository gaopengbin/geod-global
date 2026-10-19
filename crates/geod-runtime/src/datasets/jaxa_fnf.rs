//! Finite unchanged JAXA Earth PALSAR-2 annual forest/non-forest classification.
//! This is the publisher's 0.025-degree COG-level-1 distribution, not the
//! original 25 m forest-map delivery or unclassified radar backscatter.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

const ID: &str = "jaxa-palsar-fnf";
const PRODUCT: &str = "JAXA.EORC_ALOS-2.PALSAR-2_FNF.v2.1.0_global_yearly";
const HTTPS: &str = "https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/";
const YEAR: u16 = 2020;

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
    serde_json::from_str(include_str!("jaxa-fnf-reviewed-items.json"))
        .expect("actual JAXA 2020 PALSAR-2 forest/non-forest level-1 Items")
});
static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("jaxa-fnf-reviewed-collections.json"))
        .expect("actual JAXA Earth PALSAR-2 forest/non-forest Collection")
});

pub(super) fn definitions() -> Vec<Dataset> {
    let mut dataset = base(
        ID,
        "JAXA PALSAR-2 · Forest / non-forest",
        "vegetation",
        "https://www.eorc.jaxa.jp/ALOS/en/dataset/fnf_e.htm",
    );
    dataset.description = "JAXA ALOS-2 PALSAR-2 annual forest/non-forest Version 2.1.0 classification, delivered as complete JAXA Earth COG-level-1 geographical tiles. Four classified categories distinguish two forest canopy-cover classes, non-forest and water. This is a derived Random Forest classification, not optical imagery, radar backscatter or per-pixel canopy-cover percentage. The 2020 classification uses current-year and previous-year radar mosaics where available; the annual interval is not one acquisition date.".into();
    dataset.version = "PALSAR-2 FNF v2.1.0 · JAXA Earth COG v1 · reviewed 2020 level 1".into();
    dataset.nominal_resolution = "0.025° WGS84 distribution grid · 3,600 × 3,600 per 90° tile; about 2.8 km spacing at the equator, not the original 25 m delivery or local classification accuracy".into();
    dataset.unit = "forest/non-forest class code".into();
    dataset.value_interpretation = "Original UInt8 class codes are retained: 0 No Data; 1 Forest (publisher legend: x > 90%); 2 Forest (publisher legend: 10% < x < 90%); 3 Non forest; 4 Water. These are category labels, not continuous canopy percentages or radar intensity. The actual publisher asset declares slope 1, offset 0, valid codes 1–4 and NoData 0. No automatic averaging, class merging, reprojection, clipping or conversion to a continuous vegetation index is applied. Publisher class descriptions and color hints remain unchanged.".into();
    dataset.layers = vec![scaled(
        layer(
            "fnf",
            "Annual forest / non-forest · original class codes",
            "class code",
            &dataset.value_interpretation,
        ),
        1.,
        0.,
        0.,
    )];
    dataset.default_layer = Some("fnf".into());
    dataset.years = vec![YEAR];
    dataset.default_year = Some(YEAR);
    dataset.discovery = Some("stac-items".into());
    dataset.license = "proprietary".into();
    dataset.license_url = "https://earth.jaxa.jp/en/data/policy/".into();
    dataset.attribution = "©JAXA. JAXA Global PALSAR-2 Forest/Non-Forest Map, Version 2.1.0, Japan Aerospace Exploration Agency (JAXA), Earth Observation Research Center (EORC), ALOS-2/PALSAR-2. This data was obtained through JAXA Earth API. Retain the original classification year, product version, publisher Item, class legend and access date.".into();
    dataset.warnings.extend([
        "Only the actual 2020 COG-level-1 forest/non-forest classification is indexed. The publisher Collection also lists 2017–2019 and other pyramid levels, but they are not substituted or advertised as reviewed downloads. The selected area chooses intersecting 90° tiles; no demonstration area is inserted.".into(),
        "The downloaded numerical GeoTIFF is JAXA Earth's converted 0.025° distribution, not the mission's 25 m FNF tiles. The actual TIFF is 3,600 × 3,600 UInt8 on EPSG:4326. Embedded overviews use MODE; a raw grayscale preview is not a calibrated thematic class map.".into(),
        "The annual classification is derived from radar mosaics with earlier-year input where available and water/urban masks. It does not mean every pixel was acquired on one date or that forest differences are verified deforestation. Preserve Version 2 class definitions; older three-class FNF products use different class codes and must not be silently mixed.".into(),
        "Publisher Items declare proprietary research-data terms. Public delivery is fee-free under JAXA's policy with source credit and advance notification for commercial use; it is not a Creative Commons license. Display ©JAXA when publishing an unchanged dataset image.".into(),
        "The unchanged publisher Item and Collection both identify the full product ID. Items declare no file checksum: a locally computed SHA-256 receipt and observed HTTP ETag are not publisher checksum verification. Standalone Items have no self link; revalidation uses the exact independently retrieved Item URL. Relative TIFF paths resolve on public HTTPS without granting private-bucket access or an S3 transport exception.".into(),
    ]);
    dataset.limits = vec![
        "One finite published classification year, up to eight intersecting 90° geographic tiles. Unsupported year, month, layer, country, named region or season selections fail explicitly.".into(),
        "Complete original transfers retain the 512 MiB limit and native TIFF, pixel-count and per-axis decoding limits. Published availability is not proof of useful valid-area coverage or local classification accuracy.".into(),
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
        .strip_prefix(&format!("{HTTPS}{PRODUCT}/2020/1/"))
        .and_then(|suffix| suffix.strip_suffix(".json"))
        .ok_or("JAXA FNF Item is outside its exact reviewed year and level")?;
    let title = format!("JAXA FNF 2020 · {}", region.replace('/', " · "));
    Ok(DatasetSource {
        id: format!("fnf-2020-{}", region.replace('/', "-")),
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
        || request.month.is_some()
        || request.layer.as_deref() != Some("fnf")
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the reviewed JAXA PALSAR-2 forest/non-forest classification for 2020; no alternate product, year, month, region or season is substituted".into());
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
        return Err("No reviewed JAXA FNF tile intersects this area or the area exceeds the eight-tile limit".into());
    }
    Ok(selected)
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    if proof.dataset.id != ID || proof.schema_version != 2 || proof.catalog_raster.is_some() {
        return Err("Unsupported JAXA PALSAR-2 FNF publisher-Item provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let selected = plan
        .sources
        .iter()
        .find(|source| {
            source.id == proof.source_id
                && source.stac.as_ref().is_some_and(|source| source.url == url)
        })
        .ok_or("JAXA FNF source does not match the exact selected annual geographic tile")?;
    if super::provenance(&plan, selected) != *proof {
        return Err(
            "JAXA FNF product, layer, request or declaration differs from its immutable preset"
                .into(),
        );
    }
    let reviewed = ITEMS
        .iter()
        .find(|item| item.dataset_id == ID && item.url == url)
        .ok_or("JAXA FNF Item is outside the finite reviewed index")?;
    if reviewed.collection_url != format!("{HTTPS}{PRODUCT}/collection.json") {
        return Err("JAXA FNF collection endpoint differs from the exact reviewed product".into());
    }
    if raw.is_some_and(|document| document != &reviewed.document) {
        return Err("JAXA FNF publisher Item changed: original annual interval, class legend, NoData, relative asset, roles, license or tile identity differs from review".into());
    }
    if reviewed.document["assets"]
        .as_object()
        .is_none_or(|assets| assets.len() != 1 || !assets.contains_key("FNF"))
        || reviewed.document["collection"] != PRODUCT
    {
        return Err(
            "JAXA FNF Item has an unexpected classification asset or product identity".into(),
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
        .ok_or("JAXA FNF collection receipt has no exact selected publisher Item")?;
    validate(proof, selected_url, None)?;
    let reviewed = COLLECTIONS
        .iter()
        .find(|entry| entry.dataset_id == ID && entry.url == url)
        .ok_or("JAXA FNF Collection URL is outside the exact reviewed endpoint")?;
    if url != format!("{HTTPS}{PRODUCT}/collection.json") || raw != &reviewed.document {
        return Err("JAXA FNF Collection identity, license, scientific metadata or catalog contents changed since review".into());
    }
    if raw["type"] != "Collection" || raw["id"] != PRODUCT || raw["license"] != "proprietary" {
        return Err("Unexpected JAXA FNF publisher Collection identity or license".into());
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
        return Err("Reviewed standalone JAXA FNF Item unexpectedly has a self link".into());
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(tile: [f64; 4]) -> ResolveRequest {
        serde_json::from_value(json!({"datasetId":ID,"year":YEAR,"layer":"fnf","bounds":tile}))
            .unwrap()
    }

    #[test]
    fn exact_year_half_open_tiles_preserve_aoi_and_reject_unreviewed_selections() {
        assert_eq!(ITEMS.len(), 8);
        let input = request([116., 39., 117., 40.]);
        let before = input.clone();
        let selected = resolve(&input).unwrap();
        assert_eq!(input, before);
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].bounds, Some([90., 0., 180., 90.]));
        assert_eq!(
            selected[0].start_datetime.as_deref(),
            Some("2020-01-01T00:00:00Z")
        );
        assert_eq!(
            selected[0].end_datetime.as_deref(),
            Some("2020-12-31T23:59:59Z")
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
            json!({"datasetId":"jaxa-modis-ndvi"}),
            json!({"year":2019}),
            json!({"month":9}),
            json!({"layer":"backscatter"}),
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
    fn immutable_provenance_binds_class_legend_date_asset_and_original_codes() {
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
        assert_eq!(proof.layer.nodata, Some(0.));
        assert_eq!(
            raw["assets"]["FNF"]["je:rasters"]["dn"]["data_type"],
            "uint8"
        );
        assert_eq!(
            raw["assets"]["FNF"]["classification:classes"]
                .as_array()
                .unwrap()
                .len(),
            5
        );
        for field in [
            "href",
            "legend",
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
                    changed["assets"]["FNF"]["href"] = json!("https://example.com/replacement.tif")
                }
                "legend" => {
                    changed["assets"]["FNF"]["classification:classes"][1]["value"] = json!(4)
                }
                "nodata" => changed["assets"]["FNF"]["je:rasters"]["dn"]["nodata"] = json!(255),
                "license" => changed["properties"]["license"] = json!("CC-BY-4.0"),
                "interval" => changed["properties"]["end_datetime"] = json!("2021-12-31T23:59:59Z"),
                "collection" => changed["collection"] = json!("PALSAR-old-three-class"),
                "bbox" => changed["bbox"] = json!([0., 0., 90., 90.]),
                "roles" => changed["assets"]["FNF"]["roles"] = json!(["overview"]),
                _ => changed["links"]
                    .as_array_mut()
                    .unwrap()
                    .push(json!({"rel":"self","href":"s3://private/item.json"})),
            }
            assert!(validate(&proof, url, Some(&changed)).is_err(), "{field}");
        }
        let mut changed = proof.clone();
        changed.layer.nodata = Some(255.);
        assert!(validate(&changed, url, Some(&raw)).is_err());
        assert!(validate(&proof, &url.replace("/1/", "/0/"), Some(&raw)).is_err());
    }

    #[test]
    fn collection_uses_exact_matching_product_without_ndvi_alias_exception() {
        let plan = super::super::resolve(request([116., 39., 117., 40.])).unwrap();
        let proof = super::super::provenance(&plan, &plan.sources[0]);
        let reviewed = &COLLECTIONS[0];
        assert!(validate_collection(&proof, &reviewed.url, &reviewed.document).is_ok());
        assert_eq!(ITEMS[0].document["collection"], reviewed.document["id"]);
        assert!(validate_collection(
            &proof,
            &reviewed.url.replace("_FNF.", "_ndvi."),
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
        let mut changed = reviewed.document.clone();
        changed["id"] = json!("collection");
        assert!(validate_collection(&proof, &reviewed.url, &changed).is_err());
    }
}
