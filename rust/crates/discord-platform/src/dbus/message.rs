//! Wire-формат D-Bus: заголовок `yyyyuua(yv)` и тело по сигнатуре.
//!
//! Реализовано строго по спецификации D-Bus: little-endian, выравнивание относительно
//! начала сообщения (`u`/`i`/`h`/`s`/`a` — 4, `x`/`t`/`d`/struct — 8, `y`/`b`/`g`/`v` — 1),
//! длина массива не включает padding после последнего элемента, строка — `u32` длина +
//! байты + NUL.
//!
//! Оракул байтов — libgio (`fixtures/dbus/wire.json`): `build(parse(bytes))` обязан дать
//! те же байты, что и `Gio.DBusMessage.to_blob()`.
//!
//! `h` (`Value::UnixFd`) в теле — `u32`-индекс в массив fd, приложенный к сообщению
//! ancillary-данными (`SCM_RIGHTS`, см. [`super::fd`]); [`parse_with_fds`] заменяет
//! индекс на реальный fd, [`build_with_fds`] собирает обратный список fd.

use std::os::fd::RawFd;

use super::{DbusError, MAX_BODY_BYTES, MAX_HEADER_BYTES, Value, fd};

/// Поле заголовка `PATH`.
pub const FIELD_PATH: u8 = 1;
/// Поле заголовка `INTERFACE`.
pub const FIELD_INTERFACE: u8 = 2;
/// Поле заголовка `MEMBER`.
pub const FIELD_MEMBER: u8 = 3;
/// Поле заголовка `ERROR_NAME`.
pub const FIELD_ERROR_NAME: u8 = 4;
/// Поле заголовка `REPLY_SERIAL`.
pub const FIELD_REPLY_SERIAL: u8 = 5;
/// Поле заголовка `DESTINATION`.
pub const FIELD_DESTINATION: u8 = 6;
/// Поле заголовка `SENDER`.
pub const FIELD_SENDER: u8 = 7;
/// Поле заголовка `SIGNATURE`.
pub const FIELD_SIGNATURE: u8 = 8;
/// Поле заголовка `UNIX_FDS`: число fd, приложенных к сообщению (`u32`).
pub const FIELD_UNIX_FDS: u8 = 9;

/// Тип сообщения D-Bus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MessageType {
    MethodCall,
    MethodReturn,
    Error,
    Signal,
}

impl MessageType {
    /// Код типа в заголовке.
    pub fn code(self) -> u8 {
        match self {
            Self::MethodCall => 1,
            Self::MethodReturn => 2,
            Self::Error => 3,
            Self::Signal => 4,
        }
    }

    /// Разбор кода типа; неизвестный код → `None`.
    pub fn from_code(code: u8) -> Option<Self> {
        match code {
            1 => Some(Self::MethodCall),
            2 => Some(Self::MethodReturn),
            3 => Some(Self::Error),
            4 => Some(Self::Signal),
            _ => None,
        }
    }
}

/// Разобранное сообщение.
#[derive(Debug, Clone, PartialEq)]
pub struct Message {
    pub kind: MessageType,
    pub serial: u32,
    pub fields: Vec<(u8, Value)>,
    /// Сигнатура тела (поле `SIGNATURE`); пустая строка — тело пустое.
    pub signature: String,
    pub body: Vec<Value>,
}

impl Message {
    /// Значение заголовочного поля по коду.
    pub fn field(&self, code: u8) -> Option<&Value> {
        self.fields
            .iter()
            .find(|(field_code, _)| *field_code == code)
            .map(|(_, value)| value)
    }

    pub fn path(&self) -> Option<&str> {
        self.string_field(FIELD_PATH)
    }

    pub fn interface(&self) -> Option<&str> {
        self.string_field(FIELD_INTERFACE)
    }

    pub fn member(&self) -> Option<&str> {
        self.string_field(FIELD_MEMBER)
    }

    pub fn destination(&self) -> Option<&str> {
        self.string_field(FIELD_DESTINATION)
    }

    pub fn error_name(&self) -> Option<&str> {
        self.string_field(FIELD_ERROR_NAME)
    }

    pub fn reply_serial(&self) -> Option<u32> {
        match self.field(FIELD_REPLY_SERIAL)? {
            Value::U32(serial) => Some(*serial),
            _ => None,
        }
    }

