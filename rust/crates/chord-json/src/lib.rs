//! # discord-json
//!
//! Строгий JSON-парсер по RFC 8259 без внешних зависимостей и без `unsafe`.
//!
//! Foundation волны RW-22: используется i18n-каталогами (`ru-catalog.json`), RTC-кадрами
//! голосовой сигнализации и read-only REST-ресурсами. Парсер не паникует на входе, хранит
//! числа в исходном виде (чтобы не терять `u64`/точность) и возвращает ошибку с байтовым
//! смещением, не включая содержимое документа в сообщение.
//!
//! Ограничения: [`MAX_DEPTH`] вложенных контейнеров, [`MAX_NODES`] значений на документ,
//! запрет duplicate keys. Это защита от DoS на недоверенном JSON, а не «мягкий» режим.

use std::collections::BTreeMap;
use std::fmt;

/// Максимальная вложенность массивов/объектов (128 уровней).
pub const MAX_DEPTH: usize = 128;

/// Максимальное число JSON-значений в одном документе.
pub const MAX_NODES: usize = 4_000_000;

/// JSON-значение.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    /// `null`.
    Null,
    /// `true`/`false`.
    Bool(bool),
    /// Число в исходном лексическом виде.
    Number(JsonNumber),
    /// Строка.
    String(String),
    /// Массив.
    Array(Vec<JsonValue>),
    /// Объект с детерминированным порядком ключей.
    Object(BTreeMap<String, JsonValue>),
}

impl JsonValue {
    /// Число в исходном виде.
    #[must_use]
    pub fn as_number(&self) -> Option<&JsonNumber> {
        match self {
            Self::Number(value) => Some(value),
            _ => None,
        }
    }

    /// Строка.
    #[must_use]
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::String(value) => Some(value),
            _ => None,
        }
    }

    /// Булево значение.
    #[must_use]
    pub fn as_bool(&self) -> Option<bool> {
        match self {
            Self::Bool(value) => Some(*value),
            _ => None,
        }
    }

    /// Массив.
    #[must_use]
    pub fn as_array(&self) -> Option<&[JsonValue]> {
        match self {
            Self::Array(value) => Some(value),
            _ => None,
        }
    }

    /// Объект.
    #[must_use]
    pub fn as_object(&self) -> Option<&BTreeMap<String, JsonValue>> {
        match self {
            Self::Object(value) => Some(value),
            _ => None,
        }
    }

    /// Значение объекта по ключу; для не-объекта — `None`.
    #[must_use]
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        self.as_object()?.get(key)
    }

    /// Число как `i64` (только целые в диапазоне).
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        self.as_number()?.as_i64()
    }

    /// Число как `u64` (только неотрицательные целые в диапазоне).
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        self.as_number()?.as_u64()
    }

    /// Число как `f64` (для дробных/экспоненциальных форм).
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        self.as_number()?.as_f64()
    }

    /// `true`, если значение — `null`.
    #[must_use]
    pub fn is_null(&self) -> bool {
        matches!(self, Self::Null)
    }
}

/// Число в исходном лексическом виде (без потери точности при разборе).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonNumber(String);

impl JsonNumber {
    /// Целое неотрицательное число для программно собранного JSON.
    #[must_use]
    pub fn from_u64(value: u64) -> Self {
        Self(value.to_string())
    }

    /// Целое число со знаком для программно собранного JSON.
    #[must_use]
    pub fn from_i64(value: i64) -> Self {
        Self(value.to_string())
    }

    /// Исходная запись числа.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Целое `i64`, если запись — целое в диапазоне.
    #[must_use]
    pub fn as_i64(&self) -> Option<i64> {
        if self.0.contains(['.', 'e', 'E']) {
            return None;
        }
        self.0.parse().ok()
    }

    /// Неотрицательное целое `u64`, если запись — целое в диапазоне.
    #[must_use]
    pub fn as_u64(&self) -> Option<u64> {
        if self.0.starts_with('-') || self.0.contains(['.', 'e', 'E']) {
            return None;
        }
        self.0.parse().ok()
    }

    /// Число как `f64`; `None` для не-конечных значений.
    #[must_use]
    pub fn as_f64(&self) -> Option<f64> {
        self.0.parse::<f64>().ok().filter(|value| value.is_finite())
    }
}

