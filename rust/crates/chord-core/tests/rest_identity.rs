//! Честная идентичность и сборка супер-свойств (RW-09 / P1-08, кейсы I1–I8).
//!
//! **Провенанс (независимые оракулы, не подгонка под реализацию):**
//! - Политика «свои честные значения» заморожена в `waves/RW-09/TZ.md` §6.1
//!   (md5 `5dc281581e7c0ee88be77ac39e373ce3`); контракт — каркас `src/rest/identity.rs`
//!   (md5 заглушки `06d80ecf79ec57377c67c76f16f0ef28`).
//! - Среда в тестах фейковая и детерминированная: `env`/файлы — литералы §6.1,
//!   `random_bytes` отдаёт последовательно `0x00..0x0F`, `0x10..0x1F`, `0x20..0x2F`.
//!   UUID-оракул посчитан вручную по RFC 4122 v4: версия `(b[6] & 0x0f) | 0x40`,
//!   вариант `(b[8] & 0x3f) | 0x80`, формат `8-4-4-4-12`, hex в нижнем регистре.
//!   Контроль маскирования — все `0xFF` → `ffffffff-ffff-4fff-bfff-ffffffffffff`.
//! - Живой снимок, из которого взяты наблюдаемые поля (22 ключа, base64 1084 симв.):
//!   `/home/sanya/re-deep/d02-snapshot/capture-run2.snap.jsonl`, md5
//!   `abfe14b76cec1e559c3b45522bc44b7a`, requestId `95974.2323`; человекочитаемый разбор —
//!   `docs/re/rest-d02.md` §6.2 (md5 `c7e61929677efab6bdac86a05a778fce`). Поля идентичности
//!   нашего клиента намеренно **свои** (`discord-lab`, версия крейта, build 0) — §6.1.
//! - I8 проверяет стык с W9a/W9b: `SuperProperties::to_base64`/`from_base64` и сборку
//!   `AuthHeaderValues` из `rest::headers` (6 пар, значение `X-Super-Properties` непустое).
//!
//! Внешней среды нет: все источники — `FakeSources`; тест-данные не читаются.

use chord_core::rest::headers::{AUTH_HEADER_NAMES, AuthHeaderValues, auth_headers};
use chord_core::rest::identity::{IdentityConfig, IdentitySources, build, uuid_v4};
use chord_core::rest::super_properties::SuperProperties;
use std::cell::Cell;

/// Версия крейта `discord-core` из `Cargo.toml` (workspace 0.1.0) — политика §6.1:
/// `client_version = env!("CARGO_PKG_VERSION")`.
const EXPECTED_VERSION: &str = "0.1.0";

const KERNEL_PATH: &str = "/proc/sys/kernel/osrelease";
const OS_RELEASE_PATH: &str = "/etc/os-release";

/// Байты первого UUID: `0x00..0x0F`.
const UUID_BYTES_1: [u8; 16] = [
    0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x09, 0x0a, 0x0b, 0x0c, 0x0d, 0x0e, 0x0f,
];
/// Байты второго UUID: `0x10..0x1F`.
const UUID_BYTES_2: [u8; 16] = [
    0x10, 0x11, 0x12, 0x13, 0x14, 0x15, 0x16, 0x17, 0x18, 0x19, 0x1a, 0x1b, 0x1c, 0x1d, 0x1e, 0x1f,
];
/// Байты третьего UUID: `0x20..0x2F`.
const UUID_BYTES_3: [u8; 16] = [
    0x20, 0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f,
];
/// Запасной детерминированный шаблон для кейсов, где UUID не проверяются.
const DEFAULT_RANDOM: [u8; 16] = [0x5a; 16];

/// Фейковые источники: переменные окружения и файлы — точные литералы, random — очередь шаблонов.
struct FakeSources {
    os: &'static str,
    arch: &'static str,
    env: Vec<(&'static str, &'static str)>,
    files: Vec<(&'static str, &'static str)>,
    random: Vec<Vec<u8>>,
    random_calls: Cell<usize>,
}

