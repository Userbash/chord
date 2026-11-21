#![forbid(unsafe_code)]

//! # RESUME (op6) и политика реконнекта с экспоненциальным backoff 1s..60s (RW-06, P1-05 / T-GW-05…07, STAGE-05.3.2)
//!
//! RESUME продолжает прерванную сессию шлюза без сброса кэша гильдий и без повторной загрузки каналов:
//! клиент шлёт `{op:6, d:{token, session_id, seq}}`, сервер отвечает `RESUMED`, воспроизводит пропущенные
//! события и продолжает `s`-серию без разрывов. Оракулы:
//! - `rest-static.md` §4: `RESUME {token,session_id,seq}`; «при INVALID_SESSION(false) → IDENTIFY»
//!   (код веб-клиента `GatewaySocket._doResume`/`_connect`);
//! - `gatewayBackoff = new Backoff(1e3, 6e4)`: экспоненциальный backoff `1000..=60000` мс (`1s -> 2s -> 4s -> 8s -> 16s -> 32s -> 60s`),
//!   при `fails > 4` (`RESUME_RESET_AFTER_FAILURES = 4`) сессия сбрасывается в свежий `IDENTIFY`;
//! - run2-артефакты `gw-proto/msgs-run2-95974_{8269,8608,10029,13072}.jsonl`: швы серий
//!   663→664 и 331→332, `.13072` начинается с RESUMED s=332, исходящие RESUME — 173/176 Б.
//!
//! Политика (T-GW-07, контракт плана): после обрыва супервайзер решает
//! [`plan_reconnect`]: разрыв меньше [`RESUME_MAX_DOWNTIME_MS`] и сессия известна и сервер не
//! запретил resume (`op9 d=false`) → [`ReconnectAction::Resume`], иначе — свежий IDENTIFY.
//! Клиент сам не переподключается: цикл транспорта возвращает исход, решение — снаружи.

use chord_codec::etf::{self, EtfError, EtfValue};

use crate::cache::InMemoryCache;
use crate::state::{Event, EventPayload, State, apply};

/// Discord Gateway Opcode 6: `RESUME` (`{op: 6, d: {token, session_id, seq}}`).
pub const RESUME_OP: u8 = 6;

/// Минимальная (базовая) задержка экспоненциального backoff при реконнекте: **1 000 мс (1 с)**.
pub const RESUME_BACKOFF_MIN_MS: u64 = 1_000;

/// Максимальная задержка (потолок) экспоненциального backoff при реконнекте: **60 000 мс (60 с)**.
pub const RESUME_BACKOFF_MAX_MS: u64 = 60_000;

/// Порог подряд идущих неудачных попыток RESUME (`gatewayBackoff.fails > 4`), после которого
/// сессия сбрасывается (`session_id`/`seq` очищаются) и выполняется свежий `IDENTIFY` (Opcode 2).
pub const RESUME_RESET_AFTER_FAILURES: u32 = 4;

/// Синоним [`RESUME_RESET_AFTER_FAILURES`].
pub const RESUME_MAX_FAILURE_ATTEMPTS: u32 = RESUME_RESET_AFTER_FAILURES;

/// Порог удержания сессии: разрыв `>= 30 000 мс` — новый IDENTIFY (T-GW-07: 3 с → resume,
/// 30 с → IDENTIFY). Сервер может переопределить решение через `op9 d=false`.
pub const RESUME_MAX_DOWNTIME_MS: u64 = 30_000;

/// Параметры RESUME (op6): `{token, session_id, seq}`.
#[derive(Clone, PartialEq, Eq)]
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

/// Ошибки сборки и восстановления RESUME: ни одна не является паникой.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResumeError {
    /// Пустой (или состоящий из пробелов) токен.
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
            Self::Encode(error) => {
                write!(f, "собранный RESUME не разбирается ETF-кодеком: {error}")
            }
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

/// Экспоненциальный backoff для восстановления сессии (`1s .. 60s`: `1000 -> 2000 -> 4000 -> 8000 -> 16000 -> 32000 -> 60000`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeExponentialBackoff {
    base_ms: u64,
    max_ms: u64,
    next_ms: u64,
    attempts: u32,
}

