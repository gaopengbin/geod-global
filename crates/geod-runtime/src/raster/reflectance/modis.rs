//! Validate the actual GeoTIFF custom sinusoidal CRS, not a catalogue EPSG.
use super::*;
use std::io::Write;
pub(crate) fn write_crs<W: Write + Seek, K: tiff::encoder::TiffKind>(
    image: &mut tiff::encoder::DirectoryEncoder<'_, W, K>,
) -> Result<()> {
    let keys = [
        1u16, 1, 0, 16, 1024, 0, 1, 1, 1025, 0, 1, 1, 2048, 0, 1, 32767, 2050, 0, 1, 32767, 2054,
        0, 1, 9102, 2056, 0, 1, 32767, 2057, 34736, 1, 0, 2058, 34736, 1, 1, 2061, 34736, 1, 2,
        3072, 0, 1, 32767, 3074, 0, 1, 32767, 3075, 0, 1, 24, 3076, 0, 1, 9001, 3082, 34736, 1, 3,
        3083, 34736, 1, 4, 3088, 34736, 1, 5,
    ];
    image
        .write_tag(Tag::GeoKeyDirectoryTag, &keys[..])
        .map_err(io_error)?;
    image
        .write_tag(
            Tag::GeoDoubleParamsTag,
            &[
                providers::modis::RADIUS,
                providers::modis::RADIUS,
                0.0,
                0.0,
                0.0,
                0.0,
            ][..],
        )
        .map_err(io_error)?;
    Ok(())
}

