//! # RESUME (op6) и политика реконнекта (RW-06, P1-05 / T-GW-05…07)
//!
//! RESUME продолжает прерванную сессию шлюза: клиент шлёт `{op:6, d:{token, session_id, seq}}`,
//! сервер отвечает `RESUMED` и продолжает `s`-серию без разрывов. Оракулы:
//! - `rest-static.md` §4: `RESUME {token,session_id,seq}`; «при INVALID_SESSION(false) → IDENTIFY»
//!   (код веб-клиента `GatewaySocket._doResume`/`_connect`);
//! - run2-артефакты `gw-proto/msgs-run2-95974_{8269,8608,10029,13072}.jsonl`: швы серий
//!   663→664 и 331→332, `.13072` начинается с RESUMED s=332, исходящие RESUME — 173/176 Б.
//!
//! Политика (T-GW-07, контракт плана): после обрыва супервайзер решает
//! [`plan_reconnect`]: разрыв меньше [`RESUME_MAX_DOWNTIME_MS`] и сессия известна и сервер не
//! запретил resume (`op9 d=false`) → [`ReconnectAction::Resume`], иначе — свежий IDENTIFY.
//! Клиент сам не переподключается: цикл транспорта возвращает исход, решение — снаружи.

use discord_codec::etf::{self, EtfError, EtfValue};

/// Порог удержания сессии: разрыв `>= 30 000 мс` — новый IDENTIFY (T-GW-07: 3 с → resume,
/// 30 с → IDENTIFY). Сервер может переопределить решение через `op9 d=false`.
pub const RESUME_MAX_DOWNTIME_MS: u64 = 30_000;

/// Параметры RESUME (op6): `{token, session_id, seq}`.
#[derive(Clone)]
pub struct ResumeParams {
    /// Токен пользователя: **никогда** не попадает в `Debug` (см. `impl Debug`).
    pub token: String,
    /// session_id из READY (удерживается супервайзером между соединениями).
    pub session_id: String,
    /// Последний принятый `s` (RESUME с ним — сервер продолжит с `seq + 1`).
    pub seq: u64,
}

impl std::fmt::Debug for ResumeParams {
    /// Токен заменяется на `"REDACTED"` (политика санитизации).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResumeParams")
            .field("token", &"REDACTED")
            .field("session_id", &self.session_id)
            .field("seq", &self.seq)
            .finish()
    }
}

/// Состояние сессии, пригодное для RESUME (session_id из READY + последний `s`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeState {
    /// session_id из READY.
    pub session_id: String,
    /// Последний принятый `s`.
    pub last_seq: u64,
}

/// Ошибки сборки RESUME: ни одна не является паникой.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeError {
    /// Пустой токен.
    EmptyToken,
    /// Пустой session_id.
    EmptySessionId,
    /// Собранный терм не разбирается кодеком (самопроверка сборки).
    Encode(EtfError),
}

impl std::fmt::Display for ResumeError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyToken => write!(f, "пустой токен"),
            Self::EmptySessionId => write!(f, "пустой session_id"),
            Self::Encode(error) => write!(f, "собранный RESUME не разбирается ETF-кодеком: {error}"),
        }
    }
}

impl std::error::Error for ResumeError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::EmptyToken | Self::EmptySessionId => None,
        }
    }
}

/// Решение супервайзера о следующем подключении.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReconnectAction {
    /// Продолжить серию через RESUME (op6).
    Resume,
    /// Свежий IDENTIFY (op2).
    Identify,
}

/// Ключ map в форме клиентского wetf-энкодера (`key:"binary"`).
fn key(name: &str) -> EtfValue {
    EtfValue::Str(name.to_string())
}

/// `u64` → `i64` с насыщением (значения протокола не превышают `i64::MAX`).
fn int(value: u64) -> EtfValue {
    EtfValue::Int(i64::try_from(value).unwrap_or(i64::MAX))
}

/// Собирает проводную форму RESUME (op6) с ведущим `0x83`.
///
/// Самопроверка: собранные байты обязаны полностью разбираться тем же кодеком; иначе —
/// [`ResumeError::Encode`]. Пустые `token`/`session_id` — ошибки (невалидный RESUME не уйдёт).
pub fn build_resume_frame(params: &ResumeParams) -> Result<Vec<u8>, ResumeError> {
    if params.token.is_empty() {
        return Err(ResumeError::EmptyToken);
    }
    if params.session_id.is_empty() {
        return Err(ResumeError::EmptySessionId);
    }
    let frame = EtfValue::Map(vec![
        (key("op"), EtfValue::Int(6)),
        (
            key("d"),
            EtfValue::Map(vec![
                (key("token"), EtfValue::Str(params.token.clone())),
                (key("session_id"), EtfValue::Str(params.session_id.clone())),
                (key("seq"), int(params.seq)),
            ]),
        ),
    ]);
    self_check(etf::encode(&frame))
}

/// Самопроверка сборки: терм обязан разбираться кодеком целиком.
fn self_check(bytes: Vec<u8>) -> Result<Vec<u8>, ResumeError> {
    let (_decoded, consumed) = etf::decode(&bytes).map_err(ResumeError::Encode)?;
    if consumed != bytes.len() {
        return Err(ResumeError::Encode(EtfError::UnexpectedEof {
            at: consumed,
            need: bytes.len() - consumed,
        }));
    }
    Ok(bytes)
}

/// Решает, продолжать серию через RESUME или начинать с IDENTIFY (T-GW-07).
///
/// `Identify`, если сессии нет, разрыв не меньше [`RESUME_MAX_DOWNTIME_MS`] либо сервер запретил
/// resume (`invalid_session == Some(false)`); иначе `Resume` (`Some(true)` — можно пробовать
/// снова, `None` — op9 не было или `d` не распознан).
pub fn plan_reconnect(
    state: Option<&ResumeState>,
    downtime_ms: u64,
    invalid_session: Option<bool>,
) -> ReconnectAction {
    if state.is_none() {
        return ReconnectAction::Identify;
    }
    if downtime_ms >= RESUME_MAX_DOWNTIME_MS {
        return ReconnectAction::Identify;
    }
    if invalid_session == Some(false) {
        return ReconnectAction::Identify;
    }
    ReconnectAction::Resume
}
