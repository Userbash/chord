//! Contract tests for `RW51-EPIC1-T4`: Modular Tauri v2 `voice_plugin` encapsulating
//! the 10 voice & screen-sharing domain commands (`voice_connect`, `voice_disconnect`,
//! `voice_mute`, `voice_deaf`, `voice_rtc_status`, `voice_audio_devices` / `audio_devices`,
//! `voice_set_audio_device`, `voice_desktop_sources` / `screenshare_sources`,
//! `voice_stream_start` / `screenshare_start`, `voice_stream_stop` / `screenshare_stop`,
//! `voice_stream_watch` / `screenshare_watch`, `voice_stream_set_paused` / `screenshare_set_paused`),
//! bound to `chord-voice` and `chord-platform::audio_engine`, with typed `VoiceRtcStatusDto`
//! transmission to the frontend.

#![forbid(unsafe_code)]

#[path = "../src/voice_plugin.rs"]
pub mod voice_plugin;

use chord_client::dto::VoiceRtcStatusDto;
use chord_client::voice_stage::{ScreenShareSpec, StreamFps, StreamResolution};
use serde_json::json;

const VALID_GUILD_ID: &str = "111222333444555666";
const VALID_CHANNEL_ID: &str = "222333444555666777";
const VALID_USER_ID: &str = "333444555666777888";

fn run_async<F: std::future::Future>(future: F) -> F::Output {
    tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio current_thread runtime")
        .block_on(future)
}

#[test]
fn plugin_metadata_and_registered_command_inventory_match_contract() {
    assert_eq!(voice_plugin::PLUGIN_NAME, "voice");
    assert_eq!(voice_plugin::VOICE_PLUGIN_COMMANDS.len(), 10);

    for expected in [
        "voice_connect",
        "voice_disconnect",
        "voice_mute",
        "voice_deaf",
        "voice_rtc_status",
        "voice_audio_devices",
        "voice_set_audio_device",
        "voice_desktop_sources",
        "voice_stream_start",
        "voice_stream_stop",
    ] {
        assert!(
            voice_plugin::VOICE_PLUGIN_COMMANDS.contains(&expected),
            "expected `{expected}` in VOICE_PLUGIN_COMMANDS"
        );
    }

    let _plugin_init = voice_plugin::init::<tauri::Wry>();
    let _plugin_voice = voice_plugin::voice::<tauri::Wry>();
}

#[test]
fn chord_voice_core_and_chord_platform_audio_engine_bindings_are_active() {
    let meta = voice_plugin::voice_engine_metadata();
    assert_eq!(meta.dave_protocol_version, 1);
    assert_eq!(meta.media_encryption_mode, "aead_aes256_gcm_rtpsize");
    assert_eq!(meta.default_phase, "disconnected");
    assert_eq!(meta.select_protocol_op, 1);
    assert_eq!(meta.speaking_op, 5);
    assert_eq!(meta.dave_mls_welcome_op, 30);

    assert_eq!(
        voice_plugin::evaluate_rtc_connection_quality(24.0, 0.2),
        "fine"
    );
    assert_eq!(
        voice_plugin::evaluate_rtc_connection_quality(310.0, 1.0),
        "average"
    );
    assert_eq!(
        voice_plugin::evaluate_rtc_connection_quality(620.0, 0.5),
        "bad"
    );
    assert_eq!(
        voice_plugin::evaluate_rtc_connection_quality(-1.0, 0.0),
        "unknown"
    );

    let inventory = voice_plugin::native_audio_inventory();
    assert!(!inventory.inputs.is_empty());
    assert!(!inventory.outputs.is_empty());
    assert!(!inventory.default_input.is_empty());
    assert!(!inventory.default_output.is_empty());

    // Playing valid and invalid sound cues through chord_platform::audio_engine
    assert!(voice_plugin::play_voice_sound_cue("mute").is_ok());
    assert!(voice_plugin::play_voice_sound_cue("deafen").is_ok());
    assert!(voice_plugin::play_voice_sound_cue("stream_started").is_ok());
    assert!(voice_plugin::play_voice_sound_cue("nonexistent_cue").is_err());
}

