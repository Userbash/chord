//! # ETF (erlpack): декодер и канонический энкодер
//!
//! Внешние форматы шлюза Discord: поток после распаковки zstd — `0x83` (version byte) перед
//! **каждым** термом, далее терм ETF. Спека: `docs/re/gateway-protocol.md` §2 (11 фактических
//! тегов в захвате + 6 поддержанных-не-встреченных).
//!
//! Главные факты, зафиксированные захватом `capture-run2.jsonl` (сокет `95974.1389`):
//! - big-int `0x6e`/`0x6f`: `[tag][n][sign][n байт LE]` — **знак идёт ДО цифр** (ошибочный
//!   порядок даёт отрицательные snowflake, см. §2.5 спеки);
//! - `LIST_EXT` (`0x6c`) всегда с хвостовым `NIL_EXT` (`0x6a`) — 8 794/8 794;
//! - атом `"nil"` (`0x64`) — это строка, а не `null`; настоящий `null` — только `NIL_EXT`.
//!
//! Декодер не паникует на мусоре: границы проверяются **до** любых выделений памяти, глубина
//! контейнеров ограничена [`MAX_DEPTH`]. Модуль намеренно без внешних крейтов: только `std`,
//! без `unsafe`.

use crate::{TAG_NEW_FLOAT_EXT, TAG_NIL_EXT};
use std::fmt;

/// Максимальная глубина вложенности терма (защита стека); корень — уровень 1.
pub const MAX_DEPTH: usize = 64;

/// Версия/маркер терма в потоке провода (`docs/re/gateway-protocol.md` §2, «Фрейминг»).
pub const VERSION_BYTE: u8 = 0x83;

/// Тег `SMALL_INTEGER_EXT` (u8).
const TAG_SMALL_INTEGER_EXT: u8 = 0x61;
/// Тег `INTEGER_EXT` (i32 BE).
const TAG_INTEGER_EXT: u8 = 0x62;
/// Тег `FLOAT_EXT` (31 Б ASCII/latin1) — только декод, в захвате не встречался.
const TAG_FLOAT_EXT: u8 = 0x63;
/// Тег `ATOM_EXT` (u16 len, latin1).
const TAG_ATOM_EXT: u8 = 0x64;
/// Тег `SMALL_TUPLE_EXT` (u8 count).
const TAG_SMALL_TUPLE_EXT: u8 = 0x68;
/// Тег `LARGE_TUPLE_EXT` (u32 count) — только декод, в захвате не встречался.
const TAG_LARGE_TUPLE_EXT: u8 = 0x69;
/// Тег `STRING_EXT` (u16 len, latin1).
const TAG_STRING_EXT: u8 = 0x6b;
/// Тег `LIST_EXT` (u32 count, хвостовой `NIL_EXT`).
const TAG_LIST_EXT: u8 = 0x6c;
/// Тег `BINARY_EXT` (u32 len, строгий UTF-8).
const TAG_BINARY_EXT: u8 = 0x6d;
/// Тег `SMALL_BIG_EXT` (u8 n; sign; n байт LE).
const TAG_SMALL_BIG_EXT: u8 = 0x6e;
/// Тег `LARGE_BIG_EXT` (u32 n; sign; n байт LE) — только декод, в захвате не встречался.
const TAG_LARGE_BIG_EXT: u8 = 0x6f;
/// Тег `SMALL_ATOM_EXT` (u8 len, latin1) — только декод, в захвате не встречался.
const TAG_SMALL_ATOM_EXT: u8 = 0x73;
/// Тег `MAP_EXT` (u32 count; пары подряд).
const TAG_MAP_EXT: u8 = 0x74;
/// Тег `ATOM_UTF8_EXT` (u16 len, UTF-8) — только декод, в захвате не встречался.
const TAG_ATOM_UTF8_EXT: u8 = 0x76;
/// Тег `SMALL_ATOM_UTF8_EXT` (u8 len, UTF-8) — только декод, в захвате не встречался.
const TAG_SMALL_ATOM_UTF8_EXT: u8 = 0x77;

/// Максимум байт цифр big-int, помещающийся в `u128` без потери точности.
const BIG_INT_MAX_DIGITS: usize = 16;

