#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Контроль перегрузки сети WebRTC BWE (Google Congestion Control, RTCP REMB и TWCC) — `TASK-982`.
//! Плавная адаптация видеобитрейта `500..=8000 kbps` без сброса соединения по `RTCP REMB`
//! (`PT=206, FMT=15, "REMB"`) и `RTCP TWCC` (`PT=205, FMT=15`).

use std::fmt;

pub const MIN_VIDEO_BITRATE_KBPS: u32 = 500;
pub const MAX_VIDEO_BITRATE_KBPS: u32 = 8_000;
pub const DEFAULT_START_BITRATE_KBPS: u32 = 2_500;
pub const FALLBACK_MIN_BITRATE_BPS: u64 = 150_000;
pub const PROBE_INCREASE_STEP_KBPS: u32 = 250;
pub const MAX_STEP_DROP_PERCENT: u32 = 15;
pub const DEFAULT_OVERUSE_THRESHOLD_MS: f64 = 12.5;

pub const RTCP_RTPFB_PT: u8 = 205;
pub const RTCP_PSFB_PT: u8 = 206;
pub const RTCP_FMT_REMB: u8 = 15;
pub const RTCP_FMT_TWCC: u8 = 15;
pub const RTCP_REMB_MAGIC: [u8; 4] = *b"REMB";
pub const REMB_MAX_EXPONENT: u8 = 63;
pub const REMB_MAX_MANTISSA: u32 = 0x3_ffff;
pub const TWCC_DELTA_SCALE_US: i64 = 250;
pub const TWCC_REF_TIME_SCALE_MS: u64 = 64;

pub const TWCC_URI: &str =
    "http://www.ietf.org/id/draft-holmer-rmcat-transport-wide-cc-extensions-01";
pub const ABS_CAPTURE_TIME_URI: &str =
    "http://www.webrtc.org/experiments/rtp-hdrext/abs-capture-time";
pub const BWE_TRENDLINE_SYMBOL: &str =
    "kDiscordBandwidthEstimationExperimentTrendlineWindowDuration";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CongestionError {
    Truncated { needed: usize, available: usize },
    InvalidVersion(u8),
    UnsupportedPadding,
    InvalidPayloadType { expected: u8, actual: u8 },
    InvalidFeedbackFormat { expected: u8, actual: u8 },
    InvalidRembMagic,
    InvalidLength { declared: usize, actual: usize },
    BitrateOverflow { exponent: u8, mantissa: u32 },
    TooManySsrcs(usize),
    EmptyTwccFeedback,
    InvalidBitrateBounds { min_kbps: u32, max_kbps: u32 },
}

impl fmt::Display for CongestionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CongestionError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BandwidthUsageState {
    Underusing,
    Normal,
    Overusing,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BweInteractiveState {
    Probing,
    LimitBandwidth,
    AdaptedBitrate,
}

impl BweInteractiveState {
    #[must_use]
    pub const fn as_label(self) -> &'static str {
        match self {
            Self::Probing => "probing",
            Self::LimitBandwidth => "limit bandwidth",
            Self::AdaptedBitrate => "adapted bitrate",
        }
    }

