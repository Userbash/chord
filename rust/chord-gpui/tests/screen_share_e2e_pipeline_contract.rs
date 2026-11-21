//! End-to-End Screen Share Pipeline Contract Tests (`W3-3`: `P1-2.1`, `P1-2.2`, `P1-2.5`).
//!
//! Verifies all 9 mandatory dimensions across:
//! 1. `P1-2.1`: Portal -> UI binding in `src/ui/screen_share_modal.rs`
//!    (`confirm_go_live_with_portal`, `confirm_go_live_with_pipeline`,
//!    `switch_live_source_with_pipeline`, `stop_go_live_with_portal`,
//!    `MockScreenCastPortalRunner`).
//! 2. `P1-2.2`: PipeWire / DMA-BUF -> `RawVideoFrame` bridge in `src/pipewire_portal.rs`
//!    and `src/video/mod.rs` (FourCC mapping for `NV12`, `YUV420`/`I420`, `RGBA8888`,
//!    `BGRA8888` with BGRA-to-RGBA swizzle, even-dimension & stride padding validation,
//!    truncated/empty DMA-BUF rejection, and zero-copy buffer reuse / SHM fallback stats).
//! 3. `P1-2.5`: `ScreenSharePipelineController` in `src/video/mod.rs` connecting
//!    `ScreenCastCapturePipeline` -> `VideoEncoder` -> `VideoRtpSender` + Discord Gateway
//!    Opcodes 18 (`STREAM_CREATE`), 22 (`STREAM_SET_PAUSED`), and 19 (`STREAM_DELETE`),
//!    pause/resume keyframe forcing, RTCP PLI forwarding, and zero FD leaks (`active_fd_balance() == 0`).

#![recursion_limit = "512"]

#[path = "../src/audio/mod.rs"]
mod audio;
#[path = "../src/dispatch/mod.rs"]
mod dispatch;
#[path = "../src/media/mod.rs"]
mod media;
#[path = "../src/net/mod.rs"]
mod net;
#[path = "../src/pipewire_portal.rs"]
mod pipewire_portal;
#[path = "../src/platform/mod.rs"]
mod platform;
#[path = "../src/state/mod.rs"]
mod state;
#[path = "../src/storage/mod.rs"]
mod storage;
#[path = "../src/theme/mod.rs"]
mod theme;
#[path = "../src/ui/mod.rs"]
mod ui;
#[path = "../src/video/mod.rs"]
mod video;

