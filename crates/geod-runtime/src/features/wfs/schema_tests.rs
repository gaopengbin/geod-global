use super::*;
use serde_json::json;

fn fixture(fields: &str) -> String {
    format!(
        r#"<s:schema xmlns:s="{XSD}" xmlns:g="{GML}" xmlns:f="urn:geod:test" targetNamespace="urn:geod:test" elementFormDefault="qualified">
      <s:import namespace="{GML}" schemaLocation="https://invalid.example/never-fetched.xsd"/>
      <s:element name="place" type="f:PlaceType" substitutionGroup="g:AbstractFeature"/>
      <s:complexType name="PlaceType"><s:complexContent><s:extension base="g:AbstractFeatureType"><s:sequence>
      <s:element name="geom" type="g:MultiSurfacePropertyType" minOccurs="0" nillable="true"/>{fields}
      </s:sequence></s:extension></s:complexContent></s:complexType></s:schema>"#
    )
}

#[test]
fn parses_qualified_flat_fields_and_geometry_with_actual_qname_namespaces() {
    let xml = fixture(
        r#"<s:element name="id" type="s:long"/><s:element name="name" type="s:string" minOccurs="0" nillable="true"/><s:element name="cost" type="s:decimal"/><s:element name="enabled" type="s:boolean"/>"#,
    );
    let schema = parse_schema(&xml, "some_prefix:place", "urn:geod:test").unwrap();
    assert_eq!(schema.namespace, "urn:geod:test");
    assert_eq!(schema.element_name, "place");
    assert_eq!(schema.geometry_field, "geom");
    assert_eq!(schema.geometry_type, "MultiPolygon");
    assert!(schema.geometry_nullable && schema.geometry_optional);
    assert_eq!(
        schema.fields[0],
        Field {
            name: "id".into(),
            field_type: "long".into(),
            nullable: false,
            optional: false
        }
    );
    assert!(schema.fields[1].nullable && schema.fields[1].optional);
    assert_eq!(
        schema.sha256,
        format!("{:x}", Sha256::digest(xml.as_bytes()))
    );
    assert!(parse_schema(&xml, "other", "urn:geod:test").is_err());
    assert!(parse_schema(&xml, "place", "urn:other").is_err());
}

#[test]
fn default_xsd_namespace_and_relative_feature_namespace_are_resolved_without_fetching() {
    let xml = format!(
        r#"<schema xmlns="{XSD}" xmlns:f="test_namespace" xmlns:gml="{GML}" targetNamespace="test_namespace" elementFormDefault="qualified">
      <element name="places" type="f:PlaceType"/><complexType name="PlaceType"><complexContent><extension base="gml:AbstractFeatureType"><sequence>
      <element name="shape" type="gml:SurfacePropertyType"/><element name="count" type="integer" minOccurs="0"/>
      </sequence></extension></complexContent></complexType></schema>"#
    );
    let schema = parse_schema(&xml, "f:places", "test_namespace").unwrap();
    assert_eq!(schema.geometry_type, "Polygon");
    assert_eq!(schema.fields[0].field_type, "integer");
    assert_eq!(schema.namespace, "test_namespace");
}

