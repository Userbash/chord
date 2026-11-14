//! REST-фасад: auth-заголовки + транспорт + 429/5xx + телеметрия (RW-12 / I1).
//!
//! Контракт заморожен `waves/RW-12/TZ.md` §1; реализация — red-first. Фасад не добавляет
//! протокольных полей: он соединяет уже подтверждённые модули [`super::headers`],
//! [`super::http1`], [`super::transport`], [`super::rate_limit`], [`super::telemetry`].

use std::fmt;
use std::time::Duration;

use super::headers::AuthHeaderValues;
pub use super::headers::{
    BrowserImpersonationProfile, ClientHeadersConfig, DEFAULT_LOCALE, HEADER_FINGERPRINT,
    HEADER_INSTALLATION_ID, HEADER_ROUTING_KEY, HeaderConfig, OptionalHeaderConfig,
    RestHeadersConfig, format_accept_language,
};
use super::http1::Http1Error;
use super::rate_limit::{classify_failure, rate_limit_info, retry_delay, should_retry};
pub use super::routes::{
    CHANNEL_ATTACHMENTS_PATH, HttpMethod, MAX_ATTACHMENT_SIZE_BYTES, MAX_ATTACHMENTS_PER_MESSAGE,
    attachment_upload_route_spec, build_channel_attachments_path,
};
use super::super_properties::SuperProperties;
use super::telemetry::{TelemetryError, guard_target};
use super::transport::{
    Endpoint, HttpClientConfig, HttpError, HttpRequest, HttpResponse, send_with,
};
use crate::API_V9_BASE;
use chord_json::{JsonValue, parse as parse_json};

/// Шесть auth-заголовков в owned-виде (значения живут в клиенте).
#[derive(Clone, Default, PartialEq, Eq)]
pub struct ApiAuth {
    /// Полное значение `Authorization` (`"Bot …"` или токен).
    pub authorization: String,
    /// Значение `X-Super-Properties` (base64).
    pub super_properties: String,
    /// Значение `X-Discord-Locale`.
    pub locale: String,
    /// Значение `X-Discord-Timezone`.
    pub timezone: String,
    /// Значение `X-Debug-Options`.
    pub debug_options: String,
    /// Значение `Accept-Language`.
    pub accept_language: String,
}

impl fmt::Debug for ApiAuth {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ApiAuth")
            .field("authorization", &"<redacted>")
            .field("super_properties", &self.super_properties)
            .field("locale", &self.locale)
            .field("timezone", &self.timezone)
            .field("debug_options", &self.debug_options)
            .field("accept_language", &self.accept_language)
            .finish()
    }
}

/// Значение `X-Debug-Options` из живого захвата D-02 (присутствовал в 221/221 запроса).
pub const DEBUG_OPTIONS_LIVE: &str = "bugReporterEnabled";

/// Схема значения `Authorization`: префикс перед токеном.
///
/// В захвате D-02 значения санитизированы (`Authorization: REDACTED`, U-06), поэтому схема
/// user-токена — параметр, а не догадка. По умолчанию — сырой токен: так его описывает
/// статическая карта клиента (`rest-static` §4); подтверждение/смена — за NET-A.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AuthScheme {
    /// Сырой токен без префикса (user-token) — текущий дефолт до подтверждения NET-A.
    #[default]
    User,
    /// `Bot <token>` — bot-token.
    Bot,
    /// `Bearer <token>` — OAuth2-токен.
    Bearer,
}

impl AuthScheme {
    /// Префикс значения `Authorization` (пустой — сырой токен).
    const fn prefix(self) -> &'static str {
        match self {
            Self::User => "",
            Self::Bot => "Bot ",
            Self::Bearer => "Bearer ",
        }
    }
}

/// Ошибка сборки auth-заголовков из токена: пустые входные значения не уходят в сеть.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiAuthError {
    /// Токен пуст или состоит из пробелов.
    EmptyToken,
    /// Локаль пуста (заголовок `X-Discord-Locale` обязателен).
    EmptyLocale,
    /// Таймзона пуста (заголовок `X-Discord-Timezone` обязателен).
    EmptyTimezone,
}

impl fmt::Display for ApiAuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyToken => write!(formatter, "пустой токен"),
            Self::EmptyLocale => write!(formatter, "пустая локаль"),
            Self::EmptyTimezone => write!(formatter, "пустая таймзона"),
        }
    }
}

impl std::error::Error for ApiAuthError {}

impl ApiAuth {
    /// Собрать все 6 auth-заголовков из user-токена и [`SuperProperties`].
    ///
    /// `Authorization` — сырой токен ([`AuthScheme::User`]); `X-Super-Properties` —
    /// канонический base64; `X-Discord-Locale`/`X-Discord-Timezone` — переданные значения;
    /// `X-Debug-Options` — [`DEBUG_OPTIONS_LIVE`]; `Accept-Language` — локаль.
    ///
    /// TODO(NET-A): живой `Accept-Language` — q-список `navigator.languages`
    /// (`ru,ru-RU;q=0.9,ru-RU;q=0.8`); региональный профиль в захвате не подтверждён,
    /// поэтому до RE передаётся локаль как есть. Схема `Authorization` — [`AuthScheme`].
    ///
    /// # Errors
    ///
    /// [`ApiAuthError`] для пустого токена/локали/таймзоны: запрос без валидной «шапки»
    /// (дефолт [`ApiAuth::default`] — шесть пустых строк) в сеть не уходит.
    pub fn from_token(
        token: &str,
        super_properties: &SuperProperties,
        locale: &str,
        timezone: &str,
    ) -> Result<Self, ApiAuthError> {
        Self::from_token_with_scheme(
            token,
            AuthScheme::default(),
            super_properties,
            locale,
            timezone,
        )
    }

