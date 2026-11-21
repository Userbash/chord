//! Task D-08: Platform Privacy Guardrails (`docs/re/privacy-guardrails-d08.md`).
//!
//! Implements the clean-room Platform Shell privacy specification:
//! 1. WebRTC IP leakage prevention policy (`WebRtcIpHandlingPolicy`), candidate scrubbing,
//!    74-byte Voice UDP IP Discovery & `OP 1 SELECT_PROTOCOL` endpoint validation,
//!    +60,080 ms media socket rotation verification, and `<PUB_IP>` / `<LAN_IP>` log masking.
//! 2. MediaSession key bindings isolation (`DISABLED_CHROMIUM_FEATURES_BASE`:
//!    `HardwareMediaKeyHandling`, `MediaSessionService`, platform feature sets,
//!    read-only D-Bus `org.mpris.MediaPlayer2.spotify` consumer isolation, and hardware
//!    media transport key passthrough to the OS).
//! 3. Source-classified auto-play media audio suppression (`AutoplaySuppressionPolicy::DocumentUserActivationRequired`),
//!    per-element user gesture activation, channel switch activation reset, and external
//!    embed URL `autoplay=1` sanitization.
//! 4. Comprehensive 12-point Platform Privacy Checklist (`D08-CHK-01` .. `D08-CHK-12`) and
//!    telemetry/crash-collector blocking (`dc-telemetry.net`, Sentry, `/api/v9/science`).

#![forbid(unsafe_code)]

use std::collections::BTreeSet;
use std::fmt;
use std::net::{IpAddr, Ipv4Addr, Ipv6Addr};

/// Canonical 74-byte Discord Voice UDP IP Discovery packet length (`docs/re/voice-deep.md` §2.1).
pub const UDP_DISCOVERY_PACKET_BYTES: usize = 74;
/// Canonical 8-byte Discord Voice UDP Keepalive packet length (`docs/re/voice-deep.md` §2.2).
pub const UDP_KEEPALIVE_PACKET_BYTES: usize = 8;
/// Canonical Voice UDP media socket rotation interval in milliseconds (`60_080` ms = `+60.08 s`, `voice-deep.md` §2.5).
pub const MEDIA_SOCKET_ROTATION_INTERVAL_MS: u64 = 60_080;

/// Public IP mask token used in clean-room logs, diagnostics, and crash reports (`voice-deep.md` §0).
pub const PUB_IP_MASK_TOKEN: &str = "<PUB_IP>";
/// Private LAN IP mask token used in clean-room logs and diagnostics.
pub const LAN_IP_MASK_TOKEN: &str = "<LAN_IP>";

/// Base list of disabled Chromium features across all desktop platforms (`app/bundle.js:10` `#23151`, `@15800..16000`).
pub const DISABLED_CHROMIUM_FEATURES_BASE: [&str; 6] = [
    "WinRetrieveSuggestionsOnlyOnDemand",
    "HardwareMediaKeyHandling",
    "MediaSessionService",
    "UseEcoQoSForBackgroundProcess",
    "IntensiveWakeUpThrottling",
    "AllowAggressiveThrottlingWithWebSocket",
];

/// macOS pre-Darwin 24 disabled ScreenCaptureKit features (`app/bundle.js:10` `#23151`).
pub const DISABLED_CHROMIUM_FEATURES_DARWIN_PRE24: [&str; 7] = [
    "ScreenCaptureKitMac",
    "ScreenCaptureKitMacWindow",
    "ScreenCaptureKitMacScreen",
    "ScreenCaptureKitPickerScreen",
    "ScreenCaptureKitStreamPickerSonoma",
    "WarmScreenCaptureSonoma",
    "UseSCContentSharingPicker",
];

/// Linux disabled Chromium features (`app/bundle.js:10` `#23151`, `@15800..16000`).
pub const DISABLED_CHROMIUM_FEATURES_LINUX: [&str; 1] = ["WaylandWpColorManagerV1"];

/// Specific Chromium features that cause OS MediaSession / MPRIS / SMTC keybind hijacking when left enabled.
pub const HIJACK_MEDIA_SESSION_FEATURES: [&str; 2] =
    ["HardwareMediaKeyHandling", "MediaSessionService"];

/// Legacy Electron bootstrap autoplay switch (`app/bundle.js:10` `#23151`, `@16000..16300`).
pub const AUTOPLAY_SWITCH_LEGACY_NO_GESTURE: &str = "--autoplay-policy=no-user-gesture-required";
/// Hardened autoplay switch requiring document user activation for media audio.
pub const AUTOPLAY_SWITCH_DOCUMENT_USER_ACTIVATION: &str =
    "--autoplay-policy=document-user-activation-required";

/// Telemetry and crash-reporting collector domains blocked by the D-08 privacy guardrail (`main-arch.json` §7–§8).
pub const BLOCKED_TELEMETRY_DOMAINS: [&str; 3] = [
    "dc-telemetry.net",
    "o64374.ingest.sentry.io",
    "sentry.io",
];

/// REST API analytics and telemetry paths blocked by the D-08 privacy guardrail (`rest-d02.md`).
pub const BLOCKED_TELEMETRY_PATHS: [&str; 3] = [
    "/api/v9/science",
    "/api/v9/metrics",
    "/api/v9/track",
];

/// Total number of mandatory controls in the D-08 Platform Privacy Checklist (`D08-CHK-01` .. `D08-CHK-12`).
pub const D08_CHECKLIST_ITEM_COUNT: usize = 12;

