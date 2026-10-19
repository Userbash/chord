//! Честная идентичность клиента и сборка супер-свойств (RW-09 / P1-08).
//!
//! **Каркас координатора (заморожен):** трейт/структуры/сигнатуры — контракт; реализация —
//! воркер W9c (`waves/RW-09/TZ.md` §6). Политика «свои честные значения» — §6.1 ТЗ: среда
//! (ОС/ядро/архитектура/локаль/дистрибутив/дисплей) считывается честно; поля идентичности
//! клиента — свои (имя/версия discord-lab), конфигурируемы; телеметрия — свежие UUID v4.

use super::super_properties::SuperProperties;

/// Источники значений: реальная система или детерминированная подделка в тестах.
pub trait IdentitySources {
    /// Переменная окружения.
    fn env_var(&self, name: &str) -> Option<String>;
    /// Содержимое текстового файла (пути вроде `/proc/sys/kernel/osrelease`).
    fn read_text(&self, path: &str) -> Option<String>;
    /// Криптостойкие случайные байты (fallback — оставить буфер как есть).
    fn random_bytes(&self, out: &mut [u8]);
    /// `std::env::consts::OS` (`linux`/`macos`/`windows`/…).
    fn os(&self) -> &'static str;
    /// `std::env::consts::ARCH` (`x86_64`/`aarch64`/…).
    fn arch(&self) -> &'static str;
}

/// Реальные источники: `std::env`, `std::fs`, `/dev/urandom`.
#[derive(Debug, Default)]
pub struct SystemSources;

impl SystemSources {
    /// Новые системные источники.
    #[must_use]
    pub fn new() -> Self {
        Self
    }
}

impl IdentitySources for SystemSources {
    fn env_var(&self, name: &str) -> Option<String> {
        std::env::var(name).ok()
    }

    fn read_text(&self, path: &str) -> Option<String> {
        std::fs::read_to_string(path).ok()
    }

    fn random_bytes(&self, out: &mut [u8]) {
        use std::io::Read;
        if let Ok(mut file) = std::fs::File::open("/dev/urandom") {
            let _ = file.read_exact(out); // best-effort: при сбое буфер остаётся как пришёл
        }
    }

    fn os(&self) -> &'static str {
        std::env::consts::OS
    }

    fn arch(&self) -> &'static str {
        std::env::consts::ARCH
    }
}

/// Конфигурация идентичности: `None` → честное значение из источников (§6.1).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct IdentityConfig {
    /// ОС клиента.
    pub os: Option<String>,
    /// Имя клиента.
    pub browser: Option<String>,
    /// Канал обновлений.
    pub release_channel: Option<String>,
    /// Версия клиента.
    pub client_version: Option<String>,
    /// Версия ядра/ОС.
    pub os_version: Option<String>,
    /// Архитектура ОС.
    pub os_arch: Option<String>,
    /// Архитектура приложения.
    pub app_arch: Option<String>,
    /// Системная локаль.
    pub system_locale: Option<String>,
    /// Признак модов клиента.
    pub has_client_mods: Option<bool>,
    /// UUID запуска клиента.
    pub client_launch_id: Option<String>,
    /// Полный User-Agent.
    pub browser_user_agent: Option<String>,
    /// Версия Chromium/Electron-слоя.
    pub browser_version: Option<String>,
    /// Оконный менеджер.
    pub window_manager: Option<String>,
    /// Дистрибутив.
    pub distro: Option<String>,
    /// Среда выполнения.
    pub runtime_environment: Option<String>,
    /// Сервер отображения.
    pub display_server: Option<String>,
    /// Номер сборки клиента.
    pub client_build_number: Option<u32>,
    /// Номер нативной сборки.
    pub native_build_number: Option<u32>,
    /// Источник события клиента.
    pub client_event_source: Option<String>,
    /// UUID подписи запуска.
    pub launch_signature: Option<String>,
    /// UUID сессии телеметрии.
    pub client_heartbeat_session_id: Option<String>,
    /// Состояние окна клиента.
    pub client_app_state: Option<String>,
}

