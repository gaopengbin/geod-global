//! NASA SRTMGL1 v003: one-degree, 3601-point, signed big-endian HGT originals.
use crate::{io_error, Result};
use std::{
    io::{Read, Seek},
    path::Path,
    time::{Duration, Instant},
};
use zip::{CompressionMethod, ZipArchive};

pub const PREFIX: &str = "/lp-prod-protected/SRTMGL1.003/";
pub const COLLECTION: &str = "SRTMGL1_003";
pub const EDGE: u32 = 3601;
pub const HGT_BYTES: u64 = EDGE as u64 * EDGE as u64 * 2;
pub const NUM_BYTES: u64 = EDGE as u64 * EDGE as u64;
pub const NUM_PREFIX: &str = "/lp-prod-protected/SRTMGL1N.003/";
pub const NUM_COLLECTION: &str = "SRTMGL1N_003";
pub const MAX_ZIP_BYTES: u64 = 64 * 1024 * 1024;

pub fn cell(id: &str) -> Option<[i32; 2]> {
    let name = id.strip_suffix(".SRTMGL1.hgt")?;
    if !name.is_ascii() || name.len() != 7 {
        return None;
    }
    let signed = |s: &str, positive: u8, negative: u8| -> Option<i32> {
        if !s[1..].bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let value: i32 = s[1..].parse().ok()?;
        match s.as_bytes()[0] {
            b if b == positive => Some(value),
            b if b == negative && value != 0 => Some(-value),
            _ => None,
        }
    };
    let lat = signed(&name[..3], b'N', b'S')?;
    let lon = signed(&name[3..], b'E', b'W')?;
    ((-56..60).contains(&lat) && (-180..180).contains(&lon)).then_some([lon, lat])
}

pub fn asset_path(path: &str) -> bool {
    path.strip_prefix(PREFIX)
        .and_then(|tail| tail.split_once('/'))
        .is_some_and(|(id, file)| cell(id).is_some() && file == format!("{id}.zip"))
}

pub fn num_cell(id: &str) -> Option<[i32; 2]> {
    let name = id.strip_suffix(".SRTMGL1N.num")?;
    cell(&format!("{name}.SRTMGL1.hgt"))
}

pub fn num_asset_path(path: &str) -> bool {
    path.strip_prefix(NUM_PREFIX)
        .and_then(|tail| tail.split_once('/'))
        .is_some_and(|(id, file)| num_cell(id).is_some() && file == format!("{id}.zip"))
}

/// Read an exact HGT member without extraction. Reject ambiguous archives,
/// path traversal, alternative grids, encryption and excessive decompression.
pub(crate) fn read_hgt<R: Read + Seek>(reader: R, id: &str, deadline: Instant) -> Result<Vec<u8>> {
    read_hgt_cancelled(reader, id, deadline, None)
}

pub(crate) fn read_hgt_cancelled<R: Read + Seek>(
    reader: R,
    id: &str,
    deadline: Instant,
    cancel: Option<&tokio_util::sync::CancellationToken>,
) -> Result<Vec<u8>> {
    read_grid(reader, id, false, deadline, cancel)
}

pub(crate) fn read_grid<R: Read + Seek>(
    reader: R,
    id: &str,
    num: bool,
    deadline: Instant,
    cancel: Option<&tokio_util::sync::CancellationToken>,
) -> Result<Vec<u8>> {
    if (if num { num_cell(id) } else { cell(id) }).is_none() {
        return Err("Invalid SRTM original tile identifier".into());
    }
    let expected = if num { NUM_BYTES } else { HGT_BYTES };
    let suffix = if num { "num" } else { "hgt" };
    let mut archive = ZipArchive::new(reader).map_err(io_error)?;
    if archive.len() != 1 {
        return Err("SRTMGL1 ZIP must contain one original HGT member".into());
    }
    let mut entry = archive.by_index(0).map_err(io_error)?;
    let basename = &id[..7];
    if ![format!("{basename}.{suffix}"), id.to_owned()].contains(&entry.name().to_owned())
        || entry.is_dir()
        || entry.encrypted()
        || entry.is_symlink()
        || entry.size() != expected
        || entry.compressed_size() > MAX_ZIP_BYTES
        || !matches!(
            entry.compression(),
            CompressionMethod::Stored | CompressionMethod::Deflated
        )
    {
        return Err("SRTMGL1 ZIP member identity, grid or encoding is unsupported".into());
    }
    let mut bytes = Vec::with_capacity(expected as usize);
    let mut buffer = [0_u8; 65536];
    loop {
        crate::raster::check_cancel(cancel)?;
        if Instant::now() > deadline {
            return Err("SRTM HGT reading exceeded its time limit".into());
        }
        let count = entry.read(&mut buffer).map_err(io_error)?;
        if count == 0 {
            break;
        }
        if bytes.len() as u64 + count as u64 > expected {
            return Err("SRTM HGT exceeded its original grid size".into());
        }
        bytes.extend_from_slice(&buffer[..count]);
    }
    // Reading through EOF also verifies the member's ZIP CRC.
    if bytes.len() as u64 != expected {
        return Err("SRTM HGT is truncated".into());
    }
    Ok(bytes)
}

pub(crate) fn verify_hgt(path: &Path, id: &str) -> Result<()> {
    verify_grid(path, id, false)
}

pub(crate) fn verify_grid(path: &Path, id: &str, num: bool) -> Result<()> {
    let file = std::fs::File::open(path).map_err(io_error)?;
    if file.metadata().map_err(io_error)?.len() > MAX_ZIP_BYTES {
        return Err("SRTM ZIP exceeds the 64 MiB limit".into());
    }
    let deadline = Instant::now() + Duration::from_secs(30);
    if num {
        read_grid(file, id, true, deadline, None).map(|_| ())
    } else {
        read_hgt(file, id, deadline).map(|_| ())
    }
}
