use super::*;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(super) enum SelectionScope {
    Selected,
    Visible,
    Focused,
}

pub(super) fn originals_selection(receipt: &SearchReceipt, ids: &[String]) -> Result<()> {
    let actual: BTreeSet<_> = receipt
        .candidates
        .iter()
        .map(|c| c.item_id.as_str())
        .collect();
    let selected: BTreeSet<_> = ids.iter().map(String::as_str).collect();
    if receipt.selection_scope.is_none() || actual.is_empty() || actual != selected {
        return Err("Originals-only reviews require the exact native Explore selection. Read geod_catalog_selection again; area tasks still require coverage.".into());
    }
    Ok(())
}

pub(super) fn valid_item_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 256
        && id != "."
        && id != ".."
        && id
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"_.:-".contains(&v))
}

fn item_url(provider: &str, id: &str) -> Result<url::Url> {
    if !valid_item_id(id) {
        return Err("Invalid selected catalog scene ID.".into());
    }
    let source = catalog::source(provider)?;
    let base = source
        .endpoint
        .strip_suffix("/search")
        .ok_or("Unsupported catalog item endpoint.")?;
    let mut url = url::Url::parse(&format!("{base}/collections/{}/items/", source.collection))
        .map_err(|_| "Invalid reviewed catalog item endpoint.")?;
    url.path_segments_mut()
        .map_err(|_| "Invalid catalog item endpoint.")?
        .pop_if_empty()
        .push(id);
    Ok(url)
}

fn normalize_item(
    value: Value,
    id: &str,
    query: &SearchQuery,
    cloud_min: f64,
) -> Result<(Value, Candidate)> {
    if value["type"] != "Feature" || value["id"] != id {
        return Err("The provider did not return the selected scene.".into());
    }
    let document = json!({"type":"FeatureCollection","features":[value]});
    let mut candidates = normalize_catalog(&document, query)?;
    let candidate = candidates
        .pop()
        .ok_or("The selected scene has no supported native assets.")?;
    if catalog::source(&query.provider)?.clouds && candidate.cloud.is_none_or(|v| v < cloud_min) {
        return Err("The selected scene no longer matches the cloud filter.".into());
    }
    Ok((document["features"][0].clone(), candidate))
}

