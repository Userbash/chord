use chord_i18n::locale::SUPPORTED_LOCALES;
use chord_i18n::plural::{PluralCategory, PluralType, cardinal, ordinal};

fn oracle() -> chord_json::JsonValue {
    chord_json::parse(include_str!("data/cldr-oracle.json")).expect("valid oracle JSON")
}

#[test]
fn every_oracle_cardinal_and_ordinal_sample_matches() {
    let oracle = oracle();
    let locales = oracle.get("locales").unwrap();
    for locale in SUPPORTED_LOCALES {
        let entry = locales.get(locale).unwrap();
        for (key, kind) in [
            ("cardinal", PluralType::Cardinal),
            ("ordinal", PluralType::Ordinal),
        ] {
            let samples = entry
                .get("plural")
                .unwrap()
                .get(key)
                .unwrap()
                .as_object()
                .unwrap();
            for (number, expected) in samples {
                let value: f64 = number.parse().unwrap();
                let got = match kind {
                    PluralType::Cardinal => cardinal(locale, value),
                    PluralType::Ordinal => ordinal(locale, value),
                };
                assert_eq!(
                    got.as_str(),
                    expected.as_str().unwrap(),
                    "{locale} {key} {number}"
                );
            }
        }
    }
}

#[test]
fn plural_edge_cases_follow_the_locale_contract() {
    for (n, expected) in [
        (1., "one"),
        (2., "few"),
        (5., "many"),
        (11., "many"),
        (21., "one"),
        (22., "few"),
        (101., "one"),
        (-1., "one"),
    ] {
        assert_eq!(cardinal("ru", n).as_str(), expected, "ru {n}");
    }
    assert_eq!(cardinal("ru", 1.5), PluralCategory::Other);
    assert_eq!(cardinal("da", 1.5), PluralCategory::One);
    assert_eq!(cardinal("he", 0.5), PluralCategory::One);
    for (n, expected) in [
        (0., "zero"),
        (1., "one"),
        (2., "two"),
        (3., "few"),
        (11., "many"),
        (3.5, "other"),
        (11.5, "other"),
    ] {
        assert_eq!(cardinal("ar", n).as_str(), expected, "ar {n}");
    }
    assert_eq!(ordinal("uk", 3.5).as_str(), "other");
    for (n, expected) in [(1., "one"), (2., "two"), (4., "few"), (6., "many")] {
        assert_eq!(ordinal("hi", n).as_str(), expected, "hi ordinal {n}");
    }
}
