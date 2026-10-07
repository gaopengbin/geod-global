//! Fixed public provider adapters; catalog results are data, never executable URLs.
use super::*;
use crate::providers as p;
use std::collections::BTreeMap;

pub(super) const IDS: &[&str] = &[
    "earth-search",
    "planetary-computer",
    "planetary-landsat",
    "planetary-radar",
    "planetary-modis",
    "planetary-vegetation",
    "planetary-naip",
    "copernicus-dem",
    "copernicus-dem-90",
    "copernicus",
    "nasa-earthdata",
    "nasa-srtm",
    "nasa-viirs-suomi",
    "nasa-viirs-noaa20",
    "nasa-viirs-noaa21",
];
pub(super) struct Source {
    pub id: &'static str,
    pub name: &'static str,
    pub collection: &'static str,
    pub endpoint: &'static str,
    pub host: &'static str,
    pub temporal: bool,
    pub clouds: bool,
    pub keys: Vec<&'static str>,
}
pub(super) fn source(id: &str) -> Result<Source> {
    let (id, name, collection, host, temporal, clouds, keys) = match id {
        "earth-search" => (IDS[0], "Earth Search · Sentinel-2 L2A", "sentinel-2-l2a", crate::SOURCE_HOST, true, true, vec!["scl","visual"]),
        "planetary-computer" => (IDS[1], "Planetary Computer · Sentinel-2 L2A", "sentinel-2-l2a", p::PC_HOST, true, true, vec!["scl","visual"]),
        "planetary-landsat" => (IDS[2], "Planetary Computer · Landsat 8/9 L2", "landsat-c2-l2", p::LANDSAT_HOST, true, true, vec!["red","green","blue","qa_pixel","qa_radsat"]),
        "planetary-radar" => (IDS[3], "Planetary Computer · Sentinel-1 IW RTC", "sentinel-1-rtc", p::radar::HOST, true, false, p::radar::KEYS.to_vec()),
        "planetary-modis" => (IDS[4], "Planetary Computer · MODIS 8-day reflectance", "modis-09A1-061", p::modis::HOST, true, false, vec!["red","green","blue","modis_qc","modis_state"]),
        "planetary-vegetation" => (IDS[5], "Planetary Computer · MODIS 16-day vegetation", p::vegetation::COLLECTION, p::modis::HOST, true, false, p::vegetation::ASSETS.iter().map(|(k,_)| *k).collect()),
        "planetary-naip" => (IDS[6], "Planetary Computer · NAIP RGB + NIR", "naip", p::NAIP_HOST, true, false, vec!["aerial"]),
        "copernicus-dem" => (IDS[7], "Copernicus DEM · GLO-30 Public", "cop-dem-glo-30", p::DEM_HOST, false, false, vec!["elevation"]),
        "copernicus-dem-90" => (IDS[8], "Copernicus DEM · GLO-90", "cop-dem-glo-90", p::DEM90_HOST, false, false, vec!["elevation"]),
        "copernicus" => (IDS[9], "Copernicus Data Space · Sentinel-2 SAFE", "sentinel-2-l2a", p::copernicus::HOST, true, true, vec!["product"]),
        "nasa-earthdata" => (IDS[10], "NASA Earthdata · HLS L30", "HLSL30_2.0", p::nasa::HOST, true, true, vec!["red","green","blue"]),
        "nasa-srtm" => (IDS[11], "NASA Earthdata · SRTMGL1", p::srtm::COLLECTION, p::nasa::HOST, false, false, vec!["srtm"]),
        "nasa-viirs-suomi" => (IDS[12], "NASA VIIRS · Suomi-NPP", "VNP09A1_002", p::nasa::HOST, true, false, vec!["viirs"]),
        "nasa-viirs-noaa20" => (IDS[13], "NASA VIIRS · NOAA-20", "VJ109A1_002", p::nasa::HOST, true, false, vec!["viirs"]),
        "nasa-viirs-noaa21" => (IDS[14], "NASA VIIRS · NOAA-21", "VJ209A1_002", p::nasa::HOST, true, false, vec!["viirs"]),
        _ => return Err("This source has no reviewed Agent acquisition adapter. Read geod_sources_list for available sources and account setup.".into()),
    };
    Ok(Source {
        id,
        name,
        collection,
        host,
        temporal,
        clouds,
        keys,
        endpoint: if id == "copernicus" {
            "https://stac.dataspace.copernicus.eu/v1/search"
        } else if id.starts_with("nasa-") {
            "https://cmr.earthdata.nasa.gov/stac/LPCLOUD/search"
        } else if id.starts_with("planetary-") {
            "https://planetarycomputer.microsoft.com/api/stac/v1/search"
        } else {
            CATALOG
        },
    })
}
pub(super) fn capabilities(id: &str) -> Result<Value> {
    let s = source(id)?;
    Ok(
        json!({"id":s.id,"name":s.name,"collection":s.collection,"agentSearch":true,
        "accountRequired":protected::account(id).is_some(),"download":s.keys,"dateFilter":s.temporal,"cloudFilter":s.clouds,
        "dateMeaning":if !s.temporal { "reference; not acquisition" } else if s.id == "planetary-modis" || s.id == "planetary-vegetation" || s.keys == ["viirs"] { "composite period" } else { "acquisition" },
        "originalGridProcessing":!matches!(s.keys.as_slice(),["product"]|["viirs"]),"approvalRequired":true,
        "note":"A catalog result is not a preview, downloaded file, or completion receipt."}),
    )
}
pub(super) fn sources() -> Value {
    json!({"sources":IDS.iter().map(|id|capabilities(id).unwrap()).collect::<Vec<_>>(),
        "accountSources":[
            {"id":"copernicus","name":"Copernicus Data Space","accountRequired":true,"agentSearch":true},
            {"id":"nasa-earthdata","name":"NASA Earthdata · HLS / SRTM / VIIRS","accountRequired":true,"agentSearch":true}
        ],"accountSetup":"Use Settings → Data accounts. No password or token is accepted in chat.",
        "verification":"Capabilities describe adapters. Query and verify each actual request; no account entitlement is implied."})
}
pub(super) fn provider(request: &crate::CreateJobRequest) -> Result<&'static str> {
    crate::validate_request(request, None)?;
    let url = crate::validate_asset_url(&request.href)?;
    let host = url.host_str().ok_or("Invalid source host.")?;
    IDS.iter()
        .find_map(|id| {
            let s = source(id).ok()?;
            (host == s.host
                && s.keys.contains(&request.asset_key.as_str())
                && (request.asset_key != "viirs"
                    || p::viirs::identity(&request.item_id)
                        .is_some_and(|v| v.collection == s.collection)))
            .then_some(s.id)
        })
        .ok_or_else(|| "Plan contains a source outside the reviewed public Agent adapters.".into())
}
// Read the already-open native account store. No login, refresh, verification
// request or vault mutation is performed, and no credential is returned.
pub(super) async fn sources_with_accounts(manager: &JobManager) -> Value {
    let mut result = sources();
    let statuses = manager.provider_accounts().await;
    for source in result["accountSources"].as_array_mut().unwrap() {
        let provider = if source["id"] == "nasa-earthdata" {
            crate::AccountProvider::Nasa
        } else {
            crate::AccountProvider::Copernicus
        };
        source["agentDownload"] = json!(true);
        source["downloadEnabled"] = json!(statuses
            .iter()
            .find(|s| s.provider == provider)
            .is_some_and(protected::usable));
        source["entitlement"] = json!("not-checked");
        source["authorization"] = statuses
            .iter()
            .find(|status| status.provider == provider)
            .map(account_view)
            .unwrap_or_else(|| json!({"status":"unavailable","expiresAt":null,"verifiedAt":null}));
    }
    result["checkedAt"] = json!(now());
    result["customSources"] = json!(manager.list_stac_connections().await.iter().map(|c|json!({
        "id":c.id,"name":c.name,"kind":c.kind,"agentRead":true,
        "agentSearch":matches!(c.kind.as_str(),"api"|"catalog"),"agentDownload":true,"approvalRequired":true,
        "catalogTool":"geod_stac_catalog","snapshotTool":"geod_stac_snapshot",
        "setup":"Connect sources in the app. Use archived snapshots to prepare geod_stac_project_plan, confirm the card, then geod_stac_download_plan."
    })).collect::<Vec<_>>());
    result
}
pub(super) fn account_view(value: &crate::AccountStatus) -> Value {
    let timestamp = |value: &Option<String>| {
        value.as_ref().and_then(|value| {
            if value.len() > 64 {
                return None;
            }
            DateTime::parse_from_rfc3339(value).ok().map(|time| {
                time.with_timezone(&Utc)
                    .to_rfc3339_opts(chrono::SecondsFormat::Secs, true)
            })
        })
    };
    let status = match value.status {
        "not-connected" | "saved" | "connected" | "expired" | "storage-error" | "unsupported" => {
            value.status
        }
        _ => "unavailable",
    };
    json!({"status":status,"expiresAt":timestamp(&value.expires_at),"verifiedAt":timestamp(&value.verified_at)})
}

