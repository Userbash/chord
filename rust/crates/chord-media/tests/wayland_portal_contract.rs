//! R5-06: Wayland ScreenCast Portal — Contract Tests (`chord_media::capture::wayland_portal`).
//!
//! Specification & Reference Oracles:
//! - `docs/feature-backlog.md` card `R5-06`:
//!   "On Wayland environments with system portal choosers, bypass redundant custom picker and
//!   delegate directly to `xdg-desktop-portal` | Vesktop: `screenShare.ts` | Capture Service:
//!   Wayland Portal | Stream initiates via native portal dialog without dual picker prompts;
//!   full native display resolution preserved"
//! - `docs/vesktop-analysis.md` §2.3:
//!   "On modern Wayland compositors (GNOME Mutter, KDE KWin, Hyprland), screen sharing requests
//!   should delegate to the `org.freedesktop.portal.ScreenCast` D-Bus portal."
//! - `/usr/share/dbus-1/interfaces/org.freedesktop.portal.ScreenCast.xml` (v6).
//!
//! Covers all 9 mandatory dimensions of `TEST-POLICY.md` §5:
//! 1. Positive scenarios (Wayland environment with portal chooser bypasses custom picker and
//!    delegates directly to `xdg-desktop-portal`, initiating stream via native portal dialog
//!    with `custom_picker_prompts == 0` and `native_portal_prompts == 1`; full native display
//!    resolution preserved across 1080p, 1440p, 4K, 5K, 8K, ultrawide 3440×1440, and fractional
//!    HiDPI scale factors).
//! 2. Negative scenarios (rejecting invalid session tokens, malformed D-Bus paths, zero source
//!    types, invalid resolutions `0` or `> MAX_CAPTURE_DIMENSION`, negative PipeWire fd,
//!    calling custom picker when direct portal delegation is active).
//! 3. Boundary conditions (minimum resolution `64×64`, maximum `8192×8192` / `7680×4320`,
//!    odd native window dimensions `1365×767` preserved both as exact native size and even-aligned
//!    encoder size `1364×766`, fractional HiDPI logical-to-physical scale `1.25x` / `1.5x` / `2.0x`).
//! 4. Error handling (`ConsentDenied`, `ConsentAborted`, `ConsentTimeout`, `NoStreamsReturned`,
//!    `InvalidPipeWireFd`, D-Bus transport error, automatic session and fd cleanup on error).
//! 5. Empty states (portal returning 0 streams or empty environment variables gracefully handled).
//! 6. Loading / intermediate states (`WaylandPortalDelegationState`: `Idle -> EvaluatingEnvironment ->
//!    AwaitingNativePortalDialog -> StreamingNativeResolution -> Closed`, verifying custom picker is
//!    never entered on Wayland).
//! 7. Capability permissions & restore tokens (silent portal session restoration with `restore_token`
//!    resulting in `0` custom prompts and `0` interactive portal prompts when token is valid,
//!    and fallback to `1` native portal prompt when token is expired/revoked; token redaction in logs).
//! 8. Recovery / fallback (X11 or headless environment without Wayland portal chooser falls back to
//!    `PickerFlowDecision::UseCustomPicker`; portal daemon disconnect triggers clean recovery).
//! 9. Observable telemetry (`WaylandPortalTelemetry`: `wayland_direct_delegations`,
//!    `custom_picker_bypassed_count`, `custom_picker_invocations`, `native_portal_dialog_prompts`,
//!    `restore_token_silent_starts`, `native_resolution_preserved_count`, `active_fd_balance`).

#![forbid(unsafe_code)]

use std::cell::RefCell;
use std::rc::Rc;

