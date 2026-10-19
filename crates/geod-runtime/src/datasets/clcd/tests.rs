use super::*;

#[test]
fn metadata_range_digest_is_never_a_whole_original_allowance() {
    assert!(!large_grid_identity(
        &INDEX.metadata_range_sha256,
        161_378,
        135_079,
        true
    ));
    assert!(
        validate_geotiff_crs(&INDEX.metadata_range_sha256, GEOKEYS, GEODOUBLES, GEOASCII).is_err()
    );
    if !INDEX.whole_file_verified
        || !INDEX.publisher_md5_verified
        || INDEX.original_sha256.is_none()
    {
        assert!(original_sha256().unwrap_err().contains("pending"));
    }
}

#[test]
fn exact_user_defined_albers_keys_preserve_authority_datum_and_units() {
    let ascii = GEOASCII.trim_end_matches('\0');
    validate_crs_values(GEOKEYS, GEODOUBLES, ascii).unwrap();
    for index in [7, 11, 19, 47, 55, 63, 67, 71, 75] {
        let mut keys = GEOKEYS.to_vec();
        keys[index] ^= 1;
        assert!(validate_crs_values(&keys, GEODOUBLES, ascii).is_err());
    }
    for index in 0..GEODOUBLES.len() {
        let mut doubles = GEODOUBLES.to_vec();
        doubles[index] += 0.01;
        assert!(validate_crs_values(GEOKEYS, &doubles, ascii).is_err());
    }
    assert!(validate_crs_values(GEOKEYS, GEODOUBLES, "WGS 84|").is_err());
    assert!(!CRS.starts_with("EPSG:"));
}

#[test]
fn complete_index_hashes_use_canonical_u64_little_endian_values() {
    let bytes = [1u64, 512, 791_887_020]
        .into_iter()
        .flat_map(u64::to_le_bytes)
        .collect::<Vec<_>>();
    assert_eq!(
        integer_array_hash(&[1, 512, 791_887_020]),
        format!("{:x}", Sha256::digest(&bytes))
    );
    assert_ne!(
        integer_array_hash(&[1, 512, 791_887_020]),
        integer_array_hash(&[512, 1, 791_887_020])
    );
}

#[test]
fn incomplete_review_cannot_resolve_a_large_grid_or_other_year() {
    let request = ResolveRequest {
        dataset_id: "clcd".into(),
        bounds: [113., 23., 114., 24.],
        year: Some(2004),
        month: None,
        country: None,
        layer: Some("classes".into()),
        region: None,
        season: None,
    };
    if original_sha256().is_err() {
        assert!(resolve(&request).unwrap_err().contains("pending"));
        assert!(!large_grid_identity(
            &"a".repeat(64),
            161_378,
            135_079,
            true
        ));
    }
    let mut other = request;
    other.year = Some(2005);
    assert!(resolve(&other).is_err());
}
