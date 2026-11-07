//! # Супервайзер шлюза: одно соединение → серия (RW-39, NET-E1; CORE-AUDIT P0-3)
//!
//! `connect_and_run*` обслуживает **одно** соединение; этот модуль — политика вокруг него:
//! connect → HELLO → IDENTIFY/RESUME → dispatch/heartbeat → close, затем решение
//! «остановиться / переподключиться / резюмировать». Источники поведения:
//! - `AUTH-RE.md` G-16…G-18: `resume_gateway_url` (хвостовой `/` обрезан), close 4004 —
//!   logout без ретрая; op7 — новый коннект; op9 `d=true` → RESUME, `d=false` → IDENTIFY;
//! - `ADR-2-auth.md`: 4013/4014 прекращают reconnect; heartbeat — op40 с fallback op1;
//! - `F-GW-08` (`RE-GAP-BACKEND-PROTOCOL.md` §3.7, бандл `_doResumeOrIdentify` @5 019 509):
//!   сессия пригодна для `RESUME` в течение [`RESUME_TTL_MS`] (180 000 мс = 3 минуты) с момента
//!   последнего полученного `HEARTBEAT_ACK` (op11) / успешного события; при `elapsed > 180 000` мс
//!   супервайзер сразу сбрасывает `session_id`/`seq` и выполняет свежий `IDENTIFY` (op2);
//! - `F-GW-02` / `BE-P1-01`: методы формирования и постановки в очередь исходящих клиентских
//!   кадров OP 3 ([`PresenceUpdate`]), OP 4 ([`VoiceStateUpdateOut`]), OP 8
//!   ([`RequestGuildMembers`]) и OP 37 ([`GuildSubscriptionsBulk`]) в активной сессии (`Ready`).
//!
//! Backoff — как `gatewayBackoff` клиента (`new Backoff(1e3, 6e4)`): 1 с, ×2, потолок 60 с,
//! сброс после READY/RESUMED. После [`RESUME_RESET_AFTER_FAILURES`] подряд неудачных RESUME сессия
//! сбрасывается (`session_id`/`seq` обнулены, следующая попытка — IDENTIFY), как клиентский
//! `_reset` при `gatewayBackoff.fails > 4`: серия реконнектов не прекращается (F-GW-07).
//! `max_reconnects` — только для тестов/явного product-decision: `0` (боевой дефолт) — без
//! лимита, ненулевое значение — `Stop(ReconnectLimit)`. Останавливают серию лишь фатальные
//! close-коды (4004/4013/4014). Отмена — внешний future в [`Supervisor::run_until`]:
//! супервайзер не держит глобального состояния и готов к остановке на любом шаге.

use std::collections::{BTreeMap, VecDeque};
use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::dispatch::DispatchKind;
pub use crate::gateway::{
    GUILD_MEMBERS_CHUNK_SIZE, GUILD_SUBSCRIPTIONS_BULK_OP, OutboundError,
    OutboundGuildMembersRequest, OutboundGuildSubscription, OutboundPresenceUpdate,
    OutboundVoiceStateUpdate, PRESENCE_UPDATE_OP, REQUEST_GUILD_MEMBERS_OP, VOICE_STATE_UPDATE_OP,
    chunk_user_ids, encode_guild_subscriptions, encode_presence_update,
    encode_request_guild_members, encode_voice_state_update,
};
use crate::gateway::{HeartbeatMode, Session};
use crate::identify::{
    CAPABILITIES_ACTIVE, IdentifyParams, IdentifyProperties, PresenceInfo,
    qos_token_client_provided,
};
use crate::rest::super_properties::SuperProperties;
use crate::resume::{ReconnectAction, ResumeParams, ResumeState, plan_reconnect};
use crate::ws::{
    DispatchHandler, GatewayPipe, GatewayUrl, RunOptions, RunOutcome, connect_and_run_resume,
};

/// Алиас супервайзера шлюза для единообразия внешнего API (`GatewaySupervisor`).
pub type GatewaySupervisor = Supervisor;

/// OP 3 `PRESENCE_UPDATE`: `{status, since, activities, afk}`.
pub type PresenceUpdate = OutboundPresenceUpdate;

/// OP 4 `VOICE_STATE_UPDATE`: `{guild_id, channel_id, self_mute, self_deaf, self_video, flags}`.
pub type VoiceStateUpdateOut = OutboundVoiceStateUpdate;

/// OP 8 `REQUEST_GUILD_MEMBERS`: `{guild_id, query, limit, user_ids, presences, nonce}`.
pub type RequestGuildMembers = OutboundGuildMembersRequest;

/// OP 37 `GUILD_SUBSCRIPTIONS_BULK`: карта `guild_id -> OutboundGuildSubscription`.
pub type GuildSubscriptionsBulk = BTreeMap<String, OutboundGuildSubscription>;

/// Сколько подряд неудачных RESUME терпит супервайзер до сброса сессии.
///
/// Клиент (`_handleClose`, бандл @5 014 326) при `gatewayBackoff.fails > 4` вызывает
/// `_reset`: `sessionId`/`seq` обнуляются, следующая попытка — IDENTIFY. Четыре попытки —
/// порог, после которого 5-я идёт с op2, а не с op6 (F-GW-07, BE-P2-06б).
pub const RESUME_RESET_AFTER_FAILURES: u32 = 4;

/// Максимальное время жизни сессии для `RESUME` с момента последнего `HEARTBEAT_ACK` (op11)
/// или успешного события сессии: 3 минуты (`180 000` мс).
///
/// Оракул (`RE-GAP-BACKEND-PROTOCOL.md` §3.7 F-GW-08, бандл @5 019 509 `_doResumeOrIdentify`):
/// `null !== this.sessionId && (null == this.lastHeartbeatAckTime || e - this.lastHeartbeatAckTime <= tI)`,
/// где `tI = 3 * L.A.Millis.MINUTE` (`180 000` мс). Если прошло `> 180 000` мс, вызывается
/// `_doIdentify()`, который сразу сбрасывает `this.seq = 0; this.sessionId = null`.
pub const RESUME_TTL_MS: u64 = 180_000;

/// Синоним [`RESUME_TTL_MS`] для явной привязки к времени от последнего `HEARTBEAT_ACK`.
pub const RESUME_MAX_ACK_ELAPSED_MS: u64 = RESUME_TTL_MS;

/// Ошибка формирования или постановки в очередь исходящего клиентского кадра (OP 3/4/8/37).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorOutboundError {
    /// Рукопожатие (READY/RESUMED) ещё не завершено либо сессия сброшена/не подключена.
    NotReady {
        /// Оп-код кадра (3, 4, 8 или 37), отклонённого до установления сессии.
        op: i64,
    },
    /// Ошибка ETF-кодирования полезной нагрузки кадра.
    Encode(OutboundError),
}

impl fmt::Display for SupervisorOutboundError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotReady { op } => write!(
                formatter,
                "сессия шлюза не в состоянии Ready/Connected: отправка op={op} отклонена"
            ),
            Self::Encode(error) => write!(formatter, "{error}"),
        }
    }
}

impl std::error::Error for SupervisorOutboundError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Encode(error) => Some(error),
            Self::NotReady { .. } => None,
        }
    }
}

impl From<OutboundError> for SupervisorOutboundError {
    fn from(error: OutboundError) -> Self {
        Self::Encode(error)
    }
}

/// Детерминированный backoff: база, ×2, потолок; сброс после успешной сессии.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backoff {
    /// Первая задержка (клиент: 1000 мс).
    base_ms: u64,
    /// Потолок (клиент: 60 000 мс).
    max_ms: u64,
    /// Задержка, которую вернёт следующий [`Self::next_delay_ms`].
    next_ms: u64,
}

impl Backoff {
    /// `base_ms`/`max_ms` clamp-ятся (ноль/инверсия не ломают арифметику).
    pub fn new(base_ms: u64, max_ms: u64) -> Self {
        let base_ms = base_ms.max(1);
        let max_ms = max_ms.max(base_ms);
        Self {
            base_ms,
            max_ms,
            next_ms: base_ms,
        }
    }

    /// Следующая задержка; внутреннее состояние удваивается до потолка.
    pub fn next_delay_ms(&mut self) -> u64 {
        let delay = self.next_ms;
        self.next_ms = self.next_ms.saturating_mul(2).min(self.max_ms);
        delay
    }

    /// Сброс после READY/RESUMED (как `gatewayBackoff.succeed()` клиента).
    pub fn reset(&mut self) {
        self.next_ms = self.base_ms;
    }

    /// Задержка, которая будет возвращена следующей (диагностика/тесты).
    pub fn current_ms(&self) -> u64 {
        self.next_ms
    }
}

