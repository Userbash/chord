//! Discord REST v9 `LoginService` (P0-2.1, P0-2.4):
//! вход по паролю (`POST /api/v9/auth/login`), TOTP 2FA (`POST /api/v9/auth/mfa/totp`),
//! обработка Captcha (`400` с `captcha_key`, без обхода/решения) и загрузка профиля
//! (`GET /api/v9/users/@me`).
//!
//! Оракулы:
//! - `docs/re/rest-d02.md` (§2, §6.1, §6.2): заголовки `Authorization`, `X-Super-Properties`,
//!   `X-Discord-Locale`, `X-Discord-Timezone`, `X-Debug-Options`, `Accept-Language`, `User-Agent`,
//!   база `https://discord.com/api/v9`, маршрут `GET /api/v9/users/@me` и `429` rate limit.
//! - `chord_core::rest::transport` (`HttpRequest`, `HttpResponse`, `HttpClientConfig`, `Endpoint`, `send_with`),
//!   `chord_core::rest::routes::HttpMethod`, `chord_core::rest::super_properties::SuperProperties`,
//!   `chord_core::rest::client::{ApiAuth, ApiAuthError, DEBUG_OPTIONS_LIVE}`,
//!   `chord_core::rest::user::{Me, MeError, parse_me}`.

#![forbid(unsafe_code)]
#![allow(async_fn_in_trait)]
#![allow(dead_code)]

use std::fmt;
use std::sync::Mutex;

use chord_core::parse_me;
use chord_core::rest::client::{ApiAuth, ApiAuthError, DEBUG_OPTIONS_LIVE};
use chord_core::rest::rate_limit::rate_limit_info;
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::super_properties::SuperProperties;
use chord_core::rest::transport::{
    Endpoint, HttpClientConfig, HttpError, HttpRequest, HttpResponse, send_with,
};
use chord_core::rest::user::{Me, MeError};
use serde_json::{Value, json};

/// Маршрут входа по логину и паролю (`POST /api/v9/auth/login`).
pub const AUTH_LOGIN_PATH: &str = "/api/v9/auth/login";

/// Маршрут подтверждения 2FA TOTP (`POST /api/v9/auth/mfa/totp`).
pub const AUTH_MFA_TOTP_PATH: &str = "/api/v9/auth/mfa/totp";

/// Маршрут получения текущего пользователя (`GET /api/v9/users/@me`).
pub const USERS_ME_PATH: &str = "/api/v9/users/@me";

/// Дефолтная локаль клиента (`docs/re/rest-d02.md` §6.2).
pub const DEFAULT_DISCORD_LOCALE: &str = "en-US";

/// Дефолтная таймзона клиента.
pub const DEFAULT_DISCORD_TIMEZONE: &str = "UTC";

/// Дефолтный сервис капчи Discord (`hcaptcha`).
pub const DEFAULT_CAPTCHA_SERVICE: &str = "hcaptcha";

/// Дефолтный sitekey hCaptcha Discord, если `captcha_key` пришёл массивом без `captcha_sitekey`.
pub const DEFAULT_DISCORD_HCAPTCHA_SITEKEY: &str = "a9b5fb07-92ff-493f-86fe-352a2803b3df";

/// Минимальная длина кода MFA (6 цифр TOTP).
pub const MFA_CODE_MIN_DIGITS: usize = 6;

/// Максимальная длина кода MFA (8 цифр резервного/расширенного кода).
pub const MFA_CODE_MAX_DIGITS: usize = 8;

/// Расширение [`ApiAuth`] для именованного конструктора `from_user_token`.
pub trait ApiAuthUserExt: Sized {
    /// Собирает [`ApiAuth`] из пользовательского токена (`AuthScheme::User`),
    /// [`SuperProperties`], локали и таймзоны.
    fn from_user_token(
        token: &str,
        super_properties: &SuperProperties,
        locale: &str,
        timezone: &str,
    ) -> Result<Self, ApiAuthError>;
}

impl ApiAuthUserExt for ApiAuth {
    fn from_user_token(
        token: &str,
        super_properties: &SuperProperties,
        locale: &str,
        timezone: &str,
    ) -> Result<Self, ApiAuthError> {
        ApiAuth::from_token(token, super_properties, locale, timezone)
    }
}

