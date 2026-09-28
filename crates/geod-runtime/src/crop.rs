//! Pixel-aligned SCL cropping in the source UTM grid. Polygon requests mask
//! pixels outside the WGS84 boundary to nodata; output is ordinary GeoTIFF.
//!
//! Projection implementation and radians contract:
//! https://docs.rs/proj4rs/0.2.0/proj4rs/
use crate::{
    raster::{check_cancel, decode_raster, load_verified_raster, DecodedRaster},
    Job, Result,
};
use proj4rs::Proj;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
    time::{Duration, Instant},
};
use tiff::{
    encoder::{colortype, compression::DeflateLevel, Compression, TiffEncoder},
    tags::Tag,
};
use tokio_util::sync::CancellationToken;

const EDGE_SEGMENTS: usize = 64;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ClipParameters {
    /// "source" (metres in the existing UTM CRS) or "EPSG:4326" (lon/lat degrees).
    pub crs: String,
    pub bounds: [f64; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub geometry: Option<PolygonGeometry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "coordinates", deny_unknown_fields)]
pub enum PolygonGeometry {
    Polygon(Vec<Vec<[f64; 2]>>),
    MultiPolygon(Vec<Vec<Vec<[f64; 2]>>>),
}

impl PolygonGeometry {
    fn polygons(&self) -> Vec<&Vec<Vec<[f64; 2]>>> {
        match self {
            Self::Polygon(rings) => vec![rings],
            Self::MultiPolygon(polygons) => polygons.iter().collect(),
        }
    }
    pub fn bounds(&self) -> Result<[f64; 4]> {
        let polygons = self.polygons();
        if polygons.is_empty() || polygons.len() > 500 {
            return Err("Polygon must have 1 to 500 parts".into());
        }
        let mut extent = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        let mut count = 0usize;
        for polygon in polygons {
            if polygon.is_empty() || polygon.len() > 1000 {
                return Err("Polygon part must have 1 to 1000 rings".into());
            }
            for ring in polygon {
                if ring.len() < 4 {
                    return Err("Polygon rings must have at least four positions".into());
                }
                count += ring.len();
                if count > 30000 {
                    return Err("Polygon has too many positions (maximum 30000)".into());
                }
                if ring.first() != ring.last() {
                    return Err("Polygon rings must be closed".into());
                }
                let mut signed_area = 0.0;
                for edge in ring.windows(2) {
                    let [x, y] = edge[0];
                    let [next_x, next_y] = edge[1];
                    if ![x, y, next_x, next_y].iter().all(|v| v.is_finite())
                        || !(-180.0..=180.0).contains(&x)
                        || !(-80.0..=84.0).contains(&y)
                        || (next_x - x).abs() > 180.0
                    {
                        return Err("Polygon coordinates must be WGS84 within UTM coverage and must not cross the date line".into());
                    }
                    extent[0] = extent[0].min(x);
                    extent[1] = extent[1].min(y);
                    extent[2] = extent[2].max(x);
                    extent[3] = extent[3].max(y);
                    signed_area += x * next_y - next_x * y;
                }
                if signed_area.abs() < 1e-12 {
                    return Err("Polygon rings must have a nonzero area".into());
                }
            }
        }
        if extent[2] - extent[0] > 180.0 {
            return Err("Polygon longitude span must not exceed 180 degrees".into());
        }
        Ok(extent)
    }
}

struct PreparedRing<'a> {
    positions: &'a [[f64; 2]],
    bounds: [f64; 4],
}
impl PreparedRing<'_> {
    fn contains(&self, [x, y]: [f64; 2]) -> bool {
        x >= self.bounds[0]
            && x <= self.bounds[2]
            && y >= self.bounds[1]
            && y <= self.bounds[3]
            && point_in_ring([x, y], self.positions)
    }
}

fn prepared_polygons(geometry: &PolygonGeometry) -> Vec<Vec<PreparedRing<'_>>> {
    geometry
        .polygons()
        .into_iter()
        .map(|polygon| {
            polygon
                .iter()
                .map(|ring| {
                    let mut bounds = [
                        f64::INFINITY,
                        f64::INFINITY,
                        f64::NEG_INFINITY,
                        f64::NEG_INFINITY,
                    ];
                    for [x, y] in ring {
                        bounds[0] = bounds[0].min(*x);
                        bounds[1] = bounds[1].min(*y);
                        bounds[2] = bounds[2].max(*x);
                        bounds[3] = bounds[3].max(*y);
                    }
                    PreparedRing {
                        positions: ring,
                        bounds,
                    }
                })
                .collect()
        })
        .collect()
}

