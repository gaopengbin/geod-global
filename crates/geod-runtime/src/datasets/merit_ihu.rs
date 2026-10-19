//! Unchanged author-published IHU derivatives. Never substitutes for protected
//! original 3 arc-second MERIT Hydro. Ordinary file/grid/decode limits apply.
use super::*;
use std::sync::LazyLock;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Index {
    originals: Vec<Original>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Original {
    id: String,
    layer: String,
    url: String,
    bytes: u64,
    sha256: String,
}
static INDEX: LazyLock<Index> = LazyLock::new(|| {
    serde_json::from_str(include_str!("merit-ihu-reviewed-index.json"))
        .expect("Reviewed complete IHU originals")
});
// Geographic selection footprint rounded to the publisher grid definition.
// The TIFF's exact affine (east 180.00000000004087) is never rewritten.
const FOOTPRINT: [f64; 4] = [-180., -60., 180., 85.];

pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "merit-hydro",
        "MERIT Hydro IHU · 5 arc-minute water networks",
        "hydrology",
        "https://zenodo.org/records/7936280",
    );
    d.description="Three complete author-published 5 arc-minute IHU v1.0 GeoTIFFs: D8 flow direction, upstream area and Strahler stream order. IHU is an author-upscaled derivative, not the protected original 3 arc-second (~90 m) MERIT Hydro. Parent original access remains pending.".into();
    d.version = "MERIT Hydro IHU v1.0; fixed record 7936280".into();
    d.license = "Dual CC BY-NC 4.0 or ODbL 1.0".into();
    d.license_url = "https://zenodo.org/records/7936280/files/LICENSE".into();
    d.attribution="Eilander, Winsemius, Van Verseveld, Yamazaki, Weerts and Ward, MERIT Hydro IHU v1.0, doi:10.5281/zenodo.7936280; based on Yamazaki et al. (2019), doi:10.1029/2019WR024873. Preserve the original README dual-license statement and ODbL notice; publication date is not acquisition time.".into();
    d.nominal_resolution="5 arc-minute (approximately 10 km) author-upscaled grid, 4320 × 1740, WGS84 Area; not 90 m".into();
    d.unit = "D8 code, km², or Strahler order (separate layers)".into();
    let flow="Unchanged UInt8 D8 direction codes: 1 east, 2 southeast, 4 south, 8 southwest, 16 west, 32 northwest, 64 north, 128 northeast. Zero is a valid river mouth. The README describes ocean as -9; the actual unsigned TIFF stores embedded NoData 247. Do not reinterpret 247 as a valid direction or change valid zero to NoData.";
    let area="Unchanged Float64 accumulated upstream sub-grid area in km², embedded NoData -9999, scale 1 and offset 0. This is a modeled hydrography quantity, not instantaneous river flow, runoff, catchment accuracy or rainfall. No conversion to Float32 or per-pixel area is applied.";
    let order="Unchanged UInt8 Strahler stream order, embedded NoData 255. Values in the reviewed file are 1–9, not river width, depth or discharge. No inferred color legend or remapped class codes.";
    d.layers = vec![
        scaled(
            layer(
                "flwdir",
                "IHU 5 arc-minute · D8 flow direction",
                "D8 code",
                flow,
            ),
            1.,
            0.,
            247.,
        ),
        scaled(
            layer("uparea", "IHU 5 arc-minute · Upstream area", "km²", area),
            1.,
            0.,
            -9999.,
        ),
        scaled(
            layer(
                "strord",
                "IHU 5 arc-minute · Strahler stream order",
                "Strahler order",
                order,
            ),
            1.,
            0.,
            255.,
        ),
    ];
    d.default_layer = Some("flwdir".into());
    d.value_interpretation = format!("{flow} {area} {order}");
    d.warnings=vec!["This IHU release is author-upscaled from MERIT Hydro, not the original 90 m product. Only these three fixed 5 arc-minute files are enabled. Original 30 arc-second and 15 arc-minute products, other layers and 90 m access remain outside this action.".into(),"The selected area chooses one complete global file, not a clipped subset. The rounded -180/-60/180/85 geographic selection footprint does not rewrite the TIFF's exact affine or establish useful valid-data coverage throughout an area.".into(),"Static modeled hydrography; the 2020 release and 2023 record update are not observation dates. No time filter, physical discharge calibration or scientific accuracy validation is inferred.".into()];
    d.limits=vec!["Each complete source must match the reviewed URL, whole byte count and SHA-256. Ordinary 512 MiB transfer, grid, block, cumulative decode, preview-memory and deadline limits remain unchanged. No credentials or Earth Engine compute are used.".into()];
    vec![d]
}