impl ResumeExponentialBackoff {
    /// Создаёт экспоненциальный backoff с нижней границей `base_ms` и потолком `max_ms`.
    #[must_use]
    pub fn new(base_ms: u64, max_ms: u64) -> Self {
        let base_ms = base_ms.max(1);
        let max_ms = max_ms.max(base_ms);
        Self {
            base_ms,
            max_ms,
            next_ms: base_ms,
            attempts: 0,
        }
    }

    /// Базовая (начальная) задержка в миллисекундах (по умолчанию `1_000` мс).
    #[must_use]
    pub const fn base_ms(&self) -> u64 {
        self.base_ms
    }

    /// Максимальная задержка (потолок) в миллисекундах (по умолчанию `60_000` мс).
    #[must_use]
    pub const fn max_ms(&self) -> u64 {
        self.max_ms
    }

    /// Текущая задержка, которая будет возвращена при следующем вызове [`Self::next_delay_ms`].
    #[must_use]
    pub const fn current_ms(&self) -> u64 {
        self.next_ms
    }

    /// Количество последовательных шагов backoff с момента последнего сброса.
    #[must_use]
    pub const fn attempt(&self) -> u32 {
        self.attempts
    }

    /// Синоним [`Self::attempt`].
    #[must_use]
    pub const fn attempts(&self) -> u32 {
        self.attempts
    }

    /// Детерминированно вычисляет задержку (мс) для указанного индекса попытки `attempt_idx` (`0 -> 1s`, `1 -> 2s`, ..., `>= 6 -> 60s`).
    #[must_use]
    pub fn delay_for_attempt(&self, attempt_idx: u32) -> u64 {
        let mut delay = self.base_ms;
        for _ in 0..attempt_idx {
            delay = delay.saturating_mul(2).min(self.max_ms);
        }
        delay
    }

    /// Возвращает задержку для текущей попытки и удваивает следующую задержку до потолка `max_ms`.
    pub fn next_delay_ms(&mut self) -> u64 {
        let delay = self.next_ms;
        self.next_ms = self.next_ms.saturating_mul(2).min(self.max_ms);
        self.attempts = self.attempts.saturating_add(1);
        delay
    }

    /// Сбрасывает backoff к начальной задержке `base_ms` (1 с) после успешного `RESUMED`/`READY`.
    pub fn reset(&mut self) {
        self.next_ms = self.base_ms;
        self.attempts = 0;
    }
}

impl Default for ResumeExponentialBackoff {
    fn default() -> Self {
        Self::new(RESUME_BACKOFF_MIN_MS, RESUME_BACKOFF_MAX_MS)
    }
}

/// Сформированный кадр рукопожатия `RESUME` (Opcode 6) в проводном формате ETF и JSON.
#[derive(Clone, PartialEq, Eq)]
pub struct ResumeHandshakeFrame {
    /// Оп-код шлюза (`6`).
    pub op: u8,
    /// Идентификатор восстанавливаемой сессии (`session_id`).
    pub session_id: String,
    /// Последний подтверждённый клиентом порядковый номер события (`seq`).
    pub seq: u64,
    /// Текущая задержка экспоненциального backoff (мс) перед отправкой `RESUME`.
    pub backoff_delay_ms: u64,
    /// Номер текущей попытки `RESUME` (`1..=4`).
    pub attempt: u32,
    /// Гарантия сохранения кэша гильдий без сброса (`true`).
    pub preserve_guild_cache: bool,
    /// Гарантия отсутствия повторной загрузки каналов (`true`).
    pub skip_channel_reload: bool,
    /// Нормализованный `resume_gateway_url` (без хвостового `/`), если был получен в `READY`.
    pub resume_gateway_url: Option<String>,
    /// Проводной ETF-кадр с ведущим маркером `0x83`.
    pub etf_frame: Vec<u8>,
    /// JSON-представление кадра `{ "op": 6, "d": { "token": ..., "session_id": ..., "seq": ... } }`.
    pub json_payload: String,
}

