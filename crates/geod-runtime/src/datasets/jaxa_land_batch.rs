//! Four finite, independently decoded JAXA Earth numerical products.
//! Preserve original monthly Items, calibration declarations and exact files.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

const HTTPS: &str = "https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/";
struct Product {
    id: &'static str,
    collection: &'static str,
    asset: &'static str,
    layer: &'static str,
    month: u8,
    title: &'static str,
    category: &'static str,
    description: &'static str,
    version: &'static str,
    resolution: &'static str,
    unit: &'static str,
    interpretation: &'static str,
    documentation: &'static str,
    slope: f64,
    offset: f64,
    warning: &'static str,
}
const PRODUCTS: &[Product] = &[
    Product {
        id: "jaxa-sgli-et", collection: "JAXA.JASMES_GCOM-C.SGLI_ET.v1_global_monthly", asset: "ET", layer: "et", month: 4,
        title: "JAXA SGLI · Evapotranspiration", category: "climate",
        description: "JASMES GCOM-C SGLI ET Version 1 April 2026 monthly-labelled terrestrial evapotranspiration estimate, distributed as unchanged complete JAXA Earth COG-level-1 numerical raster tiles. This model-derived water flux uses SGLI land-surface temperature and weather information; it is not rainfall, soil moisture, reference evapotranspiration or a second copy of its separate dimensionless ET index.",
        version: "SGLI ET v1 · April 2026 release · JAXA Earth COG v1 · level-1 raster",
        resolution: "0.05° WGS84 publisher raster grid · 1,800 × 1,800 per 90° tile; about 5.6 km at the equator, not the underlying 250 m SGLI algorithm grid or local accuracy",
        unit: "mm/day in publisher metadata; monthly aggregation wording remains unresolved",
        interpretation: "Original UInt16 DN are retained. Exact publisher calibration: ET = DN × 0.0004615384677890688 − 5, declared unit mm/day. Scalar NoData 65535 is not ET. The TIFF embeds scale 1 / offset 0; physical scaling comes from unchanged STAC. Publisher title says '1 month total evapotranspiration [mm/day]', which conflicts with an accumulated total's expected dimension. The adapter does not silently call these monthly totals in mm, monthly daily means, or multiply values by days. The actual NE stored valid extrema 10833–26165 and all NoData remain unchanged.",
        documentation: "https://www.jstage.jst.go.jp/article/agrmet/82/1/82_D-25-00022/_article",
        slope: 0.0004615384677890688, offset: -5.,
        warning: "The publisher's monthly total title and mm/day unit are preserved as an unresolved temporal-aggregation declaration. This is the April 2026 released numerical raster, not a claim of an independently verified monthly sum or daily mean. Model uncertainty, cloud gaps and surface-temperature/weather assumptions are not removed by successful download.",
    },
    Product {
        id: "jaxa-sgli-et-index", collection: "JAXA.JASMES_GCOM-C.SGLI_ETidx.v1_global_monthly", asset: "ETidx", layer: "et-index", month: 4,
        title: "JAXA SGLI · Evapotranspiration index", category: "climate",
        description: "JASMES GCOM-C SGLI ETidx Version 1 April 2026 monthly-labelled evapotranspiration index, distributed as unchanged complete JAXA Earth COG-level-1 numerical raster tiles. This dimensionless algorithm index is a separate published product from ET water flux, rather than a renamed month, band, rainfall or vegetation index.",
        version: "SGLI ETidx v1 · April 2026 release · JAXA Earth COG v1 · level-1 raster",
        resolution: "0.05° WGS84 publisher raster grid · 1,800 × 1,800 per 90° tile; about 5.6 km at the equator, not the underlying 250 m SGLI algorithm grid or local accuracy",
        unit: "Dimensionless ET index (publisher unit N/A)",
        interpretation: "Original UInt16 DN are retained. Exact publisher calibration: ET index = DN × 0.00009999999747378752, offset 0; STAC unit N/A is preserved. NoData 65535 is not an index. Zero is valid: 21,574 NE pixels store zero. NE maximum DN 12299 corresponds to about 1.2299, so the adapter does not clamp this product to [0,1], assign percentage, or treat it as mm/day. TIFF scale 1 / offset 0 is retained; no automatic scaling, masking or calibration is applied.",
        documentation: "https://www.jstage.jst.go.jp/article/agrmet/82/1/82_D-25-00022/_article",
        slope: 0.00009999999747378752, offset: 0.,
        warning: "ETidx is an estimated dimensionless index, not the ET water-flux product, a direct observation, a guaranteed [0,1] fraction, a drought category, or a calibrated soil-moisture measurement. Its actual zero and values above one remain unchanged. The exact April 1–30 acquisition interval is retained without an unverified temporal-mean claim.",
    },
    Product {
        id: "jaxa-sgli-aerosol", collection: "JAXA.G-Portal_GCOM-C.SGLI_standard.L3-AROT.daytime.v3_global_monthly", asset: "AROT", layer: "aod", month: 9,
        title: "JAXA SGLI · Aerosol optical depth", category: "climate",
        description: "GCOM-C SGLI standard Version 3 daytime Level-3 aerosol optical depth at 500 nm from the September 2026 monthly-labelled release, distributed as complete JAXA Earth COG-level-1 numerical overview tiles. SGLI AROT is independent of the Terra+Aqua MODIS JASMES TAUA product, with a distinct sensor, algorithm, grid and scaling.",
        version: "SGLI AROT standard v3 · September 2026 release · JAXA Earth COG v1 · level-1 overview",
        resolution: "0.025° WGS84 publisher overview grid · 3,600 × 3,600 per 90° tile; about 2.8 km at the equator, not the original SGLI sensor resolution or local accuracy",
        unit: "Dimensionless aerosol optical depth at 500 nm (publisher unit NA)",
        interpretation: "Original UInt16 DN are retained. Exact publisher calibration: AOD = DN × 0.00009999999747378752, offset 0. STAC unit NA and error/NoData 65535 are unchanged; the source NE stored valid extrema are 11–50000 even though its declared lower bound is 10. TIFF scale 1 / offset 0 is retained. AOD measures column optical extinction, not surface PM2.5 concentration, aerosol mass, cloud optical thickness or a health threshold. The layer's native numerical overview and MODE resampling are not promoted to underlying sensor resolution.",
        documentation: "https://earth.jaxa.jp/en/data/products/aerosol/index.html",
        slope: 0.00009999999747378752, offset: 0.,
        warning: "The actual September 2026 interval is 2026-09-01T00:24:46.800000Z through 2026-09-30T23:40:19.200000Z. These are publisher-derived overview values, not a single scene, monthly-normal statistic or guaranteed valid retrieval at every pixel. No separate aerosol QA or cloud mask asset is supplied; no extra validity rule is invented.",
    },
    Product {
        id: "jaxa-modis-aerosol", collection: "JAXA.JASMES_Terra.MODIS-Aqua.MODIS_taua.v811_global_monthly", asset: "T500", layer: "aod", month: 9,
        title: "JAXA MODIS · Aerosol optical depth", category: "climate",
        description: "JAXA JASMES Terra+Aqua MODIS TAUA Version 811 September 2026 monthly-labelled aerosol optical depth at 500 nm, distributed as complete JAXA Earth COG-level-1 numerical raster tiles. This is an independently published MODIS-based product; it does not reuse SGLI AROT bytes, calibration or quality declarations.",
        version: "JASMES MODIS TAUA v811 · September 2026 release · JAXA Earth COG v1 · level-1 raster",
        resolution: "0.05° WGS84 publisher raster grid · 1,800 × 1,800 per 90° tile; about 5.6 km at the equator, not the MODIS instrument pixel size or local accuracy",
        unit: "Dimensionless aerosol optical depth at 500 nm (publisher unit null)",
        interpretation: "Original UInt16 T500 DN are retained. Exact publisher calibration: AOD = DN × 0.001, offset 0; the original STAC physical-unit field is null, while title and JASMES scientific documentation identify 500 nm optical depth. Scalar error/NoData 65535 is retained. NE stored valid extrema 1–5404 differ from the declared upper bound 9999; declared bounds are not universal quality limits. TIFF scale 1 / offset 0 is unchanged. Do not apply the SGLI slope, convert to PM2.5, treat this as mass concentration, or invent missing QA masks.",
        documentation: "https://www.eorc.jaxa.jp/JASMES/datalist.html",
        slope: 0.001, offset: 0.,
        warning: "Only the actual September 2026 MODIS T500 raster is indexed, with its unchanged September 1–30 acquisition interval. Monthly-labelled release and successful transfer do not independently prove a monthly mean or useful valid-pixel coverage. Underlying MODIS observations, processing and calibration differ from the separate SGLI AROT product.",
    },
];

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
    serde_json::from_str(include_str!("jaxa-land-batch-reviewed-items.json"))
        .expect("32 actual JAXA ET, ETidx, AROT and MODIS T500 Items")
});
static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("jaxa-land-batch-reviewed-collections.json"))
        .expect("four unchanged JAXA publisher Collections")
});
fn product(id: &str) -> Result<&'static Product> {
    PRODUCTS
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "Unsupported JAXA land/atmosphere product".into())
}

