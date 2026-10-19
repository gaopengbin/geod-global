//! Read-only, bounded TIFF ranges selected from immutable STAC snapshots.
//! The caller names a saved asset, never a URL or an output/download location.
use super::{
    digest, file_size_declaration, media_supported, public_url, record, resolve, Selection,
};
use crate::{features, JobManager, ProxySettings, Result, MAX_ASSET_BYTES};
use axum::http::{header, HeaderMap, Response};
use futures_util::StreamExt;
use std::{collections::BTreeSet, path::Path, time::Duration};
use url::Url;

const MAX_RANGE_BYTES: u64 = 4 * 1024 * 1024;
const MAX_REDIRECTS: usize = 3;
const READ_DEADLINE: Duration = Duration::from_secs(30);

fn decimal(value: &str) -> Option<u64> {
    (!value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| value.parse().ok())
        .flatten()
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

fn response_range(
    headers: &HeaderMap,
    requested: (u64, u64),
    declared: Option<u64>,
) -> Result<(String, u64)> {
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
        || declared.is_some_and(|value| value != total)
        || start != requested.0
        || start > end
        || end != requested.1.min(total - 1)
        || decimal(
            headers
                .get(header::CONTENT_LENGTH)
                .and_then(|value| value.to_str().ok())
                .unwrap_or(""),
        ) != Some(end - start + 1)
        || headers.get_all(header::CONTENT_RANGE).iter().count() != 1
        || headers.get_all(header::CONTENT_LENGTH).iter().count() != 1
        || headers.get(header::CONTENT_ENCODING).is_some()
    {
        return Err("The source returned a different, encoded or oversized preview range".into());
    }
    Ok((value.to_owned(), end - start + 1))
}

fn selected_asset(root: &Path, snapshot_id: &str, asset_key: &str) -> Result<(Url, Option<u64>)> {
    if !digest(snapshot_id) || asset_key.trim() != asset_key || !super::text(asset_key, 512) {
        return Err("Invalid saved STAC preview selection".into());
    }
    // resolve/record re-read and hash-check the immutable record and document,
    // including reviewed dataset/transport declarations and asset eligibility.
    let pin = Selection {
        snapshot_id: snapshot_id.into(),
        asset_key: asset_key.into(),
    };
    let asset = resolve(root, &pin)?;
    if !media_supported(Some(&asset.media_type)) {
        return Err("Only eligible original TIFF assets support map preview".into());
    }
    let (_, snapshot) = record(root, snapshot_id)?;
    let original = snapshot
        .assets
        .iter()
        .find(|asset| asset.key == asset_key)
        .ok_or("Selected STAC asset is absent from its saved item")?;
    Ok((
        public_url(&asset.href)?,
        file_size_declaration(&original.metadata)?,
    ))
}

fn redirect_target(current: &Url, headers: &HeaderMap, visited: &BTreeSet<String>) -> Result<Url> {
    if headers.get_all(header::LOCATION).iter().count() != 1 {
        return Err("The public preview source returned an ambiguous redirect".into());
    }
    let location = headers
        .get(header::LOCATION)
        .and_then(|value| value.to_str().ok())
        .filter(|value| value.len() <= 2048)
        .ok_or("The public preview source returned an invalid redirect")?;
    let target = current
        .join(location)
        .map_err(|_| "Invalid public preview redirect")?;
    // Every hop must remain public HTTPS without credentials, signed tokens,
    // fragments or nonstandard ports. DNS is separately pinned before the GET.
    let target = public_url(target.as_str())?;
    if visited.contains(target.as_str()) {
        return Err("The public preview source returned a redirect loop".into());
    }
    Ok(target)
}

#[derive(Default)]
struct Transport {
    #[cfg(test)]
    fixture_origin: Option<String>,
}
impl Transport {
    async fn request(
        &self,
        settings: &ProxySettings,
        url: &Url,
        delivery: Option<&Url>,
    ) -> Result<reqwest::RequestBuilder> {
        #[cfg(test)]
        if let Some(origin) = &self.fixture_origin {
            // Test-only mapping retains the public production URL checks above.
            // Shipped builds contain neither this alternate origin nor client.
            let mut fixture = Url::parse(origin).map_err(|_| "Invalid test origin")?;
            fixture.set_path(url.path());
            fixture.set_query(url.query());
            return Ok(reqwest::Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(READ_DEADLINE)
                .build()
                .map_err(|_| "Cannot build test preview client")?
                .get(fixture));
        }
        Ok(features::client_with_timeout(url, settings, READ_DEADLINE)
            .await?
            .get(delivery.unwrap_or(url).clone()))
    }
}

#[derive(Debug)]
pub struct PreviewRange {
    pub content_range: String,
    pub bytes: Vec<u8>,
}
impl PreviewRange {
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
            .expect("Fixed STAC preview response headers")
    }
}

