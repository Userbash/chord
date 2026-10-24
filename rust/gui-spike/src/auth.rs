//! BFF авторизации (RW-39 / NET-C2 + NET-F1): импорт токена, статус, logout,
//! keyring и интерактивный вход по логину/паролю (ADR-3).
//!
//! Импорт токена — `ADR-2-auth.md` (вариант A): токен вводится в UI, сразу уходит
//! в keyring и **никогда** не попадает в логи, JSON состояния и тексты ошибок.
//! Заголовок `Authorization` — сырой токен без `Bearer` (`AUTH-RE.md` H-01).
//!
//! Вход по паролю — `ADR-3-login.md`: штатные `/auth/login`, `/auth/mfa/{method}`,
//! `/auth/mfa/sms/send`; captcha решает пользователь в штатном виджете (никаких
//! solver'ов/обхода), 2FA-код вводит пользователь. Пароль и код живут только в
//! памяти на один запрос и затираются best-effort; в keyring/логах/Debug/ошибках
//! их нет. Логин/MFA/SMS не повторяются автоматически (ADR-3: без «долбёжки»).
//!
//! Проверка токена — `GET /users/@me` (A-15): `ApiAuth::from_token` (6 заголовков,
//! `X-Super-Properties` из [`identity::build`]) → `ApiClient::get` → [`parse_me`].
//! 401 → [`AuthError::InvalidToken`], 429 → [`AuthError::RateLimited`], транспорт →
//! [`AuthError::Network`], тело не разбирается → [`AuthError::Parse`].
//!
//! Разделение состояний (ADR-2 §4): [`AuthBackend::status`] читает **только** keyring и
//! не ходит в сеть; сеть — за явной командой [`AuthBackend::restore`].
//!
//! Keyring `!Send` (`discord-platform::keyring`), поэтому [`Keyring`] живёт в
//! выделенном потоке-владельце [`ThreadedStore`]: команды идут через канал, поток
//! создаёт соединение с Secret Service лениво, при первом запросе. Вызывающий поток
//! ждёт ответа канала не дольше [`KEYRING_TIMEOUT`] (SEC-01), а Tauri-команды (`main.rs`)
//! выполняют эти блокирующие вызовы в `spawn_blocking` — UI-поток не блокируется.
//!
//! PLAT-01: мьютекс состояния покрывает только снимок [`AuthState`]; keyring I/O и
//! сетевой `/users/@me` выполняются без него, а результат применяется по поколению
//! сессии (`Inner::generation`), чтобы не перезаписать чужой logout/login.

use std::fmt;
use std::sync::{mpsc, Arc, Mutex, MutexGuard};
use std::thread::{self, JoinHandle};
use std::time::Duration;

use discord_core::rest::client::{
    ApiAuth, ApiClient, ApiConfig, ApiError, ApiRetryPolicy, AuthScheme, DEBUG_OPTIONS_LIVE,
};
use discord_core::rest::identity::{self, IdentityConfig, SystemSources};
use discord_core::rest::rate_limit::rate_limit_info;
use discord_core::rest::super_properties::SuperProperties;
use discord_core::rest::transport::{HttpClientConfig, HttpResponse};
use discord_core::{parse_me, Me, MeError, API_V9_BASE};
use discord_platform::keyring::{Keyring, KeyringError};
use serde::ser::SerializeStruct;
use serde::{Serialize, Serializer};

/// Маршрут проверки токена (`AUTH-RE.md` A-15).
pub const ME_PATH: &str = "/users/@me";
/// Ключ записи в keyring: одна схема `com.discordlab.Token`, атрибут `key`.
pub const TOKEN_KEY: &str = "token";
/// Предел длины текста ошибки: сообщения ядра/транспорта короткие, а хвост не нужен.
const MAX_ERROR_CHARS: usize = 300;
/// SEC-01: предел ожидания ответа keyring-потока. Зависший/заблокированный Secret Service
/// не должен держать вызывающего (и `spawn_blocking`) вечно.
pub const KEYRING_TIMEOUT: Duration = Duration::from_secs(15);

// ---------------------------------------------------------------------------
// Состояние для фронтенда
// ---------------------------------------------------------------------------

/// Состояние авторизации: ровно четыре фазы (`ADR-2-auth.md` §5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum AuthStatus {
    /// Токена нет (или есть в keyring, но сессия не восстановлена).
    Idle,
    /// Идёт проверка `/users/@me` (промежуточное состояние login/restore).
    Validating,
    /// Токен проверен, сессия активна.
    Ready,
    /// Ошибка последней операции — подробности в [`AuthState::error`].
    Error,
}

/// Пользователь из `/users/@me` в форме для UI (`Me` ядра не сериализуется).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthUser {
    /// ID пользователя (снежинка строкой).
    pub id: String,
    /// Имя пользователя.
    pub username: String,
    /// Дискриминатор (`"0"` у новых аккаунтов).
    pub discriminator: String,
    /// Отображаемое имя (nullable).
    pub global_name: Option<String>,
    /// Хеш аватара (nullable).
    pub avatar: Option<String>,
    /// Бот ли аккаунт.
    pub bot: bool,
}

impl From<&Me> for AuthUser {
    fn from(me: &Me) -> Self {
        Self {
            id: me.id.clone(),
            username: me.username.clone(),
            discriminator: me.discriminator.clone(),
            global_name: me.global_name.clone(),
            avatar: me.avatar.clone(),
            bot: me.bot,
        }
    }
}

/// Типизированная ошибка авторизации. Ни один вариант не содержит токен,
/// пароль или 2FA-код.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AuthError {
    /// Пустой (или из одних пробелов) ввод — в сеть и keyring не уходит.
    EmptyToken,
    /// Пустой логин/пароль/код — в сеть не уходит.
    EmptyCredentials,
    /// 401/`50018`: неверный логин или пароль.
    InvalidCredentials,
    /// MFA: сервер отклонил код (`60006`/`60008`/`60009`).
    MfaRejected {
        /// Discord error code из тела ответа (не 2FA-код пользователя).
        code: Option<i64>,
    },
    /// Прочий отказ входа (например, `70007`, `20011`, `20013`).
    LoginRejected {
        /// HTTP-статус ответа.
        status: u16,
        /// Discord error code из тела ответа, если был.
        code: Option<i64>,
    },
    /// 401/тело `401: Unauthorized`: токен недействителен или истёк.
    InvalidToken,
    /// 429 после исчерпания повторов: повторить через `retry_after_ms`.
    RateLimited {
        /// Задержка из ответа Discord, мс.
        retry_after_ms: u64,
    },
    /// Транспорт/таймаут/5xx: сеть недоступна или сервер не ответил.
    Network {
        /// Текст без секретов (обрезан, токен вырезан).
        message: String,
    },
    /// Keyring: соединение, блокировка коллекции, отказ записи.
    Keyring {
        /// Причина от `discord-platform`/потока-владельца (без значения секрета).
        message: String,
    },
    /// Ответ `/users/@me` не разобрался (не JSON/нет обязательных полей).
    Parse {
        /// Позиция/поле из парсера, без токена.
        message: String,
    },
    /// Ошибка конфигурации/платформы (неверная локаль, поток keyring не запустился).
    Internal {
        /// Причина.
        message: String,
    },
}