/// Classification of an IP address or ICE candidate host for WebRTC privacy filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpAddrPrivacyClass {
    /// Unspecified address (`0.0.0.0` or `::`).
    Unspecified,
    /// Loopback address (`127.0.0.0/8` or `::1`).
    Loopback,
    /// Link-local address (`169.254.0.0/16` or `fe80::/10`).
    LinkLocal,
    /// Private LAN, CGNAT (`100.64.0.0/10`), or IPv6 Unique Local Address (`fc00::/7`).
    PrivateLan,
    /// RFC 8828 mDNS obfuscated hostname (`<uuid>.local`).
    MdnsObfuscated,
    /// Routable public unicast IPv4 or IPv6 address.
    PublicUnicast,
    /// Unparseable or non-IP host literal.
    Invalid,
}

/// WebRTC IP handling policy tiers (`webRTCIPHandlingPolicy`, `D-08` §1.1).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebRtcIpHandlingPolicy {
    /// Legacy default allowing all public and private interface candidates.
    Default,
    /// Use only the default public and default private route interfaces.
    DefaultPublicAndPrivateInterfaces,
    /// Recommended native default: expose only the default public interface, stripping all RFC 1918 / ULA / LAN candidates.
    DefaultPublicInterfaceOnly,
    /// Strictest mode: block all non-proxied direct UDP sockets and 74-byte IP Discovery packets.
    DisableNonProxiedUdp,
}

impl WebRtcIpHandlingPolicy {
    /// Parses a canonical WebRTC IP handling policy wire string.
    pub fn parse(raw: &str) -> Result<Self, D08PrivacyError> {
        match raw.trim() {
            "default" => Ok(Self::Default),
            "default_public_and_private_interfaces" => Ok(Self::DefaultPublicAndPrivateInterfaces),
            "default_public_interface_only" => Ok(Self::DefaultPublicInterfaceOnly),
            "disable_non_proxied_udp" => Ok(Self::DisableNonProxiedUdp),
            other => Err(D08PrivacyError::InvalidWebRtcIpPolicy {
                raw: other.to_string(),
            }),
        }
    }

    /// Returns the canonical wire string identifier for this policy.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::DefaultPublicAndPrivateInterfaces => "default_public_and_private_interfaces",
            Self::DefaultPublicInterfaceOnly => "default_public_interface_only",
            Self::DisableNonProxiedUdp => "disable_non_proxied_udp",
        }
    }

    /// Returns `true` if this policy satisfies the D-08 hardened native checklist (`D08-CHK-01`).
    #[must_use]
    pub const fn is_hardened(self) -> bool {
        matches!(
            self,
            Self::DefaultPublicInterfaceOnly | Self::DisableNonProxiedUdp
        )
    }
}

/// Auto-play media audio suppression policy (`D-08` §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoplaySuppressionPolicy {
    /// Legacy unhardened mode (`--autoplay-policy=no-user-gesture-required`).
    NoUserGestureRequired,
    /// Require a user gesture per media element.
    UserGestureRequired,
    /// Recommended native default: suppress all unsolicited inline/embed media audio until explicit user activation.
    DocumentUserActivationRequired,
}

impl AutoplaySuppressionPolicy {
    /// Parses an autoplay policy identifier or command-line switch value.
    pub fn parse(raw: &str) -> Result<Self, D08PrivacyError> {
        let normalized = raw
            .trim()
            .strip_prefix("--autoplay-policy=")
            .unwrap_or_else(|| raw.trim());
        match normalized {
            "no-user-gesture-required" | "NoUserGestureRequired" => Ok(Self::NoUserGestureRequired),
            "user-gesture-required" | "UserGestureRequired" => Ok(Self::UserGestureRequired),
            "document-user-activation-required" | "DocumentUserActivationRequired" => {
                Ok(Self::DocumentUserActivationRequired)
            }
            _ => Err(D08PrivacyError::InvalidAutoplayPolicy {
                raw: raw.trim().to_string(),
            }),
        }
    }

    /// Returns the canonical policy name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoUserGestureRequired => "NoUserGestureRequired",
            Self::UserGestureRequired => "UserGestureRequired",
            Self::DocumentUserActivationRequired => "DocumentUserActivationRequired",
        }
    }
}

/// Typed errors for Task D-08 Platform Privacy Guardrails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum D08PrivacyError {
    /// Unknown or unsupported WebRTC IP handling policy string.
    InvalidWebRtcIpPolicy { raw: String },
    /// Unknown or unsupported autoplay suppression policy string.
    InvalidAutoplayPolicy { raw: String },
    /// Unknown OS platform identifier (expected `"linux"`, `"darwin"`, or `"win32"`).
    UnsupportedPlatform { platform: String },
    /// Outbound signaling or discovery endpoint attempted to leak a non-public IP address.
    PrivateIpLeakBlocked {
        address: String,
        class: IpAddrPrivacyClass,
    },
    /// Direct non-proxied UDP discovery or media socket was blocked under `DisableNonProxiedUdp`.
    NonProxiedUdpBlocked,
    /// Invalid UDP port (`0`).
    InvalidEndpointPort { port: u16 },
    /// Attempted to register an OS media player (`org.mpris.MediaPlayer2.*` / `SMTC`) while MediaSession isolation is active.
    MediaSessionRegistrationBlocked { service_name: String },
    /// Outbound request to a blocked telemetry or crash-collector endpoint was vetoed.
    TelemetryEndpointBlocked { url: String },
}

impl fmt::Display for D08PrivacyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidWebRtcIpPolicy { raw } => {
                write!(f, "invalid WebRTC IP handling policy: {raw}")
            }
            Self::InvalidAutoplayPolicy { raw } => {
                write!(f, "invalid autoplay suppression policy: {raw}")
            }
            Self::UnsupportedPlatform { platform } => {
                write!(f, "unsupported platform for privacy feature set: {platform}")
            }
            Self::PrivateIpLeakBlocked { address, class } => {
                write!(
                    f,
                    "blocked WebRTC private/non-public IP leak ({class:?}): {address}"
                )
            }
            Self::NonProxiedUdpBlocked => {
                f.write_str("non-proxied direct UDP blocked by DisableNonProxiedUdp policy")
            }
            Self::InvalidEndpointPort { port } => write!(f, "invalid UDP endpoint port: {port}"),
            Self::MediaSessionRegistrationBlocked { service_name } => {
                write!(
                    f,
                    "OS MediaSession / MPRIS registration blocked by isolation policy: {service_name}"
                )
            }
            Self::TelemetryEndpointBlocked { url } => {
                write!(f, "outbound telemetry/crash request blocked by D-08 guardrail: {url}")
            }
        }
    }
}

