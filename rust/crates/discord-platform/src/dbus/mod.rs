//! P6-02: минимальный D-Bus-клиент на `std` (без `zbus`/`notify-rust`, без `unsafe`).
//!
//! Оракул — спецификация D-Bus (wire-формат, little-endian, выравнивание) и независимые
//! реализации: libgio (`fixtures/dbus/wire.json`) и libdbus-мок на приватном
//! `dbus-daemon` (`waves/RW-30/dbus/mock_service.py`).
//!
//! Что поддержано: `unix:path`-адреса, AUTH EXTERNAL (hex uid), `Hello`, вызовы методов,
//! ответы/ошибки, `AddMatch` и сигналы, очередь сигналов, пришедших во время `call`.
//!
//! Ограничения (документированы, не заявляются как поддержка):
//! - только little-endian (`l`); big-endian сообщения → `DbusError::Protocol`;
//! - `unix:abstract=` и `tcp:` адреса не поддерживаются (`BusAddress::parse` → `None`);
//! - percent-кодирование в адресе (`%2F`) не декодируется;
//! - типы `x`/`t`/`d`/`h`/`n`/`q` и unix-fd не поддержаны (нет в [`Value`]);
//! - вызовы не ограничены по вложенности, но размеры сообщений ограничены
//!   [`MAX_HEADER_BYTES`]/[`MAX_BODY_BYTES`].

pub mod auth;
pub mod message;

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::os::unix::net::UnixStream;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use message::MessageType;

/// Максимальный размер полей заголовка: защита от мусорной длины из сокета.
pub const MAX_HEADER_BYTES: usize = 1 << 20;
/// Максимальный размер тела сообщения.
pub const MAX_BODY_BYTES: usize = 16 << 20;
/// Таймаут ожидания ответа на метод (сервис может активироваться через шину).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// Адрес шины D-Bus.
///
/// Поддерживается только `unix:path=…` (в т.ч. в списке адресов через `;` — берётся первый
/// поддерживаемый). `unix:abstract=…` и `tcp:` возвращают `None`: абстрактные сокеты в этой
/// карточке не реализуются.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BusAddress {
    path: PathBuf,
}

impl BusAddress {
    /// Разбирает значение `DBUS_SESSION_BUS_ADDRESS`.
    pub fn parse(value: &str) -> Option<Self> {
        for entry in value.split(';') {
            let Some(rest) = entry.strip_prefix("unix:") else {
                continue;
            };
            for field in rest.split(',') {
                let Some(path) = field.strip_prefix("path=") else {
                    continue;
                };
                if path.is_empty() {
                    // `unix:path=` без значения — запись невалидна, пробуем следующий адрес.
                    break;
                }
                return Some(Self {
                    path: PathBuf::from(path),
                });
            }
        }
        None
    }

    /// Адрес из переменной окружения `DBUS_SESSION_BUS_ADDRESS`.
    pub fn from_env() -> Option<Self> {
        std::env::var("DBUS_SESSION_BUS_ADDRESS")
            .ok()
            .and_then(|value| Self::parse(&value))
    }

    /// Путь unix-сокета шины.
    pub fn path(&self) -> &Path {
        &self.path
    }
}

