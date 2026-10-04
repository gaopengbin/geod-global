//! Managed explicit 3D Tiles / glTF packages. Original bytes and their source
//! declarations remain separate from the localized files used for rendering.
mod format;
#[cfg(test)]
mod tests;
use crate::{features, io_error, now, storage, JobManager, Result};
use base64::{engine::general_purpose::STANDARD, Engine};
pub use format::{Kind, Purpose, Reference};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{Cursor, Read, Write},
    path::{Path, PathBuf},
    time::Duration,
};
use url::Url;
use uuid::Uuid;
pub const MAX_RESOURCE: usize = 32 * 1024 * 1024;
pub const MAX_PACKAGE: usize = 128 * 1024 * 1024;
pub const MAX_RESOURCES: usize = 256;
const MAX_REGISTRY: usize = 16 * 1024 * 1024;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Rights {
    pub license: String,
    pub attribution: String,
    pub license_url: Option<String>,
    pub permission_confirmed: bool,
}
impl Rights {
    fn validate(&self) -> Result<()> {
        text(&self.license, 120)?;
        text(&self.attribution, 1000)?;
        if !self.permission_confirmed {
            return Err("Confirm that the source permits saving and using these 3D assets".into());
        }
        if let Some(u) = &self.license_url {
            safe_url(u)?;
        }
        Ok(())
    }
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiscoverRequest {
    pub url: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Discovery {
    pub url: String,
    pub sha256: String,
    pub bytes: u64,
    pub kind: Kind,
    pub direct_dependencies: usize,
    pub tile_count: usize,
    pub asset: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AcquireRequest {
    pub url: String,
    pub discovery_sha256: String,
    pub name: String,
    pub rights: Rights,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalRequest {
    pub name: String,
    pub rights: Rights,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Link {
    pub reference: Reference,
    pub target: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Resource {
    pub id: String,
    pub locator: String,
    pub sha256: String,
    pub bytes: u64,
    pub kind: Kind,
    pub links: Vec<Link>,
    pub etag: Option<String>,
    pub last_modified: Option<String>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ImportReceipt {
    pub source_receipt_sha256: String,
    pub source: String,
    pub origin: String,
    pub rights: Rights,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previous: Option<Box<ImportReceipt>>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceOrigin {
    pub origin: String,
    pub source: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Package {
    pub id: String,
    pub name: String,
    pub created_at: String,
    pub origin: String,
    pub source: String,
    pub rights: Rights,
    pub entry: String,
    pub bytes: u64,
    pub tile_count: usize,
    pub resources: Vec<Resource>,
    pub receipt_sha256: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub imported_from: Option<ImportReceipt>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resource_origin: Option<SourceOrigin>,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceRequest {
    pub id: String,
    pub resource_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResourceData {
    pub resource: Resource,
    pub data_base64: String,
}
#[derive(Default, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Registry {
    pub packages: BTreeMap<String, Package>,
}
fn hash(b: &[u8]) -> String {
    format!("{:x}", Sha256::digest(b))
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
fn text(s: &str, n: usize) -> Result<()> {
    if s.trim() != s || s.is_empty() || s.chars().count() > n || s.chars().any(char::is_control) {
        Err("Invalid 3D asset name or source declaration".into())
    } else {
        Ok(())
    }
}
fn safe_url(raw: &str) -> Result<Url> {
    let u = features::public_url(raw)?;
    if u.query().is_some() {
        return Err("3D sources must use public HTTPS URLs without query credentials".into());
    }
    Ok(u)
}
fn remote_link(base: &Url, raw: &str, root: &Url) -> Result<String> {
    let u = safe_url(base.join(raw).map_err(io_error)?.as_str())?;
    if u.origin() != root.origin() {
        return Err("3D dependencies must stay on the source's HTTPS origin".into());
    }
    Ok(u.into())
}
fn directory(root: &Path) -> Result<PathBuf> {
    let p = root.join("three-d");
    if std::fs::canonicalize(&p).map_err(io_error)? != p {
        return Err("Managed 3D storage cannot be redirected".into());
    }
    Ok(p)
}
fn resource_path(root: &Path, r: &Resource) -> Result<PathBuf> {
    if !digest(&r.sha256) {
        return Err("Invalid 3D resource digest".into());
    }
    Ok(directory(root)?.join(format!("{}.{}", r.sha256, r.kind.extension())))
}
fn receipt(p: &Package) -> Result<String> {
    let mut value = p.clone();
    value.receipt_sha256.clear();
    Ok(hash(&serde_json::to_vec(&value).map_err(io_error)?))
}
fn validate_import(upstream: &ImportReceipt, depth: usize) -> Result<()> {
    if depth > 8
        || !digest(&upstream.source_receipt_sha256)
        || !matches!(
            upstream.origin.as_str(),
            "public-https" | "local-files" | "local-archive"
        )
    {
        return Err("Invalid or overly deep 3D import provenance".into());
    }
    upstream.rights.validate()?;
    if upstream.origin == "public-https" {
        safe_url(&upstream.source)?;
    } else {
        text(&upstream.source, 240)?;
    }
    if let Some(previous) = &upstream.previous {
        validate_import(previous, depth + 1)?;
    }
    Ok(())
}
fn dependency_source(p: &Package) -> (&str, &str) {
    p.resource_origin
        .as_ref()
        .map(|s| (s.origin.as_str(), s.source.as_str()))
        .unwrap_or((&p.origin, &p.source))
}
fn validate(p: &Package) -> Result<()> {
    if Uuid::parse_str(&p.id).is_err()
        || chrono::DateTime::parse_from_rfc3339(&p.created_at).is_err()
        || p.resources.is_empty()
        || p.resources.len() > MAX_RESOURCES
        || p.bytes == 0
        || p.bytes > MAX_PACKAGE as u64
        || p.tile_count > 4096 * MAX_RESOURCES
        || !digest(&p.entry)
        || !digest(&p.receipt_sha256)
        || !matches!(
            p.origin.as_str(),
            "public-https" | "local-files" | "local-archive"
        )
    {
        return Err("Invalid saved 3D package".into());
    }
    text(&p.name, 120)?;
    p.rights.validate()?;
    if let Some(upstream) = &p.imported_from {
        if p.origin != "local-archive" {
            return Err("Invalid 3D import provenance".into());
        }
        validate_import(upstream, 1)?;
    }
    if let Some(s) = &p.resource_origin {
        if p.origin != "local-archive"
            || p.imported_from.is_none()
            || !matches!(
                s.origin.as_str(),
                "public-https" | "local-files" | "local-archive"
            )
        {
            return Err("Invalid 3D resource origin".into());
        }
        if s.origin == "public-https" {
            safe_url(&s.source)?;
        } else {
            text(&s.source, 240)?;
        }
    }
    if p.origin == "public-https" {
        safe_url(&p.source)?;
    } else {
        text(&p.source, 240)?;
    }
    let ids: BTreeSet<_> = p.resources.iter().map(|r| r.id.as_str()).collect();
    if ids.len() != p.resources.len()
        || !ids.contains(p.entry.as_str())
        || p.resources
            .iter()
            .try_fold(0u64, |total, r| total.checked_add(r.bytes))
            != Some(p.bytes)
    {
        return Err("Saved 3D membership changed".into());
    }
    let entry = p.resources.iter().find(|r| r.id == p.entry).unwrap();
    if !matches!(entry.kind, Kind::Tileset | Kind::Gltf | Kind::Glb) {
        return Err("A standalone b3dm tile needs its tileset transform and bounds".into());
    }
    let (graph_origin, graph_source) = dependency_source(p);
    for r in &p.resources {
        if !digest(&r.id)
            || !digest(&r.sha256)
            || r.bytes == 0
            || r.bytes > MAX_RESOURCE as u64
            || r.links.len() > MAX_RESOURCES
            || hash(r.locator.as_bytes()) != r.id
        {
            return Err("Invalid saved 3D dependency receipt".into());
        }
        if graph_origin == "public-https" {
            remote_link(&safe_url(&r.locator)?, "", &safe_url(graph_source)?)?;
        } else {
            archive_key(&r.locator)?;
        }
        for l in &r.links {
            if !ids.contains(l.target.as_str())
                || l.reference.pointer.len() > 1024
                || l.reference.uri.len() > 2048
            {
                return Err("Saved 3D dependency is missing".into());
            }
            let locator = if graph_origin == "public-https" {
                remote_link(
                    &safe_url(&r.locator)?,
                    &l.reference.uri,
                    &safe_url(graph_source)?,
                )?
            } else {
                local_link(&r.locator, &l.reference.uri)?
            };
            let target = p
                .resources
                .iter()
                .find(|resource| resource.id == l.target)
                .unwrap();
            if hash(locator.as_bytes()) != l.target
                || !purpose_matches(l.reference.purpose, target.kind)
                || target.bytes < l.reference.min_bytes
            {
                return Err("Saved 3D dependency target changed".into());
            }
        }
    }
    graph_order(p)?;
    if receipt(p)? != p.receipt_sha256 {
        return Err("Saved 3D source receipt changed".into());
    }
    Ok(())
}
pub(crate) async fn load(root: &Path) -> Result<Registry> {
    tokio::fs::create_dir_all(root.join("three-d"))
        .await
        .map_err(io_error)?;
    directory(root)?;
    let path = root.join("three-d.json");
    let registry = if path.exists() {
        storage::regular_file(&path)?;
        if std::fs::metadata(&path).map_err(io_error)?.len() > MAX_REGISTRY as u64 {
            return Err("Saved 3D registry exceeds its limit".into());
        }
        let b = tokio::fs::read(path).await.map_err(io_error)?;
        if b.len() > MAX_REGISTRY {
            return Err("Saved 3D registry exceeds its limit".into());
        }
        serde_json::from_slice::<Registry>(&b).map_err(io_error)?
    } else {
        Registry::default()
    };
    if registry.packages.len() > 128 {
        return Err("Too many stored 3D packages".into());
    }
    for (id, p) in &registry.packages {
        if id != &p.id {
            return Err("3D registry identifier mismatch".into());
        }
        validate(p)?;
    }
    Ok(registry)
}
fn persist(root: &Path, r: &Registry) -> Result<()> {
    let b = serde_json::to_vec_pretty(r).map_err(io_error)?;
    if b.len() > MAX_REGISTRY {
        return Err("3D registry exceeds its storage limit".into());
    }
    let mut tmp = tempfile::NamedTempFile::new_in(root).map_err(io_error)?;
    tmp.write_all(&b).map_err(io_error)?;
    tmp.as_file().sync_all().map_err(io_error)?;
    tmp.persist(root.join("three-d.json")).map_err(io_error)?;
    Ok(())
}
async fn fetch(
    client: &reqwest::Client,
    url: &str,
    budget: usize,
) -> Result<(Vec<u8>, Option<String>, Option<String>)> {
    let mut response = client
        .get(url)
        .header("Accept-Encoding", "identity")
        .send()
        .await
        .map_err(|_| "Cannot reach 3D source; check the source and proxy settings")?;
    if !response.status().is_success() {
        return Err(format!(
            "3D dependency returned HTTP {}",
            response.status().as_u16()
        ));
    }
    if response.url().as_str() != url || response.headers().contains_key("content-encoding") {
        return Err("3D redirects or encoded transfer bodies are unsupported".into());
    }
    let total = response.content_length();
    if total.is_some_and(|n| n > budget.min(MAX_RESOURCE) as u64) {
        return Err("3D resource exceeds the package limit".into());
    }
    let header = |key: &str| {
        response
            .headers()
            .get(key)
            .and_then(|v| v.to_str().ok())
            .filter(|s| s.len() <= 256 && !s.chars().any(char::is_control))
            .map(str::to_owned)
    };
    let etag = header("etag");
    let modified = header("last-modified");
    let mut b = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| "3D transfer was interrupted")?
    {
        if b.len() + chunk.len() > budget.min(MAX_RESOURCE) {
            return Err("3D resource exceeds the package limit".into());
        }
        b.extend(chunk);
    }
    if total.is_some_and(|n| n != b.len() as u64) {
        return Err("3D transfer length mismatch".into());
    }
    Ok((b, etag, modified))
}
fn archive_key(raw: &str) -> Result<String> {
    if raw.is_empty()
        || raw.len() > 2048
        || raw.starts_with('/')
        || raw.contains(['\\', ':', '?', '#', '%'])
        || raw.chars().any(char::is_control)
        || raw
            .split('/')
            .any(|p| p.is_empty() || p == "." || p == "..")
    {
        return Err("Unsafe local 3D dependency path".into());
    }
    Ok(raw.into())
}
fn local_link(base: &str, raw: &str) -> Result<String> {
    if raw.contains(['\\', ':', '?', '#', '%']) || raw.starts_with('/') || raw.len() > 2048 {
        return Err("Local 3D assets cannot reference network or absolute paths".into());
    }
    let mut parts: Vec<_> = base.split('/').collect();
    parts.pop();
    for p in raw.split('/') {
        match p {
            "" | "." => {}
            ".." => {
                if parts.pop().is_none() {
                    return Err("Local 3D dependency escapes the selected folder".into());
                }
            }
            p => parts.push(p),
        }
    }
    archive_key(&parts.join("/"))
}
enum Input {
    Remote { root: Url, client: reqwest::Client },
    Folder { root: PathBuf },
}
impl Input {
    fn resolve(&self, base: &str, uri: &str) -> Result<String> {
        match self {
            Self::Remote { root, .. } => remote_link(&safe_url(base)?, uri, root),
            _ => local_link(base, uri),
        }
    }
    async fn bytes(
        &self,
        key: &str,
        budget: usize,
    ) -> Result<(Vec<u8>, Option<String>, Option<String>)> {
        match self {
            Self::Remote { client, .. } => fetch(client, key, budget).await,
            Self::Folder { root } => {
                archive_key(key)?;
                let path = root.join(key);
                let resolved = storage::regular_file(&path)?;
                if !resolved.starts_with(root) {
                    return Err("Local 3D dependency escapes the selected folder".into());
                }
                let n = std::fs::metadata(&resolved).map_err(io_error)?.len();
                if n > budget.min(MAX_RESOURCE) as u64 {
                    return Err("Local 3D resource exceeds the package limit".into());
                }
                Ok((
                    tokio::fs::read(resolved).await.map_err(io_error)?,
                    None,
                    None,
                ))
            }
        }
    }
}
fn graph_order(p: &Package) -> Result<Vec<String>> {
    fn visit(
        id: &str,
        all: &BTreeMap<&str, &Resource>,
        path: &mut BTreeSet<String>,
        done: &mut BTreeSet<String>,
        order: &mut Vec<String>,
        depth: usize,
    ) -> Result<()> {
        if depth > 32 {
            return Err("3D dependency graph exceeds 32 levels".into());
        }
        if done.contains(id) {
            return Ok(());
        }
        if !path.insert(id.into()) {
            return Err("Cyclic 3D dependencies are unsupported".into());
        }
        for l in &all.get(id).ok_or("Missing 3D dependency")?.links {
            visit(&l.target, all, path, done, order, depth + 1)?;
        }
        path.remove(id);
        done.insert(id.into());
        order.push(id.into());
        Ok(())
    }
    let all = p.resources.iter().map(|r| (r.id.as_str(), r)).collect();
    let mut order = Vec::new();
    visit(
        &p.entry,
        &all,
        &mut BTreeSet::new(),
        &mut BTreeSet::new(),
        &mut order,
        0,
    )?;
    if order.len() != p.resources.len() {
        return Err("3D package contains unreferenced resources".into());
    }
    Ok(order)
}
async fn collect(
    input: Input,
    entry: String,
    name: String,
    rights: Rights,
    origin: &str,
    source: String,
    pin: Option<&str>,
) -> Result<(Package, BTreeMap<String, Vec<u8>>)> {
    text(&name, 120)?;
    rights.validate()?;
    let mut pending = VecDeque::from([(entry.clone(), Purpose::Content, 0u64, 0usize)]);
    let mut resources = BTreeMap::<String, Resource>::new();
    let mut data = BTreeMap::new();
    let mut total = 0usize;
    let mut tiles = 0;
    while let Some((key, purpose, min, depth)) = pending.pop_front() {
        if depth > 32 {
            return Err("3D dependency graph exceeds 32 levels".into());
        }
        let id = hash(key.as_bytes());
        if let Some(r) = resources.get(&id) {
            if r.bytes < min || !purpose_matches(purpose, r.kind) {
                return Err("3D dependency has conflicting types or lengths".into());
            }
            continue;
        }
        if resources.len() >= MAX_RESOURCES {
            return Err("3D package exceeds 256 resources".into());
        }
        let (b, etag, last_modified) = input.bytes(&key, MAX_PACKAGE - total).await?;
        if (b.len() as u64) < min {
            return Err("glTF external buffer is shorter than declared".into());
        }
        let sha = hash(&b);
        if key == entry && pin.is_some_and(|s| s != sha) {
            return Err("3D source changed after discovery; inspect it again".into());
        }
        let a = format::analyze(&b, purpose)?;
        if key == entry && a.kind == Kind::B3dm {
            return Err(
                "Choose a tileset, glTF or GLB entry; a b3dm tile needs its parent tileset".into(),
            );
        }
        tiles += a.tile_count;
        let mut links = Vec::new();
        for r in a.references {
            let target = input.resolve(&key, &r.uri)?;
            links.push(Link {
                reference: r.clone(),
                target: hash(target.as_bytes()),
            });
            pending.push_back((target, r.purpose, r.min_bytes, depth + 1));
        }
        total += b.len();
        let bytes = b.len() as u64;
        data.insert(id.clone(), b);
        resources.insert(
            id.clone(),
            Resource {
                id: id.clone(),
                locator: key,
                sha256: sha,
                bytes,
                kind: a.kind,
                links,
                etag,
                last_modified,
            },
        );
        // The original length belongs to this locator even when identical bytes
        // are also referenced at another address.
        let r = resources.get_mut(&id).unwrap();
        r.bytes = data[&id].len() as u64;
    }
    let mut p = Package {
        id: Uuid::new_v4().to_string(),
        name,
        created_at: now(),
        origin: origin.into(),
        source,
        rights,
        entry: hash(entry.as_bytes()),
        bytes: total as u64,
        tile_count: tiles,
        resources: resources.into_values().collect(),
        receipt_sha256: String::new(),
        imported_from: None,
        resource_origin: None,
    };
    p.receipt_sha256 = receipt(&p)?;
    validate(&p)?;
    Ok((p, data))
}
fn purpose_matches(p: Purpose, k: Kind) -> bool {
    match p {
        Purpose::Content => matches!(k, Kind::Tileset | Kind::Gltf | Kind::Glb | Kind::B3dm),
        Purpose::Buffer => k == Kind::Buffer,
        Purpose::Image => matches!(k, Kind::Png | Kind::Jpeg),
        Purpose::Schema => k == Kind::Schema,
    }
}
fn purpose_for_kind(kind: Kind) -> Purpose {
    match kind {
        Kind::Buffer => Purpose::Buffer,
        Kind::Png | Kind::Jpeg => Purpose::Image,
        Kind::Schema => Purpose::Schema,
        _ => Purpose::Content,
    }
}
fn read_verified(root: &Path, r: &Resource) -> Result<Vec<u8>> {
    let p = storage::regular_file(&resource_path(root, r)?)?;
    let mut b = Vec::new();
    std::fs::File::open(p)
        .map_err(io_error)?
        .take(MAX_RESOURCE as u64 + 1)
        .read_to_end(&mut b)
        .map_err(io_error)?;
    if b.len() as u64 != r.bytes || hash(&b) != r.sha256 {
        return Err("Saved 3D resource failed its SHA-256 or size check".into());
    }
    let a = format::analyze(
        &b,
        match r.kind {
            Kind::Buffer => Purpose::Buffer,
            Kind::Png | Kind::Jpeg => Purpose::Image,
            Kind::Schema => Purpose::Schema,
            _ => Purpose::Content,
        },
    )?;
    if a.kind != r.kind
        || a.references
            != r.links
                .iter()
                .map(|l| l.reference.clone())
                .collect::<Vec<_>>()
    {
        return Err("Saved 3D dependency structure changed".into());
    }
    Ok(b)
}
impl JobManager {
    pub async fn discover_three_d(&self, request: DiscoverRequest) -> Result<Discovery> {
        let url = safe_url(&request.url)?;
        let settings = self.inner.proxy_settings.lock().await.clone();
        let client = features::client(&url, &settings).await?;
        let (b, _, _) = fetch(&client, url.as_str(), MAX_RESOURCE).await?;
        let a = format::analyze(&b, Purpose::Content)?;
        if a.kind == Kind::B3dm {
            return Err(
                "Choose a tileset, glTF or GLB entry; a b3dm tile needs its parent tileset".into(),
            );
        }
        Ok(Discovery {
            url: url.into(),
            sha256: hash(&b),
            bytes: b.len() as u64,
            kind: a.kind,
            direct_dependencies: a.references.len(),
            tile_count: a.tile_count,
            asset: a
                .document
                .map_or(serde_json::Value::Null, |v| v["asset"].clone()),
        })
    }
    pub async fn list_three_d(&self) -> Vec<Package> {
        self.inner
            .three_d
            .lock()
            .await
            .packages
            .values()
            .cloned()
            .collect()
    }
    async fn save_three_d(&self, p: Package, data: BTreeMap<String, Vec<u8>>) -> Result<Package> {
        let mut registry = self.inner.three_d.lock().await;
        let mut next = registry.clone();
        if next.packages.len() >= 128 {
            return Err("3D library is full".into());
        }
        for r in &p.resources {
            let path = resource_path(&self.inner.root, r)?;
            if path.exists() {
                read_verified(&self.inner.root, r)?;
            } else {
                let mut file = tempfile::NamedTempFile::new_in(directory(&self.inner.root)?)
                    .map_err(io_error)?;
                file.write_all(&data[&r.id]).map_err(io_error)?;
                file.as_file().sync_all().map_err(io_error)?;
                file.persist_noclobber(path).map_err(io_error)?;
            }
        }
        next.packages.insert(p.id.clone(), p.clone());
        persist(&self.inner.root, &next)?;
        *registry = next;
        Ok(p)
    }
    pub async fn acquire_three_d(&self, r: AcquireRequest) -> Result<Package> {
        text(&r.name, 120)?;
        r.rights.validate()?;
        if !digest(&r.discovery_sha256) {
            return Err("Inspect the 3D source before saving it".into());
        }
        let root = safe_url(&r.url)?;
        let settings = self.inner.proxy_settings.lock().await.clone();
        let client =
            features::client_with_timeout(&root, &settings, Duration::from_secs(30)).await?;
        let (p, data) = tokio::time::timeout(
            Duration::from_secs(180),
            collect(
                Input::Remote {
                    root: root.clone(),
                    client,
                },
                root.to_string(),
                r.name,
                r.rights,
                "public-https",
                root.to_string(),
                Some(&r.discovery_sha256),
            ),
        )
        .await
        .map_err(|_| "3D acquisition exceeded three minutes; no complete package was saved")??;
        self.save_three_d(p, data).await
    }
    pub async fn open_three_d_path(&self, path: PathBuf, r: LocalRequest) -> Result<Package> {
        let path = storage::regular_file(&path)?;
        let parent = path
            .parent()
            .ok_or("3D entry has no parent folder")?
            .to_owned();
        let entry = path
            .file_name()
            .and_then(|p| p.to_str())
            .ok_or("Invalid 3D entry name")?
            .to_owned();
        if entry.to_lowercase().ends_with(".zip") {
            let n = std::fs::metadata(&path).map_err(io_error)?.len();
            if n > MAX_PACKAGE as u64 * 2 + MAX_REGISTRY as u64 {
                return Err("3D archive is too large".into());
            }
            return self
                .import_three_d_archive(tokio::fs::read(path).await.map_err(io_error)?, r)
                .await;
        }
        let (p, data) = collect(
            Input::Folder { root: parent },
            entry.clone(),
            r.name,
            r.rights,
            "local-files",
            entry,
            None,
        )
        .await?;
        self.save_three_d(p, data).await
    }
    pub async fn import_three_d_archive(&self, b: Vec<u8>, r: LocalRequest) -> Result<Package> {
        text(&r.name, 120)?;
        r.rights.validate()?;
        if b.len() > MAX_PACKAGE * 2 + MAX_REGISTRY {
            return Err("3D archive is too large".into());
        }
        let mut zip =
            zip::ZipArchive::new(Cursor::new(b)).map_err(|_| "Choose a GeoD 3D export ZIP")?;
        if zip.len() > MAX_RESOURCES * 2 + 2 {
            return Err("3D archive has too many members".into());
        }
        let mut members = BTreeMap::new();
        let mut total = 0usize;
        for i in 0..zip.len() {
            let mut f = zip.by_index(i).map_err(io_error)?;
            let key = archive_key(f.name())?;
            if f.is_dir() || f.unix_mode().is_some_and(|n| n & 0o170000 == 0o120000) {
                return Err("3D archives cannot contain folders or links".into());
            }
            let limit = if key == "manifest.json" {
                MAX_REGISTRY
            } else {
                MAX_RESOURCE
            };
            if f.size() > limit as u64
                || total as u64 + f.size() > (MAX_PACKAGE * 2 + MAX_REGISTRY) as u64
            {
                return Err("3D archive exceeds its resource limit".into());
            }
            let mut bytes = Vec::new();
            f.by_ref()
                .take(limit as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            total += bytes.len();
            if bytes.len() > limit
                || total > MAX_PACKAGE * 2 + MAX_REGISTRY
                || members.insert(key, bytes).is_some()
            {
                return Err("Duplicate or oversized 3D archive member".into());
            }
        }
        let manifest = members
            .remove("manifest.json")
            .ok_or("GeoD 3D export has no manifest")?;
        let original: Package = serde_json::from_slice(&manifest).map_err(io_error)?;
        validate(&original)?;
        let mut data = BTreeMap::new();
        let mut tiles = 0usize;
        for resource in &original.resources {
            let name = format!("{}.{}", resource.id, resource.kind.extension());
            let bytes = members
                .remove(&format!("originals/{name}"))
                .ok_or("3D archive original is missing")?;
            if bytes.len() as u64 != resource.bytes || hash(&bytes) != resource.sha256 {
                return Err("3D archive original failed its SHA-256 or size check".into());
            }
            let analysis = format::analyze(&bytes, purpose_for_kind(resource.kind))?;
            tiles += analysis.tile_count;
            if analysis.kind != resource.kind
                || analysis.references
                    != resource
                        .links
                        .iter()
                        .map(|l| l.reference.clone())
                        .collect::<Vec<_>>()
            {
                return Err("3D archive original dependency structure changed".into());
            }
            let replacements = resource
                .links
                .iter()
                .map(|l| {
                    let target = original
                        .resources
                        .iter()
                        .find(|r| r.id == l.target)
                        .unwrap();
                    (
                        l.reference.pointer.clone(),
                        format!("{}.{}", target.id, target.kind.extension()),
                    )
                })
                .collect::<Vec<_>>();
            let expected = format::rewrite(&bytes, resource.kind, &replacements)?;
            let localized = members
                .remove(&format!("scene/{name}"))
                .ok_or("3D archive localized dependency is missing")?;
            if expected != localized {
                return Err("3D archive localized scene differs from its source receipt".into());
            }
            data.insert(resource.id.clone(), bytes);
        }
        if !members.is_empty() {
            return Err("3D archive contains unreferenced members".into());
        }
        if tiles != original.tile_count {
            return Err("3D archive tile count differs from its source receipt".into());
        }
        let source_origin = original.resource_origin.clone().unwrap_or(SourceOrigin {
            origin: original.origin.clone(),
            source: original.source.clone(),
        });
        let upstream = ImportReceipt {
            source_receipt_sha256: original.receipt_sha256.clone(),
            source: original.source.clone(),
            origin: original.origin.clone(),
            rights: original.rights.clone(),
            previous: original.imported_from.clone().map(Box::new),
        };
        // Import verifies both exported views, but saves the unchanged originals.
        // Viewer/export localization continues to use their exact source graph.
        let mut p = original;
        p.id = Uuid::new_v4().to_string();
        p.created_at = chrono::Utc::now().to_rfc3339();
        p.name = r.name;
        p.rights = r.rights;
        p.origin = "local-archive".into();
        p.source = "GeoD 3D export.zip".into();
        p.resource_origin = Some(source_origin);
        p.imported_from = Some(upstream);
        p.receipt_sha256 = receipt(&p)?;
        validate(&p)?;
        self.save_three_d(p, data).await
    }
    pub async fn inspect_three_d(&self, id: &str) -> Result<Package> {
        let p = self
            .inner
            .three_d
            .lock()
            .await
            .packages
            .get(id)
            .cloned()
            .ok_or("3D asset was not found")?;
        validate(&p)?;
        for r in &p.resources {
            read_verified(&self.inner.root, r)?;
        }
        Ok(p)
    }
    pub async fn read_three_d_resource(&self, r: ResourceRequest) -> Result<ResourceData> {
        let p = self
            .inner
            .three_d
            .lock()
            .await
            .packages
            .get(&r.id)
            .cloned()
            .ok_or("3D asset was not found")?;
        validate(&p)?;
        let resource = p
            .resources
            .into_iter()
            .find(|x| x.id == r.resource_id)
            .ok_or("3D resource was not found")?;
        let b = read_verified(&self.inner.root, &resource)?;
        Ok(ResourceData {
            resource,
            data_base64: STANDARD.encode(b),
        })
    }
    pub async fn three_d_export(&self, id: &str) -> Result<Vec<u8>> {
        let p = self.inspect_three_d(id).await?;
        let mut zip = zip::ZipWriter::new(Cursor::new(Vec::new()));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Deflated);
        zip.start_file("manifest.json", options).map_err(io_error)?;
        zip.write_all(&serde_json::to_vec_pretty(&p).map_err(io_error)?)
            .map_err(io_error)?;
        for r in &p.resources {
            let b = read_verified(&self.inner.root, r)?;
            zip.start_file(
                format!("originals/{}.{}", r.id, r.kind.extension()),
                options,
            )
            .map_err(io_error)?;
            zip.write_all(&b).map_err(io_error)?;
            let replacements = r
                .links
                .iter()
                .map(|l| {
                    let t = p.resources.iter().find(|x| x.id == l.target).unwrap();
                    (
                        l.reference.pointer.clone(),
                        format!("{}.{}", t.id, t.kind.extension()),
                    )
                })
                .collect::<Vec<_>>();
            let localized = format::rewrite(&b, r.kind, &replacements)?;
            if localized.len() > MAX_RESOURCE {
                return Err("Localized 3D resource exceeds 32 MiB".into());
            }
            zip.start_file(format!("scene/{}.{}", r.id, r.kind.extension()), options)
                .map_err(io_error)?;
            zip.write_all(&localized).map_err(io_error)?;
        }
        Ok(zip.finish().map_err(io_error)?.into_inner())
    }
    pub async fn export_three_d_path(&self, id: &str, path: PathBuf) -> Result<()> {
        if path
            .extension()
            .and_then(|s| s.to_str())
            .is_none_or(|s| !s.eq_ignore_ascii_case("zip"))
        {
            return Err("Export 3D assets to a ZIP file".into());
        }
        let parent = std::fs::canonicalize(path.parent().ok_or("3D export has no parent folder")?)
            .map_err(io_error)?;
        if parent.starts_with(&self.inner.root) {
            return Err("Choose an export folder outside managed application storage".into());
        }
        let destination = parent.join(path.file_name().ok_or("Invalid 3D export path")?);
        let bytes = self.three_d_export(id).await?;
        let mut file = tempfile::NamedTempFile::new_in(parent).map_err(io_error)?;
        file.write_all(&bytes).map_err(io_error)?;
        file.as_file().sync_all().map_err(io_error)?;
        file.persist_noclobber(destination)
            .map_err(|_| "3D export already exists or could not be saved")?;
        Ok(())
    }
}