pub(super) fn resolve(r: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(r.bounds)?;
    if r.dataset_id != "merit-hydro"
        || r.year.is_some()
        || r.month.is_some()
        || r.country.is_some()
        || r.region.is_some()
        || r.season.is_some()
    {
        return Err("Choose the fixed IHU 5 arc-minute product without year, month, country, region or season filters; 90 m originals are not substituted".into());
    }
    let t = INDEX
        .originals
        .iter()
        .find(|t| Some(t.layer.as_str()) == r.layer.as_deref())
        .ok_or("Choose IHU flow direction, upstream area or Strahler stream order")?;
    if r.bounds[0] >= FOOTPRINT[2]
        || r.bounds[2] <= FOOTPRINT[0]
        || r.bounds[1] >= FOOTPRINT[3]
        || r.bounds[3] <= FOOTPRINT[1]
    {
        return Err("The selected area is outside the IHU 60S–85N original footprint".into());
    }
    let mut s = source(
        format!(
            "MERIT Hydro IHU v1.0 · {} · complete 5 arc-minute original",
            t.layer
        ),
        t.url.clone(),
        Some(FOOTPRINT),
    );
    s.id = t.id.clone();
    Ok(vec![s])
}
pub(crate) fn validate_original(
    p: &SourceProvenance,
    url: &str,
    bytes: u64,
    sha: &str,
) -> Result<()> {
    super::validate_provenance(p, url)?;
    let t = INDEX
        .originals
        .iter()
        .find(|t| t.id == p.source_id)
        .ok_or("Unreviewed MERIT Hydro IHU original")?;
    if p.dataset.id != "merit-hydro"
        || p.request.layer.as_deref() != Some(t.layer.as_str())
        || p.layer.id != t.layer
        || p.source_bounds != Some(FOOTPRINT)
        || url != t.url
        || bytes != t.bytes
        || sha != t.sha256
    {
        return Err("IHU original differs from its exact selected layer, source URL, complete bytes or SHA-256".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ResolveRequest {
        ResolveRequest {
            dataset_id: "merit-hydro".into(),
            bounds: [116., 39., 117., 40.],
            year: None,
            month: None,
            country: None,
            layer: Some("flwdir".into()),
            region: None,
            season: None,
        }
    }
    #[test]
    fn ihu_original_scope_retains_derived_resolution_and_actual_unsigned_nodata() {
        let d = definitions().remove(0);
        assert_eq!(d.layers.len(), 3);
        assert!(d.years.is_empty());
        assert!(d.description.contains("not the protected original"));
        assert!(d.layers[0].value_interpretation.contains("Zero is a valid"));
        assert_eq!(d.layers[0].nodata, Some(247.));
        assert_eq!(d.layers[1].unit, "km²");
        assert!(d.layers[1].value_interpretation.contains("Float64"));
        assert_eq!(d.layers[2].nodata, Some(255.));
    }
    #[test]
    fn ihu_wrong_filters_and_outside_area_never_fall_back() {
        for n in 0..8 {
            let mut r = request();
            match n {
                0 => r.year = Some(2020),
                1 => r.month = Some(1),
                2 => r.country = Some("CHN".into()),
                3 => r.region = Some("global".into()),
                4 => r.season = Some("annual".into()),
                5 => r.layer = Some("elevation".into()),
                6 => r.bounds = [0., 86., 1., 87.],
                _ => r.dataset_id = "merit-90m".into(),
            };
            assert!(resolve(&r).is_err());
        }
    }
    #[test]
    fn ihu_layer_swaps_changed_proofs_and_complete_file_substitutions_fail() {
        for t in &INDEX.originals {
            let mut r = request();
            r.layer = Some(t.layer.clone());
            let plan = super::super::resolve(r).unwrap();
            let p = super::super::provenance(&plan, &plan.sources[0]);
            validate_original(&p, &t.url, t.bytes, &t.sha256).unwrap();
            assert!(validate_original(&p, &t.url, t.bytes - 1, &t.sha256).is_err());
            assert!(validate_original(&p, &t.url, t.bytes, &"0".repeat(64)).is_err());
            let other = INDEX.originals.iter().find(|v| v.id != t.id).unwrap();
            assert!(validate_original(&p, &other.url, other.bytes, &other.sha256).is_err());
            let mut q = p.clone();
            q.dataset.license = "CC BY 4.0".into();
            assert!(validate_original(&q, &t.url, t.bytes, &t.sha256).is_err());
            let mut q = p;
            q.source_bounds = Some([-180., -90., 180., 90.]);
            assert!(validate_original(&q, &t.url, t.bytes, &t.sha256).is_err());
        }
    }
}