/// Вызов второго фактора (`200 OK` с `"mfa": true`).
///
/// Поле `ticket` редактируется в `Debug`, чтобы секретный тикет сессии никогда не попадал в логи.
#[derive(Clone, PartialEq, Eq)]
pub struct MfaChallenge {
    /// Одноразовый тикет MFA-сессии.
    pub ticket: String,
    /// Доступен ли метод TOTP (`"totp": true`).
    pub totp: bool,
    /// Доступен ли метод SMS (`"sms": true`).
    pub sms: bool,
    /// Доступен ли резервный код (`"backup": true`).
    pub backup: bool,
}

impl fmt::Debug for MfaChallenge {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MfaChallenge")
            .field("ticket", &"[REDACTED]")
            .field("totp", &self.totp)
            .field("sms", &self.sms)
            .field("backup", &self.backup)
            .finish()
    }
}

/// Детали требования капчи (`400 Bad Request` с `captcha_key`).
///
/// Согласно политике безопасности клиент никогда не пытается автоматически решать или обходить
/// капчу — структура возвращается в [`LoginError::CaptchaRequired`], чтобы UI уведомил пользователя.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CaptchaChallenge {
    /// Провайдер капчи (например, `"hcaptcha"`).
    pub captcha_service: String,
    /// Публичный ключ сайта (`captcha_sitekey`).
    pub captcha_sitekey: String,
    /// Дополнительные данные (`captcha_rqdata`), если присутствуют.
    pub captcha_rqdata: Option<String>,
    /// Токен запроса капчи (`captcha_rqtoken`), если присутствует.
    pub captcha_rqtoken: Option<String>,
}

/// Результат успешного ответа `POST /api/v9/auth/login`.
#[derive(Clone, PartialEq, Eq)]
pub enum LoginOutcome {
    /// Прямая аутентификация без MFA: получен токен сессии и опциональные настройки.
    Authenticated {
        /// Токен сессии (маскируется в `Debug`).
        token: String,
        /// Начальные настройки пользователя (`user_settings`), если сервер их вернул.
        user_settings: Option<Value>,
    },
    /// Требуется подтверждение второго фактора (MFA).
    MfaRequired(MfaChallenge),
}

impl fmt::Debug for LoginOutcome {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Authenticated {
                token: _,
                user_settings,
            } => f
                .debug_struct("Authenticated")
                .field("token", &"[REDACTED]")
                .field("user_settings", user_settings)
                .finish(),
            Self::MfaRequired(challenge) => f.debug_tuple("MfaRequired").field(challenge).finish(),
        }
    }
}

/// Ошибки сервиса аутентификации [`LoginService`].
///
/// Ни `Debug`, ни `Display` не содержат паролей, MFA-тикетов или токенов.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginError {
    /// Ошибка валидации входных параметров до отправки в сеть.
    Validation(String),
    /// Сервер потребовал прохождения капчи (`400` с `captcha_key`).
    CaptchaRequired(CaptchaChallenge),
    /// Неверные учётные данные или неверный код/тикет MFA (`400` / `401`).
    InvalidCredentials {
        /// Код ошибки Discord API (например, `50035`, `60008`), если присутствует.
        code: Option<i64>,
        /// Безопасное описание ошибки от сервера или клиента.
        message: String,
    },
    /// Превышен лимит запросов (`429 Too Many Requests`).
    RateLimited {
        /// Задержка до следующей попытки в миллисекундах.
        retry_after_ms: u64,
        /// Признак глобального rate-limit.
        global: bool,
    },
    /// Внутренняя ошибка сервера Discord или шлюза (`500..=599`).
    ServerError {
        /// HTTP-статус ответа (`500..=599`).
        status: u16,
    },
    /// Ошибка разбора JSON-ответа или нарушение схемы ответа.
    MalformedResponse(String),
    /// Сетевая или транспортная ошибка нижнего уровня.
    Transport(HttpError),
}

