//! Three fixed JRC GHSL quantitative products for reference epoch 2020.
//! A direct original-file recipe is client provenance, not publisher STAC.
//! Legal query extents are separate from the unchanged expanded raw grids.
use super::*;
use serde::Deserialize;
use std::sync::LazyLock;

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Profiles {
    profiles: Vec<Profile>,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Profile {
    dataset_id: String,
    code: String,
    title: String,
    layer: String,
    unit: String,
    category: String,
    documentation_url: String,
    registry_url: String,
    doi: String,
    citation: String,
    url: String,
    bytes: u64,
    sha256: String,
    data_type: String,
    width: u32,
    height: u32,
    source_bounds: [f64; 4],
    query_bounds: [f64; 4],
    transform: [f64; 6],
    nodata: Option<f64>,
    publisher_no_data: f64,
    scale: f64,
    offset: f64,
    valid_zero_count: u64,
    valid_max: f64,
    decoded_pixels: u64,
}
static PROFILES: LazyLock<Profiles> = LazyLock::new(|| {
    serde_json::from_str(include_str!("next-ghsl-profiles.json"))
        .expect("compile-time independently decoded JRC GHSL 2020 original profiles")
});

fn description(id: &str) -> &'static str {
    match id {
        "ghsl-built-surface" => "Total built-up surface in square metres per cell from JRC GHS-BUILT-S R2023A, reference epoch 2020. The published model uses Landsat / Sentinel-2 evidence with temporal interpolation. This is an aggregate numerical surface quantity, not percentage cover, a 10 m scene, building footprints, non-residential-only area, volume or the connected SMOD settlement classes.",
        "ghsl-built-volume" => "Total built-up volume in cubic metres per cell from JRC GHS-BUILT-V R2023A, reference epoch 2020. The model combines built-up surface and vertical information from imagery and global elevation models. This is an aggregate numerical volume quantity, not individual building height, floor area, a measured cadastral building model, non-residential-only volume or population.",
        "ghsl-population" => "Residential population estimates per cell from JRC GHS-POP R2023A, reference epoch 2020. Population census / administrative totals from CIESIN GPWv4.11 are spatially disaggregated using corresponding GHSL built-up surface, volume and classification. Preserve fractional model counts; this is not people per square kilometre, an individual count, a live population measurement, WorldPop or a 2025/2030 projection.",
        _ => unreachable!("reviewed GHSL product profile"),
    }
}
fn interpretation(id: &str) -> &'static str {
    match id {
        "ghsl-built-surface" => "Original UInt32 square metres of total built-up surface per cell, scale 1, offset 0 and no embedded TIFF NoData. Zero is a valid raw amount. The publisher's 30 arc-second table declares NoData 4294967295, but this checked original has no such tag and all decoded values are 0–848108; keep the declaration and physical-file difference visible without inventing a mask or changing values. Cell area varies with latitude; no automatic percentage conversion is performed.",
        "ghsl-built-volume" => "Original UInt32 cubic metres of total built-up volume per cell, scale 1, offset 0 and no embedded TIFF NoData. Zero is a valid raw amount. The publisher's 30 arc-second table declares NoData 4294967295, but this checked original has no such tag and all decoded values are 0–16413999; preserve that difference without inventing a mask. Volume is not height or floor area; no automatic division by surface or elevation subtraction is performed.",
        "ghsl-population" => "Original Float64 modeled residents per cell, scale 1, offset 0 and no embedded TIFF NoData. Preserve fractional counts and valid zero; do not round to integers, downgrade to Float32 or report density without a separate area calculation. The publisher's table declares NoData -200, but this checked original lacks that tag and contains no negative values. Native scalar NoData therefore remains absent instead of applying an invented mask.",
        _ => unreachable!("reviewed GHSL product profile"),
    }
}

