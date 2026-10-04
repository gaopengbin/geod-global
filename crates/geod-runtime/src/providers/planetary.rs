//! Read-only SAS access stays in one runtime's memory, separate from job persistence.
use super::{asset_url, matches_item, modis, radar, vegetation, LANDSAT_HOST, NAIP_HOST, PC_HOST};
use crate::Result;
use futures_util::StreamExt;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use url::Url;
use zeroize::Zeroizing;

const API: &str = "https://planetarycomputer.microsoft.com";
const CATALOG_TTL: Duration = Duration::from_secs(300);
const CATALOG_LIMIT: usize = 128;
const RATE_LIMITED: &str =
    "Planetary Computer is temporarily rate limiting access. Wait before retrying.";
const CATALOG_MISMATCH: &str =
    "The asset no longer matches the official Planetary Computer catalogue item";

struct Source {
    collection: &'static str,
    account: &'static str,
    container: &'static str,
    keys: &'static [(&'static str, &'static str)],
}
const SENTINEL: Source = Source {
    collection: "sentinel-2-l2a",
    account: "sentinel2l2a01",
    container: "sentinel2-l2",
    keys: &[("visual", "visual"), ("scl", "SCL")],
};
const LANDSAT: Source = Source {
    collection: "landsat-c2-l2",
    account: "landsateuwest",
    container: "landsat-c2",
    keys: &[
        ("red", "red"),
        ("green", "green"),
        ("blue", "blue"),
        ("qa_pixel", "qa_pixel"),
        ("qa_radsat", "qa_radsat"),
    ],
};
const NAIP: Source = Source {
    collection: "naip",
    account: "naipeuwest",
    container: "naip",
    keys: &[("aerial", "image")],
};
const RADAR: Source = Source {
    collection: "sentinel-1-rtc",
    account: "sentinel1euwestrtc",
    container: "sentinel1-grd-rtc",
    keys: &[("vv", "vv"), ("vh", "vh"), ("hh", "hh"), ("hv", "hv")],
};

const MODIS: Source = Source {
    collection: "modis-09A1-061",
    account: "modiseuwest",
    container: "modis-061-cogs",
    keys: &[
        ("red", "sur_refl_b01"),
        ("green", "sur_refl_b04"),
        ("blue", "sur_refl_b03"),
        ("modis_qc", "sur_refl_qc_500m"),
        ("modis_state", "sur_refl_state_500m"),
    ],
};

struct Token {
    query: Zeroizing<String>,
    expiry: i64,
}
const VEGETATION: Source = Source {
    collection: vegetation::COLLECTION,
    account: "modiseuwest",
    container: "modis-061-cogs",
    keys: vegetation::ASSETS,
};

impl Token {
    fn from_value(value: &Value) -> Result<Self> {
        let invalid = "Planetary Computer returned invalid read-only container access";
        let expiry = value
            .get("msft:expiry")
            .and_then(Value::as_str)
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .ok_or(invalid)?
            .timestamp();
        let raw = value.get("token").and_then(Value::as_str).ok_or(invalid)?;
        if raw.is_empty()
            || raw.len() > 8192
            || raw
                .chars()
                .any(|c| c.is_control() || matches!(c, '#' | '?'))
        {
            return Err(invalid.into());
        }
        let mut pairs = BTreeMap::new();
        for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
            if key.is_empty()
                || value.is_empty()
                || pairs.insert(key.into_owned(), value.into_owned()).is_some()
            {
                return Err(invalid.into());
            }
        }
        if !matches!(pairs.get("sp").map(String::as_str), Some("r" | "rl"))
            || pairs.get("sr").map(String::as_str) != Some("c")
            || !pairs.contains_key("sig")
        {
            return Err(invalid.into());
        }
        let sas_expiry = pairs
            .get("se")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .ok_or(invalid)?
            .timestamp();
        let expiry = expiry.min(sas_expiry);
        if expiry <= chrono::Utc::now().timestamp() + 60 {
            return Err("Planetary Computer returned expired data access".into());
        }
        // Re-encode parsed pairs so raw response text cannot inject a fragment or URL.
        let mut encoded = url::form_urlencoded::Serializer::new(String::new());
        encoded.extend_pairs(&pairs);
        Ok(Self {
            query: Zeroizing::new(encoded.finish()),
            expiry,
        })
    }
    fn usable(&self) -> bool {
        self.expiry > chrono::Utc::now().timestamp() + 60
    }
}

struct CatalogueItem {
    assets: BTreeMap<String, String>,
    checked: Instant,
}

