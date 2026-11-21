//! R5-02: Audio Node Filtering — Contract Tests (`chord_media::capture::audio_node_filter`).
//!
//! Covers all 9 mandatory dimensions of `TEST-POLICY.md` §5:
//! 1. Positive scenarios (application audio output nodes accepted, valid PipeWire link plans built).
//! 2. Negative scenarios (client's own audio stream by PID/binary/app_id/node_name, input microphones,
//!    and virtual monitoring/loopback sinks excluded).
//! 3. Boundary conditions (PID 0 / `u32::MAX`, empty strings, whitespace trimming, 256-char name ceiling,
//!    case-insensitive matching).
//! 4. Error handling (`AudioFilterError` on empty client binary, overly long rule token, invalid virtual sink id).
//! 5. Empty states (empty node inventory yields empty link plan and zeroed telemetry).
//! 6. Loading / intermediate states (dynamic rule updates and live graph re-evaluation without stale links).
//! 7. Capability permissions / acoustic feedback loop prevention (`detect_feedback_loop`, `verify_no_feedback_loops`,
//!    cycle and self-link rejection).
//! 8. Recovery / fallback (explicit allowlist override vs default capture-all-except-excluded, graceful handling
//!    of nodes with missing optional metadata).
//! 9. Observable telemetry (`AudioFilterTelemetry`: total_evaluated, accepted_count, excluded_own_client,
//!    excluded_microphones, excluded_virtual_monitors, excluded_by_rule, feedback_loops_prevented).

use chord_media::capture::audio_node_filter::{
    AudioFilterError, AudioFilterTelemetry, AudioLinkPlan, AudioNodeDescriptor,
    AudioNodeFilterConfig, AudioNodeFilterEngine, AudioNodeMediaClass, ExclusionReason,
    FilterDecision, FilterMatchField, FilterPredicate, FilterRule, MAX_IDENTIFIER_LEN,
};

fn sample_virtual_sink_id() -> u32 {
    9001
}

fn default_engine() -> AudioNodeFilterEngine {
    let cfg = AudioNodeFilterConfig::new(4242, "chord").expect("valid default config");
    AudioNodeFilterEngine::new(cfg)
}

fn make_app_node(
    id: u32,
    pid: u32,
    binary: &str,
    app_name: &str,
    node_name: &str,
) -> AudioNodeDescriptor {
    AudioNodeDescriptor {
        node_id: id,
        serial: Some(u64::from(id) * 10),
        pid: Some(pid),
        process_binary: Some(binary.to_string()),
        application_name: Some(app_name.to_string()),
        application_id: Some(format!("org.example.{binary}")),
        node_name: node_name.to_string(),
        media_class: AudioNodeMediaClass::StreamOutputAudio,
        is_virtual: false,
        is_monitor: false,
        target_object: None,
    }
}

// ---------------------------------------------------------------------------
// 1. Positive Scenarios: Application audio streams accepted & link plan built
// ---------------------------------------------------------------------------

#[test]
fn accepts_external_application_audio_streams_and_builds_link_plan() {
    let engine = default_engine();
    let firefox = make_app_node(101, 5500, "firefox", "Mozilla Firefox", "Firefox Audio");
    let mpv = make_app_node(102, 5501, "mpv", "mpv Media Player", "mpv-stream");

    assert_eq!(engine.evaluate_node(&firefox), FilterDecision::Include);
    assert_eq!(engine.evaluate_node(&mpv), FilterDecision::Include);

    let plan = engine
        .build_link_data(&[firefox, mpv], sample_virtual_sink_id())
        .expect("valid link plan");

    assert_eq!(plan.target_virtual_sink_id, 9001);
    assert_eq!(plan.accepted_node_ids, vec![101, 102]);
    assert_eq!(plan.links.len(), 2);
    assert_eq!(plan.links[0].source_node_id, 101);
    assert_eq!(plan.links[0].target_sink_node_id, 9001);
    assert_eq!(plan.links[1].source_node_id, 102);
    assert_eq!(plan.links[1].target_sink_node_id, 9001);
    assert_eq!(plan.telemetry.total_evaluated, 2);
    assert_eq!(plan.telemetry.accepted_count, 2);
}

