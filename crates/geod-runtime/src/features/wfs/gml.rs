//! GML 3.2 simple linear geometry with explicit WGS84 axis interpretation.
use super::{schema, Page};
use crate::{vector, Result};
use roxmltree::Node;
use schema::{children, named, Schema, GML, XLINK, XSI};
use serde_json::{json, Map, Value};
use std::collections::BTreeSet;

const WFS: &str = "http://www.opengis.net/wfs/2.0";

pub(super) fn timestamp_valid(text: &str) -> bool {
    chrono::DateTime::parse_from_rfc3339(text).is_ok()
        || chrono::NaiveDateTime::parse_from_str(text, "%Y-%m-%dT%H:%M:%S%.f").is_ok()
}
fn text_only(node: Node<'_, '_>) -> Result<String> {
    if children(node).next().is_some() {
        return Err("WFS scalar or coordinate value contains nested XML".into());
    }
    Ok(node
        .children()
        .filter(Node::is_text)
        .filter_map(|node| node.text())
        .collect())
}
fn only_child<'a, 'input>(node: Node<'a, 'input>) -> Result<Node<'a, 'input>> {
    let elements = children(node).collect::<Vec<_>>();
    if elements.len() != 1 {
        return Err("GML property requires exactly one inline value".into());
    }
    Ok(elements[0])
}
fn direct<'a, 'input>(node: Node<'a, 'input>, name: &str) -> Result<Node<'a, 'input>> {
    let children = children(node)
        .filter(|child| named(*child, GML, name))
        .collect::<Vec<_>>();
    if children.len() != 1 {
        return Err(format!("GML geometry requires one {name}"));
    }
    Ok(children[0])
}
fn nil(node: Node<'_, '_>) -> Result<bool> {
    match node.attribute((XSI, "nil")) {
        None | Some("false" | "0") => Ok(false),
        Some("true" | "1") => Ok(true),
        _ => Err("Invalid WFS xsi:nil value".into()),
    }
}
fn nil_content(node: Node<'_, '_>) -> Result<()> {
    if children(node).next().is_some() || !text_only(node)?.trim().is_empty() {
        return Err("Nil WFS property must not contain a value".into());
    }
    Ok(())
}
fn geometry_type_valid(expected: &str, value: &Value) -> bool {
    expected == "Geometry" || value["type"] == expected
}

