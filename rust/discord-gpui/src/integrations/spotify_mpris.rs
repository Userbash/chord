//! Интеграция со Spotify по D-Bus MPRIS (`Listen Along` & Presence — `TASK-1013` / `TASK-1013A` / `TASK-1013B`).
//! Оракулы: `waves/kanban-board.json`, D-Bus `org.mpris.MediaPlayer2.spotify`, `docs/re/gateway-protocol.json`
//! (`PRESENCE_UPDATE = 107`), `/home/sanya/re-deep/gateway-schema.json`, `msgs-run1-57992_5112.jsonl` (`s=4`, `s=32`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

pub const SPOTIFY_CARD_CONTAINER: &str = "Spotify Activity Card";
pub const SPOTIFY_CARD_DIMENSIONS: &str = "100% x auto";
pub const SPOTIFY_CARD_PADDING_PX: u32 = 8;
pub const SPOTIFY_CARD_SPACING_PX: u32 = 6;
pub const MPRIS_BRIDGE_CONTAINER: &str = "MPRIS Bridge";
pub const MPRIS_BRIDGE_DIMENSIONS: &str = "D-Bus Listener";
pub const SPOTIFY_PROFILE_CONTAINER: &str = "Spotify Profile Card";

pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT_SPOTIFY_GREEN: &str = "#1db954";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const DBUS_BUS_NAME: &str = "org.mpris.MediaPlayer2.spotify";
pub const DBUS_OBJECT_PATH: &str = "/org/mpris/MediaPlayer2";
pub const DBUS_PLAYER_INTERFACE: &str = "org.mpris.MediaPlayer2.Player";
pub const DBUS_PROPERTIES_INTERFACE: &str = "org.freedesktop.DBus.Properties";
pub const DBUS_SIGNAL_PROPERTIES_CHANGED: &str = "PropertiesChanged";
pub const DBUS_SIGNAL_NAME_OWNER_CHANGED: &str = "NameOwnerChanged";

pub const GATEWAY_OPCODE_STATUS_UPDATE: u8 = 3;
pub const ACTIVITY_TYPE_LISTENING: u8 = 2;
pub const ACTIVITY_NAME_SPOTIFY: &str = "Spotify";
pub const ACTIVITY_ID_SPOTIFY: &str = "spotify:1";
pub const ACTIVITY_FLAG_SYNC: u32 = 1 << 4;
pub const ACTIVITY_FLAG_PLAY: u32 = 1 << 5;
pub const ACTIVITY_FLAGS_PLAY_SYNC: u32 = ACTIVITY_FLAG_PLAY | ACTIVITY_FLAG_SYNC;

pub const PARTY_ID_PREFIX: &str = "spotify:";
pub const ASSET_IMAGE_PREFIX: &str = "spotify:";
pub const SPOTIFY_TRACK_ID_LEN: usize = 22;
pub const MAX_DETAILS_CHARS: usize = 128;
pub const TRUNCATED_DETAILS_PREFIX_CHARS: usize = 125;
pub const MAX_SPOTIFY_ARTISTS: usize = 5;
pub const START_TIME_JITTER_MS: u64 = 1_500;
pub const SPEAKING_AUTO_PAUSE_MS: u64 = 30_000;
pub const LISTEN_ALONG_HOST_TIMEOUT_MS: u64 = 300_000;
pub const TRACK_END_GRACE_MS: u64 = 5_000;
pub const HTTP_202_RETRY_DELAY_MS: u64 = 5_000;

pub const SPOTIFY_API_PLAYER_PLAY: &str = "https://api.spotify.com/v1/me/player/play";
pub const SPOTIFY_API_PLAYER_PAUSE: &str = "https://api.spotify.com/v1/me/player/pause";
pub const SPOTIFY_API_PLAYER_DEVICES: &str = "https://api.spotify.com/v1/me/player/devices";
pub const DISCORD_SPOTIFY_INVENTORY_PATH: &str = "/content-inventory/users/@me/spotify";

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_USE_EXTERNAL_APPS: u64 = 1 << 50;
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListenAlongButtonState {
    SpotifyGreen,
    Brighten,
    SyncPlayback,
}