impl AuthError {
    /// Стабильное имя варианта для JSON/логов и фронтенда.
    pub fn kind(&self) -> &'static str {
        match self {
            Self::EmptyToken => "EmptyToken",
            Self::EmptyCredentials => "EmptyCredentials",
            Self::InvalidCredentials => "InvalidCredentials",
            Self::MfaRejected { .. } => "MfaRejected",
            Self::LoginRejected { .. } => "LoginRejected",
            Self::InvalidToken => "InvalidToken",
            Self::RateLimited { .. } => "RateLimited",
            Self::Network { .. } => "Network",
            Self::Keyring { .. } => "Keyring",
            Self::Parse { .. } => "Parse",
            Self::Internal { .. } => "Internal",
        }
    }

    /// Текст для UI: по-русски, без токена (см. [`sanitize`]).
    fn message(&self) -> String {
        match self {
            Self::EmptyToken => "токен не введён".to_string(),
            Self::EmptyCredentials => "логин или пароль не введены".to_string(),
            Self::InvalidCredentials => "неверный логин или пароль (401)".to_string(),
            Self::MfaRejected { code } => match code {
                Some(code) => format!("код 2FA не принят (код ошибки {code})"),
                None => "код 2FA не принят".to_string(),
            },
            Self::LoginRejected { status, code } => match code {
                Some(code) => format!("вход отклонён (HTTP {status}, код ошибки {code})"),
                None => format!("вход отклонён (HTTP {status})"),
            },
            Self::InvalidToken => "токен недействителен или истёк (401)".to_string(),
            Self::RateLimited { retry_after_ms } => {
                format!("слишком много запросов: повторить через {retry_after_ms} мс (429)")
            }
            Self::Network { message } => format!("сеть недоступна: {message}"),
            Self::Keyring { message } => format!("keyring: {message}"),
            Self::Parse { message } => format!("ответ /users/@me не разобран: {message}"),
            Self::Internal { message } => format!("внутренняя ошибка авторизации: {message}"),
        }
    }
}

impl fmt::Display for AuthError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message())
    }
}

impl std::error::Error for AuthError {}

/// JSON ошибки: `{kind, message, retry_after_ms?, code?}` — как `data::DataError`.
impl Serialize for AuthError {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let mut state = serializer.serialize_struct("AuthError", 3)?;
        state.serialize_field("kind", self.kind())?;
        state.serialize_field("message", &self.message())?;
        match self {
            Self::RateLimited { retry_after_ms } => {
                state.serialize_field("retry_after_ms", retry_after_ms)?;
            }
            Self::MfaRejected { code } | Self::LoginRejected { code, .. } => {
                state.serialize_field("code", code)?;
            }
            _ => {}
        }
        state.end()
    }
}

/// Снимок авторизации для `auth_status`/событий `auth://state`.
///
/// `stored` — в keyring есть токен (признак «можно восстановить сессию»); самого
/// значения здесь нет и быть не может.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AuthState {
    /// Фаза.
    pub status: AuthStatus,
    /// Пользователь, если сессия `Ready`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<AuthUser>,
    /// Ошибка последней операции, если фаза `Error`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<AuthError>,
    /// Есть ли токен в keyring (без сети).
    pub stored: bool,
}

/// Изолированный эмиттер событий авторизации; Tauri адаптер реализуется в `main.rs`.
pub trait AuthStateEmitter: Send + Sync {
    fn emit_state(&self, state: &AuthState) -> Result<(), String>;
}

/// Отправить снимок состояния без секретов в `auth://state`.
pub fn emit_auth_state(emitter: &dyn AuthStateEmitter, state: &AuthState) -> Result<(), String> {
    emitter.emit_state(state)
}

impl AuthState {
    /// Токена нет; `stored` — известное состояние keyring.
    pub fn idle(stored: bool) -> Self {
        Self {
            status: AuthStatus::Idle,
            user: None,
            error: None,
            stored,
        }
    }

    /// Проверка токена в процессе.
    pub fn validating() -> Self {
        Self {
            status: AuthStatus::Validating,
            user: None,
            error: None,
            stored: false,
        }
    }

    /// Сессия активна.
    pub fn ready(user: AuthUser, stored: bool) -> Self {
        Self {
            status: AuthStatus::Ready,
            user: Some(user),
            error: None,
            stored,
        }
    }

    /// Ошибка операции (сессии нет).
    pub fn error(error: AuthError) -> Self {
        Self {
            status: AuthStatus::Error,
            user: None,
            error: Some(error),
            stored: false,
        }
    }
}

// ---------------------------------------------------------------------------
// Текст ошибок без секретов
// ---------------------------------------------------------------------------

/// Обрезка по символам, а не по байтам (UTF-8 не рвётся).
fn truncate(text: &str) -> String {
    if text.chars().count() <= MAX_ERROR_CHARS {
        return text.to_string();
    }
    let mut truncated: String = text.chars().take(MAX_ERROR_CHARS).collect();
    truncated.push('…');
    truncated
}

/// Вырезает токен из произвольного текста ошибки и обрезает хвост.
///
/// Защита «на всякий случай»: ни `ApiError`, ни `MeError` токен не печатают, но
/// сообщение транспорта может прийти из внешнего источника — токен в нём недопустим.
fn sanitize(text: &str, token: &str) -> String {
    let token = token.trim();
    let redacted = if token.is_empty() {
        text.to_string()
    } else {
        text.replace(token, "<redacted>")
    };
    truncate(&redacted)
}

/// `ApiError` ядра → [`AuthError`] (401/429/транспорт), текст без токена.
pub fn map_api_error(error: &ApiError, token: &str) -> AuthError {
    match error {
        ApiError::Unauthorized => AuthError::InvalidToken,
        ApiError::RateLimited { retry_after, .. } => AuthError::RateLimited {
            retry_after_ms: u64::try_from(retry_after.as_millis()).unwrap_or(u64::MAX),
        },
        other => AuthError::Network {
            message: sanitize(&other.to_string(), token),
        },
    }
}

/// `MeError` ядра → [`AuthError`]: тело `401` — недействительный токен, остальное — parse.
pub fn map_me_error(error: &MeError, token: &str) -> AuthError {
    match error {
        MeError::Unauthorized => AuthError::InvalidToken,
        other => AuthError::Parse {
            message: sanitize(&other.to_string(), token),
        },
    }
}

/// Разбор тела `/users/@me` с маппингом ошибок (тело — данные, не сеть).
pub fn parse_me_body(body: &[u8], token: &str) -> Result<Me, AuthError> {
    parse_me(body).map_err(|error| map_me_error(&error, token))
}

// ---------------------------------------------------------------------------
// Identity: super-properties, локаль, таймзона
// ---------------------------------------------------------------------------

/// Значения identity для auth-заголовков: считаются один раз на приложение.
#[derive(Debug, Clone)]
pub struct IdentityContext {
    /// `X-Super-Properties` (22 поля, честные значения — RW-09).
    pub super_properties: SuperProperties,
    /// `X-Discord-Locale`/`Accept-Language`.
    pub locale: String,
    /// `X-Discord-Timezone` (IANA).
    pub timezone: String,
}

impl IdentityContext {
    /// Контекст с заданными локалью/таймзоной (супер-свойства — из среды).
    ///
    /// Пустые аргументы заменяются на `en-US`/`UTC`: `ApiAuth::from_token` не принимает
    /// пустых заголовков, а падать из-за окружения нельзя.
    pub fn new(locale: &str, timezone: &str) -> Self {
        let locale = if locale.trim().is_empty() {
            "en-US".to_string()
        } else {
            locale.trim().to_string()
        };
        let timezone = if timezone.trim().is_empty() {
            "UTC".to_string()
        } else {
            timezone.trim().to_string()
        };
        // Локаль приложения совпадает с system_locale: иначе заголовки разойдутся.
        let config = IdentityConfig {
            system_locale: Some(locale.clone()),
            ..IdentityConfig::default()
        };
        let super_properties = identity::build(&SystemSources, &config);
        Self {
            super_properties,
            locale,
            timezone,
        }
    }

    /// Контекст из среды: локаль — `identity::build`, таймзона — `TZ`/`/etc/timezone`.
    pub fn detect() -> Self {
        let super_properties = identity::build(&SystemSources, &IdentityConfig::default());
        Self {
            locale: super_properties.system_locale.clone(),
            timezone: detect_timezone(),
            super_properties,
        }
    }
}

/// IANA-таймзона: `TZ`, затем `/etc/timezone`, иначе `UTC`.
fn detect_timezone() -> String {
    if let Ok(tz) = std::env::var("TZ") {
        if !tz.trim().is_empty() {
            return tz.trim().to_string();
        }
    }
    if let Ok(text) = std::fs::read_to_string("/etc/timezone") {
        if let Some(line) = text.lines().map(str::trim).find(|line| !line.is_empty()) {
            return line.to_string();
        }
    }
    "UTC".to_string()
}

