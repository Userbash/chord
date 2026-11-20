#![forbid(unsafe_code)]

//! Global shutdown coordination, signal interception, and secret redaction (STAGE-02.2.1, STAGE-02.2.2, STAGE-02.3.1, STAGE-02.3.2).
//!
//! Provides [`ShutdownCoordinator`] for cascading graceful shutdown with a default
//! 5-second timeout via `tokio::task::JoinSet`, signal interception for `SIGINT` / `SIGTERM`,
//! and [`RedactingWriter`] to prevent Bot/Bearer tokens and encryption keys from leaking to stdout.

use std::fmt;
use std::future::Future;
use std::io::Write;
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use regex::Regex;
use tokio::task::JoinSet;
use tokio_util::sync::CancellationToken;

/// Default shutdown timeout: 5 seconds (STAGE-02.2.2).
pub const DEFAULT_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(5);

/// Shutdown error kinds for task joining, timeouts, and signal listeners.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ShutdownError {
    /// Timed out waiting for registered tasks to complete within duration.
    Timeout { elapsed: Duration },
    /// Signal listening error.
    Signal(String),
    /// Background task join error.
    Join(String),
}

impl fmt::Display for ShutdownError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Timeout { elapsed } => {
                write!(f, "Shutdown timed out after {:?}", elapsed)
            }
            Self::Signal(msg) => write!(f, "Signal listener error: {msg}"),
            Self::Join(msg) => write!(f, "Task join error: {msg}"),
        }
    }
}

impl std::error::Error for ShutdownError {}

/// Intercepted operating system signal or manual trigger initiating shutdown.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShutdownSignal {
    /// Interrupt signal (Ctrl-C / SIGINT).
    Sigint,
    /// Termination signal (SIGTERM).
    Sigterm,
    /// Manual programmatic shutdown trigger.
    Manual,
}

impl ShutdownSignal {
    /// Returns the canonical uppercase string representation of the signal.
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Sigint => "SIGINT",
            Self::Sigterm => "SIGTERM",
            Self::Manual => "MANUAL",
        }
    }

    /// Returns `true` if this was an operating system signal rather than manual trigger.
    pub fn is_signal(&self) -> bool {
        matches!(self, Self::Sigint | Self::Sigterm)
    }
}

/// Global shutdown coordinator wrapping a root [`CancellationToken`] and [`JoinSet`].
#[derive(Debug, Clone)]
pub struct ShutdownCoordinator {
    root_token: CancellationToken,
    timeout: Duration,
    tasks: Arc<Mutex<Option<JoinSet<()>>>>,
}

impl Default for ShutdownCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

impl ShutdownCoordinator {
    /// Creates a new [`ShutdownCoordinator`] with default 5-second timeout.
    pub fn new() -> Self {
        Self {
            root_token: CancellationToken::new(),
            timeout: DEFAULT_SHUTDOWN_TIMEOUT,
            tasks: Arc::new(Mutex::new(Some(JoinSet::new()))),
        }
    }

    /// Sets a custom timeout for task completion before aborting.
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Returns the configured timeout duration for graceful task shutdown.
    pub fn timeout(&self) -> Duration {
        self.timeout
    }

    /// Returns a reference to the underlying root [`CancellationToken`].
    pub fn token(&self) -> &CancellationToken {
        &self.root_token
    }

    /// Creates a child [`CancellationToken`] linked to the root token.
    pub fn child_token(&self) -> CancellationToken {
        self.root_token.child_token()
    }

    /// Triggers cascading shutdown by cancelling the root token.
    pub fn shutdown(&self) {
        self.root_token.cancel();
    }

    /// Returns `true` if shutdown has already been triggered.
    pub fn is_cancelled(&self) -> bool {
        self.root_token.is_cancelled()
    }

    /// Completes when shutdown is triggered.
    pub async fn cancelled(&self) {
        self.root_token.cancelled().await;
    }

    /// Registers and spawns a background child task into the internal [`JoinSet`].
    pub fn spawn_task<F>(&self, future: F)
    where
        F: Future<Output = ()> + Send + 'static,
    {
        if let Ok(mut guard) = self.tasks.lock() {
            if let Some(set) = guard.as_mut() {
                set.spawn(future);
            }
        }
    }

