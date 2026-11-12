//! Правая сплит-панель треда `ThreadSidebar` фиксированной ширины `360px`
//! с независимым виртуальным скроллом (`virtual_list`) и буфером сообщений (`TASK-1002B`).
//! Оракулы: `waves/kanban-board.json` (`TASK-1002B`, `TASK-1002`),
//! `/home/sanya/re-deep/frontend-d03.json` (`6095`, `657718`, `103557`, `306091`, `311582`),
//! `/home/sanya/re-deep/modules-index.json` (`456874`, `780057`, `802958`, `863005`, `970278`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeMap;
use std::fmt;

pub type Res<T = ()> = Result<T, ThreadSidebarError>;

pub const THREAD_SPLIT_VIEW_CONTAINER: &str = "Thread Split View";
pub const THREAD_SIDEBAR_DIMENSIONS: &str = "360px width";
pub const THREAD_SIDEBAR_WIDTH_PX: u32 = 360;
pub const THREAD_SIDEBAR_PADDING_PX: u32 = 8;
pub const THREAD_SIDEBAR_SPACING_PX: u32 = 8;
pub const SERVER_RAIL_WIDTH_PX: u32 = 72;
pub const CHANNEL_SIDEBAR_WIDTH_PX: u32 = 240;
pub const MIN_WINDOW_WIDTH_PX: u32 = 900;
pub const MAX_WINDOW_WIDTH_PX: u32 = 3_840;

pub const TOKEN_BACKGROUND: &str = "#2b2d31";
pub const TOKEN_SURFACE: &str = "#1e1f22";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#5865f2";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const D03_SCROLLER_BASE_CLASS: &str = "scrollerBase_d125d2";
pub const D03_VIRTUAL_LIST_CLASS: &str = "listItems_affa7e";
pub const D03_BUTTON_CLASS: &str = "button_a22cb0";
pub const D03_TEXT_INPUT_CLASS: &str = "input__75098";
pub const D03_NUMBER_BADGE_CLASS: &str = "numberBadge__463b7";
pub const D03_AVATAR_CLASS: &str = "avatar_fdd9da";
pub const D03_SCROLLBAR_THIN_THUMB_TOKEN: &str = "--scrollbar-thin-thumb";
pub const D03_SCROLLBAR_THIN_TRACK_TOKEN: &str = "--scrollbar-thin-track";
pub const RE_STORE_THREAD_MESSAGE: &str = "ThreadMessageStore";
pub const RE_STORE_THREAD_SUMMARY: &str = "ThreadSummaryStore";

pub const ALLOWED_ARCHIVE_DURATIONS: [u32; 4] = [60, 1440, 4320, 10080];
pub const MAX_THREAD_TITLE_CHARS: usize = 100;
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_MANAGE_THREADS: u64 = 1 << 34;
pub const PERM_SEND_MESSAGES_IN_THREADS: u64 = 1 << 38;

pub const EMPTY_THREAD_MESSAGES_RU: &str = "В этой ветке пока нет ответов";
pub const DELETED_STARTER_PREFIX_RU: &str = "Исходное сообщение удалено • ";
pub const ENCRYPTED_TOKEN_PREFIX: &str = "dQw4w9WgXcQ:";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadSplitInteractiveState {
    Closed,
    OpenButton,
    SidebarOpen,
}

