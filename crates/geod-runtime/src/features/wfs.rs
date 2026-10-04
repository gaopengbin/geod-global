//! Read-only WFS 2.0 with retained source documents and explicit axis handling.
use super::*;
use roxmltree::{Document, Node};
mod gml;
pub mod schema;
#[cfg(test)]
mod tests;
pub use schema::Schema;
const WFS: &str = "http://www.opengis.net/wfs/2.0";
const OWS: &str = "http://www.opengis.net/ows/1.1";
const XLINK: &str = "http://www.w3.org/1999/xlink";
const EPSG4326: &str = "urn:ogc:def:crs:EPSG::4326";
const OUTPUT_CRS84: &str = "urn:ogc:def:crs:OGC:1.3:CRS84";
const MAX_WFS_PAGES: usize = 250;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Format {
    pub id: String,
    pub mime: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Layer {
    pub type_name: String,
    pub namespace: String,
    pub default_crs: String,
    pub other_crs: Vec<String>,
    pub formats: Vec<Format>,
    pub default_format: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ExcludedLayer {
    pub id: String,
    pub title: String,
    pub reason: String,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Service {
    pub version: String,
    pub capabilities_sha256: String,
    pub fees: String,
    pub access_constraints: String,
    pub paging_supported: bool,
    pub excluded_layers: Vec<ExcludedLayer>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Snapshot {
    pub version: String,
    pub layer: Layer,
    pub format: Format,
    pub request_crs: String,
    pub response_crs: String,
    pub capabilities_sha256: String,
    pub schema: Schema,
    pub schema_receipt: PageReceipt,
    pub schema_after_receipt: PageReceipt,
    pub hits_before: PageReceipt,
    pub hits_after: PageReceipt,
    pub matched_count: usize,
    pub paging_supported: bool,
    pub raw_archive_version: u32,
    pub fees: String,
    pub access_constraints: String,
    pub sort_field: Option<String>,
    pub page_size: usize,
    pub verification_pages: Vec<PageReceipt>,
}
#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Archive {
    version: u32,
    schema_xml: String,
    schema_after_xml: String,
    hits_before_xml: String,
    hits_after_xml: String,
    pages: Vec<String>,
    verification_pages: Vec<String>,
}
pub(super) struct Page {
    pub features: Vec<Value>,
    pub number_matched: Option<usize>,
    pub number_returned: usize,
    pub time_stamp: String,
    pub next: Option<String>,
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn xml(raw: &str) -> Result<Document<'_>> {
    if raw.len() > vector::MAX_BYTES || raw.contains("<!DOCTYPE") || raw.contains("<!ENTITY") {
        return Err("WFS XML is too large or contains unsupported document declarations".into());
    }
    let doc = Document::parse(raw).map_err(|_| "Invalid WFS XML")?;
    if doc.descendants().any(|n| {
        n.is_element()
            && matches!(
                n.tag_name().name(),
                "ExceptionReport" | "ServiceExceptionReport" | "Exception" | "ServiceException"
            )
    }) {
        return Err("WFS reported a service exception; no partial file was saved".into());
    }
    Ok(doc)
}
fn children<'a, 'i>(
    n: Node<'a, 'i>,
    ns: &'a str,
    name: &'a str,
) -> impl Iterator<Item = Node<'a, 'i>> {
    n.children().filter(move |c| c.has_tag_name((ns, name)))
}
fn child_text(n: Node<'_, '_>, ns: &str, name: &str) -> String {
    n.children()
        .find(|c| c.has_tag_name((ns, name)))
        .and_then(|c| c.text())
        .unwrap_or("")
        .trim()
        .into()
}
fn limited_text(s: &str, max: usize) -> bool {
    s.len() <= max
        && !s
            .chars()
            .any(|c| c.is_control() && !matches!(c, '\r' | '\n' | '\t'))
}
fn ncname(s: &str) -> bool {
    !s.is_empty()
        && s.len() <= 160
        && s.bytes().enumerate().all(|(i, b)| {
            b.is_ascii_alphabetic()
                || b == b'_'
                || (i > 0 && (b.is_ascii_digit() || b == b'-' || b == b'.'))
        })
}
fn qname(s: &str) -> bool {
    let p = s.split(':').collect::<Vec<_>>();
    (1..=2).contains(&p.len()) && p.iter().all(|s| ncname(s)) && s.len() <= 160
}
fn endpoint(raw: &str, pasted: bool) -> Result<Url> {
    let mut u = public_url(raw)?;
    if let Some(query) = u.query() {
        let mut seen = BTreeSet::new();
        if !pasted
            || query.is_empty()
            || u.query_pairs().any(|(k, v)| {
                !seen.insert(k.to_ascii_lowercase())
                    || match k.to_ascii_lowercase().as_str() {
                        "service" => !v.eq_ignore_ascii_case("WFS"),
                        "request" => !v.eq_ignore_ascii_case("GetCapabilities"),
                        "version" => v != "2.0.0",
                        _ => true,
                    }
            })
        {
            return Err(
                "Enter a WFS endpoint or a WFS 2.0 GetCapabilities URL without credentials".into(),
            );
        }
        u.set_query(None);
    }
    if !pasted && u.as_str() != raw {
        return Err("Invalid canonical WFS endpoint".into());
    }
    Ok(u)
}
fn known_crs(s: &str) -> bool {
    matches!(
        s,
        "EPSG:4326"
            | EPSG4326
            | "urn:x-ogc:def:crs:EPSG:4326"
            | "http://www.opengis.net/def/crs/EPSG/0/4326"
            | CRS84
            | OUTPUT_CRS84
            | "CRS:84"
            | "urn:ogc:def:crs:OGC::CRS84"
    )
}
fn format_id(mime: &str) -> Option<&'static str> {
    match mime
        .to_ascii_lowercase()
        .split_whitespace()
        .collect::<String>()
        .as_str()
    {
        "application/gml+xml;version=3.2" | "text/xml;subtype=gml/3.2.1" | "gml32" => Some("gml32"),
        "application/json"
        | "application/geo+json"
        | "application/json;subtype=geojson"
        | "json" => Some("geojson"),
        _ => None,
    }
}
fn validate_layer(l: &Layer) -> Result<()> {
    if !qname(&l.type_name)
        || !clean(&l.namespace, 2048)
        || !clean(&l.default_crs, 256)
        || l.other_crs.len() > 64
        || l.other_crs.iter().any(|v| !clean(v, 256))
        || l.formats.is_empty()
        || l.formats.len() > 2
    {
        return Err("Unsupported WFS feature type metadata".into());
    }
    let mut seen = BTreeSet::new();
    for f in &l.formats {
        if !clean(&f.mime, 120) || format_id(&f.mime) != Some(f.id.as_str()) || !seen.insert(&f.id)
        {
            return Err("Invalid WFS response formats".into());
        }
    }
    let preferred = if l.formats.iter().any(|f| f.id == "gml32") {
        "gml32"
    } else {
        "geojson"
    };
    if l.default_format != preferred {
        return Err("Invalid WFS default response format".into());
    }
    Ok(())
}
fn operation_url(root: &Url, operation: &str) -> Url {
    let mut u = root.clone();
    u.query_pairs_mut()
        .append_pair("service", "WFS")
        .append_pair("version", "2.0.0")
        .append_pair("request", operation);
    u
}
fn schema_url(root: &Url, layer: &Layer) -> Url {
    let mut u = operation_url(root, "DescribeFeatureType");
    u.query_pairs_mut()
        .append_pair("typeNames", &layer.type_name);
    u
}
fn feature_url(
    root: &Url,
    layer: &Layer,
    b: [f64; 4],
    format: Option<&Format>,
    start: usize,
    count: usize,
    sort: Option<&str>,
) -> Url {
    let mut u = operation_url(root, "GetFeature");
    let bbox = format!("{},{},{},{},{}", b[1], b[0], b[3], b[2], EPSG4326);
    u.query_pairs_mut()
        .append_pair("typeNames", &layer.type_name)
        .append_pair("bbox", &bbox);
    if let Some(f) = format {
        u.query_pairs_mut()
            .append_pair("outputFormat", &f.mime)
            .append_pair("srsName", EPSG4326)
            .append_pair("startIndex", &start.to_string())
            .append_pair("count", &count.to_string());
        if let Some(field) = sort {
            u.query_pairs_mut()
                .append_pair("sortBy", &format!("{field} A"));
        }
    } else {
        u.query_pairs_mut().append_pair("resultType", "hits");
    }
    u
}
async fn fetch_text(
    c: &reqwest::Client,
    u: &Url,
    budget: &mut usize,
) -> Result<(String, PageReceipt)> {
    let r=c.get(u.clone()).header("Accept","application/xml, application/gml+xml, application/geo+json, application/json, text/xml").send().await.map_err(|_|"Cannot reach WFS; check the endpoint and proxy settings")?;
    if !r.status().is_success() {
        return Err(format!(
            "WFS returned HTTP {}; no partial file was saved",
            r.status().as_u16()
        ));
    }
    if r.content_length().is_some_and(|n| n > *budget as u64) {
        return Err("WFS response exceeds the extraction size limit".into());
    }
    let mut data = Vec::new();
    let mut stream = r.bytes_stream();
    while let Some(part) = stream.next().await {
        let part = part.map_err(|_| "WFS response was interrupted")?;
        if data.len() + part.len() > *budget {
            return Err("WFS response exceeds the extraction size limit".into());
        }
        data.extend_from_slice(&part);
    }
    if data.is_empty() {
        return Err("WFS returned an empty response".into());
    }
    *budget -= data.len();
    let receipt = PageReceipt {
        url: u.to_string(),
        sha256: hash(&data),
        bytes: data.len(),
        returned: 0,
        parameters: None,
    };
    let text = String::from_utf8(data).map_err(|_| "WFS response must use UTF-8")?;
    Ok((text, receipt))
}
fn advertised_operation(root: &Url, n: Node<'_, '_>) -> bool {
    n.descendants()
        .filter(|v| v.has_tag_name((OWS, "Get")))
        .filter_map(|v| v.attribute((XLINK, "href")))
        .any(|raw| {
            Url::parse(raw).is_ok_and(|mut u| {
                // Use only the user-selected endpoint. An HTTP advertisement may
                // describe that same endpoint behind TLS, but is never requested.
                if u.scheme() == "http" {
                    let _ = u.set_scheme("https");
                }
                let allowed = u.query_pairs().all(|(k, v)| {
                    k.eq_ignore_ascii_case("service") && v.eq_ignore_ascii_case("WFS")
                });
                u.set_query(None);
                allowed && u == *root
            })
        })
}
fn parse_capabilities(raw: &str, root: &Url, name: &str) -> Result<FeatureService> {
    let doc = xml(raw)?;
    let top = doc.root_element();
    if !top.has_tag_name((WFS, "WFS_Capabilities")) || top.attribute("version") != Some("2.0.0") {
        return Err("The service must advertise WFS 2.0.0".into());
    }
    let ops = top
        .children()
        .find(|n| n.has_tag_name((OWS, "OperationsMetadata")))
        .ok_or("WFS has no operations metadata")?;
    for op in ["GetCapabilities", "DescribeFeatureType", "GetFeature"] {
        if !ops.children().any(|n| {
            n.has_tag_name((OWS, "Operation"))
                && n.attribute("name") == Some(op)
                && advertised_operation(root, n)
        }) {
            return Err(format!(
                "WFS {op} must support GET at the configured endpoint"
            ));
        }
    }
    let paging = ops.descendants().any(|n| {
        n.has_tag_name((OWS, "Constraint"))
            && n.attribute("name") == Some("ImplementsResultPaging")
            && child_text(n, OWS, "DefaultValue").eq_ignore_ascii_case("TRUE")
    });
    let get = ops
        .children()
        .find(|n| n.has_tag_name((OWS, "Operation")) && n.attribute("name") == Some("GetFeature"))
        .unwrap();
    let global_formats = get
        .children()
        .filter(|n| {
            n.has_tag_name((OWS, "Parameter")) && n.attribute("name") == Some("outputFormat")
        })
        .flat_map(|n| n.descendants())
        .filter(|n| n.has_tag_name((OWS, "Value")))
        .filter_map(|n| n.text())
        .map(str::trim)
        .collect::<Vec<_>>();
    let ident = top
        .children()
        .find(|n| n.has_tag_name((OWS, "ServiceIdentification")))
        .ok_or("WFS has no service identity")?;
    let title = child_text(ident, OWS, "Title");
    let fees = child_text(ident, OWS, "Fees");
    let access = children(ident, OWS, "AccessConstraints")
        .filter_map(|n| n.text())
        .collect::<Vec<_>>()
        .join("\n");
    if !limited_text(&fees, 16384) || !limited_text(&access, 16384) {
        return Err("Invalid WFS access declarations".into());
    }
    let mut collections = Vec::new();
    let mut excluded = Vec::new();
    let mut seen = BTreeSet::new();
    for n in top
        .children()
        .filter(|n| n.has_tag_name((WFS, "FeatureTypeList")))
        .flat_map(|n| children(n, WFS, "FeatureType"))
    {
        if collections.len() + excluded.len() >= 512 {
            return Err("WFS advertises more than 512 feature types".into());
        }
        let id = child_text(n, WFS, "Name");
        let title = child_text(n, WFS, "Title");
        if !qname(&id) || !seen.insert(id.clone()) {
            return Err("WFS type names are invalid or duplicated".into());
        }
        let title = if clean(&title, 240) {
            title
        } else {
            id.clone()
        };
        let descriptor = (|| {
            let name_node = n
                .children()
                .find(|n| n.has_tag_name((WFS, "Name")))
                .ok_or("WFS feature type has no name")?;
            let prefix = id.split_once(':').map(|(p, _)| p);
            let namespace = name_node
                .lookup_namespace_uri(prefix)
                .ok_or("WFS feature type has an unresolved namespace")?
                .to_string();
            let default_crs = child_text(n, WFS, "DefaultCRS");
            let other_crs = children(n, WFS, "OtherCRS")
                .filter_map(|n| n.text())
                .map(|s| s.trim().to_string())
                .collect();
            let local = n
                .children()
                .filter(|n| n.has_tag_name((WFS, "OutputFormats")))
                .flat_map(|n| children(n, WFS, "Format"))
                .filter_map(|n| n.text())
                .map(str::trim)
                .collect::<Vec<_>>();
            let available = if local.is_empty() {
                &global_formats
            } else {
                &local
            };
            let mut formats = Vec::new();
            for mime in available {
                if let Some(id) = format_id(mime) {
                    if !formats.iter().any(|f: &Format| f.id == id) {
                        formats.push(Format {
                            id: id.into(),
                            mime: (*mime).into(),
                        });
                    }
                }
            }
            let default_format = if formats.iter().any(|f| f.id == "gml32") {
                "gml32"
            } else {
                "geojson"
            }
            .into();
            let layer = Layer {
                type_name: id.clone(),
                namespace,
                default_crs,
                other_crs,
                formats,
                default_format,
            };
            validate_layer(&layer)?;
            Ok::<_, String>(layer)
        })();
        match descriptor {
            Ok(layer) => collections.push(Collection {
                id: id.clone(),
                title,
                description: child_text(n, WFS, "Abstract").chars().take(16000).collect(),
                items_url: root.to_string(),
                license_links: vec![],
                arcgis: None,
                wfs: Some(layer),
            }),
            Err(reason) => excluded.push(ExcludedLayer { id, title, reason }),
        }
    }
    let result = FeatureService {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        title: if clean(&title, 240) {
            title
        } else {
            name.into()
        },
        url: root.to_string(),
        connected_at: now(),
        collections,
        arcgis: None,
        overpass: None,
        wfs: Some(Service {
            version: "2.0.0".into(),
            capabilities_sha256: hash(raw.as_bytes()),
            fees,
            access_constraints: access,
            paging_supported: paging,
            excluded_layers: excluded,
        }),
    };
    validate_service(&result)?;
    Ok(result)
}
pub(super) async fn discover(
    requested: ConnectRequest,
    settings: crate::ProxySettings,
) -> Result<FeatureService> {
    let name = requested.name.trim();
    if !clean(name, 80) {
        return Err("Enter a data service name of 1–80 characters".into());
    }
    let root = endpoint(requested.url.trim(), true)?;
    let c = client(&root, &settings).await?;
    let mut budget = MAX_METADATA;
    let (raw, _) = fetch_text(&c, &operation_url(&root, "GetCapabilities"), &mut budget).await?;
    parse_capabilities(&raw, &root, name)
}
pub(super) fn validate_service(s: &FeatureService) -> Result<()> {
    let root = endpoint(&s.url, false)?;
    let m = s.wfs.as_ref().ok_or("Missing WFS service metadata")?;
    if s.arcgis.is_some()
        || s.overpass.is_some()
        || m.version != "2.0.0"
        || !digest(&m.capabilities_sha256)
        || !limited_text(&m.fees, 16384)
        || !limited_text(&m.access_constraints, 16384)
        || s.collections.len() + m.excluded_layers.len() > 512
    {
        return Err("Invalid saved WFS service".into());
    }
    let mut ids = BTreeSet::new();
    for c in &s.collections {
        let l = c.wfs.as_ref().ok_or("Missing WFS feature type metadata")?;
        validate_layer(l)?;
        if !ids.insert(c.id.clone())
            || c.id != l.type_name
            || !clean(&c.title, 240)
            || !limited_text(&c.description, 16384)
            || c.items_url != root.as_str()
            || c.arcgis.is_some()
            || !c.license_links.is_empty()
        {
            return Err("Invalid saved WFS collection".into());
        }
    }
    for e in &m.excluded_layers {
        if !qname(&e.id)
            || !clean(&e.title, 240)
            || !clean(&e.reason, 500)
            || !ids.insert(e.id.clone())
        {
            return Err("Invalid excluded WFS feature type".into());
        }
    }
    Ok(())
}

fn sort_field(s: &Schema) -> Option<String> {
    s.fields
        .iter()
        .find(|f| {
            !f.optional
                && !f.nullable
                && ["id", "fid", "objectid"].contains(&f.name.to_ascii_lowercase().as_str())
                && matches!(
                    f.field_type.as_str(),
                    "integer"
                        | "long"
                        | "int"
                        | "short"
                        | "byte"
                        | "unsignedLong"
                        | "unsignedInt"
                        | "unsignedShort"
                        | "unsignedByte"
                        | "positiveInteger"
                        | "nonNegativeInteger"
                        | "negativeInteger"
                        | "nonPositiveInteger"
                )
        })
        .map(|f| f.name.clone())
}
fn schema_equivalent(a: &Schema, b: &Schema) -> bool {
    let mut a = a.clone();
    let mut b = b.clone();
    a.sha256.clear();
    b.sha256.clear();
    a == b
}
fn feature_identity(feature: &Value) -> Result<String> {
    match &feature["id"] {
        Value::String(s) if clean(s,512)=>Ok(format!("s:{s}")),
        Value::Number(n) if n.as_i64().is_some_and(|n|n.unsigned_abs()<=9_007_199_254_740_991)=>Ok(format!("n:{n}")),
        _=>Err("WFS response has no usable feature identifier; choose GML 3.2 if the JSON format omits IDs".into()),
    }
}
fn validate_json_geometry(
    value: &Value,
    expected: &str,
    nullable: bool,
    depth: usize,
) -> Result<()> {
    if value.is_null() {
        return if nullable {
            Ok(())
        } else {
            Err("WFS returned a null required geometry".into())
        };
    }
    if depth > 8 || !value.is_object() || value.get("crs").is_some() {
        return Err("Unsupported WFS JSON geometry".into());
    }
    let kind = value["type"]
        .as_str()
        .ok_or("WFS JSON has no geometry type")?;
    if expected != "Geometry" && kind != expected {
        return Err("WFS geometry differs from DescribeFeatureType".into());
    }
    fn coords(v: &Value, level: usize) -> Result<()> {
        if level > 5 {
            return Err("WFS JSON coordinate nesting exceeds its limit".into());
        }
        let a = v
            .as_array()
            .filter(|a| !a.is_empty())
            .ok_or("WFS geometry has no coordinates")?;
        if a[0].is_number() {
            if a.len() != 2
                || a.iter().any(|n| n.as_f64().is_none_or(|n| !n.is_finite()))
                || !(-180.0..=180.0).contains(&a[0].as_f64().unwrap())
                || !(-90.0..=90.0).contains(&a[1].as_f64().unwrap())
            {
                return Err(
                    "WFS JSON must contain two-dimensional WGS84 longitude/latitude coordinates"
                        .into(),
                );
            }
        } else {
            for v in a {
                coords(v, level + 1)?;
            }
        }
        Ok(())
    }
    if kind == "GeometryCollection" {
        for g in value["geometries"]
            .as_array()
            .filter(|v| !v.is_empty())
            .ok_or("WFS geometry collection is empty")?
        {
            validate_json_geometry(g, "Geometry", false, depth + 1)?;
        }
    } else if [
        "Point",
        "LineString",
        "Polygon",
        "MultiPoint",
        "MultiLineString",
        "MultiPolygon",
    ]
    .contains(&kind)
    {
        coords(&value["coordinates"], 0)?;
    } else {
        return Err("Unsupported WFS geometry type".into());
    }
    Ok(())
}
fn json_page(raw: &str, schema: &Schema) -> Result<Page> {
    let value: Value = serde_json::from_str(raw).map_err(|_| "WFS did not return valid GeoJSON")?;
    vector::validate_integer_precision(&value)?;
    if value["type"] != "FeatureCollection" || value.get("error").is_some() {
        return Err("WFS must return a GeoJSON FeatureCollection".into());
    }
    if let Some(crs) = value.get("crs") {
        if crs["type"] != "name" || !crs["properties"]["name"].as_str().is_some_and(known_crs) {
            return Err("WFS JSON returned an unexpected coordinate reference".into());
        }
    }
    let features = value["features"]
        .as_array()
        .filter(|v| v.len() <= vector::MAX_FEATURES)
        .ok_or("Invalid WFS JSON features array")?
        .clone();
    let mut matched = None;
    for key in ["numberMatched", "totalFeatures"] {
        if let Some(n) = value.get(key).filter(|v| *v != "unknown") {
            let n = n
                .as_u64()
                .filter(|n| *n <= vector::MAX_FEATURES as u64)
                .ok_or("WFS JSON matching count is invalid or too large")?
                as usize;
            if matched.is_some_and(|m| m != n) {
                return Err("WFS JSON matching counts disagree".into());
            }
            matched = Some(n);
        }
    }
    if value
        .get("numberReturned")
        .is_some_and(|n| n.as_u64() != Some(features.len() as u64))
    {
        return Err("WFS JSON returned an inconsistent feature count".into());
    }
    for feature in &features {
        if feature["type"] != "Feature"
            || feature.get("geometry").is_none()
            || feature.get("crs").is_some()
        {
            return Err("Invalid WFS JSON feature".into());
        }
        feature_identity(feature)?;
        schema::validate_properties(&feature["properties"], schema)?;
        validate_json_geometry(
            &feature["geometry"],
            &schema.geometry_type,
            schema.geometry_nullable || schema.geometry_optional,
            0,
        )?;
    }
    Ok(Page {
        number_returned: features.len(),
        features,
        number_matched: matched,
        time_stamp: String::new(),
        next: None,
    })
}
fn parse_page(raw: &str, schema: &Schema, format: &Format) -> Result<Page> {
    let page = if format.id == "gml32" {
        let page = gml::parse_page(raw, schema)?;
        // The transport records the requested EPSG:4326 response CRS. The
        // general GML reader also understands CRS84, but a server must not
        // silently change this extraction's declared response coordinate order.
        let doc = schema::document(raw)?;
        for position in doc.descendants().filter(|node| {
            node.has_tag_name((schema::GML, "pos")) || node.has_tag_name((schema::GML, "posList"))
        }) {
            if !matches!(
                position
                    .ancestors()
                    .find_map(|node| node.attribute("srsName")),
                Some(EPSG4326 | "http://www.opengis.net/def/crs/EPSG/0/4326")
            ) {
                return Err("WFS GML changed the requested EPSG:4326 response CRS".into());
            }
        }
        page
    } else {
        json_page(raw, schema)?
    };
    if !page.time_stamp.is_empty() && !gml::timestamp_valid(&page.time_stamp) {
        return Err("WFS returned an invalid response timestamp".into());
    }
    // next is retained in the raw document. Fetches use fixed standard KVP
    // parameters at the configured endpoint, never an advertised remote link.
    if page.next.as_ref().is_some_and(|n| n.len() > 8192) {
        return Err("WFS pagination link is too long".into());
    }
    Ok(page)
}
fn hits(raw: &str, schema: &Schema) -> Result<usize> {
    let p = gml::parse_page(raw, schema)?;
    if p.number_returned != 0 || !p.features.is_empty() {
        return Err("WFS hits response unexpectedly contains feature data".into());
    }
    p.number_matched
        .ok_or("WFS must provide a numeric matching count before extraction".into())
}
#[derive(Default)]
struct Assembly {
    features: Vec<Value>,
    identities: BTreeSet<String>,
    sort_keys: BTreeSet<i128>,
    previous_key: Option<i128>,
}
fn key_value(feature: &Value, field: &str) -> Result<i128> {
    let v = &feature["properties"][field];
    let n = if let Some(s) = v.as_str() {
        s.to_string()
    } else {
        v.to_string()
    };
    n.parse::<i128>()
        .map_err(|_| "WFS ordering field must contain bounded integers".into())
}
impl Assembly {
    fn append(
        &mut self,
        page: Page,
        expected: usize,
        maximum: usize,
        sort: Option<&str>,
    ) -> Result<usize> {
        if page.number_matched.is_some_and(|n| n != expected)
            || page.number_returned != page.features.len()
            || page.features.is_empty()
            || page.features.len() > maximum
            || self.features.len() + page.features.len() > expected
        {
            return Err("WFS pagination counts changed or the result is incomplete".into());
        }
        let returned = page.features.len();
        for feature in page.features {
            if !self.identities.insert(feature_identity(&feature)?) {
                return Err("WFS repeated a feature ID during pagination".into());
            }
            if let Some(field) = sort {
                let key = key_value(&feature, field)?;
                if !self.sort_keys.insert(key) || self.previous_key.is_some_and(|n| key <= n) {
                    return Err("WFS did not return a unique ascending ordering field".into());
                }
                self.previous_key = Some(key);
            }
            self.features.push(feature);
        }
        Ok(returned)
    }
}
fn compare_passes(first: &[Value], second: &[Value], sort: Option<&str>) -> Result<()> {
    if first.len() != second.len() {
        return Err("WFS changed during the consistency check; no file was saved".into());
    }
    for (a, b) in first.iter().zip(second) {
        let equal = if let Some(field) = sort {
            key_value(a, field)? == key_value(b, field)?
                && a["properties"] == b["properties"]
                && a["geometry"] == b["geometry"]
        } else {
            a == b
        };
        if !equal {
            return Err("WFS feature values or membership changed during the consistency check; query again".into());
        }
    }
    Ok(())
}
fn check_receipt(p: &PageReceipt, expected: &Url, returned: usize) -> Result<()> {
    if p.url != expected.as_str()
        || !digest(&p.sha256)
        || p.bytes == 0
        || p.bytes > vector::MAX_BYTES
        || p.returned != returned
        || p.parameters.is_some()
    {
        return Err("Invalid WFS request receipt".into());
    }
    Ok(())
}
pub(super) fn validate_provenance(source: &Provenance, count: usize) -> Result<()> {
    let s = source.wfs.as_ref().ok_or("Missing WFS query provenance")?;
    let root = endpoint(&source.service_url, false)?;
    bounds(source.requested_bounds)?;
    validate_layer(&s.layer)?;
    schema::validate_schema(&s.schema)?;
    if source.arcgis.is_some()
        || s.version != "2.0.0"
        || s.raw_archive_version != 1
        || s.request_crs != EPSG4326
        || s.response_crs
            != if s.format.id == "gml32" {
                EPSG4326
            } else {
                OUTPUT_CRS84
            }
        || !s.layer.formats.contains(&s.format)
        || !digest(&s.capabilities_sha256)
        || !clean(&source.service_name, 80)
        || !clean(&source.collection_title, 240)
        || source.collection_id != s.layer.type_name
        || s.schema.namespace != s.layer.namespace
        || s.schema.element_name != s.layer.type_name.rsplit(':').next().unwrap()
        || source.selection != "wfs-bbox-full-features"
        || source.feature_count != count
        || source.number_matched != Some(count)
        || s.matched_count != count
        || count > vector::MAX_FEATURES
        || !(1..=PAGE_SIZE).contains(&s.page_size)
        || s.sort_field != sort_field(&s.schema)
        || source.pages.len() > MAX_WFS_PAGES
        || s.verification_pages.len() != source.pages.len()
        || !source.license_links.is_empty()
        || !limited_text(&s.fees, 16384)
        || !limited_text(&s.access_constraints, 16384)
        || chrono::DateTime::parse_from_rfc3339(&source.requested_at).is_err()
    {
        return Err("Invalid WFS snapshot provenance".into());
    }
    if let Some(area) = &source.area_geometry {
        validate_area(area, source.requested_bounds)?;
    }
    let su = schema_url(&root, &s.layer);
    let hu = feature_url(&root, &s.layer, source.requested_bounds, None, 0, 0, None);
    for p in [&s.schema_receipt, &s.schema_after_receipt] {
        check_receipt(p, &su, 0)?;
    }
    for p in [&s.hits_before, &s.hits_after] {
        check_receipt(p, &hu, 0)?;
    }
    if s.schema_receipt.sha256 != s.schema.sha256 {
        return Err("WFS schema checksum differs from its receipt".into());
    }
    let mut total = s.schema_receipt.bytes
        + s.schema_after_receipt.bytes
        + s.hits_before.bytes
        + s.hits_after.bytes;
    let mut start = 0;
    for (p, v) in source.pages.iter().zip(&s.verification_pages) {
        let u = feature_url(
            &root,
            &s.layer,
            source.requested_bounds,
            Some(&s.format),
            start,
            s.page_size,
            s.sort_field.as_deref(),
        );
        if p.returned == 0 || p.returned > s.page_size {
            return Err("Invalid WFS page feature count".into());
        }
        check_receipt(p, &u, p.returned)?;
        check_receipt(v, &u, p.returned)?;
        start += p.returned;
        total += p.bytes + v.bytes;
    }
    if start != count
        || total > vector::MAX_BYTES
        || (!s.paging_supported && source.pages.len() > 1)
    {
        return Err("WFS snapshot page totals are incomplete".into());
    }
    Ok(())
}
fn document_matches(raw: &str, receipt: &PageReceipt) -> Result<()> {
    if raw.len() != receipt.bytes || hash(raw.as_bytes()) != receipt.sha256 {
        return Err("WFS source document differs from its recorded receipt".into());
    }
    Ok(())
}
pub(crate) fn decode_bundle(raw: &Value, source: &Provenance) -> Result<Value> {
    source.validate(source.feature_count)?;
    let s = source.wfs.as_ref().ok_or("Not a WFS snapshot")?;
    let archive: Archive =
        serde_json::from_value(raw.clone()).map_err(|_| "Invalid WFS source archive")?;
    if archive.version != 1
        || archive.pages.len() != source.pages.len()
        || archive.verification_pages.len() != s.verification_pages.len()
    {
        return Err("WFS source archive is incomplete".into());
    }
    for (raw, r) in [
        (&archive.schema_xml, &s.schema_receipt),
        (&archive.schema_after_xml, &s.schema_after_receipt),
        (&archive.hits_before_xml, &s.hits_before),
        (&archive.hits_after_xml, &s.hits_after),
    ] {
        document_matches(raw, r)?;
    }
    let schema = schema::parse_schema(&archive.schema_xml, &s.layer.type_name, &s.layer.namespace)?;
    let after = schema::parse_schema(
        &archive.schema_after_xml,
        &s.layer.type_name,
        &s.layer.namespace,
    )?;
    if schema != s.schema
        || !schema_equivalent(&schema, &after)
        || schema::definition_fingerprint(&archive.schema_xml)?
            != schema::definition_fingerprint(&archive.schema_after_xml)?
        || hits(&archive.hits_before_xml, &schema)? != s.matched_count
        || hits(&archive.hits_after_xml, &schema)? != s.matched_count
    {
        return Err("WFS source schema or matching count changed".into());
    }
    let mut passes = Vec::new();
    for (texts, receipts) in [
        (&archive.pages, &source.pages),
        (&archive.verification_pages, &s.verification_pages),
    ] {
        let mut assembled = Assembly::default();
        for (text, receipt) in texts.iter().zip(receipts) {
            document_matches(text, receipt)?;
            let n = assembled.append(
                parse_page(text, &schema, &s.format)?,
                s.matched_count,
                s.page_size,
                s.sort_field.as_deref(),
            )?;
            if n != receipt.returned {
                return Err("WFS archived page differs from its feature receipt".into());
            }
        }
        if assembled.features.len() != s.matched_count {
            return Err("WFS archive omits matching features".into());
        }
        passes.push(assembled.features);
    }
    compare_passes(&passes[0], &passes[1], s.sort_field.as_deref())?;
    Ok(json!({"type":"FeatureCollection","features":passes.remove(0),"geodSource":source}))
}
struct QueryContext<'a> {
    client: &'a reqwest::Client,
    root: &'a Url,
    layer: &'a Layer,
    bounds: [f64; 4],
    format: &'a Format,
    schema: &'a Schema,
    matched: usize,
    page_size: usize,
    paging: bool,
}
async fn fetch_pass(
    context: &QueryContext<'_>,
    budget: &mut usize,
) -> Result<(Vec<String>, Vec<PageReceipt>, Vec<Value>)> {
    let QueryContext {
        client: c,
        root,
        layer,
        bounds,
        format,
        schema,
        matched,
        page_size,
        paging,
    } = *context;
    let mut texts = Vec::new();
    let mut receipts = Vec::new();
    let mut assembled = Assembly::default();
    let sort = sort_field(schema);
    while assembled.features.len() < matched {
        if receipts.len() >= MAX_WFS_PAGES || (!paging && !receipts.is_empty()) {
            return Err(
                "WFS cannot complete this region with its supported paging; choose a smaller area"
                    .into(),
            );
        }
        let url = feature_url(
            root,
            layer,
            bounds,
            Some(format),
            assembled.features.len(),
            page_size,
            sort.as_deref(),
        );
        let (raw, mut receipt) = fetch_text(c, &url, budget).await?;
        receipt.returned = assembled.append(
            parse_page(&raw, schema, format)?,
            matched,
            page_size,
            sort.as_deref(),
        )?;
        texts.push(raw);
        receipts.push(receipt);
    }
    Ok((texts, receipts, assembled.features))
}
pub(super) async fn query(
    service: FeatureService,
    collection: Collection,
    request: QueryRequest,
    settings: crate::ProxySettings,
) -> Result<(Vec<u8>, Provenance)> {
    validate_service(&service)?;
    let root = endpoint(&service.url, false)?;
    let c = client(&root, &settings).await?;
    let meta = service.wfs.as_ref().unwrap();
    let layer = collection.wfs.as_ref().ok_or("Not a WFS collection")?;
    let format = layer
        .formats
        .iter()
        .find(|f| {
            f.id == request
                .response_format
                .as_deref()
                .unwrap_or(&layer.default_format)
        })
        .ok_or("Choose an advertised WFS response format")?
        .clone();
    let page_size = request.page_size.unwrap_or(PAGE_SIZE);
    let mut budget = vector::MAX_BYTES;
    let at = now();
    let (schema_xml, schema_receipt) =
        fetch_text(&c, &schema_url(&root, layer), &mut budget).await?;
    let schema = schema::parse_schema(&schema_xml, &layer.type_name, &layer.namespace)?;
    let hu = feature_url(&root, layer, request.bounds, None, 0, 0, None);
    let (hits_before_xml, hits_before) = fetch_text(&c, &hu, &mut budget).await?;
    let matched = hits(&hits_before_xml, &schema)?;
    if matched > vector::MAX_FEATURES || matched > page_size * MAX_WFS_PAGES {
        return Err(
            "WFS query exceeds 50,000 features or its page limit; choose a smaller area".into(),
        );
    }
    let context = QueryContext {
        client: &c,
        root: &root,
        layer,
        bounds: request.bounds,
        format: &format,
        schema: &schema,
        matched,
        page_size,
        paging: meta.paging_supported,
    };
    let (pages, receipts, first) = fetch_pass(&context, &mut budget).await?;
    let (verification_pages, verification_receipts, second) =
        fetch_pass(&context, &mut budget).await?;
    compare_passes(&first, &second, sort_field(&schema).as_deref())?;
    if receipts.len() != verification_receipts.len()
        || receipts
            .iter()
            .zip(&verification_receipts)
            .any(|(a, b)| a.returned != b.returned || a.url != b.url)
    {
        return Err("WFS pagination changed during verification".into());
    }
    let (hits_after_xml, hits_after) = fetch_text(&c, &hu, &mut budget).await?;
    let (schema_after_xml, schema_after_receipt) =
        fetch_text(&c, &schema_url(&root, layer), &mut budget).await?;
    if hits(&hits_after_xml, &schema)? != matched
        || schema::definition_fingerprint(&schema_xml)?
            != schema::definition_fingerprint(&schema_after_xml)?
        || !schema_equivalent(
            &schema,
            &schema::parse_schema(&schema_after_xml, &layer.type_name, &layer.namespace)?,
        )
    {
        return Err("WFS schema or matching count changed during extraction".into());
    }
    let snapshot = Snapshot {
        version: "2.0.0".into(),
        layer: layer.clone(),
        format: format.clone(),
        request_crs: EPSG4326.into(),
        response_crs: if format.id == "gml32" {
            EPSG4326
        } else {
            OUTPUT_CRS84
        }
        .into(),
        capabilities_sha256: meta.capabilities_sha256.clone(),
        sort_field: sort_field(&schema),
        page_size,
        schema,
        schema_receipt,
        schema_after_receipt,
        hits_before,
        hits_after,
        matched_count: matched,
        paging_supported: meta.paging_supported,
        raw_archive_version: 1,
        fees: meta.fees.clone(),
        access_constraints: meta.access_constraints.clone(),
        verification_pages: verification_receipts,
    };
    let source = Provenance {
        service_url: service.url,
        service_name: service.name,
        collection_id: collection.id,
        collection_title: collection.title,
        license_links: vec![],
        requested_bounds: request.bounds,
        area_geometry: request.area_geometry,
        requested_at: at,
        pages: receipts,
        number_matched: Some(matched),
        feature_count: matched,
        selection: "wfs-bbox-full-features".into(),
        arcgis: None,
        wfs: Some(snapshot),
    };
    source.validate(matched)?;
    let archive = Archive {
        version: 1,
        schema_xml,
        schema_after_xml,
        hits_before_xml,
        hits_after_xml,
        pages,
        verification_pages,
    };
    let bytes = serde_json::to_vec(&archive).map_err(io_error)?;
    if bytes.len() > vector::MAX_BYTES {
        return Err("WFS source archive exceeds 20 MiB; choose a smaller region".into());
    }
    Ok((bytes, source))
}
