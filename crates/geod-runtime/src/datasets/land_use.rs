//! Fixed HILDA+ numerical archive. This is an explicit direct-file connector,
//! not a correction or mirror of the publisher's inconsistent STAC extent.
use super::*;

const HILDA_BOUNDS: [f64; 4] = [-180., -61.98833333333333, 180., 87.37];

pub(super) fn hilda() -> Dataset {
    let mut dataset = base(
        "hilda-land-use",
        "HILDA+ historical land use",
        "land-cover",
        "https://doi.org/10.1594/PANGAEA.921846",
    );
    dataset.description = "Annual historical land-use/land-cover reconstruction, distributed by OpenLandMap as one original quasi-global numerical GeoTIFF for each reference year 1960–2019. This is a modeled historical product, not an optical satellite scene.".into();
    dataset.version = "HILDA+ 1960–2019; OpenLandMap v1.0 EPSG:4326 distribution".into();
    dataset.years = (1960..=2019).collect();
    dataset.default_year = Some(2019);
    dataset.nominal_resolution =
        "0.00833333333333333 degree grid (30 arc-seconds; approximately 1 km)".into();
    dataset.unit = "land-use/land-cover class code".into();
    dataset.value_interpretation = "Original UInt8 HILDA+ class codes, not category ordinal numbers: 11 urban, 22 cropland, 33 pasture/rangeland, 44 forest, 55 unmanaged grass/shrubland, 66 sparse/no vegetation, 77 water; 00 ocean and 99 publisher-declared no data. Reference year is not an acquisition date. The inspected OpenLandMap TIFF has no embedded NoData tag; 99 is a publisher scientific declaration and is not automatically installed as a TIFF mask. No codes are relabeled or discarded by this connector.".into();
    dataset.attribution = "Winkler, Karina; Fuchs, Richard; Rounsevell, Mark; Herold, Martin (2020), HILDA+ Global Land Use Change between 1960 and 2019, PANGAEA, doi:10.1594/PANGAEA.921846; Winkler et al. (2021), Nature Communications 12, 2501, doi:10.1038/s41467-021-22702-2. GeoTIFF distribution by OpenLandMap; include reference year, release and access date.".into();
    dataset.layers = vec![layer("land-use", "Historical land-use class codes", "class code", "Annual reconstructed raw codes: 11 urban, 22 cropland, 33 pasture/rangeland, 44 forest, 55 unmanaged grass/shrubland, 66 sparse/no vegetation, 77 water, 00 ocean, 99 publisher-declared no data. These codes differ from the six generic categories enumerated 1–6 in the collection abstract. The original UInt8 codes remain unchanged; not reflectance or a forecast. No embedded NoData value or automatic mask is invented.")];
    dataset.default_layer = Some("land-use".into());
    dataset.warnings = vec!["The entire published quasi-global annual file is downloaded, not clipped to the selected map area. The inspected 2019 grid has 43,200 × 17,923 area samples, EPSG:4326, and bounds approximately 180°W–180°E / 61.988333°S–87.37°N; the selected area must lie within that distribution extent.".into(), "The official OpenLandMap STAC metadata reports inconsistent longitude/latitude extents for this archive. This preset uses an explicitly reviewed direct numerical file recipe and its embedded georeferencing; it does not rewrite or silently repair publisher STAC documents. The URL retains the publisher's literal espg filename spelling.".into(), "The source author's HILDA+ class legend uses 11/22/33/44/55/66/77/99/00. The exact OpenLandMap 2019 SLD/QML style labels 77 as sparse/no vegetation and omits 66 and 99, conflicting with the author legend where 66 is sparse/no vegetation and 77 is water. The connector discloses this style conflict, preserves original numerical codes and does not apply that style or reinterpret samples as labels 1–6. Author legend: https://landchange.imk-ifu.kit.edu/hilda ; exact source style: https://s3.openlandmap.org/arco/land.use.land.cover_hilda.plus.sld .".into(), "The original file has no embedded TIFF NoData tag. The author declares code 99 as no data and code 00 as ocean; a transparent style entry is not an embedded TIFF mask. Generic native inspection preserves raw UInt8 samples and does not manufacture a mask; consult product documentation before analytical masking. A categorical numerical preview is not an optical image or validation of classification accuracy.".into(), "HILDA+ is a harmonized historical reconstruction from multiple sources. Annual class changes include model uncertainty and are not necessarily direct observed land transitions. The reference period ends in 2019; no newer year or different product is substituted.".into()];
    dataset.limits = vec!["One complete annual file per selection, with supported reference years fixed to 1960–2019 and the inspected quasi-global spatial domain. No individual scene or monthly filter exists.".into(), "Each original file uses the runtime's 512 MiB transfer limit. Only tiled TIFFs support dimensions up to 131,072 pixels per side and at most 8 billion grid pixels; stripped TIFFs remain limited to 65,536 pixels per side. Decoding is bounded by 128 MiB per chunk, 32 MiB per intermediate buffer, 512 MiB preview memory and a 30-second review deadline.".into()];
    dataset
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    if request.bounds[0] < HILDA_BOUNDS[0]
        || request.bounds[1] < HILDA_BOUNDS[1]
        || request.bounds[2] > HILDA_BOUNDS[2]
        || request.bounds[3] > HILDA_BOUNDS[3]
    {
        return Err("Choose an area entirely within the HILDA+ distribution's inspected longitude/latitude extent".into());
    }
    let year = request.year.ok_or("Choose a historical reference year")?;
    if !(1960..=2019).contains(&year)
        || request.month.is_some()
        || request.layer.as_deref() != Some("land-use")
    {
        return Err(
            "Choose a HILDA+ annual reference year from 1960–2019 without a month parameter".into(),
        );
    }
    Ok(vec![source(format!("HILDA+ historical land use · {year}"), format!("https://s3.openlandmap.org/arco/land.use.land.cover_hilda.plus_c_1km_s_{year}0101_{year}1231_go_espg.4326_v1.0.tif"), Some(HILDA_BOUNDS))])
}
