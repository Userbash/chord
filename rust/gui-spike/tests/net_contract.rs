//! Контракт сетевого слоя: статус шлюза (NET-*) и первый backend-срез L-02
//! «dispatch → UI» (`CONTRACTS/dispatch.md`).
//!
//! Оракулы диспатчей — снятые кадры `fixtures/gateway/by-type/*.json` и роутер
//! ядра (`discord_core::route_dispatch`), а не текущая реализация адаптера:
//! `d` сравнивается с кадром поле-в-поле, секреты READY проверяются рекурсивно.
//! Сети/окна/живого Discord здесь нет: драйвер — `FakeGateway`, эмиттер — fake
//! с `Condvar`-ожиданием (порядок событий проверяется без sleep-гонок).

use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

use discord_core::codec::etf::EtfValue;
use discord_core::dispatch::{route_dispatch, DispatchEvent};
use discord_core::gateway::supervisor::{StopReason, SupervisorEvent};
use discord_gui_spike::net::{
    etf_to_json, map_dispatch, map_supervisor_event, CoreGatewayDriver, DispatchChannel,
    DispatchEmitter, DispatchError, DispatchPayload, DispatchStats, FakeGateway, GatewayDriver,
    GatewayEvent, GatewayStatus, NetController, NetError, StaticTokenStore,
    CORE_DISPATCH_AVAILABLE, DISPATCH_EVENT, FORWARDED_DISPATCHES,
};
use serde_json::Value;

#[test]
fn connect_reads_token_and_reaches_ready() {
    let gateway = FakeGateway::new();
    let controller =
        NetController::new(StaticTokenStore::new(Some("secret-token")), gateway.clone());

    assert_eq!(controller.status(), GatewayStatus::Offline);
    controller.connect().expect("connect");
    assert_eq!(controller.status(), GatewayStatus::Ready);
    assert_eq!(gateway.identify_tokens(), vec!["secret-token"]);
}

#[test]
fn status_never_contains_token() {
    // D-43: ошибка драйвера проверяется на failing-драйвере; `FakeGateway::new()`
    // по контракту соседнего теста обязан подключаться успешно.
    let gateway = FakeGateway::failing();
    let controller = NetController::new(StaticTokenStore::new(Some("secret-token")), gateway);
    let error = controller
        .connect()
        .expect_err("missing gateway should fail");

    assert!(!format!("{error:?}").contains("secret-token"));
    assert!(!format!("{}", controller.status()).contains("secret-token"));
}

#[test]
fn close_policies_are_forwarded() {
    let gateway = FakeGateway::new();
    let store = StaticTokenStore::new(Some("token"));
    let controller = NetController::new(store.clone(), gateway.clone());
    controller.connect().expect("connect");

    gateway.close(4004);
    assert_eq!(controller.status(), GatewayStatus::Offline);
    assert_eq!(gateway.logout_count(), 1);
    // 4004 — недействительный токен: keyring очищен, повтор без входа невозможен.
    assert_eq!(controller.connect(), Err(NetError::MissingToken));

    store.set("token");
    controller.connect().expect("reconnect");
    gateway.close(4014);
    assert_eq!(controller.status(), GatewayStatus::Error);
    assert_eq!(gateway.stop_count(), 1);
}

#[test]
fn disconnect_is_idempotent_and_reconnect_is_visible() {
    let gateway = FakeGateway::new();
    let controller = NetController::new(StaticTokenStore::new(Some("token")), gateway.clone());
    controller.disconnect().expect("offline disconnect");
    controller.connect().expect("connect");
    gateway.reconnecting();
    assert_eq!(controller.status(), GatewayStatus::Reconnecting);
    controller.disconnect().expect("disconnect");
    controller.disconnect().expect("second disconnect");
    assert_eq!(controller.status(), GatewayStatus::Offline);
    assert_eq!(gateway.disconnect_count(), 3);
}

#[test]
fn missing_token_is_a_typed_error() {
    let controller = NetController::new(StaticTokenStore::new(None), FakeGateway::new());
    assert_eq!(controller.connect(), Err(NetError::MissingToken));
}

