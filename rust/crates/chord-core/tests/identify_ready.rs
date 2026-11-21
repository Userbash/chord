//! Идентификация шлюза Discord: IDENTIFY (op2) и сводка READY (волна RW-05, P1-04 /
//! T-GW-02, T-GW-04).
//!
//! **Провенанс оракулов** (независимые артефакты, не подгонка под реализацию):
//! - структура IDENTIFY — код реального веб-клиента Discord (бандл
//!   `discord.com_assets_web.4d4b2f75fdea5470.js`): `_doIdentify` (~5017678) собирает
//!   `{token, capabilities, properties, presence, compress, client_state, qos_token}`;
//!   `handleIdentify` (~4907630) — свойства `{...superProperties, client_app_state,
//!   is_fast_connect:false, gateway_connect_reasons, installation_id?}`; модуль 469177
//!   (~5025654) — `gateway_connect_reasons` **строкой** (`r=true` → `"COLD_START"`);
//!   QoS-типы protobuf `discord_protos.qos_token.v1` (~5000400); capabilities
//!   1 734 653 / 1 767 421; `compress` = не legacy (zstd-поток) = false.
//! - READY — снимок потока `{RE_LAB_ROOT:-/home/sanya/re-deep}/gw-proto/zstd-frames-95974_1389.hexl`
//!   (137 кадров; второй терм — READY, s=1); контрольные числа перепроверены по
//!   `gw-proto/msgs-run2-95974_1389.jsonl`.
//! - стаб-сервер: только `127.0.0.1`, все сетевые кейсы — под `tokio::time::timeout(5 с)`.
//!
//! Lab-файлы опциональны: нет файла — skip с причиной, а не падение.

