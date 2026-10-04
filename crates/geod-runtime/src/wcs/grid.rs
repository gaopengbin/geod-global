use super::*;
use proj4rs::Proj;

pub(super) fn crs(raw: &str) -> Result<(String, bool)> {
    if matches!(
        raw,
        "CRS:84"
            | "urn:ogc:def:crs:OGC:1.3:CRS84"
            | "urn:ogc:def:crs:OGC::CRS84"
            | "http://www.opengis.net/def/crs/OGC/1.3/CRS84"
    ) {
        return Ok(("EPSG:4326".into(), false));
    }
    let code = raw
        .strip_prefix("http://www.opengis.net/def/crs/EPSG/0/")
        .or_else(|| raw.strip_prefix("urn:ogc:def:crs:EPSG::"))
        .or_else(|| raw.strip_prefix("EPSG:"))
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("Unsupported WCS CRS identifier")?;
    if !matches!(code, 4326 | 3857 | 32601..=32660 | 32701..=32760) {
        return Err(
            "WCS supports WGS84, Web Mercator and WGS84 UTM native grids; this CRS is unsupported"
                .into(),
        );
    }
    Ok((format!("EPSG:{code}"), code == 4326))
}
fn xy(mut values: [f64; 2], swap: bool) -> [f64; 2] {
    if swap {
        values.swap(0, 1);
    }
    values
}
pub(super) fn definition(
    low: [i64; 2],
    high: [i64; 2],
    origin: [f64; 2],
    offsets: [[f64; 2]; 2],
    lower: [f64; 2],
    upper: [f64; 2],
    swap: bool,
) -> Result<(u32, u32, [f64; 6], [f64; 4])> {
    let count = |axis: usize| {
        high[axis]
            .checked_sub(low[axis])
            .and_then(|n| n.checked_add(1))
            .ok_or_else(|| "WCS grid index arithmetic overflow".to_string())
    };
    let counts = [count(0)?, count(1)?];
    if counts.iter().any(|n| *n <= 0 || *n > u32::MAX as i64) {
        return Err("WCS grid indices have invalid dimensions".into());
    }
    let origin = xy(origin, swap);
    let offsets = [xy(offsets[0], swap), xy(offsets[1], swap)];
    let x_index = offsets
        .iter()
        .position(|v| v[0] != 0. && v[1] == 0.)
        .ok_or("WCS grid is rotated, sheared or degenerate")?;
    let y_index = 1 - x_index;
    if offsets[y_index][0] != 0. || offsets[y_index][1] == 0. {
        return Err("WCS grid must have perpendicular axis-aligned offset vectors".into());
    }
    let width = counts[x_index] as u32;
    let height = counts[y_index] as u32;
    let mut first = origin;
    let mut last = origin;
    for axis in 0..2 {
        for component in 0..2 {
            first[component] += low[axis] as f64 * offsets[axis][component];
            last[component] += high[axis] as f64 * offsets[axis][component];
        }
    }
    let dx = offsets[x_index][0].abs();
    let dy = offsets[y_index][1].abs();
    let bounds = [
        first[0].min(last[0]) - dx / 2.,
        first[1].min(last[1]) - dy / 2.,
        first[0].max(last[0]) + dx / 2.,
        first[1].max(last[1]) + dy / 2.,
    ];
    if bounds.iter().any(|v| !v.is_finite())
        || dx < 1e-12
        || dy < 1e-12
        || bounds[0] >= bounds[2]
        || bounds[1] >= bounds[3]
    {
        return Err("WCS grid has invalid spacing or extent".into());
    }
    let lower = xy(lower, swap);
    let upper = xy(upper, swap);
    // Providers describe either the sample-center envelope or the outer cell
    // footprint. Neither may contradict the independently declared grid.
    for (i, declared) in [lower[0], lower[1], upper[0], upper[1]]
        .into_iter()
        .enumerate()
    {
        let step = if i % 2 == 0 { dx } else { dy };
        let center = bounds[i] + if i < 2 { step / 2. } else { -step / 2. };
        let tolerance = step * 1e-6 + bounds[i].abs() * 1e-12;
        if (declared - bounds[i]).abs() > tolerance && (declared - center).abs() > tolerance {
            return Err("WCS envelope conflicts with its rectified grid".into());
        }
    }
    Ok((
        width,
        height,
        [dx, 0., bounds[0], 0., -dy, bounds[3]],
        bounds,
    ))
}
fn projection(crs: &str) -> Result<Proj> {
    let code = crs
        .strip_prefix("EPSG:")
        .and_then(|s| s.parse::<u16>().ok())
        .ok_or("Unsupported WCS projection")?;
    let definition = match code {
        3857 => "+proj=merc +a=6378137 +b=6378137 +lat_ts=0 +lon_0=0 +x_0=0 +y_0=0 +k=1 +units=m +nadgrids=@null +no_defs".to_string(),
        32601..=32660 => format!("+proj=utm +zone={} +datum=WGS84 +units=m +no_defs", code - 32600),
        32701..=32760 => format!("+proj=utm +zone={} +south +datum=WGS84 +units=m +no_defs", code - 32700),
        _ => return Err("Unsupported WCS projected CRS".into()),
    };
    Proj::from_proj_string(&definition).map_err(io_error)
}
pub(super) fn envelope(bounds: [f64; 4], crs: &str, forward: bool) -> Result<[f64; 4]> {
    if crs == "EPSG:4326" {
        features::bounds(bounds)?;
        return Ok(bounds);
    }
    if forward {
        features::bounds(bounds)?;
    }
    if crs == "EPSG:3857" {
        const R: f64 = 6_378_137.;
        let output = if forward {
            if bounds[1] < -85.0511287798066 || bounds[3] > 85.0511287798066 {
                return Err("WCS Web Mercator subset is outside its latitude domain".into());
            }
            [
                R * bounds[0].to_radians(),
                R * (std::f64::consts::FRAC_PI_4 + bounds[1].to_radians() / 2.)
                    .tan()
                    .ln(),
                R * bounds[2].to_radians(),
                R * (std::f64::consts::FRAC_PI_4 + bounds[3].to_radians() / 2.)
                    .tan()
                    .ln(),
            ]
        } else {
            [
                (bounds[0] / R).to_degrees(),
                (2. * (bounds[1] / R).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees(),
                (bounds[2] / R).to_degrees(),
                (2. * (bounds[3] / R).exp().atan() - std::f64::consts::FRAC_PI_2).to_degrees(),
            ]
        };
        if output.iter().any(|v| !v.is_finite()) {
            return Err("WCS projection returned nonfinite coordinates".into());
        }
        if !forward {
            features::bounds(output)?;
        }
        return Ok(output);
    }
    let wgs = Proj::from_proj_string("+proj=longlat +datum=WGS84 +no_defs").map_err(io_error)?;
    let projected = projection(crs)?;
    if forward {
        let code = crs[5..].parse::<u16>().map_err(io_error)?;
        let zone = code % 100;
        let center = zone as f64 * 6. - 183.;
        if bounds[0] < center - 6.
            || bounds[2] > center + 6.
            || bounds[1] < -80.
            || bounds[3] > 84.
            || code < 32700 && bounds[1] < 0.
            || code >= 32700 && bounds[3] > 0.
        {
            return Err(
                "WCS area lies outside the supported native UTM zone and hemisphere".into(),
            );
        }
    }
    let mut out = [
        f64::INFINITY,
        f64::INFINITY,
        f64::NEG_INFINITY,
        f64::NEG_INFINITY,
    ];
    for index in 0..=256 {
        let t = index as f64 / 256.;
        for (x, y) in [
            (bounds[0] + t * (bounds[2] - bounds[0]), bounds[1]),
            (bounds[0] + t * (bounds[2] - bounds[0]), bounds[3]),
            (bounds[0], bounds[1] + t * (bounds[3] - bounds[1])),
            (bounds[2], bounds[1] + t * (bounds[3] - bounds[1])),
        ] {
            let mut p = if forward {
                (x.to_radians(), y.to_radians(), 0.)
            } else {
                (x, y, 0.)
            };
            let (from, to) = if forward {
                (&wgs, &projected)
            } else {
                (&projected, &wgs)
            };
            proj4rs::transform::transform(from, to, &mut p).map_err(io_error)?;
            let (x, y) = if forward {
                (p.0, p.1)
            } else {
                (p.0.to_degrees(), p.1.to_degrees())
            };
            if !x.is_finite() || !y.is_finite() {
                return Err("WCS grid projection is invalid".into());
            }
            out[0] = out[0].min(x);
            out[1] = out[1].min(y);
            out[2] = out[2].max(x);
            out[3] = out[3].max(y);
        }
    }
    if !forward {
        features::bounds(out)?;
    }
    Ok(out)
}
pub(super) fn plan(
    id: &str,
    description: Description,
    requested: [f64; 4],
    connection: &Connection,
) -> Result<Plan> {
    features::bounds(requested)?;
    let native = envelope(requested, &description.crs, true)?;
    let source = description.native_bounds;
    let dx = description.transform[0];
    let dy = -description.transform[4];
    let clipped = [
        native[0].max(source[0]),
        native[1].max(source[1]),
        native[2].min(source[2]),
        native[3].min(source[3]),
    ];
    if clipped[0] >= clipped[2] || clipped[1] >= clipped[3] {
        return Err("Selected area does not intersect this coverage".into());
    }
    let near_integer = |v: f64| {
        if (v - v.round()).abs() <= 1e-7 {
            v.round()
        } else {
            v
        }
    };
    let x0 = near_integer((clipped[0] - source[0]) / dx).floor().max(0.) as u32;
    let x1 = near_integer((clipped[2] - source[0]) / dx)
        .ceil()
        .min(description.width as f64) as u32;
    let y0 = near_integer((source[3] - clipped[3]) / dy).floor().max(0.) as u32;
    let y1 = near_integer((source[3] - clipped[1]) / dy)
        .ceil()
        .min(description.height as f64) as u32;
    let width = x1.checked_sub(x0).ok_or("Invalid WCS column range")?;
    let height = y1.checked_sub(y0).ok_or("Invalid WCS row range")?;
    let pixels = u64::from(width)
        .checked_mul(u64::from(height))
        .ok_or("WCS pixel count overflow")?;
    let samples = pixels
        .checked_mul(description.fields.len() as u64)
        .ok_or("WCS sample count overflow")?;
    if width == 0 || height == 0 || width > 65536 || height > 65536 || samples > MAX_SAMPLES {
        return Err("WCS subset exceeds 65,536 pixels per edge or its 536,870,912 sample streaming-validation budget; choose a smaller area".into());
    }
    let native_bounds = [
        source[0] + x0 as f64 * dx,
        source[3] - y1 as f64 * dy,
        source[0] + x1 as f64 * dx,
        source[3] - y0 as f64 * dy,
    ];
    let bounds = envelope(native_bounds, &description.crs, false)?;
    let transform = [dx, 0., native_bounds[0], 0., -dy, native_bounds[3]];
    let (_, swapped) = crs(&description.declared_crs)?;
    let intervals = if swapped {
        [
            [native_bounds[1], native_bounds[3]],
            [native_bounds[0], native_bounds[2]],
        ]
    } else {
        [
            [native_bounds[0], native_bounds[2]],
            [native_bounds[1], native_bounds[3]],
        ]
    };
    let format = connection
        .formats
        .iter()
        .find(|s| s.as_str() == "image/tiff")
        .or_else(|| {
            connection.formats.iter().find(|s| {
                matches!(
                    s.as_str(),
                    "image/tiff;application=geotiff" | "image/tiff; application=geotiff"
                )
            })
        })
        .ok_or("WCS no longer has a supported GeoTIFF format")?
        .clone();
    let mut url = request_url(
        &connection.coverage_url,
        "GetCoverage",
        Some(&description.coverage_id),
    )?;
    for (label, interval) in description.axis_labels.iter().zip(intervals) {
        url.query_pairs_mut().append_pair(
            "subset",
            &format!("{label}({:.15},{:.15})", interval[0], interval[1]),
        );
    }
    url.query_pairs_mut().append_pair("format", &format);
    let mut warnings = description.warnings.clone();
    warnings.push("The result is a server-produced rectangular coverage subset on its native grid; no acquisition date, source survey accuracy, scaling or calibration is inferred.".into());
    warnings.push("Declared nil values may be absent from TIFF NoData. An untagged declared nil sample causes validation to fail rather than being silently treated as valid.".into());
    if clipped != native {
        warnings.push("The requested rectangle extends beyond the coverage; the planned output is limited to their intersection.".into());
    }
    if description.crs.starts_with("EPSG:326") || description.crs.starts_with("EPSG:327") {
        warnings.push("The WGS84 rectangle is transformed to a densified native UTM envelope; output uses the enclosing native grid rectangle, not an exact polygon clip.".into());
    }
    Ok(Plan {
        id: id.into(),
        description,
        requested_bounds: requested,
        bounds,
        native_bounds,
        transform,
        width,
        height,
        request_url: url.to_string(),
        format,
        selection: "bbox-native-grid".into(),
        warnings,
    })
}
