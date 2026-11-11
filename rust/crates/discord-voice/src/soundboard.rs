//! Серверная звуковая панель (`Soundboard SFX` — `TASK-1012` / `TASK-1012A` / `TASK-1012B` / `TASK-1012C`).
//! Оракулы: `waves/kanban-board.json`, `docs/re/voice-deep.md` (Voice Op 18/19, RTP Opus PT=120 48kHz stereo),
//! `docs/re/gateway-protocol.json` (`SOUNDBOARD_SOUNDS = 21`), `/home/sanya/re-deep/rest-static.json`
//! и чанки `209932` (`SoundboardStore`), `796774`, `813564`, `904054`, `123973` (`USE_SOUNDBOARD = 1 << 42`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

pub const SOUNDBOARD_GRID_CONTAINER: &str = "Soundboard Grid";
pub const SOUNDBOARD_GRID_DIMENSIONS: &str = "320px x 360px";
pub const SOUNDBOARD_GRID_PADDING_PX: u32 = 8;
pub const SOUNDBOARD_GRID_SPACING_PX: u32 = 6;
pub const TOKEN_BACKGROUND: &str = "#2b2d31";
pub const TOKEN_SURFACE: &str = "#1e1f22";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const TOKEN_MIXER_ACCENT: &str = "#23a55a";
pub const SOUND_BUTTON_ELEMENT: &str = "sound-button";
pub const MIXER_DSP_ELEMENT: &str = "mixer-dsp";
pub const VOICE_OP_SOUNDBOARD_SOUND: u8 = 18;
pub const VOICE_OP_SPEAKING_SOUNDBOARD: u8 = 19;
pub const VOICE_OP_SPEAKING: u8 = 5;
pub const SPEAKING_FLAG_NONE: u32 = 0;
pub const SPEAKING_FLAG_MICROPHONE: u32 = 1 << 0;
pub const SPEAKING_FLAG_SOUND_SHARE: u32 = 1 << 1;
pub const SPEAKING_FLAG_PRIORITY: u32 = 1 << 2;
pub const RTP_OPUS_PAYLOAD_TYPE: u8 = 120;
pub const OPUS_SAMPLE_RATE_HZ: u32 = 48_000;
pub const OPUS_CHANNELS: u8 = 2;
pub const FRAME_DURATION_MS: u32 = 20;
pub const FRAME_SAMPLES_PER_CHANNEL_20MS: usize = 960;
pub const INTERLEAVED_STEREO_SAMPLES_20MS: usize = 1_920;
pub const MIN_SOUNDBOARD_NAME_CHARS: usize = 2;
pub const MAX_SOUNDBOARD_NAME_CHARS: usize = 32;
pub const MAX_SOUNDBOARD_BYTES: usize = 524_288;
pub const MAX_FAVORITE_SOUNDS: usize = 250;
pub const GATEWAY_SOUNDBOARD_SOUNDS_COUNT: u32 = 21;
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
pub enum SoundButtonInteractiveState {
    SfxTile,
    Scale105,
    PlayingSfx,
}

impl SoundButtonInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SfxTile => "sfx tile",
            Self::Scale105 => "scale-105",
            Self::PlayingSfx => "playing sfx",
        }
    }

    #[must_use]
    pub const fn resolve(playing: bool, hovered: bool) -> Self {
        match (playing, hovered) {
            (true, _) => Self::PlayingSfx,
            (false, true) => Self::Scale105,
            (false, false) => Self::SfxTile,
        }
    }
}

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
pub enum SoundboardError {
    InvalidSnowflake(String),
    InvalidSoundName(String),
    InvalidDataUri(String),
    SoundPayloadTooLarge(usize),
    InvalidAudioFormat { sample_rate_hz: u32, channels: u8 },
    EmptyAudioSample,
    InvalidVolume,
    UserMutedOrSuppressed,
    MissingPermission(&'static str),
    ExternalSoundsNotAllowed,
    FavoritesLimitReached(usize),
    AgeGatedRestricted,
    MessageTooLong(usize),
    TooManyAttachments(usize),
    AttachmentTooLarge(u64),
    SearchOffsetOutOfBounds(u32),
    VolumeOutOfBounds(u16),
    HttpFailure { status: u16, retry_after_ms: u64 },
}

impl fmt::Display for SoundboardError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SoundboardError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, SoundboardError> {
    let t = raw.trim();
    if t.is_empty() || !t.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SoundboardError::InvalidSnowflake(raw.to_string()));
    }
    let parsed = t
        .parse::<u64>()
        .map_err(|_| SoundboardError::InvalidSnowflake(raw.to_string()))?;
    if parsed == 0 {
        return Err(SoundboardError::InvalidSnowflake(raw.to_string()));
    }
    Ok(parsed)
}

