//! Reviewed GSCloud public terrain originals. Other products retain their separate
//! reservation, format and entitlement requirements. No URL supplied by a client.
use super::DetailRequest;
use crate::{CreateJobRequest, Job, JobManager, JobStatus, Result};
use std::{io::Read, path::Path};
use tokio_util::sync::CancellationToken;

pub(crate) const KEY: &str = "gscloud_srtm90";
#[derive(Clone, Copy)]
struct Product {
    id: &'static str,
    key: &'static str,
    prefix: &'static str,
    title: &'static str,
    source: &'static str,
    max_bytes: u64,
}
const PRODUCTS: [Product; 6] = [
    Product {
        id: "305",
        key: KEY,
        prefix: "srtm",
        title: "SRTMDEM90m",
        source: "Geospatial Data Cloud · SRTMDEM90m original ERDAS IMG ZIP",
        max_bytes: 64 * 1024 * 1024,
    },
    Product {
        id: "307",
        key: "gscloud_slope90",
        prefix: "slope",
        title: "SRTMSLOPE90m",
        source: "Geospatial Data Cloud · SRTMSLOPE90m original ZIP",
        max_bytes: 128 * 1024 * 1024,
    },
    Product {
        id: "308",
        key: "gscloud_tpi90",
        prefix: "tpi",
        title: "SRTMTPI90m",
        source: "Geospatial Data Cloud · SRTMTPI90m original ZIP",
        max_bytes: 64 * 1024 * 1024,
    },
    Product {
        id: "310",
        key: "gscloud_aster30v1",
        prefix: "ASTGTM_",
        title: "ASTER GDEM V1 30m",
        source: "Geospatial Data Cloud · ASTER GDEM V1 30m original ZIP",
        max_bytes: 64 * 1024 * 1024,
    },
    Product {
        id: "421",
        key: "gscloud_aster30v2",
        prefix: "ASTGTM2_",
        title: "ASTER GDEM V2 30m",
        source: "Geospatial Data Cloud · ASTER GDEM V2 30m original ZIP",
        max_bytes: 64 * 1024 * 1024,
    },
    Product {
        id: "aeab8000652a45b38afbb7ff023ddabb",
        key: "gscloud_aster30v3",
        prefix: "ASTGTMV003_",
        title: "ASTER GDEM V3 30m",
        source: "Geospatial Data Cloud · ASTER GDEM V3 30m original ZIP",
        max_bytes: 64 * 1024 * 1024,
    },
];
pub(crate) fn is_key(key: &str) -> bool {
    PRODUCTS.iter().any(|p| p.key == key)
}
pub(crate) fn desktop_product_ids() -> Vec<&'static str> {
    PRODUCTS.iter().map(|p| p.id).collect()
}
pub(crate) fn download_access(product_id: &str) -> serde_json::Value {
    serde_json::json!({"agentDownload":product(product_id).is_ok(),"desktopOriginalDownload":product(product_id).is_ok(),"requiresOwnAccount":true,
        "desktopPath":"Explore > Geospatial Data Cloud > Connect GSCloud account > select product/results > Download selected originals",
        "archiveRasterPreview":false,"note":"Six reviewed original ZIP products support geod_gscloud_download_plan with a native searchId and confirmation. Use archive goal/output, geod_gscloud_archive_inspect and geod_workspace_open kind archive for results. Other products remain metadata-only. No reservation, purchase or raster processing."})
}
pub(crate) fn pending_job(product_id: &str, id: &str) -> Result<Job> {
    let product = product(product_id)?;
    if cell_for(product, id).is_none() {
        return Err("Invalid GSCloud original tile for this product.".into());
    }
    let mut job = crate::new_download_job(CreateJobRequest {
        item_id: id.into(),
        asset_key: product.key.into(),
        href: href(product, id),
        media_type: "application/zip".into(),
        title: Some(format!("GSCloud {} · {id}", product.title)),
    });
    job.source = product.source.into();
    job.validation="Pending whole ZIP, selected product/tile, member format signatures, CRC and SHA-256. Original archive raster preview is not supported.".into();
    validate_job(&job)?;
    Ok(job)
}
fn product(id: &str) -> Result<&'static Product> {
    PRODUCTS
        .iter()
        .find(|p| p.id == id)
        .ok_or_else(|| "This GSCloud product has no verified original download adapter.".into())
}
fn job_product(job: &Job) -> Result<&'static Product> {
    PRODUCTS
        .iter()
        .find(|p| p.key == job.asset_key)
        .ok_or_else(|| "Invalid GSCloud original product.".into())
}
pub(crate) fn transfer_limit(job: &Job) -> Result<u64> {
    validate_job(job)?;
    Ok(job_product(job)?.max_bytes)
}

