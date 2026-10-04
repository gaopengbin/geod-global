//! Bounded, namespace-aware WFS DescribeFeatureType schema handling.
//! No schema location, import, entity or feature link is fetched by this module.
use crate::{vector, Result};
use roxmltree::{Document, Node};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

pub(super) const XSD: &str = "http://www.w3.org/2001/XMLSchema";
pub(super) const GML: &str = "http://www.opengis.net/gml/3.2";
pub(super) const XSI: &str = "http://www.w3.org/2001/XMLSchema-instance";
pub(super) const XLINK: &str = "http://www.w3.org/1999/xlink";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Field {
    pub name: String,
    pub field_type: String,
    pub nullable: bool,
    pub optional: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Schema {
    pub namespace: String,
    pub element_name: String,
    pub geometry_field: String,
    pub geometry_type: String,
    pub geometry_nullable: bool,
    pub geometry_optional: bool,
    pub fields: Vec<Field>,
    pub sha256: String,
}

pub(super) fn document(xml: &str) -> Result<Document<'_>> {
    if xml.is_empty() || xml.len() > vector::MAX_BYTES || xml.contains("<!DOCTYPE") {
        return Err("WFS XML must be bounded UTF-8 without a document type declaration".into());
    }
    Document::parse_with_options(
        xml,
        roxmltree::ParsingOptions {
            allow_dtd: false,
            nodes_limit: 1_000_000,
            entity_resolver: None,
        },
    )
    .map_err(|error| format!("Invalid WFS XML: {error}"))
}

/// Compare complete schema definitions while allowing XML formatting changes.
/// This is deliberately stricter than QName canonicalization: namespace-prefix
/// changes may fail equality, but a changed prefix binding cannot pass it.
pub fn definition_fingerprint(xml: &str) -> Result<String> {
    struct Fingerprint {
        hash: Sha256,
        bytes: usize,
    }
    impl Fingerprint {
        fn part(&mut self, tag: u8, text: &str) -> Result<()> {
            self.bytes += 9 + text.len();
            // Repeated in-scope namespace declarations can expand beyond the
            // source length; cap the total signature work independently.
            if self.bytes > vector::MAX_BYTES * 8 {
                return Err("WFS schema definition exceeds its structural limit".into());
            }
            self.hash.update([tag]);
            self.hash.update((text.len() as u64).to_be_bytes());
            self.hash.update(text.as_bytes());
            Ok(())
        }
        fn text(&mut self, pending: &mut String) -> Result<()> {
            if !pending.trim().is_empty() {
                self.part(b'T', pending)?;
            }
            pending.clear();
            Ok(())
        }
    }
    let document = document(xml)?;
    let root = document.root_element();
    if !named(root, XSD, "schema") {
        return Err("WFS definition fingerprint requires an XMLSchema document".into());
    }
    let mut fingerprint = Fingerprint {
        hash: Sha256::new(),
        bytes: 0,
    };
    let mut pending = String::new();
    let mut stack = vec![(root, false)];
    while let Some((node, exiting)) = stack.pop() {
        if exiting {
            fingerprint.text(&mut pending)?;
            fingerprint.part(b'E', "")?;
        } else if node.is_element() {
            fingerprint.text(&mut pending)?;
            fingerprint.part(b'N', node.tag_name().namespace().unwrap_or(""))?;
            fingerprint.part(b'L', node.tag_name().name())?;
            let mut namespaces = node
                .namespaces()
                .map(|ns| (ns.name().unwrap_or(""), ns.uri()))
                .collect::<Vec<_>>();
            namespaces.sort_unstable();
            for (prefix, uri) in namespaces {
                fingerprint.part(b'P', prefix)?;
                fingerprint.part(b'U', uri)?;
            }
            let mut attributes = node
                .attributes()
                .map(|attribute| {
                    (
                        attribute.namespace().unwrap_or(""),
                        attribute.name(),
                        attribute.value(),
                    )
                })
                .collect::<Vec<_>>();
            attributes.sort_unstable();
            for (namespace, name, value) in attributes {
                fingerprint.part(b'A', namespace)?;
                fingerprint.part(b'K', name)?;
                fingerprint.part(b'V', value)?;
            }
            fingerprint.part(b'C', "")?;
            stack.push((node, true));
            stack.extend(node.children().rev().map(|child| (child, false)));
        } else if node.is_text() {
            // Coalesce text separated only by comments, which carry no schema
            // definition. CDATA and normal text therefore compare identically.
            pending.push_str(node.text().unwrap_or(""));
        } else if let Some(instruction) = node.pi() {
            fingerprint.text(&mut pending)?;
            fingerprint.part(b'I', instruction.target)?;
            fingerprint.part(b'J', instruction.value.unwrap_or(""))?;
        }
    }
    Ok(format!("{:x}", fingerprint.hash.finalize()))
}