    /// То же, что [`ApiAuth::from_token`], но схема `Authorization` задана явно
    /// (`Bot <token>`/`Bearer <token>`/сырой токен).
    ///
    /// # Errors
    ///
    /// [`ApiAuthError`] для пустого токена/локали/таймзоны.
    pub fn from_token_with_scheme(
        token: &str,
        scheme: AuthScheme,
        super_properties: &SuperProperties,
        locale: &str,
        timezone: &str,
    ) -> Result<Self, ApiAuthError> {
        let token = token.trim();
        let locale = locale.trim();
        let timezone = timezone.trim();
        if token.is_empty() {
            return Err(ApiAuthError::EmptyToken);
        }
        if locale.is_empty() {
            return Err(ApiAuthError::EmptyLocale);
        }
        if timezone.is_empty() {
            return Err(ApiAuthError::EmptyTimezone);
        }
        Ok(Self {
            authorization: format!("{}{token}", scheme.prefix()),
            super_properties: super_properties.to_base64(),
            locale: locale.to_string(),
            timezone: timezone.to_string(),
            debug_options: DEBUG_OPTIONS_LIVE.to_string(),
            accept_language: locale.to_string(),
        })
    }

    /// Собрать заголовки для публичных анонимных запросов (без токена `Authorization`).
    pub fn anonymous(
        super_properties: &SuperProperties,
        locale: &str,
        timezone: &str,
    ) -> Result<Self, ApiAuthError> {
        let locale = locale.trim();
        let timezone = timezone.trim();
        if locale.is_empty() {
            return Err(ApiAuthError::EmptyLocale);
        }
        if timezone.is_empty() {
            return Err(ApiAuthError::EmptyTimezone);
        }
        Ok(Self {
            authorization: String::new(),
            super_properties: super_properties.to_base64(),
            locale: locale.to_string(),
            timezone: timezone.to_string(),
            debug_options: DEBUG_OPTIONS_LIVE.to_string(),
            accept_language: locale.to_string(),
        })
    }

    /// Собрать auth-заголовки из токена со списком локалей и взвешенным `Accept-Language`
    /// (BE-P2-02, task-165).
    ///
    /// Первой непустой локалью заполняется `X-Discord-Locale` (или [`DEFAULT_LOCALE`], если
    /// список пуст), а `Accept-Language` форматируется через [`format_accept_language`].
    pub fn from_token_with_locales(
        token: &str,
        super_properties: &SuperProperties,
        locales: &[&str],
        timezone: &str,
    ) -> Result<Self, ApiAuthError> {
        let clean: Vec<&str> = locales
            .iter()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .collect();
        let primary_locale = clean.first().copied().unwrap_or(DEFAULT_LOCALE);
        let mut auth = Self::from_token(token, super_properties, primary_locale, timezone)?;
        auth.accept_language = format_accept_language(locales);
        Ok(auth)
    }

    /// Скопировать значения из заимствованного набора [`AuthHeaderValues`].
    #[must_use]
    pub fn from_values(values: &AuthHeaderValues<'_>) -> Self {
        Self {
            authorization: values.authorization.to_string(),
            super_properties: values.super_properties.to_string(),
            locale: values.locale.to_string(),
            timezone: values.timezone.to_string(),
            debug_options: values.debug_options.to_string(),
            accept_language: values.accept_language.to_string(),
        }
    }

    /// Пары в порядке `rest::headers::AUTH_HEADER_NAMES`.
    #[must_use]
    pub fn pairs(&self) -> [(&'static str, &str); 6] {
        [
            ("Authorization", &self.authorization),
            ("X-Super-Properties", &self.super_properties),
            ("X-Discord-Locale", &self.locale),
            ("X-Discord-Timezone", &self.timezone),
            ("X-Debug-Options", &self.debug_options),
            ("Accept-Language", &self.accept_language),
        ]
    }
}

/// Политика повторов фасада.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApiRetryPolicy {
    /// Максимум попыток; 0 трактуется как 1.
    pub max_attempts: u32,
    /// Верхняя граница ожидания `Retry-After`/бэкоффа.
    pub max_retry_after: Duration,
    /// Базовая пауза 5xx (1×, 2×, 4×…).
    pub backoff: Duration,
    /// Таймаут одной попытки (включая чтение ответа).
    pub attempt_timeout: Duration,
}

impl Default for ApiRetryPolicy {
    fn default() -> Self {
        Self {
            max_attempts: 3,
            max_retry_after: Duration::from_secs(30),
            backoff: Duration::from_millis(250),
            attempt_timeout: Duration::from_secs(30),
        }
    }
}

/// Конфигурация API-клиента.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    /// База (`https://discord.com/api/v9`).
    pub base: String,
    /// Шесть auth-заголовков.
    pub auth: ApiAuth,
    /// Транспорт (TLS-корни, лимит тела, connect-таймаут).
    pub http: HttpClientConfig,
    /// Политика повторов.
    pub retry: ApiRetryPolicy,
}

impl Default for ApiConfig {
    /// База v9 и **пустая** auth-«шапка» ([`ApiAuth::default`]): годится для локальных стабов,
    /// но не для реальных запросов — там `auth` обязан прийти из [`ApiAuth::from_token`]
    /// (аудит NET-B P0-1: дефолт уходил в сеть с `Authorization: `).
    fn default() -> Self {
        Self {
            base: API_V9_BASE.to_string(),
            auth: ApiAuth::default(),
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy::default(),
        }
    }
}

