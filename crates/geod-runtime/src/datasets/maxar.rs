//! Finite, independently decoded Maui 2023 Maxar ARD analytic originals.
//! New Vantor and legacy visual YCbCr files remain outside this raw-sample
//! adapter. Publisher item, acquisition collection and event JSON stay intact.
use super::*;
use serde_json::Value;
use std::sync::LazyLock;
use url::Url;

const ID: &str = "maxar-open-legacy";
const EVENT_ID: &str = "Maui-Hawaii-fires-Aug-23";
const YEAR: u16 = 2023;
const MONTH: u8 = 8;
const POLICY: &str = "https://creativecommons.org/licenses/by-nc/4.0/";

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedItem {
    dataset_id: String,
    url: String,
    collection_url: String,
    event_url: String,
    document: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedCollection {
    dataset_id: String,
    url: String,
    event_url: String,
    document: Value,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ReviewedEvent {
    dataset_id: String,
    url: String,
    document: Value,
}

static ITEMS: LazyLock<Vec<ReviewedItem>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("maxar-reviewed-items.json"))
        .expect("actual reviewed Maui August 2023 publisher item")
});
static COLLECTIONS: LazyLock<Vec<ReviewedCollection>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("maxar-reviewed-collections.json"))
        .expect("actual reviewed Maui acquisition collection")
});
static EVENTS: LazyLock<Vec<ReviewedEvent>> = LazyLock::new(|| {
    serde_json::from_str(include_str!("maxar-reviewed-events.json"))
        .expect("actual reviewed Maui event collection")
});

