#![forbid(unsafe_code)]

//! Task R5-10: Stream Lifecycle & Streamer Mode (`Core: Event Dispatcher`).
//!
//! Handles `STREAM_CREATE`, `STREAM_SERVER_UPDATE`, `STREAM_UPDATE`, and `STREAM_DELETE`
//! gateway events via the core event dispatcher, projects stream lifecycle transitions into UI
//! state snapshots without leaking secret tokens, and manages automatic Streamer Mode toggles
//! (triggered by active self-streams or running external streaming tools such as OBS/XSplit)
//! to automatically suppress tokens, invite links, sensitive personal data, and notifications.
//!
//! Clean-room references:
//! - `docs/feature-backlog.md` row `R5-10`
//! - `waves/RW-40/VOICE/STREAM-GATEWAY-CONTRACT.md` (`A-ROUTER`, `A-PHASES`, `A-REASONS`, `D-27-1..D-27-7`)
//! - `/home/sanya/re-deep/decoded/4d2a64cb5df5fc4e_0`:
//!   - `StreamerModeStore` (`@9370993..9372950`):
//!     `enabled: false, autoToggle: true, hideInstantInvites: true, hidePersonalInformation: true,
//!      disableSounds: true, disableNotifications: true, enableContentProtection: false`
//!   - `RunningGameStore` (`@4773100..4783150`):
//!     `RUNNING_STREAMER_TOOLS_CHANGE` (`OBS`, `XSplit`, `Bebo`, `Streamlabs Desktop`, `Twitch Studio`)

use std::borrow::Cow;
use std::collections::BTreeMap;
use std::fmt;

use chord_codec::etf::EtfValue;
use chord_json::JsonValue;

use crate::dispatch::DispatchEvent;
use crate::logging::redact_secrets;
use crate::stream::{
    LocalStreamAction, StreamAction, StreamDeleteReason, StreamDispatch, StreamEffect, StreamError,
    StreamKey, StreamKind, StreamPhase, StreamState, apply as apply_stream_action, parse_dispatch,
};

/// Placeholder inserted in place of suppressed authentication or RTC tokens.
pub const TOKEN_REDACTED_PLACEHOLDER: &str = "[REDACTED]";

/// Placeholder inserted in place of suppressed Discord invite links in Streamer Mode.
pub const INVITE_REDACTED_PLACEHOLDER: &str = "[INVITE HIDDEN]";

/// Placeholder inserted in place of suppressed personal data (emails, phone numbers, tags) in Streamer Mode.
pub const PERSONAL_DATA_REDACTED_PLACEHOLDER: &str = "[PERSONAL INFO HIDDEN]";

/// Redacted wrapper for the second-RTC token delivered in `STREAM_SERVER_UPDATE`.
///
/// Never prints the raw secret in `Debug` or `Display`; access requires an explicit `.expose()` call.
#[derive(Clone, PartialEq, Eq, Default)]
pub struct RedactedStreamServerToken(String);

impl RedactedStreamServerToken {
    /// Wrap a raw `STREAM_SERVER_UPDATE` token string.
    #[must_use]
    pub fn new(token: impl Into<String>) -> Self {
        Self(token.into())
    }

    /// Explicitly expose the raw RTC token for connecting the Go Live media session.
    #[must_use]
    pub fn expose(&self) -> &str {
        self.0.as_str()
    }

    /// Whether the token is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for RedactedStreamServerToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(TOKEN_REDACTED_PLACEHOLDER)
    }
}

impl fmt::Display for RedactedStreamServerToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(TOKEN_REDACTED_PLACEHOLDER)
    }
}

/// Typed event produced by the core event dispatcher for `STREAM_*` gateway dispatches.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamLifecycleEvent {
    /// `STREAM_CREATE` (server -> client, `op 0`).
    StreamCreate {
        /// Canonical wire `stream_key`.
        stream_key: String,
        /// Stream kind (`Guild` or `Call`).
        kind: StreamKind,
        /// Guild snowflake ID (`Some` for `Guild`, `None` for `Call`).
        guild_id: Option<u64>,
        /// Voice channel snowflake ID.
        channel_id: u64,
        /// Streamer user snowflake ID.
        owner_id: u64,
        /// Voice region assigned to the stream.
        region: Option<String>,
        /// Full viewer list (`None` if omitted, `Some([])` if 0 viewers).
        viewer_ids: Option<Vec<u64>>,
        /// Dedicated RTC server ID for the stream.
        rtc_server_id: Option<String>,
        /// Dedicated RTC channel ID for the stream.
        rtc_channel_id: Option<u64>,
        /// Whether the stream is currently paused.
        paused: bool,
    },
    /// `STREAM_SERVER_UPDATE` (server -> client, `op 0`).
    ///
    /// Note: The raw secret `token` is stored in [`RedactedStreamServerToken`] so `Debug`
    /// never leaks credentials.
    StreamServerUpdate {
        /// Canonical wire `stream_key`.
        stream_key: String,
        /// Media server endpoint.
        endpoint: String,
        /// Redacted media server token.
        token: RedactedStreamServerToken,
    },
    /// `STREAM_UPDATE` (server -> client, `op 0`).
    StreamUpdate {
        /// Canonical wire `stream_key`.
        stream_key: String,
        /// Updated voice region.
        region: Option<String>,
        /// Updated viewer list (`None` if omitted, `Some([])` if 0 viewers).
        viewer_ids: Option<Vec<u64>>,
        /// Updated pause state.
        paused: Option<bool>,
    },
    /// `STREAM_DELETE` (server -> client, `op 0`).
    StreamDelete {
        /// Canonical wire `stream_key`.
        stream_key: String,
        /// Protocol deletion reason (`user_requested`, `stream_full`, `unauthorized`, `safety_guild_rate_limited`, or `Unknown`).
        reason: Option<StreamDeleteReason>,
        /// Temporary outage flag (`true` transitions stream to `Reconnecting`).
        unavailable: bool,
    },
}