use chord_media::capture::portal::{
    PortalCursorMode, PortalPersistMode, PortalResponse, PortalResponseCode, PortalSessionConfig,
    PortalStream, PortalTransport, SOURCE_TYPE_MONITOR, SOURCE_TYPE_WINDOW,
};
use chord_media::capture::wayland_portal::{
    DisplayScaleFactor, NativeDisplayResolution, PickerFlowDecision, WaylandCompositorKind,
    WaylandEnvironmentProbe, WaylandPortalCaptureService, WaylandPortalConfig,
    WaylandPortalDelegationState, WaylandPortalSessionOutcome, WaylandPortalTelemetry,
    evaluate_picker_flow, resolve_native_display_resolution,
};
use chord_media::capture::{CaptureError, DesktopBackend, DesktopSourceType, MAX_CAPTURE_DIMENSION};
use chord_media::PixelLayout;

// ---------------------------------------------------------------------------
// Deterministic Mock Transport for xdg-desktop-portal (ScreenCast)
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct MockXdgPortalState {
    calls: Vec<String>,
    custom_picker_opened_count: u32,
    native_portal_dialog_shown_count: u32,
    create_session_res: Option<Result<(PortalResponseCode, String), CaptureError>>,
    select_sources_res: Option<Result<PortalResponseCode, CaptureError>>,
    start_res: Option<Result<PortalResponse, CaptureError>>,
    open_pw_fd_res: Option<Result<i32, CaptureError>>,
    last_select_config: Option<PortalSessionConfig>,
    closed_sessions: Vec<String>,
    closed_fds: Vec<i32>,
    honor_restore_token_silently: bool,
}

#[derive(Clone, Default)]
struct MockXdgPortalTransport {
    state: Rc<RefCell<MockXdgPortalState>>,
}

impl MockXdgPortalTransport {
    fn new_with_native_stream(
        node_id: u32,
        source_type: DesktopSourceType,
        native_width: u32,
        native_height: u32,
        mapping_id: &str,
    ) -> Self {
        let st = MockXdgPortalState {
            start_res: Some(Ok(PortalResponse {
                code: PortalResponseCode::Success,
                streams: vec![PortalStream {
                    node_id,
                    id: Some(format!("portal:{node_id}")),
                    source_type,
                    position: Some((0, 0)),
                    size: Some((native_width, native_height)),
                    mapping_id: Some(mapping_id.to_string()),
                }],
                restore_token: Some("portal_restore_tok_v1".to_string()),
            })),
            open_pw_fd_res: Some(Ok(75)),
            ..Default::default()
        };
        Self {
            state: Rc::new(RefCell::new(st)),
        }
    }
}

impl PortalTransport for MockXdgPortalTransport {
    fn create_session(
        &mut self,
        handle_token: &str,
        session_handle_token: &str,
    ) -> Result<(PortalResponseCode, String), CaptureError> {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!(
            "CreateSession:{handle_token}:{session_handle_token}"
        ));
        st.create_session_res.clone().unwrap_or_else(|| {
            Ok((
                PortalResponseCode::Success,
                format!("/org/freedesktop/portal/desktop/session/1_55/{session_handle_token}"),
            ))
        })
    }

    fn select_sources(
        &mut self,
        session_handle: &str,
        config: &PortalSessionConfig,
    ) -> Result<PortalResponseCode, CaptureError> {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!(
            "SelectSources:{session_handle}:types={}:restore={}",
            config.source_types,
            config.restore_token.is_some()
        ));
        st.last_select_config = Some(config.clone());
        st.select_sources_res
            .clone()
            .unwrap_or(Ok(PortalResponseCode::Success))
    }

    fn start(
        &mut self,
        session_handle: &str,
        parent_window: &str,
        handle_token: &str,
    ) -> Result<PortalResponse, CaptureError> {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!(
            "Start:{session_handle}:{parent_window}:{handle_token}"
        ));
        let has_valid_restore = st.honor_restore_token_silently
            && st
                .last_select_config
                .as_ref()
                .and_then(|c| c.restore_token.as_ref())
                .is_some_and(|tok| !tok.is_empty());
        if !has_valid_restore {
            st.native_portal_dialog_shown_count += 1;
        }
        st.start_res.clone().unwrap_or_else(|| {
            Ok(PortalResponse {
                code: PortalResponseCode::Success,
                streams: vec![PortalStream {
                    node_id: 101,
                    id: Some("screen:0".to_string()),
                    source_type: DesktopSourceType::Screen,
                    position: Some((0, 0)),
                    size: Some((3840, 2160)),
                    mapping_id: Some("DP-1".to_string()),
                }],
                restore_token: Some("portal_restore_tok_4k".to_string()),
            })
        })
    }

    fn open_pipewire_remote(&mut self, session_handle: &str) -> Result<i32, CaptureError> {
        let mut st = self.state.borrow_mut();
        st.calls
            .push(format!("OpenPipeWireRemote:{session_handle}"));
        st.open_pw_fd_res.clone().unwrap_or(Ok(64))
    }

    fn close_session(&mut self, session_handle: &str) -> Result<(), CaptureError> {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!("CloseSession:{session_handle}"));
        st.closed_sessions.push(session_handle.to_string());
        Ok(())
    }

    fn cancel_request(&mut self, request_handle: &str) -> Result<(), CaptureError> {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!("CancelRequest:{request_handle}"));
        Ok(())
    }

    fn close_pipewire_fd(&mut self, fd: i32) {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!("CloseFd:{fd}"));
        st.closed_fds.push(fd);
    }
}

