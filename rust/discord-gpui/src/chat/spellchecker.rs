//! Нативная проверка орфографии в поле ввода (`hunspell` / `libenchant`), дебаунс `150 мс`,
//! интерактивное волнистое подчёркивание `#f23f43` (`red-wavy-underline`) и контекстное меню
//! вариантов замены при клике ПКМ (`TASK-1006` / `TASK-1006A` / `TASK-1006B`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeSet;

pub const SPELLCHECK_CONTAINER: &str = "Text Input Context";
pub const SPELLCHECK_DIMENSIONS: &str = "100%";
pub const SPELLCHECK_PADDING_PX: u32 = 0;
pub const SPELLCHECK_SPACING_PX: u32 = 0;
pub const TOKEN_BACKGROUND: &str = "#383a40";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT_WAVY_RED: &str = "#f23f43";
pub const TOKEN_BORDER: &str = "#3f4147";

pub const SPELLCHECK_DEBOUNCE_MS: u64 = 150;
pub const MAX_SUGGESTIONS: usize = 5;
pub const MAX_EDIT_DISTANCE: usize = 2;

pub const IPC_SPELLCHECK_GET_AVAILABLE_DICTIONARIES: &str =
    "DISCORD_SPELLCHECK_GET_AVAILABLE_DICTIONARIES";
pub const IPC_SPELLCHECK_REPLACE_MISSPELLING: &str = "DISCORD_SPELLCHECK_REPLACE_MISSPELLING";
pub const IPC_SPELLCHECK_RESULT: &str = "DISCORD_SPELLCHECK_RESULT";
pub const IPC_SPELLCHECK_SET_LANGUAGES: &str = "DISCORD_SPELLCHECK_SET_LANGUAGES";
pub const IPC_SPELLCHECK_SET_LEARNED_WORDS: &str = "DISCORD_SPELLCHECK_SET_LEARNED_WORDS";
pub const IPC_SPELLCHECK_SET_LOCALE: &str = "DISCORD_SPELLCHECK_SET_LOCALE";

pub const SPELLCHECK_IPC_CHANNELS: [&str; 6] = [
    IPC_SPELLCHECK_GET_AVAILABLE_DICTIONARIES,
    IPC_SPELLCHECK_REPLACE_MISSPELLING,
    IPC_SPELLCHECK_RESULT,
    IPC_SPELLCHECK_SET_LANGUAGES,
    IPC_SPELLCHECK_SET_LEARNED_WORDS,
    IPC_SPELLCHECK_SET_LOCALE,
];