fn prepared_contains(polygons: &[Vec<PreparedRing<'_>>], point: [f64; 2]) -> bool {
    polygons.iter().any(|rings| {
        rings[0].contains(point) && !rings[1..].iter().any(|hole| hole.contains(point))
    })
}

fn point_in_ring([x, y]: [f64; 2], ring: &[[f64; 2]]) -> bool {
    let mut inside = false;
    for edge in ring.windows(2) {
        let [ax, ay] = edge[0];
        let [bx, by] = edge[1];
        if (ay > y) != (by > y) && x < (bx - ax) * (y - ay) / (by - ay) + ax {
            inside = !inside;
        }
    }
    inside
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropPlan {
    pub width: u32,
    pub height: u32,
    pub band_count: u8,
    pub data_type: String,
    pub format: String,
    pub crs: String,
    pub bounds: [f64; 4],
    pub pixel_size: [f64; 2],
    pub nodata: Option<u8>,
    /// [x offset, y offset, width, height] in the original raster grid.
    pub window: [u32; 4],
    pub requested_bounds: [f64; 4],
    pub requested_crs: String,
    pub projected_bounds: [f64; 4],
    pub source_bounds: [f64; 4],
    pub source_id: String,
    pub source_sha256: String,
    pub warnings: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub masked_pixels: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CropOutput {
    pub output_path: String,
    pub sha256: String,
    pub bytes: u64,
    pub plan: CropPlan,
}

/// Blocking operation. The caller owns admission control / the raster semaphore.
pub fn plan_crop(root: &Path, source: &Job, parameters: &ClipParameters) -> Result<CropPlan> {
    validate_parameters(parameters)?;
    let raster = load_verified_raster(root, source, None)?;
    plan_from_raster(&raster, &source.id, parameters)
}

fn validate_parameters(parameters: &ClipParameters) -> Result<()> {
    let b = parameters.bounds;
    if !b.iter().all(|value| value.is_finite()) || b[1] >= b[3] {
        return Err(
            "Crop bounds must be finite with increasing minimum/maximum coordinates".into(),
        );
    }
    if b[0] >= b[2] {
        return Err(
            "Crop longitude/x bounds must increase; antimeridian-crossing bounds are unsupported"
                .into(),
        );
    }
    if parameters.crs == "EPSG:4326"
        && (b[0] < -180.0 || b[2] > 180.0 || b[1] < -80.0 || b[3] > 84.0 || b[2] - b[0] > 180.0)
    {
        return Err("WGS84 crop bounds must use longitude -180..180 and UTM latitude -80..84, without crossing the antimeridian".into());
    }
    if let Some(geometry) = &parameters.geometry {
        if parameters.crs != "EPSG:4326" {
            return Err("Polygon clips require EPSG:4326 bounds".into());
        }
        let extent = geometry.bounds()?;
        if extent[0] >= parameters.bounds[2]
            || extent[2] <= parameters.bounds[0]
            || extent[1] >= parameters.bounds[3]
            || extent[3] <= parameters.bounds[1]
        {
            return Err("Polygon mask and output window must overlap".into());
        }
    }
    Ok(())
}

fn plan_from_raster(
    raster: &DecodedRaster,
    source_id: &str,
    parameters: &ClipParameters,
) -> Result<CropPlan> {
    validate_parameters(parameters)?;
    let mut warnings = Vec::new();
    let projected = if parameters.crs == "source" || parameters.crs == raster.crs {
        parameters.bounds
    } else if parameters.crs == "EPSG:4326" {
        warnings.push(if parameters.geometry.is_some() { "WGS84 polygon envelope was aligned to the source grid; pixels outside its boundary will be set to nodata. Pixels are not reprojected." } else { "WGS84 edges were densified and enclosed in a source-CRS rectangle; pixels are not reprojected or polygon-masked." }.into());
        projected_envelope(parameters.bounds, &raster.crs)?
    } else {
        return Err("Crop CRS must be source, the source UTM EPSG code, or EPSG:4326".into());
    };
    let source = raster.bounds;
    let clipped = [
        projected[0].max(source[0]),
        projected[1].max(source[1]),
        projected[2].min(source[2]),
        projected[3].min(source[3]),
    ];
    if clipped[0] >= clipped[2] || clipped[1] >= clipped[3] {
        return Err("The requested crop does not intersect the source raster".into());
    }
    if clipped != projected {
        warnings.push("Requested bounds were clipped to the source raster extent.".into());
    }
    let [dx, dy] = raster.pixel_size;
    let x0 = ((clipped[0] - source[0]) / dx)
        .floor()
        .clamp(0.0, raster.width as f64) as u32;
    let x1 = ((clipped[2] - source[0]) / dx)
        .ceil()
        .clamp(0.0, raster.width as f64) as u32;
    let y0 = ((source[3] - clipped[3]) / dy)
        .floor()
        .clamp(0.0, raster.height as f64) as u32;
    let y1 = ((source[3] - clipped[1]) / dy)
        .ceil()
        .clamp(0.0, raster.height as f64) as u32;
    if x0 >= x1 || y0 >= y1 {
        return Err("The requested crop has no source pixels".into());
    }
    let left = source[0] + x0 as f64 * dx;
    let top = source[3] - y0 as f64 * dy;
    // Use the same origin + extent arithmetic as the encoded GeoTIFF. This
    // avoids false read-back failures from floating-point associativity.
    let bounds = [
        left,
        top - (y1 - y0) as f64 * dy,
        left + (x1 - x0) as f64 * dx,
        top,
    ];
    if bounds != clipped {
        warnings.push("Crop bounds were expanded to complete source pixel boundaries.".into());
    }
    let mut plan = CropPlan {
        width: x1 - x0,
        height: y1 - y0,
        band_count: 1,
        data_type: "UInt8".into(),
        format: "GeoTIFF".into(),
        crs: raster.crs.clone(),
        bounds,
        pixel_size: raster.pixel_size,
        nodata: raster.nodata,
        window: [x0, y0, x1 - x0, y1 - y0],
        requested_bounds: parameters.bounds,
        requested_crs: parameters.crs.clone(),
        projected_bounds: projected,
        source_bounds: source,
        source_id: source_id.into(),
        source_sha256: raster.sha256.clone(),
        warnings,
        masked_pixels: None,
    };
    if let Some(geometry) = &parameters.geometry {
        plan.masked_pixels = Some(mask_polygon(&plan, geometry, raster.nodata, None, None)?);
    }
    Ok(plan)
}

fn mask_polygon(
    plan: &CropPlan,
    geometry: &PolygonGeometry,
    nodata: Option<u8>,
    mut pixels: Option<&mut [u8]>,
    cancel: Option<&CancellationToken>,
) -> Result<u64> {
    if plan.width as u64 * plan.height as u64 > 8_000_000 {
        return Err("Polygon clip exceeds the 8 million pixel mask limit; choose a smaller region or source window".into());
    }
    let nodata = nodata.ok_or("Polygon clipping requires a source nodata value")?;
    let (wgs84, utm) = projections(&plan.crs)?;
    let polygons = prepared_polygons(geometry);
    let deadline = Instant::now() + Duration::from_secs(60);
    let mut masked = 0u64;
    let mut retained = 0u64;
    for row in 0..plan.height {
        check_cancel(cancel)?;
        if Instant::now() > deadline {
            return Err("Polygon masking exceeded the 60 second processing limit".into());
        }
        let northing = plan.bounds[3] - (row as f64 + 0.5) * plan.pixel_size[1];
        for col in 0..plan.width {
            if col % 8192 == 0 && Instant::now() > deadline {
                return Err("Polygon masking exceeded the 60 second processing limit".into());
            }
            let easting = plan.bounds[0] + (col as f64 + 0.5) * plan.pixel_size[0];
            let mut point = (easting, northing, 0.0);
            proj4rs::transform::transform(&utm, &wgs84, &mut point)
                .map_err(|e| format!("Cannot transform output pixel to WGS84: {e}"))?;
            if prepared_contains(&polygons, [point.0.to_degrees(), point.1.to_degrees()]) {
                retained += 1;
            } else {
                if let Some(buffer) = pixels.as_mut() {
                    buffer[row as usize * plan.width as usize + col as usize] = nodata;
                }
                masked += 1;
            }
        }
    }
    if retained == 0 {
        return Err("The polygon contains no source pixel centres".into());
    }
    Ok(masked)
}

fn projections(crs: &str) -> Result<(Proj, Proj)> {
    let epsg = crs
        .strip_prefix("EPSG:")
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or("Invalid source EPSG code")?;
    let (zone, south) = match epsg {
        32601..=32660 => (epsg - 32600, false),
        32701..=32760 => (epsg - 32700, true),
        _ => return Err("WGS84 crop transformation supports only WGS84 UTM source rasters".into()),
    };
    let from = Proj::from_proj_string("+proj=longlat +datum=WGS84 +no_defs")
        .map_err(|e| format!("Cannot initialize WGS84 projection: {e}"))?;
    let to = Proj::from_proj_string(&format!(
        "+proj=utm +zone={zone} {} +datum=WGS84 +units=m +no_defs",
        if south { "+south" } else { "" }
    ))
    .map_err(|e| format!("Cannot initialize source projection: {e}"))?;
    Ok((from, to))
}

fn project_point(from: &Proj, to: &Proj, longitude: f64, latitude: f64) -> Result<[f64; 2]> {
    // proj4rs expects angular coordinates in radians, regardless of user units.
    let mut point = (longitude.to_radians(), latitude.to_radians(), 0.0);
    proj4rs::transform::transform(from, to, &mut point)
        .map_err(|e| format!("WGS84 crop projection failed: {e}"))?;
    if !point.0.is_finite() || !point.1.is_finite() {
        return Err("WGS84 crop projection returned invalid coordinates".into());
    }
    Ok([point.0, point.1])
}

fn projected_envelope(bounds: [f64; 4], crs: &str) -> Result<[f64; 4]> {
    let (from, to) = projections(crs)?;
    let mut envelope = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    // All four edges include corners and 63 interior points. This is explicitly
    // a sampled envelope, not an exact geographic polygon intersection.
    for step in 0..=EDGE_SEGMENTS {
        let fraction = step as f64 / EDGE_SEGMENTS as f64;
        let longitude = bounds[0] + (bounds[2] - bounds[0]) * fraction;
        let latitude = bounds[1] + (bounds[3] - bounds[1]) * fraction;
        for (lon, lat) in [
            (longitude, bounds[1]),
            (longitude, bounds[3]),
            (bounds[0], latitude),
            (bounds[2], latitude),
        ] {
            let [x, y] = project_point(&from, &to, lon, lat)?;
            envelope[0] = envelope[0].min(x);
            envelope[1] = envelope[1].min(y);
            envelope[2] = envelope[2].max(x);
            envelope[3] = envelope[3].max(y);
        }
    }
    Ok(envelope)
}

struct CheckedWriter<'a> {
    file: &'a mut File,
    cancel: &'a CancellationToken,
    deadline: Instant,
}
impl CheckedWriter<'_> {
    fn check(&self) -> std::io::Result<()> {
        if self.cancel.is_cancelled() {
            return Err(std::io::Error::other("Crop cancelled"));
        }
        if Instant::now() > self.deadline {
            return Err(std::io::Error::new(
                std::io::ErrorKind::TimedOut,
                "Crop encoding time limit",
            ));
        }
        Ok(())
    }
}
impl Write for CheckedWriter<'_> {
    fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
        self.check()?;
        self.file.write(buffer)
    }
    fn flush(&mut self) -> std::io::Result<()> {
        self.check()?;
        self.file.flush()
    }
}
impl Seek for CheckedWriter<'_> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.check()?;
        self.file.seek(position)
    }
}

