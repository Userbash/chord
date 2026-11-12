//! X-Super-Properties: 22 поля, JSON и base64 (RW-09 / P1-08, T-REST-04).
//!
//! **Каркас координатора (заморожен):** структура/сигнатуры — контракт; реализация — воркер W9b
//! (`waves/RW-09/TZ.md` §4). Оракулы: `tests/data/rest-super-properties-2323.{json,b64}` (живой
//! заголовок, requestId `95974.2323`), `tests/data/rest-super-properties-22.tsv`,
//! разбор полей — `docs/re/rest-d02.md` §6.2.
//!
//! Реализация: компактная запись канонического порядка `field_names()` с экранированием
//! RFC 8259 и строгий однопроходный разборщик (объект ровно из 22 ключей в любом порядке,
//! `whitespace` между токенами допустим, позиции ошибок — байтовые смещения). Base64-слой
//! делегирует [`super::base64`] (W9a): ошибка декодирования отдаётся как
//! [`SuperPropertiesError::Base64`], не-UTF-8 после декодирования — как
//! [`SuperPropertiesError::Json`].

use super::base64::Base64Error;

/// Число полей супер-свойств (наблюдено живьём: 22).
pub const FIELD_COUNT: usize = 22;

/// Супер-свойства клиента (порядок полей — канонический, как в захвате D-02).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SuperProperties {
    /// ОС клиента (`Linux`).
    pub os: String,
    /// Имя клиента.
    pub browser: String,
    /// Канал обновлений.
    pub release_channel: String,
    /// Версия клиента.
    pub client_version: String,
    /// Версия ядра/ОС.
    pub os_version: String,
    /// Архитектура ОС.
    pub os_arch: String,
    /// Архитектура приложения.
    pub app_arch: String,
    /// Системная локаль.
    pub system_locale: String,
    /// Признак модов клиента.
    pub has_client_mods: bool,
    /// UUID запуска клиента.
    pub client_launch_id: String,
    /// Полный User-Agent.
    pub browser_user_agent: String,
    /// Версия Chromium/Electron-слоя.
    pub browser_version: String,
    /// Оконный менеджер.
    pub window_manager: String,
    /// Дистрибутив (дословно `PRETTY_NAME`).
    pub distro: String,
    /// Среда выполнения.
    pub runtime_environment: String,
    /// Сервер отображения.
    pub display_server: String,
    /// Номер сборки клиента.
    pub client_build_number: u32,
    /// Номер нативной сборки.
    pub native_build_number: u32,
    /// Источник события клиента (`null` в живых данных).
    pub client_event_source: Option<String>,
    /// UUID подписи запуска.
    pub launch_signature: String,
    /// UUID сессии телеметрии.
    pub client_heartbeat_session_id: String,
    /// Состояние окна клиента.
    pub client_app_state: String,
}

/// Ошибка JSON/base64-слоя супер-свойств.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SuperPropertiesError {
    /// Ошибка разбора JSON (сообщение с позицией).
    Json(String),
    /// Ошибка base64-слоя.
    Base64(Base64Error),
}

impl SuperProperties {
    /// Канонический JSON (порядок полей фиксирован, компактный; экранирование — RFC 8259).
    #[must_use]
    pub fn to_json(&self) -> String {
        let mut json = JsonWriter::new();
        json.string("os", &self.os);
        json.string("browser", &self.browser);
        json.string("release_channel", &self.release_channel);
        json.string("client_version", &self.client_version);
        json.string("os_version", &self.os_version);
        json.string("os_arch", &self.os_arch);
        json.string("app_arch", &self.app_arch);
        json.string("system_locale", &self.system_locale);
        json.boolean("has_client_mods", self.has_client_mods);
        json.string("client_launch_id", &self.client_launch_id);
        json.string("browser_user_agent", &self.browser_user_agent);
        json.string("browser_version", &self.browser_version);
        json.string("window_manager", &self.window_manager);
        json.string("distro", &self.distro);
        json.string("runtime_environment", &self.runtime_environment);
        json.string("display_server", &self.display_server);
        json.number("client_build_number", self.client_build_number);
        json.number("native_build_number", self.native_build_number);
        json.optional_string("client_event_source", self.client_event_source.as_deref());
        json.string("launch_signature", &self.launch_signature);
        json.string(
            "client_heartbeat_session_id",
            &self.client_heartbeat_session_id,
        );
        json.string("client_app_state", &self.client_app_state);
        json.finish()
    }

