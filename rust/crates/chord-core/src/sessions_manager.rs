//! Active session manager and remote revocation (`TASK-1009B`).
//!
//! Oracle: `GET /users/@me/auth/sessions`, `DELETE /users/@me/auth/sessions` (`docs/re/rest-d02.md`).

use serde::{Deserialize, Serialize};
use std::fmt;

/// Green badge color for the active current device session.
pub const CURRENT_DEVICE_BADGE_COLOR: &str = "#23a55a";
/// Badge label for the active current device session.
pub const CURRENT_DEVICE_BADGE_LABEL: &str = "Current Device";

/// Metadata for an active Discord authentication session.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    pub id_hash: String,
    pub os: String,
    pub browser: String,
    pub ip: String,
    pub location: String,
    pub approx_last_used_time: String,
    pub is_current: bool,
}

impl SessionInfo {
    /// Returns `"Current Device"` if this session is the current device.
    pub fn badge_label(&self) -> Option<&'static str> {
        if self.is_current {
            Some(CURRENT_DEVICE_BADGE_LABEL)
        } else {
            None
        }
    }

    /// Returns the green badge hex color if this session is the current device.
    pub fn badge_color(&self) -> Option<&'static str> {
        if self.is_current {
            Some(CURRENT_DEVICE_BADGE_COLOR)
        } else {
            None
        }
    }
}

/// Errors produced by [`SessionManager`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionError {
    /// Session ID hash is empty or whitespace.
    InvalidSessionId,
    /// Caller is not authorized to revoke the target session.
    Unauthorized,
    /// Target session ID was not found in the active session list.
    NotFound,
}

impl fmt::Display for SessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidSessionId => write!(f, "session id_hash must not be empty"),
            Self::Unauthorized => write!(f, "unauthorized to revoke session"),
            Self::NotFound => write!(f, "session not found"),
        }
    }
}

impl std::error::Error for SessionError {}

/// HTTP DELETE request descriptor for remote session revocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevokeRequest {
    pub method: &'static str,
    pub endpoint: &'static str,
    pub session_id_hash: String,
}

/// Manages active sessions and remote revocation.
#[derive(Debug, Clone, Default)]
pub struct SessionManager {
    sessions: Vec<SessionInfo>,
}

impl SessionManager {
    /// Creates a new [`SessionManager`] from queried sessions.
    pub fn new(sessions: Vec<SessionInfo>) -> Self {
        Self { sessions }
    }

    /// Returns all active sessions.
    pub fn sessions(&self) -> &[SessionInfo] {
        &self.sessions
    }

    /// Returns the current device session, if present.
    pub fn current_session(&self) -> Option<&SessionInfo> {
        self.sessions.iter().find(|s| s.is_current)
    }

    /// Revokes a session by `id_hash`, returning the `DELETE` request descriptor
    /// and removing it from the local list.
    pub fn revoke_session(
        &mut self,
        id_hash: &str,
        authorized: bool,
    ) -> Result<RevokeRequest, SessionError> {
        if id_hash.trim().is_empty() {
            return Err(SessionError::InvalidSessionId);
        }
        if !authorized {
            return Err(SessionError::Unauthorized);
        }

        let pos = self
            .sessions
            .iter()
            .position(|s| s.id_hash == id_hash)
            .ok_or(SessionError::NotFound)?;

        self.sessions.remove(pos);

        Ok(RevokeRequest {
            method: "DELETE",
            endpoint: "/users/@me/auth/sessions",
            session_id_hash: id_hash.to_string(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sessions() -> Vec<SessionInfo> {
        vec![
            SessionInfo {
                id_hash: "sess_current_1".to_string(),
                os: "Linux".to_string(),
                browser: "Chord Desktop".to_string(),
                ip: "192.0.2.10".to_string(),
                location: "Berlin, DE".to_string(),
                approx_last_used_time: "2026-11-21T12:00:00Z".to_string(),
                is_current: true,
            },
            SessionInfo {
                id_hash: "sess_other_2".to_string(),
                os: "Android".to_string(),
                browser: "Discord Mobile".to_string(),
                ip: "198.51.100.42".to_string(),
                location: "Munich, DE".to_string(),
                approx_last_used_time: "2026-11-20T18:30:00Z".to_string(),
                is_current: false,
            },
        ]
    }

    #[test]
    fn queries_sessions_and_highlights_current_device_badge() {
        let mgr = SessionManager::new(sample_sessions());
        assert_eq!(mgr.sessions().len(), 2);

        let current = mgr.current_session().expect("current session must exist");
        assert_eq!(current.id_hash, "sess_current_1");
        assert_eq!(current.badge_label(), Some("Current Device"));
        assert_eq!(current.badge_color(), Some("#23a55a"));

        let other = &mgr.sessions()[1];
        assert_eq!(other.badge_label(), None);
        assert_eq!(other.badge_color(), None);
    }

    #[test]
    fn revoke_session_dispatches_delete_and_removes_from_list() {
        let mut mgr = SessionManager::new(sample_sessions());
        let req = mgr
            .revoke_session("sess_other_2", true)
            .expect("authorized revoke should succeed");

        assert_eq!(req.method, "DELETE");
        assert_eq!(req.endpoint, "/users/@me/auth/sessions");
        assert_eq!(req.session_id_hash, "sess_other_2");
        assert_eq!(mgr.sessions().len(), 1);
        assert!(mgr.current_session().is_some());
    }

    #[test]
    fn rejects_unauthorized_revocation() {
        let mut mgr = SessionManager::new(sample_sessions());
        let err = mgr
            .revoke_session("sess_other_2", false)
            .expect_err("unauthorized revoke must fail");
        assert_eq!(err, SessionError::Unauthorized);
        assert_eq!(mgr.sessions().len(), 2);
    }

    #[test]
    fn rejects_empty_session_id_and_missing_session() {
        let mut mgr = SessionManager::new(sample_sessions());
        assert_eq!(
            mgr.revoke_session("   ", true).unwrap_err(),
            SessionError::InvalidSessionId
        );
        assert_eq!(
            mgr.revoke_session("nonexistent", true).unwrap_err(),
            SessionError::NotFound
        );
    }
}
