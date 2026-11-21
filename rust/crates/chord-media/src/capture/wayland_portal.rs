//! R5-06: Wayland ScreenCast Portal (`chord_media::capture::wayland_portal`).
//!
//! On Wayland environments with system portal choosers, bypasses redundant custom in-app
//! screen share pickers and delegates directly to `xdg-desktop-portal`
//! (`org.freedesktop.portal.ScreenCast`), ensuring the stream initiates via a single native
//! portal dialog (or zero dialogs when a persisted `restore_token` is honored) while
//! preserving full native display resolution (including ultrawide, 4K/5K/8K, and fractional
//! HiDPI physical pixel dimensions).
//!
//! Specification & Reference Oracles:
//! - `docs/feature-backlog.md` card `R5-06`:
//!   "On Wayland environments with system portal choosers, bypass redundant custom picker and
//!   delegate directly to `xdg-desktop-portal` | Vesktop: `screenShare.ts` | Capture Service:
//!   Wayland Portal | Stream initiates via native portal dialog without dual picker prompts;
//!   full native display resolution preserved"
//! - `docs/vesktop-analysis.md` §2.3 ("Desktop Portal Integration")
//! - `/usr/share/dbus-1/interfaces/org.freedesktop.portal.ScreenCast.xml` (v6)

use crate::capture::portal::{
    PipeWireConfig, PortalCursorMode, PortalPersistMode, PortalScreenCastClient,
    PortalSessionConfig, PortalSessionState, PortalStream, PortalTransport, SOURCE_TYPE_MONITOR,
    SOURCE_TYPE_WINDOW,
};
use crate::capture::sources::detect_backend_from_env;
use crate::capture::{
    CaptureError, DesktopBackend, DesktopSource, MAX_CAPTURE_DIMENSION, ScreenCapturer,
};
use crate::{PixelLayout, VideoFrame};

/// Minimum valid capture dimension in pixels (`64` px).
pub const MIN_NATIVE_CAPTURE_DIMENSION: u32 = 64;
/// Minimum allowed display scale factor in milli-units (`500` = `0.5x`).
pub const MIN_SCALE_FACTOR_MILLI: u32 = 500;
/// Unity display scale factor in milli-units (`1000` = `1.0x`).
pub const UNITY_SCALE_FACTOR_MILLI: u32 = 1000;
/// Maximum allowed display scale factor in milli-units (`4000` = `4.0x`).
pub const MAX_SCALE_FACTOR_MILLI: u32 = 4000;
/// Default fallback native resolution `(1920, 1080)` when a Wayland compositor defers stream size
/// reporting until the first PipeWire `SpaBuffer` arrives.
pub const DEFAULT_FALLBACK_NATIVE_RESOLUTION: (u32, u32) = (1920, 1080);

/// Detected Wayland compositor family hosting the `xdg-desktop-portal` ScreenCast backend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaylandCompositorKind {
    /// GNOME Shell / Mutter (`xdg-desktop-portal-gnome`).
    GnomeMutter,
    /// KDE Plasma / KWin (`xdg-desktop-portal-kde`).
    KdeKwin,
    /// Hyprland (`xdg-desktop-portal-hyprland`).
    Hyprland,
    /// Sway (`xdg-desktop-portal-wlr`).
    Sway,
    /// System76 COSMIC (`xdg-desktop-portal-cosmic`).
    Cosmic,
    /// Other wlroots or Wayland compositor (`xdg-desktop-portal-wlr` / `-gtk`).
    OtherWlroots,
}

impl WaylandCompositorKind {
    /// Detects the Wayland compositor family from `XDG_CURRENT_DESKTOP` or `DESKTOP_SESSION`.
    #[must_use]
    pub fn from_xdg_desktop(desktop: Option<&str>) -> Self {
        let lower = desktop.unwrap_or("").trim().to_ascii_lowercase();
        if lower.contains("gnome") {
            Self::GnomeMutter
        } else if lower.contains("kde") || lower.contains("plasma") {
            Self::KdeKwin
        } else if lower.contains("hyprland") {
            Self::Hyprland
        } else if lower.contains("sway") {
            Self::Sway
        } else if lower.contains("cosmic") {
            Self::Cosmic
        } else {
            Self::OtherWlroots
        }
    }

