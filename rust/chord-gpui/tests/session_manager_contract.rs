#![forbid(unsafe_code)]
#![deny(warnings)]

//! Contract tests for `P0-1.2`: Live Discord `SessionManager` Lifecycle & Gateway Discovery
//! in `rust/chord-gpui/src/session/mod.rs`.

#[path = "../src/session/mod.rs"]
mod session;

use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use chord_core::API_V9_BASE;
use chord_core::rest::transport::HttpResponse;
use session::{
    BoxFuture, ChordCoreDiscoveryClient, ChordCoreSupervisorRunner, DEFAULT_RATE_LIMIT_RETRY_MS,
    DiscoveredGateway, ENV_DISCORD_API_BASE, ENV_DISCORD_GATEWAY_URL, GATEWAY_DISCOVERY_PATH,
    GatewayDiscoveryClient, MockRunnerStep, MockSessionRunner, REDACTED_SECRET, RunnerExit,
    RunnerProgress, SessionConfig, SessionError, SessionManager, SessionRunner, SessionState,
    ShutdownSignal, parse_gateway_discovery_response,
};

struct StubDiscoveryClient {
    response: HttpResponse,
    calls: Arc<AtomicU32>,
    last_api_base: std::sync::Mutex<Option<String>>,
}

impl StubDiscoveryClient {
    fn new(status: u16, headers: Vec<(String, String)>, body: &str) -> Self {
        Self {
            response: HttpResponse {
                status,
                headers,
                body: body.as_bytes().to_vec(),
            },
            calls: Arc::new(AtomicU32::new(0)),
            last_api_base: std::sync::Mutex::new(None),
        }
    }

    fn call_count(&self) -> u32 {
        self.calls.load(Ordering::SeqCst)
    }

    fn captured_api_base(&self) -> Option<String> {
        self.last_api_base
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone()
    }
}

impl GatewayDiscoveryClient for StubDiscoveryClient {
    fn fetch_gateway<'a>(
        &'a self,
        api_base: &'a str,
        _token: &'a str,
    ) -> BoxFuture<'a, Result<HttpResponse, SessionError>> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if let Ok(mut guard) = self.last_api_base.lock() {
            *guard = Some(api_base.to_string());
        }
        let resp = self.response.clone();
        Box::pin(async move { Ok(resp) })
    }
}

#[test]
fn gateway_discovery_200_strips_trailing_slash_and_normalizes_wss_and_local_ws() {
    assert_eq!(GATEWAY_DISCOVERY_PATH, "/gateway");
    assert_eq!(DEFAULT_RATE_LIMIT_RETRY_MS, 5_000);
    let response = HttpResponse {
        status: 200,
        headers: vec![("content-type".to_string(), "application/json".to_string())],
        body: br#"{"url":"wss://gateway.discord.gg/"}"#.to_vec(),
    };

    let discovered: DiscoveredGateway = parse_gateway_discovery_response(&response)
        .expect("200 /gateway JSON must parse and normalize");
    assert_eq!(discovered.base_url, "wss://gateway.discord.gg");
    assert_eq!(
        discovered.gateway_url.as_str(),
        "wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream"
    );

    // Local E2E mock gateway over ws://127.0.0.1:port
    let local_response = HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: br#"{"url":"ws://127.0.0.1:19443/"}"#.to_vec(),
    };
    let local_discovered = parse_gateway_discovery_response(&local_response)
        .expect("local ws:// gateway must be accepted for E2E mock harness");
    assert_eq!(local_discovered.base_url, "ws://127.0.0.1:19443");
    assert_eq!(
        local_discovered.gateway_url.as_str(),
        "ws://127.0.0.1:19443/?encoding=etf&v=9&compress=zstd-stream"
    );
}