/// Produce one new managed output. The caller must serialize competing job
/// updates and use a fresh UUID. Existing final outputs are never overwritten.
pub fn write_crop(
    root: &Path,
    source: &Job,
    parameters: &ClipParameters,
    output_id: &str,
    cancel: &CancellationToken,
) -> Result<CropOutput> {
    validate_parameters(parameters)?;
    check_cancel(Some(cancel))?;
    if uuid::Uuid::parse_str(output_id)
        .map_err(|_| "Crop output identifier must be a UUID")?
        .to_string()
        != output_id
        || output_id == source.id
    {
        return Err("Crop output identifier must be a new canonical UUID".into());
    }
    let raster = load_verified_raster(root, source, Some(cancel))?;
    let plan = plan_from_raster(&raster, &source.id, parameters)?;
    let canonical_root = root.canonicalize().map_err(|e| e.to_string())?;
    let assets = canonical_root
        .join("assets")
        .canonicalize()
        .map_err(|e| e.to_string())?;
    if assets != canonical_root.join("assets") {
        return Err("Managed asset directory was redirected".into());
    }
    let final_path = assets.join(format!("{output_id}.tif"));
    if final_path.exists() {
        return Err("Crop output already exists; existing assets are never overwritten".into());
    }
    let [x, y, width, height] = plan.window;
    let mut pixels = Vec::with_capacity(width as usize * height as usize);
    for row in y..y + height {
        check_cancel(Some(cancel))?;
        let offset = row as usize * raster.width as usize + x as usize;
        pixels.extend_from_slice(&raster.pixels[offset..offset + width as usize]);
    }
    if let Some(geometry) = &parameters.geometry {
        let masked = mask_polygon(
            &plan,
            geometry,
            raster.nodata,
            Some(&mut pixels),
            Some(cancel),
        )?;
        if plan.masked_pixels != Some(masked) {
            return Err("Polygon mask changed between planning and writing".into());
        }
    }
    drop(raster);
    let mut partial = tempfile::Builder::new()
        .prefix(&format!("{output_id}.crop-"))
        .suffix(".part")
        .tempfile_in(&assets)
        .map_err(|e| format!("Cannot create crop partial file: {e}"))?;
    let deadline = Instant::now() + Duration::from_secs(60);
    {
        let writer = CheckedWriter {
            file: partial.as_file_mut(),
            cancel,
            deadline,
        };
        let mut encoder = TiffEncoder::new(writer)
            .map_err(|e| e.to_string())?
            .with_compression(Compression::Deflate(DeflateLevel::Balanced));
        let mut image = encoder
            .new_image::<colortype::Gray8>(width, height)
            .map_err(|e| e.to_string())?;
        image
            .rows_per_strip(height.min(128))
            .map_err(|e| e.to_string())?;
        let epsg = plan
            .crs
            .strip_prefix("EPSG:")
            .and_then(|v| v.parse::<u16>().ok())
            .ok_or("Invalid crop CRS")?;
        let keys = [
            1u16, 1, 0, 4, 1024, 0, 1, 1, 1025, 0, 1, 1, 3072, 0, 1, epsg, 3076, 0, 1, 9001,
        ];
        image
            .encoder()
            .write_tag(Tag::GeoKeyDirectoryTag, &keys[..])
            .map_err(|e| e.to_string())?;
        image
            .encoder()
            .write_tag(
                Tag::ModelPixelScaleTag,
                &[plan.pixel_size[0], plan.pixel_size[1], 0.0][..],
            )
            .map_err(|e| e.to_string())?;
        image
            .encoder()
            .write_tag(
                Tag::ModelTiepointTag,
                &[0.0, 0.0, 0.0, plan.bounds[0], plan.bounds[3], 0.0][..],
            )
            .map_err(|e| e.to_string())?;
        if let Some(nodata) = plan.nodata {
            image
                .encoder()
                .write_tag(Tag::GdalNodata, nodata.to_string().as_str())
                .map_err(|e| e.to_string())?;
        }
        // write_data installs the configured compressor around each strip;
        // direct write_strip does not in tiff 0.11.3. CheckedWriter checks
        // cancellation at every output write, with strips bounded to 128 rows.
        image
            .write_data(&pixels)
            .map_err(|e| format!("Cannot encode crop: {e}"))?;
    }
    partial
        .as_file()
        .sync_all()
        .map_err(|e| format!("Cannot sync crop output: {e}"))?;
    check_cancel(Some(cancel))?;
    partial
        .as_file_mut()
        .seek(SeekFrom::Start(0))
        .map_err(|e| e.to_string())?;
    let mut encoded = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        check_cancel(Some(cancel))?;
        let count = partial
            .as_file_mut()
            .read(&mut chunk)
            .map_err(|e| e.to_string())?;
        if count == 0 {
            break;
        }
        if encoded.len() + count > 128 * 1024 * 1024 {
            return Err("Crop output exceeded the 128 MiB raster limit".into());
        }
        encoded.extend_from_slice(&chunk[..count]);
    }
    let sha256 = format!("{:x}", Sha256::digest(&encoded));
    let verified = decode_raster(
        &encoded,
        sha256.clone(),
        Instant::now() + Duration::from_secs(60),
        Some(cancel),
    )?;
    if verified.width != plan.width
        || verified.height != plan.height
        || verified.crs != plan.crs
        || verified.bounds != plan.bounds
        || verified.pixel_size != plan.pixel_size
        || verified.nodata != plan.nodata
        || verified.pixels != pixels
    {
        return Err("Crop read-back validation failed; no completed output was published".into());
    }
    check_cancel(Some(cancel))?;
    // Atomic no-clobber publication. NamedTempFile removes partials on all error
    // paths, including cancellation and validation failure.
    partial.persist_noclobber(&final_path).map_err(|e| {
        format!(
            "Cannot commit crop without replacing an existing asset: {}",
            e.error
        )
    })?;
    Ok(CropOutput {
        output_path: final_path.to_string_lossy().into_owned(),
        sha256,
        bytes: encoded.len() as u64,
        plan,
    })
}

#[cfg(test)]
mod tests;
