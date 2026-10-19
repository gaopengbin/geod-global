//! Two whole, unchanged numerical members from one accepted author ZIP.
//! The archive source is not a TIFF and must use the native ZIP-member transfer.
use super::*;

pub(crate) const ARCHIVE_URL: &str =
    "https://data.bris.ac.uk/datasets/s5hqmjcdj8yo2ibzi9b4ew3sn/N00E050-N10E060_FABDEM_V1-2.zip";
pub(crate) const ARCHIVE_BYTES: u64 = 21_627_700;
pub(crate) const ARCHIVE_SHA256: &str =
    "be348eecf2fb835792c1d446d5528a34956a8509c37004da9b72101c313cacfe";

pub(crate) struct ArchiveSelection {
    pub member: &'static str,
    pub archive_bytes: u64,
    pub archive_sha256: &'static str,
    pub member_bytes: u64,
    pub member_sha256: &'static str,
}
struct Tile {
    name: &'static str,
    bounds: [f64; 4],
    bytes: u64,
    sha256: &'static str,
}
const TILES: [Tile; 2] = [
    Tile {
        name: "N08E050_FABDEM_V1-2.tif",
        bounds: [
            49.99986111111111,
            8.000138888888891,
            50.99986111111111,
            9.00013888888889,
        ],
        bytes: 5_592_127,
        sha256: "6a8988122a922f0f11a059829c4ecfc5451364eca1b38708d02bf789668b7837",
    },
    Tile {
        name: "N09E050_FABDEM_V1-2.tif",
        bounds: [
            49.99986111111111,
            9.000138888888891,
            50.99986111111111,
            10.00013888888889,
        ],
        bytes: 16_035_307,
        sha256: "c8d2ffb9777dddabefa856636a072af81563eb9fed3bf3cfcfebfe95a9ed9928",
    },
];

pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "fabdem",
        "FABDEM V1-2 · terrain elevation",
        "elevation",
        "https://research-information.bris.ac.uk/en/datasets/fabdem-v1-2/",
    );
    d.description = "Forest And Buildings removed Copernicus DEM: modeled removal of tree and building height biases from Copernicus GLO-30. Reviewed scope is the N08E050 and N09E050 1° original tiles in one author-published 10° ZIP. No alternate Copernicus DEM, Fathom DEM, global claim or generated AOI raster is substituted.".into();
    d.version = "FABDEM V1-2 · January 2023 release · two reviewed original tiles".into();
    d.license =
        "CC BY-NC-SA 4.0 · non-commercial; commercial use requires a separate Fathom licence"
            .into();
    d.license_url = "https://creativecommons.org/licenses/by-nc-sa/4.0/".into();
    d.attribution = "Neal, J., Hawker, L., Uhe, P., Paulo, L., Sosa, J., Savage, J. and Sampson, C. (2023), FABDEM V1-2, University of Bristol, doi:10.5523/bris.s5hqmjcdj8yo2ibzi9b4ew3sn. Hawker et al. (2022), A 30 m global map of elevation with forests and buildings removed, Environmental Research Letters 17, 024016, doi:10.1088/1748-9326/ac4d4f. Retain licence, attribution, selected tile and release. The original publisher readme at https://data.bris.ac.uk/datasets/s5hqmjcdj8yo2ibzi9b4ew3sn/readme.txt also requires retention of its Copernicus WorldDEM-30 attribution and liability statements: DLR 2010–2014 / Airbus 2014–2018, provided under Copernicus by EU and ESA, all rights reserved; organisations managing Copernicus incur no liability for data use. Preserve those original statements with any external use. Commercial enquiries: fabdem@fathom.global.".into();
    d.nominal_resolution =
        "1 arc-second (approximately 30 m at equator); original 3600 × 3600 Point grid".into();
    d.unit = "metres".into();
    d.value_interpretation = "Original Float32 modeled terrain elevation in metres, scale 1 and offset 0; NoData -9999. Preserve zero and negative elevations as valid values. Original Point registration shifts the reported outer-edge footprint by half a cell; filenames are not exact bounding boxes. No datum transformation, height correction, clipping, reprojection, resampling or scalar mask inference is performed.".into();
    d.layers = vec![scaled(
        layer(
            "elevation",
            "Terrain elevation · original metres",
            "m",
            &d.value_interpretation,
        ),
        1.,
        0.,
        -9999.,
    )];
    d.default_layer = Some("elevation".into());
    d.warnings = vec![
        "Only two fully checked 1° members of N00E050-N10E060_FABDEM_V1-2.zip are eligible. The complete official ZIP is downloaded and preserved with its SHA-256; the selected whole TIFF is extracted byte-for-byte and has its own independent SHA-256. The ZIP is not falsely presented as a TIFF or as publisher STAC.".into(),
        "FABDEM is a derived terrain model with forest/building bias removal, not surveyed bare-earth truth or the original Copernicus surface DEM. Publication in 2023 is not a 2023 observation. No date selector applies.".into(),
        "The licence is CC BY-NC-SA 4.0. Non-commercial attribution and ShareAlike terms apply; commercial use requires separate permission. Some published bundles exceed 512 MiB and remain unsupported.".into(),
    ];
    d.limits = vec!["One complete 21,627,700-byte publisher ZIP and two eligible 3600 × 3600 Float32 members only; actual Point grid footprints determine selection. Original archive/member hashes, bytes and embedded NoData are retained. Existing 512 MiB transfer, safe ZIP-member, native decoding and preview budgets remain unchanged.".into()];
    vec![d]
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != "fabdem"
        || request.layer.as_deref() != Some("elevation")
        || request.year.is_some()
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the reviewed FABDEM V1-2 elevation originals; no date, country, region, season, alternate release or other DEM is substituted".into());
    }
    let sources: Vec<_> = TILES
        .iter()
        .filter(|t| {
            request.bounds[0] < t.bounds[2]
                && request.bounds[2] > t.bounds[0]
                && request.bounds[1] < t.bounds[3]
                && request.bounds[3] > t.bounds[1]
        })
        .map(|t| {
            let mut s = source(
                format!(
                    "FABDEM V1-2 {}",
                    t.name.trim_end_matches("_FABDEM_V1-2.tif")
                ),
                ARCHIVE_URL.into(),
                Some(t.bounds),
            );
            s.id = t.name.trim_end_matches(".tif").into();
            s
        })
        .collect();
    if sources.is_empty() {
        return Err("The selected area does not intersect either exact reviewed FABDEM original tile; no demonstration tile or generated subset is substituted".into());
    }
    Ok(sources)
}