    fn string_field(&self, code: u8) -> Option<&str> {
        match self.field(code)? {
            Value::String(text) => Some(text),
            _ => None,
        }
    }
}

/// Собирает сообщение: заголовок (поля в переданном порядке) и тело по сигнатуре.
///
/// Вариант без fd: тело с `h` отвергается — иначе fd потерялись бы молча.
/// Для `h` используйте [`build_with_fds`].
pub fn build(
    kind: MessageType,
    serial: u32,
    fields: &[(u8, Value)],
    signature: &str,
    body: &[Value],
) -> Result<Vec<u8>, DbusError> {
    let (bytes, fds) = build_with_fds(kind, serial, fields, signature, body)?;
    if !fds.is_empty() {
        return Err(DbusError::Protocol(
            "body contains unix fds; use build_with_fds",
        ));
    }
    Ok(bytes)
}

/// Собирает сообщение и список fd для `SCM_RIGHTS`.
///
/// Каждое значение `h` (`Value::UnixFd`) получает индекс в порядке появления в теле
/// (0, 1, …); возвращённый вектор fd передаётся в [`super::fd::send_with_fds`].
/// Не больше [`fd::MAX_FDS_PER_MESSAGE`] fd на сообщение.
pub fn build_with_fds(
    kind: MessageType,
    serial: u32,
    fields: &[(u8, Value)],
    signature: &str,
    body: &[Value],
) -> Result<(Vec<u8>, Vec<RawFd>), DbusError> {
    if serial == 0 {
        return Err(DbusError::Protocol("serial must be non-zero"));
    }

    let mut writer = Writer {
        buffer: vec![0u8; 16],
        fds: Vec::new(),
    };
    writer.buffer[0] = b'l';
    writer.buffer[1] = kind.code();
    // Флаг 0x1 (NO_REPLY_EXPECTED) у всех сообщений, кроме method call — как у libgio:
    // ответа не ждут ни return, ни error, ни signal.
    writer.buffer[2] = if kind == MessageType::MethodCall {
        0
    } else {
        1
    };
    writer.buffer[3] = 1; // protocol version
    writer.buffer[8..12].copy_from_slice(&serial.to_le_bytes());
    // Длина массива полей заголовка (`a(yv)`) лежит в 12..16; данные начинаются с 16.
    for (code, value) in fields {
        let field_type =
            field_type(*code).ok_or(DbusError::Protocol("unsupported header field code"))?;
        writer.align(8)?;
        writer.buffer.push(*code);
        writer.signature(field_type);
        writer.value(field_type, value)?;
    }

    // D-Bus требует поле UNIX_FDS, если к сообщению приложены fd (и запрещает его без fd).
    let fd_count = fds_in_body(body).len();
    if fd_count > 0 {
        let declared = u32::try_from(fd_count)
            .map_err(|_| DbusError::Protocol("too many unix fds in message"))?;
        writer.align(8)?;
        writer.buffer.push(FIELD_UNIX_FDS);
        writer.signature("u");
        writer.value("u", &Value::U32(declared))?;
    }

    let fields_len = u32::try_from(writer.buffer.len() - 16)
        .map_err(|_| DbusError::Protocol("header too large"))?;
    writer.buffer[12..16].copy_from_slice(&fields_len.to_le_bytes());
    if writer.buffer.len() > MAX_HEADER_BYTES {
        return Err(DbusError::Protocol("header too large"));
    }

    writer.align(8)?;
    let body_offset = writer.buffer.len();
    write_body(&mut writer, signature, body)?;
    let body_len = u32::try_from(writer.buffer.len() - body_offset)
        .map_err(|_| DbusError::Protocol("body too large"))?;
    if body_len as usize > MAX_BODY_BYTES {
        return Err(DbusError::Protocol("body too large"));
    }
    writer.buffer[4..8].copy_from_slice(&body_len.to_le_bytes());
    Ok((writer.buffer, writer.fds))
}

/// Разбирает сообщение без fd: любое `h` → `Protocol` (индекс некуда разрешить).
pub fn parse(bytes: &[u8]) -> Result<Message, DbusError> {
    parse_with_fds(bytes, Vec::new())
}

