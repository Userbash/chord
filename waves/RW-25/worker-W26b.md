# W26b — отчёт воркера (PROCESS.md §4.2)

- **batch/карточка**: RW-25 / B06, лейн B — **P7-02: перф-харнесс T-HARN-04** (крейт `discord-perf`).
  Контракт: `waves/RW-25/TZ-W26b.md` §1–§6.

- **сделано**:
  - `rust/crates/discord-perf/src/workload.rs` — реализация замороженного API ТЗ §3:
    `WorkloadSpec` (+`reference()`), `Workload`, `build_workload`, `Metric` (+`as_str`),
    `PerfReport`, `REGRESSION_THRESHOLD_PCT`, `Regression`, `compare`, `measure`,
    `report_to_json`/`report_from_json`.
  - Генератор нагрузки: собственный **SplitMix64** (документирован в модуле), только `seed`,
    без `SystemTime`/`random`. Участники — `EventPayload::GuildMemberListUpdate` с реальным API
    `MemberListId`/`MemberListOp::Sync`/`MemberListGroup`/плоский список `group|member`;
    сообщения — `EventPayload::MessageCreate(Message{..})`; `seq` монотонный (иначе stale-политика
    редьюсера отбросила бы события).
  - `measure` меряет **реальное** время (`std::time::Instant`) на реальном редьюсере
    `discord_core::state::apply`; `members_loaded`/`messages_ingested` берутся из фактического
    состояния (`state.message_count()`, перебор `member_lists`), а не из `spec`.
  - `digest` — FNV-1a 64 по фиксированному порядку обхода (документирован в модуле и продублирован
    независимой реализацией в тесте).
  - `compare` проверяет три метрики в фиксированном порядке; нулевая базовая метрика регрессию не
    считает (первый прогон не «красный»).
  - `rust/crates/discord-perf/tests/perf_contract.rs` — 14 контрактных тестов (ТЗ §4 требует ≥12).
  - `rust/crates/discord-perf/tests/data/baseline-reference.json` — базовый прогон CI-истории ТЗ §6.
  - Дополнительно к замороженному API (нужно для независимого оракула в тестах, помечено в отчёте
    Team Lead): публичные `base_state(spec) -> Result<State, ReadyError>` и константы
    `PERF_GUILD_ID`, `PERF_MEMBER_LIST_ID`, `PERF_CHANNEL_BASE`, `PERF_USER_BASE`,
    `PERF_MESSAGE_BASE`, `PERF_SELF_USER_ID`, `PRESENCE_SAMPLE`.

- **red**:
  - `cd rust && CARGO_TARGET_DIR=/tmp/opencode/target-w26b cargo test --offline -p discord-perf --test perf_contract`
    → **exit code 101**: `error[E0432]: unresolved imports discord_perf::workload::{Metric, PerfReport,
    Regression, Workload, WorkloadSpec, base_state, build_workload, compare, measure,
    report_from_json, report_to_json, REGRESSION_THRESHOLD_PCT, PERF_GUILD_ID, PERF_MEMBER_LIST_ID}`
    (API ТЗ §3 ещё не реализован — `src/workload.rs` был заглушкой).
  - лог: `waves/RW-25/red-W26b.log`.

- **green**:
  - `cargo test --offline -p discord-perf --test perf_contract` → **14 passed; 0 failed**, exit code 0.
  - `cargo test --offline -p discord-perf` → **14 passed; 0 failed** + doc-tests 0, exit code 0.
  - `cargo clippy --offline -p discord-perf --all-targets -- -D warnings` → exit code 0 (0 замечаний).
  - `cargo fmt -p discord-perf -- --check` → exit code 0.
  - `cargo test --offline --release -p discord-perf --test perf_contract` → **14 passed**, exit code 0.
  - лог: `waves/RW-25/green-W26b.log`.

- **targeted**: `waves/RW-25/green-W26b.log` — 14 тестов `tests/perf_contract.rs`
  (`determinism_k`, `different_seed_k`, `spec_reference_k`, `report_shape_k`, `ingest_applied_k`,
  `regression_none_k`, `regression_detected_k`, `regression_below_threshold_k`,
  `regression_zero_baseline_k`, `json_roundtrip_k`, `metric_names_k`, `threshold_constant_k`,
  `budget_cold_start_k`, `reference_report_k`).

- **gate:worker**: не запускался — воркер в batch; полный `gate:worker`/workspace-гейт — у Integration/QA
  на batch. Локально проверены только targeted-тесты своего крейта, clippy и fmt своего крейта.

- **измеренные цифры** (AMD Ryzen 7 7800X3D, 16 лог. CPU, Linux, единственный прогон):

  | метрика | release | debug (без оптимизации) | бюджет |
  |---|---|---|---|
  | холодный старт, мс | **46,6** (медиана 3 прогонов: 46,08 / 46,57 / 47,57) | ~248 | ≤ 1500 (референс) |
  | ingest (только сообщения), мс | **56,5** | ~247 | — |
  | селекторы, мкс | **15,7** | ~104 | — |
  | участников в состоянии | 24 229 | 24 229 | = `spec.members` |
  | сообщений применено | 100 000 | 100 000 | = `spec.messages` |
  | digest референса | `4194907760224858559` (одинаков в debug и release) | то же | — |

  Запас по бюджету холодного старта — ×32. `budget_cold_start_k` в release на этой машине
  **не пропускается** (≥8 лог. CPU) и проверяет медиану 3 прогонов ≤ 1500 мс; в debug-сборке тест
  **skip-aware** (в stderr печатается причина: бюджет — характеристика release-сборки), чтобы не было
  ложного флейка.

