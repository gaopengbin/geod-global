//! Curated dataset discovery archives metadata only. Existing reviewed native
//! STAC/WCS plans remain the authority for saving projects and downloading.
use crate::{datasets, JobManager, Result};
use serde_json::{json, Value};

pub(super) fn definitions() -> Vec<Value> {
    let request = json!({"type":"object","properties":{
        "datasetId":{"type":"string","minLength":1,"maxLength":80},
        "bounds":{"type":"array","items":{"type":"number"},"minItems":4,"maxItems":4},
        "year":{"type":"integer","minimum":1900,"maximum":2100},
        "month":{"type":"integer","minimum":1,"maximum":12},
        "country":{"type":"string","pattern":"^[A-Z]{3}$"},
        "layer":{"type":"string","minLength":1,"maxLength":80},
        "region":{"type":"string","minLength":1,"maxLength":80},
        "season":{"type":"string","minLength":1,"maxLength":80}
    },"required":["datasetId","bounds"],"additionalProperties":false});
    vec![
        json!({"name":"geod_dataset_catalog","description":"Find built-in public products by query (ID, English name or category) in bounded summary pages. For the chosen product call again with datasetId to read its complete year/month/layer/country/region/season options, units and restrictions before resolving it. Follow nextOffset only when needed. No network or transfer.","inputSchema":{"type":"object","properties":{"datasetId":{"type":"string","minLength":1,"maxLength":80},"query":{"type":"string","maxLength":100},"offset":{"type":"integer","minimum":0},"limit":{"type":"integer","minimum":1,"maximum":20}},"additionalProperties":false}}),
        json!({"name":"geod_dataset_resolve","description":"Resolve a built-in dataset for explicit human-requested WGS84 bounds and supported year/month/country/layer/region/season. Use only declared options; omit region to match the actual map against a reviewed regional item index. Seasonal themes preserve their exact AEZ dates rather than assuming a calendar month or a crop-type-only mask. Pure recipe review; no connection, network, project, crop or transfer. Whole files can exceed the area. Read geod_dataset_catalog first.","inputSchema":request.clone()}),
        json!({"name":"geod_dataset_connect","description":"Archive actual metadata for a reviewed dataset recipe. Direct rasters probe TIFF headers; stac-api searches one fixed area/date page; stac-items connects exact standalone publisher documents from a finite regional/season index. Returned snapshots belong only to this request. complete means source requests succeeded, not collection metadata completeness or valid-pixel coverage; pagination is separate. WorldCereal retains raw S3 hrefs plus a separately reviewed public HTTPS transport receipt, actual AEZ seasons and category 254 as valid no-crop data. No project, original-file job or paid computation. URLs/credentials cannot be supplied. Read snapshots and separately review projects/downloads.","inputSchema":request}),
    ]
}

