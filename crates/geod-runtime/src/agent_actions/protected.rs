//! Public discovery and reviewed acquisition reuse native provider authorization.
//! This module never receives, returns or refreshes an account credential.
use super::*;
use crate::{providers as p, AccountProvider, AccountStatus};
use std::collections::BTreeMap;

pub(super) fn account(provider: &str) -> Option<AccountProvider> {
    match provider {
        "copernicus" => Some(AccountProvider::Copernicus),
        "nasa-earthdata" | "nasa-srtm" | "nasa-viirs-suomi" | "nasa-viirs-noaa20"
        | "nasa-viirs-noaa21" => Some(AccountProvider::Nasa),
        _ => None,
    }
}
pub(super) fn usable(value: &AccountStatus) -> bool {
    matches!(value.status, "saved" | "connected")
        && value
            .expires_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .is_some_and(|d| d > Utc::now())
        && value
            .verified_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .is_some_and(|d| d <= Utc::now())
}
pub(super) async fn authorization(manager: &JobManager, provider: &str) -> Option<Value> {
    let required = account(provider)?;
    let statuses = manager.provider_accounts().await;
    let status = statuses.iter().find(|s| s.provider == required);
    let id = if required == AccountProvider::Nasa {
        "nasa-earthdata"
    } else {
        "copernicus"
    };
    let view = status
        .map(catalog::account_view)
        .unwrap_or_else(|| json!({"status":"unavailable","expiresAt":null,"verifiedAt":null}));
    Some(
        json!({"provider":id,"status":view["status"],"expiresAt":view["expiresAt"],"verifiedAt":view["verifiedAt"],"downloadEnabled":status.is_some_and(usable),"entitlement":"not-checked"}),
    )
}
pub(super) async fn require_authorization(manager: &JobManager, provider: &str) -> Result<()> {
    if authorization(manager, provider)
        .await
        .is_some_and(|v| v["downloadEnabled"] != true)
    {
        return Err(
            "Connect this data source in Settings before downloading protected files.".into(),
        );
    }
    Ok(())
}
pub(super) fn format(provider: &str) -> &'static str {
    match provider {
        "copernicus" => "SAFE ZIP",
        "nasa-srtm" => "HGT ZIP",
        "nasa-viirs-suomi" | "nasa-viirs-noaa20" | "nasa-viirs-noaa21" => "HDF5",
        _ => "GeoTIFF",
    }
}
pub(super) fn download_notes(provider: &str) -> Vec<&'static str> {
    if account(provider).is_some() {
        vec!["Protected original product · authorize in Settings · file access is checked by the native worker.",
             "Encoded transfer size is unknown before download. The search area does not crop originals."]
    } else {
        vec!["Downloads retain complete source tiles; the search area does not crop originals."]
    }
}
pub(super) fn validate_pin(
    request: &crate::CreateJobRequest,
    provider: &str,
    pin: Option<&RemotePin>,
) -> Result<()> {
    if account(provider).is_some() {
        if pin.is_some() {
            return Err("Protected originals cannot inherit a public file preflight.".into());
        }
        Ok(())
    } else {
        validate_remote(
            pin.ok_or("Public source file preflight is missing.")?,
            request,
        )
    }
}