/// Configuration for Streamer Mode matching Discord's `StreamerModeStore` (`@9370993`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamerModeConfig {
    /// Manual Streamer Mode toggle (`enabled`).
    pub manual_enabled: bool,
    /// Automatically enable Streamer Mode when starting a stream or running a streamer tool (`autoToggle`).
    pub auto_toggle: bool,
    /// Hide instant invite links (`hideInstantInvites`).
    pub hide_instant_invites: bool,
    /// Hide sensitive personal data such as emails, phone numbers, discriminators (`hidePersonalInformation`).
    pub hide_personal_information: bool,
    /// Suppress desktop notifications while Streamer Mode is active (`disableNotifications`).
    pub disable_notifications: bool,
    /// Suppress notification and voice join/leave sounds while Streamer Mode is active (`disableSounds`).
    pub disable_sounds: bool,
    /// Enable OS window content protection if supported (`enableContentProtection`).
    pub enable_content_protection: bool,
}

impl Default for StreamerModeConfig {
    fn default() -> Self {
        Self {
            manual_enabled: false,
            auto_toggle: true,
            hide_instant_invites: true,
            hide_personal_information: true,
            disable_notifications: true,
            disable_sounds: true,
            enable_content_protection: false,
        }
    }
}

/// Source that triggered Streamer Mode activation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamerModeTriggerSource {
    /// Streamer Mode is currently inactive.
    None,
    /// Manually enabled by the user.
    Manual,
    /// Automatically enabled because the current user has an active or reconnecting Go Live stream.
    ActiveSelfStream,
    /// Automatically enabled because a known external streaming tool (OBS, XSplit, Streamlabs, etc.) is running.
    ExternalStreamerTool,
    /// Automatically enabled by both an active Go Live self-stream and a running external streaming tool.
    ActiveSelfStreamAndTool,
}

/// Running OS process entry inspected for streamer tool auto-detection (`RunningGameStore` `@4773100`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamerToolProcess {
    /// Process ID (`pid`).
    pub pid: u32,
    /// Executable file name (e.g., `"obs"`, `"obs64.exe"`).
    pub exe_name: String,
    /// Full executable path (e.g., `"/usr/bin/obs"`).
    pub exe_path: String,
}

/// Detected known streamer tool descriptor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamerToolDescriptor {
    /// Process ID (`pid`).
    pub pid: u32,
    /// Canonical tool display name (`"OBS"`, `"XSplit"`, `"Bebo"`, `"Streamlabs Desktop"`, `"Twitch Studio"`).
    pub tool_name: &'static str,
    /// Matched executable name.
    pub matched_executable: String,
}

/// Reason why a notification or sound was suppressed by Streamer Mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamerModeSuppressionReason {
    /// Suppressed because Streamer Mode is active and `disable_notifications` / `disable_sounds` is enabled.
    StreamerModeActive,
}

/// Outcome of evaluating a desktop notification against Streamer Mode policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamerModeNotificationDecision {
    /// Notification is permitted by Streamer Mode policy.
    Allowed,
    /// Notification is suppressed by Streamer Mode policy.
    Suppressed(StreamerModeSuppressionReason),
}

impl StreamerModeNotificationDecision {
    /// Whether the notification is suppressed.
    #[must_use]
    pub fn is_suppressed(self) -> bool {
        matches!(self, Self::Suppressed(_))
    }
}

/// Detailed result of sanitizing text under Streamer Mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SanitizedContentReport {
    /// Sanitized text safe for rendering on stream.
    pub sanitized_text: String,
    /// Number of authentication/session/RTC tokens suppressed.
    pub tokens_suppressed: u64,
    /// Number of Discord invite links suppressed.
    pub invite_links_suppressed: u64,
    /// Number of personal data items (emails, phone numbers, discriminators) redacted.
    pub personal_data_redactions: u64,
}

/// UI projection for a single stream managed by [`StreamLifecycleController`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StreamUiEntry {
    /// Canonical wire stream key.
    pub stream_key: String,
    /// Stream kind (`Guild` or `Call`).
    pub kind: StreamKind,
    /// Guild snowflake ID (`Some` for `Guild`).
    pub guild_id: Option<u64>,
    /// Voice channel snowflake ID.
    pub channel_id: u64,
    /// Stream owner (broadcaster) snowflake ID.
    pub owner_id: u64,
    /// Whether the stream belongs to the local authenticated user.
    pub is_self_stream: bool,
    /// Current lifecycle phase (`Connecting`, `Active`, `Paused`, `Reconnecting`, `Ended`, `Failed`).
    pub phase: StreamPhase,
    /// Wire phase string (`"CONNECTING"`..`"FAILED"`).
    pub phase_wire: &'static str,
    /// Voice region of the stream.
    pub region: Option<String>,
    /// Full viewer IDs list (`None` if omitted, `Some([])` if 0 viewers).
    pub viewer_ids: Option<Vec<u64>>,
    /// Viewer count (`None` if `viewer_ids` was omitted).
    pub viewer_count: Option<usize>,
    /// Whether the stream is paused.
    pub paused: bool,
    /// Whether the dedicated second-RTC transport has received `STREAM_SERVER_UPDATE`.
    pub transport_ready: bool,
    /// Sanitized hostname of the stream media server (without port, query, or token).
    pub endpoint_host: Option<String>,
    /// Whether a non-empty RTC token is present for this stream.
    pub token_present: bool,
    /// Last `STREAM_DELETE` reason, if any.
    pub delete_reason: Option<StreamDeleteReason>,
    /// Projected UI `data-reason` attribute (`"viewer-full"`, `"permission-channel"`, `"safety-rate-limited"`).
    pub ui_data_reason: Option<&'static str>,
}

