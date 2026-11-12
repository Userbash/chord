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

/// User-Agent клиентской идентичности Discord 1.0.162.
///
/// Оракул: `docs/re/rest-d02.json` auth.superProperties.browser_user_agent
pub const DEFAULT_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.162 Chrome/148.0.7778.280 Electron/42.11.12 Safari/537.36";

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
    /// Неподдерживаемая кодировка содержимого (`Content-Encoding`).
    UnsupportedContentEncoding(String),
    /// Тело chunked не по RFC 9112 §7.
    MalformedChunk(String),
    /// Повреждённое тело ответа при распаковке (gzip/deflate/crc).
    MalformedBody(String),
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

// ---------------------------------------------------------------------------
// Распаковка Content-Encoding: gzip (RFC 1952) и deflate (RFC 1950/1951)
// ---------------------------------------------------------------------------

/// Распаковать тело ответа согласно заголовку `Content-Encoding`.
///
/// Если заголовок отсутствует или равен `identity`, возвращает копию `body`.
/// Поддерживаются `gzip`, `x-gzip`, `deflate` (zlib или raw) и цепочки кодировок (RFC 9110 §8.4).
pub fn decode_response_body(
    headers: &[(String, String)],
    body: &[u8],
) -> Result<Vec<u8>, Http1Error> {
    decode_response_body_with_limit(headers, body, usize::MAX)
}

/// Распаковать тело ответа с ограничением максимального размера `max_body`.
pub fn decode_response_body_with_limit(
    headers: &[(String, String)],
    body: &[u8],
    max_body: usize,
) -> Result<Vec<u8>, Http1Error> {
    let Some(val) = header(headers, "content-encoding") else {
        return Ok(body.to_vec());
    };
    let mut current = body.to_vec();
    let encodings: Vec<&str> = val
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    for enc in encodings.into_iter().rev() {
        let lower = enc.to_ascii_lowercase();
        match lower.as_str() {
            "identity" => {}
            "gzip" | "x-gzip" => {
                current = decode_gzip(&current, max_body)?;
            }
            "deflate" => {
                current = decode_deflate_or_zlib(&current, max_body)?;
            }
            other => {
                return Err(Http1Error::UnsupportedContentEncoding(other.to_string()));
            }
        }
    }
    Ok(current)
}

