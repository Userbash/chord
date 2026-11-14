//! REST v9: auth-заголовки и коды ответов (T-REST-02, волна RW-08 / P1-07).
//!
//! Публичные типы и сигнатуры — замороженный контракт (`waves/RW-08/TZ.md` §5.1). Модуль
//! содержит шесть имён auth-заголовков полного набора API-запросов, значения этих заголовков
//! у вызывающего и классификацию наблюдённых HTTP-статусов.
//!
//! Оракул: `docs/re/rest-d02.json` (md5 `87d7a05d8cd11f3b8ae37aa5c9811c9e`) — 133 API-запроса
//! с полным набором из 6 auth-заголовков; выдержка для тестов —
//! `tests/data/rest-auth-6.tsv` (6 строк, md5 `144c34c24ecd4645540be518e69fbe3b`).

use std::fmt::Write;

/// Дефолтный язык (оракул: discord-i18n и веб-клиент Discord).
pub const DEFAULT_LOCALE: &str = "en-US";

/// Дефолтный `User-Agent` для браузерной/Electron-идентичности Discord Desktop на Linux.
pub const DEFAULT_BROWSER_USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/1.0.162 Chrome/148.0.7778.280 Electron/42.11.12 Safari/537.36";
/// Дефолтный заголовок `Sec-CH-UA`.
pub const DEFAULT_SEC_CH_UA: &str =
    r#""Chromium";v="148", "Not/A)Brand";v="24", "Google Chrome";v="148""#;
/// Дефолтный заголовок `Sec-CH-UA-Mobile`.
pub const DEFAULT_SEC_CH_UA_MOBILE: &str = "?0";
/// Дефолтный заголовок `Sec-CH-UA-Platform`.
pub const DEFAULT_SEC_CH_UA_PLATFORM: &str = r#""Linux""#;
/// Дефолтный заголовок `Sec-Fetch-Dest`.
pub const DEFAULT_SEC_FETCH_DEST: &str = "empty";
/// Дефолтный заголовок `Sec-Fetch-Mode`.
pub const DEFAULT_SEC_FETCH_MODE: &str = "cors";
/// Дефолтный заголовок `Sec-Fetch-Site`.
pub const DEFAULT_SEC_FETCH_SITE: &str = "same-origin";
/// Дефолтный заголовок `Origin`.
pub const DEFAULT_ORIGIN: &str = "https://discord.com";
/// Дефолтный заголовок `Referer`.
pub const DEFAULT_REFERER: &str = "https://discord.com/channels/@me";

/// Профиль браузерных заголовков (Client Hints, Fetch Metadata, Origin/Referer, User-Agent)
/// для имитации официального веб/Electron-клиента Discord (P2-1.1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BrowserImpersonationProfile {
    /// Значение `User-Agent`.
    pub user_agent: String,
    /// Значение `Sec-CH-UA`.
    pub sec_ch_ua: String,
    /// Значение `Sec-CH-UA-Mobile`.
    pub sec_ch_ua_mobile: String,
    /// Значение `Sec-CH-UA-Platform`.
    pub sec_ch_ua_platform: String,
    /// Значение `Sec-Fetch-Dest`.
    pub sec_fetch_dest: String,
    /// Значение `Sec-Fetch-Mode`.
    pub sec_fetch_mode: String,
    /// Значение `Sec-Fetch-Site`.
    pub sec_fetch_site: String,
    /// Значение `Origin`.
    pub origin: String,
    /// Значение `Referer`.
    pub referer: String,
}

impl Default for BrowserImpersonationProfile {
    fn default() -> Self {
        Self::desktop_default()
    }
}

impl BrowserImpersonationProfile {
    /// Профиль современного Chrome/Electron Desktop на Linux.
    #[must_use]
    pub fn chrome_linux_desktop() -> Self {
        Self {
            user_agent: DEFAULT_BROWSER_USER_AGENT.to_string(),
            sec_ch_ua: DEFAULT_SEC_CH_UA.to_string(),
            sec_ch_ua_mobile: DEFAULT_SEC_CH_UA_MOBILE.to_string(),
            sec_ch_ua_platform: DEFAULT_SEC_CH_UA_PLATFORM.to_string(),
            sec_fetch_dest: DEFAULT_SEC_FETCH_DEST.to_string(),
            sec_fetch_mode: DEFAULT_SEC_FETCH_MODE.to_string(),
            sec_fetch_site: DEFAULT_SEC_FETCH_SITE.to_string(),
            origin: DEFAULT_ORIGIN.to_string(),
            referer: DEFAULT_REFERER.to_string(),
        }
    }

    /// Дефолтный профиль для desktop-клиента.
    #[must_use]
    pub fn desktop_default() -> Self {
        Self::chrome_linux_desktop()
    }

