use super::*;
use geo::{Contains, Coord, Intersects, LineString, MultiPolygon, Point, Polygon, Validation};
pub(super) fn supported(t: u32) -> bool {
    matches!(t, 1 | 3 | 5 | 8 | 11 | 13 | 15 | 18 | 21 | 23 | 25 | 28)
}
pub(super) struct Shapes {
    pub shape_type: u32,
    pub records: Vec<(Value, Option<Value>)>,
    pub coordinates: usize,
    pub outside: usize,
    pub clamped: usize,
    pub topology_work: u64,
}
pub(super) const MAX_TOPOLOGY_WORK: u64 = 50_000_000;
struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl Reader<'_> {
    fn take<const N: usize>(&mut self) -> Result<[u8; N]> {
        let end = self.at.checked_add(N).ok_or("SHP geometry size overflow")?;
        let out = self
            .bytes
            .get(self.at..end)
            .ok_or("Truncated SHP geometry")?
            .try_into()
            .unwrap();
        self.at = end;
        Ok(out)
    }
    fn uint(&mut self) -> Result<usize> {
        Ok(u32::from_le_bytes(self.take()?) as usize)
    }
    fn double(&mut self) -> Result<f64> {
        let n = f64::from_le_bytes(self.take()?);
        if !n.is_finite() {
            return Err("SHP coordinates and ranges must be finite".into());
        }
        Ok(n)
    }
    fn count(&mut self) -> Result<usize> {
        let n = self.uint()?;
        if n > MAX_COORDINATES || n > self.bytes.len() {
            return Err("SHP exceeds bounded geometry sizes".into());
        }
        Ok(n)
    }
    fn range(&mut self) -> Result<[f64; 2]> {
        let b = [self.double()?, self.double()?];
        if b[0] > b[1] {
            return Err("Invalid SHP coordinate range".into());
        }
        Ok(b)
    }
    fn bbox(&mut self) -> Result<[f64; 4]> {
        let b = [
            self.double()?,
            self.double()?,
            self.double()?,
            self.double()?,
        ];
        if b[0] > b[2] || b[1] > b[3] {
            return Err("Invalid SHP bounding box".into());
        }
        Ok(b)
    }
}
fn header(bytes: &[u8]) -> Result<u32> {
    if bytes.len() < 100
        || u32::from_be_bytes(bytes[..4].try_into().unwrap()) != 9994
        || bytes[4..24].iter().any(|n| *n != 0)
        || (u32::from_be_bytes(bytes[24..28].try_into().unwrap()) as usize).checked_mul(2)
            != Some(bytes.len())
        || u32::from_le_bytes(bytes[28..32].try_into().unwrap()) != 1000
    {
        return Err("Invalid SHP or SHX file header or length".into());
    }
    let t = u32::from_le_bytes(bytes[32..36].try_into().unwrap());
    if !supported(t) {
        return Err("Unsupported SHP shape type; MultiPatch is not supported".into());
    }
    let mut r = Reader { bytes, at: 36 };
    r.bbox()?;
    r.range()?;
    r.range()?;
    Ok(t)
}
fn encloses(b: [f64; 4], points: &[Vec<f64>]) -> bool {
    points.iter().all(|p| {
        let eps = 1e-12_f64.max(b.iter().map(|n| n.abs() * 1e-12).fold(0., f64::max));
        p[0] >= b[0] - eps && p[0] <= b[2] + eps && p[1] >= b[1] - eps && p[1] <= b[3] + eps
    })
}
fn envelope_matches(b: [f64; 4], points: &[Vec<f64>]) -> bool {
    if points.is_empty() {
        return true;
    }
    let actual = points.iter().fold(
        [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ],
        |a, p| {
            [
                a[0].min(p[0]),
                a[1].min(p[1]),
                a[2].max(p[0]),
                a[3].max(p[1]),
            ]
        },
    );
    b.iter()
        .zip(actual)
        .all(|(a, v)| (*a - v).abs() <= 1e-12_f64.max(a.abs() * 1e-12))
}
fn area(ring: &[Vec<f64>]) -> f64 {
    let x = ring[0][0];
    let y = ring[0][1];
    ring.windows(2)
        .map(|w| (w[0][0] - x) * (w[1][1] - y) - (w[1][0] - x) * (w[0][1] - y))
        .sum::<f64>()
        * 0.5
}
fn line(points: &[Vec<f64>]) -> LineString<f64> {
    LineString::new(points.iter().map(|p| Coord { x: p[0], y: p[1] }).collect())
}
fn polygon_groups(
    points: &[Vec<f64>],
    parts: &[usize],
    remaining: &mut u64,
) -> Result<Vec<Vec<usize>>> {
    let rings = parts
        .windows(2)
        .map(|w| &points[w[0]..w[1]])
        .collect::<Vec<_>>();
    if rings.len() > 4096 {
        return Err("SHP polygon exceeds 4,096 rings".into());
    }
    // Validation also scans segment pairs within each ring, twice. Include that
    // work, not only cross-ring containment, in the complete bundle's budget.
    let total = points.len() as u64;
    let estimate = 2 * total * total;
    *remaining = remaining
        .checked_sub(estimate)
        .ok_or("Shapefile polygon topology exceeds the local work limit; import smaller layers")?;
    let mut shells = vec![];
    let mut holes = vec![];
    for (i, ring) in rings.iter().enumerate() {
        if ring.len() < 4 || ring[0][..2] != ring[ring.len() - 1][..2] {
            return Err("SHP polygon rings must be closed and have at least four positions".into());
        }
        // Consecutive duplicate XY positions are allowed by the SHP format and
        // retained with their Z/M values. A repeated nonconsecutive position is
        // a self-touch: do not rely on the validator's shared-endpoint exception.
        let mut distinct = Vec::new();
        for p in *ring {
            if distinct.last().is_none_or(|v: &&Vec<f64>| v[..2] != p[..2]) {
                distinct.push(p);
            }
        }
        distinct.pop(); // closing XY position, after consecutive duplicates
        let mut vertices = BTreeSet::new();
        if distinct.into_iter().any(|p| {
            let bits = |n: f64| if n == 0. { 0 } else { n.to_bits() };
            !vertices.insert((bits(p[0]), bits(p[1])))
        }) {
            return Err(
                "SHP polygon contains an invalid or intersecting ring; no repair is applied".into(),
            );
        }
        let a = area(ring);
        if !a.is_finite() || a == 0. {
            return Err("SHP polygon has a degenerate ring".into());
        }
        let polygon = Polygon::new(line(ring), vec![]);
        if !polygon.is_valid() {
            return Err(
                "SHP polygon contains an invalid or intersecting ring; no repair is applied".into(),
            );
        }
        if a < 0. {
            shells.push((i, polygon, a.abs()));
        } else {
            holes.push(i);
        }
    }
    if shells.is_empty() {
        return Err("SHP polygon has no clockwise exterior ring".into());
    }
    // geo's validator does not check interior connectivity. Conservatively
    // reject touching hole boundaries until that separate topology is handled.
    for hole in &holes {
        let boundary = line(rings[*hole]);
        if rings
            .iter()
            .enumerate()
            .any(|(i, ring)| i != *hole && boundary.intersects(&line(ring)))
        {
            return Err("SHP touching interior rings require a separate topology adapter".into());
        }
    }
    let mut groups = shells.iter().map(|s| vec![s.0]).collect::<Vec<_>>();
    for hole in holes {
        let owner = shells
            .iter()
            .enumerate()
            .filter(|(_, (_, p, _))| {
                rings[hole]
                    .iter()
                    .any(|v| p.contains(&Point::new(v[0], v[1])))
            })
            .min_by(|a, b| a.1 .2.total_cmp(&b.1 .2))
            .map(|(i, _)| i)
            .ok_or("SHP counterclockwise hole is not inside an exterior ring")?;
        groups[owner].push(hole);
    }
    let all = MultiPolygon::new(
        groups
            .iter()
            .map(|g| {
                Polygon::new(
                    line(rings[g[0]]),
                    g[1..].iter().map(|i| line(rings[*i])).collect(),
                )
            })
            .collect(),
    );
    if !all.is_valid() {
        return Err(
            "SHP polygon holes or exterior rings intersect or overlap; no repair is applied".into(),
        );
    }
    Ok(groups)
}
struct Conversion<'a> {
    transform: &'a proj_core::Transform,
    coordinates: usize,
    outside: usize,
    clamped: usize,
    topology_remaining: u64,
}
impl Conversion<'_> {
    fn points(&mut self, points: Vec<Vec<f64>>) -> Result<Vec<Vec<f64>>> {
        self.coordinates += points.len();
        if self.coordinates > MAX_COORDINATES {
            return Err("SHP exceeds 500,000 coordinates".into());
        }
        points
            .into_iter()
            .map(|mut p| {
                let (mut x, mut y) = self
                    .transform
                    .convert((p[0], p[1]))
                    .map_err(|_| "SHP coordinate conversion failed for a source position")?;
                if !x.is_finite()
                    || !y.is_finite()
                    || !(-180.000000001..=180.000000001).contains(&x)
                    || !(-90.000000001..=90.000000001).contains(&y)
                {
                    return Err("SHP conversion returned invalid WGS84 coordinates".into());
                }
                if x.abs() > 180. || y.abs() > 90. {
                    self.clamped += 1;
                    x = x.clamp(-180., 180.);
                    y = y.clamp(-90., 90.);
                }
                if self
                    .transform
                    .selected_operation()
                    .area_of_use
                    .as_ref()
                    .is_some_and(|a| {
                        let inside = if a.west <= a.east {
                            x >= a.west && x <= a.east
                        } else {
                            x >= a.west || x <= a.east
                        };
                        !inside || y < a.south || y > a.north
                    })
                {
                    self.outside += 1;
                }
                p[0] = x;
                p[1] = y;
                Ok(p)
            })
            .collect()
    }
    fn record(&mut self, bytes: &[u8], shape_type: u32) -> Result<(Value, Option<Value>)> {
        let mut r = Reader { bytes, at: 0 };
        let t = r.uint()? as u32;
        if t == 0 {
            if r.at != bytes.len() {
                return Err("Null SHP shape contains unexpected payload".into());
            }
            return Ok((Value::Null, None));
        }
        if t != shape_type {
            return Err("SHP record type differs from its file header".into());
        }
        let z = matches!(t, 11 | 13 | 15 | 18);
        let has_m = t >= 10;
        let base = match t {
            1 | 11 | 21 => 1,
            3 | 13 | 23 => 3,
            5 | 15 | 25 => 5,
            8 | 18 | 28 => 8,
            _ => return Err("Unsupported SHP shape type".into()),
        };
        let bbox = if base == 1 { None } else { Some(r.bbox()?) };
        let (mut parts, n) = if matches!(base, 3 | 5) {
            let count = r.count()?;
            let n = r.count()?;
            if count == 0 || n == 0 || count > 100000 {
                return Err("SHP line or polygon requires bounded nonempty parts".into());
            }
            let p = (0..count).map(|_| r.uint()).collect::<Result<Vec<_>>>()?;
            (p, n)
        } else if base == 8 {
            (vec![0], r.count()?)
        } else {
            (vec![0], 1)
        };
        if n == 0 {
            return Err("Use a null SHP record for an empty geometry".into());
        }
        if parts[0] != 0
            || parts.windows(2).any(|w| w[0] >= w[1])
            || parts.last().is_some_and(|i| *i >= n)
        {
            return Err("Invalid SHP part offsets".into());
        }
        parts.push(n);
        let mut points = (0..n)
            .map(|_| Ok(vec![r.double()?, r.double()?]))
            .collect::<Result<Vec<_>>>()?;
        if bbox.is_some_and(|b| !envelope_matches(b, &points)) {
            return Err("SHP record bounding box does not match its coordinates".into());
        }
        if z {
            let range = if base == 1 { None } else { Some(r.range()?) };
            for p in &mut points {
                let height = r.double()?;
                if range.is_some_and(|b| height < b[0] || height > b[1]) {
                    return Err("SHP Z range does not enclose its heights".into());
                }
                p.push(height);
            }
        }
        let m_present = has_m && r.at < bytes.len();
        let measures = if m_present {
            let range = if base == 1 { None } else { Some(r.range()?) };
            let m = (0..n)
                .map(|_| {
                    let m = r.double()?;
                    if m < -1e38 {
                        return Ok(Value::Null);
                    }
                    if range.is_some_and(|b| m < b[0] || m > b[1]) {
                        return Err("SHP M range does not enclose its measures".into());
                    }
                    Ok(json!(m))
                })
                .collect::<Result<Vec<_>>>()?;
            Some(m)
        } else if t == 21 {
            return Err("PointM requires its measure".into());
        } else if has_m {
            Some(vec![Value::Null; n])
        } else {
            None
        };
        if r.at != bytes.len() {
            return Err("SHP geometry has a truncated or unexpected trailing payload".into());
        }
        let groups = if base == 5 {
            Some(polygon_groups(
                &points,
                &parts,
                &mut self.topology_remaining,
            )?)
        } else {
            None
        };
        if base == 3 && parts.windows(2).any(|w| w[1] - w[0] < 2) {
            return Err("SHP lines need at least two positions per part".into());
        }
        let points = self.points(points)?;
        let (kind, coords, m) = match base {
            1 => ("Point", json!(points[0]), measures.map(|m| m[0].clone())),
            8 => ("MultiPoint", json!(points), measures.map(|m| json!(m))),
            3 => {
                let lines = parts
                    .windows(2)
                    .map(|w| json!(&points[w[0]..w[1]]))
                    .collect::<Vec<_>>();
                let m = measures.map(|m| {
                    let all = parts
                        .windows(2)
                        .map(|w| json!(&m[w[0]..w[1]]))
                        .collect::<Vec<_>>();
                    if all.len() == 1 {
                        all[0].clone()
                    } else {
                        json!(all)
                    }
                });
                if lines.len() == 1 {
                    ("LineString", lines[0].clone(), m)
                } else {
                    ("MultiLineString", json!(lines), m)
                }
            }
            5 => {
                let groups = groups.unwrap();
                let polys = groups
                    .iter()
                    .map(|g| {
                        json!(g
                            .iter()
                            .map(|i| &points[parts[*i]..parts[*i + 1]])
                            .collect::<Vec<_>>())
                    })
                    .collect::<Vec<_>>();
                let m = measures.map(|m| {
                    let polys = groups
                        .iter()
                        .map(|g| {
                            json!(g
                                .iter()
                                .map(|i| &m[parts[*i]..parts[*i + 1]])
                                .collect::<Vec<_>>())
                        })
                        .collect::<Vec<_>>();
                    if polys.len() == 1 {
                        polys[0].clone()
                    } else {
                        json!(polys)
                    }
                });
                if polys.len() == 1 {
                    ("Polygon", polys[0].clone(), m)
                } else {
                    ("MultiPolygon", json!(polys), m)
                }
            }
            _ => unreachable!(),
        };
        Ok((json!({"type":kind,"coordinates":coords}), m))
    }
}
pub(super) fn decode(
    shp: &[u8],
    shx: &[u8],
    transform: &proj_core::Transform,
    topology_available: u64,
) -> Result<Shapes> {
    let shape_type = header(shp)?;
    header(shx)?;
    if shp[..24] != shx[..24]
        || shp[28..100] != shx[28..100]
        || !(shx.len() - 100).is_multiple_of(8)
    {
        return Err("SHP and SHX header metadata do not match".into());
    }
    let count = (shx.len() - 100) / 8;
    if count > MAX_FEATURES {
        return Err("SHP exceeds 50,000 records".into());
    }
    let mut out = vec![];
    let mut at = 100usize;
    let mut convert = Conversion {
        transform,
        coordinates: 0,
        outside: 0,
        clamped: 0,
        topology_remaining: topology_available,
    };
    let mut head = Reader { bytes: shp, at: 36 };
    let bounds = head.bbox()?;
    for (i, index) in shx[100..].chunks_exact(8).enumerate() {
        let offset = (u32::from_be_bytes(index[..4].try_into().unwrap()) as usize)
            .checked_mul(2)
            .ok_or("SHX offset overflow")?;
        let len = (u32::from_be_bytes(index[4..].try_into().unwrap()) as usize)
            .checked_mul(2)
            .ok_or("SHX length overflow")?;
        let record = shp.get(at..at + 8).ok_or("Truncated SHP record header")?;
        if offset != at
            || u32::from_be_bytes(record[..4].try_into().unwrap()) as usize != i + 1
            || u32::from_be_bytes(record[4..].try_into().unwrap()) as usize * 2 != len
        {
            return Err("SHX offsets or SHP record numbers and lengths do not match".into());
        }
        let end = at
            .checked_add(8)
            .and_then(|a| a.checked_add(len))
            .ok_or("SHP record size overflow")?;
        let bytes = shp.get(at + 8..end).ok_or("Truncated SHP record")?;
        // Header envelope covers each source record envelope / point.
        if bytes.len() >= 20 && u32::from_le_bytes(bytes[..4].try_into().unwrap()) != 0 {
            let mut r = Reader { bytes, at: 4 };
            let p = if matches!(shape_type, 1 | 11 | 21) {
                vec![vec![r.double()?, r.double()?]]
            } else {
                let b = r.bbox()?;
                vec![vec![b[0], b[1]], vec![b[2], b[3]]]
            };
            if !encloses(bounds, &p) {
                return Err("SHP header bounding box does not enclose its records".into());
            }
        }
        out.push(
            convert
                .record(bytes, shape_type)
                .map_err(|e| format!("Shapefile record {}: {e}", i + 1))?,
        );
        at = end;
    }
    if at != shp.len() {
        return Err("SHP has unindexed or trailing records".into());
    }
    Ok(Shapes {
        shape_type,
        records: out,
        coordinates: convert.coordinates,
        outside: convert.outside,
        clamped: convert.clamped,
        topology_work: topology_available - convert.topology_remaining,
    })
}

