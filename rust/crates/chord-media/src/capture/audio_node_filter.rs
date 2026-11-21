//! R5-02: Audio Node Filtering (`chord_media::capture::audio_node_filter`).
//!
//! Excludes the client's own audio playback streams, input microphones, and virtual
//! monitoring/loopback sinks from desktop audio capture so only genuine external
//! application playback audio is routed into the WebRTC screen-share audio stream,
//! preventing acoustic echo and feedback loops.
//!
//! Reference Oracles:
//! - `docs/feature-backlog.md` card `R5-02` (Vesktop `venmic.ts` `buildLinkData` contract);
//! - `docs/vesktop-analysis.md` §2.2 ("Audio Source Filtering: tagging application PIDs
//!   and excluding monitoring sinks");
//! - `Serein/crates/discord-voice/src/screen/audio_linux.rs` (`OwnApplication::allows`).

use std::collections::HashSet;
use std::fmt;

/// Maximum permitted byte length for process binaries, node names, application IDs, and rule tokens.
pub const MAX_IDENTIFIER_LEN: usize = 256;
/// Default virtual capture sink name registered for desktop audio loopback mixing.
pub const DEFAULT_VIRTUAL_CAPTURE_SINK_NAME: &str = "chord-capture-sink";
/// Built-in client identifiers recognized as our own client family across host and sandbox namespaces.
pub const BUILTIN_CLIENT_IDENTIFIERS: [&str; 8] = [
    "chord",
    "chord-gpui",
    "discord-gpui",
    "discord-lab",
    "com.discord_lab.discordgpui",
    "venmic",
    "vesktop",
    "rustcord",
];
/// Built-in virtual / loopback node markers that must never be captured as application audio.
pub const BUILTIN_LOOPBACK_MARKERS: [&str; 6] = [
    "venmic",
    "pw-loopback",
    "module-loopback",
    "chord-capture-sink",
    "chord-screen-audio",
    "null-sink",
];
/// Standard stereo channel pair (`FL`, `FR`) used when linking application playback ports.
pub const STEREO_CHANNELS: [&str; 2] = ["FL", "FR"];

/// PipeWire / PulseAudio media class classification for an audio graph node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AudioNodeMediaClass {
    /// Application audio playback stream (`Stream/Output/Audio`).
    StreamOutputAudio,
    /// Application audio recording/capture stream (`Stream/Input/Audio`).
    StreamInputAudio,
    /// Hardware or physical microphone / capture source (`Audio/Source`).
    AudioSource,
    /// Virtual microphone or processed source (`Audio/Source/Virtual`).
    AudioSourceVirtual,
    /// Output sink device (`Audio/Sink`).
    AudioSink,
    /// Duplex audio device (`Audio/Duplex`).
    AudioDuplex,
    /// Non-audio or unrecognized media class.
    Other,
}

impl AudioNodeMediaClass {
    /// Parses a PipeWire `media.class` string into an [`AudioNodeMediaClass`].
    #[must_use]
    pub fn from_media_class_str(raw: &str) -> Self {
        match raw.trim().to_ascii_lowercase().as_str() {
            "stream/output/audio" => Self::StreamOutputAudio,
            "stream/input/audio" => Self::StreamInputAudio,
            "audio/source" => Self::AudioSource,
            "audio/source/virtual" | "source/virtual" => Self::AudioSourceVirtual,
            "audio/sink" | "sink" => Self::AudioSink,
            "audio/duplex" => Self::AudioDuplex,
            _ => Self::Other,
        }
    }

    /// Returns the canonical PipeWire `media.class` string.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::StreamOutputAudio => "Stream/Output/Audio",
            Self::StreamInputAudio => "Stream/Input/Audio",
            Self::AudioSource => "Audio/Source",
            Self::AudioSourceVirtual => "Audio/Source/Virtual",
            Self::AudioSink => "Audio/Sink",
            Self::AudioDuplex => "Audio/Duplex",
            Self::Other => "Other",
        }
    }

    /// Returns `true` if this media class represents a microphone or input capture stream.
    #[must_use]
    pub const fn is_microphone_or_input(self) -> bool {
        matches!(
            self,
            Self::AudioSource | Self::AudioSourceVirtual | Self::StreamInputAudio
        )
    }

    /// Returns `true` if this media class represents a sink or duplex device rather than an app stream.
    #[must_use]
    pub const fn is_sink_or_device(self) -> bool {
        matches!(self, Self::AudioSink | Self::AudioDuplex)
    }
}

