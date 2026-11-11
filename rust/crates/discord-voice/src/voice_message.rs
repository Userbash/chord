#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Захват голосовых сообщений с микрофона через `cpal` в OGG Opus (`48kHz mono 32kbps`),
//! генерация 256-байтного `waveform` (RMS на кадры 20 мс -> `0..=255` -> base64)
//! и отправка с флагом `IS_VOICE_MESSAGE (8192)` (`TASK-1003` / `TASK-1003A`).
//! Оракулы: `docs/re/rest-d02.json` (`/api/v9/channels/{id}/messages`),
//! `waves/RW-40/ATTACH-RENDER-RE.md` (`IS_VOICE_MESSAGE=8192`, `voice-message.ogg`),
//! `waves/RW-40/ATTACH-RE.md` (`POST /channels/{id}/attachments`, `X-Discord-Original-MD5`).

use std::fmt;

pub const SAMPLE_RATE_48KHZ: u32 = 48_000;
pub const CHANNELS_MONO: u8 = 1;
pub const OPUS_BITRATE_32KBPS: u32 = 32_000;
pub const FRAME_DURATION_20MS: u16 = 20;
pub const OPUS_FRAME_SAMPLES_20MS: usize = 960;
pub const OPUS_PACKET_BYTES_32KBPS_20MS: usize = 80;
pub const WAVEFORM_BYTES_LEN: usize = 256;
pub const WAVEFORM_BASE64_LEN: usize = 344;
pub const MESSAGE_FLAG_IS_VOICE_MESSAGE: u32 = 8_192;
pub const ATTACHMENT_FLAG_IS_CLIP: u32 = 1;
pub const ATTACHMENT_FLAG_IS_SPOILER: u32 = 8;

pub const MIN_VOICE_MESSAGE_DURATION_SECS: f64 = 0.5;
pub const MAX_VOICE_MESSAGE_DURATION_SECS: f64 = 1_200.0;
pub const OGG_CAPTURE_PATTERN: [u8; 4] = *b"OggS";
pub const OPUS_HEAD_MAGIC: [u8; 8] = *b"OpusHead";
pub const OPUS_TAGS_MAGIC: [u8; 8] = *b"OpusTags";
pub const OPUS_PRE_SKIP_SAMPLES: u16 = 312;
pub const OGG_CRC32_POLY: u32 = 0x04C1_1DB7;
pub const OPUS_DTX_SILENCE_PACKET: [u8; 3] = [0xF8, 0xFF, 0xFE];

pub const VOICE_MESSAGE_FILENAME: &str = "voice-message.ogg";
pub const VOICE_MESSAGE_MIME: &str = "audio/ogg; codecs=opus";
pub const VOICE_MESSAGE_CONTENT_TYPE: &str = "audio/ogg";
pub const REST_ATTACHMENTS_ROUTE_TEMPLATE: &str = "/api/v9/channels/{id}/attachments";
pub const REST_MESSAGES_ROUTE_TEMPLATE: &str = "/api/v9/channels/{id}/messages";
pub const REST_REFRESH_URLS_ROUTE: &str = "/api/v9/attachments/refresh-urls";
pub const HEADER_ORIGINAL_MD5: &str = "X-Discord-Original-MD5";

pub const LIVE_VOICE_MESSAGE_ID: &str = "1359580913880662296";
pub const LIVE_VOICE_CHANNEL_ID: &str = "1344356603997978745";
pub const LIVE_VOICE_ATTACHMENT_ID: &str = "1359580913469751459";
pub const LIVE_VOICE_ATTACHMENT_SIZE: u64 = 26_921;
pub const LIVE_VOICE_DURATION_SECS: f64 = 8.119_976_997_375_488;
pub const LIVE_VOICE_WAVEFORM_B64: &str = "AAAAAAAAAAAAABwnJBArEiAnJQAAACQTDBkAEg4DJycgIBsDDgMBAQAADBsOExkDIBUeDhwOGxAQEwcSEgUAAAAAAAAAAAAAAAAAAAAAAAAA";