fn cell_for(product: &Product, id: &str) -> Option<[f64; 4]> {
    if product.prefix.starts_with("ASTGTM") {
        let tile = id.strip_prefix(product.prefix)?.as_bytes();
        if tile.len() != 7
            || !b"NS".contains(&tile[0])
            || !b"EW".contains(&tile[3])
            || !tile[1..3]
                .iter()
                .chain(tile[4..7].iter())
                .all(u8::is_ascii_digit)
        {
            return None;
        }
        let lat = std::str::from_utf8(&tile[1..3]).ok()?.parse::<i32>().ok()?
            * if tile[0] == b'S' { -1 } else { 1 };
        let lon = std::str::from_utf8(&tile[4..7]).ok()?.parse::<i32>().ok()?
            * if tile[3] == b'W' { -1 } else { 1 };
        if !(-90..90).contains(&lat)
            || !(-180..180).contains(&lon)
            || (tile[0] == b'S' && lat == 0)
            || (tile[3] == b'W' && lon == 0)
        {
            return None;
        }
        let pad = 1.0 / 7200.0;
        return Some([
            lon as f64 - pad,
            lat as f64 - pad,
            lon as f64 + 1.0 + pad,
            lat as f64 + 1.0 + pad,
        ]);
    }
    let p = id
        .strip_prefix(product.prefix)?
        .strip_prefix('_')?
        .split('_')
        .collect::<Vec<_>>();
    if p.len() != 2
        || p.iter()
            .any(|s| s.len() != 2 || !s.bytes().all(|b| b.is_ascii_digit()))
    {
        return None;
    }
    let x = p[0].parse::<u32>().ok()?;
    let y = p[1].parse::<u32>().ok()?;
    if !(1..=72).contains(&x) || !(1..=24).contains(&y) {
        return None;
    }
    let west = -180.0 + (x - 1) as f64 * 5.0;
    let north = 60.0 - (y - 1) as f64 * 5.0;
    Some([west, north - 5.0, west + 5.0, north])
}
fn href(product: &Product, id: &str) -> String {
    format!(
        "https://bjdl.gscloud.cn/sources/download/{}/{id}",
        product.id
    )
}
pub(crate) fn validate_job(job: &Job) -> Result<()> {
    let product = job_product(job)?;
    if job.kind != "download"
        || cell_for(product, &job.item_id).is_none()
        || job.href != href(product, &job.item_id)
        || job.media_type != "application/zip"
        || job.source != product.source
        || job.stac_source.is_some()
        || job.wcs_source.is_some()
    {
        return Err("GSCloud original job differs from its reviewed source identity.".into());
    }
    Ok(())
}
pub(crate) async fn response(
    manager: &JobManager,
    client: &reqwest::Client,
    job: &Job,
) -> Result<reqwest::Response> {
    validate_job(job)?;
    #[cfg(test)]
    if let Some(origin) = manager.inner.fixture_origin.as_deref() {
        return client
            .get(format!("{origin}/gscloud-original"))
            .send()
            .await
            .map_err(|_| "GSCloud fixture transfer failed.".into());
    }
    let response = super::auth::original_response(
        &manager.inner.root,
        client,
        job_product(job)?.id,
        &job.item_id,
    )
    .await?;
    match response.status().as_u16() {
        200 => {}
        401 | 403 => {
            return Err(
                "GSCloud session expired or delivery access was denied. Sign in again.".into(),
            )
        }
        _ => {
            return Err(format!(
                "GSCloud original returned HTTP {}. No redirect or reservation was submitted.",
                response.status().as_u16()
            ))
        }
    }
    let media = response
        .headers()
        .get("Content-Type")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("")
        .split(';')
        .next()
        .unwrap_or("");
    if ![
        "application/octet-stream",
        "application/zip",
        "application/x-zip-compressed",
    ]
    .contains(&media)
        || !response
            .content_length()
            .is_some_and(|n| n > 0 && n <= job_product(job).map(|p| p.max_bytes).unwrap_or(0))
    {
        return Err("GSCloud did not return a bounded complete original ZIP file.".into());
    }
    Ok(response)
}