/// Complete UI state snapshot produced after applying stream lifecycle events.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamUiStateSnapshot {
    /// All tracked streams in deterministic key order.
    pub active_streams: Vec<StreamUiEntry>,
    /// Whether the current user has an active, paused, or reconnecting self-stream.
    pub is_self_streaming: bool,
    /// Whether the `LIVE` badge should be displayed for the local user.
    pub live_badge_visible: bool,
    /// Whether Streamer Mode is currently active (either automatically or manually).
    pub streamer_mode_active: bool,
    /// Whether the purple Streamer Mode notice banner is visible in the UI.
    pub streamer_mode_banner_visible: bool,
    /// Trigger source for Streamer Mode.
    pub trigger_source: Option<StreamerModeTriggerSource>,
    /// Stream keys that received a `stream_full` deletion alert.
    pub viewer_full_alert_stream_keys: Vec<String>,
    /// Guild IDs that triggered a `safety_guild_rate_limited` alert.
    pub safety_rate_limited_guilds: Vec<u64>,
}

/// Observable telemetry for stream lifecycle events and Streamer Mode suppressions.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct StreamLifecycleTelemetry {
    /// Number of valid `STREAM_CREATE` dispatches processed.
    pub stream_creates_handled: u64,
    /// Number of valid `STREAM_SERVER_UPDATE` dispatches processed.
    pub stream_server_updates_handled: u64,
    /// Number of valid `STREAM_UPDATE` dispatches processed.
    pub stream_updates_handled: u64,
    /// Number of valid `STREAM_DELETE` dispatches processed.
    pub stream_deletes_handled: u64,
    /// Number of malformed or invalid `STREAM_*` dispatches rejected.
    pub rejected_dispatches: u64,
    /// Number of times Streamer Mode was automatically activated.
    pub auto_toggles_activated: u64,
    /// Number of times Streamer Mode was automatically deactivated.
    pub auto_toggles_deactivated: u64,
    /// Total tokens suppressed in Streamer Mode.
    pub tokens_suppressed: u64,
    /// Total invite links suppressed in Streamer Mode.
    pub invite_links_suppressed: u64,
    /// Total personal data items redacted in Streamer Mode.
    pub personal_data_redactions: u64,
    /// Total notifications suppressed in Streamer Mode.
    pub notifications_suppressed: u64,
}

/// Errors returned by [`StreamLifecycleController`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StreamLifecycleError {
    /// Event name is not one of `STREAM_CREATE`, `STREAM_SERVER_UPDATE`, `STREAM_UPDATE`, `STREAM_DELETE`.
    UnsupportedEvent(String),
    /// Payload failed `chord_core::stream` protocol validation.
    InvalidStreamPayload(StreamError),
}

impl fmt::Display for StreamLifecycleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedEvent(name) => {
                write!(f, "event `{name}` is not a stream lifecycle dispatch")
            }
            Self::InvalidStreamPayload(err) => write!(f, "invalid stream payload: {err}"),
        }
    }
}

impl std::error::Error for StreamLifecycleError {}

#[derive(Debug, Clone, Default)]
struct StreamTransportMeta {
    endpoint_host: Option<String>,
    token_present: bool,
}

/// Stateful controller integrating the `STREAM_*` event dispatcher, pure `StreamState` reducer,
/// external streamer tool detector, and automatic Streamer Mode privacy guardrails.
#[derive(Debug, Clone)]
pub struct StreamLifecycleController {
    self_user_id: u64,
    stream_state: StreamState,
    transport_meta: BTreeMap<StreamKey, StreamTransportMeta>,
    viewer_full_alerts: Vec<String>,
    safety_rate_limited_guilds: Vec<u64>,
    config: StreamerModeConfig,
    running_tools: Vec<StreamerToolDescriptor>,
    was_auto_active: bool,
    telemetry: StreamLifecycleTelemetry,
}

impl StreamLifecycleController {
    /// Create a new controller for the given authenticated `self_user_id`.
    #[must_use]
    pub fn new(self_user_id: u64) -> Self {
        Self {
            self_user_id,
            stream_state: StreamState::new(),
            transport_meta: BTreeMap::new(),
            viewer_full_alerts: Vec::new(),
            safety_rate_limited_guilds: Vec::new(),
            config: StreamerModeConfig::default(),
            running_tools: Vec::new(),
            was_auto_active: false,
            telemetry: StreamLifecycleTelemetry::default(),
        }
    }

    /// Update the authenticated user ID (e.g., on `READY`).
    pub fn set_self_user_id(&mut self, self_user_id: u64) {
        self.self_user_id = self_user_id;
        self.sync_auto_toggle_telemetry();
    }

    /// Current authenticated user ID.
    #[must_use]
    pub fn self_user_id(&self) -> u64 {
        self.self_user_id
    }

    /// Current Streamer Mode configuration.
    #[must_use]
    pub fn config(&self) -> &StreamerModeConfig {
        &self.config
    }

    /// Update Streamer Mode configuration.
    pub fn set_config(&mut self, config: StreamerModeConfig) {
        self.config = config;
        self.sync_auto_toggle_telemetry();
    }

