# TZ-W26b — P7-02: перф-харнесс T-HARN-04 (крейт `discord-perf`)

Ты — **W26b**, кодовый воркер лейна B волны B06/RW-25. Пиши **только** файлы из §1.
`Cargo.toml`, `Cargo.lock`, `src/lib.rs` крейтов, доки и `waves/*` — не трогать.
Сначала red-тесты (команда + exit code), затем минимальный green.

## 1. Файлы

Пишешь:
- `rust/crates/discord-perf/src/workload.rs`
- `rust/crates/discord-perf/tests/perf_contract.rs`
- `rust/crates/discord-perf/tests/data/baseline-reference.json` (если нужен эталон — см. §6)

Читаешь, но не меняешь: `rust/crates/discord-core/src/state/*` (особенно `types.rs`,
`reducer.rs`, `member_list.rs`, `member_chunk.rs`), `docs/rust-client/TEST-PLAN.md` §3
(T-HARN-04), `docs/rust-client/PLAN.md` (P7-02), `waves/RW-25/WAVE-PLAN.md`.

Запрещено: `unsafe`, новые зависимости (кроме уже объявленных `discord-core`, `discord-json`,
`serde`, `serde_json`), сеть, `git commit/push`, доступ к живому Discord, запись в
`logs/`, `functions/`, `evidence/`.

## 2. Карточка и цель

`PLAN.md` P7-02: «24k членов, 100k сообщений, холодный старт ≤1,5 с на референсе».
`TEST-PLAN.md` T-HARN-04: «бенчмарк-гейт: регрессия >10% → красный», оракул — CI-история.

Смысл: headless-часть P7-02. UI-часть (T-UI-19, 60 fps на 24k участников) — вне карточки
(GUI заблокирован). Здесь — измерительный харнесс над реальным редьюсером состояния:
детерминированная нагрузка целевого масштаба, замеры и правило регрессии >10%.

**Важно:** «холодный старт ≤1,5 с» — это бюджет на *референсной* машине. На этой машине
замер обязан быть, а проверка бюджета — skip-aware (см. §5), чтобы не было флейка.

## 3. Замороженный API (`src/workload.rs`)

```rust
use std::time::Duration;

/// Описание нагрузки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkloadSpec {
    /// Число участников (референс: 24 229).
    pub members: usize,
    /// Число сообщений (референс: 100 000).
    pub messages: usize,
    /// Число каналов (референс: 512).
    pub channels: usize,
    /// Seed генератора (детерминизм обязателен).
    pub seed: u64,
}
impl WorkloadSpec {
    /// Референсная нагрузка P7-02.
    pub fn reference() -> Self;
}

/// Сгенерированная нагрузка.
pub struct Workload {
    pub spec: WorkloadSpec,
    /// События для редьюсера: участники (`GuildMemberListUpdate`) и сообщения
    /// (`MessageCreate`) в фиксированном порядке.
    pub events: Vec<discord_core::state::Event>,
}

/// Детерминированно строит нагрузку. Один и тот же seed ⇒ побайтово тот же результат.
pub fn build_workload(spec: &WorkloadSpec) -> Workload;

/// Метрика, по которой сравнивают прогоны.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Metric {
    /// Полное построение состояния с нуля.
    ColdStart,
    /// Только применение сообщений.
    Ingest,
    /// Время выборок из состояния.
    Selector,
}
impl Metric {
    pub fn as_str(self) -> &'static str;
}

/// Отчёт одного прогона.
#[derive(Debug, Clone, PartialEq)]
pub struct PerfReport {
    pub spec: WorkloadSpec,
    /// Построение состояния с нуля, мс.
    pub cold_start_ms: f64,
    /// Применение сообщений, мс.
    pub ingest_ms: f64,
    /// Выборки из состояния, мкс.
    pub selector_us: f64,
    /// Сколько участников реально попало в состояние.
    pub members_loaded: usize,
    /// Сколько сообщений реально применено.
    pub messages_ingested: usize,
    /// FNV-1a 64 по содержимому нагрузки — детерминизм проверяется по нему.
    pub digest: u64,
}

/// Порог регрессии T-HARN-04, %.
pub const REGRESSION_THRESHOLD_PCT: f64 = 10.0;

/// Результат сравнения прогона с базовым.
#[derive(Debug, Clone, PartialEq)]
pub enum Regression {
    /// Регрессии нет.
    None,
    /// Метрика ухудшилась больше чем на порог.
    Regressed {
        metric: Metric,
        previous: f64,
        current: f64,
        delta_pct: f64,
    },
}

/// Сравнивает текущий прогон с базовым по порогу `threshold_pct`.
pub fn compare(current: &PerfReport, baseline: &PerfReport, threshold_pct: f64) -> Regression;

/// Замеряет нагрузку на реальном редьюсере `discord_core::state::apply`.
pub fn measure(workload: &Workload) -> PerfReport;

/// Сериализация отчёта/базы в JSON (serde_json уже в зависимостях крейта).
pub fn report_to_json(report: &PerfReport) -> Result<String, serde_json::Error>;
pub fn report_from_json(text: &str) -> Result<PerfReport, serde_json::Error>;
```

