# TZ-W26c — P8-01: свой updater (крейт `discord-updater`)

Ты — **W26c**, кодовый воркер лейна C волны B06/RW-25. Пиши **только** файлы из §1.
`Cargo.toml`, `Cargo.lock`, `src/lib.rs` крейтов, доки и `waves/*` — не трогать.
Сначала red-тесты (команда + exit code), затем минимальный green.

## 1. Файлы

Пишешь:
- `rust/crates/discord-updater/src/manifest.rs`
- `rust/crates/discord-updater/src/plan.rs`
- `rust/crates/discord-updater/tests/updater_contract.rs`

Читаешь, но не меняешь: `rust/crates/discord-updater/src/lib.rs`,
`docs/rust-client/PLAN.md` (P8-01), `docs/rust-client/TEST-PLAN.md` (T-SYS-11),
`waves/RW-25/WAVE-PLAN.md`.

Запрещено: `unsafe`, новые зависимости, сеть, `git commit/push`, доступ к живому Discord,
запись в `logs/`, `functions/`, `evidence/`.

## 2. Карточка и границы

`PLAN.md` P8-01: «Свой updater | манифест+Ed25519, staged-замена, откат | T-SYS-11 | P7 | L».

Границы карточки (зафиксированы в `src/lib.rs`):

- **Сеть в этой карточке не выполняется.** Модуль проверяет манифест, подпись, хэши и
  строит/исполняет план локальной замены файлов. Загрузка артефакта — следующий подэтап.
- Реальная установка в каталог приложения — вне карточки; исполняется план только над
  каталогом, который вызвал тест (temp-каталог).
- Подпись: Ed25519 через `ring 0.17` (`ring::signature::ED25519`,
  `ring::signature::UnparsedPublicKey`, `ring::signature::Ed25519KeyPair`) — крейт уже
  зависит от `ring`, новых пакетов в `Cargo.lock` это не добавляет. Ключ проверки
  передаётся вызывающим кодом (встраивается в приложение в отдельной карточке).
- Версии сравниваются **локально**: крейт не подключает `semver` — реализуй разбор и
  сравнение `major.minor.patch[-pre]` сам, в `manifest.rs`.

## 3. Замороженный API

### `src/manifest.rs`

```rust
/// Версия релиза: `major.minor.patch` с необязательным pre-release суффиксом.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    /// Pre-release суффикс (`"beta.1"`); `None` — стабильный релиз.
    pub pre: Option<String>,
}
impl Version {
    /// Разбор строки версии. Допускается ровно `major.minor.patch` и
    /// `major.minor.patch-<pre>`; мусор ⇒ ошибка.
    pub fn parse(text: &str) -> Result<Self, ManifestError>;
    /// `true`, если `self` строго новее `other`.
    /// Pre-release считается младше своей же версии без суффикса
    /// (`1.0.0-beta` < `1.0.0`); pre-суффиксы сравниваются лексикографически.
    pub fn is_newer_than(&self, other: &Self) -> bool;
}

/// Один артефакт релиза.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Artifact {
    /// Имя артефакта (имя файла/цели).
    pub name: String,
    /// URL источника (сеть вне карточки — только фиксируется в плане).
    pub url: String,
    /// SHA-256 в hex (нижний регистр, 64 символа).
    pub sha256: String,
    /// Ожидаемый размер в байтах.
    pub size: u64,
}

/// Манифест релиза.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Manifest {
    pub version: Version,
    /// Дата выпуска (ISO-8601), хранится как строка.
    pub released_at: String,
    pub artifacts: Vec<Artifact>,
}

/// Ошибка манифеста.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    InvalidJson { message: String },
    MissingField { field: &'static str },
    InvalidVersion { value: String },
    InvalidDigest { artifact: String, value: String },
    DuplicateArtifact { name: String },
    /// Подпись не совпала.
    InvalidSignature,
    /// В блобе нет 64 байт подписи.
    SignatureMissing,
    /// Артефакт с таким именем не найден.
    UnknownArtifact { name: String },
    /// Текущая версия не старше кандидата.
    NotNewer { current: String, candidate: String },
    /// Хэш фактических данных не совпал с ожидаемым.
    DigestMismatch { artifact: String },
    /// Ошибка ввода-вывода (с путьём и текстом).
    Io { path: String, message: String },
    /// Откат не удался.
    RollbackFailed { path: String, message: String },
}

/// Формат подписанного блоба: `body || signature[64]`, где `signature` — Ed25519
/// над **исходными байтами** `body`. Канонизация JSON не нужна: подписывается ровно то,
/// что пришло.
pub fn parse_manifest(bytes: &[u8]) -> Result<Manifest, ManifestError>;

/// Проверяет подпись и разбирает манифест. Ошибка подписи ⇒ `InvalidSignature`.
pub fn verify_manifest(signed: &[u8], public_key: &[u8; 32]) -> Result<Manifest, ManifestError>;

/// Подписывает тело манифеста (для выпуска и для тестов).
/// `seed` — 32-байтовый seed Ed25519-ключа (`ring::signature::Ed25519KeyPair`).
/// Возвращает `body || signature[64]`.
pub fn sign_manifest(body: &[u8], seed: &[u8; 32]) -> Result<Vec<u8>, ManifestError>;

/// SHA-256 артефакта в hex (нижний регистр).
pub fn sha256_hex(bytes: &[u8]) -> String;
```

