use super::*;
fn archive(leaf: bool, unknown: bool) -> Vec<u8> {
    let (s, _, fixture) = super::super::tests::fixture();
    let raw = &fixture[s.header.as_ref().unwrap().tile_offset as usize..];
    let entries = [
        format::Entry {
            id: 5,
            run: 2,
            offset: 0,
            length: raw.len() as u64,
        },
        format::Entry {
            id: 8,
            run: 1,
            offset: 0,
            length: raw.len() as u64,
        },
    ];
    let leaf_bytes = if leaf {
        format::serialize(&entries).unwrap()
    } else {
        Vec::new()
    };
    let root = if leaf {
        format::serialize(&[format::Entry {
            id: 5,
            run: 0,
            offset: 0,
            length: leaf_bytes.len() as u64,
        }])
        .unwrap()
    } else {
        format::serialize(&entries).unwrap()
    };
    let metadata = format::gzip(
        br#"{"vector_layers":[{"id":"points","fields":{}}],"attribution":"Synthetic fixture"}"#,
    )
    .unwrap();
    let h = format::Header {
        root_offset: 127,
        root_length: root.len() as u64,
        metadata_offset: 127 + root.len() as u64,
        metadata_length: metadata.len() as u64,
        leaf_offset: 127 + root.len() as u64 + metadata.len() as u64,
        leaf_length: leaf_bytes.len() as u64,
        tile_offset: 127 + root.len() as u64 + metadata.len() as u64 + leaf_bytes.len() as u64,
        tile_length: raw.len() as u64,
        addressed_tiles: if unknown { 0 } else { 3 },
        tile_entries: if unknown { 0 } else { 2 },
        tile_contents: if unknown { 0 } else { 1 },
        clustered: true,
        internal_compression: 2,
        tile_compression: 2,
        min_zoom: 2,
        max_zoom: 2,
        bounds: [-180., -85., 180., 85.],
        center_zoom: 2,
        center: [0., 0.],
    };
    let mut b = h.encode().unwrap();
    b.extend(root);
    b.extend(metadata);
    b.extend(leaf_bytes);
    b.extend(raw);
    b
}
#[test]
fn inverse_hilbert_handles_zoom_transitions_and_large_known_coordinate() {
    for z in 0..=7 {
        for x in 0..1 << z {
            for y in 0..1 << z {
                assert_eq!(
                    format::coordinate(format::tile_id(z, x, y).unwrap()).unwrap(),
                    format::Coordinate { z, x, y }
                );
            }
        }
    }
    assert_eq!(
        format::coordinate(19078479).unwrap(),
        format::Coordinate {
            z: 12,
            x: 3423,
            y: 1763
        }
    );
    assert_eq!(
        format::coordinate(format::tile_id(24, (1 << 24) - 1, (1 << 24) - 1).unwrap())
            .unwrap()
            .z,
        24
    );
    assert!(format::coordinate(((1u64 << 50) - 1) / 3).is_err());
}
#[test]
fn sparse_rle_leaf_and_unknown_counts_preserve_exact_archive() {
    for leaf in [false, true] {
        for unknown in [false, true] {
            let b = archive(leaf, unknown);
            let p = inspect("离线地图.pmtiles", &b).unwrap();
            assert_eq!(p.tiles.len(), 3);
            assert_eq!(p.sha256, hash(&b));
            assert_eq!(p.selection, "imported-archive");
            assert!(p.tiles.iter().all(|t| t.package_offset == t.source_offset));
            assert_eq!(p.tiles[0].sha256, p.tiles[1].sha256);
            assert_eq!(p.source.local.unwrap().file_name, "离线地图.pmtiles");
        }
    }
}
#[test]
fn malformed_imports_reject_counts_versions_layers_and_escaping_leaf() {
    let b = archive(true, false);
    for (offset, value) in [(72, 4u64), (80, 1u64), (88, 2u64)] {
        let mut bad = b.clone();
        bad[offset..offset + 8].copy_from_slice(&value.to_le_bytes());
        assert!(inspect("bad.pmtiles", &bad).unwrap_err().contains("counts"));
    }
    let mut bad = b.clone();
    bad[99] = 2;
    assert!(inspect("bad.pmtiles", &bad).is_err());
    let mut bad = b.clone();
    bad[100] = 3;
    bad[101] = 3;
    assert!(inspect("bad.pmtiles", &bad).is_err());
    let h = format::Header::parse(&b[..127], b.len() as u64).unwrap();
    let bad_root = format::serialize(&[format::Entry {
        id: 6,
        run: 0,
        offset: 0,
        length: h.leaf_length,
    }])
    .unwrap();
    assert_eq!(bad_root.len(), h.root_length as usize);
    let mut bad = b.clone();
    bad[h.root_offset as usize..(h.root_offset + h.root_length) as usize]
        .copy_from_slice(&bad_root);
    assert!(inspect("bad.pmtiles", &bad).unwrap_err().contains("parent"));
    for name in ["../bad.pmtiles", "C:\\bad.pmtiles", "bad.zip", ".pmtiles\n"] {
        assert!(inspect(name, &b).is_err());
    }
    let mut bad = b;
    *bad.last_mut().unwrap() ^= 1;
    assert!(inspect("bad.pmtiles", &bad).is_err());
}
#[tokio::test]
async fn managed_copy_survives_original_removal_restart_and_export() {
    let root = tempfile::tempdir().unwrap();
    let original = tempfile::tempdir().unwrap();
    let path = original.path().join("本地.PMTILES");
    let bytes = archive(true, false);
    std::fs::write(&path, &bytes).unwrap();
    let m = JobManager::open(root.path()).await.unwrap();
    assert!(m
        .import_tile_bytes("bad.pmtiles".into(), vec![0; 127])
        .await
        .is_err());
    assert!(m.list_tile_packages().await.is_empty());
    let p = m.open_tile_path(path.clone()).await.unwrap();
    assert_eq!(m.open_tile_path(path.clone()).await.unwrap(), p);
    assert_eq!(m.list_tile_packages().await.len(), 1);
    assert!(m.list_tile_sources().await.is_empty());
    std::fs::remove_file(path).unwrap();
    drop(m);
    let m = JobManager::open(root.path()).await.unwrap();
    assert_eq!(m.inspect_tile_package(&p.id).await.unwrap().asset, p);
    let c = &p.tiles[0].coordinate;
    assert!(m
        .read_tile(TileRequest {
            id: p.id.clone(),
            z: c.z,
            x: c.x,
            y: c.y
        })
        .await
        .unwrap()
        .data_base64
        .is_some());
    let exported = m.tile_export(&p.id).await.unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(exported)).unwrap();
    let mut b = Vec::new();
    z.by_name("tiles.pmtiles")
        .unwrap()
        .read_to_end(&mut b)
        .unwrap();
    assert_eq!(b, bytes);
    let record = serde_json::to_string(&p).unwrap();
    assert!(!record.contains(&original.path().to_string_lossy().replace('\\', "\\\\")));
    let mut bad = p.clone();
    bad.tiles[0].package_offset = bad.tiles[0].package_offset.map(|n| n + 1);
    assert!(valid_package(&bad).is_err());
    let mut bad = p;
    bad.source.local.as_mut().unwrap().sha256 = "a".repeat(64);
    assert!(valid_package(&bad).is_err());
}
