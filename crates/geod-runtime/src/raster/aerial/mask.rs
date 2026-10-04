//! Standard TIFF FILETYPE_MASK directory, independent of all four imagery bands.
use crate::{io_error, Result};
use sha2::{Digest, Sha256};
use std::{
    fs::File,
    io::{Read, Seek, SeekFrom, Write},
    path::Path,
};
use tiff::{
    decoder::{ChunkType, Decoder},
    encoder::{
        compression::{CompressionAlgorithm, Deflate, DeflateLevel},
        TiffEncoder, TiffKind,
    },
    tags::Tag,
};

pub(crate) struct Spool {
    file: tempfile::NamedTempFile,
    counts: Vec<u64>,
    hash: Sha256,
}
impl Spool {
    pub(crate) fn new(directory: &Path) -> Result<Self> {
        Ok(Self {
            file: tempfile::Builder::new()
                .prefix("naip-mask-")
                .suffix(".part")
                .tempfile_in(directory)
                .map_err(io_error)?,
            counts: Vec::new(),
            hash: Sha256::new(),
        })
    }
    pub(crate) fn strip(
        &mut self,
        covered: &[bool],
        width: u32,
        cancel: &tokio_util::sync::CancellationToken,
    ) -> Result<()> {
        if width == 0 || !covered.len().is_multiple_of(width as usize) {
            return Err("NAIP coverage strip dimensions differ".into());
        }
        let stride = width.div_ceil(8) as usize;
        let mut packed = vec![0; covered.len() / width as usize * stride];
        for (row, pixels) in covered.chunks_exact(width as usize).enumerate() {
            if row.is_multiple_of(64) {
                crate::raster::check_cancel(Some(cancel))?;
            }
            for (col, valid) in pixels.iter().enumerate() {
                if *valid {
                    packed[row * stride + col / 8] |= 0x80 >> (col % 8);
                }
            }
        }
        self.hash.update(&packed);
        let mut encoded = Vec::new();
        Deflate::with_level(DeflateLevel::Balanced)
            .write_to(&mut encoded, &packed)
            .map_err(io_error)?;
        self.file.write_all(&encoded).map_err(io_error)?;
        self.counts.push(encoded.len() as u64);
        Ok(())
    }
    pub(crate) fn append<K: TiffKind>(
        &mut self,
        encoder: &mut TiffEncoder<&mut File, K>,
        width: u32,
        height: u32,
        rows: u32,
        cancel: &tokio_util::sync::CancellationToken,
    ) -> Result<String> {
        if rows == 0 || self.counts.len() != height.div_ceil(rows) as usize {
            return Err("NAIP coverage mask strip count differs".into());
        }
        self.file.seek(SeekFrom::Start(0)).map_err(io_error)?;
        let mut mask = encoder.image_directory().map_err(io_error)?;
        mask.write_tag(Tag::NewSubfileType, 4u32)
            .map_err(io_error)?;
        mask.write_tag(Tag::ImageWidth, width).map_err(io_error)?;
        mask.write_tag(Tag::ImageLength, height).map_err(io_error)?;
        mask.write_tag(Tag::BitsPerSample, &[1u16][..])
            .map_err(io_error)?;
        mask.write_tag(Tag::SamplesPerPixel, 1u16)
            .map_err(io_error)?;
        mask.write_tag(Tag::PhotometricInterpretation, 4u16)
            .map_err(io_error)?;
        mask.write_tag(Tag::Compression, 8u16).map_err(io_error)?;
        mask.write_tag(Tag::FillOrder, 1u16).map_err(io_error)?;
        mask.write_tag(Tag::PlanarConfiguration, 1u16)
            .map_err(io_error)?;
        mask.write_tag(Tag::RowsPerStrip, rows).map_err(io_error)?;
        let mut offsets = Vec::with_capacity(self.counts.len());
        let mut counts = Vec::with_capacity(self.counts.len());
        for count in &self.counts {
            crate::raster::check_cancel(Some(cancel))?;
            let mut encoded = vec![0; *count as usize];
            self.file.read_exact(&mut encoded).map_err(io_error)?;
            offsets.push(
                K::convert_offset(mask.write_data(&encoded[..]).map_err(io_error)?)
                    .map_err(io_error)?,
            );
            counts.push(K::convert_offset(*count).map_err(io_error)?);
        }
        mask.write_tag(Tag::StripOffsets, K::convert_slice(&offsets))
            .map_err(io_error)?;
        mask.write_tag(Tag::StripByteCounts, K::convert_slice(&counts))
            .map_err(io_error)?;
        mask.finish().map_err(io_error)?;
        Ok(format!("{:x}", self.hash.clone().finalize()))
    }
}