#[derive(Clone, Copy)]
enum MemberFormat {
    Img,
    Tiff,
    Pdf,
}
fn members(product: &Product, id: &str) -> Vec<(String, MemberFormat)> {
    match product.id {
        "421" => vec![
            (format!("{id}_dem.tif"), MemberFormat::Tiff),
            (format!("{id}_num.tif"), MemberFormat::Tiff),
            ("README.pdf".into(), MemberFormat::Pdf),
        ],
        "aeab8000652a45b38afbb7ff023ddabb" => vec![
            (format!("{id}_dem.tif"), MemberFormat::Tiff),
            (format!("{id}_num.tif"), MemberFormat::Tiff),
        ],
        "310" => vec![(format!("{id}N.img"), MemberFormat::Img)],
        _ => vec![(format!("{id}.img"), MemberFormat::Img)],
    }
}
pub(crate) fn verify(path: &Path, job: &Job, cancel: &CancellationToken) -> Result<()> {
    validate_job(job)?;
    let mut zip = zip::ZipArchive::new(std::fs::File::open(path).map_err(crate::io_error)?)
        .map_err(|_| "GSCloud original is not a complete ZIP.")?;
    let expected = members(job_product(job)?, &job.item_id);
    if zip.len() != expected.len() {
        return Err("GSCloud original ZIP has an unexpected member set.".into());
    }
    let mut seen = std::collections::BTreeSet::new();
    for index in 0..zip.len() {
        let mut member = zip
            .by_index(index)
            .map_err(|_| "Invalid GSCloud ZIP member.")?;
        let format = expected
            .iter()
            .find(|(name, _)| name == member.name())
            .map(|(_, format)| *format)
            .ok_or("GSCloud original ZIP member differs from its selected tile.")?;
        if !seen.insert(member.name().to_owned())
            || member.enclosed_name().is_none()
            || member.is_dir()
            || member.unix_mode().is_some_and(|m| m & 0o170000 == 0o120000)
            || member.size() == 0
            || member.size() > 256 * 1024 * 1024
        {
            return Err("GSCloud original ZIP member differs from its selected tile.".into());
        }
        let mut magic = [0u8; 16];
        member
            .read_exact(&mut magic)
            .map_err(|_| "Truncated GSCloud original member.")?;
        let valid = match format {
            MemberFormat::Img => magic.starts_with(b"EHFA_HEADER_TAG\0"),
            MemberFormat::Tiff => magic.starts_with(b"II*\0") || magic.starts_with(b"MM\0*"),
            MemberFormat::Pdf => magic.starts_with(b"%PDF-"),
        };
        if !valid {
            return Err("GSCloud member format differs from its selected original product.".into());
        }
        let mut count = 16u64;
        let mut buffer = [0u8; 65536];
        loop {
            if cancel.is_cancelled() {
                return Err("Transfer cancelled".into());
            }
            let n = member
                .read(&mut buffer)
                .map_err(|_| "GSCloud original ZIP CRC or member bytes are invalid.")?;
            if n == 0 {
                break;
            }
            count += n as u64;
            if count > 256 * 1024 * 1024 {
                return Err("GSCloud original member exceeds the archive budget.".into());
            }
        }
        if count != member.size() {
            return Err("GSCloud original ZIP member is truncated.".into());
        }
    }
    Ok(())
}

