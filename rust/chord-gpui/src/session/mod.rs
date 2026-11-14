#![forbid(unsafe_code)]

//! Live Discord `SessionManager` Lifecycle & Gateway Discovery (`P0-1.2`).
//! Clean-room references: `docs/re/gateway-protocol.md`, `docs/re/rest-d02.md`,
//! `chord_core::gateway::supervisor` (`Supervisor`, `StopReason`, `classify_close`).

use std::fmt;
use std::future::Future;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::Duration;

use chord_core::rest::rate_limit::rate_limit_info;
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::transport::{
    Endpoint, HttpClientConfig, HttpRequest, HttpResponse, send_with,
};
use chord_core::{
    API_V9_BASE, Backoff, CloseAction, GatewayUrl, StopReason, Supervisor, SupervisorConfig,
    SupervisorEvent, SupervisorOutcome, classify_close,
};
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot, watch};

pub const GATEWAY_DISCOVERY_PATH: &str = "/gateway";
pub const ENV_DISCORD_API_BASE: &str = "DISCORD_API_BASE";
pub const ENV_DISCORD_GATEWAY_URL: &str = "DISCORD_GATEWAY_URL";
pub const REDACTED_SECRET: &str = "[REDACTED]";
pub const DEFAULT_RATE_LIMIT_RETRY_MS: u64 = 5_000;

/// Lifecycle state of the Discord Gateway session (`P0-1.2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionState {
    Disconnected,
    DiscoveringGateway,
    Connecting {
        attempt: u32,
        gateway_url: String,
    },
    Connected {
        session_id: Option<String>,
        gateway_url: String,
    },
    Reconnecting {
        attempt: u32,
        resumable: bool,
    },
    LoggedOut {
        fatal_close_code: Option<u16>,
        reason: String,
    },
    ShuttingDown,
}

impl SessionState {
    #[must_use]
    pub fn is_connected(&self) -> bool {
        matches!(self, Self::Connected { .. })
    }

    #[must_use]
    pub fn is_logged_out(&self) -> bool {
        matches!(self, Self::LoggedOut { .. })
    }
}

/// Typed errors produced during Gateway discovery and session lifecycle management.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    EmptyToken,
    InvalidApiBase(String),
    InvalidGatewayUrl(String),
    InvalidDiscoveryPayload(String),
    Unauthorized { status: u16 },
    RateLimited { retry_after_ms: u64 },
    ServerError { status: u16 },
    ClientError { status: u16 },
    Transport(String),
    RunnerStopped(String),
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyToken => write!(f, "session token must not be empty"),
            Self::InvalidApiBase(msg) => write!(f, "invalid API base URL: {msg}"),
            Self::InvalidGatewayUrl(msg) => write!(f, "invalid gateway URL: {msg}"),
            Self::InvalidDiscoveryPayload(msg) => write!(f, "invalid /gateway payload: {msg}"),
            Self::Unauthorized { status } => write!(f, "unauthorized ({status})"),
            Self::RateLimited { retry_after_ms } => {
                write!(f, "rate limited (retry_after_ms={retry_after_ms})")
            }
            Self::ServerError { status } => write!(f, "server error ({status})"),
            Self::ClientError { status } => write!(f, "client error ({status})"),
            Self::Transport(msg) => write!(f, "transport error: {msg}"),
            Self::RunnerStopped(msg) => write!(f, "session runner stopped: {msg}"),
        }
    }
}

impl std::error::Error for SessionError {}

/// Normalized result of `GET /gateway` discovery or `DISCORD_GATEWAY_URL` override.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiscoveredGateway {
    pub base_url: String,
    pub gateway_url: GatewayUrl,
}

impl DiscoveredGateway {
    pub fn from_raw_url(raw: &str) -> Result<Self, SessionError> {
        let trimmed = raw.trim();
        let without_query = trimmed.split('?').next().unwrap_or_default();
        let base_url = without_query.trim_end_matches('/').to_string();
        let gateway_url = GatewayUrl::from_base(trimmed)
            .map_err(|err| SessionError::InvalidGatewayUrl(err.to_string()))?;
        Ok(Self {
            base_url,
            gateway_url,
        })
    }
}

