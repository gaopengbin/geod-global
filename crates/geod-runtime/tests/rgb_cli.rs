//! The Windows debug CLI must reach a structured error through real child
//! processes. Future-size assertions alone missed poll-frame stack overflow.
use std::{net::TcpListener, process::Command};

#[test]
fn scientific_rgb_cli_handles_direct_and_http_failures_without_stack_overflow() {
    let workspace = tempfile::tempdir().unwrap();
    let request = workspace.path().join("rgb.json");
    std::fs::write(&request, r#"{"jobIds":["00000000-0000-4000-8000-000000000001","00000000-0000-4000-8000-000000000002","00000000-0000-4000-8000-000000000003"],"qualityMask":{"qcJobId":"00000000-0000-4000-8000-000000000004","stateJobId":"00000000-0000-4000-8000-000000000005","policy":"clear_best","excludeSnow":true}}"#).unwrap();
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    let server = format!("http://{}", listener.local_addr().unwrap());
    drop(listener);
    for (mode, target) in [
        ("--server", server),
        (
            "--data-dir",
            workspace
                .path()
                .join("empty")
                .to_string_lossy()
                .into_owned(),
        ),
    ] {
        let mut command = Command::new(env!("CARGO_BIN_EXE_geod-runtime"));
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            command.creation_flags(0x08000000);
        }
        let result = command
            .args(["scientific-rgb", "plan", "--request"])
            .arg(&request)
            .args([mode, &target])
            .output()
            .unwrap();
        assert_eq!(
            result.status.code(),
            Some(1),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert!(result.stdout.is_empty());
        let error: serde_json::Value = serde_json::from_slice(&result.stderr).unwrap();
        assert!(error["error"].as_str().is_some_and(|s| !s.is_empty()));
        assert!(!String::from_utf8_lossy(&result.stderr).contains("overflowed"));
    }
}