use std::io::{Read, Write};
use std::os::fd::{AsRawFd, IntoRawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::thread;

use chord_platform::dbus::message::{
    FIELD_INTERFACE, FIELD_MEMBER, FIELD_PATH, FIELD_REPLY_SERIAL, FIELD_SIGNATURE, Message,
    MessageType,
};
use chord_platform::dbus::{BusAddress, Value, fd, message};
use chord_platform::screencast::{
    CaptureTargetKind, CapturedFrame, OwnedDmaBuf, PortalRequestError, ScreenCastCapturePipeline,
    ScreenCastPipelineState, ScreenCastPortal,
};
use pipewire_portal::{
    DRM_FORMAT_MOD_INVALID, DRM_FORMAT_MOD_LINEAR, DRM_FOURCC_BGRA8888, DRM_FOURCC_NV12,
    DRM_FOURCC_RGBA8888, DRM_FOURCC_YUV420, DmaBufNegotiationRequest, FrameBridgeError,
    OwnedPortalFd, PipeWireFrameBridge, PipeWirePortalManager, PortalBackend, PortalError,
    PortalSessionConfig, convert_captured_frame_to_raw_frame, convert_dma_buf_plane_to_raw_frame,
    map_drm_fourcc_to_pixel_format, negotiated_dma_buf_to_raw_frame, swizzle_bgra_to_rgba_in_place,
};
use serde_json::json;
use state::{AppState, CallStatus};
use ui::screen_share_modal::{
    MockScreenCastPortalRunner, ScreenShareFps, ScreenShareQualityPreset, ScreenShareResolution,
    ScreenShareSelectorState, ScreenShareTab,
};
use video::encoder::{
    VP8_START_CODE, VideoCodec, VideoEncoderConfig, VideoFramePixelFormat,
    parse_vp8_keyframe_dimensions, verify_packet_checksum,
};
use video::sender::{DEFAULT_VIDEO_PAYLOAD_TYPE, RTP_FIXED_HEADER_LEN, RtpHeader};
use video::{
    DmaBufPlaneDescriptor, ScreenSharePipelineController, ScreenSharePipelineError,
    StreamSignalingTarget, build_gateway_stream_create_op18, build_gateway_stream_delete_op19,
    build_gateway_stream_set_paused_op22, validate_stream_key,
};

static SOCKET_COUNTER: AtomicUsize = AtomicUsize::new(1);

fn unique_socket_path(tag: &str) -> PathBuf {
    let idx = SOCKET_COUNTER.fetch_add(1, Ordering::SeqCst);
    std::env::temp_dir().join(format!(
        "chord-w3-3-portal-{tag}-{}-{idx}.sock",
        std::process::id()
    ))
}

fn read_dbus_msg(mut stream: &UnixStream) -> Message {
    let mut header_prefix = [0u8; 16];
    stream
        .read_exact(&mut header_prefix)
        .expect("read 16-byte D-Bus header prefix");
    let body_len = u32::from_le_bytes([
        header_prefix[4],
        header_prefix[5],
        header_prefix[6],
        header_prefix[7],
    ]) as usize;
    let fields_len = u32::from_le_bytes([
        header_prefix[12],
        header_prefix[13],
        header_prefix[14],
        header_prefix[15],
    ]) as usize;
    let pad = (8 - (fields_len % 8)) % 8;
    let remaining = fields_len + pad + body_len;

    let mut full = Vec::with_capacity(16 + remaining);
    full.extend_from_slice(&header_prefix);
    let mut tail = vec![0u8; remaining];
    stream.read_exact(&mut tail).expect("read D-Bus body");
    full.extend_from_slice(&tail);

    message::parse(&full).expect("parse D-Bus message")
}

fn do_dbus_sasl_handshake_with_unix_fd(stream: &mut UnixStream) {
    let mut nul = [0u8; 1];
    stream.read_exact(&mut nul).expect("read SASL NUL byte");
    assert_eq!(nul[0], 0);

    let mut line_buf = Vec::new();
    let mut b = [0u8; 1];
    loop {
        stream.read_exact(&mut b).expect("read AUTH line");
        line_buf.push(b[0]);
        if line_buf.ends_with(b"\r\n") {
            break;
        }
    }
    let auth_line = String::from_utf8_lossy(&line_buf);
    assert!(auth_line.starts_with("AUTH EXTERNAL"));
    stream
        .write_all(b"OK 99887766554433221100aabbccddeeff\r\n")
        .expect("write OK");

    line_buf.clear();
    loop {
        stream.read_exact(&mut b).expect("read NEGOTIATE_UNIX_FD");
        line_buf.push(b[0]);
        if line_buf.ends_with(b"\r\n") {
            break;
        }
    }
    assert_eq!(&line_buf[..], b"NEGOTIATE_UNIX_FD\r\n");
    stream.write_all(b"AGREE_UNIX_FD\r\n").expect("write AGREE");

    line_buf.clear();
    loop {
        stream.read_exact(&mut b).expect("read BEGIN");
        line_buf.push(b[0]);
        if line_buf.ends_with(b"\r\n") {
            break;
        }
    }
    assert_eq!(&line_buf[..], b"BEGIN\r\n");

    let hello = read_dbus_msg(stream);
    assert_eq!(hello.member(), Some("Hello"));
    let hello_reply = message::build(
        MessageType::MethodReturn,
        1,
        &[
            (FIELD_REPLY_SERIAL, Value::U32(hello.serial)),
            (FIELD_SIGNATURE, Value::String("s".to_string())),
        ],
        "s",
        &[Value::String(":1.333".to_string())],
    )
    .expect("build Hello reply");
    stream.write_all(&hello_reply).expect("write Hello reply");
}

fn absorb_add_match_return_next_call(stream: &UnixStream, server_serial: &mut u32) -> Message {
    loop {
        let msg = read_dbus_msg(stream);
        if msg.member() == Some("AddMatch") {
            let reply = message::build(
                MessageType::MethodReturn,
                *server_serial,
                &[(FIELD_REPLY_SERIAL, Value::U32(msg.serial))],
                "",
                &[],
            )
            .expect("build AddMatch reply");
            *server_serial += 1;
            let mut s = stream;
            s.write_all(&reply).expect("write AddMatch reply");
        } else {
            return msg;
        }
    }
}

/// Spawns a mock XDG Desktop Portal server over a local Unix socket that completes
/// `CreateSession -> SelectSources -> Start -> OpenPipeWireRemote -> Session.Close`
/// and returns `(sock_path, pw_server_observer, join_handle)`.
fn spawn_mock_portal_server_success(
    tag: &str,
    expect_close: bool,
) -> (PathBuf, UnixStream, thread::JoinHandle<()>) {
    let sock_path = unique_socket_path(tag);
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path).expect("bind mock portal socket");
    let (pw_client, pw_observer) = UnixStream::pair().expect("create PipeWire socketpair");

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept client");
        do_dbus_sasl_handshake_with_unix_fd(&mut stream);
        let mut server_serial = 10u32;

        // 1. CreateSession
        let m1 = absorb_add_match_return_next_call(&stream, &mut server_serial);
        assert_eq!(m1.member(), Some("CreateSession"));
        let req_create = "/org/freedesktop/portal/desktop/request/1_333/req_create";
        let sess_path = "/org/freedesktop/portal/desktop/session/1_333/sess_live";

        let m1_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(m1.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_create.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&m1_reply).unwrap();

        // Expect AddMatch for req_create
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
                (FIELD_PATH, Value::String(req_create.to_string())),
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
        let m2 = absorb_add_match_return_next_call(&stream, &mut server_serial);
        assert_eq!(m2.member(), Some("SelectSources"));
        let req_select = "/org/freedesktop/portal/desktop/request/1_333/req_select";
        let m2_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(m2.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_select.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&m2_reply).unwrap();

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
                (FIELD_PATH, Value::String(req_select.to_string())),
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

        // 3. Start (returns Monitor stream node 301 + Window stream node 302)
        let m3 = absorb_add_match_return_next_call(&stream, &mut server_serial);
        assert_eq!(m3.member(), Some("Start"));
        let req_start = "/org/freedesktop/portal/desktop/request/1_333/req_start";
        let m3_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(m3.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_start.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&m3_reply).unwrap();

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

        let monitor_stream = Value::Struct(vec![
            Value::U32(301),
            Value::Dict(vec![
                (
                    Value::String("id".to_string()),
                    Value::Variant(Box::new(Value::String("screen-full-1".to_string()))),
                ),
                (
                    Value::String("source_type".to_string()),
                    Value::Variant(Box::new(Value::U32(1))),
                ),
                (
                    Value::String("size".to_string()),
                    Value::Variant(Box::new(Value::Struct(vec![
                        Value::I32(1920),
                        Value::I32(1080),
                    ]))),
                ),
            ]),
        ]);
        let window_stream = Value::Struct(vec![
            Value::U32(302),
            Value::Dict(vec![
                (
                    Value::String("id".to_string()),
                    Value::Variant(Box::new(Value::String("window-code".to_string()))),
                ),
                (
                    Value::String("source_type".to_string()),
                    Value::Variant(Box::new(Value::U32(2))),
                ),
                (
                    Value::String("size".to_string()),
                    Value::Variant(Box::new(Value::Struct(vec![
                        Value::I32(1280),
                        Value::I32(720),
                    ]))),
                ),
            ]),
        ]);

        let sig_start = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_start.to_string())),
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
                        Value::Variant(Box::new(Value::Array(vec![monitor_stream, window_stream]))),
                    ),
                    (
                        Value::String("restore_token".to_string()),
                        Value::Variant(Box::new(Value::String("restore_w3_3_token".to_string()))),
                    ),
                ]),
            ],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&sig_start).unwrap();

        // 4. OpenPipeWireRemote -> SCM_RIGHTS fd
        let open_pw = absorb_add_match_return_next_call(&stream, &mut server_serial);
        assert_eq!(open_pw.member(), Some("OpenPipeWireRemote"));
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

        if expect_close {
            // 5. Session.Close
            let close_msg = absorb_add_match_return_next_call(&stream, &mut server_serial);
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
        }
    });

    (sock_path, pw_observer, handle)
}

