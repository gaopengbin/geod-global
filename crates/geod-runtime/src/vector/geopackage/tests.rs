use super::*;

const WGS84: &str = "GEOGCS[\"WGS 84\",DATUM[\"WGS_1984\",SPHEROID[\"WGS 84\",6378137,298.257223563]],PRIMEM[\"Greenwich\",0],UNIT[\"degree\",0.0174532925199433],AUTHORITY[\"EPSG\",\"4326\"]]";

#[test]
fn independent_gdal_database_preserves_source_fields_and_projected_geometries() {
    let bytes = include_bytes!("../../../fixtures/geopackage/independent-ogr.gpkg");
    assert_eq!(
        hash(bytes),
        "a65fdcee8a141bf841fef550bc62f25170482a69aa8fa05a7ffd64668afcaa47"
    );
    let (data, provenance) = decode(bytes).unwrap();
    assert_eq!(provenance.layers.len(), 5);
    assert_eq!(provenance.other_contents[0].table, "source_notes");
    let features = data["features"].as_array().unwrap();
    assert_eq!(features.len(), 8);
    let point = features
        .iter()
        .find(|f| f["id"] == "9007199254740993")
        .unwrap();
    assert_eq!(point["properties"]["large"], "9007199254740993");
    assert_eq!(point["properties"]["name"], "独立 GDAL 样本");
    assert_eq!(point["properties"]["payload"], "AP8=");
    assert_eq!(point["geometry"]["coordinates"][2], 18.25);
    assert_eq!(point["geodMeasures"], 7.5);
    let route = features
        .iter()
        .find(|f| f["geodLayer"] == "routes_m")
        .unwrap();
    assert_eq!(route["geodMeasures"], json!([1.0, 2.0, 3.0]));
    let mercator = features
        .iter()
        .find(|f| f["geodLayer"] == "web_mercator")
        .unwrap();
    let coordinate = mercator["geometry"]["coordinates"].as_array().unwrap();
    assert!((coordinate[0].as_f64().unwrap() - 12.0).abs() < 1e-10);
    assert!((coordinate[1].as_f64().unwrap() - 48.0).abs() < 1e-10);
    let polygon = features.iter().find(|f| f["geodLayer"] == "utm33").unwrap();
    assert_eq!(
        polygon["geometry"]["coordinates"].as_array().unwrap().len(),
        2
    );
}

