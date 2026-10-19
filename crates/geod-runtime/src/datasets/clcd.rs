//! One unchanged CLCD 2004 national original. Admission requires its complete
//! independently verified SHA-256 and exact physical profile. A ranged metadata
//! digest never activates transfer, large-grid, custom-CRS or palette support.
use super::*;
use crate::io_error;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    io::{Read, Seek, SeekFrom},
    sync::LazyLock,
};
use tiff::{
    decoder::{ChunkType, Decoder},
    tags::Tag,
};

pub(crate) const SOURCE_ID: &str = "CLCD_v01_2004_albert";
pub(crate) const URL: &str =
    "https://zenodo.org/api/records/18180184/files/CLCD_v01_2004_albert.tif/content";
pub(crate) const ORIGINAL_BYTES: u64 = 791_887_020;
pub(crate) const MAX_ORIGINAL_BYTES: u64 = 1024 * 1024 * 1024;
pub(crate) const CRS: &str = "CLCD:AlbersWGS84";
pub(crate) const PROJ4: &str = "+proj=aea +lat_1=25 +lat_2=47 +lat_0=0 +lon_0=105 +x_0=0 +y_0=0 +datum=WGS84 +units=m +no_defs";
pub(crate) const CHUNK_COUNT: u32 = 83_424;
pub(crate) const EXPECTED_TILE_DIMS: [u32; 2] = [512, 512];
const DECLARATION: &str =
    "reviewed-client-preset; publisher file bytes and embedded metadata verified separately";
