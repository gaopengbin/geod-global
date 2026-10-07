//! Two opt-in live text turns against one real native transfer, with a declared
//! loopback transport fault. No injected job states and no completed-file claim.
use super::super::connection_model_tests::{checked_turn, OwnedNativeVault};
use super::super::registry::{NativeVault, Vault};
use super::super::*;
use geod_runtime::{CreateJobRequest, Job, JobStatus, ProxySettings};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

fn verify_state_turn(
    snapshot: &Value,
    previous: Option<&Value>,
    project: &str,
    name: &str,
    status: &str,
    settled: bool,
) -> String {
    assert_eq!(snapshot["selected"]["status"], "completed");
    let after = previous.map_or(0, |value| {
        value["selected"]["entries"].as_array().unwrap().len()
    });
    let entries = snapshot["selected"]["entries"].as_array().unwrap();
    let fresh = &entries[after..];
    for name in [
        "geod_workspace_context",
        "geod_project_get",
        "geod_jobs_list",
    ] {
        assert!(
            fresh.iter().any(|entry| {
                entry["type"] == "tool" && entry["name"] == name && entry["status"] == "completed"
            }),
            "Missing fresh native read {name}"
        );
    }
    assert!(
        fresh
            .iter()
            .filter(|entry| entry["type"] == "tool")
            .all(|entry| {
                entry["status"] == "completed"
                    && [
                        "geod_workspace_context",
                        "geod_project_get",
                        "geod_jobs_list",
                        "geod_job_status",
                        "geod_health",
                        "geod_projects_list",
                    ]
                    .contains(&entry["name"].as_str().unwrap())
            }),
        "Status inquiry attempted a plan or write"
    );
    let jobs = fresh
        .iter()
        .rev()
        .find(|entry| entry["name"] == "geod_jobs_list")
        .unwrap();
    assert_eq!(jobs["summary"]["projectId"], project);
    assert_eq!(jobs["summary"]["total"], 1);
    assert_eq!(jobs["summary"]["count"], 1);
    let mut expected_statuses = json!({});
    expected_statuses[status] = json!(1);
    assert_eq!(jobs["summary"]["pageStatuses"], expected_statuses);
    assert_eq!(jobs["summary"]["pageSettledCount"], usize::from(settled));
    let answer = fresh
        .iter()
        .rev()
        .find(|entry| entry["type"] == "assistant")
        .unwrap()["text"]
        .as_str()
        .unwrap()
        .to_string();
    assert!(
        answer.contains(name),
        "Answer did not read the current project name"
    );
    assert!(
        answer.chars().count() <= 400,
        "Status answer was not concise"
    );
    if status == "running" {
        assert!(
            ["进行", "下载中", "运行", "正在", "未完成"]
                .iter()
                .any(|word| answer.contains(word)),
            "Missing running state"
        );
    } else {
        assert!(
            answer.contains("失败") || answer.contains("failed"),
            "Missing actual failed state"
        );
    }
    answer
}