use chord_core::codec::etf::{self, EtfValue};
use chord_core::gateway::Session;
use chord_core::identify::{
    CAPABILITIES_ACTIVE, CAPABILITIES_CHANNEL_OBFUSCATION, IdentifyError, IdentifyParams,
    IdentifyProperties, PresenceInfo, build_identify, qos_token_client_provided,
};
use chord_core::ready::ReadySummary;
use chord_core::ws::{GatewayPipe, RunOptions, RunOutcome, connect_and_run, connect_and_run_with};
use futures_util::{SinkExt, StreamExt};
use std::fs;
use std::future::Future;
use std::io::{ErrorKind, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::protocol::Message;
use tokio_tungstenite::{WebSocketStream, accept_async};

/// Тайм-аут любого сетевого кейса (как в RW-04): цикл обязан уложиться.
const CASE_TIMEOUT: Duration = Duration::from_secs(5);

/// Интервал heartbeat стаба: 60 мс — несколько ударов укладываются в 5 с.
const STUB_INTERVAL_MS: i64 = 60;

/// Длина строки филлера (как в RW-04): больше окна ruzstd стаба (1 КиБ), прокручивает удержание.
const FILLER_TEXT_LEN: usize = 1536;

// ---------------------------------------------------------------------------
// Оракулы и кодирование термов
// ---------------------------------------------------------------------------

/// Ключ/значение-атом map (как в реальном трафике сервера: `op`/`t`/`d` — атомы).
fn atom(name: &str) -> EtfValue {
    EtfValue::Atom(name.to_string())
}

/// Map из пар «ключ-атом → значение».
fn map(pairs: Vec<(&str, EtfValue)>) -> EtfValue {
    EtfValue::Map(
        pairs
            .into_iter()
            .map(|(key, value)| (atom(key), value))
            .collect(),
    )
}

/// Список (`LIST_EXT` с хвостовым `NIL_EXT` — форма массивов в захвате).
fn list(items: Vec<EtfValue>) -> EtfValue {
    EtfValue::List(items)
}

/// Форма провода: `0x83` + терм.
fn wire(value: &EtfValue) -> Vec<u8> {
    etf::encode(value)
}

/// Разбирает терм так, как это делает пайп: ведущий `0x83` + ровно один терм на весь буфер.
fn decode_wire(bytes: &[u8]) -> EtfValue {
    assert_eq!(
        bytes.first(),
        Some(&etf::VERSION_BYTE),
        "ведущий маркер 0x83"
    );
    let (value, consumed) = etf::decode(bytes).expect("терм разбирается кодеком");
    assert_eq!(consumed, bytes.len(), "терм занимает весь буфер");
    value
}

/// Имена ключей map (ключ-атом или ключ-строка) — для проверки набора полей.
fn key_names(value: &EtfValue) -> Vec<String> {
    value
        .as_map()
        .expect("значение — map")
        .iter()
        .map(|(key, _)| match key {
            EtfValue::Atom(name) | EtfValue::Str(name) => name.clone(),
            other => panic!("нестроковый ключ map: {other:?}"),
        })
        .collect()
}

/// Параметры IDENTIFY с дефолтами нашего клиента и минимальным QoS-токеном.
fn test_params() -> IdentifyParams {
    IdentifyParams {
        token: "TEST_TOKEN".to_string(),
        capabilities: CAPABILITIES_ACTIVE,
        properties: IdentifyProperties::for_rw_client(),
        presence: None,
        compress: false,
        client_state_guild_versions: Vec::new(),
        qos_token: qos_token_client_provided(false),
    }
}

/// Синтетический READY (op0, s=1): 2 гильдии, 1 приватный канал, 3 пользователя,
/// 1 связь, read_state {2, false, 7}, user_guild_settings {1, false, 3}, user из 2 ключей,
/// session_id (32 символа), resume_gateway_url, 1 сессия, 2 эксперимента, 1 merged_members.
fn synthetic_ready_value() -> EtfValue {
    let guild = |id: &str| map(vec![("id", EtfValue::Str(id.to_string()))]);
    map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("READY")),
        ("s", EtfValue::Int(1)),
        (
            "d",
            map(vec![
                (
                    "guilds",
                    list(vec![
                        guild("1014537851418980372"),
                        guild("853389359330148352"),
                    ]),
                ),
                (
                    "private_channels",
                    list(vec![map(vec![("id", EtfValue::Str("1".to_string()))])]),
                ),
                (
                    "users",
                    list(vec![
                        map(vec![("id", EtfValue::Str("100".to_string()))]),
                        map(vec![("id", EtfValue::Str("200".to_string()))]),
                        map(vec![("id", EtfValue::Str("300".to_string()))]),
                    ]),
                ),
                (
                    "relationships",
                    list(vec![map(vec![
                        ("id", EtfValue::Str("200".to_string())),
                        ("type", EtfValue::Int(1)),
                    ])]),
                ),
                (
                    "read_state",
                    map(vec![
                        (
                            "entries",
                            list(vec![
                                map(vec![("id", EtfValue::Str("1".to_string()))]),
                                map(vec![("id", EtfValue::Str("2".to_string()))]),
                            ]),
                        ),
                        ("partial", atom("false")),
                        ("version", EtfValue::Int(7)),
                    ]),
                ),
                (
                    "user_guild_settings",
                    map(vec![
                        (
                            "entries",
                            list(vec![map(vec![(
                                "guild_id",
                                EtfValue::Str("1014537851418980372".to_string()),
                            )])]),
                        ),
                        ("partial", atom("false")),
                        ("version", EtfValue::Int(3)),
                    ]),
                ),
                (
                    "user",
                    map(vec![
                        ("id", EtfValue::Int(384_299_970_632_876_032)),
                        ("username", EtfValue::Str("tester".to_string())),
                    ]),
                ),
                (
                    "session_id",
                    EtfValue::Str("f14ac1f3d408c15027d6d98da172c398".to_string()),
                ),
                (
                    "resume_gateway_url",
                    EtfValue::Str("wss://gateway-us-east1-d.discord.gg".to_string()),
                ),
                (
                    "sessions",
                    list(vec![map(vec![("max_concurrent", EtfValue::Int(1))])]),
                ),
                (
                    "experiments",
                    list(vec![
                        list(vec![EtfValue::Int(1)]),
                        list(vec![EtfValue::Int(2)]),
                    ]),
                ),
                (
                    "merged_members",
                    list(vec![list(vec![EtfValue::Str("m".to_string())])]),
                ),
            ]),
        ),
    ])
}

/// READY-терм в форме провода (стаб).
fn synthetic_ready_bytes() -> Vec<u8> {
    wire(&synthetic_ready_value())
}

// ---------------------------------------------------------------------------
// Юнит-кейсы IDENTIFY (без сокетов)
// ---------------------------------------------------------------------------

#[test]
fn identify_has_evidence_fields() {
    // Кейс 1: дефолтная сборка — ровно 7 ключей уровня `_doIdentify`, presence = Nil,
    // client_state = {guild_versions: {}}, qos_token = 0A 02 08 00.
    let params = test_params();
    let term = build_identify(&params).expect("IDENTIFY собирается с дефолтами");
    let value = decode_wire(&term);

    let mut keys = key_names(&value);
    keys.sort();
    assert_eq!(
        keys,
        [
            "capabilities",
            "client_state",
            "compress",
            "presence",
            "properties",
            "qos_token",
            "token"
        ],
        "набор ключей IDENTIFY — как в `_doIdentify`"
    );

    assert_eq!(
        value.get("token").and_then(EtfValue::as_str),
        Some("TEST_TOKEN"),
        "токен без изменений"
    );
    assert_eq!(
        value.get("capabilities").and_then(EtfValue::as_int),
        Some(1_734_653),
        "capabilities активного клиента"
    );
    assert_eq!(
        value.get("compress"),
        Some(&EtfValue::Atom("false".to_string())),
        "zstd-поток не legacy → compress = false"
    );
    assert_eq!(
        value.get("presence"),
        Some(&EtfValue::Nil),
        "presence не задан → null/Nil"
    );
    assert_eq!(
        value
            .get("client_state")
            .and_then(|state| state.get("guild_versions")),
        Some(&EtfValue::Map(Vec::new())),
        "client_state = {{guild_versions: {{}}}}"
    );
    assert_eq!(
        value
            .get("qos_token")
            .and_then(EtfValue::as_str)
            .map(str::as_bytes),
        Some(&[0x0A, 0x02, 0x08, 0x00][..]),
        "минимальный QoS-токен client_provided{{is_active:false}}"
    );
}

