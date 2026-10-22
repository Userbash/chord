use discord_i18n::locale::{
    DEFAULT_LOCALE, DISABLED_LOCALES, ENABLED_LOCALES, SUPPORTED_LOCALES, TOTAL_LOCALES,
    canonical_locale, fallback_chain, is_enabled, is_supported, locale_language, resolve_locale,
};

#[test]
fn registry_and_disabled_locales_match_the_frozen_inventory() {
    assert_eq!(TOTAL_LOCALES, 36);
    assert_eq!(ENABLED_LOCALES, 31);
    assert_eq!(SUPPORTED_LOCALES.len(), 36);
    assert_eq!(DISABLED_LOCALES, ["pt-PT", "sk", "he", "ar", "id"]);
    for code in SUPPORTED_LOCALES {
        assert!(is_supported(code), "{code}");
        assert_eq!(canonical_locale(&code.to_ascii_lowercase()), Some(code));
        assert_eq!(is_enabled(code), !DISABLED_LOCALES.contains(&code));
    }
    assert!(!is_supported("xx"));
}

#[test]
fn locale_resolution_uses_available_enabled_exact_then_language_then_default() {
    let available = ["en-US", "en-GB", "ru", "fr", "pt-PT", "sk"];
    assert_eq!(resolve_locale("ru", &available), "ru");
    assert_eq!(resolve_locale("FR", &available), "fr");
    assert_eq!(resolve_locale("en-AU", &available), "en-US");
    assert_eq!(resolve_locale("pt-PT", &available), "en-US");
    assert_eq!(resolve_locale("sk", &available), "en-US");
    assert_eq!(resolve_locale("", &available), DEFAULT_LOCALE);
    assert_eq!(resolve_locale("ru", &["fr"]), DEFAULT_LOCALE);
    assert_eq!(fallback_chain("ru", &available), ["ru", "en-US"]);
    assert_eq!(fallback_chain("en-US", &available), ["en-US"]);
    assert_eq!(locale_language("pt-BR"), "pt");
}
