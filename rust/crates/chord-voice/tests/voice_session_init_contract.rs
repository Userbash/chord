//! Контрактные тесты живого UDP-транспорта и инициализации сессии `VoiceSessionDriver` (`P1-1.2`):
//! 1. Валидация `VoiceUdpTransportConfig` (адрес привязки, таймауты чтения/записи > 0, `max_discovery_attempts` в `1..=5`, интервал keepalive).
//! 2. Реальный `std::net::UdpSocket` (loopback) + 74-байтный обмен UDP IP Discovery через `perform_udp_discovery`
//!    (`start` -> `handle_hello` -> `handle_ready` -> `perform_udp_discovery` -> `OP 1 SELECT_PROTOCOL`, фаза `SelectingProtocol`).
//! 3. Обработка таймаута UDP peer (ограниченный retry без паники) и отбраковка некорректных пакетов
//!    (несовпадающий SSRC, обрезанный пакет `< 74` байт).
//! 4. Установление медиа-канала (`OP 4 SESSION_DESCRIPTION` с 32B `secret_key`), шифрование `encrypt_audio_frame` +
//!    реальная отправка SRTP-кадров через `flush_outgoing_udp` и отправка UDP keepalive (`send_udp_keepalive` 8B / 12B).

use chord_voice::session::{
    LiveUdpVoiceSocket, LiveVoiceSessionTransport, VoiceSessionDriver, VoiceSessionError,
    VoiceSessionPhase, VoiceStateUpdate, VoiceUdpTransportConfig, init_udp_voice_session,
    perform_udp_discovery,
};
use chord_voice::srtp::{EncryptionMode, SrtpContext};
use chord_voice::udp::{
    DISCOVERY_LEN, DiscoveryPacket, KEEPALIVE_LEN, KEEPALIVE_REQUEST, KeepalivePacket, UdpError,
};
use chord_voice::{
    Hello, Ready, SessionDescription, VoiceOp, VoicePayload, VoiceServerUpdate, parse_frame,
};
use std::net::UdpSocket;
use std::thread;
use std::time::Duration;

const SAMPLE_SECRET_KEY: [u8; 32] = [
    0x21, 0x22, 0x23, 0x24, 0x25, 0x26, 0x27, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x30,
    0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x3a, 0x3b, 0x3c, 0x3d, 0x3e, 0x3f, 0x40,
];

fn sample_driver_in_ready(server_port: u16, ssrc: u32) -> VoiceSessionDriver {
    let server = VoiceServerUpdate::parse(
        r#"{"token":"tok-live-udp-01","guild_id":"1014537851418980372","endpoint":"c-fra19-4da7fe5f.discord.media:2053"}"#,
    )
    .expect("valid VoiceServerUpdate");
    let state = VoiceStateUpdate::parse(
        r#"{"guild_id":"1014537851418980372","channel_id":"1176785373989445703","user_id":"384299970632876032","session_id":"sess-live-udp-01"}"#,
    )
    .expect("valid VoiceStateUpdate");

    let (mut driver, _identify) = VoiceSessionDriver::start(server, state).expect("start driver");
    let _ = driver.take_outgoing_ws();

    driver
        .handle_hello(Hello {
            version: 8,
            heartbeat_interval_ms: 13_750,
        })
        .expect("handle_hello");

    driver
        .handle_ready(Ready {
            ssrc: u64::from(ssrc),
            port: server_port,
            ip: "127.0.0.1".to_owned(),
            modes: vec![
                "aead_aes256_gcm_rtpsize".to_owned(),
                "aead_xchacha20_poly1305_rtpsize".to_owned(),
            ],
            experiments: vec!["fixed_keyframe_interval".to_owned()],
            streams: Vec::new(),
        })
        .expect("handle_ready");

    driver
}

