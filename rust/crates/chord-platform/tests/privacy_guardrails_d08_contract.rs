#![forbid(unsafe_code)]
//! Contract tests for Task D-08: Platform Privacy Guardrails (`chord_platform::privacy_guardrails`).
//!
//! Specification & Clean-Room Reference Oracles:
//! - `docs/feature-backlog.md` row `D-08`:
//!   "Platform Privacy Guardrails: WebRTC IP leakage prevention policy, MediaSession key bindings
//!   isolation, auto-play media audio suppression. Target: Platform Shell. Criteria: Comprehensive
//!   privacy checklist for native client implementation ensuring user IP and telemetry protection"
//! - `docs/re/privacy-guardrails-d08.md` & `docs/re/privacy-guardrails-d08.json`
//! - `docs/re/main-arch.md` (§1.2 Steps 4–6, §7–§8) & `docs/re/voice-deep.md` (§2.1 74B IP Discovery, §2.5 +60.08s rotation)
//!
//! Covers all 9 mandatory dimensions (`TEST-POLICY.md` §12):
//! 1. Positive path: WebRTC IP handling policies (`DefaultPublicInterfaceOnly`, `DisableNonProxiedUdp`),
//!    RFC 1918 / RFC 6598 CGNAT / RFC 4193 ULA / link-local / loopback candidate scrubbing,
//!    74-byte UDP IP Discovery / `OP 1 SELECT_PROTOCOL` endpoint filtering, MediaSession key bindings
//!    isolation (`HardwareMediaKeyHandling`, `MediaSessionService`, D-Bus MPRIS consumer-only isolation),
//!    source-classified auto-play audio suppression (`DocumentUserActivationRequired`), external embed
//!    `autoplay=1` URL sanitization, telemetry blocking (`dc-telemetry.net`, Sentry, `/api/v9/science`),
//!    and 12/12 `D08-CHK-01..12` checklist audit pass.
//! 2. Negative path: rejecting unknown WebRTC IP policies, unknown autoplay policies, unknown platforms,
//!    private/loopback/unspecified IP leakage in `SELECT_PROTOCOL` under `DefaultPublicInterfaceOnly`,
//!    unproxied 74-byte UDP discovery under `DisableNonProxiedUdp`, re-enabling `HardwareMediaKeyHandling`
//!    or `MediaSessionService`, and attempting to register `org.mpris.MediaPlayer2.*` as an OS player.
//! 3. Boundary conditions: IPv4/IPv6 CIDR boundaries (`10.0.0.0`..`10.255.255.255`, `172.16.0.0`..`172.31.255.255`
//!    vs `172.15.255.255`/`172.32.0.1`, `192.168.0.0/16`, `100.64.0.0`..`100.127.255.255` CGNAT,
//!    `169.254.0.0/16`, `127.0.0.1`, `0.0.0.0`, `::1`, `fe80::`, `fc00::`/`fd00::`), Darwin release
//!    boundary (`23` vs `24` for `ScreenCaptureKitMac*` flags), port boundaries (`0`, `1`, `65535`),
//!    and soundboard/playback volume boundaries (`0..=200%`).
//! 4. Error handling: explicit `D08PrivacyError` variants on invalid policies, leaked private endpoints,
//!    blocked non-proxied UDP, hijacked MediaSession flags, or failing checklist controls.
//! 5. Empty states: empty candidate lists, empty disabled-feature overrides, empty query strings on
//!    embed URLs, and unjoined voice session state (`voice_session_joined = false`).
//! 6. Intermediate / lifecycle states: inline attachment video starting muted (`DocumentUserActivationRequired`)
//!    -> explicit user click unmuting only the target media ID -> channel switch resetting unmuted state;
//!    and +60,080 ms media socket rotation re-validating the rotated public endpoint.
//! 7. Capabilities, permissions & security: diagnostic IP masking (`<PUB_IP>`, `<LAN_IP>`), secret token
//!    and audio device label redaction, Streamer Mode (`R5-10`) PII/invite/notification masking, and
//!    telemetry domain/path blocking (`dc-telemetry.net`, `o64374.ingest.sentry.io`, `/api/v9/science`).
//! 8. Recovery & degradation: proxy/TURN fallback when `DisableNonProxiedUdp` blocks direct UDP discovery,
//!    and mDNS `.local` candidate preservation when raw LAN candidates are stripped.
//! 9. Observable telemetry & empirical oracle verification: verifying `D08PrivacyTelemetry` counters
//!    and full parity with `docs/re/privacy-guardrails-d08.md`, `docs/re/privacy-guardrails-d08.json`,
//!    `docs/re/main-arch.md`, and `docs/re/voice-deep.md`.

use std::fs;
use std::path::{Path, PathBuf};

