#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Линейный микшер звуковых эффектов (`Soundboard SFX`) в исходящий поток микрофона
//! с софт-лимитером `tanh()` по стандарту RFC 3550 RTP Opus (`PT=120`, `48kHz stereo`, `20ms` = `960` отсчётов/канал) — `TASK-1012B`.
//! Оракулы: RFC 3550 RTP, RFC 7587 RTP Payload Format for Opus, `/home/sanya/re-deep/voice-deep.md`,
//! `docs/re/voice-deep.md` (`V=2, P=0, X=1, CC=0`, `PT=120`, `0xBEDE`, `960 @48 кГц`, `aead_aes256_gcm_rtpsize`),
//! `waves/kanban-board.json` (`TASK-1012B`), `docs/re/gateway-protocol.json` (`SOUNDBOARD_SOUNDS = 21`) и RE-модуль `904054`.

use std::fmt;

pub const AUDIO_MIXER_CONTAINER: &str = "Audio Mixer";
pub const AUDIO_MIXER_DIMENSIONS: &str = "20ms frames";
pub const AUDIO_MIXER_PADDING: &str = "0px";
pub const AUDIO_MIXER_SPACING: &str = "0px";

pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#23a55a";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const MIXER_DSP_ELEMENT: &str = "mixer-dsp";

pub const RTP_VERSION: u8 = 2;
pub const RTP_OPUS_PAYLOAD_TYPE: u8 = 120;
pub const RTP_HEADER_FIXED_BYTES: usize = 12;
pub const RTP_EXTENSION_PROFILE_BEDE: u16 = 0xBEDE;

pub const SAMPLE_RATE_48KHZ: u32 = 48_000;
pub const CHANNELS_STEREO: u8 = 2;
pub const FRAME_DURATION_20MS: u32 = 20;
pub const SAMPLES_PER_CHANNEL_20MS: usize = 960;
pub const INTERLEAVED_STEREO_SAMPLES_20MS: usize = 1_920;

pub const OPUS_DTX_COMFORT_NOISE_3B: [u8; 3] = [0xF8, 0xFF, 0xFE];
pub const SESSION2_AUDIO_SSRC: u32 = 17_843;
pub const SESSION3_AUDIO_SSRC: u32 = 9_208;
pub const MEDIA_ENCRYPTION_MODE: &str = "aead_aes256_gcm_rtpsize";
pub const GATEWAY_SOUNDBOARD_SOUNDS_DISPATCH_COUNT: u32 = 21;

pub const SPEAKING_FLAG_NONE: u32 = 0;
pub const SPEAKING_FLAG_MICROPHONE: u32 = 1 << 0;
pub const SPEAKING_FLAG_SOUND_SHARE: u32 = 1 << 1;
pub const SPEAKING_FLAG_PRIORITY: u32 = 1 << 2;

pub const TANH_LIMITER_THRESHOLD: f32 = 0.95;
pub const MAX_SFX_GAIN: f32 = 2.0;

pub const MIN_SOUND_NAME_CHARS: usize = 2;
pub const MAX_SOUND_NAME_CHARS: usize = 32;
pub const MAX_FAVORITE_SOUNDS: usize = 250;
pub const MAX_SOUNDBOARD_SOUND_BYTES: usize = 524_288;

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_CONNECT: u64 = 1 << 20;
pub const PERM_SPEAK: u64 = 1 << 21;
pub const PERM_USE_SOUNDBOARD: u64 = 1 << 42;
pub const PERM_USE_EXTERNAL_SOUNDS: u64 = 1 << 45;

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MixerDspState {
    Bypass,
    Mix,
    TanhLimiter,
}