#[derive(Clone, Copy)]
struct Axes {
    latitude_first: bool,
}
fn axes(node: Node<'_, '_>) -> Result<Axes> {
    let srs = node
        .ancestors()
        .find_map(|ancestor| ancestor.attribute("srsName"))
        .ok_or("GML geometry has no explicit coordinate reference system")?;
    let latitude_first = match srs {
        "urn:ogc:def:crs:EPSG::4326" | "http://www.opengis.net/def/crs/EPSG/0/4326" => true,
        "urn:ogc:def:crs:OGC:1.3:CRS84"
        | "urn:ogc:def:crs:OGC::CRS84"
        | "http://www.opengis.net/def/crs/OGC/1.3/CRS84" => false,
        _ => return Err("GML geometry requires explicit EPSG:4326 or CRS84 URI axis order".into()),
    };
    if node
        .ancestors()
        .find_map(|ancestor| ancestor.attribute("srsDimension"))
        .is_some_and(|dimension| dimension != "2")
    {
        return Err("WFS GML extraction supports two-dimensional positions".into());
    }
    if let Some(labels) = node
        .ancestors()
        .find_map(|ancestor| ancestor.attribute("axisLabels"))
    {
        let labels = labels
            .split_whitespace()
            .map(str::to_ascii_lowercase)
            .collect::<Vec<_>>();
        let expected = if latitude_first {
            ["lat", "long"]
        } else {
            ["long", "lat"]
        };
        if labels.len() != 2 || labels[0] != expected[0] || labels[1] != expected[1] {
            return Err("GML axis labels conflict with the declared CRS".into());
        }
    }
    Ok(Axes { latitude_first })
}
#[derive(Default)]
struct GeometryReader {
    positions: usize,
}
impl GeometryReader {
    fn positions(&mut self, node: Node<'_, '_>, multiple: bool) -> Result<Vec<Value>> {
        let axes = axes(node)?;
        let text = text_only(node)?;
        let mut values = Vec::new();
        for token in text.split_whitespace() {
            if values.len() >= 2 * vector::MAX_COORDINATES {
                return Err("GML coordinate limit exceeded".into());
            }
            let value = token.parse::<f64>().map_err(|_| "Invalid GML coordinate")?;
            if !value.is_finite() {
                return Err("GML coordinates must be finite".into());
            }
            values.push(value);
        }
        if values.is_empty() || values.len() % 2 != 0 || !multiple && values.len() != 2 {
            return Err("GML positions require exactly two coordinates per position".into());
        }
        if let Some(count) = node.attribute("count") {
            if count.parse::<usize>().ok() != Some(values.len() / 2) {
                return Err("GML posList count differs from its coordinates".into());
            }
        }
        self.positions += values.len() / 2;
        if self.positions > vector::MAX_COORDINATES {
            return Err("GML page exceeds 500,000 positions".into());
        }
        values
            .chunks_exact(2)
            .map(|pair| {
                let (lon, lat) = if axes.latitude_first {
                    (pair[1], pair[0])
                } else {
                    (pair[0], pair[1])
                };
                if !(-180.0..=180.0).contains(&lon) || !(-90.0..=90.0).contains(&lat) {
                    return Err("GML coordinates are outside the declared WGS84 range".into());
                }
                Ok(json!([lon, lat]))
            })
            .collect()
    }
    fn line(&mut self, node: Node<'_, '_>, ring: bool) -> Result<Vec<Value>> {
        let parts = children(node).collect::<Vec<_>>();
        let points = if parts.len() == 1 && named(parts[0], GML, "posList") {
            self.positions(parts[0], true)?
        } else if !parts.is_empty() && parts.iter().all(|node| named(*node, GML, "pos")) {
            let mut points = Vec::new();
            for part in parts {
                points.extend(self.positions(part, false)?);
            }
            points
        } else {
            return Err("GML line requires inline posList or positions; curved or referenced coordinates are unsupported".into());
        };
        if points.len() < if ring { 4 } else { 2 } || ring && points.first() != points.last() {
            return Err("GML line is incomplete or its ring is not closed".into());
        }
        Ok(points)
    }
    fn polygon(&mut self, node: Node<'_, '_>) -> Result<Value> {
        let exterior = direct(node, "exterior")?;
        let exterior_ring = only_child(exterior)?;
        if !named(exterior_ring, GML, "LinearRing") {
            return Err("GML polygon exterior must be a linear ring".into());
        }
        let mut rings = vec![self.line(exterior_ring, true)?];
        for child in children(node) {
            if named(child, GML, "exterior") {
                continue;
            }
            if !named(child, GML, "interior") {
                return Err("Unsupported GML polygon component".into());
            }
            let ring = only_child(child)?;
            if !named(ring, GML, "LinearRing") {
                return Err("GML polygon interior must be a linear ring".into());
            }
            rings.push(self.line(ring, true)?);
        }
        Ok(json!({"type":"Polygon","coordinates":rings}))
    }
    fn curve(&mut self, node: Node<'_, '_>) -> Result<Value> {
        let segments = only_child(node)?;
        if !named(segments, GML, "segments") {
            return Err("GML curve has no inline segments".into());
        }
        let mut points: Vec<Value> = Vec::new();
        for segment in children(segments) {
            if !named(segment, GML, "LineStringSegment")
                || segment
                    .attribute("interpolation")
                    .is_some_and(|mode| mode != "linear")
            {
                return Err("Curved GML segments require a separate geometry adapter".into());
            }
            let part = self.line(segment, false)?;
            if points.is_empty() {
                points.extend(part);
            } else {
                if points.last() != part.first() {
                    return Err("GML curve segments do not connect".into());
                }
                points.extend(part.into_iter().skip(1));
            }
        }
        if points.len() < 2 {
            return Err("GML curve has no complete segments".into());
        }
        Ok(json!({"type":"LineString","coordinates":points}))
    }
    fn multi(
        &mut self,
        node: Node<'_, '_>,
        member: &str,
        members: &str,
        expected: &str,
        output: &str,
        depth: usize,
    ) -> Result<Value> {
        let mut geometries = Vec::new();
        for wrapper in children(node) {
            let values = if named(wrapper, GML, member) {
                vec![only_child(wrapper)?]
            } else if named(wrapper, GML, members) {
                children(wrapper).collect::<Vec<_>>()
            } else {
                return Err("Unsupported GML multi-geometry member".into());
            };
            if values.is_empty() {
                return Err("Empty GML geometry member".into());
            }
            for value in values {
                let geometry = self.geometry(value, depth + 1)?;
                if !geometry_type_valid(expected, &geometry) {
                    return Err("GML geometry member has the wrong type".into());
                }
                geometries.push(geometry);
            }
        }
        if geometries.is_empty() {
            return Err("Empty GML multi-geometry".into());
        }
        if output == "GeometryCollection" {
            Ok(json!({"type":output,"geometries":geometries}))
        } else {
            Ok(
                json!({"type":output,"coordinates":geometries.into_iter().map(|g| g["coordinates"].clone()).collect::<Vec<_>>()}),
            )
        }
    }
    fn geometry(&mut self, node: Node<'_, '_>, depth: usize) -> Result<Value> {
        if depth > 8 || node.tag_name().namespace() != Some(GML) {
            return Err("Unsupported or excessively nested GML geometry".into());
        }
        // Every coordinate sequence must resolve an explicit supported CRS.
        // Collection containers may omit it when each member declares its own.
        match node.tag_name().name() {
            "Point" => {
                let position = only_child(node)?;
                if !named(position, GML, "pos") {
                    return Err("GML point requires one inline pos".into());
                }
                Ok(json!({"type":"Point","coordinates":self.positions(position, false)?.remove(0)}))
            }
            "LineString" => Ok(json!({"type":"LineString","coordinates":self.line(node, false)?})),
            "Curve" => self.curve(node),
            "Polygon" => self.polygon(node),
            "Surface" => {
                let patches = only_child(node)?;
                if !named(patches, GML, "patches") {
                    return Err("GML surface requires inline patches".into());
                }
                let patch = only_child(patches)?;
                if !named(patch, GML, "PolygonPatch")
                    || patch
                        .attribute("interpolation")
                        .is_some_and(|mode| mode != "planar")
                {
                    return Err(
                        "Only a single planar GML polygon patch is supported per surface".into(),
                    );
                }
                self.polygon(patch)
            }
            "MultiPoint" => self.multi(
                node,
                "pointMember",
                "pointMembers",
                "Point",
                "MultiPoint",
                depth,
            ),
            "MultiCurve" => self.multi(
                node,
                "curveMember",
                "curveMembers",
                "LineString",
                "MultiLineString",
                depth,
            ),
            "MultiLineString" => self.multi(
                node,
                "lineStringMember",
                "lineStringMembers",
                "LineString",
                "MultiLineString",
                depth,
            ),
            "MultiSurface" => self.multi(
                node,
                "surfaceMember",
                "surfaceMembers",
                "Polygon",
                "MultiPolygon",
                depth,
            ),
            "MultiPolygon" => self.multi(
                node,
                "polygonMember",
                "polygonMembers",
                "Polygon",
                "MultiPolygon",
                depth,
            ),
            "MultiGeometry" => self.multi(
                node,
                "geometryMember",
                "geometryMembers",
                "Geometry",
                "GeometryCollection",
                depth,
            ),
            _ => {
                Err("Unsupported GML geometry; curved or solid geometry is not approximated".into())
            }
        }
    }
}