/// Spawns a mock XDG Desktop Portal server where the user cancels the `Start` dialog (`Response == 1`).
fn spawn_mock_portal_server_cancelled(tag: &str) -> (PathBuf, thread::JoinHandle<()>) {
    let sock_path = unique_socket_path(tag);
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path).expect("bind mock portal socket");

    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept client");
        do_dbus_sasl_handshake_with_unix_fd(&mut stream);
        let mut server_serial = 20u32;

        // 1. CreateSession -> OK
        let m1 = absorb_add_match_return_next_call(&stream, &mut server_serial);
        let req_create = "/org/freedesktop/portal/desktop/request/1_333/req_create_cancel";
        let sess_path = "/org/freedesktop/portal/desktop/session/1_333/sess_cancel";
        let m1_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(m1.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_create.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&m1_reply).unwrap();

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
                (FIELD_PATH, Value::String(req_create.to_string())),
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

        // 2. SelectSources -> user clicks Cancel (`Response == 1` -> `ConsentDenied`)
        let m2 = absorb_add_match_return_next_call(&stream, &mut server_serial);
        assert_eq!(m2.member(), Some("SelectSources"));
        let req_select = "/org/freedesktop/portal/desktop/request/1_333/req_select_cancel";
        let m2_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(m2.serial)),
                (FIELD_SIGNATURE, Value::String("o".to_string())),
            ],
            "o",
            &[Value::String(req_select.to_string())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&m2_reply).unwrap();

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

        let sig_cancel = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_select.to_string())),
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
        stream.write_all(&sig_cancel).unwrap();

        // 3. Pipeline abort_session automatically calls Session.Close
        let close_msg = absorb_add_match_return_next_call(&stream, &mut server_serial);
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

    (sock_path, handle)
}

fn make_nv12_captured_frame(width: u32, height: u32, pts_us: u64, seed: u8) -> CapturedFrame {
    let len = ((width * height * 3) / 2) as usize;
    let data: Vec<u8> = (0..len)
        .map(|i| seed.wrapping_add(((i * 5) % 251) as u8))
        .collect();
    CapturedFrame::new(width, height, width, DRM_FOURCC_NV12)
        .with_data(data)
        .with_timestamp_us(pts_us)
}

fn make_i420_captured_frame(width: u32, height: u32, pts_us: u64, seed: u8) -> CapturedFrame {
    let len = ((width * height * 3) / 2) as usize;
    let data: Vec<u8> = (0..len)
        .map(|i| seed.wrapping_add(((i * 11) % 253) as u8))
        .collect();
    CapturedFrame::new(width, height, width, DRM_FOURCC_YUV420)
        .with_data(data)
        .with_timestamp_us(pts_us)
}

fn make_bgra_dma_buf_captured_frame(
    width: u32,
    height: u32,
    pts_us: u64,
    b: u8,
    g: u8,
    r: u8,
    a: u8,
) -> CapturedFrame {
    let (sock_a, _sock_b) = UnixStream::pair().expect("dma_buf socketpair");
    let dma_buf =
        OwnedDmaBuf::from_raw_fd_checked(sock_a.into_raw_fd()).expect("valid OwnedDmaBuf");
    let pixels = (width * height) as usize;
    let mut data = Vec::with_capacity(pixels * 4);
    for _ in 0..pixels {
        data.extend_from_slice(&[b, g, r, a]);
    }
    CapturedFrame::new_dma_buf(
        width,
        height,
        width * 4,
        DRM_FOURCC_BGRA8888,
        dma_buf,
        DRM_FORMAT_MOD_LINEAR,
    )
    .with_data(data)
    .with_timestamp_us(pts_us)
}

// ============================================================================
// 1. POSITIVE END-TO-END: UI Go Live -> ScreenCastPortal / ScreenCastCapturePipeline
//    -> PipeWire NV12 / I420 / BGRA DMA-BUF frames -> VideoEncoder -> VideoRtpSender
//    RTP packets with RFC 3550 marker bit + Gateway Opcodes 18, 22, 19.
// ============================================================================

