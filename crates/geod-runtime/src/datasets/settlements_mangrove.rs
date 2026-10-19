//! Versioned DE Africa public WSF and GMW publisher-item recipes. The compact
//! index pins the identity, true footprint/grid and complete parsed-document
//! digest of every listed supported item. It is never a network substitute.
use super::*;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::sync::LazyLock;

pub(crate) const ADAPTER: &str = "deafrica-settlements-mangrove-public-s3";
const BUCKET: &str = "s3://deafrica-input-datasets/";
const HTTPS: &str = "https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct PublishedItem {
    product: String,
    year: u16,
    tile: String,
    id: String,
    bbox: [f64; 4],
    shape: [u32; 2],
    digest: String,
    bytes: u64,
}

static ITEMS: LazyLock<Vec<PublishedItem>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("settlements-mangrove-published-index.json"))
        .expect("compile-time reviewed WSF/GMW publisher-item index")
});

fn product(dataset_id: &str) -> Result<(&'static str, &'static str)> {
    match dataset_id {
        "wsf-settlements" => Ok(("wsf", "settlement")),
        "gmw-mangroves" => Ok(("gmw", "mangrove")),
        _ => Err("Choose a reviewed WSF settlement or GMW mangrove preset".into()),
    }
}

fn path(item: &PublishedItem) -> String {
    if item.product == "wsf" {
        let version = if item.year == 2015 { "v2" } else { "v1" };
        format!(
            "wsf_{year}/{tile}/WSF{year}_{version}_{tile}.stac-item.json",
            year = item.year,
            tile = item.tile
        )
    } else {
        format!(
            "gmw/{year}/{tile}/GMW_{tile}_{year}_v3.stac-item.json",
            year = item.year,
            tile = item.tile
        )
    }
}

fn asset_key(item: &PublishedItem) -> String {
    if item.product == "wsf" {
        format!("wsf{}", item.year)
    } else {
        "mangrove".into()
    }
}

