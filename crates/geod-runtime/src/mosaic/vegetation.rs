//! MOD13Q1/MYD13Q1 C61: select an entire NDVI/EVI observation using its own QA.
//! Individual index outputs use the identical four-file scene selection. Original
//! signed DN are copied without averaging, rescaling or independent QA mosaics.
use super::*;
use crate::providers::vegetation as vi;

pub const SCHEMA: &str = "geod-modis-vi-selection/v1";
pub const SELECTION: &str = "newest-qualified-complete-ndvi-evi-observation";
pub const KEYS: [&str; 4] = ["ndvi", "evi", "vi_quality", "vi_reliability"];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Policy {
    Good,
    Usable,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub policy: Policy,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Source {
    pub job_id: String,
    pub sha256: String,
    pub href: String,
    pub bytes: u64,
    pub attribution: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scene {
    pub item_id: String,
    pub composite_start: String,
    pub sources: [Source; 4],
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Spec {
    pub schema_version: String,
    pub product: String,
    pub policy: Policy,
    pub selection: String,
    pub bounds: [f64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<crop::PolygonGeometry>,
    pub scenes: Vec<Scene>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SelectionResult {
    pub schema_version: String,
    pub policy: Policy,
    pub spec_sha256: String,
    pub counts_full_resolution: bool,
    pub input_common_valid_pixels: u64,
    pub removed_valid_pixels: u64,
    pub rejected_pixels: u64,
    pub fallback_pixels: u64,
    pub scene_valid_pixels: Vec<u64>,
    /// Row-major little-endian Int16 DN for the two selected index planes.
    pub indices_sha256: [String; 2],
    /// Row-major little-endian UInt32: zero is NoData, 1..N is ordered scene.
    /// This digest proves the two index outputs used the same observation.
    pub selection_sha256: String,
}

fn canonical_id(id: &str) -> bool {
    Uuid::parse_str(id).ok().map(|v| v.to_string()).as_deref() == Some(id)
}
fn digest(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
}
pub(crate) fn spec_hash(spec: &Spec) -> Result<String> {
    Ok(format!(
        "{:x}",
        Sha256::digest(serde_json::to_vec(spec).map_err(io_error)?)
    ))
}

pub(super) fn validate_spec(key: &str, pins: &[MosaicSource], spec: &Spec) -> Result<()> {
    if !vi::KEYS.contains(&key)
        || spec.schema_version != SCHEMA
        || spec.product != vi::PRODUCT
        || spec.selection != SELECTION
        || !crate::projects::valid_bounds(spec.bounds)
        || spec.scenes.is_empty()
        || spec.scenes.len() > crate::projects::MAX_PROJECT_SCENES
        || pins.len() != spec.scenes.len()
        || serde_json::to_vec(spec).map_err(io_error)?.len() > 1024 * 1024
    {
        return Err("Invalid MODIS vegetation selection product, area or scene count".into());
    }
    if let Some(g) = &spec.geometry {
        if g.bounds()? != spec.bounds {
            return Err("Vegetation selection geometry differs from its bounds".into());
        }
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut previous: Option<(&str, &str)> = None;
    let index = usize::from(key == "evi");
    for (scene, pin) in spec.scenes.iter().zip(pins) {
        let period = vi::period(&scene.item_id).ok_or("Invalid vegetation observation identity")?;
        let order = (scene.composite_start.as_str(), scene.item_id.as_str());
        if scene.composite_start != period[0]
            || previous.is_some_and(|v| v >= order)
            || scene.sources[index].job_id != pin.job_id
            || scene.sources[index].sha256 != pin.sha256
        {
            return Err(
                "Vegetation observations are reordered or differ from the selected index pins"
                    .into(),
            );
        }
        previous = Some(order);
        for (source, key) in scene.sources.iter().zip(KEYS) {
            let url = crate::providers::asset_url(&source.href)?;
            if !canonical_id(&source.job_id)
                || !digest(&source.sha256)
                || !ids.insert(&source.job_id)
                || source.bytes == 0
                || source.bytes > crate::MAX_ASSET_BYTES
                || source.attribution.len() > 2048
                || !crate::providers::matches_item(&url, &scene.item_id, key)
            {
                return Err("Vegetation selection needs four distinct pinned originals from each exact scene".into());
            }
        }
    }
    Ok(())
}

pub(super) fn create(
    project: &Project,
    jobs: &BTreeMap<String, Job>,
    policy: Policy,
) -> Result<Spec> {
    let mut scenes = Vec::with_capacity(project.scenes.len());
    for scene in &project.scenes {
        let period = vi::period(&scene.item_id)
            .ok_or("Quality selection supports MOD13Q1/MYD13Q1 C61 only")?;
        let mut sources = Vec::with_capacity(4);
        for key in KEYS {
            let j = crate::prepared::scene_source(scene, jobs, key)
                .ok_or("Download NDVI, EVI, detailed VI quality and pixel reliability for every scene first")?;
            if j.kind != "download" {
                return Err("Vegetation selection requires original science COGs".into());
            }
            crate::raster::reflectance::profile(j)?;
            sources.push(Source {
                job_id: j.id.clone(),
                sha256: j.sha256.clone().unwrap(),
                href: j.href.clone(),
                bytes: j.bytes_downloaded,
                attribution: j.source.clone(),
            });
        }
        scenes.push(Scene {
            item_id: scene.item_id.clone(),
            composite_start: period[0].clone(),
            sources: sources
                .try_into()
                .map_err(|_| "Missing vegetation source")?,
        });
    }
    scenes.sort_by(|a, b| {
        a.composite_start
            .cmp(&b.composite_start)
            .then(a.item_id.cmp(&b.item_id))
    });
    Ok(Spec {
        schema_version: SCHEMA.into(),
        product: vi::PRODUCT.into(),
        policy,
        selection: SELECTION.into(),
        bounds: project.bounds,
        geometry: project.geometry.clone(),
        scenes,
    })
}

/// The first N returned jobs are the selected index, followed by the companion
/// index, detailed QA and signed reliability (N jobs for each layer).
pub(super) fn sources(spec: &Spec, key: &str, jobs: &BTreeMap<String, Job>) -> Result<Vec<Job>> {
    let order = if key == "ndvi" {
        [0, 1, 2, 3]
    } else {
        [1, 0, 2, 3]
    };
    let mut out = Vec::with_capacity(spec.scenes.len() * 4);
    for k in order {
        for s in &spec.scenes {
            let pin = &s.sources[k];
            let j = jobs
                .get(&pin.job_id)
                .ok_or("A pinned vegetation original is missing")?;
            if j.kind != "download"
                || j.status != JobStatus::Succeeded
                || j.asset_key != KEYS[k]
                || j.item_id != s.item_id
                || j.href != pin.href
                || j.sha256.as_deref() != Some(&pin.sha256)
                || j.bytes_downloaded != pin.bytes
                || j.source != pin.attribution
            {
                return Err(
                    "A pinned vegetation original changed or no longer matches its observation"
                        .into(),
                );
            }
            crate::raster::reflectance::profile(j)?;
            out.push(j.clone());
        }
    }
    Ok(out)
}

pub(crate) fn validate_result(spec: &Spec, key: &str, plan: &MosaicPlan) -> Result<()> {
    let r = plan
        .vi_quality
        .as_ref()
        .ok_or("Vegetation selection result is missing")?;
    let count = u64::from(plan.width) * u64::from(plan.height);
    let p = plan
        .calibration
        .as_ref()
        .ok_or("Vegetation index calibration is missing")?;
    if !vi::KEYS.contains(&key)
        || r.schema_version != SCHEMA
        || r.policy != spec.policy
        || r.spec_sha256 != spec_hash(spec)?
        || !r.counts_full_resolution
        || plan.vi_index.as_deref() != Some(key)
        || plan.band_count != 1
        || plan.crs != crate::providers::modis::CRS
        || plan.source_count != spec.scenes.len()
        || r.scene_valid_pixels.len() != spec.scenes.len()
        || plan.width == 0
        || plan.height == 0
        || plan.width > 20000
        || plan.height > 20000
        || plan.overlap_policy != SELECTION
        || p.product != vi::PRODUCT
        || !p.signed
        || p.scale != 0.0001
        || p.offset != 0.0
        || p.nodata != -3000
        || p.bits() != 16
        || p.science_key.is_some()
        || p.calendar_year.is_some()
        || plan.elevation.is_some()
        || plan.aerial.is_some()
        || plan.radar.is_some()
        || plan.quality.is_some()
        || plan.landsat_quality.is_some()
        || plan
            .pixel_size
            .iter()
            .any(|v| !v.is_finite() || (*v - vi::PIXEL).abs() > 1e-6)
        || !plan.bounds.iter().all(|v| v.is_finite())
        || ((plan.bounds[2] - plan.bounds[0]) - f64::from(plan.width) * vi::PIXEL).abs() > 1e-6
        || ((plan.bounds[3] - plan.bounds[1]) - f64::from(plan.height) * vi::PIXEL).abs() > 1e-6
        || plan.masked_pixels > count
        || plan.covered_pixels > count - plan.masked_pixels
        || r.input_common_valid_pixels > count - plan.masked_pixels
        || r.input_common_valid_pixels < plan.covered_pixels
        || r.removed_valid_pixels != r.input_common_valid_pixels - plan.covered_pixels
        || r.rejected_pixels != count - plan.covered_pixels
        || r.fallback_pixels > plan.covered_pixels
        || r.scene_valid_pixels
            .iter()
            .try_fold(0u64, |sum, v| sum.checked_add(*v))
            != Some(plan.covered_pixels)
        || r.indices_sha256.iter().any(|v| !digest(v))
        || !digest(&r.selection_sha256)
    {
        return Err(
            "Vegetation quality rules, full-resolution counts or paired grid changed".into(),
        );
    }
    Ok(())
}

pub(crate) fn validate_header<R: Read + Seek>(decoder: &mut Decoder<R>, job: &Job) -> Result<()> {
    if let Some(spec) = job.mosaic.as_ref().and_then(|m| m.vi_selection.as_ref()) {
        let plan = job
            .mosaic_output
            .as_ref()
            .ok_or("Vegetation quality output plan is missing")?;
        validate_result(spec, &job.asset_key, plan)?;
        let actual = decoder
            .get_tag_ascii_string(Tag::ImageDescription)
            .map_err(io_error)?;
        let result: SelectionResult =
            serde_json::from_str(actual.trim_matches('\0')).map_err(io_error)?;
        if Some(&result) != plan.vi_quality.as_ref() {
            return Err("Vegetation TIFF selection metadata differs from the pinned result".into());
        }
    }
    Ok(())
}

pub(super) fn empty(spec: &Spec) -> Result<SelectionResult> {
    Ok(SelectionResult {
        schema_version: SCHEMA.into(),
        policy: spec.policy,
        spec_sha256: spec_hash(spec)?,
        counts_full_resolution: true,
        input_common_valid_pixels: 0,
        removed_valid_pixels: 0,
        rejected_pixels: 0,
        fallback_pixels: 0,
        scene_valid_pixels: vec![0; spec.scenes.len()],
        indices_sha256: [String::new(), String::new()],
        selection_sha256: String::new(),
    })
}

/// GeoD policy, not a NASA recommended threshold. Both variants reject missing
/// QA, undefined usefulness codes, usefulness >=12, high aerosol, adjacent/mixed
/// clouds, snow/ice and shadow. BRDF and land/water bits are not quality ranks.
pub(super) fn accepts(policy: Policy, quality: u16, reliability: i8) -> bool {
    let modland = quality & 3;
    let usefulness = (quality >> 2) & 15;
    quality != u16::MAX
        && matches!(usefulness, 0 | 1 | 2 | 4 | 8 | 9 | 10)
        && ((quality >> 6) & 3) != 3
        && quality & ((1 << 8) | (1 << 10) | (1 << 14) | (1 << 15)) == 0
        && match policy {
            Policy::Good => reliability == 0 && modland == 0,
            Policy::Usable => matches!(reliability, 0 | 1) && modland <= 1,
        }
}

pub(super) struct Counter {
    pub result: SelectionResult,
    indices: [Sha256; 2],
    selection: Sha256,
}
impl Counter {
    pub fn new(result: SelectionResult) -> Self {
        Self {
            result,
            indices: [Sha256::new(), Sha256::new()],
            selection: Sha256::new(),
        }
    }
    pub fn finish(mut self, count: u64, covered: u64) -> SelectionResult {
        self.result.removed_valid_pixels = self.result.input_common_valid_pixels - covered;
        self.result.rejected_pixels = count - covered;
        self.result.indices_sha256 = self.indices.map(|h| format!("{:x}", h.finalize()));
        self.result.selection_sha256 = format!("{:x}", self.selection.finalize());
        self.result
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn copy_block(
    rasters: &mut [SourceRaster],
    key: &str,
    grid: &MosaicGrid,
    pixels: &mut [u8],
    covered: &mut [bool],
    inside: Option<&[bool]>,
    counter: &mut Counter,
    cancel: &CancellationToken,
    deadline: Instant,
) -> Result<()> {
    let n = counter.result.scene_valid_pixels.len();
    if rasters.len() != 4 * n {
        return Err("Vegetation companion source count changed".into());
    }
    let count = covered.len();
    let mut ndvi = vec![-3000i16; count];
    let mut evi = ndvi.clone();
    let mut winner = vec![0u32; count];
    let mut newest_complete = vec![0u32; count];
    for s in 0..n {
        check_cancel(Some(cancel))?;
        let mut planes = Vec::with_capacity(4);
        for group in 0..4 {
            let size = if group == 3 { 1 } else { 2 };
            let fill = if group == 3 {
                vec![255]
            } else if group == 2 {
                u16::MAX.to_ne_bytes().to_vec()
            } else {
                (-3000i16).to_ne_bytes().to_vec()
            };
            let mut data = fill.repeat(count);
            let mut valid = vec![false; count];
            copy_source(
                &mut rasters[group * n + s],
                grid,
                &mut data,
                &mut valid,
                cancel,
                deadline,
            )?;
            if data.len() != count * size {
                return Err("Vegetation block sample size changed".into());
            }
            planes.push(data);
        }
        for i in 0..count {
            if i % grid.dimensions[0] as usize == 0 {
                check_cancel(Some(cancel))?;
            }
            if inside.is_some_and(|v| !v[i]) {
                continue;
            }
            let a = i16::from_ne_bytes([planes[0][2 * i], planes[0][2 * i + 1]]);
            let b = i16::from_ne_bytes([planes[1][2 * i], planes[1][2 * i + 1]]);
            if !(-2000..=10000).contains(&a) || !(-2000..=10000).contains(&b) {
                continue;
            }
            newest_complete[i] = s as u32 + 1;
            let quality = u16::from_ne_bytes([planes[2][2 * i], planes[2][2 * i + 1]]);
            if accepts(counter.result.policy, quality, planes[3][i] as i8) {
                winner[i] = s as u32 + 1;
                if key == "ndvi" {
                    ndvi[i] = a;
                    evi[i] = b;
                } else {
                    ndvi[i] = b;
                    evi[i] = a;
                }
            }
        }
    }
    for i in 0..count {
        counter.result.input_common_valid_pixels += u64::from(newest_complete[i] > 0);
        if winner[i] > 0 {
            covered[i] = true;
            counter.result.scene_valid_pixels[winner[i] as usize - 1] += 1;
            counter.result.fallback_pixels += u64::from(winner[i] < newest_complete[i]);
            let sample = if key == "ndvi" { ndvi[i] } else { evi[i] };
            pixels[2 * i..2 * i + 2].copy_from_slice(&sample.to_ne_bytes());
        }
        counter.indices[0].update(ndvi[i].to_le_bytes());
        counter.indices[1].update(evi[i].to_le_bytes());
        counter.selection.update(winner[i].to_le_bytes());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiff::encoder::colortype;

    fn synthetic(root: &Path, key: &str, values: &[i32]) -> SourceRaster {
        let path = root.join(Uuid::new_v4().to_string());
        let mut encoder = TiffEncoder::new(File::create(&path).unwrap()).unwrap();
        if key == "vi_reliability" {
            encoder
                .write_image::<colortype::GrayI8>(
                    8,
                    1,
                    &values.iter().map(|v| *v as i8).collect::<Vec<_>>(),
                )
                .unwrap();
        } else if key == "vi_quality" {
            encoder
                .write_image::<colortype::Gray16>(
                    8,
                    1,
                    &values.iter().map(|v| *v as u16).collect::<Vec<_>>(),
                )
                .unwrap();
        } else {
            encoder
                .write_image::<colortype::GrayI16>(
                    8,
                    1,
                    &values.iter().map(|v| *v as i16).collect::<Vec<_>>(),
                )
                .unwrap();
        }
        drop(encoder);
        SourceRaster {
            decoder: source::SourceDecoder::Tiff(Box::new(
                Decoder::new(BufReader::new(File::open(path).unwrap())).unwrap(),
            )),
            width: 8,
            height: 1,
            crs: "MODIS:Sinusoidal".into(),
            bounds: [0., 0., 8., 1.],
            pixel_size: [1., 1.],
            bands: 1,
            nodata: Some(if key == "vi_reliability" {
                -1.
            } else if key == "vi_quality" {
                65535.
            } else {
                -3000.
            }),
            calibration: Some(crate::raster::reflectance::Profile {
                product: vi::PRODUCT.into(),
                signed: key != "vi_quality",
                scale: if matches!(key, "ndvi" | "evi") {
                    0.0001
                } else {
                    1.
                },
                offset: 0.,
                nodata: if key == "vi_reliability" {
                    -1
                } else if key == "vi_quality" {
                    65535
                } else {
                    -3000
                },
                sample_bits: (key == "vi_reliability").then_some(8),
                science_key: None,
                calendar_year: None,
            }),
            elevation: None,
            radar: None,
            quality: None,
            landsat_quality: None,
            coverage_source: None,
        }
    }
    fn run_synthetic(key: &str, policy: Policy) -> (Vec<i16>, SelectionResult) {
        let root = tempfile::tempdir().unwrap();
        let old = [
            vec![10; 8],
            vec![20; 8],
            vec![0; 8],
            vec![0, 0, 0, 0, 0, -1, 0, 0],
        ];
        let new = [
            vec![100, 200, 300, 400, 11000, 600, 700, 0],
            vec![110, -3000, 330, 440, 550, 660, 770, -2000],
            vec![0, 0, 1 << 10, 1, 0, 0, 65535, 0],
            vec![0, 0, 0, 1, 0, -1, 0, 0],
        ];
        let order = if key == "ndvi" {
            [0, 1, 2, 3]
        } else {
            [1, 0, 2, 3]
        };
        let mut rasters = Vec::new();
        for k in order {
            for data in [&old[k], &new[k]] {
                rasters.push(synthetic(root.path(), KEYS[k], data));
            }
        }
        let r = SelectionResult {
            schema_version: SCHEMA.into(),
            policy,
            spec_sha256: "a".repeat(64),
            counts_full_resolution: true,
            input_common_valid_pixels: 0,
            removed_valid_pixels: 0,
            rejected_pixels: 0,
            fallback_pixels: 0,
            scene_valid_pixels: vec![0; 2],
            indices_sha256: [String::new(), String::new()],
            selection_sha256: String::new(),
        };
        let mut counter = Counter::new(r);
        let mut pixels = (-3000i16).to_ne_bytes().repeat(8);
        let mut covered = vec![false; 8];
        copy_block(
            &mut rasters,
            key,
            &MosaicGrid {
                origin: [0., 1.],
                dimensions: [8, 1],
                pixel_size: [1., 1.],
            },
            &mut pixels,
            &mut covered,
            Some(&[false, true, true, true, true, true, true, true]),
            &mut counter,
            &CancellationToken::new(),
            Instant::now() + Duration::from_secs(10),
        )
        .unwrap();
        let count = covered.iter().filter(|v| **v).count() as u64;
        (
            pixels
                .chunks_exact(2)
                .map(|b| i16::from_ne_bytes([b[0], b[1]]))
                .collect(),
            counter.finish(8, count),
        )
    }
    #[test]
    fn complete_pairs_fall_back_together_and_keep_zero_negative_and_masked_nodata() {
        let (ndvi, n) = run_synthetic("ndvi", Policy::Good);
        let (evi, e) = run_synthetic("evi", Policy::Good);
        assert_eq!(ndvi, [-3000, 10, 10, 10, 10, -3000, 10, 0]);
        assert_eq!(evi, [-3000, 20, 20, 20, 20, -3000, 20, -2000]);
        assert_eq!(n, e);
        assert_eq!(n.scene_valid_pixels, [5, 1]);
        assert_eq!(n.fallback_pixels, 3);
        assert_eq!(
            (
                n.input_common_valid_pixels,
                n.removed_valid_pixels,
                n.rejected_pixels
            ),
            (7, 1, 2)
        );
        assert_eq!(
            n.indices_sha256[0],
            format!(
                "{:x}",
                Sha256::digest(
                    ndvi.iter()
                        .flat_map(|v| v.to_le_bytes())
                        .collect::<Vec<_>>()
                )
            )
        );
    }
    #[test]
    fn usable_policy_accepts_marginal_pairs_with_identical_selection_digest() {
        let (ndvi, n) = run_synthetic("ndvi", Policy::Usable);
        let (evi, e) = run_synthetic("evi", Policy::Usable);
        assert_eq!((ndvi[3], evi[3]), (400, 440));
        assert_eq!(n, e);
        assert_eq!(n.scene_valid_pixels, [4, 2]);
        assert_eq!(n.fallback_pixels, 2);
    }
    #[test]
    fn policies_keep_defined_quality_and_exclude_each_adverse_field() {
        // Product labels tested as synthetic bit patterns, not real-file proof.
        for use_code in [0, 1, 2, 4, 8, 9, 10] {
            let base = (use_code << 2) | (1 << 6) | (1 << 11);
            assert!(accepts(Policy::Good, base, 0));
            assert!(!accepts(Policy::Good, base | 1, 1));
            assert!(accepts(Policy::Usable, base | 1, 1));
            for flag in [1 << 8, 1 << 10, 1 << 14, 1 << 15] {
                assert!(!accepts(Policy::Usable, base | flag, 0));
            }
            assert!(!accepts(Policy::Usable, base | (3 << 6), 0));
            for rank in [-1, 2, 3, 4] {
                assert!(!accepts(Policy::Usable, base, rank));
            }
            for modland in [2, 3] {
                assert!(!accepts(Policy::Usable, base | modland, 0));
            }
            assert!(accepts(Policy::Good, base | (1 << 9), 0));
        }
        for undefined_or_bad in [3, 5, 6, 7, 11, 12, 13, 14, 15] {
            assert!(!accepts(Policy::Usable, undefined_or_bad << 2, 0));
        }
        assert!(!accepts(Policy::Usable, 65535, 0));
    }
}
