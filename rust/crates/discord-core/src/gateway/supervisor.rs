//! # Супервайзер шлюза: одно соединение → серия (RW-39, NET-E1; CORE-AUDIT P0-3)
//!
//! `connect_and_run*` обслуживает **одно** соединение; этот модуль — политика вокруг него:
//! connect → HELLO → IDENTIFY/RESUME → dispatch/heartbeat → close, затем решение
//! «остановиться / переподключиться / резюмировать». Источники поведения:
//! - `AUTH-RE.md` G-16…G-18: `resume_gateway_url` (хвостовой `/` обрезан), close 4004 —
//!   logout без ретрая; op7 — новый коннект; op9 `d=true` → RESUME, `d=false` → IDENTIFY;
//! - `ADR-2-auth.md`: 4013/4014 прекращают reconnect; heartbeat — op40 с fallback op1;
//! - `resume.rs` T-GW-07: [`plan_reconnect`] решает RESUME/IDENTIFY по разрыву и флагу op9.
//!
//! Backoff — как `gatewayBackoff` клиента (`new Backoff(1e3, 6e4)`): 1 с, ×2, потолок 60 с,
//! сброс после READY. Лимит `max_reconnects` страхует от бесконечного цикла (например, сервер
//! закрывает соединение сразу после HELLO). Отмена — внешний future в [`Supervisor::run_until`]:
//! супервайзер не держит глобального состояния и готов к остановке на любом шаге.

use std::fmt;
use std::future::Future;
use std::sync::Arc;
use std::time::{Duration, Instant};

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
    /// Сколько реконнектов разрешено до `Stop(ReconnectLimit)`; успешный READY сбрасывает счётчик.
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
            max_reconnects: 8,
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
    /// Исчерпан лимит реконнектов (`max_reconnects`).
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
    /// Нормализованный `resume_gateway_url` из READY (G-16); приоритетнее `config.gateway`.
    resume_url: Option<GatewayUrl>,
    /// Флаг последнего op9: `Some(false)` запрещает RESUME до следующего IDENTIFY.
    invalid_session: Option<bool>,
    /// Момент разрыва (для `plan_reconnect`: разрыв ≥ 30 с → IDENTIFY).
    disconnected_at: Option<Instant>,
    /// Сколько реконнектов сделано без успешного READY подряд.
    reconnects: u32,
    /// Текущий backoff (сбрасывается на READY).
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
            resume_url: None,
            invalid_session: None,
            disconnected_at: None,
            reconnects: 0,
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
            let action = self.next_action();
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
            if !ready_emitted && pipe.last_ready().is_some() {
                ready_emitted = true;
                self.emit(SupervisorEvent::Ready);
            }

            match outcome {
                Ok(RunOutcome::Closed { code }) => {
                    self.last_error = None;
                    if let CloseAction::Stop(reason) = classify_close(code) {
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

            if self.reconnects >= self.config.max_reconnects {
                let reason = StopReason::ReconnectLimit {
                    reconnects: self.reconnects,
                };
                self.emit(SupervisorEvent::Stopped(reason.clone()));
                return SupervisorOutcome::Stop(reason);
            }
            self.reconnects = self.reconnects.saturating_add(1);
            self.disconnected_at = Some(Instant::now());
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

    /// РЕЗЮМЕ/IDENTIFY по `plan_reconnect` (T-GW-07) с учётом разрыва и op9.
    fn next_action(&self) -> ReconnectAction {
        let downtime_ms = self
            .disconnected_at
            .map(|at| u64::try_from(at.elapsed().as_millis()).unwrap_or(u64::MAX))
            .unwrap_or(0);
        plan_reconnect(self.state.as_ref(), downtime_ms, self.invalid_session)
    }

    /// Адрес следующего соединения: resume-URL из READY либо база.
    fn current_url(&self) -> GatewayUrl {
        self.resume_url
            .clone()
            .unwrap_or_else(|| self.config.gateway.clone())
    }

    /// Забирает из соединения всё, что переживает его: session_id/seq, resume-URL, флаг op9.
    fn absorb(&mut self, pipe: &GatewayPipe) {
        if let Some(ready) = pipe.last_ready() {
            match &ready.session_id {
                Some(session_id) => {
                    self.state = Some(ResumeState {
                        session_id: session_id.clone(),
                        last_seq: pipe.session().last_seq().unwrap_or(0),
                    });
                }
                // READY без session_id — резюмировать нечего; старую сессию не тащим.
                None => self.state = None,
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
            // READY — сессия установлена: бюджет и backoff сбрасываются (gatewayBackoff.succeed).
            self.invalid_session = None;
            self.reconnects = 0;
            self.backoff.reset();
            self.disconnected_at = None;
        }
        if let (Some(seq), Some(state)) = (pipe.session().last_seq(), self.state.as_mut()) {
            state.last_seq = seq;
        }
        if let Some(resumable) = pipe.last_invalid_session_resumable() {
            self.invalid_session = Some(resumable);
        }
    }

    /// Текущее состояние для RESUME (наблюдение/тесты).
    pub fn resume_state(&self) -> Option<&ResumeState> {
        self.state.as_ref()
    }

    /// Нормализованный resume-адрес из последнего READY (наблюдение/тесты).
    pub fn resume_url(&self) -> Option<&GatewayUrl> {
        self.resume_url.as_ref()
    }

    /// Сколько реконнектов сделано без успешного READY.
    pub fn reconnects(&self) -> u32 {
        self.reconnects
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
