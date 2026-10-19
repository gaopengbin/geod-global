//! Original WorldClim 2.1 national stacks. This reviewed scope is two Nepal
//! originals, not a generated subset, global coverage or an annual time series.
use super::*;

const BOUNDS: [f64; 4] = [80., 26., 88.5, 30.5];
const PREFIX: &str = "https://geodata.ucdavis.edu/climate/worldclim/2_1/tiles/iso/";

pub(crate) struct Original {
    pub layer: &'static str,
    pub name: &'static str,
    pub bytes: u64,
    pub sha256: &'static str,
}
pub(crate) const ORIGINALS: [Original; 2] = [
    Original {
        layer: "tavg",
        name: "NPL_wc2.1_30s_tavg.tif",
        bytes: 6_827_324,
        sha256: "68363b342d56bb60066c372875c0a09fd931885f429a2138b2cad7dffec0eaff",
    },
    Original {
        layer: "prec",
        name: "NPL_wc2.1_30s_prec.tif",
        bytes: 4_524_406,
        sha256: "f6707d2169193e53d86b084f8122a1c6885fa1a1e76435490f0d4fe146c0b7fb",
    },
];

pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "worldclim",
        "WorldClim 2.1 · historical climate normals",
        "climate",
        "https://www.worldclim.org/data/worldclim21.html",
    );
    d.description = "Publisher-distributed Nepal 30 arc-second national stacks of monthly mean temperature and precipitation for the 1970–2000 climate-normal period. Each unchanged GeoTIFF contains January–December as 12 numerical bands. This reviewed scope does not include annual weather, future scenarios, a global grid or other national files.".into();
    d.version = "WorldClim 2.1 · 1970–2000 normals · NPL 30s originals".into();
    d.license = "WorldClim terms: academic and other non-commercial use; redistribution or commercial use requires prior permission".into();
    d.license_url = "https://worldclim.org/about.html".into();
    d.attribution = "Fick, S.E. and R.J. Hijmans (2017), WorldClim 2: new 1km spatial resolution climate surfaces for global land areas, International Journal of Climatology 37(12), 4302–4315, doi:10.1002/joc.5086. Retain WorldClim 2.1, 1970–2000, NPL, the selected variable and access date. The data do not carry a Creative Commons redistribution licence; permission for academic article maps does not relicense these source files.".into();
    d.nominal_resolution =
        "30 arc-seconds (1/120 degree); original 1020 × 540 geographic grid".into();
    d.requires_country = true;
    d.unit = "temperature: °C; precipitation: mm per normal month".into();
    d.value_interpretation = "Bands 1–12 are January–December normals for 1970–2000, not acquisitions or separate observed years. Temperature is stored directly as Float32 degrees Celsius; precipitation as Int16 millimetres. Scale 1, offset 0; do not apply the WorldClim v1 ×0.1 temperature convention to these v2.1 files. Native preview displays band 1 (January); raw-pixel inspection preserves all 12 original band values.".into();
    d.layers = vec![
        scaled(layer("tavg", "Monthly mean temperature · January–December stack", "°C", "Original Float32 monthly mean temperature in degrees Celsius; bands 1–12 January–December, 1970–2000 normals, scale 1 and offset 0. Negative temperatures and zero are valid. The TIFF NoData declaration is -3.4e38 and its Float32 stored representation is -3.3999999521443642e38."), 1., 0., -3.4e38),
        scaled(layer("prec", "Monthly precipitation · January–December stack", "mm", "Original Int16 millimetres of precipitation for each normal calendar month, bands 1–12 January–December, 1970–2000. Scale 1, offset 0, NoData -32768. Zero is valid precipitation; no annual summation, rate conversion or rounding is performed."), 1., 0., -32768.),
    ];
    d.default_layer = Some("tavg".into());
    d.warnings = vec![
        "Only ISO3 NPL and the original Nepal temperature / precipitation national stacks are independently reviewed. The source rectangle includes neighbouring territory and is not a Nepal administrative mask. Intersecting it does not prove valid pixels or accuracy throughout the selected area.".into(),
        "Each complete 12-band file is downloaded without clipping or selecting a month. The map preview is January only; inspect all original bands for other months. Year, month, region and season selectors are unavailable for this fixed climate-normal scope.".into(),
        "WorldClim permits academic and other non-commercial use. Redistribution and commercial use require prior permission from WorldClim; academic article-map permissions do not grant general redistribution rights for original data or a commercial licence.".into(),
    ];
    d.limits = vec!["Two reviewed original files, selected one variable at a time, for NPL only. Each retains exact publisher bytes, 1020 × 540 × 12 samples and EPSG:4326 Area registration. Existing 512 MiB transfer and all native decoding / preview budgets remain unchanged.".into()];
    vec![d]
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != "worldclim"
        || request.country.as_deref() != Some("NPL")
        || request.year.is_some()
        || request.month.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose ISO3 NPL for the reviewed WorldClim 2.1 1970–2000 national stack; it has no year, month, region or season selector and no alternate file is substituted".into());
    }
    let original = ORIGINALS
        .iter()
        .find(|v| Some(v.layer) == request.layer.as_deref())
        .ok_or("Choose the reviewed WorldClim tavg or prec original stack")?;
    if !(request.bounds[0] < BOUNDS[2]
        && request.bounds[2] > BOUNDS[0]
        && request.bounds[1] < BOUNDS[3]
        && request.bounds[3] > BOUNDS[1])
    {
        return Err("The selected area does not intersect the actual reviewed Nepal original grid; no demonstration area, national mask or alternative country is substituted".into());
    }
    let mut s = source(
        format!("WorldClim 2.1 NPL {} · 1970–2000 normals", original.layer),
        format!("{PREFIX}{}", original.name),
        Some(BOUNDS),
    );
    s.id = original.name.trim_end_matches(".tif").into();
    Ok(vec![s])
}

