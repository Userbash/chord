//! Discord `<t:…>` timestamp formatting (P7-01e).
//!
//! Renders the seven client styles (`t T d D f F R`) deterministically in UTC.
//! Calendar patterns come from the frozen generated table (`timestamp_data.rs`,
//! Node 24 / ICU); the relative style reuses `crate::relative`. Locale resolution
//! mirrors `crate::locale`. Contract: `waves/RW-29/TZ-W29a.md`.
//!
//! Fallback chain: requested (canonical + enabled) → `en-US` pattern row. Tokens are
//! replaced left to right, nesting is not supported, and every malformed token
//! (`<t:abc>`, `<t:1:q>`, `<t:1:RR>`, `<t:>`, `<t:1`, overflow) is copied verbatim.
//! `i64::MIN`/`i64::MAX` never panic: the relative delta and the integer parser saturate.

// `lib.rs` is a shared file for the wave and only carries `pub mod timestamp;`, so the
// generated table is attached to this module instead of being declared next to it.
#[path = "timestamp_data.rs"]
mod timestamp_data;

use crate::format::format_date_time_pattern;
use crate::locale::{DEFAULT_LOCALE, canonical_locale, is_enabled};
use crate::relative::{RelativeNumeric, RelativeStyle, RelativeUnit, format_relative};
use timestamp_data::TIMESTAMP_LOCALES;

/// Number of locales with a generated pattern row (31 enabled locales).
pub const TIMESTAMP_LOCALE_COUNT: usize = TIMESTAMP_LOCALES.len();
/// Number of generated calendar patterns (31 locales × 6 styles `t T d D f F`).
pub const TIMESTAMP_PATTERN_COUNT: usize = TIMESTAMP_LOCALE_COUNT * 6;

/// Discord timestamp style, one per client letter.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TimestampStyle {
    /// `t` — short time (`9:01 AM`).
    ShortTime,
    /// `T` — long time with seconds (`9:01:00 AM`).
    LongTime,
    /// `d` — short date (`06/12/2021`).
    ShortDate,
    /// `D` — long date (`June 12, 2021`).
    LongDate,
    /// `f` — short date + short time.
    ShortDateTime,
    /// `F` — long date with weekday + short time.
    LongDateTime,
    /// `R` — relative time.
    Relative,
}

impl TimestampStyle {
    /// Index into the generated `t T d D f F` row; `Relative` has no pattern.
    const fn pattern_index(self) -> Option<usize> {
        match self {
            Self::ShortTime => Some(0),
            Self::LongTime => Some(1),
            Self::ShortDate => Some(2),
            Self::LongDate => Some(3),
            Self::ShortDateTime => Some(4),
            Self::LongDateTime => Some(5),
            Self::Relative => None,
        }
    }
}

/// Relative ladder units in seconds (`|delta|` thresholds, contract §1).
const MINUTE_SECS: u64 = 60;
const HOUR_SECS: u64 = 3_600;
const DAY_SECS: u64 = 86_400;
const WEEK_SECS: u64 = 7 * DAY_SECS;
const MONTH_SECS: u64 = 30 * DAY_SECS;
const YEAR_SECS: u64 = 365 * DAY_SECS;

/// Style by Discord letter (`t T d D f F R`); `None` for unknown or multi-char codes.
#[must_use]
pub fn timestamp_style(code: &str) -> Option<TimestampStyle> {
    match code.as_bytes() {
        [byte] => style_from_byte(*byte),
        _ => None,
    }
}

fn style_from_byte(byte: u8) -> Option<TimestampStyle> {
    match byte {
        b't' => Some(TimestampStyle::ShortTime),
        b'T' => Some(TimestampStyle::LongTime),
        b'd' => Some(TimestampStyle::ShortDate),
        b'D' => Some(TimestampStyle::LongDate),
        b'f' => Some(TimestampStyle::ShortDateTime),
        b'F' => Some(TimestampStyle::LongDateTime),
        b'R' => Some(TimestampStyle::Relative),
        _ => None,
    }
}

fn locale_row(locale: &str) -> Option<&'static timestamp_data::TimestampLocaleData> {
    TIMESTAMP_LOCALES
        .binary_search_by(|candidate| candidate.locale.cmp(locale))
        .ok()
        .map(|index| &TIMESTAMP_LOCALES[index])
}

fn resolve_locale(locale: &str) -> &'static str {
    canonical_locale(locale)
        .filter(|code| is_enabled(code))
        .unwrap_or(DEFAULT_LOCALE)
}

/// Render one Discord timestamp in `locale`.
///
/// `now_secs` is the UTC "now" used by the relative style; it does not affect the six
/// calendar styles. Unknown or disabled locales fall back to `en-US`. All rendering is
/// in UTC and consistent with `format_date`/`format_time` (NBSP/NNBSP → space).
#[must_use]
pub fn render_timestamp(
    timestamp_secs: i64,
    style: TimestampStyle,
    locale: &str,
    now_secs: i64,
) -> String {
    render_resolved(timestamp_secs, style, resolve_locale(locale), now_secs)
}