    /// Manually enable or disable Streamer Mode (`STREAMER_MODE_UPDATE{key: "enabled"}`).
    pub fn set_manual_enabled(&mut self, enabled: bool) {
        self.config.manual_enabled = enabled;
    }

    /// Whether the current user has an active, paused, or reconnecting self-stream.
    #[must_use]
    pub fn has_active_self_stream(&self) -> bool {
        self.self_user_id != 0
            && self.stream_state.streams().values().any(|s| {
                s.key.owner_id == self.self_user_id
                    && matches!(
                        s.phase,
                        StreamPhase::Active | StreamPhase::Paused | StreamPhase::Reconnecting
                    )
            })
    }

    /// Currently detected running external streamer tools.
    #[must_use]
    pub fn running_streamer_tools(&self) -> &[StreamerToolDescriptor] {
        &self.running_tools
    }

    /// Update the list of running OS processes and recompute `RUNNING_STREAMER_TOOLS_CHANGE`.
    pub fn update_running_processes(&mut self, processes: &[StreamerToolProcess]) {
        self.running_tools = detect_running_streamer_tools(processes);
        self.sync_auto_toggle_telemetry();
    }

    /// Determine which trigger source is keeping Streamer Mode active.
    #[must_use]
    pub fn active_trigger_source(&self) -> StreamerModeTriggerSource {
        if self.config.auto_toggle {
            let self_stream = self.has_active_self_stream();
            let has_tool = !self.running_tools.is_empty();
            match (self_stream, has_tool) {
                (true, true) => return StreamerModeTriggerSource::ActiveSelfStreamAndTool,
                (true, false) => return StreamerModeTriggerSource::ActiveSelfStream,
                (false, true) => return StreamerModeTriggerSource::ExternalStreamerTool,
                (false, false) => {}
            }
        }
        if self.config.manual_enabled {
            StreamerModeTriggerSource::Manual
        } else {
            StreamerModeTriggerSource::None
        }
    }

    /// Whether Streamer Mode is currently active (either via auto-toggle or manual toggle).
    #[must_use]
    pub fn is_streamer_mode_active(&self) -> bool {
        !matches!(
            self.active_trigger_source(),
            StreamerModeTriggerSource::None
        )
    }

    /// Effective `hideInstantInvites` getter (`this.enabled && settings.hideInstantInvites`).
    #[must_use]
    pub fn should_hide_instant_invites(&self) -> bool {
        self.is_streamer_mode_active() && self.config.hide_instant_invites
    }

    /// Effective `hidePersonalInformation` getter (`this.enabled && settings.hidePersonalInformation`).
    #[must_use]
    pub fn should_hide_personal_information(&self) -> bool {
        self.is_streamer_mode_active() && self.config.hide_personal_information
    }

    /// Effective `disableNotifications` getter (`this.enabled && settings.disableNotifications`).
    #[must_use]
    pub fn should_suppress_notifications(&self) -> bool {
        self.is_streamer_mode_active() && self.config.disable_notifications
    }

    /// Effective `disableSounds` getter (`this.enabled && settings.disableSounds`).
    #[must_use]
    pub fn should_suppress_sounds(&self) -> bool {
        self.is_streamer_mode_active() && self.config.disable_sounds
    }

    /// Record a local watch intent (`Connecting` phase prior to `STREAM_CREATE`).
    pub fn request_watch_stream(
        &mut self,
        raw_stream_key: &str,
    ) -> Result<StreamUiStateSnapshot, StreamLifecycleError> {
        let key = StreamKey::parse(raw_stream_key)
            .map_err(|reason| StreamLifecycleError::InvalidStreamPayload(StreamError::InvalidKey { reason }))?;
        let _ = apply_stream_action(
            &mut self.stream_state,
            StreamAction::Local(LocalStreamAction::WatchRequested { key }),
        );
        self.sync_auto_toggle_telemetry();
        Ok(self.ui_state())
    }

    /// Reset all active streams on Gateway `READY` and return the updated UI snapshot.
    pub fn reset_on_ready(&mut self) -> StreamUiStateSnapshot {
        let _ = apply_stream_action(&mut self.stream_state, StreamAction::Ready);
        self.transport_meta.clear();
        self.viewer_full_alerts.clear();
        self.safety_rate_limited_guilds.clear();
        self.sync_auto_toggle_telemetry();
        self.ui_state()
    }