// ---------------------------------------------------------------------------
// Вход по паролю: контракт /auth/* (ADR-3, AUTH-RE A-02…A-08, H-01, E-05…E-09)
// ---------------------------------------------------------------------------

/// Вход паролем (`AUTH-RE.md` A-02).
pub const LOGIN_PATH: &str = "/auth/login";
/// Префикс шага MFA; метод добавляется хвостом (A-05).
pub const MFA_PATH_PREFIX: &str = "/auth/mfa/";
/// Отправка SMS-кода (A-07).
pub const MFA_SMS_SEND_PATH: &str = "/auth/mfa/sms/send";
/// Заголовок ответа на captcha (E-05).
pub const CAPTCHA_KEY_HEADER: &str = "X-Captcha-Key";
/// Request-token captcha из ответа `/auth/login` (E-05/NF-05).
pub const CAPTCHA_RQTOKEN_HEADER: &str = "X-Captcha-Rqtoken";
/// Session id captcha для следующего запроса (CAP-04).
pub const CAPTCHA_SESSION_ID_HEADER: &str = "X-Captcha-Session-Id";
/// Дополнительные данные captcha для следующего запроса (CAP-04).
pub const CAPTCHA_RQDATA_HEADER: &str = "X-Captcha-Rqdata";

/// Методы MFA, которые принимает `submit_mfa` (A-06). Whitelist — часть контракта:
/// метод подставляется в путь, поэтому произвольная строка туда не попадает.
const MFA_METHODS: [&str; 5] = ["totp", "sms", "backup", "webauthn", "password"];

/// Исход одного шага входа — состояние machine без хранения пароля/кода.
///
/// `Token` содержит уже проверенный и сохранённый в keyring токен: он нужен backend'у
/// и тестам, но **не** отдаётся во фронтенд (см. [`LoginView`]). `Debug` маскирует
/// токен, ticket и captcha-rqtoken.
#[derive(Clone, PartialEq, Eq)]
pub enum LoginOutcome {
    /// Успешный вход: токен проверен (`/users/@me`) и записан в keyring.
    Token(String),
    /// Нужен второй фактор: `ticket` и доступные методы (`totp`, `sms`, …).
    Mfa {
        /// Тикет попытки входа (A-04/A-05), нужен для `submit_mfa`.
        ticket: String,
        /// Методы из `mfa.methods[]`/полей ответа (`totp`, `sms`, `backup`, …).
        methods: Vec<String>,
        /// Сквозной id попытки; обязателен в запросе MFA, если сервер его выдал.
        login_instance_id: Option<String>,
    },
    /// Нужна captcha: штатный виджет рисует UI, ответ пользователя уходит заголовком.
    Captcha {
        /// `captcha_sitekey`/`captcha_key` из ответа.
        sitekey: String,
        /// Сервис (`hcaptcha`, `recaptcha`, `turnstile`); по умолчанию `hcaptcha`.
        service: String,
        /// Идентификатор captcha-сессии для заголовка следующего запроса.
        captcha_session_id: Option<String>,
        /// Дополнительные данные captcha для заголовка следующего запроса.
        captcha_rqdata: Option<String>,
        /// `captcha_rqtoken` для заголовка следующего запроса.
        captcha_rqtoken: Option<String>,
        /// Нужно ли показывать невидимый виджет.
        should_serve_invisible: bool,
        /// Сценарий входа, переданный Discord.
        user_flow: Option<String>,
    },
    /// Ошибка шага; пароля/кода/токена в ней нет.
    Error(AuthError),
}

impl fmt::Debug for LoginOutcome {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Token(_) => formatter.debug_tuple("Token").field(&"<redacted>").finish(),
            Self::Mfa {
                ticket: _,
                methods,
                login_instance_id,
            } => formatter
                .debug_struct("Mfa")
                .field("ticket", &"<redacted>")
                .field("methods", methods)
                .field("login_instance_id", login_instance_id)
                .finish(),
            Self::Captcha {
                sitekey,
                service,
                captcha_session_id: _,
                captcha_rqdata: _,
                captcha_rqtoken: _,
                should_serve_invisible,
                user_flow,
            } => formatter
                .debug_struct("Captcha")
                .field("sitekey", sitekey)
                .field("service", service)
                .field("captcha_session_id", &"<redacted>")
                .field("captcha_rqdata", &"<redacted>")
                .field("captcha_rqtoken", &"<redacted>")
                .field("should_serve_invisible", should_serve_invisible)
                .field(
                    "user_flow",
                    if user_flow.is_some() {
                        &"<redacted>"
                    } else {
                        &"<none>"
                    },
                )
                .finish(),
            Self::Error(error) => formatter.debug_tuple("Error").field(error).finish(),
        }
    }
}

/// Ответ `/auth/mfa/sms/send`: номер показывается маскированно (A-07).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SmsChallenge {
    /// `phone` из ответа; `None`, если сервер его не прислал.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub phone: Option<String>,
}

/// Ответ Tauri-команд входа: то, что можно показать фронтенду.
///
/// Токена здесь нет: успешный вход уже сохранён в keyring (`Ready`). `ticket`,
/// `sitekey` и `rqtoken` — не учётные данные пользователя, а контекст следующего
/// шага; они нужны UI (MFA-экран/captcha-виджет).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum LoginView {
    /// Токен получен, проверен и лежит в keyring.
    Ready {
        /// Пользователь из `/users/@me`.
        user: AuthUser,
    },
    /// Нужен код 2FA.
    Mfa {
        /// Тикет попытки входа.
        ticket: String,
        /// Доступные методы.
        methods: Vec<String>,
        /// Сквозной id попытки для `auth_mfa`.
        login_instance_id: Option<String>,
    },
    /// Нужна интерактивная captcha.
    Captcha {
        /// Sitekey для штатного виджета.
        sitekey: String,
        /// Сервис captcha.
        service: String,
        /// Идентификатор captcha-сессии.
        captcha_session_id: Option<String>,
        /// Дополнительные данные captcha.
        captcha_rqdata: Option<String>,
        /// Request-token для заголовка.
        captcha_rqtoken: Option<String>,
        /// Нужно ли показывать невидимый виджет.
        should_serve_invisible: bool,
        /// Сценарий входа.
        user_flow: Option<String>,
    },
}

impl LoginView {
    /// Превратить исход в ответ UI; ошибка уходит как `Err` (без секретов).
    ///
    /// # Errors
    ///
    /// [`LoginOutcome::Error`]; также [`AuthError::Internal`], если `Token` пришёл без
    /// `Ready`-состояния (инвариант backend'а нарушен).
    pub fn from_outcome(outcome: LoginOutcome, state: &AuthState) -> Result<Self, AuthError> {
        match outcome {
            LoginOutcome::Token(_) => {
                let user = state.user.clone().ok_or_else(|| AuthError::Internal {
                    message: "успешный вход не завершил сессию".to_string(),
                })?;
                Ok(Self::Ready { user })
            }
            LoginOutcome::Mfa {
                ticket,
                methods,
                login_instance_id,
            } => Ok(Self::Mfa {
                ticket,
                methods,
                login_instance_id,
            }),
            LoginOutcome::Captcha {
                sitekey,
                service,
                captcha_session_id,
                captcha_rqdata,
                captcha_rqtoken,
                should_serve_invisible,
                user_flow,
            } => Ok(Self::Captcha {
                sitekey,
                service,
                captcha_session_id,
                captcha_rqdata,
                captcha_rqtoken,
                should_serve_invisible,
                user_flow,
            }),
            LoginOutcome::Error(error) => Err(error),
        }
    }
}