impl ThreadSplitInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Closed => "closed",
            Self::OpenButton => "open button",
            Self::SidebarOpen => "sidebar open",
        }
    }

    #[must_use]
    pub const fn resolve(sidebar_open: bool, hovered: bool) -> Self {
        match (sidebar_open, hovered) {
            (true, _) => Self::SidebarOpen,
            (false, true) => Self::OpenButton,
            (false, false) => Self::Closed,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThreadIndicatorInteractiveState {
    Link,
    Underline,
    OpenSidebar,
}

impl ThreadIndicatorInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Link => "link",
            Self::Underline => "underline",
            Self::OpenSidebar => "open sidebar",
        }
    }

    #[must_use]
    pub const fn resolve(open_for_thread: bool, hovered: bool) -> Self {
        match (open_for_thread, hovered) {
            (true, _) => Self::OpenSidebar,
            (false, true) => Self::Underline,
            (false, false) => Self::Link,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadSidebarError {
    InvalidSnowflakeId(String),
    InvalidThreadTitle,
    InvalidAutoArchiveDuration(u32),
    ThreadNotFound(String),
    NoActiveThreadOpen,
    EmptyMessageContent,
    MessageTooLong(usize),
    TooManyAttachments(usize),
    LockedThreadRequiresManageThreads,
    MissingPermission(&'static str),
    AgeGatedSpaceRestricted,
    BoundaryViolation(&'static str),
    HttpBadRequest,
    HttpUnauthorized,
    HttpForbidden,
    HttpNotFound,
    HttpPayloadTooLarge,
}

impl fmt::Display for ThreadSidebarError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{self:?}")
    }
}

impl std::error::Error for ThreadSidebarError {}

pub fn validate_snowflake_id(raw: &str) -> Res<u64> {
    if raw.is_empty() || !raw.bytes().all(|b| b.is_ascii_digit()) {
        return Err(ThreadSidebarError::InvalidSnowflakeId(raw.to_string()));
    }
    let parsed = raw
        .parse::<u64>()
        .map_err(|_| ThreadSidebarError::InvalidSnowflakeId(raw.to_string()))?;
    if parsed == 0 {
        return Err(ThreadSidebarError::InvalidSnowflakeId(raw.to_string()));
    }
    Ok(parsed)
}

pub fn format_archive_duration_label_ru(minutes: u32) -> Res<&'static str> {
    match minutes {
        60 => Ok("1 ч."),
        1440 => Ok("24 ч."),
        4320 => Ok("3 дн."),
        10080 => Ok("1 нед."),
        other => Err(ThreadSidebarError::InvalidAutoArchiveDuration(other)),
    }
}

#[must_use]
pub fn format_unread_badge(unread_count: u32) -> String {
    if unread_count > MAX_BADGE_CAP {
        format!("{MAX_BADGE_CAP}+")
    } else {
        unread_count.to_string()
    }
}

#[must_use]
pub fn format_thread_indicator_ru(reply_count: u32, last: u64, now: u64, del: bool) -> String {
    let elapsed_min = now.saturating_sub(last) / 60_000;
    let prefix = if del { DELETED_STARTER_PREFIX_RU } else { "" };
    format!("{prefix}{reply_count} ответов • Последний {elapsed_min} мин назад")
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadSidebarLayoutSpec {
    pub container: &'static str,
    pub window_width_px: u32,
    pub main_chat_width_px: u32,
    pub sidebar_width_px: u32,
    pub padding_px: u32,
    pub spacing_px: u32,
    pub close_button_visible: bool,
    pub header_title: String,
    pub archive_badge_ru: &'static str,
    pub interactive_state: ThreadSplitInteractiveState,
}

pub fn compute_split_view_layout(
    window_width_px: u32,
    sidebar_open: bool,
    title: &str,
    archive_min: u32,
    hovered: bool,
) -> Res<ThreadSidebarLayoutSpec> {
    let trimmed = title.trim();
    if trimmed.is_empty() || trimmed.chars().count() > MAX_THREAD_TITLE_CHARS {
        return Err(ThreadSidebarError::InvalidThreadTitle);
    }
    let archive_badge_ru = format_archive_duration_label_ru(archive_min)?;
    let clamped_window = window_width_px.clamp(MIN_WINDOW_WIDTH_PX, MAX_WINDOW_WIDTH_PX);
    let sidebar_w = u32::from(sidebar_open) * THREAD_SIDEBAR_WIDTH_PX;
    let fixed_chrome = SERVER_RAIL_WIDTH_PX + CHANNEL_SIDEBAR_WIDTH_PX + sidebar_w;
    Ok(ThreadSidebarLayoutSpec {
        container: THREAD_SPLIT_VIEW_CONTAINER,
        window_width_px: clamped_window,
        main_chat_width_px: clamped_window.saturating_sub(fixed_chrome),
        sidebar_width_px: sidebar_w,
        padding_px: THREAD_SIDEBAR_PADDING_PX,
        spacing_px: THREAD_SIDEBAR_SPACING_PX,
        close_button_visible: sidebar_open,
        header_title: trimmed.to_string(),
        archive_badge_ru,
        interactive_state: ThreadSplitInteractiveState::resolve(sidebar_open, hovered),
    })
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct VirtualListViewport {
    pub scroll_offset_px: u32,
    pub top_item_index: usize,
    pub visible_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMessageItem {
    pub id: String,
    pub thread_id: String,
    pub author_id: String,
    pub content: String,
    pub attachment_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ThreadMessageBuffer {
    pub thread_id: String,
    pub parent_channel_id: String,
    pub starter_message_id: String,
    pub title: String,
    pub locked: bool,
    pub auto_archive_minutes: u32,
    pub starter_deleted: bool,
    pub last_activity_ms: u64,
    pub unread_count: u32,
    pub messages: Vec<ThreadMessageItem>,
    pub viewport: VirtualListViewport,
}

impl ThreadMessageBuffer {
    pub fn new(tid: &str, pid: &str, smid: &str, title: &str, min: u32, ms: u64) -> Res<Self> {
        validate_snowflake_id(tid)?;
        validate_snowflake_id(pid)?;
        validate_snowflake_id(smid)?;
        let trimmed = title.trim();
        if trimmed.is_empty() || trimmed.chars().count() > MAX_THREAD_TITLE_CHARS {
            return Err(ThreadSidebarError::InvalidThreadTitle);
        }
        format_archive_duration_label_ru(min)?;
        let vp = VirtualListViewport {
            scroll_offset_px: 0,
            top_item_index: 0,
            visible_count: 12,
        };
        Ok(Self {
            thread_id: tid.to_string(),
            parent_channel_id: pid.to_string(),
            starter_message_id: smid.to_string(),
            title: trimmed.to_string(),
            locked: false,
            auto_archive_minutes: min,
            starter_deleted: false,
            last_activity_ms: ms,
            unread_count: 0,
            messages: Vec::new(),
            viewport: vp,
        })
    }

    #[must_use]
    pub fn empty_placeholder_ru(&self) -> Option<&'static str> {
        self.messages.is_empty().then_some(EMPTY_THREAD_MESSAGES_RU)
    }
}

#[must_use]
pub fn visible_virtual_list_slice(buffer: &ThreadMessageBuffer) -> &[ThreadMessageItem] {
    let (len, vis) = (buffer.messages.len(), buffer.viewport.visible_count);
    if len == 0 || vis == 0 {
        return &[];
    }
    let start = buffer.viewport.top_item_index.min(len);
    &buffer.messages[start..start.saturating_add(vis).min(len)]
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StarterThreadIndicatorSpec {
    pub thread_id: String,
    pub starter_message_id: String,
    pub reply_count: u32,
    pub summary_ru: String,
    pub unread_badge: String,
    pub interactive_state: ThreadIndicatorInteractiveState,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ThreadHttpOutcome {
    Ok,
    SkeletonAccepted202,
    RateLimited { retry_after_ms: u64 },
    ServerTransient { status_code: u16 },
}

#[derive(Debug, Clone, Default)]
pub struct ThreadSidebarAppState {
    pub main_chat_messages: Vec<ThreadMessageItem>,
    pub main_chat_viewport: VirtualListViewport,
    pub thread_buffers: BTreeMap<String, ThreadMessageBuffer>,
    pub active_thread_id: Option<String>,
    pub sidebar_open: bool,
    pub sidebar_width_px: u32,
    pub input_focused: bool,
    pub active_draft: String,
    pub per_thread_drafts: BTreeMap<String, String>,
}

impl ThreadSidebarAppState {
    #[must_use]
    pub fn new() -> Self {
        Self {
            sidebar_width_px: THREAD_SIDEBAR_WIDTH_PX,
            ..Self::default()
        }
    }

    pub fn register_thread_buffer(&mut self, buf: ThreadMessageBuffer) -> Res {
        validate_snowflake_id(&buf.thread_id)?;
        self.thread_buffers.insert(buf.thread_id.clone(), buf);
        Ok(())
    }

    pub fn build_starter_thread_indicator(
        &self,
        tid: &str,
        now_ms: u64,
        hovered: bool,
    ) -> Res<StarterThreadIndicatorSpec> {
        let Some(buf) = self.thread_buffers.get(tid) else {
            return Err(ThreadSidebarError::ThreadNotFound(tid.to_string()));
        };
        let cnt = buf.messages.len() as u32;
        let is_open = self.sidebar_open && self.active_thread_id.as_deref() == Some(tid);
        let sum =
            format_thread_indicator_ru(cnt, buf.last_activity_ms, now_ms, buf.starter_deleted);
        Ok(StarterThreadIndicatorSpec {
            thread_id: buf.thread_id.clone(),
            starter_message_id: buf.starter_message_id.clone(),
            reply_count: cnt,
            summary_ru: sum,
            unread_badge: format_unread_badge(buf.unread_count),
            interactive_state: ThreadIndicatorInteractiveState::resolve(is_open, hovered),
        })
    }

    pub fn open_thread_in_sidebar(&mut self, tid: &str, perms: u64) -> Res {
        validate_snowflake_id(tid)?;
        if (perms & PERM_VIEW_CHANNEL) == 0 {
            return Err(ThreadSidebarError::MissingPermission("VIEW_CHANNEL"));
        }
        if !self.thread_buffers.contains_key(tid) {
            return Err(ThreadSidebarError::ThreadNotFound(tid.to_string()));
        }
        if let Some(prev_id) = self.active_thread_id.clone() {
            self.per_thread_drafts
                .insert(prev_id, self.active_draft.clone());
        }
        self.sidebar_open = true;
        self.sidebar_width_px = THREAD_SIDEBAR_WIDTH_PX;
        self.active_thread_id = Some(tid.to_string());
        self.active_draft = self.per_thread_drafts.get(tid).cloned().unwrap_or_default();
        self.input_focused = true;
        if let Some(buf) = self.thread_buffers.get_mut(tid) {
            buf.unread_count = 0;
        }
        Ok(())
    }

    pub fn switch_active_thread(&mut self, tid: &str, perms: u64) -> Res {
        self.open_thread_in_sidebar(tid, perms)
    }

    pub fn close_thread_sidebar(&mut self) {
        if let Some(active_id) = self.active_thread_id.take() {
            self.per_thread_drafts
                .insert(active_id, self.active_draft.clone());
        }
        self.active_draft.clear();
        self.sidebar_open = false;
        self.input_focused = false;
    }

    pub fn set_active_thread_draft(&mut self, text: &str) -> Res {
        let Some(active_id) = self.active_thread_id.clone() else {
            return Err(ThreadSidebarError::NoActiveThreadOpen);
        };
        if text.chars().count() > MAX_MESSAGE_CHARS {
            return Err(ThreadSidebarError::MessageTooLong(text.chars().count()));
        }
        self.active_draft = text.to_string();
        self.per_thread_drafts.insert(active_id, text.to_string());
        Ok(())
    }

    pub fn scroll_main_chat_virtual_list(&mut self, offset_px: u32, top_idx: usize, vis: usize) {
        self.main_chat_viewport = VirtualListViewport {
            scroll_offset_px: offset_px,
            top_item_index: top_idx,
            visible_count: vis,
        };
    }

    pub fn scroll_thread_virtual_list(
        &mut self,
        tid: &str,
        offset_px: u32,
        top_idx: usize,
        vis: usize,
    ) -> Res<VirtualListViewport> {
        validate_snowflake_id(tid)?;
        let Some(buf) = self.thread_buffers.get_mut(tid) else {
            return Err(ThreadSidebarError::ThreadNotFound(tid.to_string()));
        };
        let next = VirtualListViewport {
            scroll_offset_px: offset_px,
            top_item_index: top_idx,
            visible_count: vis,
        };
        buf.viewport = next;
        Ok(next)
    }

    pub fn send_thread_reply(
        &mut self,
        mid: &str,
        uid: &str,
        content: &str,
        att_cnt: usize,
        now_ms: u64,
        permissions_bits: u64,
    ) -> Res<ThreadMessageItem> {
        let Some(tid) = self.active_thread_id.clone() else {
            return Err(ThreadSidebarError::NoActiveThreadOpen);
        };
        validate_snowflake_id(mid)?;
        validate_snowflake_id(uid)?;
        let has_send = (permissions_bits & PERM_SEND_MESSAGES_IN_THREADS) != 0;
        if !has_send {
            return Err(ThreadSidebarError::MissingPermission(
                "SEND_MESSAGES_IN_THREADS",
            ));
        }
        let trimmed = validate_reply_payload(content, att_cnt)?;
        let Some(buf) = self.thread_buffers.get_mut(&tid) else {
            return Err(ThreadSidebarError::ThreadNotFound(tid));
        };
        if buf.locked && (permissions_bits & PERM_MANAGE_THREADS) == 0 {
            return Err(ThreadSidebarError::LockedThreadRequiresManageThreads);
        }
        buf.last_activity_ms = now_ms;
        let msg = ThreadMessageItem {
            id: mid.to_string(),
            thread_id: tid.clone(),
            author_id: uid.to_string(),
            content: trimmed.to_string(),
            attachment_count: att_cnt,
        };
        buf.messages.push(msg.clone());
        self.active_draft.clear();
        self.per_thread_drafts.insert(tid, String::new());
        self.input_focused = true;
        Ok(msg)
    }

    pub fn apply_thread_list_sync_recovery(
        &mut self,
        pid: &str,
        synced: Vec<ThreadMessageBuffer>,
    ) -> Res<usize> {
        validate_snowflake_id(pid)?;
        let mut preserved = BTreeMap::new();
        for (id, b) in &self.thread_buffers {
            if b.parent_channel_id == pid {
                preserved.insert(id.clone(), b.viewport);
            }
        }
        self.thread_buffers
            .retain(|_, b| b.parent_channel_id != pid);
        let mut count = 0;
        for mut buf in synced {
            if buf.parent_channel_id == pid {
                if let Some(vp) = preserved.get(&buf.thread_id) {
                    buf.viewport = *vp;
                }
                self.thread_buffers.insert(buf.thread_id.clone(), buf);
                count += 1;
            }
        }
        if let Some(active_id) = &self.active_thread_id
            && !self.thread_buffers.contains_key(active_id)
        {
            self.sidebar_open = false;
            self.active_thread_id = None;
            self.input_focused = false;
        }
        Ok(count)
    }

    #[must_use]
    pub fn export_thread_sidebar_telemetry_json(&self, raw_note: &str) -> String {
        let safe = redact_sidebar_secrets(raw_note);
        let act = self.active_thread_id.as_deref().unwrap_or("none");
        let (open, w, n, foc) = (
            self.sidebar_open,
            self.sidebar_width_px,
            self.thread_buffers.len(),
            self.input_focused,
        );
        format!(
            "{{\"container\":\"{THREAD_SPLIT_VIEW_CONTAINER}\",\"sidebar_open\":{open},\"sidebar_width_px\":{w},\"active_thread_id\":\"{act}\",\"threads_count\":{n},\"input_focused\":{foc},\"note\":\"{safe}\"}}"
        )
    }
}

fn validate_reply_payload(content: &str, att_cnt: usize) -> Res<&str> {
    let trimmed = content.trim();
    let char_len = trimmed.chars().count();
    if char_len == 0 && att_cnt == 0 {
        return Err(ThreadSidebarError::EmptyMessageContent);
    }
    if char_len > MAX_MESSAGE_CHARS {
        return Err(ThreadSidebarError::MessageTooLong(char_len));
    }
    if att_cnt > MAX_ATTACHMENTS_COUNT {
        return Err(ThreadSidebarError::TooManyAttachments(att_cnt));
    }
    Ok(trimmed)
}

pub fn validate_sidebar_workload_boundaries(
    msg_len: usize,
    att: (usize, u64),
    offset: u32,
    vol: u16,
    perms: u64,
    age: (bool, bool),
) -> Res {
    if (perms & PERM_VIEW_CHANNEL) == 0 {
        return Err(ThreadSidebarError::MissingPermission("VIEW_CHANNEL"));
    }
    if age.0 && !age.1 {
        return Err(ThreadSidebarError::AgeGatedSpaceRestricted);
    }
    if msg_len > MAX_MESSAGE_CHARS {
        return Err(ThreadSidebarError::MessageTooLong(msg_len));
    }
    if att.0 > MAX_ATTACHMENTS_COUNT {
        return Err(ThreadSidebarError::TooManyAttachments(att.0));
    }
    if att.1 > MAX_ATTACHMENT_BYTES || offset > MAX_SEARCH_OFFSET || vol > MAX_VOLUME_PERCENT {
        return Err(ThreadSidebarError::BoundaryViolation("workload_boundary"));
    }
    Ok(())
}

pub fn classify_thread_http_status(status: u16, retry_ms: u64) -> Res<ThreadHttpOutcome> {
    match status {
        200 | 201 | 204 => Ok(ThreadHttpOutcome::Ok),
        202 => Ok(ThreadHttpOutcome::SkeletonAccepted202),
        400 => Err(ThreadSidebarError::HttpBadRequest),
        401 => Err(ThreadSidebarError::HttpUnauthorized),
        403 => Err(ThreadSidebarError::HttpForbidden),
        404 => Err(ThreadSidebarError::HttpNotFound),
        413 => Err(ThreadSidebarError::HttpPayloadTooLarge),
        429 => Ok(ThreadHttpOutcome::RateLimited {
            retry_after_ms: retry_ms.max(100),
        }),
        500..=599 => Ok(ThreadHttpOutcome::ServerTransient {
            status_code: status,
        }),
        _ => Err(ThreadSidebarError::HttpBadRequest),
    }
}

#[must_use]
pub fn redact_sidebar_secrets(input: &str) -> String {
    let s1 = redact_prefix_token(input, ENCRYPTED_TOKEN_PREFIX, "dQw4w9WgXcQ:[REDACTED]");
    let s2 = redact_prefix_token(&s1, "Bearer ", "Bearer [REDACTED]");
    redact_prefix_token(&s2, "mfa.", "mfa.[REDACTED]")
}

fn redact_prefix_token(input: &str, prefix: &str, replacement: &str) -> String {
    let mut out = input.to_string();
    if let Some(pos) = out.find(prefix) {
        let tail = &out[pos + prefix.len()..];
        let end = tail
            .find(|c: char| c.is_whitespace() || c == '"' || c == '\'')
            .unwrap_or(tail.len());
        out.replace_range(pos..pos + prefix.len() + end, replacement);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use ThreadIndicatorInteractiveState as IndState;
    use ThreadSidebarError::*;
    use ThreadSplitInteractiveState as SplitState;

    fn mk_buf(tid: &str, pid: &str, smid: &str, min: u32, ms: u64) -> ThreadMessageBuffer {
        ThreadMessageBuffer::new(tid, pid, smid, "Тред", min, ms).unwrap()
    }

    #[test]
    fn test_sub_1002b_1_design_tokens_and_d03_oracles() {
        assert_eq!(THREAD_SPLIT_VIEW_CONTAINER, "Thread Split View");
        assert_eq!(THREAD_SIDEBAR_DIMENSIONS, "360px width");
        assert_eq!(THREAD_SIDEBAR_WIDTH_PX, 360);
        assert_eq!(THREAD_SIDEBAR_PADDING_PX, 8);
        assert_eq!(THREAD_SIDEBAR_SPACING_PX, 8);
        assert_eq!(TOKEN_BACKGROUND, "#2b2d31");
        assert_eq!(TOKEN_SURFACE, "#1e1f22");
        assert_eq!(TOKEN_TEXT_PRIMARY, "#ffffff");
        assert_eq!(TOKEN_ACCENT, "#5865f2");
        assert_eq!(TOKEN_BORDER, "#3f4147");
        assert_eq!(D03_SCROLLER_BASE_CLASS, "scrollerBase_d125d2");
        assert_eq!(D03_VIRTUAL_LIST_CLASS, "listItems_affa7e");
        assert_eq!(SplitState::resolve(false, false).as_str(), "closed");
        assert_eq!(SplitState::resolve(false, true).as_str(), "open button");
        assert_eq!(SplitState::resolve(true, false).as_str(), "sidebar open");
        assert_eq!(IndState::resolve(false, false).as_str(), "link");
        assert_eq!(IndState::resolve(false, true).as_str(), "underline");
        assert_eq!(IndState::resolve(true, false).as_str(), "open sidebar");
    }

    #[test]
    fn test_sub_1002b_2_fixed_360px_layout_and_resize_invariance() {
        for w in [900_u32, 1280, 1920, 2560, 3840] {
            let s = compute_split_view_layout(w, true, "DAVE", 1440, false).unwrap();
            assert_eq!(s.sidebar_width_px, 360);
            assert_eq!(s.padding_px, 8);
            assert_eq!(s.spacing_px, 8);
            assert!(s.close_button_visible && s.archive_badge_ru == "24 ч.");
            let exp_main = w - SERVER_RAIL_WIDTH_PX - CHANNEL_SIDEBAR_WIDTH_PX - 360;
            assert_eq!(s.main_chat_width_px, exp_main);
            assert_eq!(s.interactive_state, SplitState::SidebarOpen);
        }
        let c = compute_split_view_layout(1280, false, "DAVE", 60, true).unwrap();
        assert_eq!((c.sidebar_width_px, c.close_button_visible), (0, false));
        assert_eq!(c.archive_badge_ru, "1 ч.");
        assert_eq!(c.interactive_state, SplitState::OpenButton);
    }

    #[test]
    fn test_sub_1002b_2_independent_virtual_list_and_message_buffer() {
        let mut st = ThreadSidebarAppState::new();
        let mut b1 = mk_buf("9001", "100", "5001", 60, 120_000);
        for i in 1..=10_u32 {
            b1.messages.push(ThreadMessageItem {
                id: format!("700{i}"),
                thread_id: "9001".into(),
                author_id: "42".into(),
                content: format!("Msg #{i}"),
                attachment_count: 0,
            });
        }
        st.register_thread_buffer(b1).unwrap();
        st.register_thread_buffer(mk_buf("9002", "100", "5002", 4320, 180_000))
            .unwrap();
        st.scroll_main_chat_virtual_list(1_440, 24, 15);
        let vp1 = st.scroll_thread_virtual_list("9001", 320, 3, 4).unwrap();
        let vp2 = st.scroll_thread_virtual_list("9002", 80, 1, 6).unwrap();
        assert_eq!((vp1.scroll_offset_px, vp2.scroll_offset_px), (320, 80));
        assert_eq!(st.main_chat_viewport.scroll_offset_px, 1_440);
        let slice = visible_virtual_list_slice(&st.thread_buffers["9001"]);
        assert_eq!(
            (slice.len(), slice[0].id.as_str(), slice[3].id.as_str()),
            (4, "7004", "7007")
        );
    }

    #[test]
    fn test_sub_1002b_3_starter_indicator_and_app_state_focus() {
        let mut st = ThreadSidebarAppState::new();
        let mut b1 = mk_buf("9101", "200", "6001", 1440, 600_000);
        b1.unread_count = 125;
        st.register_thread_buffer(b1).unwrap();
        let mut b2 = mk_buf("9102", "200", "6002", 10080, 780_000);
        b2.starter_deleted = true;
        st.register_thread_buffer(b2).unwrap();
        let ind = st
            .build_starter_thread_indicator("9101", 900_000, true)
            .unwrap();
        assert_eq!(ind.summary_ru, "0 ответов • Последний 5 мин назад");
        assert_eq!(ind.unread_badge, "99+");
        assert_eq!(ind.interactive_state, IndState::Underline);
        let perms = PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES_IN_THREADS;
        st.open_thread_in_sidebar("9101", perms).unwrap();
        assert!(st.sidebar_open && st.input_focused);
        st.set_active_thread_draft("Черновик 1").unwrap();
        st.switch_active_thread("9102", perms).unwrap();
        assert!(st.input_focused && st.active_draft.is_empty());
        st.send_thread_reply("8801", "77", "Ответ 2", 1, 840_000, perms)
            .unwrap();
        assert!(st.input_focused && st.main_chat_messages.is_empty());
        assert_eq!(st.thread_buffers["9102"].messages.len(), 1);
        let ind2 = st
            .build_starter_thread_indicator("9102", 1_140_000, false)
            .unwrap();
        let exp = "Исходное сообщение удалено • 1 ответов • Последний 5 мин назад";
        assert_eq!(ind2.summary_ru, exp);
        assert_eq!(ind2.interactive_state, IndState::OpenSidebar);
        st.switch_active_thread("9101", perms).unwrap();
        assert!(st.input_focused && st.active_draft == "Черновик 1");
        st.close_thread_sidebar();
        assert!(!st.sidebar_open && !st.input_focused && st.active_thread_id.is_none());
    }

    #[test]
    fn test_sub_1002b_4_boundaries_and_http_status() {
        assert!(validate_snowflake_id("0").is_err());
        assert_eq!(validate_snowflake_id("18446744073709551615"), Ok(u64::MAX));
        assert!(validate_snowflake_id("18446744073709551616").is_err());
        for d in [0_u32, 59, 61, 1439, 1441, 4319, 4321, 10079, 10081] {
            assert!(format_archive_duration_label_ru(d).is_err());
        }
        let (v, vp) = (validate_sidebar_workload_boundaries, PERM_VIEW_CHANNEL);
        assert!(v(2_000, (10, 25 * 1024 * 1024), 9_975, 200, vp, (true, true)).is_ok());
        assert_eq!(
            v(2_001, (0, 0), 0, 100, vp, (false, true)),
            Err(MessageTooLong(2_001))
        );
        assert_eq!(
            v(10, (0, 0), 0, 100, vp, (true, false)),
            Err(AgeGatedSpaceRestricted)
        );
        let c = classify_thread_http_status;
        assert_eq!(c(202, 0), Ok(ThreadHttpOutcome::SkeletonAccepted202));
        let rl = ThreadHttpOutcome::RateLimited {
            retry_after_ms: 1_250,
        };
        assert_eq!(c(429, 1_250), Ok(rl));
        let s503 = ThreadHttpOutcome::ServerTransient { status_code: 503 };
        assert_eq!(c(503, 0), Ok(s503));
        assert_eq!(c(403, 0), Err(HttpForbidden));
    }

    #[test]
    fn test_sub_1002b_4_permissions_sync_and_redaction() {
        let mut st = ThreadSidebarAppState::new();
        let mut lb = mk_buf("9500", "300", "7000", 60, 100_000);
        assert_eq!(lb.empty_placeholder_ru(), Some(EMPTY_THREAD_MESSAGES_RU));
        lb.locked = true;
        st.register_thread_buffer(lb).unwrap();
        let vp = PERM_VIEW_CHANNEL;
        st.open_thread_in_sidebar("9500", vp).unwrap();
        let r1 = st.send_thread_reply("1", "2", "hi", 0, 101_000, vp);
        assert_eq!(r1, Err(MissingPermission("SEND_MESSAGES_IN_THREADS")));
        let ps = vp | PERM_SEND_MESSAGES_IN_THREADS;
        let r2 = st.send_thread_reply("1", "2", "hi", 0, 101_000, ps);
        assert_eq!(r2, Err(LockedThreadRequiresManageThreads));
        st.scroll_thread_virtual_list("9500", 512, 8, 10).unwrap();
        let synced = mk_buf("9500", "300", "7000", 60, 200_000);
        let cnt = st
            .apply_thread_list_sync_recovery("300", vec![synced])
            .unwrap();
        assert_eq!(cnt, 1);
        assert_eq!(st.thread_buffers["9500"].viewport.scroll_offset_px, 512);
        let raw = format!("tok {ENCRYPTED_TOKEN_PREFIX}SECRET_999 mfa.ABCD_12");
        let tel = st.export_thread_sidebar_telemetry_json(&raw);
        assert!(!tel.contains("SECRET_999") && !tel.contains("ABCD_12"));
        assert!(tel.contains("dQw4w9WgXcQ:[REDACTED]") && tel.contains("mfa.[REDACTED]"));
    }
}
