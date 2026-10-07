use super::*;
use axum::{
    extract::{Path, State},
    http::StatusCode,
    routing::get,
    Json, Router,
};
use serde_json::json;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

const ID: &str = "LC09_L2SP_044034_20250628_02_T1";
const PRODUCT: &str = "LC09_L2SP_044034_20250628_20250629_02_T1";
const S2: &str = "S2C_MSIL2A_20250627T184941_R113_T10SEG_20250627T234511";
const MODIS_ID: &str = "MYD09A1.A2025177.h08v05.061.2025189031924";
#[test]
fn vegetation_catalogue_requires_matching_period_tile_index_and_calibration() {
    let id = "MOD13Q1.A2025177.h08v05.061.2025195142416";
    let mut value = json!({"id":id,"collection":"modis-13Q1-061","properties":{
        "start_datetime":"2025-06-26T00:00:00Z","end_datetime":"2025-07-11T23:59:59Z",
        "modis:horizontal-tile":8,"modis:vertical-tile":5},"assets":{}});
    for (key, name) in &vegetation::ASSETS[..2] {
        value["assets"][name] = json!({"href":format!("https://{}/modis-061-cogs/MOD13Q1/08/05/2025177/{id}{}",modis::HOST,vegetation::suffix(key).unwrap()),
            "raster:bands":[{"data_type":"int16","scale":0.0001,"unit":key.to_uppercase(),"spatial_resolution":250}]});
    }
    assert_eq!(
        CatalogueItem::from_value(value.clone(), &VEGETATION, id)
            .unwrap()
            .assets
            .len(),
        2
    );
    for (pointer, replacement) in [
        ("/properties/platform", json!("aqua")),
        ("/properties/platform", json!(false)),
        ("/properties/end_datetime", json!("2025-07-03T23:59:59Z")),
        ("/properties/modis:horizontal-tile", json!(9)),
        (
            "/assets/250m_16_days_NDVI/raster:bands/0/data_type",
            json!("uint16"),
        ),
        (
            "/assets/250m_16_days_NDVI/raster:bands/0/unit",
            json!("EVI"),
        ),
        (
            "/assets/250m_16_days_EVI/raster:bands/0/scale",
            json!(10000),
        ),
        (
            "/assets/250m_16_days_EVI/raster:bands/0/spatial_resolution",
            json!(500),
        ),
    ] {
        let mut bad = value.clone();
        if pointer == "/properties/platform" {
            bad["properties"]["platform"] = replacement;
        } else {
            *bad.pointer_mut(pointer).unwrap() = replacement;
        }
        assert!(
            CatalogueItem::from_value(bad, &VEGETATION, id).is_err(),
            "{pointer}"
        );
    }
    value["assets"]["250m_16_days_NDVI"]["raster:bands"][0]["nodata"] = json!(-28672);
    assert!(CatalogueItem::from_value(value, &VEGETATION, id).is_err());
}
fn modis_href(key: &str) -> String {
    let band = match key {
        "red" => "01",
        "green" => "04",
        _ => "03",
    };
    format!(
        "https://{}/modis-061-cogs/MYD09A1/08/05/2025177/{MODIS_ID}_sur_refl_b{band}.tif",
        modis::HOST
    )
}

