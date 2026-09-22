fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "health",
            "list_jobs",
            "create_job",
            "cancel_job",
            "retry_job",
            "reveal_job",
            "open_source",
        ]),
    ))
    .expect("failed to build GeoD Global desktop resources");
}