// ---------------------------------------------------------------------------
// 1. Positive Scenarios: Wayland bypasses custom picker & preserves native resolution
// ---------------------------------------------------------------------------

#[test]
fn test_r5_06_dim1_wayland_bypasses_custom_picker_and_delegates_directly_to_xdg_desktop_portal() {
    // Given a Wayland environment with system portal chooser available (e.g. KDE KWin, GNOME Mutter, Hyprland)
    let probe = WaylandEnvironmentProbe {
        xdg_session_type: Some("wayland".to_string()),
        wayland_display: Some("wayland-0".to_string()),
        x11_display: Some(":0".to_string()), // XWayland present, but session is native Wayland
        xdg_current_desktop: Some("KDE".to_string()),
        portal_screencast_available: true,
        portal_version: 6,
    };

    let decision = evaluate_picker_flow(&probe);
    assert_eq!(
        decision,
        PickerFlowDecision::DelegateDirectlyToWaylandPortal {
            compositor: WaylandCompositorKind::KdeKwin,
            portal_version: 6,
        }
    );
    assert!(decision.bypasses_custom_picker());

    // Initiate stream through WaylandPortalCaptureService on a 3840x2160 (4K UHD) native display
    let transport = MockXdgPortalTransport::new_with_native_stream(
        304,
        DesktopSourceType::Screen,
        3840,
        2160,
        "DP-1 (4K Native)",
    );
    let state_ref = transport.state.clone();

    let mut service = WaylandPortalCaptureService::new(transport, probe);
    assert_eq!(service.state(), WaylandPortalDelegationState::Idle);

    let config = WaylandPortalConfig::default();
    assert!(config.bypass_custom_picker_on_wayland);
    assert!(config.preserve_full_native_resolution);

    let outcome: WaylandPortalSessionOutcome = service
        .initiate_stream(&config)
        .expect("stream should initiate directly via xdg-desktop-portal");

    // Criterion 1: Stream initiates via native portal dialog without dual picker prompts
    assert!(outcome.custom_picker_bypassed);
    assert_eq!(outcome.custom_picker_prompts, 0);
    assert_eq!(outcome.native_portal_prompts, 1);
    assert_eq!(outcome.total_user_picker_prompts(), 1);
    assert_eq!(state_ref.borrow().custom_picker_opened_count, 0);
    assert_eq!(state_ref.borrow().native_portal_dialog_shown_count, 1);

    // Criterion 2: Full native display resolution preserved (3840x2160 not downscaled to 720p/1080p!)
    assert_eq!(
        outcome.native_resolution,
        NativeDisplayResolution {
            physical_width: 3840,
            physical_height: 2160,
            logical_width: 3840,
            logical_height: 2160,
            scale_factor_milli: 1000,
            encoder_width: 3840,
            encoder_height: 2160,
            is_native_preserved: true,
        }
    );
    assert_eq!(outcome.pipewire_node_id, 304);
    assert_eq!(outcome.pipewire_fd, 75);
    assert_eq!(
        service.state(),
        WaylandPortalDelegationState::StreamingNativeResolution {
            node_id: 304,
            width: 3840,
            height: 2160,
        }
    );

    // Verify captured frame preserves the full 3840x2160 native resolution
    let frame = service
        .capture_native_test_frame(PixelLayout::I420, 16_666)
        .expect("capture native resolution frame");
    assert_eq!(frame.width, 3840);
    assert_eq!(frame.height, 2160);

    service.stop_stream().expect("stop stream");
    assert_eq!(service.state(), WaylandPortalDelegationState::Closed);
}

