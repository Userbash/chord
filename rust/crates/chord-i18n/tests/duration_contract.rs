//! Contract tests for `Intl.DurationFormat`-compatible formatting (P7-01d, RW-30).
//!
//! Independent oracle: `tests/data/duration-oracle.json`, generated from Node 24
//! (`waves/RW-30/gen-duration-data.mjs`). Expectations are never derived from
//! `duration_data.rs`: the fixture is produced by ICU and only normalized
//! (NNBSP/NBSP → space), exactly like the implementation's output.

use chord_i18n::duration::{
    DURATION_JOIN_COUNT, DURATION_PATTERN_COUNT, DURATION_ROW_COUNT, Duration, DurationStyle,
    format_duration,
};

const ORACLE: &str = include_str!("data/duration-oracle.json");

fn style(name: &str) -> DurationStyle {
    match name {
        "long" => DurationStyle::Long,
        "short" => DurationStyle::Short,
        "narrow" => DurationStyle::Narrow,
        other => panic!("unknown oracle style: {other}"),
    }
}

fn duration(fields: &[(&str, u64)]) -> Duration {
    let mut duration = Duration::default();
    for &(name, value) in fields {
        match name {
            "years" => duration.years = value,
            "months" => duration.months = value,
            "weeks" => duration.weeks = value,
            "days" => duration.days = value,
            "hours" => duration.hours = value,
            "minutes" => duration.minutes = value,
            "seconds" => duration.seconds = value,
            "milliseconds" => duration.milliseconds = value,
            other => panic!("unknown duration field: {other}"),
        }
    }
    duration
}

fn oracle_duration(case: &chord_json::JsonValue) -> Duration {
    let Some(object) = case
        .as_object()
        .and_then(|case| case.get("d"))
        .and_then(chord_json::JsonValue::as_object)
    else {
        return Duration::default();
    };
    let mut duration = Duration::default();
    for (name, value) in object {
        let number = value
            .as_u64()
            .or_else(|| value.as_str().and_then(|text| text.parse().ok()))
            .unwrap_or_else(|| panic!("oracle duration field {name} is not a u64"));
        match name.as_str() {
            "years" => duration.years = number,
            "months" => duration.months = number,
            "weeks" => duration.weeks = number,
            "days" => duration.days = number,
            "hours" => duration.hours = number,
            "minutes" => duration.minutes = number,
            "seconds" => duration.seconds = number,
            "milliseconds" => duration.milliseconds = number,
            other => panic!("unknown oracle duration field: {other}"),
        }
    }
    duration
}

fn locale_style_rows() -> Vec<(&'static str, DurationStyle)> {
    let locales = [
        "en-US", "en-GB", "zh-CN", "zh-TW", "cs", "da", "nl", "fr", "de", "el", "hu", "it", "ja",
        "ko", "pl", "pt-BR", "ru", "es-419", "es-ES", "sv-SE", "tr", "bg", "uk", "fi", "no", "hr",
        "ro", "lt", "th", "vi", "hi",
    ];
    let mut rows = Vec::new();
    for locale in locales {
        for style in [
            DurationStyle::Long,
            DurationStyle::Short,
            DurationStyle::Narrow,
        ] {
            rows.push((locale, style));
        }
    }
    rows
}

#[test]
fn d01_en_long_hours_minutes_seconds() {
    assert_eq!(
        format_duration(
            "en-US",
            &duration(&[("hours", 1), ("minutes", 2), ("seconds", 3)]),
            DurationStyle::Long
        ),
        "1 hour, 2 minutes, 3 seconds"
    );
}

#[test]
fn d02_ru_long_plural_forms() {
    assert_eq!(
        format_duration(
            "ru",
            &duration(&[("hours", 1), ("minutes", 2), ("seconds", 3)]),
            DurationStyle::Long
        ),
        "1 час 2 минуты 3 секунды"
    );
}

