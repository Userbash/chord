//! D-Bus MPRIS client for Spotify and Gateway Opcode 3 `LISTENING` presence (`TASK-1013A`).
//!
//! Oracle: D-Bus `org.mpris.MediaPlayer2` (`org.mpris.MediaPlayer2.spotify`) and
//! Discord Gateway Opcode 3 `STATUS_UPDATE` (activity `type: 2`, `LISTENING`).

#![forbid(unsafe_code)]

use std::collections::BTreeMap;

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
pub const ACTIVITY_FLAGS_PLAY_SYNC: u32 = (1 << 4) | (1 << 5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlaybackStatus {
    Playing,
    Paused,
    Stopped,
}

impl PlaybackStatus {
    pub fn parse(raw: &str) -> Self {
        match raw.trim() {
            "Playing" => Self::Playing,
            "Paused" => Self::Paused,
            _ => Self::Stopped,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct MprisMetadata {
    pub track_id: String,
    pub title: String,
    pub artists: Vec<String>,
    pub album: String,
    pub art_url: String,
    pub length_us: u64,
    pub position_us: u64,
}

impl MprisMetadata {
    /// Parses a flat MPRIS metadata map (`mpris:length`, `xesam:title`, etc.).
    pub fn from_mpris_map(map: &BTreeMap<String, String>, position_us: u64) -> Self {
        let artists = map
            .get("xesam:artist")
            .map(|raw| {
                raw.split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect()
            })
            .unwrap_or_default();

        Self {
            track_id: map.get("mpris:trackid").cloned().unwrap_or_default(),
            title: map.get("xesam:title").cloned().unwrap_or_default(),
            artists,
            album: map.get("xesam:album").cloned().unwrap_or_default(),
            art_url: map.get("mpris:artUrl").cloned().unwrap_or_default(),
            length_us: map
                .get("mpris:length")
                .and_then(|v| v.parse::<u64>().ok())
                .unwrap_or(0),
            position_us,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MprisSignal {
    PropertiesChanged {
        interface: String,
        playback_status: Option<PlaybackStatus>,
        metadata: Option<MprisMetadata>,
        position_us: Option<u64>,
    },
    NameOwnerChanged {
        bus_name: String,
        new_owner: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActivityTimestamps {
    pub start: u64,
    pub end: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListeningActivity {
    pub id: String,
    pub name: String,
    pub kind: u8,
    pub details: String,
    pub state: String,
    pub large_text: String,
    pub timestamps: ActivityTimestamps,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayStatusUpdate {
    pub op: u8,
    pub status: String,
    pub activities: Vec<ListeningActivity>,
    pub afk: bool,
}

impl GatewayStatusUpdate {
    pub fn clear_presence() -> Self {
        Self {
            op: GATEWAY_OPCODE_STATUS_UPDATE,
            status: "online".to_string(),
            activities: Vec::new(),
            afk: false,
        }
    }
}

#[derive(Debug, Default)]
pub struct SpotifyMprisClient {
    status: Option<PlaybackStatus>,
    metadata: Option<MprisMetadata>,
    position_us: u64,
}

impl SpotifyMprisClient {
    pub fn new() -> Self {
        Self::default()
    }

    /// Processes an incoming D-Bus MPRIS signal and returns a Gateway Opcode 3 update if changed.
    pub fn handle_signal(
        &mut self,
        signal: MprisSignal,
        now_ms: u64,
    ) -> Option<GatewayStatusUpdate> {
        match signal {
            MprisSignal::NameOwnerChanged {
                bus_name,
                new_owner,
            } => {
                if bus_name != DBUS_BUS_NAME {
                    return None;
                }
                if new_owner.is_empty() {
                    self.status = None;
                    self.metadata = None;
                    self.position_us = 0;
                    return Some(GatewayStatusUpdate::clear_presence());
                }
                None
            }
            MprisSignal::PropertiesChanged {
                interface,
                playback_status,
                metadata,
                position_us,
            } => {
                if interface != DBUS_PLAYER_INTERFACE {
                    return None;
                }
                if let Some(st) = playback_status {
                    self.status = Some(st);
                }
                if let Some(pos) = position_us {
                    self.position_us = pos;
                }
                if let Some(meta) = metadata {
                    self.position_us = meta.position_us;
                    self.metadata = Some(meta);
                }
                self.build_gateway_update(now_ms)
            }
        }
    }

    fn build_gateway_update(&self, now_ms: u64) -> Option<GatewayStatusUpdate> {
        let status = self.status?;
        match status {
            PlaybackStatus::Playing => {
                let meta = self.metadata.as_ref()?;
                if meta.title.is_empty() {
                    return None;
                }
                let position_ms = self.position_us / 1_000;
                let length_ms = meta.length_us / 1_000;
                let start_ms = now_ms.saturating_sub(position_ms);
                let end_ms = start_ms.saturating_add(length_ms);
                let activity = ListeningActivity {
                    id: ACTIVITY_ID_SPOTIFY.to_string(),
                    name: ACTIVITY_NAME_SPOTIFY.to_string(),
                    kind: ACTIVITY_TYPE_LISTENING,
                    details: meta.title.clone(),
                    state: meta.artists.join("; "),
                    large_text: meta.album.clone(),
                    timestamps: ActivityTimestamps {
                        start: start_ms,
                        end: end_ms,
                    },
                    flags: ACTIVITY_FLAGS_PLAY_SYNC,
                };
                Some(GatewayStatusUpdate {
                    op: GATEWAY_OPCODE_STATUS_UPDATE,
                    status: "online".to_string(),
                    activities: vec![activity],
                    afk: false,
                })
            }
            PlaybackStatus::Paused | PlaybackStatus::Stopped => {
                Some(GatewayStatusUpdate::clear_presence())
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_metadata() -> MprisMetadata {
        let mut map = BTreeMap::new();
        map.insert(
            "mpris:trackid".to_string(),
            "/com/spotify/track/4uLU6hMCjMI75M1A2tKUQC".to_string(),
        );
        map.insert(
            "xesam:title".to_string(),
            "Never Gonna Give You Up".to_string(),
        );
        map.insert("xesam:artist".to_string(), "Rick Astley".to_string());
        map.insert(
            "xesam:album".to_string(),
            "Whenever You Need Somebody".to_string(),
        );
        map.insert("mpris:length".to_string(), "213000000".to_string());
        MprisMetadata::from_mpris_map(&map, 30_000_000)
    }

    #[test]
    fn extracts_mpris_metadata_fields() {
        let meta = sample_metadata();
        assert_eq!(meta.title, "Never Gonna Give You Up");
        assert_eq!(meta.artists, vec!["Rick Astley".to_string()]);
        assert_eq!(meta.album, "Whenever You Need Somebody");
        assert_eq!(meta.length_us, 213_000_000);
        assert_eq!(meta.position_us, 30_000_000);
    }

    #[test]
    fn dispatches_listening_opcode_3_with_synced_timestamps() {
        let mut client = SpotifyMprisClient::new();
        let update = client
            .handle_signal(
                MprisSignal::PropertiesChanged {
                    interface: DBUS_PLAYER_INTERFACE.to_string(),
                    playback_status: Some(PlaybackStatus::Playing),
                    metadata: Some(sample_metadata()),
                    position_us: None,
                },
                1_700_000_050_000,
            )
            .expect("expected GatewayStatusUpdate");

        assert_eq!(update.op, 3);
        assert_eq!(update.activities.len(), 1);
        let act = &update.activities[0];
        assert_eq!(act.kind, 2);
        assert_eq!(act.name, "Spotify");
        assert_eq!(act.details, "Never Gonna Give You Up");
        assert_eq!(act.state, "Rick Astley");
        assert_eq!(act.timestamps.start, 1_700_000_020_000);
        assert_eq!(act.timestamps.end, 1_700_000_233_000);
        assert!(act.timestamps.end > act.timestamps.start);
    }

    #[test]
    fn clears_presence_when_paused_or_stopped() {
        let mut client = SpotifyMprisClient::new();
        let _ = client.handle_signal(
            MprisSignal::PropertiesChanged {
                interface: DBUS_PLAYER_INTERFACE.to_string(),
                playback_status: Some(PlaybackStatus::Playing),
                metadata: Some(sample_metadata()),
                position_us: None,
            },
            1_700_000_050_000,
        );

        let paused = client
            .handle_signal(
                MprisSignal::PropertiesChanged {
                    interface: DBUS_PLAYER_INTERFACE.to_string(),
                    playback_status: Some(PlaybackStatus::Paused),
                    metadata: None,
                    position_us: None,
                },
                1_700_000_060_000,
            )
            .expect("expected clear presence on pause");
        assert_eq!(paused.op, 3);
        assert!(paused.activities.is_empty());
    }

    #[test]
    fn clears_presence_when_spotify_closes() {
        let mut client = SpotifyMprisClient::new();
        let _ = client.handle_signal(
            MprisSignal::PropertiesChanged {
                interface: DBUS_PLAYER_INTERFACE.to_string(),
                playback_status: Some(PlaybackStatus::Playing),
                metadata: Some(sample_metadata()),
                position_us: None,
            },
            1_700_000_050_000,
        );

        let closed = client
            .handle_signal(
                MprisSignal::NameOwnerChanged {
                    bus_name: DBUS_BUS_NAME.to_string(),
                    new_owner: String::new(),
                },
                1_700_000_070_000,
            )
            .expect("expected clear presence when Spotify closes");
        assert_eq!(closed.op, 3);
        assert!(closed.activities.is_empty());
    }

    #[test]
    fn ignores_unrelated_dbus_interfaces_and_bus_names() {
        let mut client = SpotifyMprisClient::new();
        assert!(
            client
                .handle_signal(
                    MprisSignal::PropertiesChanged {
                        interface: "org.mpris.MediaPlayer2".to_string(),
                        playback_status: Some(PlaybackStatus::Playing),
                        metadata: Some(sample_metadata()),
                        position_us: None,
                    },
                    1_700_000_050_000,
                )
                .is_none()
        );
        assert!(
            client
                .handle_signal(
                    MprisSignal::NameOwnerChanged {
                        bus_name: "org.mpris.MediaPlayer2.vlc".to_string(),
                        new_owner: String::new(),
                    },
                    1_700_000_050_000,
                )
                .is_none()
        );
    }

    #[test]
    fn playback_status_parses_known_and_unknown_values() {
        assert_eq!(PlaybackStatus::parse("Playing"), PlaybackStatus::Playing);
        assert_eq!(PlaybackStatus::parse("Paused"), PlaybackStatus::Paused);
        assert_eq!(PlaybackStatus::parse("Stopped"), PlaybackStatus::Stopped);
        assert_eq!(PlaybackStatus::parse("Unknown"), PlaybackStatus::Stopped);
    }
}
