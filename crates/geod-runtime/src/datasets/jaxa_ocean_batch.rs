//! Finite unchanged JAXA sea-wind, microwave SST and drought-index releases.
//! Monthly selectors locate these exact reviewed releases; daily SST indexes
//! only 2026-10-11. Publisher originals and metadata conflicts stay intact.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

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
    serde_json::from_str(include_str!("jaxa-ocean-batch-reviewed-items.json"))
        .expect("actual finite JAXA wind, microwave SST and SPI Items")
});
static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("jaxa-ocean-batch-reviewed-collections.json"))
        .expect("actual unchanged JAXA wind, microwave SST and SPI Collections")
});

struct Spec {
    id: &'static str,
    product: &'static str,
    root: &'static str,
    period: &'static str,
    month: u8,
    level: u8,
    layer: &'static str,
    asset: &'static str,
    scale: f64,
    nodata: f64,
}
const SPECS: [Spec; 3] = [
    Spec {
        id: "jaxa-amsr2-wind",
        product: "JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SSW.daytime.v4_global_monthly",
        root: "https://s3.ap-northeast-1.wasabisys.com/je-pds/cog/v1/",
        period: "2026-09",
        month: 9,
        level: 1,
        layer: "ssw",
        asset: "SSW",
        scale: 0.01,
        nodata: -32768.,
    },
    Spec {
        id: "jaxa-amsr2-sst",
        product: "JAXA.G-Portal_GCOM-W.AMSR2_standard.L3-SST.daytime.v4_global_daily",
        root: "https://s3.ap-northeast-1.wasabisys.com/je-pds2/cog/v1/",
        period: "2026-10/11",
        month: 10,
        level: 0,
        layer: "sst",
        asset: "SST",
        scale: 0.01,
        nodata: -32768.,
    },
    Spec {
        id: "jaxa-gsmap-spi",
        product: "JAXA.EORC_GSMaP_SPI.climate.gnrt6_monthly",
        root: "https://s3.ap-northeast-1.wasabisys.com/je-pds2/cog/v1/",
        period: "2026-09",
        month: 9,
        level: 0,
        layer: "spi",
        asset: "SPI",
        scale: 1.,
        nodata: -999.,
    },
];
fn spec(id: &str) -> Result<&'static Spec> {
    SPECS
        .iter()
        .find(|s| s.id == id)
        .ok_or_else(|| "Unsupported reviewed JAXA batch product".into())
}

