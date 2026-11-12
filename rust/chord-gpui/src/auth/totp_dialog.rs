//! Двухфакторная аутентификация TOTP 2FA и управление активными сессиями (`TASK-1009`).
//!
//! Оракулы:
//! - `waves/kanban-board.json` (`TASK-1009`, `TASK-1009A`, `TASK-1009B`): `TOTP Modal` (`400px x 280px`,
//!   `padding: 20px`, `spacing: 12px`), `Sessions Manager` (`Settings Tab`, `padding: 16px`, `spacing: 12px`).
//! - `docs/re/rest-d02.json`, `/home/sanya/re-deep/rest-static.json` и `/home/sanya/re-deep/decoded/36a171b84d1c0225_0`:
//!   `POST /auth/mfa/{arg}`, `GET /auth/sessions` (17 живых сессий), `DELETE /auth/sessions/{id}`, `POST /auth/sessions/logout`.

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::fmt;

pub const TOTP_MODAL_CONTAINER: &str = "TOTP Modal";
pub const TOTP_MODAL_DIMENSIONS: &str = "400px x 280px";
pub const TOTP_MODAL_WIDTH_PX: u32 = 400;
pub const TOTP_MODAL_HEIGHT_PX: u32 = 280;
pub const TOTP_MODAL_PADDING_PX: u32 = 20;
pub const TOTP_MODAL_SPACING_PX: u32 = 12;

pub const SESSIONS_MANAGER_CONTAINER: &str = "Sessions Manager";
pub const SESSIONS_MANAGER_DIMENSIONS: &str = "Settings Tab";
pub const SESSIONS_MANAGER_PADDING_PX: u32 = 16;
pub const SESSIONS_MANAGER_SPACING_PX: u32 = 12;

pub const TOKEN_BACKGROUND: &str = "#313338";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_DIGIT_FIELD_BG: &str = "#1e1f22";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_DANGER_ACCENT: &str = "#f23f43";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const DIGIT_BOX_ELEMENT: &str = "digit-box";
pub const DIGIT_FIELD_ELEMENT: &str = "digit-field";
pub const REVOKE_SESSION_ELEMENT: &str = "revoke-session";
pub const CURRENT_DEVICE_BADGE_RU: &str = "Текущее устройство";

pub const TOTP_DIGIT_COUNT: usize = 6;
pub const BACKUP_CODE_MIN_LEN: usize = 8;
pub const BACKUP_CODE_MAX_LEN: usize = 11;
pub const MFA_INVALID_CODE_ERROR: u32 = 60_008;
pub const MFA_REQUIRED_ERROR: u32 = 60_003;
pub const MFA_METHODS_WHITELIST: [&str; 5] = ["totp", "sms", "backup", "webauthn", "password"];

pub const RE_MODULE_AUTH_SESSIONS_ACTIONS: &str = "200921";
pub const RE_MODULE_MFA_MODAL: &str = "572469";
pub const RE_MODULE_MFA_FINISH: &str = "533925";
pub const RE_MODULE_MFA_STORE: &str = "670492";
pub const RE_MODULE_AUTH_SESSIONS_STORE: &str = "825542";

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

const HOUR_MS: u64 = 3_600_000;
const DAY_MS: u64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DigitBoxState {
    Border,
    BorderAccent,
    FocusedDigit,
}

