//! Public Census city geometry; no geocoding entitlement, inferred radius or
//! OSM identity. The same native proxy and bounded transport apply to both sources.
use super::*;
mod rings;
pub(super) const SOURCE: &str = "https://tigerweb.geo.census.gov/arcgis/rest/services/TIGERweb/Places_CouSub_ConCity_SubMCD/MapServer/4";

pub(super) fn eligible(query: &Query) -> bool {
    query.kind == Kind::City && query.country_code.as_deref() == Some("US")
}
pub(super) fn url(query: &Query) -> url::Url {
    let mut url = url::Url::parse(&format!("{SOURCE}/query")).unwrap();
    // A name is a SQL literal, never a provider-supplied expression.
    let name = query.query.to_uppercase().replace('\'', "''");
    url.query_pairs_mut()
        .append_pair(
            "where",
            &format!("(UPPER(BASENAME)='{name}' OR UPPER(NAME)='{name}')"),
        )
        .append_pair("outFields", "GEOID,BASENAME,NAME,INTPTLAT,INTPTLON")
        .append_pair("returnGeometry", "true")
        .append_pair("outSR", "4326")
        .append_pair("resultRecordCount", "5")
        .append_pair("f", "json");
    url
}
pub(super) fn normalize(document: &Value, query: &Query) -> Result<Vec<Place>> {
    if document.get("error").is_some()
        || document["geometryType"] != "esriGeometryPolygon"
        || document["spatialReference"]["wkid"] != 4326
    {
        return Err("Place service returned an invalid geographic document.".into());
    }
    let features = document["features"]
        .as_array()
        .filter(|f| f.len() <= 5)
        .ok_or("Place service returned an invalid candidate list.")?;
    let mut places = Vec::new();
    for feature in features {
        let a = &feature["attributes"];
        let Some((name, geoid, x, y)) = a["BASENAME"]
            .as_str()
            .zip(a["GEOID"].as_str())
            .zip(a["INTPTLON"].as_str().and_then(|s| s.parse::<f64>().ok()))
            .zip(a["INTPTLAT"].as_str().and_then(|s| s.parse::<f64>().ok()))
            .map(|(((n, id), x), y)| (n, id, x, y))
        else {
            continue;
        };
        let geometry = &feature["geometry"];
        if geometry.get("curveRings").is_some() {
            continue;
        }
        let Some(rings) = geometry["rings"]
            .as_array()
            .filter(|rings| !rings.is_empty())
        else {
            continue;
        };
        let mut extent = [
            f64::INFINITY,
            f64::INFINITY,
            f64::NEG_INFINITY,
            f64::NEG_INFINITY,
        ];
        let mut count = 0usize;
        let mut valid = true;
        for ring in rings {
            let Some(positions) = ring
                .as_array()
                .filter(|p| p.len() >= 4 && p.first() == p.last())
            else {
                valid = false;
                break;
            };
            for position in positions {
                count += 1;
                let Some((lon, lat)) = position
                    .as_array()
                    .filter(|p| p.len() == 2)
                    .and_then(|p| p[0].as_f64().zip(p[1].as_f64()))
                else {
                    valid = false;
                    break;
                };
                if count > 50000
                    || !lon.is_finite()
                    || !lat.is_finite()
                    || !(-180.0..=180.0).contains(&lon)
                    || !(-90.0..=90.0).contains(&lat)
                {
                    valid = false;
                    break;
                }
                extent[0] = extent[0].min(lon);
                extent[1] = extent[1].min(lat);
                extent[2] = extent[2].max(lon);
                extent[3] = extent[3].max(lat);
            }
            if !valid {
                break;
            }
        }
        let place = Place {
            name: name.into(),
            kind: "city".into(),
            country: Some("United States".into()),
            country_code: Some("US".into()),
            state: None,
            county: None,
            center: [x, y],
            bounds: Some(extent),
            osm_type: None,
            osm_id: None,
            census_geoid: Some(geoid.into()),
            geometry: rings::geometry(geometry).ok(),
        };
        if valid
            && place.valid(query)
            && !places
                .iter()
                .any(|p: &Place| p.census_geoid == place.census_geoid)
        {
            places.push(place);
        }
    }
    Ok(places)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn query(name: &str, country: Option<&str>) -> Query {
        Query {
            query: name.into(),
            kind: Kind::City,
            country_code: country.map(str::to_owned),
        }
    }
    fn document() -> Value {
        json!({"geometryType":"esriGeometryPolygon","spatialReference":{"wkid":4326},"features":[{
            "attributes":{"BASENAME":"New York","GEOID":"3651000","INTPTLON":"-74.0","INTPTLAT":"40.7"},
            "geometry":{"rings":[[[-74.2,40.4],[-73.7,40.4],[-73.7,41.0],[-74.2,40.4]]]}
        }]})
    }
    #[test]
    fn official_city_geometry_has_census_identity_and_computed_extent() {
        let q = query("New York", Some("US"));
        assert_eq!(q.providers(), vec![Provider::Census, Provider::Photon]);
        let places = normalize(&document(), &q).unwrap();
        assert_eq!(places[0].bounds, Some([-74.2, 40.4, -73.7, 41.0]));
        assert_eq!(places[0].census_geoid.as_deref(), Some("3651000"));
        assert!(places[0].osm_id.is_none());
        let cache = Cache {
            provider: Provider::Census,
            query: q.clone(),
            checked_at: now(),
            places,
        };
        assert!(cache.fresh(&q, Utc::now()));
        assert_eq!(cache.result(false)["provider"], Provider::Census.label());
    }
    #[tokio::test]
    async fn retained_official_new_york_response_keeps_all_three_parts_and_positions_native() {
        // Real archived provider response; this test makes no live request and
        // does not download imagery or replace the user's profile.
        let doc: Value = serde_json::from_str(include_str!(
            "../../../fixtures/boundaries/census-new-york.json"
        ))
        .unwrap();
        let query = query("New York", Some("US"));
        let places = normalize(&doc, &query).unwrap();
        let geometry = places[0].geometry.clone().unwrap();
        let crate::crop::PolygonGeometry::MultiPolygon(parts) = &geometry else {
            panic!("lost islands")
        };
        assert_eq!(parts.len(), 3);
        assert_eq!(
            parts
                .iter()
                .flat_map(|p| p.iter())
                .map(Vec::len)
                .sum::<usize>(),
            1645
        );
        let dir = tempfile::tempdir().unwrap();
        let manager = JobManager::open(dir.path()).await.unwrap();
        let cache = Cache {
            provider: Provider::Census,
            query: query.clone(),
            checked_at: now(),
            places,
        };
        write_record(&manager.inner.root, "places", &query.cache_id(), &cache)
            .await
            .unwrap();
        assert!(cache.result(true).to_string().len() < 4096);
        assert!(cache.result(true)["candidates"][0]
            .get("geometry")
            .is_none());
        let session = Uuid::new_v4().to_string();
        let source = cache.result(true)["candidates"][0]["boundarySource"].clone();
        let result = super::super::super::boundary::read(
            &manager,
            &session,
            serde_json::from_value(source).unwrap(),
        )
        .await
        .unwrap();
        let resolved = super::super::super::boundary::resolve(
            &manager,
            &session,
            serde_json::from_value(result["boundary"].clone()).unwrap(),
        )
        .await
        .unwrap();
        assert_eq!(resolved, geometry);
        assert!(manager.list().await.is_empty());
        manager.shutdown().await.unwrap();
    }
    #[test]
    fn provider_scope_and_query_literals_cannot_reinterpret_other_places() {
        assert!(!eligible(&query("New York", None)));
        assert!(!eligible(&query("Paris", None)));
        assert!(!eligible(&query("New York", Some("GB"))));
        assert!(eligible(&query("Springfield", Some("US"))));
        let q = query("O'Fallon", Some("US"));
        assert!(url(&q).query_pairs().any(|(k, v)| k == "where"
            && v == "(UPPER(BASENAME)='O''FALLON' OR UPPER(NAME)='O''FALLON')"));
        assert!(url(&query("New York city", Some("US")))
            .query_pairs()
            .any(|(k, v)| k == "where" && v.contains("UPPER(NAME)='NEW YORK CITY'")));
    }
    #[test]
    fn malformed_or_wrong_crs_geometry_never_becomes_a_search_extent() {
        for (field, value) in [
            (
                "geometry",
                json!({"rings":[[[200,0],[200,1],[201,1],[200,0]]]}),
            ),
            (
                "geometry",
                json!({"rings":[[[-74,40],[-73,40],[-73,41],[-74,41]]]}),
            ),
            (
                "attributes",
                json!({"BASENAME":"New York","GEOID":"wrong","INTPTLON":"-74","INTPTLAT":"40"}),
            ),
        ] {
            let mut doc = document();
            doc["features"][0][field] = value;
            assert!(normalize(&doc, &query("New York", None))
                .unwrap()
                .is_empty());
        }
        let mut doc = document();
        doc["spatialReference"]["wkid"] = json!(3857);
        assert!(normalize(&doc, &query("New York", None)).is_err());
    }
}
