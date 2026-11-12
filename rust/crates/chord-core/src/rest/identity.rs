//! Честная идентичность клиента и сборка супер-свойств (RW-09 / P1-08).
//!
//! **Каркас координатора (заморожен):** трейт/структуры/сигнатуры — контракт; реализация —
//! воркер W9c (`waves/RW-09/TZ.md` §6). Политика «свои честные значения» — §6.1 ТЗ: среда
//! (ОС/ядро/архитектура/локаль/дистрибутив/дисплей) считывается честно; поля идентичности
//! клиента — свои (имя/версия discord-lab), конфигурируемы; телеметрия — свежие UUID v4.
//!
//! [`ClientIdentity`] — единый носитель этих значений: один экземпляр отдаёт и супер-свойства
//! REST-заголовка, и свойства OP2 IDENTIFY (см. `ClientIdentity::to_identify_properties`).

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

/// Единая идентичность клиента: 22 супер-свойства — единственный источник значений.
///
/// Один экземпляр уходит и в `X-Super-Properties`, и в OP2 IDENTIFY
/// ([`ClientIdentity::to_identify_properties`]). Поэтому `browser`, `os_arch`,
/// `client_launch_id`, `launch_signature` и остальные общие поля не могут разойтись между
/// двумя запросами одной сессии: они физически хранятся в одном месте.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClientIdentity {
    /// Супер-свойства, они же источник полей IDENTIFY.
    super_properties: SuperProperties,
}

impl ClientIdentity {
    /// Идентичность из готовых супер-свойств (например, разобранного живого заголовка).
    #[must_use]
    pub fn new(super_properties: SuperProperties) -> Self {
        Self { super_properties }
    }

    /// Идентичность по политике §6.1: `config.Some(v)` → `v`, иначе — из источников.
    ///
    /// Делегирует [`build`]: UUID генерируются один раз на идентичность, а не на каждый
    /// запрос, поэтому REST и Gateway видят один `client_launch_id`/`launch_signature`.
    #[must_use]
    pub fn build(sources: &dyn IdentitySources, config: &IdentityConfig) -> Self {
        Self::new(build(sources, config))
    }

    /// Дефолтный клиент discord-lab для боевого запуска: честная среда, свои имя и User-Agent.
    ///
    /// Значения `browser`/`browser_user_agent`/`client_build_number` совпадают с прежним
    /// [`IdentifyProperties::for_rw_client`](crate::identify::IdentifyProperties::for_rw_client),
    /// но теперь они же уходят в `X-Super-Properties`.
    ///
    /// Идентичность кэшируется на процесс: повторные вызовы (REST-клиент и Gateway) получают
    /// один и тот же `client_launch_id`/`launch_signature`, а не новый UUID на каждый запрос.
    #[must_use]
    pub fn rw_client() -> Self {
        static RW_CLIENT: std::sync::OnceLock<ClientIdentity> = std::sync::OnceLock::new();
        RW_CLIENT.get_or_init(Self::build_rw_client).clone()
    }

    /// Первичная сборка дефолтной идентичности (ровно один раз на процесс).
    fn build_rw_client() -> Self {
        let version = env!("CARGO_PKG_VERSION");
        Self::build(
            &SystemSources,
            &IdentityConfig {
                browser: Some("Discord Client".to_string()),
                browser_user_agent: Some(format!("discord-lab-rs/{version}")),
                os_version: Some(UNKNOWN.to_string()),
                system_locale: Some("en-US".to_string()),
                client_build_number: Some(641_004),
                ..IdentityConfig::default()
            },
        )
    }

    /// Супер-свойства для `X-Super-Properties` (без клонирования).
    #[must_use]
    pub fn super_properties(&self) -> &SuperProperties {
        &self.super_properties
    }

    /// Копия супер-свойств для заголовка.
    #[must_use]
    pub fn to_super_properties(&self) -> SuperProperties {
        self.super_properties.clone()
    }

    /// Супер-свойства с передачей владения.
    #[must_use]
    pub fn into_super_properties(self) -> SuperProperties {
        self.super_properties
    }
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

#[cfg(test)]
mod tests {
    //! Единая идентичность: один `ClientIdentity` кормит и REST, и Gateway IDENTIFY.

