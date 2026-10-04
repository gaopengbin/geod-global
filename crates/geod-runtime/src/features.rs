//! Public OGC API Features connections and complete, bounded bbox snapshots.
//! Server feature geometry is retained, never implicitly clipped to the AOI.
use crate::{io_error, now, proxy, vector, JobManager, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    net::IpAddr,
    path::Path,
    time::Duration,
};
use url::Url;
use uuid::Uuid;
pub mod arcgis;
pub mod overpass;
#[cfg(test)]
mod tests;
pub mod wfs;

const MAX_METADATA: usize = 8 * 1024 * 1024;
const MAX_PAGES: usize = 25;
const PAGE_SIZE: usize = 200;
const FILE: &str = "feature-services.json";
const CRS84: &str = "http://www.opengis.net/def/crs/OGC/1.3/CRS84";

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Collection {
    pub id: String,
    pub title: String,
    pub description: String,
    pub items_url: String,
    pub license_links: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arcgis: Option<arcgis::Layer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wfs: Option<wfs::Layer>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FeatureService {
    pub id: String,
    pub name: String,
    pub url: String,
    pub title: String,
    pub collections: Vec<Collection>,
    pub connected_at: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arcgis: Option<arcgis::Service>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub overpass: Option<overpass::Service>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wfs: Option<wfs::Service>,
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ConnectRequest {
    pub name: String,
    pub url: String,
    #[serde(default = "default_protocol")]
    pub protocol: String,
}
fn default_protocol() -> String {
    "OGC".into()
}
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryRequest {
    pub service_id: String,
    pub collection_id: String,
    pub bounds: [f64; 4],
    pub area_geometry: Option<crate::crop::PolygonGeometry>,
    pub page_size: Option<usize>,
    pub response_format: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PageReceipt {
    pub url: String,
    pub sha256: String,
    pub bytes: usize,
    pub returned: usize,
    /// Form parameters identify a read-only POST; absent for legacy OGC GETs.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parameters: Option<BTreeMap<String, String>>,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Provenance {
    pub service_url: String,
    pub service_name: String,
    pub collection_id: String,
    pub collection_title: String,
    pub license_links: Vec<String>,
    pub requested_bounds: [f64; 4],
    pub area_geometry: Option<crate::crop::PolygonGeometry>,
    pub requested_at: String,
    pub pages: Vec<PageReceipt>,
    pub number_matched: Option<usize>,
    pub feature_count: usize,
    pub selection: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub arcgis: Option<arcgis::Snapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub wfs: Option<wfs::Snapshot>,
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn clean(text: &str, max: usize) -> bool {
    !text.is_empty()
        && text.trim() == text
        && text.chars().count() <= max
        && !text.chars().any(char::is_control)
}
pub(crate) fn bounds(b: [f64; 4]) -> Result<()> {
    if !b.iter().all(|n| n.is_finite())
        || b[0] < -180.0
        || b[2] > 180.0
        || b[1] < -90.0
        || b[3] > 90.0
        || b[0] >= b[2]
        || b[1] >= b[3]
    {
        return Err("Choose a non-empty WGS84 region that does not cross the antimeridian".into());
    }
    Ok(())
}
fn public_ip(ip: IpAddr) -> bool {
    match ip {
        IpAddr::V4(v) => {
            let o = v.octets();
            !v.is_private()
                && !v.is_loopback()
                && !v.is_link_local()
                && !v.is_broadcast()
                && !v.is_unspecified()
                && !v.is_documentation()
                && o[0] != 0
                && o[0] < 224
                && !(o[0] == 100 && (64..=127).contains(&o[1]))
                && !(o[0] == 192 && o[1] == 0 && o[2] == 0)
                && !(o[0] == 198 && (o[1] == 18 || o[1] == 19))
        }
        IpAddr::V6(v) => {
            let s = v.segments();
            (s[0] & 0xe000) == 0x2000
                && !(s[0] == 0x2001 && s[1] == 0x0db8)
                && !(s[0] == 0x2001 && s[1] < 0x200) // special-use transition/benchmark ranges
        }
    }
}
pub(crate) fn public_url(raw: &str) -> Result<Url> {
    let u = Url::parse(raw).map_err(|_| "Invalid data service URL")?;
    if raw.len() > 2048
        || u.scheme() != "https"
        || u.port_or_known_default() != Some(443)
        || !u.username().is_empty()
        || u.password().is_some()
        || u.fragment().is_some()
        || u.host_str().is_none_or(|h| {
            h == "localhost"
                || !h.contains('.')
                || h.ends_with(".local")
                || h.ends_with(".localhost")
        })
        || u.host_str()
            .and_then(|h| h.parse::<IpAddr>().ok())
            .is_some_and(|ip| !public_ip(ip))
    {
        return Err("Use a public HTTPS data service without credentials or fragments".into());
    }
    Ok(u)
}
fn scoped(root: &Url, raw: &str) -> Result<Url> {
    let u = public_url(root.join(raw).map_err(io_error)?.as_str())?;
    let prefix = root.path().trim_end_matches('/');
    if u.origin() != root.origin()
        || !(u.path() == prefix || u.path().starts_with(&format!("{prefix}/")))
    {
        return Err("Data service links must stay within the connected service".into());
    }
    Ok(u)
}
fn metadata_url(root: &Url, raw: &str) -> Result<Url> {
    let mut u = scoped(root, raw)?;
    if u.query_pairs().any(|(k, v)| k != "f" || v != "json") {
        return Err("Unsupported data service metadata parameters".into());
    }
    u.query_pairs_mut().clear().append_pair("f", "json");
    Ok(u)
}
fn links(v: &Value, rel: &str, media: &str) -> Result<Option<String>> {
    let Some(all) = v.get("links") else {
        return Ok(None);
    };
    let all = all
        .as_array()
        .filter(|a| a.len() <= 256)
        .ok_or("Invalid data service links")?;
    let found: Vec<_> = all
        .iter()
        .filter(|l| {
            l["rel"] == rel
                && l["type"]
                    .as_str()
                    .is_some_and(|t| t.split(';').next() == Some(media))
        })
        .collect();
    if found.len() > 1 {
        return Err("Ambiguous data service links".into());
    }
    found
        .first()
        .map(|v| {
            v["href"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| "Data service link has no address".into())
        })
        .transpose()
}
pub(crate) async fn client(root: &Url, settings: &crate::ProxySettings) -> Result<reqwest::Client> {
    client_with_timeout(root, settings, Duration::from_secs(30)).await
}

pub(crate) async fn client_with_timeout(
    root: &Url,
    settings: &crate::ProxySettings,
    timeout: Duration,
) -> Result<reqwest::Client> {
    let host = root.host_str().ok_or("Data service has no host")?;
    let addresses: Vec<_> = tokio::net::lookup_host((host, 443))
        .await
        .map_err(|_| "Cannot resolve data service host")?
        .collect();
    if addresses.is_empty() || addresses.iter().any(|a| !public_ip(a.ip())) {
        return Err("Data service DNS must resolve only to public addresses".into());
    }
    // Pin direct DNS answers. An explicitly selected system/custom proxy may
    // resolve the remote host itself; that route remains the user's proxy policy.
    proxy::download_builder(settings)?
        .resolve_to_addrs(host, &addresses)
        .timeout(timeout)
        .build()
        .map_err(io_error)
}
async fn fetch(
    client: &reqwest::Client,
    url: &Url,
    budget: &mut usize,
) -> Result<(Value, PageReceipt)> {
    fetch_request(client, url, None, budget).await
}
async fn fetch_request(
    client: &reqwest::Client,
    url: &Url,
    parameters: Option<&BTreeMap<String, String>>,
    budget: &mut usize,
) -> Result<(Value, PageReceipt)> {
    let request = match parameters {
        Some(parameters) => client.post(url.clone()).form(parameters),
        None => client.get(url.clone()),
    };
    let response = request
        .header("Accept", "application/geo+json, application/json")
        .send()
        .await
        .map_err(|_| "Cannot reach data service; check the service and proxy settings")?;
    if !response.status().is_success() {
        return Err(format!(
            "Data service returned HTTP {}",
            response.status().as_u16()
        ));
    }
    let media = response
        .headers()
        .get("content-type")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("");
    if !["application/json", "application/geo+json"].contains(&media) {
        return Err("Data service did not return JSON or GeoJSON".into());
    }
    if response.headers().get("content-crs").is_some_and(|h| {
        h.to_str()
            .map_or(true, |s| s.trim_matches(['<', '>']) != CRS84)
    }) {
        return Err("Data service returned an unsupported coordinate system".into());
    }
    if response
        .content_length()
        .is_some_and(|n| n > *budget as u64)
    {
        return Err("Data service response exceeds the extraction limit".into());
    }
    let mut bytes = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "Data service response was interrupted")?;
        if chunk.len() > *budget {
            return Err("Data service response exceeds the extraction limit".into());
        }
        *budget -= chunk.len();
        bytes.extend_from_slice(&chunk);
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Data service returned invalid JSON")?;
    let receipt = PageReceipt {
        url: url.to_string(),
        sha256: hash(&bytes),
        bytes: bytes.len(),
        returned: 0,
        parameters: parameters.cloned(),
    };
    Ok((value, receipt))
}
fn collection(root: &Url, v: &Value) -> Result<Option<Collection>> {
    if v.get("itemType").is_some_and(|t| t != "feature") {
        return Ok(None);
    }
    let Some(items) = links(v, "items", "application/geo+json")? else {
        return Ok(None);
    };
    if v.get("storageCrs").is_some_and(|t| !t.is_string()) {
        return Err("Invalid collection CRS metadata".into());
    }
    let id = v["id"]
        .as_str()
        .filter(|s| clean(s, 160))
        .ok_or("Collection has no valid identifier")?;
    let title = v["title"].as_str().unwrap_or(id);
    if !clean(title, 240) {
        return Err("Collection has an invalid title".into());
    }
    let description = v["description"].as_str().unwrap_or("");
    if description.len() > 16_384 {
        return Err("Collection description is too long".into());
    }
    let license_links = license_links(root, v)?;
    Ok(Some(Collection {
        id: id.into(),
        title: title.into(),
        description: description.into(),
        items_url: metadata_url(root, &items)?.to_string(),
        license_links,
        arcgis: None,
        wfs: None,
    }))
}
fn license_links(root: &Url, v: &Value) -> Result<Vec<String>> {
    let mut result = Vec::new();
    if let Some(a) = v["links"].as_array() {
        for l in a.iter().filter(|l| l["rel"] == "license") {
            let href = l["href"].as_str().ok_or("License link has no address")?;
            let u = root.join(href).map_err(io_error)?;
            if !["https", "http"].contains(&u.scheme())
                || !u.username().is_empty()
                || u.password().is_some()
                || href.len() > 2048
            {
                return Err("Invalid declared license link".into());
            }
            result.push(u.to_string());
        }
    }
    if result.len() > 16 {
        return Err("Too many license links".into());
    }
    Ok(result)
}
fn next_url(root: &Url, current: &Url, raw: &str, bbox: Option<[f64; 4]>) -> Result<Url> {
    let u = scoped(root, current.join(raw).map_err(io_error)?.as_str())?;
    if u.path() != current.path() {
        return Err("Data service pagination changed the resource".into());
    }
    let pairs: Vec<_> = u.query_pairs().collect();
    let mut keys = BTreeSet::new();
    for (k, v) in &pairs {
        if !keys.insert(k.to_string())
            || ![
                "f",
                "bbox",
                "limit",
                "offset",
                "startindex",
                "page",
                "cursor",
            ]
            .contains(&k.as_ref())
            || v.len() > 1024
        {
            return Err("Unsupported data service pagination parameters".into());
        }
        if k == "f" && v != "json" {
            return Err("Data service pagination changed the encoding".into());
        }
    }
    if let Some(expected) = bbox {
        let raw = pairs
            .iter()
            .find(|(k, _)| k == "bbox")
            .map(|(_, v)| v.as_ref())
            .ok_or("Data service pagination lost the query region")?;
        let actual: Vec<f64> = raw
            .split(',')
            .map(str::parse)
            .collect::<std::result::Result<_, _>>()
            .map_err(|_| "Invalid pagination region")?;
        if actual.as_slice() != expected {
            return Err("Data service pagination changed the query region".into());
        }
    } else if pairs.iter().any(|(k, _)| k == "bbox") {
        return Err("Collection pagination added a region filter".into());
    }
    Ok(u)
}
async fn discover(
    request: ConnectRequest,
    settings: crate::ProxySettings,
) -> Result<FeatureService> {
    if request.protocol == "WFS2" {
        return wfs::discover(request, settings).await;
    }
    if request.protocol == "ArcGIS" {
        return arcgis::discover(request, settings).await;
    }
    if request.protocol == "Overpass" {
        return overpass::discover(request, settings).await;
    }
    if request.protocol != "OGC" {
        return Err("Unsupported feature service protocol".into());
    }
    let name = request.name.trim();
    if !clean(name, 80) {
        return Err("Enter a data service name of 1–80 characters".into());
    }
    let root = public_url(request.url.trim())?;
    if root.query().is_some() {
        return Err("Enter the service landing URL without query parameters".into());
    }
    let c = client(&root, &settings).await?;
    let mut budget = MAX_METADATA;
    let (landing, _) = fetch(&c, &metadata_url(&root, root.as_str())?, &mut budget).await?;
    let conformance = links(&landing, "conformance", "application/json")?
        .ok_or("Service landing page has no JSON conformance link")?;
    let data = links(&landing, "data", "application/json")?
        .ok_or("Service landing page has no JSON collections link")?;
    let (conformance, _) = fetch(&c, &metadata_url(&root, &conformance)?, &mut budget).await?;
    let classes = conformance["conformsTo"]
        .as_array()
        .ok_or("Service has no conformance declaration")?;
    for class in ["core", "geojson"] {
        if !classes.iter().any(|c| {
            c == &format!("http://www.opengis.net/spec/ogcapi-features-1/1.0/conf/{class}")
        }) {
            return Err("Service must declare OGC API Features Core and GeoJSON support".into());
        }
    }
    let mut url = metadata_url(&root, &data)?;
    let mut seen = BTreeSet::new();
    let mut collections = BTreeMap::new();
    loop {
        if seen.len() >= MAX_PAGES || !seen.insert(url.to_string()) {
            return Err("Collection pagination is incomplete or repeated".into());
        }
        let (page, _) = fetch(&c, &url, &mut budget).await?;
        let entries = page["collections"]
            .as_array()
            .ok_or("Service returned no collection list")?;
        for value in entries {
            if let Some(item) = collection(&root, value)? {
                if collections.insert(item.id.clone(), item).is_some() || collections.len() > 512 {
                    return Err(
                        "Collection identifiers repeat or exceed the connection limit".into(),
                    );
                }
            }
        }
        match links(&page, "next", "application/json")? {
            Some(raw) => url = next_url(&root, &url, &raw, None)?,
            None => break,
        }
    }
    let title = landing["title"]
        .as_str()
        .filter(|s| clean(s, 240))
        .unwrap_or(name);
    Ok(FeatureService {
        id: Uuid::new_v4().to_string(),
        name: name.into(),
        url: root.to_string(),
        title: title.into(),
        collections: collections.into_values().collect(),
        connected_at: now(),
        arcgis: None,
        wfs: None,
        overpass: None,
    })
}

#[derive(Default)]
struct Assembly {
    features: Vec<Value>,
    ids: BTreeMap<String, Value>,
    matched: Option<usize>,
    pages: Vec<PageReceipt>,
}
impl Assembly {
    fn append(&mut self, page: Value, mut receipt: PageReceipt) -> Result<Option<String>> {
        if page["type"] != "FeatureCollection" || page.get("crs").is_some() {
            return Err("Data service must return WGS84 GeoJSON features".into());
        }
        let features = page["features"]
            .as_array()
            .ok_or("Data service returned no features array")?;
        if page
            .get("numberReturned")
            .is_some_and(|n| n.as_u64() != Some(features.len() as u64))
        {
            return Err("Data service returned an inconsistent feature count".into());
        }
        if let Some(n) = page.get("numberMatched") {
            if n != "unknown" {
                let n = n
                    .as_u64()
                    .filter(|n| *n <= vector::MAX_FEATURES as u64)
                    .ok_or("Query exceeds 50,000 features or has an invalid count")?
                    as usize;
                if self.matched.is_some_and(|old| old != n) {
                    return Err("Data service changed during pagination; query again".into());
                }
                self.matched = Some(n);
            }
        }
        for feature in features {
            if let Some(id) = feature.get("id") {
                let key = serde_json::to_string(id).map_err(io_error)?;
                if let Some(old) = self.ids.get(&key) {
                    if old != feature {
                        return Err(
                            "Data service returned conflicting features with the same identifier"
                                .into(),
                        );
                    }
                    continue;
                }
                self.ids.insert(key, feature.clone());
            }
            self.features.push(feature.clone());
            if self.features.len() > vector::MAX_FEATURES {
                return Err("Query exceeds 50,000 features".into());
            }
        }
        receipt.returned = features.len();
        self.pages.push(receipt);
        links(&page, "next", "application/geo+json")
    }
    fn complete(&self) -> Result<()> {
        if self.matched.is_some_and(|n| n != self.features.len()) {
            return Err(
                "Data service pagination ended before all matching features were received".into(),
            );
        }
        Ok(())
    }
}
impl Provenance {
    pub(crate) fn validate(&self, count: usize) -> Result<()> {
        if self.wfs.is_some() {
            return wfs::validate_provenance(self, count);
        }
        if self.arcgis.is_some() {
            return arcgis::validate_provenance(self, count);
        }
        bounds(self.requested_bounds)?;
        let root = public_url(&self.service_url)?;
        if root.query().is_some()
            || !clean(&self.service_name, 80)
            || !clean(&self.collection_id, 160)
            || !clean(&self.collection_title, 240)
            || self.selection != "bbox-full-features"
            || self.feature_count != count
            || self.number_matched.is_some_and(|n| n != count)
            || self.pages.is_empty()
            || self.pages.len() > MAX_PAGES
            || chrono::DateTime::parse_from_rfc3339(&self.requested_at).is_err()
        {
            return Err("Invalid feature query provenance".into());
        }
        if let Some(area) = &self.area_geometry {
            validate_area(area, self.requested_bounds)?;
        }
        let mut total = 0usize;
        let mut urls = BTreeSet::new();
        let mut returned = 0usize;
        for p in &self.pages {
            let u = scoped(&root, &p.url)?;
            next_url(&root, &u, &p.url, Some(self.requested_bounds))?;
            if !urls.insert(&p.url)
                || p.parameters.is_some()
                || p.bytes == 0
                || p.bytes > vector::MAX_BYTES
                || p.returned > vector::MAX_FEATURES
                || p.sha256.len() != 64
                || !p
                    .sha256
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            {
                return Err("Invalid feature page receipt".into());
            }
            total += p.bytes;
            returned += p.returned;
        }
        if total > vector::MAX_BYTES || returned < count {
            return Err("Invalid feature receipt totals".into());
        }
        if license_links(
            &root,
            &json!({"links":self.license_links.iter().map(|u|json!({"rel":"license","href":u})).collect::<Vec<_>>()}),
        )? != self.license_links
        {
            return Err("Invalid feature license links".into());
        }
        Ok(())
    }
}
fn validate_area(area: &crate::crop::PolygonGeometry, b: [f64; 4]) -> Result<()> {
    let a = area.bounds()?;
    if a[0] < b[0] || a[1] < b[1] || a[2] > b[2] || a[3] > b[3] {
        return Err("Query bounds must include the selected area".into());
    }
    Ok(())
}
pub(crate) async fn load(root: &Path) -> Result<BTreeMap<String, FeatureService>> {
    let bytes = match tokio::fs::read(root.join(FILE)).await {
        Ok(b) => b,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(BTreeMap::new()),
        Err(e) => return Err(io_error(e)),
    };
    if bytes.len() > MAX_METADATA {
        return Err("Data service registry is too large".into());
    }
    let records: BTreeMap<String, FeatureService> =
        serde_json::from_slice(&bytes).map_err(io_error)?;
    if records.len() > 24 {
        return Err("Too many saved data services".into());
    }
    for (id, s) in &records {
        let root = public_url(&s.url)?;
        if Uuid::parse_str(id).ok().map(|u| u.to_string()).as_ref() != Some(id)
            || s.id != *id
            || root.query().is_some()
            || !clean(&s.name, 80)
            || !clean(&s.title, 240)
            || s.collections.len() > 512
            || chrono::DateTime::parse_from_rfc3339(&s.connected_at).is_err()
        {
            return Err("Invalid saved data service".into());
        }
        if s.wfs.is_some() {
            wfs::validate_service(s)?;
            continue;
        }
        if s.overpass.is_some() {
            overpass::validate_service(s)?;
            continue;
        }
        if s.arcgis.is_some() {
            arcgis::validate_service(s)?;
            continue;
        }
        let mut ids = BTreeSet::new();
        for c in &s.collections {
            if !ids.insert(&c.id)
                || !clean(&c.id, 160)
                || !clean(&c.title, 240)
                || c.description.len() > 16_384
                || c.arcgis.is_some()
                || c.wfs.is_some()
                || metadata_url(&root, &c.items_url)?.as_str() != c.items_url
            {
                return Err("Invalid saved feature collection".into());
            }
            license_links(
                &root,
                &json!({"links":c.license_links.iter().map(|u|json!({"rel":"license","href":u})).collect::<Vec<_>>()}),
            )?;
        }
    }
    Ok(records)
}
impl JobManager {
    pub async fn list_feature_services(&self) -> Vec<FeatureService> {
        self.inner
            .feature_services
            .lock()
            .await
            .values()
            .cloned()
            .collect()
    }
    async fn persist_feature_services(
        &self,
        records: &BTreeMap<String, FeatureService>,
    ) -> Result<()> {
        use tokio::io::AsyncWriteExt;
        let bytes = serde_json::to_vec_pretty(records).map_err(io_error)?;
        if bytes.len() > MAX_METADATA {
            return Err("Data service registry is too large".into());
        }
        let pending = self.inner.root.join(format!("{FILE}.tmp"));
        let mut file = tokio::fs::File::create(&pending).await.map_err(io_error)?;
        file.write_all(&bytes).await.map_err(io_error)?;
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        tokio::fs::rename(pending, self.inner.root.join(FILE))
            .await
            .map_err(io_error)
    }
    pub async fn connect_feature_service(&self, request: ConnectRequest) -> Result<FeatureService> {
        self.inner.store.lock().await.accepting_jobs()?;
        let service = tokio::time::timeout(
            Duration::from_secs(90),
            discover(request, self.proxy_settings().await),
        )
        .await
        .map_err(|_| "Data service discovery timed out")??;
        let mut records = self.inner.feature_services.lock().await;
        let old = records.values().find(|s| s.url == service.url).cloned();
        let mut service = service;
        if let Some(old) = &old {
            service.id = old.id.clone();
        } else if records.len() >= 24 {
            return Err("Data service registry is full (24 connections)".into());
        }
        records.insert(service.id.clone(), service.clone());
        if let Err(e) = self.persist_feature_services(&records).await {
            records.remove(&service.id);
            if let Some(old) = old {
                records.insert(old.id.clone(), old);
            }
            return Err(e);
        }
        Ok(service)
    }
    pub async fn forget_feature_service(&self, id: &str) -> Result<()> {
        self.inner.store.lock().await.accepting_jobs()?;
        let mut records = self.inner.feature_services.lock().await;
        let old = records.remove(id).ok_or("Unknown data service")?;
        if let Err(e) = self.persist_feature_services(&records).await {
            records.insert(id.into(), old);
            return Err(e);
        }
        Ok(())
    }
    pub async fn query_features(&self, request: QueryRequest) -> Result<vector::VectorAsset> {
        self.inner.store.lock().await.accepting_jobs()?;
        bounds(request.bounds)?;
        let page_size = request.page_size.unwrap_or(PAGE_SIZE);
        if !(1..=PAGE_SIZE).contains(&page_size) {
            return Err("Feature page size must be between 1 and 200".into());
        }
        if let Some(area) = &request.area_geometry {
            validate_area(area, request.bounds)?;
        }
        let service = self
            .inner
            .feature_services
            .lock()
            .await
            .get(&request.service_id)
            .cloned()
            .ok_or("Unknown data service")?;
        let collection = service
            .collections
            .iter()
            .find(|c| c.id == request.collection_id)
            .cloned()
            .ok_or("Unknown feature collection")?;
        if service.wfs.is_some() {
            let (bytes, source) = tokio::time::timeout(
                Duration::from_secs(180),
                wfs::query(service, collection, request, self.proxy_settings().await),
            )
            .await
            .map_err(|_| "WFS extraction timed out; no file was registered")??;
            return self.import_wfs_source(bytes, source).await;
        }
        if request.response_format.is_some() {
            return Err("Response format selection requires a WFS connection".into());
        }
        if service.overpass.is_some() {
            let (bytes, source) = tokio::time::timeout(
                Duration::from_secs(45),
                overpass::query(service, request, self.proxy_settings().await),
            )
            .await
            .map_err(|_| "OSM extraction timed out; no file was registered")??;
            return self.import_osm_source(bytes, source).await;
        }
        if service.arcgis.is_some() {
            let (data, source, title) = tokio::time::timeout(
                Duration::from_secs(120),
                arcgis::query(service, collection, request, self.proxy_settings().await),
            )
            .await
            .map_err(|_| "Feature extraction timed out; no file was registered")??;
            return self
                .import_vector_source(
                    vector::ImportVectorRequest {
                        name: format!("{} · ArcGIS", title.chars().take(108).collect::<String>()),
                        text: serde_json::to_string(&data).map_err(io_error)?,
                    },
                    Some(source),
                )
                .await;
        }
        let settings = self.proxy_settings().await;
        let snapshot=tokio::time::timeout(Duration::from_secs(120),async {
            let root=public_url(&service.url)?;let c=client(&root,&settings).await?;
            let mut url=metadata_url(&root,&collection.items_url)?;
            let bbox=request.bounds.iter().map(f64::to_string).collect::<Vec<_>>().join(",");
            url.query_pairs_mut().append_pair("bbox",&bbox).append_pair("limit",&page_size.to_string());
            let mut assembled=Assembly::default();let mut seen=BTreeSet::new();let mut budget=vector::MAX_BYTES;let at=now();
            loop {
                if seen.len()>=MAX_PAGES || !seen.insert(url.to_string()) { return Err("Feature pagination is incomplete or repeated; use a smaller region".into()); }
                let (page,receipt)=fetch(&c,&url,&mut budget).await?;
                match assembled.append(page,receipt)? {Some(raw)=>url=next_url(&root,&url,&raw,Some(request.bounds))?,None=>break}
            }
            assembled.complete()?;
            let source=Provenance{service_url:service.url,service_name:service.name,collection_id:collection.id,collection_title:collection.title.clone(),license_links:collection.license_links,requested_bounds:request.bounds,area_geometry:request.area_geometry,requested_at:at,pages:assembled.pages,number_matched:assembled.matched,feature_count:assembled.features.len(),selection:"bbox-full-features".into(),arcgis:None,wfs:None};
            source.validate(assembled.features.len())?;
            let data=json!({"type":"FeatureCollection","features":assembled.features,"geodSource":source});
            Ok::<_,String>((data,source,collection.title))
        }).await.map_err(|_|"Feature extraction timed out; no file was registered")??;
        // Persistence is outside the network deadline: it must finish or roll back.
        let (data, source, title) = snapshot;
        self.import_vector_source(
            vector::ImportVectorRequest {
                name: format!("{} · OGC", title.chars().take(110).collect::<String>()),
                text: serde_json::to_string(&data).map_err(io_error)?,
            },
            Some(source),
        )
        .await
    }
}
