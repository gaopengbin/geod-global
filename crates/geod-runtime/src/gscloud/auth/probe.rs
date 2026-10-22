//! Opt-in local account diagnostics. Never print cookies, account names, query
//! strings or authenticated page bodies; only public routes and asset paths.
use super::*;
use sha2::{Digest, Sha256};

#[tokio::test]
#[ignore = "uses the verified GSCloud account to inspect six public original products"]
async fn live_public_original_products() {
    use tokio::io::AsyncWriteExt;
    let root =
        std::fs::canonicalize(std::env::var("GEOD_GSCLOUD_VERIFIED_RUNTIME").unwrap()).unwrap();
    let out = std::path::PathBuf::from(std::env::var("GEOD_GSCLOUD_PROBE_DIR").unwrap());
    let client = crate::proxy::download_client(&crate::proxy::load(&root).await.unwrap()).unwrap();
    for (pid, id) in [
        ("307", "slope_60_04"),
        ("308", "tpi_60_04"),
        ("309", "asp_60_04"),
        ("310", "ASTGTM_N40E116"),
        ("421", "ASTGTM2_N40E116"),
        ("aeab8000652a45b38afbb7ff023ddabb", "ASTGTMV003_N40E116"),
    ] {
        let path = out.join(format!("{pid}-{id}.zip"));
        if path.exists() {
            println!("Already saved public original: {pid} {id}");
            continue;
        }
        let response = original_response(&root, &client, pid, id).await.unwrap();
        println!(
            "Public original {pid} {id}: HTTP {}; media={:?}; bytes={:?}",
            response.status().as_u16(),
            response.headers().get("Content-Type"),
            response.content_length()
        );
        if response.status().as_u16() != 200
            || !response
                .content_length()
                .is_some_and(|n| n > 0 && n <= 128 * 1024 * 1024)
        {
            continue;
        }
        let total = response.content_length().unwrap();
        let mut file = tokio::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(path.with_extension("zip.part"))
            .await
            .unwrap();
        let mut stream = response.bytes_stream();
        let mut bytes = 0u64;
        let mut hash = Sha256::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk
                .map_err(|_| "Public original stream interrupted")
                .unwrap();
            bytes += chunk.len() as u64;
            assert!(bytes <= total);
            hash.update(&chunk);
            file.write_all(&chunk).await.unwrap();
        }
        assert_eq!(bytes, total);
        file.sync_all().await.unwrap();
        drop(file);
        tokio::fs::rename(path.with_extension("zip.part"), path)
            .await
            .unwrap();
        println!(
            "Verified complete public original: {pid} {id}; bytes={bytes}; sha256={:x}",
            hash.finalize()
        );
    }
}

fn redacted_script(script: &str) -> String {
    let mut out = String::new();
    let mut chars = script.chars().peekable();
    while let Some(ch) = chars.next() {
        if !['\'', '"', '`'].contains(&ch) {
            out.push(ch);
            continue;
        }
        let mut literal = String::new();
        while let Some(c) = chars.next() {
            if c == '\\' {
                if let Some(escaped) = chars.next() {
                    literal.push(escaped);
                }
            } else if c == ch {
                break;
            } else {
                literal.push(c);
            }
        }
        let keep = literal.len() < 180
            && (["/sources/", "/static/", "/userview/mycollect/"]
                .iter()
                .any(|p| literal.starts_with(p))
                || [
                    "GET",
                    "POST",
                    "json",
                    "data",
                    "success",
                    "url",
                    "pid",
                    "ids",
                    "dataids",
                    "dataid",
                    "action",
                    "download",
                    "can_dl",
                    "listdatas",
                    "can_order",
                    "batch_download",
                ]
                .contains(&literal.as_str()));
        out.push(ch);
        out.push_str(if keep {
            literal.split(['?', '#']).next().unwrap()
        } else {
            "[redacted]"
        });
        out.push(ch);
    }
    out
}

