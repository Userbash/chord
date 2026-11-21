#![forbid(unsafe_code)]

//! Clean lightweight `tokio-tungstenite` WebSocket transport with `rustls` (ring provider)
//! and `webpki-roots` TLS support for Discord Gateway (`STAGE-05.1.1`).
//!
//! Provides a dedicated, bounded, low-overhead WebSocket transport (`GatewayTransport`)
//! following the Serein & Chord "Thin Bridge" architecture:
//! - Explicit `rustls::crypto::ring::default_provider()` + `webpki_roots::TLS_SERVER_ROOTS`
//!   wrapped in `tokio_tungstenite::Connector::Rustls`.
//! - Bounded `WebSocketConfig` (`64 MiB` max message/frame wire capacity, `64 KiB` max write buffer).
//! - Connect timeout (`15s` default) and send timeout (`5s` default) protection against silent stalls.
//! - Automatic RFC 6455 `Ping` -> `Pong` frame responsiveness and structured `Close` outcomes.
//! - Lock-free transport telemetry (`GatewayTransportMetrics`).

use std::fmt;
use std::sync::Arc;
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tokio::io::{AsyncRead, AsyncWrite};
use tokio::net::TcpStream;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::error::Error as TungsteniteError;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message, WebSocketConfig};
use tokio_tungstenite::{
    client_async_with_config, connect_async_tls_with_config, Connector, MaybeTlsStream,
    WebSocketStream,
};

use crate::ws::{GatewayUrl, GatewayUrlError};

/// Maximum incoming Discord Gateway message/frame wire size: `64 MiB` (`67_108_864` bytes).
pub const DEFAULT_MAX_GATEWAY_WIRE_BYTES: usize = 64 * 1024 * 1024;

/// Maximum outgoing WebSocket write buffer size: `64 KiB` (`65_536` bytes).
pub const DEFAULT_MAX_WRITE_BUFFER_BYTES: usize = 64 * 1024;

/// Default timeout for establishing TCP + TLS + WebSocket handshake (`15` seconds).
pub const DEFAULT_CONNECT_TIMEOUT: Duration = Duration::from_secs(15);

/// Default timeout for sending an outgoing WebSocket frame (`5` seconds).
pub const DEFAULT_SEND_TIMEOUT: Duration = Duration::from_secs(5);

/// Typed error domain for the lightweight `tokio-tungstenite` Gateway transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayTransportError {
    /// Configured `max_message_size` must be greater than zero.
    ZeroMaxMessageSize,
    /// Configured `max_frame_size` must be greater than zero.
    ZeroMaxFrameSize,
    /// Configured `connect_timeout` must be greater than zero.
    ZeroConnectTimeout,
    /// Configured `send_timeout` must be greater than zero.
    ZeroSendTimeout,
    /// Gateway URL normalization or scheme validation failed.
    InvalidEndpoint(GatewayUrlError),
    /// Failed to construct `rustls` TLS configuration with `ring` provider.
    TlsConfig(String),
    /// Connection + TLS + WebSocket upgrade timed out.
    ConnectTimeout {
        /// Configured connect timeout duration.
        timeout: Duration,
    },
    /// Sending a frame over the WebSocket timed out.
    SendTimeout {
        /// Configured send timeout duration.
        timeout: Duration,
    },
    /// Incoming or outgoing frame exceeded the configured `max_message_size` or `max_frame_size`.
    CapacityExceeded {
        /// Configured maximum wire capacity in bytes.
        max_bytes: usize,
        /// Underlying capacity detail.
        detail: String,
    },
    /// TCP / TLS / WebSocket handshake failure.
    Connect {
        /// Underlying connection error message.
        msg: String,
    },
    /// WebSocket frame I/O or protocol error during active session.
    Io {
        /// Underlying WebSocket error message.
        msg: String,
    },
}

impl fmt::Display for GatewayTransportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ZeroMaxMessageSize => write!(f, "max_message_size must be greater than 0"),
            Self::ZeroMaxFrameSize => write!(f, "max_frame_size must be greater than 0"),
            Self::ZeroConnectTimeout => write!(f, "connect_timeout must be greater than 0"),
            Self::ZeroSendTimeout => write!(f, "send_timeout must be greater than 0"),
            Self::InvalidEndpoint(err) => write!(f, "invalid gateway endpoint: {err}"),
            Self::TlsConfig(msg) => write!(f, "TLS configuration error: {msg}"),
            Self::ConnectTimeout { timeout } => {
                write!(f, "gateway WebSocket connection timed out after {timeout:?}")
            }
            Self::SendTimeout { timeout } => {
                write!(f, "gateway WebSocket send timed out after {timeout:?}")
            }
            Self::CapacityExceeded { max_bytes, detail } => {
                write!(f, "gateway frame capacity ({max_bytes} B) exceeded: {detail}")
            }
            Self::Connect { msg } => write!(f, "gateway WebSocket connect failed: {msg}"),
            Self::Io { msg } => write!(f, "gateway WebSocket I/O error: {msg}"),
        }
    }
}

