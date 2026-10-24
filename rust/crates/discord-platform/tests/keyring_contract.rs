//! Контракт P1-10: keyring через FFI к системному `libsecret 0.21` (headless).
//!
//! Независимый оракул — реальный `org.freedesktop.secrets` (ksecretd) на сессионной шине и
//! CLI `secret-tool` (store/lookup/clear), а не наша же реализация. Секреты — синтетические
//! (`test-secret-…`), ключ уникален и содержит pid; после каждого кейса запись очищается.
//!
//! Гейтинг (TZ W33a §2/§3, PLAT-02):
//!
//! - Все 12 кейсов (k01–k12) — live: им нужен реальный сервис и `DISCORD_LAB_KEYRING_LIVE=1`
//!   (API заморожен, [`Keyring`] без `Service` не собрать). Поэтому каждый помечен
//!   `#[ignore = "live-keyring: …"]`: дефолтный прогон печатает `0 passed; 0 failed;
//!   12 ignored`, и гейт видит `skip 12` вместо «passed без skip».
//! - В live-фазе (`cargo test … -- --ignored`) рантайм-пропуска нет: [`live_guard`] отдаёт
//!   экземпляр или паникует с типизированной причиной
//!   `KEYRING-LIVE: {not-enabled|service-unavailable|tool-missing}: …` — фаза `failed`.
//!
//! Мутационная проверка опирается на k02 (утечка копии значения не должна расти по RSS),
//! k03 (NULL → `Ok(None)`), k07/k08 (атрибут схемы и CLI-оракул), k11 (double clear) и k12
//! (ручной `Debug` без секрета и без внутренних указателей). Офлайн-часть инвариантов —
//! в `tests/keyring_offline_contract.rs`.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, MutexGuard};

use discord_platform::keyring::{Keyring, KeyringError};

const LIVE_ENV: &str = "DISCORD_LAB_KEYRING_LIVE";
const SCHEMA_NAME: &str = "com.discordlab.Token";
/// Порог роста RSS на серии lookup (КиБ). Утечка копии — 4096+1 байт на вызов.
const RSS_LIMIT_KILOBYTES: u64 = 4 * 1024;
const LEAK_ITERATIONS: usize = 2000;
const LEAK_SECRET_BYTES: usize = 4096;

static SEQ: AtomicU64 = AtomicU64::new(0);
/// Live-кейсы трогают общий сервис и коллекцию: сериализуем их внутри тестового бинарника.
static LIVE_LOCK: Mutex<()> = Mutex::new(());

fn live_enabled() -> bool {
    matches!(std::env::var(LIVE_ENV), Ok(value) if value == "1")
}

/// Уникальный ключ: pid + порядковый номер (кейсы идут параллельно).
fn unique_key(tag: &str) -> String {
    format!(
        "w33a-{tag}-{}-{}",
        std::process::id(),
        SEQ.fetch_add(1, Ordering::Relaxed)
    )
}

fn live_lock() -> MutexGuard<'static, ()> {
    LIVE_LOCK
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// Live-гейт: либо экземпляр, либо паника с одной из трёх различимых причин.
///
/// Рантайм-пропуска здесь принципиально нет (PLAT-02): «нет среды» — это `#[ignore]` на кейсе,
/// а «среда есть, но не готова» — падение live-фазы с префиксом `KEYRING-LIVE:`.
fn live_guard(needs_secret_tool: bool) -> MutexGuard<'static, ()> {
    if !live_enabled() {
        panic!("KEYRING-LIVE: not-enabled: {LIVE_ENV} != 1 (ожидалось 1)");
    }
    if needs_secret_tool && !has_secret_tool() {
        panic!("KEYRING-LIVE: tool-missing: secret-tool не найден в PATH");
    }
    live_lock()
}

/// Подключение к реальному сервису в live-фазе.
fn live_keyring(needs_secret_tool: bool) -> (Keyring, MutexGuard<'static, ()>) {
    let guard = live_guard(needs_secret_tool);
    match Keyring::connect() {
        Ok(keyring) => (keyring, guard),
        Err(error) => panic!("KEYRING-LIVE: service-unavailable: Keyring::connect: {error}"),
    }
}

/// Запись с синтетическим ключом; очищается в `Drop` (best-effort).
struct Entry {
    keyring: Keyring,
    key: String,
}

impl Entry {
    fn new(keyring: Keyring, tag: &str) -> Self {
        Self {
            keyring,
            key: unique_key(tag),
        }
    }

    fn store(&mut self, secret: &str) {
        self.keyring
            .store(&self.key, secret)
            .expect("store синтетического секрета");
    }

