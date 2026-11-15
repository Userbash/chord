//! Modular Tauri plugin (`voice`) encapsulating voice, audio device, and screen-share IPC commands.
//!
//! Provides `voice_connect`, `voice_disconnect`, `voice_mute`, `voice_deaf`, `voice_rtc_status`,
//! `voice_audio_devices`, `voice_set_audio_device`, `voice_desktop_sources`, `voice_stream_start`,
//! `voice_stream_stop`, `voice_stream_watch`, and `voice_stream_set_paused`, bound to `chord-voice`
//! and `chord-platform::audio_engine`.

use chord_client::data;
use chord_client::dto::VoiceRtcStatusDto;
use chord_client::voice_diagnostics::ConnectionQuality;
use chord_platform::audio_engine::{
    global_audio_engine, AudioDeviceInventory, AudioDirection, SoundCue,
};
use chord_voice::{
    SpeakingFlags, VoiceOp, VoiceSessionPhase, DAVE_PROTOCOL_VERSION, MEDIA_ENCRYPTION_MODE,
};
use serde::{Deserialize, Serialize};
use tauri::Manager;

/// Validates that the negotiated DAVE protocol version matches the active engine contract
/// and that microphone transmission is suppressed while deafened.
#[allow(unused_parens)]
#[must_use]
pub fn verify_voice_protocol_gate(
    version: u16,
    speaking_requested: bool,
    self_deaf: bool,
) -> bool {
    if (version != 1) {
        return false;
    }
    if (self_deaf && speaking_requested) {
        return false;
    }
    return true;
}

use std::sync::atomic::{AtomicI32, AtomicU32, AtomicU64, Ordering};
use std::sync::Mutex;

/// Canonical Tauri plugin identifier for the voice & screen-sharing domain.
pub const PLUGIN_NAME: &str = "voice";

/// Canonical list of the 10 primary voice, audio device, and screen-sharing domain commands.
pub const VOICE_PLUGIN_COMMANDS: [&str; 10] = [
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
];

/// Debounce: minimum milliseconds between consecutive `voice_stream_start` calls.
static LAST_STREAM_START_MS: AtomicU64 = AtomicU64::new(0);

/// PipeWire node id obtained from the last XDG Desktop Portal screencast session.
/// `0` = not yet acquired.
static LAST_PIPEWIRE_NODE_ID: AtomicU32 = AtomicU32::new(0);

/// PipeWire fd obtained from the last XDG Desktop Portal screencast session.
/// `-1` = not yet acquired.
static LAST_PIPEWIRE_FD: AtomicI32 = AtomicI32::new(-1);

/// Active XDG ScreenCast portal session; must remain alive for the duration of the stream,
/// otherwise the portal destroys the PipeWire node after ~1.5 seconds.
static ACTIVE_PORTAL_SESSION: Mutex<
    Option<ashpd::desktop::Session<ashpd::desktop::screencast::Screencast>>,
> = Mutex::new(None);

/// Metadata snapshot describing the underlying `chord-voice` engine capabilities and opcodes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceEngineMetadataDto {
    pub dave_protocol_version: u16,
    pub media_encryption_mode: String,
    pub default_phase: String,
    pub select_protocol_op: u8,
    pub speaking_op: u8,
    pub dave_mls_welcome_op: u8,
}

/// Resolved microphone mute and deafen state enforcing `effective_mute = self_mute || self_deaf`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct VoiceMuteDeafStateDto {
    pub self_mute: bool,
    pub self_deaf: bool,
    pub effective_mute: bool,
    pub speaking_flags: u32,
}

/// Returns the static `chord-voice` protocol metadata (`DAVE v1`, `aead_aes256_gcm_rtpsize`, opcodes).
#[must_use]
pub fn voice_engine_metadata() -> VoiceEngineMetadataDto {
    let initial_phase = match VoiceSessionPhase::Closed {
        VoiceSessionPhase::Identifying => "identifying",
        VoiceSessionPhase::DiscoveringUdp => "discovering_udp",
        VoiceSessionPhase::SelectingProtocol => "selecting_protocol",
        VoiceSessionPhase::Established => "connected",
        VoiceSessionPhase::Resuming => "resuming",
        VoiceSessionPhase::TimedOut => "timed_out",
        VoiceSessionPhase::Closed => "disconnected",
    };
    VoiceEngineMetadataDto {
        dave_protocol_version: u16::from(DAVE_PROTOCOL_VERSION),
        media_encryption_mode: MEDIA_ENCRYPTION_MODE.to_string(),
        default_phase: initial_phase.to_string(),
        select_protocol_op: VoiceOp::SelectProtocol.as_u8(),
        speaking_op: VoiceOp::Speaking.as_u8(),
        dave_mls_welcome_op: VoiceOp::MlsWelcome.as_u8(),
    }
}

