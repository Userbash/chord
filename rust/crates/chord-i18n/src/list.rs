//! `Intl.ListFormat`-compatible list formatting (P7-01b).
//!
//! Patterns come from the frozen generated table (`list_data.rs`, Node 24 / ICU);
//! locale resolution and fallback mirror `crate::locale`. Contract:
//! `waves/RW-27/TZ-W27a.md`.
//!
//! Fallback chain: requested (canonical + enabled) → `en-US` with the same style/type
//! → `", "` join. Empty input yields `""`, a single item is returned unchanged.

use crate::list_data::{LIST_PATTERNS, ListPatterns};
use crate::locale::{DEFAULT_LOCALE, canonical_locale, is_enabled};

/// List style, mirroring `Intl.ListFormat`'s `style` option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListStyle {
    Long,
    Short,
    Narrow,
}

/// List type, mirroring `Intl.ListFormat`'s `type` option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ListType {
    Conjunction,
    Disjunction,
    Unit,
}

impl ListStyle {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Long => "long",
            Self::Short => "short",
            Self::Narrow => "narrow",
        }
    }
}

impl ListType {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Conjunction => "conjunction",
            Self::Disjunction => "disjunction",
            Self::Unit => "unit",
        }
    }
}

fn patterns(locale: &str, style: ListStyle, kind: ListType) -> Option<&'static ListPatterns> {
    LIST_PATTERNS
        .iter()
        .find(|(row_locale, row_style, row_kind, _)| {
            *row_locale == locale && *row_style == style.as_str() && *row_kind == kind.as_str()
        })
        .map(|(_, _, _, pattern)| pattern)
}

/// Format `items` as a localized list.
///
/// Unknown or disabled locales fall back to `en-US`; a missing table row falls back
/// to the `en-US` row for the same style/type, then to a plain `", "` join. The
/// function never panics and never allocates locale data at runtime.
#[must_use]
pub fn format_list(locale: &str, items: &[&str], style: ListStyle, kind: ListType) -> String {
    if items.is_empty() {
        return String::new();
    }
    if items.len() == 1 {
        return items[0].to_owned();
    }

    let resolved = canonical_locale(locale)
        .filter(|code| is_enabled(code))
        .unwrap_or(DEFAULT_LOCALE);
    let pattern = patterns(resolved, style, kind).or_else(|| patterns(DEFAULT_LOCALE, style, kind));
    let Some(pattern) = pattern else {
        return items.join(", ");
    };

    let mut out = String::new();
    out.push_str(items[0]);
    if items.len() == 2 {
        out.push_str(pattern.pair);
        out.push_str(items[1]);
        return out;
    }

    out.push_str(pattern.start);
    out.push_str(items[1]);
    for item in &items[2..items.len() - 1] {
        out.push_str(pattern.middle);
        out.push_str(item);
    }
    out.push_str(pattern.end);
    out.push_str(items[items.len() - 1]);
    out
}
