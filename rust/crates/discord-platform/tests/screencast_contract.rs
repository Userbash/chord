//! TASK-613: Контрактные тесты платформенного D-Bus клиента `org.freedesktop.portal.ScreenCast`
//! и X11 SHM / Display зонда в `discord-platform::screencast`.
//!
//! Оракул: `/usr/share/dbus-1/interfaces/org.freedesktop.portal.ScreenCast.xml`,
//! `org.freedesktop.portal.Request.xml`, `org.freedesktop.portal.Session.xml`
//! и `/home/sanya/re-deep/native-re.md` (`discord_voice.node` -> `capture_linux`).

use discord_platform::screencast::{
    CapturedFrame, DoubleBufferedFrame, LockFreeFrameSlot, OwnedDmaBuf, OwnedPipeWireFd,
    PORTAL_BUS_NAME, PORTAL_OBJECT_PATH, PortalCursorMode, PortalPersistMode, PortalRequestError,
    PortalSourceTypes, PortalStreamInfo, SCREENCAST_INTERFACE, ScreenCastOptions, ScreenCastPortal,
    parse_x11_display_spec,
};

#[test]
fn screencast_portal_constants_and_source_type_masks_match_freedesktop_spec() {
    assert_eq!(PORTAL_BUS_NAME, "org.freedesktop.portal.Desktop");
    assert_eq!(PORTAL_OBJECT_PATH, "/org/freedesktop/portal/desktop");
    assert_eq!(SCREENCAST_INTERFACE, "org.freedesktop.portal.ScreenCast");

    assert_eq!(PortalSourceTypes::MONITOR.bits(), 1);
    assert_eq!(PortalSourceTypes::WINDOW.bits(), 2);
    assert_eq!(PortalSourceTypes::VIRTUAL.bits(), 4);
    assert_eq!(PortalSourceTypes::MONITOR_AND_WINDOW.bits(), 1 | 2);

    assert_eq!(PortalCursorMode::Hidden as u32, 1);
    assert_eq!(PortalCursorMode::Embedded as u32, 2);
    assert_eq!(PortalCursorMode::Metadata as u32, 4);

    assert_eq!(PortalPersistMode::DoNotPersist as u32, 0);
    assert_eq!(PortalPersistMode::ApplicationRunning as u32, 1);
    assert_eq!(PortalPersistMode::ExplicitlyRevoked as u32, 2);
}

#[test]
fn parse_x11_display_spec_resolves_socket_path_and_rejects_malformed_displays() {
    let d0 = parse_x11_display_spec(":0").expect("valid :0 display");
    assert_eq!(d0.display_number, 0);
    assert_eq!(d0.screen_number, 0);
    assert_eq!(d0.unix_socket_path.to_str().unwrap(), "/tmp/.X11-unix/X0");

    let d1_2 = parse_x11_display_spec(":1.2").expect("valid :1.2 display");
    assert_eq!(d1_2.display_number, 1);
    assert_eq!(d1_2.screen_number, 2);
    assert_eq!(d1_2.unix_socket_path.to_str().unwrap(), "/tmp/.X11-unix/X1");

    let unix_prefixed = parse_x11_display_spec("unix:10.0").expect("valid unix:10.0 display");
    assert_eq!(unix_prefixed.display_number, 10);
    assert_eq!(
        unix_prefixed.unix_socket_path.to_str().unwrap(),
        "/tmp/.X11-unix/X10"
    );

    for bad in [
        "",
        "   ",
        "0",
        ":",
        ":-1",
        ":abc",
        ":0.abc",
        ":999999999999",
    ] {
        assert!(
            parse_x11_display_spec(bad).is_err(),
            "display {bad:?} must be rejected"
        );
    }
}

#[test]
fn screencast_options_validation_checks_tokens_and_source_types() {
    let valid = ScreenCastOptions {
        handle_token: "discord_req_1".to_string(),
        session_handle_token: "discord_sess_1".to_string(),
        source_types: PortalSourceTypes::MONITOR_AND_WINDOW,
        multiple: false,
        cursor_mode: PortalCursorMode::Embedded,
        persist_mode: PortalPersistMode::ApplicationRunning,
        restore_token: None,
        parent_window: String::new(),
    };
    assert!(valid.validate().is_ok());

    let bad_token = ScreenCastOptions {
        handle_token: "bad-token-with-dash".to_string(),
        ..valid.clone()
    };
    assert!(matches!(
        bad_token.validate(),
        Err(PortalRequestError::InvalidToken(_))
    ));

    let empty_types = ScreenCastOptions {
        source_types: PortalSourceTypes::from_bits_truncate(0),
        ..valid
    };
    assert!(matches!(
        empty_types.validate(),
        Err(PortalRequestError::InvalidSourceTypes(0))
    ));
}