/// Evaluates RTC connection quality (`"fine"`, `"average"`, `"bad"`, `"unknown"`) via `ConnectionQuality`.
#[must_use]
pub fn evaluate_rtc_connection_quality(rtt_ms: f64, loss_pct: f64) -> &'static str {
    let quality = if !rtt_ms.is_finite() || rtt_ms < 0.0 {
        ConnectionQuality::evaluate(false, None, Some(loss_pct))
    } else {
        ConnectionQuality::evaluate(true, Some(rtt_ms.round() as u64), Some(loss_pct))
    };
    match quality {
        ConnectionQuality::Fine => "fine",
        ConnectionQuality::Average => "average",
        ConnectionQuality::Bad => "bad",
        ConnectionQuality::Unknown => "unknown",
    }
}

/// Queries the cross-platform `cpal` audio device inventory directly from `chord_platform::audio_engine`.
#[must_use]
pub fn native_audio_inventory() -> AudioDeviceInventory {
    global_audio_engine().enumerate_devices()
}

/// Plays a synthesized UI voice cue (`"mute"`, `"unmute"`, `"deafen"`, `"undeafen"`, `"disconnect"`,
/// `"stream_started"`, `"stream_ended"`, `"user_join"`, `"user_leave"`) via `chord_platform::audio_engine`.
pub fn play_voice_sound_cue(cue_name: &str) -> Result<(), String> {
    let normalized = cue_name.trim().to_ascii_lowercase();
    let is_known = matches!(
        normalized.as_str(),
        "join"
            | "user_join"
            | "leave"
            | "user_leave"
            | "user_moved"
            | "moved"
            | "disconnect"
            | "mute"
            | "unmute"
            | "deafen"
            | "deaf"
            | "undeafen"
            | "undeaf"
            | "stream_on"
            | "stream_started"
            | "camera_on"
            | "stream_off"
            | "stream_ended"
            | "camera_off"
            | "stream_user_joined"
            | "stream_user_left"
            | "message"
            | "message1"
            | "mention"
    );
    if !is_known {
        return Err(format!("unknown voice sound cue: {cue_name}"));
    }
    let cue = SoundCue::from_name(&normalized);
    let _ = global_audio_engine().play_sound_cue(cue.as_str(), None, Some(false));
    Ok(())
}

/// Resolves effective microphone mute state and `chord-voice` speaking flags from `(self_mute, self_deaf)`.
#[must_use]
pub fn resolve_effective_mute_deaf(self_mute: bool, self_deaf: bool) -> VoiceMuteDeafStateDto {
    let effective_mute = self_mute || self_deaf;
    let speaking_flags = if effective_mute {
        SpeakingFlags::NONE.bits()
    } else {
        SpeakingFlags::MICROPHONE.bits()
    };
    VoiceMuteDeafStateDto {
        self_mute,
        self_deaf,
        effective_mute,
        speaking_flags,
    }
}

/// Normalizes an optional guild ID, filtering out empty strings and DM sentinel values (`"@me"`, `"null"`, `"nil"`).
#[must_use]
pub fn normalize_guild_id(guild_id: Option<&str>) -> Option<String> {
    guild_id
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "@me" && *s != "null" && *s != "nil")
        .map(str::to_string)
}

/// Normalizes an optional channel ID, filtering out empty strings and sentinel values (`"null"`, `"nil"`).
#[must_use]
pub fn normalize_channel_id(channel_id: Option<&str>) -> Option<String> {
    channel_id
        .map(str::trim)
        .filter(|s| !s.is_empty() && *s != "null" && *s != "nil")
        .map(str::to_string)
}

/// Builds the canonical Go Live stream key (`"guild:{gid}:{cid}:{uid}"` or `"call:{cid}:{uid}"`).
#[must_use]
pub fn build_active_stream_key(
    guild_id: Option<String>,
    channel_id: Option<String>,
    self_id: &str,
) -> String {
    let uid = if self_id.trim().is_empty() {
        "me"
    } else {
        self_id.trim()
    };
    match (guild_id, channel_id) {
        (Some(g), Some(c)) if !g.is_empty() && g != "@me" && g != "call" => {
            format!("guild:{g}:{c}:{uid}")
        }
        (_, Some(c)) if !c.is_empty() => format!("call:{c}:{uid}"),
        _ => String::new(),
    }
}