#[test]
fn identify_capabilities_can_enable_obfuscation() {
    // Кейс 2: фича private_channel_obfuscation → capabilities 1 767 421 в терме.
    let mut params = test_params();
    params.capabilities = CAPABILITIES_CHANNEL_OBFUSCATION;
    let value = decode_wire(&build_identify(&params).expect("IDENTIFY с обфускацией"));
    assert_eq!(
        value.get("capabilities").and_then(EtfValue::as_int),
        Some(1_767_421),
        "capabilities с private_channel_obfuscation"
    );
}

#[test]
fn identify_properties_evidence_defaults() {
    // Кейс 3: свойства дефолтного клиента — ключи из `handleIdentify`/супер-свойств бандла.
    let mut params = test_params();
    let properties = decode_wire(&build_identify(&params).expect("IDENTIFY"))
        .get("properties")
        .cloned()
        .expect("properties присутствуют");

    for key in [
        "os",
        "browser",
        "release_channel",
        "client_version",
        "os_version",
        "os_arch",
        "app_arch",
        "system_locale",
        "has_client_mods",
        "client_launch_id",
        "browser_user_agent",
        "browser_version",
        "client_build_number",
        "client_app_state",
        "is_fast_connect",
        "gateway_connect_reasons",
    ] {
        assert!(
            properties.get(key).is_some(),
            "в properties нет ключа {key}: {properties:?}"
        );
    }
    assert_eq!(
        properties.get("browser").and_then(EtfValue::as_str),
        Some("Discord Client"),
        "browser веб-клиента"
    );
    assert_eq!(
        properties.get("has_client_mods"),
        Some(&EtfValue::Atom("false".to_string())),
        "модификаций нет"
    );
    assert!(
        properties
            .get("client_build_number")
            .and_then(EtfValue::as_int)
            .is_some(),
        "client_build_number — целое"
    );
    assert_eq!(
        properties
            .get("client_app_state")
            .and_then(EtfValue::as_str),
        Some("focused"),
        "client_app_state по умолчанию — focused"
    );
    assert_eq!(
        properties.get("is_fast_connect"),
        Some(&EtfValue::Atom("false".to_string())),
        "обычный логин — не fast-connect"
    );
    // NB: в ТЗ стоит «массив», но оракул (код клиента) говорит иначе: модуль 469177 `L7()`
    // возвращает **строку** (`[...reasons].sort().join(",")`, пусто → "NO_REASONS"); первый
    // коннект: r=true → "COLD_START". Тест фиксирует код клиента (см. отчёт RW-05).
    assert_eq!(
        properties
            .get("gateway_connect_reasons")
            .and_then(EtfValue::as_str),
        Some("COLD_START"),
        "gateway_connect_reasons — строка из L7(), дефолт первого коннекта"
    );
    assert!(
        properties.get("installation_id").is_none(),
        "installation_id не задан → ключа нет"
    );

    // Заданный installation_id попадает в свойства.
    params.properties.installation_id = Some("123456789012345678".to_string());
    let properties = decode_wire(&build_identify(&params).expect("IDENTIFY с installation_id"))
        .get("properties")
        .cloned()
        .expect("properties присутствуют");
    assert_eq!(
        properties.get("installation_id").and_then(EtfValue::as_str),
        Some("123456789012345678"),
        "installation_id появляется, когда задан"
    );
}

#[test]
fn identify_presence_option_encodes_map() {
    // Кейс 4: Some(presence) → map {status, since, activities, afk}; None → Nil.
    let mut params = test_params();
    params.presence = Some(PresenceInfo {
        status: "online".to_string(),
        since: 0,
        activities: Vec::new(),
        afk: false,
    });
    let value = decode_wire(&build_identify(&params).expect("IDENTIFY с presence"));
    let presence = value.get("presence").expect("presence в терме");

    let mut keys = key_names(presence);
    keys.sort();
    assert_eq!(
        keys,
        ["activities", "afk", "since", "status"],
        "presence — map из getLocalPresence()"
    );
    assert_eq!(
        presence.get("status").and_then(EtfValue::as_str),
        Some("online")
    );
    assert_eq!(presence.get("since").and_then(EtfValue::as_int), Some(0));
    assert_eq!(
        presence.get("activities"),
        Some(&EtfValue::List(Vec::new())),
        "пустые активности — пустой список"
    );
    assert_eq!(
        presence.get("afk"),
        Some(&EtfValue::Atom("false".to_string())),
        "afk = false"
    );

    params.presence = None;
    let value = decode_wire(&build_identify(&params).expect("IDENTIFY без presence"));
    assert_eq!(value.get("presence"), Some(&EtfValue::Nil), "None → Nil");
}