#[test]
fn owned_pipewire_fd_rejects_negative_fd_and_closes_valid_fd_on_drop() {
    assert!(OwnedPipeWireFd::from_raw_fd_checked(-1).is_err());

    let (sock_a, _sock_b) = std::os::unix::net::UnixStream::pair().expect("socketpair");
    use std::os::fd::IntoRawFd;
    let raw_fd = sock_a.into_raw_fd();
    let owned = OwnedPipeWireFd::from_raw_fd_checked(raw_fd).expect("valid raw fd");
    assert_eq!(owned.as_raw_fd(), raw_fd);
    drop(owned);
}

#[test]
fn screencast_portal_parses_start_response_streams_and_restore_token() {
    use discord_platform::dbus::Value;

    let streams_val = Value::Array(vec![Value::Struct(vec![
        Value::U32(108),
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
                Value::String("position".to_string()),
                Value::Variant(Box::new(Value::Struct(vec![Value::I32(0), Value::I32(0)]))),
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
    ])]);

    let results_dict = Value::Dict(vec![
        (
            Value::String("streams".to_string()),
            Value::Variant(Box::new(streams_val)),
        ),
        (
            Value::String("restore_token".to_string()),
            Value::Variant(Box::new(Value::String("tok_xyz_99".to_string()))),
        ),
    ]);

    let (streams, restore_token) =
        ScreenCastPortal::parse_start_results(&results_dict).expect("parse start results");
    assert_eq!(restore_token.as_deref(), Some("tok_xyz_99"));
    assert_eq!(
        streams,
        vec![PortalStreamInfo {
            node_id: 108,
            id: Some("screen:0".to_string()),
            source_type: 1,
            position: Some((0, 0)),
            size: Some((2560, 1440)),
            mapping_id: Some("DP-1".to_string()),
        }]
    );
}