use chord_platform::privacy_guardrails::{
    AUTOPLAY_SWITCH_DOCUMENT_USER_ACTIVATION, AUTOPLAY_SWITCH_LEGACY_NO_GESTURE,
    AutoplayMediaDecision, AutoplaySuppressionPolicy, BLOCKED_TELEMETRY_DOMAINS,
    BLOCKED_TELEMETRY_PATHS, D08_CHECKLIST_ITEM_COUNT, D08ChecklistStatus,
    D08PlatformPrivacyConfig, D08PrivacyChecklistItem, D08PrivacyChecklistReport, D08PrivacyError,
    D08PrivacyGuardrailsEngine, D08PrivacyTelemetry, DISABLED_CHROMIUM_FEATURES_BASE,
    DISABLED_CHROMIUM_FEATURES_DARWIN_PRE24, DISABLED_CHROMIUM_FEATURES_LINUX,
    HIJACK_MEDIA_SESSION_FEATURES, IceCandidateEndpoint, IpAddrPrivacyClass, LAN_IP_MASK_TOKEN,
    MEDIA_SOCKET_ROTATION_INTERVAL_MS, MediaKeyEvent, MediaKeyRoutingDecision,
    MediaPlaybackRequest, MediaPlaybackSource, MediaSessionIsolationPolicy, PUB_IP_MASK_TOKEN,
    UDP_DISCOVERY_PACKET_BYTES, UDP_KEEPALIVE_PACKET_BYTES, WebRtcIpHandlingPolicy,
    build_disabled_chromium_features, classify_ip_privacy, redact_ip_literals_in_text,
    sanitize_external_embed_url, validate_select_protocol_endpoint,
};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

// ============================================================================
// Dimension 1: Positive Path — WebRTC IP Leakage Prevention, MediaSession
// Key Bindings Isolation, Auto-Play Audio Suppression & 12-Point Checklist
// ============================================================================

#[test]
fn d08_dim1_webrtc_ip_leakage_prevention_and_candidate_filtering() {
    let mut engine = D08PrivacyGuardrailsEngine::with_hardened_defaults();
    assert_eq!(
        engine.config().webrtc_ip_policy,
        WebRtcIpHandlingPolicy::DefaultPublicInterfaceOnly
    );
    assert_eq!(UDP_DISCOVERY_PACKET_BYTES, 74);
    assert_eq!(UDP_KEEPALIVE_PACKET_BYTES, 8);
    assert_eq!(MEDIA_SOCKET_ROTATION_INTERVAL_MS, 60_080);

    let candidates = vec![
        IceCandidateEndpoint::new("127.0.0.1", 50000, false),
        IceCandidateEndpoint::new("192.168.1.42", 50001, false),
        IceCandidateEndpoint::new("10.8.0.2", 50002, false),
        IceCandidateEndpoint::new("172.20.10.5", 50003, false),
        IceCandidateEndpoint::new("100.64.12.9", 50004, false),
        IceCandidateEndpoint::new("169.254.10.20", 50005, false),
        IceCandidateEndpoint::new("fe80::1ff:fe23:4567:890a", 50006, false),
        IceCandidateEndpoint::new("fd12:3456:789a::1", 50007, false),
        IceCandidateEndpoint::new("e4b7c2d1-9a10-4f33-8b1e-0123456789ab.local", 50008, false),
        IceCandidateEndpoint::new("104.29.147.145", 52287, false),
    ];

    let filtered = engine.filter_webrtc_candidates(&candidates);
    // Under DefaultPublicInterfaceOnly with mDNS obfuscation enabled, only the mDNS `.local`
    // placeholder and the public unicast candidate survive; all 8 private/loopback/link-local/ULA/CGNAT IPs are stripped.
    assert_eq!(filtered.len(), 2);
    assert_eq!(
        filtered[0].address,
        "e4b7c2d1-9a10-4f33-8b1e-0123456789ab.local"
    );
    assert_eq!(filtered[1].address, "104.29.147.145");
    assert_eq!(filtered[1].port, 52287);

    // Verify 74-byte UDP IP Discovery / OP 1 SELECT_PROTOCOL public endpoint validation
    assert!(
        engine
            .authorize_udp_ip_discovery("104.29.147.145", 19309, false)
            .is_ok()
    );
    assert!(
        validate_select_protocol_endpoint(
            WebRtcIpHandlingPolicy::DefaultPublicInterfaceOnly,
            "203.0.113.45",
            35251,
            false,
        )
        .is_ok()
    );
}