/// Распаковка gzip-потока (RFC 1952) с проверкой CRC32 и ISIZE.
pub fn decode_gzip(bytes: &[u8], max_body: usize) -> Result<Vec<u8>, Http1Error> {
    if bytes.len() < 18 {
        return Err(Http1Error::MalformedBody(
            "размер gzip меньше 18 байт".to_string(),
        ));
    }
    if bytes[0] != 0x1f || bytes[1] != 0x8b {
        return Err(Http1Error::MalformedBody(
            "неверная сигнатура gzip (ожидалось 0x1f, 0x8b)".to_string(),
        ));
    }
    if bytes[2] != 0x08 {
        return Err(Http1Error::MalformedBody(format!(
            "неподдерживаемый метод сжатия gzip: {}",
            bytes[2]
        )));
    }
    let flags = bytes[3];
    if (flags & 0xe0) != 0 {
        return Err(Http1Error::MalformedBody(
            "установлены зарезервированные флаги gzip".to_string(),
        ));
    }

    let mut pos = 10usize;
    // FEXTRA
    if (flags & 0x04) != 0 {
        if pos + 2 > bytes.len() {
            return Err(Http1Error::MalformedBody("обрыв FEXTRA".to_string()));
        }
        let xlen = (bytes[pos] as usize) | ((bytes[pos + 1] as usize) << 8);
        pos += 2 + xlen;
        if pos > bytes.len() {
            return Err(Http1Error::MalformedBody("обрыв данных FEXTRA".to_string()));
        }
    }
    // FNAME
    if (flags & 0x08) != 0 {
        while pos < bytes.len() && bytes[pos] != 0 {
            pos += 1;
        }
        if pos >= bytes.len() {
            return Err(Http1Error::MalformedBody("обрыв FNAME".to_string()));
        }
        pos += 1;
    }
    // FCOMMENT
    if (flags & 0x10) != 0 {
        while pos < bytes.len() && bytes[pos] != 0 {
            pos += 1;
        }
        if pos >= bytes.len() {
            return Err(Http1Error::MalformedBody("обрыв FCOMMENT".to_string()));
        }
        pos += 1;
    }
    // FHCRC
    if (flags & 0x02) != 0 {
        if pos + 2 > bytes.len() {
            return Err(Http1Error::MalformedBody("обрыв FHCRC".to_string()));
        }
        pos += 2;
    }

    if pos > bytes.len().saturating_sub(8) {
        return Err(Http1Error::MalformedBody(
            "нет данных DEFLATE в gzip".to_string(),
        ));
    }

    let mut reader = BitReader::new(&bytes[pos..]);
    let out = decode_deflate_stream(&mut reader, max_body)?;
    reader.align_byte();

    let footer_start = pos + reader.consumed_bytes();
    if footer_start + 8 > bytes.len() {
        return Err(Http1Error::MalformedBody(
            "нет 8 байт футера gzip".to_string(),
        ));
    }

    let expected_crc =
        u32::from_le_bytes(bytes[footer_start..footer_start + 4].try_into().unwrap());
    let expected_isize = u32::from_le_bytes(
        bytes[footer_start + 4..footer_start + 8]
            .try_into()
            .unwrap(),
    );

    let actual_crc = crc32(&out);
    if actual_crc != expected_crc {
        return Err(Http1Error::MalformedBody(format!(
            "CRC32 не совпал: ожидался {expected_crc:#x}, получен {actual_crc:#x}"
        )));
    }

    let actual_isize = (out.len() & 0xffff_ffff) as u32;
    if actual_isize != expected_isize {
        return Err(Http1Error::MalformedBody(format!(
            "ISIZE не совпал: ожидался {expected_isize}, получен {actual_isize}"
        )));
    }

    Ok(out)
}

/// Распаковка deflate: zlib (RFC 1950) или сырой DEFLATE (RFC 1951).
pub fn decode_deflate_or_zlib(bytes: &[u8], max_body: usize) -> Result<Vec<u8>, Http1Error> {
    if bytes.len() >= 6 {
        let cmf = bytes[0];
        let flg = bytes[1];
        let method = cmf & 0x0f;
        let check = ((cmf as u16) << 8) | (flg as u16);
        if method == 8 && check.is_multiple_of(31) {
            let fdict = (flg & 0x20) != 0;
            let offset = if fdict { 6 } else { 2 };
            if bytes.len() > offset + 4 {
                let mut reader = BitReader::new(&bytes[offset..]);
                if let Ok(out) = decode_deflate_stream(&mut reader, max_body) {
                    reader.align_byte();
                    let footer_offset = offset + reader.consumed_bytes();
                    if bytes.len() >= footer_offset + 4 {
                        let expected_adler = u32::from_be_bytes(
                            bytes[footer_offset..footer_offset + 4].try_into().unwrap(),
                        );
                        if expected_adler == adler32(&out) {
                            return Ok(out);
                        }
                    }
                }
            }
        }
    }
    decode_deflate(bytes, max_body)
}

/// Распаковка чистого сырого DEFLATE (RFC 1951).
pub fn decode_deflate(bytes: &[u8], max_body: usize) -> Result<Vec<u8>, Http1Error> {
    let mut reader = BitReader::new(bytes);
    decode_deflate_stream(&mut reader, max_body)
}

// ---------------------------------------------------------------------------
// Внутренний декодер RFC 1951 (DEFLATE): BitReader, HuffmanTree, LZ77
// ---------------------------------------------------------------------------