/// Ошибки D-Bus-клиента.
#[derive(Debug)]
pub enum DbusError {
    /// Адрес шины не разобран или недоступен как `unix:path`.
    Address,
    /// Ошибка ввода-вывода сокета.
    Io(std::io::Error),
    /// Отказ аутентификации (`REJECTED`/`ERROR`/нет uid).
    Auth(&'static str),
    /// Нарушение wire-формата или контракта вызова.
    Protocol(&'static str),
    /// `ERROR`-ответ сервера.
    Remote { name: String, message: String },
    /// Истёк таймаут ожидания ответа или сигнала.
    Timeout,
}

/// Значение D-Bus (подмножество типов, нужное уведомлениям).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Byte(u8),
    Bool(bool),
    U32(u32),
    I32(i32),
    String(String),
    Array(Vec<Value>),
    Struct(Vec<Value>),
    Dict(Vec<(Value, Value)>),
    Variant(Box<Value>),
}

/// Входящий сигнал шины.
#[derive(Debug, Clone, PartialEq)]
pub struct Signal {
    pub path: String,
    pub interface: String,
    pub member: String,
    pub body: Vec<Value>,
}

/// Соединение с шиной: AUTH EXTERNAL, `Hello`, вызовы и сигналы.
#[derive(Debug)]
pub struct Connection {
    stream: UnixStream,
    unique_name: String,
    serial: u32,
    pending: VecDeque<Signal>,
}

impl Connection {
    /// Подключается к шине, проходит AUTH EXTERNAL и регистрируется через `Hello`.
    pub fn connect(address: &BusAddress) -> Result<Self, DbusError> {
        let stream = UnixStream::connect(address.path()).map_err(DbusError::Io)?;
        stream
            .set_read_timeout(Some(auth::AUTH_TIMEOUT))
            .map_err(DbusError::Io)?;
        auth::authenticate(&stream)?;

        let mut connection = Self {
            stream,
            unique_name: String::new(),
            serial: 0,
            pending: VecDeque::new(),
        };
        let reply = connection.call_typed(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "Hello",
            "",
            &[],
        )?;
        let name = match reply.into_iter().next() {
            Some(Value::String(name)) if !name.is_empty() => name,
            _ => return Err(DbusError::Protocol("Hello reply without unique name")),
        };
        connection.unique_name = name;
        Ok(connection)
    }

    /// Уникальное имя соединения на шине (`:1.x`).
    pub fn unique_name(&self) -> &str {
        &self.unique_name
    }

    /// Вызов метода.
    ///
    /// `body` — пары «код типа, значение». Код — первый символ типа (`s`, `u`, `i`, `b`,
    /// `y`, `a`, `(`, `v`); полный тип контейнера выводится из значения. Пустой массив или
    /// пустой словарь неоднозначны (D-Bus не хранит тип элемента для пустого массива),
    /// поэтому для них используйте типизированные обёртки из [`crate::notifications`]
    /// (внутри — вызов с явной сигнатурой).
    pub fn call(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        body: &[(char, Value)],
    ) -> Result<Vec<Value>, DbusError> {
        let mut signature = String::new();
        let mut values = Vec::with_capacity(body.len());
        for (code, value) in body {
            let fragment = message::signature_of_value(value)
                .ok_or(DbusError::Protocol("empty container has no element type"))?;
            if !fragment.starts_with(*code) {
                return Err(DbusError::Protocol("type code does not match value"));
            }
            signature.push_str(&fragment);
            values.push(value.clone());
        }
        self.call_typed(destination, path, interface, member, &signature, &values)
    }

    /// Вызов с явной сигнатурой тела — нужен для пустых контейнеров (`a{sv}`, `a(ss)`).
    pub(crate) fn call_typed(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        signature: &str,
        body: &[Value],
    ) -> Result<Vec<Value>, DbusError> {
        let serial = self.next_serial();
        let mut fields = vec![
            (message::FIELD_PATH, Value::String(path.to_owned())),
            (
                message::FIELD_INTERFACE,
                Value::String(interface.to_owned()),
            ),
            (message::FIELD_MEMBER, Value::String(member.to_owned())),
            (
                message::FIELD_DESTINATION,
                Value::String(destination.to_owned()),
            ),
        ];
        if !signature.is_empty() {
            fields.push((
                message::FIELD_SIGNATURE,
                Value::String(signature.to_owned()),
            ));
        }

        let bytes = message::build(MessageType::MethodCall, serial, &fields, signature, body)?;
        self.stream.write_all(&bytes).map_err(DbusError::Io)?;
        self.await_reply(serial)
    }

