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
    todo!("RW-01 red")
}

    /// `Int` → значение; прочие варианты → `None`.
    pub fn as_int(&self) -> Option<i64> {
    todo!("RW-01 red")
}

    /// `Float` → значение; прочие варианты → `None`.
    pub fn as_float(&self) -> Option<f64> {
    todo!("RW-01 red")
}

    /// Только `Atom` → строка (атом `"nil"` — тоже атом).
    pub fn as_atom(&self) -> Option<&str> {
    todo!("RW-01 red")
}

    /// Только `Str` → строка (бинарники `0x6d`/`0x6b`).
    pub fn as_str(&self) -> Option<&str> {
    todo!("RW-01 red")
}

    /// `List` → срез элементов; прочие варианты → `None`.
    pub fn as_list(&self) -> Option<&[EtfValue]> {
    todo!("RW-01 red")
}

    /// `Map` → срез пар; прочие варианты → `None`.
    pub fn as_map(&self) -> Option<&[(EtfValue, EtfValue)]> {
    todo!("RW-01 red")
}
}

/// Декод одного терма: ведущий `0x83` пропускается, если присутствует (формат провода).
///
/// Возврат: `(значение, потреблённое число байт, включая 0x83)`.
pub fn decode(buf: &[u8]) -> Result<(EtfValue, usize), EtfError> {
    todo!("RW-01 red")
}

/// Декод одного терма **без** префикса `0x83` (`0x83` в начале → `UnknownTag`).
pub fn decode_term(buf: &[u8]) -> Result<(EtfValue, usize), EtfError> {
    todo!("RW-01 red")
}

/// Канонический энкод терма: `0x83` + терм.
pub fn encode(value: &EtfValue) -> Vec<u8> {
    todo!("RW-01 red")
}

/// Канонический энкод терма без префикса.
pub fn encode_term(value: &EtfValue) -> Vec<u8> {
    todo!("RW-01 red")
}

/// Позиционный читатель с проверкой границ **до** любых выделений памяти.
struct Reader<'a> {
    buf: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(buf: &'a [u8]) -> Self {
    todo!("RW-01 red")
}

    /// Сколько байт ещё не прочитано.
    fn remaining(&self) -> usize {
    todo!("RW-01 red")
}

    /// Текущий байт без продвижения (для детекта ведущего `0x83`).
    fn peek(&self) -> Result<u8, EtfError> {
    todo!("RW-01 red")
}

    /// Читает `n` байт; при нехватке — `UnexpectedEof` (без `with_capacity(n)`).
    fn take(&mut self, n: usize) -> Result<&'a [u8], EtfError> {
    todo!("RW-01 red")
}

    fn u8(&mut self) -> Result<u8, EtfError> {
    todo!("RW-01 red")
}

    fn u16_be(&mut self) -> Result<u16, EtfError> {
    todo!("RW-01 red")
}

    fn u32_be(&mut self) -> Result<u32, EtfError> {
    todo!("RW-01 red")
}

    fn i32_be(&mut self) -> Result<i32, EtfError> {
    todo!("RW-01 red")
}

    fn f64_be(&mut self) -> Result<f64, EtfError> {
    todo!("RW-01 red")
}

    /// Один терм; `depth` — уровень вложенности (корень — 1).
    fn term(&mut self, depth: usize) -> Result<EtfValue, EtfError> {
    todo!("RW-01 red")
}

    /// `0x63 FLOAT_EXT`: 31 Б ASCII/latin1, значимая часть без хвостовых `\0` и пробелов.
    fn float_ext(&mut self, at: usize) -> Result<EtfValue, EtfError> {
    todo!("RW-01 red")
}

    /// `0x64`/`0x73`/`0x6b`: latin1 — каждый байт становится code point.
    fn latin1(&mut self, len: usize) -> Result<String, EtfError> {
    todo!("RW-01 red")
}

    /// `0x6d`/`0x76`/`0x77`: строгий UTF-8 (иначе `InvalidUtf8`).
    fn utf8(&mut self, len: usize, at: usize) -> Result<String, EtfError> {
    todo!("RW-01 red")
}

    /// `0x68`/`0x69`: кортеж (в трафике — массивы без хвоста).
    fn tuple(&mut self, count: usize, depth: usize, at: usize) -> Result<EtfValue, EtfError> {
    todo!("RW-01 red")
}

    /// `0x6c`: список с обязательным хвостовым `NIL_EXT` (8794/8794 в захвате).
    fn list(&mut self, depth: usize, at: usize) -> Result<EtfValue, EtfError> {
    todo!("RW-01 red")
}

    /// `0x74`: map; порядок пар сохраняется.
    fn map(&mut self, depth: usize, at: usize) -> Result<EtfValue, EtfError> {
    todo!("RW-01 red")
}

    /// `0x6e`/`0x6f`: big-int `[n][sign][n байт LE]` — знак **до** цифр (§2.5 спеки).
    fn big_int(&mut self, digits: usize, at: usize) -> Result<EtfValue, EtfError> {
    todo!("RW-01 red")
}
}

/// Проверка лимита глубины при входе в контейнер (корень — уровень 1).
fn check_depth(depth: usize, at: usize) -> Result<(), EtfError> {
    todo!("RW-01 red")
}

/// Канонический энкод терма (без префикса `0x83`) в буфер.
fn write_term(value: &EtfValue, out: &mut Vec<u8>) {
    todo!("RW-01 red")
}

/// `0x6d`: бинарник — u32 BE длина + UTF-8 (`0x6b`/`0x63` энкодер не производит).
fn write_binary(text: &str, out: &mut Vec<u8>) {
    todo!("RW-01 red")
}

/// Атом: чистый ASCII → `0x64`; иначе ≤255 Б → `0x77`; иначе → `0x76`.
fn write_atom(name: &str, out: &mut Vec<u8>) {
    todo!("RW-01 red")
}

/// Целое: `0..=255` → `0x61`; диапазон i32 → `0x62`; иначе big-int `0x6e` (знак, затем LE-цифры).
fn write_int(number: i64, out: &mut Vec<u8>) {
    todo!("RW-01 red")
}

/// Минимальное число байт для величины (≥1); `u64::MIN.leading_zeros()` даёт 1, а не 0.
fn min_digits(magnitude: u64) -> u8 {
    todo!("RW-01 red")
}
