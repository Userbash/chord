//! TASK-624: Контрактные тесты локального слушателя Unix domain socket / Windows named pipe
//! `discord-ipc-0..9` и бинарного протокола Discord Local RPC IPC в `discord-platform::rpc_ipc`.
//!
//! Независимые оракулы:
//! - `/home/sanya/re-deep/voice/strace-renderer-s3.log` (`L14260-L14744`: `AF_UNIX` `connect` probe ->
//!   `bind("/run/user/1000/discord-ipc-0")` -> `listen(fd, 511)`)
//! - `/home/sanya/re-deep/discord-run3.log` (`L64`: `[RPCServer:IPC] Starting on /run/user/1000/discord-ipc-0`)
//! - `docs/reference-projects/RUST-DISCORD-ACTIVITY-MAP.md`
//! - `Serein/crates/platform/src/game_activity.rs` (`Listener::bind`, `0o600`, `SO_PEERCRED`, `remove_stale`,
//!   owned `(dev, ino)` cleanup, Windows `\\.\pipe\discord-ipc-0..9` и `D:P(A;;GA;;;{sid})`)
//! - `Serein/apps/desktop/src/game_activity.rs` (`serve_ipc`, `MAX_CLIENTS = 8`)
//! - `Serein/crates/discord-protocol/src/rpc.rs` (`SET_ACTIVITY`, `INVITE_BROWSER`, `ERROR 4000`, timestamps)

use discord_platform::rpc_ipc::{
    DiscordRpcClient, DiscordRpcListener, IPC_HEADER_LEN, IPC_SLOT_COUNT, InviteBrowserPayload,
    IpcEndpoint, IpcFrame, LEGACY_TIMESTAMP_SECONDS_CUTOFF, MAX_INVITE_CODE_LEN,
    MAX_IPC_FRAME_BYTES, MAX_IPC_SLOT, MAX_RPC_ASSET_KEY_LEN, MAX_RPC_BUTTONS, MAX_RPC_CLIENTS,
    MAX_RPC_TEXT_LEN, RPC_ERROR_CODE_INVALID_PAYLOAD, RPC_LISTEN_BACKLOG, RPC_PROTOCOL_VERSION,
    RPC_SOCKET_MODE, RpcActivity, RpcAssets, RpcButton, RpcCommand, RpcIpcError, RpcIpcListener,
    RpcIpcSession, RpcMultiClientRegistry, RpcOpCode, RpcParty, RpcTimestamps, SetActivityPayload,
    UNIX_SOCKET_PREFIX, WINDOWS_PIPE_MAX_INSTANCES, WINDOWS_PIPE_PREFIX, WindowsNamedPipeConfig,
    build_rpc_error_frame_for_payload, build_rpc_error_json, decode_ipc_frame, encode_ipc_frame,
    format_windows_sddl_for_sid, handle_ipc_frame, is_valid_invite_code, parse_ipc_slot,
    resolve_ipc_socket_path, resolve_ipc_socket_path_from_env, resolve_windows_named_pipe_path,
};
use std::fs;
use std::os::unix::fs::{MetadataExt, PermissionsExt};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

static TEST_COUNTER: AtomicU64 = AtomicU64::new(1);

struct TempIpcDir {
    path: PathBuf,
}

impl TempIpcDir {
    fn new(prefix: &str) -> Self {
        let seq = TEST_COUNTER.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("dlab-ipc-{prefix}-{}-{seq}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).expect("create temp ipc dir");
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o700))
            .expect("chmod 0700 temp ipc dir");
        Self { path: dir }
    }
}

