//! Whole author-published GLC_FCS10 ZIP and unchanged 2023 categorical TIFFs.
//! The ZIP URL never becomes a fictitious direct TIFF or publisher STAC item.
use super::*;
use std::sync::LazyLock;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Index {
    archive_url: String,
    archive_bytes: u64,
    archive_sha256: String,
    publisher_md5: String,
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
}
static INDEX: LazyLock<Index> = LazyLock::new(|| {
    serde_json::from_str(include_str!("glc-fcs10-reviewed-index.json"))
        .expect("Reviewed complete GLC_FCS10 archive index")
});

/// The display exception is bound to accepted whole-member bytes and their
/// unchanged numerical/grid profile, never a product name or file extension.
pub(crate) fn budget_preview_profile(
    metadata: &crate::stac::raster::GenericRasterInspection,
) -> bool {
    let Some(tile) = INDEX.tiles.iter().find(|t| t.sha256 == metadata.sha256) else {
        return false;
    };
    metadata.width == tile.width
        && metadata.height == tile.height
        && metadata.storage_layout == "tile"
        && metadata.crs.as_deref() == Some("EPSG:4326")
        && metadata.pixel_interpretation.as_deref() == Some("area")
        && metadata.bands.len() == 1
        && metadata.bands[0].data_type == "UInt8"
        && metadata.bands[0].nodata.is_null()
        && metadata.bounds.is_some_and(|bounds| {
            bounds
                .iter()
                .zip(tile.bounds)
                .all(|(a, b)| (*a - b).abs() < 1e-10)
        })
        && metadata.source_model_pixel_scale.is_some_and(|v| {
            (v[0] - 0.00008983152841195215).abs() < 1e-17
                && (v[1] - v[0]).abs() < 1e-17
                && v[2] == 0.
        })
        && metadata.source_model_tiepoint.is_some_and(|v| {
            v[0] == 0.
                && v[1] == 0.
                && v[2] == 0.
                && (v[3] - tile.bounds[0]).abs() < 1e-10
                && (v[4] - tile.bounds[3]).abs() < 1e-10
                && v[5] == 0.
        })
}

pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "glc-fcs10",
        "GLC_FCS10 · 2023 land-cover classes",
        "land-cover",
        "https://zenodo.org/records/14729665",
    );
    d.description = "Original 2023 categorical land-cover maps generated from Sentinel-1 and Sentinel-2 time-series observations. The reviewed scope is six whole W175 TIFF members in the author's W175-W180 ZIP; the four W180 members retain 180–185° coordinates and are not silently shifted or registered. This is GLC_FCS10 itself, not a GEE export, another land-cover product or original sensor bands.".into();
    d.version = "GLC_FCS10 v1 · 2023 reference map · published 2025-01-24".into();
    d.license = "CC BY 4.0".into();
    d.license_url = CC_BY.into();
    d.attribution = "Liu, Liangyun and Zhang, Xiao (2025), GLC_FCS10, doi:10.5281/zenodo.14729665. Zhang et al. (2025), Earth System Science Data 17, 4039–4062, doi:10.5194/essd-17-4039-2025. Retain the author licence, record version and publisher legend for class interpretation.".into();
    d.nominal_resolution =
        "10 m nominal; original EPSG:4326 spacing 0.00008983152841195215°".into();
    d.unit = "class code".into();
    d.value_interpretation = "Original single-band UInt8 categorical codes, scale 1 and offset 0. The actual TIFFs declare no NoData; zero and every stored code remain raw values and are not converted into a missing-data mask. Consult the publisher classification legend for class meanings. The 2023 reference year and 2025 publication are not individual image acquisition dates; no physical reflectance or spectral bands are inferred.".into();
    d.years = vec![2023];
    d.default_year = Some(2023);
    let mut classes = layer(
        "classes",
        "Land-cover class · original code",
        "class code",
        &d.value_interpretation,
    );
    classes.scale = Some(1.);
    classes.offset = Some(0.);
    d.layers = vec![classes];
    d.default_layer = Some("classes".into());
    d.warnings = vec![
        "Only six completely checked W175N55/N60/N65/N70/S10/S15 original TIFFs are eligible. Every selection downloads and preserves the whole 117,500,101-byte author ZIP and extracts the selected complete TIFF with separate byte lengths, SHA-256 and ZIP CRC checks; it does not clip or fabricate an AOI file.".into(),
        "Four W180 members have original longitudes near 180–185°. They remain preserved in the original archive but are excluded from this recipe because the existing geographic footprint contract cannot represent them. No datum/longitude rewrite, wrap, reprojection or alternate tile is substituted.".into(),
        "The source TIFFs contain tiled LZW data without internal overviews. Native previews must stay within their existing chunk, cumulative decoding, grid, axis, memory and time budgets; a valid archive or resolver result alone does not prove a successful preview.".into(),
        "A grayscale preview displays original class indices, not continuous quantities. The actual TIFFs declare no NoData, and no class-code-to-quality mask or regional accuracy is invented.".into(),
    ];
    d.limits = vec!["Fixed author v1 ZIP, 2023 map and six exact original members only. The 512 MiB archive/member ceilings and existing raster budgets remain unchanged. Bounds come from original embedded transforms, including actual small overlaps; the filename is not an exact footprint. Other longitude bundles, dates, classes and global coverage remain unverified.".into()];
    vec![d]
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != "glc-fcs10"
        || request.layer.as_deref() != Some("classes")
        || request.year.unwrap_or(2023) != 2023
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the reviewed GLC_FCS10 2023 class-code originals; no other map year, layer, month, country, region or season is substituted".into());
    }
    if INDEX.archive_bytes != 117_500_101
        || INDEX.publisher_md5 != "md5:b940ca1c7269fe024827248cf1eb25e5"
        || INDEX.archive_sha256.len() != 64
    {
        return Err("Reviewed GLC_FCS10 whole publisher ZIP receipt changed".into());
    }
    let mut sources = Vec::new();
    for tile in &INDEX.tiles {
        features::bounds(tile.bounds)?;
        if tile.bytes == 0
            || tile.bytes > 512 * 1024 * 1024
            || tile.sha256.len() != 64
            || tile.width > 131_072
            || tile.height > 131_072
            || u64::from(tile.width) * u64::from(tile.height) > 8_000_000_000
            || tile
                .pixels
                .iter()
                .any(|p| p.column >= tile.width || p.row >= tile.height || p.values.len() != 1)
        {
            return Err("Reviewed GLC_FCS10 original member receipt or grid changed".into());
        }
        if request.bounds[0] < tile.bounds[2]
            && request.bounds[2] > tile.bounds[0]
            && request.bounds[1] < tile.bounds[3]
            && request.bounds[3] > tile.bounds[1]
        {
            let id = tile.name.trim_end_matches(".tif");
            let mut s = source(
                format!(
                    "GLC_FCS10 2023 {}",
                    id.trim_start_matches("GLC_FCS10_2023_")
                ),
                INDEX.archive_url.clone(),
                Some(tile.bounds),
            );
            s.id = id.into();
            sources.push(s);
        }
    }
    if sources.is_empty() {
        return Err("The selected region intersects no exact reviewed GLC_FCS10 original; no demonstration region, alternative product or generated subset is selected".into());
    }
    Ok(sources)
}