/// Requires provenance to select an exact numerical member. A caller-supplied
/// ZIP URL, filename fragment or a matching package alone cannot pick a member.
pub(crate) fn archive_selection(proof: &SourceProvenance, url: &str) -> Result<ArchiveSelection> {
    let sources = resolve(&proof.request)?;
    let tile = TILES
        .iter()
        .find(|t| t.name.trim_end_matches(".tif") == proof.source_id)
        .ok_or("Unreviewed FABDEM ZIP member")?;
    if proof.dataset.id != "fabdem"
        || proof.layer.id != "elevation"
        || url != ARCHIVE_URL
        || proof.source_bounds != Some(tile.bounds)
        || !sources.iter().any(|s| s.id == proof.source_id)
    {
        return Err(
            "FABDEM archive/member recipe differs from its reviewed native provenance".into(),
        );
    }
    Ok(ArchiveSelection {
        member: tile.name,
        archive_bytes: ARCHIVE_BYTES,
        archive_sha256: ARCHIVE_SHA256,
        member_bytes: tile.bytes,
        member_sha256: tile.sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(bounds: [f64; 4]) -> ResolveRequest {
        ResolveRequest {
            dataset_id: "fabdem".into(),
            bounds,
            year: None,
            month: None,
            country: None,
            layer: Some("elevation".into()),
            region: None,
            season: None,
        }
    }
    #[test]
    fn actual_point_footprint_keeps_half_cell_shift_and_both_boundary_tiles() {
        let one = resolve(&request([50.1, 9.1, 50.2, 9.2])).unwrap();
        assert_eq!(one.len(), 1);
        assert_eq!(one[0].id, "N09E050_FABDEM_V1-2");
        assert_eq!(one[0].bounds, Some(TILES[1].bounds));
        let both = resolve(&request([50.1, 9., 50.2, 9.001])).unwrap();
        assert_eq!(both.len(), 2);
        assert!(both
            .iter()
            .all(|s| s.stac.as_ref().unwrap().url == ARCHIVE_URL));
        let d = definitions().pop().unwrap();
        assert_eq!(d.layers[0].nodata, Some(-9999.));
        assert!(d.license.contains("CC BY-NC-SA 4.0"));
        assert!(d.years.is_empty() && d.months.is_empty());
    }
    #[test]
    fn other_dem_time_or_area_fails_without_fallback() {
        for change in 0..7 {
            let mut r = request([50.1, 9.1, 50.2, 9.2]);
            match change {
                0 => r.layer = Some("surface".into()),
                1 => r.year = Some(2023),
                2 => r.month = Some(1),
                3 => r.country = Some("SOM".into()),
                4 => r.region = Some("N09E050".into()),
                5 => r.season = Some("annual".into()),
                _ => r.bounds = [116., 39., 117., 40.],
            }
            assert!(resolve(&r).is_err());
        }
    }

    #[test]
    #[ignore = "requires GEOD_CLIMATE_ORIGINAL_DIR containing the whole publisher ZIP"]
    fn whole_archive_members_keep_original_hashes_float32_zero_and_signed_metres() {
        use sha2::{Digest, Sha256};
        use std::{
            io::{Cursor, Read},
            path::PathBuf,
        };
        use tiff::decoder::{Decoder, DecodingResult};
        let path = PathBuf::from(std::env::var("GEOD_CLIMATE_ORIGINAL_DIR").unwrap())
            .join("N00E050-N10E060_FABDEM_V1-2.zip");
        let bytes = std::fs::read(path).unwrap();
        assert_eq!(bytes.len() as u64, ARCHIVE_BYTES);
        assert_eq!(format!("{:x}", Sha256::digest(&bytes)), ARCHIVE_SHA256);
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).unwrap();
        assert_eq!(zip.len(), 2);
        for (i, tile) in TILES.iter().enumerate() {
            let mut entry = zip.by_name(tile.name).unwrap();
            assert_eq!(entry.size(), tile.bytes);
            let mut bytes = Vec::new();
            entry.read_to_end(&mut bytes).unwrap();
            assert_eq!(format!("{:x}", Sha256::digest(&bytes)), tile.sha256);
            let mut decoder = Decoder::new(Cursor::new(bytes)).unwrap();
            assert_eq!(decoder.dimensions().unwrap(), (3600, 3600));
            let DecodingResult::F32(values) = decoder.read_image().unwrap() else {
                panic!("FABDEM original is no longer Float32")
            };
            assert_eq!(values.len(), 12_960_000);
            assert!(values.iter().all(|v| v.is_finite() && *v != -9999.));
            assert_eq!(
                values.iter().filter(|v| **v == 0.).count(),
                [9_521_361, 2_883_651][i]
            );
            assert_eq!(values.iter().filter(|v| **v < 0.).count(), [6, 71][i]);
            assert_eq!(values[1800 * 3600 + 1800], [0., 275.67_f32][i]);
        }
    }
}