const PENDING: &str = "CLCD original admission is pending independent complete-file MD5/SHA-256 and physical-profile verification";
const GEOKEYS: &[u16] = &[
    1, 1, 0, 18, 1024, 0, 1, 1, 1025, 0, 1, 1, 1026, 34737, 8, 0, 2048, 0, 1, 4326, 2049, 34737, 7,
    8, 2054, 0, 1, 9102, 2057, 34736, 1, 7, 2059, 34736, 1, 6, 3072, 0, 1, 32767, 3074, 0, 1,
    32767, 3075, 0, 1, 11, 3076, 0, 1, 9001, 3078, 34736, 1, 0, 3079, 34736, 1, 1, 3080, 34736, 1,
    3, 3081, 34736, 1, 2, 3082, 34736, 1, 4, 3083, 34736, 1, 5,
];
const GEODOUBLES: &[f64] = &[25., 47., 0., 105., 0., 0., 298.257223563, 6378137.];
const GEOASCII: &str = "unknown|WGS 84|\0";
const PIXEL_SCALE: [f64; 3] = [30., 30., 0.];
const TIEPOINT: [f64; 6] = [0., 0., 0., -2629624.7830456714, 5924251.5596752735, 0.];
const SOURCE_BOUNDS: [f64; 4] = [
    67.92919509428276,
    15.268316002676293,
    136.60167654287028,
    54.69630715597805,
];
const IMAGE_DIMS: [(u32, u32); 10] = [
    (161378, 135079),
    (80689, 67539),
    (40344, 33769),
    (20172, 16884),
    (10086, 8442),
    (5043, 4221),
    (2521, 2110),
    (1260, 1055),
    (630, 527),
    (315, 263),
];

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Index {
    schema_version: u32,
    dataset_id: String,
    release: String,
    documentation: String,
    source_id: String,
    url: String,
    original_bytes: u64,
    publisher_md5: String,
    original_sha256: Option<String>,
    whole_file_verified: bool,
    publisher_md5_verified: bool,
    metadata_range_bytes: u64,
    metadata_range_sha256: String,
    source_bounds: Option<[f64; 4]>,
    profile: Option<Profile>,
    legend: Option<Legend>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Profile {
    width: u32,
    height: u32,
    bands: u16,
    data_type: String,
    nodata: u8,
    crs: String,
    proj4: String,
    registration: String,
    pixel_scale: [f64; 3],
    tiepoint: [f64; 6],
    bounds: [f64; 4],
    geo_key_directory: Vec<u16>,
    geo_double_params: Vec<f64>,
    geo_ascii_params: String,
    color_map: Vec<u16>,
    image_levels: Vec<ImageLevel>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ImageLevel {
    width: u32,
    height: u32,
    new_subfile_type: u32,
    tile_width: u32,
    tile_height: u32,
    chunk_count: u32,
    compression: u16,
    photometric: u16,
    samples_per_pixel: u16,
    bits_per_sample: Vec<u16>,
    sample_format: Option<Vec<u16>>,
    planar_configuration: u16,
    predictor: u16,
    nodata: Option<String>,
    offsets_sha256: String,
    byte_counts_sha256: String,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Legend {
    url: String,
    sha256: String,
    bytes: u64,
    publisher_md5: String,
    classes: Vec<Class>,
}
#[derive(Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Class {
    code: u8,
    name: String,
    publisher_name: String,
    rgb: [u8; 3],
}
static INDEX: LazyLock<Index> = LazyLock::new(|| {
    serde_json::from_str(include_str!("clcd-reviewed-index.json"))
        .expect("Compiled bounded CLCD original review index")
});

// Only validation of the immutable compiled index is cached. Every actual
// source file still undergoes complete hashing, original metadata verification
// and all-image decoder-profile checks on every read.
static CHECKED_INDEX: LazyLock<Result<()>> = LazyLock::new(validate_index);

fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn projected_bounds() -> [f64; 4] {
    [
        TIEPOINT[3],
        TIEPOINT[4] - 135_079. * 30.,
        TIEPOINT[3] + 161_378. * 30.,
        TIEPOINT[4],
    ]
}
fn validate_crs_values(keys: &[u16], doubles: &[f64], ascii: &str) -> Result<()> {
    // Exact original GeoTIFF values, including user-defined PCS 32767 and
    // Albers transform 11. The ASCII citation is preserved, not used as an EPSG.
    // tiff::Decoder trims the terminating NUL from ASCII values. The complete
    // original byte check below separately preserves and verifies that byte.
    if keys != GEOKEYS || doubles != GEODOUBLES || ascii != GEOASCII.trim_end_matches('\0') {
        return Err(
            "CLCD custom Albers GeoKey, parameter, datum, unit or original citation changed".into(),
        );
    }
    Ok(())
}
fn validate_index() -> Result<()> {
    if !INDEX.whole_file_verified
        || !INDEX.publisher_md5_verified
        || INDEX.original_sha256.as_deref().is_none_or(|s| !digest(s))
        || INDEX.profile.is_none()
        || INDEX.source_bounds.is_none()
    {
        return Err(PENDING.into());
    }
    let p = INDEX.profile.as_ref().unwrap();
    let bounds = INDEX.source_bounds.unwrap();
    features::bounds(bounds)?;
    if INDEX.schema_version != 1
        || INDEX.dataset_id != "clcd"
        || INDEX.release != "CLCD v1.0.5 · unchanged national 2004 original"
        || INDEX.documentation != "https://zenodo.org/records/18180184"
        || INDEX.source_id != SOURCE_ID
        || INDEX.url != URL
        || INDEX.original_bytes != ORIGINAL_BYTES
        || ORIGINAL_BYTES > MAX_ORIGINAL_BYTES
        || INDEX.publisher_md5 != "842776ec3cbda29344344980c867cc72"
        || INDEX.metadata_range_bytes != 908_144
        || INDEX.metadata_range_sha256
            != "ab369c0520cead2c4542b93fdeb8d2d8b828898a51114b863a192fffa85712dc"
        || (p.width, p.height, p.bands) != (161_378, 135_079, 1)
        || p.data_type != "UInt8"
        || p.nodata != 0
        || p.crs != CRS
        || p.proj4 != PROJ4
        || p.registration != "area"
        || p.pixel_scale != PIXEL_SCALE
        || p.tiepoint != TIEPOINT
        || bounds != SOURCE_BOUNDS
        || p.bounds != projected_bounds()
        || p.color_map.len() != 768
        || p.image_levels.len() != 10
        || p.geo_ascii_params != GEOASCII
        || format!(
            "{:x}",
            Sha256::digest(
                p.color_map
                    .iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<_>>()
            )
        ) != "ae22425391068c11c1d9bf336558c4c1655a64b7526ac5167576e83c6eaf07f1"
        || p.image_levels[0].chunk_count != CHUNK_COUNT
        || p.image_levels.iter().enumerate().any(|(i, l)| {
            (l.width, l.height) != IMAGE_DIMS[i]
                || [l.tile_width, l.tile_height] != EXPECTED_TILE_DIMS
                || l.chunk_count != l.width.div_ceil(512) * l.height.div_ceil(512)
                || l.chunk_count > 131_072
                || l.compression != 5
                || l.photometric != 3
                || l.samples_per_pixel != 1
                || l.bits_per_sample != [8]
                || l.sample_format.as_ref().is_some_and(|v| v != &[1])
                || l.planar_configuration != 1
                || l.predictor != 1
                || l.new_subfile_type != u32::from(i != 0)
                || !digest(&l.offsets_sha256)
                || !digest(&l.byte_counts_sha256)
        })
    {
        return Err(
            "Compiled CLCD original identity or finite physical review profile changed".into(),
        );
    }
    validate_crs_values(
        &p.geo_key_directory,
        &p.geo_double_params,
        p.geo_ascii_params.trim_end_matches('\0'),
    )?;
    if let Some(legend) = &INDEX.legend {
        if legend.sha256 != "77072c4f35a0b43f6aeed119ad1c04956eca7d0982b61241910563f720b63e56"
            || legend.url != "https://zenodo.org/api/records/18180184/files/CLCD_classificationsystem.xlsx/content"
            || legend.bytes != 9440 || legend.publisher_md5 != "928388273874350c8d06649761e6c87b"
            || legend.classes.iter().map(|c| c.code).collect::<Vec<_>>() != [1,2,3,4,5,6,7,8,9]
            || legend.classes.iter().any(|c| c.name.is_empty() || c.publisher_name.is_empty()
                || [p.color_map[c.code as usize]/257,p.color_map[256+c.code as usize]/257,p.color_map[512+c.code as usize]/257] != c.rgb.map(u16::from))
            || legend.classes.iter().map(|c| c.code).collect::<std::collections::BTreeSet<_>>().len() != legend.classes.len()
        { return Err("CLCD separately reviewed class guide is invalid".into()); }
    }
    Ok(())
}

fn checked_index() -> Result<&'static Profile> {
    match &*CHECKED_INDEX {
        Ok(()) => Ok(INDEX
            .profile
            .as_ref()
            .expect("Validated compiled CLCD profile")),
        Err(error) => Err(error.clone()),
    }
}

pub(crate) fn original_sha256() -> Result<&'static str> {
    checked_index()?;
    Ok(INDEX.original_sha256.as_deref().unwrap())
}

pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "clcd",
        "CLCD · national 2004 land-cover original",
        "land-cover",
        &INDEX.documentation,
    );
    d.description = "One complete author-published national 2004 classification GeoTIFF from the fixed CLCD v1.0.5 record. The selected area organizes this unchanged original; it is not cropped, substituted by a province archive or converted to an RGB source raster.".into();
    d.version = INDEX.release.clone();
    d.attribution = "Jie Yang and Xin Huang, China Land Cover Dataset (CLCD), doi:10.5281/zenodo.18180184; Yang and Huang (2021), Earth System Science Data 13, 3907–3925, doi:10.5194/essd-13-3907-2021. Retain the selected 2004 source, v1.0.5 record, original checksum and CC BY 4.0 attribution.".into();
    d.nominal_resolution =
        "30 m nominal classification; original 30 × 30 metre WGS84 Albers Area grid".into();
    d.unit = "class code".into();
    d.value_interpretation = "Original single-band UInt8 categorical codes, scale 1, offset 0, embedded NoData 0. Display colors follow the exact original ColorMap; class meanings require the separately reviewed author guide. A predicted annual classification is not RGB sensor imagery, reflectance, or a common scene acquisition date.".into();
    d.years = vec![2004];
    d.default_year = Some(2004);
    d.layers = vec![scaled(
        layer(
            "classes",
            "National land-cover classes · 2004 whole original",
            "class code",
            &d.value_interpretation,
        ),
        1.,
        0.,
        0.,
    )];
    d.default_layer = Some("classes".into());
    d.warnings = vec![
        "Only the complete CLCD_v01_2004_albert.tif from the fixed author v1.0.5 record is selected. Other years, releases and province ZIPs are not substituted. The reference year is 2004; no acquisition datetime is fabricated.".into(),
        "The 161378 × 135079 tiled original contains 21798778862 grid pixels. Only its complete SHA-256 and exact native profile may use the reviewed CLCD axis/grid exception; the 83424 base tiles already fit the ordinary chunk ceiling. No whole-grid array is allocated.".into(),
        "Original PCS 32767 is a user-defined WGS84 Albers projection, with standard parallels 25°/47°, central meridian 105°, origin 0° and metre units. CLCD:AlbersWGS84 is a local reader identity, not an EPSG code. Original GeoTIFF keys and parameters are preserved.".into(),
        "Source bytes, full-resolution codes and georeference remain unchanged. Browsing may sample an embedded overview; preview PNG colors are categorical display only. Source footprint intersection does not establish valid-data coverage or accuracy throughout an area.".into(),
    ];
    d.limits = vec!["One independently verified unchanged 791887020-byte national original, under an exact source/proof/SHA-gated 1 GiB file allowance. Ordinary transfer, axis/grid/chunk ceilings, 128 MiB per-chunk decoding, 32 MiB intermediate buffer, 1 MiB IFD values, 512 MiB cumulative preview and existing deadlines are unchanged. Complete original and UI/export acceptance remain separate checks.".into()];
    vec![d]
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    checked_index()?;
    features::bounds(request.bounds)?;
    if request.dataset_id != "clcd"
        || request.year != Some(2004)
        || request.layer.as_deref() != Some("classes")
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the exact CLCD national 2004 classes original; other years, months, provinces and seasons are not substituted".into());
    }
    let b = INDEX.source_bounds.unwrap();
    if request.bounds[0] >= b[2]
        || request.bounds[2] <= b[0]
        || request.bounds[1] >= b[3]
        || request.bounds[3] <= b[1]
    {
        return Err(
            "The selected area does not intersect the reviewed complete CLCD source footprint"
                .into(),
        );
    }
    let mut s = source(
        "CLCD v1.0.5 national · 2004 whole classification original".into(),
        URL.into(),
        Some(b),
    );
    s.id = SOURCE_ID.into();
    Ok(vec![s])
}