pub(super) fn definitions() -> Vec<Dataset> {
    let mut wsf = base(
        "wsf-settlements",
        "World Settlement Footprint · DE Africa public tiles",
        "land-cover",
        "https://docs.digitalearthafrica.org/en/latest/data_specs/World_Settlement_Footprint_specs.html",
    );
    wsf.description = "Historical settlement extent from DLR's WSF2015 v2 and WSF2019 v1, accessed through the actual Digital Earth Africa public regional tile archive. This is a binary settlement footprint, not raw satellite reflectance, building outlines or current population.".into();
    wsf.version =
        "WSF2015 v2 / WSF2019 v1 · reviewed DE Africa publisher-item index 2026-10-12".into();
    wsf.years = vec![2015, 2019];
    wsf.default_year = Some(2019);
    wsf.discovery = Some("stac-items".into());
    wsf.nominal_resolution = "approximately 10 m (0.00008983152841195215 degree); retain each original's actual grid and edge overlap".into();
    wsf.unit = "settlement mask code".into();
    wsf.attribution = "German Aerospace Center (DLR), WSF2015 v2 / WSF2019 v1; contains processed Landsat/Copernicus Sentinel observations. Public African distribution: Digital Earth Africa. Cite the selected epoch, item, version, tile and access date; retain CC BY 4.0 attribution.".into();
    wsf.value_interpretation = "Original binary mask codes: 255 settlement, 0 background. Reviewed original files do not have an embedded TIFF NoData tag, so 0 and 255 remain readable raw samples. ODC fill conventions do not rewrite the original's missing-data declaration. Historical inferred settlement extent is not an individual-building or population census.".into();
    wsf.layers = vec![layer(
        "settlement",
        "Historical settlement footprint",
        "mask code",
        "255 settlement; 0 background. No inferred conversion to 0/1, no imposed NoData value, and no claim of raw satellite reflectance.",
    )];
    wsf.default_layer = Some("settlement".into());
    wsf.warnings = vec![
        "Only the actual listed DE Africa regional distribution is supported: 771 reviewed publisher tiles for each of 2015 and 2019, including some adjacent-region tiles. It is not a global WSF mirror. An unsupported year/month or an area with no matching published tile fails explicitly; no alternate area or epoch is substituted.".into(),
        "WSF settlement is encoded as 255. The independently decoded 2015/2019 originals have no TIFF NoData tag: native zero-background reads remain noData=false. The ODC documentation's fill value 0 is a separate loading convention and must not silently alter original TIFF samples.".into(),
        "Actual tile dimensions and padding vary by tile (including 22,487/22,488 pixel axes). Preserve publisher shape/transform and the real edge overlap; do not replace the footprint with a nominal 2-degree rectangle or assume identical grids between different files.".into(),
        "Publisher item assets use original s3 URLs. Only this preset's exact pinned bucket/path receives the fixed public HTTPS transport; archived publisher JSON and original asset declarations remain unchanged. No private bucket, signed URL or AWS account is involved.".into(),
        "Standalone items have no STAC collection/license link. CC BY 4.0 is separately documented by DLR/DE Africa, not an invented publisher collection declaration. A complete tile download is not clipped to the map area; bbox intersection and a grayscale preview are not verified valid-settlement coverage or accuracy.".into(),
    ];
    wsf.limits = common_limits();

    let mut gmw = base(
        "gmw-mangroves",
        "Global Mangrove Watch v3 · DE Africa coastal tiles",
        "vegetation",
        "https://docs.digitalearthafrica.org/en/latest/data_specs/Global_Mangrove_Watch_specs.html",
    );
    gmw.description = "Historical mangrove extent from Global Mangrove Watch v3, accessed through Digital Earth Africa's public regional coastal tiles for 2017–2020. This thematic mangrove mask is not optical imagery, biomass or a blue-carbon estimate.".into();
    gmw.version =
        "GMW v3.0 · 2017–2020 DE Africa tiled archive · reviewed publisher-item index 2026-10-12"
            .into();
    gmw.years = vec![2017, 2018, 2019, 2020];
    gmw.default_year = Some(2020);
    gmw.discovery = Some("stac-items".into());
    gmw.nominal_resolution = "approximately 25 m (0.000222222222222 degree v3 tile grid); not the older continental 0.0002-degree grid".into();
    gmw.unit = "mangrove mask code".into();
    gmw.attribution = "Global Mangrove Watch / Aberystwyth University, soloEO, Wetlands International, UNEP-WCMC and JAXA; Bunting et al. (2022), GMW Version 3.0, doi:10.5281/zenodo.6894273. African tiled distribution: Digital Earth Africa. Retain CC BY 4.0 attribution and cite the actual year, version and item.".into();
    gmw.value_interpretation = "Original mangrove mask: 1 mangrove, 0 background/fill. Reviewed v3 files embed TIFF NoData=0; the native missing-data flag follows that original declaration. Since background and missing data share the original zero code, zero does not establish valid non-mangrove observations. This is historical modeled extent rather than raw reflectance, biomass or measured carbon.".into();
    let mut mangrove = layer(
        "mangrove",
        "Historical mangrove extent",
        "mask code",
        "1 mangrove; 0 background/fill and embedded TIFF NoData. Preserve the original code and mask; zero cannot prove valid non-mangrove coverage.",
    );
    mangrove.nodata = Some(0.);
    gmw.layers = vec![mangrove];
    gmw.default_layer = Some("mangrove".into());
    gmw.warnings = vec![
        "Only the 249 actual DE Africa regional coastal v3 tiles per year for 2017–2020 are reviewed, including some adjacent-region tiles. This is not a global mirror despite the product name. Unsupported years/months or map areas with no available coastal tile fail explicitly; no sample coastline is substituted.".into(),
        "The old 1996–2016 continental originals have 454,300 × 431,600 pixel grids and exceed current native axis/grid limits even though their compressed byte size is under 512 MiB. They are excluded; this connector uses the real v3 one-degree tiles without increasing resource limits.".into(),
        "Actual v3 tiles have approximately 0.000222222222222 degree grid cells, different from the older documentation's continental 0.0002-degree grid. Most grids are 4,500 × 4,500; the eastern N26E064 archive edge has 4,500 rows × 2,250 columns and a half-degree footprint. Original shape, transform, footprint and embedded NoData are retained; do not promise exact 20 m spatial detail or impose another product's grid or mask.".into(),
        "Mangrove 1 is a positive modeled class. Zero is both background/fill and the original TIFF missing-data declaration, so an all-zero or transparent thumbnail is not evidence of useful coverage. A positive raw value, bbox intersection or map overlay does not validate mangrove accuracy or change detection.".into(),
        "Publisher JSON and original s3 assets remain unchanged. Only the exact reviewed public bucket/item/asset obtains a matching HTTPS transport. Standalone collection metadata is absent; documented CC BY 4.0 attribution is separate client provenance, not an invented STAC license field.".into(),
    ];
    gmw.limits = common_limits();
    vec![wsf, gmw]
}