/// The current publication must still match the independently accepted whole
/// file. A response size, TIFF header or publisher filename alone is insufficient.
pub(crate) fn validate_original(
    proof: &SourceProvenance,
    url: &str,
    bytes: u64,
    sha256: &str,
) -> Result<()> {
    let sources = resolve(&proof.request)?;
    let original = ORIGINALS
        .iter()
        .find(|v| v.layer == proof.layer.id)
        .ok_or("Unreviewed WorldClim layer")?;
    if proof.dataset.id != "worldclim"
        || proof.request.layer.as_deref() != Some(proof.layer.id.as_str())
        || proof.source_id != sources[0].id
        || sources[0].stac.as_ref().map(|s| s.url.as_str()) != Some(url)
        || bytes != original.bytes
        || sha256 != original.sha256
    {
        return Err("WorldClim original differs from the reviewed native source, complete byte length or SHA-256; no replacement release is accepted".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ResolveRequest {
        ResolveRequest {
            dataset_id: "worldclim".into(),
            bounds: [85., 27., 86., 28.],
            year: None,
            month: None,
            country: Some("NPL".into()),
            layer: Some("tavg".into()),
            region: None,
            season: None,
        }
    }
    #[test]
    fn normals_keep_units_and_complete_original_not_annual_dates() {
        let d = definitions().pop().unwrap();
        assert!(d.years.is_empty() && d.months.is_empty());
        assert!(d.license.contains("non-commercial") && !d.license.contains("CC BY"));
        assert_eq!(d.layers[0].scale, Some(1.));
        assert_eq!(d.layers[1].nodata, Some(-32768.));
        for original in &ORIGINALS {
            let mut r = request();
            r.layer = Some(original.layer.into());
            let s = resolve(&r).unwrap().pop().unwrap();
            assert_eq!(s.bounds, Some(BOUNDS));
            assert!(s.start_datetime.is_none() && s.end_datetime.is_none() && s.search.is_none());
            assert!(s.stac.unwrap().url.ends_with(original.name));
        }
    }
    #[test]
    fn unsupported_country_period_variable_or_footprint_never_falls_back() {
        for change in 0..8 {
            let mut r = request();
            match change {
                0 => r.country = Some("IND".into()),
                1 => r.country = None,
                2 => r.year = Some(2000),
                3 => r.month = Some(1),
                4 => r.layer = Some("bio".into()),
                5 => r.bounds = [116., 39., 117., 40.],
                6 => r.season = Some("summer".into()),
                _ => r.region = Some("NPL".into()),
            }
            assert!(resolve(&r).is_err());
        }
    }

    #[test]
    #[ignore = "requires GEOD_CLIMATE_ORIGINAL_DIR containing whole publisher originals"]
    fn whole_originals_decode_all_twelve_months_with_native_tiff_decoder() {
        use sha2::{Digest, Sha256};
        use std::{io::Cursor, path::PathBuf};
        use tiff::decoder::{Decoder, DecodingResult};
        let dir = PathBuf::from(std::env::var("GEOD_CLIMATE_ORIGINAL_DIR").unwrap());
        for original in &ORIGINALS {
            let bytes = std::fs::read(dir.join(original.name)).unwrap();
            assert_eq!(bytes.len() as u64, original.bytes);
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), original.sha256);
            let mut decoder = Decoder::new(Cursor::new(bytes)).unwrap();
            assert_eq!(decoder.dimensions().unwrap(), (1020, 540));
            let decoded = decoder.read_image().unwrap();
            let centre = (270 * 1020 + 510) * 12;
            match decoded {
                DecodingResult::F32(v) if original.layer == "tavg" => {
                    assert_eq!(v.len(), 1020 * 540 * 12);
                    assert_eq!(v[centre], 10.9_f32);
                    assert_eq!(v[centre + 6], 22.9_f32);
                    assert_eq!(v[centre + 11], 11.7_f32);
                    assert!(v.iter().all(|v| v.is_finite()));
                    assert!(v.iter().any(|v| *v < 0.) && v.contains(&0.));
                }
                DecodingResult::I16(v) if original.layer == "prec" => {
                    assert_eq!(v.len(), 1020 * 540 * 12);
                    assert_eq!([v[centre], v[centre + 6], v[centre + 11]], [19, 469, 11]);
                    assert!(!v.contains(&-32768));
                }
                _ => panic!("Original WorldClim scientific sample type changed"),
            }
        }
    }
}