pub(super) async fn read(
    manager: &JobManager,
    session: &str,
    args: Value,
    context: Option<MapContext>,
) -> Result<Value> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Args {
        scope: String,
    }
    let a: Args = serde_json::from_value(args)
        .map_err(|_| "Choose selected, visible or focused catalog scenes.")?;
    if !uuid(session) {
        return Err("Invalid Agent conversation ID.".into());
    }
    if !matches!(a.scope.as_str(), "selected" | "visible" | "focused") {
        return Err("Choose selected, visible or focused catalog scenes.".into());
    }
    // Independent conversations deliberately have no ambient Explore catalog.
    // This is an unavailable observation, not an empty selection or a failed
    // request. In particular, do not query a provider or mint a search receipt.
    let unavailable = || {
        json!({"available":false,"selectionScope":a.scope,"context":null,
        "note":"No Explore catalog is attached to this request. The selection and current provider are unknown, not empty. Continue in the Explore assistant or use an explicit task reference; no search or download was performed."})
    };
    let Some(c) = context else {
        return Ok(unavailable());
    };
    c.validate()?;
    let Some(list) = c.catalog.as_ref() else {
        return Ok(unavailable());
    };
    let ids = match a.scope.as_str() {
        "selected" if !list.selection_truncated && list.selected_item_ids.len() == list.selected_count as usize => list.selected_item_ids.clone(),
        "visible" if list.visible_item_ids.len() == list.visible_loaded_count as usize => list.visible_item_ids.clone(),
        "focused" => list.focused_item_id.iter().cloned().collect(),
        "selected" | "visible" => return Err("The complete selection is not attached. Select at most 32 scenes (16 visible layers) and send again.".into()),
        _ => return Err("Choose selected, visible or focused catalog scenes.".into()),
    };
    if ids.is_empty()
        || ids.len() > MAX_FILES
        || ids.iter().collect::<BTreeSet<_>>().len() != ids.len()
    {
        return Err("Select actual Explore scenes before preparing their review.".into());
    }
    let query = SearchQuery {
        provider: c.provider.clone(),
        bounds: c.bounds,
        start: c.start.clone(),
        end: c.end.clone(),
        cloud_max: c.cloud_max,
        limit: 20,
    };
    query.validate()?;
    let settings = manager.proxy_settings().await;
    let first = item_url(&query.provider, &ids[0])?;
    let client =
        crate::features::client_with_timeout(&first, &settings, Duration::from_secs(25)).await?;
    // Four concurrent metadata reads; every endpoint is constructed from the
    // reviewed provider registry. UI metadata is not a native receipt.
    let values = futures_util::stream::iter(ids.into_iter().map(|id| {
      let client=client.clone();let query=query.clone();let cloud_min=c.cloud_min;
      async move {
        let response = client.get(item_url(&query.provider, &id)?).header("Accept","application/geo+json").send().await
            .map_err(|_| "Selected scene metadata could not be read. Check the connection and retry.")?;
        if !response.status().is_success() { return Err("The selected scene is not available from this provider. Search again or retry.".into()); }
        let mut stream = response.bytes_stream(); let mut bytes = Vec::new();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "Selected metadata read interrupted.")?;
            if bytes.len() + chunk.len() > MAX_DOCUMENT { return Err("Selected metadata exceeds 5 MiB.".into()); }
            bytes.extend_from_slice(&chunk);
        }
        let value: Value = serde_json::from_slice(&bytes).map_err(|_| "Invalid selected scene metadata.")?;
        normalize_item(value,&id,&query,cloud_min)
      }
    })).buffered(4).collect::<Vec<Result<(Value,Candidate)>>>().await;
    let mut features = Vec::new();
    let mut candidates = Vec::new();
    for value in values {
        let (item, candidate) = value?;
        features.push(item);
        candidates.push(candidate);
    }
    let document = json!({"type":"FeatureCollection","features":features});
    if query.provider == "copernicus" {
        protected::resolve_copernicus_candidates(manager, &mut candidates).await?;
    }
    if query.provider.starts_with("planetary-") {
        manager
            .inner
            .planetary_access
            .observe_catalogue(&document, catalog::source(&query.provider)?.collection)
            .await?;
    }
    let receipt = SearchReceipt {
        id: Uuid::new_v4().to_string(),
        session_id: session.into(),
        query,
        retrieved_at: now(),
        document_sha256: digest(&serde_json::to_vec(&document).map_err(io_error)?),
        candidates,
        next: None,
        selection_scope: Some(match a.scope.as_str() {
            "selected" => SelectionScope::Selected,
            "visible" => SelectionScope::Visible,
            _ => SelectionScope::Focused,
        }),
        more_available: false,
    };
    write_record(&manager.inner.root, "searches", &receipt.id, &receipt).await?;
    Ok(
        json!({"searchId":receipt.id,"query":receipt.query,"retrievedAt":receipt.retrieved_at,"metadataSha256":receipt.document_sha256,
        "selectionScope":a.scope,"complete":true,"moreAvailable":false,"canContinue":false,
        "scenes":receipt.candidates.iter().map(|c|json!({"itemId":c.item_id,"date":c.date,"cloud":c.cloud,"bounds":c.bounds,"assets":c.assets.iter().map(|a|&a.asset_key).collect::<Vec<_>>()})).collect::<Vec<_>>(),
        "note":"Exact UI selection re-read from the reviewed catalog. Complete means all selected IDs were read, not full AOI coverage or completed downloads. Use this searchId for native project/download review. Area-wide processed deliverables still require a native coverage check."}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn absent_explore_context_is_unknown_without_a_search_or_write() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let session = Uuid::new_v4().to_string();
        for scope in ["selected", "visible", "focused"] {
            let result = read(&manager, &session, json!({"scope":scope}), None)
                .await
                .unwrap();
            assert_eq!(result["available"], false);
            assert_eq!(result["selectionScope"], scope);
            assert!(result.get("searchId").is_none());
            assert!(result.get("scenes").is_none());
            assert!(result["note"]
                .as_str()
                .unwrap()
                .contains("unknown, not empty"));
        }
        assert!(read(&manager, &session, json!({"scope":"invented"}), None)
            .await
            .is_err());
        assert!(manager.list().await.is_empty());
        assert!(!manager.inner.root.join("agent-searches").exists());
        assert!(!manager.inner.root.join("agent-plans").exists());
    }
    #[test]
    fn selected_item_urls_cannot_supply_hosts_paths_or_other_collections() {
        for id in ["..", "a/b", "https://host/secret", "a%2fb", "", "../items"] {
            assert!(item_url("earth-search", id).is_err());
        }
        let url = item_url("earth-search", "S2A_18TWL_20261008_1_L2A").unwrap();
        assert_eq!(url.host_str(), Some("earth-search.aws.element84.com"));
        assert_eq!(
            url.path(),
            "/v1/collections/sentinel-2-l2a/items/S2A_18TWL_20261008_1_L2A"
        );
        assert!(item_url("unreviewed", "scene").is_err());
    }

    #[test]
    fn selection_rejects_substituted_ids_and_preserves_the_cloud_lower_bound() {
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../prototype/public/samples/earth-search-response.json"
        ))
        .unwrap();
        let item = fixture["features"][0].clone();
        let id = item["id"].as_str().unwrap();
        let query = SearchQuery {
            provider: "earth-search".into(),
            bounds: [-180.0, -80.0, 180.0, 80.0],
            start: "2025-06-01".into(),
            end: "2025-06-30".into(),
            cloud_max: 100.0,
            limit: 20,
        };
        let (_, candidate) = normalize_item(item.clone(), id, &query, 0.0).unwrap();
        assert_eq!(candidate.item_id, id);
        assert!(normalize_item(item.clone(), "another-scene", &query, 0.0).is_err());
        assert!(normalize_item(item.clone(), id, &query, 100.0).is_err());
    }

    #[tokio::test]
    async fn model_arguments_cannot_forge_a_human_original_review_authority() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let session = Uuid::new_v4().to_string();
        let receipt = super::super::tests::project_search_fixture(&manager, &session).await;
        let ids: Vec<_> = receipt
            .candidates
            .iter()
            .map(|c| c.item_id.clone())
            .collect();
        let error=crate::agent_actions::call(manager.clone(),&session,"geod_download_plan",json!({"searchId":receipt.id,"itemIds":ids,"assetKey":"visual","originalsOnly":true,"humanOriginals":true}),None).await.unwrap_err();
        assert_eq!(error, "Invalid Agent tool arguments.");
        assert!(manager.list().await.is_empty());
        let error = manager
            .agent_explicit_originals_review(
                &Uuid::new_v4().to_string(),
                &receipt.id,
                vec!["fabricated".into()],
                "visual",
            )
            .await
            .unwrap_err();
        assert!(error.contains("another conversation"));
        assert!(manager.list().await.is_empty());
    }
    #[tokio::test]
    async fn originals_only_requires_the_exact_ui_receipt_and_keeps_area_checks_separate() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let session = Uuid::new_v4().to_string();
        let mut receipt = super::super::tests::project_search_fixture(&manager, &session).await;
        let ids: Vec<_> = receipt
            .candidates
            .iter()
            .map(|c| c.item_id.clone())
            .collect();
        assert!(originals_selection(&receipt, &ids).is_err());
        receipt.selection_scope = Some(SelectionScope::Selected);
        assert!(originals_selection(&receipt, &ids).is_ok());
        assert!(originals_selection(&receipt, &ids[..1]).is_err());
        assert!(originals_selection(&receipt, &["fabricated".into()]).is_err());
        receipt.query.bounds = [-180.0, -80.0, 180.0, 80.0];
        write_record(&manager.inner.root, "searches", &receipt.id, &receipt)
            .await
            .unwrap();
        let error = manager
            .agent_download_plan(&session, &receipt.id, ids, "visual")
            .await
            .unwrap_err();
        assert!(error.contains("does not cover"));
        assert!(manager.inner.store.lock().await.jobs.is_empty());
    }

    #[tokio::test]
    #[ignore = "Explicit read-only online catalog and pending-review acceptance"]
    async fn live_selection_receipt_review_and_preview_without_download() {
        assert_eq!(
            std::env::var("GEOD_SELECTION_ACCEPTANCE").as_deref(),
            Ok("1")
        );
        let fixture: Value = serde_json::from_str(include_str!(
            "../../../../prototype/public/samples/earth-search-response.json"
        ))
        .unwrap();
        let ids: Vec<_> = fixture["features"]
            .as_array()
            .unwrap()
            .iter()
            .take(2)
            .map(|item| item["id"].as_str().unwrap().to_string())
            .collect();
        let c:MapContext=serde_json::from_value(json!({"page":"Explore","provider":"earth-search","bounds":[-180,-80,180,80],"start":"2025-06-01","end":"2025-06-30","cloudMax":100,"projectId":null,
            "catalog":{"state":"ready","totalCount":2,"filteredCount":2,"selectedCount":2,"loadedCount":0,"visibleLoadedCount":0,"complete":true,"pages":1,"retrievedAt":null,"includesSavedProjectScenes":false,"activeDate":null,"textFilter":"","sort":"date","dateCounts":[],"dateCountsTruncated":false,"selectedItemIds":ids}})).unwrap();
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let session = Uuid::new_v4().to_string();
        let receipt = read(&manager, &session, json!({"scope":"selected"}), Some(c))
            .await
            .unwrap();
        assert_eq!(
            receipt["scenes"]
                .as_array()
                .unwrap()
                .iter()
                .map(|scene| scene["itemId"].as_str().unwrap().to_string())
                .collect::<Vec<_>>(),
            ids
        );
        let review = manager
            .agent_download_plan_with_scope(
                &session,
                receipt["searchId"].as_str().unwrap(),
                ids,
                "visual",
                true,
            )
            .await
            .unwrap();
        assert_eq!(review["status"], "pending");
        assert!(review["coverage"].is_null());
        assert!(review["notes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|note| note.as_str().unwrap().contains("not requested or verified")));
        assert!(review["jobs"].as_array().unwrap().is_empty());
        let preview = manager
            .agent_plan_map_preview(
                &session,
                review["planId"].as_str().unwrap(),
                review["planHash"].as_str().unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(preview["selections"].as_array().unwrap().len(), 2);
        assert!(manager.inner.store.lock().await.jobs.is_empty());
        let output = json!({"onlineCatalogRead":true,"exactSelectedScenes":2,"nativePendingReview":true,"previewScenes":2,"downloadJobs":0,"metadataSha256":receipt["metadataSha256"]});
        if let Ok(path) = std::env::var("GEOD_SELECTION_RECEIPT") {
            std::fs::write(path, serde_json::to_vec_pretty(&output).unwrap()).unwrap();
        }
        println!("{output}");
    }
}