/// Ошибки разбора ETF. Позиции (`at`) — смещения в переданном буфере.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EtfError {
    /// Буфер оборвался: с позиции `at` нужно `need` байт, но их нет.
    UnexpectedEof { at: usize, need: usize },
    /// Неизвестный (неподдержанный) тег `tag` на позиции `at`.
    UnknownTag { tag: u8, at: usize },
    /// big-int не влезает в `i64` (или `n > 16`) — позиция `at`.
    BigIntOverflow { at: usize },
    /// Строки `0x6d`/`0x76`/`0x77` должны быть строгим UTF-8 (неверно — начиная с `at`).
    InvalidUtf8 { at: usize },
    /// Хвост `LIST_EXT` не `NIL_EXT`: байт хвостового тега `tail_tag` на позиции `at`.
    ListTailNotNil { tail_tag: u8, at: usize },
    /// `FLOAT_EXT` (`0x63`) не разбирается как `f64` — позиция `at`.
    MalformedFloat { at: usize },
    /// Превышена [`MAX_DEPTH`] при входе в контейнер на позиции `at`.
    DepthLimit { at: usize },
}

impl fmt::Display for EtfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedEof { at, need } => {
                write!(f, "неожиданный конец буфера: с позиции {at} нужно {need} байт")
            }
            Self::UnknownTag { tag, at } => {
                write!(f, "неизвестный тег 0x{tag:02x} на позиции {at}")
            }
            Self::BigIntOverflow { at } => {
                write!(f, "big-int на позиции {at} не влезает в i64")
            }
            Self::InvalidUtf8 { at } => write!(f, "не строгий UTF-8 начиная с позиции {at}"),
            Self::ListTailNotNil { tail_tag, at } => write!(
                f,
                "хвост LIST_EXT не NIL_EXT: тег 0x{tail_tag:02x} на позиции {at}"
            ),
            Self::MalformedFloat { at } => {
                write!(f, "FLOAT_EXT на позиции {at} не разобран как f64")
            }
            Self::DepthLimit { at } => {
                write!(f, "превышена глубина {MAX_DEPTH} на позиции {at}")
            }
        }
    }
}

impl std::error::Error for EtfError {}

/// Значение терма ETF (erlpack-совместимая модель).
#[derive(Debug, Clone, PartialEq)]
pub enum EtfValue {
    /// `NIL_EXT` (`0x6a`) — настоящий `null`.
    Nil,
    /// Атом (`0x64`/`0x73`/`0x76`/`0x77`), включая `"nil"`/`"true"`/`"false"` — строками.
    Atom(String),
    /// Целое (`0x61`/`0x62`/`0x6e`/`0x6f`).
    Int(i64),
    /// Плавающее (`0x46`/`0x63`).
    Float(f64),
    /// Бинарная/строковая строка: `0x6d` (UTF-8) или `0x6b` (latin1).
    Str(String),
    /// Кортеж (`0x68`/`0x69`) — в трафике массивы без хвоста.
    Tuple(Vec<EtfValue>),
    /// Список (`0x6c`), хвост обязан быть `NIL_EXT`.
    List(Vec<EtfValue>),
    /// Map (`0x74`); порядок пар сохраняется.
    Map(Vec<(EtfValue, EtfValue)>),
}

impl EtfValue {
    /// Значение по ключу map: ключ-атом `Atom(key)` или ключ-строка `Str(key)`.
    pub fn get(&self, key: &str) -> Option<&EtfValue> {
        let EtfValue::Map(pairs) = self else {
            return None;
        };
        pairs.iter().find_map(|(name, value)| match name {
            EtfValue::Atom(name) | EtfValue::Str(name) if name == key => Some(value),
            _ => None,
        })
    }

    /// `Int` → значение; прочие варианты → `None`.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            Self::Int(value) => Some(*value),
            _ => None,
        }
    }

    /// `Float` → значение; прочие варианты → `None`.
    pub fn as_float(&self) -> Option<f64> {
        match self {
            Self::Float(value) => Some(*value),
            _ => None,
        }
    }

    /// Только `Atom` → строка (атом `"nil"` — тоже атом).
    pub fn as_atom(&self) -> Option<&str> {
        match self {
            Self::Atom(name) => Some(name.as_str()),
            _ => None,
        }
    }

    /// Только `Str` → строка (бинарники `0x6d`/`0x6b`).
    pub fn as_str(&self) -> Option<&str> {
        match self {
            Self::Str(text) => Some(text.as_str()),
            _ => None,
        }
    }

    /// `List` → срез элементов; прочие варианты → `None`.
    pub fn as_list(&self) -> Option<&[EtfValue]> {
        match self {
            Self::List(items) => Some(items.as_slice()),
            _ => None,
        }
    }

    /// `Map` → срез пар; прочие варианты → `None`.
    pub fn as_map(&self) -> Option<&[(EtfValue, EtfValue)]> {
        match self {
            Self::Map(pairs) => Some(pairs.as_slice()),
            _ => None,
        }
    }
}

