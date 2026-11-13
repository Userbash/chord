//! Contract tests for `Intl.RelativeTimeFormat`-compatible formatting (P7-01c, RW-28).
//!
//! Independent oracle: `tests/data/relative-oracle.json`, generated from Node 24
//! (`waves/RW-28/gen-relative-data.mjs`). Expectations are never derived from
//! `relative_data.rs`.

use chord_i18n::relative::{
    RELATIVE_ROW_COUNT, RelativeNumeric, RelativeStyle, RelativeUnit, format_relative,
};

const ORACLE: &str = include_str!("data/relative-oracle.json");

fn unit(name: &str) -> RelativeUnit {
    match name {
        "year" => RelativeUnit::Year,
        "quarter" => RelativeUnit::Quarter,
        "month" => RelativeUnit::Month,
        "week" => RelativeUnit::Week,
        "day" => RelativeUnit::Day,
        "hour" => RelativeUnit::Hour,
        "minute" => RelativeUnit::Minute,
        "second" => RelativeUnit::Second,
        other => panic!("unknown oracle unit: {other}"),
    }
}

fn style(name: &str) -> RelativeStyle {
    match name {
        "long" => RelativeStyle::Long,
        "short" => RelativeStyle::Short,
        "narrow" => RelativeStyle::Narrow,
        other => panic!("unknown oracle style: {other}"),
    }
}

fn numeric(name: &str) -> RelativeNumeric {
    match name {
        "always" => RelativeNumeric::Always,
        "auto" => RelativeNumeric::Auto,
        other => panic!("unknown oracle numeric mode: {other}"),
    }
}

#[test]
fn t01_en_long_always_past_future_and_grouping() {
    let day = RelativeUnit::Day;
    let long = RelativeStyle::Long;
    let always = RelativeNumeric::Always;
    assert_eq!(
        format_relative("en-US", -2, day, long, always),
        "2 days ago"
    );
    assert_eq!(format_relative("en-US", 2, day, long, always), "in 2 days");
    assert_eq!(
        format_relative("en-US", -1000, day, long, always),
        "1,000 days ago"
    );
}

#[test]
fn t02_ru_plural_categories_past_and_future() {
    let day = RelativeUnit::Day;
    let long = RelativeStyle::Long;
    let always = RelativeNumeric::Always;
    for (value, expected) in [
        (-5, "5 дней назад"),
        (-2, "2 дня назад"),
        (-1, "1 день назад"),
        (1, "через 1 день"),
        (2, "через 2 дня"),
        (5, "через 5 дней"),
    ] {
        assert_eq!(
            format_relative("ru", value, day, long, always),
            expected,
            "value {value}"
        );
    }
}

#[test]
fn t03_other_locales_match_icu() {
    assert_eq!(
        format_relative(
            "de",
            3,
            RelativeUnit::Hour,
            RelativeStyle::Short,
            RelativeNumeric::Always
        ),
        "in 3 Std."
    );
    assert_eq!(
        format_relative(
            "ja",
            5,
            RelativeUnit::Minute,
            RelativeStyle::Narrow,
            RelativeNumeric::Always
        ),
        "5分後"
    );
    assert_eq!(
        format_relative(
            "zh-CN",
            -2,
            RelativeUnit::Day,
            RelativeStyle::Long,
            RelativeNumeric::Always
        ),
        "2天前"
    );
    assert_eq!(
        format_relative(
            "es-ES",
            1,
            RelativeUnit::Week,
            RelativeStyle::Long,
            RelativeNumeric::Auto
        ),
        "la próxima semana"
    );
}

#[test]
fn t04_auto_special_forms_follow_icu() {
    let day = RelativeUnit::Day;
    let long = RelativeStyle::Long;
    let auto = RelativeNumeric::Auto;
    assert_eq!(format_relative("en-US", -1, day, long, auto), "yesterday");
    assert_eq!(format_relative("en-US", 0, day, long, auto), "today");
    assert_eq!(format_relative("en-US", 1, day, long, auto), "tomorrow");
    // ICU has day-specific ±2 forms in some locales; the frozen table mirrors them.
    assert_eq!(format_relative("ru", -2, day, long, auto), "позавчера");
    assert_eq!(format_relative("ru", 2, day, long, auto), "послезавтра");
    assert_eq!(format_relative("de", -2, day, long, auto), "vorgestern");
    assert_eq!(format_relative("ja", 2, day, long, auto), "明後日");
    // Units without ±2 data fall back to the numeric pattern.
    for unit in [
        RelativeUnit::Year,
        RelativeUnit::Quarter,
        RelativeUnit::Month,
        RelativeUnit::Week,
        RelativeUnit::Hour,
        RelativeUnit::Minute,
        RelativeUnit::Second,
    ] {
        assert_eq!(
            format_relative("ru", 2, unit, long, auto),
            format_relative("ru", 2, unit, long, RelativeNumeric::Always),
            "{unit:?}"
        );
        assert_eq!(
            format_relative("en-US", -2, unit, long, auto),
            format_relative("en-US", -2, unit, long, RelativeNumeric::Always),
            "{unit:?}"
        );
    }
}

