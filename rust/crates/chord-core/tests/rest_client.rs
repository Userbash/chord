//! REST-фасад: auth + транспорт + 429/5xx + телеметрия (RW-12 / I1).
//!
//! **Провенанс**: `docs/re/rest-d02.md` §3 (база v9, 6 auth-заголовков, 200/204/304/404/429),
//! RFC 6585 §4 (429 + Retry-After), RFC 9110 §9.2.2 (идемпотентность), `waves/RW-12/TZ.md` §3.
//! Все сетевые кейсы — loopback-стабы и `tokio::time::timeout(2 c)`.

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use chord_core::rest::client::{
    ApiAuth, ApiAuthError, ApiClient, ApiConfig, ApiError, ApiRetryPolicy, AuthScheme,
    HeaderConfig, format_accept_language,
};
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::super_properties::SuperProperties;
use chord_core::rest::transport::{HttpClientConfig, HttpError, HttpRequest};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::{sleep, timeout};

const CASE_TIMEOUT: Duration = Duration::from_secs(2);

/// Позиция подстроки.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Прочитать запрос: заголовок + тело по `Content-Length`.
async fn read_request<S: AsyncRead + Unpin>(stream: &mut S) -> Vec<u8> {
    let mut buffer = Vec::new();
    let mut chunk = [0u8; 1024];
    loop {
        if let Some(head_end) = find(&buffer, b"\r\n\r\n") {
            let head = String::from_utf8_lossy(&buffer[..head_end]);
            let content_length = head
                .lines()
                .find_map(|line| {
                    let (name, value) = line.split_once(':')?;
                    name.eq_ignore_ascii_case("content-length")
                        .then(|| value.trim().parse::<usize>().ok())
                        .flatten()
                })
                .unwrap_or(0);
            if buffer.len() >= head_end + 4 + content_length {
                return buffer;
            }
        }
        let read = stream.read(&mut chunk).await.expect("чтение запроса");
        if read == 0 {
            return buffer;
        }
        buffer.extend_from_slice(&chunk[..read]);
    }
}

/// Последовательность ответов на последовательные соединения; возвращает порт и запросы.
async fn spawn_sequence(responses: Vec<Vec<u8>>) -> (u16, JoinHandle<Vec<Vec<u8>>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = tokio::spawn(async move {
        let mut requests = Vec::new();
        for response in responses {
            let (mut stream, _) = listener.accept().await.expect("accept");
            let request = read_request(&mut stream).await;
            stream.write_all(&response).await.expect("write");
            stream.shutdown().await.expect("shutdown");
            requests.push(request);
        }
        requests
    });
    (port, handle)
}

/// Стаб, который принимает соединение, читает запрос и молчит (для таймаута).
async fn spawn_silent() -> u16 {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let _request = read_request(&mut stream).await;
        sleep(Duration::from_secs(5)).await;
    });
    port
}

/// Сервер: первое соединение молчит дольше `read_timeout`, второе отвечает 200.
///
/// Нужен для проверки, что транспортный таймаут чтения повторяется для GET.
async fn spawn_delayed_then_ok(delay: Duration) -> (u16, JoinHandle<usize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = tokio::spawn(async move {
        let (mut first, _) = listener.accept().await.expect("accept#1");
        let _ = read_request(&mut first).await;
        // Первое соединение молчит; второе принимается сразу — иначе повторная попытка
        // упрётся в тот же read_timeout и тест не отличит «нет повтора» от «повтор не успел».
        tokio::spawn(async move {
            sleep(delay).await;
            let _ = first.write_all(&response("200 OK", "{}")).await;
            let _ = first.shutdown().await;
        });

        let (mut second, _) = listener.accept().await.expect("accept#2");
        let _ = read_request(&mut second).await;
        let _ = second.write_all(&response("200 OK", "{}")).await;
        let _ = second.shutdown().await;
        2
    });
    (port, handle)
}

/// Считающий молчащий сервер: соединения инкрементируют счётчик, приём закрывается по idle.
async fn spawn_counting_silent(idle: Duration) -> (u16, Arc<AtomicUsize>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let counter = Arc::new(AtomicUsize::new(0));
    let seen = Arc::clone(&counter);
    tokio::spawn(async move {
        // Держим соединения открытыми: иначе клиент увидит EOF (`Incomplete`), а не таймаут чтения.
        let mut held = Vec::new();
        while let Ok(Ok((mut stream, _))) = timeout(idle, listener.accept()).await {
            seen.fetch_add(1, Ordering::SeqCst);
            let _ = read_request(&mut stream).await;
            held.push(stream);
        }
    });
    (port, counter)
}

