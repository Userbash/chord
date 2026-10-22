//! HTTP/1.1-транспорт REST: TCP/TLS, один запрос на соединение (RW-10, вариант A).
//!
//! Политика ретраев — [`super::retry`], статусы 429/401 — [`super::rate_limit`].
//! `Connection: close`: keep-alive появится отдельной волной, когда будет измерен выигрыш.
//!
//! Оракул: `docs/re/rest-d02.md` §3 (база `https://discord.com/api/v9`, HTTP/1.1,
//! `Content-Length`); TLS-стаб — самоподписанный сертификат
//! `tests/data/rest-test-cert.der` (SAN `DNS:localhost, IP:127.0.0.1`).

use std::net::IpAddr;
use std::sync::Arc;
use std::time::Duration;

use rustls::RootCertStore;
use rustls::pki_types::ServerName;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio_rustls::TlsConnector;

use super::http1::{
    self, ChunkedParse, HeadParse, Http1Error, content_length, is_chunked, parse_response_head,
};
use super::routes::HttpMethod;

/// Конфигурация клиента: корни TLS, таймаут connect, лимит тела ответа.
#[derive(Debug, Clone)]
pub struct HttpClientConfig {
    /// Доверенные корни (в тестах — самоподписанный сертификат стаба).
    pub roots: RootCertStore,
    /// Таймаут установления TCP/TLS-соединения.
    pub connect_timeout: Duration,
    /// Максимальный размер тела ответа в байтах.
    pub max_body_bytes: usize,
    /// Таймаут одного чтения из сокета (заголовки/тело).
    pub read_timeout: Duration,
    /// Таймаут записи запроса в сокет.
    pub write_timeout: Duration,
}

impl Default for HttpClientConfig {
    fn default() -> Self {
        let mut roots = RootCertStore::empty();
        roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
        Self {
            roots,
            connect_timeout: Duration::from_secs(10),
            max_body_bytes: 8 * 1024 * 1024,
            read_timeout: Duration::from_secs(30),
            write_timeout: Duration::from_secs(10),
        }
    }
}

/// Запрос: метод, target (путь с query), поля и тело.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpRequest {
    /// HTTP-метод.
    pub method: HttpMethod,
    /// Путь относительно базы (`/users/@me`) или с query.
    pub target: String,
    /// Поля в порядке отправки.
    pub headers: Vec<(String, String)>,
    /// Тело запроса (пусто — тела нет).
    pub body: Vec<u8>,
}

impl HttpRequest {
    /// Запрос без полей и тела.
    #[must_use]
    pub fn new(method: HttpMethod, target: impl Into<String>) -> Self {
        Self {
            method,
            target: target.into(),
            headers: Vec::new(),
            body: Vec::new(),
        }
    }

    /// Добавить поле (порядок сохраняется).
    #[must_use]
    pub fn header(mut self, name: impl Into<String>, value: impl Into<String>) -> Self {
        self.headers.push((name.into(), value.into()));
        self
    }

    /// Задать тело.
    #[must_use]
    pub fn body(mut self, body: impl Into<Vec<u8>>) -> Self {
        self.body = body.into();
        self
    }
}

/// Ответ: статус, поля и тело.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HttpResponse {
    /// HTTP-статус.
    pub status: u16,
    /// Поля ответа в порядке провода.
    pub headers: Vec<(String, String)>,
    /// Тело ответа.
    pub body: Vec<u8>,
}

/// Ошибка транспорта.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HttpError {
    /// База не разбирается (схема/хост/порт/путь).
    InvalidBase(String),
    /// Схема не `http`/`https`.
    UnsupportedScheme(String),
    /// target не начинается с `/` или содержит CR/LF.
    InvalidTarget(String),
    /// DNS/соединение не удалось.
    Connect(String),
    /// TLS-рукопожатие/проверка сертификата.
    Tls(String),
    /// Ошибка чтения/записи.
    Io(String),
    /// Нарушение HTTP/1.1-фрейминга.
    Protocol(Http1Error),
    /// Тело ответа больше `max_body_bytes`.
    TooLarge(String),
    /// Истёк таймаут DNS/чтения/записи.
    Timeout(String),
}

