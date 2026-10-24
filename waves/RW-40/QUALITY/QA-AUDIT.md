# QA-AUDIT — что делает QA и где дублирование

Дата: 2026-10-24. PM. Вход: `tests/**`, `.gate/*.json`, `PM/qa-*`, `PM/*/REPORT.md`.
Цель — убрать повторные прогоны, не ослабляя проверки.

## 1. Что QA делает сейчас

| Слой | Объём | Что проверяет |
|---|---:|---|
| unit | 27 файлов, **588** | frontend DOM/model/bench, auth, CLI, CDP, walk, Frida, парсеры |
| meta | **53** | test-policy, plan-sync, docs-sync, waves-sync, breakpoints, keyring-live |
| smoke | **22** | CLI, Frida/pids, GUI window/AT-SPI |
| e2e | **4** | cdp browser fixture, walk fixture, live Discord opt-in |
| Rust gui | **238** | auth/net/messages/reactions/search/guilds/invites/typing/settings |
| Rust workspace | **1103** + 12 ignored | core/store/platform/voice/i18n/... |
| coverage | 99.74% | line/branch/functions |
| keyring live | 12 | Secret Service; release/qualify |

QA закрыл: QA4-01a/b, QA4-02/03, PLAT-02, PLAT-11 audit, G1/G2/G4 audits, mutation-smoke,
e2e fixture/skip analysis.

## 2. Где реально дублирование

| ID | Дублирование | Доказательство | Последствие |
|---|---|---|---|
| QA-D1 | Каждый воркер гоняет 6–9 frontend-файлов, хотя менял один модуль | logs `PM/*/green.log` | повторные 319/588 прогонов |
| QA-D2 | `gate:worker` гоняет полный unit+smoke+rust; затем `gate:release` — то же снова | `scripts/gate.mjs` | двойной полный прогон на batch |
| QA-D3 | Coverage 99.74% считается полным unit-набором в каждом release | gate coverage phase | дорого, не даёт новой информации при неизменном коде |
| QA-D4 | Unit и smoke оба проверяют `--help`/`--version` | `cli-run.test.mjs`, `cli.smoke.test.mjs` | два оракула на один CLI-контракт |
| QA-D5 | «Frida недоступна → код 2» повторяется в нескольких кейсах `cli-run` | `cli-run.test.mjs` | одинаковый setup/assert |
| QA-D6 | E2E 4/4 skip; «зелёный» e2e фактически не проверяет браузер | gate e2e | статус pass не доказан |
| QA-D7 | Source-scrape тест `token_or` вместо поведения | `guilds_contract.rs` | форма проверена, поведение — нет |
| QA-D8 | Mutation-проверки разовые, не ротируются | `TEST-AUDIT` 6 caught/5 missed | покрытие мутаций не системное |

## 3. Что QA НЕ должен делать

- повторять полный suite за каждым воркером;
- принимать `skip` за pass;
- проверять реализацию вместо контракта/оракула;
- держать «вечный» source-scrape как единственное evidence;
- гонять live без окна PM.

## 4. Приоритеты улучшений

| P | Действие | Владелец |
|---|---|---|
| **P1** | Test impact selector: воркер запускает только затронутые тесты | G4 |
| **P1** | Batch gate: полный frontend/rust один раз на batch | PM/G4 |
| **P1** | E2E critical flows + browser CI; skip виден | G4 |
| **P1** | Убрать source-scrape `token_or`, заменить поведением | G1+G4 |
| P2 | Консолидировать duplicate help/version/Frida tests | G4 |
| P2 | Coverage — на release/qualify, diff coverage для карточек | G4 |
| P2 | Mutation rotation 1–2 на batch | G4 |
| P3 | Split giant frontend tests | G4 |

## 5. Метрики

- first-pass gate rate;
- duplicate suite runs per batch;
- e2e pass/skip ratio (skip не pass);
- mutation kill rate по изменённым модулям;
- cycle time red→accepted;
- число тестов, не привязанных к контракту.
