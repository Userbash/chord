//! REST v9: ack/read-state — target и тело `{token,last_viewed}` (RW-11 / P1-12,
//! T-REST-07).
//!
//! Оракул: `docs/re/rest-d02.md` §4 — `POST /channels/{id}/messages/{id}/ack` ×5,
//! тело `{"token":null,"last_viewed":4310}` (requestId `ready|57992.7577`); `token` бывает
//! `null`, `last_viewed` — номер сообщения (JSON-число).
//!
//! JSON собирается вручную, без новых зависимостей: ключи идут в порядке оракула
//! (`token`, затем `last_viewed`), `last_viewed` — число без кавычек, токен экранируется по
//! RFC 8259 §7 (управляющие U+0000…U+001F → `\u00XX` в нижнем регистре).

/// Шаблон пути ack из каталога D-02.
pub const ACK_PATH_TEMPLATE: &str = "/channels/{id}/messages/{id}/ack";

/// Ошибка построения ack-запроса.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AckError {
    /// Пустой id канала/сообщения.
    EmptyId,
    /// id не snowflake (не ASCII-цифры) — защита от подстановки в путь.
    NonNumericId,
}

/// Path ack-запроса с подставленными snowflake-идами.
///
/// Иды проверяются до подстановки: пустой — [`AckError::EmptyId`], не состоящий только из
/// ASCII-цифр — [`AckError::NonNumericId`] (защита от `..`, `+1`, пробелов и т.п.).
///
/// # Errors
///
/// [`AckError::EmptyId`] — пустой `channel_id` или `message_id`;
/// [`AckError::NonNumericId`] — id содержит не только ASCII-цифры.
pub fn build_ack_target(channel_id: &str, message_id: &str) -> Result<String, AckError> {
    validate_id(channel_id)?;
    validate_id(message_id)?;
    Ok(ACK_PATH_TEMPLATE
        .replacen("{id}", channel_id, 1)
        .replacen("{id}", message_id, 1))
}

/// Тело ack: `{"token":<null|string>,"last_viewed":<u64>}` с JSON-экранированием токена.
#[must_use]
pub fn build_ack_body(token: Option<&str>, last_viewed: u64) -> String {
    let mut body = String::with_capacity(48 + token.map_or(0, str::len));
    body.push_str("{\"token\":");
    match token {
        Some(token) => push_json_string(&mut body, token),
        None => body.push_str("null"),
    }
    body.push_str(",\"last_viewed\":");
    body.push_str(&last_viewed.to_string());
    body.push('}');
    body
}

/// Проверка id: непустые ASCII-цифры (снежинки).
fn validate_id(id: &str) -> Result<(), AckError> {
    if id.is_empty() {
        return Err(AckError::EmptyId);
    }
    if !id.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(AckError::NonNumericId);
    }
    Ok(())
}

/// Шестнадцатеричные цифры нижнего регистра для `\u00XX`.
const HEX_LOWER: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

/// Дописывает `value` как JSON-строку в кавычках с экранированием RFC 8259 §7.
///
/// `"` → `\"`, `\` → `\\`, управляющие U+0000…U+001F → `\u00XX` (hex в нижнем регистре);
/// остальные символы (включая UTF-8) копируются как есть.
fn push_json_string(out: &mut String, value: &str) {
    out.push('"');
    for ch in value.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\u{0000}'..='\u{001f}' => {
                let code = ch as u32;
                out.push_str("\\u00");
                out.push(HEX_LOWER[(code >> 4) as usize]);
                out.push(HEX_LOWER[(code & 0xf) as usize]);
            }
            other => out.push(other),
        }
    }
    out.push('"');
}
