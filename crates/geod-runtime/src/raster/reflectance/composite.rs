//! Local, read-only RGB from three verified original reflectance bands.
//! No composite file or scientific DN is manufactured from the display PNG.
use super::*;

pub mod scientific;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompositeRequest {
    /// Exactly red, green, blue, in that order. Paths and URLs are not accepted.
    pub job_ids: [String; 3],
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompositePixelRequest {
    pub job_ids: [String; 3],
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct BandPin {
    pub job_id: String,
    pub sha256: String,
    pub band: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Grid {
    pub width: u32,
    pub height: u32,
    pub crs: String,
    pub bounds: [f64; 4],
    pub pixel_size: [f64; 2],
    pub pixel_interpretation: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeDisplay {
    pub product: String,
    pub sources: [BandPin; 3],
    pub scale: f64,
    pub offset: f64,
    pub display_ranges: [[i32; 2]; 3],
    pub sample_count: u32,
    pub valid_sample_count: u32,
    pub derived: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeArtifact {
    pub job_id: String,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositeInspection {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<CompositeArtifact>,
    #[serde(flatten)]
    pub grid: Grid,
    pub band_count: u8,
    pub data_type: String,
    pub nodata: i32,
    pub preview_width: u32,
    pub preview_height: u32,
    pub preview_data_url: String,
    pub composite: CompositeDisplay,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CompositePixel {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub artifact: Option<CompositeArtifact>,
    pub sources: [BandPin; 3],
    pub crs: String,
    pub coordinate: [f64; 2],
    pub pixel: [u32; 2],
    pub center: [f64; 2],
    pub values: [i32; 3],
    pub reflectances: [Option<f64>; 3],
    pub channel_no_data: [bool; 3],
    pub is_no_data: bool,
}

fn grid(source: &Source) -> Grid {
    Grid {
        width: source.width,
        height: source.height,
        crs: source.crs.clone(),
        bounds: source.bounds,
        pixel_size: source.pixel_size,
        pixel_interpretation: if source.pixel_is_point {
            "PixelIsPoint"
        } else {
            "PixelIsArea"
        }
        .into(),
    }
}

fn validate_jobs(jobs: &[Job; 3]) -> Result<(Profile, [BandPin; 3])> {
    let first = &jobs[0];
    let expected = profile(first)?;
    let directory = providers::asset_url(&first.href)?
        .join(".")
        .map_err(io_error)?;
    for (index, job) in jobs.iter().enumerate() {
        if !matches!(
            job.kind.as_str(),
            "download" | "raster_prepare" | "raster_mosaic"
        ) || job.kind != first.kind
            || job.asset_key != ["red", "green", "blue"][index]
            || job.item_id != first.item_id
            || profile(job)? != expected
            || providers::asset_url(&job.href)?
                .join(".")
                .map_err(io_error)?
                != directory
            || jobs[..index].iter().any(|other| other.id == job.id)
            || job.viirs_prepare != first.viirs_prepare
        {
            return Err(
                "Local RGB requires red, green and blue originals from the same product and scene"
                    .into(),
            );
        }
    }
    let pins = jobs.clone().map(|job| BandPin {
        job_id: job.id,
        sha256: job.sha256.unwrap_or_default(),
        band: job.asset_key,
    });
    Ok((expected, pins))
}

fn preview_samples(source: &mut Source, edge: u32) -> Result<(u32, u32, Vec<i32>)> {
    let longest = source.width.max(source.height);
    let pw = (source.width as u64 * edge.min(longest) as u64 / longest as u64).max(1) as u32;
    let ph = (source.height as u64 * edge.min(longest) as u64 / longest as u64).max(1) as u32;
    let [cw, ch, columns] = chunk_grid(source)?;
    let mut targets: BTreeMap<u32, Vec<(usize, u32, u32)>> = BTreeMap::new();
    for y in 0..ph {
        check_time(source.deadline)?;
        for x in 0..pw {
            let sx = (x as u64 * source.width as u64 / pw as u64) as u32;
            let sy = (y as u64 * source.height as u64 / ph as u64) as u32;
            targets
                .entry(sy / ch * columns + sx / cw)
                .or_default()
                .push(((y * pw + x) as usize, sx % cw, sy % ch));
        }
    }
    let mut preview = vec![source.profile.nodata; (pw * ph) as usize];
    for (chunk, pixels) in targets {
        let values = read_chunk(source, chunk)?;
        let (width, height) = source.decoder.chunk_data_dimensions(chunk);
        for (index, x, y) in pixels {
            if x >= width || y >= height {
                return Err("RGB preview is outside its source chunk".into());
            }
            preview[index] = values
                .get((y * width + x) as usize)
                .ok_or("Missing RGB source sample")?;
        }
    }
    Ok((pw, ph, preview))
}

pub(crate) fn inspect(root: &Path, jobs: &[Job; 3], edge: u32) -> Result<CompositeInspection> {
    if edge == 0 || edge > PREVIEW_EDGE {
        return Err("Invalid RGB preview size".into());
    }
    let (profile, pins) = validate_jobs(jobs)?;
    let deadline = Instant::now() + Duration::from_secs(55);
    let mut geometry = None;
    let mut bands = Vec::with_capacity(3);
    let (mut pw, mut ph) = (0, 0);
    // Verify/decode sequentially: at most one compressed original snapshot and
    // three bounded 768px grids, rather than three complete rasters in memory.
    for job in jobs {
        let mut source = source_with_deadline(root, job, deadline)?;
        let current = grid(&source);
        if geometry
            .as_ref()
            .is_some_and(|previous| previous != &current)
        {
            return Err(
                "Local RGB bands have different grids; no implicit resampling is performed".into(),
            );
        }
        geometry = Some(current);
        let result = preview_samples(&mut source, edge)?;
        (pw, ph) = (result.0, result.1);
        bands.push(result.2);
    }
    display(
        geometry.ok_or("Missing RGB grid")?,
        profile,
        pins,
        bands,
        [pw, ph],
        deadline,
        jobs[0].kind == "raster_mosaic",
    )
}

fn display(
    geometry: Grid,
    profile: Profile,
    pins: [BandPin; 3],
    bands: Vec<Vec<i32>>,
    preview_size: [u32; 2],
    deadline: Instant,
    derived: bool,
) -> Result<CompositeInspection> {
    let [pw, ph] = preview_size;
    let common: Vec<_> = (0..(pw * ph) as usize)
        .filter(|index| bands.iter().all(|band| band[*index] != profile.nodata))
        .collect();
    let mut ranges = [[0; 2]; 3];
    for (channel, band) in bands.iter().enumerate() {
        let mut valid: Vec<_> = common.iter().map(|index| band[*index]).collect();
        valid.sort_unstable();
        if !valid.is_empty() {
            ranges[channel] = [
                valid[(valid.len() - 1) * 2 / 100],
                valid[(valid.len() - 1) * 98 / 100],
            ];
        }
    }
    let rgba: Vec<_> = (0..(pw * ph) as usize)
        .flat_map(|index| {
            let valid = bands.iter().all(|band| band[index] != profile.nodata);
            let colors: [u8; 3] = std::array::from_fn(|channel| {
                let [low, high] = ranges[channel];
                if !valid {
                    0
                } else if low == high {
                    128
                } else {
                    (((bands[channel][index] - low) as f64 / (high - low) as f64).clamp(0.0, 1.0)
                        * 255.0)
                        .round() as u8
                }
            });
            [colors[0], colors[1], colors[2], if valid { 255 } else { 0 }]
        })
        .collect();
    let mut png = Vec::new();
    let mut encoder = png::Encoder::new(&mut png, pw, ph);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    encoder
        .write_header()
        .map_err(io_error)?
        .write_image_data(&rgba)
        .map_err(io_error)?;
    check_time(deadline)?;
    Ok(CompositeInspection {
        artifact: None,
        grid: geometry,
        band_count: 3,
        data_type: if profile.signed { "Int16" } else { "UInt16" }.into(),
        nodata: profile.nodata,
        preview_width: pw,
        preview_height: ph,
        preview_data_url: format!("data:image/png;base64,{}", STANDARD.encode(png)),
        composite: CompositeDisplay {
            product: profile.product,
            sources: pins,
            scale: profile.scale,
            offset: profile.offset,
            display_ranges: ranges,
            sample_count: pw * ph,
            valid_sample_count: common.len() as u32,
            derived,
        },
    })
}

pub(crate) fn sample(root: &Path, jobs: &[Job; 3], x: f64, y: f64) -> Result<CompositePixel> {
    if !x.is_finite() || !y.is_finite() {
        return Err("RGB coordinates must be finite".into());
    }
    let (profile, pins) = validate_jobs(jobs)?;
    let deadline = Instant::now() + Duration::from_secs(55);
    let mut geometry = None;
    let mut values = [0; 3];
    let mut pixel = [0; 2];
    let mut center = [0.0; 2];
    for (channel, job) in jobs.iter().enumerate() {
        let mut source = source_with_deadline(root, job, deadline)?;
        let current = grid(&source);
        if geometry
            .as_ref()
            .is_some_and(|previous| previous != &current)
        {
            return Err(
                "Local RGB bands have different grids; no implicit resampling is performed".into(),
            );
        }
        let [left, bottom, right, top] = source.bounds;
        if x < left || x >= right || y <= bottom || y > top {
            return Err("The coordinate is outside the source raster pixel grid".into());
        }
        let col = ((x - left) / source.pixel_size[0]).floor() as u32;
        let row = ((top - y) / source.pixel_size[1]).floor() as u32;
        let [cw, ch, columns] = chunk_grid(&mut source)?;
        let index = row / ch * columns + col / cw;
        let (width, height) = source.decoder.chunk_data_dimensions(index);
        if col % cw >= width || row % ch >= height {
            return Err("RGB pixel is outside its source chunk".into());
        }
        values[channel] = read_chunk(&mut source, index)?
            .get(((row % ch) * width + col % cw) as usize)
            .ok_or("Missing original RGB DN")?;
        pixel = [col, row];
        center = [
            left + (col as f64 + 0.5) * source.pixel_size[0],
            top - (row as f64 + 0.5) * source.pixel_size[1],
        ];
        geometry = Some(current);
    }
    let channel_no_data = values.map(|value| value == profile.nodata);
    Ok(CompositePixel {
        artifact: None,
        sources: pins,
        crs: geometry.ok_or("Missing RGB grid")?.crs,
        coordinate: [x, y],
        pixel,
        center,
        values,
        reflectances: values.map(|value| {
            (value != profile.nodata).then_some(value as f64 * profile.scale + profile.offset)
        }),
        channel_no_data,
        is_no_data: channel_no_data.into_iter().any(|value| value),
    })
}

impl JobManager {
    async fn composite_jobs(&self, ids: [String; 3]) -> Result<[Job; 3]> {
        let mut jobs = Vec::with_capacity(3);
        for id in ids {
            jobs.push(self.get(&id).await.ok_or("Unknown RGB source job")?);
        }
        jobs.try_into()
            .map_err(|_| "RGB requires exactly three source jobs".into())
    }

    pub async fn inspect_composite(
        &self,
        request: CompositeRequest,
    ) -> Result<CompositeInspection> {
        let jobs = self.composite_jobs(request.job_ids).await?;
        scientific::validate_pairing(self, &jobs).await?;
        let root = self.inner.root.clone();
        let permit = tokio::time::timeout(
            Duration::from_secs(5),
            self.inner.raster_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| "Raster inspection is busy; try again shortly")?
        .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            inspect(&root, &jobs, PREVIEW_EDGE)
        })
        .await
        .map_err(io_error)?
    }

    pub async fn sample_composite(&self, request: CompositePixelRequest) -> Result<CompositePixel> {
        let jobs = self.composite_jobs(request.job_ids).await?;
        scientific::validate_pairing(self, &jobs).await?;
        let root = self.inner.root.clone();
        let permit = tokio::time::timeout(
            Duration::from_secs(5),
            self.inner.raster_permits.clone().acquire_owned(),
        )
        .await
        .map_err(|_| "Raster inspection is busy; try again shortly")?
        .map_err(io_error)?;
        tokio::task::spawn_blocking(move || {
            let _permit = permit;
            sample(&root, &jobs, request.x, request.y)
        })
        .await
        .map_err(io_error)?
    }
}

#[cfg(test)]
mod tests;
