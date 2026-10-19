//! One unchanged Shanghai original, with an explicit SHA/profile-bound large
//! tiled-grid policy. Ordinary raster ceilings and all byte/decode budgets stay
//! unchanged. This module does not declare a completed UI/download acceptance.
use super::*;
use serde_json::{json, Value};
use std::sync::LazyLock;

pub(crate) const CHUNK_COUNT: u32 = 230_136;
pub(crate) const EXPECTED_TILE_DIMS: [u32; 2] = [256, 256];
pub(crate) const ORIGINAL_MEMBER_SHA256: &str =
    "5cd5c24c414fdb3cadb89c53429d5987de70ea605e0100a9b4a83a2257b48830";
const SOURCE_ID: &str = "East_Shanghai";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Index {
    schema_version: u32,
    dataset_id: String,
    release: String,
    documentation: String,
    guide_url: String,
    legend_url: String,
    legend_sha256: String,
    archive_url: String,
    archive_bytes: u64,
    archive_sha256: String,
    publisher_md5: String,
    member: Original,
    classes: Vec<Class>,
    palette: Vec<[u8; 4]>,
    original_color_map: Vec<Vec<u16>>,
    capture_time: CaptureTime,
    tiles: Vec<Original>,
    profile: Value,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Original {
    name: String,
    bytes: u64,
    sha256: String,
    crc32: String,
    bounds: [f64; 4],
    width: u32,
    height: u32,
    bands: u16,
    data_type: String,
    nodata: u8,
    crs: String,
    registration: String,
    pixel_scale: [f64; 3],
    tiepoint: [f64; 6],
    tile_width: u32,
    tile_height: u32,
    chunk_count: u32,
    compression: u16,
    photometric: u16,
    maximum_compressed_chunk_bytes: u64,
    maximum_decoded_chunk_bytes: u64,
    overviews: Vec<u32>,
    pixels: Vec<Pixel>,
    #[serde(default)]
    edge_pixels: Vec<Pixel>,
}
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Class {
    code: u8,
    name: String,
    rgb: [u8; 3],
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Pixel {
    column: u32,
    row: u32,
    longitude: f64,
    latitude: f64,
    values: Vec<u8>,
    no_data: Vec<bool>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct CaptureTime {
    url: String,
    bytes: u64,
    sha256: String,
    grid_degrees: f64,
    observed_shanghai_years: Vec<u16>,
    per_pixel_landcover_time_verified: bool,
}
static INDEX: LazyLock<Index> = LazyLock::new(|| {
    serde_json::from_str(include_str!("sinolc-reviewed-index.json"))
        .expect("Complete publisher-MD5/CRC/SHA verified SinoLC Shanghai original")
});

fn checked_index() -> Result<()> {
    let t = &INDEX.member;
    features::bounds(t.bounds)?;
    if INDEX.schema_version != 1
        || INDEX.dataset_id != "sinolc-1"
        || INDEX.archive_url != "https://zenodo.org/records/7709370/files/East_Shanghai.zip"
        || INDEX.archive_bytes != 365_818_022
        || INDEX.archive_sha256
            != "f128cae66f38c546a850af767b42b85d29fd0c4d71ee3bb325e30b962d58d268"
        || INDEX.publisher_md5 != "md5:c898994fa496d915f73b2ffd102d93ca"
        || t.name != "East_Shanghai.tif"
        || t.bytes != 427_679_242
        || t.sha256 != ORIGINAL_MEMBER_SHA256
        || t.crc32 != "cabc1e88"
        || (t.width, t.height, t.chunk_count) != (132_000, 114_000, CHUNK_COUNT)
        || [t.tile_width, t.tile_height] != EXPECTED_TILE_DIMS
        || t.bands != 1
        || t.data_type != "UInt8"
        || t.nodata != 0
        || t.crs != "EPSG:4326"
        || t.registration != "area"
        || t.compression != 5
        || t.photometric != 1
        || t.maximum_compressed_chunk_bytes != 13_552
        || t.maximum_decoded_chunk_bytes != 65_536
        || !t.overviews.is_empty()
        || INDEX.palette.len() != 256
        || INDEX.original_color_map.len() != 3
        || INDEX.original_color_map.iter().any(|v| v.len() != 256)
        || INDEX.classes.iter().map(|v| v.code).collect::<Vec<_>>()
            != [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 12]
        || INDEX
            .classes
            .iter()
            .any(|v| INDEX.palette[v.code as usize][..3] != v.rgb)
        || t.pixels.len() != 144
        || t.edge_pixels.len() != 8
        || t.pixels.iter().any(|p| {
            p.column >= t.width
                || p.row >= t.height
                || p.values.len() != 1
                || p.no_data != [p.values[0] == 0]
                || !p.longitude.is_finite()
                || !p.latitude.is_finite()
        })
        || INDEX.capture_time.per_pixel_landcover_time_verified
        || INDEX.capture_time.observed_shanghai_years != [2019, 2020, 2021]
        || INDEX.tiles.len() != 1
        || INDEX.tiles[0].sha256 != t.sha256
        || INDEX.tiles[0].name != t.name
        || INDEX.tiles[0].bytes != t.bytes
        || INDEX.tiles[0].bounds != t.bounds
        || INDEX.tiles[0].width != t.width
        || INDEX.tiles[0].height != t.height
        || INDEX.profile["crs"] != "EPSG:4326"
        || INDEX.profile["nodata"] != 0
        || INDEX.profile["storageLayout"] != "tile"
        || INDEX.profile["chunkCount"] != CHUNK_COUNT
        || INDEX.profile["colorMap"] != json!(INDEX.original_color_map)
    {
        return Err(
            "Reviewed whole SinoLC Shanghai identity, class palette or physical profile changed"
                .into(),
        );
    }
    Ok(())
}

pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "sinolc-1",
        "SinoLC-1 · Shanghai original classification",
        "land-cover",
        &INDEX.documentation,
    );
    d.description = "The complete publisher's historical Shanghai land-cover original in the fixed March 2023 East-of-China release. One full ZIP and its unchanged full single-band GeoTIFF are stored with independent byte/SHA receipts. This is an explicitly reviewed large tiled grid, not nationwide discovery, an RGB image, a clipped AOI export or an annual observation map.".into();
    d.version = INDEX.release.clone();
    d.license = "CC BY 4.0".into();
    d.license_url = CC_BY.into();
    d.attribution = "Zhuohong Li et al., SinoLC-1, doi:10.5281/zenodo.7709370; Earth System Science Data 15, 4749–4780 (2023), doi:10.5194/essd-15-4749-2023. Preserve the fixed historical Shanghai original, author class legend, source version, attribution and full archive/member checksums. The classification data licence does not license redistribution of its input Google Earth imagery.".into();
    d.nominal_resolution = "1 m nominal classification; unchanged EPSG:4326 Area spacing 0.000010728836059570312 degrees, not a metre-valued TIFF unit".into();
    d.unit = "class code".into();
    let legend = INDEX
        .classes
        .iter()
        .map(|v| format!("{} {}", v.code, v.name))
        .collect::<Vec<_>>()
        .join(", ");
    d.value_interpretation = format!("Original UInt8 categorical codes, scale 1 and offset 0. Embedded scalar NoData is 0. Publisher legend: {legend}. Class 11 is not listed in the guide despite having an embedded green palette entry; 250 and 255 are not declared NoData or guide-defined fill codes. Their raw values and original palette are retained without inventing class meaning. This is a predicted classification, not quantitative reflectance or a calibrated land measurement.");
    d.layers = vec![scaled(
        layer(
            "classes",
            "Shanghai land-cover classes · unchanged whole original",
            "class code",
            &d.value_interpretation,
        ),
        1.,
        0.,
        0.,
    )];
    d.default_layer = Some("classes".into());
    d.warnings = vec![
        "This exact original is 132000 × 114000 (15048000000 grid pixels) in 230136 256 × 256 LZW tiles and has no overview. It exceeds ordinary axis/grid/chunk ceilings. Only the independently verified whole-member SHA and full physical profile may use the explicit reviewed large-grid policy; ordinary source limits remain unchanged.".into(),
        "Each original tile is at most 65536 decoded UInt8 bytes. Browsing chooses a lower-detail display within the unchanged 128 MiB per-chunk, 512 MiB cumulative preview and per-stage cooperative 30-second deadlines; no hard 30-second promise is made for a complete file hash/inspection. Full source bytes, original grid and full-resolution raw samples remain unchanged. A browsing PNG is not the full-resolution original.".into(),
        "The source mosaic was selected from the December 2021 Google Earth version, with actual source imagery captured across years. The publisher's separate 0.3-degree capture-time map gives Shanghai coarse-cell values including 2019, 2020 and 2021; it does not establish a common or per-pixel 1-m observation year. Publication in 2023 is not an observation date.".into(),
        "This fixed historical original retains its original edges and any overlap artefacts. The later author 2026 edge-fix script is not silently applied, and no new corrected original is fabricated.".into(),
        format!("The exact original ColorMap matches all eleven publisher class colors. Legend source: {} (SHA {}). The original TIFF photometric tag is BlackIsZero (1), separately from its ColorMap; map colors express categorical classes, not RGB sensor bands.", INDEX.legend_url, INDEX.legend_sha256),
        format!("Author guide: {}. Timing evidence only: {} ({} bytes, SHA {}, {}° grid). It is not selected as the land-cover product.", INDEX.guide_url, INDEX.capture_time.url, INDEX.capture_time.bytes, INDEX.capture_time.sha256, INDEX.capture_time.grid_degrees),
    ];
    d.limits = vec!["One fixed Shanghai full publisher ZIP/member only, each within the existing 512 MiB source/expanded-file budget. No city substitution, clipping, reprojection, source downsampling, other years, national batch or arbitrary large raster is authorized. Complete original export and lower-detail display retain separate identities; software/UI acceptance is a separate gate.".into()];
    vec![d]
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    checked_index()?;
    features::bounds(request.bounds)?;
    if request.dataset_id != "sinolc-1"
        || request.layer.as_deref() != Some("classes")
        || request.year.is_some()
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the fixed SinoLC Shanghai classes original; no common observation year, other city, RGB image or replacement layer is inferred".into());
    }
    let b = INDEX.member.bounds;
    if request.bounds[0] >= b[2]
        || request.bounds[2] <= b[0]
        || request.bounds[1] >= b[3]
        || request.bounds[3] <= b[1]
    {
        return Err(
            "The selected area does not intersect the complete reviewed Shanghai original".into(),
        );
    }
    let mut s = source(
        "SinoLC-1 Shanghai · March 2023 whole classification original".into(),
        INDEX.archive_url.clone(),
        Some(b),
    );
    s.id = SOURCE_ID.into();
    Ok(vec![s])
}

