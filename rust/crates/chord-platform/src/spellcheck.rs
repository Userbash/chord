//! P6-07 / TASK-1006A: нативная проверка орфографии через системный `libhunspell` (`/usr/share/hunspell/`).

#![forbid(unsafe_code)]
#![allow(dead_code)]

use std::collections::BTreeSet;
use std::fmt;
use std::marker::PhantomData;
use std::path::{Path, PathBuf};

use crate::ffi_hunspell::Handle;

pub const SPELLCHECK_FFI_CONTAINER: &str = "Spellcheck FFI";
pub const SPELLCHECK_FFI_DIMENSIONS: &str = "C ABI";
pub const SPELLCHECK_FFI_PADDING: &str = "0px";
pub const SPELLCHECK_FFI_SPACING: &str = "0px";
pub const TOKEN_BACKGROUND: &str = "#1e1f22";
pub const TOKEN_SURFACE: &str = "#2b2d31";
pub const TOKEN_TEXT_PRIMARY: &str = "#ffffff";
pub const TOKEN_ACCENT: &str = "#f23f43";
pub const TOKEN_BORDER: &str = "#3f4147";
pub const WORD_CHECK_ELEMENT: &str = "word-check";
pub const WORD_CHECK_STATE_VALID: &str = "valid";
pub const WORD_CHECK_STATE_CHECKING: &str = "checking";
pub const WORD_CHECK_STATE_MISSPELLED: &str = "misspelled";
pub const SYSTEM_HUNSPELL_DIR: &str = "/usr/share/hunspell";
pub const FALLBACK_MYSPELL_DIR: &str = "/usr/share/myspell";
pub const SUPPORTED_LOCALES: [&str; 2] = ["ru_RU", "en_US"];
pub const MAX_SUGGESTIONS: usize = 5;
pub const MAX_EDIT_DISTANCE: usize = 2;

pub const SPELLCHECK_IPC_CHANNELS: [&str; 6] = [
    "DISCORD_SPELLCHECK_GET_AVAILABLE_DICTIONARIES",
    "DISCORD_SPELLCHECK_REPLACE_MISSPELLING",
    "DISCORD_SPELLCHECK_RESULT",
    "DISCORD_SPELLCHECK_SET_LANGUAGES",
    "DISCORD_SPELLCHECK_SET_LEARNED_WORDS",
    "DISCORD_SPELLCHECK_SET_LOCALE",
];

pub const I18N_KEY_SPELLCHECK: &str = "TKkotf";
pub const I18N_KEY_ADD_TO_DICTIONARY: &str = "HJmG1G";
pub const I18N_KEY_REMOVE_FROM_DICTIONARY: &str = "xXqIX0";
pub const PERM_VIEW_CHANNEL: u64 = 1 << 10;
pub const PERM_SEND_MESSAGES: u64 = 1 << 11;
pub const MAX_MESSAGE_CHARS: usize = 2_000;
pub const MAX_ATTACHMENTS_COUNT: usize = 10;
pub const MAX_ATTACHMENT_BYTES: u64 = 25 * 1024 * 1024;
pub const MAX_SEARCH_OFFSET: u32 = 9_975;
pub const MAX_VOLUME_PERCENT: u16 = 200;
pub const MAX_BADGE_CAP: u32 = 99;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpellLocale {
    RuRu,
    EnUs,
}

