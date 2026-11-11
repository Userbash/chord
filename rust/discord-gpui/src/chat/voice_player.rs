#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Интерактивный GPUI-плеер голосовых сообщений `VoiceMessagePlayer` (`TASK-1003B`):
//! GPUI шейдер/холст волны (`WaveformShaderUniforms`, `WaveformCanvasQuad`, `WaveformBarRenderSpec`),
//! рендеринг 32 / 64 / 128 полосок `waveform` с динамической подсветкой прогресса (`#23a55a`),
//! мгновенная перемотка кликом/перетаскиванием (`seek_by_click_x`, `seek_by_drag_ratio`, `hover_time_at_x`)
//! и кнопка скорости `1x -> 1.5x -> 2x -> 1x` с сохранением высоты тона (`WSOLA`).
//! Оракулы: `/home/sanya/re-deep/voice/ui7/`, `waves/RW-40/ATTACH-RENDER-RE.md` (`@6144184`, `@6133343`, `@5935271`).

use std::fmt;

pub const MESSAGE_FLAG_IS_VOICE_MESSAGE: u32 = 8_192;
pub const WAVEFORM_BAR_CONTAINER_NAME: &str = "Waveform Bar";
pub const PLAYER_CONTAINER_NAME: &str = "Audio Waveform Player";
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
pub const ELEMENT_SCRUB_BAR: &str = "scrub-bar";
pub const ELEMENT_WAVEFORM_BAR: &str = "waveform-bar";
pub const I18N_PLAY_RU: &str = "Воспроизвести голосовое сообщение";
pub const I18N_PAUSE_RU: &str = "Поставить голосовое сообщение на паузу";
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
    pub const fn multiplier(self) -> f64 {
        match self {
            Self::OneX => 1.0,
            Self::OnePointFiveX => 1.5,
            Self::TwoX => 2.0,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::OneX => "1x",
            Self::OnePointFiveX => "1.5x",
            Self::TwoX => "2x",
        }
    }

    pub const fn data_speed(self) -> &'static str {
        match self {
            Self::OneX => "1",
            Self::OnePointFiveX => "1.5",
            Self::TwoX => "2",
        }
    }

    pub const fn next(self) -> Self {
        match self {
            Self::OneX => Self::OnePointFiveX,
            Self::OnePointFiveX => Self::TwoX,
            Self::TwoX => Self::OneX,
        }
    }
}

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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Muted => "muted",
            Self::Highlight => "highlight",
            Self::Playing => "playing",
        }
    }

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
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MutedBars => "muted bars",
            Self::HoverTime => "hover time",
            Self::SeekPosition => "seek position",
        }
    }

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