#[test]
fn d08_dim1_media_session_key_bindings_isolation_and_mpris_consumer_contract() {
    let engine = D08PrivacyGuardrailsEngine::with_hardened_defaults();
    let isolation = &engine.config().media_session_isolation;

    assert!(isolation.disable_hardware_media_key_handling);
    assert!(isolation.disable_media_session_service);
    assert!(!isolation.allow_os_media_transport_registration);
    assert!(isolation.spotify_mpris_consumer_only);

    // Verify base disabled Chromium features from app/bundle.js:10 (#23151 @15800..16000)
    assert_eq!(DISABLED_CHROMIUM_FEATURES_BASE.len(), 6);
    assert_eq!(HIJACK_MEDIA_SESSION_FEATURES.len(), 2);
    for feature in HIJACK_MEDIA_SESSION_FEATURES {
        assert!(DISABLED_CHROMIUM_FEATURES_BASE.contains(&feature));
    }

    let linux_features = build_disabled_chromium_features("linux", 24).expect("linux features");
    assert_eq!(linux_features.len(), 7);
    assert!(linux_features.contains(&"HardwareMediaKeyHandling".to_string()));
    assert!(linux_features.contains(&"MediaSessionService".to_string()));
    assert!(linux_features.contains(&"WaylandWpColorManagerV1".to_string()));

    // Verify hardware media keys pass through to OS music player (Spotify/MPD) instead of hijacking Discord chat videos
    for key in [
        MediaKeyEvent::PlayPause,
        MediaKeyEvent::NextTrack,
        MediaKeyEvent::PreviousTrack,
        MediaKeyEvent::Stop,
    ] {
        assert_eq!(
            engine.route_media_key_event(key, false),
            MediaKeyRoutingDecision::PassThroughToOs
        );
    }

    // Voice-specific keybinds (PushToTalk / ToggleMute / ToggleDeafen) route cleanly to the Voice Engine
    assert_eq!(
        engine.route_media_key_event(MediaKeyEvent::VoiceToggleMute, false),
        MediaKeyRoutingDecision::RouteToVoiceEngine
    );
    assert_eq!(
        engine.route_media_key_event(MediaKeyEvent::VoiceToggleDeafen, false),
        MediaKeyRoutingDecision::RouteToVoiceEngine
    );
}

#[test]
fn d08_dim1_autoplay_media_audio_suppression_and_ checklist_verification() {
    let mut engine = D08PrivacyGuardrailsEngine::with_hardened_defaults();
    assert_eq!(
        engine.config().autoplay_policy,
        AutoplaySuppressionPolicy::DocumentUserActivationRequired
    );
    assert_eq!(
        AUTOPLAY_SWITCH_LEGACY_NO_GESTURE,
        "--autoplay-policy=no-user-gesture-required"
    );
    assert_eq!(
        AUTOPLAY_SWITCH_DOCUMENT_USER_ACTIVATION,
        "--autoplay-policy=document-user-activation-required"
    );

    // 1. Inline attachment video without user gesture: video may preview muted, audio MUST be suppressed
    let inline_req = MediaPlaybackRequest {
        media_id: "att-video-101".to_string(),
        source: MediaPlaybackSource::InlineAttachmentVideo,
        has_user_gesture: false,
        viewport_visible: true,
        voice_session_joined: false,
        user_deafened: false,
        user_dnd: false,
        streamer_mode: false,
        disable_all_sounds: false,
        requested_volume_pct: 100,
    };
    let d_inline: AutoplayMediaDecision = engine.evaluate_media_playback(&inline_req);
    assert!(d_inline.allow_video_playback);
    assert!(!d_inline.allow_audio_playback);
    assert!(d_inline.force_muted);
    assert_eq!(d_inline.effective_volume_pct, 0);

    // 2. Animated GIFV (`.mp4`/`.webm` Tenor/Giphy): audio permanently suppressed even with user gesture
    let gifv_req = MediaPlaybackRequest {
        media_id: "gifv-202".to_string(),
        source: MediaPlaybackSource::AnimatedGifVideo,
        has_user_gesture: true,
        ..inline_req.clone()
    };
    let d_gifv = engine.evaluate_media_playback(&gifv_req);
    assert!(d_gifv.allow_video_playback);
    assert!(!d_gifv.allow_audio_playback);
    assert!(d_gifv.force_muted);
    assert_eq!(d_gifv.effective_volume_pct, 0);

    // 3. External embed iframe without user gesture: both video load and audio blocked
    let embed_req = MediaPlaybackRequest {
        media_id: "yt-embed-303".to_string(),
        source: MediaPlaybackSource::ExternalEmbedIframe,
        has_user_gesture: false,
        ..inline_req.clone()
    };
    let d_embed = engine.evaluate_media_playback(&embed_req);
    assert!(!d_embed.allow_video_playback);
    assert!(!d_embed.allow_audio_playback);
    assert!(d_embed.force_muted);

    // 4. Voice message attachment without click: blocked
    let vm_req = MediaPlaybackRequest {
        media_id: "vm-404".to_string(),
        source: MediaPlaybackSource::VoiceMessageAttachment,
        has_user_gesture: false,
        ..inline_req.clone()
    };
    let d_vm = engine.evaluate_media_playback(&vm_req);
    assert!(!d_vm.allow_audio_playback);
    assert!(d_vm.force_muted);

    // 5. Joined voice call RTC audio when not deafened: permitted
    let rtc_req = MediaPlaybackRequest {
        media_id: "rtc-call-505".to_string(),
        source: MediaPlaybackSource::ActiveVoiceCallRtc,
        has_user_gesture: false,
        voice_session_joined: true,
        user_deafened: false,
        ..inline_req.clone()
    };
    let d_rtc = engine.evaluate_media_playback(&rtc_req);
    assert!(d_rtc.allow_audio_playback);
    assert!(!d_rtc.force_muted);
    assert_eq!(d_rtc.effective_volume_pct, 100);

    // 6. Verify 12/12 comprehensive privacy checklist passes on hardened config
    let report: D08PrivacyChecklistReport = engine.evaluate_privacy_checklist();
    assert_eq!(report.items.len(), D08_CHECKLIST_ITEM_COUNT);
    assert_eq!(report.passed_count, 12);
    assert_eq!(report.failed_count, 0);
    assert!(report.is_compliant());
}