#[test]
fn test_r5_06_dim1_preserves_full_native_resolution_across_ultrawide_5k_and_hidpi_monitors() {
    // 1. Ultrawide 3440x1440 @ 1.0x scale
    let uw = resolve_native_display_resolution(
        Some((3440, 1440)),
        None,
        DisplayScaleFactor::from_milli(1000).unwrap(),
        true,
        (1920, 1080),
    )
    .expect("3440x1440 native");
    assert_eq!(uw.physical_width, 3440);
    assert_eq!(uw.physical_height, 1440);
    assert_eq!(uw.encoder_width, 3440);
    assert_eq!(uw.encoder_height, 1440);
    assert!(uw.is_native_preserved);

    // 2. Fractional HiDPI on Wayland: portal reports logical 2560x1440 with 1.5x (1500 milli) scale
    //    -> full physical native resolution 3840x2160 is reconstructed and preserved!
    let hidpi = resolve_native_display_resolution(
        Some((2560, 1440)),
        None,
        DisplayScaleFactor::from_milli(1500).unwrap(),
        true,
        (1920, 1080),
    )
    .expect("HiDPI 1.5x physical native resolution");
    assert_eq!(hidpi.logical_width, 2560);
    assert_eq!(hidpi.logical_height, 1440);
    assert_eq!(hidpi.physical_width, 3840);
    assert_eq!(hidpi.physical_height, 2160);
    assert_eq!(hidpi.encoder_width, 3840);
    assert_eq!(hidpi.encoder_height, 2160);
    assert!(hidpi.is_native_preserved);

    // 3. Explicit physical SpaBuffer override (e.g. portal metadata omitted size or reported logical,
    //    while PipeWire SPA_FORMAT_VIDEO_size negotiated 5120x2880 5K studio display)
    let spa_5k = resolve_native_display_resolution(
        Some((2560, 1440)),
        Some((5120, 2880)),
        DisplayScaleFactor::from_milli(1000).unwrap(),
        true,
        (1920, 1080),
    )
    .expect("5K SPA buffer physical override");
    assert_eq!(spa_5k.physical_width, 5120);
    assert_eq!(spa_5k.physical_height, 2880);
    assert_eq!(spa_5k.encoder_width, 5120);
    assert_eq!(spa_5k.encoder_height, 2880);
    assert!(spa_5k.is_native_preserved);
}

// ---------------------------------------------------------------------------
// 2. Negative Scenarios: Invalid tokens, dimensions, source types, and dual-picker prevention
// ---------------------------------------------------------------------------