impl std::fmt::Debug for ResumeHandshakeFrame {
    /// Токен никогда не выводится в `Debug` (заменяется на `"REDACTED"`).
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ResumeHandshakeFrame")
            .field("op", &self.op)
            .field("token", &"REDACTED")
            .field("session_id", &self.session_id)
            .field("seq", &self.seq)
            .field("backoff_delay_ms", &self.backoff_delay_ms)
            .field("attempt", &self.attempt)
            .field("preserve_guild_cache", &self.preserve_guild_cache)
            .field("skip_channel_reload", &self.skip_channel_reload)
            .field("resume_gateway_url", &self.resume_gateway_url)
            .field("etf_frame_len", &self.etf_frame.len())
            .finish()
    }
}

/// Результат успешного восстановления сессии через `RESUME` (Opcode 6) без сброса кэша гильдий и повторной загрузки каналов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeRecoveryOutcome {
    /// Идентификатор восстановленной сессии (`session_id`).
    pub session_id: String,
    /// Порядковый номер `s`, на котором получен `RESUMED`.
    pub resumed_at_seq: u64,
    /// Количество догнанных (воспроизведённых сервером) событий с момента разрыва.
    pub replayed_events: u64,
    /// Количество сохранённых гильдий в кэше.
    pub preserved_guilds: usize,
    /// Количество сохранённых каналов в кэше.
    pub preserved_channels: usize,
    /// Флаг: сбрасывался ли кэш гильдий (обязан быть `false`).
    pub guild_cache_reset: bool,
    /// Флаг: выполнялась ли повторная загрузка каналов (обязан быть `false`).
    pub channels_reloaded: bool,
}

/// Отчёт о восстановлении сессии через `RESUME` (Opcode 6) со слиянием догнанных событий в [`State`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeRecoveryReport {
    /// Идентификатор восстановленной сессии (`session_id`).
    pub session_id: String,
    /// Значение `seq`, отправленное клиентом в запросе `RESUME` (op6).
    pub resume_seq_sent: u64,
    /// Порядковый номер `s` после применения `RESUMED` и догнанных событий.
    pub last_seq_after_resume: u64,
    /// Число событий, воспроизведённых шлюзом и применённых после `RESUMED`.
    pub replayed_events_applied: usize,
    /// Гарантия сохранения кэша гильдий без сброса (`true`).
    pub guild_cache_preserved: bool,
    /// Гарантия отсутствия повторной загрузки каналов (`false`).
    pub channels_reloaded: bool,
    /// Число сохранённых гильдий в кэше/состоянии.
    pub preserved_guild_count: usize,
    /// Число сохранённых каналов в кэше/состоянии.
    pub preserved_channel_count: usize,
}

/// Менеджер восстановления сессии `RESUME` (Opcode 6) по `session_id` и `seq`
/// с экспоненциальным backoff (`1s..60s`) и защитой кэша гильдий/каналов от сброса (`STAGE-05.3.2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResumeSessionManager {
    state: Option<ResumeState>,
    resume_gateway_url: Option<String>,
    backoff: ResumeExponentialBackoff,
    consecutive_failures: u32,
    invalid_session: Option<bool>,
    cached_guild_count: usize,
    cached_channel_count: usize,
    seq_at_resume_request: Option<u64>,
    total_resumed_count: u64,
}

/// Алиас для единообразия API.
pub type ResumeSessionCoordinator = ResumeSessionManager;

impl Default for ResumeSessionManager {
    fn default() -> Self {
        Self::new()
    }
}

impl ResumeSessionManager {
    /// Создаёт новый менеджер с экспоненциальным backoff `1s..60s`.
    #[must_use]
    pub fn new() -> Self {
        Self {
            state: None,
            resume_gateway_url: None,
            backoff: ResumeExponentialBackoff::default(),
            consecutive_failures: 0,
            invalid_session: None,
            cached_guild_count: 0,
            cached_channel_count: 0,
            seq_at_resume_request: None,
            total_resumed_count: 0,
        }
    }

