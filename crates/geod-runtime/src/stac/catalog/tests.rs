use super::*;

fn directory(id: &str, links: Value) -> Value {
    json!({"stac_version":"1.1.0","type":"Catalog","id":id,"description":"Public static fixture","links":links})
}
fn item_value(id: &str, bbox: Value, date: Value) -> Value {
    json!({"stac_version":"1.1.0","type":"Feature","id":id,"bbox":bbox,
        "geometry":null,"properties":{"datetime":date},"links":[],
        "assets":{"original":{"href":"../original.tif","type":"image/tiff; application=geotiff"}}})
}
fn connection() -> Connection {
    Connection {
        id: "00000000-0000-4000-8000-000000000001".into(),
        name: "Static fixture".into(),
        url: "https://example.com/catalog.json".into(),
        kind: "catalog".into(),
        connected_at: now(),
        collections: vec![],
        capabilities: Capabilities {
            search_get: false,
            search_post: false,
        },
        snapshot_ids: vec![],
        search_url: None,
        search_method: SearchMethod::Get,
        metadata_sha256: vec![],
        metadata_documents: vec![],
        catalog_nodes: vec![],
    }
}
fn docs(entries: Vec<(&str, Value)>) -> BTreeMap<String, Vec<u8>> {
    entries
        .into_iter()
        .map(|(url, value)| (url.into(), serde_json::to_vec(&value).unwrap()))
        .collect()
}
fn request(c: &Connection) -> SearchRequest {
    SearchRequest {
        connection_id: c.id.clone(),
        collection_id: c.catalog_nodes[0].key.clone(),
        bounds: [13., 52., 14., 53.],
        datetime: None,
        limit: Some(1),
        cursor: None,
    }
}

#[tokio::test]
async fn recursive_discovery_keeps_actual_ids_relative_links_and_reopens_without_network() {
    let temp = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let root = manager.storage_root();
    let declaration = json!({"stac_version":"1.0.0","type":"Collection","id":"actual:collection","description":"Real declaration",
        "license":"CC0-1.0","extent":{"spatial":{"bbox":[[13.,52.,14.,53.]]},"temporal":{"interval":[[null,null]]}},
        "links":[{"rel":"item","href":"items/a.json"},{"rel":"item","href":"items/a.json"},{"rel":"root","href":"/catalog.json"}]});
    let mut a = item_value(
        "upstream:item/一",
        json!([13., 52., 14., 53.]),
        json!("2024-01-01T00:00:00Z"),
    );
    a["collection"] = json!("actual:collection");
    a["links"] = json!([{"rel":"collection","href":"../collection.json"}]);
    let sources = docs(vec![
        (
            "https://example.com/catalog.json",
            directory(
                "local-id",
                json!([
        {"rel":"child","href":"branch/collection.json"},{"rel":"root","href":"/unfollowed.json"}]),
            ),
        ),
        (
            "https://example.com/branch/collection.json",
            declaration.clone(),
        ),
        ("https://example.com/branch/items/a.json", a),
    ]);
    let mut c = connection();
    discover_with(root, &mut c, |url| {
        std::future::ready(
            sources
                .get(&url)
                .cloned()
                .ok_or("unexpected request".into()),
        )
    })
    .await
    .unwrap();
    assert_eq!(c.catalog_nodes.len(), 2);
    assert!(c.collections.is_empty());
    assert!(!c.capabilities.search_get);
    assert_eq!(c.catalog_nodes[1].id, "actual:collection");
    let mut query = request(&c);
    query.collection_id = c.catalog_nodes[1].key.clone();
    let mut cursor = initial(root, &c, query).unwrap();
    assert_eq!(cursor.pending.len(), 1);
    let page = scan_with(root, &c, &mut cursor, 1, |url| {
        std::future::ready(
            sources
                .get(&url)
                .cloned()
                .ok_or("unexpected request".into()),
        )
    })
    .await
    .unwrap();
    assert_eq!(page.len(), 1);
    assert_eq!(page[0].collection_id.as_deref(), Some("actual:collection"));
    assert_eq!(page[0].item_id, "upstream:item/一");
    assert_eq!(page[0].provenance.search_mode.as_deref(), Some("catalog"));
    assert_eq!(page[0].provenance.collection, Some(declaration));
    assert_eq!(
        page[0].assets[0].href,
        "https://example.com/branch/original.tif"
    );
    let pin = SourcePin {
        snapshot_id: page[0].id.clone(),
        asset_key: "original".into(),
    };
    assert_eq!(
        resolve(root, &pin).unwrap().collection_id.as_deref(),
        Some("actual:collection")
    );
    let mut registry = Registry::default();
    registry.connections.insert(c.id.clone(), c.clone());
    persist(root, &registry).unwrap();
    let reopened = load(root).await.unwrap();
    assert_eq!(reopened.connections.get(&c.id), Some(&c));
    assert_eq!(record(root, &pin.snapshot_id).unwrap().1, page[0]);
    let mut forged = c.clone();
    forged.catalog_nodes[1].title = "invented".into();
    assert!(validate_connection(root, &forged).is_err());
    forged = c.clone();
    forged.catalog_nodes.pop();
    forged.metadata_documents.pop();
    forged.metadata_sha256.pop();
    assert!(validate_connection(root, &forged)
        .unwrap_err()
        .contains("incomplete"));
    let (mut r, snapshot) = record(root, &pin.snapshot_id).unwrap();
    r.search.as_mut().unwrap().collection_id = "f".repeat(64);
    assert!(validate_record(root, &r, &snapshot).is_err());
}

