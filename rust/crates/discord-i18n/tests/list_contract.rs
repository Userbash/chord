//! Contract tests for `Intl.ListFormat`-compatible list formatting (P7-01b, RW-27).
//!
//! Independent oracle: `waves/RW-27/fixtures/list-oracle.json`, generated from Node 24
//! (`waves/RW-27/gen-list-data.mjs`). Expectations are never derived from `list_data.rs`.

use discord_i18n::list::{ListStyle, ListType, format_list};

const ORACLE: &str = include_str!("../../../../waves/RW-27/fixtures/list-oracle.json");
const FIVE: [&str; 5] = ["A", "B", "C", "D", "E"];

fn style(name: &str) -> ListStyle {
    match name {
        "long" => ListStyle::Long,
        "short" => ListStyle::Short,
        "narrow" => ListStyle::Narrow,
        other => panic!("unknown oracle style: {other}"),
    }
}

fn kind(name: &str) -> ListType {
    match name {
        "conjunction" => ListType::Conjunction,
        "disjunction" => ListType::Disjunction,
        "unit" => ListType::Unit,
        other => panic!("unknown oracle type: {other}"),
    }
}

#[test]
fn en_us_long_conjunction_scales_from_empty_to_five() {
    assert_eq!(
        format_list("en-US", &[], ListStyle::Long, ListType::Conjunction),
        ""
    );
    assert_eq!(
        format_list("en-US", &["A"], ListStyle::Long, ListType::Conjunction),
        "A"
    );
    assert_eq!(
        format_list("en-US", &["A", "B"], ListStyle::Long, ListType::Conjunction),
        "A and B"
    );
    assert_eq!(
        format_list(
            "en-US",
            &["A", "B", "C"],
            ListStyle::Long,
            ListType::Conjunction
        ),
        "A, B, and C"
    );
    assert_eq!(
        format_list("en-US", &FIVE, ListStyle::Long, ListType::Conjunction),
        "A, B, C, D, and E"
    );
}

#[test]
fn ru_long_conjunction_and_disjunction() {
    assert_eq!(
        format_list(
            "ru",
            &["A", "B", "C"],
            ListStyle::Long,
            ListType::Conjunction
        ),
        "A, B и C"
    );
    assert_eq!(
        format_list(
            "ru",
            &["A", "B", "C"],
            ListStyle::Long,
            ListType::Disjunction
        ),
        "A, B или C"
    );
    assert_eq!(
        format_list(
            "ru",
            &["A", "B", "C", "D"],
            ListStyle::Long,
            ListType::Conjunction
        ),
        "A, B, C и D"
    );
}

#[test]
fn zh_cn_long_unit_joins_without_separators() {
    assert_eq!(
        format_list("zh-CN", &["A", "B"], ListStyle::Long, ListType::Unit),
        "AB"
    );
    assert_eq!(
        format_list("zh-CN", &["A", "B", "C"], ListStyle::Long, ListType::Unit),
        "ABC"
    );
    assert_eq!(
        format_list(
            "zh-CN",
            &["A", "B", "C", "D"],
            ListStyle::Long,
            ListType::Unit
        ),
        "ABCD"
    );
}

#[test]
fn oracle_fixture_matches_all_1674_cases() {
    let document = discord_json::parse(ORACLE).expect("oracle fixture is valid JSON");
    assert_eq!(
        document
            .get("source")
            .and_then(discord_json::JsonValue::as_str),
        Some("Node 24 Intl.ListFormat")
    );
    let cases = document
        .get("cases")
        .and_then(discord_json::JsonValue::as_array)
        .expect("cases array");
    assert_eq!(cases.len(), 1674, "frozen oracle size");

    let mut checked = 0_usize;
    for case in cases {
        let locale = case
            .get("locale")
            .and_then(discord_json::JsonValue::as_str)
            .expect("case locale");
        let style_name = case
            .get("style")
            .and_then(discord_json::JsonValue::as_str)
            .expect("case style");
        let kind_name = case
            .get("type")
            .and_then(discord_json::JsonValue::as_str)
            .expect("case type");
        let items: Vec<&str> = case
            .get("items")
            .and_then(discord_json::JsonValue::as_array)
            .expect("case items")
            .iter()
            .map(|item| item.as_str().expect("item string"))
            .collect();
        let expected = case
            .get("expected")
            .and_then(discord_json::JsonValue::as_str)
            .expect("case expected");

        let actual = format_list(locale, &items, style(style_name), kind(kind_name));
        assert_eq!(
            actual, expected,
            "locale={locale} style={style_name} type={kind_name} items={items:?}"
        );
        checked += 1;
    }
    assert_eq!(checked, 1674, "every oracle case must be exercised");
}