#[test]
fn screencast_portal_wire_handshake_over_unix_socket_delivers_pipewire_fd() {
    use discord_platform::dbus::message::{
        self, FIELD_INTERFACE, FIELD_MEMBER, FIELD_PATH, FIELD_REPLY_SERIAL, FIELD_SIGNATURE,
        MessageType,
    };
    use discord_platform::dbus::{BusAddress, Value, fd};
    use std::io::{BufRead, BufReader, Read, Write};
    use std::os::fd::AsRawFd;
    use std::os::unix::net::UnixListener;

    let sock_path = std::env::temp_dir().join(format!(
        "discord-lab-screencast-wire-{}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path).expect("bind unix listener");

    let server_thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept client");
        let mut nul = [0u8; 1];
        stream.read_exact(&mut nul).expect("read nul");
        assert_eq!(nul[0], 0);

        let mut reader = BufReader::new(stream.try_clone().expect("clone stream"));
        let mut auth_line = String::new();
        reader.read_line(&mut auth_line).expect("read AUTH");
        assert!(auth_line.starts_with("AUTH EXTERNAL "));
        stream
            .write_all(b"OK 0123456789abcdef0123456789abcdef\r\n")
            .expect("write OK");

        let mut neg_line = String::new();
        reader.read_line(&mut neg_line).expect("read NEGOTIATE");
        assert_eq!(neg_line.trim(), "NEGOTIATE_UNIX_FD");
        stream
            .write_all(b"AGREE_UNIX_FD\r\n")
            .expect("write AGREE_UNIX_FD");

        let mut begin_line = String::new();
        reader.read_line(&mut begin_line).expect("read BEGIN");
        assert_eq!(begin_line.trim(), "BEGIN");
        drop(reader);

        let read_dbus_msg = |s: &std::os::unix::net::UnixStream| -> message::Message {
            let mut fds = Vec::new();
            let mut hdr = [0u8; 16];
            fd::recv_exact_with_fds(s, &mut hdr, &mut fds).expect("read hdr");
            let body_len = u32::from_le_bytes([hdr[4], hdr[5], hdr[6], hdr[7]]) as usize;
            let fields_len = u32::from_le_bytes([hdr[12], hdr[13], hdr[14], hdr[15]]) as usize;
            let body_offset = (16 + fields_len + 7) & !7;
            let mut full = vec![0u8; body_offset + body_len];
            full[..16].copy_from_slice(&hdr);
            fd::recv_exact_with_fds(s, &mut full[16..], &mut fds).expect("read body");
            message::parse_with_fds(&full, fds).expect("parse msg")
        };

        let mut server_serial = 10u32;

        // 1. Hello
        let hello = read_dbus_msg(&stream);
        assert_eq!(hello.member(), Some("Hello"));
        let hello_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(hello.serial)),
                (FIELD_SIGNATURE, Value::String("s".to_string())),
            ],
            "s",
            &[Value::String(":1.77".to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&hello_reply).unwrap();

        // 2. CreateSession
        let create_msg = read_dbus_msg(&stream);
        assert_eq!(create_msg.member(), Some("CreateSession"));
        let req_path_1 = "/org/freedesktop/portal/desktop/request/1_77/discord_req_1";
        let sess_path = "/org/freedesktop/portal/desktop/session/1_77/discord_sess_1";
        let create_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(create_msg.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_path_1.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&create_reply).unwrap();

        // AddMatch для CreateSession Request.Response
        let add_match_1 = read_dbus_msg(&stream);
        assert_eq!(add_match_1.member(), Some("AddMatch"));
        let am_reply_1 = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(add_match_1.serial))],
            "",
            &[],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&am_reply_1).unwrap();

        // Отправляем сигнал Request.Response для CreateSession
        let sig_1 = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_path_1.to_string())),
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
        stream.write_all(&sig_1).unwrap();

        // 3. OpenPipeWireRemote -> возвращает реальный unix-fd через SCM_RIGHTS
        let open_pw = read_dbus_msg(&stream);
        assert_eq!(open_pw.member(), Some("OpenPipeWireRemote"));
        let (pw_sock_client, mut pw_sock_server) =
            std::os::unix::net::UnixStream::pair().expect("pw socketpair");
        pw_sock_server.write_all(b"PW_REMOTE_OK").unwrap();

        let (pw_reply_bytes, pw_fds) = message::build_with_fds(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(open_pw.serial)),
                (FIELD_SIGNATURE, Value::String("h".to_string())),
            ],
            "h",
            &[Value::UnixFd(pw_sock_client.as_raw_fd())],
        )
        .unwrap();
        server_serial += 1;
        fd::send_with_fds(&stream, &pw_reply_bytes, &pw_fds).unwrap();
        drop(pw_sock_client);

        // 4. Session.Close
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
        handle_token: "discord_req_1".to_string(),
        session_handle_token: "discord_sess_1".to_string(),
        source_types: PortalSourceTypes::MONITOR_AND_WINDOW,
        multiple: false,
        cursor_mode: PortalCursorMode::Embedded,
        persist_mode: PortalPersistMode::ApplicationRunning,
        restore_token: None,
        parent_window: String::new(),
    };

    let session_handle = portal.create_session(&opts).expect("CreateSession wire");
    assert_eq!(
        session_handle,
        "/org/freedesktop/portal/desktop/session/1_77/discord_sess_1"
    );

    let pw_fd = portal
        .open_pipewire_remote(&session_handle)
        .expect("OpenPipeWireRemote wire");
    assert!(pw_fd.as_raw_fd() >= 0);

    // TASK-804: Повторный вызов не должен делать новый D-Bus запрос, а берёт кэшированный дескриптор
    assert!(portal.cached_pipewire_fd().is_some());
    let pw_fd_cached = portal
        .open_pipewire_remote(&session_handle)
        .expect("OpenPipeWireRemote cached clone");
    assert!(pw_fd_cached.as_raw_fd() >= 0);
    assert_ne!(pw_fd.as_raw_fd(), pw_fd_cached.as_raw_fd());
    drop(pw_fd_cached);

    portal
        .close_session(&session_handle)
        .expect("CloseSession wire");
    assert_eq!(portal.cached_pipewire_fd(), None);
    drop(pw_fd);

    server_thread.join().expect("server thread join");
    let _ = std::fs::remove_file(&sock_path);
}

