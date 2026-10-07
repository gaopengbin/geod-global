//! Bounded, user-requested place lookup with source-specific provenance.
//! US cities use Census geometry, other places use Photon; extents are search
//! rectangles computed from actual provider data, never invented coordinates.
use super::*;
use std::sync::OnceLock;
use tokio::sync::Mutex;
use tokio::time::Instant;

const ENDPOINT: &str = "https://photon.komoot.io/api/";
const MAX_RESPONSE: usize = 1024 * 1024;
const CACHE_DAYS: i64 = 7;
const LAYERS: &[&str] = &["city", "district", "locality", "county", "state", "country"];
static REQUEST_GATE: OnceLock<Mutex<Option<Instant>>> = OnceLock::new();
mod census;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, Default, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
enum Provider {
    #[default]
    Photon,
    Census,
}
impl Provider {
    fn label(self) -> &'static str {
        match self {
            Self::Photon => "Photon · OpenStreetMap",
            Self::Census => "U.S. Census Bureau · TIGERweb",
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
enum Kind {
    City,
    Region,
    #[default]
    Place,
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Query {
    query: String,
    #[serde(default)]
    kind: Kind,
    country_code: Option<String>,
}
impl Query {
    fn validate(&self) -> Result<()> {
        if !clean(&self.query, 120)
            || self.query.contains("://")
            || self.country_code.as_ref().is_some_and(|code| {
                code.len() != 2 || !code.bytes().all(|b| b.is_ascii_uppercase())
            })
        {
            return Err(
                "Use a place name and, optionally, a two-letter uppercase country code.".into(),
            );
        }
        Ok(())
    }
    fn layers(&self) -> &'static [&'static str] {
        match self.kind {
            Kind::City => &["city"],
            Kind::Region => &["county", "state", "country"],
            Kind::Place => LAYERS,
        }
    }
    fn url(&self) -> url::Url {
        let mut url = url::Url::parse(ENDPOINT).unwrap();
        url.query_pairs_mut()
            .append_pair("q", &self.query)
            .append_pair("limit", "5")
            .append_pair("lang", "en");
        for layer in self.layers() {
            url.query_pairs_mut().append_pair("layer", layer);
        }
        if let Some(code) = &self.country_code {
            url.query_pairs_mut().append_pair("countrycode", code);
        }
        url
    }
    fn cache_id(&self) -> String {
        let normalized = json!([
            "photon-place/v1",
            self.query.to_lowercase(),
            self.kind,
            self.country_code
        ]);
        let hash = Sha256::digest(normalized.to_string().as_bytes());
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&hash[..16]);
        Uuid::from_bytes(bytes).to_string()
    }
    fn providers(&self) -> Vec<Provider> {
        if census::eligible(self) {
            vec![Provider::Census, Provider::Photon]
        } else {
            vec![Provider::Photon]
        }
    }
}
fn clean(value: &str, max: usize) -> bool {
    !value.is_empty()
        && value.trim() == value
        && value.chars().count() <= max
        && !value.chars().any(char::is_control)
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Place {
    name: String,
    kind: String,
    country: Option<String>,
    country_code: Option<String>,
    state: Option<String>,
    county: Option<String>,
    center: [f64; 2],
    bounds: Option<[f64; 4]>,
    #[serde(skip_serializing_if = "Option::is_none")]
    osm_type: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    osm_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    census_geoid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    geometry: Option<crate::crop::PolygonGeometry>,
}
impl Place {
    fn valid(&self, query: &Query) -> bool {
        clean(&self.name, 200)
            && query.layers().contains(&self.kind.as_str())
            && self.center.iter().all(|n| n.is_finite())
            && (-180.0..=180.0).contains(&self.center[0])
            && (-90.0..=90.0).contains(&self.center[1])
            && self.bounds.is_none_or(valid_bounds)
            && self
                .geometry
                .as_ref()
                .is_none_or(|g| self.census_geoid.is_some() && g.bounds().ok() == self.bounds)
            && match (&self.osm_type, self.osm_id, &self.census_geoid) {
                (Some(kind), Some(id), None) => ["N", "W", "R"].contains(&kind.as_str()) && id > 0,
                (None, None, Some(id)) => {
                    id.len() == 7
                        && id.bytes().all(|b| b.is_ascii_digit())
                        && self.country_code.as_deref() == Some("US")
                        && self.kind == "city"
                }
                _ => false,
            }
            && [&self.country, &self.country_code, &self.state, &self.county]
                .iter()
                .all(|value| value.as_ref().is_none_or(|value| clean(value, 200)))
            && query.country_code.as_ref().is_none_or(|code| {
                self.country_code
                    .as_ref()
                    .is_some_and(|v| v.eq_ignore_ascii_case(code))
            })
    }
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Cache {
    #[serde(default)]
    provider: Provider,
    query: Query,
    checked_at: String,
    places: Vec<Place>,
}
impl Cache {
    fn fresh(&self, query: &Query, today: DateTime<Utc>) -> bool {
        self.query.cache_id() == query.cache_id()
            && DateTime::parse_from_rfc3339(&self.checked_at).is_ok_and(|date| {
                let age = today.signed_duration_since(date);
                age >= chrono::Duration::zero() && age < chrono::Duration::days(CACHE_DAYS)
            })
            && self.places.len() <= 5
            && self.places.iter().all(|place| {
                place.valid(query)
                    && (place.census_geoid.is_some() == (self.provider == Provider::Census))
            })
    }
    fn result(&self, cached: bool) -> Value {
        let candidates = self.places.iter().map(|place| {
            let mut value = serde_json::to_value(place).unwrap();
            value.as_object_mut().unwrap().remove("geometry");
            if let Some(id) = &place.census_geoid {
                value["boundarySource"] = json!({"provider":"census","lookupId":self.query.cache_id(),"candidateId":id});
            } else if census::eligible(&self.query) && !place.name.eq_ignore_ascii_case(&self.query.query) {
                // A real gazetteer canonical name can resolve the Census name
                // fields. This is generic source data, not a city-specific alias.
                value["boundarySearch"] = json!({"query":place.name,"kind":"city","countryCode":"US"});
            }
            value
        }).collect::<Vec<_>>();
        json!({"query": self.query.query, "kind": self.query.kind, "lookupId":self.query.cache_id(), "candidates": candidates,
            "checkedAt": self.checked_at, "cached": cached, "cacheDays": CACHE_DAYS,
            "provider": self.provider.label(),
            "attribution": if self.provider == Provider::Census { "Source: U.S. Census Bureau" } else { "© OpenStreetMap contributors" },
            "license": if self.provider == Provider::Census { "U.S. government public data" } else { "ODbL-1.0" },
            "source": if self.provider == Provider::Census { census::SOURCE } else { "https://www.openstreetmap.org/copyright" },
            "boundsMeaning": "WGS84 [west,south,east,north] search extent; not a polygon or proof of imagery coverage.",
            "next": "Use an actual candidate's bounds in geod_scene_search. For an administrative polygon crop, read its boundarySource with geod_boundary_read before claiming only a rectangle exists. If the candidate instead has boundarySearch, use that real canonical-name query in geod_place_search to resolve its Census city boundary; do not invent aliases or administrative coordinates. Do not invent an extent when bounds is null. Resolve genuinely ambiguous matches with the human. If no match, try the established English or local spelling."})
    }
}
fn normalize(value: &Value, query: &Query) -> Result<Vec<Place>> {
    if value["type"] != "FeatureCollection" {
        return Err("Place service returned an invalid geographic document.".into());
    }
    let features = value["features"]
        .as_array()
        .filter(|v| v.len() <= 5)
        .ok_or("Place service returned an invalid candidate list.")?;
    let mut places = Vec::new();
    for feature in features {
        let p = &feature["properties"];
        let text = |key| p[key].as_str().filter(|v| clean(v, 200)).map(str::to_owned);
        let coordinates = &feature["geometry"]["coordinates"];
        let Some((name, kind, osm_type, osm_id, x, y)) = text("name")
            .zip(text("type"))
            .zip(text("osm_type"))
            .zip(p["osm_id"].as_u64())
            .zip(coordinates[0].as_f64())
            .zip(coordinates[1].as_f64())
            .map(|(((((name, kind), osm_type), osm_id), x), y)| {
                (name, kind, osm_type, osm_id, x, y)
            })
        else {
            continue;
        };
        if feature["type"] != "Feature"
            || feature["geometry"]["type"] != "Point"
            || coordinates.as_array().is_none_or(|v| v.len() != 2)
        {
            continue;
        }
        let bounds = if p["extent"].is_null() {
            None
        } else {
            let Ok(extent) = serde_json::from_value::<[f64; 4]>(p["extent"].clone()) else {
                continue;
            };
            // Photon extents use west,north,east,south, unlike STAC and GeoD.
            Some([extent[0], extent[3], extent[2], extent[1]])
        };
        let place = Place {
            name,
            kind,
            osm_type: Some(osm_type),
            osm_id: Some(osm_id),
            census_geoid: None,
            geometry: None,
            center: [x, y],
            bounds,
            country: text("country"),
            country_code: text("countrycode"),
            state: text("state"),
            county: text("county"),
        };
        if place.valid(query)
            && !places
                .iter()
                .any(|v: &Place| v.osm_type == place.osm_type && v.osm_id == place.osm_id)
        {
            places.push(place);
        }
    }
    Ok(places)
}
pub(super) async fn boundary(
    manager: &JobManager,
    lookup_id: &str,
    candidate_id: &str,
) -> Result<(String, crate::crop::PolygonGeometry, Value)> {
    let mut cache: Cache = read_record(&manager.inner.root, "places", lookup_id).await?;
    if cache.provider != Provider::Census
        || cache.query.cache_id() != lookup_id
        || !cache.fresh(&cache.query, Utc::now())
    {
        return Err("Read a fresh Census city candidate before reading its boundary.".into());
    }
    let place = cache
        .places
        .iter()
        .find(|p| p.census_geoid.as_deref() == Some(candidate_id))
        .ok_or("Use an actual city candidate identity.")?;
    if place.geometry.is_none() {
        // Upgrade old envelope-only caches from the same reviewed source. Never
        // reconstruct a polygon from the envelope or mix gazetteer identities.
        let document = fetch(
            census::url(&cache.query),
            &manager.proxy_settings().await,
            Provider::Census,
        )
        .await?;
        cache.places = census::normalize(&document, &cache.query)?;
        cache.checked_at = now();
        write_record(&manager.inner.root, "places", lookup_id, &cache).await?;
    }
    let place = cache
        .places
        .iter()
        .find(|p| p.census_geoid.as_deref() == Some(candidate_id))
        .ok_or("The source no longer contains that city candidate.")?;
    let geometry=place.geometry.clone().ok_or("City boundary exceeds the supported native polygon limits; a rectangle is not an exact replacement.")?;
    Ok((
        place.name.clone(),
        geometry,
        json!({"provider":Provider::Census.label(),"source":census::SOURCE,"censusGeoid":candidate_id,"checkedAt":cache.checked_at,"license":"U.S. government public data","attribution":"Source: U.S. Census Bureau","precision":"Original TIGERweb incorporated-place boundary, including source-defined water areas; not a shoreline-only or current legal boundary determination."}),
    ))
}
pub(super) fn latest_defaults(today: NaiveDate) -> Value {
    json!({"todayUtc": today.to_string(), "latestImagery": {
        "provider": "earth-search", "product": "Sentinel-2 L2A", "assetKey": "visual",
        "start": (today - chrono::Duration::days(29)).to_string(), "end": today.to_string(),
        "cloudMax": 100, "limit": 5, "sort": "acquisition date descending",
        "reason": "Latest optical satellite imagery when the human did not specify another source, product or date; cloud filtering must not silently replace latest with least cloudy.",
        "emptySearch": "Expand to 90 days, then 365 days if empty; report the actual acquisition date and cloud cover. Never claim complete area coverage from catalog overlap alone."
    }})
}
pub(super) async fn search(manager: &JobManager, query: Query) -> Result<Value> {
    query.validate()?;
    let cache_id = query.cache_id();
    // Serial, at most one request per second; no autocomplete or bulk lookup.
    // Recheck the persistent cache after waiting so simultaneous equal lookups
    // share one request, including across separate Agent conversations.
    let mut gate = REQUEST_GATE.get_or_init(|| Mutex::new(None)).lock().await;
    if let Ok(cache) = read_record::<Cache>(&manager.inner.root, "places", &cache_id).await {
        if cache.fresh(&query, Utc::now()) {
            return Ok(cache.result(true));
        }
    }
    let settings = manager.proxy_settings().await;
    let providers = query.providers();
    let mut last_error = "Place lookup is temporarily unreachable.".to_owned();
    for provider in providers {
        if let Some(last) = *gate {
            tokio::time::sleep_until(last + Duration::from_secs(1)).await;
        }
        *gate = Some(Instant::now());
        let url = match provider {
            Provider::Photon => query.url(),
            Provider::Census => census::url(&query),
        };
        let attempt = async {
            let document = fetch(url, &settings, provider).await?;
            match provider {
                Provider::Photon => normalize(&document, &query),
                Provider::Census => census::normalize(&document, &query),
            }
        }
        .await;
        match attempt {
            Ok(places) if !places.is_empty() || provider == Provider::Photon => {
                let cache = Cache {
                    provider,
                    places,
                    query,
                    checked_at: now(),
                };
                write_record(&manager.inner.root, "places", &cache_id, &cache).await?;
                return Ok(cache.result(false));
            }
            Ok(_) => {}
            Err(error) if error == "Place service denied the request." => return Err(error),
            Err(error) => last_error = error,
        }
    }
    Err(last_error)
}
async fn fetch(
    url: url::Url,
    settings: &crate::ProxySettings,
    provider: Provider,
) -> Result<Value> {
    let timeout = Duration::from_secs(if provider == Provider::Census { 12 } else { 25 });
    // Include DNS, connect, headers and every body chunk in the same deadline.
    // Both providers together remain below the 60-second native-tool IPC budget.
    tokio::time::timeout(timeout, fetch_inner(url, settings, timeout))
        .await
        .map_err(|_| "Place lookup timed out.")?
}
async fn fetch_inner(
    url: url::Url,
    settings: &crate::ProxySettings,
    timeout: Duration,
) -> Result<Value> {
    let client = crate::features::client_with_timeout(&url, settings, timeout)
        .await
        .map_err(|_| "Place lookup is temporarily unreachable.")?;
    let response = client.get(url)
        .header("Accept", "application/geo+json, application/json")
        .header("User-Agent", "GeoD-Global/0.1.0 (user-requested place lookup; https://github.com/gaopengbin/geod-global)")
        .send().await.map_err(|error| if error.is_timeout() { "Place lookup timed out." } else { "Place lookup is temporarily unreachable." })?;
    if !response.status().is_success() {
        return Err(
            if response.status().is_client_error() && response.status().as_u16() != 408 {
                "Place service denied the request."
            } else {
                "Place lookup is temporarily unreachable."
            }
            .into(),
        );
    }
    if response
        .content_length()
        .is_some_and(|len| len > MAX_RESPONSE as u64)
    {
        return Err("Place response exceeds 1 MiB.".into());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "Place response interrupted.")?;
        if bytes.len() + chunk.len() > MAX_RESPONSE {
            return Err("Place response exceeds 1 MiB.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| "Place response is invalid JSON.".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query() -> Query {
        Query {
            query: "New York".into(),
            kind: Kind::City,
            country_code: Some("US".into()),
        }
    }
    fn document() -> Value {
        json!({"type":"FeatureCollection","features":[{"type":"Feature","geometry":{"type":"Point","coordinates":[-74.006,40.712]},"properties":{"name":"New York","type":"city","countrycode":"US","country":"United States","state":"New York","osm_type":"R","osm_id":175905,"extent":[-74.258843,40.91763,-73.700233,40.476578]}}]})
    }
    #[test]
    fn city_extent_is_converted_not_invented_and_pois_are_excluded() {
        let mut value = document();
        let places = normalize(&value, &query()).unwrap();
        assert_eq!(
            places[0].bounds,
            Some([-74.258843, 40.476578, -73.700233, 40.91763])
        );
        value["features"][0]["properties"]["type"] = json!("house");
        assert!(normalize(&value, &query()).unwrap().is_empty());
        value["features"][0]["properties"]["type"] = json!("city");
        value["features"][0]["properties"]["extent"] = Value::Null;
        assert_eq!(normalize(&value, &query()).unwrap()[0].bounds, None);
    }
    #[test]
    fn rejects_invalid_extents_centers_and_country_mismatches() {
        for (field, bad) in [
            ("extent", json!([-74.0, 40.0, -75.0, 41.0])),
            ("countrycode", json!("GB")),
        ] {
            let mut value = document();
            value["features"][0]["properties"][field] = bad;
            assert!(normalize(&value, &query()).unwrap().is_empty());
        }
        let mut value = document();
        value["features"][0]["geometry"]["coordinates"] = json!([200, 40]);
        assert!(normalize(&value, &query()).unwrap().is_empty());
    }
    #[test]
    fn query_scope_and_cache_binding_are_validated() {
        let query = query();
        query.validate().unwrap();
        let url = query.url();
        assert_eq!(url.host_str(), Some("photon.komoot.io"));
        assert!(url.query_pairs().any(|(k, v)| k == "layer" && v == "city"));
        let today = Utc::now();
        let mut cache = Cache {
            provider: Provider::Photon,
            query: query.clone(),
            checked_at: today.to_rfc3339(),
            places: normalize(&document(), &query).unwrap(),
        };
        assert!(cache.fresh(&query, today));
        cache.checked_at = (today - chrono::Duration::days(7)).to_rfc3339();
        assert!(!cache.fresh(&query, today));
        cache.checked_at = (today + chrono::Duration::seconds(1)).to_rfc3339();
        assert!(!cache.fresh(&query, today));
        cache.checked_at = today.to_rfc3339();
        let other = Query {
            query: "York".into(),
            ..query.clone()
        };
        assert!(!cache.fresh(&other, today));
        for name in ["", "New York\n", "https://localhost/"] {
            assert!(Query {
                query: name.into(),
                ..query.clone()
            }
            .validate()
            .is_err());
        }
    }
    #[test]
    fn latest_interval_uses_native_clock_including_year_boundary() {
        let value = latest_defaults(NaiveDate::from_ymd_opt(2026, 1, 5).unwrap());
        assert_eq!(value["latestImagery"]["start"], "2025-12-07");
        assert_eq!(value["latestImagery"]["end"], "2026-01-05");
        assert_eq!(value["latestImagery"]["cloudMax"], 100);
    }
    #[tokio::test]
    #[ignore = "actual public Photon and Earth Search requests; no model or original-file download"]
    async fn live_city_lookup_cache_catalog_and_download_review() {
        let base = std::env::var_os("GEOD_PLACE_QA")
            .map(PathBuf::from)
            .expect("explicit isolated QA directory");
        let manager = JobManager::open(base.join("core")).await.unwrap();
        let first = search(&manager, query()).await.unwrap();
        let city = first["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .find(|place| place["name"] == "New York")
            .expect("actual city match");
        let bounds: [f64; 4] = serde_json::from_value(city["bounds"].clone()).unwrap();
        assert!(bounds[0] > -75.0 && bounds[2] < -73.0);
        assert!(bounds[1] > 40.0 && bounds[3] < 42.0);
        let cached = search(&manager, query()).await.unwrap();
        assert_eq!(cached["cached"], true);
        assert_eq!(cached["checkedAt"], first["checkedAt"]);
        manager.shutdown().await.unwrap();
        drop(manager);
        let manager = JobManager::open(base.join("core")).await.unwrap();
        assert_eq!(search(&manager, query()).await.unwrap()["cached"], true);
        let session = Uuid::new_v4().to_string();
        let today = Utc::now().date_naive();
        let mut scenes = Value::Null;
        for days in [30, 90, 365] {
            scenes = manager
                .agent_search(
                    &session,
                    SearchQuery {
                        provider: "earth-search".into(),
                        bounds,
                        start: (today - chrono::Duration::days(days - 1)).to_string(),
                        end: today.to_string(),
                        cloud_max: 100.0,
                        limit: 5,
                    },
                )
                .await
                .unwrap();
            if !scenes["scenes"].as_array().unwrap().is_empty() {
                break;
            }
        }
        let scene = scenes["scenes"]
            .as_array()
            .unwrap()
            .first()
            .expect("actual latest available scene");
        let plan = manager
            .agent_download_plan(
                &session,
                scenes["searchId"].as_str().unwrap(),
                vec![scene["itemId"].as_str().unwrap().into()],
                "visual",
            )
            .await
            .unwrap();
        assert_eq!(plan["status"], "pending");
        assert!(manager.list().await.is_empty());
        let receipt = json!({"schema":"geod-place-public-acceptance/v1","status":"passed",
            "lookup":first,"cacheSurvivesRestart":true,"catalog":scenes,"downloadReview":plan,
            "persistedJobCount":0,"usedUserDesktop":false,"modelCalls":0});
        tokio::fs::write(
            base.join("native-acceptance.json"),
            serde_json::to_vec_pretty(&receipt).unwrap(),
        )
        .await
        .unwrap();
        manager.shutdown().await.unwrap();
    }
}