fn fixture(geometries: &[Option<Vec<u8>>]) -> Vec<u8> {
    let db = Connection::open_in_memory().unwrap();
    db.execute_batch("PRAGMA application_id=1196444487; PRAGMA user_version=10400;
        CREATE TABLE gpkg_spatial_ref_sys (srs_id INTEGER PRIMARY KEY, organization TEXT, organization_coordsys_id INTEGER, definition TEXT);
        CREATE TABLE gpkg_contents (table_name TEXT PRIMARY KEY, data_type TEXT, identifier TEXT, description TEXT, srs_id INTEGER);
        CREATE TABLE gpkg_geometry_columns (table_name TEXT, column_name TEXT, geometry_type_name TEXT, srs_id INTEGER, z INTEGER, m INTEGER);
        CREATE TABLE items (fid INTEGER PRIMARY KEY, geom GEOMETRY, title TEXT, large INTEGER, active BOOLEAN, value DOUBLE, payload BLOB);
        INSERT INTO gpkg_contents VALUES ('items','features','测试图层',NULL,4326);
        INSERT INTO gpkg_geometry_columns VALUES ('items','geom','GEOMETRY',4326,2,2);
        CREATE TRIGGER never_run AFTER UPDATE ON items BEGIN SELECT raise(ABORT,'do not execute'); END;").unwrap();
    db.execute(
        "INSERT INTO gpkg_spatial_ref_sys VALUES (4326,'EPSG',4326,?1)",
        [WGS84],
    )
    .unwrap();
    for (i, geometry) in geometries.iter().enumerate() {
        db.execute(
            "INSERT INTO items VALUES (?1,?2,'中文名称',?3,1,1.25,?4)",
            rusqlite::params![
                i as i64 + 1,
                geometry,
                9_007_199_254_740_993_i64,
                vec![0u8, 255]
            ],
        )
        .unwrap();
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
fn uint(bytes: &mut Vec<u8>, n: u32, little: bool) {
    bytes.extend(if little {
        n.to_le_bytes()
    } else {
        n.to_be_bytes()
    });
}
fn double(bytes: &mut Vec<u8>, n: f64, little: bool) {
    bytes.extend(if little {
        n.to_le_bytes()
    } else {
        n.to_be_bytes()
    });
}
fn point(x: f64, y: f64, little: bool, dim: u32) -> Vec<u8> {
    let mut b = vec![u8::from(little)];
    uint(&mut b, 1 + dim * 1000, little);
    double(&mut b, x, little);
    double(&mut b, y, little);
    if dim == 1 || dim == 3 {
        double(&mut b, 18.25, little);
    }
    if dim == 2 || dim == 3 {
        double(&mut b, 7.5, little);
    }
    b
}
fn points(kind: u32, values: &[[f64; 2]]) -> Vec<u8> {
    let mut b = vec![1];
    uint(&mut b, kind, true);
    if kind == 3 {
        uint(&mut b, 1, true);
    }
    uint(&mut b, values.len() as u32, true);
    for p in values {
        double(&mut b, p[0], true);
        double(&mut b, p[1], true);
    }
    b
}
fn multi(kind: u32, children: &[Vec<u8>], dim: u32) -> Vec<u8> {
    let mut b = vec![1];
    uint(&mut b, kind + dim * 1000, true);
    uint(&mut b, children.len() as u32, true);
    for c in children {
        b.extend(c);
    }
    b
}
fn gp(wkb: Vec<u8>, empty: bool) -> Vec<u8> {
    let mut b = vec![b'G', b'P', 0, 1 | if empty { 16 } else { 0 }];
    b.extend(4326_i32.to_le_bytes());
    b.extend(wkb);
    b
}

#[test]
fn core_geometries_endianness_z_m_fields_and_source_bytes_are_preserved() {
    let p = point(12., 48., false, 0);
    let l = points(2, &[[12., 48.], [13., 49.]]);
    let polygon = points(3, &[[12., 48.], [13., 48.], [13., 49.], [12., 48.]]);
    let geometries = [
        p.clone(),
        l.clone(),
        polygon.clone(),
        multi(4, std::slice::from_ref(&p), 0),
        multi(5, std::slice::from_ref(&l), 0),
        multi(6, &[polygon], 0),
        multi(7, &[p, l], 0),
        point(12., 48., true, 3),
    ]
    .into_iter()
    .map(|w| Some(gp(w, false)))
    .collect::<Vec<_>>();
    let bytes = fixture(&geometries);
    let i = normalize(&bytes, initial("cores.gpkg", "reference").unwrap()).unwrap();
    assert_eq!(i.asset.format, "geopackage");
    assert_eq!(i.asset.feature_count, 8);
    assert_eq!(i.asset.source_sha256, hash(&bytes));
    assert_eq!(i.asset.geometry_counts.len(), 7);
    assert_eq!(
        i.asset.geo_package.as_ref().unwrap().layers[0].feature_count,
        8
    );
    let f = &i.geojson["features"][7];
    assert_eq!(
        f["properties"],
        json!({"fid":8,"title":"中文名称","large":"9007199254740993","active":true,"value":1.25,"payload":"AP8="})
    );
    assert_eq!(f["geodMeasures"], json!(7.5));
    assert_eq!(f["geometry"]["coordinates"], json!([12., 48., 18.25]));
    assert_eq!(
        i.geojson["geodGeoPackage"]["layers"][0]["definition12063"],
        Value::Null
    );
}
#[test]
fn source_axis_order_authority_and_spherical_mercator_are_adapted_without_changing_source_wkt() {
    let b = fixture(&[Some(gp(point(12., 48., true, 0), false))]);
    let axis = WGS84.replace(
        ",AUTHORITY[\"EPSG\",\"4326\"]]",
        ",AXIS[\"Latitude\",NORTH],AXIS[\"Longitude\",EAST],AUTHORITY[\"EPSG\",\"4326\"]]",
    );
    let b = mutate(
        &b,
        &format!(
            "UPDATE gpkg_spatial_ref_sys SET definition='{}'",
            axis.replace('\'', "''")
        ),
    );
    let (g, p) = decode(&b).unwrap();
    assert_eq!(p.layers[0].definition, axis);
    assert!(p.layers[0]
        .coordinate_definition
        .contains("AXIS[\"Longitude\",EAST],AXIS[\"Latitude\",NORTH]"));
    assert_eq!(
        g["features"][0]["geometry"]["coordinates"],
        json!([12., 48.])
    );
    let wkt2="GEOGCRS[\"WGS 84\",DATUM[\"World Geodetic System 1984\",ELLIPSOID[\"WGS 84\",6378137,298.257223563]],CS[ellipsoidal,2],AXIS[\"latitude\",north,ORDER[1],ANGLEUNIT[\"degree\",0.0174532925199433]],AXIS[\"longitude\",east,ORDER[2],ANGLEUNIT[\"degree\",0.0174532925199433]],ID[\"EPSG\",4326]]";
    let adapted = crs::definition(wkt2, "EPSG", 4326).unwrap();
    assert!(adapted.contains("\"longitude\",east,ORDER[1],ANGLEUNIT"));
    assert!(proj_wkt::parse_crs(&adapted).is_ok());
    let datum="GEOGCS[\"GCS_North_American_1983\",DATUM[\"D_North_American_1983\",SPHEROID[\"GRS_1980\",6378137,298.257222101]],PRIMEM[\"Greenwich\",0],UNIT[\"Degree\",0.0174532925199433]]";
    let b = fixture(&[Some(gp(point(-174., 52., true, 0), false))]);
    let b = mutate(
        &b,
        &format!(
            "UPDATE gpkg_spatial_ref_sys SET organization_coordsys_id=4269,definition='{datum}'"
        ),
    );
    let (_, p) = decode(&b).unwrap();
    assert_eq!(p.layers[0].coordinate_operation_id, Some(1188));
    assert_eq!(p.layers[0].coordinate_accuracy_meters, Some(4.));
    assert_eq!(p.layers[0].coordinates_outside_operation_area, 1);
    let incorrect = datum.replace("6378137", "6300000");
    assert!(decode(&mutate(
        &b,
        &format!("UPDATE gpkg_spatial_ref_sys SET definition='{incorrect}'")
    ))
    .is_err());
    let extension="+proj=merc +a=6378137 +b=6378137 +lat_ts=0 +lon_0=0 +x_0=0 +y_0=0 +k=1 +units=m +nadgrids=@null +wktext +no_defs";
    let mercator=format!("PROJCS[\"WGS 84 / Pseudo-Mercator\",{WGS84},PROJECTION[\"Mercator_1SP\"],PARAMETER[\"central_meridian\",0],PARAMETER[\"scale_factor\",1],PARAMETER[\"false_easting\",0],PARAMETER[\"false_northing\",0],UNIT[\"metre\",1],EXTENSION[\"PROJ4\",\"{extension}\"],AUTHORITY[\"EPSG\",\"3857\"]]");
    assert!(proj_wkt::parse_crs(&crs::definition(&mercator, "EPSG", 3857).unwrap()).is_ok());
    assert!(crs::definition(
        &mercator.replace("\"false_easting\",0", "\"false_easting\",1"),
        "EPSG",
        3857
    )
    .is_err());
    assert!(proj_wkt::parse_crs(
        &crs::definition(&mercator.replace("+b=6378137", "+b=6300000"), "EPSG", 3857).unwrap()
    )
    .is_err());
    assert!(crs::definition(&wkt2.replace("north", "west"), "EPSG", 4326).is_err());
}

#[test]
fn null_and_empty_members_survive_conversion_without_drawable_coordinates() {
    let empty = point(f64::NAN, f64::NAN, true, 0);
    let b = fixture(&[
        None,
        Some(gp(empty.clone(), true)),
        Some(gp(multi(4, &[empty, point(1., 2., true, 0)], 0), false)),
    ]);
    let i = normalize(&b, initial("empty.gpkg", "managed").unwrap()).unwrap();
    assert_eq!(i.asset.coordinate_count, 1);
    assert_eq!(i.asset.bounds, Some([1., 2., 1., 2.]));
    assert_eq!(i.geojson["features"][0]["geometry"], Value::Null);
    assert_eq!(
        i.geojson["features"][1]["geometry"]["coordinates"],
        json!([])
    );
    assert_eq!(
        i.geojson["features"][2]["geometry"]["geodOriginalGeometryType"],
        "MultiPoint"
    );
    assert_eq!(
        i.geojson["features"][2]["geometry"]["geometries"][0]["coordinates"],
        json!([])
    );
}

#[test]
fn two_layers_with_duplicate_ids_and_other_contents_keep_separate_identities() {
    let b = fixture(&[Some(gp(point(12., 48., true, 0), false))]);
    let b=mutate(&b,"CREATE TABLE second (fid INTEGER PRIMARY KEY, geom POINT); INSERT INTO second SELECT fid,geom FROM items;
        INSERT INTO gpkg_contents VALUES ('second','features','Second','',4326);
        INSERT INTO gpkg_geometry_columns VALUES ('second','geom','POINT',4326,0,0);
        CREATE TABLE data (id INTEGER); INSERT INTO gpkg_contents VALUES ('data','attributes',NULL,'',NULL);");
    let (v, p) = decode(&b).unwrap();
    assert_eq!(p.layers.len(), 2);
    assert_eq!(p.other_contents[0].table, "data");
    assert_eq!(v["features"][0]["id"], v["features"][1]["id"]);
    assert_ne!(v["features"][0]["geodLayer"], v["features"][1]["geodLayer"]);
}

#[test]
fn invalid_schema_coordinates_dimensions_headers_and_limits_are_rejected() {
    let b = fixture(&[Some(gp(point(12., 48., true, 0), false))]);
    for sql in [
        "UPDATE gpkg_contents SET srs_id=3857",
        "UPDATE gpkg_geometry_columns SET z=1",
        "UPDATE gpkg_geometry_columns SET geometry_type_name='POLYGON'",
        "UPDATE gpkg_spatial_ref_sys SET definition='undefined'",
        "UPDATE gpkg_spatial_ref_sys SET organization_coordsys_id=3857",
        "INSERT INTO gpkg_geometry_columns SELECT * FROM gpkg_geometry_columns",
        "UPDATE gpkg_geometry_columns SET table_name='missing'",
        "UPDATE items SET active=4; DROP TRIGGER never_run",
    ] {
        // Disable the deliberately aborting update trigger only in malformed test copies.
        let bytes = mutate(
            &b,
            &format!(
                "DROP TRIGGER never_run; {}",
                sql.replace("; DROP TRIGGER never_run", "")
            ),
        );
        assert!(decode(&bytes).is_err(), "{sql}");
    }
    for mut raw in [
        gp(point(12., 48., true, 0), true),
        gp(point(f64::INFINITY, 48., true, 0), false),
    ] {
        assert!(decode(&fixture(&[Some(raw.clone())])).is_err());
        raw[4..8].copy_from_slice(&3857_i32.to_le_bytes());
        assert!(decode(&fixture(&[Some(raw)])).is_err());
    }
    let mut huge = vec![1];
    uint(&mut huge, 4, true);
    uint(&mut huge, u32::MAX, true);
    assert!(decode(&fixture(&[Some(gp(huge, false))])).is_err());
    let mut wal = b.clone();
    wal[18] = 2;
    assert!(decode(&wal).unwrap_err().contains("standalone"));
    let mut wrong = b.clone();
    wrong[68] = 0;
    assert!(decode(&wrong).is_err());
    assert!(validate_wkt_bounds(&format!("{}{}", "[".repeat(33), "]".repeat(33))).is_err());
}

#[tokio::test]
async fn original_managed_file_exports_restart_and_tamper_checks_work() {
    let d = tempfile::tempdir().unwrap();
    let bytes = fixture(&[Some(gp(point(12., 48., true, 0), false))]);
    let input = d.path().join("input.gpkg");
    std::fs::write(&input, &bytes).unwrap();
    let root = d.path().join("runtime");
    let manager = JobManager::open(&root).await.unwrap();
    let reference = manager
        .open_vector_path(input.clone(), false)
        .await
        .unwrap();
    let managed = manager.open_vector_path(input.clone(), true).await.unwrap();
    assert_eq!(
        manager.vector_original_bytes(&managed.id).await.unwrap(),
        bytes
    );
    let original = d.path().join("original.gpkg");
    manager
        .export_vector_original_path(&managed.id, original.clone())
        .await
        .unwrap();
    assert_eq!(std::fs::read(original).unwrap(), bytes);
    let converted = d.path().join("converted.geojson");
    manager
        .export_vector_path(&managed.id, converted.clone())
        .await
        .unwrap();
    assert_eq!(
        serde_json::from_slice::<Value>(&std::fs::read(converted).unwrap()).unwrap(),
        manager.inspect_vector(&managed.id).await.unwrap().geojson
    );
    assert!(manager
        .export_vector_original_path(&managed.id, input.clone())
        .await
        .is_err());
    drop(manager);
    std::fs::remove_file(&input).unwrap();
    let manager = JobManager::open(&root).await.unwrap();
    assert_eq!(
        manager.vector_original_bytes(&managed.id).await.unwrap(),
        bytes
    );
    assert!(manager.inspect_vector(&reference.id).await.is_err());
    std::fs::write(
        root.join("vectors").join(format!("{}.gpkg", managed.id)),
        b"changed",
    )
    .unwrap();
    assert!(manager.vector_original_bytes(&managed.id).await.is_err());
}

#[tokio::test]
async fn loopback_import_accepts_bytes_without_path_or_untrusted_mutations() {
    use axum::{
        body::{to_bytes, Body},
        http::{Request, StatusCode},
    };
    use tower::ServiceExt;
    let d = tempfile::tempdir().unwrap();
    let manager = JobManager::open(d.path()).await.unwrap();
    let app = crate::service::router(manager);
    let bytes = fixture(&[Some(gp(point(12., 48., true, 0), false))]);
    let request = |path: &str, client: bool| {
        let mut r = Request::builder()
            .method("POST")
            .uri(path)
            .header("Host", "127.0.0.1:4318");
        if client {
            r = r.header("X-GeoD-Client", "geod-global");
        }
        r.body(Body::from(bytes.clone())).unwrap()
    };
    assert_eq!(
        app.clone()
            .oneshot(request("/vectors/import-file?name=test.gpkg", false))
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        app.clone()
            .oneshot(request("/vectors/import-file?name=C%3A%2Ftest.gpkg", true))
            .await
            .unwrap()
            .status(),
        StatusCode::BAD_REQUEST
    );
    let r = app
        .clone()
        .oneshot(request("/vectors/import-file?name=test.gpkg", true))
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    let asset: VectorAsset =
        serde_json::from_slice(&to_bytes(r.into_body(), MAX_BYTES).await.unwrap()).unwrap();
    let r = app
        .oneshot(
            Request::builder()
                .uri(format!("/vectors/{}/source", asset.id))
                .header("Host", "127.0.0.1:4318")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(r.status(), StatusCode::OK);
    assert_eq!(
        r.headers()["content-type"],
        "application/geopackage+sqlite3"
    );
    assert_eq!(
        to_bytes(r.into_body(), MAX_BYTES).await.unwrap().as_ref(),
        bytes
    );
}