/// Транспорт `/auth/*`: один POST, без ретраев (ADR-3) и без классификации статуса.
///
/// Продакшн — [`DiscordLogin`] поверх `ApiClient::post_once`; тесты — fake без сети.
pub trait LoginTransport: Send + Sync {
    /// POST JSON-тела по пути; 4xx/5xx отдаются как `Ok(HttpResponse)` — это данные
    /// (captcha/MFA-коды), а не транспортная ошибка.
    fn post_json(
        &self,
        path: &str,
        body: &[u8],
        headers: &[(&str, &str)],
    ) -> Result<HttpResponse, ApiError>;
}

/// Боевой транспорт: TLS к `discord.com/api/v9`, identity-заголовки, ровно одна попытка.
#[derive(Debug, Clone)]
pub struct DiscordLogin {
    base: String,
    identity: IdentityContext,
}

impl DiscordLogin {
    /// Боевая база и identity из среды.
    pub fn new() -> Self {
        Self::with_identity(IdentityContext::detect())
    }

    /// Боевая база с заданной identity (детерминированные тесты/спайк).
    pub fn with_identity(identity: IdentityContext) -> Self {
        Self {
            base: API_V9_BASE.to_string(),
            identity,
        }
    }

    /// Другая база (локальный стаб).
    pub fn with_base_and_identity(base: impl Into<String>, identity: IdentityContext) -> Self {
        Self {
            base: base.into(),
            identity,
        }
    }
}

impl Default for DiscordLogin {
    fn default() -> Self {
        Self::new()
    }
}

impl LoginTransport for DiscordLogin {
    /// `post_once`: 400 + `captcha_key` и коды MFA должны дойти до вызывающего с телом;
    /// повторов нет (`ADR-3`).
    fn post_json(
        &self,
        path: &str,
        body: &[u8],
        headers: &[(&str, &str)],
    ) -> Result<HttpResponse, ApiError> {
        // Логин идёт без унаследованного токена: пустой `authorization` ставит
        // вызывающий (A-03), а auth-шапка клиента содержит только identity-заголовки.
        let auth = ApiAuth {
            authorization: String::new(),
            super_properties: self.identity.super_properties.to_base64(),
            locale: self.identity.locale.clone(),
            timezone: self.identity.timezone.clone(),
            debug_options: DEBUG_OPTIONS_LIVE.to_string(),
            accept_language: self.identity.locale.clone(),
        };
        let config = ApiConfig {
            base: self.base.clone(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy::default(),
        };
        let client = ApiClient::new(config)?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| {
                ApiError::InvalidRequest(format!("не удалось создать runtime: {error}"))
            })?;
        runtime.block_on(client.post_once(path, body, headers))
    }
}

/// JSON-тело `/auth/login` ровно по A-02 (+ ничего лишнего).
fn login_body(login: &str, password: &str) -> Vec<u8> {
    serde_json::json!({
        "login": login,
        "password": password,
        "undelete": false,
        "login_source": null,
        "gift_code_sku_id": null,
    })
    .to_string()
    .into_bytes()
}

/// JSON-тело `/auth/mfa/{method}` по A-05.
fn mfa_body(code: &str, ticket: &str, login_instance_id: Option<&str>) -> Vec<u8> {
    serde_json::json!({
        "code": code,
        "ticket": ticket,
        "login_source": null,
        "gift_code_sku_id": null,
        "login_instance_id": login_instance_id,
    })
    .to_string()
    .into_bytes()
}

/// Best-effort затирание heap-буфера строки (тот же буфер после `into_bytes`).
fn wipe_string(value: &mut String) {
    let mut bytes = std::mem::take(value).into_bytes();
    bytes.fill(0);
}

/// Best-effort затирание тела запроса (содержит пароль/код).
fn wipe_bytes(bytes: &mut [u8]) {
    bytes.fill(0);
}

/// Разбор JSON без паники: не-JSON — `None`.
fn parse_json(body: &[u8]) -> Option<serde_json::Value> {
    serde_json::from_slice(body).ok()
}

/// Строковое поле верхнего уровня.
fn json_string(value: &serde_json::Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .map(str::to_string)
}

/// Методы MFA из `mfa.methods[]`/`methods[]` и булевых полей (`totp`, `sms`, …).
fn mfa_methods(value: &serde_json::Value) -> Vec<String> {
    let mut methods: Vec<String> = Vec::new();
    let arrays = value
        .get("methods")
        .and_then(serde_json::Value::as_array)
        .or_else(|| {
            value
                .get("mfa")
                .and_then(|mfa| mfa.get("methods"))
                .and_then(serde_json::Value::as_array)
        });
    if let Some(items) = arrays {
        for item in items {
            if let Some(method) = item.get("type").and_then(serde_json::Value::as_str) {
                push_mfa_method(&mut methods, method);
            }
        }
    }
    for method in MFA_METHODS {
        if value
            .get(method)
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
        {
            push_mfa_method(&mut methods, method);
        }
    }
    methods
}

/// Добавить метод в список, если он в whitelist и ещё не добавлен.
fn push_mfa_method(methods: &mut Vec<String>, method: &str) {
    if MFA_METHODS.contains(&method) && !methods.iter().any(|known| known == method) {
        methods.push(method.to_string());
    }
}

/// `retry_after` из 429 (заголовок → тело → `x-ratelimit-reset-after`), мс.
fn retry_after_ms(status: u16, headers: &[(String, String)], body: &[u8]) -> u64 {
    rate_limit_info(status, headers, body)
        .map(|info| u64::try_from(info.retry_after.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or(0)
}

/// Разбор ответа `/auth/*` в состояние machine.
///
/// Порядок важен: 429 → captcha (400 + `captcha_key`) → MFA → token → коды ошибок.
/// Ни текст тела, ни значения полей не попадают в [`AuthError`].
fn parse_login_response(status: u16, headers: &[(String, String)], body: &[u8]) -> LoginOutcome {
    if status == 429 {
        return LoginOutcome::Error(AuthError::RateLimited {
            retry_after_ms: retry_after_ms(status, headers, body),
        });
    }

    let value = parse_json(body);
    if let Some(value) = &value {
        if (200..300).contains(&status) {
            if let Some(token) =
                json_string(value, "token").filter(|token| !token.trim().is_empty())
            {
                return LoginOutcome::Token(token.trim().to_string());
            }
        }
        if let Some(sitekey) =
            json_string(value, "captcha_sitekey").or_else(|| json_string(value, "captcha_key"))
        {
            return LoginOutcome::Captcha {
                sitekey,
                service: json_string(value, "captcha_service")
                    .unwrap_or_else(|| "hcaptcha".to_string()),
                captcha_session_id: json_string(value, "captcha_session_id"),
                captcha_rqdata: json_string(value, "captcha_rqdata"),
                captcha_rqtoken: json_string(value, "captcha_rqtoken"),
                should_serve_invisible: value
                    .get("should_serve_invisible")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false),
                user_flow: json_string(value, "user_flow"),
            };
        }
        let mfa = value.get("mfa");
        let has_mfa = mfa.and_then(serde_json::Value::as_bool) == Some(true)
            || mfa.and_then(|mfa| mfa.get("methods")).is_some();
        if has_mfa {
            return LoginOutcome::Mfa {
                ticket: json_string(value, "ticket").unwrap_or_default(),
                methods: mfa_methods(value),
                login_instance_id: json_string(value, "login_instance_id"),
            };
        }
    }

    let code = value
        .as_ref()
        .and_then(|value| value.get("code"))
        .and_then(serde_json::Value::as_i64);
    // Коды MFA важнее статуса: неверный 2FA-код приходит как 401 + code 60008.
    if let Some(code @ (60006 | 60008 | 60009)) = code {
        return LoginOutcome::Error(AuthError::MfaRejected { code: Some(code) });
    }
    if status == 401 || code == Some(50018) {
        return LoginOutcome::Error(AuthError::InvalidCredentials);
    }
    LoginOutcome::Error(AuthError::LoginRejected { status, code })
}

/// `ApiError` транспорта входа → [`AuthError`] без секретов.
fn map_login_api_error(error: &ApiError) -> AuthError {
    match error {
        ApiError::RateLimited { retry_after, .. } => AuthError::RateLimited {
            retry_after_ms: u64::try_from(retry_after.as_millis()).unwrap_or(u64::MAX),
        },
        other => AuthError::Network {
            message: truncate(&other.to_string()),
        },
    }
}

// ---------------------------------------------------------------------------
// Валидатор токена
// ---------------------------------------------------------------------------

/// Вход проверки: токен и identity-заголовки.
///
/// Ручной `Debug` вырезает токен: структура может попасть в лог по ошибке, а секрет
/// не должен печататься даже тогда.
#[derive(Clone)]
pub struct ValidationRequest {
    /// Сырой user-токен (передаётся только в `ApiAuth`; не логируется).
    pub token: String,
    /// `X-Super-Properties`.
    pub super_properties: SuperProperties,
    /// `X-Discord-Locale`.
    pub locale: String,
    /// `X-Discord-Timezone`.
    pub timezone: String,
}

impl fmt::Debug for ValidationRequest {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ValidationRequest")
            .field("token", &"<redacted>")
            .field("super_properties", &self.super_properties)
            .field("locale", &self.locale)
            .field("timezone", &self.timezone)
            .finish()
    }
}