impl DigitBoxState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Border => "border",
            Self::BorderAccent => "border-accent",
            Self::FocusedDigit => "focused digit",
        }
    }

    #[must_use]
    pub const fn resolve(focused: bool, hovered: bool) -> Self {
        match (focused, hovered) {
            (true, _) => Self::FocusedDigit,
            (false, true) => Self::BorderAccent,
            (false, false) => Self::Border,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RevokeSessionState {
    Button,
    RedHover,
    SessionDeleted,
}

impl RevokeSessionState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Button => "button",
            Self::RedHover => "red hover",
            Self::SessionDeleted => "session deleted",
        }
    }

    #[must_use]
    pub const fn resolve(deleted: bool, hovered: bool) -> Self {
        match (deleted, hovered) {
            (true, _) => Self::SessionDeleted,
            (false, true) => Self::RedHover,
            (false, false) => Self::Button,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TotpSessionError {
    InvalidSnowflake(String),
    InvalidDigitChar(char),
    InvalidBoxIndex(usize),
    IncompleteTotpCode(usize),
    InvalidBackupCodeLength(usize),
    InvalidMfaMethod(String),
    EmptyTicket,
    MfaRejected { code: u32, message: &'static str },
    EmptySessionIdHash,
    CannotRevokeCurrentSession,
    SessionNotFound(String),
    SessionManagerLoading,
    MfaRequiredForModeration,
    AgeGatedRestricted,
    MessageTooLong(usize),
    TooManyAttachments(usize),
    AttachmentTooLarge(u64),
    SearchOffsetOutOfBounds(u32),
    VolumeOutOfBounds(u16),
    HttpFailure { status: u16, retry_after_ms: u64 },
}

impl fmt::Display for TotpSessionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for TotpSessionError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, TotpSessionError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(TotpSessionError::InvalidSnowflake(raw.to_string()));
    }
    let parsed = trimmed
        .parse::<u64>()
        .map_err(|_| TotpSessionError::InvalidSnowflake(raw.to_string()))?;
    if parsed == 0 {
        return Err(TotpSessionError::InvalidSnowflake(raw.to_string()));
    }
    Ok(parsed)
}

#[derive(Clone, PartialEq, Eq)]
pub struct TotpDialogState {
    boxes: [Option<char>; TOTP_DIGIT_COUNT],
    pub focused_index: usize,
    pub submitting: bool,
    pub error_message: Option<String>,
}

impl Default for TotpDialogState {
    fn default() -> Self {
        Self::new()
    }
}

impl fmt::Debug for TotpDialogState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TotpDialogState")
            .field("filled_count", &self.filled_count())
            .field("focused_index", &self.focused_index)
            .field("submitting", &self.submitting)
            .field("code", &"[REDACTED]")
            .finish()
    }
}

impl TotpDialogState {
    #[must_use]
    pub const fn new() -> Self {
        Self {
            boxes: [None; TOTP_DIGIT_COUNT],
            focused_index: 0,
            submitting: false,
            error_message: None,
        }
    }

    #[must_use]
    pub fn filled_count(&self) -> usize {
        self.boxes.iter().filter(|b| b.is_some()).count()
    }

    #[must_use]
    pub fn is_complete(&self) -> bool {
        self.filled_count() == TOTP_DIGIT_COUNT && !self.submitting
    }

    #[must_use]
    pub fn code(&self) -> String {
        self.boxes.iter().flatten().copied().collect()
    }

    #[must_use]
    pub const fn digit_at(&self, index: usize) -> Option<char> {
        if index < TOTP_DIGIT_COUNT {
            self.boxes[index]
        } else {
            None
        }
    }

    pub fn input_digit(&mut self, ch: char) -> Result<usize, TotpSessionError> {
        if self.submitting {
            return Err(TotpSessionError::SessionManagerLoading);
        }
        if !ch.is_ascii_digit() {
            return Err(TotpSessionError::InvalidDigitChar(ch));
        }
        self.boxes[self.focused_index] = Some(ch);
        self.error_message = None;
        if self.focused_index + 1 < TOTP_DIGIT_COUNT {
            self.focused_index += 1;
        }
        Ok(self.focused_index)
    }

    pub fn handle_backspace(&mut self) -> usize {
        if self.submitting {
            return self.focused_index;
        }
        self.error_message = None;
        if self.boxes[self.focused_index].is_some() {
            self.boxes[self.focused_index] = None;
        } else if self.focused_index > 0 {
            self.focused_index -= 1;
            self.boxes[self.focused_index] = None;
        }
        self.focused_index
    }