pub(crate) fn archive_selection(
    proof: &SourceProvenance,
    url: &str,
) -> Result<crate::stac::archive_original::ArchiveSelection> {
    let sources = resolve(&proof.request)?;
    let d = definitions().remove(0);
    if proof.schema_version != 2 || proof.declaration_kind != "reviewed-client-preset; publisher file bytes and embedded metadata verified separately"
        || proof.dataset != d || proof.layer != d.layers[0]
        || proof.source_bounds != Some(INDEX.member.bounds) || url != INDEX.archive_url
        || !sources.iter().any(|s| s.id == proof.source_id && s.title == proof.source_title)
        || proof.catalog_raster.is_some()
    {
        return Err("SinoLC archive selection differs from its exact native original provenance".into());
    }
    Ok(crate::stac::archive_original::ArchiveSelection {
        member: INDEX.member.name.clone(),
        archive_bytes: INDEX.archive_bytes,
        archive_sha256: INDEX.archive_sha256.clone(),
        member_bytes: INDEX.member.bytes,
        member_sha256: INDEX.member.sha256.clone(),
    })
}

/// Preliminary identity only; the caller must verify bytes before decoding and
/// then require the complete profile and original TIFF tags before any read.
pub(crate) fn large_grid_identity(sha: &str, width: u32, height: u32, tiled: bool) -> bool {
    sha == ORIGINAL_MEMBER_SHA256 && width == 132_000 && height == 114_000 && tiled
}