/// Разобранная база: схема, хост, порт, базовый путь.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Endpoint {
    /// `http` или `https`.
    pub scheme: String,
    /// Хост (DNS-имя или IP-литерал без квадратных скобок).
    pub host: String,
    /// Порт (по умолчанию 80/443).
    pub port: u16,
    /// Базовый путь без хвостового `/` (пусто для корня).
    pub base_path: String,
}

impl Endpoint {
    /// Разбор базы вида `https://discord.com/api/v9`.
    pub fn parse(base: &str) -> Result<Self, HttpError> {
        let (scheme, rest) = base
            .split_once("://")
            .ok_or_else(|| HttpError::InvalidBase(base.to_string()))?;
        let scheme = scheme.to_ascii_lowercase();
        if scheme != "http" && scheme != "https" {
            return Err(HttpError::UnsupportedScheme(scheme));
        }
        if rest.is_empty() || rest.contains('?') || rest.contains('#') {
            return Err(HttpError::InvalidBase(base.to_string()));
        }

        let (authority, path) = match rest.find('/') {
            Some(index) => (&rest[..index], &rest[index..]),
            None => (rest, ""),
        };
        if authority.is_empty() || authority.contains('@') {
            return Err(HttpError::InvalidBase(base.to_string()));
        }

        let default_port = if scheme == "https" { 443 } else { 80 };
        let (host, port) = parse_authority(authority, default_port)?;
        let base_path = if path.is_empty() || path == "/" {
            String::new()
        } else {
            path.trim_end_matches('/').to_string()
        };

        Ok(Self {
            scheme,
            host,
            port,
            base_path,
        })
    }

    /// Target для относительного пути: `base_path + path` (path обязан начинаться с `/`).
    pub fn target(&self, path: &str) -> Result<String, HttpError> {
        if !path.starts_with('/')
            || path
                .bytes()
                .any(|byte| !(0x21..=0x7e).contains(&byte) || byte == b'#')
        {
            return Err(HttpError::InvalidTarget(path.to_string()));
        }
        if self.base_path.is_empty() {
            Ok(path.to_string())
        } else {
            Ok(format!("{}{path}", self.base_path))
        }
    }

    /// Значение `Host`: с портом, если он не по умолчанию; IPv6 — в скобках.
    fn host_header(&self) -> String {
        let host = if self.host.contains(':') {
            format!("[{}]", self.host)
        } else {
            self.host.clone()
        };
        let default_port = if self.scheme == "https" { 443 } else { 80 };
        if self.port == default_port {
            host
        } else {
            format!("{host}:{}", self.port)
        }
    }
}

/// Разбор authority: `host`, `host:port`, `[ipv6]`, `[ipv6]:port`.
fn parse_authority(authority: &str, default_port: u16) -> Result<(String, u16), HttpError> {
    let invalid = || HttpError::InvalidBase(authority.to_string());

    if let Some(rest) = authority.strip_prefix('[') {
        let (host, tail) = rest.split_once(']').ok_or_else(invalid)?;
        if host.is_empty() || host.parse::<IpAddr>().is_err() {
            return Err(invalid());
        }
        let port = match tail {
            "" => default_port,
            _ => {
                let raw = tail.strip_prefix(':').ok_or_else(invalid)?;
                raw.parse::<u16>().map_err(|_| invalid())?
            }
        };
        if port == 0 {
            return Err(invalid());
        }
        return Ok((host.to_string(), port));
    }

    let mut parts = authority.split(':');
    let host = parts.next().unwrap_or_default();
    let port = match parts.next() {
        Some(raw) => raw.parse::<u16>().map_err(|_| invalid())?,
        None => default_port,
    };
    if host.is_empty() || parts.next().is_some() || port == 0 {
        return Err(invalid());
    }
    Ok((host.to_string(), port))
}