impl Default for Backoff {
    fn default() -> Self {
        Self::new(1_000, 60_000)
    }
}

/// Конфигурация супервайзера.
#[derive(Clone)]
pub struct SupervisorConfig {
    /// Базовый адрес (или уже нормализованный resume-адрес).
    pub gateway: GatewayUrl,
    /// Параметры IDENTIFY (токен, свойства, presence, qos_token).
    pub identify: IdentifyParams,
    /// Предел реконнектов; `0` — без лимита (боевой дефолт, F-GW-07).
    ///
    /// Ненулевое значение — конечный лимит для тестов/явного product-decision:
    /// после него `Stop(ReconnectLimit)`. Успешный READY/RESUMED сбрасывает счётчик.
    pub max_reconnects: u32,
    /// Backoff между попытками.
    pub backoff: Backoff,
    /// Сид jitter первого heartbeat (детерминизм тестов/реплея).
    pub jitter_seed: u64,
    /// Необязательные события жизненного цикла.
    pub on_event: Option<Arc<dyn Fn(SupervisorEvent) + Send + Sync>>,
    /// Необязательный обработчик op0 (L-02): пробрасывается в `RunOptions` каждой попытки.
    pub on_dispatch: Option<DispatchHandler>,
}

impl fmt::Debug for SupervisorConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SupervisorConfig")
            .field("gateway", &self.gateway)
            .field("identify", &self.identify)
            .field("max_reconnects", &self.max_reconnects)
            .field("backoff", &self.backoff)
            .field("jitter_seed", &self.jitter_seed)
            .field("on_event", &self.on_event.as_ref().map(|_| "<handler>"))
            .field(
                "on_dispatch",
                &self.on_dispatch.as_ref().map(|_| "<handler>"),
            )
            .finish()
    }
}

impl SupervisorConfig {
    /// Боевые дефолты по RE: capabilities 1734653, `compress:false`, online-presence,
    /// `qos_token` client-provided (G-04…G-11).
    pub fn new(gateway: GatewayUrl, token: impl Into<String>) -> Self {
        Self {
            gateway,
            identify: IdentifyParams {
                token: token.into(),
                capabilities: CAPABILITIES_ACTIVE,
                properties: IdentifyProperties::for_rw_client(),
                presence: Some(PresenceInfo {
                    status: "online".to_string(),
                    since: 0,
                    activities: Vec::new(),
                    afk: false,
                }),
                compress: false,
                client_state_guild_versions: Vec::new(),
                qos_token: qos_token_client_provided(false),
            },
            max_reconnects: 0,
            backoff: Backoff::default(),
            jitter_seed: 0,
            on_event: None,
            on_dispatch: None,
        }
    }

    /// IDENTIFY из тех же супер-свойств, что уходят в `X-Super-Properties` (G-06).
    ///
    /// Пересекающиеся 14 полей берутся из `properties`; поля идентификации, которых в
    /// заголовке нет (`is_fast_connect`, `gateway_connect_reasons`, `installation_id`),
    /// остаются дефолтами клиента.
    pub fn with_super_properties(
        gateway: GatewayUrl,
        token: impl Into<String>,
        properties: &SuperProperties,
    ) -> Self {
        let mut config = Self::new(gateway, token);
        config.identify.properties = IdentifyProperties::from_super_properties(properties);
        config
    }

    /// Установить обработчик событий жизненного цикла.
    pub fn with_event_handler(
        mut self,
        handler: impl Fn(SupervisorEvent) + Send + Sync + 'static,
    ) -> Self {
        self.on_event = Some(Arc::new(handler));
        self
    }

    /// Установить обработчик dispatch-кадров (L-02): все op0, что разобрал пайп.
    ///
    /// Обработчик не должен блокировать: он вызывается в цикле соединения.
    pub fn with_dispatch_handler(
        mut self,
        handler: impl Fn(&crate::dispatch::DispatchEvent) + Send + Sync + 'static,
    ) -> Self {
        self.on_dispatch = Some(Arc::new(handler));
        self
    }
}

/// Событие жизненного цикла соединения.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorEvent {
    /// Получен READY от шлюза.
    Ready,
    /// Перед новой попыткой подключения.
    Reconnecting { delay_ms: u64 },
    /// Супервайзер завершился по политике остановки.
    Stopped(StopReason),
}

/// Почему супервайзер остановился (без паники и без бесконечного reconnect).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StopReason {
    /// close 4004: аутентификация недействительна → logout (G-18).
    Unauthorized,
    /// close 4013/4014: неверные/запрещённые intents — reconnect бессмысленен (ADR-2).
    InvalidClient {
        /// Код закрытия (4013 либо 4014).
        code: u16,
    },
    /// Исчерпан конечный лимит реконнектов (`max_reconnects > 0`; `0` — без лимита).
    ReconnectLimit {
        /// Сколько реконнектов уже сделано.
        reconnects: u32,
    },
}

/// Исход работы супервайзера.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SupervisorOutcome {
    /// Остановка по политике (logout/лимит).
    Stop(StopReason),
    /// Внешняя отмена (`run_until`).
    Cancelled,
}

/// Что делать с закрытым соединением.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CloseAction {
    /// Прекратить работу.
    Stop(StopReason),
    /// Переподключиться через backoff.
    Reconnect,
}

/// Политика close-кодов: 4004/4013/4014 — Stop, остальные (включая обрыв без кадра) — Reconnect.
pub fn classify_close(code: Option<u16>) -> CloseAction {
    match code {
        Some(4004) => CloseAction::Stop(StopReason::Unauthorized),
        Some(code @ (4013 | 4014)) => CloseAction::Stop(StopReason::InvalidClient { code }),
        _ => CloseAction::Reconnect,
    }
}

/// Супервайзер серии соединений шлюза.
pub struct Supervisor {
    config: SupervisorConfig,
    /// Состояние для RESUME (session_id + последний `s`), удерживается между соединениями.
    state: Option<ResumeState>,
    /// Активна ли установленная сессия (`READY`/`RESUMED` получен и сессия не сброшена).
    session_ready: bool,
    /// Нормализованный `resume_gateway_url` из READY (G-16); приоритетнее `config.gateway`.
    resume_url: Option<GatewayUrl>,
    /// Флаг последнего op9: `Some(false)` запрещает RESUME до следующего IDENTIFY.
    invalid_session: Option<bool>,
    /// Момент последнего полученного `HEARTBEAT_ACK` (op11) или успешного события сессии (F-GW-08).
    last_ack_at: Option<Instant>,
    /// Явное переопределение времени (мс) с последнего `HEARTBEAT_ACK` для детерминированных проверок.
    ack_elapsed_override_ms: Option<u64>,
    /// Очередь закодированных исходящих клиентских кадров (OP 3/4/8/37) активной сессии.
    outbound_queue: VecDeque<Vec<u8>>,
    /// Счётчик отклонённых попыток отправки OP 3/4/8/37 до готовности сессии (диагностика).
    rejected_outbound: u64,
    /// Сколько реконнектов сделано без успешного READY/RESUMED подряд.
    reconnects: u32,
    /// Сколько подряд неудачных RESUME с прошлого успеха/сброса (порог [`RESUME_RESET_AFTER_FAILURES`]).
    resume_failures: u32,
    /// Сколько раз сессия сбрасывалась (после серии неудачных RESUME либо по истечении [`RESUME_TTL_MS`]).
    session_resets: u32,
    /// Текущий backoff (сбрасывается на READY/RESUMED).
    backoff: Backoff,
    /// Последняя ошибка нормализации resume-URL (диагностика; не глотаем).
    last_url_error: Option<String>,
    /// Последняя ошибка соединения/кадра (диагностика; не глотаем).
    last_error: Option<String>,
}

impl Supervisor {
    /// Новый супервайзер; состояние серии пустое (первый коннект — IDENTIFY).
    pub fn new(config: SupervisorConfig) -> Self {
        Self {
            backoff: config.backoff.clone(),
            config,
            state: None,
            session_ready: false,
            resume_url: None,
            invalid_session: None,
            last_ack_at: None,
            ack_elapsed_override_ms: None,
            outbound_queue: VecDeque::new(),
            rejected_outbound: 0,
            reconnects: 0,
            resume_failures: 0,
            session_resets: 0,
            last_url_error: None,
            last_error: None,
        }
    }

    /// Работает до `Stop`; отмены нет (для фонового запуска используйте [`Self::run_until`]).
    pub async fn run(&mut self) -> SupervisorOutcome {
        self.run_until(std::future::pending::<()>()).await
    }

