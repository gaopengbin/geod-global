//! One additional real Earth Search collection. This finite adapter does not
//! claim support or completion of the entire Earth Search family entry.
use super::*;
use chrono::NaiveDate;
const API: &str = "https://earth-search.aws.element84.com/v1/";
const COLLECTION: &str = "sentinel-2-c1-l2a";

pub(super) fn definitions() -> Vec<Dataset> {
    let mut d = base(
        "earth-search-more",
        "Earth Search · Sentinel-2 Collection 1 L2A",
        "imagery",
        "https://earth-search.aws.element84.com/v1/collections/sentinel-2-c1-l2a",
    );
    d.description="A finite additional Earth Search Collection 1 Level-2A catalog. Search real publisher STAC observations by the shared map region and UTC month, preserve real items and collection metadata, and download the selected complete original numerical TIFF assets. This does not establish that all nine Earth Search collections or all family formats are supported.".into();
    d.version = "sentinel-2-c1-l2a · publisher Collection 1 archive".into();
    d.discovery = Some("stac-api".into());
    d.years = (2015..=2026).collect();
    d.default_year = Some(2025);
    d.months = (1..=12).collect();
    d.default_month = Some(9);
    d.license="Publisher STAC collection declares proprietary and links the Copernicus Sentinel Data Legal Notice; preserve that declaration and apply the linked notice".into();
    d.license_url =
        "https://sentinel.esa.int/documents/247904/690755/Sentinel_Data_Legal_Notice".into();
    d.attribution="ESA/Copernicus Sentinel-2; processing and hosting credits from the original collection: Sinergise, AWS and Element 84. Contains modified Copernicus Sentinel data for the selected observation year. Preserve original publisher item, asset, UTC observation time, processing baseline, calibration declarations and legal notice.".into();
    d.nominal_resolution =
        "Band-specific original 10, 20 or 60 m grids; independent SCL and auxiliary layers".into();
    d.unit = "band-specific raw numerical samples".into();
    d.value_interpretation="Level-2A surface-reflectance band assets retain their original stored UInt16 samples. Publisher raster:bands scale, offset and NoData are archived separately from the TIFF's embedded declarations; the verified September 2025 B01 item declares DN × 0.0001 − 0.1 and NoData 0. Do not generalize that declaration to every baseline or asset. SCL is categorical and is not reflectance or an automatic applied quality mask; B01 is a coastal/aerosol spectral band, not an aerosol-optical-depth product.".into();
    d.layers=vec![layer("c1-l2a","Sentinel-2 Collection 1 · L2A observations","band-specific","Real sentinel-2-c1-l2a observations in the selected UTC calendar month. Preserve complete individual numerical band files and the publisher metadata. No L1C JP2, legacy collection, different sensor, date or generated region export is substituted.")];
    d.default_layer = Some("c1-l2a".into());
    d.warnings=vec![
        "This adapter covers only the additional sentinel-2-c1-l2a collection. The original earth-search-more family entry remains partially implemented; this one collection is not counted as complete family integration.".into(),
        "The original collection currently declares license=proprietary and links the Copernicus Sentinel legal notice. The metadata document is retained unchanged; this adapter does not rewrite its licence field or equate anonymous file access with unrestricted reuse.".into(),
        "Candidate years/months are search filters, not guaranteed scene availability, cloud-free coverage or valid pixels. Empty searches remain empty; unsupported assets fail explicitly. The separate Sentinel-2 L1C collection currently advertises S3 JP2 originals and is not silently replaced by this TIFF collection.".into(),
        "Raw numerical previews and pixel values are not automatic reflectance calibration, cloud masking or scientific validation. Preserve each original asset's band identity, NoData and baseline-specific calibration. Complete files are saved without AOI clipping or resampling.".into(),
    ];
    d.limits=vec!["Native STAC pages are bounded to 20 results, with an explicit same-query next cursor. Existing 512 MiB full-asset, TIFF grid/axis/chunk/memory and preview deadlines remain unchanged. This finite Collection 1 adapter does not certify all Earth Search collections, observations or assets.".into()];
    vec![d]
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    let year = request.year.ok_or("Choose an observation year")?;
    let month = request.month.ok_or("Choose an observation month")?;
    if request.dataset_id != "earth-search-more"
        || request.layer.as_deref() != Some("c1-l2a")
        || !(2015..=2026).contains(&year)
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose the additional Earth Search Collection 1 L2A collection and a supported UTC observation month; no other family collection, product or region is substituted".into());
    }
    let start = NaiveDate::from_ymd_opt(i32::from(year), u32::from(month), 1)
        .ok_or("Choose a valid UTC observation month")?;
    let (next_year, next_month) = if month == 12 {
        (year + 1, 1)
    } else {
        (year, month + 1)
    };
    let end = NaiveDate::from_ymd_opt(i32::from(next_year), u32::from(next_month), 1)
        .and_then(|v| v.pred_opt())
        .ok_or("Choose a valid UTC observation month")?;
    Ok(vec![DatasetSource {
        id: COLLECTION.into(),
        title: format!("Earth Search Collection 1 L2A · observations {year}-{month:02}"),
        bounds: None,
        stac: Some(stac::ConnectRequest {
            name: "Element 84 Earth Search · Collection 1 L2A".into(),
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
    fn request() -> ResolveRequest {
        ResolveRequest {
            dataset_id: "earth-search-more".into(),
            bounds: [116.3, 39.8, 116.5, 40.],
            year: Some(2025),
            month: Some(9),
            country: None,
            layer: Some("c1-l2a".into()),
            region: None,
            season: None,
        }
    }
    #[test]
    fn additional_collection_is_exact_and_preserves_month_area_and_partial_family_boundary() {
        let s = resolve(&request()).unwrap().remove(0);
        assert_eq!(s.id, COLLECTION);
        assert_eq!(s.stac.unwrap().url, API);
        let q = s.search.unwrap();
        assert_eq!(q.collection_id, COLLECTION);
        assert_eq!(
            q.datetime,
            "2025-09-01T00:00:00Z/2025-09-30T23:59:59.999999Z"
        );
        assert_eq!(q.bounds, request().bounds);
        assert_eq!(q.limit, 20);
        let d = definitions().remove(0);
        assert!(d
            .warnings
            .iter()
            .any(|v| v.contains("partially implemented")));
        assert!(d.license.contains("proprietary"));
    }
    #[test]
    fn leap_month_is_complete_and_wrong_family_layer_time_or_region_never_falls_back() {
        let mut r = request();
        r.year = Some(2024);
        r.month = Some(2);
        assert!(resolve(&r).unwrap()[0]
            .search
            .as_ref()
            .unwrap()
            .datetime
            .ends_with("2024-02-29T23:59:59.999999Z"));
        for change in 0..7 {
            let mut r = request();
            match change {
                0 => r.layer = Some("l1c".into()),
                1 => r.month = Some(13),
                2 => r.year = Some(2027),
                3 => r.country = Some("CHN".into()),
                4 => r.region = Some("50TMK".into()),
                5 => r.season = Some("monthly".into()),
                _ => r.dataset_id = "sentinel-2-l2a".into(),
            };
            assert!(resolve(&r).is_err());
        }
    }
}
