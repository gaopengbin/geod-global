//! Controlled task metadata and loopback responses; no live-provider claims.
use super::*;

fn project() -> crate::Project {
    serde_json::from_value(json!({"id":"11111111-1111-4111-8111-111111111111", "name":"Scoped fixture",
        "bounds":[0,0,1,1], "createdAt":"2025-01-01", "updatedAt":"2025-01-01",
        "scenes":[{"itemId":"SCENE", "date":"2025-01-01", "cloud":null,
            "crs":null, "gridCode":null,"bbox":[0,0,1,1],
            "assets":{"scl":{"href":"https://example.test/private.tif?token=secret","mediaType":"image/tiff"}}}]})).unwrap()
}
fn job(project: &crate::Project, status: JobStatus, date: &str) -> Job {
    let mut job = crate::new_download_job(CreateJobRequest {
        item_id: "SCENE".into(),
        asset_key: "scl".into(),
        href: project.scenes[0].assets["scl"].href.clone(),
        media_type: "image/tiff".into(),
        title: None,
    });
    job.status = status;
    job.created_at = date.into();
    job
}

#[test]
fn project_filter_is_optional_but_null_or_invalid_scope_cannot_become_global() {
    for value in [
        json!({"projectId":null}),
        json!({"projectId":"../projects"}),
        json!({"projectId":"FFFFFFFF-FFFF-4FFF-8FFF-FFFFFFFFFFFF"}),
        json!({"projectId":3}),
        json!({"projectId":project().id,"scope":"global"}),
    ] {
        assert!(parse_operation("geod_jobs_list", value, false).is_err());
    }
    assert!(parse_operation("geod_jobs_list", json!({}), false).is_ok());
    assert!(parse_operation("geod_jobs_list", json!({"projectId":project().id}), false).is_ok());
    let tools = agent_read_definitions();
    let tool = tools
        .iter()
        .find(|tool| tool["name"] == "geod_jobs_list")
        .unwrap();
    assert_eq!(
        tool["inputSchema"]["properties"]["projectId"]["type"],
        "string"
    );
    assert_eq!(tool["inputSchema"]["additionalProperties"], false);
}

#[tokio::test]
async fn desktop_scoped_pages_exclude_newer_foreign_jobs_and_keep_fresh_settlement() {
    let directory = tempfile::tempdir().unwrap();
    let manager = JobManager::open(directory.path()).await.unwrap();
    let project = project();
    manager
        .inner
        .projects
        .lock()
        .await
        .insert(project.id.clone(), project.clone());
    let running = job(&project, JobStatus::Running, "2025-01-03");
    let failed = job(&project, JobStatus::Failed, "2025-01-02");
    let succeeded = job(&project, JobStatus::Succeeded, "2025-01-01");
    {
        let mut store = manager.inner.store.lock().await;
        for own in [&running, &failed, &succeeded] {
            store.jobs.insert(own.id.clone(), own.clone());
        }
        // A completed metadata state can precede actual worker settlement.
        store.active.insert(
            succeeded.id.clone(),
            tokio_util::sync::CancellationToken::new(),
        );
        for _ in 0..8 {
            let mut foreign = job(&project, JobStatus::Failed, "2025-02-01");
            foreign.href.push_str("&unrelated=1");
            store.jobs.insert(foreign.id.clone(), foreign);
        }
    }
    let before = serde_json::to_value(manager.list().await).unwrap();
    let read = |offset| {
        agent_read_call(
            manager.clone(),
            "geod_jobs_list",
            json!({"projectId":project.id,"limit":1,"offset":offset}),
        )
    };
    let page = read(0).await.unwrap();
    assert_eq!(page["total"], 3);
    assert_eq!(page["nextOffset"], 1);
    assert_eq!(page["jobs"][0]["id"], running.id);
    assert_eq!(page["jobs"][0]["settled"], false);
    assert_eq!(page["project"]["id"], project.id);
    assert!(page["checkedAt"].as_str().is_some());
    assert!(!page.to_string().contains("example.test"));
    assert!(!page.to_string().contains("secret"));
    let page = read(1).await.unwrap();
    assert_eq!(page["jobs"][0]["id"], failed.id);
    assert_eq!(page["jobs"][0]["settled"], true);
    let page = read(2).await.unwrap();
    assert_eq!(page["jobs"][0]["id"], succeeded.id);
    assert_eq!(page["jobs"][0]["settled"], false);
    manager
        .inner
        .store
        .lock()
        .await
        .active
        .remove(&succeeded.id);
    assert_eq!(read(2).await.unwrap()["jobs"][0]["settled"], true);
    let exhausted = read(3).await.unwrap();
    assert_eq!(exhausted["total"], 3);
    assert_eq!(exhausted["jobs"], json!([]));
    assert_eq!(serde_json::to_value(manager.list().await).unwrap(), before);
    let global = agent_read_call(manager.clone(), "geod_jobs_list", json!({"limit":1}))
        .await
        .unwrap();
    assert_eq!(global["total"], 11);
    assert_ne!(global["jobs"][0]["id"], running.id);
    assert!(agent_read_call(
        manager.clone(),
        "geod_jobs_list",
        json!({"projectId":"22222222-2222-4222-8222-222222222222"})
    )
    .await
    .is_err());
    manager.shutdown().await.unwrap();
}

#[tokio::test]
async fn loopback_scope_refreshes_each_returned_task_and_refuses_old_runtime_contract() {
    let project = project();
    let stale = job(&project, JobStatus::Queued, "2025-01-01");
    let mut fresh = serde_json::to_value(&stale).unwrap();
    fresh["status"] = json!("succeeded");
    fresh["settled"] = json!(true);
    let list = json!([stale]);
    let projects = json!([project]);
    let status = fresh.clone();
    let app = axum::Router::new()
        .route(
            "/jobs",
            axum::routing::get(move || {
                let list = list.clone();
                async move { axum::Json(list) }
            }),
        )
        .route(
            "/projects",
            axum::routing::get(move || {
                let projects = projects.clone();
                async move { axum::Json(projects) }
            }),
        )
        .route(
            "/jobs/{id}",
            axum::routing::get(move || {
                let status = status.clone();
                async move { axum::Json(status) }
            }),
        );
    let (backend, server) = tests::fake_backend(app).await;
    let result = backend
        .execute(parse_operation("geod_jobs_list", json!({"projectId":project.id}), false).unwrap())
        .await
        .unwrap();
    assert_eq!(result["jobs"][0], fresh);
    assert_eq!(result["total"], 1);
    server.abort();
    let app = axum::Router::new()
        .route(
            "/jobs",
            axum::routing::get(move || {
                let stale = stale.clone();
                async move { axum::Json(json!([stale])) }
            }),
        )
        .route(
            "/projects",
            axum::routing::get(move || {
                let project = project.clone();
                async move { axum::Json(json!([project])) }
            }),
        )
        .route(
            "/jobs/{id}",
            axum::routing::get(move || {
                let mut status = fresh.clone();
                status.as_object_mut().unwrap().remove("settled");
                async move { axum::Json(status) }
            }),
        );
    let (backend, server) = tests::fake_backend(app).await;
    assert!(backend
        .execute(
            parse_operation(
                "geod_jobs_list",
                json!({"projectId":"11111111-1111-4111-8111-111111111111"}),
                false
            )
            .unwrap()
        )
        .await
        .unwrap_err()
        .contains("settlement"));
    server.abort();
}