/// Metadata descriptor for a single PipeWire / PulseAudio audio node in the system graph.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioNodeDescriptor {
    /// Unique PipeWire / PulseAudio node index (`node.id` / `index`).
    pub node_id: u32,
    /// Optional PipeWire `object.serial`.
    pub serial: Option<u64>,
    /// Originating application process ID (`application.process.id`).
    pub pid: Option<u32>,
    /// Originating process executable name (`application.process.binary`).
    pub process_binary: Option<String>,
    /// Human-readable application name (`application.name`).
    pub application_name: Option<String>,
    /// Freedesktop / Flatpak application identifier (`application.id` / `flatpak.app-id`).
    pub application_id: Option<String>,
    /// Node name in the audio graph (`node.name`).
    pub node_name: String,
    /// Parsed `media.class` of the node.
    pub media_class: AudioNodeMediaClass,
    /// Whether the node is marked virtual (`node.virtual == true`).
    pub is_virtual: bool,
    /// Whether the node is a monitor source (`device.class == "monitor"` or `.monitor`).
    pub is_monitor: bool,
    /// Optional target sink object (`target.object` or `node.target`) that this stream already routes into.
    pub target_object: Option<String>,
}

/// Reason why an audio node was excluded from desktop audio capture.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExclusionReason {
    /// Node belongs to the client's own audio playback stream (prevents echo of remote voice call participants).
    OwnClientStream,
    /// Node is a physical/virtual microphone or recording input stream (`Audio/Source` / `Stream/Input/Audio`).
    MicrophoneInput,
    /// Node is a virtual monitoring sink, loopback device, or hardware sink monitor (`.monitor`).
    VirtualMonitoringSink,
    /// Node would create a direct or transitive acoustic feedback loop with the virtual capture sink.
    AcousticFeedbackLoop,
    /// Node lacks an identifiable non-zero application PID or binary/name metadata.
    UnidentifiableApplication,
    /// Node metadata is invalid (e.g., empty `node_name`, `node_id == 0`, or string exceeding [`MAX_IDENTIFIER_LEN`]).
    InvalidMetadata,
    /// Node matched a user-configured exclusion [`FilterRule`].
    CustomRule(String),
    /// Active include allowlist rules are configured, and this node did not match any of them.
    NotInIncludeAllowlist,
}

impl fmt::Display for ExclusionReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::OwnClientStream => write!(f, "excluded client's own audio stream"),
            Self::MicrophoneInput => write!(f, "excluded input microphone or recording stream"),
            Self::VirtualMonitoringSink => {
                write!(f, "excluded virtual monitoring or loopback sink")
            }
            Self::AcousticFeedbackLoop => {
                write!(f, "excluded node to prevent acoustic feedback loop")
            }
            Self::UnidentifiableApplication => {
                write!(f, "excluded unidentifiable system/anonymous stream")
            }
            Self::InvalidMetadata => {
                write!(f, "excluded node with malformed or oversized metadata")
            }
            Self::CustomRule(rule) => write!(f, "excluded by custom filter rule '{rule}'"),
            Self::NotInIncludeAllowlist => write!(
                f,
                "excluded because node is not in active include allowlist"
            ),
        }
    }
}

/// Result of evaluating an [`AudioNodeDescriptor`] against the [`AudioNodeFilterEngine`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterDecision {
    /// Include this application audio node in the desktop capture mix.
    Include,
    /// Exclude this node with an explicit [`ExclusionReason`].
    Exclude(ExclusionReason),
}

impl FilterDecision {
    /// Returns `true` if the node was accepted for capture.
    #[must_use]
    pub const fn is_included(&self) -> bool {
        matches!(self, Self::Include)
    }
}

/// Metadata field inspected by a configurable [`FilterRule`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterMatchField {
    /// Matches against `AudioNodeDescriptor::process_binary`.
    ProcessBinary,
    /// Matches against `AudioNodeDescriptor::application_name`.
    ApplicationName,
    /// Matches against `AudioNodeDescriptor::application_id`.
    ApplicationId,
    /// Matches against `AudioNodeDescriptor::node_name`.
    NodeName,
    /// Matches against `AudioNodeDescriptor::pid` (decimal string representation).
    ProcessId,
}