#[test]
fn d03_de_long_conjunction_before_last() {
    assert_eq!(
        format_duration(
            "de",
            &duration(&[("hours", 1), ("minutes", 2), ("seconds", 3)]),
            DurationStyle::Long
        ),
        "1 Stunde, 2 Minuten und 3 Sekunden"
    );
}

#[test]
fn d04_en_narrow_compact_units() {
    assert_eq!(
        format_duration(
            "en-US",
            &duration(&[("minutes", 5), ("seconds", 9)]),
            DurationStyle::Narrow
        ),
        "5m 9s"
    );
}

#[test]
fn d05_zero_units_are_skipped_and_all_zero_is_empty() {
    assert_eq!(
        format_duration(
            "en-US",
            &duration(&[("days", 1), ("hours", 0), ("minutes", 2)]),
            DurationStyle::Long
        ),
        "1 day, 2 minutes"
    );
    assert_eq!(
        format_duration(
            "de",
            &duration(&[("days", 1), ("hours", 0), ("minutes", 2), ("seconds", 0)]),
            DurationStyle::Short
        ),
        format_duration(
            "de",
            &duration(&[("days", 1), ("minutes", 2)]),
            DurationStyle::Short
        )
    );
    for (locale, style) in locale_style_rows() {
        assert_eq!(
            format_duration(locale, &Duration::default(), style),
            "",
            "{locale}/{style:?}: пустая длительность"
        );
    }
}

#[test]
fn d06_grouping_follows_icu_per_locale() {
    let thousand_days = duration(&[("days", 1_000)]);
    assert_eq!(
        format_duration("en-US", &thousand_days, DurationStyle::Long),
        "1,000 days"
    );
    assert_eq!(
        format_duration("ru", &thousand_days, DurationStyle::Long),
        "1 000 дней"
    );
    assert_eq!(
        format_duration("de", &thousand_days, DurationStyle::Long),
        "1.000 Tage"
    );
    assert_eq!(
        format_duration(
            "en-US",
            &duration(&[("days", 10_000_000)]),
            DurationStyle::Long
        ),
        "10,000,000 days"
    );
    assert_eq!(
        format_duration(
            "ru",
            &duration(&[("days", 10_000_000)]),
            DurationStyle::Long
        ),
        "10 000 000 дней"
    );
    // NNBSP/NBSP нормализуются в обычный пробел (как в `render_pattern`).
    for (locale, style) in locale_style_rows() {
        let out = format_duration(
            locale,
            &duration(&[("days", 1_000), ("hours", 1_234)]),
            style,
        );
        assert!(
            !out.contains('\u{202f}') && !out.contains('\u{a0}'),
            "{locale}/{style:?}: не нормализованы пробелы: {out:?}"
        );
    }
}

#[test]
fn d07_plural_categories_follow_cldr() {
    for (value, expected) in [
        (1, "1 день"),
        (2, "2 дня"),
        (5, "5 дней"),
        (11, "11 дней"),
        (21, "21 день"),
        (101, "101 день"),
    ] {
        assert_eq!(
            format_duration("ru", &duration(&[("days", value)]), DurationStyle::Long),
            expected,
            "ru days {value}"
        );
    }
    for (value, expected) in [
        (1, "1 dzień"),
        (2, "2 dni"),
        (5, "5 dni"),
        (21, "21 dni"),
        (22, "22 dni"),
    ] {
        assert_eq!(
            format_duration("pl", &duration(&[("days", value)]), DurationStyle::Long),
            expected,
            "pl days {value}"
        );
    }
    assert_eq!(
        format_duration("en-US", &duration(&[("days", 1)]), DurationStyle::Long),
        "1 day"
    );
    assert_eq!(
        format_duration("en-US", &duration(&[("days", 2)]), DurationStyle::Long),
        "2 days"
    );
    // Романские локали: `many` достижим только на кратных 10^6.
    assert_eq!(
        format_duration("fr", &duration(&[("days", 1_000_000)]), DurationStyle::Long),
        "1 000 000 jours"
    );
}

