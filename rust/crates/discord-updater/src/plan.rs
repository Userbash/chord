//! План staged-замены файла и откат.
//!
//! План описывает последовательность локальных шагов: скачать артефакт в staging,
//! сверить хэш, положить проверенный файл в staging, атомарно подменить цель с
//! сохранением прежней версии в backup и (после успешного запуска) снести backup.
//!
//! Сеть вне карточки: шаг `Download` только фиксируется, загрузку выполняет вызывающий
//! код. Исполняемая часть — [`apply_local_steps`] и [`rollback`], работают только с
//! локальной файловой системой.
//!
//! Контракт: `waves/RW-25/TZ-W26c.md` §3.

use std::path::{Path, PathBuf};

use crate::manifest::{Artifact, ManifestError, VerifiedManifest, Version, sha256_hex};

/// Шаг плана обновления.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanStep {
    /// Скачать артефакт в staging-каталог (сеть — вне карточки, шаг только фиксируется).
    Download {
        /// URL источника артефакта.
        url: String,
        /// Путь, куда класть скачанный файл.
        to: PathBuf,
    },
    /// Сверить хэш скачанного файла с манифестом.
    VerifyDigest {
        /// Имя артефакта.
        artifact: String,
        /// Ожидаемый хэш (из манифеста).
        expected: String,
        /// Фактический хэш; при построении плана ещё не вычислен — пустая строка.
        actual: String,
    },
    /// Положить проверенный файл в staging-каталог.
    Stage {
        /// Имя артефакта.
        artifact: String,
        /// Откуда берём файл (путь скачанного файла).
        from: PathBuf,
        /// Куда кладём проверенный файл (путь в staging-каталоге).
        to: PathBuf,
    },
    /// Атомарно заменить целевой файл, сохранив прежний в backup.
    Swap {
        /// Путь staged-файла (источник подмены).
        from: PathBuf,
        /// Путь целевого файла.
        to: PathBuf,
        /// Путь, куда уезжает прежняя версия цели.
        backup: PathBuf,
    },
    /// Откатить замену: вернуть backup на место цели.
    Rollback {
        /// Путь backup-файла.
        backup: PathBuf,
        /// Путь целевого файла.
        to: PathBuf,
    },
    /// Удалить backup после успешного запуска.
    RemoveBackup {
        /// Путь backup-файла.
        backup: PathBuf,
    },
}

/// План обновления.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePlan {
    /// Версия, с которой обновляем.
    pub from_version: Version,
    /// Версия, на которую обновляем.
    pub to_version: Version,
    /// Имя обновляемого артефакта.
    pub artifact: String,
    /// Последовательность шагов.
    pub steps: Vec<PlanStep>,
}

/// Ошибка планирования или исполнения обновления.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    /// Ошибка манифеста (обёртка через `From`).
    Manifest(ManifestError),
    /// Имя артефакта небезопасно как компонент пути (path traversal).
    UnsafeArtifactName {
        /// Отвергнутое имя артефакта.
        name: String,
    },
    /// Артефакта с таким именем нет в манифесте.
    UnknownArtifact {
        /// Имя артефакта.
        name: String,
    },
    /// Текущая версия не старше версии-кандидата.
    NotNewer {
        /// Текущая версия.
        current: String,
        /// Версия-кандидат.
        candidate: String,
    },
    /// Хэш фактических данных не совпал с ожидаемым.
    DigestMismatch {
        /// Имя артефакта.
        artifact: String,
    },
    /// Backup повреждён: его хэш не совпал с зафиксированным при подмене.
    ///
    /// Отличается от `DigestMismatch`: здесь речь о целостности прежней версии
    /// файла, которую откат собирается вернуть на место цели.
    BackupCorrupted {
        /// Имя артефакта.
        artifact: String,
    },
    /// Ошибка ввода-вывода (с путем и текстом).
    Io {
        /// Путь, на котором возникла ошибка.
        path: String,
        /// Текст ошибки.
        message: String,
    },
    /// Откат не удался.
    RollbackFailed {
        /// Путь, на котором возникла ошибка.
        path: String,
        /// Текст ошибки.
        message: String,
    },
}