/// Matching predicate for a configurable [`FilterRule`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FilterPredicate {
    /// Case-insensitive exact match.
    EqualsIgnoreCase(String),
    /// Case-insensitive substring match.
    ContainsIgnoreCase(String),
    /// Case-insensitive prefix match.
    StartsWithIgnoreCase(String),
    /// Case-insensitive suffix match.
    EndsWithIgnoreCase(String),
}

impl FilterPredicate {
    /// Validates that the predicate pattern is non-empty and within [`MAX_IDENTIFIER_LEN`].
    pub fn validate(&self) -> Result<(), AudioFilterError> {
        let pattern = self.pattern();
        validate_identifier_token(pattern)
    }

    /// Returns the raw pattern string of this predicate.
    #[must_use]
    pub fn pattern(&self) -> &str {
        match self {
            Self::EqualsIgnoreCase(s)
            | Self::ContainsIgnoreCase(s)
            | Self::StartsWithIgnoreCase(s)
            | Self::EndsWithIgnoreCase(s) => s.as_str(),
        }
    }

    /// Evaluates the predicate against a candidate field value.
    #[must_use]
    pub fn matches(&self, candidate: &str) -> bool {
        let cand_trimmed = candidate.trim();
        if cand_trimmed.is_empty() {
            return false;
        }
        match self {
            Self::EqualsIgnoreCase(pat) => cand_trimmed.eq_ignore_ascii_case(pat.trim()),
            Self::ContainsIgnoreCase(pat) => cand_trimmed
                .to_ascii_lowercase()
                .contains(&pat.trim().to_ascii_lowercase()),
            Self::StartsWithIgnoreCase(pat) => cand_trimmed
                .to_ascii_lowercase()
                .starts_with(&pat.trim().to_ascii_lowercase()),
            Self::EndsWithIgnoreCase(pat) => cand_trimmed
                .to_ascii_lowercase()
                .ends_with(&pat.trim().to_ascii_lowercase()),
        }
    }
}

/// Configurable user or policy rule for including or excluding specific application audio nodes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FilterRule {
    /// Unique human-readable rule identifier.
    pub id: String,
    /// Target field on [`AudioNodeDescriptor`] to inspect.
    pub field: FilterMatchField,
    /// String predicate applied to the target field.
    pub predicate: FilterPredicate,
}

impl FilterRule {
    /// Creates a new [`FilterRule`].
    #[must_use]
    pub fn new(id: impl Into<String>, field: FilterMatchField, predicate: FilterPredicate) -> Self {
        Self {
            id: id.into(),
            field,
            predicate,
        }
    }

    /// Validates both the rule ID and predicate pattern.
    pub fn validate(&self) -> Result<(), AudioFilterError> {
        validate_identifier_token(&self.id)?;
        self.predicate.validate()
    }

    /// Checks whether this rule matches the given [`AudioNodeDescriptor`].
    #[must_use]
    pub fn matches(&self, node: &AudioNodeDescriptor) -> bool {
        match self.field {
            FilterMatchField::ProcessBinary => node
                .process_binary
                .as_deref()
                .is_some_and(|v| self.predicate.matches(v)),
            FilterMatchField::ApplicationName => node
                .application_name
                .as_deref()
                .is_some_and(|v| self.predicate.matches(v)),
            FilterMatchField::ApplicationId => node
                .application_id
                .as_deref()
                .is_some_and(|v| self.predicate.matches(v)),
            FilterMatchField::NodeName => self.predicate.matches(&node.node_name),
            FilterMatchField::ProcessId => node
                .pid
                .map(|p| p.to_string())
                .as_deref()
                .is_some_and(|v| self.predicate.matches(v)),
        }
    }
}

/// Errors returned when configuring or executing the audio node filter service.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AudioFilterError {
    /// Client binary or rule identifier is empty or whitespace-only.
    EmptyClientIdentifier,
    /// An identifier or rule token exceeds [`MAX_IDENTIFIER_LEN`].
    IdentifierTooLong { len: usize, max: usize },
    /// Target virtual sink node ID is invalid (`0`).
    InvalidVirtualSinkId(u32),
}

