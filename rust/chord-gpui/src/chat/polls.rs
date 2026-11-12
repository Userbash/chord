//! Интерактивные опросы в сообщениях (`Polls & Voting` — `TASK-1004` / `TASK-1004A` / `TASK-1004B`).
//! Оракулы: `waves/kanban-board.json` (`TASK-1004`), `docs/re/gateway-protocol.json`
//! (`MESSAGE_POLL_VOTE_ADD` = 4), `/home/sanya/re-deep/rest-static.json` (`POLL_ANSWERS`,
//! `POLL_EXPIRE`, `POLL_ANSWER_VOTERS`), `/home/sanya/re-deep/modules-index.json` (`290794`,
//! `412136`, `789646`, `969632`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

pub const POLL_CARD_CONTAINER: &str = "Poll Card";
pub const POLL_CARD_DIMENSIONS: &str = "100% x auto";
pub const POLL_CARD_PADDING_PX: u32 = 12;
pub const POLL_CARD_SPACING_PX: u32 = 8;

pub const TOKEN_BACKGROUND: &str = "#2b2d31";
pub const TOKEN_SURFACE: &str = "#1e1f22";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const TOKEN_PROGRESS_FILL_HEX: &str = "#5865f233";

pub const POLL_OPTION_ELEMENT: &str = "poll-option";
pub const POLL_CREATION_MODAL_KEY: &str = "POLL_CREATION_MODAL_KEY";
pub const RE_MODULE_POLL_CONTEXT: &str = "290794";
pub const RE_MODULE_POLL_CONSTANTS: &str = "412136";
pub const RE_MODULE_POLL_FOCUS: &str = "789646";
pub const RE_MODULE_POLL_ACTIONS: &str = "969632";

pub const MIN_POLL_ANSWERS: usize = 1;
pub const MAX_POLL_ANSWERS: usize = 10;
pub const MAX_POLL_QUESTION_CHARS: usize = 300;
pub const MAX_POLL_ANSWER_CHARS: usize = 55;
pub const MAX_VOTER_PREVIEW_NAMES: usize = 3;
pub const POLL_TITLE_TRUNCATE_CHARS: usize = 50;
pub const MIN_POLL_DURATION_HOURS: u32 = 1;
pub const MAX_POLL_DURATION_HOURS: u32 = 336;
pub const POLL_DURATION_PRESETS_HOURS: [u32; 7] = [1, 4, 8, 24, 72, 168, 336];
pub const POLL_LAYOUT_TYPE_DEFAULT: u8 = 1;

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
pub const PERM_READ_MESSAGE_HISTORY: u64 = 1 << 16;
pub const PERM_SEND_POLLS: u64 = 1 << 49;

pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

const MINUTE_MS: u64 = 60_000;
const HOUR_MS: u64 = 3_600_000;
const DAY_MS: u64 = 86_400_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollOptionInteractiveState {
    Radio,
    HoverBg,
    Voted,
}

