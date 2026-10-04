use super::*;
fn config(scheme: &str, size: u32) -> Configuration {
    Configuration {
        scheme: scheme.into(),
        url_template: "https://example.com/tiles/{z}/{x}/{y}.png".into(),
        grid: GridOptions {
            tile_size: size,
            min_zoom: 0,
            max_zoom: 12,
            zoom_offset: 0,
            format: "image/png".into(),
            attribution: "Test only".into(),
            access_constraints: String::new(),
        },
    }
}
#[test]
fn template_and_policy_validation() {
    for raw in [
        "http://example.com/{z}/{x}/{y}.png",
        "https://localhost/{z}/{x}/{y}.png",
        "https://example.com/{z}/{x}/{x}.png",
        "https://{z}.example.com/{x}/{y}.png",
        "https://example.com/{z}/{x}/{y}.png?token=abc",
        "https://example.com/{z}/{x}/{y}/{s}",
        "https://example.com/../{z}/{x}/{y}.png",
        "https://tile.openstreetmap.org/{z}/{x}/{y}.png",
        "https://example.com/%2e/{z}/{x}/{y}",
    ] {
        assert!(template_url(raw).is_err(), "{raw}");
    }
    assert_eq!(
        template_url(&config("XYZ", 256).url_template)
            .unwrap()
            .as_str(),
        "https://example.com/tiles/%7Bz%7D/%7Bx%7D/%7By%7D.png"
    );
}
#[test]
fn tms_rows_and_url_zoom_offset_are_independent() {
    let mut c = config("TMS", 512);
    c.grid.min_zoom = 1;
    c.grid.zoom_offset = -1;
    assert_eq!(
        tile_url(&c, 3, 1, 2).unwrap().as_str(),
        "https://example.com/tiles/2/2/6.png"
    );
    c.scheme = "XYZ".into();
    assert_eq!(
        tile_url(&c, 3, 1, 2).unwrap().as_str(),
        "https://example.com/tiles/2/2/1.png"
    );
    assert_eq!(matrix(&c, 3).unwrap().matrix_width, 8);
    assert!(tile_url(&c, 3, 8, 1).is_err());
    assert!(matrix(&c, 0).is_err());
}
#[test]
fn tile_size_defines_resolution_not_a_hidden_zoom_shift() {
    let a = matrix(&config("XYZ", 256), 3).unwrap();
    let b = matrix(&config("XYZ", 512), 3).unwrap();
    assert_eq!(a.matrix_width, b.matrix_width);
    assert_eq!(a.scale_denominator, 2. * b.scale_denominator);
    let p = wmts::plan([-10., 20., 10., 40.], "EPSG:3857", &a, &[]).unwrap();
    let q = wmts::plan([-10., 20., 10., 40.], "EPSG:3857", &b, &[]).unwrap();
    assert!(q.window[2] >= p.window[2] * 2 - 2);
    assert_eq!(p.tiles, q.tiles);
}
#[test]
fn invalid_grid_and_service_mutations_rejected() {
    let c = config("XYZ", 256);
    let s = service("Test", c.clone()).unwrap();
    assert!(validate_service(&s).is_ok());
    let mut wrong = s.clone();
    wrong.max_width = 512;
    assert!(validate_service(&wrong).is_err());
    let mut wrong = s;
    wrong.xyz.as_mut().unwrap().scheme = "TMS".into();
    assert!(validate_service(&wrong).is_err());
    for (size, min, max, offset) in [
        (128, 0, 12, 0),
        (256, 4, 2, 0),
        (512, 0, 25, 0),
        (256, 0, 12, -1),
        (256, 0, 12, 3),
    ] {
        let mut q = c.clone();
        q.grid.tile_size = size;
        q.grid.min_zoom = min;
        q.grid.max_zoom = max;
        q.grid.zoom_offset = offset;
        assert!(validate_configuration(&q).is_err());
    }
    assert!(wmts::plan(
        [-10., 85., 10., 89.],
        "EPSG:3857",
        &matrix(&c, 3).unwrap(),
        &[]
    )
    .is_err());
    assert!(wmts::plan(
        [-120., -60., 120., 60.],
        "EPSG:3857",
        &matrix(&c, 12).unwrap(),
        &[]
    )
    .is_err());
}
fn synthetic() -> (MapImage, Vec<Vec<u8>>) {
    let c = config("TMS", 256);
    let service = service("Synthetic checker", c.clone()).unwrap();
    let q = [-2., -2., 2., 2.];
    let p = wmts::plan(q, "EPSG:3857", &matrix(&c, 6).unwrap(), &[]).unwrap();
    let mut tiles = Vec::new();
    let mut receipts = Vec::new();
    for (row, col) in &p.tiles {
        let mut bytes = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut bytes, 256, 256);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut wr = enc.write_header().unwrap();
            let mut pix = Vec::new();
            for y in 0..256 {
                for x in 0..256 {
                    pix.extend([*row as u8, *col as u8, (x ^ y) as u8, 255]);
                }
            }
            wr.write_image_data(&pix).unwrap();
            wr.finish().unwrap();
        }
        receipts.push(wmts::TileReceipt {
            row: *row,
            col: *col,
            request_url: tile_url(&c, 6, *row, *col).unwrap().to_string(),
            bytes: bytes.len(),
            sha256: hash(&bytes),
        });
        tiles.push(bytes);
    }
    let a = MapImage {
        id: Uuid::new_v4().to_string(),
        name: "Synthetic checker · TMS".into(),
        width: p.window[2] as u32,
        height: p.window[3] as u32,
        bounds: p.bounds,
        bytes: 1,
        sha256: "0".repeat(64),
        crs: "EPSG:3857".into(),
        image_extent: Some(p.extent),
        source: MapSource {
            service_url: service.url,
            service_name: service.name,
            service_title: service.title,
            version: service.version,
            map_endpoint: service.map_url,
            capabilities_sha256: service.capabilities_sha256,
            layer_name: "tiles".into(),
            layer_title: "Synthetic checker".into(),
            style: String::new(),
            time: None,
            request_crs: "EPSG:3857".into(),
            request_url: receipts[0].request_url.clone(),
            requested_at: now(),
            access_constraints: String::new(),
            attribution: Some("Test only".into()),
            area_geometry: None,
            selection: "pixel-window-rendered-tiles".into(),
            wmts: None,
            arcgis: None,
            xyz: Some(Snapshot {
                configuration: c.clone(),
                logical_zoom: 6,
                matrix_set: GRID.into(),
                matrix: matrix(&c, 6).unwrap(),
                requested_bounds: q,
                pixel_window: p.window,
                tiles: receipts,
                archive_sha256: "0".repeat(64),
                archive_bytes: 1,
            }),
        },
    };
    (a, tiles)
}
#[test]
fn copies_every_pixel_without_tms_vertical_flip_or_resampling() {
    let (mut a, tiles) = synthetic();
    let s = tile_snapshot(a.source.xyz.as_ref().unwrap()).unwrap();
    let (png, archive) = wmts::assemble_grid(&a, &s, &tiles).unwrap();
    a.bytes = png.len();
    a.sha256 = hash(&png);
    let q = a.source.xyz.as_mut().unwrap();
    q.archive_bytes = archive.len();
    q.archive_sha256 = hash(&archive);
    validate_asset(&a).unwrap();
    let mut reader = png::Decoder::new(Cursor::new(&png)).read_info().unwrap();
    let mut pix = vec![0; reader.output_buffer_size().unwrap()];
    reader.next_frame(&mut pix).unwrap();
    let [x, y, w, h] = s.pixel_window;
    for py in 0..h {
        for px in 0..w {
            let gx = x + px;
            let gy = y + py;
            let i = (py * w + px) as usize * 4;
            assert_eq!(
                &pix[i..i + 4],
                &[
                    (gy / 256) as u8,
                    (gx / 256) as u8,
                    ((gx % 256) ^ (gy % 256)) as u8,
                    255
                ]
            );
        }
    }
    let mut bad = s.clone();
    bad.tiles.swap(0, 1);
    assert!(wmts::assemble_grid(&a, &bad, &tiles).is_err());
    assert!(wmts::assemble_grid(&a, &s, &tiles[..tiles.len() - 1]).is_err());
    let mut wrong = a.clone();
    wrong.source.xyz.as_mut().unwrap().tiles[0]
        .request_url
        .push_str("?tampered=1");
    assert!(validate_asset(&wrong).is_err());
    let mut wrong = a;
    wrong.source.xyz.as_mut().unwrap().logical_zoom = 5;
    assert!(validate_asset(&wrong).is_err());
}
#[tokio::test]
async fn offline_restart_archives_and_connection_identity() {
    let dir = std::env::temp_dir().join(format!("geod-xyz-{}", Uuid::new_v4()));
    let manager = JobManager::open(&dir).await.unwrap();
    let c = config("XYZ", 256);
    let s = manager
        .connect_map_service(ConnectRequest {
            wmts_document: false,
            name: "XYZ".into(),
            url: c.url_template.clone(),
            protocol: c.scheme.clone(),
            tile_config: Some(c.grid.clone()),
        })
        .await
        .unwrap();
    let t = manager
        .connect_map_service(ConnectRequest {
            wmts_document: false,
            name: "TMS".into(),
            url: c.url_template.clone(),
            protocol: "TMS".into(),
            tile_config: Some(c.grid.clone()),
        })
        .await
        .unwrap();
    assert_ne!(s.id, t.id);
    let again = manager
        .connect_map_service(ConnectRequest {
            wmts_document: false,
            name: "Renamed".into(),
            url: c.url_template,
            protocol: c.scheme,
            tile_config: Some(c.grid),
        })
        .await
        .unwrap();
    assert_eq!(again.id, s.id);
    let (mut a, tiles) = synthetic();
    let (png, archive) = wmts::assemble_grid(
        &a,
        &tile_snapshot(a.source.xyz.as_ref().unwrap()).unwrap(),
        &tiles,
    )
    .unwrap();
    a.bytes = png.len();
    a.sha256 = hash(&png);
    let s = a.source.xyz.as_mut().unwrap();
    s.archive_bytes = archive.len();
    s.archive_sha256 = hash(&archive);
    let saved = manager.save_map_image(a, png, Some(archive)).await.unwrap();
    let pkg = manager.export_map_image(&saved.id).await.unwrap();
    drop(manager);
    let manager = JobManager::open(&dir).await.unwrap();
    assert_eq!(manager.list_map_services().await.len(), 2);
    assert_eq!(
        manager.inspect_map_image(&saved.id).await.unwrap().asset,
        saved
    );
    assert_eq!(manager.export_map_image(&saved.id).await.unwrap(), pkg);
    drop(manager);
    std::fs::remove_dir_all(&dir).unwrap();
}
