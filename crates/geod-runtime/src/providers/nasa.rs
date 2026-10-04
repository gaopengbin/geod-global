//! HLS L30, SRTMGL1 and VIIRS originals. EDL and CDN signatures stay native.
use crate::{accounts::bearer, AccountProvider, Job, JobManager, Result};
use futures_util::StreamExt;
use reqwest::{header, Client, Response, StatusCode};
use serde_json::Value;
use std::time::Duration;
use url::Url;

pub const HOST: &str = "data.lpdaac.earthdatacloud.nasa.gov";
const CDN: &str = "d1nklfio7vscoe.cloudfront.net";
const PREFIX: &str = "/lp-prod-protected/HLSL30.020/";
const ACCESS_ERROR: &str = "NASA file access was rejected. Verify Earthdata authorization and approve the LP DAAC application in your Earthdata account before retrying.";

pub fn valid_item(id: &str) -> bool {
    if !id.is_ascii() {
        return false;
    }
    let parts: Vec<_> = id.split('.').collect();
    parts.len() == 6
        && parts[0] == "HLS"
        && parts[1] == "L30"
        && parts[2].len() == 6
        && parts[2].starts_with('T')
        && parts[2][1..3].bytes().all(|b| b.is_ascii_digit())
        && parts[2][3..].bytes().all(|b| b.is_ascii_uppercase())
        && parts[3].len() == 14
        && parts[3].as_bytes()[7] == b'T'
        && parts[3]
            .bytes()
            .enumerate()
            .all(|(i, b)| i == 7 || b.is_ascii_digit())
        && parts[4] == "v2"
        && parts[5] == "0"
}
pub fn band(key: &str) -> Option<&'static str> {
    match key {
        "red" => Some("B04"),
        "green" => Some("B03"),
        "blue" => Some("B02"),
        _ => None,
    }
}
pub fn asset_path(path: &str) -> bool {
    if super::viirs::asset_path(path) {
        return true;
    }
    if super::srtm::asset_path(path) {
        return true;
    }
    path.strip_prefix(PREFIX).is_some_and(|tail| {
        let Some((id, file)) = tail.split_once('/') else {
            return false;
        };
        valid_item(id)
            && ["red", "green", "blue"]
                .iter()
                .any(|key| file == format!("{id}.{}.tif", band(key).unwrap()))
    })
}
pub fn matches_item(url: &Url, id: &str, key: &str) -> bool {
    if key == "viirs" {
        return super::viirs::matches(url.path(), id);
    }
    if key == "srtm" {
        return super::srtm::cell(id).is_some()
            && url.path() == format!("{}{id}/{id}.zip", super::srtm::PREFIX);
    }
    valid_item(id)
        && band(key).is_some_and(|band| url.path() == format!("{PREFIX}{id}/{id}.{band}.tif"))
}

async fn verify_item(client: &Client, catalog: Url, job: &Job) -> Result<()> {
    let response = client
        .get(catalog)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| "NASA catalogue could not be reached")?;
    if !response.status().is_success() {
        return Err(format!(
            "NASA catalogue returned HTTP {}",
            response.status().as_u16()
        ));
    }
    let mut stream = response.bytes_stream();
    let mut body = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "NASA metadata could not be read")?;
        if body.len() + chunk.len() > 2 * 1024 * 1024 {
            return Err("NASA metadata exceeded the size limit".into());
        }
        body.extend_from_slice(&chunk);
    }
    let item: Value =
        serde_json::from_slice(&body).map_err(|_| "NASA catalogue returned invalid metadata")?;
    let (collection, key) = catalog_asset(job)?;
    if item["id"].as_str() != Some(job.item_id.as_str())
        || item["collection"].as_str() != Some(collection)
        || item["assets"][key.as_str()]["href"].as_str() != Some(job.href.as_str())
        || job.asset_key == "viirs" && !super::viirs::period_matches(&item, &job.item_id)
    {
        return Err("The source file no longer matches its official NASA catalogue item".into());
    }
    Ok(())
}

fn catalog_asset(job: &Job) -> Result<(&'static str, String)> {
    if job.asset_key == "viirs" {
        let info = super::viirs::identity(&job.item_id).ok_or("Invalid VIIRS original product")?;
        return Ok((info.collection, info.production));
    }
    if job.asset_key == "srtm" && super::srtm::cell(&job.item_id).is_some() {
        Ok((super::srtm::COLLECTION, "hgt".into()))
    } else {
        Ok((
            "HLSL30_2.0",
            band(&job.asset_key).ok_or("Unsupported NASA asset")?.into(),
        ))
    }
}