/// Вид ошибки разбора.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JsonErrorKind {
    /// Вход закончился раньше, чем ожидалось.
    UnexpectedEof,
    /// Неожиданный байт.
    UnexpectedByte(u8),
    /// Некорректная запись числа.
    InvalidNumber,
    /// Неизвестная escape-последовательность.
    InvalidEscape,
    /// Некорректная Unicode-последовательность или одиночный surrogate.
    InvalidUnicode,
    /// Управляющий символ внутри строки.
    ControlCharacter,
    /// Повторяющийся ключ объекта.
    DuplicateKey,
    /// Превышена [`MAX_DEPTH`].
    DepthLimit,
    /// Превышен [`MAX_NODES`].
    NodeLimit,
    /// После значения остались непробельные байты.
    TrailingData,
}

impl fmt::Display for JsonErrorKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof => f.write_str("unexpected end of input"),
            Self::UnexpectedByte(byte) => write!(f, "unexpected byte 0x{byte:02x}"),
            Self::InvalidNumber => f.write_str("invalid number"),
            Self::InvalidEscape => f.write_str("invalid escape"),
            Self::InvalidUnicode => f.write_str("invalid unicode escape"),
            Self::ControlCharacter => f.write_str("unescaped control character in string"),
            Self::DuplicateKey => f.write_str("duplicate object key"),
            Self::DepthLimit => f.write_str("maximum nesting depth exceeded"),
            Self::NodeLimit => f.write_str("maximum node count exceeded"),
            Self::TrailingData => f.write_str("trailing data after value"),
        }
    }
}

/// Ошибка разбора JSON с байтовым смещением.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JsonError {
    /// Байтовое смещение от начала входа (0…=len).
    pub offset: usize,
    /// Вид ошибки.
    pub kind: JsonErrorKind,
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "JSON error at offset {}: {}", self.offset, self.kind)
    }
}

impl std::error::Error for JsonError {}

/// Сериализовать JSON-значение в компактную строку.
///
/// Строки экранируются по RFC 8259 (control-символы — `\u00XX`), не-ASCII остаётся UTF-8,
/// числа печатаются в исходном лексическом виде. Функция не паникует и всегда возвращает
/// валидный JSON для значения, полученного из [`parse`] или собранного из [`JsonValue`].
#[must_use]
pub fn to_json_string(value: &JsonValue) -> String {
    let mut out = String::new();
    write_json(value, &mut out);
    out
}

fn write_json(value: &JsonValue, out: &mut String) {
    match value {
        JsonValue::Null => out.push_str("null"),
        JsonValue::Bool(true) => out.push_str("true"),
        JsonValue::Bool(false) => out.push_str("false"),
        JsonValue::Number(number) => out.push_str(number.as_str()),
        JsonValue::String(text) => write_json_string(text, out),
        JsonValue::Array(values) => {
            out.push('[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_json(value, out);
            }
            out.push(']');
        }
        JsonValue::Object(values) => {
            out.push('{');
            for (index, (key, value)) in values.iter().enumerate() {
                if index > 0 {
                    out.push(',');
                }
                write_json_string(key, out);
                out.push(':');
                write_json(value, out);
            }
            out.push('}');
        }
    }
}

fn write_json_string(text: &str, out: &mut String) {
    out.push('"');
    for ch in text.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if ch < '\u{20}' => {
                out.push_str("\\u00");
                let byte = ch as u8;
                out.push(char::from_digit(u32::from(byte >> 4), 16).unwrap_or('0'));
                out.push(char::from_digit(u32::from(byte & 0x0f), 16).unwrap_or('0'));
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

/// Разобрать JSON-документ.
///
/// # Errors
///
/// Любое нарушение RFC 8259, duplicate keys, превышение [`MAX_DEPTH`]/[`MAX_NODES`] или
/// непробельные байты после значения.
pub fn parse(input: &str) -> Result<JsonValue, JsonError> {
    let mut parser = Parser {
        input,
        bytes: input.as_bytes(),
        pos: 0,
        nodes: 0,
    };
    parser.skip_ws();
    let value = parser.parse_value(0)?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(parser.error(JsonErrorKind::TrailingData));
    }
    Ok(value)
}

struct Parser<'a> {
    input: &'a str,
    bytes: &'a [u8],
    pos: usize,
    nodes: usize,
}

