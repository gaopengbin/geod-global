use super::*;
const ROOT: &str = "https://maps.example.com/wmts/1.0.0/capabilities.xml";
const TEMPLATE: &str = "../tiles/{Style}/{TileMatrixSet}/{TileMatrix}/{TileRow}/{TileCol}.png";
const XML: &str = r#"<Capabilities xmlns="http://www.opengis.net/wmts/1.0" xmlns:ows="http://www.opengis.net/ows/1.1" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.0.0"><ows:ServiceIdentification><ows:Title>REST fixture</ows:Title></ows:ServiceIdentification><Contents><Layer><ows:Identifier>land</ows:Identifier><Style isDefault="true"><ows:Identifier>default</ows:Identifier></Style><Format>image/png</Format><TileMatrixSetLink><TileMatrixSet>regional</TileMatrixSet></TileMatrixSetLink>RESOURCE</Layer><TileMatrixSet><ows:Identifier>regional</ows:Identifier><ows:SupportedCRS>CRS:84</ows:SupportedCRS><TileMatrix><ows:Identifier>opaque-level</ows:Identifier><ScaleDenominator>397569609.982</ScaleDenominator><TopLeftCorner>-180 90</TopLeftCorner><TileWidth>2</TileWidth><TileHeight>2</TileHeight><MatrixWidth>4</MatrixWidth><MatrixHeight>4</MatrixHeight></TileMatrix></TileMatrixSet></Contents></Capabilities>"#;
fn document(template: &str) -> String {
    XML.replace(
        "RESOURCE",
        &format!(r#"<ResourceURL resourceType="tile" format="image/png" template="{template}"/>"#),
    )
}
fn service_fixture() -> MapService {
    parse_capabilities(
        document(TEMPLATE).as_bytes(),
        &Url::parse(ROOT).unwrap(),
        "REST",
    )
    .unwrap()
}
#[test]
fn rest_only_discovery_uses_advertised_relative_resource_and_opaque_grid() {
    let s = service_fixture();
    assert_eq!(s.map_url, ROOT);
    assert!(s.wmts.as_ref().unwrap().rest_only);
    let w = s.layers[0].wmts.as_ref().unwrap();
    assert_eq!(w.resource_url.as_deref(), Some("https://maps.example.com/wmts/tiles/{Style}/{TileMatrixSet}/{TileMatrix}/{TileRow}/{TileCol}.png"));
    assert_eq!(
        s.wmts.as_ref().unwrap().matrix_sets[0].matrices[0].id,
        "opaque-level"
    );
    let mut changed = s;
    changed.layers[0].wmts.as_mut().unwrap().resource_url = None;
    assert!(validate_service(&changed).is_err());
}
#[test]
fn templates_reject_unsafe_origin_authority_and_unsupported_dimensions() {
    let root = Url::parse(ROOT).unwrap();
    for raw in [
        "https://other.example.com/{TileMatrix}/{TileRow}/{TileCol}.png",
        "https://{Style}.example.com/{TileMatrix}/{TileRow}/{TileCol}.png",
        "../{TileMatrix}/{TileRow}/{TileCol}.png?token=secret",
        "../{TileMatrix}/{TileRow}/{TileCol}/{Elevation}.png",
        "../{TileMatrix}/{TileRow}/{TileRow}.png",
        "../{TileMatrix}/{TileRow}/{TileCol}.png#fragment",
        "../%7BTileMatrix%7D/{TileRow}/{TileCol}.png",
        "https://user:pass@maps.example.com/{TileMatrix}/{TileRow}/{TileCol}.png",
        "../{TileMatrix}/{TileRow}/{TileCol}/}.png",
    ] {
        assert!(rest::template(&root, raw, None).is_err(), "{raw}");
    }
    assert!(rest::template(&root, TEMPLATE, Some("Time")).is_err());
    assert!(rest::template(
        &root,
        &TEMPLATE.replace("../tiles", "../tiles/{Time}"),
        Some("Time")
    )
    .is_ok());
}
#[test]
fn unsupported_rest_can_use_advertised_kvp_but_never_a_default_date() {
    let kvp = r#"<ows:OperationsMetadata><ows:Operation name="GetTile"><ows:DCP><ows:HTTP><ows:Get xlink:href="https://maps.example.com/wmts/wmts.cgi"><ows:Constraint name="GetEncoding"><ows:AllowedValues><ows:Value>KVP</ows:Value></ows:AllowedValues></ows:Constraint></ows:Get></ows:HTTP></ows:DCP></ows:Operation></ows:OperationsMetadata>"#;
    let xml = document("https://other.example.com/{TileMatrix}/{TileRow}/{TileCol}.png")
        .replace("<Contents>", &format!("{kvp}<Contents>"));
    let s = parse_capabilities(xml.as_bytes(), &Url::parse(ROOT).unwrap(), "Fallback").unwrap();
    assert!(!s.wmts.as_ref().unwrap().rest_only);
    assert!(s.layers[0].wmts.as_ref().unwrap().resource_url.is_none());
    assert!(s.wmts.as_ref().unwrap().excluded_layers[0]
        .reason
        .contains("KVP"));
    let time = r#"<Dimension><ows:Identifier>Time</ows:Identifier><Default>2025-06-27</Default><Value>2025-06-27</Value></Dimension>"#;
    let xml =
        document(TEMPLATE).replace("<TileMatrixSetLink>", &format!("{time}<TileMatrixSetLink>"));
    assert!(
        parse_capabilities(xml.as_bytes(), &Url::parse(ROOT).unwrap(), "Missing date").is_err()
    );
}
#[test]
fn rest_binding_cannot_offer_styles_that_the_template_cannot_select() {
    let extra = "<Style><ows:Identifier>other</ows:Identifier></Style>";
    let xml = document(&TEMPLATE.replace("{Style}", "default"))
        .replace("<Format>", &format!("{extra}<Format>"));
    assert!(parse_capabilities(
        xml.as_bytes(),
        &Url::parse(ROOT).unwrap(),
        "Ambiguous style"
    )
    .is_err());
}
fn synthetic() -> (MapImage, Vec<Vec<u8>>) {
    let service = service_fixture();
    let m = service.wmts.as_ref().unwrap().matrix_sets[0].matrices[0].clone();
    let q = [-179., 86., -176., 89.];
    let p = plan(q, "EPSG:4326", &m, &[]).unwrap();
    let snapshot = Snapshot {
        resource_url: service.layers[0]
            .wmts
            .as_ref()
            .unwrap()
            .resource_url
            .clone(),
        matrix_set: "regional".into(),
        declared_crs: "CRS:84".into(),
        matrix: m,
        format: "image/png".into(),
        time_identifier: None,
        requested_bounds: q,
        pixel_window: p.window,
        tiles: Vec::new(),
        archive_sha256: "0".repeat(64),
        archive_bytes: 1,
    };
    let mut a = MapImage {
        id: Uuid::new_v4().to_string(),
        name: "Synthetic REST image".into(),
        width: p.window[2] as u32,
        height: p.window[3] as u32,
        bounds: p.bounds,
        image_extent: Some(p.extent),
        crs: "EPSG:4326".into(),
        sha256: "0".repeat(64),
        bytes: 1,
        source: MapSource {
            service_url: service.url,
            service_name: service.name,
            service_title: service.title,
            version: service.version,
            map_endpoint: service.map_url,
            capabilities_sha256: service.capabilities_sha256,
            layer_name: "land".into(),
            layer_title: "land".into(),
            style: "default".into(),
            time: None,
            request_crs: "EPSG:4326".into(),
            request_url: String::new(),
            requested_at: now(),
            access_constraints: String::new(),
            attribution: None,
            area_geometry: None,
            selection: "pixel-window-rendered-tiles".into(),
            wmts: Some(snapshot),
            arcgis: None,
            xyz: None,
        },
    };
    let mut tiles = Vec::new();
    for (row, col) in p.tiles {
        let mut b = Vec::new();
        let mut enc = png::Encoder::new(&mut b, 2, 2);
        enc.set_color(png::ColorType::Rgba);
        enc.set_depth(png::BitDepth::Eight);
        let mut wr = enc.write_header().unwrap();
        wr.write_image_data(&[23, 55, 88, 255].repeat(4)).unwrap();
        wr.finish().unwrap();
        let url = tile_url(
            &a.source.map_endpoint,
            &a.source,
            a.source.wmts.as_ref().unwrap(),
            row,
            col,
        )
        .unwrap();
        a.source.wmts.as_mut().unwrap().tiles.push(TileReceipt {
            row,
            col,
            request_url: url.to_string(),
            bytes: b.len(),
            sha256: hash(&b),
        });
        tiles.push(b);
    }
    a.source.request_url = a.source.wmts.as_ref().unwrap().tiles[0].request_url.clone();
    let (png, archive) = assemble(&a, &tiles).unwrap();
    a.bytes = png.len();
    a.sha256 = hash(&png);
    let s = a.source.wmts.as_mut().unwrap();
    s.archive_bytes = archive.len();
    s.archive_sha256 = hash(&archive);
    validate_asset(&a).unwrap();
    (a, tiles)
}
#[test]
fn path_identifiers_encode_reserved_characters_without_becoming_xyz_levels() {
    let (mut a, _) = synthetic();
    a.source.style = "space style".into();
    let s = a.source.wmts.as_mut().unwrap();
    s.matrix.id = "level:a/b".into();
    let url = tile_url(
        &a.source.map_endpoint,
        &a.source,
        a.source.wmts.as_ref().unwrap(),
        1,
        2,
    )
    .unwrap();
    assert!(url
        .as_str()
        .ends_with("space%20style/regional/level%3Aa%2Fb/1/2.png"));
    a.source.style = "..".into();
    assert!(tile_url(
        &a.source.map_endpoint,
        &a.source,
        a.source.wmts.as_ref().unwrap(),
        1,
        2
    )
    .is_err());
}
#[test]
fn saved_receipts_bind_resource_template_style_matrix_and_date() {
    let (a, _) = synthetic();
    for kind in 0..4 {
        let mut changed = a.clone();
        match kind {
            0 => changed.source.style = "other".into(),
            1 => changed.source.wmts.as_mut().unwrap().resource_url = Some(TEMPLATE.into()),
            2 => changed.source.wmts.as_mut().unwrap().resource_url = None,
            _ => changed.source.wmts.as_mut().unwrap().tiles[0]
                .request_url
                .push_str("?tampered=1"),
        }
        assert!(validate_asset(&changed).is_err());
    }
}
#[tokio::test]
async fn rest_images_and_exact_original_archives_reopen_offline() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let (a, tiles) = synthetic();
    let (png, archive) = assemble(&a, &tiles).unwrap();
    let saved = manager.save_map_image(a, png, Some(archive)).await.unwrap();
    let before = manager.export_map_image(&saved.id).await.unwrap();
    drop(manager);
    let manager = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(
        manager.inspect_map_image(&saved.id).await.unwrap().asset,
        saved
    );
    assert_eq!(manager.export_map_image(&saved.id).await.unwrap(), before);
}
