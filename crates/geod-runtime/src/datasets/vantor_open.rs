//! One unchanged visual JPEG/YCbCr original from the current Vantor program.
//! Encoded display components never acquire a scientific RGB calibration.
use super::*;
use std::sync::LazyLock;
const ID: &str = "vantor-open";
pub(crate) const ORIGINAL_SHA256: &str =
    "54833b631a8e999ccb6b2aebd409648f79d2a4ece60041b94978aeec009c6f3a";
static INDEX: LazyLock<serde_json::Value> = LazyLock::new(|| {
    serde_json::from_str(include_str!("vantor-reviewed-index.json"))
        .expect("Reviewed complete Vantor visual original")
});

pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        ID,
        "Vantor Open Data · Nepal September 2026 visual original",
        "imagery",
        "https://vantor.com/open-data/",
    );
    d.discovery = Some("stac-items".into());
    d.description="Finite integration of one actual current Vantor Open Data disaster archive visual: Nepal-Flooding-Aug-2026, original item B120001101250A10 observed on 2026-09-07. The complete publisher JPEG-compressed GeoTIFF is preserved. This does not provide commercial imagery ordering, global coverage or support for every event asset.".into();
    d.version = "Nepal-Flooding-Aug-2026 · B120001101250A10 · 2026-09-07 visual original".into();
    d.license = "CC-BY-NC-4.0".into();
    d.license_url = "https://creativecommons.org/licenses/by-nc/4.0/".into();
    d.attribution="Vantor Open Data Program, Nepal-Flooding-Aug-2026, Legion LG02, item B120001101250A10; acquisition 2026-09-07T05:07:53.249593Z. Preserve original item and event Collection, source time, CC BY-NC 4.0 attribution and noncommercial restrictions.".into();
    d.nominal_resolution="Original EPSG:4326 36389 × 41157 canvas, spacing 0.000005190520394450596° × 0.0000045967538055144995°. Publisher sensor pan/multispectral GSD remains separate.".into();
    d.unit = "JPEG display Y/Cb/Cr components".into();
    d.value_interpretation="TIFF stores lossy JPEG YCbCr with 2×2 chroma subsampling, not scientific red/green/blue samples. Native inspection reports JPEG-decoded UInt8 Y, Cb, Cr components on the original pixel grid; chroma is upsampled with the codec's triangular fancy interpolation. These are not the original subsampled chroma planes, calibrated reflectance or a conversion to RGB. Publisher eo:bands logical red/green/blue declarations remain unchanged and separate. Preview stretches only Y luminance in grayscale.".into();
    d.years = vec![2026];
    d.default_year = Some(2026);
    d.months = vec![9];
    d.default_month = Some(9);
    d.layers=vec![layer("visual","Visual original · JPEG Y/Cb/Cr","display component","Complete original visual TIFF; raw pixel arrays report Y, Cb, Cr after JPEG chroma upsampling. Luminance-only grayscale display. No scientific RGB calibration, analytic-band replacement or damage assessment.")];
    d.default_layer = Some("visual".into());
    d.warnings=vec![
        "Only exact item B120001101250A10 and the independently hashed complete 29,534,465-byte TIFF are supported. Other current events and visual profiles remain unverified; unsupported requests return no replacement.".into(),
        "The event name contains August, but this item's actual observation instant is 2026-09-07T05:07:53.249593Z. Event date, publication and ingestion dates do not replace acquisition time.".into(),
        "Publisher cloud cover is 93 percent. Footprint intersection and the visual preview do not establish usable AOI coverage, cloud-free imagery or flood damage. The TIFF declares no scalar NoData: noData=false and black canvas pixels do not prove usable observations.".into(),
        "The original lossy JPEG components are display values. Full-resolution chroma values include codec spatial upsampling and decoder-dependent rounding; they are not lossless sensor samples or scientifically calibrated RGB bands.".into(),
        "The event Collection declares CC-BY-NC-4.0. Anonymous access or a GeoD subscription does not grant commercial reuse or imagery procurement rights. Preserve original publisher metadata, attribution and noncommercial restrictions.".into(),
        "The source's eo:bands RGB labels are retained as publisher display declarations. Actual TIFF sampleEncoding is reported separately as Y/Cb/Cr; no publisher metadata or original bytes are rewritten.".into(),
    ];
    d.limits=vec!["One exact reviewed current Vantor item and visual asset only, year 2026/month 9. Existing 512 MiB transfer/preview, TIFF axis/grid/chunk budgets and deadlines are unchanged. Embedded reduced JPEG images are used only after matching sample/codec declarations; COG conformance is not certified.".into()];
    vec![d]
}
pub(super) fn resolve(r: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(r.bounds)?;
    if r.dataset_id != ID
        || r.layer.as_deref() != Some("visual")
        || r.year != Some(2026)
        || r.month != Some(9)
        || r.country.is_some()
        || r.region.is_some()
        || r.season.is_some()
    {
        return Err("Choose the reviewed September 2026 Nepal visual original; other dates, events, layers or regions are not substituted".into());
    }
    let b: [f64; 4] =
        serde_json::from_value(INDEX["document"]["bbox"].clone()).map_err(crate::io_error)?;
    if r.bounds[0] >= b[2] || r.bounds[2] <= b[0] || r.bounds[1] >= b[3] || r.bounds[3] <= b[1] {
        return Err("No reviewed Vantor visual original overlaps this area".into());
    }
    let instant = INDEX["document"]["properties"]["datetime"]
        .as_str()
        .ok_or("Missing original acquisition time")?;
    let title = "Nepal · LG02 · 2026-09-07 · B120001101250A10".to_string();
    Ok(vec![DatasetSource {
        id: "B120001101250A10".into(),
        title: title.clone(),
        bounds: Some(b),
        stac: Some(stac::ConnectRequest {
            name: title,
            url: INDEX["itemUrl"].as_str().unwrap().into(),
            kind: "item".into(),
        }),
        wcs: None,
        coverage_id: None,
        search: None,
        region: None,
        season: None,
        start_datetime: Some(instant.into()),
        end_datetime: Some(instant.into()),
        catalog_raster: None,
    }])
}
pub(crate) fn validate(
    p: &SourceProvenance,
    url: &str,
    raw: Option<&serde_json::Value>,
) -> Result<()> {
    if p.schema_version != 2 || p.dataset.id != ID || p.catalog_raster.is_some() {
        return Err("Unsupported Vantor visual provenance".into());
    }
    let plan = super::resolve(p.request.clone())?;
    let s = plan
        .sources
        .iter()
        .find(|s| s.id == p.source_id && s.stac.as_ref().is_some_and(|v| v.url == url))
        .ok_or("Vantor source differs from the exact reviewed item")?;
    if super::provenance(&plan, s) != *p
        || url != INDEX["itemUrl"].as_str().unwrap()
        || raw.is_some_and(|v| v != &INDEX["document"])
    {
        return Err(
            "Vantor original item, time, layer, bounds or immutable declaration changed".into(),
        );
    }
    Ok(())
}
pub(crate) fn transports(
    p: &SourceProvenance,
    url: &str,
    raw: &serde_json::Value,
) -> Result<Vec<stac::AssetTransport>> {
    validate(p, url, Some(raw))?;
    Ok(vec![])
}
pub(crate) fn self_url(p: &SourceProvenance, url: &str, raw: &serde_json::Value) -> Result<String> {
    validate(p, url, Some(raw))?;
    Ok(url.into())
}
pub(crate) fn validate_collection(
    p: &SourceProvenance,
    url: &str,
    raw: &serde_json::Value,
) -> Result<()> {
    validate(p, INDEX["itemUrl"].as_str().unwrap(), None)?;
    if url != INDEX["collectionUrl"].as_str().unwrap()
        || raw != &INDEX["collection"]
        || raw["license"] != "CC-BY-NC-4.0"
    {
        return Err("Vantor event collection, licence or item membership changed".into());
    }
    Ok(())
}
pub(crate) fn visual_asset(
    p: &SourceProvenance,
    url: &str,
    key: &str,
    asset: &serde_json::Value,
) -> Result<()> {
    validate(p, url, None)?;
    if key != "visual" || asset != &INDEX["document"]["assets"]["visual"] {
        return Err(
            "Only the exact reviewed visual asset is eligible for this Vantor recipe".into(),
        );
    }
    Ok(())
}
pub(crate) fn visual_profile(m: &stac::GenericRasterInspection) -> bool {
    m.sha256 == ORIGINAL_SHA256
        && m.width == 36389
        && m.height == 41157
        && m.storage_layout == "tile"
        && m.bands.len() == 3
        && m.bands
            .iter()
            .all(|v| v.data_type == "UInt8" && v.nodata.is_null())
        && m.crs.as_deref() == Some("EPSG:4326")
        && m.pixel_interpretation.as_deref() == Some("area")
        && m.source_model_pixel_scale == Some([5.190520394450596e-6, 4.5967538055144995e-6, 0.])
        && m.source_model_tiepoint == Some([0., 0., 0., 85.37344389401046, 28.332368860911252, 0.])
}
pub(crate) fn validate_original(
    p: &SourceProvenance,
    href: &str,
    bytes: u64,
    sha256: &str,
) -> Result<()> {
    validate(p, INDEX["itemUrl"].as_str().unwrap(), None)?;
    if href != INDEX["original"]["assetUrl"].as_str().unwrap()
        || bytes != 29_534_465
        || sha256 != ORIGINAL_SHA256
    {
        return Err("Vantor complete visual original differs from its exact independently reviewed bytes or SHA-256; this is a local source-bound receipt, not a publisher checksum declaration".into());
    }
    Ok(())
}
#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ResolveRequest {
        ResolveRequest {
            dataset_id: ID.into(),
            bounds: [85.4, 28.2, 85.45, 28.25],
            year: Some(2026),
            month: Some(9),
            country: None,
            layer: Some("visual".into()),
            region: None,
            season: None,
        }
    }
    #[test]
    fn original_visual_proof_preserves_actual_september_time_publisher_rgb_and_noncommercial_licence(
    ) {
        let plan = super::super::resolve(request()).unwrap();
        let p = super::super::provenance(&plan, &plan.sources[0]);
        validate(
            &p,
            INDEX["itemUrl"].as_str().unwrap(),
            Some(&INDEX["document"]),
        )
        .unwrap();
        validate_collection(
            &p,
            INDEX["collectionUrl"].as_str().unwrap(),
            &INDEX["collection"],
        )
        .unwrap();
        assert!(plan.sources[0]
            .start_datetime
            .as_ref()
            .unwrap()
            .starts_with("2026-09-07"));
        assert_eq!(
            INDEX["document"]["assets"]["visual"]["roles"],
            serde_json::json!(["visual"])
        );
        assert_eq!(plan.dataset.license, "CC-BY-NC-4.0");
        let mut raw = INDEX["document"].clone();
        raw["properties"]["datetime"] = serde_json::json!("2026-08-26T03:30:00Z");
        assert!(validate(&p, INDEX["itemUrl"].as_str().unwrap(), Some(&raw)).is_err());
    }
    #[test]
    fn no_other_visual_scene_date_analytic_or_aoi_is_substituted() {
        for n in 0..5 {
            let mut r = request();
            match n {
                0 => r.month = Some(8),
                1 => r.year = Some(2025),
                2 => r.layer = Some("ms_analytic".into()),
                3 => r.bounds = [116., 39., 117., 40.],
                _ => r.country = Some("NPL".into()),
            };
            assert!(resolve(&r).is_err());
        }
    }
}
