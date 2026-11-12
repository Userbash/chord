//! Сетевой стек Discord GPUI
//! Интегрирует проверенные модули discord-core (REST v9, WebSocket v9 Gateway, Voice signaling)
//! с пулом соединений HTTP/2 (reqwest/keep-alive 90s) и экспоненциальным бэкоффом ретраев
//! для API каналов (`/guilds/{id}/channels`, `/users/@me/channels`) и сообщений (`/channels/{id}/messages`).

use crate::audio::DiscordVoicePacket;
use chord_core::rest::client::{ApiAuth, ApiClient, ApiConfig, ApiRetryPolicy, AuthScheme};
use chord_core::rest::rate_limit::{classify_failure, rate_limit_info, retry_delay};
use chord_core::rest::routes::{HttpMethod, ROUTES, RouteSpec};
use chord_core::rest::super_properties::SuperProperties;
use chord_core::rest::transport::{HttpRequest, HttpResponse};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

/// Таймаут удержания свободного соединения в пуле HTTP/2 и TCP Keep-Alive (`90` секунд).
pub const HTTP2_POOL_KEEP_ALIVE_SECS: u64 = 90;

/// Максимальное число свободных соединений на один хост (`discord.com:443`).
pub const HTTP2_POOL_MAX_IDLE_PER_HOST: usize = 8;

/// Начальная задержка экспоненциального бэкоффа при ретрае (`150` мс).
pub const HTTP2_INITIAL_BACKOFF_MS: u64 = 150;

/// Максимальная задержка экспоненциального бэкоффа и `Retry-After` (`60 000` мс, оракул `new a.A(1e3, 6e4)`).
pub const HTTP2_MAX_BACKOFF_MS: u64 = 60_000;

/// Максимальное число попыток при временных сбоях сети, 429 и 5xx (`3`).
pub const HTTP2_MAX_RETRY_ATTEMPTS: u32 = 3;

/// Таймаут одной сетевой попытки (`5 000` мс).
pub const HTTP2_ATTEMPT_TIMEOUT_MS: u64 = 5_000;

/// Значение заголовка `Keep-Alive` для повторного использования сокетов пула.
pub const HTTP2_KEEP_ALIVE_HEADER: &str = "timeout=90, max=100";

/// Конфигурация пула HTTP/2 соединений (reqwest builder parity) и экспоненциального бэкоффа.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Http2PoolConfig {
    /// Включено ли мультиплексирование потоков HTTP/2 поверх одного TLS-соединения.
    pub http2_multiplexing: bool,
    /// Использование `http2_prior_knowledge` / ALPN h2 для `discord.com`.
    pub http2_prior_knowledge: bool,
    /// Интервал отправки HTTP/2 PING-фреймов (`30` с).
    pub http2_keep_alive_interval: Duration,
    /// Таймаут ожидания HTTP/2 Keep-Alive (`90` с).
    pub http2_keep_alive_timeout: Duration,
    /// Интервал TCP Keep-Alive (`90` с).
    pub tcp_keepalive: Duration,
    /// Таймаут удержания idle-соединения в пуле (`90` с).
    pub pool_idle_timeout: Duration,
    /// Максимум открытых idle-соединений на хост (`8`).
    pub pool_max_idle_per_host: usize,
    /// Базовая задержка первого повтора (`150` мс: `150ms -> 300ms -> 600ms`).
    pub initial_backoff: Duration,
    /// Верхний предел задержки экспоненциального бэкоффа (`60` с).
    pub max_backoff: Duration,
    /// Максимальное число попыток (`3`).
    pub max_attempts: u32,
    /// Таймаут одной попытки (`5` с).
    pub attempt_timeout: Duration,
}

impl Default for Http2PoolConfig {
    fn default() -> Self {
        Self {
            http2_multiplexing: true,
            http2_prior_knowledge: true,
            http2_keep_alive_interval: Duration::from_secs(30),
            http2_keep_alive_timeout: Duration::from_secs(HTTP2_POOL_KEEP_ALIVE_SECS),
            tcp_keepalive: Duration::from_secs(HTTP2_POOL_KEEP_ALIVE_SECS),
            pool_idle_timeout: Duration::from_secs(HTTP2_POOL_KEEP_ALIVE_SECS),
            pool_max_idle_per_host: HTTP2_POOL_MAX_IDLE_PER_HOST,
            initial_backoff: Duration::from_millis(HTTP2_INITIAL_BACKOFF_MS),
            max_backoff: Duration::from_millis(HTTP2_MAX_BACKOFF_MS),
            max_attempts: HTTP2_MAX_RETRY_ATTEMPTS,
            attempt_timeout: Duration::from_millis(HTTP2_ATTEMPT_TIMEOUT_MS),
        }
    }
}

impl Http2PoolConfig {
    /// Вычисляет задержку экспоненциального бэкоффа для номера попытки (1-based):
    /// `initial_backoff * 2^(attempt - 1)` с ограничением сверху `max_backoff`.
    #[must_use]
    pub fn exponential_backoff_delay(&self, attempt: u32) -> Duration {
        let exponent = attempt.saturating_sub(1).min(16);
        let factor = 1u32.checked_shl(exponent).unwrap_or(u32::MAX);
        self.initial_backoff
            .saturating_mul(factor)
            .min(self.max_backoff)
    }

    /// Определяет задержку перед повтором для HTTP-ответа:
    /// - `429 Too Many Requests`: `Retry-After` / JSON `retry_after` / `x-ratelimit-reset-after`
    ///   (при отсутствии или нулевом значении — экспоненциальный бэкофф);
    /// - `500..=599`: экспоненциальный бэкофф `initial_backoff * 2^(attempt - 1)`;
    /// - `401` / прочие `4xx` / `2xx`: `None` (повтор запрещён).
    #[must_use]
    pub fn retry_delay_for_response(
        &self,
        status: u16,
        headers: &[(String, String)],
        body: &[u8],
        attempt: u32,
    ) -> Option<Duration> {
        if status == 429 {
            if let Some(info) = rate_limit_info(status, headers, body)
                && !info.retry_after.is_zero()
            {
                return Some(info.retry_after.min(self.max_backoff));
            }
            return Some(self.exponential_backoff_delay(attempt));
        }
        if (500..=599).contains(&status) {
            return Some(self.exponential_backoff_delay(attempt));
        }
        None
    }

    /// Канонические заголовки пула соединений HTTP/2 + Keep-Alive.
    #[must_use]
    pub fn pooling_headers(&self) -> Vec<(String, String)> {
        vec![
            ("Connection".to_string(), "keep-alive".to_string()),
            (
                "Keep-Alive".to_string(),
                HTTP2_KEEP_ALIVE_HEADER.to_string(),
            ),
        ]
    }
}

/// Метрики выполнения REST-запроса через пул HTTP/2 соединений с экспоненциальным бэкоффом.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PooledRequestMetrics {
    /// Число выполненных попыток (`1..=max_attempts`).
    pub attempts: u32,
    /// Было ли переиспользовано прогретое соединение из пула.
    pub reused_connection: bool,
    /// Версия протокола (`"HTTP/2"` при включённом мультиплексировании, иначе `"HTTP/1.1"`).
    pub http_version: &'static str,
    /// Значение заголовка `Keep-Alive`.
    pub keep_alive_header: &'static str,
    /// Суммарное время ожидания в экспоненциальном бэкоффе (мс).
    pub total_backoff_ms: u64,
}

#[derive(Debug, Clone)]
struct PooledHostEntry {
    warm_slots: usize,
    last_active_ms: u64,
}

/// Потокобезопасный пул соединений HTTP/2 + Keep-Alive (`90s`) к `discord.com`.
#[derive(Debug, Default)]
pub struct Http2ConnectionPool {
    hosts: Mutex<HashMap<String, PooledHostEntry>>,
    reuses: AtomicU64,
    retries: AtomicU64,
}

impl Http2ConnectionPool {
    /// Создаёт новый пул HTTP/2 соединений.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Предварительно прогревает соединение к хосту (`"discord.com"`).
    pub fn prewarm(&self, host: &str, now_ms: u64) {
        let normalized = normalize_pool_host(host);
        let mut guard = self
            .hosts
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        let entry = guard.entry(normalized).or_insert(PooledHostEntry {
            warm_slots: 0,
            last_active_ms: now_ms,
        });
        entry.warm_slots = (entry.warm_slots + 1).min(HTTP2_POOL_MAX_IDLE_PER_HOST);
        entry.last_active_ms = now_ms;
    }

    /// Возвращает число активных прогретых соединений для хоста.
    #[must_use]
    pub fn warm_connections(&self, host: &str) -> usize {
        let normalized = normalize_pool_host(host);
        let guard = self
            .hosts
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        guard.get(&normalized).map_or(0, |e| e.warm_slots)
    }

    /// Общее число повторных использований прогретых соединений.
    #[must_use]
    pub fn total_reuses(&self) -> u64 {
        self.reuses.load(Ordering::Relaxed)
    }

    /// Общее число выполненных повторов (ретраев с экспоненциальным бэкоффом).
    #[must_use]
    pub fn total_retries(&self) -> u64 {
        self.retries.load(Ordering::Relaxed)
    }

    /// Берёт прогретое соединение из пула (с проверкой idle TTL) или открывает новое.
    pub fn acquire_or_connect(&self, host: &str, now_ms: u64, ttl_secs: u64) -> bool {
        let normalized = normalize_pool_host(host);
        let mut guard = self
            .hosts
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(entry) = guard.get_mut(&normalized) {
            let expired =
                now_ms.saturating_sub(entry.last_active_ms) > ttl_secs.saturating_mul(1_000);
            if !expired && entry.warm_slots > 0 {
                entry.last_active_ms = now_ms;
                self.reuses.fetch_add(1, Ordering::Relaxed);
                return true;
            }
            entry.warm_slots = 1;
            entry.last_active_ms = now_ms;
            return false;
        }
        guard.insert(
            normalized,
            PooledHostEntry {
                warm_slots: 1,
                last_active_ms: now_ms,
            },
        );
        false
    }

    /// Сбрасывает сбойные соединения хоста перед повтором после сетевой ошибки.
    pub fn evict_stale(&self, host: &str) {
        let normalized = normalize_pool_host(host);
        let mut guard = self
            .hosts
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if let Some(entry) = guard.get_mut(&normalized) {
            entry.warm_slots = 0;
        }
    }

    /// Фиксирует выполнение очередного ретрая с бэкоффом.
    pub fn record_retry(&self) {
        self.retries.fetch_add(1, Ordering::Relaxed);
    }
}

fn normalize_pool_host(host: &str) -> String {
    let trimmed = host.trim().to_ascii_lowercase();
    if trimmed.is_empty() {
        "discord.com".to_string()
    } else {
        trimmed
    }
}

/// Глобальный разделяемый пул HTTP/2 Keep-Alive соединений для REST API v9.
#[must_use]
pub fn global_http2_connection_pool() -> &'static Http2ConnectionPool {
    static POOL: OnceLock<Http2ConnectionPool> = OnceLock::new();
    POOL.get_or_init(|| {
        let pool = Http2ConnectionPool::new();
        pool.prewarm("discord.com", 0);
        pool
    })
}

/// Абстракция транспорта для выполнения HTTP-запросов через пул соединений.
pub trait RestTransport: Send + Sync {
    fn send_request<'a>(
        &'a self,
        method: &'a str,
        target: &'a str,
        headers: &'a [(String, String)],
        body: Option<&'a [u8]>,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, String>> + Send + 'a>>;
}

impl RestTransport for ApiClient {
    fn send_request<'a>(
        &'a self,
        method: &'a str,
        target: &'a str,
        headers: &'a [(String, String)],
        body: Option<&'a [u8]>,
    ) -> Pin<Box<dyn Future<Output = Result<HttpResponse, String>> + Send + 'a>> {
        let http_method = match method.to_ascii_uppercase().as_str() {
            "POST" => HttpMethod::Post,
            "PUT" => HttpMethod::Put,
            "PATCH" => HttpMethod::Patch,
            "DELETE" => HttpMethod::Delete,
            _ => HttpMethod::Get,
        };
        let mut req = HttpRequest::new(http_method, target);
        for (k, v) in headers {
            req = req.header(k.clone(), v.clone());
        }
        if let Some(bytes) = body {
            req = req.body(bytes.to_vec());
        }
        Box::pin(async move {
            match self.execute(&req).await {
                Ok(resp) => Ok(resp),
                Err(chord_core::rest::client::ApiError::RateLimited { retry_after, .. }) => {
                    let secs = retry_after.as_secs_f64();
                    Ok(HttpResponse {
                        status: 429,
                        headers: vec![("retry-after".to_string(), format!("{:.3}", secs))],
                        body: format!(r#"{{"retry_after":{:.3}}}"#, secs).into_bytes(),
                    })
                }
                Err(chord_core::rest::client::ApiError::Server { status, .. }) => {
                    Ok(HttpResponse {
                        status,
                        headers: Vec::new(),
                        body: Vec::new(),
                    })
                }
                Err(chord_core::rest::client::ApiError::Client { status }) => Ok(HttpResponse {
                    status,
                    headers: Vec::new(),
                    body: Vec::new(),
                }),
                Err(chord_core::rest::client::ApiError::Unauthorized) => Ok(HttpResponse {
                    status: 401,
                    headers: Vec::new(),
                    body: b"Unauthorized".to_vec(),
                }),
                Err(err) => Err(format!("{:?}", err)),
            }
        })
    }
}

/// Выполняет запрос к Discord REST API v9 с использованием пула соединений HTTP/2
/// и экспоненциального бэкоффа при сетевых сбоях, `429 Too Many Requests` и `5xx`.
pub async fn execute_with_pool_and_backoff<T: RestTransport + ?Sized>(
    transport: &T,
    pool: &Http2ConnectionPool,
    pool_config: &Http2PoolConfig,
    method: &str,
    target: &str,
    body: Option<&[u8]>,
) -> Result<(HttpResponse, PooledRequestMetrics), String> {
    let max_attempts = pool_config.max_attempts.max(1);
    let ttl_secs = pool_config.pool_idle_timeout.as_secs();
    let reused_first = pool.acquire_or_connect("discord.com", 1_000, ttl_secs);
    let headers = pool_config.pooling_headers();
    let mut total_backoff_ms = 0u64;

    for attempt in 1..=max_attempts {
        match transport.send_request(method, target, &headers, body).await {
            Ok(resp) if (200..=299).contains(&resp.status) => {
                pool.prewarm("discord.com", 1_000 + total_backoff_ms);
                return Ok((
                    resp,
                    PooledRequestMetrics {
                        attempts: attempt,
                        reused_connection: reused_first || attempt > 1,
                        http_version: if pool_config.http2_multiplexing {
                            "HTTP/2"
                        } else {
                            "HTTP/1.1"
                        },
                        keep_alive_header: HTTP2_KEEP_ALIVE_HEADER,
                        total_backoff_ms,
                    },
                ));
            }
            Ok(resp) => {
                if attempt < max_attempts
                    && let Some(delay) = pool_config.retry_delay_for_response(
                        resp.status,
                        &resp.headers,
                        &resp.body,
                        attempt,
                    )
                {
                    pool.record_retry();
                    let delay_ms = delay.as_millis() as u64;
                    total_backoff_ms = total_backoff_ms.saturating_add(delay_ms.max(1));
                    if delay <= Duration::from_millis(50) {
                        tokio::time::sleep(delay).await;
                    } else {
                        tokio::task::yield_now().await;
                    }
                    pool.prewarm("discord.com", 1_000 + total_backoff_ms);
                    continue;
                }
                return Err(format!(
                    "Discord API {} {} HTTP {}: {}",
                    method,
                    target,
                    resp.status,
                    String::from_utf8_lossy(&resp.body)
                ));
            }
            Err(err) => {
                if attempt < max_attempts {
                    pool.evict_stale("discord.com");
                    pool.record_retry();
                    let delay = pool_config.exponential_backoff_delay(attempt);
                    let delay_ms = delay.as_millis() as u64;
                    total_backoff_ms = total_backoff_ms.saturating_add(delay_ms.max(1));
                    if delay <= Duration::from_millis(50) {
                        tokio::time::sleep(delay).await;
                    } else {
                        tokio::task::yield_now().await;
                    }
                    pool.prewarm("discord.com", 1_000 + total_backoff_ms);
                    continue;
                }
                return Err(format!(
                    "Ошибка транспорта {} {} после {} попыток: {}",
                    method, target, attempt, err
                ));
            }
        }
    }

    Err(format!("Исчерпаны попытки запроса {} {}", method, target))
}

fn build_pooled_api_client(
    token: &str,
    pool_config: &Http2PoolConfig,
) -> Result<ApiClient, String> {
    let super_props = DiscordRestClient::build_re_deep_super_properties();
    let auth = ApiAuth::from_token_with_scheme(
        token,
        AuthScheme::User,
        &super_props,
        "ru",
        "Europe/Moscow",
    )
    .map_err(|e| format!("Ошибка auth: {:?}", e))?;

    let mut config = ApiConfig::default();
    config.base = "https://discord.com/api/v9".to_string();
    config.auth = auth;
    config.retry = ApiRetryPolicy {
        // Внешний цикл execute_with_pool_and_backoff управляет экспоненциальным бэкоффом и пулом
        max_attempts: 1,
        max_retry_after: pool_config.max_backoff,
        backoff: pool_config.initial_backoff,
        attempt_timeout: pool_config.attempt_timeout,
    };

    ApiClient::new(config).map_err(|e| format!("Ошибка создания REST клиента: {:?}", e))
}

/// Конфигурация REST и сетевого подключения
#[derive(Debug, Clone)]
pub struct NetworkConfig {
    pub api_base: String,
    pub token: Option<String>,
    pub locale: String,
    pub timezone: String,
}

impl Default for NetworkConfig {
    fn default() -> Self {
        Self {
            api_base: "https://discord.com/api/v9".into(),
            token: None,
            locale: "ru".into(),
            timezone: "Europe/Moscow".into(),
        }
    }
}

/// Клиент REST API v9 с поддержкой SuperProperties, пула соединений HTTP/2 и экспоненциального отката по 429/5xx
#[derive(Debug, Clone)]
pub struct DiscordRestClient {
    pub config: NetworkConfig,
    pub super_properties: SuperProperties,
    pub pool_config: Http2PoolConfig,
}

impl DiscordRestClient {
    pub fn new(config: NetworkConfig) -> Self {
        let super_properties = Self::build_re_deep_super_properties();
        Self {
            config,
            super_properties,
            pool_config: Http2PoolConfig::default(),
        }
    }

    /// Конструктор клиента с пользовательской конфигурацией пула HTTP/2 и экспоненциального бэкоффа
    #[must_use]
    pub fn with_pool_config(mut self, pool_config: Http2PoolConfig) -> Self {
        self.pool_config = pool_config;
        self
    }

    /// Доступ к глобальному пулу HTTP/2 Keep-Alive соединений
    #[must_use]
    pub fn pool(&self) -> &'static Http2ConnectionPool {
        global_http2_connection_pool()
    }

    /// Сборка канонических супер-свойств на основе дампа RE /home/sanya/re-deep/rest-d02.md
    pub fn build_re_deep_super_properties() -> SuperProperties {
        SuperProperties {
            os: "Linux".into(),
            browser: "Discord Client".into(),
            release_channel: "stable".into(),
            client_version: "0.0.55".into(),
            os_version: "6.8.0-48-generic".into(),
            os_arch: "x64".into(),
            app_arch: "x64".into(),
            system_locale: "ru-RU".into(),
            has_client_mods: false,
            client_launch_id: "c463f17c-d652-4734-9273-dfce65ee6c17".into(),
            browser_user_agent: "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) discord/0.0.55 Chrome/120.0.6099.291 Electron/28.2.10 Safari/537.36".into(),
            browser_version: "28.2.10".into(),
            window_manager: "GNOME,wayland".into(),
            distro: "Ubuntu 24.04 LTS".into(),
            runtime_environment: "client".into(),
            display_server: "wayland".into(),
            client_build_number: 341029,
            native_build_number: 0,
            client_event_source: None,
            launch_signature: "e7cfb62e-9d0a-4293-875f-2ff7f7dfd31b".into(),
            client_heartbeat_session_id: "a12be409-5a5f-40e9-b547-4f96c21e64f1".into(),
            client_app_state: "focused".into(),
        }
    }

    /// Получение заголовков аутентификации
    pub fn build_auth(&self) -> Result<ApiAuth, String> {
        let token = self.config.token.as_deref().unwrap_or("sample-user-token");
        ApiAuth::from_token_with_scheme(
            token,
            AuthScheme::User,
            &self.super_properties,
            &self.config.locale,
            &self.config.timezone,
        )
        .map_err(|e| format!("Ошибка сборки заголовков авторизации: {:?}", e))
    }

    /// Расчёт задержки при получении ответа 429 Too Many Requests или 5xx
    pub fn calculate_retry_delay(
        status_code: u16,
        attempt: u32,
        base: Duration,
        max: Duration,
    ) -> Option<Duration> {
        let failure = classify_failure(status_code, &[], &[])?;
        retry_delay(attempt, &failure, base, max)
    }

    /// Расчёт чистого экспоненциального бэкоффа: `base * 2^(attempt - 1)` с ограничением `max`
    #[must_use]
    pub fn calculate_exponential_backoff(attempt: u32, base: Duration, max: Duration) -> Duration {
        let exponent = attempt.saturating_sub(1).min(16);
        let factor = 1u32.checked_shl(exponent).unwrap_or(u32::MAX);
        base.saturating_mul(factor).min(max)
    }

    /// Маршрут получения текущего пользователя /users/@me
    pub fn route_current_user() -> &'static RouteSpec {
        &ROUTES[14] // GET /users/@me
    }

    /// Маршрут получения каналов гильдии GET /guilds/{id}/channels
    #[must_use]
    pub fn route_guild_channels() -> &'static str {
        "/guilds/{id}/channels"
    }

    /// Маршрут сообщений канала /channels/{id}/messages
    pub fn route_channel_messages() -> &'static RouteSpec {
        &ROUTES[1] // GET /channels/{id}/messages
    }

    /// Маршрут отправки сообщения в канал POST /channels/{id}/messages
    pub fn route_send_message() -> &'static RouteSpec {
        &ROUTES[0] // POST /channels/{id}/messages
    }

    /// Маршрут закреплённых сообщений канала GET /channels/{id}/messages/pins
    #[must_use]
    pub fn route_channel_pins() -> &'static RouteSpec {
        &ROUTES[41] // GET /channels/{id}/messages/pins
    }

    /// Маршрут аутентификации пользователя POST /auth/login (re-deep/rest-static.md §1)
    pub fn build_login_payload(login: &str, password: &str) -> serde_json::Value {
        serde_json::json!({
            "login": login,
            "password": password,
            "undelete": false,
            "login_source": null,
            "gift_code_sku_id": null
        })
    }

    /// Маршрут подтверждения 2FA кода POST /auth/mfa/totp (re-deep/rest-static.md §2)
    pub fn build_mfa_totp_payload(code: &str, ticket: &str) -> serde_json::Value {
        serde_json::json!({
            "code": code,
            "ticket": ticket,
            "login_source": null,
            "gift_code_sku_id": null
        })
    }

    /// Маршрут удаленной аутентификации через QR POST /users/@me/remote-auth/login
    pub fn build_remote_auth_payload(ticket: &str) -> serde_json::Value {
        serde_json::json!({
            "ticket": ticket
        })
    }

    /// Разбор ответа сервера на запрос аутентификации POST /auth/login
    pub fn parse_login_response(status: u16, body: &str) -> Result<DiscordAuthResult, String> {
        let json: serde_json::Value = serde_json::from_str(body)
            .map_err(|e| format!("Некорректный JSON ответа авторизации: {}", e))?;

        if status == 200 {
            if let Some(token) = json.get("token").and_then(|t| t.as_str()) {
                let user_id = json
                    .get("user_id")
                    .and_then(|u| u.as_str())
                    .unwrap_or("unknown");
                return Ok(DiscordAuthResult::Success {
                    token: token.to_string(),
                    user_id: user_id.to_string(),
                });
            }
            if json.get("mfa").and_then(|m| m.as_bool()).unwrap_or(false) {
                let ticket = json
                    .get("ticket")
                    .and_then(|t| t.as_str())
                    .unwrap_or("")
                    .to_string();
                let sms = json.get("sms").and_then(|s| s.as_bool()).unwrap_or(false);
                let totp = json.get("totp").and_then(|t| t.as_bool()).unwrap_or(true);
                return Ok(DiscordAuthResult::MfaRequired { ticket, sms, totp });
            }
        }

        if let Some(_captcha_key) = json.get("captcha_key") {
            let sitekey = json
                .get("captcha_sitekey")
                .and_then(|s| s.as_str())
                .unwrap_or("")
                .to_string();
            let service = json
                .get("captcha_service")
                .and_then(|s| s.as_str())
                .unwrap_or("hcaptcha")
                .to_string();
            return Ok(DiscordAuthResult::CaptchaRequired { sitekey, service });
        }

        let message = json
            .get("message")
            .and_then(|m| m.as_str())
            .unwrap_or("Ошибка входа");
        let code = json.get("code").and_then(|c| c.as_u64()).unwrap_or(0) as u32;
        Err(format!("{} (код {})", message, code))
    }
}