    fn lookup(&mut self) -> Option<String> {
        self.keyring.lookup(&self.key).expect("lookup")
    }

    fn clear(&mut self) -> bool {
        self.keyring.clear(&self.key).expect("clear")
    }
}

impl Drop for Entry {
    fn drop(&mut self) {
        // best-effort: если сервис уже недоступен, уборка не должна ронять тест.
        let _ = self.keyring.clear(&self.key);
    }
}

fn has_secret_tool() -> bool {
    let Some(path) = std::env::var_os("PATH") else {
        return false;
    };
    std::env::split_paths(&path).any(|directory| directory.join("secret-tool").is_file())
}

fn secret_tool_lookup(key: &str) -> Option<String> {
    let output = Command::new("secret-tool")
        .args(["lookup", "xdg:schema", "com.discordlab.Token", "key", key])
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }
    let text = String::from_utf8(output.stdout).ok()?;
    Some(text.trim_end_matches(['\n', '\r']).to_owned())
}

/// `secret-tool store` читает сам секрет из stdin, атрибуты — из argv.
fn secret_tool_store(key: &str, secret: &str) -> bool {
    let mut child = match Command::new("secret-tool")
        .args([
            "store",
            "--label",
            "w33a secret-tool oracle",
            "xdg:schema",
            "com.discordlab.Token",
            "key",
            key,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    if let Some(mut stdin) = child.stdin.take() {
        // best-effort: ошибка записи проявится как неуспешный exit secret-tool ниже.
        let _ = stdin.write_all(secret.as_bytes());
    }
    matches!(child.wait_with_output(), Ok(output) if output.status.success())
}

fn secret_tool_clear(key: &str) -> bool {
    matches!(
        Command::new("secret-tool")
            .args([
            "clear",
            "xdg:schema",
            "com.discordlab.Token",
            "key",
            key,
        ])
            .output(),
        Ok(output) if output.status.success()
    )
}

fn rss_kilobytes() -> Option<u64> {
    let status = std::fs::read_to_string("/proc/self/status").ok()?;
    status.lines().find_map(|line| {
        let rest = line.strip_prefix("VmRSS:")?;
        rest.split_whitespace().next()?.parse().ok()
    })
}

#[test]
#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]
fn k01_connect_to_default_collection() {
    let (keyring, _guard) = live_keyring(false);
    assert!(format!("{keyring:?}").contains(SCHEMA_NAME));
}

#[test]
#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]
fn k02_store_then_lookup_roundtrip_without_leak() {
    let (keyring, _guard) = live_keyring(false);
    let mut entry = Entry::new(keyring, "roundtrip");
    entry.store("test-secret-roundtrip");
    assert_eq!(entry.lookup().as_deref(), Some("test-secret-roundtrip"));

    // Повторные lookup не должны накапливать память: копия значения освобождается g_free.
    let big = "x".repeat(LEAK_SECRET_BYTES);
    entry.store(&big);
    for _ in 0..50 {
        assert_eq!(
            entry.lookup().map(|value| value.len()),
            Some(LEAK_SECRET_BYTES)
        );
    }
    let before = rss_kilobytes().expect("VmRSS до серии lookup");
    for _ in 0..LEAK_ITERATIONS {
        let value = entry.lookup().expect("lookup в серии");
        assert_eq!(value.len(), LEAK_SECRET_BYTES);
    }
    let after = rss_kilobytes().expect("VmRSS после серии lookup");
    assert!(
        after.saturating_sub(before) < RSS_LIMIT_KILOBYTES,
        "рост RSS на {LEAK_ITERATIONS} lookup: {before} → {after} КиБ (порог {RSS_LIMIT_KILOBYTES})"
    );
}

#[test]
#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]
fn k03_lookup_missing_returns_none() {
    let (mut keyring, _guard) = live_keyring(false);
    let key = unique_key("missing");
    assert_eq!(keyring.lookup(&key).expect("lookup отсутствующего"), None);
}

#[test]
#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]
fn k04_clear_existing_then_lookup_none() {
    let (keyring, _guard) = live_keyring(false);
    let mut entry = Entry::new(keyring, "clear");
    entry.store("test-secret-clear");
    assert!(
        entry.clear(),
        "clear существующей записи должен вернуть true"
    );
    assert_eq!(entry.lookup(), None);
}

#[test]
#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]
fn k05_overwrite_replaces_value() {
    let (keyring, _guard) = live_keyring(false);
    let mut entry = Entry::new(keyring, "overwrite");
    entry.store("test-secret-first");
    entry.store("test-secret-second");
    assert_eq!(entry.lookup().as_deref(), Some("test-secret-second"));
}

#[test]
#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]
fn k06_unicode_secret_roundtrip() {
    let (keyring, _guard) = live_keyring(false);
    let mut entry = Entry::new(keyring, "unicode");
    let secret = "test-secret-кириллица-ünïcode-🔐-Ω";
    entry.store(secret);
    assert_eq!(entry.lookup().as_deref(), Some(secret));
}

#[test]
#[ignore = "live-keyring: нужны DISCORD_LAB_KEYRING_LIVE=1, org.freedesktop.secrets и CLI secret-tool"]
fn k07_secret_tool_reads_our_store() {
    let (keyring, _guard) = live_keyring(true);
    let mut entry = Entry::new(keyring, "cli-read");
    let secret = "test-secret-cli-read";
    entry.store(secret);
    assert_eq!(secret_tool_lookup(&entry.key).as_deref(), Some(secret));
}

#[test]
#[ignore = "live-keyring: нужны DISCORD_LAB_KEYRING_LIVE=1, org.freedesktop.secrets и CLI secret-tool"]
fn k08_our_lookup_reads_secret_tool_store() {
    let (mut keyring, _guard) = live_keyring(true);
    let key = unique_key("cli-write");
    let secret = "test-secret-cli-write";
    if !secret_tool_store(&key, secret) {
        panic!(
            "KEYRING-LIVE: service-unavailable: secret-tool store не удался (сервис/коллекция недоступны)"
        );
    }
    let found = keyring.lookup(&key).expect("lookup записи secret-tool");
    // best-effort уборка чужими руками: запись создана CLI, а не нашим store.
    let _ = keyring.clear(&key);
    let _ = secret_tool_clear(&key);
    assert_eq!(found.as_deref(), Some(secret));
}

#[test]
#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]
fn k09_empty_key_is_invalid() {
    let (mut keyring, _guard) = live_keyring(false);
    assert_eq!(
        keyring.store("", "test-secret-empty-key"),
        Err(KeyringError::InvalidKey)
    );
    assert_eq!(keyring.lookup(""), Err(KeyringError::InvalidKey));
}

#[test]
#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]
fn k10_nul_in_key_or_secret_is_error_without_panic() {
    let (mut keyring, _guard) = live_keyring(false);
    assert_eq!(
        keyring.store("bad\0key", "test-secret"),
        Err(KeyringError::InvalidKey)
    );
    assert_eq!(keyring.lookup("bad\0key"), Err(KeyringError::InvalidKey));
    assert_eq!(
        keyring.store(&unique_key("nul-secret"), "test-secret\0tail"),
        Err(KeyringError::InvalidSecret)
    );
}

#[test]
#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]
fn k11_repeated_clear_returns_false() {
    let (keyring, _guard) = live_keyring(false);
    let mut entry = Entry::new(keyring, "double-clear");
    entry.store("test-secret-double-clear");
    assert!(entry.clear(), "первый clear — true");
    assert!(
        !entry.clear(),
        "повторный clear отсутствующей записи — false"
    );
    assert_eq!(entry.lookup(), None);
}

#[test]
#[ignore = "live-keyring: нужен DISCORD_LAB_KEYRING_LIVE=1 и org.freedesktop.secrets"]
fn k12_debug_does_not_print_secret_or_pointers() {
    let (keyring, _guard) = live_keyring(false);
    let mut entry = Entry::new(keyring, "debug");
    let secret = "test-secret-debug-do-not-leak";
    entry.store(secret);

    let debug = format!("{:?}", entry.keyring);
    assert!(
        debug.contains("Keyring"),
        "Debug содержит имя типа: {debug}"
    );
    assert!(
        debug.contains(SCHEMA_NAME),
        "Debug содержит имя схемы: {debug}"
    );
    assert!(
        !debug.contains("0x"),
        "Debug без внутренних указателей: {debug}"
    );
    assert!(
        !debug.contains("secret"),
        "Debug без поля/значения секрета: {debug}"
    );
    assert!(!debug.contains(secret), "Debug не печатает секрет: {debug}");

    // Ошибка валидации тоже не должна нести значение секрета.
    let error = entry
        .keyring
        .store(&unique_key("debug-error"), "test-secret\0tail")
        .expect_err("NUL в секрете — ошибка");
    assert_eq!(error, KeyringError::InvalidSecret);
    let error_debug = format!("{error:?}");
    let error_display = error.to_string();
    assert!(!error_debug.contains(secret), "Debug ошибки без секрета");
    assert!(
        !error_display.contains(secret),
        "Display ошибки без секрета"
    );
}