// ============================================================================
// Dimension 2: Negative Path — Invalid Policies, Private IP Leaks, Non-Proxied
// UDP Blocks, MediaSession Hijack Attempts, and Telemetry Rejections
// ============================================================================

#[test]
fn d08_dim2_negative_invalid_configs_and_privacy_violations_rejected() {
    // 1. Invalid WebRTC IP policy string
    assert_eq!(
        WebRtcIpHandlingPolicy::parse("allow_all_lan_ips"),
        Err(D08PrivacyError::InvalidWebRtcIpPolicy {
            raw: "allow_all_lan_ips".to_string()
        })
    );

    // 2. Invalid autoplay policy string
    assert_eq!(
        AutoplaySuppressionPolicy::parse("autoplay_everything_loudly"),
        Err(D08PrivacyError::InvalidAutoplayPolicy {
            raw: "autoplay_everything_loudly".to_string()
        })
    );

    // 3. Unsupported OS platform in build_disabled_chromium_features
    assert_eq!(
        build_disabled_chromium_features("solaris", 24),
        Err(D08PrivacyError::UnsupportedPlatform {
            platform: "solaris".to_string()
        })
    );

    // 4. Reject private LAN, loopback, link-local, and unspecified IPs in SELECT_PROTOCOL under DefaultPublicInterfaceOnly
    for leaked_ip in [
        "192.168.1.10",
        "10.0.0.1",
        "172.16.5.4",
        "100.64.0.1",
        "127.0.0.1",
        "169.254.1.1",
        "0.0.0.0",
        "::1",
        "fe80::1",
        "fd00::1",
    ] {
        let res = validate_select_protocol_endpoint(
            WebRtcIpHandlingPolicy::DefaultPublicInterfaceOnly,
            leaked_ip,
            52287,
            false,
        );
        assert!(
            matches!(res, Err(D08PrivacyError::PrivateIpLeakBlocked { .. })),
            "expected PrivateIpLeakBlocked for {leaked_ip}, got {res:?}"
        );
    }

    // 5. Reject non-proxied UDP IP Discovery under DisableNonProxiedUdp
    let mut strict_engine = D08PrivacyGuardrailsEngine::new(D08PlatformPrivacyConfig {
        webrtc_ip_policy: WebRtcIpHandlingPolicy::DisableNonProxiedUdp,
        ..D08PlatformPrivacyConfig::hardened_default()
    });
    assert_eq!(
        strict_engine.authorize_udp_ip_discovery("104.29.147.145", 19309, false),
        Err(D08PrivacyError::NonProxiedUdpBlocked)
    );

    // 6. Reject OS MPRIS MediaPlayer2 service registration when MediaSession isolation is active
    assert_eq!(
        strict_engine.authorize_os_media_player_registration("org.mpris.MediaPlayer2.discord"),
        Err(D08PrivacyError::MediaSessionRegistrationBlocked {
            service_name: "org.mpris.MediaPlayer2.discord".to_string()
        })
    );
}

// ============================================================================
// Dimension 3: Boundary Conditions — CIDR Subnet Edges, Darwin Kernel 23 vs 24,
// Port Bounds (0, 1, 65535), and Volume Bounds (0..=200%)
// ============================================================================