impl PollOptionInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Radio => "radio",
            Self::HoverBg => "hover bg",
            Self::Voted => "voted",
        }
    }

    #[must_use]
    pub const fn resolve(voted: bool, hovered: bool) -> Self {
        if voted {
            Self::Voted
        } else if hovered {
            Self::HoverBg
        } else {
            Self::Radio
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollVoteAction {
    Add,
    Remove,
}

impl PollVoteAction {
    #[must_use]
    pub const fn http_method(self) -> &'static str {
        match self {
            PollVoteAction::Add => "PUT",
            PollVoteAction::Remove => "DELETE",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollError {
    InvalidSnowflake(String),
    EmptyQuestion,
    QuestionTooLong(usize),
    InvalidAnswerCount(usize),
    EmptyAnswerText(u32),
    AnswerTextTooLong { answer_id: u32, len: usize },
    InvalidAnswerId(u32),
    DuplicateAnswerId(u32),
    MultiselectNotAllowed,
    PollExpired,
    MissingPermission(&'static str),
    AgeGatedRestricted,
    BoundaryViolation(&'static str),
}

impl fmt::Display for PollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PollError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, PollError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PollError::InvalidSnowflake(raw.to_string()));
    }
    let parsed = trimmed
        .parse::<u64>()
        .map_err(|_| PollError::InvalidSnowflake(raw.to_string()))?;
    if parsed == 0 {
        return Err(PollError::InvalidSnowflake(raw.to_string()));
    }
    Ok(parsed)
}

#[must_use]
pub const fn is_valid_duration_hours(hours: u32) -> bool {
    hours >= MIN_POLL_DURATION_HOURS && hours <= MAX_POLL_DURATION_HOURS
}

#[must_use]
pub const fn is_poll_expired(expiry_ms: Option<u64>, now_ms: u64) -> bool {
    match expiry_ms {
        Some(expiry_ms) => now_ms >= expiry_ms,
        None => true,
    }
}

#[must_use]
pub fn format_poll_expiry_ru(expiry_ms: Option<u64>, now_ms: u64) -> String {
    if is_poll_expired(expiry_ms, now_ms) {
        return "Опрос завершён".to_string();
    }
    let diff = expiry_ms.unwrap_or(now_ms).saturating_sub(now_ms);
    if diff < MINUTE_MS {
        "Осталось меньше минуты".to_string()
    } else if diff < HOUR_MS {
        format!("Осталось {} мин.", diff.div_ceil(MINUTE_MS))
    } else if diff < DAY_MS {
        format!("Осталось {} ч.", diff.div_ceil(HOUR_MS))
    } else {
        format!("Осталось {} дн.", diff.div_ceil(DAY_MS))
    }
}

#[must_use]
pub const fn compute_answer_percentage(count: u32, total_votes: u32) -> u8 {
    if total_votes == 0 {
        return 0;
    }
    ((count as u64 * 100 + (total_votes as u64 / 2)) / total_votes as u64) as u8
}

pub fn toggle_poll_selection(
    selected_ids: &[u32],
    answer_id: u32,
    allow_multiselect: bool,
    max_choices: usize,
) -> Result<Vec<u32>, PollError> {
    if answer_id < MIN_POLL_ANSWERS as u32 || answer_id > MAX_POLL_ANSWERS as u32 {
        return Err(PollError::InvalidAnswerId(answer_id));
    }
    let limit = max_choices.clamp(MIN_POLL_ANSWERS, MAX_POLL_ANSWERS);
    if !allow_multiselect {
        return Ok(if selected_ids.contains(&answer_id) {
            Vec::new()
        } else {
            vec![answer_id]
        });
    }
    if selected_ids.contains(&answer_id) {
        return Ok(selected_ids
            .iter()
            .copied()
            .filter(|id| *id != answer_id)
            .collect());
    }
    let mut next = selected_ids.to_vec();
    if next.len() < limit {
        next.push(answer_id);
    }
    Ok(next)
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollVoteRequest {
    pub method: &'static str,
    pub path: String,
    pub api_v9_path: String,
    pub body_json: Option<String>,
}

impl PollVoteRequest {
    fn new(method: &'static str, path: String, body_json: Option<String>) -> Self {
        Self {
            method,
            api_v9_path: format!("/api/v9{path}"),
            path,
            body_json,
        }
    }
}

pub fn build_poll_answer_vote_request(
    channel_id: &str,
    message_id: &str,
    answer_id: u32,
    action: PollVoteAction,
) -> Result<PollVoteRequest, PollError> {
    validate_snowflake(channel_id)?;
    validate_snowflake(message_id)?;
    if answer_id < MIN_POLL_ANSWERS as u32 || answer_id > MAX_POLL_ANSWERS as u32 {
        return Err(PollError::InvalidAnswerId(answer_id));
    }
    let path = format!("/channels/{channel_id}/polls/{message_id}/answers/{answer_id}");
    Ok(PollVoteRequest::new(action.http_method(), path, None))
}

pub fn build_poll_batch_answers_request(
    channel_id: &str,
    message_id: &str,
    answer_ids: &[u32],
    allow_multiselect: bool,
) -> Result<PollVoteRequest, PollError> {
    validate_snowflake(channel_id)?;
    validate_snowflake(message_id)?;
    if answer_ids.len() > MAX_POLL_ANSWERS {
        return Err(PollError::InvalidAnswerCount(answer_ids.len()));
    }
    if !allow_multiselect && answer_ids.len() > 1 {
        return Err(PollError::MultiselectNotAllowed);
    }
    for &id in answer_ids {
        if id < MIN_POLL_ANSWERS as u32 || id > MAX_POLL_ANSWERS as u32 {
            return Err(PollError::InvalidAnswerId(id));
        }
    }
    let quoted: Vec<String> = answer_ids.iter().map(|id| format!("\"{id}\"")).collect();
    let path = format!("/channels/{channel_id}/polls/{message_id}/answers/@me");
    let body = format!("{{\"answer_ids\":[{}]}}", quoted.join(","));
    Ok(PollVoteRequest::new("PUT", path, Some(body)))
}

pub fn build_poll_expire_request(
    channel_id: &str,
    message_id: &str,
) -> Result<PollVoteRequest, PollError> {
    validate_snowflake(channel_id)?;
    validate_snowflake(message_id)?;
    let path = format!("/channels/{channel_id}/polls/{message_id}/expire");
    Ok(PollVoteRequest::new("POST", path, None))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollAnswerSpec {
    pub answer_id: u32,
    pub text: String,
    pub emoji_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollResultItem {
    pub answer_id: u32,
    pub text: String,
    pub emoji_name: Option<String>,
    pub count: u32,
    pub percentage: u8,
    pub fill_width_percent: u8,
    pub fill_color_hex: &'static str,
    pub me_voted: bool,
    pub is_leading: bool,
    pub is_winner: bool,
    pub interactive_state: PollOptionInteractiveState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollSnapshot {
    pub channel_id: String,
    pub message_id: String,
    pub question: String,
    pub allow_multiselect: bool,
    pub expired: bool,
    pub expiry_text_ru: String,
    pub total_votes: u32,
    pub max_count: u32,
    pub items: Vec<PollResultItem>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GatewayPollVoteEvent {
    pub event_type: String,
    pub guild_id: Option<String>,
    pub channel_id: String,
    pub message_id: String,
    pub user_id: String,
    pub answer_id: u32,
}

impl GatewayPollVoteEvent {
    pub fn new(
        event_type: &str,
        channel_id: &str,
        message_id: &str,
        user_id: &str,
        answer_id: u32,
    ) -> Self {
        Self {
            event_type: event_type.to_string(),
            guild_id: None,
            channel_id: channel_id.to_string(),
            message_id: message_id.to_string(),
            user_id: user_id.to_string(),
            answer_id,
        }
    }
}

fn validate_poll_answers(
    answers: &[PollAnswerSpec],
) -> Result<BTreeMap<u32, BTreeSet<String>>, PollError> {
    if answers.len() < MIN_POLL_ANSWERS || answers.len() > MAX_POLL_ANSWERS {
        return Err(PollError::InvalidAnswerCount(answers.len()));
    }
    let mut voters_by_answer = BTreeMap::new();
    for ans in answers {
        if ans.answer_id < MIN_POLL_ANSWERS as u32 || ans.answer_id > MAX_POLL_ANSWERS as u32 {
            return Err(PollError::InvalidAnswerId(ans.answer_id));
        }
        let text_len = ans.text.trim().chars().count();
        if text_len == 0 {
            return Err(PollError::EmptyAnswerText(ans.answer_id));
        }
        if text_len > MAX_POLL_ANSWER_CHARS {
            return Err(PollError::AnswerTextTooLong {
                answer_id: ans.answer_id,
                len: text_len,
            });
        }
        if voters_by_answer
            .insert(ans.answer_id, BTreeSet::new())
            .is_some()
        {
            return Err(PollError::DuplicateAnswerId(ans.answer_id));
        }
    }
    Ok(voters_by_answer)
}

#[derive(Debug, Clone)]
pub struct InteractivePollState {
    pub channel_id: String,
    pub message_id: String,
    pub current_user_id: String,
    pub question: String,
    pub answers: Vec<PollAnswerSpec>,
    pub allow_multiselect: bool,
    pub expiry_ms: u64,
    pub is_finalized: bool,
    pub voters_by_answer: BTreeMap<u32, BTreeSet<String>>,
    optimistic_backup: Option<Vec<u32>>,
}

impl InteractivePollState {
    pub fn new(
        channel_id: &str,
        message_id: &str,
        current_user_id: &str,
        question: &str,
        answers: Vec<PollAnswerSpec>,
        allow_multiselect: bool,
        expiry_ms: u64,
    ) -> Result<Self, PollError> {
        validate_snowflake(channel_id)?;
        validate_snowflake(message_id)?;
        validate_snowflake(current_user_id)?;
        let q = question.trim();
        if q.is_empty() {
            return Err(PollError::EmptyQuestion);
        }
        if q.chars().count() > MAX_POLL_QUESTION_CHARS {
            return Err(PollError::QuestionTooLong(q.chars().count()));
        }
        let voters_by_answer = validate_poll_answers(&answers)?;
        Ok(Self {
            channel_id: channel_id.to_string(),
            message_id: message_id.to_string(),
            current_user_id: current_user_id.to_string(),
            question: q.to_string(),
            answers,
            allow_multiselect,
            expiry_ms,
            is_finalized: false,
            voters_by_answer,
            optimistic_backup: None,
        })
    }

    pub fn apply_gateway_event(&mut self, ev: &GatewayPollVoteEvent) -> Result<bool, PollError> {
        if ev.channel_id != self.channel_id || ev.message_id != self.message_id {
            return Ok(false);
        }
        validate_snowflake(&ev.user_id)?;
        if !self.voters_by_answer.contains_key(&ev.answer_id) {
            return Err(PollError::InvalidAnswerId(ev.answer_id));
        }
        if ev.event_type == "MESSAGE_POLL_VOTE_ADD" {
            if !self.allow_multiselect {
                for (&aid, set) in &mut self.voters_by_answer {
                    if aid != ev.answer_id {
                        set.remove(&ev.user_id);
                    }
                }
            }
            if let Some(set) = self.voters_by_answer.get_mut(&ev.answer_id) {
                set.insert(ev.user_id.clone());
            }
            return Ok(true);
        }
        if ev.event_type == "MESSAGE_POLL_VOTE_REMOVE" {
            if let Some(set) = self.voters_by_answer.get_mut(&ev.answer_id) {
                set.remove(&ev.user_id);
            }
            return Ok(true);
        }
        Ok(false)
    }

    pub fn cast_vote(&mut self, answer_id: u32, now_ms: u64) -> Result<PollVoteRequest, PollError> {
        if self.is_finalized || is_poll_expired(Some(self.expiry_ms), now_ms) {
            return Err(PollError::PollExpired);
        }
        if !self.voters_by_answer.contains_key(&answer_id) {
            return Err(PollError::InvalidAnswerId(answer_id));
        }
        let prev: Vec<u32> = self
            .voters_by_answer
            .iter()
            .filter_map(|(&id, set)| set.contains(&self.current_user_id).then_some(id))
            .collect();
        self.optimistic_backup = Some(prev.clone());
        let already = prev.contains(&answer_id);
        let action = if already {
            PollVoteAction::Remove
        } else {
            PollVoteAction::Add
        };
        let ev_type = if already {
            "MESSAGE_POLL_VOTE_REMOVE"
        } else {
            "MESSAGE_POLL_VOTE_ADD"
        };
        let ev = GatewayPollVoteEvent::new(
            ev_type,
            &self.channel_id,
            &self.message_id,
            &self.current_user_id,
            answer_id,
        );
        self.apply_gateway_event(&ev)?;
        build_poll_answer_vote_request(&self.channel_id, &self.message_id, answer_id, action)
    }

    pub fn rollback_optimistic_vote(&mut self) -> bool {
        let Some(backup) = self.optimistic_backup.take() else {
            return false;
        };
        for set in self.voters_by_answer.values_mut() {
            set.remove(&self.current_user_id);
        }
        for aid in backup {
            if let Some(set) = self.voters_by_answer.get_mut(&aid) {
                set.insert(self.current_user_id.clone());
            }
        }
        true
    }

    fn build_item(
        &self,
        ans: &PollAnswerSpec,
        total_votes: u32,
        max_count: u32,
        expired: bool,
        hovered_answer_id: Option<u32>,
    ) -> PollResultItem {
        let set = self.voters_by_answer.get(&ans.answer_id);
        let count = set.map_or(0, |s| s.len() as u32);
        let me_voted = set.is_some_and(|s| s.contains(&self.current_user_id));
        let percentage = compute_answer_percentage(count, total_votes);
        let is_leading = total_votes > 0 && count == max_count;
        let hovered = hovered_answer_id == Some(ans.answer_id);
        PollResultItem {
            answer_id: ans.answer_id,
            text: ans.text.clone(),
            emoji_name: ans.emoji_name.clone(),
            count,
            percentage,
            fill_width_percent: percentage,
            fill_color_hex: TOKEN_PROGRESS_FILL_HEX,
            me_voted,
            is_leading,
            is_winner: expired && is_leading,
            interactive_state: PollOptionInteractiveState::resolve(me_voted, hovered),
        }
    }

    #[must_use]
    pub fn snapshot(&self, now_ms: u64, hovered_answer_id: Option<u32>) -> PollSnapshot {
        let expired = self.is_finalized || is_poll_expired(Some(self.expiry_ms), now_ms);
        let mut total_votes: u32 = 0;
        let mut max_count: u32 = 0;
        for ans in &self.answers {
            let c = self
                .voters_by_answer
                .get(&ans.answer_id)
                .map_or(0, |s| s.len() as u32);
            total_votes = total_votes.saturating_add(c);
            max_count = max_count.max(c);
        }
        let items = self
            .answers
            .iter()
            .map(|ans| self.build_item(ans, total_votes, max_count, expired, hovered_answer_id))
            .collect();
        PollSnapshot {
            channel_id: self.channel_id.clone(),
            message_id: self.message_id.clone(),
            question: self.question.clone(),
            allow_multiselect: self.allow_multiselect,
            expired,
            expiry_text_ru: format_poll_expiry_ru(Some(self.expiry_ms), now_ms),
            total_votes,
            max_count,
            items,
        }
    }
}

pub fn check_poll_creation_permissions(
    permissions: u64,
    is_private: bool,
    age_gated_restricted: bool,
) -> Result<(), PollError> {
    if age_gated_restricted {
        return Err(PollError::AgeGatedRestricted);
    }
    if !is_private && (permissions & PERM_VIEW_CHANNEL) == 0 {
        return Err(PollError::MissingPermission("VIEW_CHANNEL"));
    }
    if !is_private && (permissions & PERM_SEND_MESSAGES) == 0 {
        return Err(PollError::MissingPermission("SEND_MESSAGES"));
    }
    if !is_private && (permissions & PERM_SEND_POLLS) == 0 {
        return Err(PollError::MissingPermission("SEND_POLLS"));
    }
    Ok(())
}

pub fn validate_system_boundaries(
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
) -> Result<(), PollError> {
    if message_chars > MAX_MESSAGE_CHARS
        || attachments_count > MAX_ATTACHMENTS_COUNT
        || attachment_bytes > MAX_ATTACHMENT_BYTES
        || search_offset > MAX_SEARCH_OFFSET
        || volume_pct > MAX_VOLUME_PERCENT
    {
        return Err(PollError::BoundaryViolation("system_boundary"));
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
pub fn redact_poll_diagnostic(raw: &str) -> String {
    let mut out = raw.to_string();
    for marker in ["mfa.", "Bearer "] {
        if let Some(pos) = out.find(marker) {
            let start = pos + marker.len();
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

    fn ans(answer_id: u32, text: &str) -> PollAnswerSpec {
        PollAnswerSpec {
            answer_id,
            text: text.to_string(),
            emoji_name: None,
        }
    }

    #[test]
    fn test_sub_1004_1_constants_and_interactive_states() {
        assert_eq!(POLL_CARD_CONTAINER, "Poll Card");
        assert_eq!(POLL_CARD_DIMENSIONS, "100% x auto");
        assert_eq!(POLL_CARD_PADDING_PX, 12);
        assert_eq!(POLL_CARD_SPACING_PX, 8);
        assert_eq!(TOKEN_BACKGROUND, "#2b2d31");
        assert_eq!(TOKEN_SURFACE, "#1e1f22");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(TOKEN_PROGRESS_FILL_HEX, "#5865f233");
        assert_eq!(MIN_POLL_ANSWERS, 1);
        assert_eq!(MAX_POLL_ANSWERS, 10);
        assert_eq!(MAX_POLL_QUESTION_CHARS, 300);
        assert_eq!(MAX_POLL_ANSWER_CHARS, 55);
        assert_eq!(POLL_DURATION_PRESETS_HOURS, [1, 4, 8, 24, 72, 168, 336]);
        assert_eq!(
            PollOptionInteractiveState::resolve(false, false).as_str(),
            "radio"
        );
        assert_eq!(
            PollOptionInteractiveState::resolve(false, true).as_str(),
            "hover bg"
        );
        assert_eq!(
            PollOptionInteractiveState::resolve(true, false).as_str(),
            "voted"
        );
    }

    #[test]
    fn test_sub_1004_2_selection_and_expiry() {
        let sel1 = toggle_poll_selection(&[], 1, false, 10).unwrap_or_default();
        let sel2 = toggle_poll_selection(&sel1, 2, false, 10).unwrap_or_default();
        let sel_clear = toggle_poll_selection(&sel2, 2, false, 10).unwrap_or_default();
        assert_eq!(sel1, vec![1]);
        assert_eq!(sel2, vec![2]);
        assert!(sel_clear.is_empty());
        let m1 = toggle_poll_selection(&[1, 2], 10, true, 10).unwrap_or_default();
        assert_eq!(m1, vec![1, 2, 10]);
        let now = 1_762_000_000_000;
        assert!(is_poll_expired(Some(now), now));
        assert!(!is_poll_expired(Some(now + 1), now));
        assert_eq!(format_poll_expiry_ru(Some(now), now), "Опрос завершён");
        assert_eq!(
            format_poll_expiry_ru(Some(now + 30_000), now),
            "Осталось меньше минуты"
        );
        assert_eq!(
            format_poll_expiry_ru(Some(now + 600_000), now),
            "Осталось 10 мин."
        );
        let ch = "1294815650186788916";
        let msg = "1558237752917758108";
        let put_req = build_poll_answer_vote_request(ch, msg, 8, PollVoteAction::Add).unwrap();
        let del_req = build_poll_answer_vote_request(ch, msg, 8, PollVoteAction::Remove).unwrap();
        assert_eq!(put_req.method, "PUT");
        assert_eq!(del_req.method, "DELETE");
        let batch = build_poll_batch_answers_request(ch, msg, &[1, 7], true).unwrap();
        assert_eq!(
            batch.body_json.as_deref(),
            Some("{\"answer_ids\":[\"1\",\"7\"]}")
        );
        assert_eq!(build_poll_expire_request(ch, msg).unwrap().method, "POST");
    }

    #[test]
    fn test_sub_1004_3_percentages_and_gateway_replay() {
        assert_eq!(compute_answer_percentage(0, 0), 0);
        assert_eq!(compute_answer_percentage(1, 3), 33);
        assert_eq!(compute_answer_percentage(2, 3), 67);
        assert_eq!(compute_answer_percentage(4, 4), 100);
        let ch = "1294815650186788916";
        let msg = "1558237752917758108";
        let answers = vec![ans(1, "ETF"), ans(7, "DAVE"), ans(8, "Polls")];
        let mut poll = InteractivePollState::new(
            ch,
            msg,
            "479659568322969611",
            "Тест?",
            answers,
            false,
            1_800_000_000_000,
        )
        .unwrap();
        for uid in ["606781270382346250", "479659568322969611"] {
            let ev = GatewayPollVoteEvent::new("MESSAGE_POLL_VOTE_ADD", ch, msg, uid, 8);
            assert!(poll.apply_gateway_event(&ev).unwrap());
        }
        let snap = poll.snapshot(1_792_394_000_000, None);
        let item8 = snap.items.iter().find(|i| i.answer_id == 8).unwrap();
        assert_eq!(snap.total_votes, 2);
        assert_eq!(item8.percentage, 100);
        assert!(item8.me_voted && item8.is_leading && !item8.is_winner);
        let exp = poll.snapshot(1_900_000_000_000, None);
        assert!(
            exp.items
                .iter()
                .find(|i| i.answer_id == 8)
                .unwrap()
                .is_winner
        );
    }

    #[test]
    fn test_sub_1004_4_nine_dimensions_and_rollback() {
        assert!(validate_snowflake("0").is_err());
        assert_eq!(
            validate_snowflake("18446744073709551615").unwrap(),
            u64::MAX
        );
        assert_eq!(format_badge_cap(99), "99");
        assert_eq!(format_badge_cap(100), "99+");
        let ten: Vec<PollAnswerSpec> = (1..=10).map(|id| ans(id, "Вариант")).collect();
        let ch = "1294815650186788916";
        let msg = "1558237752917758108";
        let mut poll = InteractivePollState::new(
            ch,
            msg,
            "479659568322969611",
            "Опрос 10?",
            ten,
            false,
            1_800_000_000_000,
        )
        .unwrap();
        assert_eq!(poll.cast_vote(10, 1_700_000_000_000).unwrap().method, "PUT");
        assert_eq!(poll.snapshot(1_700_000_000_000, None).total_votes, 1);
        assert!(poll.rollback_optimistic_vote());
        assert_eq!(poll.snapshot(1_700_000_000_000, None).total_votes, 0);
        assert!(poll.cast_vote(1, 1_900_000_000_000).is_err());
        let base_perms = PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES;
        let full_perms = base_perms | PERM_SEND_POLLS;
        assert!(check_poll_creation_permissions(base_perms, false, false).is_err());
        assert!(check_poll_creation_permissions(full_perms, false, false).is_ok());
        assert!(check_poll_creation_permissions(full_perms, false, true).is_err());
        assert!(validate_system_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200).is_ok());
        assert!(validate_system_boundaries(2_001, 10, 0, 0, 100).is_err());
        assert!(!redact_poll_diagnostic("Bearer mfa.sec999").contains("sec999"));
    }
}