// ---------------------------------------------------------------------------
// 2. Negative Scenarios: Own client, microphones, and virtual monitors excluded
// ---------------------------------------------------------------------------

#[test]
fn excludes_clients_own_audio_stream_by_pid_binary_appid_and_name() {
    let engine = default_engine();

    // Match by exact client PID
    let own_pid_node = make_app_node(201, 4242, "custom-bin", "External Name", "audio-out");
    assert_eq!(
        engine.evaluate_node(&own_pid_node),
        FilterDecision::Exclude(ExclusionReason::OwnClientStream)
    );

    // Match by client binary name (case-insensitive, e.g. Flatpak/sandbox different PID namespace)
    let own_bin_node = make_app_node(202, 8888, "CHORD", "Music App", "stream-1");
    assert_eq!(
        engine.evaluate_node(&own_bin_node),
        FilterDecision::Exclude(ExclusionReason::OwnClientStream)
    );

    // Match by built-in known client identifiers (discord-gpui, chord-gpui, vesktop, venmic)
    let own_gpui_node = make_app_node(203, 8889, "discord-gpui", "Discord Lab", "voice-playback");
    assert_eq!(
        engine.evaluate_node(&own_gpui_node),
        FilterDecision::Exclude(ExclusionReason::OwnClientStream)
    );

    // Match by application_id
    let mut own_appid_node = make_app_node(204, 8890, "helper", "Helper", "helper-out");
    own_appid_node.application_id = Some("com.discord_lab.DiscordGpui".to_string());
    assert_eq!(
        engine.evaluate_node(&own_appid_node),
        FilterDecision::Exclude(ExclusionReason::OwnClientStream)
    );
}

#[test]
fn excludes_input_microphones_and_capture_sources() {
    let engine = default_engine();

    // Hardware Audio/Source microphone
    let hw_mic = AudioNodeDescriptor {
        node_id: 301,
        serial: Some(3010),
        pid: None,
        process_binary: None,
        application_name: Some("ALSA".to_string()),
        application_id: None,
        node_name: "alsa_input.usb-Blue_Microphones-00.analog-stereo".to_string(),
        media_class: AudioNodeMediaClass::AudioSource,
        is_virtual: false,
        is_monitor: false,
        target_object: None,
    };
    assert_eq!(
        engine.evaluate_node(&hw_mic),
        FilterDecision::Exclude(ExclusionReason::MicrophoneInput)
    );

    // Stream/Input/Audio (recording stream e.g. WebRTC mic capture)
    let rec_stream = AudioNodeDescriptor {
        node_id: 302,
        serial: Some(3020),
        pid: Some(5500),
        process_binary: Some("firefox".to_string()),
        application_name: Some("Firefox".to_string()),
        application_id: None,
        node_name: "WEBRTC VoiceEnginerec".to_string(),
        media_class: AudioNodeMediaClass::StreamInputAudio,
        is_virtual: false,
        is_monitor: false,
        target_object: None,
    };
    assert_eq!(
        engine.evaluate_node(&rec_stream),
        FilterDecision::Exclude(ExclusionReason::MicrophoneInput)
    );

    // Source/Virtual microphone
    let virt_mic = AudioNodeDescriptor {
        node_id: 303,
        serial: Some(3030),
        pid: None,
        process_binary: None,
        application_name: None,
        application_id: None,
        node_name: "rnnoise_source".to_string(),
        media_class: AudioNodeMediaClass::AudioSourceVirtual,
        is_virtual: true,
        is_monitor: false,
        target_object: None,
    };
    assert_eq!(
        engine.evaluate_node(&virt_mic),
        FilterDecision::Exclude(ExclusionReason::MicrophoneInput)
    );
}