pub(crate) fn budget_preview_profile(m: &crate::stac::raster::GenericRasterInspection) -> bool {
    let t = &INDEX.member;
    large_grid_identity(&m.sha256, m.width, m.height, m.storage_layout == "tile")
        && m.crs.as_deref() == Some("EPSG:4326")
        && m.horizontal_crs.as_deref() == Some("EPSG:4326")
        && m.vertical_crs.is_none()
        && m.pixel_interpretation.as_deref() == Some("area")
        && m.bands.len() == 1
        && m.bands[0].index == 1
        && m.bands[0].data_type == "UInt8"
        && m.bands[0].nodata.as_f64() == Some(0.)
        && m.bounds == Some(t.bounds)
        && m.source_model_pixel_scale == Some(t.pixel_scale)
        && m.source_model_tiepoint == Some(t.tiepoint)
        && m.transform
            == Some([
                t.pixel_scale[0],
                0.,
                t.bounds[0],
                0.,
                -t.pixel_scale[1],
                t.bounds[3],
            ])
}

/// Original TIFF tag 320, all 3 × 256 UInt16 entries. No normalized palette
/// replaces this strict low-level decoder comparison.
pub(crate) fn original_color_map() -> Vec<u16> {
    INDEX.original_color_map.iter().flatten().copied().collect()
}

pub(crate) fn categorical_color(
    m: &crate::stac::raster::GenericRasterInspection,
    value: f64,
) -> Option<[u8; 4]> {
    if !budget_preview_profile(m)
        || !value.is_finite()
        || !(0. ..=255.).contains(&value)
        || value.fract() != 0.
    {
        return None;
    }
    INDEX.palette.get(value as usize).copied()
}

pub(crate) fn categorical_encoding(
    m: &crate::stac::raster::GenericRasterInspection,
) -> Option<Value> {
    budget_preview_profile(m).then(|| json!({
        "schema":"geod-sinolc-class-palette/v1", "datasetId":"sinolc-1",
        "originalMemberSha256":ORIGINAL_MEMBER_SHA256, "unit":"class code", "nodata":0,
        "classes":INDEX.classes, "palette":INDEX.palette, "unlistedCodes":[11,250,255],
        "legendUrl":INDEX.legend_url, "legendSha256":INDEX.legend_sha256,
        "rendering":"exact original class ColorMap; unlisted codes retain their palette but no class meaning is invented"
    }))
}

#[cfg(test)]
mod tests;
