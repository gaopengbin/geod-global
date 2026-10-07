use super::*;

#[tokio::test]
#[ignore = "explicit isolated startup timing with owned Node; no model request or user credentials"]
async fn owned_runtime_history_cold_and_warm_load() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .canonicalize()
        .unwrap();
    let base =
        PathBuf::from(std::env::var_os("GEOD_AGENT_RESTORE_QA").expect("isolated QA directory"));
    let home = base.join("sessions");
    tokio::fs::create_dir_all(&home).await.unwrap();
    let id = registry::new_id();
    let text = "Owned conversation restoration timing fixture. ".repeat(55);
    tokio::fs::write(home.join("sessions.json"),serde_json::to_vec(&json!({"version":1,"selectedId":id,"sessions":[{
        "id":id,"threadId":null,"connectionId":"a".repeat(64),"toolSetId":"b".repeat(64),"title":"Owned restoration fixture",
        "createdAt":"2026-10-06T00:00:00Z","updatedAt":"2026-10-06T00:00:00Z","status":"completed","entries":[
        {"id":registry::new_id(),"type":"user","text":text,"status":"completed"},
        {"id":registry::new_id(),"type":"assistant","text":"Retained previous answer.","status":"completed"}]}]})).unwrap()).await.unwrap();
    let manager = JobManager::open(base.join("core")).await.unwrap();
    let runtime = root.join(".agent-runtime/win32-x64");
    let start = tokio::time::Instant::now();
    verify_runtime(&runtime).await.unwrap();
    let verify_ms = start.elapsed().as_millis();
    let agent = DesktopAgent::open(home, runtime, manager.clone())
        .await
        .unwrap();
    let start = tokio::time::Instant::now();
    let cold = agent.snapshot().await.unwrap();
    let cold_ms = start.elapsed().as_millis();
    assert_eq!(cold["selected"]["id"], id);
    assert_eq!(cold["selected"]["entries"].as_array().unwrap().len(), 2);
    let start = tokio::time::Instant::now();
    let warm = agent.snapshot().await.unwrap();
    let warm_ms = start.elapsed().as_millis();
    assert_eq!(warm["selected"]["id"], id);
    assert_eq!(warm["configured"], false);
    let receipt = json!({"schema":"geod-agent-startup-timing/v1","status":"passed","runtimeVerificationMs":verify_ms,
        "coldHistorySnapshotMs":cold_ms,"warmHistorySnapshotMs":warm_ms,"entriesPreserved":true,
        "paidModelCalls":0,"userCredentialsRead":false,"usedUserDesktop":false});
    tokio::fs::write(
        base.join("timing.json"),
        serde_json::to_vec_pretty(&receipt).unwrap(),
    )
    .await
    .unwrap();
    agent.shutdown().await;
    manager.shutdown().await.unwrap();
    println!("{receipt}");
}
