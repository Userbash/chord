//! Контракт P6-02 (unit): D-Bus wire-формат — адрес шины, auth EXTERNAL, маршалинг,
//! разбор заголовка и тела, устойчивость к битым пакетам.
//!
//! Независимый оракул — `fixtures/dbus/wire.json`, сгенерированный `gen-golden.py`
//! через **libgio** (`Gio.DBusMessage.to_blob()`), а не Rust-кодом W30b: байты
//! сравниваются побайтово, порядок заголовочных полей берётся из фикстуры.
//! `fixtures/dbus/notifications.json` фиксирует сигнатуры настоящего libdbus-сервиса
//! (`mock_service.py` на приватном `dbus-daemon`).

use chord_platform::dbus::message::{self, FIELD_REPLY_SERIAL, FIELD_SIGNATURE, MessageType};
use chord_platform::dbus::{
    BusAddress, Connection, DbusError, MAX_ABSTRACT_NAME_BYTES, Value, auth, fd,
};
use chord_platform::notifications;
use chord_platform::notifications::{Hints, Urgency};
use std::fs;
use std::os::fd::{AsRawFd, RawFd};
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

// --- минимальный JSON-ридер фикстур (без внешних зависимостей) -------------------

#[derive(Debug, Clone, PartialEq)]
enum Json {
    Null,
    Bool(bool),
    Num(f64),
    Str(String),
    Arr(Vec<Json>),
    Obj(Vec<(String, Json)>),
}

impl Json {
    fn parse(text: &str) -> Result<Self, String> {
        let mut parser = JsonParser {
            bytes: text.as_bytes(),
            pos: 0,
        };
        let value = parser.value()?;
        parser.skip_ws();
        if parser.pos != parser.bytes.len() {
            return Err(format!("мусор после JSON на позиции {}", parser.pos));
        }
        Ok(value)
    }

    fn get(&self, key: &str) -> Option<&Json> {
        match self {
            Self::Obj(entries) => entries
                .iter()
                .find(|(name, _)| name == key)
                .map(|(_, value)| value),
            _ => None,
        }
    }

    fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(value) => Some(value),
            _ => None,
        }
    }

    fn as_u64(&self) -> Option<u64> {
        match self {
            Self::Num(value) if *value >= 0.0 => Some(*value as u64),
            _ => None,
        }
    }

    fn as_arr(&self) -> Option<&[Json]> {
        match self {
            Self::Arr(items) => Some(items),
            _ => None,
        }
    }
}

struct JsonParser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl JsonParser<'_> {
    fn skip_ws(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8) -> Result<(), String> {
        if self.bytes.get(self.pos) == Some(&byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(format!(
                "ожидался {:?} на позиции {}",
                byte as char, self.pos
            ))
        }
    }

    fn value(&mut self) -> Result<Json, String> {
        self.skip_ws();
        match self.bytes.get(self.pos).copied() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => Ok(Json::Str(self.string()?)),
            Some(b't') => self.literal("true", Json::Bool(true)),
            Some(b'f') => self.literal("false", Json::Bool(false)),
            Some(b'n') => self.literal("null", Json::Null),
            Some(byte) if byte == b'-' || byte.is_ascii_digit() => self.number(),
            other => Err(format!(
                "неожиданный байт {other:?} на позиции {}",
                self.pos
            )),
        }
    }

    fn literal(&mut self, text: &str, value: Json) -> Result<Json, String> {
        if self.bytes[self.pos..].starts_with(text.as_bytes()) {
            self.pos += text.len();
            Ok(value)
        } else {
            Err(format!("ожидался литерал {text} на позиции {}", self.pos))
        }
    }

    fn number(&mut self) -> Result<Json, String> {
        let start = self.pos;
        if self.bytes.get(self.pos) == Some(&b'-') {
            self.pos += 1;
        }
        while matches!(self.bytes.get(self.pos), Some(byte) if byte.is_ascii_digit()) {
            self.pos += 1;
        }
        if self.bytes.get(self.pos) == Some(&b'.') {
            self.pos += 1;
            while matches!(self.bytes.get(self.pos), Some(byte) if byte.is_ascii_digit()) {
                self.pos += 1;
            }
        }
        let text =
            std::str::from_utf8(&self.bytes[start..self.pos]).map_err(|err| err.to_string())?;
        text.parse::<f64>()
            .map(Json::Num)
            .map_err(|err| format!("число {text:?}: {err}"))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"')?;
        let mut out = String::new();
        loop {
            let byte = *self
                .bytes
                .get(self.pos)
                .ok_or_else(|| "незакрытая строка".to_string())?;
            self.pos += 1;
            match byte {
                b'"' => return Ok(out),
                b'\\' => {
                    let escape = *self
                        .bytes
                        .get(self.pos)
                        .ok_or_else(|| "оборванный escape".to_string())?;
                    self.pos += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{0008}'),
                        b'f' => out.push('\u{000C}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let hex = self
                                .bytes
                                .get(self.pos..self.pos + 4)
                                .ok_or_else(|| "короткий \\u".to_string())?;
                            self.pos += 4;
                            let code = u32::from_str_radix(
                                std::str::from_utf8(hex).map_err(|err| err.to_string())?,
                                16,
                            )
                            .map_err(|err| err.to_string())?;
                            out.push(char::from_u32(code).ok_or_else(|| "плохой \\u".to_string())?);
                        }
                        other => return Err(format!("неизвестный escape {other:?}")),
                    }
                }
                byte if byte < 0x20 => return Err(format!("control-байт {byte:?} в строке")),
                byte if byte < 0x80 => out.push(byte as char),
                _ => {
                    // UTF-8: возвращаемся к началу последовательности и читаем её целиком.
                    self.pos -= 1;
                    let rest = std::str::from_utf8(&self.bytes[self.pos..])
                        .map_err(|err| err.to_string())?;
                    let ch = rest
                        .chars()
                        .next()
                        .ok_or_else(|| "пустая строка".to_string())?;
                    out.push(ch);
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    fn array(&mut self) -> Result<Json, String> {
        self.expect(b'[')?;
        let mut items = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b']') {
            self.pos += 1;
            return Ok(Json::Arr(items));
        }
        loop {
            items.push(self.value()?);
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b']') => {
                    self.pos += 1;
                    return Ok(Json::Arr(items));
                }
                other => return Err(format!("ожидался ',' или ']', получено {other:?}")),
            }
        }
    }

    fn object(&mut self) -> Result<Json, String> {
        self.expect(b'{')?;
        let mut entries = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.pos) == Some(&b'}') {
            self.pos += 1;
            return Ok(Json::Obj(entries));
        }
        loop {
            self.skip_ws();
            let key = self.string()?;
            self.skip_ws();
            self.expect(b':')?;
            let value = self.value()?;
            entries.push((key, value));
            self.skip_ws();
            match self.bytes.get(self.pos) {
                Some(b',') => self.pos += 1,
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(Json::Obj(entries));
                }
                other => return Err(format!("ожидался ',' или '}}', получено {other:?}")),
            }
        }
    }
}

// --- вспомогательные функции теста ----------------------------------------------

fn fixture_path(name: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../fixtures/dbus")
        .join(name)
}

fn load_fixture(name: &str) -> Json {
    let path = fixture_path(name);
    let text = std::fs::read_to_string(&path)
        .unwrap_or_else(|err| panic!("фикстура {}: {err}", path.display()));
    Json::parse(&text).unwrap_or_else(|err| panic!("JSON {}: {err}", path.display()))
}

fn hex_decode(text: &str) -> Vec<u8> {
    assert_eq!(text.len() % 2, 0, "нечётная hex-строка");
    let mut out = Vec::with_capacity(text.len() / 2);
    for chunk in text.as_bytes().chunks(2) {
        let pair = std::str::from_utf8(chunk).expect("hex ascii");
        out.push(u8::from_str_radix(pair, 16).expect("hex pair"));
    }
    out
}

