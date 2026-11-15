//! P6-02: минимальный D-Bus-клиент на `std` (без `zbus`/`notify-rust`).
//!
//! Оракул — спецификация D-Bus (wire-формат, little-endian, выравнивание) и независимые
//! реализации: libgio (`fixtures/dbus/wire.json`) и libdbus-моки на приватном
//! `dbus-daemon` (`waves/RW-30/dbus/mock_service.py`, `waves/RW-35/power/mock_logind.py`).
//!
//! Что поддержано: `unix:path`- и `unix:abstract`-адреса (Linux), AUTH EXTERNAL (hex uid),
//! `Hello`, вызовы методов, ответы/ошибки, `AddMatch` и сигналы, очередь сигналов,
//! пришедших во время `call`, передача unix-fd (`h`/`SCM_RIGHTS`, см. [`fd`]).
//!
//! Ограничения (документированы, не заявляются как поддержка):
//! - только little-endian (`l`); big-endian сообщения → `DbusError::Protocol`;
//! - `tcp:` адреса не поддерживаются (`BusAddress::parse` → `None`);
//! - `unix:abstract=` — только Linux (имя сокета, до [`MAX_ABSTRACT_NAME_BYTES`] байт);
//!   на других платформах разбор даёт `Abstract`, а `Connection::connect` — `DbusError::Address`;
//! - percent-кодирование в адресе (`%2F`) не декодируется;
//! - типы `x`/`t`/`d`/`n`/`q` не поддержаны (нет в [`Value`]); unix-fd — только через
//!   [`Connection::call_with_fds`] (в обычном [`Connection::call`] fd из ответа закрываются,
//!   а вызов возвращает `Protocol`: владельца у значения нет);
//! - вызовы не ограничены по вложенности, но размеры сообщений ограничены
//!   [`MAX_HEADER_BYTES`]/[`MAX_BODY_BYTES`].

pub mod auth;
#[cfg(unix)]
mod connection;
#[cfg(unix)]
pub mod fd;
#[cfg(unix)]
pub use connection::Connection;
pub mod message;

#[cfg(unix)]
use std::os::fd::RawFd;
use std::path::{Path, PathBuf};
use std::time::Duration;

pub use message::MessageType;

/// Максимальный размер полей заголовка: защита от мусорной длины из сокета.
pub const MAX_HEADER_BYTES: usize = 1 << 20;
/// Максимальный размер тела сообщения.
pub const MAX_BODY_BYTES: usize = 16 << 20;
/// Таймаут ожидания ответа на метод (сервис может активироваться через шину).
pub const CALL_TIMEOUT: Duration = Duration::from_secs(10);

/// Максимальная длина имени abstract-сокета Linux (`sun_path` минус ведущий NUL).
pub const MAX_ABSTRACT_NAME_BYTES: usize = 107;

/// Адрес шины D-Bus.
///
/// Поддерживаются `unix:path=…` и `unix:abstract=…` (в т.ч. в списке адресов через `;` —
/// берётся первый поддерживаемый). `tcp:` и прочие схемы возвращают `None`: их в этой
/// карточке нет. Percent-кодирование (`%2F`) не декодируется — байты имени берутся как есть.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BusAddress {
    /// `unix:path=/run/user/1000/bus` — путь файлового сокета.
    Path(PathBuf),
    /// `unix:abstract=name` — имя в abstract-пространстве Linux (сырые байты).
    Abstract(Vec<u8>),
}

impl BusAddress {
    /// Разбирает значение `DBUS_SESSION_BUS_ADDRESS`.
    ///
    /// Записи списка перебираются по порядку; запись с неподдерживаемой схемой, пустым
    /// значением, NUL внутри значения или слишком длинным abstract-именем пропускается.
    pub fn parse(value: &str) -> Option<Self> {
        for entry in value.split(';') {
            let Some(rest) = entry.strip_prefix("unix:") else {
                continue;
            };
            for field in rest.split(',') {
                if let Some(path) = field.strip_prefix("path=") {
                    if let Some(address) = Self::from_path(path) {
                        return Some(address);
                    }
                    // `unix:path=` без значения — запись невалидна, пробуем следующий адрес.
                    break;
                }
                if let Some(name) = field.strip_prefix("abstract=") {
                    if let Some(address) = Self::from_abstract_name(name) {
                        return Some(address);
                    }
                    break;
                }
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

    /// Путь unix-сокета шины; `None` для abstract-адреса.
    pub fn path(&self) -> Option<&Path> {
        match self {
            Self::Path(path) => Some(path),
            Self::Abstract(_) => None,
        }
    }

    /// Имя abstract-сокета (сырые байты); `None` для path-адреса.
    pub fn abstract_name(&self) -> Option<&[u8]> {
        match self {
            Self::Abstract(name) => Some(name),
            Self::Path(_) => None,
        }
    }

    fn from_path(value: &str) -> Option<Self> {
        if value.is_empty() || value.contains('\0') {
            return None;
        }
        Some(Self::Path(PathBuf::from(value)))
    }

    fn from_abstract_name(value: &str) -> Option<Self> {
        let bytes = value.as_bytes();
        if bytes.is_empty() || bytes.len() > MAX_ABSTRACT_NAME_BYTES || bytes.contains(&0) {
            return None;
        }
        Some(Self::Abstract(bytes.to_vec()))
    }
}

/// Ошибки D-Bus-клиента.
#[derive(Debug)]
pub enum DbusError {
    /// Адрес шины не разобран, не поддержан платформой или недоступен как unix-сокет.
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

/// Значение D-Bus (подмножество типов, нужное уведомлениям и питания).
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Byte(u8),
    Bool(bool),
    U32(u32),
    I32(i32),
    String(String),
    /// `h`: владелец — вызывающий (после [`Connection::call_with_fds`]).
    #[cfg(unix)]
    UnixFd(RawFd),
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

#[cfg(unix)]
impl Signal {
    /// Закрывает fd, пришедшие в теле сигнала (вызывать при отбрасывании сигнала).
    pub fn close_fds(&mut self) {
        for value in &mut self.body {
            if let Value::UnixFd(raw) = value {
                fd::close_fd(*raw);
                *raw = -1;
            }
        }
    }
}