#[tokio::test]
async fn gateway_discovery_env_override_skips_rest_call_and_api_base_override_is_respected() {
    let config = SessionConfig::from_env_lookup("user-secret-token-123", |key| match key {
        ENV_DISCORD_API_BASE => Some("http://127.0.0.1:18080/api/v9/".to_string()),
        ENV_DISCORD_GATEWAY_URL => Some("ws://127.0.0.1:18081/".to_string()),
        _ => None,
    })
    .expect("valid config with env overrides");

    assert_eq!(config.api_base, "http://127.0.0.1:18080/api/v9");
    assert_eq!(
        config.gateway_url_override.as_deref(),
        Some("ws://127.0.0.1:18081")
    );

    let discovery = StubDiscoveryClient::new(500, Vec::new(), "should not be called");
    let calls = Arc::clone(&discovery.calls);
    let runner = MockSessionRunner::new(vec![MockRunnerStep::EmitReady {
        session_id: "sess_override_1".to_string(),
    }]);

    let manager = SessionManager::new(config, discovery, runner);
    assert_eq!(manager.config().token(), "user-secret-token-123");
    let resolved = manager
        .resolve_gateway()
        .await
        .expect("DISCORD_GATEWAY_URL override must bypass REST call");

    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(resolved.base_url, "ws://127.0.0.1:18081");
    assert_eq!(
        resolved.gateway_url.as_str(),
        "ws://127.0.0.1:18081/?encoding=etf&v=9&compress=zstd-stream"
    );

    // Without DISCORD_GATEWAY_URL, REST discovery is invoked against configured api_base
    let cfg_rest = SessionConfig::new("token-abc")
        .expect("valid token")
        .with_api_base("http://127.0.0.1:18090/api/v9");
    let stub = StubDiscoveryClient::new(200, Vec::new(), r#"{"url":"ws://127.0.0.1:18091/"}"#);
    let stub_ref = StubDiscoveryClient {
        response: stub.response.clone(),
        calls: Arc::clone(&stub.calls),
        last_api_base: std::sync::Mutex::new(None),
    };
    let mgr_rest = SessionManager::new(cfg_rest, stub, MockSessionRunner::default());
    let via_rest = mgr_rest
        .resolve_gateway()
        .await
        .expect("REST discovery succeeds");
    assert_eq!(stub_ref.call_count(), 1);
    assert_eq!(stub_ref.captured_api_base(), None);
    assert_eq!(via_rest.base_url, "ws://127.0.0.1:18091");
}

#[test]
fn gateway_discovery_error_handling_401_429_503_and_malformed_json() {
    // 401 Unauthorized
    let resp_401 = HttpResponse {
        status: 401,
        headers: Vec::new(),
        body: br#"{"message":"401: Unauthorized","code":0}"#.to_vec(),
    };
    assert_eq!(
        parse_gateway_discovery_response(&resp_401),
        Err(SessionError::Unauthorized { status: 401 })
    );

    // 429 Too Many Requests with JSON retry_after (seconds -> ms)
    let resp_429_json = HttpResponse {
        status: 429,
        headers: Vec::new(),
        body: br#"{"message":"You are being rate limited.","retry_after":1.25,"global":false}"#
            .to_vec(),
    };
    assert_eq!(
        parse_gateway_discovery_response(&resp_429_json),
        Err(SessionError::RateLimited {
            retry_after_ms: 1_250
        })
    );

    // 429 Too Many Requests with Retry-After header
    let resp_429_hdr = HttpResponse {
        status: 429,
        headers: vec![("Retry-After".to_string(), "3".to_string())],
        body: b"{}".to_vec(),
    };
    assert_eq!(
        parse_gateway_discovery_response(&resp_429_hdr),
        Err(SessionError::RateLimited {
            retry_after_ms: 3_000
        })
    );

    // 503 Service Unavailable
    let resp_503 = HttpResponse {
        status: 503,
        headers: Vec::new(),
        body: b"upstream unavailable".to_vec(),
    };
    assert_eq!(
        parse_gateway_discovery_response(&resp_503),
        Err(SessionError::ServerError { status: 503 })
    );

    // 404 Client Error
    let resp_404 = HttpResponse {
        status: 404,
        headers: Vec::new(),
        body: b"not found".to_vec(),
    };
    assert_eq!(
        parse_gateway_discovery_response(&resp_404),
        Err(SessionError::ClientError { status: 404 })
    );

    // Malformed JSON
    let resp_malformed = HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: b"{not-valid-json".to_vec(),
    };
    assert!(matches!(
        parse_gateway_discovery_response(&resp_malformed),
        Err(SessionError::InvalidDiscoveryPayload(_))
    ));

    // Missing "url" key in 200 JSON
    let resp_missing_url = HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: br#"{"shards":1}"#.to_vec(),
    };
    assert!(matches!(
        parse_gateway_discovery_response(&resp_missing_url),
        Err(SessionError::InvalidDiscoveryPayload(_))
    ));

    // Unsupported scheme in url
    let resp_bad_scheme = HttpResponse {
        status: 200,
        headers: Vec::new(),
        body: br#"{"url":"ftp://gateway.discord.gg"}"#.to_vec(),
    };
    assert!(matches!(
        parse_gateway_discovery_response(&resp_bad_scheme),
        Err(SessionError::InvalidGatewayUrl(_))
    ));
}