#[test]
fn supervisor_events_map_to_gateway_contract() {
    assert_eq!(
        map_supervisor_event(SupervisorEvent::Ready),
        GatewayEvent::Ready
    );
    assert_eq!(
        map_supervisor_event(SupervisorEvent::Reconnecting { delay_ms: 25 }),
        GatewayEvent::Reconnecting
    );
    assert_eq!(
        map_supervisor_event(SupervisorEvent::Stopped(StopReason::Unauthorized)),
        GatewayEvent::Closed(4004)
    );
    assert_eq!(
        map_supervisor_event(SupervisorEvent::Stopped(StopReason::ReconnectLimit {
            reconnects: 8
        })),
        GatewayEvent::Closed(4013)
    );
}

#[test]
fn production_driver_disconnect_is_idempotent_without_connecting() {
    let driver = CoreGatewayDriver::new();
    driver.disconnect();
    driver.disconnect();
}

// ---------------------------------------------------------------------------
// L-02: dispatch → UI (первый backend-срез).
//
// Провенанс: 6 имён в `FORWARDED_DISPATCHES` — из роутера ядра (21 наблюдённый
// тип, `dispatch.rs`); READY/CHUNK/PRESENCE/LIST_UPDATE имеют снятые кадры в
// `fixtures/gateway/by-type/`; MESSAGE_REACTION_ADD наблюдён один раз (union
// 7 снимков), MESSAGE_REACTION_REMOVE в захватах **отсутствует** (см.
// `CONTRACTS/dispatch.md` §5) — поэтому payload реакции проверяется на
// инвариант конверта (данные как пришли), без выдумывания полей события.
// ---------------------------------------------------------------------------

/// Снятый кадр `by-type` как JSON; нет каталога/файла — skip с причиной.
fn fixture(name: &str) -> Option<Value> {
    let root = match discord_gui_spike::data::fixtures_root() {
        Ok(root) => root,
        Err(error) => {
            eprintln!("skip: фикстуры недоступны: {error}");
            return None;
        }
    };
    let path = root.join("gateway/by-type").join(name);
    match std::fs::read_to_string(&path) {
        Ok(text) => Some(serde_json::from_str(&text).expect("кадр by-type — валидный JSON")),
        Err(error) => {
            eprintln!("skip: нет {}: {error}", path.display());
            None
        }
    }
}

/// JSON-представление снятого кадра → ETF того же вида.
///
/// Дамп `by-type` пишет атомы строками (`"nil"`, `"online"`), `Nil` — `null`;
/// обратное отображение нужно только чтобы подать кадр в роутер ядра.
fn json_to_etf(value: &Value) -> EtfValue {
    match value {
        Value::Null => EtfValue::Nil,
        Value::Bool(flag) => EtfValue::Atom(if *flag { "true" } else { "false" }.to_string()),
        Value::Number(number) => match number.as_i64() {
            Some(int) => EtfValue::Int(int),
            None => EtfValue::Float(number.as_f64().expect("число JSON")),
        },
        Value::String(text) => EtfValue::Str(text.clone()),
        Value::Array(items) => EtfValue::Tuple(items.iter().map(json_to_etf).collect()),
        Value::Object(map) => EtfValue::Map(
            map.iter()
                .map(|(key, value)| (EtfValue::Atom(key.clone()), json_to_etf(value)))
                .collect(),
        ),
    }
}

/// op0-кадр шлюза: `{op: 0, t: name, s: seq, d: data}`.
fn dispatch_frame(name: &str, seq: i64, data: EtfValue) -> EtfValue {
    EtfValue::Map(vec![
        (EtfValue::Atom("op".to_string()), EtfValue::Int(0)),
        (
            EtfValue::Atom("t".to_string()),
            EtfValue::Atom(name.to_string()),
        ),
        (EtfValue::Atom("s".to_string()), EtfValue::Int(seq)),
        (EtfValue::Atom("d".to_string()), data),
    ])
}

