use super::*;

// Synthetic contract fixtures; live acceptance is an explicit ignored test below.
const ROOT: &str = "https://www.geoboundaries.org/api/current/gbOpen/CHN/ALL/";

fn metadata() -> Value {
    json!({
        "boundaryID":"CHN-ADM1-123456", "boundaryName":"China", "boundaryISO":"CHN",
        "boundaryYearRepresented":"2019", "boundaryType":"ADM1", "boundaryCanonical":"Synthetic",
        "boundarySource":"Synthetic source", "boundaryLicense":"Public Domain",
        "licenseSource":"example.org/license", "sourceDataUpdateDate":"Jan 19, 2023",
        "buildDate":"Dec 12, 2023", "admUnitCount":"2", "optionalNumericText":"0.00",
        "gjDownloadURL":"https://github.com/wmgeolab/geoBoundaries/raw/9469f09/releaseData/gbOpen/CHN/ADM1/geoBoundaries-CHN-ADM1.geojson"
    })
}

fn service() -> FeatureService {
    let layer = parse_layer(&metadata(), "CHN").unwrap();
    FeatureService {
        id: Uuid::new_v4().to_string(),
        name: "Synthetic gbOpen".into(),
        url: ROOT.into(),
        title: "Synthetic".into(),
        connected_at: now(),
        arcgis: None,
        overpass: None,
        wfs: None,
        geoboundaries: Some(Service {
            release_type: RELEASE_TYPE.into(),
            country_iso3: "CHN".into(),
            discovery_receipt: PageReceipt {
                url: ROOT.into(),
                sha256: "a".repeat(64),
                bytes: 1_234,
                returned: 1,
                parameters: None,
            },
        }),
        collections: vec![Collection {
            id: "ADM1".into(),
            title: "China · ADM1".into(),
            description: "Synthetic metadata".into(),
            items_url: layer_api("CHN", "ADM1"),
            license_links: vec![LICENSE.into()],
            arcgis: None,
            wfs: None,
            geoboundaries: Some(layer),
        }],
    }
}

fn data() -> Value {
    let feature = |id: &str, name: &str| {
        json!({"type":"Feature",
        "properties":{"shapeID":id,"shapeName":name,"shapeGroup":"CHN","shapeType":"ADM1","nullable":null,"originalNumber":12.125},
        "geometry":{"type":"Polygon","coordinates":[[[110.0,30.0],[110.1,30.0],[110.1,30.1],[110.0,30.0]]]}})
    };
    json!({"type":"FeatureCollection", "features":[feature("id-one","名称一"),feature("id-two","名称二")]})
}

fn provenance() -> Provenance {
    let s = service();
    let c = s.collections[0].clone();
    let layer = c.geoboundaries.unwrap();
    Provenance {
        service_url: s.url,
        service_name: s.name,
        collection_id: c.id,
        collection_title: c.title,
        license_links: vec![LICENSE.into()],
        requested_bounds: [0., 0., 1., 1.],
        area_geometry: None,
        requested_at: now(),
        number_matched: Some(2),
        feature_count: 2,
        selection: "geoboundaries-full-administrative-level".into(),
        arcgis: None,
        wfs: None,
        pages: vec![PageReceipt {
            url: layer.download_url.clone(),
            sha256: "b".repeat(64),
            bytes: 4_321,
            returned: 2,
            parameters: None,
        }],
        geoboundaries: Some(Snapshot {
            release_type: RELEASE_TYPE.into(),
            resolved_release_commit: layer.release_ref.clone(),
            source_crs: None,
            layer,
            discovery_receipt: s.geoboundaries.unwrap().discovery_receipt,
            attribution: ATTRIBUTION.into(),
            geometry_selection: "geoboundaries-full-administrative-level".into(),
        }),
    }
}

