use super::*;
fn message() -> Message {
    serde_json::from_value(json!({"id":"notice-1","revision":1,"title":{"en":"Notice","zh-CN":"通知"},"body":{"en":"A product notice.","zh-CN":"产品通知。"},"priority":"normal","publishedAt":"2026-01-01T00:00:00Z","expiresAt":null,"minVersion":null,"maxVersion":null,"action":null})).unwrap()
}
#[test]
fn read_and_seen_persist_and_revisions_become_unread_again() {
    let folder = tempfile::tempdir().unwrap();
    let state = Distribution::open(folder.path().to_owned()).unwrap();
    state
        .change(|s| {
            s.items = vec![message()];
            s.read.insert("notice-1".into(), 1);
            s.seen.insert("notice-1".into(), 1);
            s.automatic_checks = false;
        })
        .unwrap();
    drop(state);
    let reopened = Distribution::open(folder.path().to_owned()).unwrap();
    let value = reopened.snapshot("0.1.0-rc.3").unwrap();
    assert_eq!(value["unreadCount"], 0);
    assert_eq!(value["automaticChecks"], false);
    assert_eq!(value["items"][0]["seen"], true);
    reopened.change(|s| s.items[0].revision = 2).unwrap();
    let value = reopened.snapshot("0.1.0-rc.3").unwrap();
    assert_eq!(value["unreadCount"], 1);
    assert_eq!(value["items"][0]["seen"], false);
}
#[test]
fn date_and_version_filters_cover_prerelease_and_expiry() {
    let mut item = message();
    let now = "2026-01-02T00:00:00Z".parse().unwrap();
    let version = semver::Version::parse("0.1.0-rc.3").unwrap();
    assert!(eligible(&item, now, &version));
    item.min_version = Some("0.1.0".into());
    assert!(!eligible(&item, now, &version));
    item.min_version = Some("0.1.0-rc.2".into());
    assert!(eligible(&item, now, &version));
    item.max_version = Some("0.1.0-rc.2".into());
    assert!(!eligible(&item, now, &version));
    item.max_version = None;
    item.expires_at = Some(now);
    assert!(!eligible(&item, now, &version));
    item.expires_at = None;
    item.published_at = now + chrono::Duration::seconds(1);
    assert!(!eligible(&item, now, &version));
}
#[test]
fn invalid_and_cross_product_feeds_are_rejected() {
    let mut feed = Feed {
        schema_version: 1,
        product: PRODUCT.into(),
        items: vec![message()],
    };
    assert!(feed.validate().is_ok());
    feed.product = "GeoD Agent".into();
    assert!(feed.validate().is_err());
    feed.product = PRODUCT.into();
    feed.items.push(message());
    assert!(feed.validate().is_err());
    feed.items.pop();
    feed.items[0].action = Some("https://evil.example".into());
    assert!(feed.validate().is_err());
    feed.items[0].action = None;
    feed.items[0].min_version = Some("invalid".into());
    assert!(feed.validate().is_err());
}
#[test]
fn operation_guard_and_corrupt_persistence_fail_closed() {
    let folder = tempfile::tempdir().unwrap();
    let state = Distribution::open(folder.path().to_owned()).unwrap();
    let guard = state.begin().unwrap();
    assert!(state.begin().is_err());
    drop(guard);
    assert!(state.begin().is_ok());
    fs::write(folder.path().join("state.json"), b"broken").unwrap();
    let recovered = Distribution::open(folder.path().to_owned()).unwrap();
    assert!(recovered.recovered_state);
    assert!(recovered.saved.lock().unwrap().items.is_empty());
    assert!(fs::read_dir(folder.path()).unwrap().any(|item| item
        .unwrap()
        .file_name()
        .to_string_lossy()
        .starts_with("state-damaged-")));
}
#[test]
fn transport_never_accepts_credentials_remote_http_or_scripts() {
    assert!(trusted_url("https://updates.example/global/latest.json"));
    assert!(!trusted_url("http://updates.example/latest.json"));
    assert!(!trusted_url("https://user:pass@updates.example/file"));
    assert!(!trusted_url("javascript:alert(1)"));
    assert!(!trusted_url("https://updates.example/file#other"));
}

#[test]
fn actual_notice_signature_accepts_original_and_rejects_tampered_content() {
    let key = include_str!("fixtures/key.pub").trim();
    let bytes = include_bytes!("fixtures/feed-signed.json");
    let feed = signed_feed(bytes, key).unwrap();
    assert_eq!(feed.items[0].id, "test-notice");
    let mut envelope: Value = serde_json::from_slice(bytes).unwrap();
    let mut payload = STANDARD
        .decode(envelope["payload"].as_str().unwrap())
        .unwrap();
    payload[0] = b'!';
    envelope["payload"] = json!(STANDARD.encode(payload));
    assert!(signed_feed(&serde_json::to_vec(&envelope).unwrap(), key).is_err());
    assert!(signed_feed(bytes, "invalid public key").is_err());
}

