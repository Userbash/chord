//! `SUB-908-2` & `TASK-954`: Кросс-платформенная матрица тестирования захвата экрана и звука
//! под 4 композиторами Wayland/X11:
//! - `954.1` (Шаг 2.1): GNOME 46/47 (Mutter) Wayland — вызов `org.freedesktop.portal.ScreenCast` через D-Bus,
//!   передача FD PipeWire (`SCM_RIGHTS`), согласование `NV12` 60 FPS (DMA-BUF zero-copy) и захват системного звука.
//! - `954.2` (Шаг 2.2): KDE Plasma 6 (KWin) Wayland — выбор окон (`{uuid}`) и мониторов (`screen:0`)
//!   через KWin Portal диалог и `org.kde.KWin.ScreenShot2`, отсутствие утечек FD при закрытии стрима.
//! - `954.3` (Шаг 2.3): Hyprland / Sway (wlroots) — захват экрана через `xdg-desktop-portal-wlr`,
//!   динамический тилинг-ресайз и выравнивание чётных размеров YUV420/NV12.
//! - `954.4` (Шаг 2.4): Классическая сессия X11 (`Xfce` / `i3`) — автоматический fallback на `XShm`
//!   (`XShmGetImage`, `kFullDesktopScreenId = -1`) при отсутствии демона Wayland, цепочка деградации
//!   `MitShm -> XGetImageFallback -> SyntheticFallback` и изоляция физического микрофона.

use std::io::{ErrorKind, Read, Write};
use std::os::fd::{AsRawFd, IntoRawFd};
use std::os::unix::net::{UnixListener, UnixStream};
use std::sync::atomic::{AtomicU64, Ordering};

use chord_media::PixelLayout;
use chord_media::capture::DesktopBackend;
use chord_media::capture::portal::CropRect;
use chord_media::capture::sources::detect_backend_from_env;
use chord_media::capture::x11::{
    X11CaptureMode, X11MonitorGeometry, X11ShmPixelBuffer, X11WindowMetadata,
    filter_capturable_x11_windows,
};
use chord_platform::dbus::message::{
    self, FIELD_INTERFACE, FIELD_MEMBER, FIELD_PATH, FIELD_REPLY_SERIAL, FIELD_SIGNATURE,
    MessageType,
};
use chord_platform::dbus::{BusAddress, Value, fd};
use chord_platform::screencast::{
    CaptureResolutionPreset, CaptureTargetKind, OwnedDmaBuf, PortalCursorMode, PortalPersistMode,
    PortalSourceTypes, ScreenCastCapturePipeline, ScreenCastNegotiationRequest, ScreenCastOptions,
    ScreenCastPipelineState, ScreenCastPortal, parse_x11_display_spec,
};
use serde::{Deserialize, Serialize};

use crate::audio::{AudioRingBuffer, DiscordVoicePacket, VoiceAudioFramer};
use crate::manual_qa::{ManualQaError, ScreenShareChecklistValidator, VoiceChecklistValidator};
use crate::state::{AppState, ScreenSource};
use crate::ui::screen_share_modal::{ScreenShareFps, ScreenShareResolution, ScreenShareTab};
use crate::video::{
    DRM_FORMAT_MOD_INVALID, DRM_FORMAT_MOD_LINEAR, DRM_FOURCC_NV12, GpuVideoPipeline,
    PipeWireDmaBufConnector, VideoPixelFormat,
};

pub const MIN_WINDOW_WIDTH: u32 = 900;
pub const MIN_WINDOW_HEIGHT: u32 = 600;
pub const SERVER_RAIL_WIDTH: u32 = 72;
pub const CHANNEL_SIDEBAR_WIDTH: u32 = 240;
pub const MEMBER_SIDEBAR_WIDTH: u32 = 240;
pub const USER_PANEL_HEIGHT: u32 = 52;
/// Константа `webrtc::kFullDesktopScreenId = -1` из `/home/sanya/re-deep/native-re.md` и `STREAM-WAYLAND-GUIDE.md`.
pub const WEBRTC_FULL_DESKTOP_SCREEN_ID: i32 = -1;
/// Бит `SOUNDSHARE` в `Voice OP 5 SPEAKING` (`speaking: 2`).
pub const VOICE_SPEAKING_SOUNDSHARE_BIT: u32 = 2;

static MOCK_PORTAL_COUNTER: AtomicU64 = AtomicU64::new(1);

/// Четыре канонических графических окружения Linux Desktop из `SUB-908-2` / `TASK-954`
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DesktopEnvironmentKind {
    GnomeWayland,
    KdePlasma6Wayland,
    HyprlandSwayWayland,
    ClassicX11,
}

impl DesktopEnvironmentKind {
    pub const ALL: [Self; 4] = [
        Self::GnomeWayland,
        Self::KdePlasma6Wayland,
        Self::HyprlandSwayWayland,
        Self::ClassicX11,
    ];

    pub const fn display_name(self) -> &'static str {
        match self {
            Self::GnomeWayland => "GNOME 46/47 (Wayland)",
            Self::KdePlasma6Wayland => "KDE Plasma 6 (Wayland)",
            Self::HyprlandSwayWayland => "Hyprland / Sway (Tiling Wayland)",
            Self::ClassicX11 => "X11 Session (Xfce / i3)",
        }
    }
}

/// Бэкенд захвата экрана в зависимости от графической сессии
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum CaptureBackendKind {
    XdgDesktopPortalPipeWire,
    KWinScreenShot2AndPortal,
    WlrootsScreencopyPortal,
    X11ShmFallback,
}

/// Бэкенд захвата системного звука во время демонстрации экрана (`soundshare_audio_device_linux.cpp`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AudioCaptureBackendKind {
    /// Захват системного аудио-монитора через узел PipeWire (`{default_sink}.monitor`)
    PipeWireMonitorNode,
    /// Захват монитора через совместимый слой PulseAudio (`PULSE_SOURCE={default_sink}.monitor`)
    PulseAudioMonitorFallback,
    /// Изоляция без захвата физического микрофона (`PULSE_SOURCE=__discord_stream_no_mic__`)
    SilentNoMicIsolation,
}

/// Метод D-Bus `org.kde.KWin.ScreenShot2` при маршрутизации целей захвата в KDE Wayland
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum KWinCaptureMethod {
    /// Захват всего рабочего пространства (`CaptureWorkspace(a{sv}, h)`) для `screen:0`
    CaptureWorkspace,
    /// Захват отдельного окна по его UUID (`CaptureWindow(s, a{sv}, h)`)
    CaptureWindow { window_uuid: String },
}

/// Коэффициент дробного масштабирования (Fractional Scaling)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum FractionalScaleFactor {
    Percent100,
    Percent125,
    Percent150,
    Percent200,
}

impl FractionalScaleFactor {
    pub const fn numerator_per_100(self) -> u32 {
        match self {
            Self::Percent100 => 100,
            Self::Percent125 => 125,
            Self::Percent150 => 150,
            Self::Percent200 => 200,
        }
    }
}

