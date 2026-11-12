//! JSON → `EtfValue` адаптер для READY-фикстур (RW-12 / P2-01, precondition P-1).
//!
//! Оракул самого адаптера — реальный `fixtures/gateway/by-type/002-READY.json` и числа jq из
//! `waves/RW-12/DESIGN-P2-01.md` §2; проверка — `tests/ready_fixture.rs`.
//!
//! Соответствие форме захвата (`docs/re/gateway-protocol.md` §2):
//! - JSON `null` → [`EtfValue::Nil`] (`NIL_EXT`);
//! - строки `"nil"`/`"true"`/`"false"` → [`EtfValue::Atom`] (в захвате это атомы `0x64`, не строки);
//! - остальные строки → [`EtfValue::Str`]; десятичные строки снежинок остаются `Str` —
//!   [`chord_core::ready::value::unsigned`] разбирает их как ID;
//! - объекты → `Map` с ключами-`Atom`, массивы → `List`, целые → `Int`, дробные → `Float`.
//!
//! Оговорка: JSON не различает атом `"true"` и строку `"true"`; адаптер выбирает форму захвата.
//! Это тестовый помощник, не продакшн-парсер; глубина вложенности фикстуры мала.

use std::path::PathBuf;

use chord_codec::etf::EtfValue;

/// Имя READY-фикстуры в `fixtures/gateway/by-type/`.
pub const READY_FIXTURE: &str = "002-READY.json";

/// Путь к курированной фикстуре `fixtures/gateway/by-type/<name>`.
#[must_use]
pub fn fixture_path(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../..")
        .join("fixtures/gateway/by-type")
        .join(name)
}

/// Прочитать и разобрать фикстуру `fixtures/gateway/by-type/<name>`.
pub fn load(name: &str) -> Result<EtfValue, String> {
    let path = fixture_path(name);
    let text = std::fs::read_to_string(&path)
        .map_err(|error| format!("фикстура {}: {error}", path.display()))?;
    parse_json(&text).map_err(|error| format!("{}: {error}", path.display()))
}

/// Прочитать и разобрать реальный READY (`002-READY.json`).
pub fn load_ready() -> Result<EtfValue, String> {
    load(READY_FIXTURE)
}

/// Разобрать JSON-текст в `EtfValue` (строгий парсер без внешних зависимостей).
pub fn parse_json(input: &str) -> Result<EtfValue, String> {
    let mut parser = Parser::new(input);
    parser.skip_ws();
    let value = parser.value()?;
    parser.skip_ws();
    if parser.pos != parser.bytes.len() {
        return Err(parser.error("лишние байты после JSON"));
    }
    Ok(value)
}

struct Parser<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl<'a> Parser<'a> {
    fn new(input: &'a str) -> Self {
        Self {
            bytes: input.as_bytes(),
            pos: 0,
        }
    }

