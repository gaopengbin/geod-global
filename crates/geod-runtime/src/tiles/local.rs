//! Bounded local MVT archive import. Original archive bytes are copied unchanged.
use super::*;

pub(super) fn file_name(name: &str) -> Result<()> {
    if !clean(name, 240)
        || name.contains(['/', '\\', ':'])
        || ![".pmtiles", ".mbtiles"]
            .iter()
            .any(|ext| name.to_ascii_lowercase().ends_with(ext))
    {
        return Err("Choose a local .pmtiles or .mbtiles file".into());
    }
    Ok(())
}

struct Scan<'a> {
    bytes: &'a [u8],
    header: &'a format::Header,
    directories: BTreeSet<(u64, u64)>,
    entries: Vec<format::Entry>,
    ranges: BTreeMap<(u64, usize), http::Receipt>,
    decoded_directory_bytes: usize,
}
impl Scan<'_> {
    fn range(&mut self, offset: u64, length: u64) -> Result<&[u8]> {
        let end = offset.checked_add(length).ok_or("PMTiles range overflow")?;
        let b = self
            .bytes
            .get(offset as usize..end as usize)
            .ok_or("PMTiles section exceeds the archive length")?;
        self.ranges
            .entry((offset, length as usize))
            .or_insert_with(|| http::Receipt {
                offset,
                bytes: length as usize,
                sha256: hash(b),
            });
        Ok(b)
    }
    fn directory(
        &mut self,
        offset: u64,
        length: u64,
        low: u64,
        high: u64,
        depth: u8,
    ) -> Result<()> {
        if depth > 4 || self.directories.len() >= 1024 || !self.directories.insert((offset, length))
        {
            return Err("PMTiles leaf directory cycle or depth limit".into());
        }
        let h = self.header;
        let raw = self.range(offset, length)?;
        let decoded_size =
            format::decompress(raw, h.internal_compression, format::MAX_DIRECTORY)?.len();
        let entries = format::directory(raw, h)?;
        self.decoded_directory_bytes += decoded_size;
        if self.decoded_directory_bytes > 32 * 1024 * 1024 {
            return Err("Local PMTiles directory scan exceeds its limit".into());
        }
        if entries.first().is_none_or(|e| e.id < low) {
            return Err("PMTiles leaf directory escapes its parent range".into());
        }
        for (i, e) in entries.iter().enumerate() {
            let next = entries.get(i + 1).map_or(high, |v| v.id);
            if e.id >= high || e.id + e.run.max(1) > next {
                return Err("PMTiles leaf directory escapes its parent range".into());
            }
            if e.run == 0 {
                self.directory(h.leaf_offset + e.offset, e.length, e.id, next, depth + 1)?;
            } else {
                if self.entries.len() >= MAX_TILES
                    || e.run > MAX_TILES as u64
                    || self.entries.iter().map(|v| v.run).sum::<u64>() + e.run > MAX_TILES as u64
                {
                    return Err("Local PMTiles archive exceeds 512 addressed tiles".into());
                }
                self.entries.push(e.clone());
            }
        }
        Ok(())
    }
}

