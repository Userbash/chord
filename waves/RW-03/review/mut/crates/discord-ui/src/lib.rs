//! # discord-ui
//!
//! Интерфейс: дизайн-токены и контракты компонент, восстановленные из веб-клиента
//! (`docs/re/frontend-d03.md` §5 — 12 контрактов: поиск, эмодзи, участники, настройки,
//! поповер пользователя). Тулк­ит выбирается спайком P0-01 (ADR-1: iced по умолчанию).
//!
//! План: P5 (каркас UI), P6 (полный UI); тесты: T-UI-*.

/// Контрактов компонент, зафиксированных в D-03.
pub const COMPONENT_CONTRACTS: usize = 12;

/// Статус ADR-1: выбранный UI-тулкит фиксируется по итогам спайка P0-01.
pub const UI_TERMINAL_FALLBACK: &str = "egui (только прототипы)";

#[cfg(test)]
mod tests {
    #[test]
    fn contracts_count_matches_d03() {
        assert_eq!(super::COMPONENT_CONTRACTS, 12);
    }
}