impl fmt::Display for LoginError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Validation(msg) => write!(f, "login validation error: {msg}"),
            Self::CaptchaRequired(challenge) => write!(
                f,
                "captcha required by {} (sitekey: {})",
                challenge.captcha_service, challenge.captcha_sitekey
            ),
            Self::InvalidCredentials { code, message } => match code {
                Some(c) => write!(f, "invalid credentials (code {c}): {message}"),
                None => write!(f, "invalid credentials: {message}"),
            },
            Self::RateLimited {
                retry_after_ms,
                global,
            } => write!(
                f,
                "rate limited (retry_after_ms={retry_after_ms}, global={global})"
            ),
            Self::ServerError { status } => write!(f, "server error (HTTP {status})"),
            Self::MalformedResponse(reason) => write!(f, "malformed response: {reason}"),
            Self::Transport(err) => write!(f, "transport error: {err:?}"),
        }
    }
}

impl std::error::Error for LoginError {}

impl From<HttpError> for LoginError {
    fn from(err: HttpError) -> Self {
        Self::Transport(err)
    }
}

/// Абстракция HTTP-транспорта для `LoginService` (позволяет тестировать все коды ответов оффлайн).
pub trait AuthHttpTransport: Send + Sync {
    /// Выполняет одиночный HTTP-запрос относительно `base_url`.
    async fn execute(
        &self,
        base_url: &str,
        request: HttpRequest,
    ) -> Result<HttpResponse, LoginError>;
}

/// Боевой адаптер транспорта поверх `chord_core::rest::transport`.
///
/// Поддерживает как `https://host[:port]/api/v9`, так и локальный `http://127.0.0.1:port/api/v9`
/// для E2E мок-шлюза, нормализуя префикс `/api/v9` во избежание дублирования в проводе.
#[derive(Debug, Clone, Default)]
pub struct LiveAuthTransport {
    config: HttpClientConfig,
}

impl LiveAuthTransport {
    /// Создаёт боевой транспорт с конфигурацией по умолчанию.
    #[must_use]
    pub fn new() -> Self {
        Self {
            config: HttpClientConfig::default(),
        }
    }

    /// Создаёт боевой транспорт с пользовательской [`HttpClientConfig`].
    #[must_use]
    pub fn with_config(config: HttpClientConfig) -> Self {
        Self { config }
    }
}

impl AuthHttpTransport for LiveAuthTransport {
    async fn execute(
        &self,
        base_url: &str,
        mut request: HttpRequest,
    ) -> Result<HttpResponse, LoginError> {
        let endpoint = Endpoint::parse(base_url).map_err(LoginError::Transport)?;
        if !endpoint.base_path.is_empty()
            && let Some(stripped) = request.target.strip_prefix(&endpoint.base_path)
            && stripped.starts_with('/')
        {
            request.target = stripped.to_string();
        }
        send_with(&self.config, base_url, &request)
            .await
            .map_err(LoginError::Transport)
    }
}

/// Записанный вызов в [`MockAuthTransport`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordedAuthCall {
    /// Базовый URL, переданный в `execute`.
    pub base_url: String,
    /// HTTP-запрос.
    pub request: HttpRequest,
}

/// Детерминированный оффлайн-транспорт для контрактных и модульных тестов.
#[derive(Default)]
pub struct MockAuthTransport {
    queued_responses: Mutex<Vec<Result<HttpResponse, LoginError>>>,
    recorded_calls: Mutex<Vec<RecordedAuthCall>>,
}

impl MockAuthTransport {
    /// Создаёт пустой мок-транспорт.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Добавляет следующий HTTP-ответ в очередь (FIFO).
    pub fn push_response(&self, response: HttpResponse) {
        if let Ok(mut queue) = self.queued_responses.lock() {
            queue.push(Ok(response));
        }
    }

    /// Добавляет следующую ошибку транспорта в очередь (FIFO).
    pub fn push_error(&self, error: LoginError) {
        if let Ok(mut queue) = self.queued_responses.lock() {
            queue.push(Err(error));
        }
    }

    /// Возвращает все записанные запросы.
    #[must_use]
    pub fn recorded_calls(&self) -> Vec<RecordedAuthCall> {
        self.recorded_calls
            .lock()
            .map(|calls| calls.clone())
            .unwrap_or_default()
    }
}