pub(super) fn definitions() -> Vec<Dataset> {
    SPECS.iter().map(|s| {
        let (title, description, version, resolution, unit, interpretation, layer_title, warnings) = match s.id {
            "jaxa-amsr2-wind" => (
                "JAXA AMSR2 · Sea-surface wind speed · 2026-09",
                "Only the September 2026 daytime monthly GCOM-W AMSR2 standard L3-SSW Version 4 release is indexed. This is ocean-surface wind speed from a microwave sensor, not wind direction, land wind, a wind vector field or the separate all-weather research product.",
                "AMSR2 standard SSW v4 · 2026-09 · JAXA Earth v1 path level-1",
                "0.1° WGS84 publisher grid · 900 × 900 per 90° tile; about 11.1 km spacing at the equator, not the approximately 15 km sensor footprint or local accuracy",
                "m/s for scientifically valid stored DN",
                "Original Int16 DN are retained. Publisher wind speed [m/s] = DN × 0.01. Embedded TIFF scale/offset remain 1/0; calibration comes from unchanged STAC metadata. Scalar NoData -32768 and publisher error codes -32761 through -32767 are not wind speed. The complete NE original contains 31,356 values -32767; its native scalar NoData flag remains false for that distinct source error code. Zero is not NoData. No scaling, averaging, error masking, cropping or reprojection is applied.",
                "2026-09 sea-surface wind speed · original DN",
                vec![
                    "Monthly-labelled daytime release; exact acquisition intervals vary by tile and remain in each unchanged Item. The NE Item and original span 2026-09-01T00:28:10.283000Z through 2026-09-30T23:50:46.024000Z. No unsupported temporal statistic, wind direction or forecast is inferred.",
                    "The publisher level-1 path and Collection maximum disagree with Item je:img_level, which labels level-2 as raster and levels 0/1 null. The original's measured 0.1° grid is reported without inventing an overview alias or claiming level-2 acceptance.",
                    "The NE Item declares DN min/max 34/2662, while complete decoding of its actual original gives valid 60/2058. Both declarations are retained; advertised extrema are not forced onto raw observations or treated as quality checks.",
                    "Sea ice, rain and land can cause missing or source-error values. No separate QA asset or marine mask is supplied. Non-NoData does not prove a valid ocean wind observation; this is not the all-weather ASW research product.",
                ],
            ),
            "jaxa-amsr2-sst" => (
                "JAXA AMSR2 · Microwave sea-surface temperature · 2026-10-11",
                "Only the actual 2026-10-11 daytime GCOM-W AMSR2 standard L3-SST Version 4 daily release is indexed. The year/month selector locates that fixed reviewed day, not every day or a monthly statistic. Microwave radiometer SST is a separate sensor/algorithm product from the optical/thermal SGLI SST connector.",
                "AMSR2 standard SST v4 · fixed day 2026-10-11 · JAXA Earth v1.2 level-0",
                "0.2° WGS84 publisher distribution grid · 900 × 900 per 180° hemisphere; about 22.2 km spacing at the equator, not an independently verified sensor resolution or coastal accuracy",
                "°C for valid stored DN",
                "Original Int16 DN are retained. Publisher temperature [degC] = DN × 0.01; TIFF embeds scale 0.01 and offset 0 without altering stored DN. The source STAC declares fill/NoData -32768, but the complete original has no embedded TIFF NoData tag. Native raw inspection therefore honestly reports scalar NoData false even for stored -32768. That fill code is not a temperature or proof of a valid observation. Zero and valid negative DN are retained. No automatic masking, calibration, cropping or reprojection is applied.",
                "2026-10-11 daytime microwave SST · original DN",
                vec![
                    "Only 2026-10-11T00:00:00Z through 2026-10-11T23:59:59Z is indexed. The month value 10 locates this fixed daily file; it does not fetch the whole month, substitute another day, compute a monthly temperature or support all live daily releases.",
                    "The complete eastern original contains 447,212 STAC-declared -32768 fill values without a TIFF NoData tag; native scalar flags remain false. Grayscale preview does not introduce a scientific mask or change those flags. Raw storage and scientifically usable observations are separate concepts.",
                    "Actual source-valid raw extrema are -200/3506, equivalent to -2/35.06°C. STAC supplies no min/max or separate quality asset. Do not clip to the general mission -2 to 35°C specification, infer which microwave frequency channel was converted, or claim a new marine QA mask.",
                    "Microwave SST can observe through some clouds, but land contamination, precipitation, sea ice and radio interference still restrict usable coverage. This is not SGLI SST, air temperature, water-column temperature, a 250 m image or the original mission HDF5 product.",
                ],
            ),
            _ => (
                "JAXA GSMaP · Standardized precipitation index · 2026-09",
                "Only the September 2026 JAXA GSMaP climate gnrt6 monthly-labelled Standardized Precipitation Index release is indexed. SPI is a dimensionless precipitation-derived drought/wetness index, not rainfall rate or accumulation, soil moisture, vegetation stress or a forecast.",
                "GSMaP climate SPI gnrt6 · 2026-09 · JAXA Earth v1.2 level-0",
                "0.2° WGS84 publisher distribution grid · 900 × 900 per 180° hemisphere; about 22.2 km spacing at the equator, not a claimed original algorithm or sensor resolution",
                "dimensionless SPI",
                "Original Float32 SPI values are retained with publisher scale 1 and offset 0. STAC declares -999 as fill/NoData, while the complete original has no embedded TIFF NoData tag. Native scalar NoData flags therefore remain false for -999; that sentinel is not an extreme drought measurement or a valid observation. Valid negative values denote drier conditions and positive values wetter conditions. No conversion to rainfall, automatic scientific mask, threshold classification, averaging, cropping or reprojection is applied.",
                "2026-09 standardized precipitation index · original Float32",
                vec![
                    "The reviewed publisher Item is labelled Monthly Averaged and declares the exact interval 2026-09-01T00:00:00Z through 2026-09-30T23:59:59Z. No unverified baseline period, gamma fit, SPI accumulation window or all-months availability is asserted by this connector.",
                    "The complete eastern original contains 611,865 values -999 with no embedded TIFF NoData tag; native scalar flags stay false. No-data and valid drought values must be distinguished using the original source declaration, not inferred from a grayscale preview.",
                    "The complete eastern original's valid Float32 range is approximately -3.949183464050293 to 3.7198359966278076. Actual negative values remain unchanged. No separate QA mask or guarantee of observations over water or the entire selected area is supplied.",
                ],
            ),
        };
        let mut d = base(s.id, title, if s.id == "jaxa-gsmap-spi" { "climate" } else { "ocean" }, if s.id == "jaxa-gsmap-spi" { "https://sharaku.eorc.jaxa.jp/GSMaP_CLM/" } else { "https://www.eorc.jaxa.jp/AMSR/datacatalog/ocean/index_en.html" });
        d.description = description.into(); d.version = version.into();
        d.nominal_resolution = resolution.into(); d.unit = unit.into();
        d.value_interpretation = interpretation.into();
        d.layers = vec![scaled(layer(s.layer, layer_title, unit, interpretation), s.scale, 0., s.nodata)];
        d.default_layer = Some(s.layer.into()); d.years = vec![2026]; d.default_year = Some(2026);
        d.months = vec![s.month]; d.default_month = Some(s.month); d.discovery = Some("stac-items".into());
        d.license = "proprietary".into(); d.license_url = "https://earth.jaxa.jp/en/data/policy/".into();
        d.attribution = format!("Japan Aerospace Exploration Agency (JAXA), Earth Observation Research Center (EORC); {}. This data was obtained through JAXA Earth API. Retain named source-product credits, original publication interval, calibration, publisher Item and access date.", s.product);
        d.warnings = warnings.into_iter().map(str::to_owned).collect();
        d.warnings.extend([
            "Exact publisher Collection and standalone Items are archived unchanged. SSW Collection uses generic ID collection while its Items use the full product ID; no matching alias is invented. Only the finite archived source identity is eligible.",
            "Publisher Items use proprietary JAXA research-data terms. Public fee-free HTTPS is not a Creative Commons grant. Preserve JAXA and named third-party credits; the linked policy requires advance notification for commercial use. This adapter does not send that notification.",
            "No publisher file checksum is declared for these Items. Local SHA-256 receipts establish transfer identity against independent downloads, not publisher checksum verification. HTTPS originals are used without bucket credentials, S3 transport exceptions, account tokens or Google Earth Engine compute.",
        ].into_iter().map(str::to_owned));
        d.limits = vec![
            "Only the exact reviewed release and layer are indexed. Unsupported year, month, layer, country, named region or season fails explicitly. The selected area filters existing geographic tiles; no alternate demonstration area is inserted.".into(),
            "Complete original file transfers retain the 512 MiB and existing TIFF shape limits. A finite catalog is not per-file download acceptance: only one original per new product has been independently downloaded and fully decoded in this batch. The selected area does not crop, interpolate or create new numerical observations.".into(),
        ];
        d
    }).collect()
}