    pub fn paste_code(&mut self, clipboard: &str) -> usize {
        if self.submitting {
            return 0;
        }
        let digits: Vec<char> = clipboard.chars().filter(char::is_ascii_digit).collect();
        if digits.is_empty() {
            return 0;
        }
        self.boxes = [None; TOTP_DIGIT_COUNT];
        let count = digits.len().min(TOTP_DIGIT_COUNT);
        for (idx, ch) in digits.into_iter().take(TOTP_DIGIT_COUNT).enumerate() {
            self.boxes[idx] = Some(ch);
        }
        self.error_message = None;
        self.focused_index = if count >= TOTP_DIGIT_COUNT {
            TOTP_DIGIT_COUNT - 1
        } else {
            count
        };
        count
    }

    pub fn clear(&mut self) {
        self.boxes = [None; TOTP_DIGIT_COUNT];
        self.focused_index = 0;
        self.submitting = false;
        self.error_message = None;
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct TotpMfaRequest {
    pub http_method: &'static str,
    pub path: String,
    pub mfa_type: String,
    pub code: String,
    pub ticket: String,
    pub login_instance_id: Option<String>,
}

impl fmt::Debug for TotpMfaRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("TotpMfaRequest")
            .field("http_method", &self.http_method)
            .field("path", &self.path)
            .field("mfa_type", &self.mfa_type)
            .field("code", &"[REDACTED]")
            .field("ticket", &"[REDACTED]")
            .finish()
    }
}