impl AuthHttpTransport for MockAuthTransport {
    async fn execute(
        &self,
        base_url: &str,
        request: HttpRequest,
    ) -> Result<HttpResponse, LoginError> {
        if let Ok(mut calls) = self.recorded_calls.lock() {
            calls.push(RecordedAuthCall {
                base_url: base_url.to_string(),
                request,
            });
        }
        let mut queue = self
            .queued_responses
            .lock()
            .map_err(|_| LoginError::MalformedResponse("mock mutex poisoned".to_string()))?;
        if queue.is_empty() {
            return Err(LoginError::MalformedResponse(
                "no queued response in MockAuthTransport".to_string(),
            ));
        }
        queue.remove(0)
    }
}

/// Сервис аутентификации Discord REST API v9 (`login_password`, `submit_totp`, `fetch_current_user`).
#[derive(Debug, Clone)]
pub struct LoginService<T: AuthHttpTransport = LiveAuthTransport> {
    base_url: String,
    super_properties: SuperProperties,
    locale: String,
    timezone: String,
    user_agent: String,
    transport: T,
}

impl<T: AuthHttpTransport> LoginService<T> {
    /// Создаёт [`LoginService`] с явно заданным транспортом, базовым URL и [`SuperProperties`].
    #[must_use]
    pub fn new(
        base_url: impl Into<String>,
        super_properties: SuperProperties,
        transport: T,
    ) -> Self {
        let user_agent = if super_properties.browser_user_agent.trim().is_empty() {
            chord_core::rest::transport::DEFAULT_USER_AGENT.to_string()
        } else {
            super_properties.browser_user_agent.clone()
        };
        let locale = if super_properties.system_locale.trim().is_empty() {
            DEFAULT_DISCORD_LOCALE.to_string()
        } else {
            super_properties.system_locale.clone()
        };
        Self {
            base_url: base_url.into(),
            super_properties,
            locale,
            timezone: DEFAULT_DISCORD_TIMEZONE.to_string(),
            user_agent,
            transport,
        }
    }

    /// Переопределяет заголовок `X-Discord-Locale`.
    #[must_use]
    pub fn with_locale(mut self, locale: impl Into<String>) -> Self {
        self.locale = locale.into();
        self
    }

    /// Переопределяет заголовок `X-Discord-Timezone`.
    #[must_use]
    pub fn with_timezone(mut self, timezone: impl Into<String>) -> Self {
        self.timezone = timezone.into();
        self
    }

    /// Переопределяет заголовок `User-Agent`.
    #[must_use]
    pub fn with_user_agent(mut self, user_agent: impl Into<String>) -> Self {
        self.user_agent = user_agent.into();
        self
    }

    /// Возвращает ссылку на транспорт.
    #[must_use]
    pub fn transport(&self) -> &T {
        &self.transport
    }

    /// Собирает анонимный запрос с заголовками идентичности (`User-Agent`, `X-Super-Properties`,
    /// `X-Discord-Locale`, `X-Discord-Timezone`, `X-Debug-Options`, `Accept-Language`).
    fn build_unauthenticated_post(&self, target: &str, json_body: &Value) -> HttpRequest {
        HttpRequest::new(HttpMethod::Post, target)
            .header("Content-Type", "application/json")
            .header("User-Agent", &self.user_agent)
            .header("X-Super-Properties", self.super_properties.to_base64())
            .header("X-Discord-Locale", &self.locale)
            .header("X-Discord-Timezone", &self.timezone)
            .header("X-Debug-Options", DEBUG_OPTIONS_LIVE)
            .header("Accept-Language", &self.locale)
            .body(json_body.to_string().into_bytes())
    }