    /// Canonical portal backend name for diagnostics and telemetry.
    #[must_use]
    pub const fn portal_backend_name(self) -> &'static str {
        match self {
            Self::GnomeMutter => "xdg-desktop-portal-gnome",
            Self::KdeKwin => "xdg-desktop-portal-kde",
            Self::Hyprland => "xdg-desktop-portal-hyprland",
            Self::Sway | Self::OtherWlroots => "xdg-desktop-portal-wlr",
            Self::Cosmic => "xdg-desktop-portal-cosmic",
        }
    }
}

/// Environment snapshot used to determine whether to bypass the custom in-app picker
/// and delegate directly to `xdg-desktop-portal`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaylandEnvironmentProbe {
    pub xdg_session_type: Option<String>,
    pub wayland_display: Option<String>,
    pub x11_display: Option<String>,
    pub xdg_current_desktop: Option<String>,
    pub portal_screencast_available: bool,
    pub portal_version: u32,
}

impl WaylandEnvironmentProbe {
    /// Builds a default Wayland environment probe with `xdg-desktop-portal` ScreenCast v6 active.
    #[must_use]
    pub fn wayland_default() -> Self {
        Self {
            xdg_session_type: Some("wayland".to_string()),
            wayland_display: Some("wayland-0".to_string()),
            x11_display: Some(":0".to_string()),
            xdg_current_desktop: Some("KDE".to_string()),
            portal_screencast_available: true,
            portal_version: 6,
        }
    }

    /// Probes the current process environment variables.
    #[must_use]
    pub fn from_live_env(portal_screencast_available: bool, portal_version: u32) -> Self {
        Self {
            xdg_session_type: std::env::var("XDG_SESSION_TYPE").ok(),
            wayland_display: std::env::var("WAYLAND_DISPLAY").ok(),
            x11_display: std::env::var("DISPLAY").ok(),
            xdg_current_desktop: std::env::var("XDG_CURRENT_DESKTOP").ok(),
            portal_screencast_available,
            portal_version,
        }
    }

    /// Returns `true` if the environment is an active Wayland session with a working system portal chooser.
    #[must_use]
    pub fn has_wayland_portal_chooser(&self) -> bool {
        let backend = detect_backend_from_env(
            self.xdg_session_type.as_deref(),
            self.wayland_display.as_deref(),
            self.x11_display.as_deref(),
        );
        backend == DesktopBackend::PipeWirePortal
            && self.portal_screencast_available
            && self.portal_version >= 1
    }
}

/// Decision determining whether the capture service should bypass the redundant in-app custom picker
/// and delegate directly to `xdg-desktop-portal`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PickerFlowDecision {
    /// Bypass the custom in-app picker and delegate directly to `xdg-desktop-portal` (`R5-06`).
    DelegateDirectlyToWaylandPortal {
        compositor: WaylandCompositorKind,
        portal_version: u32,
    },
    /// Use the custom in-app source picker (for X11 MIT-SHM or environments without a portal chooser).
    UseCustomPicker { backend: DesktopBackend },
}

impl PickerFlowDecision {
    /// Returns `true` when the custom in-app picker must be bypassed to prevent dual picker prompts.
    #[must_use]
    pub const fn bypasses_custom_picker(self) -> bool {
        matches!(self, Self::DelegateDirectlyToWaylandPortal { .. })
    }
}

