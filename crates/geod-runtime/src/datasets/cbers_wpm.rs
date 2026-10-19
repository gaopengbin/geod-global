//! Orthorectified CBERS-4A WPM raw-DN scenes from the official INPE API.
//! This recipe does not substitute surface reflectance, L2 scenes, PCA RGB or
//! pansharpened products. Publisher asset metadata remains unchanged.
use super::*;
use chrono::NaiveDate;

const API: &str = "https://data.inpe.br/bdc/stac/v1/";
const COLLECTION: &str = "CB4A-WPM-L4-DN-1";

pub(super) fn definition() -> Dataset {
    let mut dataset = base(
        "cbers-wpm",
        "CBERS-4A WPM L4 multispectral original DN · 8 m",
        "imagery",
        "https://data.inpe.br/dados/cbers-4a/",
    );
    dataset.description = "Search the official INPE CBERS-4A WPM Level-4 orthorectified raw-DN scene collection by map area and UTC acquisition month. Select original 8 m multispectral band files; neither reflectance calibration nor pansharpening is applied.".into();
    dataset.version = "CB4A-WPM-L4-DN-1 version 1".into();
    dataset.discovery = Some("stac-api".into());
    dataset.years = (2019..=2026).collect();
    dataset.default_year = Some(2026);
    dataset.months = (1..=12).collect();
    dataset.default_month = Some(10);
    dataset.nominal_resolution =
        "8 m multispectral original grid; separate 2 m PAN files are not a pansharpened output"
            .into();
    dataset.unit = "raw digital number (DN)".into();
    dataset.value_interpretation = "L4 is an orthorectified raw-DN product, not surface reflectance. BAND1 blue, BAND2 green, BAND3 red and BAND4 near infrared are 8 m originals. The publisher describes 10-bit acquisition but its STAC eo:bands data_type says uint8; the inspected 2026-10-11 BAND1 TIFF is signed Int16 with embedded NoData 0 and scale 1. Preserve the original file and both declarations; do not force an 8-bit conversion or assign physical reflectance units.".into();
    dataset.license = "CC BY 4.0".into();
    dataset.license_url = CC_BY.into();
    dataset.attribution = "INPE / Brazil Data Cube, China-Brazil Earth Resources Satellite (CBERS) program. The selected official INPE collection declares Creative-Commons-Attribution-4.0-International; retain the scene identity, original asset URL, acquisition date, processing level and access date.".into();
    dataset.layers = vec![layer(
        "wpm-ms",
        "WPM L4 multispectral original DN · 8 m",
        "raw DN",
        "Fixed official CB4A-WPM-L4-DN-1 collection. BAND1 blue, BAND2 green, BAND3 red and BAND4 near infrared remain separate complete original 8 m files. BAND0 is a distinct 2 m panchromatic original; oversized assets cannot be selected within the 512 MiB limit. No L2, RGB quicklook, PCA fusion or surface-reflectance product is substituted.",
    )];
    dataset.default_layer = Some("wpm-ms".into());
    dataset.warnings = vec![
        "The reviewed collection begins on 2019-12-29 and contains received scenes over parts of South America. Its extent rectangle does not establish continuous or global coverage. An empty query is a valid result; the requested area, month, level and sensor are never replaced.".into(),
        "Year/month filters UTC acquisition time, not publication time or a monthly mosaic. Candidate months can have no published scenes; months before December 2019 are outside this collection. Reviewed candidate years stop at 2026.".into(),
        "Downloads retain the complete selected original band file rather than clipping to the map area. The sampled BAND1 is a stripped Deflate TIFF, not a COG; independent raw decoding does not prove online browser-map acceptance for every file.".into(),
        "BAND0 PAN can exceed 512 MiB: the sampled 2026-10-11 PAN is 2,639,842,927 bytes. Preserve the real bdc:size declaration and reject oversized selection; this connector does not promise a complete 2 m PAN or pansharpening workflow.".into(),
        "The direct INPE collection's CC BY 4.0 notice applies to this route. A separate AWS distribution declares a different license and is not silently used as a mirror.".into(),
    ];
    dataset.limits = vec![
        "The first real STAC page contains at most 20 scenes. More pages require the same area, collection and UTC interval. A scene footprint is not proof of valid pixels or cloud-free coverage; a missing cloud-cover field is not zero cloud cover.".into(),
        "Each original asset is limited to 512 MiB. Stripped TIFFs remain limited to 65,536 pixels per side and at most 8 billion grid pixels; decoding is bounded by 128 MiB per chunk, 32 MiB per intermediate buffer, 512 MiB preview memory and a 30-second review deadline. Unsupported originals fail explicitly.".into(),
    ];
    dataset
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    if request.dataset_id != "cbers-wpm"
        || request.layer.as_deref() != Some("wpm-ms")
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the reviewed WPM L4 multispectral raw-DN collection without country, region or season parameters".into());
    }
    let year = request.year.ok_or("Choose a WPM acquisition year")?;
    let month = request.month.ok_or("Choose a WPM acquisition month")?;
    if !(2019..=2026).contains(&year) || (year == 2019 && month < 12) {
        return Err("This WPM collection supports candidate acquisition months from December 2019 through 2026".into());
    }
    let start = NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), 1)
        .ok_or("Choose a valid WPM acquisition month")?;
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = NaiveDate::from_ymd_opt(i32::from(next_year), u32::from(next_month), 1)
        .and_then(|value| value.pred_opt())
        .ok_or("Choose a valid WPM acquisition month")?;
    Ok(vec![DatasetSource {
        id: COLLECTION.into(),
        title: format!("INPE WPM L4 multispectral DN · acquisitions {year}-{month:02}"),
        bounds: None,
        stac: Some(stac::ConnectRequest {
            name: "INPE WPM L4 scene catalog".into(),
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
            dataset_id: "cbers-wpm".into(),
            bounds: [116., 39., 117., 40.],
            year: Some(year),
            month: Some(month),
            layer: Some("wpm-ms".into()),
            country: None,
            region: None,
            season: None,
        }
    }

    #[test]
    fn fixes_l4_dn_identity_and_preserves_aoi_in_a_leap_month() {
        let request = request(2024, 2);
        let sources = resolve(&request).unwrap();
        assert_eq!(sources.len(), 1);
        let source = &sources[0];
        assert_eq!(source.stac.as_ref().unwrap().url, API);
        let search = source.search.as_ref().unwrap();
        assert_eq!(search.collection_id, COLLECTION);
        assert_eq!(search.bounds, request.bounds);
        assert_eq!(
            search.datetime,
            "2024-02-01T00:00:00Z/2024-02-29T23:59:59.999999Z"
        );
        assert_eq!(search.limit, 20);
        assert!(source.catalog_raster.is_none());
    }

    #[test]
    fn rejects_other_products_invalid_dates_and_pre_archive_months() {
        for (year, month) in [(2018, 12), (2019, 11), (2027, 1), (2026, 0), (2026, 13)] {
            assert!(resolve(&request(year, month)).is_err());
        }
        assert!(resolve(&request(2019, 12)).is_ok());
        let mut wrong = request(2026, 10);
        wrong.layer = Some("cbers4a-mux".into());
        assert!(resolve(&wrong).is_err());
        wrong.layer = Some("wpm-ms".into());
        wrong.dataset_id = "cbers-sr".into();
        assert!(resolve(&wrong).is_err());
    }
}