pub(crate) struct OriginalPolicy {
    pub bytes: u64,
    pub sha256: String,
}
pub(crate) fn original_policy(proof: &SourceProvenance, url: &str) -> Result<OriginalPolicy> {
    let d = definitions().remove(0);
    let sources = resolve(&proof.request)?;
    if proof.schema_version != 2
        || proof.declaration_kind != DECLARATION
        || proof.dataset != d
        || proof.layer != d.layers[0]
        || proof.source_bounds != INDEX.source_bounds
        || proof.source_id != SOURCE_ID
        || proof.source_title != sources[0].title
        || proof.catalog_raster.is_some()
        || url != URL
    {
        return Err("CLCD source differs from its exact compiled native dataset provenance".into());
    }
    Ok(OriginalPolicy {
        bytes: ORIGINAL_BYTES,
        sha256: original_sha256()?.into(),
    })
}
pub(crate) fn validate_original(
    proof: &SourceProvenance,
    url: &str,
    bytes: u64,
    sha256: &str,
) -> Result<()> {
    let p = original_policy(proof, url)?;
    if bytes != p.bytes || sha256 != p.sha256 {
        return Err("CLCD complete original byte length or SHA-256 differs from its independently reviewed publisher file".into());
    }
    Ok(())
}
pub(crate) fn large_grid_identity(sha: &str, width: u32, height: u32, tiled: bool) -> bool {
    original_sha256().is_ok_and(|expected| sha == expected)
        && width == 161_378
        && height == 135_079
        && tiled
}
pub(crate) fn validate_geotiff_crs(
    sha: &str,
    keys: &[u16],
    doubles: &[f64],
    ascii: &str,
) -> Result<()> {
    if sha != original_sha256()? {
        return Err("Custom CLCD Albers support requires the complete independently reviewed original SHA-256".into());
    }
    validate_crs_values(keys, doubles, ascii)
}