impl ListenAlongButtonState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::SpotifyGreen => "spotify green",
            Self::Brighten => "brighten",
            Self::SyncPlayback => "sync playback",
        }
    }

    #[must_use]
    pub const fn resolve(syncing: bool, hovered: bool) -> Self {
        if syncing {
            Self::SyncPlayback
        } else if hovered {
            Self::Brighten
        } else {
            Self::SpotifyGreen
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MprisEventInteractiveState {
    Idle,
    DetectTrack,
    StatusBroadcast,
}

impl MprisEventInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::DetectTrack => "detect track",
            Self::StatusBroadcast => "status broadcast",
        }
    }

    #[must_use]
    pub const fn resolve(broadcasting: bool, detecting: bool) -> Self {
        if broadcasting {
            Self::StatusBroadcast
        } else if detecting {
            Self::DetectTrack
        } else {
            Self::Idle
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpotifyError {
    InvalidSnowflake(String),
    WrongDbusInterface(String),
    EmptyTrackTitle,
    InvalidTrackId(String),
    MissingPartyId,
    AccountNotPremium,
    NoActiveDevice,
    SelfListenAlongForbidden,
    MissingPermission(&'static str),
    AgeGatedRestricted,
    BoundaryViolation(&'static str),
    HttpFailure { status: u16, retry_after_ms: u64 },
}

impl fmt::Display for SpotifyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for SpotifyError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, SpotifyError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(SpotifyError::InvalidSnowflake(raw.to_string()));
    }
    let parsed = trimmed
        .parse::<u64>()
        .map_err(|_| SpotifyError::InvalidSnowflake(raw.to_string()))?;
    if parsed == 0 {
        return Err(SpotifyError::InvalidSnowflake(raw.to_string()));
    }
    Ok(parsed)
}

#[must_use]
pub fn extract_spotify_track_id(raw_track_id: &str) -> Option<String> {
    let trimmed = raw_track_id.trim();
    if trimmed.is_empty()
        || trimmed.contains("NoTrack")
        || trimmed.starts_with("spotify:local:")
        || trimmed.contains("/local/")
    {
        return None;
    }
    let candidate = trimmed
        .strip_prefix("/com/spotify/track/")
        .or_else(|| trimmed.strip_prefix("spotify:track:"))
        .or_else(|| {
            trimmed
                .strip_prefix("https://open.spotify.com/track/")
                .and_then(|r| r.split('?').next())
        })
        .unwrap_or(trimmed);
    if candidate.len() == SPOTIFY_TRACK_ID_LEN
        && candidate.bytes().all(|b| b.is_ascii_alphanumeric())
    {
        Some(candidate.to_string())
    } else {
        None
    }
}

#[must_use]
pub fn normalize_spotify_art_asset(raw_art_url: &str) -> Option<String> {
    let trimmed = raw_art_url.trim();
    if let Some(hex) = trimmed.strip_prefix(ASSET_IMAGE_PREFIX) {
        if !hex.is_empty() && hex.bytes().all(|b| b.is_ascii_hexdigit()) {
            return Some(trimmed.to_string());
        }
    }
    for prefix in [
        "https://i.scdn.co/image/",
        "http://i.scdn.co/image/",
        "https://open.spotify.com/image/",
    ] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            let id = rest.split(['?', '/']).next().unwrap_or("");
            if !id.is_empty() && id.bytes().all(|b| b.is_ascii_hexdigit()) {
                return Some(format!("{ASSET_IMAGE_PREFIX}{id}"));
            }
        }
    }
    None
}

#[must_use]
pub fn format_spotify_details(title: &str) -> String {
    let trimmed = title.trim();
    if trimmed.chars().count() > MAX_DETAILS_CHARS {
        let prefix: String = trimmed
            .chars()
            .take(TRUNCATED_DETAILS_PREFIX_CHARS)
            .collect();
        format!("{prefix}...")
    } else {
        trimmed.to_string()
    }
}

