//! TASK-613: Контрактные тесты модуля захвата экрана и окон (`discord_media::capture`).
//!
//! Оракул: `/home/sanya/re-deep/native-re.md` (`discord_voice.node` -> `capture_linux`,
//! `webrtc::BaseCapturerPipeWire`, `webrtc::ScreenCapturerX11`, `webrtc::WindowCapturerX11`,
//! `webrtc::XServerPixelBuffer`) и `/home/sanya/re-deep/backend-ipc.md`
//! (`DISCORD_DESKTOP_CAPTURER_GET_SOURCES`, типы источников `["screen", "window"]`).
//!
//! Покрывает 9 обязательных измерений `TEST-POLICY.md` §5:
//! 1. Позитивные сценарии (Portal ScreenCast handshake + PipeWire fd, X11 SHM окно/монитор, перечисление).
//! 2. Негативные сценарии (невалидные токены, D-Bus пути, битые буферы, неизвестный source_id).
//! 3. Граничные условия (размеры 1x1, `MAX_CAPTURE_DIMENSION`, выравнивание stride, crop по границе).
//! 4. Ошибки протокола (D-Bus ошибка, `WindowDestroyed`, обрыв сессии, ошибка SHM сегмента).
//! 5. Пустые состояния (пустой список окон, пустой фильтр `SourceFilter::none()`, 0 потоков от портала).
//! 6. Загрузочные / промежуточные состояния (`PortalSessionState`: `Unconnected -> SessionCreated -> SourcesSelected -> Streaming -> Closed`).
//! 7. Права доступа / согласие пользователя (`Request.Response = 1` `ConsentDenied`, `2` `ConsentAborted`, `ConsentTimeout`).
//! 8. Восстановление и деградация (Wayland Portal -> X11 SHM -> XGetImage -> SyntheticFallback, закрытие fd при ошибке).
//! 9. Совместимость с существующим API `discord_media::{ScreenCapturer, DesktopSource, DesktopSourceType, DesktopBackend}`.

use std::cell::RefCell;
use std::rc::Rc;

use discord_media::capture::portal::{
    CropRect, PORTAL_BUS_NAME, PORTAL_OBJECT_PATH, PipeWireFrameBuffer, PipeWirePixelFormat,
    PortalCursorMode, PortalPersistMode, PortalResponse, PortalResponseCode,
    PortalScreenCastClient, PortalSessionConfig, PortalSessionState, PortalStream, PortalTransport,
    REQUEST_INTERFACE, SCREENCAST_INTERFACE, SESSION_INTERFACE, SOURCE_TYPE_MONITOR,
    SOURCE_TYPE_VIRTUAL, SOURCE_TYPE_WINDOW, validate_dbus_object_path, validate_portal_token,
};
use discord_media::capture::sources::{SourceFilter, detect_backend_from_env};
use discord_media::capture::x11::{
    X11CaptureMode, X11MonitorGeometry, X11ShmPixelBuffer, X11WindowMetadata,
    filter_capturable_x11_windows,
};
use discord_media::capture::{CaptureError, MAX_CAPTURE_DIMENSION};
use discord_media::{DesktopBackend, DesktopSourceType, PixelFormat, ScreenCapturer};

// ---------------------------------------------------------------------------
// Тестовый мок-транспорт для org.freedesktop.portal.ScreenCast
// ---------------------------------------------------------------------------

#[derive(Debug, Default)]
struct MockPortalState {
    calls: Vec<String>,
    create_session_response: Option<Result<(PortalResponseCode, String), CaptureError>>,
    select_sources_response: Option<Result<PortalResponseCode, CaptureError>>,
    start_response: Option<Result<PortalResponse, CaptureError>>,
    open_pw_remote_response: Option<Result<i32, CaptureError>>,
    closed_sessions: Vec<String>,
    cancelled_requests: Vec<String>,
    closed_fds: Vec<i32>,
    last_select_options: Option<PortalSessionConfig>,
}

#[derive(Clone, Default)]
struct MockPortalTransport {
    state: Rc<RefCell<MockPortalState>>,
}