#[cfg(test)]
async fn read_range(
    settings: &ProxySettings,
    url: Url,
    range: &str,
    declared: Option<u64>,
    transport: &Transport,
) -> Result<PreviewRange> {
    read_range_delivery(settings, url, range, declared, transport, None).await
}

async fn read_range_delivery(
    settings: &ProxySettings,
    mut url: Url,
    range: &str,
    declared: Option<u64>,
    transport: &Transport,
    delivery: Option<Url>,
) -> Result<PreviewRange> {
    let requested = request_range(range)?;
    let mut visited = BTreeSet::from([url.to_string()]);
    for hop in 0..=MAX_REDIRECTS {
        let response = transport
            .request(settings, &url, delivery.as_ref())
            .await?
            .header(header::RANGE, range)
            .header(header::ACCEPT_ENCODING, "identity")
            .header(header::ACCEPT, "image/tiff, application/octet-stream")
            .send()
            .await
            .map_err(|_| "Cannot reach the public original TIFF preview")?;
        if response.status().is_redirection() {
            if delivery.is_some() {
                return Err("Anonymous Planetary raster access cannot follow redirects".into());
            }
            if hop == MAX_REDIRECTS {
                return Err("The public preview source returned too many redirects".into());
            }
            url = redirect_target(&url, response.headers(), &visited)?;
            visited.insert(url.to_string());
            continue;
        }
        // A 200 body is never read: there is no whole-file fallback.
        if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(format!(
                "Public TIFF source returned HTTP {}; bounded Range access is required",
                response.status().as_u16()
            ));
        }
        let (content_range, length) = response_range(response.headers(), requested, declared)?;
        let mut bytes = Vec::with_capacity(length as usize);
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "The public preview byte range is incomplete")?;
            if bytes.len() as u64 + chunk.len() as u64 > length {
                return Err("The public source exceeded its declared byte range".into());
            }
            bytes.extend_from_slice(&chunk);
        }
        if bytes.len() as u64 != length {
            return Err("The public preview byte range is incomplete".into());
        }
        if requested.0 == 0
            && bytes.len() >= 4
            && !matches!(
                bytes.get(..4),
                Some(b"II*\0" | b"MM\0*" | b"II+\0" | b"MM\0+")
            )
        {
            return Err("The selected public asset did not return TIFF/BigTIFF bytes".into());
        }
        return Ok(PreviewRange {
            content_range,
            bytes,
        });
    }
    unreachable!("Bounded redirect loop always returns")
}

impl JobManager {
    pub async fn read_stac_preview(
        &self,
        snapshot_id: &str,
        asset_key: &str,
        range: &str,
    ) -> Result<PreviewRange> {
        let (url, declared) = selected_asset(&self.inner.root, snapshot_id, asset_key)?;
        request_range(range)?;
        // Covers queueing, public DNS, all redirect hops and streaming body.
        // Dropping this future also drops reqwest's response/stream.
        tokio::time::timeout(READ_DEADLINE, async {
            let _permit = self
                .inner
                .preview_permits
                .acquire()
                .await
                .map_err(|_| "Preview reader is unavailable")?;
            let settings = self.inner.proxy_settings.lock().await.clone();
            let pin = Selection {
                snapshot_id: snapshot_id.into(),
                asset_key: asset_key.into(),
            };
            let delivery =
                super::planetary_access::delivery_url(&settings, &self.inner.root, &pin).await?;
            read_range_delivery(
                &settings,
                url,
                range,
                declared,
                &Transport::default(),
                delivery,
            )
            .await
        })
        .await
        .map_err(|_| "The public preview range exceeded its 30-second deadline")?
    }
}

