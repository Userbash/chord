//! T-I18N-07/08 integration contract: registry, real catalogs, fallback and AST wiring.
//!
//! Oracle: `tests/data/cldr-oracle.json` (Node ICU, UTC) and the real ru/en catalogs
//! under `/home/sanya/re-deep` (read-only, skip-aware when absent).

use std::collections::BTreeMap;

use discord_i18n::format::{self, DateStyle};
use discord_i18n::locale::{DISABLED_LOCALES, SUPPORTED_LOCALES, TOTAL_LOCALES};
use discord_i18n::{Catalog, I18n, TranslateError, Value};

fn oracle() -> discord_json::JsonValue {
    let raw = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/cldr-oracle.json"
    ))
    .expect("cldr oracle fixture");
    discord_json::parse(&raw).expect("valid oracle JSON")
}

fn oracle_value<'a>(value: &'a discord_json::JsonValue, path: &[&str]) -> &'a str {
    let mut node = value;
    for key in path {
        node = node
            .get(key)
            .unwrap_or_else(|| panic!("missing oracle path {path:?}"));
    }
    node.as_str()
        .unwrap_or_else(|| panic!("not a string: {path:?}"))
}

fn catalog(raw: &str) -> Catalog {
    Catalog::parse(&discord_json::parse(raw).expect("catalog JSON")).expect("catalog AST")
}

fn args(pairs: &[(&str, Value)]) -> BTreeMap<String, Value> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_string(), value.clone()))
        .collect()
}

#[test]
fn all_enabled_catalogs_insert_and_switch() {
    let empty = catalog("{}");
    let mut i18n = I18n::new("en-US");
    let enabled: Vec<&str> = SUPPORTED_LOCALES
        .iter()
        .copied()
        .filter(|code| !DISABLED_LOCALES.contains(code))
        .collect();
    assert_eq!(enabled.len(), 31);
    for locale in &enabled {
        i18n.insert_catalog(locale, empty.clone())
            .unwrap_or_else(|error| panic!("insert {locale}: {error}"));
    }
    for locale in &enabled {
        i18n.set_locale(locale)
            .unwrap_or_else(|error| panic!("set {locale}: {error}"));
    }
    for locale in DISABLED_LOCALES {
        assert!(i18n.insert_catalog(locale, empty.clone()).is_err());
        assert!(i18n.set_locale(locale).is_err());
    }
    assert_eq!(SUPPORTED_LOCALES.len(), TOTAL_LOCALES);
}