    /// Инициализирует менеджер из активного состояния [`State`] и опционального [`InMemoryCache`].
    pub fn from_state(state: &State, cache: Option<&InMemoryCache>) -> Result<Self, ResumeError> {
        if state.session_id.trim().is_empty() {
            return Err(ResumeError::EmptySessionId);
        }
        let (guilds, channels) = count_state_and_cache(state, cache);
        let mut manager = Self::new();
        manager.on_ready(
            &state.session_id,
            state.resume_gateway_url.as_deref(),
            state.last_seq.unwrap_or(0),
        );
        manager.cached_guild_count = guilds;
        manager.cached_channel_count = channels;
        Ok(manager)
    }

    /// Фиксирует установленную сессию `READY` вместе с заполненным [`InMemoryCache`] гильдий и каналов.
    pub fn on_ready_established(
        &mut self,
        session_id: impl Into<String>,
        resume_gateway_url: Option<&str>,
        initial_seq: u64,
        cache: &InMemoryCache,
    ) {
        self.on_ready(session_id, resume_gateway_url, initial_seq);
        self.cached_guild_count = cache.guilds().len();
        self.cached_channel_count = cache.channels().len();
    }

    /// Фиксирует успешный `READY`, сохраняя `session_id`, нормализованный `resume_gateway_url` и `seq`.
    pub fn on_ready(
        &mut self,
        session_id: impl Into<String>,
        resume_gateway_url: Option<&str>,
        seq: u64,
    ) {
        self.state = Some(ResumeState {
            session_id: session_id.into(),
            last_seq: seq,
        });
        self.resume_gateway_url =
            resume_gateway_url.map(|url| url.trim_end_matches('/').to_string());
        self.invalid_session = None;
        self.consecutive_failures = 0;
        self.seq_at_resume_request = None;
        self.backoff.reset();
    }

    /// Проверяет, есть ли активная сессия (`session_id` + `seq`), пригодная для `RESUME` (Opcode 6).
    #[must_use]
    pub fn has_resumable_session(&self) -> bool {
        self.state.is_some()
            && self.invalid_session != Some(false)
            && self.consecutive_failures < RESUME_RESET_AFTER_FAILURES
    }

    /// Обновляет последний полученный порядковый номер `seq` (`s` из OP 0 DISPATCH).
    /// Возвращает `true`, если `seq` строго больше предыдущего (`s` монотонно растёт).
    pub fn record_dispatch_seq(&mut self, seq: u64) -> bool {
        if let Some(state) = &mut self.state
            && seq > state.last_seq
        {
            state.last_seq = seq;
            return true;
        }
        false
    }

    /// Обновляет `seq` (совместимая обёртка над [`Self::record_dispatch_seq`]).
    pub fn record_seq(&mut self, seq: u64) {
        let _ = self.record_dispatch_seq(seq);
    }

    /// Текущий `session_id`, если сессия активна и пригодна для `RESUME`.
    #[must_use]
    pub fn session_id(&self) -> Option<&str> {
        self.state.as_ref().map(|s| s.session_id.as_str())
    }

    /// Последний зафиксированный `seq` (`s`), который будет отправлен в `RESUME` (Opcode 6).
    #[must_use]
    pub fn last_seq(&self) -> u64 {
        self.state.as_ref().map_or(0, |s| s.last_seq)
    }

    /// Текущее состояние [`ResumeState`].
    #[must_use]
    pub const fn resume_state(&self) -> Option<&ResumeState> {
        self.state.as_ref()
    }

    /// Ссылка на состояние экспоненциального backoff (`1s..60s`).
    #[must_use]
    pub const fn backoff(&self) -> &ResumeExponentialBackoff {
        &self.backoff
    }

    /// Число подряд идущих неудачных попыток `RESUME`.
    #[must_use]
    pub const fn consecutive_failures(&self) -> u32 {
        self.consecutive_failures
    }

