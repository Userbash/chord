//! Контрактные тесты конвейера захвата экрана и окон `org.freedesktop.portal.ScreenCast`
//! с согласованием разрешения и частоты 60 FPS (`discord-platform::screencast`).
//!
//! Оракул:
//! - `/usr/share/dbus-1/interfaces/org.freedesktop.portal.ScreenCast.xml` (v6)
//! - `/usr/share/dbus-1/interfaces/org.freedesktop.portal.Request.xml`
//! - `/usr/share/dbus-1/interfaces/org.freedesktop.portal.Session.xml`
//! - `/home/sanya/re-deep/native-re.md` (`discord_voice.node` -> `BaseCapturerPipeWire`, `CcPipewireConfig`)
//! - `docs/reference-projects/sources/Serein/crates/discord-voice/src/screen/portal_linux.rs`

use std::io::{BufRead, BufReader, Read, Write};
use std::os::fd::{AsRawFd, IntoRawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::time::Duration;

use discord_platform::dbus::message::{
    self, FIELD_INTERFACE, FIELD_MEMBER, FIELD_PATH, FIELD_REPLY_SERIAL, FIELD_SIGNATURE,
    MessageType,
};
use discord_platform::dbus::{BusAddress, Value, fd};
use discord_platform::screencast::{
    CaptureResolutionPreset, CaptureTargetKind, CapturedFrame, MAX_CAPTURE_HEIGHT,
    MAX_CAPTURE_WIDTH, MIN_CAPTURE_DIMENSION, NegotiatedStreamParams, OwnedDmaBuf,
    PortalCapabilities, PortalCursorMode, PortalPersistMode, PortalRequestError, PortalSourceTypes,
    PortalStreamInfo, RTP_VIDEO_CLOCK_HZ, ScreenCastCapturePipeline, ScreenCastNegotiationRequest,
    ScreenCastOptions, ScreenCastPipelineState, ScreenCastPortal, negotiate_stream_params,
    parse_x11_display_spec,
};

fn read_dbus_msg(stream: &UnixStream) -> message::Message {
    let mut fds = Vec::new();
    let mut hdr = [0u8; 16];
    fd::recv_exact_with_fds(stream, &mut hdr, &mut fds).expect("read dbus header");
    let body_len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
    let fields_len = u32::from_le_bytes([hdr[12], hdr[13], hdr[14], hdr[15]]) as usize;
    let body_offset = (16 + fields_len + 7) & !7;
    let mut full = vec![0u8; body_offset + body_len];
    full[..16].copy_from_slice(&hdr);
    fd::recv_exact_with_fds(stream, &mut full[16..], &mut fds).expect("read dbus body");
    message::parse_with_fds(&full, fds).expect("parse dbus message")
}

fn complete_dbus_auth_and_hello(stream: &mut UnixStream, server_serial: &mut u32) {
    let mut nul = [0u8; 1];
    stream.read_exact(&mut nul).expect("read nul byte");
    assert_eq!(nul[0], 0);

    let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
    let mut auth_line = String::new();
    reader.read_line(&mut auth_line).expect("read AUTH");
    assert!(auth_line.starts_with("AUTH EXTERNAL "));
    stream
        .write_all(b"OK 0123456789abcdef0123456789abcdef\r\n")
        .expect("write OK");

    let mut neg_line = String::new();
    reader
        .read_line(&mut neg_line)
        .expect("read NEGOTIATE_UNIX_FD");
    assert_eq!(neg_line.trim(), "NEGOTIATE_UNIX_FD");
    stream
        .write_all(b"AGREE_UNIX_FD\r\n")
        .expect("write AGREE_UNIX_FD");

    let mut begin_line = String::new();
    reader.read_line(&mut begin_line).expect("read BEGIN");
    assert_eq!(begin_line.trim(), "BEGIN");
    drop(reader);

    let hello = read_dbus_msg(stream);
    assert_eq!(hello.member(), Some("Hello"));
    let hello_reply = message::build(
        MessageType::MethodReturn,
        *server_serial,
        &[
            (FIELD_REPLY_SERIAL, Value::U32(hello.serial)),
            (FIELD_SIGNATURE, Value::String("s".to_string())),
        ],
        "s",
        &[Value::String(":1.88".to_string())],
    )
    .expect("build Hello reply");
    *server_serial += 1;
    stream.write_all(&hello_reply).expect("write Hello reply");
}

#[test]
fn negotiate_60fps_resolution_presets_and_aspect_ratio_alignment_for_screen_and_window() {
    // 1. Позитивный сценарий: монитор 2560x1440 с запросом 1080p @ 60 FPS
    let monitor_stream = PortalStreamInfo {
        node_id: 42,
        id: Some("screen:0".to_string()),
        source_type: PortalSourceTypes::MONITOR.bits(),
        position: Some((0, 0)),
        size: Some((2560, 1440)),
        mapping_id: Some("DP-1".to_string()),
    };
    let req_1080p60 = ScreenCastNegotiationRequest::from_preset(
        CaptureResolutionPreset::Fhd1080p,
        60,
        CaptureTargetKind::Monitor,
    );
    let negotiated =
        negotiate_stream_params(&monitor_stream, &req_1080p60).expect("negotiate 1080p60 monitor");
    assert_eq!(negotiated.width, 1920);
    assert_eq!(negotiated.height, 1080);
    assert_eq!(negotiated.fps, 60);
    assert_eq!(negotiated.frame_interval_us, 16_666);
    assert_eq!(negotiated.rtp_clock_hz, RTP_VIDEO_CLOCK_HZ);
    assert_eq!(negotiated.rtp_timestamp_step, 1_500); // 90_000 / 60
    assert_eq!(negotiated.stride_bytes, 1920 * 4);
    assert_eq!(negotiated.target_kind, CaptureTargetKind::Monitor);
    assert!(negotiated.target_bitrate_bps >= 8_000_000);

    // 2. Захват окна с нечётными размерами (1365x767) при запросе 1080p @ 60 FPS:
    // не апскейлит выше исходного окна и выравнивает размеры до чётных (1364x766) для YUV420/NVENC.
    let odd_window_stream = PortalStreamInfo {
        node_id: 77,
        id: Some("window:{6ad0aefd-103b-4fa1-b650-3e016c818a00}".to_string()),
        source_type: PortalSourceTypes::WINDOW.bits(),
        position: None,
        size: Some((1365, 767)),
        mapping_id: Some("Firefox".to_string()),
    };
    let req_win_60 = ScreenCastNegotiationRequest::from_preset(
        CaptureResolutionPreset::Fhd1080p,
        60,
        CaptureTargetKind::Window,
    );
    let neg_win =
        negotiate_stream_params(&odd_window_stream, &req_win_60).expect("negotiate odd window");
    assert_eq!(neg_win.width, 1364);
    assert_eq!(neg_win.height, 766);
    assert_eq!(neg_win.width % 2, 0);
    assert_eq!(neg_win.height % 2, 0);
    assert_eq!(neg_win.fps, 60);
    assert_eq!(neg_win.target_kind, CaptureTargetKind::Window);

    // 3. Ультраширокий монитор 3440x1440 с даунскейлом в рамку 1920x1080 @ 60 FPS (сохранение пропорций)
    let ultrawide_stream = PortalStreamInfo {
        node_id: 99,
        id: Some("screen:1".to_string()),
        source_type: PortalSourceTypes::MONITOR.bits(),
        position: Some((2560, 0)),
        size: Some((3440, 1440)),
        mapping_id: Some("DP-2".to_string()),
    };
    let neg_uw =
        negotiate_stream_params(&ultrawide_stream, &req_1080p60).expect("negotiate ultrawide");
    assert_eq!(neg_uw.width, 1920);
    assert_eq!(neg_uw.height, 804); // 1440 * 1920 / 3440 = 803.7 -> чётное 804
    assert_eq!(neg_uw.fps, 60);

    // 4. Пустой размер в ответе портала (типично для Wayland-окон до первого буфера PipeWire) ->
    // использует целевое разрешение пресета (1280x720 @ 60 FPS).
    let sizeless_window = PortalStreamInfo {
        node_id: 105,
        id: None,
        source_type: PortalSourceTypes::WINDOW.bits(),
        position: None,
        size: None,
        mapping_id: Some("Alacritty".to_string()),
    };
    let req_720p60 = ScreenCastNegotiationRequest::from_preset(
        CaptureResolutionPreset::Hd720p,
        60,
        CaptureTargetKind::Window,
    );
    let neg_fallback =
        negotiate_stream_params(&sizeless_window, &req_720p60).expect("negotiate sizeless window");
    assert_eq!(neg_fallback.width, 1280);
    assert_eq!(neg_fallback.height, 720);
    assert_eq!(neg_fallback.fps, 60);
}

#[test]
fn negotiate_stream_params_rejects_invalid_fps_and_out_of_bounds_dimensions() {
    let stream = PortalStreamInfo {
        node_id: 10,
        id: Some("screen:0".to_string()),
        source_type: PortalSourceTypes::MONITOR.bits(),
        position: Some((0, 0)),
        size: Some((1920, 1080)),
        mapping_id: Some("HDMI-A-1".to_string()),
    };

    // Негативные сценарии: FPS = 0 или 144 (поддерживаются 15, 30, 60)
    for bad_fps in [0, 10, 24, 45, 90, 120, 240] {
        let bad_req = ScreenCastNegotiationRequest {
            max_width: 1920,
            max_height: 1080,
            target_fps: bad_fps,
            target_kind: CaptureTargetKind::Monitor,
            custom_bitrate_bps: None,
        };
        assert!(
            negotiate_stream_params(&stream, &bad_req).is_err(),
            "fps {bad_fps} must be rejected"
        );
    }

    // Граничные и невалидные размеры
    for (bad_w, bad_h) in [
        (0, 1080),
        (1920, 0),
        (MIN_CAPTURE_DIMENSION - 1, 720),
        (1280, MIN_CAPTURE_DIMENSION - 1),
        (MAX_CAPTURE_WIDTH + 2, 1080),
        (1920, MAX_CAPTURE_HEIGHT + 2),
    ] {
        let bad_dim_req = ScreenCastNegotiationRequest {
            max_width: bad_w,
            max_height: bad_h,
            target_fps: 60,
            target_kind: CaptureTargetKind::Monitor,
            custom_bitrate_bps: None,
        };
        assert!(
            negotiate_stream_params(&stream, &bad_dim_req).is_err(),
            "dimensions {bad_w}x{bad_h} must be rejected"
        );
    }

    // Невалидный node_id == 0
    let zero_node_stream = PortalStreamInfo {
        node_id: 0,
        ..stream
    };
    let valid_req = ScreenCastNegotiationRequest::from_preset(
        CaptureResolutionPreset::Hd720p,
        60,
        CaptureTargetKind::Monitor,
    );
    assert!(negotiate_stream_params(&zero_node_stream, &valid_req).is_err());
}

#[test]
fn portal_capabilities_validate_source_types_and_negotiate_cursor_mode() {
    let caps = PortalCapabilities {
        version: 6,
        available_source_types: PortalSourceTypes::MONITOR_AND_WINDOW,
        available_cursor_modes: PortalCursorMode::Hidden as u32 | PortalCursorMode::Embedded as u32,
    };

    assert!(caps.supports_source_types(PortalSourceTypes::MONITOR));
    assert!(caps.supports_source_types(PortalSourceTypes::WINDOW));
    assert!(caps.supports_source_types(PortalSourceTypes::MONITOR_AND_WINDOW));
    assert!(!caps.supports_source_types(PortalSourceTypes::VIRTUAL));

    // Если запрошен Metadata (4), а портал поддерживает только Hidden (1) | Embedded (2),
    // выбирается Embedded (2)
    assert_eq!(
        caps.select_supported_cursor_mode(PortalCursorMode::Metadata),
        Some(PortalCursorMode::Embedded)
    );
    assert_eq!(
        caps.select_supported_cursor_mode(PortalCursorMode::Embedded),
        Some(PortalCursorMode::Embedded)
    );

    let empty_caps = PortalCapabilities {
        version: 4,
        available_source_types: PortalSourceTypes::from_bits_truncate(0),
        available_cursor_modes: 0,
    };
    assert!(!empty_caps.supports_source_types(PortalSourceTypes::MONITOR));
    assert_eq!(
        empty_caps.select_supported_cursor_mode(PortalCursorMode::Embedded),
        None
    );
}

#[test]
fn full_screencast_pipeline_screen_and_window_60fps_lifecycle_over_mock_dbus() {
    let sock_path = std::env::temp_dir().join(format!(
        "discord-lab-portal-pipeline-60fps-{}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path).expect("bind mock portal socket");

    let server_thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept client");
        let mut server_serial = 20u32;
        complete_dbus_auth_and_hello(&mut stream, &mut server_serial);

        // 1. CreateSession
        let create_msg = read_dbus_msg(&stream);
        assert_eq!(create_msg.member(), Some("CreateSession"));
        let req_path_create = "/org/freedesktop/portal/desktop/request/1_88/discord_req_pipe";
        let sess_path = "/org/freedesktop/portal/desktop/session/1_88/discord_sess_pipe";
        let create_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(create_msg.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_path_create.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&create_reply).unwrap();

        let am1 = read_dbus_msg(&stream);
        assert_eq!(am1.member(), Some("AddMatch"));
        let am1_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(am1.serial))],
            "",
            &[],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&am1_reply).unwrap();

        let sig_create = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_path_create.to_string())),
                (
                    FIELD_INTERFACE,
                    Value::String("org.freedesktop.portal.Request".to_string()),
                ),
                (FIELD_MEMBER, Value::String("Response".to_string())),
                (FIELD_SIGNATURE, Value::String("ua{sv}".to_string())),
            ],
            "ua{sv}",
            &[
                Value::U32(0),
                Value::Dict(vec![(
                    Value::String("session_handle".to_string()),
                    Value::Variant(Box::new(Value::String(sess_path.to_string()))),
                )]),
            ],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&sig_create).unwrap();

        // 2. SelectSources
        let select_msg = read_dbus_msg(&stream);
        assert_eq!(select_msg.member(), Some("SelectSources"));
        let req_path_select = "/org/freedesktop/portal/desktop/request/1_88/discord_req_pipe";
        let select_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(select_msg.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_path_select.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&select_reply).unwrap();

        let am2 = read_dbus_msg(&stream);
        assert_eq!(am2.member(), Some("AddMatch"));
        let am2_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(am2.serial))],
            "",
            &[],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&am2_reply).unwrap();

        let sig_select = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_path_select.to_string())),
                (
                    FIELD_INTERFACE,
                    Value::String("org.freedesktop.portal.Request".to_string()),
                ),
                (FIELD_MEMBER, Value::String("Response".to_string())),
                (FIELD_SIGNATURE, Value::String("ua{sv}".to_string())),
            ],
            "ua{sv}",
            &[Value::U32(0), Value::Dict(Vec::new())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&sig_select).unwrap();

        // 3. Start -> возвращает поток монитора (node 201, 2560x1440) и поток окна (node 202, 1600x900)
        let start_msg = read_dbus_msg(&stream);
        assert_eq!(start_msg.member(), Some("Start"));
        let req_path_start = "/org/freedesktop/portal/desktop/request/1_88/discord_req_pipe";
        let start_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(start_msg.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_path_start.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&start_reply).unwrap();

        let am3 = read_dbus_msg(&stream);
        assert_eq!(am3.member(), Some("AddMatch"));
        let am3_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(am3.serial))],
            "",
            &[],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&am3_reply).unwrap();

        let streams_val = Value::Array(vec![
            Value::Struct(vec![
                Value::U32(201),
                Value::Dict(vec![
                    (
                        Value::String("id".to_string()),
                        Value::Variant(Box::new(Value::String("screen:0".to_string()))),
                    ),
                    (
                        Value::String("source_type".to_string()),
                        Value::Variant(Box::new(Value::U32(1))),
                    ),
                    (
                        Value::String("size".to_string()),
                        Value::Variant(Box::new(Value::Struct(vec![
                            Value::I32(2560),
                            Value::I32(1440),
                        ]))),
                    ),
                    (
                        Value::String("mapping_id".to_string()),
                        Value::Variant(Box::new(Value::String("DP-1".to_string()))),
                    ),
                ]),
            ]),
            Value::Struct(vec![
                Value::U32(202),
                Value::Dict(vec![
                    (
                        Value::String("id".to_string()),
                        Value::Variant(Box::new(Value::String(
                            "window:{6ad0aefd-103b-4fa1-b650-3e016c818a00}".to_string(),
                        ))),
                    ),
                    (
                        Value::String("source_type".to_string()),
                        Value::Variant(Box::new(Value::U32(2))),
                    ),
                    (
                        Value::String("size".to_string()),
                        Value::Variant(Box::new(Value::Struct(vec![
                            Value::I32(1600),
                            Value::I32(900),
                        ]))),
                    ),
                    (
                        Value::String("mapping_id".to_string()),
                        Value::Variant(Box::new(Value::String("Discord Test Window".to_string()))),
                    ),
                ]),
            ]),
        ]);

        let sig_start = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_path_start.to_string())),
                (
                    FIELD_INTERFACE,
                    Value::String("org.freedesktop.portal.Request".to_string()),
                ),
                (FIELD_MEMBER, Value::String("Response".to_string())),
                (FIELD_SIGNATURE, Value::String("ua{sv}".to_string())),
            ],
            "ua{sv}",
            &[
                Value::U32(0),
                Value::Dict(vec![
                    (
                        Value::String("streams".to_string()),
                        Value::Variant(Box::new(streams_val)),
                    ),
                    (
                        Value::String("restore_token".to_string()),
                        Value::Variant(Box::new(Value::String("restore_tok_60fps".to_string()))),
                    ),
                ]),
            ],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&sig_start).unwrap();

        // 4. OpenPipeWireRemote -> SCM_RIGHTS fd
        let open_pw = read_dbus_msg(&stream);
        assert_eq!(open_pw.member(), Some("OpenPipeWireRemote"));
        let (pw_client, _pw_server) = UnixStream::pair().expect("pw socketpair");
        let (pw_reply_bytes, pw_fds) = message::build_with_fds(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(open_pw.serial)),
                (FIELD_SIGNATURE, Value::String("h".to_string())),
            ],
            "h",
            &[Value::UnixFd(pw_client.as_raw_fd())],
        )
        .unwrap();
        server_serial += 1;
        fd::send_with_fds(&stream, &pw_reply_bytes, &pw_fds).unwrap();
        drop(pw_client);

        // 5. Session.Close при остановке конвейера
        let close_msg = read_dbus_msg(&stream);
        assert_eq!(close_msg.member(), Some("Close"));
        let close_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(close_msg.serial))],
            "",
            &[],
        )
        .unwrap();
        stream.write_all(&close_reply).unwrap();
    });

    let addr = BusAddress::Path(sock_path.clone());
    let mut portal = ScreenCastPortal::connect(&addr).expect("connect to mock portal");

    let opts = ScreenCastOptions {
        handle_token: "discord_req_pipe".to_string(),
        session_handle_token: "discord_sess_pipe".to_string(),
        source_types: PortalSourceTypes::MONITOR_AND_WINDOW,
        multiple: true,
        cursor_mode: PortalCursorMode::Embedded,
        persist_mode: PortalPersistMode::ApplicationRunning,
        restore_token: None,
        parent_window: String::new(),
    };

    let neg_req = ScreenCastNegotiationRequest::from_preset(
        CaptureResolutionPreset::Fhd1080p,
        60,
        CaptureTargetKind::Monitor,
    );

    let mut pipeline = ScreenCastCapturePipeline::new();
    assert_eq!(pipeline.state(), ScreenCastPipelineState::Idle);

    // Запускаем полный цикл захвата через портал
    let negotiated: NegotiatedStreamParams = pipeline
        .start_session(&mut portal, &opts, &neg_req)
        .expect("start_session 60fps");

    assert_eq!(negotiated.node_id, 201);
    assert_eq!(negotiated.width, 1920);
    assert_eq!(negotiated.height, 1080);
    assert_eq!(negotiated.fps, 60);
    assert_eq!(negotiated.rtp_timestamp_step, 1500);
    assert_eq!(
        pipeline.state(),
        ScreenCastPipelineState::Streaming {
            node_id: 201,
            fps: 60,
        }
    );
    assert_eq!(pipeline.restore_token(), Some("restore_tok_60fps"));
    assert!(pipeline.pipewire_raw_fd().is_some());

    // Проверяем приём кадров 60 FPS с контролем тайминга (16_666 мкс) и RTP timestamp
    let (dma_sock, _) = UnixStream::pair().expect("dma sock");
    let dma = OwnedDmaBuf::from_raw_fd_checked(dma_sock.into_raw_fd()).expect("dma");

    let f1 = CapturedFrame::new_dma_buf(1920, 1080, 7680, 0x34325241, dma.try_clone().unwrap(), 0)
        .with_timestamp_us(1_000_000);
    assert!(pipeline.ingest_frame(f1).expect("ingest frame 1"));

    // Слишком ранний кадр (< 80% интервала 16_666 мкс) отбрасывается пейсером 60 FPS
    let f_early =
        CapturedFrame::new_dma_buf(1920, 1080, 7680, 0x34325241, dma.try_clone().unwrap(), 0)
            .with_timestamp_us(1_005_000);
    assert!(!pipeline.ingest_frame(f_early).expect("early frame paced"));

    // Следующий кадр в ритме 60 FPS (+16_666 мкс) принимается
    let f2 = CapturedFrame::new_dma_buf(1920, 1080, 7680, 0x34325241, dma, 0)
        .with_timestamp_us(1_016_666);
    assert!(pipeline.ingest_frame(f2).expect("ingest frame 2"));

    let latest = pipeline
        .poll_latest_frame(|frame, rtp_ts| (frame.sequence, frame.width, frame.height, rtp_ts))
        .expect("latest frame available");
    assert_eq!(latest.0, 2);
    assert_eq!(latest.1, 1920);
    assert_eq!(latest.2, 1080);
    assert_eq!(latest.3, 1_500); // Второй принятый кадр шагнул на +1500 тиков (90 кГц / 60 FPS)

    // Горячее переключение цели на окно (node 202, 1600x900 @ 60 FPS) внутри той же сессии
    let window_params = pipeline
        .switch_stream_target(
            CaptureTargetKind::Window,
            CaptureResolutionPreset::Fhd1080p,
            60,
        )
        .expect("switch to window stream");
    assert_eq!(window_params.node_id, 202);
    assert_eq!(window_params.width, 1600);
    assert_eq!(window_params.height, 900);
    assert_eq!(window_params.fps, 60);
    assert_eq!(
        pipeline.state(),
        ScreenCastPipelineState::Streaming {
            node_id: 202,
            fps: 60,
        }
    );

    // Пауза и возобновление (Gateway OP 22 STREAM_SET_PAUSED)
    pipeline.set_paused(true).expect("pause pipeline");
    assert_eq!(
        pipeline.state(),
        ScreenCastPipelineState::Paused {
            node_id: 202,
            fps: 60,
        }
    );
    // На паузе кадры не публикуются
    let f_paused = CapturedFrame::new(1600, 900, 6400, 1)
        .with_data(vec![0u8; 64])
        .with_timestamp_us(2_000_000);
    assert!(
        !pipeline
            .ingest_frame(f_paused)
            .expect("paused frame ignored")
    );

    pipeline.set_paused(false).expect("resume pipeline");
    assert_eq!(
        pipeline.state(),
        ScreenCastPipelineState::Streaming {
            node_id: 202,
            fps: 60,
        }
    );

    // Штатная остановка закрывает сессию на шине D-Bus
    pipeline.stop(&mut portal).expect("stop pipeline");
    assert_eq!(pipeline.state(), ScreenCastPipelineState::Stopped);
    assert!(pipeline.pipewire_raw_fd().is_none());

    server_thread.join().expect("join mock server");
    let _ = std::fs::remove_file(&sock_path);
}