    /// Handle an incoming `chord_core::DispatchEvent` for `STREAM_CREATE`, `STREAM_SERVER_UPDATE`,
    /// `STREAM_UPDATE`, or `STREAM_DELETE`, updating internal stream state, automatic Streamer Mode,
    /// and returning the new [`StreamUiStateSnapshot`].
    pub fn handle_dispatch_event(
        &mut self,
        event: &DispatchEvent,
    ) -> Result<StreamUiStateSnapshot, StreamLifecycleError> {
        let name = effective_dispatch_name(event)
            .ok_or_else(|| StreamLifecycleError::UnsupportedEvent(event.name.clone()))?;
        if !matches!(
            name,
            "STREAM_CREATE" | "STREAM_SERVER_UPDATE" | "STREAM_UPDATE" | "STREAM_DELETE"
        ) {
            return Err(StreamLifecycleError::UnsupportedEvent(name.to_string()));
        }

        let data_cow = normalize_payload_data(&event.data);
        let parsed = match parse_dispatch(name, data_cow.as_ref()) {
            Some(Ok(dispatch)) => dispatch,
            Some(Err(err)) => {
                self.telemetry.rejected_dispatches =
                    self.telemetry.rejected_dispatches.saturating_add(1);
                return Err(StreamLifecycleError::InvalidStreamPayload(err));
            }
            None => {
                return Err(StreamLifecycleError::UnsupportedEvent(name.to_string()));
            }
        };

        match &parsed {
            StreamDispatch::Created(_) => {
                self.telemetry.stream_creates_handled =
                    self.telemetry.stream_creates_handled.saturating_add(1);
            }
            StreamDispatch::ServerUpdate(su) => {
                self.telemetry.stream_server_updates_handled = self
                    .telemetry
                    .stream_server_updates_handled
                    .saturating_add(1);
                self.transport_meta.insert(
                    su.stream_key.clone(),
                    StreamTransportMeta {
                        endpoint_host: extract_endpoint_host(&su.endpoint),
                        token_present: !su.token.expose().is_empty(),
                    },
                );
            }
            StreamDispatch::Update(_) => {
                self.telemetry.stream_updates_handled =
                    self.telemetry.stream_updates_handled.saturating_add(1);
            }
            StreamDispatch::Deleted(del) => {
                self.telemetry.stream_deletes_handled =
                    self.telemetry.stream_deletes_handled.saturating_add(1);
                self.transport_meta.remove(&del.stream_key);
            }
        }

        let effects = apply_stream_action(
            &mut self.stream_state,
            StreamAction::Dispatch {
                seq: event.seq,
                event: parsed,
            },
        );

        for effect in effects {
            match effect {
                StreamEffect::CloseStreamRtc { key } => {
                    self.transport_meta.remove(&key);
                }
                StreamEffect::NotifyViewerFull { key } => {
                    let wire = key.to_wire();
                    if !self.viewer_full_alerts.contains(&wire) {
                        self.viewer_full_alerts.push(wire);
                    }
                }
                StreamEffect::ShowSafetyRateLimit { guild_id } => {
                    if !self.safety_rate_limited_guilds.contains(&guild_id) {
                        self.safety_rate_limited_guilds.push(guild_id);
                    }
                }
                StreamEffect::OpenStreamRtc { .. } => {}
            }
        }

        self.sync_auto_toggle_telemetry();
        Ok(self.ui_state())
    }

    /// Build a secret-free [`StreamUiStateSnapshot`] for the UI layer.
    #[must_use]
    pub fn ui_state(&self) -> StreamUiStateSnapshot {
        let is_self_streaming = self.has_active_self_stream();
        let trigger = self.active_trigger_source();
        let streamer_mode_active = !matches!(trigger, StreamerModeTriggerSource::None);

        let active_streams = self
            .stream_state
            .streams()
            .values()
            .map(|stream| {
                let meta = self
                    .transport_meta
                    .get(&stream.key)
                    .cloned()
                    .unwrap_or_default();
                let is_self_stream =
                    self.self_user_id != 0 && stream.key.owner_id == self.self_user_id;
                StreamUiEntry {
                    stream_key: stream.key.to_wire(),
                    kind: stream.key.kind,
                    guild_id: stream.key.guild_id,
                    channel_id: stream.key.channel_id,
                    owner_id: stream.key.owner_id,
                    is_self_stream,
                    phase: stream.phase,
                    phase_wire: stream.phase.as_wire(),
                    region: stream.region.clone(),
                    viewer_ids: stream.viewer_ids.clone(),
                    viewer_count: stream.viewer_ids.as_ref().map(Vec::len),
                    paused: stream.paused,
                    transport_ready: stream.transport_ready,
                    endpoint_host: if stream.transport_ready {
                        meta.endpoint_host
                    } else {
                        None
                    },
                    token_present: stream.transport_ready && meta.token_present,
                    delete_reason: stream.delete_reason.clone(),
                    ui_data_reason: stream
                        .delete_reason
                        .as_ref()
                        .and_then(StreamDeleteReason::ui_data_reason),
                }
            })
            .collect();

        StreamUiStateSnapshot {
            active_streams,
            is_self_streaming,
            live_badge_visible: is_self_streaming,
            streamer_mode_active,
            streamer_mode_banner_visible: streamer_mode_active,
            trigger_source: Some(trigger),
            viewer_full_alert_stream_keys: self.viewer_full_alerts.clone(),
            safety_rate_limited_guilds: self.safety_rate_limited_guilds.clone(),
        }
    }

    /// Sanitize display text according to current Streamer Mode state and update telemetry counters.
    ///
    /// When Streamer Mode is active:
    /// - Always suppresses tokens (Discord JWTs, MFA tokens, Bearer headers, `token=...` secrets);
    /// - Suppresses Discord invite links if `hide_instant_invites` is true;
    /// - Redacts personal data (emails, phone numbers, legacy `#1234` discriminators) if `hide_personal_information` is true.
    pub fn sanitize_display_text(&mut self, input: &str) -> String {
        let active = self.is_streamer_mode_active();
        let report = sanitize_stream_content(input, &self.config, active);
        self.telemetry.tokens_suppressed = self
            .telemetry
            .tokens_suppressed
            .saturating_add(report.tokens_suppressed);
        self.telemetry.invite_links_suppressed = self
            .telemetry
            .invite_links_suppressed
            .saturating_add(report.invite_links_suppressed);
        self.telemetry.personal_data_redactions = self
            .telemetry
            .personal_data_redactions
            .saturating_add(report.personal_data_redactions);
        report.sanitized_text
    }

    /// Evaluate whether a desktop notification for `(channel_id, content)` should be shown or suppressed.
    pub fn evaluate_notification(
        &mut self,
        _channel_id: &str,
        _content: &str,
    ) -> StreamerModeNotificationDecision {
        if self.should_suppress_notifications() {
            self.telemetry.notifications_suppressed =
                self.telemetry.notifications_suppressed.saturating_add(1);
            StreamerModeNotificationDecision::Suppressed(
                StreamerModeSuppressionReason::StreamerModeActive,
            )
        } else {
            StreamerModeNotificationDecision::Allowed
        }
    }