impl Parser<'_> {
    fn error(&self, kind: JsonErrorKind) -> JsonError {
        JsonError {
            offset: self.pos,
            kind,
        }
    }

    fn error_at(&self, offset: usize, kind: JsonErrorKind) -> JsonError {
        JsonError { offset, kind }
    }

    fn skip_ws(&mut self) {
        while matches!(self.bytes.get(self.pos), Some(b' ' | b'\n' | b'\r' | b'\t')) {
            self.pos += 1;
        }
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn expect(&mut self, byte: u8) -> Result<(), JsonError> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(JsonErrorKind::UnexpectedByte(self.peek().unwrap_or(0))))
        }
    }

    fn parse_value(&mut self, depth: usize) -> Result<JsonValue, JsonError> {
        if depth > MAX_DEPTH {
            return Err(self.error(JsonErrorKind::DepthLimit));
        }
        self.nodes = self.nodes.saturating_add(1);
        if self.nodes > MAX_NODES {
            return Err(self.error(JsonErrorKind::NodeLimit));
        }
        match self.peek() {
            None => Err(self.error(JsonErrorKind::UnexpectedEof)),
            Some(b'n') => {
                self.consume_literal(b"null")?;
                Ok(JsonValue::Null)
            }
            Some(b't') => {
                self.consume_literal(b"true")?;
                Ok(JsonValue::Bool(true))
            }
            Some(b'f') => {
                self.consume_literal(b"false")?;
                Ok(JsonValue::Bool(false))
            }
            Some(b'"') => Ok(JsonValue::String(self.parse_string()?)),
            Some(b'{') => self.parse_object(depth),
            Some(b'[') => self.parse_array(depth),
            Some(b'-' | b'0'..=b'9') => Ok(JsonValue::Number(self.parse_number()?)),
            Some(other) => Err(self.error(JsonErrorKind::UnexpectedByte(other))),
        }
    }

    fn consume_literal(&mut self, literal: &[u8]) -> Result<(), JsonError> {
        if self.bytes[self.pos..].starts_with(literal) {
            self.pos += literal.len();
            Ok(())
        } else {
            Err(self.error(JsonErrorKind::UnexpectedByte(self.peek().unwrap_or(0))))
        }
    }

    fn parse_object(&mut self, depth: usize) -> Result<JsonValue, JsonError> {
        self.expect(b'{')?;
        self.skip_ws();
        let mut object = BTreeMap::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(JsonValue::Object(object));
        }
        loop {
            self.skip_ws();
            match self.peek() {
                Some(b'"') => {}
                Some(other) => return Err(self.error(JsonErrorKind::UnexpectedByte(other))),
                None => return Err(self.error(JsonErrorKind::UnexpectedEof)),
            }
            let key_offset = self.pos;
            let key = self.parse_string()?;
            self.skip_ws();
            self.expect(b':')?;
            self.skip_ws();
            let value = self.parse_value(depth + 1)?;
            if object.insert(key, value).is_some() {
                return Err(self.error_at(key_offset, JsonErrorKind::DuplicateKey));
            }
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b'}') => {
                    self.pos += 1;
                    return Ok(JsonValue::Object(object));
                }
                Some(other) => return Err(self.error(JsonErrorKind::UnexpectedByte(other))),
                None => return Err(self.error(JsonErrorKind::UnexpectedEof)),
            }
        }
    }

    fn parse_array(&mut self, depth: usize) -> Result<JsonValue, JsonError> {
        self.expect(b'[')?;
        self.skip_ws();
        let mut array = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(JsonValue::Array(array));
        }
        loop {
            self.skip_ws();
            array.push(self.parse_value(depth + 1)?);
            self.skip_ws();
            match self.peek() {
                Some(b',') => {
                    self.pos += 1;
                }
                Some(b']') => {
                    self.pos += 1;
                    return Ok(JsonValue::Array(array));
                }
                Some(other) => return Err(self.error(JsonErrorKind::UnexpectedByte(other))),
                None => return Err(self.error(JsonErrorKind::UnexpectedEof)),
            }
        }
    }

    fn parse_number(&mut self) -> Result<JsonNumber, JsonError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.error(JsonErrorKind::InvalidNumber));
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.peek(), Some(b'0'..=b'9')) {
                    self.pos += 1;
                }
            }
            _ => return Err(self.error(JsonErrorKind::InvalidNumber)),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error(JsonErrorKind::InvalidNumber));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error(JsonErrorKind::InvalidNumber));
            }
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
        }
        Ok(JsonNumber(self.input[start..self.pos].to_string()))
    }

    fn parse_string(&mut self) -> Result<String, JsonError> {
        self.expect(b'"')?;
        let mut out = Vec::new();
        loop {
            let byte = self
                .peek()
                .ok_or_else(|| self.error(JsonErrorKind::UnexpectedEof))?;
            match byte {
                b'"' => {
                    self.pos += 1;
                    return String::from_utf8(out)
                        .map_err(|_| self.error(JsonErrorKind::InvalidUnicode));
                }
                b'\\' => {
                    let escape_offset = self.pos;
                    self.pos += 1;
                    let escaped = self
                        .peek()
                        .ok_or_else(|| self.error(JsonErrorKind::UnexpectedEof))?;
                    self.pos += 1;
                    match escaped {
                        b'"' => out.push(b'"'),
                        b'\\' => out.push(b'\\'),
                        b'/' => out.push(b'/'),
                        b'b' => out.push(0x08),
                        b'f' => out.push(0x0c),
                        b'n' => out.push(b'\n'),
                        b'r' => out.push(b'\r'),
                        b't' => out.push(b'\t'),
                        b'u' => {
                            let first = self.parse_hex4(escape_offset)?;
                            let code = if (0xd800..=0xdbff).contains(&first) {
                                if self.bytes.get(self.pos..self.pos + 2) != Some(b"\\u") {
                                    return Err(
                                        self.error_at(escape_offset, JsonErrorKind::InvalidUnicode)
                                    );
                                }
                                self.pos += 2;
                                let second = self.parse_hex4(escape_offset)?;
                                if !(0xdc00..=0xdfff).contains(&second) {
                                    return Err(
                                        self.error_at(escape_offset, JsonErrorKind::InvalidUnicode)
                                    );
                                }
                                let code = 0x1_0000
                                    + ((u32::from(first) - 0xd800) << 10)
                                    + (u32::from(second) - 0xdc00);
                                char::from_u32(code).ok_or_else(|| {
                                    self.error_at(escape_offset, JsonErrorKind::InvalidUnicode)
                                })?
                            } else if (0xdc00..=0xdfff).contains(&first) {
                                return Err(
                                    self.error_at(escape_offset, JsonErrorKind::InvalidUnicode)
                                );
                            } else {
                                char::from_u32(u32::from(first)).ok_or_else(|| {
                                    self.error_at(escape_offset, JsonErrorKind::InvalidUnicode)
                                })?
                            };
                            let mut buffer = [0u8; 4];
                            out.extend_from_slice(code.encode_utf8(&mut buffer).as_bytes());
                        }
                        _ => {
                            return Err(self.error_at(escape_offset, JsonErrorKind::InvalidEscape));
                        }
                    }
                }
                0x00..=0x1f => {
                    return Err(self.error(JsonErrorKind::ControlCharacter));
                }
                _ => {
                    let ch = self.input[self.pos..]
                        .chars()
                        .next()
                        .ok_or_else(|| self.error(JsonErrorKind::UnexpectedEof))?;
                    let mut buffer = [0u8; 4];
                    out.extend_from_slice(ch.encode_utf8(&mut buffer).as_bytes());
                    self.pos += ch.len_utf8();
                }
            }
        }
    }

    fn parse_hex4(&mut self, escape_offset: usize) -> Result<u16, JsonError> {
        let Some(slice) = self.bytes.get(self.pos..self.pos + 4) else {
            return Err(self.error_at(escape_offset, JsonErrorKind::InvalidUnicode));
        };
        let mut value = 0u16;
        for byte in slice {
            let digit = match byte {
                b'0'..=b'9' => u16::from(byte - b'0'),
                b'a'..=b'f' => u16::from(byte - b'a') + 10,
                b'A'..=b'F' => u16::from(byte - b'A') + 10,
                _ => {
                    return Err(self.error_at(escape_offset, JsonErrorKind::InvalidUnicode));
                }
            };
            value = (value << 4) | digit;
        }
        self.pos += 4;
        Ok(value)
    }
}
