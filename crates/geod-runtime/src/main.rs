use geod_runtime::JobManager;
use std::path::PathBuf;

#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("GeoD runtime: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), String> {
    let mut args = std::env::args().skip(1);
    if args.next().as_deref() != Some("serve") {
        return Err("Usage: geod-runtime serve --data-dir PATH [--port 4318]".into());
    }
    let mut data_dir = None;
    let mut port = 4318;
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--data-dir" => {
                data_dir = Some(PathBuf::from(args.next().ok_or("Missing data directory")?))
            }
            "--port" => {
                port = args
                    .next()
                    .ok_or("Missing port")?
                    .parse::<u16>()
                    .map_err(|_| "Invalid port")?
            }
            _ => return Err(format!("Unknown argument: {argument}")),
        }
    }
    if port == 0 {
        return Err("Choose an explicit nonzero port".into());
    }
    let manager = JobManager::open(data_dir.ok_or("--data-dir is required")?).await?;
    geod_runtime::service::serve(manager, port).await
}