/// Разбирает сообщение, разрешая `h` в реальные fd из `fds`.
///
/// Владелец `fds` после вызова — этот разбор:
/// - ошибка → все fd закрываются (дескрипторы не текут);
/// - успех → fd, на которые не ссылается ни один `h`, закрываются, остальные живут в
///   [`Value::UnixFd`] и принадлежат вызывающему;
/// - повторный `h` на тот же индекс → `Protocol` (fd не клонируется).
pub fn parse_with_fds(bytes: &[u8], fds: Vec<RawFd>) -> Result<Message, DbusError> {
    let mut used = vec![false; fds.len()];
    match parse_inner(bytes, &fds, &mut used) {
        Ok(message) => {
            for (index, raw) in fds.iter().enumerate() {
                if !used[index] {
                    fd::close_fd(*raw);
                }
            }
            Ok(message)
        }
        Err(error) => {
            fd::close_all(&fds);
            Err(error)
        }
    }
}

fn parse_inner(bytes: &[u8], fds: &[RawFd], used: &mut [bool]) -> Result<Message, DbusError> {
    if bytes.len() < 16 {
        return Err(DbusError::Protocol("short header"));
    }
    if bytes[0] != b'l' {
        return Err(DbusError::Protocol("only little-endian is supported"));
    }
    let kind =
        MessageType::from_code(bytes[1]).ok_or(DbusError::Protocol("unknown message type"))?;
    let body_len = u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]) as usize;
    let serial = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let fields_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
    if fields_len > MAX_HEADER_BYTES || body_len > MAX_BODY_BYTES {
        return Err(DbusError::Protocol("message too large"));
    }

    let fields_end = 16usize
        .checked_add(fields_len)
        .ok_or(DbusError::Protocol("header length overflow"))?;
    if fields_end > bytes.len() {
        return Err(DbusError::Protocol("truncated header fields"));
    }

    let fields = {
        let mut cursor = Cursor {
            bytes,
            position: 16,
            end: fields_end,
            fds,
            used,
        };
        let mut fields = Vec::new();
        while cursor.position < fields_end {
            cursor.align(8)?;
            if cursor.position >= fields_end {
                break;
            }
            let code = cursor.byte()?;
            let value = cursor.variant_value()?;
            fields.push((code, value));
        }
        if cursor.position != fields_end {
            return Err(DbusError::Protocol("header fields length mismatch"));
        }
        fields
    };

    let body_offset = (fields_end + 7) & !7;
    let body_end = body_offset
        .checked_add(body_len)
        .ok_or(DbusError::Protocol("body length overflow"))?;
    if body_end > bytes.len() {
        return Err(DbusError::Protocol("truncated body"));
    }
    if body_end != bytes.len() {
        return Err(DbusError::Protocol("trailing bytes after body"));
    }

    let signature = match fields
        .iter()
        .find(|(code, _)| *code == FIELD_SIGNATURE)
        .map(|(_, value)| value)
    {
        None => String::new(),
        Some(Value::String(signature)) => signature.clone(),
        Some(_) => return Err(DbusError::Protocol("SIGNATURE field is not a signature")),
    };

    let mut cursor = Cursor {
        bytes,
        position: body_offset,
        end: body_end,
        fds,
        used,
    };
    let mut body = Vec::new();
    let mut rest = signature.as_str();
    while !rest.is_empty() {
        let (item, tail) = take_type(rest)?;
        body.push(cursor.value(item)?);
        rest = tail;
    }
    if cursor.position != body_end {
        return Err(DbusError::Protocol("body not fully consumed"));
    }

    let message = Message {
        kind,
        serial,
        fields,
        signature,
        body,
    };
    // Поле UNIX_FDS обязано совпадать с реально приложенным числом fd (спека D-Bus):
    // иначе индексы `h` могли бы ссылаться на чужие/потерянные дескрипторы.
    let declared = match message.field(FIELD_UNIX_FDS) {
        None => 0,
        Some(Value::U32(count)) => *count as usize,
        Some(_) => return Err(DbusError::Protocol("UNIX_FDS field is not a u32")),
    };
    if declared != fds.len() {
        return Err(DbusError::Protocol(
            "UNIX_FDS count does not match ancillary fds",
        ));
    }
    Ok(message)
}