#[test]
fn identify_client_state_guild_versions() {
    // Кейс 5: версии гильдий → client_state.guild_versions — map с этой парой.
    let mut params = test_params();
    params.client_state_guild_versions = vec![(
        "1014537851418980372".to_string(),
        "17393980185986048".to_string(),
    )];
    let value = decode_wire(&build_identify(&params).expect("IDENTIFY с версиями гильдий"));
    let guild_versions = value
        .get("client_state")
        .and_then(|state| state.get("guild_versions"))
        .expect("client_state.guild_versions");
    let pairs = guild_versions.as_map().expect("guild_versions — map");
    assert_eq!(pairs.len(), 1, "одна пара версий");
    // Ключи/значения — строки: веб-клиент кодирует ключи объектов как binary (wetf
    // `encoding:{key:"binary"}`, ~4961704), наши термы повторяют это.
    assert_eq!(pairs[0].0.as_str(), Some("1014537851418980372"));
    assert_eq!(pairs[0].1.as_str(), Some("17393980185986048"));
}

#[test]
fn identify_qos_token_minimal_and_custom() {
    // Кейс 6: кастомные QoS-байты проходят как строка; не-UTF8 — ошибка без паники.
    assert_eq!(
        qos_token_client_provided(false),
        vec![0x0A, 0x02, 0x08, 0x00],
        "client_provided{{is_active:false}}"
    );
    assert_eq!(
        qos_token_client_provided(true),
        vec![0x0A, 0x02, 0x08, 0x01],
        "client_provided{{is_active:true}}"
    );

    let mut params = test_params();
    params.qos_token = vec![0x0A, 0x02, 0x08, 0x01];
    let value = decode_wire(&build_identify(&params).expect("IDENTIFY с кастомным QoS"));
    assert_eq!(
        value
            .get("qos_token")
            .and_then(EtfValue::as_str)
            .map(str::as_bytes),
        Some(&[0x0A, 0x02, 0x08, 0x01][..]),
        "байты QoS — как строка терма"
    );

    params.qos_token = vec![0xFF, 0xFE];
    match build_identify(&params) {
        Err(IdentifyError::QosTokenNotUtf8) => {}
        other => panic!("ожидался QosTokenNotUtf8, получено {other:?}"),
    }
}

#[test]
fn identify_debug_redacts_token() {
    // Кейс 7: Debug не должен раскрывать токен.
    let params = test_params();
    let debug = format!("{params:?}");
    assert!(
        !debug.contains("TEST_TOKEN"),
        "Debug не должен содержать токен: {debug}"
    );
    assert!(debug.contains("REDACTED"), "Debug помечает токен: {debug}");
}

#[test]
fn identify_size_is_plausible_against_943() {
    // Кейс 8: реальный IDENTIFY в захвате — 943 Б (кадр маскирован len+sha256; побайловое
    // сравнение невозможно — токен и свойства живого клиента отличаются). Наш терм с токеном
    // 59 символов (длина реального токена пользователя) и явным presence — того же порядка.
    let mut params = test_params();
    params.token = "A".repeat(59);
    params.presence = Some(PresenceInfo {
        status: "online".to_string(),
        since: 0,
        activities: Vec::new(),
        afk: false,
    });
    let size = build_identify(&params).expect("IDENTIFY собирается").len();
    assert!(
        (500..=1200).contains(&size),
        "размер IDENTIFY {size} Б вне ориентира 500..=1200 (реальный — 943 Б)"
    );
}

// ---------------------------------------------------------------------------
// Юнит-кейсы READY (без сокетов)
// ---------------------------------------------------------------------------

#[test]
fn ready_summary_from_synthetic_dispatch() {
    // Кейс 9: синтетический READY → сводка со всеми полями.
    let value = decode_wire(&synthetic_ready_bytes());
    let summary = ReadySummary::from_dispatch(&value).expect("READY распознан");
    let expected = ReadySummary {
        top_keys: 12,
        guilds: 2,
        private_channels: 1,
        users: 3,
        relationships: 1,
        read_state_entries: 2,
        read_state_partial: Some(false),
        read_state_version: Some(7),
        user_guild_settings_entries: 1,
        user_keys: Some(2),
        user_id: Some("384299970632876032".to_string()),
        session_id: Some("f14ac1f3d408c15027d6d98da172c398".to_string()),
        resume_gateway_url: Some("wss://gateway-us-east1-d.discord.gg".to_string()),
        sessions: 1,
        experiments: 2,
        merged_members: 1,
    };
    assert_eq!(summary, expected, "все поля сводки READY");
}