#[test]
fn accepts_only_official_gbopen_country_discovery() {
    assert_eq!(root_url(ROOT).unwrap().1, "CHN");
    assert_eq!(
        root_url(ROOT.trim_end_matches('/')).unwrap().0.as_str(),
        ROOT
    );
    for bad in [
        "https://www.geoboundaries.org/api/current/gbHumanitarian/CHN/ALL/",
        "https://www.geoboundaries.org/api/current/gbAuthoritative/CHN/ALL/",
        "https://www.geoboundaries.org/api/current/gbOpen/ALL/ALL/",
        "https://example.org/api/current/gbOpen/CHN/ALL/",
        "https://www.geoboundaries.org/api/current/gbOpen/chn/ALL/",
        "https://www.geoboundaries.org/api/current/gbOpen/CHN/ADM1/",
        "https://www.geoboundaries.org/api/current/gbOpen/CHN/ALL/?f=json",
    ] {
        assert!(root_url(bad).is_err(), "{bad}");
    }
}

#[test]
fn rejects_mutable_branch_simplified_and_foreign_downloads() {
    let source = metadata()["gjDownloadURL"].as_str().unwrap().to_string();
    assert!(download_url(&source, "CHN", "ADM1").is_ok());
    for bad in [
        source.replace("9469f09", "main"),
        source.replace(".geojson", "_simplified.geojson"),
        source.replace("wmgeolab/geoBoundaries", "someone/geoBoundaries"),
        source.replace("/gbOpen/", "/gbHumanitarian/"),
        source.replace("/CHN/", "/USA/"),
        source.replace("github.com", "evil.example"),
        format!("{source}?download=1"),
    ] {
        assert!(download_url(&bad, "CHN", "ADM1").is_err(), "{bad}");
    }
}

#[test]
fn permits_only_exact_publisher_cdn_filename_and_commit() {
    let full = "9469f09592ced973a3448cf66b6100b741b64c0d";
    for url in [
        format!("https://raw.githubusercontent.com/wmgeolab/geoBoundaries/{full}/releaseData/gbOpen/CHN/ADM1/geoBoundaries-CHN-ADM1.geojson"),
        format!("https://media.githubusercontent.com/media/wmgeolab/geoBoundaries/{full}/releaseData/gbOpen/CHN/ADM1/geoBoundaries-CHN-ADM1.geojson"),
    ] { assert_eq!(download_url(&url,"CHN","ADM1").unwrap().1,full); }
    assert!(download_url("https://media.githubusercontent.com/media/evil/geoBoundaries/9469f09/releaseData/gbOpen/CHN/ADM1/geoBoundaries-CHN-ADM1.geojson","CHN","ADM1").is_err());
}

#[test]
fn retains_all_metadata_original_licenses_and_year_without_guessing() {
    let v = metadata();
    let layer = parse_layer(&v, "CHN").unwrap();
    assert_eq!(layer.metadata, v);
    assert_eq!(layer.year_represented, "2019");
    assert_eq!(layer.original_license, "Public Domain");
    assert_eq!(layer.metadata["optionalNumericText"], "0.00");
    assert_eq!(
        layer.metadata_sha256,
        hash(&serde_json::to_vec(&v).unwrap())
    );
}

#[test]
fn changed_missing_or_unsupported_metadata_does_not_become_a_collection() {
    for (key, value) in [
        ("boundaryISO", json!("USA")),
        ("boundaryType", json!("ADM6")),
        ("admUnitCount", json!("0")),
        ("admUnitCount", json!("50001")),
        ("boundaryLicense", Value::Null),
    ] {
        let mut v = metadata();
        v[key] = value;
        assert!(parse_layer(&v, "CHN").is_err(), "{key}");
    }
}

#[test]
fn corrupt_registry_country_links_hash_or_protocol_are_rejected() {
    assert!(validate_service(&service()).is_ok());
    let mut s = service();
    s.collections[0].items_url = s.collections[0]
        .geoboundaries
        .as_ref()
        .unwrap()
        .download_url
        .clone();
    assert!(validate_service(&s).is_err());
    let mut s = service();
    s.collections[0]
        .geoboundaries
        .as_mut()
        .unwrap()
        .year_represented = "2026".into();
    assert!(validate_service(&s).is_err());
    let mut s = service();
    s.geoboundaries.as_mut().unwrap().discovery_receipt.sha256 = "x".repeat(64);
    assert!(validate_service(&s).is_err());
    let mut s = service();
    s.collections.push(s.collections[0].clone());
    s.geoboundaries.as_mut().unwrap().discovery_receipt.returned = 2;
    assert!(validate_service(&s).is_err());
}