fn landsat(key: &str) -> String {
    let band = match key {
        "red" => 4,
        "green" => 3,
        "blue" => 2,
        _ => panic!("test band"),
    };
    format!("https://{LANDSAT_HOST}/landsat-c2/level-2/standard/oli-tirs/2025/044/034/{PRODUCT}/{PRODUCT}_SR_B{band}.TIF")
}
#[test]
fn landsat_quality_catalogue_requires_original_unsigned_flags_and_exact_item() {
    let mut value = json!({"id":ID,"collection":"landsat-c2-l2","assets":{
        "qa_pixel":{"href":landsat("red").replace("_SR_B4.TIF","_QA_PIXEL.TIF"),"raster:bands":[{"data_type":"uint16","spatial_resolution":30,"nodata":1}]},
        "qa_radsat":{"href":landsat("red").replace("_SR_B4.TIF","_QA_RADSAT.TIF"),"raster:bands":[{"data_type":"uint16","spatial_resolution":30}]}}});
    assert_eq!(
        CatalogueItem::from_value(value.clone(), &LANDSAT, ID)
            .unwrap()
            .assets
            .len(),
        2
    );
    for (path, replacement) in [
        ("/assets/qa_pixel/raster:bands/0/data_type", json!("int16")),
        ("/assets/qa_pixel/raster:bands/0/nodata", json!(0)),
        (
            "/assets/qa_radsat/raster:bands/0/spatial_resolution",
            json!(20),
        ),
        ("/assets/qa_radsat/raster:bands/0/nodata", json!(65535)),
        ("/assets/qa_pixel/href", json!(landsat("green"))),
    ] {
        let mut bad = value.clone();
        if path.ends_with("/nodata") && bad.pointer(path).is_none() {
            bad["assets"]["qa_radsat"]["raster:bands"][0]["nodata"] = replacement;
        } else {
            *bad.pointer_mut(path).unwrap() = replacement;
        }
        assert!(CatalogueItem::from_value(bad, &LANDSAT, ID).is_err());
    }
    value["assets"]["qa_pixel"]["raster:bands"][0]["scale"] = json!(0.0000275);
    assert!(CatalogueItem::from_value(value, &LANDSAT, ID).is_err());
}
fn sentinel(key: &str) -> String {
    let tail = if key == "scl" {
        "R20m/T10SEG_20250627T184941_SCL_20m.tif"
    } else {
        "R10m/T10SEG_20250627T184941_TCI_10m.tif"
    };
    format!("https://{PC_HOST}/sentinel2-l2/10/S/EG/2025/06/27/S2C_MSIL2A_20250627T184941_N0511_R113_T10SEG_20250627T234511.SAFE/GRANULE/L2A_T10SEG_A004230_20250627T185915/IMG_DATA/{tail}")
}
fn token_value(signature: &str) -> Value {
    let expiry = (chrono::Utc::now() + chrono::Duration::hours(1)).to_rfc3339();
    let token = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("sp", "r")
        .append_pair("sr", "c")
        .append_pair("se", &expiry)
        .append_pair("sig", signature)
        .finish();
    json!({"msft:expiry": expiry, "token": token})
}

#[derive(Default)]
struct Fixture {
    catalogues: AtomicUsize,
    tokens: AtomicUsize,
    limit: AtomicUsize,
    wrong: AtomicUsize,
    slow: AtomicUsize,
    entered: tokio::sync::Notify,
}
async fn item(
    State(f): State<Arc<Fixture>>,
    Path((collection, id)): Path<(String, String)>,
) -> Json<Value> {
    f.catalogues.fetch_add(1, Ordering::SeqCst);
    if collection == "sentinel-1-rtc" {
        let mut value = radar_item(&id);
        if f.wrong.load(Ordering::SeqCst) != 0 {
            value["id"] = json!("wrong");
        }
        return Json(value);
    }
    let assets = if collection == "landsat-c2-l2" {
        json!({"red":{"href":landsat("red")},"green":{"href":landsat("green")},"blue":{"href":landsat("blue")}})
    } else if collection == "modis-09A1-061" {
        let mut assets = serde_json::Map::new();
        for (key, band) in [("red", "01"), ("green", "04"), ("blue", "03")] {
            assets.insert(format!("sur_refl_b{band}"),json!({"href":modis_href(key),"eo:bands":[{"common_name":key}],"raster:bands":[{"data_type":"int16","scale":0.0001,"spatial_resolution":500}]}));
        }
        Value::Object(assets)
    } else if collection == "naip" {
        json!({"image":{"href":"https://naipeuwest.blob.core.windows.net/naip/v002/ca/2022/ca_060cm_2022/37122/m_3712221_nw_10_060_20220518.tif","eo:bands":[{"common_name":"red"},{"common_name":"green"},{"common_name":"blue"},{"common_name":"nir"}]}})
    } else {
        json!({"visual":{"href":sentinel("visual")},"SCL":{"href":sentinel("scl")}})
    };
    Json(
        json!({"id":if f.wrong.load(Ordering::SeqCst) == 0 { id } else { "wrong".into() },"collection":collection,"assets":assets}),
    )
}

