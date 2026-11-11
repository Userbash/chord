//! Локальный Crash Reporter и маскирование аварийных дампов памяти (`TASK-970`).
//! Контракт (`main-arch.md` §8.1–§8.4, `TASK-956`, `TASK-970`): запрет сетевой отправки
//! (`UPLOAD_TO_SERVER_ALLOWED = false`), перехват паник с записью `MDMP` (`0x0000A793`)
//! и `pending_minidump.txt`, маскирование токенов, `zeroize_slice` + `Drop`, ротация 10 дампов.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;
use std::hint::black_box;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

pub const MINIDUMP_MAGIC: [u8; 4] = *b"MDMP";
pub const MINIDUMP_VERSION: u32 = 0x0000_A793;
pub const MINIDUMP_STREAM_COUNT: u32 = 3;
pub const MINIDUMP_LOCAL_ONLY_FLAGS: u32 = 0x0000_0001;
pub const MAX_CRASH_DUMPS: usize = 10;
pub const MAX_METADATA_VALUE_LEN: usize = 250;
pub const PENDING_MINIDUMP_FILENAME: &str = "pending_minidump.txt";
pub const CRASHLOGS_SUBDIR: &str = "crashlogs";
pub const UPLOAD_TO_SERVER_ALLOWED: bool = false;

pub const MASK_MFA_TOKEN: &str = "mfa.***";
pub const MASK_BOT_TOKEN: &str = "Bot ***";
pub const MASK_BEARER_TOKEN: &str = "Bearer ***";
pub const MASK_USER_TOKEN: &str = "[REDACTED_TOKEN]";
pub const MASK_WEBHOOK_SECRET: &str = "[REDACTED_WEBHOOK]";
pub const MASK_DAVE_SECRET: &str = "[REDACTED_SECRET]";
pub const MASK_PRIVATE_KEY: &str = "[REDACTED_PRIVATE_KEY]";
pub const MASK_GITHUB_TOKEN: &str = "ghp_***";