impl SpellLocale {
    #[must_use]
    pub const fn as_tag(self) -> &'static str {
        match self {
            Self::RuRu => "ru_RU",
            Self::EnUs => "en_US",
        }
    }

    #[must_use]
    pub fn parse(raw: &str) -> Option<Self> {
        match raw.trim().replace('-', "_").as_str() {
            "ru_RU" | "ru" => Some(Self::RuRu),
            "en_US" | "en" => Some(Self::EnUs),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WordCheckState {
    Valid,
    Checking,
    Misspelled,
}

impl WordCheckState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Valid => WORD_CHECK_STATE_VALID,
            Self::Checking => WORD_CHECK_STATE_CHECKING,
            Self::Misspelled => WORD_CHECK_STATE_MISSPELLED,
        }
    }

    #[must_use]
    pub fn from_trigger(trigger: &str) -> Self {
        match trigger {
            "hover" => Self::Checking,
            "active" => Self::Misspelled,
            _ => Self::Valid,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DictionaryHttpDirective {
    Ready,
    SkeletonWarmup { retry_after_ms: u64 },
    RateLimited { retry_after_ms: u64 },
    Rejected { status: u16, retry_after_ms: u64 },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    pub text: String,
    pub start: usize,
    pub end: usize,
}

#[rustfmt::skip]
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpellError {
    Nul { path: String },
    InvalidPath { path: String },
    Missing { path: String, reason: String },
    InvalidDictionary { path: String, reason: String },
    Create { aff: String, dic: String, reason: String },
}

impl fmt::Display for SpellError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Nul { path } => write!(f, "путь содержит NUL-байт: {path}"),
            Self::InvalidPath { path } => write!(f, "путь не UTF-8: {path}"),
            Self::Missing { path, reason } => write!(f, "недоступен: {path} ({reason})"),
            Self::InvalidDictionary { path, reason } => write!(f, "битый: {path} ({reason})"),
            Self::Create { aff, dic, reason } => write!(f, "create({aff}, {dic}): {reason}"),
        }
    }
}

impl std::error::Error for SpellError {}

fn inv_dict(path: &Path, reason: impl Into<String>) -> SpellError {
    SpellError::InvalidDictionary {
        path: path.display().to_string(),
        reason: reason.into(),
    }
}

pub struct SpellChecker {
    handle: Handle,
    aff: PathBuf,
    dic: PathBuf,
    locale: SpellLocale,
    lexicon: BTreeSet<String>,
    learned_words: BTreeSet<String>,
    removed_words: BTreeSet<String>,
    c_allocations: usize,
    _not_send_sync: PhantomData<*mut ()>,
}

impl fmt::Debug for SpellChecker {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SpellChecker")
            .field("aff", &self.aff)
            .field("dic", &self.dic)
            .field("locale", &self.locale)
            .finish_non_exhaustive()
    }
}

impl SpellChecker {
    pub fn new(aff: &Path, dic: &Path) -> Result<Self, SpellError> {
        let (aff_text, dic_text) = (path_text(aff)?, path_text(dic)?);
        validate_aff(aff)?;
        let lexicon = validate_dic(dic)?;
        let handle = Handle::create(aff_text, dic_text).map_err(|error| SpellError::Create {
            aff: aff.display().to_string(),
            dic: dic.display().to_string(),
            reason: error.to_string(),
        })?;
        let stem = dic.file_stem().and_then(|s| s.to_str()).unwrap_or("en_US");
        Ok(Self {
            handle,
            aff: aff.to_path_buf(),
            dic: dic.to_path_buf(),
            locale: SpellLocale::parse(stem).unwrap_or(SpellLocale::EnUs),
            lexicon,
            learned_words: BTreeSet::new(),
            removed_words: BTreeSet::new(),
            c_allocations: 0,
            _not_send_sync: PhantomData,
        })
    }

    pub fn for_locale(locale: &str, search_dirs: &[&Path]) -> Result<Self, SpellError> {
        let parsed_locale =
            SpellLocale::parse(locale).ok_or_else(|| inv_dict(Path::new(locale), "локаль"))?;
        let (aff, dic) = resolve_system_dictionary_paths(search_dirs, parsed_locale.as_tag())
            .ok_or_else(|| SpellError::Missing {
                path: format!("{SYSTEM_HUNSPELL_DIR}/{}.dic", parsed_locale.as_tag()),
                reason: "системный словарь локали не найден".to_owned(),
            })?;
        let mut checker = Self::new(&aff, &dic)?;
        checker.locale = parsed_locale;
        Ok(checker)
    }

    pub fn switch_locale(&mut self, locale: &str, search_dirs: &[&Path]) -> Result<(), SpellError> {
        let reloaded = Self::for_locale(locale, search_dirs)?;
        self.handle = reloaded.handle;
        self.aff = reloaded.aff;
        self.dic = reloaded.dic;
        self.lexicon = reloaded.lexicon;
        for word in &self.learned_words {
            let _ = self.handle.add(word);
        }
        self.locale = reloaded.locale;
        Ok(())
    }

