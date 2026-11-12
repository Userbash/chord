//! Продвинутая WebRTC-диагностика и окно `RTC Internals HUD` (`TASK-1011` / `TASK-1011A` / `TASK-1011B` — `RE-GAP-22`).
//! Оракулы: `waves/kanban-board.json`, `waves/RW-40/RE-AUDIT/RE-GAP-22-VOICE-DIAGNOSTICS.md`, `docs/re/voice-deep.json`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::VecDeque;
use std::fmt;

pub const HUD_CONTAINER: &str = "RTC Internals HUD";
pub const HUD_DIMENSIONS: &str = "640px x 480px";
pub const HUD_WIDTH_PX: u32 = 640;
pub const HUD_HEIGHT_PX: u32 = 480;
pub const HUD_PADDING_PX: u32 = 16;
pub const HUD_SPACING_PX: u32 = 10;
pub const HUD_TARGET_FPS: u32 = 60;
pub const HUD_GRID_LINES: u32 = 4;

pub const RTCP_COLLECTOR_CONTAINER: &str = "RTCP Collector";
pub const RTCP_COLLECTOR_DIMENSIONS: &str = "RingBuffer 120";
pub const SPARKLINE_WIDTH_PX: u32 = 258;
pub const SPARKLINE_HEIGHT_PX: u32 = 80;

pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#23a55a";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const PING_FINE_MAX_MS: u64 = 250;
pub const PING_AVERAGE_MAX_MS: u64 = 500;
pub const LOSS_FINE_MAX_PCT: f64 = 5.0;
pub const LOSS_AVERAGE_MAX_PCT: f64 = 10.0;

pub const RTC_METRICS_RING_CAPACITY: usize = 120;
pub const RTC_METRICS_STEP_MS: u64 = 500;
pub const MAX_PING_HISTORY_LEN: usize = 199;
pub const PING_AVERAGE_WINDOW: usize = 20;
pub const DEFAULT_PING_TIMEOUT_MS: u64 = 4_000;

pub const RTCP_VERSION: u8 = 2;
pub const RTCP_PT_SR: u8 = 200;
pub const RTCP_PT_RR: u8 = 201;
pub const SRTCP_SR_WIRE_LEN: usize = 60;
pub const SRTCP_RR_WIRE_LEN: usize = 52;
pub const SRTCP_E_BIT: u32 = 0x8000_0000;
pub const OPUS_SAMPLE_RATE_KHZ: u32 = 48;

pub const PRIVACY_CODE_GROUP_COUNT: usize = 6;
pub const PRIVACY_CODE_GROUP_LEN: usize = 5;
pub const PRIVACY_CODE_TOTAL_DIGITS: usize = 30;

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_CONNECT: u64 = 1 << 20;
pub const PERM_SPEAK: u64 = 1 << 21;

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