#[tokio::test]
async fn local_filters_continue_across_empty_pages_and_keep_time_intervals_and_missing_metadata_explicit(
) {
    let temp = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let root = manager.storage_root();
    let links = (0..35)
        .map(|i| json!({"rel":"item","href":format!("items/{i}.json")}))
        .collect::<Vec<_>>();
    let mut sources = docs(vec![(
        "https://example.com/catalog.json",
        directory("root", json!(links)),
    )]);
    for i in 0..35 {
        sources.insert(
            format!("https://example.com/items/{i}.json"),
            serde_json::to_vec(&item_value(
                &format!("item-{i}"),
                json!(if i < 32 {
                    [0., 0., 1., 1.]
                } else {
                    [13., 52., 14., 53.]
                }),
                json!("2024-01-01T00:00:00Z"),
            ))
            .unwrap(),
        );
    }
    let mut c = connection();
    discover_with(root, &mut c, |url| {
        std::future::ready(sources.get(&url).cloned().ok_or("unexpected".into()))
    })
    .await
    .unwrap();
    let mut query = request(&c);
    query.datetime = Some("2024-01-01T00:00:00Z/..".into());
    let mut cursor = initial(root, &c, query.clone()).unwrap();
    assert!(
        scan_with(root, &c, &mut cursor, 20, |url| std::future::ready(
            sources.get(&url).cloned().ok_or("unexpected".into())
        ))
        .await
        .unwrap()
        .is_empty()
    );
    assert_eq!(cursor.scanned, 32);
    assert_eq!(cursor.pending.len(), 3);
    let second = scan_with(root, &c, &mut cursor, 1, |url| {
        std::future::ready(sources.get(&url).cloned().ok_or("unexpected".into()))
    })
    .await
    .unwrap();
    assert_eq!(second[0].item_id, "item-32");
    assert_eq!(cursor.pending.len(), 2);
    let mut snapshot = second[0].clone();
    snapshot.datetime = None;
    snapshot.start_datetime = Some("2023-12-30T00:00:00Z".into());
    snapshot.end_datetime = Some("2024-01-02T00:00:00Z".into());
    assert!(matches_filter(&snapshot, &query).unwrap());
    query.datetime = Some("../2023-12-29T00:00:00Z".into());
    assert!(!matches_filter(&snapshot, &query).unwrap());
    query.datetime = None;
    snapshot.bbox = None;
    assert!(!matches_filter(&snapshot, &query).unwrap());
    snapshot.bbox = Some(query.bounds);
    snapshot.start_datetime = None;
    snapshot.end_datetime = None;
    query.datetime = Some("2024-01-01T00:00:00Z".into());
    assert!(!matches_filter(&snapshot, &query).unwrap());
}

#[tokio::test]
async fn discovery_rejects_cycles_multiple_parents_foreign_origins_and_incomplete_archives() {
    let temp = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let root = manager.storage_root();
    for (links, child) in [
        (json!([{"rel":"child","href":"catalog.json"}]), None),
        (
            json!([{"rel":"child","href":"https://elsewhere.example/catalog.json"}]),
            None,
        ),
        (
            json!([{"rel":"child","href":"child.json","method":"POST"}]),
            None,
        ),
        (
            json!([{"rel":"child","href":"child.json"}]),
            Some(directory(
                "child",
                json!([{"rel":"child","href":"catalog.json"}]),
            )),
        ),
    ] {
        let mut sources = docs(vec![(
            "https://example.com/catalog.json",
            directory("root", links),
        )]);
        if let Some(child) = child {
            sources.insert(
                "https://example.com/child.json".into(),
                serde_json::to_vec(&child).unwrap(),
            );
        }
        let mut c = connection();
        assert!(discover_with(root, &mut c, |url| std::future::ready(
            sources.get(&url).cloned().ok_or("unexpected".into())
        ))
        .await
        .is_err());
    }
}

