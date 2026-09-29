fn main() {
    tauri_build::try_build(tauri_build::Attributes::new().app_manifest(
        tauri_build::AppManifest::new().commands(&[
            "health",
            "diagnostics",
            "list_jobs",
            "list_projects",
            "create_project",
            "download_project",
            "mosaic_project",
            "create_job",
            "cancel_job",
            "retry_job",
            "inspect_raster",
            "sample_raster",
            "prepare_artifact",
            "reveal_artifact",
            "list_recipes",
            "plan_recipe",
            "save_recipe",
            "run_recipe",
            "reveal_job",
            "open_source",
        ]),
    ))
    .expect("failed to build GeoD Global desktop resources");
}
