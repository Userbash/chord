# QUALITY-STRATEGY — обучение на собственном коде и снижение энтропии

Дата: 2026-10-24. Владелец: PM (DeepSeek). Статус: **active**.
Связанные документы: `PARITY/G6-CHARTER.md`, `PARITY/DESIGN-PARITY.md`,
`TEST-POLICY.md`, `PROCESS.md`, `waves/RW-40/BASELINE-QUALITY.md`.

## 1. Цель

Не «переписать всё красиво», а **уменьшать дефект-плотность и стоимость изменений** при
сохранении поведения:

- меньше кода при том же контракте;
- меньше дублей и скрытых связей;
- ошибки типизированы, не глотаются;
- тесты ловят регрессии, а не имитируют проверку;
- каждая правка имеет источник (`[L]/[S]/[D]`) и evidence.

Главный принцип: **сначала тест/оракул, потом упрощение**. Рефакторинг без теста — запрещён.

## 2. Что считаем «слабым местом»

| Класс | Пример | Severity |
|---|---|---|
| P0 correctness/security | секрет в логе, паника на входе, тихий `unwrap`, ослабленный тест | stop-the-line |
| P1 maintainability | дубли команд, 1000-строчный модуль, размазанный state, копипаста обработчиков | в очередь |
| P2 test/coverage | нет negative-кейса, e2e skip, mutation не ловится | в очередь |
| P3 style/dead code | мёртвая ветка, `console.log`, лишний слой | backlog |

Не считаем «слабостью» длинный тест или явный `match`; тестовая ясность важнее краткости.

## 3. Метрики (без самообмана)

### 3.1 Обязательные гейты

- `cargo fmt --check`, `cargo clippy -D warnings`;
- `cargo test --workspace`, frontend unit, meta, smoke;
- `npm run gate:release`; e2e skip ≠ pass;
- `tests/meta/test-policy.test.mjs` — 0 нарушений.

### 3.2 Диагностические метрики

- размер файла (строки) и число функций;
- дублирование: одинаковые команды/маппинг ошибок/селекторы;
- `unwrap/expect/panic` **в production-коде** (тесты — отдельно);
- `unsafe` — только FFI, с safety-комментарием;
- `#[allow(...)]` — с причиной;
- покрытие критичных модулей (auth, ws, rest, store, voice);
- mutation-проверки на 5 критичных контрактах;
- e2e/AT-SPI сценарии без live.

Метрики не «улучшаем» удалением тестов или переносом кода в другой файл без выигрыша.

## 4. Learning loop

1. **Baseline**: зафиксировать метрики и список smelly-мест с `file:line`.
2. **Hypothesis**: что именно болит (дубли, связность, ошибки, тесты).
3. **Oracle**: characterization/contract-тест; для бага — red-тест.
4. **Small change**: один смысловой шаг, не массовый rewrite.
5. **Verify**: targeted → gate; сравнить LOC/дубли/покрытие.
6. **Review**: G6/independent reviewer; автор не ревьюит.
7. **Lesson**: запись в `QUALITY/LESSONS.md` — причина, паттерн, тест-защита.
8. **Only then next**: без накопления незавершённых рефакторингов.

## 5. Правила сокращения кода

Можно:

- объединять дублирующиеся команды/хелперы, если публичный контракт сохраняется;
- выносить общий error-mapping и path-builder;
- удалять мёртвый код после `rg` + тестов;
- заменять копипасту таблицей/итератором, если читаемость не падает;
- уменьшать число слоёв, если тест остаётся на границе контракта.

Нельзя:

- «оптимизировать» за счёт ослабления тестов;
- склеивать разные домены в один «универсальный» helper;
- удалять обработку ошибок ради краткости;
- менять публичные имена/команды без design/ADR;
- делать mass-rewrite без поэтапных зелёных прогонов.

## 6. Усиление тестов

1. Каждый баг: **red-тест → фикс → green**, тест остаётся.
2. Contract negative: 401/403/404/429, сеть, timeout, malformed JSON, missing token.
3. Границы: пустые/огромные строки, Unicode, snowflake, повторный клик.
4. Mutation-проверка: вручную ломать критичную ветку и убеждаться, что тест падает.
5. UI: состояния loading/empty/error/rollback, resize/zoom, клавиатура/aria.
6. E2E fixture без live; live — отдельное окно.
7. Coverage — диагностика, не цель; 100% бессмысленных assert не нужен.

## 7. База багов и уроков

- `QUALITY/BUGS.md` — ID, severity, repro, evidence, owner, status, тест.
- `QUALITY/LESSONS.md` — root cause, паттерн, prevention, ссылка на тест.
- Каждый accepted bug закрывается только с тестом-регрессией.
- Повтор одного класса ошибок → правило/линтер/тест, а не «ещё раз поправили».

## 8. MCP и инструменты

| Инструмент | Роль | Ограничение |
|---|---|---|
| `rea` | evidence эталона, строки/вызовы | read-only, без live |
| `basic-memory` | уроки/контекст между волнами | не источник истины кода |
| `context7` | актуальные API библиотек | не источник поведения Discord |
| `github` | PR/issues metadata | без коммитов от PM |
| `socraticode` | семантика/граф | **degraded**: Docker/Qdrant нет |
| `rg`/`cargo`/`node` | fallback-анализ | локально, без сети |

MCP не заменяет тест и не даёт права «додумать» функцию.

## 9. Очереди по отделам

| Отдел | Первый аудит | Первый refactor | Ограничение |
|---|---|---|---|
| G1 Core/Backend | unwrap/error mapping, REST/WS дубли, размер `auth.rs`/`data.rs` | merge `join_guild`/`enter_guild`, error helper | ≤1 cargo |
| G2 Frontend | 1000-строчные `search-overlay`/`chat`, дубли обработчиков, dead buttons | split module + shared state helper | без test weakening |
| G3 Design | tokens sync, DP-02, viewport/zoom | DP-02, layout probe | только SPEC |
| G4 QA | test inventory, mutation, e2e skip | mutation-набор, e2e fixture | skip ≠ pass |
| G4b Security | секреты/логи, unsafe FFI | redaction + grep gate | P0 |
| G5 Platform | unsafe/FFI, D-Bus, capture | safety comments, typed errors | no install |
| G6 Parity | evidence/priority gate | reject unsupported | `[L]/[S]/[D]` only |
| PM | WIP/acceptance | batches | ≤2 code |

Все отделы активированы, но одновременно **≤2 кодовых воркера**; аудит — read-only.
Первый batch: G1 + G2 quality audit. Второй: G4 test audit + один refactor. Далее — по метрикам.

## 10. Первый batch (P1)

1. `QUALITY/BASELINE-QUALITY.md` — нормализованные метрики (prod vs tests).
2. G1 audit: `QUALITY/G1-AUDIT.md`.
3. G2 audit: `QUALITY/G2-AUDIT.md`.
4. G4 audit: `QUALITY/TEST-AUDIT.md`.
5. Убрать дубль `join_guild`/`enter_guild` (после теста; один Tauri command + alias UI).
6. Вынести общий error-mapping (REST) без изменения кодов.
7. DP-02 (D-04) — не «оптимизация», а паритет.
8. G-04 leave — после RE-кодов; fake 204/403.

## 11. Definition of Done для улучшения

- поведение не изменилось (или баг исправлен по red-тесту);
- тесты не ослаблены; diff тестов проверен G6;
- targeted + gate; e2e skips явно перечислены;
- LOC/дубли/сложность: есть измеримый выигрыш или обоснование;
- публичный контракт/доки обновлены, если менялись;
- урок записан; следующий шаг не оставляет «полурефактор».