#[test]
fn missing_key_falls_back_to_default_and_unknown_is_error() {
    let en = catalog(r#"{"only_en":[[0,"english"]]}"#);
    let ru = catalog(r#"{"only_ru":[[0,"русский"]]}"#);
    let mut i18n = I18n::new("en-US");
    i18n.insert_catalog("en-US", en).unwrap();
    i18n.insert_catalog("ru", ru).unwrap();
    i18n.set_locale("ru").unwrap();
    assert_eq!(
        i18n.translate("only_en", &BTreeMap::new()).unwrap(),
        "english"
    );
    assert_eq!(
        i18n.translate("only_ru", &BTreeMap::new()).unwrap(),
        "русский"
    );
    assert!(matches!(
        i18n.translate("absent", &BTreeMap::new()),
        Err(TranslateError::MissingMessage { .. })
    ));
}

#[test]
fn catalog_date_time_and_number_nodes_render_from_icu_oracle() {
    let oracle = oracle();
    let timestamp = oracle
        .get("timestamp")
        .and_then(discord_json::JsonValue::as_i64)
        .expect("timestamp");
    let expected_date = oracle_value(&oracle, &["locales", "en-US", "dates", "medium"]);
    let expected_time = oracle_value(&oracle, &["locales", "en-US", "times", "short"]);
    let expected_date_long = oracle_value(&oracle, &["locales", "en-US", "dates", "long"]);
    let expected_time_full = oracle_value(&oracle, &["locales", "en-US", "times", "full"]);
    let expected_number = oracle_value(&oracle, &["locales", "en-US", "number", "1234567"]);

    let cat = catalog(
        r#"{
          "date":[3,"ts","medium"],
          "time":[4,"ts","short"],
          "date_long":[3,"ts","long"],
          "time_full":[4,"ts","full"],
          "number":[2,"n"],
          "integer":[2,"n"]
        }"#,
    );
    let mut i18n = I18n::new("en-US");
    i18n.insert_catalog("en-US", cat).unwrap();
    let date_args = args(&[("ts", Value::Int(timestamp))]);
    assert_eq!(i18n.translate("date", &date_args).unwrap(), expected_date);
    assert_eq!(i18n.translate("time", &date_args).unwrap(), expected_time);
    assert_eq!(
        i18n.translate("date_long", &date_args).unwrap(),
        expected_date_long
    );
    assert_eq!(
        i18n.translate("time_full", &date_args).unwrap(),
        expected_time_full
    );
    assert_eq!(
        i18n.translate("number", &args(&[("n", Value::Int(1_234_567))]))
            .unwrap(),
        expected_number
    );

    // The public formatter and the AST wiring must agree with the independent oracle.
    assert_eq!(
        format::format_date(timestamp, "en-US", DateStyle::Medium),
        expected_date
    );
    assert_eq!(
        format::format_time(timestamp, "en-US", DateStyle::Short),
        expected_time
    );
}

#[test]
fn exact_selector_offset_and_pound_use_plural_rules() {
    let cat = catalog(
        r#"{
          "count":[6,"n",{"one":[[7],[0," игра"]],"few":[[7],[0," игры"]],"many":[[7],[0," игр"]],"other":[[7],[0," игр"]]},0,"cardinal"],
          "exact":[6,"n",{"=2":[[0,"exact"]],"other":[[0,"other"]]},0,"cardinal"],
          "offset":[6,"n",{"one":[[7],[0," one"]],"other":[[7],[0," other"]]},2,"cardinal"],
          "offset_exact":[6,"n",{"=3":[[0,"exact3"]],"one":[[0,"one"]],"other":[[0,"other"]]},2,"cardinal"]
        }"#,
    );
    let mut i18n = I18n::new("ru");
    i18n.insert_catalog("ru", cat).unwrap();
    let tr = |key: &str, n: i64| i18n.translate(key, &args(&[("n", Value::Int(n))])).unwrap();
    assert_eq!(tr("count", 1), "1 игра");
    assert_eq!(tr("count", 2), "2 игры");
    assert_eq!(tr("count", 5), "5 игр");
    assert_eq!(tr("count", 11), "11 игр");
    assert_eq!(tr("count", 21), "21 игра");
    assert_eq!(tr("count", 22), "22 игры");
    assert_eq!(tr("count", 101), "101 игра");
    assert_eq!(tr("exact", 2), "exact");
    assert_eq!(tr("exact", 1), "other");
    assert_eq!(tr("offset", 3), "1 one");
    assert_eq!(tr("offset", 2), "0 other");
    assert_eq!(tr("offset_exact", 3), "exact3");
}

#[test]
fn number_nodes_reject_non_numeric_arguments() {
    let cat = catalog(r#"{"number":[2,"n"]}"#);
    let mut i18n = I18n::new("en-US");
    i18n.insert_catalog("en-US", cat).unwrap();
    assert!(matches!(
        i18n.translate("number", &args(&[("n", Value::Str("abc".into()))])),
        Err(TranslateError::InvalidArgument { .. })
    ));
}

#[test]
fn real_catalogs_still_parse_and_keep_link_control_backlog() {
    for (path, count) in [
        ("/home/sanya/re-deep/ru-catalog.json", 23_836usize),
        ("/home/sanya/re-deep/en-catalog.json", 23_903usize),
    ] {
        let Ok(raw) = std::fs::read_to_string(path) else {
            eprintln!("skip: unavailable oracle {path}");
            continue;
        };
        let catalog = Catalog::parse(&discord_json::parse(&raw).unwrap()).unwrap();
        assert_eq!(catalog.len(), count, "{path}");
        if path.contains("ru-") {
            let link = catalog.get("323362").expect("link oracle");
            assert!(format!("{link:?}").contains("$link"));
        }
    }
}