    /// Returns the number of active tasks currently tracked in the [`JoinSet`].
    pub fn task_count(&self) -> usize {
        match self.tasks.lock() {
            Ok(guard) => match guard.as_ref() {
                Some(set) => set.len(),
                None => 0,
            },
            Err(_) => 0,
        }
    }

    /// Triggers shutdown and waits up to `self.timeout` for all registered tasks to complete.
    pub async fn shutdown_and_join(&self) -> Result<(), ShutdownError> {
        self.shutdown();
        let mut set = match self.tasks.lock() {
            Ok(mut guard) => match guard.take() {
                Some(s) => s,
                None => return Ok(()),
            },
            Err(_) => return Err(ShutdownError::Join("Mutex poisoned".into())),
        };

        let join_all = async {
            while let Some(res) = set.join_next().await {
                if let Err(e) = res {
                    if !e.is_cancelled() {
                        return Err(ShutdownError::Join(e.to_string()));
                    }
                }
            }
            Ok(())
        };

        match tokio::time::timeout(self.timeout, join_all).await {
            Ok(res) => res,
            Err(_) => {
                set.abort_all();
                while set.join_next().await.is_some() {}
                Err(ShutdownError::Timeout {
                    elapsed: self.timeout,
                })
            }
        }
    }

    /// Asynchronously waits for an OS termination signal (`SIGINT` or `SIGTERM`).
    #[cfg(unix)]
    pub async fn wait_for_signal() -> Result<ShutdownSignal, ShutdownError> {
        use tokio::signal::unix::{signal, SignalKind};
        let mut sigterm = signal(SignalKind::terminate())
            .map_err(|e| ShutdownError::Signal(e.to_string()))?;
        tokio::select! {
            res = tokio::signal::ctrl_c() => {
                res.map_err(|e| ShutdownError::Signal(e.to_string()))?;
                Ok(ShutdownSignal::Sigint)
            }
            _ = sigterm.recv() => {
                Ok(ShutdownSignal::Sigterm)
            }
        }
    }

    /// Asynchronously waits for an OS termination signal (`SIGINT` on non-Unix platforms).
    #[cfg(not(unix))]
    pub async fn wait_for_signal() -> Result<ShutdownSignal, ShutdownError> {
        tokio::signal::ctrl_c()
            .await
            .map_err(|e| ShutdownError::Signal(e.to_string()))?;
        Ok(ShutdownSignal::Sigint)
    }

    /// Waits for either internal coordinator cancellation or an incoming OS signal.
    pub async fn wait_for_shutdown_or_signal(&self) -> ShutdownSignal {
        tokio::select! {
            _ = self.cancelled() => ShutdownSignal::Manual,
            sig_res = Self::wait_for_signal() => {
                self.shutdown();
                match sig_res {
                    Ok(sig) => sig,
                    Err(_) => ShutdownSignal::Manual,
                }
            }
        }
    }
}

static REGEX_BOT: OnceLock<Option<Regex>> = OnceLock::new();
static REGEX_BEARER: OnceLock<Option<Regex>> = OnceLock::new();
static REGEX_DISCORD_TOKEN: OnceLock<Option<Regex>> = OnceLock::new();
static REGEX_MFA: OnceLock<Option<Regex>> = OnceLock::new();
static REGEX_PEM_KEY: OnceLock<Option<Regex>> = OnceLock::new();
static REGEX_HEX_KEY: OnceLock<Option<Regex>> = OnceLock::new();
static REGEX_WEBHOOK: OnceLock<Option<Regex>> = OnceLock::new();

fn compile_regex(pattern: &str) -> Option<Regex> {
    match Regex::new(pattern) {
        Ok(re) => Some(re),
        Err(_) => None,
    }
}

fn get_bot_regex() -> Option<&'static Regex> {
    REGEX_BOT.get_or_init(|| compile_regex(r"Bot\s+[A-Za-z0-9._-]{20,}")).as_ref()
}

fn get_bearer_regex() -> Option<&'static Regex> {
    REGEX_BEARER.get_or_init(|| compile_regex(r"Bearer\s+[A-Za-z0-9._-]{20,}")).as_ref()
}