- **файлы** (md5):

  | файл | md5 |
  |---|---|
  | `rust/crates/discord-perf/src/workload.rs` | `f6f6762153b563bb01b720daf5008bdc` |
  | `rust/crates/discord-perf/tests/perf_contract.rs` | `aa06e38f9f90441b3412efe1cf68b859` |
  | `rust/crates/discord-perf/tests/data/baseline-reference.json` | `494c7c2bd610219f4ba9216ebc22cc1b` |
  | `waves/RW-25/red-W26b.log` | `a171a249cf840c3d690dd322b672cb1b` |
  | `waves/RW-25/green-W26b.log` | `bff82a74db0a8c7cbf562b6ba25f1f35` |

  Не тронуты (по границам ТЗ §1): `Cargo.toml`, `Cargo.lock`, `src/lib.rs` крейта, `discord-core`,
  другие крейты, доки, `waves/db.jsonl`, `STATE.md`, `STATUS.md`, `PLAN.md`, `TASKS.md`, `docs/map.md`.
  Команды cargo — только офлайн и только со своим `CARGO_TARGET_DIR=/tmp/opencode/target-w26b`.

- **риски**:
  1. **Форма JSON-метрик — строки, а не числа.** `serde_json` без feature `float_roundtrip` разбирает
     числа не всегда корректно (значащая часть переводится в `f64` до деления): `0.048010000000000004`
     читается как `0.04801` — на release-прогоне это ломало требование ТЗ «round-trip без потерь».
     Решение: метрики времени пишутся строкой кратчайшей круговой записи, читаются через
     `str::parse::<f64>()` (в Rust разбор округляет корректно) — round-trip точен для любого конечного
     `f64`. Побочный эффект: в JSON тайминги — строки (`"cold_start_ms": "46.57301"`), для jq нужен
     `tonumber`. Альтернатива (включить feature `float_roundtrip` в `Cargo.toml`) — файл вне моего ТЗ.
  2. **Находка по редьюсеру (не входит в карточку, но влияет на нагрузку):** `upsert_presence`
     сканирует `state.presences` целиком на каждого участника со статусом → O(n²) на 24 229
     участников. Поэтому в харнесе presence задан только выборкой `PRESENCE_SAMPLE = 64`, остальные
     участники идут без статуса. Если дать статус всем — холодный старт вырастет на порядки и уйдёт
     за бюджет. Стоит завести отдельную карточку на перф редьюсера presence.
  3. **Клонирование событий внутри `measure`:** `apply` забирает событие по значению, а API принимает
     `&Workload` → события клонируются для двух проходов измерения. Клонирование вынесено **вне**
     измеряемых интервалов (замеряется только работа редьюсера), но удваивает время прогона.
  4. **База состояния строится через READY-фикстуру (ETF):** `State` нельзя сконструировать вне
     `discord-core` (приватные `channel_guild`/`thread_guild`, у `Ready` приватное поле `raw`), поэтому
     база — `Ready::parse` + `State::from_ready`. Ошибка разбора базы не глотается: `measure` печатает
     причину в stderr и отдаёт отчёт с нулями, что ломает `report_shape_k` (нулевой «успех» не проходит).
  5. **Файл базы снят на машине воркера, а не на эталонной машине CI** — цифры надо переснять при
     первом прогоне CI; машинно-независимая часть контракта (`spec`, `digest`, счётчики) уже
     зафиксирована и проверяется тестом.
  6. **`cargo fmt --all` не запускался целиком:** `--check` показывает неформатированные файлы чужого
     крейта `discord-voice` (другая лейна). Форматировал только `-p discord-perf`; чужие файлы не
     откатывал (`git checkout`), чтобы не затереть чужую незавершённую работу. Флаг «fmt --all чист» на
     batch — к Integration.
  7. UI-часть P7-02 (T-UI-19, 60 fps на 24k участников) вне карточки — не делалась.

- **открытые вопросы (к Team Lead)**:
  1. Устраивает ли форма JSON-метрик (строки вместо чисел) или разрешается правка `Cargo.toml`
     (`serde_json` feature `float_roundtrip`)? Влияет на потребителей базы CI-истории.
  2. `base_state()` и константы `PERF_*` — оставить публичными (нужны независимому тестовому оракулу)
     или убрать в `pub(crate)`/приватные, если карточка не предусматривает расширения API.
  3. Нужен ли отдельный прогон базы (CLI/скрипт) для CI вместо печати JSON из теста
     `reference_report_k`?
  4. Заводить ли отдельную карточку на перф редьюсера presence (п. 2 рисков)?

- **следующий шаг**: Team Lead — приёмка карточки P7-02 по §4.3 (границы файлов, red-first, независимый
  оракул, отсутствие проглоченных ошибок), затем синхронизация `STATUS`/`ROADMAP`/`TASKS`/`map`/`STATE`
  и общий batch-гейт.