#[test]
fn ready_summary_none_for_non_ready() {
    // Кейс 10: не-READY формы → None (op10 HELLO; op0 с другим t; op0 с d без `guilds`).
    let hello = wire(&map(vec![
        ("op", EtfValue::Int(10)),
        (
            "d",
            map(vec![("heartbeat_interval", EtfValue::Int(41_250))]),
        ),
    ]));
    assert_eq!(
        ReadySummary::from_dispatch(&decode_wire(&hello)),
        None,
        "op10 HELLO — не READY"
    );

    let other = wire(&map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("OTHER")),
        ("s", EtfValue::Int(1)),
        ("d", map(vec![("guilds", list(Vec::new()))])),
    ]));
    assert_eq!(
        ReadySummary::from_dispatch(&decode_wire(&other)),
        None,
        "op0 с t=OTHER — не READY"
    );

    let without_guilds = wire(&map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("READY")),
        ("s", EtfValue::Int(1)),
        ("d", map(vec![("users", list(Vec::new()))])),
    ]));
    assert_eq!(
        ReadySummary::from_dispatch(&decode_wire(&without_guilds)),
        None,
        "READY без d.guilds — не сводка"
    );
}

#[test]
fn ready_accepts_string_user_id() {
    // Кейс 10a (нит мутационного ревью M6): `user.id` может прийти строкой (JS-безопасность
    // больших чисел, ~2^53) — сводка обязана сохранить строку, а не потерять id.
    let value = map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("READY")),
        ("s", EtfValue::Int(1)),
        (
            "d",
            map(vec![
                ("guilds", list(Vec::new())),
                (
                    "user",
                    map(vec![("id", EtfValue::Str("9007199254740993".to_string()))]),
                ),
            ]),
        ),
    ]);
    let summary = ReadySummary::from_dispatch(&decode_wire(&wire(&value))).expect("READY");
    assert_eq!(
        summary.user_id.as_deref(),
        Some("9007199254740993"),
        "user.id строкой сохраняется"
    );
}

#[test]
fn ready_accepts_int_partial() {
    // Кейс 10b (нит мутационного ревью M7): `read_state.partial` может прийти числом 1/0 —
    // сводка обязана прочитать его наравне с атомами.
    let value = map(vec![
        ("op", EtfValue::Int(0)),
        ("t", atom("READY")),
        ("s", EtfValue::Int(1)),
        (
            "d",
            map(vec![
                ("guilds", list(Vec::new())),
                (
                    "read_state",
                    map(vec![
                        ("entries", list(Vec::new())),
                        ("partial", EtfValue::Int(1)),
                        ("version", EtfValue::Int(3)),
                    ]),
                ),
            ]),
        ),
    ]);
    let summary = ReadySummary::from_dispatch(&decode_wire(&wire(&value))).expect("READY");
    assert_eq!(
        summary.read_state_partial,
        Some(true),
        "partial=1 читается как true"
    );
}

// ---------------------------------------------------------------------------
// Lab-кейс: реальный снимок потока (skip-aware)
// ---------------------------------------------------------------------------

/// Lab-корень: `RE_LAB_ROOT` или `/home/sanya/re-deep` (как в тестах RW-02/RW-04).
fn lab_root() -> PathBuf {
    PathBuf::from(
        std::env::var("RE_LAB_ROOT").unwrap_or_else(|_| "/home/sanya/re-deep".to_string()),
    )
}

/// Текст lab-файла; нет файла → `None` (skip-aware), прочие ошибки чтения не глотаются.
fn read_opt(path: &Path) -> Option<String> {
    match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == ErrorKind::NotFound => None,
        Err(error) => panic!("lab-файл {}: {error}", path.display()),
    }
}

/// Строка hex → байты (пробелы игнорируются).
fn unhex(s: &str) -> Vec<u8> {
    let compact: String = s.chars().filter(|c| !c.is_whitespace()).collect();
    assert_eq!(compact.len() % 2, 0, "hex нечётной длины: {s}");
    (0..compact.len() / 2)
        .map(|i| u8::from_str_radix(&compact[2 * i..2 * i + 2], 16).expect("hex"))
        .collect()
}