fn common_limits() -> Vec<String> {
    vec![
        "At most 8 actual intersecting publisher tiles per selection; narrow larger areas. The complete original tile is downloaded without clipping.".into(),
        "Each original remains limited to 512 MiB. Existing tiled-axis (131,072), grid-pixel (8 billion), stripped-axis (65,536), chunk/intermediate/preview-budget and deadline guards remain unchanged. Metadata connection is not full-original download acceptance.".into(),
    ]
}

fn intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] < b[2] && a[2] > b[0] && a[1] < b[3] && a[3] > b[1]
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    let (wanted_product, wanted_layer) = product(&request.dataset_id)?;
    let year = request.year.ok_or("Choose a published historical epoch")?;
    let supported = match wanted_product {
        "wsf" => [2015, 2019].contains(&year),
        _ => [2017, 2018, 2019, 2020].contains(&year),
    };
    if !supported
        || request.month.is_some()
        || request.country.is_some()
        || request.season.is_some()
    {
        return Err("This historical tile preset uses a reviewed annual epoch; unsupported year, month, country or season is not substituted".into());
    }
    if request.layer.as_deref() != Some(wanted_layer) {
        return Err("Choose the reviewed thematic layer for this product".into());
    }
    let candidates = ITEMS
        .iter()
        .filter(|item| item.product == wanted_product && item.year == year)
        .filter(|item| {
            request
                .region
                .as_deref()
                .is_none_or(|tile| item.tile == tile)
        })
        .filter(|item| intersects(item.bbox, request.bounds))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err("The requested map area does not intersect an available reviewed DE Africa regional publisher tile for this product/year. No global file, different coastline, sample area or epoch is substituted".into());
    }
    if candidates.len() > MAX_TILES {
        return Err(
            "This area intersects more than 8 actual publisher tiles; narrow the map area".into(),
        );
    }
    if candidates
        .iter()
        .any(|item| item.bytes == 0 || item.bytes > 512 * 1024 * 1024)
    {
        return Err("A reviewed original exceeds the current complete-file transfer limit".into());
    }
    candidates
        .into_iter()
        .map(|item| {
            let title = format!(
                "{} {} · {}",
                if item.product == "wsf" {
                    "WSF"
                } else {
                    "GMW v3"
                },
                item.year,
                item.tile
            );
            Ok(DatasetSource {
                id: item.id.clone(),
                title: title.clone(),
                bounds: Some(item.bbox),
                stac: Some(stac::ConnectRequest {
                    name: title,
                    url: format!("{HTTPS}{}", path(item)),
                    kind: "item".into(),
                }),
                wcs: None,
                coverage_id: None,
                search: None,
                region: Some(item.tile.clone()),
                season: None,
                start_datetime: Some(format!("{}-01-01T00:00:00Z", item.year)),
                end_datetime: Some(format!("{}-12-31T23:59:59Z", item.year)),
                catalog_raster: None,
            })
        })
        .collect()
}

/// A deterministic parsed-JSON digest avoids storing 2,538 expanded documents
/// in the executable. It hashes the complete tree, including unknown publisher
/// fields. Ordered object keys, explicit type/length tags and numeric f64 bits
/// avoid formatting-dependent JSON hashes; numbers are never reinterpreted as
/// units, calibrated values or an alternative grid.
fn document_digest(value: &Value) -> Result<String> {
    fn feed(value: &Value, hash: &mut Sha256) -> Result<()> {
        match value {
            Value::Null => hash.update(b"n"),
            Value::Bool(value) => hash.update(if *value { b"t" } else { b"f" }),
            Value::Number(value) => {
                let number = value
                    .as_f64()
                    .filter(|v| v.is_finite())
                    .ok_or("Invalid publisher number")?;
                hash.update(b"d");
                hash.update(number.to_bits().to_be_bytes());
            }
            Value::String(value) => {
                hash.update(b"s");
                hash.update((value.len() as u64).to_be_bytes());
                hash.update(value.as_bytes());
            }
            Value::Array(values) => {
                hash.update(b"a");
                hash.update((values.len() as u64).to_be_bytes());
                for value in values {
                    feed(value, hash)?;
                }
            }
            Value::Object(values) => {
                hash.update(b"o");
                hash.update((values.len() as u64).to_be_bytes());
                let mut entries = values.iter().collect::<Vec<_>>();
                entries.sort_by(|a, b| a.0.cmp(b.0));
                for (key, value) in entries {
                    feed(&Value::String(key.clone()), hash)?;
                    feed(value, hash)?;
                }
            }
        }
        Ok(())
    }
    let mut hash = Sha256::new();
    feed(value, &mut hash)?;
    Ok(format!("{:x}", hash.finalize()))
}