    /// Разбор канонического JSON: объект ровно из 22 ключей в любом порядке, whitespace между
    /// токенами допустим, прочие формы/мусор — [`SuperPropertiesError::Json`] с позицией.
    pub fn from_json(text: &str) -> Result<Self, SuperPropertiesError> {
        Parser::new(text).parse_document()
    }

    /// Канонический base64 от [`SuperProperties::to_json`].
    #[must_use]
    pub fn to_base64(&self) -> String {
        super::base64::encode(self.to_json().as_bytes())
    }

    /// Разбор base64 → JSON → поля: ошибка base64 отдаётся как [`Base64Error`], не-UTF-8 и
    /// ошибки разбора — как [`SuperPropertiesError::Json`] с позицией.
    pub fn from_base64(text: &str) -> Result<Self, SuperPropertiesError> {
        let bytes = super::base64::decode(text).map_err(SuperPropertiesError::Base64)?;
        let json = String::from_utf8(bytes).map_err(|error| {
            let utf8 = error.utf8_error();
            SuperPropertiesError::Json(format!(
                "base64: не UTF-8 (байт {}): {utf8}",
                utf8.valid_up_to()
            ))
        })?;
        Self::from_json(&json)
    }
}

/// Канонический порядок имён полей (индекс = порядок в [`SuperProperties`]).
#[must_use]
pub fn field_names() -> [&'static str; FIELD_COUNT] {
    [
        "os",
        "browser",
        "release_channel",
        "client_version",
        "os_version",
        "os_arch",
        "app_arch",
        "system_locale",
        "has_client_mods",
        "client_launch_id",
        "browser_user_agent",
        "browser_version",
        "window_manager",
        "distro",
        "runtime_environment",
        "display_server",
        "client_build_number",
        "native_build_number",
        "client_event_source",
        "launch_signature",
        "client_heartbeat_session_id",
        "client_app_state",
    ]
}

/// Индекс поля в каноническом порядке (единый источник — [`field_names`]).
fn field_index(name: &str) -> Option<usize> {
    field_names().iter().position(|known| *known == name)
}

/// Hex-цифры нижнего регистра для `\u00XX`.
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

/// Экранирует строку по RFC 8259: `"` → `\"`, `\` → `\\`, управляющие `< 0x20` → короткие
/// формы (`\b \f \n \r \t`) или `\u00XX`; остальной UTF-8 — как есть.
fn write_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0008}' => out.push_str("\\b"),
            '\u{000c}' => out.push_str("\\f"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            ch if (ch as u32) < 0x20 => {
                let code = ch as u32;
                out.push_str("\\u00");
                out.push(HEX_DIGITS[((code >> 4) & 0xF) as usize] as char);
                out.push(HEX_DIGITS[(code & 0xF) as usize] as char);
            }
            ch => out.push(ch),
        }
    }
    out.push('"');
}

/// Писатель компактного JSON-объекта: сам расставляет запятые между членами.
struct JsonWriter {
    /// Готовый JSON (объект уже открыт).
    out: String,
    /// Следующий член — первый (запятая не нужна).
    first: bool,
}

impl JsonWriter {
    /// Открывает пустой объект; закрывает [`JsonWriter::finish`].
    fn new() -> Self {
        let mut out = String::with_capacity(1024);
        out.push('{');
        Self { out, first: true }
    }

    /// Начинает член объекта: `"имя":` (с запятой, если член не первый).
    fn member(&mut self, name: &str) {
        if !self.first {
            self.out.push(',');
        }
        self.first = false;
        write_json_string(&mut self.out, name);
        self.out.push(':');
    }

    /// Член-строка: `"имя":"значение"`.
    fn string(&mut self, name: &str, value: &str) {
        self.member(name);
        write_json_string(&mut self.out, value);
    }

    /// Член-bool: `"имя":true` или `"имя":false`.
    fn boolean(&mut self, name: &str, value: bool) {
        self.member(name);
        self.out.push_str(if value { "true" } else { "false" });
    }

    /// Член-число `u32`: `"имя":123` (без ведущих нулей).
    fn number(&mut self, name: &str, value: u32) {
        self.member(name);
        self.out.push_str(&value.to_string());
    }