#[test]
fn transport_config_validates_bind_addr_timeouts_attempts_and_keepalive() {
    let default_cfg = VoiceUdpTransportConfig::default();
    assert!(default_cfg.validate().is_ok());

    let loopback_cfg = VoiceUdpTransportConfig::loopback();
    assert_eq!(loopback_cfg.bind_addr, "127.0.0.1:0");
    assert!(loopback_cfg.validate().is_ok());

    // Пустой адрес привязки отклоняется
    let empty_bind = VoiceUdpTransportConfig {
        bind_addr: "   ".to_owned(),
        ..VoiceUdpTransportConfig::loopback()
    };
    assert!(matches!(
        empty_bind.validate(),
        Err(VoiceSessionError::InvalidTransportConfig { .. })
    ));

    // Некорректный SocketAddr отклоняется
    let bad_bind = VoiceUdpTransportConfig {
        bind_addr: "not-a-socket-addr".to_owned(),
        ..VoiceUdpTransportConfig::loopback()
    };
    assert!(matches!(
        bad_bind.validate(),
        Err(VoiceSessionError::InvalidTransportConfig { .. })
    ));

    // Нулевой read_timeout или write_timeout отклоняется
    let zero_read = VoiceUdpTransportConfig {
        read_timeout: Duration::ZERO,
        ..VoiceUdpTransportConfig::loopback()
    };
    assert!(matches!(
        zero_read.validate(),
        Err(VoiceSessionError::InvalidTransportConfig { .. })
    ));

    let zero_write = VoiceUdpTransportConfig {
        write_timeout: Duration::ZERO,
        ..VoiceUdpTransportConfig::loopback()
    };
    assert!(matches!(
        zero_write.validate(),
        Err(VoiceSessionError::InvalidTransportConfig { .. })
    ));

    // max_discovery_attempts вне диапазона 1..=5 отклоняется
    let zero_attempts = VoiceUdpTransportConfig {
        max_discovery_attempts: 0,
        ..VoiceUdpTransportConfig::loopback()
    };
    assert!(matches!(
        zero_attempts.validate(),
        Err(VoiceSessionError::InvalidTransportConfig { .. })
    ));

    let too_many_attempts = VoiceUdpTransportConfig {
        max_discovery_attempts: 6,
        ..VoiceUdpTransportConfig::loopback()
    };
    assert!(matches!(
        too_many_attempts.validate(),
        Err(VoiceSessionError::InvalidTransportConfig { .. })
    ));

    // Нулевой keepalive_interval отклоняется
    let zero_keepalive = VoiceUdpTransportConfig {
        keepalive_interval: Duration::ZERO,
        ..VoiceUdpTransportConfig::loopback()
    };
    assert!(matches!(
        zero_keepalive.validate(),
        Err(VoiceSessionError::InvalidTransportConfig { .. })
    ));
}

