//! Original AMAZÔNIA-1 WFI L4 DN scenes from the official INPE STAC API.
//! Conflicting publisher calibration/NoData metadata is retained; this recipe
//! does not convert the original DN into surface reflectance or replace L4 DN.
use super::*;
use chrono::NaiveDate;

const API: &str = "https://data.inpe.br/bdc/stac/v1/";
const COLLECTION: &str = "AMZ1-WFI-L4-DN-1";

pub(super) fn definition() -> Dataset {
    let mut dataset = base(
        "amazonia-wfi-dn",
        "AMAZÔNIA-1 WFI L4 original DN · 64 m",
        "imagery",
        "https://data.inpe.br/dados/amazonia-1/",
    );
    dataset.description = "Search the official INPE AMAZÔNIA-1 WFI Level-4 orthorectified digital-number scene collection by the unchanged map area and UTC acquisition month. Choose complete original blue, green, red or near-infrared 64 m band files; no reflectance calibration, RGB substitution or pansharpening is applied.".into();
    dataset.version = "AMZ1-WFI-L4-DN-1 version 1".into();
    dataset.discovery = Some("stac-api".into());
    dataset.years = (2021..=2026).collect();
    dataset.default_year = Some(2026);
    dataset.months = (1..=12).collect();
    dataset.default_month = Some(10);
    dataset.nominal_resolution = "64 m original WFI multispectral grid".into();
    dataset.unit = "raw digital number (DN)".into();
    dataset.value_interpretation = "L4 DN is an orthorectified numerical product derived from L2 DN, distinct from the L4 surface-reflectance COG collection. BAND1 blue, BAND2 green, BAND3 red and BAND4 near infrared are separate 64 m originals. Official sensor documentation describes 10-bit acquisition. The independently decoded 2026-10-12 BAND1 is signed Int16 with raw values 0–1023, embedded NoData 0, scale 1 and offset 0. STAC instead declares NoData -9999 and scale 0.0001 with maximum 10000; preserve this conflict without automatically scaling DN or assigning physical reflectance units.".into();
    dataset.license = "CC BY 4.0".into();
    dataset.license_url = CC_BY.into();
    dataset.attribution = "National Institute for Space Research (INPE), Brazil Data Cube, AMAZÔNIA-1 WFI. The official AMZ1-WFI-L4-DN-1 collection declares Creative-Commons-Attribution-4.0-International. Retain scene identity, collection, original asset URL, acquisition date, processing level and access date.".into();
    dataset.layers = vec![layer(
        "wfi-ms",
        "WFI L4 multispectral original DN · 64 m",
        "raw DN",
        "Fixed official AMZ1-WFI-L4-DN-1 collection. BAND1 blue, BAND2 green, BAND3 red and BAND4 near infrared remain independent complete original files. L2 DN, L4-SR, CMASK, RGB thumbnails and a different sensor are not substituted. Embedded original samples and NoData are inspected independently of conflicting publisher calibration declarations.",
    )];
    dataset.default_layer = Some("wfi-ms".into());
    dataset.warnings = vec![
        "The reviewed archive begins on 2021-03-17 and currently contains received scenes through 2026. Collection extent is not proof of continuous or global coverage. The original AOI, sensor, level and UTC month are retained; an empty result is valid and no demonstration area is inserted.".into(),
        "Year/month filters UTC acquisition time rather than creation time or a monthly mosaic. March 2021 is the first candidate month; years stop at 2026. Later candidate months may have no published observations. Missing cloud cover remains unknown, not zero percent.".into(),
        "This DN collection's STAC NoData -9999 / scale 0.0001 / maximum 10000 differs from the inspected original NoData 0 / scale 1 / raw maximum 1023 and the official L4 DN identity. Neither declaration is silently rewritten. The original TIFF's own tags govern raw inspection; physical calibration requires separate scientific validation.".into(),
        "The sampled URL directory contains LCC, but its actual TIFF and original XML identify UTM WGS84, EPSG:32718. A directory name is not a CRS declaration. Each original file retains its own grid; no reprojection, clipping or resampling is applied.".into(),
        "The sampled original is a stripped Deflate TIFF without overviews, not a COG. A full original transfer encountered premature EOF and some 4 MiB ranges timed out during independent verification. Bounded smaller exact ranges recovered the same publisher-checksummed original; useful online preview and first-attempt transfers are not guaranteed on every network.".into(),
    ];
    dataset.limits = vec![
        "The first real STAC page contains at most 20 scenes. Additional pages retain the same area, collection and UTC interval. Footprints and matching-item counts do not establish valid-pixel or cloud-free coverage.".into(),
        "Each complete original remains limited to 512 MiB. Stripped TIFFs are limited to 65,536 pixels per side and at most 8 billion grid pixels; native inspection retains 128 MiB per chunk, 32 MiB per intermediate buffer, 512 MiB preview memory and a 30-second review deadline. Source EOF, timeout and unsupported originals fail explicitly without replacement products.".into(),
    ];
    dataset
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    if request.dataset_id != "amazonia-wfi-dn"
        || request.layer.as_deref() != Some("wfi-ms")
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the reviewed AMAZÔNIA-1 WFI L4 DN collection without country, named region or season parameters".into());
    }
    let year = request
        .year
        .ok_or("Choose an AMAZÔNIA-1 acquisition year")?;
    let month = request
        .month
        .ok_or("Choose an AMAZÔNIA-1 acquisition month")?;
    if !(2021..=2026).contains(&year) || (year == 2021 && month < 3) {
        return Err("This AMAZÔNIA-1 DN collection supports candidate acquisition months from March 2021 through 2026".into());
    }
    let start = NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), 1)
        .ok_or("Choose a valid AMAZÔNIA-1 acquisition month")?;
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = NaiveDate::from_ymd_opt(i32::from(next_year), u32::from(next_month), 1)
        .and_then(|next| next.pred_opt())
        .ok_or("Choose a valid AMAZÔNIA-1 acquisition month")?;
    Ok(vec![DatasetSource {
        id: COLLECTION.into(),
        title: format!("INPE AMAZÔNIA-1 WFI L4 DN · acquisitions {year}-{month:02}"),
        bounds: None,
        stac: Some(stac::ConnectRequest {
            name: "INPE AMAZÔNIA-1 WFI L4 DN catalog".into(),
            url: API.into(),
            kind: "api".into(),
        }),
        wcs: None,
        coverage_id: None,
        search: Some(DatasetSearch {
            collection_id: COLLECTION.into(),
            datetime: format!("{start}T00:00:00Z/{end}T23:59:59.999999Z"),
            bounds: request.bounds,
            limit: 20,
        }),
        region: None,
        season: None,
        start_datetime: None,
        end_datetime: None,
        catalog_raster: None,
    }])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(year: u16, month: u8) -> ResolveRequest {
        ResolveRequest {
            dataset_id: "amazonia-wfi-dn".into(),
            bounds: [116., 39., 117., 40.],
            year: Some(year),
            month: Some(month),
            layer: Some("wfi-ms".into()),
            country: None,
            region: None,
            season: None,
        }
    }

    #[test]
    fn fixes_dn_collection_preserves_aoi_and_uses_leap_month_utc() {
        let request = request(2024, 2);
        let sources = resolve(&request).unwrap();
        assert_eq!(sources.len(), 1);
        assert_eq!(sources[0].stac.as_ref().unwrap().url, API);
        let search = sources[0].search.as_ref().unwrap();
        assert_eq!(search.collection_id, COLLECTION);
        assert_eq!(search.bounds, request.bounds);
        assert_eq!(
            search.datetime,
            "2024-02-01T00:00:00Z/2024-02-29T23:59:59.999999Z"
        );
        assert_eq!(search.limit, 20);
        assert!(sources[0].catalog_raster.is_none());
    }

    #[test]
    fn december_uses_next_year_only_to_find_the_utc_month_end() {
        let sources = resolve(&request(2026, 12)).unwrap();
        assert_eq!(
            sources[0].search.as_ref().unwrap().datetime,
            "2026-12-01T00:00:00Z/2026-12-31T23:59:59.999999Z"
        );
    }

    #[test]
    fn rejects_pre_archive_dates_other_products_and_invalid_months() {
        for (year, month) in [(2020, 12), (2021, 2), (2027, 1), (2026, 0), (2026, 13)] {
            assert!(resolve(&request(year, month)).is_err());
        }
        assert!(resolve(&request(2021, 3)).is_ok());
        let mut wrong = request(2026, 10);
        wrong.layer = Some("surface-reflectance".into());
        assert!(resolve(&wrong).is_err());
        wrong.layer = Some("wfi-ms".into());
        wrong.dataset_id = "cbers-wpm".into();
        assert!(resolve(&wrong).is_err());
    }

    #[test]
    fn rejects_region_country_or_season_and_keeps_raw_science_unscaled() {
        let mut wrong = request(2026, 10);
        wrong.country = Some("BRA".into());
        assert!(resolve(&wrong).is_err());
        wrong.country = None;
        wrong.region = Some("demo".into());
        assert!(resolve(&wrong).is_err());
        wrong.region = None;
        wrong.season = Some("summer".into());
        assert!(resolve(&wrong).is_err());
        let dataset = definition();
        assert_eq!(dataset.license, "CC BY 4.0");
        assert!(dataset.layers[0].scale.is_none());
        assert!(dataset.layers[0].nodata.is_none());
        assert!(dataset.value_interpretation.contains("NoData -9999"));
        assert!(dataset.value_interpretation.contains("embedded NoData 0"));
    }
}