    /// Выполняет вход по логину и паролю (`POST /api/v9/auth/login`).
    pub async fn login_password(
        &self,
        login: &str,
        password: &str,
    ) -> Result<LoginOutcome, LoginError> {
        let trimmed_login = login.trim();
        if trimmed_login.is_empty() {
            return Err(LoginError::Validation(
                "login must not be empty".to_string(),
            ));
        }
        if password.is_empty() {
            return Err(LoginError::Validation(
                "password must not be empty".to_string(),
            ));
        }

        let payload = json!({
            "login": trimmed_login,
            "password": password,
            "undelete": false
        });
        let request = self.build_unauthenticated_post(AUTH_LOGIN_PATH, &payload);
        let response = self.transport.execute(&self.base_url, request).await?;

        match response.status {
            200 => parse_login_200(&response.body),
            status => Err(map_non_200_error(status, &response.headers, &response.body)),
        }
    }

    /// Подтверждает второй фактор TOTP (`POST /api/v9/auth/mfa/totp`) и возвращает токен сессии.
    pub async fn submit_totp(&self, ticket: &str, code: &str) -> Result<String, LoginError> {
        let trimmed_ticket = ticket.trim();
        if trimmed_ticket.is_empty() {
            return Err(LoginError::Validation(
                "MFA ticket must not be empty".to_string(),
            ));
        }
        let trimmed_code = code.trim();
        if !(MFA_CODE_MIN_DIGITS..=MFA_CODE_MAX_DIGITS).contains(&trimmed_code.len())
            || !trimmed_code.bytes().all(|b| b.is_ascii_digit())
        {
            return Err(LoginError::Validation(
                "TOTP code must be 6 to 8 ASCII digits".to_string(),
            ));
        }

        let payload = json!({
            "code": trimmed_code,
            "ticket": trimmed_ticket
        });
        let request = self.build_unauthenticated_post(AUTH_MFA_TOTP_PATH, &payload);
        let response = self.transport.execute(&self.base_url, request).await?;

        match response.status {
            200 => parse_totp_200(&response.body),
            status => Err(map_non_200_error(status, &response.headers, &response.body)),
        }
    }

    /// Запрашивает профиль текущего пользователя (`GET /api/v9/users/@me`) с проверкой токена.
    pub async fn fetch_current_user(&self, token: &str) -> Result<Me, LoginError> {
        let auth =
            ApiAuth::from_user_token(token, &self.super_properties, &self.locale, &self.timezone)
                .map_err(|err| LoginError::Validation(err.to_string()))?;

        let mut request =
            HttpRequest::new(HttpMethod::Get, USERS_ME_PATH).header("User-Agent", &self.user_agent);
        for (name, value) in auth.pairs() {
            request = request.header(name, value);
        }

        let response = self.transport.execute(&self.base_url, request).await?;
        match response.status {
            200 => parse_me(&response.body).map_err(map_me_error),
            status => Err(map_non_200_error(status, &response.headers, &response.body)),
        }
    }
}

/// Разбирает тело `200 OK` для `POST /api/v9/auth/login`.
fn parse_login_200(body: &[u8]) -> Result<LoginOutcome, LoginError> {
    let value: Value = serde_json::from_slice(body).map_err(|err| {
        LoginError::MalformedResponse(format!("invalid JSON on 200 login: {err}"))
    })?;
    let obj = value.as_object().ok_or_else(|| {
        LoginError::MalformedResponse("login 200 JSON root must be an object".to_string())
    })?;

    if let Some(token) = obj
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
    {
        let user_settings = obj.get("user_settings").filter(|v| !v.is_null()).cloned();
        return Ok(LoginOutcome::Authenticated {
            token: token.to_string(),
            user_settings,
        });
    }

    let is_mfa = obj.get("mfa").and_then(Value::as_bool).unwrap_or(false);
    if is_mfa {
        let ticket = obj
            .get("ticket")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .ok_or_else(|| {
                LoginError::MalformedResponse("MFA challenge missing non-empty ticket".to_string())
            })?
            .to_string();
        let totp = obj.get("totp").and_then(Value::as_bool).unwrap_or(false);
        let sms = obj.get("sms").and_then(Value::as_bool).unwrap_or(false);
        let backup = obj.get("backup").and_then(Value::as_bool).unwrap_or(false);
        return Ok(LoginOutcome::MfaRequired(MfaChallenge {
            ticket,
            totp,
            sms,
            backup,
        }));
    }

    Err(LoginError::MalformedResponse(
        "login 200 response missing non-empty token and mfa challenge".to_string(),
    ))
}