impl FakeSources {
    fn new(os: &'static str, arch: &'static str) -> Self {
        Self {
            os,
            arch,
            env: Vec::new(),
            files: Vec::new(),
            random: vec![DEFAULT_RANDOM.to_vec(); 4],
            random_calls: Cell::new(0),
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

    fn random(mut self, patterns: &[&[u8]]) -> Self {
        self.random = patterns.iter().map(|pattern| pattern.to_vec()).collect();
        self
    }

    fn random_calls(&self) -> usize {
        self.random_calls.get()
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
        let index = self.random_calls.get();
        self.random_calls.set(index + 1);
        let pattern = self
            .random
            .get(index)
            .unwrap_or_else(|| panic!("FakeSources: нет шаблона байтов для вызова #{index}"));
        assert_eq!(
            pattern.len(),
            out.len(),
            "FakeSources: шаблон #{index} должен совпадать с длиной буфера"
        );
        out.copy_from_slice(pattern);
    }

    fn os(&self) -> &'static str {
        self.os
    }

    fn arch(&self) -> &'static str {
        self.arch
    }
}

/// Полный набор источников I1: Wayland + KDE, Arch Linux, ядро 6.16.7, локаль ru_RU.UTF-8.
fn wayland_kde_sources() -> FakeSources {
    FakeSources::new("linux", "x86_64")
        .env("LANG", "ru_RU.UTF-8")
        .env("XDG_SESSION_TYPE", "wayland")
        .env("XDG_CURRENT_DESKTOP", "KDE")
        .file(KERNEL_PATH, "6.16.7-arch1-1\n")
        .file(
            OS_RELEASE_PATH,
            "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\n",
        )
        .random(&[&UUID_BYTES_1, &UUID_BYTES_2, &UUID_BYTES_3])
}

/// Ожидаемая структура I1, выписанная по таблице §6.1 (не через `build`).
fn expected_wayland_kde() -> SuperProperties {
    SuperProperties {
        os: "Linux".to_string(),
        browser: "discord-lab".to_string(),
        release_channel: "stable".to_string(),
        client_version: EXPECTED_VERSION.to_string(),
        os_version: "6.16.7-arch1-1".to_string(),
        os_arch: "x64".to_string(),
        app_arch: "x64".to_string(),
        system_locale: "ru".to_string(),
        has_client_mods: false,
        client_launch_id: "00010203-0405-4607-8809-0a0b0c0d0e0f".to_string(),
        browser_user_agent: format!("discord-lab/{EXPECTED_VERSION} (Linux; x64)"),
        browser_version: EXPECTED_VERSION.to_string(),
        window_manager: "KDE".to_string(),
        distro: "\"Arch Linux\"".to_string(),
        runtime_environment: "native".to_string(),
        display_server: "wayland".to_string(),
        client_build_number: 0,
        native_build_number: 0,
        client_event_source: None,
        launch_signature: "10111213-1415-4617-9819-1a1b1c1d1e1f".to_string(),
        client_heartbeat_session_id: "20212223-2425-4627-a829-2a2b2c2d2e2f".to_string(),
        client_app_state: "focused".to_string(),
    }
}

/// Локаль по фейковому окружению (остальные поля не важны).
fn locale_with(env: &[(&'static str, &'static str)]) -> String {
    let mut sources = FakeSources::new("linux", "x86_64");
    for (name, value) in env {
        sources = sources.env(name, value);
    }
    build(&sources, &IdentityConfig::default()).system_locale
}

/// Формат UUID: 5 групп 8-4-4-4-12, только `0-9a-f`, версия 4, вариант RFC 4122.
fn assert_uuid_shape(value: &str) {
    let groups: Vec<&str> = value.split('-').collect();
    assert_eq!(
        groups.iter().map(|group| group.len()).collect::<Vec<_>>(),
        [8, 4, 4, 4, 12],
        "UUID {value}: формат 8-4-4-4-12"
    );
    assert_eq!(value.len(), 36, "UUID {value}: 36 символов");
    assert!(
        groups.iter().all(|group| group
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))),
        "UUID {value}: только 0-9a-f в нижнем регистре"
    );
    assert_eq!(
        groups[2].as_bytes()[0],
        b'4',
        "UUID {value}: версия 4 в третьей группе"
    );
    assert!(
        matches!(groups[3].as_bytes()[0], b'8' | b'9' | b'a' | b'b'),
        "UUID {value}: вариант RFC 4122 (8/9/a/b) в четвёртой группе"
    );
}

