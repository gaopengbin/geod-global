//! PMTiles v3 bounded header, directory and Hilbert tile addressing.
//! Format: https://github.com/protomaps/PMTiles/blob/main/spec/v3/spec.md
use super::*;
use flate2::{read::MultiGzDecoder, write::GzEncoder, Compression};
use std::io::{Read, Write};

pub const MAX_METADATA: usize = 2 * 1024 * 1024;
pub const MAX_DIRECTORY: usize = 8 * 1024 * 1024;
pub const MAX_TILE: usize = 8 * 1024 * 1024;
pub const MAX_ZOOM: u8 = 24;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Header {
    pub root_offset: u64,
    pub root_length: u64,
    pub metadata_offset: u64,
    pub metadata_length: u64,
    pub leaf_offset: u64,
    pub leaf_length: u64,
    pub tile_offset: u64,
    pub tile_length: u64,
    pub addressed_tiles: u64,
    pub tile_entries: u64,
    pub tile_contents: u64,
    pub clustered: bool,
    pub internal_compression: u8,
    pub tile_compression: u8,
    pub min_zoom: u8,
    pub max_zoom: u8,
    pub bounds: [f64; 4],
    pub center_zoom: u8,
    pub center: [f64; 2],
}
impl Header {
    pub fn parse(raw: &[u8], total: u64) -> Result<Self> {
        if raw.len() != 127 || &raw[..8] != b"PMTiles\x03" {
            return Err("Choose a PMTiles version 3 archive".into());
        }
        let integer = |i| u64::from_le_bytes(raw[i..i + 8].try_into().unwrap());
        let position =
            |i| f64::from(i32::from_le_bytes(raw[i..i + 4].try_into().unwrap())) / 10_000_000.;
        if raw[96] > 1 || raw[99] != 1 {
            return Err("This PMTiles reader requires MVT vector tiles".into());
        }
        let h = Self {
            root_offset: integer(8),
            root_length: integer(16),
            metadata_offset: integer(24),
            metadata_length: integer(32),
            leaf_offset: integer(40),
            leaf_length: integer(48),
            tile_offset: integer(56),
            tile_length: integer(64),
            addressed_tiles: integer(72),
            tile_entries: integer(80),
            tile_contents: integer(88),
            clustered: raw[96] == 1,
            internal_compression: raw[97],
            tile_compression: raw[98],
            min_zoom: raw[100],
            max_zoom: raw[101],
            bounds: [position(102), position(106), position(110), position(114)],
            center_zoom: raw[118],
            center: [position(119), position(123)],
        };
        h.validate(total)?;
        Ok(h)
    }
    pub fn validate(&self, total: u64) -> Result<()> {
        if ![1, 2].contains(&self.internal_compression)
            || ![1, 2].contains(&self.tile_compression)
            || self.min_zoom > self.max_zoom
            || self.max_zoom > MAX_ZOOM
            || self.center_zoom > MAX_ZOOM
            || self.root_length == 0
            || self.root_offset < 127
            || self
                .root_offset
                .checked_add(self.root_length)
                .is_none_or(|n| n > 16384)
            || self.metadata_length == 0
            || self.metadata_length > MAX_METADATA as u64
            || self.tile_length == 0
            || !self.center.iter().all(|n| n.is_finite())
            || self.center[0].abs() > 180.
            || self.center[1].abs() > 90.
        {
            return Err("Unsupported or invalid PMTiles archive header".into());
        }
        features::bounds(self.bounds)?;
        let mut spans = Vec::new();
        for (offset, length) in [
            (self.root_offset, self.root_length),
            (self.metadata_offset, self.metadata_length),
            (self.leaf_offset, self.leaf_length),
            (self.tile_offset, self.tile_length),
        ] {
            if length == 0 {
                continue;
            }
            let end = offset
                .checked_add(length)
                .filter(|n| *n <= total)
                .ok_or("PMTiles section exceeds the archive length")?;
            if offset < 127 {
                return Err("PMTiles sections overlap the header".into());
            }
            spans.push((offset, end));
        }
        spans.sort_unstable();
        if spans.windows(2).any(|w| w[0].1 > w[1].0) {
            return Err("PMTiles archive sections overlap".into());
        }
        Ok(())
    }
    pub fn encode(&self) -> Result<Vec<u8>> {
        self.validate(self.tile_offset + self.tile_length)?;
        let mut b = vec![0; 127];
        b[..8].copy_from_slice(b"PMTiles\x03");
        for (i, n) in [
            (8, self.root_offset),
            (16, self.root_length),
            (24, self.metadata_offset),
            (32, self.metadata_length),
            (40, self.leaf_offset),
            (48, self.leaf_length),
            (56, self.tile_offset),
            (64, self.tile_length),
            (72, self.addressed_tiles),
            (80, self.tile_entries),
            (88, self.tile_contents),
        ] {
            b[i..i + 8].copy_from_slice(&n.to_le_bytes());
        }
        b[96] = u8::from(self.clustered);
        b[97] = self.internal_compression;
        b[98] = self.tile_compression;
        b[99] = 1;
        b[100] = self.min_zoom;
        b[101] = self.max_zoom;
        b[118] = self.center_zoom;
        for (i, n) in [
            (102, self.bounds[0]),
            (106, self.bounds[1]),
            (110, self.bounds[2]),
            (114, self.bounds[3]),
            (119, self.center[0]),
            (123, self.center[1]),
        ] {
            b[i..i + 4].copy_from_slice(&((n * 10_000_000.).round() as i32).to_le_bytes());
        }
        Ok(b)
    }
}
pub fn decompress(raw: &[u8], compression: u8, max: usize) -> Result<Vec<u8>> {
    if raw.len() > max {
        return Err("PMTiles compressed section exceeds its limit".into());
    }
    let mut out = Vec::new();
    match compression {
        1 => out.extend_from_slice(raw),
        2 => {
            MultiGzDecoder::new(raw)
                .take(max as u64 + 1)
                .read_to_end(&mut out)
                .map_err(|_| "Invalid PMTiles gzip stream")?;
        }
        _ => return Err("PMTiles compression is not supported".into()),
    }
    if out.len() > max {
        return Err("PMTiles decoded section exceeds its limit".into());
    }
    Ok(out)
}
pub fn gzip(b: &[u8]) -> Result<Vec<u8>> {
    let mut e = GzEncoder::new(Vec::new(), Compression::default());
    e.write_all(b).map_err(io_error)?;
    e.finish().map_err(io_error)
}
pub(super) fn varint(raw: &[u8], p: &mut usize) -> Result<u64> {
    let mut n = 0;
    for shift in (0..70).step_by(7) {
        let b = *raw.get(*p).ok_or("Truncated PMTiles integer")?;
        *p += 1;
        if shift == 63 && b > 1 {
            return Err("PMTiles integer overflow".into());
        }
        n |= u64::from(b & 127) << shift;
        if b & 128 == 0 {
            return Ok(n);
        }
    }
    Err("PMTiles integer overflow".into())
}
fn put(out: &mut Vec<u8>, mut n: u64) {
    while n >= 128 {
        out.push((n as u8) | 128);
        n >>= 7;
    }
    out.push(n as u8);
}
#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub id: u64,
    pub run: u64,
    pub length: u64,
    pub offset: u64,
}
pub fn directory(raw: &[u8], h: &Header) -> Result<Vec<Entry>> {
    let b = decompress(raw, h.internal_compression, MAX_DIRECTORY)?;
    let mut p = 0;
    let n = varint(&b, &mut p)?;
    if n == 0 || n > 100_000 || n > b.len() as u64 / 4 {
        return Err("Invalid PMTiles directory entry count".into());
    }
    let mut entries = Vec::with_capacity(n as usize);
    let mut id = 0u64;
    for _ in 0..n {
        id = id
            .checked_add(varint(&b, &mut p)?)
            .ok_or("PMTiles TileID overflow")?;
        entries.push(Entry {
            id,
            run: 0,
            length: 0,
            offset: 0,
        });
    }
    for e in &mut entries {
        e.run = varint(&b, &mut p)?;
    }
    for e in &mut entries {
        e.length = varint(&b, &mut p)?;
    }
    for i in 0..entries.len() {
        let v = varint(&b, &mut p)?;
        entries[i].offset = if v == 0 && i > 0 {
            entries[i - 1]
                .offset
                .checked_add(entries[i - 1].length)
                .ok_or("PMTiles offset overflow")?
        } else {
            v.checked_sub(1).ok_or("Invalid first PMTiles offset")?
        };
    }
    if p != b.len() {
        return Err("PMTiles directory has trailing data".into());
    }
    let last_id = ((1u64 << (2 * (u32::from(h.max_zoom) + 1))) - 1) / 3;
    for (i, e) in entries.iter().enumerate() {
        let (section, max) = if e.run == 0 {
            (h.leaf_length, MAX_DIRECTORY)
        } else {
            (h.tile_length, MAX_TILE)
        };
        if e.id >= last_id
            || e.length == 0
            || e.length > max as u64
            || e.offset.checked_add(e.length).is_none_or(|n| n > section)
            || e.id.checked_add(e.run.max(1)).is_none_or(|end| {
                end > last_id || entries.get(i + 1).is_some_and(|next| end > next.id)
            })
        {
            return Err("PMTiles directory addresses overlap or exceed archive sections".into());
        }
    }
    Ok(entries)
}
pub fn serialize(entries: &[Entry]) -> Result<Vec<u8>> {
    if entries.is_empty() {
        return Err("The region contains no tiles".into());
    }
    let mut b = Vec::new();
    put(&mut b, entries.len() as u64);
    let mut id = 0;
    for e in entries {
        put(
            &mut b,
            e.id.checked_sub(id).ok_or("Unsorted PMTiles output")?,
        );
        id = e.id;
    }
    for e in entries {
        put(&mut b, e.run);
    }
    for e in entries {
        put(&mut b, e.length);
    }
    for (i, e) in entries.iter().enumerate() {
        put(
            &mut b,
            if i > 0 && e.offset == entries[i - 1].offset + entries[i - 1].length {
                0
            } else {
                e.offset + 1
            },
        );
    }
    gzip(&b)
}
pub fn lookup(entries: &[Entry], id: u64) -> Option<&Entry> {
    let n = entries.partition_point(|e| e.id <= id);
    let e = entries.get(n.checked_sub(1)?)?;
    (e.run == 0 || id - e.id < e.run).then_some(e)
}
pub fn tile_id(z: u8, x: u32, y: u32) -> Result<u64> {
    if z > MAX_ZOOM || u64::from(x) >= 1u64 << z || u64::from(y) >= 1u64 << z {
        return Err("Invalid PMTiles tile coordinate".into());
    }
    let (mut x, mut y) = (i64::from(x), i64::from(y));
    let mut s = (1i64 << z) / 2;
    let mut d = 0;
    while s > 0 {
        let rx = i64::from(x & s != 0);
        let ry = i64::from(y & s != 0);
        d += (s * s * ((3 * rx) ^ ry)) as u64;
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - x;
                y = s - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        s /= 2;
    }
    Ok(((1u64 << (2 * u32::from(z))) - 1) / 3 + d)
}
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Coordinate {
    pub z: u8,
    pub x: u32,
    pub y: u32,
}
/// Inverse of the cumulative Hilbert TileID, bounded by our supported zoom.
pub fn coordinate(id: u64) -> Result<Coordinate> {
    let mut z = 0u8;
    while z < MAX_ZOOM && id >= ((1u64 << (2 * u32::from(z + 1))) - 1) / 3 {
        z += 1;
    }
    let start = ((1u64 << (2 * u32::from(z))) - 1) / 3;
    let mut d = id.checked_sub(start).ok_or("Invalid PMTiles TileID")?;
    let n = 1u32 << z;
    if d >= u64::from(n) * u64::from(n) {
        return Err("Invalid PMTiles TileID".into());
    }
    let (mut x, mut y, mut s) = (0u32, 0u32, 1u32);
    while s < n {
        let rx = (d / 2) as u32 & 1;
        let ry = (d as u32 ^ rx) & 1;
        if ry == 0 {
            if rx == 1 {
                x = s - 1 - x;
                y = s - 1 - y;
            }
            std::mem::swap(&mut x, &mut y);
        }
        x += s * rx;
        y += s * ry;
        d /= 4;
        s *= 2;
    }
    Ok(Coordinate { z, x, y })
}
pub fn selected(b: [f64; 4], min: u8, max: u8) -> Result<Vec<Coordinate>> {
    features::bounds(b)?;
    if min > max || max > MAX_ZOOM || b[1] < -85.0511287798066 || b[3] > 85.0511287798066 {
        return Err("Choose a Web Mercator region and valid tile levels".into());
    }
    let mut out = Vec::new();
    for z in min..=max {
        let n = (1u32 << z) as f64;
        let x = |lon: f64| (lon + 180.) / 360. * n;
        let y = |lat: f64| (1. - lat.to_radians().tan().asinh() / std::f64::consts::PI) / 2. * n;
        let west = x(b[0]).floor().clamp(0., n - 1.) as u32;
        let east = ((x(b[2]).ceil() - 1.).clamp(0., n - 1.) as u32).max(west);
        let north = y(b[3]).floor().clamp(0., n - 1.) as u32;
        let south = ((y(b[1]).ceil() - 1.).clamp(0., n - 1.) as u32).max(north);
        let count = u64::from(east - west + 1) * u64::from(south - north + 1);
        if count + out.len() as u64 > MAX_TILES as u64 {
            return Err(
                "Choose a smaller area or fewer tile levels; the extraction exceeds 512 tiles"
                    .into(),
            );
        }
        for x in west..=east {
            for y in north..=south {
                out.push(Coordinate { z, x, y });
            }
        }
    }
    out.sort_by_key(|c| tile_id(c.z, c.x, c.y).unwrap());
    Ok(out)
}
pub fn metadata(raw: &[u8], h: &Header) -> Result<serde_json::Value> {
    let bytes = decompress(raw, h.internal_compression, MAX_METADATA)?;
    let v: serde_json::Value =
        serde_json::from_slice(&bytes).map_err(|_| "Invalid PMTiles metadata JSON")?;
    let layers = v
        .get("vector_layers")
        .and_then(|a| a.as_array())
        .filter(|a| !a.is_empty() && a.len() <= 256)
        .ok_or("PMTiles MVT metadata requires vector_layers")?;
    let mut ids = BTreeSet::new();
    for l in layers {
        let id = l["id"]
            .as_str()
            .filter(|s| clean(s, 256))
            .ok_or("Invalid PMTiles layer identifier")?;
        if !ids.insert(id)
            || !l["fields"].is_object()
            || l["fields"].as_object().unwrap().len() > 512
        {
            return Err("Invalid PMTiles vector layer metadata".into());
        }
    }
    Ok(v)
}
