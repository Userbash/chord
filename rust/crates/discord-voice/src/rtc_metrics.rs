#![forbid(unsafe_code)]
#![allow(dead_code)]
//! Сборщик RTCP метрик (`RtcpMetricsRingBuffer`: 120 измерений, 60 с с шагом 500 мс)
//! и неблокирующий мониторинг эпох DAVE MLS v1 (`DaveMlsEpochMonitor`) — `TASK-1011A` (`RE-GAP-22`).

use std::collections::VecDeque;
use std::fmt;
use std::sync::atomic::{AtomicBool, AtomicU16, AtomicU32, AtomicU64, Ordering};

pub const RTCP_COLLECTOR_CONTAINER: &str = "RTCP Collector";
pub const RTCP_COLLECTOR_DIMENSIONS: &str = "RingBuffer 120";
pub const RTCP_COLLECTOR_PADDING: &str = "0px";
pub const RTCP_COLLECTOR_SPACING: &str = "0px";
pub const TELEMETRY_PROBE_ELEMENT: &str = "telemetry-probe";
pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#23a55a";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const RTC_METRICS_RING_CAPACITY: usize = 120;
pub const RTC_METRICS_STEP_MS: u64 = 500;
pub const RTC_METRICS_WINDOW_MS: u64 = 60_000;
pub const PING_FINE_MAX_MS: u64 = 250;
pub const PING_AVERAGE_MAX_MS: u64 = 500;
pub const LOSS_FINE_MAX_PCT: f64 = 5.0;
pub const LOSS_AVERAGE_MAX_PCT: f64 = 10.0;
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
pub const SESSION2_AUDIO_SSRC: u32 = 17_843;
pub const SESSION3_AUDIO_SSRC: u32 = 9_208;
pub const DAVE_PROTOCOL_VERSION_V1: u16 = 1;
pub const VOICE_OP_DAVE_PREPARE_TRANSITION: u8 = 21;
pub const VOICE_OP_DAVE_EXECUTE_TRANSITION: u8 = 22;
pub const VOICE_OP_DAVE_READY_FOR_TRANSITION: u8 = 23;
pub const VOICE_OP_DAVE_PREPARE_EPOCH: u8 = 24;
pub const VOICE_OP_MLS_PROPOSALS: u8 = 27;
pub const VOICE_OP_MLS_COMMIT_WELCOME: u8 = 28;
pub const VOICE_OP_MLS_PREPARE_COMMIT: u8 = 29;
pub const VOICE_OP_MLS_INVALID_COMMIT: u8 = 31;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TelemetryProbeState {
    Polling,
    Collect,
    Buffered,
}

