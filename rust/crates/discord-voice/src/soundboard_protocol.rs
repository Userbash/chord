#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Протокол Voice Gateway Opcode 18 (`SOUNDBOARD_SOUND`), приём Opcode 19 (`SPEAKING` soundboard)
//! и кэш аудиосэмплов `48kHz stereo` (ОЗУ LRU + диск `~/.cache/discord-gpui/soundboard/` с FNV-1a64) — `TASK-1012A`.
//! Оракулы: `/home/sanya/re-deep/voice-deep.md`, `docs/re/voice-deep.md`, `waves/kanban-board.json` (`TASK-1012A`).

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

pub const SOUNDBOARD_PROTOCOL_CONTAINER: &str = "Soundboard Protocol";
pub const SOUNDBOARD_PROTOCOL_DIMENSIONS: &str = "Voice WS";
pub const SOUNDBOARD_PROTOCOL_PADDING: &str = "0px";
pub const SOUNDBOARD_PROTOCOL_SPACING: &str = "0px";
pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const SFX_TRIGGER_ELEMENT: &str = "sfx-trigger";
pub const DEFAULT_SOUNDBOARD_CACHE_DIR: &str = "~/.cache/discord-gpui/soundboard/";
pub const VOICE_OP_SPEAKING: u8 = 5;
pub const VOICE_OP_SOUNDBOARD_SOUND: u8 = 18;
pub const VOICE_OP_SPEAKING_SOUNDBOARD: u8 = 19;
pub const SPEAKING_FLAG_NONE: u32 = 0;
pub const SPEAKING_FLAG_MICROPHONE: u32 = 1 << 0;
pub const SPEAKING_FLAG_SOUND_SHARE: u32 = 1 << 1;
pub const SPEAKING_FLAG_PRIORITY: u32 = 1 << 2;
pub const RTP_OPUS_PAYLOAD_TYPE: u8 = 120;
pub const SAMPLE_RATE_48KHZ: u32 = 48_000;
pub const CHANNELS_STEREO: u8 = 2;
pub const FRAME_DURATION_20MS: u32 = 20;
pub const SAMPLES_PER_CHANNEL_20MS: usize = 960;
pub const INTERLEAVED_STEREO_SAMPLES_20MS: usize = 1_920;
pub const MAX_SOUNDBOARD_SOUND_BYTES: usize = 524_288;
pub const DEFAULT_CACHE_BUDGET_BYTES: usize = 16 * 1024 * 1024;
pub const GATEWAY_SOUNDBOARD_SOUNDS_DISPATCH_COUNT: u32 = 21;
pub const SESSION2_AUDIO_SSRC: u32 = 17_843;
pub const SESSION3_AUDIO_SSRC: u32 = 9_208;
pub const MEDIA_ENCRYPTION_MODE: &str = "aead_aes256_gcm_rtpsize";
pub const MIN_SOUND_NAME_CHARS: usize = 2;
pub const MAX_SOUND_NAME_CHARS: usize = 32;
pub const MAX_FAVORITE_SOUNDS: usize = 250;
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
pub enum SfxTriggerState {
    Idle,
    Preview,
    PlayingSfx,
}

