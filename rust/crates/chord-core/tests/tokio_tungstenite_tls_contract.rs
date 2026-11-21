#![forbid(unsafe_code)]

//! Contract tests for clean lightweight `tokio-tungstenite` WebSocket transport with TLS support
//! for Discord Gateway (`STAGE-05.1.1`).
//!
//! Verifies:
//! 1. Default and builder configuration of `GatewayTransportConfig` (`64 MiB` max message/frame size,
//!    `64 KiB` max write buffer, `15s` connect timeout, `5s` send timeout) and conversion into
//!    `tokio_tungstenite::tungstenite::protocol::WebSocketConfig`.
//! 2. Validation of zero limits (`ZeroMaxMessageSize`, `ZeroMaxFrameSize`, `ZeroConnectTimeout`,
//!    `ZeroSendTimeout`).
//! 3. Clean `rustls` (ring provider) + `webpki-roots` TLS `Connector` construction via
//!    `build_gateway_tls_connector()` and `ensure_gateway_crypto_provider()`.
//! 4. Strict Gateway URL normalization & validation (`wss://`, `ws://`, `https://` -> `wss://`,
//!    canonical query `encoding=etf&v=9&compress=zstd-stream`, rejection of empty/fragment/invalid schemes).
//! 5. Live local WebSocket handshake (`connect_gateway_transport`), binary & text frame exchange,
//!    Ping -> Pong auto-reply, and clean Close frame handling (`CloseOutcome` with code & reason).
//! 6. Frame capacity enforcement (`CapacityExceeded` when incoming payload exceeds `max_message_size`).
//! 7. In-memory duplex stream WebSocket connection (`connect_gateway_stream`) and atomic transport
//!    telemetry (`frames_sent`, `bytes_sent`, `frames_received`, `bytes_received`, `pings_answered`).
//! 8. Connect timeout enforcement when remote endpoint stalls handshake.

use futures_util::{SinkExt, StreamExt};
use std::time::Duration;
use tokio::net::TcpListener;
use tokio_tungstenite::accept_async;
use tokio_tungstenite::tungstenite::protocol::frame::coding::CloseCode;
use tokio_tungstenite::tungstenite::protocol::{CloseFrame, Message};

use chord_core::ws_transport::{
    DEFAULT_CONNECT_TIMEOUT, DEFAULT_MAX_GATEWAY_WIRE_BYTES, DEFAULT_MAX_WRITE_BUFFER_BYTES,
    DEFAULT_SEND_TIMEOUT, GatewayInboundFrame, GatewayTransport, GatewayTransportConfig,
    GatewayTransportError, build_gateway_tls_connector, connect_gateway_stream,
    connect_gateway_transport, ensure_gateway_crypto_provider, validate_gateway_endpoint,
};

#[test]
fn test_gateway_transport_config_defaults_and_builder() {
    let cfg = GatewayTransportConfig::default();
    assert_eq!(cfg.max_message_size, DEFAULT_MAX_GATEWAY_WIRE_BYTES);
    assert_eq!(cfg.max_frame_size, DEFAULT_MAX_GATEWAY_WIRE_BYTES);
    assert_eq!(DEFAULT_MAX_GATEWAY_WIRE_BYTES, 64 * 1024 * 1024);
    assert_eq!(cfg.write_buffer_size, 0);
    assert_eq!(cfg.max_write_buffer_size, DEFAULT_MAX_WRITE_BUFFER_BYTES);
    assert_eq!(DEFAULT_MAX_WRITE_BUFFER_BYTES, 64 * 1024);
    assert_eq!(cfg.connect_timeout, DEFAULT_CONNECT_TIMEOUT);
    assert_eq!(cfg.send_timeout, DEFAULT_SEND_TIMEOUT);
    assert!(!cfg.disable_nagle);
    assert!(cfg.validate().is_ok());

    let custom = GatewayTransportConfig::default()
        .with_max_message_size(16 * 1024 * 1024)
        .with_max_frame_size(8 * 1024 * 1024)
        .with_write_buffer_size(4096)
        .with_max_write_buffer_size(128 * 1024)
        .with_connect_timeout(Duration::from_secs(10))
        .with_send_timeout(Duration::from_secs(3))
        .with_disable_nagle(true);

    assert_eq!(custom.max_message_size, 16 * 1024 * 1024);
    assert_eq!(custom.max_frame_size, 8 * 1024 * 1024);
    assert_eq!(custom.write_buffer_size, 4096);
    assert_eq!(custom.max_write_buffer_size, 128 * 1024);
    assert_eq!(custom.connect_timeout, Duration::from_secs(10));
    assert_eq!(custom.send_timeout, Duration::from_secs(3));
    assert!(custom.disable_nagle);
    assert!(custom.validate().is_ok());

    let ws_cfg = custom.to_websocket_config();
    assert_eq!(ws_cfg.max_message_size, Some(16 * 1024 * 1024));
    assert_eq!(ws_cfg.max_frame_size, Some(8 * 1024 * 1024));
    assert_eq!(ws_cfg.write_buffer_size, 4096);
    assert_eq!(ws_cfg.max_write_buffer_size, 128 * 1024);
}