pub(super) fn definitions() -> Vec<Dataset> {
    PROFILES.profiles.iter().map(|profile| {
        let mut d = base(&profile.dataset_id, &profile.title, &profile.category, &profile.documentation_url);
        d.description = description(&profile.dataset_id).into();
        d.version = format!("{} R2023A V1.0 · EPSG:4326 30ss · reference epoch 2020", profile.code);
        d.license = "CC BY 4.0".into();
        d.license_url = CC_BY.into();
        d.nominal_resolution = "30 arc-seconds (about 0.0083333333 degree), approximately 1 km at the equator; ground cell dimensions vary with latitude".into();
        d.unit = profile.unit.clone();
        d.years = vec![2020];
        d.default_year = Some(2020);
        d.attribution = format!("{} JRC-managed AWS distribution: {}. Reviewed V1.0 / WGS84 30ss / reference epoch 2020 original. Retain CC BY 4.0 attribution, selected quantity, release, access date and an indication of changes.", profile.citation, profile.registry_url);
        d.value_interpretation = interpretation(&profile.dataset_id).into();
        let mut quantity = layer(&profile.layer, &profile.title, &profile.unit, interpretation(&profile.dataset_id));
        quantity.scale = Some(profile.scale);
        quantity.offset = Some(profile.offset);
        quantity.nodata = profile.nodata;
        d.layers = vec![quantity];
        d.default_layer = Some(profile.layer.clone());
        d.warnings = vec![
            "Only the independently decoded R2023A V1.0 / reference epoch 2020 / WGS84 30 arc-second total original for the selected quantity is connected. Other epochs, 2025/2030 projections, 2018 10 m observations, 3 arc-second files, Mollweide grids, non-residential-only variants and different GHSL products are not substituted.".into(),
            "2020 is a modeled reference epoch, not the acquisition date of one satellite scene. Census disaggregation and temporal modeling do not establish a contemporary observation or per-building truth. No artificial scene datetime or acquisition interval is assigned to the direct original.".into(),
            "The original global PixelIsArea grid extends slightly beyond longitude ±180 degrees. Only the recipe's legal query/display footprint is intersected with physical WGS84 bounds; the original full columns, TIFF transform, outer bounds, bytes and numerical samples remain unchanged. Map fitting is not scientific cropping or a new publisher extent.".into(),
            "The publisher's product-table NoData declaration and the absence of an embedded TIFF NoData tag are retained separately. All values of the checked originals are non-negative and below the declared missing code where applicable. Zero remains a valid raw number; lack of an embedded mask is not proof of observed coverage, accuracy or scientific quality everywhere.".into(),
            "Complete original files are downloaded regardless of the selected AOI, without clipping, reprojection, resampling, unit conversion or a generated subset. Published internal overviews are display layers, while original-pixel inspection uses the full-resolution grid; an averaged overview is not an exact original sample.".into(),
            "CC BY 4.0 is declared by the JRC-managed AWS registry. Public byte access does not remove attribution requirements. S3 multipart ETags are not whole-file MD5 checksums; independently measured SHA256 is retained separately from archive publication dates.".into(),
        ];
        d.limits = vec![
            "One complete reviewed historical global original per selected product; no country, named region, month or season parameter is supported.".into(),
            format!("Independently decoded original: {} bytes, {} x {} {}, SHA256 {}. Raw outer bounds {:?}; original affine transform {:?}. All {} samples were independently decoded; {} valid zeros remain; maximum raw value {}. Publisher table missing-code declaration {}, actual TIFF NoData {:?}.", profile.bytes, profile.width, profile.height, profile.data_type, profile.sha256, profile.source_bounds, profile.transform, profile.decoded_pixels, profile.valid_zero_count, profile.valid_max, profile.publisher_no_data, profile.nodata),
            "Existing 512 MiB transfer, native-axis/grid, chunk, intermediate-buffer, memory, PNG and deadline guards remain unchanged. A directory entry or successful connection is separate from actual original download, scientific inspection, online map and restart acceptance.".into(),
        ];
        d
    }).collect()
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    let profile = PROFILES
        .profiles
        .iter()
        .find(|v| v.dataset_id == request.dataset_id)
        .ok_or("Choose one reviewed GHS-BUILT-S, GHS-BUILT-V or GHS-POP product")?;
    if request.year != Some(2020)
        || request.layer.as_deref() != Some(profile.layer.as_str())
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose only the exact reviewed GHSL quantity / R2023A V1.0 WGS84 30ss / reference epoch 2020 without month, country, region or season parameters; another quantity, release or prediction epoch is not substituted".into());
    }
    let expected_dtype = if profile.dataset_id == "ghsl-population" {
        "Float64"
    } else {
        "UInt32"
    };
    if profile.bytes == 0
        || profile.bytes > 512 * 1024 * 1024
        || profile.sha256.len() != 64
        || profile.data_type != expected_dtype
        || profile.nodata.is_some()
        || profile.scale != 1.
        || profile.offset != 0.
        || profile.height != 21384
        || profile.width
            != if profile.dataset_id == "ghsl-population" {
                43202
            } else {
                43201
            }
        || !profile.doi.starts_with("10.2905/")
    {
        return Err("The fixed original GHSL profile has changed storage, calibration, grid or receipt declaration".into());
    }
    let b = request.bounds;
    let q = profile.query_bounds;
    if b[0] >= q[2] || b[2] <= q[0] || b[1] >= q[3] || b[3] <= q[1] {
        return Err("The map area does not intersect the legal query footprint of the exact reviewed GHSL original; no polar area, another epoch or generated subset is selected".into());
    }
    let original = source(
        format!("JRC {} V1.0 · total · reference epoch 2020", profile.code),
        profile.url.clone(),
        Some(profile.query_bounds),
    );
    // The recipe carries year 2020 as a model epoch. No acquisition timestamps
    // are invented. Full raw grid bounds stay in the reviewed profile and are
    // independently read again by the native original-file inspector.
    Ok(vec![original])
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(id: &str, layer: &str) -> ResolveRequest {
        ResolveRequest {
            dataset_id: id.into(),
            layer: Some(layer.into()),
            bounds: [116.3, 39.8, 116.5, 40.],
            year: Some(2020),
            month: None,
            country: None,
            region: None,
            season: None,
        }
    }
    #[test]
    fn each_quantity_selects_its_exact_complete_file_and_reference_epoch() {
        assert_eq!(PROFILES.profiles.len(), 3);
        for profile in &PROFILES.profiles {
            let original = resolve(&request(&profile.dataset_id, &profile.layer)).unwrap();
            assert_eq!(original.len(), 1);
            assert_eq!(original[0].stac.as_ref().unwrap().url, profile.url);
            assert_eq!(original[0].stac.as_ref().unwrap().kind, "raster");
            assert!(
                original[0].search.is_none()
                    && original[0].start_datetime.is_none()
                    && original[0].end_datetime.is_none()
            );
            assert!(original[0].stac.as_ref().unwrap().name.len() <= 80);
            let d = definitions()
                .into_iter()
                .find(|v| v.id == profile.dataset_id)
                .unwrap();
            assert_eq!(d.years, vec![2020]);
            assert_eq!(d.default_year, Some(2020));
            assert!(d.months.is_empty() && d.regions.is_empty() && d.seasons.is_empty());
            assert_eq!(d.layers[0].unit, profile.unit);
            assert_eq!(d.license, "CC BY 4.0");
        }
    }
    #[test]
    fn raw_grids_typed_values_valid_zero_and_absent_mask_are_preserved() {
        for profile in &PROFILES.profiles {
            assert!(profile.source_bounds[0] < -180. && profile.source_bounds[2] > 180.);
            assert_eq!(profile.query_bounds[0], -180.);
            assert_eq!(profile.query_bounds[2], 180.);
            let d = definitions()
                .into_iter()
                .find(|v| v.id == profile.dataset_id)
                .unwrap();
            assert_eq!(d.layers[0].nodata, None);
            assert_eq!(d.layers[0].scale, Some(1.));
            assert_eq!(d.layers[0].offset, Some(0.));
            assert!(d.limits.iter().any(|v| v.contains(&profile.sha256)));
            assert_eq!(
                profile.decoded_pixels,
                u64::from(profile.width) * u64::from(profile.height)
            );
            assert!(profile.valid_zero_count > 800_000_000);
            if profile.dataset_id == "ghsl-population" {
                assert_eq!(profile.data_type, "Float64");
                assert_eq!(profile.publisher_no_data, -200.);
                assert!(profile.valid_max.fract() > 0.);
            } else {
                assert_eq!(profile.data_type, "UInt32");
                assert_eq!(profile.publisher_no_data, 4294967295.);
            }
            let files = resolve(&request(&profile.dataset_id, &profile.layer)).unwrap();
            assert_eq!(files[0].bounds, Some(profile.query_bounds));
        }
    }
    #[test]
    fn unsupported_epochs_quantities_parameters_and_invalid_aoi_fail_without_fallback() {
        for profile in &PROFILES.profiles {
            let original = request(&profile.dataset_id, &profile.layer);
            for year in [None, Some(2018), Some(2025), Some(2030)] {
                let mut r = original.clone();
                r.year = year;
                assert!(resolve(&r).is_err());
            }
            for field in 0..5 {
                let mut r = original.clone();
                match field {
                    0 => r.month = Some(10),
                    1 => r.country = Some("CHN".into()),
                    2 => r.region = Some("globe".into()),
                    3 => r.season = Some("annual".into()),
                    _ => r.layer = Some("non-residential".into()),
                };
                assert!(resolve(&r).is_err());
            }
            for bounds in [
                [-181., 0., -179., 1.],
                [179.9, 0., 180.1, 1.],
                [0., 89.2, 1., 90.],
                [0., -90., 1., -89.2],
            ] {
                let mut r = original.clone();
                r.bounds = bounds;
                assert!(resolve(&r).is_err());
            }
            for bounds in [
                [-180., 0., -179.9, 0.1],
                [179.9, 0., 180., 0.1],
                [0., 0., 0.1, 0.1],
            ] {
                let mut r = original.clone();
                r.bounds = bounds;
                assert!(resolve(&r).is_ok());
            }
        }
        assert!(resolve(&request("ghsl-smod", "built-surface")).is_err());
        assert!(resolve(&request("ghsl-built-surface", "built-volume")).is_err());
    }
}