/// Кадр → событие роутером ядра (оракул, а не адаптер).
fn routed(name: &str, seq: i64, data: EtfValue) -> DispatchEvent {
    route_dispatch(&dispatch_frame(name, seq, data)).expect("op0 — диспатч")
}

fn presence(status: &str) -> EtfValue {
    EtfValue::Map(vec![
        (
            EtfValue::Atom("status".to_string()),
            EtfValue::Atom(status.to_string()),
        ),
        (
            EtfValue::Atom("user".to_string()),
            EtfValue::Map(vec![(
                EtfValue::Atom("id".to_string()),
                EtfValue::Str("42".to_string()),
            )]),
        ),
    ])
}

/// Fake UI-канал: пишет сконвертированный конверт и будит ожидающего.
#[derive(Default)]
struct RecordingEmitter {
    seen: Mutex<Vec<DispatchPayload>>,
    ready: Condvar,
    fail: bool,
}

impl RecordingEmitter {
    fn failing() -> Self {
        Self {
            fail: true,
            ..Self::default()
        }
    }

    fn wait_for(&self, count: usize) -> Vec<DispatchPayload> {
        let deadline = Instant::now() + Duration::from_secs(5);
        let mut seen = self
            .seen
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        while seen.len() < count {
            let now = Instant::now();
            if now >= deadline {
                panic!("не дождались {count} событий в UI-канале: {}", seen.len());
            }
            let (guard, timeout) = self
                .ready
                .wait_timeout(seen, deadline - now)
                .unwrap_or_else(|poison| poison.into_inner());
            seen = guard;
            if timeout.timed_out() && seen.len() < count {
                panic!("таймаут UI-канала: {}", seen.len());
            }
        }
        seen.clone()
    }
}

impl DispatchEmitter for RecordingEmitter {
    fn emit(&self, payload: &DispatchPayload) -> Result<(), String> {
        if self.fail {
            return Err("UI-канал недоступен".to_string());
        }
        self.seen
            .lock()
            .unwrap_or_else(|poison| poison.into_inner())
            .push(payload.clone());
        self.ready.notify_all();
        Ok(())
    }
}

/// Эмиттер, который «завис»: доказывает, что handler не блокирует источник.
struct BlockingEmitter {
    started: (Mutex<bool>, Condvar),
    release: (Mutex<bool>, Condvar),
}

impl BlockingEmitter {
    fn new() -> Self {
        Self {
            started: (Mutex::new(false), Condvar::new()),
            release: (Mutex::new(false), Condvar::new()),
        }
    }

    fn wait_started(&self) {
        let mut started = self
            .started
            .0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        while !*started {
            started = self
                .started
                .1
                .wait(started)
                .unwrap_or_else(|poison| poison.into_inner());
        }
    }

    fn unblock(&self) {
        *self
            .release
            .0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = true;
        self.release.1.notify_all();
    }
}

impl DispatchEmitter for BlockingEmitter {
    fn emit(&self, _payload: &DispatchPayload) -> Result<(), String> {
        *self
            .started
            .0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner()) = true;
        self.started.1.notify_all();
        let mut release = self
            .release
            .0
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        while !*release {
            release = self
                .release
                .1
                .wait(release)
                .unwrap_or_else(|poison| poison.into_inner());
        }
        Ok(())
    }
}

fn controller_with(
    emitter: Arc<dyn DispatchEmitter>,
    capacity: usize,
) -> (NetController, FakeGateway) {
    let gateway = FakeGateway::new();
    let controller = NetController::with_emitter(
        StaticTokenStore::new(Some("secret-token")),
        gateway.clone(),
        emitter,
        capacity,
    );
    controller.connect().expect("connect");
    (controller, gateway)
}