#[cfg(test)]
mod topology_tests {
    use super::*;
    fn xy(values: &[(f64, f64)]) -> Vec<Vec<f64>> {
        values.iter().map(|(x, y)| vec![*x, *y]).collect()
    }
    #[test]
    fn single_ring_validation_is_included_in_the_work_budget() {
        let points = xy(&[(0., 0.), (0., 10.), (10., 10.), (10., 0.), (0., 0.)]);
        let mut remaining = 0;
        assert!(polygon_groups(&points, &[0, 5], &mut remaining)
            .unwrap_err()
            .contains("work limit"));
    }
    #[test]
    fn touching_holes_are_explicitly_unsupported_without_connectivity_repair() {
        let points = xy(&[
            (0., 0.),
            (0., 10.),
            (10., 10.),
            (10., 0.),
            (0., 0.),
            (0., 5.),
            (2., 4.),
            (2., 6.),
            (0., 5.),
        ]);
        let mut remaining = MAX_TOPOLOGY_WORK;
        assert!(polygon_groups(&points, &[0, 5, 9], &mut remaining)
            .unwrap_err()
            .contains("touching interior rings"));
    }
    #[test]
    fn repeated_nonconsecutive_vertices_are_rejected_but_adjacent_xy_duplicates_survive() {
        let bad = xy(&[
            (0., 0.),
            (0., 1.),
            (1., 1.),
            (1., 0.),
            (0., 0.),
            (-1., 0.),
            (-1., -1.),
            (0., -1.),
            (0., 0.),
        ]);
        let mut remaining = MAX_TOPOLOGY_WORK;
        assert!(polygon_groups(&bad, &[0, 9], &mut remaining)
            .unwrap_err()
            .contains("invalid or intersecting ring"));
        let good = xy(&[
            (0., 0.),
            (0., 10.),
            (10., 10.),
            (10., 0.),
            (0., 0.),
            (0., 0.),
        ]);
        assert_eq!(
            polygon_groups(&good, &[0, 6], &mut remaining).unwrap(),
            vec![vec![0]]
        );
    }
}
