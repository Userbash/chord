//! # discord-conformance
//!
//! Доступ к курированным конформанс-фикстурам RE (`fixtures/` в корне репозитория;
//! провенанс — `fixtures/MANIFEST.json`, полное сырьё — `/home/sanya/re-deep`).
//!
//! Тесты уровня L1–L3 (`docs/rust-client/TEST-PLAN.md` §3) читают артефакты отсюда,
//! поэтому путь переопределяется переменной `RE_FIXTURES_ROOT` (например, для CI).
//! Если курированной фикстуры нет, загрузчик смотрит в лабораторию (`RE_LAB_ROOT`).
//! Отсутствие лаборатории — это `NotFound`/skip в тесте, а не паника библиотеки.

use std::fmt;
use std::io;
use std::path::{Component, Path, PathBuf};

/// Источник найденной фикстуры.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FixtureSource {
    /// Курированная копия в `fixtures/` репозитория.
    Repository,
    /// Сырьё в лаборатории (`/home/sanya/re-deep` либо `RE_LAB_ROOT`).
    Lab,
}

/// Разрешённая фикстура: относительное имя, источник и абсолютный/рабочий путь.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fixture {
    /// Имя относительно корня фикстур (например, `gateway/ready-stats.json`).
    pub name: String,
    /// Где именно найден файл.
    pub source: FixtureSource,
    /// Путь, по которому найден файл.
    pub path: PathBuf,
}

impl Fixture {
    /// Прочитать фикстуру как байты.
    pub fn read_bytes(&self) -> Result<Vec<u8>, ConformanceError> {
        read_bytes(&self.path)
    }

    /// Прочитать фикстуру как UTF-8-текст.
    pub fn read_text(&self) -> Result<String, ConformanceError> {
        read_text(&self.path)
    }
}

/// Ошибка загрузчика фикстур.
#[derive(Debug)]
pub enum ConformanceError {
    /// Имя пустое, абсолютное или содержит `..`.
    InvalidName {
        /// Отклонённое имя.
        name: String,
    },
    /// Файл не найден ни в репозитории, ни в лаборатории.
    NotFound {
        /// Относительное имя.
        name: String,
        /// Проверенный путь репозитория.
        repository: PathBuf,
        /// Проверенный путь лаборатории.
        lab: PathBuf,
    },
    /// Ошибка чтения/метаданных (кроме `NotFound`).
    Io {
        /// Путь, на котором произошла ошибка.
        path: PathBuf,
        /// Исходная ошибка ОС.
        source: io::Error,
    },
    /// Файл прочитан, но не является UTF-8.
    Utf8 {
        /// Путь к файлу.
        path: PathBuf,
    },
}

impl fmt::Display for ConformanceError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidName { name } => {
                write!(formatter, "недопустимое имя фикстуры: {name:?}")
            }
            Self::NotFound {
                name,
                repository,
                lab,
            } => write!(
                formatter,
                "фикстура {name:?} не найдена: {} или {}",
                repository.display(),
                lab.display()
            ),
            Self::Io { path, source } => {
                write!(formatter, "ошибка чтения {}: {source}", path.display())
            }
            Self::Utf8 { path } => {
                write!(formatter, "фикстура {} не является UTF-8", path.display())
            }
        }
    }
}

impl std::error::Error for ConformanceError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Io { source, .. } => Some(source),
            _ => None,
        }
    }
}

/// Корень конформанс-фикстур: `RE_FIXTURES_ROOT` либо `<repo>/fixtures`.
pub fn fixtures_root() -> PathBuf {
    if let Ok(env_root) = std::env::var("RE_FIXTURES_ROOT") {
        return PathBuf::from(env_root);
    }
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../fixtures")
}

/// Корень лаборатории: `RE_LAB_ROOT` либо `/home/sanya/re-deep`.
pub fn lab_root() -> PathBuf {
    PathBuf::from(
        std::env::var("RE_LAB_ROOT").unwrap_or_else(|_| "/home/sanya/re-deep".to_string()),
    )
}

/// Путь к манифесту фикстур.
pub fn manifest_path() -> PathBuf {
    fixtures_root().join("MANIFEST.json")
}

/// Текст манифеста фикстур.
pub fn manifest_text() -> Result<String, ConformanceError> {
    read_text(&manifest_path())
}

/// Найти фикстуру: сначала курированную копию, затем лабораторию.
///
/// Возвращает [`ConformanceError::InvalidName`] до любых обращений к файловой системе,
/// поэтому имя не может выйти за корень.
pub fn resolve(name: &str) -> Result<Fixture, ConformanceError> {
    validate_name(name)?;
    let repository = fixtures_root().join(name);
    if repository.is_file() {
        return Ok(Fixture {
            name: name.to_string(),
            source: FixtureSource::Repository,
            path: repository,
        });
    }
    let lab = lab_root().join(name);
    if lab.is_file() {
        return Ok(Fixture {
            name: name.to_string(),
            source: FixtureSource::Lab,
            path: lab,
        });
    }
    Err(ConformanceError::NotFound {
        name: name.to_string(),
        repository,
        lab,
    })
}

/// Прочитать фикстуру как байты (repo → lab).
pub fn load_bytes(name: &str) -> Result<Vec<u8>, ConformanceError> {
    resolve(name)?.read_bytes()
}

/// Прочитать фикстуру как UTF-8-текст (repo → lab).
pub fn load_text(name: &str) -> Result<String, ConformanceError> {
    resolve(name)?.read_text()
}

/// Проверка относительного имени без выхода за корень.
fn validate_name(name: &str) -> Result<(), ConformanceError> {
    let path = Path::new(name);
    let invalid = name.is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        });
    if invalid {
        Err(ConformanceError::InvalidName {
            name: name.to_string(),
        })
    } else {
        Ok(())
    }
}

/// Чтение с типизированной ошибкой; `NotFound` здесь тоже `Io` — вызывающий уже разрешил путь.
fn read_bytes(path: &Path) -> Result<Vec<u8>, ConformanceError> {
    std::fs::read(path).map_err(|source| ConformanceError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_text(path: &Path) -> Result<String, ConformanceError> {
    let bytes = read_bytes(path)?;
    String::from_utf8(bytes).map_err(|_| ConformanceError::Utf8 {
        path: path.to_path_buf(),
    })
}
