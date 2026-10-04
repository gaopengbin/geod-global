use super::*;

fn base() -> Url {
    Url::parse("https://example.com/stac/search").unwrap()
}
fn query() -> SearchRequest {
    SearchRequest {
        connection_id: "00000000-0000-4000-8000-000000000001".into(),
        collection_id: "originals".into(),
        bounds: [13., 52., 14., 53.],
        datetime: Some("2024-01-01T00:00:00Z/2024-01-02T00:00:00Z".into()),
        limit: Some(2),
        cursor: None,
    }
}
fn post() -> MetadataRequest {
    initial(
        MetadataRequest {
            url: base().to_string(),
            method: SearchMethod::Post,
            body: None,
        },
        &query(),
    )
    .unwrap()
}
fn page(link: Value) -> Value {
    json!({"links":[link]})
}

#[test]
fn post_only_discovery_and_initial_json_keep_the_requested_filters() {
    let catalog = page(
        json!({"rel":"search","href":"./search","method":"POST","body":{"query":{"provider:quality":{"eq":"accepted"}}}}),
    );
    let endpoints = endpoints(&catalog, &base()).unwrap();
    assert!(endpoints.get.is_none());
    assert!(endpoints.post_advertised);
    let request = initial(endpoints.post.unwrap(), &query()).unwrap();
    assert_eq!(request.method, SearchMethod::Post);
    assert_eq!(
        request.body,
        Some(
            json!({"bbox":[13.,52.,14.,53.], "collections":["originals"], "limit":2,
        "datetime":"2024-01-01T00:00:00Z/2024-01-02T00:00:00Z", "query":{"provider:quality":{"eq":"accepted"}}})
        )
    );
    let get = initial(MetadataRequest::get(&base()), &query()).unwrap();
    assert!(get.body.is_none());
    assert_ne!(get.identity().unwrap(), request.identity().unwrap());
}

#[test]
fn merge_uses_original_body_instead_of_carrying_stale_previous_page_fields() {
    let original = post();
    let second = next(&page(json!({"rel":"next","href":"search","method":"POST","body":{"token":"cursor-two","stale":true},"merge":true})), &original, &original).unwrap().unwrap();
    let third = next(&page(json!({"rel":"next","href":"search","method":"POST","body":{"token":"cursor-three"},"merge":true})), &second, &original).unwrap().unwrap();
    assert_eq!(second.url, third.url);
    assert_eq!(
        third.body.as_ref().unwrap()["bbox"],
        original.body.as_ref().unwrap()["bbox"]
    );
    assert_eq!(third.body.as_ref().unwrap()["token"], "cursor-three");
    assert!(third.body.as_ref().unwrap().get("stale").is_none());
    assert_ne!(second.identity().unwrap(), third.identity().unwrap());
}

#[test]
fn replacement_body_and_method_switch_are_literal_and_default_next_method_is_get() {
    let original = post();
    let next_page = next(
        &page(
            json!({"rel":"next","href":"search","method":"POST","body":{"token":"public-page-id"}}),
        ),
        &original,
        &original,
    )
    .unwrap()
    .unwrap();
    assert_eq!(next_page.body, Some(json!({"token":"public-page-id"})));
    let get = next(
        &page(json!({"rel":"next","href":"search?page=2"})),
        &original,
        &original,
    )
    .unwrap()
    .unwrap();
    assert_eq!(get.method, SearchMethod::Get);
    assert!(get.body.is_none());
    let post_again = next(
        &page(json!({"rel":"next","href":"search","method":"POST","body":{"token":"next"}})),
        &get,
        &original,
    )
    .unwrap()
    .unwrap();
    assert_eq!(post_again.method, SearchMethod::Post);
}

#[test]
fn page_identity_detects_body_repetition_even_with_different_key_order() {
    let first: MetadataRequest = serde_json::from_str(
        r#"{"url":"https://example.com/search","method":"POST","body":{"limit":2,"token":"x"}}"#,
    )
    .unwrap();
    let second: MetadataRequest = serde_json::from_str(
        r#"{"method":"POST","url":"https://example.com/search","body":{"token":"x","limit":2}}"#,
    )
    .unwrap();
    assert_eq!(first.identity().unwrap(), second.identity().unwrap());
}

#[test]
fn invalid_page_instructions_and_account_credentials_never_become_requests() {
    let original = post();
    for link in [
        json!({"rel":"next","href":"https://foreign.example/search","method":"POST","body":{}}),
        json!({"rel":"next","href":"/search","method":"DELETE"}),
        json!({"rel":"next","href":"/search","method":17}),
        json!({"rel":"next","href":"/search","method":"POST","body":[]}),
        json!({"rel":"next","href":"/search","method":"GET","body":{}}),
        json!({"rel":"next","href":"/search","method":"POST","body":{"authorization":"secret"}}),
        json!({"rel":"next","href":"/search","method":"POST","headers":{"X-Secret":"secret"}}),
        json!({"rel":"next","href":"/search","method":"POST","body":{},"merge":"true"}),
        json!({"rel":"next","href":"/search","method":"GET","merge":true}),
    ] {
        assert!(
            next(&page(link.clone()), &original, &original).is_err(),
            "{link}"
        );
    }
    let credentialed = page(
        json!({"rel":"next","href":"/search","method":"POST","body":{"nested":{"access_token":"secret"}}}),
    );
    assert!(json_document(&serde_json::to_vec(&credentialed).unwrap()).is_err());
    assert!(next(
        &json!({"links":[{"rel":"next","href":"a"},{"rel":"next","href":"b"}]}),
        &original,
        &original
    )
    .is_err());
    let oversized = page(
        json!({"rel":"next","href":"/search","method":"POST","body":{"token":"a".repeat(MAX_REQUEST)}}),
    );
    assert!(next(&oversized, &original, &original).is_err());
}

#[test]
fn legacy_get_defaults_replace_endpoint_filters_once_without_inventing_datetime() {
    let mut q = query();
    q.datetime = None;
    let endpoint = Url::parse("https://example.com/search?bbox=0,0,1,1&collections=wrong&limit=99&datetime=old&provider=stable").unwrap();
    let request = initial(MetadataRequest::get(&endpoint), &q).unwrap();
    let values = request
        .validate()
        .unwrap()
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect::<Vec<_>>();
    assert_eq!(values.iter().filter(|(k, _)| k == "bbox").count(), 1);
    assert!(values.contains(&("collections".into(), "originals".into())));
    assert!(values.contains(&("provider".into(), "stable".into())));
    assert!(!values.iter().any(|(k, _)| k == "datetime"));
    assert!(next(
        &page(json!({"rel":"next","href":"search","method":"POST","body":{},"merge":true})),
        &request,
        &request
    )
    .is_err());
}

#[test]
fn source_discovery_rejects_ambiguous_methods_and_can_use_a_public_get_beside_private_post() {
    let ambiguous = json!({"links":[{"rel":"search","href":"a","method":"POST"},{"rel":"search","href":"b","method":"POST"}]});
    assert!(endpoints(&ambiguous, &base()).is_err());
    let public = json!({"links":[{"rel":"search","href":"public"},{"rel":"search","href":"private","method":"POST","headers":{"X-Custom":"required"}}]});
    let endpoint = endpoints(&public, &base()).unwrap();
    assert!(endpoint.get.is_some());
    assert!(endpoint.post.is_none());
    assert!(endpoint.post_advertised);
}
