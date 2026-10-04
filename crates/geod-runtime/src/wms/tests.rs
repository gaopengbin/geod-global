use super::*;
const XML: &str = r#"<WMS_Capabilities xmlns="http://www.opengis.net/wms" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.3.0"><Service><Title>Test map</Title><AccessConstraints>Dataset terms apply</AccessConstraints><MaxWidth>1024</MaxWidth></Service><Capability><Request><GetMap><Format>image/png</Format><DCPType><HTTP><Get><OnlineResource xlink:href="https://maps.example.com/wms/?"/></Get></HTTP></DCPType></GetMap></Request><Layer><CRS>EPSG:4326</CRS><Style><Name>default</Name></Style><Dimension name="time" default="2025-06-27">2025-06-01/2025-06-30/P1D</Dimension><Attribution><Title>Test provider</Title></Attribution><Layer><Name>land</Name><Title>Land image</Title></Layer><Layer><Name>unsupported</Name><Dimension name="elevation">0,100</Dimension></Layer></Layer></Capability></WMS_Capabilities>"#;
fn service() -> MapService {
    parse_capabilities(
        XML.as_bytes(),
        &Url::parse("https://maps.example.com/wms/wms.cgi").unwrap(),
        "Test",
    )
    .unwrap()
}
fn png() -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut e = png::Encoder::new(&mut bytes, 2, 2);
        e.set_color(png::ColorType::Rgba);
        e.set_depth(png::BitDepth::Eight);
        e.write_header()
            .unwrap()
            .write_image_data(&[1, 2, 3, 255, 4, 5, 6, 255, 7, 8, 9, 255, 10, 11, 12, 255])
            .unwrap();
    }
    bytes
}
fn asset() -> MapImage {
    let service = service();
    let bounds = [-125., 30., -110., 43.];
    let layer = &service.layers[0];
    let source = MapSource {
        xyz: None,
        arcgis: None,
        wmts: None,
        service_url: service.url,
        service_name: service.name,
        service_title: service.title,
        version: service.version,
        map_endpoint: service.map_url,
        capabilities_sha256: service.capabilities_sha256,
        layer_name: layer.name.clone(),
        layer_title: layer.title.clone(),
        style: "default".into(),
        time: Some("2025-06-27".into()),
        request_crs: layer.crs.clone(),
        request_url: map_url(
            "https://maps.example.com/wms/",
            "1.3.0",
            "EPSG:4326",
            "land",
            "default",
            Some("2025-06-27"),
            bounds,
            [2, 2],
        )
        .unwrap()
        .into(),
        requested_at: now(),
        access_constraints: service.access_constraints,
        attribution: layer.attribution.clone(),
        area_geometry: None,
        selection: "bbox-rendered-map".into(),
    };
    let b = png();
    MapImage {
        image_extent: None,
        id: Uuid::new_v4().to_string(),
        name: "Test image".into(),
        width: 2,
        height: 2,
        bounds,
        bytes: b.len(),
        sha256: hash(&b),
        crs: "EPSG:4326".into(),
        source,
    }
}
#[test]
fn capabilities_inherit_and_skip_other_dimensions() {
    let s = service();
    assert_eq!(s.layers.len(), 1);
    let l = &s.layers[0];
    assert_eq!(l.styles, vec!["default"]);
    assert_eq!(l.attribution.as_deref(), Some("Test provider"));
    assert_eq!(s.max_width, 1024);
    assert_eq!(s.max_height, 2048);
    assert!(time_supported(l.time.as_ref().unwrap(), "2025-06-27"));
    assert!(!time_supported(l.time.as_ref().unwrap(), "2025-07-01"));
}
#[test]
fn axis_order_matches_both_versions_and_crs84() {
    for (version, crs, key, bbox) in [
        ("1.3.0", "EPSG:4326", "CRS", "30,-125,43,-110"),
        ("1.3.0", "CRS:84", "CRS", "-125,30,-110,43"),
        ("1.1.1", "EPSG:4326", "SRS", "-125,30,-110,43"),
    ] {
        let u = map_url(
            "https://maps.example.com/wms/",
            version,
            crs,
            "x",
            "",
            None,
            [-125., 30., -110., 43.],
            [512, 444],
        )
        .unwrap();
        let q: BTreeMap<_, _> = u.query_pairs().collect();
        assert_eq!(q["BBOX"], bbox);
        assert_eq!(q[key], crs);
        assert!(!q.contains_key(if key == "CRS" { "SRS" } else { "CRS" }));
    }
}
#[test]
fn service_rejects_unscoped_requests_and_entity_documents() {
    let root = Url::parse("https://maps.example.com/wms/wms.cgi").unwrap();
    for url in [
        "https://other.example.com/wms/",
        "https://maps.example.com/admin",
        "https://maps.example.com/wms/?token=secret",
    ] {
        assert!(endpoint(&root, url).is_err());
    }
    assert!(service_url("https://127.0.0.1/wms").is_err());
    assert!(parse_capabilities(
        XML.replace("<Service>", "<!ENTITY x 'expanded'><Service>")
            .as_bytes(),
        &root,
        "Test"
    )
    .is_err());
    assert!(parse_capabilities(
        XML.replace("image/png", "image/jpeg").as_bytes(),
        &root,
        "Test"
    )
    .is_err());
}
#[test]
fn legacy_external_dtd_is_not_fetched() {
    let xml = XML
        .replace("WMS_Capabilities", "WMT_MS_Capabilities")
        .replace(" xmlns=\"http://www.opengis.net/wms\"", "")
        .replace("version=\"1.3.0\"", "version=\"1.1.1\"")
        .replace("<CRS>", "<SRS>")
        .replace("</CRS>", "</SRS>");
    let xml = format!(
        "<!DOCTYPE WMT_MS_Capabilities SYSTEM 'https://not-fetched.example.com/wms.dtd'>{xml}"
    );
    assert_eq!(
        parse_capabilities(
            xml.as_bytes(),
            &Url::parse("https://maps.example.com/wms/wms.cgi").unwrap(),
            "Test"
        )
        .unwrap()
        .version,
        "1.1.1"
    );
}
#[test]
fn time_steps_are_not_silently_snapped() {
    let dim = |s: &str| TimeDimension {
        values: s.into(),
        default: None,
    };
    assert!(time_supported(
        &dim("2024-01-01/2024-12-31/P2D"),
        "2024-01-03"
    ));
    assert!(!time_supported(
        &dim("2024-01-01/2024-12-31/P2D"),
        "2024-01-02"
    ));
    assert!(time_supported(
        &dim("2024-01-01/2024-12-31/P1M"),
        "2024-06-01"
    ));
    assert!(!time_supported(
        &dim("2024-01-01/2024-12-31/P1M"),
        "2024-06-02"
    ));
    assert!(time_supported(
        &dim("2025-06-01T00:00:00Z/2025-06-02T00:00:00Z/PT3H"),
        "2025-06-01T06:00:00Z"
    ));
    assert!(!time_supported(&dim("current"), "current"));
    for period in [
        "P界",
        "PT😀",
        "P+1D",
        "P-1D",
        "P0D",
        "P9223372036854775807D",
    ] {
        assert!(!time_supported(
            &dim(&format!("2025-06-27/2025-07-01/{period}")),
            "2025-06-27"
        ));
    }
}
#[test]
fn image_dimensions_and_corruption_are_rejected() {
    let b = png();
    validate_png(&b, 2, 2).unwrap();
    assert!(validate_png(&b, 3, 2).is_err());
    assert!(validate_png(&b[..b.len() / 2], 2, 2).is_err());
    assert!(validate_png(b"<ServiceException>failed</ServiceException>", 2, 2).is_err());
}
#[test]
fn world_file_uses_pixel_centers_and_exact_source_bytes() {
    let a = asset();
    let b = png();
    let package = package(&a, b.clone()).unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(package)).unwrap();
    let mut actual = Vec::new();
    zip.by_name("map.png")
        .unwrap()
        .read_to_end(&mut actual)
        .unwrap();
    assert_eq!(actual, b);
    let mut world = String::new();
    zip.by_name("map.pgw")
        .unwrap()
        .read_to_string(&mut world)
        .unwrap();
    let v: Vec<f64> = world.lines().map(|s| s.parse().unwrap()).collect();
    assert_eq!(v, vec![7.5, 0., 0., -6.5, -121.25, 39.75]);
    let mut metadata = String::new();
    zip.by_name("source.json")
        .unwrap()
        .read_to_string(&mut metadata)
        .unwrap();
    assert_eq!(serde_json::from_str::<MapImage>(&metadata).unwrap(), a);
}
#[tokio::test]
async fn persistent_snapshots_reopen_offline_and_detect_changes() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path().canonicalize().unwrap();
    let manager = JobManager::open(&root).await.unwrap();
    let a = asset();
    let b = png();
    std::fs::write(root.join("map-images").join(format!("{}.png", a.id)), &b).unwrap();
    persist(&root, IMAGES, &BTreeMap::from([(a.id.clone(), a.clone())])).unwrap();
    drop(manager);
    let manager = JobManager::open(&root).await.unwrap();
    assert_eq!(manager.inspect_map_image(&a.id).await.unwrap().asset, a);
    let outside = tempfile::tempdir().unwrap();
    let destination = outside.path().join("map.zip");
    manager
        .export_map_image_path(&a.id, destination.clone())
        .await
        .unwrap();
    assert!(manager
        .export_map_image_path(&a.id, destination)
        .await
        .is_err());
    assert!(manager
        .export_map_image_path(&a.id, root.join("overwrite.zip"))
        .await
        .is_err());
    std::fs::write(
        root.join("map-images").join(format!("{}.png", a.id)),
        b"changed",
    )
    .unwrap();
    assert!(manager.inspect_map_image(&a.id).await.is_err());
    assert!(manager.export_map_image(&a.id).await.is_err());
}
#[test]
fn metadata_grid_and_polygon_cannot_drift() {
    let mut a = asset();
    validate_asset(&a).unwrap();
    a.width = 3;
    assert!(validate_asset(&a).is_err());
    a = asset();
    a.source.selection = "clipped-polygon".into();
    assert!(validate_asset(&a).is_err());
}
