use super::*;

// Handmade protocol fixtures only. These tests never contact a public WFS.
const ROOT: &str = "https://example.com/wfs";
const BOUNDS: [f64; 4] = [13.2, 52.4, 13.6, 52.6];
const TIME: &str = "2026-10-02T14:00:00Z";

fn type_xml(name: &str, formats: &str) -> String {
    format!(
        r#"<wfs:FeatureType><wfs:Name xmlns:sample="urn:synthetic:features">sample:{name}</wfs:Name><wfs:Title>Synthetic {name}</wfs:Title><wfs:Abstract>Handmade test source</wfs:Abstract><wfs:DefaultCRS>urn:ogc:def:crs:EPSG::4326</wfs:DefaultCRS>{formats}</wfs:FeatureType>"#
    )
}

fn capabilities() -> String {
    let operations = ["GetCapabilities", "DescribeFeatureType", "GetFeature"].iter().map(|name| {
        let formats = if *name == "GetFeature" {
            r#"<ows:Parameter name="outputFormat"><ows:AllowedValues><ows:Value>application/json</ows:Value><ows:Value>application/gml+xml; version=3.2</ows:Value><ows:Value>text/xml; subtype=gml/3.2.1</ows:Value></ows:AllowedValues></ows:Parameter>"#
        } else { "" };
        format!(r#"<ows:Operation name="{name}"><ows:DCP><ows:HTTP><ows:Get xlink:href="{ROOT}"/></ows:HTTP></ows:DCP>{formats}</ows:Operation>"#)
    }).collect::<String>();
    let global = type_xml("places", "");
    let local = type_xml(
        "json_only",
        r#"<wfs:OutputFormats><wfs:Format>application/geo+json</wfs:Format></wfs:OutputFormats>"#,
    );
    let unsupported = type_xml(
        "unsupported",
        r#"<wfs:OutputFormats><wfs:Format>SHAPE-ZIP</wfs:Format></wfs:OutputFormats>"#,
    );
    format!(
        r#"<wfs:WFS_Capabilities xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:ows="http://www.opengis.net/ows/1.1" xmlns:xlink="http://www.w3.org/1999/xlink" version="2.0.0"><ows:ServiceIdentification><ows:Title>Synthetic WFS</ows:Title><ows:Fees>NONE</ows:Fees><ows:AccessConstraints>Test-only source declaration</ows:AccessConstraints></ows:ServiceIdentification><ows:OperationsMetadata>{operations}<ows:Constraint name="ImplementsResultPaging"><ows:DefaultValue>TRUE</ows:DefaultValue></ows:Constraint></ows:OperationsMetadata><wfs:FeatureTypeList>{global}{local}{unsupported}</wfs:FeatureTypeList></wfs:WFS_Capabilities>"#
    )
}

fn service_fixture() -> FeatureService {
    parse_capabilities(&capabilities(), &Url::parse(ROOT).unwrap(), "Synthetic WFS").unwrap()
}

fn layer_fixture() -> Layer {
    service_fixture().collections[0].wfs.clone().unwrap()
}

#[test]
fn discovery_resolves_scoped_qnames_and_local_formats_without_inventing_license() {
    let raw = capabilities();
    let service = service_fixture();
    assert_eq!(service.collections.len(), 2);
    let layer = service.collections[0].wfs.as_ref().unwrap();
    assert_eq!(layer.type_name, "sample:places");
    assert_eq!(layer.namespace, "urn:synthetic:features");
    assert_eq!(layer.default_format, "gml32");
    assert_eq!(
        layer.formats.len(),
        2,
        "MIME aliases must not create duplicate format choices"
    );
    let json_layer = service.collections[1].wfs.as_ref().unwrap();
    assert_eq!(json_layer.default_format, "geojson");
    assert_eq!(
        json_layer.formats.len(),
        1,
        "local output formats override global operation choices"
    );
    let metadata = service.wfs.as_ref().unwrap();
    assert_eq!(metadata.capabilities_sha256, hash(raw.as_bytes()));
    assert!(metadata.paging_supported);
    assert_eq!(metadata.fees, "NONE");
    assert_eq!(metadata.access_constraints, "Test-only source declaration");
    assert_eq!(metadata.excluded_layers.len(), 1);
    assert_eq!(metadata.excluded_layers[0].id, "sample:unsupported");
    assert!(service
        .collections
        .iter()
        .all(|c| c.license_links.is_empty()));
    validate_service(&service).unwrap();
}

#[test]
fn xml_prefix_spelling_is_irrelevant_but_namespace_spoofing_is_rejected() {
    let renamed = capabilities()
        .replace("wfs:", "protocol:")
        .replace("xmlns:wfs=", "xmlns:protocol=")
        .replace("ows:", "metadata:")
        .replace("xmlns:ows=", "xmlns:metadata=");
    let parsed = parse_capabilities(&renamed, &Url::parse(ROOT).unwrap(), "Synthetic").unwrap();
    assert_eq!(parsed.collections.len(), 2);
    let spoofed =
        capabilities().replace("http://www.opengis.net/wfs/2.0", "urn:synthetic:wrong-wfs");
    assert!(parse_capabilities(&spoofed, &Url::parse(ROOT).unwrap(), "Synthetic").is_err());
    let unresolved = capabilities().replace("xmlns:sample=\"urn:synthetic:features\"", "");
    let parsed = parse_capabilities(&unresolved, &Url::parse(ROOT).unwrap(), "Synthetic").unwrap();
    assert!(parsed.collections.is_empty());
    assert_eq!(parsed.wfs.unwrap().excluded_layers.len(), 3);
}

#[test]
fn discovery_rejects_foreign_operations_duplicate_types_and_service_exceptions() {
    let root = Url::parse(ROOT).unwrap();
    for bad in [
        capabilities().replace(ROOT, "https://other.example/wfs"),
        capabilities().replace("sample:json_only", "sample:places"),
        capabilities().replace("version=\"2.0.0\"", "version=\"1.1.0\""),
        r#"<ows:ExceptionReport xmlns:ows="http://www.opengis.net/ows/1.1"><ows:Exception exceptionCode="InvalidParameterValue"/></ows:ExceptionReport>"#.into(),
    ] {
        assert!(parse_capabilities(&bad, &root, "Synthetic").is_err());
    }
    // A reverse proxy's HTTP advertisement never changes the configured HTTPS endpoint.
    let advertised_http = capabilities().replace(ROOT, "http://example.com/wfs");
    let service = parse_capabilities(&advertised_http, &root, "Synthetic").unwrap();
    assert_eq!(service.url, ROOT);
    assert!(service.collections.iter().all(|c| c.items_url == ROOT));
}

#[test]
fn pasted_capabilities_urls_are_normalized_without_accepting_query_credentials_or_overrides() {
    let pasted = format!("{ROOT}?SERVICE=WFS&REQUEST=GetCapabilities&VERSION=2.0.0");
    assert_eq!(endpoint(&pasted, true).unwrap().as_str(), ROOT);
    assert!(endpoint(&pasted, false).is_err());
    for suffix in [
        "?service=WFS&service=WFS",
        "?service=WFS&SERVICE=WFS",
        "?token=synthetic",
        "?request=GetFeature",
        "?version=1.1.0",
        "?service=WFS&bbox=0,0,1,1",
    ] {
        assert!(
            endpoint(&format!("{ROOT}{suffix}"), true).is_err(),
            "{suffix}"
        );
    }
    assert!(endpoint("https://user:password@example.com/wfs", true).is_err());
}

#[test]
fn geographic_aoi_becomes_explicit_latitude_first_bbox_without_property_projection() {
    let layer = layer_fixture();
    let format = layer.formats.iter().find(|f| f.id == "gml32").unwrap();
    let request = feature_url(
        &Url::parse(ROOT).unwrap(),
        &layer,
        BOUNDS,
        Some(format),
        3,
        2,
        Some("id"),
    );
    let pairs = request
        .query_pairs()
        .into_owned()
        .collect::<BTreeMap<_, _>>();
    assert_eq!(
        pairs["bbox"],
        "52.4,13.2,52.6,13.6,urn:ogc:def:crs:EPSG::4326"
    );
    assert_eq!(pairs["srsName"], EPSG4326);
    assert_eq!(pairs["startIndex"], "3");
    assert_eq!(pairs["count"], "2");
    assert_eq!(pairs["sortBy"], "id A");
    assert_eq!(pairs["typeNames"], "sample:places");
    assert!(!pairs.keys().any(|k| k.eq_ignore_ascii_case("propertyName")));
    let hits = feature_url(&Url::parse(ROOT).unwrap(), &layer, BOUNDS, None, 0, 0, None);
    let pairs = hits.query_pairs().into_owned().collect::<BTreeMap<_, _>>();
    assert_eq!(pairs["resultType"], "hits");
    assert!(!pairs.contains_key("count"));
    assert!(!pairs.contains_key("startIndex"));
}

#[test]
fn persisted_service_metadata_is_revalidated_before_reuse() {
    let source = service_fixture();
    let mut bad = source.clone();
    bad.wfs.as_mut().unwrap().capabilities_sha256 = "not-a-digest".into();
    assert!(validate_service(&bad).is_err());
    let mut bad = source.clone();
    bad.collections[0].items_url = "https://other.example/wfs".into();
    assert!(validate_service(&bad).is_err());
    let mut bad = source.clone();
    bad.collections[0].wfs.as_mut().unwrap().default_format = "geojson".into();
    assert!(validate_service(&bad).is_err());
    let mut bad = source;
    bad.collections[0].wfs.as_mut().unwrap().formats[0].mime = "text/html".into();
    assert!(validate_service(&bad).is_err());
}

fn schema_xml() -> String {
    r#"<xsd:schema xmlns:xsd="http://www.w3.org/2001/XMLSchema" xmlns:gml="http://www.opengis.net/gml/3.2" xmlns:sample="urn:synthetic:features" targetNamespace="urn:synthetic:features" elementFormDefault="qualified"><xsd:import namespace="http://www.opengis.net/gml/3.2" schemaLocation="https://example.com/gml.xsd"/><xsd:complexType name="placesType"><xsd:complexContent><xsd:extension base="gml:AbstractFeatureType"><xsd:sequence><xsd:element name="id" type="xsd:long"/><xsd:element name="name" type="xsd:string" minOccurs="0" nillable="true"/><xsd:element name="geom" type="gml:PointPropertyType"/></xsd:sequence></xsd:extension></xsd:complexContent></xsd:complexType><xsd:element name="places" type="sample:placesType" substitutionGroup="gml:AbstractFeature"/></xsd:schema>"#.into()
}

fn schema_fixture() -> Schema {
    schema::parse_schema(&schema_xml(), "sample:places", "urn:synthetic:features").unwrap()
}

fn feature_fixture(fid: &str, source_key: i64) -> Value {
    json!({"type":"Feature", "id":fid, "properties":{"id":source_key, "name":"Synthetic 地点"},
        "geometry":{"type":"Point", "coordinates":[13.4,52.5]}})
}

fn json_document(features: Vec<Value>, matched: Value) -> Value {
    json!({"type":"FeatureCollection", "numberMatched":matched, "numberReturned":features.len(),
        "timeStamp":TIME, "crs":{"type":"name","properties":{"name":EPSG4326}}, "features":features})
}

fn page_fixture(features: Vec<Value>, matched: Value) -> Page {
    json_page(
        &json_document(features, matched).to_string(),
        &schema_fixture(),
    )
    .unwrap()
}

fn hits_fixture(count: &str) -> String {
    format!(
        r#"<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:gml="http://www.opengis.net/gml/3.2" timeStamp="{TIME}" numberMatched="{count}" numberReturned="0"/>"#
    )
}

#[test]
fn geojson_preserves_original_ids_properties_and_longitude_first_coordinates() {
    let schema = schema_fixture();
    let first = feature_fixture("generated.server-fid.1", 2734);
    let original = json_document(vec![first.clone()], json!(1));
    let page = json_page(&original.to_string(), &schema).unwrap();
    assert_eq!(page.features, vec![first]);
    assert_eq!(page.number_matched, Some(1));
    assert_eq!(page.number_returned, 1);
    assert_eq!(
        page.features[0]["geometry"]["coordinates"],
        json!([13.4, 52.5]),
        "GeoJSON axes must not be swapped because its CRS string contains EPSG4326"
    );
    assert_eq!(sort_field(&schema), Some("id".into()));
}

#[test]
fn transport_does_not_claim_requested_crs_when_gml_returns_another_axis_order() {
    let schema = schema_fixture();
    let format = Format {
        id: "gml32".into(),
        mime: "application/gml+xml; version=3.2".into(),
    };
    let raw = format!(
        r#"<wfs:FeatureCollection xmlns:wfs="http://www.opengis.net/wfs/2.0" xmlns:gml="http://www.opengis.net/gml/3.2" xmlns:sample="urn:synthetic:features" timeStamp="{TIME}" numberMatched="1" numberReturned="1"><wfs:member><sample:places gml:id="places.1"><sample:id>1</sample:id><sample:geom><gml:Point srsName="{EPSG4326}"><gml:pos>52.5 13.4</gml:pos></gml:Point></sample:geom></sample:places></wfs:member></wfs:FeatureCollection>"#
    );
    assert_eq!(
        parse_page(&raw, &schema, &format).unwrap().features[0]["geometry"]["coordinates"],
        json!([13.4, 52.5])
    );
    let changed = raw
        .replace(EPSG4326, OUTPUT_CRS84)
        .replace("52.5 13.4", "13.4 52.5");
    assert!(
        gml::parse_page(&changed, &schema).is_ok(),
        "general reader can understand CRS84"
    );
    assert!(
        parse_page(&changed, &schema, &format).is_err(),
        "transport must match its recorded response CRS"
    );
}

#[test]
fn geojson_missing_identity_conflicting_counts_unknown_fields_and_precision_loss_fail() {
    let schema = schema_fixture();
    let original = json_document(vec![feature_fixture("places.a", 1)], json!(1));
    type Mutation = (&'static str, fn(&mut Value));
    let mutations: &[Mutation] = &[
        ("missing source ID", |v| {
            v["features"][0].as_object_mut().unwrap().remove("id");
        }),
        ("returned count mismatch", |v| {
            v["numberReturned"] = json!(2);
        }),
        ("contradictory totals", |v| {
            v["totalFeatures"] = json!(2);
        }),
        ("attribute absent from schema", |v| {
            v["features"][0]["properties"]["extra"] = json!(true);
        }),
        ("integer precision loss", |v| {
            v["features"][0]["properties"]["id"] = json!(9_007_199_254_740_993_u64);
        }),
        ("unexpected projected CRS", |v| {
            v["crs"]["properties"]["name"] = json!("EPSG:3857");
        }),
        ("third coordinate discarded", |v| {
            v["features"][0]["geometry"]["coordinates"] = json!([13.4, 52.5, 20.0]);
        }),
    ];
    for (label, change) in mutations {
        let mut bad = original.clone();
        change(&mut bad);
        assert!(json_page(&bad.to_string(), &schema).is_err(), "{label}");
    }
}

#[test]
fn unknown_page_totals_need_numeric_hits_and_unique_ascending_complete_assembly() {
    let schema = schema_fixture();
    assert_eq!(hits(&hits_fixture("2"), &schema).unwrap(), 2);
    assert!(hits(&hits_fixture("unknown"), &schema).is_err());
    let mut result = Assembly::default();
    result
        .append(
            page_fixture(vec![feature_fixture("places.a", 1)], json!("unknown")),
            2,
            1,
            Some("id"),
        )
        .unwrap();
    result
        .append(
            page_fixture(vec![feature_fixture("places.b", 2)], json!(2)),
            2,
            1,
            Some("id"),
        )
        .unwrap();
    assert_eq!(result.features.len(), 2);
    for (fid, key, matched) in [
        ("places.a", 2, 2),
        ("places.b", 1, 2),
        ("places.b", 0, 2),
        ("places.b", 2, 3),
    ] {
        let mut bad = Assembly::default();
        bad.append(
            page_fixture(vec![feature_fixture("places.a", 1)], json!(2)),
            2,
            1,
            Some("id"),
        )
        .unwrap();
        assert!(bad
            .append(
                page_fixture(vec![feature_fixture(fid, key)], json!(matched)),
                2,
                1,
                Some("id")
            )
            .is_err());
    }
    assert!(Assembly::default()
        .append(page_fixture(vec![], json!(2)), 2, 1, Some("id"))
        .is_err());
}

#[test]
fn second_read_may_change_server_fids_only_when_source_key_and_all_values_match() {
    let first = vec![feature_fixture("server-fid.first", 2734)];
    let second = vec![feature_fixture("server-fid.second", 2734)];
    compare_passes(&first, &second, Some("id")).unwrap();
    assert_eq!(
        first[0]["id"], "server-fid.first",
        "verification must not replace the original source ID"
    );
    assert!(compare_passes(&first, &second, None).is_err());
    for change in [0, 1, 2] {
        let mut changed = second.clone();
        match change {
            0 => changed[0]["properties"]["id"] = json!(2735),
            1 => changed[0]["properties"]["name"] = json!("Changed while paging"),
            _ => changed[0]["geometry"]["coordinates"] = json!([13.41, 52.5]),
        }
        assert!(compare_passes(&first, &changed, Some("id")).is_err());
    }
    assert!(compare_passes(&first, &[], Some("id")).is_err());
}

fn receipt(raw: &str, url: Url, returned: usize) -> PageReceipt {
    PageReceipt {
        url: url.to_string(),
        sha256: hash(raw.as_bytes()),
        bytes: raw.len(),
        returned,
        parameters: None,
    }
}

fn bundle_fixture(empty: bool) -> (Value, Provenance) {
    let service = service_fixture();
    let layer = layer_fixture();
    let format = layer
        .formats
        .iter()
        .find(|f| f.id == "geojson")
        .unwrap()
        .clone();
    let schema_text = schema_xml();
    let schema = schema_fixture();
    let count = if empty { 0 } else { 1 };
    let hits = hits_fixture(&count.to_string());
    let first = json_document(
        vec![feature_fixture("server-fid.first", 2734)],
        json!(count),
    )
    .to_string();
    let second = json_document(
        vec![feature_fixture("server-fid.second", 2734)],
        json!(count),
    )
    .to_string();
    let root = Url::parse(ROOT).unwrap();
    let page_url = feature_url(&root, &layer, BOUNDS, Some(&format), 0, 2, Some("id"));
    let pages = if empty {
        vec![]
    } else {
        vec![receipt(&first, page_url.clone(), 1)]
    };
    let verification_pages = if empty {
        vec![]
    } else {
        vec![receipt(&second, page_url, 1)]
    };
    let metadata = service.wfs.unwrap();
    let source = Provenance {
        service_url: ROOT.into(),
        service_name: "Synthetic WFS".into(),
        collection_id: "sample:places".into(),
        collection_title: "Synthetic places".into(),
        license_links: vec![],
        requested_bounds: BOUNDS,
        area_geometry: None,
        requested_at: TIME.into(),
        pages,
        number_matched: Some(count),
        feature_count: count,
        selection: "wfs-bbox-full-features".into(),
        arcgis: None,
        wfs: Some(Snapshot {
            version: "2.0.0".into(),
            layer: layer.clone(),
            format,
            request_crs: EPSG4326.into(),
            response_crs: OUTPUT_CRS84.into(),
            capabilities_sha256: metadata.capabilities_sha256,
            schema,
            schema_receipt: receipt(&schema_text, schema_url(&root, &layer), 0),
            schema_after_receipt: receipt(&schema_text, schema_url(&root, &layer), 0),
            hits_before: receipt(
                &hits,
                feature_url(&root, &layer, BOUNDS, None, 0, 0, None),
                0,
            ),
            hits_after: receipt(
                &hits,
                feature_url(&root, &layer, BOUNDS, None, 0, 0, None),
                0,
            ),
            matched_count: count,
            paging_supported: true,
            raw_archive_version: 1,
            fees: metadata.fees,
            access_constraints: metadata.access_constraints,
            sort_field: Some("id".into()),
            page_size: 2,
            verification_pages,
        }),
    };
    let archive = Archive {
        version: 1,
        schema_xml: schema_text.clone(),
        schema_after_xml: schema_text,
        hits_before_xml: hits.clone(),
        hits_after_xml: hits,
        pages: if empty { vec![] } else { vec![first] },
        verification_pages: if empty { vec![] } else { vec![second] },
    };
    (serde_json::to_value(archive).unwrap(), source)
}

#[test]
fn source_archive_reopens_original_features_and_valid_complete_empty_queries() {
    for empty in [false, true] {
        let (archive, source) = bundle_fixture(empty);
        let decoded = decode_bundle(&archive, &source).unwrap();
        assert_eq!(
            decoded["features"].as_array().unwrap().len(),
            source.feature_count
        );
        assert_eq!(
            decoded["geodSource"],
            serde_json::to_value(&source).unwrap()
        );
        if !empty {
            assert_eq!(
                decoded["features"][0],
                feature_fixture("server-fid.first", 2734)
            );
            assert_ne!(archive["pages"], archive["verificationPages"]);
        }
    }
}

#[test]
fn source_archive_checks_full_schema_definition_beyond_the_field_summary() {
    let (mut archive, mut source) = bundle_fixture(false);
    let original = archive["schemaXml"].as_str().unwrap().replace(
        "type=\"xsd:string\" minOccurs=\"0\" nillable=\"true\"/>",
        "minOccurs=\"0\" nillable=\"true\"><xsd:simpleType><xsd:restriction base=\"xsd:string\"><xsd:maxLength value=\"100\"/></xsd:restriction></xsd:simpleType></xsd:element>");
    archive["schemaXml"] = json!(original);
    archive["schemaAfterXml"] = json!(original);
    let snapshot = source.wfs.as_mut().unwrap();
    snapshot.schema =
        schema::parse_schema(&original, "sample:places", "urn:synthetic:features").unwrap();
    for receipt in [
        &mut snapshot.schema_receipt,
        &mut snapshot.schema_after_receipt,
    ] {
        receipt.sha256 = hash(original.as_bytes());
        receipt.bytes = original.len();
    }
    assert!(decode_bundle(&archive, &source).is_ok());
    let formatted = original.replace("><", ">\n<!-- formatting only -->\n<");
    archive["schemaAfterXml"] = json!(formatted);
    let after = &mut source.wfs.as_mut().unwrap().schema_after_receipt;
    after.sha256 = hash(formatted.as_bytes());
    after.bytes = formatted.len();
    assert!(decode_bundle(&archive, &source).is_ok());
    let changed = original.replace("value=\"100\"", "value=\"10\"");
    archive["schemaAfterXml"] = json!(changed);
    let after = &mut source.wfs.as_mut().unwrap().schema_after_receipt;
    after.sha256 = hash(changed.as_bytes());
    after.bytes = changed.len();
    assert!(
        decode_bundle(&archive, &source).is_err(),
        "same field summary must not hide changed XSD facets"
    );
}

#[test]
fn source_archive_rejects_raw_document_receipt_query_and_verification_tampering() {
    let (archive, source) = bundle_fixture(false);
    for field in [
        "schemaXml",
        "schemaAfterXml",
        "hitsBeforeXml",
        "hitsAfterXml",
    ] {
        let mut changed = archive.clone();
        changed[field] = Value::String(format!("{} ", changed[field].as_str().unwrap()));
        assert!(decode_bundle(&changed, &source).is_err(), "{field}");
    }
    let mut missing_pass = archive.clone();
    missing_pass["verificationPages"] = json!([]);
    assert!(decode_bundle(&missing_pass, &source).is_err());
    let mut source_bad = source.clone();
    source_bad.pages[0].url.push_str("&propertyName=name");
    assert!(decode_bundle(&archive, &source_bad).is_err());
    let mut source_bad = source.clone();
    source_bad.wfs.as_mut().unwrap().sort_field = None;
    assert!(decode_bundle(&archive, &source_bad).is_err());
    // Recomputing a receipt is insufficient if the second read changed content.
    let mut changed = archive.clone();
    let mut second: Value =
        serde_json::from_str(changed["verificationPages"][0].as_str().unwrap()).unwrap();
    second["features"][0]["geometry"]["coordinates"] = json!([13.41, 52.5]);
    let text = second.to_string();
    changed["verificationPages"][0] = json!(text);
    let mut source_bad = source;
    let metadata = source_bad.wfs.as_mut().unwrap();
    metadata.verification_pages[0].sha256 = hash(text.as_bytes());
    metadata.verification_pages[0].bytes = text.len();
    assert!(decode_bundle(&changed, &source_bad).is_err());
}

#[tokio::test]
async fn managed_original_wfs_archive_reopens_and_exports_after_connection_removal() {
    let storage = tempfile::tempdir().unwrap();
    let destination = tempfile::tempdir().unwrap();
    let manager = JobManager::open(storage.path()).await.unwrap();
    let (archive, source) = bundle_fixture(false);
    let bytes = serde_json::to_vec_pretty(&archive).unwrap();
    let asset = manager
        .import_wfs_source(bytes.clone(), source.clone())
        .await
        .unwrap();
    assert_eq!(asset.format, "wfs-snapshot");
    assert_eq!(asset.storage_mode, "managed");
    assert_eq!(asset.feature_count, 1);
    assert_eq!(asset.coordinate_count, 1);
    assert_eq!(asset.source_sha256, hash(&bytes));
    assert_eq!(asset.bytes, bytes.len());
    assert_eq!(asset.remote_source, Some(source));
    assert!(asset.osm_source.is_none());
    let original = storage
        .path()
        .join("vectors")
        .join(format!("{}.json", asset.id));
    assert_eq!(tokio::fs::read(&original).await.unwrap(), bytes);
    let first = manager.inspect_vector(&asset.id).await.unwrap().geojson;
    assert_eq!(
        first["features"][0],
        feature_fixture("server-fid.first", 2734)
    );
    let service = service_fixture();
    manager
        .persist_feature_services(&BTreeMap::from([(service.id.clone(), service.clone())]))
        .await
        .unwrap();
    drop(manager);

    let manager = JobManager::open(storage.path()).await.unwrap();
    assert_eq!(manager.list_feature_services().await, vec![service.clone()]);
    assert_eq!(
        manager.inspect_vector(&asset.id).await.unwrap().geojson,
        first
    );
    manager.forget_feature_service(&service.id).await.unwrap();
    assert!(manager.list_feature_services().await.is_empty());
    let output = destination.path().join("synthetic-wfs.geojson");
    manager
        .export_vector_path(&asset.id, output.clone())
        .await
        .unwrap();
    let exported_bytes = tokio::fs::read(&output).await.unwrap();
    assert_eq!(hash(&exported_bytes), asset.geojson_sha256);
    assert_eq!(
        serde_json::from_slice::<Value>(&exported_bytes).unwrap(),
        first
    );
    assert_eq!(tokio::fs::read(&original).await.unwrap(), bytes);
    let reimported = manager
        .import_vector(vector::ImportVectorRequest {
            name: "Synthetic untrusted reimport".into(),
            text: String::from_utf8(exported_bytes).unwrap(),
        })
        .await
        .unwrap();
    assert!(
        reimported.remote_source.is_none(),
        "GeoJSON text cannot forge a trusted WFS snapshot"
    );
    assert_eq!(
        manager
            .inspect_vector(&reimported.id)
            .await
            .unwrap()
            .geojson,
        first
    );
    let mut corrupt = bytes;
    corrupt.push(b'\n');
    tokio::fs::write(&original, corrupt).await.unwrap();
    assert!(manager.inspect_vector(&asset.id).await.is_err());
}

#[tokio::test]
async fn invalid_wfs_archive_never_registers_partial_asset_or_backing_file() {
    let storage = tempfile::tempdir().unwrap();
    let manager = JobManager::open(storage.path()).await.unwrap();
    let (mut archive, source) = bundle_fixture(false);
    archive["verificationPages"] = json!([]);
    assert!(manager
        .import_wfs_source(serde_json::to_vec(&archive).unwrap(), source)
        .await
        .is_err());
    assert!(manager.list_vectors().await.is_empty());
    let mut entries = tokio::fs::read_dir(storage.path().join("vectors"))
        .await
        .unwrap();
    assert!(entries.next_entry().await.unwrap().is_none());
}
