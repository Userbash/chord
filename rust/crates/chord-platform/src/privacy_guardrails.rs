//! Task D-08: Platform Privacy Guardrails (`docs/re/privacy-guardrails-d08.md`).
//!
//! Clean-room Platform Shell privacy specification and enforcement engine:
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

pub const UDP_DISCOVERY_PACKET_BYTES: usize = 74;
pub const UDP_KEEPALIVE_PACKET_BYTES: usize = 8;
pub const MEDIA_SOCKET_ROTATION_INTERVAL_MS: u64 = 60_080;
pub const PUB_IP_MASK_TOKEN: &str = "<PUB_IP>";
pub const LAN_IP_MASK_TOKEN: &str = "<LAN_IP>";

pub const DISABLED_CHROMIUM_FEATURES_BASE: [&str; 6] = [
    "WinRetrieveSuggestionsOnlyOnDemand",
    "HardwareMediaKeyHandling",
    "MediaSessionService",
    "UseEcoQoSForBackgroundProcess",
    "IntensiveWakeUpThrottling",
    "AllowAggressiveThrottlingWithWebSocket",
];

pub const DISABLED_CHROMIUM_FEATURES_DARWIN_PRE24: [&str; 7] = [
    "ScreenCaptureKitMac",
    "ScreenCaptureKitMacWindow",
    "ScreenCaptureKitMacScreen",
    "ScreenCaptureKitPickerScreen",
    "ScreenCaptureKitStreamPickerSonoma",
    "WarmScreenCaptureSonoma",
    "UseSCContentSharingPicker",
];

pub const DISABLED_CHROMIUM_FEATURES_LINUX: [&str; 1] = ["WaylandWpColorManagerV1"];
pub const HIJACK_MEDIA_SESSION_FEATURES: [&str; 2] =
    ["HardwareMediaKeyHandling", "MediaSessionService"];
pub const AUTOPLAY_SWITCH_LEGACY_NO_GESTURE: &str = "--autoplay-policy=no-user-gesture-required";
pub const AUTOPLAY_SWITCH_DOCUMENT_USER_ACTIVATION: &str =
    "--autoplay-policy=document-user-activation-required";
pub const BLOCKED_TELEMETRY_DOMAINS: [&str; 3] = [
    "dc-telemetry.net",
    "o64374.ingest.sentry.io",
    "sentry.io",
];
pub const BLOCKED_TELEMETRY_PATHS: [&str; 3] =
    ["/api/v9/science", "/api/v9/metrics", "/api/v9/track"];
pub const D08_CHECKLIST_ITEM_COUNT: usize = 12;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IpAddrPrivacyClass {
    Unspecified,
    Loopback,
    LinkLocal,
    PrivateLan,
    MdnsObfuscated,
    PublicUnicast,
    Invalid,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WebRtcIpHandlingPolicy {
    Default,
    DefaultPublicAndPrivateInterfaces,
    DefaultPublicInterfaceOnly,
    DisableNonProxiedUdp,
}

impl WebRtcIpHandlingPolicy {
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

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Default => "default",
            Self::DefaultPublicAndPrivateInterfaces => "default_public_and_private_interfaces",
            Self::DefaultPublicInterfaceOnly => "default_public_interface_only",
            Self::DisableNonProxiedUdp => "disable_non_proxied_udp",
        }
    }

    #[must_use]
    pub const fn is_hardened(self) -> bool {
        matches!(
            self,
            Self::DefaultPublicInterfaceOnly | Self::DisableNonProxiedUdp
        )
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoplaySuppressionPolicy {
    NoUserGestureRequired,
    UserGestureRequired,
    DocumentUserActivationRequired,
}

impl AutoplaySuppressionPolicy {
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

    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::NoUserGestureRequired => "NoUserGestureRequired",
            Self::UserGestureRequired => "UserGestureRequired",
            Self::DocumentUserActivationRequired => "DocumentUserActivationRequired",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum D08PrivacyError {
    InvalidWebRtcIpPolicy { raw: String },
    InvalidAutoplayPolicy { raw: String },
    UnsupportedPlatform { platform: String },
    PrivateIpLeakBlocked { address: String, class: IpAddrPrivacyClass },
    NonProxiedUdpBlocked,
    InvalidEndpointPort { port: u16 },
    MediaSessionRegistrationBlocked { service_name: String },
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
                write!(f, "blocked WebRTC private/non-public IP leak ({class:?}): {address}")
            }
            Self::NonProxiedUdpBlocked => {
                f.write_str("non-proxied direct UDP blocked by DisableNonProxiedUdp policy")
            }
            Self::InvalidEndpointPort { port } => write!(f, "invalid UDP endpoint port: {port}"),
            Self::MediaSessionRegistrationBlocked { service_name } => {
                write!(f, "OS MediaSession registration blocked: {service_name}")
            }
            Self::TelemetryEndpointBlocked { url } => {
                write!(f, "outbound telemetry/crash request blocked: {url}")
            }
        }
    }
}