// Check every complete original, including ZIP/HDF5, without a TIFF-only path
// or a smaller product limit. Keep the file shared-locked through admission.
pub(super) fn verified_original(root: &Path, job: &Job) -> Result<std::fs::File> {
    use std::io::{Read, Seek, SeekFrom};
    if job.kind != "download" || job.stac_source.is_some() || job.wcs_source.is_some() {
        return Err("Only native original-product tasks can be reused here.".into());
    }
    crate::validate_request(
        &crate::CreateJobRequest {
            item_id: job.item_id.clone(),
            asset_key: job.asset_key.clone(),
            href: job.href.clone(),
            media_type: job.media_type.clone(),
            title: None,
        },
        None,
    )?;
    let path = crate::verified_output_path(root, job)?;
    let mut file = std::fs::File::open(path).map_err(io_error)?;
    fs2::FileExt::try_lock_shared(&file).map_err(|_| "Original product is being modified.")?;
    let metadata = file.metadata().map_err(io_error)?;
    let size = metadata.len();
    if size == 0
        || size > crate::source_transfer_limit(job)
        || size != job.bytes_downloaded
        || job.total_bytes.is_some_and(|v| v != size)
    {
        return Err("Original product size changed or exceeds its native limit.".into());
    }
    let deadline = std::time::Instant::now() + Duration::from_secs(120);
    let mut hasher = Sha256::new();
    let mut count = 0;
    let mut buffer = [0u8; 65536];
    loop {
        if std::time::Instant::now() > deadline {
            return Err("Original product checksum verification timed out.".into());
        }
        let n = file.read(&mut buffer).map_err(io_error)?;
        if n == 0 {
            break;
        }
        count += n as u64;
        if count > size {
            return Err("Original product grew during verification.".into());
        }
        hasher.update(&buffer[..n]);
    }
    if count != size
        || job.sha256.as_deref() != Some(format!("{:x}", hasher.finalize()).as_str())
        || file.metadata().map_err(io_error)?.modified().ok() != metadata.modified().ok()
    {
        return Err("Original product size or SHA-256 changed.".into());
    }
    file.seek(SeekFrom::Start(0)).map_err(io_error)?;
    Ok(file)
}
pub(super) async fn verified_candidates(
    manager: &JobManager,
    files: &[DownloadFile],
) -> Result<Vec<(Job, Result<std::fs::File>)>> {
    let jobs = manager
        .inner
        .store
        .lock()
        .await
        .jobs
        .values()
        .filter(|j| {
            j.status == JobStatus::Succeeded
                && files.iter().any(|f| {
                    f.request.item_id == j.item_id
                        && f.request.asset_key == j.asset_key
                        && f.request.href == j.href
                })
        })
        .cloned()
        .collect::<Vec<_>>();
    let mut checked = Vec::new();
    for job in jobs {
        let root = manager.inner.root.clone();
        let copy = job.clone();
        let result = tokio::task::spawn_blocking(move || verified_original(&root, &copy))
            .await
            .map_err(io_error)?;
        checked.push((job, result));
    }
    Ok(checked)
}
pub(super) fn same_receipt(left: &Job, right: &Job) -> bool {
    left.id == right.id
        && left.status == right.status
        && left.href == right.href
        && left.asset_key == right.asset_key
        && left.item_id == right.item_id
        && left.media_type == right.media_type
        && left.sha256 == right.sha256
        && left.output_path == right.output_path
        && left.bytes_downloaded == right.bytes_downloaded
        && left.total_bytes == right.total_bytes
        && left.updated_at == right.updated_at
}

