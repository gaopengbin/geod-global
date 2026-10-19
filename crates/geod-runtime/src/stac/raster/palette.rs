//! Read-only TIFF tag view for numerical palette indices. Only an inline SHORT
//! PhotometricInterpretation value of 3 is viewed as BlackIsZero (1). Original
//! bytes, pixel streams, GeoKeys, sample layout, masks and palettes are intact.
//! This compensates for the decoder's unsupported RGBPalette interpretation;
//! it is not a conversion, a rewritten TIFF or a science calibration.
//! A separate, exact one-bit/one-sample transparency-mask IFD (type & 4 and
//! Photo=4) is also viewed as BlackIsZero only to decode its packed validity
//! bits. Such masks never become palette indices or scalar NoData values.
use crate::{io_error, Result};
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    io::{Read, Seek, SeekFrom},
};

pub(super) struct RawPaletteReader<R> {
    reader: R,
    patches: BTreeMap<u64, u8>,
    position: u64,
    has_palette: bool,
}

#[derive(Clone, Copy)]
struct Layout {
    little: bool,
    big: bool,
    length: u64,
}
impl Layout {
    fn u16(self, bytes: &[u8]) -> u16 {
        if self.little {
            u16::from_le_bytes(bytes[..2].try_into().unwrap())
        } else {
            u16::from_be_bytes(bytes[..2].try_into().unwrap())
        }
    }
    fn u32(self, bytes: &[u8]) -> u32 {
        if self.little {
            u32::from_le_bytes(bytes[..4].try_into().unwrap())
        } else {
            u32::from_be_bytes(bytes[..4].try_into().unwrap())
        }
    }
    fn u64(self, bytes: &[u8]) -> u64 {
        if self.little {
            u64::from_le_bytes(bytes[..8].try_into().unwrap())
        } else {
            u64::from_be_bytes(bytes[..8].try_into().unwrap())
        }
    }
    fn offset(self, bytes: &[u8]) -> u64 {
        if self.big {
            self.u64(bytes)
        } else {
            self.u32(bytes) as u64
        }
    }
    fn at<R: Read + Seek>(self, reader: &mut R, offset: u64, count: usize) -> Result<Vec<u8>> {
        if offset
            .checked_add(count as u64)
            .is_none_or(|end| end > self.length)
        {
            return Err("TIFF directory or palette tag lies outside the original file".into());
        }
        let mut value = vec![0; count];
        reader.seek(SeekFrom::Start(offset)).map_err(io_error)?;
        reader.read_exact(&mut value).map_err(io_error)?;
        Ok(value)
    }
}

