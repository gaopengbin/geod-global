//! Reviewed CGLS land-cover-family items. Publisher JSON retains its erroneous
//! `gls/` asset paths; one fixed product/layer transport receipt identifies the
//! actual public `cgls_landcover/` distribution without editing those documents.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;

pub(crate) const ADAPTER: &str = "deafrica-cgls-reviewed-distribution-path";
const HTTPS: &str = "https://deafrica-input-datasets.s3.af-south-1.amazonaws.com/";
const BUCKET: &str = "s3://deafrica-input-datasets/";
const LAYER: &str = "permanentwater";
const ASSET: &str = "permanentwater_cover_fraction";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ReviewedItem {
    year: u16,
    url: String,
    document: Value,
}

static ITEMS: LazyLock<Vec<ReviewedItem>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("cgls-reviewed-items.json"))
        .expect("compile-time reviewed CGLS publisher-item index")
});

pub(super) fn definition() -> Dataset {
    let mut dataset = base(
        "cgls-landcover",
        "Copernicus Global Land Service land cover · DE Africa",
        "land-cover",
        "https://docs.digitalearthafrica.org/en/latest/data_specs/CGLS_LULC_specs.html",
    );
    dataset.description = "Collection 3 Copernicus Global Land Service land-cover products from the public Digital Earth Africa distribution. This reviewed connector offers the permanent-water cover-fraction layer for reference years 2015–2019; it does not claim that all land-cover classifications, fractions or global releases have been verified.".into();
    dataset.version = "CGLS Collection 3 · DE Africa 2015–2019 reviewed publisher items; tested 2019-nrt original v3.0.1".into();
    dataset.years = (2015..=2019).collect();
    dataset.default_year = Some(2019);
    dataset.discovery = Some("stac-items".into());
    dataset.nominal_resolution =
        "approximately 100 m; actual 0.0009920634920634888 degree grid".into();
    dataset.unit = "permanent-water cover fraction, percent".into();
    dataset.attribution = "Contains the CGLS Land Cover 100m product courtesy of the Copernicus Global Land Service / European Commission, based on PROBA-V imagery from BELSPO / VITO; regional distribution by Digital Earth Africa. Cite the reference year, original item, file version and access date.".into();
    dataset.value_interpretation = "Original UInt8 permanent-water cover fractions 0–100, not discrete land-cover classes, water-occurrence counts, optical reflectance or a daily observation. Zero is a valid zero-percent fraction. Publisher fill code 255 is used as NoData only when declared by the original file; no rescaling or scientific calibration is applied.".into();
    let mut water = layer(
        LAYER,
        "Permanent-water cover fraction",
        "percent",
        "Fraction of the mapped cell occupied by permanent water, original 0–100 percent values; publisher fill code 255. This layer belongs to the CGLS land-cover family and is distinct from its categorical classification and seasonal-water fraction.",
    );
    water.nodata = Some(255.0);
    dataset.layers = vec![water];
    dataset.default_layer = Some(LAYER.into());
    dataset.warnings = vec![
        "The current connector supports one layer within the CGLS land-cover product family and five actual publisher reference-year items. The requested area must be wholly within the finite published distribution bbox; outside or oversized selections fail rather than substituting another region, year or layer. Bbox containment does not establish useful valid-pixel coverage.".into(),
        "The reference year and publisher STAC calendar interval describe the map release, not a single-year acquisition guarantee. The independently decoded 2019-nrt original is v3.0.1 and declares input coverage from 2017-04-01 through 2020-03-31. Other years' input periods and file versions require inspection of their own originals.".into(),
        "Actual standalone publisher items declare s3://deafrica-input-datasets/gls/<year>/ asset paths, but the probed 2019 permanent-water URL there returns 404. The documented and actually listed distribution uses cgls_landcover/<year>/. A named native receipt maps only this exact reviewed product/year/asset to that fixed public HTTPS path. Original JSON, original S3 href and all other asset declarations remain unchanged; this is not a generic S3 rewrite.".into(),
        "These publisher items contain only a derived_from source link, with no self or collection link. Revalidation requests the exact archived item URL. CC BY 4.0 is a separately documented DE Africa product-definition declaration, not an invented item/collection license. Source connection completion does not verify a missing collection declaration.".into(),
        "The complete continental distribution file is downloaded without cropping. Other CGLS assets remain unsupported by this scoped connector, including several originals over 512 MiB. A successful permanent-water file test does not validate the complete land-cover family, all years, scientific accuracy or every pixel in the requested area.".into(),
        "The tested file has original NoData 255 and overviews generated with bilinear resampling. A grayscale preview may use an embedded overview; raw pixel inspection reads image zero. Do not infer classification styling, calibrated values or full-resolution samples from the thumbnail.".into(),
    ];
    dataset.limits = vec![
        "One original permanent-water cover-fraction file per reference year. Its actual grid is 91,587 × 87,011, or 7,969,076,457 pixels, just below the existing 8-billion-pixel tiled-raster bound.".into(),
        "Original transfers remain limited to 512 MiB. Tiled TIFF axes are limited to 131,072; stripped axes remain limited to 65,536. Native review retains 128 MiB chunk, 32 MiB intermediate, 512 MiB preview-budget and 30-second bounds. Published availability does not bypass those limits.".into(),
    ];
    dataset
}

