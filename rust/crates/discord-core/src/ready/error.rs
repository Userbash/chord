//! READY: ошибки и диагностика (RW-12 / P2-01).
//!
//! Контракт заморожен `waves/RW-12/DESIGN-P2-01.md` §3.3. Ошибки — только для «якорей»
//! (`d`, `d.guilds`, `d.user.id`, `d.session_id`); битые элементы коллекций пропускаются в
//! [`ReadyDiagnostics::malformed`], неизвестные поля — в `unknown_top_level`/`ignored`.

use std::fmt;

/// Фатальная ошибка разбора READY.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadyError {
    /// Не op0 `READY` (или нет `d`).
    NotReady,
    /// Обязательное поле отсутствует.
    Missing {
        /// Путь (`d`, `d.guilds`, `d.user`, `d.session_id`).
        path: String,
    },
    /// Поле есть, но не того типа.
    WrongType {
        /// Путь.
        path: String,
        /// Ожидаемый тип.
        expected: &'static str,
        /// Фактический ETF-вид.
        found: &'static str,
    },
    /// Снежинка невалидна (отрицательная, не число, переполнение).
    InvalidId {
        /// Путь.
        path: String,
        /// Исходное значение (для диагностики; не секрет).
        value: String,
    },
}

impl fmt::Display for ReadyError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotReady => write!(formatter, "не op0 READY"),
            Self::Missing { path } => write!(formatter, "нет обязательного поля {path}"),
            Self::WrongType {
                path,
                expected,
                found,
            } => write!(
                formatter,
                "поле {path}: ожидался {expected}, найден {found}"
            ),
            Self::InvalidId { path, value } => {
                write!(formatter, "поле {path}: невалидная снежинка {value:?}")
            }
        }
    }
}

impl std::error::Error for ReadyError {}

/// Нефатальные расхождения: разбор продолжается, данные не теряются молча.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadyDiagnostics {
    /// Новые top-level ключи (не из [`super::TOP_LEVEL_KEYS`]) в порядке провода.
    pub unknown_top_level: Vec<String>,
    /// Пропущенные битые элементы (`guilds[2]`, `guilds[2].channels[7]`).
    pub malformed: Vec<String>,
    /// Дубликаты id (`guilds:101…`), last wins.
    pub duplicates: Vec<String>,
    /// Поля/коллекции неверного типа, заменённые дефолтом.
    pub ignored: Vec<String>,
}