impl<R: Read + Seek> RawPaletteReader<R> {
    pub(super) fn new(mut reader: R) -> Result<Self> {
        let length = reader.seek(SeekFrom::End(0)).map_err(io_error)?;
        reader.seek(SeekFrom::Start(0)).map_err(io_error)?;
        let mut header = [0; 8];
        reader.read_exact(&mut header).map_err(io_error)?;
        let little = match &header[..2] {
            b"II" => true,
            b"MM" => false,
            _ => return Err("Invalid TIFF byte order".into()),
        };
        let mut layout = Layout {
            little,
            big: false,
            length,
        };
        let version = layout.u16(&header[2..4]);
        let first = match version {
            42 => layout.u32(&header[4..8]) as u64,
            43 => {
                if layout.u16(&header[4..6]) != 8 || layout.u16(&header[6..8]) != 0 {
                    return Err("Unsupported BigTIFF offset layout".into());
                }
                layout.big = true;
                let bytes = layout.at(&mut reader, 8, 8)?;
                layout.u64(&bytes)
            }
            _ => return Err("Unsupported TIFF version".into()),
        };
        let mut patches = BTreeMap::new();
        let mut has_palette = false;
        let mut pending = VecDeque::from([first]);
        let mut visited = BTreeSet::new();
        while let Some(offset) = pending.pop_front() {
            if offset == 0 {
                continue;
            }
            if !visited.insert(offset) {
                // SubIFD pointers can legitimately share a directory with the
                // main overview chain. Visit once rather than read it twice.
                continue;
            }
            if visited.len() > 128 {
                return Err("TIFF exceeds 128 image directories".into());
            }
            let (count_size, entry_size, inline_size, inline_position) = if layout.big {
                (8, 20, 8, 12)
            } else {
                (2, 12, 4, 8)
            };
            let count_bytes = layout.at(&mut reader, offset, count_size)?;
            let count = if layout.big {
                layout.u64(&count_bytes)
            } else {
                layout.u16(&count_bytes) as u64
            };
            if count > 4096 {
                return Err("TIFF directory exceeds 4,096 tags".into());
            }
            let entries_start = offset
                .checked_add(count_size as u64)
                .ok_or("TIFF directory arithmetic overflow")?;
            let entries_bytes = count as usize * entry_size;
            let entries = layout.at(&mut reader, entries_start, entries_bytes + inline_size)?;
            // Only inline scalar declarations can authorize the mask view.
            // The unchanged directory remains subject to the same bounds,
            // tag-count and IFD-count guards as the palette path.
            let scalar = |wanted: u16, allowed: &[u16]| -> Result<Option<u32>> {
                let mut found = None;
                for entry in entries[..entries_bytes].chunks_exact(entry_size) {
                    if layout.u16(entry) != wanted {
                        continue;
                    }
                    if found.is_some() {
                        return Err("TIFF contains duplicate scalar mask declarations".into());
                    }
                    let dtype = layout.u16(&entry[2..4]);
                    let count = if layout.big {
                        layout.u64(&entry[4..12])
                    } else {
                        layout.u32(&entry[4..8]) as u64
                    };
                    if count != 1 || !allowed.contains(&dtype) {
                        return Err("TIFF mask scalar declaration is malformed".into());
                    }
                    found = Some(if dtype == 3 {
                        layout.u16(&entry[inline_position..]) as u32
                    } else {
                        layout.u32(&entry[inline_position..])
                    });
                }
                Ok(found)
            };
            let photo = scalar(262, &[3])?.unwrap_or(1);
            let reduced = match scalar(254, &[4]) {
                Ok(v) => v.unwrap_or(0),
                Err(e) if photo == 4 => return Err(e),
                Err(_) => 0,
            };
            let mask = reduced & 4 != 0 || photo == 4;
            if mask
                && (reduced & 4 == 0
                    || photo != 4
                    || scalar(258, &[3])? != Some(1)
                    || scalar(277, &[3])?.unwrap_or(1) != 1)
            {
                return Err(format!("Only a declared one-bit, one-sample TIFF transparency validity mask is supported (type {reduced}, photo {photo}, bits {:?}, samples {:?})",scalar(258,&[3])?,scalar(277,&[3])?));
            }
            for index in 0..count as usize {
                let entry = &entries[index * entry_size..(index + 1) * entry_size];
                let tag = layout.u16(entry);
                let dtype = layout.u16(&entry[2..4]);
                let count = if layout.big {
                    layout.u64(&entry[4..12])
                } else {
                    layout.u32(&entry[4..8]) as u64
                };
                if tag == 262
                    && dtype == 3
                    && count == 1
                    && (layout.u16(&entry[inline_position..]) == 3 || mask && photo == 4)
                {
                    if !mask {
                        has_palette = true;
                    }
                    let replacement = if little {
                        1u16.to_le_bytes()
                    } else {
                        1u16.to_be_bytes()
                    };
                    let position = entries_start + (index * entry_size + inline_position) as u64;
                    patches.insert(position, replacement[0]);
                    patches.insert(position + 1, replacement[1]);
                }
                if tag == 330 {
                    let value_size = match dtype {
                        4 | 13 => 4,
                        16 | 18 => 8,
                        _ => return Err("Unsupported TIFF SubIFD pointer type".into()),
                    };
                    if count > 128 {
                        return Err("TIFF exceeds 128 SubIFD pointers".into());
                    }
                    let bytes = count as usize * value_size;
                    let pointers = if bytes <= inline_size {
                        entry[inline_position..inline_position + bytes].to_vec()
                    } else {
                        let at = layout.offset(&entry[inline_position..]);
                        layout.at(&mut reader, at, bytes)?
                    };
                    for pointer in pointers.chunks_exact(value_size) {
                        pending.push_back(if value_size == 8 {
                            layout.u64(pointer)
                        } else {
                            layout.u32(pointer) as u64
                        });
                    }
                }
            }
            pending.push_back(layout.offset(&entries[entries_bytes..]));
        }
        reader.seek(SeekFrom::Start(0)).map_err(io_error)?;
        Ok(Self {
            reader,
            patches,
            position: 0,
            has_palette,
        })
    }
    pub(super) fn has_palette(&self) -> bool {
        self.has_palette
    }
}