/// Проверка токена. Продакшн — [`DiscordValidator`], тесты — фейк (без сети).
pub trait TokenValidator: Send + Sync {
    /// `GET /users/@me` → пользователь либо типизированная ошибка.
    fn validate(&self, request: &ValidationRequest) -> Result<Me, AuthError>;
}

/// Настоящая проверка через `discord-core` (TLS к `discord.com`, ADR-2 §2).
#[derive(Debug, Clone)]
pub struct DiscordValidator {
    base: String,
}

impl DiscordValidator {
    /// Клиент на боевой базе `https://discord.com/api/v9`.
    pub fn new() -> Self {
        Self {
            base: API_V9_BASE.to_string(),
        }
    }

    /// Клиент на другой базе (локальный стаб в интеграционном тесте).
    pub fn with_base(base: impl Into<String>) -> Self {
        Self { base: base.into() }
    }
}

impl Default for DiscordValidator {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenValidator for DiscordValidator {
    fn validate(&self, request: &ValidationRequest) -> Result<Me, AuthError> {
        let auth = ApiAuth::from_token_with_scheme(
            &request.token,
            AuthScheme::User,
            &request.super_properties,
            &request.locale,
            &request.timezone,
        )
        .map_err(|error| AuthError::Internal {
            message: sanitize(&error.to_string(), &request.token),
        })?;

        let config = ApiConfig {
            base: self.base.clone(),
            auth,
            http: HttpClientConfig::default(),
            retry: ApiRetryPolicy::default(),
        };
        let client =
            ApiClient::new(config).map_err(|error| map_api_error(&error, &request.token))?;

        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| AuthError::Internal {
                message: sanitize(
                    &format!("не удалось создать runtime: {error}"),
                    &request.token,
                ),
            })?;

        let response = runtime
            .block_on(client.get(ME_PATH))
            .map_err(|error| map_api_error(&error, &request.token))?;
        parse_me_body(&response.body, &request.token)
    }
}

// ---------------------------------------------------------------------------
// Хранилище токена: trait + поток-владелец
// ---------------------------------------------------------------------------

/// Хранилище токена. Сам трейт намеренно **без** `Send`: `Keyring` — `!Send + !Sync`
/// (`libsecret`/GDBus), поэтому объект создаётся и живёт внутри потока-владельца
/// [`ThreadedStore`]. Наружу (в состояние Tauri) выходит только `Box<dyn TokenStore + Send>`.
pub trait TokenStore {
    /// Токен или `None`, если записи нет.
    fn load(&mut self) -> Result<Option<String>, String>;
    /// Записать (перезапись допустима).
    fn store(&mut self, token: &str) -> Result<(), String>;
    /// Удалить; `true` — запись была, `false` — уже отсутствовала (идемпотентно).
    fn clear(&mut self) -> Result<bool, String>;
}

/// Продакшн-хранилище поверх `discord-platform::Keyring` (живёт в потоке-владельце).
struct KeyringStore {
    ring: Keyring,
}

impl TokenStore for KeyringStore {
    fn load(&mut self) -> Result<Option<String>, String> {
        self.ring
            .lookup(TOKEN_KEY)
            .map_err(|error| keyring_text(&error))
    }

    fn store(&mut self, token: &str) -> Result<(), String> {
        self.ring
            .store(TOKEN_KEY, token)
            .map_err(|error| keyring_text(&error))
    }

    fn clear(&mut self) -> Result<bool, String> {
        self.ring
            .clear(TOKEN_KEY)
            .map_err(|error| keyring_text(&error))
    }
}

/// Текст ошибки keyring: `KeyringError` не содержит значения секрета (инвариант крейта).
fn keyring_text(error: &KeyringError) -> String {
    truncate(&error.to_string())
}

/// Запрос к потоку-владельцу; ответ — по отдельному каналу (один запрос — один ответ).
enum Request {
    Load(mpsc::Sender<Result<Option<String>, String>>),
    Store(String, mpsc::Sender<Result<(), String>>),
    Clear(mpsc::Sender<Result<bool, String>>),
}

/// Владелец хранилища в выделенном потоке.
///
/// Зачем: `Keyring` — `!Send + !Sync` (libsecret/GDBus-сервис нельзя переносить между
/// потоками). Поток создаёт хранилище сам и обслуживает запросы через канал; снаружи
/// тип `Send`, поэтому его можно держать в состоянии Tauri.
pub struct ThreadedStore {
    sender: Option<mpsc::Sender<Request>>,
    handle: Option<JoinHandle<()>>,
    /// Поток не запустился (исчерпаны ресурсы ОС) — каждая операция вернёт эту причину.
    spawn_error: Option<String>,
    /// SEC-01: предел ожидания ответа от потока-владельца.
    timeout: Duration,
}

impl ThreadedStore {
    /// Запустить поток-владелец; `open` выполняется **внутри** потока.
    pub fn spawn<F>(open: F) -> Self
    where
        F: FnOnce() -> Result<Box<dyn TokenStore>, String> + Send + 'static,
    {
        let (sender, receiver) = mpsc::channel::<Request>();
        let spawned = thread::Builder::new()
            .name("auth-keyring".to_string())
            .spawn(move || serve(open, receiver));
        match spawned {
            Ok(handle) => Self {
                sender: Some(sender),
                handle: Some(handle),
                spawn_error: None,
                timeout: KEYRING_TIMEOUT,
            },
            Err(error) => Self {
                sender: None,
                handle: None,
                spawn_error: Some(truncate(&format!("поток keyring не запустился: {error}"))),
                timeout: KEYRING_TIMEOUT,
            },
        }
    }

    /// Предел ожидания ответа (тесты — короткий, чтобы не ждать 15 с).
    #[must_use]
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// Продакшн-хранилище: `Keyring` поверх Secret Service, ленивое подключение.
    pub fn keyring() -> Self {
        Self::spawn(|| {
            let ring = Keyring::connect().map_err(|error| keyring_text(&error))?;
            Ok(Box::new(KeyringStore { ring }) as Box<dyn TokenStore>)
        })
    }

    /// Один запрос к потоку-владельцу (создание канала ответа здесь, ожидание — у вызывающего).
    ///
    /// Ожидание ограничено [`ThreadedStore::timeout`]: зависший Secret Service (заблокированная
    /// коллекция, prompt) даёт ошибку, а не вечное ожидание. Цена — операция становится
    /// неопределённой: запрос остался в очереди и может выполниться позже (SEC-04).
    fn call<T>(
        &self,
        build: impl FnOnce(mpsc::Sender<Result<T, String>>) -> Request,
    ) -> Result<T, String> {
        if let Some(error) = &self.spawn_error {
            return Err(error.clone());
        }
        let Some(sender) = self.sender.as_ref() else {
            return Err("поток keyring не запущен".to_string());
        };
        let (reply, receiver) = mpsc::channel();
        sender
            .send(build(reply))
            .map_err(|_| "поток keyring завершился".to_string())?;
        match receiver.recv_timeout(self.timeout) {
            Ok(result) => result,
            Err(mpsc::RecvTimeoutError::Timeout) => Err(timeout_text(self.timeout)),
            Err(mpsc::RecvTimeoutError::Disconnected) => {
                Err("поток keyring не ответил".to_string())
            }
        }
    }
}

