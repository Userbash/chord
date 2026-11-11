#![forbid(unsafe_code)]
#![allow(dead_code)]
//! UI-карточка опроса `PollCardView` с анимированными шкалами результатов и бейджем победителя (`TASK-1004B`).
//! Оракулы: `waves/kanban-board.json` (`TASK-1004B`, `TASK-1004`), `/home/sanya/re-deep/frontend-d03.json`
//! (`button_a22cb0`, `active_a22cb0`, `numberBadge__463b7`, `iconBadge__463b7`, `badge_e7c728`, `BaseRadioGroup`,
//! `--switch-background-selected-default`, `tooltip_c36707`, `avatar_fdd9da`, `avatarStack__44b0c`, `"опрос"`),
//! `/home/sanya/re-deep/modules-index.json` (`290794`, `355682` `end-poll-early`, `412136`, `789646`,
//! `803931` `pollContainer_b7e1cb`, `969632` `poll_question_text`).

use std::fmt;

pub const POLL_CARD_CONTAINER: &str = "Poll Card";
pub const POLL_CARD_DIMENSIONS: &str = "100% x auto";
pub const POLL_CARD_PADDING_PX: u32 = 12;
pub const POLL_CARD_SPACING_PX: u32 = 8;
pub const POLL_OPTION_ROW_HEIGHT_PX: u32 = 44;
pub const MIN_CARD_WIDTH_PX: u32 = 240;
pub const MAX_CARD_WIDTH_PX: u32 = 1_200;

pub const TOKEN_BACKGROUND: &str = "#2b2d31";
pub const TOKEN_SURFACE: &str = "#1e1f22";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const TOKEN_PROGRESS_FILL_HEX: &str = "#5865f233";
pub const TOKEN_WINNER_BADGE_HEX: &str = "#23a55a";

pub const OPTION_ROW_ELEMENT: &str = "option-row";
pub const OPTION_ROW_STATE_DEFAULT: &str = "idle";
pub const OPTION_ROW_STATE_HOVER: &str = "hover-bg";
pub const OPTION_ROW_STATE_ACTIVE: &str = "selected";
pub const POLL_OPTION_ELEMENT: &str = "poll-option";

pub const D03_SEARCH_CONTAINS_POLL_RU: &str = "опрос";
pub const D03_BUTTON_CLASS: &str = "button_a22cb0";
pub const D03_BUTTON_ACTIVE_CLASS: &str = "active_a22cb0";
pub const D03_NUMBER_BADGE_CLASS: &str = "numberBadge__463b7";
pub const D03_ICON_BADGE_CLASS: &str = "iconBadge__463b7";
pub const D03_TAG_BADGE_CLASS: &str = "badge_e7c728";
pub const D03_TOOLTIP_CLASS: &str = "tooltip_c36707";
pub const D03_AVATAR_CLASS: &str = "avatar_fdd9da";
pub const D03_AVATAR_STACK_CLASS: &str = "avatarStack__44b0c";
pub const D03_RADIO_GROUP_COMPONENT: &str = "BaseRadioGroup";
pub const D03_SWITCH_SELECTED_BG_TOKEN: &str = "--switch-background-selected-default";
pub const RE_POLL_CONTAINER_CLASS: &str = "pollContainer_b7e1cb";
pub const RE_END_POLL_EARLY_ACTION: &str = "end-poll-early";
pub const RE_POLL_QUESTION_TEXT_KEY: &str = "poll_question_text";

pub const MIN_POLL_ANSWERS: usize = 1;
pub const MAX_POLL_ANSWERS: usize = 10;
pub const MAX_POLL_QUESTION_CHARS: usize = 300;
pub const MAX_POLL_ANSWER_CHARS: usize = 55;
pub const MAX_VOTER_PREVIEW_NAMES: usize = 3;
pub const DEFAULT_ANIMATION_DURATION_MS: u32 = 240;

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
pub enum OptionRowInteractiveState {
    Idle,
    HoverBg,
    Selected,
}