impl TelemetryProbeState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Polling => "polling",
            Self::Collect => "collect",
            Self::Buffered => "buffered",
        }
    }

    #[must_use]
    pub const fn resolve(buffered: bool, hovered: bool) -> Self {
        match (buffered, hovered) {
            (true, _) => Self::Buffered,
            (false, true) => Self::Collect,
            (false, false) => Self::Polling,
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
pub enum RtcMetricsError {
    InvalidSnowflake(String),
    TruncatedPacket(usize),
    InvalidRtcpVersion(u8),
    InvalidPayloadType(u8),
    InvalidRtcpLength(u16),
    StepIntervalTooShort(u64),
    UnsupportedDaveOpcode(u8),
    DaveDowngradeRejected(u16),
    EpochRegressionRejected(u64),
    InvalidPrivacyCode(String),
    MissingPermission(&'static str),
    AgeGatedSpaceRestricted,
    BoundaryViolation(&'static str),
    HttpStatusError(u16, u64),
}

impl fmt::Display for RtcMetricsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for RtcMetricsError {}

pub fn validate_snowflake_id(raw: &str) -> Result<u64, RtcMetricsError> {
    let s = raw.trim();
    if s.is_empty() || !s.bytes().all(|b| b.is_ascii_digit()) {
        return Err(RtcMetricsError::InvalidSnowflake(raw.to_string()));
    }
    let val = s
        .parse::<u64>()
        .map_err(|_| RtcMetricsError::InvalidSnowflake(raw.to_string()))?;
    (val != 0)
        .then_some(val)
        .ok_or_else(|| RtcMetricsError::InvalidSnowflake(raw.to_string()))
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
    (((i32::from(b[o]) << 16) | (i32::from(b[o + 1]) << 8) | i32::from(b[o + 2])) << 8) >> 8
}

fn parse_report_blocks(body: &[u8], count: u8) -> Vec<RtcpReportBlock> {
    (0..usize::from(count))
        .map(|idx| {
            let o = idx * 24;
            RtcpReportBlock {
                ssrc: read_u32_be(body, o),
                fraction_lost: body[o + 4],
                cumulative_lost: read_i24_be(body, o + 5),
                extended_highest_seq: read_u32_be(body, o + 8),
                jitter_ticks: read_u32_be(body, o + 12),
                last_sr: read_u32_be(body, o + 16),
                delay_since_last_sr: read_u32_be(body, o + 20),
            }
        })
        .collect()
}

pub fn parse_rtcp_packet(bytes: &[u8]) -> Result<ParsedRtcpPacket, RtcMetricsError> {
    if bytes.len() < 8 {
        return Err(RtcMetricsError::TruncatedPacket(bytes.len()));
    }
    let (first, pt) = (bytes[0], bytes[1]);
    if (first >> 6) != RTCP_VERSION || (first & 0x20) != 0 {
        return Err(RtcMetricsError::InvalidRtcpVersion(first >> 6));
    }
    let (rc, lw) = (first & 0x1f, u16::from_be_bytes([bytes[2], bytes[3]]));
    let total = (usize::from(lw) + 1) * 4;
    let hdr_len = match pt {
        RTCP_PT_SR => 28,
        RTCP_PT_RR => 8,
        other => return Err(RtcMetricsError::InvalidPayloadType(other)),
    };
    if total < hdr_len + usize::from(rc) * 24 {
        return Err(RtcMetricsError::InvalidRtcpLength(lw));
    }
    if bytes.len() < total {
        return Err(RtcMetricsError::TruncatedPacket(bytes.len()));
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
        let rtt = rx_ntp.wrapping_sub(lsr).saturating_sub(dlsr);
        ((u64::from(rtt) * 1_000) + 32_768) / 65_536
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

#[must_use]
pub fn compute_bitrate_kbps(delta_bytes: u64, elapsed_ms: u64) -> u32 {
    if elapsed_ms == 0 {
        0
    } else {
        u32::try_from((delta_bytes.saturating_mul(8)) / elapsed_ms).unwrap_or(u32::MAX)
    }
}

#[derive(Debug, Clone, Default)]
pub struct InterarrivalJitterEstimator {
    last_rx_ticks: Option<u32>,
    last_rtp_ts: Option<u32>,
    jitter_fp4: u64,
}

impl InterarrivalJitterEstimator {
    pub fn observe_packet(&mut self, rx_ticks_48khz: u32, rtp_timestamp: u32) -> u32 {
        if let (Some(prev_rx), Some(prev_ts)) = (self.last_rx_ticks, self.last_rtp_ts) {
            let d_rx = i64::from(rx_ticks_48khz.wrapping_sub(prev_rx) as i32);
            let d_ts = i64::from(rtp_timestamp.wrapping_sub(prev_ts) as i32);
            let diff = (d_rx - d_ts).unsigned_abs();
            self.jitter_fp4 = self.jitter_fp4.saturating_sub(self.jitter_fp4 >> 4) + diff;
        }
        self.last_rx_ticks = Some(rx_ticks_48khz);
        self.last_rtp_ts = Some(rtp_timestamp);
        ((self.jitter_fp4 >> 4) as u32) / OPUS_SAMPLE_RATE_KHZ
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RtcTelemetrySample {
    pub time_ms: u64,
    pub rtt_ms: u64,
    pub jitter_ms: u32,
    pub loss_pct: f64,
    pub bitrate_kbps: u32,
    pub dave_epoch: u64,
}

#[derive(Debug, Clone)]
pub struct RtcpMetricsRingBuffer {
    pub capacity: usize,
    pub step_ms: u64,
    samples: VecDeque<RtcTelemetrySample>,
}

impl Default for RtcpMetricsRingBuffer {
    fn default() -> Self {
        let cap = RTC_METRICS_RING_CAPACITY.max(1);
        Self {
            capacity: cap,
            step_ms: RTC_METRICS_STEP_MS.max(1),
            samples: VecDeque::with_capacity(cap),
        }
    }
}

impl RtcpMetricsRingBuffer {
    pub fn poll_and_record_step(
        &mut self,
        sample: RtcTelemetrySample,
    ) -> Result<(), RtcMetricsError> {
        if let Some(last) = self.samples.back() {
            let elapsed = sample.time_ms.saturating_sub(last.time_ms);
            if elapsed < self.step_ms {
                return Err(RtcMetricsError::StepIntervalTooShort(elapsed));
            }
        }
        self.samples.push_back(sample);
        while self.samples.len() > self.capacity {
            self.samples.pop_front();
        }
        Ok(())
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }

    #[must_use]
    pub fn probe_state(&self, hovered: bool) -> TelemetryProbeState {
        TelemetryProbeState::resolve(!self.samples.is_empty(), hovered)
    }
}

#[derive(Debug, Clone, Default)]
pub struct PingHistory {
    samples: VecDeque<(u64, u64)>,
    pub ping_bad_count: u64,
    timeout_seqs: Vec<u64>,
}

impl PingHistory {
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
    pub fn average_ping_ms(&self) -> u64 {
        let n = self.samples.len().min(PING_AVERAGE_WINDOW);
        let sum: u64 = self.samples.iter().take(n).map(|(_, v)| *v).sum();
        if n == 0 { 0 } else { sum / (n as u64) }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.samples.len()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DaveEpochSnapshot {
    pub active: bool,
    pub protocol_version: u16,
    pub current_epoch: u64,
    pub transition_count: u64,
    pub roster_size: u32,
    pub encrypt_success_count: u64,
    pub decrypt_success_count: u64,
    pub passthrough_count: u64,
    pub failure_count: u64,
    pub downgrade_attempts: u32,
}

#[derive(Debug, Default)]
pub struct DaveMlsEpochMonitor {
    active: AtomicBool,
    protocol_version: AtomicU16,
    current_epoch: AtomicU64,
    prepared_epoch: AtomicU64,
    transition_count: AtomicU64,
    roster_size: AtomicU32,
    encrypt_success_count: AtomicU64,
    decrypt_success_count: AtomicU64,
    passthrough_count: AtomicU64,
    failure_count: AtomicU64,
    downgrade_attempts: AtomicU32,
}

impl DaveMlsEpochMonitor {
    pub fn on_frame_encrypted(&self) {
        self.encrypt_success_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn on_frame_decrypted(&self) {
        self.decrypt_success_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn on_frame_failure(&self) {
        self.failure_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn on_passthrough_frame(&self) {
        self.passthrough_count.fetch_add(1, Ordering::Relaxed);
    }

    pub fn on_prepare_epoch(
        &self,
        opcode: u8,
        requested_version: u16,
        next_epoch: u64,
    ) -> Result<(), RtcMetricsError> {
        if !(VOICE_OP_DAVE_PREPARE_TRANSITION..=VOICE_OP_MLS_INVALID_COMMIT).contains(&opcode) {
            return Err(RtcMetricsError::UnsupportedDaveOpcode(opcode));
        }
        let current_ver = self.protocol_version.load(Ordering::Acquire);
        if requested_version < current_ver {
            self.downgrade_attempts.fetch_add(1, Ordering::Relaxed);
            return Err(RtcMetricsError::DaveDowngradeRejected(requested_version));
        }
        if next_epoch < self.current_epoch.load(Ordering::Acquire) {
            return Err(RtcMetricsError::EpochRegressionRejected(next_epoch));
        }
        self.protocol_version
            .store(requested_version, Ordering::Release);
        self.prepared_epoch.store(next_epoch, Ordering::Release);
        Ok(())
    }

    pub fn on_execute_transition(&self, roster_size: u32) -> DaveEpochSnapshot {
        let next = self.prepared_epoch.load(Ordering::Acquire);
        self.current_epoch.store(next, Ordering::Release);
        let ver = self.protocol_version.load(Ordering::Acquire);
        self.active
            .store(ver >= DAVE_PROTOCOL_VERSION_V1, Ordering::Release);
        self.roster_size.store(roster_size, Ordering::Release);
        self.transition_count.fetch_add(1, Ordering::AcqRel);
        self.snapshot()
    }

    #[must_use]
    pub fn snapshot(&self) -> DaveEpochSnapshot {
        DaveEpochSnapshot {
            active: self.active.load(Ordering::Acquire),
            protocol_version: self.protocol_version.load(Ordering::Acquire),
            current_epoch: self.current_epoch.load(Ordering::Acquire),
            transition_count: self.transition_count.load(Ordering::Acquire),
            roster_size: self.roster_size.load(Ordering::Acquire),
            encrypt_success_count: self.encrypt_success_count.load(Ordering::Relaxed),
            decrypt_success_count: self.decrypt_success_count.load(Ordering::Relaxed),
            passthrough_count: self.passthrough_count.load(Ordering::Relaxed),
            failure_count: self.failure_count.load(Ordering::Relaxed),
            downgrade_attempts: self.downgrade_attempts.load(Ordering::Relaxed),
        }
    }
}

pub fn derive_privacy_code_from_authenticator(
    bytes: &[u8],
) -> Result<[String; 6], RtcMetricsError> {
    if bytes.len() < PRIVACY_CODE_TOTAL_DIGITS {
        return Err(RtcMetricsError::InvalidPrivacyCode("short".to_string()));
    }
    Ok(std::array::from_fn(|group| {
        let off = group * PRIVACY_CODE_GROUP_LEN;
        let val =
            (0..PRIVACY_CODE_GROUP_LEN).fold(0u64, |acc, i| (acc << 8) | u64::from(bytes[off + i]));
        format!("{:05}", val % 100_000)
    }))
}

pub fn format_voice_privacy_code(raw: &str) -> Result<[String; 6], RtcMetricsError> {
    let mut digits = String::with_capacity(PRIVACY_CODE_TOTAL_DIGITS);
    for ch in raw.chars() {
        if ch.is_ascii_digit() {
            digits.push(ch);
        } else if !(ch.is_whitespace() || ch == '-') {
            return Err(RtcMetricsError::InvalidPrivacyCode(raw.to_string()));
        }
    }
    if digits.len() != PRIVACY_CODE_TOTAL_DIGITS {
        return Err(RtcMetricsError::InvalidPrivacyCode(raw.to_string()));
    }
    Ok(std::array::from_fn(|g| {
        digits[g * PRIVACY_CODE_GROUP_LEN..(g + 1) * PRIVACY_CODE_GROUP_LEN].to_string()
    }))
}

#[must_use]
pub fn should_show_verification_code(ch: &str, active: &str, stage: bool, ver: u16) -> bool {
    !ch.is_empty() && ch == active && !stage && ver >= DAVE_PROTOCOL_VERSION_V1
}

pub fn evaluate_voice_metrics_permissions(
    perms: u64,
    is_dm: bool,
    age_restricted: bool,
) -> Result<(), RtcMetricsError> {
    if age_restricted {
        return Err(RtcMetricsError::AgeGatedSpaceRestricted);
    }
    let req = PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK;
    if !is_dm && (perms & req) != req {
        return Err(RtcMetricsError::MissingPermission("VOICE_METRICS"));
    }
    Ok(())
}

pub fn validate_platform_boundaries(
    chars: usize,
    atts: usize,
    bytes: u64,
    offset: u32,
    vol: u16,
) -> Result<(), RtcMetricsError> {
    if chars > MAX_MESSAGE_CHARS
        || atts > MAX_ATTACHMENTS_COUNT
        || bytes > MAX_ATTACHMENT_BYTES
        || offset > MAX_SEARCH_OFFSET
        || vol > MAX_VOLUME_PERCENT
    {
        return Err(RtcMetricsError::BoundaryViolation("limit_exceeded"));
    }
    Ok(())
}

pub fn classify_rtc_http_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<&'static str, RtcMetricsError> {
    match status {
        200 | 204 => Ok("ready"),
        202 => Ok("skeleton"),
        _ => Err(RtcMetricsError::HttpStatusError(
            status,
            retry_after_ms.max(u64::from(status == 429)),
        )),
    }
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
pub fn redact_rtc_metrics_log(raw: &str) -> String {
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
    fn test_sub_1011a_1_tokens_layout_and_probe_states() {
        assert_eq!(RTCP_COLLECTOR_CONTAINER, "RTCP Collector");
        assert_eq!(RTCP_COLLECTOR_DIMENSIONS, "RingBuffer 120");
        assert_eq!(TELEMETRY_PROBE_ELEMENT, "telemetry-probe");
        assert_eq!((TOKEN_BACKGROUND, TOKEN_SURFACE), ("#1e1f22", "#2b2d31"));
        assert_eq!((TOKEN_TEXT_PRIMARY, TOKEN_ACCENT), ("#ffffff", "#23a55a"));
        assert_eq!((RTC_METRICS_RING_CAPACITY, RTC_METRICS_STEP_MS), (120, 500));
        assert_eq!(
            TelemetryProbeState::resolve(false, false).as_str(),
            "polling"
        );
        assert_eq!(
            TelemetryProbeState::resolve(false, true).as_str(),
            "collect"
        );
        assert_eq!(
            TelemetryProbeState::resolve(true, false).as_str(),
            "buffered"
        );
    }

    #[test]
    fn test_sub_1011a_2_rtcp_packets_rtt_jitter_and_loss() {
        let sr = RtcpSenderInfo {
            ntp_timestamp: 0x83aa_7e80_8000_0000,
            rtp_timestamp: 48_000,
            packet_count: 250,
            octet_count: 30_000,
        };
        let sr_pkt = parse_rtcp_packet(&build_srtcp_sender_report(17_843, &sr, 7)).unwrap();
        assert_eq!((sr_pkt.payload_type, sr_pkt.trailer_len), (200, 32));
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
        assert_eq!(
            (rr_pkt.payload_type, rr_pkt.reports[0].jitter_ms()),
            (201, 20)
        );
        assert_eq!(rr_pkt.reports[0].cumulative_lost, -3_128_775);
        assert_eq!(
            compute_rtt_from_lsr_dlsr_ms(0x7e81_c000, 0x7e80_8000, 65_536),
            Some(250)
        );
        assert_eq!(compute_outbound_loss_delta_pct(100, 2, 190, 12), 10.0);
        assert_eq!(compute_bitrate_kbps(8_000, 1_000), 64);
        let mut jitter = InterarrivalJitterEstimator::default();
        assert_eq!(
            (
                jitter.observe_packet(0, 0),
                jitter.observe_packet(16_320, 960)
            ),
            (0, 20)
        );
    }

    #[test]
    fn test_sub_1011a_2_ring_buffer_120_and_ping_quality() {
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
        assert_eq!(ring.probe_state(false).as_str(), "polling");
        for i in 0..125u64 {
            let s = RtcTelemetrySample {
                time_ms: 1_000 + i * 500,
                rtt_ms: 40,
                jitter_ms: 6,
                loss_pct: 0.5,
                bitrate_kbps: 64,
                dave_epoch: 1,
            };
            ring.poll_and_record_step(s).unwrap();
        }
        assert_eq!((ring.len(), ring.capacity, ring.step_ms), (120, 120, 500));
        assert_eq!(ring.probe_state(false).as_str(), "buffered");
        let mut bad = ring.samples[119];
        bad.time_ms += 200;
        assert!(ring.poll_and_record_step(bad).is_err());
    }

    #[test]
    fn test_sub_1011a_3_dave_mls_epoch_monitor_and_privacy_code() {
        let mon = DaveMlsEpochMonitor::default();
        mon.on_frame_encrypted();
        mon.on_frame_decrypted();
        mon.on_passthrough_frame();
        mon.on_frame_failure();
        mon.on_prepare_epoch(VOICE_OP_DAVE_PREPARE_EPOCH, DAVE_PROTOCOL_VERSION_V1, 1)
            .unwrap();
        let snap1 = mon.on_execute_transition(4);
        assert!(snap1.active);
        assert_eq!((snap1.protocol_version, snap1.current_epoch), (1, 1));
        assert!(
            mon.on_prepare_epoch(VOICE_OP_DAVE_PREPARE_TRANSITION, 0, 2)
                .is_err()
        );
        assert_eq!(mon.snapshot().downgrade_attempts, 1);
        let chunks = derive_privacy_code_from_authenticator(&[17u8; 30]).unwrap();
        assert_eq!(
            format_voice_privacy_code(&chunks.join(" ")).unwrap(),
            chunks
        );
        assert!(should_show_verification_code("120", "120", false, 1));
        assert!(!should_show_verification_code("120", "120", true, 1));
    }

    #[test]
    fn test_sub_1011a_4_nine_dimensions_and_security() {
        assert!(validate_snowflake_id("0").is_err());
        assert_eq!(
            validate_snowflake_id("18446744073709551615").unwrap(),
            u64::MAX
        );
        let mut pings = PingHistory::default();
        for i in 0..205u64 {
            pings.push_ping(i * 100, 40);
        }
        assert_eq!((pings.len(), pings.average_ping_ms()), (199, 40));
        assert!(!pings.note_ping_timeout(10, 1_000) && !pings.note_ping_timeout(11, 2_000));
        assert!(pings.note_ping_timeout(12, 3_000));
        assert_eq!(
            (pings.last_ping_ms(), pings.ping_bad_count),
            (Some(4_000), 1)
        );
        let perms = PERM_VIEW_CHANNEL | PERM_CONNECT | PERM_SPEAK;
        assert!(evaluate_voice_metrics_permissions(perms, false, false).is_ok());
        assert!(evaluate_voice_metrics_permissions(perms, false, true).is_err());
        assert!(validate_platform_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200).is_ok());
        assert!(validate_platform_boundaries(2_001, 10, 100, 0, 100).is_err());
        assert_eq!(classify_rtc_http_status(202, 0).unwrap(), "skeleton");
        assert!(classify_rtc_http_status(429, 1_500).is_err());
        assert_eq!(
            (format_badge_cap(99), format_badge_cap(100)),
            ("99".to_string(), "99+".to_string())
        );
        let masked = redact_rtc_metrics_log("Bearer mfa.secret_token_999 secret_key=cafebabe");
        assert!(!masked.contains("secret_token_999") && !masked.contains("cafebabe"));
    }
}
