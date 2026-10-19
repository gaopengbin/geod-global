//! Eight whole three-period members of the exact author GLC_FCS30D archive.
//! The RGB TIFF tag is storage metadata: these are classification years.
use super::*;
use std::sync::LazyLock;
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Index {
    archive_url: String,
    archive_bytes: u64,
    archive_sha256: String,
    publisher_md5: String,
    sample_years: Vec<u16>,
    tiles: Vec<Tile>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Tile {
    name: String,
    bytes: u64,
    sha256: String,
    bounds: [f64; 4],
    width: u32,
    height: u32,
    pixels: Vec<Pixel>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Pixel {
    column: u32,
    row: u32,
    values: Vec<u8>,
    longitude: Option<f64>,
    latitude: Option<f64>,
}
static INDEX: LazyLock<Index> = LazyLock::new(|| {
    serde_json::from_str(include_str!("glc-fcs30d-reviewed-index.json"))
        .expect("Reviewed complete GLC_FCS30D author archive")
});

pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "glc-fcs30d",
        "GLC_FCS30D · 1985 land-cover classes",
        "land-cover",
        "https://zenodo.org/records/8239305",
    );
    d.description="The author's original GLC_FCS30D historical classification product. Reviewed scope is eight whole three-band TIFF members in one complete W175-W180 ZIP. Native first-band display represents 1985; the unchanged file and raw pixel arrays also retain the 1990 and 1995 classifications. The separate 23-band annual files are preserved in the complete archive but remain unsupported for inspection; no band is dropped or rewritten.".into();
    d.version = "GLC_FCS30D author archive · three historical bands 1985/1990/1995".into();
    d.license = "CC BY 4.0".into();
    d.license_url = CC_BY.into();
    d.attribution="Liu, Liangyun and Zhang, Xiao, GLC_FCS30D, doi:10.5281/zenodo.8239305. Zhang et al., Earth System Science Data 16, 1353–1381, doi:10.5194/essd-16-1353-2024. Preserve author licence, original file naming, record version and the published user guide/35-class legend. The guide recommends contacting the authors about scientific use and acknowledging their contribution; no contact or co-authorship is fabricated.".into();
    d.nominal_resolution =
        "30 m nominal; original EPSG:4326 spacing 0.00026949458523585647°".into();
    d.unit = "class code".into();
    d.value_interpretation="Original three-band UInt8 categorical arrays, scale 1 and offset 0. Publisher guide defines band order as 1985, 1990 and 1995, despite the old filename containing 19852000 and TIFF photometric tag RGB. These channels are classification years, not red/green/blue sensor bands. The guide identifies 0 and 250 as filled codes, while the actual TIFFs declare no scalar NoData; raw values and null embedded NoData are retained separately, without a synthetic mask. The first-band grayscale preview is 1985 only; the other years are not silently selected for display.".into();
    d.years = vec![1985];
    d.default_year = Some(1985);
    let mut l = layer(
        "classes-1985",
        "1985 class display · whole three-period original",
        "class code",
        &d.value_interpretation,
    );
    l.scale = Some(1.);
    l.offset = Some(0.);
    d.layers = vec![l];
    d.default_layer = Some("classes-1985".into());
    d.warnings=vec![
        "The complete author ZIP is 220,928,112 bytes with 16 TIFFs. Eight accepted 1985/1990/1995 three-band members are selected by their exact embedded footprints; the whole archive and the selected whole TIFF have independent SHA-256 and byte-length receipts. No AOI clipping, band extraction, generated STAC or TIFF MIME for the ZIP is fabricated.".into(),
        "The publisher guide establishes three pre-2000 bands: 1985, 1990 and 1995. The separate annual originals have 23 bands covering 2000–2022 and exceed the existing 1–16 samples limit. They remain unregistered; there is no 2000 band in the old 19852000-named three-band member.".into(),
        "The original TIFF RGB tag does not change the historical classification semantics. Display uses only the 1985 class-code band as grayscale; raw inspection returns all three original year values. No RGB photograph, sensor calibration, temporal average, new classification or change estimate is inferred.".into(),
        "The actual TIFF has no declared scalar NoData; publisher filled class codes 0 and 250 should be handled in scientific analysis according to the guide. A native noData=false reflects the embedded scalar declaration and does not prove a scientifically usable observation.".into(),
        "These originals have no internal overviews. Only exact accepted SHA-256 and unchanged profiles may use a lower-detail browsing preview selected within the existing cumulative decoding budget. Original files and full-resolution pixel values remain unchanged.".into(),
    ];
    d.limits=vec!["One exact author ZIP and eight accepted 18554 × 18554 × 3 UInt8 tiled LZW members only. Original small grid overlaps and all three class-year channels are preserved. Fixed first-band display is 1985. Other periods, longitude bundles and 23-band annual originals remain unsupported; existing 512 MiB transfer/preview, chunk, memory and timeout limits are unchanged.".into()];
    vec![d]
}
pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != "glc-fcs30d"
        || request.layer.as_deref() != Some("classes-1985")
        || request.year.unwrap_or(1985) != 1985
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the GLC_FCS30D 1985 first-band display of the complete three-period originals; no annual file, display year, sensor band, month or region is substituted".into());
    }
    if INDEX.archive_bytes != 220_928_112
        || INDEX.publisher_md5 != "md5:c08709165b8f07a19d861cc5a5afe876"
        || INDEX.sample_years != [1985, 1990, 1995]
    {
        return Err("Reviewed GLC_FCS30D archive or original year order changed".into());
    }
    let mut result = Vec::new();
    for tile in &INDEX.tiles {
        features::bounds(tile.bounds)?;
        if tile.width != 18554
            || tile.height != 18554
            || tile.bytes == 0
            || tile.bytes > 512 * 1024 * 1024
            || tile.sha256.len() != 64
            || tile.pixels.iter().any(|p| {
                p.column >= tile.width
                    || p.row >= tile.height
                    || p.values.len() != 3
                    || p.longitude.is_some_and(|v| !v.is_finite())
                    || p.latitude.is_some_and(|v| !v.is_finite())
            })
        {
            return Err(
                "Reviewed GLC_FCS30D original grid, bytes or raw class array changed".into(),
            );
        }
        if request.bounds[0] < tile.bounds[2]
            && request.bounds[2] > tile.bounds[0]
            && request.bounds[1] < tile.bounds[3]
            && request.bounds[3] > tile.bounds[1]
        {
            let id = tile.name.trim_end_matches(".tif");
            let label = id.trim_start_matches("GLC_FCS30_19852000_5yearsMap_V1_");
            let mut s = source(
                format!("GLC_FCS30D 1985 · {label}"),
                INDEX.archive_url.clone(),
                Some(tile.bounds),
            );
            s.id = id.into();
            result.push(s);
        }
    }
    if result.is_empty() {
        return Err("The selected area intersects no exact reviewed GLC_FCS30D historical original; no demonstration tile, other product or generated subset is selected".into());
    }
    Ok(result)
}
pub(crate) fn archive_selection(
    proof: &SourceProvenance,
    url: &str,
) -> Result<crate::stac::archive_original::ArchiveSelection> {
    let sources = resolve(&proof.request)?;
    let t = INDEX
        .tiles
        .iter()
        .find(|t| t.name.trim_end_matches(".tif") == proof.source_id)
        .ok_or("Unreviewed GLC_FCS30D ZIP member")?;
    let d = definitions().remove(0);
    if proof.dataset != d
        || proof.layer != d.layers[0]
        || proof.source_bounds != Some(t.bounds)
        || url != INDEX.archive_url
        || !sources
            .iter()
            .any(|s| s.id == proof.source_id && s.title == proof.source_title)
    {
        return Err(
            "GLC_FCS30D archive recipe differs from its exact native source provenance".into(),
        );
    }
    Ok(crate::stac::archive_original::ArchiveSelection {
        member: t.name.clone(),
        archive_bytes: INDEX.archive_bytes,
        archive_sha256: INDEX.archive_sha256.clone(),
        member_bytes: t.bytes,
        member_sha256: t.sha256.clone(),
    })
}
pub(crate) fn budget_preview_profile(m: &crate::stac::raster::GenericRasterInspection) -> bool {
    let Some(t) = INDEX.tiles.iter().find(|t| t.sha256 == m.sha256) else {
        return false;
    };
    m.width == t.width
        && m.height == t.height
        && m.storage_layout == "tile"
        && m.crs.as_deref() == Some("EPSG:4326")
        && m.pixel_interpretation.as_deref() == Some("area")
        && m.bands.len() == 3
        && m.bands
            .iter()
            .all(|b| b.data_type == "UInt8" && b.nodata.is_null())
        && m.bounds
            .is_some_and(|b| b.iter().zip(t.bounds).all(|(a, b)| (*a - b).abs() < 1e-10))
        && m.source_model_pixel_scale.is_some_and(|v| {
            (v[0] - 0.00026949458523585647).abs() < 1e-17 && v[1] == v[0] && v[2] == 0.
        })
        && m.source_model_tiepoint.is_some_and(|v| {
            v[0] == 0.
                && v[1] == 0.
                && v[2] == 0.
                && (v[3] - t.bounds[0]).abs() < 1e-10
                && (v[4] - t.bounds[3]).abs() < 1e-10
                && v[5] == 0.
        })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request() -> ResolveRequest {
        ResolveRequest {
            dataset_id: "glc-fcs30d".into(),
            bounds: [-174., 66., -173.9, 66.1],
            year: Some(1985),
            month: None,
            country: None,
            layer: Some("classes-1985".into()),
            region: None,
            season: None,
        }
    }
    #[test]
    fn three_original_years_are_retained_without_fake_rgb_or_2000_display() {
        let d = definitions().remove(0);
        assert_eq!(d.id, "glc-fcs30d");
        assert_eq!(d.years, vec![1985]);
        assert_eq!(d.layers[0].nodata, None);
        assert_eq!(INDEX.sample_years, vec![1985, 1990, 1995]);
        assert_eq!(INDEX.tiles.len(), 8);
        let s = resolve(&request()).unwrap();
        assert_eq!(s.len(), 1);
        assert!(s[0].id.ends_with("W175N70"));
    }
    #[test]
    fn unsupported_annual_year_layer_and_region_fail_without_replacement() {
        for change in 0..5 {
            let mut r = request();
            match change {
                0 => r.year = Some(1990),
                1 => r.year = Some(2000),
                2 => r.layer = Some("annual".into()),
                3 => r.month = Some(1),
                _ => r.bounds = [116., 39., 117., 40.],
            };
            assert!(resolve(&r).is_err());
        }
    }
    #[test]
    fn archive_selection_binds_original_member_footprint_and_year_semantics() {
        let r = request();
        let s = resolve(&r).unwrap().remove(0);
        let d = definitions().remove(0);
        let p = SourceProvenance {
            schema_version: 1,
            dataset: d.clone(),
            request: r,
            layer: d.layers[0].clone(),
            source_id: s.id,
            source_title: s.title,
            source_bounds: s.bounds,
            declaration_kind: "reviewed-client-preset".into(),
            catalog_raster: None,
        };
        let a = archive_selection(&p, &INDEX.archive_url).unwrap();
        assert_eq!(a.archive_bytes, 220928112);
        assert!(a.member.ends_with("W175N70.tif"));
        let mut changed = p.clone();
        changed.layer.nodata = Some(0.);
        assert!(archive_selection(&changed, &INDEX.archive_url).is_err());
        changed = p.clone();
        changed.source_id = "GLC_FCS30D_20002022_W175N70_Annual".into();
        assert!(archive_selection(&changed, &INDEX.archive_url).is_err());
    }
}
