//! Per-workspace source-download proxy policy. Loopback API clients remain direct.
use crate::{io_error, JobManager, Result};
use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use std::{io::ErrorKind, time::Duration};
use tokio::io::AsyncWriteExt;
use url::Url;

const SETTINGS_FILE: &str = "proxy-settings.json";
const TEST_ASSET: &str = "https://sentinel-cogs.s3.us-west-2.amazonaws.com/sentinel-s2-l2a-cogs/10/S/EG/2025/6/S2C_10SEG_20250627_0_L2A/SCL.tif";

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ProxyMode {
    #[default]
    System,
    Direct,
    Custom,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProxySettings {
    pub mode: ProxyMode,
    pub url: Option<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProxyTest {
    pub status: &'static str,
    pub status_code: u16,
    pub elapsed_ms: u64,
    pub received_bytes: u64,
}

// A successful HEAD is insufficient: require a bounded part of the actual
// original TIFF to arrive, with the requested interval and signature intact.
async fn test_source_bytes(
    client: &reqwest::Client,
    href: &str,
    deadline: Duration,
) -> Result<ProxyTest> {
    let started = std::time::Instant::now();
    tokio::time::timeout(deadline, async {
        let response = client.get(href)
            .header(reqwest::header::RANGE, "bytes=0-65535")
            .send().await.map_err(crate::transfer::network_error)?;
        if response.status() != reqwest::StatusCode::PARTIAL_CONTENT {
            return Err(format!("The Sentinel sample request returned HTTP {}; a partial original response is required", response.status()));
        }
        let headers=response.headers();
        let invalid="The Sentinel sample has an invalid byte range";
        if headers.get_all(reqwest::header::CONTENT_RANGE).iter().count()!=1{return Err(invalid.into());}
        let (end,total)=headers.get(reqwest::header::CONTENT_RANGE).and_then(|v|v.to_str().ok())
            .and_then(|v|v.strip_prefix("bytes 0-")).and_then(|v|v.split_once('/')).ok_or(invalid)?;
        let end=end.parse::<u64>().map_err(|_|invalid)?;
        let total=total.parse::<u64>().map_err(|_|invalid)?;
        if total==0||end!=65535.min(total-1){return Err(invalid.into());}
        let expected=end+1;
        if response.content_length().is_some_and(|length|length!=expected){return Err(invalid.into());}
        let mut stream=response.bytes_stream();let mut bytes=Vec::with_capacity(expected as usize);
        while let Some(chunk)=stream.next().await {
            let chunk=chunk.map_err(crate::transfer::network_error)?;
            if bytes.len()+chunk.len()>expected as usize{return Err("The Sentinel sample exceeded its requested byte range".into());}
            bytes.extend_from_slice(&chunk);
        }
        if bytes.len()!=expected as usize{return Err("The Sentinel sample response was incomplete".into());}
        crate::verify_signature(&bytes,"image/tiff")?;
        Ok(ProxyTest {status:"connected",status_code:206,elapsed_ms:started.elapsed().as_millis().min(u64::MAX as u128) as u64,received_bytes:bytes.len() as u64})
    }).await.map_err(|_|"The Sentinel sample timed out before its requested data arrived".to_owned())?
}

impl ProxySettings {
    fn normalized(mut self) -> Result<Self> {
        match self.mode {
            ProxyMode::System | ProxyMode::Direct => {
                if self.url.as_ref().is_some_and(|url| !url.trim().is_empty()) {
                    return Err("Only custom mode accepts a proxy address".into());
                }
                self.url = None;
            }
            ProxyMode::Custom => {
                let raw = self.url.as_deref().unwrap_or("").trim();
                if raw.len() > 512 || raw.is_empty() {
                    return Err("Enter a proxy address up to 512 characters".into());
                }
                let url = Url::parse(raw).map_err(|_| "Invalid proxy address")?;
                let explicit_port = raw
                    .split_once("://")
                    .and_then(|(_, authority)| authority.split(['/', '?', '#']).next())
                    .and_then(|authority| authority.rsplit_once(':'))
                    .and_then(|(_, port)| port.parse::<u16>().ok())
                    .is_some_and(|port| port > 0);
                if !matches!(url.scheme(), "http" | "https" | "socks5" | "socks5h")
                    || url.host_str().is_none()
                    || !explicit_port
                    || !url.username().is_empty()
                    || url.password().is_some()
                    || !matches!(url.path(), "" | "/")
                    || url.query().is_some()
                    || url.fragment().is_some()
                {
                    return Err("Use an HTTP(S) or SOCKS5 proxy with an explicit port and no credentials, path or query".into());
                }
                // Preserve an explicitly entered default port. Url::parse strips :80/:443.
                self.url = Some(raw.trim_end_matches('/').to_owned());
            }
        }
        Ok(self)
    }
}

pub(crate) fn download_client(settings: &ProxySettings) -> Result<reqwest::Client> {
    download_builder(settings)?.build().map_err(io_error)
}

pub(crate) fn download_builder(settings: &ProxySettings) -> Result<reqwest::ClientBuilder> {
    download_builder_with_read_timeout(settings, Duration::from_secs(45))
}

fn download_builder_with_read_timeout(
    settings: &ProxySettings,
    read_timeout: Duration,
) -> Result<reqwest::ClientBuilder> {
    let mut builder = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(20))
        // A healthy multi-gigabyte transfer may take hours. Bound inactivity,
        // not elapsed transfer time; catalogue/account requests have their own
        // shorter total deadlines and the download worker bounds every chunk.
        .read_timeout(read_timeout)
        .user_agent(concat!("GeoD-Global/", env!("CARGO_PKG_VERSION")));
    builder = match settings.mode {
        ProxyMode::System => builder,
        ProxyMode::Direct => builder.no_proxy(),
        ProxyMode::Custom => builder.proxy(
            reqwest::Proxy::all(settings.url.as_deref().ok_or("Missing proxy address")?)
                .map_err(io_error)?,
        ),
    };
    Ok(builder)
}

pub(crate) async fn load(root: &std::path::Path) -> Result<ProxySettings> {
    match tokio::fs::read(root.join(SETTINGS_FILE)).await {
        Ok(bytes) => serde_json::from_slice::<ProxySettings>(&bytes)
            .map_err(|error| format!("Cannot read saved proxy settings: {error}"))?
            .normalized(),
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(ProxySettings::default()),
        Err(error) => Err(io_error(error)),
    }
}

impl JobManager {
    pub async fn proxy_settings(&self) -> ProxySettings {
        self.inner.proxy_settings.lock().await.clone()
    }

    /// New downloads use the new setting; already-open responses retain their route.
    pub async fn save_proxy_settings(&self, request: ProxySettings) -> Result<ProxySettings> {
        let settings = request.normalized()?;
        download_client(&settings)?;
        let mut current = self.inner.proxy_settings.lock().await;
        let pending = self.inner.root.join(format!("{SETTINGS_FILE}.tmp"));
        let final_path = self.inner.root.join(SETTINGS_FILE);
        let mut file = tokio::fs::File::create(&pending).await.map_err(io_error)?;
        file.write_all(&serde_json::to_vec_pretty(&settings).map_err(io_error)?)
            .await
            .map_err(io_error)?;
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        tokio::fs::rename(pending, final_path)
            .await
            .map_err(io_error)?;
        *current = settings.clone();
        Ok(settings)
    }

    /// Tests the unsaved selection against one known public Sentinel asset.
    pub async fn test_proxy_settings(&self, request: ProxySettings) -> Result<ProxyTest> {
        let settings = request.normalized()?;
        let client = download_client(&settings)?;
        test_source_bytes(&client, TEST_ASSET, Duration::from_secs(15)).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn connection_test_requires_the_requested_original_body_not_only_successful_headers() {
        use tokio::io::AsyncReadExt;
        let client = download_client(&ProxySettings {
            mode: ProxyMode::Direct,
            url: None,
        })
        .unwrap();
        let valid = [b"II*\0".as_slice(), &[0; 12]].concat();
        let cases = [
            (206, "bytes 0-15/16", valid.clone(), false, true),
            (200, "bytes 0-15/16", valid.clone(), false, false),
            (206, "bytes 0-16/16", valid.clone(), false, false),
            (
                206,
                "bytes 0-15/16",
                b"<html>error</html>".to_vec(),
                false,
                false,
            ),
            (206, "bytes 0-15/16", valid[..4].to_vec(), false, false),
            (206, "bytes 0-15/16", valid[..4].to_vec(), true, false),
        ];
        for (status, range, body, stall, success) in cases {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = Vec::new();
                let mut buffer = [0; 4096];
                while !request.ends_with(b"\r\n\r\n") {
                    let count = socket.read(&mut buffer).await.unwrap();
                    assert!(count > 0);
                    request.extend_from_slice(&buffer[..count]);
                    assert!(request.len() < 8192);
                }
                assert!(String::from_utf8_lossy(&request)
                    .to_ascii_lowercase()
                    .contains("\r\nrange: bytes=0-65535\r\n"));
                let header=format!("HTTP/1.1 {status} Test\r\nContent-Type: image/tiff\r\nContent-Length: 16\r\nContent-Range: {range}\r\nConnection: close\r\n\r\n");
                socket.write_all(header.as_bytes()).await.unwrap();
                let _ = socket.write_all(&body).await;
                if stall {
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }
            });
            let result = test_source_bytes(
                &client,
                &format!("http://{address}/sample"),
                Duration::from_millis(500),
            )
            .await;
            assert_eq!(result.is_ok(), success, "{result:?}");
            if let Ok(result) = result {
                assert_eq!(result.received_bytes, 16);
                assert_eq!(result.status_code, 206);
            }
            if stall {
                server.abort();
            } else {
                server.await.unwrap();
            }
        }
    }

    #[tokio::test]
    async fn streaming_read_deadline_resets_when_data_arrives_and_rejects_a_stall() {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            for stalled in [false, true] {
                let (mut socket, _) = listener.accept().await.unwrap();
                let mut request = [0; 4096];
                let mut headers = Vec::new();
                while !headers.ends_with(b"\r\n\r\n") {
                    let count = socket.read(&mut request).await.unwrap();
                    assert!(count > 0);
                    headers.extend_from_slice(&request[..count]);
                    assert!(headers.len() <= 4096);
                }
                socket
                    .write_all(
                        b"HTTP/1.1 200 OK\r\nContent-Length: 7\r\nConnection: close\r\n\r\na",
                    )
                    .await
                    .unwrap();
                if stalled {
                    tokio::time::sleep(Duration::from_millis(1500)).await;
                } else {
                    for _ in 0..6 {
                        tokio::time::sleep(Duration::from_millis(250)).await;
                        socket.write_all(b"a").await.unwrap();
                    }
                }
            }
        });
        let client = download_builder_with_read_timeout(
            &ProxySettings {
                mode: ProxyMode::Direct,
                url: None,
            },
            Duration::from_secs(1),
        )
        .unwrap()
        .build()
        .unwrap();
        let start = std::time::Instant::now();
        let response = client
            .get(format!("http://{address}/progressing"))
            .send()
            .await
            .unwrap();
        assert_eq!(response.bytes().await.unwrap(), "aaaaaaa");
        assert!(start.elapsed() > Duration::from_secs(1));
        let response = client
            .get(format!("http://{address}/stalled"))
            .send()
            .await
            .unwrap();
        assert!(response.bytes().await.unwrap_err().is_timeout());
        server.await.unwrap();
    }

    #[test]
    fn custom_proxy_requires_plain_host_and_port_without_secrets() {
        for invalid in [
            "http://user:secret@127.0.0.1:10808",
            "http://127.0.0.1:10808/path",
            "http://127.0.0.1:10808/?token=secret",
            "file:///tmp/proxy",
            "http://127.0.0.1",
        ] {
            assert!(ProxySettings {
                mode: ProxyMode::Custom,
                url: Some(invalid.into())
            }
            .normalized()
            .is_err());
        }
        for valid in [
            "http://127.0.0.1:10808",
            "http://localhost:80",
            "socks5h://localhost:10808",
        ] {
            let settings = ProxySettings {
                mode: ProxyMode::Custom,
                url: Some(valid.into()),
            }
            .normalized()
            .unwrap();
            assert_eq!(settings.url.as_deref(), Some(valid));
            download_client(&settings).unwrap();
        }
    }

    #[tokio::test]
    async fn proxy_choice_persists_and_invalid_update_preserves_previous_value() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        assert_eq!(manager.proxy_settings().await, ProxySettings::default());
        let direct = ProxySettings {
            mode: ProxyMode::Direct,
            url: None,
        };
        assert_eq!(
            manager.save_proxy_settings(direct.clone()).await.unwrap(),
            direct
        );
        assert!(manager
            .save_proxy_settings(ProxySettings {
                mode: ProxyMode::Custom,
                url: Some("http://user:pass@localhost:10808".into()),
            })
            .await
            .is_err());
        assert_eq!(manager.proxy_settings().await, direct);
        assert_eq!(
            manager.save_proxy_settings(direct.clone()).await.unwrap(),
            direct
        );
        drop(manager);
        let reopened = JobManager::open(directory.path()).await.unwrap();
        assert_eq!(reopened.proxy_settings().await, direct);
    }
}