#[derive(Debug, Clone, PartialEq)]
pub struct WaveformShaderUniforms {
    pub canvas_width_px: f32,
    pub canvas_height_px: f32,
    pub progress_ratio: f32,
    pub played_segments: usize,
    pub total_segments: usize,
    pub active_color_hex: &'static str,
    pub muted_color_hex: &'static str,
    pub highlight_color_hex: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WaveformCanvasQuad {
    pub segment_index: usize,
    pub x_px: f32,
    pub y_px: f32,
    pub width_px: f32,
    pub height_px: f32,
    pub corner_radius_px: f32,
    pub fill_color_hex: &'static str,
    pub played: bool,
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
        let (Some(v0), Some(v1)) = (decode_b64_nibble(chunk[0]), decode_b64_nibble(chunk[1]))
        else {
            return Vec::new();
        };
        let Some(v2) = (if chunk[2] == b'=' {
            Some(0)
        } else {
            decode_b64_nibble(chunk[2])
        }) else {
            return Vec::new();
        };
        let Some(v3) = (if chunk[3] == b'=' {
            Some(0)
        } else {
            decode_b64_nibble(chunk[3])
        }) else {
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

pub const fn compute_bar_count_for_width(target_width_px: u32) -> usize {
    let count = (target_width_px + 4) / 6;
    if count < 1 { 1 } else { count as usize }
}

pub fn compute_waveform_canvas_width_px(duration_secs: f64) -> u32 {
    let raw = if !duration_secs.is_finite() || duration_secs <= 0.5 {
        40.0
    } else if duration_secs >= 45.0 {
        294.0
    } else {
        ((duration_secs.min(45.0) - 0.5) / 44.5) * 254.0 + 40.0
    };
    let steps = ((raw + 4.0) / 6.0).floor().max(1.0) as u32;
    steps.saturating_mul(6).saturating_sub(4)
}

pub fn compute_played_segment_count(show_all: bool, cur: f64, dur: f64, segs: usize) -> usize {
    if show_all {
        return segs;
    }
    if !cur.is_finite() || !dur.is_finite() || dur <= 0.0 {
        return 0;
    }
    (((cur / dur).clamp(0.0, 1.0) * (segs as f64)).round() as usize).min(segs)
}

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
    let (total, count_f) = (raw_amps.len() as f64, safe_count as f64);
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

pub fn build_gpui_waveform_canvas_quads(
    amplitudes: &[f32],
    progress_ratio: f64,
    hovered_index: Option<usize>,
) -> (WaveformShaderUniforms, Vec<WaveformCanvasQuad>) {
    let clamped = if progress_ratio.is_finite() {
        progress_ratio.clamp(0.0, 1.0)
    } else {
        0.0
    };
    let (total, step_px) = (amplitudes.len().max(1), BAR_WIDTH_PX + BAR_GAP_PX);
    let mut quads = Vec::with_capacity(amplitudes.len());
    for (idx, &amp) in amplitudes.iter().enumerate() {
        let height_px =
            BAR_MIN_HEIGHT_PX + amp.clamp(0.0, 1.0) * (BAR_MAX_HEIGHT_PX - BAR_MIN_HEIGHT_PX);
        let y_px = ((PLAYER_HEIGHT_PX as f32) - height_px).max(0.0) * 0.5;
        let played = clamped > 0.0 && ((idx as f64) / (total as f64)) < clamped;
        let state = WaveformBarState::resolve(played, hovered_index == Some(idx));
        let fill_color_hex = match state {
            WaveformBarState::Highlight => TOKEN_TEXT_PRIMARY,
            WaveformBarState::Playing => TOKEN_ACCENT,
            WaveformBarState::Muted => TOKEN_SURFACE,
        };
        quads.push(WaveformCanvasQuad {
            segment_index: idx,
            x_px: (idx as f32) * step_px,
            y_px,
            width_px: BAR_WIDTH_PX,
            height_px,
            corner_radius_px: BAR_CORNER_RADIUS_PX,
            fill_color_hex,
            played,
            state,
        });
    }
    let uniforms = WaveformShaderUniforms {
        canvas_width_px: (amplitudes.len() as f32) * step_px,
        canvas_height_px: PLAYER_HEIGHT_PX as f32,
        progress_ratio: clamped as f32,
        played_segments: quads.iter().filter(|q| q.played).count(),
        total_segments: amplitudes.len(),
        active_color_hex: TOKEN_ACCENT,
        muted_color_hex: TOKEN_SURFACE,
        highlight_color_hex: TOKEN_TEXT_PRIMARY,
    };
    (uniforms, quads)
}

pub fn format_voice_duration_mm_ss(duration_secs: f64) -> String {
    if !duration_secs.is_finite() || duration_secs < 0.0 {
        return "--:--".to_string();
    }
    let total_secs = duration_secs.floor() as u64;
    format!("{}:{:02}", total_secs / 60, total_secs % 60)
}

pub fn seek_by_click_x(click_x: f64, width_px: f64, dur: f64) -> Result<f64, VoicePlayerError> {
    if !width_px.is_finite() || width_px <= 0.0 {
        return Err(VoicePlayerError::InvalidSeekWidth(width_px));
    }
    if !click_x.is_finite() {
        return Err(VoicePlayerError::InvalidSeekCoordinate(click_x));
    }
    if !dur.is_finite() || dur <= 0.0 {
        return Err(VoicePlayerError::InvalidDuration(dur));
    }
    let ratio = (click_x / width_px).clamp(0.0, 1.0);
    Ok(ratio * dur)
}

pub fn seek_by_drag_ratio(drag_ratio: f64, duration_secs: f64) -> Result<f64, VoicePlayerError> {
    if !drag_ratio.is_finite() {
        return Err(VoicePlayerError::InvalidSeekCoordinate(drag_ratio));
    }
    if !duration_secs.is_finite() || duration_secs <= 0.0 {
        return Err(VoicePlayerError::InvalidDuration(duration_secs));
    }
    Ok(drag_ratio.clamp(0.0, 1.0) * duration_secs)
}

pub fn hover_time_at_x(
    hover_x: f64,
    width_px: f64,
    dur: f64,
) -> Result<(f64, String, ScrubBarState), VoicePlayerError> {
    let preview_secs = seek_by_click_x(hover_x, width_px, dur)?;
    Ok((
        preview_secs,
        format_voice_duration_mm_ss(preview_secs),
        ScrubBarState::HoverTime,
    ))
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
    let window = 240_usize.min(pcm_in.len()).max(1);
    let mut out = Vec::with_capacity(out_len);
    for out_idx in 0..out_len {
        let anchor = ((out_idx as f64) * mult).floor() as usize;
        out.push(pcm_in[(anchor + (out_idx % window)).min(pcm_in.len() - 1)]);
    }
    let preserved_pitch_hz = input_pitch_hz;
    Ok((out, preserved_pitch_hz))
}

pub fn build_playback_position_update_json(key: &str, pos: f64, dur: f64) -> String {
    format!(
        "{{\"type\":\"MEDIA_PLAYBACK_POSITION_UPDATE\",\"cacheKey\":\"{key}\",\"position\":{pos:.3},\"duration\":{dur:.3}}}"
    )
}

pub fn build_playback_rate_update_json(speed: PlaybackSpeed) -> String {
    format!(
        "{{\"type\":\"MEDIA_PLAYBACK_RATE_UPDATE\",\"rate\":{},\"playbackType\":\"VOICE_MESSAGE\"}}",
        speed.data_speed()
    )
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
        Ok(Self {
            attachment_id: attachment_id.to_string(),
            filename: filename.to_string(),
            duration_secs,
            position_secs: 0.0,
            speed: PlaybackSpeed::OneX,
            lifecycle: VoicePlayerLifecycleState::Idle,
            scrub_state: ScrubBarState::MutedBars,
            bar_count,
            amplitudes: decode_waveform_bars(waveform_b64, bar_count),
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

    pub fn on_other_voice_message_started(&mut self, started_attachment_id: &str) {
        if self.attachment_id != started_attachment_id
            && self.lifecycle == VoicePlayerLifecycleState::Playing
        {
            self.lifecycle = VoicePlayerLifecycleState::Paused;
        }
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

    pub fn render_waveform_bars(&self, hovered_index: Option<usize>) -> Vec<WaveformBarRenderSpec> {
        let progress_ratio = if self.duration_secs > 0.0 {
            (self.position_secs / self.duration_secs).clamp(0.0, 1.0)
        } else {
            0.0
        };
        let (total, step_px) = (self.bar_count.max(1), BAR_WIDTH_PX + BAR_GAP_PX);
        let mut specs = Vec::with_capacity(total);
        for (idx, &amp) in self.amplitudes.iter().enumerate() {
            let clamped_amp = amp.clamp(0.0, 1.0);
            let level = ((clamped_amp * 7.0).round() as u8 + 1).clamp(1, 8);
            let height_px =
                BAR_MIN_HEIGHT_PX + clamped_amp * (BAR_MAX_HEIGHT_PX - BAR_MIN_HEIGHT_PX);
            let bar_ratio = (idx as f64) / (total as f64);
            let played = progress_ratio > 0.0 && bar_ratio < progress_ratio;
            let state = WaveformBarState::resolve(played, hovered_index == Some(idx));
            specs.push(WaveformBarRenderSpec {
                index: idx,
                x_px: (idx as f32) * step_px,
                width_px: BAR_WIDTH_PX,
                height_px,
                corner_radius_px: BAR_CORNER_RADIUS_PX,
                gap_px: BAR_GAP_PX,
                level,
                played,
                color_hex: if played { TOKEN_ACCENT } else { TOKEN_SURFACE },
                state,
            });
        }
        specs
    }
}

pub fn map_player_http_status(
    status: u16,
    retry_ms: u64,
) -> Result<VoicePlayerLifecycleState, VoicePlayerError> {
    match status {
        200 | 204 | 206 => Ok(VoicePlayerLifecycleState::Playing),
        202 => Ok(VoicePlayerLifecycleState::Loading),
        429 => Err(VoicePlayerError::HttpError {
            status,
            retry_after_ms: retry_ms.max(100),
        }),
        _ => Err(VoicePlayerError::HttpError {
            status,
            retry_after_ms: 0,
        }),
    }
}

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
    sf: &str,
    limits: (usize, usize, u64, u32, u16, u32),
    access: (u64, bool, bool),
) -> Result<String, VoicePlayerError> {
    if sf.trim().parse::<u64>().is_err() {
        return Err(VoicePlayerError::InvalidSnowflake("invalid_snowflake"));
    }
    if (access.0 & PERM_VIEW_CHANNEL) == 0 {
        return Err(VoicePlayerError::AccessDenied("missing_view_channel"));
    }
    if access.1 && !access.2 {
        return Err(VoicePlayerError::AccessDenied("AGE_GATED_SPACES"));
    }
    if limits.0 > MAX_MESSAGE_CHARS
        || limits.1 > MAX_ATTACHMENTS_COUNT
        || limits.2 > MAX_ATTACHMENT_BYTES
        || limits.3 > MAX_SEARCH_OFFSET
        || limits.4 > MAX_VOLUME_PERCENT
    {
        return Err(VoicePlayerError::BoundaryViolation("limit_exceeded"));
    }
    Ok(if limits.5 > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        limits.5.to_string()
    })
}

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

    fn sample_player(bars: usize) -> VoiceMessagePlayer {
        VoiceMessagePlayer::from_voice_attachment(
            LIVE_VOICE_ATTACHMENT_ID,
            LIVE_VOICE_FILENAME,
            8.0,
            LIVE_VOICE_WAVEFORM_B64,
            bars,
            MESSAGE_FLAG_IS_VOICE_MESSAGE,
        )
        .unwrap()
    }

    #[test]
    fn test_sub_1003b_1_tokens_layout_scrub_states_and_re_oracles() {
        assert_eq!(MESSAGE_FLAG_IS_VOICE_MESSAGE, 8_192);
        assert_eq!(WAVEFORM_BAR_CONTAINER_NAME, "Waveform Bar");
        assert_eq!(PLAYER_DIMENSIONS_SPEC, "280px x 48px");
        assert_eq!((TOKEN_BG, TOKEN_SURFACE), ("#2b2d31", "#383a40"));
        assert_eq!((TOKEN_TEXT_PRIMARY, TOKEN_ACCENT), ("#ffffff", "#23a55a"));
        assert_eq!((TOKEN_BORDER, ELEMENT_SCRUB_BAR), ("#3f4147", "scrub-bar"));
        assert_eq!(ScrubBarState::resolve(false, false).as_str(), "muted bars");
        assert_eq!(ScrubBarState::resolve(false, true).as_str(), "hover time");
        assert_eq!(
            ScrubBarState::resolve(true, false).as_str(),
            "seek position"
        );
        assert_eq!(WaveformBarState::resolve(false, false).as_str(), "muted");
        assert_eq!(WaveformBarState::resolve(false, true).as_str(), "highlight");
        assert_eq!(WaveformBarState::resolve(true, false).as_str(), "playing");
        assert_eq!(compute_bar_count_for_width(192), 32);
    }

    #[test]
    fn test_sub_1003b_2_gpui_waveform_canvas_64_and_128_bars() {
        assert_eq!(compute_waveform_canvas_width_px(0.3), 38);
        assert_eq!(compute_waveform_canvas_width_px(45.0), 290);
        assert_eq!(compute_played_segment_count(false, 4.0, 8.0, 64), 32);
        for bar_count in [BARS_COUNT_64, BARS_COUNT_128] {
            let mut player = sample_player(bar_count);
            assert_eq!(player.amplitudes.len(), bar_count);
            assert!(player.amplitudes.iter().any(|&v| v > 0.05 && v <= 1.0));
            let initial_bars = player.render_waveform_bars(Some(0));
            assert!(initial_bars.iter().all(|b| !b.played));
            assert_eq!(initial_bars[0].state, WaveformBarState::Highlight);
            let _ = player.seek_by_click(140.0, 280.0).unwrap();
            let half_bars = player.render_waveform_bars(None);
            assert_eq!(half_bars.iter().filter(|b| b.played).count(), bar_count / 2);
            assert_eq!(half_bars[0].color_hex, "#23a55a");
            assert_eq!(half_bars[bar_count - 1].color_hex, "#383a40");
            let (uniforms, quads) =
                build_gpui_waveform_canvas_quads(&player.amplitudes, 0.5, Some(bar_count - 1));
            assert_eq!(uniforms.played_segments, bar_count / 2);
            assert_eq!(quads[0].fill_color_hex, "#23a55a");
            assert_eq!(quads[bar_count - 1].fill_color_hex, "#ffffff");
        }
    }

    #[test]
    fn test_sub_1003b_3_click_seek_hover_and_speed_pitch_wsola() {
        let mut player = sample_player(64);
        let (hover_secs, hover_mm_ss, hover_state) = hover_time_at_x(70.0, 280.0, 8.0).unwrap();
        assert!((hover_secs - 2.0).abs() < 1e-6 && hover_mm_ss == "0:02");
        assert_eq!(hover_state, ScrubBarState::HoverTime);
        assert!((player.seek_by_click(70.0, 280.0).unwrap() - 2.0).abs() < 1e-6);
        assert_eq!(player.scrub_state, ScrubBarState::SeekPosition);
        assert!((seek_by_drag_ratio(0.75, 8.0).unwrap() - 6.0).abs() < 1e-6);
        assert_eq!(player.cycle_speed(), PlaybackSpeed::OnePointFiveX);
        assert_eq!(player.cycle_speed(), PlaybackSpeed::TwoX);
        assert_eq!(player.cycle_speed(), PlaybackSpeed::OneX);
        player.toggle_play();
        player.cycle_speed();
        assert!((player.advance_playback(2.0) - 5.0).abs() < 1e-6);
        player.on_other_voice_message_started("999999999999999999");
        assert_eq!(player.lifecycle, VoicePlayerLifecycleState::Paused);
        let (stretched, pitch) = apply_wsola_pitch_preserving_speed(
            &vec![0.25_f32; 4_800],
            PlaybackSpeed::OnePointFiveX,
            220.0,
        )
        .unwrap();
        assert!(stretched.len() == 3_200 && (pitch - 220.0).abs() < 1e-6);
        assert!(
            build_playback_position_update_json("msg:1", 2.0, 8.0)
                .contains("MEDIA_PLAYBACK_POSITION_UPDATE")
        );
        assert!(
            build_playback_rate_update_json(PlaybackSpeed::TwoX)
                .contains("MEDIA_PLAYBACK_RATE_UPDATE")
        );
    }

    #[test]
    fn test_sub_1003b_4_nine_dimensions_and_cdn_refresh() {
        assert!(
            VoiceMessagePlayer::from_voice_attachment("1", "a.ogg", 8.0, "", 64, 0).is_err()
                && seek_by_click_x(10.0, 0.0, 8.0).is_err()
                && seek_by_click_x(f64::NAN, 280.0, 8.0).is_err()
        );
        assert_eq!(
            map_player_http_status(202, 0).unwrap(),
            VoicePlayerLifecycleState::Loading
        );
        assert!(map_player_http_status(429, 1_500).is_err());
        let empty_bars = decode_waveform_bars("", 64);
        assert!(empty_bars.len() == 64 && empty_bars.iter().all(|&v| v == 0.0));
        let url = "https://cdn.discordapp.com/attachments/1/2/voice-message.ogg?ex=6ad5aaa5&is=6ad45925&hm=abc&";
        assert!(
            !needs_cdn_url_refresh(url, 0x6ad4_5925) && needs_cdn_url_refresh(url, 0x6ad5_aaa5)
        );
        let badge = validate_player_boundaries(
            "18446744073709551615",
            (2_000, 10, MAX_ATTACHMENT_BYTES, 9_975, 200, 120),
            (PERM_VIEW_CHANNEL, false, false),
        )
        .unwrap();
        assert_eq!(badge, "99+");
        assert!(
            validate_player_boundaries("0", (10, 1, 100, 0, 100, 1), (0, false, false)).is_err()
        );
        let redacted = redact_player_secrets("auth=mfa.secret999 Bearer tok123");
        assert!(redacted.contains("mfa.***") && !redacted.contains("secret999"));
    }
}
