//! Two finite unchanged JAXA numerical distributions. HRLULC is the publisher's
//! coarse overview, not the 10 m delivery. SWR retains its short actual interval.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

const HR: &str = "jaxa-hrlulc-japan";
const SWR: &str = "jaxa-modis-swr";
const HTTPS: &str = "https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/";
const HR_PRODUCT: &str = "JAXA.EORC_ALOS_HRLULC.v25.04_japan";
const SWR_PRODUCT: &str = "JAXA.JASMES_Terra.MODIS_swr.v811_global_monthly";

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
    serde_json::from_str(include_str!("jaxa-extra-reviewed-items.json"))
        .expect("actual unchanged JAXA HRLULC/SWR Items")
});
static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("jaxa-extra-reviewed-collections.json"))
        .expect("actual unchanged JAXA HRLULC/SWR Collections")
});

type ProductSpec = (
    &'static str,
    &'static str,
    &'static str,
    &'static str,
    u16,
    Option<u8>,
);

fn spec(id: &str) -> Result<ProductSpec> {
    match id {
        HR => Ok((HR_PRODUCT, "2024", "HRLULC", "hrlulc", 2024, None)),
        SWR => Ok((SWR_PRODUCT, "2026-09", "swr", "swr", 2026, Some(9))),
        _ => Err("Unknown reviewed JAXA land-cover or shortwave-radiation product".into()),
    }
}