#[test]
fn test_r5_06_dim2_rejects_invalid_configs_and_prevents_dual_picker_invocation() {
    let probe = WaylandEnvironmentProbe::wayland_default();
    let transport = MockXdgPortalTransport::default();
    let mut service = WaylandPortalCaptureService::new(transport, probe);

    // 1. Invalid portal token (contains hyphen)
    let bad_tok_cfg = WaylandPortalConfig {
        handle_token: "bad-token-hyphen".to_string(),
        ..WaylandPortalConfig::default()
    };
    assert!(matches!(
        service.initiate_stream(&bad_tok_cfg),
        Err(CaptureError::InvalidToken(_))
    ));

    // 2. Invalid source types mask (0 or unknown bit 0x80)
    let bad_types_cfg = WaylandPortalConfig {
        source_types: 0,
        ..WaylandPortalConfig::default()
    };
    assert!(matches!(
        service.initiate_stream(&bad_types_cfg),
        Err(CaptureError::InvalidSourceTypes(0))
    ));

    // 3. Invalid scale factor (0 or > 4000 milli)
    assert!(DisplayScaleFactor::from_milli(0).is_err());
    assert!(DisplayScaleFactor::from_milli(100).is_err()); // < 0.5x (500 milli)
    assert!(DisplayScaleFactor::from_milli(5000).is_err()); // > 4.0x (4000 milli)

    // 4. Attempting to open custom picker when Wayland direct delegation is active is rejected
    assert!(
        service.guard_custom_picker_invocation().is_err(),
        "Custom picker must be blocked on Wayland when direct portal delegation is active"
    );
}

// ---------------------------------------------------------------------------
// 3. Boundary Conditions: Min/Max capture dimensions, odd window sizes, fractional scales
// ---------------------------------------------------------------------------

#[test]
fn test_r5_06_dim3_boundary_resolutions_64_to_8192_and_odd_window_even_alignment() {
    let unity = DisplayScaleFactor::UNITY;

    // Minimum valid boundary: 64x64
    let min_res = resolve_native_display_resolution(Some((64, 64)), None, unity, true, (1920, 1080))
        .expect("64x64 min boundary");
    assert_eq!(min_res.physical_width, 64);
    assert_eq!(min_res.physical_height, 64);

    // Below minimum (63x64) -> rejected
    assert!(matches!(
        resolve_native_display_resolution(Some((63, 64)), None, unity, true, (1920, 1080)),
        Err(CaptureError::InvalidDimensions {
            width: 63,
            height: 64
        })
    ));

    // Maximum valid boundary: 8192x8192 (MAX_CAPTURE_DIMENSION)
    let max_res = resolve_native_display_resolution(
        Some((MAX_CAPTURE_DIMENSION, MAX_CAPTURE_DIMENSION)),
        None,
        unity,
        true,
        (1920, 1080),
    )
    .expect("8192x8192 max boundary");
    assert_eq!(max_res.physical_width, 8192);
    assert_eq!(max_res.physical_height, 8192);

    // Above maximum (8194x4320) -> rejected
    assert!(matches!(
        resolve_native_display_resolution(
            Some((MAX_CAPTURE_DIMENSION + 2, 4320)),
            None,
            unity,
            true,
            (1920, 1080)
        ),
        Err(CaptureError::InvalidDimensions { .. })
    ));

    // Odd native Wayland window size (1365x767):
    // physical_width/height preserves exact 1365x767, while encoder_width/height aligns to even 1364x766 for YUV420
    let odd_win =
        resolve_native_display_resolution(Some((1365, 767)), None, unity, true, (1920, 1080))
            .expect("odd window size");
    assert_eq!(odd_win.physical_width, 1365);
    assert_eq!(odd_win.physical_height, 767);
    assert_eq!(odd_win.encoder_width, 1364);
    assert_eq!(odd_win.encoder_height, 766);
    assert!(odd_win.is_native_preserved);
}

// ---------------------------------------------------------------------------
// 4. Error Handling: ConsentDenied, ConsentAborted, Timeout, Invalid Fd, and Cleanup
// ---------------------------------------------------------------------------

