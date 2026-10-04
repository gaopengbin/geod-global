//! HTTP descriptors for public Item Search. Paging bodies are retained so a
//! selected page can be checked again before an original-file transfer.
use super::*;

const MAX_REQUEST: usize = 16 * 1024;

#[derive(Debug, Default, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "UPPERCASE")]
pub enum SearchMethod {
    #[default]
    Get,
    Post,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MetadataRequest {
    pub url: String,
    pub method: SearchMethod,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

impl MetadataRequest {
    pub(super) fn get(url: &Url) -> Self {
        Self {
            url: url.to_string(),
            method: SearchMethod::Get,
            body: None,
        }
    }

    pub(super) fn validate(&self) -> Result<Url> {
        let url = public_url(&self.url)?;
        if self.method == SearchMethod::Get && self.body.is_some() {
            return Err("STAC GET requests cannot include a body".into());
        }
        if let Some(body) = &self.body {
            if !body.is_object() {
                return Err("STAC POST body must be a JSON object".into());
            }
            check_public_body(body)?;
        }
        if serde_json::to_vec(self).map_err(io_error)?.len() > MAX_REQUEST {
            return Err("STAC search request exceeds 16 KiB".into());
        }
        Ok(url)
    }

    pub(super) fn identity(&self) -> Result<String> {
        self.validate()?;
        Ok(hash(&serde_json::to_vec(self).map_err(io_error)?))
    }
}

pub(super) fn check_public_body(value: &Value) -> Result<()> {
    match value {
        Value::Object(values) => {
            for (key, value) in values {
                let key = key.to_ascii_lowercase();
                if matches!(
                    key.as_str(),
                    "authorization"
                        | "password"
                        | "access_token"
                        | "refresh_token"
                        | "api_key"
                        | "apikey"
                        | "credential"
                        | "credentials"
                        | "cookie"
                        | "set-cookie"
                ) || key.starts_with("x-amz-")
                    || key.starts_with("x-goog-")
                {
                    return Err("STAC request metadata contains account credentials; use a reviewed authorization adapter".into());
                }
                check_public_body(value)?;
            }
        }
        Value::Array(values) => {
            for value in values {
                check_public_body(value)?;
            }
        }
        _ => {}
    }
    Ok(())
}

fn descriptor(
    value: &Value,
    base: &Url,
    original: Option<&MetadataRequest>,
) -> Result<MetadataRequest> {
    let method = match value.get("method").and_then(Value::as_str).unwrap_or("GET") {
        "GET" => SearchMethod::Get,
        "POST" => SearchMethod::Post,
        _ => return Err("STAC search links must use GET or POST".into()),
    };
    if value.get("method").is_some_and(|v| !v.is_string()) {
        return Err("STAC link method must be a string".into());
    }
    if value
        .get("headers")
        .is_some_and(|v| !v.as_object().is_some_and(|v| v.is_empty()))
    {
        return Err("STAC links requiring custom headers need an authorization adapter".into());
    }
    let merge = match value.get("merge") {
        None => false,
        Some(Value::Bool(value)) => *value,
        _ => return Err("STAC pagination merge must be a boolean".into()),
    };
    let mut body = value.get("body").cloned();
    if merge {
        let original = original.ok_or("STAC search endpoint cannot merge a previous request")?;
        if method != SearchMethod::Post || original.method != SearchMethod::Post {
            return Err("STAC pagination body merge requires an original POST request".into());
        }
        let mut merged = original.body.clone().unwrap_or_else(|| json!({}));
        let values = body
            .as_ref()
            .and_then(Value::as_object)
            .ok_or("STAC merged page body must be a JSON object")?;
        merged
            .as_object_mut()
            .ok_or("STAC original body must be a JSON object")?
            .extend(values.clone());
        body = Some(merged);
    }
    let request = MetadataRequest {
        url: scoped(
            base,
            value["href"]
                .as_str()
                .ok_or("STAC search link has no URL")?,
        )?
        .to_string(),
        method,
        body,
    };
    request.validate()?;
    Ok(request)
}

pub(super) struct Endpoints {
    pub get: Option<MetadataRequest>,
    pub post: Option<MetadataRequest>,
    pub post_advertised: bool,
}

pub(super) fn endpoints(value: &Value, base: &Url) -> Result<Endpoints> {
    let mut result = Endpoints {
        get: None,
        post: None,
        post_advertised: false,
    };
    for link in links(value, "search")? {
        let method = link["method"].as_str().unwrap_or("GET");
        result.post_advertised |= method == "POST";
        // Capability declarations may include additional account-based endpoints.
        // Keep an available public endpoint without forwarding custom headers.
        if link
            .get("headers")
            .is_some_and(|v| !v.as_object().is_some_and(|v| v.is_empty()))
        {
            continue;
        }
        if !matches!(method, "GET" | "POST") {
            continue;
        }
        let request = descriptor(link, base, None)?;
        let slot = if request.method == SearchMethod::Post {
            &mut result.post
        } else {
            &mut result.get
        };
        if slot.replace(request).is_some() {
            return Err(
                "STAC advertises ambiguous Item Search endpoints for one HTTP method".into(),
            );
        }
    }
    if result.get.is_none() && result.post.is_none() {
        return Err("STAC requires an advertised public GET or POST Item Search endpoint without custom headers".into());
    }
    Ok(result)
}

pub(super) fn initial(
    mut endpoint: MetadataRequest,
    request: &SearchRequest,
) -> Result<MetadataRequest> {
    let limit = request.limit.unwrap_or(100);
    if endpoint.method == SearchMethod::Post {
        let mut body = endpoint.body.take().unwrap_or_else(|| json!({}));
        let values = body
            .as_object_mut()
            .ok_or("STAC search body must be a JSON object")?;
        if values.contains_key("intersects") {
            return Err("STAC search endpoint defaults use intersects; choose an endpoint compatible with bounding-box searches".into());
        }
        values.insert("collections".into(), json!([request.collection_id]));
        values.insert("bbox".into(), json!(request.bounds));
        values.insert("limit".into(), json!(limit));
        values.remove("datetime");
        if let Some(datetime) = &request.datetime {
            values.insert("datetime".into(), json!(datetime));
        }
        endpoint.body = Some(body);
    } else {
        let mut url = endpoint.validate()?;
        let pairs = url
            .query_pairs()
            .filter(|(key, _)| {
                !matches!(key.as_ref(), "collections" | "bbox" | "limit" | "datetime")
            })
            .map(|(key, value)| (key.into_owned(), value.into_owned()))
            .collect::<Vec<_>>();
        url.set_query(None);
        url.query_pairs_mut()
            .extend_pairs(pairs)
            .append_pair("collections", &request.collection_id)
            .append_pair("bbox", &request.bounds.map(|v| v.to_string()).join(","))
            .append_pair("limit", &limit.to_string());
        if let Some(datetime) = &request.datetime {
            url.query_pairs_mut().append_pair("datetime", datetime);
        }
        endpoint.url = url.to_string();
    }
    endpoint.validate()?;
    Ok(endpoint)
}

pub(super) fn next(
    value: &Value,
    current: &MetadataRequest,
    original: &MetadataRequest,
) -> Result<Option<MetadataRequest>> {
    let links = links(value, "next")?;
    if links.len() > 1 {
        return Err("STAC has ambiguous next links".into());
    }
    links
        .first()
        .map(|link| descriptor(link, &current.validate()?, Some(original)))
        .transpose()
}

pub(super) async fn fetch(settings: &ProxySettings, request: &MetadataRequest) -> Result<Vec<u8>> {
    let url = request.validate()?;
    let client = features::client(&url, settings).await?;
    let builder = match request.method {
        SearchMethod::Get => client.get(url),
        SearchMethod::Post => client
            .post(url)
            .header("Content-Type", "application/json")
            .body(
                serde_json::to_vec(request.body.as_ref().unwrap_or(&json!({})))
                    .map_err(io_error)?,
            ),
    };
    let response = builder
        .header("Accept", "application/geo+json, application/json")
        .send()
        .await
        .map_err(|_| "Cannot reach the public STAC source")?;
    super::metadata_bytes(response).await
}

#[cfg(test)]
mod tests;