impl Drop for TempIpcDir {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

#[test]
fn rpc01_constants_and_cross_platform_endpoints_match_re_and_serein_oracles() {
    assert_eq!(IPC_HEADER_LEN, 8);
    assert_eq!(MAX_IPC_FRAME_BYTES, 16 * 1024);
    assert_eq!(MAX_IPC_SLOT, 9);
    assert_eq!(IPC_SLOT_COUNT, 10);
    assert_eq!(RPC_PROTOCOL_VERSION, 1);
    assert_eq!(MAX_RPC_BUTTONS, 2);
    assert_eq!(MAX_RPC_TEXT_LEN, 128);
    assert_eq!(MAX_RPC_ASSET_KEY_LEN, 1024);
    assert_eq!(MAX_RPC_CLIENTS, 8);
    assert_eq!(WINDOWS_PIPE_MAX_INSTANCES, 9);
    assert_eq!(UNIX_SOCKET_PREFIX, "discord-ipc-");
    assert_eq!(WINDOWS_PIPE_PREFIX, r"\\.\pipe\discord-ipc-");
    assert_eq!(RPC_SOCKET_MODE, 0o600);
    assert_eq!(RPC_LISTEN_BACKLOG, 511);
    assert_eq!(MAX_INVITE_CODE_LEN, 64);
    assert_eq!(RPC_ERROR_CODE_INVALID_PAYLOAD, 4000);
    assert_eq!(LEGACY_TIMESTAMP_SECONDS_CUTOFF, 10_000_000_000);

    for slot in 0..=MAX_IPC_SLOT {
        let unix_ep =
            IpcEndpoint::unix_from_env(slot, &[("XDG_RUNTIME_DIR", "/run/user/1000")]).unwrap();
        assert_eq!(unix_ep.slot(), slot);
        assert_eq!(
            unix_ep,
            IpcEndpoint::UnixSocket {
                slot,
                path: PathBuf::from(format!("/run/user/1000/discord-ipc-{slot}")),
            }
        );

        let win_pipe = resolve_windows_named_pipe_path(slot).unwrap();
        assert_eq!(win_pipe, format!(r"\\.\pipe\discord-ipc-{slot}"));
        let win_ep = IpcEndpoint::windows(slot).unwrap();
        assert_eq!(win_ep.slot(), slot);
        assert_eq!(
            win_ep,
            IpcEndpoint::WindowsNamedPipe {
                slot,
                pipe_name: win_pipe.clone(),
            }
        );

        assert_eq!(
            parse_ipc_slot(&format!("/run/user/1000/discord-ipc-{slot}")),
            Ok(slot)
        );
        assert_eq!(parse_ipc_slot(&format!("discord-ipc-{slot}")), Ok(slot));
        assert_eq!(parse_ipc_slot(&win_pipe), Ok(slot));
    }

    for invalid_slot in [10u8, 11, 99, u8::MAX] {
        assert_eq!(
            resolve_ipc_socket_path(invalid_slot),
            Err(RpcIpcError::InvalidSlot(invalid_slot))
        );
        assert_eq!(
            resolve_ipc_socket_path_from_env(invalid_slot, &[]),
            Err(RpcIpcError::InvalidSlot(invalid_slot))
        );
        assert_eq!(
            resolve_windows_named_pipe_path(invalid_slot),
            Err(RpcIpcError::InvalidSlot(invalid_slot))
        );
        assert_eq!(
            IpcEndpoint::windows(invalid_slot),
            Err(RpcIpcError::InvalidSlot(invalid_slot))
        );
    }

    assert_eq!(
        parse_ipc_slot("discord-ipc-10"),
        Err(RpcIpcError::InvalidSlot(10))
    );
    assert_eq!(
        parse_ipc_slot(r"\\.\pipe\discord-ipc-255"),
        Err(RpcIpcError::InvalidSlot(255))
    );
    for malformed in [
        "",
        "discord-ipc-",
        "discord-ipc-00",
        "discord-ipc-01",
        "discord-ipc--1",
        "discord-ipc-abc",
        "/tmp/other-socket-0",
    ] {
        assert_eq!(
            parse_ipc_slot(malformed),
            Err(RpcIpcError::InvalidField("slot")),
            "malformed slot input must be rejected: {malformed:?}"
        );
    }
}

#[test]
fn rpc02_windows_named_pipe_config_and_sddl_dacl_validation() {
    let sid = "S-1-5-21-3623811015-3361044348-30300820-1013";
    let sddl = format_windows_sddl_for_sid(sid).expect("valid SID");
    assert_eq!(
        sddl,
        "D:P(A;;GA;;;S-1-5-21-3623811015-3361044348-30300820-1013)"
    );

    let cfg = WindowsNamedPipeConfig::for_slot(0, sid, true).expect("valid pipe config");
    assert_eq!(cfg.pipe_name, r"\\.\pipe\discord-ipc-0");
    assert_eq!(cfg.slot, 0);
    assert_eq!(cfg.sddl_dacl, sddl);
    assert!(cfg.first_pipe_instance);
    assert!(cfg.reject_remote_clients);
    assert_eq!(cfg.max_instances, 9);
    assert_eq!(cfg.in_buffer_size, 16 * 1024);
    assert_eq!(cfg.out_buffer_size, 16 * 1024);

    assert_eq!(
        WindowsNamedPipeConfig::for_slot(10, sid, false),
        Err(RpcIpcError::InvalidSlot(10))
    );

    for bad_sid in [
        "",
        "S-1-",
        "S-1-5-",
        "S-1-5--21-1000",
        "S-2-5-21-1000",
        "S-1-5-21-1000;D:(A;;GA;;;WD)",
        "S-1-5-21-1000)",
        "S-1-5-21-1000\0",
        "everyone",
    ] {
        assert_eq!(
            format_windows_sddl_for_sid(bad_sid),
            Err(RpcIpcError::InvalidField("sid")),
            "unsafe SID {bad_sid:?} must be rejected"
        );
    }
}

#[test]
fn rpc03_listener_binds_slot_0_sets_mode_0600_and_serves_client_roundtrip() {
    let tmp = TempIpcDir::new("bind0");
    let listener: RpcIpcListener =
        DiscordRpcListener::bind_in_dir(&tmp.path).expect("bind first free slot");
    assert_eq!(listener.slot(), Some(0));
    assert_eq!(listener.path(), tmp.path.join("discord-ipc-0"));
    assert_eq!(
        listener.format_startup_log(),
        format!(
            "[RPCServer:IPC] Starting on {}",
            tmp.path.join("discord-ipc-0").display()
        )
    );

    let meta = fs::symlink_metadata(listener.path()).expect("socket metadata");
    assert_eq!(meta.mode() & 0o777, 0o600);
    assert_eq!(listener.device(), meta.dev());
    assert_eq!(listener.inode(), meta.ino());
    assert_eq!(listener.uid(), meta.uid());

    let dir_clone = tmp.path.clone();
    let client_thread = std::thread::spawn(move || {
        let mut client = DiscordRpcClient::new("123456789012345678");
        assert!(!client.is_connected());
        let (slot, ready) = client
            .connect_in_dir(&dir_clone)
            .expect("client connects to slot 0");
        assert_eq!(slot, 0);
        assert!(client.is_connected());
        assert_eq!(ready.opcode, RpcOpCode::Frame);
        assert!(ready.payload.contains(r#""cmd":"DISPATCH""#));
        assert!(ready.payload.contains(r#""evt":"READY""#));

        let activity = RpcActivity::new()
            .with_name("Cyber Arena")
            .with_details("Solo Queue")
            .with_state("Match 1")
            .with_timestamps(RpcTimestamps::new(Some(1_700_000_000_000), None));
        let set_cmd = SetActivityPayload::new(9876, Some(activity), "nonce-rpc-03");
        let ack = client.send_activity(&set_cmd).expect("send_activity");
        assert_eq!(ack.opcode, RpcOpCode::Frame);
        assert!(ack.payload.contains(r#""cmd":"SET_ACTIVITY""#));
        assert!(ack.payload.contains(r#""nonce":"nonce-rpc-03""#));

        let inv = InviteBrowserPayload::new("wumpus-friends_1", "nonce-inv-1");
        let inv_ack = client
            .send_invite_browser(&inv)
            .expect("send_invite_browser");
        assert_eq!(inv_ack.opcode, RpcOpCode::Frame);
        assert!(inv_ack.payload.contains(r#""cmd":"INVITE_BROWSER""#));
        assert!(inv_ack.payload.contains(r#""code":"wumpus-friends_1""#));

        let pong = client.ping(r#"{"seq":7}"#).expect("ping");
        assert_eq!(pong.opcode, RpcOpCode::Pong);
        assert_eq!(pong.payload, r#"{"seq":7}"#);

        client.close("{}").expect("close");
        assert!(!client.is_connected());
    });

    let mut server_stream = listener.accept().expect("accept client");
    let mut session = RpcIpcSession::new();
    let responses = session
        .serve_stream(&mut server_stream)
        .expect("serve full client stream");

    client_thread.join().expect("join client thread");

    assert!(session.is_handshaked());
    assert_eq!(session.client_id(), Some("123456789012345678"));
    assert!(session.is_closed());
    assert_eq!(session.current_activity(), None);
    assert_eq!(session.invites(), &["wumpus-friends_1".to_owned()]);
    assert_eq!(responses.len(), 5);
    assert!(
        responses[1]
            .gateway_presence_update_json
            .as_deref()
            .unwrap()
            .contains("Solo Queue")
    );
    assert_eq!(
        responses[2].invite_code.as_deref(),
        Some("wumpus-friends_1")
    );
    assert!(responses[4].closed);
}

#[test]
fn rpc04_slot_contention_auto_increments_0_to_9_and_reports_all_slots_busy() {
    let tmp = TempIpcDir::new("slots0to9");
    let mut listeners = Vec::with_capacity(IPC_SLOT_COUNT);

    for expected_slot in 0..=MAX_IPC_SLOT {
        let l = DiscordRpcListener::bind_in_dir(&tmp.path)
            .unwrap_or_else(|e| panic!("slot {expected_slot} must bind: {e:?}"));
        assert_eq!(l.slot(), Some(expected_slot));
        assert_eq!(
            l.path(),
            tmp.path.join(format!("discord-ipc-{expected_slot}"))
        );
        listeners.push(l);
    }

    // Когда все 10 слотов 0..=9 заняты живыми слушателями, 11-й вызов возвращает AllSlotsBusy.
    let overflow = DiscordRpcListener::bind_in_dir(&tmp.path);
    assert_eq!(overflow.unwrap_err(), RpcIpcError::AllSlotsBusy);

    // Попытка привязаться напрямую к занятому слоту 0 отклоняется и не удаляет живой сокет.
    assert!(DiscordRpcListener::bind_slot_in_dir(&tmp.path, 0).is_err());
    assert!(tmp.path.join("discord-ipc-0").exists());

    // Освобождение слота 4 удаляет его сокет и позволяет следующему bind_in_dir занять слот 4.
    let released = listeners.remove(4);
    let slot4_path = released.path().to_path_buf();
    drop(released);
    assert!(!slot4_path.exists());

    let rebound = DiscordRpcListener::bind_in_dir(&tmp.path).expect("rebind freed slot 4");
    assert_eq!(rebound.slot(), Some(4));
    assert_eq!(rebound.path(), slot4_path);
}

#[test]
fn rpc05_stale_socket_recovery_and_owned_inode_cleanup_on_drop() {
    let tmp = TempIpcDir::new("stale");
    let sock0 = tmp.path.join("discord-ipc-0");

    // 1. Симулируем аварийно завершившийся процесс, оставивший осиротевший файл сокета discord-ipc-0.
    drop(UnixListener::bind(&sock0).expect("create raw unix socket"));
    assert!(
        sock0.exists(),
        "stale socket file remains after raw UnixListener drop"
    );

    // Слушатель обязан распознать ConnectionRefused через датаграммный зонд, удалить старый сокет и занять слот 0.
    let listener =
        DiscordRpcListener::bind_in_dir(&tmp.path).expect("recover from stale socket at slot 0");
    assert_eq!(listener.slot(), Some(0));
    assert_eq!(listener.path(), sock0);
    drop(listener);
    assert!(
        !sock0.exists(),
        "listener Drop must clean up its own socket"
    );

    // 2. Обычный файл (не сокет) на месте discord-ipc-0 НЕ должен удаляться как stale-сокет;
    // bind_in_dir должен пропустить слот 0 и привязаться к слоту 1.
    fs::write(&sock0, b"not-a-socket").expect("write regular file at slot 0");
    let listener_slot1 =
        DiscordRpcListener::bind_in_dir(&tmp.path).expect("skip non-socket file and bind slot 1");
    assert_eq!(listener_slot1.slot(), Some(1));
    assert_eq!(fs::read(&sock0).unwrap(), b"not-a-socket");
    drop(listener_slot1);
    fs::remove_file(&sock0).unwrap();

    // 3. Защита по (device, inode) в Drop: если путь был подменён другим файлом или новым сокетом,
    // Drop старого слушателя не имеет права удалить чужой файл.
    let old_listener = DiscordRpcListener::bind_path(&sock0).expect("bind sock0");
    fs::remove_file(&sock0).expect("unlink sock0 while old_listener is alive");
    fs::write(&sock0, b"replacement-file").expect("write replacement file");
    drop(old_listener);
    assert_eq!(
        fs::read(&sock0).expect("replacement file must survive old_listener drop"),
        b"replacement-file"
    );
}

#[test]
fn rpc06_security_directory_permissions_and_peer_uid_verification() {
    let tmp = TempIpcDir::new("security");

    // 1. Каталог с небезопасными правами (0o777 без sticky-бита) отклоняется с UnsafeDirectory.
    let unsafe_dir = tmp.path.join("world-writable");
    fs::create_dir_all(&unsafe_dir).unwrap();
    fs::set_permissions(&unsafe_dir, fs::Permissions::from_mode(0o777)).unwrap();
    let bad_bind = DiscordRpcListener::bind_in_dir(&unsafe_dir);
    assert!(
        matches!(bad_bind, Err(RpcIpcError::UnsafeDirectory(_))),
        "expected UnsafeDirectory, got: {bad_bind:?}"
    );
    fs::set_permissions(&unsafe_dir, fs::Permissions::from_mode(0o700)).unwrap();

    // 2. Проверка SO_PEERCRED: подключение от процесса с несовпадающим ожидаемым UID отклоняется.
    let listener = DiscordRpcListener::bind_in_dir(&tmp.path).expect("bind safe dir");
    let sock_path = listener.path().to_path_buf();
    let _client = UnixStream::connect(&sock_path).expect("raw client connect");
    let wrong_uid = listener.uid().wrapping_add(1);
    let rejected = listener.accept_checked_with_expected_uid(wrong_uid);
    assert_eq!(
        rejected.unwrap_err(),
        RpcIpcError::UnauthorizedPeer {
            peer_uid: listener.uid(),
            expected_uid: wrong_uid,
        }
    );
}

#[test]
fn rpc07_session_state_machine_and_error_frame_correlation() {
    let mut session = RpcIpcSession::new();
    assert!(!session.is_handshaked());

    // 1. Отправка команды до Handshake отклоняется с HandshakeRequired.
    let premature_frame = IpcFrame {
        opcode: RpcOpCode::Frame,
        payload: r#"{"cmd":"SET_ACTIVITY","nonce":"n1","args":{"pid":100,"activity":null}}"#
            .to_owned(),
    };
    assert_eq!(
        session.process_frame(&premature_frame),
        Err(RpcIpcError::HandshakeRequired)
    );

    // 2. Корректный Handshake переводит сессию в активное состояние.
    let hs = IpcFrame {
        opcode: RpcOpCode::Handshake,
        payload: r#"{"v":1,"client_id":"9988776655"}"#.to_owned(),
    };
    let hs_resp = session.process_frame(&hs).expect("valid handshake");
    assert!(session.is_handshaked());
    assert_eq!(session.client_id(), Some("9988776655"));
    assert!(
        hs_resp
            .reply_frame
            .as_ref()
            .unwrap()
            .payload
            .contains(r#""evt":"READY""#)
    );

    // 3. Повторный Handshake в уже открытой сессии отклоняется.
    assert_eq!(
        session.process_frame(&hs),
        Err(RpcIpcError::InvalidField("opcode"))
    );

    // 4. Неподдерживаемая RPC-команда (например SUBSCRIBE или AUTHORIZE) возвращает фрейм ERROR 4000
    // с сохранением cmd и nonce (по эталону Serein error_for_payload).
    let unsupported = IpcFrame {
        opcode: RpcOpCode::Frame,
        payload: r#"{"cmd":"SUBSCRIBE","nonce":"sub-42","evt":"ACTIVITY_JOIN"}"#.to_owned(),
    };
    let err_resp = session
        .process_frame(&unsupported)
        .expect("unsupported command yields ERROR frame");
    let err_frame = err_resp.reply_frame.expect("error reply frame");
    assert_eq!(err_frame.opcode, RpcOpCode::Frame);
    assert!(err_frame.payload.contains(r#""cmd":"SUBSCRIBE""#));
    assert!(err_frame.payload.contains(r#""evt":"ERROR""#));
    assert!(err_frame.payload.contains(r#""nonce":"sub-42""#));
    assert!(err_frame.payload.contains(r#""code":4000"#));

    // 5. Невалидный или слишком длинный cmd/nonce в ошибочном запросе откатывается к SET_ACTIVITY и nonce:null.
    let huge_bad = format!(
        r#"{{"cmd":"{}","nonce":"{}"}}"#,
        "x".repeat(100),
        "n".repeat(200)
    );
    let fallback_err = build_rpc_error_frame_for_payload(&huge_bad);
    assert!(fallback_err.payload.contains(r#""cmd":"SET_ACTIVITY""#));
    assert!(fallback_err.payload.contains(r#""nonce":null"#));
    assert!(fallback_err.payload.contains(r#""evt":"ERROR""#));

    let explicit_err = build_rpc_error_json(
        "AUTHORIZE",
        Some("auth-1"),
        RPC_ERROR_CODE_INVALID_PAYLOAD,
        "Unsupported or invalid activity payload",
    );
    assert!(explicit_err.contains(r#""cmd":"AUTHORIZE""#));
    assert!(explicit_err.contains(r#""nonce":"auth-1""#));
}

#[test]
fn rpc08_invite_browser_validation_and_legacy_timestamp_normalization() {
    // Валидные и невалидные инвайт-коды по эталону Serein/crates/discord-protocol/src/rpc.rs
    assert!(is_valid_invite_code("hTKzmak"));
    assert!(is_valid_invite_code("wumpus-friends_1"));
    assert!(is_valid_invite_code(&"a".repeat(MAX_INVITE_CODE_LEN)));
    assert!(!is_valid_invite_code(""));
    assert!(!is_valid_invite_code(&"a".repeat(MAX_INVITE_CODE_LEN + 1)));
    assert!(!is_valid_invite_code("../secret"));
    assert!(!is_valid_invite_code("https://discord.gg/a"));
    assert!(!is_valid_invite_code("bad code"));

    let cmd = RpcCommand::from_json(
        r#"{"cmd":"INVITE_BROWSER","nonce":"inv-99","args":{"code":"hTKzmak"}}"#,
    )
    .expect("valid INVITE_BROWSER command");
    assert_eq!(
        cmd,
        RpcCommand::InviteBrowser(InviteBrowserPayload::new("hTKzmak", "inv-99"))
    );

    for bad_invite_json in [
        r#"{"cmd":"INVITE_BROWSER","nonce":"n","args":{"code":"../secret"}}"#,
        r#"{"cmd":"INVITE_BROWSER","nonce":"n","args":{"code":"https://discord.gg/a"}}"#,
        r#"{"cmd":"INVITE_BROWSER","nonce":"n","args":{"code":""}}"#,
        r#"{"cmd":"INVITE_BROWSER","nonce":"n","args":{}}"#,
        r#"{"cmd":"INVITE_BROWSER","args":{"code":"hTKzmak"}}"#,
        r#"{"cmd":"AUTHORIZE","nonce":"n","args":{"scopes":["rpc"]}}"#,
    ] {
        assert!(
            RpcCommand::from_json(bad_invite_json).is_err(),
            "malformed command must fail: {bad_invite_json}"
        );
    }

    // Нормализация legacy-таймстемпов из секунд (< 10_000_000_000) в миллисекунды
    let legacy_ts = RpcTimestamps::new(Some(1_700_000_000), Some(1_700_000_030));
    assert_eq!(
        legacy_ts.normalized_millis(),
        RpcTimestamps::new(Some(1_700_000_000_000), Some(1_700_000_030_000))
    );
    let modern_ts = RpcTimestamps::new(Some(1_700_000_000_000), Some(1_700_003_600_000));
    assert_eq!(modern_ts.normalized_millis(), modern_ts);
}

#[test]
fn rpc09_multi_client_registry_tracks_8_slots_and_publishes_latest_activity() {
    let mut registry = RpcMultiClientRegistry::new();
    assert_eq!(registry.active_activity(), None);
    assert!(
        registry
            .active_gateway_presence_json()
            .contains(r#""activities":[]"#)
    );

    let mut claimed = Vec::new();
    for expected in 0..MAX_RPC_CLIENTS {
        let slot = registry.claim_slot().expect("claim slot within 0..8");
        assert_eq!(slot, expected);
        claimed.push(slot);
    }
    assert_eq!(registry.claim_slot(), None, "9th client slot must be None");

    let act0 = RpcActivity::new().with_name("Game One").with_state("Menu");
    let act1 = RpcActivity::new()
        .with_name("Game Two")
        .with_state("Boss Fight");

    let latest = registry
        .update_slot_activity(claimed[0], Some(act0.clone()))
        .unwrap();
    assert_eq!(latest, Some(act0.clone()));

    let latest = registry
        .update_slot_activity(claimed[1], Some(act1.clone()))
        .unwrap();
    assert_eq!(latest, Some(act1.clone()));
    assert!(
        registry
            .active_gateway_presence_json()
            .contains("Boss Fight")
    );

    // Отключение второго клиента возвращает приоритет первому клиенту.
    registry.release_slot(claimed[1]);
    assert_eq!(registry.active_activity(), Some(&act0));

    // Очистка активности первого клиента (SET_ACTIVITY с activity: null) обнуляет присутствие.
    let cleared = registry.update_slot_activity(claimed[0], None).unwrap();
    assert_eq!(cleared, None);
    assert!(
        registry
            .active_gateway_presence_json()
            .contains(r#""activities":[]"#)
    );
}

#[test]
fn rpc10_wire_framing_boundaries_and_validation_edges() {
    // Граница 0 байт и ровно 16 KiB (16384 байт)
    let empty_frame = encode_ipc_frame(RpcOpCode::Ping, "").unwrap();
    assert_eq!(empty_frame.len(), IPC_HEADER_LEN);
    let decoded_empty = decode_ipc_frame(&empty_frame).unwrap();
    assert_eq!(decoded_empty.opcode, RpcOpCode::Ping);
    assert_eq!(decoded_empty.payload, "");

    let max_payload = "x".repeat(MAX_IPC_FRAME_BYTES);
    let max_frame = encode_ipc_frame(RpcOpCode::Pong, &max_payload).unwrap();
    assert_eq!(max_frame.len(), IPC_HEADER_LEN + MAX_IPC_FRAME_BYTES);
    let decoded_max = decode_ipc_frame(&max_frame).unwrap();
    assert_eq!(decoded_max.opcode, RpcOpCode::Pong);
    assert_eq!(decoded_max.payload.len(), MAX_IPC_FRAME_BYTES);

    // Превышение 16 KiB на 1 байт при кодировании и декодировании
    let over_payload = "x".repeat(MAX_IPC_FRAME_BYTES + 1);
    assert_eq!(
        encode_ipc_frame(RpcOpCode::Frame, &over_payload),
        Err(RpcIpcError::FrameTooLarge {
            length: MAX_IPC_FRAME_BYTES + 1,
            max: MAX_IPC_FRAME_BYTES,
        })
    );

    // Невалидная UTF-8 последовательность в теле фрейма
    let mut bad_utf8 = Vec::new();
    bad_utf8.extend_from_slice(&1u32.to_le_bytes());
    bad_utf8.extend_from_slice(&2u32.to_le_bytes());
    bad_utf8.extend_from_slice(&[0xff, 0xfe]);
    assert_eq!(decode_ipc_frame(&bad_utf8), Err(RpcIpcError::InvalidUtf8));

    // Ограничения полей RpcActivity (тип > 5, длина > 128, кнопки > 2, небезопасные URL, party.size)
    assert_eq!(
        RpcActivity::new().with_activity_type(6).validate(),
        Err(RpcIpcError::InvalidField("type"))
    );
    assert_eq!(
        RpcActivity::new()
            .with_details("a".repeat(MAX_RPC_TEXT_LEN + 1))
            .validate(),
        Err(RpcIpcError::InvalidField("details"))
    );
    assert_eq!(
        RpcActivity::new()
            .with_timestamps(RpcTimestamps::new(Some(200), Some(100)))
            .validate(),
        Err(RpcIpcError::InvalidField("timestamps"))
    );
    assert_eq!(
        RpcActivity::new()
            .with_assets(RpcAssets::new(
                Some("k".repeat(MAX_RPC_ASSET_KEY_LEN + 1)),
                None,
                None,
                None,
            ))
            .validate(),
        Err(RpcIpcError::InvalidField("large_image"))
    );
    assert_eq!(
        RpcActivity::new()
            .with_party(RpcParty::new(Some("p".to_owned()), Some([5, 4])))
            .validate(),
        Err(RpcIpcError::InvalidField("party.size"))
    );
    assert_eq!(
        RpcActivity::new()
            .with_buttons(vec![RpcButton::new("Click", "http://insecure.example.com")])
            .validate(),
        Err(RpcIpcError::InvalidField("buttons"))
    );
    assert_eq!(
        RpcActivity::new()
            .with_url("https://user:pass@example.com/stream")
            .validate(),
        Err(RpcIpcError::InvalidField("url"))
    );

    // handle_ipc_frame напрямую обрабатывает INVITE_BROWSER
    let inv_frame = IpcFrame {
        opcode: RpcOpCode::Frame,
        payload: r#"{"cmd":"INVITE_BROWSER","nonce":"inv-direct","args":{"code":"rust-lang"}}"#
            .to_owned(),
    };
    let inv_resp = handle_ipc_frame(&inv_frame).expect("handle INVITE_BROWSER");
    assert_eq!(inv_resp.invite_code.as_deref(), Some("rust-lang"));
    assert!(
        inv_resp
            .reply_frame
            .unwrap()
            .payload
            .contains(r#""cmd":"INVITE_BROWSER""#)
    );
}