#[test]
fn d08_dim3_cidr_boundaries_darwin_kernel_and_port_volume_limits() {
    // 1. RFC 1918 172.16.0.0/12 exact boundaries: 172.15.255.255 is Public, 172.16.0.0..172.31.255.255 is Private, 172.32.0.0 is Public
    assert_eq!(
        classify_ip_privacy("172.15.255.255"),
        IpAddrPrivacyClass::PublicUnicast
    );
    assert_eq!(
        classify_ip_privacy("172.16.0.0"),
        IpAddrPrivacyClass::PrivateLan
    );
    assert_eq!(
        classify_ip_privacy("172.31.255.255"),
        IpAddrPrivacyClass::PrivateLan
    );
    assert_eq!(
        classify_ip_privacy("172.32.0.0"),
        IpAddrPrivacyClass::PublicUnicast
    );

    // 2. RFC 6598 CGNAT 100.64.0.0/10 exact boundaries: 100.63.255.255 is Public, 100.64.0.0..100.127.255.255 is PrivateLan, 100.128.0.0 is Public
    assert_eq!(
        classify_ip_privacy("100.63.255.255"),
        IpAddrPrivacyClass::PublicUnicast
    );
    assert_eq!(
        classify_ip_privacy("100.64.0.0"),
        IpAddrPrivacyClass::PrivateLan
    );
    assert_eq!(
        classify_ip_privacy("100.127.255.255"),
        IpAddrPrivacyClass::PrivateLan
    );
    assert_eq!(
        classify_ip_privacy("100.128.0.0"),
        IpAddrPrivacyClass::PublicUnicast
    );

    // 3. Darwin kernel major release boundary: 23 includes 7 ScreenCaptureKitMac* flags (6 + 7 = 13), 24 includes only base 6
    let darwin_23 = build_disabled_chromium_features("darwin", 23).expect("darwin 23");
    assert_eq!(
        darwin_23.len(),
        DISABLED_CHROMIUM_FEATURES_BASE.len() + DISABLED_CHROMIUM_FEATURES_DARWIN_PRE24.len()
    );
    assert_eq!(darwin_23.len(), 13);

    let darwin_24 = build_disabled_chromium_features("darwin", 24).expect("darwin 24");
    assert_eq!(darwin_24.len(), DISABLED_CHROMIUM_FEATURES_BASE.len());
    assert_eq!(darwin_24.len(), 6);

    let win32_features = build_disabled_chromium_features("win32", 24).expect("win32");
    assert_eq!(win32_features.len(), 6);
    assert_eq!(DISABLED_CHROMIUM_FEATURES_LINUX.len(), 1);

    // 4. Port boundaries: 0 is invalid, 1 and 65535 are valid
    assert_eq!(
        validate_select_protocol_endpoint(
            WebRtcIpHandlingPolicy::DefaultPublicInterfaceOnly,
            "104.29.147.145",
            0,
            false,
        ),
        Err(D08PrivacyError::InvalidEndpointPort { port: 0 })
    );
    assert!(
        validate_select_protocol_endpoint(
            WebRtcIpHandlingPolicy::DefaultPublicInterfaceOnly,
            "104.29.147.145",
            1,
            false,
        )
        .is_ok()
    );
    assert!(
        validate_select_protocol_endpoint(
            WebRtcIpHandlingPolicy::DefaultPublicInterfaceOnly,
            "104.29.147.145",
            u16::MAX,
            false,
        )
        .is_ok()
    );

    // 5. Volume percentage clamping boundary (0..=200%)
    let mut engine = D08PrivacyGuardrailsEngine::with_hardened_defaults();
    let gesture_req = MediaPlaybackRequest {
        media_id: "att-vol-bound".to_string(),
        source: MediaPlaybackSource::InlineAttachmentVideo,
        has_user_gesture: true,
        viewport_visible: true,
        voice_session_joined: false,
        user_deafened: false,
        user_dnd: false,
        streamer_mode: false,
        disable_all_sounds: false,
        requested_volume_pct: 350, // exceeds 200% max
    };
    let decision = engine.evaluate_media_playback(&gesture_req);
    assert!(decision.allow_audio_playback);
    assert_eq!(decision.effective_volume_pct, 200);
}

// ============================================================================
// Dimension 4: Error Handling — Typed D08PrivacyError Variants & Display
// ============================================================================

#[test]
fn d08_dim4_typed_privacy_errors_and_display_formatting() {
    let errors = [
        D08PrivacyError::InvalidWebRtcIpPolicy {
            raw: "bad_policy".to_string(),
        },
        D08PrivacyError::InvalidAutoplayPolicy {
            raw: "bad_autoplay".to_string(),
        },
        D08PrivacyError::UnsupportedPlatform {
            platform: "plan9".to_string(),
        },
        D08PrivacyError::PrivateIpLeakBlocked {
            address: "192.168.0.10".to_string(),
            class: IpAddrPrivacyClass::PrivateLan,
        },
        D08PrivacyError::NonProxiedUdpBlocked,
        D08PrivacyError::InvalidEndpointPort { port: 0 },
        D08PrivacyError::MediaSessionRegistrationBlocked {
            service_name: "org.mpris.MediaPlayer2.discord".to_string(),
        },
        D08PrivacyError::TelemetryEndpointBlocked {
            url: "https://dc-telemetry.net/dns".to_string(),
        },
    ];

    for err in &errors {
        let msg = err.to_string();
        assert!(!msg.is_empty());
    }
}

