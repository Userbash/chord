//! Подзадача 905.1 (`SUB-905-1`): Сетевой хаос-инжектор (Network Fault Injection).

use serde::{Deserialize, Serialize};
use std::time::Duration;

use super::ChaosError;
use crate::audio::{
    AdaptiveJitterBuffer, JITTER_BUFFER_MAX_MS, JITTER_BUFFER_MIN_MS, JitterPopStatus,
    OPUS_DEFAULT_BITRATE_BPS, OpusBitrateController,
};
use crate::net::{Http2PoolConfig, classify_gateway_close_code};
use chord_core::CloseAction;

/// Код операции `IDENTIFY` шлюза Discord (Opcode 2).
pub const GATEWAY_OPCODE_IDENTIFY: u8 = 2;
/// Код операции `RESUME` шлюза Discord (Opcode 6).
pub const GATEWAY_OPCODE_RESUME: u8 = 6;

/// Тип эмулируемого обрыва TCP-соединения шлюза.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TcpDisconnectKind {
    SocketReset,
    UnexpectedEof,
    CloseCode(u16),
    InvalidSession { resumable: bool },
}

/// Расписание периодических обрывов TCP-соединения шлюза каждые `interval_secs` секунд.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TcpDisconnectSchedule {
    pub interval_secs: u64,
    pub total_duration_secs: u64,
    pub kind: TcpDisconnectKind,
}

impl TcpDisconnectSchedule {
    pub fn new(
        interval_secs: u64,
        total_duration_secs: u64,
        kind: TcpDisconnectKind,
    ) -> Result<Self, ChaosError> {
        if interval_secs == 0 || total_duration_secs < interval_secs {
            return Err(ChaosError::InvalidIntervalSecs(interval_secs));
        }
        Ok(Self {
            interval_secs,
            total_duration_secs,
            kind,
        })
    }

    pub fn expected_disconnect_count(&self) -> u32 {
        (self.total_duration_secs / self.interval_secs) as u32
    }
}

/// Полезная нагрузка `Opcode 6 (RESUME)` шлюза Discord.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResumeOpcode6Payload {
    pub op: u8,
    pub token: String,
    pub session_id: String,
    pub seq: u64,
}

impl ResumeOpcode6Payload {
    pub const OPCODE_RESUME: u8 = GATEWAY_OPCODE_RESUME;

    pub fn new(token: impl Into<String>, session_id: impl Into<String>, seq: u64) -> Self {
        Self {
            op: Self::OPCODE_RESUME,
            token: token.into(),
            session_id: session_id.into(),
            seq,
        }
    }

    pub fn to_wire_json(&self) -> serde_json::Value {
        serde_json::json!({
            "op": self.op,
            "d": {
                "token": self.token,
                "session_id": self.session_id,
                "seq": self.seq,
            }
        })
    }

    pub fn to_redacted_json(&self) -> serde_json::Value {
        serde_json::json!({
            "op": self.op,
            "d": {
                "token": "[REDACTED]",
                "session_id": "[REDACTED]",
                "seq": self.seq,
            }
        })
    }
}

/// Результат одного цикла обрыва TCP и восстановления сессии шлюза.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GatewayDisconnectCycle {
    pub cycle_index: u32,
    pub elapsed_secs: u64,
    pub disconnect_kind: TcpDisconnectKind,
    pub pre_disconnect_seq: u64,
    pub post_resume_seq: u64,
    pub resume_opcode: u8,
    pub session_id_preserved: bool,
    pub used_resume_gateway_url: bool,
    pub recovered: bool,
}

/// Инжектор сетевых сбоев WebSocket-шлюза Discord (TCP Reset / EOF -> Opcode 6 RESUME).
pub struct GatewayChaosInjector {
    session_id: String,
    token: String,
    resume_gateway_url: String,
    current_seq: u64,
}

impl GatewayChaosInjector {
    pub fn new(
        session_id: impl Into<String>,
        token: impl Into<String>,
        resume_gateway_url: impl Into<String>,
        initial_seq: u64,
    ) -> Self {
        Self {
            session_id: session_id.into(),
            token: token.into(),
            resume_gateway_url: resume_gateway_url.into(),
            current_seq: initial_seq,
        }
    }

    pub fn current_seq(&self) -> u64 {
        self.current_seq
    }

    pub fn build_resume_payload(&self) -> ResumeOpcode6Payload {
        ResumeOpcode6Payload::new(&self.token, &self.session_id, self.current_seq)
    }

    fn resolve_recovery_mode(kind: TcpDisconnectKind) -> Result<(u8, bool), ChaosError> {
        match kind {
            TcpDisconnectKind::SocketReset | TcpDisconnectKind::UnexpectedEof => {
                Ok((GATEWAY_OPCODE_RESUME, true))
            }
            TcpDisconnectKind::InvalidSession { resumable } => {
                let opcode = if resumable {
                    GATEWAY_OPCODE_RESUME
                } else {
                    GATEWAY_OPCODE_IDENTIFY
                };
                Ok((opcode, resumable))
            }
            TcpDisconnectKind::CloseCode(code) => match classify_gateway_close_code(Some(code)) {
                CloseAction::Reconnect => Ok((GATEWAY_OPCODE_RESUME, true)),
                CloseAction::Stop(_) => Err(ChaosError::NonResumableCloseCode(code)),
            },
        }
    }