pub(super) fn validate_source(request: &crate::CreateJobRequest, provider_id: &str) -> Result<()> {
    if provider(request)? != provider_id {
        return Err("Source product does not match the reviewed plan provider.".into());
    }
    Ok(())
}
pub(super) fn url(query: &SearchQuery) -> Result<url::Url> {
    let s = source(&query.provider)?;
    let mut url = url::Url::parse(s.endpoint).unwrap();
    url.query_pairs_mut()
        .append_pair("collections", s.collection)
        .append_pair("bbox", &query.bounds.map(|v| v.to_string()).join(","))
        .append_pair("limit", &query.limit.to_string());
    if s.temporal {
        url.query_pairs_mut()
            .append_pair(
                "datetime",
                &format!("{}T00:00:00Z/{}T23:59:59.999Z", query.start, query.end),
            )
            .append_pair(
                "sortby",
                if matches!(s.id, "planetary-modis" | "planetary-vegetation") {
                    "-properties.start_datetime"
                } else {
                    "-properties.datetime"
                },
            );
    }
    let mut filter = serde_json::Map::new();
    if s.clouds {
        filter.insert("eo:cloud_cover".into(), json!({"lte":query.cloud_max}));
    }
    if s.id == "planetary-landsat" {
        filter.insert("platform".into(), json!({"in":["landsat-8","landsat-9"]}));
    }
    if s.id == "planetary-radar" {
        filter.insert("sar:instrument_mode".into(), json!({"eq":"IW"}));
    }
    if !filter.is_empty() {
        url.query_pairs_mut()
            .append_pair("query", &Value::Object(filter).to_string());
    }
    Ok(url)
}
pub(super) fn normalize(value: &Value, query: &SearchQuery) -> Result<Vec<Candidate>> {
    if protected::account(&query.provider).is_some() {
        return protected::normalize(value, query);
    }
    let s = source(&query.provider)?;
    if value["type"] != "FeatureCollection" {
        return Err("Catalog response is not a STAC FeatureCollection.".into());
    }
    let items = value["features"]
        .as_array()
        .ok_or("Catalog scene list is missing.")?;
    if items.len() > query.limit {
        return Err("Catalog returned too many scenes.".into());
    }
    let mut result = Vec::new();
    let mut ids = BTreeSet::new();
    for item in items {
        let id = item["id"].as_str().ok_or("Catalog scene ID is missing.")?;
        let period = if s.id == "planetary-modis" {
            p::modis::period(id)
        } else if s.id == "planetary-vegetation" {
            p::vegetation::period(id)
        } else {
            None
        };
        let date = period
            .as_ref()
            .map(|p| p[0].as_str())
            .or_else(|| item["properties"]["datetime"].as_str())
            .ok_or("Catalog date is missing.")?;
        let observed = DateTime::parse_from_rfc3339(date)
            .map_err(|_| "Catalog date is invalid.")?
            .date_naive()
            .to_string();
        let end = period
            .as_ref()
            .map(|p| p[1][..10].to_string())
            .unwrap_or(observed.clone());
        let cloud = if s.clouds {
            Some(
                item["properties"]["eo:cloud_cover"]
                    .as_f64()
                    .ok_or("Catalog cloud cover is missing.")?,
            )
        } else {
            None
        };
        let bounds: [f64; 4] = serde_json::from_value(item["bbox"].clone())
            .map_err(|_| "Catalog bounds are invalid.")?;
        if item["collection"] != s.collection
            || !valid_bounds(bounds)
            || !ids.insert(id.to_owned())
            || cloud.is_some_and(|v| {
                !v.is_finite() || !(0.0..=100.0).contains(&v) || v > query.cloud_max
            })
            || s.temporal && (end < query.start || observed > query.end)
            || bounds[0] >= query.bounds[2]
            || bounds[2] <= query.bounds[0]
            || bounds[1] >= query.bounds[3]
            || bounds[3] <= query.bounds[1]
        {
            return Err(
                "Catalog record does not match the requested collection, area, dates or clouds."
                    .into(),
            );
        }
        if let Some(period) = &period {
            for (key, expected) in [("start_datetime", &period[0]), ("end_datetime", &period[1])] {
                if item["properties"][key]
                    .as_str()
                    .and_then(|v| DateTime::parse_from_rfc3339(v).ok())
                    != DateTime::parse_from_rfc3339(expected).ok()
                {
                    return Err(
                        "Catalog composite period differs from its original product.".into(),
                    );
                }
            }
        } else if matches!(s.id, "planetary-modis" | "planetary-vegetation") {
            return Err("Unsupported composite product.".into());
        }
        let hrefs: BTreeMap<String, String> = if s.id.starts_with("planetary-") {
            p::reviewed_catalogue(item.clone(), s.collection)?
        } else if !s.temporal {
            let product = p::dem_product(id).ok_or("Unsupported elevation geocell.")?;
            let [lon, lat] = p::dem_cell(id).ok_or("Unsupported elevation geocell.")?;
            // The nominal geocell is a full degree; STAC bounds include half-pixel
            // registration offsets. Check containment without rewriting those bounds.
            if bounds[0] < f64::from(lon) - 0.001
                || bounds[2] > f64::from(lon + 1) + 0.001
                || bounds[1] < f64::from(lat) - 0.001
                || bounds[3] > f64::from(lat + 1) + 0.001
            {
                return Err("Elevation catalog bounds differ from their geocell identity.".into());
            }
            let bucket = if s.id == "copernicus-dem" {
                "copernicus-dem-30m"
            } else {
                "copernicus-dem-90m"
            };
            if product.host() != s.host
                || item["assets"]["data"]["href"] != format!("s3://{bucket}/{id}/{id}.tif")
            {
                return Err("Elevation tile differs from its reviewed product.".into());
            }
            BTreeMap::from([(
                "elevation".into(),
                format!("https://{}/{id}/{id}.tif", s.host),
            )])
        } else {
            s.keys
                .iter()
                .filter_map(|key| {
                    item["assets"][*key]["href"]
                        .as_str()
                        .map(|v| ((*key).to_owned(), v.to_owned()))
                })
                .collect()
        };
        let mut assets = Vec::new();
        let mut bands = BTreeMap::new();
        for (key, href) in hrefs {
            let band = if s.id == "planetary-vegetation" {
                let l = p::vegetation::layer(&key).ok_or("Unsupported vegetation layer.")?;
                Some(crate::projects::ReflectanceBand {
                    data_type: l.data_type.into(),
                    scale: l.scale,
                    offset: 0.0,
                    nodata: f64::from(l.nodata),
                    spatial_resolution: 250.0,
                })
            } else if matches!(key.as_str(), "red" | "green" | "blue") {
                let modis = s.id == "planetary-modis";
                let band = crate::projects::ReflectanceBand {
                    data_type: if modis { "int16" } else { "uint16" }.into(),
                    scale: if modis { 0.0001 } else { 0.0000275 },
                    offset: if modis { 0.0 } else { -0.2 },
                    nodata: if modis { -28672.0 } else { 0.0 },
                    spatial_resolution: if modis { 500.0 } else { 30.0 },
                };
                if !modis {
                    let info = &item["assets"][&key]["raster:bands"][0];
                    if info["data_type"] != band.data_type
                        || info["scale"].as_f64() != Some(band.scale)
                        || info["offset"].as_f64() != Some(band.offset)
                        || info["nodata"].as_f64() != Some(band.nodata)
                        || info["spatial_resolution"].as_f64() != Some(band.spatial_resolution)
                    {
                        return Err("Catalog reflectance calibration is unsupported.".into());
                    }
                }
                Some(band)
            } else {
                None
            };
            if let Some(band) = band {
                bands.insert(key.clone(), band);
            }
            let request = crate::CreateJobRequest {
                item_id: id.into(),
                asset_key: key.clone(),
                href,
                media_type: "image/tiff; application=geotiff".into(),
                title: Some(format!("{id} · {}", key.to_uppercase())),
            };
            validate_source(&request, s.id)?;
            assets.push(request);
        }
        if assets.is_empty() {
            return Err("Catalog scene has no reviewed downloadable asset.".into());
        }
        let crs = if matches!(s.id, "planetary-modis" | "planetary-vegetation") {
            Some(p::modis::CRS.into())
        } else if !s.temporal {
            Some("EPSG:4326".into())
        } else {
            item["properties"]["proj:code"]
                .as_str()
                .map(String::from)
                .or_else(|| {
                    item["properties"]["proj:epsg"]
                        .as_u64()
                        .map(|v| format!("EPSG:{v}"))
                })
        };
        result.push(Candidate {
            footprint: footprint::read_geometry(item),
            item_id: id.into(),
            date: date.into(),
            cloud,
            bounds,
            crs,
            assets,
            bands,
            end_date: period.map(|p| p[1].clone()),
            date_role: if s.temporal {
                None
            } else {
                Some("reference; not acquisition".into())
            },
        });
    }
    Ok(result)
}