impl<R: Read> Read for RawPaletteReader<R> {
    fn read(&mut self, output: &mut [u8]) -> std::io::Result<usize> {
        let count = self.reader.read(output)?;
        let end = self
            .position
            .checked_add(count as u64)
            .ok_or_else(|| std::io::Error::other("TIFF read position overflow"))?;
        for (position, byte) in self.patches.range(self.position..end) {
            output[(*position - self.position) as usize] = *byte;
        }
        self.position = end;
        Ok(count)
    }
}
impl<R: Seek> Seek for RawPaletteReader<R> {
    fn seek(&mut self, position: SeekFrom) -> std::io::Result<u64> {
        self.position = self.reader.seek(position)?;
        Ok(self.position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    #[test]
    fn little_and_big_endian_classic_and_bigtiff_only_view_palette_tag() {
        for little in [true, false] {
            for big in [true, false] {
                let mut bytes = Vec::new();
                let short = |n: u16| {
                    if little {
                        n.to_le_bytes()
                    } else {
                        n.to_be_bytes()
                    }
                };
                let long = |n: u32| {
                    if little {
                        n.to_le_bytes()
                    } else {
                        n.to_be_bytes()
                    }
                };
                let long8 = |n: u64| {
                    if little {
                        n.to_le_bytes()
                    } else {
                        n.to_be_bytes()
                    }
                };
                bytes.extend(if little { b"II" } else { b"MM" });
                bytes.extend(short(if big { 43 } else { 42 }));
                if big {
                    bytes.extend(short(8));
                    bytes.extend(short(0));
                    bytes.extend(long8(16));
                    bytes.extend(long8(1));
                } else {
                    bytes.extend(long(8));
                    bytes.extend(short(1));
                }
                bytes.extend(short(262));
                bytes.extend(short(3));
                if big {
                    bytes.extend(long8(1));
                } else {
                    bytes.extend(long(1));
                }
                let position = bytes.len();
                bytes.extend(short(3));
                bytes.extend(vec![0; if big { 6 } else { 2 }]);
                if big {
                    bytes.extend(long8(0));
                } else {
                    bytes.extend(long(0));
                }
                bytes.extend([0, 10, 40, 80, 100]);
                let original = bytes.clone();
                let mut view = RawPaletteReader::new(Cursor::new(bytes)).unwrap();
                assert!(view.has_palette());
                let mut read = Vec::new();
                view.read_to_end(&mut read).unwrap();
                let mut expected = original.clone();
                expected[position..position + 2].copy_from_slice(&short(1));
                assert_eq!(read, expected);
                assert_eq!(view.reader.into_inner(), original);
            }
        }
    }

    #[test]
    fn malformed_directory_offsets_are_rejected_before_decoding() {
        assert!(RawPaletteReader::new(Cursor::new(b"II*\0\xff\xff\xff\xff".to_vec())).is_err());
        assert!(
            RawPaletteReader::new(Cursor::new(b"II+\0\x04\0\0\0\0\0\0\0\0\0\0\0".to_vec()))
                .is_err()
        );
        assert!(RawPaletteReader::new(Cursor::new(b"MM\0*\0\0\0\x08\xff\xff".to_vec())).is_err());
    }
}