#[test]
fn e2e_go_live_portal_to_nv12_i420_bgra_encoder_rtp_and_gateway_opcodes_18_22_19() {
    let (sock_path, mut pw_observer, server_thread) =
        spawn_mock_portal_server_success("e2e_positive", true);
    let addr = BusAddress::Path(sock_path.clone());
    let mut portal = ScreenCastPortal::connect(&addr).expect("connect to mock portal");

    let mut app_state = AppState::new();
    app_state.start_call("u-demo-1", false);
    app_state.open_screen_share_modal();
    assert!(app_state.is_screen_share_modal_open);

    let mut selector = ScreenShareSelectorState::new();
    selector.set_resolution(ScreenShareResolution::P720);
    selector.set_fps(ScreenShareFps::Fps60);
    selector
        .select_source("screen-full-1", &mut app_state)
        .expect("select monitor source");

    // Step 1: UI Go Live via real ScreenCastPortal D-Bus handshake (`P1-2.1`)
    let mut pipeline = ScreenCastCapturePipeline::new();
    let negotiated = selector
        .confirm_go_live_with_pipeline(&mut app_state, &mut portal, &mut pipeline)
        .expect("confirm_go_live_with_pipeline should succeed");

    assert_eq!(negotiated.node_id, 301);
    assert_eq!((negotiated.width, negotiated.height), (1280, 720));
    assert_eq!(negotiated.fps, 60);
    assert!(app_state.is_screen_sharing);
    assert!(!app_state.is_screen_share_modal_open);
    assert_eq!(
        app_state.selected_screen_source_id.as_deref(),
        Some("screen-full-1")
    );
    assert!(selector.last_error.is_none());
    if let CallStatus::Connected {
        is_screen_sharing, ..
    } = app_state.active_call
    {
        assert!(is_screen_sharing);
    } else {
        panic!("expected Connected call with is_screen_sharing = true");
    }

    // Step 2: Attach started pipeline to `ScreenSharePipelineController` (`P1-2.5`)
    let enc_cfg = VideoEncoderConfig::new(VideoCodec::Vp8, 1280, 720, 60)
        .with_pixel_format(VideoFramePixelFormat::Nv12)
        .with_bitrates(500_000, 4_800_000, 8_000_000);
    let mut controller =
        ScreenSharePipelineController::new(enc_cfg, 0xCAFE_1234, DEFAULT_VIDEO_PAYLOAD_TYPE)
            .expect("create ScreenSharePipelineController");

    let guild_target = StreamSignalingTarget::guild(
        "100000000000000001",
        "200000000000000002",
        "300000000000000003",
    );
    let op18 = controller
        .attach_started_pipeline(pipeline, &guild_target)
        .expect("attach started pipeline and emit OP 18");

    assert_eq!(
        op18,
        json!({
            "op": 18,
            "d": {
                "type": "guild",
                "guild_id": "100000000000000001",
                "channel_id": "200000000000000002",
                "preferred_region": null
            }
        })
    );
    assert_eq!(
        controller.active_stream_key(),
        Some("guild:100000000000000001:200000000000000002:300000000000000003")
    );
    assert_eq!(controller.active_fd_balance(), 1);

    // Also verify DM / Call Opcode 18 payload schema (`type: "call", guild_id: null`)
    let call_target = StreamSignalingTarget::call("200000000000000002", "300000000000000003");
    assert_eq!(
        call_target.stream_key().unwrap(),
        "call:200000000000000002:300000000000000003"
    );
    assert_eq!(
        build_gateway_stream_create_op18(&call_target).unwrap(),
        json!({
            "op": 18,
            "d": {
                "type": "call",
                "guild_id": null,
                "channel_id": "200000000000000002",
                "preferred_region": null
            }
        })
    );

    // Step 3: Process Frame #1 (NV12) -> Keyframe -> Multi-packet RTP with RFC 3550 marker bit
    let nv12_frame = make_nv12_captured_frame(1280, 720, 100_000, 0x20);
    let rtp_pkts_nv12 = controller
        .process_captured_frame(nv12_frame)
        .expect("process NV12 frame");
    assert!(
        rtp_pkts_nv12.len() >= 2,
        "720p keyframe at 4.8 Mbps should fragment into >= 2 RTP packets"
    );

    let mut reassembled_nv12 = Vec::new();
    for (idx, pkt) in rtp_pkts_nv12.iter().enumerate() {
        let hdr = RtpHeader::decode(pkt).expect("decode RTP header");
        let is_last = idx + 1 == rtp_pkts_nv12.len();
        assert_eq!(
            hdr.marker, is_last,
            "RFC 3550 marker bit must be true ONLY on last fragment"
        );
        assert_eq!(hdr.ssrc, 0xCAFE_1234);
        assert_eq!(hdr.payload_type, DEFAULT_VIDEO_PAYLOAD_TYPE);
        assert_eq!(hdr.timestamp, 9_000); // 100_000 us * 90 / 1000 = 9_000
        reassembled_nv12.extend_from_slice(&pkt[RTP_FIXED_HEADER_LEN..]);
    }
    assert_eq!(&reassembled_nv12[3..6], &VP8_START_CODE);
    assert_eq!(
        parse_vp8_keyframe_dimensions(&reassembled_nv12),
        Some((1280, 720))
    );
    assert!(verify_packet_checksum(&reassembled_nv12));

    // Step 4: Process Frame #2 (I420 / DRM_FOURCC_YUV420 at +20_000 us)
    let i420_frame = make_i420_captured_frame(1280, 720, 120_000, 0x40);
    let rtp_pkts_i420 = controller
        .process_captured_frame(i420_frame)
        .expect("process I420 frame");
    assert!(!rtp_pkts_i420.is_empty());
    assert!(
        RtpHeader::decode(rtp_pkts_i420.last().unwrap())
            .unwrap()
            .marker
    );

    // Step 5: Process Frame #3 (BGRA8888 DMA-BUF at +20_000 us -> BGRA-to-RGBA swizzle)
    let bgra_dma_frame = make_bgra_dma_buf_captured_frame(1280, 720, 140_000, 10, 20, 240, 255);
    let rtp_pkts_bgra = controller
        .process_captured_frame(bgra_dma_frame)
        .expect("process BGRA DMA-BUF frame");
    assert!(!rtp_pkts_bgra.is_empty());
    assert!(
        RtpHeader::decode(rtp_pkts_bgra.last().unwrap())
            .unwrap()
            .marker
    );

    let snap_mid = controller.telemetry_snapshot();
    assert_eq!(snap_mid.frames_captured, 3);
    assert_eq!(snap_mid.frames_encoded, 3);
    assert_eq!(snap_mid.dma_buf_zero_copy_frames, 1);
    assert_eq!(snap_mid.shm_fallback_frames, 2);
    assert_eq!(snap_mid.bgra_swizzled_frames, 1);
    assert_eq!(snap_mid.active_fd_balance(), 1);

    // Step 6: Stop pipeline -> emits Opcode 19 (`STREAM_DELETE`) & closes PipeWire FD (`active_fd_balance() == 0`)
    let op19 = controller
        .stop(&mut portal)
        .expect("stop controller and close portal session");
    selector.stop_go_live(&mut app_state);
    assert!(!app_state.is_screen_sharing);

    assert_eq!(
        op19,
        json!({
            "op": 19,
            "d": {
                "stream_key": "guild:100000000000000001:200000000000000002:300000000000000003"
            }
        })
    );
    let snap_end = controller.telemetry_snapshot();
    assert_eq!(snap_end.state, ScreenCastPipelineState::Stopped);
    assert_eq!(snap_end.active_fd_balance(), 0);
    assert!(portal.cached_pipewire_fd().is_none());

    server_thread.join().expect("mock portal thread join");
    let _ = std::fs::remove_file(&sock_path);

    // Verify the remote PipeWire socket peer received EOF (0 leaked FDs)
    pw_observer
        .set_read_timeout(Some(std::time::Duration::from_millis(50)))
        .unwrap();
    let mut probe = [0u8; 1];
    let eof = matches!(pw_observer.read(&mut probe), Ok(0));
    assert!(eof, "PipeWire remote FD must be completely closed on stop");
}

