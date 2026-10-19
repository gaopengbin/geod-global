//! Anonymous read-only access for exact reviewed Planetary Computer products.
//! Canonical metadata stays immutable; SAS queries exist only in runtime memory.
use super::{document, json_document, record, Selection};
use crate::{datasets::next_planetary, features, ProxySettings, Result};
use futures_util::StreamExt;
use serde_json::Value;
use std::{
    collections::BTreeMap,
    future::Future,
    path::Path,
    sync::LazyLock,
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use url::Url;
use zeroize::Zeroizing;

const API: &str = "https://planetarycomputer.microsoft.com/api/sas/v1/token";
const MAX_RESPONSE: usize = 16 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(15);
const EXPIRY_MARGIN: i64 = 90;
const INVALID_ACCESS: &str = "Planetary Computer returned invalid read-only container access";
const INVALID_SOURCE: &str =
    "Planetary Computer access does not match the immutable reviewed original";
const RATE_LIMITED: &str =
    "Planetary Computer is temporarily rate limiting public data access. Wait before retrying.";

#[derive(Clone, Copy)]
struct Scope {
    account: &'static str,
    container: &'static str,
}
impl Scope {
    fn reviewed(account: &'static str, container: &'static str) -> Result<Self> {
        if !matches!(
            (account, container),
            ("hreadatasa", "hrea")
                | ("pcdata01euw", "impact")
                | ("noaacdr", "cogs")
                | ("nclimgridwesteurope", "nclimgrid-cogs")
                | ("modiseuwest", "modis-061-cogs")
        ) {
            return Err(INVALID_SOURCE.into());
        }
        Ok(Self { account, container })
    }
    fn endpoint(self) -> Result<Url> {
        // Neither a caller-provided account nor a source URL enters this path.
        Url::parse(&format!("{API}/{}/{}", self.account, self.container))
            .map_err(|_| INVALID_ACCESS.into())
    }
    fn canonical(self, href: &str) -> Result<Url> {
        let url = super::public_url(href).map_err(|_| INVALID_SOURCE)?;
        if url.scheme() != "https"
            || url.host_str() != Some(format!("{}.blob.core.windows.net", self.account).as_str())
            || url.port().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || !url.path().starts_with(&format!("/{}/", self.container))
        {
            return Err(INVALID_SOURCE.into());
        }
        Ok(url)
    }
}

struct Token {
    query: Zeroizing<String>,
    expiry: i64,
}
impl Token {
    /// The existing providers/planetary.rs read-only-container contract is kept:
    /// unique nonempty fields, sp=r/rl, sr=c, sig and both expiry declarations.
    /// This adapter additionally rejects policy/account-SAS and URL-injection
    /// fields and keeps a 90-second safety margin instead of 60 seconds.
    fn from_value_at(value: &Value, now: i64) -> Result<Self> {
        let expiry = value["msft:expiry"]
            .as_str()
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .ok_or(INVALID_ACCESS)?
            .timestamp();
        let raw = value["token"].as_str().ok_or(INVALID_ACCESS)?;
        if raw.is_empty()
            || raw.len() > 8192
            || raw
                .chars()
                .any(|c| c.is_control() || matches!(c, '#' | '?'))
        {
            return Err(INVALID_ACCESS.into());
        }
        let mut pairs = BTreeMap::new();
        for (key, value) in url::form_urlencoded::parse(raw.as_bytes()) {
            if !matches!(
                key.as_ref(),
                "st" | "se"
                    | "sp"
                    | "spr"
                    | "sv"
                    | "sr"
                    | "sig"
                    | "skoid"
                    | "sktid"
                    | "skt"
                    | "ske"
                    | "sks"
                    | "skv"
            ) || value.is_empty()
                || value.chars().any(char::is_control)
                || pairs.insert(key.into_owned(), value.into_owned()).is_some()
            {
                return Err(INVALID_ACCESS.into());
            }
        }
        if !matches!(pairs.get("sp").map(String::as_str), Some("r" | "rl"))
            || pairs.get("sr").map(String::as_str) != Some("c")
            || !pairs.contains_key("sig")
            || pairs.get("spr").is_some_and(|value| value != "https")
            || pairs.get("sks").is_some_and(|value| value != "b")
        {
            return Err(INVALID_ACCESS.into());
        }
        let sas_expiry = pairs
            .get("se")
            .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
            .ok_or(INVALID_ACCESS)?
            .timestamp();
        let expiry = expiry.min(sas_expiry);
        if expiry <= now.saturating_add(EXPIRY_MARGIN) {
            return Err("Planetary Computer returned expired data access".into());
        }
        // Parse then encode; response text cannot set an origin, fragment or path.
        let mut encoded = url::form_urlencoded::Serializer::new(String::new());
        encoded.extend_pairs(&pairs);
        Ok(Self {
            query: Zeroizing::new(encoded.finish()),
            expiry,
        })
    }
    fn usable_at(&self, now: i64) -> bool {
        self.expiry > now.saturating_add(EXPIRY_MARGIN)
    }
    fn usable(&self) -> bool {
        self.usable_at(chrono::Utc::now().timestamp())
    }
}

#[derive(Default)]
struct ContainerState {
    token: Option<Token>,
    limited_until: Option<Instant>,
}
#[derive(Default)]
struct AccessCache {
    hrea: Mutex<ContainerState>,
    impact: Mutex<ContainerState>,
    ocean: Mutex<ContainerState>,
    climate: Mutex<ContainerState>,
    modis: Mutex<ContainerState>,
}
static CACHE: LazyLock<AccessCache> = LazyLock::new(AccessCache::default);

impl AccessCache {
    fn state(&self, scope: Scope) -> Result<&Mutex<ContainerState>> {
        match (scope.account, scope.container) {
            ("hreadatasa", "hrea") => Ok(&self.hrea),
            ("pcdata01euw", "impact") => Ok(&self.impact),
            ("noaacdr", "cogs") => Ok(&self.ocean),
            ("nclimgridwesteurope", "nclimgrid-cogs") => Ok(&self.climate),
            ("modiseuwest", "modis-061-cogs") => Ok(&self.modis),
            _ => Err(INVALID_SOURCE.into()),
        }
    }
    async fn query_with<F, Fut>(&self, scope: Scope, fetch: F) -> Result<Zeroizing<String>>
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = Result<Token>>,
    {
        // One lock per reviewed container merges Range/HEAD workers. Cancelled
        // requests drop this guard, with no detached fetch or persisted token.
        let mut state = self.state(scope)?.lock().await;
        if state
            .limited_until
            .is_some_and(|until| until > Instant::now())
        {
            return Err(RATE_LIMITED.into());
        }
        state.limited_until = None;
        if !state.token.as_ref().is_some_and(Token::usable) {
            match fetch().await {
                Ok(token) => state.token = Some(token),
                Err(error) => {
                    if error == RATE_LIMITED {
                        state.limited_until = Some(Instant::now() + Duration::from_secs(30));
                    }
                    return Err(error);
                }
            }
        }
        let token = state
            .token
            .as_ref()
            .filter(|token| token.usable())
            .ok_or("Planetary Computer data access expired before use")?;
        Ok(Zeroizing::new(token.query.to_string()))
    }
}

async fn fetch_token(settings: &ProxySettings, scope: Scope) -> Result<Token> {
    // The deadline includes DNS, connection establishment and every body chunk.
    tokio::time::timeout(REQUEST_TIMEOUT, async {
        let endpoint = scope.endpoint()?;
        let unsigned = super::public_url(endpoint.as_str()).map_err(|_| INVALID_ACCESS)?;
        let client = features::client_with_timeout(&unsigned, settings, REQUEST_TIMEOUT)
            .await
            .map_err(|_| "Cannot reach Planetary Computer public access service")?;
        let response = client
            .get(unsigned)
            .header("Accept", "application/json")
            .send()
            .await
            .map_err(|_| "Planetary Computer public access request failed")?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS {
            return Err(RATE_LIMITED.into());
        }
        if !response.status().is_success() {
            return Err(format!(
                "Planetary Computer public access returned HTTP {}",
                response.status().as_u16()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_RESPONSE as u64)
        {
            return Err("Planetary Computer public access response exceeded 16 KiB".into());
        }
        let media = response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .split(';')
            .next()
            .unwrap_or("")
            .trim();
        if media != "application/json" {
            return Err("Planetary Computer public access did not return JSON".into());
        }
        let mut body = Zeroizing::new(Vec::<u8>::new());
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk =
                chunk.map_err(|_| "Planetary Computer public access response could not be read")?;
            if body.len().saturating_add(chunk.len()) > MAX_RESPONSE {
                return Err("Planetary Computer public access response exceeded 16 KiB".into());
            }
            body.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&body)
            .map_err(|_| "Planetary Computer public access returned invalid JSON")?;
        Token::from_value_at(&value, chrono::Utc::now().timestamp())
    })
    .await
    .map_err(|_| "Planetary Computer public access request timed out")?
}

fn signed_url(scope: Scope, canonical: &Url, query: &str) -> Result<Url> {
    // Independently guard the final output even after exact source validation.
    let original = scope.canonical(canonical.as_str())?;
    let mut signed = original.clone();
    signed.set_query(Some(query));
    if signed.origin() != original.origin()
        || signed.path() != original.path()
        || signed.fragment().is_some()
        || !signed.username().is_empty()
        || signed.password().is_some()
    {
        return Err(INVALID_ACCESS.into());
    }
    Ok(signed)
}

pub(super) async fn delivery_url(
    settings: &ProxySettings,
    root: &Path,
    selection: &Selection,
) -> Result<Option<Url>> {
    let (saved, snapshot) = record(root, &selection.snapshot_id)?;
    let asset = snapshot
        .assets
        .iter()
        .find(|asset| asset.key == selection.asset_key)
        .ok_or(INVALID_SOURCE)?;
    if asset.transport_adapter.as_deref() != Some(next_planetary::ADAPTER) {
        return Ok(None);
    }
    if saved.kind != "item"
        || saved.item_index.is_some()
        || !asset.eligible
        || asset.original_href.as_deref() != Some(asset.href.as_str())
    {
        return Err(INVALID_SOURCE.into());
    }
    let proof = saved.reviewed_dataset.as_ref().ok_or(INVALID_SOURCE)?;
    let raw = json_document(&document(root, &saved.document_sha256)?)?;
    let (account, container) = if crate::datasets::additional_pc::is_product(&proof.dataset.id) {
        crate::datasets::additional_pc::access_scope(
            proof,
            &saved.document_url,
            &raw,
            &selection.asset_key,
            &asset.href,
        )?
    } else {
        next_planetary::access_scope(
            proof,
            &saved.document_url,
            &raw,
            &selection.asset_key,
            &asset.href,
        )?
    };
    let scope = Scope::reviewed(account, container)?;
    let canonical = scope.canonical(&asset.href)?;
    let query = CACHE
        .query_with(scope, || fetch_token(settings, scope))
        .await?;
    Ok(Some(signed_url(scope, &canonical, &query)?))
}

#[cfg(test)]
mod tests {
    #[test]
    fn every_reviewed_container_has_an_independent_access_cache() {
        let cache = super::AccessCache::default();
        let mut states = std::collections::BTreeSet::new();
        for (account, container) in [
            ("hreadatasa", "hrea"),
            ("pcdata01euw", "impact"),
            ("noaacdr", "cogs"),
            ("nclimgridwesteurope", "nclimgrid-cogs"),
            ("modiseuwest", "modis-061-cogs"),
        ] {
            let scope = super::Scope::reviewed(account, container).unwrap();
            let state = cache.state(scope).unwrap();
            assert!(states.insert(state as *const _ as usize));
        }
        assert!(super::Scope::reviewed("modiseuwest", "cogs").is_err());
    }
    use super::*;
    use serde_json::json;
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    fn value(now: i64, sp: &str, msft_seconds: i64, sas_seconds: i64) -> Value {
        let expiry = chrono::DateTime::<chrono::Utc>::from_timestamp(now + msft_seconds, 0)
            .unwrap()
            .to_rfc3339();
        let se = chrono::DateTime::<chrono::Utc>::from_timestamp(now + sas_seconds, 0)
            .unwrap()
            .to_rfc3339();
        let mut encoded = url::form_urlencoded::Serializer::new(String::new());
        encoded.extend_pairs([
            ("sp", sp),
            ("sr", "c"),
            ("se", se.as_str()),
            ("sig", "fixture+/=signature"),
        ]);
        json!({"msft:expiry":expiry,"token":encoded.finish()})
    }
    #[test]
    fn container_read_only_permissions_and_earliest_expiry_remain_strict() {
        let now = 1_800_000_000;
        for permission in ["r", "rl"] {
            let token = Token::from_value_at(&value(now, permission, 600, 400), now).unwrap();
            assert_eq!(token.expiry, now + 400);
            assert!(token.usable_at(now + 309));
            assert!(!token.usable_at(now + 310));
        }
        assert!(Token::from_value_at(&value(now, "r", 90, 600), now).is_err());
        assert!(Token::from_value_at(&value(now, "r", 600, 90), now).is_err());
        assert!(Token::from_value_at(&value(now, "r", 91, 600), now).is_ok());
        for permission in ["", "w", "rwl", "racwdl", "l", "R", "rlw"] {
            assert!(Token::from_value_at(&value(now, permission, 600, 600), now).is_err());
        }
    }
    #[test]
    fn ambiguous_policy_account_and_injected_token_fields_are_rejected() {
        let now = 1_800_000_000;
        let valid = value(now, "rl", 600, 600);
        for suffix in [
            "&sp=w",
            "&%73p=w",
            "&SP=w",
            "&sr=b",
            "&sig=",
            "&si=write-policy",
            "&ss=b",
            "&srt=sco",
            "&spr=https,http",
            "&sks=f",
            "&unknown=x",
            "#fragment",
            "?other",
            "&sig=%0d%0a",
        ] {
            let mut malformed = valid.clone();
            malformed["token"] = json!(format!("{}{suffix}", valid["token"].as_str().unwrap()));
            assert!(
                Token::from_value_at(&malformed, now).is_err(),
                "unexpected accepted suffix"
            );
        }
        for token in [
            "",
            "sp=r&sr=c&sig=x",
            "sp=r&sr=c&sig=x&se=bad",
            "https://evil.example/?token=x",
        ] {
            let mut malformed = valid.clone();
            malformed["token"] = json!(token);
            assert!(Token::from_value_at(&malformed, now).is_err());
        }
        let mut oversized = valid.clone();
        oversized["token"] = json!("x".repeat(8193));
        assert!(Token::from_value_at(&oversized, now).is_err());
    }
    #[test]
    fn signed_output_cannot_change_reviewed_origin_container_path_or_fragment() {
        let now = 1_800_000_000;
        let token = Token::from_value_at(&value(now, "rl", 600, 600), now).unwrap();
        let scope = Scope::reviewed("hreadatasa", "hrea").unwrap();
        let original = scope
            .canonical("https://hreadatasa.blob.core.windows.net/hrea/item.tif")
            .unwrap();
        let signed = signed_url(scope, &original, &token.query).unwrap();
        assert_eq!(signed.origin(), original.origin());
        assert_eq!(signed.path(), original.path());
        assert!(signed.fragment().is_none());
        assert!(signed.query_pairs().any(|(key, _)| key == "sig"));
        assert!(super::super::public_url(signed.as_str()).is_err());
        for href in [
            "https://pcdata01euw.blob.core.windows.net/hrea/item.tif",
            "https://hreadatasa.blob.core.windows.net/other/item.tif",
            "https://hreadatasa.blob.core.windows.net/hrea/item.tif?sig=outside",
            "http://hreadatasa.blob.core.windows.net/hrea/item.tif",
            "https://user@hreadatasa.blob.core.windows.net/hrea/item.tif",
            "https://hreadatasa.blob.core.windows.net/hrea/item.tif#fragment",
        ] {
            assert!(scope.canonical(href).is_err());
        }
        assert!(Scope::reviewed("hreadatasa", "other").is_err());
        assert!(Scope::reviewed("unreviewed", "hrea").is_err());
    }
    #[test]
    fn source_permission_requires_unchanged_item_proof_selected_layer_and_canonical_href() {
        use crate::datasets::{ResolveRequest, SourceProvenance};
        let items: Vec<Value> = serde_json::from_str(include_str!(
            "../datasets/next-planetary-reviewed-items.json"
        ))
        .unwrap();
        for reviewed in items {
            let id = reviewed["datasetId"].as_str().unwrap();
            let definition = crate::datasets::list()
                .into_iter()
                .find(|d| d.id == id)
                .unwrap();
            for layer in &definition.layers {
                let request = ResolveRequest {
                    dataset_id: id.into(),
                    bounds: serde_json::from_value(reviewed["document"]["bbox"].clone()).unwrap(),
                    year: definition.default_year,
                    month: definition.default_month,
                    country: None,
                    layer: Some(layer.id.clone()),
                    region: None,
                    season: None,
                };
                let plan = crate::datasets::resolve(request).unwrap();
                let source = &plan.sources[0];
                let proof = SourceProvenance {
                    schema_version: 2,
                    dataset: plan.dataset.clone(),
                    request: plan.request.clone(),
                    layer: layer.clone(),
                    source_id: source.id.clone(),
                    source_title: source.title.clone(),
                    source_bounds: source.bounds,
                    declaration_kind: "reviewed-client-preset; publisher file bytes and embedded metadata verified separately".into(),
                    catalog_raster: None,
                };
                let raw = &reviewed["document"];
                let item_url = reviewed["url"].as_str().unwrap();
                let href = raw["assets"][&layer.id]["href"].as_str().unwrap();
                let (account, container) =
                    next_planetary::access_scope(&proof, item_url, raw, &layer.id, href).unwrap();
                let scope = Scope::reviewed(account, container).unwrap();
                assert!(scope.canonical(href).is_ok());
                let mut counterfeit = proof.clone();
                counterfeit.dataset.id = "custom-stac".into();
                assert!(
                    next_planetary::access_scope(&counterfeit, item_url, raw, &layer.id, href,)
                        .is_err()
                );
                counterfeit = proof.clone();
                counterfeit.source_id.push_str("-unrelated");
                assert!(
                    next_planetary::access_scope(&counterfeit, item_url, raw, &layer.id, href,)
                        .is_err()
                );
                assert!(
                    next_planetary::access_scope(&proof, item_url, raw, "visual", href,).is_err()
                );
                assert!(next_planetary::access_scope(
                    &proof,
                    item_url,
                    raw,
                    &layer.id,
                    &format!("{href}?sig=outside"),
                )
                .is_err());
                let mut changed = raw.clone();
                changed["properties"]["datetime"] = json!("2030-01-01T00:00:00Z");
                assert!(
                    next_planetary::access_scope(&proof, item_url, &changed, &layer.id, href,)
                        .is_err()
                );
            }
        }
    }
    #[tokio::test]
    async fn simultaneous_ranges_share_one_ephemeral_container_fetch() {
        let cache = Arc::new(AccessCache::default());
        let calls = Arc::new(AtomicUsize::new(0));
        let scope = Scope::reviewed("hreadatasa", "hrea").unwrap();
        let mut workers = Vec::new();
        for _ in 0..32 {
            let cache = cache.clone();
            let calls = calls.clone();
            workers.push(tokio::spawn(async move {
                cache
                    .query_with(scope, || async {
                        calls.fetch_add(1, Ordering::SeqCst);
                        tokio::time::sleep(Duration::from_millis(10)).await;
                        let now = chrono::Utc::now().timestamp();
                        Token::from_value_at(&value(now, "r", 600, 600), now)
                    })
                    .await
                    .unwrap()
            }));
        }
        for worker in workers {
            assert!(!worker.await.unwrap().is_empty());
        }
        assert_eq!(calls.load(Ordering::SeqCst), 1);
        let separate = Scope::reviewed("pcdata01euw", "impact").unwrap();
        cache
            .query_with(separate, || async {
                calls.fetch_add(1, Ordering::SeqCst);
                let now = chrono::Utc::now().timestamp();
                Token::from_value_at(&value(now, "r", 600, 600), now)
            })
            .await
            .unwrap();
        assert_eq!(calls.load(Ordering::SeqCst), 2);
    }
    #[tokio::test]
    async fn expired_entry_refreshes_and_cancelled_fetch_releases_container_lock() {
        let cache = Arc::new(AccessCache::default());
        let scope = Scope::reviewed("noaacdr", "cogs").unwrap();
        let now = chrono::Utc::now().timestamp();
        cache.state(scope).unwrap().lock().await.token = Some(Token {
            query: Zeroizing::new("old-entry".into()),
            expiry: now + 90,
        });
        let called = Arc::new(AtomicUsize::new(0));
        let calls = called.clone();
        cache
            .query_with(scope, || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Token::from_value_at(&value(now, "r", 600, 600), now)
            })
            .await
            .unwrap();
        assert_eq!(called.load(Ordering::SeqCst), 1);
        let waiting = Scope::reviewed("nclimgridwesteurope", "nclimgrid-cogs").unwrap();
        let child_cache = cache.clone();
        let (started, observed) = tokio::sync::oneshot::channel();
        let worker = tokio::spawn(async move {
            child_cache
                .query_with(waiting, || async {
                    started.send(()).unwrap();
                    std::future::pending::<Result<Token>>().await
                })
                .await
        });
        observed.await.unwrap();
        worker.abort();
        assert!(worker.await.unwrap_err().is_cancelled());
        tokio::time::timeout(
            Duration::from_secs(1),
            cache.query_with(waiting, || async {
                Token::from_value_at(&value(now, "rl", 600, 600), now)
            }),
        )
        .await
        .unwrap()
        .unwrap();
    }
    #[tokio::test]
    async fn upstream_rate_limit_is_shared_without_followup_request_amplification() {
        let cache = AccessCache::default();
        let scope = Scope::reviewed("noaacdr", "cogs").unwrap();
        let calls = AtomicUsize::new(0);
        let first = cache
            .query_with(scope, || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err(RATE_LIMITED.into())
            })
            .await;
        assert_eq!(first.unwrap_err(), RATE_LIMITED);
        let second = cache
            .query_with(scope, || async {
                calls.fetch_add(1, Ordering::SeqCst);
                Err("unexpected request".into())
            })
            .await;
        assert_eq!(second.unwrap_err(), RATE_LIMITED);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }
}