struct BitReader<'a> {
    bytes: &'a [u8],
    byte_pos: usize,
    bit_buf: u64,
    bits_in_buf: u8,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            byte_pos: 0,
            bit_buf: 0,
            bits_in_buf: 0,
        }
    }

    fn fill(&mut self, n: u8) {
        while self.bits_in_buf < n && self.byte_pos < self.bytes.len() {
            self.bit_buf |= (self.bytes[self.byte_pos] as u64) << self.bits_in_buf;
            self.byte_pos += 1;
            self.bits_in_buf += 8;
        }
    }

    fn read_bits(&mut self, n: u8) -> Result<u32, Http1Error> {
        if n == 0 {
            return Ok(0);
        }
        self.fill(n);
        if self.bits_in_buf < n {
            return Err(Http1Error::MalformedBody(
                "неожиданный конец сжатого потока".to_string(),
            ));
        }
        let val = (self.bit_buf & ((1u64 << n) - 1)) as u32;
        self.bit_buf >>= n;
        self.bits_in_buf -= n;
        Ok(val)
    }

    fn align_byte(&mut self) {
        let rem = self.bits_in_buf % 8;
        if rem > 0 {
            self.bit_buf >>= rem;
            self.bits_in_buf -= rem;
        }
    }

    fn read_byte(&mut self) -> Result<u8, Http1Error> {
        self.read_bits(8).map(|b| b as u8)
    }

    fn consumed_bytes(&self) -> usize {
        self.byte_pos - (self.bits_in_buf / 8) as usize
    }
}

#[derive(Debug, Clone)]
struct HuffmanTree {
    nodes: Vec<[u16; 2]>,
    symbols: Vec<u16>,
}

impl HuffmanTree {
    fn new() -> Self {
        Self {
            nodes: vec![[0, 0]],
            symbols: vec![u16::MAX],
        }
    }

    fn insert(&mut self, code: u16, len: u8, symbol: u16) -> Result<(), Http1Error> {
        let mut curr = 0usize;
        for i in (0..len).rev() {
            let bit = ((code >> i) & 1) as usize;
            let mut next = self.nodes[curr][bit] as usize;
            if next == 0 {
                next = self.nodes.len();
                if next >= u16::MAX as usize {
                    return Err(Http1Error::MalformedBody(
                        "дерево Хаффмана переполнено".to_string(),
                    ));
                }
                self.nodes.push([0, 0]);
                self.symbols.push(u16::MAX);
                self.nodes[curr][bit] = next as u16;
            }
            curr = next;
        }
        self.symbols[curr] = symbol;
        Ok(())
    }

    fn build(lengths: &[u8]) -> Result<Self, Http1Error> {
        let mut max_len = 0;
        for &l in lengths {
            if l > max_len {
                max_len = l;
            }
        }
        if max_len == 0 {
            return Ok(Self::new());
        }
        let mut bl_count = vec![0u16; (max_len + 1) as usize];
        for &l in lengths {
            if l > 0 {
                bl_count[l as usize] += 1;
            }
        }
        let mut code = 0u16;
        let mut next_code = vec![0u16; (max_len + 1) as usize];
        for bits in 1..=max_len as usize {
            code = (code + bl_count[bits - 1]) << 1;
            next_code[bits] = code;
        }
        let mut tree = Self::new();
        for (sym, &len) in lengths.iter().enumerate() {
            if len != 0 {
                let c = next_code[len as usize];
                next_code[len as usize] += 1;
                tree.insert(c, len, sym as u16)?;
            }
        }
        Ok(tree)
    }

    fn decode(&self, reader: &mut BitReader<'_>) -> Result<u16, Http1Error> {
        let mut curr = 0usize;
        loop {
            let sym = self.symbols[curr];
            if sym != u16::MAX {
                return Ok(sym);
            }
            let bit = reader.read_bits(1)? as usize;
            let next = self.nodes[curr][bit] as usize;
            if next == 0 {
                return Err(Http1Error::MalformedBody(
                    "недопустимый код Хаффмана".to_string(),
                ));
            }
            curr = next;
        }
    }
}

fn fixed_trees() -> (HuffmanTree, HuffmanTree) {
    let mut lit_lens = [0u8; 288];
    lit_lens[0..=143].fill(8);
    lit_lens[144..=255].fill(9);
    lit_lens[256..=279].fill(7);
    lit_lens[280..=287].fill(8);
    let lit_tree = HuffmanTree::build(&lit_lens).expect("fixed lit");

    let dist_lens = [5u8; 32];
    let dist_tree = HuffmanTree::build(&dist_lens).expect("fixed dist");

    (lit_tree, dist_tree)
}

