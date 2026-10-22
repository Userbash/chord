//! HTTP/1.1: кодирование запроса и разбор ответа (RW-10, транспорт REST варианта A).
//!
//! Модуль **чистый**: без I/O и таймеров. Сетевой слой — [`super::transport`].
//!
//! Оракул: RFC 9110 §5 (поля), RFC 9112 §6 (framing), §7 (chunked). Живой захват D-02
//! (`docs/re/rest-d02.md` §3) подтверждает HTTP/1.1 и `Content-Length` у небольших тел.

use super::routes::HttpMethod;

/// Верхняя граница заголовочной части ответа (защита от бесконечного заголовка).
pub const MAX_HEAD_BYTES: usize = 64 * 1024;

/// Верхняя граница строки размера chunk (вместе с расширениями).
pub const MAX_CHUNK_LINE_BYTES: usize = 1024;

/// Верхняя граница блока трейлеров chunked (суммарно).
pub const MAX_TRAILER_BYTES: usize = 8 * 1024;

/// Ошибка HTTP/1.1-фрейминга.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Http1Error {
    /// Данных пока недостаточно; нужен ещё один `read`.
    Incomplete,
    /// Заголовок превысил [`MAX_HEAD_BYTES`] до `\r\n\r\n`.
    HeadTooLarge,
    /// Строка статуса/заголовок не по RFC 9112.
    MalformedHead(String),
    /// Версия протокола не HTTP/1.0 и не HTTP/1.1.
    UnsupportedVersion(String),
    /// Недопустимое имя/значение заголовка (включая CR/LF-инъекцию).
    InvalidHeader(String),
    /// `Content-Length` не число, расходится у повторов или не совпал с телом.
    InvalidContentLength(String),
    /// Одновременно `Content-Length` и любое `Transfer-Encoding`.
    ConflictingFraming,
    /// Неподдерживаемая цепочка transfer-coding (умеем только одиночный `chunked`).
    UnsupportedTransferEncoding(String),
    /// Тело chunked не по RFC 9112 §7.
    MalformedChunk(String),
    /// Тело превысило лимит вызывающего.
    BodyTooLarge,
}

/// Результат разбора заголовка ответа.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HeadParse {
    /// Заголовок разобран; `consumed` — длина вместе с `\r\n\r\n`.
    Complete {
        /// HTTP-статус (200, 204, 429, …).
        status: u16,
        /// Поля в порядке провода; имена сохраняются дословно.
        headers: Vec<(String, String)>,
        /// Сколько байт занял заголовок.
        consumed: usize,
    },
    /// `\r\n\r\n` ещё не найдено; нужен ещё `read`.
    Incomplete,
}

/// Результат разбора chunked-тела.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ChunkedParse {
    /// Тело собрано; `consumed` — длина вместе с завершающим `0\r\n...\r\n`.
    Complete {
        /// Распакованное тело.
        body: Vec<u8>,
        /// Сколько байт заняло chunked-тело.
        consumed: usize,
    },
    /// Завершающего блока ещё нет; нужен ещё `read`.
    Incomplete,
}

/// Поиск поля без учёта регистра имени (первое вхождение).
#[must_use]
pub fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// Разбор `Content-Length`: `None` — поля нет; повторы обязаны совпадать.
pub fn content_length(headers: &[(String, String)]) -> Result<Option<usize>, Http1Error> {
    let mut found: Option<usize> = None;
    for (_, value) in headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case("content-length"))
    {
        let trimmed = value.trim_matches(|c| c == ' ' || c == '\t');
        if trimmed.is_empty() || !trimmed.bytes().all(|byte| byte.is_ascii_digit()) {
            return Err(Http1Error::InvalidContentLength(format!(
                "Content-Length: {value:?}"
            )));
        }
        let parsed = trimmed
            .parse::<usize>()
            .map_err(|_| Http1Error::InvalidContentLength(format!("Content-Length: {value:?}")))?;
        match found {
            Some(previous) if previous != parsed => {
                return Err(Http1Error::InvalidContentLength(format!(
                    "повторы расходятся: {previous} и {parsed}"
                )));
            }
            Some(_) => {}
            None => found = Some(parsed),
        }
    }
    Ok(found)
}