impl From<ManifestError> for PlanError {
    fn from(error: ManifestError) -> Self {
        Self::Manifest(error)
    }
}

/// Строит план: текущая версия → версия из манифеста для артефакта `artifact_name`.
///
/// Манифест обязан быть подписанным: принимается только [`VerifiedManifest`], который
/// можно получить исключительно из `verify_manifest`. Неподписанный манифест,
/// разобранный через `parse_manifest`, в план попасть не может.
///
/// Ошибки: `NotNewer`, `UnknownArtifact`, `UnsafeArtifactName`.
///
/// Планировать можно только по подписанному манифесту — требование проверяется
/// компиляцией, а не соглашением:
///
/// ```compile_fail
/// use discord_updater::manifest::{Version, parse_manifest};
/// use discord_updater::plan::plan_update;
///
/// let body = br#"{"version":"1.0.0","artifacts":[]}"#;
/// // Неподписанный манифест — обычный `Manifest`, план по нему собрать нельзя.
/// let manifest = parse_manifest(body).expect("валидный JSON");
/// let current = Version::parse("0.1.0").expect("валидная версия");
/// let _ = plan_update(&current, &manifest, "app", std::path::Path::new("/tmp"));
/// ```
pub fn plan_update(
    current: &Version,
    manifest: &VerifiedManifest,
    artifact_name: &str,
    install_dir: &Path,
) -> Result<UpdatePlan, PlanError> {
    let artifact: &Artifact =
        manifest
            .artifact(artifact_name)
            .ok_or_else(|| PlanError::UnknownArtifact {
                name: artifact_name.to_string(),
            })?;

    if !manifest.version().is_newer_than(current) {
        return Err(PlanError::NotNewer {
            current: format_version(current),
            candidate: format_version(manifest.version()),
        });
    }

    // Имя артефакта приходит из сети: до построения путей проверяем, что это
    // безопасный одиночный компонент пути (иначе `../../outside` ушёл бы наружу).
    if !is_safe_component(artifact_name) {
        return Err(PlanError::UnsafeArtifactName {
            name: artifact_name.to_string(),
        });
    }

    // Раскладка внутри каталога установки:
    //   stage/<имя>.download — скачанный файл (шаг Download, сеть вне карточки);
    //   stage/<имя>.new      — проверенный staged-файл;
    //   <имя>               — целевой файл;
    //   <имя>.bak           — прежняя версия цели (backup).
    let stage_dir = install_dir.join("stage");
    let downloaded = stage_dir.join(format!("{}.download", artifact.name));
    let staged = stage_dir.join(format!("{}.new", artifact.name));
    let target = install_dir.join(&artifact.name);
    let backup = install_dir.join(format!("{}.bak", artifact.name));

    // Вторая линия защиты: все пути плана обязаны лежать внутри install_dir.
    for path in [&downloaded, &staged, &target, &backup] {
        if !is_inside(install_dir, path) {
            return Err(PlanError::UnsafeArtifactName {
                name: artifact.name.clone(),
            });
        }
    }

    let steps = vec![
        PlanStep::Download {
            url: artifact.url.clone(),
            to: downloaded.clone(),
        },
        PlanStep::VerifyDigest {
            artifact: artifact.name.clone(),
            expected: artifact.sha256.clone(),
            // Фактический хэш вычисляется при исполнении плана.
            actual: String::new(),
        },
        PlanStep::Stage {
            artifact: artifact.name.clone(),
            from: downloaded,
            to: staged.clone(),
        },
        PlanStep::Swap {
            from: staged,
            to: target,
            backup: backup.clone(),
        },
        PlanStep::RemoveBackup { backup },
    ];

    Ok(UpdatePlan {
        from_version: current.clone(),
        to_version: manifest.version().clone(),
        artifact: artifact.name.clone(),
        steps,
    })
}