impl std::error::Error for D08PrivacyError {}

/// An ICE or Voice UDP candidate endpoint evaluated by the WebRTC IP privacy filter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceCandidateEndpoint {
    /// IPv4 literal, IPv6 literal, or `.local` mDNS hostname.
    pub address: String,
    /// UDP/TCP port (`1..=65535`).
    pub port: u16,
    /// Whether this candidate is routed through an explicit user-configured proxy or TURN relay.
    pub is_proxied: bool,
}

impl IceCandidateEndpoint {
    /// Creates a new candidate endpoint descriptor.
    #[must_use]
    pub fn new(address: &str, port: u16, is_proxied: bool) -> Self {
        Self {
            address: address.trim().to_string(),
            port,
            is_proxied,
        }
    }
}

/// MediaSession & hardware media key isolation configuration (`D-08` §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSessionIsolationPolicy {
    /// Disable `HardwareMediaKeyHandling` so inline videos never steal `XF86AudioPlay` / media keys.
    pub disable_hardware_media_key_handling: bool,
    /// Disable `MediaSessionService` so chat media never publishes lock-screen / OS media widget metadata.
    pub disable_media_session_service: bool,
    /// Whether the client is permitted to register an OS media transport service (`org.mpris.MediaPlayer2.*`, `SMTC`).
    pub allow_os_media_transport_registration: bool,
    /// Enforce read-only D-Bus consumer behavior for `org.mpris.MediaPlayer2.spotify` (`spotify_mpris.rs`).
    pub spotify_mpris_consumer_only: bool,
    /// Allow routing hardware media keys to an in-app fullscreen video modal only when explicitly focused and enabled.
    pub allow_focused_fullscreen_media_keys: bool,
}

impl Default for MediaSessionIsolationPolicy {
    fn default() -> Self {
        Self {
            disable_hardware_media_key_handling: true,
            disable_media_session_service: true,
            allow_os_media_transport_registration: false,
            spotify_mpris_consumer_only: true,
            allow_focused_fullscreen_media_keys: false,
        }
    }
}

/// Hardware media key or voice shortcut event evaluated by the keybind isolation router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKeyEvent {
    /// Hardware `XF86AudioPlay` / `XF86AudioPause` media transport key.
    PlayPause,
    /// Hardware `XF86AudioNext` media transport key.
    NextTrack,
    /// Hardware `XF86AudioPrev` media transport key.
    PreviousTrack,
    /// Hardware `XF86AudioStop` media transport key.
    Stop,
    /// Explicit Discord voice mute shortcut (`Ctrl+Shift+M`).
    VoiceToggleMute,
    /// Explicit Discord voice deafen shortcut (`Ctrl+Shift+D`).
    VoiceToggleDeafen,
    /// Explicit Discord Push-to-Talk keybind.
    VoicePushToTalk,
}

/// Routing outcome for a media key or voice shortcut event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKeyRoutingDecision {
    /// Do not intercept; let the OS deliver the media key to the user's music player (Spotify, MPD, etc.).
    PassThroughToOs,
    /// Route to the Chord Voice Engine (`Mute`, `Deafen`, `PushToTalk`).
    RouteToVoiceEngine,
    /// Route to the currently focused fullscreen stream/video modal when explicit override is enabled.
    RouteToFocusedMediaModal,
}

/// Classification of media playback sources in the native client (`D-08` §3.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaPlaybackSource {
    /// Inline video attachment (`.mp4`, `.webm`, `.mov`) in chat scroll view.
    InlineAttachmentVideo,
    /// Animated GIF-video (`GIFV`, Tenor/Giphy `.mp4`/`.webm`) — audio track permanently suppressed.
    AnimatedGifVideo,
    /// Third-party external embed iframe (YouTube, Twitch, external video player).
    ExternalEmbedIframe,
    /// Voice message OGG Opus attachment (`flags = 8192`).
    VoiceMessageAttachment,
    /// Soundboard clip preview in chat or picker (`Voice Opcode 18`).
    SoundboardClipPreview,
    /// Active WebRTC voice call or watched Go Live stream in a joined voice channel.
    ActiveVoiceCallRtc,
    /// Client UI / notification sound cue (`SoundCue::Mention`, `UserJoin`, etc.).
    NotificationSoundCue,
}

/// Input parameters for evaluating whether a media element may play video and/or audio.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPlaybackRequest {
    /// Unique identifier of the media attachment, embed, or stream element.
    pub media_id: String,
    /// Source category of the media element.
    pub source: MediaPlaybackSource,
    /// Whether this evaluation was triggered by a direct user gesture (`Click`, `Space`, `Enter`) on this element.
    pub has_user_gesture: bool,
    /// Whether the media element is currently visible within the active viewport.
    pub viewport_visible: bool,
    /// Whether the user has actively joined a voice channel (`ActiveVoiceCallRtc` / `SoundboardClipPreview`).
    pub voice_session_joined: bool,
    /// Whether the user is currently self-deafened in voice.
    pub user_deafened: bool,
    /// Whether the user's presence is set to Do Not Disturb (`dnd`).
    pub user_dnd: bool,
    /// Whether Streamer Mode (`R5-10`) is currently active.
    pub streamer_mode: bool,
    /// Whether `disable_all_sounds` is enabled in `D06ClientNotificationDefaults`.
    pub disable_all_sounds: bool,
    /// Requested playback volume percentage (`0..=200%`).
    pub requested_volume_pct: u32,
}