impl CatalogueItem {
    fn from_value(value: Value, source: &Source, id: &str) -> Result<Self> {
        if value.get("id").and_then(Value::as_str) != Some(id)
            || value.get("collection").and_then(Value::as_str) != Some(source.collection)
        {
            return Err(CATALOG_MISMATCH.into());
        }
        let mut assets = BTreeMap::new();
        if source.collection == "naip" {
            let bands = value
                .pointer("/assets/image/eo:bands")
                .and_then(Value::as_array)
                .ok_or(CATALOG_MISMATCH)?;
            if bands.len() != 4
                || bands
                    .iter()
                    .zip(["red", "green", "blue", "nir"])
                    .any(|(band, name)| {
                        band.get("common_name").and_then(Value::as_str) != Some(name)
                    })
            {
                return Err(CATALOG_MISMATCH.into());
            }
        }
        for (key, catalogue_key) in source.keys {
            if let Some(href) = value
                .get("assets")
                .and_then(|a| a.get(catalogue_key))
                .and_then(|a| a.get("href"))
                .and_then(Value::as_str)
            {
                if source.collection == vegetation::COLLECTION {
                    vegetation::validate_catalogue(&value, id)?;
                    let asset = &value["assets"][catalogue_key];
                    let bands = asset["raster:bands"].as_array().ok_or(CATALOG_MISMATCH)?;
                    if bands.len() != 1 || !vegetation::validate_band(&bands[0], key) {
                        return Err(CATALOG_MISMATCH.into());
                    }
                }
                if source.collection == "modis-09A1-061" {
                    let asset = &value["assets"][catalogue_key];
                    let quality = modis::QUALITY_KEYS.contains(key);
                    let data_type = match *key {
                        "modis_qc" => "uint32",
                        "modis_state" => "uint16",
                        _ => "int16",
                    };
                    if asset
                        .pointer("/raster:bands/0/data_type")
                        .and_then(Value::as_str)
                        != Some(data_type)
                        || (!quality
                            && (asset
                                .pointer("/raster:bands/0/scale")
                                .and_then(Value::as_f64)
                                != Some(0.0001)
                                || asset
                                    .pointer("/eo:bands/0/common_name")
                                    .and_then(Value::as_str)
                                    != Some(*key)))
                        || (quality
                            && (asset.pointer("/raster:bands/0/scale").is_some()
                                || asset.pointer("/raster:bands/0/offset").is_some()
                                || asset.get("eo:bands").is_some()))
                        || asset
                            .pointer("/raster:bands/0/spatial_resolution")
                            .and_then(Value::as_f64)
                            != Some(500.0)
                    {
                        return Err(CATALOG_MISMATCH.into());
                    }
                }
                // Only reviewed assets from this exact product can enter the snapshot.
                if source.collection == "landsat-c2-l2" && matches!(*key, "qa_pixel" | "qa_radsat")
                {
                    let asset = &value["assets"][catalogue_key];
                    if asset
                        .pointer("/raster:bands/0/data_type")
                        .and_then(Value::as_str)
                        != Some("uint16")
                        || asset
                            .pointer("/raster:bands/0/spatial_resolution")
                            .and_then(Value::as_f64)
                            != Some(30.0)
                        || asset.pointer("/raster:bands/0/scale").is_some()
                        || asset.pointer("/raster:bands/0/offset").is_some()
                        || asset.get("eo:bands").is_some()
                        || (*key == "qa_pixel"
                            && asset
                                .pointer("/raster:bands/0/nodata")
                                .and_then(Value::as_u64)
                                != Some(1))
                        || (*key == "qa_radsat"
                            && asset
                                .pointer("/raster:bands/0/nodata")
                                .is_some_and(|v| v.as_u64() != Some(0)))
                        || asset
                            .get("raster:bands")
                            .and_then(Value::as_array)
                            .map(Vec::len)
                            != Some(1)
                    {
                        return Err(CATALOG_MISMATCH.into());
                    }
                }
                let url = asset_url(href)?;
                if source.collection == "sentinel-1-rtc"
                    && (value
                        .pointer("/properties/sar:instrument_mode")
                        .and_then(Value::as_str)
                        != Some("IW")
                        || value["assets"][catalogue_key]
                            .pointer("/raster:bands/0/data_type")
                            .and_then(Value::as_str)
                            != Some("float32")
                        || value["assets"][catalogue_key]
                            .pointer("/raster:bands/0/nodata")
                            .and_then(Value::as_f64)
                            != Some(-32768.0)
                        || value["assets"][catalogue_key]
                            .pointer("/raster:bands/0/spatial_resolution")
                            .and_then(Value::as_f64)
                            != Some(10.0)
                        || !value
                            .pointer("/properties/sar:polarizations")
                            .and_then(Value::as_array)
                            .is_some_and(|bands| {
                                bands
                                    .iter()
                                    .any(|band| band.as_str() == Some(key.to_uppercase().as_str()))
                            }))
                {
                    return Err(CATALOG_MISMATCH.into());
                }
                if !matches_item(&url, id, key) {
                    return Err(CATALOG_MISMATCH.into());
                }
                assets.insert((*key).into(), href.into());
            }
        }
        Ok(Self {
            assets,
            checked: Instant::now(),
        })
    }
}

