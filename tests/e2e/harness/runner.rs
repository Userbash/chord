//! TASK-901: Архитектурный каркас нативного E2E-харнесса (`tests/e2e/harness/runner.rs`).
//!
//! Объединяет:
//! - `compositor`: изолированный композитор Wayland в ОЗУ (`wayland-99`, `llvmpipe`);
//! - `trace_loader`: загрузчик эталонного дампа `/home/sanya/re-deep/capture-ready.jsonl`;
//! - `mock_gateway`: локальный Tokio демон Discord Gateway v9 (HTTP/2 + HTTP/1.1 REST + WS);
//! - `E2eRunner`: оркестратор жизненного цикла с перехватом паник/бэктрейсов и контролем зомби/FD.

pub mod compositor;
pub mod mock_gateway;
pub mod trace_loader;

use std::fmt;
use std::fs;
use std::net::SocketAddr;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

pub use compositor::{CompositorConfig, CompositorSession};
pub use mock_gateway::{
    DaemonMetrics, Http2ClientResponse, MockGatewayConfig, MockGatewayDaemon, MockWsClient,
};
pub use trace_loader::{
    CaptureReadyTrace, DEFAULT_CAPTURE_READY_PATH, HydratedChannel, HydratedGuild, HydratedMessage,
    HydratedUser,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HarnessError {
    Io(String),
    Protocol(String),
    Timeout(String),
    Process(String),
    Hygiene(String),
}

impl fmt::Display for HarnessError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(msg) => write!(f, "IO error: {msg}"),
            Self::Protocol(msg) => write!(f, "Protocol error: {msg}"),
            Self::Timeout(msg) => write!(f, "Timeout error: {msg}"),
            Self::Process(msg) => write!(f, "Process error: {msg}"),
            Self::Hygiene(msg) => write!(f, "Hygiene error: {msg}"),
        }
    }
}

impl std::error::Error for HarnessError {}

#[derive(Debug, Clone)]
pub struct E2eRunnerConfig {
    pub compositor: CompositorConfig,
    pub gateway: MockGatewayConfig,
    pub trace_path: Option<PathBuf>,
    pub command_timeout: Duration,
}