pub(super) async fn call(manager: &JobManager, name: &str, args: Value) -> Result<Value> {
    if name == "geod_dataset_catalog" {
        return catalog(args);
    }
    let request: datasets::ResolveRequest =
        serde_json::from_value(args).map_err(|_| "Invalid dataset connector arguments.")?;
    if name == "geod_dataset_resolve" {
        let value = serde_json::to_value(datasets::resolve(request)?).map_err(crate::io_error)?;
        return Ok(
            json!({"dataset":value["dataset"],"request":value["request"],
            "sources":value["sources"].as_array().unwrap_or(&vec![]).iter().map(source_summary).collect::<Vec<_>>(),
            "warnings":value["warnings"],"downloadsStarted":false}),
        );
    }
    if name != "geod_dataset_connect" {
        return Err("Unknown dataset tool.".into());
    }
    let value =
        serde_json::to_value(manager.connect_dataset(request).await?).map_err(crate::io_error)?;
    let mut output = json!({"dataset":value["plan"]["dataset"],"request":value["plan"]["request"],
        "warnings":value["plan"]["warnings"],"downloadsStarted":false,
        "complete":value["complete"],"failures":value["failures"],
        "count":value["stacConnections"].as_array().map_or(0,Vec::len)+value["wcsConnections"].as_array().map_or(0,Vec::len),
        "connections":value["stacConnections"].as_array().unwrap_or(&vec![]).iter().map(|c|json!({"id":c["id"],"name":c["name"],"kind":c["kind"],"snapshotIds":c["snapshotIds"]})).collect::<Vec<_>>(),
        "coverages":value["wcsConnections"].as_array().unwrap_or(&vec![]).iter().map(|c|json!({"id":c["id"],"name":c["name"],"coverages":c["coverages"]})).collect::<Vec<_>>(),
        "next":"Read geod_stac_assets with {id: snapshotId} (not connectionId or snapshotId as the argument name), unless the returned items already identify the eligible original keys. Prepare geod_stac_project_plan with name, bounds=request.bounds and selections=[{snapshotId,assetKey}] using those exact keys; a dataset layer is not necessarily an asset key. After native project confirmation, use geod_stac_download_plan with projectId from that confirmed project. Never pass connection IDs instead of selections. For a search nextCursor, use geod_stac_search with its unchanged query plus that cursor. File validation and valid pixels require settled native tasks."});
    if let Some(searches) = value["searches"]
        .as_array()
        .filter(|values| !values.is_empty())
    {
        output["searches"] = Value::Array(
            searches
                .iter()
                .map(|search| search_summary(search, &value["plan"]))
                .collect(),
        );
    }
    let standalone_items = value["plan"]["dataset"]["discovery"] == "stac-items";
    if standalone_items
        || value["stacConnections"]
            .as_array()
            .is_some_and(|connections| {
                connections
                    .iter()
                    .any(|connection| connection["kind"] == "raster")
            })
    {
        let mut summaries = Vec::new();
        for connection in value["stacConnections"]
            .as_array()
            .unwrap_or(&vec![])
            .iter()
            .filter(|connection| standalone_items || connection["kind"] == "raster")
            .take(8)
        {
            for id in connection["snapshotIds"]
                .as_array()
                .unwrap_or(&vec![])
                .iter()
                .take(1)
            {
                if let Some(id) = id.as_str() {
                    let item = manager.stac_snapshot(id).await?;
                    summaries.push(json!({"snapshotId":item.id,"connectionId":item.connection_id,"itemId":item.item_id,"collectionId":item.collection_id,"bbox":item.bbox,"startDatetime":item.start_datetime,"endDatetime":item.end_datetime,"region":item.properties["odc:region_code"],"productVersion":item.properties["odc:dataset_version"],"collectionMetadataRetrieved":item.provenance.collection.is_some(),"assets":item.assets.iter().take(8).map(|asset|json!({"key":asset.key,"eligible":asset.eligible,"transportAdapter":asset.transport_adapter})).collect::<Vec<_>>() }));
                }
            }
        }
        output["items"] = Value::Array(summaries);
    }
    Ok(output)
}

fn catalog(args: Value) -> Result<Value> {
    #[derive(serde::Deserialize)]
    #[serde(rename_all = "camelCase", deny_unknown_fields)]
    struct Query {
        dataset_id: Option<String>,
        query: Option<String>,
        #[serde(default)]
        offset: usize,
        limit: Option<usize>,
    }
    let query: Query =
        serde_json::from_value(args).map_err(|_| "Invalid dataset catalog arguments.")?;
    let all = datasets::list();
    if let Some(id) = query.dataset_id {
        let dataset = all
            .into_iter()
            .find(|dataset| dataset.id == id)
            .ok_or("Choose a supported dataset ID.")?;
        return Ok(
            json!({"datasets":[dataset],"totalCount":1,"complete":true,"nextOffset":null,"detail":true,"downloadsStarted":false}),
        );
    }
    let limit = query.limit.unwrap_or(10);
    let text = query.query.unwrap_or_default();
    if !(1..=20).contains(&limit) || query.offset > 10000 || text.len() > 100 {
        return Err("Invalid dataset catalog page.".into());
    }
    let text = text.to_lowercase();
    let filtered = all
        .into_iter()
        .filter(|dataset| {
            text.is_empty()
                || format!("{} {} {}", dataset.id, dataset.title, dataset.category)
                    .to_lowercase()
                    .contains(&text)
        })
        .collect::<Vec<_>>();
    let total = filtered.len();
    let end = query.offset.saturating_add(limit).min(total);
    let page=filtered.iter().skip(query.offset).take(limit).map(|dataset|json!({"id":dataset.id,"title":dataset.title,"category":dataset.category,"discovery":dataset.discovery,"version":dataset.version})).collect::<Vec<_>>();
    Ok(
        json!({"datasets":page,"totalCount":total,"complete":end>=total,"nextOffset":if end<total{Some(end)}else{None},"detail":false,"downloadsStarted":false,"next":"Read the selected datasetId for complete supported options before resolve/connect."}),
    )
}