    /// Работает до `Stop` либо до завершения `shutdown` (отмена на любом шаге, включая backoff).
    pub async fn run_until<F>(&mut self, shutdown: F) -> SupervisorOutcome
    where
        F: Future<Output = ()>,
    {
        tokio::pin!(shutdown);
        let mut ready_emitted = false;
        loop {
            let action = self.plan_next_reconnect();
            let url = self.current_url();
            let resume = match action {
                ReconnectAction::Resume => self.state.as_ref().map(|state| ResumeParams {
                    token: self.config.identify.token.clone(),
                    session_id: state.session_id.clone(),
                    seq: state.last_seq,
                }),
                ReconnectAction::Identify => None,
            };
            let mut pipe = GatewayPipe::new(Session::with_heartbeat_mode(
                self.config.jitter_seed,
                HeartbeatMode::Qos,
            ));
            let options = RunOptions {
                identify: Some(self.config.identify.clone()),
                on_dispatch: self.config.on_dispatch.clone(),
            };
            let outcome = tokio::select! {
                biased;
                () = &mut shutdown => return SupervisorOutcome::Cancelled,
                result = connect_and_run_resume(url.as_str(), &mut pipe, options, resume) => result,
            };
            self.absorb(&pipe);
            let established = pipe.last_ready().is_some()
                || pipe.resumed()
                || pipe
                    .events()
                    .iter()
                    .any(|event| event.kind == Some(DispatchKind::Resumed));
            if !ready_emitted && pipe.last_ready().is_some() {
                ready_emitted = true;
                self.emit(SupervisorEvent::Ready);
            }

            match outcome {
                Ok(RunOutcome::Closed { code }) => {
                    self.last_error = None;
                    if let CloseAction::Stop(reason) = classify_close(code) {
                        self.session_ready = false;
                        self.emit(SupervisorEvent::Stopped(reason.clone()));
                        return SupervisorOutcome::Stop(reason);
                    }
                }
                // op7/op9/зомби: причина уже учтена в pipe (op9 d, READY/resume state).
                Ok(RunOutcome::Reconnect { .. }) => {
                    self.last_error = None;
                }
                // Ошибка соединения/кадра: повтор с backoff; текст доступен в `last_error`
                // (молчаливого проглатывания нет — ошибка наблюдается вызывающим).
                Err(error) => {
                    self.last_error = Some(error.to_string());
                }
            }

            // F-GW-07: после 4 подряд неудачных RESUME — клиентский `_reset` (свежий IDENTIFY).
            self.note_attempt(action, established);

            if self.config.max_reconnects != 0 && self.reconnects >= self.config.max_reconnects {
                let reason = StopReason::ReconnectLimit {
                    reconnects: self.reconnects,
                };
                self.emit(SupervisorEvent::Stopped(reason.clone()));
                return SupervisorOutcome::Stop(reason);
            }
            self.reconnects = self.reconnects.saturating_add(1);
            let delay = self.backoff.next_delay_ms();
            self.emit(SupervisorEvent::Reconnecting { delay_ms: delay });
            tokio::select! {
                biased;
                () = &mut shutdown => return SupervisorOutcome::Cancelled,
                () = tokio::time::sleep(Duration::from_millis(delay)) => {}
            }
        }
    }

    fn emit(&self, event: SupervisorEvent) {
        if let Some(handler) = &self.config.on_event {
            handler(event);
        }
    }

    /// Сбрасывает сессионное состояние (`session_id`/`seq` → `None`), как `_reset`/`_doIdentify` клиента.
    fn reset_session(&mut self) {
        self.state = None;
        self.session_ready = false;
        self.invalid_session = None;
        self.resume_failures = 0;
        self.outbound_queue.clear();
        self.session_resets = self.session_resets.saturating_add(1);
    }

    /// Учитывает исход попытки для политики F-GW-07 (BE-P2-06б).
    ///
    /// Считаются только подряд идущие неудачные RESUME: успешная сессия (READY или RESUMED)
    /// либо попытка со свежим IDENTIFY обнуляют счётчик. На [`RESUME_RESET_AFTER_FAILURES`]
    /// сессия сбрасывается (`session_id`/`seq` → `None`, следующая попытка — op2), backoff и
    /// счётчик реконнектов не трогаются: серия продолжается с той же задержкой.
    fn note_attempt(&mut self, action: ReconnectAction, established: bool) {
        if established || action == ReconnectAction::Identify {
            self.resume_failures = 0;
            return;
        }
        self.resume_failures = self.resume_failures.saturating_add(1);
        if self.resume_failures < RESUME_RESET_AFTER_FAILURES {
            return;
        }
        // Резюмировать мёртвую сессию бессмысленно: клиентский `_reset` (sessionId/seq = null).
        self.reset_session();
    }

    /// Выбирает `RESUME` (op6) либо `IDENTIFY` (op2) для следующего переподключения (F-GW-08).
    ///
    /// Контракт (`_doResumeOrIdentify`, бандл @5 019 509):
    /// - если сессии нет (`state == None`), возвращает [`ReconnectAction::Identify`];
    /// - если с момента последнего `HEARTBEAT_ACK` (op11) / успешного события прошло
    ///   `> RESUME_TTL_MS` (`180 000` мс), немедленно сбрасывает `session_id`/`seq` в `None`
    ///   и возвращает [`ReconnectAction::Identify`];
    /// - если `elapsed <= RESUME_TTL_MS` и сервер не запретил resume (`op9 d=false`),
    ///   сохраняет `session_id`/`seq` и возвращает [`ReconnectAction::Resume`].
    pub fn plan_next_reconnect(&mut self) -> ReconnectAction {
        if self.state.is_none() {
            self.session_ready = false;
            return ReconnectAction::Identify;
        }
        if self
            .last_ack_elapsed_ms()
            .is_some_and(|elapsed_ms| elapsed_ms > RESUME_TTL_MS)
        {
            self.reset_session();
            return ReconnectAction::Identify;
        }
        if self.invalid_session == Some(false) {
            self.session_ready = false;
            return ReconnectAction::Identify;
        }
        plan_reconnect(self.state.as_ref(), 0, self.invalid_session)
    }

    /// Адрес следующего соединения: resume-URL из READY либо база.
    fn current_url(&self) -> GatewayUrl {
        self.resume_url
            .clone()
            .unwrap_or_else(|| self.config.gateway.clone())
    }

    /// Обновляет состояние супервайзера по итогам работы [`GatewayPipe`].
    pub fn absorb_pipe(&mut self, pipe: &GatewayPipe) {
        self.absorb(pipe);
    }

    /// Забирает из соединения всё, что переживает его: session_id/seq, resume-URL, флаг op9, время ACK.
    fn absorb(&mut self, pipe: &GatewayPipe) {
        let resumed = pipe.resumed()
            || pipe
                .events()
                .iter()
                .any(|event| event.kind == Some(DispatchKind::Resumed));
        let acked = pipe.session().rtt_ms().is_some();
        if let Some(ready) = pipe.last_ready() {
            match &ready.session_id {
                Some(session_id) => {
                    self.state = Some(ResumeState {
                        session_id: session_id.clone(),
                        last_seq: pipe.session().last_seq().unwrap_or(0),
                    });
                    self.session_ready = true;
                }
                // READY без session_id — резюмировать нечего; старую сессию не тащим.
                None => {
                    self.state = None;
                    self.session_ready = false;
                }
            }
            if let Some(raw) = &ready.resume_gateway_url {
                match GatewayUrl::resume(raw) {
                    Ok(url) => {
                        self.resume_url = Some(url);
                        self.last_url_error = None;
                    }
                    Err(error) => {
                        // Ошибку не глотаем: адрес остаётся прежним, причина доступна вызывающему.
                        self.last_url_error = Some(error.to_string());
                    }
                }
            }
        }
        if resumed && self.state.is_some() {
            self.session_ready = true;
        }
        if pipe.last_ready().is_some() || resumed {
            // READY/RESUMED — сессия установлена: бюджет и backoff сбрасываются (gatewayBackoff.succeed).
            self.invalid_session = None;
            self.reconnects = 0;
            self.backoff.reset();
        }
        if pipe.last_ready().is_some()
            || resumed
            || (self.state.is_some() && (acked || pipe.session().last_seq().is_some()))
        {
            self.last_ack_at = Some(Instant::now());
        }
        if let (Some(seq), Some(state)) = (pipe.session().last_seq(), self.state.as_mut()) {
            state.last_seq = seq;
        }
        if let Some(resumable) = pipe.last_invalid_session_resumable() {
            self.invalid_session = Some(resumable);
            if !resumable {
                self.session_ready = false;
            }
        }
    }

    /// Фиксирует получение свежего `HEARTBEAT_ACK` (op11) в текущий момент времени (F-GW-08).
    pub fn note_heartbeat_ack(&mut self) {
        self.last_ack_at = Some(Instant::now());
        self.ack_elapsed_override_ms = None;
    }