    pub fn run_schedule(
        &mut self,
        schedule: &TcpDisconnectSchedule,
        events_per_interval: u64,
    ) -> Result<Vec<GatewayDisconnectCycle>, ChaosError> {
        let count = schedule.expected_disconnect_count();
        let (resume_opcode, session_id_preserved) = Self::resolve_recovery_mode(schedule.kind)?;
        let used_resume_url = session_id_preserved && !self.resume_gateway_url.trim().is_empty();
        let mut cycles = Vec::with_capacity(count as usize);

        for idx in 1..=count {
            self.current_seq = self.current_seq.saturating_add(events_per_interval);
            let pre_seq = self.current_seq;
            let post_seq = if session_id_preserved { pre_seq } else { 0 };
            self.current_seq = post_seq;
            cycles.push(GatewayDisconnectCycle {
                cycle_index: idx,
                elapsed_secs: u64::from(idx).saturating_mul(schedule.interval_secs),
                disconnect_kind: schedule.kind,
                pre_disconnect_seq: pre_seq,
                post_resume_seq: post_seq,
                resume_opcode,
                session_id_preserved,
                used_resume_gateway_url: used_resume_url,
                recovered: !self.session_id.is_empty() && !self.token.is_empty(),
            });
        }
        Ok(cycles)
    }
}

/// Результат инжекции HTTP 429 / 5xx / 4xx в REST-клиент.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Http429RetryOutcome {
    pub status: u16,
    pub attempt: u32,
    pub injected_retry_after_ms: Option<u64>,
    pub computed_backoff_ms: Option<u64>,
    pub will_retry: bool,
}

/// Инжектор ошибок HTTP 429 Too Many Requests и проверка экспоненциального бэкоффа.
pub struct Http429FaultInjector;

impl Http429FaultInjector {
    pub fn evaluate_status_backoff(
        status: u16,
        retry_after_ms: Option<u64>,
        attempt: u32,
    ) -> Result<Http429RetryOutcome, ChaosError> {
        let pool_cfg = Http2PoolConfig::default();
        let (headers, body) = Self::build_synthetic_429_wire(status, retry_after_ms);
        let computed = pool_cfg
            .retry_delay_for_response(status, &headers, &body, attempt.max(1))
            .map(|d| d.as_millis() as u64);

        Ok(Http429RetryOutcome {
            status,
            attempt: attempt.max(1),
            injected_retry_after_ms: retry_after_ms,
            computed_backoff_ms: computed,
            will_retry: computed.is_some(),
        })
    }

    fn build_synthetic_429_wire(
        status: u16,
        retry_after_ms: Option<u64>,
    ) -> (Vec<(String, String)>, Vec<u8>) {
        if status != 429 {
            return (Vec::new(), Vec::new());
        }
        if let Some(ms) = retry_after_ms {
            let secs = (ms as f64) / 1_000.0;
            let headers = vec![("retry-after".to_string(), format!("{secs:.3}"))];
            let body = serde_json::json!({
                "message": "You are being rate limited.",
                "retry_after": secs,
                "global": false
            })
            .to_string()
            .into_bytes();
            (headers, body)
        } else {
            (Vec::new(), b"{}".to_vec())
        }
    }

    pub fn run_429_burst_series(
        retry_after_series_ms: &[u64],
    ) -> Result<Vec<Http429RetryOutcome>, ChaosError> {
        let mut outcomes = Vec::with_capacity(retry_after_series_ms.len());
        for (i, &ms) in retry_after_series_ms.iter().enumerate() {
            if ms == 0 {
                return Err(ChaosError::InvalidRetryAfterMs(0));
            }
            outcomes.push(Self::evaluate_status_backoff(
                429,
                Some(ms),
                (i + 1) as u32,
            )?);
        }
        Ok(outcomes)
    }

    pub fn deterministic_jitter_retry_after_ms(seed: u64, min_ms: u64, max_ms: u64) -> Duration {
        let low = min_ms.min(max_ms);
        let high = max_ms.max(min_ms);
        let span = high.saturating_sub(low).saturating_add(1);
        let mixed = seed
            .wrapping_mul(6_364_136_223_846_793_005)
            .wrapping_add(1_442_695_040_888_963_407);
        Duration::from_millis(low.saturating_add(mixed % span))
    }
}

/// Отчёт симуляции потерь UDP-пакетов голоса (5%, 15%, 30%) и джиттера (20–60 мс).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UdpLossSimulationReport {
    pub loss_percent: u8,
    pub rtcp_fraction_lost: u8,
    pub injected_jitter_ms: u32,
    pub total_packets_sent: u32,
    pub packets_delivered: u32,
    pub packets_dropped: u32,
    pub initial_bitrate_bps: u32,
    pub adapted_bitrate_bps: u32,
    pub recovered_bitrate_bps: u32,
    pub jitter_buffer_target_ms: u32,
    pub plc_frames_generated: u64,
    pub ready_frames_popped: u64,
}