    #[must_use]
    pub const fn resolve(adapted: bool, hovered: bool) -> Self {
        if hovered {
            Self::LimitBandwidth
        } else if adapted {
            Self::AdaptedBitrate
        } else {
            Self::Probing
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VideoResolutionTier {
    pub label: &'static str,
    pub width: u32,
    pub height: u32,
    pub max_fps: u32,
}

impl VideoResolutionTier {
    #[must_use]
    pub const fn new(label: &'static str, width: u32, height: u32, max_fps: u32) -> Self {
        Self {
            label,
            width,
            height,
            max_fps,
        }
    }

    #[must_use]
    pub const fn for_bitrate_kbps(b: u32) -> Self {
        if b >= 4_500 {
            Self::new("1080p60", 1920, 1080, 60)
        } else if b >= 2_500 {
            Self::new("1080p30", 1920, 1080, 30)
        } else if b >= 1_200 {
            Self::new("720p30", 1280, 720, 30)
        } else {
            Self::new("480p20", 854, 480, 20)
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RembPacket {
    pub sender_ssrc: u32,
    pub media_ssrc: u32,
    pub exponent: u8,
    pub mantissa: u32,
    pub bitrate_bps: u64,
    pub ssrcs: Vec<u32>,
}

#[must_use]
pub fn encode_remb_bitrate(bitrate_bps: u64) -> (u8, u32) {
    let mut mantissa = bitrate_bps;
    let mut exponent = 0u8;
    while mantissa > u64::from(REMB_MAX_MANTISSA) && exponent < REMB_MAX_EXPONENT {
        mantissa >>= 1;
        exponent += 1;
    }
    (exponent, (mantissa & u64::from(REMB_MAX_MANTISSA)) as u32)
}

pub fn decode_remb_bitrate(exponent: u8, mantissa: u32) -> Result<u64, CongestionError> {
    if exponent > REMB_MAX_EXPONENT || mantissa > REMB_MAX_MANTISSA {
        return Err(CongestionError::BitrateOverflow { exponent, mantissa });
    }
    if mantissa == 0 {
        return Ok(0);
    }
    if u64::from(mantissa) > (u64::MAX >> exponent) {
        return Err(CongestionError::BitrateOverflow { exponent, mantissa });
    }
    Ok(u64::from(mantissa) << exponent)
}

pub fn build_remb_packet(
    sender_ssrc: u32,
    bitrate_bps: u64,
    ssrcs: &[u32],
) -> Result<Vec<u8>, CongestionError> {
    if ssrcs.len() > 255 {
        return Err(CongestionError::TooManySsrcs(ssrcs.len()));
    }
    let (exp, mant) = encode_remb_bitrate(bitrate_bps);
    let length_words = 4u16.saturating_add(ssrcs.len() as u16);
    let mut out = Vec::with_capacity(20 + ssrcs.len() * 4);
    out.extend_from_slice(&[0x80 | RTCP_FMT_REMB, RTCP_PSFB_PT]);
    out.extend_from_slice(&length_words.to_be_bytes());
    out.extend_from_slice(&sender_ssrc.to_be_bytes());
    out.extend_from_slice(&0u32.to_be_bytes());
    out.extend_from_slice(&RTCP_REMB_MAGIC);
    out.extend_from_slice(&[
        ssrcs.len() as u8,
        (exp << 2) | ((mant >> 16) as u8 & 0x03),
        ((mant >> 8) & 0xff) as u8,
        (mant & 0xff) as u8,
    ]);
    for &ssrc in ssrcs {
        out.extend_from_slice(&ssrc.to_be_bytes());
    }
    Ok(out)
}

fn validate_rtcp_fb_header(
    bytes: &[u8],
    expected_pt: u8,
    expected_fmt: u8,
    min_len: usize,
) -> Result<usize, CongestionError> {
    if bytes.len() < min_len {
        return Err(CongestionError::Truncated {
            needed: min_len,
            available: bytes.len(),
        });
    }
    if (bytes[0] >> 6) != 2 {
        return Err(CongestionError::InvalidVersion(bytes[0] >> 6));
    }
    if (bytes[0] & 0x20) != 0 {
        return Err(CongestionError::UnsupportedPadding);
    }
    let fmt = bytes[0] & 0x1f;
    if fmt != expected_fmt {
        return Err(CongestionError::InvalidFeedbackFormat {
            expected: expected_fmt,
            actual: fmt,
        });
    }
    let pt = bytes[1];
    if pt != expected_pt {
        return Err(CongestionError::InvalidPayloadType {
            expected: expected_pt,
            actual: pt,
        });
    }
    let total = (usize::from(u16::from_be_bytes([bytes[2], bytes[3]])) + 1) * 4;
    let needed = total.max(min_len);
    if total < min_len || bytes.len() < total {
        return Err(CongestionError::Truncated {
            needed,
            available: bytes.len(),
        });
    }
    Ok(total)
}

pub fn parse_remb_packet(bytes: &[u8]) -> Result<RembPacket, CongestionError> {
    let total = validate_rtcp_fb_header(bytes, RTCP_PSFB_PT, RTCP_FMT_REMB, 20)?;
    if bytes.get(12..16) != Some(&RTCP_REMB_MAGIC) {
        return Err(CongestionError::InvalidRembMagic);
    }
    let actual = 20 + usize::from(bytes[16]) * 4;
    if total != actual {
        return Err(CongestionError::InvalidLength {
            declared: total,
            actual,
        });
    }
    let exponent = bytes[17] >> 2;
    let mantissa =
        (u32::from(bytes[17] & 0x03) << 16) | (u32::from(bytes[18]) << 8) | u32::from(bytes[19]);
    let bitrate_bps = decode_remb_bitrate(exponent, mantissa)?;
    let sender_ssrc = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let media_ssrc = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let ssrcs = bytes[20..total]
        .chunks_exact(4)
        .map(|c| u32::from_be_bytes([c[0], c[1], c[2], c[3]]))
        .collect();
    Ok(RembPacket {
        sender_ssrc,
        media_ssrc,
        exponent,
        mantissa,
        bitrate_bps,
        ssrcs,
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TwccPacketStatus {
    NotReceived,
    SmallDelta,
    LargeOrNegativeDelta,
}

impl TwccPacketStatus {
    const fn symbol_bits(self) -> u16 {
        match self {
            Self::NotReceived => 0,
            Self::SmallDelta => 1,
            Self::LargeOrNegativeDelta => 2,
        }
    }

    const fn from_symbol(bits: u16) -> Self {
        match bits & 0x03 {
            1 => Self::SmallDelta,
            2 => Self::LargeOrNegativeDelta,
            _ => Self::NotReceived,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TwccPacketSample {
    pub sequence: u16,
    pub received: bool,
    pub recv_delta_us: i64,
    pub send_delta_us: i64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TwccParsedFeedback {
    pub sequence: u16,
    pub status: TwccPacketStatus,
    pub received: bool,
    pub recv_delta_us: Option<i64>,
    pub delay_gradient_ms: Option<f64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct TwccFeedbackPacket {
    pub sender_ssrc: u32,
    pub media_ssrc: u32,
    pub base_seq: u16,
    pub packet_status_count: u16,
    pub reference_time_64ms: u32,
    pub fb_pkt_count: u8,
    pub feedbacks: Vec<TwccParsedFeedback>,
}

pub fn build_twcc_packet(
    sender_ssrc: u32,
    media_ssrc: u32,
    base_seq: u16,
    reference_time_64ms: u32,
    fb_pkt_count: u8,
    samples: &[TwccPacketSample],
) -> Result<Vec<u8>, CongestionError> {
    if samples.is_empty() {
        return Err(CongestionError::EmptyTwccFeedback);
    }
    let mut body = Vec::with_capacity(16 + samples.len() * 3);
    body.extend_from_slice(&sender_ssrc.to_be_bytes());
    body.extend_from_slice(&media_ssrc.to_be_bytes());
    body.extend_from_slice(&base_seq.to_be_bytes());
    body.extend_from_slice(&(samples.len() as u16).to_be_bytes());
    let ref_24 = reference_time_64ms & 0x00ff_ffff;
    body.extend_from_slice(&[
        ((ref_24 >> 16) & 0xff) as u8,
        ((ref_24 >> 8) & 0xff) as u8,
        (ref_24 & 0xff) as u8,
        fb_pkt_count,
    ]);
    append_twcc_chunks_and_deltas(&mut body, samples);
    while !body.len().is_multiple_of(4) {
        body.push(0);
    }
    let length_words = (body.len() / 4) as u16;
    let mut out = Vec::with_capacity(4 + body.len());
    out.extend_from_slice(&[0x80 | RTCP_FMT_TWCC, RTCP_RTPFB_PT]);
    out.extend_from_slice(&length_words.to_be_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

fn append_twcc_chunks_and_deltas(body: &mut Vec<u8>, samples: &[TwccPacketSample]) {
    let mut deltas = Vec::with_capacity(samples.len() * 2);
    for sample in samples {
        let (status, ticks) = classify_twcc_sample(sample);
        body.extend_from_slice(&((status.symbol_bits() << 13) | 1u16).to_be_bytes());
        match status {
            TwccPacketStatus::NotReceived => {}
            TwccPacketStatus::SmallDelta => deltas.push(ticks as u8),
            TwccPacketStatus::LargeOrNegativeDelta => {
                deltas.extend_from_slice(&(ticks as i16).to_be_bytes());
            }
        }
    }
    body.extend_from_slice(&deltas);
}

fn classify_twcc_sample(sample: &TwccPacketSample) -> (TwccPacketStatus, i64) {
    if !sample.received {
        return (TwccPacketStatus::NotReceived, 0);
    }
    let ticks =
        (sample.recv_delta_us / TWCC_DELTA_SCALE_US).clamp(i16::MIN as i64, i16::MAX as i64);
    let status = if (0..=255).contains(&ticks) {
        TwccPacketStatus::SmallDelta
    } else {
        TwccPacketStatus::LargeOrNegativeDelta
    };
    (status, ticks)
}

pub fn parse_twcc_packet(
    bytes: &[u8],
    default_send_delta_us: i64,
) -> Result<TwccFeedbackPacket, CongestionError> {
    let total = validate_rtcp_fb_header(bytes, RTCP_RTPFB_PT, RTCP_FMT_TWCC, 20)?;
    let sender_ssrc = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    let media_ssrc = u32::from_be_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let base_seq = u16::from_be_bytes([bytes[12], bytes[13]]);
    let packet_status_count = u16::from_be_bytes([bytes[14], bytes[15]]);
    if packet_status_count == 0 {
        return Err(CongestionError::EmptyTwccFeedback);
    }
    let reference_time_64ms =
        (u32::from(bytes[16]) << 16) | (u32::from(bytes[17]) << 8) | u32::from(bytes[18]);
    let (statuses, offset) = decode_twcc_statuses(&bytes[..total], 20, packet_status_count)?;
    let feedbacks = decode_twcc_deltas(
        &bytes[..total],
        offset,
        base_seq,
        &statuses,
        default_send_delta_us,
    )?;
    Ok(TwccFeedbackPacket {
        sender_ssrc,
        media_ssrc,
        base_seq,
        packet_status_count,
        reference_time_64ms,
        fb_pkt_count: bytes[19],
        feedbacks,
    })
}

fn decode_twcc_statuses(
    bytes: &[u8],
    mut offset: usize,
    target_count: u16,
) -> Result<(Vec<TwccPacketStatus>, usize), CongestionError> {
    let mut statuses = Vec::with_capacity(usize::from(target_count));
    while statuses.len() < usize::from(target_count) {
        let needed = offset + 2;
        if needed > bytes.len() {
            return Err(CongestionError::Truncated {
                needed,
                available: bytes.len(),
            });
        }
        let chunk = u16::from_be_bytes([bytes[offset], bytes[offset + 1]]);
        offset += 2;
        if (chunk & 0x8000) == 0 {
            let status = TwccPacketStatus::from_symbol((chunk >> 13) & 0x03);
            let run_len = usize::from(chunk & 0x1fff);
            if run_len == 0 {
                return Err(CongestionError::EmptyTwccFeedback);
            }
            let take = run_len.min(usize::from(target_count) - statuses.len());
            statuses.extend(std::iter::repeat_n(status, take));
        } else {
            decode_status_vector_chunk(chunk, target_count, &mut statuses);
        }
    }
    Ok((statuses, offset))
}

fn decode_status_vector_chunk(chunk: u16, target_count: u16, statuses: &mut Vec<TwccPacketStatus>) {
    let two_bit = (chunk & 0x4000) != 0;
    let count = if two_bit { 7 } else { 14 };
    for idx in 0..count {
        if statuses.len() >= usize::from(target_count) {
            break;
        }
        let status = if two_bit {
            TwccPacketStatus::from_symbol((chunk >> (12 - idx * 2)) & 0x03)
        } else if ((chunk >> (13 - idx)) & 0x01) == 1 {
            TwccPacketStatus::SmallDelta
        } else {
            TwccPacketStatus::NotReceived
        };
        statuses.push(status);
    }
}

fn read_one_twcc_delta(
    bytes: &[u8],
    offset: &mut usize,
    status: TwccPacketStatus,
) -> Result<Option<i64>, CongestionError> {
    match status {
        TwccPacketStatus::NotReceived => Ok(None),
        TwccPacketStatus::SmallDelta => {
            let needed = *offset + 1;
            if needed > bytes.len() {
                return Err(CongestionError::Truncated {
                    needed,
                    available: bytes.len(),
                });
            }
            let recv_us = i64::from(bytes[*offset]) * TWCC_DELTA_SCALE_US;
            *offset += 1;
            Ok(Some(recv_us))
        }
        TwccPacketStatus::LargeOrNegativeDelta => {
            let needed = *offset + 2;
            if needed > bytes.len() {
                return Err(CongestionError::Truncated {
                    needed,
                    available: bytes.len(),
                });
            }
            let ticks = i16::from_be_bytes([bytes[*offset], bytes[*offset + 1]]);
            *offset += 2;
            Ok(Some(i64::from(ticks) * TWCC_DELTA_SCALE_US))
        }
    }
}

fn decode_twcc_deltas(
    bytes: &[u8],
    mut offset: usize,
    base_seq: u16,
    statuses: &[TwccPacketStatus],
    default_send_delta_us: i64,
) -> Result<Vec<TwccParsedFeedback>, CongestionError> {
    let mut out = Vec::with_capacity(statuses.len());
    for (idx, &status) in statuses.iter().enumerate() {
        let sequence = base_seq.wrapping_add(idx as u16);
        let recv_delta_us = read_one_twcc_delta(bytes, &mut offset, status)?;
        let delay_gradient_ms =
            recv_delta_us.map(|r| ((r - default_send_delta_us) as f64) / 1000.0);
        out.push(TwccParsedFeedback {
            sequence,
            status,
            received: recv_delta_us.is_some(),
            recv_delta_us,
            delay_gradient_ms,
        });
    }
    Ok(out)
}

#[derive(Debug, Clone, PartialEq)]
pub struct BweControllerSnapshot {
    pub current_bitrate_kbps: u32,
    pub remb_cap_kbps: u32,
    pub usage_state: BandwidthUsageState,
    pub interactive_state: BweInteractiveState,
    pub resolution_tier: VideoResolutionTier,
    pub smoothed_gradient_ms: f64,
    pub loss_pct: f64,
    pub connection_alive: bool,
    pub reconnect_count: u32,
}

#[derive(Debug, Clone)]
pub struct GccCongestionController {
    min_bitrate_kbps: u32,
    max_bitrate_kbps: u32,
    current_bitrate_kbps: u32,
    remb_cap_kbps: u32,
    overuse_threshold_ms: f64,
    smoothed_gradient_ms: f64,
    last_loss_pct: f64,
    usage_state: BandwidthUsageState,
    adapted: bool,
    connection_alive: bool,
    reconnect_count: u32,
}

impl Default for GccCongestionController {
    fn default() -> Self {
        Self {
            min_bitrate_kbps: MIN_VIDEO_BITRATE_KBPS,
            max_bitrate_kbps: MAX_VIDEO_BITRATE_KBPS,
            current_bitrate_kbps: DEFAULT_START_BITRATE_KBPS,
            remb_cap_kbps: MAX_VIDEO_BITRATE_KBPS,
            overuse_threshold_ms: DEFAULT_OVERUSE_THRESHOLD_MS,
            smoothed_gradient_ms: 0.0,
            last_loss_pct: 0.0,
            usage_state: BandwidthUsageState::Normal,
            adapted: false,
            connection_alive: true,
            reconnect_count: 0,
        }
    }
}

impl GccCongestionController {
    pub fn new(min_kbps: u32, max_kbps: u32, start_kbps: u32) -> Result<Self, CongestionError> {
        if min_kbps == 0 || min_kbps > max_kbps {
            return Err(CongestionError::InvalidBitrateBounds { min_kbps, max_kbps });
        }
        let min_c = min_kbps.max(MIN_VIDEO_BITRATE_KBPS);
        let max_c = max_kbps.min(MAX_VIDEO_BITRATE_KBPS);
        Ok(Self {
            min_bitrate_kbps: min_c,
            max_bitrate_kbps: max_c,
            current_bitrate_kbps: start_kbps.clamp(min_c, max_c),
            remb_cap_kbps: max_c,
            ..Self::default()
        })
    }

    pub fn on_remb_bitrate_bps(&mut self, bitrate_bps: u64) -> BweControllerSnapshot {
        let raw_kbps = (bitrate_bps / 1_000) as u32;
        let clamped_cap = raw_kbps.clamp(self.min_bitrate_kbps, self.max_bitrate_kbps);
        self.remb_cap_kbps = clamped_cap;
        self.adapted = true;
        if self.current_bitrate_kbps > clamped_cap {
            let min_allowed = (self.current_bitrate_kbps * 85) / 100;
            self.current_bitrate_kbps = clamped_cap
                .max(min_allowed)
                .clamp(self.min_bitrate_kbps, self.max_bitrate_kbps);
        }
        self.snapshot(false)
    }

    pub fn on_twcc_samples(&mut self, samples: &[TwccPacketSample]) -> BweControllerSnapshot {
        if samples.is_empty() {
            return self.snapshot(false);
        }
        let (mean_grad_ms, loss_pct) = summarize_twcc_samples(samples);
        self.smoothed_gradient_ms = self.smoothed_gradient_ms * 0.3 + mean_grad_ms * 0.7;
        self.last_loss_pct = loss_pct;
        let ceiling = self.remb_cap_kbps.min(self.max_bitrate_kbps);
        if self.smoothed_gradient_ms > self.overuse_threshold_ms || loss_pct > 10.0 {
            self.usage_state = BandwidthUsageState::Overusing;
            self.adapted = true;
            let min_allowed = (self.current_bitrate_kbps * 85) / 100;
            self.current_bitrate_kbps = min_allowed.clamp(self.min_bitrate_kbps, ceiling);
        } else {
            self.usage_state = if self.smoothed_gradient_ms < -self.overuse_threshold_ms {
                BandwidthUsageState::Underusing
            } else {
                BandwidthUsageState::Normal
            };
            let next = self
                .current_bitrate_kbps
                .saturating_add(PROBE_INCREASE_STEP_KBPS);
            self.current_bitrate_kbps = next.clamp(self.min_bitrate_kbps, ceiling);
        }
        self.snapshot(false)
    }

    #[must_use]
    pub fn snapshot(&self, hovered: bool) -> BweControllerSnapshot {
        BweControllerSnapshot {
            current_bitrate_kbps: self.current_bitrate_kbps,
            remb_cap_kbps: self.remb_cap_kbps,
            usage_state: self.usage_state,
            interactive_state: BweInteractiveState::resolve(self.adapted, hovered),
            resolution_tier: VideoResolutionTier::for_bitrate_kbps(self.current_bitrate_kbps),
            smoothed_gradient_ms: self.smoothed_gradient_ms,
            loss_pct: self.last_loss_pct,
            connection_alive: self.connection_alive,
            reconnect_count: self.reconnect_count,
        }
    }
}

fn summarize_twcc_samples(samples: &[TwccPacketSample]) -> (f64, f64) {
    let recv: Vec<f64> = samples
        .iter()
        .filter(|s| s.received)
        .map(|s| ((s.recv_delta_us - s.send_delta_us) as f64) / 1000.0)
        .collect();
    let mean_grad = if recv.is_empty() {
        25.0
    } else {
        recv.iter().sum::<f64>() / (recv.len() as f64)
    };
    let lost = samples.len() - recv.len();
    (mean_grad, (lost as f64) * 100.0 / (samples.len() as f64))
}

#[must_use]
pub fn redact_rtc_secrets(text: &str) -> String {
    let mut out = text.to_string();
    if let Some(pos) = out.find("mfa.") {
        let end = out[pos..]
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .map_or(out.len(), |idx| pos + idx);
        out.replace_range(pos..end, "mfa.***");
    }
    if let Some(pos) = out.find("secret_key=") {
        let start = pos + "secret_key=".len();
        let end = out[start..]
            .find(|c: char| c.is_whitespace())
            .map_or(out.len(), |idx| start + idx);
        out.replace_range(start..end, "\"[REDACTED]\"");
    }
    out
}

pub fn zeroize_key_material(buf: &mut [u8]) -> usize {
    for byte in buf.iter_mut() {
        *byte = std::hint::black_box(0u8);
    }
    std::hint::black_box(buf.len())
}

#[cfg(test)]
mod tests {
    use super::*;

    const fn sample(seq: u16, recv: bool, r_us: i64, s_us: i64) -> TwccPacketSample {
        TwccPacketSample {
            sequence: seq,
            received: recv,
            recv_delta_us: r_us,
            send_delta_us: s_us,
        }
    }

    #[test]
    fn test_sub_982_1_constants_and_tiers() {
        assert_eq!(MIN_VIDEO_BITRATE_KBPS, 500);
        assert_eq!(MAX_VIDEO_BITRATE_KBPS, 8_000);
        assert_eq!(DEFAULT_START_BITRATE_KBPS, 2_500);
        assert_eq!(FALLBACK_MIN_BITRATE_BPS, 150_000);
        assert!(TWCC_URI.contains("transport-wide-cc-extensions-01"));
        assert!(BWE_TRENDLINE_SYMBOL.contains("TrendlineWindowDuration"));
        let resolve = BweInteractiveState::resolve;
        assert_eq!(resolve(false, false).as_label(), "probing");
        assert_eq!(resolve(false, true).as_label(), "limit bandwidth");
        assert_eq!(resolve(true, false).as_label(), "adapted bitrate");
        let tier = VideoResolutionTier::for_bitrate_kbps;
        assert_eq!(tier(8_000).label, "1080p60");
        assert_eq!(tier(2_500).label, "1080p30");
        assert_eq!(tier(1_500).label, "720p30");
        assert_eq!(tier(500).label, "480p20");
    }

    #[test]
    fn test_sub_982_2_remb_encode_decode_and_errors() {
        for bps in [0u64, 500_000, 2_500_000, 8_000_000] {
            let wire = build_remb_packet(9_208, bps, &[17_844, 17_845]).unwrap();
            let parsed = parse_remb_packet(&wire).unwrap();
            assert_eq!(parsed.sender_ssrc, 9_208);
            assert_eq!(parsed.ssrcs, vec![17_844, 17_845]);
            assert!(parsed.bitrate_bps.abs_diff(bps) <= 64);
        }
        let valid = build_remb_packet(9_208, 2_500_000, &[17_844]).unwrap();
        assert!(parse_remb_packet(&valid[..10]).is_err());
        let mut bad_magic = valid.clone();
        bad_magic[12..16].copy_from_slice(b"BAD!");
        assert_eq!(
            parse_remb_packet(&bad_magic),
            Err(CongestionError::InvalidRembMagic)
        );
        let mut bad_cnt = valid.clone();
        bad_cnt[16] = 42;
        assert!(parse_remb_packet(&bad_cnt).is_err());
        assert!(decode_remb_bitrate(63, REMB_MAX_MANTISSA).is_err());
    }

    #[test]
    fn test_sub_982_3_twcc_encode_decode_wraparound_and_errors() {
        let samples = [
            sample(65_534, true, 20_000, 20_000),
            sample(65_535, true, 25_500, 20_000),
            sample(0, false, 0, 20_000),
            sample(1, true, 70_000, 20_000),
            sample(2, true, -2_000, 20_000),
        ];
        let wire = build_twcc_packet(9_208, 17_844, 65_534, 1_000, 7, &samples).unwrap();
        let parsed = parse_twcc_packet(&wire, 20_000).unwrap();
        assert_eq!((parsed.base_seq, parsed.packet_status_count), (65_534, 5));
        assert_eq!(parsed.feedbacks[1].sequence, 65_535);
        assert_eq!(parsed.feedbacks[1].delay_gradient_ms, Some(5.5));
        assert!(!parsed.feedbacks[2].received);
        assert_eq!(parsed.feedbacks[3].recv_delta_us, Some(70_000));
        assert_eq!(parsed.feedbacks[4].recv_delta_us, Some(-2_000));
        assert!(parse_twcc_packet(&wire[..12], 20_000).is_err());
    }

    #[test]
    fn test_sub_982_4_gcc_smooth_adaptation_and_security() {
        let mut ctrl = GccCongestionController::new(500, 8_000, 2_500).unwrap();
        let ok = [sample(1, true, 20_000, 20_000)];
        for _ in 0..30 {
            ctrl.on_twcc_samples(&ok);
        }
        assert_eq!(ctrl.snapshot(false).current_bitrate_kbps, 8_000);

        let ov = [sample(2, true, 45_000, 20_000)];
        let mut prev = ctrl.snapshot(false).current_bitrate_kbps;
        for _ in 0..5 {
            let snap = ctrl.on_twcc_samples(&ov);
            assert_eq!(snap.usage_state, BandwidthUsageState::Overusing);
            assert!(snap.current_bitrate_kbps < prev);
            assert!(((prev - snap.current_bitrate_kbps) * 100) / prev <= 15);
            assert!(snap.connection_alive);
            prev = snap.current_bitrate_kbps;
        }

        let remb_step = ctrl.on_remb_bitrate_bps(150_000);
        assert!(remb_step.current_bitrate_kbps >= 2_500);
        for _ in 0..20 {
            ctrl.on_remb_bitrate_bps(150_000);
        }
        let floor_snap = ctrl.snapshot(false);
        assert_eq!(floor_snap.current_bitrate_kbps, 500);
        assert_eq!(floor_snap.remb_cap_kbps, 500);

        let masked = redact_rtc_secrets("secret_key=\"012345\" token=mfa.secret982");
        assert!(masked.contains("[REDACTED]") && masked.contains("mfa.***"));
        let mut key = [0xAAu8; 32];
        assert_eq!(zeroize_key_material(&mut key), 32);
        assert!(key.iter().all(|&b| b == 0));
    }
}