impl Default for E2eRunnerConfig {
    fn default() -> Self {
        Self {
            compositor: CompositorConfig::default(),
            gateway: MockGatewayConfig::default(),
            trace_path: None,
            command_timeout: Duration::from_secs(5),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanicReport {
    pub message: String,
    pub location: Option<String>,
    pub backtrace: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct E2eRunResult {
    pub exit_code: Option<i32>,
    pub stdout: String,
    pub stderr: String,
    pub duration: Duration,
    pub panic_report: Option<PanicReport>,
    pub wayland_display: String,
    pub gateway_addr: SocketAddr,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResourceHygieneReport {
    pub zombie_processes: usize,
    pub lingering_children: usize,
    pub fd_before: usize,
    pub fd_after: usize,
    pub fd_leaked: usize,
    pub socket_cleaned: bool,
    pub runtime_dir_cleaned: bool,
}

fn count_open_fds() -> usize {
    fs::read_dir("/proc/self/fd").map_or(0, Iterator::count)
}

fn snapshot_fd_targets() -> std::collections::HashSet<String> {
    fs::read_dir("/proc/self/fd")
        .into_iter()
        .flatten()
        .flatten()
        .filter_map(|e| fs::read_link(e.path()).ok())
        .map(|p| p.to_string_lossy().into_owned())
        .collect()
}

fn inspect_child_processes(self_pid: u32) -> (usize, usize) {
    let Ok(entries) = fs::read_dir("/proc") else {
        return (0, 0);
    };
    let mut zombies = 0usize;
    let mut lingering = 0usize;
    let ppid_line_prefix = format!("PPid:\t{self_pid}");

    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        if !name_str.chars().all(|c| c.is_ascii_digit()) {
            continue;
        }
        let status_path = entry.path().join("status");
        let Ok(status) = fs::read_to_string(status_path) else {
            continue;
        };
        let is_our_child = status.lines().any(|l| l.trim_end() == ppid_line_prefix);
        if !is_our_child {
            continue;
        }
        let is_zombie = status
            .lines()
            .find(|l| l.starts_with("State:"))
            .is_some_and(|l| l.contains('Z'));
        if is_zombie {
            zombies += 1;
        } else {
            lingering += 1;
        }
    }
    (zombies, lingering)
}

fn collect_secret_values(extra_env: &[(&str, &str)]) -> Vec<String> {
    let mut secrets = Vec::new();
    for (k, v) in extra_env {
        let upper = k.to_ascii_uppercase();
        if upper.contains("TOKEN") || upper.contains("AUTH") || upper.contains("SECRET") {
            let trimmed = v.trim();
            if !trimmed.is_empty() {
                secrets.push(trimmed.to_string());
                if let Some(stripped) = trimmed
                    .strip_prefix("Bearer ")
                    .or_else(|| trimmed.strip_prefix("Bot "))
                    && !stripped.trim().is_empty()
                {
                    secrets.push(stripped.trim().to_string());
                }
            }
        }
    }
    secrets
}

fn mask_secrets(text: &str, secrets: &[String]) -> String {
    let mut masked = text.to_string();
    for secret in secrets {
        if !secret.is_empty() {
            masked = masked.replace(secret, "[REDACTED_TOKEN]");
        }
    }
    masked
}

fn extract_panic_report(stderr: &str) -> Option<PanicReport> {
    let lines: Vec<&str> = stderr.lines().collect();
    let panic_idx = lines.iter().position(|l| l.contains("panicked at"))?;
    let header_line = lines[panic_idx];

    let mut location = None;
    let mut inline_msg = String::new();
    if let Some(pos) = header_line.find("panicked at ") {
        let after = &header_line[pos + "panicked at ".len()..];
        if let Some(colon_end) = after.strip_suffix(':') {
            location = Some(colon_end.trim().to_string());
        } else if let Some((loc, msg)) = after.split_once(": ") {
            location = Some(loc.trim().to_string());
            inline_msg = msg.trim().to_string();
        }
    }

    let message = if !inline_msg.is_empty() {
        inline_msg
    } else {
        lines
            .get(panic_idx + 1)
            .map(|s| (*s).trim().to_string())
            .unwrap_or_else(|| header_line.to_string())
    };

    let bt_start = lines
        .iter()
        .position(|l| l.contains("stack backtrace:"))
        .unwrap_or(panic_idx);
    let backtrace = lines[bt_start..].join("\n");

    Some(PanicReport {
        message,
        location,
        backtrace,
    })
}

pub struct E2eRunner {
    config: E2eRunnerConfig,
    compositor: CompositorSession,
    daemon: MockGatewayDaemon,
    boot_elapsed: Duration,
    fd_before: usize,
    owned_fd_targets: Vec<String>,
}

impl E2eRunner {
    /// Запускает изолированный композитор Wayland и локальный Mock Gateway Daemon.
    pub async fn boot(config: E2eRunnerConfig) -> Result<Self, HarnessError> {
        let fd_before = count_open_fds();
        let boot_start = Instant::now();

        let trace = match config.trace_path.as_deref() {
            Some(p) => CaptureReadyTrace::load_from_path(p)?,
            None => CaptureReadyTrace::load_default()?,
        };
        let compositor = CompositorSession::start(config.compositor.clone())?;
        let daemon = MockGatewayDaemon::start_with_config(trace, config.gateway.clone()).await?;
        let boot_elapsed = boot_start.elapsed();
        let mut owned_fd_targets = Vec::new();
        if let Some(inode) = compositor.socket_inode() {
            owned_fd_targets.push(inode.to_string());
        }
        if let Some(inode) = daemon.listener_inode() {
            owned_fd_targets.push(inode.to_string());
        }

        Ok(Self {
            config,
            compositor,
            daemon,
            boot_elapsed,
            fd_before,
            owned_fd_targets,
        })
    }

    #[must_use]
    pub fn boot_elapsed(&self) -> Duration {
        self.boot_elapsed
    }

    #[must_use]
    pub fn compositor(&self) -> &CompositorSession {
        &self.compositor
    }

    #[must_use]
    pub fn daemon(&self) -> &MockGatewayDaemon {
        &self.daemon
    }

    fn build_binary_command(
        &self,
        bin_path: &Path,
        args: &[&str],
        extra_env: &[(&str, &str)],
    ) -> Command {
        let mut cmd = Command::new(bin_path);
        cmd.args(args)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .env_remove("DISPLAY")
            .env("RUST_BACKTRACE", "1")
            .env("DISCORD_API_BASE", self.daemon.http_base_url())
            .env("DISCORD_GATEWAY_URL", self.daemon.ws_url());
        for (k, v) in self.compositor.env_vars() {
            cmd.env(k, v);
        }
        for (k, v) in extra_env {
            cmd.env(k, v);
        }
        cmd
    }

    /// Запускает целевой бинарник в изолированном Wayland + MockGateway окружении,
    /// перехватывая stdout/stderr, маскируя токены и извлекая бэктрейс при панике.
    pub async fn run_binary(
        &mut self,
        bin_path: &Path,
        args: &[&str],
        extra_env: &[(&str, &str)],
    ) -> Result<E2eRunResult, HarnessError> {
        if !bin_path.exists() {
            return Err(HarnessError::Io(format!(
                "Бинарник не найден: {}",
                bin_path.display()
            )));
        }
        let secrets = collect_secret_values(extra_env);
        let run_start = Instant::now();
        let mut cmd = self.build_binary_command(bin_path, args, extra_env);
        let timeout_dur = self.config.command_timeout;
        let output = tokio::time::timeout(
            timeout_dur,
            tokio::task::spawn_blocking(move || cmd.output()),
        )
        .await
        .map_err(|_| HarnessError::Timeout("Превышен таймаут выполнения бинарника".into()))?
        .map_err(|e| HarnessError::Process(format!("join error: {e}")))?
        .map_err(|e| HarnessError::Io(format!("spawn error: {e}")))?;

        let stdout = mask_secrets(&String::from_utf8_lossy(&output.stdout), &secrets);
        let stderr = mask_secrets(&String::from_utf8_lossy(&output.stderr), &secrets);
        let panic_report = extract_panic_report(&stderr);

        Ok(E2eRunResult {
            exit_code: output.status.code(),
            stdout,
            stderr,
            duration: run_start.elapsed(),
            panic_report,
            wayland_display: self.compositor.wayland_display().to_string(),
            gateway_addr: self.daemon.addr(),
        })
    }

    /// Выполняет graceful shutdown демона и композитора и проверяет отсутствие зомби и утечек FD.
    pub async fn shutdown(mut self) -> Result<ResourceHygieneReport, HarnessError> {
        let sock_path = self.compositor.socket_path();
        let runtime_dir = self.compositor.runtime_dir().to_path_buf();

        self.daemon.shutdown().await?;
        self.compositor.shutdown()?;
        tokio::task::yield_now().await;

        let (zombie_processes, lingering_children) = inspect_child_processes(std::process::id());
        let fd_after = count_open_fds();
        let remaining_targets = snapshot_fd_targets();
        let fd_leaked = self
            .owned_fd_targets
            .iter()
            .filter(|t| remaining_targets.contains(*t))
            .count();
        let socket_cleaned = !sock_path.exists();
        let runtime_dir_cleaned = !runtime_dir.exists();

        Ok(ResourceHygieneReport {
            zombie_processes,
            lingering_children,
            fd_before: self.fd_before,
            fd_after,
            fd_leaked,
            socket_cleaned,
            runtime_dir_cleaned,
        })
    }
}