    /// Общее количество успешных восстановлений сессии (`RESUMED`).
    #[must_use]
    pub const fn total_resumed_count(&self) -> u64 {
        self.total_resumed_count
    }

    /// Число гильдий, удерживаемых в кэше без сброса.
    #[must_use]
    pub const fn cached_guild_count(&self) -> usize {
        self.cached_guild_count
    }

    /// Число каналов, удерживаемых в кэше без повторной загрузки.
    #[must_use]
    pub const fn cached_channel_count(&self) -> usize {
        self.cached_channel_count
    }

    /// Вычисляет следующую задержку экспоненциального backoff (`1000..=60000` мс) при обрыве соединения.
    pub fn on_disconnect_next_backoff_ms(&mut self) -> u64 {
        self.backoff.next_delay_ms()
    }

    /// Определяет действие при реконнекте (`Resume` vs `Identify`) с учётом времени обрыва и флага `op9`.
    #[must_use]
    pub fn plan_action(&self, downtime_ms: u64) -> ReconnectAction {
        if self.consecutive_failures >= RESUME_RESET_AFTER_FAILURES {
            return ReconnectAction::Identify;
        }
        plan_reconnect(self.state.as_ref(), downtime_ms, self.invalid_session)
    }

    /// Синоним [`Self::plan_action`].
    #[must_use]
    pub fn plan_next_action(&self, downtime_ms: u64) -> ReconnectAction {
        self.plan_action(downtime_ms)
    }

    /// Обрабатывает неудачную попытку `RESUME`: после [`RESUME_RESET_AFTER_FAILURES`] (4) неудач
    /// сбрасывает `session_id`/`seq` и переключает следующую попытку на `IDENTIFY` (Opcode 2).
    pub fn on_resume_attempt_failed(&mut self) -> ReconnectAction {
        self.consecutive_failures = self.consecutive_failures.saturating_add(1);
        if self.consecutive_failures >= RESUME_RESET_AFTER_FAILURES {
            self.state = None;
            self.invalid_session = None;
            self.consecutive_failures = 0;
            self.seq_at_resume_request = None;
            return ReconnectAction::Identify;
        }
        ReconnectAction::Resume
    }

    /// Обрабатывает кадр `OP 9 INVALID_SESSION (d: resumable)`:
    /// при `resumable == true` сохраняет сессию и кэш для `RESUME`,
    /// при `resumable == false` очищает `session_id`/`seq` и возвращает [`ReconnectAction::Identify`].
    pub fn on_invalid_session(&mut self, resumable: bool) -> ReconnectAction {
        self.invalid_session = Some(resumable);
        if !resumable {
            self.state = None;
            self.consecutive_failures = 0;
            self.seq_at_resume_request = None;
            return ReconnectAction::Identify;
        }
        if self.state.is_some() {
            ReconnectAction::Resume
        } else {
            ReconnectAction::Identify
        }
    }

    /// Подготавливает кадр `RESUME` (Opcode 6) с очередным шагом экспоненциального backoff (`1s..60s`),
    /// сохраняя кэш гильдий и пропуская повторную загрузку каналов.
    pub fn prepare_resume_frame(
        &mut self,
        token: &str,
        cache: &InMemoryCache,
    ) -> Result<ResumeHandshakeFrame, ResumeError> {
        if token.trim().is_empty() {
            return Err(ResumeError::EmptyToken);
        }
        let Some(resume_state) = self.state.clone() else {
            return Err(ResumeError::EmptySessionId);
        };
        let params = ResumeParams {
            token: token.to_string(),
            session_id: resume_state.session_id.clone(),
            seq: resume_state.last_seq,
        };
        let etf_frame = build_resume_frame(&params)?;
        let json_payload = build_resume_json_payload(&params)?;
        let backoff_delay_ms = self.backoff.next_delay_ms();
        let attempt = self.backoff.attempt();
        self.seq_at_resume_request = Some(resume_state.last_seq);
        self.cached_guild_count = cache.guilds().len();
        self.cached_channel_count = cache.channels().len();
        Ok(ResumeHandshakeFrame {
            op: RESUME_OP,
            session_id: resume_state.session_id,
            seq: resume_state.last_seq,
            backoff_delay_ms,
            attempt,
            preserve_guild_cache: true,
            skip_channel_reload: true,
            resume_gateway_url: self.resume_gateway_url.clone(),
            etf_frame,
            json_payload,
        })
    }