impl fmt::Display for AudioFilterError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyClientIdentifier => {
                write!(f, "audio filter identifier or rule token must not be empty")
            }
            Self::IdentifierTooLong { len, max } => {
                write!(
                    f,
                    "audio filter identifier length {len} exceeds maximum {max}"
                )
            }
            Self::InvalidVirtualSinkId(id) => {
                write!(f, "invalid target virtual capture sink node id: {id}")
            }
        }
    }
}

impl std::error::Error for AudioFilterError {}

fn validate_identifier_token(raw: &str) -> Result<(), AudioFilterError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Err(AudioFilterError::EmptyClientIdentifier);
    }
    if raw.len() > MAX_IDENTIFIER_LEN {
        return Err(AudioFilterError::IdentifierTooLong {
            len: raw.len(),
            max: MAX_IDENTIFIER_LEN,
        });
    }
    Ok(())
}

/// Configuration for the [`AudioNodeFilterEngine`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioNodeFilterConfig {
    /// Client's own process ID (`std::process::id()`).
    pub own_pid: u32,
    /// Client's own executable binary name (e.g., `"chord"` or `"discord-gpui"`).
    pub own_binary: String,
    /// Name of the virtual capture sink registered by our Capture Service.
    pub virtual_capture_sink_name: String,
    /// Whether to exclude microphone and recording inputs (`true` by default).
    pub exclude_microphones: bool,
    /// Whether to exclude virtual monitoring sinks and loopback nodes (`true` by default).
    pub exclude_virtual_monitors: bool,
    /// Custom exclusion rules evaluated before accept.
    pub exclude_rules: Vec<FilterRule>,
    /// Custom inclusion (allowlist) rules. When non-empty, only nodes matching at least
    /// one include rule (and passing all safety exclusions) are captured.
    pub include_rules: Vec<FilterRule>,
}

impl AudioNodeFilterConfig {
    /// Creates a new verified filter configuration for the given client PID and binary name.
    pub fn new(own_pid: u32, own_binary: impl Into<String>) -> Result<Self, AudioFilterError> {
        let own_binary_str = own_binary.into();
        validate_identifier_token(&own_binary_str)?;
        Ok(Self {
            own_pid,
            own_binary: own_binary_str.trim().to_string(),
            virtual_capture_sink_name: DEFAULT_VIRTUAL_CAPTURE_SINK_NAME.to_string(),
            exclude_microphones: true,
            exclude_virtual_monitors: true,
            exclude_rules: Vec::new(),
            include_rules: Vec::new(),
        })
    }

    /// Sets a custom virtual capture sink name.
    pub fn with_virtual_capture_sink_name(
        mut self,
        sink_name: impl Into<String>,
    ) -> Result<Self, AudioFilterError> {
        let name = sink_name.into();
        validate_identifier_token(&name)?;
        self.virtual_capture_sink_name = name.trim().to_string();
        Ok(self)
    }

    /// Appends or replaces a custom exclusion rule after validating its bounds.
    pub fn add_exclude_rule(&mut self, rule: FilterRule) -> Result<(), AudioFilterError> {
        rule.validate()?;
        self.exclude_rules.retain(|r| r.id != rule.id);
        self.exclude_rules.push(rule);
        Ok(())
    }

    /// Appends or replaces a custom inclusion (allowlist) rule after validating its bounds.
    pub fn add_include_rule(&mut self, rule: FilterRule) -> Result<(), AudioFilterError> {
        rule.validate()?;
        self.include_rules.retain(|r| r.id != rule.id);
        self.include_rules.push(rule);
        Ok(())
    }

    /// Removes a custom rule by ID from both exclude and include lists.
    pub fn remove_rule(&mut self, rule_id: &str) -> bool {
        let prev_ex = self.exclude_rules.len();
        let prev_in = self.include_rules.len();
        self.exclude_rules.retain(|r| r.id != rule_id);
        self.include_rules.retain(|r| r.id != rule_id);
        self.exclude_rules.len() != prev_ex || self.include_rules.len() != prev_in
    }