/// Декод одного терма: ведущий `0x83` пропускается, если присутствует (формат провода).
///
/// Возврат: `(значение, потреблённое число байт, включая 0x83)`.
pub fn decode(buf: &[u8]) -> Result<(EtfValue, usize), EtfError> {
    let mut reader = Reader::new(buf);
    if reader.peek()? == VERSION_BYTE {
        reader.pos += 1;
    }
    let value = reader.term(1)?;
    Ok((value, reader.pos))
}

/// Декод одного терма **без** префикса `0x83` (`0x83` в начале → `UnknownTag`).
pub fn decode_term(buf: &[u8]) -> Result<(EtfValue, usize), EtfError> {
    let mut reader = Reader::new(buf);
    let value = reader.term(1)?;
    Ok((value, reader.pos))
}

/// Канонический энкод терма: `0x83` + терм.
pub fn encode(value: &EtfValue) -> Vec<u8> {
    let mut out = Vec::new();
    out.push(VERSION_BYTE);
    write_term(value, &mut out);
    out
}

/// Канонический энкод терма без префикса.
pub fn encode_term(value: &EtfValue) -> Vec<u8> {
    let mut out = Vec::new();
    write_term(value, &mut out);
    out
}

/// Позиционный читатель с проверкой границ **до** любых выделений памяти.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
        Self { buf, pos: 0 }
    }

    /// Сколько байт ещё не прочитано.
    fn remaining(&self) -> usize {
        self.buf.len().saturating_sub(self.pos)
    }

    /// Текущий байт без продвижения (для детекта ведущего `0x83`).
    fn peek(&self) -> Result<u8, EtfError> {
        self.buf.get(self.pos).copied().ok_or(EtfError::UnexpectedEof {
            at: self.pos,
            need: 1,
        })
    }

    /// Читает `n` байт; при нехватке — `UnexpectedEof` (без `with_capacity(n)`).
    fn take(&mut self, n: usize) -> Result<&'a [u8], EtfError> {
        if self.remaining() < n {
            return Err(EtfError::UnexpectedEof {
                at: self.pos,
                need: n,
            });
        }
        let slice = &self.buf[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn u8(&mut self) -> Result<u8, EtfError> {
        Ok(self.take(1)?[0])
    }

    fn u16_be(&mut self) -> Result<u16, EtfError> {
        let bytes = self.take(2)?;
        Ok(u16::from_be_bytes([bytes[0], bytes[1]]))
    }

    fn u32_be(&mut self) -> Result<u32, EtfError> {
        let bytes = self.take(4)?;
        Ok(u32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn i32_be(&mut self) -> Result<i32, EtfError> {
        let bytes = self.take(4)?;
        Ok(i32::from_be_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
    }

    fn f64_be(&mut self) -> Result<f64, EtfError> {
        let bytes = self.take(8)?;
        let mut raw = [0u8; 8];
        raw.copy_from_slice(bytes);
        Ok(f64::from_be_bytes(raw))
    }

    /// Один терм; `depth` — уровень вложенности (корень — 1).
    fn term(&mut self, depth: usize) -> Result<EtfValue, EtfError> {
        let at = self.pos;
        let tag = self.u8()?;
        match tag {
            TAG_NEW_FLOAT_EXT => Ok(EtfValue::Float(self.f64_be()?)),
            TAG_SMALL_INTEGER_EXT => Ok(EtfValue::Int(i64::from(self.u8()?))),
            TAG_INTEGER_EXT => Ok(EtfValue::Int(i64::from(self.i32_be()?))),
            TAG_FLOAT_EXT => self.float_ext(at),
            TAG_ATOM_EXT => {
                let len = usize::from(self.u16_be()?);
                Ok(EtfValue::Atom(self.latin1(len)?))
            }
            TAG_SMALL_ATOM_EXT => {
                let len = usize::from(self.u8()?);
                Ok(EtfValue::Atom(self.latin1(len)?))
            }
            TAG_SMALL_TUPLE_EXT => {
                let count = usize::from(self.u8()?);
                self.tuple(count, depth, at)
            }
            TAG_LARGE_TUPLE_EXT => {
                let count = self.u32_be()? as usize;
                self.tuple(count, depth, at)
            }
            TAG_NIL_EXT => Ok(EtfValue::Nil),
            TAG_STRING_EXT => {
                let len = usize::from(self.u16_be()?);
                Ok(EtfValue::Str(self.latin1(len)?))
            }
            TAG_LIST_EXT => self.list(depth, at),
            TAG_BINARY_EXT => {
                let len = self.u32_be()? as usize;
                let data_at = self.pos;
                Ok(EtfValue::Str(self.utf8(len, data_at)?))
            }
            TAG_SMALL_BIG_EXT => {
                let digits = usize::from(self.u8()?);
                self.big_int(digits, at)
            }
            TAG_LARGE_BIG_EXT => {
                let digits = self.u32_be()? as usize;
                self.big_int(digits, at)
            }
            TAG_ATOM_UTF8_EXT => {
                let len = usize::from(self.u16_be()?);
                let data_at = self.pos;
                Ok(EtfValue::Atom(self.utf8(len, data_at)?))
            }
            TAG_SMALL_ATOM_UTF8_EXT => {
                let len = usize::from(self.u8()?);
                let data_at = self.pos;
                Ok(EtfValue::Atom(self.utf8(len, data_at)?))
            }
            TAG_MAP_EXT => self.map(depth, at),
            other => Err(EtfError::UnknownTag { tag: other, at }),
        }
    }

    /// `0x63 FLOAT_EXT`: 31 Б ASCII/latin1, значимая часть без хвостовых `\0` и пробелов.
    fn float_ext(&mut self, at: usize) -> Result<EtfValue, EtfError> {
        let raw = self.take(31)?;
        let mut end = raw.len();
        while end > 0 && matches!(raw[end - 1], 0 | b' ') {
            end -= 1;
        }
        let text = std::str::from_utf8(&raw[..end]).map_err(|_| EtfError::MalformedFloat { at })?;
        text.parse::<f64>()
            .map(EtfValue::Float)
            .map_err(|_| EtfError::MalformedFloat { at })
    }

    /// `0x64`/`0x73`/`0x6b`: latin1 — каждый байт становится code point.
    fn latin1(&mut self, len: usize) -> Result<String, EtfError> {
        let bytes = self.take(len)?;
        Ok(bytes.iter().map(|&byte| char::from(byte)).collect())
    }

    /// `0x6d`/`0x76`/`0x77`: строгий UTF-8 (иначе `InvalidUtf8`).
    fn utf8(&mut self, len: usize, at: usize) -> Result<String, EtfError> {
        let bytes = self.take(len)?;
        std::str::from_utf8(bytes)
            .map(str::to_owned)
            .map_err(|_| EtfError::InvalidUtf8 { at })
    }

    /// `0x68`/`0x69`: кортеж (в трафике — массивы без хвоста).
    fn tuple(&mut self, count: usize, depth: usize, at: usize) -> Result<EtfValue, EtfError> {
        check_depth(depth, at)?;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(self.term(depth + 1)?);
        }
        Ok(EtfValue::Tuple(items))
    }

    /// `0x6c`: список с обязательным хвостовым `NIL_EXT` (8794/8794 в захвате).
    fn list(&mut self, depth: usize, at: usize) -> Result<EtfValue, EtfError> {
        check_depth(depth, at)?;
        let count = self.u32_be()? as usize;
        let mut items = Vec::new();
        for _ in 0..count {
            items.push(self.term(depth + 1)?);
        }
        let tail_at = self.pos;
        let tail_tag = self.u8()?;
        if tail_tag != TAG_NIL_EXT {
            return Err(EtfError::ListTailNotNil {
                tail_tag,
                at: tail_at,
            });
        }
        Ok(EtfValue::List(items))
    }

    /// `0x74`: map; порядок пар сохраняется.
    fn map(&mut self, depth: usize, at: usize) -> Result<EtfValue, EtfError> {
        check_depth(depth, at)?;
        let count = self.u32_be()? as usize;
        let mut pairs = Vec::new();
        for _ in 0..count {
            let key = self.term(depth + 1)?;
            let value = self.term(depth + 1)?;
            pairs.push((key, value));
        }
        Ok(EtfValue::Map(pairs))
    }

    /// `0x6e`/`0x6f`: big-int `[n][sign][n байт LE]` — знак **до** цифр (§2.5 спеки).
    fn big_int(&mut self, digits: usize, at: usize) -> Result<EtfValue, EtfError> {
        if digits > BIG_INT_MAX_DIGITS {
            return Err(EtfError::BigIntOverflow { at });
        }
        let sign = self.u8()?;
        let bytes = self.take(digits)?;
        let mut magnitude: u128 = 0;
        for &byte in bytes.iter().rev() {
            magnitude = (magnitude << 8) | u128::from(byte);
        }
        const TWO_POW_63: u128 = 1 << 63;
        if sign == 0 {
            if magnitude > i64::MAX as u128 {
                return Err(EtfError::BigIntOverflow { at });
            }
            Ok(EtfValue::Int(magnitude as i64))
        } else {
            if magnitude > TWO_POW_63 {
                return Err(EtfError::BigIntOverflow { at });
            }
            if magnitude == TWO_POW_63 {
                return Ok(EtfValue::Int(i64::MIN));
            }
            Ok(EtfValue::Int(-(magnitude as i64)))
        }
    }
}

/// Проверка лимита глубины при входе в контейнер (корень — уровень 1).
fn check_depth(depth: usize, at: usize) -> Result<(), EtfError> {
    if depth > MAX_DEPTH {
        Err(EtfError::DepthLimit { at })
    } else {
        Ok(())
    }
}

/// Канонический энкод терма (без префикса `0x83`) в буфер.
fn write_term(value: &EtfValue, out: &mut Vec<u8>) {
    match value {
        EtfValue::Nil => out.push(TAG_NIL_EXT),
        EtfValue::Int(number) => write_int(*number, out),
        EtfValue::Float(number) => {
            out.push(TAG_NEW_FLOAT_EXT);
            out.extend_from_slice(&number.to_be_bytes());
        }
        EtfValue::Str(text) => write_binary(text, out),
        EtfValue::Atom(name) => write_atom(name, out),
        EtfValue::Tuple(items) => {
            if items.len() <= usize::from(u8::MAX) {
                out.push(TAG_SMALL_TUPLE_EXT);
                out.push(items.len() as u8);
            } else {
                out.push(TAG_LARGE_TUPLE_EXT);
                out.extend_from_slice(&(items.len() as u32).to_be_bytes());
            }
            for item in items {
                write_term(item, out);
            }
        }
        EtfValue::List(items) => {
            out.push(TAG_LIST_EXT);
            out.extend_from_slice(&(items.len() as u32).to_be_bytes());
            for item in items {
                write_term(item, out);
            }
            out.push(TAG_NIL_EXT);
        }
        EtfValue::Map(pairs) => {
            out.push(TAG_MAP_EXT);
            out.extend_from_slice(&(pairs.len() as u32).to_be_bytes());
            for (key, value) in pairs {
                write_term(key, out);
                write_term(value, out);
            }
        }
    }
}

/// `0x6d`: бинарник — u32 BE длина + UTF-8 (`0x6b`/`0x63` энкодер не производит).
fn write_binary(text: &str, out: &mut Vec<u8>) {
    out.push(TAG_BINARY_EXT);
    out.extend_from_slice(&(text.len() as u32).to_be_bytes());
    out.extend_from_slice(text.as_bytes());
}

/// Атом: чистый ASCII → `0x64`; иначе ≤255 Б → `0x77`; иначе → `0x76`.
fn write_atom(name: &str, out: &mut Vec<u8>) {
    let bytes = name.as_bytes();
    if bytes.is_ascii() {
        out.push(TAG_ATOM_EXT);
        out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    } else if bytes.len() <= usize::from(u8::MAX) {
        out.push(TAG_SMALL_ATOM_UTF8_EXT);
        out.push(bytes.len() as u8);
    } else {
        out.push(TAG_ATOM_UTF8_EXT);
        out.extend_from_slice(&(bytes.len() as u16).to_be_bytes());
    }
    out.extend_from_slice(bytes);
}

/// Целое: `0..=255` → `0x61`; диапазон i32 → `0x62`; иначе big-int `0x6e` (знак, затем LE-цифры).
fn write_int(number: i64, out: &mut Vec<u8>) {
    if (0..=i64::from(u8::MAX)).contains(&number) {
        out.push(TAG_SMALL_INTEGER_EXT);
        out.push(number as u8);
        return;
    }
    if let Ok(narrow) = i32::try_from(number) {
        out.push(TAG_INTEGER_EXT);
        out.extend_from_slice(&narrow.to_be_bytes());
        return;
    }
    out.push(TAG_SMALL_BIG_EXT);
    let magnitude = number.unsigned_abs();
    let digits = min_digits(magnitude);
    out.push(digits);
    out.push(u8::from(number < 0));
    out.extend_from_slice(&magnitude.to_le_bytes()[..usize::from(digits)]);
}

/// Минимальное число байт для величины (≥1); `u64::MIN.leading_zeros()` даёт 1, а не 0.
fn min_digits(magnitude: u64) -> u8 {
    let bits = u64::BITS - magnitude.leading_zeros();
    bits.div_ceil(8).max(1) as u8
}