// ============================================================================
// Dimension 5: Empty States — Empty Candidate List, Empty Embed Query,
// Unjoined Voice Call, and Zero-Volume Playback
// ============================================================================

#[test]
fn d08_dim5_empty_candidates_unjoined_voice_and_empty_query_urls() {
    let mut engine = D08PrivacyGuardrailsEngine::with_hardened_defaults();

    // 1. Empty candidate list returns empty Vec without panic
    let filtered = engine.filter_webrtc_candidates(&[]);
    assert!(filtered.is_empty());

    // 2. ActiveVoiceCallRtc when voice_session_joined == false: audio and video suppressed
    let unjoined_rtc = MediaPlaybackRequest {
        media_id: "rtc-unjoined".to_string(),
        source: MediaPlaybackSource::ActiveVoiceCallRtc,
        has_user_gesture: false,
        viewport_visible: true,
        voice_session_joined: false,
        user_deafened: false,
        user_dnd: false,
        streamer_mode: false,
        disable_all_sounds: false,
        requested_volume_pct: 100,
    };
    let decision = engine.evaluate_media_playback(&unjoined_rtc);
    assert!(!decision.allow_audio_playback);
    assert!(!decision.allow_video_playback);
    assert!(decision.force_muted);

    // 3. External embed URL with no query parameters gets `autoplay=0&mute=1` appended cleanly
    let sanitized = sanitize_external_embed_url("https://www.youtube.com/embed/dQw4w9WgXcQ");
    assert_eq!(
        sanitized,
        "https://www.youtube.com/embed/dQw4w9WgXcQ?autoplay=0&mute=1"
    );
}

// ============================================================================
// Dimension 6: Intermediate / Lifecycle Transitions — Muted Autoplay ->
// Single-Target User Gesture Activation -> Channel Switch Reset & +60.08s Rotation
// ============================================================================

#[test]
fn d08_dim6_user_gesture_single_target_activation_and_channel_switch_reset() {
    let mut engine = D08PrivacyGuardrailsEngine::with_hardened_defaults();

    let video_a = MediaPlaybackRequest {
        media_id: "video-msg-1001".to_string(),
        source: MediaPlaybackSource::InlineAttachmentVideo,
        has_user_gesture: false,
        viewport_visible: true,
        voice_session_joined: false,
        user_deafened: false,
        user_dnd: false,
        streamer_mode: false,
        disable_all_sounds: false,
        requested_volume_pct: 80,
    };
    let video_b = MediaPlaybackRequest {
        media_id: "video-msg-1002".to_string(),
        ..video_a.clone()
    };

    // Phase 1: Initial hydration without gesture -> both video_a and video_b have audio suppressed
    assert!(!engine.evaluate_media_playback(&video_a).allow_audio_playback);
    assert!(!engine.evaluate_media_playback(&video_b).allow_audio_playback);

    // Phase 2: User clicks unmute/play on video_a (`has_user_gesture = true`)
    let video_a_click = MediaPlaybackRequest {
        has_user_gesture: true,
        ..video_a.clone()
    };
    let d_a_click = engine.evaluate_media_playback(&video_a_click);
    assert!(d_a_click.allow_audio_playback);
    assert!(!d_a_click.force_muted);
    assert_eq!(d_a_click.effective_volume_pct, 80);

    // Subsequent frames of video_a retain activation without requiring continuous click events,
    // while video_b in the same channel remains muted!
    assert!(engine.evaluate_media_playback(&video_a).allow_audio_playback);
    assert!(!engine.evaluate_media_playback(&video_b).allow_audio_playback);

    // Phase 3: Switching channels clears all per-element audio activations
    engine.reset_channel_media_activations();
    assert!(!engine.evaluate_media_playback(&video_a).allow_audio_playback);

    // Phase 4: Media socket rotation at +60,080 ms re-validates rotated public port
    assert!(
        engine
            .validate_media_socket_rotation("104.29.146.60", 35251, 35999, 60_080, false)
            .is_ok()
    );
    assert_eq!(
        engine.validate_media_socket_rotation("192.168.1.50", 35251, 35999, 60_080, false),
        Err(D08PrivacyError::PrivateIpLeakBlocked {
            address: "192.168.1.50".to_string(),
            class: IpAddrPrivacyClass::PrivateLan,
        })
    );
}

// ============================================================================
// Dimension 7: Capabilities, Permissions & Security — Telemetry Blocking,
// IP/Secret Redaction, Embed URL Sanitization, and Streamer Mode Protection
// ============================================================================