async fn wait_state(manager: &JobManager, id: &str, expected: JobStatus, settled: bool) -> Job {
    tokio::time::timeout(Duration::from_secs(60), async {
        loop {
            let (job, ready) = manager.get_with_settled(id).await.unwrap();
            if job.status == expected && ready == settled && (settled || job.bytes_downloaded > 0) {
                return job;
            }
            assert!(
                matches!(job.status, JobStatus::Queued | JobStatus::Running)
                    || job.status == expected,
                "Transfer reached an unexpected state"
            );
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .expect("Native transfer did not reach the bounded expected state")
}

async fn trip_owned_transport(port: u16) {
    tokio::time::timeout(Duration::from_secs(5), async {
        let mut stream = tokio::net::TcpStream::connect(("127.0.0.1", port)).await.unwrap();
        stream.write_all(b"POST /task-fault/trip HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").await.unwrap();
        let mut bytes = vec![0; 8192];
        let size = stream.read(&mut bytes).await.unwrap();
        let header = std::str::from_utf8(&bytes[..size]).unwrap();
        assert!(header.starts_with("HTTP/1.0 200") || header.starts_with("HTTP/1.1 200"));
    }).await.expect("Owned fault control did not respond")
}

#[tokio::test]
#[ignore = "Two live read-only model turns; one public source transfer through an explicitly controlled transport fault"]
async fn live_active_failed_tasks_are_native_read_only_and_resume() {
    assert_eq!(
        std::env::var("GEOD_AGENT_TASK_FAULT_SCENARIO").unwrap(),
        "1"
    );
    assert_eq!(std::env::var("GEOD_AGENT_TEST_NATIVE_VAULT").unwrap(), "1");
    let secret = Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").unwrap());
    let model = std::env::var("GEOD_AGENT_TEST_MODEL").unwrap();
    assert_eq!(model, "deepseek-v4-flash");
    let endpoint = std::env::var("GEOD_AGENT_TEST_BASE_URL").unwrap();
    let proxy = std::env::var("GEOD_AGENT_TASK_PROXY").unwrap();
    let proxy_url = url::Url::parse(&proxy).unwrap();
    assert_eq!(proxy_url.scheme(), "http");
    assert_eq!(proxy_url.host_str(), Some("127.0.0.1"));
    assert!(proxy_url.port().is_some());
    let control: u16 = std::env::var("GEOD_AGENT_TASK_FAULT_CONTROL_PORT")
        .unwrap()
        .parse()
        .unwrap();
    assert!(control > 0);
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let requested = PathBuf::from(std::env::var("GEOD_AGENT_CONNECTION_QA").unwrap());
    assert!(requested.is_absolute() && !requested.exists());
    let parent = requested.parent().unwrap().canonicalize().unwrap();
    assert!(parent.starts_with(root.join(".verification").canonicalize().unwrap()));
    let base = parent.join(requested.file_name().unwrap());
    let source = PathBuf::from(std::env::var("GEOD_AGENT_TASK_JOBS").unwrap())
        .canonicalize()
        .unwrap();
    assert!(source.starts_with(root.join(".verification")));
    let source_bytes = tokio::fs::read(source).await.unwrap();
    let records: HashMap<String, Job> = serde_json::from_slice(&source_bytes).unwrap();
    let original = records
        .values()
        .find(|job| {
            job.kind == "download" && job.asset_key == "scl" && job.status == JobStatus::Succeeded
        })
        .unwrap();
    let source_url = geod_runtime::validate_asset_url(&original.href).unwrap();
    assert_eq!(
        source_url.host_str(),
        Some("sentinel-cogs.s3.us-west-2.amazonaws.com")
    );
    assert!(original.bytes_downloaded > 256 * 1024);
    let core = base.join("core");
    let manager = JobManager::open(&core).await.unwrap();
    assert!(manager.list().await.is_empty());
    let accounts_before = serde_json::to_value(manager.provider_accounts().await).unwrap();
    let source_proxy: ProxySettings =
        serde_json::from_value(json!({"mode":"custom","url":proxy})).unwrap();
    let settings = manager.save_proxy_settings(source_proxy).await.unwrap();
    let nonce = &super::super::tests::uuid_for_test()[..8];
    let running_name = format!("进行中工程-{nonce}");
    let failed_name = format!("故障后工程-{nonce}");
    let bounds = [-122.55, 37.68, -122.32, 37.84];
    let project = manager
        .create_project(geod_runtime::CreateProjectRequest {
            name: running_name.clone(),
            bounds,
            geometry: None,
            scenes: vec![geod_runtime::projects::ProjectScene {
                footprint: None,
                item_id: original.item_id.clone(),
                date: "2025-07-07T00:00:00Z".into(),
                cloud: None,
                crs: Some("EPSG:32610".into()),
                grid_code: Some("10SEG".into()),
                bbox: bounds,
                assets: std::collections::BTreeMap::from([(
                    "scl".into(),
                    geod_runtime::projects::ProjectAsset {
                        href: original.href.clone(),
                        media_type: original.media_type.clone(),
                        raster_band: None,
                    },
                )]),
            }],
        })
        .await
        .unwrap();
    // A normal native queue submission, not a model write or fabricated job record.
    let job = manager
        .create(CreateJobRequest {
            item_id: original.item_id.clone(),
            asset_key: original.asset_key.clone(),
            href: original.href.clone(),
            media_type: original.media_type.clone(),
            title: Some("实际传输·受控网络故障·SCL".into()),
        })
        .await
        .unwrap();
    let running = wait_state(&manager, &job.id, JobStatus::Running, false).await;
    assert_eq!(running.total_bytes, Some(original.bytes_downloaded));
    assert!(running.bytes_downloaded < original.bytes_downloaded);
    assert!(running.output_path.is_none() && running.sha256.is_none());
    let context = json!({"page":"My Data","provider":"earth-search","bounds":bounds,
        "start":"2025-07-01","end":"2025-07-31","cloudMax":100,"projectId":project.id});
    let home = base.join("agent");
    let runtime = root.join(".agent-runtime/win32-x64");
    verify_runtime(&runtime).await.unwrap();
    let vault = Arc::new(OwnedNativeVault::default());
    let agent = DesktopAgent::open_with_vault(
        home.clone(),
        runtime.clone(),
        manager.clone(),
        vault.clone(),
    )
    .await
    .unwrap();
    let saved = agent
        .save_model(
            serde_json::from_value(json!({"provider":"deepseek",
        "label":"Owned task fault route","protocol":"openai-compatible","baseUrl":endpoint,
        "model":model,"apiKey":secret.as_str()}))
            .unwrap(),
        )
        .await
        .unwrap();
    let connection_id = saved["registry"]["selectedId"]
        .as_str()
        .unwrap()
        .to_string();
    agent.operation("send", json!({"text":"这个工程当前叫什么，下载到哪一步了？查最新本地状态，用两句话回答，不准备方案或开始下载。","context":context})).await.unwrap();
    let connection = agent.0.state.lock().await.connection.clone().unwrap();
    let first = tokio::time::timeout(Duration::from_secs(90), checked_turn(&connection, &base))
        .await
        .unwrap();
    let first_answer =
        verify_state_turn(&first, None, &project.id, &running_name, "running", false);
    let still_running = manager.get_with_settled(&job.id).await.unwrap();
    assert_eq!(still_running.0.status, JobStatus::Running);
    assert!(!still_running.1);
    trip_owned_transport(control).await;
    let failed = wait_state(&manager, &job.id, JobStatus::Failed, true).await;
    assert!(failed.bytes_downloaded > 0 && failed.bytes_downloaded < original.bytes_downloaded);
    assert!(failed.output_path.is_none() && failed.sha256.is_none() && failed.error.is_some());
    assert!(manager.inspect_raster(&job.id).await.is_err());
    assert!(core
        .join("assets")
        .join(format!("{}.part", job.id))
        .exists());
    let failed_before_reads = serde_json::to_value(manager.list().await).unwrap();
    drop(connection);
    agent.shutdown().await;
    drop(agent);
    manager
        .rename_project(&project.id, &failed_name)
        .await
        .unwrap();
    let agent =
        DesktopAgent::open_with_vault(home.clone(), runtime, manager.clone(), vault.clone())
            .await
            .unwrap();
    agent
        .save_model(serde_json::from_value(json!({"action":"select","id":connection_id})).unwrap())
        .await
        .unwrap();
    agent.operation("send", json!({"sessionId":first["selected"]["id"],
        "text":"软件刚重启，这个工程现在叫什么，下载状态怎么样？查当前真实任务，用两句话回答，不重新下载。","context":context})).await.unwrap();
    let connection = agent.0.state.lock().await.connection.clone().unwrap();
    let second = tokio::time::timeout(Duration::from_secs(90), checked_turn(&connection, &base))
        .await
        .unwrap();
    let second_answer = verify_state_turn(
        &second,
        Some(&first),
        &project.id,
        &failed_name,
        "failed",
        true,
    );
    assert_eq!(second["selected"]["id"], first["selected"]["id"]);
    assert_eq!(
        second["selected"]["threadId"],
        first["selected"]["threadId"]
    );
    assert_eq!(
        serde_json::to_value(manager.list().await).unwrap(),
        failed_before_reads
    );
    assert_eq!(
        serde_json::to_value(manager.provider_accounts().await).unwrap(),
        accounts_before
    );
    assert_eq!(manager.proxy_settings().await, settings);
    let registry: Value =
        serde_json::from_slice(&tokio::fs::read(home.join("registry.json")).await.unwrap())
            .unwrap();
    let reference = registry["connections"]
        .as_array()
        .unwrap()
        .iter()
        .find(|value| value["id"] == connection_id)
        .unwrap()["credentialRef"]
        .as_str()
        .unwrap();
    assert!(vault
        .read(reference)
        .unwrap()
        .is_some_and(|value| value.as_str() == secret.as_str()));
    agent
        .save_model(serde_json::from_value(json!({"action":"delete","id":connection_id})).unwrap())
        .await
        .unwrap();
    assert!(vault.read(reference).unwrap().is_none());
    drop(connection);
    let receipt = json!({"schema":"geod-agent-active-failed-task-model/v1","status":"passed",
        "scope":"One normal native public-SCL transfer receives real source bytes, then deliberately loses its owned HTTPS tunnel. This is a controlled transport fault, not a natural provider outage or completed raster download.",
        "model":model,"protocol":"openai-compatible","modelTurns":2,"upstreamVendorVerified":false,
        "nativeJobsCreated":1,"jobStatesInjected":false,"modelWrites":0,
        "sourceJobsSha256":format!("{:x}",Sha256::digest(source_bytes)),
        "originalHref":original.href,"projectId":project.id,"jobId":job.id,
        "runningObservation":running,"runningAfterAnswer":still_running.0,"failedObservation":failed,
        "sameConversationAndThreadResumed":true,"nativeJobsUnchangedByFailedStatusInquiry":true,
        "providerAccountsUnchanged":true,"ownedSourceProxyUnchangedByModel":true,
        "ownedVaultEntriesWritten":1,"ownedVaultEntriesDeletedAndReadAbsent":true,
        "usedUserDesktop":false,"nativeWindowTested":false,"installerCreated":false,"published":false,
        "turns":[first,second],"answers":[first_answer,second_answer]});
    tokio::fs::write(
        base.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
}

fn owned_rollouts(directory: &std::path::Path, files: &mut Vec<PathBuf>) {
    for entry in std::fs::read_dir(directory).unwrap() {
        let path = entry.unwrap().path();
        assert!(!path.is_symlink());
        if path.is_dir() {
            owned_rollouts(&path, files);
        } else if path
            .extension()
            .is_some_and(|extension| extension == "jsonl")
        {
            files.push(path);
            assert!(files.len() <= 16);
        }
    }
}

#[tokio::test]
#[ignore = "Read-only audit of two already recorded live turns and the actual native partial file; no models or new transfers"]
async fn recorded_active_failed_workflow_is_consistent_with_native_files() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let run = PathBuf::from(std::env::var("GEOD_AGENT_FAULT_RECORDING").unwrap())
        .canonicalize()
        .unwrap();
    assert_eq!(
        run.parent().unwrap(),
        root.join(".verification").canonicalize().unwrap()
    );
    assert!(run
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("agent-project-task-model-fault-"));
    let launch: Value =
        serde_json::from_slice(&tokio::fs::read(run.join("launch.json")).await.unwrap()).unwrap();
    // Preserve the original failed assertion as evidence, rather than relabel it green.
    assert_eq!(launch["nativeExit"], 101);
    assert_eq!(launch["status"], "failed");
    assert_eq!(launch["modelRequests"].as_array().unwrap().len(), 5);
    assert!(launch["modelRequests"]
        .as_array()
        .unwrap()
        .iter()
        .all(|request| {
            request["model"] == "deepseek-v4-flash"
                && request["httpStatus"] == 200
                && request["status"] == "completed"
                && request["toolDefinitionCount"] == 48
        }));
    assert_eq!(launch["ownedVaultCleanupReadVerified"], true);
    assert_eq!(launch["ownedTunnelStopped"], true);
    assert_eq!(launch["ownedServersStopped"], true);
    assert_eq!(launch["jobStatesInjected"], false);
    assert!(launch["sourceTunnels"]
        .as_array()
        .unwrap()
        .iter()
        .any(|tunnel| {
            tunnel["controlledAbort"] == true
                && tunnel["encryptedSourceBytesForwarded"].as_u64().unwrap() > 65536
        }));
    let base = run.join("case");
    let core = base.join("core");
    let home = base.join("agent");
    let sessions: Value =
        serde_json::from_slice(&tokio::fs::read(home.join("sessions.json")).await.unwrap())
            .unwrap();
    let session = &sessions["sessions"].as_array().unwrap()[0];
    assert_eq!(sessions["sessions"].as_array().unwrap().len(), 1);
    assert_eq!(session["status"], "completed");
    let entries = session["entries"].as_array().unwrap();
    let user_indices: Vec<usize> = entries
        .iter()
        .enumerate()
        .filter_map(|(index, entry)| (entry["type"] == "user").then_some(index))
        .collect();
    assert_eq!(user_indices.len(), 2);
    assert_eq!(user_indices[0], 0);
    let first_jobs = entries[..user_indices[1]]
        .iter()
        .find(|entry| entry["name"] == "geod_jobs_list")
        .unwrap();
    let project_id = first_jobs["summary"]["projectId"].as_str().unwrap();
    let before: Value =
        serde_json::from_slice(&tokio::fs::read(core.join("jobs.json")).await.unwrap()).unwrap();
    assert_eq!(before.as_object().unwrap().len(), 1);
    let manager = JobManager::open(&core).await.unwrap();
    let local = geod_runtime::agent_actions::call(
        manager.clone(),
        session["id"].as_str().unwrap(),
        "geod_jobs_list",
        json!({"projectId":project_id,"limit":5}),
        None,
    )
    .await
    .unwrap();
    assert_eq!(local["total"], 1);
    assert_eq!(local["jobs"][0]["status"], "failed");
    assert_eq!(local["jobs"][0]["settled"], true);
    let failed_name = local["project"]["name"].as_str().unwrap();
    let suffix = failed_name.strip_prefix("故障后工程-").unwrap();
    let running_name = format!("进行中工程-{suffix}");
    // These wrappers only replay the recorded application history in the checker.
    // They are not injected runtime snapshots, tasks or simulated model turns.
    let first = json!({"selected":{"status":"completed","entries":entries[..user_indices[1]]}});
    let second = json!({"selected":session});
    let first_answer = verify_state_turn(&first, None, project_id, &running_name, "running", false);
    let second_answer = verify_state_turn(
        &second,
        Some(&first),
        project_id,
        failed_name,
        "failed",
        true,
    );
    let jobs = manager.list().await;
    assert_eq!(jobs.len(), 1);
    let job = &jobs[0];
    let (current, settled) = manager.get_with_settled(&job.id).await.unwrap();
    assert_eq!(current.status, JobStatus::Failed);
    assert!(settled && current.error.is_some());
    assert!(current.sha256.is_none() && current.output_path.is_none());
    assert!(manager.inspect_raster(&current.id).await.is_err());
    let partial = core.join("assets").join(format!("{}.part", job.id));
    let received = tokio::fs::read(&partial).await.unwrap();
    assert_eq!(received.len() as u64, current.bytes_downloaded);
    assert!(received.len() > 65536 && (received.len() as u64) < current.total_bytes.unwrap());
    assert!(!core.join("assets").join(format!("{}.tif", job.id)).exists());
    let original_records: HashMap<String, Job> = serde_json::from_slice(
        &tokio::fs::read(root.join(".verification/cli-e2e-roundtrip/store/jobs.json"))
            .await
            .unwrap(),
    )
    .unwrap();
    let original = original_records
        .values()
        .find(|source| {
            source.kind == "download"
                && source.asset_key == "scl"
                && source.href == current.href
                && source.status == JobStatus::Succeeded
        })
        .unwrap();
    let original_path = PathBuf::from(original.output_path.as_ref().unwrap())
        .canonicalize()
        .unwrap();
    assert!(original_path.starts_with(root.join(".verification")));
    let original_bytes = tokio::fs::read(original_path).await.unwrap();
    assert_eq!(original_bytes.len() as u64, original.bytes_downloaded);
    assert_eq!(current.total_bytes, Some(original.bytes_downloaded));
    assert!(received.len() < original_bytes.len());
    assert_eq!(
        format!("{:x}", Sha256::digest(&original_bytes)),
        original.sha256.as_ref().unwrap().as_str()
    );
    assert_eq!(
        received.as_slice(),
        &original_bytes[..received.len()],
        "Partial bytes differ from the independently retained real source"
    );
    let mut rollouts = Vec::new();
    owned_rollouts(&home.join("codex/sessions"), &mut rollouts);
    assert_eq!(rollouts.len(), 1);
    let transcript = tokio::fs::read_to_string(&rollouts[0]).await.unwrap();
    assert!(transcript.len() <= 2 * 1024 * 1024);
    let rows: Vec<Value> = transcript
        .lines()
        .map(|line| serde_json::from_str(line).unwrap())
        .collect();
    let metadata: Vec<&Value> = rows
        .iter()
        .filter(|row| row["type"] == "session_meta")
        .collect();
    assert_eq!(metadata.len(), 1);
    assert_eq!(metadata[0]["payload"]["id"], session["threadId"]);
    assert_eq!(
        rows.iter()
            .filter(|row| row["type"] == "turn_context")
            .count(),
        2
    );
    for event in ["task_started", "task_complete", "thread_settings_applied"] {
        assert_eq!(
            rows.iter()
                .filter(|row| row["type"] == "event_msg" && row["payload"]["type"] == event)
                .count(),
            2
        );
    }
    let registry: super::super::registry::Registry =
        serde_json::from_slice(&tokio::fs::read(home.join("registry.json")).await.unwrap())
            .unwrap();
    let connections = &registry.connections;
    assert_eq!(connections.len(), 1);
    assert_eq!(connections[0].settings.label, "Owned task fault route");
    let reference = connections[0].credential_ref.as_str();
    assert!(reference.strip_prefix("connection-").is_some_and(|id| {
        id.len() == 36
            && id
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() || byte == b'-')
    }));
    let cleanup: Value = serde_json::from_slice(
        &tokio::fs::read(run.join("owned-vault-cleanup.json"))
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(cleanup["status"], "passed");
    assert_eq!(cleanup["scopedReferencesRead"], 1);
    assert_eq!(cleanup["scopedReferencesAbsent"], 1);
    assert!(NativeVault.read(reference).unwrap().is_none());
    manager.shutdown().await.unwrap();
    let after: Value =
        serde_json::from_slice(&tokio::fs::read(core.join("jobs.json")).await.unwrap()).unwrap();
    assert_eq!(before, after);
    let receipt = json!({"schema":"geod-agent-recorded-active-failed-workflow/v1","status":"passed",
        "scope":"Read-only audit of the two actual model turns and real native transfer retained by the original run. The original native test remains failed because it required the literal Chinese word for failure; the actual reply explicitly used status=failed and not successful.",
        "originalNativeTestExit":101,"originalFailureRetained":true,
        "newModelCalls":0,"newTransfers":0,"jobStatesInjected":false,
        "actualModelTurns":2,"actualModelRequests":5,"actualNativeModelReads":entries.iter().filter(|entry| entry["type"]=="tool").count(),
        "model":"deepseek-v4-flash","protocol":"openai-compatible","upstreamVendorVerified":false,
        "projectId":project_id,"jobId":job.id,"partialBytes":received.len(),"totalBytes":current.total_bytes,
        "partialSha256":format!("{:x}",Sha256::digest(&received)),"partialMatchesRetainedRealSourcePrefix":true,
        "failedTaskSettled":true,"noFinalArtifact":true,"partialRasterInspectionRejected":true,
        "singleCodexThreadHasBothCompletedTurns":true,"sameConversationResumed":true,
        "nativeJobsUnchangedByAudit":true,"ownedCredentialReadAbsent":true,
        "usedUserDesktop":false,"nativeWindowTested":false,"installerCreated":false,"published":false,
        "answers":[first_answer,second_answer]});
    tokio::fs::write(
        run.join("recorded-workflow-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
}