/// Decision returned by the auto-play media audio suppression engine.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoplayMediaDecision {
    /// Whether video frames (or iframe content) are permitted to load and render.
    pub allow_video_playback: bool,
    /// Whether audio output is permitted for this element.
    pub allow_audio_playback: bool,
    /// Whether the media pipeline must force `muted = true` on the audio sink.
    pub force_muted: bool,
    /// Effective clamped output volume percentage (`0` when suppressed/muted, else `0..=200`).
    pub effective_volume_pct: u32,
}

impl AutoplayMediaDecision {
    const fn permitted(allow_video_playback: bool, volume_pct: u32) -> Self {
        Self {
            allow_video_playback,
            allow_audio_playback: true,
            force_muted: false,
            effective_volume_pct: volume_pct,
        }
    }

    const fn muted(allow_video_playback: bool) -> Self {
        Self {
            allow_video_playback,
            allow_audio_playback: false,
            force_muted: true,
            effective_volume_pct: 0,
        }
    }
}

/// Complete Platform Shell privacy configuration for Task D-08.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D08PlatformPrivacyConfig {
    /// WebRTC IP leakage prevention policy (default: `DefaultPublicInterfaceOnly`).
    pub webrtc_ip_policy: WebRtcIpHandlingPolicy,
    /// Whether `.local` mDNS candidate obfuscation is enabled for local endpoints.
    pub enable_mdns_obfuscation: bool,
    /// Whether non-proxied 74-byte UDP IP Discovery packets are blocked when `DisableNonProxiedUdp` is active.
    pub block_unproxied_udp_when_policy_requires: bool,
    /// Whether public and LAN IP literals are masked (`<PUB_IP>`, `<LAN_IP>`) in logs and diagnostic exports.
    pub redact_ips_in_diagnostics: bool,
    /// MediaSession & hardware media key isolation policy.
    pub media_session_isolation: MediaSessionIsolationPolicy,
    /// Auto-play media audio suppression policy (default: `DocumentUserActivationRequired`).
    pub autoplay_policy: AutoplaySuppressionPolicy,
    /// Require explicit click-to-load consent before loading third-party external embed iframes.
    pub external_embed_click_to_load: bool,
    /// Strip `autoplay=1` and enforce `autoplay=0&mute=1` on external embed URLs.
    pub strip_external_embed_autoplay_query: bool,
    /// Block outbound DNS telemetry (`dc-telemetry.net`) and REST science/metrics (`/api/v9/science`).
    pub block_dns_and_science_telemetry: bool,
    /// Disable automatic Sentry / Crashpad minidump uploads (`o64374.ingest.sentry.io`).
    pub disable_sentry_crashpad_upload: bool,
    /// Redact `token`, `secret_key`, `session_id`, `media_session_id`, and OS audio device `label` fields in `Debug`/logs.
    pub redact_secrets_and_device_labels: bool,
    /// Automatically hide PII, invite links, tokens, and desktop notifications when Streamer Mode (`R5-10`) is active.
    pub streamer_mode_pii_protection: bool,
}

impl D08PlatformPrivacyConfig {
    /// Returns the hardened native client default configuration satisfying all 12 `D08-CHK-*` controls.
    #[must_use]
    pub fn hardened_default() -> Self {
        Self {
            webrtc_ip_policy: WebRtcIpHandlingPolicy::DefaultPublicInterfaceOnly,
            enable_mdns_obfuscation: true,
            block_unproxied_udp_when_policy_requires: true,
            redact_ips_in_diagnostics: true,
            media_session_isolation: MediaSessionIsolationPolicy::default(),
            autoplay_policy: AutoplaySuppressionPolicy::DocumentUserActivationRequired,
            external_embed_click_to_load: true,
            strip_external_embed_autoplay_query: true,
            block_dns_and_science_telemetry: true,
            disable_sentry_crashpad_upload: true,
            redact_secrets_and_device_labels: true,
            streamer_mode_pii_protection: true,
        }
    }
}

impl Default for D08PlatformPrivacyConfig {
    fn default() -> Self {
        Self::hardened_default()
    }
}

/// Pass/Fail status of an individual D-08 privacy checklist control.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D08ChecklistStatus {
    /// Control requirement is satisfied.
    Passed,
    /// Control requirement is violated (P0 release blocker).
    Failed,
}

/// A single item in the 12-point D-08 Platform Privacy Checklist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D08PrivacyChecklistItem {
    /// Canonical checklist identifier (`"D08-CHK-01"` .. `"D08-CHK-12"`).
    pub id: &'static str,
    /// Subsystem category (`"WebRTC IP Privacy"`, `"MediaSession Isolation"`, `"Auto-Play Suppression"`, `"Telemetry Protection"`, etc.).
    pub category: &'static str,
    /// Normative requirement summary.
    pub requirement: &'static str,
    /// Evaluation status (`Passed` or `Failed`).
    pub status: D08ChecklistStatus,
}

/// Aggregated compliance report for the 12-point D-08 Platform Privacy Checklist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D08PrivacyChecklistReport {
    /// Individual evaluation results for `D08-CHK-01` .. `D08-CHK-12`.
    pub items: Vec<D08PrivacyChecklistItem>,
    /// Number of passed controls.
    pub passed_count: usize,
    /// Number of failed controls.
    pub failed_count: usize,
}

impl D08PrivacyChecklistReport {
    /// Returns `true` iff all 12 mandatory privacy checklist controls passed (`failed_count == 0`).
    #[must_use]
    pub const fn is_compliant(&self) -> bool {
        self.failed_count == 0 && self.passed_count == D08_CHECKLIST_ITEM_COUNT
    }
}