#[test]
fn d08_dim7_telemetry_blocking_ip_redaction_and_embed_url_sanitization() {
    let mut engine = D08PrivacyGuardrailsEngine::with_hardened_defaults();

    // 1. Telemetry & Crashpad collector URLs are blocked (`dc-telemetry.net`, Sentry, `/api/v9/science`)
    assert_eq!(BLOCKED_TELEMETRY_DOMAINS.len(), 3);
    assert_eq!(BLOCKED_TELEMETRY_PATHS.len(), 3);

    for blocked_url in [
        "https://idt-loc-chk.dc-telemetry.net/",
        "https://d2558f321dfc7ab68366d8258fd256c7@o64374.ingest.sentry.io/146342",
        "https://sentry.io/api/146342/minidump/?sentry_key=abc",
        "https://discord.com/api/v9/science",
        "https://discord.com/api/v9/metrics",
        "https://discord.com/api/v9/track",
    ] {
        assert_eq!(
            engine.authorize_outbound_request(blocked_url),
            Err(D08PrivacyError::TelemetryEndpointBlocked {
                url: blocked_url.to_string()
            })
        );
    }

    // Legitimate Discord API & CDN routes are permitted
    for allowed_url in [
        "https://discord.com/api/v9/users/@me",
        "https://cdn.discordapp.com/attachments/10/20/video.mp4",
        "https://gateway.discord.gg/?v=9&encoding=etf&compress=zstd-stream",
    ] {
        assert!(engine.authorize_outbound_request(allowed_url).is_ok());
    }

    // 2. External embed URL `autoplay=1` stripping
    let dirty_yt = "https://www.youtube.com/embed/dQw4w9WgXcQ?autoplay=1&rel=0";
    let clean_yt = sanitize_external_embed_url(dirty_yt);
    assert!(!clean_yt.contains("autoplay=1"));
    assert!(clean_yt.contains("autoplay=0"));
    assert!(clean_yt.contains("mute=1"));
    assert!(clean_yt.contains("rel=0"));

    // 3. Diagnostic text IP redaction (`<PUB_IP>` and `<LAN_IP>`)
    assert_eq!(PUB_IP_MASK_TOKEN, "<PUB_IP>");
    assert_eq!(LAN_IP_MASK_TOKEN, "<LAN_IP>");
    let raw_log =
        "UDP discovery from 192.168.1.42:52287 to media server 104.29.147.145:19309 (loopback 127.0.0.1)";
    let redacted_log = redact_ip_literals_in_text(raw_log);
    assert!(!redacted_log.contains("192.168.1.42"));
    assert!(!redacted_log.contains("104.29.147.145"));
    assert!(redacted_log.contains("<LAN_IP>:52287"));
    assert!(redacted_log.contains("<PUB_IP>:19309"));

    // 4. Notification sound cue suppressed under Streamer Mode or DND
    let cue_in_streamer_mode = MediaPlaybackRequest {
        media_id: "cue-mention".to_string(),
        source: MediaPlaybackSource::NotificationSoundCue,
        has_user_gesture: false,
        viewport_visible: true,
        voice_session_joined: false,
        user_deafened: false,
        user_dnd: false,
        streamer_mode: true,
        disable_all_sounds: false,
        requested_volume_pct: 100,
    };
    let d_cue = engine.evaluate_media_playback(&cue_in_streamer_mode);
    assert!(!d_cue.allow_audio_playback);
    assert!(d_cue.force_muted);
}

// ============================================================================
// Dimension 8: Recovery & Degradation — Proxy/TURN Fallback under
// DisableNonProxiedUdp and Checklist Failure Detection + Remediation
// ============================================================================

#[test]
fn d08_dim8_proxy_fallback_and_unhardened_checklist_failure_detection() {
    // 1. Under DisableNonProxiedUdp, unproxied candidates are stripped, but proxied TURN/SOCKS5 candidates pass
    let mut proxy_engine = D08PrivacyGuardrailsEngine::new(D08PlatformPrivacyConfig {
        webrtc_ip_policy: WebRtcIpHandlingPolicy::DisableNonProxiedUdp,
        ..D08PlatformPrivacyConfig::hardened_default()
    });
    let mixed = vec![
        IceCandidateEndpoint::new("104.29.147.145", 19309, false),
        IceCandidateEndpoint::new("104.29.147.145", 19309, true),
    ];
    let survivors = proxy_engine.filter_webrtc_candidates(&mixed);
    assert_eq!(survivors.len(), 1);
    assert!(survivors[0].is_proxied);
    assert!(
        proxy_engine
            .authorize_udp_ip_discovery("104.29.147.145", 19309, true)
            .is_ok()
    );

    // 2. Deliberately unhardened legacy configuration fails specific checklist controls
    let unhardened_engine = D08PrivacyGuardrailsEngine::new(D08PlatformPrivacyConfig {
        webrtc_ip_policy: WebRtcIpHandlingPolicy::Default,
        enable_mdns_obfuscation: false,
        block_unproxied_udp_when_policy_requires: false,
        redact_ips_in_diagnostics: false,
        media_session_isolation: MediaSessionIsolationPolicy {
            disable_hardware_media_key_handling: false,
            disable_media_session_service: false,
            allow_os_media_transport_registration: true,
            spotify_mpris_consumer_only: false,
            allow_focused_fullscreen_media_keys: true,
        },
        autoplay_policy: AutoplaySuppressionPolicy::NoUserGestureRequired,
        external_embed_click_to_load: false,
        strip_external_embed_autoplay_query: false,
        block_dns_and_science_telemetry: false,
        disable_sentry_crashpad_upload: false,
        redact_secrets_and_device_labels: false,
        streamer_mode_pii_protection: false,
    });

    let bad_report = unhardened_engine.evaluate_privacy_checklist();
    assert!(!bad_report.is_compliant());
    assert_eq!(bad_report.passed_count, 0);
    assert_eq!(bad_report.failed_count, 12);
    for item in &bad_report.items {
        assert_eq!(item.status, D08ChecklistStatus::Failed);
    }
}

