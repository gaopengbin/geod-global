use super::*;
use roxmltree::{Document, Node};
use std::collections::BTreeSet;
const WCS: &str = "http://www.opengis.net/wcs/2.0";
const OWS: &str = "http://www.opengis.net/ows/2.0";
const GML: &str = "http://www.opengis.net/gml/3.2";
const COV: &str = "http://www.opengis.net/gmlcov/1.0";
const SWE: &str = "http://www.opengis.net/swe/2.0";
const XLINK: &str = "http://www.w3.org/1999/xlink";

pub(super) fn document(bytes: &[u8]) -> Result<Document<'_>> {
    if bytes.is_empty() || bytes.len() > MAX_XML {
        return Err("WCS XML is empty or exceeds 8 MiB".into());
    }
    let value = std::str::from_utf8(bytes).map_err(|_| "WCS XML must be UTF-8")?;
    if value.contains("<!DOCTYPE") || value.contains("<!ENTITY") {
        return Err("WCS XML DTD and entity declarations are unsupported".into());
    }
    let doc = Document::parse_with_options(
        value,
        roxmltree::ParsingOptions {
            nodes_limit: 200_000,
            ..Default::default()
        },
    )
    .map_err(|_| "WCS returned invalid XML")?;
    if doc.root_element().tag_name().name().contains("Exception") {
        return Err("WCS returned an OGC exception document".into());
    }
    for node in doc.descendants().filter(Node::is_element) {
        if node.ancestors().take(130).count() > 129 {
            return Err("WCS XML nesting exceeds 128 elements".into());
        }
        if let Some(raw) = node.attribute((XLINK, "href")) {
            let url = Url::parse(raw)
                .or_else(|_| Url::parse("https://metadata.invalid/").unwrap().join(raw))
                .map_err(io_error)?;
            if credential_url(&url) {
                return Err("WCS XML contains credentials or signatures and cannot be retained as a public source".into());
            }
        }
    }
    Ok(doc)
}
fn children<'a, 'i>(
    node: Node<'a, 'i>,
    ns: &'static str,
    name: &'static str,
) -> impl Iterator<Item = Node<'a, 'i>> {
    node.children().filter(move |n| {
        n.is_element() && n.tag_name().namespace() == Some(ns) && n.tag_name().name() == name
    })
}
fn optional<'a, 'i>(
    node: Node<'a, 'i>,
    ns: &'static str,
    name: &'static str,
) -> Result<Option<Node<'a, 'i>>> {
    let mut found = children(node, ns, name);
    let first = found.next();
    if found.next().is_some() {
        return Err(format!("WCS XML repeats {name}"));
    }
    Ok(first)
}
fn one<'a, 'i>(node: Node<'a, 'i>, ns: &'static str, name: &'static str) -> Result<Node<'a, 'i>> {
    optional(node, ns, name)?.ok_or_else(|| format!("WCS XML is missing {name}"))
}
fn value(node: Node<'_, '_>, max: usize) -> Result<String> {
    let value = node.text().unwrap_or("").trim();
    if value.len() > max || node.children().any(|n| n.is_element()) {
        return Err("WCS metadata contains oversized or complex text".into());
    }
    Ok(value.into())
}
fn field(node: Node<'_, '_>, ns: &'static str, name: &'static str, max: usize) -> Result<String> {
    optional(node, ns, name)?
        .map(|n| value(n, max))
        .transpose()
        .map(|v| v.unwrap_or_default())
}
fn required(
    node: Node<'_, '_>,
    ns: &'static str,
    name: &'static str,
    max: usize,
) -> Result<String> {
    let v = value(one(node, ns, name)?, max)?;
    if !text(&v, max) {
        return Err(format!("WCS {name} is empty or invalid"));
    }
    Ok(v)
}
fn operation(root: Node<'_, '_>, name: &str, base: &Url) -> Result<String> {
    let ops = one(root, OWS, "OperationsMetadata")?;
    let found = children(ops, OWS, "Operation")
        .filter(|n| n.attribute("name") == Some(name))
        .collect::<Vec<_>>();
    if found.len() != 1 {
        return Err(format!("WCS must advertise one {name} operation"));
    }
    let mut urls = BTreeSet::new();
    for dcp in children(found[0], OWS, "DCP") {
        if let Some(http) = optional(dcp, OWS, "HTTP")? {
            for get in children(http, OWS, "Get") {
                let raw = get
                    .attribute((XLINK, "href"))
                    .ok_or("WCS GET endpoint has no URL")?;
                urls.insert(endpoint(base, raw)?.to_string());
            }
        }
    }
    if urls.len() != 1 {
        return Err(format!(
            "WCS {name} needs one unambiguous public GET endpoint"
        ));
    }
    Ok(urls.into_iter().next().unwrap())
}
pub(super) fn capabilities(
    bytes: &[u8],
    base: &Url,
    name: &str,
    id: &str,
    time: &str,
) -> Result<Connection> {
    let doc = document(bytes)?;
    let root = doc.root_element();
    if root.tag_name().namespace() != Some(WCS)
        || root.tag_name().name() != "Capabilities"
        || root.attribute("version") != Some("2.0.1")
    {
        return Err("Only WCS 2.0.1 capabilities are supported".into());
    }
    let service = one(root, OWS, "ServiceIdentification")?;
    let title = required(service, OWS, "Title", 512)?;
    let formats = children(one(root, WCS, "ServiceMetadata")?, WCS, "formatSupported")
        .map(|n| value(n, 256))
        .collect::<Result<Vec<_>>>()?;
    if formats.len() > 128
        || !formats.iter().any(|s| {
            matches!(
                s.as_str(),
                "image/tiff" | "image/tiff;application=geotiff" | "image/tiff; application=geotiff"
            )
        })
    {
        return Err("WCS must advertise a supported GeoTIFF encoding".into());
    }
    let profiles = children(service, OWS, "Profile")
        .map(|n| value(n, 2048))
        .collect::<Result<Vec<_>>>()?;
    if profiles.len() > 128 {
        return Err("WCS profile list exceeds its limit".into());
    }
    operation(root, "GetCapabilities", base)?;
    let describe_url = operation(root, "DescribeCoverage", base)?;
    let coverage_url = operation(root, "GetCoverage", base)?;
    let mut coverages = Vec::new();
    let mut seen = BTreeSet::new();
    for node in children(one(root, WCS, "Contents")?, WCS, "CoverageSummary") {
        let id = required(node, WCS, "CoverageId", 512)?;
        if !seen.insert(id.clone()) || coverages.len() >= 4096 {
            return Err("WCS coverage identifiers repeat or exceed 4096".into());
        }
        let title = field(node, OWS, "Title", 512)?;
        coverages.push(Coverage {
            title: if title.is_empty() { id.clone() } else { title },
            id,
            subtype: required(node, WCS, "CoverageSubtype", 128)?,
        });
    }
    if coverages.is_empty() {
        return Err("WCS advertises no coverages".into());
    }
    let attribution = optional(root, OWS, "ServiceProvider")?
        .map(|n| field(n, OWS, "ProviderName", 512))
        .transpose()?
        .filter(|s| !s.is_empty());
    let access_constraints = children(service, OWS, "AccessConstraints")
        .map(|n| value(n, 4096))
        .collect::<Result<Vec<_>>>()?
        .join("; ");
    Ok(Connection {
        id: id.into(),
        name: name.into(),
        url: base.to_string(),
        title,
        version: "2.0.1".into(),
        connected_at: time.into(),
        capabilities_sha256: hash(bytes),
        coverages,
        formats,
        profiles,
        access_constraints,
        fees: field(service, OWS, "Fees", 4096)?,
        attribution,
        describe_url,
        coverage_url,
    })
}
fn pair(node: Node<'_, '_>) -> Result<[f64; 2]> {
    let values = value(node, 256)?
        .split_whitespace()
        .map(|s| s.parse::<f64>().map_err(io_error))
        .collect::<Result<Vec<_>>>()?;
    if values.len() != 2 || values.iter().any(|v| !v.is_finite()) {
        return Err("WCS requires finite two-dimensional coordinates".into());
    }
    Ok([values[0], values[1]])
}
fn indices(node: Node<'_, '_>) -> Result<[i64; 2]> {
    let values = value(node, 128)?
        .split_whitespace()
        .map(|s| s.parse::<i64>().map_err(io_error))
        .collect::<Result<Vec<_>>>()?;
    if values.len() != 2 || values.iter().any(|v| v.unsigned_abs() > 1_000_000_000) {
        return Err("WCS requires bounded two-dimensional grid indices".into());
    }
    Ok([values[0], values[1]])
}
fn labels(raw: &str) -> Result<[String; 2]> {
    let labels = raw.split_whitespace().collect::<Vec<_>>();
    if labels.len() != 2
        || labels[0] == labels[1]
        || labels.iter().any(|s| {
            s.is_empty()
                || s.len() > 64
                || !s
                    .bytes()
                    .all(|c| c.is_ascii_alphanumeric() || b"_.-".contains(&c))
        })
    {
        return Err("WCS axis labels must be two distinct simple names".into());
    }
    Ok([labels[0].into(), labels[1].into()])
}
pub(super) fn description(
    bytes: &[u8],
    record: &DescriptionRecord,
    id: &str,
) -> Result<Description> {
    let doc = document(bytes)?;
    let root = doc.root_element();
    if root.tag_name().namespace() != Some(WCS) || root.tag_name().name() != "CoverageDescriptions"
    {
        return Err("WCS response is not CoverageDescriptions".into());
    }
    let coverage = one(root, WCS, "CoverageDescription")?;
    let coverage_id = required(coverage, WCS, "CoverageId", 512)?;
    if coverage_id != record.coverage_id {
        return Err("WCS described a different coverage".into());
    }
    let advertised = record
        .connection
        .coverages
        .iter()
        .find(|c| c.id == coverage_id)
        .ok_or("Coverage is absent from pinned capabilities")?;
    let params = one(coverage, WCS, "ServiceParameters")?;
    if required(params, WCS, "CoverageSubtype", 128)? != "RectifiedGridCoverage"
        || advertised.subtype != "RectifiedGridCoverage"
    {
        return Err("Only WCS RectifiedGridCoverage is supported".into());
    }
    let envelope = one(one(coverage, GML, "boundedBy")?, GML, "Envelope")?;
    if envelope.attribute("srsDimension") != Some("2") {
        return Err("WCS coverage must declare exactly two CRS dimensions".into());
    }
    let declared_crs = envelope
        .attribute("srsName")
        .filter(|s| text(s, 2048))
        .ok_or("WCS envelope CRS is missing")?
        .to_string();
    let (crs, swapped) = grid::crs(&declared_crs)?;
    let axis_labels = labels(
        envelope
            .attribute("axisLabels")
            .ok_or("WCS envelope axis labels are missing")?,
    )?;
    let lower = pair(one(envelope, GML, "lowerCorner")?)?;
    let upper = pair(one(envelope, GML, "upperCorner")?)?;
    if lower[0] >= upper[0] || lower[1] >= upper[1] {
        return Err("WCS envelope is empty or inverted".into());
    }
    let domain = one(coverage, GML, "domainSet")?;
    if domain.children().filter(Node::is_element).count() != 1 {
        return Err("WCS has unsupported domain members".into());
    }
    let grid = one(domain, GML, "RectifiedGrid")?;
    if grid.attribute("dimension") != Some("2") {
        return Err("WCS rectified grid must have two dimensions".into());
    }
    let limits = one(one(grid, GML, "limits")?, GML, "GridEnvelope")?;
    let low = indices(one(limits, GML, "low")?)?;
    let high = indices(one(limits, GML, "high")?)?;
    let grid_axis_labels = labels(&required(grid, GML, "axisLabels", 256)?)?;
    let point = one(one(grid, GML, "origin")?, GML, "Point")?;
    if point.attribute("srsDimension").is_some_and(|s| s != "2")
        || point
            .attribute("srsName")
            .map(grid::crs)
            .transpose()?
            .is_some_and(|v| v != (crs.clone(), swapped))
    {
        return Err("WCS origin uses a different CRS or dimension".into());
    }
    let origin = pair(one(point, GML, "pos")?)?;
    let vectors = children(grid, GML, "offsetVector").collect::<Vec<_>>();
    if vectors.len() != 2 {
        return Err("WCS requires two grid offset vectors".into());
    }
    let mut offsets = [[0.; 2]; 2];
    for (i, node) in vectors.into_iter().enumerate() {
        if node.attribute("srsDimension").is_some_and(|s| s != "2")
            || node
                .attribute("srsName")
                .map(grid::crs)
                .transpose()?
                .is_some_and(|v| v != (crs.clone(), swapped))
        {
            return Err("WCS offset vector has a different CRS or dimension".into());
        }
        offsets[i] = pair(node)?;
    }
    let (width, height, transform, native_bounds) =
        grid::definition(low, high, origin, offsets, lower, upper, swapped)?;
    let range = one(one(coverage, COV, "rangeType")?, SWE, "DataRecord")?;
    let mut fields = Vec::new();
    let mut names = BTreeSet::new();
    for field_node in range.children().filter(Node::is_element) {
        if field_node.tag_name().namespace() != Some(SWE) || field_node.tag_name().name() != "field"
        {
            return Err("WCS range contains unsupported components".into());
        }
        let name = field_node
            .attribute("name")
            .filter(|s| text(s, 512))
            .ok_or("WCS range field has no name")?
            .to_string();
        if !names.insert(name.clone()) || fields.len() >= 16 {
            return Err("WCS range fields repeat or exceed 16".into());
        }
        if field_node.children().filter(Node::is_element).count() != 1 {
            return Err("WCS range field is not a simple quantity".into());
        }
        let quantity = one(field_node, SWE, "Quantity")?;
        let unit = optional(quantity, SWE, "uom")?
            .map(|u| -> Result<String> {
                let s = u
                    .attribute("code")
                    .or_else(|| u.attribute((XLINK, "href")))
                    .ok_or("WCS quantity unit is invalid")?;
                if !text(s, 256) {
                    return Err("WCS quantity unit is too large or invalid".into());
                }
                Ok(s.into())
            })
            .transpose()?;
        let mut nil_values = Vec::new();
        if let Some(nil) = optional(quantity, SWE, "nilValues")? {
            if let Some(list) = optional(nil, SWE, "NilValues")? {
                for n in children(list, SWE, "nilValue") {
                    let value = value(n, 256)?;
                    if value.is_empty() || nil_values.len() >= 32 {
                        return Err("WCS nil declarations are empty or exceed their limit".into());
                    }
                    let reason = n.attribute("reason").map(str::to_owned);
                    if reason.as_ref().is_some_and(|s| !text(s, 2048)) {
                        return Err("WCS nil reason is invalid".into());
                    }
                    nil_values.push(NilValue { value, reason });
                }
            }
        }
        fields.push(Field {
            name,
            description: field(quantity, SWE, "description", 16384)?,
            unit,
            nil_values,
        });
    }
    if fields.is_empty() {
        return Err("WCS range contains no numeric fields".into());
    }
    let mut metadata_links = Vec::new();
    for n in coverage.descendants().filter(Node::is_element) {
        if n.tag_name().namespace() == Some(OWS) && n.tag_name().name() == "Metadata" {
            if let Some(href) = n.attribute((XLINK, "href")) {
                if !text(href, 8192) || metadata_links.len() >= 256 {
                    return Err("WCS metadata links exceed their limit".into());
                }
                metadata_links.push(href.into());
            }
        }
    }
    let title = field(coverage, GML, "name", 512)?;
    let bounds = grid::envelope(native_bounds, &crs, false)?;
    Ok(Description { id: id.into(), connection_id: record.connection.id.clone(), coverage_id, title: if title.is_empty() { advertised.title.clone() } else { title }, service_name: record.connection.name.clone(), crs, declared_crs, axis_labels, grid_axis_labels, width, height, transform, native_bounds, bounds, fields, metadata_links, capabilities_sha256: record.connection.capabilities_sha256.clone(), description_sha256: hash(bytes), description_url: record.description_url.clone(), retrieved_at: record.retrieved_at.clone(), warnings: vec!["Units and nil values are source declarations; they are preserved without inferring calibrated values or physical meaning.".into(), "Sample types and file NoData are verified from the returned TIFF, independently of the coverage's range declarations.".into(), "Service access constraints are not a blanket data licence; retain and consult linked product metadata.".into()] })
}
pub(super) fn fingerprint(bytes: &[u8]) -> Result<String> {
    let doc = document(bytes)?;
    fn visit(node: Node<'_, '_>, out: &mut Vec<serde_json::Value>) {
        if node.is_element() {
            let mut attrs = node
                .attributes()
                .map(|a| (a.namespace().unwrap_or(""), a.name(), a.value()))
                .collect::<Vec<_>>();
            attrs.sort_unstable();
            let mut namespaces = node
                .namespaces()
                .map(|n| (n.name().unwrap_or(""), n.uri()))
                .collect::<Vec<_>>();
            namespaces.sort_unstable();
            out.push(serde_json::json!([
                "element",
                node.tag_name().namespace(),
                node.tag_name().name(),
                attrs,
                namespaces
            ]));
            for child in node.children() {
                visit(child, out);
            }
            out.push(serde_json::json!(["end"]));
        } else if node.is_text() {
            let s = node.text().unwrap_or("").trim();
            if !s.is_empty() {
                out.push(serde_json::json!(["text", s]));
            }
        }
    }
    let mut nodes = Vec::new();
    visit(doc.root_element(), &mut nodes);
    Ok(hash(&serde_json::to_vec(&nodes).map_err(io_error)?))
}
