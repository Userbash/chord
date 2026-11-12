use chord_i18n::format::{
    DateStyle, NumberOptions, NumberStyle, format_currency, format_date, format_date_skeleton,
    format_integer, format_number, format_number_named, format_number_with, format_percent,
    format_time, format_time_skeleton,
};
use chord_i18n::locale::SUPPORTED_LOCALES;

fn oracle() -> chord_json::JsonValue {
    chord_json::parse(include_str!("data/cldr-oracle.json")).expect("valid oracle JSON")
}

#[test]
fn all_locale_numbers_match_the_independent_intl_oracle() {
    let oracle = oracle();
    let locales = oracle.get("locales").unwrap();
    for locale in SUPPORTED_LOCALES {
        let expected = locales.get(locale).unwrap().get("number").unwrap();
        for number in ["0", "1", "1.5", "1234", "1234567", "1234567.89"] {
            if let Some(want) = expected
                .get(number)
                .and_then(chord_json::JsonValue::as_str)
            {
                let got = format_number(number.parse().unwrap(), locale);
                assert_eq!(got, want, "{locale} {number}");
            }
        }
    }
}

#[test]
fn all_locale_dates_and_times_match_the_independent_intl_oracle() {
    let oracle = oracle();
    let timestamp = oracle.get("timestamp").unwrap().as_i64().unwrap();
    let timestamp2 = oracle.get("timestamp2").unwrap().as_i64().unwrap();
    let locales = oracle.get("locales").unwrap();
    let styles = [
        DateStyle::Short,
        DateStyle::Medium,
        DateStyle::Long,
        DateStyle::Full,
    ];
    let names = ["short", "medium", "long", "full"];
    for locale in SUPPORTED_LOCALES {
        let entry = locales.get(locale).unwrap();
        for (style, name) in styles.into_iter().zip(names) {
            let date = entry
                .get("dates")
                .unwrap()
                .get(name)
                .unwrap()
                .as_str()
                .unwrap();
            let time = entry
                .get("times")
                .unwrap()
                .get(name)
                .unwrap()
                .as_str()
                .unwrap();
            assert_eq!(
                format_date(timestamp, locale, style),
                date,
                "date {locale} {name}"
            );
            assert_eq!(
                format_time(timestamp, locale, style),
                time,
                "time {locale} {name}"
            );
        }
    }
    for locale in SUPPORTED_LOCALES {
        let entry = locales.get(locale).unwrap();
        for (style, name) in styles.into_iter().zip(names) {
            let date = entry
                .get("dates2")
                .unwrap()
                .get(name)
                .unwrap()
                .as_str()
                .unwrap();
            let time = entry
                .get("times2")
                .unwrap()
                .get(name)
                .unwrap()
                .as_str()
                .unwrap();
            assert_eq!(
                format_date(timestamp2, locale, style),
                date,
                "date2 {locale} {name}"
            );
            assert_eq!(
                format_time(timestamp2, locale, style),
                time,
                "time2 {locale} {name}"
            );
        }
    }
}

#[test]
fn locale_specific_number_options_currency_and_skeletons_are_supported() {
    assert_eq!(format_number(1_234_567.89, "ru"), "1\u{a0}234\u{a0}567,89");
    assert_eq!(
        format_number(1_234_567.89, "fr"),
        "1\u{202f}234\u{202f}567,89"
    );
    assert_eq!(format_number(123_456_789., "hi"), "12,34,56,789");
    assert_eq!(format_integer(-1234, "en-US"), "-1,234");
    assert_eq!(format_percent(0.25, "en-US"), "25%");
    assert_eq!(format_currency(1234.5, "en-US", "USD"), "$1,234.50");
    assert_eq!(format_currency(1234.5, "ru", "RUB"), "1\u{a0}234,50 ₽");
    assert_eq!(format_currency(1234.5, "de", "EUR"), "1.234,50 €");
    assert_eq!(format_currency(1234.5, "fr", "EUR"), "1\u{202f}234,50 €");
    assert_eq!(format_currency(1234.5, "ja", "JPY"), "¥1,235");
    assert_eq!(format_currency(1234.5, "en-US", "XXX"), "1234.50 XXX");
    assert_eq!(format_currency(1234.5, "ru", "XXX"), "1234.50 XXX");
    assert_eq!(
        format_number_with(
            1.5,
            "en-US",
            NumberOptions {
                style: NumberStyle::Decimal,
                minimum_fraction_digits: 2,
                maximum_fraction_digits: 2,
                use_grouping: false,
            }
        ),
        "1.50"
    );
    assert_eq!(format_number_named(1.5, "en-US", "other"), "1.5");
    assert!(format_date(1_712_326_029, "th", DateStyle::Full).contains("พ.ศ."));
    assert!(format_date(1_712_326_029, "th", DateStyle::Full).contains("2567"));
    assert_eq!(
        format_date(1_712_326_029, "ru", DateStyle::Long),
        "5 апреля 2024 г."
    );
    assert!(format_date_skeleton(1_712_326_029, "en-US", "M/d").is_some());
    assert_eq!(
        format_date_skeleton(1_712_326_029, "en-US", "::MMMMd").as_deref(),
        Some("April 5")
    );
    assert_eq!(format_date_skeleton(0, "en-US", "bogus"), None);
    assert!(format_time_skeleton(-1, "en-US", "HH:mm").is_some());
    assert_eq!(format_time_skeleton(0, "en-US", "bogus"), None);
    assert_eq!(format_number(f64::NAN, "en-US"), "0");
    assert_eq!(format_number(f64::INFINITY, "en-US"), "0");
}