#[derive(Default)]
struct ContainerState {
    token: Option<Token>,
    catalogue: BTreeMap<String, CatalogueItem>,
    limited_until: Option<Instant>,
    #[cfg(test)]
    requests: usize,
}
impl ContainerState {
    fn prune(&mut self) {
        self.catalogue
            .retain(|_, item| item.checked.elapsed() < CATALOG_TTL);
    }
    fn reserve_catalogue_slot(&mut self) {
        if self.catalogue.len() >= CATALOG_LIMIT {
            if let Some(oldest) = self
                .catalogue
                .iter()
                .min_by_key(|(_, v)| v.checked)
                .map(|(k, _)| k.clone())
            {
                self.catalogue.remove(&oldest);
            }
        }
    }
    async fn json(
        &mut self,
        client: &reqwest::Client,
        url: Url,
        max_bytes: usize,
    ) -> Result<Value> {
        #[cfg(test)]
        {
            self.requests += 1;
        }
        let response = client
            .get(url)
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .map_err(|_| "Planetary Computer request failed")?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            // Queued workers share the cooldown; do not amplify an upstream 429.
            let seconds = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|s| s.to_str().ok())
                .and_then(|s| {
                    s.parse::<u64>().ok().or_else(|| {
                        chrono::DateTime::parse_from_rfc2822(s).ok().map(|date| {
                            (date.timestamp() - chrono::Utc::now().timestamp()).max(1) as u64
                        })
                    })
                })
                .unwrap_or(30)
                .clamp(1, 300);
            self.limited_until = Some(Instant::now() + Duration::from_secs(seconds));
            return Err(RATE_LIMITED.into());
        }
        if !response.status().is_success() {
            return Err(format!(
                "Planetary Computer returned HTTP {}",
                response.status().as_u16()
            ));
        }
        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "Planetary Computer response could not be read")?;
            if body.len() + chunk.len() > max_bytes {
                return Err("Planetary Computer response exceeded the metadata limit".into());
            }
            body.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&body)
            .map_err(|_| "Planetary Computer returned invalid metadata".into())
    }
}

#[derive(Default)]
pub(crate) struct AccessCache {
    sentinel: Mutex<ContainerState>,
    landsat: Mutex<ContainerState>,
    naip: Mutex<ContainerState>,
    modis: Mutex<ContainerState>,
    radar: Mutex<ContainerState>,
    #[cfg(test)]
    api: Option<String>,
}

impl AccessCache {
    pub(crate) async fn resolve(
        &self,
        client: &reqwest::Client,
        href: &str,
        id: &str,
        key: &str,
    ) -> Result<Url> {
        let original = Url::parse(href).map_err(|_| "Invalid asset URL")?;
        // Other providers and constrained local transfer fixtures do not use Azure SAS.
        let (source, state) = match original.host_str() {
            Some(PC_HOST) => (&SENTINEL, &self.sentinel),
            Some(LANDSAT_HOST) => (&LANDSAT, &self.landsat),
            Some(NAIP_HOST) => (&NAIP, &self.naip),
            Some(modis::HOST) => (
                if vegetation::asset_path(original.path()) {
                    &VEGETATION
                } else {
                    &MODIS
                },
                &self.modis,
            ),
            Some(radar::HOST) => (&RADAR, &self.radar),
            _ => return Ok(original),
        };
        let original = asset_url(href)?;
        if !matches_item(&original, id, key) || !source.keys.iter().any(|(k, _)| *k == key) {
            return Err(CATALOG_MISMATCH.into());
        }
        let api = API;
        #[cfg(test)]
        let api = self.api.as_deref().unwrap_or(api);
        // One lock per reviewed container coalesces concurrent catalogue/token requests.
        // Dropping a cancelled worker releases it; no detached request owns a credential.
        let mut state = state.lock().await;
        if state
            .limited_until
            .is_some_and(|until| until > Instant::now())
        {
            return Err(RATE_LIMITED.into());
        }
        state.limited_until = None;
        state.prune();
        if !state.catalogue.contains_key(id) {
            let mut item_url = Url::parse(&format!(
                "{api}/api/stac/v1/collections/{}/items/",
                source.collection
            ))
            .map_err(|_| "Invalid catalogue item URL")?;
            item_url
                .path_segments_mut()
                .map_err(|_| "Invalid catalogue item URL")?
                .pop_if_empty()
                .push(id);
            let value = state.json(client, item_url, 2 * 1024 * 1024).await?;
            let item = CatalogueItem::from_value(value, source, id)?;
            state.reserve_catalogue_slot();
            state.catalogue.insert(id.into(), item);
        }
        if state
            .catalogue
            .get(id)
            .and_then(|i| i.assets.get(key))
            .map(String::as_str)
            != Some(href)
        {
            return Err(CATALOG_MISMATCH.into());
        }
        if state.token.as_ref().is_none_or(|t| !t.usable()) {
            state.token = None;
            let token_url = Url::parse(&format!(
                "{api}/api/sas/v1/token/{}/{}",
                source.account, source.container
            ))
            .map_err(|_| "Invalid signing endpoint")?;
            let value = state.json(client, token_url, 16 * 1024).await?;
            state.token = Some(Token::from_value(&value)?);
        }
        let mut signed = original;
        signed.set_query(Some(
            &state
                .token
                .as_ref()
                .ok_or("Missing read-only data access")?
                .query,
        ));
        Ok(signed)
    }
}

#[cfg(test)]
mod tests;