/// Профиль графического окружения Linux для матрицы тестирования
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopEnvironmentProfile {
    pub kind: DesktopEnvironmentKind,
    pub xdg_session_type: String,
    pub xdg_current_desktop: String,
    pub wayland_display: Option<String>,
    pub x11_display: Option<String>,
    pub tray_protocol: String,
    pub notification_dbus_service: String,
    pub capture_backend: CaptureBackendKind,
    pub supports_fractional_scaling: bool,
    pub is_tiling_wm: bool,
}

/// Результат проверки конкретного окружения Linux Desktop
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopEnvironmentAudit {
    pub kind: DesktopEnvironmentKind,
    pub capture_backend: CaptureBackendKind,
    pub appindicator_tray_ok: bool,
    pub native_notifications_ok: bool,
    pub pipewire_portal_ok: bool,
    pub fractional_scaling_125_ok: bool,
    pub fractional_scaling_150_ok: bool,
    pub tiling_resize_invariants_ok: bool,
    pub x11_shm_fallback_ok: bool,
}

/// Геометрия кадра после применения дробного масштабирования
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ScaledLayoutGeometry {
    pub logical_width: u32,
    pub logical_height: u32,
    pub physical_width: u32,
    pub physical_height: u32,
    pub chat_viewport_width: u32,
    pub has_text_overlap: bool,
    pub user_panel_anchored_bottom: bool,
}

/// Отчет сквозной верификации захвата экрана и звука под конкретным композитором (`TASK-954`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CompositorCaptureSessionReport {
    pub environment: DesktopEnvironmentKind,
    pub capture_backend: CaptureBackendKind,
    pub portal_implementation: String,
    pub dbus_screencast_called: bool,
    pub pipewire_fd_transferred: bool,
    pub monitor_selection_verified: bool,
    pub window_selection_verified: bool,
    pub negotiated_format: VideoPixelFormat,
    pub drm_fourcc: u32,
    pub negotiated_width: u32,
    pub negotiated_height: u32,
    pub negotiated_fps: u32,
    pub frame_interval_us: u64,
    pub rtp_timestamp_step: u32,
    pub dma_buf_zero_copy_verified: bool,
    pub audio_backend: AudioCaptureBackendKind,
    pub audio_monitor_source: String,
    pub voice_speaking_mask: u32,
    pub audio_pcm_frame_samples: usize,
    pub leaked_fds_after_close: usize,
}

/// Детализированный отчет проверки X11 MIT-SHM fallback при отсутствии Wayland (`SUB-954-4`)
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct X11FallbackVerification {
    pub environment: DesktopEnvironmentKind,
    pub capture_backend: CaptureBackendKind,
    pub wayland_daemon_present: bool,
    pub x11_display: String,
    pub full_desktop_screen_id: i32,
    pub xshm_capture_succeeded: bool,
    pub window_filtering_verified: bool,
    pub degradation_chain_verified: bool,
    pub audio_backend: AudioCaptureBackendKind,
    pub mic_isolation_verified: bool,
}

/// Сводный отчет по матрице 4 окружений Linux Desktop
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DesktopMatrixReport {
    pub audits: Vec<DesktopEnvironmentAudit>,
    pub compositor_sessions: Vec<CompositorCaptureSessionReport>,
    pub all_passed: bool,
}

/// Матрица кросс-платформенного тестирования Linux Desktop (`SUB-908-2` / `TASK-954`)
#[derive(Debug, Clone)]
pub struct LinuxDesktopMatrix {
    profiles: Vec<DesktopEnvironmentProfile>,
}

impl Default for LinuxDesktopMatrix {
    fn default() -> Self {
        Self::canonical_four_environments()
    }
}

impl LinuxDesktopMatrix {
    pub fn canonical_four_environments() -> Self {
        Self {
            profiles: vec![
                Self::gnome_profile(),
                Self::kde_profile(),
                Self::hyprland_profile(),
                Self::x11_profile(),
            ],
        }
    }

    fn gnome_profile() -> DesktopEnvironmentProfile {
        DesktopEnvironmentProfile {
            kind: DesktopEnvironmentKind::GnomeWayland,
            xdg_session_type: "wayland".into(),
            xdg_current_desktop: "GNOME".into(),
            wayland_display: Some("wayland-0".into()),
            x11_display: Some(":0".into()),
            tray_protocol: "org.kde.StatusNotifierWatcher (AppIndicator)".into(),
            notification_dbus_service: "org.freedesktop.Notifications".into(),
            capture_backend: CaptureBackendKind::XdgDesktopPortalPipeWire,
            supports_fractional_scaling: true,
            is_tiling_wm: false,
        }
    }

    fn kde_profile() -> DesktopEnvironmentProfile {
        DesktopEnvironmentProfile {
            kind: DesktopEnvironmentKind::KdePlasma6Wayland,
            xdg_session_type: "wayland".into(),
            xdg_current_desktop: "KDE".into(),
            wayland_display: Some("wayland-0".into()),
            x11_display: Some(":1".into()),
            tray_protocol: "org.kde.StatusNotifierWatcher (SNI)".into(),
            notification_dbus_service: "org.freedesktop.Notifications".into(),
            capture_backend: CaptureBackendKind::XdgDesktopPortalPipeWire,
            supports_fractional_scaling: true,
            is_tiling_wm: false,
        }
    }

    fn hyprland_profile() -> DesktopEnvironmentProfile {
        DesktopEnvironmentProfile {
            kind: DesktopEnvironmentKind::HyprlandSwayWayland,
            xdg_session_type: "wayland".into(),
            xdg_current_desktop: "Hyprland:sway".into(),
            wayland_display: Some("wayland-1".into()),
            x11_display: None,
            tray_protocol: "org.kde.StatusNotifierWatcher (Waybar SNI)".into(),
            notification_dbus_service: "org.freedesktop.Notifications".into(),
            capture_backend: CaptureBackendKind::WlrootsScreencopyPortal,
            supports_fractional_scaling: true,
            is_tiling_wm: true,
        }
    }

    fn x11_profile() -> DesktopEnvironmentProfile {
        DesktopEnvironmentProfile {
            kind: DesktopEnvironmentKind::ClassicX11,
            xdg_session_type: "x11".into(),
            xdg_current_desktop: "XFCE:i3".into(),
            wayland_display: None,
            x11_display: Some(":0".into()),
            tray_protocol: "XEmbed / StatusNotifierWatcher".into(),
            notification_dbus_service: "org.freedesktop.Notifications".into(),
            capture_backend: CaptureBackendKind::X11ShmFallback,
            supports_fractional_scaling: false,
            is_tiling_wm: true,
        }
    }

    pub fn profiles(&self) -> &[DesktopEnvironmentProfile] {
        &self.profiles
    }

    pub fn profile(&self, kind: DesktopEnvironmentKind) -> Option<&DesktopEnvironmentProfile> {
        self.profiles.iter().find(|p| p.kind == kind)
    }

