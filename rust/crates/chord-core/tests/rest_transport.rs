//! HTTP/1.1-транспорт REST на локальных стабах (волна RW-10 / W10b).
//!
//! **Провенанс**:
//! - `docs/re/rest-d02.md` §3: база `https://discord.com/api/v9`, HTTP/1.1, `Content-Length`;
//! - TLS-стаб — самоподписанный сертификат `tests/data/rest-test-cert.der`
//!   (SAN `DNS:localhost, IP:127.0.0.1`), ключ `rest-test-key.der` (PKCS#8 DER);
//! - RFC 9112 §6/§7 — framing и chunked.
//!
//! Все сетевые кейсы — только `127.0.0.1` и `tokio::time::timeout(5 c)`.

use std::sync::Arc;
use std::time::Duration;

use chord_core::rest::http1::Http1Error;
use chord_core::rest::routes::HttpMethod;
use chord_core::rest::transport::{
    Endpoint, HttpClientConfig, HttpError, HttpRequest, HttpResponse, send, send_with,
};
use rustls::RootCertStore;
use rustls::pki_types::{CertificateDer, PrivateKeyDer, PrivatePkcs8KeyDer};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use tokio::time::timeout;
use tokio_rustls::TlsAcceptor;

const CASE_TIMEOUT: Duration = Duration::from_secs(5);
const CA_DER: &[u8] = include_bytes!("data/rest-test-ca.der");
const CERT_DER: &[u8] = include_bytes!("data/rest-test-cert.der");
const KEY_DER: &[u8] = include_bytes!("data/rest-test-key.der");

/// Позиция подстроки.
fn find(haystack: &[u8], needle: &[u8]) -> Option<usize> {
    haystack
        .windows(needle.len())
        .position(|window| window == needle)
}

/// Прочитать запрос целиком: заголовок + тело по `Content-Length`.
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

/// TCP-стаб: один запрос, фиксированный ответ; возвращает порт и прочитанный запрос.
async fn spawn_tcp_stub(response: Vec<u8>) -> (u16, JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let request = read_request(&mut stream).await;
        stream.write_all(&response).await.expect("write response");
        stream.shutdown().await.expect("shutdown");
        request
    });
    (port, handle)
}

/// TLS-стаб с самоподписанным сертификатом.
async fn spawn_tls_stub(response: Vec<u8>) -> (u16, JoinHandle<Vec<u8>>) {
    let cert = CertificateDer::from(CERT_DER.to_vec());
    let key = PrivateKeyDer::Pkcs8(PrivatePkcs8KeyDer::from(KEY_DER.to_vec()));
    let config = rustls::ServerConfig::builder()
        .with_no_client_auth()
        .with_single_cert(vec![cert], key)
        .expect("server config");
    let acceptor = TlsAcceptor::from(Arc::new(config));

    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = tokio::spawn(async move {
        let (stream, _) = listener.accept().await.expect("accept");
        let mut stream = acceptor.accept(stream).await.expect("tls accept");
        let request = read_request(&mut stream).await;
        stream.write_all(&response).await.expect("write response");
        stream.shutdown().await.expect("shutdown");
        request
    });
    (port, handle)
}

/// Конфигурация с единственным доверенным корнем — тестовым сертификатом.
fn test_config(max_body_bytes: usize) -> HttpClientConfig {
    let mut roots = RootCertStore::empty();
    roots
        .add(CertificateDer::from(CA_DER.to_vec()))
        .expect("add test CA");
    HttpClientConfig {
        roots,
        connect_timeout: Duration::from_secs(5),
        max_body_bytes,
        read_timeout: Duration::from_secs(5),
        write_timeout: Duration::from_secs(5),
    }
}

// ---------------------------------------------------------------------------
// Endpoint
// ---------------------------------------------------------------------------

#[test]
fn endpoint_parse_https_base() {
    let endpoint = Endpoint::parse("https://discord.com/api/v9").expect("parse");
    assert_eq!(endpoint.scheme, "https");
    assert_eq!(endpoint.host, "discord.com");
    assert_eq!(endpoint.port, 443);
    assert_eq!(endpoint.base_path, "/api/v9");
    assert_eq!(
        endpoint.target("/users/@me").expect("target"),
        "/api/v9/users/@me"
    );
    assert_eq!(endpoint.target("/").expect("root"), "/api/v9/");
}

