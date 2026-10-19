//! # discord-conformance
//!
//! Доступ к курированным конформанс-фикстурам RE (`fixtures/` в корне репозитория;
//! провенанс — `fixtures/MANIFEST.json`, полное сырьё — `/home/sanya/re-deep`).
//!
//! Тесты уровня L1–L3 (`docs/rust-client/TEST-PLAN.md` §3) читают артефакты отсюда,
//! поэтому путь переопределяется переменной `RE_FIXTURES_ROOT` (например, для CI).

use std::path::PathBuf;

/// Корень конформанс-фикстур: `RE_FIXTURES_ROOT` либо `<repo>/fixtures`.
pub fn fixtures_root() -> PathBuf {
    if let Ok(env_root) = std::env::var("RE_FIXTURES_ROOT") {
        return PathBuf::from(env_root);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures")
}

/// Путь к манифесту фикстур.
pub fn manifest_path() -> PathBuf {
    fixtures_root().join("MANIFEST.json")
}

#[cfg(test)]
mod tests {
    #[test]
    fn manifest_exists_and_lists_fixtures() {
        let path = super::manifest_path();
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("не читается {}: {e}", path.display()));
        assert!(
            text.contains("\"fixtures\""),
            "в манифесте есть список fixtures"
        );
        let md5_entries = text.matches("\"md5\"").count();
        assert!(
            md5_entries >= 12,
            "ожидается ≥12 записей с md5, найдено {md5_entries}"
        );
    }

    #[test]
    fn ready_fixture_is_present() {
        let ready = super::fixtures_root().join("gateway/by-type/002-READY.json");
        assert!(ready.is_file(), "нет фикстуры {}", ready.display());
    }
}