#[test]
fn mute_and_deaf_resolution_enforces_effective_mute_invariant() {
    assert!(voice_plugin::verify_voice_protocol_gate(1, false, false));
    assert!(voice_plugin::verify_voice_protocol_gate(1, true, false));
    assert!(voice_plugin::verify_voice_protocol_gate(1, false, true));
    assert!(!voice_plugin::verify_voice_protocol_gate(1, true, true));
    assert!(!voice_plugin::verify_voice_protocol_gate(0, false, false));
    assert!(!voice_plugin::verify_voice_protocol_gate(2, false, false));

    let state_unmuted = voice_plugin::resolve_effective_mute_deaf(false, false);
    assert!(!state_unmuted.self_mute);
    assert!(!state_unmuted.self_deaf);
    assert!(!state_unmuted.effective_mute);
    assert_eq!(state_unmuted.speaking_flags, 1);

    let state_muted = voice_plugin::resolve_effective_mute_deaf(true, false);
    assert!(state_muted.self_mute);
    assert!(!state_muted.self_deaf);
    assert!(state_muted.effective_mute);
    assert_eq!(state_muted.speaking_flags, 0);

    let state_deafened = voice_plugin::resolve_effective_mute_deaf(false, true);
    assert!(!state_deafened.self_mute);
    assert!(state_deafened.self_deaf);
    assert!(
        state_deafened.effective_mute,
        "self_deaf must force effective_mute = true"
    );
    assert_eq!(state_deafened.speaking_flags, 0);

    let mute_cmd = run_async(voice_plugin::voice_mute(true)).expect("voice_mute(true)");
    assert_eq!(mute_cmd["ok"], json!(true));
    assert_eq!(mute_cmd["selfMute"], json!(true));
    assert_eq!(mute_cmd["effectiveMute"], json!(true));

    let deaf_cmd = run_async(voice_plugin::voice_deaf(true)).expect("voice_deaf(true)");
    assert_eq!(deaf_cmd["ok"], json!(true));
    assert_eq!(deaf_cmd["selfDeaf"], json!(true));
    assert_eq!(deaf_cmd["effectiveMute"], json!(true));

    let unmute_cmd = run_async(voice_plugin::voice_mute(false)).expect("voice_mute(false)");
    assert_eq!(unmute_cmd["ok"], json!(true));
    assert_eq!(unmute_cmd["selfMute"], json!(false));

    let undeaf_cmd = run_async(voice_plugin::voice_deaf(false)).expect("voice_deaf(false)");
    assert_eq!(undeaf_cmd["ok"], json!(true));
    assert_eq!(undeaf_cmd["selfDeaf"], json!(false));
    assert_eq!(undeaf_cmd["effectiveMute"], json!(false));
}

