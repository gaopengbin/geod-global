//! Opt-in public vector and actual model acceptance in isolated native stores.
//! Read-only and review-confirm-extract scenarios are recorded separately.
use super::tests::{assistant_text, completed};
use super::*;

fn calls(snapshot: &Value, name: &str) -> usize {
    snapshot["selected"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| {
            entry["type"] == "tool" && entry["status"] == "completed" && entry["name"] == name
        })
        .count()
}
fn first_position(geometry: &Value) -> (String, Value) {
    let mut value = &geometry["coordinates"];
    let mut pointer = "/geometry/coordinates".to_string();
    loop {
        let values = value
            .as_array()
            .expect("Actual public polygon coordinate array");
        if values.first().is_some_and(Value::is_number) {
            return (pointer, value.clone());
        }
        value = values.first().expect("Nonempty public polygon");
        pointer.push_str("/0");
    }
}

#[derive(serde::Deserialize, serde::Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct VectorSourceCase {
    name: String,
    url: String,
    protocol: String,
    collection_id: String,
    bounds: [f64; 4],
    page_size: Option<usize>,
    response_format: Option<String>,
}

fn label_node(feature: &Value) -> (String, String) {
    if let Some((key, value)) = feature["properties"].as_object().and_then(|p| {
        p.iter().find(|(k, v)| {
            k.to_lowercase().contains("name")
                && v.as_str().is_some_and(|s| !s.is_empty() && s.len() < 160)
        })
    }) {
        return (
            format!("/properties/{}", key.replace('~', "~0").replace('/', "~1")),
            value.as_str().unwrap().into(),
        );
    }
    if let Some(name) = feature
        .pointer("/properties/tags/name")
        .and_then(Value::as_str)
    {
        return ("/properties/tags/name".into(), name.into());
    }
    let id = &feature["id"];
    assert!(
        id.is_string() || id.is_number(),
        "An actual feature identity is required"
    );
    (
        "/id".into(),
        id.as_str()
            .map(str::to_string)
            .unwrap_or_else(|| id.to_string()),
    )
}