impl MixerDspState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Bypass => "bypass",
            Self::Mix => "mix",
            Self::TanhLimiter => "tanh limiter",
        }
    }

    #[must_use]
    pub const fn resolve(active_sfx: bool, limiting: bool) -> Self {
        match (active_sfx, limiting) {
            (false, _) => Self::Bypass,
            (true, true) => Self::TanhLimiter,
            (true, false) => Self::Mix,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundboardMixerError {
    InvalidSnowflake(String),
    InvalidSoundName(String),
    InvalidVolume,
    InvalidAudioFormat { sample_rate_hz: u32, channels: u8 },
    EmptySfxSamples,
    UnalignedStereoSamples(usize),
    SamplePayloadTooLarge(usize),
    InvalidRtpVersion(u8),
    InvalidRtpPayloadType(u8),
    TruncatedRtpHeader(usize),
    UserMutedOrDeafened,
    MissingPermission(&'static str),
    ExternalSoundsNotAllowed,
    AgeGatedSpaceRestricted,
    BoundaryViolation(&'static str),
    HttpStatusError { status: u16, retry_after_ms: u64 },
}

impl fmt::Display for SoundboardMixerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SoundboardMixerError {}

pub fn validate_snowflake_id(raw: &str) -> Result<u64, SoundboardMixerError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SoundboardMixerError::InvalidSnowflake(raw.to_string()));
    }
    let id = trimmed
        .parse::<u64>()
        .map_err(|_| SoundboardMixerError::InvalidSnowflake(raw.to_string()))?;
    if id == 0 {
        return Err(SoundboardMixerError::InvalidSnowflake(raw.to_string()));
    }
    Ok(id)
}

#[must_use]
pub fn is_valid_sound_name(name: &str) -> bool {
    let count = name.trim().chars().count();
    (MIN_SOUND_NAME_CHARS..=MAX_SOUND_NAME_CHARS).contains(&count)
}

#[must_use]
pub fn sanitize_f32_sample(sample: f32) -> f32 {
    if sample.is_finite() { sample } else { 0.0 }
}

#[must_use]
pub fn clamp_soundboard_volume(vol: f32) -> f32 {
    if !vol.is_finite() {
        return 1.0;
    }
    let clamped = vol.clamp(0.0, 1.0);
    (clamped * 100.0).round() / 100.0
}

#[must_use]
pub fn compute_effective_soundboard_volume(
    sound_vol: f32,
    setting_vol_pct: f32,
    output_vol_pct: f32,
) -> f32 {
    let s = if sound_vol.is_finite() {
        sound_vol.clamp(0.0, 1.0)
    } else {
        1.0
    };
    let set_v = if setting_vol_pct.is_finite() {
        setting_vol_pct.clamp(0.0, 200.0)
    } else {
        100.0
    };
    let out_v = if output_vol_pct.is_finite() {
        output_vol_pct.clamp(0.0, 200.0)
    } else {
        100.0
    };
    (s * (set_v / 100.0) * (out_v / 100.0).min(1.0)).min(1.0)
}

#[must_use]
pub fn soft_clip_tanh_sample(mic: f32, sfx: f32, gain: f32) -> f32 {
    let m = sanitize_f32_sample(mic);
    let s = sanitize_f32_sample(sfx);
    let g = if gain.is_finite() {
        gain.clamp(0.0, MAX_SFX_GAIN)
    } else {
        0.0
    };
    (m + s * g).tanh()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtpOpusHeader {
    pub version: u8,
    pub padding: bool,
    pub extension: bool,
    pub csrc_count: u8,
    pub marker: bool,
    pub payload_type: u8,
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
}

impl RtpOpusHeader {
    #[must_use]
    pub const fn new_opus(sequence: u16, timestamp: u32, ssrc: u32, extension: bool) -> Self {
        Self {
            version: RTP_VERSION,
            padding: false,
            extension,
            csrc_count: 0,
            marker: false,
            payload_type: RTP_OPUS_PAYLOAD_TYPE,
            sequence,
            timestamp,
            ssrc,
        }
    }

    #[must_use]
    pub fn encode_fixed_12b(&self) -> [u8; RTP_HEADER_FIXED_BYTES] {
        let b0 = ((self.version & 0x03) << 6)
            | (u8::from(self.padding) << 5)
            | (u8::from(self.extension) << 4)
            | (self.csrc_count & 0x0F);
        let b1 = (u8::from(self.marker) << 7) | (self.payload_type & 0x7F);
        let seq = self.sequence.to_be_bytes();
        let ts = self.timestamp.to_be_bytes();
        let ssrc = self.ssrc.to_be_bytes();
        [
            b0, b1, seq[0], seq[1], ts[0], ts[1], ts[2], ts[3], ssrc[0], ssrc[1], ssrc[2], ssrc[3],
        ]
    }

    pub fn decode_fixed_12b(bytes: &[u8]) -> Result<Self, SoundboardMixerError> {
        if bytes.len() < RTP_HEADER_FIXED_BYTES {
            return Err(SoundboardMixerError::TruncatedRtpHeader(bytes.len()));
        }
        let version = (bytes[0] >> 6) & 0x03;
        if version != RTP_VERSION {
            return Err(SoundboardMixerError::InvalidRtpVersion(version));
        }
        let payload_type = bytes[1] & 0x7F;
        if payload_type != RTP_OPUS_PAYLOAD_TYPE {
            return Err(SoundboardMixerError::InvalidRtpPayloadType(payload_type));
        }
        let sequence = u16::from_be_bytes([bytes[2], bytes[3]]);
        let timestamp = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        let ssrc = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
        Ok(Self {
            version,
            padding: ((bytes[0] >> 5) & 0x01) != 0,
            extension: ((bytes[0] >> 4) & 0x01) != 0,
            csrc_count: bytes[0] & 0x0F,
            marker: ((bytes[1] >> 7) & 0x01) != 0,
            payload_type,
            sequence,
            timestamp,
            ssrc,
        })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct RtpOpusMixedFrame {
    pub rtp_header: RtpOpusHeader,
    pub rtp_header_bytes: [u8; RTP_HEADER_FIXED_BYTES],
    pub payload_type: u8,
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub speaking_flags: u32,
    pub dsp_state: MixerDspState,
    pub effective_gain: f32,
    pub peak_abs: f32,
    pub rms: f32,
    pub limited_samples_count: usize,
    pub sanitized_non_finite_count: usize,
    pub mixed_stereo_samples: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SoundboardLinearMixer {
    ssrc: u32,
    sequence: u16,
    timestamp: u32,
    setting_volume_percent: f32,
    output_volume_percent: f32,
    active_sound_id: Option<String>,
    active_sound_volume: f32,
    sfx_cursor: usize,
    sfx_samples: Vec<f32>,
    frames_mixed_count: u64,
    auto_stops_count: u64,
}

impl SoundboardLinearMixer {
    #[must_use]
    pub fn new(
        ssrc: u32,
        initial_sequence: u16,
        initial_timestamp: u32,
        setting_volume_percent: f32,
        output_volume_percent: f32,
    ) -> Self {
        Self {
            ssrc,
            sequence: initial_sequence,
            timestamp: initial_timestamp,
            setting_volume_percent,
            output_volume_percent,
            active_sound_id: None,
            active_sound_volume: 1.0,
            sfx_cursor: 0,
            sfx_samples: Vec::new(),
            frames_mixed_count: 0,
            auto_stops_count: 0,
        }
    }

    pub fn start_sfx_playback(
        &mut self,
        sound_id: &str,
        sample_rate_hz: u32,
        channels: u8,
        sound_volume: f32,
        pcm_stereo_f32: Vec<f32>,
    ) -> Result<(), SoundboardMixerError> {
        validate_snowflake_id(sound_id)?;
        if sample_rate_hz != SAMPLE_RATE_48KHZ || channels != CHANNELS_STEREO {
            return Err(SoundboardMixerError::InvalidAudioFormat {
                sample_rate_hz,
                channels,
            });
        }
        if !sound_volume.is_finite() || !(0.0..=1.0).contains(&sound_volume) {
            return Err(SoundboardMixerError::InvalidVolume);
        }
        if pcm_stereo_f32.is_empty() {
            return Err(SoundboardMixerError::EmptySfxSamples);
        }
        if !pcm_stereo_f32
            .len()
            .is_multiple_of(usize::from(CHANNELS_STEREO))
        {
            return Err(SoundboardMixerError::UnalignedStereoSamples(
                pcm_stereo_f32.len(),
            ));
        }
        self.active_sound_id = Some(sound_id.trim().to_string());
        self.active_sound_volume = clamp_soundboard_volume(sound_volume);
        self.sfx_cursor = 0;
        self.sfx_samples = pcm_stereo_f32;
        Ok(())
    }

    pub fn stop_sfx_playback(&mut self) {
        self.active_sound_id = None;
        self.sfx_cursor = 0;
        self.sfx_samples.clear();
    }

    #[must_use]
    pub fn is_active(&self) -> bool {
        self.active_sound_id.is_some() && self.sfx_cursor < self.sfx_samples.len()
    }

    #[must_use]
    pub const fn stats(&self) -> (u64, u64, u16, u32) {
        (
            self.frames_mixed_count,
            self.auto_stops_count,
            self.sequence,
            self.timestamp,
        )
    }

    pub fn mix_next_20ms_rtp_frame(&mut self, mic_stereo_frame: &[f32]) -> RtpOpusMixedFrame {
        let seq = self.sequence;
        let ts = self.timestamp;
        let ts_step = SAMPLES_PER_CHANNEL_20MS as u32;
        self.sequence = self.sequence.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(ts_step);
        let rtp_header = RtpOpusHeader::new_opus(seq, ts, self.ssrc, true);
        let rtp_header_bytes = rtp_header.encode_fixed_12b();

        if !self.is_active() {
            return self.build_bypass_frame(rtp_header, rtp_header_bytes, mic_stereo_frame);
        }

        let gain = compute_effective_soundboard_volume(
            self.active_sound_volume,
            self.setting_volume_percent,
            self.output_volume_percent,
        );
        let (mixed, peak_abs, rms, limited_cnt, sanitized_cnt) =
            self.mix_active_window_20ms(mic_stereo_frame, gain);
        self.frames_mixed_count = self.frames_mixed_count.saturating_add(1);
        self.sfx_cursor = self
            .sfx_cursor
            .saturating_add(INTERLEAVED_STEREO_SAMPLES_20MS);
        if self.sfx_cursor >= self.sfx_samples.len() {
            self.stop_sfx_playback();
            self.auto_stops_count = self.auto_stops_count.saturating_add(1);
        }

        RtpOpusMixedFrame {
            rtp_header,
            rtp_header_bytes,
            payload_type: RTP_OPUS_PAYLOAD_TYPE,
            sequence: seq,
            timestamp: ts,
            ssrc: self.ssrc,
            speaking_flags: SPEAKING_FLAG_MICROPHONE | SPEAKING_FLAG_SOUND_SHARE,
            dsp_state: MixerDspState::resolve(true, limited_cnt > 0),
            effective_gain: gain,
            peak_abs,
            rms,
            limited_samples_count: limited_cnt,
            sanitized_non_finite_count: sanitized_cnt,
            mixed_stereo_samples: mixed,
        }
    }

    fn build_bypass_frame(
        &self,
        rtp_header: RtpOpusHeader,
        rtp_header_bytes: [u8; RTP_HEADER_FIXED_BYTES],
        mic_stereo_frame: &[f32],
    ) -> RtpOpusMixedFrame {
        let mut out = Vec::with_capacity(INTERLEAVED_STEREO_SAMPLES_20MS);
        let mut peak_abs = 0.0_f32;
        let mut sum_sq = 0.0_f32;
        let mut sanitized_cnt = 0_usize;
        for i in 0..INTERLEAVED_STEREO_SAMPLES_20MS {
            let raw_mic = mic_stereo_frame.get(i).copied().unwrap_or(0.0);
            if !raw_mic.is_finite() {
                sanitized_cnt += 1;
            }
            let clamped = sanitize_f32_sample(raw_mic).clamp(-1.0, 1.0);
            peak_abs = peak_abs.max(clamped.abs());
            sum_sq += clamped * clamped;
            out.push(clamped);
        }
        let rms = (sum_sq / (INTERLEAVED_STEREO_SAMPLES_20MS as f32)).sqrt();
        RtpOpusMixedFrame {
            rtp_header,
            rtp_header_bytes,
            payload_type: RTP_OPUS_PAYLOAD_TYPE,
            sequence: rtp_header.sequence,
            timestamp: rtp_header.timestamp,
            ssrc: self.ssrc,
            speaking_flags: SPEAKING_FLAG_MICROPHONE,
            dsp_state: MixerDspState::Bypass,
            effective_gain: 0.0,
            peak_abs,
            rms,
            limited_samples_count: 0,
            sanitized_non_finite_count: sanitized_cnt,
            mixed_stereo_samples: out,
        }
    }

    fn mix_active_window_20ms(
        &self,
        mic_stereo_frame: &[f32],
        gain: f32,
    ) -> (Vec<f32>, f32, f32, usize, usize) {
        let mut mixed = Vec::with_capacity(INTERLEAVED_STEREO_SAMPLES_20MS);
        let mut peak_abs = 0.0_f32;
        let mut sum_sq = 0.0_f32;
        let mut limited_cnt = 0_usize;
        let mut sanitized_cnt = 0_usize;
        for i in 0..INTERLEAVED_STEREO_SAMPLES_20MS {
            let raw_mic = mic_stereo_frame.get(i).copied().unwrap_or(0.0);
            let raw_sfx = self
                .sfx_samples
                .get(self.sfx_cursor + i)
                .copied()
                .unwrap_or(0.0);
            if !raw_mic.is_finite() || !raw_sfx.is_finite() {
                sanitized_cnt += 1;
            }
            let m = sanitize_f32_sample(raw_mic);
            let s = sanitize_f32_sample(raw_sfx);
            if (m + s * gain).abs() > TANH_LIMITER_THRESHOLD {
                limited_cnt += 1;
            }
            let sample = soft_clip_tanh_sample(m, s, gain);
            peak_abs = peak_abs.max(sample.abs());
            sum_sq += sample * sample;
            mixed.push(sample);
        }
        let rms = (sum_sq / (INTERLEAVED_STEREO_SAMPLES_20MS as f32)).sqrt();
        (mixed, peak_abs, rms, limited_cnt, sanitized_cnt)
    }
}

pub fn evaluate_mixer_permissions(
    permissions: u64,
    is_dm: bool,
    is_external: bool,
    is_muted_or_deaf: bool,
    age_gated_restricted: bool,
) -> Result<(), SoundboardMixerError> {
    if age_gated_restricted {
        return Err(SoundboardMixerError::AgeGatedSpaceRestricted);
    }
    if is_muted_or_deaf {
        return Err(SoundboardMixerError::UserMutedOrDeafened);
    }
    if !is_dm && (permissions & PERM_CONNECT) == 0 {
        return Err(SoundboardMixerError::MissingPermission("CONNECT"));
    }
    if !is_dm && (permissions & PERM_SPEAK) == 0 {
        return Err(SoundboardMixerError::MissingPermission("SPEAK"));
    }
    if !is_dm && (permissions & PERM_USE_SOUNDBOARD) == 0 {
        return Err(SoundboardMixerError::MissingPermission("USE_SOUNDBOARD"));
    }
    if !is_dm && is_external && (permissions & PERM_USE_EXTERNAL_SOUNDS) == 0 {
        return Err(SoundboardMixerError::ExternalSoundsNotAllowed);
    }
    Ok(())
}

#[must_use]
pub fn format_badge_cap(count: u32) -> String {
    if count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

pub fn validate_platform_boundaries(
    message_chars: usize,
    attachments: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
    favorites_count: usize,
) -> Result<String, SoundboardMixerError> {
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(SoundboardMixerError::BoundaryViolation("message_chars"));
    }
    if attachments > MAX_ATTACHMENTS_COUNT {
        return Err(SoundboardMixerError::BoundaryViolation("attachments"));
    }
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(SoundboardMixerError::BoundaryViolation("attachment_bytes"));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(SoundboardMixerError::BoundaryViolation("search_offset"));
    }
    if volume_pct > MAX_VOLUME_PERCENT {
        return Err(SoundboardMixerError::BoundaryViolation("volume_pct"));
    }
    if favorites_count > MAX_FAVORITE_SOUNDS {
        return Err(SoundboardMixerError::BoundaryViolation("favorites_count"));
    }
    Ok(format_badge_cap(150))
}

pub fn classify_mixer_http_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<&'static str, SoundboardMixerError> {
    match status {
        200 | 201 | 204 => Ok("ok"),
        202 => Ok("skeleton_retry"),
        _ => Err(SoundboardMixerError::HttpStatusError {
            status,
            retry_after_ms,
        }),
    }
}

#[must_use]
pub fn redact_mixer_log(raw: &str) -> String {
    let mut out = raw.to_string();
    for marker in ["Bearer ", "token="] {
        while let Some(pos) = out.find(marker) {
            let start = pos + marker.len();
            let tail = &out[start..];
            if tail.starts_with("[REDACTED]") {
                break;
            }
            let end_rel = tail
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .unwrap_or(tail.len());
            out.replace_range(start..start + end_rel, "[REDACTED]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1012b_1_tokens_layout_and_rfc3550_constants() {
        assert_eq!(AUDIO_MIXER_CONTAINER, "Audio Mixer");
        assert_eq!(AUDIO_MIXER_DIMENSIONS, "20ms frames");
        assert_eq!((AUDIO_MIXER_PADDING, AUDIO_MIXER_SPACING), ("0px", "0px"));
        assert_eq!(
            (TOKEN_BACKGROUND, TOKEN_SURFACE, TOKEN_ACCENT),
            ("#1e1f22", "#2b2d31", "#23a55a")
        );
        assert_eq!((TOKEN_TEXT_PRIMARY, TOKEN_BORDER), ("#ffffff", "#3f4147"));
        assert_eq!(MIXER_DSP_ELEMENT, "mixer-dsp");
        assert_eq!(MixerDspState::resolve(false, false).as_str(), "bypass");
        assert_eq!(MixerDspState::resolve(true, false).as_str(), "mix");
        assert_eq!(MixerDspState::resolve(true, true).as_str(), "tanh limiter");
        assert_eq!(
            (
                RTP_VERSION,
                RTP_OPUS_PAYLOAD_TYPE,
                RTP_EXTENSION_PROFILE_BEDE
            ),
            (2, 120, 0xBEDE)
        );
        assert_eq!((SESSION2_AUDIO_SSRC, SESSION3_AUDIO_SSRC), (17_843, 9_208));
        assert_eq!(MEDIA_ENCRYPTION_MODE, "aead_aes256_gcm_rtpsize");
    }

    #[test]
    fn test_sub_1012b_2_linear_mixing_rtp_header_and_auto_stop() {
        let mut mixer = SoundboardLinearMixer::new(SESSION2_AUDIO_SSRC, 500, 96_000, 100.0, 100.0);
        let sfx_pcm = vec![0.3_f32; INTERLEAVED_STEREO_SAMPLES_20MS * 2];
        let mic_pcm = vec![0.2_f32; INTERLEAVED_STEREO_SAMPLES_20MS];

        assert!(
            mixer
                .start_sfx_playback("1102645616632397904", 48_000, 2, 1.0, sfx_pcm)
                .is_ok()
        );
        assert!(mixer.is_active());

        let f1 = mixer.mix_next_20ms_rtp_frame(&mic_pcm);
        assert_eq!(
            (f1.payload_type, f1.sequence, f1.timestamp),
            (120, 500, 96_000)
        );
        assert_eq!(
            f1.speaking_flags,
            SPEAKING_FLAG_MICROPHONE | SPEAKING_FLAG_SOUND_SHARE
        );
        assert_eq!(f1.dsp_state, MixerDspState::Mix);
        assert_eq!(
            RtpOpusHeader::decode_fixed_12b(&f1.rtp_header_bytes),
            Ok(f1.rtp_header)
        );
        assert!(mixer.is_active());

        let f2 = mixer.mix_next_20ms_rtp_frame(&mic_pcm);
        assert_eq!((f2.sequence, f2.timestamp), (501, 96_960));
        assert!(!mixer.is_active());

        let f3 = mixer.mix_next_20ms_rtp_frame(&mic_pcm);
        assert_eq!((f3.sequence, f3.timestamp), (502, 97_920));
        assert_eq!(f3.speaking_flags, SPEAKING_FLAG_MICROPHONE);
        assert_eq!(f3.dsp_state, MixerDspState::Bypass);
        assert_eq!(mixer.stats(), (2, 1, 503, 98_880));
    }

    #[test]
    fn test_sub_1012b_3_soft_clip_tanh_limiter_prevents_speech_clipping() {
        assert!((compute_effective_soundboard_volume(0.5, 80.0, 100.0) - 0.4).abs() < 1e-6);
        let pos_limited = soft_clip_tanh_sample(0.85, 0.85, 1.0);
        let neg_limited = soft_clip_tanh_sample(-0.85, -0.85, 1.0);
        assert!(pos_limited > 0.93 && pos_limited < 1.0);
        assert!(neg_limited < -0.93 && neg_limited > -1.0);

        let mut mixer = SoundboardLinearMixer::new(SESSION3_AUDIO_SSRC, 10, 4_800, 100.0, 100.0);
        let loud_sfx = vec![0.9_f32; INTERLEAVED_STEREO_SAMPLES_20MS];
        let loud_mic = vec![0.85_f32; INTERLEAVED_STEREO_SAMPLES_20MS];
        assert!(
            mixer
                .start_sfx_playback("1102650064771485746", 48_000, 2, 1.0, loud_sfx)
                .is_ok()
        );
        let frame = mixer.mix_next_20ms_rtp_frame(&loud_mic);
        assert_eq!(frame.dsp_state, MixerDspState::TanhLimiter);
        assert_eq!(frame.limited_samples_count, INTERLEAVED_STEREO_SAMPLES_20MS);
        assert!(frame.peak_abs > 0.93 && frame.peak_abs < 1.0);
    }

    #[test]
    fn test_sub_1012b_4_permissions_boundaries_and_http_status() {
        assert!(validate_snowflake_id("0").is_err());
        assert_eq!(validate_snowflake_id("18446744073709551615"), Ok(u64::MAX));
        assert!(is_valid_sound_name("Оу") && !is_valid_sound_name("А"));

        let base_perms = PERM_CONNECT | PERM_SPEAK;
        assert_eq!(
            evaluate_mixer_permissions(base_perms, false, false, false, false),
            Err(SoundboardMixerError::MissingPermission("USE_SOUNDBOARD"))
        );
        assert_eq!(
            evaluate_mixer_permissions(base_perms | PERM_USE_SOUNDBOARD, false, true, false, false),
            Err(SoundboardMixerError::ExternalSoundsNotAllowed)
        );
        let full_perms = base_perms | PERM_USE_SOUNDBOARD | PERM_USE_EXTERNAL_SOUNDS;
        assert!(evaluate_mixer_permissions(full_perms, false, true, false, false).is_ok());
        assert_eq!(
            validate_platform_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200, 250),
            Ok("99+".to_string())
        );
        assert_eq!(classify_mixer_http_status(202, 0), Ok("skeleton_retry"));
        assert!(classify_mixer_http_status(429, 1_500).is_err());
    }

    #[test]
    fn test_sub_1012b_4_rtp_wrap_around_nan_healing_and_redaction() {
        let sid = "1102645616632397904";
        let mut mixer = SoundboardLinearMixer::new(17_843, u16::MAX, u32::MAX - 479, 100.0, 100.0);
        assert!(
            mixer
                .start_sfx_playback(sid, 44_100, 2, 1.0, vec![0.1; 1920])
                .is_err()
        );
        assert!(
            mixer
                .start_sfx_playback(sid, 48_000, 2, 1.0, vec![0.1; 1919])
                .is_err()
        );
        assert!(
            mixer
                .start_sfx_playback(sid, 48_000, 2, 1.0, vec![f32::NAN; 1920])
                .is_ok()
        );
        let f_wrap = mixer.mix_next_20ms_rtp_frame(&[f32::INFINITY; 1920]);
        assert_eq!(
            (f_wrap.sequence, f_wrap.timestamp),
            (u16::MAX, u32::MAX - 479)
        );
        assert_eq!(
            (f_wrap.sanitized_non_finite_count, f_wrap.peak_abs),
            (1_920, 0.0)
        );
        let f_next = mixer.mix_next_20ms_rtp_frame(&[]);
        assert_eq!((f_next.sequence, f_next.timestamp), (0, 480));
        let redacted = redact_mixer_log("Authorization: Bearer mfa.secret123 token=xyz.987");
        assert!(!redacted.contains("mfa.secret123") && redacted.contains("[REDACTED]"));
    }
}