/// Ошибка фасада: каждый исход имеет вариант, ничего не глотается.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    /// База не разбирается.
    InvalidBase(String),
    /// Некорректный target/заголовки запроса.
    InvalidRequest(String),
    /// Путь телеметрии заблокирован политикой (до сети).
    TelemetryBlocked,
    /// Таймаут попытки после исчерпания повторов.
    Timeout {
        /// Сколько попыток выполнено.
        attempts: u32,
    },
    /// Транспортная ошибка, повторять которую нельзя/исчерпаны повторы.
    Transport(HttpError),
    /// 401: сессия протухла.
    Unauthorized,
    /// 429 после исчерпания попыток.
    RateLimited {
        /// Последняя задержка из ответа.
        retry_after: Duration,
        /// Сколько попыток выполнено.
        attempts: u32,
    },
    /// 5xx после исчерпания повторов.
    Server {
        /// HTTP-статус.
        status: u16,
        /// Сколько попыток выполнено.
        attempts: u32,
    },
    /// Прочие 4xx.
    Client {
        /// HTTP-статус.
        status: u16,
    },
}

impl fmt::Display for ApiError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidBase(base) => write!(formatter, "некорректная база: {base}"),
            Self::InvalidRequest(message) => write!(formatter, "некорректный запрос: {message}"),
            Self::TelemetryBlocked => write!(formatter, "телеметрия заблокирована политикой"),
            Self::Timeout { attempts } => {
                write!(formatter, "таймаут после {attempts} попыток")
            }
            Self::Transport(error) => write!(formatter, "транспорт: {error:?}"),
            Self::Unauthorized => write!(formatter, "401: сессия протухла"),
            Self::RateLimited {
                retry_after,
                attempts,
            } => write!(
                formatter,
                "429 после {attempts} попыток (retry_after {retry_after:?})"
            ),
            Self::Server { status, attempts } => {
                write!(formatter, "HTTP {status} после {attempts} попыток")
            }
            Self::Client { status } => write!(formatter, "HTTP {status}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<TelemetryError> for ApiError {
    fn from(_: TelemetryError) -> Self {
        Self::TelemetryBlocked
    }
}

impl From<HttpError> for ApiError {
    fn from(error: HttpError) -> Self {
        match error {
            HttpError::InvalidBase(message) => Self::InvalidBase(message),
            HttpError::InvalidTarget(message) => Self::InvalidRequest(message),
            HttpError::UnsupportedScheme(message) => Self::InvalidBase(message),
            other => Self::Transport(other),
        }
    }
}

impl From<Http1Error> for ApiError {
    fn from(error: Http1Error) -> Self {
        Self::InvalidRequest(format!("{error:?}"))
    }
}

/// Клиент API: база, auth, транспорт, повторы.
#[derive(Debug, Clone)]
pub struct ApiClient {
    endpoint: Endpoint,
    base: String,
    auth: ApiAuth,
    http: HttpClientConfig,
    retry: ApiRetryPolicy,
    headers: HeaderConfig,
    browser_impersonation: Option<BrowserImpersonationProfile>,
}

impl ApiClient {
    /// Создать клиент; база проверяется сразу.
    pub fn new(config: ApiConfig) -> Result<Self, ApiError> {
        let endpoint = Endpoint::parse(&config.base)
            .map_err(|error| ApiError::InvalidBase(format!("{error:?}")))?;
        Ok(Self {
            endpoint,
            base: config.base,
            auth: config.auth,
            http: config.http,
            retry: config.retry,
            headers: HeaderConfig::default(),
            browser_impersonation: Some(BrowserImpersonationProfile::desktop_default()),
        })
    }

    /// Создать клиент с опциональной конфигурацией заголовков.
    pub fn new_with_headers(config: ApiConfig, headers: HeaderConfig) -> Result<Self, ApiError> {
        let mut client = Self::new(config)?;
        client.headers = headers;
        Ok(client)
    }

    /// Установить опциональную конфигурацию заголовков (builder-стиль).
    #[must_use]
    pub fn with_headers(mut self, headers: HeaderConfig) -> Self {
        self.headers = headers;
        self
    }

    /// Установить опциональный `X-Fingerprint` (builder-стиль).
    #[must_use]
    pub fn with_fingerprint(mut self, fingerprint: impl Into<String>) -> Self {
        self.headers.fingerprint = Some(fingerprint.into());
        self
    }

    /// Установить опциональный `X-Installation-ID` (builder-стиль).
    #[must_use]
    pub fn with_installation_id(mut self, installation_id: impl Into<String>) -> Self {
        self.headers.installation_id = Some(installation_id.into());
        self
    }

    /// Установить опциональный `X-Routing-Key` (builder-стиль).
    #[must_use]
    pub fn with_routing_key(mut self, routing_key: impl Into<String>) -> Self {
        self.headers.routing_key = Some(routing_key.into());
        self
    }

    /// Ссылка на текущую конфигурацию опциональных заголовков.
    #[must_use]
    pub fn headers(&self) -> &HeaderConfig {
        &self.headers
    }

    /// Мутабельная ссылка на текущую конфигурацию опциональных заголовков.
    pub fn headers_mut(&mut self) -> &mut HeaderConfig {
        &mut self.headers
    }

    /// Установить профиль браузерной имитации заголовков (builder-стиль).
    #[must_use]
    pub fn with_browser_impersonation(mut self, profile: BrowserImpersonationProfile) -> Self {
        self.browser_impersonation = Some(profile);
        self
    }

    /// Отключить добавление браузерных заголовков (builder-стиль).
    #[must_use]
    pub fn without_browser_impersonation(mut self) -> Self {
        self.browser_impersonation = None;
        self
    }

    /// Текущий профиль браузерной имитации заголовков, если включён.
    #[must_use]
    pub fn browser_impersonation(&self) -> Option<&BrowserImpersonationProfile> {
        self.browser_impersonation.as_ref()
    }