#[must_use]
pub fn format_spotify_artists(artists: &[String]) -> String {
    artists
        .iter()
        .map(|a| a.replace(';', "").trim().to_string())
        .filter(|a| !a.is_empty())
        .take(MAX_SPOTIFY_ARTISTS)
        .collect::<Vec<_>>()
        .join("; ")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DbusMprisSignal {
    pub interface_name: String,
    pub playback_status: String,
    pub position_us: u64,
    pub track_id: String,
    pub length_us: u64,
    pub art_url: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpotifyMprisTrack {
    pub track_id: Option<String>,
    pub title: String,
    pub artists_joined: String,
    pub album: String,
    pub large_image: Option<String>,
    pub duration_ms: u64,
    pub position_ms: u64,
    pub start_ms: u64,
    pub end_ms: u64,
    pub is_playing: bool,
    pub is_local: bool,
}

pub fn parse_mpris_signal(
    signal: &DbusMprisSignal,
    now_ms: u64,
    prev_start_ms: Option<u64>,
) -> Result<SpotifyMprisTrack, SpotifyError> {
    if signal.interface_name != DBUS_PLAYER_INTERFACE {
        return Err(SpotifyError::WrongDbusInterface(
            signal.interface_name.clone(),
        ));
    }
    let details = format_spotify_details(&signal.title);
    if details.is_empty() {
        return Err(SpotifyError::EmptyTrackTitle);
    }
    let is_playing = signal.playback_status.trim() == "Playing";
    let track_id = extract_spotify_track_id(&signal.track_id);
    let is_local = track_id.is_none();
    let duration_ms = signal.length_us / 1_000;
    let position_ms = (signal.position_us / 1_000).min(duration_ms.max(1));
    let candidate_start = now_ms.saturating_sub(position_ms);
    let start_ms = match prev_start_ms {
        Some(prev) if candidate_start.abs_diff(prev) <= START_TIME_JITTER_MS => prev,
        _ => candidate_start,
    };
    let end_ms = start_ms.saturating_add(duration_ms);
    Ok(SpotifyMprisTrack {
        track_id,
        title: details,
        artists_joined: format_spotify_artists(&signal.artists),
        album: signal.album.trim().to_string(),
        large_image: normalize_spotify_art_asset(&signal.art_url),
        duration_ms,
        position_ms,
        start_ms,
        end_ms,
        is_playing,
        is_local,
    })
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayListeningActivity {
    pub activity_type: u8,
    pub name: String,
    pub id: String,
    pub flags: u32,
    pub details: String,
    pub state: String,
    pub sync_id: Option<String>,
    pub party_id: String,
    pub large_image: Option<String>,
    pub large_text: Option<String>,
    pub start_ms: u64,
    pub end_ms: u64,
    pub session_id: String,
    pub created_at: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayStatusUpdateFrame {
    pub op: u8,
    pub status: String,
    pub afk: bool,
    pub since: Option<u64>,
    pub activities: Vec<GatewayListeningActivity>,
}

pub fn build_gateway_listening_activity(
    track: &SpotifyMprisTrack,
    user_id: &str,
    session_id: &str,
    party_override: Option<&str>,
    now_ms: u64,
) -> Result<GatewayListeningActivity, SpotifyError> {
    validate_snowflake(user_id)?;
    let party_id = match party_override {
        Some(custom) if custom.starts_with(PARTY_ID_PREFIX) => custom.to_string(),
        _ => format!("{PARTY_ID_PREFIX}{user_id}"),
    };
    let flags = if track.is_local {
        0
    } else {
        ACTIVITY_FLAGS_PLAY_SYNC
    };
    let large_text = (!track.album.is_empty()).then(|| track.album.clone());
    Ok(GatewayListeningActivity {
        activity_type: ACTIVITY_TYPE_LISTENING,
        name: ACTIVITY_NAME_SPOTIFY.to_string(),
        id: ACTIVITY_ID_SPOTIFY.to_string(),
        flags,
        details: track.title.clone(),
        state: track.artists_joined.clone(),
        sync_id: track.track_id.clone(),
        party_id,
        large_image: track.large_image.clone(),
        large_text,
        start_ms: track.start_ms,
        end_ms: track.end_ms,
        session_id: session_id.to_string(),
        created_at: now_ms,
    })
}

#[must_use]
pub fn build_gateway_status_update_frame(
    activity: Option<GatewayListeningActivity>,
    status: &str,
    afk: bool,
) -> GatewayStatusUpdateFrame {
    GatewayStatusUpdateFrame {
        op: GATEWAY_OPCODE_STATUS_UPDATE,
        status: status.to_string(),
        afk,
        since: None,
        activities: activity.into_iter().collect(),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TrackTimelineProgress {
    pub elapsed_ms: u64,
    pub duration_ms: u64,
    pub progress_percent: u8,
    pub elapsed_formatted: String,
    pub total_formatted: String,
}

#[must_use]
pub fn format_mm_ss(ms: u64) -> String {
    let total_secs = ms / 1_000;
    format!("{:02}:{:02}", total_secs / 60, total_secs % 60)
}

#[must_use]
pub fn compute_track_progress(
    start_ms: u64,
    end_ms: u64,
    now_ms: u64,
    is_paused: bool,
    paused_at_ms: Option<u64>,
) -> TrackTimelineProgress {
    let duration_ms = end_ms.saturating_sub(start_ms);
    if duration_ms == 0 {
        return TrackTimelineProgress {
            elapsed_ms: 0,
            duration_ms: 0,
            progress_percent: 0,
            elapsed_formatted: "00:00".to_string(),
            total_formatted: "00:00".to_string(),
        };
    }
    let effective_now = if is_paused {
        paused_at_ms.unwrap_or(now_ms)
    } else {
        now_ms
    };
    let elapsed_ms = effective_now.saturating_sub(start_ms).min(duration_ms);
    let progress_percent = ((elapsed_ms * 100 + (duration_ms / 2)) / duration_ms).min(100) as u8;
    TrackTimelineProgress {
        elapsed_ms,
        duration_ms,
        progress_percent,
        elapsed_formatted: format_mm_ss(elapsed_ms),
        total_formatted: format_mm_ss(duration_ms),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListenAlongPlayRequest {
    pub method: &'static str,
    pub url: String,
    pub track_uri: String,
    pub position_ms: u64,
    pub party_id: String,
    pub host_user_id: String,
}

pub fn build_listen_along_play_request(
    host_user_id: &str,
    current_user_id: &str,
    host_activity: &GatewayListeningActivity,
    device_id: &str,
    is_premium: bool,
    now_ms: u64,
) -> Result<ListenAlongPlayRequest, SpotifyError> {
    validate_snowflake(host_user_id)?;
    validate_snowflake(current_user_id)?;
    if host_user_id == current_user_id {
        return Err(SpotifyError::SelfListenAlongForbidden);
    }
    if !is_premium {
        return Err(SpotifyError::AccountNotPremium);
    }
    if device_id.trim().is_empty() {
        return Err(SpotifyError::NoActiveDevice);
    }
    if !host_activity.party_id.starts_with(PARTY_ID_PREFIX) {
        return Err(SpotifyError::MissingPartyId);
    }
    let sync_id = host_activity.sync_id.as_deref().unwrap_or("");
    if extract_spotify_track_id(sync_id).is_none() {
        return Err(SpotifyError::InvalidTrackId(sync_id.to_string()));
    }
    Ok(ListenAlongPlayRequest {
        method: "PUT",
        url: format!("{SPOTIFY_API_PLAYER_PLAY}?device_id={}", device_id.trim()),
        track_uri: format!("spotify:track:{sync_id}"),
        position_ms: now_ms.saturating_sub(host_activity.start_ms),
        party_id: host_activity.party_id.clone(),
        host_user_id: host_user_id.to_string(),
    })
}

#[derive(Debug, Clone)]
pub struct SpotifyMprisBridge {
    pub current_user_id: String,
    pub session_id: String,
    pub is_premium: bool,
    pub active_device_id: Option<String>,
    pub current_track: Option<SpotifyMprisTrack>,
    pub current_activity: Option<GatewayListeningActivity>,
    pub syncing_host: Option<(String, String, u64)>,
    pub known_presences: BTreeMap<String, GatewayListeningActivity>,
}

impl SpotifyMprisBridge {
    pub fn new(
        current_user_id: &str,
        session_id: &str,
        is_premium: bool,
        active_device_id: Option<String>,
    ) -> Result<Self, SpotifyError> {
        validate_snowflake(current_user_id)?;
        Ok(Self {
            current_user_id: current_user_id.to_string(),
            session_id: session_id.to_string(),
            is_premium,
            active_device_id,
            current_track: None,
            current_activity: None,
            syncing_host: None,
            known_presences: BTreeMap::new(),
        })
    }

    pub fn on_properties_changed(
        &mut self,
        signal: &DbusMprisSignal,
        now_ms: u64,
    ) -> Result<GatewayStatusUpdateFrame, SpotifyError> {
        let prev_start = self
            .current_track
            .as_ref()
            .filter(|t| {
                t.track_id.is_some() && t.track_id == extract_spotify_track_id(&signal.track_id)
            })
            .map(|t| t.start_ms);
        let parsed = parse_mpris_signal(signal, now_ms, prev_start)?;
        if !parsed.is_playing {
            self.current_track = None;
            self.current_activity = None;
            return Ok(build_gateway_status_update_frame(None, "online", false));
        }
        let party = self.syncing_host.as_ref().map(|(_, pid, _)| pid.as_str());
        let act = build_gateway_listening_activity(
            &parsed,
            &self.current_user_id,
            &self.session_id,
            party,
            now_ms,
        )?;
        self.current_track = Some(parsed);
        self.current_activity = Some(act.clone());
        Ok(build_gateway_status_update_frame(
            Some(act),
            "online",
            false,
        ))
    }

    pub fn on_name_owner_changed(
        &mut self,
        bus_name: &str,
        new_owner: &str,
    ) -> Option<GatewayStatusUpdateFrame> {
        if bus_name == DBUS_BUS_NAME && new_owner.trim().is_empty() {
            self.current_track = None;
            self.current_activity = None;
            self.syncing_host = None;
            return Some(build_gateway_status_update_frame(None, "online", false));
        }
        None
    }

    pub fn start_listen_along(
        &mut self,
        host_user_id: &str,
        host_activity: &GatewayListeningActivity,
        now_ms: u64,
    ) -> Result<ListenAlongPlayRequest, SpotifyError> {
        let dev = self.active_device_id.as_deref().unwrap_or("");
        let req = build_listen_along_play_request(
            host_user_id,
            &self.current_user_id,
            host_activity,
            dev,
            self.is_premium,
            now_ms,
        )?;
        let sync_id = host_activity.sync_id.clone().unwrap_or_default();
        self.syncing_host = Some((host_user_id.to_string(), sync_id, host_activity.start_ms));
        Ok(req)
    }
}

pub fn check_spotify_activity_permissions(
    permissions: u64,
    is_private: bool,
    age_gated_restricted: bool,
) -> Result<(), SpotifyError> {
    if age_gated_restricted {
        return Err(SpotifyError::AgeGatedRestricted);
    }
    if !is_private && (permissions & PERM_VIEW_CHANNEL) == 0 {
        return Err(SpotifyError::MissingPermission("VIEW_CHANNEL"));
    }
    Ok(())
}

pub fn validate_system_boundaries(
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
) -> Result<(), SpotifyError> {
    if message_chars > MAX_MESSAGE_CHARS
        || attachments_count > MAX_ATTACHMENTS_COUNT
        || attachment_bytes > MAX_ATTACHMENT_BYTES
        || search_offset > MAX_SEARCH_OFFSET
        || volume_pct > MAX_VOLUME_PERCENT
    {
        return Err(SpotifyError::BoundaryViolation("system_boundary"));
    }
    Ok(())
}

#[must_use]
pub fn format_badge_cap(count: u32) -> String {
    if count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

#[must_use]
pub fn redact_spotify_diagnostic(raw: &str) -> String {
    let mut out = raw.to_string();
    for marker in ["mfa.", "Bearer ", "BQD"] {
        if let Some(pos) = out.find(marker) {
            let start = if marker == "BQD" {
                pos
            } else {
                pos + marker.len()
            };
            let end = out[start..]
                .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
                .map_or(out.len(), |i| start + i);
            out.replace_range(start..end, "[REDACTED]");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_signal() -> DbusMprisSignal {
        DbusMprisSignal {
            interface_name: DBUS_PLAYER_INTERFACE.to_string(),
            playback_status: "Playing".to_string(),
            position_us: 13_471_000,
            track_id: "/com/spotify/track/3cdUEnMVspkAfx6nnxSdTl".to_string(),
            length_us: 230_222_000,
            art_url: "https://i.scdn.co/image/ab67616d0000b2736700e765df4db64f4b0c6276".to_string(),
            title: "Freeman".to_string(),
            artists: vec!["Miyagi & Andy Panda".to_string()],
            album: "Freeman".to_string(),
        }
    }

    #[test]
    fn test_sub_1013_1_constants_and_interactive_states() {
        assert_eq!(SPOTIFY_CARD_CONTAINER, "Spotify Activity Card");
        assert_eq!(SPOTIFY_CARD_DIMENSIONS, "100% x auto");
        assert_eq!((SPOTIFY_CARD_PADDING_PX, SPOTIFY_CARD_SPACING_PX), (8, 6));
        assert_eq!((TOKEN_BACKGROUND, TOKEN_SURFACE), ("#1e1f22", "#2b2d31"));
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT_SPOTIFY_GREEN, "#1db954");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(DBUS_BUS_NAME, "org.mpris.MediaPlayer2.spotify");
        assert_eq!(DBUS_OBJECT_PATH, "/org/mpris/MediaPlayer2");
        assert_eq!(GATEWAY_OPCODE_STATUS_UPDATE, 3);
        assert_eq!(ACTIVITY_TYPE_LISTENING, 2);
        assert_eq!(ACTIVITY_FLAGS_PLAY_SYNC, 48);
        let s0 = ListenAlongButtonState::resolve(false, false).as_str();
        let s1 = ListenAlongButtonState::resolve(false, true).as_str();
        let s2 = ListenAlongButtonState::resolve(true, false).as_str();
        assert_eq!((s0, s1, s2), ("spotify green", "brighten", "sync playback"));
        let m0 = MprisEventInteractiveState::resolve(false, false).as_str();
        let m2 = MprisEventInteractiveState::resolve(true, false).as_str();
        assert_eq!((m0, m2), ("idle", "status broadcast"));
    }

    #[test]
    fn test_sub_1013_2_mpris_metadata_and_timestamps() {
        let tid1 = extract_spotify_track_id("/com/spotify/track/3cdUEnMVspkAfx6nnxSdTl");
        let tid2 = extract_spotify_track_id("spotify:track:6IPwKM3fUUzlElbvKw2sKl");
        assert_eq!(tid1.as_deref(), Some("3cdUEnMVspkAfx6nnxSdTl"));
        assert_eq!(tid2.as_deref(), Some("6IPwKM3fUUzlElbvKw2sKl"));
        assert!(extract_spotify_track_id("/org/mpris/MediaPlayer2/TrackList/NoTrack").is_none());
        let art = normalize_spotify_art_asset(
            "https://i.scdn.co/image/ab67616d0000b2736700e765df4db64f4b0c6276",
        );
        assert_eq!(
            art.as_deref(),
            Some("spotify:ab67616d0000b2736700e765df4db64f4b0c6276")
        );
        let sig = sample_signal();
        let now_ms = 1_792_390_379_228;
        let track = parse_mpris_signal(&sig, now_ms, None).unwrap();
        assert_eq!(track.track_id.as_deref(), Some("3cdUEnMVspkAfx6nnxSdTl"));
        assert_eq!(track.start_ms, 1_792_390_365_757);
        assert_eq!(track.end_ms, 1_792_390_595_979);
        let mut jitter_sig = sig;
        jitter_sig.position_us = 13_000_000;
        let track_jitter = parse_mpris_signal(&jitter_sig, now_ms, Some(track.start_ms)).unwrap();
        assert_eq!(track_jitter.start_ms, track.start_ms);
    }

    #[test]
    fn test_sub_1013_3_gateway_opcode3_and_live_replay() {
        let uid = "366982137616007169";
        let sid = "5658ed3b2c7e19a11f36e2b86b0db208";
        let mut bridge = SpotifyMprisBridge::new(uid, sid, true, Some("dev1".into())).unwrap();
        let frame = bridge
            .on_properties_changed(&sample_signal(), 1_792_390_379_228)
            .unwrap();
        assert_eq!((frame.op, frame.activities.len()), (3, 1));
        let act = &frame.activities[0];
        assert_eq!((act.activity_type, act.flags), (2, 48));
        assert_eq!(
            (act.name.as_str(), act.id.as_str()),
            ("Spotify", "spotify:1")
        );
        assert_eq!(act.party_id, "spotify:366982137616007169");
        assert_eq!(act.start_ms, 1_792_390_365_757);
        assert_eq!(act.end_ms, 1_792_390_595_979);
        let cleared = bridge.on_name_owner_changed(DBUS_BUS_NAME, "").unwrap();
        assert_eq!((cleared.op, cleared.activities.is_empty()), (3, true));
        assert!(bridge.current_activity.is_none());
    }

    #[test]
    fn test_sub_1013_4_listen_along_and_nine_dimensions() {
        assert!(validate_snowflake("0").is_err());
        assert_eq!(
            validate_snowflake("18446744073709551615").unwrap(),
            u64::MAX
        );
        let prog = compute_track_progress(1_000_000, 1_200_000, 1_100_000, false, None);
        assert_eq!(prog.progress_percent, 50);
        assert_eq!(
            (
                prog.elapsed_formatted.as_str(),
                prog.total_formatted.as_str()
            ),
            ("01:40", "03:20")
        );
        let track = parse_mpris_signal(&sample_signal(), 1_792_390_379_228, None).unwrap();
        let host = "293808922463174658";
        let me = "479659568322969611";
        let sid = "5658ed3b2c7e19a11f36e2b86b0db208";
        let host_act =
            build_gateway_listening_activity(&track, host, sid, None, 1_792_390_379_228).unwrap();
        let mut bridge = SpotifyMprisBridge::new(me, sid, true, Some("dev_main".into())).unwrap();
        let req = bridge
            .start_listen_along(host, &host_act, 1_792_390_395_757)
            .unwrap();
        assert_eq!((req.method, req.position_ms), ("PUT", 30_000));
        let non_prem = build_listen_along_play_request(
            host,
            me,
            &host_act,
            "dev_main",
            false,
            1_792_390_395_757,
        );
        assert!(non_prem.is_err());
        assert_eq!(
            (format_badge_cap(99), format_badge_cap(100)),
            ("99".into(), "99+".into())
        );
        assert!(check_spotify_activity_permissions(PERM_VIEW_CHANNEL, false, false).is_ok());
        assert!(check_spotify_activity_permissions(PERM_VIEW_CHANNEL, false, true).is_err());
        assert!(validate_system_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200).is_ok());
        assert!(validate_system_boundaries(2_001, 10, 0, 0, 100).is_err());
        assert!(!redact_spotify_diagnostic("Bearer BQDsecret123").contains("BQDsecret123"));
    }
}
