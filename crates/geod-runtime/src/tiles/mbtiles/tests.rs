use super::*;
fn fixture(views: bool, image: bool) -> Vec<u8> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch(if views {
        "CREATE TABLE meta(name TEXT, value TEXT); CREATE VIEW metadata AS SELECT name,value FROM meta; CREATE TABLE map(zoom_level INTEGER,tile_column INTEGER,tile_row INTEGER,tile_id TEXT); CREATE TABLE images(tile_id TEXT,tile_data BLOB); CREATE VIEW tiles AS SELECT zoom_level,tile_column,tile_row,tile_data FROM map JOIN images USING(tile_id);"
    } else {"CREATE TABLE metadata(name TEXT,value TEXT); CREATE TABLE tiles(zoom_level INTEGER,tile_column INTEGER,tile_row INTEGER,tile_data BLOB);"}).unwrap();
    let table = if views { "meta" } else { "metadata" };
    for (name, value) in [
        ("name", "Synthetic MBTiles fixture"),
        ("format", if image { "png" } else { "pbf" }),
        ("minzoom", "2"),
        ("maxzoom", "2"),
        ("json", r#"{"vector_layers":[{"id":"points","fields":{}}]}"#),
    ] {
        db.execute(
            &format!("INSERT INTO {table} VALUES (?1,?2)"),
            rusqlite::params![name, value],
        )
        .unwrap();
    }
    let raw = if image {
        let mut b = Vec::new();
        {
            let mut enc = png::Encoder::new(&mut b, 2, 2);
            enc.set_color(png::ColorType::Rgba);
            enc.set_depth(png::BitDepth::Eight);
            let mut w = enc.write_header().unwrap();
            w.write_image_data(&[255, 0, 0, 255].repeat(4)).unwrap();
        }
        b
    } else {
        let (s, _, b) = super::super::tests::fixture();
        b[s.header.unwrap().tile_offset as usize..].to_vec()
    };
    if views {
        db.execute("INSERT INTO images VALUES ('a',?1)", [&raw])
            .unwrap();
        db.execute_batch("INSERT INTO map VALUES (2,1,3,'a'),(2,1,0,'a');")
            .unwrap();
    } else {
        for row in [0, 3] {
            db.execute(
                "INSERT INTO tiles VALUES (2,1,?1,?2)",
                rusqlite::params![row, raw],
            )
            .unwrap();
        }
    }
    db.serialize("main").unwrap().to_vec()
}
fn mutate(bytes: &[u8], sql: &str) -> Vec<u8> {
    let mut db = Connection::open_in_memory().unwrap();
    db.deserialize_read_exact("main", Cursor::new(bytes), bytes.len(), false)
        .unwrap();
    db.execute_batch(sql).unwrap();
    db.serialize("main").unwrap().to_vec()
}
#[test]
fn tables_and_normalized_views_have_identical_tms_membership_and_payloads() {
    for views in [false, true] {
        let b = fixture(views, false);
        let p = inspect("中文.MBTILES", &b).unwrap();
        assert_eq!(p.source.mbtiles.as_ref().unwrap().scheme, "tms");
        assert_eq!(
            p.tiles
                .iter()
                .map(|t| t.coordinate.y)
                .collect::<BTreeSet<_>>(),
            BTreeSet::from([0, 3])
        );
        assert!(p
            .tiles
            .iter()
            .all(|t| t.source_offset.is_none() && t.package_offset.is_none()));
        for t in &p.tiles {
            let data = read(&p, &b, t).unwrap();
            assert_eq!(data.layers[0].features, 1);
            assert_eq!(data.content_type, None);
        }
        verify(&p, &b).unwrap();
    }
}
#[test]
fn metadata_columns_can_be_reordered_in_a_table_or_view() {
    let b = fixture(false, false);
    let reference = inspect("columns.mbtiles", &b).unwrap();
    for sql in [
        "ALTER TABLE metadata RENAME TO meta; CREATE VIEW metadata AS SELECT value,name FROM meta;",
        "ALTER TABLE metadata RENAME TO meta; CREATE TABLE metadata(value TEXT,name TEXT); INSERT INTO metadata SELECT value,name FROM meta; DROP TABLE meta;",
    ] {
        let reordered = mutate(&b, sql);
        let actual = inspect("columns.mbtiles", &reordered).unwrap();
        assert_eq!(actual.source.metadata, reference.source.metadata);
        assert_eq!(actual.tiles, reference.tiles);
    }
    assert!(inspect(
        "columns.mbtiles",
        &mutate(&b, "ALTER TABLE metadata ADD COLUMN extra TEXT;")
    )
    .is_err());
}
#[test]
fn valid_empty_mvt_tiles_are_not_misclassified_as_corrupt_images() {
    let b = fixture(false, false);
    let mut db = Connection::open_in_memory().unwrap();
    db.deserialize_read_exact("main", Cursor::new(&b), b.len(), false)
        .unwrap();
    db.execute(
        "UPDATE tiles SET tile_data=?1",
        [format::gzip(&[]).unwrap()],
    )
    .unwrap();
    let b = db.serialize("main").unwrap().to_vec();
    let p = inspect("empty-vectors.mbtiles", &b).unwrap();
    assert!(p.tiles.iter().all(|t| t.layers.is_empty()));
    assert!(read(&p, &b, &p.tiles[0])
        .unwrap()
        .data_base64
        .unwrap()
        .is_empty());
}
#[test]
fn png_payloads_are_decoded_and_keep_exact_compressed_bytes() {
    let b = fixture(false, true);
    let p = inspect("images.mbtiles", &b).unwrap();
    assert_eq!(p.source.mbtiles.as_ref().unwrap().tile_size, Some(2));
    for t in &p.tiles {
        let r = read(&p, &b, t).unwrap();
        let raw = STANDARD.decode(r.data_base64.unwrap()).unwrap();
        assert_eq!(hash(&raw), t.sha256);
        assert_eq!(r.content_type.as_deref(), Some("image/png"));
        assert!(r.layers.is_empty());
    }
}
#[test]
fn corrupt_images_wrong_formats_and_mixed_dimensions_are_rejected() {
    let b = fixture(false, true);
    for sql in [
        "UPDATE metadata SET value='jpg' WHERE name='format';",
        "UPDATE tiles SET tile_data=x'89504E470D0A1A0A';",
        "UPDATE tiles SET tile_data=substr(tile_data,1,length(tile_data)-2) WHERE tile_row=0;",
    ] {
        assert!(inspect("corrupt-images.mbtiles", &mutate(&b, sql)).is_err());
    }
    let mut raw = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut raw, 4, 4);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        encoder
            .write_header()
            .unwrap()
            .write_image_data(&[255, 0, 0, 255].repeat(16))
            .unwrap();
    }
    let mut db = Connection::open_in_memory().unwrap();
    db.deserialize_read_exact("main", Cursor::new(&b), b.len(), false)
        .unwrap();
    db.execute("UPDATE tiles SET tile_data=?1 WHERE tile_row=0", [&raw])
        .unwrap();
    assert!(
        inspect("mixed-images.mbtiles", &db.serialize("main").unwrap())
            .unwrap_err()
            .contains("inconsistent tile dimensions")
    );
}
#[test]
fn tile_count_limit_rejects_a_valid_513_tile_database() {
    let b = fixture(false, true);
    let many = mutate(&b, "CREATE TABLE payload AS SELECT tile_data FROM tiles LIMIT 1; DELETE FROM tiles; WITH RECURSIVE n(x) AS (SELECT 0 UNION ALL SELECT x+1 FROM n WHERE x<512) INSERT INTO tiles SELECT 10,x,0,(SELECT tile_data FROM payload) FROM n; UPDATE metadata SET value='10' WHERE name IN ('minzoom','maxzoom');");
    assert!(inspect("too-many-images.mbtiles", &many)
        .unwrap_err()
        .contains("exceeds 512 tiles"));
}
#[test]
fn invalid_schema_metadata_coordinates_and_gzip_cannot_be_registered() {
    let b = fixture(false, false);
    for sql in ["INSERT INTO metadata VALUES ('name','duplicate');","UPDATE metadata SET value='xyz' WHERE name='scheme'; INSERT INTO metadata VALUES ('scheme','xyz');","UPDATE metadata SET value='webp' WHERE name='format';","UPDATE metadata SET value='1' WHERE name='minzoom';","UPDATE tiles SET tile_row=4;","UPDATE tiles SET zoom_level=25;","UPDATE tiles SET tile_data='not a blob';","INSERT INTO tiles SELECT * FROM tiles;","DELETE FROM metadata WHERE name='json';","UPDATE metadata SET value='bad' WHERE name='json';","UPDATE tiles SET tile_data=x'1A0000';","DELETE FROM tiles;"] {assert!(inspect("bad.mbtiles",&mutate(&b,sql)).is_err(),"{sql}");}
    assert!(inspect("bad.mbtiles", b"SQLite format 3\0").is_err());
    let mut wal = b.clone();
    wal[18] = 2;
    wal[19] = 2;
    assert!(inspect("wal.mbtiles", &wal)
        .unwrap_err()
        .contains("Checkpoint"));
    let too_many=mutate(&b,"DELETE FROM tiles; WITH RECURSIVE n(x) AS (SELECT 0 UNION ALL SELECT x+1 FROM n WHERE x<512) INSERT INTO tiles SELECT 10,x,0,(SELECT tile_data FROM (SELECT x'1F8B' AS tile_data)) FROM n;");
    assert!(inspect("too-many.mbtiles", &too_many).is_err());
}
#[test]
fn virtual_tables_cannot_bypass_schema_checks_using_sql_comments() {
    let b = fixture(false, false);
    for sql in [
        "CREATE VIRTUAL TABLE extra USING fts5(content);",
        "CREATE /* comment */ VIRTUAL -- another comment\n TABLE extra USING fts5(content);",
    ] {
        let rejected = inspect("virtual.mbtiles", &mutate(&b, sql)).unwrap_err();
        assert!(rejected.contains("ordinary SQLite tables and views"));
    }
}
#[test]
fn views_cannot_call_functions_attach_files_or_run_unbounded_queries() {
    let b = fixture(false, false);
    for sql in ["ALTER TABLE metadata RENAME TO meta; CREATE VIEW metadata AS SELECT name,load_extension(value) AS value FROM meta;","ALTER TABLE metadata RENAME TO meta; CREATE VIEW metadata AS SELECT name,hex(zeroblob(100000000)) AS value FROM meta;","ALTER TABLE tiles RENAME TO original; CREATE VIEW tiles AS WITH RECURSIVE n(x) AS (SELECT 0 UNION ALL SELECT x+1 FROM n) SELECT 2 AS zoom_level, x AS tile_column,0 AS tile_row,x'1F8B' AS tile_data FROM n;","ALTER TABLE metadata RENAME TO meta; CREATE VIEW metadata AS SELECT name,value FROM meta,pragma_database_list;"] {assert!(inspect("hostile.mbtiles",&mutate(&b,sql)).is_err(),"{sql}");}
}
#[tokio::test]
async fn managed_original_survives_input_removal_restart_dedup_and_export() {
    let root = tempfile::tempdir().unwrap();
    let input = tempfile::tempdir().unwrap();
    let path = input.path().join("测试.mbtiles");
    let b = fixture(true, false);
    std::fs::write(&path, &b).unwrap();
    let m = JobManager::open(root.path()).await.unwrap();
    let p = m.open_tile_path(path.clone()).await.unwrap();
    std::fs::remove_file(path).unwrap();
    assert_eq!(
        m.import_tile_bytes("测试.mbtiles".into(), b.clone())
            .await
            .unwrap(),
        p
    );
    let zip = m.tile_export(&p.id).await.unwrap();
    let mut z = zip::ZipArchive::new(Cursor::new(&zip)).unwrap();
    let mut original = Vec::new();
    z.by_name("tiles.mbtiles")
        .unwrap()
        .read_to_end(&mut original)
        .unwrap();
    assert_eq!(original, b);
    assert!(z.by_name("tiles.pmtiles").is_err());
    drop(z);
    drop(m);
    let m = JobManager::open(root.path()).await.unwrap();
    assert_eq!(m.list_tile_packages().await, vec![p.clone()]);
    m.inspect_tile_package(&p.id).await.unwrap();
    assert_eq!(zip, m.tile_export(&p.id).await.unwrap());
    let r = m
        .read_tile(TileRequest {
            id: p.id.clone(),
            z: 2,
            x: 1,
            y: 0,
        })
        .await
        .unwrap();
    assert!(r.data_base64.is_some());
    assert!(m
        .read_tile(TileRequest {
            id: p.id.clone(),
            z: 2,
            x: 2,
            y: 0
        })
        .await
        .unwrap()
        .data_base64
        .is_none());
    let managed = file(&m.inner.root, &p).unwrap();
    let mut corrupt = b;
    let end = corrupt.len() - 1;
    corrupt[end] ^= 1;
    std::fs::write(managed, corrupt).unwrap();
    assert!(m
        .read_tile(TileRequest {
            id: p.id,
            z: 2,
            x: 1,
            y: 0
        })
        .await
        .is_err());
}
