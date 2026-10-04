use super::*;

fn schema(geometry: &str, fields: &str) -> Schema {
    let xml = format!(
        r#"<x:schema xmlns:x="http://www.w3.org/2001/XMLSchema" xmlns:g="{GML}" xmlns:t="urn:geod:test" targetNamespace="urn:geod:test" elementFormDefault="qualified">
      <x:element name="object" type="t:ObjectType"/><x:complexType name="ObjectType"><x:complexContent><x:extension base="g:AbstractFeatureType"><x:sequence>
      <x:element name="shape" type="g:{geometry}PropertyType" minOccurs="0" nillable="true"/>{fields}
      </x:sequence></x:extension></x:complexContent></x:complexType></x:schema>"#
    );
    schema::parse_schema(&xml, "t:object", "urn:geod:test").unwrap()
}
fn page(body: &str, matched: &str, returned: usize) -> String {
    format!(
        r#"<w:FeatureCollection xmlns:w="{WFS}" xmlns:g="{GML}" xmlns:t="urn:geod:test" xmlns:xsi="{XSI}" numberMatched="{matched}" numberReturned="{returned}" timeStamp="2026-10-02T11:19:49">
      {body}</w:FeatureCollection>"#
    )
}
fn member(geometry: &str, properties: &str) -> String {
    format!(
        r#"<w:member><t:object g:id="object.1"><t:shape>{geometry}</t:shape>{properties}</t:object></w:member>"#
    )
}
const SRS: &str = "urn:ogc:def:crs:EPSG::4326";