#[test]
fn test_gateway_transport_config_validation_rejects_zero_limits() {
    let zero_msg = GatewayTransportConfig::default().with_max_message_size(0);
    assert_eq!(
        zero_msg.validate(),
        Err(GatewayTransportError::ZeroMaxMessageSize)
    );

    let zero_frame = GatewayTransportConfig::default().with_max_frame_size(0);
    assert_eq!(
        zero_frame.validate(),
        Err(GatewayTransportError::ZeroMaxFrameSize)
    );

    let zero_conn = GatewayTransportConfig::default().with_connect_timeout(Duration::ZERO);
    assert_eq!(
        zero_conn.validate(),
        Err(GatewayTransportError::ZeroConnectTimeout)
    );

    let zero_send = GatewayTransportConfig::default().with_send_timeout(Duration::ZERO);
    assert_eq!(
        zero_send.validate(),
        Err(GatewayTransportError::ZeroSendTimeout)
    );
}

#[test]
fn test_gateway_tls_connector_builds_clean_rustls_ring_connector() {
    ensure_gateway_crypto_provider();
    let connector =
        build_gateway_tls_connector().expect("TLS connector with ring + webpki-roots must build");
    match connector {
        tokio_tungstenite::Connector::Rustls(_) => {}
        _ => panic!("Expected Connector::Rustls"),
    }
}

#[test]
fn test_validate_gateway_endpoint_normalizes_and_rejects_invalid_urls() {
    let url = validate_gateway_endpoint("wss://gateway.discord.gg").expect("valid default gateway");
    assert_eq!(
        url.as_str(),
        "wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream"
    );

    let resume = validate_gateway_endpoint("https://gateway-us-east1-b.discord.gg/?v=9")
        .expect("https mapped to wss");
    assert_eq!(
        resume.as_str(),
        "wss://gateway-us-east1-b.discord.gg/?encoding=etf&v=9&compress=zstd-stream"
    );

    let local = validate_gateway_endpoint("ws://127.0.0.1:9999/").expect("local ws allowed");
    assert_eq!(
        local.as_str(),
        "ws://127.0.0.1:9999/?encoding=etf&v=9&compress=zstd-stream"
    );

    for invalid in [
        "",
        "   ",
        "ftp://gateway.discord.gg",
        "wss://",
        "wss://gateway.discord.gg#frag",
    ] {
        assert!(
            validate_gateway_endpoint(invalid).is_err(),
            "Expected endpoint {invalid:?} to be rejected"
        );
    }
}

#[tokio::test]
async fn test_gateway_transport_local_ws_handshake_frames_ping_pong_and_close() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let url = format!("ws://{addr}");

    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.expect("accept");
        let mut ws = accept_async(tcp).await.expect("accept_async");

        // 1. Server sends binary ETF-like payload
        ws.send(Message::Binary(vec![0x83, 0x61, 0x0a].into()))
            .await
            .expect("send binary");

        // 2. Server sends Ping; client transport must auto-reply with matching Pong
        ws.send(Message::Ping(vec![1, 2, 3, 4].into()))
            .await
            .expect("send ping");

        // 3. Server sends Text payload after Ping
        ws.send(Message::Text(
            r#"{"op":10,"d":{"heartbeat_interval":41250}}"#.into(),
        ))
        .await
        .expect("send text");

        // Verify client sent Pong([1, 2, 3, 4])
        let pong_msg = ws.next().await.expect("stream open").expect("msg ok");
        assert_eq!(pong_msg, Message::Pong(vec![1, 2, 3, 4].into()));

        // Verify client binary frame
        let client_bin = ws.next().await.expect("stream open").expect("msg ok");
        assert_eq!(client_bin, Message::Binary(vec![0x83, 0x61, 0x01].into()));

        // 4. Server sends Close(4000)
        ws.send(Message::Close(Some(CloseFrame {
            code: CloseCode::Library(4000),
            reason: "session reset".into(),
        })))
        .await
        .expect("send close");
    });

    let cfg = GatewayTransportConfig::default();
    let mut transport = connect_gateway_transport(&url, &cfg)
        .await
        .expect("connect_gateway_transport must succeed");

    assert!(!transport.is_tls());
    assert!(transport.endpoint().starts_with(&format!("ws://{addr}/?")));

    // Receive binary frame
    let frame1 = transport
        .next_frame()
        .await
        .expect("next_frame 1")
        .expect("some frame 1");
    assert_eq!(frame1, GatewayInboundFrame::Binary(vec![0x83, 0x61, 0x0a]));

    // Next call processes Ping transparently (auto-sending Pong) and yields the Text frame
    let frame2 = transport
        .next_frame()
        .await
        .expect("next_frame 2")
        .expect("some frame 2");
    assert_eq!(
        frame2,
        GatewayInboundFrame::Text(r#"{"op":10,"d":{"heartbeat_interval":41250}}"#.to_string())
    );
    assert_eq!(transport.metrics().pings_answered, 1);

    // Send binary frame from client
    transport
        .send_binary(vec![0x83, 0x61, 0x01])
        .await
        .expect("send_binary");
    assert_eq!(transport.metrics().frames_sent, 1);
    assert_eq!(transport.metrics().bytes_sent, 3);

    // Receive Close frame
    let frame3 = transport
        .next_frame()
        .await
        .expect("next_frame 3")
        .expect("some frame 3");
    assert_eq!(
        frame3,
        GatewayInboundFrame::Closed {
            code: Some(4000),
            reason: "session reset".to_string(),
        }
    );

    server.await.expect("server join");
}