#[tokio::test]
async fn sibling_collection_link_is_archived_without_fabricating_directory_identity() {
    let temp = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let root = manager.storage_root();
    let mut a = item_value(
        "a",
        json!([13., 52., 14., 53.]),
        json!("2024-01-01T00:00:00Z"),
    );
    a["collection"] = json!("actual");
    a["links"] = json!([{"rel":"collection","href":"../collection.json"}]);
    let collection = json!({"stac_version":"1.0.0","type":"Collection","id":"actual","description":"Declaration","license":"other","links":[]});
    let sources = docs(vec![
        (
            "https://example.com/catalog.json",
            directory("root", json!([{"rel":"item","href":"items/a.json"}])),
        ),
        ("https://example.com/items/a.json", a),
        ("https://example.com/collection.json", collection.clone()),
    ]);
    let mut c = connection();
    discover_with(root, &mut c, |url| {
        std::future::ready(sources.get(&url).cloned().ok_or("unexpected".into()))
    })
    .await
    .unwrap();
    let mut cursor = initial(root, &c, request(&c)).unwrap();
    let items = scan_with(root, &c, &mut cursor, 1, |url| {
        std::future::ready(sources.get(&url).cloned().ok_or("unexpected".into()))
    })
    .await
    .unwrap();
    assert_eq!(items[0].provenance.collection, Some(collection));
    assert_eq!(items[0].provenance.metadata_documents.len(), 2);
    record(root, &items[0].id).unwrap();
}

#[tokio::test]
async fn scan_stops_at_1000_documents_without_misreporting_an_exhausted_directory() {
    let temp = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let root = manager.storage_root();
    let listing = directory(
        "root",
        json!((0..1001)
            .map(|i| json!({"rel":"item","href":format!("items/{i}.json")}))
            .collect::<Vec<_>>()),
    );
    let mut c = connection();
    discover_with(root, &mut c, |_| {
        std::future::ready(Ok(serde_json::to_vec(&listing).unwrap()))
    })
    .await
    .unwrap();
    let mut cursor = initial(root, &c, request(&c)).unwrap();
    while cursor.scanned < MAX_ITEMS {
        let items = scan_with(root, &c, &mut cursor, 100, |url| {
            let id = url.rsplit('/').next().unwrap();
            std::future::ready(Ok(serde_json::to_vec(&item_value(
                id,
                json!([0., 0., 1., 1.]),
                json!("2024-01-01T00:00:00Z"),
            ))
            .unwrap()))
        })
        .await
        .unwrap();
        assert!(items.is_empty());
    }
    assert_eq!(cursor.scanned, 1000);
    assert_eq!(cursor.pending.len(), 1);
    let before = cursor.pending.front().unwrap().url.clone();
    scan_with(root, &c, &mut cursor, 100, |_| {
        std::future::ready(Err("must not fetch after cap".into()))
    })
    .await
    .unwrap();
    assert_eq!(cursor.pending.front().unwrap().url, before);
}

#[tokio::test]
async fn shared_child_directories_are_refused_instead_of_silently_omitting_a_selected_branch() {
    let temp = tempfile::tempdir().unwrap();
    let manager = JobManager::open(temp.path()).await.unwrap();
    let sources = docs(vec![
        (
            "https://example.com/catalog.json",
            directory(
                "root",
                json!([{"rel":"child","href":"a.json"},{"rel":"child","href":"b.json"}]),
            ),
        ),
        (
            "https://example.com/a.json",
            directory("a", json!([{"rel":"child","href":"shared.json"}])),
        ),
        (
            "https://example.com/b.json",
            directory("b", json!([{"rel":"child","href":"shared.json"}])),
        ),
        (
            "https://example.com/shared.json",
            directory("shared", json!([])),
        ),
    ]);
    let mut c = connection();
    let error = discover_with(manager.storage_root(), &mut c, |url| {
        std::future::ready(sources.get(&url).cloned().ok_or("unexpected".into()))
    })
    .await
    .unwrap_err();
    assert!(error.contains("multiple parents"));
}