fn reviewed(url: &str) -> Result<&'static PublishedItem> {
    ITEMS
        .iter()
        .find(|item| format!("{HTTPS}{}", path(item)) == url)
        .ok_or("Publisher item is outside the exact reviewed public WSF/GMW index".into())
}

fn check_document(item: &PublishedItem, url: &str, raw: &Value) -> Result<()> {
    let item_path = path(item);
    let stem = item_path
        .strip_suffix(".stac-item.json")
        .ok_or("Invalid reviewed item path")?;
    let key = asset_key(item);
    let properties = &raw["properties"];
    let shape: [u32; 2] =
        serde_json::from_value(properties["proj:shape"].clone()).map_err(crate::io_error)?;
    let bounds: [f64; 4] = serde_json::from_value(raw["bbox"].clone()).map_err(crate::io_error)?;
    let expected_product = if item.product == "wsf" {
        format!("wsf_{}", item.year)
    } else {
        "gmw".into()
    };
    let assets = raw["assets"]
        .as_object()
        .ok_or("Publisher assets are missing")?;
    let self_links = raw["links"]
        .as_array()
        .ok_or("Publisher links are missing")?
        .iter()
        .filter(|link| link["rel"] == "self")
        .collect::<Vec<_>>();
    if url != format!("{HTTPS}{item_path}")
        || raw["type"] != "Feature"
        || raw["id"] != item.id
        || properties["odc:product"] != expected_product
        || properties["odc:region_code"] != item.tile
        || properties["proj:epsg"] != 4326
        || properties["start_datetime"] != format!("{}-01-01T00:00:00Z", item.year)
        || properties["end_datetime"] != format!("{}-12-31T23:59:59Z", item.year)
        || shape != item.shape
        || bounds != item.bbox
        || assets.len() != 1
        || raw["assets"][&key]["href"] != format!("{BUCKET}{stem}.tif")
        || self_links.len() != 1
        || self_links[0]["href"] != format!("{BUCKET}{item_path}")
        || document_digest(raw)? != item.digest
    {
        return Err("Reviewed WSF/GMW publisher document changed: identity, epoch, version, grid, region, original asset or other publisher metadata differs from the pinned document".into());
    }
    Ok(())
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    if proof.schema_version != 2 {
        return Err("Unsupported reviewed publisher-item provenance version".into());
    }
    product(&proof.dataset.id)?;
    let plan = super::resolve(proof.request.clone())?;
    let source = plan
        .sources
        .iter()
        .find(|source| {
            source.id == proof.source_id
                && source.stac.as_ref().is_some_and(|value| value.url == url)
        })
        .ok_or(
            "Reviewed WSF/GMW publisher item does not match the requested fixed tile/year recipe",
        )?;
    if super::provenance(&plan, source) != *proof {
        return Err("Reviewed WSF/GMW declaration differs from its fixed preset".into());
    }
    let item = reviewed(url)?;
    if let Some(raw) = raw {
        check_document(item, url, raw)?;
    }
    Ok(())
}

pub(crate) fn transports(
    proof: &SourceProvenance,
    url: &str,
    raw: &Value,
) -> Result<Vec<stac::AssetTransport>> {
    validate(proof, url, Some(raw))?;
    let item = reviewed(url)?;
    let key = asset_key(item);
    let item_path = path(item);
    let stem = item_path
        .strip_suffix(".stac-item.json")
        .ok_or("Invalid reviewed item path")?;
    Ok(vec![stac::AssetTransport {
        asset_key: key.clone(),
        original_href: raw["assets"][&key]["href"]
            .as_str()
            .ok_or("Missing publisher asset href")?
            .into(),
        href: format!("{HTTPS}{stem}.tif"),
        adapter: ADAPTER.into(),
    }])
}

