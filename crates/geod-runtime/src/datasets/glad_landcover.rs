//! UMD GLAD GLCLUC v2 / 2020 author-published direct GeoTIFF tiles.
//! The complete URL index is an author listing, not publisher STAC. Its
//! filename-derived declared tile extents do not prove useful pixel coverage.
use super::*;
use std::sync::OnceLock;

pub(crate) const VERIFIED_TILE_URL: &str =
    "https://storage.googleapis.com/earthenginepartners-hansen/GLCLU2000-2020/v2/2020/30N_030E.tif";
pub(crate) const VERIFIED_TILE_SHA256: &str =
    "87406f90ee15916222ddceef02f8b7088e32e9b528b808f5a35aed40de68fcd7";
pub(crate) const VERIFIED_TILE_BYTES: u64 = 44_495_039;
const INDEX_URL: &str =
    "https://storage.googleapis.com/earthenginepartners-hansen/GLCLU2000-2020/v2/2020.txt";
const INDEX_SHA256: &str = "f68eaa40a51e144ee9aa0b4426ca9f668e54e6e96144894093573a47fbf9d6aa";

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PublishedIndex {
    schema_version: u32,
    publisher_index_url: String,
    publisher_index_sha256: String,
    grid_interpretation: String,
    year: u16,
    records: Vec<PublishedTile>,
}

#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishedTile {
    tile: String,
    url: String,
    bbox: [f64; 4],
}

fn index() -> &'static PublishedIndex {
    static INDEX: OnceLock<PublishedIndex> = OnceLock::new();
    INDEX.get_or_init(|| {
        let index: PublishedIndex =
            serde_json::from_str(include_str!("glad-landcover-reviewed-index.json"))
                .expect("checked-in GLAD v2 / 2020 publisher URL index");
        assert!(
            index.schema_version == 1
                && index.publisher_index_url == INDEX_URL
                && index.publisher_index_sha256 == INDEX_SHA256
                && index.year == 2020
                && index.records.len() == 280
                && index.grid_interpretation.contains("not fabricated STAC")
                && index
                    .records
                    .iter()
                    .any(|tile| tile.url == VERIFIED_TILE_URL),
            "reviewed GLAD v2 / 2020 index contract changed"
        );
        index
    })
}