// LP DAAC may redirect to this reviewed signed CDN. Never forward the bearer
// outside the original data host or follow a login redirect as a file response.
fn cdn_target(location: &str, original: &Url, now: i64) -> Result<Url> {
    let target = original
        .join(location)
        .map_err(|_| "NASA returned an invalid file redirect")?;
    if target.host_str() == Some("urs.earthdata.nasa.gov") {
        return Err(ACCESS_ERROR.into());
    }
    let suffix = original
        .path()
        .strip_prefix("/lp-prod-protected/")
        .ok_or("Invalid HLS source path")?;
    let prefix = target.path().strip_suffix(&format!(
        "/lp-prod-protected.s3.us-west-2.amazonaws.com/{suffix}"
    ));
    let pairs: Vec<_> = target.query_pairs().collect();
    let unique = |key: &str| {
        let values: Vec<_> = pairs
            .iter()
            .filter(|(k, _)| k == key)
            .map(|(_, v)| v.as_ref())
            .collect();
        if values.len() == 1 {
            Some(values[0])
        } else {
            None
        }
    };
    let expiry = unique("Expires").and_then(|v| v.parse::<i64>().ok());
    if target.scheme() != "https"
        || target.port_or_known_default() != Some(443)
        || target.host_str() != Some(CDN)
        || !target.username().is_empty()
        || target.password().is_some()
        || target.fragment().is_some()
        || target.query().is_none_or(|query| query.len() > 16384)
        || prefix.is_none_or(|prefix| {
            !prefix.starts_with("/get-")
                || prefix.len() > 128
                || prefix[5..].is_empty()
                || !prefix[5..]
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
        || expiry.is_none_or(|expiry| expiry <= now || expiry > now + 86400)
        || ["Signature", "Key-Pair-Id"]
            .iter()
            .any(|key| unique(key).is_none_or(str::is_empty))
    {
        return Err("NASA returned an unreviewed or expired file redirect".into());
    }
    Ok(target)
}

async fn protected_response(client: &Client, href: Url, token: &str) -> Result<Response> {
    let response = tokio::time::timeout(
        Duration::from_secs(30),
        client
            .get(href.clone())
            .header(header::AUTHORIZATION, bearer(token)?)
            .send(),
    )
    .await
    .map_err(|_| "NASA source request timed out")?
    .map_err(|_| "NASA source request failed")?;
    match response.status() {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => Err(ACCESS_ERROR.into()),
        StatusCode::MOVED_PERMANENTLY
        | StatusCode::FOUND
        | StatusCode::SEE_OTHER
        | StatusCode::TEMPORARY_REDIRECT
        | StatusCode::PERMANENT_REDIRECT => {
            let location = response
                .headers()
                .get(header::LOCATION)
                .and_then(|value| value.to_str().ok())
                .ok_or("NASA returned a file redirect without a valid location")?;
            let target = cdn_target(location, &href, chrono::Utc::now().timestamp())?;
            // No Authorization, cookies or referer on the signed CDN request.
            let response = tokio::time::timeout(Duration::from_secs(30), client.get(target).send())
                .await
                .map_err(|_| "NASA file request timed out")?
                .map_err(|_| "NASA file transfer could not start")?;
            if !response.status().is_success() {
                return Err(
                    if matches!(
                        response.status(),
                        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
                    ) || response.status().is_redirection()
                    {
                        ACCESS_ERROR.into()
                    } else {
                        format!(
                            "NASA file server returned HTTP {}",
                            response.status().as_u16()
                        )
                    },
                );
            }
            Ok(response)
        }
        _ => Ok(response),
    }
}

impl JobManager {
    pub(crate) async fn nasa_response(&self, client: &Client, job: &Job) -> Result<Response> {
        let original = crate::providers::asset_url(&job.href)?;
        if !matches_item(&original, &job.item_id, &job.asset_key) {
            return Err("Invalid NASA source file".into());
        }
        // Check authorization before making catalogue/file requests. A newly
        // connected account can retry the same unchanged job after restart.
        let token = self.download_token(AccountProvider::Nasa, client).await?;
        let (collection, _) = catalog_asset(job)?;
        let catalog = Url::parse(&format!(
            "https://cmr.earthdata.nasa.gov/stac/LPCLOUD/collections/{collection}/items/{}",
            job.item_id
        ))
        .map_err(|_| "Invalid NASA item URL")?;
        verify_item(client, catalog, job).await?;
        protected_response(client, original, &token).await
    }
}

#[cfg(test)]
mod tests;
