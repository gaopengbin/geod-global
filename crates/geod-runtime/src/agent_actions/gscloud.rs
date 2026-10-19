//! Session-bound metadata receipts and confirmed original ZIP downloads.
//! The model supplies only receipt/item identities, never URLs or account data.
use super::*;
use crate::gscloud::{original, DetailRequest};
fn invalid(_: serde_json::Error) -> String {
    "Invalid GSCloud Agent arguments.".into()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct Scope {
    product_id: String,
    pub(super) item_ids: Vec<String>,
    bounds: [f64; 4],
    metadata_sha256: String,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Receipt {
    session: String,
    retrieved_at: String,
    scope: Scope,
}
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Review {
    search_id: String,
    item_ids: Vec<String>,
}
fn validate_receipt(scope: &Scope) -> Result<()> {
    if !(valid_bounds(scope.bounds)
        && scope.metadata_sha256.len() == 64
        && scope
            .metadata_sha256
            .bytes()
            .all(|b| b.is_ascii_hexdigit() && !b.is_ascii_uppercase()))
        || scope.item_ids.is_empty()
        || scope.item_ids.len() > 20
        || scope.item_ids.iter().collect::<BTreeSet<_>>().len() != scope.item_ids.len()
    {
        return Err("Invalid GSCloud original review.".into());
    }
    for id in &scope.item_ids {
        original::pending_job(&scope.product_id, id)?;
    }
    Ok(())
}
pub(super) fn validate(scope: &Scope) -> Result<()> {
    validate_receipt(scope)?;
    if scope.item_ids.len() > 4 {
        return Err("Choose up to 4 GSCloud originals per review.".into());
    }
    Ok(())
}
pub(super) async fn search(manager: &JobManager, session: &str, args: Value) -> Result<Value> {
    if !uuid(session) {
        return Err("Invalid GSCloud conversation.".into());
    }
    let mut value = crate::gscloud::agent_call(manager, "geod_gscloud_search", args).await?;
    // Only reviewed original products receive download receipts. Other catalogue
    // products remain fully searchable without acquiring unreviewed capability.
    if value["access"]["agentDownload"] == true {
        let scope = Scope {
            product_id: value["request"]["productId"]
                .as_str()
                .ok_or("Missing product")?
                .into(),
            item_ids: value["items"]
                .as_array()
                .ok_or("Missing GSCloud items")?
                .iter()
                .filter(|item| item["presence"] == "available")
                .filter_map(|item| item["id"].as_str().map(String::from))
                .collect(),
            bounds: serde_json::from_value(value["request"]["bounds"].clone()).map_err(invalid)?,
            metadata_sha256: value["metadataSha256"]
                .as_str()
                .ok_or("Missing GSCloud metadata hash")?
                .into(),
        };
        if !scope.item_ids.is_empty() {
            validate_receipt(&scope)?;
            let id = Uuid::new_v4().to_string();
            write_record(
                &manager.inner.root,
                "gscloud-searches",
                &id,
                &Receipt {
                    session: session.into(),
                    retrieved_at: now(),
                    scope,
                },
            )
            .await?;
            value["searchId"] = json!(id);
        }
    }
    Ok(value)
}
pub(super) async fn prepare(manager: &JobManager, session: &str, args: Value) -> Result<Value> {
    let args: Review = serde_json::from_value(args).map_err(invalid)?;
    let receipt: Receipt =
        read_record(&manager.inner.root, "gscloud-searches", &args.search_id).await?;
    if receipt.session != session
        || !uuid(session)
        || DateTime::parse_from_rfc3339(&receipt.retrieved_at).map_or(true, |date| {
            date > Utc::now()
                || Utc::now() - date.with_timezone(&Utc) > chrono::Duration::minutes(TTL_MINUTES)
        })
    {
        return Err(
            "GSCloud search is stale or belongs to another conversation. Search again.".into(),
        );
    }
    validate_receipt(&receipt.scope)?;
    if args.item_ids.is_empty()
        || args.item_ids.len() > 4
        || args.item_ids.iter().collect::<BTreeSet<_>>().len() != args.item_ids.len()
        || args
            .item_ids
            .iter()
            .any(|id| !receipt.scope.item_ids.contains(id))
    {
        return Err(
            "Choose up to 4 available originals from this conversation's GSCloud search.".into(),
        );
    }
    let scope = Scope {
        item_ids: args.item_ids,
        ..receipt.scope
    };
    for id in &scope.item_ids {
        manager
            .gscloud_validate_original(DetailRequest {
                product_id: scope.product_id.clone(),
                item_id: id.clone(),
            })
            .await?;
    }
    let plan = Plan {
        id: Uuid::new_v4().to_string(),
        session_id: session.into(),
        policy: POLICY.into(),
        hash: String::new(),
        created_at: now(),
        expires_at: (Utc::now() + chrono::Duration::minutes(TTL_MINUTES)).to_rfc3339(),
        action: Action::GscloudDownload { scope },
    };
    let mut plan = plan;
    plan.hash = plan.effective_hash()?;
    plan.validate(session)?;
    write_record(&manager.inner.root, "plans", &plan.id, &plan).await?;
    status(manager, &plan).await
}
fn scope(plan: &Plan) -> &Scope {
    match &plan.action {
        Action::GscloudDownload { scope } => scope,
        _ => unreachable!(),
    }
}
pub(super) async fn status(manager: &JobManager, plan: &Plan) -> Result<Value> {
    let scope = scope(plan);
    let store = manager.inner.store.lock().await;
    let jobs = existing(plan, &store.jobs)?;
    if let Some(jobs) = &jobs {
        for (job, id) in jobs.iter().zip(&scope.item_ids) {
            original::validate_job(job)?;
            if job.item_id != *id
                || job.asset_key != original::pending_job(&scope.product_id, id)?.asset_key
            {
                return Err("GSCloud approval source differs from its review.".into());
            }
        }
    }
    let files = scope.item_ids.iter().map(|id| {
        let job = original::pending_job(&scope.product_id, id)?;
        Ok::<_, String>(json!({"itemId":id,"assetKey":job.asset_key,"bytes":null,"productId":scope.product_id}))
    }).collect::<Result<Vec<_>>>()?;
    Ok(
        json!({"planId":plan.id,"planHash":plan.hash,"kind":"download",
        "status":if jobs.is_some(){"submitted"}else if plan.expired(){"expired"}else{"pending"},
        "approvalRequired":true,"source":"Geospatial Data Cloud / 地理空间数据云","bounds":scope.bounds,"boundsCrs":"EPSG:4326",
        "start":null,"end":null,"polygon":null,"files":files,"expectedBytes":null,"format":"Original ZIP","archive":true,
        "expiresAt":plan.expires_at,"notes":["Downloads complete original ZIP files using your saved GSCloud account. No reservation, purchase, crop or source substitution.",
          "Whole ZIP, product/tile identity, member signatures, CRC and SHA-256 are verified. View the archive manifest and open its folder; archive raster map preview is unavailable."],
        "jobs":jobs.unwrap_or_default().iter().map(|job|job_view(job,!crate::active(&job.status)&&!store.active.contains_key(&job.id))).collect::<Vec<_>>() }),
    )
}
pub(super) async fn commit(manager: &JobManager, plan: &Plan) -> Result<Value> {
    let scope = scope(plan);
    if existing(plan, &manager.inner.store.lock().await.jobs)?.is_some() {
        return status(manager, plan).await;
    }
    if plan.expired() {
        return Err("Agent plan expired. Create a new plan before confirming.".into());
    }
    // Check every selected item before admitting any task. Worker independently
    // checks the actual account and delivery response; no anonymous fallback.
    for id in &scope.item_ids {
        manager
            .gscloud_validate_original(DetailRequest {
                product_id: scope.product_id.clone(),
                item_id: id.clone(),
            })
            .await?;
    }
    manager.gscloud_require_account().await?;
    let at = now();
    let jobs = scope
        .item_ids
        .iter()
        .zip(plan.job_ids())
        .map(|(item, id)| {
            let mut job = original::pending_job(&scope.product_id, item)?;
            job.id = id;
            job.agent_approval = Some(approval(plan, &at, None));
            Ok::<_, String>(job)
        })
        .collect::<Result<Vec<_>>>()?;
    let mut store = manager.inner.store.lock().await;
    store.accepting_jobs()?;
    if store.active.len() + jobs.len() > 64 {
        return Err("The local queue is full (64 jobs)".into());
    }
    if jobs.iter().any(|job| store.jobs.contains_key(&job.id)) {
        return Err("GSCloud task identity conflicts with the queue.".into());
    }
    for job in &jobs {
        store.jobs.insert(job.id.clone(), job.clone());
    }
    if let Err(error) = manager.persist(&store.jobs).await {
        for job in &jobs {
            store.jobs.remove(&job.id);
        }
        return Err(error);
    }
    for job in jobs {
        let token = CancellationToken::new();
        store.active.insert(job.id.clone(), token.clone());
        manager.spawn(job.id, token);
    }
    drop(store);
    status(manager, plan).await
}
pub(super) async fn inspect(manager: &JobManager, session: &str, args: Value) -> Result<Value> {
    #[derive(Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Request {
        id: String,
    }
    let args: Request = serde_json::from_value(args).map_err(invalid)?;
    let job = manager
        .get(&args.id)
        .await
        .ok_or("GSCloud original task was not found.")?;
    let approval = job
        .agent_approval
        .as_ref()
        .filter(|a| a.session_id == session)
        .ok_or("Choose a GSCloud archive from this conversation's approved plan.")?;
    let plan: Plan = read_record(&manager.inner.root, "plans", &approval.plan_id).await?;
    plan.validate_reference(session, &approval.plan_id)?;
    if !matches!(plan.action, Action::GscloudDownload { .. })
        || approval.plan_hash != plan.hash
        || !plan.job_ids().contains(&job.id)
    {
        return Err("GSCloud archive differs from its approved plan.".into());
    }
    manager.gscloud_inspect_original(&args.id).await
}
pub(super) fn definitions() -> Vec<Value> {
    vec![
        json!({"name":"geod_gscloud_download_plan","description":"Prepare a native confirmation card for 1..4 available original ZIPs from this conversation's geod_gscloud_search searchId. Supports six reviewed GSCloud terrain products only, with the user's saved desktop GSCloud account. Planning starts no transfer. Bind an archive goal, obtain native confirmation, then the native workflow resumes after download. No reservation, crop, alternate source or URLs.","inputSchema":{"type":"object","properties":{"searchId":{"type":"string","format":"uuid"},"itemIds":{"type":"array","minItems":1,"maxItems":4,"uniqueItems":true,"items":{"type":"string","maxLength":100}}},"required":["searchId","itemIds"],"additionalProperties":false}}),
        json!({"name":"geod_gscloud_archive_inspect","description":"Verify an actual completed GSCloud original ZIP owned by this conversation: SHA-256, whole file, product/tile, member names/signatures and CRC. Returns a safe archive manifest, not decoded raster pixels. Use kind archive with geod_workspace_open to show the delivered archive in the results pane.","inputSchema":{"type":"object","properties":{"id":{"type":"string","format":"uuid"}},"required":["id"],"additionalProperties":false}}),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    const SESSION: &str = "a1234567-1234-1234-1234-123456789abc";
    fn scope() -> Scope {
        Scope {
            product_id: "421".into(),
            item_ids: vec!["ASTGTM2_N40E116".into()],
            bounds: [116.2, 40.1, 116.4, 40.3],
            metadata_sha256: "a".repeat(64),
        }
    }
    #[test]
    fn archive_scope_cannot_smuggle_unreviewed_products_urls_or_duplicate_files() {
        assert!(validate(&scope()).is_ok());
        for (product, id) in [
            ("309", "aspect_60_04"),
            ("421", "ASTGTM_N40E116"),
            ("421", "../ASTGTM2_N40E116"),
            ("305", "https://example.com/file"),
        ] {
            let mut s = scope();
            s.product_id = product.into();
            s.item_ids = vec![id.into()];
            assert!(validate(&s).is_err());
        }
        let mut s = scope();
        s.item_ids.push(s.item_ids[0].clone());
        assert!(validate(&s).is_err());
        assert!(serde_json::from_value::<Review>(
            json!({"searchId":SESSION,"itemIds":["ASTGTM2_N40E116"],"approved":true})
        )
        .is_err());
    }
    #[tokio::test]
    async fn receipt_session_expiry_and_selection_fail_before_network_or_queue() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        let id = Uuid::new_v4().to_string();
        write_record(
            &manager.inner.root,
            "gscloud-searches",
            &id,
            &Receipt {
                session: SESSION.into(),
                retrieved_at: now(),
                scope: scope(),
            },
        )
        .await
        .unwrap();
        let args = json!({"searchId":id,"itemIds":["ASTGTM2_N40E116"]});
        assert!(prepare(
            &manager,
            "b1234567-1234-1234-1234-123456789abc",
            args.clone()
        )
        .await
        .unwrap_err()
        .contains("another conversation"));
        assert!(prepare(
            &manager,
            SESSION,
            json!({"searchId":id,"itemIds":["ASTGTM2_N41E116"]})
        )
        .await
        .is_err());
        write_record(
            &manager.inner.root,
            "gscloud-searches",
            &id,
            &Receipt {
                session: SESSION.into(),
                retrieved_at: "2000-01-01T00:00:00Z".into(),
                scope: scope(),
            },
        )
        .await
        .unwrap();
        assert!(prepare(&manager, SESSION, args)
            .await
            .unwrap_err()
            .contains("stale"));
        assert!(manager.list().await.is_empty());
        manager.shutdown().await.unwrap();
    }
    #[tokio::test]
    async fn review_requires_exact_session_hash_and_refuses_expiry_without_transfer() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        let mut plan = Plan {
            id: Uuid::new_v4().to_string(),
            session_id: SESSION.into(),
            policy: POLICY.into(),
            hash: String::new(),
            created_at: now(),
            expires_at: "2000-01-01T00:00:00Z".into(),
            action: Action::GscloudDownload { scope: scope() },
        };
        plan.hash = plan.effective_hash().unwrap();
        write_record(&manager.inner.root, "plans", &plan.id, &plan)
            .await
            .unwrap();
        let state = manager.agent_plan_status(SESSION, &plan.id).await.unwrap();
        assert_eq!(state["status"], "expired");
        assert_eq!(state["archive"], true);
        assert!(manager
            .approve_agent_plan(SESSION, &plan.id, &"b".repeat(64))
            .await
            .unwrap_err()
            .contains("changed"));
        assert!(manager
            .approve_agent_plan("b1234567-1234-1234-1234-123456789abc", &plan.id, &plan.hash)
            .await
            .is_err());
        assert!(manager
            .approve_agent_plan(SESSION, &plan.id, &plan.hash)
            .await
            .unwrap_err()
            .contains("expired"));
        assert!(manager.list().await.is_empty());
        manager.shutdown().await.unwrap();
    }
}