/// I1: полная сборка «Wayland + KDE + Arch» — каждое из 22 полей по §6.1, три UUID из трёх
/// последовательных вызовов `random_bytes`.
#[test]
fn wayland_kde_full_build() {
    assert_eq!(
        EXPECTED_VERSION,
        env!("CARGO_PKG_VERSION"),
        "политика §6.1: client_version = версия крейта"
    );

    let sources = wayland_kde_sources();
    let actual = build(&sources, &IdentityConfig::default());
    assert_eq!(actual, expected_wayland_kde());
    assert_eq!(
        sources.random_calls(),
        3,
        "ровно три UUID: client_launch_id, launch_signature, client_heartbeat_session_id"
    );
}

/// I2: каскад `display_server`: `XDG_SESSION_TYPE` (дословно) → `WAYLAND_DISPLAY` → `DISPLAY`
/// → `unknown`.
#[test]
fn display_server_fallbacks() {
    let wayland = FakeSources::new("linux", "x86_64").env("WAYLAND_DISPLAY", "wayland-0");
    assert_eq!(
        build(&wayland, &IdentityConfig::default()).display_server,
        "wayland",
        "WAYLAND_DISPLAY без XDG_SESSION_TYPE → wayland"
    );

    let x11 = FakeSources::new("linux", "x86_64").env("DISPLAY", ":0");
    assert_eq!(
        build(&x11, &IdentityConfig::default()).display_server,
        "x11",
        "только DISPLAY → x11"
    );

    let none = FakeSources::new("linux", "x86_64");
    assert_eq!(
        build(&none, &IdentityConfig::default()).display_server,
        "unknown",
        "ни одной переменной → unknown"
    );

    let explicit = FakeSources::new("linux", "x86_64")
        .env("XDG_SESSION_TYPE", "x11")
        .env("WAYLAND_DISPLAY", "wayland-0")
        .env("DISPLAY", ":0");
    assert_eq!(
        build(&explicit, &IdentityConfig::default()).display_server,
        "x11",
        "XDG_SESSION_TYPE приоритетнее WAYLAND_DISPLAY"
    );

    let tty = FakeSources::new("linux", "x86_64")
        .env("XDG_SESSION_TYPE", "tty")
        .env("WAYLAND_DISPLAY", "wayland-0");
    assert_eq!(
        build(&tty, &IdentityConfig::default()).display_server,
        "tty",
        "XDG_SESSION_TYPE сохраняется дословно"
    );
}

/// I3: локаль — `LC_ALL` → `LANG` → `unknown`; обрезка после первого `.`/`@`, затем после `_`.
#[test]
fn locale_parsing() {
    assert_eq!(locale_with(&[("LANG", "ru_RU.UTF-8")]), "ru");
    assert_eq!(locale_with(&[("LANG", "C")]), "C");
    assert_eq!(locale_with(&[("LANG", "en_US@euro")]), "en");
    assert_eq!(
        locale_with(&[("LC_ALL", "de_DE.UTF-8"), ("LANG", "ru_RU.UTF-8")]),
        "de",
        "LC_ALL приоритетнее LANG"
    );
    assert_eq!(
        locale_with(&[("LC_ALL", "C"), ("LANG", "ru_RU.UTF-8")]),
        "C",
        "LC_ALL=C — валидная локаль, не отсутствие"
    );
    assert_eq!(locale_with(&[("LANG", "ru_RU")]), "ru");
    assert_eq!(locale_with(&[]), "unknown");
}