/// Сигнатура значения (для [`super::Connection::call`]); `None` для пустых контейнеров,
/// тип элемента которых из значения не выводится.
pub(crate) fn signature_of_value(value: &Value) -> Option<String> {
    Some(match value {
        Value::Byte(_) => "y".to_owned(),
        Value::Bool(_) => "b".to_owned(),
        Value::U32(_) => "u".to_owned(),
        Value::I32(_) => "i".to_owned(),
        Value::String(_) => "s".to_owned(),
        Value::UnixFd(_) => "h".to_owned(),
        Value::Variant(_) => "v".to_owned(),
        Value::Struct(items) => {
            let mut signature = String::from("(");
            for item in items {
                signature.push_str(&signature_of_value(item)?);
            }
            signature.push(')');
            signature
        }
        Value::Array(items) => format!("a{}", signature_of_value(items.first()?)?),
        Value::Dict(entries) => {
            let (key, value) = entries.first()?;
            format!(
                "a{{{}{}}}",
                signature_of_value(key)?,
                signature_of_value(value)?
            )
        }
    })
}

fn field_type(code: u8) -> Option<&'static str> {
    match code {
        FIELD_PATH => Some("o"),
        FIELD_INTERFACE | FIELD_MEMBER | FIELD_ERROR_NAME | FIELD_DESTINATION | FIELD_SENDER => {
            Some("s")
        }
        FIELD_REPLY_SERIAL => Some("u"),
        FIELD_SIGNATURE => Some("g"),
        FIELD_UNIX_FDS => Some("u"),
        _ => None,
    }
}

fn write_body(writer: &mut Writer, signature: &str, body: &[Value]) -> Result<(), DbusError> {
    let mut rest = signature;
    let mut index = 0usize;
    while !rest.is_empty() {
        let (item, tail) = take_type(rest)?;
        let value = body
            .get(index)
            .ok_or(DbusError::Protocol("body has fewer values than signature"))?;
        writer.value(item, value)?;
        rest = tail;
        index += 1;
    }
    if index != body.len() {
        return Err(DbusError::Protocol("body has more values than signature"));
    }
    Ok(())
}

struct Writer {
    buffer: Vec<u8>,
    /// fd в порядке появления `h` в теле; индекс `h` — позиция в этом векторе.
    fds: Vec<RawFd>,
}

impl Writer {
    fn align(&mut self, alignment: usize) -> Result<(), DbusError> {
        while !self.buffer.len().is_multiple_of(alignment) {
            self.buffer.push(0);
        }
        Ok(())
    }

    fn signature(&mut self, signature: &str) {
        let length = signature.len().min(u8::MAX as usize);
        self.buffer.push(length as u8);
        self.buffer
            .extend_from_slice(&signature.as_bytes()[..length]);
        self.buffer.push(0);
    }

    fn string(&mut self, text: &str) -> Result<(), DbusError> {
        self.align(4)?;
        let length =
            u32::try_from(text.len()).map_err(|_| DbusError::Protocol("string too long"))?;
        self.buffer.extend_from_slice(&length.to_le_bytes());
        self.buffer.extend_from_slice(text.as_bytes());
        self.buffer.push(0);
        Ok(())
    }