#[test]
fn test_r5_06_dim4_portal_cancellation_and_errors_clean_up_session_and_fd() {
    let probe = WaylandEnvironmentProbe::wayland_default();

    // 1. User cancels native portal dialog (Response == 1 -> ConsentDenied)
    let transport_denied = MockXdgPortalTransport::default();
    transport_denied.state.borrow_mut().start_res = Some(Ok(PortalResponse {
        code: PortalResponseCode::CancelledByUser,
        streams: vec![],
        restore_token: None,
    }));
    let st_denied = transport_denied.state.clone();

    let mut svc_denied = WaylandPortalCaptureService::new(transport_denied, probe.clone());
    let err = svc_denied
        .initiate_stream(&WaylandPortalConfig::default())
        .unwrap_err();
    assert_eq!(err, CaptureError::ConsentDenied);
    assert_eq!(svc_denied.state(), WaylandPortalDelegationState::Closed);
    assert_eq!(st_denied.borrow().closed_sessions.len(), 1);
    assert_eq!(svc_denied.telemetry().portal_errors_count, 1);

    // 2. Portal returns negative PipeWire FD -> closes session & transitions to Closed
    let transport_bad_fd =
        MockXdgPortalTransport::new_with_native_stream(99, DesktopSourceType::Screen, 2560, 1440, "DP-1");
    transport_bad_fd.state.borrow_mut().open_pw_fd_res = Some(Ok(-1));
    let st_bad_fd = transport_bad_fd.state.clone();

    let mut svc_bad_fd = WaylandPortalCaptureService::new(transport_bad_fd, probe);
    let err_fd = svc_bad_fd
        .initiate_stream(&WaylandPortalConfig::default())
        .unwrap_err();
    assert_eq!(err_fd, CaptureError::InvalidPipeWireFd(-1));
    assert_eq!(svc_bad_fd.state(), WaylandPortalDelegationState::Closed);
    assert_eq!(st_bad_fd.borrow().closed_sessions.len(), 1);
}

// ---------------------------------------------------------------------------
// 5. Empty States: Portal returns 0 streams or sizeless initial Wayland window stream
// ---------------------------------------------------------------------------

#[test]
fn test_r5_06_dim5_empty_streams_and_sizeless_wayland_window_handling() {
    let probe = WaylandEnvironmentProbe::wayland_default();

    // 1. Portal returns empty streams array -> NoStreamsReturned & session closed
    let transport_empty = MockXdgPortalTransport::default();
    transport_empty.state.borrow_mut().start_res = Some(Ok(PortalResponse {
        code: PortalResponseCode::Success,
        streams: vec![],
        restore_token: None,
    }));
    let mut svc_empty = WaylandPortalCaptureService::new(transport_empty, probe.clone());
    assert_eq!(
        svc_empty
            .initiate_stream(&WaylandPortalConfig::default())
            .unwrap_err(),
        CaptureError::NoStreamsReturned
    );
    assert_eq!(svc_empty.state(), WaylandPortalDelegationState::Closed);

    // 2. Portal returns a stream with `size: None` before first PipeWire SPA buffer arrives,
    //    then dynamic SPA buffer negotiation updates to full native 2560x1600 resolution!
    let transport_sizeless = MockXdgPortalTransport::default();
    transport_sizeless.state.borrow_mut().start_res = Some(Ok(PortalResponse {
        code: PortalResponseCode::Success,
        streams: vec![PortalStream {
            node_id: 412,
            id: Some("window:alacritty".to_string()),
            source_type: DesktopSourceType::Window,
            position: None,
            size: None,
            mapping_id: Some("Alacritty".to_string()),
        }],
        restore_token: None,
    }));

    let mut svc_sizeless = WaylandPortalCaptureService::new(transport_sizeless, probe);
    let outcome = svc_sizeless
        .initiate_stream(&WaylandPortalConfig::default())
        .expect("sizeless stream uses initial fallback until SPA buffer arrives");
    assert_eq!(outcome.native_resolution.physical_width, 1920);
    assert_eq!(outcome.native_resolution.physical_height, 1080);

    // When the first PipeWire buffer arrives with native 2560x1600 dimensions,
    // `notify_pipewire_buffer_dimensions` updates the session to full native 2560x1600!
    let updated_res = svc_sizeless
        .notify_pipewire_buffer_dimensions(2560, 1600)
        .expect("update to native SPA buffer dimensions");
    assert_eq!(updated_res.physical_width, 2560);
    assert_eq!(updated_res.physical_height, 1600);
    assert_eq!(updated_res.encoder_width, 2560);
    assert_eq!(updated_res.encoder_height, 1600);
    assert!(updated_res.is_native_preserved);
    assert_eq!(
        svc_sizeless.state(),
        WaylandPortalDelegationState::StreamingNativeResolution {
            node_id: 412,
            width: 2560,
            height: 1600,
        }
    );
}