pub(super) fn definition() -> Dataset {
    let mut dataset = base(
        ID,
        "Maxar / Vantor Open Data · Maui ARD analytic imagery",
        "imagery",
        "https://registry.opendata.aws/maxar-open-data/",
    );
    dataset.description = "A finite reviewed WorldView-2 ARD tile from the Maui August 2023 disaster activation, with complete original eight-band multispectral and single-band panchromatic GeoTIFFs. This is an event archive, not a commercial imagery procurement service or global on-demand high-resolution catalog.".into();
    dataset.version =
        "Maui 2023-08-12 · one reviewed WV02 ARD tile · original metadata 0.0.1".into();
    dataset.license = "CC-BY-NC-4.0".into();
    dataset.license_url = POLICY.into();
    dataset.attribution = "Maxar Open Data Program, Maui-Hawaii-fires-Aug-23, WorldView-2 acquisition 10300100EB15FF00 and original item 4/122000312222/10300100EB15FF00. Retain the original item, acquisition/event collection, source date, CC BY-NC 4.0 notice and access date.".into();
    dataset.nominal_resolution = "Tested output grids: multispectral 2.056716995741386 m; panchromatic 0.5141792489353465 m, EPSG:32604. Publisher item sensor GSD 0.53 m is retained separately.".into();
    dataset.unit = "Original stored UInt16 image samples".into();
    dataset.value_interpretation = "Original stored integer bands remain unchanged. The publisher item does not declare a scalar radiometric scale, offset or physical unit, so no reflectance calibration, radiance conversion or automatic scientific scaling is inferred. Full files include an independent dataset validity mask with no scalar NoData tag; a raw zero and native noData=false do not establish usable imagery.".into();
    dataset.years = vec![YEAR];
    dataset.default_year = Some(YEAR);
    dataset.months = vec![MONTH];
    dataset.default_month = Some(MONTH);
    dataset.layers = vec![
        layer(
            "ms_analytic",
            "Multispectral analytic · eight original bands",
            "Stored UInt16 image samples",
            "Eight original bands in publisher order: coastal blue, blue, green, yellow, red, red edge, NIR1 and NIR2. The tested 2.056716995741386 m grid, integer samples and independent validity mask are retained. No RGB composition, reflectance conversion, vegetation index or analytic processing result is synthesized.",
        ),
        layer(
            "pan_analytic",
            "Panchromatic analytic · original single band",
            "Stored UInt16 image samples",
            "One original panchromatic band on the tested 0.5141792489353465 m grid, with its independent validity mask retained. It is grayscale imagery, not RGB or a promise of equivalent sensor detail, and is not calibrated or resampled by this adapter.",
        ),
    ];
    dataset.default_layer = Some("pan_analytic".into());
    dataset.discovery = Some("stac-items".into());
    dataset.warnings = vec![
        "Only the independently downloaded and decoded Maui 2023-08-12 item 4/122000312222/10300100EB15FF00 is currently indexed. Map bounds select overlap with that original footprint bbox; no different event, date, unreviewed tile or demonstration area is substituted.".into(),
        "The actual acquisition instant is 2023-08-12 21:12:49Z. That exact publisher string remains in the item and source start/end; the event name, ingestion date and publication time are not observation dates. A point observation is not a month-long acquisition interval.".into(),
        "The tested TIFFs declare no scalar NoData. Their original per-dataset binary mask is stored in a separate second IFD, with additional reduced mask IFDs; it is not an alpha channel. Current native raw previews and pixel noData flags do not apply that independent mask. An unmasked raw zero or noData=false must not be interpreted as usable coverage, ocean, dry terrain or absence of damage.".into(),
        "The entire original 5.3125 km tile canvas is delivered, including masked areas outside the smaller acquisition footprint. Bbox intersection, the original scene cloud statement and first-band grayscale preview do not establish valid-AOI coverage, cloud-free imagery or disaster damage.".into(),
        "Event and acquisition collection both declare CC-BY-NC-4.0 for this selected Maui item. Attribution and noncommercial-use restrictions remain; public access or a GeoD subscription does not grant commercial reuse or an imagery purchase right. Other legacy acquisition collections can declare proprietary and are not substituted.".into(),
        "The publisher visual asset and GeoPackage data-mask declarations remain unchanged in the source item. They are outside these two supported layers: the actual visual JPEG/YCbCr file is not decoded by the current raw-sample reader, and the GeoPackage is not a raster imagery asset. New 2026 Vantor event visuals retain that separate decoder boundary.".into(),
        "Complete originals, original band order, embedded metadata, dataset masks and publisher relative asset hrefs are retained. Native preview is a first-band grayscale stretch, not a multispectral RGB display. Publisher file checksums are absent; local SHA-256 transfer receipts are not publisher checksum certification.".into(),
    ];
    dataset.limits = vec![
        "One finite actual Maui event item and two verified analytic asset keys; only observation year 2023 and month 8. Country, named region, season, other dates and visual/thumbnail layers are rejected.".into(),
        "Each complete original is bounded by the existing 512 MiB transfer cap and native TIFF decoding limits. Metadata discovery is separate from successful original transfer and does not certify valid imagery coverage.".into(),
    ];
    dataset
}

fn bbox(document: &Value) -> Result<[f64; 4]> {
    let bounds = serde_json::from_value(document["bbox"].clone()).map_err(crate::io_error)?;
    features::bounds(bounds)?;
    Ok(bounds)
}

fn source(item: &ReviewedItem) -> Result<DatasetSource> {
    let id = item.document["id"]
        .as_str()
        .ok_or("Reviewed Maxar item has no original ID")?;
    let instant = item.document["properties"]["datetime"]
        .as_str()
        .ok_or("Reviewed Maxar item has no acquisition instant")?;
    let title = format!("Maui · WV02 · 2023-08-12 · {id}");
    Ok(DatasetSource {
        id: id.into(),
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
        region: None,
        season: None,
        start_datetime: Some(instant.into()),
        end_datetime: Some(instant.into()),
        catalog_raster: None,
    })
}