#[tokio::test]
#[ignore = "Explicit live model credential, owned runtime and fresh public OGC vector extraction"]
async fn live_vector_reads_and_resume() {
    let secret = Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("explicit test key"));
    let endpoint =
        std::env::var("GEOD_AGENT_TEST_BASE_URL").unwrap_or("http://127.0.0.1:19094/v1".into());
    let model = std::env::var("GEOD_AGENT_TEST_MODEL").unwrap_or("deepseek-v4-flash".into());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let requested = PathBuf::from(
        std::env::var("GEOD_AGENT_VECTOR_MODEL_QA").expect("fresh isolated QA directory"),
    );
    assert!(requested.is_absolute());
    let parent = requested.parent().unwrap().canonicalize().unwrap();
    assert!(parent.starts_with(root.join(".verification").canonicalize().unwrap()));
    let base = parent.join(requested.file_name().unwrap());
    assert!(!base.exists());
    tokio::fs::create_dir_all(&base).await.unwrap();
    let home = base.join("sessions");
    tokio::fs::create_dir_all(&home).await.unwrap();
    let manager = JobManager::open(base.join("core")).await.unwrap();
    if let Ok(proxy) = std::env::var("GEOD_AGENT_TEST_PROXY") {
        manager
            .save_proxy_settings(geod_runtime::ProxySettings {
                mode: geod_runtime::proxy::ProxyMode::Custom,
                url: Some(proxy),
            })
            .await
            .unwrap();
    }
    let service = manager
        .connect_feature_service(geod_runtime::features::ConnectRequest {
            name: "Live Agent vector read acceptance".into(),
            url: "https://demo.pygeoapi.io/stable".into(),
            protocol: "OGC".into(),
        })
        .await
        .unwrap();
    let asset = manager
        .query_features(geod_runtime::features::QueryRequest {
            service_id: service.id.clone(),
            collection_id: "lakes".into(),
            bounds: [-130.0, 20.0, -60.0, 65.0],
            area_geometry: None,
            page_size: Some(2),
            response_format: None,
        })
        .await
        .unwrap();
    let original = manager.inspect_vector(&asset.id).await.unwrap();
    assert!(asset.feature_count > 0);
    let first = &original.geojson["features"][0];
    let (key, label) = first["properties"]
        .as_object()
        .unwrap()
        .iter()
        .find(|(key, value)| {
            key.to_lowercase().contains("name")
                && value
                    .as_str()
                    .is_some_and(|v| !v.is_empty() && v.len() < 160)
        })
        .expect("Actual public feature name");
    let name = label.as_str().unwrap().to_string();
    let label_pointer = format!("/properties/{}", key.replace('~', "~0").replace('/', "~1"));
    let (geometry_pointer, position) = first_position(&first["geometry"]);
    tokio::fs::write(
        base.join("original-inspection.json"),
        serde_json::to_vec_pretty(&original).unwrap(),
    )
    .await
    .unwrap();
    tokio::fs::write(
        base.join("saved-service.json"),
        serde_json::to_vec_pretty(&service).unwrap(),
    )
    .await
    .unwrap();
    let registry_before = tokio::fs::read(base.join("core/vectors.json"))
        .await
        .unwrap();
    let services_before = tokio::fs::read(base.join("core/feature-services.json"))
        .await
        .unwrap();
    let runtime = root.join(".agent-runtime/win32-x64");
    verify_runtime(&runtime).await.unwrap();
    let config = json!({"label":"Isolated vector read acceptance","protocol":"openai-compatible","baseUrl":endpoint,"model":model,"apiKey":secret.as_str()});
    let connection = Connection::spawn(&runtime, &home, manager.clone())
        .await
        .unwrap();
    connection
        .rpc(
            "configure",
            json!({"config":config,"definitions":definitions()}),
        )
        .await
        .unwrap();
    let agent = DesktopAgent::open(home.clone(), runtime.clone(), manager.clone())
        .await
        .unwrap();
    agent.0.state.lock().await.connection = Some(connection.clone());
    let sent = agent.operation("send",json!({"text":format!("请核查已有矢量文件，禁止查询远程服务或创建任何文件。先调用 geod_feature_services 读取已连接服务；用服务 {} 调用 geod_feature_collections 读取真实 lakes 集合声明。调用 geod_vectors_list 查找现有文件 {}，geod_vector_inspect 校验它，geod_vector_features(limit=2) 读取真实首批要素，再用 geod_vector_node(feature=0,pointer={}) 读取名称，用 geod_vector_node(feature=0,pointer={}) 读取一个原生坐标位置。不要猜属性值或几何。最后只报告真实文件名、要素数、完整原文件 SHA-256、第一个要素名称、这个坐标位置及这些读取是否创建了文件。",service.id,asset.id,json!(label_pointer),json!(geometry_pointer)),"context":null})).await.unwrap();
    let session = sent["selected"]["id"].as_str().unwrap().to_string();
    let read = completed(&connection).await;
    assert_eq!(
        read["selected"]["status"], "completed",
        "{}",
        read["selected"]["error"]
    );
    for tool in [
        "geod_feature_services",
        "geod_feature_collections",
        "geod_vectors_list",
        "geod_vector_inspect",
        "geod_vector_features",
        "geod_vector_node",
    ] {
        assert!(calls(&read, tool) > 0, "Missing actual model call: {tool}");
    }
    assert!(assistant_text(&read).contains(&asset.source_sha256));
    assert!(assistant_text(&read).contains(&name));
    assert!(read["selected"]["entries"]
        .as_array()
        .unwrap()
        .iter()
        .any(|e| e["references"].as_array().is_some_and(|refs| refs
            .iter()
            .any(|r| r["kind"] == "vector" && r["id"] == asset.id))));
    let thread = read["selected"]["threadId"].clone();
    agent.shutdown().await;
    drop(agent);
    drop(connection);
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(base.join("core")).await.unwrap();
    let connection = Connection::spawn(&runtime, &home, reopened.clone())
        .await
        .unwrap();
    connection
        .rpc(
            "configure",
            json!({"config":config,"definitions":definitions()}),
        )
        .await
        .unwrap();
    let agent = DesktopAgent::open(home, runtime, reopened.clone())
        .await
        .unwrap();
    agent.0.state.lock().await.connection = Some(connection.clone());
    agent.operation("send",json!({"sessionId":session,"text":format!("软件已经重启。请重新调用 geod_vector_inspect 校验前一轮的同一个文件，并调用 geod_vector_features(limit=1) 和 geod_vector_node(feature=0,pointer={}) 重新读取第一个要素名称。只回答当前完整原文件 SHA-256、要素名称及是否校验通过；不要用对话记忆代替读取，不创建文件。",json!(label_pointer)),"context":null})).await.unwrap();
    let resumed = completed(&connection).await;
    assert_eq!(
        resumed["selected"]["status"], "completed",
        "{}",
        resumed["selected"]["error"]
    );
    assert_eq!(resumed["selected"]["id"], session);
    assert_eq!(resumed["selected"]["threadId"], thread);
    assert!(assistant_text(&resumed).contains(&asset.source_sha256));
    assert!(assistant_text(&resumed).contains(&name));
    for tool in [
        "geod_vector_inspect",
        "geod_vector_features",
        "geod_vector_node",
    ] {
        assert!(calls(&resumed, tool) > calls(&read, tool));
    }
    assert_eq!(
        tokio::fs::read(base.join("core/vectors.json"))
            .await
            .unwrap(),
        registry_before
    );
    assert_eq!(
        tokio::fs::read(base.join("core/feature-services.json"))
            .await
            .unwrap(),
        services_before
    );
    assert_eq!(reopened.list_vectors().await.len(), 1);
    assert!(reopened.list().await.is_empty());
    assert!(reopened.list_projects().await.is_empty());
    assert!(agent.snapshot().await.unwrap()["plans"]
        .as_array()
        .unwrap()
        .is_empty());
    let receipt = json!({"schema":"geod-agent-vector-model-acceptance/v1","status":"passed","modelRoute":model,"upstreamVendorVerified":false,
        "nativeTools":["geod_feature_services","geod_feature_collections","geod_vectors_list","geod_vector_inspect","geod_vector_features","geod_vector_node"],
        "serviceId":service.id,"asset":asset,"labelPointer":label_pointer,"geometryPointer":geometry_pointer,"firstName":name,"firstPosition":position,
        "harnessAcquiredPublicVector":true,"modelPerformedExtraction":false,"persistedVectorCount":1,"persistedJobCount":0,
        "unchangedVectorRegistry":true,"unchangedServiceRegistry":true,"resumedSameConversation":true,"resumedSameModelThread":true,
        "usedUserDesktop":false,"credentialVaultWritten":false,"published":false,"beforeRestart":read,"afterRestart":resumed});
    tokio::fs::write(
        base.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    reopened.shutdown().await.unwrap();
}

