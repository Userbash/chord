#![forbid(unsafe_code)]
#![allow(dead_code)]
//! GPUI-оверлей поля ввода `InputDecoratorView` (`TextView overlay`):
//! рендеринг красной волнистой линии `#f23f43` (`wavy-underline`) с дебаунсом расчёта
//! координат `150 мс` и всплывающее контекстное меню замены при клике ПКМ (`TASK-1006B`).
//! Оракулы: `waves/kanban-board.json` (`TASK-1006B`), `/home/sanya/re-deep/frontend-d03.json`
//! (`input__75098`, `editor__25bec`, `menu_c1e9c4`, `scroller_c1e9c4`, `separator_c1e9c4`,
//! `CONTEXT_MENU_OPEN`, `CONTEXT_MENU_CLOSE`, `layer__59d0d`, `tooltip_c36707`,
//! `--input-border-error-default`, `--input-text-error-default`).

use std::collections::BTreeSet;

pub const INPUT_DECORATOR_CONTAINER: &str = "Input Decorator";
pub const INPUT_DECORATOR_DIMENSIONS: &str = "TextView overlay";
pub const INPUT_DECORATOR_PADDING_PX: u32 = 0;
pub const INPUT_DECORATOR_SPACING_PX: u32 = 0;
pub const TOKEN_BACKGROUND_HEX: &str = "#383a40";
pub const TOKEN_SURFACE_HEX: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY_HEX: &str = "#ffffff";
pub const TOKEN_WAVY_UNDERLINE_HEX: &str = "#f23f43";
pub const TOKEN_BORDER_HEX: &str = "#3f4147";
pub const WAVY_UNDERLINE_ELEMENT: &str = "wavy-underline";
pub const COORDINATE_DEBOUNCE_MS: u64 = 150;
pub const MAX_SUGGESTIONS: usize = 5;
pub const MAX_EDIT_DISTANCE: usize = 2;

pub const DEFAULT_GLYPH_WIDTH_PX: f32 = 8.0;
pub const DEFAULT_LINE_HEIGHT_PX: f32 = 22.0;
pub const DEFAULT_BASELINE_OFFSET_PX: f32 = 18.0;
pub const WAVE_WAVELENGTH_PX: f32 = 4.0;
pub const WAVE_AMPLITUDE_PX: f32 = 1.5;
pub const POPUP_MENU_MIN_WIDTH_PX: u32 = 220;
pub const POPUP_ITEM_HEIGHT_PX: u32 = 32;
pub const POPUP_SCROLLER_PADDING_PX: u32 = 8;

pub const D03_TEXT_INPUT_CLASS: &str = "input__75098";
pub const D03_SLATE_EDITOR_CLASS: &str = "editor__25bec";
pub const D03_INPUT_BORDER_ERROR_TOKEN: &str = "--input-border-error-default";
pub const D03_INPUT_TEXT_ERROR_TOKEN: &str = "--input-text-error-default";
pub const D03_MENU_CLASS: &str = "menu_c1e9c4";
pub const D03_MENU_SCROLLER_CLASS: &str = "scroller_c1e9c4";
pub const D03_MENU_SEPARATOR_CLASS: &str = "separator_c1e9c4";
pub const D03_CONTEXT_MENU_OPEN_EVENT: &str = "CONTEXT_MENU_OPEN";
pub const D03_CONTEXT_MENU_CLOSE_EVENT: &str = "CONTEXT_MENU_CLOSE";
pub const D03_POPOVER_LAYER_CLASS: &str = "layer__59d0d";
pub const D03_TOOLTIP_CLASS: &str = "tooltip_c36707";

pub const IPC_SPELLCHECK_REPLACE_MISSPELLING: &str = "DISCORD_SPELLCHECK_REPLACE_MISSPELLING";
pub const IPC_SPELLCHECK_SET_LEARNED_WORDS: &str = "DISCORD_SPELLCHECK_SET_LEARNED_WORDS";
pub const I18N_KEY_ADD_TO_DICTIONARY: &str = "HJmG1G";
pub const LABEL_RU_ADD_TO_DICTIONARY: &str = "Добавить в словарь";
pub const LABEL_RU_NO_SUGGESTIONS: &str = "Нет вариантов замены";

pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