/// Ответ с телом и `Content-Length`.
fn response(status: &str, body: &str) -> Vec<u8> {
    format!(
        "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}",
        body.len()
    )
    .into_bytes()
}

fn auth() -> ApiAuth {
    ApiAuth {
        authorization: "Bot test-token".to_string(),
        super_properties: "sp-base64".to_string(),
        locale: "en-US".to_string(),
        timezone: "UTC".to_string(),
        debug_options: "debug".to_string(),
        accept_language: "en-US,en;q=0.9".to_string(),
    }
}

fn policy() -> ApiRetryPolicy {
    ApiRetryPolicy {
        max_attempts: 3,
        max_retry_after: Duration::from_secs(1),
        backoff: Duration::from_millis(1),
        attempt_timeout: Duration::from_secs(1),
    }
}

fn client_for(port: u16, auth: ApiAuth, retry: ApiRetryPolicy) -> ApiClient {
    ApiClient::new(ApiConfig {
        base: format!("http://127.0.0.1:{port}"),
        auth,
        http: HttpClientConfig::default(),
        retry,
    })
    .expect("клиент")
}

// ---------------------------------------------------------------------------
// Заголовки и тело
// ---------------------------------------------------------------------------

#[tokio::test]
async fn auth_headers_added_in_order() {
    let (port, server) = spawn_sequence(vec![response("200 OK", "{}")]).await;
    let client = client_for(port, auth(), policy());

    let result = timeout(CASE_TIMEOUT, client.get("/users/@me"))
        .await
        .expect("timeout")
        .expect("ответ");
    assert_eq!(result.status, 200);

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw[0]);
    let names = [
        "Authorization: Bot test-token",
        "X-Super-Properties: sp-base64",
        "X-Discord-Locale: en-US",
        "X-Discord-Timezone: UTC",
        "X-Debug-Options: debug",
        "Accept-Language: en-US,en;q=0.9",
    ];
    let mut previous = 0;
    for name in names {
        let position = text.find(name).unwrap_or_else(|| panic!("нет {name:?}"));
        assert!(position >= previous, "порядок нарушен: {name:?}");
        previous = position;
    }
    for name in [
        "Authorization:",
        "X-Super-Properties:",
        "X-Discord-Locale:",
        "X-Discord-Timezone:",
        "X-Debug-Options:",
        "Accept-Language:",
    ] {
        assert_eq!(
            text.matches(name).count(),
            1,
            "ожидался ровно один заголовок {name:?}"
        );
    }
}

#[tokio::test]
async fn explicit_auth_header_is_not_duplicated() {
    let (port, server) = spawn_sequence(vec![response("200 OK", "{}")]).await;
    let client = client_for(port, auth(), policy());
    let request = HttpRequest::new(HttpMethod::Get, "/x").header("Authorization", "Bot explicit");

    timeout(CASE_TIMEOUT, client.execute(&request))
        .await
        .expect("timeout")
        .expect("ответ");

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw[0]);
    assert_eq!(text.matches("Authorization:").count(), 1);
    assert!(text.contains("Authorization: Bot explicit"));
    assert!(!text.contains("Bot test-token"));
}

#[tokio::test]
async fn content_type_and_length_for_json_body() {
    let body = br#"{"content":"hi"}"#.to_vec();
    let (port, server) = spawn_sequence(vec![b"HTTP/1.1 204 No Content\r\n\r\n".to_vec()]).await;
    let client = client_for(port, auth(), policy());
    let request = HttpRequest::new(HttpMethod::Post, "/channels/1/messages").body(body.clone());

    let result = timeout(CASE_TIMEOUT, client.execute(&request))
        .await
        .expect("timeout")
        .expect("ответ");
    assert_eq!(result.status, 204);

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw[0]);
    assert!(text.contains("Content-Type: application/json\r\n"));
    assert!(text.contains(&format!("Content-Length: {}\r\n", body.len())));
    assert!(text.ends_with(std::str::from_utf8(&body).expect("utf8")));
}

// ---------------------------------------------------------------------------
// Телеметрия
// ---------------------------------------------------------------------------

#[tokio::test]
async fn telemetry_blocked_before_network() {
    // Порт 1 закрыт: если бы фасад попытался соединиться, была бы Connect-ошибка.
    let client = client_for(1, auth(), policy());
    for target in ["/science", "/beaker?x=1", "/track#fragment"] {
        let error = timeout(CASE_TIMEOUT, client.get(target))
            .await
            .expect("timeout")
            .expect_err("телеметрия обязана блокироваться");
        assert_eq!(error, ApiError::TelemetryBlocked, "target {target:?}");
    }
}