/// Проверяет, что строка пригодна как одиночный компонент пути.
///
/// Требования: непустая, не длиннее 255 байт, только ASCII-печатные символы
/// (`0x21..=0x7E`, без пробела и управляющих), без `/` и `\`, не `.` и не `..`.
fn is_safe_component(name: &str) -> bool {
    if name.is_empty() || name.len() > 255 {
        return false;
    }
    if name == "." || name == ".." {
        return false;
    }
    if name.contains('/') || name.contains('\\') {
        return false;
    }
    name.bytes().all(|byte| (0x21..=0x7E).contains(&byte))
}

/// Проверяет, что `path` лежит внутри `base` (без канонизации: файлы могут ещё не существовать).
///
/// Сравнение идёт по компонентам: `base` должен быть буквальным префиксом `path`,
/// а в `path` не должно быть ни `..`, ни корня.
fn is_inside(base: &Path, path: &Path) -> bool {
    let base_parts: Vec<_> = base.components().collect();
    let path_parts: Vec<_> = path.components().collect();

    if path_parts.len() <= base_parts.len() {
        return false;
    }
    if path_parts[..base_parts.len()] != base_parts[..] {
        return false;
    }
    !path_parts[base_parts.len()..].iter().any(|part| {
        matches!(
            part,
            std::path::Component::ParentDir | std::path::Component::RootDir
        )
    })
}

/// Исполняет локальную часть плана: проверяет хэш файла `source`, кладёт его в staging,
/// делает swap цели с backup, при сбое — откат.
///
/// Возвращает `Ok(())`; при несовпадении хэша — `PlanError::DigestMismatch`.
/// Backup после успеха остаётся на месте: его снимает шаг `RemoveBackup` после
/// успешного запуска обновлённого приложения (вне карточки).
///
/// Ожидаемый хэш берётся из шага `VerifyDigest` плана, а он построен по манифесту,
/// поэтому сверка фактических данных идёт с тем же значением, что и в манифесте.
pub fn apply_local_steps(plan: &UpdatePlan, source: &Path) -> Result<(), PlanError> {
    let (artifact, expected) = verify_step(plan)?;
    let (stage_to, swap_from, swap_to, swap_backup) = swap_steps(plan)?;

    // (1) Сверка хэша — до любых перемещений файлов.
    let actual = sha256_hex(&read_file(source)?);
    if !actual.eq_ignore_ascii_case(&expected) {
        return Err(PlanError::DigestMismatch { artifact });
    }

    // (2) Копия проверенного файла в staging.
    //     В корректном плане `Stage.to` и `Swap.from` совпадают; при расхождении
    //     подмена упадёт на шаге (4) и сработает откат.
    if let Some(parent) = stage_to.parent() {
        create_dir_all(parent)?;
    }
    copy_file(source, &stage_to)?;

    // (3) Прежняя цель — в backup. Перед переносом фиксируем её хэш: по нему откат
    //     поймёт, что backup не повреждён (сбой питания в момент подмены).
    let had_backup = if swap_to.exists() {
        let digest = sha256_hex(&read_file(&swap_to)?);
        write_backup_digest(&swap_backup, &digest)?;
        if let Err(error) = rename_file(&swap_to, &swap_backup) {
            // Метка без самого backup бессмысленна — убираем её.
            let _ = remove_file(&backup_digest_path(&swap_backup));
            return Err(PlanError::Io {
                path: swap_to.display().to_string(),
                message: format!("не удалось увести прежнюю цель в backup: {error}"),
            });
        }
        true
    } else {
        false
    };

    // (4) Staged-файл — на место цели. При сбое возвращаем backup на место.
    if let Err(error) = rename_file(&swap_from, &swap_to) {
        if had_backup && let Err(restore_error) = rename_file(&swap_backup, &swap_to) {
            return Err(PlanError::RollbackFailed {
                path: swap_backup.display().to_string(),
                message: format!(
                    "не удалось вернуть backup на место: {restore_error}; исходная ошибка: {error}"
                ),
            });
        }
        // Backup вернулся на место цели — метка его хэша больше не нужна.
        if had_backup {
            let _ = remove_file(&backup_digest_path(&swap_backup));
        }
        return Err(PlanError::Io {
            path: swap_from.display().to_string(),
            message: format!("не удалось подменить цель: {error}"),
        });
    }

    Ok(())
}