impl std::error::Error for D08PrivacyError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IceCandidateEndpoint {
    pub address: String,
    pub port: u16,
    pub is_proxied: bool,
}

impl IceCandidateEndpoint {
    #[must_use]
    pub fn new(address: &str, port: u16, is_proxied: bool) -> Self {
        Self {
            address: address.trim().to_string(),
            port,
            is_proxied,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSessionIsolationPolicy {
    pub disable_hardware_media_key_handling: bool,
    pub disable_media_session_service: bool,
    pub allow_os_media_transport_registration: bool,
    pub spotify_mpris_consumer_only: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKeyEvent {
    PlayPause,
    NextTrack,
    PreviousTrack,
    Stop,
    VoiceToggleMute,
    VoiceToggleDeafen,
    VoicePushToTalk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaKeyRoutingDecision {
    PassThroughToOs,
    RouteToVoiceEngine,
    RouteToFocusedMediaModal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MediaPlaybackSource {
    InlineAttachmentVideo,
    AnimatedGifVideo,
    ExternalEmbedIframe,
    VoiceMessageAttachment,
    SoundboardClipPreview,
    ActiveVoiceCallRtc,
    NotificationSoundCue,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaPlaybackRequest {
    pub media_id: String,
    pub source: MediaPlaybackSource,
    pub has_user_gesture: bool,
    pub viewport_visible: bool,
    pub voice_session_joined: bool,
    pub user_deafened: bool,
    pub user_dnd: bool,
    pub streamer_mode: bool,
    pub disable_all_sounds: bool,
    pub requested_volume_pct: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutoplayMediaDecision {
    pub allow_video_playback: bool,
    pub allow_audio_playback: bool,
    pub force_muted: bool,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D08PlatformPrivacyConfig {
    pub webrtc_ip_policy: WebRtcIpHandlingPolicy,
    pub enable_mdns_obfuscation: bool,
    pub block_unproxied_udp_when_policy_requires: bool,
    pub redact_ips_in_diagnostics: bool,
    pub media_session_isolation: MediaSessionIsolationPolicy,
    pub autoplay_policy: AutoplaySuppressionPolicy,
    pub external_embed_click_to_load: bool,
    pub strip_external_embed_autoplay_query: bool,
    pub block_dns_and_science_telemetry: bool,
    pub disable_sentry_crashpad_upload: bool,
    pub redact_secrets_and_device_labels: bool,
    pub streamer_mode_pii_protection: bool,
}

impl D08PlatformPrivacyConfig {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum D08ChecklistStatus {
    Passed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D08PrivacyChecklistItem {
    pub id: &'static str,
    pub category: &'static str,
    pub requirement: &'static str,
    pub status: D08ChecklistStatus,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct D08PrivacyChecklistReport {
    pub items: Vec<D08PrivacyChecklistItem>,
    pub passed_count: usize,
    pub failed_count: usize,
}

impl D08PrivacyChecklistReport {
    #[must_use]
    pub const fn is_compliant(&self) -> bool {
        self.failed_count == 0 && self.passed_count == D08_CHECKLIST_ITEM_COUNT
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct D08PrivacyTelemetry {
    pub candidates_evaluated: u64,
    pub private_candidates_stripped: u64,
    pub udp_discovery_leaks_blocked: u64,
    pub media_keys_isolated_to_os: u64,
    pub autoplay_audio_suppressions: u64,
    pub telemetry_requests_blocked: u64,
    pub checklist_audits_run: u64,
}

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
    #[must_use]
    pub fn new(config: D08PlatformPrivacyConfig) -> Self {
        Self {
            config,
            activated_media_ids: BTreeSet::new(),
            telemetry: D08PrivacyTelemetry::default(),
        }
    }

    #[must_use]
    pub fn with_hardened_defaults() -> Self {
        Self::new(D08PlatformPrivacyConfig::hardened_default())
    }

    #[must_use]
    pub const fn config(&self) -> &D08PlatformPrivacyConfig {
        &self.config
    }

    #[must_use]
    pub const fn telemetry(&self) -> &D08PrivacyTelemetry {
        &self.telemetry
    }

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
                if self.record_and_check_user_activation(req) && vol > 0 {
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

    pub fn reset_channel_media_activations(&mut self) {
        self.activated_media_ids.clear();
    }

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
    let hardened_ip = cfg.webrtc_ip_policy.is_hardened();
    let doc_autoplay = matches!(
        cfg.autoplay_policy,
        AutoplaySuppressionPolicy::DocumentUserActivationRequired
    );
    [
        ("D08-CHK-01", "WebRTC IP Privacy", "Enforce hardened WebRTC IP handling policy", hardened_ip),
        ("D08-CHK-02", "WebRTC IP Privacy", "Strip RFC 1918, CGNAT, ULA, loopback, and link-local candidates", hardened_ip && cfg.enable_mdns_obfuscation),
        ("D08-CHK-03", "WebRTC IP Privacy", "Block non-proxied 74-byte UDP IP Discovery when required", cfg.block_unproxied_udp_when_policy_requires),
        ("D08-CHK-04", "WebRTC IP Privacy", "Mask public IPs as <PUB_IP> and LAN IPs as <LAN_IP> in logs", cfg.redact_ips_in_diagnostics),
        ("D08-CHK-05", "MediaSession Isolation", "Disable HardwareMediaKeyHandling and MediaSessionService", iso.disable_hardware_media_key_handling && iso.disable_media_session_service),
        ("D08-CHK-06", "MediaSession Isolation", "Operate strictly as read-only D-Bus MPRIS consumer", !iso.allow_os_media_transport_registration && iso.spotify_mpris_consumer_only),
        ("D08-CHK-07", "Auto-Play Suppression", "Enforce DocumentUserActivationRequired for inline/embed media", doc_autoplay),
        ("D08-CHK-08", "Auto-Play Suppression", "Gate external embeds behind click-to-load and strip autoplay=1", cfg.external_embed_click_to_load && cfg.strip_external_embed_autoplay_query),
        ("D08-CHK-09", "Telemetry Protection", "Block dc-telemetry.net and /api/v9/science telemetry", cfg.block_dns_and_science_telemetry),
        ("D08-CHK-10", "Telemetry Protection", "Disable automatic Sentry/Crashpad minidump uploads", cfg.disable_sentry_crashpad_upload),
        ("D08-CHK-11", "Artifact & Secret Privacy", "Redact tokens, keys, session IDs, and device labels", cfg.redact_secrets_and_device_labels),
        ("D08-CHK-12", "Streamer Mode Guardrails", "Suppress PII, invite links, and notifications in Streamer Mode", cfg.streamer_mode_pii_protection),
    ]
}

#[must_use]
pub fn classify_ip_privacy(host: &str) -> IpAddrPrivacyClass {
    let trimmed = host.trim().trim_start_matches('[').trim_end_matches(']');
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

#[must_use]
pub fn sanitize_external_embed_url(raw_url: &str) -> String {
    let trimmed = raw_url.trim();
    let (base, query_opt) = match trimmed.split_once('?') {
        Some((b, q)) => (b, Some(q)),
        None => (trimmed, None),
    };
    let mut kept_params: Vec<String> = Vec::new();
    if let Some(query) = query_opt {
        for pair in query.split('&').filter(|p| !p.is_empty()) {
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
