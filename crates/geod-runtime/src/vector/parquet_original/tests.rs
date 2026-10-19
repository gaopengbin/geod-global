use super::*;

#[test]
fn parquet_format_budgets_are_distinct_and_source_requests_are_pinned() {
    assert_eq!(ORIGINAL_LIMIT, 512 * 1024 * 1024);
    assert_eq!(ROW_GROUP_LIMIT, 32 * 1024 * 1024);
    assert_eq!(super::super::MAX_BYTES, 20 * 1024 * 1024);
    assert_eq!(super::super::MAX_FEATURES, 50_000);
    assert_eq!(super::super::MAX_COORDINATES, 500_000);
    assert!(search(Request {
        dataset_id: "overture-maps".into(),
        package_id: None,
        bounds: [-71.5, 40.5, -70.5, 41.5]
    })
    .is_ok());
    assert!(search(Request {
        dataset_id: "overture-maps".into(),
        package_id: Some("another-release-or-part".into()),
        bounds: [-71.5, 40.5, -70.5, 41.5]
    })
    .is_err());
    assert!(search(Request {
        dataset_id: "ms-buildings".into(),
        package_id: None,
        bounds: [1., 2., 3., 4.]
    })
    .is_err());
}

#[test]
fn geoparquet_metadata_never_guesses_unknown_crs_or_alternative_encoding() {
    let good = json!({"version":"1.1.0","primary_column":"geometry","columns":{"geometry":{
        "encoding":"WKB","geometry_types":["Polygon"],"covering":{"bbox":{"xmin":["bbox","xmin"],
        "ymin":["bbox","ymin"],"xmax":["bbox","xmax"],"ymax":["bbox","ymax"]}}}}});
    validate_geo(&good).unwrap();
    for change in [json!(null), json!({"id":{"authority":"EPSG","code":3857}})] {
        let mut invalid = good.clone();
        invalid["columns"]["geometry"]["crs"] = change;
        assert!(validate_geo(&invalid).is_err());
    }
    let mut invalid = good.clone();
    invalid["columns"]["geometry"]["encoding"] = json!("polygon");
    assert!(validate_geo(&invalid).is_err());
}

#[test]
fn source_bbox_selection_is_inclusive_and_preserves_complete_geometry() {
    let p = json!({"bbox":{"xmin":-72.,"xmax":-71.,"ymin":40.,"ymax":41.}});
    assert!(matches(&p, [-71.5, 40.5, -70.5, 41.5]).unwrap());
    assert!(matches(&p, [-71., 41., -70., 42.]).unwrap());
    assert!(!matches(&p, [-70., 42., -69., 43.]).unwrap());
    assert!(matches(
        &json!({"bbox":{"xmin":2.,"xmax":1.,"ymin":1.,"ymax":2.}}),
        [0., 0., 1., 1.]
    )
    .is_err());
}

#[tokio::test]
#[ignore = "requires full pinned official Overture original and independent pyarrow/Shapely reference"]
async fn real_overture_original_filter_all_properties_geometry_and_reopen_match_independent_reference(
) {
    let source = PathBuf::from(std::env::var("GEOD_OFFICIAL_VECTOR_VERIFICATION_DIR").unwrap());
    let reference: Value =
        serde_json::from_slice(&std::fs::read(source.join("overture-reference.json")).unwrap())
            .unwrap();
    let temp = tempfile::tempdir().unwrap();
    let directory = temp.path().join("parquet-originals");
    std::fs::create_dir(&directory).unwrap();
    let original = directory.join(format!("{ORIGINAL_HASH}.parquet"));
    std::fs::copy(source.join("overture-bathymetry.parquet"), &original).unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let asset = manager
        .download_official_parquet(Request {
            dataset_id: "overture-maps".into(),
            package_id: Some(ID.into()),
            bounds: [-71.5, 40.5, -70.5, 41.5],
        })
        .await
        .unwrap();
    assert_eq!(asset.feature_count, 11);
    assert!(asset.bytes < MAX_BYTES);
    assert_eq!(asset.format, "geoparquet-preview");
    let p = asset.geo_parquet.as_ref().unwrap();
    assert_eq!(p.original_rows, 407277);
    assert_eq!(p.original_bytes, 52018294);
    assert_eq!(p.original_sha256, ORIGINAL_HASH);
    assert_eq!(p.geo_metadata, reference["geo"]);
    let before = manager.inspect_vector(&asset.id).await.unwrap();
    let features = before.geojson["features"].as_array().unwrap();
    for (actual, expected) in features
        .iter()
        .zip(reference["selected"].as_array().unwrap())
    {
        assert_eq!(actual["geometry"], expected["geometry"]);
        assert_eq!(actual["properties"], expected["properties"]);
        assert_eq!(actual["geodOriginalRow"], expected["originalRow"]);
        assert_eq!(
            actual["geodOriginalWkbSha256"],
            expected["geometryWkbSha256"]
        );
    }
    let index = features[0]["geodOriginalRow"].as_u64().unwrap() as usize;
    let rows = manager
        .parquet_original_rows(
            &asset.id,
            Rows {
                offset: Some(index),
                limit: Some(1),
            },
        )
        .await
        .unwrap();
    assert_eq!(rows["rows"][0]["properties"], features[0]["properties"]);
    assert_eq!(
        rows["rows"][0]["geometryWkbSha256"],
        features[0]["geodOriginalWkbSha256"]
    );
    let exported = tempfile::tempdir().unwrap();
    let full = exported.path().join("complete-original.parquet");
    manager
        .export_vector_original_path(&asset.id, full.clone())
        .await
        .unwrap();
    assert_eq!(
        digest_file(&full).unwrap(),
        (ORIGINAL_BYTES, ORIGINAL_HASH.into())
    );
    assert!(manager
        .vector_original_bytes(&asset.id)
        .await
        .unwrap_err()
        .contains("not the original"));
    assert!(manager
        .parquet_original_rows(
            &asset.id,
            Rows {
                offset: Some(ORIGINAL_ROWS + 1),
                limit: Some(1)
            }
        )
        .await
        .is_err());
    assert!(manager
        .parquet_original_rows(
            &asset.id,
            Rows {
                offset: Some(0),
                limit: Some(21)
            }
        )
        .await
        .is_err());
    drop(manager);
    let reopened = JobManager::open(temp.path()).await.unwrap();
    let after = reopened.inspect_vector(&asset.id).await.unwrap();
    assert_eq!(before.asset, after.asset);
    assert_eq!(before.geojson, after.geojson);
    let mut bytes = std::fs::read(&original).unwrap();
    bytes[12] ^= 1;
    std::fs::write(&original, bytes).unwrap();
    assert!(reopened
        .inspect_vector(&asset.id)
        .await
        .unwrap_err()
        .contains("original changed"));
}
