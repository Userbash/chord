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
pub mod fd;
pub mod message;

use std::collections::VecDeque;
use std::io::{ErrorKind, Read, Write};
use std::os::fd::RawFd;
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

/// Максимальная длина имени abstract-сокета Linux (`sun_path` минус ведущий NUL).
pub const MAX_ABSTRACT_NAME_BYTES: usize = 107;

/// Предел длины строки auth-ответа шины (защита от мусора в сокете).
const MAX_AUTH_LINE: usize = 1024;

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

/// Соединение с шиной: AUTH EXTERNAL, `Hello`, вызовы и сигналы.
#[derive(Debug)]
pub struct Connection {
    stream: UnixStream,
    unique_name: String,
    serial: u32,
    pending: VecDeque<Signal>,
    /// Шина согласовала `NEGOTIATE_UNIX_FD` (иначе `SCM_RIGHTS` не пересылается).
    unix_fd: bool,
}

impl Connection {
    /// Подключается к шине, проходит AUTH EXTERNAL, согласует передачу unix-fd и
    /// регистрируется через `Hello`.
    ///
    /// `Path` — обычный `UnixStream::connect`; `Abstract` — `SocketAddr::from_abstract_name`
    /// + `UnixStream::connect_addr` (stable `std`, без `unsafe`).
    pub fn connect(address: &BusAddress) -> Result<Self, DbusError> {
        let stream = open_stream(address)?;
        stream
            .set_read_timeout(Some(auth::AUTH_TIMEOUT))
            .map_err(DbusError::Io)?;
        let unix_fd = authenticate_with_fds(&stream)?;

        let mut connection = Self {
            stream,
            unique_name: String::new(),
            serial: 0,
            pending: VecDeque::new(),
            unix_fd,
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
    /// `y`, `h`, `a`, `(`, `v`); полный тип контейнера выводится из значения. Пустой массив
    /// или пустой словарь неоднозначны (D-Bus не хранит тип элемента для пустого массива),
    /// поэтому для них используйте типизированные обёртки из [`crate::notifications`]
    /// (внутри — вызов с явной сигнатурой).
    ///
    /// fd из ответа этот метод не отдаёт: они закрываются, а вызов возвращает
    /// `DbusError::Protocol` — [`Value::UnixFd`] без владельца было бы утечкой/двойным
    /// закрытием. Для `h` в ответе используйте [`Self::call_with_fds`].
    pub fn call(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        body: &[(char, Value)],
    ) -> Result<Vec<Value>, DbusError> {
        let (signature, values) = body_signature(body)?;
        self.call_typed(destination, path, interface, member, &signature, &values)
    }

    /// Вызов метода с приёмом/передачей unix-fd (`h`, `SCM_RIGHTS`).
    ///
    /// `body` может содержать `('h', Value::UnixFd(fd))`: fd уходят на сервер вместе с
    /// сообщением (владение остаётся у вызывающего). Возвращённые fd — в теле
    /// ([`Value::UnixFd`]) и в векторе (тот же порядок и те же номера: вектор — удобный
    /// список, а не дополнительное владение). Закрывайте каждый fd ровно один раз
    /// ([`fd::close_fd`]); при ошибке вызова полученные fd уже закрыты.
    pub fn call_with_fds(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        body: &[(char, Value)],
    ) -> Result<(Vec<Value>, Vec<RawFd>), DbusError> {
        let (signature, values) = body_signature(body)?;
        self.call_typed_with_fds(destination, path, interface, member, &signature, &values)
    }

    /// Вызов с явной сигнатурой тела — нужен для пустых контейнеров (`a{sv}`, `a(ss)`).
    ///
    /// Ответ с fd — `Protocol` после закрытия этих fd (см. [`Self::call`]).
    pub(crate) fn call_typed(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        signature: &str,
        body: &[Value],
    ) -> Result<Vec<Value>, DbusError> {
        let (values, fds) =
            self.call_typed_with_fds(destination, path, interface, member, signature, body)?;
        if fds.is_empty() {
            return Ok(values);
        }
        fd::close_all(&fds);
        Err(DbusError::Protocol(
            "reply contains unix fds; use call_with_fds",
        ))
    }

    fn call_typed_with_fds(
        &mut self,
        destination: &str,
        path: &str,
        interface: &str,
        member: &str,
        signature: &str,
        body: &[Value],
    ) -> Result<(Vec<Value>, Vec<RawFd>), DbusError> {
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

        let (bytes, fds) =
            message::build_with_fds(MessageType::MethodCall, serial, &fields, signature, body)?;
        if !fds.is_empty() && !self.unix_fd {
            // Шина не согласовала SCM_RIGHTS: fd потерялись бы молча. Владение остаётся
            // у вызывающего (значения `Value::UnixFd`), поэтому не закрываем.
            return Err(DbusError::Protocol(
                "unix fd passing is not negotiated with the bus",
            ));
        }
        fd::send_with_fds(&self.stream, &bytes, &fds)?;
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
                // Возможные fd в их теле закрываются — иначе утечка.
                MessageType::MethodReturn | MessageType::Error | MessageType::MethodCall => {
                    message::close_fds_in_body(&message.body);
                    continue;
                }
            }
        }
    }