const CL_ORDER: [usize; 19] = [
    16, 17, 18, 0, 8, 7, 9, 6, 10, 5, 11, 4, 12, 3, 13, 2, 14, 1, 15,
];

fn read_dynamic_trees(
    reader: &mut BitReader<'_>,
) -> Result<(HuffmanTree, HuffmanTree), Http1Error> {
    let hlit = reader.read_bits(5)? + 257;
    let hdist = reader.read_bits(5)? + 1;
    let hclen = reader.read_bits(4)? + 4;

    let mut cl_lengths = [0u8; 19];
    for i in 0..hclen {
        cl_lengths[CL_ORDER[i as usize]] = reader.read_bits(3)? as u8;
    }
    let cl_tree = HuffmanTree::build(&cl_lengths)?;

    let total_codes = (hlit + hdist) as usize;
    let mut code_lengths = Vec::with_capacity(total_codes);
    while code_lengths.len() < total_codes {
        let sym = cl_tree.decode(reader)?;
        match sym {
            0..=15 => {
                code_lengths.push(sym as u8);
            }
            16 => {
                if code_lengths.is_empty() {
                    return Err(Http1Error::MalformedBody(
                        "повтор без предыдущей длины".to_string(),
                    ));
                }
                let prev = *code_lengths.last().unwrap();
                let repeat = 3 + reader.read_bits(2)? as usize;
                if code_lengths.len() + repeat > total_codes {
                    return Err(Http1Error::MalformedBody(
                        "переполнение списка длин Хаффмана".to_string(),
                    ));
                }
                code_lengths.extend(std::iter::repeat_n(prev, repeat));
            }
            17 => {
                let repeat = 3 + reader.read_bits(3)? as usize;
                if code_lengths.len() + repeat > total_codes {
                    return Err(Http1Error::MalformedBody(
                        "переполнение списка длин Хаффмана".to_string(),
                    ));
                }
                code_lengths.extend(std::iter::repeat_n(0, repeat));
            }
            18 => {
                let repeat = 11 + reader.read_bits(7)? as usize;
                if code_lengths.len() + repeat > total_codes {
                    return Err(Http1Error::MalformedBody(
                        "переполнение списка длин Хаффмана".to_string(),
                    ));
                }
                code_lengths.extend(std::iter::repeat_n(0, repeat));
            }
            _ => {
                return Err(Http1Error::MalformedBody(
                    "недопустимый символ длины кода".to_string(),
                ));
            }
        }
    }

    let lit_tree = HuffmanTree::build(&code_lengths[..hlit as usize])?;
    let dist_tree = HuffmanTree::build(&code_lengths[hlit as usize..])?;
    Ok((lit_tree, dist_tree))
}

const LEN_BASE: [u16; 29] = [
    3, 4, 5, 6, 7, 8, 9, 10, 11, 13, 15, 17, 19, 23, 27, 31, 35, 43, 51, 59, 67, 83, 99, 115, 131,
    163, 195, 227, 258,
];
const LEN_EXTRA: [u8; 29] = [
    0, 0, 0, 0, 0, 0, 0, 0, 1, 1, 1, 1, 2, 2, 2, 2, 3, 3, 3, 3, 4, 4, 4, 4, 5, 5, 5, 5, 0,
];

const DIST_BASE: [u16; 30] = [
    1, 2, 3, 4, 5, 7, 9, 13, 17, 25, 33, 49, 65, 97, 129, 193, 257, 385, 513, 769, 1025, 1537,
    2049, 3073, 4097, 6145, 8193, 12289, 16385, 24577,
];
const DIST_EXTRA: [u8; 30] = [
    0, 0, 0, 0, 1, 1, 2, 2, 3, 3, 4, 4, 5, 5, 6, 6, 7, 7, 8, 8, 9, 9, 10, 10, 11, 11, 12, 12, 13,
    13,
];

