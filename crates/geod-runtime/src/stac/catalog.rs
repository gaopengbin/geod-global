//! Bounded static STAC discovery. Directory keys are local URL hashes, never
//! substituted for an upstream Collection ID. Search is a local bbox filter,
//! not an advertised STAC API geometry-intersection service.
use super::*;
use std::{collections::VecDeque, future::Future};

const MAX_NODES: usize = 128;
const MAX_DEPTH: usize = 24;
const MAX_LINKS: usize = 4096;
const MAX_PENDING: usize = 10000;
const PAGE_SCAN: usize = 32;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogNode {
    pub key: String,
    pub id: String,
    pub title: String,
    pub kind: String,
    pub description: String,
    pub license: Option<String>,
    pub url: String,
    pub parent_key: Option<String>,
}

#[derive(Debug, Clone)]
struct Target {
    url: String,
    receipts: Vec<DocumentReceipt>,
}
#[derive(Debug, Clone)]
pub(super) struct CatalogCursor {
    pub request: SearchRequest,
    pending: VecDeque<Target>,
    identities: BTreeSet<String>,
    scanned: usize,
    bytes: usize,
}

fn directory_links(value: &Value, rel: &str, base: &Url) -> Result<Vec<Url>> {
    let links = value["links"]
        .as_array()
        .filter(|v| v.len() <= MAX_LINKS)
        .ok_or("Static STAC links are missing or exceed 4096; connect a smaller directory")?;
    let mut found = Vec::new();
    let mut seen = BTreeSet::new();
    for entry in links.iter().filter(|v| v["rel"] == rel) {
        if entry.get("method").is_some_and(|v| v != "GET")
            || entry.get("body").is_some()
            || entry.get("headers").is_some()
        {
            return Err("Static STAC directory links must use public GET without headers".into());
        }
        let url = scoped(
            base,
            entry["href"]
                .as_str()
                .ok_or("Static STAC link has no URL")?,
        )?;
        if seen.insert(url.to_string()) {
            found.push(url);
        }
    }
    Ok(found)
}

fn node(value: &Value, url: &Url, parent_key: Option<String>) -> Result<CatalogNode> {
    version(value)?;
    let kind = value["type"]
        .as_str()
        .filter(|v| matches!(*v, "Catalog" | "Collection"))
        .ok_or("A static STAC child must be a Catalog or Collection")?;
    let id = value["id"]
        .as_str()
        .filter(|v| text(v, 512))
        .ok_or("Static STAC directory has no valid ID")?;
    let title = value["title"]
        .as_str()
        .filter(|v| text(v, 512))
        .unwrap_or(id);
    let description = value["description"]
        .as_str()
        .filter(|v| v.len() <= 32768)
        .ok_or("Static STAC directory description is invalid")?;
    let license = if kind == "Collection" {
        Some(collection(value)?.license)
    } else {
        None
    };
    // Validate both link classes even when discovery only follows children.
    directory_links(value, "child", url)?;
    directory_links(value, "item", url)?;
    Ok(CatalogNode {
        key: hash(url.as_str().as_bytes()),
        id: id.into(),
        title: title.into(),
        kind: kind.into(),
        description: description.into(),
        license,
        url: url.to_string(),
        parent_key,
    })
}

fn chain<'a>(nodes: &'a [CatalogNode], key: &str) -> Result<Vec<&'a CatalogNode>> {
    let mut result = Vec::new();
    let mut current = Some(key);
    while let Some(key) = current {
        if result.len() > MAX_DEPTH || result.iter().any(|n: &&CatalogNode| n.key == key) {
            return Err("Static STAC directory is cyclic or exceeds 24 levels".into());
        }
        let n = nodes
            .iter()
            .find(|n| n.key == key)
            .ok_or("Static STAC parent directory is missing")?;
        result.push(n);
        current = n.parent_key.as_deref();
    }
    result.reverse();
    Ok(result)
}