pub(super) fn named(node: Node<'_, '_>, namespace: &str, local: &str) -> bool {
    node.is_element()
        && node.tag_name().namespace() == Some(namespace)
        && node.tag_name().name() == local
}
pub(super) fn children<'a, 'input>(
    node: Node<'a, 'input>,
) -> impl Iterator<Item = Node<'a, 'input>> {
    node.children().filter(Node::is_element)
}
fn ncname(name: &str) -> bool {
    let mut chars = name.chars();
    name.len() <= 256
        && chars.next().is_some_and(|c| c == '_' || c.is_alphabetic())
        && chars.all(|c| c.is_alphanumeric() || matches!(c, '_' | '-' | '.'))
}
fn qname(node: Node<'_, '_>, text: &str) -> Result<(String, String)> {
    let (prefix, local) = match text.split_once(':') {
        Some((prefix, local)) if ncname(prefix) => (Some(prefix), local),
        Some(_) => return Err("Invalid WFS schema QName".into()),
        None => (None, text),
    };
    if !ncname(local) {
        return Err("Invalid WFS schema QName".into());
    }
    let namespace = node
        .lookup_namespace_uri(prefix)
        .ok_or("Unresolved WFS schema QName")?;
    Ok((namespace.into(), local.into()))
}
fn one<'a, 'input>(
    node: Node<'a, 'input>,
    namespace: &str,
    local: &str,
) -> Result<Node<'a, 'input>> {
    let matches = children(node)
        .filter(|child| named(*child, namespace, local))
        .collect::<Vec<_>>();
    if matches.len() != 1 {
        return Err(format!("WFS schema requires one {local}"));
    }
    Ok(matches[0])
}
fn allowed_children(node: Node<'_, '_>, names: &[&str]) -> Result<()> {
    if children(node).any(|child| {
        child.tag_name().namespace() != Some(XSD) || !names.contains(&child.tag_name().name())
    }) {
        return Err("WFS schema has unsupported complex or repeated fields".into());
    }
    Ok(())
}
fn builtin(kind: &str) -> bool {
    matches!(
        kind,
        "string"
            | "normalizedString"
            | "token"
            | "anyURI"
            | "boolean"
            | "integer"
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
            | "decimal"
            | "double"
            | "float"
            | "date"
            | "dateTime"
            | "time"
    )
}
fn geometry_type(kind: &str) -> Option<&'static str> {
    match kind {
        "PointPropertyType" => Some("Point"),
        "LineStringPropertyType" | "CurvePropertyType" => Some("LineString"),
        "PolygonPropertyType" | "SurfacePropertyType" => Some("Polygon"),
        "MultiPointPropertyType" => Some("MultiPoint"),
        "MultiLineStringPropertyType" | "MultiCurvePropertyType" => Some("MultiLineString"),
        "MultiPolygonPropertyType" | "MultiSurfacePropertyType" => Some("MultiPolygon"),
        "MultiGeometryPropertyType" => Some("GeometryCollection"),
        "GeometryPropertyType" => Some("Geometry"),
        _ => None,
    }
}
fn simple_type(
    root: Node<'_, '_>,
    node: Node<'_, '_>,
    namespace: &str,
    depth: usize,
) -> Result<String> {
    if depth > 8 {
        return Err("WFS simple type nesting exceeds eight levels".into());
    }
    let (uri, kind) = if let Some(kind) = node.attribute("type") {
        qname(node, kind)?
    } else {
        let simple = one(node, XSD, "simpleType")?;
        return restricted_type(root, simple, namespace, depth + 1);
    };
    if uri == XSD && builtin(&kind) {
        return Ok(kind);
    }
    if uri == namespace {
        let definitions = children(root)
            .filter(|n| named(*n, XSD, "simpleType") && n.attribute("name") == Some(kind.as_str()))
            .collect::<Vec<_>>();
        if definitions.len() == 1 {
            return restricted_type(root, definitions[0], namespace, depth + 1);
        }
    }
    Err("WFS field uses an unsupported or externally defined type".into())
}
fn restricted_type(
    root: Node<'_, '_>,
    node: Node<'_, '_>,
    namespace: &str,
    depth: usize,
) -> Result<String> {
    if depth > 8 {
        return Err("WFS simple type nesting exceeds eight levels".into());
    }
    allowed_children(node, &["annotation", "restriction"])?;
    let restriction = one(node, XSD, "restriction")?;
    let (uri, kind) = qname(
        restriction,
        restriction
            .attribute("base")
            .ok_or("WFS restriction has no base type")?,
    )?;
    if uri == XSD && builtin(&kind) {
        return Ok(kind);
    }
    if uri == namespace {
        let definitions = children(root)
            .filter(|n| named(*n, XSD, "simpleType") && n.attribute("name") == Some(kind.as_str()))
            .collect::<Vec<_>>();
        if definitions.len() == 1 {
            return restricted_type(root, definitions[0], namespace, depth + 1);
        }
    }
    Err("WFS simple type has an unsupported restriction base".into())
}
fn bool_attribute(node: Node<'_, '_>, name: &str) -> Result<bool> {
    match node.attribute(name) {
        None | Some("false" | "0") => Ok(false),
        Some("true" | "1") => Ok(true),
        _ => Err(format!("Invalid WFS schema {name}")),
    }
}
fn optional(node: Node<'_, '_>) -> Result<bool> {
    if !matches!(node.attribute("maxOccurs"), None | Some("1")) {
        return Err("Repeated WFS properties require a separate cardinality adapter".into());
    }
    match node.attribute("minOccurs") {
        None | Some("1") => Ok(false),
        Some("0") => Ok(true),
        _ => Err("Invalid WFS property cardinality".into()),
    }
}