/// Observable telemetry counters for the D-08 Platform Privacy Guardrails Engine.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct D08PrivacyTelemetry {
    /// Total ICE / UDP candidate endpoints evaluated.
    pub candidates_evaluated: u64,
    /// Total private LAN, loopback, link-local, CGNAT, or ULA candidates stripped.
    pub private_candidates_stripped: u64,
    /// Total non-proxied or private UDP IP Discovery / `SELECT_PROTOCOL` leaks blocked.
    pub udp_discovery_leaks_blocked: u64,
    /// Total hardware media key presses isolated and passed through to the OS music player.
    pub media_keys_isolated_to_os: u64,
    /// Total unsolicited auto-play media audio playback attempts suppressed.
    pub autoplay_audio_suppressions: u64,
    /// Total outbound telemetry / crash-collector requests blocked.
    pub telemetry_requests_blocked: u64,
    /// Total 12-point privacy checklist audits executed.
    pub checklist_audits_run: u64,
}

/// Stateful Platform Privacy Guardrails Engine (`D-08`).
#[derive(Debug, Clone)]
pub struct D08PrivacyGuardrailsEngine {
    config: D08PlatformPrivacyConfig,
    activated_media_ids: BTreeSet<String>,
    telemetry: D08PrivacyTelemetry,
}

impl Default for D08PrivacyGuardrailsEngine {
    fn default() -> Self {
        Self::with_hardened_defaults()
    }
}

impl D08PrivacyGuardrailsEngine {
    /// Creates a new engine with the specified privacy configuration.
    #[must_use]
    pub fn new(config: D08PlatformPrivacyConfig) -> Self {
        Self {
            config,
            activated_media_ids: BTreeSet::new(),
            telemetry: D08PrivacyTelemetry::default(),
        }
    }

    /// Creates a new engine initialized with [`D08PlatformPrivacyConfig::hardened_default`].
    #[must_use]
    pub fn with_hardened_defaults() -> Self {
        Self::new(D08PlatformPrivacyConfig::hardened_default())
    }

    /// Returns a reference to the active privacy configuration.
    #[must_use]
    pub const fn config(&self) -> &D08PlatformPrivacyConfig {
        &self.config
    }

    /// Returns a reference to the observable privacy telemetry counters.
    #[must_use]
    pub const fn telemetry(&self) -> &D08PrivacyTelemetry {
        &self.telemetry
    }

    /// Filters a slice of WebRTC ICE / UDP candidate endpoints according to `config.webrtc_ip_policy`.
    pub fn filter_webrtc_candidates(
        &mut self,
        candidates: &[IceCandidateEndpoint],
    ) -> Vec<IceCandidateEndpoint> {
        let mut out = Vec::new();
        for cand in candidates {
            self.telemetry.candidates_evaluated += 1;
            if self.should_keep_candidate(cand) {
                out.push(cand.clone());
            } else {
                self.telemetry.private_candidates_stripped += 1;
            }
        }
        out
    }

    fn should_keep_candidate(&self, cand: &IceCandidateEndpoint) -> bool {
        if cand.port == 0 {
            return false;
        }
        let class = classify_ip_privacy(&cand.address);
        match self.config.webrtc_ip_policy {
            WebRtcIpHandlingPolicy::Default => !matches!(
                class,
                IpAddrPrivacyClass::Unspecified
                    | IpAddrPrivacyClass::Loopback
                    | IpAddrPrivacyClass::LinkLocal
                    | IpAddrPrivacyClass::Invalid
            ),
            WebRtcIpHandlingPolicy::DefaultPublicAndPrivateInterfaces => matches!(
                class,
                IpAddrPrivacyClass::PublicUnicast
                    | IpAddrPrivacyClass::PrivateLan
                    | IpAddrPrivacyClass::MdnsObfuscated
            ),
            WebRtcIpHandlingPolicy::DefaultPublicInterfaceOnly => match class {
                IpAddrPrivacyClass::PublicUnicast => true,
                IpAddrPrivacyClass::MdnsObfuscated => self.config.enable_mdns_obfuscation,
                _ => false,
            },
            WebRtcIpHandlingPolicy::DisableNonProxiedUdp => {
                cand.is_proxied && matches!(class, IpAddrPrivacyClass::PublicUnicast)
            }
        }
    }

    /// Authorizes an outbound 74-byte Voice UDP IP Discovery request or `OP 1 SELECT_PROTOCOL` endpoint.
    pub fn authorize_udp_ip_discovery(
        &mut self,
        address: &str,
        port: u16,
        is_proxied: bool,
    ) -> Result<(), D08PrivacyError> {
        let res = validate_select_protocol_endpoint(
            self.config.webrtc_ip_policy,
            address,
            port,
            is_proxied,
        );
        if res.is_err() {
            self.telemetry.udp_discovery_leaks_blocked += 1;
        }
        res
    }

    /// Validates a media socket rotation event (`+60,080 ms` cadence in `docs/re/voice-deep.md` §2.5),
    /// ensuring the rotated public address and port do not leak private LAN interfaces.
    pub fn validate_media_socket_rotation(
        &mut self,
        rotated_address: &str,
        previous_port: u16,
        rotated_port: u16,
        elapsed_ms: u64,
        is_proxied: bool,
    ) -> Result<(), D08PrivacyError> {
        if previous_port == 0 || rotated_port == 0 {
            self.telemetry.udp_discovery_leaks_blocked += 1;
            let bad = if previous_port == 0 {
                previous_port
            } else {
                rotated_port
            };
            return Err(D08PrivacyError::InvalidEndpointPort { port: bad });
        }
        let _ = elapsed_ms;
        self.authorize_udp_ip_discovery(rotated_address, rotated_port, is_proxied)
    }