/// Значение для неизвестных строковых полей (§6.1).
const UNKNOWN: &str = "unknown";

/// Версия ядра Linux (`os_version`).
const KERNEL_RELEASE_PATH: &str = "/proc/sys/kernel/osrelease";

/// Описание дистрибутива (`distro` — `PRETTY_NAME`).
const OS_RELEASE_PATH: &str = "/etc/os-release";

/// Непустое значение: пустая строка трактуется как отсутствие источника.
fn non_empty(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.trim().is_empty())
}

/// Первая непустая переменная окружения из списка (приоритет — порядок в `names`).
fn first_env(sources: &dyn IdentitySources, names: &[&str]) -> Option<String> {
    names
        .iter()
        .find_map(|name| non_empty(sources.env_var(name)))
}

/// Есть ли непустая переменная окружения.
fn has_env(sources: &dyn IdentitySources, name: &str) -> bool {
    non_empty(sources.env_var(name)).is_some()
}

/// `Some(value)` из конфига побеждает; `None` → значение по политике §6.1 (лениво).
fn configured_or(value: &Option<String>, fallback: impl FnOnce() -> String) -> String {
    value.clone().unwrap_or_else(fallback)
}

/// Значение или [`UNKNOWN`] — единая подстановка для строковых полей среды.
fn or_unknown(value: Option<String>) -> String {
    value.unwrap_or_else(|| UNKNOWN.to_string())
}

/// Отображение `std::env::consts::OS` в имя ОС клиента (§6.1).
fn map_os(os: &str) -> String {
    match os {
        "linux" => "Linux",
        "macos" => "Mac OS X",
        "windows" => "Windows",
        other => other,
    }
    .to_string()
}

/// Отображение архитектуры: `x86_64→x64`, `aarch64→arm64`, `x86→x86`, иначе как есть.
fn map_arch(arch: &str) -> String {
    match arch {
        "x86_64" => "x64",
        "aarch64" => "arm64",
        "x86" => "x86",
        other => other,
    }
    .to_string()
}

/// Разбор локали: обрезать после первого `.`/`@`, затем после `_` (`ru_RU.UTF-8` → `ru`).
fn parse_locale(raw: &str) -> String {
    let end = raw.find(['.', '@']).unwrap_or(raw.len());
    let base = &raw[..end];
    let lang_end = base.find('_').unwrap_or(base.len());
    let locale = &base[..lang_end];
    if locale.is_empty() {
        UNKNOWN.to_string()
    } else {
        locale.to_string()
    }
}

/// Значение `PRETTY_NAME` из `/etc/os-release` дословно (кавычки — часть значения).
fn parse_pretty_name(text: &str) -> Option<String> {
    text.lines().find_map(|line| {
        line.strip_suffix('\r')
            .unwrap_or(line)
            .strip_prefix("PRETTY_NAME=")
            .filter(|value| !value.is_empty())
            .map(str::to_string)
    })
}

/// Версия ядра из `/proc/sys/kernel/osrelease` (trim); нет файла → `unknown`.
fn resolve_os_version(sources: &dyn IdentitySources) -> String {
    or_unknown(non_empty(
        sources
            .read_text(KERNEL_RELEASE_PATH)
            .map(|text| text.trim().to_string()),
    ))
}

/// Дистрибутив из `PRETTY_NAME`; нет файла или ключа → `unknown`.
fn resolve_distro(sources: &dyn IdentitySources) -> String {
    or_unknown(
        sources
            .read_text(OS_RELEASE_PATH)
            .and_then(|text| parse_pretty_name(&text)),
    )
}