    /// Clears all custom include and exclude rules, restoring default safety filtering.
    pub fn clear_custom_rules(&mut self) {
        self.exclude_rules.clear();
        self.include_rules.clear();
    }
}

/// Descriptor for a single PipeWire link connecting an accepted application output port
/// to our virtual capture sink (`venmic.ts` `buildLinkData` equivalent).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PipeWireLinkSpec {
    /// Accepted application output node ID (`output.node.id`).
    pub source_node_id: u32,
    /// Target virtual capture sink node ID (`input.node.id`).
    pub target_sink_node_id: u32,
    /// Stereo channel pair linked (`["FL", "FR"]`).
    pub channels: [&'static str; 2],
    /// Whether the link is passive (`node.passive = true`) so it never prevents idle suspend.
    pub passive: bool,
}

/// Telemetry counters recorded during a node graph evaluation pass.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct AudioFilterTelemetry {
    /// Total unique nodes evaluated.
    pub total_evaluated: usize,
    /// Number of application audio nodes accepted for capture.
    pub accepted_count: usize,
    /// Number of nodes excluded because they belong to the client's own audio stream.
    pub excluded_own_client: usize,
    /// Number of nodes excluded because they are microphones or input capture streams.
    pub excluded_microphones: usize,
    /// Number of nodes excluded because they are virtual monitoring or loopback sinks.
    pub excluded_virtual_monitors: usize,
    /// Number of nodes excluded by custom rules, allowlist filtering, or unidentifiable metadata.
    pub excluded_by_rule: usize,
    /// Number of direct or transitive acoustic feedback loops prevented.
    pub feedback_loops_prevented: usize,
}

/// Computed routing plan produced by [`AudioNodeFilterEngine::build_link_data`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AudioLinkPlan {
    /// Node ID of the virtual capture sink receiving application audio.
    pub target_virtual_sink_id: u32,
    /// Ordered list of accepted application node IDs.
    pub accepted_node_ids: Vec<u32>,
    /// PipeWire link specifications connecting each accepted node to `target_virtual_sink_id`.
    pub links: Vec<PipeWireLinkSpec>,
    /// Rejected `(node_id, ExclusionReason)` pairs for diagnostics.
    pub rejected: Vec<(u32, ExclusionReason)>,
    /// Aggregated telemetry counters for this pass.
    pub telemetry: AudioFilterTelemetry,
}

impl AudioLinkPlan {
    /// Verifies that no link in the plan creates a self-loop with `target_virtual_sink_id`
    /// or duplicates a `(source_node_id, target_sink_node_id)` edge.
    #[must_use]
    pub fn verify_no_feedback_loops(&self) -> bool {
        let mut seen = HashSet::with_capacity(self.links.len());
        for link in &self.links {
            if link.source_node_id == 0
                || link.source_node_id == self.target_virtual_sink_id
                || link.target_sink_node_id != self.target_virtual_sink_id
            {
                return false;
            }
            if !seen.insert((link.source_node_id, link.target_sink_node_id)) {
                return false;
            }
        }
        true
    }
}

/// Pure, deterministic audio node filter and link-plan builder for the Capture Service (`R5-02`).
#[derive(Debug, Clone)]
pub struct AudioNodeFilterEngine {
    config: AudioNodeFilterConfig,
}

impl AudioNodeFilterEngine {
    /// Creates a new [`AudioNodeFilterEngine`] with the provided configuration.
    #[must_use]
    pub const fn new(config: AudioNodeFilterConfig) -> Self {
        Self { config }
    }

    /// Returns an immutable reference to the active filter configuration.
    #[must_use]
    pub const fn config(&self) -> &AudioNodeFilterConfig {
        &self.config
    }

    /// Returns a mutable reference to the active filter configuration for dynamic rule updates.
    pub fn config_mut(&mut self) -> &mut AudioNodeFilterConfig {
        &mut self.config
    }