fn decoded_component(encoded: &str) -> Result<String> {
    let mut decoded = Vec::new();
    let mut bytes = encoded.as_bytes().iter().copied();
    while let Some(byte) = bytes.next() {
        if byte == b'%' {
            let high = bytes.next().and_then(|value| (value as char).to_digit(16));
            let low = bytes.next().and_then(|value| (value as char).to_digit(16));
            decoded.push(
                (high.ok_or("Invalid STAC asset encoding")? * 16
                    + low.ok_or("Invalid STAC asset encoding")?) as u8,
            );
        } else {
            decoded.push(byte);
        }
    }
    String::from_utf8(decoded).map_err(|_| "Invalid STAC asset encoding".into())
}

/// Parse the fixed desktop URI path emitted by convertFileSrc. Tauri encodes
/// the whole filePath once; the client has separately encoded its asset key.
/// Strip that known outer layer if present, then decode only the key once.
pub fn protocol_selection(path: &str) -> Result<(String, String)> {
    let raw = path.strip_prefix('/').ok_or("Invalid STAC preview path")?;
    if raw.len() > 18432 {
        return Err("Invalid STAC preview path".into());
    }
    let outer;
    let raw = if raw.contains('/') {
        raw
    } else {
        outer = decoded_component(raw)?;
        &outer
    };
    let (snapshot, encoded) = raw.split_once('/').ok_or("Invalid STAC preview path")?;
    if !digest(snapshot) || encoded.is_empty() || encoded.contains('/') || encoded.len() > 6144 {
        return Err("Invalid STAC preview path".into());
    }
    let key = decoded_component(encoded)?;
    if key.trim() != key || !super::text(&key, 512) {
        return Err("Invalid saved STAC asset key".into());
    }
    Ok((snapshot.into(), key))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::Body,
        http::{Request, StatusCode},
    };
    use serde_json::json;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tower::ServiceExt;

    fn archive(root: &Path, key: &str, href: &str, media: &str) -> String {
        std::fs::create_dir_all(root.join("stac")).unwrap();
        let raw = json!({"stac_version":"1.0.0", "type":"Feature", "id":"preview-original", "bbox":[13.,52.,14.,53.], "geometry":null, "properties":{"datetime":"2024-01-01T00:00:00Z"}, "links":[], "assets":{key:{"href":href, "type":media, "roles":["data"]}}});
        let bytes = serde_json::to_vec(&raw).unwrap();
        let document_sha256 = super::super::immutable(root, "document", &bytes).unwrap();
        let saved = super::super::Record {
            version: 1,
            connection_id: "00000000-0000-4000-8000-000000000001".into(),
            service_name: "Public preview fixture".into(),
            kind: "item".into(),
            document_url: "https://example.com/item.json".into(),
            document_request: None,
            document_sha256,
            item_index: None,
            retrieved_at: crate::now(),
            metadata_documents: vec![],
            search: None,
            reviewed_dataset: None,
            asset_transports: vec![],
        };
        super::super::save_snapshot(root, saved, &raw).unwrap().id
    }

    async fn fixture(responses: Vec<Vec<u8>>) -> (Transport, tokio::task::JoinHandle<Vec<String>>) {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let mut requests = Vec::new();
            for response in responses {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut received = Vec::new();
                let mut buffer = [0u8; 4096];
                while !received.ends_with(b"\r\n\r\n") {
                    let count = socket.read(&mut buffer).await.unwrap();
                    assert!(count > 0 && received.len() < 8192);
                    received.extend_from_slice(&buffer[..count]);
                }
                requests.push(String::from_utf8(received).unwrap());
                socket.write_all(&response).await.unwrap();
            }
            requests
        });
        (
            Transport {
                fixture_origin: Some(format!("http://{address}")),
            },
            server,
        )
    }

    fn reply(status: u16, extra: &str, body: &[u8]) -> Vec<u8> {
        let mut bytes =
            format!("HTTP/1.1 {status} Test\r\nConnection: close\r\n{extra}\r\n").into_bytes();
        bytes.extend_from_slice(body);
        bytes
    }
    fn tiff_reply() -> Vec<u8> {
        reply(
            206,
            "Content-Range: bytes 0-7/16\r\nContent-Length: 8\r\n",
            b"II*\0abcd",
        )
    }
    fn source() -> Url {
        public_url("https://example.com/original.tif").unwrap()
    }

    #[test]
    fn range_requires_one_closed_interval_with_the_existing_limits() {
        assert_eq!(request_range("bytes=0-4194303").unwrap(), (0, 4194303));
        assert_eq!(
            request_range("bytes=536870911-536870911").unwrap(),
            (536870911, 536870911)
        );
        for value in [
            "",
            "bytes=0-",
            "bytes=-10",
            "bytes=0-1,5-9",
            "bytes=3-2",
            "bytes=0-4194304",
            "bytes=536870912-536870912",
            "bytes=0-18446744073709551615",
            "bytes=+1-2",
            "Bytes=0-7",
        ] {
            assert!(request_range(value).is_err(), "{value}");
        }
    }

    #[test]
    fn protocol_decodes_only_the_saved_key_once_and_preserves_literal_plus() {
        let id = "a".repeat(64);
        assert_eq!(
            protocol_selection(&format!("/{id}/band%2Fred%2B%E4%B8%80")).unwrap(),
            (id.clone(), "band/red+一".into())
        );
        assert_eq!(
            protocol_selection(&format!("/{id}/band+red")).unwrap().1,
            "band+red"
        );
        assert_eq!(
            protocol_selection(&format!("/{id}/%252F")).unwrap().1,
            "%2F"
        );
        // Actual Tauri convertFileSrc adds this outer whole-path encoding.
        assert_eq!(
            protocol_selection(&format!("/{id}%2Fband%252Fred%252B%25E4%25B8%2580")).unwrap(),
            (id.clone(), "band/red+一".into())
        );
        assert_eq!(
            protocol_selection(&format!("/{id}%2F%25252F")).unwrap().1,
            "%2F"
        );
        for path in [
            format!("/{id}/red/extra"),
            format!("/{id}/%"),
            format!("/{id}/%FF"),
            format!("/{id}/%00"),
            format!("/{id}/%20red"),
            "/../red".into(),
            format!("{id}/red"),
        ] {
            assert!(protocol_selection(&path).is_err(), "{path}");
        }
    }

    #[tokio::test]
    async fn immutable_snapshot_selection_rejects_unknown_key_unsupported_media_private_href_and_tampering(
    ) {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let root = manager.storage_root();
        let id = archive(
            root,
            "red",
            "https://example.com/original.tif",
            "image/tiff; profile=cloud-optimized",
        );
        assert_eq!(selected_asset(root, &id, "red").unwrap().0, source());
        assert!(manager
            .read_stac_preview("../private", "red", "bytes=0-7")
            .await
            .is_err());
        assert!(manager
            .read_stac_preview(&id, "not-in-snapshot", "bytes=0-7")
            .await
            .is_err());
        assert!(manager
            .read_stac_preview(&id, "red", "bytes=0-")
            .await
            .is_err());
        let jpeg = archive(
            root,
            "visual",
            "https://example.com/quicklook.jpg",
            "image/jpeg",
        );
        assert!(selected_asset(root, &jpeg, "visual").is_err());
        let private = archive(root, "red", "https://127.0.0.1/private.tif", "image/tiff");
        assert!(selected_asset(root, &private, "red").is_err());
        let (_, snapshot) = record(root, &id).unwrap();
        std::fs::write(
            root.join("stac")
                .join(format!("document-{}.json", snapshot.document_sha256)),
            b"{}",
        )
        .unwrap();
        assert!(selected_asset(root, &id, "red").is_err());
        assert!(manager.list().await.is_empty());
        assert!(manager.list_projects().await.is_empty());
    }

    #[test]
    fn response_headers_must_identify_the_exact_unencoded_source_interval() {
        let mut headers = HeaderMap::new();
        headers.insert(header::CONTENT_RANGE, "bytes 0-7/16".parse().unwrap());
        headers.insert(header::CONTENT_LENGTH, "8".parse().unwrap());
        assert_eq!(response_range(&headers, (0, 7), Some(16)).unwrap().1, 8);
        assert!(response_range(&headers, (1, 7), Some(16)).is_err());
        assert!(response_range(&headers, (0, 7), Some(17)).is_err());
        headers.insert(header::CONTENT_RANGE, "bytes 0-3/4".parse().unwrap());
        headers.insert(header::CONTENT_LENGTH, "4".parse().unwrap());
        assert_eq!(response_range(&headers, (0, 7), None).unwrap().1, 4);
        for range in [
            "bytes 0-3/*",
            "bytes 0-3/0",
            "bytes 0-3/536870913",
            "bytes 1-3/4",
            "bytes 0-2/4",
        ] {
            headers.insert(header::CONTENT_RANGE, range.parse().unwrap());
            assert!(response_range(&headers, (0, 7), None).is_err(), "{range}");
        }
        headers.insert(header::CONTENT_RANGE, "bytes 0-3/4".parse().unwrap());
        headers.insert(header::CONTENT_ENCODING, "identity".parse().unwrap());
        assert!(response_range(&headers, (0, 7), None).is_err());
        headers.remove(header::CONTENT_ENCODING);
        headers.append(header::CONTENT_RANGE, "bytes 0-3/4".parse().unwrap());
        assert!(response_range(&headers, (0, 7), None).is_err());
    }

    #[test]
    fn redirects_reject_private_signed_credentialed_non_https_and_loops() {
        let visited = BTreeSet::from([source().to_string()]);
        let mut headers = HeaderMap::new();
        for location in [
            "https://127.0.0.1/private.tif",
            "https://10.0.0.1/a.tif",
            "http://example.com/a.tif",
            "https://example.com/a.tif?sig=secret",
            "https://user:secret@example.com/a.tif",
            "/original.tif",
            "https://example.com:444/a.tif",
        ] {
            headers.insert(header::LOCATION, location.parse().unwrap());
            assert!(
                redirect_target(&source(), &headers, &visited).is_err(),
                "{location}"
            );
        }
        headers.insert(header::LOCATION, "/next.tif".parse().unwrap());
        assert_eq!(
            redirect_target(&source(), &headers, &visited)
                .unwrap()
                .as_str(),
            "https://example.com/next.tif"
        );
        headers.append(header::LOCATION, "/ambiguous.tif".parse().unwrap());
        assert!(redirect_target(&source(), &headers, &visited).is_err());
    }

    #[tokio::test]
    async fn public_partial_reads_return_real_tiff_bytes_and_no_store_range_headers() {
        let (transport, server) = fixture(vec![tiff_reply()]).await;
        let data = read_range(
            &ProxySettings::default(),
            source(),
            "bytes=0-7",
            Some(16),
            &transport,
        )
        .await
        .unwrap();
        assert_eq!(data.bytes, b"II*\0abcd");
        let response = data.into_response();
        assert_eq!(response.status(), 206);
        assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 0-7/16");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let requests = server.await.unwrap();
        assert!(requests[0].contains("GET /original.tif "));
        assert!(requests[0]
            .to_ascii_lowercase()
            .contains("range: bytes=0-7"));
        assert!(requests[0]
            .to_ascii_lowercase()
            .contains("accept-encoding: identity"));
    }

    #[tokio::test]
    async fn ephemeral_access_preserves_range_checks_and_never_follows_redirects() {
        let delivery =
            Url::parse("https://example.com/original.tif?sp=r&sr=c&sig=fixture").unwrap();
        let (transport, server) = fixture(vec![tiff_reply()]).await;
        let data = read_range_delivery(
            &ProxySettings::default(),
            source(),
            "bytes=0-7",
            Some(16),
            &transport,
            Some(delivery.clone()),
        )
        .await
        .unwrap();
        assert_eq!(data.bytes, b"II*\0abcd");
        assert_eq!(server.await.unwrap().len(), 1);
        let (transport, server) = fixture(vec![b"HTTP/1.1 302 Found\r\nLocation: /next.tif\r\nContent-Length: 0\r\nConnection: close\r\n\r\n".to_vec()]).await;
        let error = read_range_delivery(
            &ProxySettings::default(),
            source(),
            "bytes=0-7",
            Some(16),
            &transport,
            Some(delivery),
        )
        .await
        .unwrap_err();
        assert_eq!(
            error,
            "Anonymous Planetary raster access cannot follow redirects"
        );
        assert!(!error.contains("sig="));
        assert_eq!(server.await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn no_whole_file_fallback_or_non_tiff_encoded_truncated_body_is_accepted() {
        let cases = [
            reply(200, "Content-Length: 8\r\n", b"II*\0abcd"),
            reply(206, "Content-Length: 8\r\n", b"II*\0abcd"),
            reply(
                206,
                "Content-Range: bytes 0-7/16\r\nContent-Length: 8\r\nContent-Encoding: gzip\r\n",
                b"II*\0abcd",
            ),
            reply(
                206,
                "Content-Range: bytes 0-7/16\r\nContent-Length: 8\r\n",
                b"II*\0",
            ),
            reply(
                206,
                "Content-Range: bytes 0-7/16\r\nContent-Length: 8\r\n",
                b"PNGxxxxx",
            ),
            reply(
                206,
                "Content-Range: bytes 0-8/16\r\nContent-Length: 9\r\n",
                b"II*\0abcde",
            ),
        ];
        for response in cases {
            let (transport, server) = fixture(vec![response]).await;
            assert!(read_range(
                &ProxySettings::default(),
                source(),
                "bytes=0-7",
                None,
                &transport
            )
            .await
            .is_err());
            assert_eq!(server.await.unwrap().len(), 1);
        }
    }

    #[tokio::test]
    async fn redirects_are_manually_revalidated_and_preserve_range_without_forwarding_credentials()
    {
        let redirect = reply(
            302,
            "Location: https://other-public.example.com/next.tif\r\nContent-Length: 0\r\n",
            b"",
        );
        let (transport, server) = fixture(vec![redirect, tiff_reply()]).await;
        assert_eq!(
            read_range(
                &ProxySettings::default(),
                source(),
                "bytes=0-7",
                None,
                &transport
            )
            .await
            .unwrap()
            .bytes,
            b"II*\0abcd"
        );
        let requests = server.await.unwrap();
        assert_eq!(requests.len(), 2);
        assert!(requests[1].contains("GET /next.tif "));
        assert!(requests[1]
            .to_ascii_lowercase()
            .contains("range: bytes=0-7"));
        assert!(!requests[1].to_ascii_lowercase().contains("authorization:"));
        let private = reply(
            302,
            "Location: https://127.0.0.1/private.tif\r\nContent-Length: 0\r\n",
            b"",
        );
        let (transport, server) = fixture(vec![private]).await;
        assert!(read_range(
            &ProxySettings::default(),
            source(),
            "bytes=0-7",
            None,
            &transport
        )
        .await
        .is_err());
        assert_eq!(server.await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn redirect_hops_are_bounded_without_reading_a_redirect_body() {
        let replies = (0..4)
            .map(|index| {
                reply(
                    302,
                    &format!("Location: /hop-{index}.tif\r\nContent-Length: 0\r\n"),
                    b"",
                )
            })
            .collect();
        let (transport, server) = fixture(replies).await;
        assert!(read_range(
            &ProxySettings::default(),
            source(),
            "bytes=0-7",
            None,
            &transport
        )
        .await
        .unwrap_err()
        .contains("too many"));
        assert_eq!(server.await.unwrap().len(), 4);
    }

    #[tokio::test]
    async fn loopback_route_requires_a_saved_key_range_and_the_existing_browser_boundary() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let router = crate::service::router(manager.clone());
        for (uri, range, host, origin, expected) in [
            (
                "/stac/snapshots/invalid/preview?assetKey=red",
                None,
                "127.0.0.1:4318",
                None,
                StatusCode::BAD_REQUEST,
            ),
            (
                "/stac/snapshots/invalid/preview?assetKey=red",
                Some("bytes=0-7"),
                "127.0.0.1:4318",
                None,
                StatusCode::BAD_REQUEST,
            ),
            (
                "/stac/snapshots/invalid/preview?assetKey=red&href=https://example.com/a.tif",
                Some("bytes=0-7"),
                "127.0.0.1:4318",
                None,
                StatusCode::BAD_REQUEST,
            ),
            (
                "/stac/snapshots/invalid/preview?assetKey=red",
                Some("bytes=0-7"),
                "evil.example.com",
                None,
                StatusCode::FORBIDDEN,
            ),
            (
                "/stac/snapshots/invalid/preview?assetKey=red",
                Some("bytes=0-7"),
                "127.0.0.1:4318",
                Some("https://evil.example.com"),
                StatusCode::FORBIDDEN,
            ),
        ] {
            let mut request = Request::builder().uri(uri).header(header::HOST, host);
            if let Some(range) = range {
                request = request.header(header::RANGE, range);
            }
            if let Some(origin) = origin {
                request = request.header(header::ORIGIN, origin);
            }
            assert_eq!(
                router
                    .clone()
                    .oneshot(request.body(Body::empty()).unwrap())
                    .await
                    .unwrap()
                    .status(),
                expected
            );
        }
        assert!(manager.list().await.is_empty());
    }
}