fn wait_for_stat(
    controller: &NetController,
    predicate: impl Fn(&DispatchStats) -> bool,
) -> DispatchStats {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let stats = controller.dispatch_stats();
        if predicate(&stats) {
            return stats;
        }
        assert!(
            Instant::now() < deadline,
            "статистика диспатчей не сошлась: {stats:?}"
        );
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// Имена секретных ключей, которые не должны покидать backend (READY их несёт).
///
/// Список — из контракта `CONTRACTS/dispatch.md` §5, не из реализации: там точный
/// ключ `token` стоит рядом с `access_token`/`refresh_token`, а не только `*_token`.
fn secret_paths(value: &Value, path: &str, found: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, nested) in map {
                let next = format!("{path}/{key}");
                if key == "session_id"
                    || key == "resume_gateway_url"
                    || key == "auth_session_id_hash"
                    || key == "client_secret"
                    || key == "password"
                    || key == "token"
                    || key == "access_token"
                    || key == "refresh_token"
                    || key.ends_with("_token")
                {
                    found.push(next.clone());
                }
                secret_paths(nested, &next, found);
            }
        }
        Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                secret_paths(item, &format!("{path}[{index}]"), found);
            }
        }
        Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => {}
    }
}

#[test]
fn presence_dispatch_is_forwarded_verbatim_in_kind_data_envelope() {
    let Some(frame) = fixture("010-PRESENCE_UPDATE.json") else {
        return;
    };
    let data = frame.get("d").expect("d").clone();
    let seq = frame.get("s").and_then(Value::as_i64).unwrap_or(1);
    let event = routed("PRESENCE_UPDATE", seq, json_to_etf(&data));

    let mapped = map_dispatch(&event)
        .expect("конвертация не падает")
        .expect("PRESENCE_UPDATE форвардится");

    assert_eq!(mapped.payload.kind, "PRESENCE_UPDATE");
    assert_eq!(mapped.payload.data, data, "d не переписывается");
    assert_eq!(mapped.redacted, 0, "в PRESENCE нет секретов");

    // Контракт UI: ровно `{kind, data}`, без служебных/придуманных полей.
    let json = serde_json::to_value(&mapped.payload).expect("payload → JSON");
    // Порядок ключей JSON не несёт смысла (`serde_json::Map` сортирует), поэтому
    // проверяется набор: ровно `kind` и `data`, без служебных полей.
    let mut keys: Vec<&str> = json
        .as_object()
        .expect("объект")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(keys, ["data", "kind"], "конверт: только kind и data");
    assert_eq!(json["kind"], "PRESENCE_UPDATE");
    assert_eq!(json["data"], data);
}

#[test]
fn members_chunk_is_forwarded_verbatim() {
    let Some(frame) = fixture("009-GUILD_MEMBERS_CHUNK.json") else {
        return;
    };
    let data = frame.get("d").expect("d").clone();
    let seq = frame.get("s").and_then(Value::as_i64).unwrap_or(1);
    let event = routed("GUILD_MEMBERS_CHUNK", seq, json_to_etf(&data));
    let mapped = map_dispatch(&event)
        .expect("конвертация не падает")
        .expect("GUILD_MEMBERS_CHUNK форвардится");
    assert_eq!(mapped.payload.kind, "GUILD_MEMBERS_CHUNK");
    assert_eq!(mapped.payload.data, data, "d не переписывается");
    assert_eq!(mapped.redacted, 0);
}

#[test]
fn members_list_update_keeps_counts_and_drops_nested_presence_session_ids() {
    // Реальный поток участников: `d` содержит presence вложенных member'ов, а в
    // нём — `session_id` активности. Поля не переписываются, но session-ключи
    // вырезаются и на этой глубине (не только в READY).
    let Some(frame) = fixture("005-GUILD_MEMBER_LIST_UPDATE.json") else {
        return;
    };
    let data = frame.get("d").expect("d").clone();
    let seq = frame.get("s").and_then(Value::as_i64).unwrap_or(1);

    let mut expected_secrets = Vec::new();
    secret_paths(&data, "", &mut expected_secrets);

    let event = routed("GUILD_MEMBER_LIST_UPDATE", seq, json_to_etf(&data));
    let mapped = map_dispatch(&event)
        .expect("конвертация не падает")
        .expect("GUILD_MEMBER_LIST_UPDATE форвардится");

    assert_eq!(mapped.payload.kind, "GUILD_MEMBER_LIST_UPDATE");
    assert_eq!(mapped.redacted, expected_secrets.len());
    let mut left = Vec::new();
    secret_paths(&mapped.payload.data, "", &mut left);
    assert!(left.is_empty(), "секреты во вложенных presence: {left:?}");

    for field in ["guild_id", "id", "member_count", "online_count", "groups"] {
        assert_eq!(
            mapped.payload.data[field], data[field],
            "поле {field} не переписывается"
        );
    }
    assert_eq!(
        mapped.payload.data["ops"].as_array().map(Vec::len),
        data["ops"].as_array().map(Vec::len),
        "состав ops сохраняется"
    );
}