Требования:

- `parse_manifest` **не** проверяет подпись — это отдельная операция (`verify_manifest`).
- `parse_manifest` проверяет: валидный JSON, наличие `version`/`artifacts`, корректность
  каждого `sha256` (ровно 64 hex-символа), уникальность имён артефактов, `size >= 0`.
- `verify_manifest`: длина < 64 ⇒ `SignatureMissing`; подпись не сошлась ⇒
  `InvalidSignature` (включая подменённое тело и чужой ключ).
- `Version::parse` отвергает: пустую строку, `1.2`, `1.2.3.4`, `1.2.x`, `v1.2.3`,
  ведущие нули в компонентах (`01.2.3` ⇒ ошибка).
- `Version::is_newer_than`: `1.10.0 > 1.9.9`, `2.0.0 > 1.99.99`, `1.0.0 > 1.0.0-beta`,
  `1.0.0-beta.2 > 1.0.0-beta.1`, `1.0.0 == 1.0.0` (не новее).

### `src/plan.rs`

```rust
use std::path::PathBuf;

/// Шаг плана обновления.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanStep {
    /// Скачать артефакт в staging-каталог (сеть — вне карточки, шаг только фиксируется).
    Download { url: String, to: PathBuf },
    /// Сверить хэш скачанного файла с манифестом.
    VerifyDigest { artifact: String, expected: String, actual: String },
    /// Положить проверенный файл в staging-каталог.
    Stage { artifact: String, from: PathBuf, to: PathBuf },
    /// Атомарно заменить целевой файл, сохранив прежний в backup.
    Swap { from: PathBuf, to: PathBuf, backup: PathBuf },
    /// Откатить замену: вернуть backup на место цели.
    Rollback { backup: PathBuf, to: PathBuf },
    /// Удалить backup после успешного запуска.
    RemoveBackup { backup: PathBuf },
}

/// План обновления.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UpdatePlan {
    pub from_version: Version,
    pub to_version: Version,
    pub artifact: String,
    pub steps: Vec<PlanStep>,
}

/// Строит план: текущая версия → версия из манифеста для артефакта `artifact_name`.
/// Ошибки: `NotNewer`, `UnknownArtifact`.
pub fn plan_update(
    current: &Version,
    manifest: &Manifest,
    artifact_name: &str,
    install_dir: &std::path::Path,
) -> Result<UpdatePlan, PlanError>;

/// Исполняет локальную часть плана: проверяет хэш файла `source`, кладёт его в staging,
/// делает swap цели с backup, при сбое — откат.
/// Возвращает `Ok(())`; при несовпадении хэша — `PlanError::DigestMismatch`.
pub fn apply_local_steps(plan: &UpdatePlan, source: &std::path::Path) -> Result<(), PlanError>;

/// Откат: вернуть backup на место цели и снять backup.
pub fn rollback(plan: &UpdatePlan) -> Result<(), PlanError>;
```

Тип ошибки: **один** тип `PlanError` на весь модуль. Он либо совпадает с `ManifestError`
(используй `#[from]`/`From`), либо определён так:

