# W26b-fix — отчёт по замечаниям приёмки (PROCESS.md §4.2)

- **batch/карточка**: RW-25 / B06, лейн B — **P7-02: перф-харнесс T-HARN-04**, доработка по
  замечаниям приёмки Team Lead (**F1** P1, **F2** P2, **F3** P1).

- **сделано** (по замечаниям):
  - **F1** — правило сравнения метрик изменено: база `0.0` и текущая `0.0` ⇒ `Regression::None`;
    база `0.0` и текущая `> 0.0` ⇒ **`Regression::Regressed`** (деградация «метрика завелась с
    нуля»), `delta_pct` = `f64::INFINITY` (процент от нуля не определён — формат зафиксирован в
    doc-комментарии `compare`); база `> 0` — прежнее правило порога 10 %. Метрики по-прежнему
    проверяются в фиксированном порядке (холодный старт → ingest → селектор).
  - **F2** — метрики времени снова **JSON-числа** (`f64`), строковый обход (`#[serde(with = "metric")]`)
    удалён; точность обеспечивает feature `serde_json/float_roundtrip` (Cargo.toml изменён Team Lead,
    мой файл — не трогал, `lexical` внутри `serde_json` — свой модуль, `Cargo.lock` не требует правок).
    Эталон `tests/data/baseline-reference.json` перегенерирован под новый формат.
  - **F3** — обходной путь `PRESENCE_SAMPLE = 64` **удалён**: presence получают **все** участники
    нагрузки (в т.ч. все 24 229 в референсе), группа «offline» пуста (`count: 0`). Обоснование:
    Team Lead исправил `upsert_presence` (диапазон по ключу `(user_id, scope)`), квадратичности больше нет.

- **red** (лог: `waves/RW-25/red-W26b-fix.log`; каждый прогон — падающий тест, exit code ≠ 0):

  | # | команда | результат | exit code |
  |---|---|---|---|
  | F1 | `cargo test --offline -p discord-perf --test perf_contract regression_` | `regression_zero_baseline_to_nonzero_k` FAILED: «ожидалась регрессия, получено None» | **101** |
  | F2 | `cargo test --offline -p discord-perf --test perf_contract json_float_roundtrip_k` | FAILED: «метрика времени — JSON-число, получено "0.048010000000000004"» (строка) | **101** |
  | F2b | `cargo test --offline -p discord-perf --test perf_contract reference_report_k` | FAILED: «эталон — валидный JSON отчёта: Error("invalid type: string \"44.802397\", expected f64")» — старый файл базы несовместим | **101** |
  | F3 | `cargo test --offline -p discord-perf --test perf_contract presence_for_every_member_k` | FAILED: `left: 64, right: 200` (presence только у выборки) | **101** |

- **green** (лог: `waves/RW-25/green-W26b-fix.log`):

  | команда | результат | exit code |
  |---|---|---|
  | `cargo test --offline -p discord-perf --test perf_contract` | **17 passed; 0 failed** | 0 |
  | `cargo test --offline -p discord-perf` | **17 passed; 0 failed** (+ doc-tests 0) | 0 |
  | `cargo clippy --offline -p discord-perf --all-targets -- -D warnings` | без замечаний | 0 |
  | `cargo fmt -p discord-perf -- --check` | чисто (только свой крейт: `discord-voice` дописывает другой воркер) | 0 |
  | `cargo test --offline --release -p discord-perf --test perf_contract` | **17 passed; 0 failed** | 0 |

  Новые тесты: `regression_zero_baseline_to_nonzero_k` (F1), `json_float_roundtrip_k` (F2),
  `presence_for_every_member_k` (F3); `regression_zero_baseline_k` переписан под новое правило
  (база 0.0 и текущая 0.0 ⇒ `None`), в `json_roundtrip_k` ассерт сменён со «строка» на «число»
  (формат базы изменён по решению Team Lead — фикс контракта, а не подгонка под код).

- **targeted**: `waves/RW-25/green-W26b-fix.log` — 17 тестов `tests/perf_contract.rs`.

- **gate:worker**: не запускался — воркер в batch; полный гейт — у Integration/QA.

- **измеренные цифры** (AMD Ryzen 7 7800X3D, 16 лог. CPU; на машине параллельно работают другие лейны,
  поэтому разброс прогонов ±10 мс):

  | метрика | release, полный presence | release, было (выборка 64) | debug, полный presence | бюджет |
  |---|---|---|---|---|
  | холодный старт, мс | **46,6 / 48,2 / 47,5 / 49,0** (медиана 3 прогонов в каждом замере) | 46,1–47,5 | ~285 | ≤ 1500 (референс) |
  | ingest, мс | **56,3–58,6** | 56,5–62,2 | ~270 | — |
  | селекторы, мкс | **14,6–14,7** | 11,8–15,7 | ~116 | — |
  | участников в состоянии | 24 229 (presence у **всех**) | 24 229 (presence у 64) | 24 229 | = `spec.members` |
  | сообщений применено | 100 000 | 100 000 | 100 000 | = `spec.messages` |
  | digest референса | `4194907760224858559` | то же | то же | — |

  Полный presence добавил к холодному старту ~1–3 мс (24 229 вставок в `state.presences` с
  поиском по диапазону ключа) — запас по бюджету ×30. `budget_cold_start_k` на release **не
  пропускается** (≥ 8 лог. CPU) и проверяет медиану 3 прогонов ≤ 1500 мс; в debug — skip-aware
  с причиной в stderr.