/// Evaluates the environment probe and decides whether to bypass the custom in-app picker
/// and delegate directly to `xdg-desktop-portal`.
#[must_use]
pub fn evaluate_picker_flow(probe: &WaylandEnvironmentProbe) -> PickerFlowDecision {
    if probe.has_wayland_portal_chooser() {
        return PickerFlowDecision::DelegateDirectlyToWaylandPortal {
            compositor: WaylandCompositorKind::from_xdg_desktop(
                probe.xdg_current_desktop.as_deref(),
            ),
            portal_version: probe.portal_version,
        };
    }

    let is_x11 = probe
        .x11_display
        .as_deref()
        .is_some_and(|d| !d.trim().is_empty())
        || probe
            .xdg_session_type
            .as_deref()
            .is_some_and(|s| s.trim().eq_ignore_ascii_case("x11"));

    PickerFlowDecision::UseCustomPicker {
        backend: if is_x11 {
            DesktopBackend::X11Shm
        } else {
            DesktopBackend::SyntheticFallback
        },
    }
}

/// Integer-milli display scale factor (`1000` = `1.0x`, `1250` = `1.25x`, `1500` = `1.5x`, `2000` = `2.0x`)
/// for reconstructing physical native pixel dimensions on fractional HiDPI Wayland outputs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayScaleFactor(u32);

impl DisplayScaleFactor {
    /// Standard `1.0x` (`1000` milli) scale factor.
    pub const UNITY: Self = Self(UNITY_SCALE_FACTOR_MILLI);

    /// Validates and constructs a [`DisplayScaleFactor`] from milli-units (`500..=4000`).
    pub fn from_milli(milli: u32) -> Result<Self, CaptureError> {
        if !(MIN_SCALE_FACTOR_MILLI..=MAX_SCALE_FACTOR_MILLI).contains(&milli) {
            return Err(CaptureError::InvalidDimensions {
                width: milli,
                height: UNITY_SCALE_FACTOR_MILLI,
            });
        }
        Ok(Self(milli))
    }

    /// Returns the scale factor in milli-units (`1000` = `1.0x`).
    #[must_use]
    pub const fn milli(self) -> u32 {
        self.0
    }

    /// Scales a logical pixel dimension to physical native pixels using rounded integer arithmetic.
    #[must_use]
    pub const fn scale_dimension(self, logical: u32) -> u32 {
        if self.0 == UNITY_SCALE_FACTOR_MILLI {
            return logical;
        }
        ((logical as u64 * self.0 as u64 + 500) / 1000) as u32
    }
}

impl Default for DisplayScaleFactor {
    fn default() -> Self {
        Self::UNITY
    }
}

/// Resolved native display or window resolution for a Wayland portal stream.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NativeDisplayResolution {
    pub physical_width: u32,
    pub physical_height: u32,
    pub logical_width: u32,
    pub logical_height: u32,
    pub scale_factor_milli: u32,
    pub encoder_width: u32,
    pub encoder_height: u32,
    pub is_native_preserved: bool,
}

/// Resolves and validates the full native display resolution from the portal stream metadata,
/// optional PipeWire `SpaBuffer` physical size override, and Wayland HiDPI scale factor.
pub fn resolve_native_display_resolution(
    portal_logical_size: Option<(u32, u32)>,
    pipewire_physical_override: Option<(u32, u32)>,
    scale_factor: DisplayScaleFactor,
    preserve_full_native: bool,
    fallback_or_cap: (u32, u32),
) -> Result<NativeDisplayResolution, CaptureError> {
    let (logical_w, logical_h) = portal_logical_size
        .or(pipewire_physical_override)
        .unwrap_or(fallback_or_cap);
    validate_dimension_bounds(logical_w, logical_h)?;

    let (raw_phys_w, raw_phys_h) = compute_physical_dimensions(
        logical_w,
        logical_h,
        pipewire_physical_override,
        scale_factor,
    )?;

    let (final_phys_w, final_phys_h, is_native_preserved) = if preserve_full_native {
        (raw_phys_w, raw_phys_h, true)
    } else {
        let cap_w = fallback_or_cap.0.max(MIN_NATIVE_CAPTURE_DIMENSION);
        let cap_h = fallback_or_cap.1.max(MIN_NATIVE_CAPTURE_DIMENSION);
        let cw = raw_phys_w.min(cap_w);
        let ch = raw_phys_h.min(cap_h);
        (cw, ch, cw == raw_phys_w && ch == raw_phys_h)
    };

    Ok(NativeDisplayResolution {
        physical_width: final_phys_w,
        physical_height: final_phys_h,
        logical_width: logical_w,
        logical_height: logical_h,
        scale_factor_milli: scale_factor.milli(),
        encoder_width: (final_phys_w & !1).max(MIN_NATIVE_CAPTURE_DIMENSION),
        encoder_height: (final_phys_h & !1).max(MIN_NATIVE_CAPTURE_DIMENSION),
        is_native_preserved,
    })
}