#[test]
fn complete_original_properties_are_kept_even_outside_requested_aoi() {
    let layer = parse_layer(&metadata(), "CHN").unwrap();
    let v = data();
    let original = v.clone();
    assert_eq!(validate_geometry(&v, &layer).unwrap(), 2);
    assert_eq!(v, original);
    assert!(validate_provenance(&provenance(), 2).is_ok());
    assert_eq!(v["features"][0]["properties"]["originalNumber"], 12.125);
    assert_eq!(v["features"][0]["properties"]["nullable"], Value::Null);
}

#[test]
fn incomplete_or_duplicate_or_other_country_file_is_rejected() {
    let layer = parse_layer(&metadata(), "CHN").unwrap();
    let mut v = data();
    v["features"].as_array_mut().unwrap().pop();
    assert!(validate_geometry(&v, &layer).is_err());
    let mut v = data();
    v["features"][1]["properties"]["shapeID"] = json!("id-one");
    assert!(validate_geometry(&v, &layer).is_err());
    let mut v = data();
    v["features"][1]["properties"]["shapeGroup"] = json!("USA");
    assert!(validate_geometry(&v, &layer).is_err());
    let mut v = data();
    v["geodSource"] = json!({"fake":true});
    assert!(validate_geometry(&v, &layer).is_err());
}

#[test]
fn pinned_download_and_full_country_receipt_are_required_on_reopen() {
    let mut p = provenance();
    p.selection = "bbox-full-features".into();
    assert!(validate_provenance(&p, 2).is_err());
    let mut p = provenance();
    p.pages[0].returned = 1;
    assert!(validate_provenance(&p, 2).is_err());
    let mut p = provenance();
    p.pages[0].url = p.pages[0].url.replace("9469f09", "1234567");
    assert!(validate_provenance(&p, 2).is_err());
    let mut p = provenance();
    p.pages[0].parameters = Some(BTreeMap::new());
    assert!(validate_provenance(&p, 2).is_err());
    let mut p = provenance();
    p.geoboundaries.as_mut().unwrap().attribution = "anonymous".into();
    assert!(validate_provenance(&p, 2).is_err());
}

#[tokio::test]
async fn full_country_snapshot_preserves_properties_hashes_and_provenance_after_reopen() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    let mut p = provenance();
    p.geoboundaries.as_mut().unwrap().source_crs =
        Some(json!({"type":"name","properties":{"name":"urn:ogc:def:crs:OGC:1.3:CRS84"}}));
    let mut v = data();
    v["crs"] = p
        .geoboundaries
        .as_ref()
        .unwrap()
        .source_crs
        .clone()
        .unwrap();
    v["geodSource"] = serde_json::to_value(&p).unwrap();
    let asset = manager
        .import_vector_source(
            vector::ImportVectorRequest {
                name: "Synthetic gbOpen file".into(),
                text: serde_json::to_string(&v).unwrap(),
            },
            Some(p.clone()),
        )
        .await
        .unwrap();
    assert_eq!(asset.feature_count, 2);
    assert_eq!(asset.remote_source.as_ref(), Some(&p));
    let inspect = manager.inspect_vector(&asset.id).await.unwrap();
    assert_eq!(inspect.geojson["features"], v["features"]);
    assert!(inspect.geojson.get("crs").is_none());
    let stored: Value =
        serde_json::from_slice(&manager.vector_original_bytes(&asset.id).await.unwrap()).unwrap();
    assert_eq!(stored["crs"], v["crs"]);
    assert_eq!(stored["features"], v["features"]);
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(dir.path()).await.unwrap();
    let read = reopened.inspect_vector(&asset.id).await.unwrap();
    assert_eq!(read.asset.remote_source, Some(p));
    assert_eq!(read.geojson["features"], v["features"]);
    let stored: Value =
        serde_json::from_slice(&reopened.vector_original_bytes(&asset.id).await.unwrap()).unwrap();
    assert_eq!(stored["crs"], v["crs"]);
}