impl OptionRowInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Idle => "idle",
            Self::HoverBg => "hover-bg",
            Self::Selected => "selected",
        }
    }

    #[must_use]
    pub const fn resolve(selected: bool, hovered: bool) -> Self {
        if selected {
            Self::Selected
        } else if hovered {
            Self::HoverBg
        } else {
            Self::Idle
        }
    }
}

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
pub enum WinnerBadgeKind {
    None,
    LeadingActive,
    SoleWinner,
    TiedWinner,
}

impl WinnerBadgeKind {
    #[must_use]
    pub const fn label_ru(self) -> Option<&'static str> {
        match self {
            Self::None => None,
            Self::LeadingActive => Some("Лидирует"),
            Self::SoleWinner => Some("Победитель"),
            Self::TiedWinner => Some("Ничья (победитель)"),
        }
    }

    #[must_use]
    pub const fn is_winner(self) -> bool {
        matches!(self, Self::SoleWinner | Self::TiedWinner)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollViewError {
    InvalidSnowflake(String),
    EmptyQuestion,
    QuestionTooLong(usize),
    InvalidAnswerCount(usize),
    InvalidAnswerId(u32),
    DuplicateAnswerId(u32),
    EmptyAnswerText(u32),
    AnswerTextTooLong { answer_id: u32, len: usize },
    MissingPermission(&'static str),
    AgeGatedRestricted,
    BoundaryViolation(&'static str),
    HttpError { status: u16, retry_after_ms: u64 },
}

impl fmt::Display for PollViewError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PollViewError {}

pub fn validate_snowflake(raw: &str) -> Result<u64, PollViewError> {
    let trimmed = raw.trim();
    if trimmed.is_empty() || !trimmed.bytes().all(|b| b.is_ascii_digit()) {
        return Err(PollViewError::InvalidSnowflake(raw.to_string()));
    }
    let parsed = trimmed
        .parse::<u64>()
        .map_err(|_| PollViewError::InvalidSnowflake(raw.to_string()))?;
    if parsed == 0 {
        return Err(PollViewError::InvalidSnowflake(raw.to_string()));
    }
    Ok(parsed)
}

#[must_use]
pub const fn compute_answer_percentage(count: u32, total_votes: u32) -> u8 {
    if total_votes == 0 {
        return 0;
    }
    ((count as u64 * 100 + (total_votes as u64 / 2)) / total_votes as u64) as u8
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
pub const fn is_poll_expired(expiry_ms: Option<u64>, now_ms: u64) -> bool {
    match expiry_ms {
        Some(exp) => now_ms >= exp,
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

#[derive(Debug, Clone, PartialEq)]
pub struct ProgressAnimationSpec {
    pub from_pct: f32,
    pub to_pct: f32,
    pub current_pct: f32,
    pub current_width_px: f32,
    pub fill_color_hex: &'static str,
    pub completed: bool,
}

#[must_use]
pub fn ease_out_cubic(t: f32) -> f32 {
    let inv = 1.0 - t.clamp(0.0, 1.0);
    1.0 - (inv * inv * inv)
}

#[must_use]
pub fn interpolate_progress_bar(
    from_pct: f32,
    to_pct: f32,
    elapsed_ms: u32,
    duration_ms: u32,
    track_width_px: u32,
) -> ProgressAnimationSpec {
    let from_clamped = from_pct.clamp(0.0, 100.0);
    let to_clamped = to_pct.clamp(0.0, 100.0);
    let safe_duration = duration_ms.max(1);
    let completed = elapsed_ms >= safe_duration;
    let ratio = if completed {
        1.0
    } else {
        elapsed_ms as f32 / safe_duration as f32
    };
    let eased = ease_out_cubic(ratio);
    let current_pct = from_clamped + (to_clamped - from_clamped) * eased;
    let current_width_px = (current_pct / 100.0) * (track_width_px as f32);
    ProgressAnimationSpec {
        from_pct: from_clamped,
        to_pct: to_clamped,
        current_pct,
        current_width_px,
        fill_color_hex: TOKEN_PROGRESS_FILL_HEX,
        completed,
    }
}

#[must_use]
pub fn resolve_winner_badge(
    count: u32,
    total_votes: u32,
    max_votes: u32,
    expired: bool,
    leader_count: usize,
) -> WinnerBadgeKind {
    if total_votes == 0 || max_votes == 0 || count < max_votes {
        return WinnerBadgeKind::None;
    }
    if !expired {
        return WinnerBadgeKind::LeadingActive;
    }
    if leader_count > 1 {
        WinnerBadgeKind::TiedWinner
    } else {
        WinnerBadgeKind::SoleWinner
    }
}

#[must_use]
pub fn format_voter_tooltip_ru(voters: &[String], total_count: u32) -> String {
    if total_count == 0 || voters.is_empty() {
        return "Пока никто не проголосовал".to_string();
    }
    let preview: Vec<&str> = voters
        .iter()
        .take(MAX_VOTER_PREVIEW_NAMES)
        .map(String::as_str)
        .collect();
    let rest = total_count.saturating_sub(preview.len() as u32);
    if rest > 0 {
        format!("{} и ещё {}", preview.join(", "), rest)
    } else {
        preview.join(", ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollAnswerViewInput {
    pub answer_id: u32,
    pub text: String,
    pub emoji_name: Option<String>,
    pub vote_count: u32,
    pub me_voted: bool,
    pub recent_voters: Vec<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PollOptionRowViewSpec {
    pub answer_id: u32,
    pub text: String,
    pub vote_count: u32,
    pub vote_badge_text: String,
    pub percentage: u8,
    pub progress: ProgressAnimationSpec,
    pub row_state: OptionRowInteractiveState,
    pub option_state: PollOptionInteractiveState,
    pub border_color_hex: &'static str,
    pub me_voted: bool,
    pub is_leading: bool,
    pub is_winner: bool,
    pub winner_badge: WinnerBadgeKind,
    pub winner_badge_label_ru: Option<&'static str>,
    pub voter_tooltip_ru: String,
    pub aria_role: &'static str,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PollCardViewSpec {
    pub container: &'static str,
    pub container_class: &'static str,
    pub dimensions: &'static str,
    pub card_width_px: u32,
    pub padding_px: u32,
    pub spacing_px: u32,
    pub channel_id: String,
    pub message_id: String,
    pub question: String,
    pub allow_multiselect: bool,
    pub expired: bool,
    pub expiry_label_ru: String,
    pub total_votes: u32,
    pub total_votes_badge: String,
    pub max_votes: u32,
    pub winner_answer_ids: Vec<u32>,
    pub rows: Vec<PollOptionRowViewSpec>,
    pub submit_button_class: &'static str,
    pub can_vote: bool,
    pub skeleton_loading: bool,
}

#[derive(Debug, Clone)]
pub struct PollCardBuildParams {
    pub channel_id: String,
    pub message_id: String,
    pub question: String,
    pub answers: Vec<PollAnswerViewInput>,
    pub allow_multiselect: bool,
    pub expiry_ms: Option<u64>,
    pub now_ms: u64,
    pub is_finalized: bool,
    pub hovered_answer_id: Option<u32>,
    pub anim_elapsed_ms: u32,
    pub card_width_px: u32,
    pub can_vote: bool,
    pub skeleton_loading: bool,
}

fn validate_answer_inputs(answers: &[PollAnswerViewInput]) -> Result<(), PollViewError> {
    if answers.len() < MIN_POLL_ANSWERS || answers.len() > MAX_POLL_ANSWERS {
        return Err(PollViewError::InvalidAnswerCount(answers.len()));
    }
    let mut seen_ids = Vec::with_capacity(answers.len());
    for ans in answers {
        if ans.answer_id < MIN_POLL_ANSWERS as u32 || ans.answer_id > MAX_POLL_ANSWERS as u32 {
            return Err(PollViewError::InvalidAnswerId(ans.answer_id));
        }
        if seen_ids.contains(&ans.answer_id) {
            return Err(PollViewError::DuplicateAnswerId(ans.answer_id));
        }
        seen_ids.push(ans.answer_id);
        let text_len = ans.text.trim().chars().count();
        if text_len == 0 {
            return Err(PollViewError::EmptyAnswerText(ans.answer_id));
        }
        if text_len > MAX_POLL_ANSWER_CHARS {
            return Err(PollViewError::AnswerTextTooLong {
                answer_id: ans.answer_id,
                len: text_len,
            });
        }
    }
    Ok(())
}

struct RowBuildCtx {
    total_votes: u32,
    max_votes: u32,
    expired: bool,
    leader_count: usize,
    allow_multiselect: bool,
    hovered_answer_id: Option<u32>,
    anim_elapsed_ms: u32,
    track_width_px: u32,
}

fn resolve_row_border_color(is_winner: bool, me_voted: bool, hovered: bool) -> &'static str {
    if is_winner {
        TOKEN_WINNER_BADGE_HEX
    } else if me_voted || hovered {
        TOKEN_ACCENT
    } else {
        TOKEN_BORDER
    }
}

fn build_option_row(ans: &PollAnswerViewInput, ctx: &RowBuildCtx) -> PollOptionRowViewSpec {
    let pct = compute_answer_percentage(ans.vote_count, ctx.total_votes);
    let hovered = ctx.hovered_answer_id == Some(ans.answer_id);
    let winner_badge = resolve_winner_badge(
        ans.vote_count,
        ctx.total_votes,
        ctx.max_votes,
        ctx.expired,
        ctx.leader_count,
    );
    let is_leading = ctx.total_votes > 0 && ctx.max_votes > 0 && ans.vote_count == ctx.max_votes;
    let is_winner = winner_badge.is_winner();
    let progress = interpolate_progress_bar(
        0.0,
        f32::from(pct),
        ctx.anim_elapsed_ms,
        DEFAULT_ANIMATION_DURATION_MS,
        ctx.track_width_px,
    );
    PollOptionRowViewSpec {
        answer_id: ans.answer_id,
        text: ans.text.trim().to_string(),
        vote_count: ans.vote_count,
        vote_badge_text: format_badge_cap(ans.vote_count),
        percentage: pct,
        progress,
        row_state: OptionRowInteractiveState::resolve(ans.me_voted, hovered),
        option_state: PollOptionInteractiveState::resolve(ans.me_voted, hovered),
        border_color_hex: resolve_row_border_color(is_winner, ans.me_voted, hovered),
        me_voted: ans.me_voted,
        is_leading,
        is_winner,
        winner_badge,
        winner_badge_label_ru: winner_badge.label_ru(),
        voter_tooltip_ru: format_voter_tooltip_ru(&ans.recent_voters, ans.vote_count),
        aria_role: if ctx.allow_multiselect {
            "checkbox"
        } else {
            "radio"
        },
    }
}

fn validate_card_params(params: &PollCardBuildParams) -> Result<String, PollViewError> {
    validate_snowflake(&params.channel_id)?;
    validate_snowflake(&params.message_id)?;
    let q = params.question.trim();
    if q.is_empty() {
        return Err(PollViewError::EmptyQuestion);
    }
    let q_len = q.chars().count();
    if q_len > MAX_POLL_QUESTION_CHARS {
        return Err(PollViewError::QuestionTooLong(q_len));
    }
    validate_answer_inputs(&params.answers)?;
    Ok(q.to_string())
}

fn compute_vote_aggregates(answers: &[PollAnswerViewInput]) -> (u32, u32, usize) {
    let total: u32 = answers
        .iter()
        .fold(0_u32, |acc, a| acc.saturating_add(a.vote_count));
    let max_v: u32 = answers.iter().map(|a| a.vote_count).max().unwrap_or(0);
    let leaders = if total > 0 && max_v > 0 {
        answers.iter().filter(|a| a.vote_count == max_v).count()
    } else {
        0
    };
    (total, max_v, leaders)
}

fn build_card_rows(
    params: &PollCardBuildParams,
    agg: (u32, u32, usize),
    expired: bool,
    card_width_px: u32,
) -> Vec<PollOptionRowViewSpec> {
    let ctx = RowBuildCtx {
        total_votes: agg.0,
        max_votes: agg.1,
        expired,
        leader_count: agg.2,
        allow_multiselect: params.allow_multiselect,
        hovered_answer_id: params.hovered_answer_id,
        anim_elapsed_ms: params.anim_elapsed_ms,
        track_width_px: card_width_px.saturating_sub(POLL_CARD_PADDING_PX * 2),
    };
    params
        .answers
        .iter()
        .map(|a| build_option_row(a, &ctx))
        .collect()
}

pub fn build_poll_card_view(
    params: &PollCardBuildParams,
) -> Result<PollCardViewSpec, PollViewError> {
    let question = validate_card_params(params)?;
    let expired = params.is_finalized || is_poll_expired(params.expiry_ms, params.now_ms);
    let agg = compute_vote_aggregates(&params.answers);
    let card_width_px = params
        .card_width_px
        .clamp(MIN_CARD_WIDTH_PX, MAX_CARD_WIDTH_PX);
    let rows = build_card_rows(params, agg, expired, card_width_px);
    let winner_answer_ids = rows
        .iter()
        .filter(|r| r.is_winner)
        .map(|r| r.answer_id)
        .collect();
    Ok(PollCardViewSpec {
        container: POLL_CARD_CONTAINER,
        container_class: RE_POLL_CONTAINER_CLASS,
        dimensions: POLL_CARD_DIMENSIONS,
        card_width_px,
        padding_px: POLL_CARD_PADDING_PX,
        spacing_px: POLL_CARD_SPACING_PX,
        channel_id: params.channel_id.clone(),
        message_id: params.message_id.clone(),
        question,
        allow_multiselect: params.allow_multiselect,
        expired,
        expiry_label_ru: format_poll_expiry_ru(params.expiry_ms, params.now_ms),
        total_votes: agg.0,
        total_votes_badge: format_badge_cap(agg.0),
        max_votes: agg.1,
        winner_answer_ids,
        rows,
        submit_button_class: D03_BUTTON_CLASS,
        can_vote: params.can_vote && !expired,
        skeleton_loading: params.skeleton_loading,
    })
}

#[must_use]
pub fn navigate_poll_options_keyboard(
    answer_ids: &[u32],
    current_focus: Option<u32>,
    key: &str,
) -> Option<u32> {
    if answer_ids.is_empty() {
        return None;
    }
    let cur_idx = current_focus
        .and_then(|id| answer_ids.iter().position(|&x| x == id))
        .unwrap_or(0);
    let next_idx = match key {
        "ArrowDown" | "ArrowRight" => (cur_idx + 1) % answer_ids.len(),
        "ArrowUp" | "ArrowLeft" => (cur_idx + answer_ids.len() - 1) % answer_ids.len(),
        "Home" => 0,
        "End" => answer_ids.len() - 1,
        _ => cur_idx,
    };
    answer_ids.get(next_idx).copied()
}

pub fn classify_poll_http_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<&'static str, PollViewError> {
    match status {
        200 | 204 => Ok("ok"),
        202 => Ok("skeleton_warming"),
        status => Err(PollViewError::HttpError {
            status,
            retry_after_ms,
        }),
    }
}

pub fn check_poll_view_permissions(
    permissions: u64,
    is_private: bool,
    age_gated_restricted: bool,
) -> Result<bool, PollViewError> {
    if age_gated_restricted {
        return Err(PollViewError::AgeGatedRestricted);
    }
    if !is_private && (permissions & PERM_VIEW_CHANNEL) == 0 {
        return Err(PollViewError::MissingPermission("VIEW_CHANNEL"));
    }
    if !is_private && (permissions & PERM_SEND_MESSAGES) == 0 {
        return Err(PollViewError::MissingPermission("SEND_MESSAGES"));
    }
    if !is_private && (permissions & PERM_SEND_POLLS) == 0 {
        return Err(PollViewError::MissingPermission("SEND_POLLS"));
    }
    Ok(true)
}

pub fn validate_poll_view_boundaries(
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_pct: u16,
) -> Result<(), PollViewError> {
    if message_chars > MAX_MESSAGE_CHARS
        || attachments_count > MAX_ATTACHMENTS_COUNT
        || attachment_bytes > MAX_ATTACHMENT_BYTES
        || search_offset > MAX_SEARCH_OFFSET
        || volume_pct > MAX_VOLUME_PERCENT
    {
        return Err(PollViewError::BoundaryViolation("system_boundary"));
    }
    Ok(())
}

#[must_use]
pub fn redact_poll_view_secrets(raw: &str) -> String {
    let mut out = raw.to_string();
    for marker in ["mfa.", "Bearer ", "dQw4w9WgXcQ:"] {
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

    fn sample_answer(id: u32, text: &str, count: u32, voted: bool) -> PollAnswerViewInput {
        PollAnswerViewInput {
            answer_id: id,
            text: text.to_string(),
            emoji_name: Some("🔥".to_string()),
            vote_count: count,
            me_voted: voted,
            recent_voters: vec!["alice".to_string(), "bob".to_string()],
        }
    }

    fn base_params(answers: Vec<PollAnswerViewInput>, finalized: bool) -> PollCardBuildParams {
        let exp = if finalized {
            1_700_000_000_000
        } else {
            1_800_000_000_000
        };
        PollCardBuildParams {
            channel_id: "1294815650186788916".to_string(),
            message_id: "1558237752917758108".to_string(),
            question: "Какой рендерер быстрее?".to_string(),
            answers,
            allow_multiselect: false,
            expiry_ms: Some(exp),
            now_ms: 1_762_000_000_000,
            is_finalized: finalized,
            hovered_answer_id: Some(2),
            anim_elapsed_ms: 240,
            card_width_px: 424,
            can_vote: true,
            skeleton_loading: false,
        }
    }

    #[test]
    fn test_sub_1004b_1_tokens_layout_and_interactive_states() {
        assert!(POLL_CARD_CONTAINER == "Poll Card" && POLL_CARD_DIMENSIONS == "100% x auto");
        assert!(POLL_CARD_PADDING_PX == 12 && POLL_CARD_SPACING_PX == 8);
        assert!(TOKEN_BACKGROUND == "#2b2d31" && TOKEN_SURFACE == "#1e1f22");
        assert!(TOKEN_TEXT_PRIMARY == "#ffffff" && TOKEN_ACCENT == "#5865f2");
        assert!(TOKEN_BORDER == "#3f4147" && TOKEN_PROGRESS_FILL_HEX == "#5865f233");
        assert!(OPTION_ROW_ELEMENT == "option-row" && D03_SEARCH_CONTAINS_POLL_RU == "опрос");
        assert!(OptionRowInteractiveState::resolve(false, false).as_str() == "idle");
        assert!(OptionRowInteractiveState::resolve(false, true).as_str() == "hover-bg");
        assert!(OptionRowInteractiveState::resolve(true, false).as_str() == "selected");
        assert!(PollOptionInteractiveState::resolve(false, false).as_str() == "radio");
        assert!(PollOptionInteractiveState::resolve(false, true).as_str() == "hover bg");
        assert!(PollOptionInteractiveState::resolve(true, false).as_str() == "voted");
    }

    #[test]
    fn test_sub_1004b_2_card_view_and_progress_animation() {
        let half = interpolate_progress_bar(0.0, 80.0, 120, 240, 400);
        assert!(!half.completed && half.current_pct > 60.0 && half.current_pct < 80.0);
        assert_eq!(half.fill_color_hex, "#5865f233");

        let done = interpolate_progress_bar(0.0, 60.0, 240, 240, 400);
        assert!(done.completed && (done.current_pct - 60.0).abs() < 1e-4);
        assert!((done.current_width_px - 240.0).abs() < 1e-3);

        let answers = vec![
            sample_answer(1, "Rust + GPUI 120 FPS", 6, true),
            sample_answer(2, "Electron WebView", 1, false),
            sample_answer(3, "Tauri Hybrid", 3, false),
        ];
        let card = build_poll_card_view(&base_params(answers, false)).unwrap();
        assert!(card.total_votes == 10 && card.rows[0].percentage == 60);
        assert!(card.rows[0].row_state.as_str() == "selected");
        assert!(card.rows[1].row_state.as_str() == "hover-bg");
        assert!(card.rows[2].row_state.as_str() == "idle");
        assert!(card.rows[0].progress.fill_color_hex == "#5865f233");
        assert!((card.rows[0].progress.current_width_px - 240.0).abs() < 1e-3);
        assert!(card.winner_answer_ids.is_empty());
    }

    #[test]
    fn test_sub_1004b_3_winner_badge_and_keyboard_navigation() {
        assert!(resolve_winner_badge(5, 10, 5, false, 1) == WinnerBadgeKind::LeadingActive);
        assert!(resolve_winner_badge(5, 10, 5, true, 1) == WinnerBadgeKind::SoleWinner);
        assert!(resolve_winner_badge(5, 10, 5, true, 2) == WinnerBadgeKind::TiedWinner);
        assert!(resolve_winner_badge(0, 0, 0, true, 0) == WinnerBadgeKind::None);

        let answers = vec![
            sample_answer(1, "GPUI", 8, true),
            sample_answer(2, "Web", 2, false),
        ];
        let card = build_poll_card_view(&base_params(answers, true)).unwrap();
        assert!(card.expired && !card.can_vote && card.rows[0].is_winner);
        assert_eq!(card.winner_answer_ids, vec![1]);
        assert_eq!(card.rows[0].winner_badge_label_ru, Some("Победитель"));
        assert_eq!(card.rows[0].border_color_hex, TOKEN_WINNER_BADGE_HEX);

        let voters = vec!["u1".into(), "u2".into(), "u3".into(), "u4".into()];
        assert_eq!(format_voter_tooltip_ru(&voters, 5), "u1, u2, u3 и ещё 2");
        let ids = [1, 2, 3];
        assert!(navigate_poll_options_keyboard(&ids, Some(1), "ArrowDown") == Some(2));
        assert!(navigate_poll_options_keyboard(&ids, Some(1), "ArrowUp") == Some(3));
        assert!(navigate_poll_options_keyboard(&ids, Some(2), "Home") == Some(1));
        assert!(navigate_poll_options_keyboard(&ids, Some(2), "End") == Some(3));
    }

    #[test]
    fn test_sub_1004b_4_nine_dimensions_and_security() {
        assert!(validate_snowflake("0").is_err());
        assert!(validate_snowflake("18446744073709551615").unwrap() == u64::MAX);
        assert!(compute_answer_percentage(0, 0) == 0);
        assert!(format_badge_cap(99) == "99" && format_badge_cap(125) == "99+");
        assert!(classify_poll_http_status(200, 0).unwrap() == "ok");
        assert!(classify_poll_http_status(202, 0).unwrap() == "skeleton_warming");
        assert!(classify_poll_http_status(429, 1_500).is_err());
        assert!(classify_poll_http_status(503, 0).is_err());

        let base_perms = PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES;
        let full_perms = base_perms | PERM_SEND_POLLS;
        assert!(check_poll_view_permissions(base_perms, false, false).is_err());
        assert!(check_poll_view_permissions(full_perms, false, false).unwrap());
        assert!(check_poll_view_permissions(full_perms, false, true).is_err());

        assert!(validate_poll_view_boundaries(2_000, 10, 25 * 1024 * 1024, 9_975, 200).is_ok());
        assert!(validate_poll_view_boundaries(2_001, 10, 0, 0, 100).is_err());
        assert!(validate_poll_view_boundaries(100, 11, 0, 0, 100).is_err());
        assert!(validate_poll_view_boundaries(100, 1, 25 * 1024 * 1024 + 1, 0, 100).is_err());
        assert!(validate_poll_view_boundaries(100, 1, 0, 9_976, 100).is_err());
        assert!(validate_poll_view_boundaries(100, 1, 0, 0, 201).is_err());

        let redacted = redact_poll_view_secrets("Authorization: Bearer mfa.secret_poll_999");
        assert!(!redacted.contains("secret_poll_999") && redacted.contains("[REDACTED]"));
    }
}
