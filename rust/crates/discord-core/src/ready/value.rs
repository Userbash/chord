//! READY: помощники над `EtfValue` (RW-12 / P2-01).
//!
//! Все функции — чистые, без паник: неверный вид значения даёт `None`, а вызывающий решает,
//! является ли это фатальной ошибкой или диагностикой. Снежинки (`ID`) — `u64`; отрицательные
//! big-int старого парсера невалидны (`gateway-protocol.md:105-117`).

use std::collections::HashMap;
use std::hash::Hash;

use discord_codec::etf::EtfValue;

use super::ReadyDiagnostics;

/// Вид ETF-значения для диагностики (`WrongType.found`).
#[must_use]
pub fn kind(value: &EtfValue) -> &'static str {
    match value {
        EtfValue::Nil => "null",
        EtfValue::Atom(_) => "atom",
        EtfValue::Int(_) => "int",
        EtfValue::Float(_) => "float",
        EtfValue::Str(_) => "str",
        EtfValue::Tuple(_) => "tuple",
        EtfValue::List(_) => "list",
        EtfValue::Map(_) => "map",
    }
}

/// Значение map по ключу; ключ — атом или строка.
#[must_use]
pub fn get<'a>(value: &'a EtfValue, key: &str) -> Option<&'a EtfValue> {
    value.get(key)
}

/// `Nil` или атом `"nil"` — «нет значения» (в захвате оба вида обозначают null).
#[must_use]
pub fn is_null(value: &EtfValue) -> bool {
    matches!(value, EtfValue::Nil) || matches!(value, EtfValue::Atom(name) if name == "nil")
}

/// Текст: настоящая строка (`Str`) либо атом-строка, кроме зарезервированных
/// `nil`/`true`/`false` (они обрабатываются как null/bool раньше).
#[must_use]
pub fn text(value: &EtfValue) -> Option<&str> {
    match value {
        EtfValue::Str(text) => Some(text),
        EtfValue::Atom(name) if !matches!(name.as_str(), "nil" | "true" | "false") => Some(name),
        _ => None,
    }
}

/// Строка или отсутствие; `nil`-атом и `null` → `None`.
#[must_use]
pub fn opt_text(value: Option<&EtfValue>) -> Option<String> {
    value.and_then(|value| {
        if is_null(value) {
            None
        } else {
            text(value).map(str::to_string)
        }
    })
}

/// Целое (`Int`), включая отрицательные (для счётчиков/флагов).
#[must_use]
pub fn integer(value: &EtfValue) -> Option<i64> {
    match value {
        EtfValue::Int(number) => Some(*number),
        _ => None,
    }
}

/// Снежинка/беззнаковое: `Int >= 0` либо десятичная строка/атом без знака.
#[must_use]
pub fn unsigned(value: &EtfValue) -> Option<u64> {
    match value {
        EtfValue::Int(number) if *number >= 0 => u64::try_from(*number).ok(),
        EtfValue::Str(text) | EtfValue::Atom(text) => (!text.is_empty()
            && text.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| text.parse::<u64>().ok())
        .flatten(),
        _ => None,
    }
}

/// Bool: атом/строка `"true"`/`"false"` либо `Int 0/1`.
#[must_use]
pub fn boolean(value: &EtfValue) -> Option<bool> {
    match value {
        EtfValue::Atom(name) | EtfValue::Str(name) if name == "true" => Some(true),
        EtfValue::Atom(name) | EtfValue::Str(name) if name == "false" => Some(false),
        EtfValue::Int(1) => Some(true),
        EtfValue::Int(0) => Some(false),
        _ => None,
    }
}

/// Массив (`List`/`Tuple`).
#[must_use]
pub fn items(value: &EtfValue) -> Option<&[EtfValue]> {
    match value {
        EtfValue::List(items) | EtfValue::Tuple(items) => Some(items),
        _ => None,
    }
}

/// Массив; отсутствие/`nil` → пустой срез.
#[must_use]
pub fn items_or_empty(value: Option<&EtfValue>) -> &[EtfValue] {
    value.and_then(items).unwrap_or(&[])
}

/// Map-пары.
#[must_use]
pub fn object(value: &EtfValue) -> Option<&[(EtfValue, EtfValue)]> {
    value.as_map()
}

/// Массив строк (`geo_ordered_rtc_regions`).
#[must_use]
pub fn text_list(value: Option<&EtfValue>) -> Vec<String> {
    items_or_empty(value)
        .iter()
        .filter_map(|item| text(item).map(str::to_string))
        .collect()
}

/// Стабильный dedup: позиция первого вхождения, значение — последнее; дубликаты в диагностику.
pub fn dedup_last_by<T, K: Eq + Hash>(
    items: Vec<T>,
    key: impl Fn(&T) -> K,
    label: impl Fn(&T) -> String,
    collection: &str,
    diagnostics: &mut ReadyDiagnostics,
) -> Vec<T> {
    let mut positions = HashMap::new();
    let mut unique = Vec::with_capacity(items.len());
    for item in items {
        let item_key = key(&item);
        let item_label = label(&item);
        if let Some(&position) = positions.get(&item_key) {
            unique[position] = item;
            diagnostics
                .duplicates
                .push(format!("{collection}:{item_label}"));
        } else {
            positions.insert(item_key, unique.len());
            unique.push(item);
        }
    }
    unique
}