pub fn parse_schema(xml: &str, type_name: &str, namespace: &str) -> Result<Schema> {
    let document = document(xml)?;
    let root = document.root_element();
    if !named(root, XSD, "schema")
        || root.attribute("targetNamespace") != Some(namespace)
        || root.attribute("elementFormDefault") != Some("qualified")
    {
        return Err(
            "WFS schema must declare the requested namespace and qualified properties".into(),
        );
    }
    for node in document.descendants().filter(Node::is_element) {
        if node.attributes().any(|a| a.namespace() == Some(XLINK))
            || named(node, XSD, "include")
            || named(node, XSD, "redefine")
            || named(node, XSD, "import") && node.attribute("namespace") != Some(GML)
        {
            return Err("External WFS schema dependencies are not fetched".into());
        }
    }
    let local = type_name.rsplit(':').next().unwrap_or("");
    if !ncname(local) {
        return Err("Invalid WFS feature type name".into());
    }
    let declarations = children(root)
        .filter(|n| named(*n, XSD, "element") && n.attribute("name") == Some(local))
        .collect::<Vec<_>>();
    if declarations.len() != 1 {
        return Err("DescribeFeatureType did not declare exactly one requested feature".into());
    }
    let declaration = declarations[0];
    if let Some(group) = declaration.attribute("substitutionGroup") {
        if qname(declaration, group)? != (GML.into(), "AbstractFeature".into()) {
            return Err("WFS feature does not use the GML 3.2 feature model".into());
        }
    }
    let complex = if let Some(name) = declaration.attribute("type") {
        let (uri, name) = qname(declaration, name)?;
        if uri != namespace {
            return Err("WFS feature type is externally defined".into());
        }
        let definitions = children(root)
            .filter(|n| named(*n, XSD, "complexType") && n.attribute("name") == Some(name.as_str()))
            .collect::<Vec<_>>();
        if definitions.len() != 1 {
            return Err("WFS schema has no unique feature definition".into());
        }
        definitions[0]
    } else {
        one(declaration, XSD, "complexType")?
    };
    allowed_children(complex, &["annotation", "complexContent"])?;
    let content = one(complex, XSD, "complexContent")?;
    allowed_children(content, &["annotation", "extension"])?;
    let extension = one(content, XSD, "extension")?;
    if qname(
        extension,
        extension
            .attribute("base")
            .ok_or("WFS feature has no base type")?,
    )? != (GML.into(), "AbstractFeatureType".into())
    {
        return Err("WFS feature must directly extend GML AbstractFeatureType".into());
    }
    allowed_children(extension, &["annotation", "sequence"])?;
    let sequence = one(extension, XSD, "sequence")?;
    if optional(sequence)? {
        return Err("Optional WFS schema sequences are not supported".into());
    }
    allowed_children(sequence, &["annotation", "element"])?;
    let mut fields = Vec::new();
    let mut geometry = None;
    let mut names = BTreeSet::new();
    for field in children(sequence).filter(|n| named(*n, XSD, "element")) {
        let name = field
            .attribute("name")
            .filter(|name| ncname(name))
            .ok_or("WFS property has no local name")?;
        if !names.insert(name)
            || names.len() > 513
            || field.attribute("ref").is_some()
            || field
                .attribute("form")
                .is_some_and(|form| form != "qualified")
        {
            return Err("WFS schema has repeated, referenced or unqualified properties".into());
        }
        allowed_children(field, &["annotation", "simpleType"])?;
        let optional = optional(field)?;
        let nullable = bool_attribute(field, "nillable")?;
        if let Some((uri, kind)) = field
            .attribute("type")
            .map(|kind| qname(field, kind))
            .transpose()?
        {
            if uri == GML {
                let kind = geometry_type(&kind).ok_or("Unsupported GML geometry property type")?;
                if geometry
                    .replace((name.to_string(), kind.to_string(), nullable, optional))
                    .is_some()
                {
                    return Err("WFS feature has multiple geometry properties".into());
                }
                continue;
            }
        }
        fields.push(Field {
            name: name.into(),
            field_type: simple_type(root, field, namespace, 0)?,
            nullable,
            optional,
        });
    }
    let (geometry_field, geometry_type, geometry_nullable, geometry_optional) =
        geometry.ok_or("WFS feature has no supported geometry property")?;
    let schema = Schema {
        namespace: namespace.into(),
        element_name: local.into(),
        geometry_field,
        geometry_type,
        geometry_nullable,
        geometry_optional,
        fields,
        sha256: format!("{:x}", Sha256::digest(xml.as_bytes())),
    };
    validate_schema(&schema)?;
    Ok(schema)
}