// ============================================================================
// 2. NEGATIVE: User cancels portal (`Cancelled` / `ConsentDenied`) & Permission Denied
// ============================================================================

#[test]
fn portal_user_cancellation_and_permission_denied_preserve_modal_and_surface_error() {
    let (sock_path, server_thread) = spawn_mock_portal_server_cancelled("user_cancelled");
    let addr = BusAddress::Path(sock_path.clone());
    let mut portal = ScreenCastPortal::connect(&addr).expect("connect to mock portal");

    let mut app_state = AppState::new();
    app_state.open_screen_share_modal();
    let mut selector = ScreenShareSelectorState::new();

    let err = selector
        .confirm_go_live_with_portal(&mut app_state, &mut portal)
        .expect_err("user cancellation in portal must return Err");
    assert!(
        err.contains("Cancelled") || err.contains("отклонил"),
        "unexpected error message: {err}"
    );
    assert_eq!(selector.last_error.as_deref(), Some(err.as_str()));
    assert!(!app_state.is_screen_sharing);
    assert!(
        app_state.is_screen_share_modal_open,
        "modal must remain open when user cancels portal dialog"
    );

    server_thread.join().expect("join cancelled portal server");
    let _ = std::fs::remove_file(&sock_path);

    // Also test offline MockScreenCastPortalRunner for ConsentAborted and PermissionDenied
    let mut runner_aborted =
        MockScreenCastPortalRunner::failing(PortalRequestError::ConsentAborted);
    let err_aborted = selector
        .confirm_go_live_with_runner(&mut app_state, &mut runner_aborted)
        .expect_err("aborted portal must fail");
    assert!(err_aborted.contains("Cancelled") || err_aborted.contains("прерван"));
    assert!(!app_state.is_screen_sharing);

    let mut runner_perm = MockScreenCastPortalRunner::failing(PortalRequestError::Dbus(
        "org.freedesktop.portal.Error.NotAllowed: Permission denied".to_string(),
    ));
    let err_perm = selector
        .confirm_go_live_with_runner(&mut app_state, &mut runner_perm)
        .expect_err("permission denied must fail");
    assert!(err_perm.contains("NotAllowed") || err_perm.contains("Permission denied"));
    assert_eq!(selector.last_error.as_deref(), Some(err_perm.as_str()));
}

// ============================================================================
// 3. NEGATIVE & BOUNDARIES: PipeWire / DMA-BUF -> `RawVideoFrame` Bridge (`P1-2.2`)
//    Odd NV12/I420 dimensions, invalid FDs, truncated/empty DMA-BUF, stride padding,
//    BGRA-to-RGBA swizzle fidelity, and zero-copy buffer reuse / SHM fallback stats.
// ============================================================================