/// Текст таймаута keyring: без секретов и без значений из окружения.
fn timeout_text(timeout: Duration) -> String {
    if timeout >= Duration::from_secs(1) && timeout.subsec_millis() == 0 {
        format!("keyring не ответил за {} с", timeout.as_secs())
    } else {
        format!("keyring не ответил за {} мс", timeout.as_millis())
    }
}

impl TokenStore for ThreadedStore {
    fn load(&mut self) -> Result<Option<String>, String> {
        self.call(Request::Load)
    }

    fn store(&mut self, token: &str) -> Result<(), String> {
        let token = token.to_string();
        self.call(move |reply| Request::Store(token, reply))
    }

    fn clear(&mut self) -> Result<bool, String> {
        self.call(Request::Clear)
    }
}

impl Drop for ThreadedStore {
    /// Закрыть канал и дождаться потока: иначе он переживёт приложение.
    ///
    /// Если поток ждёт разблокировки коллекции (prompt Secret Service), `join` ждёт
    /// столько же — это завершение приложения, а не UI-операция. **Остаток SEC-01:**
    /// таймаут в [`ThreadedStore::call`] не ограничивает этот `join`.
    fn drop(&mut self) {
        self.sender = None;
        if let Some(handle) = self.handle.take() {
            let _ = handle.join(); // поток не возвращает ошибок: причина уходит в ответы
        }
    }
}