    /// Формирует кадр `RESUME` (Opcode 6) без продвижения счётчика backoff (read-only).
    pub fn build_resume_handshake(
        &self,
        token: impl Into<String>,
    ) -> Result<ResumeHandshakeFrame, ResumeError> {
        let token = token.into();
        if token.trim().is_empty() {
            return Err(ResumeError::EmptyToken);
        }
        let Some(resume_state) = &self.state else {
            return Err(ResumeError::EmptySessionId);
        };
        let params = ResumeParams {
            token,
            session_id: resume_state.session_id.clone(),
            seq: resume_state.last_seq,
        };
        let etf_frame = build_resume_frame(&params)?;
        let json_payload = build_resume_json_payload(&params)?;
        Ok(ResumeHandshakeFrame {
            op: RESUME_OP,
            session_id: resume_state.session_id.clone(),
            seq: resume_state.last_seq,
            backoff_delay_ms: self.backoff.current_ms(),
            attempt: self.backoff.attempt(),
            preserve_guild_cache: true,
            skip_channel_reload: true,
            resume_gateway_url: self.resume_gateway_url.clone(),
            etf_frame,
            json_payload,
        })
    }

    /// Обрабатывает получение `RESUMED` от шлюза: сбрасывает экспоненциальный backoff к `1 000` мс
    /// и подтверждает сохранение кэша гильдий и отсутствие повторной загрузки каналов.
    pub fn on_resumed_received(
        &mut self,
        resumed_seq: u64,
        cache: &InMemoryCache,
    ) -> ResumeRecoveryOutcome {
        let start_seq = self
            .seq_at_resume_request
            .unwrap_or_else(|| self.last_seq());
        let _ = self.record_dispatch_seq(resumed_seq);
        let final_seq = self.last_seq().max(resumed_seq);
        let replayed_events = final_seq.saturating_sub(start_seq);
        let session_id = self
            .state
            .as_ref()
            .map(|s| s.session_id.clone())
            .unwrap_or_default();

        self.invalid_session = None;
        self.consecutive_failures = 0;
        self.seq_at_resume_request = None;
        self.backoff.reset();
        self.total_resumed_count = self.total_resumed_count.saturating_add(1);
        self.cached_guild_count = cache.guilds().len();
        self.cached_channel_count = cache.channels().len();

        ResumeRecoveryOutcome {
            session_id,
            resumed_at_seq: final_seq,
            replayed_events,
            preserved_guilds: self.cached_guild_count,
            preserved_channels: self.cached_channel_count,
            guild_cache_reset: false,
            channels_reloaded: false,
        }
    }

    /// Завершает восстановление сессии при получении `RESUMED` и очереди догнанных событий в [`State`],
    /// сохраняя существующий кэш гильдий без сброса и без повторной загрузки каналов.
    pub fn complete_resume_without_cache_reset(
        &mut self,
        state: &mut State,
        cache: Option<&InMemoryCache>,
        resumed_seq: u64,
        replayed_events: Vec<Event>,
    ) -> Result<ResumeRecoveryReport, ResumeError> {
        let Some(current_resume) = self.state.clone() else {
            return Err(ResumeError::EmptySessionId);
        };
        let (guilds_before, _) = count_state_and_cache(state, cache);
        let applied_count = apply_resumed_and_replay(state, resumed_seq, replayed_events);
        let final_seq = state.last_seq.unwrap_or(resumed_seq);
        let (guilds_after, channels_after) = count_state_and_cache(state, cache);

        self.state = Some(ResumeState {
            session_id: current_resume.session_id.clone(),
            last_seq: final_seq,
        });
        self.invalid_session = None;
        self.consecutive_failures = 0;
        self.seq_at_resume_request = None;
        self.backoff.reset();
        self.total_resumed_count = self.total_resumed_count.saturating_add(1);
        self.cached_guild_count = guilds_after;
        self.cached_channel_count = channels_after;

        Ok(ResumeRecoveryReport {
            session_id: current_resume.session_id,
            resume_seq_sent: current_resume.last_seq,
            last_seq_after_resume: final_seq,
            replayed_events_applied: applied_count,
            guild_cache_preserved: guilds_after >= guilds_before,
            channels_reloaded: false,
            preserved_guild_count: guilds_after,
            preserved_channel_count: channels_after,
        })
    }
}