#[test]
fn d08_all_units_keep_the_frozen_order() {
    let all = duration(&[
        ("years", 1),
        ("months", 2),
        ("weeks", 3),
        ("days", 4),
        ("hours", 5),
        ("minutes", 6),
        ("seconds", 7),
        ("milliseconds", 8),
    ]);
    assert_eq!(
        format_duration("en-US", &all, DurationStyle::Long),
        "1 year, 2 months, 3 weeks, 4 days, 5 hours, 6 minutes, 7 seconds, 8 milliseconds"
    );
    assert_eq!(
        format_duration("ru", &all, DurationStyle::Long),
        "1 год 2 месяца 3 недели 4 дня 5 часов 6 минут 7 секунд 8 миллисекунд"
    );
    assert_eq!(
        format_duration("en-US", &all, DurationStyle::Narrow),
        "1y 2m 3w 4d 5h 6m 7s 8ms"
    );
}

#[test]
fn d09_unknown_and_disabled_locales_fall_back_to_en_us() {
    let value = duration(&[("hours", 3), ("minutes", 4), ("seconds", 5)]);
    let expected = format_duration("en-US", &value, DurationStyle::Long);
    for locale in ["xx", "", "he", "ar", "pt-PT", "de-DE"] {
        assert_eq!(
            format_duration(locale, &value, DurationStyle::Long),
            expected,
            "локаль {locale:?}"
        );
    }
    // Канонизация регистра работает как в `crate::locale`.
    assert_eq!(
        format_duration("RU", &duration(&[("days", 1)]), DurationStyle::Long),
        "1 день"
    );
}

#[test]
fn d10_extremes_and_empty_do_not_panic() {
    assert_eq!(
        format_duration("en-US", &Duration::default(), DurationStyle::Long),
        ""
    );
    assert_eq!(
        format_duration(
            "en-US",
            &duration(&[("days", u64::MAX)]),
            DurationStyle::Long
        ),
        "18,446,744,073,709,551,615 days"
    );
    assert_eq!(
        format_duration("ru", &duration(&[("days", u64::MAX)]), DurationStyle::Long),
        "18 446 744 073 709 551 615 дней"
    );
    let saturated = Duration {
        years: u64::MAX,
        months: u64::MAX,
        weeks: u64::MAX,
        days: u64::MAX,
        hours: u64::MAX,
        minutes: u64::MAX,
        seconds: u64::MAX,
        milliseconds: u64::MAX,
    };
    for (locale, style) in locale_style_rows() {
        let out = format_duration(locale, &saturated, style);
        assert!(!out.is_empty(), "{locale}/{style:?}");
        assert!(
            !out.contains('{') && !out.contains('}'),
            "{locale}/{style:?}: {out}"
        );
    }
}