#[tokio::test]
async fn telemetry_lookalike_reaches_server() {
    let (port, server) = spawn_sequence(vec![response("200 OK", "{}")]).await;
    let client = client_for(port, auth(), policy());
    let result = timeout(CASE_TIMEOUT, client.get("/science%2Ffoo"))
        .await
        .expect("timeout")
        .expect("lookalike не телеметрия");
    assert_eq!(result.status, 200);
    timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
}

// ---------------------------------------------------------------------------
// 429 / 5xx / 4xx
// ---------------------------------------------------------------------------

#[tokio::test]
async fn retry_429_then_success_with_delay() {
    let limited = b"HTTP/1.1 429 Too Many Requests\r\nRetry-After: 0.01\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}".to_vec();
    let (port, server) = spawn_sequence(vec![limited, response("200 OK", "{}")]).await;
    let client = client_for(port, auth(), policy());

    let started = Instant::now();
    let result = timeout(CASE_TIMEOUT, client.get("/users/@me"))
        .await
        .expect("timeout")
        .expect("после 429");
    assert_eq!(result.status, 200);
    assert!(
        started.elapsed() >= Duration::from_millis(10),
        "пауза Retry-After не выдержана: {:?}",
        started.elapsed()
    );
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server")
            .len(),
        2
    );
}

#[tokio::test]
async fn retry_429_for_post() {
    let (port, server) = spawn_sequence(vec![
        response("429 Too Many Requests", "{}"),
        response("200 OK", "{}"),
    ])
    .await;
    let client = client_for(port, auth(), policy());
    let request = HttpRequest::new(HttpMethod::Post, "/x").body(b"{}".to_vec());

    timeout(CASE_TIMEOUT, client.execute(&request))
        .await
        .expect("timeout")
        .expect("429 повторяется и для POST");
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server")
            .len(),
        2
    );
}

#[tokio::test]
async fn retry_5xx_only_for_idempotent() {
    let (port, server) = spawn_sequence(vec![
        response("500 Internal Server Error", "{}"),
        response("200 OK", "{}"),
    ])
    .await;
    let client = client_for(port, auth(), policy());
    timeout(CASE_TIMEOUT, client.get("/x"))
        .await
        .expect("timeout")
        .expect("GET повторяется");
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server")
            .len(),
        2
    );

    let (port, server) = spawn_sequence(vec![response("500 Internal Server Error", "{}")]).await;
    let client = client_for(port, auth(), policy());
    let request = HttpRequest::new(HttpMethod::Post, "/x").body(b"{}".to_vec());
    let error = timeout(CASE_TIMEOUT, client.execute(&request))
        .await
        .expect("timeout")
        .expect_err("POST не повторяется");
    assert_eq!(
        error,
        ApiError::Server {
            status: 500,
            attempts: 1
        }
    );
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server")
            .len(),
        1
    );
}

#[tokio::test]
async fn no_retry_for_401_and_404() {
    let (port, server) = spawn_sequence(vec![response("401 Unauthorized", "{}")]).await;
    let client = client_for(port, auth(), policy());
    let error = timeout(CASE_TIMEOUT, client.get("/x"))
        .await
        .expect("timeout")
        .expect_err("401");
    assert_eq!(error, ApiError::Unauthorized);
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server")
            .len(),
        1
    );

    let (port, server) = spawn_sequence(vec![response("404 Not Found", "{}")]).await;
    let client = client_for(port, auth(), policy());
    let error = timeout(CASE_TIMEOUT, client.get("/x"))
        .await
        .expect("timeout")
        .expect_err("404");
    assert_eq!(error, ApiError::Client { status: 404 });
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server")
            .len(),
        1
    );
}

#[tokio::test]
async fn exhausted_429_returns_rate_limited() {
    let (port, server) = spawn_sequence(vec![
        response("429 Too Many Requests", "{}"),
        response("429 Too Many Requests", "{}"),
    ])
    .await;
    let retry = ApiRetryPolicy {
        max_attempts: 2,
        ..policy()
    };
    let client = client_for(port, auth(), retry);
    let error = timeout(CASE_TIMEOUT, client.get("/x"))
        .await
        .expect("timeout")
        .expect_err("429 исчерпан");
    assert!(matches!(
        error,
        ApiError::RateLimited {
            attempts: 2,
            retry_after: Duration::ZERO
        }
    ));
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server")
            .len(),
        2
    );
}

// ---------------------------------------------------------------------------
// Конфигурация и таймаут
// ---------------------------------------------------------------------------

#[test]
fn invalid_base_rejected() {
    for base in [
        "ftp://example.com",
        "https:///api",
        "https://example.com:abc/api",
    ] {
        let error = ApiClient::new(ApiConfig {
            base: base.to_string(),
            ..ApiConfig::default()
        })
        .expect_err("база обязана быть отклонена");
        assert!(
            matches!(error, ApiError::InvalidBase(_)),
            "{base:?}: {error:?}"
        );
    }
}