fn decode_block(
    reader: &mut BitReader<'_>,
    lit_tree: &HuffmanTree,
    dist_tree: &HuffmanTree,
    out: &mut Vec<u8>,
    max_body: usize,
) -> Result<(), Http1Error> {
    loop {
        let sym = lit_tree.decode(reader)?;
        match sym {
            0..=255 => {
                if out.len() >= max_body {
                    return Err(Http1Error::BodyTooLarge);
                }
                out.push(sym as u8);
            }
            256 => break,
            257..=285 => {
                let len_idx = (sym - 257) as usize;
                let extra_bits = LEN_EXTRA[len_idx];
                let length = LEN_BASE[len_idx] as usize + reader.read_bits(extra_bits)? as usize;

                let dist_sym = dist_tree.decode(reader)?;
                if dist_sym as usize >= DIST_BASE.len() {
                    return Err(Http1Error::MalformedBody(
                        "недопустимый код дистанции".to_string(),
                    ));
                }
                let dist_idx = dist_sym as usize;
                let dist_extra = DIST_EXTRA[dist_idx];
                let distance =
                    DIST_BASE[dist_idx] as usize + reader.read_bits(dist_extra)? as usize;

                if distance > out.len() || distance == 0 {
                    return Err(Http1Error::MalformedBody(
                        "дистанция за пределами буфера".to_string(),
                    ));
                }
                if out.len() + length > max_body {
                    return Err(Http1Error::BodyTooLarge);
                }
                let start = out.len() - distance;
                for i in 0..length {
                    let b = out[start + i];
                    out.push(b);
                }
            }
            _ => {
                return Err(Http1Error::MalformedBody(
                    "недопустимый символ литерала/длины".to_string(),
                ));
            }
        }
    }
    Ok(())
}

fn decode_uncompressed(
    reader: &mut BitReader<'_>,
    out: &mut Vec<u8>,
    max_body: usize,
) -> Result<(), Http1Error> {
    reader.align_byte();
    let b0 = reader.read_byte()?;
    let b1 = reader.read_byte()?;
    let len = (b0 as u16) | ((b1 as u16) << 8);

    let n0 = reader.read_byte()?;
    let n1 = reader.read_byte()?;
    let nlen = (n0 as u16) | ((n1 as u16) << 8);

    if len != !nlen {
        return Err(Http1Error::MalformedBody(
            "LEN и NLEN не совпадают".to_string(),
        ));
    }
    let len = len as usize;
    if out.len() + len > max_body {
        return Err(Http1Error::BodyTooLarge);
    }
    for _ in 0..len {
        out.push(reader.read_byte()?);
    }
    Ok(())
}

fn decode_deflate_stream(
    reader: &mut BitReader<'_>,
    max_body: usize,
) -> Result<Vec<u8>, Http1Error> {
    let mut out = Vec::new();
    let (fixed_lit, fixed_dist) = fixed_trees();
    let mut bfinal = false;

    while !bfinal {
        bfinal = reader.read_bits(1)? != 0;
        let btype = reader.read_bits(2)?;
        match btype {
            0 => decode_uncompressed(reader, &mut out, max_body)?,
            1 => decode_block(reader, &fixed_lit, &fixed_dist, &mut out, max_body)?,
            2 => {
                let (dyn_lit, dyn_dist) = read_dynamic_trees(reader)?;
                decode_block(reader, &dyn_lit, &dyn_dist, &mut out, max_body)?;
            }
            3 => {
                return Err(Http1Error::MalformedBody(
                    "BTYPE = 3 зарезервирован".to_string(),
                ));
            }
            _ => unreachable!(),
        }
    }
    Ok(out)
}

const fn make_crc_table() -> [u32; 256] {
    let mut table = [0u32; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ 0xedb8_8320;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

const CRC_TABLE: [u32; 256] = make_crc_table();

/// Вычисление CRC32 по алгоритму IEEE 802.3 (полином 0xEDB88320).
pub fn crc32(bytes: &[u8]) -> u32 {
    let mut crc = 0xffff_ffffu32;
    for &b in bytes {
        crc = CRC_TABLE[((crc ^ (b as u32)) & 0xff) as usize] ^ (crc >> 8);
    }
    !crc
}

fn adler32(bytes: &[u8]) -> u32 {
    let mut s1 = 1u32;
    let mut s2 = 0u32;
    for &b in bytes {
        s1 = (s1 + b as u32) % 65521;
        s2 = (s2 + s1) % 65521;
    }
    (s2 << 16) | s1
}