fn bounds(document: &Value) -> Result<[f64; 4]> {
    let value = serde_json::from_value(document["bbox"].clone()).map_err(crate::io_error)?;
    features::bounds(value)?;
    Ok(value)
}
fn source(item: &ReviewedItem, s: &Spec) -> Result<DatasetSource> {
    let prefix = format!("{}{}/{}/{}/", s.root, s.product, s.period, s.level);
    let region = item
        .url
        .strip_prefix(&prefix)
        .and_then(|r| r.strip_suffix(".json"))
        .ok_or("JAXA Item is outside the exact reviewed release, pyramid level and product")?;
    let date = s.period.replace('/', "-");
    let title = format!("JAXA {} {} · {}", s.asset, date, region.replace('/', " · "));
    Ok(DatasetSource {
        id: format!("{}-{}-{}", s.layer, date, region.replace('/', "-")),
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
    let s = spec(&request.dataset_id)?;
    if request.year != Some(2026)
        || request.month != Some(s.month)
        || request.layer.as_deref() != Some(s.layer)
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the exact reviewed JAXA release and original layer; no alternate product, daily acquisition, year, month, region or season is substituted".into());
    }
    let mut selected = Vec::new();
    for item in ITEMS.iter().filter(|i| i.dataset_id == s.id) {
        let b = bounds(&item.document)?;
        if request.bounds[0] < b[2]
            && request.bounds[2] > b[0]
            && request.bounds[1] < b[3]
            && request.bounds[3] > b[1]
        {
            selected.push(source(item, s)?);
        }
    }
    if selected.is_empty() || selected.len() > MAX_TILES {
        return Err("No reviewed JAXA geographic tile intersects this area or the area exceeds the eight-tile limit".into());
    }
    Ok(selected)
}
pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    let s = spec(&proof.dataset.id)?;
    if proof.schema_version != 2 || proof.catalog_raster.is_some() {
        return Err("Unsupported reviewed JAXA publisher-Item provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let selected = plan
        .sources
        .iter()
        .find(|source| {
            source.id == proof.source_id
                && source.stac.as_ref().is_some_and(|source| source.url == url)
        })
        .ok_or("JAXA source does not match its exact selected geographic release tile")?;
    if super::provenance(&plan, selected) != *proof {
        return Err("JAXA product, layer, request or scientific declaration differs from its immutable preset".into());
    }
    let item = ITEMS
        .iter()
        .find(|i| i.dataset_id == s.id && i.url == url)
        .ok_or("JAXA Item is outside the finite reviewed index")?;
    if item.collection_url != format!("{}{}/collection.json", s.root, s.product)
        || raw.is_some_and(|r| r != &item.document)
    {
        return Err("JAXA publisher Item changed: original interval, calibration, fill/error codes, relative asset, license or tile identity differs from review".into());
    }
    if item.document["assets"]
        .as_object()
        .is_none_or(|assets| assets.len() != 1 || !assets.contains_key(s.asset))
    {
        return Err("Reviewed JAXA Item has an unexpected measurement asset".into());
    }
    Ok(())
}
pub(crate) fn validate_collection(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<()> {
    let s = spec(&proof.dataset.id)?;
    let plan = super::resolve(proof.request.clone())?;
    let selected_url = plan
        .sources
        .iter()
        .find(|source| source.id == proof.source_id)
        .and_then(|source| source.stac.as_ref())
        .map(|source| source.url.as_str())
        .ok_or("JAXA Collection receipt has no exact selected publisher Item")?;
    validate(proof, selected_url, None)?;
    let collection = COLLECTIONS
        .iter()
        .find(|c| c.dataset_id == s.id && c.url == url)
        .ok_or("JAXA Collection URL is outside its exact reviewed endpoint")?;
    let id = if s.id == "jaxa-amsr2-wind" {
        "collection"
    } else {
        s.product
    };
    if url != format!("{}{}/collection.json", s.root, s.product)
        || raw != &collection.document
        || raw["type"] != "Collection"
        || raw["id"] != id
        || raw["license"] != "proprietary"
    {
        return Err("JAXA Collection identity, license, scientific metadata or catalog contents changed since review".into());
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
        .is_none_or(|links| links.iter().any(|l| l["rel"] == "self"))
    {
        return Err("Reviewed standalone JAXA Item unexpectedly has a self link".into());
    }
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn request(s: &Spec, b: [f64; 4]) -> ResolveRequest {
        serde_json::from_value(
            json!({"datasetId":s.id,"year":2026,"month":s.month,"layer":s.layer,"bounds":b}),
        )
        .unwrap()
    }
    #[test]
    fn exact_release_half_open_tiles_preserve_aoi_and_reject_substitution() {
        assert_eq!(ITEMS.len(), 12);
        assert_eq!(COLLECTIONS.len(), 3);
        for s in &SPECS {
            let input = request(s, [125., 30., 126., 31.]);
            let before = input.clone();
            let selected = resolve(&input).unwrap();
            assert_eq!(input, before);
            assert_eq!(selected.len(), 1);
            assert!(selected[0].stac.as_ref().unwrap().name.len() <= 80);
            assert!(selected[0].catalog_raster.is_none());
            assert_eq!(
                resolve(&request(s, [-180., -90., 180., 90.]))
                    .unwrap()
                    .len(),
                if s.id.ends_with("wind") { 8 } else { 2 }
            );
            let mut invalid = input.clone();
            invalid.month = Some(8);
            assert!(resolve(&invalid).is_err());
            invalid = input.clone();
            invalid.year = Some(2025);
            assert!(resolve(&invalid).is_err());
            invalid = input.clone();
            invalid.layer = Some("rgb".into());
            assert!(resolve(&invalid).is_err());
            invalid = input.clone();
            invalid.country = Some("JPN".into());
            assert!(resolve(&invalid).is_err());
            invalid = input.clone();
            invalid.region = Some("demo".into());
            assert!(resolve(&invalid).is_err());
            invalid = input;
            invalid.season = Some("summer".into());
            assert!(resolve(&invalid).is_err());
        }
    }
    #[test]
    fn immutable_provenance_keeps_scientific_values_intervals_and_metadata_conflicts() {
        for s in &SPECS {
            let plan = super::super::resolve(request(s, [125., 30., 126., 31.])).unwrap();
            let proof = super::super::provenance(&plan, &plan.sources[0]);
            let url = &plan.sources[0].stac.as_ref().unwrap().url;
            let raw = &ITEMS.iter().find(|i| &i.url == url).unwrap().document;
            assert!(validate(&proof, url, Some(raw)).is_ok());
            assert_eq!(self_url(&proof, url, raw).unwrap(), *url);
            assert!(transports(&proof, url, raw).unwrap().is_empty());
            assert_eq!(proof.layer.scale, Some(s.scale));
            assert_eq!(proof.layer.nodata, Some(s.nodata));
            assert_eq!(raw["properties"]["datetime"], Value::Null);
            for field in [
                "href", "scale", "nodata", "interval", "bbox", "roles", "license",
            ] {
                let mut changed = raw.clone();
                match field {
                    "href" => {
                        changed["assets"][s.asset]["href"] =
                            json!("https://example.com/alternate.tif")
                    }
                    "scale" => {
                        changed["assets"][s.asset]["je:rasters"]["dn2value"]["slope"] = json!(100)
                    }
                    "nodata" => changed["assets"][s.asset]["je:rasters"]["dn"]["nodata"] = json!(0),
                    "interval" => {
                        changed["properties"]["start_datetime"] = json!("2026-01-01T00:00:00Z")
                    }
                    "bbox" => changed["bbox"] = json!([0, 0, 90, 90]),
                    "roles" => changed["assets"][s.asset]["roles"] = json!(["overview"]),
                    _ => changed["properties"]["license"] = json!("CC-BY-4.0"),
                }
                assert!(
                    validate(&proof, url, Some(&changed)).is_err(),
                    "{} {field}",
                    s.id
                );
            }
            let mut changed = proof.clone();
            changed.layer.offset = Some(10.);
            assert!(validate(&changed, url, Some(raw)).is_err());
        }
    }
    #[test]
    fn exact_collection_archive_rejects_changed_document_or_cross_product_alias() {
        for s in &SPECS {
            let plan = super::super::resolve(request(s, [125., 30., 126., 31.])).unwrap();
            let proof = super::super::provenance(&plan, &plan.sources[0]);
            let c = COLLECTIONS.iter().find(|c| c.dataset_id == s.id).unwrap();
            assert!(validate_collection(&proof, &c.url, &c.document).is_ok());
            let mut changed = c.document.clone();
            changed["license"] = json!("CC-BY-4.0");
            assert!(validate_collection(&proof, &c.url, &changed).is_err());
            assert!(validate_collection(
                &proof,
                "https://example.com/collection.json",
                &c.document
            )
            .is_err());
        }
        let s = &SPECS[1];
        let source = resolve(&request(s, [125., 30., 126., 31.]))
            .unwrap()
            .remove(0);
        assert!(source.title.contains("2026-10-11"));
        assert_eq!(
            source.start_datetime.as_deref(),
            Some("2026-10-11T00:00:00Z")
        );
        assert_eq!(source.end_datetime.as_deref(), Some("2026-10-11T23:59:59Z"));
    }
}