/// Инжектор потерь UDP-пакетов (5%, 15%, 30%) и джиттера 20–60 мс для голосового тракта Opus.
pub struct UdpVoiceChaosInjector;

impl UdpVoiceChaosInjector {
    pub fn loss_percent_to_fraction_lost(loss_percent: u8) -> Result<u8, ChaosError> {
        if loss_percent > 100 {
            return Err(ChaosError::InvalidLossPercent(loss_percent));
        }
        let scaled = (u32::from(loss_percent) * 256 + 50) / 100;
        Ok(scaled.min(255) as u8)
    }

    pub fn simulate_udp_loss_and_jitter(
        loss_percent: u8,
        jitter_ms: u32,
        total_packets: u32,
        start_sequence: u16,
    ) -> Result<UdpLossSimulationReport, ChaosError> {
        let fraction_lost = Self::loss_percent_to_fraction_lost(loss_percent)?;
        if !(JITTER_BUFFER_MIN_MS..=JITTER_BUFFER_MAX_MS).contains(&jitter_ms) {
            return Err(ChaosError::InvalidJitterMs(jitter_ms));
        }
        let mut opus_ctrl = OpusBitrateController::new();
        let initial_bps = opus_ctrl.target_bitrate_bps();
        let mut jitter_buf = AdaptiveJitterBuffer::with_target_delay_ms(jitter_ms);

        let report_rounds = if loss_percent >= 25 { 2 } else { 1 };
        for _ in 0..report_rounds {
            opus_ctrl.on_rtcp_fraction_lost(fraction_lost);
            jitter_buf.on_rtcp_fraction_lost(fraction_lost);
        }
        let adapted_bps = opus_ctrl.target_bitrate_bps();
        let target_ms = jitter_buf.target_delay_ms();

        let (delivered, dropped, ready_popped) =
            Self::pump_udp_frames(&mut jitter_buf, loss_percent, total_packets, start_sequence)?;

        while opus_ctrl.target_bitrate_bps() < OPUS_DEFAULT_BITRATE_BPS {
            opus_ctrl.on_rtcp_fraction_lost(0);
        }

        Ok(UdpLossSimulationReport {
            loss_percent,
            rtcp_fraction_lost: fraction_lost,
            injected_jitter_ms: jitter_ms,
            total_packets_sent: total_packets,
            packets_delivered: delivered,
            packets_dropped: dropped,
            initial_bitrate_bps: initial_bps,
            adapted_bitrate_bps: adapted_bps,
            recovered_bitrate_bps: opus_ctrl.target_bitrate_bps(),
            jitter_buffer_target_ms: target_ms,
            plc_frames_generated: jitter_buf.plc_frames_generated(),
            ready_frames_popped: ready_popped,
        })
    }

    fn pump_udp_frames(
        jitter_buf: &mut AdaptiveJitterBuffer,
        loss_percent: u8,
        total_packets: u32,
        start_sequence: u16,
    ) -> Result<(u32, u32, u64), ChaosError> {
        let drop_target = (total_packets * u32::from(loss_percent)) / 100;
        let interior_slots = total_packets.saturating_sub(2).max(1);
        let mut dropped = 0u32;
        let mut delivered = 0u32;
        let mut ready_popped = 0u64;

        for idx in 0..total_packets {
            let is_interior = idx > 0 && idx + 1 < total_packets;
            let slot = idx.saturating_sub(1);
            let should_drop = is_interior
                && (((slot + 1) * drop_target) / interior_slots)
                    > ((slot * drop_target) / interior_slots);
            if should_drop {
                dropped += 1;
                continue;
            }
            let seq = start_sequence.wrapping_add(idx as u16);
            let ts = idx.wrapping_mul(960);
            let _ = jitter_buf.push(seq, ts, vec![0xF8, 0xFF, 0xFE]);
            delivered += 1;
            while !jitter_buf.is_buffering() && jitter_buf.len() > jitter_buf.target_frames() {
                if let JitterPopStatus::Ready(_) = jitter_buf.pop() {
                    ready_popped += 1;
                }
            }
        }
        while !jitter_buf.is_empty() {
            if let JitterPopStatus::Ready(_) = jitter_buf.pop() {
                ready_popped += 1;
            }
        }
        Ok((delivered, dropped, ready_popped))
    }
}

/// Сводка сетевого хаос-тестирования (`SUB-905-1`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NetworkChaosSummary {
    pub tcp_disconnects_injected: u32,
    pub resume_opcode6_verified: u32,
    pub http_429_injected: u32,
    pub udp_loss_profiles_verified: Vec<u8>,
    pub min_opus_bitrate_bps: u32,
    pub max_opus_bitrate_bps: u32,
    pub total_plc_frames: u64,
}