    fn await_reply(&mut self, serial: u32) -> Result<(Vec<Value>, Vec<RawFd>), DbusError> {
        loop {
            let message = self.read_message(CALL_TIMEOUT)?;
            match message.kind {
                MessageType::Signal => self.pending.push_back(signal_from(message)),
                MessageType::MethodCall => message::close_fds_in_body(&message.body),
                MessageType::MethodReturn => {
                    if message.reply_serial() == Some(serial) {
                        let fds = message::fds_in_body(&message.body);
                        return Ok((message.body, fds));
                    }
                    // Ответ на чужой/завершённый вызов: fd из него закрываем.
                    message::close_fds_in_body(&message.body);
                }
                MessageType::Error => {
                    if message.reply_serial() == Some(serial) {
                        let name = message.error_name().unwrap_or("<unknown error>").to_owned();
                        let text = match message.body.first() {
                            Some(Value::String(text)) => text.clone(),
                            _ => String::new(),
                        };
                        message::close_fds_in_body(&message.body);
                        return Err(DbusError::Remote {
                            name,
                            message: text,
                        });
                    }
                    message::close_fds_in_body(&message.body);
                }
            }
        }
    }

    fn read_message(&mut self, timeout: Duration) -> Result<message::Message, DbusError> {
        self.stream
            .set_read_timeout(Some(timeout))
            .map_err(DbusError::Io)?;

        let mut fds: Vec<RawFd> = Vec::new();
        let mut header = [0u8; 16];
        fd::recv_exact_with_fds(&self.stream, &mut header, &mut fds)?;
        if header[0] != b'l' {
            fd::close_all(&fds);
            return Err(DbusError::Protocol("only little-endian is supported"));
        }
        let body_len = u32::from_le_bytes([header[4], header[5], header[6], header[7]]) as usize;
        let fields_len =
            u32::from_le_bytes([header[12], header[13], header[14], header[15]]) as usize;
        if fields_len > MAX_HEADER_BYTES {
            fd::close_all(&fds);
            return Err(DbusError::Protocol("header too large"));
        }
        if body_len > MAX_BODY_BYTES {
            fd::close_all(&fds);
            return Err(DbusError::Protocol("body too large"));
        }

        let fields_end = 16 + fields_len;
        let body_offset = (fields_end + 7) & !7;
        let mut bytes = vec![0u8; body_offset + body_len];
        bytes[..16].copy_from_slice(&header);
        fd::recv_exact_with_fds(&self.stream, &mut bytes[16..], &mut fds)?;
        // Владелец fd теперь `parse_with_fds`: при ошибке он их закроет.
        message::parse_with_fds(&bytes, fds)
    }

    fn next_serial(&mut self) -> u32 {
        self.serial = self.serial.wrapping_add(1);
        if self.serial == 0 {
            self.serial = 1;
        }
        self.serial
    }
}