    /// Подписка на сигналы по правилу `AddMatch`.
    pub fn add_match(&mut self, rule: &str) -> Result<(), DbusError> {
        self.call(
            "org.freedesktop.DBus",
            "/org/freedesktop/DBus",
            "org.freedesktop.DBus",
            "AddMatch",
            &[('s', Value::String(rule.to_owned()))],
        )?;
        Ok(())
    }

    /// Следующий сигнал (уже пришедшие отдаются из очереди).
    pub fn next_signal(&mut self, timeout: Duration) -> Result<Signal, DbusError> {
        loop {
            if let Some(signal) = self.pending.pop_front() {
                return Ok(signal);
            }
            let message = self.read_message(timeout)?;
            match message.kind {
                MessageType::Signal => return Ok(signal_from(message)),
                // Ответы на уже завершённые вызовы и чужие method call'ы игнорируются:
                // сервер не должен присылать их в этот момент, но падать из-за них нельзя.
                MessageType::MethodReturn | MessageType::Error | MessageType::MethodCall => {
                    continue;
                }
            }
        }
    }

    fn await_reply(&mut self, serial: u32) -> Result<Vec<Value>, DbusError> {
        loop {
            let message = self.read_message(CALL_TIMEOUT)?;
            match message.kind {
                MessageType::Signal => self.pending.push_back(signal_from(message)),
                MessageType::MethodCall => continue,
                MessageType::MethodReturn => {
                    if message.reply_serial() == Some(serial) {
                        return Ok(message.body);
                    }
                }
                MessageType::Error => {
                    if message.reply_serial() == Some(serial) {
                        let name = message.error_name().unwrap_or("<unknown error>").to_owned();
                        let text = match message.body.first() {
                            Some(Value::String(text)) => text.clone(),
                            _ => String::new(),
                        };
                        return Err(DbusError::Remote {
                            name,
                            message: text,
                        });
                    }
                }
            }
        }
    }

    fn read_message(&mut self, timeout: Duration) -> Result<message::Message, DbusError> {
        self.stream
            .set_read_timeout(Some(timeout))
            .map_err(DbusError::Io)?;

        let mut header = [0u8; 16];
        read_exact(&mut self.stream, &mut header)?;
        if header[0] != b'l' {
            return Err(DbusError::Protocol("only little-endian is supported"));
        }
        let body_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let fields_len =
            u32::from_le_bytes([header[12], header[13], header[14], header[15]]) as usize;
        if fields_len > MAX_HEADER_BYTES {
            return Err(DbusError::Protocol("header too large"));
        }
        if body_len > MAX_BODY_BYTES {
            return Err(DbusError::Protocol("body too large"));
        }

        let fields_end = 16 + fields_len;
        let body_offset = (fields_end + 7) & !7;
        let mut bytes = vec![0u8; body_offset + body_len];
        bytes[..16].copy_from_slice(&header);
        read_exact(&mut self.stream, &mut bytes[16..])?;
        message::parse(&bytes)
    }

    fn next_serial(&mut self) -> u32 {
        self.serial = self.serial.wrapping_add(1);
        if self.serial == 0 {
            self.serial = 1;
        }
        self.serial
    }
}

fn signal_from(message: message::Message) -> Signal {
    Signal {
        path: message.path().unwrap_or("").to_owned(),
        interface: message.interface().unwrap_or("").to_owned(),
        member: message.member().unwrap_or("").to_owned(),
        body: message.body,
    }
}

fn read_exact(stream: &mut UnixStream, buffer: &mut [u8]) -> Result<(), DbusError> {
    match stream.read_exact(buffer) {
        Ok(()) => Ok(()),
        // Обрезанный пакет (пир закрыл сокет посреди сообщения) — нарушение протокола.
        Err(error) if error.kind() == ErrorKind::UnexpectedEof => {
            Err(DbusError::Protocol("truncated message"))
        }
        Err(error)
            if error.kind() == ErrorKind::WouldBlock || error.kind() == ErrorKind::TimedOut =>
        {
            Err(DbusError::Timeout)
        }
        Err(error) => Err(DbusError::Io(error)),
    }
}