    /// Evaluates a single [`AudioNodeDescriptor`] against mandatory safety rules and custom rules.
    ///
    /// Order of precedence (safety exclusions ALWAYS win over custom include rules):
    /// 1. Metadata validation (`node_id != 0`, non-empty `node_name`, all fields `<= 256` bytes).
    /// 2. Microphone & input capture stream exclusion (`Audio/Source`, `Stream/Input/Audio`).
    /// 3. Virtual monitoring sink, hardware sink, and loopback stream exclusion (`.monitor`, `is_monitor`, `Audio/Sink`, loopback markers).
    /// 4. Client's own audio playback stream exclusion (by PID, binary name, application name, application ID, or node name).
    /// 5. Identifiable application check (`pid != Some(0)` and at least one non-empty application identifier).
    /// 6. Custom exclusion rules (`exclude_rules`).
    /// 7. Custom inclusion allowlist rules (`include_rules`, if non-empty).
    #[must_use]
    pub fn evaluate_node(&self, node: &AudioNodeDescriptor) -> FilterDecision {
        if !Self::has_valid_metadata(node) {
            return FilterDecision::Exclude(ExclusionReason::InvalidMetadata);
        }

        if self.config.exclude_microphones && Self::is_microphone_node(node) {
            return FilterDecision::Exclude(ExclusionReason::MicrophoneInput);
        }

        if self.config.exclude_virtual_monitors && self.is_virtual_or_monitor_sink(node) {
            return FilterDecision::Exclude(ExclusionReason::VirtualMonitoringSink);
        }

        if self.is_own_client_node(node) {
            return FilterDecision::Exclude(ExclusionReason::OwnClientStream);
        }

        if !Self::has_identifiable_application(node) {
            return FilterDecision::Exclude(ExclusionReason::UnidentifiableApplication);
        }

        for rule in &self.config.exclude_rules {
            if rule.matches(node) {
                return FilterDecision::Exclude(ExclusionReason::CustomRule(rule.id.clone()));
            }
        }

        if !self.config.include_rules.is_empty()
            && !self.config.include_rules.iter().any(|r| r.matches(node))
        {
            return FilterDecision::Exclude(ExclusionReason::NotInIncludeAllowlist);
        }

        FilterDecision::Include
    }

    /// Evaluates a snapshot of PipeWire / PulseAudio nodes, prevents direct and transitive
    /// acoustic feedback loops against `target_virtual_sink_id`, deduplicates node IDs, and
    /// builds the PipeWire link plan (`venmic.ts` `buildLinkData` contract).
    pub fn build_link_data(
        &self,
        nodes: &[AudioNodeDescriptor],
        target_virtual_sink_id: u32,
    ) -> Result<AudioLinkPlan, AudioFilterError> {
        if target_virtual_sink_id == 0 {
            return Err(AudioFilterError::InvalidVirtualSinkId(0));
        }
        let mut accepted_node_ids = Vec::new();
        let mut links = Vec::new();
        let mut rejected = Vec::new();
        let mut telemetry = AudioFilterTelemetry::default();
        let mut seen_ids = HashSet::with_capacity(nodes.len());

        for node in nodes {
            if !seen_ids.insert(node.node_id) {
                continue;
            }
            telemetry.total_evaluated += 1;
            if self.would_cause_feedback_loop(node, target_virtual_sink_id) {
                telemetry.feedback_loops_prevented += 1;
                rejected.push((node.node_id, ExclusionReason::AcousticFeedbackLoop));
                continue;
            }
            match self.evaluate_node(node) {
                FilterDecision::Include => {
                    telemetry.accepted_count += 1;
                    accepted_node_ids.push(node.node_id);
                    links.push(PipeWireLinkSpec {
                        source_node_id: node.node_id,
                        target_sink_node_id: target_virtual_sink_id,
                        channels: STEREO_CHANNELS,
                        passive: true,
                    });
                }
                FilterDecision::Exclude(reason) => {
                    Self::record_exclusion_telemetry(&mut telemetry, &reason);
                    rejected.push((node.node_id, reason));
                }
            }
        }
        Ok(AudioLinkPlan {
            target_virtual_sink_id,
            accepted_node_ids,
            links,
            rejected,
            telemetry,
        })
    }

    fn record_exclusion_telemetry(telemetry: &mut AudioFilterTelemetry, reason: &ExclusionReason) {
        match reason {
            ExclusionReason::OwnClientStream => telemetry.excluded_own_client += 1,
            ExclusionReason::MicrophoneInput => telemetry.excluded_microphones += 1,
            ExclusionReason::VirtualMonitoringSink => telemetry.excluded_virtual_monitors += 1,
            ExclusionReason::AcousticFeedbackLoop => telemetry.feedback_loops_prevented += 1,
            ExclusionReason::UnidentifiableApplication
            | ExclusionReason::InvalidMetadata
            | ExclusionReason::CustomRule(_)
            | ExclusionReason::NotInIncludeAllowlist => telemetry.excluded_by_rule += 1,
        }
    }

