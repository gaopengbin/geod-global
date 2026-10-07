//! Decode user-selected bounded clips without playback, upload, transcription
//! or transcoding. Original bytes remain the attachment sent to the model.
use serde::Serialize;
use std::io::Cursor;
use symphonia::core::{
    codecs::audio::{
        well_known::{CODEC_ID_FLAC, CODEC_ID_MP3, CODEC_ID_VORBIS},
        AudioDecoderOptions,
    },
    common::Limit,
    formats::{probe::Hint, FormatOptions, TrackType},
    io::MediaSourceStream,
    meta::MetadataOptions,
};
pub const AUDIO_BYTES: usize = 2 * 1024 * 1024;
const INVALID: &str = "Choose a valid WAV, MP3, FLAC or OGG Vorbis audio file.";
const LIMIT: &str = "Choose an audio clip up to 2 MiB and 10 minutes.";
pub fn mime(extension: &str) -> Option<&'static str> {
    match extension {
        "wav" => Some("audio/wav"),
        "mp3" => Some("audio/mpeg"),
        "flac" => Some("audio/flac"),
        "ogg" => Some("audio/ogg"),
        _ => None,
    }
}
pub fn extension(media_type: &str) -> Option<&'static str> {
    ["wav", "mp3", "flac", "ogg"]
        .into_iter()
        .find(|name| mime(name) == Some(media_type))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AudioInfo {
    pub duration_ms: u64,
    pub sample_rate: u32,
    pub channels: usize,
}
pub fn inspect(bytes: &[u8], extension: &str) -> Result<AudioInfo, String> {
    if bytes.is_empty() || bytes.len() > AUDIO_BYTES {
        return Err(LIMIT.into());
    }
    let signature = match extension {
        "wav" => bytes.starts_with(b"RIFF") && bytes.get(8..12) == Some(b"WAVE".as_slice()),
        "mp3" => {
            bytes.starts_with(b"ID3")
                || bytes.len() > 1 && bytes[0] == 0xff && bytes[1] & 0xe0 == 0xe0
        }
        "flac" => bytes.starts_with(b"fLaC"),
        "ogg" => bytes.starts_with(b"OggS"),
        _ => false,
    };
    if !signature {
        return Err(INVALID.into());
    }
    let stream = MediaSourceStream::new(Box::new(Cursor::new(bytes.to_vec())), Default::default());
    let mut hint = Hint::new();
    hint.with_extension(extension);
    let metadata = MetadataOptions::default()
        .limit_tag_bytes(Limit::Maximum(4096))
        .limit_visual_bytes(Limit::Maximum(0));
    let mut format = symphonia::default::get_probe()
        .probe(&hint, stream, FormatOptions::default(), metadata)
        .map_err(|_| INVALID)?;
    if format.tracks().len() != 1 {
        return Err(INVALID.into());
    }
    let track = format.default_track(TrackType::Audio).ok_or(INVALID)?;
    let track_id = track.id;
    let parameters = track
        .codec_params
        .as_ref()
        .and_then(|parameters| parameters.audio())
        .ok_or(INVALID)?;
    if match extension {
        "ogg" => parameters.codec != CODEC_ID_VORBIS,
        "mp3" => parameters.codec != CODEC_ID_MP3,
        "flac" => parameters.codec != CODEC_ID_FLAC,
        _ => false,
    } || parameters
        .sample_rate
        .is_some_and(|rate| !(8000..=192000).contains(&rate))
        || parameters
            .channels
            .as_ref()
            .is_some_and(|channels| !(1..=8).contains(&channels.count()))
        || parameters
            .max_frames_per_packet
            .is_some_and(|frames| frames > 192000)
    {
        return Err(INVALID.into());
    }
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(parameters, &AudioDecoderOptions::default())
        .map_err(|_| INVALID)?;
    let mut frames = 0u64;
    let mut rate = 0u32;
    let mut channels = 0usize;
    let mut packets = 0usize;
    while let Some(packet) = format.next_packet().map_err(|_| INVALID)? {
        packets += 1;
        if packets > 100_000 {
            return Err(LIMIT.into());
        }
        if packet.track_id != track_id {
            return Err(INVALID.into());
        }
        let decoded = decoder.decode(&packet).map_err(|_| INVALID)?;
        let next_rate = decoded.spec().rate();
        let next_channels = decoded.spec().channels().count();
        if !(8000..=192000).contains(&next_rate) || !(1..=8).contains(&next_channels) {
            return Err(INVALID.into());
        }
        if rate != 0 && (rate != next_rate || channels != next_channels) {
            return Err(INVALID.into());
        }
        rate = next_rate;
        channels = next_channels;
        frames = frames.checked_add(decoded.frames() as u64).ok_or(LIMIT)?;
        if frames > u64::from(rate) * 600 {
            return Err(LIMIT.into());
        }
    }
    if rate == 0 || frames == 0 {
        return Err(INVALID.into());
    }
    Ok(AudioInfo {
        duration_ms: (frames * 1000).div_ceil(u64::from(rate)),
        sample_rate: rate,
        channels,
    })
}