pub(super) fn definition() -> Dataset {
    let mut dataset = base(
        "glad-landcover-2020",
        "GLAD land cover and use · 2020",
        "land-cover",
        "https://glad.umd.edu/dataset/GLCLUC2020",
    );
    dataset.description = "UMD GLAD's combined global land-cover and land-use v2 map for reference year 2020, published as original 10-degree GeoTIFF tiles. This reviewed connector uses the author's complete 280-URL annual-map listing. The combined thematic indices are distinct from Hansen forest-change layers, raw Landsat reflectance and the separate 2000–2020 change map.".into();
    dataset.version =
        "GLCLUC2000–2020 v2 · annual map 2020 · published URL index reviewed 2026-10-12".into();
    dataset.license = "CC BY 4.0".into();
    dataset.license_url = CC_BY.into();
    dataset.years = vec![2020];
    dataset.default_year = Some(2020);
    dataset.nominal_resolution = "0.00025 degree (about 30 m at the equator), author-published WGS84 10-degree tiles; ground cell distance varies with latitude".into();
    dataset.unit = "combined land-cover / land-use legend index".into();
    dataset.attribution = "University of Maryland Global Land Analysis and Discovery (GLAD); Potapov et al. (2022), The global 2000–2020 land cover and land use change dataset derived from the Landsat archive: first results, Frontiers in Remote Sensing 3:856903, doi:10.3389/frsen.2022.856903. Author-linked distribution: earthenginepartners-hansen/GLCLU2000-2020/v2. Retain the annual-map year, v2, tile, access date, CC BY 4.0 attribution and indication of changes.".into();
    dataset.value_interpretation = "Preserve original UInt8 annual-map indices and the embedded palette. The author's specific annual legend combines terra-firma short-vegetation cover (0–24), terra-firma tree-height classes (25–48), wetland short-vegetation cover (100–124), wetland tree-height classes (125–148), open surface-water frequency classes (200–207), snow/ice (241), cropland (244), built-up (250) and ocean (254); other ranges are not used by that annual legend. Code 0 is valid desert / 3-percent short vegetation, not missing data. Code 255 is the author's missing-data legend code, but the independently checked TIFF has no scalar NoData tag. These indices are not continuous reflectance, vegetation percentages or metres across all categories.".into();
    let map = layer(
        "land-cover",
        "Combined land cover and use · annual map 2020",
        "author's annual legend index",
        "Original UInt8 combined annual classes with the actual embedded palette. Keep valid code 0 (desert / 3-percent short vegetation), 254 ocean and author legend code 255 missing separate. The checked original has no scalar TIFF NoData declaration; no preset mask or continuous-value conversion is silently applied. The 2000–2020 change legend is not substituted.",
    );
    // A publisher legend code is not an embedded scalar NoData declaration.
    // Leave scale/offset/NoData undeclared for this categorical preset rather
    // than assigning a blanket continuous conversion or invented native mask.
    dataset.layers = vec![map];
    dataset.default_layer = Some("land-cover".into());
    dataset.warnings = vec![
        "Only the author's v2 annual map for 2020 is supported through the actual 280-URL published index. Maps for 2000, 2005, 2010, 2015 and the separate 2000–2020 change layer remain outside this connector. No unlisted tile, alternate year or sample region is substituted.".into(),
        "Published 10-degree tile bounds are derived from the author's top-left-corner filename convention and select whole originals intersecting the requested AOI. They are declared tile footprints, not verified valid-pixel, complete-coverage or cloud-free-area claims. Connecting and inspecting each selected original checks its actual grid; a tested tile does not validate every other listed original.".into(),
        "The independently decoded 30N_030E / 2020 original is EPSG:4326 PixelIsArea, 40,000 × 40,000 UInt8 samples, extent [30,20,40,30], 0.00025-degree spacing and LZW strips of one row. All 1,600,000,000 original pixels were decoded. It has a 256-entry in-file palette and no scalar TIFF NoData tag. Preserve palette indices and original bytes; do not replace numerical indices with RGB samples.".into(),
        "The author's annual-map legend specifies 255 as missing data, but the independently checked original does not embed scalar NoData. The native missing-data flag follows the physical TIFF declaration and does not silently impose the preset legend as a file mask. Before scientific analysis, explicitly interpret missing code 255 using the annual legend. Code 0 is a valid desert/short-vegetation class and must not be discarded as missing.".into(),
        "Combined class codes encode different thematic quantities and categories. Do not treat their arithmetic mean, grayscale stretch or differences between arbitrary class indices as a scientific measurement. Use the exact annual-map legend at https://storage.googleapis.com/earthenginepartners-hansen/GLCLU2000-2020/legend.xlsx; the separate change-map legend has different meanings.".into(),
        "The author warns that map pixel counts alone do not provide unbiased statistically valid area/change estimates. The maps can inform appropriate probability-based sampling; a visual map or successful transfer is not an independent scientific accuracy assessment.".into(),
        "CC BY 4.0 is specified on the author's exact v2 download page. The direct-file recipe is client provenance, not a fabricated publisher STAC collection or item. Complete original transfers are not AOI crops, resampled files or Earth Engine exports made by the client.".into(),
    ];
    dataset.limits = vec![
        format!("Up to {MAX_TILES} intersecting published originals per selection; larger or unsupported areas fail explicitly. All selections retain the requested AOI without replacing it with a test tile."),
        format!("The frozen author URL index is {INDEX_URL}, SHA-256 {INDEX_SHA256}, containing 280 original v2 / 2020 tiles. Listed availability does not guarantee every file meets existing transfer and native inspection budgets."),
        format!("The independently checked 30N_030E original is {VERIFIED_TILE_BYTES} bytes with SHA-256 {VERIFIED_TILE_SHA256}; publisher whole-file MD5 6a5c47d302414cffb4c061cc5af0dd13 was verified. Existing 512 MiB transfer, native-axis/grid, chunk/intermediate/preview-budget and deadline guards remain unchanged."),
    ];
    dataset
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != "glad-landcover-2020"
        || request.year != Some(2020)
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
        || request.layer.as_deref() != Some("land-cover")
    {
        return Err("Choose only GLAD GLCLUC v2 annual land-cover map / reference year 2020, without month, country, region or season parameters; no change layer, other year or product is substituted".into());
    }
    let b = request.bounds;
    let tiles: Vec<_> = index()
        .records
        .iter()
        .filter(|tile| {
            let t = tile.bbox;
            b[0] < t[2] && b[2] > t[0] && b[1] < t[3] && b[3] > t[1]
        })
        .collect();
    if tiles.is_empty() {
        return Err("No author-published GLAD v2 / 2020 tile intersects the requested map area; an ocean file or demonstration area is not substituted".into());
    }
    if tiles.len() > MAX_TILES {
        return Err(format!("The requested map area intersects {} GLAD originals, exceeding {MAX_TILES}; narrow the area without changing its intended location", tiles.len()));
    }
    Ok(tiles
        .into_iter()
        .map(|tile| {
            source(
                format!("UMD GLAD v2 · annual map 2020 · {}", tile.tile),
                tile.url.clone(),
                Some(tile.bbox),
            )
        })
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request() -> ResolveRequest {
        ResolveRequest {
            dataset_id: "glad-landcover-2020".into(),
            bounds: [31., 25., 32., 26.],
            year: Some(2020),
            month: None,
            country: None,
            layer: Some("land-cover".into()),
            region: None,
            season: None,
        }
    }

    #[test]
    fn complete_author_index_is_finite_unique_and_matches_only_actual_2020_paths() {
        let index = index();
        assert_eq!(index.schema_version, 1);
        assert_eq!(index.publisher_index_url, INDEX_URL);
        assert_eq!(index.publisher_index_sha256, INDEX_SHA256);
        assert!(index.grid_interpretation.contains("not fabricated STAC"));
        assert_eq!(index.year, 2020);
        assert_eq!(index.records.len(), 280);
        let mut names = std::collections::HashSet::new();
        for tile in &index.records {
            assert!(names.insert(&tile.tile));
            assert!(features::bounds(tile.bbox).is_ok());
            assert_eq!(tile.bbox[2] - tile.bbox[0], 10.);
            assert_eq!(tile.bbox[3] - tile.bbox[1], 10.);
            assert_eq!(
                tile.url,
                format!("https://storage.googleapis.com/earthenginepartners-hansen/GLCLU2000-2020/v2/2020/{}.tif", tile.tile)
            );
        }
    }

    #[test]
    fn positive_original_keeps_palette_codes_mask_distinction_and_no_fake_datetime() {
        let r = request();
        let s = resolve(&r).unwrap();
        assert_eq!(s.len(), 1);
        assert_eq!(s[0].id, "30N_030E");
        assert_eq!(s[0].stac.as_ref().unwrap().url, VERIFIED_TILE_URL);
        assert_eq!(s[0].bounds, Some([30., 20., 40., 30.]));
        assert!(s[0].start_datetime.is_none() && s[0].end_datetime.is_none());
        assert_eq!(r.bounds, [31., 25., 32., 26.]);
        let d = definition();
        assert_eq!(d.years, vec![2020]);
        assert!(d.layers[0].nodata.is_none());
        assert!(d.layers[0].scale.is_none() && d.layers[0].offset.is_none());
        assert!(d.value_interpretation.contains("Code 0 is valid"));
        assert!(d.value_interpretation.contains("no scalar NoData tag"));
        assert!(d.warnings.iter().any(|w| w.contains("RGB samples")));
        assert!(d.warnings.iter().any(|w| w.contains("arithmetic mean")));
        assert!(d.limits.iter().any(|l| l.contains(VERIFIED_TILE_SHA256)));
        assert_eq!(d.license, "CC BY 4.0");
    }

    #[test]
    fn tile_boundary_intersections_keep_both_real_files() {
        let mut r = request();
        r.bounds = [29.9, 25., 30.1, 26.];
        let s = resolve(&r).unwrap();
        assert_eq!(s.len(), 2);
        assert!(s.iter().any(|s| s.id == "30N_020E"));
        assert!(s.iter().any(|s| s.id == "30N_030E"));
        r.bounds = [30., 25., 31., 26.];
        assert_eq!(resolve(&r).unwrap().len(), 1);
    }

    #[test]
    fn unsupported_or_excessive_aoi_does_not_become_a_sample_selection() {
        for bounds in [
            [0., 85., 1., 86.],
            [0., -85., 1., -84.],
            [-180., -80., 180., 80.],
            [180.1, 0., 181., 1.],
            [32., 25., 31., 26.],
        ] {
            let mut r = request();
            r.bounds = bounds;
            assert!(resolve(&r).is_err());
        }
    }

    #[test]
    fn other_years_layers_country_season_month_region_are_rejected() {
        for year in [2000, 2005, 2010, 2015, 2021] {
            let mut r = request();
            r.year = Some(year);
            assert!(resolve(&r).is_err());
        }
        let mut r = request();
        r.year = None;
        assert!(resolve(&r).is_err());
        let mut r = request();
        r.month = Some(1);
        assert!(resolve(&r).is_err());
        let mut r = request();
        r.country = Some("EGY".into());
        assert!(resolve(&r).is_err());
        let mut r = request();
        r.region = Some("30N_030E".into());
        assert!(resolve(&r).is_err());
        let mut r = request();
        r.season = Some("annual".into());
        assert!(resolve(&r).is_err());
        for layer in ["2000-2020change", "forest-height", "treecover2000", "rgb"] {
            let mut r = request();
            r.layer = Some(layer.into());
            assert!(resolve(&r).is_err());
        }
    }
}