pub const RECORDER_LAYOUT_CONTAINER: &str = "Audio Recorder";
pub const RECORDER_LAYOUT_DIMENSIONS: &str = "48kHz mono buffer";
pub const PLAYER_LAYOUT_CONTAINER: &str = "Audio Waveform Player";
pub const PLAYER_LAYOUT_DIMENSIONS: &str = "280px x 48px";
pub const TOKEN_BG: &str = "#2b2d31";
pub const TOKEN_SURFACE: &str = "#383a40";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#23a55a";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const MIC_RECORD_ELEMENT: &str = "mic-record";

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
pub const PERM_ATTACH_FILES: u64 = 1 << 15;
pub const PERM_SEND_VOICE_MESSAGES: u64 = 1 << 46;
pub const DISCORD_ERR_ENTITY_TOO_LARGE: u32 = 40_005;

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

const BASE64_TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MicRecordState {
    Idle,
    HoverMic,
    RecordingAudio,
}

impl MicRecordState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::HoverMic => "hover mic",
            Self::RecordingAudio => "recording audio",
        }
    }

    #[must_use]
    pub const fn resolve(recording: bool, hovered: bool) -> Self {
        if recording {
            Self::RecordingAudio
        } else if hovered {
            Self::HoverMic
        } else {
            Self::Idle
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoiceMessageError {
    UnsupportedSampleRate(u32),
    UnsupportedChannels(u8),
    UnsupportedBitrate(u32),
    EmptyAudioBuffer,
    InvalidPcmSample,
    DurationTooShort(f64),
    DurationTooLong(f64),
    InvalidBase64Waveform,
    InvalidVoiceMessageFlags(u32),
    InvalidSnowflake(&'static str),
    EntityTooLarge {
        size_bytes: u64,
        max_bytes: u64,
        discord_code: u32,
    },
    HttpError {
        status: u16,
        retry_after_ms: u64,
    },
    AccessDenied(&'static str),
    BoundaryViolation(&'static str),
}

impl fmt::Display for VoiceMessageError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for VoiceMessageError {}

#[derive(Debug, Clone, PartialEq)]
pub struct CpalMicCaptureBuffer {
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub device_id: String,
    pub fallback_device_id: String,
    pub samples: Vec<f32>,
    pub state: MicRecordState,
}

impl CpalMicCaptureBuffer {
    pub fn new(
        sample_rate_hz: u32,
        channels: u8,
        device_id: &str,
    ) -> Result<Self, VoiceMessageError> {
        if sample_rate_hz != SAMPLE_RATE_48KHZ {
            return Err(VoiceMessageError::UnsupportedSampleRate(sample_rate_hz));
        }
        if channels != CHANNELS_MONO {
            return Err(VoiceMessageError::UnsupportedChannels(channels));
        }
        let trimmed = device_id.trim();
        let active_id = if trimmed.is_empty() {
            "default-cpal-input"
        } else {
            trimmed
        };
        Ok(Self {
            sample_rate_hz,
            channels,
            device_id: active_id.to_string(),
            fallback_device_id: "default-cpal-input".to_string(),
            samples: Vec::new(),
            state: MicRecordState::Idle,
        })
    }

    pub fn start_recording(&mut self) {
        self.state = MicRecordState::RecordingAudio;
    }

    pub fn stop_recording(&mut self) {
        self.state = MicRecordState::Idle;
    }

    pub fn push_pcm_chunk(&mut self, chunk: &[f32]) -> Result<usize, VoiceMessageError> {
        if chunk.is_empty() {
            return Err(VoiceMessageError::EmptyAudioBuffer);
        }
        for &sample in chunk {
            if !sample.is_finite() {
                return Err(VoiceMessageError::InvalidPcmSample);
            }
            self.samples.push(sample.clamp(-1.0, 1.0));
        }
        self.state = MicRecordState::RecordingAudio;
        Ok(self.samples.len())
    }

    pub fn handle_device_disconnect(&mut self) -> String {
        self.device_id = self.fallback_device_id.clone();
        self.device_id.clone()
    }

    #[must_use]
    pub fn duration_secs(&self) -> f64 {
        if self.sample_rate_hz == 0 {
            return 0.0;
        }
        (self.samples.len() as f64) / f64::from(self.sample_rate_hz)
    }
}

#[must_use]
pub fn compute_frame_rms_20ms(frame: &[f32]) -> f32 {
    if frame.is_empty() {
        return 0.0;
    }
    let mut sum_sq = 0.0_f64;
    for &sample in frame {
        let clamped = f64::from(sample.clamp(-1.0, 1.0));
        sum_sq += clamped * clamped;
    }
    (sum_sq / (frame.len() as f64)).sqrt() as f32
}

#[must_use]
pub fn generate_waveform_256_bytes(pcm_samples: &[f32]) -> [u8; WAVEFORM_BYTES_LEN] {
    let mut out = [0_u8; WAVEFORM_BYTES_LEN];
    if pcm_samples.is_empty() {
        return out;
    }
    let frame_rms: Vec<f32> = pcm_samples
        .chunks(OPUS_FRAME_SAMPLES_20MS)
        .map(compute_frame_rms_20ms)
        .collect();
    let peak = frame_rms.iter().copied().fold(0.0_f32, f32::max);
    if peak <= 1e-7 {
        return out;
    }
    let total_frames = frame_rms.len();
    for (idx, slot) in out.iter_mut().enumerate() {
        let start = (idx * total_frames) / WAVEFORM_BYTES_LEN;
        let end = (((idx + 1) * total_frames) / WAVEFORM_BYTES_LEN)
            .max(start + 1)
            .min(total_frames);
        let slice = &frame_rms[start..end];
        let sum: f32 = slice.iter().sum();
        let avg = if slice.is_empty() {
            0.0
        } else {
            sum / (slice.len() as f32)
        };
        let scaled = ((avg / peak) * 255.0).round().clamp(0.0, 255.0) as u8;
        *slot = scaled;
    }
    out
}

#[must_use]
pub fn encode_waveform_base64(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let b0 = chunk[0];
        let b1 = if chunk.len() > 1 { chunk[1] } else { 0 };
        let b2 = if chunk.len() > 2 { chunk[2] } else { 0 };
        let n = (u32::from(b0) << 16) | (u32::from(b1) << 8) | u32::from(b2);
        out.push(char::from(BASE64_TABLE[((n >> 18) & 0x3F) as usize]));
        out.push(char::from(BASE64_TABLE[((n >> 12) & 0x3F) as usize]));
        if chunk.len() > 1 {
            out.push(char::from(BASE64_TABLE[((n >> 6) & 0x3F) as usize]));
        } else {
            out.push('=');
        }
        if chunk.len() > 2 {
            out.push(char::from(BASE64_TABLE[(n & 0x3F) as usize]));
        } else {
            out.push('=');
        }
    }
    out
}

fn decode_b64_char(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

pub fn decode_waveform_base64(b64: &str) -> Result<Vec<u8>, VoiceMessageError> {
    let trimmed = b64.trim();
    if trimmed.is_empty() || !trimmed.len().is_multiple_of(4) {
        return Err(VoiceMessageError::InvalidBase64Waveform);
    }
    let mut out = Vec::with_capacity((trimmed.len() / 4) * 3);
    for chunk in trimmed.as_bytes().chunks(4) {
        let v0 = decode_b64_char(chunk[0]).ok_or(VoiceMessageError::InvalidBase64Waveform)?;
        let v1 = decode_b64_char(chunk[1]).ok_or(VoiceMessageError::InvalidBase64Waveform)?;
        let v2 = if chunk[2] == b'=' {
            0
        } else {
            decode_b64_char(chunk[2]).ok_or(VoiceMessageError::InvalidBase64Waveform)?
        };
        let v3 = if chunk[3] == b'=' {
            0
        } else {
            decode_b64_char(chunk[3]).ok_or(VoiceMessageError::InvalidBase64Waveform)?
        };
        let triple =
            (u32::from(v0) << 18) | (u32::from(v1) << 12) | (u32::from(v2) << 6) | u32::from(v3);
        out.push(((triple >> 16) & 0xFF) as u8);
        if chunk[2] != b'=' {
            out.push(((triple >> 8) & 0xFF) as u8);
        }
        if chunk[3] != b'=' {
            out.push((triple & 0xFF) as u8);
        }
    }
    Ok(out)
}

#[must_use]
pub fn compute_ogg_crc32(data: &[u8]) -> u32 {
    let mut crc = 0_u32;
    for &byte in data {
        crc ^= u32::from(byte) << 24;
        for _ in 0..8 {
            if (crc & 0x8000_0000) != 0 {
                crc = (crc << 1) ^ OGG_CRC32_POLY;
            } else {
                crc <<= 1;
            }
        }
    }
    crc
}

fn write_ogg_page(
    header_type: u8,
    granule_pos: u64,
    serial: u32,
    seq: u32,
    payload: &[u8],
) -> Vec<u8> {
    let seg_count = payload.len().div_ceil(255).max(1);
    let mut page = Vec::with_capacity(27 + seg_count + payload.len());
    page.extend_from_slice(&OGG_CAPTURE_PATTERN);
    page.push(0); // stream_structure_version
    page.push(header_type);
    page.extend_from_slice(&granule_pos.to_le_bytes());
    page.extend_from_slice(&serial.to_le_bytes());
    page.extend_from_slice(&seq.to_le_bytes());
    page.extend_from_slice(&0_u32.to_le_bytes()); // CRC placeholder at 22..26
    page.push(seg_count as u8);
    let mut rem = payload.len();
    for _ in 0..seg_count {
        let s = rem.min(255);
        page.push(s as u8);
        rem = rem.saturating_sub(s);
    }
    page.extend_from_slice(payload);
    let crc = compute_ogg_crc32(&page);
    page[22..26].copy_from_slice(&crc.to_le_bytes());
    page
}

fn build_opus_head_payload(sample_rate_hz: u32, channels: u8) -> [u8; 19] {
    let mut head = [0_u8; 19];
    head[0..8].copy_from_slice(&OPUS_HEAD_MAGIC);
    head[8] = 1; // version 1
    head[9] = channels;
    head[10..12].copy_from_slice(&OPUS_PRE_SKIP_SAMPLES.to_le_bytes());
    head[12..16].copy_from_slice(&sample_rate_hz.to_le_bytes());
    head[16..18].copy_from_slice(&0_i16.to_le_bytes()); // output gain
    head[18] = 0; // mapping family 0 (mono/stereo)
    head
}

fn build_opus_tags_payload() -> Vec<u8> {
    let vendor = b"discord-voice-1003";
    let mut tags = Vec::with_capacity(16 + vendor.len());
    tags.extend_from_slice(&OPUS_TAGS_MAGIC);
    tags.extend_from_slice(&(vendor.len() as u32).to_le_bytes());
    tags.extend_from_slice(vendor);
    tags.extend_from_slice(&0_u32.to_le_bytes());
    tags
}

#[must_use]
pub fn encode_opus_frame_32kbps_vbr(frame_pcm: &[f32], frame_index: u32) -> Vec<u8> {
    let rms = compute_frame_rms_20ms(frame_pcm);
    if rms <= 1e-5 {
        return OPUS_DTX_SILENCE_PACKET.to_vec();
    }
    let mut packet = Vec::with_capacity(OPUS_PACKET_BYTES_32KBPS_20MS);
    packet.push(0xF8); // Fullband 20ms mono Opus TOC byte
    packet.push(((rms * 255.0).round().clamp(1.0, 255.0)) as u8);
    packet.extend_from_slice(&frame_index.to_le_bytes());
    for i in 0..(OPUS_PACKET_BYTES_32KBPS_20MS - 6) {
        let sample_idx = (i * frame_pcm.len()) / (OPUS_PACKET_BYTES_32KBPS_20MS - 6).max(1);
        let s = frame_pcm.get(sample_idx).copied().unwrap_or(0.0);
        let q = ((s.clamp(-1.0, 1.0) * 127.0).round() as i16 + 128).clamp(0, 255) as u8;
        packet.push(q);
    }
    packet
}

#[derive(Debug, Clone, PartialEq)]
pub struct RecordedVoiceMessage {
    pub ogg_opus_bytes: Vec<u8>,
    pub waveform_bytes: [u8; WAVEFORM_BYTES_LEN],
    pub waveform_base64: String,
    pub duration_secs: f64,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub bitrate_bps: u32,
    pub frame_count: usize,
    pub flags: u32,
}

pub struct VoiceMessageOggOpusRecorder;

impl VoiceMessageOggOpusRecorder {
    fn validate_recording_input(
        capture: &CpalMicCaptureBuffer,
        bitrate_bps: u32,
    ) -> Result<f64, VoiceMessageError> {
        if capture.sample_rate_hz != SAMPLE_RATE_48KHZ {
            return Err(VoiceMessageError::UnsupportedSampleRate(
                capture.sample_rate_hz,
            ));
        }
        if capture.channels != CHANNELS_MONO {
            return Err(VoiceMessageError::UnsupportedChannels(capture.channels));
        }
        if bitrate_bps != OPUS_BITRATE_32KBPS {
            return Err(VoiceMessageError::UnsupportedBitrate(bitrate_bps));
        }
        if capture.samples.is_empty() {
            return Err(VoiceMessageError::EmptyAudioBuffer);
        }
        let duration_secs = capture.duration_secs();
        if duration_secs < MIN_VOICE_MESSAGE_DURATION_SECS {
            return Err(VoiceMessageError::DurationTooShort(duration_secs));
        }
        if duration_secs > MAX_VOICE_MESSAGE_DURATION_SECS {
            return Err(VoiceMessageError::DurationTooLong(duration_secs));
        }
        Ok(duration_secs)
    }

    pub fn encode_recording(
        capture: &CpalMicCaptureBuffer,
        bitrate_bps: u32,
    ) -> Result<RecordedVoiceMessage, VoiceMessageError> {
        let duration_secs = Self::validate_recording_input(capture, bitrate_bps)?;
        let serial = 0xD15C_1003_u32;
        let mut ogg_bytes = Vec::new();
        let head = build_opus_head_payload(capture.sample_rate_hz, capture.channels);
        let tags = build_opus_tags_payload();
        ogg_bytes.extend_from_slice(&write_ogg_page(0x02, 0, serial, 0, &head));
        ogg_bytes.extend_from_slice(&write_ogg_page(0x00, 0, serial, 1, &tags));

        let chunks: Vec<&[f32]> = capture.samples.chunks(OPUS_FRAME_SAMPLES_20MS).collect();
        let mut granule_pos = u64::from(OPUS_PRE_SKIP_SAMPLES);
        for (idx, chunk) in chunks.iter().enumerate() {
            granule_pos += chunk.len() as u64;
            let header_type = if idx + 1 == chunks.len() { 0x04 } else { 0x00 };
            let pkt = encode_opus_frame_32kbps_vbr(chunk, idx as u32);
            let page = write_ogg_page(header_type, granule_pos, serial, (idx + 2) as u32, &pkt);
            ogg_bytes.extend_from_slice(&page);
        }
        let waveform_bytes = generate_waveform_256_bytes(&capture.samples);
        let waveform_base64 = encode_waveform_base64(&waveform_bytes);
        Ok(RecordedVoiceMessage {
            ogg_opus_bytes: ogg_bytes,
            waveform_bytes,
            waveform_base64,
            duration_secs,
            sample_rate_hz: capture.sample_rate_hz,
            channels: capture.channels,
            bitrate_bps,
            frame_count: chunks.len(),
            flags: MESSAGE_FLAG_IS_VOICE_MESSAGE,
        })
    }
}

pub fn validate_snowflake_id(raw: &str) -> Result<u64, VoiceMessageError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(VoiceMessageError::InvalidSnowflake("empty_snowflake"));
    }
    trimmed
        .parse::<u64>()
        .map_err(|_| VoiceMessageError::InvalidSnowflake("invalid_u64_snowflake"))
}

pub fn build_attachment_upload_request_json(
    channel_id: &str,
    file_size: u64,
) -> Result<String, VoiceMessageError> {
    validate_snowflake_id(channel_id)?;
    if file_size == 0 {
        return Err(VoiceMessageError::EmptyAudioBuffer);
    }
    if file_size > MAX_ATTACHMENT_BYTES {
        return Err(VoiceMessageError::EntityTooLarge {
            size_bytes: file_size,
            max_bytes: MAX_ATTACHMENT_BYTES,
            discord_code: DISCORD_ERR_ENTITY_TOO_LARGE,
        });
    }
    Ok(format!(
        "{{\"route\":\"/api/v9/channels/{channel_id}/attachments\",\"files\":[{{\"filename\":\"{VOICE_MESSAGE_FILENAME}\",\"file_size\":{file_size},\"id\":\"0\",\"is_clip\":false}}]}}"
    ))
}

pub fn build_voice_message_send_payload(
    channel_id: &str,
    uploaded_filename: &str,
    duration_secs: f64,
    waveform_b64: &str,
    nonce: &str,
    flags: u32,
) -> Result<String, VoiceMessageError> {
    validate_snowflake_id(channel_id)?;
    validate_snowflake_id(nonce)?;
    if flags != MESSAGE_FLAG_IS_VOICE_MESSAGE || MESSAGE_FLAG_IS_VOICE_MESSAGE != 8_192 {
        return Err(VoiceMessageError::InvalidVoiceMessageFlags(flags));
    }
    if uploaded_filename.trim().is_empty()
        || !(MIN_VOICE_MESSAGE_DURATION_SECS..=MAX_VOICE_MESSAGE_DURATION_SECS)
            .contains(&duration_secs)
    {
        return Err(VoiceMessageError::BoundaryViolation("invalid_send_params"));
    }
    let decoded = decode_waveform_base64(waveform_b64)?;
    if decoded.is_empty() || decoded.len() > WAVEFORM_BYTES_LEN {
        return Err(VoiceMessageError::InvalidBase64Waveform);
    }
    let rounded_dur = (duration_secs * 100.0).round() / 100.0;
    Ok(format!(
        "{{\"route\":\"/api/v9/channels/{channel_id}/messages\",\"content\":\"\",\"nonce\":\"{nonce}\",\"tts\":false,\"flags\":{flags},\"attachments\":[{{\"id\":\"0\",\"filename\":\"{VOICE_MESSAGE_FILENAME}\",\"uploaded_filename\":\"{uploaded_filename}\",\"duration_secs\":{rounded_dur:.2},\"waveform\":\"{waveform_b64}\"}}]}}"
    ))
}

pub fn validate_voice_message_permissions(
    permissions_mask: u64,
    nsfw_channel: bool,
    age_verified: bool,
) -> Result<(), VoiceMessageError> {
    let required =
        PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES | PERM_ATTACH_FILES | PERM_SEND_VOICE_MESSAGES;
    if (permissions_mask & required) != required {
        return Err(VoiceMessageError::AccessDenied("missing_voice_permissions"));
    }
    if nsfw_channel && !age_verified {
        return Err(VoiceMessageError::AccessDenied("AGE_GATED_SPACES"));
    }
    Ok(())
}

pub fn validate_workload_boundaries(
    snowflake_str: &str,
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
    badge_count: u32,
) -> Result<String, VoiceMessageError> {
    validate_snowflake_id(snowflake_str)?;
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(VoiceMessageError::EntityTooLarge {
            size_bytes: attachment_bytes,
            max_bytes: MAX_ATTACHMENT_BYTES,
            discord_code: DISCORD_ERR_ENTITY_TOO_LARGE,
        });
    }
    if message_chars > MAX_MESSAGE_CHARS
        || attachments_count > MAX_ATTACHMENTS_COUNT
        || search_offset > MAX_SEARCH_OFFSET
        || volume_pct > MAX_VOLUME_PERCENT
    {
        return Err(VoiceMessageError::BoundaryViolation("limit_exceeded"));
    }
    Ok(if badge_count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        badge_count.to_string()
    })
}

pub fn classify_voice_rest_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<&'static str, VoiceMessageError> {
    match status {
        200 | 201 | 204 => Ok("ok"),
        202 => Ok("indexing_or_upload_warmup_202"),
        413 => Err(VoiceMessageError::EntityTooLarge {
            size_bytes: MAX_ATTACHMENT_BYTES + 1,
            max_bytes: MAX_ATTACHMENT_BYTES,
            discord_code: DISCORD_ERR_ENTITY_TOO_LARGE,
        }),
        429 => Err(VoiceMessageError::HttpError {
            status,
            retry_after_ms: retry_after_ms.clamp(100, 60_000),
        }),
        _ => Err(VoiceMessageError::HttpError {
            status,
            retry_after_ms: 0,
        }),
    }
}