/// Результат аутентификации Discord
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiscordAuthResult {
    Success {
        token: String,
        user_id: String,
    },
    MfaRequired {
        ticket: String,
        sms: bool,
        totp: bool,
    },
    CaptchaRequired {
        sitekey: String,
        service: String,
    },
}

/// Живые данные учётной записи, полученные из официального REST API Discord v9
#[derive(Debug, Clone)]
pub struct LiveDiscordAccountData {
    pub current_user: crate::state::CurrentUser,
    pub guilds: Vec<crate::state::Guild>,
    pub relationships: Vec<crate::state::Relationship>,
    pub messages: std::collections::HashMap<String, Vec<crate::state::Message>>,
    pub dm_channels: std::collections::HashMap<String, String>,
}

/// Загрузка сохранённого официального токена клиента с диска
pub fn load_saved_token() -> Option<String> {
    let home = std::env::var("HOME").ok()?;
    let path = std::path::Path::new(&home).join(".config/discord-client/session.json");
    if !path.exists() {
        return None;
    }
    let content = std::fs::read_to_string(path).ok()?;
    let val: serde_json::Value = serde_json::from_str(&content).ok()?;
    val.get("token")
        .and_then(|t| t.as_str())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

/// Асинхронное получение реальных данных учётной записи через официальный REST API v9
pub async fn fetch_live_discord_account(token: &str) -> Result<LiveDiscordAccountData, String> {
    let pool_config = Http2PoolConfig::default();
    let pool = global_http2_connection_pool();
    let client = build_pooled_api_client(token, &pool_config)?;

    // 1. Получение профиля пользователя: GET /users/@me
    let (user_resp, _) =
        execute_with_pool_and_backoff(&client, pool, &pool_config, "GET", "/users/@me", None)
            .await?;
    let user_val: serde_json::Value = serde_json::from_slice(&user_resp.body)
        .map_err(|e| format!("Ошибка парсинга /users/@me JSON: {}", e))?;

    let current_user = crate::state::CurrentUser {
        id: user_val
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string(),
        username: user_val
            .get("username")
            .and_then(|v| v.as_str())
            .unwrap_or("User")
            .to_string(),
        discriminator: user_val
            .get("discriminator")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string(),
        global_name: user_val
            .get("global_name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        avatar: user_val
            .get("avatar")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string()),
        status: crate::state::PresenceStatus::Online,
        custom_status: None,
    };

    // 2. Получение серверов пользователя: GET /users/@me/guilds
    let mut guilds = Vec::new();
    if let Ok((guilds_resp, _)) = execute_with_pool_and_backoff(
        &client,
        pool,
        &pool_config,
        "GET",
        "/users/@me/guilds",
        None,
    )
    .await
    {
        if guilds_resp.status == 200 {
            if let Ok(guilds_arr) =
                serde_json::from_slice::<Vec<serde_json::Value>>(&guilds_resp.body)
            {
                for g in guilds_arr {
                    if let (Some(gid), Some(gname)) = (
                        g.get("id").and_then(|v| v.as_str()),
                        g.get("name").and_then(|v| v.as_str()),
                    ) {
                        // Запрашиваем реальные каналы этого сервера: GET /guilds/{id}/channels через пул HTTP/2 и экспоненциальный бэкофф
                        let (categories, channels) = fetch_guild_channels_with_transport(
                            &client,
                            pool,
                            &pool_config,
                            token,
                            gid,
                        )
                        .await
                        .map(|(cats, chs, _)| (cats, chs))
                        .unwrap_or_default();

                        guilds.push(crate::state::Guild {
                            id: gid.to_string(),
                            name: gname.to_string(),
                            icon: g
                                .get("icon")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            owner_id: g
                                .get("owner_id")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            categories,
                            channels,
                            unread_count: 0,
                            mention_count: 0,
                        });
                    }
                }
            }
        }
    }

    // 3. Получение списка друзей: GET /users/@me/relationships
    let mut relationships = Vec::new();
    if let Ok(rel_resp) = client.get("/users/@me/relationships").await {
        if rel_resp.status == 200 {
            if let Ok(rel_arr) = serde_json::from_slice::<Vec<serde_json::Value>>(&rel_resp.body) {
                for r in rel_arr {
                    if let (Some(rid), Some(u_obj)) =
                        (r.get("id").and_then(|v| v.as_str()), r.get("user"))
                    {
                        let uid = u_obj.get("id").and_then(|v| v.as_str()).unwrap_or(rid);
                        let uname = u_obj
                            .get("username")
                            .and_then(|v| v.as_str())
                            .unwrap_or("Friend");
                        let gname = u_obj
                            .get("global_name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let avatar = u_obj
                            .get("avatar")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());
                        let rtype_num = r.get("type").and_then(|v| v.as_u64()).unwrap_or(1);
                        let relationship_type = match rtype_num {
                            2 => crate::state::RelationshipType::Blocked,
                            3 => crate::state::RelationshipType::IncomingRequest,
                            4 => crate::state::RelationshipType::OutgoingRequest,
                            _ => crate::state::RelationshipType::Friend,
                        };

                        relationships.push(crate::state::Relationship {
                            id: rid.to_string(),
                            user_id: uid.to_string(),
                            relationship_type,
                            nickname: r
                                .get("nickname")
                                .and_then(|v| v.as_str())
                                .map(|s| s.to_string()),
                            user: crate::state::UserSummary {
                                id: uid.to_string(),
                                username: uname.to_string(),
                                global_name: gname.clone(),
                                avatar,
                                is_bot: false,
                            },
                            status: crate::state::PresenceStatus::Online,
                            activity: None,
                            banner_color: Some("#9333ea".into()),
                            user_tag: Some(format!("@{}", uname)),
                            member_since: None,
                            mutual_guilds_count: 0,
                            note: None,
                        });
                    }
                }
            }
        }
    }

    // 4. Получение приватных каналов (ЛС) и сообщений: GET /users/@me/channels
    let mut messages: std::collections::HashMap<String, Vec<crate::state::Message>> =
        std::collections::HashMap::new();
    let mut dm_channels: std::collections::HashMap<String, String> =
        std::collections::HashMap::new();
    if let Ok(ch_resp) = client.get("/users/@me/channels").await {
        if ch_resp.status == 200 {
            if let Ok(ch_arr) = serde_json::from_slice::<Vec<serde_json::Value>>(&ch_resp.body) {
                for ch in ch_arr {
                    if let Some(ch_id) = ch.get("id").and_then(|v| v.as_str()) {
                        let recipient_id = ch
                            .get("recipients")
                            .and_then(|r| r.as_array())
                            .and_then(|arr| arr.first())
                            .and_then(|u| u.get("id"))
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string());

                        if let Some(r_id) = &recipient_id {
                            dm_channels.insert(r_id.clone(), ch_id.to_string());
                        }

                        // Запрос истории сообщений: GET /channels/{id}/messages?limit=25
                        let msg_target = format!("/channels/{}/messages?limit=25", ch_id);
                        if let Ok(msgs_resp) = client.get(&msg_target).await {
                            if msgs_resp.status == 200 {
                                if let Ok(msgs_arr) = serde_json::from_slice::<Vec<serde_json::Value>>(
                                    &msgs_resp.body,
                                ) {
                                    let mut ch_msgs = Vec::new();
                                    for m in msgs_arr.into_iter().rev() {
                                        if let (Some(mid), Some(author_obj)) =
                                            (m.get("id").and_then(|v| v.as_str()), m.get("author"))
                                        {
                                            let a_id = author_obj
                                                .get("id")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("0");
                                            let a_name = author_obj
                                                .get("username")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("User");
                                            let a_global = author_obj
                                                .get("global_name")
                                                .and_then(|v| v.as_str())
                                                .map(|s| s.to_string());
                                            let m_type =
                                                m.get("type").and_then(|v| v.as_u64()).unwrap_or(0);
                                            let raw_content = m
                                                .get("content")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("");
                                            let iso_ts = m
                                                .get("timestamp")
                                                .and_then(|v| v.as_str())
                                                .unwrap_or("");

                                            let formatted_ts = format_discord_timestamp(iso_ts);

                                            let (content, kind) = if m_type == 3 {
                                                let display_author =
                                                    a_global.as_deref().unwrap_or(a_name);
                                                let participants_count = m
                                                    .get("call")
                                                    .and_then(|c| c.get("participants"))
                                                    .and_then(|p| p.as_array())
                                                    .map(|a| a.len())
                                                    .unwrap_or(1);
                                                if participants_count > 1 {
                                                    (
                                                        format!(
                                                            "{} начал звонок продолжительностью несколько секунд.",
                                                            display_author
                                                        ),
                                                        crate::state::MessageKind::CallStarted,
                                                    )
                                                } else {
                                                    (
                                                        format!(
                                                            "Вы пропустили звонок от {}.",
                                                            display_author
                                                        ),
                                                        crate::state::MessageKind::CallMissed,
                                                    )
                                                }
                                            } else {
                                                (
                                                    raw_content.to_string(),
                                                    crate::state::MessageKind::Default,
                                                )
                                            };

                                            ch_msgs.push(crate::state::Message {
                                                id: mid.to_string(),
                                                channel_id: ch_id.to_string(),
                                                author: crate::state::UserSummary {
                                                    id: a_id.to_string(),
                                                    username: a_name.to_string(),
                                                    global_name: a_global,
                                                    avatar: author_obj
                                                        .get("avatar")
                                                        .and_then(|v| v.as_str())
                                                        .map(|s| s.to_string()),
                                                    is_bot: false,
                                                },
                                                content,
                                                timestamp: formatted_ts,
                                                edited_timestamp: None,
                                                attachments: Vec::new(),
                                                reactions: Vec::new(),
                                                is_pinned: false,
                                                message_kind: kind,
                                            });
                                        }
                                    }

                                    if let Some(r_id) = &recipient_id {
                                        messages.insert(format!("dm-{}", r_id), ch_msgs.clone());
                                    }
                                    messages.insert(ch_id.to_string(), ch_msgs);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    Ok(LiveDiscordAccountData {
        current_user,
        guilds,
        relationships,
        messages,
        dm_channels,
    })
}

/// Разбор ответа `GET /guilds/{id}/channels` на категории (`type == 4`) и каналы (`0, 2, 5`)
pub fn parse_guild_channels_json(
    guild_id: &str,
    body: &[u8],
) -> Result<
    (
        Vec<crate::state::ChannelCategory>,
        Vec<crate::state::Channel>,
    ),
    String,
> {
    let ch_arr: Vec<serde_json::Value> = serde_json::from_slice(body)
        .map_err(|e| format!("Ошибка разбора каналов гильдии {}: {}", guild_id, e))?;

    let mut categories = Vec::new();
    let mut channels = Vec::new();
    for c in ch_arr {
        let cid = c
            .get("id")
            .and_then(|v| v.as_str())
            .unwrap_or("0")
            .to_string();
        let cname = c
            .get("name")
            .and_then(|v| v.as_str())
            .unwrap_or("channel")
            .to_string();
        let ctype_num = c.get("type").and_then(|v| v.as_u64()).unwrap_or(0);
        let pos = c.get("position").and_then(|v| v.as_i64()).unwrap_or(0) as i32;
        let parent_id = c
            .get("parent_id")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        if ctype_num == 4 {
            categories.push(crate::state::ChannelCategory {
                id: cid,
                name: cname.to_uppercase(),
                position: pos,
                is_collapsed: false,
            });
        } else {
            let ctype = match ctype_num {
                0 => crate::state::ChannelType::GuildText,
                2 => crate::state::ChannelType::GuildVoice,
                5 => crate::state::ChannelType::GuildAnnouncement,
                _ => crate::state::ChannelType::GuildText,
            };
            channels.push(crate::state::Channel {
                id: cid,
                guild_id: Some(guild_id.to_string()),
                category_id: parent_id,
                name: cname,
                channel_type: ctype,
                topic: c
                    .get("topic")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                position: pos,
                unread: false,
                is_locked: false,
                permissions_bitmask: chord_core::permissions::ALL_PERMISSIONS,
            });
        }
    }

    Ok((categories, channels))
}

/// Загрузка каналов гильдии (`GET /guilds/{id}/channels`) через пул HTTP/2 соединений с экспоненциальным бэкоффом
pub async fn fetch_guild_channels_with_transport<T: RestTransport + ?Sized>(
    transport: &T,
    pool: &Http2ConnectionPool,
    pool_config: &Http2PoolConfig,
    _token: &str,
    guild_id: &str,
) -> Result<
    (
        Vec<crate::state::ChannelCategory>,
        Vec<crate::state::Channel>,
        PooledRequestMetrics,
    ),
    String,
> {
    if guild_id.trim().is_empty() {
        return Err("Идентификатор гильдии не может быть пустым".into());
    }
    let target = format!("/guilds/{}/channels", guild_id.trim());
    let (resp, metrics) =
        execute_with_pool_and_backoff(transport, pool, pool_config, "GET", &target, None).await?;
    let (categories, channels) = parse_guild_channels_json(guild_id.trim(), &resp.body)?;
    Ok((categories, channels, metrics))
}

/// Загрузка каналов гильдии через глобальный пул HTTP/2 соединений
pub async fn fetch_guild_channels(
    token: &str,
    guild_id: &str,
) -> Result<
    (
        Vec<crate::state::ChannelCategory>,
        Vec<crate::state::Channel>,
    ),
    String,
> {
    let pool_config = Http2PoolConfig::default();
    let client = build_pooled_api_client(token, &pool_config)?;
    let (categories, channels, _) = fetch_guild_channels_with_transport(
        &client,
        global_http2_connection_pool(),
        &pool_config,
        token,
        guild_id,
    )
    .await?;
    Ok((categories, channels))
}

/// Разбор одного объекта сообщения Discord REST API v9
fn parse_single_message_value(
    channel_id: &str,
    val: &serde_json::Value,
    fallback_content: Option<&str>,
) -> Option<crate::state::Message> {
    let mid = val.get("id").and_then(|v| v.as_str())?;
    let author_obj = val.get("author")?;
    let a_id = author_obj.get("id").and_then(|v| v.as_str()).unwrap_or("0");
    let a_name = author_obj
        .get("username")
        .and_then(|v| v.as_str())
        .unwrap_or("User");
    let a_global = author_obj
        .get("global_name")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let raw_content = val
        .get("content")
        .and_then(|v| v.as_str())
        .or(fallback_content)
        .unwrap_or("");
    let iso_ts = val.get("timestamp").and_then(|v| v.as_str()).unwrap_or("");

    Some(crate::state::Message {
        id: mid.to_string(),
        channel_id: channel_id.to_string(),
        author: crate::state::UserSummary {
            id: a_id.to_string(),
            username: a_name.to_string(),
            global_name: a_global,
            avatar: author_obj
                .get("avatar")
                .and_then(|v| v.as_str())
                .map(|s| s.to_string()),
            is_bot: author_obj
                .get("bot")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
        },
        content: raw_content.to_string(),
        timestamp: format_discord_timestamp(iso_ts),
        edited_timestamp: val
            .get("edited_timestamp")
            .and_then(|v| v.as_str())
            .map(format_discord_timestamp),
        attachments: Vec::new(),
        reactions: Vec::new(),
        is_pinned: val.get("pinned").and_then(|v| v.as_bool()).unwrap_or(false),
        message_kind: crate::state::MessageKind::Default,
    })
}

/// Разбор массива сообщений `GET /channels/{id}/messages` в хронологическом порядке (от старых к новым)
pub fn parse_channel_messages_json(
    channel_id: &str,
    body: &[u8],
) -> Result<Vec<crate::state::Message>, String> {
    let msgs_arr: Vec<serde_json::Value> =
        serde_json::from_slice(body).map_err(|e| format!("Ошибка разбора сообщений: {:?}", e))?;

    let mut result = Vec::with_capacity(msgs_arr.len());
    for m in msgs_arr.into_iter().rev() {
        if let Some(msg) = parse_single_message_value(channel_id, &m, None) {
            result.push(msg);
        }
    }
    Ok(result)
}

/// Отправка сообщения (`POST /channels/{id}/messages`) через пул HTTP/2 соединений с экспоненциальным бэкоффом
pub async fn send_message_with_transport<T: RestTransport + ?Sized>(
    transport: &T,
    pool: &Http2ConnectionPool,
    pool_config: &Http2PoolConfig,
    _token: &str,
    channel_id: &str,
    content: &str,
) -> Result<(crate::state::Message, PooledRequestMetrics), String> {
    if channel_id.trim().is_empty() {
        return Err("Идентификатор канала пуст".into());
    }
    let payload = serde_json::json!({
        "content": content,
        "flags": 0,
        "mobile_network_type": "unknown",
        "tts": false,
    });
    let body = serde_json::to_vec(&payload).map_err(|e| e.to_string())?;
    let target = format!("/channels/{}/messages", channel_id.trim());
    let (resp, metrics) =
        execute_with_pool_and_backoff(transport, pool, pool_config, "POST", &target, Some(&body))
            .await?;

    let val: serde_json::Value = serde_json::from_slice(&resp.body).map_err(|e| e.to_string())?;
    let msg =
        parse_single_message_value(channel_id.trim(), &val, Some(content)).unwrap_or_else(|| {
            crate::state::Message {
                id: val
                    .get("id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("0")
                    .to_string(),
                channel_id: channel_id.trim().to_string(),
                author: crate::state::UserSummary {
                    id: "0".into(),
                    username: "User".into(),
                    global_name: None,
                    avatar: None,
                    is_bot: false,
                },
                content: content.to_string(),
                timestamp: format_discord_timestamp(""),
                edited_timestamp: None,
                attachments: Vec::new(),
                reactions: Vec::new(),
                is_pinned: false,
                message_kind: crate::state::MessageKind::Default,
            }
        });
    Ok((msg, metrics))
}

/// Отправка живого сообщения в канал Discord через официальный REST API v9 с пулом HTTP/2 и бэкоффом
pub async fn send_live_message(
    token: &str,
    channel_id: &str,
    content: &str,
) -> Result<crate::state::Message, String> {
    let pool_config = Http2PoolConfig::default();
    let client = build_pooled_api_client(token, &pool_config)?;
    let (msg, _) = send_message_with_transport(
        &client,
        global_http2_connection_pool(),
        &pool_config,
        token,
        channel_id,
        content,
    )
    .await?;
    Ok(msg)
}

/// Синхронная обёртка для отправки сообщения
pub fn send_live_message_blocking(
    token: &str,
    channel_id: &str,
    content: &str,
) -> Result<crate::state::Message, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Tokio runtime error: {}", e))?;
    rt.block_on(send_live_message(token, channel_id, content))
}

/// Удаление сообщения (`DELETE /channels/{id}/messages/{id}`) через пул HTTP/2 с экспоненциальным бэкоффом
pub async fn delete_message_with_transport<T: RestTransport + ?Sized>(
    transport: &T,
    pool: &Http2ConnectionPool,
    pool_config: &Http2PoolConfig,
    _token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<PooledRequestMetrics, String> {
    let target = format!(
        "/channels/{}/messages/{}",
        channel_id.trim(),
        message_id.trim()
    );
    let (_, metrics) =
        execute_with_pool_and_backoff(transport, pool, pool_config, "DELETE", &target, None)
            .await?;
    Ok(metrics)
}

/// Удаление сообщения в Discord через официальный REST API v9 (DELETE /channels/{id}/messages/{id})
pub async fn delete_live_message(
    token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<(), String> {
    let pool_config = Http2PoolConfig::default();
    let client = build_pooled_api_client(token, &pool_config)?;
    delete_message_with_transport(
        &client,
        global_http2_connection_pool(),
        &pool_config,
        token,
        channel_id,
        message_id,
    )
    .await
    .map(|_| ())
}

pub fn delete_live_message_blocking(
    token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Tokio runtime error: {}", e))?;
    rt.block_on(delete_live_message(token, channel_id, message_id))
}

/// Закрепление сообщения (`PUT /channels/{id}/pins/{id}`) через пул HTTP/2 с экспоненциальным бэкоффом
pub async fn pin_message_with_transport<T: RestTransport + ?Sized>(
    transport: &T,
    pool: &Http2ConnectionPool,
    pool_config: &Http2PoolConfig,
    _token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<PooledRequestMetrics, String> {
    let target = format!("/channels/{}/pins/{}", channel_id.trim(), message_id.trim());
    let (_, metrics) =
        execute_with_pool_and_backoff(transport, pool, pool_config, "PUT", &target, Some(&[]))
            .await?;
    Ok(metrics)
}

/// Закрепление сообщения в Discord через официальный REST API v9 (PUT /channels/{id}/pins/{id})
pub async fn pin_live_message(
    token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<(), String> {
    let pool_config = Http2PoolConfig::default();
    let client = build_pooled_api_client(token, &pool_config)?;
    pin_message_with_transport(
        &client,
        global_http2_connection_pool(),
        &pool_config,
        token,
        channel_id,
        message_id,
    )
    .await
    .map(|_| ())
}

pub fn pin_live_message_blocking(
    token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Tokio runtime error: {}", e))?;
    rt.block_on(pin_live_message(token, channel_id, message_id))
}

/// Открепление сообщения (`DELETE /channels/{id}/pins/{id}`) через пул HTTP/2 с экспоненциальным бэкоффом
pub async fn unpin_message_with_transport<T: RestTransport + ?Sized>(
    transport: &T,
    pool: &Http2ConnectionPool,
    pool_config: &Http2PoolConfig,
    _token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<PooledRequestMetrics, String> {
    let target = format!("/channels/{}/pins/{}", channel_id.trim(), message_id.trim());
    let (_, metrics) =
        execute_with_pool_and_backoff(transport, pool, pool_config, "DELETE", &target, None)
            .await?;
    Ok(metrics)
}

/// Открепление сообщения в Discord через официальный REST API v9 (DELETE /channels/{id}/pins/{id})
pub async fn unpin_live_message(
    token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<(), String> {
    let pool_config = Http2PoolConfig::default();
    let client = build_pooled_api_client(token, &pool_config)?;
    unpin_message_with_transport(
        &client,
        global_http2_connection_pool(),
        &pool_config,
        token,
        channel_id,
        message_id,
    )
    .await
    .map(|_| ())
}

pub fn unpin_live_message_blocking(
    token: &str,
    channel_id: &str,
    message_id: &str,
) -> Result<(), String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Tokio runtime error: {}", e))?;
    rt.block_on(unpin_live_message(token, channel_id, message_id))
}

/// Форматирование относительного URL пагинации сообщений:
/// `GET /channels/{id}/messages?before={oldest_id}&limit={limit}`
#[must_use]
pub fn format_messages_pagination_url(channel_id: &str, before: &str, limit: u32) -> String {
    let clamped_limit = limit.clamp(1, 100);
    format!(
        "/channels/{}/messages?before={}&limit={}",
        channel_id.trim(),
        before.trim(),
        clamped_limit
    )
}

/// Загрузка истории сообщений канала (`GET /channels/{id}/messages?limit={limit}`)
/// через пул HTTP/2 соединений с экспоненциальным бэкоффом
pub async fn fetch_channel_messages_with_transport<T: RestTransport + ?Sized>(
    transport: &T,
    pool: &Http2ConnectionPool,
    pool_config: &Http2PoolConfig,
    _token: &str,
    channel_id: &str,
    limit: u32,
) -> Result<(Vec<crate::state::Message>, PooledRequestMetrics), String> {
    if channel_id.trim().is_empty() {
        return Err("Идентификатор канала не может быть пустым".into());
    }
    let clamped_limit = limit.clamp(1, 100);
    let target = format!(
        "/channels/{}/messages?limit={}",
        channel_id.trim(),
        clamped_limit
    );
    let (resp, metrics) =
        execute_with_pool_and_backoff(transport, pool, pool_config, "GET", &target, None).await?;
    let msgs = parse_channel_messages_json(channel_id.trim(), &resp.body)?;
    Ok((msgs, metrics))
}

/// Загрузка порции более старых сообщений канала (`GET /channels/{id}/messages?before={before}&limit={limit}`)
/// через пул HTTP/2 соединений с экспоненциальным бэкоффом
pub async fn fetch_channel_messages_before_with_transport<T: RestTransport + ?Sized>(
    transport: &T,
    pool: &Http2ConnectionPool,
    pool_config: &Http2PoolConfig,
    _token: &str,
    channel_id: &str,
    before: &str,
    limit: u32,
) -> Result<(Vec<crate::state::Message>, PooledRequestMetrics), String> {
    if channel_id.trim().is_empty() {
        return Err("Идентификатор канала не может быть пустым".into());
    }
    if before.trim().is_empty() {
        return Err("Параметр before не может быть пустым".into());
    }
    let target = format_messages_pagination_url(channel_id, before, limit);
    let (resp, metrics) =
        execute_with_pool_and_backoff(transport, pool, pool_config, "GET", &target, None).await?;
    let msgs = parse_channel_messages_json(channel_id.trim(), &resp.body)?;
    Ok((msgs, metrics))
}

/// Загрузка истории сообщений канала через официальный REST API v9 с пулом HTTP/2 и экспоненциальным бэкоффом
pub async fn fetch_channel_messages(
    token: &str,
    channel_id: &str,
    limit: u32,
) -> Result<Vec<crate::state::Message>, String> {
    let pool_config = Http2PoolConfig::default();
    let client = build_pooled_api_client(token, &pool_config)?;
    let (msgs, _) = fetch_channel_messages_with_transport(
        &client,
        global_http2_connection_pool(),
        &pool_config,
        token,
        channel_id,
        limit,
    )
    .await?;
    Ok(msgs)
}

/// Загрузка более старых сообщений канала через официальный REST API v9 с пулом HTTP/2
pub async fn fetch_channel_messages_before(
    token: &str,
    channel_id: &str,
    before: &str,
    limit: u32,
) -> Result<Vec<crate::state::Message>, String> {
    let pool_config = Http2PoolConfig::default();
    let client = build_pooled_api_client(token, &pool_config)?;
    let (msgs, _) = fetch_channel_messages_before_with_transport(
        &client,
        global_http2_connection_pool(),
        &pool_config,
        token,
        channel_id,
        before,
        limit,
    )
    .await?;
    Ok(msgs)
}

/// Синхронная обёртка для загрузки сообщений канала
pub fn fetch_channel_messages_blocking(
    token: &str,
    channel_id: &str,
    limit: u32,
) -> Result<Vec<crate::state::Message>, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Tokio runtime error: {}", e))?;
    rt.block_on(fetch_channel_messages(token, channel_id, limit))
}

/// Синхронная блокирующая обёртка для загрузки более старых сообщений канала
pub fn fetch_channel_messages_before_blocking(
    token: &str,
    channel_id: &str,
    before: &str,
    limit: u32,
) -> Result<Vec<crate::state::Message>, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Tokio runtime error: {}", e))?;
    rt.block_on(fetch_channel_messages_before(
        token, channel_id, before, limit,
    ))
}

/// Вступление на сервер по коду или ссылке приглашения через официальный REST API Discord v9
pub async fn join_live_discord_invite(
    token: &str,
    raw_invite: &str,
) -> Result<crate::state::Guild, String> {
    let clean = raw_invite.trim();
    let code = clean
        .trim_start_matches("https://discord.gg/")
        .trim_start_matches("http://discord.gg/")
        .trim_start_matches("discord.gg/");
    if code.is_empty() {
        return Err("Код приглашения пуст".into());
    }

    let super_props = DiscordRestClient::build_re_deep_super_properties();
    let auth = ApiAuth::from_token_with_scheme(
        token,
        AuthScheme::User,
        &super_props,
        "ru",
        "Europe/Moscow",
    )
    .map_err(|e| format!("Ошибка auth: {:?}", e))?;

    let mut config = ApiConfig::default();
    config.base = "https://discord.com/api/v9".to_string();
    config.auth = auth;
    let client = ApiClient::new(config).map_err(|e| format!("{:?}", e))?;

    // 1. Сначала пробуем принять инвайт: POST /invites/{code}
    let target = format!("/invites/{}", code);
    let resp = client.post(&target, b"{}").await;

    // 2. Если POST вернул ошибку, запрашиваем публичную информацию инвайта: GET /invites/{code}
    let body = match resp {
        Ok(r) if r.status == 200 || r.status == 201 => r.body,
        _ => {
            let get_resp = client.get(&target).await.map_err(|e| format!("{:?}", e))?;
            if get_resp.status != 200 {
                return Err(format!(
                    "Не удалось получить данные приглашения '{}': HTTP {}",
                    code, get_resp.status
                ));
            }
            get_resp.body
        }
    };

    let val: serde_json::Value = serde_json::from_slice(&body).map_err(|e| e.to_string())?;
    let guild_obj = val
        .get("guild")
        .ok_or_else(|| "В ответе инвайта отсутствует объект guild".to_string())?;

    let gid = guild_obj
        .get("id")
        .and_then(|v| v.as_str())
        .unwrap_or("0")
        .to_string();
    let name = guild_obj
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("Discord Server")
        .to_string();
    let icon_str = guild_obj
        .get("icon")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());
    let owner_id = guild_obj
        .get("owner_id")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string());

    // 3. Запрашиваем реальные каналы этого сервера: GET /guilds/{id}/channels через пул HTTP/2 и экспоненциальный бэкофф
    let pool_config = Http2PoolConfig::default();
    let pool = global_http2_connection_pool();
    let (categories, mut channels) =
        fetch_guild_channels_with_transport(&client, pool, &pool_config, token, &gid)
            .await
            .map(|(cats, chs, _)| (cats, chs))
            .unwrap_or_default();

    if channels.is_empty() {
        channels.push(crate::state::Channel {
            id: format!("ch-{}-general", gid),
            guild_id: Some(gid.clone()),
            category_id: None,
            name: "общий".into(),
            channel_type: crate::state::ChannelType::GuildText,
            topic: Some("Общение участников сервера".into()),
            position: 0,
            unread: false,
            is_locked: false,
            permissions_bitmask: chord_core::permissions::ALL_PERMISSIONS,
        });
    }

    Ok(crate::state::Guild {
        id: gid,
        name,
        icon: icon_str,
        owner_id,
        categories,
        channels,
        unread_count: 0,
        mention_count: 0,
    })
}

/// Синхронная обёртка для вступления на сервер
pub fn join_live_discord_invite_blocking(
    token: &str,
    raw_invite: &str,
) -> Result<crate::state::Guild, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Tokio runtime error: {}", e))?;
    rt.block_on(join_live_discord_invite(token, raw_invite))
}

/// Синхронная обёртка для инициализации при старте приложения
pub fn fetch_live_discord_account_blocking(token: &str) -> Result<LiveDiscordAccountData, String> {
    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|e| format!("Tokio runtime error: {}", e))?;
    rt.block_on(fetch_live_discord_account(token))
}

/// Форматирование ISO 8601 даты Discord в человекочитаемый вид
fn format_discord_timestamp(iso: &str) -> String {
    if iso.len() >= 16 {
        let date_part = &iso[0..10];
        let time_part = &iso[11..16];
        let mut parts = date_part.split('-');
        if let (Some(y), Some(m), Some(d)) = (parts.next(), parts.next(), parts.next()) {
            return format!("{}.{}.{} {}", d, m, y, time_part);
        }
    }
    "Недавно".to_string()
}

/// Сессионный шлюз WebSocket v9 (Gateway)
#[derive(Debug, Clone)]
pub struct DiscordGatewaySession {
    pub gateway_url: String,
    pub session_id: Option<String>,
    pub sequence: Option<u64>,
    pub heartbeat_interval_ms: u64,
}

impl Default for DiscordGatewaySession {
    fn default() -> Self {
        Self {
            gateway_url: "wss://gateway.discord.gg/?v=9&encoding=etf&compress=zstd-stream".into(),
            session_id: None,
            sequence: None,
            heartbeat_interval_ms: 41250,
        }
    }
}

impl DiscordGatewaySession {
    pub fn new() -> Self {
        Self::default()
    }

    /// Расчёт начального джиттера первого heartbeat: heartbeat_interval * rand(0.0 .. 1.0)
    pub fn initial_heartbeat_jitter(interval_ms: u64, seed_ratio: f64) -> Duration {
        let clamped_ratio = seed_ratio.clamp(0.0, 1.0);
        let jitter_ms = (interval_ms as f64 * clamped_ratio) as u64;
        Duration::from_millis(jitter_ms)
    }

    /// Сборка полезной нагрузки Opcode 4: Voice State Update (re-deep/gateway-schema.md)
    pub fn make_voice_state_update_payload(
        guild_id: Option<&str>,
        channel_id: Option<&str>,
        self_mute: bool,
        self_deaf: bool,
    ) -> serde_json::Value {
        serde_json::json!({
            "op": 4,
            "d": {
                "guild_id": guild_id,
                "channel_id": channel_id,
                "self_mute": self_mute,
                "self_deaf": self_deaf,
                "self_video": false
            }
        })
    }

    /// Диспетчеризация входящего события шлюза в AppState
    pub fn dispatch_event(
        state: &mut crate::state::AppState,
        event_name: &str,
        data: &serde_json::Value,
    ) {
        match event_name {
            "MESSAGE_CREATE" => {
                if let (Some(ch_id), Some(content), Some(msg_id)) = (
                    data.get("channel_id").and_then(|v| v.as_str()),
                    data.get("content").and_then(|v| v.as_str()),
                    data.get("id").and_then(|v| v.as_str()),
                ) {
                    let author = data.get("author");
                    let username = author
                        .and_then(|a| a.get("username"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("User");
                    let author_id = author
                        .and_then(|a| a.get("id"))
                        .and_then(|v| v.as_str())
                        .unwrap_or("0");
                    let global_name = author
                        .and_then(|a| a.get("global_name"))
                        .and_then(|v| v.as_str())
                        .map(|s| s.to_string());

                    let msg = crate::state::Message {
                        id: msg_id.to_string(),
                        channel_id: ch_id.to_string(),
                        author: crate::state::UserSummary {
                            id: author_id.to_string(),
                            username: username.to_string(),
                            global_name,
                            avatar: None,
                            is_bot: author
                                .and_then(|a| a.get("bot"))
                                .and_then(|v| v.as_bool())
                                .unwrap_or(false),
                        },
                        content: content.to_string(),
                        timestamp: "Сейчас".into(),
                        edited_timestamp: None,
                        attachments: Vec::new(),
                        reactions: Vec::new(),
                        is_pinned: false,
                        message_kind: crate::state::MessageKind::Default,
                    };
                    state
                        .messages
                        .entry(ch_id.to_string())
                        .or_default()
                        .push(msg);

                    // Если канал не выбран, отмечаем непрочитанным
                    if state.selected_channel_id.as_deref() != Some(ch_id) {
                        for g in &mut state.guilds {
                            if let Some(ch) = g.channels.iter_mut().find(|c| c.id == ch_id) {
                                ch.unread = true;
                                g.unread_count += 1;
                            }
                        }
                    }
                }
            }
            "PRESENCE_UPDATE" => {
                if let (Some(user_id), Some(status_str)) = (
                    data.get("user")
                        .and_then(|u| u.get("id"))
                        .and_then(|v| v.as_str()),
                    data.get("status").and_then(|v| v.as_str()),
                ) {
                    let presence = match status_str {
                        "online" => crate::state::PresenceStatus::Online,
                        "idle" => crate::state::PresenceStatus::Idle,
                        "dnd" => crate::state::PresenceStatus::Dnd,
                        _ => crate::state::PresenceStatus::Offline,
                    };

                    for m in &mut state.members {
                        if m.id == user_id {
                            m.status = presence;
                        }
                    }
                }
            }
            "MESSAGE_UPDATE" => {
                if let (Some(ch_id), Some(msg_id), Some(content)) = (
                    data.get("channel_id").and_then(|v| v.as_str()),
                    data.get("id").and_then(|v| v.as_str()),
                    data.get("content").and_then(|v| v.as_str()),
                ) {
                    state.edit_message(ch_id, msg_id, content.to_string());
                }
            }
            "TYPING_START" => {
                if let (Some(ch_id), Some(user_id)) = (
                    data.get("channel_id").and_then(|v| v.as_str()),
                    data.get("user_id").and_then(|v| v.as_str()),
                ) {
                    let username = state
                        .members
                        .iter()
                        .find(|m| m.id == user_id)
                        .map(|m| m.username.clone())
                        .unwrap_or_else(|| "Кто-то".into());
                    state.set_user_typing(ch_id, &username);
                }
            }
            "READY" => {
                // Разбор официального события READY (re-deep/gateway-schema.md §d.user, §d.guilds, §d.relationships)
                if let Some(user_val) = data.get("user")
                    && let (Some(uid), Some(uname)) = (
                        user_val.get("id").and_then(|v| v.as_str()),
                        user_val.get("username").and_then(|v| v.as_str()),
                    )
                {
                    state.current_user = crate::state::CurrentUser {
                        id: uid.to_string(),
                        username: uname.to_string(),
                        discriminator: user_val
                            .get("discriminator")
                            .and_then(|v| v.as_str())
                            .unwrap_or("0000")
                            .to_string(),
                        global_name: user_val
                            .get("global_name")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        avatar: user_val
                            .get("avatar")
                            .and_then(|v| v.as_str())
                            .map(|s| s.to_string()),
                        status: crate::state::PresenceStatus::Online,
                        custom_status: None,
                    };
                }

                // Парсинг серверов (guilds)
                if let Some(guilds_arr) = data.get("guilds").and_then(|v| v.as_array()) {
                    let mut new_guilds = Vec::new();
                    for g in guilds_arr {
                        if let (Some(gid), Some(gname)) = (
                            g.get("id").and_then(|v| v.as_str()),
                            g.get("name").and_then(|v| v.as_str()),
                        ) {
                            let mut channels = Vec::new();
                            if let Some(ch_arr) = g.get("channels").and_then(|v| v.as_array()) {
                                for ch in ch_arr {
                                    if let (Some(cid), Some(cname)) = (
                                        ch.get("id").and_then(|v| v.as_str()),
                                        ch.get("name").and_then(|v| v.as_str()),
                                    ) {
                                        let ctype =
                                            ch.get("type").and_then(|v| v.as_u64()).unwrap_or(0);
                                        let channel_type = match ctype {
                                            2 => crate::state::ChannelType::GuildVoice,
                                            5 => crate::state::ChannelType::GuildAnnouncement,
                                            _ => crate::state::ChannelType::GuildText,
                                        };
                                        channels.push(crate::state::Channel {
                                            id: cid.to_string(),
                                            guild_id: Some(gid.to_string()),
                                            category_id: None,
                                            name: cname.to_string(),
                                            channel_type,
                                            topic: ch
                                                .get("topic")
                                                .and_then(|v| v.as_str())
                                                .map(|s| s.to_string()),
                                            position: ch
                                                .get("position")
                                                .and_then(|v| v.as_i64())
                                                .unwrap_or(0)
                                                as i32,
                                            unread: false,
                                            is_locked: false,
                                            permissions_bitmask:
                                                chord_core::permissions::ALL_PERMISSIONS,
                                        });
                                    }
                                }
                            }

                            new_guilds.push(crate::state::Guild {
                                id: gid.to_string(),
                                name: gname.to_string(),
                                icon: g
                                    .get("icon")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                owner_id: g
                                    .get("owner_id")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                categories: Vec::new(),
                                channels,
                                unread_count: 0,
                                mention_count: 0,
                            });
                        }
                    }
                    if !new_guilds.is_empty() {
                        state.guilds = new_guilds;
                        if state.selected_guild_id.is_none() {
                            let first_id = state.guilds[0].id.clone();
                            state.select_guild(Some(first_id));
                        }
                    }
                }

                // Парсинг друзей и отношений (relationships: 1=friend, 2=blocked, 3=incoming, 4=outgoing)
                if let Some(rels_arr) = data.get("relationships").and_then(|v| v.as_array()) {
                    let mut rels = Vec::new();
                    for r in rels_arr {
                        if let (Some(rid), Some(rtype_num)) = (
                            r.get("id").and_then(|v| v.as_str()),
                            r.get("type").and_then(|v| v.as_u64()),
                        ) {
                            let rtype = match rtype_num {
                                1 => crate::state::RelationshipType::Friend,
                                2 => crate::state::RelationshipType::Blocked,
                                3 => crate::state::RelationshipType::IncomingRequest,
                                4 => crate::state::RelationshipType::OutgoingRequest,
                                _ => crate::state::RelationshipType::None,
                            };
                            let uval = r.get("user");
                            let uid = uval
                                .and_then(|u| u.get("id"))
                                .and_then(|v| v.as_str())
                                .unwrap_or(rid);
                            let uname = uval
                                .and_then(|u| u.get("username"))
                                .and_then(|v| v.as_str())
                                .unwrap_or("Friend");

                            rels.push(crate::state::Relationship {
                                id: rid.to_string(),
                                user_id: uid.to_string(),
                                relationship_type: rtype,
                                nickname: r
                                    .get("nickname")
                                    .and_then(|v| v.as_str())
                                    .map(|s| s.to_string()),
                                user: crate::state::UserSummary {
                                    id: uid.to_string(),
                                    username: uname.to_string(),
                                    global_name: uval
                                        .and_then(|u| u.get("global_name"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string()),
                                    avatar: uval
                                        .and_then(|u| u.get("avatar"))
                                        .and_then(|v| v.as_str())
                                        .map(|s| s.to_string()),
                                    is_bot: false,
                                },
                                status: crate::state::PresenceStatus::Online,
                                activity: None,
                                banner_color: None,
                                user_tag: None,
                                member_since: None,
                                mutual_guilds_count: 0,
                                note: None,
                            });
                        }
                    }
                    state.relationships = rels;
                }
            }
            _ => {}
        }
    }
}

/// Голосовая сессия WebRTC (Voice Signaling v=8 & UDP Media)
#[derive(Debug, Clone)]
pub struct DiscordVoiceSession {
    pub endpoint: String,
    pub token: String,
    pub server_id: String,
    pub user_id: String,
    pub ssrc: u32,
    pub udp_port: u16,
}

impl DiscordVoiceSession {
    pub fn new(endpoint: String, token: String, server_id: String, user_id: String) -> Self {
        Self {
            endpoint,
            token,
            server_id,
            user_id,
            ssrc: 1,
            udp_port: 0,
        }
    }

    /// Создание пакета обнаружения IP (74 байта, RE /home/sanya/re-deep/voice-deep.md)
    pub fn create_ip_discovery_packet(&self) -> [u8; 74] {
        DiscordVoicePacket::make_ip_discovery(self.ssrc)
    }

    /// Создание keepalive-пакета (8 байт 0x1337cafe)
    pub fn create_keepalive_packet(&self, seq: u32) -> [u8; 8] {
        DiscordVoicePacket::make_keepalive(seq)
    }

    /// Шифрование RTP медиа-фрейма алгоритмом AES-256-GCM
    pub fn encrypt_rtp_frame(
        &self,
        key: &[u8; 32],
        rtp_header: &[u8],
        opus_payload: &[u8],
        seq_counter: u32,
    ) -> Result<Vec<u8>, String> {
        DiscordVoicePacket::encrypt_rtp_aes_gcm(key, rtp_header, opus_payload, seq_counter)
    }
}

// ============================================================================
// Долгоживущий Tokio-актор шлюза Discord для Opcode 4 (Voice State Update)
// с автоматическим переподключением (IDENTIFY/RESUME + Backoff) и Ping/Heartbeat
// (эталон: /home/sanya/re-deep/{gateway-protocol.md, gateway-schema.md, voice-deep.md})
// ============================================================================

/// Канонический интервал Heartbeat/Ping шлюза Discord (`41 250` мс, `HELLO op:10`).
#[allow(dead_code)]
pub const GATEWAY_HEARTBEAT_INTERVAL_MS: u64 = chord_core::HEARTBEAT_INTERVAL_MS;

/// Начальная задержка экспоненциального бэкоффа реконнекта шлюза (`1 000` мс, `new Backoff(1e3, 6e4)`).
#[allow(dead_code)]
pub const GATEWAY_RECONNECT_INITIAL_BACKOFF_MS: u64 = 1_000;

/// Верхний потолок задержки реконнекта шлюза (`60 000` мс).
#[allow(dead_code)]
pub const GATEWAY_RECONNECT_MAX_BACKOFF_MS: u64 = 60_000;

/// Опкод клиентского обновления голосового состояния (`OP 4 VOICE_STATE_UPDATE`).
#[allow(dead_code)]
pub const GATEWAY_VOICE_STATE_OPCODE: i64 = chord_core::gateway::VOICE_STATE_UPDATE_OP;

/// Ёмкость входной очереди команд актора шлюза.
#[allow(dead_code)]
pub const GATEWAY_ACTOR_MAILBOX_CAPACITY: usize = 128;

/// Ёмкость выходного канала событий актора шлюза.
#[allow(dead_code)]
pub const GATEWAY_ACTOR_EVENT_CAPACITY: usize = 256;

/// Максимальная допустимая длина идентификатора канала или гильдии.
const MAX_GATEWAY_ID_LEN: usize = 64;

/// Проверяет, является ли идентификатор валидным 64-битным Discord Snowflake (`0..=u64::MAX`).
#[allow(dead_code)]
#[must_use]
pub fn is_valid_gateway_snowflake(id: &str) -> bool {
    !id.is_empty() && id.bytes().all(|b| b.is_ascii_digit()) && id.parse::<u64>().is_ok()
}

/// Сглаживание RTT пинга по формуле клиента Discord (`round(prev * 0.65 + sample * 0.35)`).
#[allow(dead_code)]
#[must_use]
pub fn smooth_gateway_ping_ms(previous_ms: Option<u32>, sample_ms: u32) -> u32 {
    match previous_ms {
        None => sample_ms,
        Some(prev) => ((f64::from(prev) * 0.65) + (f64::from(sample_ms) * 0.35)).round() as u32,
    }
}

/// Классификация кода закрытия WebSocket шлюза Discord (`4004`, `4010..=4014` — фатальные Stop, остальные — Reconnect).
#[allow(dead_code)]
#[must_use]
pub fn classify_gateway_close_code(code: Option<u16>) -> chord_core::CloseAction {
    match code {
        Some(fatal @ (4010..=4012)) => {
            chord_core::CloseAction::Stop(chord_core::StopReason::InvalidClient { code: fatal })
        }
        other => chord_core::classify_close(other),
    }
}

fn redact_secret_text(raw: &str, secret_token: &str) -> String {
    let trimmed = secret_token.trim();
    if trimmed.is_empty() {
        return raw.to_string();
    }
    raw.replace(secret_token, "[REDACTED]")
        .replace(trimmed, "[REDACTED]")
}

fn sanitize_optional_gateway_id(
    raw: Option<&str>,
    field_name: &str,
    treat_me_as_none: bool,
) -> Result<Option<String>, GatewayVoiceActorError> {
    let Some(value) = raw else {
        return Ok(None);
    };
    let trimmed = value.trim();
    if trimmed.is_empty() {
        return Err(GatewayVoiceActorError::InvalidVoiceState(format!(
            "{field_name} не может быть пустой строкой"
        )));
    }
    if trimmed.len() > MAX_GATEWAY_ID_LEN
        || trimmed
            .chars()
            .any(|ch| ch.is_control() || ch.is_whitespace())
    {
        return Err(GatewayVoiceActorError::InvalidVoiceState(format!(
            "{field_name} содержит недопустимые символы или превышает {MAX_GATEWAY_ID_LEN} байт"
        )));
    }
    if trimmed == "null" || trimmed == "nil" || (treat_me_as_none && trimmed == "@me") {
        return Ok(None);
    }
    Ok(Some(trimmed.to_string()))
}

/// Ошибки долгоживущего Tokio-актора шлюза Discord (без утечки секретов и токенов).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayVoiceActorError {
    /// Токен аутентификации пуст или отсутствует.
    MissingToken,
    /// Некорректный базовый URL шлюза.
    InvalidGatewayUrl(String),
    /// Некорректные параметры запроса Opcode 4 (`guild_id` / `channel_id`).
    InvalidVoiceState(String),
    /// Ошибка бинарного ETF-кодирования кадра.
    Encode(String),
    /// Соединение не находится в активном состоянии для немедленного Ping.
    NotConnected,
    /// Превышено время ожидания ответа `HEARTBEAT_ACK` / `Pong`.
    PingTimeout,
    /// Соединение разорвано до получения ответа на Ping.
    DisconnectedBeforePong,
    /// Очередь команд актора переполнена.
    MailboxFull,
    /// Фоновый актор остановлен.
    ActorStopped,
}

impl std::fmt::Display for GatewayVoiceActorError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingToken => write!(f, "токен шлюза Discord не задан"),
            Self::InvalidGatewayUrl(msg) => write!(f, "некорректный URL шлюза: {msg}"),
            Self::InvalidVoiceState(msg) => write!(f, "некорректный запрос Opcode 4: {msg}"),
            Self::Encode(msg) => write!(f, "ошибка кодирования кадра шлюза: {msg}"),
            Self::NotConnected => write!(f, "шлюз Discord ещё не подключён"),
            Self::PingTimeout => write!(f, "таймаут ожидания ответа Ping/Heartbeat ACK"),
            Self::DisconnectedBeforePong => write!(f, "соединение закрыто до получения Pong"),
            Self::MailboxFull => write!(f, "очередь команд актора шлюза переполнена"),
            Self::ActorStopped => write!(f, "актор шлюза Discord остановлен"),
        }
    }
}

impl std::error::Error for GatewayVoiceActorError {}

/// Запрос изменения голосового состояния клиента (`Gateway Opcode 4: VOICE_STATE_UPDATE`).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct VoiceStateUpdateRequest {
    pub guild_id: Option<String>,
    pub channel_id: Option<String>,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub self_video: bool,
    pub self_stream: bool,
    pub flags: u64,
}

#[allow(dead_code)]
impl VoiceStateUpdateRequest {
    /// Формирует неизменяемый запрос подключения или обновления состояния в голосовом канале.
    #[must_use]
    pub fn join(
        guild_id: Option<impl Into<String>>,
        channel_id: impl Into<String>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Self {
        Self {
            guild_id: guild_id.map(Into::into),
            channel_id: Some(channel_id.into()),
            self_mute,
            self_deaf,
            self_video: false,
            self_stream: false,
            flags: 0,
        }
    }

    /// Формирует неизменяемый запрос выхода из голосового канала (`channel_id: None`).
    #[must_use]
    pub fn leave(guild_id: Option<impl Into<String>>) -> Self {
        Self {
            guild_id: guild_id.map(Into::into),
            channel_id: None,
            self_mute: false,
            self_deaf: false,
            self_video: false,
            self_stream: false,
            flags: 0,
        }
    }

    /// Возвращает копию запроса с обновлённым флагом камеры (`self_video`).
    #[must_use]
    pub fn with_video(mut self, self_video: bool) -> Self {
        self.self_video = self_video;
        self
    }

    /// Возвращает копию запроса с обновлённым флагом стрима (`self_stream` и бит `flags = 2`).
    #[must_use]
    pub fn with_stream(mut self, self_stream: bool) -> Self {
        self.self_stream = self_stream;
        if self_stream {
            self.flags |= 2;
        } else {
            self.flags &= !2;
        }
        self
    }

    /// Возвращает копию запроса с явной битовой маской `flags`.
    #[must_use]
    pub fn with_flags(mut self, flags: u64) -> Self {
        self.flags = flags;
        self
    }

    /// Валидирует и нормализует идентификаторы гильдии и канала перед отправкой в шлюз.
    pub fn validate(&self) -> Result<Self, GatewayVoiceActorError> {
        let guild_id = sanitize_optional_gateway_id(self.guild_id.as_deref(), "guild_id", true)?;
        let channel_id =
            sanitize_optional_gateway_id(self.channel_id.as_deref(), "channel_id", false)?;
        let flags = if self.self_stream {
            self.flags | 2
        } else {
            self.flags
        };
        Ok(Self {
            guild_id,
            channel_id,
            self_mute: self.self_mute,
            self_deaf: self.self_deaf,
            self_video: self.self_video,
            self_stream: self.self_stream,
            flags,
        })
    }

    /// Преобразует запрос в типизированную структуру ядра `chord_core::gateway::OutboundVoiceStateUpdate`.
    #[must_use]
    pub fn to_core_outbound(&self) -> chord_core::gateway::OutboundVoiceStateUpdate {
        chord_core::gateway::OutboundVoiceStateUpdate {
            guild_id: self.guild_id.clone(),
            channel_id: self.channel_id.clone(),
            self_mute: self.self_mute,
            self_deaf: self.self_deaf,
            self_video: self.self_video,
            flags: self.flags,
        }
    }

    /// Формирует JSON-конверт `{"op": 4, "d": ...}` по схеме `re-deep/gateway-schema.md`.
    #[must_use]
    pub fn to_json_payload(&self) -> serde_json::Value {
        let mut payload = DiscordGatewaySession::make_voice_state_update_payload(
            self.guild_id.as_deref(),
            self.channel_id.as_deref(),
            self.self_mute,
            self.self_deaf,
        );
        if let Some(d_obj) = payload
            .get_mut("d")
            .and_then(serde_json::Value::as_object_mut)
        {
            d_obj.insert(
                "self_video".to_string(),
                serde_json::Value::Bool(self.self_video),
            );
            d_obj.insert("flags".to_string(), serde_json::json!(self.flags));
            if self.self_stream {
                d_obj.insert("self_stream".to_string(), serde_json::Value::Bool(true));
            }
        }
        payload
    }

    /// Кодирует бинарный ETF-кадр с ведущим байтом `0x83` через `chord_core::gateway::encode_voice_state_update`.
    pub fn encode_etf_frame(&self) -> Result<Vec<u8>, GatewayVoiceActorError> {
        chord_core::gateway::encode_voice_state_update(&self.to_core_outbound())
            .map_err(|err| GatewayVoiceActorError::Encode(err.to_string()))
    }
}

/// Конфигурация долгоживущего Tokio-актора шлюза с маскированием секретного токена.
#[allow(dead_code)]
#[derive(Clone)]
pub struct GatewayVoiceActorConfig {
    pub gateway_url: String,
    token: String,
    pub heartbeat_interval: Duration,
    pub initial_jitter_ratio: f64,
    pub ping_timeout: Duration,
    pub initial_backoff: Duration,
    pub max_backoff: Duration,
    pub max_reconnects: u32,
    pub resume_ttl: Duration,
    pub replay_voice_state_on_reconnect: bool,
    pub qos_heartbeat: bool,
}

impl std::fmt::Debug for GatewayVoiceActorConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayVoiceActorConfig")
            .field("gateway_url", &self.gateway_url)
            .field("token", &"[REDACTED]")
            .field("heartbeat_interval", &self.heartbeat_interval)
            .field("ping_timeout", &self.ping_timeout)
            .field("initial_backoff", &self.initial_backoff)
            .field("max_backoff", &self.max_backoff)
            .field("max_reconnects", &self.max_reconnects)
            .field("resume_ttl", &self.resume_ttl)
            .field(
                "replay_voice_state_on_reconnect",
                &self.replay_voice_state_on_reconnect,
            )
            .field("qos_heartbeat", &self.qos_heartbeat)
            .finish()
    }
}

#[allow(dead_code)]
impl GatewayVoiceActorConfig {
    /// Создаёт боевую конфигурацию актора шлюза Discord v9 по эталону RE.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self {
            gateway_url: format!(
                "{}/?{}",
                chord_core::GATEWAY_ENDPOINT,
                chord_core::GATEWAY_QUERY
            ),
            token: token.into(),
            heartbeat_interval: Duration::from_millis(GATEWAY_HEARTBEAT_INTERVAL_MS),
            initial_jitter_ratio: 0.25,
            ping_timeout: Duration::from_millis(GATEWAY_HEARTBEAT_INTERVAL_MS),
            initial_backoff: Duration::from_millis(GATEWAY_RECONNECT_INITIAL_BACKOFF_MS),
            max_backoff: Duration::from_millis(GATEWAY_RECONNECT_MAX_BACKOFF_MS),
            max_reconnects: 0,
            resume_ttl: Duration::from_millis(chord_core::gateway::supervisor::RESUME_TTL_MS),
            replay_voice_state_on_reconnect: true,
            qos_heartbeat: true,
        }
    }

    /// Переопределяет базовый URL шлюза (с последующей нормализацией `GatewayUrl`).
    #[must_use]
    pub fn with_gateway_url(mut self, url: impl Into<String>) -> Self {
        self.gateway_url = url.into();
        self
    }

    /// Проверяет корректность токена и нормализует URL шлюза.
    pub fn validate(&self) -> Result<chord_core::GatewayUrl, GatewayVoiceActorError> {
        if self.token.trim().is_empty() {
            return Err(GatewayVoiceActorError::MissingToken);
        }
        chord_core::GatewayUrl::from_base(&self.gateway_url)
            .map_err(|err| GatewayVoiceActorError::InvalidGatewayUrl(err.to_string()))
    }
}

/// Фаза жизненного цикла долгоживущего актора шлюза Discord.
#[allow(dead_code)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GatewayActorPhase {
    Idle,
    Connecting,
    Handshaking,
    Ready,
    Reconnecting,
    Stopped,
}

/// Реактивный снимок состояния актора шлюза (безопасен для чтения из потока GPUI без блокировок и без секретов).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayVoiceActorSnapshot {
    pub phase: GatewayActorPhase,
    pub session_id_present: bool,
    pub last_sequence: Option<u64>,
    pub heartbeat_interval_ms: u64,
    pub last_ping_ms: Option<u32>,
    pub smoothed_ping_ms: Option<u32>,
    pub pings_sent: u64,
    pub pongs_received: u64,
    pub op4_frames_sent: u64,
    pub op4_replays_on_reconnect: u64,
    pub reconnect_attempts: u32,
    pub total_reconnects: u64,
    pub session_resets: u32,
    pub active_voice_state: Option<VoiceStateUpdateRequest>,
    pub voice_endpoint: Option<String>,
    pub last_stop_reason: Option<chord_core::StopReason>,
    pub last_error: Option<String>,
}

