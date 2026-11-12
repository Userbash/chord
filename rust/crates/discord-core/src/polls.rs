//! Модель данных интерактивного опроса `Poll`, десериализация JSON-конверта,
//! REST-эндпоинты голосования (`PUT /channels/{id}/polls/{msg_id}/answers/{answer_id}`,
//! `PUT /polls/{id}/answers/{id}`) и математика процентных шкал (`TASK-1004A`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fmt;

pub const POLL_ENGINE_CONTAINER: &str = "Poll Engine";
pub const POLL_ENGINE_DIMENSIONS: &str = "Backend Module";
pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const REST_POLL_ANSWER_VOTE_TEMPLATE: &str =
    "/channels/{id}/polls/{msg_id}/answers/{answer_id}";
pub const REST_POLL_SHORT_VOTE_TEMPLATE: &str = "/polls/{id}/answers/{id}";
pub const REST_POLL_ANSWERS_ME_TEMPLATE: &str = "/channels/{id}/polls/{msg_id}/answers/@me";
pub const REST_POLL_EXPIRE_TEMPLATE: &str = "/channels/{id}/polls/{msg_id}/expire";
pub const MIN_POLL_ANSWERS: usize = 1;
pub const MAX_POLL_ANSWERS: usize = 10;
pub const MAX_POLL_QUESTION_CHARS: usize = 300;
pub const MAX_POLL_ANSWER_CHARS: usize = 55;
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
pub const PERM_SEND_POLLS: u64 = 1 << 49;
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_VOTES_BADGE_CAP: u32 = 99;
pub const ENCRYPTED_TOKEN_PREFIX: &str = "dQw4w9WgXcQ:";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollError {
    InvalidSnowflake(&'static str),
    InvalidQuestionLength(usize),
    InvalidAnswersCount(usize),
    InvalidAnswer(&'static str),
    MalformedJson(&'static str),
    PollExpired,
    EmptyVoteSelection,
    MultiselectNotAllowed,
    VoteNotPresent(u32),
    AccessDenied(&'static str),
    BoundaryViolation(&'static str),
    HttpError(u16, Option<u64>),
}

type Res<T> = Result<T, PollError>;

impl fmt::Display for PollError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for PollError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollVoteState {
    Ready,
    Voting,
    Voted,
}

impl PollVoteState {
    pub const fn element_name() -> &'static str {
        "poll-vote"
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Voting => "voting",
            Self::Voted => "voted",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PollLayoutType {
    Unknown = 0,
    Default = 1,
    ImageOnlyAnswers = 2,
}

impl PollLayoutType {
    pub const fn from_code(code: u8) -> Res<Self> {
        match code {
            0 => Ok(Self::Unknown),
            1 => Ok(Self::Default),
            2 => Ok(Self::ImageOnlyAnswers),
            _ => Err(PollError::InvalidAnswer("layout_type")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollEmoji {
    pub name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollMedia {
    pub text: Option<String>,
    pub emoji: Option<PollEmoji>,
}

impl PollMedia {
    pub fn with_text(text: &str) -> Self {
        Self {
            text: Some(text.to_string()),
            emoji: None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollAnswer {
    pub answer_id: u32,
    pub poll_media: PollMedia,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollAnswerCount {
    pub id: u32,
    pub count: u32,
    pub me_voted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PollResults {
    pub is_finalized: bool,
    pub answer_counts: Vec<PollAnswerCount>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct PollAnswerPercentage {
    pub answer_id: u32,
    pub count: u32,
    pub exact_percentage: f64,
    pub rounded_percentage: u8,
    pub me_voted: bool,
    pub is_victor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestPollVoteRequest {
    pub method: &'static str,
    pub path: String,
    pub short_path: String,
    pub json_body: String,
}

type VoteReq = RestPollVoteRequest;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PollVoteGatewayEvent {
    pub channel_id: u64,
    pub message_id: u64,
    pub user_id: u64,
    pub answer_id: u32,
}

type GwEv = PollVoteGatewayEvent;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PollHttpAction {
    Success,
    RetryLater(u64),
    ShowSkeleton,
    AbortFatal(u16),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Poll {
    pub question: PollMedia,
    pub answers: Vec<PollAnswer>,
    pub expiry_ms: Option<u64>,
    pub allow_multiselect: bool,
    pub layout_type: PollLayoutType,
    pub results: PollResults,
    pub vote_state: PollVoteState,
}

pub const fn validate_snowflake_id(id: u64, field: &'static str) -> Res<u64> {
    if id == 0 {
        return Err(PollError::InvalidSnowflake(field));
    }
    Ok(id)
}

pub fn calculate_vote_percentage_f64(votes: u32, total_votes: u32) -> f64 {
    if total_votes == 0 {
        return 0.0;
    }
    let ratio = (votes as f64 / total_votes as f64) * 100.0;
    if !ratio.is_finite() {
        return -1.0;
    }
    ratio.clamp(0.0, 100.0)
}

pub fn calculate_vote_percentage_rounded(votes: u32, total_votes: u32) -> u8 {
    if total_votes == 0 {
        return 0;
    }
    let pct = ((votes as f64 / total_votes as f64) * 100.0).round();
    if !pct.is_finite() || pct <= 0.0 {
        return 0;
    }
    pct.min(100.0) as u8
}

impl Poll {
    pub fn new(q: PollMedia, answers: Vec<PollAnswer>, exp: Option<u64>, multi: bool) -> Res<Self> {
        let q_len = q.text.as_deref().map_or(0, |s| s.trim().chars().count());
        if q_len == 0 || q_len > MAX_POLL_QUESTION_CHARS {
            return Err(PollError::InvalidQuestionLength(q_len));
        }
        if !(MIN_POLL_ANSWERS..=MAX_POLL_ANSWERS).contains(&answers.len()) {
            return Err(PollError::InvalidAnswersCount(answers.len()));
        }
        let mut seen = BTreeSet::new();
        for ans in &answers {
            let t = ans.poll_media.text.as_deref();
            let len = t.map_or(0, |s| s.trim().chars().count());
            if ans.answer_id == 0
                || !seen.insert(ans.answer_id)
                || len == 0
                || len > MAX_POLL_ANSWER_CHARS
            {
                return Err(PollError::InvalidAnswer("answer_spec"));
            }
        }
        Ok(Self {
            question: q,
            answers,
            expiry_ms: exp,
            allow_multiselect: multi,
            layout_type: PollLayoutType::Default,
            results: PollResults::default(),
            vote_state: PollVoteState::Ready,
        })
    }

    pub fn is_expired_at(&self, now_ms: u64) -> bool {
        self.results.is_finalized || self.expiry_ms.is_some_and(|exp| now_ms >= exp)
    }

    pub fn total_votes(&self) -> u32 {
        self.results
            .answer_counts
            .iter()
            .fold(0u32, |a, i| a.saturating_add(i.count))
    }

    pub fn answer_vote_count(&self, aid: u32) -> u32 {
        self.results
            .answer_counts
            .iter()
            .find(|c| c.id == aid)
            .map_or(0, |c| c.count)
    }

    fn apply_selected_answers(&mut self, unique_ids: &[u32]) {
        for c in &mut self.results.answer_counts {
            if c.me_voted && !unique_ids.contains(&c.id) {
                c.count = c.count.saturating_sub(1);
                c.me_voted = false;
            }
        }
        for &aid in unique_ids {
            if let Some(c) = self.results.answer_counts.iter_mut().find(|c| c.id == aid) {
                if !c.me_voted {
                    c.count = c.count.saturating_add(1);
                    c.me_voted = true;
                }
            } else {
                self.results.answer_counts.push(PollAnswerCount {
                    id: aid,
                    count: 1,
                    me_voted: true,
                });
            }
        }
    }

    pub fn cast_poll_vote(&mut self, ch: u64, msg: u64, aids: &[u32], now_ms: u64) -> Res<VoteReq> {
        validate_snowflake_id(ch, "channel_id")?;
        validate_snowflake_id(msg, "message_id")?;
        if self.is_expired_at(now_ms) {
            return Err(PollError::PollExpired);
        }
        if aids.is_empty() {
            return Err(PollError::EmptyVoteSelection);
        }
        let mut unique_ids = Vec::new();
        for &aid in aids {
            if !self.answers.iter().any(|a| a.answer_id == aid) {
                return Err(PollError::InvalidAnswer("unknown_aid"));
            }
            if !unique_ids.contains(&aid) {
                unique_ids.push(aid);
            }
        }
        if !self.allow_multiselect && unique_ids.len() > 1 {
            return Err(PollError::MultiselectNotAllowed);
        }
        self.vote_state = PollVoteState::Voting;
        self.apply_selected_answers(&unique_ids);
        self.vote_state = PollVoteState::Voted;
        Ok(build_vote_request("PUT", ch, msg, &unique_ids))
    }

    pub fn remove_poll_vote(&mut self, ch: u64, msg: u64, aid: u32, now_ms: u64) -> Res<VoteReq> {
        validate_snowflake_id(ch, "channel_id")?;
        validate_snowflake_id(msg, "message_id")?;
        if self.is_expired_at(now_ms) {
            return Err(PollError::PollExpired);
        }
        let counts = &mut self.results.answer_counts;
        let Some(entry) = counts.iter_mut().find(|c| c.id == aid && c.me_voted) else {
            return Err(PollError::VoteNotPresent(aid));
        };
        entry.count = entry.count.saturating_sub(1);
        entry.me_voted = false;
        let still = self.results.answer_counts.iter().any(|c| c.me_voted);
        self.vote_state = if still {
            PollVoteState::Voted
        } else {
            PollVoteState::Ready
        };
        Ok(build_vote_request("DELETE", ch, msg, &[aid]))
    }

    pub fn expire_poll_early(&mut self, ch: u64, msg: u64, now: u64) -> Res<VoteReq> {
        validate_snowflake_id(ch, "channel_id")?;
        validate_snowflake_id(msg, "message_id")?;
        self.results.is_finalized = true;
        self.expiry_ms = Some(now);
        let path = format!("/channels/{ch}/polls/{msg}/expire");
        Ok(VoteReq {
            method: "POST",
            path: path.clone(),
            short_path: path,
            json_body: "{}".to_string(),
        })
    }
}

fn build_vote_request(m: &'static str, ch: u64, msg: u64, aids: &[u32]) -> VoteReq {
    let first = aids.first().copied().unwrap_or(0);
    let path = format!("/channels/{ch}/polls/{msg}/answers/{first}");
    let short_path = format!("/polls/{msg}/answers/{first}");
    let csv = aids
        .iter()
        .map(u32::to_string)
        .collect::<Vec<_>>()
        .join(",");
    let json_body = if m == "DELETE" {
        "{\"answer_ids\":[]}".to_string()
    } else {
        format!("{{\"answer_ids\":[{csv}]}}")
    };
    VoteReq {
        method: m,
        path,
        short_path,
        json_body,
    }
}

pub fn compute_poll_percentages(poll: &Poll, now_ms: u64) -> Vec<PollAnswerPercentage> {
    let total = poll.total_votes();
    let max_v = poll
        .answers
        .iter()
        .map(|a| poll.answer_vote_count(a.answer_id))
        .max()
        .unwrap_or(0);
    let expired = poll.is_expired_at(now_ms);
    let mut out = Vec::with_capacity(poll.answers.len());
    for ans in &poll.answers {
        let count = poll.answer_vote_count(ans.answer_id);
        let me_voted = poll
            .results
            .answer_counts
            .iter()
            .any(|c| c.id == ans.answer_id && c.me_voted);
        out.push(PollAnswerPercentage {
            answer_id: ans.answer_id,
            count,
            exact_percentage: calculate_vote_percentage_f64(count, total),
            rounded_percentage: calculate_vote_percentage_rounded(count, total),
            me_voted,
            is_victor: expired && count >= max_v && count > 0,
        });
    }
    out
}

pub fn apply_gateway_poll_vote_add(poll: &mut Poll, ev: &GwEv, uid: u64) -> Res<u32> {
    validate_snowflake_id(ev.channel_id, "channel_id")?;
    validate_snowflake_id(ev.message_id, "message_id")?;
    validate_snowflake_id(ev.user_id, "user_id")?;
    if !poll.answers.iter().any(|a| a.answer_id == ev.answer_id) {
        return Err(PollError::InvalidAnswer("unknown_aid"));
    }
    let is_self = ev.user_id == uid;
    let counts = &mut poll.results.answer_counts;
    let count = if let Some(c) = counts.iter_mut().find(|c| c.id == ev.answer_id) {
        c.count = c.count.saturating_add(1);
        c.me_voted |= is_self;
        c.count
    } else {
        counts.push(PollAnswerCount {
            id: ev.answer_id,
            count: 1,
            me_voted: is_self,
        });
        1
    };
    if is_self {
        poll.vote_state = PollVoteState::Voted;
    }
    Ok(count)
}

pub fn apply_gateway_poll_vote_remove(poll: &mut Poll, ev: &GwEv, uid: u64) -> Res<u32> {
    validate_snowflake_id(ev.channel_id, "channel_id")?;
    validate_snowflake_id(ev.message_id, "message_id")?;
    validate_snowflake_id(ev.user_id, "user_id")?;
    let counts = &mut poll.results.answer_counts;
    let Some(c) = counts.iter_mut().find(|c| c.id == ev.answer_id) else {
        return Ok(0);
    };
    c.count = c.count.saturating_sub(1);
    let out = c.count;
    if ev.user_id == uid {
        c.me_voted = false;
        if !poll.results.answer_counts.iter().any(|x| x.me_voted) {
            poll.vote_state = PollVoteState::Ready;
        }
    }
    Ok(out)
}

fn extract_json_str(raw: &str, key: &str) -> Option<String> {
    let pat = format!("\"{key}\":\"");
    let start = raw.find(&pat)? + pat.len();
    let end = raw[start..].find('"')? + start;
    Some(raw[start..end].to_string())
}

fn extract_json_bool(raw: &str, key: &str) -> Option<bool> {
    let pat = format!("\"{key}\":");
    let rest = raw[raw.find(&pat)? + pat.len()..].trim_start();
    rest.starts_with("true")
        .then_some(true)
        .or_else(|| rest.starts_with("false").then_some(false))
}

fn extract_json_u64(raw: &str, key: &str) -> Option<u64> {
    let pat = format!("\"{key}\":");
    let rest = raw[raw.find(&pat)? + pat.len()..].trim_start();
    let digits: String = rest.chars().take_while(char::is_ascii_digit).collect();
    digits.parse::<u64>().ok()
}

fn parse_answers_block(raw: &str) -> Res<Vec<PollAnswer>> {
    let err = PollError::MalformedJson("answers");
    let after = &raw[raw.find("\"answers\":[").ok_or(err.clone())? + 11..];
    let end = after.find(']').ok_or(err.clone())?;
    let mut answers = Vec::new();
    for seg in after[..end].split("},{") {
        if !seg.trim().is_empty() {
            let aid = extract_json_u64(seg, "answer_id").ok_or(err.clone())? as u32;
            let emoji = extract_json_str(seg, "name").map(|n| PollEmoji { name: Some(n) });
            let pm = PollMedia {
                text: extract_json_str(seg, "text"),
                emoji,
            };
            answers.push(PollAnswer {
                answer_id: aid,
                poll_media: pm,
            });
        }
    }
    Ok(answers)
}

fn parse_results_block(raw: &str) -> PollResults {
    let Some(res_pos) = raw.find("\"results\":{") else {
        return PollResults::default();
    };
    let after = &raw[res_pos..];
    let is_finalized = extract_json_bool(after, "is_finalized").unwrap_or(false);
    let mut answer_counts = Vec::new();
    if let Some(ac_pos) = after.find("\"answer_counts\":[") {
        let ac_after = &after[ac_pos + 17..];
        if let Some(ac_end) = ac_after.find(']') {
            for seg in ac_after[..ac_end].split("},{") {
                if let (Some(id), Some(cnt)) =
                    (extract_json_u64(seg, "id"), extract_json_u64(seg, "count"))
                {
                    let me_voted = extract_json_bool(seg, "me_voted").unwrap_or(false);
                    answer_counts.push(PollAnswerCount {
                        id: id as u32,
                        count: cnt as u32,
                        me_voted,
                    });
                }
            }
        }
    }
    PollResults {
        is_finalized,
        answer_counts,
    }
}

pub fn deserialize_poll_json(raw: &str) -> Res<Poll> {
    let err = PollError::MalformedJson("poll");
    let q_pos = raw.find("\"question\":{").ok_or(err.clone())?;
    let q_text = extract_json_str(&raw[q_pos..], "text").ok_or(err.clone())?;
    let multi = extract_json_bool(raw, "allow_multiselect").ok_or(err)?;
    let mut poll = Poll::new(
        PollMedia::with_text(&q_text),
        parse_answers_block(raw)?,
        extract_json_u64(raw, "expiry_ms"),
        multi,
    )?;
    poll.layout_type =
        PollLayoutType::from_code(extract_json_u64(raw, "layout_type").unwrap_or(1) as u8)?;
    poll.results = parse_results_block(raw);
    if poll.results.answer_counts.iter().any(|c| c.me_voted) {
        poll.vote_state = PollVoteState::Voted;
    }
    Ok(poll)
}

pub fn rollback_optimistic_vote(poll: &mut Poll, snapshot: &PollResults) {
    poll.results = snapshot.clone();
    let voted = poll.results.answer_counts.iter().any(|c| c.me_voted);
    poll.vote_state = if voted {
        PollVoteState::Voted
    } else {
        PollVoteState::Ready
    };
}

pub fn validate_poll_workload_boundaries(
    sf: u64,
    msg_chars: usize,
    att: (usize, u64),
    offset: u32,
    vol: u16,
    perms: u64,
    age: (bool, bool),
) -> Res<()> {
    validate_snowflake_id(sf, "snowflake_id")?;
    if msg_chars > MAX_MESSAGE_CHARS
        || att.0 > MAX_ATTACHMENTS_COUNT
        || att.1 > MAX_ATTACHMENT_BYTES
    {
        return Err(PollError::BoundaryViolation("message_or_attachment_limit"));
    }
    if offset > MAX_SEARCH_OFFSET || vol > MAX_VOLUME_PERCENT {
        return Err(PollError::BoundaryViolation("offset_or_volume_exceeded"));
    }
    let req_perms = PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES | PERM_SEND_POLLS;
    if (perms & req_perms) != req_perms || (age.0 && !age.1) {
        return Err(PollError::AccessDenied(
            "missing_poll_permissions_or_age_gated",
        ));
    }
    Ok(())
}

pub const fn classify_poll_http_status(status: u16, retry_ms: Option<u64>) -> Res<PollHttpAction> {
    match status {
        200 | 201 | 204 => Ok(PollHttpAction::Success),
        202 => Ok(PollHttpAction::ShowSkeleton),
        429 => Ok(PollHttpAction::RetryLater(match retry_ms {
            Some(ms) if ms > 0 => ms,
            _ => 1_000,
        })),
        500..=599 => Ok(PollHttpAction::RetryLater(2_000)),
        400 | 401 | 403 | 404 | 413 => Err(PollError::HttpError(status, retry_ms)),
        other => Ok(PollHttpAction::AbortFatal(other)),
    }
}

pub fn format_poll_votes_badge(votes_count: u32) -> String {
    if votes_count > MAX_VOTES_BADGE_CAP {
        format!("{MAX_VOTES_BADGE_CAP}+")
    } else {
        votes_count.to_string()
    }
}

pub fn redact_poll_log(raw: &str) -> String {
    let mut words = Vec::new();
    for token in raw.split_whitespace() {
        let clean = token.trim_matches(|c: char| c == ',' || c == ';' || c == '"');
        if clean.starts_with("mfa.") && clean.len() > 8 {
            words.push("mfa.***".to_string());
        } else if clean.starts_with(ENCRYPTED_TOKEN_PREFIX) {
            words.push(format!("{ENCRYPTED_TOKEN_PREFIX}[REDACTED]"));
        } else if clean.len() >= 50 && clean.matches('.').count() == 2 {
            words.push("[REDACTED_TOKEN]".to_string());
        } else {
            words.push(token.to_string());
        }
    }
    words
        .join(" ")
        .replace("Bearer [REDACTED_TOKEN]", "Bearer ***")
        .replace("Bot [REDACTED_TOKEN]", "Bot ***")
}

pub fn export_poll_telemetry_json(poll: &Poll, note: &str) -> String {
    let safe = redact_poll_log(note);
    let (total, st) = (poll.total_votes(), poll.vote_state.as_str());
    format!(
        "{{\"container\":\"{POLL_ENGINE_CONTAINER}\",\"dimensions\":\"{POLL_ENGINE_DIMENSIONS}\",\"state\":\"{st}\",\"total_votes\":{total},\"note\":\"{safe}\"}}"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ans(id: u32, text: &str) -> PollAnswer {
        PollAnswer {
            answer_id: id,
            poll_media: PollMedia::with_text(text),
        }
    }

    fn sample_poll(multi: bool, exp_ms: Option<u64>) -> Poll {
        let answers = vec![ans(1, "Rust GPUI"), ans(2, "Electron"), ans(3, "Wayland")];
        Poll::new(
            PollMedia::with_text("Какой стек быстрее?"),
            answers,
            exp_ms,
            multi,
        )
        .unwrap()
    }

    #[test]
    fn test_sub_1004a_1_oracles_tokens_states_and_routes() {
        assert_eq!(POLL_ENGINE_CONTAINER, "Poll Engine");
        assert_eq!(POLL_ENGINE_DIMENSIONS, "Backend Module");
        assert_eq!(TOKEN_BACKGROUND, "#1e1f22");
        assert_eq!(TOKEN_SURFACE, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(PollVoteState::element_name(), "poll-vote");
        assert_eq!(PollVoteState::Ready.as_str(), "ready");
        assert_eq!(PollVoteState::Voting.as_str(), "voting");
        assert_eq!(PollVoteState::Voted.as_str(), "voted");
        assert_eq!(REST_POLL_SHORT_VOTE_TEMPLATE, "/polls/{id}/answers/{id}");
    }

    #[test]
    fn test_sub_1004a_2_deserialize_poll_and_gateway_vote_events() {
        let raw_json = r#"{"question":{"text":"Лучший рендерер?"},"answers":[{"answer_id":1,"poll_media":{"text":"WGPU"}},{"answer_id":7,"poll_media":{"text":"Vulkan"}},{"answer_id":8,"poll_media":{"text":"OpenGL"}}],"expiry":"2026-11-12T15:00:00.000Z","expiry_ms":2000000,"allow_multiselect":true,"layout_type":1,"results":{"is_finalized":false,"answer_counts":[{"id":1,"count":2,"me_voted":false},{"id":8,"count":1,"me_voted":false}]}}"#;
        let mut poll = deserialize_poll_json(raw_json).unwrap();
        assert_eq!(poll.question.text.as_deref(), Some("Лучший рендерер?"));
        assert_eq!(poll.answers.len(), 3);
        assert!(poll.allow_multiselect);
        assert_eq!(poll.layout_type, PollLayoutType::Default);
        assert_eq!(poll.total_votes(), 3);
        assert_eq!(poll.vote_state.as_str(), "ready");
        let uid = 606_781_270_382_346_250;
        let ev1 = GwEv {
            channel_id: 2,
            message_id: 3,
            user_id: uid,
            answer_id: 8,
        };
        assert_eq!(
            apply_gateway_poll_vote_add(&mut poll, &ev1, uid).unwrap(),
            2
        );
        assert_eq!(poll.vote_state, PollVoteState::Voted);
        assert_eq!(
            apply_gateway_poll_vote_remove(&mut poll, &ev1, uid).unwrap(),
            1
        );
        assert_eq!(poll.vote_state, PollVoteState::Ready);
    }

    #[test]
    fn test_sub_1004a_3_rest_voting_expiry_and_percentage_math() {
        assert_eq!(calculate_vote_percentage_f64(0, 0), 0.0);
        assert_eq!(calculate_vote_percentage_f64(5, 0), 0.0);
        assert_eq!(calculate_vote_percentage_rounded(0, 0), 0);
        assert_eq!(calculate_vote_percentage_rounded(5, 0), 0);
        assert_eq!(calculate_vote_percentage_rounded(1, 3), 33);
        assert_eq!(calculate_vote_percentage_rounded(2, 3), 67);
        assert_eq!(calculate_vote_percentage_rounded(1, 2), 50);
        let mut sp = sample_poll(false, Some(5_000));
        let err_multi = sp.cast_poll_vote(100, 200, &[1, 2], 1_000);
        assert_eq!(err_multi, Err(PollError::MultiselectNotAllowed));
        let v_ok = sp.cast_poll_vote(100, 200, &[1], 1_000).unwrap();
        assert_eq!(v_ok.method, "PUT");
        assert_eq!(v_ok.path, "/channels/100/polls/200/answers/1");
        assert_eq!(v_ok.short_path, "/polls/200/answers/1");
        assert_eq!(v_ok.json_body, "{\"answer_ids\":[1]}");
        assert_eq!(sp.answer_vote_count(1), 1);
        let del = sp.remove_poll_vote(100, 200, 1, 2_000).unwrap();
        assert_eq!(del.method, "DELETE");
        assert_eq!(sp.answer_vote_count(1), 0);
        assert_eq!(sp.vote_state, PollVoteState::Ready);
        assert_eq!(
            sp.cast_poll_vote(100, 200, &[1], 5_000),
            Err(PollError::PollExpired)
        );
        assert_eq!(
            sp.remove_poll_vote(100, 200, 1, 6_000),
            Err(PollError::PollExpired)
        );
    }

    #[test]
    fn test_sub_1004a_3_multiselect_and_victor() {
        let mut mp = sample_poll(true, Some(10_000));
        assert!(
            compute_poll_percentages(&mp, 1_000)
                .iter()
                .all(|p| p.rounded_percentage == 0)
        );
        let req = mp.cast_poll_vote(555, 777, &[1, 3], 2_000).unwrap();
        assert_eq!(req.json_body, "{\"answer_ids\":[1,3]}");
        let act = compute_poll_percentages(&mp, 3_000);
        assert_eq!(act[0].rounded_percentage, 50);
        assert!(!act[0].is_victor);
        let exp_req = mp.expire_poll_early(555, 777, 4_000).unwrap();
        assert_eq!(exp_req.path, "/channels/555/polls/777/expire");
        let fin = compute_poll_percentages(&mp, 4_000);
        assert!(fin[0].is_victor && !fin[1].is_victor && fin[2].is_victor);
    }

    #[test]
    fn test_sub_1004a_4_nine_dimensions_boundaries_and_http_statuses() {
        use PollHttpAction::{RetryLater, ShowSkeleton, Success};
        assert!(validate_snowflake_id(0, "id").is_err());
        assert_eq!(validate_snowflake_id(u64::MAX, "id").unwrap(), u64::MAX);
        assert_eq!(format_poll_votes_badge(42), "42");
        assert_eq!(format_poll_votes_badge(150), "99+");
        assert_eq!(classify_poll_http_status(204, None), Ok(Success));
        assert_eq!(classify_poll_http_status(202, None), Ok(ShowSkeleton));
        assert_eq!(
            classify_poll_http_status(429, Some(1500)),
            Ok(RetryLater(1500))
        );
        assert_eq!(classify_poll_http_status(503, None), Ok(RetryLater(2000)));
        for code in [400, 401, 403, 404, 413] {
            assert!(classify_poll_http_status(code, None).is_err());
        }
        let fp = PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES | PERM_SEND_POLLS;
        let v = validate_poll_workload_boundaries;
        assert!(v(1, 2000, (10, 25 * 1024 * 1024), 9975, 200, fp, (true, true)).is_ok());
        assert!(v(1, 2000, (1, 10), 0, 100, PERM_VIEW_CHANNEL, (false, false)).is_err());
        assert!(v(1, 2001, (1, 10), 0, 100, fp, (false, false)).is_err());
        assert!(v(1, 10, (1, 10), 0, 100, fp, (true, false)).is_err());
        let mut poll = sample_poll(false, Some(10_000));
        let snap = poll.results.clone();
        let _ = poll.cast_poll_vote(10, 20, &[2], 1_000).unwrap();
        rollback_optimistic_vote(&mut poll, &snap);
        assert_eq!(poll.vote_state, PollVoteState::Ready);
        let dyn_mfa = format!("mfa.{}", "x".repeat(84));
        let json = export_poll_telemetry_json(&poll, &format!("auth {dyn_mfa}"));
        assert!(json.contains("mfa.***") && !json.contains(&dyn_mfa));
    }
}
