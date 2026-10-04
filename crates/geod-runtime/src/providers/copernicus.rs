//! Sentinel-2 SAFE originals: stable OData UUIDs, native authorization only.
use crate::{accounts::bearer, AccountProvider, Job, JobManager, Result};
use futures_util::{stream, StreamExt};
use reqwest::{header, Client, Response, StatusCode};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{collections::BTreeSet, time::Duration};
use url::Url;
use uuid::Uuid;

pub const HOST: &str = "download.dataspace.copernicus.eu";
const CATALOG: &str = "https://catalogue.dataspace.copernicus.eu/odata/v1/Products";
pub const MAX_PRODUCT_BYTES: u64 = 4 * 1024 * 1024 * 1024;
const ACCESS_ERROR: &str = "Copernicus product access was rejected. Check the account authorization in Settings and retry.";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolveProductsRequest {
    pub item_ids: Vec<String>,
}
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProductAsset {
    pub item_id: String,
    pub href: String,
    pub media_type: &'static str,
    pub bytes: u64,
}

pub fn valid_item(id: &str) -> bool {
    if !id.is_ascii() {
        return false;
    }
    let p: Vec<_> = id.split('_').collect();
    let digits = |s: &str| s.bytes().all(|b| b.is_ascii_digit());
    let stamp =
        |s: &str| s.len() == 15 && s.as_bytes()[8] == b'T' && digits(&s[..8]) && digits(&s[9..]);
    p.len() == 7
        && matches!(p[0], "S2A" | "S2B" | "S2C")
        && p[1] == "MSIL2A"
        && stamp(p[2])
        && p[3].len() == 5
        && p[3].starts_with('N')
        && digits(&p[3][1..])
        && p[4].len() == 4
        && p[4].starts_with('R')
        && digits(&p[4][1..])
        && p[5].len() == 6
        && p[5].starts_with('T')
        && digits(&p[5][1..3])
        && p[5][3..].bytes().all(|b| b.is_ascii_uppercase())
        && stamp(p[6])
}
pub fn product_id(path: &str) -> Option<String> {
    let id = path
        .strip_prefix("/odata/v1/Products(")?
        .strip_suffix(")/$value")?;
    let parsed = Uuid::parse_str(id).ok()?.to_string();
    (parsed == id).then_some(parsed)
}

async fn metadata(client: &Client, url: Url) -> Result<Value> {
    let response = client
        .get(url)
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|_| "Copernicus product catalogue could not be reached")?;
    if !response.status().is_success() {
        return Err(format!(
            "Copernicus product catalogue returned HTTP {}",
            response.status().as_u16()
        ));
    }
    let mut body = Vec::new();
    let mut stream = response.bytes_stream();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "Copernicus product metadata could not be read")?;
        if body.len() + chunk.len() > 128 * 1024 {
            return Err("Copernicus product metadata exceeded the size limit".into());
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| "Copernicus returned invalid product metadata".into())
}
fn product(value: &Value, item: &str) -> Result<ProductAsset> {
    let id = value["Id"]
        .as_str()
        .and_then(|id| {
            Uuid::parse_str(id)
                .ok()
                .filter(|parsed| parsed.to_string() == id)
        })
        .ok_or("Copernicus returned an invalid product identifier")?;
    if value["Name"].as_str() != Some(format!("{item}.SAFE").as_str()) {
        return Err("The Copernicus original product does not match the selected scene".into());
    }
    if value["Online"].as_bool() != Some(true) {
        return Err("This Copernicus product is offline. Choose another scene or check its availability in Copernicus Browser.".into());
    }
    let bytes = value["ContentLength"]
        .as_u64()
        .filter(|bytes| *bytes > 0 && *bytes <= MAX_PRODUCT_BYTES)
        .ok_or("Copernicus product exceeds the 4 GiB transfer limit or has an invalid size")?;
    Ok(ProductAsset {
        item_id: item.into(),
        href: format!("https://{HOST}/odata/v1/Products({id})/$value"),
        media_type: "application/zip",
        bytes,
    })
}
async fn resolve_one(client: &Client, catalog: &str, item: &str) -> Result<ProductAsset> {
    let mut url = Url::parse(catalog).map_err(|_| "Invalid Copernicus catalogue endpoint")?;
    url.query_pairs_mut()
        .append_pair("$filter", &format!("Name eq '{item}.SAFE'"))
        .append_pair("$select", "Id,Name,ContentLength,Online");
    let value = metadata(client, url).await?;
    let entries = value["value"]
        .as_array()
        .filter(|entries| entries.len() == 1)
        .ok_or("Copernicus could not uniquely resolve this original product")?;
    product(&entries[0], item)
}

