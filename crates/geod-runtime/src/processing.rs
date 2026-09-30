//! Versioned, data-only recipes. These never accept paths, commands, or network sources.
use crate::{active, crop, io_error, now, Job, JobManager, JobStatus, Result};
use serde::{Deserialize, Serialize};
use std::{collections::BTreeMap, path::Path};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

pub const RECIPE_SCHEMA_VERSION: &str = "geod-raster-recipe/v1";
pub const POLYGON_RECIPE_SCHEMA_VERSION: &str = "geod-raster-recipe/v2";

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RasterRecipe {
    pub schema_version: String,
    pub name: String,
    pub source: RecipeSource,
    pub operation: ClipOperation,
    pub output: RecipeOutput,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecipeSource {
    pub job_id: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ClipOperation {
    #[serde(rename = "type")]
    pub operation_type: String,
    pub crs: String,
    pub bounds: [f64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<crop::PolygonGeometry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RecipeOutput {
    pub format: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SavedRecipe {
    pub id: String,
    pub recipe: RasterRecipe,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecipePlan {
    pub recipe: RasterRecipe,
    pub plan: crop::CropPlan,
}

impl RasterRecipe {
    pub fn validate(&self) -> Result<()> {
        if self.schema_version != RECIPE_SCHEMA_VERSION
            && self.schema_version != POLYGON_RECIPE_SCHEMA_VERSION
        {
            return Err(format!(
                "Unsupported recipe schemaVersion; expected {RECIPE_SCHEMA_VERSION} or {POLYGON_RECIPE_SCHEMA_VERSION}"
            ));
        }
        if (self.schema_version == RECIPE_SCHEMA_VERSION) != self.operation.geometry.is_none() {
            return Err(
                "Recipe v1 requires rectangular bounds; v2 requires a WGS84 polygon geometry"
                    .into(),
            );
        }
        if self.name.trim().is_empty()
            || self.name.chars().count() > 120
            || self.name.chars().any(char::is_control)
        {
            return Err(
                "Recipe name must contain 1 to 120 characters and no control characters".into(),
            );
        }
        if Uuid::parse_str(&self.source.job_id)
            .ok()
            .map(|id| id.to_string())
            .as_deref()
            != Some(self.source.job_id.as_str())
        {
            return Err("Recipe source.jobId must be a lowercase hyphenated UUID".into());
        }
        if self.source.sha256.len() != 64
            || !self
                .source
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
        {
            return Err("Recipe source.sha256 must be 64 lowercase hexadecimal characters".into());
        }
        if self.operation.operation_type != "clip" {
            return Err("Unsupported recipe operation; only clip is executable".into());
        }
        if self.output.format != "GeoTIFF" {
            return Err("Unsupported recipe output; only GeoTIFF is executable".into());
        }
        let [west, south, east, north] = self.operation.bounds;
        if !self.operation.bounds.iter().all(|value| value.is_finite())
            || west >= east
            || south >= north
        {
            return Err("Clip bounds must be finite and ordered [west, south, east, north]".into());
        }
        match self.operation.crs.as_str() {
            "source" => {},
            "EPSG:4326" if west >= -180.0 && east <= 180.0 && south >= -80.0 && north <= 84.0 && east - west <= 180.0 => {},
            "EPSG:4326" => return Err("WGS84 clip bounds must use longitude -180..180 and UTM latitude -80..84, without crossing the antimeridian".into()),
            _ => return Err("Clip CRS must be source or EPSG:4326".into()),
        }
        if let Some(geometry) = &self.operation.geometry {
            if self.operation.crs != "EPSG:4326" {
                return Err("Polygon clips require EPSG:4326 coordinates".into());
            }
            let extent = geometry.bounds()?;
            if extent[0] >= east || extent[2] <= west || extent[1] >= north || extent[3] <= south {
                return Err("Polygon mask and output window must overlap".into());
            }
        }
        Ok(())
    }

    fn parameters(&self) -> crop::ClipParameters {
        crop::ClipParameters {
            crs: self.operation.crs.clone(),
            bounds: self.operation.bounds,
            geometry: self.operation.geometry.clone(),
        }
    }
}

pub(crate) fn validate_source(recipe: &RasterRecipe, source: Option<&Job>) -> Result<Job> {
    let source = source.ok_or("Recipe source job is not available in this local storage")?;
    if source.status != JobStatus::Succeeded
        || source.asset_key != "scl"
        || crate::extension(&source.media_type)? != "tif"
    {
        return Err("Recipe source must be a completed local SCL GeoTIFF job".into());
    }
    if source.sha256.as_deref() != Some(recipe.source.sha256.as_str()) {
        return Err("Recipe source SHA-256 does not match the completed local job".into());
    }
    Ok(source.clone())
}

pub(crate) async fn load_recipes(root: &Path) -> Result<BTreeMap<String, SavedRecipe>> {
    let recipes: BTreeMap<String, SavedRecipe> =
        match tokio::fs::read(root.join("recipes.json")).await {
            Ok(bytes) => serde_json::from_slice(&bytes)
                .map_err(|error| format!("Cannot read stored recipes: {error}"))?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => BTreeMap::new(),
            Err(error) => return Err(io_error(error)),
        };
    for (id, saved) in &recipes {
        if id != &saved.id || Uuid::parse_str(id).is_err() {
            return Err("Stored recipe has an invalid identifier".into());
        }
        saved.recipe.validate()?;
    }
    Ok(recipes)
}

impl JobManager {
    pub async fn list_recipes(&self) -> Vec<SavedRecipe> {
        let mut recipes: Vec<_> = self.inner.recipes.lock().await.values().cloned().collect();
        recipes.sort_by(|a, b| b.created_at.cmp(&a.created_at));
        recipes
    }

    pub async fn plan_recipe(&self, recipe: RasterRecipe) -> Result<RecipePlan> {
        recipe.validate()?;
        let source = validate_source(&recipe, self.get(&recipe.source.job_id).await.as_ref())?;
        let permit = self
            .inner
            .raster_permits
            .clone()
            .try_acquire_owned()
            .map_err(|_| {
                "The raster worker is busy; try planning again after the current operation finishes"
            })?;
        let root = self.inner.root.clone();
        let parameters = recipe.parameters();
        let plan = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            crop::plan_crop(&root, &source, &parameters)
        })
        .await
        .map_err(io_error)??;
        Ok(RecipePlan { recipe, plan })
    }

    pub async fn save_recipe(&self, recipe: RasterRecipe) -> Result<SavedRecipe> {
        // Saving proves the pinned local source and requested crop are currently usable.
        self.plan_recipe(recipe.clone()).await?;
        let timestamp = now();
        let saved = SavedRecipe {
            id: Uuid::new_v4().to_string(),
            recipe,
            created_at: timestamp.clone(),
            updated_at: timestamp,
        };
        let mut recipes = self.inner.recipes.lock().await;
        recipes.insert(saved.id.clone(), saved.clone());
        let result = async {
            let bytes = serde_json::to_vec_pretty(&*recipes).map_err(io_error)?;
            let temporary = self.inner.root.join("recipes.json.tmp");
            let mut file = tokio::fs::File::create(&temporary)
                .await
                .map_err(io_error)?;
            file.write_all(&bytes).await.map_err(io_error)?;
            file.sync_all().await.map_err(io_error)?;
            drop(file);
            tokio::fs::rename(temporary, self.inner.root.join("recipes.json"))
                .await
                .map_err(io_error)
        }
        .await;
        if let Err(error) = result {
            recipes.remove(&saved.id);
            return Err(error);
        }
        Ok(saved)
    }

    pub async fn run_recipe(&self, recipe: RasterRecipe) -> Result<Job> {
        recipe.validate()?;
        let mut store = self.inner.store.lock().await;
        store.accepting_jobs()?;
        let source = validate_source(&recipe, store.jobs.get(&recipe.source.job_id))?;
        if store.active.len() >= 64 {
            return Err("The local queue is full (64 jobs)".into());
        }
        let timestamp = now();
        let job = Job {
            id: Uuid::new_v4().to_string(),
            kind: "raster_clip".into(),
            parent_id: Some(source.id),
            title: recipe.name.clone(),
            recipe: Some(recipe),
            crop: None,
            mosaic: None,
            mosaic_output: None,
            manifest_path: None,
            item_id: source.item_id,
            asset_key: "scl".into(),
            href: source.href,
            media_type: "image/tiff".into(),
            status: JobStatus::Queued,
            bytes_downloaded: 0,
            total_bytes: None,
            sha256: None,
            output_path: None,
            error: None,
            created_at: timestamp.clone(),
            updated_at: timestamp,
            source: source.source,
            validation: "Pending pinned source validation and exact pixel-window clip".into(),
            attempts: 1,
        };
        store.jobs.insert(job.id.clone(), job.clone());
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.remove(&job.id);
            return Err(error);
        }
        let token = CancellationToken::new();
        store.active.insert(job.id.clone(), token.clone());
        self.spawn(job.id.clone(), token);
        Ok(job)
    }

    pub(crate) async fn process_clip(&self, id: &str, token: &CancellationToken) -> Result<()> {
        let permit = tokio::select! {
            _ = token.cancelled() => return Err("Raster clip cancelled".into()),
            permit = self.inner.raster_permits.clone().acquire_owned() => permit.map_err(io_error)?,
        };
        self.progress(id, 0, None).await?;
        let job = self.get(id).await.ok_or("Unknown job")?;
        let recipe = job.recipe.ok_or("The clip job has no recipe")?;
        recipe.validate()?;
        let source = validate_source(&recipe, self.get(&recipe.source.job_id).await.as_ref())?;
        let source_provenance = serde_json::json!({"jobId":source.id,"itemId":source.item_id,"href":source.href,"attribution":source.source,"sha256":recipe.source.sha256});
        let root = self.inner.root.clone();
        let parameters = recipe.parameters();
        let output_id = id.to_owned();
        let cancellation = token.clone();
        let output = tokio::task::spawn_blocking(move || {
            // Keep the permit inside the blocking worker even if its async caller is dropped.
            let _permit = permit;
            crop::write_crop(&root, &source, &parameters, &output_id, &cancellation)
        })
        .await
        .map_err(io_error)??;
        let manifest_path = self
            .inner
            .root
            .join("assets")
            .join(format!("{id}.metadata.json"));
        let temporary = self
            .inner
            .root
            .join("assets")
            .join(format!("{id}.metadata.json.tmp"));
        let manifest = serde_json::json!({
            "schemaVersion":"geod-raster-artifact/v1", "createdAt":now(),
            "output":{"file":format!("{id}.tif"),"format":"GeoTIFF","bytes":output.bytes,"sha256":output.sha256},
            "source":source_provenance,"recipe":recipe,"crop":output.plan,
        });
        let mut file = tokio::fs::File::create(&temporary)
            .await
            .map_err(io_error)?;
        file.write_all(&serde_json::to_vec_pretty(&manifest).map_err(io_error)?)
            .await
            .map_err(io_error)?;
        file.sync_all().await.map_err(io_error)?;
        drop(file);
        tokio::fs::rename(&temporary, &manifest_path)
            .await
            .map_err(io_error)?;
        let mut store = self.inner.store.lock().await;
        let record = store.jobs.get_mut(id).ok_or("Unknown job")?;
        if token.is_cancelled() || !active(&record.status) {
            let _ = tokio::fs::remove_file(&output.output_path).await;
            return Err("Raster clip cancelled".into());
        }
        let before_commit = record.clone();
        record.status = JobStatus::Succeeded;
        record.bytes_downloaded = output.bytes;
        record.total_bytes = Some(output.bytes);
        record.sha256 = Some(output.sha256);
        record.output_path = Some(output.output_path);
        let polygon_clip = output.plan.masked_pixels.is_some();
        record.crop = Some(output.plan);
        record.manifest_path = Some(manifest_path.to_string_lossy().into_owned());
        record.updated_at = now();
        record.error = None;
        record.validation = if polygon_clip { "Pinned source SHA-256 verified; WGS84 polygon pixel-centre mask, UInt8 SCL output, and GeoTIFF georeferencing validated; no resampling" } else { "Pinned source SHA-256 verified; exact UInt8 SCL pixel window and GeoTIFF georeferencing validated; no resampling" }.into();
        if let Err(error) = self.persist(&store.jobs).await {
            store.jobs.insert(id.to_owned(), before_commit);
            return Err(error);
        }
        Ok(())
    }
}

/// Only generated files for this validated job UUID are eligible for recovery cleanup.
pub(crate) async fn cleanup_clip(root: &Path, id: &str) {
    if Uuid::parse_str(id).ok().map(|id| id.to_string()).as_deref() != Some(id) {
        return;
    }
    let assets = root.join("assets");
    for suffix in ["tif", "metadata.json", "metadata.json.tmp"] {
        let _ = tokio::fs::remove_file(assets.join(format!("{id}.{suffix}"))).await;
    }
    if let Ok(mut entries) = tokio::fs::read_dir(&assets).await {
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with(&format!("{id}.crop-")) && name.ends_with(".part") {
                let _ = tokio::fs::remove_file(entry.path()).await;
            }
        }
    }
}
