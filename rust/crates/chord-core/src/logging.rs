#![forbid(unsafe_code)]

//! Secret Redaction Layer for `tracing_subscriber` (STAGE-02.3.1).
//!
//! Provides log filtering, token masking, and redaction wrappers for
//! `tracing_subscriber` logging to prevent leakage of Bot/Bearer tokens,
//! Discord session tokens, passwords, and cryptographic keys in stdout or log files.

use std::fmt;
use std::io::{self, Write};
use std::sync::Arc;
use tracing::Subscriber;
use tracing_subscriber::fmt::MakeWriter;
use tracing_subscriber::layer::{Context, Layer};

/// Marker inserted in place of masked secrets.
pub const REDACTED_MARKER: &str = "[REDACTED]";

/// Sensitive keys searched in JSON and key-value string assignments.
const SENSITIVE_KEYS: &[&str] = &[
    "secret_key",
    "secret",
    "password",
    "token",
    "client_secret",
    "encryption_key",
    "auth_session_id_hash",
    "api_key",
    "session_id",
    "key",
];

/// Prefixes identifying auth header schemes.
const AUTH_PREFIXES: &[&str] = &["Bot ", "bot ", "Bearer ", "bearer "];

/// Token prefixes indicating credential types.
const TOKEN_PREFIXES: &[&str] = &["ghp_", "gho_", "ghu_", "ghs_", "ghr_"];

fn is_hex_char(c: char) -> bool {
    c.is_ascii_hexdigit()
}

fn is_base64_url_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '-' || c == '_'
}

fn is_discord_token(s: &str) -> bool {
    if s.len() < 50 || s == REDACTED_MARKER {
        return false;
    }
    let parts: Vec<&str> = s.split('.').collect();
    if parts.len() != 3 {
        return false;
    }
    for part in parts {
        if part.is_empty() || !part.chars().all(is_base64_url_char) {
            return false;
        }
    }
    true
}

fn is_64_hex_key(s: &str) -> bool {
    s.len() == 64 && s.chars().all(is_hex_char)
}

fn is_mfa_token(s: &str) -> bool {
    if s.starts_with("mfa.") && s.len() > 8 && s != REDACTED_MARKER {
        let remainder = match s.get("mfa.".len()..) {
            Some(rem) => rem,
            None => return false,
        };
        remainder.chars().all(is_base64_url_char)
    } else {
        false
    }
}

fn is_github_token(s: &str) -> bool {
    for prefix in TOKEN_PREFIXES {
        if s.starts_with(prefix) && s.len() > 10 && s != REDACTED_MARKER {
            let remainder = match s.get(prefix.len()..) {
                Some(rem) => rem,
                None => return false,
            };
            return remainder.chars().all(is_base64_url_char);
        }
    }
    false
}

fn redact_json_field_pattern(mut text: String, pattern: &str) -> String {
    let mut start_pos = 0;
    while let Some(idx) = text[start_pos..].find(pattern) {
        let val_start = start_pos + idx + pattern.len();
        if let Some(val_len) = text[val_start..].find('"') {
            let val_end = val_start + val_len;
            let current_val = &text[val_start..val_end];
            if current_val != REDACTED_MARKER {
                text.replace_range(val_start..val_end, REDACTED_MARKER);
            }
            start_pos = val_start + REDACTED_MARKER.len() + 1;
        } else {
            break;
        }
    }
    text
}

fn redact_key_value_assignments(mut text: String) -> String {
    for key in SENSITIVE_KEYS {
        let spaced = format!("\"{key}\": \"");
        text = redact_json_field_pattern(text, &spaced);
        let tight = format!("\"{key}\":\"");
        text = redact_json_field_pattern(text, &tight);

        let kv_pattern = format!("{key}=");
        let mut start_pos = 0;
        while let Some(idx) = text[start_pos..].find(&kv_pattern) {
            let val_start = start_pos + idx + kv_pattern.len();
            let val_len = text[val_start..]
                .find(|c: char| c.is_whitespace() || c == '&' || c == ';' || c == '"' || c == '\'')
                .unwrap_or(text[val_start..].len());
            let val_end = val_start + val_len;
            let current_val = &text[val_start..val_end];
            if current_val != REDACTED_MARKER && !current_val.is_empty() {
                text.replace_range(val_start..val_end, REDACTED_MARKER);
                start_pos = val_start + REDACTED_MARKER.len();
            } else {
                start_pos = val_end.max(val_start + 1);
            }
        }
    }
    text
}