#[test]
fn live_udp_discovery_binds_socket_exchanges_74b_packet_and_emits_op1_select_protocol() {
    let responder = UdpSocket::bind("127.0.0.1:0").expect("bind loopback responder");
    responder
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("set responder timeout");
    let server_port = responder.local_addr().expect("responder addr").port();
    let expected_ssrc = 9208u32;

    let responder_handle = thread::spawn(move || {
        let mut buf = [0u8; 128];
        let (n, client_addr) = responder.recv_from(&mut buf).expect("recv 74B request");
        assert_eq!(n, DISCOVERY_LEN);
        let parsed_req = DiscoveryPacket::parse(&buf[..n]).expect("valid 74B discovery request");
        match parsed_req {
            DiscoveryPacket::Request { ssrc, .. } => assert_eq!(ssrc, expected_ssrc),
            other => panic!("expected DiscoveryPacket::Request, got {other:?}"),
        }

        let response = DiscoveryPacket::Response {
            ssrc: expected_ssrc,
            address: "95.164.91.98".to_owned(),
            port: client_addr.port(),
        }
        .encode()
        .expect("encode 74B discovery response");
        responder
            .send_to(&response, client_addr)
            .expect("send 74B discovery response");
        client_addr.port()
    });

    let mut driver = sample_driver_in_ready(server_port, expected_ssrc);
    assert_eq!(driver.phase(), VoiceSessionPhase::DiscoveringUdp);

    let cfg = VoiceUdpTransportConfig {
        bind_addr: "127.0.0.1:0".to_owned(),
        read_timeout: Duration::from_millis(500),
        write_timeout: Duration::from_millis(500),
        max_discovery_attempts: 3,
        keepalive_interval: Duration::from_secs(5),
    };

    let (socket, select): (LiveVoiceSessionTransport, _) =
        perform_udp_discovery(&mut driver, &cfg).expect("perform_udp_discovery must succeed");
    let observed_client_port = responder_handle.join().expect("responder thread join");

    let local_addr = socket.local_addr().expect("socket local_addr");
    assert_eq!(local_addr.port(), observed_client_port);
    assert_eq!(driver.phase(), VoiceSessionPhase::SelectingProtocol);
    assert_eq!(
        driver.discovered_endpoint_udp(),
        Some(("95.164.91.98", observed_client_port))
    );
    assert_eq!(select.protocol, "udp");
    assert_eq!(select.address, "95.164.91.98");
    assert_eq!(select.port, observed_client_port);
    assert_eq!(select.mode, "aead_xchacha20_poly1305_rtpsize");

    // Проверяем, что OP 1 SELECT_PROTOCOL записан в очередь исходящих WS-кадров
    let ws_out = driver.take_outgoing_ws();
    assert_eq!(ws_out.len(), 1);
    let parsed_op1 = parse_frame(&ws_out[0]).expect("parse OP 1 SELECT_PROTOCOL");
    assert_eq!(parsed_op1.op, VoiceOp::SelectProtocol);
    assert_eq!(parsed_op1.payload, VoicePayload::SelectProtocol(select));
}

#[test]
fn udp_discovery_retries_on_packet_loss_and_times_out_cleanly_when_peer_silent() {
    // 1. Потеря первого пакета, ответ на второй попытке -> успешное завершение с attempts_used == 2
    let flaky_responder = UdpSocket::bind("127.0.0.1:0").expect("bind flaky responder");
    flaky_responder
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    let flaky_port = flaky_responder.local_addr().expect("addr").port();
    let ssrc = 13370u32;

    let flaky_handle = thread::spawn(move || {
        let mut buf = [0u8; 128];
        // Первая попытка игнорируется (имитация потери UDP-пакета в сети)
        let (n1, _) = flaky_responder.recv_from(&mut buf).expect("recv attempt 1");
        assert_eq!(n1, DISCOVERY_LEN);

        // На вторую попытку отвечаем валидным 74-байтным пакетом
        let (n2, client_addr) = flaky_responder.recv_from(&mut buf).expect("recv attempt 2");
        assert_eq!(n2, DISCOVERY_LEN);
        let resp = DiscoveryPacket::Response {
            ssrc,
            address: "127.0.0.1".to_owned(),
            port: client_addr.port(),
        }
        .encode()
        .expect("encode");
        flaky_responder
            .send_to(&resp, client_addr)
            .expect("send attempt 2 response");
    });

    let mut driver = sample_driver_in_ready(flaky_port, ssrc);
    let retry_cfg = VoiceUdpTransportConfig {
        bind_addr: "127.0.0.1:0".to_owned(),
        read_timeout: Duration::from_millis(60),
        write_timeout: Duration::from_millis(60),
        max_discovery_attempts: 3,
        keepalive_interval: Duration::from_secs(5),
    };
    let outcome = init_udp_voice_session(&mut driver, &retry_cfg)
        .expect("discovery must succeed on 2nd attempt");
    flaky_handle.join().expect("join flaky responder");
    assert_eq!(outcome.attempts_used, 2);
    assert_eq!(driver.phase(), VoiceSessionPhase::SelectingProtocol);

    // 2. Полное молчание UDP-пира -> ограниченный retry (2 попытки) и чистый возврат UdpError::Timeout
    let silent_responder = UdpSocket::bind("127.0.0.1:0").expect("bind silent responder");
    let silent_port = silent_responder.local_addr().expect("addr").port();
    let mut driver_timeout = sample_driver_in_ready(silent_port, ssrc);
    let timeout_cfg = VoiceUdpTransportConfig {
        bind_addr: "127.0.0.1:0".to_owned(),
        read_timeout: Duration::from_millis(40),
        write_timeout: Duration::from_millis(40),
        max_discovery_attempts: 2,
        keepalive_interval: Duration::from_secs(5),
    };

    let err = driver_timeout
        .perform_udp_discovery(&timeout_cfg)
        .expect_err("silent peer must time out cleanly");
    assert_eq!(err, VoiceSessionError::Udp(UdpError::Timeout));
    assert_eq!(driver_timeout.phase(), VoiceSessionPhase::DiscoveringUdp);
    drop(silent_responder);
}