pub(super) fn resolve(request: &ResolveRequest) -> Result<Vec<DatasetSource>> {
    features::bounds(request.bounds)?;
    if request.dataset_id != ID
        || request.year != Some(YEAR)
        || request.month != Some(MONTH)
        || !matches!(
            request.layer.as_deref(),
            Some("ms_analytic" | "pan_analytic")
        )
        || request.country.is_some()
        || request.region.is_some()
        || request.season.is_some()
    {
        return Err("Choose a reviewed Maui August 2023 analytic asset; other events, dates, visual, country, named region and season selections are unsupported".into());
    }
    let mut selected = Vec::new();
    for item in ITEMS.iter().filter(|item| item.dataset_id == ID) {
        let footprint = bbox(&item.document)?;
        if request.bounds[0] < footprint[2]
            && request.bounds[2] > footprint[0]
            && request.bounds[1] < footprint[3]
            && request.bounds[3] > footprint[1]
        {
            selected.push(source(item)?);
        }
    }
    if selected.is_empty() || selected.len() > MAX_TILES {
        return Err("The requested area has no reviewed Maui analytic item or exceeds the existing selection limit; no alternate tile or event is inserted".into());
    }
    Ok(selected)
}

fn reviewed(proof: &SourceProvenance, url: &str) -> Result<&'static ReviewedItem> {
    if proof.schema_version != 2 || proof.dataset.id != ID || proof.catalog_raster.is_some() {
        return Err("Unsupported Maxar finite analytic provenance".into());
    }
    let plan = super::resolve(proof.request.clone())?;
    let selected = plan
        .sources
        .iter()
        .find(|candidate| {
            candidate.id == proof.source_id
                && candidate
                    .stac
                    .as_ref()
                    .is_some_and(|endpoint| endpoint.url == url)
        })
        .ok_or("Maxar source differs from the exact reviewed Maui item and requested footprint")?;
    if super::provenance(&plan, selected) != *proof {
        return Err("Maxar product, layer, request or declaration differs from its immutable reviewed recipe".into());
    }
    ITEMS
        .iter()
        .find(|item| item.dataset_id == ID && item.url == url)
        .ok_or("Maxar item URL is outside the finite reviewed archive".into())
}

pub(crate) fn validate(proof: &SourceProvenance, url: &str, raw: Option<&Value>) -> Result<()> {
    let item = reviewed(proof, url)?;
    if raw.is_some_and(|document| document != &item.document) {
        return Err("Maxar publisher item changed: original acquisition instant, footprint, band order, relative asset declaration or identity differs from the reviewed document".into());
    }
    if item.document["type"] != "Feature"
        || item.document["id"] != proof.source_id
        || item.document["properties"]["datetime"] != "2023-08-12 21:12:49Z"
        || item.document["collection"] != "10300100EB15FF00"
        || item.document["assets"][&proof.layer.id]["roles"] != serde_json::json!(["data"])
    {
        return Err("Maxar reviewed document is not the selected original analytic asset".into());
    }
    Ok(())
}

