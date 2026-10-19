//! Local verification helper. Request secrets arrive only on stdin; output is
//! the same redacted account status exposed by the trusted desktop command.
//! Never use against an active transfer store or replace a connected account.
use geod_runtime::{AccountProvider, ConnectAccountRequest, JobManager};
use std::io::{self, Read};
#[tokio::main]
async fn main() -> Result<(), String> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.len() != 3 {
        return Err("Expected owned QA store and connect/disconnect".into());
    }
    let store = std::path::PathBuf::from(&args[1])
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if !store.to_string_lossy().contains("\\.verification\\") {
        return Err("An owned QA store is required".into());
    }
    let manager = JobManager::open(store).await?;
    match args[2].as_str() {
        "connect" => {
            let states = manager.provider_accounts().await;
            if states
                .iter()
                .any(|v| v.provider == AccountProvider::Nasa && v.status != "not-connected")
            {
                return Err("Preserve the existing QA authorization".into());
            }
            let mut bytes = Vec::new();
            io::stdin()
                .take(16385)
                .read_to_end(&mut bytes)
                .map_err(|e| e.to_string())?;
            if bytes.len() > 16384 {
                return Err("Request exceeds bound".into());
            }
            let request: ConnectAccountRequest =
                serde_json::from_slice(&bytes).map_err(|_| "Invalid request")?;
            bytes.fill(0);
            if request.provider != AccountProvider::Nasa {
                return Err("Only existing NASA access is scoped for this verification".into());
            }
            let status = manager.connect_provider_account(request).await?;
            println!(
                "{}",
                serde_json::to_string(&status).map_err(|e| e.to_string())?
            );
        }
        "disconnect" => {
            let status = manager
                .disconnect_provider_account(AccountProvider::Nasa)
                .await?;
            println!(
                "{}",
                serde_json::to_string(&status).map_err(|e| e.to_string())?
            );
        }
        _ => return Err("Unknown verification operation".into()),
    }
    Ok(())
}
