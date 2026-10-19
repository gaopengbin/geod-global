//! Finite, versioned publisher-item recipes. The documents in the reviewed
//! index are real publisher JSON; they are compared to fresh metadata and are
//! never used as a substitute for connecting or downloading from the source.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

pub(crate) const ADAPTER: &str = "deafrica-worldcereal-public-s3";
const BUCKET: &str = "s3://deafrica-input-datasets/";
const HTTPS: &str = "https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedItem {
    layer: String,
    url: String,
    document: Value,
}

static ITEMS: LazyLock<Vec<ReviewedItem>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("worldcereal-reviewed-items.json"))
        .expect("compile-time reviewed WorldCereal index")
});

fn season(layer: &str) -> Option<&'static str> {
    match layer {
        "temporarycrops" => Some("tc-annual"),
        "maize-main" | "maize-active" | "maize-irrigation" => Some("tc-maize-main"),
        "activecropland" | "wintercereals" | "wintercereals-irrigation" => Some("tc-wintercereals"),
        _ => None,
    }
}

pub(super) fn definition() -> Dataset {
    let mut dataset = base(
        "worldcereal",
        "ESA WorldCereal · Mauritius / Réunion · DE Africa archive",
        "agriculture",
        "https://docs.digitalearthafrica.org/en/latest/data_specs/ESA_World_Cereal_specs.html",
    );
    dataset.description = "Seven model-derived annual/seasonal crop themes from ESA WorldCereal, accessed through Digital Earth Africa's public regional copy. This reviewed release supports the actual standalone publisher items for Mauritius/Réunion (AEZ 9026), reference year 2021; it is not a global scene search or a guarantee of agricultural accuracy.".into();
    dataset.version = "WorldCereal v1.0.0 · DE Africa AEZ 9026 / 2021 reviewed item index".into();
    dataset.years = vec![2021];
    dataset.default_year = Some(2021);
    dataset.discovery = Some("stac-items".into());
    dataset.nominal_resolution = "10 m classification (approximately 1/12000 degree); confidence assets have independent publisher grids".into();
    dataset.unit = "original categorical codes; asset-specific confidence".into();
    dataset.attribution = "ESA WorldCereal consortium / VITO; contains modified Copernicus Sentinel data (2020–2021). Regional distribution: Digital Earth Africa, public deafrica-input-datasets archive. Cite the original item, AEZ, actual season, product version and access date.".into();
    dataset.value_interpretation = "Original categorical classification samples, not optical reflectance or observed crop yield. Annual temporary crops, crop-type classes, seasonal active cropland and seasonal active irrigation have different meanings. 254 is a valid no-crop category in seasonal products; it must not be treated as NoData. Embedded TIFF masks and each asset's independent grid are retained without reclassification or calibration.".into();
    dataset.regions = vec![DatasetRegion {
        id: "9026".into(),
        title: "Mauritius / Réunion · AEZ 9026".into(),
        bounds: [
            53.864516174824,
            -22.69539316166828,
            59.99509950815733,
            -18.981793161668282,
        ],
    }];
    // No default region: omitted region means actual AOI/index intersection,
    // never replacing a user's map with a demonstration area.
    dataset.seasons = vec![
        DatasetSeason {
            id: "tc-annual".into(),
            title: "Annual temporary-crop period · actual AEZ dates".into(),
        },
        DatasetSeason {
            id: "tc-maize-main".into(),
            title: "Main maize-season window · actual AEZ dates".into(),
        },
        DatasetSeason {
            id: "tc-wintercereals".into(),
            title: "Winter-cereals season window · actual AEZ dates".into(),
        },
    ];
    let definitions = [
        ("temporarycrops", "Annual temporary crops", "0 other land cover/use, 100 temporary crops. This annual crop mask is not a specific crop-type classification."),
        ("activecropland", "Active cropland · winter-cereals window", "0 inactive, 100 active, 254 no crop. Seasonal activity on the temporary-crop mask; this is not a winter-cereals-only crop-type class."),
        ("maize-main", "Maize crop type · main season", "Original main-season maize classification, with publisher codes and masks retained. This theme identifies crop type rather than all active cropland."),
        ("maize-active", "Active cropland · main maize-season window", "0 inactive, 100 active, 254 no crop. Activity on temporary cropland in the main maize-season window; not a maize-only classification."),
        ("maize-irrigation", "Active irrigation · main maize-season window", "Original active-irrigation classification on temporary cropland during the main maize-season window. It does not identify exclusively irrigated maize or irrigation infrastructure."),
        ("wintercereals", "Winter-cereals crop type", "Original seasonal winter-cereals classification, distinct from the general active-cropland theme. Preserve the publisher's categorical legend."),
        ("wintercereals-irrigation", "Active irrigation · winter-cereals window", "Original active-irrigation classification on temporary cropland during the winter-cereals season window. It does not identify exclusively irrigated winter cereals."),
    ];
    dataset.layers = definitions
        .into_iter()
        .map(|(id, title, meaning)| {
            let mut value = layer(id, title, "class code", meaning);
            value.seasons = vec![season(id).unwrap().into()];
            value
        })
        .collect();
    dataset.default_layer = Some("temporarycrops".into());
    dataset.warnings = vec![
        "Availability is limited to the seven reviewed AEZ 9026 / reference-year 2021 publisher items. An omitted region matches the current map area against their actual published bounding boxes; an area outside the reviewed region fails explicitly. No different AEZ, season, year or product is substituted.".into(),
        "Reference year 2021 does not mean a January–December observation interval. For this AEZ the annual period is 2020-11-09 to 2021-11-09, the main maize-season window is 2020-11-11 to 2021-05-08, and the winter-cereals window is 2021-04-03 to 2021-11-09. Seasonal activity and irrigation do not imply a crop-type-only mask.".into(),
        "Publisher STAC items contain original s3 asset URLs. Only the exact reviewed public bucket, regional endpoint and matching item assets receive a native HTTPS transport receipt; the archived item, asset metadata and original s3 href remain unchanged. No AWS account, temporary signature or private bucket is used.".into(),
        "The standalone items have no collection link, so collection metadata is not retrieved. CC BY 4.0 is a separate documented distribution/TIFF declaration, not an invented STAC collection license. Source-metadata completion does not mean that the collection declaration or valid-pixel coverage has been verified.".into(),
        "The complete regional original is downloaded without clipping. Publisher item bbox and original raster bounds can differ slightly (about 0.2 pixel in the annual example); bbox intersection is not exact geometric or valid-crop coverage. Confidence files are independent assets and must retain their own projection/shape/transform rather than inheriting the classification grid.".into(),
        "Classification 254 means no crop in relevant seasonal themes; only the embedded missing-data mask is used by native inspection. A numerical thumbnail is not a scientific accuracy assessment or evidence that any particular pixel contains crops.".into(),
        "The reviewed annual confidence original contains some raw values 101–253 although its TIFF legend describes confidence as 0–100. Original values are preserved without clipping, conversion to a percentage or inferred calibration. Category 254 remains valid no-crop data; 255 is used as NoData only through the original embedded TIFF declaration. Consult the publisher before scientific interpretation of values outside its legend.".into(),
    ];
    dataset.limits = vec![
        "Up to 8 reviewed regional items per selection; this release currently contains one AEZ and seven distinct themes, not seven satellite platforms or global coverage.".into(),
        "Each original file remains limited to 512 MiB. Tiled TIFF axes are limited to 131,072 pixels and 8 billion grid pixels; stripped TIFF axes remain limited to 65,536. Native decoding retains 128 MiB chunk, 32 MiB intermediate, 512 MiB preview-budget and 30-second review bounds. Other publisher AEZ files can exceed these limits and are not silently substituted.".into(),
    ];
    dataset
}