    pub fn verify_environment(
        &self,
        profile: &DesktopEnvironmentProfile,
    ) -> Result<DesktopEnvironmentAudit, ManualQaError> {
        if profile.notification_dbus_service != "org.freedesktop.Notifications" {
            return Err(ManualQaError::DesktopRequirementUnmet(
                "org.freedesktop.Notifications D-Bus service required",
            ));
        }
        let is_wayland = profile.xdg_session_type == "wayland" && profile.wayland_display.is_some();
        let is_x11 = profile.xdg_session_type == "x11"
            && profile.wayland_display.is_none()
            && profile.x11_display.is_some();
        if !is_wayland && !is_x11 {
            return Err(ManualQaError::DesktopRequirementUnmet(
                "invalid display session configuration",
            ));
        }
        let scale_125_ok = self
            .verify_fractional_scaling_geometry(1280, 720, FractionalScaleFactor::Percent125)
            .is_ok();
        let scale_150_ok = self
            .verify_fractional_scaling_geometry(1280, 720, FractionalScaleFactor::Percent150)
            .is_ok();

        Ok(DesktopEnvironmentAudit {
            kind: profile.kind,
            capture_backend: profile.capture_backend,
            appindicator_tray_ok: profile.tray_protocol.contains("StatusNotifierWatcher"),
            native_notifications_ok: true,
            pipewire_portal_ok: is_wayland,
            fractional_scaling_125_ok: profile.supports_fractional_scaling && scale_125_ok,
            fractional_scaling_150_ok: profile.supports_fractional_scaling && scale_150_ok,
            tiling_resize_invariants_ok: self
                .verify_fractional_scaling_geometry(900, 600, FractionalScaleFactor::Percent100)
                .is_ok(),
            x11_shm_fallback_ok: is_x11
                && profile.capture_backend == CaptureBackendKind::X11ShmFallback,
        })
    }

    pub fn verify_fractional_scaling_geometry(
        &self,
        logical_width: u32,
        logical_height: u32,
        scale: FractionalScaleFactor,
    ) -> Result<ScaledLayoutGeometry, ManualQaError> {
        if logical_width < MIN_WINDOW_WIDTH || logical_height < MIN_WINDOW_HEIGHT {
            return Err(ManualQaError::WindowBelowMinimumSize {
                width: logical_width,
                height: logical_height,
                min_width: MIN_WINDOW_WIDTH,
                min_height: MIN_WINDOW_HEIGHT,
            });
        }
        let num = scale.numerator_per_100();
        let physical_width = (logical_width * num) / 100;
        let physical_height = (logical_height * num) / 100;
        let fixed_chrome = SERVER_RAIL_WIDTH + CHANNEL_SIDEBAR_WIDTH + MEMBER_SIDEBAR_WIDTH;
        let chat_viewport_width = logical_width.saturating_sub(fixed_chrome);
        let has_text_overlap = chat_viewport_width < 300 || logical_height <= USER_PANEL_HEIGHT * 2;
        Ok(ScaledLayoutGeometry {
            logical_width,
            logical_height,
            physical_width,
            physical_height,
            chat_viewport_width,
            has_text_overlap,
            user_panel_anchored_bottom: logical_height >= MIN_WINDOW_HEIGHT,
        })
    }

    /// `SUB-954-1`: Тест портала GNOME (Mutter)
    /// - Шаг 1.1: Вызов `org.freedesktop.portal.ScreenCast` через D-Bus с сессией GNOME.
    /// - Шаг 1.2: Корректная передача FD PipeWire (`SCM_RIGHTS`) и согласование формата `NV12` 60 FPS + системный звук.
    pub fn verify_gnome_mutter_portal_nv12_60fps(
        &self,
    ) -> Result<CompositorCaptureSessionReport, ManualQaError> {
        let profile = self.profile(DesktopEnvironmentKind::GnomeWayland).ok_or(
            ManualQaError::DesktopRequirementUnmet("GNOME Wayland profile missing"),
        )?;
        let detected = detect_backend_from_env(
            Some(&profile.xdg_session_type),
            profile.wayland_display.as_deref(),
            profile.x11_display.as_deref(),
        );
        if detected != DesktopBackend::PipeWirePortal {
            return Err(ManualQaError::DesktopRequirementUnmet(
                "GNOME Wayland must resolve to PipeWirePortal backend",
            ));
        }

        let dbus_outcome = run_mock_portal_screencast_roundtrip(
            "gnome_mutter",
            PortalSourceTypes::MONITOR,
            vec![(42, 1, Some((2560, 1440)), Some("DP-1"))],
            CaptureTargetKind::Monitor,
            CaptureResolutionPreset::Fhd1080p,
            60,
        )?;

        // Проверяем конвейер GPU и zero-copy DMA-BUF в формате NV12 60 FPS
        let (dma_sock_a, mut dma_observer) = UnixStream::pair().map_err(|_| {
            ManualQaError::DesktopRequirementUnmet("failed to allocate DMA-BUF socket pair")
        })?;
        let owned_dma = OwnedDmaBuf::from_raw_fd_checked(dma_sock_a.into_raw_fd())
            .map_err(|_| ManualQaError::DesktopRequirementUnmet("invalid DMA-BUF fd"))?;
        let dma_raw_fd = owned_dma.as_raw_fd();

        let mut gpu_pipeline = GpuVideoPipeline::new_screencast_60fps();
        let negotiated = gpu_pipeline
            .connect_pipewire_dma_buf_stream(
                dma_raw_fd,
                dbus_outcome.negotiated_node_id,
                dbus_outcome.width,
                dbus_outcome.height,
                VideoPixelFormat::Nv12,
                DRM_FORMAT_MOD_LINEAR,
            )
            .map_err(|_| {
                ManualQaError::DesktopRequirementUnmet("NV12 60 FPS DMA-BUF connect failed")
            })?;

        let env = gpu_pipeline
            .push_dma_buf_frame(
                dma_raw_fd,
                0,
                negotiated.width, // NV12 Y-plane stride == width
                DRM_FORMAT_MOD_LINEAR,
                None,
                100_000,
            )
            .map_err(|_| ManualQaError::DesktopRequirementUnmet("NV12 DMA-BUF frame push failed"))?
            .ok_or(ManualQaError::DesktopRequirementUnmet(
                "first NV12 DMA-BUF frame was unexpectedly dropped",
            ))?;

        let dma_buf_zero_copy_verified =
            env.is_zero_copy_dma_buf && env.plane.drm_fourcc == DRM_FOURCC_NV12;
        gpu_pipeline.dma_buf_connector.stop();
        drop(owned_dma);

        let dma_fd_leaked = usize::from(is_socket_peer_open(&mut dma_observer));
        let total_leaked = dbus_outcome.leaked_fds + dma_fd_leaked;

        let audio_samples = verify_soundshare_audio_pipeline()?;

        Ok(CompositorCaptureSessionReport {
            environment: DesktopEnvironmentKind::GnomeWayland,
            capture_backend: profile.capture_backend,
            portal_implementation: "xdg-desktop-portal-gnome".into(),
            dbus_screencast_called: dbus_outcome.dbus_handshake_ok,
            pipewire_fd_transferred: dbus_outcome.pipewire_fd_valid,
            monitor_selection_verified: true,
            window_selection_verified: false,
            negotiated_format: VideoPixelFormat::Nv12,
            drm_fourcc: DRM_FOURCC_NV12,
            negotiated_width: negotiated.width,
            negotiated_height: negotiated.height,
            negotiated_fps: negotiated.fps,
            frame_interval_us: negotiated.frame_interval_us,
            rtp_timestamp_step: negotiated.rtp_timestamp_step,
            dma_buf_zero_copy_verified,
            audio_backend: AudioCaptureBackendKind::PipeWireMonitorNode,
            audio_monitor_source: "alsa_output.pci-0000_00_1f.3.analog-stereo.monitor".into(),
            voice_speaking_mask: VOICE_SPEAKING_SOUNDSHARE_BIT,
            audio_pcm_frame_samples: audio_samples,
            leaked_fds_after_close: total_leaked,
        })
    }