pub(super) async fn discover(
    root: &Path,
    settings: &ProxySettings,
    c: &mut Connection,
) -> Result<()> {
    discover_with(root, c, |url| async move {
        fetch(settings, &public_url(&url)?).await
    })
    .await
}

async fn discover_with<F, Fut>(root: &Path, c: &mut Connection, mut get: F) -> Result<()>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>>>,
{
    let mut pending = VecDeque::from([(public_url(&c.url)?, None)]);
    let mut scheduled = BTreeSet::from([c.url.clone()]);
    let mut collection_ids = BTreeSet::new();
    let mut bytes_total = 0;
    while let Some((url, parent)) = pending.pop_front() {
        let bytes = get(url.to_string()).await?;
        bytes_total += bytes.len();
        if bytes_total > MAX_SEARCH_BYTES {
            return Err("Static STAC discovery exceeds 20 MiB; connect a smaller directory".into());
        }
        let value = json_document(&bytes)?;
        let entry = node(&value, &url, parent)?;
        if entry.kind == "Collection" && !collection_ids.insert(entry.id.clone()) {
            return Err("Static STAC repeats a Collection ID at different URLs".into());
        }
        let digest = immutable(root, "document", &bytes)?;
        c.metadata_sha256.push(digest.clone());
        c.metadata_documents.push(DocumentReceipt {
            url: url.to_string(),
            sha256: digest,
        });
        c.catalog_nodes.push(entry.clone());
        let ancestors = chain(&c.catalog_nodes, &entry.key)?;
        for child in directory_links(&value, "child", &url)? {
            if ancestors.iter().any(|n| n.url == child.as_str()) {
                return Err("Static STAC child links form a cycle".into());
            }
            if scheduled.insert(child.to_string()) {
                if scheduled.len() > MAX_NODES {
                    return Err("Static STAC discovery exceeds 128 directories; connect a smaller directory".into());
                }
                pending.push_back((child, Some(entry.key.clone())));
            } else {
                return Err(
                    "Static STAC child has multiple parents; connect that directory directly"
                        .into(),
                );
            }
        }
    }
    validate_connection(root, c)
}

pub(super) fn validate_connection(root: &Path, c: &Connection) -> Result<()> {
    if c.kind != "catalog"
        || c.capabilities.search_get
        || c.capabilities.search_post
        || c.search_url.is_some()
        || c.search_method != SearchMethod::Get
        || !c.collections.is_empty()
        || !c.snapshot_ids.is_empty()
        || c.catalog_nodes.is_empty()
        || c.catalog_nodes.len() > MAX_NODES
        || c.catalog_nodes.len() != c.metadata_documents.len()
        || c.metadata_documents
            .iter()
            .map(|r| r.sha256.clone())
            .collect::<Vec<_>>()
            != c.metadata_sha256
    {
        return Err("Static STAC connection has conflicting discovery metadata".into());
    }
    let mut expected = VecDeque::from([(c.url.clone(), None)]);
    let mut scheduled = BTreeSet::from([c.url.clone()]);
    let mut collections = BTreeSet::new();
    let mut total = 0;
    for (index, (entry, receipt)) in c
        .catalog_nodes
        .iter()
        .zip(&c.metadata_documents)
        .enumerate()
    {
        let (url, parent) = expected
            .pop_front()
            .ok_or("Static STAC contains an unlinked directory")?;
        if url != receipt.url {
            return Err("Static STAC directory receipt URL changed".into());
        }
        let bytes = document(root, &receipt.sha256)?;
        total += bytes.len();
        let value = json_document(&bytes)?;
        let base = public_url(&url)?;
        if *entry != node(&value, &base, parent)? {
            return Err("Static STAC directory differs from its original metadata".into());
        }
        if entry.kind == "Collection" && !collections.insert(entry.id.clone()) {
            return Err("Static STAC repeats a Collection ID".into());
        }
        let ancestors = chain(&c.catalog_nodes[..=index], &entry.key)?;
        for child in directory_links(&value, "child", &base)? {
            if ancestors.iter().any(|n| n.url == child.as_str()) {
                return Err("Static STAC child links form a cycle".into());
            }
            if scheduled.insert(child.to_string()) {
                expected.push_back((child.to_string(), Some(entry.key.clone())));
            } else {
                return Err("Static STAC child has multiple parents".into());
            }
        }
    }
    if !expected.is_empty() || total > MAX_SEARCH_BYTES {
        return Err("Static STAC discovery is incomplete or too large".into());
    }
    Ok(())
}