    use super::*;
    use std::cell::Cell;

    const KERNEL_PATH: &str = "/proc/sys/kernel/osrelease";

    /// Детерминированные источники: фиксированные ОС/архитектура, окружение, файлы и UUID.
    struct FakeSources {
        os: &'static str,
        arch: &'static str,
        env: Vec<(&'static str, &'static str)>,
        files: Vec<(&'static str, &'static str)>,
        random: Vec<[u8; 16]>,
        calls: Cell<usize>,
    }

    impl FakeSources {
        fn new(os: &'static str, arch: &'static str, random: &[[u8; 16]]) -> Self {
            Self {
                os,
                arch,
                env: Vec::new(),
                files: Vec::new(),
                random: random.to_vec(),
                calls: Cell::new(0),
            }
        }

        fn env(mut self, name: &'static str, value: &'static str) -> Self {
            self.env.push((name, value));
            self
        }

        fn file(mut self, path: &'static str, text: &'static str) -> Self {
            self.files.push((path, text));
            self
        }

        fn lookup(table: &[(&'static str, &'static str)], name: &str) -> Option<String> {
            table
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_string())
        }
    }

    impl IdentitySources for FakeSources {
        fn env_var(&self, name: &str) -> Option<String> {
            Self::lookup(&self.env, name)
        }

        fn read_text(&self, path: &str) -> Option<String> {
            Self::lookup(&self.files, path)
        }

        fn random_bytes(&self, out: &mut [u8]) {
            let index = self.calls.get();
            self.calls.set(index + 1);
            let pattern = self
                .random
                .get(index)
                .unwrap_or_else(|| panic!("FakeSources: нет шаблона для вызова #{index}"));
            out.copy_from_slice(pattern);
        }

        fn os(&self) -> &'static str {
            self.os
        }

        fn arch(&self) -> &'static str {
            self.arch
        }
    }

    const UUID_BYTES: [[u8; 16]; 3] = [
        [
            0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d,
            0x0e, 0x0f,
        ],
        [
            0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d,
            0x1e, 0x1f,
        ],
        [
            0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d,
            0x2e, 0x2f,
        ],
    ];

    fn sources() -> FakeSources {
        FakeSources::new("linux", "x86_64", &UUID_BYTES)
            .env("LANG", "ru_RU.UTF-8")
            .file(KERNEL_PATH, "6.16.7-arch1-1\n")
    }

    #[test]
    fn client_identity_builds_rest_and_identify_from_one_set_of_uuids() {
        let sources = sources();
        let identity = ClientIdentity::build(&sources, &IdentityConfig::default());
        let sp = identity.super_properties().clone();

        assert_eq!(
            sources.calls.get(),
            3,
            "ровно три UUID: client_launch_id, launch_signature, client_heartbeat_session_id"
        );
        assert_eq!(sp.client_launch_id, "00010203-0405-4607-8809-0a0b0c0d0e0f");
        assert_eq!(sp.launch_signature, "10111213-1415-4617-9819-1a1b1c1d1e1f");
        assert_eq!(
            sp.client_heartbeat_session_id,
            "20212223-2425-4627-a829-2a2b2c2d2e2f"
        );

        let identify = identity.to_identify_properties();
        assert_eq!(identify.client_launch_id, sp.client_launch_id);
        assert_eq!(identify.launch_signature, sp.launch_signature);
        assert_eq!(identify.browser, sp.browser);
        assert_eq!(identify.os_arch, sp.os_arch);
        assert_eq!(identify.system_locale, sp.system_locale);
        assert_eq!(identify.browser_user_agent, sp.browser_user_agent);
        assert_eq!(
            sources.calls.get(),
            3,
            "IDENTIFY не генерирует свои UUID — берёт готовую идентичность"
        );
    }

    #[test]
    fn build_stays_a_wrapper_over_client_identity() {
        let sp = build(&sources(), &IdentityConfig::default());
        let identity = ClientIdentity::build(&sources(), &IdentityConfig::default());
        assert_eq!(sp, *identity.super_properties());
    }
}
