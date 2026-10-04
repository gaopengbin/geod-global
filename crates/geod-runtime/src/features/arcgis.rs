//! Public ArcGIS FeatureServer extraction. Preserve service GeoJSON and its field schema.
use super::*;
#[cfg(test)]
mod tests;

const MAX_IDS: usize = MAX_PAGES * PAGE_SIZE;
const SAFE_ID: u64 = 9_007_199_254_740_991;
type Parameters = BTreeMap<String, String>;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub alias: String,
    pub field_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layer {
    pub object_id_field: String,
    pub geometry_type: String,
    pub spatial_reference: Value,
    pub fields: Vec<Field>,
    pub max_record_count: usize,
    pub metadata_sha256: String,
    pub copyright_text: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExcludedLayer {
    pub id: String,
    pub name: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Service {
    pub current_version: f64,
    pub copyright_text: String,
    pub metadata_sha256: String,
    pub excluded_layers: Vec<ExcludedLayer>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub layer: Layer,
    pub object_ids: Vec<u64>,
    pub id_receipts: Vec<PageReceipt>,
    pub count_receipts: Vec<PageReceipt>,
}

fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn layer_id(v: &str) -> bool {
    v.parse::<u32>().ok().is_some_and(|n| n.to_string() == v)
}
fn root_url(raw: &str) -> Result<Url> {
    let mut root = public_url(raw)?;
    if root.query().is_some()
        || !root
            .path()
            .trim_end_matches('/')
            .ends_with("/FeatureServer")
    {
        return Err("Enter a public ArcGIS FeatureServer URL without query parameters".into());
    }
    let path = root.path().trim_end_matches('/').to_string();
    root.set_path(&path);
    Ok(root)
}
fn resource(root: &Url, path: &str) -> Result<Url> {
    scoped(
        root,
        &format!("{}/{path}", root.as_str().trim_end_matches('/')),
    )
}
fn query_url(root: &Url, id: &str) -> Result<Url> {
    if !layer_id(id) {
        return Err("Invalid ArcGIS layer identifier".into());
    }
    resource(root, &format!("{id}/query"))
}
fn metadata(root: &Url, path: &str) -> Result<Url> {
    let mut u = if path.is_empty() {
        root.clone()
    } else {
        resource(root, path)?
    };
    u.query_pairs_mut().append_pair("f", "json");
    Ok(u)
}
fn response_ok(v: &Value) -> Result<()> {
    if v.get("error").is_some() {
        let code = v["error"]["code"].as_i64().unwrap_or(0);
        return Err(if [401, 403, 498, 499].contains(&code) {
            "This ArcGIS service requires authorization; use a public service".into()
        } else {
            format!("ArcGIS query failed (service error {code}); check the service and retry")
        });
    }
    if !v.is_object() {
        return Err("Invalid ArcGIS response".into());
    }
    Ok(())
}
fn complete_response(v: &Value) -> Result<()> {
    response_ok(v)?;
    if [
        v.get("exceededTransferLimit"),
        v.get("properties")
            .and_then(|p| p.get("exceededTransferLimit")),
    ]
    .into_iter()
    .flatten()
    .any(|flag| flag != false)
    {
        return Err("ArcGIS truncated the query; use a smaller region or batch".into());
    }
    Ok(())
}
fn has_capability(v: &Value, key: &str, wanted: &str) -> bool {
    v[key]
        .as_str()
        .is_some_and(|s| s.split(',').any(|s| s.trim().eq_ignore_ascii_case(wanted)))
}
fn field_type(t: &str) -> bool {
    matches!(
        t,
        "esriFieldTypeOID"
            | "esriFieldTypeSmallInteger"
            | "esriFieldTypeInteger"
            | "esriFieldTypeSingle"
            | "esriFieldTypeDouble"
            | "esriFieldTypeString"
            | "esriFieldTypeDate"
            | "esriFieldTypeGUID"
            | "esriFieldTypeGlobalID"
            | "esriFieldTypeBigInteger"
            | "esriFieldTypeDateOnly"
            | "esriFieldTypeTimeOnly"
            | "esriFieldTypeTimestampOffset"
    )
}
fn validate_layer(layer: &Layer) -> Result<()> {
    let mut names = BTreeSet::new();
    if !clean(&layer.object_id_field, 256)
        || !matches!(
            layer.geometry_type.as_str(),
            "esriGeometryPoint"
                | "esriGeometryMultipoint"
                | "esriGeometryPolyline"
                | "esriGeometryPolygon"
        )
        || layer
            .spatial_reference
            .as_object()
            .is_none_or(|v| v.is_empty())
        || serde_json::to_vec(&layer.spatial_reference)
            .map_err(io_error)?
            .len()
            > 65_536
        || layer.fields.is_empty()
        || layer.fields.len() > 512
        || !(1..=1_000_000).contains(&layer.max_record_count)
        || !digest(&layer.metadata_sha256)
        || layer.copyright_text.len() > 16_384
    {
        return Err("Invalid ArcGIS layer metadata".into());
    }
    for f in &layer.fields {
        if !clean(&f.name, 256)
            || f.alias.chars().count() > 256
            || !field_type(&f.field_type)
            || !names.insert(&f.name)
        {
            return Err("ArcGIS layer contains unsupported or repeated fields".into());
        }
    }
    if layer
        .fields
        .iter()
        .filter(|f| f.field_type == "esriFieldTypeOID")
        .count()
        != 1
        || !layer
            .fields
            .iter()
            .any(|f| f.name == layer.object_id_field && f.field_type == "esriFieldTypeOID")
    {
        return Err("ArcGIS layer must have one numeric object ID field".into());
    }
    Ok(())
}
fn parse_layer(
    root: &Url,
    v: &Value,
    metadata_sha256: &str,
    service_copyright: &str,
) -> Result<Collection> {
    response_ok(v)?;
    if v["type"] != "Feature Layer" || !has_capability(v, "capabilities", "Query") {
        return Err("This layer does not provide queryable feature geometry".into());
    }
    if !has_capability(v, "supportedQueryFormats", "geojson") {
        return Err("This layer does not advertise GeoJSON query output".into());
    }
    if v["uniqueIdInfo"]["OIDFieldContainsHashValue"] == true {
        return Err("Hashed object IDs need a separate unique ID query adapter".into());
    }
    if v["hasZ"].as_bool() != Some(false)
        || v["hasM"].as_bool() != Some(false)
        || v["hasCurves"] == true
    {
        return Err("Z, M and declared curve layers need a separate geometry adapter".into());
    }
    let id = v["id"]
        .as_u64()
        .filter(|n| *n <= u32::MAX as u64)
        .ok_or("Invalid ArcGIS layer identifier")?
        .to_string();
    let title = v["name"]
        .as_str()
        .filter(|s| clean(s, 240))
        .ok_or("ArcGIS layer has no valid name")?;
    let description = v["description"].as_str().unwrap_or("");
    if description.len() > 16_384 {
        return Err("ArcGIS layer description is too long".into());
    }
    let fields = v["fields"]
        .as_array()
        .filter(|a| a.len() <= 512)
        .ok_or("Invalid ArcGIS fields")?
        .iter()
        .map(|f| {
            Ok(Field {
                name: f["name"].as_str().ok_or("ArcGIS field has no name")?.into(),
                alias: f["alias"].as_str().unwrap_or("").into(),
                field_type: f["type"].as_str().ok_or("ArcGIS field has no type")?.into(),
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let reference = v
        .get("sourceSpatialReference")
        .or_else(|| v.get("extent").and_then(|e| e.get("spatialReference")))
        .ok_or("ArcGIS layer has no declared source spatial reference")?
        .clone();
    let layer = Layer {
        object_id_field: v["objectIdField"]
            .as_str()
            .ok_or("ArcGIS layer has no object ID field")?
            .into(),
        geometry_type: v["geometryType"].as_str().unwrap_or("").into(),
        spatial_reference: reference,
        fields,
        max_record_count: v["maxRecordCount"]
            .as_u64()
            .filter(|n| *n <= 1_000_000)
            .ok_or("Invalid ArcGIS query limit")? as usize,
        metadata_sha256: metadata_sha256.into(),
        copyright_text: v["copyrightText"]
            .as_str()
            .filter(|s| !s.is_empty())
            .unwrap_or(service_copyright)
            .into(),
    };
    validate_layer(&layer)?;
    Ok(Collection {
        id: id.clone(),
        title: title.into(),
        description: description.into(),
        items_url: query_url(root, &id)?.to_string(),
        license_links: vec![],
        arcgis: Some(layer),
        wfs: None,
    })
}
pub(super) async fn discover(
    request: ConnectRequest,
    settings: crate::ProxySettings,
) -> Result<FeatureService> {
    let name = request.name.trim();
    if !clean(name, 80) {
        return Err("Enter a data service name of 1–80 characters".into());
    }
    let root = root_url(request.url.trim())?;
    let c = client(&root, &settings).await?;
    let mut budget = MAX_METADATA;
    let (service, receipt) = fetch(&c, &metadata(&root, "")?, &mut budget).await?;
    response_ok(&service)?;
    if !has_capability(&service, "capabilities", "Query") {
        return Err("ArcGIS service does not support queries".into());
    }
    let version = service["currentVersion"]
        .as_f64()
        .filter(|v| v.is_finite() && *v >= 10.)
        .ok_or("Invalid ArcGIS service version")?;
    let copyright = service["copyrightText"].as_str().unwrap_or("");
    if copyright.len() > 16_384 {
        return Err("ArcGIS attribution is too long".into());
    }
    let (all, all_receipt) = fetch(&c, &metadata(&root, "layers")?, &mut budget).await?;
    response_ok(&all)?;
    let layers = all["layers"]
        .as_array()
        .filter(|v| v.len() <= 512)
        .ok_or("ArcGIS service has no bounded layer list")?;
    let mut collections = Vec::new();
    let mut excluded_layers = Vec::new();
    let mut seen = BTreeSet::new();
    for v in layers {
        let id = v["id"]
            .as_u64()
            .filter(|v| *v <= u32::MAX as u64)
            .ok_or("Invalid ArcGIS layer identifier")?
            .to_string();
        if !seen.insert(id.clone()) {
            return Err("Repeated ArcGIS layer identifiers".into());
        }
        match parse_layer(&root, v, &all_receipt.sha256, copyright) {
            Ok(layer) => collections.push(layer),
            Err(reason) => excluded_layers.push(ExcludedLayer {
                id,
                name: v["name"]
                    .as_str()
                    .unwrap_or("Unnamed layer")
                    .chars()
                    .take(240)
                    .collect(),
                reason,
            }),
        }
    }
    let title = service["mapName"]
        .as_str()
        .filter(|s| clean(s, 240))
        .unwrap_or(name);
    let result = FeatureService {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        url: root.to_string(),
        title: title.into(),
        collections,
        connected_at: now(),
        overpass: None,
        wfs: None,
        arcgis: Some(Service {
            current_version: version,
            copyright_text: copyright.into(),
            metadata_sha256: receipt.sha256,
            excluded_layers,
        }),
    };
    validate_service(&result)?;
    Ok(result)
}
pub(super) fn validate_service(service: &FeatureService) -> Result<()> {
    let root = root_url(&service.url)?;
    let meta = service
        .arcgis
        .as_ref()
        .ok_or("Missing ArcGIS service metadata")?;
    if service.wfs.is_some()
        || service.overpass.is_some()
        || root.as_str() != service.url
        || !meta.current_version.is_finite()
        || meta.current_version < 10.
        || !digest(&meta.metadata_sha256)
        || meta.copyright_text.len() > 16_384
        || meta.excluded_layers.len() > 512
        || service.collections.len() + meta.excluded_layers.len() > 512
    {
        return Err("Invalid ArcGIS service metadata".into());
    }
    let mut seen = BTreeSet::new();
    for c in &service.collections {
        if !seen.insert(c.id.clone())
            || c.items_url != query_url(&root, &c.id)?.as_str()
            || !clean(&c.title, 240)
            || c.wfs.is_some()
            || c.description.len() > 16_384
            || !c.license_links.is_empty()
        {
            return Err("Invalid saved ArcGIS layer".into());
        }
        validate_layer(c.arcgis.as_ref().ok_or("Missing ArcGIS layer metadata")?)?;
    }
    for e in &meta.excluded_layers {
        if !layer_id(&e.id)
            || !seen.insert(e.id.clone())
            || e.name.len() > 960
            || !clean(&e.reason, 512)
        {
            return Err("Invalid ArcGIS discovery exclusion".into());
        }
    }
    Ok(())
}

fn selection_parameters(b: [f64; 4], count: bool) -> Parameters {
    BTreeMap::from([
        ("f".into(), "json".into()),
        ("where".into(), "1=1".into()),
        (
            "geometry".into(),
            b.iter().map(f64::to_string).collect::<Vec<_>>().join(","),
        ),
        ("geometryType".into(), "esriGeometryEnvelope".into()),
        ("inSR".into(), "4326".into()),
        ("spatialRel".into(), "esriSpatialRelIntersects".into()),
        (
            (if count {
                "returnCountOnly"
            } else {
                "returnIdsOnly"
            })
            .into(),
            "true".into(),
        ),
    ])
}
fn batch_parameters(ids: &[u64]) -> Parameters {
    BTreeMap::from([
        ("f".into(), "geojson".into()),
        (
            "objectIds".into(),
            ids.iter().map(u64::to_string).collect::<Vec<_>>().join(","),
        ),
        ("outFields".into(), "*".into()),
        ("outSR".into(), "4326".into()),
        ("returnGeometry".into(), "true".into()),
    ])
}
fn parse_ids(v: &Value, layer: &Layer, count: usize) -> Result<Vec<u64>> {
    complete_response(v)?;
    if v["objectIdFieldName"] != layer.object_id_field {
        return Err("ArcGIS changed the object ID field".into());
    }
    // ArcGIS uses null for a real empty ID result. Accept it only with an
    // independently obtained zero count, never for a missing objectIds key.
    if count == 0 && v.get("objectIds") == Some(&Value::Null) {
        return Ok(Vec::new());
    }
    let ids = v["objectIds"]
        .as_array()
        .filter(|a| a.len() <= MAX_IDS)
        .ok_or("ArcGIS returned no bounded object ID list")?;
    let mut set = BTreeSet::new();
    for id in ids {
        let id = id
            .as_u64()
            .filter(|v| *v <= SAFE_ID)
            .ok_or("ArcGIS object IDs must be safe nonnegative integers")?;
        if !set.insert(id) {
            return Err("ArcGIS returned repeated object IDs".into());
        }
    }
    if count != set.len() {
        return Err(
            "ArcGIS object ID list is incomplete or the service changed; query again".into(),
        );
    }
    Ok(set.into_iter().collect())
}
async fn membership(
    c: &reqwest::Client,
    url: &Url,
    layer: &Layer,
    b: [f64; 4],
    budget: &mut usize,
) -> Result<(Vec<u64>, PageReceipt, PageReceipt)> {
    let count_params = selection_parameters(b, true);
    let (value, mut count_receipt) = fetch_request(c, url, Some(&count_params), budget).await?;
    complete_response(&value)?;
    let count = value["count"]
        .as_u64()
        .filter(|n| *n <= MAX_IDS as u64)
        .ok_or(
            "ArcGIS query exceeds 5,000 features or has an invalid count; use a smaller region",
        )? as usize;
    count_receipt.returned = count;
    let params = selection_parameters(b, false);
    let (value, mut receipt) = fetch_request(c, url, Some(&params), budget).await?;
    let ids = parse_ids(&value, layer, count)?;
    receipt.returned = ids.len();
    Ok((ids, receipt, count_receipt))
}
fn coordinates_2d(value: &Value) -> bool {
    let Some(a) = value.as_array() else {
        return false;
    };
    if a.first().is_some_and(Value::is_number) {
        return a.len() == 2 && a.iter().all(|x| x.as_f64().is_some_and(f64::is_finite));
    }
    !a.is_empty() && a.iter().all(coordinates_2d)
}
fn append_batch(value: Value, expected: &[u64], layer: &Layer) -> Result<Vec<Value>> {
    complete_response(&value)?;
    if value["type"] != "FeatureCollection" || value.get("crs").is_some() {
        return Err("ArcGIS must return WGS84 GeoJSON".into());
    }
    let features = value["features"]
        .as_array()
        .ok_or("ArcGIS returned no feature list")?;
    if features.len() != expected.len() {
        return Err("ArcGIS feature batch is incomplete; no partial file was saved".into());
    }
    let fields: BTreeSet<_> = layer.fields.iter().map(|f| f.name.as_str()).collect();
    let mut returned = BTreeMap::new();
    for feature in features {
        let attributes = feature["properties"]
            .as_object()
            .ok_or("ArcGIS feature has no attributes")?;
        let id = attributes
            .get(&layer.object_id_field)
            .and_then(Value::as_u64)
            .filter(|n| *n <= SAFE_ID)
            .ok_or("ArcGIS feature has no safe object ID")?;
        if feature["type"] != "Feature"
            || feature.get("geometry").is_none()
            || feature["id"].as_u64() != Some(id)
            || !expected.contains(&id)
            || attributes
                .keys()
                .map(String::as_str)
                .collect::<BTreeSet<_>>()
                != fields
        {
            return Err(
                "ArcGIS feature identity or fields do not match the requested layer".into(),
            );
        }
        let g = &feature["geometry"];
        let valid_type = match layer.geometry_type.as_str() {
            "esriGeometryPoint" => g["type"] == "Point",
            "esriGeometryMultipoint" => g["type"] == "MultiPoint",
            "esriGeometryPolyline" => g["type"] == "LineString" || g["type"] == "MultiLineString",
            "esriGeometryPolygon" => g["type"] == "Polygon" || g["type"] == "MultiPolygon",
            _ => false,
        };
        if !g.is_null() && (!valid_type || !coordinates_2d(&g["coordinates"])) {
            return Err("ArcGIS returned unsupported or changed geometry".into());
        }
        if returned.insert(id, feature.clone()).is_some() {
            return Err("ArcGIS repeated a feature within a batch".into());
        }
    }
    Ok(returned.into_values().collect())
}
fn ensure_same_layer(before: &Collection, after: &Collection) -> Result<()> {
    let first = before
        .arcgis
        .as_ref()
        .ok_or("Missing ArcGIS layer metadata")?;
    let mut last = after
        .arcgis
        .clone()
        .ok_or("Missing ArcGIS layer metadata")?;
    // The raw response may change ordering or volatile metadata without a
    // change to the field schema or the extraction contract.
    last.metadata_sha256.clone_from(&first.metadata_sha256);
    if before.id != after.id || before.title != after.title || first != &last {
        return Err("ArcGIS layer definition changed while downloading; query again".into());
    }
    Ok(())
}
pub(super) async fn query(
    service: FeatureService,
    selected: Collection,
    request: QueryRequest,
    settings: crate::ProxySettings,
) -> Result<(Value, Provenance, String)> {
    validate_service(&service)?;
    let root = root_url(&service.url)?;
    let url = query_url(&root, &selected.id)?;
    let c = client(&root, &settings).await?;
    let mut budget = vector::MAX_BYTES;
    let (current, layer_receipt) = fetch(&c, &metadata(&root, &selected.id)?, &mut budget).await?;
    let selected = parse_layer(
        &root,
        &current,
        &layer_receipt.sha256,
        &service.arcgis.as_ref().unwrap().copyright_text,
    )?;
    if selected.id != request.collection_id {
        return Err("ArcGIS changed the selected layer identity".into());
    }
    let layer = selected.arcgis.clone().unwrap();
    let page_size = request
        .page_size
        .unwrap_or(PAGE_SIZE)
        .min(layer.max_record_count);
    let at = now();
    let (ids, before_ids, before_count) =
        membership(&c, &url, &layer, request.bounds, &mut budget).await?;
    if ids.len().div_ceil(page_size) > MAX_PAGES {
        return Err(
            "ArcGIS query exceeds 25 batches; use a smaller region or larger batch size".into(),
        );
    }
    let mut pages = Vec::new();
    let mut features = Vec::new();
    for batch in ids.chunks(page_size) {
        let params = batch_parameters(batch);
        let (value, mut receipt) = fetch_request(&c, &url, Some(&params), &mut budget).await?;
        let part = append_batch(value, batch, &layer)?;
        receipt.returned = part.len();
        pages.push(receipt);
        features.extend(part);
    }
    let (after, after_ids, after_count) =
        membership(&c, &url, &layer, request.bounds, &mut budget).await?;
    if after != ids {
        return Err("ArcGIS matching objects changed while downloading; query again".into());
    }
    let (definition, definition_receipt) =
        fetch(&c, &metadata(&root, &selected.id)?, &mut budget).await?;
    let after_layer = parse_layer(
        &root,
        &definition,
        &definition_receipt.sha256,
        &service.arcgis.as_ref().unwrap().copyright_text,
    )?;
    ensure_same_layer(&selected, &after_layer)?;
    let source = Provenance {
        service_url: service.url,
        service_name: service.name,
        collection_id: selected.id,
        collection_title: selected.title.clone(),
        license_links: vec![],
        requested_bounds: request.bounds,
        area_geometry: request.area_geometry,
        requested_at: at,
        pages,
        number_matched: Some(ids.len()),
        feature_count: features.len(),
        selection: "bbox-full-features".into(),
        wfs: None,
        arcgis: Some(Snapshot {
            layer,
            object_ids: ids,
            id_receipts: vec![before_ids, after_ids],
            count_receipts: vec![before_count, after_count],
        }),
    };
    source.validate(features.len())?;
    let data = json!({"type":"FeatureCollection","features":features,"geodSource":source});
    Ok((data, source, selected.title))
}
fn receipt_valid(
    p: &PageReceipt,
    url: &Url,
    params: &Parameters,
    returned: usize,
) -> Result<usize> {
    if p.url != url.as_str()
        || p.parameters.as_ref() != Some(params)
        || p.returned != returned
        || !digest(&p.sha256)
        || p.bytes == 0
        || p.bytes > vector::MAX_BYTES
    {
        return Err("Invalid ArcGIS request receipt".into());
    }
    Ok(p.bytes)
}
pub(super) fn validate_provenance(source: &Provenance, count: usize) -> Result<()> {
    let root = root_url(&source.service_url)?;
    let snapshot = source
        .arcgis
        .as_ref()
        .ok_or("Missing ArcGIS query provenance")?;
    let url = query_url(&root, &source.collection_id)?;
    bounds(source.requested_bounds)?;
    validate_layer(&snapshot.layer)?;
    if source.wfs.is_some()
        || !clean(&source.service_name, 80)
        || !clean(&source.collection_title, 240)
        || source.selection != "bbox-full-features"
        || source.feature_count != count
        || source.number_matched != Some(count)
        || count > MAX_IDS
        || !source.license_links.is_empty()
        || source.pages.len() > MAX_PAGES
        || snapshot.object_ids.len() != count
        || snapshot.id_receipts.len() != 2
        || snapshot.count_receipts.len() != 2
        || snapshot.object_ids.iter().any(|id| *id > SAFE_ID)
        || snapshot.object_ids.windows(2).any(|w| w[0] >= w[1])
        || chrono::DateTime::parse_from_rfc3339(&source.requested_at).is_err()
    {
        return Err("Invalid ArcGIS query provenance".into());
    }
    if let Some(area) = &source.area_geometry {
        validate_area(area, source.requested_bounds)?;
    }
    let mut bytes = 0usize;
    for (receipts, count_mode) in [
        (&snapshot.id_receipts, false),
        (&snapshot.count_receipts, true),
    ] {
        for p in receipts {
            bytes += receipt_valid(
                p,
                &url,
                &selection_parameters(source.requested_bounds, count_mode),
                count,
            )?;
        }
    }
    let mut offset = 0usize;
    for page in &source.pages {
        if page.returned == 0
            || page.returned > PAGE_SIZE.min(snapshot.layer.max_record_count)
            || offset + page.returned > count
        {
            return Err("Invalid ArcGIS batch coverage".into());
        }
        bytes += receipt_valid(
            page,
            &url,
            &batch_parameters(&snapshot.object_ids[offset..offset + page.returned]),
            page.returned,
        )?;
        offset += page.returned;
    }
    if offset != count || bytes > vector::MAX_BYTES {
        return Err("Incomplete ArcGIS query provenance".into());
    }
    Ok(())
}