/// Каскад `display_server`: `XDG_SESSION_TYPE` → `WAYLAND_DISPLAY` → `DISPLAY` → `unknown`.
fn resolve_display_server(sources: &dyn IdentitySources) -> String {
    if let Some(value) = non_empty(sources.env_var("XDG_SESSION_TYPE")) {
        return value;
    }
    if has_env(sources, "WAYLAND_DISPLAY") {
        return "wayland".to_string();
    }
    if has_env(sources, "DISPLAY") {
        return "x11".to_string();
    }
    UNKNOWN.to_string()
}

/// UUID v4 (RFC 4122) из `sources.random_bytes`: версия 4, вариант RFC 4122, hex в нижнем
/// регистре. Формат `8-4-4-4-12`.
#[must_use]
pub fn uuid_v4(sources: &dyn IdentitySources) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut bytes = [0_u8; 16];
    sources.random_bytes(&mut bytes);
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;

    let mut out = String::with_capacity(36);
    for (index, byte) in bytes.iter().enumerate() {
        if matches!(index, 4 | 6 | 8 | 10) {
            out.push('-');
        }
        out.push(char::from(HEX[usize::from(*byte >> 4)]));
        out.push(char::from(HEX[usize::from(*byte & 0x0f)]));
    }
    out
}

/// Собирает супер-свойства по политике «свои честные значения» (§6.1): `config.Some(v)` → `v`,
/// иначе — значение из источников/константа. Три UUID генерируются в каноническом порядке
/// полей: `client_launch_id`, `launch_signature`, `client_heartbeat_session_id`.
#[must_use]
pub fn build(sources: &dyn IdentitySources, config: &IdentityConfig) -> SuperProperties {
    let os = configured_or(&config.os, || map_os(sources.os()));
    let os_arch = configured_or(&config.os_arch, || map_arch(sources.arch()));
    let client_version = configured_or(&config.client_version, || {
        env!("CARGO_PKG_VERSION").to_string()
    });
    let app_arch = configured_or(&config.app_arch, || os_arch.clone());
    let browser_version = configured_or(&config.browser_version, || client_version.clone());
    let default_user_agent = format!("discord-lab/{client_version} ({os}; {os_arch})");
    let system_locale = configured_or(&config.system_locale, || {
        or_unknown(first_env(sources, &["LC_ALL", "LANG"]).map(|raw| parse_locale(&raw)))
    });

    // Порядок вызовов фиксирован: он определяет, какие байты получает каждый UUID.
    let client_launch_id = configured_or(&config.client_launch_id, || uuid_v4(sources));
    let launch_signature = configured_or(&config.launch_signature, || uuid_v4(sources));
    let client_heartbeat_session_id =
        configured_or(&config.client_heartbeat_session_id, || uuid_v4(sources));

    SuperProperties {
        os,
        browser: configured_or(&config.browser, || "discord-lab".to_string()),
        release_channel: configured_or(&config.release_channel, || "stable".to_string()),
        client_version,
        os_version: configured_or(&config.os_version, || resolve_os_version(sources)),
        os_arch,
        app_arch,
        system_locale,
        has_client_mods: config.has_client_mods.unwrap_or(false),
        client_launch_id,
        browser_user_agent: configured_or(&config.browser_user_agent, || default_user_agent),
        browser_version,
        window_manager: configured_or(&config.window_manager, || {
            or_unknown(non_empty(sources.env_var("XDG_CURRENT_DESKTOP")))
        }),
        distro: configured_or(&config.distro, || resolve_distro(sources)),
        runtime_environment: configured_or(&config.runtime_environment, || "native".to_string()),
        display_server: configured_or(&config.display_server, || resolve_display_server(sources)),
        client_build_number: config.client_build_number.unwrap_or(0),
        native_build_number: config.native_build_number.unwrap_or(0),
        client_event_source: config.client_event_source.clone(),
        launch_signature,
        client_heartbeat_session_id,
        client_app_state: configured_or(&config.client_app_state, || "focused".to_string()),
    }
}