/// I4: отображение `os` и `arch`: linux→Linux, macos→Mac OS X, windows→Windows; иначе как есть;
/// x86_64→x64, aarch64→arm64, x86→x86; `app_arch` повторяет `os_arch`.
#[test]
fn os_arch_mapping() {
    let cases = [
        ("linux", "x86_64", "Linux", "x64"),
        ("macos", "aarch64", "Mac OS X", "arm64"),
        ("windows", "x86", "Windows", "x86"),
        ("freebsd", "riscv64", "freebsd", "riscv64"),
    ];
    for (os, arch, expected_os, expected_arch) in cases {
        let properties = build(&FakeSources::new(os, arch), &IdentityConfig::default());
        assert_eq!(properties.os, expected_os, "{os}: os");
        assert_eq!(properties.os_arch, expected_arch, "{os}/{arch}: os_arch");
        assert_eq!(
            properties.app_arch, expected_arch,
            "{os}/{arch}: app_arch = os_arch"
        );
    }
}

/// I5: дистрибутив — `PRETTY_NAME` дословно (кавычки сохраняются); нет файла или нет ключа →
/// `unknown`.
#[test]
fn distro_sources() {
    let quoted = FakeSources::new("linux", "x86_64").file(
        OS_RELEASE_PATH,
        "NAME=\"Arch Linux\"\nPRETTY_NAME=\"Arch Linux\"\n",
    );
    assert_eq!(
        build(&quoted, &IdentityConfig::default()).distro,
        "\"Arch Linux\"",
        "PRETTY_NAME дословно, кавычки сохраняются"
    );

    let missing = FakeSources::new("linux", "x86_64");
    assert_eq!(
        build(&missing, &IdentityConfig::default()).distro,
        "unknown",
        "нет /etc/os-release → unknown"
    );

    let without_pretty = FakeSources::new("linux", "x86_64")
        .file(OS_RELEASE_PATH, "NAME=\"Debian GNU/Linux\"\nID=debian\n");
    assert_eq!(
        build(&without_pretty, &IdentityConfig::default()).distro,
        "unknown",
        "нет PRETTY_NAME → unknown"
    );

    let unquoted =
        FakeSources::new("linux", "x86_64").file(OS_RELEASE_PATH, "PRETTY_NAME=Alpine Linux\n");
    assert_eq!(
        build(&unquoted, &IdentityConfig::default()).distro,
        "Alpine Linux",
        "значение без кавычек — как есть"
    );
}

/// I6: UUID v4 — точные строки I1, формат/регистр, маскирование версии/варианта на всех `0xFF`,
/// разные байты → разные UUID.
#[test]
fn uuid_v4_shape() {
    let first = FakeSources::new("linux", "x86_64").random(&[&UUID_BYTES_1]);
    let first_uuid = uuid_v4(&first);
    assert_eq!(first_uuid, "00010203-0405-4607-8809-0a0b0c0d0e0f");
    assert_uuid_shape(&first_uuid);

    let second = FakeSources::new("linux", "x86_64").random(&[&UUID_BYTES_2]);
    let second_uuid = uuid_v4(&second);
    assert_eq!(second_uuid, "10111213-1415-4617-9819-1a1b1c1d1e1f");
    assert_uuid_shape(&second_uuid);

    let third = FakeSources::new("linux", "x86_64").random(&[&UUID_BYTES_3]);
    let third_uuid = uuid_v4(&third);
    assert_eq!(third_uuid, "20212223-2425-4627-a829-2a2b2c2d2e2f");
    assert_uuid_shape(&third_uuid);

    assert_ne!(first_uuid, second_uuid, "разные байты → разные UUID");
    assert_ne!(second_uuid, third_uuid, "разные байты → разные UUID");

    let all_ones = [0xff_u8; 16];
    let masked = FakeSources::new("linux", "x86_64").random(&[&all_ones]);
    let masked_uuid = uuid_v4(&masked);
    assert_eq!(
        masked_uuid, "ffffffff-ffff-4fff-bfff-ffffffffffff",
        "версия и вариант маскируются, а не OR-ятся поверх старших битов"
    );
    assert_uuid_shape(&masked_uuid);
}