#[test]
fn lab_ready_from_run2_stream() {
    // Кейс 11: все 137 кадров снимка через пайп — READY (s=1) и его числа.
    let path = lab_root().join("gw-proto/zstd-frames-95974_1389.hexl");
    let Some(text) = read_opt(&path) else {
        eprintln!("skip: нет lab-файла {}", path.display());
        return;
    };
    let frames: Vec<Vec<u8>> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(unhex)
        .collect();
    assert_eq!(frames.len(), 137, "кадров в снимке потока");

    let mut pipe = GatewayPipe::new(Session::new(0));
    for (index, frame) in frames.iter().enumerate() {
        pipe.push_binary(frame, index as u64)
            .unwrap_or_else(|error| panic!("кадр {index}: {error}"));
    }

    let ready = pipe.last_ready().expect("READY — второй терм потока");
    assert_eq!(ready.top_keys, 40, "ключей верхнего уровня в d");
    assert_eq!(ready.guilds, 7, "гильдий");
    assert_eq!(ready.private_channels, 17, "приватных каналов");
    assert_eq!(ready.users, 146, "пользователей");
    assert_eq!(ready.relationships, 60, "связей");
    assert_eq!(ready.read_state_entries, 157, "записей read_state");
    assert_eq!(ready.read_state_partial, Some(false), "read_state.partial");
    assert_eq!(
        ready.read_state_version,
        Some(128_949),
        "read_state.version"
    );
    assert_eq!(
        ready.user_guild_settings_entries, 58,
        "записей user_guild_settings"
    );
    assert_eq!(ready.user_keys, Some(32), "user — 32 ключа");
    assert_eq!(ready.user_id.as_deref(), Some("384299970632876032"));
    let session_id = ready
        .session_id
        .as_deref()
        .expect("session_id присутствует");
    assert_eq!(session_id.len(), 32, "session_id — 32 символа");
    assert_eq!(session_id, "f14ac1f3d408c15027d6d98da172c398");
    assert_eq!(
        ready.resume_gateway_url.as_deref(),
        Some("wss://gateway-us-east1-d.discord.gg"),
        "resume_gateway_url из READY"
    );
    assert_eq!(ready.sessions, 1, "сессий");
    assert_eq!(ready.experiments, 6, "экспериментов");
    assert_eq!(ready.merged_members, 7, "merged_members");
}

// ---------------------------------------------------------------------------
// Стаб-сервер на 127.0.0.1 (WS-handshake + сценарий обработчика)
// ---------------------------------------------------------------------------

/// WS-сокет стаба (plain TCP: TLS в тестах не поднимается — только локальный `ws://`).
type StubSocket = WebSocketStream<tokio::net::TcpStream>;

/// Запускает стаб-сервер на `127.0.0.1:0` и возвращает его `ws://`-URL и задачу-сценарий.
async fn spawn_stub<T, F, Fut>(handler: F) -> (String, tokio::task::JoinHandle<Result<T, String>>)
where
    F: FnOnce(StubSocket) -> Fut + Send + 'static,
    Fut: Future<Output = Result<T, String>> + Send,
    T: Send + 'static,
{
    let listener = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("стаб: bind 127.0.0.1:0");
    let addr = listener.local_addr().expect("стаб: local_addr");
    let task = tokio::spawn(async move {
        let (tcp, _) = listener
            .accept()
            .await
            .map_err(|error| format!("accept: {error}"))?;
        let ws = accept_async(tcp)
            .await
            .map_err(|error| format!("WS-handshake: {error}"))?;
        handler(ws).await
    });
    (format!("ws://{addr}"), task)
}

/// Ждёт завершения стаба (не дольше 5 с) и разворачивает результат сценария.
async fn stub_result<T>(task: tokio::task::JoinHandle<Result<T, String>>) -> T {
    timeout(CASE_TIMEOUT, task)
        .await
        .expect("стаб-задача не завершилась за 5 с")
        .expect("стаб-задача упала")
        .expect("сценарий стаба вернул ошибку")
}

/// Ошибка слоя WS/TCP → текст для сценария стаба.
fn io_err(error: impl std::fmt::Display) -> String {
    error.to_string()
}

/// Отправляет фрагмент WS-сообщением Binary.
async fn send_binary(ws: &mut StubSocket, bytes: Vec<u8>) -> Result<(), String> {
    ws.send(Message::binary(bytes)).await.map_err(io_err)
}

/// Формат `zt`-стаба: **непрерывный** zstd-поток без `finish()` (как у шлюза).
struct StubStream {
    /// C-энкодер zstd поверх `Vec<u8>`; кадр никогда не завершается.
    encoder: zstd::stream::write::Encoder<'static, Vec<u8>>,
    /// Сколько сжатых байт уже отдано потребителю.
    sent: usize,
}

impl StubStream {
    /// Энкодер с окном ≈1 КиБ (`WindowLog(10)`): удержание окна видно и на маленьких порциях.
    fn new() -> Self {
        let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), 1).expect("zstd-энкодер");
        encoder
            .set_parameter(zstd::zstd_safe::CParameter::WindowLog(10))
            .expect("окно zstd 1 КиБ");
        Self { encoder, sent: 0 }
    }

    /// Терм + филлер одним фрагментом (обычный шаг стаба).
    fn write_frame(&mut self, term: &[u8]) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(term.len() + FILLER_TEXT_LEN + 6);
        bytes.extend_from_slice(term);
        bytes.extend_from_slice(&{ wire(&EtfValue::Str("f".repeat(FILLER_TEXT_LEN))) });
        self.write_raw(&bytes)
    }

    /// Дописывает произвольные байты провода и завершает блок (`flush`), отдаёт новые байты.
    fn write_raw(&mut self, bytes: &[u8]) -> Vec<u8> {
        self.encoder.write_all(bytes).expect("запись в zstd");
        self.encoder.flush().expect("flush zstd-блока");
        self.take_fresh()
    }

    /// Новые сжатые байты (без завершения кадра).
    fn take_fresh(&mut self) -> Vec<u8> {
        let buffer = self.encoder.get_mut();
        let fresh = buffer[self.sent..].to_vec();
        self.sent = buffer.len();
        fresh
    }
}