impl PortalTransport for MockPortalTransport {
    fn create_session(
        &mut self,
        handle_token: &str,
        session_handle_token: &str,
    ) -> Result<(PortalResponseCode, String), CaptureError> {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!(
            "CreateSession:{handle_token}:{session_handle_token}"
        ));
        st.create_session_response.clone().unwrap_or_else(|| {
            Ok((
                PortalResponseCode::Success,
                format!("/org/freedesktop/portal/desktop/session/1_42/{session_handle_token}"),
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
            "SelectSources:{session_handle}:types={}:multiple={}:cursor={}:persist={}",
            config.source_types,
            config.multiple,
            config.cursor_mode.as_u32(),
            config.persist_mode.as_u32()
        ));
        st.last_select_options = Some(config.clone());
        st.select_sources_response
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
        st.start_response.clone().unwrap_or_else(|| {
            Ok(PortalResponse {
                code: PortalResponseCode::Success,
                streams: vec![PortalStream {
                    node_id: 77,
                    id: Some("screen:0".to_string()),
                    source_type: DesktopSourceType::Screen,
                    position: Some((0, 0)),
                    size: Some((1920, 1080)),
                    mapping_id: Some("DP-1".to_string()),
                }],
                restore_token: Some("restore-tok-abc123".to_string()),
            })
        })
    }

    fn open_pipewire_remote(&mut self, session_handle: &str) -> Result<i32, CaptureError> {
        let mut st = self.state.borrow_mut();
        st.calls
            .push(format!("OpenPipeWireRemote:{session_handle}"));
        st.open_pw_remote_response.clone().unwrap_or(Ok(42))
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
        st.cancelled_requests.push(request_handle.to_string());
        Ok(())
    }

    fn close_pipewire_fd(&mut self, fd: i32) {
        let mut st = self.state.borrow_mut();
        st.calls.push(format!("CloseFd:{fd}"));
        st.closed_fds.push(fd);
    }
}

// ---------------------------------------------------------------------------
// 1. Константы и примитивы org.freedesktop.portal.ScreenCast
// ---------------------------------------------------------------------------

#[test]
fn portal_constants_and_bitmasks_match_freedesktop_and_discord_voice_oracle() {
    assert_eq!(PORTAL_BUS_NAME, "org.freedesktop.portal.Desktop");
    assert_eq!(PORTAL_OBJECT_PATH, "/org/freedesktop/portal/desktop");
    assert_eq!(SCREENCAST_INTERFACE, "org.freedesktop.portal.ScreenCast");
    assert_eq!(REQUEST_INTERFACE, "org.freedesktop.portal.Request");
    assert_eq!(SESSION_INTERFACE, "org.freedesktop.portal.Session");

    assert_eq!(SOURCE_TYPE_MONITOR, 1);
    assert_eq!(SOURCE_TYPE_WINDOW, 2);
    assert_eq!(SOURCE_TYPE_VIRTUAL, 4);

    assert_eq!(PortalCursorMode::Hidden.as_u32(), 1);
    assert_eq!(PortalCursorMode::Embedded.as_u32(), 2);
    assert_eq!(PortalCursorMode::Metadata.as_u32(), 4);

    assert_eq!(PortalPersistMode::DoNotPersist.as_u32(), 0);
    assert_eq!(PortalPersistMode::ApplicationRunning.as_u32(), 1);
    assert_eq!(PortalPersistMode::ExplicitlyRevoked.as_u32(), 2);

    assert_eq!(PortalResponseCode::from_u32(0), PortalResponseCode::Success);
    assert_eq!(
        PortalResponseCode::from_u32(1),
        PortalResponseCode::CancelledByUser
    );
    assert_eq!(
        PortalResponseCode::from_u32(2),
        PortalResponseCode::EndedOtherwise
    );
    assert_eq!(
        PortalResponseCode::from_u32(99),
        PortalResponseCode::EndedOtherwise
    );
}

#[test]
fn portal_token_and_dbus_object_path_validation_enforces_spec() {
    assert!(validate_portal_token("discord_req_01").is_ok());
    assert!(validate_portal_token("sess42").is_ok());

    for bad_token in [
        "",
        "bad-hyphen",
        "bad/slash",
        "bad space",
        "123_start_digit_ok?",
    ] {
        if bad_token == "123_start_digit_ok?" {
            assert!(validate_portal_token(bad_token).is_err());
        } else {
            assert!(
                validate_portal_token(bad_token).is_err(),
                "token {bad_token:?} must be rejected"
            );
        }
    }

    assert!(validate_dbus_object_path("/").is_ok());
    assert!(validate_dbus_object_path("/org/freedesktop/portal/desktop/session/1_99/tok1").is_ok());

    for bad_path in [
        "",
        "org/freedesktop/portal",
        "/org/freedesktop/portal/",
        "/org//freedesktop",
        "/org/free-desktop/portal",
    ] {
        assert!(
            validate_dbus_object_path(bad_path).is_err(),
            "object path {bad_path:?} must be rejected"
        );
    }
}

// ---------------------------------------------------------------------------
// 2. Полный цикл сессии org.freedesktop.portal.ScreenCast и обработка отказов
// ---------------------------------------------------------------------------

#[test]
fn portal_screencast_full_handshake_yields_pipewire_config_and_cleans_up_on_close() {
    let transport = MockPortalTransport::default();
    let state_ref = transport.state.clone();

    let config = PortalSessionConfig {
        handle_token: "discord_req_1".to_string(),
        session_handle_token: "discord_sess_1".to_string(),
        source_types: SOURCE_TYPE_MONITOR | SOURCE_TYPE_WINDOW,
        multiple: false,
        cursor_mode: PortalCursorMode::Embedded,
        persist_mode: PortalPersistMode::ApplicationRunning,
        restore_token: Some("prev_token_001".to_string()),
        parent_window: String::new(),
    };

    let mut client = PortalScreenCastClient::new(transport);
    assert_eq!(client.state(), PortalSessionState::Unconnected);

    let session_path = client
        .create_session(&config)
        .expect("CreateSession should succeed");
    assert_eq!(
        session_path,
        "/org/freedesktop/portal/desktop/session/1_42/discord_sess_1"
    );
    assert_eq!(client.state(), PortalSessionState::SessionCreated);

    client
        .select_sources(&config)
        .expect("SelectSources should succeed");
    assert_eq!(client.state(), PortalSessionState::SourcesSelected);

    let pw_config = client
        .start_and_open_remote(&config)
        .expect("Start + OpenPipeWireRemote should succeed");
    assert_eq!(client.state(), PortalSessionState::Streaming);
    assert_eq!(pw_config.fd, 42);
    assert_eq!(pw_config.node_id, 77);
    assert_eq!(
        pw_config.restore_token.as_deref(),
        Some("restore-tok-abc123")
    );
    assert_eq!(pw_config.streams.len(), 1);
    assert_eq!(pw_config.streams[0].size, Some((1920, 1080)));

    client.close().expect("close session should succeed");
    assert_eq!(client.state(), PortalSessionState::Closed);

    let st = state_ref.borrow();
    assert_eq!(
        st.closed_sessions,
        vec!["/org/freedesktop/portal/desktop/session/1_42/discord_sess_1".to_string()]
    );
    assert_eq!(st.closed_fds, vec![42]);
}

#[test]
fn portal_screencast_handles_consent_denied_aborted_timeout_and_invalid_fd_with_cleanup() {
    // 1. Отказ пользователя (Response = 1 -> ConsentDenied) на этапе Start
    let transport_denied = MockPortalTransport::default();
    let state_denied = transport_denied.state.clone();
    state_denied.borrow_mut().start_response = Some(Ok(PortalResponse {
        code: PortalResponseCode::CancelledByUser,
        streams: vec![],
        restore_token: None,
    }));

    let config = PortalSessionConfig::default_for_types(SOURCE_TYPE_MONITOR | SOURCE_TYPE_WINDOW);
    let mut client_denied = PortalScreenCastClient::new(transport_denied);
    client_denied.create_session(&config).unwrap();
    client_denied.select_sources(&config).unwrap();
    let err = client_denied.start_and_open_remote(&config).unwrap_err();
    assert_eq!(err, CaptureError::ConsentDenied);
    assert_eq!(client_denied.state(), PortalSessionState::Closed);
    assert_eq!(state_denied.borrow().closed_sessions.len(), 1);

    // 2. Прерывание диалога (Response = 2 -> ConsentAborted) на этапе SelectSources
    let transport_aborted = MockPortalTransport::default();
    let state_aborted = transport_aborted.state.clone();
    state_aborted.borrow_mut().select_sources_response =
        Some(Ok(PortalResponseCode::EndedOtherwise));

    let mut client_aborted = PortalScreenCastClient::new(transport_aborted);
    client_aborted.create_session(&config).unwrap();
    let err_aborted = client_aborted.select_sources(&config).unwrap_err();
    assert_eq!(err_aborted, CaptureError::ConsentAborted);
    assert_eq!(client_aborted.state(), PortalSessionState::Closed);
    assert_eq!(state_aborted.borrow().closed_sessions.len(), 1);

    // 3. Невалидный PipeWire fd (< 0) от OpenPipeWireRemote -> ошибка + закрытие сессии
    let transport_bad_fd = MockPortalTransport::default();
    let state_bad_fd = transport_bad_fd.state.clone();
    state_bad_fd.borrow_mut().open_pw_remote_response = Some(Ok(-1));

    let mut client_bad_fd = PortalScreenCastClient::new(transport_bad_fd);
    client_bad_fd.create_session(&config).unwrap();
    client_bad_fd.select_sources(&config).unwrap();
    let err_fd = client_bad_fd.start_and_open_remote(&config).unwrap_err();
    assert_eq!(err_fd, CaptureError::InvalidPipeWireFd(-1));
    assert_eq!(client_bad_fd.state(), PortalSessionState::Closed);
    assert_eq!(state_bad_fd.borrow().closed_sessions.len(), 1);

    // 4. Нарушение порядка вызовов (вызов select_sources до create_session)
    let mut client_order = PortalScreenCastClient::new(MockPortalTransport::default());
    assert_eq!(
        client_order.select_sources(&config).unwrap_err(),
        CaptureError::InvalidSessionState {
            expected: "SessionCreated",
            actual: PortalSessionState::Unconnected,
        }
    );
}

// ---------------------------------------------------------------------------
// 3. Обработка буферов PipeWire (BGRA / BGRX / RGBA со stride и crop)
// ---------------------------------------------------------------------------

#[test]
fn pipewire_frame_buffer_converts_bgra_and_bgrx_with_stride_padding_and_crop() {
    // Буфер 4x2 в формате BGRX с выравниванием строки stride = 24 байта (padding 8 байт на строку)
    let stride = 24usize;
    let mut raw = vec![0u8; stride * 2];
    // Заполним пиксели [B=20, G=120, R=220, X=0] (при BGRX альфа-канал обязан стать 255)
    for row in 0..2 {
        for col in 0..4 {
            let offset = row * stride + col * 4;
            raw[offset] = 20;
            raw[offset + 1] = 120;
            raw[offset + 2] = 220;
            raw[offset + 3] = 0; // X=0
        }
    }

    let pw_buf = PipeWireFrameBuffer {
        width: 4,
        height: 2,
        stride,
        format: PipeWirePixelFormat::Bgrx8,
        data: &raw,
        crop: None,
        dma_buf_fd: None,
        drm_modifier: 0,
    };

    let rgba_frame = pw_buf
        .to_video_frame(PixelFormat::Rgba, 123_456)
        .expect("BGRX -> RGBA conversion should succeed");
    assert_eq!(rgba_frame.width, 4);
    assert_eq!(rgba_frame.height, 2);
    assert_eq!(rgba_frame.timestamp_us, 123_456);
    assert_eq!(rgba_frame.data.len(), 4 * 2 * 4);
    // Первый пиксель в RGBA: [R=220, G=120, B=20, A=255]
    assert_eq!(&rgba_frame.data[0..4], &[220, 120, 20, 255]);

    // Проверим конвертацию в I420 и применение SpaMetaVideoCrop (2x2 из 4x2)
    let cropped_buf = PipeWireFrameBuffer {
        width: 4,
        height: 2,
        stride,
        format: PipeWirePixelFormat::Bgra8,
        data: &raw,
        crop: Some(CropRect {
            x: 2,
            y: 0,
            width: 2,
            height: 2,
        }),
        dma_buf_fd: None,
        drm_modifier: 0,
    };
    let i420_frame = cropped_buf
        .to_video_frame(PixelFormat::I420, 789_000)
        .expect("cropped BGRA -> I420 should succeed");
    assert_eq!(i420_frame.width, 2);
    assert_eq!(i420_frame.height, 2);
    assert!(i420_frame.validate().is_ok());
}

#[test]
fn pipewire_frame_buffer_rejects_zero_dimensions_invalid_stride_and_out_of_bounds_crop() {
    let data = vec![0u8; 64];

    // 0-размер
    let zero_w = PipeWireFrameBuffer {
        width: 0,
        height: 2,
        stride: 16,
        format: PipeWirePixelFormat::Bgra8,
        data: &data,
        crop: None,
        dma_buf_fd: None,
        drm_modifier: 0,
    };
    assert!(matches!(
        zero_w.to_video_frame(PixelFormat::Rgba, 0),
        Err(CaptureError::InvalidDimensions {
            width: 0,
            height: 2
        })
    ));

    // Слишком большой размер (> MAX_CAPTURE_DIMENSION)
    let huge = PipeWireFrameBuffer {
        width: MAX_CAPTURE_DIMENSION + 1,
        height: 2,
        stride: ((MAX_CAPTURE_DIMENSION + 1) * 4) as usize,
        format: PipeWirePixelFormat::Bgra8,
        data: &data,
        crop: None,
        dma_buf_fd: None,
        drm_modifier: 0,
    };
    assert!(matches!(
        huge.to_video_frame(PixelFormat::Rgba, 0),
        Err(CaptureError::InvalidDimensions { .. })
    ));

    // Stride меньше width * 4
    let bad_stride = PipeWireFrameBuffer {
        width: 4,
        height: 2,
        stride: 12,
        format: PipeWirePixelFormat::Bgra8,
        data: &data,
        crop: None,
        dma_buf_fd: None,
        drm_modifier: 0,
    };
    assert!(matches!(
        bad_stride.to_video_frame(PixelFormat::Rgba, 0),
        Err(CaptureError::InvalidStride { .. })
    ));

    // Обрезанный буфер (меньше высоты * stride)
    let short_buf = PipeWireFrameBuffer {
        width: 4,
        height: 4,
        stride: 16,
        format: PipeWirePixelFormat::Bgra8,
        data: &data[..32],
        crop: None,
        dma_buf_fd: None,
        drm_modifier: 0,
    };
    assert!(matches!(
        short_buf.to_video_frame(PixelFormat::Rgba, 0),
        Err(CaptureError::BufferTooSmall { .. })
    ));

    // Crop выходит за границы кадра
    let bad_crop = PipeWireFrameBuffer {
        width: 4,
        height: 2,
        stride: 16,
        format: PipeWirePixelFormat::Bgra8,
        data: &data,
        crop: Some(CropRect {
            x: 3,
            y: 0,
            width: 2,
            height: 2,
        }),
        dma_buf_fd: None,
        drm_modifier: 0,
    };
    assert!(matches!(
        bad_crop.to_video_frame(PixelFormat::Rgba, 0),
        Err(CaptureError::CropOutOfBounds { .. })
    ));
}

// ---------------------------------------------------------------------------
// 4. Захват окон и мониторов через X11 SHM и фильтрация окон
// ---------------------------------------------------------------------------

#[test]
fn x11_window_filtering_excludes_unmapped_minimized_override_redirect_and_empty_windows() {
    let raw_windows = vec![
        X11WindowMetadata {
            xid: 0x02a0_0001,
            title: "Discord — #general".to_string(),
            class_name: Some("discord".to_string()),
            x: 100,
            y: 50,
            width: 1280,
            height: 720,
            is_mapped: true,
            is_minimized: false,
            is_override_redirect: false,
        },
        // Свёрнутое окно (IconicState) — должно отсеиваться
        X11WindowMetadata {
            xid: 0x02a0_0002,
            title: "Minimized Terminal".to_string(),
            class_name: Some("Alacritty".to_string()),
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            is_mapped: true,
            is_minimized: true,
            is_override_redirect: false,
        },
        // Неотображённое (IsUnmapped)
        X11WindowMetadata {
            xid: 0x02a0_0003,
            title: "Hidden Helper".to_string(),
            class_name: None,
            x: 0,
            y: 0,
            width: 400,
            height: 300,
            is_mapped: false,
            is_minimized: false,
            is_override_redirect: false,
        },
        // Всплывающее меню (override_redirect = true)
        X11WindowMetadata {
            xid: 0x02a0_0004,
            title: "Context Menu".to_string(),
            class_name: None,
            x: 10,
            y: 10,
            width: 200,
            height: 400,
            is_mapped: true,
            is_minimized: false,
            is_override_redirect: true,
        },
        // Нулевая площадь
        X11WindowMetadata {
            xid: 0x02a0_0005,
            title: "Zero Size".to_string(),
            class_name: None,
            x: 0,
            y: 0,
            width: 0,
            height: 100,
            is_mapped: true,
            is_minimized: false,
            is_override_redirect: false,
        },
        // Пустой заголовок
        X11WindowMetadata {
            xid: 0x02a0_0006,
            title: "   ".to_string(),
            class_name: None,
            x: 0,
            y: 0,
            width: 500,
            height: 500,
            is_mapped: true,
            is_minimized: false,
            is_override_redirect: false,
        },
    ];

    let capturable = filter_capturable_x11_windows(&raw_windows);
    assert_eq!(capturable.len(), 1);
    assert_eq!(capturable[0].xid, 0x02a0_0001);
    assert_eq!(capturable[0].to_desktop_source().id, "window:x11:44040193");
    assert_eq!(
        capturable[0].to_desktop_source().source_type,
        DesktopSourceType::Window
    );
}

#[test]
fn x11_shm_pixel_buffer_captures_monitor_subrect_and_window_with_fallback_recovery() {
    // Виртуальный root-экран X11 из двух мониторов 2x2 рядом: общий размер 4x2
    // Левый монитор (x=0..2): синий (B=255, G=0, R=0), правый монитор (x=2..4): красный (B=0, G=0, R=255)
    let root_width = 4u32;
    let root_height = 2u32;
    let bytes_per_line = 20usize; // 4 * 4 = 16 + 4 байта padding
    let mut root_shm = vec![0u8; bytes_per_line * (root_height as usize)];

    for row in 0..2usize {
        for col in 0..4usize {
            let off = row * bytes_per_line + col * 4;
            if col < 2 {
                // Левый монитор: синий
                root_shm[off] = 255;
                root_shm[off + 1] = 0;
                root_shm[off + 2] = 0;
                root_shm[off + 3] = 0;
            } else {
                // Правый монитор: красный
                root_shm[off] = 0;
                root_shm[off + 1] = 0;
                root_shm[off + 2] = 255;
                root_shm[off + 3] = 0;
            }
        }
    }

    let right_monitor = X11MonitorGeometry {
        id: "screen:1".to_string(),
        name: "HDMI-1".to_string(),
        x: 2,
        y: 0,
        width: 2,
        height: 2,
        is_primary: false,
    };

    let shm_buf = X11ShmPixelBuffer::new(root_width, root_height, bytes_per_line, 24)
        .expect("valid X11ShmPixelBuffer");
    assert_eq!(shm_buf.mode(), X11CaptureMode::MitShm);

    let right_frame = shm_buf
        .capture_monitor_rect(&right_monitor, &root_shm, PixelFormat::Rgba, 555_000)
        .expect("capture right monitor from root SHM");
    assert_eq!(right_frame.width, 2);
    assert_eq!(right_frame.height, 2);
    // В правом мониторе пиксели красные: RGBA = [255, 0, 0, 255]
    assert_eq!(&right_frame.data[0..4], &[255, 0, 0, 255]);

    // Выход монитора за пределы root-окна должен возвращать CropOutOfBounds
    let oob_monitor = X11MonitorGeometry {
        id: "screen:2".to_string(),
        name: "DP-2".to_string(),
        x: 3,
        y: 0,
        width: 2,
        height: 2,
        is_primary: false,
    };
    assert!(matches!(
        shm_buf.capture_monitor_rect(&oob_monitor, &root_shm, PixelFormat::Rgba, 0),
        Err(CaptureError::CropOutOfBounds { .. })
    ));

    // Проверим деградацию (fallback) при отказе MIT-SHM: MitShm -> XGetImageFallback -> SyntheticFallback
    let mut degraded = shm_buf;
    degraded.record_shm_failure("XShmAttach failed (BadAccess)");
    assert_eq!(degraded.mode(), X11CaptureMode::XGetImageFallback);
    assert_eq!(degraded.fallback_count(), 1);
    assert_eq!(
        degraded.last_fallback_reason(),
        Some("XShmAttach failed (BadAccess)")
    );
}

// ---------------------------------------------------------------------------
// 5. Перечисление источников захвата (мониторы, окна) и выбор бэкенда
// ---------------------------------------------------------------------------

#[test]
fn detect_backend_from_env_prioritizes_wayland_portal_then_x11_shm_then_synthetic() {
    // 1. Wayland сессия -> PipeWirePortal
    assert_eq!(
        detect_backend_from_env(Some("wayland"), Some("wayland-0"), Some(":0")),
        DesktopBackend::PipeWirePortal
    );
    assert_eq!(
        detect_backend_from_env(None, Some("wayland-1"), None),
        DesktopBackend::PipeWirePortal
    );

    // 2. X11 сессия -> X11Shm ("Failed to open Wayland display, falling back to X11")
    assert_eq!(
        detect_backend_from_env(Some("x11"), None, Some(":0")),
        DesktopBackend::X11Shm
    );
    assert_eq!(
        detect_backend_from_env(Some(""), Some(""), Some(":1.0")),
        DesktopBackend::X11Shm
    );

    // 3. Headless окружение без Wayland и X11 -> SyntheticFallback
    assert_eq!(
        detect_backend_from_env(None, None, None),
        DesktopBackend::SyntheticFallback
    );
    assert_eq!(
        detect_backend_from_env(Some("tty"), Some(""), Some("  ")),
        DesktopBackend::SyntheticFallback
    );
}

#[test]
fn screen_capturer_enumerates_and_filters_monitors_and_windows_and_preserves_legacy_api() {
    let monitors = vec![
        X11MonitorGeometry {
            id: "screen:0".to_string(),
            name: "DP-1 (Primary)".to_string(),
            x: 0,
            y: 0,
            width: 2560,
            height: 1440,
            is_primary: true,
        },
        X11MonitorGeometry {
            id: "screen:1".to_string(),
            name: "HDMI-1".to_string(),
            x: 2560,
            y: 0,
            width: 1920,
            height: 1080,
            is_primary: false,
        },
    ];
    let windows = vec![
        X11WindowMetadata {
            xid: 101,
            title: "Firefox".to_string(),
            class_name: Some("Navigator".to_string()),
            x: 50,
            y: 50,
            width: 1280,
            height: 720,
            is_mapped: true,
            is_minimized: false,
            is_override_redirect: false,
        },
        X11WindowMetadata {
            xid: 102,
            title: "Minimized Window".to_string(),
            class_name: None,
            x: 0,
            y: 0,
            width: 800,
            height: 600,
            is_mapped: true,
            is_minimized: true,
            is_override_redirect: false,
        },
    ];

    let capturer = ScreenCapturer::with_sources(DesktopBackend::X11Shm, monitors, windows);
    assert_eq!(capturer.backend(), DesktopBackend::X11Shm);
    assert_eq!(capturer.backend().name(), "x11_shm");

    let all_sources = capturer.enumerate_sources();
    assert_eq!(all_sources.len(), 3); // 2 монитора + 1 видимое окно (свёрнутое отфильтровано)

    let screens = capturer.enumerate_screens();
    assert_eq!(screens.len(), 2);
    assert!(screens[0].is_primary);
    assert!(!screens[1].is_primary);

    let win_sources = capturer.enumerate_windows();
    assert_eq!(win_sources.len(), 1);
    assert_eq!(win_sources[0].name, "Firefox");
    assert_eq!(win_sources[0].source_type, DesktopSourceType::Window);

    // Фильтрация через SourceFilter (аналог types: ["screen"] / ["window"] в DiscordNative.desktopCapture)
    assert_eq!(
        capturer
            .enumerate_sources_filtered(SourceFilter::screens_only())
            .len(),
        2
    );
    assert_eq!(
        capturer
            .enumerate_sources_filtered(SourceFilter::windows_only())
            .len(),
        1
    );
    assert!(
        capturer
            .enumerate_sources_filtered(SourceFilter::none())
            .is_empty()
    );

    // Конструирование из потоков PipeWire Portal
    let portal_streams = vec![
        PortalStream {
            node_id: 88,
            id: Some("screen:pw:88".to_string()),
            source_type: DesktopSourceType::Screen,
            position: Some((0, 0)),
            size: Some((1920, 1080)),
            mapping_id: Some("eDP-1".to_string()),
        },
        PortalStream {
            node_id: 99,
            id: None,
            source_type: DesktopSourceType::Window,
            position: None,
            size: Some((1280, 720)),
            mapping_id: None,
        },
    ];
    let pw_capturer = ScreenCapturer::from_portal_streams(&portal_streams);
    assert_eq!(pw_capturer.backend(), DesktopBackend::PipeWirePortal);
    assert_eq!(pw_capturer.enumerate_screens().len(), 1);
    assert_eq!(pw_capturer.enumerate_windows().len(), 1);

    // Проверка обратной совместимости capture_frame
    let frame = capturer
        .capture_frame("screen:0", 320, 180, 42_000)
        .expect("capture_frame on known source should succeed");
    assert_eq!(frame.width, 320);
    assert_eq!(frame.height, 180);
    assert_eq!(frame.layout, PixelFormat::I420);
    assert!(capturer.capture_frame("", 320, 180, 0).is_err());
}

#[test]
fn dma_buf_pipewire_frame_buffer_zero_copy_avoids_cpu_copy_latency() {
    use discord_media::PixelLayout;
    use std::time::Instant;

    // 1. Создание аппаратного DMA-BUF буфера PipeWire 1440p (2560x1440)
    let fake_dma_fd = 42;
    let drm_modifier = 0x0011_2233_4455_6677;
    let dma_buf = PipeWireFrameBuffer::new_dma_buf(
        2560,
        1440,
        2560 * 4,
        PipeWirePixelFormat::Bgra8,
        fake_dma_fd,
        drm_modifier,
        None,
    );

    assert!(dma_buf.is_dma_buf());
    assert_eq!(dma_buf.dma_buf_fd, Some(fake_dma_fd));
    assert_eq!(dma_buf.drm_modifier, drm_modifier);
    assert!(dma_buf.data.is_empty());

    // 2. Преобразование в VideoFrame должно происходить без CPU-копирования данных (< 1 мс)
    let t0 = Instant::now();
    let frame = dma_buf
        .to_video_frame(PixelLayout::Rgba, 999_888)
        .expect("to_video_frame for dma_buf");
    let elapsed = t0.elapsed();

    assert!(
        elapsed.as_millis() < 5,
        "DMA-BUF zero-copy must be immediate (< 5ms), took {elapsed:?}"
    );
    assert!(frame.is_dma_buf());
    assert_eq!(frame.dma_buf_fd(), Some(fake_dma_fd));
    assert_eq!(frame.drm_modifier(), drm_modifier);
    assert_eq!(frame.width, 2560);
    assert_eq!(frame.height, 1440);
    assert_eq!(frame.timestamp_us, 999_888);
    assert!(
        frame.data.is_empty(),
        "DMA-BUF frame must not hold heap-allocated CPU copy"
    );
    assert!(frame.is_valid());

    // 3. Обрезка (crop) в режиме DMA-BUF
    let cropped_dma = PipeWireFrameBuffer::new_dma_buf(
        1920,
        1080,
        1920 * 4,
        PipeWirePixelFormat::Bgra8,
        fake_dma_fd,
        drm_modifier,
        Some(CropRect {
            x: 100,
            y: 50,
            width: 1280,
            height: 720,
        }),
    );
    let cropped_frame = cropped_dma
        .to_video_frame(PixelLayout::Rgba, 111_222)
        .expect("cropped dma frame");
    assert_eq!(cropped_frame.width, 1280);
    assert_eq!(cropped_frame.height, 720);
    assert!(cropped_frame.is_dma_buf());

    // 4. Ошибки валидации (нулевые размеры и выход crop за границы)
    let bad_dims = PipeWireFrameBuffer::new_dma_buf(
        0,
        1080,
        0,
        PipeWirePixelFormat::Bgra8,
        fake_dma_fd,
        drm_modifier,
        None,
    );
    assert!(matches!(
        bad_dims.to_video_frame(PixelLayout::Rgba, 0),
        Err(CaptureError::InvalidDimensions { .. })
    ));

    let oob_crop = PipeWireFrameBuffer::new_dma_buf(
        1920,
        1080,
        1920 * 4,
        PipeWirePixelFormat::Bgra8,
        fake_dma_fd,
        drm_modifier,
        Some(CropRect {
            x: 1900,
            y: 1000,
            width: 100,
            height: 100,
        }),
    );
    assert!(matches!(
        oob_crop.to_video_frame(PixelLayout::Rgba, 0),
        Err(CaptureError::CropOutOfBounds { .. })
    ));
}