pub const I18N_KEY_SPELLCHECK: &str = "TKkotf";
pub const I18N_KEY_ADD_TO_DICTIONARY: &str = "HJmG1G";
pub const I18N_KEY_REMOVE_FROM_DICTIONARY: &str = "xXqIX0";
pub const LABEL_RU_SPELLCHECK: &str = "Проверка правописания";
pub const LABEL_RU_ADD_TO_DICTIONARY: &str = "Добавить в словарь";
pub const LABEL_RU_REMOVE_FROM_DICTIONARY: &str = "Удалить из словаря";

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpellcheckError {
    NulByteInContent,
    EmptyAffFile,
    MissingAffSetDirective,
    EmptyDicFile,
    InvalidDicHeader(String),
    DicEntryCountMismatch { declared: usize, actual: usize },
    InvalidDicEntry(String),
    InvalidSnowflake(u64),
    MessageTooLong(usize),
    BoundaryViolation(&'static str),
    AccessDenied(&'static str),
    AgeGatedSpaceRestricted,
    NoActiveContextMenu,
    SuggestionIndexOutOfBounds(usize),
    HttpFailure { status: u16, retry_after_ms: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MisspelledWordTrigger {
    Default,
    Hover,
    Active,
}

impl MisspelledWordTrigger {
    #[must_use]
    pub const fn state_label(self) -> &'static str {
        match self {
            Self::Default => "red-wavy-underline",
            Self::Hover => "pointer",
            Self::Active => "open suggestions",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordCheckTrigger {
    Default,
    Hover,
    Active,
}

impl WordCheckTrigger {
    #[must_use]
    pub const fn state_label(self) -> &'static str {
        match self {
            Self::Default => "valid",
            Self::Hover => "checking",
            Self::Active => "misspelled",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WavyUnderlineTrigger {
    Default,
    Hover,
    Active,
}

impl WavyUnderlineTrigger {
    #[must_use]
    pub const fn state_label(self) -> &'static str {
        match self {
            Self::Default => "red-wave",
            Self::Hover => "pointer",
            Self::Active => "context menu",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellToken {
    pub text: String,
    pub start_byte: usize,
    pub end_byte: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedHunspellPair {
    pub encoding: String,
    pub wordchars: String,
    pub declared_count: usize,
    pub words: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MisspelledSpan {
    pub word: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub underline_color: &'static str,
    pub trigger: MisspelledWordTrigger,
    pub wavy_trigger: WavyUnderlineTrigger,
    pub word_check_trigger: WordCheckTrigger,
    pub suggestions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpellcheckContextMenu {
    pub span_index: usize,
    pub target_word: String,
    pub suggestions: Vec<String>,
    pub interactive_state: &'static str,
    pub wavy_state: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReplacementOutcome {
    pub updated_text: String,
    pub ipc_channel: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestDictionaryDirective {
    Ready,
    WarmingUpSkeleton { retry_after_ms: u64 },
    RateLimited { retry_after_ms: u64 },
}

pub fn validate_and_parse_hunspell(
    aff_text: &str,
    dic_text: &str,
) -> Result<ParsedHunspellPair, SpellcheckError> {
    if aff_text.contains('\0') || dic_text.contains('\0') {
        return Err(SpellcheckError::NulByteInContent);
    }
    if aff_text.trim().is_empty() {
        return Err(SpellcheckError::EmptyAffFile);
    }
    let (mut encoding, mut wordchars) = (String::new(), String::new());
    for raw_line in aff_text.lines() {
        let line = raw_line.trim_start_matches('\u{feff}').trim();
        if let Some(rest) = line.strip_prefix("SET ") {
            encoding = rest.trim().to_string();
        } else if let Some(rest) = line.strip_prefix("WORDCHARS ") {
            wordchars = rest.trim().to_string();
        }
    }
    if encoding.is_empty() {
        return Err(SpellcheckError::MissingAffSetDirective);
    }
    let (declared_count, words) = parse_dic_entries(dic_text)?;
    Ok(ParsedHunspellPair {
        encoding,
        wordchars,
        declared_count,
        words,
    })
}

fn parse_dic_entries(dic_text: &str) -> Result<(usize, Vec<String>), SpellcheckError> {
    let mut lines = dic_text.lines();
    let header = lines
        .next()
        .ok_or(SpellcheckError::EmptyDicFile)?
        .trim_start_matches('\u{feff}')
        .trim();
    let declared = header
        .parse::<usize>()
        .map_err(|_| SpellcheckError::InvalidDicHeader(header.to_string()))?;
    if declared == 0 {
        return Err(SpellcheckError::InvalidDicHeader("0".to_string()));
    }
    let mut words = Vec::new();
    for line in lines.map(str::trim).filter(|l| !l.is_empty()) {
        let stem = line.split(['/', ' ', '\t']).next().unwrap_or("");
        if stem.is_empty()
            || !stem
                .chars()
                .all(|c| c.is_alphabetic() || c == '\'' || c == '-')
        {
            return Err(SpellcheckError::InvalidDicEntry(line.to_string()));
        }
        words.push(stem.to_string());
    }
    if words.len() != declared {
        return Err(SpellcheckError::DicEntryCountMismatch {
            declared,
            actual: words.len(),
        });
    }
    Ok((declared, words))
}

#[must_use]
pub fn tokenize_input_text(text: &str) -> Vec<SpellToken> {
    let positions: Vec<(usize, char)> = text.char_indices().collect();
    let (mut tokens, mut index) = (Vec::new(), 0);
    while index < positions.len() {
        if let Some(next_idx) = skip_non_word_construct(&positions, index) {
            index = next_idx;
            continue;
        }
        let (start_byte, ch) = positions[index];
        if !ch.is_alphabetic() {
            index += 1;
            continue;
        }
        let mut chunk_end = index;
        while chunk_end < positions.len() && !positions[chunk_end].1.is_whitespace() {
            chunk_end += 1;
        }
        let chunk_end_byte = positions.get(chunk_end).map_or(text.len(), |(b, _)| *b);
        let chunk = &text[start_byte..chunk_end_byte];
        if chunk.contains("://") || chunk.contains('@') {
            index = chunk_end;
            continue;
        }
        let word_end = scan_word_end(&positions, index + 1);
        let end_byte = positions.get(word_end).map_or(text.len(), |(b, _)| *b);
        tokens.push(SpellToken {
            text: text[start_byte..end_byte].to_string(),
            start_byte,
            end_byte,
        });
        index = word_end;
    }
    tokens
}

fn skip_non_word_construct(positions: &[(usize, char)], index: usize) -> Option<usize> {
    let ch = positions[index].1;
    if ch.is_whitespace() {
        return Some(index + 1);
    }
    if ch == '`' {
        let mut idx = index + 1;
        while idx < positions.len() && positions[idx].1 != '`' {
            idx += 1;
        }
        return Some(if idx < positions.len() { idx + 1 } else { idx });
    }
    let next = positions.get(index + 1).map(|(_, c)| *c);
    let is_tag = ch == '<'
        && (matches!(next, Some('@' | '#' | ':'))
            || (next == Some('a') && positions.get(index + 2).is_some_and(|(_, c)| *c == ':')));
    if is_tag {
        let mut idx = index + 2;
        while idx < positions.len() && positions[idx].1 != '>' && !positions[idx].1.is_whitespace()
        {
            idx += 1;
        }
        return Some(if idx < positions.len() && positions[idx].1 == '>' {
            idx + 1
        } else {
            idx
        });
    }
    (ch == ':').then(|| skip_shortcode_emoji(positions, index))?
}

fn skip_shortcode_emoji(positions: &[(usize, char)], start: usize) -> Option<usize> {
    let (mut idx, mut len) = (start + 1, 0usize);
    while idx < positions.len() {
        let ch = positions[idx].1;
        if ch == ':' {
            return (len >= 2).then_some(idx + 1);
        }
        if !(ch.is_ascii_alphanumeric() || ch == '_' || ch == '+' || ch == '-') {
            return None;
        }
        len += 1;
        idx += 1;
    }
    None
}

fn scan_word_end(positions: &[(usize, char)], mut cursor: usize) -> usize {
    while cursor < positions.len() {
        let current = positions[cursor].1;
        let next_is_alpha = positions
            .get(cursor + 1)
            .is_some_and(|(_, next)| next.is_alphabetic());
        if current.is_alphabetic() || ((current == '\'' || current == '-') && next_is_alpha) {
            cursor += 1;
        } else {
            break;
        }
    }
    cursor
}

#[must_use]
pub fn damerau_levenshtein_distance(left: &str, right: &str) -> usize {
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
pub fn suggest_corrections(word: &str, lexicon: &BTreeSet<String>) -> Vec<String> {
    let query_lower = word.to_lowercase();
    if query_lower.is_empty() || MAX_SUGGESTIONS == 0 {
        return Vec::new();
    }
    let (query_first, query_len) = (query_lower.chars().next(), query_lower.chars().count());
    let first_upper = word.chars().next().is_some_and(char::is_uppercase);
    let mut ranked: Vec<(usize, usize, usize, String)> = Vec::new();
    for candidate in lexicon {
        let dist = damerau_levenshtein_distance(&query_lower, candidate);
        if dist == 0 || dist > MAX_EDIT_DISTANCE {
            continue;
        }
        let same_first = usize::from(candidate.chars().next() != query_first);
        let len_diff = candidate.chars().count().abs_diff(query_len);
        let formatted = if first_upper {
            let mut chars = candidate.chars();
            chars.next().map_or_else(String::new, |f| {
                f.to_uppercase().collect::<String>() + chars.as_str()
            })
        } else {
            candidate.clone()
        };
        ranked.push((dist, same_first, len_diff, formatted));
    }
    ranked.sort_by(|a, b| a.cmp(b));
    ranked
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, _, _, w)| w)
        .collect()
}

#[derive(Default)]
pub struct SpellcheckInputController {
    lexicon: BTreeSet<String>,
    learned_words: BTreeSet<String>,
    text: String,
    pending_debounce: bool,
    debounce_deadline_ms: u64,
    misspelled_spans: Vec<MisspelledSpan>,
    active_menu: Option<SpellcheckContextMenu>,
}

impl SpellcheckInputController {
    #[must_use]
    pub fn new() -> Self {
        let lexicon = BUILTIN_WORDS_EN
            .iter()
            .chain(BUILTIN_WORDS_RU.iter())
            .map(|w| (*w).to_lowercase())
            .collect();
        Self {
            lexicon,
            ..Self::default()
        }
    }

    pub fn load_hunspell_pair(&mut self, aff_text: &str, dic_text: &str) -> bool {
        let Ok(parsed) = validate_and_parse_hunspell(aff_text, dic_text) else {
            return false;
        };
        for word in parsed.words {
            self.lexicon.insert(word.to_lowercase());
        }
        true
    }

    #[must_use]
    pub fn check_word(&self, word: &str) -> bool {
        let lower = word.to_lowercase();
        self.lexicon.contains(&lower) || self.learned_words.contains(&lower)
    }

    #[must_use]
    pub fn suggest(&self, word: &str) -> Vec<String> {
        let mut combined = self.lexicon.clone();
        for learned in &self.learned_words {
            combined.insert(learned.clone());
        }
        suggest_corrections(word, &combined)
    }

    pub fn on_text_input(&mut self, next_text: &str, timestamp_ms: u64) {
        self.text = next_text.to_string();
        self.debounce_deadline_ms = timestamp_ms.saturating_add(SPELLCHECK_DEBOUNCE_MS);
        self.pending_debounce = true;
        self.active_menu = None;
    }

    #[must_use]
    pub const fn is_debounce_pending(&self) -> bool {
        self.pending_debounce
    }

    pub fn flush_debounced(&mut self, now_ms: u64) -> bool {
        if !self.pending_debounce || now_ms < self.debounce_deadline_ms {
            return false;
        }
        self.recompute_spans();
        self.pending_debounce = false;
        true
    }

    fn recompute_spans(&mut self) {
        let mut spans = Vec::new();
        for token in tokenize_input_text(&self.text) {
            if !self.check_word(&token.text) {
                let suggestions = self.suggest(&token.text);
                spans.push(MisspelledSpan {
                    word: token.text,
                    start_byte: token.start_byte,
                    end_byte: token.end_byte,
                    underline_color: TOKEN_ACCENT_WAVY_RED,
                    trigger: MisspelledWordTrigger::Default,
                    wavy_trigger: WavyUnderlineTrigger::Default,
                    word_check_trigger: WordCheckTrigger::Active,
                    suggestions,
                });
            }
        }
        self.misspelled_spans = spans;
    }

    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub fn misspelled_spans(&self) -> &[MisspelledSpan] {
        &self.misspelled_spans
    }

    pub fn hover_span(&mut self, span_index: usize) -> Option<&MisspelledSpan> {
        let span = self.misspelled_spans.get_mut(span_index)?;
        span.trigger = MisspelledWordTrigger::Hover;
        span.wavy_trigger = WavyUnderlineTrigger::Hover;
        Some(&*span)
    }

    pub fn open_context_menu_on_right_click(
        &mut self,
        byte_offset: usize,
        _cursor_x: i32,
        _cursor_y: i32,
    ) -> Option<SpellcheckContextMenu> {
        let span_index = self
            .misspelled_spans
            .iter()
            .position(|s| byte_offset >= s.start_byte && byte_offset <= s.end_byte)?;
        let span = self.misspelled_spans.get_mut(span_index)?;
        span.trigger = MisspelledWordTrigger::Active;
        span.wavy_trigger = WavyUnderlineTrigger::Active;
        let menu = SpellcheckContextMenu {
            span_index,
            target_word: span.word.clone(),
            suggestions: span.suggestions.clone(),
            interactive_state: span.trigger.state_label(),
            wavy_state: span.wavy_trigger.state_label(),
        };
        self.active_menu = Some(menu.clone());
        Some(menu)
    }

    #[rustfmt::skip]
    pub fn select_context_menu_suggestion(
        &mut self,
        suggestion_index: usize,
    ) -> Result<ReplacementOutcome, SpellcheckError> {
        let menu = self.active_menu.take().ok_or(SpellcheckError::NoActiveContextMenu)?;
        let replacement = menu
            .suggestions
            .get(suggestion_index)
            .cloned()
            .ok_or(SpellcheckError::SuggestionIndexOutOfBounds(suggestion_index))?;
        let span = self
            .misspelled_spans
            .get(menu.span_index)
            .cloned()
            .ok_or(SpellcheckError::NoActiveContextMenu)?;
        self.text.replace_range(span.start_byte..span.end_byte, &replacement);
        self.recompute_spans();
        Ok(ReplacementOutcome {
            updated_text: self.text.clone(),
            ipc_channel: IPC_SPELLCHECK_REPLACE_MISSPELLING,
        })
    }

    pub fn add_target_word_to_dictionary(&mut self) -> Result<&'static str, SpellcheckError> {
        let menu = self
            .active_menu
            .take()
            .ok_or(SpellcheckError::NoActiveContextMenu)?;
        self.learned_words.insert(menu.target_word.to_lowercase());
        self.recompute_spans();
        Ok(IPC_SPELLCHECK_SET_LEARNED_WORDS)
    }
}

pub fn validate_spellcheck_workload_boundaries(
    channel_snowflake: u64,
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_percent: u16,
    badge_count: u32,
) -> Result<String, SpellcheckError> {
    if channel_snowflake == 0 {
        return Err(SpellcheckError::InvalidSnowflake(0));
    }
    if message_chars > MAX_MESSAGE_CHARS {
        return Err(SpellcheckError::MessageTooLong(message_chars));
    }
    if attachments_count > MAX_ATTACHMENTS_COUNT
        || attachment_bytes > MAX_ATTACHMENT_BYTES
        || search_offset > MAX_SEARCH_OFFSET
        || volume_percent > MAX_VOLUME_PERCENT
    {
        return Err(SpellcheckError::BoundaryViolation("workload_limit"));
    }
    Ok(if badge_count > MAX_BADGE_CAP {
        "99+".to_string()
    } else {
        badge_count.to_string()
    })
}

pub fn check_spellcheck_channel_access(
    permissions: u64,
    age_gated_channel: bool,
    user_age_verified: bool,
) -> Result<(), SpellcheckError> {
    if permissions & PERM_VIEW_CHANNEL == 0 {
        return Err(SpellcheckError::AccessDenied("missing VIEW_CHANNEL"));
    }
    if permissions & PERM_SEND_MESSAGES == 0 {
        return Err(SpellcheckError::AccessDenied("missing SEND_MESSAGES"));
    }
    if age_gated_channel && !user_age_verified {
        return Err(SpellcheckError::AgeGatedSpaceRestricted);
    }
    Ok(())
}

pub fn handle_dictionary_rest_status(
    status: u16,
    retry_after_ms: u64,
) -> Result<RestDictionaryDirective, SpellcheckError> {
    match status {
        200 | 204 => Ok(RestDictionaryDirective::Ready),
        202 => Ok(RestDictionaryDirective::WarmingUpSkeleton {
            retry_after_ms: retry_after_ms.max(150),
        }),
        429 => Ok(RestDictionaryDirective::RateLimited {
            retry_after_ms: retry_after_ms.max(250),
        }),
        other => Err(SpellcheckError::HttpFailure {
            status: other,
            retry_after_ms,
        }),
    }
}

#[must_use]
pub fn redact_spellcheck_telemetry(raw_log: &str) -> String {
    let mut output = String::with_capacity(raw_log.len());
    for token in raw_log.split_whitespace() {
        if !output.is_empty() {
            output.push(' ');
        }
        let clean = token.trim_matches(|c: char| c == '"' || c == '\'' || c == ',' || c == ';');
        let is_secret = clean.starts_with("mfa.")
            || clean.starts_with("Bearer")
            || clean.contains("token=")
            || (clean.matches('.').count() == 2 && clean.len() >= 40);
        output.push_str(if is_secret { "[REDACTED]" } else { token });
    }
    output
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE_AFF: &str = "SET UTF-8\nFLAG UTF-8\nWORDCHARS '-\n";
    const SAMPLE_DIC: &str = "6\nhello\nworld\nпривет\nмир\ndon't\nwell-known\n";

    #[test]
    fn test_sub_1006_1_oracle_tokens_states_and_ipc_channels() {
        assert_eq!(SPELLCHECK_CONTAINER, "Text Input Context");
        assert_eq!(SPELLCHECK_DIMENSIONS, "100%");
        assert_eq!(TOKEN_BACKGROUND, "#383a40");
        assert_eq!(TOKEN_ACCENT_WAVY_RED, "#f23f43");
        assert_eq!(SPELLCHECK_DEBOUNCE_MS, 150);
        assert_eq!(MAX_SUGGESTIONS, 5);
        assert_eq!(
            MisspelledWordTrigger::Default.state_label(),
            "red-wavy-underline"
        );
        assert_eq!(MisspelledWordTrigger::Hover.state_label(), "pointer");
        assert_eq!(
            MisspelledWordTrigger::Active.state_label(),
            "open suggestions"
        );
        assert_eq!(WordCheckTrigger::Default.state_label(), "valid");
        assert_eq!(WordCheckTrigger::Hover.state_label(), "checking");
        assert_eq!(WordCheckTrigger::Active.state_label(), "misspelled");
        assert_eq!(WavyUnderlineTrigger::Default.state_label(), "red-wave");
        assert_eq!(WavyUnderlineTrigger::Hover.state_label(), "pointer");
        assert_eq!(WavyUnderlineTrigger::Active.state_label(), "context menu");
        assert_eq!(SPELLCHECK_IPC_CHANNELS.len(), 6);
        assert_eq!(I18N_KEY_SPELLCHECK, "TKkotf");
    }

    #[test]
    fn test_sub_1006_2_hunspell_validation_tokenizer_and_suggestions() {
        assert!(validate_and_parse_hunspell(SAMPLE_AFF, SAMPLE_DIC).is_ok());
        assert_eq!(
            validate_and_parse_hunspell("FLAG UTF-8\n", SAMPLE_DIC),
            Err(SpellcheckError::MissingAffSetDirective)
        );
        assert_eq!(
            validate_and_parse_hunspell(SAMPLE_AFF, "1\nhel\0lo\n"),
            Err(SpellcheckError::NulByteInContent)
        );
        let text = "Привет, wrold! https://discord.com/a user@mail.org `fn_code()` <@123> <#456> <:CoP:3527262836281581568> :smile: don't well-known";
        let words: Vec<String> = tokenize_input_text(text)
            .into_iter()
            .map(|t| t.text)
            .collect();
        assert_eq!(words, vec!["Привет", "wrold", "don't", "well-known"]);
        let ctrl = SpellcheckInputController::new();
        assert!(ctrl.check_word("hello") && ctrl.check_word("Привет") && !ctrl.check_word("wrold"));
        assert_eq!(
            ctrl.suggest("wrold").first().map(String::as_str),
            Some("world")
        );
    }

    #[test]
    fn test_sub_1006_3_debounce_150ms_wavy_underline_and_context_menu() {
        let mut ctrl = SpellcheckInputController::new();
        ctrl.on_text_input("Привет wrold и helo!", 1_000);
        assert!(ctrl.is_debounce_pending() && !ctrl.flush_debounced(1_100));
        ctrl.on_text_input("Привет wrold и helo!", 1_120);
        assert!(!ctrl.flush_debounced(1_269) && ctrl.flush_debounced(1_270));
        assert_eq!(ctrl.misspelled_spans().len(), 2);
        let target_byte = ctrl.misspelled_spans()[0].start_byte + 1;
        assert_eq!(
            ctrl.hover_span(0).map(|s| s.trigger.state_label()),
            Some("pointer")
        );
        let menu = ctrl.open_context_menu_on_right_click(target_byte, 200, 400);
        assert!(
            menu.is_some_and(|m| m.interactive_state == "open suggestions"
                && m.wavy_state == "context menu"
                && m.suggestions[0] == "world")
        );
        assert!(ctrl.select_context_menu_suggestion(0).is_ok());
        assert_eq!(ctrl.text(), "Привет world и helo!");
        let rem_byte = ctrl.misspelled_spans()[0].start_byte;
        let _ = ctrl.open_context_menu_on_right_click(rem_byte, 220, 400);
        assert_eq!(
            ctrl.add_target_word_to_dictionary(),
            Ok(IPC_SPELLCHECK_SET_LEARNED_WORDS)
        );
        assert_eq!(ctrl.misspelled_spans().len(), 0);
    }

    #[test]
    fn test_sub_1006_4_nine_policy_dimensions_and_secret_redaction() {
        assert_eq!(
            validate_spellcheck_workload_boundaries(0, 10, 0, 0, 0, 100, 1),
            Err(SpellcheckError::InvalidSnowflake(0))
        );
        assert_eq!(
            validate_spellcheck_workload_boundaries(
                u64::MAX,
                2_000,
                10,
                25 * 1024 * 1024,
                9_975,
                200,
                120
            ),
            Ok("99+".to_string())
        );
        assert_eq!(
            check_spellcheck_channel_access(PERM_VIEW_CHANNEL, false, true),
            Err(SpellcheckError::AccessDenied("missing SEND_MESSAGES"))
        );
        assert_eq!(
            handle_dictionary_rest_status(202, 180),
            Ok(RestDictionaryDirective::WarmingUpSkeleton {
                retry_after_ms: 180
            })
        );
        let redacted = redact_spellcheck_telemetry(
            "spellcheck auth=Bearer mfa.secret_token_1234567890 token=abc",
        );
        assert!(
            !redacted.contains("mfa.secret_token_1234567890") && redacted.contains("[REDACTED]")
        );
    }
}