static DUMP_SEQUENCE: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CrashReporterError {
    NetworkUploadForbidden { attempted_url: String },
    InvalidMaxDumps(usize),
    EmptyCrashDir,
    InvalidMinidumpHeader,
    UnmaskedSecretDetected { kind: &'static str },
    IoError(String),
}

impl fmt::Display for CrashReporterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for CrashReporterError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ZeroizeMetrics {
    pub zeroized_bytes: usize,
    pub all_zero: bool,
}

/// Принудительно перезаписывает каждый байт среза нулём через `black_box(0)`.
pub fn zeroize_slice(buf: &mut [u8]) -> ZeroizeMetrics {
    for byte in buf.iter_mut() {
        *byte = black_box(0);
    }
    let non_zero = buf.iter().fold(0u8, |acc, &b| acc | black_box(b));
    ZeroizeMetrics {
        zeroized_bytes: buf.len(),
        all_zero: non_zero == 0,
    }
}

/// Защищённый временный буфер для необработанных строк и стека паники с авто-очисткой в `Drop`.
pub struct SensitiveScratchBuffer {
    buf: Vec<u8>,
    zeroized: bool,
}

impl SensitiveScratchBuffer {
    pub fn from_raw_str(raw: &str) -> Self {
        Self::from_raw_bytes(raw.as_bytes().to_vec())
    }

    pub fn from_raw_bytes(raw: Vec<u8>) -> Self {
        Self {
            buf: raw,
            zeroized: false,
        }
    }

    pub fn extract_sanitized_text_and_wipe(&mut self) -> (String, ZeroizeMetrics) {
        let sanitized = sanitize_crash_text(&String::from_utf8_lossy(&self.buf));
        let metrics = self.zeroize_now();
        (sanitized, metrics)
    }

    pub fn extract_sanitized_bytes_and_wipe(&mut self) -> (Vec<u8>, ZeroizeMetrics) {
        let (text, metrics) = self.extract_sanitized_text_and_wipe();
        (text.into_bytes(), metrics)
    }

    pub fn zeroize_now(&mut self) -> ZeroizeMetrics {
        let metrics = zeroize_slice(&mut self.buf);
        self.zeroized = metrics.all_zero;
        metrics
    }

    pub fn is_zeroized(&self) -> bool {
        self.zeroized && self.buf.iter().all(|&b| black_box(b) == 0)
    }
}

impl Drop for SensitiveScratchBuffer {
    fn drop(&mut self) {
        let _ = zeroize_slice(&mut self.buf);
        self.zeroized = true;
    }
}

impl fmt::Debug for SensitiveScratchBuffer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SensitiveScratchBuffer")
            .field("len", &self.buf.len())
            .field("bytes", &"<redacted>")
            .field("zeroized", &self.zeroized)
            .finish()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashReporterConfig {
    pub crash_dir: PathBuf,
    pub release_channel: String,
    pub app_version: String,
    pub max_dumps: usize,
    pub upload_to_server: bool,
    pub submit_url: Option<String>,
}

impl CrashReporterConfig {
    pub fn local_only(crash_dir: PathBuf) -> Self {
        Self {
            crash_dir,
            release_channel: "beta".to_string(),
            app_version: "1.0.0-beta.1".to_string(),
            max_dumps: MAX_CRASH_DUMPS,
            upload_to_server: UPLOAD_TO_SERVER_ALLOWED,
            submit_url: None,
        }
    }

    pub fn validate(&self) -> Result<(), CrashReporterError> {
        if self.upload_to_server {
            let url = self
                .submit_url
                .as_deref()
                .unwrap_or("upload_to_server=true");
            return Err(CrashReporterError::NetworkUploadForbidden {
                attempted_url: url.to_string(),
            });
        }
        if let Some(url) = &self.submit_url {
            return Err(CrashReporterError::NetworkUploadForbidden {
                attempted_url: sanitize_crash_text(url),
            });
        }
        if self.crash_dir.as_os_str().is_empty() {
            return Err(CrashReporterError::EmptyCrashDir);
        }
        if self.max_dumps == 0 || self.max_dumps > 100 {
            return Err(CrashReporterError::InvalidMaxDumps(self.max_dumps));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinuxCrashMetadata {
    pub release_channel: String,
    pub wm: String,
    pub runtime_environment: String,
    pub display_server: String,
    pub extra: BTreeMap<String, String>,
}

impl LinuxCrashMetadata {
    pub fn from_env_map(release_channel: &str, env: &[(String, String)]) -> Self {
        let find = |key: &str| {
            env.iter()
                .find(|(k, v)| k == key && !v.trim().is_empty())
                .map(|(_, v)| v.as_str())
        };
        let wm = format!(
            "{},{}",
            find("XDG_CURRENT_DESKTOP").unwrap_or("unknown"),
            find("GDMSESSION").unwrap_or("unknown")
        );
        let runtime_env = if find("FLATPAK_ID").is_some() {
            "flatpak"
        } else if find("SNAP").is_some() || find("SNAP_NAME").is_some() {
            "snap"
        } else if find("APPIMAGE").is_some() || find("APPDIR").is_some() {
            "appimage"
        } else {
            "native"
        };
        Self {
            release_channel: sanitize_and_truncate(release_channel),
            wm: sanitize_and_truncate(&wm),
            runtime_environment: runtime_env.to_string(),
            display_server: sanitize_and_truncate(find("XDG_SESSION_TYPE").unwrap_or("unknown")),
            extra: BTreeMap::new(),
        }
    }

    pub fn collect_from_process_env(release_channel: &str) -> Self {
        let pairs: Vec<(String, String)> = std::env::vars().collect();
        Self::from_env_map(release_channel, &pairs)
    }

    pub fn reconcile_metadata(&mut self, additional: &[(String, String)]) {
        for (key, value) in additional {
            self.extra
                .insert(sanitize_and_truncate(key), sanitize_and_truncate(value));
        }
    }
}

fn sanitize_and_truncate(value: &str) -> String {
    sanitize_crash_text(value)
        .chars()
        .take(MAX_METADATA_VALUE_LEN)
        .collect()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MemorySnapshot {
    pub vm_rss_kb: u64,
    pub vm_peak_kb: u64,
}

impl MemorySnapshot {
    pub fn read_proc_self_status() -> Self {
        std::fs::read_to_string("/proc/self/status")
            .map(|text| Self::parse_proc_status(&text))
            .unwrap_or(Self {
                vm_rss_kb: 1024,
                vm_peak_kb: 1024,
            })
    }

    pub fn parse_proc_status(status_text: &str) -> Self {
        let parse_kb = |s: &str| {
            s.split_whitespace()
                .next()
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0)
        };
        let (mut rss, mut peak) = (0u64, 0u64);
        for line in status_text.lines() {
            if let Some(rest) = line.strip_prefix("VmRSS:") {
                rss = parse_kb(rest);
            } else if let Some(rest) = line.strip_prefix("VmPeak:") {
                peak = parse_kb(rest);
            }
        }
        let fallback_rss = rss.max(1024);
        Self {
            vm_rss_kb: fallback_rss,
            vm_peak_kb: peak.max(fallback_rss),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MinidumpInspection {
    pub magic: [u8; 4],
    pub version: u32,
    pub stream_count: u32,
    pub timestamp_unix_sec: u64,
    pub local_only: bool,
    pub network_upload_enabled: bool,
    pub payload_text: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CrashDumpRecord {
    pub crash_id: String,
    pub dump_filename: String,
    pub minidump_path: PathBuf,
    pub json_report_path: PathBuf,
    pub pending_sentinel_path: PathBuf,
    pub sanitized_message: String,
    pub zeroized_bytes: usize,
    pub upload_to_server: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastCrashReport {
    pub crash_id: String,
    pub pending_minidump_path: PathBuf,
    pub json_report_path: PathBuf,
    pub sanitized_message: String,
    pub upload_to_server: bool,
}

pub fn sanitize_crash_text(input: &str) -> String {
    if input.is_empty() {
        return String::new();
    }
    let mut out = mask_pem_blocks(input);
    for host in [
        "https://discord.com/api/webhooks/",
        "https://discordapp.com/api/webhooks/",
        "https://ptb.discord.com/api/webhooks/",
        "https://canary.discord.com/api/webhooks/",
    ] {
        out = mask_prefixed_tokens(&out, host, &format!("{host}{MASK_WEBHOOK_SECRET}"), |c| {
            is_token_base64url_char(c) || c == '/'
        });
    }
    for key in [
        "joiner_secret",
        "epoch_secret",
        "exporter_secret",
        "welcome_secret",
        "confirmation_key",
        "private_key_hex",
        "secret_signing_key",
    ] {
        out = mask_key_hex_assignment(&out, key);
    }
    out = mask_prefixed_tokens(&out, "Bot ", MASK_BOT_TOKEN, is_token_dot_char);
    out = mask_prefixed_tokens(&out, "Bearer ", MASK_BEARER_TOKEN, is_token_dot_char);
    out = mask_prefixed_tokens(&out, "mfa.", MASK_MFA_TOKEN, is_token_base64url_char);
    for prefix in ["ghp_", "gho_", "ghu_", "ghs_", "ghr_", "github_pat_"] {
        out = mask_prefixed_tokens(&out, prefix, MASK_GITHUB_TOKEN, is_token_base64url_char);
    }
    mask_jwt_and_discord_user_tokens(&out)
}

pub fn sanitize_memory_dump_bytes(raw: &[u8]) -> Vec<u8> {
    sanitize_crash_text(&String::from_utf8_lossy(raw)).into_bytes()
}

pub fn verify_no_unmasked_secrets(text: &str) -> Result<(), CrashReporterError> {
    if sanitize_crash_text(text) != text {
        return Err(CrashReporterError::UnmaskedSecretDetected {
            kind: "unmasked_token_or_key",
        });
    }
    Ok(())
}

fn is_token_base64url_char(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_' || ch == '-'
}

fn is_token_dot_char(ch: char) -> bool {
    is_token_base64url_char(ch) || ch == '.'
}

fn mask_pem_blocks(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(start) = rest.find("-----BEGIN ") {
        out.push_str(&rest[..start]);
        let after = &rest[start..];
        if let Some(e1) = after.find("PRIVATE KEY-----") {
            let tail = &after[e1 + "PRIVATE KEY-----".len()..];
            if let Some(e2) = tail.find("PRIVATE KEY-----") {
                out.push_str(MASK_PRIVATE_KEY);
                rest = &tail[e2 + "PRIVATE KEY-----".len()..];
                continue;
            }
        }
        out.push_str(MASK_PRIVATE_KEY);
        return out;
    }
    out.push_str(rest);
    out
}

fn mask_key_hex_assignment(input: &str, key: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find(key) {
        out.push_str(&rest[..pos]);
        let after_key = &rest[pos + key.len()..];
        let trimmed = after_key.trim_start();
        if let Some(after_sep) = trimmed
            .strip_prefix('=')
            .or_else(|| trimmed.strip_prefix(':'))
        {
            let val = after_sep.trim_start();
            let quoted = val.starts_with('"') || val.starts_with('\'');
            let hex_s = if quoted { &val[1..] } else { val };
            let hex_len = hex_s.chars().take_while(|c| c.is_ascii_hexdigit()).count();
            if hex_len >= 64 {
                out.push_str(&format!("{key}=\"{MASK_DAVE_SECRET}\""));
                rest = &hex_s[hex_len + usize::from(quoted)..];
                continue;
            }
        }
        out.push_str(key);
        rest = after_key;
    }
    out.push_str(rest);
    out
}

fn mask_prefixed_tokens(
    input: &str,
    prefix: &str,
    replacement: &str,
    valid_char: fn(char) -> bool,
) -> String {
    let mut out = String::with_capacity(input.len());
    let mut rest = input;
    while let Some(pos) = rest.find(prefix) {
        out.push_str(&rest[..pos]);
        let after = &rest[pos + prefix.len()..];
        let tok_bytes: usize = after
            .chars()
            .take_while(|&c| valid_char(c))
            .map(char::len_utf8)
            .sum();
        if tok_bytes >= 20 {
            out.push_str(replacement);
            rest = &after[tok_bytes..];
        } else {
            out.push_str(prefix);
            rest = after;
        }
    }
    out.push_str(rest);
    out
}

fn mask_jwt_and_discord_user_tokens(input: &str) -> String {
    let mut out = String::with_capacity(input.len());
    let mut i = 0usize;
    while i < input.len() {
        if let Some(consumed) = match_three_part_secret(&input[i..]) {
            out.push_str(MASK_USER_TOKEN);
            i += consumed;
        } else if let Some(ch) = input[i..].chars().next() {
            out.push(ch);
            i += ch.len_utf8();
        } else {
            break;
        }
    }
    out
}

fn match_three_part_secret(slice: &str) -> Option<usize> {
    let mut parts = slice.splitn(3, '.');
    let p1 = parts.next()?;
    let p2 = parts.next()?;
    let p3: String = parts
        .next()?
        .chars()
        .take_while(|&c| is_token_base64url_char(c))
        .collect();
    let is_jwt = p1.starts_with("eyJ")
        && p2.starts_with("eyJ")
        && p1.len() >= 10
        && p2.len() >= 10
        && p3.len() >= 16
        && p1.chars().all(is_token_base64url_char)
        && p2.chars().all(is_token_base64url_char);
    let is_discord = matches!(p1.as_bytes().first(), Some(b'M' | b'N' | b'O'))
        && (24..=29).contains(&p1.len())
        && p1.chars().all(|c| c.is_ascii_alphanumeric())
        && p2.len() == 6
        && p2.chars().all(is_token_base64url_char)
        && (27..=40).contains(&p3.len());
    (is_jwt || is_discord).then_some(p1.len() + 1 + p2.len() + 1 + p3.len())
}

pub fn build_minidump_bytes(
    timestamp_unix_sec: u64,
    exc: (&str, &str, &str),
    meta: &LinuxCrashMetadata,
    mem: MemorySnapshot,
    sanitized_stack: &[u8],
) -> Vec<u8> {
    let (sanitized_message, location, thread_name) = exc;
    let mut buf = Vec::with_capacity(256 + sanitized_message.len() + sanitized_stack.len());
    buf.extend_from_slice(&MINIDUMP_MAGIC);
    buf.extend_from_slice(&MINIDUMP_VERSION.to_le_bytes());
    buf.extend_from_slice(&MINIDUMP_STREAM_COUNT.to_le_bytes());
    buf.extend_from_slice(&timestamp_unix_sec.to_le_bytes());
    buf.extend_from_slice(&MINIDUMP_LOCAL_ONLY_FLAGS.to_le_bytes());
    let payload = format!(
        "STREAM_SYS:channel={};wm={};env={};display={};rss_kb={};peak_kb={}\nSTREAM_EXC:thread={};loc={};msg={}\nSTREAM_MEM:",
        meta.release_channel,
        meta.wm,
        meta.runtime_environment,
        meta.display_server,
        mem.vm_rss_kb,
        mem.vm_peak_kb,
        sanitize_crash_text(thread_name),
        sanitize_crash_text(location),
        sanitized_message,
    );
    buf.extend_from_slice(payload.as_bytes());
    buf.extend_from_slice(sanitized_stack);
    buf
}

pub fn parse_and_verify_minidump(bytes: &[u8]) -> Result<MinidumpInspection, CrashReporterError> {
    if bytes.len() < 24 {
        return Err(CrashReporterError::InvalidMinidumpHeader);
    }
    let magic = [bytes[0], bytes[1], bytes[2], bytes[3]];
    let version = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
    if magic != MINIDUMP_MAGIC || version != MINIDUMP_VERSION {
        return Err(CrashReporterError::InvalidMinidumpHeader);
    }
    let stream_count = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let timestamp_unix_sec = u64::from_le_bytes([
        bytes[12], bytes[13], bytes[14], bytes[15], bytes[16], bytes[17], bytes[18], bytes[19],
    ]);
    let flags = u32::from_le_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    let local_only = (flags & 0x01) == 1;
    let network_upload_enabled = (flags >> 31) != 0;
    if !local_only || network_upload_enabled {
        return Err(CrashReporterError::NetworkUploadForbidden {
            attempted_url: "minidump_header_network_bit_set".to_string(),
        });
    }
    let payload_text = String::from_utf8_lossy(&bytes[24..]).into_owned();
    verify_no_unmasked_secrets(&payload_text)?;
    Ok(MinidumpInspection {
        magic,
        version,
        stream_count,
        timestamp_unix_sec,
        local_only,
        network_upload_enabled,
        payload_text,
    })
}

fn escape_json_str(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

fn format_crash_json_report(
    crash_id: &str,
    now_sec: u64,
    total_zeroized: usize,
    exc: (&str, &str, &str),
    meta: &LinuxCrashMetadata,
    mem: MemorySnapshot,
) -> String {
    let (sanitized_msg, loc_str, thread_name) = exc;
    let (lb, rb) = (char::from(123u8), char::from(125u8));
    format!(
        "{lb}\"crash_id\":\"{}\",\"timestamp_unix_sec\":{},\"upload_to_server\":false,\"network_sent\":false,\"tokens_masked\":true,\"memory_zeroized\":true,\"zeroized_bytes\":{},\"panic_message\":\"{}\",\"location\":\"{}\",\"thread_name\":\"{}\",\"metadata\":{lb}\"release_channel\":\"{}\",\"wm\":\"{}\",\"runtime_environment\":\"{}\",\"display_server\":\"{}\"{rb},\"memory_snapshot\":{lb}\"vm_rss_kb\":{},\"vm_peak_kb\":{}{rb}{rb}\n",
        escape_json_str(crash_id),
        now_sec,
        total_zeroized,
        escape_json_str(sanitized_msg),
        escape_json_str(&sanitize_crash_text(loc_str)),
        escape_json_str(&sanitize_crash_text(thread_name)),
        escape_json_str(&meta.release_channel),
        escape_json_str(&meta.wm),
        escape_json_str(&meta.runtime_environment),
        escape_json_str(&meta.display_server),
        mem.vm_rss_kb,
        mem.vm_peak_kb,
    )
}

pub struct CrashReporter {
    config: CrashReporterConfig,
    crashlogs_dir: PathBuf,
}

impl CrashReporter {
    pub fn new(config: CrashReporterConfig) -> Result<Self, CrashReporterError> {
        config.validate()?;
        let crashlogs_dir = config.crash_dir.join(CRASHLOGS_SUBDIR);
        std::fs::create_dir_all(&crashlogs_dir)
            .map_err(|e| CrashReporterError::IoError(e.to_string()))?;
        Ok(Self {
            config,
            crashlogs_dir,
        })
    }

    pub fn write_crash_dump(
        &self,
        panic_message: &str,
        location: Option<&str>,
        thread_name: &str,
        raw_stack_memory: &[u8],
    ) -> Result<CrashDumpRecord, CrashReporterError> {
        self.config.validate()?;
        let mut msg_scratch = SensitiveScratchBuffer::from_raw_str(panic_message);
        let (sanitized_msg, z_msg) = msg_scratch.extract_sanitized_text_and_wipe();
        let mut mem_scratch = SensitiveScratchBuffer::from_raw_bytes(raw_stack_memory.to_vec());
        let (sanitized_stack, z_mem) = mem_scratch.extract_sanitized_bytes_and_wipe();
        verify_no_unmasked_secrets(&sanitized_msg)?;
        let total_zeroized = z_msg.zeroized_bytes + z_mem.zeroized_bytes;
        self.persist_sanitized_dump(
            (&sanitized_msg, location.unwrap_or("<unknown>"), thread_name),
            &sanitized_stack,
            total_zeroized,
        )
    }

    fn persist_sanitized_dump(
        &self,
        exc: (&str, &str, &str),
        sanitized_stack: &[u8],
        total_zeroized: usize,
    ) -> Result<CrashDumpRecord, CrashReporterError> {
        let seq = DUMP_SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let now_sec = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(1_762_860_000);
        let crash_id = format!("crash-{now_sec}-{}-{seq:04}", std::process::id());
        let dump_filename = format!("{crash_id}.dmp");
        let minidump_path = self.crashlogs_dir.join(&dump_filename);
        let json_report_path = self.crashlogs_dir.join(format!("{crash_id}.json"));
        let pending_sentinel_path = self.crashlogs_dir.join(PENDING_MINIDUMP_FILENAME);
        let meta = LinuxCrashMetadata::collect_from_process_env(&self.config.release_channel);
        let mem = MemorySnapshot::read_proc_self_status();
        let dmp = build_minidump_bytes(now_sec, exc, &meta, mem, sanitized_stack);
        let json = format_crash_json_report(&crash_id, now_sec, total_zeroized, exc, &meta, mem);
        std::fs::write(&minidump_path, &dmp)
            .and_then(|()| std::fs::write(&json_report_path, json.as_bytes()))
            .and_then(|()| std::fs::write(&pending_sentinel_path, format!("{dump_filename}\n")))
            .map_err(|e| CrashReporterError::IoError(e.to_string()))?;
        self.enforce_retention_limit()?;
        Ok(CrashDumpRecord {
            crash_id,
            dump_filename,
            minidump_path,
            json_report_path,
            pending_sentinel_path,
            sanitized_message: exc.0.to_string(),
            zeroized_bytes: total_zeroized,
            upload_to_server: false,
        })
    }

    pub fn list_minidump_files(&self) -> Result<Vec<PathBuf>, CrashReporterError> {
        let mut files = Vec::new();
        for entry in std::fs::read_dir(&self.crashlogs_dir)
            .map_err(|e| CrashReporterError::IoError(e.to_string()))?
        {
            let path = entry
                .map_err(|e| CrashReporterError::IoError(e.to_string()))?
                .path();
            if path.extension().and_then(|s| s.to_str()) == Some("dmp") {
                files.push(path);
            }
        }
        files.sort();
        Ok(files)
    }

    pub fn enforce_retention_limit(&self) -> Result<usize, CrashReporterError> {
        let files = self.list_minidump_files()?;
        let remove_count = files.len().saturating_sub(self.config.max_dumps);
        for old_dmp in files.iter().take(remove_count) {
            let _ = std::fs::remove_file(old_dmp);
            let _ = std::fs::remove_file(old_dmp.with_extension("json"));
        }
        Ok(remove_count)
    }

    pub fn consume_last_crash_report(&self) -> Result<Option<LastCrashReport>, CrashReporterError> {
        let sentinel = self.crashlogs_dir.join(PENDING_MINIDUMP_FILENAME);
        if !sentinel.exists() {
            return Ok(None);
        }
        let raw_name = std::fs::read_to_string(&sentinel)
            .map_err(|e| CrashReporterError::IoError(e.to_string()))?;
        let _ = std::fs::remove_file(&sentinel);
        let dump_name = raw_name.trim();
        if dump_name.is_empty() {
            return Ok(None);
        }
        let dmp_path = self.crashlogs_dir.join(dump_name);
        let crash_id = dump_name.trim_end_matches(".dmp").to_string();
        let json_path = self.crashlogs_dir.join(format!("{crash_id}.json"));
        let dmp_bytes =
            std::fs::read(&dmp_path).map_err(|e| CrashReporterError::IoError(e.to_string()))?;
        let inspection = parse_and_verify_minidump(&dmp_bytes)?;
        Ok(Some(LastCrashReport {
            crash_id,
            pending_minidump_path: dmp_path,
            json_report_path: json_path,
            sanitized_message: inspection.payload_text,
            upload_to_server: false,
        }))
    }
}

pub fn install_panic_hook(
    config: CrashReporterConfig,
    initial_stack_context: Vec<u8>,
) -> Result<(), CrashReporterError> {
    let reporter = Arc::new(CrashReporter::new(config)?);
    let stack_holder = Arc::new(Mutex::new(initial_stack_context));
    let prev_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let payload_msg = panic_info
            .payload()
            .downcast_ref::<&str>()
            .map(|s| (*s).to_string())
            .or_else(|| panic_info.payload().downcast_ref::<String>().cloned())
            .unwrap_or_else(|| "Box<dyn Any> panic payload".to_string());
        let loc_string = panic_info
            .location()
            .map(|l| format!("{}:{}:{}", l.file(), l.line(), l.column()));
        let thread_curr = std::thread::current();
        let thread_name = thread_curr.name().unwrap_or("unnamed-thread");
        let raw_stack = stack_holder
            .lock()
            .map(|mut g| {
                let c = g.clone();
                let _ = zeroize_slice(&mut g);
                c
            })
            .unwrap_or_default();
        let _ =
            reporter.write_crash_dump(&payload_msg, loc_string.as_deref(), thread_name, &raw_stack);
        prev_hook(panic_info);
    }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sanitize_tokens_zeroize_and_rotation() {
        assert_eq!(sanitize_crash_text(""), "");
        assert_eq!(
            sanitize_crash_text("mfa.short_19_chars_"),
            "mfa.short_19_chars_"
        );
        let mfa = format!("mfa.{}", "A9_b-7xQ".repeat(10));
        let bot = format!(
            "Bot Nzk5{}.X9kLmN.{}",
            "MDEyMzQ1Njc4OTAxMjM0NTY", "qR7vW2pL9xK4mN8bV1cZ5jH3gF6dS0aP"
        );
        let dave = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
        let clean = sanitize_crash_text(&format!("mfa={mfa} {bot} epoch_secret=\"{dave}\""));
        assert!(clean.contains(MASK_MFA_TOKEN) && clean.contains(MASK_BOT_TOKEN));
        assert!(clean.contains(MASK_DAVE_SECRET) && verify_no_unmasked_secrets(&clean).is_ok());

        let mut scratch = SensitiveScratchBuffer::from_raw_str(&mfa);
        assert!(format!("{scratch:?}").contains("<redacted>"));
        let (_, z) = scratch.extract_sanitized_text_and_wipe();
        assert!(z.all_zero && scratch.is_zeroized());

        let seq = DUMP_SEQUENCE.fetch_add(1, Ordering::SeqCst);
        let dir = std::env::temp_dir().join(format!("gpui-crash-ut-{}-{seq}", std::process::id()));
        let mut cfg = CrashReporterConfig::local_only(dir.clone());
        cfg.max_dumps = 2;
        let rep = CrashReporter::new(cfg).expect("init");
        for i in 0..3 {
            rep.write_crash_dump(&format!("e {i} {mfa}"), None, "main", b"")
                .expect("w");
        }
        assert_eq!(rep.list_minidump_files().expect("ls").len(), 2);
        assert!(rep.consume_last_crash_report().expect("c1").is_some());
        assert!(rep.consume_last_crash_report().expect("c2").is_none());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
