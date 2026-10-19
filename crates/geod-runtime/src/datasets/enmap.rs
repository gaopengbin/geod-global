//! One complete official land-mode EnMAP L2A example, not an archive search.
//! The ENVI exception is bound to the complete ZIP, BSQ and HDR identities.
use super::*;
use crate::stac::archive_original::ArchiveSelection;
use std::sync::LazyLock;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Member {
    pub name: String,
    pub bytes: u64,
    pub sha256: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Profile {
    pub width: u32,
    pub height: u32,
    pub bands: u16,
    pub transform: [f64; 6],
    pub bounds: [f64; 4],
    pub source_bounds: [f64; 4],
    pub wavelengths: Vec<f64>,
    pub fwhm: Vec<f64>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct Index {
    pub url: String,
    pub archive_bytes: u64,
    pub archive_sha256: String,
    pub expanded_bytes: u64,
    pub main: Member,
    pub header: Member,
    pub profile: Profile,
    pub whole_archive_verified: bool,
    pub whole_all_members_crc_verified: bool,
    pub whole_cube_decoded_band_by_band: bool,
}
pub(crate) static INDEX: LazyLock<Index> = LazyLock::new(|| {
    serde_json::from_str(include_str!("enmap-reviewed-index.json"))
        .expect("Complete reviewed EnMAP original")
});
pub(crate) const SOURCE_ID: &str = "enmap-nevada-l2a-20230707";
pub(crate) fn selection() -> ArchiveSelection {
    ArchiveSelection {
        member: INDEX.main.name.clone(),
        archive_bytes: INDEX.archive_bytes,
        archive_sha256: INDEX.archive_sha256.clone(),
        member_bytes: INDEX.main.bytes,
        member_sha256: INDEX.main.sha256.clone(),
    }
}
pub(crate) fn is_selection(value: &ArchiveSelection) -> bool {
    INDEX.whole_archive_verified
        && INDEX.whole_all_members_crc_verified
        && INDEX.whole_cube_decoded_band_by_band
        && value == &selection()
}
pub(crate) fn is_member(bytes: u64, sha: &str) -> bool {
    bytes == INDEX.main.bytes && sha == INDEX.main.sha256 && is_selection(&selection())
}
pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "enmap",
        "EnMAP · Nevada L2A hyperspectral original",
        "hyperspectral",
        "https://www.enmap.org/data_tools/exampledata/",
    );
    d.description="One fixed official Nevada, USA L2A land-mode example acquired 7 July 2023. Downloads the whole publisher ZIP with the unchanged 224-band Int16 ENVI BSQ cube, original HDR, metadata and quality files. Not a global scene catalog or a quicklook substitute.".into();
    d.version = "Nevada L2A V010506; processed 2026-05-28; acquired 2023-07-07".into();
    d.license = "EnMAP Data License v1.1; restricted original redistribution".into();
    d.license_url = "https://www.enmap.org/data/resources/EnMAP_Data_License.pdf".into();
    d.attribution="EnMAP data ©DLR 2023 All rights reserved. Preserve original notices. Local commercial/noncommercial use is permitted by EnMAP terms; do not publicly redistribute downloadable original pixels or resell/sublicense them. This connector downloads from the official publisher to the user's local workspace.".into();
    d.nominal_resolution = "30 m original UTM 11N Area grid; 1286 × 1197 × 224".into();
    d.unit = "Int16 stored sample; surface reflectance = sample × 0.0001".into();
    d.value_interpretation="Original land-mode surface reflectance. All 224 stored Int16 bands, wavelength centers and FWHM in nm are retained. Header gain is 0.0001 and offset 0 for every band; NoData -32768 is excluded before scaling. Negative and above-one valid values remain unchanged. A stretched single-band grayscale preview is not an RGB product or a quality mask.".into();
    d.layers = vec![scaled(
        layer(
            "spectral-cube",
            "Nevada · complete 224-band spectral cube",
            "surface reflectance",
            &d.value_interpretation,
        ),
        0.0001,
        0.,
        -32768.,
    )];
    d.default_layer = Some("spectral-cube".into());
    d.warnings=vec!["Only the fixed Nevada example is enabled. Tyrol, Groningen, other dates, L1B/L1C and arbitrary ENVI files are not substituted. The selected area must intersect the full original footprint; the download is not clipped.".into(),
        "Official processing uses UTM projection and bilinear interpolation in land mode, with no cirrus/haze removal or terrain correction. Original quality masks remain in the ZIP; raw-pixel validity here uses only declared NoData and does not establish cloud-free or scientifically useful coverage.".into(),
        "The May 2026 processing date is not acquisition time. Original data licence is not CC BY; this is local direct publisher access, not a hosted original-data mirror.".into()];
    d.limits=vec!["The whole ZIP remains below the ordinary 512 MiB transfer limit. Only this exact whole ZIP/BSQ/HDR identity permits its reviewed 689625216-byte expanded BSQ. Other archives retain ordinary expansion limits. Band browsing holds at most one approximately 3 MiB source band; no complete cube is loaded into memory.".into()];
    vec![d]
}
pub(super) fn resolve(r: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(r.bounds)?;
    if r.dataset_id != "enmap"
        || r.layer.as_deref() != Some("spectral-cube")
        || r.year.is_some()
        || r.month.is_some()
        || r.country.is_some()
        || r.region.is_some()
        || r.season.is_some()
    {
        return Err(
            "Choose the fixed Nevada L2A 224-band original without other time or region parameters"
                .into(),
        );
    }
    let b = INDEX.profile.source_bounds;
    if r.bounds[0] >= b[2] || r.bounds[2] <= b[0] || r.bounds[1] >= b[3] || r.bounds[3] <= b[1] {
        return Err(
            "The selected area does not intersect the reviewed Nevada EnMAP original".into(),
        );
    }
    let mut s = source(
        "EnMAP Nevada · 2023-07-07 · complete L2A spectral cube".into(),
        INDEX.url.clone(),
        Some(b),
    );
    s.id = SOURCE_ID.into();
    Ok(vec![s])
}
pub(crate) fn archive_selection(p: &SourceProvenance, url: &str) -> Result<ArchiveSelection> {
    let sources = resolve(&p.request)?;
    let d = definitions().remove(0);
    if p.schema_version!=2 || p.declaration_kind!="reviewed-client-preset; publisher file bytes and embedded metadata verified separately"
        || p.dataset!=d || p.layer!=d.layers[0] || p.source_bounds!=Some(INDEX.profile.source_bounds)
        || url!=INDEX.url || p.catalog_raster.is_some()
        || !sources.iter().any(|s|s.id==p.source_id && s.title==p.source_title) {
        return Err("EnMAP original differs from its exact native source provenance".into());
    }
    Ok(selection())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn complete_cube_exception_is_finite_and_not_an_arbitrary_large_archive_allowance() {
        let s = selection();
        assert!(is_selection(&s));
        assert_eq!(s.archive_bytes, 367338178);
        assert_eq!(s.member_bytes, 689625216);
        assert_eq!(INDEX.profile.width, 1286);
        assert_eq!(INDEX.profile.height, 1197);
        assert_eq!(INDEX.profile.bands, 224);
        assert_eq!(INDEX.profile.wavelengths.len(), 224);
        assert_eq!(INDEX.profile.fwhm.len(), 224);
        let mut changed = s.clone();
        changed.member_sha256 = "0".repeat(64);
        assert!(!is_selection(&changed));
        changed = s.clone();
        changed.archive_bytes += 1;
        assert!(!is_selection(&changed));
        assert!(!is_member(s.member_bytes, "0"));
        let r = ResolveRequest {
            dataset_id: "enmap".into(),
            bounds: [116., 39., 117., 40.],
            year: None,
            month: None,
            layer: Some("spectral-cube".into()),
            country: None,
            region: None,
            season: None,
        };
        assert!(resolve(&r).is_err());
    }
}