    /// Выполнить запрос с auth-заголовками, защитой телеметрии и повторами.
    pub async fn execute(&self, request: &HttpRequest) -> Result<HttpResponse, ApiError> {
        let prepared = self.prepare(request)?;

        let max_attempts = self.retry.max_attempts.max(1);
        let idempotent = matches!(
            prepared.method,
            HttpMethod::Get | HttpMethod::Put | HttpMethod::Delete
        );
        let mut attempt = 0u32;

        loop {
            attempt += 1;
            let result = tokio::time::timeout(
                self.retry.attempt_timeout,
                send_with(&self.http, &self.base, &prepared),
            )
            .await;

            let response = match result {
                Err(_elapsed) => {
                    if idempotent && attempt < max_attempts {
                        self.sleep_backoff(attempt).await;
                        continue;
                    }
                    return Err(ApiError::Timeout { attempts: attempt });
                }
                Ok(Err(error)) => {
                    if idempotent && attempt < max_attempts && is_retryable_transport(&error) {
                        self.sleep_backoff(attempt).await;
                        continue;
                    }
                    return Err(ApiError::from(error));
                }
                Ok(Ok(response)) => response,
            };

            let failure = match classify_failure(response.status, &response.headers, &response.body)
            {
                None => return Ok(response),
                Some(failure) => failure,
            };

            // Политика повторов — один источник истины: rate_limit::should_retry/retry_delay.
            if should_retry(prepared.method, &failure)
                && attempt < max_attempts
                && let Some(delay) = retry_delay(
                    attempt,
                    &failure,
                    self.retry.backoff,
                    self.retry.max_retry_after,
                )
            {
                tokio::time::sleep(delay).await;
                continue;
            }

            return Err(match failure {
                super::rate_limit::RestFailure::RateLimited(info) => ApiError::RateLimited {
                    retry_after: info.retry_after,
                    attempts: attempt,
                },
                super::rate_limit::RestFailure::Server { status } => ApiError::Server {
                    status,
                    attempts: attempt,
                },
                super::rate_limit::RestFailure::Unauthorized => ApiError::Unauthorized,
                super::rate_limit::RestFailure::Client { status } => ApiError::Client { status },
            });
        }
    }

    /// GET-запрос без тела.
    pub async fn get(&self, target: &str) -> Result<HttpResponse, ApiError> {
        self.execute(&HttpRequest::new(HttpMethod::Get, target))
            .await
    }

    /// POST-запрос с JSON-телом: те же auth-заголовки, телеметрия и политика повторов,
    /// что у [`ApiClient::execute`].
    ///
    /// Тело передаётся как есть (вызывающий уже сериализовал JSON); `Content-Type:
    /// application/json` ставится в [`ApiClient::prepare`], если тело непустое.
    pub async fn post(&self, target: &str, body: &[u8]) -> Result<HttpResponse, ApiError> {
        self.execute(&HttpRequest::new(HttpMethod::Post, target).body(body.to_vec()))
            .await
    }

    /// PATCH-запрос с JSON-телом: auth, телеметрия и политика повторов общие с [`execute`].
    pub async fn patch(&self, target: &str, body: &[u8]) -> Result<HttpResponse, ApiError> {
        self.execute(&HttpRequest::new(HttpMethod::Patch, target).body(body.to_vec()))
            .await
    }

    /// PUT-запрос с JSON-телом: auth, телеметрия и политика повторов общие с [`execute`].
    pub async fn put(&self, target: &str, body: &[u8]) -> Result<HttpResponse, ApiError> {
        self.execute(&HttpRequest::new(HttpMethod::Put, target).body(body.to_vec()))
            .await
    }

    /// DELETE-запрос без тела: auth, телеметрия и политика повторов общие с [`execute`].
    pub async fn delete(&self, target: &str) -> Result<HttpResponse, ApiError> {
        self.execute(&HttpRequest::new(HttpMethod::Delete, target))
            .await
    }

    /// Одноразовый POST: отдаёт **любой** HTTP-ответ (включая 4xx/5xx) с телом, без
    /// классификации статуса и без повторов.
    ///
    /// Нужен для `/auth/*`, где 4xx — часть контракта (`400 + captcha_key`, коды MFA), а
    /// ADR-3 запрещает автоматические повторы логина. Транспортная ошибка/таймаут —
    /// ошибка; статус ответа — данные для вызывающего. Заголовки `headers` кладутся после
    /// auth-шапки, поэтому явный пустой `authorization` (A-03) перекрывает токен клиента.
    pub async fn post_once(
        &self,
        target: &str,
        body: &[u8],
        headers: &[(&str, &str)],
    ) -> Result<HttpResponse, ApiError> {
        let mut request = HttpRequest::new(HttpMethod::Post, target).body(body.to_vec());
        for (name, value) in headers {
            request
                .headers
                .push(((*name).to_string(), (*value).to_string()));
        }
        let prepared = self.prepare(&request)?;
        match tokio::time::timeout(
            self.retry.attempt_timeout,
            send_with(&self.http, &self.base, &prepared),
        )
        .await
        {
            Err(_elapsed) => Err(ApiError::Timeout { attempts: 1 }),
            Ok(Err(error)) => Err(ApiError::from(error)),
            Ok(Ok(response)) => Ok(response),
        }
    }

    /// Проверить target и добавить auth/content-type заголовки (общая часть execute/post_once).
    fn prepare(&self, request: &HttpRequest) -> Result<HttpRequest, ApiError> {
        // Политика телеметрии смотрит на сырой target: fragment/query отбрасываются внутри
        // guard_target, поэтому `/track#f` блокируется до проверки target транспортом.
        guard_target(&request.target)?;
        self.endpoint.target(&request.target)?;

        let mut prepared = request.clone();
        for (name, value) in self.auth.pairs() {
            if !value.is_empty() && !has_header(&prepared.headers, name) {
                prepared.headers.push((name.to_string(), value.to_string()));
            }
        }
        for (name, value) in self.headers.extra_headers() {
            if !has_header(&prepared.headers, name) {
                prepared.headers.push((name.to_string(), value.to_string()));
            }
        }
        if let Some(ref profile) = self.browser_impersonation {
            for (name, value) in profile.header_pairs() {
                if !value.trim().is_empty() && !has_header(&prepared.headers, name) {
                    prepared.headers.push((name.to_string(), value.to_string()));
                }
            }
        }
        if !prepared.body.is_empty() && !has_header(&prepared.headers, "content-type") {
            prepared
                .headers
                .push(("Content-Type".to_string(), "application/json".to_string()));
        }
        Ok(prepared)
    }