#[tokio::test]
async fn attempt_timeout_retries_idempotent_only() {
    let port = spawn_silent().await;
    let retry = ApiRetryPolicy {
        max_attempts: 2,
        backoff: Duration::from_millis(1),
        attempt_timeout: Duration::from_millis(20),
        ..policy()
    };
    let client = client_for(port, auth(), retry.clone());

    let error = timeout(CASE_TIMEOUT, client.get("/slow"))
        .await
        .expect("timeout")
        .expect_err("GET обязан упереться в таймаут");
    assert_eq!(error, ApiError::Timeout { attempts: 2 });

    let port = spawn_silent().await;
    let client = client_for(port, auth(), retry);
    let request = HttpRequest::new(HttpMethod::Post, "/slow").body(b"{}".to_vec());
    let error = timeout(CASE_TIMEOUT, client.execute(&request))
        .await
        .expect("timeout")
        .expect_err("POST не повторяется");
    assert_eq!(error, ApiError::Timeout { attempts: 1 });
}

// ---------------------------------------------------------------------------
// Ниты независимого ревью I1 (P2/P3): транспортный таймаут, Protocol, Debug
// ---------------------------------------------------------------------------

#[tokio::test]
async fn transport_read_timeout_is_retried_for_get() {
    let (port, server) = spawn_delayed_then_ok(Duration::from_millis(120)).await;
    let retry = ApiRetryPolicy {
        max_attempts: 2,
        backoff: Duration::from_millis(1),
        attempt_timeout: Duration::from_secs(1),
        ..policy()
    };
    let client = ApiClient::new(ApiConfig {
        base: format!("http://127.0.0.1:{port}"),
        auth: auth(),
        http: HttpClientConfig {
            read_timeout: Duration::from_millis(40),
            ..HttpClientConfig::default()
        },
        retry,
    })
    .expect("клиент");

    let result = timeout(CASE_TIMEOUT, client.get("/slow"))
        .await
        .expect("timeout")
        .expect("GET обязан повторить транспортный таймаут чтения");
    assert_eq!(result.status, 200);
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server"),
        2,
        "ожидались две попытки"
    );
}

#[tokio::test]
async fn transport_read_timeout_is_not_retried_for_post() {
    let (port, counter) = spawn_counting_silent(Duration::from_millis(500)).await;
    let retry = ApiRetryPolicy {
        max_attempts: 3,
        backoff: Duration::from_millis(1),
        attempt_timeout: Duration::from_secs(1),
        ..policy()
    };
    let client = ApiClient::new(ApiConfig {
        base: format!("http://127.0.0.1:{port}"),
        auth: auth(),
        http: HttpClientConfig {
            read_timeout: Duration::from_millis(40),
            ..HttpClientConfig::default()
        },
        retry,
    })
    .expect("клиент");

    let request = HttpRequest::new(HttpMethod::Post, "/slow").body(b"{}".to_vec());
    let error = timeout(CASE_TIMEOUT, client.execute(&request))
        .await
        .expect("timeout")
        .expect_err("POST не повторяется");
    assert!(
        matches!(error, ApiError::Transport(HttpError::Timeout(_))),
        "ожидался Transport(Timeout), получено {error:?}"
    );
    sleep(Duration::from_millis(100)).await;
    assert_eq!(
        counter.load(Ordering::SeqCst),
        1,
        "POST не должен повторяться после таймаута"
    );
}

#[tokio::test]
async fn protocol_error_maps_to_transport_without_retry() {
    let (port, server) =
        spawn_sequence(vec![b"HTTP/1.1 200 OK\r\nBroken-Header\r\n\r\n".to_vec()]).await;
    let client = client_for(port, auth(), policy());

    let error = timeout(CASE_TIMEOUT, client.get("/x"))
        .await
        .expect("timeout")
        .expect_err("битый ответ обязан быть ошибкой");
    assert!(
        matches!(error, ApiError::Transport(HttpError::Protocol(_))),
        "ожидался Transport(Protocol), получено {error:?}"
    );
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server")
            .len(),
        1,
        "Protocol не повторяется"
    );
}

#[test]
fn debug_redacts_authorization() {
    let config = ApiConfig {
        auth: auth(),
        ..ApiConfig::default()
    };
    let text = format!("{config:?}");
    assert!(
        !text.contains("Bot test-token"),
        "токен утёк в Debug ApiConfig: {text}"
    );
    assert!(
        text.contains("<redacted>"),
        "ожидалась маскировка authorization: {text}"
    );

    let client = ApiClient::new(config).expect("клиент");
    let text = format!("{client:?}");
    assert!(
        !text.contains("Bot test-token"),
        "токен утёк в Debug ApiClient: {text}"
    );
}

