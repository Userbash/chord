//! REST-фасад: auth-заголовки + транспорт + 429/5xx + телеметрия (RW-12 / I1).
//!
//! Контракт заморожен `waves/RW-12/TZ.md` §1; реализация — red-first. Фасад не добавляет
//! протокольных полей: он соединяет уже подтверждённые модули [`super::headers`],
//! [`super::http1`], [`super::transport`], [`super::rate_limit`], [`super::telemetry`].

use std::fmt;
use std::time::Duration;

use super::headers::AuthHeaderValues;
pub use super::headers::{
    ClientHeadersConfig, DEFAULT_LOCALE, HEADER_FINGERPRINT, HEADER_INSTALLATION_ID,
    HEADER_ROUTING_KEY, HeaderConfig, OptionalHeaderConfig, RestHeadersConfig,
    format_accept_language,
};
use super::http1::Http1Error;
use super::rate_limit::{classify_failure, retry_delay, should_retry};
use super::routes::HttpMethod;
use super::super_properties::SuperProperties;
use super::telemetry::{TelemetryError, guard_target};
use super::transport::{
    Endpoint, HttpClientConfig, HttpError, HttpRequest, HttpResponse, send_with,
};
use crate::API_V9_BASE;

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
            if !has_header(&prepared.headers, name) {
                prepared.headers.push((name.to_string(), value.to_string()));
            }
        }
        for (name, value) in self.headers.extra_headers() {
            if !has_header(&prepared.headers, name) {
                prepared.headers.push((name.to_string(), value.to_string()));
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