/// Extracts a strongly-typed `VoiceRtcStatusDto` from a raw RTC status JSON value,
/// normalizing both `camelCase` and `snake_case` wire fields.
#[must_use]
pub fn extract_rtc_status_dto(raw: &serde_json::Value) -> VoiceRtcStatusDto {
    let connected = raw
        .get("connected")
        .and_then(serde_json::Value::as_bool)
        .unwrap_or(false);
    let state = raw
        .get("state")
        .or_else(|| raw.get("phase"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(if connected {
            "connected"
        } else {
            "disconnected"
        })
        .to_string();
    let guild_id = raw
        .get("guildId")
        .or_else(|| raw.get("guild_id"))
        .and_then(serde_json::Value::as_str)
        .and_then(|s| normalize_guild_id(Some(s)));
    let channel_id = raw
        .get("channelId")
        .or_else(|| raw.get("channel_id"))
        .and_then(serde_json::Value::as_str)
        .and_then(|s| normalize_channel_id(Some(s)));
    let endpoint = raw
        .get("endpoint")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);
    let ssrc = raw
        .get("ssrc")
        .and_then(serde_json::Value::as_u64)
        .and_then(|v| u32::try_from(v).ok())
        .unwrap_or(0);
    let tx_packets = raw
        .get("txPackets")
        .or_else(|| raw.get("tx_packets"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let rx_packets = raw
        .get("rxPackets")
        .or_else(|| raw.get("rx_packets"))
        .and_then(serde_json::Value::as_u64)
        .unwrap_or(0);
    let rtt_ms = raw
        .get("rttMs")
        .or_else(|| raw.get("rtt_ms"))
        .or_else(|| raw.get("lastPingMs"))
        .or_else(|| raw.get("averagePingMs"))
        .and_then(serde_json::Value::as_u64);
    let last_error = raw
        .get("lastError")
        .or_else(|| raw.get("last_error"))
        .or_else(|| raw.get("error"))
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string);

    VoiceRtcStatusDto {
        connected,
        state,
        guild_id,
        channel_id,
        endpoint,
        ssrc,
        tx_packets,
        rx_packets,
        rtt_ms,
        last_error,
    }
}

async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, String> {
    tokio::task::spawn_blocking(work)
        .await
        .map_err(|error| format!("blocking task failed: {error}"))
}

/// Acquires a PipeWire node id via the XDG Desktop Portal ScreenCast API on Wayland.
async fn acquire_pipewire_node_id() -> Result<(u32, i32), String> {
    if std::env::var("WAYLAND_DISPLAY").is_err() {
        return Err("not Wayland".to_string());
    }
    use ashpd::desktop::screencast::{CursorMode, Screencast, SelectSourcesOptions, SourceType};
    use enumflags2::BitFlags;
    let proxy = Screencast::new()
        .await
        .map_err(|e| format!("portal proxy: {e}"))?;
    let session = proxy
        .create_session(Default::default())
        .await
        .map_err(|e| format!("create session: {e}"))?;
    let opts = SelectSourcesOptions::default()
        .set_cursor_mode(CursorMode::Embedded)
        .set_sources(BitFlags::from(SourceType::Monitor) | BitFlags::from(SourceType::Window));
    proxy
        .select_sources(&session, opts)
        .await
        .map_err(|e| format!("select sources: {e}"))?;
    let response = proxy
        .start(
            &session,
            None::<&ashpd::WindowIdentifier>,
            Default::default(),
        )
        .await
        .map_err(|e| format!("start: {e}"))?
        .response()
        .map_err(|e| format!("response: {e}"))?;
    let node_id = response
        .streams()
        .first()
        .map(|s| s.pipe_wire_node_id())
        .ok_or_else(|| "no streams selected".to_string())?;
    let fd = match proxy
        .open_pipe_wire_remote(&session, Default::default())
        .await
    {
        Ok(owned_fd) => {
            use std::os::unix::io::IntoRawFd;
            let raw = owned_fd.into_raw_fd();
            println!("[IPC-TRACE] acquire_pipewire: got fd={raw} node_id={node_id}");
            raw
        }
        Err(e) => {
            println!(
                "[IPC-TRACE] acquire_pipewire: open_pipe_wire_remote failed ({e}), using fd=-1"
            );
            -1
        }
    };
    if let Ok(mut guard) = ACTIVE_PORTAL_SESSION.lock() {
        *guard = Some(session);
    }
    if fd >= 0 {
        chord_platform::screencast::allow_ptrace_yama();
        let pid = std::process::id();
        let _ = std::fs::write(
            "/dev/shm/discord_pipewire_fd.txt",
            format!("{pid}:{fd}\n").as_bytes(),
        );
        chord_platform::screencast::start_pipewire_fd_server(fd);
    }
    Ok((node_id, fd))
}

/// Connects to or updates the active live voice channel and mute/deaf/video/stream flags.
pub async fn voice_connect(
    guild_id: Option<String>,
    channel_id: Option<String>,
    self_mute: Option<bool>,
    self_deaf: Option<bool>,
    self_video: Option<bool>,
    self_stream: Option<bool>,
) -> Result<serde_json::Value, String> {
    blocking(move || {
        let (cur_gid, cur_cid) = data::current_live_voice_target();
        let (cur_mute, cur_deaf) = data::current_live_voice_mute_deaf();
        let (cur_video, cur_stream) = data::current_live_voice_video_stream();
        let norm_gid = normalize_guild_id(guild_id.as_deref());
        let norm_cid = normalize_channel_id(channel_id.as_deref());
        let same_channel = norm_cid.is_some() && norm_cid == cur_cid && norm_gid == cur_gid;
        let eff_mute = self_mute.unwrap_or(if same_channel { cur_mute } else { false });
        let eff_deaf = self_deaf.unwrap_or(if same_channel { cur_deaf } else { false });
        let eff_video = self_video.unwrap_or(if same_channel { cur_video } else { false });
        let eff_stream = self_stream.unwrap_or(if same_channel { cur_stream } else { false });
        let resolved_audio = resolve_effective_mute_deaf(eff_mute, eff_deaf);
        global_audio_engine().set_mute_or_deaf(AudioDirection::Input, resolved_audio.effective_mute);
        global_audio_engine().set_mute_or_deaf(AudioDirection::Output, resolved_audio.self_deaf);
        if !same_channel
            || eff_mute != cur_mute
            || eff_deaf != cur_deaf
            || eff_video != cur_video
            || eff_stream != cur_stream
        {
            println!(
                "[IPC-TRACE] voice_connect guild_id={:?} channel_id={:?} self_mute={:?} self_deaf={:?} self_video={:?} self_stream={:?}",
                guild_id, channel_id, eff_mute, eff_deaf, eff_video, eff_stream
            );
        }
        data::connect_or_update_live_voice_ext(
            guild_id,
            channel_id,
            eff_mute,
            eff_deaf,
            eff_video,
            eff_stream,
        )
    })
    .await?
}

/// Disconnects from the active voice channel and tears down any active screen share.
pub async fn voice_disconnect(guild_id: Option<String>) -> Result<serde_json::Value, String> {
    println!("[IPC-TRACE] voice_disconnect guild_id={guild_id:?}");
    blocking(move || {
        let (gid, cid) = data::current_live_voice_target();
        let self_id = data::current_live_self_uid();
        let stop_key = build_active_stream_key(gid, cid, &self_id);
        if !stop_key.is_empty() {
            let _ = chord_core::ws::queue_live_stream_delete(&stop_key);
        }
        chord_core::ws::clear_last_stream_key();
        data::reset_live_stream_state();
        let res = data::connect_or_update_live_voice(guild_id, None, false, false);
        let _ = chord_core::ws::queue_live_voice_state_ext(
            None, None, false, false, false, false, None,
        );
        res
    })
    .await?
}

/// Synchronously disconnects active live voice state during window close (`CloseRequested`).
pub fn disconnect_on_window_close() {
    let (active_gid, _) = data::active_voice_guild_and_channel();
    let _ = data::connect_or_update_live_voice(active_gid.clone(), None, false, false);
    let _ = chord_core::ws::queue_live_voice_state(active_gid, None, false, false);
}

/// Updates the local microphone mute state (`self_mute`) on the active voice session
/// and synchronizes `chord_platform::audio_engine` input mute + cue playback.
pub async fn voice_mute(self_mute: bool) -> Result<serde_json::Value, String> {
    blocking(move || {
        let (gid, cid) = data::current_live_voice_target();
        let (_, cur_deaf) = data::current_live_voice_mute_deaf();
        let (cur_video, cur_stream) = data::current_live_voice_video_stream();
        let resolved = resolve_effective_mute_deaf(self_mute, cur_deaf);
        global_audio_engine().set_mute_or_deaf(AudioDirection::Input, resolved.effective_mute);
        let cue = if self_mute {
            SoundCue::Mute
        } else {
            SoundCue::Unmute
        };
        let _ = global_audio_engine().play_sound_cue(cue.as_str(), None, Some(false));
        if cid.is_some() {
            data::connect_or_update_live_voice_ext(
                gid,
                cid,
                resolved.self_mute,
                resolved.self_deaf,
                cur_video,
                cur_stream,
            )
        } else {
            Ok(serde_json::json!({
                "ok": true,
                "selfMute": resolved.self_mute,
                "selfDeaf": resolved.self_deaf,
                "effectiveMute": resolved.effective_mute,
                "speakingFlags": resolved.speaking_flags,
            }))
        }
    })
    .await?
}

/// Updates the local audio deafen state (`self_deaf`) on the active voice session,
/// enforcing `effective_mute = self_mute || self_deaf` on `chord_platform::audio_engine`.
pub async fn voice_deaf(self_deaf: bool) -> Result<serde_json::Value, String> {
    blocking(move || {
        let (gid, cid) = data::current_live_voice_target();
        let (cur_mute, _) = data::current_live_voice_mute_deaf();
        let (cur_video, cur_stream) = data::current_live_voice_video_stream();
        let resolved = resolve_effective_mute_deaf(cur_mute, self_deaf);
        global_audio_engine().set_mute_or_deaf(AudioDirection::Input, resolved.effective_mute);
        global_audio_engine().set_mute_or_deaf(AudioDirection::Output, resolved.self_deaf);
        let cue = if self_deaf {
            SoundCue::Deafen
        } else {
            SoundCue::Undeafen
        };
        let _ = global_audio_engine().play_sound_cue(cue.as_str(), None, Some(false));
        if cid.is_some() {
            data::connect_or_update_live_voice_ext(
                gid,
                cid,
                resolved.self_mute,
                resolved.self_deaf,
                cur_video,
                cur_stream,
            )
        } else {
            Ok(serde_json::json!({
                "ok": true,
                "selfMute": resolved.self_mute,
                "selfDeaf": resolved.self_deaf,
                "effectiveMute": resolved.effective_mute,
                "speakingFlags": resolved.speaking_flags,
            }))
        }
    })
    .await?
}

/// Returns the current live voice RTC status snapshot as JSON, enriched with canonical
/// `VoiceRtcStatusDto` fields (`txPackets`, `rxPackets`, `rttMs`, `lastError`) and `chord-voice` metadata.
pub async fn voice_rtc_status() -> Result<serde_json::Value, String> {
    blocking(|| {
        let mut raw = data::live_voice_rtc_status_json();
        let dto = extract_rtc_status_dto(&raw);
        let rtt_f64 = dto.rtt_ms.map(|v| v as f64).unwrap_or(-1.0);
        let loss_f64 = raw
            .get("packetLossPct")
            .and_then(serde_json::Value::as_f64)
            .unwrap_or(0.0);
        let quality = evaluate_rtc_connection_quality(rtt_f64, loss_f64);
        if let Some(obj) = raw.as_object_mut() {
            obj.entry("txPackets".to_string())
                .or_insert_with(|| serde_json::json!(dto.tx_packets));
            obj.entry("rxPackets".to_string())
                .or_insert_with(|| serde_json::json!(dto.rx_packets));
            obj.entry("rttMs".to_string())
                .or_insert_with(|| serde_json::json!(dto.rtt_ms));
            obj.entry("lastError".to_string())
                .or_insert_with(|| serde_json::json!(dto.last_error));
            obj.entry("connectionQuality".to_string())
                .or_insert_with(|| serde_json::json!(quality));
            obj.entry("daveProtocolVersion".to_string())
                .or_insert_with(|| serde_json::json!(DAVE_PROTOCOL_VERSION));
            obj.entry("mediaEncryptionMode".to_string())
                .or_insert_with(|| serde_json::json!(MEDIA_ENCRYPTION_MODE));
        }
        raw
    })
    .await
}

/// Returns the current live voice RTC status as a strongly-typed `VoiceRtcStatusDto`.
pub async fn voice_rtc_status_dto() -> Result<VoiceRtcStatusDto, String> {
    let raw = voice_rtc_status().await?;
    Ok(extract_rtc_status_dto(&raw))
}

/// Lists available system audio input/output devices as JSON.
pub async fn voice_audio_devices() -> Result<serde_json::Value, String> {
    blocking(data::list_system_audio_devices_json).await
}

/// Domain alias for `voice_audio_devices`.
pub async fn audio_devices() -> Result<serde_json::Value, String> {
    voice_audio_devices().await
}

/// Updates system audio device selection/volume/mute or handles tray/telemetry control actions.
pub async fn voice_set_audio_device<R: tauri::Runtime>(
    app: tauri::AppHandle<R>,
    kind: String,
    device_id: Option<String>,
    volume_pct: Option<u32>,
    volume_percent: Option<u32>,
    muted: Option<bool>,
) -> Result<serde_json::Value, String> {
    if kind == "app_quit" {
        data::shutdown_live_workers();
        std::process::exit(0);
    }
    if kind == "app_hide_to_tray" {
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.hide();
        }
        return Ok(serde_json::json!({ "ok": true, "hidden": true }));
    }
    if kind == "ui_telemetry_log" || kind == "ui_telemetry" {
        if let Some(payload) = device_id {
            println!("[UI-TELEMETRY] {payload}");
            for path in &[
                "/tmp/discord-gui-spike-telemetry.jsonl",
                "/tmp/live-ui-telemetry.jsonl",
            ] {
                let _ = std::fs::OpenOptions::new()
                    .create(true)
                    .append(true)
                    .open(path)
                    .and_then(|mut f| {
                        use std::io::Write;
                        writeln!(f, "{payload}")
                    });
            }
        }
        return Ok(serde_json::json!({ "ok": true }));
    }
    let vol = volume_pct.or(volume_percent);
    blocking(move || data::set_system_audio_device(&kind, device_id.as_deref(), vol, muted)).await
}

/// Enumerates available desktop screens and windows for screen sharing.
pub async fn voice_desktop_sources() -> Result<serde_json::Value, String> {
    blocking(|| {
        let capturer = chord_media::ScreenCapturer::new();
        let mut sources = capturer.enumerate_sources();
        let live_windows = data::query_kwin_active_windows();
        for w in live_windows {
            let wid = w.get("id").and_then(|i| i.as_str()).unwrap_or("").trim();
            let label = w
                .get("label")
                .and_then(|l| l.as_str())
                .unwrap_or(wid)
                .trim();
            if !wid.is_empty() && !sources.iter().any(|s| s.id == wid) {
                sources.push(chord_media::DesktopSource {
                    id: wid.to_string(),
                    name: label.to_string(),
                    source_type: chord_media::DesktopSourceType::Window,
                    is_primary: false,
                });
            }
        }
        let json_sources: Vec<_> = sources
            .into_iter()
            .map(|s| {
                let st = match s.source_type {
                    chord_media::DesktopSourceType::Screen => "screen",
                    chord_media::DesktopSourceType::Window => "window",
                };
                serde_json::json!({
                    "id": s.id,
                    "name": s.name,
                    "sourceType": st,
                    "source_type": st,
                    "isPrimary": s.is_primary,
                    "is_primary": s.is_primary,
                })
            })
            .collect();
        Ok(serde_json::json!({
            "backend": capturer.backend().name(),
            "sources": json_sources
        }))
    })
    .await?
}

/// Domain alias for `voice_desktop_sources`.
pub async fn screenshare_sources() -> Result<serde_json::Value, String> {
    voice_desktop_sources().await
}

/// Starts a screen-share stream (OP 18), acquiring a PipeWire portal stream on Wayland when needed.
pub async fn voice_stream_start(
    mut spec: chord_client::voice_stage::ScreenShareSpec,
) -> Result<serde_json::Value, String> {
    if spec
        .guild_id
        .as_deref()
        .is_some_and(|g| matches!(g.trim(), "" | "@me" | "null" | "nil"))
    {
        spec.guild_id = None;
    }
    spec.validate()?;

    {
        let now_ms = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis() as u64;
        let last_ms = LAST_STREAM_START_MS.load(Ordering::SeqCst);
        if now_ms.saturating_sub(last_ms) < 8_000 && last_ms > 0 {
            println!(
                "[IPC-TRACE] voice_stream_start: debounced ({}ms since last)",
                now_ms - last_ms
            );
            return Ok(serde_json::json!({ "ok": true, "debounced": true, "streamKey": "" }));
        }
        LAST_STREAM_START_MS.store(now_ms, Ordering::SeqCst);
    }

    let is_wayland = std::env::var("WAYLAND_DISPLAY").is_ok();
    let pipewire_info: Option<(u32, i32)> = if is_wayland
        || spec.source_id.is_empty()
        || spec.source_id.starts_with("screen")
    {
        match acquire_pipewire_node_id().await {
            Ok((nid, fd)) => {
                println!("[IPC-TRACE] voice_stream_start: PipeWire node_id={nid} fd={fd}");
                LAST_PIPEWIRE_NODE_ID.store(nid, Ordering::SeqCst);
                LAST_PIPEWIRE_FD.store(fd, Ordering::SeqCst);
                chord_core::ws::set_cached_pipewire_node_id(nid);
                chord_core::ws::set_cached_pipewire_fd(fd);
                let _ = std::fs::write(
                    "/dev/shm/discord_capture_target.txt",
                    format!("pipewire:{nid}").as_bytes(),
                );
                Some((nid, fd))
            }
            Err(e) => {
                println!("[IPC-TRACE] voice_stream_start: PipeWire portal failed ({e}), falling back to screen");
                None
            }
        }
    } else {
        None
    };

    let op18 = chord_client::voice_stage::build_gateway_stream_create_op18(&spec)?;
    let stream_key = blocking(move || {
        if let Some(ref gid_str) = spec.guild_id {
            if !data::can_user_stream_in_channel(Some(gid_str), &spec.channel_id) {
                return Err("Missing STREAM permission in this channel".to_string());
            }
        }
        let self_id = data::current_live_self_uid();
        let uid = if self_id.is_empty() {
            "me".to_string()
        } else {
            self_id
        };
        let stream_key = spec.to_stream_key(&uid);
        println!("[IPC-TRACE] voice_stream_start: stream_key={stream_key}");
        let gid = spec.guild_id.clone();
        let cid = Some(spec.channel_id.clone());
        let (cur_gid, cur_cid) = data::current_live_voice_target();
        let (vvideo, vstream_active) = data::current_live_voice_video_stream();
        if !vstream_active || cur_gid != gid || cur_cid != cid {
            chord_core::ws::clear_last_stream_key();
            data::reset_live_stream_state();
        }
        if !spec.source_id.is_empty() && pipewire_info.is_none() {
            let clean_src = spec.source_id.trim();
            let target_kind = if clean_src.starts_with('{') || clean_src.starts_with("win") {
                "window"
            } else {
                "screen"
            };
            let _ = std::fs::write("/dev/shm/discord_capture_target.txt", clean_src.as_bytes());
            let _ = std::fs::write("/dev/shm/discord_capture_kick.bin", &[1u8]);
            let ucmd = format!(
                "{{\"cmd\":\"set_stream_source\",\"sourceType\":\"{target_kind}\",\"sourceId\":\"{clean_src}\"}}"
            );
            chord_core::ws::send_native_stream_cmd(&ucmd);
        }
        if let Some((nid, fd)) = pipewire_info {
            let _ = std::fs::write("/dev/shm/discord_capture_kick.bin", &[1u8]);
            let pid = std::process::id();
            let fd_field = if fd >= 0 {
                format!(",\"fd\":{fd},\"pid\":{pid}")
            } else {
                String::new()
            };
            let ucmd = format!(
                "{{\"cmd\":\"set_stream_source\",\"sourceType\":\"pipewire\",\"nodeId\":{nid}{fd_field}}}"
            );
            chord_core::ws::send_native_stream_cmd(&ucmd);
            println!(
                "[IPC-TRACE] voice_stream_start: sent PipeWire nodeId={nid} fd={fd} pid={pid} to native worker (overrides screen:{}))",
                spec.source_id
            );
        }
        let (vmute, vdeaf) = data::current_live_voice_mute_deaf();
        let share_audio = spec.share_audio;
        if share_audio {
            chord_core::ws::set_live_stream_audio_state(false, 100);
        } else {
            chord_core::ws::set_live_stream_audio_state(true, 0);
        }
        data::connect_or_update_live_voice_ext(gid, cid, vmute, vdeaf, vvideo, true)?;
        Ok::<String, String>(stream_key)
    })
    .await??;
    Ok(serde_json::json!({ "ok": true, "streamKey": stream_key, "payload": op18 }))
}

/// Domain alias for `voice_stream_start`.
pub async fn screenshare_start(
    spec: chord_client::voice_stage::ScreenShareSpec,
) -> Result<serde_json::Value, String> {
    voice_stream_start(spec).await
}

/// Stops the active screen-share stream (OP 19) and releases any active portal session.
pub async fn voice_stream_stop(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
) -> Result<serde_json::Value, String> {
    let key = stream_key
        .or(streamKey)
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(|| {
            let (gid, cid) = data::current_live_voice_target();
            let self_id = data::current_live_self_uid();
            let uid = if self_id.is_empty() {
                "me".to_string()
            } else {
                self_id
            };
            match (gid, cid) {
                (Some(g), Some(c)) if !g.is_empty() && g != "@me" && g != "call" => {
                    format!("guild:{g}:{c}:{uid}")
                }
                (_, Some(c)) => format!("call:{c}:{uid}"),
                _ => String::new(),
            }
        });
    let op19 = if !key.is_empty() {
        chord_client::voice_stage::build_gateway_stream_delete_op19(&key)?
    } else {
        serde_json::json!({ "op": 19, "d": { "stream_key": "" } })
    };
    println!("[IPC-TRACE] voice_stream_stop: stream_key={key}");
    let k = key.clone();
    let _ = blocking(move || {
        chord_core::ws::clear_last_stream_key();
        if let Ok(mut guard) = ACTIVE_PORTAL_SESSION.lock() {
            *guard = None;
        }
        let _ = std::fs::remove_file("/dev/shm/discord_pipewire_fd.txt");
        let _ = std::fs::remove_file("/dev/shm/discord_pipewire_fd.sock");
        let (gid, cid) = data::current_live_voice_target();
        let (vmute, vdeaf) = data::current_live_voice_mute_deaf();
        let _ = data::connect_or_update_live_voice_ext(gid, cid, vmute, vdeaf, false, false);
        if !k.is_empty() {
            let _ = chord_core::ws::queue_live_stream_delete(&k);
        }
        data::reset_live_stream_state();
        Ok::<(), String>(())
    })
    .await?;
    Ok(serde_json::json!({
        "ok": true,
        "streamKey": key,
        "payload": op19
    }))
}

/// Domain alias for `voice_stream_stop`.
pub async fn screenshare_stop(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
) -> Result<serde_json::Value, String> {
    voice_stream_stop(stream_key, streamKey).await
}

/// Joins an active Go Live stream as a viewer (OP 20).
pub async fn voice_stream_watch(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
) -> Result<serde_json::Value, String> {
    let key = stream_key
        .or(streamKey)
        .ok_or_else(|| "missing required parameter stream_key / streamKey".to_string())?;
    let op20 = chord_client::voice_stage::build_gateway_stream_watch_op20(&key)?;
    println!("[IPC-TRACE] voice_stream_watch: stream_key={key}");
    let k = key.clone();
    let _ =
        blocking(move || chord_core::ws::queue_live_stream_watch(&k).map_err(|e| e.to_string()))
            .await?;
    Ok(serde_json::json!({
        "ok": true,
        "streamKey": key,
        "payload": op20
    }))
}

/// Domain alias for `voice_stream_watch`.
pub async fn screenshare_watch(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
) -> Result<serde_json::Value, String> {
    voice_stream_watch(stream_key, streamKey).await
}

/// Pauses or resumes an active screen-share stream (OP 22).
pub async fn voice_stream_set_paused(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
    paused: bool,
) -> Result<serde_json::Value, String> {
    let key = stream_key
        .or(streamKey)
        .ok_or_else(|| "missing required parameter stream_key / streamKey".to_string())?;
    let op22 = chord_client::voice_stage::build_gateway_stream_set_paused_op22(&key, paused)?;
    println!("[IPC-TRACE] voice_stream_set_paused: stream_key={key} paused={paused}");
    let k = key.clone();
    let _ = blocking(move || {
        chord_core::ws::queue_live_stream_set_paused(&k, paused).map_err(|e| e.to_string())
    })
    .await?;
    Ok(serde_json::json!({
        "ok": true,
        "streamKey": key,
        "paused": paused,
        "payload": op22
    }))
}

/// Domain alias for `voice_stream_set_paused`.
pub async fn screenshare_set_paused(
    stream_key: Option<String>,
    #[allow(non_snake_case)] streamKey: Option<String>,
    paused: bool,
) -> Result<serde_json::Value, String> {
    voice_stream_set_paused(stream_key, streamKey, paused).await
}

/// Initializes the modular Tauri v2 `"voice"` plugin.
#[must_use]
pub fn init<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    tauri::plugin::Builder::new(PLUGIN_NAME).build()
}