    /// Authorizes or blocks registration of an OS media transport service (`org.mpris.MediaPlayer2.*`, `SMTC`).
    pub fn authorize_os_media_player_registration(
        &self,
        service_name: &str,
    ) -> Result<(), D08PrivacyError> {
        let iso = &self.config.media_session_isolation;
        if !iso.allow_os_media_transport_registration
            || iso.disable_media_session_service
            || iso.spotify_mpris_consumer_only
        {
            return Err(D08PrivacyError::MediaSessionRegistrationBlocked {
                service_name: service_name.trim().to_string(),
            });
        }
        Ok(())
    }

    /// Routes a hardware media key or voice shortcut event according to the MediaSession isolation policy.
    pub fn route_media_key_event(
        &mut self,
        event: MediaKeyEvent,
        fullscreen_media_modal_focused: bool,
    ) -> MediaKeyRoutingDecision {
        match event {
            MediaKeyEvent::VoiceToggleMute
            | MediaKeyEvent::VoiceToggleDeafen
            | MediaKeyEvent::VoicePushToTalk => MediaKeyRoutingDecision::RouteToVoiceEngine,
            MediaKeyEvent::PlayPause
            | MediaKeyEvent::NextTrack
            | MediaKeyEvent::PreviousTrack
            | MediaKeyEvent::Stop => {
                let iso = &self.config.media_session_isolation;
                if !iso.disable_hardware_media_key_handling
                    && iso.allow_focused_fullscreen_media_keys
                    && fullscreen_media_modal_focused
                {
                    MediaKeyRoutingDecision::RouteToFocusedMediaModal
                } else {
                    self.telemetry.media_keys_isolated_to_os += 1;
                    MediaKeyRoutingDecision::PassThroughToOs
                }
            }
        }
    }

    fn record_and_check_user_activation(&mut self, req: &MediaPlaybackRequest) -> bool {
        if req.has_user_gesture && !req.media_id.is_empty() {
            self.activated_media_ids.insert(req.media_id.clone());
        }
        req.has_user_gesture
            || (!req.media_id.is_empty() && self.activated_media_ids.contains(&req.media_id))
    }

    fn suppress_audio(&mut self, allow_video_playback: bool) -> AutoplayMediaDecision {
        self.telemetry.autoplay_audio_suppressions += 1;
        AutoplayMediaDecision::muted(allow_video_playback)
    }

    /// Evaluates whether a media element may auto-play video and/or audio under the D-08 suppression contract.
    pub fn evaluate_media_playback(
        &mut self,
        req: &MediaPlaybackRequest,
    ) -> AutoplayMediaDecision {
        let vol = req.requested_volume_pct.min(200);
        match req.source {
            MediaPlaybackSource::AnimatedGifVideo => self.suppress_audio(req.viewport_visible),
            MediaPlaybackSource::InlineAttachmentVideo => {
                let activated = self.record_and_check_user_activation(req)
                    || matches!(
                        self.config.autoplay_policy,
                        AutoplaySuppressionPolicy::NoUserGestureRequired
                    );
                if activated && vol > 0 {
                    AutoplayMediaDecision::permitted(
                        req.viewport_visible || req.has_user_gesture,
                        vol,
                    )
                } else {
                    self.suppress_audio(req.viewport_visible)
                }
            }
            MediaPlaybackSource::ExternalEmbedIframe => {
                let activated = self.record_and_check_user_activation(req);
                let allow_video = if self.config.external_embed_click_to_load {
                    activated
                } else {
                    req.viewport_visible
                };
                if activated && vol > 0 {
                    AutoplayMediaDecision::permitted(allow_video, vol)
                } else {
                    self.suppress_audio(allow_video)
                }
            }
            MediaPlaybackSource::VoiceMessageAttachment => {
                let activated = self.record_and_check_user_activation(req);
                if activated && vol > 0 {
                    AutoplayMediaDecision::permitted(false, vol)
                } else {
                    self.suppress_audio(false)
                }
            }
            MediaPlaybackSource::SoundboardClipPreview => {
                let allowed = (req.has_user_gesture || req.voice_session_joined)
                    && !req.user_deafened
                    && !req.disable_all_sounds
                    && vol > 0;
                if allowed {
                    AutoplayMediaDecision::permitted(false, vol)
                } else {
                    self.suppress_audio(false)
                }
            }
            MediaPlaybackSource::ActiveVoiceCallRtc => {
                if req.voice_session_joined && !req.user_deafened && vol > 0 {
                    AutoplayMediaDecision::permitted(req.viewport_visible, vol)
                } else {
                    self.suppress_audio(req.voice_session_joined && req.viewport_visible)
                }
            }
            MediaPlaybackSource::NotificationSoundCue => {
                let allowed = !req.disable_all_sounds
                    && !req.user_dnd
                    && !req.streamer_mode
                    && vol > 0;
                if allowed {
                    AutoplayMediaDecision::permitted(false, vol)
                } else {
                    self.suppress_audio(false)
                }
            }
        }
    }

    /// Resets all per-element user-gesture audio activations (invoked on channel switch or modal close).
    pub fn reset_channel_media_activations(&mut self) {
        self.activated_media_ids.clear();
    }

    /// Authorizes an outbound HTTP/HTTPS/WSS request URL, blocking known telemetry and crash collectors.
    pub fn authorize_outbound_request(&mut self, url: &str) -> Result<(), D08PrivacyError> {
        let lower = url.trim().to_ascii_lowercase();
        let blocked_dns_or_science = self.config.block_dns_and_science_telemetry
            && (lower.contains("dc-telemetry.net")
                || BLOCKED_TELEMETRY_PATHS
                    .iter()
                    .any(|path| lower.contains(path)));
        let blocked_sentry = self.config.disable_sentry_crashpad_upload
            && (lower.contains("o64374.ingest.sentry.io") || lower.contains("sentry.io"));

        if blocked_dns_or_science || blocked_sentry {
            self.telemetry.telemetry_requests_blocked += 1;
            return Err(D08PrivacyError::TelemetryEndpointBlocked {
                url: url.trim().to_string(),
            });
        }
        Ok(())
    }