#[tokio::test]
#[ignore = "Explicit live model credential and native review confirmation in a fresh public OGC store"]
async fn live_vector_plan_and_resume() {
    let source: VectorSourceCase = std::env::var("GEOD_AGENT_VECTOR_SOURCE_QA")
        .ok()
        .map(|v| serde_json::from_str(&v).expect("explicit vector source test contract"))
        .unwrap_or(VectorSourceCase {
            name: "OGC public lakes".into(),
            url: "https://demo.pygeoapi.io/stable".into(),
            protocol: "OGC".into(),
            collection_id: "lakes".into(),
            bounds: [-130.0, 20.0, -60.0, 65.0],
            page_size: Some(2),
            response_format: None,
        });
    let reviewed_name = format!("Reviewed {}", source.name);
    let confirmed_name = format!("Confirmed {}", source.name);
    let secret = Zeroizing::new(std::env::var("GEOD_AGENT_TEST_KEY").expect("explicit test key"));
    let endpoint =
        std::env::var("GEOD_AGENT_TEST_BASE_URL").unwrap_or("http://127.0.0.1:19094/v1".into());
    let model = std::env::var("GEOD_AGENT_TEST_MODEL").unwrap_or("deepseek-v4-flash".into());
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let requested = PathBuf::from(
        std::env::var("GEOD_AGENT_VECTOR_MODEL_QA").expect("fresh isolated QA directory"),
    );
    assert!(requested.is_absolute());
    let parent = requested.parent().unwrap().canonicalize().unwrap();
    assert!(parent.starts_with(root.join(".verification").canonicalize().unwrap()));
    let base = parent.join(requested.file_name().unwrap());
    assert!(!base.exists());
    tokio::fs::create_dir_all(base.join("sessions"))
        .await
        .unwrap();
    let home = base.join("sessions");
    let manager = JobManager::open(base.join("core")).await.unwrap();
    if std::env::var("GEOD_AGENT_TEST_DIRECT").as_deref() == Ok("1") {
        manager
            .save_proxy_settings(geod_runtime::ProxySettings {
                mode: geod_runtime::proxy::ProxyMode::Direct,
                url: None,
            })
            .await
            .unwrap();
    } else if let Ok(proxy) = std::env::var("GEOD_AGENT_TEST_PROXY") {
        manager
            .save_proxy_settings(geod_runtime::ProxySettings {
                mode: geod_runtime::proxy::ProxyMode::Custom,
                url: Some(proxy),
            })
            .await
            .unwrap();
    }
    let service = manager
        .connect_feature_service(geod_runtime::features::ConnectRequest {
            name: format!("Live Agent {}", source.name),
            url: source.url.clone(),
            protocol: source.protocol.clone(),
        })
        .await
        .unwrap();
    tokio::fs::write(
        base.join("saved-service.json"),
        serde_json::to_vec_pretty(&service).unwrap(),
    )
    .await
    .unwrap();
    let services_before = tokio::fs::read(base.join("core/feature-services.json"))
        .await
        .unwrap();
    let runtime = root.join(".agent-runtime/win32-x64");
    verify_runtime(&runtime).await.unwrap();
    let config = json!({"label":"Isolated vector plan acceptance","protocol":"openai-compatible","baseUrl":endpoint,"model":model,"apiKey":secret.as_str()});
    let connection = Connection::spawn(&runtime, &home, manager.clone())
        .await
        .unwrap();
    connection
        .rpc(
            "configure",
            json!({"config":config,"definitions":definitions()}),
        )
        .await
        .unwrap();
    let agent = DesktopAgent::open(home.clone(), runtime.clone(), manager.clone())
        .await
        .unwrap();
    agent.0.state.lock().await.connection = Some(connection.clone());
    let mut arguments = json!({"serviceId":service.id,"collectionId":source.collection_id,"bounds":source.bounds,
        "pageSize":source.page_size,"name":reviewed_name,"responseFormat":source.response_format});
    if source.page_size.is_none() {
        arguments.as_object_mut().unwrap().remove("pageSize");
    }
    if source.response_format.is_none() {
        arguments.as_object_mut().unwrap().remove("responseFormat");
    }
    let sent = agent.operation("send",json!({"text":format!("先调用 geod_feature_services 和 geod_feature_collections 核查软件已经连接的服务 {} 及真实集合 {}，再调用一次 geod_vector_extract_plan，准确使用以下参数 JSON：{}。只创建待用户确认的方案，绝不要执行提取。不要猜数量、大小、下载状态，也不要创建工程或栅格任务。最后报告待确认、当前范围以及实际数量需要在确认提取后才能知道。", service.id,json!(source.collection_id),arguments),"context":null})).await.unwrap();
    let session = sent["selected"]["id"].as_str().unwrap().to_string();
    let prepared = completed(&connection).await;
    assert_eq!(
        prepared["selected"]["status"], "completed",
        "{}",
        prepared["selected"]["error"]
    );
    for tool in [
        "geod_feature_services",
        "geod_feature_collections",
        "geod_vector_extract_plan",
    ] {
        assert!(
            calls(&prepared, tool) > 0,
            "Missing actual model call: {tool}"
        );
    }
    let before = agent.snapshot().await.unwrap();
    assert_eq!(before["plans"].as_array().unwrap().len(), 1);
    let old = &before["plans"][0];
    assert_eq!(old["kind"], "vector");
    assert_eq!(old["status"], "pending");
    assert_eq!(old["bounds"], json!(source.bounds));
    assert_eq!(old["vectorReview"]["collectionId"], source.collection_id);
    if let Some(format) = &source.response_format {
        assert_eq!(old["vectorReview"]["responseFormat"], *format);
    }
    assert!(old["vector"].is_null() && old["expectedBytes"].is_null());
    assert_eq!(old["vectorReview"]["liveAvailabilityChecked"], false);
    assert!(manager.list_vectors().await.is_empty() && manager.list().await.is_empty());
    let old_id = old["planId"].as_str().unwrap().to_string();
    let old_hash = old["planHash"].as_str().unwrap().to_string();
    let form = agent
        .plan_revision_draft(&session, &old_id, &old_hash)
        .await
        .unwrap();
    assert_eq!(form["kind"], "vector");
    let edited = agent
        .revise_plan(
            &session,
            &old_id,
            &old_hash,
            agent_actions::PlanRevision::Vector {
                bounds: source.bounds,
                name: confirmed_name.clone(),
                keep_polygon: false,
            },
        )
        .await
        .unwrap();
    let next = edited["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["status"] == "pending")
        .unwrap();
    let plan_id = next["planId"].as_str().unwrap().to_string();
    let plan_hash = next["planHash"].as_str().unwrap().to_string();
    assert_ne!(plan_hash, old_hash);
    assert_eq!(
        next["vectorReview"]["serviceSha256"],
        old["vectorReview"]["serviceSha256"]
    );
    assert!(manager.list_vectors().await.is_empty());
    assert!(agent
        .approve_plan(&session, &old_id, &old_hash)
        .await
        .is_err());
    assert!(agent
        .approve_plan(&session, &plan_id, &"0".repeat(64))
        .await
        .is_err());
    // This explicit native confirmation emulates the isolated user's click.
    // The model tool cannot reach the approval/extraction path.
    let confirmed = agent
        .approve_plan(&session, &plan_id, &plan_hash)
        .await
        .unwrap();
    let final_plan = confirmed["plans"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["planId"] == plan_id)
        .unwrap();
    assert_eq!(final_plan["status"], "submitted");
    assert_eq!(final_plan["vector"]["verified"], true);
    let vector_id = final_plan["vector"]["id"].as_str().unwrap().to_string();
    let original = manager.inspect_vector(&vector_id).await.unwrap();
    let asset = original.asset.clone();
    assert_eq!(asset.name, confirmed_name);
    assert!(asset.feature_count > 0);
    assert_eq!(asset.agent_approval.as_ref().unwrap().plan_hash, plan_hash);
    tokio::fs::write(
        base.join("original-inspection.json"),
        serde_json::to_vec_pretty(&original).unwrap(),
    )
    .await
    .unwrap();
    let first = &original.geojson["features"][0];
    let (label_pointer, name) = label_node(first);
    let (geometry_pointer, position) = first_position(&first["geometry"]);
    let registry_after = tokio::fs::read(base.join("core/vectors.json"))
        .await
        .unwrap();
    agent
        .approve_plan(&session, &plan_id, &plan_hash)
        .await
        .unwrap();
    assert_eq!(
        tokio::fs::read(base.join("core/vectors.json"))
            .await
            .unwrap(),
        registry_after
    );
    agent.operation("send",json!({"sessionId":session,"text":format!("用户已在软件内确认修订方案 {}，请调用 geod_plan_status(planId={}) 核查，再用 geod_vectors_list 和 geod_vector_inspect 校验已登记文件 {}；调用 geod_vector_features(limit=2) 读取真实要素，并用 geod_vector_node(feature=0,pointer={}) 读取名称、geod_vector_node(feature=0,pointer={}) 读取原始位置。只报告实际文件名、要素数、完整原文件 SHA-256、首个要素名称和位置。不要重新创建方案或执行任务。",plan_id,json!(plan_id),vector_id,json!(label_pointer),json!(geometry_pointer)),"context":null})).await.unwrap();
    let read = completed(&connection).await;
    assert_eq!(
        read["selected"]["status"], "completed",
        "{}",
        read["selected"]["error"]
    );
    for tool in [
        "geod_plan_status",
        "geod_vectors_list",
        "geod_vector_inspect",
        "geod_vector_features",
        "geod_vector_node",
    ] {
        assert!(
            calls(&read, tool) > calls(&prepared, tool),
            "Missing post-confirmation call: {tool}"
        );
    }
    assert!(assistant_text(&read).contains(&asset.source_sha256));
    assert!(assistant_text(&read).contains(&name));
    let thread = read["selected"]["threadId"].clone();
    agent.shutdown().await;
    drop(agent);
    drop(connection);
    manager.shutdown().await.unwrap();
    drop(manager);
    let reopened = JobManager::open(base.join("core")).await.unwrap();
    let connection = Connection::spawn(&runtime, &home, reopened.clone())
        .await
        .unwrap();
    connection
        .rpc(
            "configure",
            json!({"config":config,"definitions":definitions()}),
        )
        .await
        .unwrap();
    let agent = DesktopAgent::open(home, runtime, reopened.clone())
        .await
        .unwrap();
    agent.0.state.lock().await.connection = Some(connection.clone());
    agent.operation("send",json!({"sessionId":session,"text":format!("软件已重启，请重新调用 geod_plan_status(planId={}) 检查同一个已确认方案，geod_vector_inspect 校验同一个文件 {}，geod_vector_features(limit=1) 和 geod_vector_node(feature=0,pointer={}) 读取首个要素的真实名称。报告当前完整原文件 SHA-256、要素名称和校验状态，不依赖旧回复，不再执行提取。",json!(plan_id),vector_id,json!(label_pointer)),"context":null})).await.unwrap();
    let resumed = completed(&connection).await;
    assert_eq!(
        resumed["selected"]["status"], "completed",
        "{}",
        resumed["selected"]["error"]
    );
    assert_eq!(resumed["selected"]["id"], session);
    assert_eq!(resumed["selected"]["threadId"], thread);
    assert!(assistant_text(&resumed).contains(&asset.source_sha256));
    assert!(assistant_text(&resumed).contains(&name));
    for tool in [
        "geod_plan_status",
        "geod_vector_inspect",
        "geod_vector_features",
        "geod_vector_node",
    ] {
        assert!(calls(&resumed, tool) > calls(&read, tool));
    }
    for snapshot in [&prepared, &read, &resumed] {
        assert!(!snapshot["selected"]["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|e| e["type"] == "tool" && e["status"] == "failed"));
    }
    agent
        .approve_plan(&session, &plan_id, &plan_hash)
        .await
        .unwrap();
    assert_eq!(
        tokio::fs::read(base.join("core/vectors.json"))
            .await
            .unwrap(),
        registry_after
    );
    assert_eq!(
        tokio::fs::read(base.join("core/feature-services.json"))
            .await
            .unwrap(),
        services_before
    );
    assert_eq!(reopened.list_vectors().await.len(), 1);
    assert!(reopened.list().await.is_empty() && reopened.list_projects().await.is_empty());
    assert_eq!(
        reopened
            .inspect_vector(&vector_id)
            .await
            .unwrap()
            .asset
            .source_sha256,
        asset.source_sha256
    );
    let receipt = json!({"schema":"geod-agent-vector-plan-model-acceptance/v1","status":"passed","modelRoute":model,"upstreamVendorVerified":false,
        "sourceCase":source,"serviceId":service.id,"asset":asset,"planId":plan_id,"planHash":plan_hash,"originalPlanId":old_id,
        "labelPointer":label_pointer,"geometryPointer":geometry_pointer,"firstName":name,"firstPosition":position,
        "harnessConnectedPublicService":true,"harnessAcquiredPublicVector":false,"modelPreparedExtractionPlan":true,"modelPerformedExtraction":false,
        "nativeConfirmationPerformedExtraction":true,"humanRevisionRequiredSeparateConfirmation":true,
        "persistedVectorCount":1,"persistedJobCount":0,"unchangedVectorRegistryAfterRepeatedConfirmation":true,"unchangedServiceRegistry":true,
        "resumedSameConversation":true,"resumedSameModelThread":true,"usedUserDesktop":false,"credentialVaultWritten":false,"published":false,
        "beforeConfirmation":prepared,"afterRevision":edited,"beforeRestart":read,"afterRestart":resumed});
    tokio::fs::write(
        base.join("native-acceptance.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    reopened.shutdown().await.unwrap();
}