fn bbox(document: &Value) -> Result<[f64; 4]> {
    serde_json::from_value(document["bbox"].clone()).map_err(crate::io_error)
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != "cgls-landcover"
        || request.month.is_some()
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
        || request.layer.as_deref() != Some(LAYER)
    {
        return Err("Choose the reviewed CGLS permanent-water layer and reference year; this product does not use month, country, region or season parameters".into());
    }
    let item = ITEMS
        .iter()
        .find(|item| Some(item.year) == request.year)
        .ok_or("Choose a published CGLS reference year from 2015 through 2019")?;
    let bounds = bbox(&item.document)?;
    if request.bounds[0] < bounds[0]
        || request.bounds[1] < bounds[1]
        || request.bounds[2] > bounds[2]
        || request.bounds[3] > bounds[3]
    {
        return Err("The requested area is outside or extends beyond the reviewed CGLS DE Africa distribution bbox. Narrow the map area; no alternate area, year or product is substituted".into());
    }
    let title = format!("CGLS {} · permanent-water fraction · DE Africa", item.year);
    Ok(vec![DatasetSource {
        id: item.document["id"]
            .as_str()
            .ok_or("Reviewed CGLS item identifier is missing")?
            .into(),
        title: title.clone(),
        bounds: Some(bounds),
        stac: Some(stac::ConnectRequest {
            name: title,
            url: item.url.clone(),
            kind: "item".into(),
        }),
        wcs: None,
        coverage_id: None,
        search: None,
        region: None,
        season: None,
        start_datetime: item.document["properties"]["start_datetime"]
            .as_str()
            .map(str::to_owned),
        end_datetime: item.document["properties"]["end_datetime"]
            .as_str()
            .map(str::to_owned),
        catalog_raster: None,
    }])
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    if proof.schema_version != 2 || proof.dataset.id != "cgls-landcover" {
        return Err("Unsupported reviewed CGLS publisher-item provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let source = plan
        .sources
        .iter()
        .find(|source| {
            source.id == proof.source_id
                && source
                    .stac
                    .as_ref()
                    .is_some_and(|request| request.url == url)
        })
        .ok_or("CGLS source does not match its exact reviewed year/item recipe")?;
    if super::provenance(&plan, source) != *proof {
        return Err("CGLS declaration differs from its immutable reviewed preset".into());
    }
    let reviewed = ITEMS
        .iter()
        .find(|item| item.url == url)
        .ok_or("Publisher item is outside the reviewed CGLS index")?;
    if raw.is_some_and(|raw| *raw != reviewed.document) {
        return Err("Reviewed CGLS publisher document changed; original asset declarations, bounds, reference-year interval or item identity differ from the fixed index".into());
    }
    Ok(())
}

pub(crate) fn transports(
    proof: &SourceProvenance,
    url: &str,
    raw: &Value,
) -> Result<Vec<stac::AssetTransport>> {
    validate(proof, url, Some(raw))?;
    let year = proof.request.year.ok_or("CGLS reference year is missing")?;
    let original = raw["assets"][ASSET]["href"]
        .as_str()
        .ok_or("Original CGLS permanent-water asset href is missing")?;
    let expected_item =
        format!("{HTTPS}cgls_landcover/{year}/cgls_landcover_{year}.stac-item.json");
    let expected_original = format!("{BUCKET}gls/{year}/cgls_landcover_{year}_{ASSET}.tif");
    if url != expected_item || original != expected_original {
        return Err("Only the exact reviewed CGLS year and permanent-water S3 asset receive the documented distribution-path receipt".into());
    }
    Ok(vec![stac::AssetTransport {
        asset_key: ASSET.into(),
        original_href: original.into(),
        href: format!("{HTTPS}cgls_landcover/{year}/cgls_landcover_{year}_{ASSET}.tif"),
        adapter: ADAPTER.into(),
    }])
}

pub(crate) fn self_url(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<String> {
    validate(proof, url, Some(raw))?;
    let links = raw["links"]
        .as_array()
        .ok_or("Reviewed CGLS publisher links are missing")?;
    if links
        .iter()
        .any(|link| matches!(link["rel"].as_str(), Some("self" | "collection")))
    {
        return Err(
            "Reviewed CGLS metadata unexpectedly declares a self or collection link".into(),
        );
    }
    // No self link exists in the actual publisher document. Use its exact
    // independently retrieved URL, without fabricating a link or collection.
    Ok(url.into())
}

#[cfg(test)]
pub(crate) fn reviewed_document(year: u16) -> (String, Value) {
    let item = ITEMS.iter().find(|item| item.year == year).unwrap();
    (item.url.clone(), item.document.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn request(year: u16) -> ResolveRequest {
        serde_json::from_value(json!({
            "datasetId":"cgls-landcover", "bounds":[32.5,-1.,33.,0.],
            "year":year, "layer":"permanentwater"
        }))
        .unwrap()
    }

    #[test]
    fn cgls_original_items_resolve_exact_year_bounds_without_substitution() {
        assert_eq!(ITEMS.len(), 5);
        assert_eq!(definition().layers.len(), 1);
        for year in 2015..=2019 {
            let initial = request(year);
            let source = &resolve(&initial).unwrap()[0];
            assert_eq!(source.bounds.unwrap()[0], -26.36011904761955);
            assert!(source.stac.as_ref().unwrap().name.len() <= 80);
            assert!(source
                .stac
                .as_ref()
                .unwrap()
                .url
                .contains(&format!("/{year}/")));
            assert_eq!(
                source.start_datetime,
                Some(format!("{year}-01-01T00:00:00Z"))
            );
            assert_eq!(initial.bounds, [32.5, -1., 33., 0.]);
        }
        let mut invalid = request(2019);
        invalid.bounds = [116., 39., 117., 40.];
        assert!(resolve(&invalid).is_err());
        invalid.bounds = [-40., -50., 70., 45.];
        assert!(resolve(&invalid).is_err());
        assert!(resolve(&request(2020)).is_err());
        invalid = request(2019);
        invalid.layer = Some("classification".into());
        assert!(resolve(&invalid).is_err());
        invalid = request(2019);
        invalid.month = Some(1);
        assert!(resolve(&invalid).is_err());
    }

    #[test]
    fn cgls_exact_document_and_one_asset_receipt_preserve_publisher_path_conflict() {
        for year in 2015..=2019 {
            let plan = super::super::resolve(request(year)).unwrap();
            let proof = super::super::provenance(&plan, &plan.sources[0]);
            let item = ITEMS.iter().find(|item| item.year == year).unwrap();
            let before = item.document.clone();
            let mapping = transports(&proof, &item.url, &item.document).unwrap();
            assert_eq!(mapping.len(), 1);
            assert_eq!(mapping[0].asset_key, ASSET);
            assert_eq!(mapping[0].adapter, ADAPTER);
            assert!(mapping[0].original_href.contains("/gls/"));
            assert!(mapping[0].href.contains("/cgls_landcover/"));
            assert_eq!(
                self_url(&proof, &item.url, &item.document).unwrap(),
                item.url
            );
            assert_eq!(item.document, before);
            for replacement in [
                "s3://private-bucket/gls/2019/file.tif",
                "s3://deafrica-input-datasets/gls/2019/../file.tif",
                "https://unreviewed.test/file.tif",
            ] {
                let mut changed = before.clone();
                changed["assets"][ASSET]["href"] = json!(replacement);
                assert!(transports(&proof, &item.url, &changed).is_err());
            }
            let mut changed = before.clone();
            changed["properties"]["start_datetime"] = json!("2019-02-01T00:00:00Z");
            assert!(validate(&proof, &item.url, Some(&changed)).is_err());
            changed = before;
            changed["links"] = json!([{"rel":"self","href":"s3://unreviewed/item.json"}]);
            assert!(self_url(&proof, &item.url, &changed).is_err());
            let mut corrupt = proof;
            corrupt.request.bounds = [116., 39., 117., 40.];
            assert!(validate(&corrupt, &item.url, Some(&item.document)).is_err());
        }
    }
}