/// Разбирает тело `200 OK` для `POST /api/v9/auth/mfa/totp`.
fn parse_totp_200(body: &[u8]) -> Result<String, LoginError> {
    let value: Value = serde_json::from_slice(body)
        .map_err(|err| LoginError::MalformedResponse(format!("invalid JSON on 200 totp: {err}")))?;
    let obj = value.as_object().ok_or_else(|| {
        LoginError::MalformedResponse("totp 200 JSON root must be an object".to_string())
    })?;

    let token = obj
        .get("token")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .ok_or_else(|| {
            LoginError::MalformedResponse("totp 200 response missing non-empty token".to_string())
        })?;
    Ok(token.to_string())
}

/// Преобразует ошибку `chord_core::parse_me` в [`LoginError`].
fn map_me_error(err: MeError) -> LoginError {
    match err {
        MeError::Unauthorized => LoginError::InvalidCredentials {
            code: Some(0),
            message: "401: Unauthorized".to_string(),
        },
        other => LoginError::MalformedResponse(other.to_string()),
    }
}

/// Разбирает коды ошибок HTTP (`400`, `401`, `429`, `500..=599` и прочие).
fn map_non_200_error(status: u16, headers: &[(String, String)], body: &[u8]) -> LoginError {
    match status {
        429 => {
            let info = rate_limit_info(429, headers, body).unwrap_or_default();
            let retry_after_ms = u64::try_from(info.retry_after.as_millis()).unwrap_or(u64::MAX);
            LoginError::RateLimited {
                retry_after_ms,
                global: info.global,
            }
        }
        500..=599 => LoginError::ServerError { status },
        400 => parse_400_error(body),
        401 => {
            let (code, message) = extract_discord_code_and_message(body, "401: Unauthorized");
            LoginError::InvalidCredentials { code, message }
        }
        other => {
            let (_, message) = extract_discord_code_and_message(body, "unexpected status");
            LoginError::MalformedResponse(format!("HTTP {other}: {message}"))
        }
    }
}

/// Разбирает ответ `400 Bad Request`: выделяет требование капчи (`captcha_key`) либо ошибку учётных данных.
fn parse_400_error(body: &[u8]) -> LoginError {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return LoginError::MalformedResponse("invalid JSON on 400 response".to_string());
    };
    let Some(obj) = value.as_object() else {
        return LoginError::MalformedResponse("400 JSON root must be an object".to_string());
    };

    let has_captcha_key = obj.get("captcha_key").is_some_and(|v| match v {
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(arr) => !arr.is_empty(),
        _ => false,
    });
    let explicit_sitekey = obj
        .get("captcha_sitekey")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty());

    if has_captcha_key || explicit_sitekey.is_some() {
        let captcha_sitekey = explicit_sitekey
            .or_else(|| {
                obj.get("captcha_key")
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
            })
            .unwrap_or(DEFAULT_DISCORD_HCAPTCHA_SITEKEY)
            .to_string();

        let captcha_service = obj
            .get("captcha_service")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .unwrap_or(DEFAULT_CAPTCHA_SERVICE)
            .to_string();

        let captcha_rqdata = obj
            .get("captcha_rqdata")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);

        let captcha_rqtoken = obj
            .get("captcha_rqtoken")
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(ToOwned::to_owned);

        return LoginError::CaptchaRequired(CaptchaChallenge {
            captcha_service,
            captcha_sitekey,
            captcha_rqdata,
            captcha_rqtoken,
        });
    }

    let (code, message) = extract_discord_code_and_message(body, "Invalid credentials");
    LoginError::InvalidCredentials { code, message }
}

/// Извлекает `code` и `message` из JSON-тела ошибки Discord.
fn extract_discord_code_and_message(body: &[u8], fallback_message: &str) -> (Option<i64>, String) {
    let Ok(value) = serde_json::from_slice::<Value>(body) else {
        return (None, fallback_message.to_string());
    };
    let code = value.get("code").and_then(Value::as_i64);
    let message = value
        .get("message")
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(fallback_message)
        .to_string();
    (code, message)
}