/// HELLO с интервалом `interval` (форма провода).
fn hello_bytes(interval: i64) -> Vec<u8> {
    wire(&map(vec![
        ("op", EtfValue::Int(10)),
        (
            "d",
            map(vec![("heartbeat_interval", EtfValue::Int(interval))]),
        ),
    ]))
}

/// ACK (op11) — форма провода.
fn ack_bytes() -> Vec<u8> {
    wire(&map(vec![("op", EtfValue::Int(11))]))
}

/// Разбор кадра клиента: ровно один терм на весь буфер.
fn decode_client_frame(data: &[u8]) -> Result<EtfValue, String> {
    let (value, consumed) = etf::decode(data).map_err(|error| format!("кадр клиента: {error}"))?;
    if consumed != data.len() {
        return Err(format!(
            "кадр клиента {} Б: разобран терм на {consumed} Б (хвост)",
            data.len()
        ));
    }
    Ok(value)
}

// ---------------------------------------------------------------------------
// Стаб-кейсы: реальные сокеты 127.0.0.1
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ws_identify_sent_once_after_hello() {
    // Кейс 12: HELLO → клиент с RunOptions{identify} шлёт op2 ровно один раз; стаб проверяет
    // поля по структуре веб-клиента, отвечает READY (s=1) и ACK на удары; после двух ударов
    // закрывает соединение → Closed; в пайпе — сводка READY, RTT измерен.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
        let mut identifies: Vec<Vec<u8>> = Vec::new();
        let mut beats = 0u64;
        while let Some(message) = ws.next().await {
            match message.map_err(io_err)? {
                Message::Binary(data) => {
                    let value = decode_client_frame(&data)?;
                    match value.get("op").and_then(EtfValue::as_int) {
                        Some(2) => {
                            identifies.push(data.to_vec());
                            send_binary(&mut ws, stream.write_frame(&synthetic_ready_bytes()))
                                .await?;
                        }
                        Some(1) => {
                            beats += 1;
                            send_binary(&mut ws, stream.write_frame(&ack_bytes())).await?;
                            if beats >= 2 {
                                ws.close(None).await.map_err(io_err)?;
                                break;
                            }
                        }
                        other => {
                            return Err(format!(
                                "от клиента ждали op1/op2, пришло {other:?}; терм: {value:?}"
                            ));
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        Ok((identifies, beats))
    })
    .await;

    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(
        CASE_TIMEOUT,
        connect_and_run_with(
            &url,
            &mut pipe,
            RunOptions {
                identify: Some(test_params()),
                on_dispatch: None,
            },
        ),
    )
    .await
    .expect("цикл транспорта не уложился в 5 с");
    let (identifies, beats) = stub_result(stub).await;

    assert_eq!(
        outcome,
        Ok(RunOutcome::Closed { code: None }),
        "стаб закрыл соединение без кода"
    );
    assert_eq!(identifies.len(), 1, "IDENTIFY отправлен ровно один раз");
    let identify = decode_wire(&identifies[0]);
    assert_eq!(
        identify.get("op").and_then(EtfValue::as_int),
        Some(2),
        "op = IDENTIFY"
    );
    // Оракул формы — код клиента: `send = (e,t,n) => pack({op:e, d:t})`; payload — под `d`.
    let payload = identify
        .get("d")
        .expect("payload IDENTIFY лежит под ключом d");
    assert_eq!(
        payload.get("token").and_then(EtfValue::as_str),
        Some("TEST_TOKEN"),
        "token placeholder стаба"
    );
    assert_eq!(
        payload.get("capabilities").and_then(EtfValue::as_int),
        Some(1_734_653),
        "capabilities активного клиента"
    );
    let properties = payload.get("properties").expect("properties в IDENTIFY");
    assert_eq!(
        properties.get("is_fast_connect"),
        Some(&EtfValue::Atom("false".to_string())),
        "is_fast_connect = false"
    );
    assert_eq!(
        payload
            .get("client_state")
            .and_then(|state| state.get("guild_versions")),
        Some(&EtfValue::Map(Vec::new())),
        "client_state.guild_versions = {{}}"
    );

    let ready = pipe.last_ready().expect("сводка READY из стаба");
    assert_eq!(ready.guilds, 2, "гильдий в синтетическом READY");
    assert_eq!(ready.users, 3, "пользователей");
    assert_eq!(ready.private_channels, 1, "приватный канал");
    assert_eq!(ready.read_state_entries, 2, "записей read_state");
    assert_eq!(ready.experiments, 2, "экспериментов");
    assert_eq!(ready.user_id.as_deref(), Some("384299970632876032"));
    assert_eq!(
        ready.session_id.as_deref().map(str::len),
        Some(32),
        "session_id — 32 символа"
    );
    assert_eq!(
        ready.resume_gateway_url.as_deref(),
        Some("wss://gateway-us-east1-d.discord.gg")
    );
    assert_eq!(
        pipe.session().last_seq(),
        Some(1),
        "READY обновил s-последовательность"
    );
    assert!(
        pipe.session().rtt_ms().is_some(),
        "RTT измерен по ACK: {:?}",
        pipe.session().rtt_ms()
    );
    assert!(beats >= 2, "стаб принял {beats} heartbeat (ожидалось ≥ 2)");
}

#[tokio::test]
async fn ws_no_identify_without_option() {
    // Кейс 13: без RunOptions.identify клиент не шлёт ни одного op2, но протокол не сломан:
    // удары идут, ACK доходят (не зомби), стаб закрывает — Closed.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
        let mut identify_frames = 0u64;
        let mut beats = 0u64;
        while let Some(message) = ws.next().await {
            match message.map_err(io_err)? {
                Message::Binary(data) => {
                    let value = decode_client_frame(&data)?;
                    match value.get("op").and_then(EtfValue::as_int) {
                        Some(2) => identify_frames += 1,
                        Some(1) => {
                            beats += 1;
                            send_binary(&mut ws, stream.write_frame(&ack_bytes())).await?;
                            if beats >= 2 {
                                ws.close(None).await.map_err(io_err)?;
                                break;
                            }
                        }
                        other => {
                            return Err(format!(
                                "от клиента ждали op1/op2, пришло {other:?}; терм: {value:?}"
                            ));
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        Ok((identify_frames, beats))
    })
    .await;

    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(CASE_TIMEOUT, connect_and_run(&url, &mut pipe))
        .await
        .expect("цикл транспорта не уложился в 5 с");
    let (identify_frames, beats) = stub_result(stub).await;

    assert_eq!(
        outcome,
        Ok(RunOutcome::Closed { code: None }),
        "стаб закрыл соединение без кода"
    );
    assert_eq!(
        identify_frames, 0,
        "без RunOptions.identify op2 не отправляется"
    );
    assert!(beats >= 2, "протокол не сломан: удары идут ({beats})");
    assert!(
        pipe.session().rtt_ms().is_some(),
        "ACK доходят — не зомби: {:?}",
        pipe.session().rtt_ms()
    );
    assert!(
        pipe.last_ready().is_none(),
        "READY стаб не присылал — сводки нет"
    );
}

#[tokio::test]
async fn ws_identify_only_after_hello() {
    // Кейс 13a (нит мутационного ревью M10): до HELLO клиент молчит — IDENTIFY отправляется
    // только после оп10. Стаб 300 мс не шлёт ничего и проверяет тишину, затем HELLO → op2.
    let (url, stub) = spawn_stub(|mut ws| async move {
        let quiet = timeout(Duration::from_millis(300), ws.next())
            .await
            .is_err();
        let mut stream = StubStream::new();
        send_binary(&mut ws, stream.write_frame(&hello_bytes(STUB_INTERVAL_MS))).await?;
        let mut identifies = 0u64;
        while let Some(message) = ws.next().await {
            match message.map_err(io_err)? {
                Message::Binary(data) => {
                    let value = decode_client_frame(&data)?;
                    match value.get("op").and_then(EtfValue::as_int) {
                        Some(2) => {
                            identifies += 1;
                            ws.close(None).await.map_err(io_err)?;
                            break;
                        }
                        // Удар мог обогнать IDENTIFY (первый срок при seed=0 — сразу после HELLO).
                        Some(1) => {}
                        other => {
                            return Err(format!(
                                "от клиента ждали op1/op2, пришло {other:?}; терм: {value:?}"
                            ));
                        }
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        Ok((quiet, identifies))
    })
    .await;

    let mut pipe = GatewayPipe::new(Session::new(0));
    let outcome = timeout(
        CASE_TIMEOUT,
        connect_and_run_with(
            &url,
            &mut pipe,
            RunOptions {
                identify: Some(test_params()),
                on_dispatch: None,
            },
        ),
    )
    .await
    .expect("цикл транспорта не уложился в 5 с");
    let (quiet, identifies) = stub_result(stub).await;

    assert_eq!(
        outcome,
        Ok(RunOutcome::Closed { code: None }),
        "стаб закрыл соединение без кода"
    );
    assert!(quiet, "до HELLO клиент не отправил ни одного кадра");
    assert_eq!(identifies, 1, "после HELLO IDENTIFY отправлен один раз");
}