#[must_use]
pub fn is_valid_soundboard_name(name: &str) -> bool {
    let count = name.trim().chars().count();
    (MIN_SOUNDBOARD_NAME_CHARS..=MAX_SOUNDBOARD_NAME_CHARS).contains(&count)
}

#[must_use]
pub fn clamp_soundboard_volume(vol: f32) -> f32 {
    vol.is_finite().then(|| vol.clamp(0.0, 1.0)).unwrap_or(1.0)
}

#[must_use]
pub fn compute_effective_soundboard_volume(s_vol: f32, set_pct: f32, out_pct: f32) -> f32 {
    let s = clamp_soundboard_volume(s_vol);
    let set_v = set_pct
        .is_finite()
        .then(|| set_pct.clamp(0.0, 200.0))
        .unwrap_or(100.0);
    let out_v = out_pct
        .is_finite()
        .then(|| out_pct.clamp(0.0, 200.0))
        .unwrap_or(100.0);
    (s * (set_v / 100.0) * (out_v / 100.0).min(1.0)).min(1.0)
}

#[must_use]
pub fn soft_clip_tanh_sample(mic: f32, sfx: f32, gain: f32) -> f32 {
    let m = mic.is_finite().then_some(mic).unwrap_or(0.0);
    let s = sfx.is_finite().then_some(sfx).unwrap_or(0.0);
    let g = gain
        .is_finite()
        .then(|| gain.clamp(0.0, 2.0))
        .unwrap_or(0.0);
    (m + s * g).tanh()
}