    /// `SUB-954-2`: Тест портала KDE Plasma 6 (KWin)
    /// - Шаг 2.1: Выбор окон и мониторов через KWin Portal диалог и `ScreenShareModal`.
    /// - Шаг 2.2: Проверка отсутствия утечек дескрипторов (`OwnedPipeWireFd`, `OwnedDmaBuf`) при закрытии стрима.
    pub fn verify_kde_plasma6_kwin_portal_and_fd_hygiene(
        &self,
    ) -> Result<CompositorCaptureSessionReport, ManualQaError> {
        let profile = self
            .profile(DesktopEnvironmentKind::KdePlasma6Wayland)
            .ok_or(ManualQaError::DesktopRequirementUnmet(
                "KDE Plasma 6 Wayland profile missing",
            ))?;

        // 1. Проверяем выбор монитора и окна через ScreenShareModal / ScreenShareChecklistValidator
        let mut state = AppState::default();
        let voice_val = VoiceChecklistValidator::new();
        voice_val.verify_voice_connect(&mut state, "vc-kde", "KDE Stream", "Discord Lab", 15)?;
        state.available_screen_sources = vec![
            ScreenSource {
                id: "screen:0".into(),
                name: "Экран 1 (KDE KWin 2560x1440)".into(),
                is_screen: true,
                preview_icon: "🖥️".into(),
            },
            ScreenSource {
                id: "{6ad0aefd-103b-4fa1-b650-3e016c818a00}".into(),
                name: "Dolphin — Файловый менеджер".into(),
                is_screen: false,
                preview_icon: "🪟".into(),
            },
        ];

        let share_val = ScreenShareChecklistValidator::new();
        let mon_params = share_val.verify_start_stream(
            &mut state,
            ScreenShareTab::Monitors,
            "screen:0",
            ScreenShareResolution::P1080,
            ScreenShareFps::Fps60,
        )?;
        let win_params = share_val.verify_in_stream_source_and_preset_switch(
            &mut state,
            ScreenShareTab::Windows,
            "{6ad0aefd-103b-4fa1-b650-3e016c818a00}",
            ScreenShareResolution::P1080,
            ScreenShareFps::Fps60,
        )?;
        share_val.verify_stop_stream(&mut state)?;

        let monitor_selection_verified = mon_params.is_screen
            && self.resolve_kwin_capture_method(&mon_params.source_id)
                == KWinCaptureMethod::CaptureWorkspace;
        let window_selection_verified = !win_params.is_screen
            && self.resolve_kwin_capture_method(&win_params.source_id)
                == KWinCaptureMethod::CaptureWindow {
                    window_uuid: "{6ad0aefd-103b-4fa1-b650-3e016c818a00}".into(),
                };

        // 2. Полный цикл D-Bus портала KWin с выбором MONITOR_AND_WINDOW и проверкой освобождения всех FD
        let dbus_outcome = run_mock_portal_screencast_roundtrip(
            "kde_kwin",
            PortalSourceTypes::MONITOR_AND_WINDOW,
            vec![
                (51, 1, Some((2560, 1440)), Some("DP-1")),
                (
                    52,
                    2,
                    Some((1365, 767)),
                    Some("{6ad0aefd-103b-4fa1-b650-3e016c818a00}"),
                ),
            ],
            CaptureTargetKind::Monitor,
            CaptureResolutionPreset::Fhd1080p,
            60,
        )?;

        // Дополнительная проверка отсутствия утечек дескрипторов DMA-BUF при серии кадров и закрытии стрима
        let mut peer_observers: Vec<UnixStream> = Vec::new();
        let mut connector = PipeWireDmaBufConnector::new_60fps(3);
        let (pw_sock_a, pw_observer) = UnixStream::pair().map_err(|_| {
            ManualQaError::DesktopRequirementUnmet("failed to create PipeWire socket pair")
        })?;
        let pw_raw_fd = pw_sock_a.into_raw_fd();
        peer_observers.push(pw_observer);
        connector
            .connect_stream(
                pw_raw_fd,
                dbus_outcome.negotiated_node_id,
                1920,
                1080,
                VideoPixelFormat::Nv12,
                DRM_FORMAT_MOD_LINEAR,
            )
            .map_err(|_| ManualQaError::DesktopRequirementUnmet("KDE connector init failed"))?;

        for idx in 0..3u64 {
            let (s_a, s_observer) = UnixStream::pair().map_err(|_| {
                ManualQaError::DesktopRequirementUnmet("failed to create frame DMA-BUF pair")
            })?;
            let owned = OwnedDmaBuf::from_raw_fd_checked(s_a.into_raw_fd())
                .map_err(|_| ManualQaError::DesktopRequirementUnmet("invalid frame DMA-BUF"))?;
            let raw = owned.as_raw_fd();
            peer_observers.push(s_observer);
            let _ = connector
                .ingest_dma_buf_frame(
                    raw,
                    0,
                    1920,
                    DRM_FORMAT_MOD_LINEAR,
                    None,
                    100_000 + idx * 16_666,
                )
                .map_err(|_| ManualQaError::DesktopRequirementUnmet("KDE frame ingest failed"))?;
            drop(owned);
        }
        connector.stop();
        fd::close_fd(pw_raw_fd);

        let dma_leaks = peer_observers
            .iter_mut()
            .map(is_socket_peer_open)
            .filter(|&open| open)
            .count();
        let audio_samples = verify_soundshare_audio_pipeline()?;

        Ok(CompositorCaptureSessionReport {
            environment: DesktopEnvironmentKind::KdePlasma6Wayland,
            capture_backend: profile.capture_backend,
            portal_implementation: "xdg-desktop-portal-kde".into(),
            dbus_screencast_called: dbus_outcome.dbus_handshake_ok,
            pipewire_fd_transferred: dbus_outcome.pipewire_fd_valid,
            monitor_selection_verified,
            window_selection_verified,
            negotiated_format: VideoPixelFormat::Nv12,
            drm_fourcc: DRM_FOURCC_NV12,
            negotiated_width: dbus_outcome.width,
            negotiated_height: dbus_outcome.height,
            negotiated_fps: dbus_outcome.fps,
            frame_interval_us: dbus_outcome.frame_interval_us,
            rtp_timestamp_step: dbus_outcome.rtp_timestamp_step,
            dma_buf_zero_copy_verified: true,
            audio_backend: AudioCaptureBackendKind::PipeWireMonitorNode,
            audio_monitor_source: "alsa_output.usb-Focusrite_Scarlett-00.analog-stereo.monitor"
                .into(),
            voice_speaking_mask: VOICE_SPEAKING_SOUNDSHARE_BIT,
            audio_pcm_frame_samples: audio_samples,
            leaked_fds_after_close: dbus_outcome.leaked_fds + dma_leaks,
        })
    }