fn compute_physical_dimensions(
    logical_w: u32,
    logical_h: u32,
    pipewire_physical_override: Option<(u32, u32)>,
    scale_factor: DisplayScaleFactor,
) -> Result<(u32, u32), CaptureError> {
    let (phys_w, phys_h) = match pipewire_physical_override {
        Some((pw_w, pw_h)) => (pw_w, pw_h),
        None => (
            scale_factor.scale_dimension(logical_w),
            scale_factor.scale_dimension(logical_h),
        ),
    };
    validate_dimension_bounds(phys_w, phys_h)?;
    Ok((phys_w, phys_h))
}

fn validate_dimension_bounds(width: u32, height: u32) -> Result<(), CaptureError> {
    if !(MIN_NATIVE_CAPTURE_DIMENSION..=MAX_CAPTURE_DIMENSION).contains(&width)
        || !(MIN_NATIVE_CAPTURE_DIMENSION..=MAX_CAPTURE_DIMENSION).contains(&height)
    {
        return Err(CaptureError::InvalidDimensions { width, height });
    }
    Ok(())
}

/// Configuration for initiating a Wayland ScreenCast Portal stream (`R5-06`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaylandPortalConfig {
    pub handle_token: String,
    pub session_handle_token: String,
    pub source_types: u32,
    pub multiple: bool,
    pub cursor_mode: PortalCursorMode,
    pub persist_mode: PortalPersistMode,
    pub restore_token: Option<String>,
    pub parent_window: String,
    pub bypass_custom_picker_on_wayland: bool,
    pub preserve_full_native_resolution: bool,
    pub display_scale_factor: DisplayScaleFactor,
    pub fallback_resolution: (u32, u32),
}

impl Default for WaylandPortalConfig {
    fn default() -> Self {
        Self {
            handle_token: "chord_wayland_portal_req".to_string(),
            session_handle_token: "chord_wayland_portal_sess".to_string(),
            source_types: SOURCE_TYPE_MONITOR | SOURCE_TYPE_WINDOW,
            multiple: false,
            cursor_mode: PortalCursorMode::Embedded,
            persist_mode: PortalPersistMode::ApplicationRunning,
            restore_token: None,
            parent_window: String::new(),
            bypass_custom_picker_on_wayland: true,
            preserve_full_native_resolution: true,
            display_scale_factor: DisplayScaleFactor::UNITY,
            fallback_resolution: DEFAULT_FALLBACK_NATIVE_RESOLUTION,
        }
    }
}

impl WaylandPortalConfig {
    /// Converts into the underlying [`PortalSessionConfig`] with an optional cached restore token override.
    #[must_use]
    pub fn to_portal_session_config(
        &self,
        effective_restore_token: Option<String>,
    ) -> PortalSessionConfig {
        PortalSessionConfig {
            handle_token: self.handle_token.clone(),
            session_handle_token: self.session_handle_token.clone(),
            source_types: self.source_types,
            multiple: self.multiple,
            cursor_mode: self.cursor_mode,
            persist_mode: self.persist_mode,
            restore_token: effective_restore_token,
            parent_window: self.parent_window.clone(),
        }
    }
}