/// Validate the original unmodified bytes before RawPaletteReader changes the
/// photometric view. This finite streamed prefix covers every original IFD,
/// tag type/value, structural declaration, palette and tile index. Its digest
/// cannot authorize a partial file: complete original SHA readiness is required.
pub(crate) fn validate_original_metadata<R: Read + Seek>(reader: &mut R, sha: &str) -> Result<()> {
    if sha != original_sha256()? {
        return Err(
            "CLCD metadata profile requires its complete independently reviewed original SHA-256"
                .into(),
        );
    }
    let position = reader.stream_position().map_err(io_error)?;
    let result = (|| -> Result<()> {
        reader.seek(SeekFrom::Start(0)).map_err(io_error)?;
        let mut remaining = INDEX.metadata_range_bytes;
        let mut hash = Sha256::new();
        let mut buffer = [0u8; 65536];
        while remaining > 0 {
            let size = buffer.len().min(remaining as usize);
            reader.read_exact(&mut buffer[..size]).map_err(io_error)?;
            hash.update(&buffer[..size]);
            remaining -= size as u64;
        }
        if format!("{:x}", hash.finalize()) != INDEX.metadata_range_sha256 {
            return Err(
                "CLCD original metadata bytes, tag types, image directory or tile indexes changed"
                    .into(),
            );
        }
        Ok(())
    })();
    reader.seek(SeekFrom::Start(position)).map_err(io_error)?;
    result
}

fn integer_array_hash(values: &[u64]) -> String {
    let mut hash = Sha256::new();
    for value in values {
        hash.update(value.to_le_bytes());
    }
    format!("{:x}", hash.finalize())
}
fn optional_unsigned<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    tag: Tag,
    default: u16,
) -> Result<u16> {
    Ok(decoder
        .find_tag_unsigned::<u16>(tag)
        .map_err(io_error)?
        .unwrap_or(default))
}
fn optional_u16_array<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    tag: Tag,
) -> Result<Option<Vec<u16>>> {
    decoder
        .find_tag(tag)
        .map_err(io_error)?
        .map(|v| v.into_u16_vec().map_err(io_error))
        .transpose()
}
fn optional_ascii<R: Read + Seek>(decoder: &mut Decoder<R>, tag: Tag) -> Result<Option<String>> {
    decoder
        .find_tag(tag)
        .map_err(io_error)?
        .map(|v| v.into_string().map_err(io_error))
        .transpose()
}

/// Call only after a complete file hash and native source validation, using the
/// read-only RawPaletteReader view. Checks every image, then restores image 0.
pub(crate) fn validate_decoder_profile<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    sha: &str,
    raw_palette: bool,
) -> Result<()> {
    let p = checked_index()?;
    if sha != original_sha256()? || !raw_palette {
        return Err("CLCD raw palette profile requires its exact complete original".into());
    }
    decoder.seek_to_image(0).map_err(io_error)?;
    validate_geotiff_crs(
        sha,
        &decoder
            .get_tag_u16_vec(Tag::GeoKeyDirectoryTag)
            .map_err(io_error)?,
        &decoder
            .get_tag_f64_vec(Tag::GeoDoubleParamsTag)
            .map_err(io_error)?,
        &decoder
            .get_tag_ascii_string(Tag::GeoAsciiParamsTag)
            .map_err(io_error)?,
    )?;
    if decoder
        .get_tag_f64_vec(Tag::ModelPixelScaleTag)
        .map_err(io_error)?
        != p.pixel_scale
        || decoder
            .get_tag_f64_vec(Tag::ModelTiepointTag)
            .map_err(io_error)?
            != p.tiepoint
        || decoder
            .find_tag(Tag::ModelTransformationTag)
            .map_err(io_error)?
            .is_some()
    {
        return Err("CLCD original scale/tiepoint or north-up registration changed".into());
    }
    for (index, level) in p.image_levels.iter().enumerate() {
        if index > 0 {
            decoder.next_image().map_err(io_error)?;
        }
        let offsets = decoder
            .get_tag_u64_vec(Tag::TileOffsets)
            .map_err(io_error)?;
        let counts = decoder
            .get_tag_u64_vec(Tag::TileByteCounts)
            .map_err(io_error)?;
        if decoder.dimensions().map_err(io_error)? != (level.width, level.height)
            || decoder.get_chunk_type() != ChunkType::Tile
            || decoder.chunk_dimensions() != (level.tile_width, level.tile_height)
            || decoder.tile_count().map_err(io_error)? != level.chunk_count
            || decoder
                .find_tag_unsigned::<u32>(Tag::NewSubfileType)
                .map_err(io_error)?
                .unwrap_or(0)
                != level.new_subfile_type
            || optional_unsigned(decoder, Tag::Compression, 1)? != level.compression
            || optional_unsigned(decoder, Tag::PhotometricInterpretation, 1)? != 1
            || optional_unsigned(decoder, Tag::SamplesPerPixel, 1)? != level.samples_per_pixel
            || decoder
                .get_tag_u16_vec(Tag::BitsPerSample)
                .map_err(io_error)?
                != level.bits_per_sample
            || optional_u16_array(decoder, Tag::SampleFormat)? != level.sample_format
            || optional_unsigned(decoder, Tag::PlanarConfiguration, 1)?
                != level.planar_configuration
            || optional_unsigned(decoder, Tag::Predictor, 1)? != level.predictor
            || optional_unsigned(decoder, Tag::Orientation, 1)? != 1
            || optional_ascii(decoder, Tag::GdalNodata)? != level.nodata
            || decoder.get_tag_u16_vec(Tag::ColorMap).map_err(io_error)? != p.color_map
            || offsets.len() != level.chunk_count as usize
            || counts.len() != offsets.len()
            || integer_array_hash(&offsets) != level.offsets_sha256
            || integer_array_hash(&counts) != level.byte_counts_sha256
            || offsets.iter().zip(&counts).any(|(offset, count)| {
                *count == 0
                    || offset
                        .checked_add(*count)
                        .is_none_or(|end| end > ORIGINAL_BYTES)
            })
        {
            return Err(format!(
                "CLCD original image level {index} differs from its exact bounded physical profile"
            ));
        }
    }
    if decoder.more_images() {
        return Err("CLCD original contains unreviewed additional image or mask levels".into());
    }
    decoder.seek_to_image(0).map_err(io_error)?;
    Ok(())
}