    /// Пары `(имя, значение)` всех 9 заголовков профиля.
    #[must_use]
    pub fn header_pairs(&self) -> [(&'static str, &str); 9] {
        [
            ("User-Agent", &self.user_agent),
            ("Sec-CH-UA", &self.sec_ch_ua),
            ("Sec-CH-UA-Mobile", &self.sec_ch_ua_mobile),
            ("Sec-CH-UA-Platform", &self.sec_ch_ua_platform),
            ("Sec-Fetch-Dest", &self.sec_fetch_dest),
            ("Sec-Fetch-Mode", &self.sec_fetch_mode),
            ("Sec-Fetch-Site", &self.sec_fetch_site),
            ("Origin", &self.origin),
            ("Referer", &self.referer),
        ]
    }
}

/// Имя опционального заголовка `X-Fingerprint` (BE-P2-02, task-165).
pub const HEADER_FINGERPRINT: &str = "X-Fingerprint";
/// Имя опционального заголовка `X-Installation-ID` (BE-P2-02, task-165).
pub const HEADER_INSTALLATION_ID: &str = "X-Installation-ID";
/// Имя опционального заголовка `X-Routing-Key` (BE-P2-02, task-165).
pub const HEADER_ROUTING_KEY: &str = "X-Routing-Key";

/// Имена опциональных auth-заголовков (BE-P2-02, task-165).
pub const OPTIONAL_HEADER_NAMES: [&str; 3] = [
    HEADER_FINGERPRINT,
    HEADER_INSTALLATION_ID,
    HEADER_ROUTING_KEY,
];

/// Сформировать значение заголовка `Accept-Language` со списком q-весов `q=0.9..0.1`
/// как в браузере/Electron клиенте Discord (BE-P2-02, task-165).
///
/// Первый язык идёт без `q`, второй — `;q=0.9`, третий — `;q=0.8` и т.д. до `;q=0.1`.
/// Пустой список (или список только из пустых/пробельных строк) возвращает [`DEFAULT_LOCALE`].
#[must_use]
pub fn format_accept_language(locales: &[&str]) -> String {
    let clean: Vec<&str> = locales
        .iter()
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();
    if clean.is_empty() {
        return DEFAULT_LOCALE.to_string();
    }
    let mut result = String::new();
    for (idx, locale) in clean.into_iter().enumerate() {
        if idx > 0 {
            result.push(',');
        }
        result.push_str(locale);
        if idx > 0 {
            let q_tenth = 10usize.saturating_sub(idx).max(1);
            let _ = write!(result, ";q=0.{q_tenth}");
        }
    }
    result
}

/// Конфигурация опциональных заголовков REST-запросов (BE-P2-02, task-165).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct HeaderConfig {
    /// Значение заголовка `X-Fingerprint`.
    pub fingerprint: Option<String>,
    /// Значение заголовка `X-Installation-ID`.
    pub installation_id: Option<String>,
    /// Значение заголовка `X-Routing-Key`.
    pub routing_key: Option<String>,
}

/// Алиас типа конфигурации заголовков для обратной совместимости.
pub type OptionalHeaderConfig = HeaderConfig;
/// Алиас типа конфигурации заголовков для клиента.
pub type ClientHeadersConfig = HeaderConfig;
/// Алиас типа конфигурации заголовков REST.
pub type RestHeadersConfig = HeaderConfig;

impl HeaderConfig {
    /// Создать пустую конфигурацию опциональных заголовков.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Установить `fingerprint`.
    #[must_use]
    pub fn with_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.fingerprint = Some(fingerprint.into());
        self
    }

    /// Установить `installation_id`.
    #[must_use]
    pub fn with_installation_id(mut self, installation_id: impl Into<String>) -> Self {
        self.installation_id = Some(installation_id.into());
        self
    }

    /// Установить `routing_key`.
    #[must_use]
    pub fn with_routing_key(mut self, routing_key: impl Into<String>) -> Self {
        self.routing_key = Some(routing_key.into());
        self
    }

    /// Проверить, задан ли непустой `fingerprint`.
    #[must_use]
    pub fn has_fingerprint(&self) -> bool {
        self.fingerprint
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
    }

    /// Проверить, задан ли непустой `installation_id`.
    #[must_use]
    pub fn has_installation_id(&self) -> bool {
        self.installation_id
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
    }

    /// Проверить, задан ли непустой `routing_key`.
    #[must_use]
    pub fn has_routing_key(&self) -> bool {
        self.routing_key
            .as_ref()
            .is_some_and(|s| !s.trim().is_empty())
    }

    /// Пары `(имя, значение)` для непустых опциональных заголовков.
    /// Пустые строки и `None` не включаются.
    #[must_use]
    pub fn extra_headers(&self) -> Vec<(&'static str, &str)> {
        let mut pairs = Vec::with_capacity(3);
        if let Some(ref val) = self.fingerprint
            && !val.trim().is_empty()
        {
            pairs.push((HEADER_FINGERPRINT, val.as_str()));
        }
        if let Some(ref val) = self.installation_id
            && !val.trim().is_empty()
        {
            pairs.push((HEADER_INSTALLATION_ID, val.as_str()));
        }
        if let Some(ref val) = self.routing_key
            && !val.trim().is_empty()
        {
            pairs.push((HEADER_ROUTING_KEY, val.as_str()));
        }
        pairs
    }
}

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