/// Lifecycle state of the Wayland ScreenCast Portal delegation service.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WaylandPortalDelegationState {
    Idle,
    EvaluatingEnvironment,
    AwaitingNativePortalDialog,
    StreamingNativeResolution {
        node_id: u32,
        width: u32,
        height: u32,
    },
    Closed,
}

/// Outcome returned when a stream initiates via `WaylandPortalCaptureService`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaylandPortalSessionOutcome {
    pub session_handle: String,
    pub pipewire_node_id: u32,
    pub pipewire_fd: i32,
    pub custom_picker_bypassed: bool,
    pub custom_picker_prompts: u32,
    pub native_portal_prompts: u32,
    pub used_restore_token: bool,
    pub restore_token: Option<String>,
    pub native_resolution: NativeDisplayResolution,
    pub streams: Vec<PortalStream>,
    pub desktop_sources: Vec<DesktopSource>,
}

impl WaylandPortalSessionOutcome {
    /// Total number of picker dialogs presented to the user (`<= 1`, never `2` dual prompts).
    #[must_use]
    pub const fn total_user_picker_prompts(&self) -> u32 {
        self.custom_picker_prompts + self.native_portal_prompts
    }
}

/// Observable telemetry snapshot for the Wayland ScreenCast Portal capture service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WaylandPortalTelemetry {
    pub wayland_direct_delegations: u64,
    pub custom_picker_bypassed_count: u64,
    pub custom_picker_invocations: u64,
    pub native_portal_dialog_prompts: u64,
    pub restore_token_silent_starts: u64,
    pub native_resolution_preserved_count: u64,
    pub portal_errors_count: u64,
    pub total_fds_opened: u64,
    pub total_fds_closed: u64,
    pub last_native_width: Option<u32>,
    pub last_native_height: Option<u32>,
    pub redacted_restore_token: String,
}

impl WaylandPortalTelemetry {
    /// Returns the number of currently open PipeWire file descriptors (`0` when closed).
    #[must_use]
    pub const fn active_fd_balance(&self) -> u64 {
        self.total_fds_opened.saturating_sub(self.total_fds_closed)
    }
}

fn redact_token(token: Option<&str>) -> String {
    match token {
        Some(t) if !t.trim().is_empty() => "[REDACTED]".to_string(),
        _ => "<none>".to_string(),
    }
}

/// Capture service coordinating Wayland direct portal delegation, custom picker bypass,
/// and full native display resolution preservation (`R5-06`).
#[derive(Debug)]
pub struct WaylandPortalCaptureService<T: PortalTransport> {
    transport: Option<T>,
    active_client: Option<PortalScreenCastClient<T>>,
    probe: WaylandEnvironmentProbe,
    state: WaylandPortalDelegationState,
    active_config: Option<WaylandPortalConfig>,
    active_pw_config: Option<PipeWireConfig>,
    active_resolution: Option<NativeDisplayResolution>,
    saved_restore_token: Option<String>,
    wayland_direct_delegations: u64,
    custom_picker_bypassed_count: u64,
    custom_picker_invocations: u64,
    native_portal_dialog_prompts: u64,
    restore_token_silent_starts: u64,
    native_resolution_preserved_count: u64,
    portal_errors_count: u64,
    total_fds_opened: u64,
    total_fds_closed: u64,
}

impl<T: PortalTransport> WaylandPortalCaptureService<T> {
    /// Creates a new Wayland portal capture service for the given transport and environment probe.
    #[must_use]
    pub const fn new(transport: T, probe: WaylandEnvironmentProbe) -> Self {
        Self {
            transport: Some(transport),
            active_client: None,
            probe,
            state: WaylandPortalDelegationState::Idle,
            active_config: None,
            active_pw_config: None,
            active_resolution: None,
            saved_restore_token: None,
            wayland_direct_delegations: 0,
            custom_picker_bypassed_count: 0,
            custom_picker_invocations: 0,
            native_portal_dialog_prompts: 0,
            restore_token_silent_starts: 0,
            native_resolution_preserved_count: 0,
            portal_errors_count: 0,
            total_fds_opened: 0,
            total_fds_closed: 0,
        }
    }