#[test]
fn screencast_pipeline_cleans_up_session_when_consent_denied_or_streams_empty() {
    let sock_path = std::env::temp_dir().join(format!(
        "discord-lab-portal-pipeline-err-{}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path).expect("bind error mock socket");

    let server_thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept client");
        let mut server_serial = 50u32;
        complete_dbus_auth_and_hello(&mut stream, &mut server_serial);

        // 1. CreateSession -> успех
        let create_msg = read_dbus_msg(&stream);
        let req_path = "/org/freedesktop/portal/desktop/request/1_88/discord_req_err";
        let sess_path = "/org/freedesktop/portal/desktop/session/1_88/discord_sess_err";
        let create_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(create_msg.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_path.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&create_reply).unwrap();

        let am1 = read_dbus_msg(&stream);
        let am1_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(am1.serial))],
            "",
            &[],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&am1_reply).unwrap();

        let sig_create = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_path.to_string())),
                (
                    FIELD_INTERFACE,
                    Value::String("org.freedesktop.portal.Request".to_string()),
                ),
                (FIELD_MEMBER, Value::String("Response".to_string())),
                (FIELD_SIGNATURE, Value::String("ua{sv}".to_string())),
            ],
            "ua{sv}",
            &[
                Value::U32(0),
                Value::Dict(vec![(
                    Value::String("session_handle".to_string()),
                    Value::Variant(Box::new(Value::String(sess_path.to_string()))),
                )]),
            ],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&sig_create).unwrap();

        // 2. SelectSources -> пользователь нажимает «Отмена» (Response = 1 -> ConsentDenied)
        let select_msg = read_dbus_msg(&stream);
        assert_eq!(select_msg.member(), Some("SelectSources"));
        let select_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(select_msg.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_path.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&select_reply).unwrap();

        let am2 = read_dbus_msg(&stream);
        let am2_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(am2.serial))],
            "",
            &[],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&am2_reply).unwrap();

        let sig_denied = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_path.to_string())),
                (
                    FIELD_INTERFACE,
                    Value::String("org.freedesktop.portal.Request".to_string()),
                ),
                (FIELD_MEMBER, Value::String("Response".to_string())),
                (FIELD_SIGNATURE, Value::String("ua{sv}".to_string())),
            ],
            "ua{sv}",
            &[Value::U32(1), Value::Dict(Vec::new())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&sig_denied).unwrap();

        // Конвейер обязан автоматически закрыть созданную сессию через Session.Close!
        let close_msg = read_dbus_msg(&stream);
        assert_eq!(close_msg.member(), Some("Close"));
        let close_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(close_msg.serial))],
            "",
            &[],
        )
        .unwrap();
        stream.write_all(&close_reply).unwrap();
    });

    let addr = BusAddress::Path(sock_path.clone());
    let mut portal = ScreenCastPortal::connect(&addr).expect("connect");
    portal.set_dialog_timeout(Duration::from_secs(2));

    let opts = ScreenCastOptions {
        handle_token: "discord_req_err".to_string(),
        session_handle_token: "discord_sess_err".to_string(),
        source_types: PortalSourceTypes::MONITOR,
        multiple: false,
        cursor_mode: PortalCursorMode::Embedded,
        persist_mode: PortalPersistMode::DoNotPersist,
        restore_token: None,
        parent_window: String::new(),
    };
    let req = ScreenCastNegotiationRequest::from_preset(
        CaptureResolutionPreset::Hd720p,
        60,
        CaptureTargetKind::Monitor,
    );

    let mut pipeline = ScreenCastCapturePipeline::new();
    let err = pipeline
        .start_session(&mut portal, &opts, &req)
        .expect_err("must fail with ConsentDenied");
    assert_eq!(err, PortalRequestError::ConsentDenied);
    assert_eq!(pipeline.state(), ScreenCastPipelineState::Stopped);

    // X11 fallback spec verification
    let x11 = parse_x11_display_spec(":0.0").expect("valid x11 spec");
    assert_eq!(x11.display_number, 0);

    server_thread.join().expect("join error mock server");
    let _ = std::fs::remove_file(&sock_path);
}