pub(crate) fn archive_selection(
    proof: &SourceProvenance,
    url: &str,
) -> Result<crate::stac::archive_original::ArchiveSelection> {
    let sources = resolve(&proof.request)?;
    let tile = INDEX
        .tiles
        .iter()
        .find(|tile| tile.name.trim_end_matches(".tif") == proof.source_id)
        .ok_or("Unreviewed GLC_FCS10 original ZIP member")?;
    let definition = definitions().remove(0);
    if proof.dataset != definition
        || proof.layer != definition.layers[0]
        || proof.source_bounds != Some(tile.bounds)
        || url != INDEX.archive_url
        || !sources
            .iter()
            .any(|s| s.id == proof.source_id && s.title == proof.source_title)
    {
        return Err(
            "GLC_FCS10 archive/member recipe differs from its reviewed native provenance".into(),
        );
    }
    Ok(crate::stac::archive_original::ArchiveSelection {
        member: tile.name.clone(),
        archive_bytes: INDEX.archive_bytes,
        archive_sha256: INDEX.archive_sha256.clone(),
        member_bytes: tile.bytes,
        member_sha256: tile.sha256.clone(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(bounds: [f64; 4]) -> ResolveRequest {
        ResolveRequest {
            dataset_id: "glc-fcs10".into(),
            bounds,
            year: Some(2023),
            month: None,
            country: None,
            layer: Some("classes".into()),
            region: None,
            season: None,
        }
    }
    #[test]
    fn fixed_product_keeps_original_categorical_calibration_and_absent_nodata() {
        let d = definitions().remove(0);
        assert_eq!(d.id, "glc-fcs10");
        assert_eq!(d.years, vec![2023]);
        assert_eq!(d.layers[0].scale, Some(1.));
        assert_eq!(d.layers[0].offset, Some(0.));
        assert_eq!(d.layers[0].nodata, None);
        assert_eq!(d.license, "CC BY 4.0");
        assert_eq!(INDEX.tiles.len(), 6);
        assert!(INDEX
            .tiles
            .iter()
            .all(|t| t.bounds[0] >= -180. && t.bounds[2] <= 180.));
    }
    #[test]
    fn selection_uses_real_footprints_and_keeps_both_hemispheres() {
        let north = resolve(&request([-174., 62., -173.9, 62.1])).unwrap();
        assert_eq!(north.len(), 1);
        assert_eq!(north[0].id, "GLC_FCS10_2023_W175N65");
        let south = resolve(&request([-171.8, -13.9, -171.7, -13.8])).unwrap();
        assert_eq!(south.len(), 1);
        assert_eq!(south[0].id, "GLC_FCS10_2023_W175S10");
        let overlap = resolve(&request([-174., 54.99999, -173.9, 55.00001])).unwrap();
        assert_eq!(overlap.len(), 2);
        assert!(south[0].stac.as_ref().unwrap().url.ends_with(".zip"));
    }
    #[test]
    fn other_years_layers_wrapped_members_and_remote_regions_fail_without_replacement() {
        for change in 0..8 {
            let mut r = request([-174., 62., -173.9, 62.1]);
            match change {
                0 => r.year = Some(2024),
                1 => r.month = Some(1),
                2 => r.layer = Some("reflectance".into()),
                3 => r.country = Some("USA".into()),
                4 => r.region = Some("W180N65".into()),
                5 => r.season = Some("annual".into()),
                6 => r.bounds = [179., 62., 180., 62.1],
                _ => r.bounds = [116., 39., 117., 40.],
            };
            assert!(resolve(&r).is_err());
        }
    }
    #[test]
    fn only_exact_original_recipe_can_select_a_zip_member() {
        let r = request([-174., 62., -173.9, 62.1]);
        let s = resolve(&r).unwrap().remove(0);
        let d = definitions().remove(0);
        let proof = SourceProvenance {
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
        let selected = archive_selection(&proof, &INDEX.archive_url).unwrap();
        assert_eq!(selected.member, "GLC_FCS10_2023_W175N65.tif");
        assert_eq!(selected.archive_bytes, 117_500_101);
        assert_eq!(selected.member_bytes, 32_067_963);
        for change in 0..4 {
            let mut p = proof.clone();
            match change {
                0 => p.source_id = "GLC_FCS10_2023_W180N65".into(),
                1 => p.source_bounds = Some([-175., 60., -170., 65.]),
                2 => p.layer.nodata = Some(0.),
                _ => p.dataset.license = "Public domain".into(),
            };
            assert!(archive_selection(&p, &INDEX.archive_url).is_err());
        }
        assert!(archive_selection(&proof, "https://example.com/fake.zip").is_err());
    }
    #[test]
    #[ignore = "requires GEOD_GLC_FCS10_ARCHIVE_DIR containing the whole unchanged author ZIP"]
    fn whole_original_archive_and_raw_class_pixels_match_independent_review() {
        use sha2::{Digest, Sha256};
        use std::io::{Cursor, Read};
        use tiff::decoder::{Decoder, DecodingResult};
        use tiff::tags::Tag;
        let root = std::path::PathBuf::from(std::env::var("GEOD_GLC_FCS10_ARCHIVE_DIR").unwrap());
        let bytes = std::fs::read(root.join("GLC_FCS10maps_2023_W175-W180.zip")).unwrap();
        assert_eq!(bytes.len() as u64, INDEX.archive_bytes);
        assert_eq!(
            format!("{:x}", Sha256::digest(&bytes)),
            INDEX.archive_sha256
        );
        let mut archive = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(archive.len(), 10);
        for tile in &INDEX.tiles {
            let mut member = archive.by_name(&tile.name).unwrap();
            let mut raw = Vec::new();
            member.read_to_end(&mut raw).unwrap();
            assert_eq!(raw.len() as u64, tile.bytes);
            assert_eq!(format!("{:x}", Sha256::digest(&raw)), tile.sha256);
            let mut decoder = Decoder::new(Cursor::new(raw)).unwrap();
            assert_eq!(decoder.dimensions().unwrap(), (tile.width, tile.height));
            assert_eq!(decoder.find_tag(Tag::GdalNodata).unwrap(), None);
            let across = tile.width.div_ceil(256);
            for pixel in &tile.pixels {
                let index = (pixel.row / 256) * across + pixel.column / 256;
                let width = decoder.chunk_data_dimensions(index).0;
                let DecodingResult::U8(values) = decoder.read_chunk(index).unwrap() else {
                    panic!("Original GLC_FCS10 is no longer UInt8")
                };
                let offset = ((pixel.row % 256) * width + pixel.column % 256) as usize;
                assert_eq!(
                    values[offset], pixel.values[0],
                    "{} {},{}",
                    tile.name, pixel.column, pixel.row
                );
            }
        }
    }
}
