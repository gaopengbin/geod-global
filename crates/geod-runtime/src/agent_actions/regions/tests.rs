use super::*;
#[tokio::test]
async fn offline_source_polygons_are_available_across_countries_with_original_precision() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    for (name, country, level) in [
        ("北京市", "CHN", 1),
        ("Bayern", "DEU", 1),
        ("東京都", "JPN", 1),
        ("Gauteng", "ZAF", 1),
        ("Hong Kong", "HKG", 0),
    ] {
        let result = search(&manager, query(name, Some(country), Some(level)))
            .await
            .unwrap();
        let source = &result["candidates"][0]["boundarySource"];
        let (_, geometry, provenance) = boundary(
            &manager,
            source["provider"].as_str().unwrap(),
            source["candidateId"].as_str().unwrap(),
            country,
            level,
        )
        .await
        .unwrap();
        assert_eq!(provenance["provider"], "Natural Earth");
        assert!(geometry.bounds().is_ok());
        assert!(serde_json::to_vec(&geometry).unwrap().len() > 100);
    }
    assert!(
        boundary(&manager, "natural-earth", "ne:ADM1:USA-3513", "JPN", 1)
            .await
            .is_err()
    );
    manager.shutdown().await.unwrap();
}
fn query(name: &str, country: Option<&str>, level: Option<u8>) -> Query {
    Query {
        query: name.into(),
        country_code: country.map(String::from),
        admin_level: level,
        limit: 5,
    }
}
#[tokio::test]
async fn global_offline_chinese_native_diacritics_and_real_extents() {
    let dir = tempfile::tempdir().unwrap();
    let manager = JobManager::open(dir.path()).await.unwrap();
    for (name, country, level, expected) in [
        ("中国", None, 0, "CHN"),
        ("加利福尼亚", "US".into(), 1, "USA"),
        ("Bayern", "DE".into(), 1, "DEU"),
        ("東京都", "JP".into(), 1, "JPN"),
        ("Sao Paulo", "BR".into(), 1, "BRA"),
        ("Ardeche", "FR".into(), 1, "FRA"),
        ("Gauteng", "ZA".into(), 1, "ZAF"),
        ("New South Wales", "AU".into(), 1, "AUS"),
    ] {
        let response = search(&manager, query(name, country, Some(level)))
            .await
            .unwrap();
        let candidates = response["candidates"].as_array().unwrap();
        assert!(!candidates.is_empty(), "{name} {response}");
        assert!(candidates
            .iter()
            .all(|r| r["countryCode"] == expected && r["adminLevel"] == level));
        assert!(candidates
            .iter()
            .all(|r| valid_bounds(serde_json::from_value(r["bounds"].clone()).unwrap())));
        assert_eq!(response["provenance"]["offline"], true);
    }
    assert_eq!(bundled::regions().unwrap().len(), 4838);
    assert_eq!(bundled::iso3("AU").unwrap(), "AUS");
}
#[tokio::test]
async fn disambiguates_country_and_state_and_preserves_region_presentation() {
    let dir = tempfile::tempdir().unwrap();
    let m = JobManager::open(dir.path()).await.unwrap();
    let all = search(&m, query("Georgia", None, None)).await.unwrap();
    assert!(all["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["countryCode"] == "GEO" && r["adminLevel"] == 0));
    assert!(all["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r["countryCode"] == "USA" && r["adminLevel"] == 1));
    let filtered = search(&m, query("Georgia", Some("US"), Some(1)))
        .await
        .unwrap();
    assert_eq!(filtered["matchCount"], 1);
    assert_eq!(filtered["candidates"][0]["countryCode"], "USA");
    for (name, expected) in [
        ("香港特别行政区", "Hong Kong"),
        ("澳門", "Macau"),
        ("臺灣", "Taiwan"),
    ] {
        let r = search(&m, query(name, None, Some(0))).await.unwrap();
        assert_eq!(r["candidates"][0]["name"], expected);
    }
    let absent = search(&m, query("not-a-real-region", Some("DEU"), Some(1)))
        .await
        .unwrap();
    assert_eq!(absent["matchCount"], 0);
}
#[test]
fn rejects_unbounded_queries_and_unknown_parameters_without_network() {
    for q in [
        query("https://localhost", None, None),
        query("../secret", None, None),
        query("x", Some("ALL"), Some(2)),
        query("x", None, Some(2)),
        query("x", Some("usa"), Some(1)),
        query("x", Some("USA"), Some(6)),
    ] {
        assert!(q.validate().is_err());
    }
    assert!(
        serde_json::from_value::<Query>(json!({"query":"Paris","url":"http://localhost"})).is_err()
    );
}
#[tokio::test]
#[ignore = "real global administrative data requests, isolated persistent cache, no imagery downloads"]
async fn live_global_administrative_lookup_and_persistent_cache() {
    let home = PathBuf::from(std::env::var_os("GEOD_REGIONS_QA").expect("isolated QA directory"));
    let mut manager = JobManager::open(home.join("core")).await.unwrap();
    if let Ok(proxy) = std::env::var("GEOD_AGENT_TEST_PROXY") {
        manager
            .save_proxy_settings(crate::ProxySettings {
                mode: crate::proxy::ProxyMode::Custom,
                url: Some(proxy),
            })
            .await
            .unwrap();
    }
    let mut cases = Vec::new();
    // Real lower-level data from Europe, Asia, Africa and the Americas, with
    // provider-specific numbering and exact source-native names.
    for (name, iso, level) in [
        ("Köln", "DEU", 2),
        ("Île-de-France", "FRA", 1),
        ("Paris", "FRA", 2),
        ("Pune", "IND", 2),
        ("Nairobi", "KEN", 1),
        ("Los Angeles", "USA", 2),
    ] {
        let q = query(name, Some(iso), Some(level));
        println!(
            "{}",
            json!({"administrativeCase":name,"countryCode":iso,"adminLevel":level})
        );
        let start = std::time::Instant::now();
        let r = search(&manager, q.clone()).await.unwrap();
        assert!(r["matchCount"].as_u64().unwrap() > 0, "{name} {r}");
        let elapsed = start.elapsed().as_millis();
        manager.shutdown().await.unwrap();
        drop(manager);
        manager = JobManager::open(home.join("core")).await.unwrap();
        let cached = search(&manager, q).await.unwrap();
        if level >= 2 {
            assert_eq!(cached["provenance"]["cached"], true);
        }
        assert_eq!(cached["candidates"], r["candidates"]);
        cases.push(json!({"name":name,"countryCode":iso,"adminLevel":level,"elapsedMs":elapsed,"result":r,"cacheAfterReopen":cached["provenance"]["cached"]}));
    }
    tokio::fs::write(
        home.join("global-administrative-acceptance.json"),
        serde_json::to_vec_pretty(&json!({"checkedAt":now(),"cases":cases,"originalDownloads":0}))
            .unwrap(),
    )
    .await
    .unwrap();
}