/// Признак `Transfer-Encoding: chunked`; конфликт с `Content-Length` — ошибка.
///
/// RFC 9112 §6.1: `Transfer-Encoding` вместе с `Content-Length` — ошибка framing
/// (независимо от кодировки). Поддерживается только одиночный `chunked`: gzip-декодера нет,
/// а `chunked` обязан быть последним coding'ом.
pub fn is_chunked(headers: &[(String, String)]) -> Result<bool, Http1Error> {
    let tokens: Vec<String> = headers
        .iter()
        .filter(|(key, _)| key.eq_ignore_ascii_case("transfer-encoding"))
        .flat_map(|(_, value)| value.split(','))
        .map(|token| token.trim().to_ascii_lowercase())
        .filter(|token| !token.is_empty())
        .collect();

    if tokens.is_empty() {
        return Ok(false);
    }
    if content_length(headers)?.is_some() {
        return Err(Http1Error::ConflictingFraming);
    }
    if tokens.len() == 1 && tokens[0] == "chunked" {
        return Ok(true);
    }
    Err(Http1Error::UnsupportedTransferEncoding(tokens.join(",")))
}

/// Разбор заголовка ответа из накопленного буфера.
///
/// Неполный буфер (`\r\n\r\n` ещё нет) → [`HeadParse::Incomplete`]; превышение
/// [`MAX_HEAD_BYTES`] → [`Http1Error::HeadTooLarge`].
pub fn parse_response_head(bytes: &[u8]) -> Result<HeadParse, Http1Error> {
    let Some(end) = find_subslice(bytes, b"\r\n\r\n") else {
        return if bytes.len() > MAX_HEAD_BYTES {
            Err(Http1Error::HeadTooLarge)
        } else {
            Ok(HeadParse::Incomplete)
        };
    };

    let consumed = end + 4;
    if consumed > MAX_HEAD_BYTES {
        return Err(Http1Error::HeadTooLarge);
    }

    let head = std::str::from_utf8(&bytes[..end])
        .map_err(|_| Http1Error::MalformedHead("заголовок не UTF-8/ASCII".to_string()))?;
    let mut lines = head.split("\r\n");

    let status_line = lines
        .next()
        .ok_or_else(|| Http1Error::MalformedHead("пустой ответ".to_string()))?;
    let mut parts = status_line.splitn(3, ' ');
    let version = parts
        .next()
        .ok_or_else(|| Http1Error::MalformedHead("нет версии".to_string()))?;
    match version {
        "HTTP/1.1" | "HTTP/1.0" => {}
        other if other.starts_with("HTTP/") => {
            return Err(Http1Error::UnsupportedVersion(other.to_string()));
        }
        _ => {
            return Err(Http1Error::MalformedHead(format!(
                "не HTTP-статус: {status_line:?}"
            )));
        }
    }
    let code = parts
        .next()
        .ok_or_else(|| Http1Error::MalformedHead("нет статуса".to_string()))?;
    if code.len() != 3 || !code.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(Http1Error::MalformedHead(format!("статус {code:?}")));
    }
    let status = code
        .parse::<u16>()
        .map_err(|_| Http1Error::MalformedHead(format!("статус {code:?}")))?;
    if !(100..=599).contains(&status) {
        return Err(Http1Error::MalformedHead(format!("статус {status}")));
    }

    let mut headers = Vec::new();
    for line in lines {
        if line.starts_with(' ') || line.starts_with('\t') {
            return Err(Http1Error::MalformedHead(
                "obs-fold запрещён (RFC 9112 §5.2)".to_string(),
            ));
        }
        let (name, value) = line
            .split_once(':')
            .ok_or_else(|| Http1Error::MalformedHead(format!("поле без двоеточия: {line:?}")))?;
        if !is_token(name) {
            return Err(Http1Error::InvalidHeader(format!("имя поля {name:?}")));
        }
        let value = value.trim_matches(|c| c == ' ' || c == '\t');
        if !is_field_value(value) {
            return Err(Http1Error::MalformedHead(format!(
                "недопустимое значение поля {name:?}"
            )));
        }
        headers.push((name.to_string(), value.to_string()));
    }

    Ok(HeadParse::Complete {
        status,
        headers,
        consumed,
    })
}

