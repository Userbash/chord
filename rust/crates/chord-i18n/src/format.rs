//! Locale-aware number, date and time formatting (P7-01).

use crate::format_data::{self, LocaleFormatData};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DateStyle {
    Short,
    Medium,
    Long,
    Full,
}
impl DateStyle {
    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "short" => Some(Self::Short),
            "medium" => Some(Self::Medium),
            "long" => Some(Self::Long),
            "full" => Some(Self::Full),
            _ => None,
        }
    }
    fn index(self) -> usize {
        match self {
            Self::Short => 0,
            Self::Medium => 1,
            Self::Long => 2,
            Self::Full => 3,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NumberStyle {
    Decimal,
    Integer,
    Percent,
    Currency,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NumberOptions {
    pub style: NumberStyle,
    pub minimum_fraction_digits: u8,
    pub maximum_fraction_digits: u8,
    pub use_grouping: bool,
}
impl Default for NumberOptions {
    fn default() -> Self {
        Self {
            style: NumberStyle::Decimal,
            minimum_fraction_digits: 0,
            maximum_fraction_digits: 3,
            use_grouping: true,
        }
    }
}

#[must_use]
pub fn format_number(value: f64, locale: &str) -> String {
    format_number_with(value, locale, NumberOptions::default())
}

#[must_use]
pub fn format_number_with(value: f64, locale: &str, options: NumberOptions) -> String {
    let data = format_data::data(locale);
    let value = if value.is_finite() { value } else { 0.0 };
    let value = if options.style == NumberStyle::Percent {
        value * 100.0
    } else {
        value
    };
    let max = if options.style == NumberStyle::Integer {
        0
    } else {
        options.maximum_fraction_digits.min(12) as usize
    };
    let min = (options.minimum_fraction_digits as usize).min(max);
    let factor = 10_f64.powi(max as i32);
    let rounded = (value.abs() * factor).round() / factor;
    let mut body = format!("{rounded:.max$}");
    while body.contains('.')
        && body.ends_with('0')
        && body.split_once('.').map_or(0, |(_, f)| f.len()) > min
    {
        body.pop();
    }
    if body.ends_with('.') {
        body.pop();
    }
    let (integer, fraction) = body.split_once('.').unwrap_or((&body, ""));
    let integer = if options.use_grouping {
        group_integer(integer, locale, data)
    } else {
        integer.to_owned()
    };
    let sign = if value.is_sign_negative() && rounded != 0.0 {
        "-"
    } else {
        ""
    };
    let mut out = format!("{sign}{integer}");
    if !fraction.is_empty() || min > 0 {
        out.push_str(data.decimal);
        out.push_str(fraction);
        for _ in fraction.len()..min {
            out.push('0');
        }
    }
    if options.style == NumberStyle::Percent {
        out.push('%');
    }
    out
}

fn group_digits(digits: &str, separator: &str, primary: usize, secondary: usize) -> String {
    if separator.is_empty() || digits.len() <= primary || primary == 0 {
        return digits.to_owned();
    }
    let mut chunks = Vec::new();
    let mut end = digits.len();
    let mut size = primary;
    while end > 0 {
        let start = end.saturating_sub(size);
        chunks.push(&digits[start..end]);
        end = start;
        size = secondary.max(1);
    }
    chunks.reverse();
    chunks.join(separator)
}

fn group_integer(digits: &str, locale: &str, data: &LocaleFormatData) -> String {
    if ["hu", "it", "pl", "pt-PT", "es-ES", "bg"]
        .iter()
        .any(|code| locale.eq_ignore_ascii_case(code))
        && digits.len() <= 4
    {
        return digits.to_owned();
    }
    group_digits(
        digits,
        data.group,
        data.group_primary as usize,
        data.group_secondary as usize,
    )
}

/// Group a non-negative integer exactly, without an f64 round-trip.
///
/// Used by `format_integer` and by relative-time formatting, so magnitudes above 2^53
/// keep every digit (`i64::MAX as f64` would round). Grouping symbols, primary and
/// secondary group sizes come from the locale data.
#[must_use]
pub fn format_integer_unsigned(value: u64, locale: &str) -> String {
    let data = format_data::data(locale);
    group_integer(&value.to_string(), locale, data)
}

#[must_use]
pub fn format_integer(value: i64, locale: &str) -> String {
    let sign = if value < 0 { "-" } else { "" };
    format!(
        "{sign}{}",
        format_integer_unsigned(value.unsigned_abs(), locale)
    )
}
#[must_use]
pub fn format_percent(ratio: f64, locale: &str) -> String {
    format!(
        "{}%",
        format_number_with(
            ratio * 100.0,
            locale,
            NumberOptions {
                maximum_fraction_digits: 0,
                ..NumberOptions::default()
            }
        )
    )
}
#[must_use]
pub fn format_currency(value: f64, locale: &str, currency: &str) -> String {
    let decimals = if currency == "JPY" { 0 } else { 2 };
    let number = format_number_with(
        value,
        locale,
        NumberOptions {
            minimum_fraction_digits: decimals,
            maximum_fraction_digits: decimals,
            ..NumberOptions::default()
        },
    );
    let (symbol, prefix) = match currency {
        "USD" => ("$", locale.starts_with("en")),
        "EUR" => ("€", false),
        "RUB" => ("₽", false),
        "GBP" => ("£", locale.starts_with("en")),
        "JPY" => ("¥", true),
        _ => return format!("{value:.2} {currency}"),
    };
    if prefix {
        format!("{symbol}{number}")
    } else {
        format!("{number} {symbol}")
    }
}

#[derive(Clone, Copy)]
struct DateParts {
    year: i64,
    month: usize,
    day: u32,
    weekday: usize,
    hour: u32,
    minute: u32,
    second: u32,
}
fn parts(timestamp_secs: i64, offset: i32) -> DateParts {
    let days = timestamp_secs.div_euclid(86_400);
    let seconds = timestamp_secs.rem_euclid(86_400);
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let mut year = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let day = doy - (153 * mp + 2) / 5 + 1;
    let month = mp + if mp < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    DateParts {
        year: year + i64::from(offset),
        month: month as usize,
        day: day as u32,
        weekday: (days + 4).rem_euclid(7) as usize,
        hour: (seconds / 3600) as u32,
        minute: ((seconds % 3600) / 60) as u32,
        second: (seconds % 60) as u32,
    }
}

fn render_pattern(
    pattern: &str,
    p: DateParts,
    data: &LocaleFormatData,
    skeleton: bool,
    weekdays_long: Option<&[&'static str; 7]>,
) -> Option<String> {
    let chars: Vec<char> = pattern.chars().collect();
    let mut out = String::new();
    let mut pos = 0;
    while pos < chars.len() {
        if chars[pos] == '\'' {
            pos += 1;
            while pos < chars.len() {
                if chars[pos] == '\'' {
                    if chars.get(pos + 1) == Some(&'\'') {
                        out.push('\'');
                        pos += 2;
                    } else {
                        pos += 1;
                        break;
                    }
                } else {
                    out.push(chars[pos]);
                    pos += 1;
                }
            }
            continue;
        }
        let c = chars[pos];
        if c.is_ascii_alphabetic() {
            let start = pos;
            while pos < chars.len() && chars[pos] == c {
                pos += 1;
            }
            let width = pos - start;
            let value = match (c, width) {
                ('y', 1) => p.year.to_string(),
                ('y', 2) => format!("{:02}", p.year.rem_euclid(100)),
                ('y', _) => format!("{:0width$}", p.year, width = width),
                ('M', 1) => p.month.to_string(),
                ('M', 2) => format!("{:02}", p.month),
                ('M', 3) => data.months_short[p.month - 1].to_owned(),
                ('M', _) => data.months_long[p.month - 1].to_owned(),
                ('d', 1) => p.day.to_string(),
                ('d', _) => format!("{:02}", p.day),
                ('E', 1..=3) => data.weekdays_short[p.weekday].to_owned(),
                ('E', _) => weekdays_long.map_or_else(
                    || data.weekdays_long[p.weekday].to_owned(),
                    |names| names[p.weekday].to_owned(),
                ),
                ('G', _) => data.era.to_owned(),
                ('H', 1) => p.hour.to_string(),
                ('H', _) => format!("{:02}", p.hour),
                ('h', 1) => ((p.hour + 11) % 12 + 1).to_string(),
                ('h', _) => format!("{:02}", (p.hour + 11) % 12 + 1),
                ('m', 1) => p.minute.to_string(),
                ('m', _) => format!("{:02}", p.minute),
                ('s', 1) => p.second.to_string(),
                ('s', _) => format!("{:02}", p.second),
                ('a', _) => data.day_periods[usize::from(p.hour >= 12)].to_owned(),
                ('z', 1..=3) => data.timezone_short.to_owned(),
                ('z', _) => data.timezone_long.to_owned(),
                _ => return if skeleton { None } else { Some(String::new()) },
            };
            out.push_str(&value);
        } else {
            out.push(c);
            pos += 1;
        }
    }
    Some(out.replace(['\u{202f}', '\u{a0}'], " "))
}

#[must_use]
pub fn format_date(timestamp_secs: i64, locale: &str, style: DateStyle) -> String {
    let data = format_data::data(locale);
    render_pattern(
        data.date_patterns[style.index()],
        parts(timestamp_secs, data.year_offset),
        data,
        false,
        None,
    )
    .unwrap_or_default()
}
#[must_use]
pub fn format_time(timestamp_secs: i64, locale: &str, style: DateStyle) -> String {
    let data = format_data::data(locale);
    render_pattern(
        data.time_patterns[style.index()],
        parts(timestamp_secs, 0),
        data,
        false,
        None,
    )
    .unwrap_or_default()
}
#[must_use]
pub fn format_date_skeleton(timestamp_secs: i64, locale: &str, skeleton: &str) -> Option<String> {
    let skeleton = skeleton.strip_prefix("::").unwrap_or(skeleton);
    let pattern = match skeleton {
        "M/d" => "M'/'d",
        "MMMMd" => "MMMM' 'd",
        "yyyy-MM-dd" => "yyyy'-'MM'-'dd",
        _ => return None,
    };
    let data = format_data::data(locale);
    let p = parts(timestamp_secs, data.year_offset);
    render_pattern(pattern, p, data, true, None)
}
#[must_use]
pub fn format_time_skeleton(timestamp_secs: i64, locale: &str, skeleton: &str) -> Option<String> {
    let skeleton = skeleton.strip_prefix("::").unwrap_or(skeleton);
    if skeleton != "HH:mm" {
        return None;
    }
    let data = format_data::data(locale);
    render_pattern("HH':'mm", parts(timestamp_secs, 0), data, true, None)
}

/// Render an arbitrary CLDR date/time pattern for `timestamp_secs` in `locale` (UTC).
///
/// Additive entry point for the generated Discord timestamp table (P7-01e): the token
/// set (`y M d E G H h m s a z`) and the NBSP/NNBSP normalization are the same as
/// `format_date`/`format_time`. `weekdays_long` overrides the standalone long weekday
/// names used by `E` tokens; some locales inflect the weekday in date context (Finnish
/// `lauantaina` versus standalone `lauantai`). Unknown tokens render as an empty string,
/// matching the non-skeleton behaviour of `format_date`.
#[must_use]
pub fn format_date_time_pattern(
    timestamp_secs: i64,
    locale: &str,
    pattern: &str,
    weekdays_long: Option<&[&'static str; 7]>,
) -> String {
    let data = format_data::data(locale);
    render_pattern(
        pattern,
        parts(timestamp_secs, data.year_offset),
        data,
        false,
        weekdays_long,
    )
    .unwrap_or_default()
}

#[must_use]
pub fn format_number_named(value: f64, locale: &str, name: &str) -> String {
    match name {
        "integer" => format_integer(value as i64, locale),
        "percent" => format_percent(value, locale),
        "currency" => format_currency(value, locale, "USD"),
        _ => format_number(value, locale),
    }
}
