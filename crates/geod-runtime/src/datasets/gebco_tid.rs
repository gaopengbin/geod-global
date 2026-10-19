//! The complete eight-tile GEBCO 2026 TID release. Codes describe source
//! methods, not water depths. Only the independently verified whole ZIP can
//! activate the finite expanded-directory allowance.
use super::*;
use crate::stac::{archive_original::ArchiveSelection, raster::GenericRasterInspection};
use std::sync::LazyLock;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Index {
    pub archive_url: String,
    pub archive_bytes: u64,
    pub archive_sha256: String,
    pub expanded_bytes: u64,
    pub whole_archive_verified: bool,
    pub whole_all_members_crc_verified: bool,
    pub all_raster_base_pixels_decoded: bool,
    pub tiles: Vec<Tile>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct Tile {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
    pub bounds: [f64; 4],
    pub transform: [f64; 6],
    pub width: u32,
    pub height: u32,
    pub nodata: Option<f64>,
}
pub(crate) static INDEX: LazyLock<Index> = LazyLock::new(|| {
    serde_json::from_str(include_str!("gebco-tid-reviewed-index.json"))
        .expect("Whole GEBCO TID archive and all eight raw grids verified")
});
fn selection(t: &Tile) -> ArchiveSelection {
    ArchiveSelection {
        member: t.name.clone(),
        archive_bytes: INDEX.archive_bytes,
        archive_sha256: INDEX.archive_sha256.clone(),
        member_bytes: t.bytes,
        member_sha256: t.sha256.clone(),
    }
}
pub(crate) fn is_selection(s: &ArchiveSelection) -> bool {
    INDEX.whole_archive_verified
        && INDEX.whole_all_members_crc_verified
        && INDEX.all_raster_base_pixels_decoded
        && INDEX.tiles.len() == 8
        && INDEX.tiles.iter().any(|t| selection(t) == *s)
}
pub(crate) fn budget_preview_profile(m: &GenericRasterInspection) -> bool {
    INDEX.tiles.iter().any(|t| {
        m.sha256 == t.sha256
            && m.width == t.width
            && m.height == t.height
            && m.storage_layout == "strip"
            && m.crs.as_deref() == Some("EPSG:4326")
            && m.pixel_interpretation.as_deref() == Some("area")
            && m.bands.len() == 1
            && m.bands[0].data_type == "Int8"
            && m.bands[0].nodata == serde_json::json!(t.nodata)
            && m.bounds
                .is_some_and(|b| b.iter().zip(t.bounds).all(|(a, b)| (*a - b).abs() < 1e-10))
            && m.transform.is_some_and(|b| {
                b.iter()
                    .zip(t.transform)
                    .all(|(a, b)| (*a - b).abs() < 1e-10)
            })
    })
}
pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "gebco-tid",
        "GEBCO 2026 · source type identifiers",
        "elevation",
        "https://www.gebco.net/data-products-gridded-bathymetry-data/gebco2026-grid",
    );
    d.description="Complete official 2026 Type Identifier Grid, distributed as eight original 90° GeoTIFF tiles in one publisher ZIP. TID describes the source method behind each GEBCO grid cell; it is not elevation, measured depth, a quality rank or a new satellite image.".into();
    d.version = "GEBCO 2026 TID · eight global original tiles".into();
    d.license = "Public domain; GEBCO attribution and terms apply".into();
    d.license_url = d.documentation_url.clone();
    d.attribution="GEBCO Bathymetric Compilation Group 2026 (2026). The GEBCO_2026 Grid - a continuous terrain model for oceans and land at 15 arc-second intervals. NERC EDS British Oceanographic Data Centre NOC. doi:10.5285/4f68d5c7-45eb-f999-e063-7086abc036fa. Preserve the complete original documentation and GEBCO terms; no official endorsement is implied.".into();
    d.nominal_resolution =
        "15 arc-second geographic grid; original 21600 × 21600 Area tiles".into();
    d.unit = "source method code".into();
    d.value_interpretation="Int8 source-type identifiers, original TIFF NoData 127. 0 = land (valid), 10–17 = direct measurement methods, 40–48 = indirect methods, 70 = mixed pre-generated grid, 71 = unknown source, 72 = steering points. Numeric magnitude is not accuracy or depth. NoData follows only each original TIFF declaration; no code is silently masked.".into();
    let mut l = layer(
        "tid",
        "Source method · original code",
        "source method code",
        &d.value_interpretation,
    );
    l.scale = Some(1.);
    l.offset = Some(0.);
    l.nodata = INDEX.tiles.first().and_then(|t| t.nodata);
    d.layers = vec![l];
    d.default_layer = Some("tid".into());
    d.warnings=vec!["Every selected tile retains the whole official ZIP, accompanying documentation and selected unchanged TIFF with separate complete SHA-256 and CRC verification. The search area selects intersecting original tiles and does not clip, resample or reproject them.".into(),
        "TID 0 is valid land. A grayscale browse image is not a bathymetric map, confidence score or scientific quality mask. The 2026 release combines inputs of different dates and resolutions.".into(),
        "This static TID release complements the official regional elevation subset service. Full static elevation and sub-ice originals remain outside this action. GEBCO data must not be used for navigation or safety at sea.".into()];
    d.limits=vec!["Eight exact original TID members only. The whole ZIP and each original TIFF remain below the ordinary 512 MiB transfer/member limit. Only this exact archive identity admits its finite approximately 3.7 GB expanded directory, streamed one member at a time. Preview and raw-pixel reads keep existing decoding, memory and time limits.".into()];
    vec![d]
}
pub(super) fn resolve(r: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(r.bounds)?;
    if r.dataset_id != "gebco-tid"
        || r.layer.as_deref() != Some("tid")
        || r.year.is_some()
        || r.month.is_some()
        || r.country.is_some()
        || r.region.is_some()
        || r.season.is_some()
    {
        return Err(
            "Choose the fixed GEBCO 2026 source-type grid without dates or another product".into(),
        );
    }
    let sources = INDEX
        .tiles
        .iter()
        .filter(|t| {
            r.bounds[0] < t.bounds[2]
                && r.bounds[2] > t.bounds[0]
                && r.bounds[1] < t.bounds[3]
                && r.bounds[3] > t.bounds[1]
        })
        .map(|t| {
            let mut s = source(
                format!("GEBCO 2026 TID · {}", t.name.trim_end_matches(".tif")),
                INDEX.archive_url.clone(),
                Some(t.bounds),
            );
            s.id = t.name.trim_end_matches(".tif").into();
            s
        })
        .collect::<Vec<_>>();
    if sources.is_empty() {
        return Err("The selected region intersects no GEBCO TID original tile".into());
    }
    Ok(sources)
}
pub(crate) fn archive_selection(p: &SourceProvenance, url: &str) -> Result<ArchiveSelection> {
    let sources = resolve(&p.request)?;
    let d = definitions().remove(0);
    let t = INDEX
        .tiles
        .iter()
        .find(|t| t.name.trim_end_matches(".tif") == p.source_id)
        .ok_or("Unknown GEBCO TID original")?;
    if p.schema_version!=2 || p.declaration_kind!="reviewed-client-preset; publisher file bytes and embedded metadata verified separately"
        || p.dataset!=d || p.layer!=d.layers[0] || p.source_bounds!=Some(t.bounds)
        || p.catalog_raster.is_some() || url!=INDEX.archive_url
        || !sources.iter().any(|s|s.id==p.source_id && s.title==p.source_title) {
        return Err("GEBCO TID original differs from its native immutable provenance".into());
    }
    Ok(selection(t))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(bounds: [f64; 4]) -> ResolveRequest {
        ResolveRequest {
            dataset_id: "gebco-tid".into(),
            bounds,
            layer: Some("tid".into()),
            year: None,
            month: None,
            country: None,
            region: None,
            season: None,
        }
    }
    #[test]
    fn all_eight_global_originals_are_finite_and_zero_is_not_a_missing_code() {
        let d = definitions().remove(0);
        assert_ne!(d.layers[0].nodata, Some(0.));
        assert_eq!(
            resolve(&request([-180., -90., 180., 90.])).unwrap().len(),
            8
        );
        assert!(INDEX.tiles.iter().all(|t| t.bytes < crate::MAX_ASSET_BYTES));
        let s = selection(&INDEX.tiles[0]);
        assert!(is_selection(&s));
        let mut forged = s.clone();
        forged.archive_sha256 = "0".repeat(64);
        assert!(!is_selection(&forged));
        forged = s.clone();
        forged.member_bytes += 1;
        assert!(!is_selection(&forged));
        let mut r = request([116., 39., 117., 40.]);
        r.year = Some(2025);
        assert!(resolve(&r).is_err());
        r = request([116., 39., 117., 40.]);
        r.layer = Some("elevation".into());
        assert!(resolve(&r).is_err());
    }
}