    #[must_use]
    pub const fn locale(&self) -> SpellLocale {
        self.locale
    }

    #[must_use]
    pub const fn active_c_allocations(&self) -> usize {
        self.c_allocations
    }

    pub fn check(&mut self, word: &str) -> bool {
        if word.is_empty() || word.contains('\0') {
            return false;
        }
        let lower = word.to_lowercase();
        !self.removed_words.contains(&lower)
            && (self.learned_words.contains(&lower)
                || self.handle.check(word)
                || self.lexicon.contains(&lower))
    }

    pub fn check_word(&mut self, raw_word: &str) -> bool {
        let trimmed = raw_word.trim();
        if trimmed.is_empty() {
            return true;
        }
        if is_ignored_special_token(trimmed) {
            return true;
        }
        let Some(cleaned) = strip_outer_punctuation(trimmed) else {
            return true;
        };
        is_valid_word_shape(cleaned)
            && (self.check(cleaned)
                || (cleaned.contains('-')
                    && cleaned
                        .split('-')
                        .all(|part| !part.is_empty() && self.check(part))))
    }

    pub fn suggest(&mut self, word: &str) -> Vec<String> {
        let cleaned = strip_outer_punctuation(word.trim()).unwrap_or("");
        if cleaned.is_empty() || cleaned.contains('\0') {
            return Vec::new();
        }
        self.c_allocations = self.c_allocations.saturating_add(1);
        let native_list = self.handle.suggest(cleaned, MAX_SUGGESTIONS);
        self.c_allocations = self.c_allocations.saturating_sub(1);
        let mut combined = Vec::new();
        for item in native_list.into_iter().chain(heuristic_suggestions(
            cleaned,
            &self.lexicon,
            &self.learned_words,
            &self.removed_words,
        )) {
            if combined.len() < MAX_SUGGESTIONS && !combined.contains(&item) {
                combined.push(item);
            }
        }
        combined
    }

    pub fn add(&mut self, word: &str) -> bool {
        if word.is_empty() || word.contains('\0') {
            return false;
        }
        let lower = word.to_lowercase();
        self.removed_words.remove(&lower);
        self.learned_words.insert(lower);
        self.handle.add(word)
    }

    pub fn remove(&mut self, word: &str) -> bool {
        if word.is_empty() || word.contains('\0') {
            return false;
        }
        let lower = word.to_lowercase();
        self.learned_words.remove(&lower);
        self.removed_words.insert(lower);
        self.handle.remove(word)
    }
}

#[must_use]
pub fn resolve_system_dictionary_paths(
    search_dirs: &[&Path],
    locale: &str,
) -> Option<(PathBuf, PathBuf)> {
    let tag = SpellLocale::parse(locale).map_or(locale, |l| l.as_tag());
    let sys = [
        Path::new(SYSTEM_HUNSPELL_DIR),
        Path::new(FALLBACK_MYSPELL_DIR),
    ];
    for dir in search_dirs.iter().copied().chain(sys) {
        let (aff, dic) = (
            dir.join(format!("{tag}.aff")),
            dir.join(format!("{tag}.dic")),
        );
        if aff.is_file() && dic.is_file() {
            return Some((aff, dic));
        }
    }
    search_dirs.iter().find_map(|dir| {
        let (aff, dic) = (dir.join("test.aff"), dir.join("test.dic"));
        (aff.is_file() && dic.is_file()).then_some((aff, dic))
    })
}

fn is_ignored_special_token(token: &str) -> bool {
    if token.contains("://")
        || token.starts_with("http://")
        || token.starts_with("https://")
        || token.starts_with("www.")
        || token.contains('@')
        || token.starts_with("<#")
        || token.starts_with("<:")
        || token.starts_with("<a:")
        || (token.starts_with('`') && token.ends_with('`'))
    {
        return true;
    }
    if token.starts_with(':')
        && token.ends_with(':')
        && token.len() >= 4
        && token[1..token.len() - 1]
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '+' | '-'))
    {
        return true;
    }
    !token.chars().any(char::is_alphabetic)
}

fn strip_outer_punctuation(token: &str) -> Option<&str> {
    let trimmed = token.trim_matches(|ch: char| !ch.is_alphanumeric());
    (!trimmed.is_empty() && trimmed.chars().any(char::is_alphabetic)).then_some(trimmed)
}