/// Один запрос с конфигурацией по умолчанию (webpki-roots).
pub async fn send(base: &str, request: &HttpRequest) -> Result<HttpResponse, HttpError> {
    send_with(&HttpClientConfig::default(), base, request).await
}

/// Один запрос с явной конфигурацией (в тестах — свой корень TLS).
pub async fn send_with(
    config: &HttpClientConfig,
    base: &str,
    request: &HttpRequest,
) -> Result<HttpResponse, HttpError> {
    let endpoint = Endpoint::parse(base)?;
    let target = endpoint.target(&request.target)?;

    let mut headers = request.headers.clone();
    if !has_header(&headers, "host") {
        headers.push(("Host".to_string(), endpoint.host_header()));
    }
    if !has_header(&headers, "connection") {
        headers.push(("Connection".to_string(), "close".to_string()));
    }

    let bytes = http1::encode_request(request.method, &target, &headers, &request.body)
        .map_err(HttpError::Protocol)?;

    let tcp = connect_tcp(&endpoint, config.connect_timeout).await?;
    if endpoint.scheme == "https" {
        let tls = tokio::time::timeout(config.connect_timeout, tls_connect(tcp, &endpoint, config))
            .await
            .map_err(|_| HttpError::Timeout("TLS-рукопожатие".to_string()))??;
        exchange(
            tls,
            &bytes,
            config.max_body_bytes,
            config.read_timeout,
            config.write_timeout,
        )
        .await
    } else {
        exchange(
            tcp,
            &bytes,
            config.max_body_bytes,
            config.read_timeout,
            config.write_timeout,
        )
        .await
    }
}

/// Есть ли поле без учёта регистра.
fn has_header(headers: &[(String, String)], name: &str) -> bool {
    headers
        .iter()
        .any(|(key, _)| key.eq_ignore_ascii_case(name))
}

/// TCP-соединение с таймаутом; перебирает все адреса DNS.
async fn connect_tcp(
    endpoint: &Endpoint,
    connect_timeout: Duration,
) -> Result<TcpStream, HttpError> {
    let addresses: Vec<_> = tokio::time::timeout(
        connect_timeout,
        tokio::net::lookup_host((endpoint.host.as_str(), endpoint.port)),
    )
    .await
    .map_err(|_| HttpError::Timeout(format!("DNS {}", endpoint.host)))?
    .map_err(|error| HttpError::Connect(format!("DNS {}: {error}", endpoint.host)))?
    .collect();
    if addresses.is_empty() {
        return Err(HttpError::Connect(format!(
            "DNS {}: пустой ответ",
            endpoint.host
        )));
    }

    let mut last = String::from("нет попыток");
    for address in addresses {
        match tokio::time::timeout(connect_timeout, TcpStream::connect(address)).await {
            Ok(Ok(stream)) => return Ok(stream),
            Ok(Err(error)) => last = format!("{address}: {error}"),
            Err(_) => last = format!("{address}: таймаут {connect_timeout:?}"),
        }
    }
    Err(HttpError::Connect(last))
}

/// TLS поверх TCP с проверкой сертификата по корням конфигурации.
async fn tls_connect(
    tcp: TcpStream,
    endpoint: &Endpoint,
    config: &HttpClientConfig,
) -> Result<tokio_rustls::client::TlsStream<TcpStream>, HttpError> {
    let client = rustls::ClientConfig::builder()
        .with_root_certificates(config.roots.clone())
        .with_no_client_auth();
    let connector = TlsConnector::from(Arc::new(client));

    let server_name = if let Ok(ip) = endpoint.host.parse::<IpAddr>() {
        ServerName::IpAddress(ip.into())
    } else {
        ServerName::try_from(endpoint.host.clone())
            .map_err(|error| HttpError::Tls(format!("ServerName {}: {error}", endpoint.host)))?
    };

    connector
        .connect(server_name, tcp)
        .await
        .map_err(|error| HttpError::Tls(error.to_string()))
}