/// Подсчитывает количество гильдий и каналов в [`State`] и [`InMemoryCache`].
fn count_state_and_cache(state: &State, cache: Option<&InMemoryCache>) -> (usize, usize) {
    let state_channels = state
        .guilds
        .values()
        .map(|g| g.channels.len())
        .sum::<usize>()
        .saturating_add(state.private_channels.len());
    let guilds = cache
        .map(|c| c.guilds().len().max(state.guilds.len()))
        .unwrap_or(state.guilds.len());
    let channels = cache
        .map(|c| c.channels().len().max(state_channels))
        .unwrap_or(state_channels);
    (guilds, channels)
}

/// Применяет событие `RESUMED` и догнанные события к [`State`] без очистки гильдий и каналов.
fn apply_resumed_and_replay(
    state: &mut State,
    resumed_seq: u64,
    replayed_events: Vec<Event>,
) -> usize {
    let taken = state.clone();
    let (mut next_state, _) = apply(
        taken,
        Event {
            seq: Some(resumed_seq),
            payload: EventPayload::Resumed,
            diagnostics: Default::default(),
        },
    );
    let mut applied_count = 0_usize;
    for event in replayed_events {
        let is_stale = event
            .seq
            .zip(next_state.last_seq)
            .is_some_and(|(s, last)| s <= last);
        let (updated, _) = apply(next_state, event);
        next_state = updated;
        if !is_stale {
            applied_count = applied_count.saturating_add(1);
        }
    }
    *state = next_state;
    applied_count
}

/// Ключ map в форме клиентского wetf-энкодера (`key:"binary"`).
fn key(name: &str) -> EtfValue {
    EtfValue::Str(name.to_string())
}

/// `u64` → `i64` с насыщением (значения протокола не превышают `i64::MAX`).
fn int(value: u64) -> EtfValue {
    EtfValue::Int(i64::try_from(value).unwrap_or(i64::MAX))
}

/// Проверяет непустоту `token` и `session_id` перед сборкой `RESUME` (op6).
fn validate_resume_params(params: &ResumeParams) -> Result<(), ResumeError> {
    if params.token.trim().is_empty() {
        return Err(ResumeError::EmptyToken);
    }
    if params.session_id.trim().is_empty() {
        return Err(ResumeError::EmptySessionId);
    }
    Ok(())
}

/// Собирает проводную форму RESUME (op6) с ведущим `0x83`.
///
/// Самопроверка: собранные байты обязаны полностью разбираться тем же кодеком; иначе —
/// [`ResumeError::Encode`]. Пустые `token`/`session_id` — ошибки (невалидный RESUME не уйдёт).
pub fn build_resume_frame(params: &ResumeParams) -> Result<Vec<u8>, ResumeError> {
    validate_resume_params(params)?;
    let frame = EtfValue::Map(vec![
        (key("op"), EtfValue::Int(i64::from(RESUME_OP))),
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

/// Собирает JSON-представление кадра `RESUME` (Opcode 6):
/// `{"op":6,"d":{"token":"...","session_id":"...","seq":...}}`.
pub fn build_resume_json_payload(params: &ResumeParams) -> Result<String, ResumeError> {
    validate_resume_params(params)?;
    let payload = serde_json::json!({
        "op": RESUME_OP,
        "d": {
            "token": params.token,
            "session_id": params.session_id,
            "seq": params.seq,
        }
    });
    Ok(payload.to_string())
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