- **формат базы изменился (F2)**: `tests/data/baseline-reference.json` — метрики времени были
  **строками** (`"cold_start_ms": "44.802397"`), теперь **числа** (`"cold_start_ms": 55.580174`).
  **Старый файл несовместим** с новым парсером: `report_from_json` возвращает
  `Error("invalid type: string \"44.802397\", expected f64")` — это зафиксировано red-прогоном F2b.
  Файл перегенерирован на текущей машине (не эталонная машина CI) — машинно-независимая часть
  контракта (`spec`, `digest`, счётчики) неизменна и проверяется тестом.

- **файлы** (md5):

  | файл | md5 |
  |---|---|
  | `rust/crates/discord-perf/src/workload.rs` | `01ff9490e9655b3d092146a3d2c0063e` |
  | `rust/crates/discord-perf/tests/perf_contract.rs` | `a83b990c5861a37e3e8ba10b150b7ae8` |
  | `rust/crates/discord-perf/tests/data/baseline-reference.json` | `0071e189c3f223e681de987cace8fb6f` |
  | `waves/RW-25/red-W26b-fix.log` | `37583db16547c44ea33ae42faee3cb53` |
  | `waves/RW-25/green-W26b-fix.log` | `6096580992bdcad2f4baf7ae3300109d` |

  Не тронуты: `Cargo.toml`/`Cargo.lock` крейта (Cargo.toml менял Team Lead, `float_roundtrip`
  подключён им; `Cargo.lock` править не потребовалось — `serde_json` везёт `lexical` сам),
  `discord-core` (исправление presence — Team Lead), доки, `waves/db.jsonl`, `STATE.md`,
  `STATUS.md`, `PLAN.md`, `TASKS.md`, `docs/map.md`. Команды cargo — только офлайн, только
  со своим `CARGO_TARGET_DIR=/tmp/opencode/target-w26b`; `cargo fmt -p discord-perf` (не `--all`).

- **риски/вопросы**:
  1. **F1 меняет семантику `compare`**: база с нулевой метрикой больше не считается «нет данных» —
     теперь это регрессия, если текущая метрика ненулевая. Для потребителей базы CI-истории это
     означает, что **первый** прогон (когда базы ещё нет) формально «красный», если метрика ненулевая.
     Так и задумано Team Lead («скрывать деградацию нельзя»), но при сравнении «с нуля» с базой
     из другого прогона нужно трактовать `delta_pct = INFINITY` как «базы не было».
  2. `delta_pct = f64::INFINITY` в JSON не представима (`serde_json` не сериализует бесконечности) —
     если отчёт с такой регрессией поедет в CI-историю, её нельзя записать как JSON без
     преобразования в `null`/строку. Сейчас `Regression` в JSON не пишется (в базе только `PerfReport`),
     но при расширении базы это надо учесть.
  3. **F2**: точность JSON-round-trip теперь зависит от feature `float_roundtrip` у `serde_json`.
     Если кто-то пересоберёт крейт без него (или соберёт в другом окружении) — round-trip снова
     станет неточным на 1 ULP. Стоит зафиксировать feature в `Cargo.toml` крейта (уже сделано) и
     не откатывать его молча.
  4. **F3**: холодный старт вырос на ~1–3 мс; запас по бюджету сохраняется (×30). Замеры на этой
     машине шумные (параллельно работают другие лейны, разброс прогонов до ±10 мс) — для эталонной
     цифры нужен прогон на референсной машине CI.
  5. База `tests/data/baseline-reference.json` снята на машине воркера; цифры надо переснять при
     первом прогоне CI (контракт `spec`/`digest` машинно-независим).
  6. UI-часть P7-02 (T-UI-19, 60 fps на 24k участников) по-прежнему вне карточки.

- **осталось открытым**:
  1. Нужно ли Team Lead решение по п. рисков 2 (`delta_pct = INFINITY` в JSON-базе регрессий)?
  2. Базовые цифры — переснять на референсной машине CI (сейчас эталон снят на машине воркера).
  3. Публичные `base_state()` и константы `PERF_*` — оставить или убрать (вопрос из прошлого отчёта
     остаётся открытым).
  4. Нужен ли отдельный CLI/скрипт прогона базы вместо печати JSON из теста `reference_report_k`.

- **следующий шаг**: Team Lead — повторная приёмка P7-02 по §4.3 и синхронизация
  `STATUS`/`ROADMAP`/`TASKS`/`map`/`STATE`; общий batch-гейт.