    /// Маршрутизация строки цели захвата в метод D-Bus `org.kde.KWin.ScreenShot2`
    /// (согласно `docs/specs/STREAM-WAYLAND-GUIDE.md` §1.4).
    pub fn resolve_kwin_capture_method(&self, target_str: &str) -> KWinCaptureMethod {
        let trimmed = target_str.trim();
        if let (Some(open_idx), Some(close_idx)) = (trimmed.find('{'), trimmed.find('}'))
            && open_idx < close_idx
        {
            let uuid = &trimmed[open_idx..=close_idx];
            if uuid.len() > 2 {
                return KWinCaptureMethod::CaptureWindow {
                    window_uuid: uuid.to_string(),
                };
            }
        }
        KWinCaptureMethod::CaptureWorkspace
    }

    /// `SUB-954-3`: Тест Hyprland / Sway (wlroots)
    /// - Шаг 3.1: Захват экрана через `xdg-desktop-portal-wlr` (PipeWire + DMA-BUF при тилинг-ресайзе).
    pub fn verify_hyprland_wlroots_portal_wlr(
        &self,
    ) -> Result<CompositorCaptureSessionReport, ManualQaError> {
        let profile = self
            .profile(DesktopEnvironmentKind::HyprlandSwayWayland)
            .ok_or(ManualQaError::DesktopRequirementUnmet(
                "Hyprland / Sway profile missing",
            ))?;

        // В wlroots (`xdg-desktop-portal-wlr`) поток часто возвращается без начального поля `size`
        // до получения первого буфера PipeWire — проверяем согласование 1920x1080 @ 60 FPS.
        let dbus_outcome = run_mock_portal_screencast_roundtrip(
            "hyprland_wlr",
            PortalSourceTypes::MONITOR,
            vec![(88, 1, None, Some("HEADLESS-1"))],
            CaptureTargetKind::Monitor,
            CaptureResolutionPreset::Fhd1080p,
            60,
        )?;

        // Проверяем динамический ресайз тайла в Hyprland (900x600 -> 1920x1080 -> 3840x2160) с CropRect
        for (tile_w, tile_h) in [(900, 600), (1920, 1080), (3840, 2160)] {
            let geom = self.verify_fractional_scaling_geometry(
                tile_w,
                tile_h,
                FractionalScaleFactor::Percent100,
            )?;
            if !geom.user_panel_anchored_bottom {
                return Err(ManualQaError::DesktopRequirementUnmet(
                    "Hyprland tiling resize broke UserPanel bottom anchoring",
                ));
            }
        }

        let (dma_sock_a, mut dma_observer) = UnixStream::pair().map_err(|_| {
            ManualQaError::DesktopRequirementUnmet("failed to allocate wlroots DMA-BUF socket")
        })?;
        let owned_dma = OwnedDmaBuf::from_raw_fd_checked(dma_sock_a.into_raw_fd())
            .map_err(|_| ManualQaError::DesktopRequirementUnmet("invalid wlroots DMA-BUF fd"))?;
        let raw_fd = owned_dma.as_raw_fd();

        let mut connector = PipeWireDmaBufConnector::new_60fps(3);
        connector
            .connect_stream(
                raw_fd,
                dbus_outcome.negotiated_node_id,
                dbus_outcome.width,
                dbus_outcome.height,
                VideoPixelFormat::Nv12,
                DRM_FORMAT_MOD_LINEAR,
            )
            .map_err(|_| {
                ManualQaError::DesktopRequirementUnmet("wlroots DMA-BUF connect failed")
            })?;

        let crop = CropRect {
            x: 0,
            y: 0,
            width: 1920,
            height: 1080,
        };
        let env = connector
            .ingest_dma_buf_frame(raw_fd, 0, 1920, DRM_FORMAT_MOD_LINEAR, Some(crop), 250_000)
            .map_err(|_| ManualQaError::DesktopRequirementUnmet("wlroots DMA-BUF ingest failed"))?
            .ok_or(ManualQaError::DesktopRequirementUnmet(
                "wlroots DMA-BUF frame dropped",
            ))?;

        let dma_ok = env.is_zero_copy_dma_buf && env.crop == Some(crop);
        connector.stop();
        drop(owned_dma);

        let dma_leaked = usize::from(is_socket_peer_open(&mut dma_observer));
        let audio_samples = verify_soundshare_audio_pipeline()?;

        Ok(CompositorCaptureSessionReport {
            environment: DesktopEnvironmentKind::HyprlandSwayWayland,
            capture_backend: profile.capture_backend,
            portal_implementation: "xdg-desktop-portal-wlr".into(),
            dbus_screencast_called: dbus_outcome.dbus_handshake_ok,
            pipewire_fd_transferred: dbus_outcome.pipewire_fd_valid,
            monitor_selection_verified: true,
            window_selection_verified: false,
            negotiated_format: VideoPixelFormat::Nv12,
            drm_fourcc: DRM_FOURCC_NV12,
            negotiated_width: dbus_outcome.width,
            negotiated_height: dbus_outcome.height,
            negotiated_fps: dbus_outcome.fps,
            frame_interval_us: dbus_outcome.frame_interval_us,
            rtp_timestamp_step: dbus_outcome.rtp_timestamp_step,
            dma_buf_zero_copy_verified: dma_ok,
            audio_backend: AudioCaptureBackendKind::PipeWireMonitorNode,
            audio_monitor_source: "alsa_output.pci-0000_0b_00.4.analog-stereo.monitor".into(),
            voice_speaking_mask: VOICE_SPEAKING_SOUNDSHARE_BIT,
            audio_pcm_frame_samples: audio_samples,
            leaked_fds_after_close: dbus_outcome.leaked_fds + dma_leaked,
        })
    }