#[test]
fn endpoint_parse_local_and_normalize() {
    let endpoint = Endpoint::parse("http://127.0.0.1:8080").expect("parse");
    assert_eq!(endpoint.scheme, "http");
    assert_eq!(endpoint.host, "127.0.0.1");
    assert_eq!(endpoint.port, 8080);
    assert_eq!(endpoint.base_path, "");
    assert_eq!(endpoint.target("/x").expect("target"), "/x");

    let endpoint = Endpoint::parse("https://example.com/api/v9/").expect("trailing slash");
    assert_eq!(endpoint.base_path, "/api/v9");
}

#[test]
fn endpoint_rejects_bad_base_and_target() {
    for base in [
        "ftp://example.com",
        "https:///api",
        "https://example.com:abc/api",
        "https://example.com/api?v=9",
        "https://example.com/api#frag",
    ] {
        assert!(
            Endpoint::parse(base).is_err(),
            "{base:?}: обязан быть InvalidBase/UnsupportedScheme"
        );
    }

    let endpoint = Endpoint::parse("https://example.com/api").expect("parse");
    for target in ["relative", "/x\r\nY: z", "/x y", "/x#frag"] {
        assert!(
            matches!(endpoint.target(target), Err(HttpError::InvalidTarget(_))),
            "{target:?}: обязан быть InvalidTarget"
        );
    }
}

// ---------------------------------------------------------------------------
// TCP
// ---------------------------------------------------------------------------

#[tokio::test]
async fn get_plain_tcp_with_auto_headers() {
    let response =
        b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 2\r\n\r\n{}"
            .to_vec();
    let (port, server) = spawn_tcp_stub(response).await;
    let base = format!("http://127.0.0.1:{port}/api/v9");
    let request = HttpRequest::new(HttpMethod::Get, "/users/@me");

    let result = timeout(
        CASE_TIMEOUT,
        send_with(&HttpClientConfig::default(), &base, &request),
    )
    .await
    .expect("timeout")
    .expect("response");

    assert_eq!(result.status, 200);
    assert_eq!(result.body, b"{}");
    assert_eq!(
        chord_core::rest::http1::header(&result.headers, "content-type"),
        Some("application/json")
    );

    let raw = server.await.expect("server");
    let text = String::from_utf8_lossy(&raw);
    assert!(
        text.starts_with("GET /api/v9/users/@me HTTP/1.1\r\n"),
        "request line: {text:?}"
    );
    assert!(text.contains(&format!("Host: 127.0.0.1:{port}\r\n")));
    assert!(text.contains("Connection: close\r\n"));
    assert!(
        !text.to_ascii_lowercase().contains("content-length"),
        "GET без тела"
    );
}

#[tokio::test]
async fn post_body_and_204() {
    let response = b"HTTP/1.1 204 No Content\r\n\r\n".to_vec();
    let (port, server) = spawn_tcp_stub(response).await;
    let base = format!("http://127.0.0.1:{port}");
    let body = br#"{"content":"hi"}"#.to_vec();
    let request = HttpRequest::new(HttpMethod::Post, "/channels/1/messages").body(body.clone());

    let result = timeout(CASE_TIMEOUT, send(&base, &request))
        .await
        .expect("timeout")
        .expect("response");
    assert_eq!(result.status, 204);
    assert!(result.body.is_empty());

    let raw = server.await.expect("server");
    let text = String::from_utf8_lossy(&raw);
    assert!(text.contains(&format!("Content-Length: {}\r\n", body.len())));
    assert!(text.ends_with(std::str::from_utf8(&body).expect("utf8")));
}

#[tokio::test]
async fn chunked_response() {
    let response =
        b"HTTP/1.1 200 OK\r\nTransfer-Encoding: chunked\r\n\r\n4\r\nWiki\r\n5\r\npedia\r\n0\r\n\r\n"
            .to_vec();
    let (port, _server) = spawn_tcp_stub(response).await;
    let base = format!("http://127.0.0.1:{port}");

    let result = timeout(
        CASE_TIMEOUT,
        send(&base, &HttpRequest::new(HttpMethod::Get, "/chunked")),
    )
    .await
    .expect("timeout")
    .expect("response");
    assert_eq!(result.status, 200);
    assert_eq!(result.body, b"Wikipedia");
}

#[tokio::test]
async fn read_until_close() {
    let response = b"HTTP/1.1 200 OK\r\nConnection: close\r\n\r\nhello".to_vec();
    let (port, _server) = spawn_tcp_stub(response).await;
    let base = format!("http://127.0.0.1:{port}");

    let result = timeout(
        CASE_TIMEOUT,
        send(&base, &HttpRequest::new(HttpMethod::Get, "/close")),
    )
    .await
    .expect("timeout")
    .expect("response");
    assert_eq!(result.body, b"hello");
}