    /// Пауза 5xx: `backoff * 2^(attempt-1)`, не больше `max_retry_after`.
    async fn sleep_backoff(&self, attempt: u32) {
        let exponent = attempt.saturating_sub(1);
        let factor = 1u32.checked_shl(exponent).unwrap_or(u32::MAX);
        let delay = self.retry.backoff.saturating_mul(factor);
        tokio::time::sleep(delay.min(self.retry.max_retry_after)).await;
    }
}

/// Есть ли поле без учёта регистра.
fn has_header(headers: &[(String, String)], name: &str) -> bool {
    headers
        .iter()
        .any(|(key, _)| key.eq_ignore_ascii_case(name))
}

/// Транспортные ошибки, которые имеет смысл повторить для идемпотентного метода.
fn is_retryable_transport(error: &HttpError) -> bool {
    matches!(
        error,
        HttpError::Connect(_) | HttpError::Io(_) | HttpError::Timeout(_)
    )
}

// ============================================================================
// REST v9 Attachments (`POST /channels/{id}/attachments` & Multipart `POST /channels/{id}/messages`)
// ============================================================================

/// Максимальная длина текста сообщения Discord (`2000` символов).
pub const MAX_MESSAGE_CONTENT_CHARS: usize = 2000;

/// Граница multipart/form-data по RFC 7578 для отправки файлов вложений.
pub const MULTIPART_BOUNDARY: &str = "----DiscordLabAttachmentBoundary9f8e7d6c5b4a";

/// Спецификация файла для выделения pre-signed слота загрузки (`POST /channels/{id}/attachments`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingAttachmentSpec {
    /// Имя файла (санитизируется перед отправкой).
    pub filename: String,
    /// Размер файла в байтах (`1..=25 MiB`).
    pub file_size: u64,
    /// MIME-тип содержимого (например, `"image/png"`).
    pub content_type: String,
    /// Локальный индекс файла в пакете (`0..10`).
    pub id: u64,
}

impl PendingAttachmentSpec {
    /// Создать спецификацию ожидающего загрузки вложения.
    #[must_use]
    pub fn new(
        id: u64,
        filename: impl Into<String>,
        file_size: u64,
        content_type: impl Into<String>,
    ) -> Self {
        Self {
            filename: filename.into(),
            file_size,
            content_type: content_type.into(),
            id,
        }
    }
}

/// Бинарная часть файла для отправки в `multipart/form-data` (`POST /channels/{id}/messages`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultipartUploadFile {
    /// Индекс поля `files[{field_index}]` (`0..10`).
    pub field_index: usize,
    /// Имя файла для `Content-Disposition` (санитизируется от инъекций и `..`).
    pub filename: String,
    /// MIME-тип части (`Content-Type`).
    pub content_type: String,
    /// Сырые байты файла (`<= 25 MiB`).
    pub bytes: Vec<u8>,
}

impl MultipartUploadFile {
    /// Создать бинарную часть файла для multipart-отправки.
    #[must_use]
    pub fn new(
        field_index: usize,
        filename: impl Into<String>,
        content_type: impl Into<String>,
        bytes: impl Into<Vec<u8>>,
    ) -> Self {
        Self {
            field_index,
            filename: filename.into(),
            content_type: content_type.into(),
            bytes: bytes.into(),
        }
    }
}

/// Выделенный сервером слот загрузки вложения (`POST /channels/{id}/attachments`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AttachmentUploadSlot {
    /// Локальный идентификатор файла из запроса.
    pub id: u64,
    /// Pre-signed URL облачного хранилища для загрузки.
    pub upload_url: String,
    /// Серверное имя загруженного файла (`upload_filename`).
    pub upload_filename: String,
}

/// Вложение в составе отправленного сообщения (`POST /channels/{id}/messages`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UploadedMessageAttachment {
    /// Snowflake ID вложения.
    pub id: String,
    /// Имя файла.
    pub filename: String,
    /// Размер файла в байтах.
    pub size: u64,
    /// CDN URL вложения.
    pub url: String,
    /// MIME-тип вложения, если присутствует.
    pub content_type: Option<String>,
}

/// Результат успешной отправки сообщения с вложениями (`POST /channels/{id}/messages`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SentAttachmentMessage {
    /// Snowflake ID созданного сообщения.
    pub id: String,
    /// Snowflake ID канала.
    pub channel_id: String,
    /// Текст сообщения.
    pub content: String,
    /// Список прикреплённых файлов.
    pub attachments: Vec<UploadedMessageAttachment>,
}

/// Типизированная ошибка валидации и отправки вложений (`P2-2.2`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttachmentUploadError {
    /// Некорректный Snowflake ID канала.
    InvalidChannelId(String),
    /// Превышен лимит количества файлов на одно сообщение (`> 10`).
    AttachmentLimitExceeded,
    /// Размер файла превышает `25 MiB` (`26_214_400` байт) или равен `0`.
    AttachmentTooLarge {
        /// Фактический размер файла в байтах.
        size: u64,
        /// Максимально допустимый размер (`26_214_400` байт).
        limit: u64,
    },
    /// Имя файла пусто или стало пустым после санитизации.
    EmptyFilename,
    /// Одновременно пустой текст сообщения и пустой список вложений.
    EmptyContentAndAttachments,
    /// HTTP `413 Payload Too Large` от сервера.
    PayloadTooLarge,
    /// HTTP `401 Unauthorized`.
    Unauthorized,
    /// HTTP `403 Forbidden`.
    Forbidden {
        /// Сообщение об ошибке от сервера, если есть.
        message: Option<String>,
    },
    /// HTTP `400 Bad Request` или превышение лимита длины текста.
    BadRequest {
        /// Код ошибки Discord API, если есть.
        code: Option<i64>,
        /// Описание ошибки.
        message: Option<String>,
    },
    /// HTTP `429 Too Many Requests` с задержкой `Retry-After`.
    RateLimited {
        /// Время ожидания до повторной попытки.
        retry_after: Duration,
    },
    /// HTTP `500..=599` ошибка сервера.
    Server {
        /// HTTP-статус ответа.
        status: u16,
    },
    /// Прочие HTTP `4xx` ошибки клиента.
    Client {
        /// HTTP-статус ответа.
        status: u16,
    },
    /// Нарушение JSON-схемы ответа сервера.
    MalformedResponse(String),
    /// Ошибка нижнего уровня `ApiError` / `HttpError`.
    Api(ApiError),
}