fn get_discord_token_regex() -> Option<&'static Regex> {
    REGEX_DISCORD_TOKEN.get_or_init(|| {
        compile_regex(r"\b[MNO][A-Za-z0-9]{23,28}\.[A-Za-z0-9_-]{6}\.[A-Za-z0-9_-]{27,40}\b")
    }).as_ref()
}

fn get_mfa_regex() -> Option<&'static Regex> {
    REGEX_MFA.get_or_init(|| compile_regex(r"\bmfa\.[A-Za-z0-9_-]{20,}\b")).as_ref()
}

fn get_pem_regex() -> Option<&'static Regex> {
    REGEX_PEM_KEY.get_or_init(|| {
        compile_regex(r"-----BEGIN (?:[A-Z0-9 ]+)?PRIVATE KEY-----[\s\S]*?-----END (?:[A-Z0-9 ]+)?PRIVATE KEY-----")
    }).as_ref()
}

fn get_hex_regex() -> Option<&'static Regex> {
    REGEX_HEX_KEY.get_or_init(|| {
        compile_regex(r#"(?i)\b((?:private_key|secret_key|signing_key)\s*[:=]\s*["']?)[0-9a-f]{64}(["']?)"#)
    }).as_ref()
}

fn get_webhook_regex() -> Option<&'static Regex> {
    REGEX_WEBHOOK.get_or_init(|| {
        compile_regex(r"https://(?:ptb\.|canary\.)?discord(?:app)?\.com/api/webhooks/\d{17,20}/[A-Za-z0-9_-]{60,68}")
    }).as_ref()
}

fn replace_with(input: &str, re_opt: Option<&Regex>, rep: &str) -> String {
    match re_opt {
        Some(re) => re.replace_all(input, rep).into_owned(),
        None => input.to_string(),
    }
}

/// Redacts secrets, authorization tokens, MFA tokens, and private keys from input text.
///
/// Replaces Bot/Bearer tokens, raw Discord tokens, MFA tokens, and private keys with `[REDACTED]`.
pub fn redact_secrets(input: &str) -> String {
    let s1 = replace_with(input, get_pem_regex(), "[REDACTED]");
    let s2 = replace_with(&s1, get_bot_regex(), "Bot [REDACTED]");
    let s3 = replace_with(&s2, get_bearer_regex(), "Bearer [REDACTED]");
    let s4 = replace_with(&s3, get_mfa_regex(), "[REDACTED]");
    let s5 = replace_with(&s4, get_discord_token_regex(), "[REDACTED]");
    let s6 = replace_with(&s5, get_hex_regex(), "${1}[REDACTED]${2}");
    replace_with(&s6, get_webhook_regex(), "[REDACTED]")
}

/// A wrapper around any [`Write`] stream that automatically redacts secrets before writing.
#[derive(Debug, Clone)]
pub struct RedactingWriter<W> {
    writer: W,
}

impl<W> RedactingWriter<W> {
    /// Creates a new [`RedactingWriter`] wrapping the given inner writer.
    pub fn new(writer: W) -> Self {
        Self { writer }
    }

    /// Unwraps this [`RedactingWriter`], returning the underlying writer.
    pub fn into_inner(self) -> W {
        self.writer
    }

    /// Gets a reference to the underlying writer.
    pub fn inner(&self) -> &W {
        &self.writer
    }

    /// Gets a mutable reference to the underlying writer.
    pub fn inner_mut(&mut self) -> &mut W {
        &mut self.writer
    }
}

impl<W: Write> Write for RedactingWriter<W> {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        match std::str::from_utf8(buf) {
            Ok(s) => {
                let redacted = redact_secrets(s);
                self.writer.write_all(redacted.as_bytes())?;
                Ok(buf.len())
            }
            Err(_) => self.writer.write(buf),
        }
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.writer.flush()
    }
}

impl<'a, W> tracing_subscriber::fmt::MakeWriter<'a> for RedactingWriter<W>
where
    W: tracing_subscriber::fmt::MakeWriter<'a>,
{
    type Writer = RedactingWriter<W::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactingWriter::new(self.writer.make_writer())
    }
}
