//! `Intl.DurationFormat`-compatible formatting (P7-01d).
//!
//! Patterns and separators come from the frozen generated table (`duration_data.rs`,
//! Node 24 / ICU); locale resolution mirrors `crate::locale`, plural categories —
//! `crate::plural`, digit grouping — `crate::format`. Contract:
//! `waves/RW-30/TZ-W30a.md`.
//!
//! Semantics: units are emitted in the fixed order years → months → weeks → days →
//! hours → minutes → seconds → milliseconds, zero units are skipped, an all-zero
//! duration yields `""`. Each unit is rendered as its CLDR template for the cardinal
//! plural category of the exact integer magnitude (no `f64` round-trip, so values
//! above 2^53 keep every digit and their category). Separators are frozen per
//! locale/style: `join2` for exactly two units, otherwise `join_middle` between the
//! leading units and `join_last` before the final one.
//!
//! Fallback chain: requested (canonical + enabled) → `en-US` row → `"{n} {unit}"`.
//! NNBSP/NBSP are normalized to a plain space in the output (as in `render_pattern`).
//! The function never panics, including on `u64::MAX`; there is no `unsafe`.

use crate::duration_data::{DURATION_JOINS, DURATION_ROWS, DurationJoin, DurationRow};
use crate::format::format_integer_unsigned;
use crate::locale::{DEFAULT_LOCALE, canonical_locale, is_enabled};
use crate::plural::{PluralType, plural_category_integer};

/// Duration value, mirroring the fields of `Intl.DurationFormat`'s duration argument.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Duration {
    pub years: u64,
    pub months: u64,
    pub weeks: u64,
    pub days: u64,
    pub hours: u64,
    pub minutes: u64,
    pub seconds: u64,
    pub milliseconds: u64,
}

/// Duration style, mirroring `Intl.DurationFormat`'s `style` option.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurationStyle {
    Long,
    Short,
    Narrow,
}

impl DurationStyle {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Long => "long",
            Self::Short => "short",
            Self::Narrow => "narrow",
        }
    }
}

impl Duration {
    /// Units in contract order: `(table key, value)`.
    const fn units(&self) -> [(&'static str, u64); 8] {
        [
            ("year", self.years),
            ("month", self.months),
            ("week", self.weeks),
            ("day", self.days),
            ("hour", self.hours),
            ("minute", self.minutes),
            ("second", self.seconds),
            ("millisecond", self.milliseconds),
        ]
    }
}

/// Number of frozen table rows (31 enabled locales × 3 styles × 8 units).
pub const DURATION_ROW_COUNT: usize = DURATION_ROWS.len();
/// Number of frozen separator rows (31 enabled locales × 3 styles).
pub const DURATION_JOIN_COUNT: usize = DURATION_JOINS.len();
/// Number of frozen unit patterns (rows × 6 CLDR categories).
pub const DURATION_PATTERN_COUNT: usize = DURATION_ROW_COUNT * 6;

fn row(locale: &str, style: DurationStyle, unit: &str) -> Option<&'static DurationRow> {
    DURATION_ROWS
        .binary_search_by(|candidate| {
            (candidate.locale, candidate.style, candidate.unit).cmp(&(locale, style.as_str(), unit))
        })
        .ok()
        .map(|index| &DURATION_ROWS[index])
}

fn joins(locale: &str, style: DurationStyle) -> Option<&'static DurationJoin> {
    DURATION_JOINS
        .binary_search_by(|candidate| {
            (candidate.locale, candidate.style).cmp(&(locale, style.as_str()))
        })
        .ok()
        .map(|index| &DURATION_JOINS[index])
}

fn render_unit(row: &DurationRow, value: u64) -> String {
    let category = plural_category_integer(row.locale, value, PluralType::Cardinal).as_str();
    let template = row
        .patterns
        .iter()
        .find(|(candidate, _)| *candidate == category)
        .or_else(|| {
            row.patterns
                .iter()
                .find(|(candidate, _)| *candidate == "other")
        })
        .map(|(_, template)| *template);
    let number = format_integer_unsigned(value, row.locale);
    match template {
        Some(template) => template.replace("{0}", &number),
        // Generated rows always carry all six categories; keep a deterministic
        // per-unit fallback instead of panicking if a future table loses one.
        None => format!("{number} {}", row.unit),
    }
}

fn join_chunks(chunks: &[String], joins: &DurationJoin) -> String {
    match chunks {
        [] => String::new(),
        [only] => only.clone(),
        [first, second] => {
            let mut out = String::with_capacity(first.len() + joins.join2.len() + second.len());
            out.push_str(first);
            out.push_str(joins.join2);
            out.push_str(second);
            out
        }
        [first, rest @ ..] => {
            let Some((last, middle)) = rest.split_last() else {
                return first.clone();
            };
            let mut out = String::with_capacity(
                chunks.iter().map(String::len).sum::<usize>()
                    + joins.join_middle.len() * middle.len(),
            );
            out.push_str(first);
            for chunk in middle {
                out.push_str(joins.join_middle);
                out.push_str(chunk);
            }
            out.push_str(joins.join_last);
            out.push_str(last);
            out
        }
    }
}

/// Format `duration` in `locale` using `style`.
///
/// Unknown or disabled locales fall back to `en-US`; a missing table row falls back
/// to the `en-US` row for the same style/unit, then to `"{n} {unit}"`. Zero units are
/// omitted, an all-zero duration is the empty string. Output never contains NNBSP or
/// NBSP (they are normalized to a plain space) and the function never panics.
#[must_use]
pub fn format_duration(locale: &str, duration: &Duration, style: DurationStyle) -> String {
    let resolved = canonical_locale(locale)
        .filter(|code| is_enabled(code))
        .unwrap_or(DEFAULT_LOCALE);

    let mut chunks: Vec<String> = Vec::with_capacity(8);
    for (unit, value) in duration.units() {
        if value == 0 {
            continue;
        }
        let row = row(resolved, style, unit).or_else(|| row(DEFAULT_LOCALE, style, unit));
        chunks.push(match row {
            Some(row) => render_unit(row, value),
            None => format!("{} {unit}", format_integer_unsigned(value, resolved)),
        });
    }

    let rendered = if chunks.is_empty() {
        String::new()
    } else if let Some(joins) = joins(resolved, style).or_else(|| joins(DEFAULT_LOCALE, style)) {
        join_chunks(&chunks, joins)
    } else {
        chunks.join(", ")
    };
    rendered.replace(['\u{202f}', '\u{a0}'], " ")
}