Требования к реализации:

- Генератор — **собственный детерминированный PRNG** (SplitMix64/xorshift — придумай и
  зафиксируй в doc-комментарии), без внешних зависимостей.
- Никаких `SystemTime`/`random` в генераторе: только `seed`.
- `build_workload` строит `discord_core::state::Event`:
  - участники — через `EventPayload::GuildMemberListUpdate(..)` (прочитай
    `discord-core/src/state/member_list.rs` и используй его реальный API: `MemberListId`,
    `MemberListOp::Sync`, группы);
  - сообщения — через `EventPayload::MessageCreate(Message { .. })` (прочитай
    `discord-core/src/state/types.rs`, у `Message` много полей — заполни осмысленно:
    `id`, `channel_id`, `guild_id`, `author`, `content`, `timestamp`, `message_type`,
    `flags`, `mention_user_ids`, `attachment_count`, `embed_count`).
  - `Event` у `discord_core::state` имеет поля `seq`, `payload`, `diagnostics` — заполни
  `seq` монотонно, иначе сработает stale-политика и сообщения не применятся.
- `measure` обязан считать **реальное** время (`std::time::Instant`), а не константы.
- `measure` возвращает `members_loaded`/`messages_ingested` из фактического состояния
  (а не из `spec`), чтобы «молчащая» потеря данных была видна.
- `digest` — FNV-1a 64 по `(id, channel_id, content)` всех сообщений и `(user_id)` всех
  участников; порядок обхода фиксирован.
- `compare` сравнивает все три метрики; при отсутствии базовой метрики (0.0) — регрессии
  не считать (`Regression::None`), чтобы первый прогон не был «красным».
- `report_to_json`/`report_from_json` — round-trip без потерь (`PartialEq` на `PerfReport`).

## 4. Тесты (`tests/perf_contract.rs`, минимум 12)

- `determinism_k` — два `build_workload` с одним seed ⇒ равны `digest` и `events.len()`.
- `different_seed_k` — другой seed ⇒ разные `digest`.
- `spec_reference_k` — `WorkloadSpec::reference()`: `members == 24_229`, `messages == 100_000`.
- `report_shape_k` — `measure` заполняет все поля; `members_loaded == spec.members`,
  `messages_ingested == spec.messages`.
- `ingest_applied_k` — после `measure` состояние содержит ожидаемое число сообщений
  (проверка через публичный API редьюсера, а не через приватные поля).
- `regression_none_k` — одинаковые отчёты ⇒ `Regression::None`.
- `regression_detected_k` — текущий хуже базового на 11% ⇒ `Regression::Regressed`
  с `delta_pct ≈ 11`.
- `regression_below_threshold_k` — ухудшение на 9% ⇒ `Regression::None`.
- `regression_zero_baseline_k` — базовая метрика 0.0 ⇒ `Regression::None`.
- `json_roundtrip_k` — `report_to_json` → `report_from_json` ⇒ равны исходному отчёту.
- `metric_names_k` — `Metric::as_str` даёт стабильные имена (`cold_start`, `ingest`,
  `selector`).
- `threshold_constant_k` — `REGRESSION_THRESHOLD_PCT == 10.0`.
- `budget_cold_start_k` — **skip-aware**: если машина не профили референса → `eprintln!`
  с причиной и `return` (тест зелёный, причина в выводе); иначе `cold_start_ms <= 1500.0`.
  Профиль референса определи сам и задокументируй: минимум 8 логических CPU и
  `available_parallelism() >= 8`; измерение медианы из 3 прогонов, чтобы не ловить флейк.

## 5. Правила

- Тесты сверяются с независимым оракулом (детерминизм, арифметика регрессии, JSON),
  а не с собственной реализацией.
- Ошибки не глотать: `serde_json::Error` пробрасывается, `expect` в тестах допустим
  только с осмысленным сообщением.
- Тайминги в тестах **не** сравнивать с жёсткими числами, кроме бюджета §4 (skip-aware).

## 6. Базовый прогон (T-HARN-04)

`report_to_json`/`report_from_json` — база для CI-истории. Эталонный файл
`tests/data/baseline-reference.json` **не обязан** быть создан: тесты не читают диск,
кроме опционального baseline. Если создаёшь — он должен быть валидным JSON отчёта с
`spec` референсной нагрузки; тесты, читающие его, обязаны корректно skip-ить при его
отсутствии (сообщив причину).

## 7. Red/green и сдача

```bash
cd /home/user/orca/project/rust
export CARGO_TARGET_DIR=/tmp/opencode/target-w26b
cargo test --offline -p discord-perf --test perf_contract   # red
cargo test --offline -p discord-perf                          # green
cargo fmt --all
cargo clippy --offline -p discord-perf --all-targets -- -D warnings
```

Логи: `waves/RW-25/red-W26b.log`, `waves/RW-25/green-W26b.log` (команда + exit code).
Отчёт: `waves/RW-25/worker-W26b.md` по шаблону `PROCESS.md` §4.2.
