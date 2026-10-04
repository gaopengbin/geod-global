use super::*;
fn point() -> Vec<u8> {
    let mut layer = vec![10, 6];
    layer.extend(b"points");
    layer.extend([18, 9, 8, 1, 24, 1, 34, 3, 9, 20, 40, 40, 128, 32, 120, 2]);
    let mut tile = vec![26, layer.len() as u8];
    tile.extend(layer);
    tile
}
pub(super) fn fixture() -> (Source, Package, Vec<u8>) {
    let raw = format::gzip(&point()).unwrap();
    let entry = format::Entry {
        id: 0,
        run: 1,
        length: raw.len() as u64,
        offset: 0,
    };
    let root = format::serialize(&[entry]).unwrap();
    let meta = serde_json::json!({"name":"Fixture","vector_layers":[{"id":"points","fields":{}}]});
    let metadata = format::gzip(&serde_json::to_vec(&meta).unwrap()).unwrap();
    let h = format::Header {
        root_offset: 127,
        root_length: root.len() as u64,
        metadata_offset: 127 + root.len() as u64,
        metadata_length: metadata.len() as u64,
        leaf_offset: 127 + root.len() as u64 + metadata.len() as u64,
        leaf_length: 0,
        tile_offset: 127 + root.len() as u64 + metadata.len() as u64,
        tile_length: raw.len() as u64,
        addressed_tiles: 1,
        tile_entries: 1,
        tile_contents: 1,
        clustered: true,
        internal_compression: 2,
        tile_compression: 2,
        min_zoom: 0,
        max_zoom: 0,
        bounds: [-180., -85.0511287, 180., 85.0511287],
        center_zoom: 0,
        center: [0., 0.],
    };
    let header = h.encode().unwrap();
    let mut bytes = header.clone();
    bytes.extend(&root);
    bytes.extend(&metadata);
    bytes.extend(&raw);
    let ranges = vec![
        http::Receipt {
            offset: 0,
            bytes: 127,
            sha256: hash(&header),
        },
        http::Receipt {
            offset: h.root_offset,
            bytes: root.len(),
            sha256: hash(&root),
        },
        http::Receipt {
            offset: h.metadata_offset,
            bytes: metadata.len(),
            sha256: hash(&metadata),
        },
    ];
    let mut s = Source {
        id: Uuid::new_v4().to_string(),
        name: "Synthetic point fixture".into(),
        url: "https://tiles.example.com/sample.pmtiles".into(),
        etag: "\"fixture\"".into(),
        total_bytes: bytes.len() as u64,
        header: Some(h.clone()),
        mbtiles: None,
        metadata: meta,
        connected_at: now(),
        ranges: ranges.clone(),
        discovery_sha256: String::new(),
        local: None,
    };
    s.discovery_sha256 = fingerprint(&s).unwrap();
    let mut ranges = ranges;
    ranges.push(http::Receipt {
        offset: h.tile_offset,
        bytes: raw.len(),
        sha256: hash(&raw),
    });
    let p = Package {
        id: Uuid::new_v4().to_string(),
        name: s.name.clone(),
        requested_bounds: [10., 10., 11., 11.],
        tile_coverage_bounds: h.bounds,
        min_zoom: 0,
        max_zoom: 0,
        bytes: bytes.len(),
        sha256: hash(&bytes),
        source: s,
        created_at: now(),
        ranges,
        tiles: vec![TileReceipt {
            coordinate: format::Coordinate { z: 0, x: 0, y: 0 },
            source_offset: Some(h.tile_offset),
            bytes: raw.len(),
            sha256: hash(&raw),
            package_offset: Some(h.tile_offset),
            image: None,
            layers: mvt::inspect(&point()).unwrap(),
        }],
        absent: Vec::new(),
        selection: "tile-aligned-pyramid".into(),
    };
    (p.source.clone(), p, bytes)
}
#[test]
fn hilbert_coordinates_match_standard_examples_and_stay_distinct() {
    assert_eq!(format::tile_id(0, 0, 0).unwrap(), 0);
    assert_eq!(format::tile_id(1, 0, 1).unwrap(), 2);
    assert_eq!(format::tile_id(1, 1, 0).unwrap(), 4);
    assert_eq!(format::tile_id(12, 3423, 1763).unwrap(), 19078479);
    for z in 0..=6 {
        let n = 1 << z;
        let mut ids = BTreeSet::new();
        for x in 0..n {
            for y in 0..n {
                ids.insert(format::tile_id(z, x, y).unwrap());
            }
        }
        assert_eq!(ids.len(), (n * n) as usize);
    }
    assert!(format::tile_id(2, 4, 0).is_err());
}
#[test]
fn headers_directories_and_bounded_gzip_reject_invalid_layouts() {
    let (s, _, b) = fixture();
    assert_eq!(
        format::Header::parse(&b[..127], b.len() as u64).unwrap(),
        s.header.clone().unwrap()
    );
    let h = s.header.as_ref().unwrap();
    let d = format::directory(
        &b[h.root_offset as usize..(h.root_offset + h.root_length) as usize],
        h,
    )
    .unwrap();
    assert_eq!(d[0].id, 0);
    let mut invalid = b[..127].to_vec();
    invalid[24..32].copy_from_slice(&127u64.to_le_bytes());
    assert!(format::Header::parse(&invalid, b.len() as u64).is_err());
    assert!(format::decompress(&format::gzip(&vec![0; 1000]).unwrap(), 2, 999).is_err());
    assert!(format::directory(&format::gzip(&[1, 0, 1, 1, 0]).unwrap(), h).is_err());
}
#[test]
fn mvt_validates_layer_versions_geometry_and_tables() {
    let layers = mvt::inspect(&point()).unwrap();
    assert_eq!(layers[0].features, 1);
    assert_eq!(layers[0].extent, 4096);
    let mut b = point();
    *b.last_mut().unwrap() = 3;
    assert!(mvt::inspect(&b).is_err());
    assert!(mvt::inspect(&[26, 20, 10, 1]).is_err());
}
#[test]
fn membership_and_source_version_cannot_silently_change() {
    let (s, p, _) = fixture();
    valid_source(&s).unwrap();
    valid_package(&p).unwrap();
    let mut bad = p.clone();
    bad.tiles[0].coordinate.x = 1;
    assert!(valid_package(&bad).is_err());
    let mut bad = p.clone();
    bad.source.etag = "\"changed\"".into();
    assert!(valid_package(&bad).is_err());
    let mut bad = p;
    bad.tiles[0].source_offset = bad.tiles[0].source_offset.map(|n| n + 1);
    assert!(valid_package(&bad).is_err());
    assert!(http::source_url("https://tiles.example.com/a.pmtiles?token=x").is_err());
    assert!(!http::strong_etag("W/\"weak\""));
}
#[tokio::test]
async fn exact_tiles_inspection_export_and_offline_restart_use_managed_bytes() {
    let dir = tempfile::tempdir().unwrap();
    let m = JobManager::open(dir.path()).await.unwrap();
    let (s, p, b) = fixture();
    std::fs::write(
        directory(&m.inner.root)
            .unwrap()
            .join(format!("{}.pmtiles", p.id)),
        &b,
    )
    .unwrap();
    {
        let mut r = m.inner.tiles.lock().await;
        r.sources.insert(s.id.clone(), s);
        r.packages.insert(p.id.clone(), p.clone());
        persist(&m.inner.root, &r).unwrap();
    }
    let id = p.id.clone();
    assert_eq!(m.inspect_tile_package(&id).await.unwrap().asset, p);
    let tile = m
        .read_tile(TileRequest {
            id: id.clone(),
            z: 0,
            x: 0,
            y: 0,
        })
        .await
        .unwrap();
    assert_eq!(STANDARD.decode(tile.data_base64.unwrap()).unwrap(), point());
    let zip = m.tile_export(&id).await.unwrap();
    let mut zip = zip::ZipArchive::new(Cursor::new(zip)).unwrap();
    let mut exported = Vec::new();
    zip.by_name("tiles.pmtiles")
        .unwrap()
        .read_to_end(&mut exported)
        .unwrap();
    assert_eq!(exported, b);
    drop(m);
    let m = JobManager::open(dir.path()).await.unwrap();
    assert_eq!(m.inspect_tile_package(&id).await.unwrap().asset, p);
    let path = file(&m.inner.root, &p).unwrap();
    let mut changed = b;
    *changed.last_mut().unwrap() ^= 1;
    std::fs::write(path, changed).unwrap();
    assert!(m
        .read_tile(TileRequest {
            id,
            z: 0,
            x: 0,
            y: 0
        })
        .await
        .is_err());
}