/// Session configuration with redacted token debug formatting and environment overrides.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionConfig {
    token: String,
    pub api_base: String,
    pub gateway_url_override: Option<String>,
    pub max_reconnects: u32,
    pub initial_backoff_ms: u64,
    pub max_backoff_ms: u64,
}

impl fmt::Debug for SessionConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionConfig")
            .field("token", &REDACTED_SECRET)
            .field("api_base", &self.api_base)
            .field("gateway_url_override", &self.gateway_url_override)
            .field("max_reconnects", &self.max_reconnects)
            .field("initial_backoff_ms", &self.initial_backoff_ms)
            .field("max_backoff_ms", &self.max_backoff_ms)
            .finish()
    }
}

impl SessionConfig {
    pub fn new(token: impl Into<String>) -> Result<Self, SessionError> {
        let token = token.into();
        let trimmed = token.trim();
        if trimmed.is_empty() {
            return Err(SessionError::EmptyToken);
        }
        Ok(Self {
            token: trimmed.to_string(),
            api_base: API_V9_BASE.to_string(),
            gateway_url_override: None,
            max_reconnects: 0,
            initial_backoff_ms: 1_000,
            max_backoff_ms: 60_000,
        })
    }

    pub fn from_env_lookup<F>(token: impl Into<String>, lookup: F) -> Result<Self, SessionError>
    where
        F: Fn(&str) -> Option<String>,
    {
        let mut cfg = Self::new(token)?;
        if let Some(raw_base) = lookup(ENV_DISCORD_API_BASE) {
            let clean = raw_base.trim().trim_end_matches('/').to_string();
            if !clean.is_empty() {
                Endpoint::parse(&clean)
                    .map_err(|err| SessionError::InvalidApiBase(format!("{err:?}")))?;
                cfg.api_base = clean;
            }
        }
        if let Some(raw_gw) = lookup(ENV_DISCORD_GATEWAY_URL) {
            let clean = raw_gw.trim().trim_end_matches('/').to_string();
            if !clean.is_empty() {
                cfg.gateway_url_override = Some(clean);
            }
        }
        Ok(cfg)
    }

    pub fn from_env(token: impl Into<String>) -> Result<Self, SessionError> {
        Self::from_env_lookup(token, |key| std::env::var(key).ok())
    }

    #[must_use]
    pub fn with_api_base(mut self, api_base: impl Into<String>) -> Self {
        self.api_base = api_base.into().trim().trim_end_matches('/').to_string();
        self
    }

    #[must_use]
    pub fn with_gateway_override(mut self, gateway_url: impl Into<String>) -> Self {
        let clean = gateway_url.into().trim().trim_end_matches('/').to_string();
        self.gateway_url_override = Some(clean);
        self
    }

    #[must_use]
    pub fn with_backoff_ms(mut self, initial_ms: u64, max_ms: u64) -> Self {
        self.initial_backoff_ms = initial_ms.max(1);
        self.max_backoff_ms = max_ms.max(self.initial_backoff_ms);
        self
    }

    #[must_use]
    pub fn token(&self) -> &str {
        &self.token
    }
}

#[derive(Deserialize)]
struct GatewayDiscoveryPayload {
    url: Option<String>,
}

/// Parses and validates the HTTP response from `GET /gateway`.
pub fn parse_gateway_discovery_response(
    response: &HttpResponse,
) -> Result<DiscoveredGateway, SessionError> {
    match response.status {
        200 => {
            let parsed: GatewayDiscoveryPayload = serde_json::from_slice(&response.body)
                .map_err(|err| SessionError::InvalidDiscoveryPayload(err.to_string()))?;
            let raw_url = parsed.url.ok_or_else(|| {
                SessionError::InvalidDiscoveryPayload("missing \"url\" field".to_string())
            })?;
            DiscoveredGateway::from_raw_url(&raw_url)
        }
        401 | 403 => Err(SessionError::Unauthorized {
            status: response.status,
        }),
        429 => {
            let retry_after_ms =
                rate_limit_info(response.status, &response.headers, &response.body)
                    .map(|info| u64::try_from(info.retry_after.as_millis()).unwrap_or(u64::MAX))
                    .filter(|ms| *ms > 0)
                    .unwrap_or(DEFAULT_RATE_LIMIT_RETRY_MS);
            Err(SessionError::RateLimited { retry_after_ms })
        }
        500..=599 => Err(SessionError::ServerError {
            status: response.status,
        }),
        status => Err(SessionError::ClientError { status }),
    }
}