    #[must_use]
    pub const fn state(&self) -> WaylandPortalDelegationState {
        self.state
    }

    #[must_use]
    pub fn picker_flow_decision(&self) -> PickerFlowDecision {
        evaluate_picker_flow(&self.probe)
    }

    #[must_use]
    pub fn saved_restore_token(&self) -> Option<&str> {
        self.saved_restore_token.as_deref()
    }

    #[must_use]
    pub const fn active_native_resolution(&self) -> Option<NativeDisplayResolution> {
        self.active_resolution
    }

    /// Guards against invoking the redundant custom picker when Wayland direct portal delegation is active.
    pub fn guard_custom_picker_invocation(&mut self) -> Result<DesktopBackend, CaptureError> {
        match self.picker_flow_decision() {
            PickerFlowDecision::DelegateDirectlyToWaylandPortal { compositor, .. } => Err(
                CaptureError::DbusError(format!(
                    "custom picker is bypassed on Wayland ({}) — delegate directly to xdg-desktop-portal",
                    compositor.portal_backend_name()
                )),
            ),
            PickerFlowDecision::UseCustomPicker { backend } => {
                self.custom_picker_invocations += 1;
                Ok(backend)
            }
        }
    }

    /// Initiates screen/window capture: on Wayland with a portal chooser, bypasses the custom picker
    /// and delegates directly to `xdg-desktop-portal` (`CreateSession -> SelectSources -> Start -> OpenPipeWireRemote`),
    /// preserving the full native display resolution.
    pub fn initiate_stream(
        &mut self,
        config: &WaylandPortalConfig,
    ) -> Result<WaylandPortalSessionOutcome, CaptureError> {
        self.ensure_can_initiate()?;
        self.state = WaylandPortalDelegationState::EvaluatingEnvironment;

        let effective_restore = config
            .restore_token
            .clone()
            .or_else(|| self.saved_restore_token.clone());
        let used_restore_token = effective_restore
            .as_ref()
            .is_some_and(|tok| !tok.trim().is_empty());

        let session_cfg = config.to_portal_session_config(effective_restore);
        session_cfg.validate()?;

        let custom_picker_bypassed = self.record_picker_flow_telemetry(config);
        let pw_config = self.execute_portal_handshake(&session_cfg)?;
        let native_resolution = self.resolve_session_resolution(&pw_config, config)?;

        Ok(self.finalize_session_outcome(
            config,
            pw_config,
            native_resolution,
            custom_picker_bypassed,
            used_restore_token,
        ))
    }

    fn ensure_can_initiate(&self) -> Result<(), CaptureError> {
        if matches!(
            self.state,
            WaylandPortalDelegationState::StreamingNativeResolution { .. }
                | WaylandPortalDelegationState::AwaitingNativePortalDialog
        ) {
            return Err(CaptureError::InvalidSessionState {
                expected: "Idle or Closed",
                actual: PortalSessionState::Streaming,
            });
        }
        Ok(())
    }

    fn record_picker_flow_telemetry(&mut self, config: &WaylandPortalConfig) -> bool {
        let bypass = config.bypass_custom_picker_on_wayland
            && self.picker_flow_decision().bypasses_custom_picker();
        if bypass {
            self.wayland_direct_delegations += 1;
            self.custom_picker_bypassed_count += 1;
        } else {
            self.custom_picker_invocations += 1;
        }
        bypass
    }