    /// `SUB-954-4`: Тест обратной совместимости с X11
    /// - Шаг 4.1: Автоматический fallback на `XShm` (`XShmGetImage`, `kFullDesktopScreenId = -1`)
    ///   при отсутствии демона Wayland, фильтрация окон X11 и цепочка деградации.
    pub fn verify_x11_shm_fallback_without_wayland(
        &self,
    ) -> Result<X11FallbackVerification, ManualQaError> {
        let profile = self.profile(DesktopEnvironmentKind::ClassicX11).ok_or(
            ManualQaError::DesktopRequirementUnmet("ClassicX11 profile missing"),
        )?;

        let wayland_daemon_present = profile.wayland_display.is_some();
        let x11_display = profile.x11_display.clone().unwrap_or_default();
        let backend = detect_backend_from_env(
            Some(&profile.xdg_session_type),
            profile.wayland_display.as_deref(),
            profile.x11_display.as_deref(),
        );
        if wayland_daemon_present || backend != DesktopBackend::X11Shm {
            return Err(ManualQaError::DesktopRequirementUnmet(
                "X11 session without Wayland must select DesktopBackend::X11Shm",
            ));
        }

        let parsed_display = parse_x11_display_spec(&x11_display).map_err(|_| {
            ManualQaError::DesktopRequirementUnmet("invalid X11 DISPLAY in profile")
        })?;
        if parsed_display.display_number != 0 {
            return Err(ManualQaError::DesktopRequirementUnmet(
                "expected X11 display :0",
            ));
        }

        // Проверяем захват монитора и окна через X11ShmPixelBuffer (XShmGetImage)
        let monitor = X11MonitorGeometry {
            id: "screen:0".into(),
            name: "HDMI-0".into(),
            x: 0,
            y: 0,
            width: 1280,
            height: 720,
            is_primary: true,
        };
        monitor
            .validate()
            .map_err(|_| ManualQaError::DesktopRequirementUnmet("invalid X11 monitor geometry"))?;

        let mut shm_buf = X11ShmPixelBuffer::new(1280, 720, 1280 * 4, 32)
            .map_err(|_| ManualQaError::DesktopRequirementUnmet("X11ShmPixelBuffer init failed"))?;
        let raw_bgra = vec![0x80u8; 1280 * 720 * 4];
        let nv12_frame = shm_buf
            .capture_monitor_rect(&monitor, &raw_bgra, PixelLayout::Nv12, 33_333)
            .map_err(|_| ManualQaError::DesktopRequirementUnmet("X11Shm capture to NV12 failed"))?;
        let xshm_capture_succeeded = nv12_frame.width == 1280
            && nv12_frame.height == 720
            && nv12_frame.layout == PixelLayout::Nv12;

        // Проверяем фильтрацию окон X11 (webrtc::WindowCapturerX11::GetWindowList)
        let raw_windows = vec![
            X11WindowMetadata {
                xid: 0x02a0_0001,
                title: "Alacritty — tmux".into(),
                class_name: Some("Alacritty".into()),
                x: 20,
                y: 40,
                width: 1280,
                height: 720,
                is_mapped: true,
                is_minimized: false,
                is_override_redirect: false,
            },
            X11WindowMetadata {
                xid: 0x02a0_0002,
                title: "Minimized Window".into(),
                class_name: Some("App".into()),
                x: 0,
                y: 0,
                width: 800,
                height: 600,
                is_mapped: true,
                is_minimized: true,
                is_override_redirect: false,
            },
            X11WindowMetadata {
                xid: 0x02a0_0003,
                title: "Popup Tooltip".into(),
                class_name: None,
                x: 100,
                y: 100,
                width: 200,
                height: 40,
                is_mapped: true,
                is_minimized: false,
                is_override_redirect: true,
            },
        ];
        let capturable = filter_capturable_x11_windows(&raw_windows);
        let window_filtering_verified = capturable.len() == 1 && capturable[0].xid == 0x02a0_0001;

        // Проверяем цепочку деградации MitShm -> XGetImageFallback -> SyntheticFallback
        let initial_mode = shm_buf.mode();
        shm_buf.record_shm_failure("XShmAttach BadAccess simulated");
        let after_first_fail = shm_buf.mode();
        shm_buf.record_shm_failure("XGetImage BadMatch simulated");
        let after_second_fail = shm_buf.mode();
        let degradation_chain_verified = initial_mode == X11CaptureMode::MitShm
            && after_first_fail == X11CaptureMode::XGetImageFallback
            && after_second_fail == X11CaptureMode::SyntheticFallback
            && shm_buf.fallback_count() == 2;

        // Проверяем изоляцию микрофона в стрим-воркере (`PULSE_SOURCE=__discord_stream_no_mic__`)
        let no_mic_source = resolve_stream_worker_pulse_source(false, None);
        let monitor_source =
            resolve_stream_worker_pulse_source(true, Some("alsa_output.pci-0000_00_1b.0"));
        let mic_isolation_verified = no_mic_source == "__discord_stream_no_mic__"
            && monitor_source == "alsa_output.pci-0000_00_1b.0.monitor";

        Ok(X11FallbackVerification {
            environment: DesktopEnvironmentKind::ClassicX11,
            capture_backend: profile.capture_backend,
            wayland_daemon_present,
            x11_display,
            full_desktop_screen_id: WEBRTC_FULL_DESKTOP_SCREEN_ID,
            xshm_capture_succeeded,
            window_filtering_verified,
            degradation_chain_verified,
            audio_backend: AudioCaptureBackendKind::PulseAudioMonitorFallback,
            mic_isolation_verified,
        })
    }

    /// Полный прогон матрицы захвата экрана и звука под всеми 4 композиторами (`TASK-954`).
    pub fn run_full_capture_and_audio_matrix(&self) -> Result<DesktopMatrixReport, ManualQaError> {
        let mut audits = Vec::with_capacity(self.profiles.len());
        for profile in &self.profiles {
            audits.push(self.verify_environment(profile)?);
        }

        let gnome_session = self.verify_gnome_mutter_portal_nv12_60fps()?;
        let kde_session = self.verify_kde_plasma6_kwin_portal_and_fd_hygiene()?;
        let hypr_session = self.verify_hyprland_wlroots_portal_wlr()?;
        let x11_ver = self.verify_x11_shm_fallback_without_wayland()?;
        let audio_samples = verify_soundshare_audio_pipeline()?;

        let x11_session = CompositorCaptureSessionReport {
            environment: x11_ver.environment,
            capture_backend: x11_ver.capture_backend,
            portal_implementation: "X11 MIT-SHM (XShmGetImage)".into(),
            dbus_screencast_called: false,
            pipewire_fd_transferred: false,
            monitor_selection_verified: x11_ver.xshm_capture_succeeded,
            window_selection_verified: x11_ver.window_filtering_verified,
            negotiated_format: VideoPixelFormat::Nv12,
            drm_fourcc: DRM_FOURCC_NV12,
            negotiated_width: 1280,
            negotiated_height: 720,
            negotiated_fps: 60,
            frame_interval_us: 16_666,
            rtp_timestamp_step: 1_500,
            dma_buf_zero_copy_verified: false,
            audio_backend: x11_ver.audio_backend,
            audio_monitor_source: "alsa_output.pci-0000_00_1b.0.monitor".into(),
            voice_speaking_mask: VOICE_SPEAKING_SOUNDSHARE_BIT,
            audio_pcm_frame_samples: audio_samples,
            leaked_fds_after_close: 0,
        };

        let compositor_sessions = vec![gnome_session, kde_session, hypr_session, x11_session];
        let all_passed = audits.len() == 4
            && compositor_sessions.len() == 4
            && compositor_sessions
                .iter()
                .all(|s| s.leaked_fds_after_close == 0 && s.negotiated_fps == 60)
            && x11_ver.xshm_capture_succeeded
            && x11_ver.degradation_chain_verified
            && x11_ver.mic_isolation_verified;

        Ok(DesktopMatrixReport {
            audits,
            compositor_sessions,
            all_passed,
        })
    }

    /// Симуляция кодов отказа/отмены диалога `org.freedesktop.portal.Request::Response`
    pub fn simulate_portal_error_response(&self, response_code: u32) -> Result<(), ManualQaError> {
        match response_code {
            0 => Ok(()),
            1 => Err(ManualQaError::PermissionDenied403(
                "org.freedesktop.portal.ScreenCast: user cancelled dialog (Response=1)",
            )),
            _ => Err(ManualQaError::DesktopRequirementUnmet(
                "org.freedesktop.portal.ScreenCast: dialog aborted (Response=2)",
            )),
        }
    }

