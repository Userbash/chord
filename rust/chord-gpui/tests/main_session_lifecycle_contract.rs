#![forbid(unsafe_code)]
#![deny(warnings)]

//! Contract tests for `P0-1.5` & `P2-3.3`:
//! `SessionManager` Runtime Bootstrap, Module Exports & Graceful `SIGTERM`/`SIGINT` Shutdown.

#[allow(dead_code)]
#[path = "../src/auth/mod.rs"]
mod auth;
#[allow(dead_code)]
#[path = "../src/session/mod.rs"]
mod session;

use std::time::{Duration, Instant};

use chord_core::rest::transport::HttpResponse;
use session::runtime::{SessionRuntimeSupervisor, ShutdownCoordinator, ShutdownReason};
use session::{
    BoxFuture, GatewayDiscoveryClient, MockRunnerStep, MockSessionRunner, SessionConfig,
    SessionError, SessionManager, SessionState,
};

struct StubDiscoveryClient {
    gateway_url: String,
}

impl StubDiscoveryClient {
    fn new(gateway_url: impl Into<String>) -> Self {
        Self {
            gateway_url: gateway_url.into(),
        }
    }
}

impl GatewayDiscoveryClient for StubDiscoveryClient {
    fn fetch_gateway<'a>(
        &'a self,
        _api_base: &'a str,
        _token: &'a str,
    ) -> BoxFuture<'a, Result<HttpResponse, SessionError>> {
        let body = format!(r#"{{"url":"{}"}}"#, self.gateway_url);
        Box::pin(async move {
            Ok(HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: body.into_bytes(),
            })
        })
    }
}

#[test]
fn auth_submodules_are_publicly_exported_and_accessible() {
    assert_eq!(auth::login_service::AUTH_LOGIN_PATH, "/api/v9/auth/login");
    assert_eq!(
        auth::remote_auth::REMOTE_AUTH_LOGIN_PATH,
        "/api/v9/users/@me/remote-auth/login"
    );
    assert_eq!(auth::token_store::DEFAULT_TOKEN_KEY, "chord.active_account");
    assert_eq!(auth::totp_dialog::TOTP_DIGIT_COUNT, 6);
}

#[test]
fn shutdown_coordinator_is_atomic_idempotent_and_notifies_subscribers() {
    let coordinator = ShutdownCoordinator::new();
    assert!(!coordinator.is_shutdown_requested());
    assert_eq!(coordinator.reason(), None);

    let mut sub_a = coordinator.subscribe();
    let mut sub_b = coordinator.subscribe();
    assert!(!*sub_a.borrow());
    assert!(!*sub_b.borrow());

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_time()
        .build()
        .expect("tokio runtime");

    rt.block_on(async {
        coordinator.request_shutdown(ShutdownReason::SignalSigTerm);
        assert!(coordinator.is_shutdown_requested());
        assert_eq!(coordinator.reason(), Some(ShutdownReason::SignalSigTerm));

        // Duplicate calls must keep the first recorded reason.
        coordinator.request_shutdown(ShutdownReason::SignalSigInt);
        coordinator.request_shutdown(ShutdownReason::AppQuit);
        coordinator.request_shutdown(ShutdownReason::FatalGatewayClose);
        assert_eq!(coordinator.reason(), Some(ShutdownReason::SignalSigTerm));

        tokio::time::timeout(Duration::from_millis(200), sub_a.changed())
            .await
            .expect("sub_a notified in time")
            .expect("sub_a channel open");
        tokio::time::timeout(Duration::from_millis(200), sub_b.changed())
            .await
            .expect("sub_b notified in time")
            .expect("sub_b channel open");

        assert!(*sub_a.borrow());
        assert!(*sub_b.borrow());
    });
}

fn wait_for_connected_state(supervisor: &SessionRuntimeSupervisor, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    let mut saw_connecting = false;
    while Instant::now() < deadline {
        match supervisor.state_snapshot() {
            SessionState::Connecting { .. } => {
                saw_connecting = true;
            }
            SessionState::Connected { session_id, .. } => {
                assert_eq!(session_id.as_deref(), Some("sess-runtime-1"));
                let _ = saw_connecting;
                return;
            }
            _ => {}
        }
        std::thread::sleep(Duration::from_millis(10));
    }
    panic!(
        "timed out waiting for Connected state; last state = {:?}",
        supervisor.state_snapshot()
    );
}

#[test]
fn runtime_supervisor_reaches_connected_and_exits_cleanly_on_sigterm_and_sigint() {
    for reason in [ShutdownReason::SignalSigTerm, ShutdownReason::SignalSigInt] {
        let config = SessionConfig::new("token-runtime-test")
            .expect("valid token")
            .with_api_base("http://127.0.0.1:19444");
        let discovery = StubDiscoveryClient::new("ws://127.0.0.1:19445/");
        let runner = MockSessionRunner::new(vec![
            MockRunnerStep::EmitReady {
                session_id: "sess-runtime-1".to_string(),
            },
            MockRunnerStep::HoldUntilShutdown,
        ]);
        let manager = SessionManager::new(config, discovery, runner);
        let coordinator = ShutdownCoordinator::new();

        let supervisor = SessionRuntimeSupervisor::start_with(
            manager,
            coordinator.clone(),
            Duration::from_millis(500),
        )
        .expect("supervisor starts");

        wait_for_connected_state(&supervisor, Duration::from_secs(2));

        let shutdown_started = Instant::now();
        supervisor.shutdown_coordinator().request_shutdown(reason);
        let exit = supervisor
            .join_with_timeout(Duration::from_secs(2))
            .expect("supervisor joins cleanly");

        let elapsed = shutdown_started.elapsed();
        assert!(
            elapsed < Duration::from_millis(500),
            "shutdown took {elapsed:?}, expected < 500ms for {reason:?}"
        );
        assert_eq!(exit, session::RunnerExit::Cancelled);
        assert_eq!(coordinator.reason(), Some(reason));
    }
}