#[test]
fn t05_auto_falls_back_to_numeric_outside_specials() {
    let day = RelativeUnit::Day;
    let long = RelativeStyle::Long;
    assert_eq!(
        format_relative("en-US", 2, day, long, RelativeNumeric::Auto),
        "in 2 days"
    );
    assert_eq!(
        format_relative("ru", 5, day, long, RelativeNumeric::Auto),
        format_relative("ru", 5, day, long, RelativeNumeric::Always)
    );
}

#[test]
fn t06_short_and_narrow_styles() {
    let hour = RelativeUnit::Hour;
    let auto = RelativeNumeric::Auto;
    assert_eq!(
        format_relative("en-US", -1, hour, RelativeStyle::Short, auto),
        "1 hr. ago"
    );
    assert_eq!(
        format_relative("en-US", 0, hour, RelativeStyle::Long, auto),
        "this hour"
    );
    assert_eq!(
        format_relative(
            "ru",
            2,
            RelativeUnit::Minute,
            RelativeStyle::Narrow,
            RelativeNumeric::Always
        ),
        "+2 мин"
    );
}

#[test]
fn t07_all_units_are_supported() {
    let long = RelativeStyle::Long;
    let always = RelativeNumeric::Always;
    for (unit, expected) in [
        (RelativeUnit::Year, "in 2 years"),
        (RelativeUnit::Quarter, "in 2 quarters"),
        (RelativeUnit::Month, "in 2 months"),
        (RelativeUnit::Week, "in 2 weeks"),
        (RelativeUnit::Day, "in 2 days"),
        (RelativeUnit::Hour, "in 2 hours"),
        (RelativeUnit::Minute, "in 2 minutes"),
        (RelativeUnit::Second, "in 2 seconds"),
    ] {
        assert_eq!(format_relative("en-US", 2, unit, long, always), expected);
    }
}

#[test]
fn t08_unknown_and_disabled_locales_fall_back_to_en_us() {
    let day = RelativeUnit::Day;
    let long = RelativeStyle::Long;
    let always = RelativeNumeric::Always;
    let expected = format_relative("en-US", -3, day, long, always);
    assert_eq!(format_relative("xx", -3, day, long, always), expected);
    assert_eq!(format_relative("he", -3, day, long, always), expected);
}

#[test]
fn t09_oracle_matrix_matches_exactly() {
    let document = chord_json::parse(ORACLE).expect("oracle fixture is valid JSON");
    let locales = document
        .as_object()
        .and_then(|object| object.get("locales"))
        .and_then(chord_json::JsonValue::as_array)
        .expect("oracle locales");
    assert_eq!(locales.len(), 31, "включённых локалей");
    let expected_locales = [
        "bg", "cs", "da", "de", "el", "en-GB", "en-US", "es-419", "es-ES", "fi", "fr", "hi", "hr",
        "hu", "it", "ja", "ko", "lt", "nl", "no", "pl", "pt-BR", "ro", "ru", "sv-SE", "th", "tr",
        "uk", "vi", "zh-CN", "zh-TW",
    ];
    let mut actual_locales: Vec<&str> = locales
        .iter()
        .map(|value| value.as_str().expect("oracle locale"))
        .collect();
    actual_locales.sort_unstable();
    let mut expected_sorted = expected_locales.to_vec();
    expected_sorted.sort_unstable();
    assert_eq!(actual_locales, expected_sorted, "набор включённых локалей");
    let cases = document
        .as_object()
        .and_then(|object| object.get("cases"))
        .and_then(chord_json::JsonValue::as_array)
        .expect("oracle cases");
    assert!(cases.len() >= 3000, "кейсов: {}", cases.len());
    let mut units = std::collections::BTreeSet::new();
    let mut styles = std::collections::BTreeSet::new();
    let mut numerics = std::collections::BTreeSet::new();
    for case in cases {
        if let Some(object) = case.as_object() {
            if let Some(value) = object.get("unit").and_then(chord_json::JsonValue::as_str) {
                units.insert(value.to_owned());
            }
            if let Some(value) = object.get("style").and_then(chord_json::JsonValue::as_str) {
                styles.insert(value.to_owned());
            }
            if let Some(value) = object
                .get("numeric")
                .and_then(chord_json::JsonValue::as_str)
            {
                numerics.insert(value.to_owned());
            }
        }
    }
    assert_eq!(units.len(), 8, "единицы: {units:?}");
    assert_eq!(styles.len(), 3, "стили: {styles:?}");
    assert_eq!(numerics.len(), 2, "numeric: {numerics:?}");
    for case in cases {
        let field = |name: &str| {
            case.as_object()
                .and_then(|object| object.get(name))
                .and_then(chord_json::JsonValue::as_str)
                .unwrap_or_else(|| panic!("oracle field {name}"))
        };
        let value = case
            .as_object()
            .and_then(|object| object.get("value"))
            .and_then(chord_json::JsonValue::as_i64)
            .expect("oracle value");
        let locale = field("locale");
        let actual = format_relative(
            locale,
            value,
            unit(field("unit")),
            style(field("style")),
            numeric(field("numeric")),
        );
        assert_eq!(
            actual,
            field("expected"),
            "{locale} {value} {}",
            field("unit")
        );
    }
}

