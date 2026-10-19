//! # discord-i18n
//!
//! Локализация: хеширование ключей `runtimeHashMessageKey` (xxh64 → base64),
//! плурализация CLDR, цепочка fallback `запрошенная → en-US → пусто`.
//!
//! Спека: `docs/re/i18n-deep.md` §1–3, §7 (36 кодов, включено 31).
//! План: P3 (базовый рантайм) → P7 (полные 36 локалей); тесты: T-I18N-*.

/// Длина вычисляемого ключа: base64(xxh64) → 6 символов (`[0..6]`).
pub const MESSAGE_KEY_LEN: usize = 6;

/// Кодов локалей в сборке клиента (`lang-list.json`).
pub const TOTAL_LOCALES: usize = 36;

/// Включённых локалей (ключ вычисляется и находится в ru-каталоге).
pub const ENABLED_LOCALES: usize = 31;

/// Отключённые локали из списка 36.
pub const DISABLED_LOCALES: [&str; 5] = ["pt-PT", "sk", "he", "ar", "id"];

/// Демонстрация категорий плурализации ru (CLDR): (n, категория).
pub const PLURAL_RU_DEMO: [(i64, &str); 7] = [
    (1, "one"),
    (2, "few"),
    (5, "many"),
    (11, "many"),
    (21, "one"),
    (22, "few"),
    (101, "one"),
];

#[cfg(test)]
mod tests {
    #[test]
    fn locale_math_is_consistent() {
        assert_eq!(super::ENABLED_LOCALES + super::DISABLED_LOCALES.len(), super::TOTAL_LOCALES);
        assert_eq!(super::MESSAGE_KEY_LEN, 6);
    }

    #[test]
    fn ru_plural_categories() {
        for (n, cat) in super::PLURAL_RU_DEMO {
            let expected = match n % 10 {
                1 if n % 100 != 11 => "one",
                2..=4 if !(12..=14).contains(&(n % 100)) => "few",
                _ => "many",
            };
            assert_eq!(cat, expected, "n={n}");
        }
    }
}