pub const BUILTIN_WORDS_EN: [&str; 6] = ["hello", "world", "check", "don't", "well-known", "would"];
pub const BUILTIN_WORDS_RU: [&str; 6] = ["привет", "мир", "и", "проверка", "орфография", "словарь"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavyUnderlineInteractiveState {
    RedWave,
    Pointer,
    ContextMenu,
}

impl WavyUnderlineInteractiveState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::RedWave => "red-wave",
            Self::Pointer => "pointer",
            Self::ContextMenu => "context menu",
        }
    }

    #[must_use]
    pub const fn resolve(menu_open: bool, hovered: bool) -> Self {
        if menu_open {
            Self::ContextMenu
        } else if hovered {
            Self::Pointer
        } else {
            Self::RedWave
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputSpellcheckError {
    InvalidSnowflake(u64),
    MessageTooLong(usize),
    BoundaryViolation(&'static str),
    MissingPermission(&'static str),
    AgeGatedSpaceRestricted,
    NoMisspelledSpanAtTarget,
    NoActiveContextMenu,
    InvalidSuggestionIndex(usize),
    HttpFailure { status: u16, retry_after_ms: u64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct WavyWavePoint {
    pub x_px: f32,
    pub y_px: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WavyUnderlineSegment {
    pub word: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub line_row: u32,
    pub start_col: u32,
    pub x_px: f32,
    pub y_px: f32,
    pub width_px: f32,
    pub baseline_y_px: f32,
    pub color_hex: &'static str,
    pub interactive_state: WavyUnderlineInteractiveState,
    pub vertices: Vec<WavyWavePoint>,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellcheckMenuItemSpec {
    pub label: String,
    pub enabled: bool,
    pub focused: bool,
    pub separator_before: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellcheckContextMenuPopup {
    pub span_index: usize,
    pub target_word: String,
    pub x_px: u32,
    pub y_px: u32,
    pub width_px: u32,
    pub height_px: u32,
    pub wavy_state: &'static str,
    pub focused_index: usize,
    pub items: Vec<SpellcheckMenuItemSpec>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OneClickReplacementResult {
    pub updated_text: String,
    pub updated_cursor_byte: usize,
    pub ipc_channel: &'static str,
    pub remaining_misspellings: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictionarySyncDirective {
    Ready,
    SkeletonWarmingUp { retry_after_ms: u64 },
    RateLimited { retry_after_ms: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InputWordToken {
    pub word: String,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[must_use]
pub fn tokenize_text_view(text: &str) -> Vec<InputWordToken> {
    let (mut out, mut cursor) = (Vec::new(), 0usize);
    for raw in text.split_whitespace() {
        let Some(rel) = text[cursor..].find(raw) else {
            continue;
        };
        let chunk_start = cursor + rel;
        cursor = chunk_start + raw.len();
        if raw.contains("://")
            || raw.contains('@')
            || raw.starts_with('`')
            || raw.starts_with('<')
            || (raw.starts_with(':') && raw.ends_with(':') && raw.len() >= 4)
        {
            continue;
        }
        let trimmed = raw.trim_matches(|c: char| !c.is_alphabetic());
        if trimmed.is_empty()
            || !trimmed
                .chars()
                .all(|c| c.is_alphabetic() || c == '\'' || c == '-')
        {
            continue;
        }
        if let Some(word_rel) = raw.find(trimmed) {
            let start_byte = chunk_start + word_rel;
            out.push(InputWordToken {
                word: trimmed.to_string(),
                start_byte,
                end_byte: start_byte + trimmed.len(),
            });
        }
    }
    out
}

#[must_use]
pub fn damerau_levenshtein(left: &str, right: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (left.chars().collect(), right.chars().collect());
    let (m, n, width) = (a.len(), b.len(), b.len() + 1);
    let mut dp = vec![0usize; (m + 1) * width];
    for i in 0..=m {
        dp[i * width] = i;
    }
    for j in 0..=n {
        dp[j] = j;
    }
    for i in 1..=m {
        for j in 1..=n {
            let cost = usize::from(a[i - 1] != b[j - 1]);
            let best = (dp[(i - 1) * width + j] + 1)
                .min(dp[i * width + (j - 1)] + 1)
                .min(dp[(i - 1) * width + (j - 1)] + cost);
            dp[i * width + j] = if i > 1 && j > 1 && a[i - 1] == b[j - 2] && a[i - 2] == b[j - 1] {
                best.min(dp[(i - 2) * width + (j - 2)] + 1)
            } else {
                best
            };
        }
    }
    dp[m * width + n]
}

#[must_use]
pub fn suggest_word_replacements(word: &str, lexicon: &BTreeSet<String>) -> Vec<String> {
    let lower = word.to_lowercase();
    if lower.is_empty() || MAX_SUGGESTIONS == 0 {
        return Vec::new();
    }
    let (first_ch, q_len) = (lower.chars().next(), lower.chars().count());
    let capitalized = word.chars().next().is_some_and(char::is_uppercase);
    let mut candidates: Vec<(usize, usize, usize, String)> = Vec::new();
    for entry in lexicon {
        let dist = damerau_levenshtein(&lower, entry);
        if dist == 0 || dist > MAX_EDIT_DISTANCE {
            continue;
        }
        let first_penalty = usize::from(entry.chars().next() != first_ch);
        let len_diff = entry.chars().count().abs_diff(q_len);
        let formatted = if capitalized {
            let mut chs = entry.chars();
            chs.next().map_or_else(String::new, |c| {
                c.to_uppercase().collect::<String>() + chs.as_str()
            })
        } else {
            entry.clone()
        };
        candidates.push((dist, first_penalty, len_diff, formatted));
    }
    candidates.sort_by(|a, b| a.cmp(b));
    candidates
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, _, _, w)| w)
        .collect()
}

#[must_use]
pub fn generate_wavy_vertices(start_x: f32, width: f32, baseline_y: f32) -> Vec<WavyWavePoint> {
    let safe_w = width.max(WAVE_WAVELENGTH_PX);
    let half_w = (WAVE_WAVELENGTH_PX * 0.5).max(1.0);
    let steps = ((safe_w / half_w).ceil() as usize).max(2);
    let mut points = Vec::with_capacity(steps + 1);
    for step in 0..=steps {
        let x_px = (start_x + (step as f32) * half_w).min(start_x + safe_w);
        let phase = match step % 4 {
            1 => -WAVE_AMPLITUDE_PX,
            3 => WAVE_AMPLITUDE_PX,
            _ => 0.0,
        };
        points.push(WavyWavePoint {
            x_px,
            y_px: baseline_y + phase,
        });
    }
    points
}

fn compute_row_and_col(text: &str, start_byte: usize, max_cols: u32) -> (u32, u32) {
    let wrap_cols = max_cols.max(8);
    let (mut row, mut col) = (0u32, 0u32);
    for ch in text.get(..start_byte).unwrap_or("").chars() {
        if ch == '\n' || col + 1 >= wrap_cols {
            row += 1;
            col = 0;
        } else {
            col += 1;
        }
    }
    (row, col)
}

#[must_use]
pub fn build_wavy_underline_segments(
    text: &str,
    lexicon: &BTreeSet<String>,
    max_cols: u32,
) -> Vec<WavyUnderlineSegment> {
    let mut segments = Vec::new();
    for token in tokenize_text_view(text) {
        if lexicon.contains(&token.word.to_lowercase()) {
            continue;
        }
        let (line_row, start_col) = compute_row_and_col(text, token.start_byte, max_cols);
        let char_len = (token.word.chars().count() as u32).max(1);
        let x_px = (start_col as f32) * DEFAULT_GLYPH_WIDTH_PX;
        let y_px = (line_row as f32) * DEFAULT_LINE_HEIGHT_PX;
        let width_px = (char_len as f32) * DEFAULT_GLYPH_WIDTH_PX;
        let baseline_y_px = y_px + DEFAULT_BASELINE_OFFSET_PX;
        let vertices = generate_wavy_vertices(x_px, width_px, baseline_y_px);
        let suggestions = suggest_word_replacements(&token.word, lexicon);
        segments.push(WavyUnderlineSegment {
            word: token.word,
            start_byte: token.start_byte,
            end_byte: token.end_byte,
            line_row,
            start_col,
            x_px,
            y_px,
            width_px,
            baseline_y_px,
            color_hex: TOKEN_WAVY_UNDERLINE_HEX,
            interactive_state: WavyUnderlineInteractiveState::RedWave,
            vertices,
            suggestions,
        });
    }
    segments
}

#[must_use]
pub fn clamp_popup_to_viewport(
    anchor: (i32, i32),
    popup: (u32, u32),
    viewport: (u32, u32),
) -> (u32, u32) {
    let clamped_x = (anchor.0.max(0) as u32).min(viewport.0.saturating_sub(popup.0));
    let clamped_y = (anchor.1.max(0) as u32).min(viewport.1.saturating_sub(popup.1));
    (clamped_x, clamped_y)
}

#[derive(Default)]
pub struct InputSpellcheckDecorator {
    lexicon: BTreeSet<String>,
    learned_words: BTreeSet<String>,
    text: String,
    cursor_byte: usize,
    max_columns: u32,
    viewport: (u32, u32),
    pending_debounce: bool,
    debounce_deadline_ms: u64,
    segments: Vec<WavyUnderlineSegment>,
    active_popup: Option<SpellcheckContextMenuPopup>,
}

impl InputSpellcheckDecorator {
    #[must_use]
    pub fn new() -> Self {
        let lexicon = BUILTIN_WORDS_EN
            .iter()
            .chain(BUILTIN_WORDS_RU.iter())
            .map(|w| (*w).to_lowercase())
            .collect();
        Self {
            lexicon,
            max_columns: 80,
            viewport: (1_280, 720),
            ..Self::default()
        }
    }

    pub fn set_viewport(&mut self, width_px: u32, height_px: u32, max_columns: u32) {
        self.viewport = (width_px.max(320), height_px.max(240));
        self.max_columns = max_columns.max(16);
    }

    pub fn on_text_input(&mut self, next_text: &str, cursor_byte: usize, timestamp_ms: u64) {
        self.text = next_text.to_string();
        self.cursor_byte = cursor_byte.min(self.text.len());
        self.debounce_deadline_ms = timestamp_ms.saturating_add(COORDINATE_DEBOUNCE_MS);
        self.pending_debounce = true;
        self.active_popup = None;
    }

    #[must_use]
    pub const fn is_debounce_pending(&self) -> bool {
        self.pending_debounce
    }

    pub fn flush_debounced_coordinates(&mut self, now_ms: u64) -> bool {
        if !self.pending_debounce || now_ms < self.debounce_deadline_ms {
            return false;
        }
        self.recompute_segments();
        self.pending_debounce = false;
        true
    }

    fn recompute_segments(&mut self) {
        let mut combined = self.lexicon.clone();
        for learned in &self.learned_words {
            combined.insert(learned.clone());
        }
        self.segments = build_wavy_underline_segments(&self.text, &combined, self.max_columns);
    }

    #[must_use]
    pub fn segments(&self) -> &[WavyUnderlineSegment] {
        &self.segments
    }

    pub fn hover_at_point(&mut self, x_px: f32, y_px: f32) -> Option<usize> {
        let mut hit = None;
        for (idx, seg) in self.segments.iter_mut().enumerate() {
            let inside = x_px >= seg.x_px
                && x_px <= seg.x_px + seg.width_px
                && y_px >= seg.y_px
                && y_px <= seg.y_px + DEFAULT_LINE_HEIGHT_PX;
            if inside && hit.is_none() {
                seg.interactive_state = WavyUnderlineInteractiveState::Pointer;
                hit = Some(idx);
            } else {
                seg.interactive_state = WavyUnderlineInteractiveState::RedWave;
            }
        }
        hit
    }

    pub fn open_context_menu_on_right_click(
        &mut self,
        byte_offset: usize,
        click_x: i32,
        click_y: i32,
    ) -> Result<SpellcheckContextMenuPopup, InputSpellcheckError> {
        let span_index = self
            .segments
            .iter()
            .position(|s| byte_offset >= s.start_byte && byte_offset <= s.end_byte)
            .ok_or(InputSpellcheckError::NoMisspelledSpanAtTarget)?;
        let seg = self
            .segments
            .get_mut(span_index)
            .ok_or(InputSpellcheckError::NoMisspelledSpanAtTarget)?;
        seg.interactive_state = WavyUnderlineInteractiveState::ContextMenu;
        let items = build_popup_items(&seg.suggestions);
        let height_px = (items.len() as u32) * POPUP_ITEM_HEIGHT_PX + POPUP_SCROLLER_PADDING_PX * 2;
        let (x_px, y_px) = clamp_popup_to_viewport(
            (click_x, click_y),
            (POPUP_MENU_MIN_WIDTH_PX, height_px),
            self.viewport,
        );
        let popup = SpellcheckContextMenuPopup {
            span_index,
            target_word: seg.word.clone(),
            x_px,
            y_px,
            width_px: POPUP_MENU_MIN_WIDTH_PX,
            height_px,
            wavy_state: seg.interactive_state.as_str(),
            focused_index: 0,
            items,
        };
        self.active_popup = Some(popup.clone());
        Ok(popup)
    }

    pub fn navigate_context_menu(&mut self, key: &str) -> Option<usize> {
        if key == "Escape" {
            self.active_popup = None;
            return None;
        }
        let popup = self.active_popup.as_mut()?;
        let count = popup.items.len();
        if count == 0 {
            return None;
        }
        popup.focused_index = match key {
            "ArrowDown" => (popup.focused_index + 1) % count,
            "ArrowUp" => (popup.focused_index + count - 1) % count,
            _ => popup.focused_index,
        };
        for (idx, item) in popup.items.iter_mut().enumerate() {
            item.focused = idx == popup.focused_index;
        }
        Some(popup.focused_index)
    }

    pub fn replace_misspelled_word_one_click(
        &mut self,
        suggestion_index: usize,
    ) -> Result<OneClickReplacementResult, InputSpellcheckError> {
        let popup = self
            .active_popup
            .take()
            .ok_or(InputSpellcheckError::NoActiveContextMenu)?;
        let seg = self
            .segments
            .get(popup.span_index)
            .cloned()
            .ok_or(InputSpellcheckError::NoActiveContextMenu)?;
        let replacement = seg.suggestions.get(suggestion_index).cloned().ok_or(
            InputSpellcheckError::InvalidSuggestionIndex(suggestion_index),
        )?;
        let (span_start, span_end) = (seg.start_byte, seg.end_byte);
        self.text.replace_range(span_start..span_end, &replacement);
        self.cursor_byte = span_start + replacement.len();
        self.recompute_segments();
        Ok(OneClickReplacementResult {
            updated_text: self.text.clone(),
            updated_cursor_byte: self.cursor_byte,
            ipc_channel: IPC_SPELLCHECK_REPLACE_MISSPELLING,
            remaining_misspellings: self.segments.len(),
        })
    }

    pub fn add_active_word_to_dictionary(&mut self) -> Result<&'static str, InputSpellcheckError> {
        let popup = self
            .active_popup
            .take()
            .ok_or(InputSpellcheckError::NoActiveContextMenu)?;
        self.learned_words.insert(popup.target_word.to_lowercase());
        self.recompute_segments();
        Ok(IPC_SPELLCHECK_SET_LEARNED_WORDS)
    }

    pub fn remove_word_from_dictionary(&mut self, word: &str) -> &'static str {
        self.learned_words.remove(&word.to_lowercase());
        self.recompute_segments();
        IPC_SPELLCHECK_SET_LEARNED_WORDS
    }
}

fn build_popup_items(suggestions: &[String]) -> Vec<SpellcheckMenuItemSpec> {
    let mut items = Vec::new();
    if suggestions.is_empty() {
        items.push(SpellcheckMenuItemSpec {
            label: LABEL_RU_NO_SUGGESTIONS.to_string(),
            enabled: false,
            focused: true,
            separator_before: false,
        });
    } else {
        for (idx, word) in suggestions.iter().take(MAX_SUGGESTIONS).enumerate() {
            items.push(SpellcheckMenuItemSpec {
                label: word.clone(),
                enabled: true,
                focused: idx == 0,
                separator_before: false,
            });
        }
    }
    items.push(SpellcheckMenuItemSpec {
        label: LABEL_RU_ADD_TO_DICTIONARY.to_string(),
        enabled: true,
        focused: false,
        separator_before: true,
    });
    items
}

pub fn validate_input_spellcheck_boundaries(
    snowflake: u64,
    chars: usize,
    files: usize,
    bytes: u64,
    offset: u32,
    vol: u16,
    badge: u32,
) -> Result<String, InputSpellcheckError> {
    if snowflake == 0 {
        return Err(InputSpellcheckError::InvalidSnowflake(0));
    }
    if chars > MAX_MESSAGE_CHARS {
        return Err(InputSpellcheckError::MessageTooLong(chars));
    }
    if files > MAX_ATTACHMENTS_COUNT
        || bytes > MAX_ATTACHMENT_BYTES
        || offset > MAX_SEARCH_OFFSET
        || vol > MAX_VOLUME_PERCENT
    {
        return Err(InputSpellcheckError::BoundaryViolation("workload_limit"));
    }
    Ok(if badge > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        badge.to_string()
    })
}

pub fn verify_input_spellcheck_access(
    permissions: u64,
    age_gated_channel: bool,
    user_age_verified: bool,
) -> Result<(), InputSpellcheckError> {
    if (permissions & PERM_VIEW_CHANNEL) == 0 {
        return Err(InputSpellcheckError::MissingPermission("VIEW_CHANNEL"));
    }
    if (permissions & PERM_SEND_MESSAGES) == 0 {
        return Err(InputSpellcheckError::MissingPermission("SEND_MESSAGES"));
    }
    if age_gated_channel && !user_age_verified {
        return Err(InputSpellcheckError::AgeGatedSpaceRestricted);
    }
    Ok(())
}

pub fn classify_spellcheck_http_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<DictionarySyncDirective, InputSpellcheckError> {
    match status {
        200 | 204 => Ok(DictionarySyncDirective::Ready),
        202 => Ok(DictionarySyncDirective::SkeletonWarmingUp {
            retry_after_ms: retry_after_ms.max(COORDINATE_DEBOUNCE_MS),
        }),
        429 => Ok(DictionarySyncDirective::RateLimited {
            retry_after_ms: retry_after_ms.max(250),
        }),
        code => Err(InputSpellcheckError::HttpFailure {
            status: code,
            retry_after_ms,
        }),
    }
}

#[must_use]
pub fn redact_input_spellcheck_log(raw_log: &str) -> String {
    let mut out = String::with_capacity(raw_log.len());
    for part in raw_log.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        let stripped = part.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';'));
        let secret = stripped.starts_with("mfa.")
            || stripped.starts_with("Bearer")
            || stripped.contains("token=")
            || (stripped.matches('.').count() == 2 && stripped.len() >= 40);
        out.push_str(if secret { "[REDACTED]" } else { part });
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sub_1006b_1_oracle_tokens_states_and_d03_markers() {
        assert_eq!(INPUT_DECORATOR_CONTAINER, "Input Decorator");
        assert_eq!(INPUT_DECORATOR_DIMENSIONS, "TextView overlay");
        assert_eq!(INPUT_DECORATOR_PADDING_PX, 0);
        assert_eq!(INPUT_DECORATOR_SPACING_PX, 0);
        assert_eq!(TOKEN_BACKGROUND_HEX, "#383a40");
        assert_eq!(TOKEN_SURFACE_HEX, "#2b2d31");
        assert_eq!(TOKEN_TEXT_PRIMARY_HEX, "#ffffff");
        assert_eq!(TOKEN_WAVY_UNDERLINE_HEX, "#f23f43");
        assert_eq!(TOKEN_BORDER_HEX, "#3f4147");
        assert_eq!(WAVY_UNDERLINE_ELEMENT, "wavy-underline");
        assert_eq!(COORDINATE_DEBOUNCE_MS, 150);
        assert_eq!(
            WavyUnderlineInteractiveState::resolve(false, false).as_str(),
            "red-wave"
        );
        assert_eq!(
            WavyUnderlineInteractiveState::resolve(false, true).as_str(),
            "pointer"
        );
        assert_eq!(
            WavyUnderlineInteractiveState::resolve(true, false).as_str(),
            "context menu"
        );
        assert_eq!(D03_MENU_CLASS, "menu_c1e9c4");
        assert_eq!(D03_CONTEXT_MENU_OPEN_EVENT, "CONTEXT_MENU_OPEN");
    }

    #[test]
    fn test_sub_1006b_2_debounced_150ms_wavy_underline_geometry_and_hover() {
        let mut dec = InputSpellcheckDecorator::new();
        dec.set_viewport(1_280, 720, 40);
        dec.on_text_input("Привет wrold\nи helo!", 20, 1_000);
        assert!(dec.is_debounce_pending() && !dec.flush_debounced_coordinates(1_100));
        dec.on_text_input("Привет wrold\nи helo!", 20, 1_120);
        assert!(!dec.flush_debounced_coordinates(1_269) && dec.flush_debounced_coordinates(1_270));
        assert_eq!(dec.segments().len(), 2);
        let first = &dec.segments()[0];
        assert_eq!(first.word, "wrold");
        assert_eq!((first.line_row, first.start_col), (0, 7));
        assert_eq!(first.color_hex, "#f23f43");
        assert_eq!(first.interactive_state.as_str(), "red-wave");
        assert!(first.vertices.len() >= 4);
        assert!(first.vertices.iter().any(|p| p.y_px < first.baseline_y_px));
        assert!(first.vertices.iter().any(|p| p.y_px > first.baseline_y_px));
        assert_eq!(dec.segments()[1].line_row, 1);
        let (hx, hy) = (first.x_px + 4.0, first.y_px + 4.0);
        assert_eq!(dec.hover_at_point(hx, hy), Some(0));
        assert_eq!(dec.segments()[0].interactive_state.as_str(), "pointer");
    }

    #[test]
    fn test_sub_1006b_3_right_click_popup_menu_and_one_click_word_replace() {
        let mut dec = InputSpellcheckDecorator::new();
        dec.set_viewport(640, 480, 80);
        dec.on_text_input("Привет wrold и helo!", 12, 2_000);
        assert!(dec.flush_debounced_coordinates(2_150));
        let target_byte = dec.segments()[0].start_byte + 1;
        let popup = dec
            .open_context_menu_on_right_click(target_byte, 620, 470)
            .unwrap_or_else(|e| panic!("expected popup: {e:?}"));
        assert_eq!(popup.target_word, "wrold");
        assert_eq!(popup.wavy_state, "context menu");
        assert!(popup.x_px + popup.width_px <= 640 && popup.y_px + popup.height_px <= 480);
        assert_eq!(dec.navigate_context_menu("ArrowDown"), Some(1));
        assert_eq!(dec.navigate_context_menu("ArrowUp"), Some(0));
        let replaced = dec
            .replace_misspelled_word_one_click(0)
            .unwrap_or_else(|e| panic!("replace failed: {e:?}"));
        assert_eq!(replaced.updated_text, "Привет world и helo!");
        assert_eq!(replaced.ipc_channel, IPC_SPELLCHECK_REPLACE_MISSPELLING);
        assert_eq!(replaced.remaining_misspellings, 1);
        let next_byte = dec.segments()[0].start_byte;
        assert!(
            dec.open_context_menu_on_right_click(next_byte, 50, 50)
                .is_ok()
        );
        assert_eq!(
            dec.add_active_word_to_dictionary(),
            Ok(IPC_SPELLCHECK_SET_LEARNED_WORDS)
        );
        assert_eq!(dec.segments().len(), 0);
        assert_eq!(
            dec.remove_word_from_dictionary("helo"),
            IPC_SPELLCHECK_SET_LEARNED_WORDS
        );
        assert_eq!(dec.segments().len(), 1);
    }

    #[test]
    fn test_sub_1006b_4_nine_policy_dimensions_and_telemetry_redaction() {
        assert!(validate_input_spellcheck_boundaries(0, 10, 0, 0, 0, 100, 0).is_err());
        let ok_bound = validate_input_spellcheck_boundaries(
            u64::MAX,
            2_000,
            10,
            25 * 1024 * 1024,
            9_975,
            200,
            150,
        );
        assert_eq!(ok_bound, Ok("99+".to_string()));
        let err_send = verify_input_spellcheck_access(PERM_VIEW_CHANNEL, false, true);
        assert_eq!(
            err_send,
            Err(InputSpellcheckError::MissingPermission("SEND_MESSAGES"))
        );
        let err_age =
            verify_input_spellcheck_access(PERM_VIEW_CHANNEL | PERM_SEND_MESSAGES, true, false);
        assert_eq!(err_age, Err(InputSpellcheckError::AgeGatedSpaceRestricted));
        let dir_202 = classify_spellcheck_http_status(202, 100);
        assert_eq!(
            dir_202,
            Ok(DictionarySyncDirective::SkeletonWarmingUp {
                retry_after_ms: 150
            })
        );
        for code in [400u16, 401, 403, 404, 413, 500, 503] {
            assert!(classify_spellcheck_http_status(code, 0).is_err());
        }
        let redacted =
            redact_input_spellcheck_log("spellcheck auth=Bearer mfa.secret_token_9876543210");
        assert!(
            !redacted.contains("mfa.secret_token_9876543210") && redacted.contains("[REDACTED]")
        );
    }
}
