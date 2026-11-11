//! Стенд Chaos Engineering (`TASK-905`):
//! - `network_faults` (`SUB-905-1`): обрывы TCP каждые N сек с проверкой `RESUME` (Opcode 6),
//!   инжекция HTTP 429 `retry_after_ms` с экспоненциальным бэкоффом, потери 5%/15%/30% UDP
//!   пакетов и джиттер 20–60 мс для проверки адаптивного битрейта Opus и PLC.
//! - `hardware_faults` (`SUB-905-2`): внезапное удаление аудиоустройств с авто-переключением
//!   на Default, аварийный перезапуск демона PipeWire во время скриншаринга, OOM-сброс `MediaCache`.
//! - `state_integrity` (`SUB-905-3`): идемпотентность ленты сообщений после серии реконнектов,
//!   сохранение черновика `draft_text` при закрытии модалок, контроль 0 утечек FD и сокетов
//!   на 100 последовательных циклах сбоя.

#![allow(dead_code)]

pub mod hardware_faults;
pub mod network_faults;
pub mod state_integrity;

use serde::{Deserialize, Serialize};

pub use hardware_faults::{
    AudioDeviceChaosController, HardwareChaosSummary, OomChaosInjector, PipeWireChaosSupervisor,
};
pub use network_faults::{
    GatewayChaosInjector, Http429FaultInjector, NetworkChaosSummary, ResumeOpcode6Payload,
    TcpDisconnectKind, TcpDisconnectSchedule, UdpVoiceChaosInjector,
};
pub use state_integrity::{
    ChaosResilienceRunner, DraftIntegrityGuard, ReconnectMessageDeduplicator, ResourceLeakTracker,
    StateIntegritySummary,
};

/// Ошибки стенда Chaos Engineering.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ChaosError {
    InvalidIntervalSecs(u64),
    InvalidLossPercent(u8),
    InvalidJitterMs(u32),
    InvalidRetryAfterMs(u64),
    NonResumableCloseCode(u16),
    InvalidPipeWireFd(i32),
    InvalidPipeWireNodeId(u32),
    AudioFallbackUnavailable,
    PipeWireDaemonOffline(String),
    ResourceLeakDetected {
        leaked_fds: usize,
        leaked_sockets: usize,
    },
    DuplicateMessagesDetected(usize),
    DraftCorrupted {
        expected: String,
        actual: String,
    },
    SerializationError(String),
}

impl std::fmt::Display for ChaosError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

impl std::error::Error for ChaosError {}

/// Сводный отчёт выполнения стенда Chaos Engineering (`TASK-905`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChaosAuditReport {
    pub cycles_completed: u32,
    pub network: NetworkChaosSummary,
    pub hardware: HardwareChaosSummary,
    pub integrity: StateIntegritySummary,
    pub leaked_fds: usize,
    pub leaked_sockets: usize,
    pub redacted_credentials_verified: bool,
    pub passed: bool,
}

impl ChaosAuditReport {
    /// Сериализация отчёта в форматированный JSON (без секретов).
    pub fn to_json_pretty(&self) -> Result<String, ChaosError> {
        serde_json::to_string_pretty(self)
            .map_err(|e| ChaosError::SerializationError(e.to_string()))
    }
}