#[test]
fn ready_payload_drops_session_secrets_and_keeps_data() {
    let Some(frame) = fixture("002-READY.json") else {
        return;
    };
    let data = frame.get("d").expect("d").clone();
    let seq = frame.get("s").and_then(Value::as_i64).unwrap_or(1);

    let mut before = Vec::new();
    secret_paths(&data, "", &mut before);
    assert!(
        !before.is_empty(),
        "оракул обязан содержать секретные ключи, иначе тест ничего не проверяет"
    );

    let event = routed("READY", seq, json_to_etf(&data));
    let mapped = map_dispatch(&event)
        .expect("READY конвертируется")
        .expect("READY форвардится");

    let mut after = Vec::new();
    secret_paths(&mapped.payload.data, "", &mut after);
    assert!(
        after.is_empty(),
        "секреты не должны уходить в UI: {after:?}"
    );
    assert_eq!(
        mapped.redacted,
        before.len(),
        "счётчик вырезанных ключей равен числу секретов оракула"
    );

    assert_eq!(
        mapped.payload.data["guilds"].as_array().map(Vec::len),
        data["guilds"].as_array().map(Vec::len),
        "гильдии READY не теряются"
    );
    assert_eq!(
        mapped.payload.data["user"]["id"], data["user"]["id"],
        "пользователь READY не теряется"
    );
    let text = serde_json::to_string(&mapped.payload).expect("JSON");
    assert!(
        !text.contains("secret-token"),
        "токен драйвера не попадает в payload"
    );
}

#[test]
fn exact_token_and_nested_auth_tokens_are_redacted_and_counted() {
    // CONTRACTS/dispatch.md §5: секрет — и точный ключ `token`, не только `*_token`.
    // Оракул выписан руками (путь + число), а не выведен из реализации: `user.id`
    // и `status` остаются, четыре ключа исчезают и попадают в `redacted`.
    let data = serde_json::json!({
        "user": {
            "id": "80351110224678912",
            "token": "nested-secret-token"
        },
        "token": "top-level-secret-token",
        "access_token": "access-secret",
        "refresh_token": "refresh-secret",
        "status": "online"
    });
    let event = routed("PRESENCE_UPDATE", 2, json_to_etf(&data));
    let mapped = map_dispatch(&event)
        .expect("конвертация не падает")
        .expect("PRESENCE_UPDATE форвардится");

    assert_eq!(
        mapped.redacted, 4,
        "token + access_token + refresh_token + вложенный token"
    );
    assert_eq!(mapped.payload.data["user"]["id"], data["user"]["id"]);
    assert_eq!(mapped.payload.data["status"], data["status"]);
    assert!(
        mapped.payload.data["user"].get("token").is_none(),
        "вложенный token не уходит в UI: {:?}",
        mapped.payload.data
    );
    for key in ["token", "access_token", "refresh_token"] {
        assert!(
            mapped.payload.data.get(key).is_none(),
            "ключ {key} вырезан: {:?}",
            mapped.payload.data
        );
    }
    let text = serde_json::to_string(&mapped.payload).expect("JSON");
    assert!(
        !text.contains("secret"),
        "значения токенов не попадают в payload: {text}"
    );
}