    /// Задаёт время (в мс), прошедшее с момента последнего `HEARTBEAT_ACK` (для детерминированных проверок F-GW-08).
    pub fn set_last_ack_elapsed_ms(&mut self, elapsed_ms: u64) {
        self.ack_elapsed_override_ms = Some(elapsed_ms);
        self.last_ack_at = Instant::now()
            .checked_sub(Duration::from_millis(elapsed_ms))
            .or_else(|| Some(Instant::now()));
    }

    /// Сколько миллисекунд прошло с момента последнего `HEARTBEAT_ACK` (op11) / успешного события сессии.
    pub fn last_ack_elapsed_ms(&self) -> Option<u64> {
        if let Some(override_ms) = self.ack_elapsed_override_ms {
            return Some(override_ms);
        }
        self.last_ack_at
            .map(|at| u64::try_from(at.elapsed().as_millis()).unwrap_or(u64::MAX))
    }

    /// Готова ли сессия (`Ready`/`Connected`) к отправке клиентских кадров OP 3/4/8/37.
    pub fn is_session_ready(&self) -> bool {
        self.session_ready
            && self.state.is_some()
            && self.invalid_session != Some(false)
            && !self
                .last_ack_elapsed_ms()
                .is_some_and(|elapsed_ms| elapsed_ms > RESUME_TTL_MS)
    }

    fn ensure_session_ready(&mut self, op: i64) -> Result<(), SupervisorOutboundError> {
        if !self.is_session_ready() {
            self.rejected_outbound = self.rejected_outbound.saturating_add(1);
            return Err(SupervisorOutboundError::NotReady { op });
        }
        Ok(())
    }

    /// Кодирует и ставит в очередь кадр OP 3 (`PRESENCE_UPDATE`) в активной сессии.
    pub fn send_presence_update(
        &mut self,
        update: &PresenceUpdate,
    ) -> Result<Vec<u8>, SupervisorOutboundError> {
        self.ensure_session_ready(PRESENCE_UPDATE_OP)?;
        let frame = encode_presence_update(update)?;
        self.outbound_queue.push_back(frame.clone());
        Ok(frame)
    }

    /// Синоним [`Self::send_presence_update`] для постановки OP 3 в очередь.
    pub fn queue_presence_update(
        &mut self,
        update: &PresenceUpdate,
    ) -> Result<Vec<u8>, SupervisorOutboundError> {
        self.send_presence_update(update)
    }

    /// Кодирует и ставит в очередь кадр OP 4 (`VOICE_STATE_UPDATE`) в активной сессии.
    pub fn send_voice_state_update(
        &mut self,
        update: &VoiceStateUpdateOut,
    ) -> Result<Vec<u8>, SupervisorOutboundError> {
        self.ensure_session_ready(VOICE_STATE_UPDATE_OP)?;
        let frame = encode_voice_state_update(update)?;
        self.outbound_queue.push_back(frame.clone());
        Ok(frame)
    }

    /// Синоним [`Self::send_voice_state_update`] для постановки OP 4 в очередь.
    pub fn queue_voice_state_update(
        &mut self,
        update: &VoiceStateUpdateOut,
    ) -> Result<Vec<u8>, SupervisorOutboundError> {
        self.send_voice_state_update(update)
    }

    /// Кодирует и ставит в очередь кадр OP 8 (`REQUEST_GUILD_MEMBERS`) в активной сессии.
    pub fn send_request_guild_members(
        &mut self,
        request: &RequestGuildMembers,
    ) -> Result<Vec<u8>, SupervisorOutboundError> {
        self.ensure_session_ready(REQUEST_GUILD_MEMBERS_OP)?;
        let frame = encode_request_guild_members(request)?;
        self.outbound_queue.push_back(frame.clone());
        Ok(frame)
    }

    /// Синоним [`Self::send_request_guild_members`] для постановки OP 8 в очередь.
    pub fn queue_request_guild_members(
        &mut self,
        request: &RequestGuildMembers,
    ) -> Result<Vec<u8>, SupervisorOutboundError> {
        self.send_request_guild_members(request)
    }

    /// Кодирует и ставит в очередь чанкованные по [`GUILD_MEMBERS_CHUNK_SIZE`] (100 `user_ids`)
    /// кадры OP 8 (`REQUEST_GUILD_MEMBERS`) в активной сессии.
    pub fn send_request_guild_members_chunked(
        &mut self,
        request: &RequestGuildMembers,
    ) -> Result<Vec<Vec<u8>>, SupervisorOutboundError> {
        self.ensure_session_ready(REQUEST_GUILD_MEMBERS_OP)?;
        if request.user_ids.is_empty() {
            if request.query.is_none() {
                return Ok(Vec::new());
            }
            let frame = encode_request_guild_members(request)?;
            self.outbound_queue.push_back(frame.clone());
            return Ok(vec![frame]);
        }
        let chunks = chunk_user_ids(&request.user_ids);
        let mut frames = Vec::with_capacity(chunks.len());
        for chunk in chunks {
            let chunk_req = RequestGuildMembers {
                guild_id: request.guild_id.clone(),
                query: request.query.clone(),
                limit: request.limit,
                user_ids: chunk.to_vec(),
                presences: request.presences,
                nonce: request.nonce.clone(),
            };
            let frame = encode_request_guild_members(&chunk_req)?;
            self.outbound_queue.push_back(frame.clone());
            frames.push(frame);
        }
        Ok(frames)
    }

    /// Кодирует и ставит в очередь кадр OP 37 (`GUILD_SUBSCRIPTIONS_BULK`) в активной сессии.
    pub fn send_guild_subscriptions_bulk(
        &mut self,
        subscriptions: &GuildSubscriptionsBulk,
    ) -> Result<Vec<u8>, SupervisorOutboundError> {
        self.ensure_session_ready(GUILD_SUBSCRIPTIONS_BULK_OP)?;
        let frame = encode_guild_subscriptions(subscriptions)?;
        self.outbound_queue.push_back(frame.clone());
        Ok(frame)
    }

    /// Синоним [`Self::send_guild_subscriptions_bulk`] для постановки OP 37 в очередь.
    pub fn queue_guild_subscriptions_bulk(
        &mut self,
        subscriptions: &GuildSubscriptionsBulk,
    ) -> Result<Vec<u8>, SupervisorOutboundError> {
        self.send_guild_subscriptions_bulk(subscriptions)
    }

    /// Сколько исходящих клиентских кадров ждёт отправки в очереди супервайзера.
    pub fn pending_outbound_frames(&self) -> usize {
        self.outbound_queue.len()
    }

    /// Ссылка на очередь подготовленных исходящих кадров (OP 3/4/8/37).
    pub fn outbound_frames(&self) -> &VecDeque<Vec<u8>> {
        &self.outbound_queue
    }

    /// Извлекает один кадр из головы очереди исходящих кадров.
    pub fn pop_outbound_frame(&mut self) -> Option<Vec<u8>> {
        self.outbound_queue.pop_front()
    }

    /// Забирает все накопленные исходящие кадры в порядке постановки в очередь.
    pub fn take_outbound_frames(&mut self) -> Vec<Vec<u8>> {
        self.outbound_queue.drain(..).collect()
    }

    /// Сколько попыток отправки OP 3/4/8/37 было отклонено до установления сессии.
    pub fn rejected_outbound(&self) -> u64 {
        self.rejected_outbound
    }

    /// Текущее состояние для RESUME (наблюдение/тесты).
    pub fn resume_state(&self) -> Option<&ResumeState> {
        self.state.as_ref()
    }

    /// Нормализованный resume-адрес из последнего READY (наблюдение/тесты).
    pub fn resume_url(&self) -> Option<&GatewayUrl> {
        self.resume_url.as_ref()
    }

    /// Сколько реконнектов сделано без успешного READY/RESUMED.
    pub fn reconnects(&self) -> u32 {
        self.reconnects
    }

    /// Сколько подряд неудачных RESUME накоплено с последнего успеха/сброса.
    pub fn resume_failures(&self) -> u32 {
        self.resume_failures
    }

    /// Сколько раз сессия сбрасывалась после серии неудачных RESUME (F-GW-07) либо по TTL (F-GW-08).
    pub fn session_resets(&self) -> u32 {
        self.session_resets
    }

    /// Ошибка последней нормализации `resume_gateway_url` (если была).
    pub fn last_url_error(&self) -> Option<&str> {
        self.last_url_error.as_deref()
    }

    /// Последняя ошибка соединения/кадра (сбрасывается успешным исходом).
    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }
}