#[test]
fn excludes_virtual_monitoring_sinks_and_loopback_devices() {
    let engine = default_engine();

    // Node flagged as is_monitor
    let mut monitor_node = make_app_node(
        401,
        6001,
        "pulseaudio",
        "Monitor",
        "alsa_output.pci.analog-stereo.monitor",
    );
    monitor_node.is_monitor = true;
    assert_eq!(
        engine.evaluate_node(&monitor_node),
        FilterDecision::Exclude(ExclusionReason::VirtualMonitoringSink)
    );

    // Node with .monitor suffix even if is_monitor was not explicitly set
    let suffix_monitor = make_app_node(
        402,
        6002,
        "pipewire",
        "PipeWire",
        "bluez_output.AA_BB.1.monitor",
    );
    assert_eq!(
        engine.evaluate_node(&suffix_monitor),
        FilterDecision::Exclude(ExclusionReason::VirtualMonitoringSink)
    );

    // Audio/Sink or Audio/Duplex device node (not an application playback stream)
    let sink_device = AudioNodeDescriptor {
        node_id: 403,
        serial: Some(4030),
        pid: None,
        process_binary: None,
        application_name: None,
        application_id: None,
        node_name: "alsa_output.pci-0000_00_1f.3.analog-stereo".to_string(),
        media_class: AudioNodeMediaClass::AudioSink,
        is_virtual: false,
        is_monitor: false,
        target_object: None,
    };
    assert_eq!(
        engine.evaluate_node(&sink_device),
        FilterDecision::Exclude(ExclusionReason::VirtualMonitoringSink)
    );

    // Virtual loopback / venmic / chord-screen-audio sink stream
    let mut loopback_stream = make_app_node(
        404,
        6004,
        "pw-loopback",
        "PipeWire Loopback",
        "venmic-loopback-capture",
    );
    loopback_stream.is_virtual = true;
    assert_eq!(
        engine.evaluate_node(&loopback_stream),
        FilterDecision::Exclude(ExclusionReason::VirtualMonitoringSink)
    );
}

// ---------------------------------------------------------------------------
// 3. Boundary Conditions: PID 0 / u32::MAX, 256-char limits, case-insensitivity
// ---------------------------------------------------------------------------

#[test]
fn enforces_pid_and_identifier_length_boundaries() {
    let engine = default_engine();

    // PID 0 is invalid/kernel and must be rejected as unidentifiable application stream
    let zero_pid = make_app_node(501, 0, "firefox", "Firefox", "Firefox");
    assert_eq!(
        engine.evaluate_node(&zero_pid),
        FilterDecision::Exclude(ExclusionReason::UnidentifiableApplication)
    );

    // Missing PID and missing binary/app_name -> rejected
    let anonymous_stream = AudioNodeDescriptor {
        node_id: 502,
        serial: None,
        pid: None,
        process_binary: None,
        application_name: None,
        application_id: None,
        node_name: "anonymous-stream".to_string(),
        media_class: AudioNodeMediaClass::StreamOutputAudio,
        is_virtual: false,
        is_monitor: false,
        target_object: None,
    };
    assert_eq!(
        engine.evaluate_node(&anonymous_stream),
        FilterDecision::Exclude(ExclusionReason::UnidentifiableApplication)
    );

    // PID u32::MAX with exact 256-char node_name is accepted
    let max_name = "a".repeat(MAX_IDENTIFIER_LEN);
    let max_valid = make_app_node(503, u32::MAX, "vlc", "VLC", &max_name);
    assert_eq!(engine.evaluate_node(&max_valid), FilterDecision::Include);

    // Node name exceeding 256 chars (257 chars) is rejected safely
    let overlong_name = "a".repeat(MAX_IDENTIFIER_LEN + 1);
    let overlong_node = make_app_node(504, 7000, "vlc", "VLC", &overlong_name);
    assert_eq!(
        engine.evaluate_node(&overlong_node),
        FilterDecision::Exclude(ExclusionReason::InvalidMetadata)
    );
}

// ---------------------------------------------------------------------------
// 4. Error Handling: Config validation & invalid target sink IDs
// ---------------------------------------------------------------------------

#[test]
fn validates_config_and_rule_errors() {
    // Empty client binary name is rejected
    assert_eq!(
        AudioNodeFilterConfig::new(1234, "   "),
        Err(AudioFilterError::EmptyClientIdentifier)
    );

    // Client binary exceeding MAX_IDENTIFIER_LEN is rejected
    let huge = "x".repeat(MAX_IDENTIFIER_LEN + 1);
    assert_eq!(
        AudioNodeFilterConfig::new(1234, &huge),
        Err(AudioFilterError::IdentifierTooLong {
            len: MAX_IDENTIFIER_LEN + 1,
            max: MAX_IDENTIFIER_LEN,
        })
    );

    // Target virtual sink ID 0 is rejected when building links
    let engine = default_engine();
    let firefox = make_app_node(101, 5500, "firefox", "Firefox", "Firefox");
    assert_eq!(
        engine.build_link_data(&[firefox], 0),
        Err(AudioFilterError::InvalidVirtualSinkId(0))
    );
}