    /// Access observable telemetry counters.
    #[must_use]
    pub fn telemetry(&self) -> &StreamLifecycleTelemetry {
        &self.telemetry
    }

    fn sync_auto_toggle_telemetry(&mut self) {
        let auto_now = self.config.auto_toggle
            && (self.has_active_self_stream() || !self.running_tools.is_empty());
        if auto_now && !self.was_auto_active {
            self.telemetry.auto_toggles_activated =
                self.telemetry.auto_toggles_activated.saturating_add(1);
        } else if !auto_now && self.was_auto_active {
            self.telemetry.auto_toggles_deactivated =
                self.telemetry.auto_toggles_deactivated.saturating_add(1);
        }
        self.was_auto_active = auto_now;
    }
}

/// Pure mapper from a `chord_core::DispatchEvent` into a [`StreamLifecycleEvent`].
///
/// Returns `None` for non-stream events or malformed payloads (never panics).
#[must_use]
pub fn map_gateway_stream_dispatch(event: &DispatchEvent) -> Option<StreamLifecycleEvent> {
    let name = effective_dispatch_name(event)?;
    let data_cow = normalize_payload_data(&event.data);
    let parsed = parse_dispatch(name, data_cow.as_ref())?.ok()?;

    match parsed {
        StreamDispatch::Created(created) => {
            let stream_key = created.stream_key.to_wire();
            Some(StreamLifecycleEvent::StreamCreate {
                stream_key,
                kind: created.stream_key.kind,
                guild_id: created.stream_key.guild_id,
                channel_id: created.stream_key.channel_id,
                owner_id: created.stream_key.owner_id,
                region: created.region,
                viewer_ids: created.viewer_ids,
                rtc_server_id: created.rtc_server_id,
                rtc_channel_id: created.rtc_channel_id,
                paused: created.paused.unwrap_or(false),
            })
        }
        StreamDispatch::ServerUpdate(server_update) => {
            let stream_key = server_update.stream_key.to_wire();
            Some(StreamLifecycleEvent::StreamServerUpdate {
                stream_key,
                endpoint: server_update.endpoint,
                token: RedactedStreamServerToken::new(server_update.token.expose()),
            })
        }
        StreamDispatch::Update(update) => {
            let stream_key = update.stream_key.to_wire();
            Some(StreamLifecycleEvent::StreamUpdate {
                stream_key,
                region: update.region,
                viewer_ids: update.viewer_ids,
                paused: update.paused,
            })
        }
        StreamDispatch::Deleted(deleted) => {
            let stream_key = deleted.stream_key.to_wire();
            Some(StreamLifecycleEvent::StreamDelete {
                stream_key,
                reason: deleted.reason,
                unavailable: deleted.unavailable.unwrap_or(false),
            })
        }
    }
}

/// Extract the [`RedactedStreamServerToken`] from a `STREAM_SERVER_UPDATE` dispatch event.
#[must_use]
pub fn extract_stream_server_token(event: &DispatchEvent) -> Option<RedactedStreamServerToken> {
    match map_gateway_stream_dispatch(event)? {
        StreamLifecycleEvent::StreamServerUpdate { token, .. } if !token.is_empty() => Some(token),
        _ => None,
    }
}

/// Detect known external streaming tools from a list of running OS processes.
///
/// Matches clean-room RE oracle `RunningGameStore` (`@4773100..4774950`):
/// - `OBS`: `obs`, `obs.exe`, `obs32.exe`, `obs64.exe`, `obs.app`
/// - `XSplit`: `xsplit.gamecaster.exe`, `xsplit.core.exe`, `gamecaster.exe`
/// - `Bebo`: `bebo.exe`
/// - `Streamlabs Desktop`: `streamlabs obs.exe`, `streamlabs desktop.exe`, `streamlabs desktop.app`
/// - `Twitch Studio`: `twitchstudio.exe`, `twitch studio.app`
#[must_use]
pub fn detect_running_streamer_tools(
    processes: &[StreamerToolProcess],
) -> Vec<StreamerToolDescriptor> {
    let mut detected = Vec::new();
    for proc_entry in processes {
        if let Some((tool_name, matched)) = classify_streamer_tool_process(proc_entry) {
            detected.push(StreamerToolDescriptor {
                pid: proc_entry.pid,
                tool_name,
                matched_executable: matched,
            });
        }
    }
    detected
}

fn classify_streamer_tool_process(
    proc_entry: &StreamerToolProcess,
) -> Option<(&'static str, String)> {
    let exe_lower = proc_entry.exe_name.trim().to_ascii_lowercase();
    let path_lower = proc_entry
        .exe_path
        .trim()
        .replace('\\', "/")
        .to_ascii_lowercase();
    let basename = if !exe_lower.is_empty() {
        exe_lower.as_str()
    } else {
        path_lower.rsplit('/').next().unwrap_or("")
    };

    let tool = match basename {
        "obs" | "obs.exe" | "obs32.exe" | "obs64.exe" | "obs.app" | "obs-studio" => Some("OBS"),
        "xsplit.gamecaster.exe" | "xsplit.core.exe" | "gamecaster.exe" | "xsplit" => {
            Some("XSplit")
        }
        "bebo.exe" | "bebo" => Some("Bebo"),
        "streamlabs obs.exe"
        | "streamlabs desktop.exe"
        | "streamlabs desktop.app"
        | "streamlabs-obs" => Some("Streamlabs Desktop"),
        "twitchstudio.exe" | "twitch studio.app" | "twitchstudio" => Some("Twitch Studio"),
        _ => {
            if path_lower.ends_with("/obs.app") || path_lower.ends_with("/obs/obs.exe") {
                Some("OBS")
            } else if path_lower.ends_with("/streamlabs desktop.app") {
                Some("Streamlabs Desktop")
            } else if path_lower.ends_with("/twitch studio.app") {
                Some("Twitch Studio")
            } else {
                None
            }
        }
    }?;

    Some((tool, basename.to_string()))
}