fn feature(node: Node<'_, '_>, schema: &Schema, reader: &mut GeometryReader) -> Result<Value> {
    if !named(node, &schema.namespace, &schema.element_name) {
        return Err("WFS returned a feature from a different type or namespace".into());
    }
    let id = node
        .attribute((GML, "id"))
        .filter(|id| !id.is_empty() && id.len() <= 512 && !id.chars().any(char::is_control))
        .ok_or("WFS feature has no stable GML identity")?;
    if node
        .attributes()
        .any(|attribute| attribute.namespace() != Some(GML) || attribute.name() != "id")
    {
        return Err("WFS feature has unsupported XML attributes".into());
    }
    let mut seen = BTreeSet::new();
    let mut properties = Map::new();
    let mut geometry = None;
    for property in children(node) {
        if named(property, GML, "boundedBy") {
            continue;
        }
        if property.tag_name().namespace() != Some(schema.namespace.as_str())
            || !seen.insert(property.tag_name().name().to_string())
        {
            return Err("WFS feature has a repeated or foreign-namespace property".into());
        }
        if property.attributes().any(|attribute| {
            !(attribute.namespace() == Some(XSI) && attribute.name() == "nil"
                || attribute.namespace().is_none() && attribute.name() == "nilReason")
        }) {
            return Err("WFS property has unsupported XML attributes".into());
        }
        let is_nil = nil(property)?;
        if property.tag_name().name() == schema.geometry_field {
            if is_nil {
                if !schema.geometry_nullable {
                    return Err("WFS geometry is not nullable".into());
                }
                nil_content(property)?;
                geometry = Some(Value::Null);
            } else {
                let value = reader.geometry(only_child(property)?, 0)?;
                if !geometry_type_valid(&schema.geometry_type, &value) {
                    return Err("WFS geometry differs from its schema".into());
                }
                geometry = Some(value);
            }
        } else {
            let field = schema
                .fields
                .iter()
                .find(|field| field.name == property.tag_name().name())
                .ok_or("WFS feature has a property absent from its schema")?;
            let value = if is_nil {
                if !field.nullable {
                    return Err("WFS property is not nullable".into());
                }
                nil_content(property)?;
                Value::Null
            } else {
                schema::scalar(&text_only(property)?, field)?
            };
            properties.insert(field.name.clone(), value);
        }
    }
    let geometry = match geometry {
        Some(geometry) => geometry,
        None if schema.geometry_optional => Value::Null,
        None => return Err("WFS feature is missing its required geometry property".into()),
    };
    let properties = Value::Object(properties);
    schema::validate_properties(&properties, schema)?;
    Ok(json!({"type":"Feature","id":id,"properties":properties,"geometry":geometry}))
}

