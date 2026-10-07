//! Continuation URLs come only from the pinned catalog, never model arguments.
use super::*;
pub(super) fn next_url(value: &Value, base: &url::Url) -> Result<Option<String>> {
    let Some(link) = value["links"]
        .as_array()
        .and_then(|links| links.iter().find(|l| l["rel"] == "next"))
    else {
        return Ok(None);
    };
    if link
        .get("method")
        .and_then(Value::as_str)
        .is_some_and(|m| m != "GET")
    {
        return Ok(None);
    }
    let next = base
        .join(
            link["href"]
                .as_str()
                .ok_or("Invalid catalog continuation.")?,
        )
        .map_err(|_| "Invalid catalog continuation.")?;
    if next.scheme() != base.scheme()
        || next.host_str() != base.host_str()
        || next.port_or_known_default() != base.port_or_known_default()
        || next.path() != base.path()
        || !next.username().is_empty()
        || next.password().is_some()
        || next.fragment().is_some()
    {
        return Err("Catalog continuation left the reviewed search endpoint.".into());
    }
    Ok(Some(next.to_string()))
}
pub(super) async fn more(manager: &JobManager, session: &str, args: Value) -> Result<Value> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Args {
        search_id: String,
    }
    let a: Args =
        serde_json::from_value(args).map_err(|_| "Use a native search ID to continue.")?;
    let mut receipt = footprint::search(manager, session, &a.search_id).await?;
    if receipt.candidates.len() >= 200 {
        return Err("Catalog continuation reached 200 scenes. Narrow the area or review a different product.".into());
    }
    let base = catalog::url(&receipt.query)?;
    let next=receipt.next.as_ref().ok_or("This native search has no supported next page. Search another date interval without relaxing user constraints.")?;
    // Revalidate persisted source links after a restart.
    next_url(&json!({"links":[{"rel":"next","href":next}]}), &base)?;
    let url = url::Url::parse(next).map_err(|_| "Invalid catalog continuation.")?;
    let client = crate::features::client_with_timeout(
        &url,
        &manager.proxy_settings().await,
        Duration::from_secs(25),
    )
    .await?;
    let response = client
        .get(url)
        .header("Accept", "application/geo+json")
        .send()
        .await
        .map_err(|_| "Agent catalog search failed. Check your proxy and try again.")?;
    if !response.status().is_success() {
        return Err("Agent catalog search failed. Try again later.".into());
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| "Catalog response interrupted.")?;
        if bytes.len() + chunk.len() > MAX_DOCUMENT {
            return Err("Catalog response exceeds 5 MiB.".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "Catalog response is invalid JSON.")?;
    let mut candidates = normalize_catalog(&value, &receipt.query)?;
    if receipt.query.provider == "copernicus" {
        protected::resolve_copernicus_candidates(manager, &mut candidates).await?;
    }
    if receipt.query.provider.starts_with("planetary-") {
        manager
            .inner
            .planetary_access
            .observe_catalogue(&value, catalog::source(&receipt.query.provider)?.collection)
            .await?;
    }
    for candidate in candidates {
        if !receipt
            .candidates
            .iter()
            .any(|c| c.item_id == candidate.item_id)
        {
            receipt.candidates.push(candidate);
        }
    }
    if receipt.candidates.len() > 200 {
        return Err("Catalog continuation exceeds 200 scenes. Narrow the area or review a different product.".into());
    }
    receipt.next = next_url(&value, &base)?;
    receipt.more_available = value["links"]
        .as_array()
        .is_some_and(|l| l.iter().any(|v| v["rel"] == "next"));
    receipt.document_sha256 =
        digest(format!("{}:{}", receipt.document_sha256, digest(&bytes)).as_bytes());
    receipt.id = Uuid::new_v4().to_string();
    receipt.retrieved_at = now();
    write_record(&manager.inner.root, "searches", &receipt.id, &receipt).await?;
    Ok(
        json!({"searchId":receipt.id,"query":receipt.query,"sceneCount":receipt.candidates.len(),"moreAvailable":receipt.more_available,"canContinue":receipt.next.is_some(),
        "note":"This receipt retains all validated pages. Use geod_scene_coverage to obtain a newest-first covering selection. Do not infer coverage from counts or rectangles."}),
    )
}
