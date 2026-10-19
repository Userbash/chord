//! Заглушка RW-04 (red-фаза): API поверхности транспорта шлюза без реализации.
//!
//! Временно подменяет настоящий `ws.rs` только на время повторного red-прогона
//! (тест-файл уточнён: стаб терпит ожидаемый обрыв после `Reconnect`); после прогона
//! файл возвращается из `/tmp/opencode/rw-04/ws-impl.rs`.

use crate::gateway::{Effect, ReconnectReason, Session};

/// Пайп шлюза: заглушка red-фазы.
pub struct GatewayPipe;

impl GatewayPipe {
    /// Заглушка red-фазы.
    pub fn new(_session: Session) -> Self {
        todo!("RW-04: пайп (red-фаза)")
    }

    /// Заглушка red-фазы.
    pub fn push_binary(&mut self, _msg: &[u8], _now_ms: u64) -> Result<Vec<Effect>, TransportError> {
        todo!("RW-04: пайп (red-фаза)")
    }

    /// Заглушка red-фазы.
    pub fn session(&self) -> &Session {
        todo!("RW-04: пайп (red-фаза)")
    }

    /// Заглушка red-фазы.
    pub fn frames_received(&self) -> u64 {
        todo!("RW-04: пайп (red-фаза)")
    }

    /// Заглушка red-фазы.
    pub fn bytes_received(&self) -> u64 {
        todo!("RW-04: пайп (red-фаза)")
    }

    /// Заглушка red-фазы.
    pub fn terms_parsed(&self) -> u64 {
        todo!("RW-04: пайп (red-фаза)")
    }

    /// Заглушка red-фазы.
    pub fn pending_bytes(&self) -> usize {
        todo!("RW-04: пайп (red-фаза)")
    }

    /// Заглушка red-фазы.
    pub fn ignored_messages(&self) -> u64 {
        todo!("RW-04: пайп (red-фаза)")
    }

    /// Заглушка red-фазы.
    pub fn note_ignored_message(&mut self) {
        todo!("RW-04: пайп (red-фаза)")
    }
}

/// Кадр heartbeat (op1): заглушка red-фазы.
pub fn encode_heartbeat(_d: Option<u64>) -> Vec<u8> {
    todo!("RW-04: heartbeat (red-фаза)")
}

/// Исход прогона: заглушка red-фазы.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunOutcome {
    /// Нужен реконнект.
    Reconnect {
        /// Почему нужен реконнект.
        reason: ReconnectReason,
    },
    /// Соединение закрыто.
    Closed {
        /// Код закрытия WS, если он был в кадре.
        code: Option<u16>,
    },
}

/// Ошибки транспорта: заглушка red-фазы.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransportError {
    /// Соединение не установлено.
    Connect {
        /// Текст ошибки.
        msg: String,
    },
    /// Ошибка WebSocket-слоя.
    Ws {
        /// Текст ошибки.
        msg: String,
    },
    /// Ошибка декодера zstd-потока.
    Zstd(discord_codec::zstd::ZstdError),
    /// Ошибка кадра.
    Frame(crate::gateway::GatewayError),
}

impl std::fmt::Display for TransportError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "заглушка транспорта RW-04 (red-фаза)")
    }
}

impl std::error::Error for TransportError {}

/// Подключение к шлюзу: заглушка red-фазы.
pub async fn connect_and_run(
    _url: &str,
    _pipe: &mut GatewayPipe,
) -> Result<RunOutcome, TransportError> {
    todo!("RW-04: соединение (red-фаза)")
}