pub(super) fn definitions() -> Vec<Dataset> {
    let mut hr = base(
        HR,
        "JAXA HRLULC · Japan land cover overview",
        "thematic",
        "https://www.eorc.jaxa.jp/ALOS/en/dataset/lulc/lulc_v2504_e.htm",
    );
    hr.description="JAXA High-Resolution Land-Use and Land-Cover Map of Japan Version 25.04, reviewed 2024 classification delivered as one complete JAXA Earth COG-level-1 overview. This converted distribution retains 15 land-cover categories derived from multi-satellite and auxiliary data. It is not optical imagery, unclassified radar, continuous canopy percentage or the mission's original 10 m tile delivery.".into();
    hr.version =
        "HRLULC v25.04 · 2024 classification · JAXA Earth COG v1 · level-1 overview".into();
    hr.nominal_resolution="0.025° WGS84 publisher overview grid · 3,600 × 3,600 across the NE 90° tile; about 2.8 km spacing at the equator, not the underlying 10 m HRLULC product".into();
    hr.unit = "land-use/land-cover class code".into();
    hr.value_interpretation="Original UInt8 categories: 0 No data; 1 Water bodies; 2 Built-up; 3 Paddy field; 4 Cropland; 5 Grassland; 6 Deciduous broad-leaf forest; 7 Deciduous needle-leaf forest; 8 Evergreen broad-leaf forest; 9 Evergreen needle-leaf forest; 10 Bare; 11 Bamboo forest; 12 Solar panel; 13 Wetland; 14 Greenhouse; 15 Rock reef and Tidal flat. Publisher slope 1, offset 0, scalar NoData/error 0. Codes are labels, not percentages or radiance. No averaging, class merging, resampling, clipping or reprojection is applied.".into();
    hr.layers = vec![scaled(
        layer(
            "hrlulc",
            "Japan land-cover classification · original class codes",
            "class code",
            &hr.value_interpretation,
        ),
        1.,
        0.,
        0.,
    )];
    hr.default_layer = Some("hrlulc".into());
    hr.years = vec![2024];
    hr.default_year = Some(2024);
    hr.attribution="JAXA HRLULC Japan Version 25.04, 2024 classification, Earth Observation Research Center (EORC), Japan Aerospace Exploration Agency (JAXA). This data was obtained through JAXA Earth API. Preserve JAXA and named third-party source credits, original year, class legend, product version, publisher Item and access date.".into();
    hr.warnings=vec![
        "Only the actual 2024 COG-level-1 HRLULC Japan overview is indexed. The official underlying 10 m product has 131 one-degree files per year and requires a separate original-product workflow; those files, 2020/2022 and finer pyramid levels are not substituted or advertised as accepted downloads.".into(),
        "The Collection declares Japan's geographic envelope [122,20,154,46], while the single rectangular file covers [90,0,180,90]. Requests outside the Collection envelope fail explicitly, and an intersection within that envelope still does not prove useful Japanese land coverage. The independently decoded original contains substantial ocean/outside-area NoData; Beijing is NoData, not an alternate demonstration region.".into(),
        "The original Item and TIFF interval identifies calendar year 2024, not a single acquisition. Official classification inputs include current-year and previous-year weighted optical composites and auxiliary data. April 2025 is the product version release, and May 2026 is this COG creation; neither is a 2025/2026 observation year.".into(),
        "The unchanged Item uses STAC 1.1 and proj:code EPSG:4326. Its Collection has the matching full product ID. The publisher marks the level-1 file as overview and MODE resampling. A raw grayscale preview is not the original thematic legend or evidence of 10 m detail.".into(),
    ];
    hr.limits=vec!["One complete reviewed 2024 Japan overview file; unsupported year, month, layer, country, named region and season selections fail explicitly. The selected AOI filters the original file; it never crops or creates new class values.".into()];

    let mut swr = base(
        SWR,
        "JAXA MODIS · Shortwave radiation",
        "climate",
        "https://kuroshio.eorc.jaxa.jp/JASMES/datalist.html",
    );
    swr.description="JAXA JASMES Terra MODIS Version 811 surface shortwave-radiation product from the September 2026 monthly release, delivered as complete numerical COG-level-1 tiles. Shortwave radiation is surface solar irradiance, not air temperature, rainfall, solar-panel electricity output, optical RGB or MODIS mission HDF delivery. The exact publisher interval in this release is September 1–15 and is retained without expansion.".into();
    swr.version =
        "JASMES Terra MODIS SWR v811 · September 2026 release · JAXA Earth COG v1 · level 1".into();
    swr.nominal_resolution="0.05° WGS84 publisher distribution grid · 1,800 × 1,800 per 90° tile; grid spacing is not an independent sensor footprint or local irradiance accuracy".into();
    swr.unit = "W/m² surface shortwave radiation".into();
    swr.value_interpretation="Original UInt16 DN are retained. Publisher surface shortwave radiation [W/m²] = DN × 0.01, offset 0. Scalar NoData/error is 65535; zero is a valid declared value, not missing. The original TIFF embeds scale 1 and offset 0; physical calibration comes from the exact STAC je:rasters declaration. No automatic scaling, temporal averaging, integration to energy, unit conversion, quality masking, clipping or reprojection is applied.".into();
    swr.layers = vec![scaled(
        layer(
            "swr",
            "Surface shortwave radiation · original DN",
            "W/m² (valid DN × 0.01)",
            &swr.value_interpretation,
        ),
        0.01,
        0.,
        65535.,
    )];
    swr.default_layer = Some("swr".into());
    swr.years = vec![2026];
    swr.default_year = Some(2026);
    swr.months = vec![9];
    swr.default_month = Some(9);
    swr.attribution="JAXA JASMES Terra MODIS surface Shortwave Radiation Version 811; Japan Aerospace Exploration Agency (JAXA), Earth Observation Research Center (EORC), NASA/GSFC MODIS. This data was obtained through JAXA Earth API. Retain the exact Item acquisition interval, original DN and calibration metadata, source product, license and access date.".into();
    swr.warnings=vec![
        "Only the actual September 2026 Terra MODIS SWR COG-level-1 release is indexed. Aqua MODIS, daily/half-month products, monthly normals, other months and pyramid levels are not substitutes. The selected area filters intersecting complete 90° tiles; no demonstration area is inserted.".into(),
        "The Collection title and YYYY-MM path describe a monthly shortwave-radiation product, but all eight archived Items and the independently decoded TIFF declare 2026-09-01T00:00:00.000000Z through 2026-09-15T23:59:59.999999Z. The exact half-month interval remains visible. This conflict does not prove a complete September mean, and no month-end date is fabricated.".into(),
        "This is JAXA Earth's converted numerical 0.05° distribution, not a native MODIS granule. Embedded overview resampling is MODE; raw grayscale preview is not a calibrated radiation palette or a computed energy total. Original gaps and NoData remain; useful local coverage and solar-energy yield are not guaranteed.".into(),
        "The exact publisher Collection uses generic ID collection while Items name the full Terra MODIS SWR product ID. The unchanged conflict is archived without inventing an alias or asserting a matching verified Collection.".into(),
    ];
    swr.limits=vec!["One finite September 2026 publisher release, up to eight intersecting 90° geographic tiles. Unsupported year, month, layer, country, named region and season selections fail explicitly.".into()];
    for dataset in [&mut hr, &mut swr] {
        dataset.discovery = Some("stac-items".into());
        dataset.license = "proprietary".into();
        dataset.license_url = "https://earth.jaxa.jp/en/data/policy/".into();
        dataset.warnings.extend([
            "Publisher Items declare proprietary research-data terms. The linked JAXA policy requires source and named third-party credits and advance notification for commercial use; public fee-free delivery is not a Creative Commons license.".into(),
            "Items declare no file checksum. Local SHA-256 receipts and observed HTTP ETags are not publisher checksum verification. Standalone Items have no self link; exact independently retrieved URLs are used for revalidation. Relative TIFF paths use public HTTPS without private-bucket access or an S3 transport exception.".into(),
        ]);
        dataset.limits.push("Complete original transfers retain the 512 MiB limit and current TIFF pixel-count/per-axis limits. Only the actual NE original has been independently transferred and fully decoded for each product; other indexed SWR tiles retain exact publisher metadata without a per-file download acceptance claim.".into());
    }
    vec![hr, swr]
}