#[test]
fn bridge_rejects_odd_nv12_i420_dimensions_invalid_fds_and_truncated_or_empty_dma_buf() {
    // 1) FourCC mapping verification
    assert_eq!(
        map_drm_fourcc_to_pixel_format(DRM_FOURCC_NV12).unwrap(),
        (VideoFramePixelFormat::Nv12, false)
    );
    assert_eq!(
        map_drm_fourcc_to_pixel_format(DRM_FOURCC_YUV420).unwrap(),
        (VideoFramePixelFormat::I420, false)
    );
    assert_eq!(
        map_drm_fourcc_to_pixel_format(DRM_FOURCC_RGBA8888).unwrap(),
        (VideoFramePixelFormat::Rgba8, false)
    );
    assert_eq!(
        map_drm_fourcc_to_pixel_format(DRM_FOURCC_BGRA8888).unwrap(),
        (VideoFramePixelFormat::Rgba8, true)
    );
    assert_eq!(
        map_drm_fourcc_to_pixel_format(0xDEAD_BEEF).unwrap_err(),
        FrameBridgeError::UnsupportedFourCc(0xDEAD_BEEF)
    );

    // 2) Odd NV12 and I420 dimensions rejected
    let odd_nv12_w =
        CapturedFrame::new(1281, 720, 1281, DRM_FOURCC_NV12)
            .with_data(vec![0x80; (1281 * 720 * 3) / 2]);
    assert_eq!(
        convert_captured_frame_to_raw_frame(odd_nv12_w).unwrap_err(),
        FrameBridgeError::OddSubsampledDimensions {
            width: 1281,
            height: 720,
            format: VideoFramePixelFormat::Nv12,
        }
    );

    let odd_i420_h =
        CapturedFrame::new(1280, 721, 1280, DRM_FOURCC_YUV420)
            .with_data(vec![0x80; (1280 * 721 * 3) / 2]);
    assert_eq!(
        convert_captured_frame_to_raw_frame(odd_i420_h).unwrap_err(),
        FrameBridgeError::OddSubsampledDimensions {
            width: 1280,
            height: 721,
            format: VideoFramePixelFormat::I420,
        }
    );

    // 3) Out-of-range dimensions (< 64 or > 3840) rejected
    let too_small = CapturedFrame::new(32, 32, 32, DRM_FOURCC_NV12).with_data(vec![0u8; 32 * 48]);
    assert_eq!(
        convert_captured_frame_to_raw_frame(too_small).unwrap_err(),
        FrameBridgeError::InvalidDimensions {
            width: 32,
            height: 32
        }
    );

    // 4) Empty DMA-BUF / plane buffer rejected
    let (s_a, _s_b) = UnixStream::pair().unwrap();
    let dma = OwnedDmaBuf::from_raw_fd_checked(s_a.into_raw_fd()).unwrap();
    let empty_dma = CapturedFrame::new_dma_buf(
        128,
        128,
        128,
        DRM_FOURCC_NV12,
        dma.try_clone().unwrap(),
        DRM_FORMAT_MOD_LINEAR,
    );
    assert_eq!(
        convert_captured_frame_to_raw_frame(empty_dma).unwrap_err(),
        FrameBridgeError::EmptyPlaneBuffer
    );

    // 5) Truncated DMA-BUF buffer rejected
    let expected_nv12_128 = (128 * 128 * 3) / 2;
    let truncated_dma =
        CapturedFrame::new_dma_buf(128, 128, 128, DRM_FOURCC_NV12, dma, DRM_FORMAT_MOD_LINEAR)
            .with_data(vec![0x55; expected_nv12_128 - 1]);
    assert_eq!(
        convert_captured_frame_to_raw_frame(truncated_dma).unwrap_err(),
        FrameBridgeError::TruncatedPlaneBuffer {
            actual_bytes: expected_nv12_128 - 1,
            expected_bytes: expected_nv12_128,
        }
    );

    // 6) Undersized stride rejected
    let bad_stride = CapturedFrame::new(128, 128, 128 * 4 - 4, DRM_FOURCC_BGRA8888)
        .with_data(vec![0xAA; 128 * 128 * 4]);
    assert_eq!(
        convert_captured_frame_to_raw_frame(bad_stride).unwrap_err(),
        FrameBridgeError::InvalidStride {
            stride_bytes: 508,
            minimum_stride_bytes: 512,
        }
    );

    // 7) Invalid FD (< 0) and DRM_FORMAT_MOD_INVALID rejected on DmaBufPlaneDescriptor
    let bad_fd_plane = DmaBufPlaneDescriptor {
        dma_buf_fd: -1,
        offset_bytes: 0,
        stride_bytes: 128,
        size_bytes: expected_nv12_128,
        drm_fourcc: DRM_FOURCC_NV12,
        drm_modifier: DRM_FORMAT_MOD_LINEAR,
    };
    assert_eq!(
        convert_dma_buf_plane_to_raw_frame(
            128,
            128,
            &bad_fd_plane,
            &vec![0u8; expected_nv12_128],
            0
        )
        .unwrap_err(),
        FrameBridgeError::InvalidFd(-1)
    );

    let bad_mod_plane = DmaBufPlaneDescriptor {
        dma_buf_fd: 10,
        offset_bytes: 0,
        stride_bytes: 128,
        size_bytes: expected_nv12_128,
        drm_fourcc: DRM_FOURCC_NV12,
        drm_modifier: DRM_FORMAT_MOD_INVALID,
    };
    assert_eq!(
        convert_dma_buf_plane_to_raw_frame(
            128,
            128,
            &bad_mod_plane,
            &vec![0u8; expected_nv12_128],
            0
        )
        .unwrap_err(),
        FrameBridgeError::InvalidDrmModifier(DRM_FORMAT_MOD_INVALID)
    );

    // 8) BGRA-to-RGBA swizzle with padded stride preserves padding and swaps B <-> R accurately
    let width = 64u32;
    let height = 64u32;
    let padded_stride = 64 * 4 + 16; // 272 bytes per row (16 bytes padding)
    let mut padded_bgra = vec![0xEEu8; (padded_stride * height) as usize];
    // Write pixel (0, 0) as BGRA = [11, 22, 33, 44]
    padded_bgra[0..4].copy_from_slice(&[11, 22, 33, 44]);
    swizzle_bgra_to_rgba_in_place(&mut padded_bgra, width, height, padded_stride)
        .expect("padded BGRA swizzle should succeed");
    assert_eq!(&padded_bgra[0..4], &[33, 22, 11, 44]);
    assert_eq!(
        &padded_bgra[(width * 4) as usize..padded_stride as usize],
        &[0xEEu8; 16]
    );

    // 9) PipeWireFrameBridge buffer reuse & DMA-BUF / SHM bookkeeping
    let mut bridge = PipeWireFrameBridge::new();
    let shm_rgba = CapturedFrame::new(64, 64, 64 * 4, DRM_FOURCC_RGBA8888)
        .with_data(vec![0x77; 64 * 64 * 4])
        .with_timestamp_us(5_000);
    let raw1 = bridge
        .convert_captured_frame_reusing_buffer(&shm_rgba)
        .expect("convert with reusable buffer");
    assert_eq!(raw1.pixel_format, VideoFramePixelFormat::Rgba8);
    assert_eq!(bridge.stats().buffer_reuses, 1);
    assert_eq!(bridge.stats().shm_fallback_frames, 1);
}

