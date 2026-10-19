# Отчёт W8a — каталог REST-маршрутов v9 (RW-08 / P1-07 / T-REST-01)

- **сделано:** `rust/crates/discord-core/src/rest/routes.rs` — данные и логика поверх
  замороженного каркаса §4.1 (сигнатуры не менялись; добавлены только приватные хелперы
  `route`, `placeholder_count`, `next_placeholder`):
  - `ROUTES` — 47 маршрутов в порядке выдержки `rest-routes-47.tsv`: GET 40, POST 3, PUT 2,
    PATCH 2, DELETE 0; пути относительные (без `/api/v9`), `body_keys` дословно (6 маршрутов с
    телом, сентинелы `(empty-object)`/`(non-json)`); процентные escape-последовательности
    (`%F0%9F%91%8B`, `%40me`) сохранены как в захвате;
  - `find` — точное равенство `(method, path)` (без базы, без префиксов), линейный поиск по 47;
  - `render_path` — число плейсхолдеров `{id}`/`{num}` (15+2 в каталоге), проверка
    `parts.len()` → `PartsMismatch`, затем пустая/со скобками часть → `InvalidPart`, затем
    подстановка слева направо; без паник и без поглощения ошибок.
  Тест `rust/crates/discord-core/tests/rest_routes.rs` — 8 кейсов U1–U8, читает только публичный
  API и выдержки `tests/data/*.tsv` через `CARGO_MANIFEST_DIR`; в шапке — провенанс (источник
  `docs/re/rest-d02.json` md5 `87d7a05d8cd11f3b8ae37aa5c9811c9e`, md5 выдержек, jq-команды,
  счётчики 47/40-3-2-2-0, 44/2/1). Оракул перепроверен независимо: `jq '.endpoints|length'` → 47;
  распределение методов 40/3/2/2; вердикты кросс-сверки 44/2/1; md5 источников совпали с ТЗ §3.
- **red:** `cd rust && cargo test --offline -p discord-core --test rest_routes` →
  **1 passed / 7 failed**, `# EXIT=101`; падают U1 (пустой путь), U2 (нет маршрута), U3 (GET 47≠40),
  U4 (0 маршрутов с телом), U5 (`find` → None), U7 (нет маршрута для `render_path`), U8 (нет
  маршрута); проходит U6 (`as_str` каркаса) — ровно red-ориентир ТЗ §4.2.
  Лог: `/tmp/opencode/rw-08/w8a/red.log`; md5 теста `ae203dec7708d2f8164e374a946db62a`;
  копия `/tmp/opencode/rw-08/w8a/rest_routes-red.rs` (md5 совпадает побайтово, `diff` пуст).
  Тест после red не менялся (md5 финального = md5 в red-логе).
- **green:**
  1. `cargo test --offline -p discord-core --test rest_routes` **×5** подряд → каждый раз
     **8 passed / 0 failed**, `# RUN=1..5 EXIT=0` (`/tmp/opencode/rw-08/w8a/green.log`);
  2. `cargo test --offline -p discord-core` → **84 passed / 0 failed**, `# FULL_EXIT=0`
     (наш бинарь 8/8; `rest_headers` W8b 5/5; `rest_retry` W8c на момент прогона ещё не появился
     в дереве — чужие файлы не трогались);
  3. `rustfmt --check` своих двух файлов → `# FMT_EXIT=0`; `rustfmt` применён только к ним;
  4. `cargo clippy --offline -p discord-core --all-targets -- -D warnings` → **чисто**,
     `# CLIPPY_EXIT=0` (`/tmp/opencode/rw-08/w8a/clippy.log`), чужих замечаний нет.
- **gate:worker:** **passed** — `.gate/worker-2026-10-19T18-49-10-397Z.json`:
  typecheck ✓, meta 25/25, unit 169/169, smoke 20/20, skip 0, warnings 0 (`# GATE_EXIT=0`;
  лог `/tmp/opencode/rw-08/w8a/gate-worker.log`).
- **файлы:**
  - `rust/crates/discord-core/src/rest/routes.rs` — md5 `0aab077171d97fc98f944e4b75f7c350`;
  - `rust/crates/discord-core/tests/rest_routes.rs` (новый) — md5
    `ae203dec7708d2f8164e374a946db62a` (= red-копия);
  - артефакты: `/tmp/opencode/rw-08/w8a/{red.log, green.log, clippy.log, gate-worker.log,
    rest_routes-red.rs, report.md}`.
  Границы соблюдены: `Cargo.toml`/`Cargo.lock`, `tests/data/*`, чужие `src`/`tests`, `docs/`,
  `waves/`, golden и `/home/sanya/re-deep` не менялись; git-мутаций не было; сеть не использовалась.
- **риски/вопросы:**
  1. `find` — линейный поиск по 47 элементам (O(47), без аллокаций). Для каталога такого размера
     этого достаточно; индексация/`match` не нужны, контракт §4.1 не требует.
  2. `PathRenderError::InvalidPart` возвращается после проверки числа частей: при одновременно
     неверном числе и плохой части приоритет — `PartsMismatch` (так записано в §4.1; тест
     фиксирует обе ветки отдельно).
  3. Сентинелы тела `(empty-object)`/`(non-json)` — данные оракула, не валидация JSON; разбор
     тел — вне волны (P1-09/P1-12).
  4. `DELETE` в каталоге не встречается (0 маршрутов), вариант enum оставлен для будущего —
     тест U6 это фиксирует.
