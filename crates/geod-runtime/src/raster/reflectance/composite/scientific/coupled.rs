//! Coherent product-specific scene selection. Completed band/QA mosaics pin the output
//! area and source list; their independently selected pixel values are unused.
use super::*;
use std::io::Write;
use tempfile::NamedTempFile;
use tokio::sync::mpsc::UnboundedSender;

const SELECTION: &str =
    "newest complete qualified RGB scene wins; composite start then item ID break ties";
const LANDSAT_SELECTION: &str =
    "newest complete qualified RGB scene wins; acquisition date then item ID break ties";
fn keys(mask: &QualityMaskSpec) -> [&str; 5] {
    match mask {
        QualityMaskSpec::Modis(_) => ["red", "green", "blue", "modis_qc", "modis_state"],
        QualityMaskSpec::Landsat(_) => ["red", "green", "blue", "qa_pixel", "qa_radsat"],
    }
}
fn selection_rule(mask: &QualityMaskSpec) -> &'static str {
    match mask {
        QualityMaskSpec::Modis(_) => SELECTION,
        QualityMaskSpec::Landsat(_) => LANDSAT_SELECTION,
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModisCoupledScene {
    pub sources: [RgbSource; 5],
    pub grid: Grid,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModisCoupledSpec {
    pub selection: String,
    /// Ordered from oldest to newest, including a deterministic identity tie.
    pub scenes: Vec<ModisCoupledScene>,
    pub geometry: Value,
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModisCoupledResult {
    /// Final retained pixel counts in the same order as the pinned scenes.
    pub scene_valid_pixels: Vec<u64>,
    /// A qualified older scene replaced the newest complete RGB candidate.
    pub fallback_pixels: u64,
}

fn pin(job: &Job) -> Result<RgbSource> {
    if job.kind != "download" || job.status != JobStatus::Succeeded {
        return Err("Coupled RGB selection needs every pinned original download".into());
    }
    Ok(RgbSource {
        pin: BandPin {
            job_id: job.id.clone(),
            band: job.asset_key.clone(),
            sha256: job.sha256.clone().ok_or("Missing original checksum")?,
        },
        kind: job.kind.clone(),
        item_id: job.item_id.clone(),
        href: job.href.clone(),
        bytes: job.bytes_downloaded,
        attribution: job.source.clone(),
        provenance: None,
    })
}
fn inputs(source: &RgbSource) -> Result<&Vec<Value>> {
    if source.kind != "raster_mosaic" {
        return Err(
            "Coupled RGB selection requires matched project bands and quality layers".into(),
        );
    }
    source
        .provenance
        .as_ref()
        .and_then(|p| p["sources"].as_array())
        .ok_or("Coupled RGB original source list is missing".into())
}
fn parents<'a>(sources: &'a [RgbSource; 3], mask: &'a QualityMaskSpec) -> [&'a RgbSource; 5] {
    [
        &sources[0],
        &sources[1],
        &sources[2],
        &mask.sources()[0],
        &mask.sources()[1],
    ]
}
fn order(scene: &ModisCoupledScene, mask: &QualityMaskSpec) -> Result<(String, String)> {
    let id = &scene.sources[0].item_id;
    Ok((
        match mask {
            QualityMaskSpec::Modis(_) => {
                providers::modis::period(id).ok_or("Invalid MODIS composite period")?[0].clone()
            }
            QualityMaskSpec::Landsat(_) => chrono::NaiveDate::parse_from_str(
                id.split('_')
                    .nth(3)
                    .ok_or("Invalid Landsat acquisition date")?,
                "%Y%m%d",
            )
            .map_err(io_error)?
            .to_string(),
        },
        id.clone(),
    ))
}

pub(super) async fn original_jobs(
    manager: &JobManager,
    sources: &[RgbSource; 3],
    mask: &QualityMaskSpec,
) -> Result<Option<Vec<[Job; 5]>>> {
    if sources[0].kind != "raster_mosaic" || inputs(&sources[0])?.len() <= 1 {
        return Ok(None);
    }
    let keys = keys(mask);
    let all = parents(sources, mask);
    let count = inputs(all[0])?.len();
    let identity = pair_key(all[0])?;
    if count > crate::projects::MAX_PROJECT_SCENES {
        return Err(
            "Coupled RGB bands and quality layers have different scene selections or areas".into(),
        );
    }
    for source in all {
        if pair_key(source)? != identity {
            return Err(
                "Coupled RGB bands and quality layers have different scene selections or areas"
                    .into(),
            );
        }
    }
    let store = manager.inner.store.lock().await;
    let mut scenes = Vec::with_capacity(count);
    for index in 0..count {
        let mut jobs = Vec::with_capacity(5);
        for (channel, source) in all.iter().enumerate() {
            let input = inputs(source)?
                .get(index)
                .ok_or("Missing coupled RGB scene")?;
            let id = input["jobId"]
                .as_str()
                .ok_or("Missing coupled original identifier")?;
            let job = store.jobs.get(id).ok_or(
                "A coupled RGB original was removed; restore all five files for every scene",
            )?;
            if job.asset_key != keys[channel]
                || input["sha256"].as_str() != job.sha256.as_deref()
                || input["href"].as_str() != Some(&job.href)
                || input["itemId"].as_str() != Some(&job.item_id)
            {
                return Err("Coupled RGB original differs from project provenance".into());
            }
            pin(job)?;
            jobs.push(job.clone());
        }
        scenes.push(
            jobs.try_into()
                .map_err(|_| "Five same-scene RGB and quality files are required")?,
        );
    }
    Ok(Some(scenes))
}

fn scene_spec(spec: &RgbSpec, scene: &ModisCoupledScene, masked: bool) -> RgbSpec {
    let mut result = spec.clone();
    result.sources = scene.sources[..3].to_vec().try_into().unwrap();
    result.grid = scene.grid.clone();
    result.project_id = None;
    result.quality_mask = if masked {
        let mut mask = spec.quality_mask.as_ref().unwrap().clone();
        match &mut mask {
            QualityMaskSpec::Modis(m) => {
                m.schema_version = "geod-modis-rgb-mask/v1".into();
                m.coupled = None;
            }
            QualityMaskSpec::Landsat(m) => {
                m.schema_version = landsat_mask::SCHEMA.into();
                m.coupled = None;
            }
        }
        *mask.sources_mut() = scene.sources[3..].to_vec().try_into().unwrap();
        Some(mask)
    } else {
        None
    };
    result
}
fn offset(grid: &Grid, source: &Grid) -> Result<[i64; 2]> {
    let values = [
        (source.bounds[0] - grid.bounds[0]) / grid.pixel_size[0],
        (grid.bounds[3] - source.bounds[3]) / grid.pixel_size[1],
    ];
    if source.crs != grid.crs
        || !matches!(
            source.pixel_interpretation.as_str(),
            "PixelIsArea" | "PixelIsPoint"
        )
        || source
            .pixel_size
            .iter()
            .zip(grid.pixel_size)
            .any(|(a, b)| (a - b).abs() > 1e-7)
        || values
            .iter()
            .any(|v| !v.is_finite() || (v - v.round()).abs() > 1e-6 || v.abs() > 100000.0)
    {
        return Err(
            "Coupled RGB original grid is not pixel-aligned; no resampling is performed".into(),
        );
    }
    Ok(values.map(|v| v.round() as i64))
}
pub(super) fn pin_spec(root: &Path, spec: &RgbSpec, jobs: &[[Job; 5]]) -> Result<ModisCoupledSpec> {
    let mut scenes = Vec::with_capacity(jobs.len());
    for originals in jobs {
        let rgb: [Job; 3] = originals[..3].to_vec().try_into().unwrap();
        if validate_jobs(&rgb)?.0 != spec.profile {
            return Err("Coupled RGB original calibration differs".into());
        }
        let grid = inspect_grid(root, &rgb)?;
        let qa: [Job; 2] = originals[3..].to_vec().try_into().unwrap();
        quality_mask::inspect_grids(root, spec.quality_mask.as_ref().unwrap(), &grid, &qa)?;
        let scene = ModisCoupledScene {
            sources: originals
                .iter()
                .map(pin)
                .collect::<Result<Vec<_>>>()?
                .try_into()
                .unwrap(),
            grid,
        };
        offset(&spec.grid, &scene.grid)?;
        scenes.push(scene);
    }
    Ok(ModisCoupledSpec {
        selection: selection_rule(spec.quality_mask.as_ref().unwrap()).into(),
        scenes,
        geometry: spec.sources[0].provenance.as_ref().unwrap()["project"]["geometry"].clone(),
    })
}

pub(super) fn validate_spec(spec: &RgbSpec, selection: &ModisCoupledSpec) -> Result<()> {
    let mask = spec
        .quality_mask
        .as_ref()
        .ok_or("Missing coupled RGB quality rules")?;
    let keys = keys(mask);
    if selection.selection != selection_rule(mask)
        || spec.grid.pixel_interpretation != "PixelIsArea"
        || selection.scenes.len() < 2
        || selection.scenes.len() > crate::projects::MAX_PROJECT_SCENES
        || selection.geometry
            != spec.sources[0]
                .provenance
                .as_ref()
                .ok_or("Missing project provenance")?["project"]["geometry"]
    {
        return Err("Invalid coupled RGB selection rule or area".into());
    }
    if !selection.geometry.is_null() {
        let geometry: crate::crop::PolygonGeometry =
            serde_json::from_value(selection.geometry.clone()).map_err(io_error)?;
        geometry.bounds()?;
    }
    let all = parents(&spec.sources, mask);
    let mut used = std::collections::BTreeSet::new();
    let mut previous = None;
    for (index, scene) in selection.scenes.iter().enumerate() {
        let priority = order(scene, mask)?;
        if previous.as_ref().is_some_and(|p| p >= &priority) {
            return Err("Coupled RGB scenes are not in deterministic acquisition order".into());
        }
        previous = Some(priority);
        let single = scene_spec(spec, scene, true);
        super::validate_spec(&single)?;
        if matches!(mask, QualityMaskSpec::Modis(_)) {
            crate::raster::reflectance::modis::grid(
                &crate::raster::reflectance::Header {
                    width: scene.grid.width,
                    height: scene.grid.height,
                    crs: scene.grid.crs.clone(),
                    bounds: scene.grid.bounds,
                    pixel_size: scene.grid.pixel_size,
                    pixel_is_point: false,
                },
                &scene.sources[0].item_id,
            )?;
        }
        offset(&spec.grid, &scene.grid)?;
        for (channel, source) in scene.sources.iter().enumerate() {
            let url = providers::asset_url(&source.href)?;
            let original = inputs(all[channel])?
                .get(index)
                .ok_or("Missing coupled RGB provenance scene")?;
            if inputs(all[channel])?.len() != selection.scenes.len()
                || source.kind != "download"
                || source.provenance.is_some()
                || source.pin.band != keys[channel]
                || !used.insert(&source.pin.job_id)
                || !match mask {
                    QualityMaskSpec::Modis(_) => {
                        url.host_str() == Some(providers::modis::HOST)
                            && providers::modis::matches(url.path(), &source.item_id, keys[channel])
                    }
                    QualityMaskSpec::Landsat(_) => {
                        url.host_str() == Some(providers::LANDSAT_HOST)
                            && providers::matches_item(&url, &source.item_id, keys[channel])
                    }
                }
                || original["jobId"] != source.pin.job_id
                || original["sha256"] != source.pin.sha256
                || original["href"] != source.href
                || original["itemId"] != source.item_id
            {
                return Err("Coupled RGB pins differ from all five parent source lists".into());
            }
        }
    }
    Ok(())
}
pub(super) fn validate_sources(
    spec: &RgbSpec,
    jobs: &BTreeMap<String, Job>,
) -> Result<Option<Vec<[Job; 5]>>> {
    let Some(selection) = spec.quality_mask.as_ref().and_then(|m| m.coupled()) else {
        return Ok(None);
    };
    let mut result = Vec::with_capacity(selection.scenes.len());
    for scene in &selection.scenes {
        let mut originals = Vec::with_capacity(5);
        for source in &scene.sources {
            let job = jobs
                .get(&source.pin.job_id)
                .ok_or("A coupled RGB original was removed")?;
            if pin(job)? != *source {
                return Err("Coupled RGB original changed after planning".into());
            }
            originals.push(job.clone());
        }
        result.push(originals.try_into().unwrap());
    }
    Ok(Some(result))
}
pub(super) fn validate_result(spec: &RgbSpec, output: &RgbOutput) -> Result<()> {
    match (
        spec.quality_mask.as_ref().and_then(|m| m.coupled()),
        output
            .quality_mask
            .as_ref()
            .and_then(|m| m.coupled.as_ref()),
    ) {
        (None, None) => Ok(()),
        (Some(selection), Some(result)) => {
            let pixels = u64::from(spec.grid.width) * u64::from(spec.grid.height);
            let counts = output.quality_mask.as_ref().unwrap();
            if result.scene_valid_pixels.len() != selection.scenes.len()
                || result.scene_valid_pixels.iter().any(|n| *n > pixels)
                || result.scene_valid_pixels.iter().sum::<u64>() != output.common_valid_pixels
                || output.channel_valid_pixels != [output.common_valid_pixels; 3]
                || result.fallback_pixels > output.common_valid_pixels
                || counts.rejected_pixels != pixels - output.common_valid_pixels
            {
                Err("Coupled RGB result counts differ from coherent RGB selection".into())
            } else {
                Ok(())
            }
        }
        _ => Err("Coupled RGB validation result differs from its specification".into()),
    }
}
pub(super) fn disk_bytes(spec: &RgbSpec) -> Result<u64> {
    if spec
        .quality_mask
        .as_ref()
        .and_then(|m| m.coupled())
        .is_some()
    {
        // Output planes + encoded TIFF, four-byte baseline/winner markers,
        // and one full scene's three DN and two UInt32 QA scratch planes.
        let raw = raw_bytes(&spec.grid)?;
        let scratch = spec
            .quality_mask
            .as_ref()
            .and_then(|m| m.coupled())
            .unwrap()
            .scenes
            .iter()
            .map(|s| u64::from(s.grid.width) * u64::from(s.grid.height) * 14)
            .max()
            .ok_or("Missing coherent original scene")?;
        Ok(raw * 2 + raw / 100 + raw / 6 * 4 + scratch + 8 * 1024 * 1024)
    } else {
        super::disk_bytes(&spec.grid, spec.quality_mask.is_some())
    }
}

fn at(file: &mut NamedTempFile, offset: u64) -> Result<&mut File> {
    file.as_file_mut()
        .seek(SeekFrom::Start(offset))
        .map_err(io_error)?;
    Ok(file.as_file_mut())
}
fn complete(bands: &[Vec<u8>; 3], index: usize, nodata: i32) -> bool {
    bands
        .iter()
        .all(|band| u16::from_le_bytes([band[index * 2], band[index * 2 + 1]]) != nodata as u16)
}

pub(super) fn write(
    root: &Path,
    id: &str,
    spec: &RgbSpec,
    jobs: &[[Job; 5]],
    token: &CancellationToken,
    progress: &UnboundedSender<(u64, &'static str)>,
) -> Result<encode::Output> {
    super::validate_spec(spec)?;
    if !canonical_id(id)
        || fs2::available_space(root.join("assets")).map_err(io_error)? < disk_bytes(spec)?
    {
        return Err("Insufficient workspace space or invalid coupled RGB output identifier".into());
    }
    let selection = spec
        .quality_mask
        .as_ref()
        .and_then(|m| m.coupled())
        .ok_or("Missing coupled RGB selection")?;
    if jobs.len() != selection.scenes.len() {
        return Err("Coupled RGB source count changed".into());
    }
    let width = spec.grid.width as usize;
    let mut files = (0..3)
        .map(|_| encode::staged(root, id))
        .collect::<Result<Vec<_>>>()?;
    let fill: Vec<u8> = std::iter::repeat_n(spec.profile.nodata as i16, width)
        .flat_map(i16::to_le_bytes)
        .collect();
    for row in 0..spec.grid.height {
        encode::cancelled(token)?;
        for file in &mut files {
            file.as_file_mut().write_all(&fill).map_err(io_error)?;
        }
        if row % 64 == 0 {
            encode::cancelled(token)?;
        }
    }
    let mut markers = encode::staged(root, id)?;
    markers
        .as_file()
        .set_len(u64::from(spec.grid.width) * u64::from(spec.grid.height) * 4)
        .map_err(io_error)?;
    let geometry: Option<crate::crop::PolygonGeometry> = if selection.geometry.is_null() {
        None
    } else {
        Some(serde_json::from_value(selection.geometry.clone()).map_err(io_error)?)
    };
    let (quiet, receiver) = tokio::sync::mpsc::unbounded_channel();
    drop(receiver);
    let mut reference = None;
    for (scene_index, (scene, originals)) in selection.scenes.iter().zip(jobs).enumerate() {
        let _ = progress.send((
            600 * scene_index as u64 / jobs.len() as u64,
            "Selecting qualified same-scene RGB",
        ));
        for (source, job) in scene.sources.iter().zip(originals) {
            if pin(job)? != *source {
                return Err("Coupled RGB original pin changed".into());
            }
        }
        let context = scene_spec(spec, scene, true);
        let mut planes = Vec::with_capacity(3);
        for (channel, job) in originals[..3].iter().enumerate() {
            planes.push(encode::stage(
                root,
                id,
                &context,
                job,
                token,
                &quiet,
                channel as u64,
            )?);
        }
        let mut planes: [encode::Plane; 3] = planes
            .try_into()
            .map_err(|_| "Missing coupled RGB planes")?;
        reference.get_or_insert_with(|| planes[0].reference.clone());
        let qa: [Job; 2] = originals[3..].to_vec().try_into().unwrap();
        let mut quality = quality_mask::Planes::stage(root, id, &context, &qa, token)?;
        let [dx, dy] = offset(&spec.grid, &scene.grid)?;
        let start_x = 0i64.max(-dx) as usize;
        let end_x = i64::from(scene.grid.width)
            .min(i64::from(spec.grid.width) - dx)
            .max(0) as usize;
        for y in (0..scene.grid.height).step_by(64) {
            encode::cancelled(token)?;
            let height = 64.min(scene.grid.height - y);
            let mut bands = encode::read_bands(&mut planes, y, height, scene.grid.width as usize)?;
            let baseline: Vec<bool> = (0..scene.grid.width as usize * height as usize)
                .map(|i| complete(&bands, i, spec.profile.nodata))
                .collect();
            quality.apply(y, height, &mut bands, spec.profile.nodata)?;
            let first_y = (i64::from(y) + dy).max(0);
            let last_y = (i64::from(y + height) + dy).min(i64::from(spec.grid.height));
            if first_y >= last_y || start_x >= end_x {
                continue;
            }
            let coverage = geometry
                .as_ref()
                .map(|g| {
                    crate::crop::polygon_coverage(
                        g,
                        &spec.grid.crs,
                        [
                            spec.grid.bounds[0],
                            spec.grid.bounds[3] - last_y as f64 * spec.grid.pixel_size[1],
                            spec.grid.bounds[2],
                            spec.grid.bounds[3] - first_y as f64 * spec.grid.pixel_size[1],
                        ],
                        spec.grid.pixel_size,
                        spec.grid.width,
                        (last_y - first_y) as u32,
                        token,
                    )
                })
                .transpose()?;
            for target_y in first_y..last_y {
                encode::cancelled(token)?;
                let source_y = (target_y - dy - i64::from(y)) as usize;
                let mut outputs: [Vec<u8>; 3] = std::array::from_fn(|_| vec![0; width * 2]);
                for (file, output) in files.iter_mut().zip(&mut outputs) {
                    at(file, target_y as u64 * width as u64 * 2)?
                        .read_exact(output)
                        .map_err(io_error)?;
                }
                let mut row_markers = vec![0; width * 4];
                at(&mut markers, target_y as u64 * width as u64 * 4)?
                    .read_exact(&mut row_markers)
                    .map_err(io_error)?;
                for source_x in start_x..end_x {
                    let target_x = (source_x as i64 + dx) as usize;
                    if coverage
                        .as_ref()
                        .is_some_and(|c| !c[(target_y - first_y) as usize * width + target_x])
                    {
                        continue;
                    }
                    let index = source_y * scene.grid.width as usize + source_x;
                    let chosen = (scene_index as u16 + 1).to_le_bytes();
                    if baseline[index] {
                        row_markers[target_x * 4..target_x * 4 + 2].copy_from_slice(&chosen);
                    }
                    if complete(&bands, index, spec.profile.nodata) {
                        row_markers[target_x * 4 + 2..target_x * 4 + 4].copy_from_slice(&chosen);
                        for (output, band) in outputs.iter_mut().zip(&bands) {
                            output[target_x * 2..target_x * 2 + 2]
                                .copy_from_slice(&band[index * 2..index * 2 + 2]);
                        }
                    }
                }
                for (file, output) in files.iter_mut().zip(&outputs) {
                    at(file, target_y as u64 * width as u64 * 2)?
                        .write_all(output)
                        .map_err(io_error)?;
                }
                at(&mut markers, target_y as u64 * width as u64 * 4)?
                    .write_all(&row_markers)
                    .map_err(io_error)?;
            }
        }
        encode::verify_planes(&planes)?;
        quality.verify()?;
    }
    let mut counts = ModisMaskResult {
        examined_pixels: u64::from(spec.grid.width) * u64::from(spec.grid.height),
        coupled: Some(ModisCoupledResult {
            scene_valid_pixels: vec![0; jobs.len()],
            fallback_pixels: 0,
        }),
        ..Default::default()
    };
    markers
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(io_error)?;
    for _ in 0..spec.grid.height {
        encode::cancelled(token)?;
        let mut row = vec![0; width * 4];
        markers
            .as_file_mut()
            .read_exact(&mut row)
            .map_err(io_error)?;
        for pixel in row.chunks_exact(4) {
            let baseline = u16::from_le_bytes(pixel[..2].try_into().unwrap());
            let winner = u16::from_le_bytes(pixel[2..].try_into().unwrap());
            if baseline as usize > jobs.len() || winner > baseline {
                return Err("Invalid coupled RGB staging selection".into());
            }
            counts.input_common_valid_pixels += u64::from(baseline != 0);
            counts.rejected_pixels += u64::from(winner == 0);
            counts.removed_valid_pixels += u64::from(baseline != 0 && winner == 0);
            if winner != 0 {
                let result = counts.coupled.as_mut().unwrap();
                result.scene_valid_pixels[winner as usize - 1] += 1;
                result.fallback_pixels += u64::from(winner != baseline);
            }
        }
    }
    let mut planes = Vec::with_capacity(3);
    let reference = reference.ok_or("No coupled RGB CRS reference")?;
    for mut file in files {
        file.as_file().sync_all().map_err(io_error)?;
        file.as_file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(io_error)?;
        let mut windows = Vec::new();
        for y in (0..spec.grid.height).step_by(64) {
            encode::cancelled(token)?;
            let height = 64.min(spec.grid.height - y);
            let mut bytes = vec![0; width * height as usize * 2];
            file.as_file_mut()
                .read_exact(&mut bytes)
                .map_err(io_error)?;
            windows.push(encode::Window {
                x: 0,
                y,
                width: spec.grid.width,
                height,
                expected: format!("{:x}", Sha256::digest(&bytes)),
                readback: Sha256::new(),
            });
        }
        file.as_file_mut()
            .seek(SeekFrom::Start(0))
            .map_err(io_error)?;
        planes.push(encode::Plane {
            file,
            windows,
            reference: reference.clone(),
        });
    }
    encode::finish(
        root,
        id,
        spec,
        encode::Staged {
            planes: planes
                .try_into()
                .map_err(|_| "Missing coupled RGB output planes")?,
            quality: None,
            coupled_result: Some(counts),
        },
        token,
        progress,
    )
}