#[test]
fn repeated_complex_foreign_and_multiple_geometry_fields_are_rejected() {
    for fields in [
        r#"<s:element name="other_geom" type="g:PointPropertyType"/>"#,
        r#"<s:element name="values" type="s:string" maxOccurs="unbounded"/>"#,
        r#"<s:element name="name" type="s:string"/><s:element name="name" type="s:string"/>"#,
        r#"<s:element name="name" type="unknown:string"/>"#,
        r#"<s:element name="address"><s:complexType><s:sequence/></s:complexType></s:element>"#,
        r#"<s:element name="name" type="s:string" form="unqualified"/>"#,
        r#"<s:element name="name" type="s:string" nillable="maybe"/>"#,
    ] {
        assert!(
            parse_schema(&fixture(fields), "f:place", "urn:geod:test").is_err(),
            "{fields}"
        );
    }
    let external = fixture("").replace("<s:element name=",r#"<s:import namespace="https://invalid.example/schema" schemaLocation="https://invalid.example/types.xsd"/><s:element name="#);
    assert!(parse_schema(&external, "place", "urn:geod:test").is_err());
    let dtd = format!(
        r#"<!DOCTYPE schema [<!ENTITY external SYSTEM "https://invalid.example/secret">]>{}"#,
        fixture("")
    );
    assert!(parse_schema(&dtd, "place", "urn:geod:test").is_err());
}

#[test]
fn scalar_values_keep_integer_precision_decimal_scale_and_nullability() {
    let xml = fixture(
        r#"<s:element name="id" type="s:long"/><s:element name="decimal" type="s:decimal"/><s:element name="flag" type="s:boolean"/><s:element name="when" type="s:dateTime"/><s:element name="optional" type="s:string" minOccurs="0"/><s:element name="nullable" type="s:string" nillable="true"/>"#,
    );
    let schema = parse_schema(&xml, "place", "urn:geod:test").unwrap();
    assert_eq!(
        scalar("9007199254740993", &schema.fields[0]).unwrap(),
        json!("9007199254740993")
    );
    assert_eq!(
        scalar("9007199254740991", &schema.fields[0]).unwrap(),
        json!(9007199254740991_i64)
    );
    assert!(scalar("9223372036854775808", &schema.fields[0]).is_err());
    assert_eq!(
        scalar("001.2300", &schema.fields[1]).unwrap(),
        json!("001.2300")
    );
    assert!(scalar("1e3", &schema.fields[1]).is_err());
    assert_eq!(scalar("1", &schema.fields[2]).unwrap(), json!(true));
    let valid = json!({"id":"9007199254740993","decimal":"001.2300","flag":true,"when":"2026-10-02T12:30:00+02:00","nullable":null});
    validate_properties(&valid, &schema).unwrap();
    for (field, value) in [
        ("id", json!(9007199254740993_i64)),
        ("decimal", json!(1.23)),
        ("flag", json!("true")),
        ("when", json!(null)),
    ] {
        let mut invalid = valid.clone();
        invalid[field] = value;
        assert!(validate_properties(&invalid, &schema).is_err(), "{field}");
    }
    let mut missing = valid.clone();
    missing.as_object_mut().unwrap().remove("flag");
    assert!(validate_properties(&missing, &schema).is_err());
    let mut extra = valid;
    extra["undeclared"] = json!(true);
    assert!(validate_properties(&extra, &schema).is_err());
}

#[test]
fn inline_and_named_simple_type_restrictions_preserve_their_builtin_encoding() {
    let xml = fixture(r#"<s:element name="status"><s:simpleType><s:restriction base="s:string"><s:enumeration value="open"/></s:restriction></s:simpleType></s:element><s:element name="code" type="f:Code"/>"#)
        .replace("</s:schema>",r#"<s:simpleType name="Code"><s:restriction base="s:integer"/></s:simpleType></s:schema>"#);
    let schema = parse_schema(&xml, "place", "urn:geod:test").unwrap();
    assert_eq!(schema.fields[0].field_type, "string");
    assert_eq!(schema.fields[1].field_type, "integer");
}

#[test]
fn definition_fingerprint_ignores_only_xml_layout_attribute_order_and_comments() {
    let original = fixture(
        r#"<s:element name="name" type="s:string" minOccurs="0" nillable="true"><s:annotation><s:documentation>street name</s:documentation></s:annotation></s:element>"#,
    );
    let reformatted = original
        .replace(
            r#"name="name" type="s:string" minOccurs="0" nillable="true""#,
            r#"nillable="true" minOccurs="0" type="s:string" name="name""#,
        )
        .replace("><", ">\n  <!-- layout comment -->\n  <")
        .replace("street name", "street<!-- ignored --> name");
    assert_ne!(
        format!("{:x}", Sha256::digest(original.as_bytes())),
        format!("{:x}", Sha256::digest(reformatted.as_bytes()))
    );
    assert_eq!(
        definition_fingerprint(&original).unwrap(),
        definition_fingerprint(&reformatted).unwrap()
    );
    assert_eq!(
        definition_fingerprint(&original).unwrap(),
        definition_fingerprint(&original.replace("street name", "<![CDATA[street name]]>"))
            .unwrap()
    );
    assert_ne!(
        definition_fingerprint(&original).unwrap(),
        definition_fingerprint(&original.replace("street name", "road name")).unwrap()
    );
}

#[test]
fn definition_fingerprint_detects_facets_and_qname_binding_changes() {
    let original = fixture(
        r#"<s:element name="code"><s:simpleType><s:restriction base="s:integer"><s:enumeration value="12"/><s:pattern value="[0-9]+"/><s:minInclusive value="0"/><s:maxExclusive value="100"/></s:restriction></s:simpleType></s:element>"#,
    );
    let before = definition_fingerprint(&original).unwrap();
    for changed in [
        original.replace(r#"value="12""#, r#"value="13""#),
        original.replace(r#"value="[0-9]+""#, r#"value="[1-9]+""#),
        original.replace(r#"value="0""#, r#"value="1""#),
        original.replace(r#"value="100""#, r#"value="101""#),
        original.replace(
            r#"xmlns:f="urn:geod:test""#,
            r#"xmlns:f="urn:geod:changed""#,
        ),
        original.replace("<s:enumeration", "<s:enumeration fixed=\"true\""),
    ] {
        assert_ne!(before, definition_fingerprint(&changed).unwrap());
    }
    assert!(definition_fingerprint("<not-a-schema/>").is_err());
    assert!(definition_fingerprint(&format!("<!DOCTYPE schema>{original}")).is_err());
}