impl std::error::Error for GatewayTransportError {}

impl From<GatewayUrlError> for GatewayTransportError {
    fn from(err: GatewayUrlError) -> Self {
        Self::InvalidEndpoint(err)
    }
}

/// Configuration for the lightweight `tokio-tungstenite` Discord Gateway transport.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayTransportConfig {
    /// Maximum message size in bytes (default: `64 MiB`).
    pub max_message_size: usize,
    /// Maximum single frame size in bytes (default: `64 MiB`).
    pub max_frame_size: usize,
    /// Target write buffer size before flushing (default: `0` for immediate flush).
    pub write_buffer_size: usize,
    /// Maximum write buffer capacity in bytes (default: `64 KiB`).
    pub max_write_buffer_size: usize,
    /// Handshake timeout for TCP + TLS + WS upgrade (default: `15s`).
    pub connect_timeout: Duration,
    /// Write timeout for outbound WebSocket frames (default: `5s`).
    pub send_timeout: Duration,
    /// Whether to set `TCP_NODELAY` (disable Nagle's algorithm) on connect.
    pub disable_nagle: bool,
}

impl Default for GatewayTransportConfig {
    fn default() -> Self {
        Self {
            max_message_size: DEFAULT_MAX_GATEWAY_WIRE_BYTES,
            max_frame_size: DEFAULT_MAX_GATEWAY_WIRE_BYTES,
            write_buffer_size: 0,
            max_write_buffer_size: DEFAULT_MAX_WRITE_BUFFER_BYTES,
            connect_timeout: DEFAULT_CONNECT_TIMEOUT,
            send_timeout: DEFAULT_SEND_TIMEOUT,
            disable_nagle: false,
        }
    }
}

impl GatewayTransportConfig {
    /// Creates a new [`GatewayTransportConfig`] with default bounds.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Sets the maximum WebSocket message size in bytes.
    #[must_use]
    pub fn with_max_message_size(mut self, bytes: usize) -> Self {
        self.max_message_size = bytes;
        self
    }

    /// Sets the maximum WebSocket frame size in bytes.
    #[must_use]
    pub fn with_max_frame_size(mut self, bytes: usize) -> Self {
        self.max_frame_size = bytes;
        self
    }

    /// Sets the target write buffer threshold in bytes.
    #[must_use]
    pub fn with_write_buffer_size(mut self, bytes: usize) -> Self {
        self.write_buffer_size = bytes;
        self
    }

    /// Sets the maximum write buffer size in bytes.
    #[must_use]
    pub fn with_max_write_buffer_size(mut self, bytes: usize) -> Self {
        self.max_write_buffer_size = bytes;
        self
    }

    /// Sets the handshake connect timeout duration.
    #[must_use]
    pub fn with_connect_timeout(mut self, timeout: Duration) -> Self {
        self.connect_timeout = timeout;
        self
    }

    /// Sets the outbound frame send timeout duration.
    #[must_use]
    pub fn with_send_timeout(mut self, timeout: Duration) -> Self {
        self.send_timeout = timeout;
        self
    }

    /// Configures whether Nagle's algorithm (`TCP_NODELAY`) is disabled.
    #[must_use]
    pub fn with_disable_nagle(mut self, disable_nagle: bool) -> Self {
        self.disable_nagle = disable_nagle;
        self
    }

    /// Validates configuration invariants before dialing.
    pub fn validate(&self) -> Result<(), GatewayTransportError> {
        if self.max_message_size == 0 {
            return Err(GatewayTransportError::ZeroMaxMessageSize);
        }
        if self.max_frame_size == 0 {
            return Err(GatewayTransportError::ZeroMaxFrameSize);
        }
        if self.connect_timeout.is_zero() {
            return Err(GatewayTransportError::ZeroConnectTimeout);
        }
        if self.send_timeout.is_zero() {
            return Err(GatewayTransportError::ZeroSendTimeout);
        }
        Ok(())
    }