#[test]
fn display_normalization_removes_only_verified_redundant_source_crs() {
    let crs = json!({"type":"name","properties":{"name":"urn:ogc:def:crs:OGC:1.3:CRS84"}});
    let mut v = data();
    v["crs"] = crs.clone();
    let original = v.clone();
    let mut p = provenance();
    p.geoboundaries.as_mut().unwrap().source_crs = Some(crs);
    normalize_for_display(&mut v, &p).unwrap();
    assert!(v.get("crs").is_none());
    assert!(original.get("crs").is_some());
    assert_eq!(v["features"], original["features"]);
    let mut wrong = original.clone();
    wrong["crs"]["properties"]["name"] = json!("EPSG:3857");
    assert!(normalize_for_display(&mut wrong, &p).is_err());
    let mut unknown = p.clone();
    unknown.geoboundaries.as_mut().unwrap().source_crs = Some(wrong["crs"].clone());
    assert!(validate_provenance(&unknown, 2).is_err());
}

#[tokio::test]
#[ignore = "Explicit live publisher-network acceptance; downloads complete original CHN ADM0 and ADM1 files"]
async fn live_gbopen_complete_country_files_download_save_and_reopen() {
    let output = std::env::var_os("GEOD_GBOPEN_LIVE_OUTPUT")
        .map(std::path::PathBuf::from)
        .expect("Set GEOD_GBOPEN_LIVE_OUTPUT to a task-owned acceptance directory");
    tokio::fs::create_dir_all(&output).await.unwrap();
    let manager = JobManager::open(output.join("store")).await.unwrap();
    let s = manager
        .connect_feature_service(ConnectRequest {
            name: "geoBoundaries · China · gbOpen".into(),
            url: ROOT.into(),
            protocol: "geoBoundaries".into(),
        })
        .await
        .unwrap();
    let mut acceptance = Vec::new();
    let mut saved = Vec::new();
    for (level, count) in [("ADM0", 1), ("ADM1", 34)] {
        let request = QueryRequest {
            service_id: s.id.clone(),
            collection_id: level.into(),
            bounds: [115.8, 39.8, 116.2, 39.95],
            area_geometry: None,
            page_size: None,
            response_format: None,
        };
        let asset = manager.query_features(request).await.unwrap();
        assert_eq!(asset.feature_count, count);
        let inspection = manager.inspect_vector(&asset.id).await.unwrap();
        let p = asset.remote_source.as_ref().unwrap();
        let gb = p.geoboundaries.as_ref().unwrap();
        assert_eq!(gb.layer.admin_level, level);
        assert_eq!(gb.layer.country_iso3, "CHN");
        assert_eq!(p.selection, "geoboundaries-full-administrative-level");
        assert!(gb
            .resolved_release_commit
            .starts_with(&gb.layer.release_ref));
        let managed: Value =
            serde_json::from_slice(&manager.vector_original_bytes(&asset.id).await.unwrap())
                .unwrap();
        assert_eq!(managed.get("crs"), gb.source_crs.as_ref());
        assert!(inspection.geojson.get("crs").is_none());
        assert_eq!(managed["features"], inspection.geojson["features"]);
        let properties: Vec<_> = inspection.geojson["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["properties"].clone())
            .collect();
        acceptance.push(json!({"level":level,"asset":asset,"firstProperties":properties[0],"propertiesSha256":hash(&serde_json::to_vec(&properties).unwrap())}));
        saved.push((inspection.asset.id, properties));
    }
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(output.join("store")).await.unwrap();
    for (id, expected) in saved {
        let inspected = reopened.inspect_vector(&id).await.unwrap();
        let properties: Vec<_> = inspected.geojson["features"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["properties"].clone())
            .collect();
        assert_eq!(properties, expected);
        assert!(inspected
            .asset
            .remote_source
            .unwrap()
            .geoboundaries
            .is_some());
    }
    tokio::fs::write(output.join("acceptance.json"),serde_json::to_vec_pretty(&json!({"testedAt":now(),"officialDocumentation":"https://www.geoboundaries.org/api.html","service":s,"files":acceptance,"reopened":true})).unwrap()).await.unwrap();
}