pub(super) fn definitions() -> Vec<Dataset> {
    PRODUCTS.iter().map(|p| {
        let mut d=base(p.id,p.title,p.category,p.documentation);
        d.description=p.description.into(); d.version=p.version.into(); d.nominal_resolution=p.resolution.into(); d.unit=p.unit.into();d.value_interpretation=p.interpretation.into();
        d.layers=vec![scaled(layer(p.layer,"Original numerical DN",p.unit,p.interpretation),p.slope,p.offset,65535.)];
        d.default_layer=Some(p.layer.into());d.years=vec![2026];d.default_year=Some(2026);d.months=vec![p.month];d.default_month=Some(p.month);d.discovery=Some("stac-items".into());
        d.license="proprietary".into();d.license_url="https://earth.jaxa.jp/en/data/policy/".into();
        d.attribution=format!("{}; Japan Aerospace Exploration Agency (JAXA) Earth Observation Research Center (EORC), JASMES/GCOM-C science researchers, using the named original sensor. Credit JAXA, the research product and NASA Terra/Aqua for MODIS-based data. This data was obtained through JAXA Earth API. Retain source Item, acquisition interval, original DN scaling, version and access date.",p.collection);
        d.warnings=vec![p.warning.into(),
            "Only the finite reviewed year, month, named product and COG level are indexed. The selected area filters complete intersecting 90° geographic tiles; no demonstration AOI, alternative sensor, other month or missing file is substituted.".into(),
            "The numerical TIFF, every stored DN, scalar NoData, exact timestamps and original STAC metadata remain unchanged. No cropping, averaging, added scientific mask, pixel replacement, reprojection or calibration is applied. A grayscale preview is not physical calibration or scientific quality validation.".into(),
            "Publisher Items have no self link; exact retrieved HTTPS URLs are used for revalidation. ET and ETidx Collections name their actual products, while both aerosol Collections use generic ID collection. The exact original conflict is archived without inventing a Collection alias.".into(),
            "JAXA Items declare proprietary research-data terms. The linked JAXA policy requires source and contributing-research credits and advance notification for commercial use. Public delivery is not a Creative Commons grant; the algorithm paper's license does not replace the raster license. No commercial-use notification is sent by this adapter.".into(),
            "Items declare no publisher file checksum. Local SHA-256 and observed ETag establish local byte receipts, not publisher checksum verification. Relative TIFF paths use ordinary public HTTPS without credentials or S3 access exceptions.".into()];
        d.limits=vec!["One finite reviewed monthly-labelled release per independent product, up to eight actual intersecting geographic tiles. Unsupported date, layer, named country, region and season requests fail explicitly.".into(),"The complete original transfer preserves the existing 512 MiB, raster dimension and pixel-count limits. The NE original of each product has been fully downloaded and independently decoded; other seven indexed tiles retain exact publisher metadata without per-file download acceptance claims.".into()]; d
    }).collect()
}
fn bounds(doc: &Value) -> Result<[f64; 4]> {
    let b = serde_json::from_value(doc["bbox"].clone()).map_err(crate::io_error)?;
    features::bounds(b)?;
    Ok(b)
}
fn source(p: &Product, item: &ReviewedItem) -> Result<DatasetSource> {
    let prefix = format!("{HTTPS}{}/2026-{:02}/1/", p.collection, p.month);
    let region = item
        .url
        .strip_prefix(&prefix)
        .and_then(|x| x.strip_suffix(".json"))
        .ok_or("JAXA Item is outside its exact reviewed product, month and level")?;
    let title = format!(
        "{} 2026-{:02} · {}",
        p.title,
        p.month,
        region.replace('/', " · ")
    );
    Ok(DatasetSource {
        id: format!(
            "{}-2026-{:02}-{}",
            p.layer,
            p.month,
            region.replace('/', "-")
        ),
        title: title.clone(),
        bounds: Some(bounds(&item.document)?),
        stac: Some(stac::ConnectRequest {
            name: format!(
                "JAXA {} 2026-{:02} · {}",
                p.asset,
                p.month,
                region.replace('/', " · ")
            ),
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
    let p = product(&request.dataset_id)?;
    if request.year != Some(2026)
        || request.month != Some(p.month)
        || request.layer.as_deref() != Some(p.layer)
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the exact reviewed JAXA product, year, month and original layer; no alternate product, date, region or calibration is substituted".into());
    }
    let mut selected = Vec::new();
    for item in ITEMS.iter().filter(|i| i.dataset_id == p.id) {
        let b = bounds(&item.document)?;
        if request.bounds[0] < b[2]
            && request.bounds[2] > b[0]
            && request.bounds[1] < b[3]
            && request.bounds[3] > b[1]
        {
            selected.push(source(p, item)?);
        }
    }
    if selected.is_empty() || selected.len() > MAX_TILES {
        return Err(
            "No reviewed tile intersects this area or the request exceeds the eight-tile limit"
                .into(),
        );
    }
    Ok(selected)
}
pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    let p = product(&proof.dataset.id)?;
    if proof.schema_version != 2 || proof.catalog_raster.is_some() {
        return Err("Unsupported JAXA land/atmosphere preset provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let s = plan
        .sources
        .iter()
        .find(|s| s.id == proof.source_id && s.stac.as_ref().is_some_and(|s| s.url == url))
        .ok_or("JAXA source differs from the exact selected product tile")?;
    if super::provenance(&plan, s) != *proof {
        return Err(
            "JAXA product, request, layer or scaling differs from its immutable preset".into(),
        );
    }
    let item = ITEMS
        .iter()
        .find(|i| i.dataset_id == p.id && i.url == url)
        .ok_or("JAXA Item is outside its finite reviewed index")?;
    if item.collection_url != format!("{HTTPS}{}/collection.json", p.collection)
        || raw.is_some_and(|d| d != &item.document)
    {
        return Err("JAXA publisher Item changed: exact acquisition interval, DN scaling, scalar NoData, roles, license, tile or relative asset declaration differs from review".into());
    }
    if item.document["assets"]
        .as_object()
        .is_none_or(|a| a.len() != 1 || !a.contains_key(p.asset))
    {
        return Err("JAXA Item has an unexpected numerical measurement asset".into());
    }
    Ok(())
}
pub(crate) fn validate_collection(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<()> {
    let p = product(&proof.dataset.id)?;
    let plan = super::resolve(proof.request.clone())?;
    let selected = plan
        .sources
        .iter()
        .find(|s| s.id == proof.source_id)
        .and_then(|s| s.stac.as_ref())
        .ok_or("JAXA Collection has no exact selected publisher Item")?;
    validate(proof, &selected.url, None)?;
    let c = COLLECTIONS
        .iter()
        .find(|c| c.dataset_id == p.id && c.url == url)
        .ok_or("JAXA Collection URL is outside the exact reviewed endpoint")?;
    if url != format!("{HTTPS}{}/collection.json", p.collection)
        || raw != &c.document
        || raw["type"] != "Collection"
        || raw["license"] != "proprietary"
    {
        return Err("JAXA Collection identity, license, scientific metadata or contents changed since review".into());
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
        .is_none_or(|a| a.iter().any(|l| l["rel"] == "self"))
    {
        return Err("Reviewed standalone JAXA Item unexpectedly has a self link".into());
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn request(p: &Product, b: [f64; 4]) -> ResolveRequest {
        serde_json::from_value(
            json!({"datasetId":p.id,"bounds":b,"year":2026,"month":p.month,"layer":p.layer}),
        )
        .unwrap()
    }
    #[test]
    fn each_product_preserves_its_independent_physics_and_finite_release() {
        assert_eq!(definitions().len(), 4);
        assert_eq!(ITEMS.len(), 32);
        assert_eq!(COLLECTIONS.len(), 4);
        for p in PRODUCTS {
            let d = definitions().into_iter().find(|d| d.id == p.id).unwrap();
            assert_eq!(d.layers[0].scale, Some(p.slope));
            assert_eq!(d.layers[0].offset, Some(p.offset));
            assert_eq!(d.layers[0].nodata, Some(65535.));
            let r = request(p, [116., 39., 117., 40.]);
            let a = resolve(&r).unwrap();
            assert_eq!(a.len(), 1);
            assert_eq!(a[0].bounds, Some([90., 0., 180., 90.]));
            assert!(a[0].stac.as_ref().unwrap().name.chars().count() <= 80);
            for patch in [
                json!({"year":2025}),
                json!({"month":1}),
                json!({"layer":"rgb"}),
                json!({"country":"CHN"}),
                json!({"region":"Europe"}),
                json!({"season":"winter"}),
            ] {
                let mut v = serde_json::to_value(&r).unwrap();
                for (k, x) in patch.as_object().unwrap() {
                    v[k] = x.clone();
                }
                assert!(resolve(&serde_json::from_value(v).unwrap()).is_err());
            }
            assert_eq!(resolve(&request(p, [-1., -1., 1., 1.])).unwrap().len(), 4);
        }
        assert_ne!(PRODUCTS[2].slope, PRODUCTS[3].slope);
        let e = ITEMS
            .iter()
            .find(|i| {
                i.dataset_id == "jaxa-sgli-et-index"
                    && i.document["bbox"] == json!([90., 0., 180., 90.])
            })
            .unwrap();
        assert_eq!(e.document["assets"]["ETidx"]["je:rasters"]["dn"]["min"], 0.);
        assert_eq!(
            e.document["assets"]["ETidx"]["je:rasters"]["dn"]["max"],
            12299.
        );
    }
    #[test]
    fn all_unchanged_items_and_exact_collections_validate_but_scientific_mutations_fail() {
        for p in PRODUCTS {
            let plan = super::super::resolve(request(p, [116., 39., 117., 40.])).unwrap();
            let s = &plan.sources[0];
            let proof = super::super::provenance(&plan, s);
            let url = &s.stac.as_ref().unwrap().url;
            let item = ITEMS
                .iter()
                .find(|i| i.dataset_id == p.id && i.url == *url)
                .unwrap();
            let raw = &item.document;
            assert!(validate(&proof, url, Some(raw)).is_ok());
            assert_eq!(self_url(&proof, url, raw).unwrap(), *url);
            assert!(transports(&proof, url, raw).unwrap().is_empty());
            let c = COLLECTIONS.iter().find(|c| c.dataset_id == p.id).unwrap();
            assert!(validate_collection(&proof, &c.url, &c.document).is_ok());
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
                        changed["assets"][p.asset]["href"] =
                            json!("https://example.com/replacement.tif")
                    }
                    "slope" => {
                        changed["assets"][p.asset]["je:rasters"]["dn2value"]["slope"] = json!(1.)
                    }
                    "nodata" => changed["assets"][p.asset]["je:rasters"]["dn"]["nodata"] = json!(0),
                    "license" => changed["properties"]["license"] = json!("CC-BY-4.0"),
                    "interval" => {
                        changed["properties"]["start_datetime"] = json!("2026-10-01T00:00:00Z")
                    }
                    "collection" => changed["collection"] = json!("alias"),
                    "bbox" => changed["bbox"] = json!([0., 0., 90., 90.]),
                    "roles" => changed["assets"][p.asset]["roles"] = json!(["overview"]),
                    _ => changed["links"]
                        .as_array_mut()
                        .unwrap()
                        .push(json!({"rel":"self","href":"https://example.com/item.json"})),
                }
                assert!(
                    validate(&proof, url, Some(&changed)).is_err(),
                    "{} {field}",
                    p.id
                );
            }
            let mut changed = proof.clone();
            changed.layer.scale = Some(1.);
            assert!(validate(&changed, url, Some(raw)).is_err());
            assert!(validate(&proof, &url.replace("/1/", "/0/"), Some(raw)).is_err());
            for field in ["id", "license", "description", "links"] {
                let mut raw = c.document.clone();
                raw[field] = json!("replacement");
                assert!(validate_collection(&proof, &c.url, &raw).is_err());
            }
        }
    }
}