pub(super) fn scene_temporal(scene: &crate::projects::ProjectScene) -> bool {
    !scene.assets.values().any(|a| {
        url::Url::parse(&a.href).is_ok_and(|u| {
            matches!(u.host_str(), Some(p::DEM_HOST | p::DEM90_HOST))
                || u.host_str() == Some(p::nasa::HOST) && p::srtm::asset_path(u.path())
        })
    })
}

#[cfg(test)]
mod account_tests {
    use super::*;
    #[test]
    fn account_projection_is_closed_and_retains_only_valid_status_dates() {
        let value = crate::AccountStatus {
            provider: crate::AccountProvider::Nasa,
            status: "saved",
            expires_at: Some("2026-10-06T08:00:00+08:00".into()),
            verified_at: Some("synthetic-secret-in-invalid-timestamp".into()),
        };
        assert_eq!(
            account_view(&value),
            json!({"status":"saved","expiresAt":"2026-10-06T00:00:00Z","verifiedAt":null})
        );
        let unknown = crate::AccountStatus {
            status: "unexpected-private-string",
            ..value
        };
        assert_eq!(account_view(&unknown)["status"], "unavailable");
    }
    #[tokio::test]
    async fn source_status_uses_native_accounts_without_creating_jobs_or_authorization() {
        let directory = tempfile::tempdir().unwrap();
        let manager = JobManager::open(directory.path()).await.unwrap();
        let session = Uuid::new_v4().to_string();
        let result = call(
            manager.clone(),
            &session,
            "geod_sources_list",
            json!({}),
            None,
        )
        .await
        .unwrap();
        assert_eq!(result["sources"].as_array().unwrap().len(), 15);
        assert!(DateTime::parse_from_rfc3339(result["checkedAt"].as_str().unwrap()).is_ok());
        for account in result["accountSources"].as_array().unwrap() {
            assert_eq!(account["authorization"]["status"], "not-connected");
            assert_eq!(account["agentDownload"], true);
            assert_eq!(account["downloadEnabled"], false);
            assert_eq!(account["entitlement"], "not-checked");
            assert_eq!(account["authorization"].as_object().unwrap().len(), 3);
        }
        assert!(manager.list().await.is_empty());
        assert!(manager
            .provider_accounts()
            .await
            .iter()
            .all(|status| status.status == "not-connected"));
        manager.shutdown().await.unwrap();
    }
}