fn radar_item(id: &str) -> Value {
    let catalogue: Value = serde_json::from_str(include_str!(
        "../../../../../prototype/qa/sentinel-1-rtc-catalog.json"
    ))
    .unwrap();
    let current: Value = serde_json::from_str(include_str!(
        "../../../../../prototype/qa/sentinel-1d-rtc-catalog.json"
    ))
    .unwrap();
    catalogue["features"]
        .as_array()
        .unwrap()
        .iter()
        .chain(current["features"].as_array().unwrap().iter())
        .find(|item| item["id"].as_str() == Some(id))
        .unwrap()
        .clone()
}

const RADAR_ID: &str = "S1C_IW_GRDH_1SDV_20250630T140654_20250630T140719_003013_rtc";
const RADAR_D_ID: &str = "S1D_IW_GRDH_1SDV_20260930T140731_20260930T140756_004808_00905F_rtc";

#[tokio::test]
async fn sentinel_1d_access_pins_the_official_item_and_both_polarizations() {
    let (cache, client, state, server) = fixture().await;
    let item = radar_item(RADAR_D_ID);
    for key in ["vv", "vh"] {
        let href = item["assets"][key]["href"].as_str().unwrap();
        let signed = cache.resolve(&client, href, RADAR_D_ID, key).await.unwrap();
        assert!(signed
            .query_pairs()
            .any(|(k, value)| k == "sp" && value == "r"));
        let mut unsigned = signed;
        unsigned.set_query(None);
        assert_eq!(unsigned.as_str(), href);
        let other_key = if key == "vv" { "vh" } else { "vv" };
        assert!(cache
            .resolve(&client, href, RADAR_D_ID, other_key)
            .await
            .is_err());
        // Matching scene tokens alone must not authorize a different source file.
        let other_product = href.replace("00905F_1D03", "00905F_FFFF");
        assert!(cache
            .resolve(&client, &other_product, RADAR_D_ID, key)
            .await
            .is_err());
    }
    assert_eq!(state.catalogues.load(Ordering::SeqCst), 1);
    assert_eq!(state.tokens.load(Ordering::SeqCst), 1);
    server.abort();
}

#[tokio::test]
async fn radar_checks_exact_catalogue_polarizations_and_coalesces_read_only_access() {
    let (cache, client, state, server) = fixture().await;
    let item = radar_item(RADAR_ID);
    let vv = item["assets"]["vv"]["href"].as_str().unwrap();
    let vh = item["assets"]["vh"]["href"].as_str().unwrap();
    let (a, b) = tokio::join!(
        cache.resolve(&client, vv, RADAR_ID, "vv"),
        cache.resolve(&client, vh, RADAR_ID, "vh")
    );
    for (signed, original) in [(a.unwrap(), vv), (b.unwrap(), vh)] {
        assert!(signed
            .query_pairs()
            .any(|(key, value)| key == "sp" && value == "r"));
        let mut unsigned = signed;
        unsigned.set_query(None);
        assert_eq!(unsigned.as_str(), original);
    }
    assert_eq!(state.catalogues.load(Ordering::SeqCst), 1);
    assert_eq!(state.tokens.load(Ordering::SeqCst), 1);
    assert!(cache.resolve(&client, vv, RADAR_ID, "vh").await.is_err());
    // A valid source-derived ID alias still requires an exact official item match.
    let other = vv.replace("00622B_A30F", "00622B_FFFF");
    assert!(cache
        .resolve(&client, &other, RADAR_ID, "vv")
        .await
        .is_err());
    assert_eq!(state.tokens.load(Ordering::SeqCst), 1);
    let provenance = crate::providers::source_name(vv);
    assert!(provenance.contains("Sentinel-1 IW"));
    assert!(provenance.contains("Catalyst"));
    assert!(provenance.contains("CC-BY-4.0"));
    server.abort();
}