#[test]
fn point_uses_declared_axes_and_retains_typed_properties_and_unknown_matched() {
    let schema = schema(
        "Point",
        r#"<x:element name="id" type="x:long"/><x:element name="price" type="x:decimal"/><x:element name="enabled" type="x:boolean"/><x:element name="name" type="x:string" nillable="true"/><x:element name="missing" type="x:string" minOccurs="0"/>"#,
    );
    let xml = page(
        &member(
            &format!(r#"<g:Point srsName="{SRS}"><g:pos>52.516667 13.400000</g:pos></g:Point>"#),
            r#"<t:id>9007199254740993</t:id><t:price>001.2300</t:price><t:enabled>1</t:enabled><t:name xsi:nil="true"/>"#,
        ),
        "unknown",
        1,
    );
    let actual = parse_page(&xml, &schema).unwrap();
    assert_eq!(actual.number_matched, None);
    assert_eq!(actual.number_returned, 1);
    assert_eq!(actual.time_stamp, "2026-10-02T11:19:49");
    assert_eq!(
        actual.features[0]["geometry"],
        json!({"type":"Point","coordinates":[13.4,52.516667]})
    );
    assert_eq!(
        actual.features[0]["properties"],
        json!({"id":"9007199254740993","price":"001.2300","enabled":true,"name":null})
    );
    assert_eq!(actual.features[0]["id"], "object.1");
    let crs84 = xml
        .replace(SRS, "urn:ogc:def:crs:OGC:1.3:CRS84")
        .replace("52.516667 13.400000", "13.400000 52.516667");
    assert_eq!(
        parse_page(&crs84, &schema).unwrap().features,
        actual.features
    );
}

#[test]
fn simple_linear_gml_geometries_preserve_coordinates_holes_and_member_order() {
    let exterior = "0 0 0 4 4 4 4 0 0 0";
    let interior = "1 1 2 1 2 2 1 2 1 1";
    let polygon = format!(
        r#"<g:Polygon><g:exterior><g:LinearRing><g:posList>{exterior}</g:posList></g:LinearRing></g:exterior><g:interior><g:LinearRing><g:posList>{interior}</g:posList></g:LinearRing></g:interior></g:Polygon>"#
    );
    let line = "<g:LineString><g:posList>1 2 3 4</g:posList></g:LineString>";
    let point = "<g:Point><g:pos>1 2</g:pos></g:Point>";
    let cases = [
        ("LineString",line.to_string(),"LineString"),
        ("Polygon",polygon.clone(),"Polygon"),
        ("MultiPoint",format!("<g:MultiPoint><g:pointMember>{point}</g:pointMember></g:MultiPoint>"),"MultiPoint"),
        ("MultiCurve",format!("<g:MultiCurve><g:curveMember>{line}</g:curveMember></g:MultiCurve>"),"MultiLineString"),
        ("MultiSurface",format!("<g:MultiSurface><g:surfaceMember>{polygon}</g:surfaceMember></g:MultiSurface>"),"MultiPolygon"),
        ("MultiGeometry",format!("<g:MultiGeometry><g:geometryMember>{point}</g:geometryMember><g:geometryMember>{line}</g:geometryMember></g:MultiGeometry>"),"GeometryCollection"),
    ];
    for (declared, geometry, expected) in cases {
        let geometry = geometry.replacen('>', &format!(r#" srsName="{SRS}">"#), 1);
        let actual =
            parse_page(&page(&member(&geometry, ""), "1", 1), &schema(declared, "")).unwrap();
        assert_eq!(
            actual.features[0]["geometry"]["type"], expected,
            "{declared}"
        );
        if expected == "Polygon" {
            assert_eq!(
                actual.features[0]["geometry"]["coordinates"]
                    .as_array()
                    .unwrap()
                    .len(),
                2
            );
        }
        if expected == "GeometryCollection" {
            assert_eq!(
                actual.features[0]["geometry"]["geometries"][0]["coordinates"],
                json!([2.0, 1.0])
            );
            assert_eq!(
                actual.features[0]["geometry"]["geometries"][1]["coordinates"],
                json!([[2.0, 1.0], [4.0, 3.0]])
            );
        }
    }
}

#[test]
fn straight_curve_segments_and_single_planar_surface_patch_are_decoded() {
    let curve = format!(
        r#"<g:Curve srsName="{SRS}"><g:segments><g:LineStringSegment interpolation="linear"><g:posList>1 2 3 4</g:posList></g:LineStringSegment><g:LineStringSegment><g:posList>3 4 5 6</g:posList></g:LineStringSegment></g:segments></g:Curve>"#
    );
    let actual = parse_page(&page(&member(&curve, ""), "1", 1), &schema("Curve", "")).unwrap();
    assert_eq!(
        actual.features[0]["geometry"]["coordinates"],
        json!([[2.0, 1.0], [4.0, 3.0], [6.0, 5.0]])
    );
    let surface = format!(
        r#"<g:Surface srsName="{SRS}"><g:patches><g:PolygonPatch interpolation="planar"><g:exterior><g:LinearRing><g:posList>0 0 0 1 1 1 0 0</g:posList></g:LinearRing></g:exterior></g:PolygonPatch></g:patches></g:Surface>"#
    );
    assert_eq!(
        parse_page(&page(&member(&surface, ""), "1", 1), &schema("Surface", ""))
            .unwrap()
            .features[0]["geometry"]["type"],
        "Polygon"
    );
    assert!(parse_page(
        &page(
            &member(&curve.replace("LineStringSegment", "Arc"), ""),
            "1",
            1
        ),
        &schema("Curve", "")
    )
    .is_err());
    assert!(parse_page(
        &page(&member(&curve.replace("3 4 5 6", "7 8 5 6"), ""), "1", 1),
        &schema("Curve", "")
    )
    .is_err());
}

#[test]
fn missing_or_nil_geometry_and_scalar_properties_follow_the_schema() {
    let mut schema = schema(
        "Point",
        r#"<x:element name="name" type="x:string" nillable="true"/><x:element name="optional" type="x:string" minOccurs="0"/>"#,
    );
    let missing = page(
        r#"<w:member><t:object g:id="object.1"><t:name xsi:nil="true"/></t:object></w:member>"#,
        "1",
        1,
    );
    assert_eq!(
        parse_page(&missing, &schema).unwrap().features[0]["geometry"],
        Value::Null
    );
    schema.geometry_optional = false;
    assert!(parse_page(&missing, &schema).is_err());
    let nil = missing.replace("<t:name", r#"<t:shape xsi:nil="true"/><t:name"#);
    assert!(parse_page(&nil, &schema).is_ok());
    schema.geometry_nullable = false;
    assert!(parse_page(&nil, &schema).is_err());
    schema.geometry_nullable = true;
    assert!(parse_page(&nil.replace(r#"<t:name xsi:nil="true"/>"#, ""), &schema).is_err());
    assert!(parse_page(
        &nil.replace(
            r#"<t:name xsi:nil="true"/>"#,
            r#"<t:name xsi:nil="true">not empty</t:name>"#
        ),
        &schema
    )
    .is_err());
}

#[test]
fn malformed_or_ambiguous_source_data_is_rejected_without_guessing() {
    let geometry = format!(r#"<g:Point srsName="{SRS}"><g:pos>52 13</g:pos></g:Point>"#);
    let xml = page(&member(&geometry, ""), "1", 1);
    let schema = schema("Point", "");
    let variants = [
        xml.replace(SRS,"EPSG:4326"),
        xml.replace("52 13","52 13 1"),
        xml.replace("52 13","91 13"),
        xml.replace("52 13","NaN 13"),
        xml.replace("<g:pos>",r#"<g:pos srsDimension="3">"#),
        xml.replace(r#"numberReturned="1""#,r#"numberReturned="2""#),
        xml.replace(r#"numberMatched="1""#,""),
        xml.replace(GML,"https://invalid.example/spoofed-gml"),
        xml.replace("<g:pos>",r#"<g:pos xmlns:link="http://www.w3.org/1999/xlink" link:href="https://invalid.example/coords">"#),
        xml.replace("</t:object>","<t:unknown>not declared</t:unknown></t:object>"),
        xml.replace("</w:FeatureCollection>","<w:truncatedResponse/></w:FeatureCollection>"),
    ];
    for invalid in variants {
        assert!(parse_page(&invalid, &schema).is_err(), "{invalid}");
    }
    let duplicate = page(
        &format!("{}{}", member(&geometry, ""), member(&geometry, "")),
        "2",
        2,
    );
    assert!(parse_page(&duplicate, &schema).is_err());
    let polygon = format!(
        r#"<g:Polygon srsName="{SRS}"><g:exterior><g:LinearRing><g:posList>0 0 0 1 1 1 1 0</g:posList></g:LinearRing></g:exterior></g:Polygon>"#
    );
    assert!(parse_page(
        &page(&member(&polygon, ""), "1", 1),
        &super::tests::schema("Polygon", "")
    )
    .is_err());
}

#[test]
fn empty_collection_and_next_link_are_explicitly_counted() {
    let schema = schema("Point", "");
    let empty = page("", "0", 0).replace(
        "numberMatched",
        r#"next="https://example.com/wfs?startIndex=1&amp;count=1" numberMatched"#,
    );
    let actual = parse_page(&empty, &schema).unwrap();
    assert!(actual.features.is_empty());
    assert_eq!(actual.number_matched, Some(0));
    assert_eq!(
        actual.next.as_deref(),
        Some("https://example.com/wfs?startIndex=1&count=1")
    );
}