pub fn validate_schema(schema: &Schema) -> Result<()> {
    if schema.namespace.is_empty()
        || schema.namespace.len() > 2048
        || schema.namespace.chars().any(char::is_control)
        || !ncname(&schema.element_name)
        || !ncname(&schema.geometry_field)
        || ![
            "Point",
            "LineString",
            "Polygon",
            "MultiPoint",
            "MultiLineString",
            "MultiPolygon",
            "GeometryCollection",
            "Geometry",
        ]
        .contains(&schema.geometry_type.as_str())
        || schema.fields.len() > 512
        || schema.sha256.len() != 64
        || !schema
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err("Invalid saved WFS schema".into());
    }
    let mut names = BTreeSet::from([schema.geometry_field.as_str()]);
    for field in &schema.fields {
        if !ncname(&field.name) || !builtin(&field.field_type) || !names.insert(field.name.as_str())
        {
            return Err("Invalid or repeated saved WFS property".into());
        }
    }
    Ok(())
}
fn integer(kind: &str) -> bool {
    matches!(
        kind,
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
}
fn integer_lexical(text: &str, kind: &str) -> Result<()> {
    let digits = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err("Invalid WFS integer value".into());
    }
    let zero = digits.bytes().all(|b| b == b'0');
    let negative = text.starts_with('-') && !zero;
    let valid = match kind {
        "integer" => true,
        "positiveInteger" => !negative && !zero,
        "nonNegativeInteger" => !negative,
        "negativeInteger" => negative,
        "nonPositiveInteger" => negative || zero,
        "long" => text.parse::<i64>().is_ok(),
        "int" => text.parse::<i32>().is_ok(),
        "short" => text.parse::<i16>().is_ok(),
        "byte" => text.parse::<i8>().is_ok(),
        "unsignedLong" => !negative && text.trim_start_matches('+').parse::<u64>().is_ok(),
        "unsignedInt" => !negative && text.trim_start_matches('+').parse::<u32>().is_ok(),
        "unsignedShort" => !negative && text.trim_start_matches('+').parse::<u16>().is_ok(),
        "unsignedByte" => !negative && text.trim_start_matches('+').parse::<u8>().is_ok(),
        _ => false,
    };
    if !valid {
        return Err("WFS integer is outside its declared XMLSchema type".into());
    }
    Ok(())
}
fn decimal_lexical(text: &str) -> bool {
    let digits = text
        .strip_prefix('+')
        .or_else(|| text.strip_prefix('-'))
        .unwrap_or(text);
    let mut decimal_points = 0;
    let mut count = 0;
    for byte in digits.bytes() {
        if byte.is_ascii_digit() {
            count += 1;
        } else if byte == b'.' {
            decimal_points += 1;
        } else {
            return false;
        }
    }
    count > 0 && decimal_points <= 1
}
pub(super) fn scalar(text: &str, field: &Field) -> Result<Value> {
    let kind = field.field_type.as_str();
    let lexical = text.trim();
    if integer(kind) {
        integer_lexical(lexical, kind)?;
        if let Ok(value) = lexical.parse::<i64>() {
            if (-9_007_199_254_740_991..=9_007_199_254_740_991).contains(&value) {
                return Ok(Value::from(value));
            }
        }
        return Ok(Value::String(lexical.into()));
    }
    match kind {
        "boolean" => match lexical {
            "true" | "1" => Ok(Value::Bool(true)),
            "false" | "0" => Ok(Value::Bool(false)),
            _ => Err("Invalid WFS boolean value".into()),
        },
        "decimal" if decimal_lexical(lexical) => Ok(Value::String(lexical.into())),
        "decimal" => Err("Invalid WFS decimal value".into()),
        "float" | "double" => {
            let number = lexical
                .parse::<f64>()
                .map_err(|_| "Invalid WFS floating-point value")?;
            if !number.is_finite() || kind == "float" && !(number as f32).is_finite() {
                return Err(
                    "Non-finite WFS floating-point values cannot be represented in GeoJSON".into(),
                );
            }
            Ok(Value::from(number))
        }
        "string" | "normalizedString" | "token" | "anyURI" | "date" | "dateTime" | "time" => {
            Ok(Value::String(text.into()))
        }
        _ => Err("Unsupported WFS scalar type".into()),
    }
}

