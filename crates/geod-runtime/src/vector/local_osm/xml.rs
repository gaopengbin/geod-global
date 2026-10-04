use super::*;
use roxmltree::{Document, Node, ParsingOptions};

fn attr<'a>(node: Node<'a, 'a>, key: &str) -> Result<&'a str> {
    node.attribute(key)
        .ok_or_else(|| format!("OSM XML {} requires {key}", node.tag_name().name()))
}
fn number(node: Node<'_, '_>, key: &str, positive: bool) -> Result<i64> {
    integer(
        attr(node, key)?
            .parse()
            .map_err(|_| format!("OSM XML has invalid {key}"))?,
        positive,
    )
}
fn coordinate(node: Node<'_, '_>, key: &str) -> Result<f64> {
    let value: f64 = attr(node, key)?
        .parse()
        .map_err(|_| format!("OSM XML has invalid {key}"))?;
    let limit = if key.contains("lon") { 180. } else { 90. };
    if !value.is_finite() || !(-limit..=limit).contains(&value) {
        return Err("OSM XML coordinate is outside WGS84".into());
    }
    Ok(value)
}
fn attributes(node: Node<'_, '_>, allowed: &[&str]) -> Result<()> {
    if node.tag_name().namespace().is_some()
        || node
            .attributes()
            .any(|a| a.namespace().is_some() || !allowed.contains(&a.name()))
    {
        return Err(format!(
            "Unsupported OSM XML {} attribute or namespace",
            node.tag_name().name()
        ));
    }
    Ok(())
}
pub(super) fn decode(bytes: &[u8]) -> Result<(Vec<Value>, Provenance)> {
    let text = std::str::from_utf8(bytes.strip_prefix(&[0xef, 0xbb, 0xbf]).unwrap_or(bytes))
        .map_err(|_| "OSM XML must be valid UTF-8")?;
    if text.contains("<!DOCTYPE") || text.contains("<!ENTITY") {
        return Err("OSM XML DTD and external entities are not supported".into());
    }
    let doc = Document::parse_with_options(
        text,
        ParsingOptions {
            allow_dtd: false,
            nodes_limit: 1_000_000,
            ..Default::default()
        },
    )
    .map_err(|e| format!("Invalid OSM XML: {e}"))?;
    if doc
        .descendants()
        .any(|n| n.is_text() && n.text().is_some_and(|s| !s.trim().is_empty()))
    {
        return Err("OSM XML contains unsupported text content".into());
    }
    let root = doc.root_element();
    if root.tag_name().name() != "osm" || root.attribute("version") != Some("0.6") {
        return Err(
            "Open an OSM 0.6 snapshot; OSMChange, history and editor actions are not supported"
                .into(),
        );
    }
    attributes(
        root,
        &[
            "version",
            "generator",
            "copyright",
            "attribution",
            "license",
            "timestamp",
        ],
    )?;
    let mut source = Provenance::new("xml");
    source.generator = root.attribute("generator").map(str::to_owned);
    source.xml_root_attributes = root
        .attributes()
        .map(|a| (a.name().into(), a.value().into()))
        .collect();
    source.dataset_timestamp = root.attribute("timestamp").map(str::to_owned);
    let mut elements = Vec::new();
    let mut reference_count = 0usize;
    let mut tag_count = 0usize;
    for node in root.children().filter(Node::is_element) {
        let kind = node.tag_name().name();
        if kind == "bounds" {
            attributes(node, &["minlon", "minlat", "maxlon", "maxlat"])?;
            if source.declared_bounds.is_some() || node.children().any(|n| n.is_element()) {
                return Err("OSM XML has duplicate or invalid bounds".into());
            }
            let bounds = [
                coordinate(node, "minlon")?,
                coordinate(node, "minlat")?,
                coordinate(node, "maxlon")?,
                coordinate(node, "maxlat")?,
            ];
            if !valid_bounds(bounds) {
                return Err("OSM XML has invalid declared bounds".into());
            }
            source.declared_bounds = Some(bounds);
            continue;
        }
        if !["node", "way", "relation"].contains(&kind) {
            return Err(format!("Unsupported OSM XML element: {kind}"));
        }
        attributes(
            node,
            &[
                "id",
                "lat",
                "lon",
                "version",
                "timestamp",
                "changeset",
                "uid",
                "user",
                "visible",
            ],
        )?;
        if kind != "node" && (node.attribute("lat").is_some() || node.attribute("lon").is_some()) {
            return Err("OSM XML way / relation has unsupported coordinates".into());
        }
        let mut element = json!({"type":kind,"id":number(node,"id",true)?});
        if kind == "node" {
            element["lon"] = json!(coordinate(node, "lon")?);
            element["lat"] = json!(coordinate(node, "lat")?);
        }
        for field in ["version", "changeset", "uid"] {
            if node.attribute(field).is_some() {
                element[field] = json!(number(node, field, false)?);
            }
        }
        if let Some(time) = node.attribute("timestamp") {
            chrono::DateTime::parse_from_rfc3339(time)
                .map_err(|_| "OSM XML has invalid object timestamp")?;
            element["timestamp"] = json!(time);
        }
        if let Some(user) = node.attribute("user") {
            element["user"] = json!(user);
        }
        if let Some(visible) = node.attribute("visible") {
            if visible != "true" {
                return Err("Deleted / historical OSM objects are not supported".into());
            }
            element["visible"] = json!(true);
        }
        let mut tags = BTreeMap::new();
        let mut refs = Vec::new();
        let mut members = Vec::new();
        for child in node.children().filter(Node::is_element) {
            if child.children().any(|n| n.is_element()) {
                return Err("Nested OSM XML object content is unsupported".into());
            }
            match child.tag_name().name() {
                "tag" => {
                    attributes(child, &["k", "v"])?;
                    let k = attr(child, "k")?;
                    if k.is_empty()
                        || tags
                            .insert(k.to_owned(), attr(child, "v")?.to_owned())
                            .is_some()
                    {
                        return Err("OSM XML has an empty or duplicate tag key".into());
                    }
                    tag_count += 1;
                }
                "nd" if kind == "way" => {
                    attributes(child, &["ref"])?;
                    refs.push(number(child, "ref", true)?);
                    reference_count += 1;
                }
                "member" if kind == "relation" => {
                    attributes(child, &["type", "ref", "role"])?;
                    let t = attr(child, "type")?;
                    if !["node", "way", "relation"].contains(&t) {
                        return Err("OSM XML has invalid member type".into());
                    }
                    members.push(json!({"type":t,"ref":number(child,"ref",true)?,"role":attr(child,"role")?}));
                    reference_count += 1;
                }
                other => return Err(format!("Unsupported OSM XML {kind} child: {other}")),
            }
            if reference_count > 500_000 || tag_count > 500_000 {
                return Err("OSM XML exceeds 500,000 references or tags".into());
            }
        }
        element["tags"] = json!(tags);
        if kind == "way" {
            element["nodes"] = json!(refs);
        }
        if kind == "relation" {
            element["members"] = json!(members);
        }
        push(&mut elements, &mut source, element)?;
    }
    Ok((elements, source))
}