pub(crate) fn budget_preview_profile(m: &crate::stac::raster::GenericRasterInspection) -> bool {
    let Ok(p) = checked_index() else {
        return false;
    };
    large_grid_identity(&m.sha256, m.width, m.height, m.storage_layout == "tile")
        && m.crs.as_deref() == Some(CRS)
        && m.horizontal_crs.as_deref() == Some(CRS)
        && m.vertical_crs.is_none()
        && m.pixel_interpretation.as_deref() == Some("area")
        && m.bands.len() == 1
        && m.bands[0].index == 1
        && m.bands[0].data_type == "UInt8"
        && m.bands[0].nodata == json!(0)
        && m.bounds == Some(p.bounds)
        && m.source_model_pixel_scale == Some(p.pixel_scale)
        && m.source_model_tiepoint == Some(p.tiepoint)
        && m.transform == Some([30., 0., p.bounds[0], 0., -30., p.bounds[3]])
}
fn palette(p: &Profile) -> Vec<[u8; 4]> {
    (0..256)
        .map(|code| {
            [
                (p.color_map[code] / 257) as u8,
                (p.color_map[256 + code] / 257) as u8,
                (p.color_map[512 + code] / 257) as u8,
                if code == 0 { 0 } else { 255 },
            ]
        })
        .collect()
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
    let p = checked_index().ok()?;
    let code = value as usize;
    Some([
        (p.color_map[code] / 257) as u8,
        (p.color_map[256 + code] / 257) as u8,
        (p.color_map[512 + code] / 257) as u8,
        if code == 0 { 0 } else { 255 },
    ])
}
pub(crate) fn categorical_encoding(
    m: &crate::stac::raster::GenericRasterInspection,
) -> Option<Value> {
    if !budget_preview_profile(m) {
        return None;
    }
    let p = checked_index().ok()?;
    Some(
        json!({"schema":"geod-clcd-class-palette/v1","datasetId":"clcd","originalSha256":original_sha256().ok()?,
        "referenceYear":2004,"unit":"class code","nodata":0,"palette":palette(p),
        "classes":INDEX.legend.as_ref().map(|l| &l.classes),
        "legendUrl":INDEX.legend.as_ref().map(|l| &l.url),"legendSha256":INDEX.legend.as_ref().map(|l| &l.sha256),
        "rendering":"exact original ColorMap; NoData 0 transparent; guide meanings are separate from embedded colors"}),
    )
}

#[cfg(test)]
mod tests;