/// Разбор chunked-тела из накопленного буфера; лимит `max_body` — распакованное тело.
///
/// Дополнительно ограничены строка размера ([`MAX_CHUNK_LINE_BYTES`]) и блок трейлеров
/// ([`MAX_TRAILER_BYTES`]), чтобы поток без `\r\n` не растил буфер бесконечно.
pub fn decode_chunked(bytes: &[u8], max_body: usize) -> Result<ChunkedParse, Http1Error> {
    let mut position = 0usize;
    let mut body = Vec::new();

    loop {
        let Some(relative_end) = find_subslice(&bytes[position..], b"\r\n") else {
            if bytes.len().saturating_sub(position) > MAX_CHUNK_LINE_BYTES {
                return Err(Http1Error::MalformedChunk(format!(
                    "строка размера > {MAX_CHUNK_LINE_BYTES} Б"
                )));
            }
            return Ok(ChunkedParse::Incomplete);
        };
        let line_end = position + relative_end;
        if line_end.saturating_sub(position) > MAX_CHUNK_LINE_BYTES {
            return Err(Http1Error::MalformedChunk(format!(
                "строка размера > {MAX_CHUNK_LINE_BYTES} Б"
            )));
        }
        let size_field = &bytes[position..line_end];
        let size_text = size_field
            .split(|byte| *byte == b';')
            .next()
            .unwrap_or_default();
        // RFC 9112 §7.1: 1*HEXDIG; `from_str_radix` ошибочно принимает `+5`.
        if size_text.is_empty()
            || size_text.len() > 16
            || !size_text.iter().all(u8::is_ascii_hexdigit)
        {
            return Err(Http1Error::MalformedChunk(format!(
                "размер {:?}",
                String::from_utf8_lossy(size_text)
            )));
        }
        let size = usize::from_str_radix(
            std::str::from_utf8(size_text)
                .map_err(|_| Http1Error::MalformedChunk("размер не ASCII".to_string()))?,
            16,
        )
        .map_err(|_| {
            Http1Error::MalformedChunk(format!("размер {:?}", String::from_utf8_lossy(size_text)))
        })?;

        position = line_end + 2;

        if size == 0 {
            // Трейлеры: строки до пустой; пустая строка завершает блок. Суммарный лимит
            // защищает от потока без завершающего `\r\n\r\n`.
            let trailers_start = position;
            loop {
                if bytes.len().saturating_sub(position) > MAX_TRAILER_BYTES
                    || position.saturating_sub(trailers_start) > MAX_TRAILER_BYTES
                {
                    return Err(Http1Error::MalformedChunk(format!(
                        "трейлеры > {MAX_TRAILER_BYTES} Б"
                    )));
                }
                let Some(relative_end) = find_subslice(&bytes[position..], b"\r\n") else {
                    return Ok(ChunkedParse::Incomplete);
                };
                let end = position + relative_end;
                if relative_end > MAX_CHUNK_LINE_BYTES {
                    return Err(Http1Error::MalformedChunk(format!(
                        "строка трейлера > {MAX_CHUNK_LINE_BYTES} Б"
                    )));
                }
                if end == position {
                    if end + 2 - trailers_start > MAX_TRAILER_BYTES {
                        return Err(Http1Error::MalformedChunk(format!(
                            "трейлеры > {MAX_TRAILER_BYTES} Б"
                        )));
                    }
                    return Ok(ChunkedParse::Complete {
                        body,
                        consumed: position + 2,
                    });
                }
                validate_trailer(&bytes[position..end])?;
                position = end + 2;
            }
        }

        // Лимит проверяется до ожидания данных: объявленный размер уже нарушает контракт.
        if size > max_body.saturating_sub(body.len()) {
            return Err(Http1Error::BodyTooLarge);
        }
        let data_end = position
            .checked_add(size)
            .ok_or_else(|| Http1Error::MalformedChunk("переполнение размера".to_string()))?;
        let framed_end = data_end
            .checked_add(2)
            .ok_or_else(|| Http1Error::MalformedChunk("переполнение размера".to_string()))?;
        if framed_end > bytes.len() {
            return Ok(ChunkedParse::Incomplete);
        }
        body.extend_from_slice(&bytes[position..data_end]);
        if &bytes[data_end..data_end + 2] != b"\r\n" {
            return Err(Http1Error::MalformedChunk(
                "после данных нет CRLF".to_string(),
            ));
        }
        position = data_end + 2;
    }
}