    /// Evaluates the 12-point Platform Privacy Checklist (`D08-CHK-01` .. `D08-CHK-12`).
    pub fn evaluate_privacy_checklist(&mut self) -> D08PrivacyChecklistReport {
        self.telemetry.checklist_audits_run += 1;
        let checks = build_checklist_definitions(&self.config);
        let mut items = Vec::with_capacity(D08_CHECKLIST_ITEM_COUNT);
        let mut passed_count = 0;
        let mut failed_count = 0;

        for (id, category, requirement, passed) in checks {
            let status = if passed {
                passed_count += 1;
                D08ChecklistStatus::Passed
            } else {
                failed_count += 1;
                D08ChecklistStatus::Failed
            };
            items.push(D08PrivacyChecklistItem {
                id,
                category,
                requirement,
                status,
            });
        }

        D08PrivacyChecklistReport {
            items,
            passed_count,
            failed_count,
        }
    }
}

fn build_checklist_definitions(
    cfg: &D08PlatformPrivacyConfig,
) -> [(&'static str, &'static str, &'static str, bool); D08_CHECKLIST_ITEM_COUNT] {
    let iso = &cfg.media_session_isolation;
    [
        (
            "D08-CHK-01",
            "WebRTC IP Privacy",
            "Enforce DefaultPublicInterfaceOnly or DisableNonProxiedUdp WebRTC IP handling policy",
            cfg.webrtc_ip_policy.is_hardened(),
        ),
        (
            "D08-CHK-02",
            "WebRTC IP Privacy",
            "Strip RFC 1918, RFC 6598 CGNAT, RFC 4193 ULA, loopback, and link-local addresses from outbound signaling",
            cfg.webrtc_ip_policy.is_hardened() && cfg.enable_mdns_obfuscation,
        ),
        (
            "D08-CHK-03",
            "WebRTC IP Privacy",
            "Block non-proxied 74-byte UDP IP Discovery packets when DisableNonProxiedUdp is active",
            cfg.block_unproxied_udp_when_policy_requires,
        ),
        (
            "D08-CHK-04",
            "WebRTC IP Privacy",
            "Mask public IP addresses as <PUB_IP> and LAN IPs as <LAN_IP> in logs and diagnostics",
            cfg.redact_ips_in_diagnostics,
        ),
        (
            "D08-CHK-05",
            "MediaSession Isolation",
            "Disable HardwareMediaKeyHandling and MediaSessionService in all runtime feature sets",
            iso.disable_hardware_media_key_handling && iso.disable_media_session_service,
        ),
        (
            "D08-CHK-06",
            "MediaSession Isolation",
            "Operate strictly as a read-only D-Bus MPRIS consumer without registering org.mpris.MediaPlayer2.*",
            !iso.allow_os_media_transport_registration && iso.spotify_mpris_consumer_only,
        ),
        (
            "D08-CHK-07",
            "Auto-Play Suppression",
            "Enforce DocumentUserActivationRequired for all inline videos, external embeds, and voice messages",
            matches!(
                cfg.autoplay_policy,
                AutoplaySuppressionPolicy::DocumentUserActivationRequired
            ),
        ),
        (
            "D08-CHK-08",
            "Auto-Play Suppression",
            "Gate external embed iframes behind click-to-load consent and strip autoplay=1 query parameters",
            cfg.external_embed_click_to_load && cfg.strip_external_embed_autoplay_query,
        ),
        (
            "D08-CHK-09",
            "Telemetry Protection",
            "Block DnsReporter beacons to dc-telemetry.net and REST analytics to /api/v9/science",
            cfg.block_dns_and_science_telemetry,
        ),
        (
            "D08-CHK-10",
            "Telemetry Protection",
            "Disable automatic Crashpad / Sentry minidump uploads to o64374.ingest.sentry.io",
            cfg.disable_sentry_crashpad_upload,
        ),
        (
            "D08-CHK-11",
            "Artifact & Secret Privacy",
            "Redact token, secret_key, session_id, media_session_id, and OS audio device labels in Debug/logs",
            cfg.redact_secrets_and_device_labels,
        ),
        (
            "D08-CHK-12",
            "Streamer Mode Guardrails",
            "Automatically suppress PII, invite links, tokens, and desktop notifications when Streamer Mode is active",
            cfg.streamer_mode_pii_protection,
        ),
    ]
}

/// Classifies an IP literal or candidate host string into an [`IpAddrPrivacyClass`].
#[must_use]
pub fn classify_ip_privacy(host: &str) -> IpAddrPrivacyClass {
    let trimmed = host
        .trim()
        .trim_start_matches('[')
        .trim_end_matches(']');
    if trimmed.is_empty() {
        return IpAddrPrivacyClass::Invalid;
    }
    if trimmed.to_ascii_lowercase().ends_with(".local") {
        let prefix = &trimmed[..trimmed.len() - 6];
        if !prefix.is_empty() && !prefix.contains('.') {
            return IpAddrPrivacyClass::MdnsObfuscated;
        }
    }

    match trimmed.parse::<IpAddr>() {
        Ok(IpAddr::V4(v4)) => classify_ipv4_privacy(v4),
        Ok(IpAddr::V6(v6)) => classify_ipv6_privacy(v6),
        Err(_) => IpAddrPrivacyClass::Invalid,
    }
}

fn classify_ipv4_privacy(ip: Ipv4Addr) -> IpAddrPrivacyClass {
    let octets = ip.octets();
    if ip.is_unspecified() {
        IpAddrPrivacyClass::Unspecified
    } else if ip.is_loopback() {
        IpAddrPrivacyClass::Loopback
    } else if ip.is_link_local() {
        IpAddrPrivacyClass::LinkLocal
    } else if ip.is_private()
        || (octets[0] == 100 && (64..=127).contains(&octets[1]))
        || ip.is_broadcast()
    {
        IpAddrPrivacyClass::PrivateLan
    } else {
        IpAddrPrivacyClass::PublicUnicast
    }
}

fn classify_ipv6_privacy(ip: Ipv6Addr) -> IpAddrPrivacyClass {
    let segments = ip.segments();
    if ip.is_unspecified() {
        IpAddrPrivacyClass::Unspecified
    } else if ip.is_loopback() {
        IpAddrPrivacyClass::Loopback
    } else if (segments[0] & 0xffc0) == 0xfe80 {
        IpAddrPrivacyClass::LinkLocal
    } else if (segments[0] & 0xfe00) == 0xfc00 {
        IpAddrPrivacyClass::PrivateLan
    } else if let Some(mapped_v4) = ip.to_ipv4_mapped() {
        classify_ipv4_privacy(mapped_v4)
    } else {
        IpAddrPrivacyClass::PublicUnicast
    }
}

/// Validates an address and port before transmitting Voice Gateway `OP 1 SELECT_PROTOCOL` or 74-byte UDP IP Discovery.
pub fn validate_select_protocol_endpoint(
    policy: WebRtcIpHandlingPolicy,
    address: &str,
    port: u16,
    is_proxied: bool,
) -> Result<(), D08PrivacyError> {
    if port == 0 {
        return Err(D08PrivacyError::InvalidEndpointPort { port: 0 });
    }
    if matches!(policy, WebRtcIpHandlingPolicy::DisableNonProxiedUdp) && !is_proxied {
        return Err(D08PrivacyError::NonProxiedUdpBlocked);
    }

    let class = classify_ip_privacy(address);
    let allowed = match policy {
        WebRtcIpHandlingPolicy::DefaultPublicInterfaceOnly
        | WebRtcIpHandlingPolicy::DisableNonProxiedUdp => {
            class == IpAddrPrivacyClass::PublicUnicast
        }
        WebRtcIpHandlingPolicy::DefaultPublicAndPrivateInterfaces
        | WebRtcIpHandlingPolicy::Default => !matches!(
            class,
            IpAddrPrivacyClass::Unspecified
                | IpAddrPrivacyClass::Loopback
                | IpAddrPrivacyClass::LinkLocal
                | IpAddrPrivacyClass::Invalid
        ),
    };

    if !allowed {
        return Err(D08PrivacyError::PrivateIpLeakBlocked {
            address: address.trim().to_string(),
            class,
        });
    }
    Ok(())
}

/// Builds the platform-specific list of disabled Chromium features (`app/bundle.js:10` `#23151`).
pub fn build_disabled_chromium_features(
    platform: &str,
    darwin_major_release: u32,
) -> Result<Vec<String>, D08PrivacyError> {
    let mut features: Vec<String> = DISABLED_CHROMIUM_FEATURES_BASE
        .iter()
        .map(|s| (*s).to_string())
        .collect();

    match platform.trim() {
        "linux" => {
            features.extend(DISABLED_CHROMIUM_FEATURES_LINUX.iter().map(|s| (*s).to_string()));
            Ok(features)
        }
        "darwin" => {
            if darwin_major_release < 24 {
                features.extend(
                    DISABLED_CHROMIUM_FEATURES_DARWIN_PRE24
                        .iter()
                        .map(|s| (*s).to_string()),
                );
            }
            Ok(features)
        }
        "win32" => Ok(features),
        other => Err(D08PrivacyError::UnsupportedPlatform {
            platform: other.to_string(),
        }),
    }
}

/// Sanitizes an external video embed URL by stripping `autoplay=1` / `autoplay=true` and enforcing `autoplay=0&mute=1`.
#[must_use]
pub fn sanitize_external_embed_url(raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    let (base, query_opt) = match trimmed.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (trimmed, None),
    };

    let mut kept_params: Vec<String> = Vec::new();
    if let Some(query) = query_opt {
        for pair in query.split('&') {
            if pair.is_empty() {
                continue;
            }
            let key = pair
                .split_once('=')
                .map_or(pair, |(k, _)| k)
                .trim()
                .to_ascii_lowercase();
            if key != "autoplay" && key != "mute" && key != "muted" {
                kept_params.push(pair.to_string());
            }
        }
    }
    kept_params.push("autoplay=0".to_string());
    kept_params.push("mute=1".to_string());

    format!("{base}?{}", kept_params.join("&"))
}

/// Redacts IPv4 literals in diagnostic logs or crash strings, replacing public IPs with `<PUB_IP>`
/// and private/loopback/link-local IPs with `<LAN_IP>`.
#[must_use]
pub fn redact_ip_literals_in_text(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let bytes = text.as_bytes();
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i].is_ascii_digit() {
            let start = i;
            let mut j = i;
            let mut dots = 0;
            while j < bytes.len() && (bytes[j].is_ascii_digit() || bytes[j] == b'.') {
                if bytes[j] == b'.' {
                    dots += 1;
                }
                j += 1;
            }
            let candidate = &text[start..j];
            if dots == 3
                && let Ok(ipv4) = candidate.parse::<Ipv4Addr>()
            {
                match classify_ipv4_privacy(ipv4) {
                    IpAddrPrivacyClass::PublicUnicast => out.push_str(PUB_IP_MASK_TOKEN),
                    _ => out.push_str(LAN_IP_MASK_TOKEN),
                }
                i = j;
                continue;
            }
        }
        let ch = text[i..].chars().next().unwrap_or('\0');
        out.push(ch);
        i += ch.len_utf8();
    }

    out
}