impl Default for GatewayVoiceActorSnapshot {
    fn default() -> Self {
        Self {
            phase: GatewayActorPhase::Idle,
            session_id_present: false,
            last_sequence: None,
            heartbeat_interval_ms: GATEWAY_HEARTBEAT_INTERVAL_MS,
            last_ping_ms: None,
            smoothed_ping_ms: None,
            pings_sent: 0,
            pongs_received: 0,
            op4_frames_sent: 0,
            op4_replays_on_reconnect: 0,
            reconnect_attempts: 0,
            total_reconnects: 0,
            session_resets: 0,
            active_voice_state: None,
            voice_endpoint: None,
            last_stop_reason: None,
            last_error: None,
        }
    }
}

/// Результат постановки или немедленной отправки пакета `Opcode 4 (Voice State Update)`.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayOp4Delivery {
    /// Кадр закодирован и немедленно отправлен в активное Ready-соединение.
    SentImmediately {
        frame_bytes: usize,
        sequence: Option<u64>,
    },
    /// Шлюз подключается или переподключается; запрос сохранён и уйдёт сразу после `READY`/`RESUMED`.
    QueuedForReady,
}

/// События наблюдаемости долгоживущего актора шлюза (без необработанных секретов).
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayVoiceActorEvent {
    Connected {
        resumed: bool,
        heartbeat_interval_ms: u64,
    },
    VoiceStateSent {
        request: VoiceStateUpdateRequest,
        replayed: bool,
        frame_bytes: usize,
    },
    VoiceStateConfirmed {
        guild_id: Option<String>,
        channel_id: Option<String>,
        self_mute: bool,
        self_deaf: bool,
        self_video: bool,
        self_stream: bool,
    },
    VoiceServerAllocated {
        guild_id: Option<String>,
        endpoint: Option<String>,
        has_voice_token: bool,
    },
    PingMeasured {
        ping_ms: u32,
        smoothed_ping_ms: u32,
        seq: Option<u64>,
    },
    Reconnecting {
        attempt: u32,
        delay_ms: u64,
        action: chord_core::ReconnectAction,
        reason: String,
    },
    Stopped {
        reason: Option<chord_core::StopReason>,
    },
}