#[test]
fn filtered_and_unknown_kinds_do_not_panic_and_are_not_forwarded() {
    // Известный ядру, но не входящий в срез L-02 тип.
    let created = routed("MESSAGE_CREATE", 3, EtfValue::Map(Vec::new()));
    assert_eq!(map_dispatch(&created), Ok(None));

    // Неизвестный вообще: ядро отдаёт raw (kind: None), имя сохраняется.
    let unknown = routed("TOTALLY_UNKNOWN", 4, EtfValue::Map(Vec::new()));
    assert!(unknown.kind.is_none());
    assert_eq!(map_dispatch(&unknown), Ok(None));

    // Нестроковый ключ map — типизированная ошибка, не паника.
    let broken = routed(
        "PRESENCE_UPDATE",
        5,
        EtfValue::Map(vec![(EtfValue::Nil, EtfValue::Int(1))]),
    );
    assert_eq!(map_dispatch(&broken), Err(DispatchError::UnsupportedKey));
}

#[test]
fn reaction_kinds_use_wire_name_and_forward_data_as_is() {
    assert!(FORWARDED_DISPATCHES.contains(&"MESSAGE_REACTION_ADD"));
    assert!(FORWARDED_DISPATCHES.contains(&"MESSAGE_REACTION_REMOVE"));

    // Форма события реакции в наших захватах не снята (ADD — один раз, REMOVE —
    // ни разу): проверяется только инвариант конверта, поля не выдумываются.
    let empty = EtfValue::Map(Vec::new());
    for name in ["MESSAGE_REACTION_ADD", "MESSAGE_REACTION_REMOVE"] {
        let event = routed(name, 7, empty.clone());
        let mapped = map_dispatch(&event)
            .expect("конвертация не падает")
            .unwrap_or_else(|| panic!("{name} форвардится по имени провода"));
        assert_eq!(mapped.payload.kind, name);
        assert_eq!(mapped.payload.data, Value::Object(Default::default()));
    }
}

#[test]
fn etf_to_json_keeps_atoms_as_strings_and_handles_nil_and_numbers() {
    let value = EtfValue::Map(vec![
        (EtfValue::Atom("nil".to_string()), EtfValue::Nil),
        (
            EtfValue::Atom("atom".to_string()),
            EtfValue::Atom("online".to_string()),
        ),
        (EtfValue::Atom("int".to_string()), EtfValue::Int(-7)),
        (EtfValue::Atom("float".to_string()), EtfValue::Float(1.5)),
        (
            EtfValue::Atom("str".to_string()),
            EtfValue::Str("x".to_string()),
        ),
        (
            EtfValue::Atom("tuple".to_string()),
            EtfValue::Tuple(vec![EtfValue::Int(1), EtfValue::Nil]),
        ),
        (
            EtfValue::Atom("list".to_string()),
            EtfValue::List(Vec::new()),
        ),
        // JSON не умеет Infinity/NaN: null, а не паника.
        (
            EtfValue::Atom("inf".to_string()),
            EtfValue::Float(f64::INFINITY),
        ),
        // Ключ-целое: JSON-ключ — строка, значение не теряется.
        (EtfValue::Int(7), EtfValue::Atom("v".to_string())),
    ]);

    let json = etf_to_json(&value).expect("конверсия");
    assert_eq!(json["nil"], Value::Null);
    assert_eq!(
        json["atom"], "online",
        "атом остаётся строкой (как в дампе)"
    );
    assert_eq!(json["int"], -7);
    assert_eq!(json["float"], 1.5);
    assert_eq!(json["str"], "x");
    assert_eq!(json["tuple"], serde_json::json!([1, null]));
    assert_eq!(json["list"], serde_json::json!([]));
    assert_eq!(json["inf"], Value::Null);
    assert_eq!(json["7"], "v");
}

