//! Four finite, unchanged Planetary Computer publisher Items.
//! Public SAS is a transient transport capability, never persisted metadata.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

pub(crate) const ADAPTER: &str = "planetary-anonymous-sas";
const API: &str = "https://planetarycomputer.microsoft.com/api/stac/v1";
struct Product {
    id: &'static str,
    collection: &'static str,
    year: u16,
    month: Option<u8>,
    title: &'static str,
    category: &'static str,
    description: &'static str,
    version: &'static str,
    resolution: &'static str,
    interpretation: &'static str,
    documentation: &'static str,
    license: &'static str,
    license_url: &'static str,
    attribution: &'static str,
    layers: &'static [(&'static str, &'static str, &'static str)],
    account: &'static str,
    container: &'static str,
}
const PRODUCTS: &[Product] = &[
    Product {
        id: "pc-hrea", collection: "hrea", year: 2019, month: None,
        title: "HREA · Settlement electricity access", category: "population",
        description: "HREA 2019 v1 Hong Kong settlement-level predicted likelihood of electrification, derived from nightly VIIRS comparisons with matched uninhabited areas. This is a model probability product, separate from a nighttime-radiance composite, a utility connection inventory, energy consumption or measured outages. Only the exact reviewed Hong Kong Item and lightscore original are indexed.",
        version: "HREA v1 · Hong Kong · 2019 reference year",
        resolution: "0.0002777778° WGS84 grid · nominal 1 arc-second; country Item bbox and actual TIFF outer bounds differ slightly",
        interpretation: "Original Float32 model probability in [0,1], TIFF scale 1 / offset 0; the exact embedded scalar NoData is -3.3999999521443642e38. Valid zero is not NoData. Values exist at modeled settlement pixels, not throughout the country or across all empty land. The 2019-12-31 timestamp is the annual reference endpoint, not a single-night acquisition. Successful download does not verify real utility service or local model accuracy.",
        documentation: "https://planetarycomputer.microsoft.com/dataset/hrea",
        license: "CC-BY-4.0", license_url: CC_BY,
        attribution: "HREA research team; Brian Min, University of Michigan; Microsoft Planetary Computer distribution. Cite HREA, the named 2019 v1 Hong Kong Item, the published methodology and Creative Commons BY 4.0; retain version, source links and access date.",
        layers: &[("lightscore", "Probability of electrification", "Dimensionless predicted probability")],
        account: "hreadatasa", container: "hrea",
    },
    Product {
        id: "pc-biodiversity", collection: "io-biodiversity", year: 2020, month: None,
        title: "Impact Observatory · Biodiversity intactness", category: "vegetation",
        description: "Impact Observatory and Vizzuality v1 2020 modeled terrestrial biodiversity intactness, on the unchanged published WGS84 original covering the reviewed Beijing-region tile. Its abundance and compositional-similarity model uses human-pressure covariates; it is independent of land-cover classes, forest height or a count of observed species.",
        version: "Biodiversity Intactness v1 · 2020 · one reviewed 7.18° regional tile",
        resolution: "0.0008983152841195215° WGS84 original grid · 7,992 × 7,992 · nominal 100 m at the equator",
        interpretation: "Original Float32 model scores remain unchanged, with TIFF scale 1 / offset 0 and no embedded scalar NoData tag. The original has a separate internal 1-bit validity mask at every image level; masked finite raw values are not scientifically valid model scores. Raw scalar NoData flags remain distinct from this validity mask. Source NaN/nonfinite samples, if present, are not silently given a new scalar NoData declaration. The publisher default rendering declares 0.97*(data_b1**3.84); this visualization expression is retained as metadata but is not applied to the stored raster or asserted as an independent calibration. No species-count, field-survey, percentage or universal local-accuracy claim is inferred from a grayscale preview.",
        documentation: "https://ai4edatasetspublicassets.blob.core.windows.net/assets/pdfs/io-biodiversity/Biodiversity_Intactness_whitepaper.pdf",
        license: "CC-BY-4.0", license_url: CC_BY,
        attribution: "Impact Observatory and Vizzuality; PREDICTS research data and named human-pressure covariates; Microsoft Planetary Computer distribution. Credit the product, white paper, 2020 v1 Item and CC BY 4.0; retain source links and access date.",
        layers: &[("data", "Original biodiversity model score", "Dimensionless model score")],
        account: "pcdata01euw", container: "impact",
    },
    Product {
        id: "noaa-ohc", collection: "noaa-cdr-ocean-heat-content", year: 2020, month: None,
        title: "NOAA · Ocean heat content anomaly", category: "ocean",
        description: "NOAA Global Ocean Heat Content CDR 2020 Q4 seasonal 0–700 m integrated ocean heat-content anomaly relative to WOA09, distributed by Planetary Computer as a complete 1° COG derived from the source NetCDF. It is a historical ocean energy-content anomaly, not sea-surface temperature or current forecasts.",
        version: "NOAA OHC CDR · 2020 Q4 · 0–700 m · published COG derivative",
        resolution: "1° WGS84 area grid · 360 × 180 global cells; nominal 1° is not coastal detail",
        interpretation: "Original Float32 heat_content anomaly values in publisher unit 10^18 joules, embedded NaN NoData, scale 1 and offset 0. Signed anomalies and valid zero are retained. The actual 2020-10-01 through 2020-12-31 interval and noaa_cdr:max_depth 700 are immutable. No Celsius, heat flux, absolute heat content, per-area normalization or new anomaly baseline is substituted.",
        documentation: "https://www.ncei.noaa.gov/products/climate-data-records/global-ocean-heat-content",
        license: "proprietary", license_url: "https://www.ncei.noaa.gov/pub/data/sds/cdr/CDRs/Ocean_Heat_Content/UseAgreement_01B-41.pdf",
        attribution: "NOAA National Centers for Environmental Information Global Ocean Heat Content CDR, doi:10.7289/V53F4MVP; Microsoft Planetary Computer published COG derivative. Cite the specific quarterly interval, 0–700 m depth and access date; preserve the publisher Use Agreement rather than replacing proprietary metadata with a guessed CC license.",
        layers: &[("heat_content", "0–700 m ocean heat-content anomaly", "10^18 joules")],
        account: "noaacdr", container: "cogs",
    },
    Product {
        id: "noaa-nclimgrid", collection: "noaa-nclimgrid-monthly", year: 2022, month: Some(9),
        title: "NOAA NClimGrid · Monthly climate observations", category: "climate",
        description: "NOAA NClimGrid September 2022 CONUS monthly average, maximum and minimum air-temperature statistics and precipitation, based on GHCN-D station observations and distributed as unchanged Planetary Computer COG derivatives. Four variables belong to one climate product, not four new imagery channels.",
        version: "NClimGrid Monthly · September 2022 · reviewed CONUS published COGs",
        resolution: "0.04166667° WGS84 grid · 1,385 × 596 · nominal 5 km, not 5 arc-seconds or station-scale accuracy",
        interpretation: "Original Float32 tavg/tmax/tmin are in degree Celsius; original prcp is in mm for the published monthly interval. NaN NoData, scale 1 and offset 0 remain unchanged. This is surface air temperature and precipitation inferred from stations, not satellite land-surface temperature, instantaneous weather or guaranteed final quality status. The 2022-09-01 through 2022-09-30 interval is retained, with no daily archive or Hawaii/Alaska expansion claim.",
        documentation: "https://www.ncei.noaa.gov/access/metadata/landing-page/bin/iso?id=gov.noaa.ncdc:C00332",
        license: "proprietary", license_url: "https://www.ncei.noaa.gov/access/metadata/landing-page/bin/iso?id=gov.noaa.ncdc:C00332",
        attribution: "Vose et al. (2014), NOAA Monthly U.S. Climate Gridded Dataset (NClimGrid), Version 1, doi:10.7289/V5SX6B56; NOAA NCEI; Microsoft Planetary Computer published COG derivatives. State September 2022 variable/subset and access date; preserve NOAA use and distribution constraints and original proprietary Collection field.",
        layers: &[("tavg", "Monthly average air temperature", "degree Celsius"),("tmax", "Monthly maximum-temperature statistic", "degree Celsius"),("tmin", "Monthly minimum-temperature statistic", "degree Celsius"),("prcp", "Monthly precipitation", "mm")],
        account: "nclimgridwesteurope", container: "nclimgrid-cogs",
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
    serde_json::from_str(include_str!("next-planetary-reviewed-items.json"))
        .expect("four exact unchanged Planetary Computer Items")
});
static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("next-planetary-reviewed-collections.json"))
        .expect("four exact unchanged Planetary Computer Collections")
});
pub(crate) fn is_product(id: &str) -> bool {
    PRODUCTS.iter().any(|p| p.id == id)
}
fn product(id: &str) -> Result<&'static Product> {
    PRODUCTS
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "Unsupported reviewed Planetary product".into())
}
fn reviewed(p: &Product) -> Result<&'static ReviewedItem> {
    ITEMS
        .iter()
        .find(|i| i.dataset_id == p.id)
        .ok_or_else(|| "Reviewed Planetary Item is missing".into())
}