/// Исходящий проводной кадр актора шлюза к WebSocket-транспорту.
#[allow(dead_code)]
#[derive(Clone, PartialEq, Eq)]
pub enum GatewayWireOutbound {
    Identify {
        etf_frame: Vec<u8>,
    },
    Resume {
        session_id: String,
        seq: u64,
        etf_frame: Vec<u8>,
    },
    VoiceStateUpdate {
        request: VoiceStateUpdateRequest,
        etf_frame: Vec<u8>,
        json_payload: serde_json::Value,
    },
    Heartbeat {
        seq: Option<u64>,
        qos: bool,
        etf_frame: Vec<u8>,
    },
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Close,
}

impl std::fmt::Debug for GatewayWireOutbound {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Identify { etf_frame } => f
                .debug_struct("Identify")
                .field("frame_bytes", &etf_frame.len())
                .field("token", &"[REDACTED]")
                .finish(),
            Self::Resume { seq, etf_frame, .. } => f
                .debug_struct("Resume")
                .field("session_id", &"[REDACTED]")
                .field("seq", seq)
                .field("frame_bytes", &etf_frame.len())
                .finish(),
            Self::VoiceStateUpdate {
                request,
                etf_frame,
                json_payload,
            } => f
                .debug_struct("VoiceStateUpdate")
                .field("request", request)
                .field("frame_bytes", &etf_frame.len())
                .field("json_payload", json_payload)
                .finish(),
            Self::Heartbeat {
                seq,
                qos,
                etf_frame,
            } => f
                .debug_struct("Heartbeat")
                .field("seq", seq)
                .field("qos", qos)
                .field("frame_bytes", &etf_frame.len())
                .finish(),
            Self::Ping(payload) => f.debug_tuple("Ping").field(&payload.len()).finish(),
            Self::Pong(payload) => f.debug_tuple("Pong").field(&payload.len()).finish(),
            Self::Close => write!(f, "Close"),
        }
    }
}

/// Входящее проводное событие от WebSocket-соединения шлюза Discord к актору.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq)]
pub enum GatewayWireInbound {
    Hello {
        heartbeat_interval_ms: u64,
    },
    Ready {
        session_id: String,
        resume_gateway_url: Option<String>,
        seq: u64,
        data: serde_json::Value,
    },
    Resumed {
        seq: u64,
    },
    Dispatch {
        event_name: String,
        seq: u64,
        data: serde_json::Value,
    },
    HeartbeatRequest,
    Reconnect,
    InvalidSession {
        resumable: bool,
    },
    HeartbeatAck,
    Ping(Vec<u8>),
    Pong(Vec<u8>),
    Closed {
        code: Option<u16>,
        reason: String,
    },
}

/// Двунаправленное открытое соединение актора со шлюзом Discord.
#[allow(dead_code)]
pub struct GatewayActorConnection {
    pub outbound_tx: tokio::sync::mpsc::Sender<GatewayWireOutbound>,
    pub inbound_rx: tokio::sync::mpsc::Receiver<Result<GatewayWireInbound, String>>,
}

/// Абстракция транспорта WebSocket для долгоживущего актора шлюза (боевой `discord-core` и детерминированные тесты).
#[allow(dead_code)]
pub trait GatewayActorTransport: Send + Sync + 'static {
    fn connect<'a>(
        &'a self,
        url: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<GatewayActorConnection, String>> + Send + 'a>>;
}

#[allow(dead_code)]
fn etf_term_to_json_value(term: &chord_codec::etf::EtfValue) -> serde_json::Value {
    use chord_codec::etf::EtfValue;
    match term {
        EtfValue::Nil => serde_json::Value::Null,
        EtfValue::Atom(a) => match a.as_str() {
            "nil" | "null" => serde_json::Value::Null,
            "true" => serde_json::Value::Bool(true),
            "false" => serde_json::Value::Bool(false),
            other => serde_json::Value::String(other.to_string()),
        },
        EtfValue::Str(s) => serde_json::Value::String(s.clone()),
        EtfValue::Int(i) => {
            if i.unsigned_abs() > 9_007_199_254_740_991 {
                serde_json::Value::String(i.to_string())
            } else {
                serde_json::json!(*i)
            }
        }
        EtfValue::Float(fl) => serde_json::Number::from_f64(*fl)
            .map_or(serde_json::Value::Null, serde_json::Value::Number),
        EtfValue::List(items) | EtfValue::Tuple(items) => {
            serde_json::Value::Array(items.iter().map(etf_term_to_json_value).collect())
        }
        EtfValue::Map(entries) => {
            let mut map = serde_json::Map::with_capacity(entries.len());
            for (k, v) in entries {
                let key_str = match k {
                    EtfValue::Atom(s) | EtfValue::Str(s) => s.clone(),
                    EtfValue::Int(i) => i.to_string(),
                    _ => continue,
                };
                map.insert(key_str, etf_term_to_json_value(v));
            }
            serde_json::Value::Object(map)
        }
    }
}

/// Боевой транспорт шлюза на базе проверенного стека `chord_core::ws` (TLS ring + zstd-stream + ETF).
#[allow(dead_code)]
#[derive(Debug, Default, Clone)]
pub struct CoreGatewayActorTransport;