    fn error(&self, message: impl Into<String>) -> String {
        format!("{} (байт {})", message.into(), self.pos)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    fn bump(&mut self) -> Option<u8> {
        let byte = self.peek()?;
        self.pos += 1;
        Some(byte)
    }

    fn skip_ws(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, byte: u8, what: &str) -> Result<(), String> {
        if self.peek() == Some(byte) {
            self.pos += 1;
            Ok(())
        } else {
            Err(self.error(format!("ожидалось {what}")))
        }
    }

    fn literal(&mut self, text: &str) -> Result<(), String> {
        if self.bytes[self.pos..].starts_with(text.as_bytes()) {
            self.pos += text.len();
            Ok(())
        } else {
            Err(self.error(format!("ожидалось {text:?}")))
        }
    }

    fn value(&mut self) -> Result<EtfValue, String> {
        match self.peek() {
            Some(b'{') => self.object(),
            Some(b'[') => self.array(),
            Some(b'"') => {
                let text = self.string()?;
                Ok(match text.as_str() {
                    "nil" | "true" | "false" => EtfValue::Atom(text),
                    _ => EtfValue::Str(text),
                })
            }
            Some(b't') => {
                self.literal("true")?;
                Ok(EtfValue::Atom("true".to_string()))
            }
            Some(b'f') => {
                self.literal("false")?;
                Ok(EtfValue::Atom("false".to_string()))
            }
            Some(b'n') => {
                self.literal("null")?;
                Ok(EtfValue::Nil)
            }
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(_) => Err(self.error("неожиданный символ")),
            None => Err(self.error("неожиданный конец JSON")),
        }
    }

    fn object(&mut self) -> Result<EtfValue, String> {
        self.expect(b'{', "начало объекта")?;
        self.skip_ws();
        let mut pairs = Vec::new();
        if self.peek() == Some(b'}') {
            self.pos += 1;
            return Ok(EtfValue::Map(pairs));
        }
        loop {
            self.skip_ws();
            let key = self.string().map_err(|error| format!("ключ: {error}"))?;
            self.skip_ws();
            self.expect(b':', "двоеточие")?;
            self.skip_ws();
            let value = self.value()?;
            pairs.push((EtfValue::Atom(key), value));
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b'}') => break,
                _ => return Err(self.error("ожидалась ',' или '}'")),
            }
        }
        Ok(EtfValue::Map(pairs))
    }

    fn array(&mut self) -> Result<EtfValue, String> {
        self.expect(b'[', "начало массива")?;
        self.skip_ws();
        let mut items = Vec::new();
        if self.peek() == Some(b']') {
            self.pos += 1;
            return Ok(EtfValue::List(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value()?);
            self.skip_ws();
            match self.bump() {
                Some(b',') => continue,
                Some(b']') => break,
                _ => return Err(self.error("ожидалась ',' или ']'")),
            }
        }
        Ok(EtfValue::List(items))
    }

    fn string(&mut self) -> Result<String, String> {
        self.expect(b'"', "начало строки")?;
        let mut out: Vec<u8> = Vec::new();
        loop {
            let byte = self.bump().ok_or_else(|| self.error("незакрытая строка"))?;
            match byte {
                b'"' => break,
                b'\\' => self.escape(&mut out)?,
                control if control < 0x20 => {
                    return Err(self.error("управляющий символ в строке"));
                }
                byte => out.push(byte),
            }
        }
        String::from_utf8(out).map_err(|_| self.error("строка не UTF-8"))
    }

    fn escape(&mut self, out: &mut Vec<u8>) -> Result<(), String> {
        let escape = self.bump().ok_or_else(|| self.error("обрыв escape"))?;
        match escape {
            b'"' => out.push(b'"'),
            b'\\' => out.push(b'\\'),
            b'/' => out.push(b'/'),
            b'b' => out.push(0x08),
            b'f' => out.push(0x0c),
            b'n' => out.push(b'\n'),
            b'r' => out.push(b'\r'),
            b't' => out.push(b'\t'),
            b'u' => {
                let first = self.hex4()?;
                let scalar = if (0xD800..=0xDBFF).contains(&first) {
                    self.expect(b'\\', "второй surrogate")?;
                    self.expect(b'u', "второй surrogate")?;
                    let second = self.hex4()?;
                    if !(0xDC00..=0xDFFF).contains(&second) {
                        return Err(self.error("невалидная пара surrogate"));
                    }
                    0x1_0000 + ((first - 0xD800) << 10) + (second - 0xDC00)
                } else {
                    first
                };
                let scalar =
                    char::from_u32(scalar).ok_or_else(|| self.error("невалидный код в \\u"))?;
                let mut buffer = [0u8; 4];
                out.extend_from_slice(scalar.encode_utf8(&mut buffer).as_bytes());
            }
            other => {
                return Err(self.error(format!("неизвестный escape \\{}", char::from(other))));
            }
        }
        Ok(())
    }

    fn hex4(&mut self) -> Result<u32, String> {
        let mut value = 0u32;
        for _ in 0..4 {
            let byte = self.bump().ok_or_else(|| self.error("обрыв \\u"))?;
            let digit = u32::from(byte)
                .checked_sub(u32::from(b'0'))
                .filter(|digit| *digit < 10)
                .or_else(|| {
                    u32::from(byte.to_ascii_lowercase())
                        .checked_sub(u32::from(b'a'))
                        .filter(|digit| *digit < 6)
                        .map(|digit| digit + 10)
                })
                .ok_or_else(|| self.error("не hex-цифра в \\u"))?;
            value = value * 16 + digit;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<EtfValue, String> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        let integer_start = self.pos;
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
        if self.pos == integer_start {
            return Err(self.error("ожидалась цифра"));
        }
        let mut is_float = false;
        if self.peek() == Some(b'.') {
            is_float = true;
            self.pos += 1;
            let fraction_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if self.pos == fraction_start {
                return Err(self.error("ожидались цифры после '.'"));
            }
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            is_float = true;
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            let exponent_start = self.pos;
            while matches!(self.peek(), Some(b'0'..=b'9')) {
                self.pos += 1;
            }
            if self.pos == exponent_start {
                return Err(self.error("ожидались цифры экспоненты"));
            }
        }
        let text = std::str::from_utf8(&self.bytes[start..self.pos])
            .map_err(|_| self.error("число не UTF-8"))?;
        if is_float {
            text.parse::<f64>()
                .map(EtfValue::Float)
                .map_err(|_| self.error("невалидное число"))
        } else {
            match text.parse::<i64>() {
                Ok(value) => Ok(EtfValue::Int(value)),
                // Big-int вне i64 в JSON пришёл бы десятичной строкой; сохраняем её как Str,
                // `ready::value::unsigned` разберёт такие ID.
                Err(_) => Ok(EtfValue::Str(text.to_string())),
            }
        }
    }
}