/// Leaves the decoder on the mask directory. These reviewed results have one
/// main image followed by one mask, not arbitrary overviews or multi-page TIFFs.
pub(crate) fn select<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    width: u32,
    height: u32,
) -> Result<()> {
    decoder.seek_to_image(0).map_err(io_error)?;
    if !decoder.more_images() {
        return Err("NAIP result coverage mask is missing".into());
    }
    decoder.next_image().map_err(io_error)?;
    if decoder.dimensions().map_err(io_error)? != (width, height)
        || decoder.more_images()
        || decoder.get_chunk_type() != ChunkType::Strip
        || decoder
            .find_tag_unsigned::<u32>(Tag::NewSubfileType)
            .map_err(io_error)?
            != Some(4)
        || decoder
            .find_tag_unsigned::<u16>(Tag::PhotometricInterpretation)
            .map_err(io_error)?
            != Some(4)
        || decoder
            .find_tag_unsigned_vec::<u16>(Tag::BitsPerSample)
            .map_err(io_error)?
            .as_deref()
            != Some(&[1])
        || decoder
            .find_tag_unsigned::<u16>(Tag::SamplesPerPixel)
            .map_err(io_error)?
            != Some(1)
        || decoder
            .find_tag_unsigned::<u16>(Tag::Compression)
            .map_err(io_error)?
            != Some(8)
        || decoder
            .find_tag_unsigned::<u16>(Tag::Predictor)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::FillOrder)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::PlanarConfiguration)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag_unsigned::<u16>(Tag::Orientation)
            .map_err(io_error)?
            .unwrap_or(1)
            != 1
        || decoder
            .find_tag(Tag::GdalNodata)
            .map_err(io_error)?
            .is_some()
    {
        return Err("NAIP result coverage mask tags differ".into());
    }
    let rows = decoder.chunk_dimensions().1;
    if rows == 0 || rows > 512 || decoder.strip_count().map_err(io_error)? != height.div_ceil(rows)
    {
        return Err("NAIP result mask strips are invalid".into());
    }
    Ok(())
}
pub(crate) fn strip<R: Read + Seek>(decoder: &mut Decoder<R>, index: u32) -> Result<Vec<u8>> {
    let (width, height) = decoder.chunk_data_dimensions(index);
    super::decoded_block(
        decoder,
        index,
        u64::from(width.div_ceil(8)) * u64::from(height),
    )
}
pub(crate) fn sample<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    dimensions: [u32; 2],
    pixel: [u32; 2],
) -> Result<bool> {
    select(decoder, dimensions[0], dimensions[1])?;
    let rows = decoder.chunk_dimensions().1;
    let data = strip(decoder, pixel[1] / rows)?;
    let offset =
        (pixel[1] % rows) as usize * dimensions[0].div_ceil(8) as usize + pixel[0] as usize / 8;
    let value = data.get(offset).ok_or("NAIP mask pixel is missing")?;
    Ok(*value & (0x80 >> (pixel[0] % 8)) != 0)
}
pub(crate) fn apply_preview<R: Read + Seek>(
    decoder: &mut Decoder<R>,
    dimensions: [u32; 2],
    preview: [u32; 2],
    rgba: &mut [u8],
    deadline: std::time::Instant,
) -> Result<()> {
    if preview.contains(&0) || rgba.len() != preview[0] as usize * preview[1] as usize * 4 {
        return Err("NAIP preview mask dimensions differ".into());
    }
    select(decoder, dimensions[0], dimensions[1])?;
    let rows = decoder.chunk_dimensions().1;
    let mut last = u32::MAX;
    let mut data = Vec::new();
    let stride = dimensions[0].div_ceil(8) as usize;
    for y in 0..preview[1] {
        crate::raster::check_time(deadline)?;
        let sy = (u64::from(y) * u64::from(dimensions[1]) / u64::from(preview[1])) as u32;
        let chunk = sy / rows;
        if chunk != last {
            data = strip(decoder, chunk)?;
            last = chunk;
        }
        for x in 0..preview[0] {
            let sx = (u64::from(x) * u64::from(dimensions[0]) / u64::from(preview[0])) as u32;
            let value = data
                .get((sy % rows) as usize * stride + sx as usize / 8)
                .ok_or("NAIP preview mask sample is missing")?;
            rgba[((y * preview[0] + x) * 4 + 3) as usize] = if value & (0x80 >> (sx % 8)) != 0 {
                255
            } else {
                0
            };
        }
    }
    Ok(())
}