    fn value(&mut self, signature: &str, value: &Value) -> Result<(), DbusError> {
        let Some(first) = signature.as_bytes().first().copied() else {
            return Err(DbusError::Protocol("empty type signature"));
        };
        match (first, value) {
            (b'y', Value::Byte(byte)) => self.buffer.push(*byte),
            (b'b', Value::Bool(flag)) => {
                self.align(4)?;
                self.buffer
                    .extend_from_slice(&u32::from(*flag).to_le_bytes());
            }
            (b'u', Value::U32(number)) => {
                self.align(4)?;
                self.buffer.extend_from_slice(&number.to_le_bytes());
            }
            (b'i', Value::I32(number)) => {
                self.align(4)?;
                self.buffer.extend_from_slice(&number.to_le_bytes());
            }
            (b'h', Value::UnixFd(raw)) => {
                if self.fds.len() >= fd::MAX_FDS_PER_MESSAGE {
                    return Err(DbusError::Protocol("too many unix fds in message"));
                }
                self.align(4)?;
                let index = u32::try_from(self.fds.len())
                    .map_err(|_| DbusError::Protocol("too many unix fds in message"))?;
                self.fds.push(*raw);
                self.buffer.extend_from_slice(&index.to_le_bytes());
            }
            (b's' | b'o', Value::String(text)) => self.string(text)?,
            // `g` — сигнатура: длина один байт и выравнивание 1, а не строка.
            (b'g', Value::String(text)) => self.signature(text),
            (b'v', Value::Variant(inner)) => self.variant(inner)?,
            (b'a', Value::Array(items)) => {
                let (element, tail) = take_type(&signature[1..])?;
                if !tail.is_empty() {
                    return Err(DbusError::Protocol("array is not the last type"));
                }
                if element.starts_with('{') {
                    return Err(DbusError::Protocol("expected dict value"));
                }
                self.align(4)?;
                let length_position = self.buffer.len();
                self.buffer.extend_from_slice(&[0u8; 4]);
                self.align(alignment_of(element))?;
                let data_start = self.buffer.len();
                for item in items {
                    self.value(element, item)?;
                }
                let data_len = u32::try_from(self.buffer.len() - data_start)
                    .map_err(|_| DbusError::Protocol("array too long"))?;
                self.buffer[length_position..length_position + 4]
                    .copy_from_slice(&data_len.to_le_bytes());
            }
            (b'a', Value::Dict(entries)) => {
                let (element, tail) = take_type(&signature[1..])?;
                if !tail.is_empty() {
                    return Err(DbusError::Protocol("array is not the last type"));
                }
                if !element.starts_with('{') {
                    return Err(DbusError::Protocol("expected array value"));
                }
                let (key_type, value_type) = dict_types(element)?;
                self.align(4)?;
                let length_position = self.buffer.len();
                self.buffer.extend_from_slice(&[0u8; 4]);
                self.align(8)?;
                let data_start = self.buffer.len();
                for (key, item) in entries {
                    self.align(8)?;
                    self.value(key_type, key)?;
                    self.value(value_type, item)?;
                }
                let data_len = u32::try_from(self.buffer.len() - data_start)
                    .map_err(|_| DbusError::Protocol("dict too long"))?;
                self.buffer[length_position..length_position + 4]
                    .copy_from_slice(&data_len.to_le_bytes());
            }
            (b'(', Value::Struct(items)) => {
                if !signature.ends_with(')') {
                    return Err(DbusError::Protocol("unbalanced struct signature"));
                }
                self.align(8)?;
                let inner = &signature[1..signature.len() - 1];
                let mut rest = inner;
                let mut index = 0usize;
                while !rest.is_empty() {
                    let (item, tail) = take_type(rest)?;
                    let value = items.get(index).ok_or(DbusError::Protocol(
                        "struct has fewer fields than signature",
                    ))?;
                    self.value(item, value)?;
                    rest = tail;
                    index += 1;
                }
                if index != items.len() {
                    return Err(DbusError::Protocol("struct has more fields than signature"));
                }
            }
            _ => return Err(DbusError::Protocol("value does not match signature")),
        }
        Ok(())
    }

    fn variant(&mut self, value: &Value) -> Result<(), DbusError> {
        let signature = signature_of_value(value).ok_or(DbusError::Protocol(
            "variant of empty container has no type",
        ))?;
        self.signature(&signature);
        let (item, tail) = take_type(&signature)?;
        if !tail.is_empty() {
            return Err(DbusError::Protocol("variant signature has extra types"));
        }
        self.value(item, value)
    }
}

const fn alignment_of(signature: &str) -> usize {
    match signature.as_bytes()[0] {
        b'n' | b'q' => 2,
        b'u' | b'i' | b'h' | b's' | b'o' | b'a' => 4,
        b'x' | b't' | b'd' | b'(' | b'{' => 8,
        // y, b, g, v
        _ => 1,
    }
}

/// Отделяет первый полный тип сигнатуры от остатка.
fn take_type(signature: &str) -> Result<(&str, &str), DbusError> {
    let bytes = signature.as_bytes();
    let first = *bytes
        .first()
        .ok_or(DbusError::Protocol("empty signature"))?;
    match first {
        b'y' | b'b' | b'u' | b'i' | b'h' | b's' | b'o' | b'g' | b'v' => {
            Ok((&signature[..1], &signature[1..]))
        }
        b'a' => {
            let (inner, rest) = take_type(&signature[1..])?;
            let end = 1 + inner.len();
            Ok((&signature[..end], rest))
        }
        b'(' => {
            let mut depth = 0usize;
            for (index, byte) in bytes.iter().enumerate() {
                match byte {
                    b'(' => depth += 1,
                    b')' => {
                        depth = depth.saturating_sub(1);
                        if depth == 0 {
                            return Ok((&signature[..=index], &signature[index + 1..]));
                        }
                    }
                    _ => {}
                }
            }
            Err(DbusError::Protocol("unbalanced struct signature"))
        }
        b'{' => {
            let (key, after_key) = take_type(&signature[1..])?;
            let (value, _) = take_type(after_key)?;
            let end = 1 + key.len() + value.len() + 1;
            if bytes.get(end - 1) != Some(&b'}') {
                return Err(DbusError::Protocol("unbalanced dict signature"));
            }
            Ok((&signature[..end], &signature[end..]))
        }
        _ => Err(DbusError::Protocol("unsupported type in signature")),
    }
}