pub const RTC_DEBUG_SECTIONS: [&str; 8] = [
    "TRANSPORT",
    "OUTBOUND",
    "INBOUND",
    "SCREENSHARE",
    "CAMERA",
    "CLIPS",
    "STREAMS_OUTBOUND",
    "STREAMS_INBOUND",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RtcGraphInteractiveState {
    Default,
    Hover,
    Active,
}

impl RtcGraphInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "60fps graph",
            Self::Hover => "data point tooltip",
            Self::Active => "zoomed metric",
        }
    }

    #[must_use]
    pub const fn resolve(hovered: bool, zoomed: bool) -> Self {
        match (zoomed, hovered) {
            (true, _) => Self::Active,
            (false, true) => Self::Hover,
            (false, false) => Self::Default,
        }
    }

    #[must_use]
    pub const fn canvas_str(self) -> &'static str {
        match self {
            Self::Default => "rendering",
            Self::Hover => "tooltip value",
            Self::Active => "zoom range",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionQuality {
    Unknown,
    Fine,
    Average,
    Bad,
}

impl ConnectionQuality {
    #[must_use]
    pub fn evaluate(connected: bool, last_ping_ms: Option<u64>, loss_pct: Option<f64>) -> Self {
        let Some(ping_ms) = last_ping_ms.filter(|_| connected) else {
            return Self::Unknown;
        };
        if loss_pct.is_some_and(|l| !l.is_finite() || !(0.0..=100.0).contains(&l)) {
            return Self::Unknown;
        }
        if ping_ms > PING_AVERAGE_MAX_MS || loss_pct.is_some_and(|l| l > LOSS_AVERAGE_MAX_PCT) {
            Self::Bad
        } else if ping_ms > PING_FINE_MAX_MS || loss_pct.is_some_and(|l| l > LOSS_FINE_MAX_PCT) {
            Self::Average
        } else {
            Self::Fine
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RtcDiagError {
    InvalidSnowflake(String),
    TruncatedPacket(usize),
    InvalidRtcpVersion(u8),
    InvalidPayloadType(u8),
    InvalidRtcpLength(u16),
    InvalidPrivacyCode(String),
    MissingPermission(&'static str),
    AgeGatedRestricted,
    BoundaryViolation(&'static str),
}

impl fmt::Display for RtcDiagError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for RtcDiagError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, RtcDiagError> {
    let s = raw.trim();
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(RtcDiagError::InvalidSnowflake(raw.to_string()));
    }
    let val = s
        .parse::<u64>()
        .map_err(|_| RtcDiagError::InvalidSnowflake(raw.to_string()))?;
    if val == 0 {
        return Err(RtcDiagError::InvalidSnowflake(raw.to_string()));
    }
    Ok(val)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtcpSenderInfo {
    pub ntp_timestamp: u64,
    pub rtp_timestamp: u32,
    pub packet_count: u32,
    pub octet_count: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RtcpReportBlock {
    pub ssrc: u32,
    pub fraction_lost: u8,
    pub cumulative_lost: i32,
    pub extended_highest_seq: u32,
    pub jitter_ticks: u32,
    pub last_sr: u32,
    pub delay_since_last_sr: u32,
}

impl RtcpReportBlock {
    #[must_use]
    pub fn fraction_lost_pct(self) -> f64 {
        (f64::from(self.fraction_lost) / 256.0) * 100.0
    }

    #[must_use]
    pub const fn jitter_ms(self) -> u32 {
        self.jitter_ticks / OPUS_SAMPLE_RATE_KHZ
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedRtcpPacket {
    pub payload_type: u8,
    pub sender_ssrc: u32,
    pub sender_info: Option<RtcpSenderInfo>,
    pub reports: Vec<RtcpReportBlock>,
    pub trailer_len: usize,
}

fn read_u32_be(b: &[u8], o: usize) -> u32 {
    u32::from_be_bytes([b[o], b[o + 1], b[o + 2], b[o + 3]])
}

fn read_i24_be(b: &[u8], o: usize) -> i32 {
    let raw = (i32::from(b[o]) << 16) | (i32::from(b[o + 1]) << 8) | i32::from(b[o + 2]);
    (raw << 8) >> 8
}

fn parse_report_blocks(body: &[u8], count: u8) -> Vec<RtcpReportBlock> {
    (0..usize::from(count))
        .map(|idx| {
            let off = idx * 24;
            RtcpReportBlock {
                ssrc: read_u32_be(body, off),
                fraction_lost: body[off + 4],
                cumulative_lost: read_i24_be(body, off + 5),
                extended_highest_seq: read_u32_be(body, off + 8),
                jitter_ticks: read_u32_be(body, off + 12),
                last_sr: read_u32_be(body, off + 16),
                delay_since_last_sr: read_u32_be(body, off + 20),
            }
        })
        .collect()
}

pub fn parse_rtcp_packet(bytes: &[u8]) -> Result<ParsedRtcpPacket, RtcDiagError> {
    if bytes.len() < 8 {
        return Err(RtcDiagError::TruncatedPacket(bytes.len()));
    }
    let (first, pt) = (bytes[0], bytes[1]);
    if (first >> 6) != RTCP_VERSION || (first & 0x20) != 0 {
        return Err(RtcDiagError::InvalidRtcpVersion(first >> 6));
    }
    let rc = first & 0x1f;
    let length_words = u16::from_be_bytes([bytes[2], bytes[3]]);
    let total = (usize::from(length_words) + 1) * 4;
    let hdr_len = match pt {
        RTCP_PT_SR => 28,
        RTCP_PT_RR => 8,
        other => return Err(RtcDiagError::InvalidPayloadType(other)),
    };
    if total < hdr_len + usize::from(rc) * 24 {
        return Err(RtcDiagError::InvalidRtcpLength(length_words));
    }
    if bytes.len() < total {
        return Err(RtcDiagError::TruncatedPacket(bytes.len()));
    }
    let sender_info = (pt == RTCP_PT_SR).then(|| RtcpSenderInfo {
        ntp_timestamp: (u64::from(read_u32_be(bytes, 8)) << 32) | u64::from(read_u32_be(bytes, 12)),
        rtp_timestamp: read_u32_be(bytes, 16),
        packet_count: read_u32_be(bytes, 20),
        octet_count: read_u32_be(bytes, 24),
    });
    Ok(ParsedRtcpPacket {
        payload_type: pt,
        sender_ssrc: read_u32_be(bytes, 4),
        sender_info,
        reports: parse_report_blocks(&bytes[hdr_len..total], rc),
        trailer_len: bytes.len() - total,
    })
}

#[must_use]
pub fn build_srtcp_sender_report(ssrc: u32, info: &RtcpSenderInfo, idx: u32) -> [u8; 60] {
    let mut b = [0u8; SRTCP_SR_WIRE_LEN];
    b[0] = 0x80;
    b[1] = RTCP_PT_SR;
    b[2..4].copy_from_slice(&6u16.to_be_bytes());
    b[4..8].copy_from_slice(&ssrc.to_be_bytes());
    b[8..16].copy_from_slice(&info.ntp_timestamp.to_be_bytes());
    b[16..20].copy_from_slice(&info.rtp_timestamp.to_be_bytes());
    b[20..24].copy_from_slice(&info.packet_count.to_be_bytes());
    b[24..28].copy_from_slice(&info.octet_count.to_be_bytes());
    b[56..60].copy_from_slice(&(idx | SRTCP_E_BIT).to_le_bytes());
    b
}

#[must_use]
pub fn build_srtcp_receiver_report(ssrc: u32, block: &RtcpReportBlock, cnt: u32) -> [u8; 52] {
    let mut b = [0u8; SRTCP_RR_WIRE_LEN];
    b[0] = 0x81;
    b[1] = RTCP_PT_RR;
    b[2..4].copy_from_slice(&7u16.to_be_bytes());
    b[4..8].copy_from_slice(&ssrc.to_be_bytes());
    b[8..12].copy_from_slice(&block.ssrc.to_be_bytes());
    b[12] = block.fraction_lost;
    b[13..16].copy_from_slice(&block.cumulative_lost.to_be_bytes()[1..4]);
    b[16..20].copy_from_slice(&block.extended_highest_seq.to_be_bytes());
    b[20..24].copy_from_slice(&block.jitter_ticks.to_be_bytes());
    b[24..28].copy_from_slice(&block.last_sr.to_be_bytes());
    b[28..32].copy_from_slice(&block.delay_since_last_sr.to_be_bytes());
    b[48..52].copy_from_slice(&cnt.to_le_bytes());
    b
}

#[must_use]
pub fn compute_rtt_from_lsr_dlsr_ms(rx_ntp: u32, lsr: u32, dlsr: u32) -> Option<u64> {
    (lsr != 0).then(|| {
        let rtt_compact = rx_ntp.wrapping_sub(lsr).saturating_sub(dlsr);
        ((u64::from(rtt_compact) * 1_000) + 32_768) / 65_536
    })
}

#[must_use]
pub fn compute_outbound_loss_delta_pct(p_sent: u64, p_lost: u64, c_sent: u64, c_lost: u64) -> f64 {
    let (ds, dl) = (c_sent.saturating_sub(p_sent), c_lost.saturating_sub(p_lost));
    if ds == 0 {
        0.0
    } else {
        ((dl as f64 / ds.saturating_add(dl) as f64).clamp(0.0, 1.0)) * 100.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RtcTelemetrySample {
    pub time_ms: u64,
    pub rtt_ms: u64,
    pub jitter_ms: u32,
    pub loss_pct: f64,
    pub bitrate_kbps: u32,
}

#[derive(Debug, Clone)]
pub struct RtcpMetricsRingBuffer {
    capacity: usize,
    samples: VecDeque<RtcTelemetrySample>,
}

impl Default for RtcpMetricsRingBuffer {
    fn default() -> Self {
        Self::new(RTC_METRICS_RING_CAPACITY)
    }
}

impl RtcpMetricsRingBuffer {
    #[must_use]
    pub fn new(capacity: usize) -> Self {
        let cap = capacity.max(1).min(RTC_METRICS_RING_CAPACITY);
        Self {
            capacity: cap,
            samples: VecDeque::with_capacity(cap),
        }
    }

    pub fn push(&mut self, sample: RtcTelemetrySample) {
        self.samples.push_back(sample);
        while self.samples.len() > self.capacity {
            self.samples.pop_front();
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }
}

#[derive(Debug, Clone, Default)]
pub struct PingHistory {
    samples: VecDeque<(u64, u64)>,
    ping_bad_count: u64,
    timeout_seqs: Vec<u64>,
}

impl PingHistory {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn push_ping(&mut self, time_ms: u64, value_ms: u64) {
        self.samples.push_back((time_ms, value_ms));
        while self.samples.len() > MAX_PING_HISTORY_LEN {
            self.samples.pop_front();
        }
        if value_ms > PING_AVERAGE_MAX_MS {
            self.ping_bad_count += 1;
        }
    }

    pub fn note_ping_timeout(&mut self, seq: u64, time_ms: u64) -> bool {
        self.timeout_seqs.push(seq);
        let n = self.timeout_seqs.len();
        let consecutive = n >= 3
            && self.timeout_seqs[n - 2] == self.timeout_seqs[n - 3] + 1
            && self.timeout_seqs[n - 1] == self.timeout_seqs[n - 2] + 1;
        if consecutive {
            self.push_ping(time_ms, DEFAULT_PING_TIMEOUT_MS);
        }
        consecutive
    }

    #[must_use]
    pub fn last_ping_ms(&self) -> Option<u64> {
        self.samples.back().map(|(_, v)| *v)
    }

    #[must_use]
    pub fn ping_bad_count(&self) -> u64 {
        self.ping_bad_count
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GraphPoint2D {
    pub x: f64,
    pub y: f64,
    pub value: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RtcHudFrame {
    pub graph_state: &'static str,
    pub canvas_state: &'static str,
    pub rtt_points: Vec<GraphPoint2D>,
    pub jitter_points: Vec<GraphPoint2D>,
    pub bitrate_points: Vec<GraphPoint2D>,
    pub tooltip_text_ru: Option<String>,
}

#[must_use]
pub fn compute_curve_points(values: &[f64], width: f64, height: f64) -> Vec<GraphPoint2D> {
    if values.is_empty() {
        return Vec::new();
    }
    let min_v = values.iter().copied().fold(values[0], f64::min);
    let max_v = values.iter().copied().fold(values[0], f64::max);
    let span = max_v - min_v;
    values
        .iter()
        .enumerate()
        .map(|(idx, &val)| {
            let x = if values.len() == 1 {
                0.0
            } else {
                (idx as f64 / (values.len() - 1) as f64) * width
            };
            let y = if span == 0.0 {
                height / 2.0
            } else {
                height - ((val - min_v) / span) * height
            };
            GraphPoint2D {
                x: x.clamp(0.0, width),
                y: y.clamp(0.0, height),
                value: val,
            }
        })
        .collect()
}

#[must_use]
pub fn render_hud_frame_60fps(
    samples: &[RtcTelemetrySample],
    zoom_factor: usize,
    hover_index: Option<usize>,
) -> RtcHudFrame {
    let zoom = zoom_factor.max(1);
    let visible_len = (samples.len() / zoom).max(usize::from(!samples.is_empty()));
    let slice = &samples[samples.len().saturating_sub(visible_len)..];
    let hovered_sample = hover_index.and_then(|idx| slice.get(idx));
    let state = RtcGraphInteractiveState::resolve(hovered_sample.is_some(), zoom > 1);
    let (w, h) = (f64::from(HUD_WIDTH_PX - HUD_PADDING_PX * 2), 120.0);
    let rtt: Vec<f64> = slice.iter().map(|s| s.rtt_ms as f64).collect();
    let jit: Vec<f64> = slice.iter().map(|s| f64::from(s.jitter_ms)).collect();
    let bit: Vec<f64> = slice.iter().map(|s| f64::from(s.bitrate_kbps)).collect();
    let tooltip_text_ru = hovered_sample.map(|s| {
        format!(
            "RTT: {} мс | Джиттер: {} мс | Потери: {:.1}%",
            s.rtt_ms, s.jitter_ms, s.loss_pct
        )
    });
    RtcHudFrame {
        graph_state: state.as_str(),
        canvas_state: state.canvas_str(),
        rtt_points: compute_curve_points(&rtt, w, h),
        jitter_points: compute_curve_points(&jit, w, h),
        bitrate_points: compute_curve_points(&bit, w, h),
        tooltip_text_ru,
    }
}

pub fn format_voice_privacy_code(raw: &str) -> Result<[String; 6], RtcDiagError> {
    let mut digits = String::with_capacity(PRIVACY_CODE_TOTAL_DIGITS);
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !(ch.is_whitespace() || ch == '-') {
            return Err(RtcDiagError::InvalidPrivacyCode(raw.to_string()));
        }
    }
    if digits.len() != PRIVACY_CODE_TOTAL_DIGITS {
        return Err(RtcDiagError::InvalidPrivacyCode(raw.to_string()));
    }
    Ok(std::array::from_fn(|g| {
        digits[g * PRIVACY_CODE_GROUP_LEN..(g + 1) * PRIVACY_CODE_GROUP_LEN].to_string()
    }))
}

pub fn derive_privacy_code_from_authenticator(bytes: &[u8]) -> Result<[String; 6], RtcDiagError> {
    if bytes.len() < PRIVACY_CODE_TOTAL_DIGITS {
        return Err(RtcDiagError::InvalidPrivacyCode(format!(
            "short: {}",
            bytes.len()
        )));
    }
    Ok(std::array::from_fn(|group| {
        let offset = group * PRIVACY_CODE_GROUP_LEN;
        let val = (0..PRIVACY_CODE_GROUP_LEN)
            .fold(0u64, |acc, i| (acc << 8) | u64::from(bytes[offset + i]));
        format!("{:05}", val % 100_000)
    }))
}

#[must_use]
pub fn should_show_verification_code(ch: &str, active: &str, stage: bool, ver: u16) -> bool {
    !ch.is_empty() && ch == active && !stage && ver > 0
}

#[must_use]
pub fn short_rtc_hostname(endpoint: &str) -> &str {
    let trimmed = endpoint.trim();
    &trimmed[..trimmed.find(['.', ':']).unwrap_or(trimmed.len())]
}

pub fn check_voice_diag_permissions(
    perms: u64,
    priv_ch: bool,
    age: bool,
) -> Result<(), RtcDiagError> {
    if age {
        return Err(RtcDiagError::AgeGatedRestricted);
    }
    let req = PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK;
    if !priv_ch && (perms & req) != req {
        return Err(RtcDiagError::MissingPermission("VOICE_DIAG"));
    }
    Ok(())
}

pub fn validate_system_boundaries(
    chars: usize,
    atts: usize,
    bytes: u64,
    offset: u32,
    vol: u16,
) -> Result<(), RtcDiagError> {
    if chars > MAX_MESSAGE_CHARS
        || atts > MAX_ATTACHMENTS_COUNT
        || bytes > MAX_ATTACHMENT_BYTES
        || offset > MAX_SEARCH_OFFSET
        || vol > MAX_VOLUME_PERCENT
    {
        return Err(RtcDiagError::BoundaryViolation("limit_exceeded"));
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

#[must_use]
pub fn redact_rtc_diagnostic(raw: &str) -> String {
    let mut out = raw.to_string();
    for marker in ["mfa.", "Bearer ", "secret_key=", "token="] {
        if let Some(pos) = out.find(marker) {
            let start = pos + marker.len();
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .map_or(out.len(), |i| start + i);
            out.replace_range(start..end, "[REDACTED]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1011_1_constants_and_interactive_states() {
        assert_eq!(HUD_CONTAINER, "RTC Internals HUD");
        assert_eq!(HUD_DIMENSIONS, "640px x 480px");
        assert_eq!(
            (HUD_WIDTH_PX, HUD_HEIGHT_PX, HUD_PADDING_PX),
            (640, 480, 16)
        );
        assert_eq!((TOKEN_BACKGROUND, TOKEN_ACCENT), ("#1e1f22", "#23a55a"));
        assert_eq!((PING_FINE_MAX_MS, PING_AVERAGE_MAX_MS), (250, 500));
        assert_eq!(
            (RTC_METRICS_RING_CAPACITY, PRIVACY_CODE_TOTAL_DIGITS),
            (120, 30)
        );
        assert_eq!(
            RtcGraphInteractiveState::resolve(false, false).as_str(),
            "60fps graph"
        );
        assert_eq!(
            RtcGraphInteractiveState::resolve(true, false).as_str(),
            "data point tooltip"
        );
        assert_eq!(
            RtcGraphInteractiveState::resolve(false, true).as_str(),
            "zoomed metric"
        );
        assert_eq!(RTC_DEBUG_SECTIONS.len(), 8);
    }

    #[test]
    fn test_sub_1011_2_rtcp_sr_rr_parsing() {
        let sr = RtcpSenderInfo {
            ntp_timestamp: 0x83aa_7e80_8000_0000,
            rtp_timestamp: 48_000,
            packet_count: 250,
            octet_count: 30_000,
        };
        let sr_pkt = parse_rtcp_packet(&build_srtcp_sender_report(17_843, &sr, 7)).unwrap();
        assert_eq!(sr_pkt.payload_type, 200);
        assert_eq!((sr_pkt.sender_ssrc, sr_pkt.trailer_len), (17_843, 32));

        let blk = RtcpReportBlock {
            ssrc: 857_040_390,
            fraction_lost: 20,
            cumulative_lost: -3_128_775,
            extended_highest_seq: 2_268_601_906,
            jitter_ticks: 960,
            last_sr: 0x7e80_8000,
            delay_since_last_sr: 65_536,
        };
        let rr_pkt = parse_rtcp_packet(&build_srtcp_receiver_report(17_843, &blk, 1)).unwrap();
        assert_eq!(rr_pkt.payload_type, 201);
        assert_eq!((rr_pkt.sender_ssrc, rr_pkt.trailer_len), (17_843, 20));
        assert_eq!(rr_pkt.reports[0].cumulative_lost, -3_128_775);
        assert_eq!(rr_pkt.reports[0].jitter_ms(), 20);
        assert!((rr_pkt.reports[0].fraction_lost_pct() - 7.8125).abs() < 1e-6);
    }

    #[test]
    fn test_sub_1011_2_quality_and_ring_buffer() {
        assert_eq!(
            compute_rtt_from_lsr_dlsr_ms(0x7e81_c000, 0x7e80_8000, 65_536),
            Some(250)
        );
        assert_eq!(compute_outbound_loss_delta_pct(100, 2, 190, 12), 10.0);
        assert_eq!(
            ConnectionQuality::evaluate(true, Some(200), Some(1.0)),
            ConnectionQuality::Fine
        );
        assert_eq!(
            ConnectionQuality::evaluate(true, Some(251), Some(1.0)),
            ConnectionQuality::Average
        );
        assert_eq!(
            ConnectionQuality::evaluate(true, Some(501), Some(1.0)),
            ConnectionQuality::Bad
        );

        let mut ring = RtcpMetricsRingBuffer::default();
        for i in 0..125u64 {
            ring.push(RtcTelemetrySample {
                time_ms: i * 500,
                rtt_ms: 40,
                jitter_ms: 5,
                loss_pct: 0.0,
                bitrate_kbps: 64,
            });
        }
        assert_eq!(ring.len(), 120);
    }

    #[test]
    fn test_sub_1011_3_hud_60fps_curves_and_dave_mls() {
        let samples: Vec<RtcTelemetrySample> = (0..60u64)
            .map(|i| RtcTelemetrySample {
                time_ms: i * 500,
                rtt_ms: 40 + (i % 10),
                jitter_ms: 5,
                loss_pct: 0.5,
                bitrate_kbps: 64,
            })
            .collect();
        let f_hover = render_hud_frame_60fps(&samples, 1, Some(2));
        assert_eq!(f_hover.graph_state, "data point tooltip");
        assert_eq!(
            (f_hover.canvas_state, f_hover.rtt_points.len()),
            ("tooltip value", 60)
        );
        assert!(f_hover.tooltip_text_ru.is_some());
        let f_zoom = render_hud_frame_60fps(&samples, 2, None);
        assert_eq!(
            (f_zoom.graph_state, f_zoom.rtt_points.len()),
            ("zoomed metric", 30)
        );

        let chunks = derive_privacy_code_from_authenticator(&[11u8; 30]).unwrap();
        assert_eq!(
            format_voice_privacy_code(&chunks.join(" ")).unwrap(),
            chunks
        );
        assert!(should_show_verification_code("120", "120", false, 1));
        assert!(!should_show_verification_code("120", "120", true, 1));
        assert_eq!(
            short_rtc_hostname("c-fra19-4da7fe5f.discord.media:2053"),
            "c-fra19-4da7fe5f"
        );
    }

    #[test]
    fn test_sub_1011_4_nine_dimensions_and_redaction() {
        assert!(validate_snowflake("0").is_err());
        assert_eq!(
            validate_snowflake("18446744073709551615").unwrap(),
            u64::MAX
        );
        assert!(parse_rtcp_packet(&[0x00; 4]).is_err());
        assert!(format_voice_privacy_code("12345").is_err());

        let mut pings = PingHistory::new();
        assert!(!pings.note_ping_timeout(1, 1_000) && !pings.note_ping_timeout(2, 2_000));
        assert!(pings.note_ping_timeout(3, 3_000));
        assert_eq!(pings.last_ping_ms(), Some(DEFAULT_PING_TIMEOUT_MS));
        assert_eq!(pings.ping_bad_count(), 1);

        let perms = PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK;
        assert!(check_voice_diag_permissions(perms, false, false).is_ok());
        assert!(check_voice_diag_permissions(perms, false, true).is_err());
        assert!(validate_system_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200).is_ok());
        assert!(validate_system_boundaries(2_001, 10, 100, 0, 100).is_err());
        assert_eq!(format_badge_cap(99), "99");
        assert_eq!(format_badge_cap(100), "99+");
        let masked = redact_rtc_diagnostic("Bearer mfa.secret123 secret_key=deadbeef");
        assert!(!masked.contains("secret123") && !masked.contains("deadbeef"));
    }
}