/// I7: конфиг с 22 заполненными полями побеждает и env/файлы, и константы; `u32` и
/// `client_event_source = Some(...)` возвращаются дословно.
#[test]
fn config_overrides_win() {
    let config = IdentityConfig {
        os: Some("Plan9".to_string()),
        browser: Some("probe-browser".to_string()),
        release_channel: Some("beta".to_string()),
        client_version: Some("9.9.9".to_string()),
        os_version: Some("kernel-x".to_string()),
        os_arch: Some("mips".to_string()),
        app_arch: Some("app-mips".to_string()),
        system_locale: Some("xx_XX".to_string()),
        has_client_mods: Some(true),
        client_launch_id: Some("00000000-0000-4000-8000-000000000001".to_string()),
        browser_user_agent: Some("probe-agent".to_string()),
        browser_version: Some("probe-engine".to_string()),
        window_manager: Some("probe-wm".to_string()),
        distro: Some("Probe Distro".to_string()),
        runtime_environment: Some("probe-runtime".to_string()),
        display_server: Some("probe-display".to_string()),
        client_build_number: Some(641_004),
        native_build_number: Some(94_186),
        client_event_source: Some("probe".to_string()),
        launch_signature: Some("00000000-0000-4000-8000-000000000002".to_string()),
        client_heartbeat_session_id: Some("00000000-0000-4000-8000-000000000003".to_string()),
        client_app_state: Some("unfocused".to_string()),
    };
    let sources = FakeSources::new("linux", "x86_64")
        .env("LANG", "ru_RU.UTF-8")
        .env("XDG_SESSION_TYPE", "wayland")
        .env("XDG_CURRENT_DESKTOP", "KDE")
        .file(KERNEL_PATH, "6.16.7-arch1-1\n")
        .file(OS_RELEASE_PATH, "PRETTY_NAME=\"Arch Linux\"\n");

    let properties = build(&sources, &config);
    let expected = SuperProperties {
        os: "Plan9".to_string(),
        browser: "probe-browser".to_string(),
        release_channel: "beta".to_string(),
        client_version: "9.9.9".to_string(),
        os_version: "kernel-x".to_string(),
        os_arch: "mips".to_string(),
        app_arch: "app-mips".to_string(),
        system_locale: "xx_XX".to_string(),
        has_client_mods: true,
        client_launch_id: "00000000-0000-4000-8000-000000000001".to_string(),
        browser_user_agent: "probe-agent".to_string(),
        browser_version: "probe-engine".to_string(),
        window_manager: "probe-wm".to_string(),
        distro: "Probe Distro".to_string(),
        runtime_environment: "probe-runtime".to_string(),
        display_server: "probe-display".to_string(),
        client_build_number: 641_004,
        native_build_number: 94_186,
        client_event_source: Some("probe".to_string()),
        launch_signature: "00000000-0000-4000-8000-000000000002".to_string(),
        client_heartbeat_session_id: "00000000-0000-4000-8000-000000000003".to_string(),
        client_app_state: "unfocused".to_string(),
    };
    assert_eq!(
        properties, expected,
        "все 22 поля из конфига — без примесей"
    );
}

/// I8: стык с W9a/W9b — `build` → `to_base64` → `from_base64` без потерь; значение
/// `X-Super-Properties` собирается в 6 пар `rest::headers` и непустое.
#[test]
fn integration_base64_headers() {
    let properties = build(&wayland_kde_sources(), &IdentityConfig::default());
    let encoded = properties.to_base64();
    assert!(!encoded.is_empty(), "X-Super-Properties не пуст");

    let decoded =
        SuperProperties::from_base64(&encoded).expect("round-trip base64 → JSON → 22 поля");
    assert_eq!(decoded, properties, "структура после round-trip совпадает");
    assert_eq!(
        decoded.to_base64(),
        encoded,
        "канонический base64 стабилен при повторной сборке"
    );

    let values = AuthHeaderValues {
        authorization: "probe-token",
        super_properties: &encoded,
        locale: "ru",
        timezone: "Europe/Moscow",
        debug_options: "",
        accept_language: "ru",
    };
    let pairs = auth_headers(&values);
    assert_eq!(pairs.len(), 6, "6 пар auth-заголовков");
    for (index, (name, _)) in pairs.iter().enumerate() {
        assert_eq!(*name, AUTH_HEADER_NAMES[index], "имя пары #{index}");
    }
    assert_eq!(pairs[1].0, "X-Super-Properties");
    assert_eq!(pairs[1].1, encoded.as_str());
    assert!(
        !pairs[1].1.is_empty(),
        "значение X-Super-Properties в headers непустое"
    );
}