fn bbox(document: &Value) -> Result<[f64; 4]> {
    serde_json::from_value(document["bbox"].clone()).map_err(crate::io_error)
}

fn intersects(a: [f64; 4], b: [f64; 4]) -> bool {
    a[0] <= b[2] && a[2] >= b[0] && a[1] <= b[3] && a[3] >= b[1]
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    if request.year != Some(2021) || request.month.is_some() || request.country.is_some() {
        return Err("WorldCereal regional items use reference year 2021 and an AEZ season, not a month or country parameter".into());
    }
    let selected = request
        .layer
        .as_deref()
        .ok_or("Choose a WorldCereal thematic layer")?;
    let wanted_season = season(selected).ok_or("Choose a reviewed WorldCereal thematic layer")?;
    if request.season.as_deref() != Some(wanted_season) {
        return Err("The selected WorldCereal theme does not use that published AEZ season".into());
    }
    let candidates = ITEMS
        .iter()
        .filter(|item| item.layer == selected)
        .filter(|item| {
            request
                .region
                .as_deref()
                .is_none_or(|id| item.document["properties"]["odc:region_code"] == id)
        })
        .filter(|item| bbox(&item.document).is_ok_and(|bounds| intersects(bounds, request.bounds)))
        .collect::<Vec<_>>();
    if candidates.is_empty() {
        return Err("The map area does not intersect a reviewed WorldCereal 2021 regional item for this theme/season. Current supported distribution is Mauritius/Réunion (AEZ 9026); no alternate region or season is substituted".into());
    }
    if candidates.len() > MAX_TILES {
        return Err(
            "This selection intersects more than 8 reviewed regional items; narrow the map area"
                .into(),
        );
    }
    candidates
        .into_iter()
        .map(|item| {
            let title = format!("WorldCereal {} · AEZ 9026 · {wanted_season}", item.layer);
            Ok(DatasetSource {
                catalog_raster: None,
                id: item.document["id"].as_str().unwrap().into(),
                title: title.clone(),
                bounds: Some(bbox(&item.document)?),
                stac: Some(stac::ConnectRequest {
                    name: title,
                    url: item.url.clone(),
                    kind: "item".into(),
                }),
                wcs: None,
                coverage_id: None,
                search: None,
                region: Some("9026".into()),
                season: Some(wanted_season.into()),
                start_datetime: Some(
                    item.document["properties"]["start_datetime"]
                        .as_str()
                        .unwrap()
                        .into(),
                ),
                end_datetime: Some(
                    item.document["properties"]["end_datetime"]
                        .as_str()
                        .unwrap()
                        .into(),
                ),
            })
        })
        .collect()
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    if proof.schema_version != 2 || proof.dataset.id != "worldcereal" {
        return Err("Unsupported reviewed publisher-item provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let source = plan
        .sources
        .iter()
        .find(|s| s.id == proof.source_id && s.stac.as_ref().is_some_and(|r| r.url == url))
        .ok_or("Reviewed publisher item does not match its fixed regional recipe")?;
    if super::provenance(&plan, source) != *proof {
        return Err("Reviewed publisher-item declaration differs from its fixed preset".into());
    }
    let reviewed = ITEMS
        .iter()
        .find(|item| item.url == url)
        .ok_or("Publisher item is outside the reviewed regional index")?;
    if raw.is_some_and(|raw| *raw != reviewed.document) {
        return Err("Reviewed WorldCereal publisher document changed; source identity, season, bounds, version or original asset declarations differ from the pinned index".into());
    }
    Ok(())
}

pub(crate) fn transports(
    proof: &SourceProvenance,
    url: &str,
    raw: &Value,
) -> Result<Vec<stac::AssetTransport>> {
    validate(proof, url, Some(raw))?;
    let mut result = Vec::new();
    for (key, asset) in raw["assets"]
        .as_object()
        .ok_or("Publisher item assets are missing")?
    {
        let original = asset["href"]
            .as_str()
            .ok_or("Publisher asset href is missing")?;
        let path = original
            .strip_prefix(BUCKET)
            .filter(|path| {
                path.starts_with("esa_worldcereal/")
                    && !path.contains('%')
                    && !path.contains('?')
                    && !path.contains('#')
                    && path
                        .split('/')
                        .all(|part| !part.is_empty() && part != "." && part != "..")
            })
            .ok_or("Only the exact reviewed public WorldCereal S3 asset may be transported")?;
        let stem = url
            .strip_prefix(HTTPS)
            .and_then(|p| p.strip_suffix(".stac-item.json"))
            .ok_or("Reviewed WorldCereal item URL does not match the public regional endpoint")?;
        if !matches!(key.as_str(), "classification" | "confidence")
            || path != format!("{stem}_{key}.tif")
        {
            return Err(
                "WorldCereal transport must match the reviewed item's exact asset filename".into(),
            );
        }
        result.push(stac::AssetTransport {
            asset_key: key.clone(),
            original_href: original.into(),
            href: format!("{HTTPS}{path}"),
            adapter: ADAPTER.into(),
        });
    }
    Ok(result)
}

pub(crate) fn self_url(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<String> {
    validate(proof, url, Some(raw))?;
    let self_href = format!(
        "{BUCKET}{}",
        url.strip_prefix(HTTPS)
            .ok_or("Invalid reviewed item endpoint")?
    );
    let links = raw["links"]
        .as_array()
        .ok_or("Publisher item has no links")?;
    if links.len() != 1 || links[0]["rel"] != "self" || links[0]["href"] != self_href {
        return Err("Reviewed WorldCereal self link changed or is ambiguous".into());
    }
    Ok(url.into())
}

#[cfg(test)]
pub(crate) fn reviewed_document(layer: &str) -> (String, Value) {
    let item = ITEMS.iter().find(|item| item.layer == layer).unwrap();
    (item.url.clone(), item.document.clone())
}