/// Трейлер — обычное поле HTTP (`name: value`); мусор отвергается.
fn validate_trailer(line: &[u8]) -> Result<(), Http1Error> {
    let line = std::str::from_utf8(line)
        .map_err(|_| Http1Error::MalformedChunk("трейлер не UTF-8".to_string()))?;
    let (name, value) = line
        .split_once(':')
        .ok_or_else(|| Http1Error::MalformedChunk(format!("трейлер без ':' {line:?}")))?;
    let value = value.trim_matches(|c| c == ' ' || c == '\t');
    if !is_token(name) || !is_field_value(value) {
        return Err(Http1Error::MalformedChunk(format!(
            "недопустимый трейлер {line:?}"
        )));
    }
    Ok(())
}

/// Кодирование запроса: строка запроса, поля, пустая строка, тело.
///
/// `target` обязан начинаться с `/`; `Content-Length` добавляется автоматически, если тела
/// нет в `headers` (пустое тело — без поля). CR/LF в target/полях → [`Http1Error::InvalidHeader`].
pub fn encode_request(
    method: HttpMethod,
    target: &str,
    headers: &[(String, String)],
    body: &[u8],
) -> Result<Vec<u8>, Http1Error> {
    validate_target(target)?;
    for (name, value) in headers {
        if !is_token(name) {
            return Err(Http1Error::InvalidHeader(format!("имя поля {name:?}")));
        }
        if !is_field_value(value) {
            return Err(Http1Error::InvalidHeader(format!(
                "недопустимое значение поля {name:?}"
            )));
        }
        if name.eq_ignore_ascii_case("transfer-encoding") {
            // Кодировщик умеет только Content-Length; TE в запросе — smuggling-риск.
            return Err(Http1Error::InvalidHeader(
                "Transfer-Encoding в запросе не поддерживается".to_string(),
            ));
        }
    }

    let declared_length = content_length(headers)?;
    if let Some(declared) = declared_length
        && declared != body.len()
    {
        return Err(Http1Error::InvalidContentLength(format!(
            "объявлено {declared}, тела {}",
            body.len()
        )));
    }

    let mut out = Vec::with_capacity(128 + body.len());
    out.extend_from_slice(method.as_str().as_bytes());
    out.push(b' ');
    out.extend_from_slice(target.as_bytes());
    out.extend_from_slice(b" HTTP/1.1\r\n");
    for (name, value) in headers {
        out.extend_from_slice(name.as_bytes());
        out.extend_from_slice(b": ");
        out.extend_from_slice(value.as_bytes());
        out.extend_from_slice(b"\r\n");
    }
    if !body.is_empty() && declared_length.is_none() {
        out.extend_from_slice(format!("Content-Length: {}\r\n", body.len()).as_bytes());
    }
    out.extend_from_slice(b"\r\n");
    out.extend_from_slice(body);
    Ok(out)
}

/// Валидация target: абсолютный путь без fragment/пробелов/управляющих символов.
fn validate_target(target: &str) -> Result<(), Http1Error> {
    if !target.starts_with('/')
        || target
            .bytes()
            .any(|byte| !(0x21..=0x7e).contains(&byte) || byte == b'#')
    {
        return Err(Http1Error::InvalidHeader(format!("target {target:?}")));
    }
    Ok(())
}

/// RFC 9110 §5.5: field-value без CR/LF и управляющих символов, кроме HTAB.
fn is_field_value(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte == b'\t' || (0x20..=0x7e).contains(&byte) || byte >= 0x80)
}

/// RFC 9110 §5.6.2: token = 1*tchar.
fn is_token(name: &str) -> bool {
    const TCHAR: &[u8] = b"!#$%&'*+-.^_`|~";
    !name.is_empty()
        && name
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || TCHAR.contains(&byte))
}

/// Позиция первого вхождения `needle` в `haystack`.
fn find_subslice(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}