pub(crate) fn validate_collection(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<()> {
    let item_url = ITEMS
        .iter()
        .find(|item| item.dataset_id == ID && item.document["id"] == proof.source_id)
        .map(|item| item.url.as_str())
        .ok_or("Maxar collection has no reviewed source item")?;
    let item = reviewed(proof, item_url)?;
    validate(proof, item_url, None)?;
    let expected = COLLECTIONS
        .iter()
        .find(|collection| collection.dataset_id == ID && collection.url == url)
        .ok_or("Maxar acquisition collection URL is outside the reviewed archive")?;
    if item.collection_url != url
        || item.event_url != expected.event_url
        || raw != &expected.document
        || raw["id"] != item.document["collection"]
        || raw["type"] != "Collection"
        || raw["license"] != "CC-BY-NC-4.0"
    {
        return Err("Maxar acquisition collection changed or differs from the original item identity and license".into());
    }
    let base = Url::parse(url).map_err(crate::io_error)?;
    if !raw["links"].as_array().is_some_and(|links| {
        links.iter().any(|link| {
            link["rel"] == "item"
                && link["href"].as_str().is_some_and(|href| {
                    base.join(href)
                        .is_ok_and(|endpoint| endpoint.as_str() == item_url)
                })
        })
    }) {
        return Err("Maxar acquisition collection does not bind the exact selected item".into());
    }
    Ok(())
}

pub(crate) fn event_url(proof: &SourceProvenance) -> Result<String> {
    let item = ITEMS
        .iter()
        .find(|item| item.dataset_id == ID && item.document["id"] == proof.source_id)
        .ok_or("Maxar event receipt has no exact source item")?;
    validate(proof, &item.url, None)?;
    Ok(item.event_url.clone())
}

pub(crate) fn validate_event(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<()> {
    if event_url(proof)? != url {
        return Err("Maxar event receipt does not match the selected Maui source".into());
    }
    let event = EVENTS
        .iter()
        .find(|event| event.dataset_id == ID && event.url == url)
        .ok_or("Maxar event URL is outside the finite reviewed archive")?;
    if raw != &event.document
        || raw["type"] != "Collection"
        || raw["id"] != EVENT_ID
        || raw["license"] != "CC-BY-NC-4.0"
    {
        return Err("Maxar event declaration changed: exact identity, license and acquisition contents must remain intact".into());
    }
    let item = ITEMS
        .iter()
        .find(|item| item.dataset_id == ID && item.document["id"] == proof.source_id)
        .ok_or("Maxar event has no exact source item")?;
    let base = Url::parse(url).map_err(crate::io_error)?;
    if !raw["links"].as_array().is_some_and(|links| {
        links.iter().any(|link| {
            link["rel"] == "child"
                && link["href"].as_str().is_some_and(|href| {
                    base.join(href)
                        .is_ok_and(|endpoint| endpoint.as_str() == item.collection_url)
                })
        })
    }) {
        return Err("Maxar event does not bind the reviewed acquisition collection".into());
    }
    Ok(())
}

pub(crate) fn transports(
    proof: &SourceProvenance,
    url: &str,
    raw: &Value,
) -> Result<Vec<stac::AssetTransport>> {
    validate(proof, url, Some(raw))?;
    // Published relative hrefs already resolve to their exact anonymous HTTPS
    // originals. No alias, S3 permission expansion or transport substitution.
    Ok(Vec::new())
}

pub(crate) fn self_url(proof: &SourceProvenance, url: &str, raw: &Value) -> Result<String> {
    validate(proof, url, Some(raw))?;
    // The real publisher item has no self link. Revalidate the exact actual URL;
    // never add a link to the item or authorize another missing-self source.
    Ok(url.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn request(asset: &str) -> ResolveRequest {
        ResolveRequest {
            dataset_id: ID.into(),
            bounds: [-156.671, 20.965, -156.667, 20.968],
            year: Some(YEAR),
            month: Some(MONTH),
            layer: Some(asset.into()),
            country: None,
            region: None,
            season: None,
        }
    }

    fn proof(asset: &str) -> SourceProvenance {
        let plan = super::super::resolve(request(asset)).unwrap();
        super::super::provenance(&plan, &plan.sources[0])
    }

    #[test]
    fn reviewed_analytic_sources_keep_original_identity_point_time_and_band_semantics() {
        let dataset = definition();
        assert_eq!(dataset.license, "CC-BY-NC-4.0");
        assert_eq!(
            dataset
                .layers
                .iter()
                .map(|layer| layer.id.as_str())
                .collect::<Vec<_>>(),
            ["ms_analytic", "pan_analytic"]
        );
        assert!(dataset
            .layers
            .iter()
            .all(|layer| layer.nodata.is_none() && layer.scale.is_none()));
        for asset in ["ms_analytic", "pan_analytic"] {
            let sources = resolve(&request(asset)).unwrap();
            assert_eq!(sources.len(), 1);
            assert_eq!(sources[0].id, "4/122000312222/10300100EB15FF00");
            assert_eq!(sources[0].region, None);
            assert_eq!(
                sources[0].start_datetime.as_deref(),
                Some("2023-08-12 21:12:49Z")
            );
            assert_eq!(sources[0].start_datetime, sources[0].end_datetime);
            let proof = proof(asset);
            let item = &ITEMS[0];
            assert!(validate(&proof, &item.url, Some(&item.document)).is_ok());
            assert_eq!(
                self_url(&proof, &item.url, &item.document).unwrap(),
                item.url
            );
            assert!(transports(&proof, &item.url, &item.document)
                .unwrap()
                .is_empty());
            assert!(
                validate_collection(&proof, &COLLECTIONS[0].url, &COLLECTIONS[0].document).is_ok()
            );
            assert!(validate_event(&proof, &EVENTS[0].url, &EVENTS[0].document).is_ok());
        }
    }

    #[test]
    fn no_unreviewed_region_date_layer_or_empty_overlap_is_substituted() {
        let baseline = request("pan_analytic");
        for key in ["visual", "thumbnail", "data-mask", "pan", "reflectance", ""] {
            let mut changed = baseline.clone();
            changed.layer = Some(key.into());
            assert!(resolve(&changed).is_err(), "unexpected asset {key}");
        }
        for bounds in [
            [-74., 40., -73., 41.],
            [85., 28., 85.1, 28.1],
            [-156.642091971835, 20.963, -156.64, 20.968],
            [0., 0., 0., 1.],
            [f64::NAN, 0., 1., 1.],
        ] {
            let mut changed = baseline.clone();
            changed.bounds = bounds;
            assert!(resolve(&changed).is_err());
        }
        for year in [None, Some(2024), Some(2026)] {
            let mut changed = baseline.clone();
            changed.year = year;
            assert!(resolve(&changed).is_err());
        }
        for month in [None, Some(0), Some(9)] {
            let mut changed = baseline.clone();
            changed.month = month;
            assert!(resolve(&changed).is_err());
        }
        let mut changed = baseline.clone();
        changed.country = Some("USA".into());
        assert!(resolve(&changed).is_err());
        let mut changed = baseline.clone();
        changed.region = Some(EVENT_ID.into());
        assert!(resolve(&changed).is_err());
        let mut changed = baseline;
        changed.season = Some("post".into());
        assert!(resolve(&changed).is_err());
    }

    #[test]
    fn original_item_collection_event_and_client_proof_are_exactly_bound() {
        let proof = proof("ms_analytic");
        let item = &ITEMS[0];
        for value in [serde_json::json!("CC-BY-4.0"), Value::Null] {
            let mut changed = item.document.clone();
            changed["assets"]["ms_analytic"]["href"] = value;
            assert!(validate(&proof, &item.url, Some(&changed)).is_err());
        }
        let mut changed = item.document.clone();
        changed["properties"]["datetime"] = serde_json::json!("2023-08-01T00:00:00Z");
        assert!(validate(&proof, &item.url, Some(&changed)).is_err());
        let mut changed = item.document.clone();
        changed["collection"] = serde_json::json!(EVENT_ID);
        assert!(validate(&proof, &item.url, Some(&changed)).is_err());
        let mut changed = proof.clone();
        changed.request.bounds = [-74., 40., -73., 41.];
        assert!(validate(&changed, &item.url, Some(&item.document)).is_err());
        let mut changed = proof.clone();
        changed.dataset.license = "CC-BY-4.0".into();
        assert!(validate(&changed, &item.url, Some(&item.document)).is_err());
        assert!(validate(&proof, "https://vantor-opendata.s3.amazonaws.com/events/Nepal-Flooding-Aug-2026/B120001101250A10.json", None).is_err());
        for url in ["https://example.com/collection.json", &item.event_url] {
            assert!(validate_collection(&proof, url, &COLLECTIONS[0].document).is_err());
        }
        let mut changed = COLLECTIONS[0].document.clone();
        changed["license"] = serde_json::json!("proprietary");
        assert!(validate_collection(&proof, &COLLECTIONS[0].url, &changed).is_err());
        let mut changed = EVENTS[0].document.clone();
        changed["license"] = serde_json::json!("CC-BY-4.0");
        assert!(validate_event(&proof, &EVENTS[0].url, &changed).is_err());
        assert!(validate_event(&proof, &COLLECTIONS[0].url, &EVENTS[0].document).is_err());
    }
}
