use super::*;

pub(super) fn check_header<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    spec: &RgbSpec,
) -> Result<Header> {
    let header = validate_sample_header(decoder, &spec.profile, 3)?;
    let actual = Grid {
        width: header.width,
        height: header.height,
        crs: header.crs.clone(),
        bounds: header.bounds,
        pixel_size: header.pixel_size,
        pixel_interpretation: if header.pixel_is_point {
            "PixelIsPoint"
        } else {
            "PixelIsArea"
        }
        .into(),
    };
    if actual != spec.grid
        || decoder
            .get_tag_ascii_string(Tag::Unknown(42112))
            .map_err(io_error)?
            != encode::metadata(spec)?
    {
        return Err(
            "Scientific RGB grid or calibration metadata differs from its pinned specification"
                .into(),
        );
    }
    Ok(header)
}

fn source(root: &Path, job: &Job) -> Result<Source> {
    validate_stored(job)?;
    if job.status != JobStatus::Succeeded {
        return Err("Scientific RGB is not complete".into());
    }
    let spec = job.rgb_spec.as_ref().unwrap();
    let deadline = Instant::now() + Duration::from_secs(55);
    let mut decoder = verified_decoder(root, job, deadline)?;
    let header = check_header(&mut decoder, spec)?;
    Ok(Source {
        decoder,
        width: header.width,
        height: header.height,
        crs: header.crs,
        bounds: header.bounds,
        pixel_size: header.pixel_size,
        profile: spec.profile.clone(),
        deadline,
        pixel_is_point: header.pixel_is_point,
    })
}

pub(super) fn chunk<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    profile: &Profile,
    index: u32,
) -> Result<Samples> {
    let values = match decoder.read_chunk(index).map_err(io_error)? {
        DecodingResult::U16(v) if !profile.signed => Samples::Unsigned(v),
        DecodingResult::I16(v) if profile.signed => Samples::Signed(v),
        _ => return Err("Scientific RGB sample type changed".into()),
    };
    let (w, h) = decoder.chunk_data_dimensions(index);
    if values.len() != w as usize * h as usize * 3 {
        return Err("Scientific RGB chunk sample count is invalid".into());
    }
    Ok(values)
}

fn artifact(job: &Job) -> Option<CompositeArtifact> {
    Some(CompositeArtifact {
        job_id: job.id.clone(),
        sha256: job.sha256.clone().unwrap_or_default(),
    })
}

pub(crate) fn inspect(root: &Path, job: &Job, edge: u32) -> Result<CompositeInspection> {
    if edge == 0 || edge > PREVIEW_EDGE {
        return Err("Invalid scientific RGB preview size".into());
    }
    let mut source = source(root, job)?;
    let longest = source.width.max(source.height);
    let pw =
        (u64::from(source.width) * u64::from(edge.min(longest)) / u64::from(longest)).max(1) as u32;
    let ph = (u64::from(source.height) * u64::from(edge.min(longest)) / u64::from(longest)).max(1)
        as u32;
    let [cw, ch, columns] = chunk_grid(&mut source)?;
    let mut targets: BTreeMap<u32, Vec<(usize, u32, u32)>> = BTreeMap::new();
    for y in 0..ph {
        check_time(source.deadline)?;
        for x in 0..pw {
            let sx = (u64::from(x) * u64::from(source.width) / u64::from(pw)) as u32;
            let sy = (u64::from(y) * u64::from(source.height) / u64::from(ph)) as u32;
            targets
                .entry(sy / ch * columns + sx / cw)
                .or_default()
                .push(((y * pw + x) as usize, sx % cw, sy % ch));
        }
    }
    let mut bands = vec![vec![source.profile.nodata; (pw * ph) as usize]; 3];
    for (index, pixels) in targets {
        check_time(source.deadline)?;
        let values = chunk(&mut source.decoder, &source.profile, index)?;
        let (w, h) = source.decoder.chunk_data_dimensions(index);
        for (target, x, y) in pixels {
            if x >= w || y >= h {
                return Err("RGB preview is outside its output chunk".into());
            }
            for (c, band) in bands.iter_mut().enumerate() {
                band[target] = values
                    .get(((y * w + x) * 3) as usize + c)
                    .ok_or("Missing RGB output sample")?;
            }
        }
    }
    let spec = job.rgb_spec.as_ref().unwrap();
    let mut result = display(
        grid(&source),
        source.profile.clone(),
        spec.sources.clone().map(|s| s.pin),
        bands,
        [pw, ph],
        source.deadline,
        true,
    )?;
    result.artifact = artifact(job);
    Ok(result)
}

fn sample(root: &Path, job: &Job, x: f64, y: f64) -> Result<CompositePixel> {
    if !x.is_finite() || !y.is_finite() {
        return Err("RGB coordinates must be finite".into());
    }
    let mut source = source(root, job)?;
    let [left, bottom, right, top] = source.bounds;
    if x < left || x >= right || y <= bottom || y > top {
        return Err("The coordinate is outside the RGB output grid".into());
    }
    let col = ((x - left) / source.pixel_size[0]).floor() as u32;
    let row = ((top - y) / source.pixel_size[1]).floor() as u32;
    let [cw, ch, columns] = chunk_grid(&mut source)?;
    let index = row / ch * columns + col / cw;
    let (w, h) = source.decoder.chunk_data_dimensions(index);
    if col % cw >= w || row % ch >= h {
        return Err("RGB pixel is outside its output chunk".into());
    }
    let samples = chunk(&mut source.decoder, &source.profile, index)?;
    let offset = ((row % ch * w + col % cw) * 3) as usize;
    let values = [
        samples.get(offset),
        samples.get(offset + 1),
        samples.get(offset + 2),
    ]
    .map(|v| v.ok_or("Missing RGB output sample"));
    let values = [values[0]?, values[1]?, values[2]?];
    let nodata = values.map(|v| v == source.profile.nodata);
    Ok(CompositePixel {
        artifact: artifact(job),
        sources: job
            .rgb_spec
            .as_ref()
            .unwrap()
            .sources
            .clone()
            .map(|s| s.pin),
        crs: source.crs,
        coordinate: [x, y],
        pixel: [col, row],
        center: [
            left + (f64::from(col) + 0.5) * source.pixel_size[0],
            top - (f64::from(row) + 0.5) * source.pixel_size[1],
        ],
        values,
        reflectances: values.map(|v| {
            (v != source.profile.nodata)
                .then_some(f64::from(v) * source.profile.scale + source.profile.offset)
        }),
        channel_no_data: nodata,
        is_no_data: nodata.into_iter().any(|v| v),
    })
}

impl JobManager {
    pub async fn inspect_scientific_rgb(&self, id: &str) -> Result<CompositeInspection> {
        let job = self.get(id).await.ok_or("Unknown scientific RGB job")?;
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
            inspect(&root, &job, PREVIEW_EDGE)
        })
        .await
        .map_err(io_error)?
    }
    pub async fn sample_scientific_rgb(&self, id: &str, x: f64, y: f64) -> Result<CompositePixel> {
        let job = self.get(id).await.ok_or("Unknown scientific RGB job")?;
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
            sample(&root, &job, x, y)
        })
        .await
        .map_err(io_error)?
    }
}