#[test]
fn rtc_status_dto_extraction_normalizes_camel_and_snake_case_payloads() {
    let camel_raw = json!({
        "connected": true,
        "state": "connected",
        "guildId": VALID_GUILD_ID,
        "channelId": VALID_CHANNEL_ID,
        "endpoint": "finland10153.discord.media:2053",
        "ssrc": 4242,
        "txPackets": 1200,
        "rxPackets": 3400,
        "rttMs": 28,
        "lastError": ""
    });

    let dto = voice_plugin::extract_rtc_status_dto(&camel_raw);
    assert_eq!(
        dto,
        VoiceRtcStatusDto {
            connected: true,
            state: "connected".to_string(),
            guild_id: Some(VALID_GUILD_ID.to_string()),
            channel_id: Some(VALID_CHANNEL_ID.to_string()),
            endpoint: Some("finland10153.discord.media:2053".to_string()),
            ssrc: 4242,
            tx_packets: 1200,
            rx_packets: 3400,
            rtt_ms: Some(28),
            last_error: None,
        }
    );

    let serialized = serde_json::to_value(&dto).expect("serialize VoiceRtcStatusDto");
    assert_eq!(serialized["connected"], json!(true));
    assert_eq!(serialized["state"], json!("connected"));
    assert_eq!(serialized["channelId"], json!(VALID_CHANNEL_ID));
    assert_eq!(serialized["guildId"], json!(VALID_GUILD_ID));
    assert_eq!(
        serialized["endpoint"],
        json!("finland10153.discord.media:2053")
    );
    assert_eq!(serialized["ssrc"], json!(4242));
    assert_eq!(serialized["txPackets"], json!(1200));
    assert_eq!(serialized["rxPackets"], json!(3400));
    assert_eq!(serialized["rttMs"], json!(28));
    assert_eq!(serialized["lastError"], json!(null));

    let snake_raw = json!({
        "connected": false,
        "state": "signaling",
        "guild_id": "@me",
        "channel_id": "   ",
        "endpoint": "  ",
        "ssrc": 99,
        "tx_packets": 15,
        "rx_packets": 30,
        "lastPingMs": 45,
        "error": "UDP socket timeout"
    });

    let snake_dto = voice_plugin::extract_rtc_status_dto(&snake_raw);
    assert!(!snake_dto.connected);
    assert_eq!(snake_dto.state, "signaling");
    assert_eq!(snake_dto.guild_id, None);
    assert_eq!(snake_dto.channel_id, None);
    assert_eq!(snake_dto.endpoint, None);
    assert_eq!(snake_dto.ssrc, 99);
    assert_eq!(snake_dto.tx_packets, 15);
    assert_eq!(snake_dto.rx_packets, 30);
    assert_eq!(snake_dto.rtt_ms, Some(45));
    assert_eq!(snake_dto.last_error.as_deref(), Some("UDP socket timeout"));
}

#[test]
fn live_voice_rtc_status_and_dto_commands_succeed() {
    let raw = run_async(voice_plugin::voice_rtc_status()).expect("voice_rtc_status JSON");
    assert!(
        raw.is_object(),
        "voice_rtc_status must return a JSON object"
    );

    let deserialized: VoiceRtcStatusDto = serde_json::from_value(raw.clone())
        .expect("voice_rtc_status payload must deserialize cleanly into VoiceRtcStatusDto");
    let dto = run_async(voice_plugin::voice_rtc_status_dto()).expect("voice_rtc_status_dto");
    assert_eq!(deserialized.state, dto.state);
    assert_eq!(deserialized.connected, dto.connected);
    assert!(!dto.state.is_empty());
}

#[test]
fn guild_and_channel_normalization_and_stream_key_builders_handle_edge_cases() {
    assert_eq!(voice_plugin::normalize_guild_id(None), None);
    assert_eq!(voice_plugin::normalize_guild_id(Some("")), None);
    assert_eq!(voice_plugin::normalize_guild_id(Some("  @me ")), None);
    assert_eq!(voice_plugin::normalize_guild_id(Some("null")), None);
    assert_eq!(voice_plugin::normalize_guild_id(Some("nil")), None);
    assert_eq!(
        voice_plugin::normalize_guild_id(Some(" 111222333444555666 ")),
        Some("111222333444555666".to_string())
    );

    assert_eq!(voice_plugin::normalize_channel_id(None), None);
    assert_eq!(voice_plugin::normalize_channel_id(Some("   ")), None);
    assert_eq!(voice_plugin::normalize_channel_id(Some("null")), None);
    assert_eq!(
        voice_plugin::normalize_channel_id(Some(" 222333444555666777 ")),
        Some("222333444555666777".to_string())
    );

    assert_eq!(
        voice_plugin::build_active_stream_key(
            Some(VALID_GUILD_ID.to_string()),
            Some(VALID_CHANNEL_ID.to_string()),
            VALID_USER_ID
        ),
        format!("guild:{VALID_GUILD_ID}:{VALID_CHANNEL_ID}:{VALID_USER_ID}")
    );
    assert_eq!(
        voice_plugin::build_active_stream_key(
            Some("@me".to_string()),
            Some(VALID_CHANNEL_ID.to_string()),
            ""
        ),
        format!("call:{VALID_CHANNEL_ID}:me")
    );
    assert_eq!(
        voice_plugin::build_active_stream_key(None, None, VALID_USER_ID),
        ""
    );
}