#[test]
fn d11_full_oracle_matches_icu_exactly() {
    let document = chord_json::parse(ORACLE).expect("oracle fixture is valid JSON");
    let object = document.as_object().expect("oracle object");
    let locales = object
        .get("locales")
        .and_then(chord_json::JsonValue::as_array)
        .expect("oracle locales");
    assert_eq!(locales.len(), 31, "включённых локалей");
    let cases = object
        .get("cases")
        .and_then(chord_json::JsonValue::as_array)
        .expect("oracle cases");
    assert!(cases.len() >= 4_000, "кейсов: {}", cases.len());
    let counts = object
        .get("counts")
        .and_then(chord_json::JsonValue::as_object)
        .expect("oracle counts");
    let meta = |name: &str| {
        counts
            .get(name)
            .and_then(chord_json::JsonValue::as_u64)
            .unwrap_or_else(|| panic!("oracle count {name}"))
    };
    assert_eq!(meta("rows"), DURATION_ROW_COUNT as u64);
    assert_eq!(meta("joins"), DURATION_JOIN_COUNT as u64);
    assert_eq!(meta("patterns"), DURATION_PATTERN_COUNT as u64);
    assert!(
        meta("randomCases") >= 2_000,
        "случайных кейсов: {}",
        meta("randomCases")
    );
    assert_eq!(meta("edgeCases") + meta("randomCases"), meta("cases"));
    assert_eq!(meta("cases"), cases.len() as u64);

    let mut seen_locales = std::collections::BTreeSet::new();
    let mut seen_styles = std::collections::BTreeSet::new();
    for case in cases {
        let field = |name: &str| {
            case.as_object()
                .and_then(|case| case.get(name))
                .and_then(chord_json::JsonValue::as_str)
                .unwrap_or_else(|| panic!("oracle field {name}"))
        };
        let locale = field("locale");
        let style_name = field("style");
        let expected = field("expected");
        seen_locales.insert(locale.to_owned());
        seen_styles.insert(style_name.to_owned());
        let actual = format_duration(locale, &oracle_duration(case), style(style_name));
        assert_eq!(actual, expected, "{locale}/{style_name}: {case:?}");
    }
    assert_eq!(seen_locales.len(), 31, "локали в кейсах: {seen_locales:?}");
    assert_eq!(seen_styles.len(), 3, "стили в кейсах: {seen_styles:?}");
}

#[test]
fn d12_generated_table_invariants() {
    assert_eq!(DURATION_ROW_COUNT, 31 * 3 * 8, "строк таблицы");
    assert_eq!(DURATION_JOIN_COUNT, 31 * 3, "join-строк таблицы");
    assert_eq!(
        DURATION_PATTERN_COUNT,
        31 * 3 * 8 * 6,
        "паттернов (6 категорий)"
    );

    // Разделители: либо все три непустые, либо все три пустые (локали без пробелов).
    // Набор пустых троек зафиксирован по ICU 78 и проверяется независимо от реализации.
    let empty_joins = [
        ("ja", "narrow"),
        ("zh-CN", "long"),
        ("zh-CN", "short"),
        ("zh-CN", "narrow"),
        ("zh-TW", "narrow"),
    ];
    for (locale, style) in locale_style_rows() {
        let style_name = match style {
            DurationStyle::Long => "long",
            DurationStyle::Short => "short",
            DurationStyle::Narrow => "narrow",
        };
        let one = format_duration(locale, &duration(&[("days", 2)]), style);
        let two = format_duration(locale, &duration(&[("days", 2), ("hours", 3)]), style);
        let three = format_duration(
            locale,
            &duration(&[("days", 2), ("hours", 3), ("minutes", 4)]),
            style,
        );
        assert!(!one.contains('{'), "{locale}/{style_name}: {one}");
        assert!(
            two.starts_with(&one),
            "{locale}/{style_name}: {two} vs {one}"
        );
        assert!(
            three.starts_with(&one),
            "{locale}/{style_name}: {three} vs {one}"
        );
        let has_empty = empty_joins.contains(&(locale, style_name));
        let hours_only = format_duration(locale, &duration(&[("hours", 3)]), style);
        let adjacent = format!("{one}{hours_only}");
        if has_empty {
            assert_eq!(two, adjacent, "{locale}/{style_name}: пустой join2");
        } else {
            assert_ne!(two, adjacent, "{locale}/{style_name}: join2 потерян");
        }
    }
}

#[test]
fn d13_formatting_is_deterministic() {
    let value = duration(&[
        ("years", 3),
        ("months", 1),
        ("days", 21),
        ("hours", 1_000),
        ("minutes", 2),
        ("milliseconds", 999),
    ]);
    for (locale, style) in locale_style_rows() {
        let first = format_duration(locale, &value, style);
        let second = format_duration(locale, &value, style);
        assert_eq!(first, second, "{locale}/{style:?}");
    }
}