    fn has_valid_metadata(node: &AudioNodeDescriptor) -> bool {
        if node.node_id == 0
            || node.node_name.trim().is_empty()
            || node.node_name.len() > MAX_IDENTIFIER_LEN
        {
            return false;
        }
        for opt in [
            node.process_binary.as_deref(),
            node.application_name.as_deref(),
            node.application_id.as_deref(),
            node.target_object.as_deref(),
        ]
        .into_iter()
        .flatten()
        {
            if opt.len() > MAX_IDENTIFIER_LEN {
                return false;
            }
        }
        true
    }

    fn is_microphone_node(node: &AudioNodeDescriptor) -> bool {
        let lower_name = node.node_name.to_ascii_lowercase();
        if node.is_monitor || lower_name.ends_with(".monitor") || lower_name.contains(".monitor") {
            return false;
        }
        if node.media_class.is_microphone_or_input() {
            return true;
        }
        lower_name.starts_with("alsa_input.") || lower_name.starts_with("bluez_input.")
    }

    fn is_virtual_or_monitor_sink(&self, node: &AudioNodeDescriptor) -> bool {
        if node.is_monitor || node.media_class.is_sink_or_device() {
            return true;
        }
        let lower_name = node.node_name.to_ascii_lowercase();
        if lower_name.ends_with(".monitor") || lower_name.contains(".monitor") {
            return true;
        }
        if node.is_virtual {
            return true;
        }
        let sink_lower = self.config.virtual_capture_sink_name.to_ascii_lowercase();
        if lower_name == sink_lower {
            return true;
        }
        for marker in BUILTIN_LOOPBACK_MARKERS {
            if lower_name.contains(marker)
                || node
                    .process_binary
                    .as_deref()
                    .is_some_and(|b| b.to_ascii_lowercase().contains(marker))
            {
                return true;
            }
        }
        false
    }

    fn is_own_client_node(&self, node: &AudioNodeDescriptor) -> bool {
        if let Some(pid) = node.pid
            && self.config.own_pid != 0
            && pid == self.config.own_pid
        {
            return true;
        }

        let own_bin_lower = self.config.own_binary.to_ascii_lowercase();
        let candidates = [
            node.process_binary.as_deref(),
            node.application_name.as_deref(),
            node.application_id.as_deref(),
            Some(node.node_name.as_str()),
        ];

        for raw in candidates.into_iter().flatten() {
            let lower = raw.trim().to_ascii_lowercase();
            if lower.is_empty() {
                continue;
            }
            if lower == own_bin_lower {
                return true;
            }
            for builtin in BUILTIN_CLIENT_IDENTIFIERS {
                if lower == builtin || lower.contains(builtin) {
                    return true;
                }
            }
        }
        false
    }

    fn has_identifiable_application(node: &AudioNodeDescriptor) -> bool {
        if node.media_class != AudioNodeMediaClass::StreamOutputAudio {
            return false;
        }
        let Some(pid) = node.pid else {
            return false;
        };
        if pid == 0 {
            return false;
        }
        let has_binary = node
            .process_binary
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty());
        let has_app_name = node
            .application_name
            .as_deref()
            .is_some_and(|s| !s.trim().is_empty());
        has_binary || has_app_name
    }

    fn would_cause_feedback_loop(
        &self,
        node: &AudioNodeDescriptor,
        target_virtual_sink_id: u32,
    ) -> bool {
        if node.node_id == target_virtual_sink_id {
            return true;
        }
        if let Some(target) = node.target_object.as_deref() {
            let trimmed = target.trim();
            if trimmed.eq_ignore_ascii_case(&self.config.virtual_capture_sink_name)
                || trimmed == target_virtual_sink_id.to_string()
                || BUILTIN_LOOPBACK_MARKERS
                    .iter()
                    .any(|m| trimmed.to_ascii_lowercase().contains(m))
            {
                return true;
            }
        }
        false
    }
}