// ============================================================================
// 4. PAUSE / RESUME & RTCP PLI RECOVERY:
//    Pause emits Opcode 22 (`paused: true`), skips encoding during pause,
//    resumes with Opcode 22 (`paused: false`) and forces an immediate keyframe.
// ============================================================================

#[test]
fn pause_emits_op22_skips_encoding_and_resumes_with_forced_keyframe_and_pli_recovery() {
    let enc_cfg = VideoEncoderConfig::new(VideoCodec::Vp8, 640, 360, 60)
        .with_pixel_format(VideoFramePixelFormat::Nv12)
        .with_keyframe_interval(120);
    let mut controller =
        ScreenSharePipelineController::new(enc_cfg, 0x1122_3344, DEFAULT_VIDEO_PAYLOAD_TYPE)
            .expect("create controller");

    let target = StreamSignalingTarget::guild(
        "111111111111111111",
        "222222222222222222",
        "333333333333333333",
    );
    let negotiated = chord_platform::screencast::NegotiatedStreamParams {
        node_id: 404,
        stream_id: Some("pw-node-404".to_string()),
        mapping_id: None,
        target_kind: CaptureTargetKind::Monitor,
        width: 640,
        height: 360,
        stride_bytes: 640,
        fps: 60,
        frame_interval_us: 16_666,
        rtp_clock_hz: 90_000,
        rtp_timestamp_step: 1_500,
        target_bitrate_bps: 2_500_000,
    };
    let op18 = controller
        .start_with_negotiated(negotiated, &target)
        .expect("start_with_negotiated");
    assert_eq!(op18["op"], 18);

    // Frame 0 -> initial keyframe
    let pkts0 = controller
        .process_captured_frame(make_nv12_captured_frame(640, 360, 0, 1))
        .expect("frame 0");
    assert!(!pkts0.is_empty());
    assert_eq!(controller.telemetry_snapshot().keyframes_encoded, 1);

    // Frame 1 -> inter-frame
    let pkts1 = controller
        .process_captured_frame(make_nv12_captured_frame(640, 360, 20_000, 2))
        .expect("frame 1");
    assert!(!pkts1.is_empty());
    assert_eq!(controller.telemetry_snapshot().keyframes_encoded, 1);
    assert_eq!(controller.telemetry_snapshot().inter_frames_encoded, 1);

    // Pause stream -> emits Opcode 22 (`paused: true`)
    let op22_pause = controller.set_paused(true).expect("set_paused(true)");
    assert_eq!(
        op22_pause,
        json!({
            "op": 22,
            "d": {
                "stream_key": "guild:111111111111111111:222222222222222222:333333333333333333",
                "paused": true
            }
        })
    );

    // Frames arriving while paused MUST be skipped without encoding
    let pkts_paused_1 = controller
        .process_captured_frame(make_nv12_captured_frame(640, 360, 40_000, 3))
        .expect("paused frame 1");
    let pkts_paused_2 = controller
        .process_captured_frame(make_nv12_captured_frame(640, 360, 60_000, 4))
        .expect("paused frame 2");
    assert!(pkts_paused_1.is_empty());
    assert!(pkts_paused_2.is_empty());
    assert_eq!(controller.telemetry_snapshot().frames_encoded, 2);
    assert_eq!(controller.telemetry_snapshot().paused_frames_skipped, 2);

    // Resume stream -> emits Opcode 22 (`paused: false`) and forces next frame to be a keyframe
    let op22_resume = controller.set_paused(false).expect("set_paused(false)");
    assert_eq!(
        op22_resume,
        json!({
            "op": 22,
            "d": {
                "stream_key": "guild:111111111111111111:222222222222222222:333333333333333333",
                "paused": false
            }
        })
    );

    let pkts_resumed = controller
        .process_captured_frame(make_nv12_captured_frame(640, 360, 80_000, 5))
        .expect("resumed frame must encode as keyframe");
    assert!(!pkts_resumed.is_empty());
    assert_eq!(
        controller.telemetry_snapshot().keyframes_encoded,
        2,
        "first frame after resume must be forced as a keyframe"
    );

    // Next frame is an inter-frame, then trigger RTCP PLI `request_keyframe()`
    let _pkts_inter = controller
        .process_captured_frame(make_nv12_captured_frame(640, 360, 100_000, 6))
        .expect("inter frame");
    assert_eq!(controller.telemetry_snapshot().keyframes_encoded, 2);

    controller.request_keyframe();
    let _pkts_pli = controller
        .process_captured_frame(make_nv12_captured_frame(640, 360, 120_000, 7))
        .expect("PLI keyframe");
    assert_eq!(controller.telemetry_snapshot().keyframes_encoded, 3);
    assert_eq!(controller.telemetry_snapshot().forced_keyframes, 2);
}

// ============================================================================
// 5. SOURCE SWITCHING, EMPTY STREAM / UNKNOWN SOURCE RECOVERY & FD BALANCE == 0
// ============================================================================

struct PortalTestBackend {
    held_peers: Vec<UnixStream>,
}

impl Default for PortalTestBackend {
    fn default() -> Self {
        Self {
            held_peers: Vec::new(),
        }
    }
}