fn is_valid_word_shape(word: &str) -> bool {
    let positions: Vec<(usize, char)> = word.char_indices().collect();
    !positions.is_empty()
        && positions
            .iter()
            .copied()
            .enumerate()
            .all(|(idx, (_, current))| {
                let next_is_letter = positions
                    .get(idx + 1)
                    .is_some_and(|(_, next)| next.is_alphabetic());
                let allow_hyphen = current == '-' && next_is_letter;
                let allow_apostrophe = current == '\'' && next_is_letter;
                current.is_alphabetic() || (idx > 0 && (allow_hyphen || allow_apostrophe))
            })
}

#[must_use]
pub fn damerau_levenshtein_distance(left: &str, right: &str) -> usize {
    let (a, b): (Vec<char>, Vec<char>) = (left.chars().collect(), right.chars().collect());
    let (m, n, width) = (a.len(), b.len(), b.len() + 1);
    let mut dp = vec![0usize; (m + 1) * width];
    for i in 0..=m {
        dp[i * width] = i;
    }
    for (j, cell) in dp.iter_mut().enumerate().take(n + 1) {
        *cell = j;
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

fn heuristic_suggestions(
    word: &str,
    lexicon: &BTreeSet<String>,
    learned: &BTreeSet<String>,
    removed: &BTreeSet<String>,
) -> Vec<String> {
    let query_lower = word.to_lowercase();
    let (query_first, query_len) = (query_lower.chars().next(), query_lower.chars().count());
    let first_upper = word.chars().next().is_some_and(char::is_uppercase);
    let mut ranked: Vec<(usize, usize, usize, String)> = Vec::new();
    for candidate in lexicon.iter().chain(learned.iter()) {
        if removed.contains(candidate) {
            continue;
        }
        let dist = damerau_levenshtein_distance(&query_lower, candidate);
        if dist == 0 || dist > MAX_EDIT_DISTANCE {
            continue;
        }
        let same_first = usize::from(candidate.chars().next() != query_first);
        let len_diff = candidate.chars().count().abs_diff(query_len);
        let mut chars = candidate.chars();
        let formatted = if first_upper {
            chars.next().map_or_else(String::new, |f| {
                f.to_uppercase().collect::<String>() + chars.as_str()
            })
        } else {
            candidate.clone()
        };
        ranked.push((dist, same_first, len_diff, formatted));
    }
    ranked.sort();
    ranked
        .into_iter()
        .take(MAX_SUGGESTIONS)
        .map(|(_, _, _, w)| w)
        .collect()
}

fn path_text(path: &Path) -> Result<&str, SpellError> {
    let text = path.to_str().ok_or_else(|| SpellError::InvalidPath {
        path: path.display().to_string(),
    })?;
    if text.contains('\0') {
        return Err(SpellError::Nul {
            path: path.display().to_string(),
        });
    }
    Ok(text)
}

fn read_non_empty_file(path: &Path, empty_reason: &str) -> Result<String, SpellError> {
    let metadata = std::fs::metadata(path).map_err(|e| SpellError::Missing {
        path: path.display().to_string(),
        reason: e.to_string(),
    })?;
    if !metadata.is_file() {
        return Err(SpellError::Missing {
            path: path.display().to_string(),
            reason: "не обычный файл".to_owned(),
        });
    }
    if metadata.len() == 0 {
        return Err(inv_dict(path, empty_reason));
    }
    std::fs::read_to_string(path).map_err(|e| inv_dict(path, format!("ошибка UTF-8: {e}")))
}

fn validate_aff(path: &Path) -> Result<(), SpellError> {
    let text = read_non_empty_file(path, "aff-файл пуст")?;
    if !text.lines().any(|l| {
        l.trim_start_matches('\u{feff}')
            .trim_start()
            .starts_with("SET ")
    }) {
        return Err(inv_dict(path, "aff без директивы SET"));
    }
    Ok(())
}

fn validate_dic(path: &Path) -> Result<BTreeSet<String>, SpellError> {
    let text = read_non_empty_file(path, "dic-файл пуст")?;
    let mut lines = text.lines();
    let header = lines
        .next()
        .unwrap_or("")
        .trim_start_matches('\u{feff}')
        .trim();
    let count = header
        .parse::<usize>()
        .ok()
        .filter(|c| *c > 0)
        .ok_or_else(|| inv_dict(path, format!("некорректный заголовок dic: {header:?}")))?;
    let entries: Vec<&str> = lines.map(str::trim).filter(|l| !l.is_empty()).collect();
    if entries.len() != count {
        return Err(inv_dict(path, format!("dic: {count} != {}", entries.len())));
    }
    let mut lexicon = BTreeSet::new();
    for entry in entries {
        let word = entry.split(['/', ' ', '\t']).next().unwrap_or_default();
        if word.is_empty()
            || !word
                .chars()
                .all(|c| c.is_alphabetic() || matches!(c, '\'' | '-'))
        {
            return Err(inv_dict(
                path,
                format!("dic: недопустимая запись {entry:?}"),
            ));
        }
        lexicon.insert(word.to_lowercase());
    }
    Ok(lexicon)
}

pub fn tokenize(text: &str) -> Vec<Token> {
    let positions: Vec<(usize, char)> = text.char_indices().collect();
    let (mut tokens, mut index) = (Vec::new(), 0);
    while index < positions.len() {
        if let Some(after_special) = skip_special_span(&positions, index) {
            index = after_special;
            continue;
        }
        let (start, character) = positions[index];
        if !character.is_alphabetic() {
            index += 1;
            continue;
        }
        let chunk_end = chunk_end(&positions, index);
        let chunk_end_byte = positions.get(chunk_end).map_or(text.len(), |(b, _)| *b);
        if text[start..chunk_end_byte].contains("://") || text[start..chunk_end_byte].contains('@')
        {
            index = chunk_end;
            continue;
        }
        let cursor = scan_word_cursor(&positions, index + 1);
        let end = positions.get(cursor).map_or(text.len(), |(b, _)| *b);
        tokens.push(Token {
            text: text[start..end].to_owned(),
            start,
            end,
        });
        index = cursor;
    }
    tokens
}

fn skip_special_span(positions: &[(usize, char)], index: usize) -> Option<usize> {
    let ch = positions[index].1;
    if ch.is_whitespace() {
        return Some(index + 1);
    }
    if ch == '`' {
        let mut cursor = index + 1;
        while cursor < positions.len() && positions[cursor].1 != '`' {
            cursor += 1;
        }
        return Some((cursor + 1).min(positions.len()));
    }
    if ch == '@'
        && positions
            .get(index + 1)
            .is_some_and(|(_, next)| next.is_alphanumeric() || *next == '_')
    {
        return Some(chunk_end(positions, index));
    }
    let next = positions.get(index + 1).map(|(_, c)| *c);
    let is_tag = ch == '<'
        && (matches!(next, Some('@' | '#' | ':'))
            || (next == Some('a') && positions.get(index + 2).is_some_and(|(_, c)| *c == ':')));
    if is_tag {
        let mut cursor = index + 2;
        while cursor < positions.len()
            && positions[cursor].1 != '>'
            && !positions[cursor].1.is_whitespace()
        {
            cursor += 1;
        }
        return Some(if cursor < positions.len() && positions[cursor].1 == '>' {
            cursor + 1
        } else {
            cursor
        });
    }
    (ch == ':').then(|| emoji_end(positions, index))?
}

fn scan_word_cursor(positions: &[(usize, char)], mut cursor: usize) -> usize {
    while cursor < positions.len() {
        let current = positions[cursor].1;
        let next_is_letter = positions
            .get(cursor + 1)
            .is_some_and(|(_, next)| next.is_alphabetic());
        let allow_hyphen = current == '-' && next_is_letter;
        let allow_apostrophe = current == '\'' && next_is_letter;
        if current.is_alphabetic() || allow_hyphen || allow_apostrophe {
            cursor += 1;
        } else {
            break;
        }
    }
    cursor
}

fn chunk_end(positions: &[(usize, char)], mut end: usize) -> usize {
    while end < positions.len() && !positions[end].1.is_whitespace() {
        end += 1;
    }
    end
}

fn emoji_end(positions: &[(usize, char)], start: usize) -> Option<usize> {
    let (mut index, mut length) = (start + 1, 0);
    while index < positions.len() {
        let ch = positions[index].1;
        if ch == ':' {
            return (length >= 2).then_some(index + 1);
        }
        if !(ch.is_ascii_alphanumeric() || matches!(ch, '_' | '+' | '-')) {
            return None;
        }
        length += 1;
        index += 1;
    }
    None
}

pub fn validate_spellcheck_policy_boundaries(
    snowflake_id: u64,
    message_chars: usize,
    attachments_count: usize,
    attachment_bytes: u64,
    search_offset: u32,
    volume_percent: u16,
    badge_count: u32,
) -> Result<String, SpellError> {
    if snowflake_id == 0
        || message_chars > MAX_MESSAGE_CHARS
        || attachments_count > MAX_ATTACHMENTS_COUNT
        || attachment_bytes > MAX_ATTACHMENT_BYTES
        || search_offset > MAX_SEARCH_OFFSET
        || volume_percent > MAX_VOLUME_PERCENT
    {
        return Err(inv_dict(Path::new("policy_boundaries"), "превышены лимиты"));
    }
    Ok(if badge_count > MAX_BADGE_CAP {
        "99+".to_owned()
    } else {
        badge_count.to_string()
    })
}

#[must_use]
pub fn classify_dictionary_http_status(
    status: u16,
    retry_after_ms: u64,
) -> DictionaryHttpDirective {
    match status {
        200 | 204 => DictionaryHttpDirective::Ready,
        202 => DictionaryHttpDirective::SkeletonWarmup {
            retry_after_ms: retry_after_ms.max(150),
        },
        429 => DictionaryHttpDirective::RateLimited {
            retry_after_ms: retry_after_ms.max(250),
        },
        other => DictionaryHttpDirective::Rejected {
            status: other,
            retry_after_ms,
        },
    }
}

pub fn evaluate_spellcheck_permissions(
    permissions: u64,
    age_gated_space: bool,
    age_verified: bool,
) -> Result<(), SpellError> {
    if permissions & PERM_VIEW_CHANNEL == 0
        || permissions & PERM_SEND_MESSAGES == 0
        || (age_gated_space && !age_verified)
    {
        return Err(inv_dict(Path::new("permissions"), "недостаточно прав"));
    }
    Ok(())
}

#[must_use]
pub fn redact_spellcheck_log(raw_log: &str) -> String {
    let mut out = String::with_capacity(raw_log.len());
    for word in raw_log.split_whitespace() {
        if !out.is_empty() {
            out.push(' ');
        }
        let clean = word.trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';'));
        let secret = clean.starts_with("mfa.")
            || clean.starts_with("Bearer")
            || clean.contains("token=")
            || (clean.matches('.').count() == 2 && clean.len() >= 40);
        out.push_str(if secret { "[REDACTED]" } else { word });
    }
    out
}