    /// Converts this transport config into a `tokio_tungstenite` [`WebSocketConfig`].
    #[must_use]
    pub fn to_websocket_config(&self) -> WebSocketConfig {
        WebSocketConfig::default()
            .max_message_size(Some(self.max_message_size))
            .max_frame_size(Some(self.max_frame_size))
            .write_buffer_size(self.write_buffer_size)
            .max_write_buffer_size(self.max_write_buffer_size)
    }
}

/// Normalized inbound frame delivered by [`GatewayTransport::next_frame`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayInboundFrame {
    /// Binary Gateway payload (ETF or compressed zstd/zlib chunk).
    Binary(Vec<u8>),
    /// Text Gateway payload (JSON mode or Voice Gateway signaling).
    Text(String),
    /// Remote peer initiated a WebSocket close handshake.
    Closed {
        /// Optional RFC 6455 / Discord Gateway close code (`1000`, `4000..=4014`).
        code: Option<u16>,
        /// Close reason string if provided.
        reason: String,
    },
}

/// Snapshot of transport-level frame and byte counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct GatewayTransportMetrics {
    /// Total outbound binary/text frames sent.
    pub frames_sent: u64,
    /// Total outbound payload bytes sent.
    pub bytes_sent: u64,
    /// Total inbound binary/text frames received.
    pub frames_received: u64,
    /// Total inbound payload bytes received.
    pub bytes_received: u64,
    /// Total incoming WebSocket `Ping` frames automatically answered with `Pong`.
    pub pings_answered: u64,
}

/// Installs the `ring` crypto provider as the process-wide `rustls` default if not yet set.
pub fn ensure_gateway_crypto_provider() {
    let _ = rustls::crypto::ring::default_provider().install_default();
}

/// Builds a clean `tokio_tungstenite::Connector::Rustls` configured with the explicit
/// `ring` cryptography provider and Mozilla `webpki-roots` root certificates.
pub fn build_gateway_tls_connector() -> Result<Connector, GatewayTransportError> {
    ensure_gateway_crypto_provider();
    let mut roots = rustls::RootCertStore::empty();
    roots.extend(webpki_roots::TLS_SERVER_ROOTS.iter().cloned());
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let builder = rustls::ClientConfig::builder_with_provider(provider)
        .with_safe_default_protocol_versions()
        .map_err(|err| GatewayTransportError::TlsConfig(err.to_string()))?;
    let client_config = builder.with_root_certificates(roots).with_no_client_auth();
    Ok(Connector::Rustls(Arc::new(client_config)))
}

/// Validates and normalizes a raw Discord Gateway endpoint (`wss://`, `ws://`, `https://`, `http://`)
/// into a canonical [`GatewayUrl`].
pub fn validate_gateway_endpoint(raw_url: &str) -> Result<GatewayUrl, GatewayTransportError> {
    GatewayUrl::from_base(raw_url).map_err(GatewayTransportError::InvalidEndpoint)
}

fn map_ws_io_error(err: TungsteniteError, max_bytes: usize) -> GatewayTransportError {
    match err {
        TungsteniteError::Capacity(cap_err) => GatewayTransportError::CapacityExceeded {
            max_bytes,
            detail: cap_err.to_string(),
        },
        other => GatewayTransportError::Io {
            msg: other.to_string(),
        },
    }
}

/// Lightweight WebSocket transport wrapping `tokio_tungstenite::WebSocketStream<S>`.
pub struct GatewayTransport<S> {
    stream: WebSocketStream<S>,
    endpoint: String,
    is_tls: bool,
    config: GatewayTransportConfig,
    metrics: GatewayTransportMetrics,
}

impl<S: fmt::Debug> fmt::Debug for GatewayTransport<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("GatewayTransport")
            .field("endpoint", &self.endpoint)
            .field("is_tls", &self.is_tls)
            .field("config", &self.config)
            .field("metrics", &self.metrics)
            .finish()
    }
}