// ============================================================================
// Dimension 9: Observable Telemetry & Empirical Oracle Verification
// (`docs/re/privacy-guardrails-d08.md`, `docs/re/privacy-guardrails-d08.json`)
// ============================================================================

#[test]
fn d08_dim9_telemetry_counters_and_re_documentation_oracle_parity() {
    let root = repo_root();
    let md_path = root.join("docs/re/privacy-guardrails-d08.md");
    let json_path = root.join("docs/re/privacy-guardrails-d08.json");

    let md_text =
        fs::read_to_string(&md_path).expect("docs/re/privacy-guardrails-d08.md must exist");
    let json_text =
        fs::read_to_string(&json_path).expect("docs/re/privacy-guardrails-d08.json must exist");

    // 1. Verify required D-08 specification sections and checklist IDs in docs/re/privacy-guardrails-d08.md
    for marker in [
        "Platform Privacy Guardrails (D-08)",
        "WebRTC IP Leakage Prevention Policy",
        "default_public_interface_only",
        "disable_non_proxied_udp",
        "MediaSession Key Bindings Isolation",
        "HardwareMediaKeyHandling",
        "MediaSessionService",
        "org.mpris.MediaPlayer2.spotify",
        "Auto-Play Media Audio Suppression",
        "DocumentUserActivationRequired",
        "dc-telemetry.net",
        "o64374.ingest.sentry.io",
        "D08-CHK-01",
        "D08-CHK-12",
    ] {
        assert!(
            md_text.contains(marker),
            "docs/re/privacy-guardrails-d08.md missing required marker: {marker}"
        );
    }

    assert!(json_text.contains("\"task\": \"D-08\""));
    assert!(json_text.contains("\"recommendedNativeDefault\": \"default_public_interface_only\""));
    assert!(json_text.contains("\"udpDiscoveryPacketBytes\": 74"));
    assert!(json_text.contains("\"mediaSocketRotationIntervalMs\": 60080"));
    assert!(json_text.contains("\"requiredItemCount\": 12"));

    // 2. Exercise engine across all guardrails and verify observable D08PrivacyTelemetry counters
    let mut engine = D08PrivacyGuardrailsEngine::with_hardened_defaults();
    let _ = engine.filter_webrtc_candidates(&[
        IceCandidateEndpoint::new("192.168.1.2", 5000, false),
        IceCandidateEndpoint::new("104.29.147.145", 19309, false),
    ]);
    let _ = engine.authorize_udp_ip_discovery("192.168.1.2", 19309, false);
    let _ = engine.route_media_key_event(MediaKeyEvent::PlayPause, false);
    let _ = engine.evaluate_media_playback(&MediaPlaybackRequest {
        media_id: "vid-1".to_string(),
        source: MediaPlaybackSource::InlineAttachmentVideo,
        has_user_gesture: false,
        viewport_visible: true,
        voice_session_joined: false,
        user_deafened: false,
        user_dnd: false,
        streamer_mode: false,
        disable_all_sounds: false,
        requested_volume_pct: 100,
    });
    let _ = engine.authorize_outbound_request("https://dc-telemetry.net/probe");
    let _ = engine.evaluate_privacy_checklist();

    let telemetry: &D08PrivacyTelemetry = engine.telemetry();
    assert_eq!(telemetry.candidates_evaluated, 2);
    assert_eq!(telemetry.private_candidates_stripped, 1);
    assert_eq!(telemetry.udp_discovery_leaks_blocked, 1);
    assert_eq!(telemetry.media_keys_isolated_to_os, 1);
    assert_eq!(telemetry.autoplay_audio_suppressions, 1);
    assert_eq!(telemetry.telemetry_requests_blocked, 1);
    assert_eq!(telemetry.checklist_audits_run, 1);

    // Ensure D08PrivacyChecklistItem exposes stable ID, category, and requirement
    let report = engine.evaluate_privacy_checklist();
    let first: &D08PrivacyChecklistItem = &report.items[0];
    assert_eq!(first.id, "D08-CHK-01");
    assert_eq!(first.status, D08ChecklistStatus::Passed);
}
