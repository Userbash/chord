//! REST v9: auth-заголовки и коды ответов (T-REST-02, волна RW-08 / P1-07).
//!
//! Публичные типы и сигнатуры — замороженный контракт (`waves/RW-08/TZ.md` §5.1). Модуль
//! содержит шесть имён auth-заголовков полного набора API-запросов, значения этих заголовков
//! у вызывающего и классификацию наблюдённых HTTP-статусов.
//!
//! Оракул: `docs/re/rest-d02.json` (md5 `87d7a05d8cd11f3b8ae37aa5c9811c9e`) — 133 API-запроса
//! с полным набором из 6 auth-заголовков; выдержка для тестов —
//! `tests/data/rest-auth-6.tsv` (6 строк, md5 `144c34c24ecd4645540be518e69fbe3b`).

/// Имена шести auth-заголовков полного набора API-запросов в порядке §3.1.
///
/// Порядок зафиксирован выдержкой `tests/data/rest-auth-6.tsv` (колонка 1):
/// `Authorization`, `X-Super-Properties`, `X-Discord-Locale`, `X-Discord-Timezone`,
/// `X-Debug-Options`, `Accept-Language`.
pub const AUTH_HEADER_NAMES: [&str; 6] = [
    "Authorization",
    "X-Super-Properties",
    "X-Discord-Locale",
    "X-Discord-Timezone",
    "X-Debug-Options",
    "Accept-Language",
];

/// Значения auth-заголовков вызывающего: токен, `X-Super-Properties`, локаль, таймзона,
/// debug-опции и `Accept-Language`. Строки живут не меньше возвращаемых пар.
#[derive(Debug, Clone, Copy)]
pub struct AuthHeaderValues<'a> {
    /// Значение заголовка `Authorization`.
    pub authorization: &'a str,
    /// Значение заголовка `X-Super-Properties`.
    pub super_properties: &'a str,
    /// Значение заголовка `X-Discord-Locale`.
    pub locale: &'a str,
    /// Значение заголовка `X-Discord-Timezone`.
    pub timezone: &'a str,
    /// Значение заголовка `X-Debug-Options`.
    pub debug_options: &'a str,
    /// Значение заголовка `Accept-Language`.
    pub accept_language: &'a str,
}

/// Пары `(имя, значение)` в порядке [`AUTH_HEADER_NAMES`].
///
/// Имена — статические (`'static`), значения заимствуются у [`AuthHeaderValues`]; пустые
/// значения не подменяются и не отбрасываются — политика заполнения у вызывающего.
#[must_use]
pub fn auth_headers<'a>(values: &AuthHeaderValues<'a>) -> [(&'static str, &'a str); 6] {
    [
        (AUTH_HEADER_NAMES[0], values.authorization),
        (AUTH_HEADER_NAMES[1], values.super_properties),
        (AUTH_HEADER_NAMES[2], values.locale),
        (AUTH_HEADER_NAMES[3], values.timezone),
        (AUTH_HEADER_NAMES[4], values.debug_options),
        (AUTH_HEADER_NAMES[5], values.accept_language),
    ]
}

/// Класс HTTP-статуса ответа REST; в захвате D-02 наблюдались 200/204/304/404/429.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResponseClass {
    /// `200 OK`.
    Ok200,
    /// `204 No Content`.
    NoContent204,
    /// `304 Not Modified`.
    NotModified304,
    /// `404 Not Found`.
    NotFound404,
    /// `429 Too Many Requests`.
    RateLimited429,
    /// Любой другой статус (включая ненаблюдённые 2xx/3xx/5xx); код сохраняется.
    Other(u16),
}

/// Классификация HTTP-статуса: пять наблюдённых кодов — отдельные варианты, остальные —
/// [`ResponseClass::Other`] с исходным кодом.
#[must_use]
pub fn classify_status(status: u16) -> ResponseClass {
    match status {
        200 => ResponseClass::Ok200,
        204 => ResponseClass::NoContent204,
        304 => ResponseClass::NotModified304,
        404 => ResponseClass::NotFound404,
        429 => ResponseClass::RateLimited429,
        other => ResponseClass::Other(other),
    }
}