impl<S> GatewayTransport<S>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    /// Wraps an already-established `WebSocketStream<S>` with the given endpoint and configuration.
    #[must_use]
    pub fn from_parts(
        stream: WebSocketStream<S>,
        endpoint: String,
        is_tls: bool,
        config: GatewayTransportConfig,
    ) -> Self {
        Self {
            stream,
            endpoint,
            is_tls,
            config,
            metrics: GatewayTransportMetrics::default(),
        }
    }

    /// Returns the normalized endpoint URL used for this connection.
    #[must_use]
    pub fn endpoint(&self) -> &str {
        &self.endpoint
    }

    /// Returns `true` if the connection uses `wss://` TLS encryption.
    #[must_use]
    pub fn is_tls(&self) -> bool {
        self.is_tls
    }

    /// Returns the active [`GatewayTransportConfig`].
    #[must_use]
    pub fn config(&self) -> &GatewayTransportConfig {
        &self.config
    }

    /// Returns a snapshot of transport metrics (`frames_sent`, `bytes_sent`, `frames_received`, etc.).
    #[must_use]
    pub fn metrics(&self) -> GatewayTransportMetrics {
        self.metrics
    }

    /// Mutable access to the underlying `tokio_tungstenite::WebSocketStream<S>`.
    pub fn inner_mut(&mut self) -> &mut WebSocketStream<S> {
        &mut self.stream
    }

    /// Consumes the transport and returns the underlying `WebSocketStream<S>`.
    pub fn into_inner(self) -> WebSocketStream<S> {
        self.stream
    }

    /// Sends a binary WebSocket frame bounded by `config.send_timeout` and `config.max_message_size`.
    pub async fn send_binary(&mut self, payload: Vec<u8>) -> Result<(), GatewayTransportError> {
        if payload.len() > self.config.max_message_size {
            return Err(GatewayTransportError::CapacityExceeded {
                max_bytes: self.config.max_message_size,
                detail: format!("outbound binary frame size {} exceeds limit", payload.len()),
            });
        }
        let len = payload.len() as u64;
        self.send_raw_message(Message::Binary(payload.into())).await?;
        self.metrics.frames_sent = self.metrics.frames_sent.saturating_add(1);
        self.metrics.bytes_sent = self.metrics.bytes_sent.saturating_add(len);
        Ok(())
    }

    /// Sends a text WebSocket frame bounded by `config.send_timeout` and `config.max_message_size`.
    pub async fn send_text(&mut self, text: String) -> Result<(), GatewayTransportError> {
        if text.len() > self.config.max_message_size {
            return Err(GatewayTransportError::CapacityExceeded {
                max_bytes: self.config.max_message_size,
                detail: format!("outbound text frame size {} exceeds limit", text.len()),
            });
        }
        let len = text.len() as u64;
        self.send_raw_message(Message::Text(text.into())).await?;
        self.metrics.frames_sent = self.metrics.frames_sent.saturating_add(1);
        self.metrics.bytes_sent = self.metrics.bytes_sent.saturating_add(len);
        Ok(())
    }

    /// Initiates a clean WebSocket close handshake with `code` and `reason`.
    ///
    /// If the remote peer has already closed or dropped the underlying transport
    /// (`ConnectionClosed`, `AlreadyClosed`, `BrokenPipe`, or `ConnectionReset`),
    /// the close is treated as completed (`Ok(())`).
    pub async fn close_gracefully(
        &mut self,
        code: u16,
        reason: &str,
    ) -> Result<(), GatewayTransportError> {
        let frame = CloseFrame {
            code: CloseCode::from(code),
            reason: reason.to_string().into(),
        };
        let send_timeout = self.config.send_timeout;
        let max_bytes = self.config.max_message_size;
        let res = timeout(send_timeout, self.stream.close(Some(frame)))
            .await
            .map_err(|_| GatewayTransportError::SendTimeout {
                timeout: send_timeout,
            })?;
        match res {
            Ok(())
            | Err(TungsteniteError::ConnectionClosed)
            | Err(TungsteniteError::AlreadyClosed) => Ok(()),
            Err(TungsteniteError::Io(ref io_err))
                if matches!(
                    io_err.kind(),
                    std::io::ErrorKind::BrokenPipe
                        | std::io::ErrorKind::ConnectionReset
                        | std::io::ErrorKind::NotConnected
                ) =>
            {
                Ok(())
            }
            Err(err) => Err(map_ws_io_error(err, max_bytes)),
        }
    }

    /// Reads the next inbound frame from the WebSocket stream.
    ///
    /// Automatically replies to incoming `Message::Ping` frames with `Message::Pong`
    /// and skips unsolicited `Pong` / raw `Frame` messages.
    pub async fn next_frame(&mut self) -> Result<Option<GatewayInboundFrame>, GatewayTransportError> {
        let max_bytes = self.config.max_message_size;
        while let Some(item) = self.stream.next().await {
            let msg = item.map_err(|err| map_ws_io_error(err, max_bytes))?;
            match msg {
                Message::Binary(data) => {
                    let bytes = data.to_vec();
                    self.metrics.frames_received = self.metrics.frames_received.saturating_add(1);
                    self.metrics.bytes_received =
                        self.metrics.bytes_received.saturating_add(bytes.len() as u64);
                    return Ok(Some(GatewayInboundFrame::Binary(bytes)));
                }
                Message::Text(text) => {
                    let s = text.to_string();
                    self.metrics.frames_received = self.metrics.frames_received.saturating_add(1);
                    self.metrics.bytes_received =
                        self.metrics.bytes_received.saturating_add(s.len() as u64);
                    return Ok(Some(GatewayInboundFrame::Text(s)));
                }
                Message::Ping(payload) => {
                    self.send_raw_message(Message::Pong(payload)).await?;
                    self.metrics.pings_answered = self.metrics.pings_answered.saturating_add(1);
                }
                Message::Close(close_frame) => {
                    let (code, reason) = match close_frame {
                        Some(cf) => (Some(u16::from(cf.code)), cf.reason.to_string()),
                        None => (None, String::new()),
                    };
                    return Ok(Some(GatewayInboundFrame::Closed { code, reason }));
                }
                Message::Pong(_) | Message::Frame(_) => {}
            }
        }
        Ok(None)
    }

    async fn send_raw_message(&mut self, msg: Message) -> Result<(), GatewayTransportError> {
        let send_timeout = self.config.send_timeout;
        let max_bytes = self.config.max_message_size;
        let res = timeout(send_timeout, self.stream.send(msg))
            .await
            .map_err(|_| GatewayTransportError::SendTimeout {
                timeout: send_timeout,
            })?;
        res.map_err(|err| map_ws_io_error(err, max_bytes))
    }
}