async fn protected_response(client: &Client, url: Url, token: &str) -> Result<Response> {
    let mut url = url;
    // OData download redirects remain on the exact reviewed host and object.
    // Unexpected storage/login redirects fail closed without exposing Location.
    for _ in 0..3 {
        let response = tokio::time::timeout(
            Duration::from_secs(30),
            client
                .get(url.clone())
                .header(header::AUTHORIZATION, bearer(token)?)
                .send(),
        )
        .await
        .map_err(|_| "Copernicus product request timed out")?
        .map_err(|_| "Copernicus product transfer could not start")?;
        if matches!(
            response.status(),
            StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN
        ) {
            return Err(ACCESS_ERROR.into());
        }
        if !response.status().is_redirection() {
            return Ok(response);
        }
        let next = response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .and_then(|v| url.join(v).ok())
            .ok_or("Copernicus returned an invalid product redirect")?;
        if next.scheme() != "https"
            || next.host_str() != Some(HOST)
            || next.port_or_known_default() != Some(443)
            || !next.username().is_empty()
            || next.password().is_some()
            || next.fragment().is_some()
            || next.path() != url.path()
            || next.query().is_some()
        {
            return Err("Copernicus returned an unreviewed product redirect".into());
        }
        url = next;
    }
    Err("Copernicus returned too many product redirects".into())
}

impl JobManager {
    pub async fn resolve_copernicus_products(
        &self,
        request: ResolveProductsRequest,
    ) -> Result<Vec<ProductAsset>> {
        if request.item_ids.is_empty()
            || request.item_ids.len() > 32
            || request.item_ids.iter().any(|id| !valid_item(id))
            || request.item_ids.iter().collect::<BTreeSet<_>>().len() != request.item_ids.len()
        {
            return Err("Choose between 1 and 32 distinct Sentinel-2 L2A scenes".into());
        }
        let client = crate::proxy::download_client(&self.proxy_settings().await)?;
        let values: Vec<Result<ProductAsset>> = stream::iter(request.item_ids.into_iter())
            .map(|id| {
                let client = client.clone();
                async move { resolve_one(&client, CATALOG, &id).await }
            })
            .buffered(4)
            .collect()
            .await;
        values.into_iter().collect()
    }
    pub(crate) async fn copernicus_response(&self, client: &Client, job: &Job) -> Result<Response> {
        let original = crate::providers::asset_url(&job.href)?;
        let id = product_id(original.path()).ok_or("Invalid Copernicus product URL")?;
        let token = self
            .download_token(AccountProvider::Copernicus, client)
            .await?;
        let mut catalog = Url::parse(&format!("{CATALOG}({id})"))
            .map_err(|_| "Invalid Copernicus catalogue endpoint")?;
        catalog
            .query_pairs_mut()
            .append_pair("$select", "Id,Name,ContentLength,Online");
        let product = product(&metadata(client, catalog).await?, &job.item_id)?;
        if product.href != job.href {
            return Err(
                "The original product no longer matches its official Copernicus catalogue entry"
                    .into(),
            );
        }
        let response = protected_response(client, original, &token).await?;
        // Catalogue ContentLength describes the product; a streamed ZIP may
        // differ. The transfer is checked against its HTTP length when present,
        // and always checked for a complete SAFE directory and file checksum.
        Ok(response)
    }
}

pub fn verify_safe(path: &std::path::Path, item: &str) -> Result<()> {
    let file = std::fs::File::open(path).map_err(crate::io_error)?;
    let mut archive = zip::ZipArchive::new(file)
        .map_err(|_| "The Copernicus product is not a complete ZIP archive")?;
    if archive.is_empty() || archive.len() > 10000 {
        return Err("Invalid SAFE archive entry count".into());
    }
    let root = format!("{item}.SAFE/");
    let mut names = BTreeSet::new();
    let mut uncompressed = 0u64;
    for index in 0..archive.len() {
        let entry = archive
            .by_index_raw(index)
            .map_err(|_| "Invalid SAFE archive directory")?;
        let name = entry.name();
        if !name.starts_with(&root)
            || name.len() > 1024
            || name.contains('\\')
            || name.split('/').any(|part| part == ".." || part == ".")
            || entry.enclosed_name().is_none()
            || entry.encrypted()
            || entry.is_symlink()
            || !names.insert(name.to_string())
        {
            return Err("The SAFE archive contains unexpected or unsafe entries".into());
        }
        uncompressed = uncompressed
            .checked_add(entry.size())
            .ok_or("Invalid SAFE archive size")?;
        if uncompressed > 16 * 1024 * 1024 * 1024 {
            return Err("The SAFE archive exceeds the unpacked size limit".into());
        }
    }
    if !names.contains(&format!("{root}manifest.safe"))
        || !names.contains(&format!("{root}MTD_MSIL2A.xml"))
        || !names
            .iter()
            .any(|name| name.contains("/IMG_DATA/R10m/") && name.ends_with("_TCI_10m.jp2"))
        || !names
            .iter()
            .any(|name| name.contains("/IMG_DATA/R20m/") && name.ends_with("_SCL_20m.jp2"))
    {
        return Err(
            "The SAFE archive is missing Sentinel-2 L2A metadata, true-color or SCL files".into(),
        );
    }
    // Directory validation does not decode JP2 pixels or extract untrusted ZIPs.
    Ok(())
}

#[cfg(test)]
mod tests;