#[test]
fn udp_discovery_rejects_wrong_ssrc_and_truncated_packets() {
    // 1. Ответ с чужим SSRC -> VoiceSessionError::SsrcMismatch
    let wrong_ssrc_responder = UdpSocket::bind("127.0.0.1:0").expect("bind responder");
    wrong_ssrc_responder
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    let port_wrong_ssrc = wrong_ssrc_responder.local_addr().expect("addr").port();

    let wrong_ssrc_handle = thread::spawn(move || {
        let mut buf = [0u8; 128];
        let (_, client_addr) = wrong_ssrc_responder.recv_from(&mut buf).expect("recv");
        let bad_resp = DiscoveryPacket::Response {
            ssrc: 99999,
            address: "127.0.0.1".to_owned(),
            port: client_addr.port(),
        }
        .encode()
        .expect("encode");
        wrong_ssrc_responder
            .send_to(&bad_resp, client_addr)
            .expect("send wrong ssrc");
    });

    let mut driver_ssrc = sample_driver_in_ready(port_wrong_ssrc, 4242);
    let cfg = VoiceUdpTransportConfig {
        bind_addr: "127.0.0.1:0".to_owned(),
        read_timeout: Duration::from_millis(250),
        write_timeout: Duration::from_millis(250),
        max_discovery_attempts: 1,
        keepalive_interval: Duration::from_secs(5),
    };
    let ssrc_err = driver_ssrc
        .perform_udp_discovery(&cfg)
        .expect_err("wrong SSRC must be rejected");
    wrong_ssrc_handle.join().expect("join");
    assert_eq!(
        ssrc_err,
        VoiceSessionError::SsrcMismatch {
            expected: 4242,
            actual: 99999,
        }
    );
    assert_eq!(driver_ssrc.phase(), VoiceSessionPhase::DiscoveringUdp);

    // 2. Обрезанный пакет (< 74 байт) -> VoiceSessionError::Udp(UdpError::InvalidLength)
    let trunc_responder = UdpSocket::bind("127.0.0.1:0").expect("bind responder");
    trunc_responder
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    let port_trunc = trunc_responder.local_addr().expect("addr").port();

    let trunc_handle = thread::spawn(move || {
        let mut buf = [0u8; 128];
        let (_, client_addr) = trunc_responder.recv_from(&mut buf).expect("recv");
        trunc_responder
            .send_to(&[0x00, 0x02, 0x00, 0x46, 0x00, 0x00], client_addr)
            .expect("send truncated 6B packet");
    });

    let mut driver_trunc = sample_driver_in_ready(port_trunc, 4242);
    let trunc_err = driver_trunc
        .perform_udp_discovery(&cfg)
        .expect_err("truncated packet must be rejected");
    trunc_handle.join().expect("join");
    assert_eq!(trunc_err, VoiceSessionError::Udp(UdpError::InvalidLength));
    assert_eq!(driver_trunc.phase(), VoiceSessionPhase::DiscoveringUdp);

    // 3. Попытка вызвать perform_udp_discovery до OP 2 READY (в фазе Identifying) отклоняется InvalidPhase
    let server = VoiceServerUpdate::parse(
        r#"{"token":"tok-live-udp-01","guild_id":"1014537851418980372","endpoint":"c-fra19-4da7fe5f.discord.media:2053"}"#,
    )
    .expect("server");
    let state = VoiceStateUpdate::parse(
        r#"{"guild_id":"1014537851418980372","channel_id":"1176785373989445703","user_id":"384299970632876032","session_id":"sess-live-udp-01"}"#,
    )
    .expect("state");
    let (mut early_driver, _) = VoiceSessionDriver::start(server, state).expect("start");
    assert_eq!(
        early_driver.perform_udp_discovery(&cfg).unwrap_err(),
        VoiceSessionError::InvalidPhase {
            expected: "DiscoveringUdp",
            actual: VoiceSessionPhase::Identifying,
        }
    );
}