#[test]
fn t10_zero_and_extreme_values_do_not_panic() {
    let second = RelativeUnit::Second;
    let long = RelativeStyle::Long;
    let always = RelativeNumeric::Always;
    assert_eq!(
        format_relative("en-US", 0, second, long, always),
        "in 0 seconds"
    );
    assert_eq!(
        format_relative("en-US", -1, second, long, always),
        "1 second ago"
    );
    let extreme = format_relative("en-US", i64::MIN, second, long, always);
    assert_eq!(
        extreme, "9,223,372,036,854,775,808 seconds ago",
        "точные цифры i64::MIN"
    );
    assert!(extreme.contains("ago"), "{extreme}");
    assert!(!extreme.is_empty());
}

#[test]
fn t11_no_unsubstituted_placeholders_across_all_rows() {
    assert_eq!(RELATIVE_ROW_COUNT, 31 * 8 * 3, "строк таблицы");
    let locales = [
        "en-US", "en-GB", "zh-CN", "zh-TW", "cs", "da", "nl", "fr", "de", "el", "hu", "it", "ja",
        "ko", "pl", "pt-BR", "ru", "es-419", "es-ES", "sv-SE", "tr", "bg", "uk", "fi", "no", "hr",
        "ro", "lt", "th", "vi", "hi",
    ];
    let units = [
        RelativeUnit::Year,
        RelativeUnit::Quarter,
        RelativeUnit::Month,
        RelativeUnit::Week,
        RelativeUnit::Day,
        RelativeUnit::Hour,
        RelativeUnit::Minute,
        RelativeUnit::Second,
    ];
    for locale in locales {
        for unit in units {
            for style in [
                RelativeStyle::Long,
                RelativeStyle::Short,
                RelativeStyle::Narrow,
            ] {
                for numeric in [RelativeNumeric::Always, RelativeNumeric::Auto] {
                    for value in [-2, -1, 0, 1, 2, 11] {
                        let out = format_relative(locale, value, unit, style, numeric);
                        assert!(!out.is_empty(), "{locale} {unit:?} {style:?} {value}");
                        assert!(
                            !out.contains('{') && !out.contains('}'),
                            "незамещённый плейсхолдер: {locale} {unit:?} {style:?} {numeric:?} {value} → {out}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn t12_integer_values_above_f64_precision_keep_digits_and_category() {
    let second = RelativeUnit::Second;
    let long = RelativeStyle::Long;
    let always = RelativeNumeric::Always;
    assert_eq!(
        format_relative("en-US", 9_007_199_254_740_993, second, long, always),
        "in 9,007,199,254,740,993 seconds"
    );
    assert_eq!(
        format_relative("en-US", i64::MAX, second, long, always),
        "in 9,223,372,036,854,775,807 seconds"
    );
    let grouped = chord_i18n::format::format_integer_unsigned(1_000_000_000_000_000_001, "ru");
    assert_eq!(
        format_relative("ru", 1_000_000_000_000_000_001, second, long, always),
        format!("через {grouped} секунду")
    );
    assert_eq!(
        format_relative("ru", -1_000_000_000_000_000_001, second, long, always),
        format!("{grouped} секунду назад")
    );
}