    /// Член-строка или `null` (`client_event_source`).
    fn optional_string(&mut self, name: &str, value: Option<&str>) {
        self.member(name);
        match value {
            Some(text) => write_json_string(&mut self.out, text),
            None => self.out.push_str("null"),
        }
    }

    /// Закрывает объект и возвращает готовый JSON.
    fn finish(mut self) -> String {
        self.out.push('}');
        self.out
    }
}

/// Однопроходный разборщик канонического JSON супер-свойств (RFC 8259).
struct Parser<'a> {
    /// Исходный текст (для срезов значений).
    text: &'a str,
    /// Байты текста (ASCII-структура и UTF-8-содержимое).
    bytes: &'a [u8],
    /// Текущее байтовое смещение.
    pos: usize,
}

impl<'a> Parser<'a> {
    /// Создаёт разборщик на начале текста.
    const fn new(text: &'a str) -> Self {
        Self {
            text,
            bytes: text.as_bytes(),
            pos: 0,
        }
    }

    /// Ошибка в текущей позиции.
    fn error(&self, message: impl Into<String>) -> SuperPropertiesError {
        self.error_at(self.pos, message)
    }

    /// Ошибка с указанной байтовой позицией.
    fn error_at(&self, position: usize, message: impl Into<String>) -> SuperPropertiesError {
        SuperPropertiesError::Json(format!("{} (байт {position})", message.into()))
    }