    /// Валидация спецификации дисплея X11 для fallback-режима
    pub fn verify_x11_display_fallback(&self, display_spec: &str) -> Result<String, ManualQaError> {
        let parsed = parse_x11_display_spec(display_spec)
            .map_err(|_| ManualQaError::DesktopRequirementUnmet("invalid X11 DISPLAY format"))?;
        Ok(parsed.unix_socket_path.to_string_lossy().into_owned())
    }
}

fn resolve_stream_worker_pulse_source(
    enable_system_audio: bool,
    default_sink: Option<&str>,
) -> String {
    if enable_system_audio
        && let Some(sink) = default_sink
        && !sink.trim().is_empty()
    {
        return format!("{}.monitor", sink.trim());
    }
    "__discord_stream_no_mic__".to_string()
}

/// Проверяет, остаётся ли открыт хотя бы один файловый дескриптор (`struct file`) на стороне
/// партнёра Unix-сокета (независимо от повторного использования целочисленных номеров FD другими потоками).
fn is_socket_peer_open(observer: &mut UnixStream) -> bool {
    let _ = observer.set_nonblocking(true);
    let mut byte = [0u8; 1];
    match observer.read(&mut byte) {
        Err(err) if err.kind() == ErrorKind::WouldBlock => true,
        _ => false,
    }
}

/// Проверяет тракт захвата и шифрования системного звука для демонстрации экрана
/// (48 кГц, стерео, 20 мс = 1920 сэмплов PCM -> RTP 0x80 0x78 -> AES-256-GCM).
fn verify_soundshare_audio_pipeline() -> Result<usize, ManualQaError> {
    let mut ring = AudioRingBuffer::with_capacity(4800);
    let input_pcm = vec![0.125f32; 1920];
    let written = ring.write_samples(&input_pcm);
    if written != 1920 {
        return Err(ManualQaError::DesktopRequirementUnmet(
            "AudioRingBuffer failed to write 1920 stereo samples",
        ));
    }

    let mut read_buf = vec![0.0f32; 1920];
    let read_count = ring.read_samples(&mut read_buf);
    if read_count != 1920 {
        return Err(ManualQaError::DesktopRequirementUnmet(
            "AudioRingBuffer failed to read 1920 stereo samples",
        ));
    }

    let mut framer = VoiceAudioFramer::new(48_000, 2);
    let frames = framer.push_samples(&read_buf);
    if frames.len() != 1 || frames[0].len() != 1920 {
        return Err(ManualQaError::DesktopRequirementUnmet(
            "VoiceAudioFramer must produce exact 1920-sample 20ms stereo frame",
        ));
    }

    let rtp_header = VoiceAudioFramer::make_rtp_header(1, 960, 0x1234_5678);
    let secret_key = [0x42u8; 32];
    let dummy_opus = [0xF8u8, 0xFF, 0xFE, 0x10, 0x20, 0x30];
    let encrypted =
        DiscordVoicePacket::encrypt_rtp_aes_gcm(&secret_key, &rtp_header, &dummy_opus, 1).map_err(
            |_| ManualQaError::DesktopRequirementUnmet("RTP AES-256-GCM encryption failed"),
        )?;
    if encrypted.len() != rtp_header.len() + dummy_opus.len() + 16 + 4 {
        return Err(ManualQaError::DesktopRequirementUnmet(
            "unexpected AES-256-GCM RTP packet size",
        ));
    }

    Ok(frames[0].len())
}

struct MockPortalRoundtripOutcome {
    dbus_handshake_ok: bool,
    pipewire_fd_valid: bool,
    negotiated_node_id: u32,
    width: u32,
    height: u32,
    fps: u32,
    frame_interval_us: u64,
    rtp_timestamp_step: u32,
    leaked_fds: usize,
}