pub(super) fn normalize(value: &Value, query: &SearchQuery) -> Result<Vec<Candidate>> {
    let source = catalog::source(&query.provider)?;
    if value["type"] != "FeatureCollection" {
        return Err("Catalog response is not a STAC FeatureCollection.".into());
    }
    let items = value["features"]
        .as_array()
        .ok_or("Catalog scene list is missing.")?;
    if items.len() > query.limit {
        return Err("Catalog returned too many scenes.".into());
    }
    let mut ids = BTreeSet::new();
    let mut result = Vec::new();
    for item in items {
        let id = item["id"].as_str().ok_or("Catalog scene ID is missing.")?;
        let bounds: [f64; 4] = serde_json::from_value(item["bbox"].clone())
            .map_err(|_| "Catalog bounds are invalid.")?;
        if item["collection"] != source.collection
            || !ids.insert(id.to_owned())
            || !valid_bounds(bounds)
            || bounds[0] >= query.bounds[2]
            || bounds[2] <= query.bounds[0]
            || bounds[1] >= query.bounds[3]
            || bounds[3] <= query.bounds[1]
        {
            return Err("Catalog record does not match the requested collection or area.".into());
        }
        let viirs = p::viirs::identity(id);
        let date = item["properties"]["datetime"]
            .as_str()
            .or_else(|| item["properties"]["start_datetime"].as_str())
            .ok_or("Catalog date is missing.")?;
        let observed =
            DateTime::parse_from_rfc3339(date).map_err(|_| "Catalog date is invalid.")?;
        let (date, end_date, date_role, crs) = if let Some(info) = &viirs {
            if info.collection != source.collection || !p::viirs::period_matches(item, id) {
                return Err(
                    "Catalog composite period differs from the original VIIRS product.".into(),
                );
            }
            (
                format!("{}T00:00:00Z", info.start),
                Some(format!("{}T23:59:59Z", info.end)),
                Some("composite period".into()),
                Some("VIIRS:Sinusoidal".into()),
            )
        } else if query.provider == "nasa-srtm" {
            let [lon, lat] = p::srtm::cell(id).ok_or("Unsupported SRTM geocell.")?;
            // CMR envelopes may extend by one arc-second around the native
            // one-degree geocell (e.g. N37W123: -123.0002778..-121.9997222).
            // Retain that original envelope; it is not the HGT sample grid.
            let cell_bounds = [
                f64::from(lon),
                f64::from(lat),
                f64::from(lon + 1),
                f64::from(lat + 1),
            ];
            if bounds
                .iter()
                .zip(cell_bounds)
                .any(|(actual, expected)| (actual - expected).abs() > 1.0 / 3600.0 + 1e-7)
            {
                return Err("SRTM catalog bounds differ from their geocell identity.".into());
            }
            (
                date.into(),
                None,
                Some("reference; not acquisition".into()),
                Some("EPSG:4326".into()),
            )
        } else {
            (
                date.into(),
                None,
                None,
                item["properties"]["proj:code"].as_str().map(String::from),
            )
        };
        let cloud = if source.clouds {
            Some(
                item["properties"]["eo:cloud_cover"]
                    .as_f64()
                    .ok_or("Catalog cloud cover is missing.")?,
            )
        } else {
            None
        };
        if cloud.is_some_and(|v| !v.is_finite() || !(0.0..=100.0).contains(&v)) {
            return Err("Catalog cloud cover is invalid.".into());
        }
        let start = if let Some(info) = &viirs {
            info.start.to_string()
        } else {
            observed.date_naive().to_string()
        };
        let end = viirs
            .as_ref()
            .map_or_else(|| start.clone(), |v| v.end.to_string());
        if source.temporal && (end < query.start || start > query.end) {
            return Err("Catalog record does not match the requested dates.".into());
        }
        // CMR implementations may ignore the optional STAC query extension.
        // Apply the requested cloud limit to the bounded page without inventing
        // a zero-result server response or marking later pages complete.
        if cloud.is_some_and(|v| v > query.cloud_max) {
            continue;
        }
        let mut assets = Vec::new();
        let mut bands = BTreeMap::new();
        for key in &source.keys {
            if query.provider == "copernicus" {
                continue;
            }
            let original = if *key == "viirs" {
                viirs
                    .as_ref()
                    .ok_or("Unsupported VIIRS product.")?
                    .production
                    .as_str()
            } else if *key == "srtm" {
                "hgt"
            } else {
                p::nasa::band(key).ok_or("Unsupported HLS band.")?
            };
            let href = item["assets"][original]["href"]
                .as_str()
                .ok_or("Catalog original asset is missing.")?;
            let media = match *key {
                "viirs" => "application/x-hdf5",
                "srtm" => "application/zip",
                _ => "image/tiff; application=geotiff",
            };
            let request = crate::CreateJobRequest {
                item_id: id.into(),
                asset_key: (*key).into(),
                href: href.into(),
                media_type: media.into(),
                title: Some(format!("{id} · {}", key.to_uppercase())),
            };
            catalog::validate_source(&request, source.id)?;
            if matches!(*key, "red" | "green" | "blue") {
                bands.insert(
                    (*key).into(),
                    crate::projects::ReflectanceBand {
                        data_type: "int16".into(),
                        scale: 0.0001,
                        offset: 0.0,
                        nodata: -9999.0,
                        spatial_resolution: 30.0,
                    },
                );
            }
            assets.push(request);
        }
        if query.provider == "copernicus" && !p::copernicus::valid_item(id) {
            return Err("Unsupported Sentinel-2 SAFE identity.".into());
        }
        result.push(Candidate {
            footprint: footprint::read_geometry(item),
            item_id: id.into(),
            date,
            cloud,
            bounds,
            crs,
            assets,
            bands,
            end_date,
            date_role,
        });
    }
    Ok(result)
}