#[tokio::test]
#[ignore = "uses the human-verified local GSCloud session for read-only delivery discovery"]
async fn live_saved_session_delivery_routes() {
    let root = std::fs::canonicalize(
        std::env::var("GEOD_GSCLOUD_VERIFIED_RUNTIME")
            .expect("Set the verified desktop runtime directory"),
    )
    .unwrap();
    let cookies = stored(&root)
        .unwrap()
        .expect("Sign in and verify in the desktop first");
    let client = crate::proxy::download_client(&crate::proxy::load(&root).await.unwrap()).unwrap();
    let mut delivery_url: Option<Zeroizing<String>> = None;
    let mut paths = vec![
        "/".to_owned(),
        "/userview/mycollect/data_collect/can_dl".to_owned(),
    ];
    if let Ok(path) = std::env::var("GEOD_GSCLOUD_PROBE_PATH") {
        assert!(path.starts_with('/'));
        paths.push(path);
    }
    for path in paths {
        let origin = if path.starts_with("/sources/download/") {
            "https://bjdl.gscloud.cn"
        } else {
            ORIGIN
        };
        let response = client
            .get(format!("{origin}{path}"))
            .header(COOKIE, cookies.as_str())
            .timeout(Duration::from_secs(30))
            .send()
            .await
            .unwrap();
        let status = response.status().as_u16();
        let media = response
            .headers()
            .get("Content-Type")
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .to_owned();
        let location = response
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .and_then(|u| url::Url::parse(u).ok())
            .map(|u| {
                format!(
                    "{}://{}{}",
                    u.scheme(),
                    u.host_str().unwrap_or(""),
                    u.path()
                )
            });
        println!("Official GET {origin}{path}; HTTP {status}; media={media}; redirect origin/path={location:?}");
        if status != 200 || !media.contains("text/html") {
            continue;
        }
        let html = Zeroizing::new(response.text().await.unwrap());
        assert!(html.len() < 2 * 1024 * 1024);
        if path == "/" {
            assert!(
                authenticated_header(&html),
                "Website session is not authenticated"
            );
        }
        let routes: std::collections::BTreeSet<_> = html
            .split(['\'', '"'])
            .filter(|s| {
                s.len() < 300
                    && [
                        "/sources/",
                        "/static/",
                        "/userview/mycollect/",
                        "/download/",
                    ]
                    .iter()
                    .any(|prefix| s.starts_with(prefix))
            })
            .filter_map(|s| s.split(['?', '#']).next())
            .collect();
        let functions: std::collections::BTreeSet<_> = html
            .split("function ")
            .skip(1)
            .filter_map(|s| s.split('(').next())
            .filter(|s| s.len() < 70 && s.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'_'))
            .collect();
        println!("Verified GSCloud page {path}; HTTP {status}; public routes={routes:?}; function names={functions:?}");
        if let Some((_, tail)) = html.split_once("var download = function") {
            if let Some((body, _)) = tail.split_once("};") {
                let literals = body.split('"').skip(1).step_by(2).collect::<Vec<_>>();
                if literals.len() == 3
                    && literals[0] == "https://bjdl.gscloud.cn/sources/download/"
                    && literals[1] == "/"
                    && literals[2].starts_with("?sid=")
                {
                    delivery_url = Some(Zeroizing::new(format!(
                        "{}305/srtm_60_04{}",
                        literals[0], literals[2]
                    )));
                }
                for literal in body.split(['\'', '"']).filter(|s| {
                    s.len() < 240
                        && (s.starts_with('/')
                            || s.starts_with("https://")
                            || s.starts_with("http://")
                            || s.starts_with('&')
                            || s.starts_with('?'))
                }) {
                    println!(
                        "Official single-download route literal: {}",
                        literal.split(['?', '#']).next().unwrap()
                    );
                    if let Some((_, query)) = literal.split_once('?') {
                        println!(
                            "Query field names: {:?}",
                            query
                                .split('&')
                                .map(|q| q.split('=').next().unwrap())
                                .collect::<Vec<_>>()
                        );
                    }
                }
            }
        }
        if path.contains("mycollect") || path.contains("/download/") {
            let mut scripts = String::new();
            for after in html.split("<script").skip(1) {
                let Some((tag, body)) = after.split_once('>') else {
                    continue;
                };
                if tag.contains("src=") {
                    continue;
                }
                if let Some((body, _)) = body.split_once("</script>") {
                    scripts.push_str(&redacted_script(body));
                    scripts.push('\n');
                }
            }
            if let Ok(out) = std::env::var("GEOD_GSCLOUD_PROBE_DIR") {
                let name = if path.contains("mycollect") {
                    "collection-inline-redacted.js"
                } else {
                    "delivery-inline-redacted.js"
                };
                std::fs::write(std::path::Path::new(&out).join(name), scripts).unwrap();
            }
        }
    }
    if let Some(url) = delivery_url {
        let response = client
            .get(url.as_str())
            .header("Referer", format!("{ORIGIN}/"))
            .send()
            .await
            .map_err(|e| e.without_url().to_string())
            .unwrap();
        let location = response
            .headers()
            .get("Location")
            .and_then(|v| v.to_str().ok())
            .and_then(|u| url::Url::parse(u).ok())
            .map(|u| {
                format!(
                    "{}://{}{}",
                    u.scheme(),
                    u.host_str().unwrap_or(""),
                    u.path()
                )
            });
        println!("Official credential-bound SRTM request: status={}, media={:?}, declaredBytes={:?}, redirect origin/path={location:?}",response.status().as_u16(),response.headers().get("Content-Type"),response.content_length());
        if std::env::var("GEOD_GSCLOUD_PROBE_DOWNLOAD").as_deref() == Ok("1") {
            use tokio::io::AsyncWriteExt;
            assert_eq!(response.status().as_u16(), 200);
            let total = response.content_length().unwrap();
            assert!(total > 0 && total <= 64 * 1024 * 1024);
            let out = std::path::PathBuf::from(std::env::var("GEOD_GSCLOUD_PROBE_DIR").unwrap())
                .join("srtm_60_04-original.zip");
            let partial = out.with_extension("zip.part");
            let mut file = tokio::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&partial)
                .await
                .unwrap();
            let mut stream = response.bytes_stream();
            let mut bytes = 0u64;
            let mut hash = Sha256::new();
            let mut magic: Vec<u8> = Vec::new();
            while let Some(chunk) = stream.next().await {
                let chunk = chunk.map_err(|e| e.without_url().to_string()).unwrap();
                bytes += chunk.len() as u64;
                assert!(bytes <= total);
                magic.extend(chunk.iter().take(4usize.saturating_sub(magic.len())));
                hash.update(&chunk);
                file.write_all(&chunk).await.unwrap();
            }
            assert_eq!(bytes, total);
            assert_eq!(magic, b"PK\x03\x04");
            file.sync_all().await.unwrap();
            drop(file);
            tokio::fs::rename(partial, out).await.unwrap();
            println!(
                "Whole GSCloud SRTM original saved: bytes={bytes}; sha256={:x}",
                hash.finalize()
            );
        }
    }
}