/// AUTH EXTERNAL + `NEGOTIATE_UNIX_FD` + `BEGIN`.
///
/// [`auth::authenticate`] отправляет `BEGIN` сразу после `OK` и потому не оставляет места
/// для согласования fd. Повторяем ту же последовательность здесь (файл `auth.rs` — чужая
/// зона в матрице W35a), добавляя шаг `NEGOTIATE_UNIX_FD`: без `AGREE_UNIX_FD` шина не
/// пересылает `SCM_RIGHTS` (спека D-Bus, раздел «Authentication»).
///
/// Возвращает `true`, если шина согласилась принимать/передавать unix-fd. Ответ `ERROR`
/// на `NEGOTIATE_UNIX_FD` не фатален: соединение работает без fd.
fn authenticate_with_fds(stream: &UnixStream) -> Result<bool, DbusError> {
    let uid = auth::current_uid()?;
    let mut request = Vec::with_capacity(32);
    request.push(0u8); // credentials byte: без него шина не читает команды
    request.extend_from_slice(auth::external_auth_line(uid).as_bytes());
    request.extend_from_slice(b"\r\n");

    let mut writer = stream;
    writer.write_all(&request).map_err(DbusError::Io)?;

    let reply = read_auth_line(stream)?;
    if reply.starts_with("REJECTED") {
        return Err(DbusError::Auth("EXTERNAL auth rejected"));
    }
    if reply.starts_with("ERROR") {
        return Err(DbusError::Auth("auth error"));
    }
    if !reply.starts_with("OK ") {
        return Err(DbusError::Protocol("unexpected auth reply"));
    }

    writer
        .write_all(b"NEGOTIATE_UNIX_FD\r\n")
        .map_err(DbusError::Io)?;
    let unix_fd = match read_auth_line(stream)?.as_str() {
        "AGREE_UNIX_FD" => true,
        reply if reply.starts_with("ERROR") => false,
        _ => return Err(DbusError::Protocol("unexpected NEGOTIATE_UNIX_FD reply")),
    };

    writer.write_all(b"BEGIN\r\n").map_err(DbusError::Io)?;
    Ok(unix_fd)
}

/// Строка auth-протокола до `\n` (с отбросом `\r`), не длиннее [`MAX_AUTH_LINE`].
fn read_auth_line(stream: &UnixStream) -> Result<String, DbusError> {
    let mut reader = stream;
    let mut bytes = Vec::with_capacity(64);
    let mut byte = [0u8; 1];
    loop {
        match reader.read(&mut byte) {
            Ok(0) => return Err(DbusError::Protocol("auth connection closed")),
            Ok(_) => {
                if byte[0] == b'\n' {
                    break;
                }
                if bytes.len() >= MAX_AUTH_LINE {
                    return Err(DbusError::Protocol("auth reply too long"));
                }
                bytes.push(byte[0]);
            }
            Err(error)
                if error.kind() == ErrorKind::WouldBlock || error.kind() == ErrorKind::TimedOut =>
            {
                return Err(DbusError::Timeout);
            }
            Err(error) => return Err(DbusError::Io(error)),
        }
    }
    if bytes.last() == Some(&b'\r') {
        bytes.pop();
    }
    String::from_utf8(bytes).map_err(|_| DbusError::Protocol("auth reply is not utf-8"))
}

/// Открывает unix-сокет шины по адресу.
fn open_stream(address: &BusAddress) -> Result<UnixStream, DbusError> {
    match address {
        BusAddress::Path(path) => UnixStream::connect(path).map_err(DbusError::Io),
        BusAddress::Abstract(name) => {
            #[cfg(target_os = "linux")]
            {
                use std::os::linux::net::SocketAddrExt;
                let target = std::os::unix::net::SocketAddr::from_abstract_name(name)
                    .map_err(|_| DbusError::Address)?;
                UnixStream::connect_addr(&target).map_err(DbusError::Io)
            }
            #[cfg(not(target_os = "linux"))]
            {
                // Abstract-сокеты — свойство Linux; на других платформах честный отказ.
                let _ = name;
                Err(DbusError::Address)
            }
        }
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

/// Сигнатура и значения тела из пар «код, значение» для [`Connection::call`].
fn body_signature(body: &[(char, Value)]) -> Result<(String, Vec<Value>), DbusError> {
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
    Ok((signature, values))
}

impl Drop for Connection {
    fn drop(&mut self) {
        // Непрочитанные сигналы могли принести fd: закрываем, иначе утечка.
        for signal in self.pending.drain(..) {
            message::close_fds_in_body(&signal.body);
        }
    }
}
