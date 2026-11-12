//! Locale registry and fallback selection (P7-01).
//!
//! Contract: `waves/RW-24/WAVE-PLAN.md` §3.1. The implementation is owned by W25a.

/// Number of locale codes shipped in the desktop client.
pub const TOTAL_LOCALES: usize = 36;
/// Number of enabled locales (a catalog key exists in the ru inventory).
pub const ENABLED_LOCALES: usize = 31;
/// Locale codes observed in the desktop client.
pub const SUPPORTED_LOCALES: [&str; 36] = [
    "en-US", "en-GB", "zh-CN", "zh-TW", "cs", "da", "nl", "fr", "de", "el", "hu", "it", "ja", "ko",
    "pl", "pt-PT", "pt-BR", "ru", "sk", "es-419", "es-ES", "sv-SE", "tr", "bg", "uk", "fi", "no",
    "hr", "ro", "lt", "th", "vi", "hi", "he", "ar", "id",
];
/// Default locale used when a request cannot be resolved.
pub const DEFAULT_LOCALE: &str = "en-US";
/// Locales present in the asset list but disabled in the client build.
pub const DISABLED_LOCALES: [&str; 5] = ["pt-PT", "sk", "he", "ar", "id"];

/// Return true when `code` is one of the 36 shipped locale codes.
#[must_use]
pub fn is_supported(code: &str) -> bool {
    SUPPORTED_LOCALES
        .iter()
        .any(|candidate| candidate.eq_ignore_ascii_case(code))
}

/// Return true when `code` is supported and not disabled.
#[must_use]
pub fn is_enabled(code: &str) -> bool {
    is_supported(code)
        && !DISABLED_LOCALES
            .iter()
            .any(|candidate| candidate.eq_ignore_ascii_case(code))
}

/// Canonical spelling for a supported locale code.
#[must_use]
pub fn canonical_locale(code: &str) -> Option<&'static str> {
    SUPPORTED_LOCALES
        .iter()
        .copied()
        .find(|candidate| candidate.eq_ignore_ascii_case(code))
}

/// Language part of a locale code (`pt-BR` -> `pt`).
#[must_use]
pub fn locale_language(code: &str) -> &str {
    code.split('-').next().unwrap_or(code)
}

/// Resolve a requested locale against enabled available locales.
#[must_use]
pub fn resolve_locale(requested: &str, available: &[&str]) -> String {
    let usable: Vec<&str> = available
        .iter()
        .copied()
        .filter(|code| is_enabled(code))
        .collect();
    let find = |name: &str| usable.iter().copied().find(|code| *code == name);
    if !requested.is_empty() {
        if let Some(code) = find(requested) {
            return code.to_owned();
        }
        if let Some(code) = usable
            .iter()
            .copied()
            .find(|code| code.eq_ignore_ascii_case(requested))
        {
            return code.to_owned();
        }
        let language = locale_language(requested);
        if let Some(code) = usable
            .iter()
            .copied()
            .find(|code| code.eq_ignore_ascii_case(language))
        {
            return code.to_owned();
        }
    }
    find(DEFAULT_LOCALE).unwrap_or(DEFAULT_LOCALE).to_owned()
}

/// Fallback chain for a request: resolved locale followed by the default.
#[must_use]
pub fn fallback_chain(requested: &str, available: &[&str]) -> Vec<String> {
    let resolved = resolve_locale(requested, available);
    if resolved == DEFAULT_LOCALE {
        vec![resolved]
    } else {
        vec![resolved, DEFAULT_LOCALE.to_owned()]
    }
}
