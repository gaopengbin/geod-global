//! Bounded, read-only Range access to the two reviewed public DEM products.
//! AWS permits anonymous reads but does not provide browser CORS headers.
use crate::{io_error, providers, proxy, JobManager, Result, MAX_ASSET_BYTES};
use axum::http::{header, HeaderMap, Response};
use futures_util::StreamExt;
use std::time::Duration;

const MAX_RANGE_BYTES: u64 = 4 * 1024 * 1024;

fn decimal(value: &str) -> Option<u64> {
    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
}

pub fn elevation_href(item: &str) -> Result<String> {
    let product = providers::dem_product(item).ok_or("Unsupported public elevation tile")?;
    Ok(format!("https://{}/{item}/{item}.tif", product.host()))
}

fn request_range(value: &str) -> Result<(u64, u64)> {
    let (start, end) = value
        .strip_prefix("bytes=")
        .and_then(|value| value.split_once('-'))
        .ok_or("A single bounded byte range is required")?;
    let start = decimal(start).ok_or("Invalid range start")?;
    let end = decimal(end).ok_or("Invalid range end")?;
    if start > end || end >= MAX_ASSET_BYTES || end - start + 1 > MAX_RANGE_BYTES {
        return Err("The public preview range exceeds its limit".into());
    }
    Ok((start, end))
}

fn response_range(headers: &HeaderMap, requested: (u64, u64)) -> Result<(String, u64)> {
    let value = headers
        .get(header::CONTENT_RANGE)
        .and_then(|value| value.to_str().ok())
        .ok_or("The public source omitted Content-Range")?;
    let (interval, total) = value
        .strip_prefix("bytes ")
        .and_then(|value| value.split_once('/'))
        .ok_or("Invalid public source Content-Range")?;
    let (start, end) = interval
        .split_once('-')
        .ok_or("Invalid public source byte interval")?;
    let start = decimal(start).ok_or("Invalid source range start")?;
    let end = decimal(end).ok_or("Invalid source range end")?;
    let total = decimal(total).ok_or("Invalid public source length")?;
    if total == 0
        || total > MAX_ASSET_BYTES
        || start != requested.0
        || start > end
        || end != requested.1.min(total - 1)
        || decimal(
            headers
                .get(header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .unwrap_or(""),
        ) != Some(end - start + 1)
        || headers.get(header::CONTENT_ENCODING).is_some()
    {
        return Err("The source returned a different or oversized preview range".into());
    }
    Ok((value.to_owned(), end - start + 1))
}

pub struct ElevationRange {
    pub content_range: String,
    pub bytes: Vec<u8>,
}
impl ElevationRange {
    pub fn into_response(self) -> Response<Vec<u8>> {
        Response::builder()
            .status(206)
            .header(header::CONTENT_TYPE, "image/tiff")
            .header(header::CONTENT_RANGE, self.content_range)
            .header(header::CONTENT_LENGTH, self.bytes.len())
            .header(header::ACCEPT_RANGES, "bytes")
            .header(header::CACHE_CONTROL, "no-store")
            .header(header::ACCESS_CONTROL_ALLOW_ORIGIN, "*")
            .header(
                header::ACCESS_CONTROL_EXPOSE_HEADERS,
                "Content-Range, Content-Length, Accept-Ranges",
            )
            .header("x-content-type-options", "nosniff")
            .body(self.bytes)
            .expect("Fixed public preview response headers")
    }
}

impl JobManager {
    pub async fn read_elevation_preview(&self, item: &str, range: &str) -> Result<ElevationRange> {
        let href = elevation_href(item)?;
        let requested = request_range(range)?;
        let _permit = self
            .inner
            .preview_permits
            .acquire()
            .await
            .map_err(io_error)?;
        let settings = self.inner.proxy_settings.lock().await.clone();
        let client = proxy::download_builder(&settings)?
            .timeout(Duration::from_secs(30))
            .build()
            .map_err(io_error)?;
        let response = client
            .get(href)
            .header(header::RANGE, range)
            .header(header::ACCEPT_ENCODING, "identity")
            .send()
            .await
            .map_err(io_error)?;
        if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(format!(
                "Public elevation source returned HTTP {}",
                response.status().as_u16()
            ));
        }
        let (content_range, length) = response_range(response.headers(), requested)?;
        let mut bytes = Vec::with_capacity(length as usize);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(io_error)?;
            if bytes.len() as u64 + chunk.len() as u64 > length {
                return Err("The public source exceeded its declared byte range".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.len() as u64 != length {
            return Err("The public preview byte range is incomplete".into());
        }
        Ok(ElevationRange {
            content_range,
            bytes,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn only_reviewed_product_cells_and_bounded_single_ranges_are_readable() {
        for (code, host) in [("10", providers::DEM_HOST), ("30", providers::DEM90_HOST)] {
            let item = format!("Copernicus_DSM_COG_{code}_N37_00_W123_00_DEM");
            assert_eq!(
                elevation_href(&item).unwrap(),
                format!("https://{host}/{item}/{item}.tif")
            );
        }
        for item in [
            "../../private",
            "https://example.test/file",
            "Copernicus_DSM_COG_10_N37_00_W123_00_DEM?file=x",
            "Copernicus_DSM_COG_11_N37_00_W123_00_DEM",
        ] {
            assert!(elevation_href(item).is_err());
        }
        assert_eq!(request_range("bytes=0-65535").unwrap(), (0, 65535));
        for range in [
            "",
            "bytes=0-",
            "bytes=-100",
            "bytes=3-2",
            "bytes=0-1,4-6",
            "bytes=0-4194304",
            "bytes=0-18446744073709551615",
        ] {
            assert!(request_range(range).is_err(), "{range}");
        }
    }
    #[test]
    fn responses_must_match_the_requested_interval_and_actual_bounded_length() {
        let mut headers = HeaderMap::new();
        headers.insert(
            header::CONTENT_RANGE,
            "bytes 0-65535/20060161".parse().unwrap(),
        );
        headers.insert(header::CONTENT_LENGTH, "65536".parse().unwrap());
        assert_eq!(response_range(&headers, (0, 65535)).unwrap().1, 65536);
        assert!(response_range(&headers, (1, 65535)).is_err());
        headers.insert(header::CONTENT_LENGTH, "65537".parse().unwrap());
        assert!(response_range(&headers, (0, 65535)).is_err());
        headers.insert(
            header::CONTENT_RANGE,
            "bytes 0-65535/536870913".parse().unwrap(),
        );
        assert!(response_range(&headers, (0, 65535)).is_err());
        headers.insert(header::CONTENT_RANGE, "bytes 0-99/100".parse().unwrap());
        headers.insert(header::CONTENT_LENGTH, "100".parse().unwrap());
        assert_eq!(response_range(&headers, (0, 65535)).unwrap().1, 100);
        headers.insert(header::CONTENT_ENCODING, "gzip".parse().unwrap());
        assert!(response_range(&headers, (0, 65535)).is_err());
    }
}