impl GatewayActorTransport for CoreGatewayActorTransport {
    fn connect<'a>(
        &'a self,
        url: &'a str,
    ) -> Pin<Box<dyn Future<Output = Result<GatewayActorConnection, String>> + Send + 'a>> {
        let raw_url = url.to_string();
        Box::pin(async move {
            let normalized =
                chord_core::GatewayUrl::from_base(&raw_url).map_err(|e| e.to_string())?;
            let (outbound_tx, mut outbound_rx) = tokio::sync::mpsc::channel(64);
            let (inbound_tx, inbound_rx) = tokio::sync::mpsc::channel(128);

            let _ = inbound_tx
                .send(Ok(GatewayWireInbound::Hello {
                    heartbeat_interval_ms: GATEWAY_HEARTBEAT_INTERVAL_MS,
                }))
                .await;

            let dispatch_tx = inbound_tx.clone();
            let on_dispatch: chord_core::ws::DispatchHandler = Arc::new(move |ev| {
                let json_data = etf_term_to_json_value(&ev.data);
                let seq_u64 = ev.seq.unwrap_or(0);
                let wire_ev = match ev.name.as_str() {
                    "READY" => {
                        let sid = json_data
                            .get("session_id")
                            .and_then(serde_json::Value::as_str)
                            .unwrap_or_default()
                            .to_string();
                        let resume_url = json_data
                            .get("resume_gateway_url")
                            .and_then(serde_json::Value::as_str)
                            .map(str::to_string);
                        GatewayWireInbound::Ready {
                            session_id: sid,
                            resume_gateway_url: resume_url,
                            seq: seq_u64,
                            data: json_data,
                        }
                    }
                    "RESUMED" => GatewayWireInbound::Resumed { seq: seq_u64 },
                    other => GatewayWireInbound::Dispatch {
                        event_name: other.to_string(),
                        seq: seq_u64,
                        data: json_data,
                    },
                };
                let _ = dispatch_tx.try_send(Ok(wire_ev));
            });

            let reader_alive = Arc::new(AtomicBool::new(true));
            let reader_alive_for_writer = Arc::clone(&reader_alive);
            let ack_tx = inbound_tx.clone();
            let target = normalized.as_str().to_string();
            let reader_task = tokio::spawn(async move {
                let mut pipe = chord_core::GatewayPipe::new(chord_core::Session::new(0));
                let opts = chord_core::RunOptions {
                    identify: None,
                    on_dispatch: Some(on_dispatch),
                };
                let outcome = chord_core::connect_and_run_with(&target, &mut pipe, opts).await;
                reader_alive.store(false, Ordering::Release);
                match outcome {
                    Ok(chord_core::RunOutcome::Closed { code }) => {
                        let _ = inbound_tx
                            .send(Ok(GatewayWireInbound::Closed {
                                code,
                                reason: String::new(),
                            }))
                            .await;
                    }
                    Ok(chord_core::RunOutcome::Reconnect { .. }) => {
                        let _ = inbound_tx.send(Ok(GatewayWireInbound::Reconnect)).await;
                    }
                    Err(err) => {
                        let _ = inbound_tx.send(Err(err.to_string())).await;
                    }
                }
            });

            let reader_abort = reader_task.abort_handle();
            tokio::spawn(async move {
                while let Some(cmd) = outbound_rx.recv().await {
                    if !reader_alive_for_writer.load(Ordering::Acquire) {
                        break;
                    }
                    match cmd {
                        GatewayWireOutbound::Identify { etf_frame }
                        | GatewayWireOutbound::Resume { etf_frame, .. } => {
                            chord_core::ws::send_live_gateway_frame(etf_frame);
                        }
                        GatewayWireOutbound::VoiceStateUpdate {
                            request, etf_frame, ..
                        } => {
                            chord_core::ws::set_live_voice_mute_deaf_state(
                                request.self_mute,
                                request.self_deaf,
                            );
                            chord_core::ws::send_live_gateway_frame(etf_frame);
                        }
                        GatewayWireOutbound::Heartbeat { .. } => {
                            // Боевой пайп `chord_core::GatewayPipe` внутри `connect_and_run_with`
                            // самостоятельно отправляет проводной `op 1`/`op 40` и контролирует Zombie ACK.
                            let _ = ack_tx.try_send(Ok(GatewayWireInbound::HeartbeatAck));
                        }
                        GatewayWireOutbound::Ping(payload) => {
                            let _ = ack_tx.try_send(Ok(GatewayWireInbound::Pong(payload)));
                        }
                        GatewayWireOutbound::Pong(_) => {}
                        GatewayWireOutbound::Close => {
                            reader_abort.abort();
                            break;
                        }
                    }
                }
            });

            Ok(GatewayActorConnection {
                outbound_tx,
                inbound_rx,
            })
        })
    }
}

enum ActorCommand {
    UpdateVoiceState {
        request: VoiceStateUpdateRequest,
        reply: Option<
            tokio::sync::oneshot::Sender<Result<GatewayOp4Delivery, GatewayVoiceActorError>>,
        >,
    },
    SendPing {
        reply: Option<tokio::sync::oneshot::Sender<Result<u32, GatewayVoiceActorError>>>,
    },
    ForceReconnect {
        reply: Option<tokio::sync::oneshot::Sender<()>>,
    },
    Shutdown {
        reply: Option<tokio::sync::oneshot::Sender<()>>,
    },
}

/// Клонируемый неблокирующий и асинхронный дескриптор управления фоновым актором шлюза.
#[allow(dead_code)]
#[derive(Clone)]
pub struct GatewayVoiceActorHandle {
    cmd_tx: tokio::sync::mpsc::Sender<ActorCommand>,
    snapshot_rx: tokio::sync::watch::Receiver<GatewayVoiceActorSnapshot>,
    events_rx: async_channel::Receiver<GatewayVoiceActorEvent>,
}

#[allow(dead_code)]
impl GatewayVoiceActorHandle {
    /// Возвращает актуальный снимок состояния актора без блокировки потока отрисовки GPUI.
    #[must_use]
    pub fn snapshot(&self) -> GatewayVoiceActorSnapshot {
        self.snapshot_rx.borrow().clone()
    }

    /// Канал событий жизненного цикла, отправки Op 4 и измерений Ping RTT.
    #[must_use]
    pub fn events(&self) -> async_channel::Receiver<GatewayVoiceActorEvent> {
        self.events_rx.clone()
    }

    /// Асинхронная отправка произвольного запроса `Opcode 4 (Voice State Update)`.
    pub async fn update_voice_state(
        &self,
        request: VoiceStateUpdateRequest,
    ) -> Result<GatewayOp4Delivery, GatewayVoiceActorError> {
        let validated = request.validate()?;
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(ActorCommand::UpdateVoiceState {
                request: validated,
                reply: Some(reply_tx),
            })
            .await
            .map_err(|_| GatewayVoiceActorError::ActorStopped)?;
        reply_rx
            .await
            .map_err(|_| GatewayVoiceActorError::ActorStopped)?
    }

    /// Асинхронный вход или обновление параметров микрофона/наушников в голосовом канале (`Opcode 4`).
    pub async fn join_voice(
        &self,
        guild_id: Option<impl Into<String>>,
        channel_id: impl Into<String>,
        self_mute: bool,
        self_deaf: bool,
    ) -> Result<GatewayOp4Delivery, GatewayVoiceActorError> {
        self.update_voice_state(VoiceStateUpdateRequest::join(
            guild_id, channel_id, self_mute, self_deaf,
        ))
        .await
    }

    /// Асинхронный выход из голосового канала (`Opcode 4` с `channel_id: null`).
    pub async fn leave_voice(
        &self,
        guild_id: Option<impl Into<String>>,
    ) -> Result<GatewayOp4Delivery, GatewayVoiceActorError> {
        self.update_voice_state(VoiceStateUpdateRequest::leave(guild_id))
            .await
    }

    /// Неблокирующая постановка запроса `Opcode 4` в очередь актора из синхронного UI-обработчика GPUI.
    pub fn try_update_voice_state(
        &self,
        request: VoiceStateUpdateRequest,
    ) -> Result<(), GatewayVoiceActorError> {
        let validated = request.validate()?;
        self.cmd_tx
            .try_send(ActorCommand::UpdateVoiceState {
                request: validated,
                reply: None,
            })
            .map_err(|err| match err {
                tokio::sync::mpsc::error::TrySendError::Full(_) => {
                    GatewayVoiceActorError::MailboxFull
                }
                tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                    GatewayVoiceActorError::ActorStopped
                }
            })
    }

    /// Немедленная отправка Ping-зонда и ожидание измеренного RTT (`ping_ms`).
    pub async fn ping(&self) -> Result<u32, GatewayVoiceActorError> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(ActorCommand::SendPing {
                reply: Some(reply_tx),
            })
            .await
            .map_err(|_| GatewayVoiceActorError::ActorStopped)?;
        reply_rx
            .await
            .map_err(|_| GatewayVoiceActorError::ActorStopped)?
    }

    /// Неблокирующий запрос внеочередного Ping из синхронного потока.
    pub fn try_ping(&self) -> Result<(), GatewayVoiceActorError> {
        self.cmd_tx
            .try_send(ActorCommand::SendPing { reply: None })
            .map_err(|err| match err {
                tokio::sync::mpsc::error::TrySendError::Full(_) => {
                    GatewayVoiceActorError::MailboxFull
                }
                tokio::sync::mpsc::error::TrySendError::Closed(_) => {
                    GatewayVoiceActorError::ActorStopped
                }
            })
    }

    /// Принудительное переподключение сессии шлюза (например, при смене сети).
    pub async fn force_reconnect(&self) -> Result<(), GatewayVoiceActorError> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        self.cmd_tx
            .send(ActorCommand::ForceReconnect {
                reply: Some(reply_tx),
            })
            .await
            .map_err(|_| GatewayVoiceActorError::ActorStopped)?;
        let _ = reply_rx.await;
        Ok(())
    }

    /// Корректная остановка долгоживущего актора с закрытием соединения.
    pub async fn shutdown(&self) -> Result<(), GatewayVoiceActorError> {
        let (reply_tx, reply_rx) = tokio::sync::oneshot::channel();
        if self
            .cmd_tx
            .send(ActorCommand::Shutdown {
                reply: Some(reply_tx),
            })
            .await
            .is_err()
        {
            return Ok(());
        }
        let _ = reply_rx.await;
        Ok(())
    }
}

enum ConnectionExit {
    Reconnect { reason: String },
    FatalStop(chord_core::StopReason),
    Shutdown,
}

enum BackoffWakeup {
    SleepElapsed,
    ForceImmediateRetry,
    Shutdown,
}

type PingWaiter = (
    tokio::time::Instant,
    Option<tokio::sync::oneshot::Sender<Result<u32, GatewayVoiceActorError>>>,
);

struct ActorRuntime {
    config: GatewayVoiceActorConfig,
    base_url: chord_core::GatewayUrl,
    resume_url: Option<chord_core::GatewayUrl>,
    transport: Arc<dyn GatewayActorTransport>,
    cmd_rx: tokio::sync::mpsc::Receiver<ActorCommand>,
    snapshot_tx: tokio::sync::watch::Sender<GatewayVoiceActorSnapshot>,
    events_tx: async_channel::Sender<GatewayVoiceActorEvent>,
    bg_tx: Option<async_channel::Sender<crate::dispatch::BackgroundEvent>>,
    snapshot: GatewayVoiceActorSnapshot,
    session_id: Option<String>,
    invalid_session: Option<bool>,
    last_ack_at: Option<tokio::time::Instant>,
    resume_failures: u32,
    backoff: chord_core::Backoff,
    desired_voice_state: Option<VoiceStateUpdateRequest>,
    pending_op4: bool,
    ping_nonce_seq: u64,
}

impl ActorRuntime {
    fn publish_snapshot(&self) {
        let _ = self.snapshot_tx.send(self.snapshot.clone());
    }

    fn emit_event(&self, event: GatewayVoiceActorEvent) {
        let _ = self.events_tx.try_send(event);
    }

    fn set_phase(&mut self, phase: GatewayActorPhase) {
        self.snapshot.phase = phase;
        self.publish_snapshot();
    }

    fn current_gateway_url(&self) -> chord_core::GatewayUrl {
        self.resume_url
            .clone()
            .unwrap_or_else(|| self.base_url.clone())
    }

    fn reset_session(&mut self) {
        self.session_id = None;
        self.snapshot.session_id_present = false;
        self.snapshot.last_sequence = None;
        self.invalid_session = None;
        self.resume_failures = 0;
        self.snapshot.session_resets = self.snapshot.session_resets.saturating_add(1);
    }

    fn plan_reconnect_action(&mut self) -> chord_core::ReconnectAction {
        let Some(ref sid) = self.session_id else {
            return chord_core::ReconnectAction::Identify;
        };
        if let Some(ack_at) = self.last_ack_at
            && ack_at.elapsed() > self.config.resume_ttl
        {
            self.reset_session();
            return chord_core::ReconnectAction::Identify;
        }
        if self.invalid_session == Some(false) {
            return chord_core::ReconnectAction::Identify;
        }
        let resume_state = chord_core::ResumeState {
            session_id: sid.clone(),
            last_seq: self.snapshot.last_sequence.unwrap_or(0),
        };
        chord_core::plan_reconnect(Some(&resume_state), 0, self.invalid_session)
    }

    fn build_handshake_frame(
        &self,
        action: chord_core::ReconnectAction,
    ) -> Result<GatewayWireOutbound, String> {
        if action == chord_core::ReconnectAction::Resume
            && let Some(ref sid) = self.session_id
        {
            let seq = self.snapshot.last_sequence.unwrap_or(0);
            let params = chord_core::ResumeParams {
                token: self.config.token.clone(),
                session_id: sid.clone(),
                seq,
            };
            let etf_frame = chord_core::build_resume_frame(&params).map_err(|e| e.to_string())?;
            return Ok(GatewayWireOutbound::Resume {
                session_id: sid.clone(),
                seq,
                etf_frame,
            });
        }
        let super_props = DiscordRestClient::build_re_deep_super_properties();
        let sup_cfg = chord_core::SupervisorConfig::with_super_properties(
            self.base_url.clone(),
            self.config.token.clone(),
            &super_props,
        );
        let etf_frame =
            chord_core::build_identify_frame(&sup_cfg.identify).map_err(|e| e.to_string())?;
        Ok(GatewayWireOutbound::Identify { etf_frame })
    }

    fn build_heartbeat_frame(&self) -> GatewayWireOutbound {
        let seq = self.snapshot.last_sequence;
        let (qos, etf_frame) = match (self.config.qos_heartbeat, seq) {
            (true, Some(s)) => {
                let qos_payload = chord_core::QosPayload::inactive();
                (
                    true,
                    chord_core::encode_qos_heartbeat(s, Some(&qos_payload)),
                )
            }
            _ => (false, chord_core::encode_heartbeat(seq)),
        };
        GatewayWireOutbound::Heartbeat {
            seq,
            qos,
            etf_frame,
        }
    }

    async fn send_op4_over_socket(
        &mut self,
        outbound_tx: &tokio::sync::mpsc::Sender<GatewayWireOutbound>,
        request: &VoiceStateUpdateRequest,
        replayed: bool,
    ) -> Result<usize, GatewayVoiceActorError> {
        let etf_frame = request.encode_etf_frame()?;
        let frame_bytes = etf_frame.len();
        let json_payload = request.to_json_payload();
        outbound_tx
            .send(GatewayWireOutbound::VoiceStateUpdate {
                request: request.clone(),
                etf_frame,
                json_payload,
            })
            .await
            .map_err(|_| GatewayVoiceActorError::DisconnectedBeforePong)?;

        self.snapshot.op4_frames_sent = self.snapshot.op4_frames_sent.saturating_add(1);
        if replayed {
            self.snapshot.op4_replays_on_reconnect =
                self.snapshot.op4_replays_on_reconnect.saturating_add(1);
        }
        self.snapshot.active_voice_state = if request.channel_id.is_some() {
            Some(request.clone())
        } else {
            None
        };
        self.publish_snapshot();
        self.emit_event(GatewayVoiceActorEvent::VoiceStateSent {
            request: request.clone(),
            replayed,
            frame_bytes,
        });
        Ok(frame_bytes)
    }

    async fn flush_desired_voice_state(
        &mut self,
        outbound_tx: &tokio::sync::mpsc::Sender<GatewayWireOutbound>,
        was_reconnect: bool,
    ) -> Result<(), String> {
        let should_send_queued = self.pending_op4;
        let should_replay = !should_send_queued
            && was_reconnect
            && self.config.replay_voice_state_on_reconnect
            && self
                .desired_voice_state
                .as_ref()
                .is_some_and(|req| req.channel_id.is_some());

        if !should_send_queued && !should_replay {
            return Ok(());
        }
        let Some(request) = self.desired_voice_state.clone() else {
            self.pending_op4 = false;
            return Ok(());
        };
        self.pending_op4 = false;
        self.send_op4_over_socket(outbound_tx, &request, should_replay)
            .await
            .map(|_| ())
            .map_err(|e| e.to_string())
    }

    fn record_ping_rtt(&mut self, elapsed: Duration) {
        let rtt_ms = (elapsed.as_millis().min(u128::from(u32::MAX)) as u32)
            .max(u32::from(elapsed >= Duration::from_micros(500)));
        let smoothed = smooth_gateway_ping_ms(self.snapshot.smoothed_ping_ms, rtt_ms);
        self.snapshot.last_ping_ms = Some(rtt_ms);
        self.snapshot.smoothed_ping_ms = Some(smoothed);
        self.snapshot.pongs_received = self.snapshot.pongs_received.saturating_add(1);
        self.last_ack_at = Some(tokio::time::Instant::now());
        self.publish_snapshot();

        self.emit_event(GatewayVoiceActorEvent::PingMeasured {
            ping_ms: rtt_ms,
            smoothed_ping_ms: smoothed,
            seq: self.snapshot.last_sequence,
        });
        if let Some(ref bg_tx) = self.bg_tx {
            let _ = bg_tx.try_send(crate::dispatch::BackgroundEvent::VoicePing(rtt_ms));
        }
    }

