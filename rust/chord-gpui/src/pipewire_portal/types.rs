//! Public portal session contracts, negotiated parameters, and telemetry.

#![forbid(unsafe_code)]

use std::fmt;
use std::os::fd::RawFd;

use super::{
    OwnedPortalFd, SOURCE_TYPE_ALL_MASK, SOURCE_TYPE_MONITOR, SOURCE_TYPE_WINDOW,
    validate_portal_token,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PortalError {
    InvalidToken(String),
    InvalidObjectPath(String),
    InvalidSourceTypes(u32),
    InvalidSessionState(&'static str, PortalSessionState),
    NoStreamsReturned,
    InvalidFd(RawFd),
    TooManyFds(usize),
    EmptyPayloadWithFds,
    ControlDataTruncated,
    MalformedAncillaryData,
    InvalidDimensions(u32, u32),
    InvalidStride(u32, u32),
    NoCompatibleDmaBufModifier,
    PipeWireDaemonDisconnected(String),
    FrameBridge(String),
    IoError(String),
}

impl fmt::Display for PortalError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PortalError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortalSessionState {
    Unconnected,
    SessionCreated,
    SourcesSelected,
    Streaming,
    ReconnectingDaemon,
    Closed,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalSessionConfig {
    pub handle_token: String,
    pub session_handle_token: String,
    pub source_types: u32,
    pub restore_token: Option<String>,
}

impl PortalSessionConfig {
    #[must_use]
    pub fn default_monitor_and_window() -> Self {
        Self {
            handle_token: "discord_gpui_req_971".to_string(),
            session_handle_token: "discord_gpui_sess_971".to_string(),
            source_types: SOURCE_TYPE_MONITOR | SOURCE_TYPE_WINDOW,
            restore_token: None,
        }
    }

    pub fn validate(&self) -> Result<(), PortalError> {
        validate_portal_token(&self.handle_token)?;
        validate_portal_token(&self.session_handle_token)?;
        if self.source_types == 0 || (self.source_types & !SOURCE_TYPE_ALL_MASK) != 0 {
            return Err(PortalError::InvalidSourceTypes(self.source_types));
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DmaBufNegotiationRequest {
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub drm_fourcc: u32,
    pub candidate_stride: u32,
    pub advertised_modifiers: Vec<u64>,
    pub allow_shm_fallback: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiatedDmaBufConfig {
    pub drm_fourcc: u32,
    pub drm_modifier: u64,
    pub stride_bytes: u32,
    pub is_zero_copy: bool,
}

pub trait PortalBackend {
    fn create_session(&mut self, req: &str, sess: &str) -> Result<String, PortalError>;
    fn select_sources(&mut self, path: &str, cfg: &PortalSessionConfig) -> Result<(), PortalError>;
    fn start(&mut self, path: &str, req: &str) -> Result<(u32, Option<String>), PortalError>;
    fn open_pipewire_remote(&mut self, path: &str) -> Result<Vec<OwnedPortalFd>, PortalError>;
    fn close_session(&mut self, path: &str) -> Result<(), PortalError>;
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DaemonRestartReport {
    pub closed_old_fd: Option<RawFd>,
    pub new_fd: RawFd,
    pub node_id: u32,
    pub session_recreated: bool,
    pub used_restore_token: bool,
    pub negotiated: NegotiatedDmaBufConfig,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PortalTelemetrySnapshot {
    pub state: PortalSessionState,
    pub active_node_id: Option<u32>,
    pub active_fd: Option<RawFd>,
    pub redacted_restore_token: String,
    pub total_fds_opened: u64,
    pub total_fds_closed: u64,
}

impl PortalTelemetrySnapshot {
    #[must_use]
    pub const fn active_fd_balance(&self) -> u64 {
        self.total_fds_opened.saturating_sub(self.total_fds_closed)
    }
}