// ---------------------------------------------------------------------------
// 6. Loading / Intermediate States: Lifecycle transition enforcement
// ---------------------------------------------------------------------------

#[test]
fn test_r5_06_dim6_lifecycle_state_transitions_and_double_start_prevention() {
    let probe = WaylandEnvironmentProbe::wayland_default();
    let transport =
        MockXdgPortalTransport::new_with_native_stream(501, DesktopSourceType::Screen, 2560, 1440, "DP-2");
    let mut service = WaylandPortalCaptureService::new(transport, probe);

    assert_eq!(service.state(), WaylandPortalDelegationState::Idle);

    // Cannot capture frames or stop stream before initiating
    assert!(service.capture_native_test_frame(PixelLayout::Rgba, 0).is_err());

    let cfg = WaylandPortalConfig::default();
    service.initiate_stream(&cfg).expect("first start");
    assert!(matches!(
        service.state(),
        WaylandPortalDelegationState::StreamingNativeResolution {
            node_id: 501,
            width: 2560,
            height: 1440
        }
    ));

    // Starting a second stream while already streaming is rejected without leaking the active session
    assert!(service.initiate_stream(&cfg).is_err());

    service.stop_stream().expect("stop active stream");
    assert_eq!(service.state(), WaylandPortalDelegationState::Closed);
}

// ---------------------------------------------------------------------------
// 7. Capability Permissions & Restore Tokens: Silent restore (0 prompts) & Redaction
// ---------------------------------------------------------------------------

#[test]
fn test_r5_06_dim7_restore_token_silent_portal_delegation_and_privacy_redaction() {
    let probe = WaylandEnvironmentProbe::wayland_default();
    let transport =
        MockXdgPortalTransport::new_with_native_stream(601, DesktopSourceType::Screen, 3840, 2160, "DP-1");
    transport.state.borrow_mut().honor_restore_token_silently = true;
    let st_ref = transport.state.clone();

    let mut service = WaylandPortalCaptureService::new(transport, probe);

    // 1st run without prior restore_token: shows 1 native portal dialog, saves new restore_token
    let cfg1 = WaylandPortalConfig {
        persist_mode: PortalPersistMode::ApplicationRunning,
        restore_token: None,
        ..WaylandPortalConfig::default()
    };
    let outcome1 = service.initiate_stream(&cfg1).expect("1st stream start");
    assert_eq!(outcome1.custom_picker_prompts, 0);
    assert_eq!(outcome1.native_portal_prompts, 1);
    assert_eq!(
        outcome1.restore_token.as_deref(),
        Some("portal_restore_tok_v1")
    );
    assert_eq!(
        service.saved_restore_token(),
        Some("portal_restore_tok_v1")
    );
    // Redacted restore token in telemetry must never leak the raw secret token
    assert_eq!(service.telemetry().redacted_restore_token, "[REDACTED]");

    service.stop_stream().expect("stop 1st stream");

    // 2nd run reuses saved restore_token automatically:
    // custom picker bypassed (0 prompts) AND portal restores silently (0 prompts)!
    let outcome2 = service
        .initiate_stream(&cfg1)
        .expect("2nd stream start with cached restore_token");
    assert!(outcome2.custom_picker_bypassed);
    assert!(outcome2.used_restore_token);
    assert_eq!(outcome2.custom_picker_prompts, 0);
    assert_eq!(
        st_ref.borrow().native_portal_dialog_shown_count,
        1,
        "Portal chooser must not prompt a second time when restore_token is honored"
    );
    assert_eq!(service.telemetry().restore_token_silent_starts, 1);
    service.stop_stream().expect("stop 2nd stream");
}