impl JobManager {
    pub(crate) async fn gscloud_require_account(&self) -> Result<()> {
        #[cfg(test)]
        if self.inner.fixture_origin.is_some() {
            return Ok(());
        }
        if self.gscloud_auth_status().await?["status"] != "saved" {
            return Err(
                "Connect your GSCloud account in Explore before confirming the download.".into(),
            );
        }
        Ok(())
    }
    pub(crate) async fn gscloud_validate_original(&self, request: DetailRequest) -> Result<()> {
        let product = product(&request.product_id)?;
        if cell_for(product, &request.item_id).is_none() {
            return Err("Invalid GSCloud original tile for this product.".into());
        }
        let detail = self.gscloud_detail(request.clone()).await?;
        if detail["item"]["presence"] != "available" {
            return Err(
                "This GSCloud original requires reservation; no order was submitted.".into(),
            );
        }
        let bounds: [f64; 4] = serde_json::from_value(detail["item"]["bbox"].clone())
            .map_err(|_| "GSCloud original has no published footprint.")?;
        if bounds
            .iter()
            .zip(cell_for(product, &request.item_id).unwrap())
            .any(|(a, b)| (*a - b).abs() > 2e-8)
        {
            return Err(
                "GSCloud original footprint differs from the selected terrain tile.".into(),
            );
        }
        Ok(())
    }
    pub async fn gscloud_download_original(&self, request: DetailRequest) -> Result<Job> {
        self.gscloud_validate_original(request.clone()).await?;
        self.gscloud_queue_original(product(&request.product_id)?, &request.item_id)
            .await
    }
    pub async fn gscloud_inspect_original(&self, id: &str) -> Result<serde_json::Value> {
        let job = self
            .get(id)
            .await
            .ok_or("GSCloud original task was not found.")?;
        validate_job(&job)?;
        if job.status != JobStatus::Succeeded
            || self.inner.store.lock().await.active.contains_key(id)
        {
            return Err("GSCloud original download has not finished.".into());
        }
        let root = self.inner.root.clone();
        tokio::task::spawn_blocking(move || {
            use sha2::{Digest,Sha256};
            let path = crate::verified_output_path(&root,&job)?;
            let mut file = std::fs::File::open(&path).map_err(crate::io_error)?;
            fs2::FileExt::try_lock_shared(&file).map_err(|_|"GSCloud archive is being modified.")?;
            let before=file.metadata().map_err(crate::io_error)?;
            let size=before.len();
            if size==0 || size>transfer_limit(&job)? || size!=job.bytes_downloaded || job.total_bytes!=Some(size) { return Err("GSCloud archive size changed.".into()); }
            let mut hash=Sha256::new();let mut buffer=[0u8;65536];let mut count=0u64;
            while count<=size { let n=file.read(&mut buffer).map_err(crate::io_error)?; if n==0 {break;} count+=n as u64;hash.update(&buffer[..n]); }
            let sha=format!("{:x}",hash.finalize());
            if count!=size || job.sha256.as_deref()!=Some(sha.as_str()) {return Err("GSCloud archive SHA-256 changed.".into());}
            verify(&path,&job,&CancellationToken::new())?;
            let mut zip=zip::ZipArchive::new(file).map_err(|_|"Invalid GSCloud original ZIP.")?;
            let mut files=Vec::new();
            for index in 0..zip.len() {let member=zip.by_index(index).map_err(|_|"Invalid GSCloud original member.")?;files.push(serde_json::json!({"name":member.name(),"bytes":member.size(),"crc32":format!("{:08x}",member.crc32())}));}
            let after=std::fs::metadata(&path).map_err(crate::io_error)?;
            if before.modified().ok()!=after.modified().ok() || before.len()!=after.len() {return Err("GSCloud archive changed during inspection.".into());}
            Ok(serde_json::json!({"kind":"archive","jobId":job.id,"itemId":job.item_id,"productId":job_product(&job)?.id,"title":job.title,"source":job.source,"bytes":size,"sha256":sha,"verified":true,"format":"Original ZIP","members":files,"rasterPreview":false}))
        }).await.map_err(crate::io_error)?
    }
    async fn gscloud_queue_original(&self, product: &Product, id: &str) -> Result<Job> {
        if cell_for(product, id).is_none() {
            return Err("Invalid GSCloud terrain tile.".into());
        }
        let mut store = self.inner.store.lock().await;
        store.accepting_jobs()?;
        if let Some(job) = store.jobs.values().find(|j| {
            j.asset_key == product.key
                && j.item_id == id
                && matches!(
                    j.status,
                    JobStatus::Queued | JobStatus::Running | JobStatus::Succeeded
                )
        }) {
            validate_job(job)?;
            return Ok(job.clone());
        }
        if store.active.len() >= 64 {
            return Err("The local queue is full (64 jobs)".into());
        }
        let mut job = crate::new_download_job(CreateJobRequest {
            item_id: id.into(),
            asset_key: product.key.into(),
            href: href(product, id),
            media_type: "application/zip".into(),
            title: Some(format!("GSCloud {} · {id}", product.title)),
        });
        job.source = product.source.into();
        job.validation="Pending whole ZIP, selected product/tile, member format signatures, CRC and SHA-256. Original archive raster preview is not supported.".into();
        validate_job(&job)?;
        store.jobs.insert(job.id.clone(), job.clone());
        if let Err(err) = self.persist(&store.jobs).await {
            store.jobs.remove(&job.id);
            return Err(err);
        }
        let token = CancellationToken::new();
        store.active.insert(job.id.clone(), token.clone());
        self.spawn(job.id.clone(), token);
        Ok(job)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn job() -> Job {
        let mut j = crate::new_download_job(CreateJobRequest {
            item_id: "srtm_60_04".into(),
            asset_key: KEY.into(),
            href: href(&PRODUCTS[0], "srtm_60_04"),
            media_type: "application/zip".into(),
            title: None,
        });
        j.source = PRODUCTS[0].source.into();
        j
    }
    fn archive(name: &str, valid: bool) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        zip.start_file(
            name,
            zip::write::SimpleFileOptions::default()
                .compression_method(zip::CompressionMethod::Stored),
        )
        .unwrap();
        zip.write_all(if valid {
            b"EHFA_HEADER_TAG\0\x14fixture-header-only"
        } else {
            b"<html>not an IMG file</html>"
        })
        .unwrap();
        zip.finish().unwrap().into_inner()
    }
    fn public_job(p: &Product, id: &str) -> Job {
        let mut j = crate::new_download_job(CreateJobRequest {
            item_id: id.into(),
            asset_key: p.key.into(),
            href: href(p, id),
            media_type: "application/zip".into(),
            title: None,
        });
        j.source = p.source.into();
        j
    }
    fn public_archive(p: &Product, id: &str, corrupt_name: bool) -> Vec<u8> {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
        for (name, format) in members(p, id) {
            zip.start_file(
                if corrupt_name {
                    format!("../{name}")
                } else {
                    name
                },
                zip::write::SimpleFileOptions::default(),
            )
            .unwrap();
            zip.write_all(match format {
                MemberFormat::Img => b"EHFA_HEADER_TAG\0 fixture-only",
                MemberFormat::Tiff => b"II*\0 fixture-only-not-a-raster",
                MemberFormat::Pdf => b"%PDF-1.7 fixture-only",
            })
            .unwrap();
        }
        zip.finish().unwrap().into_inner()
    }
    #[test]
    fn public_product_identity_grid_member_set_and_transfer_caps() {
        let temp = tempfile::tempdir().unwrap();
        let path = temp.path().join("source.zip");
        let token = CancellationToken::new();
        for (pid, id) in [
            ("305", "srtm_60_04"),
            ("307", "slope_60_04"),
            ("308", "tpi_60_04"),
            ("310", "ASTGTM_N40E116"),
            ("421", "ASTGTM2_N40E116"),
            ("aeab8000652a45b38afbb7ff023ddabb", "ASTGTMV003_N40E116"),
        ] {
            let p = product(pid).unwrap();
            let j = public_job(p, id);
            validate_job(&j).unwrap();
            assert_eq!(transfer_limit(&j).unwrap(), p.max_bytes);
            assert!(!crate::supports_partial_transfer(&j));
            assert!(crate::validate_request(
                &CreateJobRequest {
                    item_id: id.into(),
                    asset_key: p.key.into(),
                    href: j.href.clone(),
                    media_type: j.media_type.clone(),
                    title: None
                },
                None
            )
            .is_err());
            std::fs::write(&path, public_archive(p, id, false)).unwrap();
            verify(&path, &j, &token).unwrap();
            std::fs::write(&path, public_archive(p, id, true)).unwrap();
            assert!(verify(&path, &j, &token).is_err());
            let mut changed = j.clone();
            changed.href.push_str("?sid=private");
            assert!(validate_job(&changed).is_err());
            changed = j.clone();
            changed.source = PRODUCTS[0].source.into();
            if pid != "305" {
                assert!(validate_job(&changed).is_err());
            }
        }
        let v2 = product("421").unwrap();
        let v3 = product("aeab8000652a45b38afbb7ff023ddabb").unwrap();
        let pad = 1. / 7200.;
        assert_eq!(
            cell_for(v2, "ASTGTM2_N40E116"),
            Some([116. - pad, 40. - pad, 117. + pad, 41. + pad])
        );
        assert_eq!(
            cell_for(v3, "ASTGTMV003_S01W001"),
            Some([-1. - pad, -1. - pad, pad, pad])
        );
        assert!(cell_for(v2, "ASTGTMV003_N40E116").is_none());
        assert!(cell_for(v2, "ASTGTM2_N90E116").is_none());
        std::fs::write(&path, public_archive(v3, "ASTGTMV003_N40E116", false)).unwrap();
        assert!(verify(&path, &public_job(v2, "ASTGTM2_N40E116"), &token).is_err());
        assert!(product("306").is_err());
        assert!(product("309").is_err());
    }
    async fn settled(manager: &JobManager, id: &str) -> Job {
        for _ in 0..500 {
            let (j, done) = manager.get_with_settled(id).await.unwrap();
            if done {
                return j;
            }
            tokio::time::sleep(std::time::Duration::from_millis(20)).await;
        }
        panic!("GSCloud worker did not settle")
    }
    #[test]
    fn original_identity_and_archive_crc_are_bound_to_selected_tile() {
        assert_eq!(
            cell_for(&PRODUCTS[0], "srtm_60_04"),
            Some([115., 40., 120., 45.])
        );
        assert!(cell_for(&PRODUCTS[0], "srtm_99_00").is_none());
        let mut j = job();
        assert!(validate_job(&j).is_ok());
        j.href.push_str("?sid=private");
        assert!(validate_job(&j).is_err());
        let temp = tempfile::tempdir().unwrap();
        let p = temp.path().join("original.zip");
        let token = CancellationToken::new();
        for (name, valid) in [
            ("other.img", true),
            ("../srtm_60_04.img", true),
            ("srtm_60_04.img", false),
        ] {
            std::fs::write(&p, archive(name, valid)).unwrap();
            assert!(verify(&p, &job(), &token).is_err());
        }
        let mut bytes = archive("srtm_60_04.img", true);
        std::fs::write(&p, &bytes).unwrap();
        verify(&p, &job(), &token).unwrap();
        let marker = bytes
            .windows(15)
            .position(|w| w == b"EHFA_HEADER_TAG")
            .unwrap();
        bytes[marker + 20] ^= 1;
        std::fs::write(&p, bytes).unwrap();
        assert!(verify(&p, &job(), &token).is_err());
        token.cancel();
        assert!(verify(&p, &job(), &token).is_err());
    }
    #[tokio::test]
    async fn original_queue_cancellation_retry_integrity_and_reopen() {
        use axum::{routing::get, Router};
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        let hits = Arc::new(AtomicUsize::new(0));
        let requests = hits.clone();
        let bytes = archive("srtm_60_04.img", true);
        let expected = bytes.clone();
        let server = Router::new().route(
            "/gscloud-original",
            get(move || {
                let hits = requests.clone();
                let bytes = bytes.clone();
                async move {
                    if hits.fetch_add(1, Ordering::SeqCst) == 0 {
                        tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    }
                    bytes
                }
            }),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let origin = format!("http://{}", listener.local_addr().unwrap());
        let handle = tokio::spawn(async move {
            axum::serve(listener, server).await.unwrap();
        });
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open_inner(temp.path(), Some(origin))
            .await
            .unwrap();
        let j = manager
            .gscloud_queue_original(&PRODUCTS[0], "srtm_60_04")
            .await
            .unwrap();
        let duplicate = manager
            .gscloud_queue_original(&PRODUCTS[0], "srtm_60_04")
            .await
            .unwrap();
        assert_eq!(j.id, duplicate.id);
        while hits.load(Ordering::SeqCst) == 0 {
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        manager.cancel(&j.id).await.unwrap();
        assert_eq!(settled(&manager, &j.id).await.status, JobStatus::Cancelled);
        manager.retry(&j.id).await.unwrap();
        let done = settled(&manager, &j.id).await;
        assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
        assert_eq!(
            std::fs::read(done.output_path.as_ref().unwrap()).unwrap(),
            expected
        );
        assert!(!serde_json::to_string(&done).unwrap().contains("?sid="));
        assert!(done.validation.contains("not supported"));
        let inspected = manager.gscloud_inspect_original(&done.id).await.unwrap();
        assert_eq!(inspected["verified"], true);
        assert_eq!(inspected["rasterPreview"], false);
        assert_eq!(inspected["sha256"], done.sha256.clone().unwrap());
        assert_eq!(inspected["members"][0]["name"], "srtm_60_04.img");
        manager.shutdown().await.unwrap();
        drop(manager);
        let reopened = JobManager::open(temp.path()).await.unwrap();
        assert_eq!(reopened.get(&j.id).await.unwrap().sha256, done.sha256);
        reopened.shutdown().await.unwrap();
        handle.abort();
    }
    #[tokio::test]
    #[ignore = "downloads the five reviewed public products using the verified local account and persists real desktop jobs"]
    async fn live_gscloud_public_native_queue() {
        use sha2::{Digest, Sha256};
        let root =
            std::path::PathBuf::from(std::env::var("GEOD_GSCLOUD_VERIFIED_RUNTIME").unwrap());
        let out = std::path::PathBuf::from(std::env::var("GEOD_GSCLOUD_PROBE_DIR").unwrap());
        let reference: serde_json::Value =
            serde_json::from_slice(&std::fs::read(out.join("independent-reference.json")).unwrap())
                .unwrap();
        let manager = JobManager::open(&root).await.unwrap();
        let mut ids = Vec::new();
        for (pid, id) in [
            ("307", "slope_60_04"),
            ("308", "tpi_60_04"),
            ("310", "ASTGTM_N40E116"),
            ("421", "ASTGTM2_N40E116"),
            ("aeab8000652a45b38afbb7ff023ddabb", "ASTGTMV003_N40E116"),
        ] {
            let j = manager
                .gscloud_download_original(DetailRequest {
                    product_id: pid.into(),
                    item_id: id.into(),
                })
                .await
                .unwrap();
            ids.push((pid, j.id));
        }
        let mut complete = Vec::new();
        for (pid, id) in &ids {
            let mut retried = 0;
            let done = loop {
                let (j, settled) = manager.get_with_settled(id).await.unwrap();
                if settled {
                    if j.status == JobStatus::Failed && retried < 2 {
                        println!("Retrying interrupted public original product {pid}; credentials omitted");
                        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
                        manager.retry(id).await.unwrap();
                        retried += 1;
                        continue;
                    }
                    break j;
                }
                tokio::time::sleep(std::time::Duration::from_millis(500)).await;
            };
            assert_eq!(
                done.status,
                JobStatus::Succeeded,
                "Product {pid}: {:?}",
                done.error
            );
            let expected = reference
                .as_array()
                .unwrap()
                .iter()
                .find(|r| r["productId"] == *pid)
                .unwrap();
            let file = std::fs::read(done.output_path.as_ref().unwrap()).unwrap();
            assert_eq!(
                done.bytes_downloaded,
                expected["archiveBytes"].as_u64().unwrap()
            );
            let sha = format!("{:x}", Sha256::digest(&file));
            assert_eq!(Some(sha.as_str()), expected["archiveSha256"].as_str());
            assert_eq!(done.sha256.as_deref(), Some(sha.as_str()));
            assert!(!serde_json::to_string(&done).unwrap().contains("?sid="));
            println!("Native public original complete: product {pid}; {} bytes; independent SHA matches; attempts={}",done.bytes_downloaded,done.attempts);
            complete.push(done);
        }
        manager.shutdown().await.unwrap();
        drop(manager);
        let reopened = JobManager::open(root).await.unwrap();
        for j in &complete {
            let saved = reopened.get(&j.id).await.unwrap();
            assert_eq!(saved.status, JobStatus::Succeeded);
            assert_eq!(saved.sha256, j.sha256);
        }
        reopened.shutdown().await.unwrap();
        std::fs::write(
            out.join("native-original-jobs.json"),
            serde_json::to_vec_pretty(&complete).unwrap(),
        )
        .unwrap();
    }
    #[tokio::test]
    #[ignore = "uses the verified local account and saves one real GSCloud original to its desktop task queue"]
    async fn live_gscloud_srtm90_native_queue() {
        use sha2::{Digest, Sha256};
        let root =
            std::path::PathBuf::from(std::env::var("GEOD_GSCLOUD_VERIFIED_RUNTIME").unwrap());
        let manager = JobManager::open(&root).await.unwrap();
        let job = manager
            .gscloud_download_original(DetailRequest {
                product_id: "305".into(),
                item_id: "srtm_60_04".into(),
            })
            .await
            .unwrap();
        let done = loop {
            let (j, done) = manager.get_with_settled(&job.id).await.unwrap();
            if done {
                break j;
            }
            tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        };
        assert_eq!(done.status, JobStatus::Succeeded, "{:?}", done.error);
        assert_eq!(done.bytes_downloaded, 34890696);
        let bytes = std::fs::read(done.output_path.as_ref().unwrap()).unwrap();
        let sha = format!("{:x}", Sha256::digest(&bytes));
        assert_eq!(
            sha,
            "bc40bbf3beec41921e2aa58b2fb91f37a317a92d3b0b3d95b22734087b91095b"
        );
        assert_eq!(done.sha256.as_deref(), Some(sha.as_str()));
        if let Ok(out) = std::env::var("GEOD_GSCLOUD_PROBE_DIR") {
            std::fs::write(
                std::path::Path::new(&out).join("native-original-job.json"),
                serde_json::to_vec_pretty(&done).unwrap(),
            )
            .unwrap();
        }
        println!("Real GSCloud original task verified: {} bytes, ZIP identity/CRC/SHA-256; credentials absent from job",done.bytes_downloaded);
        let id = done.id.clone();
        manager.shutdown().await.unwrap();
        drop(manager);
        let reopened = JobManager::open(root).await.unwrap();
        assert_eq!(reopened.get(&id).await.unwrap().sha256, done.sha256);
        reopened.shutdown().await.unwrap();
    }
}