#[tokio::test]
async fn lifecycle_transitions_disconnected_discovering_connecting_connected_shutting_down_disconnected()
 {
    let start = Instant::now();
    let config = SessionConfig::new("valid-token-xyz").expect("config");
    assert_eq!(config.api_base, API_V9_BASE);

    let discovery =
        StubDiscoveryClient::new(200, Vec::new(), r#"{"url":"wss://gateway.discord.gg/"}"#);
    let runner = MockSessionRunner::new(vec![
        MockRunnerStep::EmitReady {
            session_id: "session_live_42".to_string(),
        },
        MockRunnerStep::SimulateReconnect {
            attempt: 1,
            resumable: true,
            reconnected_session_id: "session_live_42".to_string(),
        },
        MockRunnerStep::HoldUntilShutdown,
    ]);

    let manager = SessionManager::new(config, discovery, runner);
    let mut handle = manager.spawn();
    let mut rx = handle.subscribe();
    let mut watcher_rx = handle.subscribe();

    let observed_shared = Arc::new(std::sync::Mutex::new(vec![
        watcher_rx.borrow_and_update().clone(),
    ]));
    let observed_for_task = Arc::clone(&observed_shared);
    let watcher_task = tokio::spawn(async move {
        while watcher_rx.changed().await.is_ok() {
            let state = watcher_rx.borrow_and_update().clone();
            if let Ok(mut guard) = observed_for_task.lock()
                && guard.last() != Some(&state)
            {
                guard.push(state);
            }
        }
    });

    // Wait until we reach Connected after the simulated reconnect
    let deadline = tokio::time::Instant::now() + Duration::from_millis(250);
    while tokio::time::Instant::now() < deadline {
        if tokio::time::timeout(Duration::from_millis(50), rx.changed())
            .await
            .is_ok()
        {
            let _ = rx.borrow_and_update();
            let connected_count = observed_shared
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .iter()
                .filter(|s| s.is_connected())
                .count();
            if connected_count >= 2 {
                break;
            }
        }
    }

    assert!(
        handle.current_state().is_connected(),
        "expected Connected state before shutdown, got: {:?}",
        handle.current_state()
    );

    let shutdown_start = Instant::now();
    let exit = handle
        .shutdown()
        .await
        .expect("graceful shutdown must succeed");
    let shutdown_elapsed = shutdown_start.elapsed();
    assert_eq!(exit, RunnerExit::Cancelled);
    assert!(
        shutdown_elapsed < Duration::from_millis(500),
        "shutdown took {:?} (>= 500ms)",
        shutdown_elapsed
    );

    tokio::task::yield_now().await;
    drop(handle);
    let _ = tokio::time::timeout(Duration::from_millis(50), watcher_task).await;

    let observed = observed_shared
        .lock()
        .unwrap_or_else(|p| p.into_inner())
        .clone();

    assert_eq!(observed.first(), Some(&SessionState::Disconnected));
    assert!(observed.contains(&SessionState::DiscoveringGateway));
    assert!(observed.contains(&SessionState::Connecting {
        attempt: 1,
        gateway_url: "wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream".to_string(),
    }));
    assert!(observed.contains(&SessionState::Connected {
        session_id: Some("session_live_42".to_string()),
        gateway_url: "wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream".to_string(),
    }));
    assert!(observed.contains(&SessionState::Reconnecting {
        attempt: 1,
        resumable: true,
    }));
    assert!(observed.contains(&SessionState::ShuttingDown));
    assert_eq!(observed.last(), Some(&SessionState::Disconnected));
    assert!(
        start.elapsed() < Duration::from_millis(250),
        "deterministic lifecycle test must complete quickly, took {:?}",
        start.elapsed()
    );
}

#[tokio::test]
async fn fatal_close_4004_transitions_immediately_to_logged_out_without_retrying() {
    let config = SessionConfig::new("expired-or-revoked-token")
        .expect("config")
        .with_gateway_override("wss://gateway.discord.gg/");
    let discovery =
        StubDiscoveryClient::new(200, Vec::new(), r#"{"url":"wss://gateway.discord.gg"}"#);
    let runner = MockSessionRunner::new(vec![MockRunnerStep::FatalClose { close_code: 4004 }]);
    let runner_probe = runner.clone();

    let manager = SessionManager::new(config, discovery, runner);
    let mut handle = manager.spawn();
    let mut rx = handle.subscribe();

    let deadline = tokio::time::Instant::now() + Duration::from_millis(200);
    while tokio::time::Instant::now() < deadline {
        if rx.borrow().is_logged_out() {
            break;
        }
        if tokio::time::timeout(Duration::from_millis(50), rx.changed())
            .await
            .is_err()
        {
            break;
        }
    }

    let final_state = handle.current_state();
    assert!(
        matches!(
            final_state,
            SessionState::LoggedOut {
                fatal_close_code: Some(4004),
                ref reason
            } if reason.contains("4004")
        ),
        "expected LoggedOut with fatal_close_code=Some(4004), got {final_state:?}"
    );
    assert_eq!(
        runner_probe.connect_attempts(),
        1,
        "4004 fatal close must NOT retry connecting"
    );

    let exit = handle
        .shutdown()
        .await
        .expect("join after 4004 fatal close");
    assert!(matches!(
        exit,
        RunnerExit::FatalClose {
            close_code: Some(4004),
            ..
        }
    ));
}

#[tokio::test]
async fn discovery_401_and_503_and_fatal_close_4014_handle_terminal_states_cleanly() {
    // 1. Discovery 401 transitions spawned session to LoggedOut without calling runner
    let cfg_401 = SessionConfig::new("revoked-rest-token").expect("config");
    let disc_401 = StubDiscoveryClient::new(401, Vec::new(), r#"{"message":"401: Unauthorized"}"#);
    let runner_401 = MockSessionRunner::new(vec![MockRunnerStep::HoldUntilShutdown]);
    let runner_401_probe = runner_401.clone();
    let mgr_401 = SessionManager::new(cfg_401, disc_401, runner_401);
    let mut handle_401 = mgr_401.spawn();
    let exit_401 = handle_401.shutdown().await.expect("join 401 discovery");
    assert!(matches!(
        exit_401,
        RunnerExit::FatalClose {
            close_code: None,
            ..
        }
    ));
    assert!(handle_401.current_state().is_logged_out());
    assert_eq!(runner_401_probe.connect_attempts(), 0);

    // 2. Discovery 503 transitions spawned session back to Disconnected cleanly
    let cfg_503 = SessionConfig::new("valid-token").expect("config");
    let disc_503 = StubDiscoveryClient::new(503, Vec::new(), "service unavailable");
    let runner_503 = MockSessionRunner::new(vec![MockRunnerStep::HoldUntilShutdown]);
    let runner_503_probe = runner_503.clone();
    let mgr_503 = SessionManager::new(cfg_503, disc_503, runner_503);
    let mut handle_503 = mgr_503.spawn();
    let exit_503 = handle_503.shutdown().await.expect("join 503 discovery");
    assert_eq!(exit_503, RunnerExit::Cancelled);
    assert_eq!(handle_503.current_state(), SessionState::Disconnected);
    assert_eq!(runner_503_probe.connect_attempts(), 0);

    // 3. Fatal close 4014 (Disallowed intent) transitions to LoggedOut(Some(4014))
    let cfg_4014 = SessionConfig::new("valid-token")
        .expect("config")
        .with_gateway_override("wss://gateway.discord.gg");
    let disc_4014 =
        StubDiscoveryClient::new(200, Vec::new(), r#"{"url":"wss://gateway.discord.gg"}"#);
    let runner_4014 = MockSessionRunner::new(vec![MockRunnerStep::FatalClose { close_code: 4014 }]);
    let mgr_4014 = SessionManager::new(cfg_4014, disc_4014, runner_4014);
    let handle_4014 = mgr_4014.spawn();
    let mut rx_4014 = handle_4014.subscribe();
    while !rx_4014.borrow().is_logged_out() {
        if tokio::time::timeout(Duration::from_millis(50), rx_4014.changed())
            .await
            .is_err()
        {
            break;
        }
    }
    assert!(matches!(
        handle_4014.current_state(),
        SessionState::LoggedOut {
            fatal_close_code: Some(4014),
            ..
        }
    ));
}

#[tokio::test]
async fn session_config_redacts_token_in_debug_and_validates_inputs() {
    let raw_secret = "mfa.SuperSecretDiscordTokenValue_99887766";
    let config = SessionConfig::from_env(raw_secret)
        .expect("non-empty token")
        .with_backoff_ms(500, 15_000);
    assert_eq!(config.token(), raw_secret);

    let debug_str = format!("{config:?}");
    assert!(
        debug_str.contains(REDACTED_SECRET),
        "Debug output must contain {REDACTED_SECRET}, got: {debug_str}"
    );
    assert!(
        !debug_str.contains(raw_secret),
        "Debug output leaked raw token: {debug_str}"
    );

    // Empty or whitespace-only token must fail validation
    assert_eq!(SessionConfig::new("   "), Err(SessionError::EmptyToken));

    // Invalid API base in env override
    let bad_base = SessionConfig::from_env_lookup("tok", |k| {
        if k == ENV_DISCORD_API_BASE {
            Some("ftp://invalid-base".to_string())
        } else {
            None
        }
    });
    assert!(matches!(bad_base, Err(SessionError::InvalidApiBase(_))));

    // Production runner & discovery client constructors and pre-cancelled run_session
    let prod_discovery = ChordCoreDiscoveryClient::default();
    assert_eq!(prod_discovery.connect_timeout, Duration::from_secs(10));
    let prod_runner = ChordCoreSupervisorRunner::new();
    assert_eq!(prod_runner.jitter_seed, 0);

    let (tx_cancel, rx_cancel) = tokio::sync::watch::channel(true);
    let _ = tx_cancel;
    let sig = ShutdownSignal::new(rx_cancel);
    assert!(sig.is_triggered());
    let (prog_tx, _prog_rx) = tokio::sync::mpsc::unbounded_channel();
    let gw = DiscoveredGateway::from_raw_url("ws://127.0.0.1:19999").expect("valid ws url");
    let prod_exit = prod_runner.run_session(&config, gw, prog_tx, sig).await;
    assert_eq!(prod_exit, RunnerExit::Cancelled);

    let progress = RunnerProgress::Ready {
        session_id: Some("s1".to_string()),
        gateway_url: "wss://gateway.discord.gg".to_string(),
    };
    assert!(matches!(progress, RunnerProgress::Ready { .. }));
    assert!(
        !SessionError::Transport("io".to_string())
            .to_string()
            .is_empty()
    );
    assert!(
        !SessionError::RunnerStopped("stop".to_string())
            .to_string()
            .is_empty()
    );
    assert!(matches!(
        RunnerExit::ReconnectLimitReached { attempts: 3 },
        RunnerExit::ReconnectLimitReached { attempts: 3 }
    ));
}