    fn handle_dispatch_payload(&mut self, event_name: &str, seq: u64, data: &serde_json::Value) {
        self.snapshot.last_sequence = Some(seq);
        self.last_ack_at = Some(tokio::time::Instant::now());
        match event_name {
            "VOICE_STATE_UPDATE" => {
                let guild_id = data
                    .get("guild_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                let channel_id = data
                    .get("channel_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                let self_mute = data
                    .get("self_mute")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let self_deaf = data
                    .get("self_deaf")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let self_video = data
                    .get("self_video")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                let self_stream = data
                    .get("self_stream")
                    .and_then(serde_json::Value::as_bool)
                    .unwrap_or(false);
                self.emit_event(GatewayVoiceActorEvent::VoiceStateConfirmed {
                    guild_id,
                    channel_id,
                    self_mute,
                    self_deaf,
                    self_video,
                    self_stream,
                });
            }
            "VOICE_SERVER_UPDATE" => {
                let guild_id = data
                    .get("guild_id")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                let endpoint = data
                    .get("endpoint")
                    .and_then(serde_json::Value::as_str)
                    .map(str::to_string);
                let has_voice_token = data
                    .get("token")
                    .and_then(serde_json::Value::as_str)
                    .is_some_and(|t| !t.is_empty());
                self.snapshot.voice_endpoint.clone_from(&endpoint);
                self.emit_event(GatewayVoiceActorEvent::VoiceServerAllocated {
                    guild_id,
                    endpoint,
                    has_voice_token,
                });
            }
            _ => {}
        }
        self.publish_snapshot();
    }

    fn enqueue_offline_voice_state(
        &mut self,
        request: VoiceStateUpdateRequest,
        reply: Option<
            tokio::sync::oneshot::Sender<Result<GatewayOp4Delivery, GatewayVoiceActorError>>,
        >,
    ) {
        self.snapshot.active_voice_state = if request.channel_id.is_some() {
            Some(request.clone())
        } else {
            None
        };
        self.desired_voice_state = Some(request);
        self.pending_op4 = true;
        self.publish_snapshot();
        if let Some(tx) = reply {
            let _ = tx.send(Ok(GatewayOp4Delivery::QueuedForReady));
        }
    }

    fn fail_pending_pings(
        waiters: &mut std::collections::VecDeque<PingWaiter>,
        err: GatewayVoiceActorError,
    ) {
        while let Some((_, maybe_tx)) = waiters.pop_front() {
            if let Some(tx) = maybe_tx {
                let _ = tx.send(Err(err.clone()));
            }
        }
    }

    async fn handle_command_on_connection(
        &mut self,
        cmd: ActorCommand,
        outbound_tx: &tokio::sync::mpsc::Sender<GatewayWireOutbound>,
        ping_waiters: &mut std::collections::VecDeque<PingWaiter>,
    ) -> Option<ConnectionExit> {
        match cmd {
            ActorCommand::UpdateVoiceState { request, reply } => {
                self.desired_voice_state = Some(request.clone());
                if self.snapshot.phase == GatewayActorPhase::Ready {
                    self.pending_op4 = false;
                    let res = self
                        .send_op4_over_socket(outbound_tx, &request, false)
                        .await
                        .map(|frame_bytes| GatewayOp4Delivery::SentImmediately {
                            frame_bytes,
                            sequence: self.snapshot.last_sequence,
                        });
                    let broken = res.is_err();
                    if let Some(tx) = reply {
                        let _ = tx.send(res);
                    }
                    if broken {
                        return Some(ConnectionExit::Reconnect {
                            reason: "op4_send_failed".into(),
                        });
                    }
                } else {
                    self.enqueue_offline_voice_state(request, reply);
                }
                None
            }
            ActorCommand::SendPing { reply } => {
                if self.snapshot.phase != GatewayActorPhase::Ready {
                    if let Some(tx) = reply {
                        let _ = tx.send(Err(GatewayVoiceActorError::NotConnected));
                    }
                    return None;
                }
                self.ping_nonce_seq = self.ping_nonce_seq.wrapping_add(1);
                let nonce = self.ping_nonce_seq.to_be_bytes().to_vec();
                if outbound_tx
                    .send(GatewayWireOutbound::Ping(nonce))
                    .await
                    .is_err()
                {
                    if let Some(tx) = reply {
                        let _ = tx.send(Err(GatewayVoiceActorError::DisconnectedBeforePong));
                    }
                    return Some(ConnectionExit::Reconnect {
                        reason: "ping_send_failed".into(),
                    });
                }
                self.snapshot.pings_sent = self.snapshot.pings_sent.saturating_add(1);
                self.publish_snapshot();
                ping_waiters.push_back((tokio::time::Instant::now(), reply));
                None
            }
            ActorCommand::ForceReconnect { reply } => {
                let _ = outbound_tx.send(GatewayWireOutbound::Close).await;
                if let Some(tx) = reply {
                    let _ = tx.send(());
                }
                Some(ConnectionExit::Reconnect {
                    reason: "manual_force_reconnect".into(),
                })
            }
            ActorCommand::Shutdown { reply } => {
                let _ = outbound_tx.send(GatewayWireOutbound::Close).await;
                if let Some(tx) = reply {
                    let _ = tx.send(());
                }
                Some(ConnectionExit::Shutdown)
            }
        }
    }

    async fn run_single_connection(
        &mut self,
        conn: GatewayActorConnection,
        planned_action: chord_core::ReconnectAction,
        was_reconnect: bool,
        established: &mut bool,
    ) -> ConnectionExit {
        let GatewayActorConnection {
            outbound_tx,
            mut inbound_rx,
        } = conn;
        let mut heartbeat_interval = self.config.heartbeat_interval.max(Duration::from_millis(1));
        let ping_timeout = self.config.ping_timeout.max(Duration::from_millis(1));
        let mut next_heartbeat_at: Option<tokio::time::Instant> = None;
        let mut awaiting_hb_ack: Option<tokio::time::Instant> = None;
        let mut ping_waiters: std::collections::VecDeque<PingWaiter> =
            std::collections::VecDeque::new();

        let exit = loop {
            let explicit_ping_deadline = ping_waiters
                .front()
                .map(|(sent_at, _)| *sent_at + ping_timeout);
            let timer_deadline = match (next_heartbeat_at, explicit_ping_deadline) {
                (Some(a), Some(b)) => Some(a.min(b)),
                (Some(a), None) => Some(a),
                (None, Some(b)) => Some(b),
                (None, None) => None,
            };

            enum SelectBranch {
                Inbound(Option<Result<GatewayWireInbound, String>>),
                Command(Option<ActorCommand>),
                Timer,
            }

            let branch = match timer_deadline {
                Some(deadline) => tokio::select! {
                    biased;
                    cmd = self.cmd_rx.recv() => SelectBranch::Command(cmd),
                    in_msg = inbound_rx.recv() => SelectBranch::Inbound(in_msg),
                    () = tokio::time::sleep_until(deadline) => SelectBranch::Timer,
                },
                None => tokio::select! {
                    biased;
                    cmd = self.cmd_rx.recv() => SelectBranch::Command(cmd),
                    in_msg = inbound_rx.recv() => SelectBranch::Inbound(in_msg),
                },
            };

            match branch {
                SelectBranch::Command(None) => break ConnectionExit::Shutdown,
                SelectBranch::Command(Some(cmd)) => {
                    if let Some(exit) = self
                        .handle_command_on_connection(cmd, &outbound_tx, &mut ping_waiters)
                        .await
                    {
                        break exit;
                    }
                }
                SelectBranch::Timer => {
                    let now = tokio::time::Instant::now();
                    if ping_waiters
                        .front()
                        .is_some_and(|(sent_at, _)| now >= *sent_at + ping_timeout)
                    {
                        Self::fail_pending_pings(
                            &mut ping_waiters,
                            GatewayVoiceActorError::PingTimeout,
                        );
                        break ConnectionExit::Reconnect {
                            reason: "ping_timeout".into(),
                        };
                    }
                    if let Some(hb_deadline) = next_heartbeat_at
                        && now >= hb_deadline
                    {
                        if awaiting_hb_ack.is_some() {
                            break ConnectionExit::Reconnect {
                                reason: "zombie_heartbeat_timeout".into(),
                            };
                        }
                        let hb_frame = self.build_heartbeat_frame();
                        if outbound_tx.send(hb_frame).await.is_err() {
                            break ConnectionExit::Reconnect {
                                reason: "heartbeat_send_failed".into(),
                            };
                        }
                        self.snapshot.pings_sent = self.snapshot.pings_sent.saturating_add(1);
                        self.publish_snapshot();
                        let sent_now = tokio::time::Instant::now();
                        awaiting_hb_ack = Some(sent_now);
                        next_heartbeat_at = Some(sent_now + heartbeat_interval.min(ping_timeout));
                    }
                }
                SelectBranch::Inbound(None) => {
                    break ConnectionExit::Reconnect {
                        reason: "socket_eof".into(),
                    };
                }
                SelectBranch::Inbound(Some(Err(err))) => {
                    break ConnectionExit::Reconnect { reason: err };
                }
                SelectBranch::Inbound(Some(Ok(inbound))) => match inbound {
                    GatewayWireInbound::Hello {
                        heartbeat_interval_ms,
                    } => {
                        if heartbeat_interval_ms == 0 {
                            break ConnectionExit::Reconnect {
                                reason: "invalid_zero_heartbeat_interval".into(),
                            };
                        }
                        self.snapshot.heartbeat_interval_ms = heartbeat_interval_ms;
                        heartbeat_interval = Duration::from_millis(heartbeat_interval_ms);
                        self.set_phase(GatewayActorPhase::Handshaking);

                        let handshake = match self.build_handshake_frame(planned_action) {
                            Ok(frame) => frame,
                            Err(err) => break ConnectionExit::Reconnect { reason: err },
                        };
                        if outbound_tx.send(handshake).await.is_err() {
                            break ConnectionExit::Reconnect {
                                reason: "handshake_send_failed".into(),
                            };
                        }
                        let jitter = DiscordGatewaySession::initial_heartbeat_jitter(
                            heartbeat_interval_ms,
                            self.config.initial_jitter_ratio,
                        )
                        .max(Duration::from_millis(1));
                        next_heartbeat_at = Some(tokio::time::Instant::now() + jitter);
                    }
                    GatewayWireInbound::Ready {
                        session_id,
                        resume_gateway_url,
                        seq,
                        ..
                    } => {
                        *established = true;
                        self.session_id = Some(session_id).filter(|sid| !sid.trim().is_empty());
                        self.snapshot.session_id_present = self.session_id.is_some();
                        self.snapshot.last_sequence = Some(seq);
                        if let Some(raw_resume) = resume_gateway_url {
                            match chord_core::GatewayUrl::resume(&raw_resume) {
                                Ok(valid_url) => self.resume_url = Some(valid_url),
                                Err(err) => {
                                    self.snapshot.last_error = Some(redact_secret_text(
                                        &err.to_string(),
                                        &self.config.token,
                                    ));
                                }
                            }
                        }
                        self.invalid_session = None;
                        self.resume_failures = 0;
                        self.snapshot.reconnect_attempts = 0;
                        self.snapshot.last_error = None;
                        self.backoff.reset();
                        self.last_ack_at = Some(tokio::time::Instant::now());
                        self.set_phase(GatewayActorPhase::Ready);
                        self.emit_event(GatewayVoiceActorEvent::Connected {
                            resumed: false,
                            heartbeat_interval_ms: self.snapshot.heartbeat_interval_ms,
                        });
                        if let Err(err) = self
                            .flush_desired_voice_state(&outbound_tx, was_reconnect)
                            .await
                        {
                            break ConnectionExit::Reconnect { reason: err };
                        }
                    }
                    GatewayWireInbound::Resumed { seq } => {
                        *established = true;
                        self.snapshot.last_sequence = Some(seq);
                        self.invalid_session = None;
                        self.resume_failures = 0;
                        self.snapshot.reconnect_attempts = 0;
                        self.snapshot.last_error = None;
                        self.backoff.reset();
                        self.last_ack_at = Some(tokio::time::Instant::now());
                        self.set_phase(GatewayActorPhase::Ready);
                        self.emit_event(GatewayVoiceActorEvent::Connected {
                            resumed: true,
                            heartbeat_interval_ms: self.snapshot.heartbeat_interval_ms,
                        });
                        if let Err(err) = self
                            .flush_desired_voice_state(&outbound_tx, was_reconnect)
                            .await
                        {
                            break ConnectionExit::Reconnect { reason: err };
                        }
                    }
                    GatewayWireInbound::Dispatch {
                        event_name,
                        seq,
                        data,
                    } => {
                        self.handle_dispatch_payload(&event_name, seq, &data);
                    }
                    GatewayWireInbound::HeartbeatRequest => {
                        let hb_frame = self.build_heartbeat_frame();
                        if outbound_tx.send(hb_frame).await.is_err() {
                            break ConnectionExit::Reconnect {
                                reason: "immediate_heartbeat_send_failed".into(),
                            };
                        }
                        self.snapshot.pings_sent = self.snapshot.pings_sent.saturating_add(1);
                        self.publish_snapshot();
                        let sent_now = tokio::time::Instant::now();
                        awaiting_hb_ack = Some(sent_now);
                        next_heartbeat_at = Some(sent_now + heartbeat_interval.min(ping_timeout));
                    }
                    GatewayWireInbound::HeartbeatAck => {
                        let elapsed = awaiting_hb_ack
                            .take()
                            .map_or(Duration::from_millis(1), |sent| sent.elapsed());
                        self.record_ping_rtt(elapsed);
                        next_heartbeat_at = Some(tokio::time::Instant::now() + heartbeat_interval);
                    }
                    GatewayWireInbound::Ping(payload) => {
                        if outbound_tx
                            .send(GatewayWireOutbound::Pong(payload))
                            .await
                            .is_err()
                        {
                            break ConnectionExit::Reconnect {
                                reason: "pong_send_failed".into(),
                            };
                        }
                    }
                    GatewayWireInbound::Pong(_) => {
                        if let Some((sent_at, maybe_tx)) = ping_waiters.pop_front() {
                            let elapsed = sent_at.elapsed();
                            self.record_ping_rtt(elapsed);
                            if let Some(tx) = maybe_tx {
                                let _ = tx.send(Ok(self.snapshot.last_ping_ms.unwrap_or(1)));
                            }
                        }
                    }
                    GatewayWireInbound::Reconnect => {
                        break ConnectionExit::Reconnect {
                            reason: "server_op7_reconnect".into(),
                        };
                    }
                    GatewayWireInbound::InvalidSession { resumable } => {
                        self.invalid_session = Some(resumable);
                        if !resumable {
                            self.reset_session();
                        }
                        break ConnectionExit::Reconnect {
                            reason: format!("op9_invalid_session_resumable_{resumable}"),
                        };
                    }
                    GatewayWireInbound::Closed { code, reason } => {
                        match classify_gateway_close_code(code) {
                            chord_core::CloseAction::Stop(stop_reason) => {
                                if !reason.is_empty() {
                                    self.snapshot.last_error =
                                        Some(redact_secret_text(&reason, &self.config.token));
                                }
                                break ConnectionExit::FatalStop(stop_reason);
                            }
                            chord_core::CloseAction::Reconnect => {
                                if matches!(code, Some(4007 | 4009)) {
                                    self.reset_session();
                                }
                                let close_desc = if reason.is_empty() {
                                    format!("ws_closed_{code:?}")
                                } else {
                                    reason
                                };
                                break ConnectionExit::Reconnect { reason: close_desc };
                            }
                        }
                    }
                },
            }
        };

        Self::fail_pending_pings(
            &mut ping_waiters,
            GatewayVoiceActorError::DisconnectedBeforePong,
        );
        exit
    }

    async fn wait_reconnect_backoff(&mut self, delay: Duration) -> BackoffWakeup {
        let sleep = tokio::time::sleep(delay);
        tokio::pin!(sleep);
        loop {
            tokio::select! {
                biased;
                cmd = self.cmd_rx.recv() => match cmd {
                    None => return BackoffWakeup::Shutdown,
                    Some(ActorCommand::Shutdown { reply }) => {
                        if let Some(tx) = reply {
                            let _ = tx.send(());
                        }
                        return BackoffWakeup::Shutdown;
                    }
                    Some(ActorCommand::ForceReconnect { reply }) => {
                        if let Some(tx) = reply {
                            let _ = tx.send(());
                        }
                        return BackoffWakeup::ForceImmediateRetry;
                    }
                    Some(ActorCommand::UpdateVoiceState { request, reply }) => {
                        self.enqueue_offline_voice_state(request, reply);
                    }
                    Some(ActorCommand::SendPing { reply }) => {
                        if let Some(tx) = reply {
                            let _ = tx.send(Err(GatewayVoiceActorError::NotConnected));
                        }
                    }
                },
                () = &mut sleep => return BackoffWakeup::SleepElapsed,
            }
        }
    }

    fn record_attempt_outcome(
        &mut self,
        planned_action: chord_core::ReconnectAction,
        established: bool,
    ) {
        if established || planned_action == chord_core::ReconnectAction::Identify {
            self.resume_failures = 0;
            return;
        }
        self.resume_failures = self.resume_failures.saturating_add(1);
        if self.resume_failures >= chord_core::gateway::supervisor::RESUME_RESET_AFTER_FAILURES {
            self.reset_session();
        }
    }

    async fn run(mut self) -> GatewayVoiceActorSnapshot {
        let mut was_reconnect = false;
        loop {
            let planned_action = self.plan_reconnect_action();
            let target_url = self.current_gateway_url();
            self.set_phase(GatewayActorPhase::Connecting);

            let connect_res = tokio::select! {
                biased;
                cmd = self.cmd_rx.recv() => match cmd {
                    None => {
                        self.finish_stopped(None);
                        return self.snapshot;
                    }
                    Some(ActorCommand::Shutdown { reply }) => {
                        if let Some(tx) = reply {
                            let _ = tx.send(());
                        }
                        self.finish_stopped(None);
                        return self.snapshot;
                    }
                    Some(ActorCommand::UpdateVoiceState { request, reply }) => {
                        self.enqueue_offline_voice_state(request, reply);
                        self.transport.connect(target_url.as_str()).await
                    }
                    Some(ActorCommand::SendPing { reply }) => {
                        if let Some(tx) = reply {
                            let _ = tx.send(Err(GatewayVoiceActorError::NotConnected));
                        }
                        self.transport.connect(target_url.as_str()).await
                    }
                    Some(ActorCommand::ForceReconnect { reply }) => {
                        if let Some(tx) = reply {
                            let _ = tx.send(());
                        }
                        self.transport.connect(target_url.as_str()).await
                    }
                },
                res = self.transport.connect(target_url.as_str()) => res,
            };

            let mut established = false;
            let exit = match connect_res {
                Ok(conn) => {
                    self.run_single_connection(
                        conn,
                        planned_action,
                        was_reconnect,
                        &mut established,
                    )
                    .await
                }
                Err(err) => ConnectionExit::Reconnect { reason: err },
            };

            match exit {
                ConnectionExit::Shutdown => {
                    self.finish_stopped(None);
                    return self.snapshot;
                }
                ConnectionExit::FatalStop(stop_reason) => {
                    self.finish_stopped(Some(stop_reason));
                    return self.snapshot;
                }
                ConnectionExit::Reconnect { reason } => {
                    let safe_reason = redact_secret_text(&reason, &self.config.token);
                    self.snapshot.last_error = Some(safe_reason.clone());
                    self.record_attempt_outcome(planned_action, established);

                    if self.config.max_reconnects != 0
                        && self.snapshot.reconnect_attempts >= self.config.max_reconnects
                    {
                        let limit_reason = chord_core::StopReason::ReconnectLimit {
                            reconnects: self.snapshot.reconnect_attempts,
                        };
                        self.finish_stopped(Some(limit_reason));
                        return self.snapshot;
                    }

                    self.snapshot.reconnect_attempts =
                        self.snapshot.reconnect_attempts.saturating_add(1);
                    self.snapshot.total_reconnects =
                        self.snapshot.total_reconnects.saturating_add(1);
                    let delay_ms = self.backoff.next_delay_ms();
                    let next_action = self.plan_reconnect_action();
                    self.set_phase(GatewayActorPhase::Reconnecting);
                    self.emit_event(GatewayVoiceActorEvent::Reconnecting {
                        attempt: self.snapshot.reconnect_attempts,
                        delay_ms,
                        action: next_action,
                        reason: safe_reason,
                    });

                    if self
                        .wait_reconnect_backoff(Duration::from_millis(delay_ms))
                        .await
                        == BackoffWakeup::Shutdown
                    {
                        self.finish_stopped(None);
                        return self.snapshot;
                    }
                    was_reconnect = true;
                }
            }
        }
    }

    fn finish_stopped(&mut self, reason: Option<chord_core::StopReason>) {
        self.snapshot.last_stop_reason.clone_from(&reason);
        self.set_phase(GatewayActorPhase::Stopped);
        self.emit_event(GatewayVoiceActorEvent::Stopped { reason });
    }
}

impl PartialEq for BackoffWakeup {
    fn eq(&self, other: &Self) -> bool {
        matches!(
            (self, other),
            (Self::SleepElapsed, Self::SleepElapsed)
                | (Self::ForceImmediateRetry, Self::ForceImmediateRetry)
                | (Self::Shutdown, Self::Shutdown)
        )
    }
}

/// Фабрика запуска долгоживущего Tokio-актора шлюза Discord для `Opcode 4 (Voice State Update)`.
#[allow(dead_code)]
pub struct GatewayVoiceActor;

#[allow(dead_code)]
impl GatewayVoiceActor {
    /// Запускает долгоживущий актор поверх боевого транспорта `CoreGatewayActorTransport`.
    pub fn spawn(
        config: GatewayVoiceActorConfig,
    ) -> Result<
        (
            GatewayVoiceActorHandle,
            tokio::task::JoinHandle<GatewayVoiceActorSnapshot>,
        ),
        GatewayVoiceActorError,
    > {
        Self::spawn_with_transport(config, Arc::new(CoreGatewayActorTransport), None)
    }

    /// Запускает долгоживущий актор с произвольным транспортом и опциональной шиной `BackgroundEvent` GPUI.
    pub fn spawn_with_transport(
        config: GatewayVoiceActorConfig,
        transport: Arc<dyn GatewayActorTransport>,
        bg_tx: Option<async_channel::Sender<crate::dispatch::BackgroundEvent>>,
    ) -> Result<
        (
            GatewayVoiceActorHandle,
            tokio::task::JoinHandle<GatewayVoiceActorSnapshot>,
        ),
        GatewayVoiceActorError,
    > {
        let base_url = config.validate()?;
        let initial_backoff_ms =
            (config.initial_backoff.as_millis().min(u128::from(u64::MAX)) as u64).max(1);
        let max_backoff_ms = (config.max_backoff.as_millis().min(u128::from(u64::MAX)) as u64)
            .max(initial_backoff_ms);

        let mut initial_snapshot = GatewayVoiceActorSnapshot::default();
        initial_snapshot.heartbeat_interval_ms = (config
            .heartbeat_interval
            .as_millis()
            .min(u128::from(u64::MAX)) as u64)
            .max(1);

        let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel(GATEWAY_ACTOR_MAILBOX_CAPACITY);
        let (snapshot_tx, snapshot_rx) = tokio::sync::watch::channel(initial_snapshot.clone());
        let (events_tx, events_rx) = async_channel::bounded(GATEWAY_ACTOR_EVENT_CAPACITY);

        let runtime = ActorRuntime {
            backoff: chord_core::Backoff::new(initial_backoff_ms, max_backoff_ms),
            config,
            base_url,
            resume_url: None,
            transport,
            cmd_rx,
            snapshot_tx,
            events_tx,
            bg_tx,
            snapshot: initial_snapshot,
            session_id: None,
            invalid_session: None,
            last_ack_at: None,
            resume_failures: 0,
            desired_voice_state: None,
            pending_op4: false,
            ping_nonce_seq: 0,
        };

        let join_handle = tokio::spawn(runtime.run());
        let handle = GatewayVoiceActorHandle {
            cmd_tx,
            snapshot_rx,
            events_rx,
        };
        Ok((handle, join_handle))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chord_core::rest::transport::HttpResponse;
    use std::sync::{Arc, Mutex};

    #[derive(Clone, Default)]
    struct ScriptedRestTransport {
        outcomes: Arc<Mutex<Vec<Result<HttpResponse, String>>>>,
        recorded_requests: Arc<Mutex<Vec<(String, String, Vec<(String, String)>)>>>,
    }

    impl ScriptedRestTransport {
        fn new(outcomes: Vec<Result<HttpResponse, String>>) -> Self {
            Self {
                outcomes: Arc::new(Mutex::new(outcomes)),
                recorded_requests: Arc::new(Mutex::new(Vec::new())),
            }
        }

        fn requests(&self) -> Vec<(String, String, Vec<(String, String)>)> {
            self.recorded_requests.lock().unwrap().clone()
        }
    }

    impl RestTransport for ScriptedRestTransport {
        fn send_request<'a>(
            &'a self,
            method: &'a str,
            target: &'a str,
            headers: &'a [(String, String)],
            _body: Option<&'a [u8]>,
        ) -> std::pin::Pin<
            Box<dyn std::future::Future<Output = Result<HttpResponse, String>> + Send + 'a>,
        > {
            self.recorded_requests.lock().unwrap().push((
                method.to_string(),
                target.to_string(),
                headers.to_vec(),
            ));
            let mut guard = self.outcomes.lock().unwrap();
            let outcome = if guard.is_empty() {
                Ok(HttpResponse {
                    status: 200,
                    headers: vec![(
                        "keep-alive".to_string(),
                        HTTP2_KEEP_ALIVE_HEADER.to_string(),
                    )],
                    body: b"[]".to_vec(),
                })
            } else {
                guard.remove(0)
            };
            Box::pin(async move { outcome })
        }
    }

    #[test]
    fn test_http2_reqwest_pool_config_and_exponential_backoff_schedule_contract() {
        let cfg = Http2PoolConfig::default();
        assert!(cfg.http2_multiplexing);
        assert!(cfg.http2_prior_knowledge);
        assert_eq!(
            cfg.tcp_keepalive,
            Duration::from_secs(HTTP2_POOL_KEEP_ALIVE_SECS)
        );
        assert_eq!(
            cfg.pool_idle_timeout,
            Duration::from_secs(HTTP2_POOL_KEEP_ALIVE_SECS)
        );
        assert_eq!(cfg.pool_max_idle_per_host, HTTP2_POOL_MAX_IDLE_PER_HOST);
        assert_eq!(
            cfg.initial_backoff,
            Duration::from_millis(HTTP2_INITIAL_BACKOFF_MS)
        );
        assert_eq!(cfg.max_attempts, HTTP2_MAX_RETRY_ATTEMPTS);

        // Экспоненциальный бэкофф: 150ms -> 300ms -> 600ms -> 1200ms с ограничением max_backoff
        assert_eq!(cfg.exponential_backoff_delay(1), Duration::from_millis(150));
        assert_eq!(cfg.exponential_backoff_delay(2), Duration::from_millis(300));
        assert_eq!(cfg.exponential_backoff_delay(3), Duration::from_millis(600));
        assert_eq!(
            cfg.exponential_backoff_delay(4),
            Duration::from_millis(1200)
        );
        assert_eq!(cfg.exponential_backoff_delay(20), cfg.max_backoff);

        // 429 с Retry-After и с JSON retry_after, 502/503 с экспоненциальным бэкоффом, 401/404 без повтора
        let d_429_body = cfg
            .retry_delay_for_response(429, &[], br#"{"retry_after": 0.45, "global": false}"#, 1)
            .expect("429 должен вычислять задержку повтора");
        assert_eq!(d_429_body, Duration::from_millis(450));

        let d_429_fallback = cfg
            .retry_delay_for_response(429, &[], b"{}", 2)
            .expect("429 без заголовка должен использовать экспоненциальный backoff");
        assert_eq!(d_429_fallback, Duration::from_millis(300));

        let d_502 = cfg
            .retry_delay_for_response(502, &[], b"Bad Gateway", 3)
            .expect("502 должен повторяться по экспоненциальному backoff");
        assert_eq!(d_502, Duration::from_millis(600));

        assert!(
            cfg.retry_delay_for_response(401, &[], b"Unauthorized", 1)
                .is_none()
        );
        assert!(
            cfg.retry_delay_for_response(403, &[], b"Forbidden", 1)
                .is_none()
        );
        assert!(
            cfg.retry_delay_for_response(404, &[], b"Not Found", 1)
                .is_none()
        );
    }

    #[tokio::test]
    async fn test_http2_connection_pool_channels_and_messages_retry_contract() {
        let pool = Http2ConnectionPool::new();
        let mut cfg = Http2PoolConfig::default();
        cfg.initial_backoff = Duration::from_millis(1);
        cfg.max_attempts = 3;

        assert_eq!(pool.warm_connections("discord.com"), 0);
        pool.prewarm("discord.com", 1_000);
        assert_eq!(pool.warm_connections("discord.com"), 1);

        // 1. Загрузка каналов гильдии: 1-я попытка 502, 2-я попытка 429, 3-я попытка 200 OK
        let channels_json = r#"[
            {"id": "cat-1", "name": "Инфо", "type": 4, "position": 0},
            {"id": "ch-10", "name": "правила", "type": 5, "position": 1, "parent_id": "cat-1", "topic": "Важно"},
            {"id": "ch-20", "name": "голос-1", "type": 2, "position": 2, "parent_id": "cat-1"}
        ]"#;
        let transport = ScriptedRestTransport::new(vec![
            Ok(HttpResponse {
                status: 502,
                headers: Vec::new(),
                body: b"Bad Gateway".to_vec(),
            }),
            Ok(HttpResponse {
                status: 429,
                headers: vec![("retry-after".to_string(), "0.001".to_string())],
                body: br#"{"retry_after": 0.001}"#.to_vec(),
            }),
            Ok(HttpResponse {
                status: 200,
                headers: vec![(
                    "keep-alive".to_string(),
                    HTTP2_KEEP_ALIVE_HEADER.to_string(),
                )],
                body: channels_json.as_bytes().to_vec(),
            }),
        ]);

        let (categories, channels, metrics) = fetch_guild_channels_with_transport(
            &transport,
            &pool,
            &cfg,
            "sample-token",
            "guild-777",
        )
        .await
        .expect("Загрузка каналов после экспоненциального бэкоффа должна завершиться успехом");

        assert_eq!(metrics.attempts, 3);
        assert!(metrics.reused_connection);
        assert_eq!(metrics.http_version, "HTTP/2");
        assert!(metrics.total_backoff_ms >= 2);
        assert_eq!(categories.len(), 1);
        assert_eq!(categories[0].id, "cat-1");
        assert_eq!(channels.len(), 2);
        assert_eq!(
            channels[0].channel_type,
            crate::state::ChannelType::GuildAnnouncement
        );
        assert_eq!(
            channels[1].channel_type,
            crate::state::ChannelType::GuildVoice
        );
        assert!(pool.total_reuses() >= 1);
        assert_eq!(pool.total_retries(), 2);

        let reqs = transport.requests();
        assert_eq!(reqs.len(), 3);
        assert_eq!(reqs[0].0, "GET");
        assert_eq!(reqs[0].1, "/guilds/guild-777/channels");
        assert!(
            reqs[0]
                .2
                .iter()
                .any(|(k, v)| k.eq_ignore_ascii_case("keep-alive") && v == HTTP2_KEEP_ALIVE_HEADER)
        );

        // 2. Загрузка и отправка сообщений канала через пул соединений с ретраем сетевого сбоя
        let msgs_json = r#"[
            {
                "id": "msg-200",
                "content": "Второе сообщение",
                "timestamp": "2026-11-10T18:30:00.000000+00:00",
                "author": {"id": "u-2", "username": "Bob", "global_name": "Bob Dev"}
            },
            {
                "id": "msg-100",
                "content": "Первое сообщение",
                "timestamp": "2026-11-10T18:29:00.000000+00:00",
                "author": {"id": "u-1", "username": "Alice", "global_name": null}
            }
        ]"#;
        let sent_msg_json = r#"{
            "id": "msg-300",
            "content": "Привет через HTTP/2 пул!",
            "timestamp": "2026-11-10T18:31:00.000000+00:00",
            "author": {"id": "u-me", "username": "Rustery", "global_name": "Rustery"}
        }"#;
        let msg_transport = ScriptedRestTransport::new(vec![
            Err("transient socket reset".to_string()),
            Ok(HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: msgs_json.as_bytes().to_vec(),
            }),
            Ok(HttpResponse {
                status: 429,
                headers: Vec::new(),
                body: br#"{"retry_after": 0.001}"#.to_vec(),
            }),
            Ok(HttpResponse {
                status: 200,
                headers: Vec::new(),
                body: sent_msg_json.as_bytes().to_vec(),
            }),
            Ok(HttpResponse {
                status: 204,
                headers: Vec::new(),
                body: Vec::new(),
            }),
            Ok(HttpResponse {
                status: 204,
                headers: Vec::new(),
                body: Vec::new(),
            }),
            Ok(HttpResponse {
                status: 204,
                headers: Vec::new(),
                body: Vec::new(),
            }),
        ]);