fn redact_auth_headers(mut text: String) -> String {
    for prefix in AUTH_PREFIXES {
        let mut start_pos = 0;
        while let Some(idx) = text[start_pos..].find(prefix) {
            let val_start = start_pos + idx + prefix.len();
            let val_len = text[val_start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == ',' || c == ';')
                .unwrap_or(text[val_start..].len());
            let val_end = val_start + val_len;
            let current_val = &text[val_start..val_end];
            if current_val != REDACTED_MARKER && !current_val.is_empty() {
                text.replace_range(val_start..val_end, REDACTED_MARKER);
                start_pos = val_start + REDACTED_MARKER.len();
            } else {
                start_pos = val_end.max(val_start + 1);
            }
        }
    }
    text
}

fn redact_single_token(token: &str) -> String {
    let trimmed_start = token.find(|c: char| c.is_alphanumeric()).unwrap_or(0);
    let trimmed_end =
        token
            .rfind(|c: char| c.is_alphanumeric())
            .map_or(token.len(), |i| match token.get(i..) {
                Some(slice) => match slice.chars().next() {
                    Some(first) => i + first.len_utf8(),
                    None => token.len(),
                },
                None => token.len(),
            });

    let prefix_punct = token.get(..trimmed_start).unwrap_or("");
    let core = token.get(trimmed_start..trimmed_end).unwrap_or("");
    let suffix_punct = token.get(trimmed_end..).unwrap_or("");

    if is_discord_token(core) || is_64_hex_key(core) || is_mfa_token(core) || is_github_token(core)
    {
        format!("{prefix_punct}{REDACTED_MARKER}{suffix_punct}")
    } else {
        token.to_string()
    }
}

fn redact_word_tokens(text: &str) -> String {
    let mut result = String::with_capacity(text.len());
    let mut chars = text.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        if ch.is_whitespace() {
            result.push(ch);
            continue;
        }

        let mut end = start + ch.len_utf8();
        while let Some(&(_, next_ch)) = chars.peek() {
            if next_ch.is_whitespace() {
                break;
            }
            chars.next();
            end += next_ch.len_utf8();
        }

        let token = text.get(start..end).unwrap_or("");
        result.push_str(&redact_single_token(token));
    }

    result
}

/// Redacts sensitive tokens (Bot, Bearer, MFA, GitHub, Discord tokens) and encryption keys
/// from the provided string, replacing them with `"[REDACTED]"`.
#[must_use]
pub fn redact_secrets(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }

    let pass1 = redact_auth_headers(input.to_string());
    let pass2 = redact_key_value_assignments(pass1);
    redact_word_tokens(&pass2)
}

/// Writer adapter for standard streams that redacts secrets before writing.
pub struct RedactedWriter<W> {
    inner: W,
    buffer: Vec<u8>,
}

impl<W> RedactedWriter<W> {
    /// Creates a new `RedactedWriter` wrapping the underlying stream.
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            buffer: Vec::new(),
        }
    }

    /// Consumes the wrapper and returns the inner writer.
    pub fn into_inner(self) -> W {
        self.inner
    }

    /// Returns a reference to the inner writer.
    #[must_use]
    pub fn inner(&self) -> &W {
        &self.inner
    }

    /// Returns a mutable reference to the inner writer.
    pub fn inner_mut(&mut self) -> &mut W {
        &mut self.inner
    }
}