pub fn validate_properties(properties: &Value, schema: &Schema) -> Result<()> {
    validate_schema(schema)?;
    let properties = properties
        .as_object()
        .ok_or("WFS feature has no properties object")?;
    if properties
        .keys()
        .any(|name| !schema.fields.iter().any(|field| &field.name == name))
    {
        return Err("WFS feature contains a property absent from its schema".into());
    }
    for field in &schema.fields {
        let Some(value) = properties.get(&field.name) else {
            if field.optional {
                continue;
            }
            return Err(format!(
                "WFS feature is missing required property {}",
                field.name
            ));
        };
        if value.is_null() {
            if field.nullable {
                continue;
            }
            return Err(format!("WFS property {} is not nullable", field.name));
        }
        let valid = if integer(&field.field_type) {
            if let Some(text) = value.as_str() {
                scalar(text, field).is_ok_and(|v| v == *value && v.is_string())
            } else if value.is_number() {
                scalar(&value.to_string(), field).is_ok_and(|v| v == *value && v.is_number())
            } else {
                false
            }
        } else {
            match field.field_type.as_str() {
                "boolean" => value.is_boolean(),
                "decimal" => value.as_str().is_some_and(decimal_lexical),
                "float" | "double" => value.as_f64().is_some_and(|number| {
                    number.is_finite()
                        && (field.field_type != "float" || (number as f32).is_finite())
                }),
                _ => value.is_string(),
            }
        };
        if !valid {
            return Err(format!(
                "WFS property {} does not preserve its declared type; use GML output",
                field.name
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "schema_tests.rs"]
mod tests;