pub type BoxFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

/// Trait abstracting `GET /gateway` discovery for production REST and deterministic tests.
pub trait GatewayDiscoveryClient: Send + Sync {
    fn fetch_gateway<'a>(
        &'a self,
        api_base: &'a str,
        token: &'a str,
    ) -> BoxFuture<'a, Result<HttpResponse, SessionError>>;
}

/// Production REST discovery client backed by `chord_core::rest::transport`.
#[derive(Debug, Clone)]
pub struct ChordCoreDiscoveryClient {
    pub connect_timeout: Duration,
}

impl Default for ChordCoreDiscoveryClient {
    fn default() -> Self {
        Self::new()
    }
}

impl ChordCoreDiscoveryClient {
    #[must_use]
    pub fn new() -> Self {
        Self {
            connect_timeout: Duration::from_secs(10),
        }
    }
}

impl GatewayDiscoveryClient for ChordCoreDiscoveryClient {
    fn fetch_gateway<'a>(
        &'a self,
        api_base: &'a str,
        token: &'a str,
    ) -> BoxFuture<'a, Result<HttpResponse, SessionError>> {
        let mut http_cfg = HttpClientConfig::default();
        http_cfg.connect_timeout = self.connect_timeout;
        Box::pin(async move {
            let mut req = HttpRequest::new(HttpMethod::Get, GATEWAY_DISCOVERY_PATH);
            if !token.trim().is_empty() {
                req = req.header("Authorization", token.trim());
            }
            send_with(&http_cfg, api_base, &req)
                .await
                .map_err(|err| SessionError::Transport(format!("{err:?}")))
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerProgress {
    Connecting {
        attempt: u32,
        gateway_url: String,
    },
    Ready {
        session_id: Option<String>,
        gateway_url: String,
    },
    Reconnecting {
        attempt: u32,
        resumable: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RunnerExit {
    Cancelled,
    FatalClose {
        close_code: Option<u16>,
        reason: String,
    },
    ReconnectLimitReached {
        attempts: u32,
    },
}

pub struct ShutdownSignal {
    rx: watch::Receiver<bool>,
}

impl ShutdownSignal {
    #[must_use]
    pub fn new(rx: watch::Receiver<bool>) -> Self {
        Self { rx }
    }

    pub async fn wait(&mut self) {
        while !*self.rx.borrow_and_update() {
            if self.rx.changed().await.is_err() {
                break;
            }
        }
    }

    #[must_use]
    pub fn is_triggered(&self) -> bool {
        *self.rx.borrow()
    }
}

pub trait SessionRunner: Send + Sync {
    fn run_session<'a>(
        &'a self,
        config: &'a SessionConfig,
        gateway: DiscoveredGateway,
        progress_tx: mpsc::UnboundedSender<RunnerProgress>,
        shutdown: ShutdownSignal,
    ) -> BoxFuture<'a, RunnerExit>;
}

#[derive(Debug, Clone, Default)]
pub struct ChordCoreSupervisorRunner {
    pub jitter_seed: u64,
}

impl ChordCoreSupervisorRunner {
    #[must_use]
    pub fn new() -> Self {
        Self { jitter_seed: 0 }
    }
}

fn build_supervisor_config(
    config: &SessionConfig,
    gateway: GatewayUrl,
    jitter_seed: u64,
    progress_tx: mpsc::UnboundedSender<RunnerProgress>,
) -> SupervisorConfig {
    let gw_for_cb = gateway.as_str().to_string();
    let attempt_counter = Arc::new(AtomicU32::new(1));
    let mut sup_cfg = SupervisorConfig::new(gateway, config.token());
    sup_cfg.max_reconnects = config.max_reconnects;
    sup_cfg.jitter_seed = jitter_seed;
    sup_cfg.backoff = Backoff::new(config.initial_backoff_ms, config.max_backoff_ms);
    sup_cfg.with_event_handler(move |event| match event {
        SupervisorEvent::Ready => {
            let _ = progress_tx.send(RunnerProgress::Ready {
                session_id: None,
                gateway_url: gw_for_cb.clone(),
            });
        }
        SupervisorEvent::Reconnecting { .. } => {
            let attempt = attempt_counter.fetch_add(1, Ordering::SeqCst);
            let _ = progress_tx.send(RunnerProgress::Reconnecting {
                attempt,
                resumable: true,
            });
        }
        SupervisorEvent::Stopped(_) => {}
    })
}

fn map_supervisor_outcome(outcome: SupervisorOutcome) -> RunnerExit {
    match outcome {
        SupervisorOutcome::Cancelled => RunnerExit::Cancelled,
        SupervisorOutcome::Stop(StopReason::Unauthorized) => RunnerExit::FatalClose {
            close_code: Some(4004),
            reason: "Gateway close 4004: Authentication failed".to_string(),
        },
        SupervisorOutcome::Stop(StopReason::InvalidClient { code }) => RunnerExit::FatalClose {
            close_code: Some(code),
            reason: format!("Gateway fatal client close {code}"),
        },
        SupervisorOutcome::Stop(StopReason::ReconnectLimit { reconnects }) => {
            RunnerExit::ReconnectLimitReached {
                attempts: reconnects,
            }
        }
    }
}

impl SessionRunner for ChordCoreSupervisorRunner {
    fn run_session<'a>(
        &'a self,
        config: &'a SessionConfig,
        gateway: DiscoveredGateway,
        progress_tx: mpsc::UnboundedSender<RunnerProgress>,
        mut shutdown: ShutdownSignal,
    ) -> BoxFuture<'a, RunnerExit> {
        let jitter_seed = self.jitter_seed;
        Box::pin(async move {
            let gw_wire = gateway.gateway_url.as_str().to_string();
            let _ = progress_tx.send(RunnerProgress::Connecting {
                attempt: 1,
                gateway_url: gw_wire,
            });
            let sup_cfg =
                build_supervisor_config(config, gateway.gateway_url, jitter_seed, progress_tx);
            let mut supervisor = Supervisor::new(sup_cfg);
            map_supervisor_outcome(supervisor.run_until(shutdown.wait()).await)
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MockRunnerStep {
    EmitReady {
        session_id: String,
    },
    SimulateReconnect {
        attempt: u32,
        resumable: bool,
        reconnected_session_id: String,
    },
    FatalClose {
        close_code: u16,
    },
    HoldUntilShutdown,
}

#[derive(Debug, Clone, Default)]
pub struct MockSessionRunner {
    steps: Vec<MockRunnerStep>,
    connect_calls: Arc<AtomicU32>,
}

impl MockSessionRunner {
    #[must_use]
    pub fn new(steps: Vec<MockRunnerStep>) -> Self {
        Self {
            steps,
            connect_calls: Arc::new(AtomicU32::new(0)),
        }
    }

    #[must_use]
    pub fn connect_attempts(&self) -> u32 {
        self.connect_calls.load(Ordering::SeqCst)
    }
}

async fn execute_mock_step(
    step: MockRunnerStep,
    gw_wire: &str,
    connect_calls: &AtomicU32,
    progress_tx: &mpsc::UnboundedSender<RunnerProgress>,
    shutdown: &mut ShutdownSignal,
) -> Option<RunnerExit> {
    match step {
        MockRunnerStep::EmitReady { session_id } => {
            let _ = progress_tx.send(RunnerProgress::Ready {
                session_id: Some(session_id),
                gateway_url: gw_wire.to_string(),
            });
            tokio::task::yield_now().await;
            None
        }
        MockRunnerStep::SimulateReconnect {
            attempt,
            resumable,
            reconnected_session_id,
        } => {
            let _ = progress_tx.send(RunnerProgress::Reconnecting { attempt, resumable });
            tokio::task::yield_now().await;
            let next_attempt = connect_calls.fetch_add(1, Ordering::SeqCst) + 1;
            let _ = progress_tx.send(RunnerProgress::Connecting {
                attempt: next_attempt,
                gateway_url: gw_wire.to_string(),
            });
            tokio::task::yield_now().await;
            let _ = progress_tx.send(RunnerProgress::Ready {
                session_id: Some(reconnected_session_id),
                gateway_url: gw_wire.to_string(),
            });
            tokio::task::yield_now().await;
            None
        }
        MockRunnerStep::FatalClose { close_code } => {
            if let CloseAction::Stop(reason) = classify_close(Some(close_code)) {
                return Some(RunnerExit::FatalClose {
                    close_code: Some(close_code),
                    reason: format!("Fatal gateway close {close_code} ({reason:?})"),
                });
            }
            None
        }
        MockRunnerStep::HoldUntilShutdown => {
            shutdown.wait().await;
            Some(RunnerExit::Cancelled)
        }
    }
}

impl SessionRunner for MockSessionRunner {
    fn run_session<'a>(
        &'a self,
        _config: &'a SessionConfig,
        gateway: DiscoveredGateway,
        progress_tx: mpsc::UnboundedSender<RunnerProgress>,
        mut shutdown: ShutdownSignal,
    ) -> BoxFuture<'a, RunnerExit> {
        let steps = self.steps.clone();
        let connect_calls = Arc::clone(&self.connect_calls);
        Box::pin(async move {
            let gw_wire = gateway.gateway_url.as_str().to_string();
            let first_attempt = connect_calls.fetch_add(1, Ordering::SeqCst) + 1;
            let _ = progress_tx.send(RunnerProgress::Connecting {
                attempt: first_attempt,
                gateway_url: gw_wire.clone(),
            });
            tokio::task::yield_now().await;

            for step in steps {
                if shutdown.is_triggered() {
                    return RunnerExit::Cancelled;
                }
                if let Some(exit) = execute_mock_step(
                    step,
                    &gw_wire,
                    connect_calls.as_ref(),
                    &progress_tx,
                    &mut shutdown,
                )
                .await
                {
                    return exit;
                }
            }
            RunnerExit::Cancelled
        })
    }
}

pub struct SessionHandle {
    state_tx: watch::Sender<SessionState>,
    state_rx: watch::Receiver<SessionState>,
    shutdown_tx: watch::Sender<bool>,
    done_rx: Option<oneshot::Receiver<RunnerExit>>,
}

impl SessionHandle {
    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<SessionState> {
        self.state_rx.clone()
    }

    #[must_use]
    pub fn current_state(&self) -> SessionState {
        self.state_rx.borrow().clone()
    }

    pub async fn shutdown(&mut self) -> Result<RunnerExit, SessionError> {
        if !self.state_rx.borrow().is_logged_out() {
            let _ = self.state_tx.send(SessionState::ShuttingDown);
            tokio::task::yield_now().await;
        }
        let _ = self.shutdown_tx.send(true);
        if let Some(done_rx) = self.done_rx.take() {
            let exit = done_rx
                .await
                .map_err(|_| SessionError::RunnerStopped("session task dropped".to_string()))?;
            if !self.state_rx.borrow().is_logged_out() {
                let _ = self.state_tx.send(SessionState::Disconnected);
            }
            Ok(exit)
        } else {
            Ok(RunnerExit::Cancelled)
        }
    }
}

pub struct SessionManager<D, R> {
    config: SessionConfig,
    discovery: Arc<D>,
    runner: Arc<R>,
}

impl<D, R> SessionManager<D, R>
where
    D: GatewayDiscoveryClient + 'static,
    R: SessionRunner + 'static,
{
    #[must_use]
    pub fn new(config: SessionConfig, discovery: D, runner: R) -> Self {
        Self {
            config,
            discovery: Arc::new(discovery),
            runner: Arc::new(runner),
        }
    }

    #[must_use]
    pub fn config(&self) -> &SessionConfig {
        &self.config
    }

    pub async fn resolve_gateway(&self) -> Result<DiscoveredGateway, SessionError> {
        resolve_gateway_with(&self.config, self.discovery.as_ref()).await
    }

    pub fn spawn(&self) -> SessionHandle {
        let (state_tx, state_rx) = watch::channel(SessionState::Disconnected);
        let (shutdown_tx, shutdown_rx) = watch::channel(false);
        let (done_tx, done_rx) = oneshot::channel();

        let config = self.config.clone();
        let discovery = Arc::clone(&self.discovery);
        let runner = Arc::clone(&self.runner);
        let task_state_tx = state_tx.clone();

        tokio::spawn(async move {
            let exit = drive_session_lifecycle(
                config,
                discovery,
                runner,
                task_state_tx,
                ShutdownSignal::new(shutdown_rx),
            )
            .await;
            let _ = done_tx.send(exit);
        });

        SessionHandle {
            state_tx,
            state_rx,
            shutdown_tx,
            done_rx: Some(done_rx),
        }
    }
}

async fn resolve_gateway_with<D: GatewayDiscoveryClient>(
    config: &SessionConfig,
    discovery: &D,
) -> Result<DiscoveredGateway, SessionError> {
    if let Some(ref override_url) = config.gateway_url_override {
        return DiscoveredGateway::from_raw_url(override_url);
    }
    let response = discovery
        .fetch_gateway(&config.api_base, config.token())
        .await?;
    parse_gateway_discovery_response(&response)
}

fn apply_progress(state_tx: &watch::Sender<SessionState>, progress: RunnerProgress) {
    let next = match progress {
        RunnerProgress::Connecting {
            attempt,
            gateway_url,
        } => SessionState::Connecting {
            attempt,
            gateway_url,
        },
        RunnerProgress::Ready {
            session_id,
            gateway_url,
        } => SessionState::Connected {
            session_id,
            gateway_url,
        },
        RunnerProgress::Reconnecting { attempt, resumable } => {
            SessionState::Reconnecting { attempt, resumable }
        }
    };
    let _ = state_tx.send(next);
}

fn apply_terminal_exit(state_tx: &watch::Sender<SessionState>, exit: &RunnerExit) {
    match exit {
        RunnerExit::FatalClose { close_code, reason } => {
            let _ = state_tx.send(SessionState::LoggedOut {
                fatal_close_code: *close_code,
                reason: reason.clone(),
            });
        }
        RunnerExit::Cancelled | RunnerExit::ReconnectLimitReached { .. } => {
            let _ = state_tx.send(SessionState::Disconnected);
        }
    }
}

async fn drive_session_lifecycle<D, R>(
    config: SessionConfig,
    discovery: Arc<D>,
    runner: Arc<R>,
    state_tx: watch::Sender<SessionState>,
    shutdown: ShutdownSignal,
) -> RunnerExit
where
    D: GatewayDiscoveryClient,
    R: SessionRunner,
{
    let _ = state_tx.send(SessionState::DiscoveringGateway);
    tokio::task::yield_now().await;

    let gateway = match resolve_gateway_with(&config, discovery.as_ref()).await {
        Ok(gw) => gw,
        Err(SessionError::Unauthorized { status }) => {
            let reason = format!("Gateway discovery unauthorized ({status})");
            let exit = RunnerExit::FatalClose {
                close_code: None,
                reason,
            };
            apply_terminal_exit(&state_tx, &exit);
            return exit;
        }
        Err(_) => {
            let exit = RunnerExit::Cancelled;
            apply_terminal_exit(&state_tx, &exit);
            return exit;
        }
    };

    let (progress_tx, mut progress_rx) = mpsc::unbounded_channel();
    let state_tx_progress = state_tx.clone();
    let forwarder = tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            apply_progress(&state_tx_progress, progress);
        }
    });

    let exit = runner
        .run_session(&config, gateway, progress_tx, shutdown)
        .await;
    let _ = forwarder.await;
    apply_terminal_exit(&state_tx, &exit);
    exit
}