pub(super) async fn resolve_copernicus_candidates(
    manager: &JobManager,
    candidates: &mut [Candidate],
) -> Result<()> {
    if candidates.is_empty() {
        return Ok(());
    }
    let products = manager
        .resolve_copernicus_products(p::copernicus::ResolveProductsRequest {
            item_ids: candidates.iter().map(|c| c.item_id.clone()).collect(),
        })
        .await?;
    for candidate in candidates {
        let product = products
            .iter()
            .find(|p| p.item_id == candidate.item_id)
            .ok_or("Copernicus original product is missing.")?;
        let request = crate::CreateJobRequest {
            item_id: product.item_id.clone(),
            asset_key: "product".into(),
            href: product.href.clone(),
            media_type: product.media_type.into(),
            title: Some(format!("{} · SAFE", product.item_id)),
        };
        catalog::validate_source(&request, "copernicus")?;
        candidate.assets = vec![request];
    }
    Ok(())
}

pub(super) async fn recheck(
    manager: &JobManager,
    provider: &str,
    files: &[DownloadFile],
) -> Result<()> {
    if provider == "copernicus" {
        let ids = files
            .iter()
            .map(|f| f.request.item_id.clone())
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect();
        let products = manager
            .resolve_copernicus_products(p::copernicus::ResolveProductsRequest { item_ids: ids })
            .await?;
        if files.iter().any(|f| {
            !products.iter().any(|p| {
                p.item_id == f.request.item_id
                    && p.href == f.request.href
                    && p.media_type == f.request.media_type
            })
        }) {
            return Err("Copernicus original product changed. Search and review again.".into());
        }
        return Ok(());
    }
    let source = catalog::source(provider)?;
    let settings = manager.proxy_settings().await;
    let mut groups: BTreeMap<&str, Vec<&DownloadFile>> = BTreeMap::new();
    for file in files {
        groups.entry(&file.request.item_id).or_default().push(file);
    }
    for (id, files) in groups {
        let url = url::Url::parse(&format!(
            "https://cmr.earthdata.nasa.gov/stac/LPCLOUD/collections/{}/items/{id}",
            source.collection
        ))
        .map_err(|_| "Invalid NASA catalog reference.")?;
        let client =
            crate::features::client_with_timeout(&url, &settings, Duration::from_secs(25)).await?;
        let response = client
            .get(url)
            .send()
            .await
            .map_err(|_| "NASA catalog recheck failed.")?;
        if !response.status().is_success() {
            return Err("NASA catalog recheck failed.".into());
        }
        let mut body = Vec::new();
        let mut stream = response.bytes_stream();
        while let Some(chunk) = stream.next().await {
            let chunk = chunk.map_err(|_| "NASA metadata recheck was interrupted.")?;
            if body.len() + chunk.len() > MAX_DOCUMENT {
                return Err("NASA metadata exceeds 5 MiB.".into());
            }
            body.extend_from_slice(&chunk);
        }
        let item: Value =
            serde_json::from_slice(&body).map_err(|_| "NASA returned invalid metadata.")?;
        if item["id"] != id || item["collection"] != source.collection {
            return Err("NASA catalog identity changed.".into());
        }
        for file in files {
            let key = match file.request.asset_key.as_str() {
                "viirs" => {
                    p::viirs::identity(id)
                        .ok_or("Invalid VIIRS identity.")?
                        .production
                }
                "srtm" => "hgt".into(),
                k => p::nasa::band(k).ok_or("Invalid HLS band.")?.into(),
            };
            if item["assets"][&key]["href"] != file.request.href
                || file.request.asset_key == "viirs" && !p::viirs::period_matches(&item, id)
            {
                return Err("NASA original asset changed. Search and review again.".into());
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests;