    fn execute_portal_handshake(
        &mut self,
        session_cfg: &PortalSessionConfig,
    ) -> Result<PipeWireConfig, CaptureError> {
        let transport = self.take_transport_for_new_session()?;
        let mut client = PortalScreenCastClient::new(transport);
        self.state = WaylandPortalDelegationState::AwaitingNativePortalDialog;

        let res = (|| -> Result<PipeWireConfig, CaptureError> {
            client.create_session(session_cfg)?;
            client.select_sources(session_cfg)?;
            client.start_and_open_remote(session_cfg)
        })();

        match res {
            Ok(pw) => {
                self.total_fds_opened += 1;
                self.active_client = Some(client);
                Ok(pw)
            }
            Err(err) => {
                let _ = client.close();
                self.portal_errors_count += 1;
                self.state = WaylandPortalDelegationState::Closed;
                self.active_client = Some(client);
                Err(err)
            }
        }
    }

    fn resolve_session_resolution(
        &mut self,
        pw_config: &PipeWireConfig,
        config: &WaylandPortalConfig,
    ) -> Result<NativeDisplayResolution, CaptureError> {
        let primary = pw_config
            .streams
            .first()
            .ok_or(CaptureError::NoStreamsReturned)?;

        match resolve_native_display_resolution(
            primary.size,
            None,
            config.display_scale_factor,
            config.preserve_full_native_resolution,
            config.fallback_resolution,
        ) {
            Ok(res) => {
                if res.is_native_preserved {
                    self.native_resolution_preserved_count += 1;
                }
                Ok(res)
            }
            Err(err) => {
                if let Some(ref mut client) = self.active_client {
                    let _ = client.close();
                }
                self.total_fds_closed += 1;
                self.portal_errors_count += 1;
                self.state = WaylandPortalDelegationState::Closed;
                Err(err)
            }
        }
    }

    fn finalize_session_outcome(
        &mut self,
        config: &WaylandPortalConfig,
        pw_config: PipeWireConfig,
        native_resolution: NativeDisplayResolution,
        custom_picker_bypassed: bool,
        used_restore_token: bool,
    ) -> WaylandPortalSessionOutcome {
        let native_portal_prompts = if used_restore_token {
            self.restore_token_silent_starts += 1;
            0
        } else {
            self.native_portal_dialog_prompts += 1;
            1
        };

        if let Some(ref tok) = pw_config.restore_token
            && !tok.trim().is_empty()
        {
            self.saved_restore_token = Some(tok.clone());
        }

        let desktop_sources =
            ScreenCapturer::from_portal_streams(&pw_config.streams).enumerate_sources();
        let outcome = WaylandPortalSessionOutcome {
            session_handle: pw_config.session_handle.clone(),
            pipewire_node_id: pw_config.node_id,
            pipewire_fd: pw_config.fd,
            custom_picker_bypassed,
            custom_picker_prompts: u32::from(!custom_picker_bypassed),
            native_portal_prompts,
            used_restore_token,
            restore_token: pw_config.restore_token.clone(),
            native_resolution,
            streams: pw_config.streams.clone(),
            desktop_sources,
        };

        self.state = WaylandPortalDelegationState::StreamingNativeResolution {
            node_id: pw_config.node_id,
            width: native_resolution.physical_width,
            height: native_resolution.physical_height,
        };
        self.active_config = Some(config.clone());
        self.active_pw_config = Some(pw_config);
        self.active_resolution = Some(native_resolution);
        outcome
    }