/// Тело потока-владельца: создать хранилище и обслуживать запросы до закрытия канала.
fn serve<F>(open: F, receiver: mpsc::Receiver<Request>)
where
    F: FnOnce() -> Result<Box<dyn TokenStore>, String>,
{
    match open() {
        Ok(mut store) => {
            while let Ok(request) = receiver.recv() {
                match request {
                    Request::Load(reply) => {
                        let _ = reply.send(store.load());
                    }
                    Request::Store(token, reply) => {
                        let _ = reply.send(store.store(&token));
                    }
                    Request::Clear(reply) => {
                        let _ = reply.send(store.clear());
                    }
                }
            }
        }
        Err(error) => {
            // Отвечаем на каждый запрос причиной: UI не должен висеть на «мёртвом» потоке.
            while let Ok(request) = receiver.recv() {
                match request {
                    Request::Load(reply) => {
                        let _ = reply.send(Err(error.clone()));
                    }
                    Request::Store(_token, reply) => {
                        let _ = reply.send(Err(error.clone()));
                    }
                    Request::Clear(reply) => {
                        let _ = reply.send(Err(error.clone()));
                    }
                }
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Backend: состояние + операции
// ---------------------------------------------------------------------------

/// Состояние авторизации под мьютексом. Keyring здесь больше не живёт: он за
/// отдельным мьютексом, чтобы keyring I/O и сеть не задерживали `status`/`token`.
struct Inner {
    state: AuthState,
    /// keyring уже читали при старте — повторно не дёргаем.
    keyring_checked: bool,
    /// Поколение сессии (PLAT-01): растёт при старте `login`/`restore` и при
    /// `logout`/коммите. Результат сетевой проверки применяется только к своему
    /// поколению — устаревший не перезаписывает чужой logout/login.
    generation: u64,
}

/// Backend авторизации: Tauri держит его в `Arc` (см. `main.rs`).
pub struct AuthBackend {
    /// Только состояние: никогда не удерживается через keyring или сеть.
    inner: Mutex<Inner>,
    /// Keyring (`TokenStore` не `Sync`) — под собственным мьютексом. Держится
    /// лишь на время вызова хранилища, но никогда — на время `/users/@me`.
    store: Mutex<Box<dyn TokenStore + Send>>,
    /// Сериализует «прочитать/записать keyring + применить состояние»: `logout`
    /// не разъезжается с коммитом `login`. Сетевая проверка сюда не входит.
    commit: Mutex<()>,
    validator: Arc<dyn TokenValidator>,
    identity: IdentityContext,
    login: Arc<dyn LoginTransport>,
}

impl AuthBackend {
    /// Боевой backend: keyring в потоке-владельце, сеть — `discord-core`.
    pub fn keyring() -> Self {
        Self::new(
            Box::new(ThreadedStore::keyring()),
            Arc::new(DiscordValidator::new()),
        )
    }

    /// Backend с identity из среды.
    pub fn new(store: Box<dyn TokenStore + Send>, validator: Arc<dyn TokenValidator>) -> Self {
        Self::with_identity(store, validator, IdentityContext::detect())
    }

    /// Backend с заданными identity-значениями (детерминированные тесты).
    pub fn with_identity(
        store: Box<dyn TokenStore + Send>,
        validator: Arc<dyn TokenValidator>,
        identity: IdentityContext,
    ) -> Self {
        let login = Arc::new(DiscordLogin::with_identity(identity.clone()));
        Self {
            inner: Mutex::new(Inner {
                state: AuthState::idle(false),
                keyring_checked: false,
                generation: 0,
            }),
            store: Mutex::new(store),
            commit: Mutex::new(()),
            validator,
            identity,
            login,
        }
    }

    /// Заменить транспорт входа (тесты — fake без сети).
    #[must_use]
    pub fn with_login_transport(mut self, transport: Arc<dyn LoginTransport>) -> Self {
        self.login = transport;
        self
    }

    fn lock(&self) -> Result<MutexGuard<'_, Inner>, AuthError> {
        self.inner.lock().map_err(|_| AuthError::Internal {
            message: "состояние авторизации отравлено после паники".to_string(),
        })
    }

    /// Мьютекс keyring: отравление — ошибка хранилища, а не состояния.
    fn lock_store(&self) -> Result<MutexGuard<'_, Box<dyn TokenStore + Send>>, AuthError> {
        self.store.lock().map_err(|_| AuthError::Internal {
            message: "хранилище токена отравлено после паники".to_string(),
        })
    }

    fn lock_commit(&self) -> Result<MutexGuard<'_, ()>, AuthError> {
        self.commit.lock().map_err(|_| AuthError::Internal {
            message: "коммит авторизации отравлен после паники".to_string(),
        })
    }

    /// keyring.load под мьютексом хранилища (не состояния).
    fn read_store(&self) -> Result<Option<String>, AuthError> {
        self.lock_store()?
            .load()
            .map_err(|message| AuthError::Keyring { message })
    }

    /// keyring.store под мьютексом хранилища (не состояния).
    fn write_store(&self, token: &str) -> Result<(), AuthError> {
        self.lock_store()?
            .store(token)
            .map_err(|message| AuthError::Keyring { message })
    }

    /// keyring.clear под мьютексом хранилища (не состояния).
    fn clear_store(&self) -> Result<bool, AuthError> {
        self.lock_store()?
            .clear()
            .map_err(|message| AuthError::Keyring { message })
    }

    /// `auth_status`: только keyring (один раз за жизнь backend), **без сети**.
    ///
    /// Keyring читается **до** лока состояния и коммита: даже зависший Secret
    /// Service не задерживает `login`/`logout`. Применение результата идёт под
    /// `commit` и только если keyring ещё никто не проверил — поздний ответ не
    /// воскрешает `stored` после logout.
    pub fn status(&self) -> AuthState {
        // Быстрый путь: keyring уже прочитан — сеть/keyring не нужны.
        match self.lock() {
            Ok(inner) if inner.keyring_checked => return inner.state.clone(),
            Ok(_) => {}
            Err(error) => return AuthState::error(error),
        }
        let loaded = self.read_store();
        let _commit = match self.lock_commit() {
            Ok(commit) => commit,
            Err(error) => return AuthState::error(error),
        };
        let mut inner = match self.lock() {
            Ok(inner) => inner,
            Err(error) => return AuthState::error(error),
        };
        // Пока читали keyring, его уже проверил login/restore/logout — не затираем.
        if inner.keyring_checked {
            return inner.state.clone();
        }
        inner.keyring_checked = true;
        match loaded {
            Ok(token) => inner.state.stored = token.is_some(),
            Err(error) => inner.state = AuthState::error(error),
        }
        inner.state.clone()
    }

    /// Прочитать сохранённый токен из keyring без проверки по сети и без логирования.
    ///
    /// Состояние не читается и не меняется, поэтому мьютекс состояния не берётся.
    pub fn token(&self) -> Result<Option<String>, AuthError> {
        self.read_store()
    }

    /// `auth_login_token`: проверка `/users/@me`, затем запись в keyring.
    ///
    /// Ошибка проверки — токен в keyring не пишется. Ошибка keyring — сессия не
    /// объявляется `Ready` (fail closed): без сохранения вход бессмысленен.
    /// Сеть и keyring вызываются без мьютекса состояния; коммит результата
    /// отменяется, если за это время сессию изменила другая операция.
    pub fn login(&self, token: &str) -> AuthState {
        let token = token.trim();
        if token.is_empty() {
            let _commit = match self.lock_commit() {
                Ok(commit) => commit,
                Err(error) => return AuthState::error(error),
            };
            let mut inner = match self.lock() {
                Ok(inner) => inner,
                Err(error) => return AuthState::error(error),
            };
            inner.keyring_checked = true;
            inner.generation += 1;
            let stored = inner.state.stored;
            inner.state = AuthState::error(AuthError::EmptyToken);
            inner.state.stored = stored;
            return inner.state.clone();
        }
        let generation = match self.begin_validation(true) {
            Ok(generation) => generation,
            Err(error) => return AuthState::error(error),
        };
        self.authenticate(token, true, generation)
    }

    /// `auth_restore`: явное восстановление из keyring — единственное место, где
    /// статус при старте ходит в сеть.
    ///
    /// Keyring читается без `inner`/`commit`; пока идёт чтение и сеть, сессию может
    /// изменить logout/login — тогда результат отбрасывается по поколению.
    pub fn restore(&self) -> AuthState {
        let generation = match self.begin_validation(true) {
            Ok(generation) => generation,
            Err(error) => return AuthState::error(error),
        };
        match self.read_store() {
            Ok(Some(token)) => self.authenticate(&token, false, generation),
            Ok(None) => self.apply_restore(generation, AuthState::idle(false)),
            Err(error) => match self.reject_validation(error, generation) {
                ValidationOutcome::Committed(state)
                | ValidationOutcome::Superseded(state)
                | ValidationOutcome::Rejected(_, state) => state,
            },
        }
    }

    /// Применить «пустой keyring» к состоянию, если поколение не сменилось.
    fn apply_restore(&self, generation: u64, state: AuthState) -> AuthState {
        let _commit = match self.lock_commit() {
            Ok(commit) => commit,
            Err(error) => return AuthState::error(error),
        };
        let mut inner = match self.lock() {
            Ok(inner) => inner,
            Err(error) => return AuthState::error(error),
        };
        if inner.generation != generation {
            return inner.state.clone();
        }
        inner.state = state;
        inner.state.clone()
    }

    /// `auth_logout`: keyring.clear + сброс сессии. Идемпотентен.
    ///
    /// Сессия гасится даже при отказе keyring: иначе UI остался бы в `Ready` без
    /// возможности выйти. Причина отказа приходит в [`AuthState::error`].
    pub fn logout(&self) -> AuthState {
        self.logout_with_disconnect(|| Ok(()))
    }

    /// Остановить сеть до очистки keyring; hook нужен Tauri и unit-тестам.
    ///
    /// `disconnect` вызывается **вне** мьютексов: отключение сети может быть
    /// долгим, а `token`/`status` не должны его ждать. Очистка keyring и сброс
    /// состояния идут под `commit`, поэтому поздний `login` их не перезапишет.
    pub fn logout_with_disconnect<F>(&self, disconnect: F) -> AuthState
    where
        F: FnOnce() -> Result<(), String>,
    {
        let disconnect_error = disconnect().err();
        let _commit = match self.lock_commit() {
            Ok(commit) => commit,
            Err(error) => return AuthState::error(error),
        };
        let cleared = self.clear_store();
        let mut inner = match self.lock() {
            Ok(inner) => inner,
            Err(error) => return AuthState::error(error),
        };
        inner.keyring_checked = true;
        inner.generation += 1;
        inner.state = AuthState::idle(false);
        let error = match disconnect_error {
            Some(message) => Some(AuthError::Keyring { message }),
            None => cleared.err(),
        };
        if let Some(error) = error {
            inner.state = AuthState::error(error);
        }
        inner.state.clone()
    }

    // -----------------------------------------------------------------------
    // ADR-3: интерактивный вход (пароль → MFA/captcha → token → keyring)
    // -----------------------------------------------------------------------

    /// `auth_login_password`: `POST /auth/login` (A-02).
    ///
    /// Пароль принимается по значению и затирается сразу после сериализации тела;
    /// тело — после ответа. Ответ: [`LoginOutcome::Token`] (уже проверен и в keyring),
    /// [`LoginOutcome::Mfa`], [`LoginOutcome::Captcha`] или [`LoginOutcome::Error`].
    /// Ровно один POST: автоматических повторов нет (ADR-3).
    pub fn login_password(
        &self,
        login: &str,
        mut password: String,
        captcha_key: Option<&str>,
        captcha_rqtoken: Option<&str>,
        captcha_session_id: Option<&str>,
        captcha_rqdata: Option<&str>,
    ) -> LoginOutcome {
        let login = login.trim();
        if login.is_empty() || password.is_empty() {
            wipe_string(&mut password);
            return LoginOutcome::Error(AuthError::EmptyCredentials);
        }

        let mut body = login_body(login, &password);
        wipe_string(&mut password);

        // A-03: явный пустой authorization — не наследуем текущий токен.
        let mut headers: Vec<(&str, &str)> = vec![("authorization", "")];
        if let Some(key) = captcha_key.map(str::trim).filter(|key| !key.is_empty()) {
            headers.push((CAPTCHA_KEY_HEADER, key));
        }
        if let Some(rqtoken) = captcha_rqtoken
            .map(str::trim)
            .filter(|rqtoken| !rqtoken.is_empty())
        {
            headers.push((CAPTCHA_RQTOKEN_HEADER, rqtoken));
        }
        if let Some(session_id) = captcha_session_id
            .map(str::trim)
            .filter(|session_id| !session_id.is_empty())
        {
            headers.push((CAPTCHA_SESSION_ID_HEADER, session_id));
        }
        if let Some(rqdata) = captcha_rqdata
            .map(str::trim)
            .filter(|rqdata| !rqdata.is_empty())
        {
            headers.push((CAPTCHA_RQDATA_HEADER, rqdata));
        }

        let result = self.login.post_json(LOGIN_PATH, &body, &headers);
        wipe_bytes(&mut body);
        match result {
            Err(error) => LoginOutcome::Error(map_login_api_error(&error)),
            Ok(response) => self.finish_login(response),
        }
    }

    /// `auth_mfa`: `POST /auth/mfa/{method}` (A-05).
    ///
    /// Код принимается по значению и затирается после сериализации; метод — только из
    /// whitelist [`MFA_METHODS`] (он подставляется в путь). Успех — `Token`, иначе
    /// `Error`/`Mfa`.
    pub fn submit_mfa(
        &self,
        method: &str,
        mut code: String,
        ticket: &str,
        login_instance_id: Option<&str>,
    ) -> LoginOutcome {
        let method = method.trim();
        let ticket = ticket.trim();
        if !MFA_METHODS.contains(&method) {
            wipe_string(&mut code);
            return LoginOutcome::Error(AuthError::Internal {
                message: format!("неизвестный метод MFA: {}", truncate(method)),
            });
        }
        if code.is_empty() || ticket.is_empty() {
            wipe_string(&mut code);
            return LoginOutcome::Error(AuthError::EmptyCredentials);
        }

        let mut body = mfa_body(&code, ticket, login_instance_id);
        wipe_string(&mut code);
        let path = format!("{MFA_PATH_PREFIX}{method}");
        let result = self.login.post_json(&path, &body, &[("authorization", "")]);
        wipe_bytes(&mut body);
        match result {
            Err(error) => LoginOutcome::Error(map_login_api_error(&error)),
            Ok(response) => self.finish_login(response),
        }
    }

    /// `auth_sms`: `POST /auth/mfa/sms/send` `{ticket}` (A-07).
    ///
    /// # Errors
    ///
    /// [`AuthError::EmptyCredentials`] для пустого ticket, [`AuthError::RateLimited`]
    /// для 429, [`AuthError::Network`] для транспорта, [`AuthError::LoginRejected`] для
    /// прочих отказов; тела/тикета в тексте ошибки нет.
    pub fn request_sms(&self, ticket: &str) -> Result<SmsChallenge, AuthError> {
        let ticket = ticket.trim();
        if ticket.is_empty() {
            return Err(AuthError::EmptyCredentials);
        }
        let mut body = serde_json::json!({ "ticket": ticket })
            .to_string()
            .into_bytes();
        let result = self
            .login
            .post_json(MFA_SMS_SEND_PATH, &body, &[("authorization", "")]);
        wipe_bytes(&mut body);
        let response = result.map_err(|error| map_login_api_error(&error))?;

        if (200..300).contains(&response.status) {
            let phone = parse_json(&response.body)
                .as_ref()
                .and_then(|value| json_string(value, "phone"));
            return Ok(SmsChallenge { phone });
        }
        match parse_login_response(response.status, &response.headers, &response.body) {
            LoginOutcome::Error(error) => Err(error),
            _ => Err(AuthError::LoginRejected {
                status: response.status,
                code: None,
            }),
        }
    }

    /// Разобрать ответ `/auth/*`; `Token` дополнительно проверяется и сохраняется.
    fn finish_login(&self, response: HttpResponse) -> LoginOutcome {
        match parse_login_response(response.status, &response.headers, &response.body) {
            LoginOutcome::Token(token) => self.accept_token(token),
            outcome => outcome,
        }
    }

    /// Общая часть успешного входа/MFA: `/users/@me` → keyring → `Ready`.
    fn accept_token(&self, token: String) -> LoginOutcome {
        let generation = match self.begin_validation(true) {
            Ok(generation) => generation,
            Err(error) => return LoginOutcome::Error(error),
        };
        let request = ValidationRequest {
            token: token.clone(),
            super_properties: self.identity.super_properties.clone(),
            locale: self.identity.locale.clone(),
            timezone: self.identity.timezone.clone(),
        };
        match self.finish_validation(&token, self.validator.validate(&request), true, generation) {
            ValidationOutcome::Committed(_) => LoginOutcome::Token(token),
            ValidationOutcome::Superseded(_) => LoginOutcome::Error(AuthError::Internal {
                message: "вход отменён более новой операцией авторизации".to_string(),
            }),
            ValidationOutcome::Rejected(error, _) => LoginOutcome::Error(error),
        }
    }

    /// Общая часть login/restore: `Validating` → `/users/@me` → (`store`) → `Ready`.
    ///
    /// Гвард состояния уже отпущен: параметр `generation` — поколение, к которому
    /// относится проверка. Коммит применяется только к нему (PLAT-01).
    fn authenticate(&self, token: &str, persist: bool, generation: u64) -> AuthState {
        let request = ValidationRequest {
            token: token.to_string(),
            super_properties: self.identity.super_properties.clone(),
            locale: self.identity.locale.clone(),
            timezone: self.identity.timezone.clone(),
        };
        match self.finish_validation(
            token,
            self.validator.validate(&request),
            persist,
            generation,
        ) {
            ValidationOutcome::Committed(state)
            | ValidationOutcome::Superseded(state)
            | ValidationOutcome::Rejected(_, state) => state,
        }
    }

    /// Начать проверку: `Validating` + новое поколение. Гвард не держится.
    fn begin_validation(&self, mark_keyring_checked: bool) -> Result<u64, AuthError> {
        let _commit = self.lock_commit()?;
        let mut inner = self.lock()?;
        if mark_keyring_checked {
            inner.keyring_checked = true;
        }
        inner.generation += 1;
        let stored = inner.state.stored;
        let mut validating = AuthState::validating();
        validating.stored = stored;
        inner.state = validating;
        Ok(inner.generation)
    }

    /// Применить результат `/users/@me` к состоянию.
    ///
    /// Сетевой вызов уже выполнен; здесь только короткий `commit` + keyring-запись.
    /// Если поколение изменилось (logout или более новый login), результат
    /// отбрасывается — чужая сессия не перезаписывается.
    fn finish_validation(
        &self,
        token: &str,
        validated: Result<Me, AuthError>,
        persist: bool,
        generation: u64,
    ) -> ValidationOutcome {
        let me = match validated {
            Ok(me) => me,
            Err(error) => return self.reject_validation(error, generation),
        };
        let _commit = match self.lock_commit() {
            Ok(commit) => commit,
            Err(error) => return self.reject_validation(error, generation),
        };
        match self.lock() {
            Ok(inner) => {
                if inner.generation != generation {
                    return ValidationOutcome::Superseded(inner.state.clone());
                }
            }
            Err(error) => return self.reject_locked(error, generation),
        };
        if persist {
            if let Err(error) = self.write_store(token) {
                return self.reject_locked(error, generation);
            }
        }
        let mut inner = match self.lock() {
            Ok(inner) => inner,
            Err(error) => return self.reject_locked(error, generation),
        };
        // `begin_validation`/`logout` берут тот же `commit`, поэтому поколение
        // здесь не могло измениться с момента проверки выше.
        inner.state = AuthState::ready(AuthUser::from(&me), true);
        ValidationOutcome::Committed(inner.state.clone())
    }

    /// Ошибка проверки: применяется только к своему поколению.
    fn reject_validation(&self, error: AuthError, generation: u64) -> ValidationOutcome {
        let _commit = match self.lock_commit() {
            Ok(commit) => commit,
            Err(lock_error) => {
                let state = AuthState::error(lock_error.clone());
                return ValidationOutcome::Rejected(lock_error, state);
            }
        };
        self.reject_locked(error, generation)
    }

    /// `reject_validation` для вызывающего, уже удерживающего `commit`
    /// (повторный `lock_commit` на std-мьютексе был бы дедлоком).
    fn reject_locked(&self, error: AuthError, generation: u64) -> ValidationOutcome {
        let mut inner = match self.lock() {
            Ok(inner) => inner,
            Err(lock_error) => {
                let state = AuthState::error(lock_error.clone());
                return ValidationOutcome::Rejected(lock_error, state);
            }
        };
        if inner.generation != generation {
            return ValidationOutcome::Superseded(inner.state.clone());
        }
        let stored = inner.state.stored;
        inner.state = AuthState::error(error.clone());
        inner.state.stored = stored;
        ValidationOutcome::Rejected(error, inner.state.clone())
    }
}

/// Итог применения сетевой проверки к состоянию сессии.
enum ValidationOutcome {
    /// Состояние этой операции стало текущим.
    Committed(AuthState),
    /// Состояние уже обновлено более новой операцией — оно не перезаписано.
    Superseded(AuthState),
    /// Проверка/keyring отказали; состояние — ошибка этой операции.
    Rejected(AuthError, AuthState),
}

impl fmt::Debug for AuthBackend {
    /// Печатается только состояние и identity: токена в backend нет (он в keyring).
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthBackend")
            .field("identity", &self.identity)
            .finish_non_exhaustive()
    }
}