/// Connects to a Discord Gateway endpoint (`ws://` or `wss://`) using `tokio-tungstenite`
/// with explicit `rustls` (ring provider + `webpki-roots`) TLS configuration and bounded frame limits.
pub async fn connect_gateway_transport(
    raw_url: &str,
    config: &GatewayTransportConfig,
) -> Result<GatewayTransport<MaybeTlsStream<TcpStream>>, GatewayTransportError> {
    config.validate()?;
    let normalized = validate_gateway_endpoint(raw_url)?;
    let endpoint = normalized.as_str().to_string();
    let is_tls = endpoint.starts_with("wss://");
    let connector = if is_tls {
        Some(build_gateway_tls_connector()?)
    } else {
        ensure_gateway_crypto_provider();
        None
    };
    let ws_config = Some(config.to_websocket_config());
    let connect_fut = connect_async_tls_with_config(
        &endpoint,
        ws_config,
        config.disable_nagle,
        connector,
    );
    let (stream, _response) = timeout(config.connect_timeout, connect_fut)
        .await
        .map_err(|_| GatewayTransportError::ConnectTimeout {
            timeout: config.connect_timeout,
        })?
        .map_err(|err| GatewayTransportError::Connect {
            msg: err.to_string(),
        })?;

    Ok(GatewayTransport::from_parts(
        stream,
        endpoint,
        is_tls,
        config.clone(),
    ))
}

/// Performs a client WebSocket handshake over an arbitrary async stream `S` (such as an in-memory
/// `tokio::io::DuplexStream` in isolated contract tests) with bounded `WebSocketConfig`.
pub async fn connect_gateway_stream<S>(
    raw_url: &str,
    io_stream: S,
    config: &GatewayTransportConfig,
) -> Result<GatewayTransport<S>, GatewayTransportError>
where
    S: AsyncRead + AsyncWrite + Unpin,
{
    config.validate()?;
    let normalized = validate_gateway_endpoint(raw_url)?;
    let endpoint = normalized.as_str().to_string();
    let is_tls = endpoint.starts_with("wss://");
    let ws_config = Some(config.to_websocket_config());
    let (stream, _response) = timeout(
        config.connect_timeout,
        client_async_with_config(&endpoint, io_stream, ws_config),
    )
    .await
    .map_err(|_| GatewayTransportError::ConnectTimeout {
        timeout: config.connect_timeout,
    })?
    .map_err(|err| GatewayTransportError::Connect {
        msg: err.to_string(),
    })?;

    Ok(GatewayTransport::from_parts(
        stream,
        endpoint,
        is_tls,
        config.clone(),
    ))
}