fn bounds(document: &Value) -> Result<[f64; 4]> {
    let value = serde_json::from_value(document["bbox"].clone()).map_err(crate::io_error)?;
    features::bounds(value)?;
    Ok(value)
}
fn intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}
fn source(item: &ReviewedItem) -> Result<DatasetSource> {
    let (product, date, _, _, _, _) = spec(&item.dataset_id)?;
    let region = item
        .url
        .strip_prefix(&format!("{HTTPS}{product}/{date}/1/"))
        .and_then(|v| v.strip_suffix(".json"))
        .ok_or("JAXA extra Item is outside its exact reviewed product, period and level")?;
    let label = if item.dataset_id == HR {
        "HRLULC"
    } else {
        "MODIS SWR"
    };
    let title = format!("{label} {date} · {}", region.replace('/', " · "));
    Ok(DatasetSource {
        id: format!(
            "{}-{date}-{}",
            if item.dataset_id == HR {
                "hrlulc"
            } else {
                "swr"
            },
            region.replace('/', "-")
        ),
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
    let (_, _, _, layer, year, month) = spec(&request.dataset_id)?;
    if request.year != Some(year)
        || request.month != month
        || request.layer.as_deref() != Some(layer)
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the exact reviewed JAXA HRLULC 2024 or Terra MODIS SWR September 2026 release; other dates, layers, countries, regions and seasons are not substituted".into());
    }
    if request.dataset_id == HR && !intersects(request.bounds, [122., 20., 154., 46.]) {
        return Err("The selected area is outside the reviewed HRLULC Japan Collection envelope; a Japanese demonstration area is not substituted".into());
    }
    let selected = ITEMS
        .iter()
        .filter(|item| item.dataset_id == request.dataset_id)
        .filter_map(|item| match bounds(&item.document) {
            Ok(tile) if intersects(request.bounds, tile) => Some(source(item)),
            Ok(_) => None,
            Err(e) => Some(Err(e)),
        })
        .collect::<Result<Vec<_>>>()?;
    if selected.is_empty() || selected.len() > MAX_TILES {
        return Err("No exact reviewed JAXA extra-product file intersects this area or the eight-tile limit is exceeded".into());
    }
    Ok(selected)
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    let (product, _, asset, _, _, _) = spec(&proof.dataset.id)?;
    if proof.schema_version != 2 || proof.catalog_raster.is_some() {
        return Err("Unsupported JAXA extra-product publisher-Item provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let selected = plan
        .sources
        .iter()
        .find(|source| {
            source.id == proof.source_id
                && source.stac.as_ref().is_some_and(|entry| entry.url == url)
        })
        .ok_or(
            "JAXA extra source differs from the exact selected product, period and geographic tile",
        )?;
    if super::provenance(&plan, selected) != *proof {
        return Err("JAXA extra product, layer, request or scientific declaration differs from its immutable preset".into());
    }
    let reviewed = ITEMS
        .iter()
        .find(|item| item.dataset_id == proof.dataset.id && item.url == url)
        .ok_or("JAXA extra Item is outside the finite reviewed index")?;
    if reviewed.collection_url != format!("{HTTPS}{product}/collection.json") {
        return Err("JAXA extra Collection URL differs from the exact reviewed product".into());
    }
    if raw.is_some_and(|value| value != &reviewed.document) {
        return Err("JAXA extra publisher Item changed: exact period, classification, scaling, NoData, asset, grid, roles, license or metadata conflict differs from review".into());
    }
    if reviewed.document["collection"] != product
        || reviewed.document["assets"]
            .as_object()
            .is_none_or(|assets| assets.len() != 1 || !assets.contains_key(asset))
    {
        return Err("JAXA extra Item has an unexpected asset or product identity".into());
    }
    Ok(())
}
pub(crate) fn validate_collection(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<()> {
    let (product, _, _, _, _, _) = spec(&proof.dataset.id)?;
    let plan = super::resolve(proof.request.clone())?;
    let item_url = plan
        .sources
        .iter()
        .find(|entry| entry.id == proof.source_id)
        .and_then(|entry| entry.stac.as_ref())
        .map(|entry| entry.url.as_str())
        .ok_or("JAXA extra Collection receipt has no exact selected Item")?;
    validate(proof, item_url, None)?;
    let reviewed = COLLECTIONS
        .iter()
        .find(|entry| entry.dataset_id == proof.dataset.id && entry.url == url)
        .ok_or("JAXA extra Collection is outside its exact reviewed endpoint")?;
    if url != format!("{HTTPS}{product}/collection.json")
        || raw != &reviewed.document
        || raw["type"] != "Collection"
        || raw["license"] != "proprietary"
        || raw["id"]
            != if proof.dataset.id == HR {
                HR_PRODUCT
            } else {
                "collection"
            }
    {
        return Err("JAXA extra original Collection identity, license, scientific metadata or catalog contents changed since review".into());
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
        .is_none_or(|links| links.iter().any(|entry| entry["rel"] == "self"))
    {
        return Err("Reviewed standalone JAXA extra Item unexpectedly has a self link".into());
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn request(id: &str, area: [f64; 4]) -> ResolveRequest {
        let (_, _, _, layer, year, month) = spec(id).unwrap();
        serde_json::from_value(
            json!({"datasetId":id,"year":year,"month":month,"layer":layer,"bounds":area}),
        )
        .unwrap()
    }
    #[test]
    fn exact_aoi_dates_and_categories_do_not_substitute_a_japanese_demo_or_full_month() {
        assert_eq!(ITEMS.len(), 9);
        let input = request(HR, [139., 35., 140., 36.]);
        let before = input.clone();
        let hr = resolve(&input).unwrap();
        assert_eq!(before, input);
        assert_eq!(hr.len(), 1);
        assert_eq!(
            hr[0].start_datetime.as_deref(),
            Some("2024-01-01T00:00:00+00:00")
        );
        assert_eq!(
            hr[0].end_datetime.as_deref(),
            Some("2024-12-31T23:59:59+00:00")
        );
        assert!(resolve(&request(HR, [116., 39., 117., 40.])).is_err());
        assert!(resolve(&request(HR, [0., 0., 1., 1.])).is_err());
        let swr = resolve(&request(SWR, [116., 39., 117., 40.])).unwrap();
        assert_eq!(swr.len(), 1);
        assert_eq!(
            swr[0].end_datetime.as_deref(),
            Some("2026-09-15T23:59:59.999999Z")
        );
        assert_eq!(
            resolve(&request(SWR, [89., -1., 91., 1.])).unwrap().len(),
            4
        );
        assert_eq!(
            resolve(&request(SWR, [-180., -90., 180., 90.]))
                .unwrap()
                .len(),
            8
        );
        assert_eq!(
            resolve(&request(SWR, [90., 0., 180., 90.])).unwrap().len(),
            1
        );
        for id in [HR, SWR] {
            let base = request(id, [139., 35., 140., 36.]);
            for changed in [
                json!({"year":2022}),
                json!({"month":8}),
                json!({"layer":"rgb"}),
                json!({"country":"JPN"}),
                json!({"region":"demo"}),
                json!({"season":"annual"}),
            ] {
                let mut value = serde_json::to_value(&base).unwrap();
                value
                    .as_object_mut()
                    .unwrap()
                    .extend(changed.as_object().unwrap().clone());
                assert!(resolve(&serde_json::from_value(value).unwrap()).is_err());
            }
        }
        for item in ITEMS.iter() {
            assert!(source(item).unwrap().stac.unwrap().name.len() <= 80);
        }
        let definitions = definitions();
        assert_eq!(definitions[0].category, "thematic");
        assert_eq!(definitions[1].category, "climate");
        assert_eq!(definitions[0].layers[0].nodata, Some(0.));
        assert_eq!(definitions[1].layers[0].scale, Some(0.01));
        assert_eq!(definitions[1].layers[0].nodata, Some(65535.));
    }
    #[test]
    fn immutable_provenance_keeps_class_legend_unit_grid_nodata_and_actual_intervals() {
        for id in [HR, SWR] {
            let plan = super::super::resolve(request(id, [139., 35., 140., 36.])).unwrap();
            let selected = &plan.sources[0];
            let proof = super::super::provenance(&plan, selected);
            let url = &selected.stac.as_ref().unwrap().url;
            let raw = ITEMS
                .iter()
                .find(|item| &item.url == url && item.dataset_id == id)
                .unwrap()
                .document
                .clone();
            let (_, _, asset, _, _, _) = spec(id).unwrap();
            assert!(validate(&proof, url, Some(&raw)).is_ok());
            assert_eq!(self_url(&proof, url, &raw).unwrap(), *url);
            assert!(transports(&proof, url, &raw).unwrap().is_empty());
            if id == HR {
                assert_eq!(raw["stac_version"], "1.1.0");
                assert_eq!(raw["properties"]["proj:code"], "EPSG:4326");
                assert_eq!(
                    raw["assets"][asset]["classification:classes"]
                        .as_array()
                        .unwrap()
                        .len(),
                    16
                );
            } else {
                assert_eq!(raw["assets"][asset]["je:rasters"]["value"]["unit"], "W/m^2");
                assert_eq!(raw["assets"][asset]["je:rasters"]["dn"]["min"], json!(0.));
            }
            for field in [
                "href",
                "interval",
                "slope",
                "nodata",
                "unit",
                "license",
                "bbox",
                "roles",
                "collection",
                "legend",
                "self",
            ] {
                let mut changed = raw.clone();
                match field {
                    "href" => {
                        changed["assets"][asset]["href"] =
                            json!("https://example.com/replacement.tif")
                    }
                    "interval" => {
                        changed["properties"]["end_datetime"] = json!("2026-09-30T23:59:59Z")
                    }
                    "slope" => {
                        changed["assets"][asset]["je:rasters"]["dn2value"]["slope"] = json!(2.)
                    }
                    "nodata" => changed["assets"][asset]["je:rasters"]["dn"]["nodata"] = json!(255),
                    "unit" => {
                        changed["assets"][asset]["je:rasters"]["value"]["unit"] =
                            json!("Watt-hours")
                    }
                    "license" => changed["properties"]["license"] = json!("CC-BY-4.0"),
                    "bbox" => changed["bbox"] = json!([0., 0., 90., 90.]),
                    "roles" => changed["assets"][asset]["roles"] = json!(["overview"]),
                    "collection" => changed["collection"] = json!("alias"),
                    "legend" => changed["assets"][asset]["classification:classes"] = json!([]),
                    _ => changed["links"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"rel":"self","href":"s3://private/item.json"})),
                };
                assert!(
                    validate(&proof, url, Some(&changed)).is_err(),
                    "{id}:{field}"
                );
            }
            assert!(validate(&proof, &url.replace("/1/", "/0/"), Some(&raw)).is_err());
            let mut changed = proof.clone();
            changed.layer.scale = Some(42.);
            assert!(validate(&changed, url, Some(&raw)).is_err());
        }
    }
    #[test]
    fn collections_preserve_hr_full_identity_and_swr_generic_conflict_without_aliasing() {
        for id in [HR, SWR] {
            let plan = super::super::resolve(request(id, [139., 35., 140., 36.])).unwrap();
            let proof = super::super::provenance(&plan, &plan.sources[0]);
            let reviewed = COLLECTIONS
                .iter()
                .find(|entry| entry.dataset_id == id)
                .unwrap();
            assert!(validate_collection(&proof, &reviewed.url, &reviewed.document).is_ok());
            assert_eq!(
                reviewed.document["id"],
                if id == HR { HR_PRODUCT } else { "collection" }
            );
            for field in ["id", "license", "summaries", "links", "description"] {
                let mut changed = reviewed.document.clone();
                changed[field] = json!("replacement");
                assert!(validate_collection(&proof, &reviewed.url, &changed).is_err());
            }
            let wrong = if id == HR {
                format!("{HTTPS}{SWR_PRODUCT}/collection.json")
            } else {
                format!("{HTTPS}{HR_PRODUCT}/collection.json")
            };
            assert!(validate_collection(&proof, &wrong, &reviewed.document).is_err());
        }
    }
}
