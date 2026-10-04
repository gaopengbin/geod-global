//! JP2 box/SIZ preflight before the decoder can allocate component buffers.
use super::*;

fn u32_at(data: &[u8], offset: usize) -> Result<u32> {
    Ok(u32::from_be_bytes(
        data.get(offset..offset + 4)
            .ok_or("Truncated JP2 header")?
            .try_into()
            .map_err(io_error)?,
    ))
}
fn u16_at(data: &[u8], offset: usize) -> Result<u16> {
    Ok(u16::from_be_bytes(
        data.get(offset..offset + 2)
            .ok_or("Truncated JP2 header")?
            .try_into()
            .map_err(io_error)?,
    ))
}
fn bytes(file: &mut File, start: u64, count: usize) -> Result<Vec<u8>> {
    file.seek(SeekFrom::Start(start)).map_err(io_error)?;
    let mut data = vec![0; count];
    file.read_exact(&mut data).map_err(io_error)?;
    Ok(data)
}

pub(super) fn preflight(path: &Path, grid: &SafeOutput) -> Result<()> {
    let mut file = File::open(path).map_err(io_error)?;
    let size = file.metadata().map_err(io_error)?.len();
    if size < 64
        || bytes(&mut file, 0, 12)? != [0, 0, 0, 12, b'j', b'P', b' ', b' ', 13, 10, 135, 10]
    {
        return Err("SAFE image is not a complete JP2 file".into());
    }
    let mut offset = 12;
    let mut codestream = None;
    let mut header = None;
    let mut count = 0;
    while offset < size {
        count += 1;
        if count > 128 || size - offset < 8 {
            return Err("Invalid JP2 box directory".into());
        }
        let box_header = bytes(&mut file, offset, 8)?;
        let mut length = u32_at(&box_header, 0)? as u64;
        let mut prefix = 8;
        if length == 1 {
            length = u64::from_be_bytes(
                bytes(&mut file, offset + 8, 8)?
                    .as_slice()
                    .try_into()
                    .map_err(io_error)?,
            );
            prefix = 16;
        }
        if length == 0 {
            length = size - offset;
        }
        if length < prefix || length > size - offset {
            return Err("JP2 box is truncated or has invalid length".into());
        }
        match &box_header[4..8] {
            b"jp2h" => {
                if header.is_some() || length > 1024 * 1024 {
                    return Err("JP2 image header is repeated or oversized".into());
                }
                header = Some(bytes(
                    &mut file,
                    offset + prefix,
                    (length - prefix) as usize,
                )?);
            }
            b"jp2c" => {
                if codestream.is_some() {
                    return Err("Multiple JP2 codestreams are unsupported".into());
                }
                codestream = Some((offset + prefix, length - prefix));
            }
            _ => {}
        }
        offset += length;
    }
    let header = header.ok_or("JP2 image header is missing")?;
    let mut offset = 0;
    let mut image_header = false;
    let mut color = false;
    while offset < header.len() {
        let length = u32_at(&header, offset)? as usize;
        if length < 8 || length > header.len() - offset {
            return Err("Invalid JP2 image-header box".into());
        }
        let value = &header[offset + 8..offset + length];
        match header
            .get(offset + 4..offset + 8)
            .ok_or("Truncated JP2 header")?
        {
            b"ihdr" => {
                if image_header
                    || value.len() != 14
                    || u32_at(value, 0)? != grid.height
                    || u32_at(value, 4)? != grid.width
                    || u16_at(value, 8)? != grid.band_count as u16
                    || value[10] != 7
                    || value[11] != 7
                {
                    return Err("JP2 header does not match the SAFE UInt8 grid/bands".into());
                }
                image_header = true;
            }
            b"colr" => {
                if color
                    || value.len() != 7
                    || value[..3] != [1, 0, 0]
                    || u32_at(value, 3)? != if grid.band_count == 3 { 16 } else { 17 }
                {
                    return Err("JP2 color interpretation is unsupported for TCI/SCL".into());
                }
                color = true;
            }
            b"pclr" | b"cmap" | b"cdef" | b"bpcc" => {
                return Err(
                    "JP2 palette, alpha or variable bit depth is unsupported for TCI/SCL".into(),
                )
            }
            _ => {}
        }
        offset += length;
    }
    if !image_header || !color {
        return Err("JP2 lacks its image/color header".into());
    }
    let (offset, length) = codestream.ok_or("JP2 codestream is missing")?;
    let expected = 42 + 3 * grid.band_count as usize;
    if length < expected as u64 + 2 {
        return Err("JP2 codestream is truncated".into());
    }
    let siz = bytes(&mut file, offset, expected)?;
    if siz[..4] != [255, 79, 255, 81]
        || u16_at(&siz, 4)? as usize != 38 + 3 * grid.band_count as usize
        || u32_at(&siz, 8)? != grid.width
        || u32_at(&siz, 12)? != grid.height
        || u32_at(&siz, 16)? != 0
        || u32_at(&siz, 20)? != 0
        || u32_at(&siz, 32)? != 0
        || u32_at(&siz, 36)? != 0
        || u16_at(&siz, 40)? != grid.band_count as u16
    {
        return Err("JP2 codestream dimensions/origin/components differ from SAFE XML".into());
    }
    let tw = u32_at(&siz, 24)?;
    let th = u32_at(&siz, 28)?;
    if tw == 0
        || th == 0
        || tw > 10980
        || th > 10980
        || (grid.width.div_ceil(tw) as u64 * grid.height.div_ceil(th) as u64) > 65536
    {
        return Err("JP2 coding tile geometry is unsupported".into());
    }
    if !siz[42..]
        .chunks_exact(3)
        .all(|component| component == [7, 1, 1])
    {
        return Err("JP2 components must be unsigned 8-bit, un-subsampled TCI/SCL".into());
    }
    if bytes(&mut file, offset + length - 2, 2)? != [255, 217] {
        return Err("JP2 codestream has no complete end marker".into());
    }
    Ok(())
}

pub(super) fn strip(path: &Path, grid: &SafeOutput, row: u32, height: u32) -> Result<Vec<u8>> {
    let image = jpeg2k::Image::from_file_with(
        path,
        jpeg2k::DecodeParameters::new()
            .strict(true)
            .decode_area(Some(jpeg2k::DecodeArea::new(
                0,
                row,
                grid.width,
                row + height,
            ))),
    )
    .map_err(|e| format!("SAFE JP2 decode failed: {e}"))?;
    if image.width() != grid.width
        || image.height() != height
        || image.num_components() != grid.band_count as u32
        || image.x_offset() != 0
        || image.y_offset() != row
    {
        return Err("Decoded JP2 strip geometry differs from its requested grid".into());
    }
    let components = image.components();
    if components.iter().any(|c| {
        c.is_signed()
            || c.is_alpha()
            || c.precision() != 8
            || c.width() != grid.width
            || c.height() != height
    }) {
        return Err("Decoded JP2 components are not original UInt8 samples".into());
    }
    let samples = grid.width as usize * height as usize;
    let mut pixels = vec![0u8; samples * grid.band_count as usize];
    for (band, component) in components.iter().enumerate() {
        // Do not use data_u8(): that helper scales samples. SCL class codes and
        // TCI channels are copied from the original decoded component values.
        for (index, &sample) in component.data().iter().enumerate() {
            if !(0..=if grid.band_count == 1 { 11 } else { 255 }).contains(&sample) {
                return Err("JP2 sample is outside its TCI/SCL range".into());
            }
            pixels[index * grid.band_count as usize + band] = sample as u8;
        }
    }
    Ok(pixels)
}