#[test]
fn radar_rejects_wrong_science_profile_or_catalogue_identity() {
    for id in [RADAR_ID, RADAR_D_ID] {
        let good = radar_item(id);
        assert!(CatalogueItem::from_value(good.clone(), &RADAR, id).is_ok());
        for (pointer, value) in [
            ("/properties/sar:instrument_mode", json!("EW")),
            ("/properties/sar:polarizations", json!(["HH", "HV"])),
            ("/assets/vv/raster:bands/0/data_type", json!("uint16")),
            ("/assets/vv/raster:bands/0/nodata", json!(0)),
            ("/assets/vv/raster:bands/0/spatial_resolution", json!(30)),
            ("/collection", json!("sentinel-1-grd")),
            ("/id", json!("another-scene")),
        ] {
            let mut bad = good.clone();
            *bad.pointer_mut(pointer).unwrap() = value;
            assert!(
                CatalogueItem::from_value(bad, &RADAR, id).is_err(),
                "{pointer}"
            );
        }
    }
}

#[tokio::test]
async fn modis_checks_actual_cog_calibration_and_coalesces_container_access() {
    let (cache, client, state, server) = fixture().await;
    let red_href = modis_href("red");
    let green_href = modis_href("green");
    let (red, green) = tokio::join!(
        cache.resolve(&client, &red_href, MODIS_ID, "red"),
        cache.resolve(&client, &green_href, MODIS_ID, "green")
    );
    assert!(red.unwrap().query().is_some());
    assert!(green.unwrap().query().is_some());
    assert_eq!(state.catalogues.load(Ordering::SeqCst), 1);
    assert_eq!(state.tokens.load(Ordering::SeqCst), 1);
    assert!(cache
        .resolve(&client, &modis_href("red"), MODIS_ID, "blue")
        .await
        .is_err());
    let value = json!({"id":MODIS_ID,"collection":"modis-09A1-061","assets":{"sur_refl_b01":{"href":modis_href("red"),"eo:bands":[{"common_name":"red"}],"raster:bands":[{"data_type":"uint16","scale":0.0000275,"spatial_resolution":30}]}}});
    assert!(CatalogueItem::from_value(value, &MODIS, MODIS_ID).is_err());
    server.abort();
}
async fn access(State(f): State<Arc<Fixture>>) -> axum::response::Response {
    use axum::response::IntoResponse;
    let count = f.tokens.fetch_add(1, Ordering::SeqCst) + 1;
    if f.slow.load(Ordering::SeqCst) != 0 {
        f.entered.notify_one();
        tokio::time::sleep(Duration::from_millis(150)).await;
    }
    if f.limit.load(Ordering::SeqCst) != 0 {
        return (
            StatusCode::TOO_MANY_REQUESTS,
            [("retry-after", "120")],
            "SECRET-RESPONSE",
        )
            .into_response();
    }
    Json(token_value(&format!("EPHEMERAL-{count}"))).into_response()
}
async fn fixture() -> (
    AccessCache,
    reqwest::Client,
    Arc<Fixture>,
    tokio::task::JoinHandle<()>,
) {
    let f = Arc::new(Fixture::default());
    let app = Router::new()
        .route(
            "/api/stac/v1/collections/{collection}/items/{id}",
            get(item),
        )
        .route("/api/sas/v1/token/{account}/{container}", get(access))
        .with_state(f.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let origin = format!("http://{}", listener.local_addr().unwrap());
    let server = tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    let cache = AccessCache {
        api: Some(origin),
        ..Default::default()
    };
    let client = crate::proxy::download_client(&crate::ProxySettings {
        mode: crate::proxy::ProxyMode::Direct,
        url: None,
    })
    .unwrap();
    (cache, client, f, server)
}

#[tokio::test]
async fn naip_uses_its_own_reviewed_container_catalogue_and_coalesced_signature() {
    let (cache, client, state, server) = fixture().await;
    let href="https://naipeuwest.blob.core.windows.net/naip/v002/ca/2022/ca_060cm_2022/37122/m_3712221_nw_10_060_20220518.tif";
    let id = "ca_m_3712221_nw_10_060_20220518";
    let (a, b) = tokio::join!(
        cache.resolve(&client, href, id, "aerial"),
        cache.resolve(&client, href, id, "aerial")
    );
    assert_eq!(a.unwrap(), b.unwrap());
    assert_eq!(state.catalogues.load(Ordering::SeqCst), 1);
    assert_eq!(state.tokens.load(Ordering::SeqCst), 1);
    let bad = json!({"id":id,"collection":"naip","assets":{"image":{"href":href,"eo:bands":[{"common_name":"red"},{"common_name":"green"},{"common_name":"blue"},{"common_name":"alpha"}]}}});
    assert!(CatalogueItem::from_value(bad, &NAIP, id).is_err());
    server.abort();
}

#[test]
fn captured_naip_variants_require_the_exact_official_catalogue_id_and_four_bands() {
    let captured: Value = serde_json::from_str(include_str!(
        "../../../../../prototype/qa/naip-resolution-catalog.json"
    ))
    .unwrap();
    for item in captured["features"].as_array().unwrap() {
        let id = item["id"].as_str().unwrap();
        let resolved = CatalogueItem::from_value(item.clone(), &NAIP, id).unwrap();
        assert_eq!(resolved.assets["aerial"], item["assets"]["image"]["href"]);
        // The filename alias never permits substituting a different catalogue record.
        assert!(CatalogueItem::from_value(item.clone(), &NAIP, &format!("{id}_20240101")).is_err());
        let mut wrong_band = item.clone();
        wrong_band["assets"]["image"]["eo:bands"][3]["common_name"] = json!("alpha");
        assert!(CatalogueItem::from_value(wrong_band, &NAIP, id).is_err());
        let mut wrong_grid = item.clone();
        let href = item["assets"]["image"]["href"].as_str().unwrap();
        let original_grid = id.split('_').nth(2).unwrap();
        wrong_grid["assets"]["image"]["href"] = json!(href.replace(original_grid, "0000000"));
        assert!(CatalogueItem::from_value(wrong_grid, &NAIP, id).is_err());
    }
}

#[test]
fn captured_legacy_naip_half_metre_filenames_keep_full_catalogue_ids_and_nir_roles() {
    let captured: Value = serde_json::from_str(include_str!(
        "../../../../../prototype/qa/naip-legacy-catalog.json"
    ))
    .unwrap();
    let items = captured["features"].as_array().unwrap();
    assert_eq!(items.len(), 15);
    for item in items {
        let id = item["id"].as_str().unwrap();
        let resolved = CatalogueItem::from_value(item.clone(), &NAIP, id).unwrap();
        assert_eq!(resolved.assets["aerial"], item["assets"]["image"]["href"]);
        assert_eq!(super::super::naip_pixel_size(id), Some(0.6));
        for wrong_id in [
            id.replace("_.6_", "_060_"),
            id.replace("_10_", "_11_"),
            format!("{id}_20250101"),
        ] {
            assert!(CatalogueItem::from_value(item.clone(), &NAIP, &wrong_id).is_err());
        }
        let mut wrong = item.clone();
        wrong["assets"]["image"]["eo:bands"][3]["common_name"] = json!("alpha");
        assert!(CatalogueItem::from_value(wrong, &NAIP, id).is_err());
        let mut wrong = item.clone();
        let href = item["assets"]["image"]["href"].as_str().unwrap();
        wrong["assets"]["image"]["href"] = json!(href.replace("060cm", "100cm"));
        assert!(CatalogueItem::from_value(wrong, &NAIP, id).is_err());
    }
}

#[test]
fn container_access_rejects_ambiguous_writable_expired_or_fragment_tokens() {
    let good = token_value("SENSITIVE+/=");
    assert!(Token::from_value(&good).is_ok());
    let raw = good["token"].as_str().unwrap();
    for invalid in [
        raw.replace("sp=r", "sp=rw"),
        raw.replace("sr=c", "sr=b"),
        format!("{raw}&sig=OTHER"),
        format!("{raw}&sp=w"),
        format!("{raw}#fragment"),
        raw.replace("sig=SENSITIVE%2B%2F%3D", "sig="),
        raw.replace("se=", "missing="),
        "x".repeat(8193),
    ] {
        let mut bad = good.clone();
        bad["token"] = Value::String(invalid);
        let error = match Token::from_value(&bad) {
            Ok(_) => panic!("invalid access accepted"),
            Err(e) => e,
        };
        assert!(!error.contains("SENSITIVE"));
    }
    let mut expired = good.clone();
    expired["msft:expiry"] = json!("2020-01-01T00:00:00Z");
    assert!(Token::from_value(&expired).is_err());
    let mut earlier = good;
    earlier["token"] = json!("sp=r&sr=c&se=2020-01-01T00%3A00%3A00Z&sig=SENSITIVE");
    assert!(Token::from_value(&earlier).is_err());
    // The official API field is msft:expiry; an unrelated expiry must not be accepted.
    let mut wrong_field = token_value("PRIVATE");
    wrong_field["expiry"] = wrong_field["msft:expiry"].take();
    assert!(Token::from_value(&wrong_field).is_err());
}

#[tokio::test]
async fn concurrent_bands_share_one_catalogue_and_one_token_and_containers_are_separate() {
    let (cache, client, f, server) = fixture().await;
    let keys = ["red", "green", "blue", "red", "green", "blue"];
    let hrefs: Vec<_> = keys.iter().map(|key| landsat(key)).collect();
    let urls = futures_util::future::join_all(
        keys.iter()
            .zip(&hrefs)
            .map(|(key, href)| cache.resolve(&client, href, ID, key)),
    )
    .await;
    for (url, href) in urls.into_iter().zip(hrefs) {
        let mut url = url.unwrap();
        assert_eq!(
            url.query_pairs().find(|(k, _)| k == "sig").unwrap().1,
            "EPHEMERAL-1"
        );
        url.set_query(None);
        assert_eq!(url.as_str(), href);
    }
    assert_eq!(f.catalogues.load(Ordering::SeqCst), 1);
    assert_eq!(f.tokens.load(Ordering::SeqCst), 1);
    cache
        .resolve(&client, &sentinel("scl"), S2, "scl")
        .await
        .unwrap();
    cache
        .resolve(&client, &sentinel("visual"), S2, "visual")
        .await
        .unwrap();
    assert_eq!(f.catalogues.load(Ordering::SeqCst), 2);
    assert_eq!(f.tokens.load(Ordering::SeqCst), 2);
    assert_eq!(cache.landsat.lock().await.catalogue.len(), 1);
    assert_eq!(cache.sentinel.lock().await.catalogue.len(), 1);
    server.abort();
}

#[tokio::test]
async fn expiry_refreshes_token_and_stale_catalogue_is_rechecked_without_changing_asset() {
    let (cache, client, f, server) = fixture().await;
    cache
        .resolve(&client, &landsat("red"), ID, "red")
        .await
        .unwrap();
    {
        let mut state = cache.landsat.lock().await;
        state.token.as_mut().unwrap().expiry = chrono::Utc::now().timestamp() + 59;
    }
    let url = cache
        .resolve(&client, &landsat("green"), ID, "green")
        .await
        .unwrap();
    assert_eq!(
        url.query_pairs().find(|(k, _)| k == "sig").unwrap().1,
        "EPHEMERAL-2"
    );
    assert_eq!(f.catalogues.load(Ordering::SeqCst), 1);
    assert_eq!(f.tokens.load(Ordering::SeqCst), 2);
    {
        cache
            .landsat
            .lock()
            .await
            .catalogue
            .get_mut(ID)
            .unwrap()
            .checked = Instant::now() - CATALOG_TTL;
    }
    f.wrong.store(1, Ordering::SeqCst);
    assert!(cache
        .resolve(&client, &landsat("blue"), ID, "blue")
        .await
        .unwrap_err()
        .contains("catalogue"));
    assert_eq!(f.catalogues.load(Ordering::SeqCst), 2);
    assert_eq!(f.tokens.load(Ordering::SeqCst), 2);
    assert!(cache.landsat.lock().await.catalogue.is_empty());
    server.abort();
}

#[tokio::test]
async fn invalid_product_or_channel_never_fetches_access_and_cached_catalogue_pins_full_path() {
    let (cache, client, f, server) = fixture().await;
    for (href, id, key) in [
        (landsat("red"), ID, "blue"),
        (landsat("red"), "other", "red"),
        (format!("{}?sig=SENSITIVE", landsat("red")), ID, "red"),
    ] {
        assert!(cache.resolve(&client, &href, id, key).await.is_err());
    }
    assert_eq!(f.catalogues.load(Ordering::SeqCst), 0);
    assert_eq!(f.tokens.load(Ordering::SeqCst), 0);
    cache
        .resolve(&client, &landsat("red"), ID, "red")
        .await
        .unwrap();
    // Same acquisition id with a different processing date cannot borrow catalogue approval.
    let other = landsat("green").replace("20250629", "20250630");
    assert!(cache.resolve(&client, &other, ID, "green").await.is_err());
    assert_eq!(f.catalogues.load(Ordering::SeqCst), 1);
    assert_eq!(f.tokens.load(Ordering::SeqCst), 1);
    server.abort();
}

#[tokio::test]
async fn rate_limited_batch_shares_cooldown_and_can_retry_after_it() {
    let (cache, client, f, server) = fixture().await;
    f.limit.store(1, Ordering::SeqCst);
    let hrefs: Vec<_> = ["red", "green", "blue"]
        .iter()
        .map(|key| landsat(key))
        .collect();
    let results = futures_util::future::join_all(
        ["red", "green", "blue"]
            .iter()
            .zip(&hrefs)
            .map(|(key, href)| cache.resolve(&client, href, ID, key)),
    )
    .await;
    for r in results {
        assert_eq!(r.unwrap_err(), RATE_LIMITED);
    }
    assert_eq!(f.tokens.load(Ordering::SeqCst), 1);
    assert_eq!(f.catalogues.load(Ordering::SeqCst), 1);
    f.limit.store(0, Ordering::SeqCst);
    cache.landsat.lock().await.limited_until = Some(Instant::now() - Duration::from_secs(1));
    assert!(cache
        .resolve(&client, &landsat("red"), ID, "red")
        .await
        .is_ok());
    assert_eq!(f.tokens.load(Ordering::SeqCst), 2);
    server.abort();
}

#[tokio::test]
async fn cancelled_request_releases_coalescing_lock_for_a_new_worker() {
    let (cache, client, f, server) = fixture().await;
    f.slow.store(1, Ordering::SeqCst);
    {
        let href = landsat("red");
        let resolving = cache.resolve(&client, &href, ID, "red");
        tokio::pin!(resolving);
        tokio::time::timeout(Duration::from_secs(2), async {
            tokio::select! {
                _ = &mut resolving => panic!("Slow fixture should still be pending"),
                _ = f.entered.notified() => (),
            }
        })
        .await
        .unwrap();
        // Dropping the caller's future cancels its in-flight metadata request.
    }
    f.slow.store(0, Ordering::SeqCst);
    assert!(tokio::time::timeout(
        Duration::from_secs(1),
        cache.resolve(&client, &landsat("green"), ID, "green")
    )
    .await
    .unwrap()
    .is_ok());
    assert_eq!(f.catalogues.load(Ordering::SeqCst), 1);
    server.abort();
}

#[test]
fn catalogue_memory_is_bounded_and_evicts_oldest_snapshot() {
    let mut state = ContainerState::default();
    for i in 0..CATALOG_LIMIT {
        state.catalogue.insert(
            i.to_string(),
            CatalogueItem {
                assets: BTreeMap::new(),
                checked: Instant::now(),
            },
        );
    }
    state.catalogue.get_mut("0").unwrap().checked = Instant::now() - Duration::from_secs(1);
    state.prune();
    assert_eq!(state.catalogue.len(), CATALOG_LIMIT);
    state.reserve_catalogue_slot();
    assert_eq!(state.catalogue.len(), CATALOG_LIMIT - 1);
    assert!(!state.catalogue.contains_key("0"));
}

#[tokio::test]
#[ignore = "Reads actual public Planetary Computer Sentinel COG data"]
async fn live_sentinel_container_access_supports_both_reviewed_assets() {
    let cache = AccessCache::default();
    let client = crate::proxy::download_client(&crate::ProxySettings::default()).unwrap();
    let hrefs = [sentinel("visual"), sentinel("scl")];
    let urls = futures_util::future::join_all(
        ["visual", "scl"]
            .iter()
            .zip(&hrefs)
            .map(|(key, href)| cache.resolve(&client, href, S2, key)),
    )
    .await;
    for result in urls {
        let response = client
            .get(result.unwrap())
            .header(reqwest::header::RANGE, "bytes=0-4095")
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::PARTIAL_CONTENT);
        let bytes = response.bytes().await.unwrap();
        assert_eq!(bytes.len(), 4096);
        crate::verify_signature(&bytes, "image/tiff").unwrap();
    }
    let state = cache.sentinel.lock().await;
    assert_eq!(state.requests, 2);
    println!("Actual Sentinel visual/SCL: two HTTPS 206 responses, 4096 bytes each; one catalogue + one container SAS.");
}