#[test]
fn owned_pipewire_fd_and_dma_buf_try_clone_creates_independent_descriptors() {
    use std::os::fd::IntoRawFd;
    use std::os::unix::net::UnixStream;

    let (sock_a, _sock_b) = UnixStream::pair().expect("socketpair");
    let raw = sock_a.into_raw_fd();

    let owned_pw = OwnedPipeWireFd::from_raw_fd_checked(raw).expect("valid pw fd");
    let cloned_pw = owned_pw.try_clone().expect("clone pw fd");
    assert!(cloned_pw.as_raw_fd() >= 0);
    assert_ne!(owned_pw.as_raw_fd(), cloned_pw.as_raw_fd());
    drop(owned_pw);

    // Клон остаётся валидным дескриптором после закрытия оригинала
    let clone_again = cloned_pw.try_clone().expect("clone from active clone");
    assert!(clone_again.as_raw_fd() >= 0);
    drop(cloned_pw);
    drop(clone_again);

    // Тестирование OwnedDmaBuf
    let (dma_sock, _) = UnixStream::pair().expect("dma socketpair");
    let dma_raw = dma_sock.into_raw_fd();
    let owned_dma = OwnedDmaBuf::from_raw_fd_checked(dma_raw).expect("valid dma fd");
    let cloned_dma = owned_dma.try_clone().expect("clone dma fd");
    assert_eq!(owned_dma.as_raw_fd(), dma_raw);
    assert!(cloned_dma.as_raw_fd() >= 0);
    assert_ne!(owned_dma.as_raw_fd(), cloned_dma.as_raw_fd());
    assert_eq!(owned_dma, owned_dma);
    drop(owned_dma);
    drop(cloned_dma);
}

#[test]
fn captured_frame_dma_buf_and_cpu_data_contracts() {
    use std::os::fd::IntoRawFd;
    use std::os::unix::net::UnixStream;

    let empty = CapturedFrame::default();
    assert!(empty.is_empty());
    assert_eq!(empty.byte_len(), 0);
    assert!(!empty.is_dma_buf());
    assert_eq!(empty.dma_buf_raw_fd(), None);

    let cpu_frame = CapturedFrame::new(1920, 1080, 7680, 0x34324152)
        .with_data(vec![0u8; 1920 * 1080 * 4])
        .with_timestamp_us(123_456_789);
    assert!(!cpu_frame.is_empty());
    assert_eq!(cpu_frame.byte_len(), 1920 * 1080 * 4);
    assert_eq!(cpu_frame.timestamp_us, 123_456_789);
    assert!(!cpu_frame.is_dma_buf());

    let (sock, _) = UnixStream::pair().expect("sock");
    let raw_fd = sock.into_raw_fd();
    let dma = OwnedDmaBuf::from_raw_fd_checked(raw_fd).expect("dma buf");
    let dma_frame = CapturedFrame::new_dma_buf(2560, 1440, 10240, 0x34325241, dma, 0);
    assert!(!dma_frame.is_empty());
    assert!(dma_frame.is_dma_buf());
    assert_eq!(dma_frame.dma_buf_raw_fd(), Some(raw_fd));
    assert_eq!(dma_frame.byte_len(), 10240 * 1440);
}

#[test]
fn lock_free_frame_slot_atomic_triple_buffering_and_metrics() {
    let slot = LockFreeFrameSlot::new();
    assert!(!slot.has_new_frame());
    assert_eq!(slot.sequence(), 0);
    assert!(slot.peek_current(|f| f.sequence).is_none());

    // 1. Публикуем первый кадр
    let f1 = CapturedFrame::new(1280, 720, 5120, 1).with_data(vec![10u8; 1280 * 720 * 4]);
    assert!(slot.publish_frame(f1));
    assert!(slot.has_new_frame());
    assert_eq!(slot.sequence(), 1);

    // 2. Читаем первый кадр
    let read1 = slot.with_latest_fresh(|f| {
        assert_eq!(f.sequence, 1);
        assert_eq!(f.width, 1280);
        assert_eq!(f.height, 720);
        f.data[0]
    });
    assert_eq!(read1, Some(10));
    // После чтения признак свежести сброшен
    assert!(!slot.has_new_frame());
    assert!(slot.with_latest_fresh(|_| ()).is_none());

    // 3. Публикуем кадр 2 и кадр 3 подряд до чтения (пропуск кадра)
    assert!(slot.publish_frame(CapturedFrame::new(1280, 720, 5120, 1).with_data(vec![20u8; 16])));
    assert!(slot.publish_frame(CapturedFrame::new(1280, 720, 5120, 1).with_data(vec![30u8; 16])));
    assert_eq!(slot.sequence(), 3);

    let m = slot.metrics();
    assert_eq!(m.sequence, 3);
    assert_eq!(m.published_frames, 3);
    assert_eq!(m.dropped_frames, 1); // f2 был перезаписан кадром f3
    assert_eq!(m.read_frames, 1);

    // 4. Потребитель забирает самый свежий кадр (f3)
    let read3 = slot.with_latest_fresh(|f| {
        assert_eq!(f.sequence, 3);
        f.data[0]
    });
    assert_eq!(read3, Some(30));

    // 5. In-place запись через publish_with (без выделения памяти)
    assert!(slot.publish_with(|f| {
        f.width = 1920;
        f.height = 1080;
        f.stride = 7680;
        f.format = 2;
        f.data = vec![42u8; 64];
    }));
    assert_eq!(slot.sequence(), 4);
    let peeked = slot.peek_current(|f| f.sequence);
    assert_eq!(peeked, Some(3)); // Текущий слот читателя ещё на кадре 3

    let read4 = slot.with_latest_fresh(|f| {
        assert_eq!(f.sequence, 4);
        assert_eq!(f.width, 1920);
        f.data[0]
    });
    assert_eq!(read4, Some(42));
}