fn source_summary(source: &Value) -> Value {
    let mut value = json!({"id":source["id"],"title":source["title"],"bounds":source["bounds"]});
    if source.get("search").is_some() {
        value["search"] = source["search"].clone();
    }
    for key in ["region", "season", "startDatetime", "endDatetime"] {
        if source.get(key).is_some() {
            value[key] = source[key].clone();
        }
    }
    value
}

fn search_summary(search: &Value, plan: &Value) -> Value {
    let source = plan["sources"].as_array().and_then(|values| {
        values
            .iter()
            .find(|source| source["id"] == search["sourceId"])
    });
    let page = &search["page"];
    let mut query = source.map_or_else(|| json!({}), |source| source["search"].clone());
    query["connectionId"] = search["connectionId"].clone();
    json!({"sourceId":search["sourceId"],"connectionId":search["connectionId"],"query":query,
        "count":page["items"].as_array().map_or(0,Vec::len),"pageComplete":page["complete"],"limitReached":page["limitReached"],"nextCursor":page["nextCursor"],
        "items":page["items"].as_array().unwrap_or(&vec![]).iter().take(20).map(|item|json!({"snapshotId":item["id"],"itemId":item["itemId"],"collectionId":item["collectionId"],"title":item["title"],"datetime":item["datetime"],"startDatetime":item["startDatetime"],"endDatetime":item["endDatetime"],"bbox":item["bbox"]})).collect::<Vec<_>>()})
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn growing_catalog_remains_bounded_and_all_product_options_are_reachable() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        let session = "a1234567-1234-1234-1234-123456789abc";
        let mut offset = 0;
        let mut ids = std::collections::BTreeSet::new();
        loop {
            let page = crate::agent_actions::call(
                manager.clone(),
                session,
                "geod_dataset_catalog",
                json!({"offset":offset,"limit":7}),
                None,
            )
            .await
            .unwrap();
            assert!(serde_json::to_vec(&page).unwrap().len() < 32768);
            for item in page["datasets"].as_array().unwrap() {
                assert!(ids.insert(item["id"].as_str().unwrap().to_string()));
            }
            if page["nextOffset"].is_null() {
                break;
            }
            offset = page["nextOffset"].as_u64().unwrap();
        }
        assert_eq!(ids.len(), datasets::list().len());
        for id in ids {
            let detail = crate::agent_actions::call(
                manager.clone(),
                session,
                "geod_dataset_catalog",
                json!({"datasetId":id}),
                None,
            )
            .await
            .unwrap();
            assert_eq!(detail["datasets"][0]["id"], id);
            assert!(detail["datasets"][0]["layers"].is_array());
        }
        assert_eq!(
            catalog(json!({"query":"worldpop"})).unwrap()["datasets"][0]["id"],
            "worldpop"
        );
        assert!(catalog(json!({"limit":0})).is_err());
        assert!(catalog(json!({"href":"https://example.test"})).is_err());
        assert!(manager.list().await.is_empty());
    }
    #[test]
    fn dataset_tools_accept_scope_not_transfer_urls() {
        let definitions = definitions();
        assert_eq!(definitions.len(), 3);
        for tool in definitions.iter().skip(1) {
            assert_eq!(tool["inputSchema"]["additionalProperties"], false);
            assert!(tool["inputSchema"]["properties"].get("href").is_none());
            assert_eq!(
                tool["inputSchema"]["required"],
                json!(["datasetId", "bounds"])
            );
            assert_eq!(tool["inputSchema"]["properties"]["month"]["minimum"], 1);
            assert_eq!(tool["inputSchema"]["properties"]["month"]["maximum"], 12);
            assert_eq!(tool["inputSchema"]["properties"]["region"]["maxLength"], 80);
            assert_eq!(tool["inputSchema"]["properties"]["season"]["maxLength"], 80);
        }
        assert!(serde_json::from_value::<datasets::ResolveRequest>(json!({"datasetId":"worldcover","bounds":[0,0,1,1],"href":"https://unreviewed.test/file.tif"})).is_err());
    }

    #[tokio::test]
    async fn regional_resolution_returns_actual_season_without_urls_or_jobs() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        let value=call(&manager,"geod_dataset_resolve",json!({"datasetId":"worldcereal","bounds":[57.5,-20.1,57.6,-20.],"layer":"maize-active","region":"9026","season":"tc-maize-main"})).await.unwrap();
        assert_eq!(value["request"]["season"], "tc-maize-main");
        assert_eq!(value["sources"][0]["region"], "9026");
        assert_eq!(value["sources"][0]["startDatetime"], "2020-11-11T00:00:00Z");
        assert_eq!(value["sources"][0]["endDatetime"], "2021-05-08T23:59:59Z");
        assert!(value["sources"][0].get("url").is_none());
        assert!(value["sources"][0].get("stac").is_none());
        assert_eq!(value["downloadsStarted"], false);
        assert!(manager.list().await.is_empty());
        assert!(manager.list_stac_connections().await.is_empty());
    }

    #[tokio::test]
    async fn monthly_resolution_preserves_period_without_transfer_or_url_arguments() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        let value = call(
            &manager,
            "geod_dataset_resolve",
            json!({"datasetId":"chirps","bounds":[116,39,117,40],"year":2024,"month":2}),
        )
        .await
        .unwrap();
        assert_eq!(value["request"]["month"], 2);
        assert_eq!(value["downloadsStarted"], false);
        assert_eq!(value["sources"].as_array().unwrap().len(), 1);
        assert!(value["sources"][0].get("stac").is_none());
        assert!(value["sources"][0].get("url").is_none());
        assert_eq!(manager.list_stac_connections().await.len(), 0);
        assert_eq!(manager.list().await.len(), 0);
    }

    #[tokio::test]
    async fn curated_catalog_resolution_has_fixed_scopes_and_no_connection_or_job() {
        let temp = tempfile::tempdir().unwrap();
        let manager = JobManager::open(temp.path()).await.unwrap();
        for (id, collection, month) in [
            ("cbers-sr", "CB4A-MUX-L4-SR-1", Some(9)),
            ("io-lulc", "io-10m-annual-lulc", None),
        ] {
            let value = call(
                &manager,
                "geod_dataset_resolve",
                json!({"datasetId":id,"bounds":[-44,-24,-43,-22]}),
            )
            .await
            .unwrap();
            assert_eq!(value["dataset"]["discovery"], "stac-api");
            assert_eq!(value["sources"][0]["search"]["collectionId"], collection);
            assert_eq!(
                value["sources"][0]["search"]["bounds"],
                json!([-44., -24., -43., -22.])
            );
            assert_eq!(value["sources"][0]["search"]["limit"], 20);
            assert!(value["sources"][0].get("url").is_none());
            assert!(value["sources"][0].get("stac").is_none());
            assert!(value["sources"][0]["bounds"].is_null());
            assert_eq!(value["request"]["month"].as_u64(), month);
            assert_eq!(value["downloadsStarted"], false);
        }
        assert_eq!(manager.list_stac_connections().await.len(), 0);
        assert_eq!(manager.list().await.len(), 0);
    }

    #[test]
    fn search_summaries_bound_model_context_and_bind_pagination_to_the_same_query() {
        let plan = serde_json::to_value(
            datasets::resolve(
                serde_json::from_value(json!({"datasetId":"cbers-sr","bounds":[-44,-24,-43,-22]}))
                    .unwrap(),
            )
            .unwrap(),
        )
        .unwrap();
        let item = json!({"id":"current-snapshot","itemId":"publisher-item","collectionId":"CB4A-MUX-L4-SR-1","bbox":[-44,-24,-43,-22],"properties":{"hugePublisherDocument":"must stay in immutable source, not copied into model"},"assets":[{"href":"https://publisher.test/raw.tif"}]});
        let value = search_summary(
            &json!({"sourceId":"CB4A-MUX-L4-SR-1","connectionId":"connection","page":{"items":vec![item; 21],"nextCursor":"opaque-next","complete":false,"limitReached":false}}),
            &plan,
        );
        assert_eq!(value["query"]["connectionId"], "connection");
        assert_eq!(value["query"]["bounds"], json!([-44., -24., -43., -22.]));
        assert_eq!(value["query"]["collectionId"], "CB4A-MUX-L4-SR-1");
        assert_eq!(
            value["query"]["datetime"],
            "2026-09-01T00:00:00Z/2026-09-30T23:59:59.999999Z"
        );
        assert_eq!(value["nextCursor"], "opaque-next");
        assert_eq!(value["pageComplete"], false);
        assert_eq!(value["items"].as_array().unwrap().len(), 20);
        assert!(value["items"][0].get("properties").is_none());
        assert!(value["items"][0].get("assets").is_none());
    }
}