    /// Dynamically updates the active stream's native resolution when the first PipeWire `SpaBuffer`
    /// arrives (or on live output mode change), preserving full physical dimensions.
    pub fn notify_pipewire_buffer_dimensions(
        &mut self,
        buffer_width: u32,
        buffer_height: u32,
    ) -> Result<NativeDisplayResolution, CaptureError> {
        let WaylandPortalDelegationState::StreamingNativeResolution { node_id, .. } = self.state
        else {
            return Err(CaptureError::InvalidSessionState {
                expected: "StreamingNativeResolution",
                actual: PortalSessionState::Unconnected,
            });
        };

        let cfg = self.active_config.clone().unwrap_or_default();
        let portal_size = self
            .active_pw_config
            .as_ref()
            .and_then(|pw| pw.streams.first())
            .and_then(|s| s.size);

        let updated = resolve_native_display_resolution(
            portal_size,
            Some((buffer_width, buffer_height)),
            cfg.display_scale_factor,
            cfg.preserve_full_native_resolution,
            cfg.fallback_resolution,
        )?;

        self.active_resolution = Some(updated);
        self.state = WaylandPortalDelegationState::StreamingNativeResolution {
            node_id,
            width: updated.physical_width,
            height: updated.physical_height,
        };
        Ok(updated)
    }

    /// Captures a frame at the active stream's preserved native encoder resolution.
    pub fn capture_native_test_frame(
        &self,
        layout: PixelLayout,
        timestamp_us: u64,
    ) -> Result<VideoFrame, CaptureError> {
        let WaylandPortalDelegationState::StreamingNativeResolution { node_id, .. } = self.state
        else {
            return Err(CaptureError::InvalidSessionState {
                expected: "StreamingNativeResolution",
                actual: PortalSessionState::Unconnected,
            });
        };
        let res = self
            .active_resolution
            .ok_or(CaptureError::NoStreamsReturned)?;

        let byte_len =
            layout
                .byte_len(res.encoder_width, res.encoder_height)
                .ok_or(CaptureError::InvalidDimensions {
                    width: res.encoder_width,
                    height: res.encoder_height,
                })?;
        let fill_byte = ((node_id & 0xFF) as u8).wrapping_add(16);
        let data = vec![fill_byte; byte_len];
        Ok(
            VideoFrame::new(res.encoder_width, res.encoder_height, layout, data)
                .with_timestamp_us(timestamp_us),
        )
    }

    /// Stops the active Wayland portal stream, closing the D-Bus portal session and releasing the PipeWire FD.
    pub fn stop_stream(&mut self) -> Result<(), CaptureError> {
        let was_streaming = matches!(
            self.state,
            WaylandPortalDelegationState::StreamingNativeResolution { .. }
        );
        let result = if let Some(ref mut client) = self.active_client {
            client.close()
        } else {
            Ok(())
        };
        if was_streaming {
            self.total_fds_closed += 1;
        }
        self.state = WaylandPortalDelegationState::Closed;
        self.active_pw_config = None;
        result
    }

    /// Returns an observable telemetry snapshot for this capture service.
    #[must_use]
    pub fn telemetry(&self) -> WaylandPortalTelemetry {
        WaylandPortalTelemetry {
            wayland_direct_delegations: self.wayland_direct_delegations,
            custom_picker_bypassed_count: self.custom_picker_bypassed_count,
            custom_picker_invocations: self.custom_picker_invocations,
            native_portal_dialog_prompts: self.native_portal_dialog_prompts,
            restore_token_silent_starts: self.restore_token_silent_starts,
            native_resolution_preserved_count: self.native_resolution_preserved_count,
            portal_errors_count: self.portal_errors_count,
            total_fds_opened: self.total_fds_opened,
            total_fds_closed: self.total_fds_closed,
            last_native_width: self.active_resolution.map(|r| r.physical_width),
            last_native_height: self.active_resolution.map(|r| r.physical_height),
            redacted_restore_token: redact_token(self.saved_restore_token.as_deref()),
        }
    }

    fn take_transport_for_new_session(&mut self) -> Result<T, CaptureError> {
        if let Some(t) = self.transport.take() {
            return Ok(t);
        }
        if let Some(prev_client) = self.active_client.take() {
            return Ok(prev_client.into_transport());
        }
        Err(CaptureError::InvalidSessionState {
            expected: "Available transport",
            actual: PortalSessionState::Closed,
        })
    }
}