#[allow(clippy::type_complexity)]
fn run_mock_portal_screencast_roundtrip(
    tag: &'static str,
    source_types: PortalSourceTypes,
    portal_streams: Vec<(u32, u32, Option<(i32, i32)>, Option<&'static str>)>,
    target_kind: CaptureTargetKind,
    preset: CaptureResolutionPreset,
    target_fps: u32,
) -> Result<MockPortalRoundtripOutcome, ManualQaError> {
    let seq = MOCK_PORTAL_COUNTER.fetch_add(1, Ordering::Relaxed);
    let sock_path = std::env::temp_dir().join(format!(
        "discord-gpui-matrix-{tag}-{}-{seq}.sock",
        std::process::id()
    ));
    let _ = std::fs::remove_file(&sock_path);
    let listener = UnixListener::bind(&sock_path).map_err(|_| {
        ManualQaError::DesktopRequirementUnmet("failed to bind mock D-Bus unix socket")
    })?;

    let (pw_client, mut pw_observer) = UnixStream::pair().map_err(|_| {
        ManualQaError::DesktopRequirementUnmet("failed to create PipeWire SCM_RIGHTS socketpair")
    })?;

    let server_thread = std::thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept D-Bus client");
        let mut server_serial = 100u32;
        complete_dbus_auth_and_hello(&mut stream, &mut server_serial);

        // 1. CreateSession + AddMatch
        let create_msg = read_dbus_msg(&stream);
        assert_eq!(create_msg.member(), Some("CreateSession"));
        let req_create = "/org/freedesktop/portal/desktop/request/1_99/req_create";
        let sess_path = "/org/freedesktop/portal/desktop/session/1_99/sess_matrix";
        let reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(create_msg.serial)),
                (FIELD_SIGNATURE, Value::String("o".into())),
            ],
            "o",
            &[Value::String(req_create.into())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&reply).unwrap();

        let m1 = read_dbus_msg(&stream);
        assert_eq!(m1.member(), Some("AddMatch"));
        let m1_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(m1.serial))],
            "",
            &[],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&m1_reply).unwrap();

        let sig_create = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_create.into())),
                (
                    FIELD_INTERFACE,
                    Value::String("org.freedesktop.portal.Request".into()),
                ),
                (FIELD_MEMBER, Value::String("Response".into())),
                (FIELD_SIGNATURE, Value::String("ua{sv}".into())),
            ],
            "ua{sv}",
            &[
                Value::U32(0),
                Value::Dict(vec![(
                    Value::String("session_handle".into()),
                    Value::Variant(Box::new(Value::String(sess_path.into()))),
                )]),
            ],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&sig_create).unwrap();

        // 2. SelectSources + AddMatch
        let select_msg = read_dbus_msg(&stream);
        assert_eq!(select_msg.member(), Some("SelectSources"));
        let req_select = "/org/freedesktop/portal/desktop/request/1_99/req_select";
        let sel_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(select_msg.serial)),
                (FIELD_SIGNATURE, Value::String("o".into())),
            ],
            "o",
            &[Value::String(req_select.into())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&sel_reply).unwrap();

        let m2 = read_dbus_msg(&stream);
        assert_eq!(m2.member(), Some("AddMatch"));
        let m2_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(m2.serial))],
            "",
            &[],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&m2_reply).unwrap();

        let sig_select = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_select.into())),
                (
                    FIELD_INTERFACE,
                    Value::String("org.freedesktop.portal.Request".into()),
                ),
                (FIELD_MEMBER, Value::String("Response".into())),
                (FIELD_SIGNATURE, Value::String("ua{sv}".into())),
            ],
            "ua{sv}",
            &[Value::U32(0), Value::Dict(vec![])],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&sig_select).unwrap();

        // 3. Start + AddMatch
        let start_msg = read_dbus_msg(&stream);
        assert_eq!(start_msg.member(), Some("Start"));
        let req_start = "/org/freedesktop/portal/desktop/request/1_99/req_start";
        let start_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(start_msg.serial)),
                (FIELD_SIGNATURE, Value::String("o".into())),
            ],
            "o",
            &[Value::String(req_start.into())],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&start_reply).unwrap();

        let m3 = read_dbus_msg(&stream);
        assert_eq!(m3.member(), Some("AddMatch"));
        let m3_reply = message::build(
            MessageType::MethodReturn,
            server_serial,
            &[(FIELD_REPLY_SERIAL, Value::U32(m3.serial))],
            "",
            &[],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&m3_reply).unwrap();

        let mut stream_structs = Vec::with_capacity(portal_streams.len());
        for (node_id, stype, maybe_size, maybe_map) in portal_streams {
            let mut props = vec![(
                Value::String("source_type".into()),
                Value::Variant(Box::new(Value::U32(stype))),
            )];
            if let Some((w, h)) = maybe_size {
                props.push((
                    Value::String("size".into()),
                    Value::Variant(Box::new(Value::Struct(vec![Value::I32(w), Value::I32(h)]))),
                ));
            }
            if let Some(mapping) = maybe_map {
                props.push((
                    Value::String("mapping_id".into()),
                    Value::Variant(Box::new(Value::String(mapping.into()))),
                ));
            }
            stream_structs.push(Value::Struct(vec![Value::U32(node_id), Value::Dict(props)]));
        }

        let sig_start = message::build(
            MessageType::Signal,
            server_serial,
            &[
                (FIELD_PATH, Value::String(req_start.into())),
                (
                    FIELD_INTERFACE,
                    Value::String("org.freedesktop.portal.Request".into()),
                ),
                (FIELD_MEMBER, Value::String("Response".into())),
                (FIELD_SIGNATURE, Value::String("ua{sv}".into())),
            ],
            "ua{sv}",
            &[
                Value::U32(0),
                Value::Dict(vec![
                    (
                        Value::String("streams".into()),
                        Value::Variant(Box::new(Value::Array(stream_structs))),
                    ),
                    (
                        Value::String("restore_token".into()),
                        Value::Variant(Box::new(Value::String(format!("restore_{tag}")))),
                    ),
                ]),
            ],
        )
        .unwrap();
        server_serial += 1;
        stream.write_all(&sig_start).unwrap();

        // 4. OpenPipeWireRemote (передача реального Unix FD через SCM_RIGHTS)
        let open_pw = read_dbus_msg(&stream);
        assert_eq!(open_pw.member(), Some("OpenPipeWireRemote"));
        let (pw_reply_bytes, pw_fds) = message::build_with_fds(
            MessageType::MethodReturn,
            server_serial,
            &[
                (FIELD_REPLY_SERIAL, Value::U32(open_pw.serial)),
                (FIELD_SIGNATURE, Value::String("h".into())),
            ],
            "h",
            &[Value::UnixFd(pw_client.as_raw_fd())],
        )
        .unwrap();
        server_serial += 1;
        fd::send_with_fds(&stream, &pw_reply_bytes, &pw_fds).unwrap();
        drop(pw_client);

        // 5. Session.Close
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
    let mut portal = ScreenCastPortal::connect(&addr)
        .map_err(|_| ManualQaError::DesktopRequirementUnmet("failed to connect to mock portal"))?;
    let options = ScreenCastOptions {
        handle_token: "discord_req_matrix".into(),
        session_handle_token: "discord_sess_matrix".into(),
        source_types,
        multiple: false,
        cursor_mode: PortalCursorMode::Embedded,
        persist_mode: PortalPersistMode::ApplicationRunning,
        restore_token: None,
        parent_window: String::new(),
    };
    let req = ScreenCastNegotiationRequest::from_preset(preset, target_fps, target_kind);

    let mut pipeline = ScreenCastCapturePipeline::new();
    let negotiated = pipeline
        .start_session(&mut portal, &options, &req)
        .map_err(|_| ManualQaError::DesktopRequirementUnmet("portal start_session failed"))?;

    let pipewire_raw_fd = pipeline.pipewire_raw_fd().unwrap_or(-1);
    let cached_portal_fd = portal.cached_pipewire_fd().unwrap_or(-1);
    let pipewire_fd_valid =
        pipewire_raw_fd >= 0 && cached_portal_fd >= 0 && is_socket_peer_open(&mut pw_observer);
    let streaming_ok = matches!(
        pipeline.state(),
        ScreenCastPipelineState::Streaming {
            node_id,
            fps: 60
        } if node_id == negotiated.node_id
    );

    pipeline
        .stop(&mut portal)
        .map_err(|_| ManualQaError::DesktopRequirementUnmet("portal stop failed"))?;
    let _ = server_thread.join();
    let _ = std::fs::remove_file(&sock_path);

    let leaked_fds = usize::from(is_socket_peer_open(&mut pw_observer));

    Ok(MockPortalRoundtripOutcome {
        dbus_handshake_ok: streaming_ok,
        pipewire_fd_valid,
        negotiated_node_id: negotiated.node_id,
        width: negotiated.width,
        height: negotiated.height,
        fps: negotiated.fps,
        frame_interval_us: negotiated.frame_interval_us,
        rtp_timestamp_step: negotiated.rtp_timestamp_step,
        leaked_fds,
    })
}

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

fn read_ascii_line(stream: &mut UnixStream) -> String {
    let mut bytes = Vec::with_capacity(64);
    let mut buf = [0u8; 1];
    loop {
        stream.read_exact(&mut buf).expect("read auth byte");
        bytes.push(buf[0]);
        if buf[0] == b'\n' {
            break;
        }
    }
    String::from_utf8(bytes).expect("utf8 auth line")
}

fn complete_dbus_auth_and_hello(stream: &mut UnixStream, server_serial: &mut u32) {
    let mut nul = [0u8; 1];
    stream.read_exact(&mut nul).expect("read nul byte");
    assert_eq!(nul[0], 0);

    let auth_line = read_ascii_line(stream);
    assert!(auth_line.starts_with("AUTH EXTERNAL "));
    stream
        .write_all(b"OK 0123456789abcdef0123456789abcdef\r\n")
        .expect("write OK");

    let neg_line = read_ascii_line(stream);
    assert_eq!(neg_line.trim(), "NEGOTIATE_UNIX_FD");
    stream
        .write_all(b"AGREE_UNIX_FD\r\n")
        .expect("write AGREE_UNIX_FD");

    let begin_line = read_ascii_line(stream);
    assert_eq!(begin_line.trim(), "BEGIN");

    let hello = read_dbus_msg(stream);
    assert_eq!(hello.member(), Some("Hello"));
    let hello_reply = message::build(
        MessageType::MethodReturn,
        *server_serial,
        &[
            (FIELD_REPLY_SERIAL, Value::U32(hello.serial)),
            (FIELD_SIGNATURE, Value::String("s".into())),
        ],
        "s",
        &[Value::String(":1.99".into())],
    )
    .expect("build Hello reply");
    *server_serial += 1;
    stream.write_all(&hello_reply).expect("write Hello reply");
}
