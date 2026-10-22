//! Контракт P6-02 (unit): D-Bus wire-формат — адрес шины, auth EXTERNAL, маршалинг,
//! разбор заголовка и тела, устойчивость к битым пакетам.
//!
//! Независимый оракул — `fixtures/dbus/wire.json`, сгенерированный `gen-golden.py`
//! через **libgio** (`Gio.DBusMessage.to_blob()`), а не Rust-кодом W30b: байты
//! сравниваются побайтово, порядок заголовочных полей берётся из фикстуры.
//! `fixtures/dbus/notifications.json` фиксирует сигнатуры настоящего libdbus-сервиса
//! (`mock_service.py` на приватном `dbus-daemon`).

use discord_platform::dbus::message::{self, FIELD_REPLY_SERIAL, FIELD_SIGNATURE, MessageType};
use discord_platform::dbus::{BusAddress, DbusError, Value, auth};
use discord_platform::notifications;
use std::path::{Path, PathBuf};
use std::process::Command;

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

// --- n01: адрес шины -------------------------------------------------------------

#[test]
fn n01_bus_address_parse_accepts_path_and_rejects_abstract() {
    let plain = BusAddress::parse("unix:path=/run/user/1000/bus").expect("unix:path");
    assert_eq!(plain.path(), Path::new("/run/user/1000/bus"));

    // abstract-сокеты не поддерживаются и документированы как None.
    assert_eq!(BusAddress::parse("unix:abstract=/tmp/dbus-abc"), None);
    assert_eq!(BusAddress::parse("tcp:host=localhost,port=1234"), None);
    assert_eq!(BusAddress::parse(""), None);
    assert_eq!(BusAddress::parse("unix:path="), None);
    assert_eq!(BusAddress::parse("мусор"), None);

    // Список адресов: берётся первый поддерживаемый.
    let list = BusAddress::parse("unix:abstract=/tmp/abstract;unix:path=/tmp/real");
    assert_eq!(
        list.map(|address| address.path().to_path_buf()),
        Some(PathBuf::from("/tmp/real"))
    );

    // guid и прочие ключи в той же записи не мешают.
    let with_guid = BusAddress::parse("unix:guid=abc,path=/tmp/a").expect("path после guid");
    assert_eq!(with_guid.path(), Path::new("/tmp/a"));

    // `from_env` — ровно разбор DBUS_SESSION_BUS_ADDRESS (если переменная задана).
    match std::env::var("DBUS_SESSION_BUS_ADDRESS") {
        Ok(value) => {
            let from_env = BusAddress::from_env().expect("адрес из env");
            let parsed = BusAddress::parse(&value).expect("разбор env-адреса");
            assert_eq!(from_env.path(), parsed.path());
        }
        Err(_) => println!("SKIP: DBUS_SESSION_BUS_ADDRESS не задан, from_env не проверяется"),
    }
}

// --- n02: uid для AUTH EXTERNAL --------------------------------------------------

#[test]
fn n02b_external_auth_line_uses_hex_uid_from_proc_status() {
    let output = Command::new("id").arg("-u").output().expect("id -u");
    assert!(output.status.success(), "id -u завершился неуспешно");
    let expected: u32 = String::from_utf8_lossy(&output.stdout)
        .trim()
        .parse()
        .expect("uid из id -u");

    assert_eq!(
        auth::current_uid().expect("uid из /proc/self/status"),
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
        Some("(ssss)")
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