// ---------------------------------------------------------------------------
// NET-C1 (RW-39): ApiAuth из токена, redaction запроса
// ---------------------------------------------------------------------------

/// Супер-свойства с непустым base64 (значения не важны — важна связка с заголовком).
fn super_properties() -> SuperProperties {
    SuperProperties {
        os: "Linux".to_string(),
        browser: "Discord Client".to_string(),
        release_channel: "stable".to_string(),
        client_version: "0.1.0".to_string(),
        ..SuperProperties::default()
    }
}

const USER_TOKEN: &str = "super-secret-user-token";

#[test]
fn from_token_builds_six_non_empty_headers() {
    let sp = super_properties();
    let auth = ApiAuth::from_token(USER_TOKEN, &sp, "ru", "Asia/Yekaterinburg")
        .expect("валидные входные данные");

    let pairs = auth.pairs();
    assert_eq!(
        pairs.map(|(name, _)| name),
        [
            "Authorization",
            "X-Super-Properties",
            "X-Discord-Locale",
            "X-Discord-Timezone",
            "X-Debug-Options",
            "Accept-Language",
        ],
        "порядок заголовков — контракт живого захвата"
    );
    for (name, value) in pairs {
        assert!(!value.is_empty(), "заголовок {name} пуст");
    }
    assert_eq!(auth.authorization, USER_TOKEN);
    assert_eq!(auth.super_properties, sp.to_base64());
    assert_eq!(auth.locale, "ru");
    assert_eq!(auth.timezone, "Asia/Yekaterinburg");
    assert_eq!(
        auth.debug_options, "bugReporterEnabled",
        "значение из живого захвата D-02"
    );
}

#[test]
fn from_token_trims_token_and_rejects_empty_inputs() {
    let sp = super_properties();
    let auth = ApiAuth::from_token("  token-with-spaces  ", &sp, "en-US", "UTC")
        .expect("пробелы по краям обрезаются");
    assert_eq!(auth.authorization, "token-with-spaces");

    assert_eq!(
        ApiAuth::from_token("", &sp, "ru", "UTC").expect_err("пустой токен"),
        ApiAuthError::EmptyToken
    );
    assert_eq!(
        ApiAuth::from_token("   ", &sp, "ru", "UTC").expect_err("пробельный токен"),
        ApiAuthError::EmptyToken
    );
    assert_eq!(
        ApiAuth::from_token(USER_TOKEN, &sp, "", "UTC").expect_err("пустая локаль"),
        ApiAuthError::EmptyLocale
    );
    assert_eq!(
        ApiAuth::from_token(USER_TOKEN, &sp, "ru", "").expect_err("пустая таймзона"),
        ApiAuthError::EmptyTimezone
    );
}

#[test]
fn from_token_scheme_is_parameterized() {
    let sp = super_properties();

    let bot = ApiAuth::from_token_with_scheme("abc", AuthScheme::Bot, &sp, "en-US", "UTC")
        .expect("bot-схема");
    assert_eq!(bot.authorization, "Bot abc");

    let bearer = ApiAuth::from_token_with_scheme("abc", AuthScheme::Bearer, &sp, "en-US", "UTC")
        .expect("bearer-схема");
    assert_eq!(bearer.authorization, "Bearer abc");

    // TODO(NET-A): схема user-токена в захвате санитизирована (U-06), по умолчанию — сырой
    // токен (статическая карта клиента). После подтверждения схема фиксируется здесь.
    assert_eq!(AuthScheme::default(), AuthScheme::User);
}

#[test]
fn from_token_debug_hides_token() {
    let auth = ApiAuth::from_token(USER_TOKEN, &super_properties(), "ru", "UTC")
        .expect("валидные входные данные");
    let text = format!("{auth:?}");
    assert!(!text.contains(USER_TOKEN), "токен утёк в Debug: {text}");
    assert!(text.contains("<redacted>"), "ожидалась маскировка: {text}");
}

#[test]
fn http_request_debug_redacts_authorization_and_cookie() {
    let request = HttpRequest::new(HttpMethod::Get, "/users/@me")
        .header("Authorization", USER_TOKEN)
        .header("Cookie", "session=secret-cookie")
        .header("X-Super-Properties", "sp-base64");

    let text = format!("{request:?}");
    assert!(!text.contains(USER_TOKEN), "Authorization утёк: {text}");
    assert!(!text.contains("secret-cookie"), "Cookie утёк: {text}");
    assert_eq!(
        text.matches("<redacted>").count(),
        2,
        "оба секретных заголовка маскируются: {text}"
    );
    assert!(
        text.contains("X-Super-Properties") && text.contains("sp-base64"),
        "несекретные заголовки остаются видимыми: {text}"
    );
}