fn render_resolved(
    timestamp_secs: i64,
    style: TimestampStyle,
    locale: &'static str,
    now_secs: i64,
) -> String {
    let Some(index) = style.pattern_index() else {
        return render_relative(timestamp_secs, locale, now_secs);
    };
    let Some(row) = locale_row(locale).or_else(|| locale_row(DEFAULT_LOCALE)) else {
        return String::new();
    };
    format_date_time_pattern(
        timestamp_secs,
        locale,
        row.patterns[index],
        Some(&row.weekdays_long),
    )
}

/// Discord `R` ladder: unit by `|delta|`, amount truncated toward zero.
fn relative_unit(delta: i64) -> (RelativeUnit, i64) {
    let magnitude = delta.unsigned_abs();
    if magnitude >= YEAR_SECS {
        (RelativeUnit::Year, YEAR_SECS as i64)
    } else if magnitude >= MONTH_SECS {
        (RelativeUnit::Month, MONTH_SECS as i64)
    } else if magnitude >= WEEK_SECS {
        (RelativeUnit::Week, WEEK_SECS as i64)
    } else if magnitude >= DAY_SECS {
        (RelativeUnit::Day, DAY_SECS as i64)
    } else if magnitude >= HOUR_SECS {
        (RelativeUnit::Hour, HOUR_SECS as i64)
    } else if magnitude >= MINUTE_SECS {
        (RelativeUnit::Minute, MINUTE_SECS as i64)
    } else {
        (RelativeUnit::Second, 1)
    }
}

fn render_relative(timestamp_secs: i64, locale: &str, now_secs: i64) -> String {
    // Saturating subtraction keeps `i64::MIN`/`i64::MAX` panic-free.
    let delta = timestamp_secs.saturating_sub(now_secs);
    let (unit, seconds) = relative_unit(delta);
    let amount = delta / seconds;
    // `Auto` is the frozen contract: zero becomes "now", and ICU special forms
    // (`yesterday`, Russian `послезавтра`, …) apply where the frozen table has them.
    format_relative(
        locale,
        amount,
        unit,
        RelativeStyle::Long,
        RelativeNumeric::Auto,
    )
}

/// Replace every valid `<t:…>` token in `input`, keeping the rest byte-for-byte.
///
/// Valid syntax: `<t:` + optional `-` + decimal seconds (`i64`) + optional `:` + exactly
/// one letter from `t T d D f F R` + `>`. An omitted style means `f` (short date/time).
/// Malformed or overflowing tokens are copied unchanged; nesting is not supported.
#[must_use]
pub fn render_timestamp_tokens(input: &str, locale: &str, now_secs: i64) -> String {
    if !input.contains("<t:") {
        return input.to_owned();
    }
    let resolved = resolve_locale(locale);
    let mut out = String::with_capacity(input.len());
    let mut cursor = 0;
    while let Some(offset) = input[cursor..].find("<t:") {
        let start = cursor + offset;
        out.push_str(&input[cursor..start]);
        match parse_token(&input[start..]) {
            Some((timestamp_secs, style, consumed)) => {
                out.push_str(&render_resolved(timestamp_secs, style, resolved, now_secs));
                cursor = start + consumed;
            }
            // Invalid token: copy its `<t:` prefix and rescan from the next byte.
            None => {
                out.push_str("<t:");
                cursor = start + 3;
            }
        }
    }
    out.push_str(&input[cursor..]);
    out
}

/// Parse a token at the start of `input`; returns `(seconds, style, consumed_bytes)`.
fn parse_token(input: &str) -> Option<(i64, TimestampStyle, usize)> {
    let rest = input.strip_prefix("<t:")?;
    let bytes = rest.as_bytes();
    let mut index = 0;
    let negative = bytes.first() == Some(&b'-');
    if negative {
        index += 1;
    }
    let digits_start = index;
    while bytes.get(index).is_some_and(u8::is_ascii_digit) {
        index += 1;
    }
    if index == digits_start {
        return None;
    }
    let timestamp_secs = parse_seconds(&rest[digits_start..index], negative)?;
    let (style, end) = match bytes.get(index) {
        Some(b':') => {
            let style = style_from_byte(*bytes.get(index + 1)?)?;
            if bytes.get(index + 2) != Some(&b'>') {
                return None;
            }
            (style, index + 3)
        }
        Some(b'>') => (TimestampStyle::ShortDateTime, index + 1),
        _ => return None,
    };
    // `<t:` is three ASCII bytes, so `end` is relative to the token start.
    Some((timestamp_secs, style, 3 + end))
}

fn parse_seconds(digits: &str, negative: bool) -> Option<i64> {
    let mut magnitude: u64 = 0;
    for byte in digits.bytes() {
        magnitude = magnitude
            .checked_mul(10)?
            .checked_add(u64::from(byte - b'0'))?;
    }
    if negative {
        if magnitude == 1 << 63 {
            Some(i64::MIN)
        } else {
            i64::try_from(magnitude).ok().map(|value| -value)
        }
    } else {
        i64::try_from(magnitude).ok()
    }
}