/// Откат: вернуть backup на место цели и снять backup.
///
/// Порядок не зависит от платформенного поведения `rename` (на Windows переименование
/// не заменяет существующий файл):
/// 1. существующая цель переносится во временное имя `<имя>.failed`;
/// 2. backup переносится на место цели;
/// 3. временный файл удаляется.
///
/// Если что-то не удалось — backup остаётся на месте, ошибка `RollbackFailed`.
/// Перед восстановлением сверяется целостность backup: его хэш должен совпасть с
/// зафиксированным при подмене (см. [`apply_local_steps`]). При несовпадении отказ
/// (`BackupCorrupted`), backup и цель не трогаются.
pub fn rollback(plan: &UpdatePlan) -> Result<(), PlanError> {
    let (_, _, swap_to, swap_backup) = swap_steps(plan)?;

    if !swap_backup.exists() {
        return Err(PlanError::RollbackFailed {
            path: swap_backup.display().to_string(),
            message: "backup отсутствует: откатывать нечего".to_string(),
        });
    }

    // Целостность backup: хэш фиксируется в момент подмены (проверка, что файл не
    // повреждён сбоем питания). Если метки нет (backup сделан не нашей подменой) —
    // сверять не с чем, откат выполняется без проверки.
    if let Some(expected) = read_backup_digest(&swap_backup)? {
        let actual = sha256_hex(&read_file(&swap_backup)?);
        if !actual.eq_ignore_ascii_case(&expected) {
            return Err(PlanError::BackupCorrupted {
                artifact: plan.artifact.clone(),
            });
        }
    }

    // Временное имя для существующей цели: `rename` не заменяет файл на Windows,
    // поэтому цель сначала освобождается.
    let temp = temp_path(&swap_to);
    if temp.exists() {
        return Err(PlanError::RollbackFailed {
            path: temp.display().to_string(),
            message: "временный файл уже существует: откат невозможен".to_string(),
        });
    }

    let had_target = swap_to.exists();
    if had_target && let Err(error) = rename_file(&swap_to, &temp) {
        return Err(PlanError::RollbackFailed {
            path: swap_to.display().to_string(),
            message: format!("не удалось увести текущую цель во временный файл: {error}"),
        });
    }

    if let Err(error) = rename_file(&swap_backup, &swap_to) {
        // Возвращаем цель на место: откат не состоялся, система остаётся как была.
        if had_target {
            let _ = rename_file(&temp, &swap_to);
        }
        return Err(PlanError::RollbackFailed {
            path: swap_backup.display().to_string(),
            message: format!("не удалось вернуть backup на место цели: {error}"),
        });
    }

    if had_target {
        // Временный файл (прежнее содержимое цели) больше не нужен.
        if let Err(error) = remove_file(&temp) {
            return Err(PlanError::RollbackFailed {
                path: temp.display().to_string(),
                message: format!("цель восстановлена, но временный файл не удалён: {error:?}"),
            });
        }
    }

    // Метка хэша больше не актуальна.
    let _ = remove_file(&backup_digest_path(&swap_backup));

    Ok(())
}

/// Путь метки с хэшем backup: рядом с самим backup, с суффиксом `.sha256`.
fn backup_digest_path(backup: &Path) -> PathBuf {
    let mut name = backup
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.push_str(".sha256");
    backup.with_file_name(name)
}

/// Путь временного файла для существующей цели: `<имя>.failed` рядом с целью.
fn temp_path(target: &Path) -> PathBuf {
    let mut name = target
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_default();
    name.push_str(".failed");
    target.with_file_name(name)
}

/// Фиксирует хэш backup, чтобы откат мог проверить его целостность.
fn write_backup_digest(backup: &Path, digest: &str) -> Result<(), PlanError> {
    let path = backup_digest_path(backup);
    std::fs::write(&path, format!("{digest}\n")).map_err(|error| PlanError::Io {
        path: path.display().to_string(),
        message: format!("не удалось зафиксировать хэш backup: {error}"),
    })
}