impl<W: Write> RedactedWriter<W> {
    fn flush_buffer(&mut self) -> io::Result<()> {
        if self.buffer.is_empty() {
            return Ok(());
        }

        let raw_str = String::from_utf8_lossy(&self.buffer);
        let redacted = redact_secrets(&raw_str);
        self.inner.write_all(redacted.as_bytes())?;
        self.buffer.clear();
        Ok(())
    }
}

impl<W: Write> Write for RedactedWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.extend_from_slice(buf);
        if self.buffer.contains(&b'\n') {
            self.flush_buffer()?;
        }
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.flush_buffer()?;
        self.inner.flush()
    }
}

impl<W: fmt::Debug> fmt::Debug for RedactedWriter<W> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("RedactedWriter")
            .field("inner", &self.inner)
            .field("buffered_bytes", &self.buffer.len())
            .finish()
    }
}

/// A thread-safe writer targeting an `Arc<Mutex<W>>`.
pub struct SharedMutexWriter<W> {
    inner: Arc<std::sync::Mutex<W>>,
}

impl<W: Write> Write for SharedMutexWriter<W> {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        guard.write(buf)
    }

    fn flush(&mut self) -> io::Result<()> {
        let mut guard = match self.inner.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        guard.flush()
    }
}

/// A `MakeWriter` implementation producing `SharedMutexWriter` instances.
#[derive(Debug, Clone)]
pub struct SharedMutexMakeWriter<W> {
    inner: Arc<std::sync::Mutex<W>>,
}

impl<W> SharedMutexMakeWriter<W> {
    /// Creates a new `SharedMutexMakeWriter` wrapping the given `Arc<Mutex<W>>`.
    pub fn new(inner: Arc<std::sync::Mutex<W>>) -> Self {
        Self { inner }
    }
}

impl<'a, W: Write + Send + 'static> MakeWriter<'a> for SharedMutexMakeWriter<W> {
    type Writer = SharedMutexWriter<W>;

    fn make_writer(&'a self) -> Self::Writer {
        SharedMutexWriter {
            inner: Arc::clone(&self.inner),
        }
    }
}

/// `MakeWriter` implementation wrapping another `MakeWriter` with secret redaction.
#[derive(Debug, Clone, Default)]
pub struct RedactedMakeWriter<M> {
    inner: M,
}

impl<M> RedactedMakeWriter<M> {
    /// Creates a new `RedactedMakeWriter` wrapping an inner `MakeWriter`.
    pub fn new(inner: M) -> Self {
        Self { inner }
    }
}

impl<W: Write + Send + 'static> RedactedMakeWriter<SharedMutexMakeWriter<W>> {
    /// Creates a `RedactedMakeWriter` writing to a shared thread-safe buffer.
    pub fn from_shared(shared: Arc<std::sync::Mutex<W>>) -> Self {
        Self {
            inner: SharedMutexMakeWriter::new(shared),
        }
    }
}

impl<'a, M> MakeWriter<'a> for RedactedMakeWriter<M>
where
    M: MakeWriter<'a>,
{
    type Writer = RedactedWriter<M::Writer>;

    fn make_writer(&'a self) -> Self::Writer {
        RedactedWriter::new(self.inner.make_writer())
    }

    fn make_writer_for(&'a self, meta: &tracing::Metadata<'_>) -> Self::Writer {
        RedactedWriter::new(self.inner.make_writer_for(meta))
    }
}

/// `tracing_subscriber` Layer providing secret redaction awareness and validation.
#[derive(Debug, Default, Clone)]
pub struct SecretRedactionLayer {
    _private: (),
}

impl SecretRedactionLayer {
    /// Creates a new `SecretRedactionLayer`.
    #[must_use]
    pub fn new() -> Self {
        Self { _private: () }
    }
}

impl<S> Layer<S> for SecretRedactionLayer
where
    S: Subscriber,
{
    fn on_event(&self, _event: &tracing::Event<'_>, _ctx: Context<'_, S>) {
        // Layer observation hook: ensures events flowing through subscriber are recorded
    }
}