fn dict_types(entry: &str) -> Result<(&str, &str), DbusError> {
    if !entry.starts_with('{') || !entry.ends_with('}') {
        return Err(DbusError::Protocol("not a dict entry signature"));
    }
    let inner = &entry[1..entry.len() - 1];
    let (key, rest) = take_type(inner)?;
    let (value, tail) = take_type(rest)?;
    if !tail.is_empty() {
        return Err(DbusError::Protocol("dict entry has extra types"));
    }
    Ok((key, value))
}

struct Cursor<'a> {
    bytes: &'a [u8],
    position: usize,
    end: usize,
    /// fd, приложенные к сообщению; индекс `h` адресует этот срез.
    fds: &'a [RawFd],
    /// Какие fd уже использованы (повторный индекс → `Protocol`).
    used: &'a mut [bool],
}

impl<'a> Cursor<'a> {
    fn align(&mut self, alignment: usize) -> Result<(), DbusError> {
        let remainder = self.position % alignment;
        if remainder != 0 {
            self.position += alignment - remainder;
        }
        if self.position > self.end {
            return Err(DbusError::Protocol("truncated padding"));
        }
        Ok(())
    }

    fn byte(&mut self) -> Result<u8, DbusError> {
        if self.position >= self.end {
            return Err(DbusError::Protocol("read past field boundary"));
        }
        let byte = *self
            .bytes
            .get(self.position)
            .ok_or(DbusError::Protocol("unexpected end of message"))?;
        self.position += 1;
        Ok(byte)
    }

    fn u32(&mut self) -> Result<u32, DbusError> {
        if self.position + 4 > self.end {
            return Err(DbusError::Protocol("u32 past field boundary"));
        }
        let slice = self
            .bytes
            .get(self.position..self.position + 4)
            .ok_or(DbusError::Protocol("truncated u32"))?;
        self.position += 4;
        Ok(u32::from_le_bytes([slice[0], slice[1], slice[2], slice[3]]))
    }

    fn string(&mut self) -> Result<String, DbusError> {
        self.align(4)?;
        let length = self.u32()? as usize;
        let slice = self
            .bytes
            .get(self.position..self.position + length)
            .ok_or(DbusError::Protocol("truncated string"))?;
        if self.position + length + 1 > self.end {
            return Err(DbusError::Protocol("string past field boundary"));
        }
        if self.bytes.get(self.position + length) != Some(&0) {
            return Err(DbusError::Protocol("string is not NUL-terminated"));
        }
        let text = std::str::from_utf8(slice)
            .map_err(|_| DbusError::Protocol("string is not utf-8"))?
            .to_owned();
        self.position += length + 1;
        Ok(text)
    }

    fn signature_string(&mut self) -> Result<String, DbusError> {
        let length = self.byte()? as usize;
        let slice = self
            .bytes
            .get(self.position..self.position + length)
            .ok_or(DbusError::Protocol("truncated signature"))?;
        if self.position + length + 1 > self.end {
            return Err(DbusError::Protocol("signature past field boundary"));
        }
        if self.bytes.get(self.position + length) != Some(&0) {
            return Err(DbusError::Protocol("signature is not NUL-terminated"));
        }
        let text = std::str::from_utf8(slice)
            .map_err(|_| DbusError::Protocol("signature is not utf-8"))?
            .to_owned();
        self.position += length + 1;
        Ok(text)
    }

    /// Читает вариант (сигнатура + значение) и возвращает внутреннее значение.
    fn variant_value(&mut self) -> Result<Value, DbusError> {
        let signature = self.signature_string()?;
        if signature.is_empty() {
            return Err(DbusError::Protocol("empty variant signature"));
        }
        let (item, tail) = take_type(&signature)?;
        if !tail.is_empty() {
            return Err(DbusError::Protocol("variant signature has extra types"));
        }
        self.value(item)
    }

