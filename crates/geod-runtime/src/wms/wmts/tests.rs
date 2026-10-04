use super::*;
#[test]
fn published_world_edge_scale_rounding_keeps_the_native_grid() {
    let m = Matrix {
        id: "1".into(),
        scale_denominator: 111816452.8057436,
        top_left: [-180., 90.],
        tile_width: 512,
        tile_height: 512,
        matrix_width: 3,
        matrix_height: 2,
    };
    let p = plan([179.9, 0., 180., 1.], "EPSG:4326", &m, &[]).unwrap();
    assert_eq!(p.bounds[2], 180.);
    assert!(p.extent[2] > 180.);
    assert!(p.extent[2] - 180. < 1e-9);
    let mut invalid = m;
    invalid.top_left[0] += 1.;
    assert!(plan([179.9, 0., 180., 1.], "EPSG:4326", &invalid, &[]).is_err());
}
#[test]
fn high_resolution_matrix_windows_do_not_overflow_u32_pixel_offsets() {
    let m = Matrix {
        id: "large-offset".into(),
        scale_denominator: (METERS_PER_DEGREE / 0.00028) / 1e8,
        top_left: [-180., 90.],
        tile_width: 1024,
        tile_height: 1024,
        matrix_width: 1 << 24,
        matrix_height: 1 << 24,
    };
    let p = plan([-130., 39.99999, -129.99999, 40.], "EPSG:4326", &m, &[]).unwrap();
    assert!(p.window[0] > u64::from(u32::MAX));
    assert!(p.window[1] > u64::from(u32::MAX));
    assert!(p.window[2] <= 1002 && p.window[3] <= 1002);
    assert!(p.tiles.len() <= 4);
}
const XML: &str = r#"<Capabilities xmlns="http://www.opengis.net/wmts/1.0" xmlns:ows="http://www.opengis.net/ows/1.1" xmlns:xlink="http://www.w3.org/1999/xlink" version="1.0.0"><ows:ServiceIdentification><ows:Title>Fixture tile service</ows:Title></ows:ServiceIdentification><ows:OperationsMetadata><ows:Operation name="GetTile"><ows:DCP><ows:HTTP><ows:Get xlink:href="https://maps.example.com/wmts/wmts.cgi?"><ows:Constraint name="GetEncoding"><ows:AllowedValues><ows:Value>KVP</ows:Value></ows:AllowedValues></ows:Constraint></ows:Get></ows:HTTP></ows:DCP></ows:Operation></ows:OperationsMetadata><Contents><Layer><ows:Identifier>fixture</ows:Identifier><ows:Title>Fixture tiles</ows:Title><Style isDefault="true"><ows:Identifier>default</ows:Identifier></Style><Format>image/png</Format><TileMatrixSetLink><TileMatrixSet>regional-grid</TileMatrixSet><TileMatrixSetLimits><TileMatrixLimits><TileMatrix>opaque-level</TileMatrix><MinTileRow>0</MinTileRow><MaxTileRow>3</MaxTileRow><MinTileCol>0</MinTileCol><MaxTileCol>0</MaxTileCol></TileMatrixLimits><TileMatrixLimits><TileMatrix>opaque-level</TileMatrix><MinTileRow>0</MinTileRow><MaxTileRow>3</MaxTileRow><MinTileCol>2</MinTileCol><MaxTileCol>3</MaxTileCol></TileMatrixLimits></TileMatrixSetLimits></TileMatrixSetLink></Layer><TileMatrixSet><ows:Identifier>regional-grid</ows:Identifier><ows:SupportedCRS>EPSG:4326</ows:SupportedCRS><TileMatrix><ows:Identifier>opaque-level</ows:Identifier><ScaleDenominator>397569609.982</ScaleDenominator><TopLeftCorner>90 -180</TopLeftCorner><TileWidth>2</TileWidth><TileHeight>2</TileHeight><MatrixWidth>4</MatrixWidth><MatrixHeight>4</MatrixHeight></TileMatrix></TileMatrixSet></Contents></Capabilities>"#;
#[test]
fn invalid_layer_coverage_is_reported_without_hiding_valid_layers() {
    let document = roxmltree::Document::parse(XML).unwrap();
    let layer = document
        .descendants()
        .find(|n| n.has_tag_name((WMTS, "Layer")))
        .unwrap();
    let bad = XML[layer.range()]
        .replace(
            "<ows:Identifier>fixture</ows:Identifier>",
            "<ows:Identifier>invalid-coverage</ows:Identifier>",
        )
        .replace("<MaxTileCol>3</MaxTileCol>", "<MaxTileCol>8</MaxTileCol>");
    let xml = XML.replace("<Contents>", &format!("<Contents>{bad}"));
    let s = parse_capabilities(
        xml.as_bytes(),
        &Url::parse("https://maps.example.com/wmts/wmts.cgi").unwrap(),
        "Fixture",
    )
    .unwrap();
    assert_eq!(s.layers.len(), 1);
    assert_eq!(s.layers[0].name, "fixture");
    let excluded = &s.wmts.as_ref().unwrap().excluded_layers;
    assert_eq!(excluded.len(), 1);
    assert_eq!(excluded[0].name, "invalid-coverage");
    assert!(excluded[0].reason.contains("tile limits"));
}
#[test]
fn discovery_keeps_multiple_coverage_rectangles_without_merging_the_gap() {
    let root = Url::parse("https://maps.example.com/wmts/wmts.cgi").unwrap();
    let s = parse_capabilities(XML.as_bytes(), &root, "Fixture").unwrap();
    let m = &s.wmts.as_ref().unwrap().matrix_sets[0].matrices[0];
    assert_eq!(m.top_left, [-180., 90.]);
    let links = &s.layers[0].wmts.as_ref().unwrap().links[0];
    assert_eq!(links.limits.len(), 2);
    assert!(plan([-177., 86., -175., 88.], "EPSG:4326", m, &links.limits).is_err());
    assert!(parse_capabilities(
        XML.replace("maps.example.com", "other.example.com")
            .as_bytes(),
        &root,
        "Fixture"
    )
    .is_err());
    assert!(parse_capabilities(
        XML.replace(
            "<ows:Value>KVP</ows:Value>",
            "<ows:Value>RESTful</ows:Value>"
        )
        .as_bytes(),
        &root,
        "Fixture"
    )
    .is_err());
}
#[test]
fn png_tile_copy_preserves_all_channels_across_partial_tile_boundaries() {
    let service = parse_capabilities(
        XML.as_bytes(),
        &Url::parse("https://maps.example.com/wmts/wmts.cgi").unwrap(),
        "Fixture",
    )
    .unwrap();
    let mut m = service.wmts.as_ref().unwrap().matrix_sets[0].matrices[0].clone();
    m.scale_denominator = METERS_PER_DEGREE / 0.00028;
    let requested = [-179., 86., -176., 89.];
    let p = plan(requested, "EPSG:4326", &m, &[]).unwrap();
    assert_eq!(p.window, [1, 1, 3, 3]);
    let mut source = MapSource {
        xyz: None,
        arcgis: None,
        service_url: service.url.clone(),
        service_name: service.name,
        service_title: service.title,
        version: "1.0.0".into(),
        map_endpoint: service.map_url,
        capabilities_sha256: service.capabilities_sha256,
        layer_name: "fixture".into(),
        layer_title: "Fixture tiles".into(),
        style: "default".into(),
        time: None,
        request_crs: "EPSG:4326".into(),
        request_url: String::new(),
        requested_at: now(),
        access_constraints: String::new(),
        attribution: None,
        area_geometry: None,
        selection: "pixel-window-rendered-tiles".into(),
        wmts: None,
    };
    let mut s = Snapshot {
        resource_url: None,
        matrix_set: "regional-grid".into(),
        declared_crs: "EPSG:4326".into(),
        matrix: m.clone(),
        format: "image/png".into(),
        time_identifier: None,
        requested_bounds: requested,
        pixel_window: p.window,
        tiles: Vec::new(),
        archive_sha256: "0".repeat(64),
        archive_bytes: 1,
    };
    let mut tiles = Vec::new();
    for (row, col) in p.tiles {
        let mut bytes = Vec::new();
        let pixels: Vec<u8> = (0..2)
            .flat_map(|y| {
                (0..2).flat_map(move |x| {
                    [
                        ((col * 2 + x) as u8),
                        ((row * 2 + y) as u8),
                        77,
                        ((row * 2 + y) * 10 + (col * 2 + x)) as u8,
                    ]
                })
            })
            .collect();
        {
            let mut encoder = png::Encoder::new(&mut bytes, 2, 2);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            encoder
                .write_header()
                .unwrap()
                .write_image_data(&pixels)
                .unwrap();
        }
        s.tiles.push(TileReceipt {
            row,
            col,
            request_url: tile_url(&source.map_endpoint, &source, &s, row, col)
                .unwrap()
                .to_string(),
            bytes: bytes.len(),
            sha256: hash(&bytes),
        });
        tiles.push(bytes);
    }
    source.request_url = s.tiles[0].request_url.clone();
    source.wmts = Some(s);
    let mut a = MapImage {
        id: Uuid::new_v4().to_string(),
        name: "Fixture assembled tiles".into(),
        width: 3,
        height: 3,
        bounds: p.bounds,
        bytes: 1,
        sha256: "0".repeat(64),
        crs: "EPSG:4326".into(),
        source,
        image_extent: Some(p.extent),
    };
    let (png, archive) = assemble(&a, &tiles).unwrap();
    let s = a.source.wmts.as_mut().unwrap();
    s.archive_sha256 = hash(&archive);
    s.archive_bytes = archive.len();
    a.sha256 = hash(&png);
    a.bytes = png.len();
    validate_asset(&a).unwrap();
    let mut reader = png::Decoder::new(Cursor::new(png)).read_info().unwrap();
    let mut out = vec![0; 36];
    reader.next_frame(&mut out).unwrap();
    let expected: Vec<u8> = (1..4)
        .flat_map(|y| (1..4).flat_map(move |x| [x, y, 77, y * 10 + x]))
        .collect();
    assert_eq!(out, expected);
    let tmp = tempfile::tempdir().unwrap();
    let file = tmp.path().join("tiles.zip");
    std::fs::write(&file, &archive).unwrap();
    assert_eq!(
        read_archive(&file, a.source.wmts.as_ref().unwrap()).unwrap(),
        archive
    );
    std::fs::write(&file, b"changed").unwrap();
    assert!(read_archive(&file, a.source.wmts.as_ref().unwrap()).is_err());
    a.image_extent.as_mut().unwrap()[0] += 1.;
    assert!(validate_asset(&a).is_err());
}
fn matrix() -> Matrix {
    Matrix {
        id: "opaque-level".into(),
        scale_denominator: METERS_PER_DEGREE / 0.00028,
        top_left: [-180., 90.],
        tile_width: 512,
        tile_height: 256,
        matrix_width: 1,
        matrix_height: 1,
    }
}
#[test]
fn matrix_identifiers_tile_sizes_and_pixel_windows_are_explicit() {
    let m = matrix();
    let p = plan([-125., 30., -110., 43.], "EPSG:4326", &m, &[]).unwrap();
    assert_eq!(p.window, [55, 47, 15, 13]);
    assert_eq!(p.extent, [-125., 30., -110., 43.]);
    assert_eq!(p.tiles, vec![(0, 0)]);
    let mut m = m;
    m.tile_width = 16;
    m.tile_height = 16;
    m.matrix_width = 32;
    m.matrix_height = 16;
    let p = plan([-125., 30., -110., 43.], "EPSG:4326", &m, &[]).unwrap();
    assert_eq!(p.window, [55, 47, 15, 13]);
    assert_eq!(p.tiles, vec![(2, 3), (2, 4), (3, 3), (3, 4)]);
    let limits = Limits {
        matrix: m.id.clone(),
        min_row: 2,
        max_row: 2,
        min_col: 3,
        max_col: 4,
    };
    assert!(plan([-125., 30., -110., 43.], "EPSG:4326", &m, &[limits]).is_err());
}
#[test]
fn axis_order_and_projection_are_not_assumed_to_be_xyz() {
    assert!(latitude_first("urn:ogc:def:crs:EPSG::4326"));
    assert!(!latitude_first("urn:ogc:def:crs:OGC:1.3:CRS84"));
    assert_eq!(crs("urn:ogc:def:crs:EPSG:6.18:3:3857"), Ok("EPSG:3857"));
    assert!(crs("EPSG:3413").is_err());
    let b = [-125., 30., -110., 43.];
    let projected = project(b, "EPSG:3857").unwrap();
    let actual = unproject(projected, "EPSG:3857");
    for (a, e) in actual.iter().zip(b) {
        assert!((a - e).abs() < 1e-12);
    }
    assert!(project([-10., -90., 10., 90.], "EPSG:3857").is_err());
}
#[test]
fn tile_limits_sizes_and_empty_grids_are_rejected() {
    let mut m = matrix();
    m.tile_width = 0;
    assert!(validate_matrix(&m).is_err());
    m.tile_width = 1025;
    assert!(validate_matrix(&m).is_err());
    m = matrix();
    m.scale_denominator = f64::NAN;
    assert!(plan([-1., -1., 1., 1.], "EPSG:4326", &m, &[]).is_err());
    m = matrix();
    m.scale_denominator /= 4096.;
    m.matrix_width = 4096;
    m.matrix_height = 4096;
    assert!(plan([-125., 30., -110., 43.], "EPSG:4326", &m, &[]).is_err());
}
