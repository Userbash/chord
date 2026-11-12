//! Base64 (RFC 4648) для супер-свойств (RW-09 / P1-08, T-REST-04).
//!
//! **Каркас координатора (заморожен):** типы и сигнатуры — контракт; реализация — воркер W9a
//! (`waves/RW-09/TZ.md` §3). Оракулы: `tests/data/rest-base64-vectors.tsv` (векторы python3)
//! и живой заголовок `tests/data/rest-super-properties-2323.b64` (requestId `95974.2323`).
//!
//! Декодер намеренно **не строгий к неканоничности**: ненулевые хвостовые биты последней
//! группы отбрасываются (`"Zh=="` → `b"f"`), а [`encode`] от результата даёт каноническую форму
//! (`"Zg=="`) — это кейс B5 ТЗ §4. Позиции в ошибках — байтовые смещения во входной строке
//! (`&str`), а не номера символов Unicode.

/// Алфавит RFC 4648 §4 (стандартный, `+` и `/`).
const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// Ошибка декодирования base64.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Base64Error {
    /// Длина строки (в байтах) не кратна 4.
    InvalidLength(usize),
    /// Символ вне алфавита `A-Za-z0-9+/` (и не `=` в допустимой позиции).
    InvalidChar { byte: u8, index: usize },
    /// `=` не в хвосте, больше двух или противоречит длине данных.
    InvalidPadding { index: usize },
}

/// Значение sextet для символа данных: `A-Za-z0-9+/`; `None` — вне алфавита (`=` не входит).
const fn sextet(byte: u8) -> Option<u8> {
    match byte {
        b'A'..=b'Z' => Some(byte - b'A'),
        b'a'..=b'z' => Some(byte - b'a' + 26),
        b'0'..=b'9' => Some(byte - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    }
}

/// Добавляет в вывод символ алфавита для 6-битного значения (`0..=63`).
fn push_sextet(out: &mut String, value: u8) {
    out.push(char::from(ALPHABET[usize::from(value)]));
}

/// Кодирует в канонический base64 с `=`-паддингом (RFC 4648 §4).
#[must_use]
pub fn encode(data: &[u8]) -> String {
    let mut out = String::with_capacity(data.len().div_ceil(3) * 4);
    for chunk in data.chunks(3) {
        let first = chunk[0];
        let second = chunk.get(1).copied().unwrap_or(0);
        let third = chunk.get(2).copied().unwrap_or(0);
        push_sextet(&mut out, first >> 2);
        push_sextet(&mut out, ((first & 0x03) << 4) | (second >> 4));
        if chunk.len() == 1 {
            out.push_str("==");
            continue;
        }
        push_sextet(&mut out, ((second & 0x0f) << 2) | (third >> 6));
        if chunk.len() == 2 {
            out.push('=');
        } else {
            push_sextet(&mut out, third & 0x3f);
        }
    }
    out
}

/// Длина данных до хвостового ряда `=`: проверяет паддинг, возвращает позицию первого `=`
/// (или длину строки, если паддинга нет). Ошибка указывает на первый `=`.
///
/// `=` допустим только хвостовым рядом из 0–2 символов; `data_chars % 4 == 1` несовместимо
/// с паддингом (1 секстет — меньше байта).
fn data_length(bytes: &[u8]) -> Result<usize, Base64Error> {
    let Some(first_pad) = bytes.iter().position(|&byte| byte == b'=') else {
        return Ok(bytes.len());
    };
    let pad_len = bytes.len() - first_pad;
    let tail_has_data = bytes[first_pad..].iter().any(|&byte| byte != b'=');
    if pad_len > 2 || tail_has_data || first_pad % 4 == 1 {
        return Err(Base64Error::InvalidPadding { index: first_pad });
    }
    Ok(first_pad)
}

/// Декодирует base64 (RFC 4648 §4) с типизированными ошибками.
///
/// Порядок проверок: `=`-паддинг → символы данных → длина всей строки. Паддинг и символы
/// проверяются до длины, потому что тест B3 ТЗ §4 требует `InvalidChar` для `"Zm9v\n"`
/// (недопустимый символ в 5-байтной строке важнее её некратной длины); для строки только из
/// алфавитных символов некратная длина даёт [`Base64Error::InvalidLength`] (`"AA"` → 2).
///
/// Ненулевые хвостовые биты последней группы игнорируются: `"Zh=="` → `b"f"` (B5).
pub fn decode(text: &str) -> Result<Vec<u8>, Base64Error> {
    let bytes = text.as_bytes();

    // 1. Паддинг: '=' допустим только хвостовым рядом из 0–2 символов.
    let data_len = data_length(bytes)?;

    // 2. Символы данных — только алфавит; '=' уже разобран выше. Алфавит и декодирование
    //    идут одним проходом, но недопустимый символ возвращается раньше проверки длины (B3).
    let mut out = Vec::with_capacity(data_len.div_ceil(4) * 3);
    let mut accum = 0_u32;
    let mut sextets = 0_u8;
    for (index, &byte) in bytes[..data_len].iter().enumerate() {
        let Some(value) = sextet(byte) else {
            return Err(Base64Error::InvalidChar { byte, index });
        };
        accum = accum << 6 | u32::from(value);
        sextets += 1;
        if sextets == 4 {
            out.extend_from_slice(&accum.to_be_bytes()[1..]);
            accum = 0;
            sextets = 0;
        }
    }

    // 3. Длина всей строки кратна 4 (каноническая запись).
    if !bytes.len().is_multiple_of(4) {
        return Err(Base64Error::InvalidLength(bytes.len()));
    }

    // 4. Хвост последней группы: 2 секстета → 1 байт, 3 → 2 байта; лишние биты отбрасываются.
    match sextets {
        0 => {}
        2 => out.push((accum >> 4) as u8),
        3 => {
            out.push((accum >> 10) as u8);
            out.push((accum >> 2) as u8);
        }
        // 1 секстет невозможен: при отсутствии паддинга длина кратна 4, а паддинг
        // не допускает `data_chars % 4 == 1`; ветка защитная и типизированная.
        _ => return Err(Base64Error::InvalidLength(bytes.len())),
    }
    Ok(out)
}