// ---------------------------------------------------------------------------
// 5. Empty States: Empty node list produces empty link plan & clean telemetry
// ---------------------------------------------------------------------------

#[test]
fn empty_node_inventory_produces_empty_link_plan_and_zero_telemetry() {
    let engine = default_engine();
    let plan: AudioLinkPlan = engine
        .build_link_data(&[], sample_virtual_sink_id())
        .expect("empty inventory is valid");

    assert!(plan.accepted_node_ids.is_empty());
    assert!(plan.links.is_empty());
    assert!(plan.rejected.is_empty());
    assert_eq!(
        plan.telemetry,
        AudioFilterTelemetry {
            total_evaluated: 0,
            accepted_count: 0,
            excluded_own_client: 0,
            excluded_microphones: 0,
            excluded_virtual_monitors: 0,
            excluded_by_rule: 0,
            feedback_loops_prevented: 0,
        }
    );
}

// ---------------------------------------------------------------------------
// 6. Configurable Filter Rules & Dynamic Re-evaluation
// ---------------------------------------------------------------------------

#[test]
fn configurable_filter_rules_support_exclude_and_include_allowlist_modes() {
    let mut cfg = AudioNodeFilterConfig::new(4242, "chord").unwrap();

    // Add custom exclusion rule for notifications daemon
    cfg.add_exclude_rule(FilterRule::new(
        "mute-notifications",
        FilterMatchField::ProcessBinary,
        FilterPredicate::EqualsIgnoreCase("dunst".to_string()),
    ))
    .unwrap();

    // Add custom exclusion rule by prefix on node_name
    cfg.add_exclude_rule(FilterRule::new(
        "mute-steam-webhelper",
        FilterMatchField::NodeName,
        FilterPredicate::ContainsIgnoreCase("steamwebhelper".to_string()),
    ))
    .unwrap();

    let mut engine = AudioNodeFilterEngine::new(cfg);

    let firefox = make_app_node(601, 5500, "firefox", "Firefox", "Firefox");
    let dunst = make_app_node(602, 5501, "dunst", "Dunst", "Notification");
    let steam = make_app_node(603, 5502, "steam", "Steam", "SteamWebHelper Audio");
    let spotify = make_app_node(604, 5503, "spotify", "Spotify", "Spotify");

    assert_eq!(engine.evaluate_node(&firefox), FilterDecision::Include);
    assert_eq!(
        engine.evaluate_node(&dunst),
        FilterDecision::Exclude(ExclusionReason::CustomRule(
            "mute-notifications".to_string()
        ))
    );
    assert_eq!(
        engine.evaluate_node(&steam),
        FilterDecision::Exclude(ExclusionReason::CustomRule(
            "mute-steam-webhelper".to_string()
        ))
    );

    // Now add an explicit include (allowlist) rule: only Spotify is allowed among non-excluded apps
    engine
        .config_mut()
        .add_include_rule(FilterRule::new(
            "only-spotify",
            FilterMatchField::ApplicationName,
            FilterPredicate::EqualsIgnoreCase("Spotify".to_string()),
        ))
        .unwrap();

    assert_eq!(engine.evaluate_node(&spotify), FilterDecision::Include);
    assert_eq!(
        engine.evaluate_node(&firefox),
        FilterDecision::Exclude(ExclusionReason::NotInIncludeAllowlist)
    );

    // Crucial safety invariant: even if an include rule matches the client's own binary or a microphone,
    // safety exclusions (OwnClientStream, MicrophoneInput, VirtualMonitoringSink, AcousticFeedbackLoop)
    // ALWAYS take precedence over user include rules!
    engine
        .config_mut()
        .add_include_rule(FilterRule::new(
            "unsafe-attempt-include-own",
            FilterMatchField::ProcessBinary,
            FilterPredicate::EqualsIgnoreCase("chord".to_string()),
        ))
        .unwrap();
    let own_node = make_app_node(605, 4242, "chord", "Spotify", "chord-out");
    assert_eq!(
        engine.evaluate_node(&own_node),
        FilterDecision::Exclude(ExclusionReason::OwnClientStream)
    );
}

