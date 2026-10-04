use super::*;
#[cfg(test)]
#[path = "http_tests.rs"]
mod tests;
use futures_util::StreamExt;
use reqwest::header::{CONTENT_ENCODING, CONTENT_RANGE, ETAG, IF_MATCH, RANGE};
use url::Url;

pub(super) fn source_url(raw: &str) -> Result<Url> {
    let u = features::public_url(raw)?;
    if u.query().is_some() || !u.path().ends_with(".pmtiles") || u.as_str() != raw {
        return Err("Use a public HTTPS PMTiles archive URL without query parameters".into());
    }
    Ok(u)
}
pub(super) fn strong_etag(raw: &str) -> bool {
    raw.len() >= 2
        && raw.len() <= 256
        && raw.starts_with('"')
        && raw.ends_with('"')
        && raw[1..raw.len() - 1]
            .bytes()
            .all(|b| (0x21..=0x7e).contains(&b) && b != b'"')
}
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Receipt {
    pub offset: u64,
    pub bytes: usize,
    pub sha256: String,
}
pub(super) struct Reader {
    client: reqwest::Client,
    url: Url,
    pub etag: String,
    pub total: u64,
    pub blocks: BTreeMap<(u64, u64), Vec<u8>>,
    bytes: usize,
}
impl Reader {
    pub async fn open(raw: &str, settings: &crate::ProxySettings) -> Result<Self> {
        let url = source_url(raw)?;
        let client = features::client(&url, settings).await?;
        let mut r = Self {
            client,
            url,
            etag: String::new(),
            total: 0,
            blocks: BTreeMap::new(),
            bytes: 0,
        };
        r.get(0, 127).await?;
        Ok(r)
    }
    pub async fn get(&mut self, offset: u64, length: u64) -> Result<Vec<u8>> {
        if let Some(b) = self.blocks.get(&(offset, length)) {
            return Ok(b.clone());
        }
        let end = offset
            .checked_add(length)
            .filter(|_| length > 0 && length <= format::MAX_TILE as u64)
            .ok_or("Invalid PMTiles byte range")?;
        if self.total != 0 && end > self.total
            || self.blocks.len() >= 2048
            || self
                .bytes
                .checked_add(length as usize)
                .is_none_or(|n| n > MAX_PACKAGE)
        {
            return Err("PMTiles range request exceeds extraction limits".into());
        }
        let mut request = self
            .client
            .get(self.url.clone())
            .header(RANGE, format!("bytes={offset}-{}", end - 1))
            .header("Accept-Encoding", "identity");
        if !self.etag.is_empty() {
            request = request.header(IF_MATCH, &self.etag);
        }
        let response = request
            .send()
            .await
            .map_err(|_| "Cannot reach PMTiles archive; check the source and proxy")?;
        if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(format!(
                "PMTiles requires exact HTTP range responses; source returned HTTP {}",
                response.status().as_u16()
            ));
        }
        let text = response
            .headers()
            .get(CONTENT_RANGE)
            .and_then(|v| v.to_str().ok())
            .ok_or("PMTiles response has no Content-Range")?;
        let (prefix, total) = text
            .split_once('/')
            .ok_or("Invalid PMTiles Content-Range")?;
        let total = total
            .parse::<u64>()
            .map_err(|_| "Unknown PMTiles archive length")?;
        let etag = response
            .headers()
            .get(ETAG)
            .and_then(|v| v.to_str().ok())
            .filter(|s| strong_etag(s))
            .ok_or("PMTiles source requires a strong ETag to keep its ranges consistent")?
            .to_owned();
        if prefix != format!("bytes {offset}-{}", end - 1)
            || total < end
            || self.total != 0 && (self.total != total || self.etag != etag)
            || response.content_length().is_some_and(|n| n != length)
            || response
                .headers()
                .get(CONTENT_ENCODING)
                .is_some_and(|v| v != "identity")
        {
            return Err("PMTiles archive changed or returned a different byte range".into());
        }
        self.total = total;
        self.etag = etag;
        let mut b = Vec::with_capacity(length as usize);
        let mut stream = response.bytes_stream();
        while let Some(c) = stream.next().await {
            let c = c.map_err(|_| "PMTiles response was interrupted")?;
            if b.len() + c.len() > length as usize {
                return Err("PMTiles response exceeded its requested range".into());
            }
            b.extend_from_slice(&c);
        }
        if b.len() != length as usize {
            return Err("PMTiles response was truncated".into());
        }
        self.bytes += b.len();
        self.blocks.insert((offset, length), b.clone());
        Ok(b)
    }
    pub async fn metadata(&mut self) -> Result<(format::Header, serde_json::Value)> {
        let h = format::Header::parse(&self.get(0, 127).await?, self.total)?;
        let root = self.get(h.root_offset, h.root_length).await?;
        format::directory(&root, &h)?;
        let raw = self.get(h.metadata_offset, h.metadata_length).await?;
        let meta = format::metadata(&raw, &h)?;
        Ok((h, meta))
    }
    pub async fn entry(&mut self, h: &format::Header, id: u64) -> Result<Option<format::Entry>> {
        let (mut offset, mut length) = (h.root_offset, h.root_length);
        let mut seen = BTreeSet::new();
        let mut scope = (0, u64::MAX);
        for _ in 0..4 {
            if !seen.insert((offset, length)) {
                return Err("PMTiles directory contains a cycle".into());
            }
            let entries = format::directory(&self.get(offset, length).await?, h)?;
            if entries.iter().any(|e| {
                e.id < scope.0 || e.id.checked_add(e.run.max(1)).is_none_or(|n| n > scope.1)
            }) {
                return Err("PMTiles leaf exceeds its parent TileID range".into());
            }
            let Some(e) = format::lookup(&entries, id) else {
                return Ok(None);
            };
            if e.run > 0 {
                return Ok(Some(e.clone()));
            }
            scope = (
                e.id,
                entries
                    .iter()
                    .find(|n| n.id > e.id)
                    .map_or(scope.1, |n| n.id),
            );
            offset = h.leaf_offset + e.offset;
            length = e.length;
        }
        Err("PMTiles directory depth exceeds four levels".into())
    }
    pub fn receipts(&self) -> Vec<Receipt> {
        self.blocks
            .iter()
            .map(|(&(offset, _), b)| Receipt {
                offset,
                bytes: b.len(),
                sha256: hash(b),
            })
            .collect()
    }
}
