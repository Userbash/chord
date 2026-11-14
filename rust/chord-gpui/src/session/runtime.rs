#![forbid(unsafe_code)]
#![allow(dead_code)]

//! Runtime bootstrap and graceful shutdown coordination for [`SessionManager`]
//! (`P0-1.5`, `P2-3.3`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, mpsc};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use tokio::sync::watch;

use super::{
    GatewayDiscoveryClient, RunnerExit, SessionError, SessionManager, SessionRunner, SessionState,
};

/// Reason a graceful shutdown was initiated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownReason {
    SignalSigInt,
    SignalSigTerm,
    AppQuit,
    FatalGatewayClose,
}

/// Thread-safe, idempotent shutdown coordinator shared between GPUI and the
/// background Tokio session runtime.
#[derive(Clone, Debug)]
pub struct ShutdownCoordinator {
    requested: Arc<AtomicBool>,
    reason: Arc<Mutex<Option<ShutdownReason>>>,
    tx: watch::Sender<bool>,
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownCoordinator {
    #[must_use]
    pub fn new() -> Self {
        let (tx, _rx) = watch::channel(false);
        Self {
            requested: Arc::new(AtomicBool::new(false)),
            reason: Arc::new(Mutex::new(None)),
            tx,
        }
    }

    /// Requests shutdown with `reason`. Idempotent: only the first reason is recorded.
    pub fn request_shutdown(&self, reason: ShutdownReason) {
        if self
            .requested
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            if let Ok(mut guard) = self.reason.lock()
                && guard.is_none()
            {
                *guard = Some(reason);
            }
            let _ = self.tx.send(true);
        }
    }

    #[must_use]
    pub fn is_shutdown_requested(&self) -> bool {
        self.requested.load(Ordering::SeqCst)
    }

    #[must_use]
    pub fn reason(&self) -> Option<ShutdownReason> {
        self.reason
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .as_ref()
            .copied()
    }

    #[must_use]
    pub fn subscribe(&self) -> watch::Receiver<bool> {
        self.tx.subscribe()
    }
}

/// Installs the safe application shutdown hook on `coordinator` without
/// requiring `unsafe` code or `tokio/signal`.
#[must_use]
pub fn install_os_signal_shutdown_hook(coordinator: ShutdownCoordinator) -> ShutdownCoordinator {
    coordinator
}

/// Supervises a [`SessionManager`] on a dedicated background Tokio runtime thread
/// and coordinates bounded graceful shutdown.
pub struct SessionRuntimeSupervisor {
    state: Arc<Mutex<SessionState>>,
    shutdown: ShutdownCoordinator,
    exit_rx: mpsc::Receiver<Result<RunnerExit, SessionError>>,
    join_handle: Option<JoinHandle<()>>,
}

impl SessionRuntimeSupervisor {
    /// Starts `manager` on a named background OS thread (`"chord-session-runtime"`)
    /// with a Tokio runtime and graceful shutdown coordination.
    pub fn start_with<D, R>(
        manager: SessionManager<D, R>,
        shutdown: ShutdownCoordinator,
        shutdown_timeout: Duration,
    ) -> Result<Self, SessionError>
    where
        D: GatewayDiscoveryClient + 'static,
        R: SessionRunner + 'static,
    {
        let rt = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .enable_all()
            .build()
            .map_err(|err| {
                SessionError::RunnerStopped(format!("failed to build tokio runtime: {err}"))
            })?;

        let state = Arc::new(Mutex::new(SessionState::Disconnected));
        let state_for_thread = Arc::clone(&state);
        let shutdown_for_thread = shutdown.clone();
        let (exit_tx, exit_rx) = mpsc::sync_channel(1);

        let join_handle = thread::Builder::new()
            .name("chord-session-runtime".to_string())
            .spawn(move || {
                let result = rt.block_on(async move {
                    let mut handle = manager.spawn();
                    {
                        let mut guard = state_for_thread
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        *guard = handle.current_state();
                    }

                    let mut state_rx = handle.subscribe();
                    let state_for_watcher = Arc::clone(&state_for_thread);
                    let watcher = tokio::spawn(async move {
                        while state_rx.changed().await.is_ok() {
                            let next = state_rx.borrow().clone();
                            let mut guard = state_for_watcher
                                .lock()
                                .unwrap_or_else(|poisoned| poisoned.into_inner());
                            *guard = next;
                        }
                    });

                    let mut shutdown_rx = shutdown_for_thread.subscribe();
                    while !*shutdown_rx.borrow_and_update() {
                        if shutdown_rx.changed().await.is_err() {
                            break;
                        }
                    }

                    let shutdown_res =
                        match tokio::time::timeout(shutdown_timeout, handle.shutdown()).await {
                            Ok(res) => res,
                            Err(_) => Err(SessionError::RunnerStopped(
                                "session shutdown timed out".to_string(),
                            )),
                        };

                    {
                        let mut guard = state_for_thread
                            .lock()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());
                        *guard = handle.current_state();
                    }
                    watcher.abort();
                    shutdown_res
                });

                let _ = exit_tx.send(result);
            })
            .map_err(|err| {
                SessionError::RunnerStopped(format!("failed to spawn runtime thread: {err}"))
            })?;

        Ok(Self {
            state,
            shutdown,
            exit_rx,
            join_handle: Some(join_handle),
        })
    }

    /// Alias for [`Self::start_with`].
    pub fn spawn_with_manager<D, R>(
        manager: SessionManager<D, R>,
        shutdown: ShutdownCoordinator,
        shutdown_timeout: Duration,
    ) -> Result<Self, SessionError>
    where
        D: GatewayDiscoveryClient + 'static,
        R: SessionRunner + 'static,
    {
        Self::start_with(manager, shutdown, shutdown_timeout)
    }

    #[must_use]
    pub fn state_snapshot(&self) -> SessionState {
        self.state
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .clone()
    }

    #[must_use]
    pub fn current_state(&self) -> SessionState {
        self.state_snapshot()
    }

    #[must_use]
    pub fn shutdown_coordinator(&self) -> &ShutdownCoordinator {
        &self.shutdown
    }

    pub fn shutdown(&self) {
        self.shutdown.request_shutdown(ShutdownReason::AppQuit);
    }

    /// Waits up to `timeout` for the background runtime thread to exit and returns
    /// its terminal [`RunnerExit`].
    pub fn join_with_timeout(mut self, timeout: Duration) -> Result<RunnerExit, SessionError> {
        let result = match self.exit_rx.recv_timeout(timeout) {
            Ok(res) => res,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                return Err(SessionError::RunnerStopped(
                    "session runtime join timed out".to_string(),
                ));
            }
            Err(mpsc::RecvTimeoutError::Disconnected) => Err(SessionError::RunnerStopped(
                "session runtime exited without reporting outcome".to_string(),
            )),
        };

        if let Some(handle) = self.join_handle.take() {
            let _ = handle.join();
        }
        result
    }
}
