//! Two finite numerical PC actions under their original Planned family IDs.
//! Complete published COG derivatives are retained, not relabeled NASA HDF/NOAA NetCDF.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

pub(crate) const ADAPTER: &str = "planetary-anonymous-sas";
const API: &str = "https://planetarycomputer.microsoft.com/api/stac/v1";
pub(crate) const BURN_DATE_SHA256: &str =
    "6537d61e333a8e7e478affd9de6fc7752a4ac60cbf513340cde33b043a04d073";
pub(crate) const BURN_QA_SHA256: &str =
    "5f2f9800fcfad9aabc6bbe5beade4ccd5922fd589868212adb012b2bbe6b1820";
pub(crate) const OISST_SHA256: &str =
    "47b19bb75c95cdd302b1372a4d6a5a910c3e7213ccc013e6fee5f82f48646bfa";
const OISST_SCALE: f64 = 0.009999999776482582;
const MCD_DIMENSION: u32 = 2400;
const MCD_TRANSFORM: [f64; 6] = [
    463.3127165283333,
    0.,
    10007554.677014,
    0.,
    -463.3127165279167,
    4447802.078664,
];

struct Product {
    id: &'static str,
    collection: &'static str,
    title: &'static str,
    category: &'static str,
    description: &'static str,
    version: &'static str,
    resolution: &'static str,
    interpretation: &'static str,
    documentation: &'static str,
    license_url: &'static str,
    attribution: &'static str,
    layers: &'static [(&'static str, &'static str, &'static str)],
    account: &'static str,
    container: &'static str,
}
const PRODUCTS: &[Product] = &[
    Product {
        id: "planetary-more",
        collection: "noaa-cdr-sea-surface-temperature-optimum-interpolation",
        title: "NOAA OISST · Daily sea-surface temperature",
        category: "ocean",
        description: "One complete Planetary Computer published NOAA OISST v2.1 SST COG for 2020-09-01. This L4 analysis blends bias-adjusted satellite, ship and buoy observations and interpolates gaps; it is separate from a raw sensor scene, a current forecast or the original NOAA NetCDF. This finite action leaves the additional-Planetary-collections family Planned.",
        version: "OISST v2.1 · 2020-09-01 · fixed published SST COG derivative",
        resolution: "0.25° WGS84 Area grid · 1440 × 720 global cells; no coastal-resolution or local-accuracy guarantee",
        interpretation: "Original Int16 SST DN are retained. Publisher STAC Celsius scale is exactly 0.009999999776482582 with default offset 0; the original TIFF embeds scale 1 / offset 0, so raw DN and declared physical conversion remain separate. Scalar NoData is -999. Valid zero and negative temperatures are retained. Only 2020-09-01 00:00:00Z–23:59:59Z SST is supported: choosing September does not authorize other days, sea ice, SST anomaly or interpolation-error assets. A spatially filled analysis grid is not a direct observation at every pixel.",
        documentation: "https://www.ncei.noaa.gov/products/optimum-interpolation-sst",
        license_url: "https://www.ncei.noaa.gov/pub/data/sds/cdr/CDRs/Sea_Surface_Temperature_Optimum_Interpolation/UseAgreement_01B-09.pdf",
        attribution: "NOAA NCEI OISST v2.1; Huang et al. (2021), doi:10.1175/JCLI-D-20-0166.1; NOAA CDR Program; Microsoft Planetary Computer published COG derivative. Retain the exact daily Item, source metadata and access date. Preserve the original Collection proprietary field alongside the linked NOAA CDR Use Agreement, which states public unrestricted data use and requests scientific credit; do not replace that field with a guessed Creative Commons license.",
        layers: &[("sst", "2020-09-01 sea-surface temperature", "Celsius after declared scaling; original Int16 DN retained")],
        account: "noaacdr", container: "cogs",
    },
    Product {
        id: "modis-more", collection: "modis-64A1-061",
        title: "MODIS MCD64A1 · Monthly burned area",
        category: "land-cover",
        description: "One reviewed September 2020 h27v05 Terra+Aqua MCD64A1 V061 burned-area Item, with its complete published Burn_Date and separate QA COG assets. This product derives burned-area timing from surface-reflectance change and active-fire information; it is independent of NDVI, reflectance and land-cover classes. The finite COG action leaves the additional-MODIS-products/NASA-HDF family Planned.",
        version: "MCD64A1 V061 · September 2020 · h27v05 · published COG derivatives",
        resolution: "2400 × 2400 original MODLAND spherical sinusoidal Area grid; actual spacing about 463.312716528 m, nominal product resolution 500 m",
        interpretation: "Burn_Date is original signed Int16 ordinal day of burn within calendar year 2020: 1–366 denote mapped fire timing, 0 means unburned land, -1 is scalar NoData, and -2 is the separate water code, not scalar NoData. QA is a separate original UInt8 bit field with no embedded scalar NoData; zero remains a stored value. QA bit 0 is land/water, bit 1 data validity, bit 2 shortened mapping period, bit 3 contextual relabeling; bits 5–7 carry special unburned-cell conditions. The original grid uses sphere radius 6371007.181 m, not EPSG:4326. Grayscale of raw day/flag codes is browsing only; no fire severity, emissions, probability, hectares or independently confirmed fire event is inferred. The 2021-11-05 processing time and 2022 Blob modification time do not replace the September 2020 observation interval.",
        documentation: "https://ladsweb.modaps.eosdis.nasa.gov/filespec/MODIS/61/MCD64A1",
        license_url: "https://lpdaac.usgs.gov/data/data-citation-and-policies/",
        attribution: "Giglio, L., Justice, C., Boschetti, L., and Roy, D. (2021), MODIS/Terra+Aqua Burned Area Monthly L3 Global 500m SIN Grid V061, doi:10.5067/MODIS/MCD64A1.061; NASA LP DAAC at USGS EROS; Microsoft Planetary Computer distribution. Retain the source Collection proprietary license field and its LP DAAC citation/policy link, the September 2020 h27v05 Item, separate QA and access date. The LP DAAC policy page returned HTTP 403 during this review; no substitute guessed CC license is asserted.",
        layers: &[("Burn_Date", "Original ordinal day of burn", "day of year / original -2, -1 and 0 codes"),("QA", "Original burned-area quality flags", "bit field")],
        account: "modiseuwest", container: "modis-061-cogs",
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
    serde_json::from_str(include_str!("additional-pc-reviewed-items.json"))
        .expect("two unchanged finite Planetary Computer Items")
});
static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("additional-pc-reviewed-collections.json"))
        .expect("two unchanged finite Planetary Computer Collections")
});
pub(crate) fn is_product(id: &str) -> bool {
    PRODUCTS.iter().any(|p| p.id == id)
}
fn product(id: &str) -> Result<&'static Product> {
    PRODUCTS
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "Unsupported finite additional-PC product".into())
}
fn reviewed(p: &Product) -> Result<&'static ReviewedItem> {
    ITEMS
        .iter()
        .find(|v| v.dataset_id == p.id)
        .ok_or_else(|| "Exact reviewed additional-PC Item is missing".into())
}
pub(super) fn definitions() -> Vec<Dataset> {
    PRODUCTS.iter().map(|p| {
        let mut d = base(p.id, p.title, p.category, p.documentation);
        d.description=p.description.into(); d.version=p.version.into();
        d.nominal_resolution=p.resolution.into(); d.unit=p.layers[0].2.into();
        d.value_interpretation=p.interpretation.into(); d.license="proprietary".into();
        d.license_url=p.license_url.into(); d.attribution=p.attribution.into();
        d.layers=p.layers.iter().map(|(id,title,unit)| {
            let mut v=layer(id,title,unit,p.interpretation);
            v.scale=Some(if p.id=="planetary-more" { OISST_SCALE } else { 1. });
            v.offset=Some(0.);
            v.nodata=match *id { "sst"=>Some(-999.), "Burn_Date"=>Some(-1.), _=>None };
            v
        }).collect();
        d.default_layer=Some(p.layers[0].0.into()); d.years=vec![2020];
        d.default_year=Some(2020); d.months=vec![9]; d.default_month=Some(9);
        d.discovery=Some("stac-items".into());
        d.warnings=vec![p.interpretation.into(),
            "This finite action does not complete its original Planned parent family or authorize unreviewed collections, dates, tiles or assets. One product with multiple scientific layers still counts as one product.".into(),
            "AOI filters the publisher Item footprint and does not clip, reproject, replace the chosen location or promise useful valid pixels. Complete published COG derivatives are retained; NOAA NetCDF/NASA HDF originals are not claimed.".into(),
            "The original Item and Collection are immutable. Only the selected reviewed numerical asset receives transient anonymous read-only PC transport; SAS capabilities are never persisted in source metadata, settings or code.".into()];
        d.limits=vec!["Exactly one named Item per product, fixed September 2020. OISST only September 1 SST; MCD64A1 only h27v05 Burn_Date or QA. No other day, tile, original HDF/NetCDF, variable or family completion is inferred.".into(),
            "Existing 512 MiB transfer, raster dimension, chunk, preview and decoder limits remain unchanged. No user account, credentials, paid cloud work, generated subset or commercial procurement is used.".into()];
        d
    }).collect()
}
pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    let p = product(&request.dataset_id)?;
    if request.year != Some(2020)
        || request.month != Some(9)
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
        || !p
            .layers
            .iter()
            .any(|(id, _, _)| request.layer.as_deref() == Some(*id))
    {
        return Err("Choose the exact reviewed September 2020 product and original layer; alternate days, tiles, dates or variables are not substituted".into());
    }
    let item = reviewed(p)?;
    let b: [f64; 4] =
        serde_json::from_value(item.document["bbox"].clone()).map_err(crate::io_error)?;
    features::bounds(b)?;
    if !(request.bounds[0] < b[2]
        && request.bounds[2] > b[0]
        && request.bounds[1] < b[3]
        && request.bounds[3] > b[1])
        || (p.id == "modis-more" && !intersects_modis_grid(request.bounds))
    {
        return Err("No reviewed publisher Item intersects this area; the selected location will not be replaced".into());
    }
    Ok(vec![DatasetSource {
        id: item.document["id"]
            .as_str()
            .ok_or("Reviewed Item ID missing")?
            .into(),
        title: format!("{} · {}", p.title, item.document["id"].as_str().unwrap()),
        bounds: Some(b),
        stac: Some(stac::ConnectRequest {
            name: format!("{} September 2020", p.title),
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
            .map(str::to_owned),
        end_datetime: item.document["properties"]["end_datetime"]
            .as_str()
            .map(str::to_owned),
        catalog_raster: None,
    }])
}
/// Exact positive-area intersection with this finite northern/eastern MODLAND
/// tile, rather than its much wider WGS84 bbox. Across the clipped latitude
/// interval x = R * longitude * cos(latitude), so the allowable cos interval
/// can be solved continuously. Testing only projected AOI corners would also
/// miss valid AOIs that contain the tile. This tests the grid, not valid data.
fn intersects_modis_grid(bounds: [f64; 4]) -> bool {
    let radius = crate::providers::modis::RADIUS;
    let xmin = MCD_TRANSFORM[2];
    let xmax = xmin + MCD_TRANSFORM[0] * f64::from(MCD_DIMENSION);
    let ymax = MCD_TRANSFORM[5];
    let ymin = ymax + MCD_TRANSFORM[4] * f64::from(MCD_DIMENSION);
    let south = bounds[1].max((ymin / radius).to_degrees());
    let north = bounds[3].min((ymax / radius).to_degrees());
    if south >= north || bounds[2] <= 0. {
        return false;
    }
    let minimum_cos = north.to_radians().cos();
    let maximum_cos = south.to_radians().cos();
    let east_threshold = xmin / (radius * bounds[2].to_radians());
    let west_threshold = if bounds[0] > 0. {
        xmax / (radius * bounds[0].to_radians())
    } else {
        f64::INFINITY
    };
    minimum_cos.max(east_threshold) < maximum_cos.min(west_threshold)
}
pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    let p = product(&proof.dataset.id)?;
    if proof.schema_version != 2 || proof.catalog_raster.is_some() {
        return Err("Unsupported finite additional-PC source proof".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let source = plan
        .sources
        .iter()
        .find(|s| s.id == proof.source_id && s.stac.as_ref().is_some_and(|r| r.url == url))
        .ok_or("Additional-PC source differs from its exact reviewed request and Item")?;
    if super::provenance(&plan, source) != *proof {
        return Err("Additional-PC product, layer, time, raw encoding, calibration or license differs from its immutable preset".into());
    }
    let item = reviewed(p)?;
    if item.url != url
        || item.collection_url != format!("{API}/collections/{}", p.collection)
        || item.document["collection"] != p.collection
        || raw.is_some_and(|v| v != &item.document)
    {
        return Err("Exact additional-PC publisher Item changed: identity, footprint, original grid, date, scientific codes, roles, license or asset href differs".into());
    }
    let a = &item.document["assets"][&proof.layer.id];
    let href = a["href"]
        .as_str()
        .ok_or("Selected reviewed numerical asset missing")?;
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
            .is_some_and(|v| v.starts_with("image/tiff"))
        || a["roles"] != serde_json::json!(["data"])
    {
        return Err(
            "Selected original is not the reviewed unsigned canonical PC numerical TIFF".into(),
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
        .find(|v| v.dataset_id == p.id && v.url == url)
        .ok_or("Additional-PC Collection URL is outside its reviewed endpoint")?;
    if url != format!("{API}/collections/{}", p.collection)
        || raw != &c.document
        || raw["id"] != p.collection
        || raw["type"] != "Collection"
        || raw["license"] != "proprietary"
    {
        return Err("Additional-PC Collection identity, scientific declarations or original license changed".into());
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
        .ok_or("PC asset href missing")?
        .to_owned();
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
        ls.iter().filter(|v| v["rel"] == "self").count() != 1
            || !ls.iter().any(|v| v["rel"] == "self" && v["href"] == url)
    }) {
        return Err("Reviewed additional-PC self URL differs from the exact endpoint".into());
    }
    Ok(url.into())
}
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
            "PC signing permission is bound to the one selected reviewed original asset".into(),
        );
    }
    Ok((p.account, p.container))
}
/// Full-file references are a separate reviewed client policy. They never
/// manufacture publisher file:size/checksum fields or permit another layer's
/// bytes under the selected canonical original, even when its grid matches.
pub(crate) fn validate_original(
    proof: &SourceProvenance,
    url: &str,
    bytes: u64,
    sha256: &str,
) -> Result<()> {
    let p = product(&proof.dataset.id)?;
    let item = reviewed(p)?;
    validate(proof, &item.url, None)?;
    let (expected_bytes, expected_sha) = match (p.id, proof.layer.id.as_str()) {
        ("planetary-more", "sst") => (1_096_226, OISST_SHA256),
        ("modis-more", "Burn_Date") => (112_419, BURN_DATE_SHA256),
        ("modis-more", "QA") => (811_891, BURN_QA_SHA256),
        _ => return Err("Original reference is outside the finite selected layer".into()),
    };
    if item.document["assets"][&proof.layer.id]["href"] != url
        || bytes != expected_bytes
        || sha256 != expected_sha
    {
        return Err("Complete original differs from the selected reviewed additional-PC canonical layer, byte length or SHA-256".into());
    }
    Ok(())
}
/// Allows only the two independently transferred original MCD64A1 byte identities.
/// The caller must additionally run the existing physical MODIS sphere/CRS validator.
pub(crate) fn is_modis_original_sha(sha: &str) -> bool {
    matches!(sha, BURN_DATE_SHA256 | BURN_QA_SHA256)
}
/// No catalog CRS or file name can substitute for the physical original tile grid.
pub(crate) fn validate_modis_grid(m: &crate::stac::GenericRasterInspection) -> Result<()> {
    let (dtype, nodata) = match m.sha256.as_str() {
        BURN_DATE_SHA256 => ("Int16", serde_json::json!(-1)),
        BURN_QA_SHA256 => ("UInt8", Value::Null),
        _ => {
            return Err(
                "MCD64A1 original byte identity is outside the reviewed finite scope".into(),
            )
        }
    };
    if m.width != MCD_DIMENSION
        || m.height != MCD_DIMENSION
        || m.storage_layout != "tile"
        || m.bands.len() != 1
        || m.bands[0].data_type != dtype
        || m.bands[0].nodata != nodata
        || m.crs.as_deref() != Some(crate::providers::modis::CRS)
        || m.pixel_interpretation.as_deref() != Some("area")
        || m.transform.as_ref().is_none_or(|actual| {
            actual
                .iter()
                .zip(MCD_TRANSFORM)
                .any(|(a, b)| !a.is_finite() || (*a - b).abs() > 1e-8)
        })
    {
        return Err("Reviewed MCD64A1 raster differs from its original h27v05 dimensions, codes, Area grid or metre transform".into());
    }
    Ok(())
}

#[cfg(test)]
#[path = "additional_pc/tests.rs"]
mod tests;