#[tokio::test]
async fn test_gateway_transport_enforces_max_message_capacity() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let url = format!("ws://{addr}");

    let server = tokio::spawn(async move {
        let (tcp, _) = listener.accept().await.expect("accept");
        let mut ws = accept_async(tcp).await.expect("accept_async");
        // Send a 2048-byte binary message while client max_message_size is 512 bytes
        let oversized = vec![0x42u8; 2048];
        let _ = ws.send(Message::Binary(oversized.into())).await;
    });

    let cfg = GatewayTransportConfig::default()
        .with_max_message_size(512)
        .with_max_frame_size(512);
    let mut transport = connect_gateway_transport(&url, &cfg)
        .await
        .expect("connect");

    let err = transport
        .next_frame()
        .await
        .expect_err("oversized message must be rejected with CapacityExceeded");
    match err {
        GatewayTransportError::CapacityExceeded { max_bytes, .. } => {
            assert_eq!(max_bytes, 512);
        }
        other => panic!("Expected CapacityExceeded, got {other:?}"),
    }

    server.await.expect("server join");
}

#[tokio::test]
async fn test_gateway_transport_duplex_stream_and_telemetry_counters() {
    let (client_io, server_io) = tokio::io::duplex(64 * 1024);
    let cfg = GatewayTransportConfig::default();

    let server_fut = async move {
        let mut ws = accept_async(server_io).await.expect("accept duplex");
        let msg1 = ws.next().await.expect("msg1").expect("ok");
        assert_eq!(msg1, Message::Binary(vec![10, 20, 30, 40].into()));

        let msg2 = ws.next().await.expect("msg2").expect("ok");
        assert_eq!(msg2, Message::Text("hello-gateway".into()));

        ws.send(Message::Binary(vec![99, 100].into()))
            .await
            .expect("reply binary");
        let close_msg = ws.next().await.expect("close msg").expect("ok");
        assert!(matches!(close_msg, Message::Close(_)));
    };

    let client_fut = async move {
        let mut transport: GatewayTransport<tokio::io::DuplexStream> =
            connect_gateway_stream("ws://localhost/", client_io, &cfg)
                .await
                .expect("connect_gateway_stream");

        transport
            .send_binary(vec![10, 20, 30, 40])
            .await
            .expect("send bin");
        transport
            .send_text("hello-gateway".to_string())
            .await
            .expect("send text");

        let inbound = transport.next_frame().await.expect("ok").expect("frame");
        assert_eq!(inbound, GatewayInboundFrame::Binary(vec![99, 100]));

        let m = transport.metrics();
        assert_eq!(m.frames_sent, 2);
        assert_eq!(m.bytes_sent, 4 + "hello-gateway".len() as u64);
        assert_eq!(m.frames_received, 1);
        assert_eq!(m.bytes_received, 2);

        transport
            .close_gracefully(1000, "normal")
            .await
            .expect("close");
    };

    tokio::join!(server_fut, client_fut);
}

#[tokio::test]
async fn test_gateway_transport_connect_timeout_on_stalled_handshake() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let url = format!("ws://{addr}");

    // Server accepts TCP connection but never performs WebSocket HTTP Upgrade handshake
    let _server = tokio::spawn(async move {
        let (_tcp, _) = listener.accept().await.expect("accept");
        tokio::time::sleep(Duration::from_secs(2)).await;
    });

    let cfg = GatewayTransportConfig::default().with_connect_timeout(Duration::from_millis(80));
    let res = connect_gateway_transport(&url, &cfg).await;
    assert!(
        matches!(res, Err(GatewayTransportError::ConnectTimeout { .. })),
        "Expected ConnectTimeout when WebSocket handshake stalls, got: {res:?}"
    );
}