// Actual updater SDK over an owned loopback HTTP server. No installer is ever
// called; signatures were generated with the official pinned Tauri CLI.
struct Server {
    endpoint: String,
    stop: std::sync::Arc<AtomicBool>,
    requests: std::sync::Arc<Mutex<Vec<String>>>,
    worker: Option<std::thread::JoinHandle<()>>,
}
impl Drop for Server {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            worker.join().unwrap();
        }
    }
}
fn server(version: &str, signature: &str, payload: Vec<u8>, truncated: bool) -> Server {
    use std::io::{Read, Write};
    use std::net::TcpListener;
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let manifest=serde_json::to_vec(&json!({"version":version,"url":format!("{base}/payload"),"signature":signature,"notes":"Owned SDK verification"})).unwrap();
    let stop = std::sync::Arc::new(AtomicBool::new(false));
    let requests = std::sync::Arc::new(Mutex::new(vec![]));
    let flag = stop.clone();
    let paths = requests.clone();
    let worker = std::thread::spawn(move || {
        while !flag.load(Ordering::Acquire) {
            match listener.accept() {
                Ok((mut stream, _)) => {
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut request = [0u8; 8192];
                    // TCP can split even a small request. Wait for the headers
                    // before selecting the fixture response by its request path.
                    let mut n = 0;
                    while n < request.len()
                        && !request[..n].windows(4).any(|part| part == b"\r\n\r\n")
                    {
                        match stream.read(&mut request[n..]) {
                            Ok(0) | Err(_) => break,
                            Ok(count) => n += count,
                        }
                    }
                    if !request[..n].windows(4).any(|part| part == b"\r\n\r\n") {
                        continue;
                    }
                    let request = String::from_utf8_lossy(&request[..n]);
                    let path = request
                        .lines()
                        .next()
                        .unwrap_or("")
                        .split_whitespace()
                        .nth(1)
                        .unwrap_or("")
                        .to_owned();
                    paths.lock().unwrap().push(path.clone());
                    let body = if path == "/latest" {
                        &manifest
                    } else {
                        &payload
                    };
                    let length = body.len()
                        + if path == "/payload" && truncated {
                            100
                        } else {
                            0
                        };
                    let _=write!(stream,"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n");
                    let _ = stream.write_all(body);
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(5))
                }
                Err(e) => panic!("{e}"),
            }
        }
    });
    Server {
        endpoint: format!("{base}/latest"),
        stop,
        requests,
        worker: Some(worker),
    }
}
fn sdk_app() -> tauri::App<tauri::test::MockRuntime> {
    let mut context = tauri::test::mock_context(tauri::test::noop_assets());
    context.config_mut().plugins.0.insert(
        "updater".into(),
        json!({"pubkey":"","requireSignedVersion":true,"dangerousInsecureTransportProtocol":true}),
    );
    tauri::test::mock_builder()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .build(context)
        .unwrap()
}
#[tokio::test]
async fn actual_sdk_downloads_valid_bytes_and_rejects_tampering_and_version_substitution() {
    let bytes = include_bytes!("fixtures/update.bin").to_vec();
    let signature = include_str!("fixtures/update.bin.sig").trim();
    let key = include_str!("fixtures/key.pub").trim();
    let app = sdk_app();
    for (version, key, payload, truncated, success) in [
        ("0.1.1", key, bytes.clone(), false, true),
        (
            "0.1.1",
            key,
            {
                let mut changed = bytes.clone();
                changed[0] ^= 1;
                changed
            },
            false,
            false,
        ),
        (
            "0.1.1",
            include_str!("fixtures/other-key.pub").trim(),
            bytes.clone(),
            false,
            false,
        ),
        ("0.2.0", key, bytes.clone(), false, false),
        ("0.1.1", key, bytes.clone(), true, false),
    ] {
        let server = server(version, signature, payload, truncated);
        let update = app
            .updater_builder()
            .pubkey(key)
            .endpoints(vec![server.endpoint.parse().unwrap()])
            .unwrap()
            .no_proxy()
            .timeout(Duration::from_secs(3))
            .build()
            .unwrap()
            .check()
            .await
            .unwrap()
            .unwrap();
        let mut received = 0;
        let result = tokio::time::timeout(
            Duration::from_secs(5),
            update.download(|chunk, _| received += chunk, || {}),
        )
        .await
        .unwrap();
        assert_eq!(result.is_ok(), success, "{version}: {result:?}");
        if success {
            assert_eq!(result.unwrap(), bytes);
            assert_eq!(received, bytes.len());
        }
        assert!(server
            .requests
            .lock()
            .unwrap()
            .iter()
            .any(|p| p == "/payload"));
    }
}
#[tokio::test]
async fn actual_sdk_skips_same_and_older_versions_and_requires_a_signed_version() {
    let app = sdk_app();
    let bytes = include_bytes!("fixtures/update.bin").to_vec();
    let key = include_str!("fixtures/key.pub").trim();
    for version in ["0.1.0", "0.0.9"] {
        let server = server(
            version,
            include_str!("fixtures/update.bin.sig").trim(),
            bytes.clone(),
            false,
        );
        let update = app
            .updater_builder()
            .pubkey(key)
            .endpoints(vec![server.endpoint.parse().unwrap()])
            .unwrap()
            .no_proxy()
            .build()
            .unwrap()
            .check()
            .await
            .unwrap();
        assert!(update.is_none());
        assert_eq!(*server.requests.lock().unwrap(), vec!["/latest"]);
    }
    let server = server(
        "0.1.1",
        include_str!("fixtures/legacy.sig").trim(),
        bytes,
        false,
    );
    let update = app
        .updater_builder()
        .pubkey(key)
        .endpoints(vec![server.endpoint.parse().unwrap()])
        .unwrap()
        .no_proxy()
        .build()
        .unwrap()
        .check()
        .await
        .unwrap()
        .unwrap();
    assert!(update.download(|_, _| {}, || {}).await.is_err());
}