/// Обмен: запись запроса и чтение ответа по framing-правилам.
async fn exchange<S>(
    mut stream: S,
    request: &[u8],
    max_body: usize,
    read_timeout: Duration,
    write_timeout: Duration,
) -> Result<HttpResponse, HttpError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    tokio::time::timeout(write_timeout, async {
        stream
            .write_all(request)
            .await
            .map_err(|error| HttpError::Io(format!("запись: {error}")))?;
        stream
            .flush()
            .await
            .map_err(|error| HttpError::Io(format!("flush: {error}")))
    })
    .await
    .map_err(|_| HttpError::Timeout("запись запроса".to_string()))??;

    let mut buffer = Vec::with_capacity(4096);
    let (status, headers, consumed) = loop {
        match parse_response_head(&buffer).map_err(HttpError::Protocol)? {
            HeadParse::Complete {
                status,
                headers,
                consumed,
            } => break (status, headers, consumed),
            HeadParse::Incomplete => {
                if read_more(&mut stream, &mut buffer, read_timeout).await? == 0 {
                    return Err(HttpError::Protocol(Http1Error::Incomplete));
                }
            }
        }
    };

    if status == 204 || status == 304 {
        return Ok(HttpResponse {
            status,
            headers,
            body: Vec::new(),
        });
    }

    if is_chunked(&headers).map_err(HttpError::Protocol)? {
        let mut body_bytes = buffer[consumed..].to_vec();
        // Запас на служебные байты chunked; защищает от потока миллионов мелких чанков.
        let wire_limit = max_body.saturating_mul(8).saturating_add(64 * 1024);
        loop {
            if body_bytes.len() > wire_limit {
                return Err(HttpError::TooLarge(format!(
                    "chunked wire > {wire_limit} при body {max_body}"
                )));
            }
            match http1::decode_chunked(&body_bytes, max_body).map_err(HttpError::Protocol)? {
                ChunkedParse::Complete { body, .. } => {
                    return Ok(HttpResponse {
                        status,
                        headers,
                        body,
                    });
                }
                ChunkedParse::Incomplete => {
                    if read_more(&mut stream, &mut body_bytes, read_timeout).await? == 0 {
                        return Err(HttpError::Protocol(Http1Error::Incomplete));
                    }
                }
            }
        }
    }

    if let Some(length) = content_length(&headers).map_err(HttpError::Protocol)? {
        if length > max_body {
            return Err(HttpError::TooLarge(format!(
                "Content-Length {length} > {max_body}"
            )));
        }
        while buffer.len() < consumed + length {
            if read_more(&mut stream, &mut buffer, read_timeout).await? == 0 {
                return Err(HttpError::Protocol(Http1Error::MalformedHead(
                    "тело короче Content-Length".to_string(),
                )));
            }
        }
        return Ok(HttpResponse {
            status,
            headers,
            body: buffer[consumed..consumed + length].to_vec(),
        });
    }

    let mut body = buffer[consumed..].to_vec();
    if body.len() > max_body {
        return Err(HttpError::TooLarge(format!("тело > {max_body}")));
    }
    loop {
        if read_more(&mut stream, &mut body, read_timeout).await? == 0 {
            return Ok(HttpResponse {
                status,
                headers,
                body,
            });
        }
        if body.len() > max_body {
            return Err(HttpError::TooLarge(format!("тело > {max_body}")));
        }
    }
}

/// Дочитать в буфер; 0 — EOF.
async fn read_more<S>(
    stream: &mut S,
    buffer: &mut Vec<u8>,
    read_timeout: Duration,
) -> Result<usize, HttpError>
where
    S: AsyncRead + Unpin,
{
    let mut chunk = [0u8; 8192];
    let read = tokio::time::timeout(read_timeout, stream.read(&mut chunk))
        .await
        .map_err(|_| HttpError::Timeout("чтение ответа".to_string()))?
        .map_err(|error| HttpError::Io(format!("чтение: {error}")))?;
    buffer.extend_from_slice(&chunk[..read]);
    Ok(read)
}
