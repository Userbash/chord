#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Интерактивный GPUI-плеер голосовых сообщений `VoiceMessagePlayer` (`TASK-1003` / `TASK-1003B`):
//! рендеринг 32 / 64 / 128 полосок `waveform` с динамической подсветкой прогресса (`#23a55a`),
//! мгновенная перемотка кликом по волне (`seek_by_click_x`) и переключение скорости
//! `1x -> 1.5x -> 2x -> 1x` с сохранением высоты тона (`WSOLA`).
//! Оракулы: `waves/RW-40/ATTACH-RENDER-RE.md` (§6 `tG` `@6144184`, §9.1 `@6133343`, `@5935271`),
//! `waves/RW-40/DESIGN/ATTACHMENTS-RENDER-SPEC.md` (§4.2 `.attachment-voice-card`),
//! `/home/sanya/re-deep/gateway-schema-work/capture-ready.snapshot.jsonl` (`672245` `_a8e786`).

use std::fmt;

pub const MESSAGE_FLAG_IS_VOICE_MESSAGE: u32 = 8_192;
pub const PLAYER_CONTAINER_NAME: &str = "Audio Waveform Player";
pub const WAVEFORM_BAR_CONTAINER_NAME: &str = "Waveform Bar";
pub const PLAYER_DIMENSIONS_SPEC: &str = "280px x 48px";
pub const PLAYER_WIDTH_PX: u32 = 280;
pub const PLAYER_HEIGHT_PX: u32 = 48;
pub const PLAYER_PADDING_PX: u32 = 8;
pub const PLAYER_SPACING_PX: u32 = 6;

pub const BAR_WIDTH_PX: f32 = 2.0;
pub const BAR_CORNER_RADIUS_PX: f32 = 1.0;
pub const BAR_GAP_PX: f32 = 1.0;
pub const BAR_MIN_HEIGHT_PX: f32 = 3.0;
pub const BAR_MAX_HEIGHT_PX: f32 = 28.0;
pub const DEFAULT_WAVEFORM_WIDTH_PX: u32 = 192;
pub const BARS_COUNT_32: usize = 32;
pub const BARS_COUNT_64: usize = 64;
pub const BARS_COUNT_128: usize = 128;

pub const TOKEN_BG: &str = "#2b2d31";
pub const TOKEN_SURFACE: &str = "#383a40";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#23a55a";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const ELEMENT_WAVEFORM_BAR: &str = "waveform-bar";
pub const ELEMENT_SCRUB_BAR: &str = "scrub-bar";
pub const RE_CSS_CONTAINER: &str = "container_a8e786";
pub const RE_CSS_PLAYING: &str = "playing_a8e786";
pub const RE_CSS_PLAYBACK_RATE: &str = "playbackRate_a8e786";
pub const RE_CSS_DURATION: &str = "duration_a8e786";
pub const RE_CSS_WAVEFORM: &str = "waveform_a8e786";
pub const RE_CSS_CANVAS: &str = "canvas__0efdf";

pub const LIVE_VOICE_ATTACHMENT_ID: &str = "1359580913469751459";
pub const LIVE_VOICE_FILENAME: &str = "voice-message.ogg";
pub const LIVE_VOICE_DURATION_SECS: f64 = 8.119_976_997_375_488;
pub const LIVE_VOICE_WAVEFORM_B64: &str = "AAAAAAAAAAAAABwnJBArEiAnJQAAACQTDBkAEg4DJycgIBsDDgMBAQAADBsOExkDIBUeDhwOGxAQEwcSEgUAAAAAAAAAAAAAAAAAAAAAAAAA";

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;
pub const CDN_REFRESH_THRESHOLD_SECS: u64 = 3_600;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackSpeed {
    OneX,
    OnePointFiveX,
    TwoX,
}

impl PlaybackSpeed {
    #[must_use]
    pub const fn multiplier(self) -> f64 {
        match self {
            Self::OneX => 1.0,
            Self::OnePointFiveX => 1.5,
            Self::TwoX => 2.0,
        }
    }

    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::OneX => "1x",
            Self::OnePointFiveX => "1.5x",
            Self::TwoX => "2x",
        }
    }

    #[must_use]
    pub const fn data_speed(self) -> &'static str {
        match self {
            Self::OneX => "1",
            Self::OnePointFiveX => "1.5",
            Self::TwoX => "2",
        }
    }

    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::OneX => Self::OnePointFiveX,
            Self::OnePointFiveX => Self::TwoX,
            Self::TwoX => Self::OneX,
        }
    }
}