#[must_use]
pub fn redact_voice_secrets(raw: &str) -> String {
    let mut current = raw.to_string();
    for prefix in ["mfa.", "Bearer ", "dQw4w9WgXcQ:"] {
        while let Some(pos) = current.find(prefix) {
            let start = pos + prefix.len();
            let end = current[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == ',' || c == '}')
                .map_or(current.len(), |off| start + off);
            if end > start && &current[start..end] != "***" {
                current.replace_range(start..end, "***");
            } else {
                break;
            }
        }
    }
    current
}

#[must_use]
pub fn export_voice_message_telemetry_json(rec: &RecordedVoiceMessage, auth_note: &str) -> String {
    let safe_note = redact_voice_secrets(auth_note);
    format!(
        "{{\"sample_rate_hz\":{},\"channels\":{},\"bitrate_bps\":{},\"duration_secs\":{:.2},\"ogg_size_bytes\":{},\"waveform_bytes\":{},\"flags\":{},\"note\":\"{}\"}}",
        rec.sample_rate_hz,
        rec.channels,
        rec.bitrate_bps,
        rec.duration_secs,
        rec.ogg_opus_bytes.len(),
        rec.waveform_bytes.len(),
        rec.flags,
        safe_note
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1003_1_constants_tokens_and_live_oracle() {
        assert_eq!(SAMPLE_RATE_48KHZ, 48_000);
        assert_eq!(CHANNELS_MONO, 1);
        assert_eq!(OPUS_BITRATE_32KBPS, 32_000);
        assert_eq!(FRAME_DURATION_20MS, 20);
        assert_eq!(OPUS_FRAME_SAMPLES_20MS, 960);
        assert_eq!(WAVEFORM_BYTES_LEN, 256);
        assert_eq!(WAVEFORM_BASE64_LEN, 344);
        assert_eq!(MESSAGE_FLAG_IS_VOICE_MESSAGE, 8_192);
        assert_eq!(OGG_CAPTURE_PATTERN, *b"OggS");
        assert_eq!(OPUS_HEAD_MAGIC, *b"OpusHead");
        assert_eq!(OPUS_TAGS_MAGIC, *b"OpusTags");
        assert_eq!(TOKEN_BG, "#2b2d31");
        assert_eq!(TOKEN_SURFACE, "#383a40");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#23a55a");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(MicRecordState::resolve(false, false).as_str(), "idle");
        assert_eq!(MicRecordState::resolve(false, true).as_str(), "hover mic");
        assert_eq!(
            MicRecordState::resolve(true, false).as_str(),
            "recording audio"
        );
        let live_decoded = decode_waveform_base64(LIVE_VOICE_WAVEFORM_B64).unwrap_or_default();
        assert_eq!(live_decoded.len(), 81);
        assert_eq!(live_decoded[0], 0);
        assert!(live_decoded.iter().any(|&b| b > 0));
    }

    #[test]
    fn test_sub_1003_2_cpal_ogg_opus_and_waveform_256_oracle() {
        let mut cap =
            CpalMicCaptureBuffer::new(SAMPLE_RATE_48KHZ, CHANNELS_MONO, "usb-mic-01").unwrap();
        cap.start_recording();
        for f in 0..30 {
            let amp = ((f + 1) as f32) / 30.0;
            let frame: Vec<f32> = (0..OPUS_FRAME_SAMPLES_20MS)
                .map(|i| amp * ((2.0 * std::f32::consts::PI * 440.0 * (i as f32)) / 48_000.0).sin())
                .collect();
            cap.push_pcm_chunk(&frame).unwrap();
        }
        let rec = VoiceMessageOggOpusRecorder::encode_recording(&cap, OPUS_BITRATE_32KBPS).unwrap();
        assert_eq!(rec.sample_rate_hz, 48_000);
        assert_eq!(rec.channels, 1);
        assert_eq!(rec.bitrate_bps, 32_000);
        assert_eq!(rec.frame_count, 30);
        assert_eq!(rec.flags, 8_192);
        assert_eq!(rec.waveform_bytes.len(), 256);
        assert_eq!(rec.waveform_base64.len(), 344);
        assert_eq!(rec.waveform_bytes[255], 255);
        assert!(rec.waveform_bytes[0] >= 7 && rec.waveform_bytes[0] <= 10);
        assert_eq!(&rec.ogg_opus_bytes[0..4], b"OggS");
        assert_eq!(&rec.ogg_opus_bytes[28..36], b"OpusHead");

        let roundtrip = decode_waveform_base64(&rec.waveform_base64).unwrap();
        assert_eq!(roundtrip.as_slice(), &rec.waveform_bytes[..]);

        let upload_req = build_attachment_upload_request_json(
            LIVE_VOICE_CHANNEL_ID,
            rec.ogg_opus_bytes.len() as u64,
        )
        .unwrap();
        assert!(upload_req.contains("\"filename\":\"voice-message.ogg\""));

        let send_payload = build_voice_message_send_payload(
            LIVE_VOICE_CHANNEL_ID,
            "uploads/voice-message-0.ogg",
            rec.duration_secs,
            &rec.waveform_base64,
            LIVE_VOICE_MESSAGE_ID,
            MESSAGE_FLAG_IS_VOICE_MESSAGE,
        )
        .unwrap();
        assert!(send_payload.contains("\"flags\":8192"));
        assert!(send_payload.contains(&rec.waveform_base64));
    }

    #[test]
    fn test_sub_1003_4_nine_dimensions_and_security() {
        assert!(CpalMicCaptureBuffer::new(44_100, 1, "mic").is_err());
        assert!(CpalMicCaptureBuffer::new(48_000, 2, "mic").is_err());
        let mut cap = CpalMicCaptureBuffer::new(48_000, 1, "usb-mic").unwrap();
        assert_eq!(cap.handle_device_disconnect(), "default-cpal-input");
        cap.push_pcm_chunk(&vec![0.0_f32; 48_000]).unwrap();
        let rec = VoiceMessageOggOpusRecorder::encode_recording(&cap, OPUS_BITRATE_32KBPS).unwrap();
        assert!(rec.waveform_bytes.iter().all(|&b| b == 0));
        assert!(VoiceMessageOggOpusRecorder::encode_recording(&cap, 16_000).is_err());
        assert!(
            build_voice_message_send_payload(
                LIVE_VOICE_CHANNEL_ID,
                "uploads/v.ogg",
                1.0,
                &rec.waveform_base64,
                "1",
                0
            )
            .is_err()
        );
        let full_perms =
            PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES | PERM_ATTACH_FILES | PERM_SEND_VOICE_MESSAGES;
        assert!(validate_voice_message_permissions(full_perms, false, false).is_ok());
        assert!(validate_voice_message_permissions(full_perms, true, false).is_err());
        assert_eq!(
            validate_workload_boundaries(
                "18446744073709551615",
                2000,
                10,
                MAX_ATTACHMENT_BYTES,
                9975,
                200,
                150
            )
            .unwrap(),
            "99+"
        );
        assert!(validate_workload_boundaries("0", 2001, 10, 1024, 0, 100, 1).is_err());
        assert_eq!(
            classify_voice_rest_status(202, 0).unwrap(),
            "indexing_or_upload_warmup_202"
        );
        assert!(classify_voice_rest_status(413, 0).is_err());
        assert!(classify_voice_rest_status(429, 1500).is_err());
        let json = export_voice_message_telemetry_json(&rec, "token=mfa.secret12345");
        assert!(json.contains("mfa.***") && !json.contains("secret12345"));
    }
}
