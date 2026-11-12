# B02 / P0-04 — conformance harness skeleton (contract)

Дата: 2026-10-21. Карточка **P0-04** (M). Файлы: `discord-conformance/src/lib.rs`,
`discord-conformance/tests/fixtures.rs`. Без новых зависимостей: только `std`.

## 1. Цель

Дать тестам один детерминированный загрузчик фикстур: сначала курированный `fixtures/`
репозитория, затем сырьё `reference-oracle/` (переопределяется `RE_LAB_ROOT`). Ошибки
типизированы, путь и источник видны в отчёте; отсутствие lab — skip-aware на уровне теста,
а не паника загрузчика.

## 2. Контракт

```rust
pub enum FixtureSource { Repository, Lab }
pub struct Fixture { pub name: String, pub source: FixtureSource, pub path: PathBuf }
pub enum ConformanceError { InvalidName{..}, NotFound{..}, Io{..}, Utf8{..} }

pub fn fixtures_root() -> PathBuf;   // RE_FIXTURES_ROOT | <repo>/fixtures
pub fn lab_root() -> PathBuf;        // RE_LAB_ROOT | reference-oracle/
pub fn manifest_path() -> PathBuf;
pub fn manifest_text() -> Result<String, ConformanceError>;
pub fn resolve(name: &str) -> Result<Fixture, ConformanceError>; // repo → lab
pub fn load_bytes(name: &str) -> Result<Vec<u8>, ConformanceError>;
pub fn load_text(name: &str) -> Result<String, ConformanceError>;
```

`name` — относительный путь без `..`/корня; иначе `InvalidName`. `NotFound` содержит оба
проверенных пути. `Fixture::read_bytes/read_text` доступны и после `resolve`. Никакой
сетевой/записывающей логики.

Обычный прогон без lab — soft-skip с причиной (правило `TEST-PLAN.md`). Для приёмки
`RE_REQUIRE_LAB=1` превращает отсутствие lab-файла в падение, чтобы skip не засчитывался
как pass.

## 3. Тесты (T-HARN-01)

| ID | Проверка |
|---|---|
| h01 | манифест читается, содержит `labRoot` и ≥12 `md5`; `fixtures_root` существует |
| h02 | `resolve("gateway/ready-stats.json")` → Repository, файл непустой и читается |
| h03 | `resolve` lab-only файла (`gw-proto/zstd-frames-95974_1389.hexl`) → Lab; нет lab → skip |
| h04 | абсолютный путь/`..` → `InvalidName`; отсутствующий → `NotFound` с двумя путями |
| h05 | `load_text`/`load_bytes` возвращают те же байты, что прямой `fs::read` |
| h06 | `RE_FIXTURES_ROOT`/`RE_LAB_ROOT` переопределяют корни (проверка функций, без записи) |

T-HARN-02/03 (полный реплей gateway/REST) остаются отдельными карточками: P0-04 — скелет
загрузчика, не подмена протокольных тестов.