pub fn build_soundboard_cdn_url(sound_id: &str) -> Result<String, SoundboardError> {
    validate_snowflake(sound_id)?;
    Ok(format!(
        "https://cdn.discordapp.com/soundboard-sounds/{}",
        sound_id.trim()
    ))
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSoundboardOp18Frame {
    pub op: u8,
    pub sound_id: String,
    pub source_guild_id: Option<String>,
    pub volume: f32,
    pub ssrc: u32,
}

impl VoiceSoundboardOp18Frame {
    pub fn build(
        sid: &str,
        gid: Option<&str>,
        vol: f32,
        ssrc: u32,
    ) -> Result<Self, SoundboardError> {
        validate_snowflake(sid)?;
        if !vol.is_finite() || !(0.0..=1.0).contains(&vol) {
            return Err(SoundboardError::InvalidVolume);
        }
        let normalized_guild = match gid {
            Some("0") | None => None,
            Some(g) => {
                validate_snowflake(g)?;
                Some(g.trim().to_string())
            }
        };
        Ok(Self {
            op: VOICE_OP_SOUNDBOARD_SOUND,
            sound_id: sid.trim().to_string(),
            source_guild_id: normalized_guild,
            volume: clamp_soundboard_volume(vol),
            ssrc,
        })
    }

    #[must_use]
    pub fn to_json(&self) -> String {
        let gid_part = match &self.source_guild_id {
            Some(gid) => format!(",\"source_guild_id\":\"{gid}\""),
            None => String::new(),
        };
        format!(
            "{{\"op\":{},\"d\":{{\"sound_id\":\"{}\"{},\"volume\":{:.2},\"ssrc\":{}}}}}",
            self.op, self.sound_id, gid_part, self.volume, self.ssrc
        )
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceSoundboardSpeakingEvent {
    pub op: u8,
    pub user_id: String,
    pub ssrc: u32,
    pub speaking_bits: u32,
    pub sound_id: Option<String>,
    pub volume: f32,
}

impl VoiceSoundboardSpeakingEvent {
    pub fn parse(
        op: u8,
        uid: &str,
        ssrc: u32,
        bits: u32,
        sid: Option<&str>,
        vol: f32,
    ) -> Result<Self, SoundboardError> {
        validate_snowflake(uid)?;
        if let Some(s) = sid {
            validate_snowflake(s)?;
        }
        Ok(Self {
            op,
            user_id: uid.trim().to_string(),
            ssrc,
            speaking_bits: bits,
            sound_id: sid.map(|s| s.trim().to_string()),
            volume: clamp_soundboard_volume(vol),
        })
    }

    #[must_use]
    pub const fn is_soundboard_active(&self) -> bool {
        (self.speaking_bits & SPEAKING_FLAG_SOUND_SHARE) != 0
    }

    #[must_use]
    pub const fn is_microphone_active(&self) -> bool {
        (self.speaking_bits & SPEAKING_FLAG_MICROPHONE) != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundboardRestRequest {
    pub method: &'static str,
    pub path: String,
    pub api_v9_path: String,
    pub body_json: String,
}

pub fn build_send_soundboard_sound_request(
    cid: &str,
    sid: &str,
    gid: &str,
) -> Result<SoundboardRestRequest, SoundboardError> {
    validate_snowflake(cid)?;
    let s = sid.trim();
    if s != "1" {
        validate_snowflake(s)?;
    }
    let path = format!("/channels/{}/send-soundboard-sound", cid.trim());
    let mut parts = vec![format!("\"sound_id\":\"{s}\"")];
    if gid.trim() != "0" && !gid.trim().is_empty() {
        validate_snowflake(gid)?;
        parts.push(format!("\"source_guild_id\":\"{}\"", gid.trim()));
    }
    Ok(SoundboardRestRequest {
        method: "POST",
        api_v9_path: format!("/api/v9{path}"),
        path,
        body_json: format!("{{{}}}", parts.join(",")),
    })
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedSfxSample {
    pub sound_id: String,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub opus_payload_bytes: Vec<u8>,
    pub pcm_stereo_samples: Vec<f32>,
    pub fnv1a_hash: u64,
    pub last_accessed_ms: u64,
}

#[derive(Debug, Clone)]
pub struct SoundboardAudioCache {
    max_bytes: usize,
    total_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    entries: BTreeMap<String, CachedSfxSample>,
}

impl SoundboardAudioCache {
    #[must_use]
    pub fn new(max_bytes: usize) -> Self {
        Self {
            max_bytes: max_bytes.max(64),
            total_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            entries: BTreeMap::new(),
        }
    }

    pub fn put_sample(
        &mut self,
        sid: &str,
        rate_hz: u32,
        ch: u8,
        opus: Vec<u8>,
        pcm: Vec<f32>,
        now_ms: u64,
    ) -> Result<&CachedSfxSample, SoundboardError> {
        validate_snowflake(sid)?;
        if rate_hz != OPUS_SAMPLE_RATE_HZ || ch != OPUS_CHANNELS {
            return Err(SoundboardError::InvalidAudioFormat {
                sample_rate_hz: rate_hz,
                channels: ch,
            });
        }
        let byte_len = opus.len();
        if byte_len == 0 || pcm.is_empty() {
            return Err(SoundboardError::EmptyAudioSample);
        }
        if byte_len > MAX_SOUNDBOARD_BYTES || byte_len > self.max_bytes {
            return Err(SoundboardError::SoundPayloadTooLarge(byte_len));
        }
        self.evict_until_fits(sid.trim(), byte_len);
        let key = sid.trim().to_string();
        let hash = fnv1a_64(&opus);
        self.total_bytes += byte_len;
        let sample = CachedSfxSample {
            sound_id: key.clone(),
            sample_rate_hz: rate_hz,
            channels: ch,
            opus_payload_bytes: opus,
            pcm_stereo_samples: pcm,
            fnv1a_hash: hash,
            last_accessed_ms: now_ms,
        };
        self.entries.insert(key.clone(), sample);
        self.entries
            .get(&key)
            .ok_or(SoundboardError::EmptyAudioSample)
    }

    fn evict_until_fits(&mut self, incoming_key: &str, needed_bytes: usize) {
        if let Some(prev) = self.entries.remove(incoming_key) {
            self.total_bytes = self
                .total_bytes
                .saturating_sub(prev.opus_payload_bytes.len());
        }
        while self.total_bytes + needed_bytes > self.max_bytes && !self.entries.is_empty() {
            let oldest = self
                .entries
                .iter()
                .min_by_key(|(_, v)| v.last_accessed_ms)
                .map(|(k, _)| k.clone());
            if let Some(Some(r)) = oldest.map(|k| self.entries.remove(&k)) {
                self.total_bytes = self.total_bytes.saturating_sub(r.opus_payload_bytes.len());
                self.evictions += 1;
            } else {
                break;
            }
        }
    }

    pub fn get_sample(&mut self, sound_id: &str, now_ms: u64) -> Option<&CachedSfxSample> {
        let key = sound_id.trim();
        if let Some(entry) = self.entries.get_mut(key) {
            entry.last_accessed_ms = now_ms;
            self.hits += 1;
        } else {
            self.misses += 1;
            return None;
        }
        self.entries.get(key)
    }

    #[must_use]
    pub const fn stats(&self) -> (usize, u64, u64, u64) {
        (self.total_bytes, self.hits, self.misses, self.evictions)
    }
}

#[must_use]
pub fn fnv1a_64(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[derive(Debug, Clone, PartialEq)]
pub struct MixedRtpOpusFrame {
    pub payload_type: u8,
    pub sequence: u16,
    pub timestamp: u32,
    pub ssrc: u32,
    pub speaking_flags: u32,
    pub dsp_state: MixerDspState,
    pub peak_abs: f32,
    pub mixed_stereo_samples: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct SoundboardRtpMixer {
    ssrc: u32,
    sequence: u16,
    timestamp: u32,
    setting_volume_percent: f32,
    output_volume_percent: f32,
    active_sound_id: Option<String>,
    active_sound_volume: f32,
    sfx_cursor: usize,
    sfx_samples: Vec<f32>,
}

impl SoundboardRtpMixer {
    #[must_use]
    pub fn new(ssrc: u32, seq: u16, ts: u32, setting_pct: f32, output_pct: f32) -> Self {
        Self {
            ssrc,
            sequence: seq,
            timestamp: ts,
            setting_volume_percent: setting_pct,
            output_volume_percent: output_pct,
            active_sound_id: None,
            active_sound_volume: 1.0,
            sfx_cursor: 0,
            sfx_samples: Vec::new(),
        }
    }

    pub fn start_sfx_playback(
        &mut self,
        sid: &str,
        vol: f32,
        pcm: Vec<f32>,
    ) -> Result<(), SoundboardError> {
        validate_snowflake(sid)?;
        if pcm.is_empty() {
            return Err(SoundboardError::EmptyAudioSample);
        }
        self.active_sound_id = Some(sid.trim().to_string());
        self.active_sound_volume = clamp_soundboard_volume(vol);
        self.sfx_cursor = 0;
        self.sfx_samples = pcm;
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

    pub fn mix_next_20ms_rtp_frame(&mut self, mic_stereo_frame: &[f32]) -> MixedRtpOpusFrame {
        let seq = self.sequence;
        let ts = self.timestamp;
        let step = FRAME_SAMPLES_PER_CHANNEL_20MS as u32;
        self.sequence = self.sequence.wrapping_add(1);
        self.timestamp = self.timestamp.wrapping_add(step);

        if !self.is_active() {
            let passthrough: Vec<f32> = mic_stereo_frame
                .iter()
                .take(INTERLEAVED_STEREO_SAMPLES_20MS)
                .map(|s| s.clamp(-1.0, 1.0))
                .collect();
            let peak = passthrough.iter().fold(0.0_f32, |acc, &x| acc.max(x.abs()));
            return MixedRtpOpusFrame {
                payload_type: RTP_OPUS_PAYLOAD_TYPE,
                sequence: seq,
                timestamp: ts,
                ssrc: self.ssrc,
                speaking_flags: SPEAKING_FLAG_MICROPHONE,
                dsp_state: MixerDspState::Bypass,
                peak_abs: peak,
                mixed_stereo_samples: passthrough,
            };
        }

        let gain = compute_effective_soundboard_volume(
            self.active_sound_volume,
            self.setting_volume_percent,
            self.output_volume_percent,
        );
        let (mixed, limiting, peak) = self.mix_active_window(mic_stereo_frame, gain);
        self.sfx_cursor += INTERLEAVED_STEREO_SAMPLES_20MS;
        if self.sfx_cursor >= self.sfx_samples.len() {
            self.stop_sfx_playback();
        }
        MixedRtpOpusFrame {
            payload_type: RTP_OPUS_PAYLOAD_TYPE,
            sequence: seq,
            timestamp: ts,
            ssrc: self.ssrc,
            speaking_flags: SPEAKING_FLAG_MICROPHONE | SPEAKING_FLAG_SOUND_SHARE,
            dsp_state: MixerDspState::resolve(true, limiting),
            peak_abs: peak,
            mixed_stereo_samples: mixed,
        }
    }

    fn mix_active_window(&self, mic_stereo_frame: &[f32], gain: f32) -> (Vec<f32>, bool, f32) {
        let mut mixed = Vec::with_capacity(INTERLEAVED_STEREO_SAMPLES_20MS);
        let mut limiting = false;
        let mut peak = 0.0_f32;
        for i in 0..INTERLEAVED_STEREO_SAMPLES_20MS {
            let idx = self.sfx_cursor + i;
            let mic = mic_stereo_frame.get(i).copied().unwrap_or(0.0);
            let sfx = self.sfx_samples.get(idx).copied().unwrap_or(0.0);
            if (mic + sfx * gain).abs() > 0.95 {
                limiting = true;
            }
            let out = (mic + sfx * gain).tanh();
            peak = peak.max(out.abs());
            mixed.push(out);
        }
        (mixed, limiting, peak)
    }
}

pub fn check_soundboard_permissions(
    permissions: u64,
    is_private: bool,
    ext: bool,
    muted: bool,
    age_gated: bool,
) -> Result<(), SoundboardError> {
    if age_gated {
        return Err(SoundboardError::AgeGatedRestricted);
    }
    if muted {
        return Err(SoundboardError::UserMutedOrSuppressed);
    }
    if !is_private && (permissions & PERM_CONNECT) == 0 {
        return Err(SoundboardError::MissingPermission("CONNECT"));
    }
    if !is_private && (permissions & PERM_SPEAK) == 0 {
        return Err(SoundboardError::MissingPermission("SPEAK"));
    }
    if !is_private && (permissions & PERM_USE_SOUNDBOARD) == 0 {
        return Err(SoundboardError::MissingPermission("USE_SOUNDBOARD"));
    }
    if !is_private && ext && (permissions & PERM_USE_EXTERNAL_SOUNDS) == 0 {
        return Err(SoundboardError::ExternalSoundsNotAllowed);
    }
    Ok(())
}

pub fn check_platform_boundaries(
    chars: usize,
    files: usize,
    bytes: u64,
    offset: u32,
    vol: u16,
    favs: usize,
) -> Result<String, SoundboardError> {
    if chars > MAX_MESSAGE_CHARS {
        return Err(SoundboardError::MessageTooLong(chars));
    }
    if files > MAX_ATTACHMENTS_COUNT {
        return Err(SoundboardError::TooManyAttachments(files));
    }
    if bytes > MAX_ATTACHMENT_BYTES {
        return Err(SoundboardError::AttachmentTooLarge(bytes));
    }
    if offset > MAX_SEARCH_OFFSET {
        return Err(SoundboardError::SearchOffsetOutOfBounds(offset));
    }
    if vol > MAX_VOLUME_PERCENT {
        return Err(SoundboardError::VolumeOutOfBounds(vol));
    }
    if favs > MAX_FAVORITE_SOUNDS {
        return Err(SoundboardError::FavoritesLimitReached(favs));
    }
    Ok("99+".to_string())
}

pub fn map_soundboard_http_status(
    status: u16,
    retry_ms: u64,
) -> Result<&'static str, SoundboardError> {
    match status {
        200 | 201 | 204 => Ok("ok"),
        202 => Ok("skeleton_retry"),
        _ => Err(SoundboardError::HttpFailure {
            status,
            retry_after_ms: retry_ms,
        }),
    }
}

#[must_use]
pub fn redact_soundboard_telemetry(raw: &str) -> String {
    let mut out = raw.to_string();
    for marker in ["Bearer ", "token="] {
        if let Some(pos) = out.find(marker) {
            let start = pos + marker.len();
            let tail = &out[start..];
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
    fn test_sub_1012_1_design_tokens_and_states() {
        assert_eq!(SOUNDBOARD_GRID_CONTAINER, "Soundboard Grid");
        assert_eq!(SOUNDBOARD_GRID_DIMENSIONS, "320px x 360px");
        assert_eq!((TOKEN_BACKGROUND, TOKEN_ACCENT), ("#2b2d31", "#5865f2"));
        assert_eq!(
            SoundButtonInteractiveState::resolve(false, false).as_str(),
            "sfx tile"
        );
        assert_eq!(
            SoundButtonInteractiveState::resolve(false, true).as_str(),
            "scale-105"
        );
        assert_eq!(
            SoundButtonInteractiveState::resolve(true, false).as_str(),
            "playing sfx"
        );
        assert_eq!(MixerDspState::resolve(false, false).as_str(), "bypass");
        assert_eq!(MixerDspState::resolve(true, false).as_str(), "mix");
        assert_eq!(MixerDspState::resolve(true, true).as_str(), "tanh limiter");
        assert_eq!(
            (VOICE_OP_SOUNDBOARD_SOUND, SPEAKING_FLAG_SOUND_SHARE),
            (18, 2)
        );
        assert_eq!(GATEWAY_SOUNDBOARD_SOUNDS_COUNT, 21);
    }

    #[test]
    fn test_sub_1012_2_voice_op18_and_op19_speaking() {
        let sid = "1102645616632397904";
        let gid = "274093052673392641";
        let frame = VoiceSoundboardOp18Frame::build(sid, Some(gid), 0.82, 17843);
        assert!(frame.is_ok());
        if let Ok(f) = frame {
            assert_eq!(f.op, 18);
            assert!(f.to_json().contains("\"op\":18") && f.to_json().contains(gid));
        }
        assert!(VoiceSoundboardOp18Frame::build(sid, None, 1.5, 1).is_err());
        let flags = SPEAKING_FLAG_MICROPHONE | SPEAKING_FLAG_SOUND_SHARE;
        let ev = VoiceSoundboardSpeakingEvent::parse(
            19,
            "275258435199827968",
            9208,
            flags,
            Some(sid),
            0.99,
        );
        assert!(ev.is_ok());
        if let Ok(parsed) = ev {
            assert!(parsed.is_soundboard_active() && parsed.is_microphone_active());
        }
    }

    #[test]
    fn test_sub_1012_2_rest_requests_and_audio_cache_lru() {
        let (sid1, sid2) = ("1102645616632397904", "1102650064771485746");
        let req =
            build_send_soundboard_sound_request("1294815650186788916", sid1, "274093052673392641");
        assert!(req.is_ok());
        if let Ok(r) = req {
            assert!(r.method == "POST" && r.path.ends_with("/send-soundboard-sound"));
        }
        let mut cache = SoundboardAudioCache::new(1_000);
        let p1 = cache
            .put_sample(sid1, 48_000, 2, vec![0x4f; 600], vec![0.25; 1920], 100)
            .is_ok();
        let p2 = cache
            .put_sample(sid2, 48_000, 2, vec![0x50; 600], vec![0.5; 1920], 200)
            .is_ok();
        let g1 = cache.get_sample(sid1, 300).is_none();
        let g2 = cache.get_sample(sid2, 300).is_some();
        assert!(p1 && p2 && g1 && g2 && cache.stats() == (600, 1, 1, 1));
    }

    #[test]
    fn test_sub_1012_3_effective_volume_and_rtp_mixer() {
        assert!((compute_effective_soundboard_volume(0.5, 80.0, 100.0) - 0.4).abs() < 1e-5);
        let clipped = soft_clip_tanh_sample(0.85, 0.85, 1.0);
        assert!(clipped > 0.9 && clipped < 1.0);

        let mut mixer = SoundboardRtpMixer::new(17843, 100, 48_000, 100.0, 100.0);
        let ok = mixer.start_sfx_playback("1102645616632397904", 0.9, vec![0.6; 3840]);
        assert!(ok.is_ok());
        let mic = vec![0.5; 1920];
        let f1 = mixer.mix_next_20ms_rtp_frame(&mic);
        assert_eq!(f1.payload_type, 120);
        assert_eq!((f1.sequence, f1.timestamp), (100, 48_000));
        assert_eq!(f1.speaking_flags, 3);
        assert_eq!(f1.dsp_state, MixerDspState::TanhLimiter);
        assert!(f1.mixed_stereo_samples[0] > 0.7 && f1.mixed_stereo_samples[0] < 1.0);

        let f2 = mixer.mix_next_20ms_rtp_frame(&mic);
        assert_eq!((f2.sequence, f2.timestamp), (101, 48_960));
        assert!(!mixer.is_active());
        let f3 = mixer.mix_next_20ms_rtp_frame(&mic);
        assert_eq!(f3.timestamp, 49_920);
        assert_eq!(f3.speaking_flags, 1);
        assert_eq!(f3.dsp_state, MixerDspState::Bypass);
    }

    #[test]
    fn test_sub_1012_4_nine_dimensions_and_permissions() {
        assert!(validate_snowflake("0").is_err());
        assert_eq!(validate_snowflake("18446744073709551615"), Ok(u64::MAX));
        assert!(is_valid_soundboard_name("Оу") && !is_valid_soundboard_name("А"));
        let base_p = PERM_CONNECT | PERM_SPEAK;
        assert_eq!(
            check_soundboard_permissions(base_p, false, false, false, false),
            Err(SoundboardError::MissingPermission("USE_SOUNDBOARD"))
        );
        assert_eq!(
            check_soundboard_permissions(base_p | PERM_USE_SOUNDBOARD, false, true, false, false),
            Err(SoundboardError::ExternalSoundsNotAllowed)
        );
        let full_p = base_p | PERM_USE_SOUNDBOARD | PERM_USE_EXTERNAL_SOUNDS;
        assert!(check_soundboard_permissions(full_p, false, true, false, false).is_ok());
        let b = check_platform_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200, 250);
        assert_eq!(b, Ok("99+".to_string()));
        assert_eq!(map_soundboard_http_status(202, 0), Ok("skeleton_retry"));
        assert!(map_soundboard_http_status(429, 1_500).is_err());
        let red = redact_soundboard_telemetry("Authorization: Bearer mfa.secret123 token=abc.def");
        assert!(!red.contains("mfa.secret123") && red.contains("[REDACTED]"));
    }
}