#[tokio::test]
async fn from_token_headers_reach_network() {
    let (port, server) = spawn_sequence(vec![response("200 OK", "{}")]).await;
    let auth = ApiAuth::from_token(USER_TOKEN, &super_properties(), "ru", "Asia/Yekaterinburg")
        .expect("валидные входные данные");
    let client = client_for(port, auth, policy());

    timeout(CASE_TIMEOUT, client.get("/users/@me"))
        .await
        .expect("timeout")
        .expect("ответ");

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw[0]);
    assert!(text.contains("Authorization: super-secret-user-token"));
    let head = text.split("\r\n\r\n").next().expect("голова запроса");
    for name in [
        "Authorization",
        "X-Super-Properties",
        "X-Discord-Locale",
        "X-Discord-Timezone",
        "X-Debug-Options",
        "Accept-Language",
    ] {
        let line = head
            .lines()
            .find(|line| {
                line.split_once(':')
                    .is_some_and(|(field, _)| field.eq_ignore_ascii_case(name))
            })
            .unwrap_or_else(|| panic!("нет заголовка {name}: {text}"));
        let (_, value) = line.split_once(':').expect("пара имя:значение");
        assert!(!value.trim().is_empty(), "заголовок {name} пуст: {text}");
    }
}

// ---------------------------------------------------------------------------
// RW-39 / NET-F1 (NF-01): POST с JSON-телом и одноразовый POST для /auth/*
// ---------------------------------------------------------------------------