#[test]
fn dispatch_reaches_ui_channel_in_order_without_changing_status() {
    let emitter = Arc::new(RecordingEmitter::default());
    let (controller, gateway) = controller_with(emitter.clone(), 16);

    gateway.dispatch("PRESENCE_UPDATE", 10, presence("online"));
    gateway.dispatch("PRESENCE_UPDATE", 11, presence("idle"));
    gateway.dispatch("MESSAGE_REACTION_REMOVE", 12, EtfValue::Map(Vec::new()));

    let seen = emitter.wait_for(3);
    let kinds: Vec<&str> = seen.iter().map(|payload| payload.kind.as_str()).collect();
    assert_eq!(
        kinds,
        [
            "PRESENCE_UPDATE",
            "PRESENCE_UPDATE",
            "MESSAGE_REACTION_REMOVE"
        ],
        "порядок потока сохраняется"
    );
    assert_eq!(seen[0].data["status"], "online");
    assert_eq!(seen[1].data["status"], "idle");
    assert_eq!(
        controller.status(),
        GatewayStatus::Ready,
        "dispatch не меняет статус"
    );

    let stats = wait_for_stat(&controller, |stats| stats.delivered == 3);
    assert_eq!(stats.seen, 3);
    assert_eq!(stats.forwarded, 3);
    assert_eq!(stats.queued, 3);
    assert_eq!(stats.emit_failed, 0, "{stats:?}");
    assert_eq!(stats.last_error, None);
}

#[test]
fn emit_error_is_counted_and_not_swallowed() {
    let emitter = Arc::new(RecordingEmitter::failing());
    let (controller, gateway) = controller_with(emitter, 8);

    gateway.dispatch("PRESENCE_UPDATE", 10, presence("online"));

    let stats = wait_for_stat(&controller, |stats| stats.emit_failed == 1);
    assert_eq!(stats.queued, 1, "событие дошло до канала");
    assert_eq!(stats.delivered, 0);
    assert_eq!(
        stats.last_error.as_deref(),
        Some("UI-канал недоступен"),
        "ошибка эмита наблюдаема, а не проглочена"
    );
    assert_eq!(controller.status(), GatewayStatus::Ready);
}

#[test]
fn blocked_ui_channel_never_blocks_the_gateway_source() {
    let emitter = Arc::new(BlockingEmitter::new());
    let channel = DispatchChannel::start(emitter.clone(), 1);

    let first = DispatchPayload {
        kind: "PRESENCE_UPDATE".to_string(),
        data: Value::Object(Default::default()),
    };
    assert!(channel.deliver(first.clone()), "первый уходит в поток");
    emitter.wait_started();
    assert!(channel.deliver(first.clone()), "второй занимает очередь");
    let started = Instant::now();
    assert!(
        !channel.deliver(first),
        "переполнение очереди — отказ, а не блокировка"
    );
    assert!(
        started.elapsed() < Duration::from_secs(1),
        "handler не ждёт медленный UI-канал"
    );

    let stats = channel.stats();
    assert_eq!(stats.queued, 2);
    assert_eq!(stats.overflow, 1, "потеря под backpressure считается");

    emitter.unblock();
    let deadline = Instant::now() + Duration::from_secs(5);
    while channel.stats().delivered < 2 && Instant::now() < deadline {
        std::thread::sleep(Duration::from_millis(5));
    }
    assert_eq!(
        channel.stats().delivered,
        2,
        "очередь разгружается после UI"
    );
}

