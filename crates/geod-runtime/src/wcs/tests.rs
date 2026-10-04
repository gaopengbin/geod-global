use super::*;
use std::io::Cursor;
use tiff::{
    encoder::{colortype, TiffEncoder},
    tags::Tag,
};

fn capabilities() -> Vec<u8> {
    br#"<wcs:Capabilities xmlns:wcs="http://www.opengis.net/wcs/2.0" xmlns:ows="http://www.opengis.net/ows/2.0" xmlns:xlink="http://www.w3.org/1999/xlink" version="2.0.1"><ows:ServiceIdentification><ows:Title>Synthetic public coverage</ows:Title><ows:Fees>NONE</ows:Fees><ows:AccessConstraints>Fixture only</ows:AccessConstraints></ows:ServiceIdentification><ows:ServiceProvider><ows:ProviderName>Synthetic fixture</ows:ProviderName></ows:ServiceProvider><ows:OperationsMetadata><ows:Operation name="GetCapabilities"><ows:DCP><ows:HTTP><ows:Get xlink:href="https://example.com/wcs?"/></ows:HTTP></ows:DCP></ows:Operation><ows:Operation name="DescribeCoverage"><ows:DCP><ows:HTTP><ows:Get xlink:href="https://example.com/wcs?"/></ows:HTTP></ows:DCP></ows:Operation><ows:Operation name="GetCoverage"><ows:DCP><ows:HTTP><ows:Get xlink:href="https://example.com/wcs?"/></ows:HTTP></ows:DCP></ows:Operation></ows:OperationsMetadata><wcs:ServiceMetadata><wcs:formatSupported>image/tiff</wcs:formatSupported></wcs:ServiceMetadata><wcs:Contents><wcs:CoverageSummary><ows:Title>Synthetic depth declaration</ows:Title><wcs:CoverageId>custom:coverage</wcs:CoverageId><wcs:CoverageSubtype>RectifiedGridCoverage</wcs:CoverageSubtype></wcs:CoverageSummary></wcs:Contents></wcs:Capabilities>"#.to_vec()
}
fn coverage() -> Vec<u8> {
    br#"<wcs:CoverageDescriptions xmlns:wcs="http://www.opengis.net/wcs/2.0" xmlns:gml="http://www.opengis.net/gml/3.2" xmlns:gmlcov="http://www.opengis.net/gmlcov/1.0" xmlns:swe="http://www.opengis.net/swe/2.0" xmlns:ows="http://www.opengis.net/ows/2.0" xmlns:xlink="http://www.w3.org/1999/xlink"><wcs:CoverageDescription gml:id="fixture"><gml:name>Synthetic coverage</gml:name><gml:boundedBy><gml:Envelope srsName="http://www.opengis.net/def/crs/EPSG/0/4326" srsDimension="2" axisLabels="Lat Long"><gml:lowerCorner>52 13</gml:lowerCorner><gml:upperCorner>54 15</gml:upperCorner></gml:Envelope></gml:boundedBy><wcs:CoverageId>custom:coverage</wcs:CoverageId><gmlcov:metadata><gmlcov:Extension><ows:Metadata xlink:href="https://example.com/product?version=1"/></gmlcov:Extension></gmlcov:metadata><gml:domainSet><gml:RectifiedGrid dimension="2"><gml:limits><gml:GridEnvelope><gml:low>0 0</gml:low><gml:high>1 1</gml:high></gml:GridEnvelope></gml:limits><gml:axisLabels>i j</gml:axisLabels><gml:origin><gml:Point srsName="http://www.opengis.net/def/crs/EPSG/0/4326"><gml:pos>53.5 13.5</gml:pos></gml:Point></gml:origin><gml:offsetVector>0 1</gml:offsetVector><gml:offsetVector>-1 0</gml:offsetVector></gml:RectifiedGrid></gml:domainSet><gmlcov:rangeType><swe:DataRecord><swe:field name="Depth"><swe:Quantity><swe:description>Uninterpreted source quantity</swe:description><swe:nilValues><swe:NilValues><swe:nilValue reason="unknown">NaN</swe:nilValue></swe:NilValues></swe:nilValues><swe:uom code="source-declared-unit"/></swe:Quantity></swe:field></swe:DataRecord></gmlcov:rangeType><wcs:ServiceParameters><wcs:CoverageSubtype>RectifiedGridCoverage</wcs:CoverageSubtype><wcs:nativeFormat>image/tiff</wcs:nativeFormat></wcs:ServiceParameters></wcs:CoverageDescription></wcs:CoverageDescriptions>"#.to_vec()
}
fn source() -> (Connection, DescriptionRecord) {
    let connection = xml::capabilities(
        &capabilities(),
        &Url::parse("https://example.com/wcs").unwrap(),
        "Fixture WCS",
        "00000000-0000-4000-8000-000000000001",
        "2026-10-02T00:00:00Z",
    )
    .unwrap();
    let record = DescriptionRecord {
        version: 1,
        connection: connection.clone(),
        coverage_id: "custom:coverage".into(),
        description_sha256: hash(&coverage()),
        description_url: request_url(
            &connection.describe_url,
            "DescribeCoverage",
            Some("custom:coverage"),
        )
        .unwrap()
        .to_string(),
        retrieved_at: "2026-10-02T00:00:00Z".into(),
    };
    (connection, record)
}
pub(super) fn fixture(root: &Path) -> SourcePin {
    let root = root.canonicalize().unwrap();
    std::fs::create_dir_all(root.join("wcs")).unwrap();
    let (_, record) = source();
    immutable(&root, "capabilities", "xml", &capabilities()).unwrap();
    immutable(&root, "coverage", "xml", &coverage()).unwrap();
    let description_id = immutable(
        &root,
        "description",
        "json",
        &serde_json::to_vec(&record).unwrap(),
    )
    .unwrap();
    let plan = PlanRecord {
        version: 1,
        description_id,
        bounds: [13., 52., 15., 54.],
    };
    SourcePin {
        plan_id: immutable(&root, "plan", "json", &serde_json::to_vec(&plan).unwrap()).unwrap(),
    }
}
fn tiny_plan() -> Plan {
    let (connection, record) = source();
    let description = xml::description(&coverage(), &record, &"a".repeat(64)).unwrap();
    grid::plan(
        &"b".repeat(64),
        description,
        [13., 52., 15., 54.],
        &connection,
    )
    .unwrap()
}
fn tiff(values: [f32; 4], shifted: bool, nodata: Option<&str>) -> Vec<u8> {
    let mut cursor = Cursor::new(Vec::new());
    {
        let mut encoder = TiffEncoder::new(&mut cursor).unwrap();
        let mut image = encoder.new_image::<colortype::Gray32Float>(2, 2).unwrap();
        image
            .encoder()
            .write_tag(
                Tag::GeoKeyDirectoryTag,
                &[
                    1u16, 1, 0, 3, 1024, 0, 1, 2, 1025, 0, 1, 1, 2048, 0, 1, 4326,
                ][..],
            )
            .unwrap();
        image
            .encoder()
            .write_tag(Tag::ModelPixelScaleTag, &[1f64, 1., 0.][..])
            .unwrap();
        image
            .encoder()
            .write_tag(
                Tag::ModelTiepointTag,
                &[0f64, 0., 0., if shifted { 13.5 } else { 13. }, 54., 0.][..],
            )
            .unwrap();
        if let Some(value) = nodata {
            image.encoder().write_tag(Tag::GdalNodata, value).unwrap();
        }
        image.write_data(&values).unwrap();
    }
    cursor.into_inner()
}
#[test]
fn lat_long_crs_axes_remain_distinct_from_grid_axes_and_raw_units() {
    let plan = tiny_plan();
    assert_eq!(plan.description.axis_labels, ["Lat", "Long"]);
    assert_eq!(plan.description.grid_axis_labels, ["i", "j"]);
    assert_eq!(plan.description.transform, [1., 0., 13., 0., -1., 54.]);
    assert_eq!(
        plan.description.fields[0].unit.as_deref(),
        Some("source-declared-unit")
    );
    let url = Url::parse(&plan.request_url).unwrap();
    let subsets = url
        .query_pairs()
        .filter(|(key, _)| key == "subset")
        .map(|(_, v)| v.into_owned())
        .collect::<Vec<_>>();
    assert_eq!(
        subsets,
        vec![
            "Lat(52.000000000000000,54.000000000000000)",
            "Long(13.000000000000000,15.000000000000000)"
        ]
    );
    assert!(!url
        .query_pairs()
        .any(|(k, _)| k.starts_with("scale") || k == "outputCrs"));
}
#[test]
fn off_grid_and_boundary_requests_cover_native_cells_without_scaling() {
    let (connection, record) = source();
    let description = xml::description(&coverage(), &record, &"a".repeat(64)).unwrap();
    let p = grid::plan(
        "test",
        description.clone(),
        [13.1, 52.1, 13.9, 52.9],
        &connection,
    )
    .unwrap();
    assert_eq!((p.width, p.height), (1, 1));
    assert_eq!(p.native_bounds, [13., 52., 14., 53.]);
    let p = grid::plan(
        "test",
        description.clone(),
        [13., 53., 14., 54.],
        &connection,
    )
    .unwrap();
    assert_eq!((p.width, p.height), (1, 1));
    assert_eq!(p.native_bounds, [13., 53., 14., 54.]);
    assert!(grid::plan("test", description, [16., 52., 17., 53.], &connection).is_err());
}
#[test]
fn nil_declaration_does_not_fabricate_file_nodata_and_shifted_grids_fail() {
    let plan = tiny_plan();
    let cancel = tokio_util::sync::CancellationToken::new();
    verify::tiff(
        Cursor::new(tiff([1., 2., 3., 4.], false, None)),
        &plan,
        &cancel,
    )
    .unwrap();
    verify::tiff(
        Cursor::new(tiff([f32::NAN, 2., 3., 4.], false, Some("nan"))),
        &plan,
        &cancel,
    )
    .unwrap();
    assert!(verify::tiff(
        Cursor::new(tiff([f32::NAN, 2., 3., 4.], false, None)),
        &plan,
        &cancel
    )
    .is_err());
    assert!(verify::tiff(
        Cursor::new(tiff([1., 2., 3., 4.], true, None)),
        &plan,
        &cancel
    )
    .is_err());
    assert!(verify::tiff(
        Cursor::new(tiff([1., 2., 3., 4.], false, Some("-9999"))),
        &plan,
        &cancel
    )
    .is_err());
}
#[test]
fn tiff_validation_honors_cancellation_and_rejects_uncomparable_nil() {
    let cancel = tokio_util::sync::CancellationToken::new();
    cancel.cancel();
    assert!(
        verify::tiff(Cursor::new(Vec::<u8>::new()), &tiny_plan(), &cancel)
            .unwrap_err()
            .contains("cancelled")
    );
    let mut plan = tiny_plan();
    plan.description.fields[0].nil_values[0].value = "unknown-value".into();
    assert!(verify::tiff(
        Cursor::new(tiff([1., 2., 3., 4.], false, None)),
        &plan,
        &tokio_util::sync::CancellationToken::new()
    )
    .unwrap_err()
    .contains("numeric literal"));
    plan.description.fields[0].nil_values[0].value = "1e100".into();
    assert!(verify::tiff(
        Cursor::new(tiff([1., 2., 3., 4.], false, None)),
        &plan,
        &tokio_util::sync::CancellationToken::new()
    )
    .unwrap_err()
    .contains("overflows"));
}
#[test]
fn metadata_gzip_is_bounded_and_retains_decoded_xml_exactly() {
    fn gzip(bytes: &[u8]) -> Vec<u8> {
        let mut encoder = flate2::write::GzEncoder::new(Vec::new(), flate2::Compression::fast());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }
    let xml = coverage();
    let compressed = gzip(&xml);
    assert_eq!(decode_xml(compressed.clone(), "gzip").unwrap(), xml);
    assert_eq!(decode_xml(xml.clone(), "identity").unwrap(), xml);
    assert!(decode_xml(compressed[..compressed.len() - 4].to_vec(), "gzip").is_err());
    assert!(decode_xml(gzip(&vec![b' '; MAX_XML + 1]), "gzip")
        .unwrap_err()
        .contains("8 MiB"));
    assert!(decode_xml(xml, "br")
        .unwrap_err()
        .contains("unsupported content encoding"));
    let deep = format!("{}{}", "<n>".repeat(129), "</n>".repeat(129));
    assert!(xml::fingerprint(deep.as_bytes())
        .unwrap_err()
        .contains("nesting"));
}
#[test]
fn saved_plan_rederives_from_hashed_xml_and_survives_registry_removal() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().canonicalize().unwrap();
    let pin = fixture(&root);
    let selected = resolve(&root, &pin).unwrap();
    assert_eq!(selected.coverage_id, "custom:coverage");
    assert_eq!(selected.bounds, [13., 52., 15., 54.]);
    assert!(same_selection(&root, &pin, &pin).unwrap());
    let p = plan(&root, &pin.plan_id).unwrap();
    std::fs::write(
        root.join("wcs")
            .join(format!("coverage-{}.xml", p.description.description_sha256)),
        b"changed",
    )
    .unwrap();
    assert!(resolve(&root, &pin).is_err());
}
#[test]
fn malformed_or_extra_dimensions_and_grid_arithmetic_are_rejected() {
    let (_, record) = source();
    let original = String::from_utf8(coverage()).unwrap();
    for changed in [
        original.replace("srsDimension=\"2\"", "srsDimension=\"3\""),
        original.replace(
            "<gml:offsetVector>0 1</gml:offsetVector>",
            "<gml:offsetVector>0.1 1</gml:offsetVector>",
        ),
        original.replace(
            "<gml:high>1 1</gml:high>",
            "<gml:high>9223372036854775807 1</gml:high>",
        ),
    ] {
        assert!(xml::description(changed.as_bytes(), &record, "test").is_err());
    }
    assert!(grid::definition(
        [i64::MIN, 0],
        [i64::MAX, 1],
        [0., 0.],
        [[1., 0.], [0., -1.]],
        [0., 0.],
        [1., 1.],
        false
    )
    .is_err());
    assert!(xml::document(b"<!DOCTYPE a><a/>").is_err());
}
#[test]
fn operation_urls_allow_only_exact_http_to_connected_https_upgrade() {
    let base = Url::parse("https://example.com/wcs").unwrap();
    assert_eq!(endpoint(&base, "http://example.com/wcs?").unwrap(), base);
    for url in [
        "http://example.com/other",
        "https://other.example/wcs",
        "https://example.com/wcs?token=secret",
        "https://127.0.0.1/wcs",
    ] {
        assert!(endpoint(&base, url).is_err());
    }
    let raw = String::from_utf8(capabilities())
        .unwrap()
        .replace("https://example.com/wcs?", "http://example.com/wcs?");
    assert!(xml::capabilities(raw.as_bytes(), &base, "name", "id", "time").is_ok());
}
#[test]
fn structure_fingerprint_ignores_layout_but_retains_nil_units_and_grid() {
    let raw = coverage();
    let formatted = String::from_utf8(raw.clone())
        .unwrap()
        .replace("><", ">\n<!-- layout -->\n<");
    assert_eq!(
        xml::fingerprint(&raw).unwrap(),
        xml::fingerprint(formatted.as_bytes()).unwrap()
    );
    assert_ne!(
        xml::fingerprint(&raw).unwrap(),
        xml::fingerprint(
            &String::from_utf8(raw)
                .unwrap()
                .replace("source-declared-unit", "m")
                .into_bytes()
        )
        .unwrap()
    );
}
#[test]
fn projected_and_swapped_grid_definitions_are_supported_without_axis_guessing() {
    assert_eq!(
        grid::crs("urn:ogc:def:crs:EPSG::4326").unwrap(),
        ("EPSG:4326".into(), true)
    );
    assert_eq!(
        grid::crs("http://www.opengis.net/def/crs/OGC/1.3/CRS84").unwrap(),
        ("EPSG:4326".into(), false)
    );
    let (w, h, transform, _) = grid::definition(
        [5, 7],
        [6, 9],
        [105., 195.],
        [[0., -10.], [10., 0.]],
        [170., 130.],
        [200., 150.],
        false,
    )
    .unwrap();
    assert_eq!((w, h), (3, 2));
    assert_eq!(transform, [10., 0., 170., 0., -10., 150.]);
    let mercator = grid::envelope([13., 52., 14., 53.], "EPSG:3857", true).unwrap();
    let result = grid::envelope(mercator, "EPSG:3857", false).unwrap();
    for (a, b) in result.into_iter().zip([13., 52., 14., 53.]) {
        assert!((a - b).abs() < 1e-10);
    }
}
