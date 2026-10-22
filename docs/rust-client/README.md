# Rust-клиент Discord: концепт, архитектура, план, тесты

Раздел готовит создание **полноценного клиента Discord на Rust** — нативный бэкенд в оболочке
Tauri (системный WebView; без Electron и без встроенного Chromium). Всё построено на данных
обратного инжиниринга волны R7
(`../re/` — шлюз, REST, голос, UI, i18n, бэкенд) и прежних волн (`../feature-backlog.md`,
R4/R5/R6/D).

## Состав

| Документ | Содержание |
|---|---|
| `STATUS.md` | **Фактическое состояние:** 25/90 принято, 580 тестов, крейты, блокеры, доказательства |
| `ROADMAP.md` | **Дальнейший план:** batch-волны, вехи и решения владельца |
| `TEAM.md` | **Отдел разработки:** seats, RACI, pipeline, WIP, playbook воркера, KPI |
| `WAVE-PLAN.md` | **Batch-план:** 90 карточек → ~18–22 волны, ready-queue, критический путь, сроки |
| `CONCEPT.md` | Полный концепт: что делает приложение, принципы, стек, сценарии, критерии успеха |
| `FEATURES.md` | Каталог функций (ID, зачем, как работает по RE, источник, фаза) — 14 групп |
| `ARCHITECTURE.md` | Процессы, крейты, шины, хранилище, замена Electron-IPC, ADR по UI, 8 графов Mermaid |
| `PLAN.md` | Волны P0–P8: 90 карточек с DoD, тестами, зависимостями, размерами; гейты и риск-план |
| `TEST-PLAN.md` | Уровни L1–L5, манифест RE-фикстур, каталог ~130 тестов, red-first протокол |
| `CODE-STANDARD.md` | **Эталонный метод:** research → frozen contract → red-first → mutation/review → gate; безопасность, async/UI, Definition of Done, уроки волн |
| `LEGAL.md` | **Правовая рамка:** ToS, телеметрия off, честный отпечаток, секреты/ассеты, живые операции только по approval |
| `CODEX-WORKERS.md` | **Codex CLI playbook:** роли, sandbox, промты воркера/ревьюера, DAG, mutation feedback loop |
| `PROJECT-STRUCTURE.md` | Строение объединённого проекта: лаборатория ↔ репозиторий ↔ `fixtures/` ↔ `rust/`, правила раскладки |
| `TRACEABILITY.md` | Функции → спека → фикстуры → тесты: матрица 14 групп (141 F / 133 T) |
| `VERIFICATION.md` | Журнал сверки сырья с документами: 20/20 проверок, разбор расхождений |
| `DEVELOPMENT-PROMPT.md` | Рабочий промт координатора/воркеров: карта, волны, RE-источники, TDD, агенты, ближайшая матрица |

## Как читать

0. **Понять, где мы сейчас** → `STATUS.md`; **что делать дальше** → `ROADMAP.md`.
1. **Понять продукт** → `CONCEPT.md` (§2 «Что делает приложение», §3 сценарии).
2. **Понять функции** → `FEATURES.md` (фильтр по фазе: F-* с колонкой «Фаза»).
3. **Понять устройство** → `ARCHITECTURE.md` (графы 1–7).
4. **Планировать работу** → `PLAN.md` (волновая карта и карточки P0-01…P8-05).
5. **Писать тест-первым** → `TEST-PLAN.md` §3 (ID тестов к каждой карточке).
6. **Запускать волну** → `TEAM.md` (роли, pipeline, WIP) и `WAVE-PLAN.md` (batch, ready-queue);
   `DEVELOPMENT-PROMPT.md` — исторический контекст и архитектура.
7. **Понять, что где лежит** → `PROJECT-STRUCTURE.md`; **сверить функцию с тестом** →
   `TRACEABILITY.md`; **убедиться, что данные сходятся** → `VERIFICATION.md` +
   `node verify/verify-re.mjs`.

## Источники (в этом репозитории)

- `../re/README.md` — индекс волны R7: методика, покрытие, границы достоверности.
- `../re/gateway-protocol.md`, `../re/rest-d02.md`, `../re/voice-deep.md` — протоколы.
- `../re/frontend-d03.md` — UI-контракты; `../re/i18n-deep.md` — локализация.
- `../re/main-arch.md`, `../re/backend-ipc.md`, `../re/native-re.md` — бэкенд и нативные модули.
- `../re/architecture.md` — «как устроен Discord» в графах (прообраз архитектуры клиента).

## Сырые артефакты

Лаборатория: **`/home/sanya/re-deep`** (1,7 ГБ, 22 072 файла): захваты шлюза/REST/UI, pcap и
strace голоса, каталоги i18n, распакованный asar. **Курированный срез** для конформанс-тестов —
`../../fixtures/` (12 записей, 1,59 МБ, `MANIFEST.json` с md5; полный манифест — `TEST-PLAN.md` §2).
Секреты в документах и фикстурах этого раздела заменены (`REDACTED`/placeholder).

## Каркас и проверки

- `../../rust/` — cargo-воркспейс (10 крейтов, офлайн): `cargo test --offline` внутри каталога
  (**580/580**; `discord-core` — 524). Волны: RW-01 — ETF-кодек (`discord-codec`), RW-02 —
  zstd-stream декодер (`ruzstd`, окно 64 КиБ), RW-03 — сессия шлюза (`discord-core::gateway`:
  HELLO/heartbeat 41 250/ACK/RTT/зомби), RW-04 — WS-транспорт (`ws`: ws://+wss://, пайп
  zstd→термы→сессия), RW-05 — IDENTIFY/READY-сводка, RW-06 — RESUME/реконнект (op6, 3 с/30 с,
  op9 `d`), RW-07 — роутер 21 диспатча, RW-08 — REST-каталог/заголовки/ретраи, RW-09 —
  X-Super-Properties, RW-10 — HTTP/1.1/TLS-транспорт, RW-11 — rate-limit/telemetry/ack,
  RW-12 — I1 REST-фасад и P2-01 READY (A1); RW-13 — P2-02 state/reducer; RW-14 — P2-06
  history; RW-15 — P2-04 member list; RW-16 — P2-05 chunks; RW-17 — P2-07 read-state
  (все приняты 2026-10-20); RW-18 — P2-09 presence; RW-19/RW-20 — P2-10/P2-11;
  RW-21 — P2-08/P1-01/P1-02/P0-04/P0-07 (B02 принят 2026-10-21);
  legacy `ready_data` удалён.
  Статусы — `STATUS.md`, `TASKS.md` §RW, `waves/STATE.md`; актуальная матрица — `TEAM.md`
  и `WAVE-PLAN.md`.
- `verify/verify-re.mjs` — оракул сырьё↔доки (20 проверок; коды 0/1/2), отчёт — `verify/report.json`.
- `../../fixtures/MANIFEST.json` — провенанс фикстур (источник, md5, T-ID).

## Статус

- Документы: **готовы** (2026-10-19); каркас `rust/` и фикстуры `fixtures/` собраны; верификация
  сырья — **20/20** (`verify/report.json`). Реализация — **25/90 принято**, B02 (P2-08, P1-01, P1-02,
  P0-04, P0-07) принят 2026-10-21; подробно — `STATUS.md`. Следующий batch — B03;
  гейт B02 — passed (`../../waves/RW-21/gate-worker-b02.json`).
- Входные риски и пробелы RE перечислены в `CONCEPT.md` §7; открытые вопросы D-04…D-08 — в
  `../feature-backlog.md`.
- Синхронность документов стережёт мета-тест `../../tests/meta/rust-client-sync.test.mjs`.