fn hex_encode(bytes: &[u8]) -> String {
    let mut out = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn golden_messages() -> Vec<(String, Vec<u8>)> {
    let wire = load_fixture("wire.json");
    let messages = wire
        .get("messages")
        .and_then(Json::as_arr)
        .expect("wire.json: messages");
    messages
        .iter()
        .map(|message| {
            let name = message
                .get("name")
                .and_then(Json::as_str)
                .expect("имя сообщения")
                .to_owned();
            let hex = message
                .get("hex")
                .and_then(Json::as_str)
                .expect("hex сообщения");
            (name, hex_decode(hex))
        })
        .collect()
}

fn golden_message(name: &str) -> Vec<u8> {
    golden_messages()
        .into_iter()
        .find(|(candidate, _)| candidate == name)
        .unwrap_or_else(|| panic!("нет золотого сообщения {name}"))
        .1
}

fn expected_serial(name: &str) -> u64 {
    let wire = load_fixture("wire.json");
    let messages = wire
        .get("messages")
        .and_then(Json::as_arr)
        .expect("wire.json: messages");
    messages
        .iter()
        .find(|message| message.get("name").and_then(Json::as_str) == Some(name))
        .and_then(|message| message.get("serial"))
        .and_then(Json::as_u64)
        .unwrap_or_else(|| panic!("{name}: нет serial"))
}

fn golden_attr(name: &str, key: &str) -> String {
    let wire = load_fixture("wire.json");
    let messages = wire
        .get("messages")
        .and_then(Json::as_arr)
        .expect("wire.json: messages");
    messages
        .iter()
        .find(|message| message.get("name").and_then(Json::as_str) == Some(name))
        .and_then(|message| message.get(key))
        .and_then(Json::as_str)
        .unwrap_or_else(|| panic!("{name}: нет строкового поля {key}"))
        .to_owned()
}

// --- n01: адрес шины (path-ветка; abstract — в a01–a07) -------------------------

#[test]
fn n01_bus_address_parse_accepts_path_and_rejects_unsupported() {
    let plain = BusAddress::parse("unix:path=/run/user/1000/bus").expect("unix:path");
    assert_eq!(plain.path(), Some(Path::new("/run/user/1000/bus")));
    assert_eq!(plain.abstract_name(), None);

    assert_eq!(BusAddress::parse("tcp:host=localhost,port=1234"), None);
    assert_eq!(BusAddress::parse(""), None);
    assert_eq!(BusAddress::parse("unix:path="), None);
    assert_eq!(BusAddress::parse("мусор"), None);

    // Список адресов: берётся первый поддерживаемый (abstract поддерживается с W34a).
    let list =
        BusAddress::parse("unix:abstract=/tmp/abstract;unix:path=/tmp/real").expect("список");
    assert_eq!(list.abstract_name(), Some(&b"/tmp/abstract"[..]));
    assert_eq!(list.path(), None);

    // guid и прочие ключи в той же записи не мешают.
    let with_guid = BusAddress::parse("unix:guid=abc,path=/tmp/a").expect("path после guid");
    assert_eq!(with_guid.path(), Some(Path::new("/tmp/a")));

    // `from_env` — ровно разбор DBUS_SESSION_BUS_ADDRESS (если переменная задана).
    match std::env::var("DBUS_SESSION_BUS_ADDRESS") {
        Ok(value) => {
            let from_env = BusAddress::from_env().expect("адрес из env");
            let parsed = BusAddress::parse(&value).expect("разбор env-адреса");
            assert_eq!(from_env, parsed);
        }
        Err(_) => {
            println!("INFO: DBUS_SESSION_BUS_ADDRESS не задан — ветка from_env не проверяется")
        }
    }
}

// --- n02: uid для AUTH EXTERNAL --------------------------------------------------

#[test]
fn n02b_external_auth_line_uses_effective_uid_from_os() {
    let output = Command::new("id").arg("-u").output().expect("id -u");
    assert!(output.status.success(), "id -u завершился неуспешно");
    let expected: u32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("uid из id -u");

    assert_eq!(
        auth::current_uid().expect("effective UID from the operating system"),
        expected
    );
    // Идентичность EXTERNAL — десятичная запись uid в hex (libdbus: 1000 → "31303030").
    let decimal_hex: String = expected
        .to_string()
        .bytes()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    assert_eq!(
        auth::external_auth_line(expected),
        format!("AUTH EXTERNAL {decimal_hex}")
    );
    assert_eq!(auth::external_auth_line(1000), "AUTH EXTERNAL 31303030");
    assert_eq!(auth::parse_uid("Name:\troot\nUid:\t0\t0\t0\t0\n"), Some(0));
    assert_eq!(
        auth::parse_uid("Uid:\t1000\t1000\t1000\t1000\n"),
        Some(1000)
    );
    assert_eq!(auth::parse_uid("Name:\tx\n"), None);
}

// --- n03/n04: маршалинг и выравнивание против libgio -----------------------------

#[test]
fn n03_marshal_round_trip_matches_libgio_bytes() {
    let messages = golden_messages();
    assert!(
        messages.len() >= 4,
        "ожидались золотые сообщения (call/signal/reply/error), получено {}",
        messages.len()
    );

    for (name, bytes) in messages {
        let parsed = message::parse(&bytes).unwrap_or_else(|err| panic!("{name}: {err:?}"));
        assert_eq!(
            parsed.serial as u64,
            expected_serial(&name),
            "{name}: serial"
        );
        let rebuilt = message::build(
            parsed.kind,
            parsed.serial,
            &parsed.fields,
            &parsed.signature,
            &parsed.body,
        )
        .unwrap_or_else(|err| panic!("{name}: сборка: {err:?}"));
        assert_eq!(
            hex_encode(&rebuilt),
            hex_encode(&bytes),
            "{name}: байты расходятся с libgio"
        );
    }
}

#[test]
fn n04_header_body_boundary_is_aligned_and_padded_with_zeroes() {
    for (name, bytes) in golden_messages() {
        assert!(bytes.len() >= 16, "{name}: короткое сообщение");
        let body_len = u32::from_le_bytes(bytes[4..8].try_into().expect("body len")) as usize;
        let fields_len = u32::from_le_bytes(bytes[12..16].try_into().expect("fields len")) as usize;
        let fields_end = 16 + fields_len;
        let body_offset = (fields_end + 7) & !7;

        assert_eq!(body_offset % 8, 0, "{name}: тело не выровнено по 8");
        assert!(
            bytes[fields_end..body_offset].iter().all(|byte| *byte == 0),
            "{name}: padding между заголовком и телом не нулевой"
        );
        assert_eq!(
            bytes.len(),
            body_offset + body_len,
            "{name}: длина сообщения не совпадает с body_len"
        );
    }

    // `a(ss)` из golden: два элемента-структуры идут с шагом 8, пары доходят целиком.
    let parsed = message::parse(&golden_message("pairs_signal")).expect("pairs_signal");
    assert_eq!(parsed.signature, "a(ss)");
    assert_eq!(
        parsed.body,
        vec![Value::Array(vec![
            Value::Struct(vec![
                Value::String("a".to_owned()),
                Value::String("b".to_owned()),
            ]),
            Value::Struct(vec![
                Value::String("c".to_owned()),
                Value::String("d".to_owned()),
            ]),
        ])]
    );
}

// --- n05: заголовок и тело -------------------------------------------------------

#[test]
fn n05_parse_recovers_header_fields_and_body() {
    let call = message::parse(&golden_message("notify_call")).expect("notify_call");
    assert_eq!(call.kind, MessageType::MethodCall);
    assert_eq!(call.serial, 7);
    assert_eq!(call.path(), Some("/org/freedesktop/Notifications"));
    assert_eq!(call.interface(), Some("org.freedesktop.Notifications"));
    assert_eq!(call.member(), Some("Notify"));
    assert_eq!(call.destination(), Some("org.freedesktop.Notifications"));
    assert_eq!(call.signature, "susssasa{sv}i");
    assert_eq!(call.reply_serial(), None);
    assert_eq!(call.error_name(), None);
    assert_eq!(
        call.body,
        vec![
            Value::String("Discord Lab".to_owned()),
            Value::U32(0),
            Value::String(String::new()),
            Value::String("Alice".to_owned()),
            Value::String("hello".to_owned()),
            Value::Array(vec![
                Value::String("open".to_owned()),
                Value::String("Open".to_owned()),
            ]),
            Value::Dict(Vec::new()),
            Value::I32(-1),
        ]
    );

    let signal = message::parse(&golden_message("action_invoked_signal")).expect("signal");
    assert_eq!(signal.kind, MessageType::Signal);
    assert_eq!(signal.serial, 3);
    assert_eq!(signal.path(), Some("/org/freedesktop/Notifications"));
    assert_eq!(signal.interface(), Some("org.freedesktop.Notifications"));
    assert_eq!(signal.member(), Some("ActionInvoked"));
    // Тело сигнала `ActionInvoked` — `us`: два значения подряд, без struct-обёртки.
    assert_eq!(signal.signature, "us");
    assert_eq!(
        signal.body,
        vec![Value::U32(100), Value::String("open".to_owned()),]
    );

    let reply = message::parse(&golden_message("capabilities_reply")).expect("reply");
    assert_eq!(reply.kind, MessageType::MethodReturn);
    assert_eq!(reply.reply_serial(), Some(7));
    assert_eq!(reply.signature, "as");
    assert_eq!(
        reply.body,
        vec![Value::Array(vec![
            Value::String("body".to_owned()),
            Value::String("actions".to_owned()),
            Value::String("persistence".to_owned()),
            Value::String("icon-static".to_owned()),
        ])]
    );

    let error = message::parse(&golden_message("error_reply")).expect("error");
    assert_eq!(error.kind, MessageType::Error);
    assert_eq!(error.reply_serial(), Some(7));
    assert_eq!(
        error.error_name(),
        Some("org.freedesktop.Notifications.Error.MockRejected")
    );
    assert_eq!(
        error.body,
        vec![Value::String("mock rejected notification".to_owned())]
    );

    // Поле SIGNATURE разбирается как тип `g`, а не как обычная строка.
    assert_eq!(
        call.field(FIELD_SIGNATURE),
        Some(&Value::String("susssasa{sv}i".to_owned()))
    );
    assert_eq!(golden_attr("notify_call", "signature"), call.signature);
}

// --- n14: битые пакеты -----------------------------------------------------------

#[test]
fn n14_parse_rejects_truncated_and_corrupt_messages_without_panic() {
    let bytes = golden_message("notify_call");

    for cut in [
        0usize,
        1,
        8,
        15,
        16,
        17,
        32,
        bytes.len() / 2,
        bytes.len() - 1,
    ] {
        let result = message::parse(&bytes[..cut]);
        assert!(
            matches!(result, Err(DbusError::Protocol(_))),
            "обрезание {cut}: {result:?}"
        );
    }

    let mut big_endian = bytes.clone();
    big_endian[0] = b'B';
    assert!(matches!(
        message::parse(&big_endian),
        Err(DbusError::Protocol(_))
    ));

    let mut unknown_type = bytes.clone();
    unknown_type[1] = 9;
    assert!(matches!(
        message::parse(&unknown_type),
        Err(DbusError::Protocol(_))
    ));

    let mut trailing = bytes.clone();
    trailing.push(0);
    assert!(
        matches!(message::parse(&trailing), Err(DbusError::Protocol(_))),
        "лишний байт после тела должен быть Protocol"
    );

    let mut bad_body_len = bytes.clone();
    bad_body_len[4..8].copy_from_slice(&0x00ff_ffffu32.to_le_bytes());
    assert!(matches!(
        message::parse(&bad_body_len),
        Err(DbusError::Protocol(_))
    ));

    assert!(matches!(
        message::parse(b"GET / HTTP/1.1\r\n\r\n"),
        Err(DbusError::Protocol(_))
    ));
}

// --- сигнатуры уведомлений против libdbus-оракула --------------------------------

#[test]
fn n07b_notifications_signatures_match_libdbus_golden() {
    let golden = load_fixture("notifications.json");
    let methods = golden
        .get("methods")
        .and_then(Json::as_arr)
        .expect("notifications.json: methods");

    let method = |name: &str| {
        methods
            .iter()
            .find(|method| method.get("member").and_then(Json::as_str) == Some(name))
            .unwrap_or_else(|| panic!("нет метода {name}"))
    };

    let notify = method("Notify");
    assert_eq!(
        notify.get("signature").and_then(Json::as_str),
        Some(notifications::NOTIFY_SIGNATURE)
    );
    assert_eq!(
        notify.get("reply_signature").and_then(Json::as_str),
        Some("u")
    );
    assert_eq!(
        method("GetCapabilities")
            .get("reply_signature")
            .and_then(Json::as_str),
        Some("as")
    );
    assert_eq!(
        method("GetServerInformation")
            .get("reply_signature")
            .and_then(Json::as_str),
        Some("ssss")
    );
    assert_eq!(
        method("CloseNotification")
            .get("signature")
            .and_then(Json::as_str),
        Some("u")
    );

    // Отсутствие поддержки `actions`/`body` в golden нет: мок объявляет capabilities,
    // и клиент обязан отдавать их без искажений.
    let capabilities = method("GetCapabilities")
        .get("reply")
        .and_then(Json::as_arr)
        .expect("GetCapabilities.reply");
    let declared: Vec<&str> = capabilities.iter().filter_map(Json::as_str).collect();
    for required in ["body", "actions", "persistence"] {
        assert!(
            declared.contains(&required),
            "мок не объявил {required}: {declared:?}"
        );
    }

    // Ответ того же вида, собранный нашим кодом, разбирается обратно без потерь.
    let body = vec![Value::Array(
        declared
            .iter()
            .map(|item| Value::String((*item).to_owned()))
            .collect(),
    )];
    let reply = message::build(
        MessageType::MethodReturn,
        4,
        &[
            (FIELD_SIGNATURE, Value::String("as".to_owned())),
            (FIELD_REPLY_SERIAL, Value::U32(0)),
        ],
        "as",
        &body,
    )
    .expect("сборка MethodReturn");
    let parsed = message::parse(&reply).expect("разбор MethodReturn");
    assert_eq!(parsed.reply_serial(), Some(0));
    assert_eq!(parsed.body, body);
}

// --- q08: hints `a{sv}` — байты libgio и round-trip ------------------------------

/// Собирает `Hints` из эталонного набора `fixtures/dbus/hints.json`.
fn json_hints(items: &[Json]) -> Hints {
    let mut hints = Hints::default();
    for item in items {
        let key = item.get("key").and_then(Json::as_str).expect("hints.key");
        let signature = item
            .get("signature")
            .and_then(Json::as_str)
            .expect("hints.signature");
        let value = item.get("value").expect("hints.value");
        match (key, signature) {
            ("urgency", "y") => {
                hints.urgency = Some(match value.as_u64() {
                    Some(0) => Urgency::Low,
                    Some(1) => Urgency::Normal,
                    Some(2) => Urgency::Critical,
                    other => panic!("{key}: неожиданная urgency {other:?}"),
                });
            }
            ("category", "s") => hints.category = value.as_str().map(str::to_owned),
            ("desktop-entry", "s") => hints.desktop_entry = value.as_str().map(str::to_owned),
            ("resident", "b") => {
                hints.resident = Some(match value {
                    Json::Bool(flag) => *flag,
                    other => panic!("{key}: ожидался bool, получено {other:?}"),
                });
            }
            other => panic!("неизвестная запись hints: {other:?}"),
        }
    }
    hints
}

/// Ожидаемый `a{sv}` из фикстуры: ключи, порядок и **типы вариантов** заданы оракулом.
fn json_hints_value(items: &[Json]) -> Value {
    let mut entries = Vec::new();
    for item in items {
        let key = item.get("key").and_then(Json::as_str).expect("hints.key");
        let signature = item
            .get("signature")
            .and_then(Json::as_str)
            .expect("hints.signature");
        let value = item.get("value").expect("hints.value");
        let inner = match signature {
            "y" => Value::Byte(
                u8::try_from(value.as_u64().expect("hints.y")).expect("hints.y в диапазоне u8"),
            ),
            "s" => Value::String(value.as_str().expect("hints.s").to_owned()),
            "b" => Value::Bool(match value {
                Json::Bool(flag) => *flag,
                other => panic!("{key}: ожидался bool, получено {other:?}"),
            }),
            other => panic!("{key}: неизвестная сигнатура варианта {other:?}"),
        };
        entries.push((
            Value::String(key.to_owned()),
            Value::Variant(Box::new(inner)),
        ));
    }
    Value::Dict(entries)
}

/// Тело `Notify`, собранное из тех же общих полей, что и golden в `hints.json`.
fn hints_notify_body(golden: &Json, hints: &Hints) -> Vec<Value> {
    let notify = golden.get("notify").expect("hints.json: notify");
    let text = |key: &str| {
        notify
            .get(key)
            .and_then(Json::as_str)
            .unwrap_or_else(|| panic!("hints.json: notify.{key}"))
            .to_owned()
    };
    let actions = notify
        .get("actions")
        .and_then(Json::as_arr)
        .expect("hints.json: notify.actions")
        .iter()
        .map(|item| Value::String(item.as_str().expect("notify.action").to_owned()))
        .collect();
    let timeout_ms = match notify
        .get("timeout_ms")
        .expect("hints.json: notify.timeout_ms")
    {
        Json::Num(value) => *value as i32,
        other => panic!("timeout_ms: {other:?}"),
    };

    vec![
        Value::String(text("app_name")),
        Value::U32(0),                // replaces_id
        Value::String(String::new()), // app_icon
        Value::String(text("summary")),
        Value::String(text("body")),
        Value::Array(actions),
        hints.to_dbus(),
        Value::I32(timeout_ms),
    ]
}

#[test]
fn q08_hints_dict_matches_libgio_bytes_and_round_trips() {
    let golden = load_fixture("hints.json");
    assert_eq!(
        golden.get("hints_signature").and_then(Json::as_str),
        Some("a{sv}"),
        "hints — именно a{{sv}}"
    );
    assert_eq!(
        golden.get("signature").and_then(Json::as_str),
        Some(notifications::NOTIFY_SIGNATURE),
        "сигнатура Notify не меняется"
    );

    let cases = golden
        .get("cases")
        .and_then(Json::as_arr)
        .expect("hints.json: cases");
    assert!(
        cases.len() >= 5,
        "ожидались наборы empty/urgency/category/desktop-entry+resident/all, есть {}",
        cases.len()
    );

    for case in cases {
        let name = case.get("name").and_then(Json::as_str).expect("case.name");
        let items = case
            .get("hints")
            .and_then(Json::as_arr)
            .expect("case.hints");
        let hints = json_hints(items);
        let expected = json_hints_value(items);

        // Структура, типы вариантов и порядок записей: urgency, category, desktop-entry,
        // resident (в фикстуре dict уже в этом порядке).
        assert_eq!(hints.to_dbus(), expected, "{name}: форма a{{sv}}");
        assert_eq!(
            hints.to_dbus(),
            hints.to_dbus(),
            "{name}: порядок записей нестабилен"
        );

        let hex = case
            .get("notify_hex")
            .and_then(Json::as_str)
            .expect("case.notify_hex");
        let bytes = hex_decode(hex);
        let libgio = message::parse(&bytes)
            .unwrap_or_else(|err| panic!("{name}: байты libgio не разобраны: {err:?}"));
        assert_eq!(libgio.kind, MessageType::MethodCall);
        assert_eq!(libgio.member(), Some("Notify"));
        assert_eq!(
            libgio.signature,
            notifications::NOTIFY_SIGNATURE,
            "{name}: сигнатура вызова"
        );
        assert_eq!(
            libgio.body.get(6),
            Some(&expected),
            "{name}: libdbus/libgio демаршалит другой a{{sv}}"
        );

        // Сборка нашего сообщения из разобранных полей заголовка и нашего `a{sv}`:
        // побайтовое совпадение с libgio проверяет выравнивание и порядок записей.
        let body = hints_notify_body(&golden, &hints);
        let rebuilt = message::build(
            MessageType::MethodCall,
            libgio.serial,
            &libgio.fields,
            &libgio.signature,
            &body,
        )
        .unwrap_or_else(|err| panic!("{name}: сборка сообщения: {err:?}"));
        assert_eq!(
            hex_encode(&rebuilt),
            hex_encode(&bytes),
            "{name}: байты a{{sv}} расходятся с libgio"
        );

        let round_trip = message::parse(&rebuilt).expect("разбор собранного сообщения");
        assert_eq!(round_trip.body, body, "{name}: round-trip тела");
        assert_eq!(
            round_trip.body.get(6),
            Some(&expected),
            "{name}: round-trip a{{sv}}"
        );
    }
}

// --- W34a (P6-02c): abstract-адреса `unix:abstract=…` ---------------------------
//
// Независимый оракул — приватный `dbus-daemon --address=unix:abstract=…` и libdbus-мок
// `waves/RW-30/dbus/mock_service.py` (журнал TSV). Наш код здесь только клиент.
// Если окружение запрещает abstract-сокеты, тест печатает `SKIP: причина` и не падает.

const SKIP_NO_DAEMON: &str = "нет dbus-daemon";
const SKIP_NO_PYTHON_DBUS: &str = "нет python3 с модулями dbus и gi";
const MOCK_DESTINATION: &str = "org.freedesktop.Notifications";
const MOCK_PATH: &str = "/org/freedesktop/Notifications";
const MOCK_INTERFACE: &str = "org.freedesktop.Notifications";

/// Приватная шина (`unix:abstract=…` либо штатный `unix:path`) и, по запросу, мок.
/// Демон, мок и tempdir убираются в `Drop`.
struct TestBus {
    address: BusAddress,
    journal: Option<PathBuf>,
    dir: PathBuf,
    daemon_pid: u32,
    mock: Option<Child>,
}

impl TestBus {
    /// Приватная шина строго на abstract-адресе + мок уведомлений.
    fn abstract_with_mock() -> Result<Self, String> {
        if !command_exists("dbus-daemon") {
            return Err(SKIP_NO_DAEMON.to_owned());
        }
        if !python_dbus_available() {
            return Err(SKIP_NO_PYTHON_DBUS.to_owned());
        }
        let name = format!("discord-lab-{}-{}", std::process::id(), unique_suffix());
        Self::start(&format!("unix:abstract={name}"), true)
    }

    /// Штатная приватная шина `--session`: dbus-daemon печатает `unix:path=…`.
    fn path_only() -> Result<Self, String> {
        if !command_exists("dbus-daemon") {
            return Err(SKIP_NO_DAEMON.to_owned());
        }
        Self::start("", false)
    }

    fn start(address_arg: &str, with_mock: bool) -> Result<Self, String> {
        let dir = std::env::temp_dir().join(format!(
            "discord-lab-w34a-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&dir).map_err(|err| format!("tempdir {}: {err}", dir.display()))?;

        let mut daemon = Command::new("dbus-daemon");
        daemon.args(["--session", "--print-address=1", "--print-pid=1", "--fork"]);
        if !address_arg.is_empty() {
            daemon.arg(format!("--address={address_arg}"));
        }
        let started = match daemon.output() {
            Ok(output) => output,
            Err(err) => {
                let _ = fs::remove_dir_all(&dir);
                return Err(format!("запуск dbus-daemon: {err}"));
            }
        };
        if !started.status.success() {
            let stderr = String::from_utf8_lossy(&started.stderr).trim().to_owned();
            let _ = fs::remove_dir_all(&dir);
            return Err(format!("dbus-daemon не поднялся: {stderr}"));
        }
        let stdout = String::from_utf8_lossy(&started.stdout).into_owned();
        let mut lines = stdout.lines().filter(|line| !line.trim().is_empty());
        let printed = match lines.next() {
            Some(line) => line.trim().to_owned(),
            None => {
                let _ = fs::remove_dir_all(&dir);
                return Err("dbus-daemon не напечатал адрес".to_owned());
            }
        };
        let daemon_pid: u32 = match lines
            .next()
            .ok_or_else(|| "dbus-daemon не напечатал pid".to_owned())
            .and_then(|line| {
                line.trim()
                    .parse()
                    .map_err(|err| format!("pid dbus-daemon: {err}"))
            }) {
            Ok(pid) => pid,
            Err(reason) => {
                let _ = fs::remove_dir_all(&dir);
                return Err(reason);
            }
        };

        // Адрес печатает демон, разбирает наш `BusAddress`: расхождение — дефект
        // продукта, поэтому паника, а не skip.
        let address = BusAddress::parse(&printed)
            .unwrap_or_else(|| panic!("BusAddress::parse не разобрал адрес шины {printed:?}"));
        if address_arg.starts_with("unix:abstract=") {
            assert!(
                address.abstract_name().is_some_and(|name| !name.is_empty()),
                "демон слушает abstract-адрес {address_arg:?}, напечатано {printed:?}"
            );
        }

        let (journal, mock) = if with_mock {
            let journal = dir.join("journal.tsv");
            let mut mock = Command::new("python3")
                .arg(repo_path("waves/RW-30/dbus/mock_service.py"))
                .arg("--address")
                .arg(&printed)
                .arg("--log")
                .arg(&journal)
                .stdout(Stdio::null())
                .stderr(Stdio::piped())
                .spawn()
                .map_err(|err| abort(daemon_pid, &dir, format!("запуск mock_service.py: {err}")))?;
            wait_ready(&mut mock, &journal, daemon_pid, &dir)?;
            (Some(journal), Some(mock))
        } else {
            (None, None)
        };

        Ok(Self {
            address,
            journal,
            dir,
            daemon_pid,
            mock,
        })
    }

    fn journal_member(&self, member: &str) -> Vec<Vec<String>> {
        let Some(journal) = self.journal.as_ref() else {
            return Vec::new();
        };
        let text = fs::read_to_string(journal).unwrap_or_default();
        text.lines()
            .map(|line| line.split('\t').map(str::to_owned).collect::<Vec<_>>())
            .filter(|fields| fields.first().map(String::as_str) == Some(member))
            .collect()
    }

    /// Ждёт `count` записей журнала: мок пишет их асинхронно.
    fn wait_for_member(&self, member: &str, count: usize) -> Vec<Vec<String>> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let records = self.journal_member(member);
            if records.len() >= count || Instant::now() > deadline {
                return records;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for TestBus {
    fn drop(&mut self) {
        if let Some(mock) = self.mock.as_mut() {
            let _ = mock.kill();
            let _ = mock.wait();
        }
        if self.daemon_pid != 0 {
            let _ = Command::new("kill")
                .arg(self.daemon_pid.to_string())
                .status();
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

/// Убирает приватный демон и tempdir при ошибке старта (skip-путь без `Drop`).
fn abort(daemon_pid: u32, dir: &Path, reason: String) -> String {
    let _ = Command::new("kill").arg(daemon_pid.to_string()).status();
    let _ = fs::remove_dir_all(dir);
    reason
}

/// Ждёт строку `READY` в журнале мока; иначе — причина для skip с очисткой.
fn wait_ready(mock: &mut Child, journal: &Path, daemon_pid: u32, dir: &Path) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if fs::read_to_string(journal)
            .map(|text| text.contains("READY"))
            .unwrap_or(false)
        {
            return Ok(());
        }
        if let Ok(Some(status)) = mock.try_wait() {
            return Err(abort(
                daemon_pid,
                dir,
                format!("мок-сервис завершился ({status})"),
            ));
        }
        if Instant::now() > deadline {
            return Err(abort(
                daemon_pid,
                dir,
                "мок-сервис не поднялся за 10 с".to_owned(),
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn repo_path(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join(relative)
}

fn command_exists(name: &str) -> bool {
    Command::new("sh")
        .arg("-c")
        .arg(format!("command -v {name}"))
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn python_dbus_available() -> bool {
    Command::new("python3")
        .args(["-c", "import dbus, gi"])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .map(|status| status.success())
        .unwrap_or(false)
}

fn unique_suffix() -> u64 {
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(0);
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|duration| duration.as_nanos() as u64)
        .unwrap_or(0);
    nanos ^ (COUNTER.fetch_add(1, Ordering::Relaxed) << 32)
}

/// Поднимает стенд или печатает `SKIP: причина` и выходит из теста.
macro_rules! test_bus {
    ($constructor:expr) => {
        match $constructor {
            Ok(bus) => bus,
            Err(reason) => {
                println!("SKIP: {reason}");
                return;
            }
        }
    };
}

// --- a01: разбор `unix:path` -----------------------------------------------------

#[test]
fn a01_parse_path_yields_path_and_no_abstract_name() {
    let address = BusAddress::parse("unix:path=/run/user/1000/bus").expect("unix:path");
    assert_eq!(address.path(), Some(Path::new("/run/user/1000/bus")));
    assert_eq!(address.abstract_name(), None);
    assert!(matches!(address, BusAddress::Path(_)));
}

// --- a02: разбор `unix:abstract` -------------------------------------------------

#[test]
fn a02_parse_abstract_keeps_raw_bytes_up_to_kernel_limit() {
    let address = BusAddress::parse("unix:abstract=/tmp/dbus-abc").expect("unix:abstract");
    assert_eq!(address.abstract_name(), Some(&b"/tmp/dbus-abc"[..]));
    assert_eq!(address.path(), None);
    assert!(matches!(address, BusAddress::Abstract(_)));

    // `guid`, который dbus-daemon дописывает к своему адресу, не входит в имя.
    let with_guid =
        BusAddress::parse("unix:abstract=discord-lab-42,guid=0f0e2d").expect("abstract + guid");
    assert_eq!(with_guid.abstract_name(), Some(&b"discord-lab-42"[..]));

    // Linux: имя abstract-сокета — не длиннее 107 байт.
    let longest = "a".repeat(MAX_ABSTRACT_NAME_BYTES);
    let at_limit = BusAddress::parse(&format!("unix:abstract={longest}")).expect("107 байт");
    assert_eq!(
        at_limit.abstract_name().map(<[u8]>::len),
        Some(MAX_ABSTRACT_NAME_BYTES)
    );
    let too_long = "a".repeat(MAX_ABSTRACT_NAME_BYTES + 1);
    assert_eq!(
        BusAddress::parse(&format!("unix:abstract={too_long}")),
        None
    );
}

// --- a03: список `;`, мусор, `tcp:`, NUL -----------------------------------------

#[test]
fn a03_parse_skips_unsupported_and_invalid_entries() {
    assert_eq!(BusAddress::parse("tcp:host=localhost,port=1234"), None);
    assert_eq!(BusAddress::parse(""), None);
    assert_eq!(BusAddress::parse("мусор"), None);
    assert_eq!(BusAddress::parse("unix:path="), None);
    assert_eq!(BusAddress::parse("unix:abstract="), None);
    assert_eq!(BusAddress::parse("unix:path=/tmp/\0bus"), None);
    assert_eq!(BusAddress::parse("unix:abstract=bad\0name"), None);
    assert_eq!(BusAddress::parse("unix:guid=abc"), None);

    // Первый поддерживаемый адрес в списке, а не первый вообще.
    let mixed = BusAddress::parse("tcp:host=localhost;unix:abstract=bus-name;unix:path=/tmp/b")
        .expect("abstract в середине списка");
    assert_eq!(mixed.abstract_name(), Some(&b"bus-name"[..]));
    assert_eq!(mixed.path(), None);

    // Невалидная запись пропускается, валидная следующая — берётся.
    let after_invalid = BusAddress::parse("unix:abstract=;unix:path=/tmp/real")
        .expect("path после пустого abstract");
    assert_eq!(after_invalid.path(), Some(Path::new("/tmp/real")));

    // Неизвестные ключи (`guid`) в той же записи не мешают.
    let with_guid = BusAddress::parse("unix:guid=abc,path=/tmp/a").expect("path после guid");
    assert_eq!(with_guid.path(), Some(Path::new("/tmp/a")));
}

// --- a04: connect abstract → Hello -----------------------------------------------

#[test]
fn a04_connect_abstract_bus_yields_unique_name() {
    let bus = test_bus!(TestBus::abstract_with_mock());
    assert!(
        bus.address.abstract_name().is_some(),
        "стенд обязан быть на abstract-адресе"
    );
    assert_eq!(bus.address.path(), None);

    let connection = Connection::connect(&bus.address)
        .unwrap_or_else(|err| panic!("connect abstract + Hello: {err:?}"));
    let name = connection.unique_name();
    assert!(name.starts_with(":1."), "уникальное имя: {name:?}");
    assert!(!name.is_empty(), "уникальное имя пустое");
}

// --- a05: вызов через abstract-шину ----------------------------------------------

#[test]
fn a05_call_over_abstract_bus_returns_mock_reply() {
    let bus = test_bus!(TestBus::abstract_with_mock());
    let mut connection =
        Connection::connect(&bus.address).unwrap_or_else(|err| panic!("connect abstract: {err:?}"));

    let reply = connection
        .call(
            MOCK_DESTINATION,
            MOCK_PATH,
            MOCK_INTERFACE,
            "GetCapabilities",
            &[],
        )
        .unwrap_or_else(|err| panic!("GetCapabilities через abstract: {err:?}"));
    assert_eq!(
        reply,
        vec![Value::Array(vec![
            Value::String("body".to_owned()),
            Value::String("actions".to_owned()),
            Value::String("persistence".to_owned()),
            Value::String("icon-static".to_owned()),
        ])]
    );

    // Журнал libdbus-мока подтверждает, что вызов дошёл именно по abstract-шине.
    let records = bus.wait_for_member("GetCapabilities", 1);
    assert_eq!(
        records.len(),
        1,
        "мок не увидел GetCapabilities: {:?}",
        records
    );
}

// --- a06: несуществующий abstract ------------------------------------------------

#[test]
#[cfg(target_os = "linux")]
fn a06_connect_missing_abstract_returns_error_without_panic() {
    let name = format!(
        "discord-lab-missing-{}-{}",
        std::process::id(),
        unique_suffix()
    );
    let address = BusAddress::parse(&format!("unix:abstract={name}")).expect("abstract-адрес");
    assert_eq!(address.abstract_name(), Some(name.as_bytes()));

    match Connection::connect(&address) {
        Ok(_) => panic!("подключение к несуществующему abstract не должно удаваться"),
        Err(DbusError::Io(error)) => {
            println!("INFO: ожидаемая ошибка connect: {error}");
        }
        Err(other) => panic!("ожидалась DbusError::Io, получено {other:?}"),
    }
}

// --- a07: path-ветка не сломана --------------------------------------------------

#[test]
fn a07_path_branch_still_connects_to_path_bus() {
    let bus = test_bus!(TestBus::path_only());
    assert!(
        bus.address.path().is_some(),
        "штатный --session обязан дать unix:path-адрес"
    );
    assert_eq!(bus.address.abstract_name(), None);

    let connection =
        Connection::connect(&bus.address).unwrap_or_else(|err| panic!("connect path: {err:?}"));
    assert!(
        connection.unique_name().starts_with(":1."),
        "уникальное имя: {:?}",
        connection.unique_name()
    );
}

// --- W35a (P6-06c): передача unix-fd (SCM_RIGHTS) --------------------------------
//
// Оракул — спецификация D-Bus (тело `h` = u32-индекс в массив fd, приложенный к
// сообщению ancillary-данными) и libdbus-мок `waves/RW-35/power/mock_logind.py`,
// который отдаёт реальные fd через настоящий `dbus-daemon`. Тесты не подгоняют
// поведение под нашу реализацию: fd-ы создаются моком/тестом и проверяются через
// `/proc/self/fd`.

const LOGIN1_DESTINATION: &str = "org.freedesktop.login1";
const LOGIN1_PATH: &str = "/org/freedesktop/login1";
const LOGIN1_INTERFACE: &str = "org.freedesktop.login1.Manager";
const LOGIND_CONTROL_DESTINATION: &str = "lab.Mock.Login1";
const LOGIND_CONTROL_PATH: &str = "/lab/Mock";
const LOGIND_CONTROL_INTERFACE: &str = "lab.Mock.Login1";

fn fd_is_open(fd: RawFd) -> bool {
    Path::new(&format!("/proc/self/fd/{fd}")).exists()
}

/// Число открытых fd, чей `/proc/self/fd`-линк содержит `needle`.
///
/// Счёт по содержимому линка, а не по всему `/proc/self/fd`: тесты одного бинарника идут
/// параллельно и открывают свои сокеты/файлы, общий счётчик был бы недетерминирован.
fn count_fds_matching(needle: &str) -> usize {
    fs::read_dir("/proc/self/fd")
        .map(|entries| {
            entries
                .filter_map(|entry| fs::read_link(entry.ok()?.path()).ok())
                .filter(|target| target.to_string_lossy().contains(needle))
                .count()
        })
        .unwrap_or(0)
}

fn fd_link(fd: RawFd) -> String {
    fs::read_link(format!("/proc/self/fd/{fd}"))
        .map(|target| target.to_string_lossy().into_owned())
        .unwrap_or_default()
}

/// Приватная шина + `mock_logind.py`; всё убирается в `Drop`.
struct LogindBus {
    address: String,
    journal: PathBuf,
    dir: PathBuf,
    daemon_pid: u32,
    mock: Child,
}

impl LogindBus {
    fn start() -> Result<Self, String> {
        if !command_exists("dbus-daemon") {
            return Err(SKIP_NO_DAEMON.to_owned());
        }
        if !python_dbus_available() {
            return Err(SKIP_NO_PYTHON_DBUS.to_owned());
        }

        let dir = std::env::temp_dir().join(format!(
            "discord-lab-w35a-dbus-{}-{}",
            std::process::id(),
            unique_suffix()
        ));
        fs::create_dir_all(&dir).map_err(|err| format!("tempdir {}: {err}", dir.display()))?;

        let started = Command::new("dbus-daemon")
            .args(["--session", "--print-address=1", "--print-pid=1", "--fork"])
            .output()
            .map_err(|err| format!("запуск dbus-daemon: {err}"))?;
        if !started.status.success() {
            let _ = fs::remove_dir_all(&dir);
            return Err(format!(
                "dbus-daemon завершился: {}",
                String::from_utf8_lossy(&started.stderr).trim()
            ));
        }
        let stdout = String::from_utf8_lossy(&started.stdout).into_owned();
        let mut lines = stdout.lines().filter(|line| !line.trim().is_empty());
        let address = lines
            .next()
            .ok_or_else(|| "dbus-daemon не напечатал адрес".to_owned())?
            .trim()
            .to_owned();
        let daemon_pid: u32 = match lines
            .next()
            .ok_or_else(|| "dbus-daemon не напечатал pid".to_owned())
            .and_then(|line| {
                line.trim()
                    .parse()
                    .map_err(|err| format!("pid dbus-daemon: {err}"))
            }) {
            Ok(pid) => pid,
            Err(reason) => {
                let _ = fs::remove_dir_all(&dir);
                return Err(reason);
            }
        };

        let journal = dir.join("journal.tsv");
        let mut mock = Command::new("python3")
            .arg(repo_path("waves/RW-35/power/mock_logind.py"))
            .arg("--address")
            .arg(&address)
            .arg("--log")
            .arg(&journal)
            .stdout(Stdio::null())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|err| abort(daemon_pid, &dir, format!("запуск mock_logind.py: {err}")))?;

        let deadline = Instant::now() + Duration::from_secs(10);
        loop {
            if fs::read_to_string(&journal)
                .map(|text| text.contains("READY"))
                .unwrap_or(false)
            {
                break;
            }
            if let Ok(Some(status)) = mock.try_wait() {
                return Err(abort(
                    daemon_pid,
                    &dir,
                    format!("mock_logind.py завершился ({status})"),
                ));
            }
            if Instant::now() > deadline {
                return Err(abort(
                    daemon_pid,
                    &dir,
                    "mock_logind.py не поднялся за 10 с".to_owned(),
                ));
            }
            std::thread::sleep(Duration::from_millis(50));
        }

        Ok(Self {
            address,
            journal,
            dir,
            daemon_pid,
            mock,
        })
    }

    fn bus_address(&self) -> BusAddress {
        BusAddress::parse(&self.address).expect("адрес приватного демона")
    }

    fn journal_member(&self, member: &str) -> Vec<Vec<String>> {
        fs::read_to_string(&self.journal)
            .unwrap_or_default()
            .lines()
            .map(|line| line.split('\t').map(str::to_owned).collect::<Vec<_>>())
            .filter(|fields| fields.first().map(String::as_str) == Some(member))
            .collect()
    }

    fn wait_for_member(&self, member: &str, count: usize) -> Vec<Vec<String>> {
        let deadline = Instant::now() + Duration::from_secs(5);
        loop {
            let records = self.journal_member(member);
            if records.len() >= count || Instant::now() > deadline {
                return records;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
    }
}

impl Drop for LogindBus {
    fn drop(&mut self) {
        let _ = self.mock.kill();
        let _ = self.mock.wait();
        if self.daemon_pid != 0 {
            let _ = Command::new("kill")
                .arg(self.daemon_pid.to_string())
                .status();
        }
        let _ = fs::remove_dir_all(&self.dir);
    }
}

macro_rules! logind_bus {
    () => {
        match LogindBus::start() {
            Ok(bus) => bus,
            Err(reason) => {
                println!("SKIP: {reason}");
                return;
            }
        }
    };
}

/// Тело `Inhibit(sleep, discord-lab, why, block)`.
fn inhibit_body(why: &str) -> [(char, Value); 4] {
    [
        ('s', Value::String("sleep".to_owned())),
        ('s', Value::String("discord-lab".to_owned())),
        ('s', Value::String(why.to_owned())),
        ('s', Value::String("block".to_owned())),
    ]
}

/// Смещение тела в собранном сообщении (после заголовка, выравнивание 8).
fn body_offset(bytes: &[u8]) -> usize {
    let fields_len = u32::from_le_bytes(bytes[12..16].try_into().expect("fields len")) as usize;
    (16 + fields_len + 7) & !7
}

// --- f01: `h` — индекс в массиве fd, а не «просто u32» ---------------------------

#[test]
fn f01_unix_fd_value_round_trips_through_per_message_index() {
    let (reader, _writer) = std::io::pipe().expect("pipe");
    let raw = reader.as_raw_fd();
    // fd принадлежит тесту; `OwnedFd` забыт, чтобы закрытие через `fd::close_fd`
    // не было двойным (std в debug-сборке ловит такой close и абортит процесс).
    std::mem::forget(reader);

    let (bytes, fds) = message::build_with_fds(
        MessageType::MethodReturn,
        1,
        &[
            (FIELD_SIGNATURE, Value::String("h".to_owned())),
            (FIELD_REPLY_SERIAL, Value::U32(0)),
        ],
        "h",
        &[Value::UnixFd(raw)],
    )
    .expect("сборка MethodReturn с h");
    assert_eq!(fds, vec![raw], "fd сообщения — ровно переданный");
    let offset = body_offset(&bytes);
    assert_eq!(
        u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("index")),
        0,
        "в теле `h` лежит индекс 0, а не значение fd"
    );
    assert_eq!(bytes[offset + 4..].len(), 0, "тело `h` — ровно 4 байта");

    let parsed = message::parse_with_fds(&bytes, fds).expect("разбор h");
    assert_eq!(parsed.signature, "h");
    assert_eq!(parsed.body, vec![Value::UnixFd(raw)], "индекс → fd");

    // Владелец fd после разбора — тест; закрываем ровно один раз.
    fd::close_fd(raw);
}

// --- f04: два fd в одном сообщении -----------------------------------------------

#[test]
#[cfg(target_os = "linux")]
fn f04_two_fds_in_one_reply_are_received_in_order_and_closed() {
    let bus = logind_bus!();
    let mut connection =
        Connection::connect(&bus.bus_address()).unwrap_or_else(|err| panic!("connect: {err:?}"));

    assert_eq!(count_fds_matching("discord-lab-first"), 0);
    assert_eq!(count_fds_matching("discord-lab-second"), 0);
    let (values, fds) = connection
        .call_with_fds(
            LOGIND_CONTROL_DESTINATION,
            LOGIND_CONTROL_PATH,
            LOGIND_CONTROL_INTERFACE,
            "TwoFds",
            &[],
        )
        .unwrap_or_else(|err| panic!("TwoFds: {err:?}"));

    assert_eq!(fds.len(), 2, "оба fd из одного сообщения: {fds:?}");
    assert_eq!(
        values,
        vec![Value::UnixFd(fds[0]), Value::UnixFd(fds[1])],
        "порядок fd — порядок тел `h`"
    );
    for raw in &fds {
        assert!(fd_is_open(*raw), "fd {raw} не открыт");
        let link = fd_link(*raw);
        assert!(
            link.contains("memfd:discord-lab"),
            "fd {raw} — не memfd мока: {link:?}"
        );
    }
    assert_eq!(
        count_fds_matching("discord-lab-first"),
        1,
        "первый fd ещё открыт"
    );
    assert_eq!(
        count_fds_matching("discord-lab-second"),
        1,
        "второй fd ещё открыт"
    );

    for raw in &fds {
        fd::close_fd(*raw);
    }
    assert_eq!(count_fds_matching("discord-lab-first"), 0);
    assert_eq!(count_fds_matching("discord-lab-second"), 0);
    assert_eq!(bus.wait_for_member("TwoFds", 1).len(), 1);
}

// --- f05: индекс вне диапазона ----------------------------------------------------

#[test]
#[cfg(target_os = "linux")]
fn f05_out_of_range_index_is_protocol_and_fds_are_closed() {
    // Ровно один fd в очереди и один в заголовке UNIX_FDS, а индекс в теле — 7:
    // это проверяет именно границу индекса, а не сверку счётчика.
    let prefix = format!("w35a-f05-{}-{}", std::process::id(), unique_suffix());
    let path = std::env::temp_dir().join(format!("{prefix}-0"));
    let file = fs::File::create(&path).expect("temp file");
    let _ = fs::remove_file(&path);
    let raw = file.as_raw_fd();
    std::mem::forget(file);
    assert_eq!(count_fds_matching(&prefix), 1, "подготовлен fd");

    let (mut bytes, _declared) = message::build_with_fds(
        MessageType::MethodReturn,
        2,
        &[
            (FIELD_SIGNATURE, Value::String("h".to_owned())),
            (FIELD_REPLY_SERIAL, Value::U32(0)),
        ],
        "h",
        &[Value::UnixFd(raw)],
    )
    .expect("сборка MethodReturn с h");
    let offset = body_offset(&bytes);
    bytes[offset..offset + 4].copy_from_slice(&7u32.to_le_bytes());
    let queue = vec![raw];

    let result = message::parse_with_fds(&bytes, queue);
    assert!(
        matches!(result, Err(DbusError::Protocol(_))),
        "индекс вне диапазона → Protocol, получено {result:?}"
    );
    assert_eq!(
        count_fds_matching(&prefix),
        0,
        "fd из сообщения с битым индексом обязан быть закрыт"
    );
}

// --- f06: MSG_CTRUNC (ancillary не влез в буфер) ----------------------------------

#[test]
#[cfg(target_os = "linux")]
fn f06_control_truncation_is_protocol_and_closes_received_fds() {
    let (sender, receiver) = std::os::unix::net::UnixStream::pair().expect("socketpair");
    // Уникальные имена файлов: счёт по линкам `/proc/self/fd` не пересекается с другими
    // параллельными тестами процесса.
    let prefix = format!("w35a-f06-{}-{}", std::process::id(), unique_suffix());
    let mut files = Vec::new();
    for index in 0..fd::MAX_FDS_PER_MESSAGE + 4 {
        let path = std::env::temp_dir().join(format!("{prefix}-{index}"));
        files.push(fs::File::create(&path).expect("temp file"));
        let _ = fs::remove_file(&path);
    }
    let raw: Vec<RawFd> = files.iter().map(AsRawFd::as_raw_fd).collect();

    fd::send_with_fds(&sender, b"x", &raw).expect("sendmsg с лишними fd");
    // Копии в ядре уже отправлены; закрываем исходные fd отправителя.
    drop(files);
    assert_eq!(count_fds_matching(&prefix), 0);

    let mut buffer = [0u8; 1];
    let mut received = Vec::new();
    let result = fd::recv_exact_with_fds(&receiver, &mut buffer, &mut received);
    assert!(
        matches!(result, Err(DbusError::Protocol(_))),
        "MSG_CTRUNC → Protocol, получено {result:?}"
    );
    assert!(received.is_empty(), "обрезанные fd не отдаются наружу");
    assert_eq!(
        count_fds_matching(&prefix),
        0,
        "все доставленные fd закрыты, недоехавшие закрыл ядро"
    );
}

// --- f07: `call` закрывает fd из ответа ------------------------------------------

#[test]
#[cfg(target_os = "linux")]
fn f07_plain_call_closes_fds_from_reply_and_reports_protocol() {
    let bus = logind_bus!();
    let mut connection =
        Connection::connect(&bus.bus_address()).unwrap_or_else(|err| panic!("connect: {err:?}"));

    assert_eq!(count_fds_matching("discord-lab-inhibit"), 0);
    let body = inhibit_body("In call");
    let result: Result<Vec<Value>, DbusError> = connection.call(
        LOGIN1_DESTINATION,
        LOGIN1_PATH,
        LOGIN1_INTERFACE,
        "Inhibit",
        &body,
    );
    assert!(
        matches!(result, Err(DbusError::Protocol(_))),
        "fd без владельца в `call` → Protocol, получено {result:?}"
    );
    assert_eq!(
        count_fds_matching("discord-lab-inhibit"),
        0,
        "fd из ответа закрыт, утечки нет"
    );
    assert_eq!(
        bus.wait_for_member("Inhibit", 1).len(),
        1,
        "вызов дошёл до мока"
    );
}

// --- f09: отправка двух fd (SCM_RIGHTS) ------------------------------------------

#[test]
fn f09_call_with_fds_sends_two_pipe_fds_to_service() {
    use std::io::Write as _;

    let bus = logind_bus!();
    let mut connection =
        Connection::connect(&bus.bus_address()).unwrap_or_else(|err| panic!("connect: {err:?}"));

    let (first_reader, mut first_writer) = std::io::pipe().expect("pipe 1");
    let (second_reader, mut second_writer) = std::io::pipe().expect("pipe 2");
    first_writer.write_all(b"alpha").expect("write alpha");
    second_writer.write_all(b"beta").expect("write beta");
    // EOF обязателен: мок читает fd до конца, иначе ответа не будет.
    drop(first_writer);
    drop(second_writer);

    let body = [
        ('h', Value::UnixFd(first_reader.as_raw_fd())),
        ('h', Value::UnixFd(second_reader.as_raw_fd())),
    ];
    let (reply, fds) = connection
        .call_with_fds(
            LOGIND_CONTROL_DESTINATION,
            LOGIND_CONTROL_PATH,
            LOGIND_CONTROL_INTERFACE,
            "AcceptFds",
            &body,
        )
        .unwrap_or_else(|err| panic!("AcceptFds: {err:?}"));
    assert!(fds.is_empty(), "ответ AcceptFds без fd: {fds:?}");
    assert_eq!(reply, vec![Value::String("ok".to_owned())]);

    let records = bus.wait_for_member("AcceptFds", 1);
    let fields = records.first().expect("журнал AcceptFds пуст");
    assert_eq!(
        fields,
        &vec![
            "AcceptFds".to_owned(),
            "alpha".to_owned(),
            "beta".to_owned()
        ],
        "мок прочитал содержимое обоих отправленных fd"
    );
}

// --- f11: дубликат индекса fd (мутационный пробел x2) ------------------------------

#[test]
#[cfg(target_os = "linux")]
fn f11_duplicate_unix_fd_index_is_protocol_and_closes_fds() {
    let prefix = format!("w35a-f11-{}-{}", std::process::id(), unique_suffix());
    let mut raw = Vec::new();
    for index in 0..2 {
        let path = std::env::temp_dir().join(format!("{prefix}-{index}"));
        let file = fs::File::create(&path).expect("temp file");
        let _ = fs::remove_file(&path);
        raw.push(file.as_raw_fd());
        std::mem::forget(file);
    }
    let (mut bytes, declared) = message::build_with_fds(
        MessageType::MethodReturn,
        2,
        &[
            (FIELD_SIGNATURE, Value::String("hh".to_owned())),
            (FIELD_REPLY_SERIAL, Value::U32(0)),
        ],
        "hh",
        &[Value::UnixFd(raw[0]), Value::UnixFd(raw[1])],
    )
    .expect("сборка MethodReturn hh");
    assert_eq!(declared.len(), 2);
    let offset = body_offset(&bytes);
    // Второй индекс подменяем на 0 — повторное использование fd.
    bytes[offset + 4..offset + 8].copy_from_slice(&0u32.to_le_bytes());

    let result = message::parse_with_fds(&bytes, declared);
    assert!(
        matches!(result, Err(DbusError::Protocol(_))),
        "дубликат индекса → Protocol, получено {result:?}"
    );
    assert_eq!(
        count_fds_matching(&prefix),
        0,
        "оба fd обязаны быть закрыты при ошибке разбора"
    );
}

// --- f13: лимит fd на отправку (мутационный пробел x7) ----------------------------

#[test]
fn f13_build_rejects_more_than_max_unix_fds() {
    let files: Vec<fs::File> = (0..=fd::MAX_FDS_PER_MESSAGE)
        .map(|_| fs::File::open("/dev/null").expect("/dev/null"))
        .collect();
    let signature = "h".repeat(files.len());
    let body: Vec<Value> = files
        .iter()
        .map(|file| Value::UnixFd(file.as_raw_fd()))
        .collect();
    let result = message::build_with_fds(
        MessageType::MethodReturn,
        2,
        &[
            (FIELD_SIGNATURE, Value::String(signature.clone())),
            (FIELD_REPLY_SERIAL, Value::U32(0)),
        ],
        &signature,
        &body,
    );
    assert!(
        matches!(result, Err(DbusError::Protocol(_))),
        "больше MAX_FDS_PER_MESSAGE → Protocol, получено {result:?}"
    );
}

// --- f14: received descriptors are closed automatically on exec -------------------

#[test]
#[allow(unsafe_code)] // Independent POSIX fcntl oracle and ownership of received descriptors.
fn f14_received_unix_fds_are_close_on_exec() {
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::os::unix::net::UnixStream;

    let (sender, receiver) = UnixStream::pair().expect("transport socketpair");
    let (first, second) = UnixStream::pair().expect("source socketpair");
    let originals = [first.as_raw_fd(), second.as_raw_fd()];
    let flags = |raw| {
        // SAFETY: every descriptor is owned by a live UnixStream or OwnedFd; F_GETFD
        // only reads its flags and does not take ownership or access memory.
        let result = unsafe { libc::fcntl(raw, libc::F_GETFD) };
        assert!(
            result >= 0,
            "F_GETFD({raw}): {}",
            std::io::Error::last_os_error()
        );
        result
    };
    let original_flags = originals.map(flags);

    fd::send_with_fds(&sender, b"fd", &originals).expect("send two descriptors");
    let mut payload = [0u8; 2];
    let mut received = Vec::new();
    fd::recv_exact_with_fds(&receiver, &mut payload, &mut received)
        .expect("receive two descriptors");
    let owned: Vec<OwnedFd> = received
        .into_iter()
        .map(|raw| {
            // SAFETY: SCM_RIGHTS returns new, uniquely owned descriptors; the receive
            // function transfers them to its caller, and no other owner is created.
            unsafe { OwnedFd::from_raw_fd(raw) }
        })
        .collect();

    assert_eq!(&payload, b"fd");
    assert_eq!(owned.len(), 2);
    for received in &owned {
        assert_ne!(
            flags(received.as_raw_fd()) & libc::FD_CLOEXEC,
            0,
            "every received descriptor must have FD_CLOEXEC before it is returned"
        );
    }
    assert_eq!(
        originals.map(flags),
        original_flags,
        "sender flags are unchanged"
    );
}

#[test]
#[allow(unsafe_code)] // Independent kernel flag oracle and ownership of received SCM_RIGHTS copies.
fn f15_single_receive_returns_short_payload_and_close_on_exec_fds() {
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::os::unix::net::UnixStream;

    let (sender, receiver) = UnixStream::pair().expect("socketpair");
    receiver
        .set_read_timeout(Some(Duration::from_secs(1)))
        .expect("timeout");
    let source = fs::File::open("/dev/null").expect("source descriptor");
    fd::send_with_fds(&sender, b"PW", &[source.as_raw_fd()]).expect("send short payload");
    let mut payload = [0u8; 8];
    let mut received = Vec::new();
    let count = fd::recv_with_fds(&receiver, &mut payload, &mut received).expect("short receive");
    let owned: Vec<OwnedFd> = received
        .into_iter()
        .map(|raw| {
            // SAFETY: the receiver returns new uniquely owned descriptor copies.
            unsafe { OwnedFd::from_raw_fd(raw) }
        })
        .collect();
    assert_eq!(count, 2, "one receive must not wait to fill the buffer");
    assert_eq!(&payload[..count], b"PW");
    assert_eq!(owned.len(), 1);
    // SAFETY: owned retains the live descriptor; F_GETFD only queries its flags.
    let flags = unsafe { libc::fcntl(owned[0].as_raw_fd(), libc::F_GETFD) };
    assert!(flags >= 0);
    assert_ne!(flags & libc::FD_CLOEXEC, 0);
}

#[test]
#[allow(unsafe_code)] // Independent kernel flag oracle and ownership of the returned duplicate.
fn f16_duplicate_cloexec_is_independent_and_preserves_io() {
    use std::io::{Read, Write};
    use std::os::fd::{FromRawFd, OwnedFd};
    use std::os::unix::net::UnixStream;

    let (mut writer, source) = UnixStream::pair().expect("socketpair");
    let raw = fd::duplicate_cloexec(source.as_raw_fd()).expect("duplicate");
    assert_ne!(raw, source.as_raw_fd());
    // SAFETY: duplicate_cloexec transfers the new descriptor's unique ownership.
    let owned = unsafe { OwnedFd::from_raw_fd(raw) };
    // SAFETY: owned retains the live descriptor; F_GETFD only queries its flags.
    let flags = unsafe { libc::fcntl(raw, libc::F_GETFD) };
    assert!(flags >= 0);
    assert_ne!(flags & libc::FD_CLOEXEC, 0);
    drop(source);
    let mut duplicate = UnixStream::from(owned);
    writer
        .write_all(b"ping")
        .expect("write after source is closed");
    let mut data = [0u8; 4];
    duplicate
        .read_exact(&mut data)
        .expect("read through duplicate");
    assert_eq!(&data, b"ping");
    assert!(fd::duplicate_cloexec(-1).is_err());
}

#[test]
#[cfg(target_os = "linux")]
fn f17_single_receive_eof_keeps_owned_fds_but_exact_eof_closes_them() {
    use std::os::fd::IntoRawFd;
    use std::os::unix::net::UnixStream;

    let prefix = format!("chord-f17-{}-{}", std::process::id(), unique_suffix());
    let path = std::env::temp_dir().join(&prefix);
    let file = fs::File::create(&path).expect("unique source");
    fs::remove_file(&path).expect("unlink source");
    let mut received = vec![file.into_raw_fd()];
    let (sender, receiver) = UnixStream::pair().expect("socketpair");
    drop(sender);
    let mut data = [0u8; 1];
    assert_eq!(
        fd::recv_with_fds(&receiver, &mut data, &mut received).expect("EOF"),
        0
    );
    assert_eq!(received.len(), 1);
    assert_eq!(count_fds_matching(&prefix), 1);
    assert!(matches!(
        fd::recv_exact_with_fds(&receiver, &mut data, &mut received),
        Err(DbusError::Protocol("truncated message"))
    ));
    assert!(received.is_empty());
    assert_eq!(
        count_fds_matching(&prefix),
        0,
        "premature EOF must close collected fds"
    );
}

#[test]
#[cfg(target_os = "linux")]
fn f18_single_receive_would_block_closes_collected_fds_and_keeps_os_error() {
    use std::os::fd::IntoRawFd;
    use std::os::unix::net::UnixStream;

    let prefix = format!("chord-f18-{}-{}", std::process::id(), unique_suffix());
    let path = std::env::temp_dir().join(&prefix);
    let file = fs::File::create(&path).expect("unique source");
    fs::remove_file(&path).expect("unlink source");
    let mut received = vec![file.into_raw_fd()];
    let (_sender, receiver) = UnixStream::pair().expect("socketpair");
    receiver
        .set_nonblocking(true)
        .expect("nonblocking receiver");
    let result = fd::recv_with_fds(&receiver, &mut [0u8; 1], &mut received);
    assert!(matches!(result, Err(DbusError::Io(ref error))
        if error.kind() == std::io::ErrorKind::WouldBlock));
    assert!(received.is_empty());
    assert_eq!(
        count_fds_matching(&prefix),
        0,
        "failed receive must close collected fds"
    );
    assert!(matches!(
        fd::recv_exact_with_fds(&receiver, &mut [0u8; 1], &mut received),
        Err(DbusError::Timeout)
    ));
}

#[test]
#[cfg(target_os = "linux")]
fn f19_single_receive_truncation_closes_every_delivered_fd() {
    use std::os::unix::net::UnixStream;

    let (sender, receiver) = UnixStream::pair().expect("socketpair");
    let prefix = format!("chord-f19-{}-{}", std::process::id(), unique_suffix());
    let files: Vec<_> = (0..fd::MAX_FDS_PER_MESSAGE + 4)
        .map(|index| {
            let path = std::env::temp_dir().join(format!("{prefix}-{index}"));
            let file = fs::File::create(&path).expect("unique source");
            fs::remove_file(&path).expect("unlink source");
            file
        })
        .collect();
    let originals: Vec<_> = files.iter().map(AsRawFd::as_raw_fd).collect();
    fd::send_with_fds(&sender, b"x", &originals).expect("send excess descriptors");
    drop(files);
    let mut received = Vec::new();
    let result = fd::recv_with_fds(&receiver, &mut [0u8; 8], &mut received);
    assert!(matches!(
        result,
        Err(DbusError::Protocol("control data truncated"))
    ));
    assert!(received.is_empty());
    assert_eq!(
        count_fds_matching(&prefix),
        0,
        "no truncated descriptor may leak"
    );
}

#[test]
fn f20_empty_receive_does_not_consume_pending_payload() {
    use std::os::unix::net::UnixStream;

    let (sender, receiver) = UnixStream::pair().expect("socketpair");
    fd::send_with_fds(&sender, b"x", &[]).expect("pending payload");
    let mut received = Vec::new();
    assert_eq!(
        fd::recv_with_fds(&receiver, &mut [], &mut received).expect("empty read"),
        0
    );
    let mut data = [0u8; 1];
    assert_eq!(
        fd::recv_with_fds(&receiver, &mut data, &mut received).expect("pending byte"),
        1
    );
    assert_eq!(&data, b"x");
    assert!(received.is_empty());
}

#[test]
#[cfg(target_os = "linux")]
#[allow(unsafe_code)] // Independent SO_PASSCRED kernel fixture; all pointers reference live values.
fn f21_unexpected_ancillary_still_closes_rights_from_later_headers() {
    use std::os::unix::net::UnixStream;

    let (sender, receiver) = UnixStream::pair().expect("socketpair");
    let enabled: libc::c_int = 1;
    // SAFETY: receiver is live, enabled is aligned and readable, and its exact
    // byte size is passed to the checked socket-option syscall.
    let result = unsafe {
        libc::setsockopt(
            receiver.as_raw_fd(),
            libc::SOL_SOCKET,
            libc::SO_PASSCRED,
            (&raw const enabled).cast(),
            std::mem::size_of_val(&enabled) as libc::socklen_t,
        )
    };
    assert_eq!(result, 0, "enable SO_PASSCRED");
    let prefix = format!("chord-f21-{}-{}", std::process::id(), unique_suffix());
    let path = std::env::temp_dir().join(&prefix);
    let file = fs::File::create(&path).expect("unique source");
    fs::remove_file(&path).expect("unlink source");
    fd::send_with_fds(&sender, b"x", &[file.as_raw_fd()]).expect("send rights after credentials");
    drop(file);
    assert_eq!(count_fds_matching(&prefix), 0);
    let mut received = Vec::new();
    let result = fd::recv_with_fds(&receiver, &mut [0u8; 1], &mut received);
    assert!(matches!(
        result,
        Err(DbusError::Protocol("unexpected ancillary data"))
    ));
    assert!(received.is_empty());
    assert_eq!(
        count_fds_matching(&prefix),
        0,
        "rights from later headers must be closed when earlier credentials are rejected"
    );
}