#[test]
fn unknown_and_disabled_locales_fall_back_to_en_us() {
    for locale in ["xx", "he", "ar", "id", "pt-PT", "sk", ""] {
        assert_eq!(
            format_list(
                locale,
                &["A", "B", "C"],
                ListStyle::Long,
                ListType::Conjunction
            ),
            "A, B, and C",
            "locale {locale:?} must fall back to en-US"
        );
    }
    // pt-BR is enabled and now has its own ICU row (was a data gap in the first
    // generation); it must not silently use the en-US pattern.
    assert_eq!(
        format_list(
            "pt-BR",
            &["A", "B", "C"],
            ListStyle::Long,
            ListType::Conjunction
        ),
        "A, B e C"
    );
}

#[test]
fn short_and_narrow_styles_match_oracle_for_en_de_ja() {
    for (locale, style_name, kind_name, expected) in [
        ("en-US", "short", "conjunction", "A, B, & C"),
        ("en-US", "narrow", "unit", "A B C"),
        ("en-US", "narrow", "disjunction", "A, B, or C"),
        ("de", "short", "unit", "A, B und C"),
        ("de", "narrow", "disjunction", "A, B oder C"),
        ("ja", "short", "disjunction", "A、B、またはC"),
        ("ja", "narrow", "unit", "ABC"),
        ("ja", "narrow", "conjunction", "A、B、C"),
    ] {
        assert_eq!(
            format_list(locale, &["A", "B", "C"], style(style_name), kind(kind_name)),
            expected,
            "{locale}/{style_name}/{kind_name}"
        );
    }
}

#[test]
fn locale_codes_are_case_insensitive() {
    for (locale, expected) in [
        ("RU", "A, B и C"),
        ("EN-US", "A, B, & C"),
        ("De", "A, B und C"),
    ] {
        let (style_name, kind_name) = if locale == "EN-US" {
            ("short", "conjunction")
        } else {
            ("long", "conjunction")
        };
        assert_eq!(
            format_list(locale, &["A", "B", "C"], style(style_name), kind(kind_name)),
            expected,
            "locale {locale:?}"
        );
    }
    assert_eq!(
        format_list(
            "ZH-cn",
            &["A", "B", "C", "D"],
            ListStyle::Long,
            ListType::Unit
        ),
        "ABCD"
    );
}

#[test]
fn middle_separator_repeats_for_inner_items() {
    assert_eq!(
        format_list("de", &FIVE, ListStyle::Long, ListType::Disjunction),
        "A, B, C, D oder E"
    );
    assert_eq!(
        format_list("ru", &FIVE, ListStyle::Long, ListType::Unit),
        "A B C D E"
    );
    assert_eq!(
        format_list(
            "ja",
            &["A", "B", "C", "D"],
            ListStyle::Narrow,
            ListType::Conjunction
        ),
        "A、B、C、D"
    );
}

#[test]
fn pair_separator_depends_on_type_and_style() {
    for (locale, style_name, kind_name, expected) in [
        ("en-US", "long", "conjunction", "A and B"),
        ("en-US", "long", "disjunction", "A or B"),
        ("en-US", "long", "unit", "A, B"),
        ("en-US", "narrow", "unit", "A B"),
        ("de", "long", "conjunction", "A und B"),
        ("ja", "long", "disjunction", "AまたはB"),
        ("ru", "long", "unit", "A B"),
    ] {
        assert_eq!(
            format_list(locale, &["A", "B"], style(style_name), kind(kind_name)),
            expected,
            "{locale}/{style_name}/{kind_name}"
        );
    }
}

#[test]
fn empty_and_single_item_lists_are_locale_independent() {
    for locale in ["en-US", "ru", "ja", "xx", "he"] {
        assert_eq!(
            format_list(locale, &[], ListStyle::Short, ListType::Unit),
            "",
            "empty list for {locale:?}"
        );
        assert_eq!(
            format_list(locale, &["solo"], ListStyle::Narrow, ListType::Unit),
            "solo",
            "single item for {locale:?}"
        );
    }
}