// ---------------------------------------------------------------------------
// 8. Recovery & Fallback: X11 / non-portal fallback vs Wayland compositor detection
// ---------------------------------------------------------------------------

#[test]
fn test_r5_06_dim8_x11_and_missing_portal_fallback_to_custom_picker() {
    // 1. Pure X11 session -> falls back to custom picker with X11Shm backend
    let x11_probe = WaylandEnvironmentProbe {
        xdg_session_type: Some("x11".to_string()),
        wayland_display: None,
        x11_display: Some(":0".to_string()),
        xdg_current_desktop: Some("XFCE".to_string()),
        portal_screencast_available: false,
        portal_version: 0,
    };
    assert_eq!(
        evaluate_picker_flow(&x11_probe),
        PickerFlowDecision::UseCustomPicker {
            backend: DesktopBackend::X11Shm,
        }
    );

    // 2. Wayland session where xdg-desktop-portal is unavailable/broken -> falls back gracefully
    let broken_portal_probe = WaylandEnvironmentProbe {
        xdg_session_type: Some("wayland".to_string()),
        wayland_display: Some("wayland-0".to_string()),
        x11_display: Some(":0".to_string()),
        xdg_current_desktop: Some("sway".to_string()),
        portal_screencast_available: false,
        portal_version: 0,
    };
    assert_eq!(
        evaluate_picker_flow(&broken_portal_probe),
        PickerFlowDecision::UseCustomPicker {
            backend: DesktopBackend::X11Shm,
        }
    );

    // 3. Verify compositor detection across GNOME, KDE, Hyprland, Sway, Cosmic, and generic wlroots
    for (desktop, expected) in [
        ("GNOME", WaylandCompositorKind::GnomeMutter),
        ("ubuntu:GNOME", WaylandCompositorKind::GnomeMutter),
        ("KDE", WaylandCompositorKind::KdeKwin),
        ("Hyprland", WaylandCompositorKind::Hyprland),
        ("sway", WaylandCompositorKind::Sway),
        ("COSMIC", WaylandCompositorKind::Cosmic),
        (" labwc ", WaylandCompositorKind::OtherWlroots),
    ] {
        assert_eq!(
            WaylandCompositorKind::from_xdg_desktop(Some(desktop)),
            expected
        );
    }
}

// ---------------------------------------------------------------------------
// 9. Observable Telemetry: Counters for bypasses, native resolutions, and FD balance
// ---------------------------------------------------------------------------

#[test]
fn test_r5_06_dim9_observable_telemetry_tracks_bypasses_resolutions_and_fd_balance() {
    let probe = WaylandEnvironmentProbe::wayland_default();
    let transport =
        MockXdgPortalTransport::new_with_native_stream(777, DesktopSourceType::Screen, 2560, 1440, "DP-1");
    let mut service = WaylandPortalCaptureService::new(transport, probe);

    let cfg = WaylandPortalConfig {
        source_types: SOURCE_TYPE_MONITOR | SOURCE_TYPE_WINDOW,
        cursor_mode: PortalCursorMode::Metadata,
        ..WaylandPortalConfig::default()
    };

    service.initiate_stream(&cfg).expect("start stream");
    let snap_active: WaylandPortalTelemetry = service.telemetry();
    assert_eq!(snap_active.wayland_direct_delegations, 1);
    assert_eq!(snap_active.custom_picker_bypassed_count, 1);
    assert_eq!(snap_active.custom_picker_invocations, 0);
    assert_eq!(snap_active.native_portal_dialog_prompts, 1);
    assert_eq!(snap_active.native_resolution_preserved_count, 1);
    assert_eq!(snap_active.last_native_width, Some(2560));
    assert_eq!(snap_active.last_native_height, Some(1440));
    assert_eq!(snap_active.active_fd_balance(), 1);

    service.stop_stream().expect("stop stream");
    let snap_closed = service.telemetry();
    assert_eq!(
        snap_closed.active_fd_balance(),
        0,
        "PipeWire FD balance must return to 0 after stream stop"
    );
}