pub fn parse_page(xml: &str, schema: &Schema) -> Result<Page> {
    schema::validate_schema(schema)?;
    let document = schema::document(xml)?;
    let root = document.root_element();
    if !named(root, WFS, "FeatureCollection") {
        return Err("WFS did not return a WFS 2.0 feature collection".into());
    }
    if document.descendants().filter(Node::is_element).any(|node| {
        node.attributes()
            .any(|attribute| attribute.namespace() == Some(XLINK))
    }) {
        return Err("External or referenced WFS geometries are not fetched".into());
    }
    let count = |text: &str| -> Result<usize> {
        if text.is_empty() || !text.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err("Invalid WFS result count".into());
        }
        text.parse::<usize>()
            .ok()
            .filter(|count| *count <= vector::MAX_FEATURES)
            .ok_or("WFS query exceeds 50,000 features".into())
    };
    let number_matched = match root.attribute("numberMatched") {
        Some("unknown") => None,
        Some(text) => Some(count(text)?),
        None => return Err("WFS response has no numberMatched declaration".into()),
    };
    let number_returned = count(
        root.attribute("numberReturned")
            .ok_or("WFS response has no numberReturned declaration")?,
    )?;
    if number_matched.is_some_and(|matched| matched < number_returned) {
        return Err("WFS result counts are inconsistent".into());
    }
    let time_stamp = root
        .attribute("timeStamp")
        .filter(|value| value.len() <= 128 && timestamp_valid(value))
        .ok_or("WFS response has no valid dataset response timestamp")?
        .to_string();
    let next = root
        .attribute("next")
        .map(|next| -> Result<String> {
            if next.is_empty() || next.len() > 2048 || next.chars().any(char::is_control) {
                Err("Invalid WFS next-page link".into())
            } else {
                Ok(next.to_string())
            }
        })
        .transpose()?;
    let mut reader = GeometryReader::default();
    let mut features = Vec::new();
    let mut ids = BTreeSet::new();
    for child in children(root) {
        if named(child, WFS, "boundedBy") || named(child, GML, "boundedBy") {
            continue;
        }
        if !named(child, WFS, "member") {
            return Err("WFS response has unsupported or truncated result content".into());
        }
        if features.len() >= vector::MAX_FEATURES {
            return Err("WFS page exceeds 50,000 features".into());
        }
        let value = feature(only_child(child)?, schema, &mut reader)?;
        if !ids.insert(value["id"].as_str().unwrap().to_string()) {
            return Err("WFS page contains duplicate feature identity".into());
        }
        features.push(value);
    }
    if features.len() != number_returned {
        return Err("WFS response ended before all declared features were received".into());
    }
    Ok(Page {
        features,
        number_matched,
        number_returned,
        time_stamp,
        next,
    })
}

#[cfg(test)]
#[path = "gml_tests.rs"]
mod tests;