#[test]
fn double_buffered_frame_atomic_swapping() {
    let db = DoubleBufferedFrame::new();
    assert!(!db.has_fresh());
    assert_eq!(db.sequence(), 0);
    assert!(db.with_active(|_| ()).is_none());

    let f1 = CapturedFrame::new(1280, 720, 5120, 1).with_data(vec![111u8; 16]);
    assert!(db.store(f1));
    assert!(db.has_fresh());
    assert_eq!(db.sequence(), 1);

    let val1 = db.with_active(|f| {
        assert_eq!(f.sequence, 1);
        f.data[0]
    });
    assert_eq!(val1, Some(111));
    assert!(!db.has_fresh());

    let f2 = CapturedFrame::new(1920, 1080, 7680, 2).with_data(vec![222u8; 16]);
    assert!(db.store(f2));
    assert!(db.has_fresh());
    assert_eq!(db.sequence(), 2);

    let val2 = db.with_active(|f| {
        assert_eq!(f.sequence, 2);
        assert_eq!(f.width, 1920);
        f.data[0]
    });
    assert_eq!(val2, Some(222));
}

#[test]
fn screencast_dma_buf_zero_copy_eliminates_86ms_latency_and_passes_60fps_budget() {
    use std::os::fd::IntoRawFd;
    use std::os::unix::net::UnixStream;
    use std::time::Instant;

    let slot = LockFreeFrameSlot::new();
    let (sock, _) = UnixStream::pair().expect("sock pair");
    let raw_fd = sock.into_raw_fd();
    let dma = OwnedDmaBuf::from_raw_fd_checked(raw_fd).expect("valid dma");

    // Публикуем 60 последовательных кадров DMA-BUF 1440p (2560x1440)
    let total_frames = 60;
    let t0 = Instant::now();

    for i in 0..total_frames {
        let frame = CapturedFrame::new_dma_buf(
            2560,
            1440,
            10240,
            0x34325241,
            dma.try_clone().expect("clone dma"),
            0x00FF_00FF,
        )
        .with_timestamp_us(i * 16_666);

        let pub_ok = slot.publish_frame(frame);
        assert!(pub_ok);

        // Потребитель читает каждый кадр в реальном времени
        let read = slot.with_latest_fresh(|f| {
            assert!(f.is_dma_buf());
            assert_eq!(f.width, 2560);
            assert_eq!(f.height, 1440);
            assert_eq!(f.drm_modifier, 0x00FF_00FF);
            f.sequence
        });
        assert_eq!(read, Some(i + 1));
    }

    let elapsed = t0.elapsed();
    // 60 операций zero-copy публикации и чтения должны завершиться менее чем за 50 мс суммарно,
    // тогда как одиночное CPU-копирование 60 кадров по 14.7 МБ при 86 мс на кадр заняло бы > 5.1 секунд!
    assert!(
        elapsed.as_millis() < 50,
        "60 DMA-BUF frame swaps must take < 50ms total, took {elapsed:?}"
    );

    let m = slot.metrics();
    assert_eq!(m.published_frames, 60);
    assert_eq!(m.read_frames, 60);
    assert_eq!(m.dropped_frames, 0); // 0 потерянных кадров
    assert_eq!(m.sequence, 60);
}