#[tokio::test]
#[ignore = "Downloads 281 MB of actual public Landsat data into GEOD_LIVE_BATCH_DIR"]
async fn live_public_batch_downloads_match_verified_originals_without_persisting_sas() {
    use crate::{CreateJobRequest, JobManager, JobStatus};
    let root = std::env::var("GEOD_LIVE_BATCH_DIR").expect("Set an isolated data directory");
    let manager = JobManager::open(&root).await.unwrap();
    assert!(
        manager.list().await.is_empty(),
        "Use a new isolated verification directory"
    );
    let mut jobs = Vec::new();
    for key in ["red", "green", "blue"] {
        jobs.push(
            manager
                .create(CreateJobRequest {
                    item_id: ID.into(),
                    asset_key: key.into(),
                    href: landsat(key),
                    media_type: "image/tiff; application=geotiff; profile=cloud-optimized".into(),
                    title: None,
                })
                .await
                .unwrap(),
        );
    }
    let finished = tokio::time::timeout(
        Duration::from_secs(600),
        futures_util::future::join_all(jobs.iter().map(|job| manager.wait(&job.id))),
    )
    .await
    .unwrap();
    let expected = [
        (
            95_591_695,
            "3642840ee224c5af67a398f12532adc0f8d09d0eded5e8a1e512e01c09e4f546",
        ),
        (
            93_388_645,
            "679c701ee7e0b0d9f6abc3dd49ebb0a58a9d4692b527f9e4643b7f35a4b2c1db",
        ),
        (
            92_080_083,
            "e350eebc6dc62e40fd0f6ed7a8b4419be4a2787a897391e8ddbfaf59d2fb9cd1",
        ),
    ];
    for (job, (size, hash)) in finished.into_iter().zip(expected) {
        let job = job.unwrap();
        assert_eq!(job.status, JobStatus::Succeeded, "{:?}", job.error);
        assert_eq!(job.bytes_downloaded, size);
        assert_eq!(job.sha256.as_deref(), Some(hash));
        assert!(Url::parse(&job.href).unwrap().query().is_none());
    }
    let state = manager.inner.planetary_access.landsat.lock().await;
    assert_eq!(
        state.requests, 2,
        "Exactly one catalogue lookup and one container token request"
    );
    assert_eq!(state.catalogue.len(), 1);
    drop(state);
    let stored = std::fs::read_to_string(std::path::Path::new(&root).join("jobs.json")).unwrap();
    assert!(!stored.contains("sig=") && !stored.contains("EPHEMERAL"));
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(&root).await.unwrap();
    assert_eq!(reopened.list().await.len(), 3);
    assert_eq!(
        reopened
            .inner
            .planetary_access
            .landsat
            .lock()
            .await
            .requests,
        0
    );
    for job in reopened.list().await {
        assert_eq!(job.status, JobStatus::Succeeded);
    }
    reopened.shutdown().await.unwrap();
    println!("Actual Landsat batch: 3 files, 281060423 bytes, original SHA-256 matches; 1 catalogue + 1 container SAS; restart restored unsigned records.");
}
#[tokio::test]
async fn reviewed_search_cache_is_atomic_and_rejects_changed_product_assets() {
    let value: serde_json::Value = serde_json::from_str(include_str!(
        "../../../../../prototype/public/samples/planetary-computer-response.json"
    ))
    .unwrap();
    let cache = super::AccessCache::default();
    cache
        .observe_catalogue(&value, "sentinel-2-l2a")
        .await
        .unwrap();
    let expected = value["features"].as_array().unwrap().len();
    assert_eq!(cache.sentinel.lock().await.catalogue.len(), expected);
    let mut substituted = value;
    substituted["features"][0]["assets"]["SCL"]["href"] =
        serde_json::json!("https://example.com/forged.tif");
    assert!(cache
        .observe_catalogue(&substituted, "sentinel-2-l2a")
        .await
        .is_err());
    assert_eq!(cache.sentinel.lock().await.catalogue.len(), expected);
}