```rust
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PlanError {
    Manifest(ManifestError),
    UnknownArtifact { name: String },
    NotNewer { current: String, candidate: String },
    DigestMismatch { artifact: String },
    Io { path: String, message: String },
    RollbackFailed { path: String, message: String },
}
```

(Если выберешь другой вариант — задокументируй его в отчёте и в doc-комментарии; Public
API должен быть непротиворечивым.)

Требования к исполнению:

- Все файловые операции — через `std::fs`; рабочий каталог в тестах — под
  `std::env::temp_dir()` с уникальным суффиксом; после теста — убрать за собой.
- `apply_local_steps` обязан: (1) посчитать `sha256_hex` файла `source` и сверить с
  манифестом (при несовпадении — `DigestMismatch`, **до** любых перемещений файлов);
  (2) скопировать в staging (`stage/<имя>.new`); (3) переименовать текущую цель в
  backup (`<имя>.bak`); (4) переименовать staging в цель; при сбое шага (4) — вернуть
  backup на место и вернуть ошибку.
- `rollback` восстанавливает прежнее содержимое цели из backup и удаляет backup.
- Никаких `unwrap`/`expect` на пользовательских путях — только `Result`.
- Никаких глобальных переменных окружения и никакой сети.

## 4. Тесты (`tests/updater_contract.rs`, минимум 18)

- `manifest_parse_k` — валидный JSON → корректный `Manifest`.
- `manifest_invalid_json_k` — битый JSON ⇒ `InvalidJson`.
- `manifest_missing_field_k` — нет `version` ⇒ `MissingField`.
- `manifest_invalid_version_k` — `"1.2.x"` ⇒ `InvalidVersion`.
- `manifest_invalid_digest_k` — `sha256` длиной 10 ⇒ `InvalidDigest`.
- `manifest_duplicate_artifact_k` — два артефакта с одинаковым `name` ⇒ `DuplicateArtifact`.
- `signature_ok_k` — подпись валидна ⇒ `Ok`.
- `signature_bad_k` — битый байт подписи ⇒ `InvalidSignature`.
- `signature_wrong_key_k` — проверка чужим ключом ⇒ `InvalidSignature`.
- `signature_tampered_body_k` — тело изменено после подписи ⇒ `InvalidSignature`.
- `signature_missing_k` — блоб короче 64 Б ⇒ `SignatureMissing`.
- `version_order_k` — набор сравнений версий (см. §3).
- `version_parse_rejects_k` — невалидные строки отвергаются.
- `plan_not_newer_k` — текущая версия ≥ кандидата ⇒ `NotNewer`.
- `plan_unknown_artifact_k` — нет такого имени ⇒ `UnknownArtifact`.
- `plan_steps_k` — план содержит `Download` → `VerifyDigest` → `Stage` → `Swap`
  (и `RemoveBackup` после успеха) в этом порядке; пути — внутри `install_dir`.
- `digest_mismatch_k` — файл с другим содержимым ⇒ `DigestMismatch`, **цель не тронута**.
- `staged_swap_k` — успешная замена: цель содержит новое содержимое, backup — прежнее.
- `rollback_k` — `rollback` возвращает прежнее содержимое и убирает backup.
- `swap_failure_rollback_k` — сбой на шаге подмены ⇒ прежнее содержимое цели цело,
  ошибка возвращена.

Ключи для подписи в тестах — фиксированные seed-детерминированные (`[7u8; 32]`),
не случайные. Публичный ключ для проверки получай из seed через
`ring::signature::Ed25519KeyPair::from_seed_unchecked(seed)` — это детерминировано и не
требует сети.

## 5. Red/green и сдача

```bash
cd /home/sanya/orca/project/rust
export CARGO_TARGET_DIR=/tmp/opencode/target-w26c
cargo test --offline -p discord-updater --test updater_contract   # red
cargo test --offline -p discord-updater                              # green
cargo fmt --all
cargo clippy --offline -p discord-updater --all-targets -- -D warnings
```

Логи: `waves/RW-25/red-W26c.log`, `waves/RW-25/green-W26c.log` (команда + exit code).
Отчёт: `waves/RW-25/worker-W26c.md` по шаблону `PROCESS.md` §4.2.