impl SfxTriggerState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Preview => "preview",
            Self::PlayingSfx => "playing sfx",
        }
    }

    #[must_use]
    pub const fn resolve(playing: bool, hovered: bool) -> Self {
        match (playing, hovered) {
            (true, _) => Self::PlayingSfx,
            (false, true) => Self::Preview,
            (false, false) => Self::Idle,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SoundboardProtocolError {
    InvalidSnowflake(String),
    InvalidCacheKey(String),
    UnsupportedVoiceOpcode(u8),
    InvalidVolume,
    InvalidSoundName(String),
    InvalidAudioFormat {
        sample_rate_hz: u32,
        channels: u8,
    },
    UnalignedStereoPcm(usize),
    EmptySamplePayload,
    SamplePayloadTooLarge(usize),
    CorruptedCacheEntry {
        sound_id: String,
        expected_hash: u64,
        actual_hash: u64,
    },
    DiskIoError(String),
    UserMutedOrDeafened,
    MissingPermission(&'static str),
    ExternalSoundsNotAllowed,
    AgeGatedSpaceRestricted,
    BoundaryViolation(&'static str),
    HttpStatusError {
        status: u16,
        retry_after_ms: u64,
    },
}

impl fmt::Display for SoundboardProtocolError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SoundboardProtocolError {}

pub fn validate_snowflake_id(raw: &str) -> Result<u64, SoundboardProtocolError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SoundboardProtocolError::InvalidSnowflake(raw.to_string()));
    }
    let id = trimmed
        .parse::<u64>()
        .map_err(|_| SoundboardProtocolError::InvalidSnowflake(raw.to_string()))?;
    if id == 0 {
        return Err(SoundboardProtocolError::InvalidSnowflake(raw.to_string()));
    }
    Ok(id)
}

fn norm_opt_snowflake(raw: Option<&str>) -> Result<Option<String>, SoundboardProtocolError> {
    match raw.map(str::trim) {
        None | Some("") | Some("0") => Ok(None),
        Some(v) => {
            validate_snowflake_id(v)?;
            Ok(Some(v.to_string()))
        }
    }
}

pub fn validate_cache_key(sound_id: &str) -> Result<String, SoundboardProtocolError> {
    let trimmed = sound_id.trim();
    let valid_key = !trimmed.is_empty()
        && !trimmed.contains("..")
        && !trimmed.contains('/')
        && !trimmed.contains('\\')
        && !trimmed.contains('\0')
        && trimmed.bytes().all(|b| b.is_ascii_digit());
    if !valid_key {
        return Err(SoundboardProtocolError::InvalidCacheKey(
            sound_id.to_string(),
        ));
    }
    validate_snowflake_id(trimmed)?;
    Ok(trimmed.to_string())
}

#[must_use]
pub fn clamp_soundboard_volume(vol: f32) -> f32 {
    vol.is_finite()
        .then(|| ((vol.clamp(0.0, 1.0) * 100.0).round()) / 100.0)
        .unwrap_or(1.0)
}

#[must_use]
pub fn is_valid_sound_name(name: &str) -> bool {
    let count = name.trim().chars().count();
    (MIN_SOUND_NAME_CHARS..=MAX_SOUND_NAME_CHARS).contains(&count)
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceOp18SoundboardPayload {
    pub op: u8,
    pub sound_id: String,
    pub source_guild_id: Option<String>,
    pub volume: f32,
    pub emoji_id: Option<String>,
    pub emoji_name: Option<String>,
    pub ssrc: u32,
}

impl VoiceOp18SoundboardPayload {
    pub fn build(
        sound_id: &str,
        source_guild_id: Option<&str>,
        volume: f32,
        emoji_id: Option<&str>,
        emoji_name: Option<&str>,
        ssrc: u32,
    ) -> Result<Self, SoundboardProtocolError> {
        validate_snowflake_id(sound_id)?;
        if !volume.is_finite() || !(0.0..=1.0).contains(&volume) {
            return Err(SoundboardProtocolError::InvalidVolume);
        }
        Ok(Self {
            op: VOICE_OP_SOUNDBOARD_SOUND,
            sound_id: sound_id.trim().to_string(),
            source_guild_id: norm_opt_snowflake(source_guild_id)?,
            volume: clamp_soundboard_volume(volume),
            emoji_id: norm_opt_snowflake(emoji_id)?,
            emoji_name: emoji_name.map(|s| s.trim().to_string()),
            ssrc,
        })
    }

    #[must_use]
    pub fn to_ws_json(&self) -> String {
        let mut fields = vec![format!("\"sound_id\":\"{}\"", self.sound_id)];
        if let Some(gid) = &self.source_guild_id {
            fields.push(format!("\"source_guild_id\":\"{gid}\""));
        }
        fields.push(format!("\"volume\":{:.2}", self.volume));
        if let Some(eid) = &self.emoji_id {
            fields.push(format!("\"emoji_id\":\"{eid}\""));
        }
        if let Some(ename) = &self.emoji_name {
            fields.push(format!("\"emoji_name\":\"{ename}\""));
        }
        fields.push(format!("\"ssrc\":{}", self.ssrc));
        format!("{{\"op\":{},\"d\":{{{}}}}}", self.op, fields.join(","))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceOp19SpeakingSoundboardEvent {
    pub op: u8,
    pub user_id: String,
    pub ssrc: u32,
    pub speaking_bits: u32,
    pub sound_id: Option<String>,
    pub source_guild_id: Option<String>,
    pub volume: f32,
}

impl VoiceOp19SpeakingSoundboardEvent {
    pub fn parse(
        op: u8,
        user_id: &str,
        ssrc: u32,
        speaking_bits: u32,
        sound_id: Option<&str>,
        source_guild_id: Option<&str>,
        volume: f32,
    ) -> Result<Self, SoundboardProtocolError> {
        if op != VOICE_OP_SPEAKING_SOUNDBOARD && op != VOICE_OP_SPEAKING {
            return Err(SoundboardProtocolError::UnsupportedVoiceOpcode(op));
        }
        validate_snowflake_id(user_id)?;
        Ok(Self {
            op,
            user_id: user_id.trim().to_string(),
            ssrc,
            speaking_bits,
            sound_id: norm_opt_snowflake(sound_id)?,
            source_guild_id: norm_opt_snowflake(source_guild_id)?,
            volume: clamp_soundboard_volume(volume),
        })
    }

    #[must_use]
    pub const fn is_soundboard_speaking(&self) -> bool {
        (self.speaking_bits & SPEAKING_FLAG_SOUND_SHARE) != 0
    }

    #[must_use]
    pub const fn is_microphone_speaking(&self) -> bool {
        (self.speaking_bits & SPEAKING_FLAG_MICROPHONE) != 0
    }

    #[must_use]
    pub const fn is_priority_speaker(&self) -> bool {
        (self.speaking_bits & SPEAKING_FLAG_PRIORITY) != 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SoundboardRestEndpointRequest {
    pub method: &'static str,
    pub path: String,
    pub api_v9_path: String,
    pub body_json: String,
}

pub fn build_send_soundboard_sound_rest(
    channel_id: &str,
    sound_id: &str,
    source_guild_id: Option<&str>,
    emoji_id: Option<&str>,
    emoji_name: Option<&str>,
) -> Result<SoundboardRestEndpointRequest, SoundboardProtocolError> {
    validate_snowflake_id(channel_id)?;
    validate_snowflake_id(sound_id)?;
    let path = format!("/channels/{}/send-soundboard-sound", channel_id.trim());
    let mut parts = vec![format!("\"sound_id\":\"{}\"", sound_id.trim())];
    if let Some(gid) = norm_opt_snowflake(source_guild_id)? {
        parts.push(format!("\"source_guild_id\":\"{gid}\""));
    }
    if let Some(eid) = norm_opt_snowflake(emoji_id)? {
        parts.push(format!("\"emoji_id\":\"{eid}\""));
    }
    if let Some(ename) = emoji_name.map(str::trim).filter(|s| !s.is_empty()) {
        parts.push(format!("\"emoji_name\":\"{ename}\""));
    }
    Ok(SoundboardRestEndpointRequest {
        method: "POST",
        api_v9_path: format!("/api/v9{path}"),
        path,
        body_json: format!("{{{}}}", parts.join(",")),
    })
}

pub fn build_soundboard_cdn_url(sound_id: &str) -> Result<String, SoundboardProtocolError> {
    let key = validate_cache_key(sound_id)?;
    Ok(format!(
        "https://cdn.discordapp.com/soundboard-sounds/{key}"
    ))
}

#[must_use]
pub fn fnv1a64_checksum(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for &b in bytes {
        hash = (hash ^ u64::from(b)).wrapping_mul(0x0100_0000_01b3);
    }
    hash
}

#[derive(Debug, Clone, PartialEq)]
pub struct Stereo48kSampleEntry {
    pub sound_id: String,
    pub sample_rate_hz: u32,
    pub channels: u8,
    pub frame_count_20ms: usize,
    pub opus_bytes: Vec<u8>,
    pub pcm_stereo_f32: Vec<f32>,
    pub fnv1a_hash: u64,
    pub disk_path: Option<String>,
    pub last_accessed_ms: u64,
}

#[derive(Debug, Clone)]
pub struct SoundboardSampleCache {
    max_bytes: usize,
    used_bytes: usize,
    hits: u64,
    misses: u64,
    evictions: u64,
    corruptions_healed: u64,
    disk_dir: Option<PathBuf>,
    entries: BTreeMap<String, Stereo48kSampleEntry>,
}

impl SoundboardSampleCache {
    #[must_use]
    pub fn new(max_bytes: usize, disk_dir: Option<PathBuf>) -> Self {
        Self {
            max_bytes: max_bytes.max(64),
            used_bytes: 0,
            hits: 0,
            misses: 0,
            evictions: 0,
            corruptions_healed: 0,
            disk_dir,
            entries: BTreeMap::new(),
        }
    }

    pub fn store_sample(
        &mut self,
        sound_id: &str,
        sample_rate_hz: u32,
        channels: u8,
        opus_bytes: Vec<u8>,
        pcm_stereo_f32: Vec<f32>,
        now_ms: u64,
    ) -> Result<Stereo48kSampleEntry, SoundboardProtocolError> {
        let key = validate_cache_key(sound_id)?;
        if sample_rate_hz != SAMPLE_RATE_48KHZ || channels != CHANNELS_STEREO {
            return Err(SoundboardProtocolError::InvalidAudioFormat {
                sample_rate_hz,
                channels,
            });
        }
        let byte_len = self.validate_sample_buffers(&opus_bytes, &pcm_stereo_f32)?;
        self.evict_until_fits(&key, byte_len);
        let disk_path = self.persist_opus_to_disk(&key, &opus_bytes)?;
        let frames = pcm_stereo_f32
            .len()
            .div_ceil(INTERLEAVED_STEREO_SAMPLES_20MS);
        let entry = Stereo48kSampleEntry {
            sound_id: key.clone(),
            sample_rate_hz,
            channels,
            frame_count_20ms: frames,
            fnv1a_hash: fnv1a64_checksum(&opus_bytes),
            opus_bytes,
            pcm_stereo_f32,
            disk_path,
            last_accessed_ms: now_ms,
        };
        self.used_bytes = self.used_bytes.saturating_add(byte_len);
        self.entries.insert(key, entry.clone());
        Ok(entry)
    }

    fn validate_sample_buffers(
        &self,
        opus_bytes: &[u8],
        pcm_stereo_f32: &[f32],
    ) -> Result<usize, SoundboardProtocolError> {
        if opus_bytes.is_empty() || pcm_stereo_f32.is_empty() {
            return Err(SoundboardProtocolError::EmptySamplePayload);
        }
        if !pcm_stereo_f32
            .len()
            .is_multiple_of(usize::from(CHANNELS_STEREO))
        {
            return Err(SoundboardProtocolError::UnalignedStereoPcm(
                pcm_stereo_f32.len(),
            ));
        }
        let byte_len = opus_bytes.len();
        if byte_len > MAX_SOUNDBOARD_SOUND_BYTES || byte_len > self.max_bytes {
            return Err(SoundboardProtocolError::SamplePayloadTooLarge(byte_len));
        }
        Ok(byte_len)
    }

    fn persist_opus_to_disk(
        &self,
        key: &str,
        opus_bytes: &[u8],
    ) -> Result<Option<String>, SoundboardProtocolError> {
        let Some(dir) = &self.disk_dir else {
            return Ok(None);
        };
        fs::create_dir_all(dir).map_err(|e| SoundboardProtocolError::DiskIoError(e.to_string()))?;
        let file_path = dir.join(format!("{key}.opus"));
        fs::write(&file_path, opus_bytes)
            .map_err(|e| SoundboardProtocolError::DiskIoError(e.to_string()))?;
        Ok(Some(file_path.to_string_lossy().into_owned()))
    }

    fn evict_until_fits(&mut self, incoming_key: &str, needed_bytes: usize) {
        if let Some(prev) = self.entries.remove(incoming_key) {
            self.used_bytes = self.used_bytes.saturating_sub(prev.opus_bytes.len());
        }
        while self.used_bytes.saturating_add(needed_bytes) > self.max_bytes
            && !self.entries.is_empty()
        {
            let oldest_key = self
                .entries
                .iter()
                .min_by_key(|(_, v)| v.last_accessed_ms)
                .map(|(k, _)| k.clone());
            let Some(k) = oldest_key else {
                break;
            };
            if let Some(removed) = self.entries.remove(&k) {
                self.used_bytes = self.used_bytes.saturating_sub(removed.opus_bytes.len());
                if let Some(dp) = &removed.disk_path {
                    let _ = fs::remove_file(Path::new(dp));
                }
                self.evictions += 1;
            }
        }
    }

    pub fn get_sample(
        &mut self,
        sound_id: &str,
        now_ms: u64,
    ) -> Result<Option<Stereo48kSampleEntry>, SoundboardProtocolError> {
        let key = validate_cache_key(sound_id)?;
        let Some(existing) = self.entries.get(&key).cloned() else {
            self.misses += 1;
            return Ok(None);
        };
        if let Some(dp) = &existing.disk_path
            && let Ok(disk_bytes) = fs::read(Path::new(dp))
        {
            let actual_hash = fnv1a64_checksum(&disk_bytes);
            if actual_hash != existing.fnv1a_hash {
                self.entries.remove(&key);
                self.used_bytes = self.used_bytes.saturating_sub(existing.opus_bytes.len());
                let _ = fs::remove_file(Path::new(dp));
                self.corruptions_healed += 1;
                return Err(SoundboardProtocolError::CorruptedCacheEntry {
                    sound_id: key,
                    expected_hash: existing.fnv1a_hash,
                    actual_hash,
                });
            }
        }
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_accessed_ms = now_ms;
            self.hits += 1;
            return Ok(Some(entry.clone()));
        }
        Ok(None)
    }

    #[must_use]
    pub const fn stats(&self) -> (usize, u64, u64, u64, u64) {
        (
            self.used_bytes,
            self.hits,
            self.misses,
            self.evictions,
            self.corruptions_healed,
        )
    }
}

pub fn evaluate_soundboard_permissions(
    permissions: u64,
    is_dm: bool,
    is_external: bool,
    is_muted_or_deaf: bool,
    age_gated_restricted: bool,
) -> Result<(), SoundboardProtocolError> {
    if age_gated_restricted {
        return Err(SoundboardProtocolError::AgeGatedSpaceRestricted);
    }
    if is_muted_or_deaf {
        return Err(SoundboardProtocolError::UserMutedOrDeafened);
    }
    if !is_dm && (permissions & PERM_CONNECT) == 0 {
        return Err(SoundboardProtocolError::MissingPermission("CONNECT"));
    }
    if !is_dm && (permissions & PERM_SPEAK) == 0 {
        return Err(SoundboardProtocolError::MissingPermission("SPEAK"));
    }
    if !is_dm && (permissions & PERM_USE_SOUNDBOARD) == 0 {
        return Err(SoundboardProtocolError::MissingPermission("USE_SOUNDBOARD"));
    }
    if !is_dm && is_external && (permissions & PERM_USE_EXTERNAL_SOUNDS) == 0 {
        return Err(SoundboardProtocolError::ExternalSoundsNotAllowed);
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
) -> Result<String, SoundboardProtocolError> {
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(SoundboardProtocolError::BoundaryViolation("message_chars"));
    }
    if attachments > MAX_ATTACHMENTS_COUNT {
        return Err(SoundboardProtocolError::BoundaryViolation("attachments"));
    }
    if attachment_bytes > MAX_ATTACHMENT_BYTES {
        return Err(SoundboardProtocolError::BoundaryViolation(
            "attachment_bytes",
        ));
    }
    if search_offset > MAX_SEARCH_OFFSET {
        return Err(SoundboardProtocolError::BoundaryViolation("search_offset"));
    }
    if volume_pct > MAX_VOLUME_PERCENT {
        return Err(SoundboardProtocolError::BoundaryViolation("volume_pct"));
    }
    if favorites_count > MAX_FAVORITE_SOUNDS {
        return Err(SoundboardProtocolError::BoundaryViolation(
            "favorites_count",
        ));
    }
    Ok(format_badge_cap(150))
}

pub fn classify_soundboard_http_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<&'static str, SoundboardProtocolError> {
    match status {
        200 | 201 | 204 => Ok("ok"),
        202 => Ok("skeleton_retry"),
        _ => Err(SoundboardProtocolError::HttpStatusError {
            status,
            retry_after_ms,
        }),
    }
}

#[must_use]
pub fn redact_soundboard_log(raw: &str) -> String {
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
    use std::env;

    #[test]
    fn test_sub_1012a_1_tokens_layout_and_oracle_constants() {
        assert_eq!(SOUNDBOARD_PROTOCOL_CONTAINER, "Soundboard Protocol");
        assert_eq!(SOUNDBOARD_PROTOCOL_DIMENSIONS, "Voice WS");
        assert_eq!(SOUNDBOARD_PROTOCOL_PADDING, "0px");
        assert_eq!(SOUNDBOARD_PROTOCOL_SPACING, "0px");
        assert_eq!(
            (TOKEN_BACKGROUND, TOKEN_SURFACE, TOKEN_ACCENT),
            ("#1e1f22", "#2b2d31", "#5865f2")
        );
        assert_eq!((TOKEN_TEXT_PRIMARY, TOKEN_BORDER), ("#ffffff", "#3f4147"));
        assert_eq!(SFX_TRIGGER_ELEMENT, "sfx-trigger");
        assert_eq!(SfxTriggerState::resolve(false, false).as_str(), "idle");
        assert_eq!(SfxTriggerState::resolve(false, true).as_str(), "preview");
        assert_eq!(
            SfxTriggerState::resolve(true, false).as_str(),
            "playing sfx"
        );
        assert_eq!(RTP_OPUS_PAYLOAD_TYPE, 120);
        assert_eq!(GATEWAY_SOUNDBOARD_SOUNDS_DISPATCH_COUNT, 21);
        assert_eq!((SESSION2_AUDIO_SSRC, SESSION3_AUDIO_SSRC), (17_843, 9_208));
        assert_eq!(MEDIA_ENCRYPTION_MODE, "aead_aes256_gcm_rtpsize");
    }

    #[test]
    fn test_sub_1012a_2_voice_op18_and_op19_speaking_protocol() {
        let (sid, gid, eid) = (
            "1102645616632397904",
            "274093052673392641",
            "1024454092543758356",
        );
        let frame =
            VoiceOp18SoundboardPayload::build(sid, Some(gid), 0.82, Some(eid), None, 17_843);
        assert!(matches!(frame, Ok(ref f) if f.op == 18 && f.to_ws_json().contains("\"op\":18")));
        assert!(VoiceOp18SoundboardPayload::build(sid, None, 1.4, None, None, 1).is_err());
        assert_eq!(SPEAKING_FLAG_SOUND_SHARE, 2);
        let uid = "275258435199827968";
        let ev =
            VoiceOp19SpeakingSoundboardEvent::parse(19, uid, 9_208, 7, Some(sid), Some(gid), 0.99);
        assert!(matches!(
            ev,
            Ok(ref p) if p.is_soundboard_speaking() && p.is_microphone_speaking() && p.is_priority_speaker()
        ));
        let rest = build_send_soundboard_sound_rest(
            "1294815650186788916",
            sid,
            Some(gid),
            Some(eid),
            None,
        );
        assert!(
            matches!(rest, Ok(ref r) if r.method == "POST" && r.path.ends_with("/send-soundboard-sound"))
        );
        assert_eq!(
            build_soundboard_cdn_url(sid),
            Ok(format!(
                "https://cdn.discordapp.com/soundboard-sounds/{sid}"
            ))
        );
    }

    #[test]
    fn test_sub_1012a_3_stereo_48khz_disk_and_lru_cache() {
        let dir = env::temp_dir().join("discord_soundboard_1012a_cache_test");
        let _ = fs::remove_dir_all(&dir);
        let mut cache = SoundboardSampleCache::new(1_000, Some(dir.clone()));
        let (sid1, sid2) = ("1102645616632397904", "1102650064771485746");
        let pcm1 = vec![0.25; 1920];
        let pcm2 = vec![0.5; 3840];
        assert!(
            cache
                .store_sample(sid1, 44_100, 2, vec![1; 100], pcm1.clone(), 10)
                .is_err()
        );
        assert!(
            cache
                .store_sample(sid1, 48_000, 1, vec![1; 100], pcm1.clone(), 10)
                .is_err()
        );
        assert!(
            cache
                .store_sample("../etc", 48_000, 2, vec![1; 100], pcm1.clone(), 10)
                .is_err()
        );
        assert!(
            cache
                .store_sample(sid1, 48_000, 2, vec![0x4f; 600], pcm1, 100)
                .is_ok()
        );
        assert!(
            cache
                .store_sample(sid2, 48_000, 2, vec![0x50; 600], pcm2, 200)
                .is_ok()
        );
        assert_eq!(cache.get_sample(sid1, 300), Ok(None));
        assert!(matches!(cache.get_sample(sid2, 300), Ok(Some(ref s)) if s.frame_count_20ms == 2));
        assert_eq!(cache.stats(), (600, 1, 1, 1, 0));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sub_1012a_4_nine_dimensions_and_corruption_healing() {
        assert!(validate_snowflake_id("0").is_err());
        assert_eq!(validate_snowflake_id("18446744073709551615"), Ok(u64::MAX));
        assert!(is_valid_sound_name("Оу") && !is_valid_sound_name("А"));
        let bp = PERM_CONNECT | PERM_SPEAK;
        assert_eq!(
            evaluate_soundboard_permissions(bp, false, false, false, false),
            Err(SoundboardProtocolError::MissingPermission("USE_SOUNDBOARD"))
        );
        let sp = bp | PERM_USE_SOUNDBOARD;
        assert_eq!(
            evaluate_soundboard_permissions(sp, false, true, false, false),
            Err(SoundboardProtocolError::ExternalSoundsNotAllowed)
        );
        let fp = sp | PERM_USE_EXTERNAL_SOUNDS;
        assert!(evaluate_soundboard_permissions(fp, false, true, false, false).is_ok());
        assert_eq!(
            evaluate_soundboard_permissions(fp, false, false, false, true),
            Err(SoundboardProtocolError::AgeGatedSpaceRestricted)
        );
        let b = validate_platform_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200, 250);
        assert_eq!(b, Ok("99+".to_string()));
        assert_eq!(
            classify_soundboard_http_status(202, 0),
            Ok("skeleton_retry")
        );
        assert!(classify_soundboard_http_status(429, 1_500).is_err());

        let dir = env::temp_dir().join("discord_soundboard_1012a_heal_test");
        let _ = fs::remove_dir_all(&dir);
        let mut cache = SoundboardSampleCache::new(4_096, Some(dir.clone()));
        let sid = "1102645616632397904";
        assert!(
            cache
                .store_sample(sid, 48_000, 2, vec![0x4f; 200], vec![0.1; 1920], 10)
                .is_ok()
        );
        let _ = fs::write(dir.join(format!("{sid}.opus")), b"corrupted_bytes");
        assert!(matches!(
            cache.get_sample(sid, 20),
            Err(SoundboardProtocolError::CorruptedCacheEntry { .. })
        ));
        assert_eq!(cache.stats().4, 1);
        let _ = fs::remove_dir_all(&dir);
        let redacted = redact_soundboard_log("Authorization: Bearer mfa.secret123 token=xyz.987");
        assert!(!redacted.contains("mfa.secret123") && redacted.contains("[REDACTED]"));
    }
}