pub(super) fn inspect(name: &str, bytes: &[u8]) -> Result<Package> {
    file_name(name)?;
    if bytes.len() > MAX_PACKAGE {
        return Err("Local tile file exceeds 128 MiB".into());
    }
    let header = format::Header::parse(
        bytes
            .get(..127)
            .ok_or("Choose a PMTiles version 3 archive")?,
        bytes.len() as u64,
    )?;
    if header.addressed_tiles > MAX_TILES as u64 || header.tile_entries > MAX_TILES as u64 {
        return Err("Local PMTiles archive exceeds 512 addressed tiles".into());
    }
    let mut scan = Scan {
        bytes,
        header: &header,
        directories: BTreeSet::new(),
        entries: Vec::new(),
        ranges: BTreeMap::new(),
        decoded_directory_bytes: 0,
    };
    scan.range(0, 127)?;
    let metadata = format::metadata(
        scan.range(header.metadata_offset, header.metadata_length)?,
        &header,
    )?;
    scan.directory(
        header.root_offset,
        header.root_length,
        0,
        ((1u64 << (2 * (u32::from(header.max_zoom) + 1))) - 1) / 3,
        0,
    )?;
    let mut discovery = vec![
        scan.ranges[&(0, 127)].clone(),
        scan.ranges[&(header.root_offset, header.root_length as usize)].clone(),
        scan.ranges[&(header.metadata_offset, header.metadata_length as usize)].clone(),
    ];
    discovery.sort_by_key(|r| r.offset);
    let entries = scan.entries.clone();
    let contents: BTreeSet<_> = entries.iter().map(|e| (e.offset, e.length)).collect();
    let spans: Vec<_> = contents.iter().copied().collect();
    if spans.windows(2).any(|w| w[0].0 + w[0].1 > w[1].0) {
        return Err("Local PMTiles tile contents overlap".into());
    }
    let addressed = entries.iter().map(|e| e.run).sum::<u64>();
    for (declared, actual) in [
        (header.addressed_tiles, addressed),
        (header.tile_entries, entries.len() as u64),
        (header.tile_contents, contents.len() as u64),
    ] {
        // PMTiles permits zero as an unknown count; still scan and verify every entry.
        if declared != 0 && declared != actual {
            return Err("Local PMTiles header counts do not match its directory".into());
        }
    }
    let mut decoded_total = 0;
    let mut summaries = BTreeMap::new();
    for (offset, length) in &contents {
        let raw = scan.range(header.tile_offset + offset, *length)?;
        let decoded = format::decompress(raw, header.tile_compression, format::MAX_TILE)?;
        decoded_total += decoded.len();
        if decoded_total > 256 * 1024 * 1024 {
            return Err("Local PMTiles decoded tile data exceeds its limit".into());
        }
        let layers = mvt::inspect(&decoded)?;
        if layers.iter().any(|l| {
            !metadata["vector_layers"]
                .as_array()
                .unwrap()
                .iter()
                .any(|v| v["id"] == l.name)
        }) {
            return Err("Local PMTiles tile layer is not declared in its metadata".into());
        }
        summaries.insert((*offset, *length), layers);
    }
    let mut tiles = Vec::new();
    let mut coverage: [f64; 4] = [180., 90., -180., -90.];
    for e in &entries {
        let receipt = &scan.ranges[&(header.tile_offset + e.offset, e.length as usize)];
        for id in e.id..e.id + e.run {
            let coordinate = format::coordinate(id)?;
            if coordinate.z < header.min_zoom || coordinate.z > header.max_zoom {
                return Err("Local PMTiles tile level is outside its header range".into());
            }
            let b = tile_bounds(&coordinate);
            coverage = [
                coverage[0].min(b[0]),
                coverage[1].min(b[1]),
                coverage[2].max(b[2]),
                coverage[3].max(b[3]),
            ];
            tiles.push(TileReceipt {
                coordinate,
                source_offset: Some(receipt.offset),
                package_offset: Some(receipt.offset),
                image: None,
                bytes: receipt.bytes,
                sha256: receipt.sha256.clone(),
                layers: summaries[&(e.offset, e.length)].clone(),
            });
        }
    }
    if tiles.is_empty() {
        return Err("The archive contains no tiles".into());
    }
    let source_name: String = name[..name.len() - 8]
        .trim()
        .chars()
        .take(80)
        .collect::<String>()
        .trim()
        .to_owned();
    let source_name = if source_name.is_empty() {
        "PMTiles".to_owned()
    } else {
        source_name
    };
    let mut source = Source {
        id: Uuid::new_v4().to_string(),
        name: source_name.clone(),
        url: String::new(),
        etag: String::new(),
        total_bytes: bytes.len() as u64,
        header: Some(header.clone()),
        mbtiles: None,
        metadata,
        connected_at: now(),
        ranges: discovery,
        discovery_sha256: String::new(),
        local: Some(LocalSource {
            file_name: name.into(),
            sha256: hash(bytes),
        }),
    };
    source.discovery_sha256 = fingerprint(&source)?;
    let p = Package {
        id: Uuid::new_v4().to_string(),
        name: source_name,
        requested_bounds: header.bounds,
        tile_coverage_bounds: coverage,
        min_zoom: header.min_zoom,
        max_zoom: header.max_zoom,
        bytes: bytes.len(),
        sha256: hash(bytes),
        source,
        created_at: now(),
        ranges: scan.ranges.into_values().collect(),
        tiles,
        absent: Vec::new(),
        selection: "imported-archive".into(),
    };
    valid_package(&p)?;
    Ok(p)
}

impl JobManager {
    pub async fn import_tile_bytes(&self, name: String, bytes: Vec<u8>) -> Result<Package> {
        self.inner.store.lock().await.accepting_jobs()?;
        let _permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        self.store_imported_tile(name, bytes).await
    }
    async fn store_imported_tile(&self, name: String, bytes: Vec<u8>) -> Result<Package> {
        let (p, bytes) = tokio::task::spawn_blocking(move || {
            file_name(&name)?;
            let p = if name.to_ascii_lowercase().ends_with(".mbtiles") {
                mbtiles::inspect(&name, &bytes)?
            } else {
                inspect(&name, &bytes)?
            };
            Ok::<_, String>((p, bytes))
        })
        .await
        .map_err(io_error)??;
        self.save_tile_package(p, bytes).await
    }
    pub async fn open_tile_path(&self, path: PathBuf) -> Result<Package> {
        self.inner.store.lock().await.accepting_jobs()?;
        let _permit = self
            .inner
            .raster_permits
            .clone()
            .acquire_owned()
            .await
            .map_err(io_error)?;
        let (name, bytes) = tokio::task::spawn_blocking(move || {
            let local_name = path
                .file_name()
                .and_then(|s| s.to_str())
                .ok_or("Choose a local .pmtiles or .mbtiles file")?
                .to_owned();
            file_name(&local_name)?;
            let f = std::fs::File::open(storage::regular_file(&path)?).map_err(io_error)?;
            if f.metadata().map_err(io_error)?.len() > MAX_PACKAGE as u64 {
                return Err("Local tile file exceeds 128 MiB".to_owned());
            }
            let mut bytes = Vec::new();
            f.take(MAX_PACKAGE as u64 + 1)
                .read_to_end(&mut bytes)
                .map_err(io_error)?;
            Ok::<_, String>((local_name, bytes))
        })
        .await
        .map_err(io_error)??;
        self.store_imported_tile(name, bytes).await
    }
}

#[cfg(test)]
mod tests;