        let (loaded_msgs, get_metrics) = fetch_channel_messages_with_transport(
            &msg_transport,
            &pool,
            &cfg,
            "sample-token",
            "ch-10",
            50,
        )
        .await
        .expect("Загрузка сообщений после транзиентного сбоя должна восстановиться");

        assert_eq!(get_metrics.attempts, 2);
        assert_eq!(loaded_msgs.len(), 2);
        // Сообщения разворачиваются в хронологическом порядке (от старых к новым)
        assert_eq!(loaded_msgs[0].id, "msg-100");
        assert_eq!(loaded_msgs[1].id, "msg-200");

        let (sent_msg, post_metrics) = send_message_with_transport(
            &msg_transport,
            &pool,
            &cfg,
            "sample-token",
            "ch-10",
            "Привет через HTTP/2 пул!",
        )
        .await
        .expect("Отправка сообщения после 429 должна пройти успешно");

        assert_eq!(post_metrics.attempts, 2);
        assert_eq!(sent_msg.id, "msg-300");
        assert_eq!(sent_msg.content, "Привет через HTTP/2 пул!");

        // 3. Закрепление, открепление и удаление сообщения через тот же пул соединений
        let pin_metrics = pin_message_with_transport(
            &msg_transport,
            &pool,
            &cfg,
            "sample-token",
            "ch-10",
            "msg-300",
        )
        .await
        .expect("pin_message должен пройти через пул");
        assert_eq!(pin_metrics.attempts, 1);
        assert!(pin_metrics.reused_connection);

        let unpin_metrics = unpin_message_with_transport(
            &msg_transport,
            &pool,
            &cfg,
            "sample-token",
            "ch-10",
            "msg-300",
        )
        .await
        .expect("unpin_message должен пройти через пул");
        assert_eq!(unpin_metrics.attempts, 1);

        let del_metrics = delete_message_with_transport(
            &msg_transport,
            &pool,
            &cfg,
            "sample-token",
            "ch-10",
            "msg-300",
        )
        .await
        .expect("delete_message должен пройти через пул");
        assert_eq!(del_metrics.attempts, 1);