#[test]
fn screenshare_stop_watch_and_pause_commands_validate_keys_and_build_gateway_opcodes() {
    let guild_stream_key = format!("guild:{VALID_GUILD_ID}:{VALID_CHANNEL_ID}:{VALID_USER_ID}");

    // Stop with explicit stream_key -> OP 19
    let stop_res = run_async(voice_plugin::screenshare_stop(
        Some(guild_stream_key.clone()),
        None,
    ))
    .expect("screenshare_stop with valid key");
    assert_eq!(stop_res["ok"], json!(true));
    assert_eq!(stop_res["streamKey"], json!(guild_stream_key));
    assert_eq!(stop_res["payload"]["op"], json!(19));
    assert_eq!(
        stop_res["payload"]["d"]["stream_key"],
        json!(guild_stream_key)
    );

    // Stop with None when no active call -> fallback OP 19 with empty stream_key
    let stop_empty = run_async(voice_plugin::voice_stream_stop(None, None))
        .expect("voice_stream_stop without active stream");
    assert_eq!(stop_empty["ok"], json!(true));
    assert_eq!(stop_empty["payload"]["op"], json!(19));

    // Watch missing key -> explicit English error
    let watch_missing = run_async(voice_plugin::voice_stream_watch(None, None))
        .expect_err("watch without stream_key must fail");
    assert_eq!(
        watch_missing,
        "missing required parameter stream_key / streamKey"
    );

    // Pause missing key -> explicit English error
    let pause_missing = run_async(voice_plugin::voice_stream_set_paused(None, None, true))
        .expect_err("set_paused without stream_key must fail");
    assert_eq!(
        pause_missing,
        "missing required parameter stream_key / streamKey"
    );

    // Invalid stream key format rejected before queueing
    assert!(run_async(voice_plugin::screenshare_watch(
        Some("invalid:key".to_string()),
        None
    ))
    .is_err());
    assert!(run_async(voice_plugin::screenshare_set_paused(
        None,
        Some("invalid:key".to_string()),
        false
    ))
    .is_err());

    // Invalid ScreenShareSpec rejected by screenshare_start / voice_stream_start
    let invalid_spec = ScreenShareSpec {
        guild_id: Some("0".to_string()),
        channel_id: VALID_CHANNEL_ID.to_string(),
        source_id: "screen:0".to_string(),
        resolution: StreamResolution::P1080,
        fps: StreamFps::Fps60,
        share_audio: true,
    };
    assert!(run_async(voice_plugin::screenshare_start(invalid_spec)).is_err());
}

#[test]
fn main_rs_wires_voice_plugin_and_voice_plugin_is_strictly_english() {
    let main_src = include_str!("../src/main.rs");
    let plugin_src = include_str!("../src/voice_plugin.rs");

    assert!(
        main_src.contains("pub mod voice_plugin;"),
        "main.rs must declare pub mod voice_plugin"
    );
    assert!(
        main_src.contains(".plugin(voice_plugin::init())"),
        "main.rs must register .plugin(voice_plugin::init())"
    );
    assert!(
        main_src.contains("voice_plugin::disconnect_on_window_close()"),
        "main.rs CloseRequested handler must delegate voice teardown to voice_plugin::disconnect_on_window_close()"
    );

    let has_cyrillic = plugin_src
        .chars()
        .any(|ch| ('\u{0400}'..='\u{04FF}').contains(&ch));
    assert!(
        !has_cyrillic,
        "voice_plugin.rs must contain strictly English code, comments, and error messages"
    );
}