pub(crate) fn crs<R: Read + Seek>(decoder: &mut Decoder<R>, keys: &[u16]) -> Result<String> {
    let invalid = "MODIS COG must use its original metre-based sinusoidal sphere and Area grid";
    if keys.len() < 4 || keys[0..3] != [1, 1, 0] || keys.len() != 4 + usize::from(keys[3]) * 4 {
        return Err(invalid.into());
    }
    let doubles = decoder
        .get_tag_f64_vec(Tag::GeoDoubleParamsTag)
        .map_err(io_error)?;
    let mut entries = BTreeMap::new();
    for key in keys[4..].chunks_exact(4) {
        if entries.insert(key[0], &key[1..]).is_some() {
            return Err(invalid.into());
        }
    }
    for (key, value) in [
        (1024, 1),
        (1025, 1),
        (2048, 32767),
        (2050, 32767),
        (2054, 9102),
        (2056, 32767),
        (3072, 32767),
        (3074, 32767),
        (3075, 24),
        (3076, 9001),
    ] {
        if entries.get(&key).copied() != Some([0, 1, value].as_slice()) {
            return Err(invalid.into());
        }
    }
    for (key, expected) in [
        (2057, 6371007.181),
        (2058, 6371007.181),
        (2061, 0.0),
        (3082, 0.0),
        (3083, 0.0),
        (3088, 0.0),
    ] {
        let entry = entries.get(&key).ok_or(invalid)?;
        if entry[0] != 34736
            || entry[1] != 1
            || doubles
                .get(usize::from(entry[2]))
                .is_none_or(|value| !value.is_finite() || (*value - expected).abs() > 1e-8)
        {
            return Err(invalid.into());
        }
    }
    if entries.keys().any(|key| {
        ![
            1024, 1025, 1026, 2048, 2049, 2050, 2054, 2056, 2057, 2058, 2061, 3072, 3074, 3075,
            3076, 3082, 3083, 3088,
        ]
        .contains(key)
    }) {
        return Err(invalid.into());
    }
    Ok(providers::modis::CRS.into())
}
pub(crate) fn grid(header: &Header, id: &str) -> Result<()> {
    let (h, v) = providers::modis::identity(id).ok_or("Invalid MODIS tile identity")?;
    tile_grid(header, h, v, 2400)
}
pub(crate) fn vegetation_grid(header: &Header, id: &str) -> Result<()> {
    let (h, v) =
        providers::vegetation::identity(id).ok_or("Invalid MODIS vegetation tile identity")?;
    tile_grid(header, h, v, 4800)
}
fn tile_grid(header: &Header, h: u32, v: u32, edge: u32) -> Result<()> {
    let size = providers::modis::PIXEL * 2400.0;
    let expected = [
        (f64::from(h) - 18.0) * size,
        (8.0 - f64::from(v)) * size,
        (f64::from(h) - 17.0) * size,
        (9.0 - f64::from(v)) * size,
    ];
    if header.width != edge
        || header.height != edge
        || header.pixel_is_point
        || header
            .bounds
            .iter()
            .zip(expected)
            .any(|(a, b)| (*a - b).abs() > 0.02)
    {
        return Err(
            "MODIS original grid does not match its horizontal/vertical tile identity".into(),
        );
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tiff::encoder::{colortype, TiffEncoder};
    fn encoded(keys: &[u16], doubles: &[f64]) -> Vec<u8> {
        let mut bytes = Cursor::new(Vec::new());
        {
            let mut encoder = TiffEncoder::new(&mut bytes).unwrap();
            let mut image = encoder.new_image::<colortype::GrayI16>(2, 1).unwrap();
            image
                .encoder()
                .write_tag(Tag::GeoKeyDirectoryTag, keys)
                .unwrap();
            image
                .encoder()
                .write_tag(Tag::GeoDoubleParamsTag, doubles)
                .unwrap();
            image.write_data(&[-28672, 0]).unwrap();
        }
        bytes.into_inner()
    }
    #[test]
    fn sinusoidal_requires_the_actual_sphere_transform_units_and_area_grid() {
        let keys = [
            1, 1, 0, 16, 1024, 0, 1, 1, 1025, 0, 1, 1, 2048, 0, 1, 32767, 2050, 0, 1, 32767, 2054,
            0, 1, 9102, 2056, 0, 1, 32767, 2057, 34736, 1, 3, 2058, 34736, 1, 4, 2061, 34736, 1, 5,
            3072, 0, 1, 32767, 3074, 0, 1, 32767, 3075, 0, 1, 24, 3076, 0, 1, 9001, 3082, 34736, 1,
            1, 3083, 34736, 1, 2, 3088, 34736, 1, 0,
        ];
        let doubles = [0.0, 0.0, 0.0, 6371007.181, 6371007.181, 0.0];
        let mut decoder = Decoder::new(Cursor::new(encoded(&keys, &doubles))).unwrap();
        assert_eq!(crs(&mut decoder, &keys).unwrap(), providers::modis::CRS);
        for (key, value) in [(1025, 2), (3075, 1), (3076, 9002), (2057, 99)] {
            let mut changed = keys;
            let entry = changed[4..]
                .chunks_exact_mut(4)
                .find(|k| k[0] == key)
                .unwrap();
            entry[3] = value;
            assert!(crs(&mut decoder, &changed).is_err());
        }
        let mut changed = doubles;
        changed[3] = 6378137.0;
        let mut decoder = Decoder::new(Cursor::new(encoded(&keys, &changed))).unwrap();
        assert!(crs(&mut decoder, &keys).is_err());
        let mut duplicate = keys.to_vec();
        duplicate[3] += 1;
        duplicate.extend([1024, 0, 1, 1]);
        assert!(crs(&mut decoder, &duplicate).is_err());
    }
    #[test]
    fn original_modis_tile_shape_and_extent_cannot_move_to_a_different_grid() {
        let id = "MYD09A1.A2025177.h08v05.061.2025189031924";
        let header = Header {
            width: 2400,
            height: 2400,
            crs: providers::modis::CRS.into(),
            bounds: [
                -11119505.196667,
                3335851.559000,
                -10007554.677000,
                4447802.078667,
            ],
            pixel_size: [providers::modis::PIXEL; 2],
            pixel_is_point: false,
        };
        grid(&header, id).unwrap();
        assert!(grid(&header, &id.replace("h08v05", "h09v05")).is_err());
        let mut changed = header;
        changed.pixel_is_point = true;
        assert!(grid(&changed, id).is_err());
        changed.pixel_is_point = false;
        changed.width = 1200;
        assert!(grid(&changed, id).is_err());
    }
    #[test]
    fn vegetation_original_grid_uses_4800_samples_at_the_same_modland_tile_edges() {
        let id = "MOD13Q1.A2025177.h08v05.061.2025195142416";
        let size = providers::modis::PIXEL * 2400.0;
        let mut header = Header {
            width: 4800,
            height: 4800,
            crs: providers::modis::CRS.into(),
            bounds: [-10.0 * size, 3.0 * size, -9.0 * size, 4.0 * size],
            pixel_size: [providers::vegetation::PIXEL; 2],
            pixel_is_point: false,
        };
        vegetation_grid(&header, id).unwrap();
        assert!(vegetation_grid(&header, &id.replace("h08v05", "h09v05")).is_err());
        header.width = 2400;
        assert!(vegetation_grid(&header, id).is_err());
        header.width = 4800;
        header.pixel_is_point = true;
        assert!(vegetation_grid(&header, id).is_err());
        assert_eq!(vegetation_color(-32768), vegetation_color(-2000));
        assert_eq!(vegetation_color(32767), vegetation_color(10000));
        assert_ne!(vegetation_color(0), vegetation_color(10000));
    }
    #[test]
    fn vegetation_palette_rounds_exact_half_ties_consistently() {
        // The blue channel here is exactly 104.5. Floating interpolation used
        // to round it down for real MOD13Q1 samples with this DN.
        assert_eq!(vegetation_color(4625), [143, 185, 105]);
        assert_eq!(vegetation_color(-2000), [42, 91, 135]);
        assert_eq!(vegetation_color(10000), [24, 86, 51]);
    }
}