#[test]
fn establish_and_flush_media_sends_real_srtp_frames_and_keepalives_over_udp_socket() {
    let responder = UdpSocket::bind("127.0.0.1:0").expect("bind media responder");
    responder
        .set_read_timeout(Some(Duration::from_secs(2)))
        .expect("timeout");
    let server_port = responder.local_addr().expect("addr").port();
    let ssrc = 7788u32;

    let responder_handle = thread::spawn(move || {
        let mut buf = [0u8; 2048];
        // 1. Принимаем 74B IP Discovery и отправляем 74B ответ
        let (n_disc, client_addr) = responder.recv_from(&mut buf).expect("recv discovery");
        assert_eq!(n_disc, DISCOVERY_LEN);
        let disc_resp = DiscoveryPacket::Response {
            ssrc,
            address: "127.0.0.1".to_owned(),
            port: client_addr.port(),
        }
        .encode()
        .expect("encode discovery response");
        responder
            .send_to(&disc_resp, client_addr)
            .expect("send discovery response");

        // 2. Принимаем два SRTP аудио-кадра, отправленных через flush_outgoing_udp
        let (n_srtp_1, peer_1) = responder.recv_from(&mut buf).expect("recv SRTP 1");
        assert_eq!(peer_1, client_addr);
        let srtp_pkt_1 = buf[..n_srtp_1].to_vec();

        let (n_srtp_2, peer_2) = responder.recv_from(&mut buf).expect("recv SRTP 2");
        assert_eq!(peer_2, client_addr);
        let srtp_pkt_2 = buf[..n_srtp_2].to_vec();

        // 3. Принимаем 8-байтный и 12-байтный UDP keepalive пакеты
        let (n_ka8, _) = responder.recv_from(&mut buf).expect("recv 8B keepalive");
        let ka8 = buf[..n_ka8].to_vec();

        let (n_ka12, _) = responder.recv_from(&mut buf).expect("recv 12B keepalive");
        let ka12 = buf[..n_ka12].to_vec();

        (srtp_pkt_1, srtp_pkt_2, ka8, ka12)
    });

    let mut driver = sample_driver_in_ready(server_port, ssrc);
    let cfg = VoiceUdpTransportConfig {
        bind_addr: "127.0.0.1:0".to_owned(),
        read_timeout: Duration::from_millis(500),
        write_timeout: Duration::from_millis(500),
        max_discovery_attempts: 2,
        keepalive_interval: Duration::from_secs(5),
    };

    let (mut socket, _select): (LiveUdpVoiceSocket, _) = driver
        .perform_udp_discovery(&cfg)
        .expect("perform_udp_discovery");
    assert_eq!(driver.phase(), VoiceSessionPhase::SelectingProtocol);

    // Переводим сессию в Established через OP 4 SESSION_DESCRIPTION
    driver
        .handle_session_description(SessionDescription {
            mode: "aead_xchacha20_poly1305_rtpsize".to_owned(),
            secret_key: SAMPLE_SECRET_KEY.to_vec(),
            media_session_id: "media-session-live-01".to_owned(),
            dave_protocol_version: 1,
            secure_frames_version: 1,
            audio_codec: Some("opus".to_owned()),
            video_codec: Some("AV1".to_owned()),
        })
        .expect("handle OP 4 SESSION_DESCRIPTION");
    assert_eq!(driver.phase(), VoiceSessionPhase::Established);

    // Шифруем два Opus-кадра (первый 6 байт без расширения, второй 10 байт с расширением 0xBEDE)
    let opus_1 = b"\xf8\xff\xfe\x01\x02\x03";
    let ext_2 = [0x10, 0xff, 0x00, 0x00];
    let opus_2 = b"\xf8\xff\xfe\x10\x20\x30\x40\x50\x60\x70";

    let enc_1 = driver
        .encrypt_audio_frame(opus_1, None)
        .expect("encrypt_audio_frame 1");
    let enc_2 = driver
        .encrypt_audio_frame(opus_2, Some(&ext_2))
        .expect("encrypt_audio_frame 2");

    // 12B RTP header + 6B payload + 16B AEAD tag + 4B nonce trailer = 38 bytes
    assert_eq!(enc_1.len(), 12 + opus_1.len() + 16 + 4);
    // 12B RTP header + 4B (0xBEDE profile+len) + 4B ext + 10B payload + 16B AEAD tag + 4B trailer = 50 bytes
    assert_eq!(enc_2.len(), 12 + 4 + ext_2.len() + opus_2.len() + 16 + 4);

    // Сбрасываем очередь исходящих UDP-пакетов драйвера в живой сокет
    let flushed_count = driver
        .flush_outgoing_udp(&mut socket)
        .expect("flush_outgoing_udp");
    assert_eq!(flushed_count, 2);
    assert!(driver.take_outgoing_udp().is_empty());

    // Отправляем 8-байтный и 12-байтный UDP keepalive пинги
    let sent_ka8 = driver
        .send_udp_keepalive(&mut socket)
        .expect("send 8-byte UDP keepalive");
    assert_eq!(sent_ka8.len(), KEEPALIVE_LEN);
    assert_eq!(&sent_ka8[..4], &KEEPALIVE_REQUEST[..4]);
    assert_eq!(socket.keepalive_sequence(), 1);

    let sent_ka12 = socket
        .send_udp_keepalive_12b(123_456)
        .expect("send 12-byte UDP keepalive ping");
    assert_eq!(sent_ka12.len(), 12);
    assert_eq!(&sent_ka12[..4], &KEEPALIVE_REQUEST[..4]);
    assert_eq!(socket.keepalive_sequence(), 2);

    // 1 discovery (74B) + 2 SRTP (38B + 50B) + 2 keepalives (8B + 12B) = 5 пакетов, 182 байта
    assert_eq!(socket.packets_sent(), 5);
    assert_eq!(socket.bytes_sent(), 74 + 38 + 50 + 8 + 12);
    assert_eq!(socket.peer_addr().port(), server_port);

    let (wire_srtp_1, wire_srtp_2, wire_ka8, wire_ka12) =
        responder_handle.join().expect("responder join");

    // Проверяем побайтовое совпадение и расшифровку независимым SrtpContext на стороне приёмника
    assert_eq!(wire_srtp_1, enc_1);
    assert_eq!(wire_srtp_2, enc_2);

    let mut receiver_srtp =
        SrtpContext::from_secret_key(&SAMPLE_SECRET_KEY, EncryptionMode::XChaCha20Poly1305RtpSize);
    let plain_rtp_1 = receiver_srtp
        .decrypt_rtp(&wire_srtp_1)
        .expect("receiver decrypts SRTP 1");
    assert_eq!(&plain_rtp_1[12..], opus_1);

    let plain_rtp_2 = receiver_srtp
        .decrypt_rtp(&wire_srtp_2)
        .expect("receiver decrypts SRTP 2");
    assert_eq!(&plain_rtp_2[20..], opus_2);

    let parsed_ka8 = KeepalivePacket::parse(&wire_ka8).expect("valid 8B keepalive on wire");
    assert_eq!(parsed_ka8, KeepalivePacket::Request { sequence: 1 });
    assert_eq!(wire_ka12, sent_ka12);
    assert_eq!(
        u32::from_le_bytes([wire_ka12[4], wire_ka12[5], wire_ka12[6], wire_ka12[7]]),
        2
    );
    assert_eq!(
        u32::from_le_bytes([wire_ka12[8], wire_ka12[9], wire_ka12[10], wire_ka12[11]]),
        123_456
    );
}