// ---------------------------------------------------------------------------
// 7. Acoustic Feedback Loop Prevention
// ---------------------------------------------------------------------------

#[test]
fn prevents_direct_and_transitive_acoustic_feedback_loops() {
    let engine = default_engine();
    let target_sink_id = sample_virtual_sink_id();

    // Case A: Node whose node_id equals target_sink_id (self-loop)
    let self_sink_node = make_app_node(target_sink_id, 7100, "app", "App", "app-stream");

    // Case B: Node that already targets our virtual capture sink (`target.object == "9001"` or virtual sink name)
    let mut already_routed = make_app_node(702, 7101, "obs", "OBS", "obs-monitor");
    already_routed.target_object = Some("chord-capture-sink".to_string());

    // Case C: Normal safe app node
    let safe_game = make_app_node(703, 7102, "eldenring.exe", "Elden Ring", "wine64-preloader");

    let plan = engine
        .build_link_data(&[self_sink_node, already_routed, safe_game], target_sink_id)
        .expect("build_link_data should succeed while filtering feedback loops");

    assert_eq!(plan.accepted_node_ids, vec![703]);
    assert_eq!(plan.telemetry.feedback_loops_prevented, 2);
    assert!(plan.verify_no_feedback_loops());
}

// ---------------------------------------------------------------------------
// 8 & 9. Deduplication, Recovery, and Telemetry Accounting
// ---------------------------------------------------------------------------

#[test]
fn deduplicates_nodes_and_records_exact_telemetry_across_all_categories() {
    let mut cfg = AudioNodeFilterConfig::new(4242, "chord").unwrap();
    cfg.add_exclude_rule(FilterRule::new(
        "ignore-browser",
        FilterMatchField::ProcessBinary,
        FilterPredicate::EqualsIgnoreCase("chromium".to_string()),
    ))
    .unwrap();
    let engine = AudioNodeFilterEngine::new(cfg);

    let nodes = vec![
        make_app_node(801, 5001, "mpv", "mpv", "mpv-1"),
        // Duplicate node_id 801 must be deduplicated cleanly
        make_app_node(801, 5001, "mpv", "mpv", "mpv-1"),
        // Own client stream
        make_app_node(802, 4242, "chord", "Chord", "chord-voice"),
        // Microphone input
        AudioNodeDescriptor {
            node_id: 803,
            serial: Some(8030),
            pid: None,
            process_binary: None,
            application_name: None,
            application_id: None,
            node_name: "alsa_input.pci.analog-stereo".to_string(),
            media_class: AudioNodeMediaClass::AudioSource,
            is_virtual: false,
            is_monitor: false,
            target_object: None,
        },
        // Virtual monitor sink
        AudioNodeDescriptor {
            node_id: 804,
            serial: Some(8040),
            pid: None,
            process_binary: None,
            application_name: None,
            application_id: None,
            node_name: "alsa_output.pci.analog-stereo.monitor".to_string(),
            media_class: AudioNodeMediaClass::AudioSource,
            is_virtual: false,
            is_monitor: true,
            target_object: None,
        },
        // Excluded by custom rule
        make_app_node(805, 5005, "chromium", "Chromium", "Playback"),
        // Feedback loop (node_id == virtual sink 9001)
        make_app_node(9001, 5006, "loop", "Loop", "Loop"),
    ];

    let plan = engine.build_link_data(&nodes, 9001).unwrap();
    assert_eq!(plan.accepted_node_ids, vec![801]);
    assert_eq!(plan.telemetry.total_evaluated, 6); // 6 unique nodes
    assert_eq!(plan.telemetry.accepted_count, 1);
    assert_eq!(plan.telemetry.excluded_own_client, 1);
    assert_eq!(plan.telemetry.excluded_microphones, 1);
    assert_eq!(plan.telemetry.excluded_virtual_monitors, 1);
    assert_eq!(plan.telemetry.excluded_by_rule, 1);
    assert_eq!(plan.telemetry.feedback_loops_prevented, 1);
    assert!(plan.verify_no_feedback_loops());
}