/// Alias for [`init`] matching the plugin constructor pattern across `chord-client`.
#[must_use]
pub fn voice<R: tauri::Runtime>() -> tauri::plugin::TauriPlugin<R> {
    init()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_gate_enforces_dave_version_and_deaf_suppression() {
        assert!(verify_voice_protocol_gate(1, false, false));
        assert!(verify_voice_protocol_gate(1, true, false));
        assert!(verify_voice_protocol_gate(1, false, true));
        assert!(!verify_voice_protocol_gate(1, true, true));
        assert!(!verify_voice_protocol_gate(0, false, false));
        assert!(!verify_voice_protocol_gate(2, false, false));
    }

    #[test]
    fn effective_mute_and_speaking_flags_follow_chord_voice_invariants() {
        let normal = resolve_effective_mute_deaf(false, false);
        assert!(!normal.effective_mute);
        assert_eq!(normal.speaking_flags, SpeakingFlags::MICROPHONE.bits());

        let muted = resolve_effective_mute_deaf(true, false);
        assert!(muted.effective_mute);
        assert_eq!(muted.speaking_flags, SpeakingFlags::NONE.bits());

        let deafened = resolve_effective_mute_deaf(false, true);
        assert!(deafened.effective_mute);
        assert_eq!(deafened.speaking_flags, SpeakingFlags::NONE.bits());
    }
}