    /// Текущий байт (None — конец текста).
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.pos).copied()
    }

    /// Поглощает ключевое слово (`true`/`false`/`null`); при несовпадении позиция не меняется.
    fn consume_keyword(&mut self, keyword: &[u8]) -> bool {
        if self.bytes[self.pos..].starts_with(keyword) {
            self.pos += keyword.len();
            true
        } else {
            false
        }
    }

    /// Пропускает whitespace JSON: пробел, `\t`, `\n`, `\r`.
    fn skip_whitespace(&mut self) {
        while matches!(self.peek(), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.pos += 1;
        }
    }

    /// Пропускает подряд идущие десятичные цифры.
    fn skip_digits(&mut self) {
        while matches!(self.peek(), Some(b'0'..=b'9')) {
            self.pos += 1;
        }
    }

    /// Разбирает документ: объект ровно с 22 ключами и без мусора после него.
    fn parse_document(&mut self) -> Result<SuperProperties, SuperPropertiesError> {
        self.skip_whitespace();
        if self.peek() != Some(b'{') {
            return Err(self.error("в корне ожидался объект"));
        }
        self.pos += 1;

        let mut properties = SuperProperties::default();
        let mut seen = [false; FIELD_COUNT];
        let mut count = 0_usize;
        self.skip_whitespace();
        if self.peek() == Some(b'}') {
            self.pos += 1;
        } else {
            loop {
                self.skip_whitespace();
                if self.peek() != Some(b'"') {
                    return Err(self.error("ожидался ключ (строка в кавычках)"));
                }
                let key_position = self.pos;
                let key = self.parse_string()?;
                let index = field_index(&key).ok_or_else(|| {
                    self.error_at(key_position, format!("неизвестный ключ {key:?}"))
                })?;
                if seen[index] {
                    return Err(self.error_at(key_position, format!("дубликат ключа {key:?}")));
                }
                self.skip_whitespace();
                if self.peek() != Some(b':') {
                    return Err(self.error("после ключа ожидалось ':'"));
                }
                self.pos += 1;
                self.skip_whitespace();
                self.parse_field(&mut properties, index)?;
                seen[index] = true;
                count += 1;

                self.skip_whitespace();
                match self.peek() {
                    Some(b',') => {
                        self.pos += 1;
                        self.skip_whitespace();
                        if self.peek() == Some(b'}') {
                            return Err(self.error("запятая перед '}'"));
                        }
                    }
                    Some(b'}') => {
                        self.pos += 1;
                        break;
                    }
                    _ => return Err(self.error("ожидалось ',' или '}'")),
                }
            }
        }

        if count != FIELD_COUNT {
            let missing: Vec<&str> = field_names()
                .iter()
                .enumerate()
                .filter_map(|(index, name)| if seen[index] { None } else { Some(*name) })
                .collect();
            return Err(self.error(format!("отсутствуют ключи: {}", missing.join(", "))));
        }
        self.skip_whitespace();
        if self.pos != self.bytes.len() {
            return Err(self.error("мусор после объекта"));
        }
        Ok(properties)
    }

    /// Разбирает значение поля по его каноническому индексу (тип задан полем).
    fn parse_field(
        &mut self,
        properties: &mut SuperProperties,
        index: usize,
    ) -> Result<(), SuperPropertiesError> {
        match index {
            0 => properties.os = self.parse_string()?,
            1 => properties.browser = self.parse_string()?,
            2 => properties.release_channel = self.parse_string()?,
            3 => properties.client_version = self.parse_string()?,
            4 => properties.os_version = self.parse_string()?,
            5 => properties.os_arch = self.parse_string()?,
            6 => properties.app_arch = self.parse_string()?,
            7 => properties.system_locale = self.parse_string()?,
            8 => properties.has_client_mods = self.parse_bool()?,
            9 => properties.client_launch_id = self.parse_string()?,
            10 => properties.browser_user_agent = self.parse_string()?,
            11 => properties.browser_version = self.parse_string()?,
            12 => properties.window_manager = self.parse_string()?,
            13 => properties.distro = self.parse_string()?,
            14 => properties.runtime_environment = self.parse_string()?,
            15 => properties.display_server = self.parse_string()?,
            16 => properties.client_build_number = self.parse_u32()?,
            17 => properties.native_build_number = self.parse_u32()?,
            18 => properties.client_event_source = self.parse_optional_string()?,
            19 => properties.launch_signature = self.parse_string()?,
            20 => properties.client_heartbeat_session_id = self.parse_string()?,
            21 => properties.client_app_state = self.parse_string()?,
            other => {
                return Err(self.error(format!("внутренняя ошибка: неизвестный индекс {other}")));
            }
        }
        Ok(())
    }

    /// Строка в кавычках с escape-последовательностями RFC 8259 (включая `\uXXXX` и
    /// суррогатные пары); сырые управляющие символы отвергаются.
    fn parse_string(&mut self) -> Result<String, SuperPropertiesError> {
        if self.peek() != Some(b'"') {
            return Err(self.error("ожидалась строка (кавычка)"));
        }
        self.pos += 1;
        let mut result = String::new();
        let mut chunk_start = self.pos;
        loop {
            let Some(byte) = self.peek() else {
                return Err(self.error("незакрытая строка"));
            };
            match byte {
                b'"' => {
                    result.push_str(&self.text[chunk_start..self.pos]);
                    self.pos += 1;
                    return Ok(result);
                }
                b'\\' => {
                    result.push_str(&self.text[chunk_start..self.pos]);
                    let escape_position = self.pos;
                    self.pos += 1;
                    let Some(escape) = self.peek() else {
                        return Err(self.error("незакрытая escape-последовательность"));
                    };
                    self.pos += 1;
                    match escape {
                        b'"' => result.push('"'),
                        b'\\' => result.push('\\'),
                        b'/' => result.push('/'),
                        b'b' => result.push('\u{0008}'),
                        b'f' => result.push('\u{000c}'),
                        b'n' => result.push('\n'),
                        b'r' => result.push('\r'),
                        b't' => result.push('\t'),
                        b'u' => result.push(self.parse_unicode_escape(escape_position)?),
                        _ => {
                            return Err(self.error_at(
                                escape_position,
                                format!("недопустимая escape-последовательность: 0x{escape:02x}"),
                            ));
                        }
                    }
                    chunk_start = self.pos;
                }
                0x00..=0x1f => {
                    return Err(self.error("сырой управляющий символ в строке"));
                }
                _ => self.pos += 1,
            }
        }
    }

    /// `\uXXXX` (позиция `u` в `escape_position`) с поддержкой суррогатных пар.
    fn parse_unicode_escape(
        &mut self,
        escape_position: usize,
    ) -> Result<char, SuperPropertiesError> {
        let first = self.parse_hex4(escape_position)?;
        let code_point = if (0xD800..=0xDBFF).contains(&first) {
            if self.peek() != Some(b'\\') {
                return Err(self.error_at(escape_position, "высокий суррогат без пары"));
            }
            self.pos += 1;
            if self.peek() != Some(b'u') {
                return Err(self.error_at(escape_position, "высокий суррогат без пары \\u"));
            }
            self.pos += 1;
            let second = self.parse_hex4(escape_position)?;
            if !(0xDC00..=0xDFFF).contains(&second) {
                return Err(
                    self.error_at(escape_position, "второй код суррогатной пары вне DC00–DFFF")
                );
            }
            0x1_0000 + ((first - 0xD800) << 10) + (second - 0xDC00)
        } else if (0xDC00..=0xDFFF).contains(&first) {
            return Err(self.error_at(escape_position, "низкий суррогат без высокого"));
        } else {
            first
        };
        char::from_u32(code_point).ok_or_else(|| {
            self.error_at(
                escape_position,
                format!("недопустимый код \\u-escape: U+{code_point:04X}"),
            )
        })
    }

    /// Ровно 4 hex-цифры; ошибка указывает на начало `\u`-escape.
    fn parse_hex4(&mut self, escape_position: usize) -> Result<u32, SuperPropertiesError> {
        let mut value = 0_u32;
        for _ in 0..4 {
            let Some(byte) = self.peek() else {
                return Err(self.error_at(escape_position, "\\u-escape: ожидались 4 hex-цифры"));
            };
            let Some(digit) = char::from(byte).to_digit(16) else {
                return Err(self.error("\\u-escape: ожидались 4 hex-цифры"));
            };
            value = value * 16 + digit;
            self.pos += 1;
        }
        Ok(value)
    }

    /// `true`/`false`.
    fn parse_bool(&mut self) -> Result<bool, SuperPropertiesError> {
        if self.consume_keyword(b"true") {
            Ok(true)
        } else if self.consume_keyword(b"false") {
            Ok(false)
        } else {
            Err(self.error("ожидалось true или false"))
        }
    }

    /// `null`.
    fn parse_null(&mut self) -> Result<(), SuperPropertiesError> {
        if self.consume_keyword(b"null") {
            Ok(())
        } else {
            Err(self.error("ожидалось null"))
        }
    }

    /// Строка или `null` (поле `client_event_source`).
    fn parse_optional_string(&mut self) -> Result<Option<String>, SuperPropertiesError> {
        match self.peek() {
            Some(b'"') => Ok(Some(self.parse_string()?)),
            Some(b'n') => {
                self.parse_null()?;
                Ok(None)
            }
            _ => Err(self.error("ожидалась строка или null")),
        }
    }

    /// Число для `u32`: только целое без знака и без ведущих нулей, без переполнения.
    fn parse_u32(&mut self) -> Result<u32, SuperPropertiesError> {
        let start = self.pos;
        let token = self.parse_number_token()?;
        if token.starts_with('-') {
            return Err(self.error_at(
                start,
                format!("{token}: отрицательное число недопустимо для u32"),
            ));
        }
        if token.contains(['.', 'e', 'E']) {
            return Err(self.error_at(
                start,
                format!("{token}: дробное/экспоненциальное число недопустимо для u32"),
            ));
        }
        let mut value = 0_u32;
        for digit in token.bytes() {
            let digit = u32::from(digit - b'0');
            value = value
                .checked_mul(10)
                .and_then(|current| current.checked_add(digit))
                .ok_or_else(|| self.error_at(start, format!("{token}: переполнение u32")))?;
        }
        Ok(value)
    }

    /// Лексема числа по грамматике JSON; возвращает текст токена.
    fn parse_number_token(&mut self) -> Result<&'a str, SuperPropertiesError> {
        let start = self.pos;
        if self.peek() == Some(b'-') {
            self.pos += 1;
        }
        match self.peek() {
            Some(b'0') => {
                self.pos += 1;
                if matches!(self.peek(), Some(b'0'..=b'9')) {
                    return Err(self.error("число с ведущим нулём"));
                }
            }
            Some(b'1'..=b'9') => {
                self.pos += 1;
                self.skip_digits();
            }
            _ => return Err(self.error("ожидалось число")),
        }
        if self.peek() == Some(b'.') {
            self.pos += 1;
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("дробная часть без цифр"));
            }
            self.skip_digits();
        }
        if matches!(self.peek(), Some(b'e' | b'E')) {
            self.pos += 1;
            if matches!(self.peek(), Some(b'+' | b'-')) {
                self.pos += 1;
            }
            if !matches!(self.peek(), Some(b'0'..=b'9')) {
                return Err(self.error("экспонента без цифр"));
            }
            self.skip_digits();
        }
        Ok(&self.text[start..self.pos])
    }
}