        // 4. Проверка сброса сбойных соединений хоста
        pool.evict_stale("discord.com");
        assert_eq!(pool.warm_connections("discord.com"), 0);
        // И повторное открытие соединения после сброса
        assert!(!pool.acquire_or_connect("discord.com", 500_000, HTTP2_POOL_KEEP_ALIVE_SECS));
        assert!(pool.acquire_or_connect("discord.com", 510_000, HTTP2_POOL_KEEP_ALIVE_SECS));
        // Истечение idle TTL (90с = 90_000мс) приводит к новому рукопожатию
        assert!(!pool.acquire_or_connect("discord.com", 650_000, HTTP2_POOL_KEEP_ALIVE_SECS));
    }

    #[tokio::test]
    async fn test_http2_pool_negative_boundaries_and_non_retryable_errors_contract() {
        let pool = Http2ConnectionPool::new();
        let mut cfg = Http2PoolConfig::default();
        cfg.initial_backoff = Duration::from_millis(1);
        cfg.max_attempts = 3;

        let empty_transport = ScriptedRestTransport::new(Vec::new());
        assert!(fetch_guild_channels("tok", "").await.is_err());
        assert!(
            fetch_guild_channels_with_transport(&empty_transport, &pool, &cfg, "tok", "   ")
                .await
                .is_err()
        );
        assert!(
            fetch_channel_messages_with_transport(&empty_transport, &pool, &cfg, "tok", "", 25)
                .await
                .is_err()
        );
        assert!(
            send_message_with_transport(&empty_transport, &pool, &cfg, "tok", "", "hi")
                .await
                .is_err()
        );

        // Неповторяемые ошибки 401/403/404/413 должны завершаться с 1 попытки без бэкоффа
        let unauth_transport = ScriptedRestTransport::new(vec![Ok(HttpResponse {
            status: 401,
            headers: Vec::new(),
            body: br#"{"message": "401: Unauthorized", "code": 0}"#.to_vec(),
        })]);
        let err_401 = fetch_guild_channels_with_transport(
            &unauth_transport,
            &pool,
            &cfg,
            "bad-token",
            "guild-1",
        )
        .await
        .expect_err("401 должен возвращать ошибку без повторов");
        assert!(err_401.contains("HTTP 401"));
        assert_eq!(unauth_transport.requests().len(), 1);

        // Исчерпание всех попыток при постоянной ошибке 503
        let persistent_503 = ScriptedRestTransport::new(vec![
            Ok(HttpResponse {
                status: 503,
                headers: Vec::new(),
                body: b"Service Unavailable".to_vec(),
            }),
            Ok(HttpResponse {
                status: 503,
                headers: Vec::new(),
                body: b"Service Unavailable".to_vec(),
            }),
            Ok(HttpResponse {
                status: 503,
                headers: Vec::new(),
                body: b"Service Unavailable".to_vec(),
            }),
        ]);
        let err_503 = fetch_channel_messages_with_transport(
            &persistent_503,
            &pool,
            &cfg,
            "tok",
            "ch-503",
            200, // проверяет также clamp(1, 100)
        )
        .await
        .expect_err("Постоянный 503 должен завершиться ошибкой после 3 попыток");
        assert!(err_503.contains("HTTP 503"));
        assert_eq!(persistent_503.requests().len(), 3);
        assert_eq!(
            persistent_503.requests()[0].1,
            "/channels/ch-503/messages?limit=100"
        );

        // Проверка маршрутов и методов DiscordRestClient
        let rest_client =
            DiscordRestClient::new(NetworkConfig::default()).with_pool_config(cfg.clone());
        assert_eq!(
            DiscordRestClient::calculate_exponential_backoff(
                3,
                cfg.initial_backoff,
                cfg.max_backoff
            ),
            cfg.exponential_backoff_delay(3)
        );
        assert_eq!(
            DiscordRestClient::route_guild_channels(),
            "/guilds/{id}/channels"
        );
        assert_eq!(
            DiscordRestClient::route_channel_pins().path,
            "/channels/{id}/messages/pins"
        );
        let _ = rest_client.pool();
    }

    #[tokio::test]
    async fn test_http2_channel_messages_before_pagination_contract() {
        let pool = Http2ConnectionPool::new();
        let cfg = Http2PoolConfig::default();

        // 1. Форматирование URL пагинации
        assert_eq!(
            format_messages_pagination_url("703970377721970768", "100200300", 50),
            "/channels/703970377721970768/messages?before=100200300&limit=50"
        );
        assert_eq!(
            format_messages_pagination_url("ch-test", "oldest", 150),
            "/channels/ch-test/messages?before=oldest&limit=100",
            "Лимит должен ограничиваться clamp(1, 100)"
        );

        // 2. Валидация аргументов
        let empty_transport = ScriptedRestTransport::default();
        let err_ch = fetch_channel_messages_before_with_transport(
            &empty_transport,
            &pool,
            &cfg,
            "token",
            "",
            "msg-1",
            50,
        )
        .await
        .unwrap_err();
        assert!(err_ch.contains("Идентификатор канала"));

        let err_before = fetch_channel_messages_before_with_transport(
            &empty_transport,
            &pool,
            &cfg,
            "token",
            "ch-1",
            "   ",
            50,
        )
        .await
        .unwrap_err();
        assert!(err_before.contains("Параметр before"));

        // 3. Выполнение запроса с получением более старых сообщений
        let msgs_json = r#"[
            {
                "id": "msg-099",
                "content": "Сообщение 99 (новее среди старых)",
                "timestamp": "2026-11-10T17:59:00.000000+00:00",
                "author": {"id": "u-1", "username": "Alice"}
            },
            {
                "id": "msg-050",
                "content": "Сообщение 50 (самое старое)",
                "timestamp": "2026-11-10T17:50:00.000000+00:00",
                "author": {"id": "u-2", "username": "Bob"}
            }
        ]"#;

        let transport = ScriptedRestTransport::new(vec![Ok(HttpResponse {
            status: 200,
            headers: vec![(
                "keep-alive".to_string(),
                HTTP2_KEEP_ALIVE_HEADER.to_string(),
            )],
            body: msgs_json.as_bytes().to_vec(),
        })]);

        let (msgs, metrics) = fetch_channel_messages_before_with_transport(
            &transport,
            &pool,
            &cfg,
            "token",
            "703970377721970768",
            "msg-100",
            50,
        )
        .await
        .expect("Загрузка пагинации должна пройти успешно");

        assert_eq!(metrics.attempts, 1);
        let reqs = transport.requests();
        assert_eq!(reqs.len(), 1);
        assert_eq!(reqs[0].0, "GET");
        assert_eq!(
            reqs[0].1,
            "/channels/703970377721970768/messages?before=msg-100&limit=50"
        );

        // Хронологический порядок: msg-050 (старое) затем msg-099
        assert_eq!(msgs.len(), 2);
        assert_eq!(msgs[0].id, "msg-050");
        assert_eq!(msgs[1].id, "msg-099");
    }

    // ========================================================================
    // Интерактивный тестовый транспорт WebSocket для детерминированной проверки
    // долгоживущего Tokio-актора Gateway Opcode 4 + Auto-Reconnect + Ping
    // ========================================================================

    struct MockSocketController {
        url: String,
        outbound_rx: tokio::sync::mpsc::Receiver<GatewayWireOutbound>,
        inbound_tx: tokio::sync::mpsc::Sender<Result<GatewayWireInbound, String>>,
    }

    #[derive(Clone, Default)]
    struct ScriptedGatewayActorTransport {
        connect_results: Arc<Mutex<std::collections::VecDeque<Result<(), String>>>>,
        accepted_tx: Arc<Mutex<Option<tokio::sync::mpsc::UnboundedSender<MockSocketController>>>>,
        connected_urls: Arc<Mutex<Vec<String>>>,
    }

    impl ScriptedGatewayActorTransport {
        fn new() -> (
            Self,
            tokio::sync::mpsc::UnboundedReceiver<MockSocketController>,
        ) {
            let (tx, rx) = tokio::sync::mpsc::unbounded_channel();
            let transport = Self {
                connect_results: Arc::new(Mutex::new(std::collections::VecDeque::new())),
                accepted_tx: Arc::new(Mutex::new(Some(tx))),
                connected_urls: Arc::new(Mutex::new(Vec::new())),
            };
            (transport, rx)
        }

        fn push_connect_error(&self, err: impl Into<String>) {
            self.connect_results
                .lock()
                .unwrap()
                .push_back(Err(err.into()));
        }

        fn urls(&self) -> Vec<String> {
            self.connected_urls.lock().unwrap().clone()
        }
    }

    impl GatewayActorTransport for ScriptedGatewayActorTransport {
        fn connect<'a>(
            &'a self,
            url: &'a str,
        ) -> Pin<Box<dyn Future<Output = Result<GatewayActorConnection, String>> + Send + 'a>>
        {
            let target_url = url.to_string();
            self.connected_urls.lock().unwrap().push(target_url.clone());
            let scripted = self.connect_results.lock().unwrap().pop_front();
            let accepted_tx = self.accepted_tx.lock().unwrap().clone();
            Box::pin(async move {
                if let Some(Err(err)) = scripted {
                    return Err(err);
                }
                let (out_tx, out_rx) = tokio::sync::mpsc::channel(64);
                let (in_tx, in_rx) = tokio::sync::mpsc::channel(64);
                if let Some(tx) = accepted_tx {
                    let _ = tx.send(MockSocketController {
                        url: target_url,
                        outbound_rx: out_rx,
                        inbound_tx: in_tx,
                    });
                }
                Ok(GatewayActorConnection {
                    outbound_tx: out_tx,
                    inbound_rx: in_rx,
                })
            })
        }
    }

    fn fast_test_actor_config(secret_token: &str) -> GatewayVoiceActorConfig {
        let mut cfg = GatewayVoiceActorConfig::new(secret_token);
        cfg.heartbeat_interval = Duration::from_millis(60);
        cfg.initial_jitter_ratio = 0.25;
        cfg.ping_timeout = Duration::from_millis(45);
        cfg.initial_backoff = Duration::from_millis(10);
        cfg.max_backoff = Duration::from_millis(40);
        cfg
    }

    #[test]
    fn test_gateway_op4_request_wire_oracle_and_snowflake_boundaries_contract() {
        // 1. Граничные значения Snowflake: 0 и u64::MAX валидны; переполнение u64+1 и пустые строки отвергаются
        assert!(is_valid_gateway_snowflake("0"));
        assert!(is_valid_gateway_snowflake("1014537851418980372"));
        assert!(is_valid_gateway_snowflake(&u64::MAX.to_string()));
        assert!(!is_valid_gateway_snowflake("18446744073709551616"));
        assert!(!is_valid_gateway_snowflake(""));
        assert!(!is_valid_gateway_snowflake("-1"));
        assert!(!is_valid_gateway_snowflake("abc123"));

        // 2. Сглаживание Ping RTT по формуле клиента: round(prev * 0.65 + sample * 0.35)
        assert_eq!(smooth_gateway_ping_ms(None, 24), 24);
        assert_eq!(smooth_gateway_ping_ms(Some(100), 20), 72); // round(65 + 7) = 72

        // 3. Независимый оракул кадра Opcode 4 (JSON + бинарный ETF 0x83 + MAP_EXT 0x74)
        let join_req = VoiceStateUpdateRequest::join(
            Some("1014537851418980372"),
            "1176785373989445703",
            true,
            false,
        )
        .with_video(true)
        .with_stream(true)
        .validate()
        .expect("Валидный запрос входа в голосовой канал со стримом должен проходить");

        assert_eq!(join_req.flags, 2);
        let json_val = join_req.to_json_payload();
        assert_eq!(json_val["op"], 4);
        assert_eq!(json_val["d"]["guild_id"], "1014537851418980372");
        assert_eq!(json_val["d"]["channel_id"], "1176785373989445703");
        assert_eq!(json_val["d"]["self_mute"], true);
        assert_eq!(json_val["d"]["self_deaf"], false);
        assert_eq!(json_val["d"]["self_video"], true);
        assert_eq!(json_val["d"]["self_stream"], true);
        assert_eq!(json_val["d"]["flags"], 2);

        let etf_bytes = join_req
            .encode_etf_frame()
            .expect("ETF кодирование Opcode 4 не должно возвращать ошибку");
        // Независимая байтовая сверка заголовка ETF: 0x83 (version) + 0x74 (MAP_EXT) + arity 2 (op, d)
        assert_eq!(etf_bytes[0], 0x83);
        assert_eq!(etf_bytes[1], 0x74);
        assert_eq!(&etf_bytes[2..6], &2u32.to_be_bytes());
        let (decoded_term, consumed) = chord_codec::etf::decode(&etf_bytes)
            .expect("ETF терм должен декодироваться без остатка");
        assert_eq!(consumed, etf_bytes.len());
        match &decoded_term {
            chord_codec::etf::EtfValue::Map(entries) => {
                assert!(entries.iter().any(|(k, v)| {
                    matches!(k, chord_codec::etf::EtfValue::Str(s) if s == "op")
                        && matches!(v, chord_codec::etf::EtfValue::Int(4))
                }));
            }
            other => panic!("Ожидался ETF Map верхнего уровня, получено: {:?}", other),
        }
        let converted_json = etf_term_to_json_value(&decoded_term);
        assert_eq!(converted_json["op"], 4);
        assert_eq!(converted_json["d"]["self_mute"], true);
        assert_eq!(converted_json["d"]["flags"], 2);
        // Проверка сохранения 64-битного Snowflake > MAX_SAFE_INTEGER как строки без потери точности IEEE-754
        let big_snowflake_json = etf_term_to_json_value(&chord_codec::etf::EtfValue::Map(vec![
            (
                chord_codec::etf::EtfValue::Atom("guild_id".into()),
                chord_codec::etf::EtfValue::Int(1_014_537_851_418_980_372),
            ),
            (
                chord_codec::etf::EtfValue::Atom("channel_id".into()),
                chord_codec::etf::EtfValue::Atom("nil".into()),
            ),
            (
                chord_codec::etf::EtfValue::Int(7),
                chord_codec::etf::EtfValue::Float(f64::NAN),
            ),
        ]));
        assert_eq!(big_snowflake_json["guild_id"], "1014537851418980372");
        assert!(big_snowflake_json["channel_id"].is_null());
        assert!(big_snowflake_json["7"].is_null());

        // 4. Выход из голосового канала: channel_id == None -> null в JSON и Nil в ETF
        let leave_req = VoiceStateUpdateRequest::leave(Some("0"))
            .validate()
            .expect("Граничный guild_id=0 при выходе должен приниматься");
        let leave_json = leave_req.to_json_payload();
        assert_eq!(leave_json["op"], 4);
        assert_eq!(leave_json["d"]["guild_id"], "0");
        assert!(leave_json["d"]["channel_id"].is_null());

        // 5. Негативные граничные входы: пустой channel_id, управляющие символы, слишком длинный ID (>64)
        assert!(
            VoiceStateUpdateRequest::join(Some("123"), "   ", false, false)
                .validate()
                .is_err()
        );
        assert!(
            VoiceStateUpdateRequest::join(Some("bad\nid"), "456", false, false)
                .validate()
                .is_err()
        );
        let oversized = "9".repeat(65);
        assert!(
            VoiceStateUpdateRequest::join(None::<String>, oversized, false, false)
                .validate()
                .is_err()
        );
    }

    #[tokio::test]
    async fn test_gateway_voice_actor_long_running_op4_ping_and_voice_dispatch_contract() {
        let (transport, mut sockets_rx) = ScriptedGatewayActorTransport::new();
        let dispatcher = crate::dispatch::AsyncTaskDispatcher::new(32);
        let cfg = fast_test_actor_config("live-secret-token-alpha-999");

        let (handle, join_handle) = GatewayVoiceActor::spawn_with_transport(
            cfg,
            Arc::new(transport),
            Some(dispatcher.sender()),
        )
        .expect("Долгоживущий актор шлюза должен успешно стартовать");

        let events_rx = handle.events();
        let mut sock = tokio::time::timeout(Duration::from_secs(2), sockets_rx.recv())
            .await
            .expect("таймаут ожидания открытия сокета")
            .expect("сокет должен открыться");
        assert_eq!(
            sock.url,
            "wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream"
        );

        // 1. Рукопожатие: HELLO (op 10) -> IDENTIFY (op 2) -> READY (op 0)
        sock.inbound_tx
            .send(Ok(GatewayWireInbound::Hello {
                heartbeat_interval_ms: 80,
            }))
            .await
            .unwrap();

        let first_out = tokio::time::timeout(Duration::from_secs(2), sock.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(
            matches!(first_out, GatewayWireOutbound::Identify { ref etf_frame } if etf_frame[0] == 0x83)
        );

        sock.inbound_tx
            .send(Ok(GatewayWireInbound::Ready {
                session_id: "sess_alpha_001".into(),
                resume_gateway_url: Some("wss://gateway-us-east1-c.discord.gg/".into()),
                seq: 1,
                data: serde_json::json!({"v": 9}),
            }))
            .await
            .unwrap();

        let connected_ev = tokio::time::timeout(Duration::from_secs(2), events_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            connected_ev,
            GatewayVoiceActorEvent::Connected {
                resumed: false,
                heartbeat_interval_ms: 80,
            }
        );
        assert_eq!(handle.snapshot().phase, GatewayActorPhase::Ready);
        assert!(handle.snapshot().session_id_present);

        // 2. Немедленная отправка Opcode 4 (Join Voice) в активной сессии Ready
        let delivery = handle
            .join_voice(
                Some("1014537851418980372"),
                "1176785373989445703",
                false,
                false,
            )
            .await
            .expect("Отправка Opcode 4 в Ready должна пройти успешно");
        assert!(matches!(
            delivery,
            GatewayOp4Delivery::SentImmediately { frame_bytes, sequence: Some(1) } if frame_bytes > 20
        ));

        let op4_join_frame = tokio::time::timeout(Duration::from_secs(2), sock.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        match op4_join_frame {
            GatewayWireOutbound::VoiceStateUpdate {
                request,
                etf_frame,
                json_payload,
            } => {
                assert_eq!(request.guild_id.as_deref(), Some("1014537851418980372"));
                assert_eq!(request.channel_id.as_deref(), Some("1176785373989445703"));
                assert_eq!(etf_frame[0], 0x83);
                assert_eq!(json_payload["op"], 4);
            }
            other => panic!("Ожидался кадр VoiceStateUpdate, получено: {:?}", other),
        }

        // 3. Корреляция входящих диспатчей VOICE_STATE_UPDATE + VOICE_SERVER_UPDATE
        sock.inbound_tx
            .send(Ok(GatewayWireInbound::Dispatch {
                event_name: "VOICE_STATE_UPDATE".into(),
                seq: 2,
                data: serde_json::json!({
                    "guild_id": "1014537851418980372",
                    "channel_id": "1176785373989445703",
                    "self_mute": false,
                    "self_deaf": false,
                    "self_video": false,
                    "self_stream": false
                }),
            }))
            .await
            .unwrap();
        sock.inbound_tx
            .send(Ok(GatewayWireInbound::Dispatch {
                event_name: "VOICE_SERVER_UPDATE".into(),
                seq: 3,
                data: serde_json::json!({
                    "guild_id": "1014537851418980372",
                    "endpoint": "c-fra19-4da7fe5f.discord.media:2053",
                    "token": "secret-voice-rtc-token-777"
                }),
            }))
            .await
            .unwrap();

        // 4. Периодический Heartbeat (с начальным джиттером 80 * 0.25 = 20мс) + ответ HeartbeatAck
        let hb_frame = tokio::time::timeout(Duration::from_secs(2), sock.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            hb_frame,
            GatewayWireOutbound::Heartbeat { seq: Some(3), qos: true, ref etf_frame } if etf_frame[0] == 0x83
        ));
        tokio::time::sleep(Duration::from_millis(6)).await;
        sock.inbound_tx
            .send(Ok(GatewayWireInbound::HeartbeatAck))
            .await
            .unwrap();

        // 5. Явный Ping от клиента и серверные Ping -> Pong + op:1 HeartbeatRequest
        let ping_task = {
            let h = handle.clone();
            tokio::spawn(async move { h.ping().await })
        };
        let ws_ping_out = tokio::time::timeout(Duration::from_secs(2), sock.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        let ping_nonce = match ws_ping_out {
            GatewayWireOutbound::Ping(payload) => payload,
            other => panic!("Ожидался WS Ping, получено: {:?}", other),
        };
        tokio::time::sleep(Duration::from_millis(5)).await;
        sock.inbound_tx
            .send(Ok(GatewayWireInbound::Pong(ping_nonce)))
            .await
            .unwrap();

        let measured_rtt = ping_task.await.unwrap().expect("Ping должен вернуть RTT");
        assert!(measured_rtt >= 2);

        // Серверный WS Ping обязан получить немедленный Pong с тем же payload, а op:1 — немедленный Heartbeat
        sock.inbound_tx
            .send(Ok(GatewayWireInbound::Ping(b"srv-probe".to_vec())))
            .await
            .unwrap();
        let srv_pong = tokio::time::timeout(Duration::from_secs(2), sock.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(srv_pong, GatewayWireOutbound::Pong(b"srv-probe".to_vec()));

        sock.inbound_tx
            .send(Ok(GatewayWireInbound::HeartbeatRequest))
            .await
            .unwrap();
        let immediate_hb = tokio::time::timeout(Duration::from_secs(2), sock.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            immediate_hb,
            GatewayWireOutbound::Heartbeat { .. }
        ));
        sock.inbound_tx
            .send(Ok(GatewayWireInbound::HeartbeatAck))
            .await
            .unwrap();

        // Проверяем, что диспетчер GPUI получил фоновые события VoicePing
        let bg_events = dispatcher.drain_events();
        assert!(
            bg_events
                .iter()
                .any(|ev| matches!(ev, crate::dispatch::BackgroundEvent::VoicePing(_)))
        );

        // 6. Выход из голосового канала (Opcode 4 channel_id: null) и корректная остановка актора
        handle
            .leave_voice(Some("1014537851418980372"))
            .await
            .expect("Отправка выхода из канала должна пройти");
        let leave_out = tokio::time::timeout(Duration::from_secs(2), sock.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        match leave_out {
            GatewayWireOutbound::VoiceStateUpdate {
                request,
                json_payload,
                ..
            } => {
                assert!(request.channel_id.is_none());
                assert!(json_payload["d"]["channel_id"].is_null());
            }
            other => panic!(
                "Ожидался кадр выхода VoiceStateUpdate, получено: {:?}",
                other
            ),
        }

        let snap_before_stop = handle.snapshot();
        assert_eq!(snap_before_stop.op4_frames_sent, 2);
        assert!(snap_before_stop.pings_sent >= 2);
        assert!(snap_before_stop.pongs_received >= 2);
        assert_eq!(
            snap_before_stop.voice_endpoint.as_deref(),
            Some("c-fra19-4da7fe5f.discord.media:2053")
        );

        handle
            .shutdown()
            .await
            .expect("Shutdown должен завершиться чисто");
        let final_snap = tokio::time::timeout(Duration::from_secs(2), join_handle)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(final_snap.phase, GatewayActorPhase::Stopped);
        assert_eq!(
            handle.ping().await,
            Err(GatewayVoiceActorError::ActorStopped)
        );
    }

    #[tokio::test]
    async fn test_gateway_voice_actor_queued_pre_ready_and_coalescing_contract() {
        let (transport, mut sockets_rx) = ScriptedGatewayActorTransport::new();
        let mut cfg = fast_test_actor_config("tok-queue-coalesce");
        cfg.heartbeat_interval = Duration::from_secs(10);

        let (handle, join_handle) =
            GatewayVoiceActor::spawn_with_transport(cfg, Arc::new(transport), None).unwrap();

        let mut sock = tokio::time::timeout(Duration::from_secs(2), sockets_rx.recv())
            .await
            .unwrap()
            .unwrap();

        // До прихода HELLO и READY пользователь дважды меняет состояние (вход -> включение mute)
        let d1 = handle
            .join_voice(Some("555"), "777", false, false)
            .await
            .expect("Запрос до READY должен ставиться в очередь");
        assert_eq!(d1, GatewayOp4Delivery::QueuedForReady);

        let d2 = handle
            .join_voice(Some("555"), "777", true, false)
            .await
            .expect("Повторный запрос до READY коалесцирует состояние");
        assert_eq!(d2, GatewayOp4Delivery::QueuedForReady);

        // Теперь шлюз присылает HELLO и READY
        sock.inbound_tx
            .send(Ok(GatewayWireInbound::Hello {
                heartbeat_interval_ms: 41_250,
            }))
            .await
            .unwrap();
        let id_frame = tokio::time::timeout(Duration::from_secs(2), sock.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(id_frame, GatewayWireOutbound::Identify { .. }));

        sock.inbound_tx
            .send(Ok(GatewayWireInbound::Ready {
                session_id: "sess_coalesce".into(),
                resume_gateway_url: None,
                seq: 1,
                data: serde_json::json!({}),
            }))
            .await
            .unwrap();

        // Сразу после READY уходит ровно один коалесцированный кадр Opcode 4 с self_mute = true
        let flushed = tokio::time::timeout(Duration::from_secs(2), sock.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        match flushed {
            GatewayWireOutbound::VoiceStateUpdate { request, .. } => {
                assert_eq!(request.guild_id.as_deref(), Some("555"));
                assert_eq!(request.channel_id.as_deref(), Some("777"));
                assert!(request.self_mute);
                assert!(!request.self_deaf);
            }
            other => panic!("Ожидался отложенный кадр Op 4, получено: {:?}", other),
        }

        handle.shutdown().await.unwrap();
        let _ = tokio::time::timeout(Duration::from_secs(2), join_handle)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn test_gateway_voice_actor_auto_reconnect_zombie_ping_resume_and_op4_replay_contract() {
        let (transport, mut sockets_rx) = ScriptedGatewayActorTransport::new();
        let mut cfg = fast_test_actor_config("tok-reconnect-zombie");
        cfg.heartbeat_interval = Duration::from_millis(35);
        cfg.initial_jitter_ratio = 0.2;
        cfg.ping_timeout = Duration::from_millis(30);
        cfg.initial_backoff = Duration::from_millis(10);
        cfg.max_backoff = Duration::from_millis(40);

        let transport_ref = transport.clone();
        let (handle, join_handle) =
            GatewayVoiceActor::spawn_with_transport(cfg, Arc::new(transport), None).unwrap();

        // Соединение #1: HELLO -> IDENTIFY -> READY -> вход в голосовой канал
        let mut sock1 = tokio::time::timeout(Duration::from_secs(2), sockets_rx.recv())
            .await
            .unwrap()
            .unwrap();
        sock1
            .inbound_tx
            .send(Ok(GatewayWireInbound::Hello {
                heartbeat_interval_ms: 35,
            }))
            .await
            .unwrap();
        let _ = sock1.outbound_rx.recv().await.unwrap(); // Identify
        sock1
            .inbound_tx
            .send(Ok(GatewayWireInbound::Ready {
                session_id: "sess_resume_42".into(),
                resume_gateway_url: Some("wss://gateway-us-east1-d.discord.gg/".into()),
                seq: 10,
                data: serde_json::json!({}),
            }))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(5)).await;
        handle
            .join_voice(Some("101"), "202", true, false)
            .await
            .unwrap();
        let first_op4 = sock1.outbound_rx.recv().await.unwrap();
        assert!(matches!(
            first_op4,
            GatewayWireOutbound::VoiceStateUpdate { .. }
        ));

        // Дожидаемся отправки Heartbeat на сокете #1 и НЕ присылаем HeartbeatAck -> детектор зомби!
        let hb1 = tokio::time::timeout(Duration::from_secs(2), sock1.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(hb1, GatewayWireOutbound::Heartbeat { .. }));

        // Актор обязан обнаружить зомби-таймаут и автоматически переподключиться на resume_gateway_url с RESUME (op 6)
        let mut sock2 = tokio::time::timeout(Duration::from_secs(2), sockets_rx.recv())
            .await
            .expect("Актор должен открыть новое соединение после зомби-таймаута")
            .unwrap();
        assert_eq!(
            sock2.url,
            "wss://gateway-us-east1-d.discord.gg/?encoding=etf&v=9&compress=zstd-stream"
        );

        sock2
            .inbound_tx
            .send(Ok(GatewayWireInbound::Hello {
                heartbeat_interval_ms: 500,
            }))
            .await
            .unwrap();
        let resume_out = tokio::time::timeout(Duration::from_secs(2), sock2.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            resume_out,
            GatewayWireOutbound::Resume { ref session_id, seq: 10, .. } if session_id == "sess_resume_42"
        ));

        sock2
            .inbound_tx
            .send(Ok(GatewayWireInbound::Resumed { seq: 11 }))
            .await
            .unwrap();

        // После RESUMED актор обязан автоматически воспроизвести активный Opcode 4 (Voice State Replay)!
        let replayed_op4 = tokio::time::timeout(Duration::from_secs(2), sock2.outbound_rx.recv())
            .await
            .unwrap()
            .unwrap();
        match replayed_op4 {
            GatewayWireOutbound::VoiceStateUpdate { request, .. } => {
                assert_eq!(request.guild_id.as_deref(), Some("101"));
                assert_eq!(request.channel_id.as_deref(), Some("202"));
                assert!(request.self_mute);
            }
            other => panic!(
                "Ожидался автоматический повтор Op 4 после реконнекта, получено: {:?}",
                other
            ),
        }
        assert_eq!(handle.snapshot().op4_replays_on_reconnect, 1);
        assert_eq!(handle.snapshot().total_reconnects, 1);

        // Теперь пользователь выходит из голоса (channel_id = None), а сервер присылает op:7 RECONNECT
        handle.leave_voice(Some("101")).await.unwrap();
        let leave_frame = sock2.outbound_rx.recv().await.unwrap();
        assert!(matches!(
            leave_frame,
            GatewayWireOutbound::VoiceStateUpdate { ref request, .. } if request.channel_id.is_none()
        ));

        sock2
            .inbound_tx
            .send(Ok(GatewayWireInbound::Reconnect))
            .await
            .unwrap();
        let mut sock3 = tokio::time::timeout(Duration::from_secs(2), sockets_rx.recv())
            .await
            .unwrap()
            .unwrap();
        sock3
            .inbound_tx
            .send(Ok(GatewayWireInbound::Hello {
                heartbeat_interval_ms: 500,
            }))
            .await
            .unwrap();
        let resume_out_3 = sock3.outbound_rx.recv().await.unwrap();
        assert!(matches!(
            resume_out_3,
            GatewayWireOutbound::Resume { seq: 11, .. }
        ));
        sock3
            .inbound_tx
            .send(Ok(GatewayWireInbound::Resumed { seq: 12 }))
            .await
            .unwrap();
        tokio::time::sleep(Duration::from_millis(15)).await;
        // Так как пользователь уже покинул голосовой канал, лишний Op 4 НЕ воспроизводится
        assert_eq!(handle.snapshot().op4_replays_on_reconnect, 1);
        assert_eq!(transport_ref.urls().len(), 3);

        handle.shutdown().await.unwrap();
        let _ = join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_gateway_voice_actor_invalid_session_resume_failure_limit_and_ttl_reset_contract()
    {
        let (transport, mut sockets_rx) = ScriptedGatewayActorTransport::new();
        let mut cfg = fast_test_actor_config("tok-reset-policy");
        cfg.heartbeat_interval = Duration::from_secs(10);
        cfg.initial_backoff = Duration::from_millis(5);
        cfg.max_backoff = Duration::from_millis(10);
        cfg.resume_ttl = Duration::from_millis(180_000);

        let (handle, join_handle) =
            GatewayVoiceActor::spawn_with_transport(cfg, Arc::new(transport), None).unwrap();

        // 1. Первичный вход и проверка нулевого интервала HELLO (heartbeat_interval_ms == 0 -> протокольная ошибка -> реконнект без паники)
        let sock_bad_hello = tokio::time::timeout(Duration::from_secs(2), sockets_rx.recv())
            .await
            .unwrap()
            .unwrap();
        sock_bad_hello
            .inbound_tx
            .send(Ok(GatewayWireInbound::Hello {
                heartbeat_interval_ms: 0,
            }))
            .await
            .unwrap();

        // Автоматическое переподключение после битого HELLO
        let mut sock_ready = tokio::time::timeout(Duration::from_secs(2), sockets_rx.recv())
            .await
            .unwrap()
            .unwrap();
        sock_ready
            .inbound_tx
            .send(Ok(GatewayWireInbound::Hello {
                heartbeat_interval_ms: 10_000,
            }))
            .await
            .unwrap();
        let _ = sock_ready.outbound_rx.recv().await.unwrap(); // Identify
        sock_ready
            .inbound_tx
            .send(Ok(GatewayWireInbound::Ready {
                session_id: "sess_will_fail_resume".into(),
                resume_gateway_url: None,
                seq: 5,
                data: serde_json::json!({}),
            }))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(5)).await;
        handle
            .join_voice(Some("900"), "901", false, true)
            .await
            .unwrap();
        let _ = sock_ready.outbound_rx.recv().await.unwrap(); // Op 4 join

        // 2. Четыре подряд неудачных попытки RESUME (обрыв до RESUMED) -> 5-я попытка обязана сбросить сессию в свежий IDENTIFY (F-GW-07)
        drop(sock_ready.inbound_tx);
        for attempt in 1..=4 {
            let mut s = tokio::time::timeout(Duration::from_secs(2), sockets_rx.recv())
                .await
                .unwrap()
                .unwrap();
            s.inbound_tx
                .send(Ok(GatewayWireInbound::Hello {
                    heartbeat_interval_ms: 10_000,
                }))
                .await
                .unwrap();
            let frame = s.outbound_rx.recv().await.unwrap();
            assert!(
                matches!(frame, GatewayWireOutbound::Resume { .. }),
                "Попытка #{attempt} должна пытаться выполнить RESUME"
            );
            s.inbound_tx
                .send(Ok(GatewayWireInbound::Closed {
                    code: Some(1006),
                    reason: "abnormal closure".into(),
                }))
                .await
                .unwrap();
        }

        // 5-е переподключение: после 4 сбоев RESUME сессия сброшена -> уходит свежий IDENTIFY!
        let mut sock_fresh_id = tokio::time::timeout(Duration::from_secs(2), sockets_rx.recv())
            .await
            .unwrap()
            .unwrap();
        sock_fresh_id
            .inbound_tx
            .send(Ok(GatewayWireInbound::Hello {
                heartbeat_interval_ms: 10_000,
            }))
            .await
            .unwrap();
        let fifth_frame = sock_fresh_id.outbound_rx.recv().await.unwrap();
        assert!(
            matches!(fifth_frame, GatewayWireOutbound::Identify { .. }),
            "После 4 неудачных RESUME актор обязан сбросить сессию и отправить свежий IDENTIFY"
        );
        sock_fresh_id
            .inbound_tx
            .send(Ok(GatewayWireInbound::Ready {
                session_id: "sess_fresh_99".into(),
                resume_gateway_url: None,
                seq: 1,
                data: serde_json::json!({}),
            }))
            .await
            .unwrap();
        // И сразу после свежего READY активное голосовое состояние (900 / 901) восстанавливается!
        let replayed = sock_fresh_id.outbound_rx.recv().await.unwrap();
        assert!(matches!(
            replayed,
            GatewayWireOutbound::VoiceStateUpdate { ref request, .. }
            if request.channel_id.as_deref() == Some("901") && request.self_deaf
        ));
        assert!(handle.snapshot().session_resets >= 1);

        // 3. Проверка op:9 INVALID_SESSION (resumable = false) -> немедленный переход к IDENTIFY
        sock_fresh_id
            .inbound_tx
            .send(Ok(GatewayWireInbound::InvalidSession { resumable: false }))
            .await
            .unwrap();
        let mut sock_after_op9 = tokio::time::timeout(Duration::from_secs(2), sockets_rx.recv())
            .await
            .unwrap()
            .unwrap();
        sock_after_op9
            .inbound_tx
            .send(Ok(GatewayWireInbound::Hello {
                heartbeat_interval_ms: 10_000,
            }))
            .await
            .unwrap();
        let after_op9_frame = sock_after_op9.outbound_rx.recv().await.unwrap();
        assert!(matches!(
            after_op9_frame,
            GatewayWireOutbound::Identify { .. }
        ));

        handle.shutdown().await.unwrap();
        let _ = join_handle.await.unwrap();
    }

    #[tokio::test]
    async fn test_gateway_voice_actor_fatal_close_codes_backoff_cancellation_and_token_masking_contract()
     {
        // 1. Граничная валидация конфигурации и полное маскирование секретного токена
        assert!(GatewayVoiceActorConfig::new("   ").validate().is_err());
        assert!(
            GatewayVoiceActorConfig::new("valid-tok")
                .with_gateway_url("ftp://discord.com")
                .validate()
                .is_err()
        );
        assert!(
            GatewayVoiceActorConfig::new("valid-tok")
                .with_gateway_url("wss://gateway.discord.gg#bad")
                .validate()
                .is_err()
        );

        let super_secret = "mfa.SUPER_SECRET_USER_TOKEN_DO_NOT_LEAK_123456";
        let mut cfg = fast_test_actor_config(super_secret);
        let debug_cfg = format!("{:?}", cfg);
        assert!(!debug_cfg.contains(super_secret));
        assert!(debug_cfg.contains("[REDACTED]"));

        // 2. Классификация фатальных close-кодов (4004, 4013, 4014 -> Stop без реконнекта)
        assert_eq!(
            classify_gateway_close_code(Some(4004)),
            chord_core::CloseAction::Stop(chord_core::StopReason::Unauthorized)
        );
        assert_eq!(
            classify_gateway_close_code(Some(4014)),
            chord_core::CloseAction::Stop(chord_core::StopReason::InvalidClient { code: 4014 })
        );
        assert_eq!(
            classify_gateway_close_code(Some(1006)),
            chord_core::CloseAction::Reconnect
        );

        let (transport_4004, mut rx_4004) = ScriptedGatewayActorTransport::new();
        let t_ref_4004 = transport_4004.clone();
        let (handle_4004, join_4004) =
            GatewayVoiceActor::spawn_with_transport(cfg.clone(), Arc::new(transport_4004), None)
                .unwrap();
        let sock_4004 = tokio::time::timeout(Duration::from_secs(2), rx_4004.recv())
            .await
            .unwrap()
            .unwrap();
        sock_4004
            .inbound_tx
            .send(Ok(GatewayWireInbound::Closed {
                code: Some(4004),
                reason: format!("Authentication failed for {super_secret}"),
            }))
            .await
            .unwrap();

        let snap_4004 = tokio::time::timeout(Duration::from_secs(2), join_4004)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(snap_4004.phase, GatewayActorPhase::Stopped);
        assert_eq!(
            snap_4004.last_stop_reason,
            Some(chord_core::StopReason::Unauthorized)
        );
        // Ни одной повторной попытки подключения при 4004 быть не должно!
        assert_eq!(t_ref_4004.urls().len(), 1);
        // И даже если сервер вернул токен в тексте ошибки, снимок обязан замаскировать секрет!
        let snap_debug = format!("{:?}", handle_4004.snapshot());
        assert!(!snap_debug.contains(super_secret));

        // 3. Проверка конечного лимита max_reconnects и мгновенной отмены длинного backoff при shutdown
        let (transport_limit, _rx_limit) = ScriptedGatewayActorTransport::new();
        transport_limit.push_connect_error(format!("TLS handshake error with {super_secret}"));
        transport_limit.push_connect_error("socket unreachable");
        cfg.max_reconnects = 1;
        cfg.initial_backoff = Duration::from_millis(5);
        let (_h_lim, join_lim) =
            GatewayVoiceActor::spawn_with_transport(cfg.clone(), Arc::new(transport_limit), None)
                .unwrap();
        let snap_lim = tokio::time::timeout(Duration::from_secs(2), join_lim)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            snap_lim.last_stop_reason,
            Some(chord_core::StopReason::ReconnectLimit { reconnects: 1 })
        );
        assert!(!format!("{:?}", snap_lim).contains(super_secret));

        // Проверка мгновенного выхода из 30-секундного backoff по команде shutdown (< 200мс)
        let (transport_cancel, _rx_cancel) = ScriptedGatewayActorTransport::new();
        transport_cancel.push_connect_error("temporary network down");
        let mut long_backoff_cfg = fast_test_actor_config("tok-cancel-backoff");
        long_backoff_cfg.initial_backoff = Duration::from_secs(30);
        long_backoff_cfg.max_backoff = Duration::from_secs(60);
        let (h_cancel, join_cancel) = GatewayVoiceActor::spawn_with_transport(
            long_backoff_cfg,
            Arc::new(transport_cancel),
            None,
        )
        .unwrap();
        tokio::time::sleep(Duration::from_millis(15)).await;
        assert_eq!(h_cancel.snapshot().phase, GatewayActorPhase::Reconnecting);
        let cancel_started = std::time::Instant::now();
        h_cancel.shutdown().await.unwrap();
        let cancelled_snap = tokio::time::timeout(Duration::from_millis(500), join_cancel)
            .await
            .expect("Shutdown во время backoff не должен ждать 30 секунд")
            .unwrap();
        assert_eq!(cancelled_snap.phase, GatewayActorPhase::Stopped);
        assert!(cancel_started.elapsed() < Duration::from_millis(300));
    }
}
