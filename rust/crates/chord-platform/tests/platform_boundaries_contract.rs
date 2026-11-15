//! Portable contracts shared by Windows, Linux and macOS. Socket/FD integration
//! remains in the Unix contracts; these assertions never require a desktop session.
use chord_platform::dbus::{BusAddress, Value, auth, message};
use chord_platform::power::{IdlePolicy, PresenceStatus};
use chord_platform::rpc_ipc::{self, IpcEndpoint, RpcOpCode, WindowsNamedPipeConfig};
use chord_platform::screencast::{CapturedFrame, LockFreeFrameSlot};
use chord_platform::tray::{self, TrayIconState};

#[test]
fn dbus_descriptor_free_wire_matches_existing_gio_oracle() {
    // Gio.DBusMessage.to_blob(), fixtures/dbus/wire.json: action_invoked_signal.
    let hex = "6c0401010d000000030000006e00000001016f001e0000002f6f72672f667265656465736b746f702f4e6f74696669636174696f6e730000020173001d0000006f72672e667265656465736b746f702e4e6f74696669636174696f6e730000000801670002757300030173000d000000416374696f6e496e766f6b656400000064000000040000006f70656e00";
    let bytes: Vec<u8> = hex
        .as_bytes()
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect();
    let parsed = message::parse(&bytes).expect("portable D-Bus parse");
    assert_eq!(parsed.serial, 3);
    assert_eq!(parsed.signature, "us");
    assert_eq!(
        parsed.body,
        vec![Value::U32(100), Value::String("open".into())]
    );
    assert_eq!(
        message::build(
            parsed.kind,
            parsed.serial,
            &parsed.fields,
            &parsed.signature,
            &parsed.body
        )
        .unwrap(),
        bytes
    );
    assert_eq!(auth::external_auth_line(1000), "AUTH EXTERNAL 31303030");
    assert!(BusAddress::parse("unix:path=/tmp/chord-test-bus").is_some());
}

#[test]
fn rpc_framing_and_windows_endpoint_policy_are_portable() {
    let encoded = rpc_ipc::encode_ipc_frame(RpcOpCode::Ping, "hello").unwrap();
    assert_eq!(
        encoded,
        [3, 0, 0, 0, 5, 0, 0, 0, b'h', b'e', b'l', b'l', b'o']
    );
    let parsed = rpc_ipc::decode_ipc_frame(&encoded).unwrap();
    assert_eq!(parsed.opcode, RpcOpCode::Ping);
    assert_eq!(parsed.payload, "hello");
    assert!(rpc_ipc::decode_ipc_frame(&encoded[..12]).is_err());
    assert_eq!(IpcEndpoint::windows(9).unwrap().slot(), 9);
    assert!(IpcEndpoint::windows(10).is_err());
    let config = WindowsNamedPipeConfig::for_slot(0, "S-1-5-21-42", true).unwrap();
    assert_eq!(config.pipe_name, r"\\.\pipe\discord-ipc-0");
    assert_eq!(config.sddl_dacl, "D:P(A;;GA;;;S-1-5-21-42)");
    assert!(config.reject_remote_clients);
    assert_eq!(config.max_instances, 9);
}

#[test]
fn cpu_frames_and_presence_tray_policy_work_without_unix_fds() {
    let frame = CapturedFrame::new(64, 64, 256, 0).with_data(vec![7; 64 * 256]);
    assert!(!frame.is_empty());
    assert!(!frame.is_dma_buf());
    assert_eq!(frame.byte_len(), 64 * 256);
    let slot = LockFreeFrameSlot::new();
    assert!(slot.publish_frame(frame));
    assert_eq!(slot.with_latest_fresh(|frame| frame.data[0]), Some(7));
    assert_eq!(IdlePolicy::DEFAULT.status(599), PresenceStatus::Online);
    assert_eq!(IdlePolicy::DEFAULT.status(600), PresenceStatus::Idle);
    assert_eq!(IdlePolicy::DEFAULT.status(1800), PresenceStatus::Dnd);
    assert_eq!(
        tray::compute_tray_icon_state(true, true, false, 0),
        TrayIconState::Muted
    );
}

#[test]
fn rpc_stream_codec_does_not_depend_on_unix_socket_type() {
    let mut encoded = Vec::new();
    assert_eq!(
        rpc_ipc::write_ipc_frame(&mut encoded, RpcOpCode::Ping, "hello").unwrap(),
        13
    );
    let frame = rpc_ipc::read_ipc_frame(&mut encoded.as_slice()).unwrap();
    assert_eq!(frame.opcode, RpcOpCode::Ping);
    assert_eq!(frame.payload, "hello");
    let mut oversized = [0u8; 8];
    oversized[4..8].copy_from_slice(&(rpc_ipc::MAX_IPC_FRAME_BYTES as u32 + 1).to_le_bytes());
    assert!(matches!(
        rpc_ipc::read_ipc_frame(&mut oversized.as_slice()),
        Err(rpc_ipc::RpcIpcError::FrameTooLarge { .. })
    ));
}

#[test]
fn unsupported_capabilities_are_typed_and_identify_the_platform() {
    let error = chord_platform::UnsupportedCapability::new("freedesktop notifications");
    assert_eq!(error.capability, "freedesktop notifications");
    assert_eq!(error.platform, std::env::consts::OS);
    let reported: &dyn std::error::Error = &error;
    assert!(reported.to_string().contains("freedesktop notifications"));
    assert!(reported.to_string().contains(std::env::consts::OS));
}