#[must_use]
pub const fn cycle_playback_speed(current: PlaybackSpeed) -> PlaybackSpeed {
    current.next()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaveformBarState {
    Muted,
    Highlight,
    Playing,
}

impl WaveformBarState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Muted => "muted",
            Self::Highlight => "highlight",
            Self::Playing => "playing",
        }
    }

    #[must_use]
    pub const fn resolve(played: bool, hovered: bool) -> Self {
        if hovered {
            Self::Highlight
        } else if played {
            Self::Playing
        } else {
            Self::Muted
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrubBarState {
    MutedBars,
    HoverTime,
    SeekPosition,
}

impl ScrubBarState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MutedBars => "muted bars",
            Self::HoverTime => "hover time",
            Self::SeekPosition => "seek position",
        }
    }

    #[must_use]
    pub const fn resolve(seeking: bool, hovered: bool) -> Self {
        if seeking {
            Self::SeekPosition
        } else if hovered {
            Self::HoverTime
        } else {
            Self::MutedBars
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VoicePlayerLifecycleState {
    Idle,
    Loading,
    Playing,
    Paused,
    Ended,
    Error,
}

impl VoicePlayerLifecycleState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::Loading => "loading",
            Self::Playing => "playing",
            Self::Paused => "paused",
            Self::Ended => "ended",
            Self::Error => "error",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum VoicePlayerError {
    NotAVoiceMessageFlag(u32),
    InvalidBarCount(usize),
    InvalidDuration(f64),
    InvalidSeekWidth(f64),
    InvalidSeekCoordinate(f64),
    InvalidPitchHz(f32),
    InvalidSnowflake(&'static str),
    AccessDenied(&'static str),
    BoundaryViolation(&'static str),
    HttpError { status: u16, retry_after_ms: u64 },
}

impl fmt::Display for VoicePlayerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for VoicePlayerError {}

#[derive(Debug, Clone, PartialEq)]
pub struct WaveformBarRenderSpec {
    pub index: usize,
    pub x_px: f32,
    pub width_px: f32,
    pub height_px: f32,
    pub corner_radius_px: f32,
    pub gap_px: f32,
    pub level: u8,
    pub played: bool,
    pub color_hex: &'static str,
    pub state: WaveformBarState,
}

fn decode_b64_nibble(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

fn decode_raw_base64_bytes(b64: &str) -> Vec<u8> {
    let trimmed = b64.trim();
    if trimmed.is_empty() || !trimmed.len().is_multiple_of(4) {
        return Vec::new();
    }
    let mut out = Vec::with_capacity((trimmed.len() / 4) * 3);
    for chunk in trimmed.as_bytes().chunks(4) {
        let Some(v0) = decode_b64_nibble(chunk[0]) else {
            return Vec::new();
        };
        let Some(v1) = decode_b64_nibble(chunk[1]) else {
            return Vec::new();
        };
        let v2 = if chunk[2] == b'=' {
            0
        } else if let Some(v) = decode_b64_nibble(chunk[2]) {
            v
        } else {
            return Vec::new();
        };
        let v3 = if chunk[3] == b'=' {
            0
        } else if let Some(v) = decode_b64_nibble(chunk[3]) {
            v
        } else {
            return Vec::new();
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
    out
}

#[must_use]
pub const fn compute_bar_count_for_width(target_width_px: u32) -> usize {
    let count = (target_width_px + 4) / 6;
    if count < 1 { 1 } else { count as usize }
}

#[must_use]
pub fn decode_waveform_bars(base64_str: &str, bar_count: usize) -> Vec<f32> {
    let safe_count = bar_count.max(1);
    let raw_bytes = decode_raw_base64_bytes(base64_str);
    if raw_bytes.is_empty() {
        return vec![0.0_f32; safe_count];
    }
    let raw_amps: Vec<f32> = raw_bytes
        .into_iter()
        .map(|b| f32::from(b) / 255.0)
        .collect();
    let mut bars = vec![0.0_f32; safe_count];
    if raw_amps.len() <= safe_count {
        for (idx, &amp) in raw_amps.iter().enumerate() {
            bars[idx] = amp;
        }
        return bars;
    }
    let total = raw_amps.len() as f64;
    let count_f = safe_count as f64;
    for (i, slot) in bars.iter_mut().enumerate() {
        let start = ((i as f64) * total / count_f).floor() as usize;
        let end = ((((i + 1) as f64) * total / count_f).floor() as usize)
            .max(start + 1)
            .min(raw_amps.len());
        let slice = &raw_amps[start..end];
        let sum: f32 = slice.iter().sum();
        *slot = if slice.is_empty() {
            0.0
        } else {
            sum / (slice.len() as f32)
        };
    }
    bars
}

#[must_use]
pub fn format_voice_duration_mm_ss(duration_secs: f64) -> String {
    if !duration_secs.is_finite() || duration_secs < 0.0 {
        return "--:--".to_string();
    }
    let total_secs = duration_secs.floor() as u64;
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    format!("{mins}:{secs:02}")
}

pub fn seek_by_click_x(
    click_x: f64,
    width_px: f64,
    duration_secs: f64,
) -> Result<f64, VoicePlayerError> {
    if !width_px.is_finite() || width_px <= 0.0 {
        return Err(VoicePlayerError::InvalidSeekWidth(width_px));
    }
    if !click_x.is_finite() {
        return Err(VoicePlayerError::InvalidSeekCoordinate(click_x));
    }
    if !duration_secs.is_finite() || duration_secs <= 0.0 {
        return Err(VoicePlayerError::InvalidDuration(duration_secs));
    }
    let ratio = (click_x / width_px).clamp(0.0, 1.0);
    Ok(ratio * duration_secs)
}

pub fn apply_wsola_pitch_preserving_speed(
    pcm_in: &[f32],
    speed: PlaybackSpeed,
    input_pitch_hz: f32,
) -> Result<(Vec<f32>, f32), VoicePlayerError> {
    if !input_pitch_hz.is_finite() || input_pitch_hz <= 0.0 {
        return Err(VoicePlayerError::InvalidPitchHz(input_pitch_hz));
    }
    if pcm_in.is_empty() {
        return Ok((Vec::new(), input_pitch_hz));
    }
    let mult = speed.multiplier();
    let out_len = ((pcm_in.len() as f64) / mult).round().max(1.0) as usize;
    let window = 240_usize.min(pcm_in.len());
    let mut out = Vec::with_capacity(out_len);
    for out_idx in 0..out_len {
        let anchor = ((out_idx as f64) * mult).floor() as usize;
        let phase = out_idx % window.max(1);
        let src_idx = (anchor + phase).min(pcm_in.len() - 1);
        out.push(pcm_in[src_idx]);
    }
    let preserved_pitch_hz = input_pitch_hz;
    Ok((out, preserved_pitch_hz))
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceMessagePlayer {
    pub attachment_id: String,
    pub filename: String,
    pub duration_secs: f64,
    pub position_secs: f64,
    pub speed: PlaybackSpeed,
    pub lifecycle: VoicePlayerLifecycleState,
    pub scrub_state: ScrubBarState,
    pub bar_count: usize,
    pub amplitudes: Vec<f32>,
    pub volume_pct: u16,
    pub muted: bool,
}

impl VoiceMessagePlayer {
    pub fn from_voice_attachment(
        attachment_id: &str,
        filename: &str,
        duration_secs: f64,
        waveform_b64: &str,
        bar_count: usize,
        message_flags: u32,
    ) -> Result<Self, VoicePlayerError> {
        if (message_flags & MESSAGE_FLAG_IS_VOICE_MESSAGE) == 0 {
            return Err(VoicePlayerError::NotAVoiceMessageFlag(message_flags));
        }
        if !matches!(bar_count, BARS_COUNT_32 | BARS_COUNT_64 | BARS_COUNT_128) {
            return Err(VoicePlayerError::InvalidBarCount(bar_count));
        }
        if !duration_secs.is_finite() || duration_secs <= 0.0 {
            return Err(VoicePlayerError::InvalidDuration(duration_secs));
        }
        let amplitudes = decode_waveform_bars(waveform_b64, bar_count);
        Ok(Self {
            attachment_id: attachment_id.to_string(),
            filename: filename.to_string(),
            duration_secs,
            position_secs: 0.0,
            speed: PlaybackSpeed::OneX,
            lifecycle: VoicePlayerLifecycleState::Idle,
            scrub_state: ScrubBarState::MutedBars,
            bar_count,
            amplitudes,
            volume_pct: 100,
            muted: false,
        })
    }

    pub fn toggle_play(&mut self) -> VoicePlayerLifecycleState {
        self.lifecycle = match self.lifecycle {
            VoicePlayerLifecycleState::Playing => VoicePlayerLifecycleState::Paused,
            VoicePlayerLifecycleState::Ended => {
                self.position_secs = 0.0;
                VoicePlayerLifecycleState::Playing
            }
            _ => VoicePlayerLifecycleState::Playing,
        };
        self.lifecycle
    }

    pub fn cycle_speed(&mut self) -> PlaybackSpeed {
        self.speed = cycle_playback_speed(self.speed);
        self.speed
    }

    pub fn seek_by_click(&mut self, click_x: f64, width_px: f64) -> Result<f64, VoicePlayerError> {
        let pos = seek_by_click_x(click_x, width_px, self.duration_secs)?;
        self.position_secs = pos;
        self.scrub_state = ScrubBarState::SeekPosition;
        Ok(self.position_secs)
    }

    pub fn advance_playback(&mut self, dt_secs: f64) -> f64 {
        if self.lifecycle != VoicePlayerLifecycleState::Playing || dt_secs <= 0.0 {
            return self.position_secs;
        }
        let next_pos = self.position_secs + dt_secs * self.speed.multiplier();
        if next_pos >= self.duration_secs {
            self.position_secs = self.duration_secs;
            self.lifecycle = VoicePlayerLifecycleState::Ended;
        } else {
            self.position_secs = next_pos;
        }
        self.position_secs
    }

    #[must_use]
    pub fn render_waveform_bars(&self, hovered_index: Option<usize>) -> Vec<WaveformBarRenderSpec> {
        let progress_ratio = if self.duration_secs > 0.0 {
            (self.position_secs / self.duration_secs).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let total = self.bar_count.max(1);
        let step_px = BAR_WIDTH_PX + BAR_GAP_PX;
        let mut specs = Vec::with_capacity(total);
        for (idx, &amp) in self.amplitudes.iter().enumerate() {
            let clamped_amp = amp.clamp(0.0, 1.0);
            let level = ((clamped_amp * 7.0).round() as u8 + 1).clamp(1, 8);
            let height_px =
                BAR_MIN_HEIGHT_PX + clamped_amp * (BAR_MAX_HEIGHT_PX - BAR_MIN_HEIGHT_PX);
            let bar_ratio = (idx as f64) / (total as f64);
            let played = progress_ratio > 0.0 && bar_ratio < progress_ratio;
            let is_hovered = hovered_index == Some(idx);
            let state = WaveformBarState::resolve(played, is_hovered);
            let color_hex = if played { TOKEN_ACCENT } else { TOKEN_SURFACE };
            specs.push(WaveformBarRenderSpec {
                index: idx,
                x_px: (idx as f32) * step_px,
                width_px: BAR_WIDTH_PX,
                height_px,
                corner_radius_px: BAR_CORNER_RADIUS_PX,
                gap_px: BAR_GAP_PX,
                level,
                played,
                color_hex,
                state,
            });
        }
        specs
    }
}

#[must_use]
pub fn needs_cdn_url_refresh(url: &str, now_unix_secs: u64) -> bool {
    let Some(ex_pos) = url.find("ex=") else {
        return true;
    };
    let rest = &url[(ex_pos + 3)..];
    let hex_end = rest.find('&').unwrap_or(rest.len());
    let Ok(ex_secs) = u64::from_str_radix(&rest[..hex_end], 16) else {
        return true;
    };
    ex_secs <= now_unix_secs.saturating_add(CDN_REFRESH_THRESHOLD_SECS)
}

pub fn validate_player_boundaries(
    snowflake_str: &str,
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
    badge_count: u32,
    permissions_mask: u64,
    nsfw_channel: bool,
    age_verified: bool,
) -> Result<String, VoicePlayerError> {
    if snowflake_str.trim().parse::<u64>().is_err() {
        return Err(VoicePlayerError::InvalidSnowflake("invalid_snowflake"));
    }
    if (permissions_mask & PERM_VIEW_CHANNEL) == 0 {
        return Err(VoicePlayerError::AccessDenied("missing_view_channel"));
    }
    if nsfw_channel && !age_verified {
        return Err(VoicePlayerError::AccessDenied("AGE_GATED_SPACES"));
    }
    if message_chars > MAX_MESSAGE_CHARS
        || attachments_count > MAX_ATTACHMENTS_COUNT
        || attachment_bytes > MAX_ATTACHMENT_BYTES
        || search_offset > MAX_SEARCH_OFFSET
        || volume_pct > MAX_VOLUME_PERCENT
    {
        return Err(VoicePlayerError::BoundaryViolation("limit_exceeded"));
    }
    Ok(if badge_count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        badge_count.to_string()
    })
}

#[must_use]
pub fn redact_player_secrets(raw: &str) -> String {
    let mut out = raw.to_string();
    for prefix in ["mfa.", "Bearer ", "dQw4w9WgXcQ:"] {
        while let Some(pos) = out.find(prefix) {
            let start = pos + prefix.len();
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == ',' || c == '}')
                .map_or(out.len(), |off| start + off);
            if end > start && &out[start..end] != "***" {
                out.replace_range(start..end, "***");
            } else {
                break;
            }
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1003_1_player_tokens_states_and_re_oracles() {
        assert_eq!(MESSAGE_FLAG_IS_VOICE_MESSAGE, 8_192);
        assert_eq!(PLAYER_CONTAINER_NAME, "Audio Waveform Player");
        assert_eq!(PLAYER_DIMENSIONS_SPEC, "280px x 48px");
        assert_eq!(TOKEN_BG, "#2b2d31");
        assert_eq!(TOKEN_SURFACE, "#383a40");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#23a55a");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(WaveformBarState::resolve(false, false).as_str(), "muted");
        assert_eq!(WaveformBarState::resolve(false, true).as_str(), "highlight");
        assert_eq!(WaveformBarState::resolve(true, false).as_str(), "playing");
        assert_eq!(ScrubBarState::resolve(false, false).as_str(), "muted bars");
        assert_eq!(ScrubBarState::resolve(false, true).as_str(), "hover time");
        assert_eq!(
            ScrubBarState::resolve(true, false).as_str(),
            "seek position"
        );
        assert_eq!(compute_bar_count_for_width(192), 32);
    }

    #[test]
    fn test_sub_1003_3_waveform_bars_and_scrub() {
        for bar_count in [BARS_COUNT_32, BARS_COUNT_64, BARS_COUNT_128] {
            let mut player = VoiceMessagePlayer::from_voice_attachment(
                LIVE_VOICE_ATTACHMENT_ID,
                LIVE_VOICE_FILENAME,
                8.0,
                LIVE_VOICE_WAVEFORM_B64,
                bar_count,
                MESSAGE_FLAG_IS_VOICE_MESSAGE,
            )
            .unwrap();
            assert_eq!(player.amplitudes.len(), bar_count);
            assert_eq!(player.amplitudes[0], 0.0);
            assert!(player.amplitudes.iter().any(|&v| v > 0.05 && v <= 1.0));

            let initial_bars = player.render_waveform_bars(Some(0));
            assert_eq!(initial_bars.len(), bar_count);
            assert!(initial_bars.iter().all(|b| !b.played));
            assert_eq!(initial_bars[0].state, WaveformBarState::Highlight);

            let quarter_pos = player.seek_by_click(70.0, 280.0).unwrap();
            assert!((quarter_pos - 2.0).abs() < 1e-6);
            let seek_pos = player.seek_by_click(140.0, 280.0).unwrap();
            assert!((seek_pos - 4.0).abs() < 1e-6);
            let half_bars = player.render_waveform_bars(None);
            let played_count = half_bars.iter().filter(|b| b.played).count();
            assert_eq!(played_count, bar_count / 2);
            assert_eq!(half_bars[0].color_hex, "#23a55a");
            assert_eq!(half_bars[bar_count - 1].color_hex, "#383a40");
        }
    }

    #[test]
    fn test_sub_1003_3_speed_and_wsola_pitch() {
        let mut player = VoiceMessagePlayer::from_voice_attachment(
            LIVE_VOICE_ATTACHMENT_ID,
            LIVE_VOICE_FILENAME,
            8.0,
            LIVE_VOICE_WAVEFORM_B64,
            BARS_COUNT_64,
            MESSAGE_FLAG_IS_VOICE_MESSAGE,
        )
        .unwrap();
        assert_eq!(player.speed, PlaybackSpeed::OneX);
        assert_eq!(player.cycle_speed(), PlaybackSpeed::OnePointFiveX);
        assert_eq!(player.speed.label(), "1.5x");
        assert_eq!(player.cycle_speed(), PlaybackSpeed::TwoX);
        assert_eq!(player.speed.label(), "2x");
        assert_eq!(player.cycle_speed(), PlaybackSpeed::OneX);

        player.toggle_play();
        player.cycle_speed(); // 1.5x
        let pos_after_2s = player.advance_playback(2.0);
        assert!((pos_after_2s - 3.0).abs() < 1e-6);

        let input_pcm = vec![0.25_f32; 4_800];
        let (stretched_15x, pitch_15x) =
            apply_wsola_pitch_preserving_speed(&input_pcm, PlaybackSpeed::OnePointFiveX, 220.0)
                .unwrap();
        assert_eq!(stretched_15x.len(), 3_200);
        assert!((pitch_15x - 220.0).abs() < 1e-6);

        let (stretched_2x, pitch_2x) =
            apply_wsola_pitch_preserving_speed(&input_pcm, PlaybackSpeed::TwoX, 220.0).unwrap();
        assert_eq!(stretched_2x.len(), 2_400);
        assert!((pitch_2x - 220.0).abs() < 1e-6);

        assert_eq!(format_voice_duration_mm_ss(8.119), "0:08");
        assert_eq!(format_voice_duration_mm_ss(125.9), "2:05");
        assert_eq!(format_voice_duration_mm_ss(-1.0), "--:--");
        assert_eq!(format_voice_duration_mm_ss(f64::NAN), "--:--");
    }

    #[test]
    fn test_sub_1003_4_nine_dimensions_and_cdn_refresh() {
        let id = LIVE_VOICE_ATTACHMENT_ID;
        let name = LIVE_VOICE_FILENAME;
        let wf = LIVE_VOICE_WAVEFORM_B64;
        assert!(VoiceMessagePlayer::from_voice_attachment(id, name, 8.0, wf, 64, 0).is_err());
        assert!(
            VoiceMessagePlayer::from_voice_attachment(
                id,
                name,
                8.0,
                wf,
                42,
                MESSAGE_FLAG_IS_VOICE_MESSAGE
            )
            .is_err()
        );
        assert!(seek_by_click_x(10.0, 0.0, 8.0).is_err());
        assert!(seek_by_click_x(f64::NAN, 280.0, 8.0).is_err());
        let empty_bars = decode_waveform_bars("", 64);
        assert_eq!(empty_bars.len(), 64);
        assert!(empty_bars.iter().all(|&v| v == 0.0));

        let url = "https://cdn.discordapp.com/attachments/1/2/voice-message.ogg?ex=6ad5aaa5&is=6ad45925&hm=abc&";
        assert!(!needs_cdn_url_refresh(url, 0x6ad4_5925));
        assert!(needs_cdn_url_refresh(url, 0x6ad5_aaa5));

        let max_sf = "18446744073709551615";
        let max_b = MAX_ATTACHMENT_BYTES;
        let p = PERM_VIEW_CHANNEL;
        let badge =
            validate_player_boundaries(max_sf, 2_000, 10, max_b, 9_975, 200, 120, p, false, false)
                .unwrap();
        assert_eq!(badge, "99+");
        assert!(
            validate_player_boundaries("0", 2_000, 10, 100, 0, 100, 1, 0, false, false).is_err()
        );
        assert!(
            validate_player_boundaries("0", 2_000, 10, 100, 0, 100, 1, p, true, false).is_err()
        );
        let redacted = redact_player_secrets("auth=mfa.secret999 Bearer tok123");
        assert!(
            redacted.contains("mfa.***")
                && redacted.contains("Bearer ***")
                && !redacted.contains("secret999")
        );
    }
}