fn initial(root: &Path, c: &Connection, mut request: SearchRequest) -> Result<CatalogCursor> {
    validate_connection(root, c)?;
    if !c
        .catalog_nodes
        .iter()
        .any(|n| n.key == request.collection_id)
    {
        return Err("Choose a discovered static STAC directory".into());
    }
    request.cursor = None;
    let mut pending = VecDeque::new();
    let mut seen = BTreeSet::new();
    for n in &c.catalog_nodes {
        let ancestors = chain(&c.catalog_nodes, &n.key)?;
        if !ancestors.iter().any(|n| n.key == request.collection_id) {
            continue;
        }
        let receipts = ancestors
            .iter()
            .map(|n| {
                c.metadata_documents
                    .iter()
                    .find(|r| r.url == n.url)
                    .cloned()
                    .ok_or_else(|| "Static STAC directory receipt missing".to_string())
            })
            .collect::<Result<Vec<_>>>()?;
        let value = json_document(&document(root, &receipts.last().unwrap().sha256)?)?;
        for url in directory_links(&value, "item", &public_url(&n.url)?)? {
            if seen.insert(url.to_string()) {
                if seen.len() > MAX_PENDING {
                    return Err(
                        "Static STAC contains over 10000 item links; choose a smaller directory"
                            .into(),
                    );
                }
                pending.push_back(Target {
                    url: url.to_string(),
                    receipts: receipts.clone(),
                });
            }
        }
    }
    Ok(CatalogCursor {
        request,
        pending,
        identities: BTreeSet::new(),
        scanned: 0,
        bytes: 0,
    })
}

fn matches_filter(snapshot: &ItemSnapshot, request: &SearchRequest) -> Result<bool> {
    let Some(bbox) = snapshot.bbox else {
        return Ok(false);
    };
    let b = request.bounds;
    if bbox[0] > b[2] || bbox[2] < b[0] || bbox[1] > b[3] || bbox[3] < b[1] {
        return Ok(false);
    }
    let Some(filter) = &request.datetime else {
        return Ok(true);
    };
    validate_datetime(filter)?;
    let parts = filter.split('/').collect::<Vec<_>>();
    let parse = |s: &str| chrono::DateTime::parse_from_rfc3339(s).map_err(io_error);
    let from = if parts[0] == ".." {
        None
    } else {
        Some(parse(parts[0])?)
    };
    let to_str = if parts.len() == 1 { parts[0] } else { parts[1] };
    let to = if to_str == ".." {
        None
    } else {
        Some(parse(to_str)?)
    };
    let (start, end) = if let Some(datetime) = &snapshot.datetime {
        (parse(datetime)?, parse(datetime)?)
    } else if let (Some(start), Some(end)) = (&snapshot.start_datetime, &snapshot.end_datetime) {
        (parse(start)?, parse(end)?)
    } else {
        return Ok(false);
    };
    Ok(from.is_none_or(|from| end >= from) && to.is_none_or(|to| start <= to))
}