/// Sanitize content under Streamer Mode settings, returning the sanitized text and suppression counts.
#[must_use]
pub fn sanitize_stream_content(
    input: &str,
    config: &StreamerModeConfig,
    streamer_mode_active: bool,
) -> SanitizedContentReport {
    if !streamer_mode_active || input.is_empty() {
        return SanitizedContentReport {
            sanitized_text: input.to_string(),
            tokens_suppressed: 0,
            invite_links_suppressed: 0,
            personal_data_redactions: 0,
        };
    }

    // 1. Always suppress authentication, MFA, Bearer, and RTC tokens when streamer mode is active
    let (after_tokens, tokens_suppressed) = sanitize_tokens(input);

    // 2. Suppress Discord invite links if `hide_instant_invites` is enabled
    let (after_invites, invite_links_suppressed) = if config.hide_instant_invites {
        sanitize_invite_links(&after_tokens)
    } else {
        (after_tokens, 0)
    };

    // 3. Suppress personal information (emails, phone numbers, discriminators) if `hide_personal_information` is enabled
    let (after_pii, personal_data_redactions) = if config.hide_personal_information {
        sanitize_personal_data(&after_invites)
    } else {
        (after_invites, 0)
    };

    SanitizedContentReport {
        sanitized_text: after_pii,
        tokens_suppressed,
        invite_links_suppressed,
        personal_data_redactions,
    }
}

/// Suppress Discord JWT tokens, MFA tokens (`mfa.*`), Bearer/Bot headers, and `token=...` secrets.
#[must_use]
pub fn sanitize_tokens(input: &str) -> (String, u64) {
    if input.is_empty() {
        return (String::new(), 0);
    }
    let before_markers = count_occurrences(input, TOKEN_REDACTED_PLACEHOLDER);
    let redacted = redact_secrets(input);
    let after_markers = count_occurrences(&redacted, TOKEN_REDACTED_PLACEHOLDER);
    let suppressed = after_markers.saturating_sub(before_markers);
    (redacted, suppressed)
}

/// Suppress Discord instant invite links (`discord.gg/<code>`, `discord.com/invite/<code>`,
/// `discordapp.com/invite/<code>`, `ptb.discord.com/invite/<code>`, `canary.discord.com/invite/<code>`).
#[must_use]
pub fn sanitize_invite_links(input: &str) -> (String, u64) {
    if input.is_empty() {
        return (String::new(), 0);
    }

    const INVITE_PREFIXES: &[&str] = &[
        "https://discord.gg/",
        "http://discord.gg/",
        "https://www.discord.gg/",
        "discord.gg/",
        "https://discord.com/invite/",
        "http://discord.com/invite/",
        "discord.com/invite/",
        "https://discordapp.com/invite/",
        "http://discordapp.com/invite/",
        "discordapp.com/invite/",
        "https://ptb.discord.com/invite/",
        "ptb.discord.com/invite/",
        "https://canary.discord.com/invite/",
        "canary.discord.com/invite/",
    ];

    let mut result = String::with_capacity(input.len());
    let mut suppressed = 0_u64;
    let mut cursor = 0;

    while cursor < input.len() {
        let remaining = &input[cursor..];
        let remaining_lower = remaining.to_ascii_lowercase();

        // Find the earliest matching invite prefix in `remaining_lower`
        let mut best_match: Option<(usize, usize)> = None;
        for prefix in INVITE_PREFIXES {
            if let Some(pos) = remaining_lower.find(prefix) {
                // Ensure the prefix starts at a word/URL boundary (not preceded by alphanumeric)
                let abs_pos = cursor + pos;
                let boundary_ok = if abs_pos == 0 {
                    true
                } else {
                    input[..abs_pos]
                        .chars()
                        .next_back()
                        .is_none_or(|c| !c.is_ascii_alphanumeric())
                };
                if boundary_ok {
                    match best_match {
                        None => best_match = Some((pos, prefix.len())),
                        Some((best_pos, best_len)) => {
                            if pos < best_pos || (pos == best_pos && prefix.len() > best_len) {
                                best_match = Some((pos, prefix.len()));
                            }
                        }
                    }
                }
            }
        }

        let Some((rel_pos, prefix_len)) = best_match else {
            result.push_str(remaining);
            break;
        };

        result.push_str(&remaining[..rel_pos]);
        let code_start = rel_pos + prefix_len;
        let code_slice = &remaining[code_start..];
        let code_len = code_slice
            .bytes()
            .take_while(|b| b.is_ascii_alphanumeric() || *b == b'-' || *b == b'_')
            .count();

        if code_len >= 2 {
            result.push_str(INVITE_REDACTED_PLACEHOLDER);
            suppressed = suppressed.saturating_add(1);
            cursor += code_start + code_len;
        } else {
            result.push_str(&remaining[rel_pos..code_start]);
            cursor += code_start;
        }
    }

    (result, suppressed)
}