#[tokio::test]
async fn post_sends_json_body_with_auth_headers_and_no_duplicate() {
    let (port, server) = spawn_sequence(vec![response("200 OK", r#"{"token":"t"}"#)]).await;
    let client = client_for(port, auth(), policy());

    let body = br#"{"login":"user@example.test","password":"secret"}"#;
    let result = timeout(CASE_TIMEOUT, client.post("/auth/login", body))
        .await
        .expect("timeout")
        .expect("ответ POST");
    assert_eq!(result.status, 200);
    assert_eq!(result.body, br#"{"token":"t"}"#.to_vec());

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    assert_eq!(raw.len(), 1, "POST делает одну попытку при 200");
    let text = String::from_utf8_lossy(&raw[0]);
    assert!(
        text.starts_with("POST /auth/login HTTP/1.1"),
        "метод/путь: {text}"
    );
    assert!(
        text.contains("Content-Type: application/json\r\n"),
        "JSON-тело объявляет content-type: {text}"
    );
    assert!(
        text.contains(&format!("Content-Length: {}\r\n", body.len())),
        "длина тела: {text}"
    );
    assert!(
        text.ends_with(std::str::from_utf8(body).expect("utf8")),
        "тело доехало целиком: {text}"
    );
    assert!(
        text.contains("Authorization: Bot test-token"),
        "auth-заголовки те же, что у GET: {text}"
    );
    assert_eq!(
        text.matches("Authorization:").count(),
        1,
        "auth-заголовок не дублируется"
    );
}

#[tokio::test]
async fn post_classifies_client_error_like_execute() {
    let (port, server) = spawn_sequence(vec![response("401 Unauthorized", "{}")]).await;
    let client = client_for(port, auth(), policy());

    let error = timeout(CASE_TIMEOUT, client.post("/auth/login", b"{}"))
        .await
        .expect("timeout")
        .expect_err("401 — ошибка");
    assert_eq!(error, ApiError::Unauthorized);
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server")
            .len(),
        1
    );
}

#[tokio::test]
async fn post_once_returns_4xx_body_with_extra_headers_and_never_retries() {
    // 400 + тело captcha: для /auth/login это не «ошибка транспорта», а контракт.
    let captcha =
        r#"{"captcha_key":"site-key","captcha_service":"hcaptcha","captcha_rqtoken":"rq"}"#;
    let (port, server) = spawn_sequence(vec![response("400 Bad Request", captcha)]).await;
    let client = client_for(port, auth(), policy());

    let result = timeout(
        CASE_TIMEOUT,
        client.post_once(
            "/auth/login",
            br#"{"login":"u","password":"p"}"#,
            &[
                ("authorization", ""),
                ("X-Captcha-Key", "captcha-answer"),
                ("X-Captcha-Rqtoken", "rq"),
            ],
        ),
    )
    .await
    .expect("timeout")
    .expect("post_once отдаёт 400 с телом");
    assert_eq!(result.status, 400);
    assert_eq!(result.body, captcha.as_bytes().to_vec());

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    assert_eq!(raw.len(), 1, "post_once не повторяет запрос");
    let text = String::from_utf8_lossy(&raw[0]);
    assert!(text.starts_with("POST /auth/login HTTP/1.1"), "{text}");
    assert!(text.contains("X-Captcha-Key: captcha-answer\r\n"), "{text}");
    assert!(text.contains("X-Captcha-Rqtoken: rq\r\n"), "{text}");
    // Явный пустой authorization (A-03) не заменяется auth-шапкой клиента.
    assert_eq!(
        text.matches("authorization:").count(),
        1,
        "ровно один authorization: {text}"
    );
    assert!(
        !text
            .to_ascii_lowercase()
            .contains("authorization: bot test-token"),
        "пустой authorization перекрывает токен: {text}"
    );
}

#[tokio::test]
async fn post_once_returns_5xx_instead_of_retrying() {
    let (port, server) = spawn_sequence(vec![response(
        "500 Internal Server Error",
        r#"{"message":"boom"}"#,
    )])
    .await;
    let client = client_for(port, auth(), policy());

    let result = timeout(CASE_TIMEOUT, client.post_once("/auth/login", b"{}", &[]))
        .await
        .expect("timeout")
        .expect("5xx как данные, без повторов");
    assert_eq!(result.status, 500);
    assert_eq!(
        timeout(CASE_TIMEOUT, server)
            .await
            .expect("server timeout")
            .expect("server")
            .len(),
        1,
        "post_once не повторяет 5xx"
    );
}

// ---------------------------------------------------------------------------
// BE-P2-02 (task-165): Accept-Language q-list и опциональные REST-заголовки
// ---------------------------------------------------------------------------

#[test]
fn format_accept_language_qlist_spec_examples() {
    // 3 языка из ТЗ
    assert_eq!(
        format_accept_language(&["ru", "ru-RU", "en-US"]),
        "ru,ru-RU;q=0.9,en-US;q=0.8"
    );

    // Пустой список -> дефолтный язык (en-US)
    assert_eq!(format_accept_language(&[]), "en-US");

    // Пустые/пробельные строки -> дефолтный язык (en-US)
    assert_eq!(format_accept_language(&["", "   "]), "en-US");

    // 1 язык -> без q
    assert_eq!(format_accept_language(&["ru"]), "ru");

    // 2 языка -> первый без q, второй с q=0.9
    assert_eq!(format_accept_language(&["en-US", "en"]), "en-US,en;q=0.9");

    // 10 языков: q убывает от 0.9 до 0.1
    let ten = ["l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9", "l10"];
    assert_eq!(
        format_accept_language(&ten),
        "l1,l2;q=0.9,l3;q=0.8,l4;q=0.7,l5;q=0.6,l6;q=0.5,l7;q=0.4,l8;q=0.3,l9;q=0.2,l10;q=0.1"
    );

    // 11 языков: не опускается ниже 0.1
    let eleven = [
        "l1", "l2", "l3", "l4", "l5", "l6", "l7", "l8", "l9", "l10", "l11",
    ];
    assert_eq!(
        format_accept_language(&eleven),
        "l1,l2;q=0.9,l3;q=0.8,l4;q=0.7,l5;q=0.6,l6;q=0.5,l7;q=0.4,l8;q=0.3,l9;q=0.2,l10;q=0.1,l11;q=0.1"
    );
}

#[test]
fn api_auth_from_token_with_locales_sets_accept_language_and_discord_locale() {
    let sp = super_properties();
    let auth = ApiAuth::from_token_with_locales(
        USER_TOKEN,
        &sp,
        &["ru", "ru-RU", "en-US"],
        "Europe/Moscow",
    )
    .expect("валидный auth с локалями");

    assert_eq!(auth.locale, "ru");
    assert_eq!(auth.accept_language, "ru,ru-RU;q=0.9,en-US;q=0.8");
    assert_eq!(auth.timezone, "Europe/Moscow");
    assert_eq!(auth.authorization, USER_TOKEN);
    assert_eq!(auth.debug_options, "bugReporterEnabled");
    assert_eq!(auth.super_properties, sp.to_base64());
}

#[tokio::test]
async fn optional_headers_sent_when_some_and_non_empty() {
    let (port, server) = spawn_sequence(vec![response("200 OK", "{}")]).await;
    let headers_config = HeaderConfig {
        fingerprint: Some("test-fingerprint-123".to_string()),
        installation_id: Some("test-installation-456".to_string()),
        routing_key: Some("test-routing-key-789".to_string()),
    };
    let client = client_for(port, auth(), policy()).with_headers(headers_config);

    let result = timeout(CASE_TIMEOUT, client.get("/users/@me"))
        .await
        .expect("timeout")
        .expect("ответ");
    assert_eq!(result.status, 200);

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw[0]);

    // Проверяем наличие всех 3 опциональных заголовков
    assert!(
        text.contains("X-Fingerprint: test-fingerprint-123\r\n"),
        "X-Fingerprint отсутствует: {text}"
    );
    assert!(
        text.contains("X-Installation-ID: test-installation-456\r\n"),
        "X-Installation-ID отсутствует: {text}"
    );
    assert!(
        text.contains("X-Routing-Key: test-routing-key-789\r\n"),
        "X-Routing-Key отсутствует: {text}"
    );

    // Проверяем, что все 6 базовых обязательных заголовков также на месте
    for base in [
        "Authorization: Bot test-token",
        "X-Super-Properties: sp-base64",
        "X-Discord-Locale: en-US",
        "X-Discord-Timezone: UTC",
        "X-Debug-Options: debug",
        "Accept-Language: en-US,en;q=0.9",
    ] {
        assert!(text.contains(base), "базовый заголовок отсутствует: {base}");
    }
}

#[tokio::test]
async fn optional_headers_omitted_when_none_or_empty_or_whitespace() {
    let (port, server) = spawn_sequence(vec![response("200 OK", "{}")]).await;
    let headers_config = HeaderConfig {
        fingerprint: Some("".to_string()),
        installation_id: Some("   ".to_string()),
        routing_key: None,
    };
    let client = client_for(port, auth(), policy()).with_headers(headers_config);

    let result = timeout(CASE_TIMEOUT, client.get("/users/@me"))
        .await
        .expect("timeout")
        .expect("ответ");
    assert_eq!(result.status, 200);

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw[0]);

    // Ни один из опциональных заголовков не должен уйти в сеть
    assert!(
        !text.contains("X-Fingerprint"),
        "пустой X-Fingerprint утёк в запрос: {text}"
    );
    assert!(
        !text.contains("X-Installation-ID"),
        "пробельный X-Installation-ID утёк в запрос: {text}"
    );
    assert!(
        !text.contains("X-Routing-Key"),
        "None X-Routing-Key утёк в запрос: {text}"
    );

    // Базовый набор из 6 обязательных заголовков остаётся неизменным
    assert_eq!(
        text.matches("Authorization:").count(),
        1,
        "ровно один Authorization"
    );
    assert_eq!(
        text.matches("X-Super-Properties:").count(),
        1,
        "ровно один X-Super-Properties"
    );
    assert_eq!(
        text.matches("X-Discord-Locale:").count(),
        1,
        "ровно один X-Discord-Locale"
    );
    assert_eq!(
        text.matches("X-Discord-Timezone:").count(),
        1,
        "ровно один X-Discord-Timezone"
    );
    assert_eq!(
        text.matches("X-Debug-Options:").count(),
        1,
        "ровно один X-Debug-Options"
    );
    assert_eq!(
        text.matches("Accept-Language:").count(),
        1,
        "ровно один Accept-Language"
    );
}

#[tokio::test]
async fn optional_headers_builder_methods_and_post_once() {
    let (port, server) = spawn_sequence(vec![response("200 OK", "{}")]).await;
    let client = client_for(port, auth(), policy())
        .with_fingerprint("builder-fp")
        .with_installation_id("builder-inst")
        .with_routing_key("builder-rk");

    assert_eq!(client.headers().fingerprint.as_deref(), Some("builder-fp"));
    assert_eq!(
        client.headers().installation_id.as_deref(),
        Some("builder-inst")
    );
    assert_eq!(client.headers().routing_key.as_deref(), Some("builder-rk"));

    let result = timeout(
        CASE_TIMEOUT,
        client.post_once("/auth/register", b"{}", &[("X-Fingerprint", "explicit-fp")]),
    )
    .await
    .expect("timeout")
    .expect("post_once ответ");
    assert_eq!(result.status, 200);

    let raw = timeout(CASE_TIMEOUT, server)
        .await
        .expect("server timeout")
        .expect("server");
    let text = String::from_utf8_lossy(&raw[0]);

    // Явный X-Fingerprint не должен дублироваться заголовком из конфигурации клиента
    assert_eq!(
        text.matches("X-Fingerprint:").count(),
        1,
        "ровно один X-Fingerprint: {text}"
    );
    assert!(
        text.contains("X-Fingerprint: explicit-fp"),
        "явный заголовок перекрывает: {text}"
    );
    assert!(
        text.contains("X-Installation-ID: builder-inst"),
        "X-Installation-ID дошёл: {text}"
    );
    assert!(
        text.contains("X-Routing-Key: builder-rk"),
        "X-Routing-Key дошёл: {text}"
    );
}