/// Читает зафиксированный хэш backup; `None`, если метки нет.
fn read_backup_digest(backup: &Path) -> Result<Option<String>, PlanError> {
    let path = backup_digest_path(backup);
    match std::fs::read_to_string(&path) {
        Ok(text) => Ok(Some(text.trim().to_string())),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(error) => Err(PlanError::Io {
            path: path.display().to_string(),
            message: format!("не удалось прочитать хэш backup: {error}"),
        }),
    }
}

/// Достаёт из плана шаг проверки хэша: имя артефакта и ожидаемый хэш.
fn verify_step(plan: &UpdatePlan) -> Result<(String, String), PlanError> {
    plan.steps
        .iter()
        .find_map(|step| match step {
            PlanStep::VerifyDigest {
                artifact, expected, ..
            } => Some((artifact.clone(), expected.clone())),
            _ => None,
        })
        .ok_or_else(|| PlanError::Io {
            path: plan.artifact.clone(),
            message: "в плане нет шага проверки хэша".to_string(),
        })
}

/// Достаёт из плана пути staging и подмены: `(stage_to, swap_from, swap_to, swap_backup)`.
fn swap_steps(plan: &UpdatePlan) -> Result<(PathBuf, PathBuf, PathBuf, PathBuf), PlanError> {
    let stage_to = plan
        .steps
        .iter()
        .find_map(|step| match step {
            PlanStep::Stage { to, .. } => Some(to.clone()),
            _ => None,
        })
        .ok_or_else(|| PlanError::Io {
            path: plan.artifact.clone(),
            message: "в плане нет шага staging".to_string(),
        })?;

    let (swap_from, swap_to, swap_backup) = plan
        .steps
        .iter()
        .find_map(|step| match step {
            PlanStep::Swap { from, to, backup } => Some((from.clone(), to.clone(), backup.clone())),
            _ => None,
        })
        .ok_or_else(|| PlanError::Io {
            path: plan.artifact.clone(),
            message: "в плане нет шага подмены".to_string(),
        })?;

    Ok((stage_to, swap_from, swap_to, swap_backup))
}

/// Форматирует версию как `major.minor.patch[-pre]`.
fn format_version(version: &Version) -> String {
    match &version.pre {
        Some(pre) => format!(
            "{}.{}.{}-{pre}",
            version.major, version.minor, version.patch
        ),
        None => format!("{}.{}.{}", version.major, version.minor, version.patch),
    }
}

/// Читает файл, отдавая ошибку в формате плана.
fn read_file(path: &Path) -> Result<Vec<u8>, PlanError> {
    std::fs::read(path).map_err(|error| PlanError::Io {
        path: path.display().to_string(),
        message: format!("не удалось прочитать файл: {error}"),
    })
}

/// Создаёт каталог (и родителей), отдавая ошибку в формате плана.
fn create_dir_all(path: &Path) -> Result<(), PlanError> {
    std::fs::create_dir_all(path).map_err(|error| PlanError::Io {
        path: path.display().to_string(),
        message: format!("не удалось создать каталог: {error}"),
    })
}

/// Копирует файл, отдавая ошибку в формате плана.
fn copy_file(from: &Path, to: &Path) -> Result<(), PlanError> {
    std::fs::copy(from, to)
        .map(|_| ())
        .map_err(|error| PlanError::Io {
            path: to.display().to_string(),
            message: format!("не удалось скопировать файл: {error}"),
        })
}

/// Переименовывает файл. Возвращает исходную ошибку ввода-вывода: вызывающий код
/// сам решает, как её упаковать в `PlanError` (обычная ошибка или откат).
fn rename_file(from: &Path, to: &Path) -> Result<(), std::io::Error> {
    std::fs::rename(from, to)
}

/// Удаляет файл, отдавая ошибку в формате плана.
fn remove_file(path: &Path) -> Result<(), PlanError> {
    std::fs::remove_file(path).map_err(|error| PlanError::Io {
        path: path.display().to_string(),
        message: format!("не удалось удалить файл: {error}"),
    })
}