// Replaying archived child/item links makes the local branch selection auditable
// after restart and after a connection is forgotten. No registry trust is needed.
pub(super) fn validate_record(root: &Path, r: &Record, snapshot: &ItemSnapshot) -> Result<()> {
    let request = r
        .search
        .as_ref()
        .ok_or("Static STAC snapshot has no local filter receipt")?;
    if r.document_request.is_some()
        || r.item_index.is_some()
        || r.metadata_documents.is_empty()
        || r.metadata_documents.len() > MAX_DEPTH + 2
        || request.cursor.is_some()
        || request.connection_id != r.connection_id
        || !digest(&request.collection_id)
    {
        return Err("Static STAC snapshot has conflicting query provenance".into());
    }
    let mut branch_found = false;
    let mut previous: Option<(Url, Value)> = None;
    let mut seen = BTreeSet::new();
    let raw_item = json_document(&document(root, &r.document_sha256)?)?;
    let collection_url = link(&raw_item, "collection", &public_url(&r.document_url)?)?;
    for (index, receipt) in r.metadata_documents.iter().enumerate() {
        let url = public_url(&receipt.url)?;
        if !seen.insert(receipt.url.clone()) {
            return Err("Static STAC snapshot repeats a directory".into());
        }
        let value = json_document(&document(root, &receipt.sha256)?)?;
        node(&value, &url, None)?;
        if let Some((base, parent)) = &previous {
            if !directory_links(parent, "child", base)?.contains(&url) {
                if index + 1 == r.metadata_documents.len()
                    && collection_url.as_ref() == Some(&url)
                    && value["type"] == "Collection"
                    && value["id"].as_str() == snapshot.collection_id.as_deref()
                {
                    continue;
                }
                return Err("Static STAC snapshot contains an unlinked parent directory".into());
            }
        }
        branch_found |= hash(url.as_str().as_bytes()) == request.collection_id;
        previous = Some((url, value));
    }
    let (base, parent) = previous.unwrap();
    if !branch_found
        || !directory_links(&parent, "item", &base)?.contains(&public_url(&r.document_url)?)
    {
        return Err("Static STAC item is outside the selected directory".into());
    }
    if snapshot.collection_id.is_some() && snapshot.provenance.collection.is_none() {
        return Err("Static STAC item's declared Collection metadata is missing".into());
    }
    if !matches_filter(snapshot, request)? {
        return Err("Static STAC item does not match its saved local filter".into());
    }
    Ok(())
}

async fn scan_with<F, Fut>(
    root: &Path,
    c: &Connection,
    cursor: &mut CatalogCursor,
    limit: usize,
    mut get: F,
) -> Result<Vec<ItemSnapshot>>
where
    F: FnMut(String) -> Fut,
    Fut: Future<Output = Result<Vec<u8>>>,
{
    let start = std::time::Instant::now();
    let mut items = Vec::new();
    let mut page_scanned = 0;
    while !cursor.pending.is_empty()
        && items.len() < limit
        && page_scanned < PAGE_SCAN
        && cursor.scanned < MAX_ITEMS
        && cursor.bytes < MAX_SEARCH_BYTES
        && start.elapsed() < Duration::from_secs(30)
    {
        let target = cursor.pending.pop_front().unwrap();
        let bytes = get(target.url.clone()).await?;
        cursor.bytes += bytes.len();
        cursor.scanned += 1;
        page_scanned += 1;
        if cursor.bytes > MAX_SEARCH_BYTES {
            return Err("Static STAC search exceeds 20 MiB; choose a smaller directory".into());
        }
        let value = json_document(&bytes)?;
        let sha = immutable(root, "document", &bytes)?;
        let mut r = Record {
            version: 1,
            connection_id: c.id.clone(),
            service_name: c.name.clone(),
            kind: "catalog".into(),
            document_url: target.url,
            document_request: None,
            document_sha256: sha,
            item_index: None,
            retrieved_at: now(),
            metadata_documents: target.receipts,
            search: Some(cursor.request.clone()),
        };
        let snapshot = item(
            &r,
            &hash(&serde_json::to_vec(&r).map_err(io_error)?),
            &value,
        )?;
        if !cursor.identities.insert(
            serde_json::to_string(&(&snapshot.collection_id, &snapshot.item_id))
                .map_err(io_error)?,
        ) {
            return Err("Static STAC has duplicate item identities at different URLs".into());
        }
        if matches_filter(&snapshot, &cursor.request)? {
            if let Some(collection_url) = link(&value, "collection", &public_url(&r.document_url)?)?
            {
                if !r
                    .metadata_documents
                    .iter()
                    .any(|receipt| receipt.url == collection_url.as_str())
                {
                    let bytes = get(collection_url.to_string()).await?;
                    cursor.bytes += bytes.len();
                    if cursor.bytes > MAX_SEARCH_BYTES {
                        return Err("Static STAC search exceeds 20 MiB".into());
                    }
                    let declaration = json_document(&bytes)?;
                    if Some(collection(&declaration)?.id.as_str())
                        != snapshot.collection_id.as_deref()
                    {
                        return Err(
                            "Static STAC item's Collection link has a different identity".into(),
                        );
                    }
                    r.metadata_documents.push(DocumentReceipt {
                        url: collection_url.to_string(),
                        sha256: immutable(root, "document", &bytes)?,
                    });
                }
            }
            items.push(save_snapshot(root, r, &value)?);
        }
    }
    Ok(items)
}

