//! Bounded standalone video containers. Playback decoding belongs to WebView;
//! native inspection neither transcodes nor extracts or executes attachments.
use serde::Serialize;
use std::{collections::HashMap, io::Cursor};
use symphonia::core::{
    codecs::{
        audio::well_known::{CODEC_ID_AAC, CODEC_ID_OPUS, CODEC_ID_VORBIS},
        video::well_known::{CODEC_ID_H264, CODEC_ID_VP8, CODEC_ID_VP9},
    },
    common::Limit,
    formats::{probe::Hint, FormatOptions, TrackType},
    io::MediaSourceStream,
    meta::MetadataOptions,
    units::TimeBase,
};
pub const VIDEO_BYTES: usize = 8 * 1024 * 1024;
const INVALID: &str = "Choose a valid MP4 (H.264/AAC) or WebM (VP8/VP9, Opus/Vorbis) video.";
const LIMIT: &str = "Choose a video up to 8 MiB, 10 minutes and 4096 pixels per side.";
pub fn mime(extension: &str) -> Option<&'static str> {
    match extension {
        "mp4" => Some("video/mp4"),
        "webm" => Some("video/webm"),
        _ => None,
    }
}
pub fn extension(media_type: &str) -> Option<&'static str> {
    ["mp4", "webm"]
        .into_iter()
        .find(|extension| mime(extension) == Some(media_type))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct VideoInfo {
    pub duration_ms: u64,
    pub width: u16,
    pub height: u16,
    pub has_audio: bool,
    pub codec: &'static str,
}