impl fmt::Display for AttachmentUploadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidChannelId(id) => write!(f, "некорректный ID канала: {id}"),
            Self::AttachmentLimitExceeded => write!(
                f,
                "превышен лимит вложений: максимум {MAX_ATTACHMENTS_PER_MESSAGE} файлов"
            ),
            Self::AttachmentTooLarge { size, limit } => {
                write!(f, "файл слишком большой ({size} Б, лимит {limit} Б)")
            }
            Self::EmptyFilename => write!(f, "пустое или недопустимое имя файла"),
            Self::EmptyContentAndAttachments => {
                write!(f, "сообщение не может быть пустым без вложений")
            }
            Self::PayloadTooLarge => write!(f, "413 Payload Too Large: файл превышает лимит"),
            Self::Unauthorized => write!(f, "401 Unauthorized: требуется авторизация"),
            Self::Forbidden { message } => match message {
                Some(msg) => write!(f, "403 Forbidden: {msg}"),
                None => write!(f, "403 Forbidden: нет прав на отправку файлов"),
            },
            Self::BadRequest { code, message } => match (code, message) {
                (Some(c), Some(msg)) => write!(f, "400 Bad Request (code {c}): {msg}"),
                (None, Some(msg)) => write!(f, "400 Bad Request: {msg}"),
                _ => write!(f, "400 Bad Request"),
            },
            Self::RateLimited { retry_after } => {
                write!(f, "429 Too Many Requests (retry after {retry_after:?})")
            }
            Self::Server { status } => write!(f, "ошибка сервера HTTP {status}"),
            Self::Client { status } => write!(f, "ошибка клиента HTTP {status}"),
            Self::MalformedResponse(reason) => write!(f, "некорректный ответ сервера: {reason}"),
            Self::Api(err) => write!(f, "{err}"),
        }
    }
}

impl std::error::Error for AttachmentUploadError {}

impl From<ApiError> for AttachmentUploadError {
    fn from(err: ApiError) -> Self {
        Self::Api(err)
    }
}

impl From<HttpError> for AttachmentUploadError {
    fn from(err: HttpError) -> Self {
        Self::Api(ApiError::from(err))
    }
}

/// Инжектируемый HTTP-транспорт для тестирования и выполнения запросов загрузки вложений.
#[allow(async_fn_in_trait)]
pub trait HttpTransport: Send + Sync {
    /// Отправить подготовленный [`HttpRequest`] и вернуть [`HttpResponse`].
    async fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError>;
}

impl HttpTransport for ApiClient {
    async fn send(&self, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
        send_with(&self.http, &self.base, request).await
    }
}

/// Санитизировать имя файла вложения:
/// - отсекает путевые разделители `/` и `\` (оставляя только безопасный базовый компонент);
/// - удаляет управляющие символы (`\r`, `\n`, `\0` и др.), кавычки `"` и сегменты `..`;
/// - отклоняет пустые имена и имена, состоящие только из точек/пробелов ([`AttachmentUploadError::EmptyFilename`]).
pub fn sanitize_attachment_filename(raw: &str) -> Result<String, AttachmentUploadError> {
    // Сначала удаляем управляющие символы (\r, \n, \0 и ASCII control) и кавычки
    let stripped_controls: String = raw
        .chars()
        .filter(|&c| !c.is_control() && c != '"')
        .collect();

    // Берём последний непустой сегмент после разделителей пути `/` и `\`, не равный `.` или `..`
    let candidate = stripped_controls
        .rsplit(['/', '\\'])
        .find(|seg| {
            let trimmed = seg.trim();
            !trimmed.is_empty() && trimmed != "." && trimmed != ".."
        })
        .ok_or(AttachmentUploadError::EmptyFilename)?;

    // Дополнительно убираем любые оставшиеся последовательности `..` и слэши
    let cleaned = candidate
        .replace("..", "")
        .replace(['/', '\\', '"', '\r', '\n', '\0'], "");
    let trimmed = cleaned.trim().trim_matches('.');
    if trimmed.is_empty() {
        return Err(AttachmentUploadError::EmptyFilename);
    }

    Ok(cleaned.trim().to_string())
}

fn sanitize_mime_type(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|&c| !c.is_control() && c != '"')
        .collect();
    let trimmed = cleaned.trim();
    if trimmed.is_empty() {
        "application/octet-stream".to_string()
    } else {
        trimmed.to_string()
    }
}

const HEX_LOWER: [char; 16] = [
    '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
];