impl TotpMfaRequest {
    pub fn build(
        method: &str,
        raw_code: &str,
        ticket: &str,
        login_instance_id: Option<&str>,
    ) -> Result<Self, TotpSessionError> {
        let method_trimmed = method.trim();
        if !MFA_METHODS_WHITELIST.contains(&method_trimmed) {
            return Err(TotpSessionError::InvalidMfaMethod(
                method_trimmed.to_string(),
            ));
        }
        let ticket_trimmed = ticket.trim();
        if ticket_trimmed.is_empty() {
            return Err(TotpSessionError::EmptyTicket);
        }
        let normalized_code = if method_trimmed == "backup" {
            let clean: String = raw_code.trim().chars().filter(|c| *c != '-').collect();
            if clean.len() < BACKUP_CODE_MIN_LEN
                || clean.len() > BACKUP_CODE_MAX_LEN
                || !clean.chars().all(|c| c.is_ascii_alphanumeric())
            {
                return Err(TotpSessionError::InvalidBackupCodeLength(clean.len()));
            }
            clean
        } else {
            let trimmed_code = raw_code.trim();
            if trimmed_code.len() != TOTP_DIGIT_COUNT
                || !trimmed_code.bytes().all(|b| b.is_ascii_digit())
            {
                return Err(TotpSessionError::IncompleteTotpCode(trimmed_code.len()));
            }
            trimmed_code.to_string()
        };
        Ok(Self {
            http_method: "POST",
            path: format!("/auth/mfa/{method_trimmed}"),
            mfa_type: method_trimmed.to_string(),
            code: normalized_code,
            ticket: ticket_trimmed.to_string(),
            login_instance_id: login_instance_id
                .map(str::trim)
                .filter(|s| !s.is_empty())
                .map(ToOwned::to_owned),
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSessionClientInfo {
    pub os: String,
    pub platform: String,
    pub location: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSessionItem {
    pub id_hash: String,
    pub approx_last_used_ms: u64,
    pub client_info: AuthSessionClientInfo,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthSessionViewItem {
    pub id_hash: String,
    pub is_current: bool,
    pub badge_ru: Option<&'static str>,
    pub os_label: String,
    pub platform_label: String,
    pub location_label: String,
    pub last_active_ru: String,
    pub revoke_state: RevokeSessionState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SessionHttpRequest {
    pub method: &'static str,
    pub path: String,
    pub session_id_hashes: Vec<String>,
}

#[must_use]
pub fn build_fetch_sessions_request() -> SessionHttpRequest {
    SessionHttpRequest {
        method: "GET",
        path: "/auth/sessions".to_string(),
        session_id_hashes: Vec::new(),
    }
}

pub fn build_delete_session_request(id_hash: &str) -> Result<SessionHttpRequest, TotpSessionError> {
    let trimmed = id_hash.trim();
    if trimmed.is_empty() || trimmed.contains('/') && trimmed.starts_with("..") {
        return Err(TotpSessionError::EmptySessionIdHash);
    }
    Ok(SessionHttpRequest {
        method: "DELETE",
        path: format!("/auth/sessions/{trimmed}"),
        session_id_hashes: vec![trimmed.to_string()],
    })
}

pub fn build_logout_sessions_request(
    hashes: &[String],
) -> Result<SessionHttpRequest, TotpSessionError> {
    let cleaned: Vec<String> = hashes
        .iter()
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .collect();
    if cleaned.is_empty() {
        return Err(TotpSessionError::EmptySessionIdHash);
    }
    Ok(SessionHttpRequest {
        method: "POST",
        path: "/auth/sessions/logout".to_string(),
        session_id_hashes: cleaned,
    })
}

#[must_use]
pub fn format_session_last_active_ru(last_used_ms: u64, now_ms: u64) -> String {
    let diff = now_ms.saturating_sub(last_used_ms);
    if diff < HOUR_MS {
        "Менее часа назад".to_string()
    } else if diff < DAY_MS {
        format!("{} ч. назад", diff / HOUR_MS)
    } else {
        format!("{} дн. назад", diff / DAY_MS)
    }
}

#[derive(Debug, Clone)]
pub struct AuthSessionsManager {
    current_session_id_hash: String,
    sessions: Vec<AuthSessionItem>,
    rollback_snapshot: Option<Vec<AuthSessionItem>>,
    pub loading: bool,
}

impl AuthSessionsManager {
    #[must_use]
    pub fn new(current_session_id_hash: &str, sessions: Vec<AuthSessionItem>) -> Self {
        Self {
            current_session_id_hash: current_session_id_hash.trim().to_string(),
            sessions,
            rollback_snapshot: None,
            loading: false,
        }
    }

    #[must_use]
    pub fn current_session(&self) -> Option<&AuthSessionItem> {
        self.sessions
            .iter()
            .find(|s| s.id_hash == self.current_session_id_hash)
    }

    #[must_use]
    pub fn other_sessions_sorted(&self) -> Vec<AuthSessionItem> {
        let mut others: Vec<AuthSessionItem> = self
            .sessions
            .iter()
            .filter(|s| s.id_hash != self.current_session_id_hash)
            .cloned()
            .collect();
        others.sort_by(|a, b| b.approx_last_used_ms.cmp(&a.approx_last_used_ms));
        others
    }

    #[must_use]
    pub fn format_view_item(&self, item: &AuthSessionItem, now_ms: u64) -> AuthSessionViewItem {
        let is_current = item.id_hash == self.current_session_id_hash;
        let os_trimmed = item.client_info.os.trim();
        let os_label = if os_trimmed.is_empty() {
            "Неизвестная ОС"
        } else {
            os_trimmed
        };
        let last_active_ru = if is_current {
            "Активна сейчас".to_string()
        } else {
            format_session_last_active_ru(item.approx_last_used_ms, now_ms)
        };
        AuthSessionViewItem {
            id_hash: item.id_hash.clone(),
            is_current,
            badge_ru: is_current.then_some(CURRENT_DEVICE_BADGE_RU),
            os_label: os_label.to_string(),
            platform_label: item.client_info.platform.trim().to_string(),
            location_label: item.client_info.location.trim().to_string(),
            last_active_ru,
            revoke_state: RevokeSessionState::Button,
        }
    }

    pub fn revoke_session(
        &mut self,
        id_hash: &str,
    ) -> Result<SessionHttpRequest, TotpSessionError> {
        if self.loading {
            return Err(TotpSessionError::SessionManagerLoading);
        }
        let trimmed = id_hash.trim();
        if trimmed.is_empty() {
            return Err(TotpSessionError::EmptySessionIdHash);
        }
        if trimmed == self.current_session_id_hash {
            return Err(TotpSessionError::CannotRevokeCurrentSession);
        }
        let idx = self
            .sessions
            .iter()
            .position(|s| s.id_hash == trimmed)
            .ok_or_else(|| TotpSessionError::SessionNotFound(trimmed.to_string()))?;
        self.rollback_snapshot = Some(self.sessions.clone());
        self.sessions.remove(idx);
        build_delete_session_request(trimmed)
    }

    pub fn revoke_all_other_sessions(&mut self) -> Result<SessionHttpRequest, TotpSessionError> {
        if self.loading {
            return Err(TotpSessionError::SessionManagerLoading);
        }
        let hashes: Vec<String> = self
            .other_sessions_sorted()
            .into_iter()
            .map(|s| s.id_hash)
            .collect();
        let req = build_logout_sessions_request(&hashes)?;
        self.rollback_snapshot = Some(self.sessions.clone());
        self.sessions
            .retain(|s| s.id_hash == self.current_session_id_hash);
        Ok(req)
    }

    pub fn rollback_last_revoke(&mut self) -> bool {
        self.rollback_snapshot
            .take()
            .map(|prev| self.sessions = prev)
            .is_some()
    }
}

pub fn check_boundary_limits(
    msg_len: usize,
    att_cnt: usize,
    att_bytes: u64,
    offset: u32,
    vol: u16,
) -> Result<(), TotpSessionError> {
    match () {
        () if msg_len > MAX_MESSAGE_CHARS => Err(TotpSessionError::MessageTooLong(msg_len)),
        () if att_cnt > MAX_ATTACHMENTS_COUNT => Err(TotpSessionError::TooManyAttachments(att_cnt)),
        () if att_bytes > MAX_ATTACHMENT_BYTES => {
            Err(TotpSessionError::AttachmentTooLarge(att_bytes))
        }
        () if offset > MAX_SEARCH_OFFSET => Err(TotpSessionError::SearchOffsetOutOfBounds(offset)),
        () if vol > MAX_VOLUME_PERCENT => Err(TotpSessionError::VolumeOutOfBounds(vol)),
        () => Ok(()),
    }
}

#[must_use]
pub fn format_badge_cap(count: u32) -> String {
    if count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        count.to_string()
    }
}

pub fn map_mfa_or_http_error(
    status: u16,
    discord_code: Option<u32>,
    retry_after_ms: u64,
) -> Result<(), TotpSessionError> {
    match discord_code {
        Some(MFA_INVALID_CODE_ERROR) => Err(TotpSessionError::MfaRejected {
            code: MFA_INVALID_CODE_ERROR,
            message: "Неверный двухфакторный код",
        }),
        Some(MFA_REQUIRED_ERROR) => Err(TotpSessionError::MfaRejected {
            code: MFA_REQUIRED_ERROR,
            message: "Для этого действия требуется 2FA",
        }),
        _ if (200..300).contains(&status) => Ok(()),
        _ => Err(TotpSessionError::HttpFailure {
            status,
            retry_after_ms: match status {
                429 => retry_after_ms.max(100),
                500.. => 1_000,
                _ => 0,
            },
        }),
    }
}

pub fn check_moderation_mfa_and_age_gate(
    guild_mfa: bool,
    user_mfa: bool,
    age_gated: bool,
) -> Result<(), TotpSessionError> {
    match (age_gated, guild_mfa && !user_mfa) {
        (true, _) => Err(TotpSessionError::AgeGatedRestricted),
        (false, true) => Err(TotpSessionError::MfaRequiredForModeration),
        (false, false) => Ok(()),
    }
}

#[must_use]
pub fn redact_totp_telemetry(raw: &str) -> String {
    let mut out = raw.to_string();
    for key in ["code=", "ticket=", "token=", "Authorization="] {
        if let Some(pos) = out.find(key) {
            let start = pos + key.len();
            let end = out[start..].find(' ').unwrap_or(out.len() - start);
            out.replace_range(start..start + end, "\"[REDACTED]\"");
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_sessions() -> Vec<AuthSessionItem> {
        [
            (
                "DxuLb6GvCUXGAZzsjeGM0n4SpqkVaPO2KLgqoZj+Nc4=",
                1_790_858_395_809,
            ),
            (
                "OdKq9gNLX69NrpWTtUFE3MH2d2evCrU0o5eLIUvFP4s=",
                1_792_326_798_747,
            ),
            (
                "SIYXCYXQnxanTF0V1iOSleP9FEADrhLFZZAiuLEeS7Y=",
                1_792_325_510_439,
            ),
        ]
        .into_iter()
        .map(|(h, ts)| AuthSessionItem {
            id_hash: h.to_string(),
            approx_last_used_ms: ts,
            client_info: AuthSessionClientInfo {
                os: "Linux".to_string(),
                platform: "Discord Client".to_string(),
                location: "Riga, Riga, Latvia".to_string(),
            },
        })
        .collect()
    }

    #[test]
    fn test_sub_1009_1_oracle_layout_and_tokens() {
        assert_eq!(TOTP_MODAL_CONTAINER, "TOTP Modal");
        assert_eq!(TOTP_MODAL_DIMENSIONS, "400px x 280px");
        assert_eq!((TOTP_MODAL_PADDING_PX, TOTP_MODAL_SPACING_PX), (20, 12));
        assert_eq!((TOKEN_BACKGROUND, TOKEN_ACCENT), ("#313338", "#5865f2"));
        assert_eq!(TOTP_DIGIT_COUNT, 6);
        assert_eq!(DigitBoxState::resolve(false, false).as_str(), "border");
        assert_eq!(
            DigitBoxState::resolve(false, true),
            DigitBoxState::BorderAccent
        );
        assert_eq!(
            DigitBoxState::resolve(true, false),
            DigitBoxState::FocusedDigit
        );
        assert_eq!(
            RevokeSessionState::resolve(false, false),
            RevokeSessionState::Button
        );
        assert_eq!(
            RevokeSessionState::resolve(false, true),
            RevokeSessionState::RedHover
        );
        assert_eq!(
            RevokeSessionState::resolve(true, false),
            RevokeSessionState::SessionDeleted
        );
    }

    #[test]
    fn test_sub_1009_2_totp_autofocus_backspace_paste_and_mfa_request() {
        let mut dialog = TotpDialogState::new();
        assert_eq!(dialog.focused_index, 0);
        assert!(dialog.input_digit('x').is_err());
        for (idx, ch) in ['4', '8', '2', '9', '1', '0'].into_iter().enumerate() {
            assert_eq!(dialog.input_digit(ch).unwrap_or(99), (idx + 1).min(5));
        }
        assert!(dialog.is_complete());
        assert_eq!(dialog.code(), "482910");
        assert!(!format!("{dialog:?}").contains("482910"));

        assert_eq!(dialog.handle_backspace(), 5);
        assert_eq!(dialog.digit_at(5), None);
        assert_eq!(dialog.handle_backspace(), 4);
        assert_eq!(dialog.code(), "4829");

        dialog.clear();
        assert_eq!(dialog.paste_code(" 739-204 "), 6);
        assert_eq!(dialog.code(), "739204");
        assert_eq!(dialog.focused_index, 5);

        let totp_req = TotpMfaRequest::build("totp", &dialog.code(), "ticket-1", Some("iid-1"))
            .unwrap_or_else(|e| panic!("totp err: {e}"));
        assert_eq!(totp_req.http_method, "POST");
        assert_eq!(totp_req.path, "/auth/mfa/totp");
        assert_eq!(totp_req.code, "739204");
        assert!(!format!("{totp_req:?}").contains("739204"));

        let backup_req = TotpMfaRequest::build("backup", "abcd-ef12", "ticket-1", None)
            .unwrap_or_else(|e| panic!("backup err: {e}"));
        assert_eq!(backup_req.path, "/auth/mfa/backup");
        assert_eq!(backup_req.code, "abcdef12");
        assert!(TotpMfaRequest::build("../logout", "739204", "ticket-1", None).is_err());
    }

    #[test]
    fn test_sub_1009_3_sessions_manager_get_and_delete() {
        let fetch_req = build_fetch_sessions_request();
        assert_eq!(fetch_req.method, "GET");
        assert_eq!(fetch_req.path, "/auth/sessions");

        let current_hash = "OdKq9gNLX69NrpWTtUFE3MH2d2evCrU0o5eLIUvFP4s=";
        let mut mgr = AuthSessionsManager::new(current_hash, sample_sessions());
        let cur = mgr
            .current_session()
            .unwrap_or_else(|| panic!("no current"));
        let cur_view = mgr.format_view_item(cur, 1_792_327_000_000);
        assert!(cur_view.is_current);
        assert_eq!(cur_view.badge_ru, Some("Текущее устройство"));

        let others = mgr.other_sessions_sorted();
        assert_eq!(others.len(), 2);
        assert_eq!(
            others[0].id_hash,
            "SIYXCYXQnxanTF0V1iOSleP9FEADrhLFZZAiuLEeS7Y="
        );
        assert_eq!(
            mgr.revoke_session(current_hash),
            Err(TotpSessionError::CannotRevokeCurrentSession)
        );

        let target = "DxuLb6GvCUXGAZzsjeGM0n4SpqkVaPO2KLgqoZj+Nc4=";
        let del_req = mgr
            .revoke_session(target)
            .unwrap_or_else(|e| panic!("err: {e}"));
        assert_eq!(del_req.method, "DELETE");
        assert_eq!(del_req.path, format!("/auth/sessions/{target}"));
        assert_eq!(mgr.other_sessions_sorted().len(), 1);

        assert!(mgr.rollback_last_revoke());
        assert_eq!(mgr.other_sessions_sorted().len(), 2);

        let batch_req = mgr
            .revoke_all_other_sessions()
            .unwrap_or_else(|e| panic!("err: {e}"));
        assert_eq!(batch_req.method, "POST");
        assert_eq!(batch_req.path, "/auth/sessions/logout");
        assert_eq!(batch_req.session_id_hashes.len(), 2);
        assert_eq!(mgr.other_sessions_sorted().len(), 0);
    }

    #[test]
    fn test_sub_1009_4_nine_dimensions_and_observability() {
        assert!(validate_snowflake("0").is_err());
        assert_eq!(validate_snowflake("18446744073709551615"), Ok(u64::MAX));
        assert!(check_boundary_limits(2_000, 10, 25 * 1024 * 1024, 9_975, 200).is_ok());
        assert!(check_boundary_limits(2_001, 10, 0, 0, 100).is_err());
        assert_eq!(format_badge_cap(99), "99");
        assert_eq!(format_badge_cap(100), "99+");
        assert!(map_mfa_or_http_error(401, Some(60_008), 0).is_err());
        assert!(map_mfa_or_http_error(429, None, 1_500).is_err());
        assert!(check_moderation_mfa_and_age_gate(true, true, false).is_ok());
        assert!(check_moderation_mfa_and_age_gate(true, false, false).is_err());
        assert!(check_moderation_mfa_and_age_gate(false, true, true).is_err());

        let redacted = redact_totp_telemetry(
            "POST /auth/mfa/totp code=482910 ticket=secret_ticket_1 token=mfa.secret_99",
        );
        assert!(!redacted.contains("482910"));
        assert!(!redacted.contains("secret_ticket_1"));
        assert!(!redacted.contains("mfa.secret_99"));
    }
}