    fn value(&mut self, signature: &str) -> Result<Value, DbusError> {
        let first = *signature
            .as_bytes()
            .first()
            .ok_or(DbusError::Protocol("empty type signature"))?;
        Ok(match first {
            b'y' => Value::Byte(self.byte()?),
            b'b' => {
                self.align(4)?;
                match self.u32()? {
                    0 => Value::Bool(false),
                    1 => Value::Bool(true),
                    _ => return Err(DbusError::Protocol("bool is not 0/1")),
                }
            }
            b'u' => {
                self.align(4)?;
                Value::U32(self.u32()?)
            }
            b'i' => {
                self.align(4)?;
                Value::I32(self.u32()? as i32)
            }
            b'h' => {
                self.align(4)?;
                let index = self.u32()? as usize;
                let raw = *self
                    .fds
                    .get(index)
                    .ok_or(DbusError::Protocol("unix fd index out of range"))?;
                let used = self
                    .used
                    .get_mut(index)
                    .ok_or(DbusError::Protocol("unix fd index out of range"))?;
                if *used {
                    return Err(DbusError::Protocol("duplicate unix fd index"));
                }
                *used = true;
                Value::UnixFd(raw)
            }
            b's' | b'o' => Value::String(self.string()?),
            b'g' => Value::String(self.signature_string()?),
            b'v' => Value::Variant(Box::new(self.variant_value()?)),
            b'a' => {
                self.align(4)?;
                let length = self.u32()? as usize;
                let (element, tail) = take_type(&signature[1..])?;
                if !tail.is_empty() {
                    return Err(DbusError::Protocol("array is not the last type"));
                }
                self.align(alignment_of(element))?;
                let data_start = self.position;
                let data_end = data_start
                    .checked_add(length)
                    .ok_or(DbusError::Protocol("array length overflow"))?;
                if data_end > self.end {
                    return Err(DbusError::Protocol("truncated array"));
                }

                if element.starts_with('{') {
                    let (key_type, value_type) = dict_types(element)?;
                    let mut entries = Vec::new();
                    while self.position < data_end {
                        self.align(8)?;
                        if self.position >= data_end {
                            break;
                        }
                        let key = self.value(key_type)?;
                        let value = self.value(value_type)?;
                        entries.push((key, value));
                    }
                    if self.position != data_end {
                        return Err(DbusError::Protocol("dict length mismatch"));
                    }
                    Value::Dict(entries)
                } else {
                    let mut items = Vec::new();
                    while self.position < data_end {
                        let start = self.position;
                        let item = self.value(element)?;
                        if self.position == start {
                            return Err(DbusError::Protocol("array element did not advance"));
                        }
                        items.push(item);
                    }
                    if self.position != data_end {
                        return Err(DbusError::Protocol("array length mismatch"));
                    }
                    Value::Array(items)
                }
            }
            b'(' => {
                if !signature.ends_with(')') {
                    return Err(DbusError::Protocol("unbalanced struct signature"));
                }
                self.align(8)?;
                let inner = &signature[1..signature.len() - 1];
                let mut items = Vec::new();
                let mut rest = inner;
                while !rest.is_empty() {
                    let (item, tail) = take_type(rest)?;
                    items.push(self.value(item)?);
                    rest = tail;
                }
                Value::Struct(items)
            }
            _ => return Err(DbusError::Protocol("unsupported type in body")),
        })
    }
}

/// fd, на которые ссылается тело (значения `h`), в порядке появления.
pub(crate) fn fds_in_body(body: &[Value]) -> Vec<RawFd> {
    let mut fds = Vec::new();
    collect_fds(body, &mut fds);
    fds
}

/// Закрывает все fd, на которые ссылается тело (ответ/сигнал отбрасывается).
pub(crate) fn close_fds_in_body(body: &[Value]) {
    fd::close_all(&fds_in_body(body));
}

fn collect_fds(values: &[Value], fds: &mut Vec<RawFd>) {
    for value in values {
        match value {
            Value::UnixFd(raw) => fds.push(*raw),
            Value::Array(items) | Value::Struct(items) => collect_fds(items, fds),
            Value::Dict(entries) => {
                for (key, item) in entries {
                    collect_fds(std::slice::from_ref(key), fds);
                    collect_fds(std::slice::from_ref(item), fds);
                }
            }
            Value::Variant(inner) => collect_fds(std::slice::from_ref(inner.as_ref()), fds),
            _ => {}
        }
    }
}