fn push_json_str(out: &mut String, value: &str) {
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

fn validate_channel_messages_target(channel_id: &str) -> Result<String, AttachmentUploadError> {
    let _ = build_channel_attachments_path(channel_id)
        .map_err(|_| AttachmentUploadError::InvalidChannelId(channel_id.to_string()))?;
    let spec = super::routes::find(HttpMethod::Post, "/channels/{id}/messages")
        .expect("POST /channels/{id}/messages route in catalog");
    super::routes::render_path(spec, &[channel_id])
        .map_err(|_| AttachmentUploadError::InvalidChannelId(channel_id.to_string()))
}

fn map_attachment_error_response(response: &HttpResponse) -> AttachmentUploadError {
    let status = response.status;
    if status == 401 {
        return AttachmentUploadError::Unauthorized;
    }
    if status == 413 {
        return AttachmentUploadError::PayloadTooLarge;
    }
    if status == 429 {
        let retry_after = rate_limit_info(status, &response.headers, &response.body)
            .map(|info| info.retry_after)
            .unwrap_or(Duration::from_secs(1));
        return AttachmentUploadError::RateLimited { retry_after };
    }
    if (500..=599).contains(&status) {
        return AttachmentUploadError::Server { status };
    }

    let parsed = std::str::from_utf8(&response.body)
        .ok()
        .and_then(|text| parse_json(text).ok());
    let code = parsed
        .as_ref()
        .and_then(|v| v.get("code"))
        .and_then(JsonValue::as_i64);
    let message = parsed
        .as_ref()
        .and_then(|v| v.get("message"))
        .and_then(JsonValue::as_str)
        .map(str::to_string);

    if status == 400 {
        return AttachmentUploadError::BadRequest { code, message };
    }
    if status == 403 {
        return AttachmentUploadError::Forbidden { message };
    }

    AttachmentUploadError::Client { status }
}

impl ApiClient {
    /// Собрать подготовленный [`HttpRequest`] для `POST /channels/{channel_id}/attachments`
    /// с JSON-телом `{"files":[...]}` и полным набором заголовков `ApiAuth` + `BrowserImpersonationProfile`.
    pub fn request_attachment_upload_slots(
        &self,
        channel_id: &str,
        files: &[PendingAttachmentSpec],
    ) -> Result<HttpRequest, AttachmentUploadError> {
        let target = build_channel_attachments_path(channel_id)
            .map_err(|_| AttachmentUploadError::InvalidChannelId(channel_id.to_string()))?;

        if files.is_empty() {
            return Err(AttachmentUploadError::EmptyContentAndAttachments);
        }
        if files.len() > MAX_ATTACHMENTS_PER_MESSAGE {
            return Err(AttachmentUploadError::AttachmentLimitExceeded);
        }

        let mut body = String::from("{\"files\":[");
        for (idx, file) in files.iter().enumerate() {
            if file.file_size == 0 || file.file_size > MAX_ATTACHMENT_SIZE_BYTES as u64 {
                return Err(AttachmentUploadError::AttachmentTooLarge {
                    size: file.file_size,
                    limit: MAX_ATTACHMENT_SIZE_BYTES as u64,
                });
            }
            let clean_name = sanitize_attachment_filename(&file.filename)?;
            let clean_mime = sanitize_mime_type(&file.content_type);
            if idx > 0 {
                body.push(',');
            }
            body.push_str("{\"filename\":");
            push_json_str(&mut body, &clean_name);
            body.push_str(",\"file_size\":");
            body.push_str(&file.file_size.to_string());
            body.push_str(",\"content_type\":");
            push_json_str(&mut body, &clean_mime);
            body.push_str(",\"id\":");
            body.push_str(&file.id.to_string());
            body.push('}');
        }
        body.push_str("]}");

        let raw_req = HttpRequest::new(HttpMethod::Post, target).body(body.into_bytes());
        self.prepare(&raw_req).map_err(AttachmentUploadError::Api)
    }

    /// Собрать подготовленный `multipart/form-data` [`HttpRequest`] для `POST /channels/{channel_id}/messages`
    /// по RFC 7578 (`payload_json` + бинарные части `files[0]`..`files[N]`).
    pub fn build_multipart_message_request(
        &self,
        channel_id: &str,
        content: &str,
        files: &[MultipartUploadFile],
        nonce: Option<&str>,
    ) -> Result<HttpRequest, AttachmentUploadError> {
        let target = validate_channel_messages_target(channel_id)?;

        if content.trim().is_empty() && files.is_empty() {
            return Err(AttachmentUploadError::EmptyContentAndAttachments);
        }
        if content.chars().count() > MAX_MESSAGE_CONTENT_CHARS {
            return Err(AttachmentUploadError::BadRequest {
                code: Some(50035),
                message: Some(format!(
                    "Must be {MAX_MESSAGE_CONTENT_CHARS} or fewer in length."
                )),
            });
        }
        if files.len() > MAX_ATTACHMENTS_PER_MESSAGE {
            return Err(AttachmentUploadError::AttachmentLimitExceeded);
        }

        let mut sanitized_files = Vec::with_capacity(files.len());
        for file in files {
            let size = file.bytes.len() as u64;
            if size > MAX_ATTACHMENT_SIZE_BYTES as u64 {
                return Err(AttachmentUploadError::AttachmentTooLarge {
                    size,
                    limit: MAX_ATTACHMENT_SIZE_BYTES as u64,
                });
            }
            let clean_name = sanitize_attachment_filename(&file.filename)?;
            let clean_mime = sanitize_mime_type(&file.content_type);
            sanitized_files.push((file.field_index, clean_name, clean_mime, &file.bytes));
        }

        let mut payload_json = String::from("{\"content\":");
        push_json_str(&mut payload_json, content);
        if let Some(nonce_val) = nonce {
            payload_json.push_str(",\"nonce\":");
            push_json_str(&mut payload_json, nonce_val);
        }
        payload_json.push_str(",\"tts\":false,\"attachments\":[");
        for (idx, (field_index, clean_name, _, _)) in sanitized_files.iter().enumerate() {
            if idx > 0 {
                payload_json.push(',');
            }
            payload_json.push_str("{\"id\":");
            payload_json.push_str(&field_index.to_string());
            payload_json.push_str(",\"filename\":");
            push_json_str(&mut payload_json, clean_name);
            payload_json.push('}');
        }
        payload_json.push_str("]}");

        let boundary = MULTIPART_BOUNDARY;
        let mut body = Vec::with_capacity(
            payload_json.len() + files.iter().map(|f| f.bytes.len() + 256).sum::<usize>() + 128,
        );

        // Часть 1: payload_json
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            b"Content-Disposition: form-data; name=\"payload_json\"\r\nContent-Type: application/json\r\n\r\n",
        );
        body.extend_from_slice(payload_json.as_bytes());
        body.extend_from_slice(b"\r\n");

        // Части 2..N: files[0]..files[N]
        for (field_index, clean_name, clean_mime, file_bytes) in sanitized_files {
            body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
            body.extend_from_slice(
                format!(
                    "Content-Disposition: form-data; name=\"files[{field_index}]\"; filename=\"{clean_name}\"\r\nContent-Type: {clean_mime}\r\n\r\n"
                )
                .as_bytes(),
            );
            body.extend_from_slice(file_bytes);
            body.extend_from_slice(b"\r\n");
        }

        // Закрывающая граница RFC 7578
        body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());

        let raw_req = HttpRequest::new(HttpMethod::Post, target)
            .header(
                "Content-Type",
                format!("multipart/form-data; boundary={boundary}"),
            )
            .body(body);
        self.prepare(&raw_req).map_err(AttachmentUploadError::Api)
    }

    /// Выделить слоты загрузки вложений (`POST /channels/{channel_id}/attachments`) через инжектируемый [`HttpTransport`].
    pub async fn create_attachment_upload_slots<T: HttpTransport>(
        &self,
        transport: &T,
        channel_id: &str,
        files: &[PendingAttachmentSpec],
    ) -> Result<Vec<AttachmentUploadSlot>, AttachmentUploadError> {
        let req = self.request_attachment_upload_slots(channel_id, files)?;
        let response = transport.send(&req).await?;

        if !(200..300).contains(&response.status) {
            return Err(map_attachment_error_response(&response));
        }

        let text = std::str::from_utf8(&response.body)
            .map_err(|_| AttachmentUploadError::MalformedResponse("ответ не UTF-8".to_string()))?;
        let root = parse_json(text).map_err(|e| {
            AttachmentUploadError::MalformedResponse(format!("невалидный JSON: {e}"))
        })?;
        let attachments = root
            .get("attachments")
            .and_then(JsonValue::as_array)
            .ok_or_else(|| {
                AttachmentUploadError::MalformedResponse(
                    "отсутствует массив attachments".to_string(),
                )
            })?;

        let mut slots = Vec::with_capacity(attachments.len());
        for (idx, item) in attachments.iter().enumerate() {
            let id = item
                .get("id")
                .and_then(|v| {
                    v.as_u64()
                        .or_else(|| v.as_str().and_then(|s| s.parse::<u64>().ok()))
                })
                .unwrap_or(idx as u64);
            let upload_url = item
                .get("upload_url")
                .and_then(JsonValue::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    AttachmentUploadError::MalformedResponse(
                        "отсутствует поле upload_url".to_string(),
                    )
                })?
                .to_string();
            let upload_filename = item
                .get("upload_filename")
                .and_then(JsonValue::as_str)
                .filter(|s| !s.is_empty())
                .ok_or_else(|| {
                    AttachmentUploadError::MalformedResponse(
                        "отсутствует поле upload_filename".to_string(),
                    )
                })?
                .to_string();
            slots.push(AttachmentUploadSlot {
                id,
                upload_url,
                upload_filename,
            });
        }

        Ok(slots)
    }

    /// Отправить сообщение с вложениями (`POST /channels/{channel_id}/messages` multipart/form-data)
    /// через инжектируемый [`HttpTransport`].
    pub async fn send_message_with_attachments<T: HttpTransport>(
        &self,
        transport: &T,
        channel_id: &str,
        content: &str,
        files: &[MultipartUploadFile],
        nonce: Option<&str>,
    ) -> Result<SentAttachmentMessage, AttachmentUploadError> {
        let req = self.build_multipart_message_request(channel_id, content, files, nonce)?;
        let response = transport.send(&req).await?;

        if !(200..300).contains(&response.status) {
            return Err(map_attachment_error_response(&response));
        }

        let text = std::str::from_utf8(&response.body)
            .map_err(|_| AttachmentUploadError::MalformedResponse("ответ не UTF-8".to_string()))?;
        let root = parse_json(text).map_err(|e| {
            AttachmentUploadError::MalformedResponse(format!("невалидный JSON: {e}"))
        })?;

        let id = root
            .get("id")
            .and_then(JsonValue::as_str)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                AttachmentUploadError::MalformedResponse("отсутствует id сообщения".to_string())
            })?
            .to_string();
        let resp_channel_id = root
            .get("channel_id")
            .and_then(JsonValue::as_str)
            .unwrap_or(channel_id)
            .to_string();
        let resp_content = root
            .get("content")
            .and_then(JsonValue::as_str)
            .unwrap_or(content)
            .to_string();

        let mut uploaded_attachments = Vec::new();
        if let Some(arr) = root.get("attachments").and_then(JsonValue::as_array) {
            for item in arr {
                let att_id = item
                    .get("id")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("0")
                    .to_string();
                let filename = item
                    .get("filename")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                    .to_string();
                let size = item.get("size").and_then(JsonValue::as_u64).unwrap_or(0);
                let url = item
                    .get("url")
                    .and_then(JsonValue::as_str)
                    .unwrap_or("")
                    .to_string();
                let content_type = item
                    .get("content_type")
                    .and_then(JsonValue::as_str)
                    .map(str::to_string);
                uploaded_attachments.push(UploadedMessageAttachment {
                    id: att_id,
                    filename,
                    size,
                    url,
                    content_type,
                });
            }
        }

        Ok(SentAttachmentMessage {
            id,
            channel_id: resp_channel_id,
            content: resp_content,
            attachments: uploaded_attachments,
        })
    }
}