/// Suppress sensitive personal information (email addresses and legacy `#1234` discriminators)
/// when `hide_personal_information` is active in Streamer Mode.
#[must_use]
pub fn sanitize_personal_data(input: &str) -> (String, u64) {
    if input.is_empty() {
        return (String::new(), 0);
    }

    let mut suppressed = 0_u64;
    let mut words_out = String::with_capacity(input.len());
    let mut chars = input.char_indices().peekable();

    while let Some((start, ch)) = chars.next() {
        if ch.is_whitespace() {
            words_out.push(ch);
            continue;
        }
        let mut end = start + ch.len_utf8();
        while let Some(&(_, next_ch)) = chars.peek() {
            if next_ch.is_whitespace() {
                break;
            }
            chars.next();
            end += next_ch.len_utf8();
        }
        let token = &input[start..end];
        let (sanitized_tok, count) = sanitize_personal_token(token);
        suppressed = suppressed.saturating_add(count);
        words_out.push_str(&sanitized_tok);
    }

    (words_out, suppressed)
}

fn sanitize_personal_token(token: &str) -> (String, u64) {
    // Strip surrounding punctuation such as parentheses, commas, quotes
    let trim_chars: &[char] = &['(', ')', '[', ']', '<', '>', ',', ';', '"', '\''];
    let start_idx = token
        .find(|c: char| !trim_chars.contains(&c))
        .unwrap_or(0);
    let end_idx = token
        .rfind(|c: char| !trim_chars.contains(&c) && c != '.' && c != '!' && c != '?')
        .map_or(token.len(), |idx| {
            let ch_len = token[idx..].chars().next().map_or(1, char::len_utf8);
            idx + ch_len
        });

    if start_idx >= end_idx {
        return (token.to_string(), 0);
    }

    let prefix = &token[..start_idx];
    let core = &token[start_idx..end_idx];
    let suffix = &token[end_idx..];

    // 1. Check for email address (`local@domain.tld`)
    if looks_like_email(core) {
        return (
            format!("{prefix}{PERSONAL_DATA_REDACTED_PLACEHOLDER}{suffix}"),
            1,
        );
    }

    // 2. Check for legacy Discord discriminator tag (`Username#1234`)
    if let Some(hash_idx) = core.rfind('#') {
        let user_part = &core[..hash_idx];
        let disc_part = &core[hash_idx + 1..];
        if !user_part.is_empty()
            && disc_part.len() == 4
            && disc_part.bytes().all(|b| b.is_ascii_digit())
        {
            return (
                format!("{prefix}{user_part}#{PERSONAL_DATA_REDACTED_PLACEHOLDER}{suffix}"),
                1,
            );
        }
    }

    (token.to_string(), 0)
}

fn looks_like_email(s: &str) -> bool {
    let Some(at_pos) = s.find('@') else {
        return false;
    };
    if at_pos == 0 || at_pos + 3 >= s.len() {
        return false;
    }
    let local = &s[..at_pos];
    let domain = &s[at_pos + 1..];
    if domain.contains('@') || !domain.contains('.') || domain.starts_with('.') || domain.ends_with('.') {
        return false;
    }
    let local_ok = local
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'_' | b'-' | b'+'));
    let domain_ok = domain
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'.' | b'-'));
    local_ok && domain_ok
}

fn count_occurrences(haystack: &str, needle: &str) -> u64 {
    if needle.is_empty() {
        return 0;
    }
    haystack.matches(needle).count() as u64
}

fn extract_endpoint_host(endpoint: &str) -> Option<String> {
    let trimmed = endpoint.trim();
    if trimmed.is_empty() {
        return None;
    }
    let without_scheme = trimmed
        .strip_prefix("wss://")
        .or_else(|| trimmed.strip_prefix("ws://"))
        .or_else(|| trimmed.strip_prefix("https://"))
        .or_else(|| trimmed.strip_prefix("http://"))
        .unwrap_or(trimmed);
    let without_path = without_scheme.split('/').next().unwrap_or(without_scheme);
    let host = without_path.split(':').next().unwrap_or(without_path).trim();
    if host.is_empty() {
        None
    } else {
        Some(host.to_string())
    }
}

fn effective_dispatch_name(event: &DispatchEvent) -> Option<&str> {
    if !event.name.is_empty() {
        if let Some(kind) = event.kind
            && kind.as_str() != event.name.as_str()
        {
            return None;
        }
        Some(event.name.as_str())
    } else {
        event.kind.map(|k| k.as_str())
    }
}

fn json_value_to_etf(value: &JsonValue) -> EtfValue {
    match value {
        JsonValue::Null => EtfValue::Nil,
        JsonValue::Bool(flag) => EtfValue::Atom(if *flag { "true" } else { "false" }.to_string()),
        JsonValue::Number(num) => {
            if let Some(i) = num.as_i64() {
                EtfValue::Int(i)
            } else if let Some(u) = num.as_u64() {
                EtfValue::Str(u.to_string())
            } else if let Some(f) = num.as_f64() {
                EtfValue::Float(f)
            } else {
                EtfValue::Str(num.as_str().to_string())
            }
        }
        JsonValue::String(text) => EtfValue::Str(text.clone()),
        JsonValue::Array(items) => EtfValue::List(items.iter().map(json_value_to_etf).collect()),
        JsonValue::Object(entries) => EtfValue::Map(
            entries
                .iter()
                .map(|(k, v)| (EtfValue::Str(k.clone()), json_value_to_etf(v)))
                .collect(),
        ),
    }
}

fn normalize_payload_data(data: &EtfValue) -> Cow<'_, EtfValue> {
    if let EtfValue::Str(raw_json) = data
        && raw_json.trim_start().starts_with('{')
        && let Ok(parsed) = chord_json::parse(raw_json)
    {
        return Cow::Owned(json_value_to_etf(&parsed));
    }
    Cow::Borrowed(data)
}