impl PortalBackend for PortalTestBackend {
    fn create_session(&mut self, _r: &str, _s: &str) -> Result<String, PortalError> {
        Ok("/org/freedesktop/portal/desktop/session/1_999/s1".to_string())
    }
    fn select_sources(&mut self, _p: &str, _c: &PortalSessionConfig) -> Result<(), PortalError> {
        Ok(())
    }
    fn start(&mut self, _p: &str, _r: &str) -> Result<(u32, Option<String>), PortalError> {
        Ok((77, Some("tok_secret_w3_3".to_string())))
    }
    fn open_pipewire_remote(&mut self, _p: &str) -> Result<Vec<OwnedPortalFd>, PortalError> {
        let (pw, peer) = UnixStream::pair().map_err(|e| PortalError::IoError(e.to_string()))?;
        self.held_peers.push(peer);
        Ok(vec![OwnedPortalFd::from_raw_fd_checked(pw.into_raw_fd())?])
    }
    fn close_session(&mut self, _p: &str) -> Result<(), PortalError> {
        Ok(())
    }
}

#[test]
fn source_switching_empty_streams_unknown_source_and_zero_fd_leak_telemetry_contract() {
    let (sock_path, _pw_observer, server_thread) =
        spawn_mock_portal_server_success("switch_source", true);
    let addr = BusAddress::Path(sock_path.clone());
    let mut portal = ScreenCastPortal::connect(&addr).expect("connect to mock portal");

    let mut app_state = AppState::new();
    let mut selector = ScreenShareSelectorState::new();
    let mut pipeline = ScreenCastCapturePipeline::new();

    // Start on monitor (node 301)
    let mon_params = selector
        .confirm_go_live_with_pipeline(&mut app_state, &mut portal, &mut pipeline)
        .expect("start monitor share");
    assert_eq!(mon_params.node_id, 301);

    // Switch live source to window-code (node 302) at 720p @ 30 FPS
    selector.apply_quality_preset(ScreenShareQualityPreset::P720Fps30);
    let win_params = selector
        .switch_live_source_with_pipeline("window-code", &mut app_state, &mut pipeline)
        .expect("switch to window-code");
    assert_eq!(win_params.node_id, 302);
    assert_eq!((win_params.width, win_params.height), (1280, 720));
    assert_eq!(win_params.fps, 30);
    assert_eq!(selector.active_tab, ScreenShareTab::Windows);
    assert_eq!(
        app_state.selected_screen_source_id.as_deref(),
        Some("window-code")
    );

    // Unknown source ID error recovery (does not panic or corrupt active state)
    let bad_switch = selector.switch_live_source_with_pipeline(
        "unknown-window-999",
        &mut app_state,
        &mut pipeline,
    );
    assert!(bad_switch.is_err());
    assert!(selector.last_error.is_some());

    // Stop cleanly via `stop_go_live_with_pipeline`
    selector
        .stop_go_live_with_pipeline(&mut app_state, &mut portal, &mut pipeline)
        .expect("stop_go_live_with_pipeline");
    assert!(!app_state.is_screen_sharing);
    assert_eq!(pipeline.state(), ScreenCastPipelineState::Stopped);

    server_thread.join().expect("join server");
    let _ = std::fs::remove_file(&sock_path);

    // Empty stream error recovery on `AppState::empty()`
    let mut empty_state = AppState::empty();
    let mut runner_empty = MockScreenCastPortalRunner::failing(PortalRequestError::EmptyStreams);
    assert!(
        selector
            .confirm_go_live_with_runner(&mut empty_state, &mut runner_empty)
            .is_err()
    );

    // PipeWirePortalManager + negotiated_dma_buf_to_raw_frame + active_fd_balance() == 0
    let mut pw_mgr = PipeWirePortalManager::new(PortalTestBackend::default());
    let sess_cfg = PortalSessionConfig::default_monitor_and_window();
    let dma_req = DmaBufNegotiationRequest {
        width: 128,
        height: 128,
        fps: 60,
        drm_fourcc: DRM_FOURCC_BGRA8888,
        candidate_stride: 128 * 4,
        advertised_modifiers: vec![DRM_FORMAT_MOD_LINEAR],
        allow_shm_fallback: true,
    };
    let neg_dma = pw_mgr
        .open_session(&sess_cfg, &dma_req)
        .expect("open_session");
    let raw_from_mgr = pw_mgr
        .convert_frame_for_encoder(128, 128, &vec![0x42; 128 * 128 * 4], 16_666)
        .expect("convert_frame_for_encoder");
    assert_eq!(raw_from_mgr.pixel_format, VideoFramePixelFormat::Rgba8);

    let direct_raw = negotiated_dma_buf_to_raw_frame(
        &neg_dma,
        128,
        128,
        pw_mgr.telemetry_snapshot().active_fd,
        &vec![0x42; 128 * 128 * 4],
        33_333,
    )
    .expect("negotiated_dma_buf_to_raw_frame");
    assert_eq!(direct_raw.width, 128);

    pw_mgr.close().expect("close PipeWirePortalManager");
    assert_eq!(pw_mgr.telemetry_snapshot().active_fd_balance(), 0);

    // Stream key & Opcode 19/22 negative validation
    assert!(validate_stream_key("").is_err());
    assert!(validate_stream_key("guild:1:2").is_err());
    assert!(build_gateway_stream_delete_op19("invalid_key").is_err());
    assert!(build_gateway_stream_set_paused_op22("invalid_key", true).is_err());
    let mut unstarted = ScreenSharePipelineController::new(
        VideoEncoderConfig::new(VideoCodec::Vp8, 128, 128, 30),
        1234,
        96,
    )
    .unwrap();
    assert_eq!(
        unstarted.set_paused(true).unwrap_err(),
        ScreenSharePipelineError::NotStreaming
    );
}