#[tokio::test]
async fn read_timeout_is_enforced() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    let handle = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.expect("accept");
        let _ = read_request(&mut stream).await;
        tokio::time::sleep(Duration::from_millis(200)).await;
        let _ = stream
            .write_all(b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n")
            .await;
        let _ = stream.shutdown().await;
    });

    let mut config = test_config(1024);
    config.read_timeout = Duration::from_millis(40);
    let base = format!("http://127.0.0.1:{port}");
    let result = timeout(
        CASE_TIMEOUT,
        send_with(&config, &base, &HttpRequest::new(HttpMethod::Get, "/")),
    )
    .await
    .expect("внешний таймаут: send_with завис");
    assert!(
        matches!(result, Err(HttpError::Timeout(_))),
        "ожидался Timeout чтения, получено {result:?}"
    );
    handle.abort();
}

#[tokio::test]
async fn too_large_body_is_rejected_before_read() {
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 100\r\n\r\n".to_vec();
    let (port, _server) = spawn_tcp_stub(response).await;
    let base = format!("http://127.0.0.1:{port}");
    let config = HttpClientConfig {
        max_body_bytes: 10,
        ..HttpClientConfig::default()
    };

    let error = timeout(
        CASE_TIMEOUT,
        send_with(&config, &base, &HttpRequest::new(HttpMethod::Get, "/big")),
    )
    .await
    .expect("timeout")
    .expect_err("ответ больше лимита обязан быть ошибкой");
    assert!(
        matches!(error, HttpError::TooLarge(_)),
        "получено {error:?}"
    );
}

#[tokio::test]
async fn malformed_response_is_protocol_error() {
    let response = b"garbage\r\n\r\n".to_vec();
    let (port, _server) = spawn_tcp_stub(response).await;
    let base = format!("http://127.0.0.1:{port}");

    let error = timeout(
        CASE_TIMEOUT,
        send(&base, &HttpRequest::new(HttpMethod::Get, "/bad")),
    )
    .await
    .expect("timeout")
    .expect_err("мусор обязан быть ошибкой");
    assert!(
        matches!(
            error,
            HttpError::Protocol(Http1Error::UnsupportedVersion(_) | Http1Error::MalformedHead(_))
        ),
        "получено {error:?}"
    );
}

// ---------------------------------------------------------------------------
// TLS
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tls_with_custom_root_and_ip_san() {
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 2\r\n\r\nok".to_vec();
    let (port, server) = spawn_tls_stub(response).await;
    let base = format!("https://127.0.0.1:{port}/api/v9");

    let result = timeout(
        CASE_TIMEOUT,
        send_with(
            &test_config(1024),
            &base,
            &HttpRequest::new(HttpMethod::Get, "/users/@me"),
        ),
    )
    .await
    .expect("timeout")
    .expect("TLS-ответ");
    assert_eq!(result.status, 200);
    assert_eq!(result.body, b"ok");

    let raw = server.await.expect("server");
    assert!(String::from_utf8_lossy(&raw).contains("Host: 127.0.0.1:"));
}

#[tokio::test]
async fn tls_untrusted_root_fails() {
    let response = b"HTTP/1.1 200 OK\r\nContent-Length: 0\r\n\r\n".to_vec();
    let (port, _server) = spawn_tls_stub(response).await;
    let base = format!("https://127.0.0.1:{port}");

    let error = timeout(
        CASE_TIMEOUT,
        send(&base, &HttpRequest::new(HttpMethod::Get, "/")),
    )
    .await
    .expect("timeout")
    .expect_err("самоподписанный корень не должен доверяться по умолчанию");
    assert!(matches!(error, HttpError::Tls(_)), "получено {error:?}");
}

#[tokio::test]
async fn connect_refused_is_error_not_panic() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let port = listener.local_addr().expect("addr").port();
    drop(listener);

    let base = format!("http://127.0.0.1:{port}");
    let error = timeout(
        CASE_TIMEOUT,
        send(&base, &HttpRequest::new(HttpMethod::Get, "/")),
    )
    .await
    .expect("timeout")
    .expect_err("закрытый порт");
    assert!(
        matches!(error, HttpError::Connect(_) | HttpError::Io(_)),
        "получено {error:?}"
    );
}

/// Компиляционная проверка формы ответа (не сетевой кейс).
#[test]
fn response_shape_is_public() {
    let response = HttpResponse {
        status: 200,
        headers: vec![("X".to_string(), "y".to_string())],
        body: Vec::new(),
    };
    assert_eq!(response.status, 200);
}