// Reject out-of-file sizes and excessive nesting before the demuxer reads
// allocation-bearing table/element declarations. Original bytes stay intact.
fn mp4_boxes(
    bytes: &[u8],
    depth: usize,
    budget: &mut usize,
    tracks: &mut usize,
) -> Result<(), String> {
    if depth > 16 {
        return Err(INVALID.into());
    }
    let mut offset = 0usize;
    while offset < bytes.len() {
        *budget += 1;
        if *budget > 100_000 || bytes.len() - offset < 8 {
            return Err(INVALID.into());
        }
        let small = u32::from_be_bytes(bytes[offset..offset + 4].try_into().unwrap());
        let kind: &[u8; 4] = bytes[offset + 4..offset + 8].try_into().unwrap();
        let (size, header) = if small == 1 {
            if bytes.len() - offset < 16 {
                return Err(INVALID.into());
            }
            (
                usize::try_from(u64::from_be_bytes(
                    bytes[offset + 8..offset + 16].try_into().unwrap(),
                ))
                .map_err(|_| INVALID)?,
                16,
            )
        } else {
            (
                if small == 0 {
                    bytes.len() - offset
                } else {
                    small as usize
                },
                8,
            )
        };
        if size < header || size > bytes.len() - offset {
            return Err(INVALID.into());
        }
        let body = &bytes[offset + header..offset + size];
        if kind == b"trak" {
            *tracks += 1;
            if *tracks > 2 {
                return Err(INVALID.into());
            }
        }
        if [b"moov", b"trak", b"mdia", b"minf", b"stbl", b"edts"].contains(&kind) {
            mp4_boxes(body, depth + 1, budget, tracks)?;
        }
        if [
            b"stts", b"ctts", b"stsc", b"stco", b"co64", b"stss", b"stsz",
        ]
        .contains(&kind)
        {
            let index = if kind == b"stsz" { 8 } else { 4 };
            let field = body.get(index..index + 4).ok_or(INVALID)?;
            let count = u32::from_be_bytes(field.try_into().unwrap()) as usize;
            if count > 100_000 {
                return Err(LIMIT.into());
            }
            let step = match kind {
                b"stsc" => 12,
                b"stts" | b"ctts" | b"co64" => 8,
                _ => 4,
            };
            if kind == b"stsz" && body.get(4..8) != Some([0, 0, 0, 0].as_slice()) {
                let sample_size = u32::from_be_bytes(body[4..8].try_into().unwrap()) as usize;
                if sample_size > VIDEO_BYTES
                    || sample_size
                        .checked_mul(count)
                        .is_none_or(|total| total > VIDEO_BYTES)
                {
                    return Err(INVALID.into());
                }
            } else if count > body.len().saturating_sub(index + 4) / step {
                return Err(INVALID.into());
            }
        }
        // Fragmented/encrypted streams are a distinct input contract; do not
        // treat a remote streaming manifest or incomplete segment as a clip.
        if [b"moof", b"mvex", b"sinf", b"pssh"].contains(&kind) {
            return Err(INVALID.into());
        }
        offset += size;
    }
    Ok(())
}
fn ebml_integer(bytes: &[u8], id: bool) -> Result<(u64, usize), String> {
    let first = *bytes.first().ok_or(INVALID)?;
    if first == 0 {
        return Err(INVALID.into());
    }
    let length = first.leading_zeros() as usize + 1;
    if length > if id { 4 } else { 8 } || bytes.len() < length {
        return Err(INVALID.into());
    }
    let mut value = if id {
        u64::from(first)
    } else {
        u64::from(first & ((1 << (8 - length)) - 1))
    };
    for byte in &bytes[1..length] {
        value = (value << 8) | u64::from(*byte);
    }
    if !id && value == (1u64 << (7 * length)) - 1 {
        return Err(INVALID.into());
    }
    Ok((value, length))
}
fn ebml_elements(
    bytes: &[u8],
    depth: usize,
    budget: &mut usize,
    tracks: &mut usize,
    webm: &mut bool,
) -> Result<(), String> {
    if depth > 16 {
        return Err(INVALID.into());
    }
    let mut offset = 0usize;
    while offset < bytes.len() {
        *budget += 1;
        if *budget > 100_000 {
            return Err(LIMIT.into());
        }
        let (id, id_len) = ebml_integer(&bytes[offset..], true)?;
        let (size, size_len) = ebml_integer(&bytes[offset + id_len..], false)?;
        let start = offset + id_len + size_len;
        let size = usize::try_from(size).map_err(|_| INVALID)?;
        if size > bytes.len() - start {
            return Err(INVALID.into());
        }
        let body = &bytes[start..start + size];
        if id == 0x4282 {
            if body != b"webm" {
                return Err(INVALID.into());
            }
            *webm = true;
        }
        if id == 0xae {
            *tracks += 1;
            if *tracks > 2 {
                return Err(INVALID.into());
            }
        }
        // Disallow compressed/encrypted tracks and embedded non-video files.
        if [0x6d80, 0x6240, 0x1941a469].contains(&id) {
            return Err(INVALID.into());
        }
        if [
            0x1a45dfa3, 0x18538067, 0x1549a966, 0x1654ae6b, 0xae, 0xe0, 0xe1, 0x1f43b675, 0xa0,
            0x114d9b74, 0x4dbb, 0x1c53bb6b, 0xbb, 0xb7, 0x1254c367, 0x7373, 0x63c0, 0x67c8,
            0x1043a770, 0x45b9, 0xb6, 0x80,
        ]
        .contains(&id)
        {
            ebml_elements(body, depth + 1, budget, tracks, webm)?;
        }
        offset = start + size;
    }
    Ok(())
}
pub fn inspect(bytes: &[u8], extension: &str) -> Result<VideoInfo, String> {
    if bytes.is_empty() || bytes.len() > VIDEO_BYTES {
        return Err(LIMIT.into());
    }
    let mut budget = 0usize;
    let mut count = 0usize;
    match extension {
        "mp4" => {
            if bytes.get(4..8) != Some(b"ftyp") {
                return Err(INVALID.into());
            }
            mp4_boxes(bytes, 0, &mut budget, &mut count)?;
        }
        "webm" => {
            if !bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
                return Err(INVALID.into());
            }
            let mut webm = false;
            ebml_elements(bytes, 0, &mut budget, &mut count, &mut webm)?;
            if !webm {
                return Err(INVALID.into());
            }
        }
        _ => return Err(INVALID.into()),
    }
    let mut hint = Hint::new();
    hint.with_extension(extension);
    let metadata = MetadataOptions::default()
        .limit_tag_bytes(Limit::Maximum(4096))
        .limit_visual_bytes(Limit::Maximum(0));
    let mut format = symphonia::default::get_probe()
        .probe(
            &hint,
            MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default()),
            FormatOptions::default(),
            metadata,
        )
        .map_err(|_| INVALID)?;
    let tracks = format.tracks();
    if tracks.is_empty() || tracks.len() > 2 {
        return Err(INVALID.into());
    }
    let video = tracks
        .iter()
        .filter(|track| track.track_type() == Some(TrackType::Video))
        .collect::<Vec<_>>();
    if video.len() != 1
        || tracks.iter().any(|track| {
            !matches!(
                track.track_type(),
                Some(TrackType::Video | TrackType::Audio)
            )
        })
    {
        return Err(INVALID.into());
    }
    let video = video[0];
    let params = video
        .codec_params
        .as_ref()
        .and_then(|params| params.video())
        .ok_or(INVALID)?;
    let codec = match (extension, params.codec) {
        ("mp4", CODEC_ID_H264) => "H.264",
        ("webm", CODEC_ID_VP8) => "VP8",
        ("webm", CODEC_ID_VP9) => "VP9",
        _ => return Err(INVALID.into()),
    };
    let width = params.width.ok_or(INVALID)?;
    let height = params.height.ok_or(INVALID)?;
    if width == 0 || height == 0 || width > 4096 || height > 4096 {
        return Err(LIMIT.into());
    }
    let video_id = video.id;
    let mut times = HashMap::<u32, TimeBase>::new();
    let mut duration_ms = 0u64;
    for track in tracks {
        let base = track.time_base.ok_or(INVALID)?;
        times.insert(track.id, base);
        if let Some(duration) = track.duration {
            let time = base.calc_duration(duration).ok_or(INVALID)?.as_nanos();
            if !(0..=600_000_000_000).contains(&time) {
                return Err(LIMIT.into());
            }
            duration_ms = duration_ms.max(
                u64::try_from(time)
                    .map_err(|_| INVALID)?
                    .div_ceil(1_000_000),
            );
        }
        if track.track_type() == Some(TrackType::Audio) {
            let audio = track
                .codec_params
                .as_ref()
                .and_then(|params| params.audio())
                .ok_or(INVALID)?;
            if match extension {
                "mp4" => audio.codec != CODEC_ID_AAC,
                _ => ![CODEC_ID_OPUS, CODEC_ID_VORBIS].contains(&audio.codec),
            } || audio
                .sample_rate
                .is_some_and(|rate| !(8000..=192000).contains(&rate))
                || audio
                    .channels
                    .as_ref()
                    .is_some_and(|channels| !(1..=8).contains(&channels.count()))
            {
                return Err(INVALID.into());
            }
        }
    }
    let has_audio = tracks.len() == 2;
    let mut packets = 0usize;
    let mut video_packets = 0usize;
    while let Some(packet) = format.next_packet().map_err(|_| INVALID)? {
        packets += 1;
        if packets > 100_000 {
            return Err(LIMIT.into());
        }
        let base = times.get(&packet.track_id).ok_or(INVALID)?;
        if packet.data.is_empty() || packet.data.len() > bytes.len() {
            return Err(INVALID.into());
        }
        let end = packet.pts.checked_add(packet.dur).ok_or(INVALID)?;
        let time = base.calc_time(end).ok_or(INVALID)?.as_nanos();
        if !(-1_000_000_000..=600_000_000_000).contains(&time) {
            return Err(LIMIT.into());
        }
        duration_ms = duration_ms.max(
            u64::try_from(time.max(0))
                .map_err(|_| INVALID)?
                .div_ceil(1_000_000),
        );
        if packet.track_id == video_id {
            video_packets += 1;
        }
    }
    if video_packets == 0 || duration_ms == 0 {
        return Err(INVALID.into());
    }
    Ok(VideoInfo {
        duration_ms,
        width,
        height,
        has_audio,
        codec,
    })
}