pub(super) fn definitions() -> Vec<Dataset> {
    PRODUCTS.iter().map(|p|{
        let mut d=base(p.id,p.title,p.category,p.documentation);
        d.description=p.description.into();d.version=p.version.into();d.nominal_resolution=p.resolution.into();d.unit=p.layers[0].2.into();d.value_interpretation=p.interpretation.into();
        d.license=p.license.into();d.license_url=p.license_url.into();d.attribution=p.attribution.into();
        d.layers=p.layers.iter().map(|(id,title,unit)|{let mut v=layer(id,title,unit,p.interpretation);v.scale=Some(1.);v.offset=Some(0.);if p.id=="pc-hrea"{v.nodata=Some(-3.3999999521443642e38);}v}).collect();
        d.default_layer=Some(p.layers[0].0.into());d.years=vec![p.year];d.default_year=Some(p.year);d.months=p.month.into_iter().collect();d.default_month=p.month;d.discovery=Some("stac-items".into());
        d.warnings=vec![p.interpretation.into(),
            "Only the exact named historical Item, reference interval and reviewed measurement assets are indexed. AOI filters the actual publisher bbox; it does not clip the original, replace the selected location, imply valid-pixel coverage or create unreviewed years/tiles.".into(),
            "The unchanged publisher Item and Collection retain every original asset and license declaration. Only the selected reviewed numerical layer receives transient anonymous Planetary Computer data access; unrelated assets are not granted broader transport permission.".into(),
            "Planetary Computer public anonymous SAS may be throttled. Temporary read-only capability URLs exist only during transfer and are not saved to Items, snapshots, settings or the code repository. Source URLs, downloaded bytes and local SHA-256 receipts remain canonical.".into(),
            "A complete transfer and visible preview are separate from scientific quality, calibration validation and useful local coverage. Original grids, masks, timestamps, NoData and raw values are preserved without reprojection, percentage conversion or modeled-data relabeling.".into()];
        d.limits=vec!["One exact reviewed publisher Item per independent product. Other archive countries, tiles, years, quarters, depths and daily products require independent acceptance before expansion.".into(),"Existing 512 MiB transfer, raster dimension, raw-pixel, preview network and decoder limits remain unchanged. No account, paid cloud processing, export generation or commercial-data procurement is used.".into()];d
    }).collect()
}
pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    let p = product(&request.dataset_id)?;
    if request.year != Some(p.year)
        || request.month != p.month
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
        || !p
            .layers
            .iter()
            .any(|(id, _, _)| request.layer.as_deref() == Some(*id))
    {
        return Err("Choose the exact reviewed product, interval and supported original layer; no alternate country, tile, date, depth or variable is substituted".into());
    }
    let item = reviewed(p)?;
    let b: [f64; 4] =
        serde_json::from_value(item.document["bbox"].clone()).map_err(crate::io_error)?;
    features::bounds(b)?;
    if !(request.bounds[0] < b[2]
        && request.bounds[2] > b[0]
        && request.bounds[1] < b[3]
        && request.bounds[3] > b[1])
    {
        return Err("No reviewed publisher Item intersects the selected area; choose a location within the documented finite product footprint".into());
    }
    let title = format!(
        "{} · {}",
        p.title,
        item.document["id"]
            .as_str()
            .ok_or("Publisher Item ID is missing")?
    );
    Ok(vec![DatasetSource {
        id: item.document["id"].as_str().unwrap().into(),
        title,
        bounds: Some(b),
        stac: Some(stac::ConnectRequest {
            name: format!("{} {}", p.title, p.year),
            url: item.url.clone(),
            kind: "item".into(),
        }),
        wcs: None,
        coverage_id: None,
        search: None,
        region: None,
        season: None,
        start_datetime: item.document["properties"]["start_datetime"]
            .as_str()
            .or_else(|| item.document["properties"]["datetime"].as_str())
            .map(str::to_owned),
        end_datetime: item.document["properties"]["end_datetime"]
            .as_str()
            .or_else(|| item.document["properties"]["datetime"].as_str())
            .map(str::to_owned),
        catalog_raster: None,
    }])
}
pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    let p = product(&proof.dataset.id)?;
    if proof.schema_version != 2 || proof.catalog_raster.is_some() {
        return Err("Unsupported reviewed Planetary source proof".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let source = plan
        .sources
        .iter()
        .find(|s| s.id == proof.source_id && s.stac.as_ref().is_some_and(|r| r.url == url))
        .ok_or("Planetary source differs from its exact reviewed Item and request")?;
    if super::provenance(&plan, source) != *proof {
        return Err("Planetary product, selected variable, reference interval or scientific declaration differs from its immutable preset".into());
    }
    let item = reviewed(p)?;
    if item.url != url
        || item.collection_url != format!("{API}/collections/{}", p.collection)
        || item.document["collection"] != p.collection
        || raw.is_some_and(|v| v != &item.document)
    {
        return Err("Exact publisher Item changed: identity, geometry, timestamps, roles, values, license or canonical asset declarations differ from review".into());
    }
    let key = proof.layer.id.as_str();
    let a = &item.document["assets"][key];
    let href = a["href"]
        .as_str()
        .ok_or("Selected reviewed numerical asset is missing")?;
    let u = url::Url::parse(href).map_err(crate::io_error)?;
    let host = format!("{}.blob.core.windows.net", p.account);
    if u.scheme() != "https"
        || u.host_str() != Some(host.as_str())
        || u.query().is_some()
        || u.fragment().is_some()
        || !u.username().is_empty()
        || u.password().is_some()
        || u.port().is_some()
        || !u.path().starts_with(&format!("/{}/", p.container))
        || !a["type"]
            .as_str()
            .is_some_and(|t| t.starts_with("image/tiff"))
        || a["roles"] != serde_json::json!(["data"])
    {
        return Err(
            "Selected asset is not the exact reviewed public PC numerical TIFF declaration".into(),
        );
    }
    Ok(())
}
pub(crate) fn validate_collection(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<()> {
    let p = product(&proof.dataset.id)?;
    let item = reviewed(p)?;
    validate(proof, &item.url, None)?;
    let c = COLLECTIONS
        .iter()
        .find(|c| c.dataset_id == p.id && c.url == url)
        .ok_or("Planetary Collection URL is outside its finite reviewed endpoint")?;
    if url != format!("{API}/collections/{}", p.collection)
        || raw != &c.document
        || raw["id"] != p.collection
        || raw["type"] != "Collection"
        || raw["license"] != p.license
    {
        return Err("Planetary Collection identity, scientific declarations or license changed since review".into());
    }
    Ok(())
}
pub(crate) fn transports(
    proof: &SourceProvenance,
    url: &str,
    raw: &Value,
) -> Result<Vec<stac::AssetTransport>> {
    validate(proof, url, Some(raw))?;
    let key = proof.layer.id.clone();
    let href = raw["assets"][&key]["href"]
        .as_str()
        .ok_or("Selected PC asset href is absent")?
        .to_string();
    Ok(vec![stac::AssetTransport {
        asset_key: key,
        original_href: href.clone(),
        href,
        adapter: ADAPTER.into(),
    }])
}
pub(crate) fn self_url(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<String> {
    validate(proof, url, Some(raw))?;
    if raw["links"].as_array().is_none_or(|ls| {
        ls.iter().filter(|l| l["rel"] == "self").count() != 1
            || !ls.iter().any(|l| l["rel"] == "self" && l["href"] == url)
    }) {
        return Err("Reviewed Planetary Item self URL differs from the exact endpoint".into());
    }
    Ok(url.into())
}
/// Call before every native anonymous signature request. Returns a container
/// scope only after exact immutable proof/Item/selected-layer/href validation.
pub(crate) fn access_scope(
    proof: &SourceProvenance,
    url: &str,
    raw: &Value,
    key: &str,
    href: &str,
) -> Result<(&'static str, &'static str)> {
    validate(proof, url, Some(raw))?;
    let p = product(&proof.dataset.id)?;
    if key != proof.layer.id || raw["assets"][key]["href"] != href {
        return Err(
            "PC signing permission is bound to one exact selected reviewed original asset".into(),
        );
    }
    Ok((p.account, p.container))
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn req(id: &str, b: [f64; 4], key: &str) -> ResolveRequest {
        let p = product(id).unwrap();
        serde_json::from_value(
            json!({"datasetId":id,"bounds":b,"year":p.year,"month":p.month,"layer":key}),
        )
        .unwrap()
    }
    #[test]
    fn finite_items_keep_independent_values_and_reject_scope_changes() {
        assert_eq!(definitions().len(), 4);
        assert_eq!(ITEMS.len(), 4);
        assert_eq!(COLLECTIONS.len(), 4);
        for p in PRODUCTS {
            let item = reviewed(p).unwrap();
            let b = serde_json::from_value(item.document["bbox"].clone()).unwrap();
            for (key, _, _) in p.layers {
                let r = req(p.id, b, key);
                let plan = super::super::resolve(r.clone()).unwrap();
                let source = &plan.sources[0];
                let proof = super::super::provenance(&plan, source);
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
                assert!(
                    access_scope(&proof, &item.url, &item.document, "other", &map[0].href).is_err()
                );
                assert!(access_scope(
                    &proof,
                    &item.url,
                    &item.document,
                    key,
                    &format!("{}?sig=changed", map[0].href)
                )
                .is_err());
                let c = COLLECTIONS.iter().find(|c| c.dataset_id == p.id).unwrap();
                validate_collection(&proof, &c.url, &c.document).unwrap();
                let mut changed = item.document.clone();
                changed["assets"][key]["href"] =
                    json!("https://unreviewed.blob.core.windows.net/a/other.tif");
                assert!(validate(&proof, &item.url, Some(&changed)).is_err());
                let mut wrong = r.clone();
                wrong.year = Some(2026);
                assert!(resolve(&wrong).is_err());
                wrong = r.clone();
                wrong.layer = Some("visual".into());
                assert!(resolve(&wrong).is_err());
                wrong = r.clone();
                wrong.country = Some("CHN".into());
                assert!(resolve(&wrong).is_err());
            }
        }
    }
    #[test]
    fn probability_and_ocean_energy_are_not_converted_or_masked_by_zero() {
        let defs = definitions();
        let h = defs.iter().find(|d| d.id == "pc-hrea").unwrap();
        assert_eq!(h.layers[0].nodata, Some(-3.3999999521443642e38));
        assert_eq!(h.layers[0].scale, Some(1.));
        let o = defs.iter().find(|d| d.id == "noaa-ohc").unwrap();
        assert_eq!(o.layers[0].unit, "10^18 joules");
        assert!(o.layers[0].nodata.is_none());
        assert!(o.months.is_empty());
        let n = defs.iter().find(|d| d.id == "noaa-nclimgrid").unwrap();
        assert_eq!(n.layers.len(), 4);
        assert_eq!(n.months, vec![9]);
        assert_eq!(n.layers.iter().find(|l| l.id == "prcp").unwrap().unit, "mm");
        assert!(!is_product("pc-hgb"));
        assert!(!is_product("pc-other"));
        assert!(resolve(&req("pc-hrea", [116., 39., 117., 40.], "lightscore")).is_err());
    }
}
