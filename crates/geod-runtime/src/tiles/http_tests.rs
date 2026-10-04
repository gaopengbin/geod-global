use super::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
async fn peer(responses: Vec<String>) -> (Reader, tokio::task::JoinHandle<Vec<String>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in responses {
            let (mut connection, _) = listener.accept().await.unwrap();
            let mut b = vec![0; 4096];
            let n = connection.read(&mut b).await.unwrap();
            requests.push(String::from_utf8(b[..n].to_vec()).unwrap());
            connection.write_all(response.as_bytes()).await.unwrap();
        }
        requests
    });
    let reader = Reader {
        client: reqwest::Client::builder()
            .no_proxy()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap(),
        url: Url::parse(&format!("http://{address}/fixture.pmtiles")).unwrap(),
        etag: String::new(),
        total: 0,
        blocks: BTreeMap::new(),
        bytes: 0,
    };
    (reader, task)
}
fn response(status: u16, range: &str, etag: &str, length: usize, body: &str) -> String {
    format!("HTTP/1.1 {status} Test\r\nContent-Range: {range}\r\nETag: {etag}\r\nContent-Length: {length}\r\nConnection: close\r\n\r\n{body}")
}
#[tokio::test]
async fn rejects_full_archive_wrong_ranges_weak_versions_and_truncation() {
    for r in [
        response(200, "bytes 0-1/100", "\"a\"", 2, "ab"),
        response(206, "bytes 1-2/100", "\"a\"", 2, "ab"),
        response(206, "bytes 0-1/100", "W/\"a\"", 2, "ab"),
        response(206, "bytes 0-1/100", "\"a\"", 2, "a"),
        response(206, "bytes 0-1/100", "\"a\"", 3, "abc"),
    ] {
        let (mut reader, task) = peer(vec![r]).await;
        assert!(reader.get(0, 2).await.is_err());
        task.await.unwrap();
        assert!(reader.blocks.is_empty());
    }
}
#[tokio::test]
async fn pins_ranges_to_one_version_and_reuses_verified_blocks() {
    let (mut reader, task) = peer(vec![
        response(206, "bytes 0-1/100", "\"a\"", 2, "ab"),
        response(206, "bytes 2-3/100", "\"b\"", 2, "cd"),
    ])
    .await;
    assert_eq!(reader.get(0, 2).await.unwrap(), b"ab");
    assert_eq!(reader.get(0, 2).await.unwrap(), b"ab");
    assert!(reader.get(2, 2).await.is_err());
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests[1].to_ascii_lowercase().contains("if-match: \"a\""));
    assert_eq!(reader.receipts().len(), 1);
}
#[tokio::test]
async fn leaf_lookup_supports_run_lengths_and_rejects_parent_scope_escape() {
    let (s, _, _) = crate::tiles::tests::fixture();
    let mut h = s.header.unwrap();
    h.max_zoom = 2;
    h.tile_length = 100;
    h.leaf_offset = 200;
    h.leaf_length = 100;
    let leaf = format::serialize(&[format::Entry {
        id: 5,
        run: 2,
        length: 10,
        offset: 0,
    }])
    .unwrap();
    let root = format::serialize(&[format::Entry {
        id: 5,
        run: 0,
        length: leaf.len() as u64,
        offset: 0,
    }])
    .unwrap();
    h.root_length = root.len() as u64;
    let (mut reader, task) = peer(Vec::new()).await;
    task.await.unwrap();
    reader.blocks.insert((h.root_offset, h.root_length), root);
    reader
        .blocks
        .insert((h.leaf_offset, leaf.len() as u64), leaf);
    assert_eq!(reader.entry(&h, 6).await.unwrap().unwrap().run, 2);
    assert!(reader.entry(&h, 7).await.unwrap().is_none());
    let bad = format::serialize(&[format::Entry {
        id: 4,
        run: 1,
        length: 10,
        offset: 0,
    }])
    .unwrap();
    reader
        .blocks
        .insert((h.leaf_offset, bad.len() as u64), bad.clone());
    let root = format::serialize(&[format::Entry {
        id: 5,
        run: 0,
        length: bad.len() as u64,
        offset: 0,
    }])
    .unwrap();
    h.root_length = root.len() as u64;
    reader.blocks.insert((h.root_offset, h.root_length), root);
    assert!(reader.entry(&h, 5).await.is_err());
}