pub(crate) fn self_url(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<String> {
    validate(proof, url, Some(raw))?;
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(id: &str, year: u16, bounds: [f64; 4]) -> ResolveRequest {
        ResolveRequest {
            dataset_id: id.into(),
            bounds,
            year: Some(year),
            month: None,
            country: None,
            layer: Some(product(id).unwrap().1.into()),
            region: None,
            season: None,
        }
    }

    fn document(product: &str, year: u16) -> Value {
        match (product, year) {
            ("wsf", 2015) => {
                serde_json::from_str(include_str!("settlements_mangrove_fixtures/wsf2015.json"))
                    .unwrap()
            }
            ("wsf", 2019) => {
                serde_json::from_str(include_str!("settlements_mangrove_fixtures/wsf2019.json"))
                    .unwrap()
            }
            ("gmw", 2020) => {
                serde_json::from_str(include_str!("settlements_mangrove_fixtures/gmw2020.json"))
                    .unwrap()
            }
            _ => panic!("No independently downloaded fixture"),
        }
    }

    #[test]
    fn whole_index_contains_only_actual_complete_supported_year_tiles() {
        assert_eq!(ITEMS.len(), 2538);
        for year in [2015, 2019] {
            assert_eq!(
                ITEMS
                    .iter()
                    .filter(|item| item.product == "wsf" && item.year == year)
                    .count(),
                771
            );
        }
        for year in [2017, 2018, 2019, 2020] {
            assert_eq!(
                ITEMS
                    .iter()
                    .filter(|item| item.product == "gmw" && item.year == year)
                    .count(),
                249
            );
        }
        let mut ids = std::collections::HashSet::new();
        let mut paths = std::collections::HashSet::new();
        for item in ITEMS.iter() {
            assert!(ids.insert(item.id.clone()));
            assert!(paths.insert(path(item)));
            assert!(item.bytes > 0 && item.bytes <= 512 * 1024 * 1024);
            assert!(item.shape.into_iter().all(|axis| axis > 0 && axis <= 22500));
            assert_eq!(item.digest.len(), 64);
        }
    }

    #[test]
    fn actual_aoi_and_fixed_item_periods_resolve_without_replacing_area() {
        for year in [2015, 2019] {
            let selected = request("wsf-settlements", year, [36.818, -1.292, 36.822, -1.288]);
            let sources = resolve(&selected).unwrap();
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0].region.as_deref(), Some("36_-2"));
            assert_eq!(
                sources[0].start_datetime,
                Some(format!("{year}-01-01T00:00:00Z"))
            );
            assert_eq!(
                sources[0].end_datetime,
                Some(format!("{year}-12-31T23:59:59Z"))
            );
            assert!(sources[0].season.is_none());
            assert_eq!(
                sources[0].bounds,
                Some([
                    35.989923879907685,
                    -2.010070279745841,
                    38.010055290835666,
                    0.01006113118213864
                ])
            );
        }
        let sources = resolve(&request(
            "gmw-mangroves",
            2020,
            [39.105, -5.023, 39.110, -5.018],
        ))
        .unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].region.as_deref(), Some("S05E039"));
        assert_eq!(sources[0].id, "91259ca9-521e-5886-a032-eede2de1f73e");
    }

    #[test]
    fn unsupported_epoch_region_parameters_and_large_area_fail_explicitly() {
        for (id, year) in [("wsf-settlements", 2020), ("gmw-mangroves", 2016)] {
            assert!(resolve(&request(id, year, [36.8, -1.3, 36.9, -1.2])).is_err());
        }
        for id in ["wsf-settlements", "gmw-mangroves"] {
            let year = if id == "wsf-settlements" { 2019 } else { 2020 };
            assert!(resolve(&request(id, year, [116., 39., 117., 40.])).is_err());
            assert!(resolve(&request(id, year, [-26., -48., 64., 38.])).is_err());
            let mut selected = request(id, year, [39.1, -5.1, 39.2, -5.01]);
            selected.month = Some(1);
            assert!(resolve(&selected).is_err());
            selected.month = None;
            selected.region = Some("../../outside-bucket".into());
            assert!(resolve(&selected).is_err());
            selected.region = None;
            selected.season = Some("unknown".into());
            assert!(resolve(&selected).is_err());
        }
    }

    #[test]
    fn true_wsf_edge_overlap_selects_both_published_original_footprints() {
        let selected = request("wsf-settlements", 2019, [37.995, -1.3, 38.005, -1.2]);
        let sources = resolve(&selected).unwrap();
        assert_eq!(sources.len(), 2);
        assert!(sources
            .iter()
            .any(|source| source.region.as_deref() == Some("36_-2")));
        assert!(sources
            .iter()
            .any(|source| source.region.as_deref() == Some("38_-2")));
        assert!(sources
            .iter()
            .all(|source| intersects(source.bounds.unwrap(), selected.bounds)));
    }

    #[test]
    fn gmw_eastern_archive_edge_retains_actual_half_width_and_footprint() {
        let sources = resolve(&request("gmw-mangroves", 2020, [64.1, 25.4, 64.2, 25.5])).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].region.as_deref(), Some("N26E064"));
        assert_eq!(sources[0].bounds.unwrap()[2], 64.4999999999995);
        assert!(resolve(&request("gmw-mangroves", 2020, [64.6, 25.4, 64.7, 25.5])).is_err());
    }

    #[test]
    fn publisher_json_whole_tree_digest_and_original_contract_are_pinned() {
        for (product, year, tile) in [
            ("wsf", 2015, "36_-2"),
            ("wsf", 2019, "36_-2"),
            ("gmw", 2020, "S05E039"),
        ] {
            let item = ITEMS
                .iter()
                .find(|item| item.product == product && item.year == year && item.tile == tile)
                .unwrap();
            let url = format!("{HTTPS}{}", path(item));
            let raw = document(product, year);
            check_document(item, &url, &raw).unwrap();
            for pointer in [
                "/id",
                "/properties/odc:product",
                "/properties/odc:region_code",
                "/properties/start_datetime",
            ] {
                let mut changed = raw.clone();
                *changed.pointer_mut(pointer).unwrap() = json!("unreviewed");
                assert!(check_document(item, &url, &changed).is_err());
            }
            let mut changed = raw.clone();
            changed["bbox"][0] = json!(116.);
            assert!(check_document(item, &url, &changed).is_err());
            let mut changed = raw.clone();
            changed["properties"]["unknown_extra_publisher_field"] =
                json!("not present in reviewed document");
            assert!(check_document(item, &url, &changed).is_err());
            let mut changed = raw.clone();
            changed["assets"][asset_key(item)]["href"] =
                json!("s3://private-bucket/unreviewed.tif");
            assert!(check_document(item, &url, &changed).is_err());
        }
    }

    #[test]
    fn dataset_proof_cannot_relabel_epoch_layer_grid_transport_or_source() {
        for (id, year, bounds, product) in [
            (
                "wsf-settlements",
                2019,
                [36.818, -1.292, 36.822, -1.288],
                "wsf",
            ),
            (
                "gmw-mangroves",
                2020,
                [39.105, -5.023, 39.110, -5.018],
                "gmw",
            ),
        ] {
            let plan = super::super::resolve(request(id, year, bounds)).unwrap();
            let proof = super::super::provenance(&plan, &plan.sources[0]);
            let url = &plan.sources[0].stac.as_ref().unwrap().url;
            let raw = document(product, year);
            validate(&proof, url, Some(&raw)).unwrap();
            let transport = transports(&proof, url, &raw).unwrap();
            assert_eq!(transport.len(), 1);
            assert!(transport[0].original_href.starts_with(BUCKET));
            assert!(transport[0].href.starts_with(HTTPS));
            assert_eq!(self_url(&proof, url, &raw).unwrap(), *url);
            let mut changed = proof.clone();
            changed.dataset.nominal_resolution = "1 cm optical reflectance".into();
            assert!(validate(&changed, url, Some(&raw)).is_err());
            let mut changed = proof.clone();
            changed.request.bounds = [116., 39., 117., 40.];
            assert!(validate(&changed, url, Some(&raw)).is_err());
            let mut changed = proof.clone();
            changed.source_id = "unreviewed".into();
            assert!(validate(&changed, url, Some(&raw)).is_err());
            assert!(validate(&proof, "https://unreviewed.example/item.json", Some(&raw)).is_err());
        }
    }
}
