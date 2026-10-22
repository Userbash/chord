//! `Intl.RelativeTimeFormat`-compatible formatting (P7-01c).
//!
//! Patterns come from the frozen generated table (`relative_data.rs`, Node 24 / ICU);
//! locale resolution mirrors `crate::locale`, plural categories — `crate::plural`.
//! Contract: `waves/RW-28/TZ-W28a.md`.
//!
//! Fallback chain: requested (canonical + enabled) → `en-US` row → `"{n} {unit}"`.
//! Numeric categories use `|value|`; `Auto` special forms follow ICU (for `day` some locales
//! also have −2/+2 forms, e.g. Russian «позавчера»); when a form is absent the numeric
//! pattern is used. Integer magnitudes are formatted and categorized without an f64
//! round-trip, so values above 2^53 keep their digits and plural category.

use crate::format::format_integer_unsigned;
use crate::locale::{DEFAULT_LOCALE, canonical_locale, is_enabled};
use crate::plural::{PluralType, plural_category_integer};
use crate::relative_data::{RELATIVE_ROWS, RelativeRow};

/// Relative time unit, mirroring `Intl.RelativeTimeFormat`'s `unit` option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeUnit {
    Year,
    Quarter,
    Month,
    Week,
    Day,
    Hour,
    Minute,
    Second,
}

/// Relative time style, mirroring `Intl.RelativeTimeFormat`'s `style` option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeStyle {
    Long,
    Short,
    Narrow,
}

/// Numeric mode, mirroring `Intl.RelativeTimeFormat`'s `numeric` option.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RelativeNumeric {
    Always,
    Auto,
}

impl RelativeUnit {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Year => "year",
            Self::Quarter => "quarter",
            Self::Month => "month",
            Self::Week => "week",
            Self::Day => "day",
            Self::Hour => "hour",
            Self::Minute => "minute",
            Self::Second => "second",
        }
    }
}

impl RelativeStyle {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Long => "long",
            Self::Short => "short",
            Self::Narrow => "narrow",
        }
    }
}

/// Number of frozen table rows (31 enabled locales × 8 units × 3 styles).
pub const RELATIVE_ROW_COUNT: usize = RELATIVE_ROWS.len();

fn row(locale: &str, unit: RelativeUnit, style: RelativeStyle) -> Option<&'static RelativeRow> {
    RELATIVE_ROWS
        .binary_search_by(|candidate| {
            (candidate.locale, candidate.unit, candidate.style).cmp(&(
                locale,
                unit.as_str(),
                style.as_str(),
            ))
        })
        .ok()
        .map(|index| &RELATIVE_ROWS[index])
}

/// Format `value` in `unit` as a localized relative time string.
///
/// `value >= 0` is future, `value < 0` is past. `Auto` uses ICU special forms
/// (`yesterday`, `today`, `tomorrow`, and locale-specific −2/+2) and falls back to
/// numeric patterns otherwise. Missing rows fall back to `en-US`, then to
/// `"{n} {unit}"`. The function never panics (including `i64::MIN`).
///
/// Locale resolution accepts only exact canonical codes (`ru`, `zh-CN`, `pt-BR`, …):
/// `canonical_locale` + `is_enabled`, otherwise `DEFAULT_LOCALE` (en-US).
#[must_use]
pub fn format_relative(
    locale: &str,
    value: i64,
    unit: RelativeUnit,
    style: RelativeStyle,
    numeric: RelativeNumeric,
) -> String {
    let resolved = canonical_locale(locale)
        .filter(|code| is_enabled(code))
        .unwrap_or(DEFAULT_LOCALE);
    let Some(row) = row(resolved, unit, style).or_else(|| row(DEFAULT_LOCALE, unit, style)) else {
        return format!("{value} {}", unit.as_str());
    };

    if numeric == RelativeNumeric::Auto
        && let Ok(small) = i32::try_from(value)
        && let Some((_, text)) = row.auto.iter().find(|(candidate, _)| *candidate == small)
    {
        return (*text).to_owned();
    }

    let magnitude = value.unsigned_abs();
    let category = plural_category_integer(row.locale, magnitude, PluralType::Cardinal).as_str();
    let patterns = if value < 0 { row.past } else { row.future };
    let template = patterns
        .iter()
        .find(|(candidate, _)| *candidate == category)
        .or_else(|| patterns.iter().find(|(candidate, _)| *candidate == "other"));
    let Some((_, template)) = template else {
        return format!("{value} {}", unit.as_str());
    };

    let number = format_integer_unsigned(magnitude, row.locale);
    template.replace("{0}", &number)
}