#[test]
fn duplicate_sequences_are_dropped_and_reconnect_does_not_replay() {
    let emitter = Arc::new(RecordingEmitter::default());
    let (controller, gateway) = controller_with(emitter.clone(), 16);

    gateway.dispatch("PRESENCE_UPDATE", 5, presence("online"));
    gateway.dispatch("PRESENCE_UPDATE", 5, presence("duplicate"));
    gateway.dispatch("PRESENCE_UPDATE", 6, presence("idle"));

    let seen = emitter.wait_for(2);
    assert_eq!(
        seen.iter()
            .map(|payload| payload.data["status"].as_str().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["online", "idle"],
        "повтор s той же сессии не доезжает до UI"
    );
    let stats = wait_for_stat(&controller, |stats| stats.delivered == 2);
    assert_eq!(stats.duplicates, 1);

    // После reconnect старые события не переигрываются, а новый поток снова
    // принимается (s начинается заново — дубликаты считаются по соединению).
    gateway.reconnecting();
    gateway.dispatch("PRESENCE_UPDATE", 5, presence("after-reconnect"));
    let seen = emitter.wait_for(3);
    assert_eq!(
        seen.len(),
        3,
        "реконнект не дублирует уже доставленные события"
    );
    assert_eq!(seen[2].data["status"], "after-reconnect");
    let stats = wait_for_stat(&controller, |stats| stats.delivered == 3);
    assert_eq!(stats.duplicates, 1, "сброс дедупа на новом соединении");
    assert_eq!(controller.status(), GatewayStatus::Reconnecting);
}

#[test]
fn non_forwarded_kind_is_counted_as_filtered() {
    let emitter = Arc::new(RecordingEmitter::default());
    let (controller, gateway) = controller_with(emitter.clone(), 8);

    gateway.dispatch("MESSAGE_CREATE", 3, EtfValue::Map(Vec::new()));
    gateway.dispatch("TOTALLY_UNKNOWN", 4, EtfValue::Map(Vec::new()));

    let stats = wait_for_stat(&controller, |stats| stats.filtered == 2);
    assert_eq!(stats.forwarded, 0);
    assert_eq!(stats.queued, 0);
    assert!(emitter.seen.lock().expect("lock").is_empty());
}

#[test]
fn broken_payload_is_counted_instead_of_panicking() {
    let emitter = Arc::new(RecordingEmitter::default());
    let (controller, gateway) = controller_with(emitter, 8);

    gateway.dispatch(
        "PRESENCE_UPDATE",
        3,
        EtfValue::Map(vec![(EtfValue::Nil, EtfValue::Int(1))]),
    );

    let stats = wait_for_stat(&controller, |stats| stats.unsupported == 1);
    assert_eq!(stats.forwarded, 0);
    assert_eq!(stats.delivered, 0);
    assert_eq!(controller.status(), GatewayStatus::Ready);
}

#[test]
fn dispatch_stats_report_core_source_available() {
    // L-02-CORE-FINAL: хук ядра подключён (`RunOptions.on_dispatch`), значит канал
    // `gateway://dispatch` имеет живой источник, а не только готовый конверт.
    let available = CORE_DISPATCH_AVAILABLE;
    assert!(available, "источник диспатчей в ядре подключён");
    assert_eq!(DISPATCH_EVENT, "gateway://dispatch");

    let controller = NetController::new(StaticTokenStore::new(Some("token")), FakeGateway::new());
    let stats = controller.dispatch_stats();
    assert_eq!(stats.source_available, CORE_DISPATCH_AVAILABLE);
    assert!(
        stats.source_detail.contains("on_dispatch"),
        "механизм источника назван: {}",
        stats.source_detail
    );
    assert_eq!(stats.seen, 0);
}

#[test]
fn core_supervisor_config_wires_lifecycle_and_dispatch() {
    // Проводка без сети: один callback получает и жизненный цикл, и сырой op0.
    // Если бы драйвер не подключал хук, `on_dispatch` был бы `None`, а
    // `CORE_DISPATCH_AVAILABLE = true` — ложью.
    let received = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&received);
    let config = discord_gui_spike::net::core_supervisor_config(
        "secret-token",
        Arc::new(move |event| sink.lock().expect("журнал событий").push(event)),
    );
    assert_eq!(
        config.gateway,
        discord_core::ws::GatewayUrl::default(),
        "боевой адрес шлюза"
    );

    let emit_lifecycle = config.on_event.as_ref().expect("on_event подключён");
    emit_lifecycle(SupervisorEvent::Ready);

    let event = DispatchEvent {
        kind: Some(discord_core::dispatch::DispatchKind::PresenceUpdate),
        name: "PRESENCE_UPDATE".to_string(),
        seq: Some(7),
        data: EtfValue::Map(vec![(
            EtfValue::Atom("status".to_string()),
            EtfValue::Atom("online".to_string()),
        )]),
    };
    let emit_dispatch = config.on_dispatch.as_ref().expect("on_dispatch подключён");
    emit_dispatch(&event);

    assert_eq!(
        *received.lock().expect("журнал событий"),
        vec![GatewayEvent::Ready, GatewayEvent::Dispatch(event)],
        "имя и `d` не переписываются, жизненный цикл идёт тем же callback"
    );
}