impl JobManager {
    pub(super) async fn search_stac_catalog(&self, request: SearchRequest) -> Result<SearchPage> {
        let limit = request.limit.unwrap_or(100);
        if !(1..=100).contains(&limit) {
            return Err("STAC page size must be between 1 and 100".into());
        }
        if let Some(datetime) = &request.datetime {
            validate_datetime(datetime)?;
        }
        let (c, mut cursor) = {
            let registry = self.inner.stac.lock().await;
            let c = registry
                .connections
                .get(&request.connection_id)
                .cloned()
                .ok_or("Unknown STAC connection")?;
            if c.kind != "catalog" {
                return Err("STAC connection changed; restart the search".into());
            }
            let cursor = if let Some(id) = &request.cursor {
                let cursor = registry
                    .catalog_cursors
                    .get(id)
                    .cloned()
                    .ok_or("Static STAC cursor expired; restart the search")?;
                let mut expected = request.clone();
                expected.cursor = None;
                if cursor.request != expected {
                    return Err("Static STAC cursor belongs to different search filters".into());
                }
                cursor
            } else {
                initial(&self.inner.root, &c, request.clone())?
            };
            (c, cursor)
        };
        let settings = self.proxy_settings().await;
        let settings = &settings;
        let items = scan_with(&self.inner.root, &c, &mut cursor, limit, |url| async move {
            tokio::time::timeout(Duration::from_secs(30), fetch(settings, &public_url(&url)?))
                .await
                .map_err(|_| "Static STAC item request timed out")?
        })
        .await?;
        let complete = cursor.pending.is_empty();
        let limit_reached =
            !complete && (cursor.scanned >= MAX_ITEMS || cursor.bytes >= MAX_SEARCH_BYTES);
        let next_cursor = (!complete && !limit_reached).then(|| Uuid::new_v4().to_string());
        let scanned_items = Some(cursor.scanned);
        let mut registry = self.inner.stac.lock().await;
        if registry.connections.get(&c.id) != Some(&c) {
            return Err("STAC connection changed while scanning the directory".into());
        }
        if let Some(previous) = &request.cursor {
            if registry.catalog_cursors.remove(previous).is_none() {
                return Err("Static STAC cursor was already consumed; restart the search".into());
            }
        }
        if let Some(id) = &next_cursor {
            if registry.catalog_cursors.len() >= 64 {
                registry.catalog_cursors.clear();
            }
            registry.catalog_cursors.insert(id.clone(), cursor);
        }
        Ok(SearchPage {
            items,
            next_cursor,
            complete,
            limit_reached,
            scanned_items,
        })
    }
}

#[cfg(test)]
mod tests;