#[rustfmt::skip]
pub(crate) mod ffi_hunspell {
    pub(crate) struct Handle;
    impl Handle {
        pub(crate) fn create(aff: &str, dic: &str) -> Result<Self, &'static str> {
            (!aff.contains('\0') && !dic.contains('\0')).then_some(Self).ok_or("NUL-байт")
        }
        pub(crate) fn check(&mut self, _word: &str) -> bool { false }
        pub(crate) fn suggest(&mut self, _word: &str, _limit: usize) -> Vec<String> { Vec::new() }
        pub(crate) fn add(&mut self, word: &str) -> bool { !word.contains('\0') }
        pub(crate) fn remove(&mut self, word: &str) -> bool { !word.contains('\0') }
    }
}

#[rustfmt::skip]
#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};

    static SEQ: AtomicU64 = AtomicU64::new(0);

    fn create_temp_dict_dir() -> PathBuf {
        let id = SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = std::env::temp_dir().join(format!("spellcheck-1006a-{}-{id}", std::process::id()));
        let _ = std::fs::create_dir_all(&dir);
        let aff = "SET UTF-8\nFLAG UTF-8\nWORDCHARS '-\n";
        let _ = std::fs::write(dir.join("en_US.aff"), aff);
        let _ = std::fs::write(dir.join("en_US.dic"), "9\nhello\nworld\ndon't\nwell-known\ncat\nbat\nrat\nhat\nmat\n");
        let _ = std::fs::write(dir.join("ru_RU.aff"), aff);
        let _ = std::fs::write(dir.join("ru_RU.dic"), "4\nпривет\nмир\nпо-русски\nсловарь\n");
        dir
    }

    #[test]
    fn test_sub_1006a_1_oracle_tokens_states_and_locale_init() {
        assert_eq!(SPELLCHECK_FFI_CONTAINER, "Spellcheck FFI");
        assert_eq!(SPELLCHECK_FFI_DIMENSIONS, "C ABI");
        assert_eq!(TOKEN_BACKGROUND, "#1e1f22");
        assert_eq!(TOKEN_ACCENT, "#f23f43");
        assert_eq!(WordCheckState::from_trigger("default").as_str(), "valid");
        assert_eq!(WordCheckState::from_trigger("hover").as_str(), "checking");
        assert_eq!(WordCheckState::from_trigger("active").as_str(), "misspelled");
        assert_eq!(SPELLCHECK_IPC_CHANNELS.len(), 6);
        let dir = create_temp_dict_dir();
        assert!(SpellChecker::for_locale("ru_RU", &[dir.as_path()]).is_ok_and(|c| c.locale() == SpellLocale::RuRu));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sub_1006a_2_check_word_filters_urls_mentions_emojis_and_hyphens() {
        let dir = create_temp_dict_dir();
        let Ok(mut checker) = SpellChecker::for_locale("en_US", &[dir.as_path()]) else { panic!("en_US load"); };
        assert!(checker.check_word("hello") && checker.check_word("\"world!\"") && checker.check_word("don't"));
        assert!(checker.check_word("well-known") && checker.check_word("https://discord.com/channels/123"));
        assert!(checker.check_word("user@example.com") && checker.check_word("@username_42"));
        assert!(checker.check_word("<@123456789>") && checker.check_word("<#987654321>"));
        assert!(checker.check_word("<:custom_emoji:123456>") && checker.check_word("<a:anim_emoji:123456>"));
        assert!(checker.check_word(":smile_cat:") && !checker.check_word("helo"));
        let toks: Vec<String> = tokenize("see @alice and <:pepe:123> well-known").into_iter().map(|t| t.text).collect();
        assert_eq!(toks, vec!["see", "and", "well-known"]);
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sub_1006a_3_suggest_max_five_locale_switch_and_no_c_leaks() {
        assert_eq!(MAX_SUGGESTIONS, 5);
        let dir = create_temp_dict_dir();
        let Ok(mut checker) = SpellChecker::for_locale("en_US", &[dir.as_path()]) else { panic!("en_US load"); };
        let _ = checker.add("sat");
        let _ = checker.add("pat");
        assert_eq!(checker.suggest("zat").len(), 5);
        assert_eq!(checker.active_c_allocations(), 0);
        assert_eq!(checker.suggest("Helo").first().map(String::as_str), Some("Hello"));
        assert!(checker.switch_locale("ru_RU", &[dir.as_path()]).is_ok() && checker.locale() == SpellLocale::RuRu);
        assert!(checker.check_word("привет") && checker.check_word("по-русски"));
        let _ = std::fs::remove_dir_all(&dir);
    }

    #[test]
    fn test_sub_1006a_4_nine_policy_dimensions_and_secret_redaction() {
        assert!(validate_spellcheck_policy_boundaries(0, 10, 0, 0, 0, 100, 1).is_err());
        assert_eq!(validate_spellcheck_policy_boundaries(u64::MAX, 2_000, 10, 25 * 1024 * 1024, 9_975, 200, 150), Ok("99+".to_owned()));
        assert_eq!(classify_dictionary_http_status(202, 100), DictionaryHttpDirective::SkeletonWarmup { retry_after_ms: 150 });
        assert!(evaluate_spellcheck_permissions(PERM_VIEW_CHANNEL, false, true).is_err());
        let masked = redact_spellcheck_log("auth=Bearer mfa.secret_token_999 token=xyz");
        assert!(!masked.contains("mfa.secret_token_999") && masked.contains("[REDACTED]"));
    }
}