#[cfg(test)]
mod tests {
    //! Контрактные тесты F-GW-07 / BE-RE-06 (`RE-GAP-BACKEND-PROTOCOL.md` §3.6):
    //! - оракул: `/home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js`
    //!   (`_handleClose` @5 014 326, `_reset` @5 015 390, `_handleDispatch` @5 013 000):
    //!   1. `gatewayBackoff = new Backoff(1e3, 6e4)` — экспоненциальный backoff 1 с → 60 с;
    //!   2. при `gatewayBackoff.fails > 4` вызывается `_reset`: `this.sessionId = null; this.seq = 0`,
    //!      после чего следующая попытка идёт со свежим `IDENTIFY` (op2), а не `RESUME` (op6);
    //!   3. жёсткого лимита на 8 попыток нет: клиент продолжает переподключаться с потолком 60 с,
    //!      останавливаясь немедленно только на фатальном коде закрытия (`4004===t` и др.).

    use super::*;
    use discord_codec::etf::{self, EtfValue};
    use futures_util::{SinkExt, StreamExt};
    use std::io::Write;
    use std::sync::Mutex;
    use tokio::net::TcpListener;
    use tokio::time::timeout;
    use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
    use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};
    use tokio_tungstenite::{WebSocketStream, accept_async};

    const CASE_TIMEOUT: Duration = Duration::from_secs(5);
    const STUB_INTERVAL_MS: i64 = 41_250;
    const JITTER_SEED: u64 = 1_000;
    const SESSION_ID: &str = "f14ac1f3d408c15027d6d98da172c398";
    const FILLER_TEXT_LEN: usize = 1536;

    type StubSocket = WebSocketStream<tokio::net::TcpStream>;

    fn atom(name: &str) -> EtfValue {
        EtfValue::Atom(name.to_string())
    }

    fn map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
        EtfValue::Map(
            pairs
                .into_iter()
                .map(|(key, value)| (atom(key), value))
                .collect(),
        )
    }

    fn hello_value(interval: i64) -> EtfValue {
        map(vec![
            ("op", EtfValue::Int(10)),
            (
                "d",
                map(vec![("heartbeat_interval", EtfValue::Int(interval))]),
            ),
        ])
    }

    fn ready_value(session_id: &str) -> EtfValue {
        map(vec![
            ("op", EtfValue::Int(0)),
            ("t", atom("READY")),
            ("s", EtfValue::Int(1)),
            (
                "d",
                map(vec![
                    ("guilds", EtfValue::List(Vec::new())),
                    ("session_id", EtfValue::Str(session_id.to_string())),
                ]),
            ),
        ])
    }

    fn resumed_value(seq: i64) -> EtfValue {
        map(vec![
            ("op", EtfValue::Int(0)),
            ("t", atom("RESUMED")),
            ("s", EtfValue::Int(seq)),
            ("d", map(Vec::new())),
        ])
    }

    fn presence_value(seq: i64) -> EtfValue {
        map(vec![
            ("op", EtfValue::Int(0)),
            ("t", atom("PRESENCE_UPDATE")),
            ("s", EtfValue::Int(seq)),
            (
                "d",
                map(vec![
                    ("user", map(vec![("id", EtfValue::Str("42".to_string()))])),
                    ("status", atom("online")),
                ]),
            ),
        ])
    }

    fn op7_value() -> EtfValue {
        map(vec![("op", EtfValue::Int(7))])
    }

    struct StubStream {
        encoder: zstd::stream::write::Encoder<'static, Vec<u8>>,
        sent: usize,
    }

    impl StubStream {
        fn new() -> Self {
            let mut encoder =
                zstd::stream::write::Encoder::new(Vec::new(), 1).expect("zstd-энкодер");
            encoder
                .set_parameter(zstd::zstd_safe::CParameter::WindowLog(10))
                .expect("окно zstd 1 КиБ");
            Self { encoder, sent: 0 }
        }

        fn frame(&mut self, terms: &[EtfValue]) -> Vec<u8> {
            let mut payload = Vec::new();
            for term in terms {
                payload.extend_from_slice(&etf::encode(term));
            }
            payload.extend_from_slice(&etf::encode(&EtfValue::Str("f".repeat(FILLER_TEXT_LEN))));
            self.encoder.write_all(&payload).expect("запись в zstd");
            self.encoder.flush().expect("flush zstd-блока");
            let buffer = self.encoder.get_mut();
            let fresh = buffer[self.sent..].to_vec();
            self.sent = buffer.len();
            fresh
        }
    }

    async fn spawn_multi<T, F, Fut>(
        count: usize,
        handler: F,
    ) -> (String, tokio::task::JoinHandle<Result<Vec<T>, String>>)
    where
        T: Send + 'static,
        F: Fn(usize, StubSocket) -> Fut + Send + 'static,
        Fut: Future<Output = Result<T, String>> + Send,
    {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("стаб: bind 127.0.0.1:0");
        let addr = listener.local_addr().expect("стаб: local_addr");
        let task = tokio::spawn(async move {
            let mut results = Vec::with_capacity(count);
            for index in 0..count {
                let (tcp, _) = listener
                    .accept()
                    .await
                    .map_err(|error| format!("accept #{index}: {error}"))?;
                let ws = accept_async(tcp)
                    .await
                    .map_err(|error| format!("WS-handshake #{index}: {error}"))?;
                results.push(handler(index, ws).await?);
            }
            Ok(results)
        });
        (format!("ws://{addr}"), task)
    }

    async fn stub_result<T>(task: tokio::task::JoinHandle<Result<Vec<T>, String>>) -> Vec<T> {
        timeout(CASE_TIMEOUT, task)
            .await
            .expect("стаб-задача не завершилась за 5 с")
            .expect("стаб-задача упала")
            .expect("сценарий стаба вернул ошибку")
    }

    async fn send_terms(
        ws: &mut StubSocket,
        stream: &mut StubStream,
        terms: &[EtfValue],
    ) -> Result<(), String> {
        ws.send(Message::binary(stream.frame(terms)))
            .await
            .map_err(|error| error.to_string())
    }

    async fn close_with(ws: &mut StubSocket, code: u16, reason: &str) -> Result<(), String> {
        ws.close(Some(CloseFrame {
            code: CloseCode::from(code),
            reason: reason.into(),
        }))
        .await
        .map_err(|error| error.to_string())
    }

    async fn drain_until_drop(ws: &mut StubSocket) -> Result<(), String> {
        loop {
            match ws.next().await {
                Some(Ok(_)) => {}
                Some(Err(error)) => {
                    let text = error.to_string();
                    if text.contains("ResetWithoutClosingHandshake")
                        || text.contains("Connection reset")
                    {
                        return Ok(());
                    }
                    return Err(text);
                }
                None => return Ok(()),
            }
        }
    }

    async fn next_handshake_frame(ws: &mut StubSocket) -> Result<EtfValue, String> {
        loop {
            match timeout(CASE_TIMEOUT, ws.next())
                .await
                .map_err(|_| "клиент молчит дольше 5 с".to_string())?
            {
                Some(Ok(Message::Binary(data))) => {
                    let (value, consumed) =
                        etf::decode(&data).map_err(|error| format!("кадр клиента: {error}"))?;
                    if consumed != data.len() {
                        return Err(format!("хвост кадра клиента: {consumed} != {}", data.len()));
                    }
                    if value.get("op").and_then(EtfValue::as_int) == Some(40) {
                        continue;
                    }
                    return Ok(value);
                }
                Some(Ok(Message::Close(_))) | None => {
                    return Err("клиент закрыл соединение до рукопожатия".to_string());
                }
                Some(Ok(_)) => {}
                Some(Err(error)) => return Err(error.to_string()),
            }
        }
    }

    /// (а) F-GW-07: после 4 последовательных неудачных попыток RESUME супервайзер
    /// сбрасывает `session_id` и `seq` (`_reset` в бандле при `gatewayBackoff.fails > 4`)
    /// и на 5-м переподключении отправляет свежий `IDENTIFY` (op=2), а не `RESUME` (op=6).
    #[tokio::test]
    async fn resets_session_after_four_failed_resumes_and_switches_to_fresh_identify() {
        let (url, stub) = spawn_multi(6, |index, mut ws| async move {
            let mut stream = StubStream::new();
            send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
            let frame = next_handshake_frame(&mut ws).await?;
            if index == 0 {
                send_terms(
                    &mut ws,
                    &mut stream,
                    &[ready_value(SESSION_ID), presence_value(7), op7_value()],
                )
                .await?;
                drain_until_drop(&mut ws).await?;
            } else if index < 5 {
                // Попытки 1..=4: RESUME не дождался RESUMED, сокет закрыт нефатальным 4000.
                close_with(&mut ws, 4000, "resume failed").await?;
            } else {
                // Попытка 5 (после 4 неудачных RESUME): ожидаем свежий IDENTIFY и завершаем 4004.
                close_with(&mut ws, 4004, "authentication failed").await?;
            }
            Ok(frame)
        })
        .await;

        let gateway = GatewayUrl::from_base(&url).expect("URL стаба");
        let mut config = SupervisorConfig::new(gateway, "TEST_TOKEN");
        config.jitter_seed = JITTER_SEED;
        config.backoff = Backoff::new(1, 2);
        let mut supervisor = Supervisor::new(config);

        let outcome = timeout(CASE_TIMEOUT, supervisor.run())
            .await
            .expect("супервайзер не завершился за 5 с");
        let frames = stub_result(stub).await;

        assert_eq!(
            outcome,
            SupervisorOutcome::Stop(StopReason::Unauthorized),
            "серию останавливает только фатальный код 4004"
        );
        let ops: Vec<Option<i64>> = frames
            .iter()
            .map(|frame| frame.get("op").and_then(EtfValue::as_int))
            .collect();
        assert_eq!(
            ops,
            vec![Some(2), Some(6), Some(6), Some(6), Some(6), Some(2)],
            "после 4 неудачных RESUME (op=6) 5-я попытка обязана перейти на свежий IDENTIFY (op=2)"
        );

        for (attempt, frame) in frames[1..=4].iter().enumerate() {
            let d = frame.get("d").expect("payload RESUME");
            assert_eq!(
                d.get("session_id").and_then(EtfValue::as_str),
                Some(SESSION_ID),
                "попытка RESUME #{} сохраняет session_id",
                attempt + 1
            );
            assert_eq!(
                d.get("seq").and_then(EtfValue::as_int),
                Some(7),
                "попытка RESUME #{} сохраняет последний seq=7",
                attempt + 1
            );
        }

        let fresh_identify_d = frames[5].get("d").expect("payload свежего IDENTIFY");
        assert_eq!(
            fresh_identify_d.get("token").and_then(EtfValue::as_str),
            Some("TEST_TOKEN"),
            "5-я попытка несёт токен в свежем IDENTIFY"
        );
        assert!(
            supervisor.resume_state().is_none(),
            "после 4 неудачных RESUME session_id и seq сброшены в None"
        );
        assert_eq!(
            supervisor.resume_failures(),
            0,
            "после сброса счётчик неудачных RESUME обнулён"
        );
        assert_eq!(
            supervisor.session_resets(),
            1,
            "сброс сессии зафиксирован ровно один раз"
        );
    }

    /// (а) F-GW-07: успешный RESUMED сбрасывает счётчик неудачных RESUME и backoff
    /// (`gatewayBackoff.succeed()` в `_handleDispatch`), поэтому серия из 3 неудач до RESUMED
    /// и 3 неудач после RESUMED не сбрасывает сессию преждевременно, а сброс наступает ровно
    /// после 4 подряд неудачных RESUME.
    #[tokio::test]
    async fn successful_resumed_resets_failure_streak_and_backoff() {
        let (url, stub) = spawn_multi(10, |index, mut ws| async move {
            let mut stream = StubStream::new();
            send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
            let frame = next_handshake_frame(&mut ws).await?;
            match index {
                0 => {
                    send_terms(
                        &mut ws,
                        &mut stream,
                        &[ready_value(SESSION_ID), presence_value(3), op7_value()],
                    )
                    .await?;
                    drain_until_drop(&mut ws).await?;
                }
                1..=3 => {
                    // 3 неудачных RESUME (ещё не порог 4).
                    close_with(&mut ws, 4000, "transient failure").await?;
                }
                4 => {
                    // 4-я попытка RESUME успешна: сервер присылает RESUMED (s=9) и затем op7.
                    send_terms(&mut ws, &mut stream, &[resumed_value(9), op7_value()]).await?;
                    drain_until_drop(&mut ws).await?;
                }
                5..=8 => {
                    // Новая серия из 4 подряд неудачных RESUME после успешного RESUMED.
                    close_with(&mut ws, 4000, "resume failed again").await?;
                }
                _ => {
                    // После 4 подряд неудач — свежий IDENTIFY, стоп по 4004.
                    close_with(&mut ws, 4004, "authentication failed").await?;
                }
            }
            Ok(frame)
        })
        .await;

        let delays = Arc::new(Mutex::new(Vec::new()));
        let delays_sink = Arc::clone(&delays);
        let gateway = GatewayUrl::from_base(&url).expect("URL стаба");
        let mut config =
            SupervisorConfig::new(gateway, "TEST_TOKEN").with_event_handler(move |event| {
                if let SupervisorEvent::Reconnecting { delay_ms } = event {
                    delays_sink.lock().expect("delays").push(delay_ms);
                }
            });
        config.jitter_seed = JITTER_SEED;
        config.backoff = Backoff::new(1, 8);
        let mut supervisor = Supervisor::new(config);

        let outcome = timeout(CASE_TIMEOUT, supervisor.run())
            .await
            .expect("супервайзер не завершился за 5 с");
        assert_eq!(outcome, SupervisorOutcome::Stop(StopReason::Unauthorized));
        let frames = stub_result(stub).await;

        let ops: Vec<Option<i64>> = frames
            .iter()
            .map(|frame| frame.get("op").and_then(EtfValue::as_int))
            .collect();
        assert_eq!(
            ops,
            vec![
                Some(2),
                Some(6),
                Some(6),
                Some(6),
                Some(6),
                Some(6),
                Some(6),
                Some(6),
                Some(6),
                Some(2),
            ],
            "3 неудачи + успешный RESUMED не сбрасывают сессию; сброс только после 4 подряд неудач"
        );
        assert_eq!(
            frames[5]
                .get("d")
                .and_then(|d| d.get("seq"))
                .and_then(EtfValue::as_int),
            Some(9),
            "после RESUMED (s=9) следующий RESUME продолжает с seq=9"
        );
        assert_eq!(
            *delays.lock().expect("delays"),
            vec![1, 2, 4, 8, 1, 2, 4, 8, 8],
            "RESUMED сбрасывает backoff к базе (1 мс), а серия из 4 неудач удерживает потолок"
        );
    }

    /// (б) F-GW-07: режим неограниченного переподключения по умолчанию (`max_reconnects == 0`)
    /// удерживает экспоненциальный backoff на потолке 60 с (60 000 мс), не останавливается
    /// на старом лимите в 8 попыток, но немедленно прекращает работу при фатальном коде 4004.
    #[tokio::test]
    async fn unlimited_reconnect_mode_holds_60s_ceiling_and_stops_immediately_on_4004() {
        let dummy_gateway =
            GatewayUrl::from_base("wss://gateway.discord.gg").expect("валидный URL шлюза");
        let mut default_config = SupervisorConfig::new(dummy_gateway, "TEST_TOKEN");
        let schedule: Vec<u64> = (0..10)
            .map(|_| default_config.backoff.next_delay_ms())
            .collect();
        assert_eq!(
            schedule,
            vec![
                1_000, 2_000, 4_000, 8_000, 16_000, 32_000, 60_000, 60_000, 60_000, 60_000
            ],
            "боевой backoff экспоненциально растёт 1 с ×2 и удерживается на потолке 60 000 мс"
        );

        // 12 неудачных попыток подряд (> старого порога 8), на 13-м соединении — фатальный 4004.
        let (url, stub) = spawn_multi(13, |index, mut ws| async move {
            let mut stream = StubStream::new();
            send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
            let frame = next_handshake_frame(&mut ws).await?;
            if index < 12 {
                close_with(&mut ws, 4000, "temporary outage").await?;
            } else {
                close_with(&mut ws, 4004, "authentication failed").await?;
            }
            Ok(frame)
        })
        .await;

        let events = Arc::new(Mutex::new(Vec::new()));
        let events_sink = Arc::clone(&events);
        let gateway = GatewayUrl::from_base(&url).expect("URL стаба");
        let mut config =
            SupervisorConfig::new(gateway, "TEST_TOKEN").with_event_handler(move |event| {
                events_sink.lock().expect("events").push(event);
            });
        config.jitter_seed = JITTER_SEED;
        config.backoff = Backoff::new(1, 4);
        let mut supervisor = Supervisor::new(config);

        let outcome = timeout(CASE_TIMEOUT, supervisor.run())
            .await
            .expect("супервайзер не дошёл до 13-го соединения с 4004 за 5 с");
        assert_eq!(
            outcome,
            SupervisorOutcome::Stop(StopReason::Unauthorized),
            "неограниченный режим проходит >8 попыток и останавливается только на 4004"
        );
        assert_eq!(
            default_config.max_reconnects, 0,
            "боевой дефолт SupervisorConfig::new — неограниченный режим (0)"
        );
        stub_result(stub).await;
        assert_eq!(
            supervisor.reconnects(),
            12,
            "выполнено 12 реконнектов без Stop(ReconnectLimit)"
        );

        let recorded = events.lock().expect("events").clone();
        let delays: Vec<u64> = recorded
            .iter()
            .filter_map(|event| match event {
                SupervisorEvent::Reconnecting { delay_ms } => Some(*delay_ms),
                _ => None,
            })
            .collect();
        assert_eq!(
            delays,
            vec![1, 2, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4],
            "backoff удерживается на потолке все 12 переподключений, после 4004 нового Reconnecting нет"
        );
        assert_eq!(
            recorded.last(),
            Some(&SupervisorEvent::Stopped(StopReason::Unauthorized)),
            "последнее событие — немедленный Stopped(Unauthorized)"
        );

        // Проверка немедленной остановки при 4004 даже при 60-секундном потолке backoff:
        // если бы супервайзер ждал 60 с перед обработкой 4004, тест упал бы по тайм-ауту 500 мс.
        let (url_4004, stub_4004) = spawn_multi(1, |_index, mut ws| async move {
            let mut stream = StubStream::new();
            send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
            let _ = next_handshake_frame(&mut ws).await?;
            close_with(&mut ws, 4004, "authentication failed").await?;
            Ok(())
        })
        .await;

        let gateway_4004 = GatewayUrl::from_base(&url_4004).expect("URL стаба");
        let mut config_60s = SupervisorConfig::new(gateway_4004, "TEST_TOKEN");
        config_60s.jitter_seed = JITTER_SEED;
        config_60s.backoff = Backoff::new(60_000, 60_000);
        let mut supervisor_60s = Supervisor::new(config_60s);

        let fast_outcome = timeout(Duration::from_millis(500), supervisor_60s.run())
            .await
            .expect("4004 обязан останавливать супервайзер немедленно, не ожидая 60 с backoff");
        stub_result(stub_4004).await;
        assert_eq!(
            fast_outcome,
            SupervisorOutcome::Stop(StopReason::Unauthorized)
        );
        assert_eq!(supervisor_60s.reconnects(), 0);
    }

    fn op1_value() -> EtfValue {
        map(vec![("op", EtfValue::Int(1))])
    }

    fn op11_value() -> EtfValue {
        map(vec![("op", EtfValue::Int(11))])
    }

    /// (в) F-GW-08: сессия пригодна для RESUME в течение `RESUME_TTL_MS` (180 000 мс = 3 мин)
    /// с момента последнего полученного `HEARTBEAT_ACK` (op11) / успешного события; при
    /// `elapsed <= 180_000` мс сохраняется `RESUME` (op6) с прежними `session_id` и `seq`,
    /// а при `elapsed > 180_000` мс супервайзер сразу сбрасывает `session_id`/`seq` и выбирает
    /// свежий `IDENTIFY` (op2).
    #[tokio::test]
    async fn resume_ttl_preserves_session_up_to_180s_and_resets_to_identify_after_180s() {
        assert_eq!(
            RESUME_TTL_MS, 180_000,
            "F-GW-08: окно RESUME от последнего ACK равно 180 000 мс (3 минуты)"
        );

        // 1. Граничная проверка на `elapsed <= 180_000` мс (включая > старых 30 с):
        // при 180 000 мс с последнего ACK соединение #1 отправляет RESUME (op=6).
        let (url_within, stub_within) = spawn_multi(2, |index, mut ws| async move {
            let mut stream = StubStream::new();
            send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
            let frame = next_handshake_frame(&mut ws).await?;
            if index == 0 {
                send_terms(
                    &mut ws,
                    &mut stream,
                    &[
                        ready_value(SESSION_ID),
                        op1_value(),
                        op11_value(),
                        presence_value(7),
                        op7_value(),
                    ],
                )
                .await?;
                drain_until_drop(&mut ws).await?;
            } else {
                close_with(&mut ws, 4004, "done").await?;
            }
            Ok(frame)
        })
        .await;

        let gateway_within = GatewayUrl::from_base(&url_within).expect("URL стаба");
        let mut config_within = SupervisorConfig::new(gateway_within, "TEST_TOKEN");
        config_within.jitter_seed = JITTER_SEED;
        config_within.backoff = Backoff::new(1, 2);
        let mut supervisor_within = GatewaySupervisor::new(config_within);
        supervisor_within.set_last_ack_elapsed_ms(RESUME_TTL_MS);

        let outcome_within = timeout(CASE_TIMEOUT, supervisor_within.run())
            .await
            .expect("супервайзер (<= 180 с) не завершился за 5 с");
        assert_eq!(
            outcome_within,
            SupervisorOutcome::Stop(StopReason::Unauthorized)
        );
        let frames_within = stub_result(stub_within).await;
        assert_eq!(
            frames_within[0].get("op").and_then(EtfValue::as_int),
            Some(2),
            "первое соединение отправляет IDENTIFY (op=2)"
        );
        assert_eq!(
            frames_within[1].get("op").and_then(EtfValue::as_int),
            Some(6),
            "при elapsed == 180 000 мс (<= TTL) супервайзер обязан сохранять RESUME (op=6)"
        );
        let resume_d = frames_within[1].get("d").expect("payload RESUME");
        assert_eq!(
            resume_d.get("session_id").and_then(EtfValue::as_str),
            Some(SESSION_ID),
            "при elapsed == 180 000 мс session_id сохранён"
        );
        assert_eq!(
            resume_d.get("seq").and_then(EtfValue::as_int),
            Some(7),
            "при elapsed == 180 000 мс последний seq=7 сохранён"
        );
        assert_eq!(
            supervisor_within.session_resets(),
            0,
            "при elapsed <= 180 000 мс сброса сессии нет"
        );

        // 2. Граничная проверка на `elapsed > 180_000` мс (180 001 мс с последнего ACK):
        // супервайзер сразу сбрасывает `session_id`/`seq` и на соединении #1 шлёт свежий IDENTIFY (op=2).
        let (url_expired, stub_expired) = spawn_multi(2, |index, mut ws| async move {
            let mut stream = StubStream::new();
            send_terms(&mut ws, &mut stream, &[hello_value(STUB_INTERVAL_MS)]).await?;
            let frame = next_handshake_frame(&mut ws).await?;
            if index == 0 {
                send_terms(
                    &mut ws,
                    &mut stream,
                    &[
                        ready_value(SESSION_ID),
                        op1_value(),
                        op11_value(),
                        presence_value(7),
                        op7_value(),
                    ],
                )
                .await?;
                drain_until_drop(&mut ws).await?;
            } else {
                close_with(&mut ws, 4004, "done").await?;
            }
            Ok(frame)
        })
        .await;

        let gateway_expired = GatewayUrl::from_base(&url_expired).expect("URL стаба");
        let mut config_expired = SupervisorConfig::new(gateway_expired, "TEST_TOKEN");
        config_expired.jitter_seed = JITTER_SEED;
        config_expired.backoff = Backoff::new(1, 2);
        let mut supervisor_expired = GatewaySupervisor::new(config_expired);
        supervisor_expired.set_last_ack_elapsed_ms(RESUME_TTL_MS + 1);

        let outcome_expired = timeout(CASE_TIMEOUT, supervisor_expired.run())
            .await
            .expect("супервайзер (> 180 с) не завершился за 5 с");
        assert_eq!(
            outcome_expired,
            SupervisorOutcome::Stop(StopReason::Unauthorized)
        );
        let frames_expired = stub_result(stub_expired).await;
        assert_eq!(
            frames_expired[0].get("op").and_then(EtfValue::as_int),
            Some(2),
            "первое соединение отправляет IDENTIFY (op=2)"
        );
        assert_eq!(
            frames_expired[1].get("op").and_then(EtfValue::as_int),
            Some(2),
            "при elapsed == 180 001 мс (> TTL) супервайзер обязан сбросить сессию и отправить свежий IDENTIFY (op=2)"
        );
        let identify_d = frames_expired[1]
            .get("d")
            .expect("payload свежего IDENTIFY");
        assert_eq!(
            identify_d.get("token").and_then(EtfValue::as_str),
            Some("TEST_TOKEN"),
            "свежий IDENTIFY после истечения TTL несёт токен"
        );
        assert!(
            supervisor_expired.resume_state().is_none(),
            "после elapsed > 180 000 мс session_id и seq сброшены в None"
        );
        assert_eq!(
            supervisor_expired.session_resets(),
            1,
            "истечение 3-минутного TTL зафиксировано как сброс сессии"
        );
    }

    /// (г) F-GW-02 / BE-P1-01: кодирование и постановка в очередь клиентских кадров
    /// OP 3 (`PresenceUpdate`), OP 4 (`VoiceStateUpdateOut`), OP 8 (`RequestGuildMembers`)
    /// и OP 37 (`GuildSubscriptionsBulk`) через `GatewaySupervisor` работают в активной
    /// сессии (`Ready`/`Connected`) и отвергаются до завершения рукопожатия либо после сброса.
    #[test]
    fn outbound_opcodes_3_4_8_37_reject_before_ready_and_encode_in_active_session() {
        use crate::state::PresenceStatus;

        let gateway =
            GatewayUrl::from_base("wss://gateway.discord.gg").expect("валидный URL шлюза");
        let mut supervisor = GatewaySupervisor::new(SupervisorConfig::new(gateway, "TEST_TOKEN"));

        let presence = PresenceUpdate {
            status: PresenceStatus::Online,
            since: 0,
            activities: Vec::new(),
            afk: false,
        };
        let voice = VoiceStateUpdateOut {
            guild_id: Some("1014537851418980372".to_string()),
            channel_id: Some("1014537851997798404".to_string()),
            self_mute: true,
            self_deaf: false,
            self_video: false,
            flags: 2,
        };
        let members_single = RequestGuildMembers {
            guild_id: "1014537851418980372".to_string(),
            query: Some(String::new()),
            limit: Some(0),
            user_ids: Vec::new(),
            presences: true,
            nonce: Some("req-1".to_string()),
        };
        let mut subs = GuildSubscriptionsBulk::default();
        subs.insert(
            "1014537851418980372".to_string(),
            OutboundGuildSubscription {
                typing: true,
                threads: true,
                activities: true,
                members: vec!["80351110224678912".to_string()],
                member_updates: false,
                channels: vec!["1014537851997798404".to_string()],
            },
        );

        // 1. До рукопожатия (и при получении только HELLO без READY) сессия не готова:
        // все четыре опкода 3/4/8/37 обязаны вернуть `NotReady` и не попасть в очередь.
        assert!(!supervisor.is_session_ready());
        let mut pipe = GatewayPipe::new(Session::with_heartbeat_mode(
            JITTER_SEED,
            HeartbeatMode::Qos,
        ));
        let mut stream = StubStream::new();
        pipe.push_binary(&stream.frame(&[hello_value(STUB_INTERVAL_MS)]), 0)
            .expect("HELLO разобран");
        supervisor.absorb_pipe(&pipe);
        assert!(
            !supervisor.is_session_ready(),
            "одного HELLO без READY недостаточно для отправки клиентских опкодов"
        );

        assert_eq!(
            supervisor.send_presence_update(&presence),
            Err(SupervisorOutboundError::NotReady { op: 3 })
        );
        assert_eq!(
            supervisor.send_voice_state_update(&voice),
            Err(SupervisorOutboundError::NotReady { op: 4 })
        );
        assert_eq!(
            supervisor.send_request_guild_members(&members_single),
            Err(SupervisorOutboundError::NotReady { op: 8 })
        );
        assert_eq!(
            supervisor.send_guild_subscriptions_bulk(&subs),
            Err(SupervisorOutboundError::NotReady { op: 37 })
        );
        assert_eq!(supervisor.pending_outbound_frames(), 0);
        assert_eq!(supervisor.rejected_outbound(), 4);

        // 2. После завершения рукопожатия (READY + HEARTBEAT_ACK) сессия активна:
        // кодировщики OP 3/4/8/37 формируют валидные ETF-кадры и ставят их в очередь.
        pipe.push_binary(
            &stream.frame(&[
                ready_value(SESSION_ID),
                op1_value(),
                op11_value(),
                presence_value(5),
            ]),
            10,
        )
        .expect("READY + ACK разобраны");
        supervisor.absorb_pipe(&pipe);
        assert!(supervisor.is_session_ready());

        let frame_op3 = supervisor
            .send_presence_update(&presence)
            .expect("OP 3 кодируется в активной сессии");
        let expected_op3_hex = concat!(
            "83",
            "7400000002",
            "6d000000026f70",
            "6103",
            "6d0000000164",
            "7400000004",
            "6d00000006737461747573",
            "6d000000066f6e6c696e65",
            "6d0000000573696e6365",
            "6100",
            "6d0000000a61637469766974696573",
            "6c000000006a",
            "6d0000000361666b",
            "64000566616c7365",
        );
        let actual_op3_hex: String = frame_op3.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(
            actual_op3_hex, expected_op3_hex,
            "OP 3 обязан побайтно совпадать с независимым wetf-оракулом"
        );

        let frame_op4 = supervisor
            .send_voice_state_update(&voice)
            .expect("OP 4 кодируется в активной сессии");
        let (decoded_op4, consumed_op4) = etf::decode(&frame_op4).expect("декод OP 4");
        assert_eq!(consumed_op4, frame_op4.len());
        assert_eq!(decoded_op4.get("op").and_then(EtfValue::as_int), Some(4));
        let d4 = decoded_op4.get("d").expect("d в OP 4");
        assert_eq!(
            d4.get("guild_id").and_then(EtfValue::as_str),
            Some("1014537851418980372")
        );
        assert_eq!(
            d4.get("channel_id").and_then(EtfValue::as_str),
            Some("1014537851997798404")
        );
        assert_eq!(
            d4.get("self_mute").and_then(EtfValue::as_atom),
            Some("true")
        );
        assert_eq!(
            d4.get("self_deaf").and_then(EtfValue::as_atom),
            Some("false")
        );
        assert_eq!(d4.get("flags").and_then(EtfValue::as_int), Some(2));

        let frame_op8 = supervisor
            .send_request_guild_members(&members_single)
            .expect("OP 8 кодируется в активной сессии");
        let (decoded_op8, consumed_op8) = etf::decode(&frame_op8).expect("декод OP 8");
        assert_eq!(consumed_op8, frame_op8.len());
        assert_eq!(decoded_op8.get("op").and_then(EtfValue::as_int), Some(8));
        let d8 = decoded_op8.get("d").expect("d в OP 8");
        assert_eq!(
            d8.get("guild_id").and_then(EtfValue::as_str),
            Some("1014537851418980372")
        );
        assert_eq!(
            d8.get("presences").and_then(EtfValue::as_atom),
            Some("true")
        );
        assert_eq!(d8.get("nonce").and_then(EtfValue::as_str), Some("req-1"));

        // Проверка чанкования OP 8 по 100 user_ids (101 id → 2 кадра: 100 + 1).
        let members_101 = RequestGuildMembers {
            guild_id: "1014537851418980372".to_string(),
            query: None,
            limit: None,
            user_ids: (0..101).map(|i| format!("user-{i}")).collect(),
            presences: false,
            nonce: None,
        };
        let chunk_frames = supervisor
            .send_request_guild_members_chunked(&members_101)
            .expect("OP 8 чанкуется по 100 user_ids");
        assert_eq!(
            chunk_frames.len(),
            2,
            "101 user_id → 2 кадра OP 8 (100 + 1)"
        );

        let frame_op37 = supervisor
            .send_guild_subscriptions_bulk(&subs)
            .expect("OP 37 кодируется в активной сессии");
        let (decoded_op37, consumed_op37) = etf::decode(&frame_op37).expect("декод OP 37");
        assert_eq!(consumed_op37, frame_op37.len());
        assert_eq!(decoded_op37.get("op").and_then(EtfValue::as_int), Some(37));
        let guild_sub = decoded_op37
            .get("d")
            .and_then(|d| d.get("subscriptions"))
            .and_then(|s| s.get("1014537851418980372"))
            .expect("подписка гильдии в OP 37");
        assert_eq!(
            guild_sub.get("typing").and_then(EtfValue::as_atom),
            Some("true")
        );
        assert_eq!(
            guild_sub.get("member_updates").and_then(EtfValue::as_atom),
            Some("false")
        );

        let queued = supervisor.take_outbound_frames();
        assert_eq!(
            queued.len(),
            6,
            "в очереди 6 кадров: OP 3, OP 4, одиночный OP 8, 2 чанка OP 8 и OP 37"
        );
        assert_eq!(supervisor.pending_outbound_frames(), 0);

        // 3. После истечения 3-минутного TTL с последнего ACK сессия сбрасывается
        // и отправка OP 3/4/8/37 снова блокируется до нового READY.
        supervisor.set_last_ack_elapsed_ms(RESUME_TTL_MS + 1);
        assert_eq!(supervisor.plan_next_reconnect(), ReconnectAction::Identify);
        assert!(!supervisor.is_session_ready());
        assert_eq!(
            supervisor.send_presence_update(&presence),
            Err(SupervisorOutboundError::NotReady { op: 3 })
        );
    }
}
