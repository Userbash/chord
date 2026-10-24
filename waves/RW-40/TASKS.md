## СТАТУС: АКТИВНА (2026-10-24, после паузы)

Frontend 212/0, core 574/0, gui 161/0, workspace 1112/0.
Открыт **G6 RE Parity** и **Quality-программа**: `QUALITY/STRATEGY.md`,
`QUALITY/BASELINE-QUALITY.md`. WIP: ≤2 code + 2 read-only audit.

# RW-40 — TASKS: отделы, роли, распределение, доска

Дата: 2026-10-24. PM: DeepSeek. Источник истины по процессу — `TEST-POLICY.md`/`PROCESS.md`.

## 1. Отделы и роли

| Отдел | Роль | Зона | Файлы |
|---|---|---|---|
| **Design** | UX/UI | login, темы, компоненты, паритет | `DESIGN/**`, `NET/LOGIN-SPEC.md` |
| **Frontend** | UI-код | экраны, auth, сообщения, настройки | `ui/**`, `tests/unit/gui-spike-*` |
| **Backend** | BFF/auth | команды, keyring, события, ошибки | `gui-spike/src/**`, `tests/auth_contract.rs` |
| **Network** | live-сеть | gateway, REST, reconnect, READY | `core/rest|ws|gateway`, `gui-spike/src/net*` |
| **Core** | протокол/состояние | state/store/history/voice | `rust/crates/discord-*` |
| **QA** | проверки | unit/meta/smoke/e2e/AT-SPI/матрица | `tests/**`, `QA/**` |
| **Security** | ревью | утечки, 401/429, ToS-границы | `REVIEW*` |
| **G6 RE Parity** | сверка с эталоном | REA, inventory, matrix, gaps, priority gate | `waves/RW-40/PARITY/**` |
| **PM** | оркестрация | брифы, приёмка, гейт, журнал | `PM/**` |

## 2. Доступность исполнителей

| Исполнитель | Статус | Примечание |
|---|---|---|
| Субагенты-воркеры | **доступны** | ≤2 кодовых одновременно |
| Codex CLI | **только review/security** | код не пишет |
| G6 REA-сессия | **active** | отдельная сессия, read-only эталон |
| QA-воркер | доступен | targeted/gate |
| PM | доступен | планы, приёмка, live-окна |

Правило: ≤2 кодовых воркера; Codex — только review; live-окно — 1 на приёмку.
При падении баланса — не стартуем новые крупные потоки, закрываем критический путь.

## 3. Доска задач

### Фаза A — завершить auth (P0, блокер всего)

| ID | Задача | Владелец | Файлы | Приёмка | Статус |
|---|---|---|---|---|---|
| A1 | Окно входа: captcha-adapter + tsc | **G2 Frontend lead** / Codex | `ui/features/auth.js`, `auth.css` | 41/41, 130/130, tsc 0, geometry 31/31 | **accepted (D-39)** |
| A2 | Captcha: session/rqdata/headers | **G1 Backend lead** / Codex | `auth.rs`, `main.rs`, тесты | red→green, 401/429 | **accepted (D-40)** |
| A3 | `connect`/`reader` + READY | **G1 Network lead** | `core/ws|gateway`, `gui-spike/src/net*` | стаб-тест + live | **accepted (D-44/D-45), live READY pending** |
| A3b | SupervisorEvent + AuthBackend::token | **G1 Core/Backend lead** / Codex | `gateway/supervisor.rs`, `auth.rs` | callback Ready/Stopped, token no-log | **accepted (D-42)** |
| A3c | NetController/state machine + FakeGateway | **G1 Network lead** / PM | `gui-spike/src/net.rs`, `tests/net_contract.rs` | 5 контрактных, 101/0 | **accepted (D-44)** |
| A3d | CoreGatewayDriver + net_* commands | **G1 Network lead** / Codex | `net.rs`, `main.rs` | 103/0, clippy/fmt | **accepted (D-45)** |
| A4 | `auth://state` + logout stop | **G1 Backend** / Codex | `main.rs`, `auth.rs` | событие + идемпотентность | **accepted (D-46)** |
| A5 | QA-AUTH матрица | **G4 QA** | `QA/AUTH-*` | 401/429/captcha/MFA/logout/leaks | **accepted (offline PASS; live pending)** |
| A6 | Codex-Security | **G4b Security** | `REVIEW-auth.md` | P0/P1=0 | **accepted (D-49)** |
| A6-P2 | регресс logout-reject | **G4 QA** | auth test | **accepted (D-62)** |

### Фаза B — продукт P0/P1

| ID | Задача | Владелец | Зависит |
|---|---|---|---|
| B1 | отправка сообщений (REST + optimistic) | **G1+G2** | A3 |
| B2 | вложения (upload/прогресс/превью) | Frontend+Network | B1 |
| B3a | backend реакций | **G1** | **accepted (D-53)** |
| B3b | UI реакций | **G2** | **accepted (D-56)** |
| B4a | edit/delete/reply backend | **G1** | **accepted (D-55)** |
| B4b | UI меню: edit/delete/reply | **G2** | **accepted (D-57)** |
| UI-FIX-1 | P1/P2 из Codex-review | **G2** | **accepted (D-66)** |
| B2a | вложения: upload backend | **G1** | **blocked: нет upload-контракта (ATTACH-RE)** |
| B7a | guild-search backend | **G1** | **accepted (D-64)** |
| B7b | UI сетевого поиска | **G2** | **accepted (D-65)** |
| B6 | QuickSwitcher Ctrl+K | **G2** | **accepted (D-59)** |
| B4 | меню сообщения (правка/удаление/ответ) | Frontend | B1 |
| B5 | группы участников + полный поиск | Frontend+Backend | A3 |
| B6 | QuickSwitcher Ctrl+K | Frontend | A3 |
| B7 | реальный поиск по серверу | Network+Frontend | A3 |
| B8a | settings persistence backend | **G1** | **accepted (D-68)** |
| B8b | settings UI hydration | **G2** | **accepted (D-69)** |
| B9 | уведомления (D-Bus) в UI | Platform+Frontend | A3 |

### Фаза C — P2

| ID | Задача | Владелец |
|---|---|---|
| C1 | треды/пины | Frontend+Network |
| C2 | profile-popout, меню гильдии, DM | Frontend |
| C3a | темы light/cozy (design) | **G3** | **accepted (D-71)** |
| C3b | темы light/cozy (CSS/UI) | G2 | queued |
| C4 | i18n-рантайм | Core+Frontend |
| C5 | голос UI (устройства/PTT) | Frontend+Platform |
| C6 | трей/автозапуск/deep links/updater UI | Platform+Frontend |

### Фаза D — качество/релиз

| ID | Задача | Владелец |
|---|---|---|
| D1 | e2e fixture (без live) включить | QA |
| D2 | QA-матрица `PLAN-E9` | QA |
| D3 | UI-coverage + COV-01…05 | QA/Core |
| D4 | один live READY | PM+QA |
| D5 | merge gate: gate:release + workspace + live | PM |

### Фаза N — live-сеть (план: `waves/RW-40/NETWORK-ROADMAP.md`)

| ID | Задача | Владелец | Приоритет | Статус |
|---|---|---|---|---|
| N0-01 | live READY | PM+G4+G1 | **P0** | queued: токен+окно |
| N0-02 | live send | PM+G4+G1 | **P0** | queued: после N0-01 |
| N0-03 | reconnect/offline UI | G1+G2 | P1 | queued |
| N1-01 | join/leave/list серверов (REST) | G1 | **P1** | join [L] accepted; invite/leave [S] queued |
| G-03a | enter guild/lurker backend | **G1** | **P1** | **accepted (D-76)** |
| N1-02 | меню сервера (join/leave) | G3+G2 | **P1** | design accepted (D-74); UI queued |
| N1-03 | переключение гильдий | G2 | P1 | queued |
| N1-04 | typing/presence | G1+G2 | P1 | queued |
| N1-05 | members-search | G1+G2 | P2 | deferred: `confirmed:false` |
| N1-06 | вложения | G1+G2 | P2 | **blocked**: upload-контракт |
| N1-07 | DM/друзья | G1+G2 | P3 | queued |
| N2-01 | RTC session manager | G1 | **P1** | queued |
| N2-02 | UDP transport live | G1 | **P1** | queued |
| N2-03 | SRTP live | G1+G4b | **P1** | queued |
| N2-04 | Opus pipeline | G5+G1 | **P1** | G5 plan accepted (D-113); queued |
| N2-05 | устройства/PTT/mute | G5 | **P1** | G5 plan accepted (D-113); queued |
| N2-06 | голосовой UI | G2 | **P1** | queued: дизайн |
| N2-07 | voice robustness/stats | G1+G5 | P2 | queued |
| N2-08 | voice security | G4b | P2 | queued |
| N3-01…03 | DAVE/MLS | G1+G4 | P2 | queued |
| N3-04…07 | screen/encode/RTP/Go Live | G5+G1+G2 | P2 | queued |
| N3-08…11 | camera/system audio/viewer/clips | G5+G2 | P3 | queued |
| N4-01…03 | notifications/tray/deep links | G5+G2 | P2/P3 | queued |

### Дизайн-паритет (G6 §13, `PARITY/DESIGN-PARITY.md`)

| ID | Задача | Владелец | Приоритет | Статус |
|---|---|---|---|---|
| DP-01 | media 1200/960/720, member/tree по SPEC | G2 | **P1** | **accepted (D-77)** |
| DP-02 | D-04: 26/48/+22, member 42, height index | G2+G4 | **P1** | **accepted (D-102)** |
| DP-03 | settings overlay 960×600 / 200% | G2 | P2 | queued |
| DP-04 | probe 5 viewport × 4 zoom + screenshot diff | G4 | **P2** | queued |
| DP-05 | resize/breakpoint meta-test | G4 | P2 | queued |

### Quality-программа (`QUALITY/STRATEGY.md`)

| ID | Задача | Владелец | Приоритет | Статус |
|---|---|---|---|---|
| Q-AUDIT-G1 | Rust production audit: unwrap/errors/dupes/unsafe | G1 | P1 | **done (D-80)** |
| Q-AUDIT-G2 | Frontend/test audit: large modules, dupes, dead code | G2 | P1 | **done (D-81)** |
| Q-AUDIT-G4 | test strength/mutation/e2e audit | G4 | P1 | **done (D-85)** |
| QA-IMPACT | test impact selector | G4 | P1 | **accepted (D-125)** |
| Q-G4-REG | G2-03/G2-11 regression tests | G4 | P1 | **accepted (D-87)** |
| Q-TOKEN-01 | token helper + remove Tauri alias | G1 | P1 | **accepted (D-82)** |
| Q-G2-P1 | 3 P1: composer reply, error overlay, auth focus-trap | G2 | P1 | **accepted (D-83)** |
| G1-01 | единый REST error classifier 401/403/404/429/5xx | G1 | P1 | **accepted (D-84); UI follow-up** |
| SEC-01 | keyring recv timeout | G1 | P1 | **accepted (D-96)** |
| G1-01-UI | search-overlay kinds для 403/404/client/server | G2 | P1 | **accepted (D-86)** |
| Q-02 | split `search-overlay.js` | G2 | P1 | after P1 fixes |
| Q-03 | split giant feature test | G4 | P2 | after audit |
| Q-04 | split `data.rs`/`auth.rs` by responsibility | G1 | P1 | queued |
| Q-05 | remove unexplained `allow(unused_imports)` | G1 | P2 | after audit |
| Q-08 | unsafe/FFI safety registry | G4b/G5 | P1 | audit |
| DP-02 | D-04 26/48/+22, member 42 | G2+G4 | P1 | queued |
| G-04 | leave server (`[S]`, codes `[A]`) | G1 | P1 | **in work** |

### Pipeline/load

- G6: `G6-REPORT-02` (decision-ready) — in work.
- G1 SEC-02, G2 DP-02 — code 2/2; G3 STREAM, G4 WALK-PLAN, G4b/G5 PLAT-11 — docs/audit.

### Process

Полный процесс: `DEVELOPMENT-PROCESS.md`.

### Master plan

Пул/приоритеты/владельцы: `waves/RW-40/MASTER-PLAN.md`.

### Merge/ready

Пул/очередь приоритетов: `waves/RW-40/QUEUE.md`.

Правила окна и возврата: `waves/RW-40/MERGE-POLICY.md`.

### Stub-replacement program (`STUB-MAP.md`, `STUB-IMPLEMENTATION-PLAN.md`, `STUB-TRACKER.md`)

| ID | Задача | Владелец | Приоритет | Статус |
|---|---|---|---|---|
| STUB-31/32 | CLI `walk` + `cdp eval/clicktext/list/shot` | G4 | **P1** | 32 accepted (D-90); 31 plan accepted (D-104) |
| STUB-33/34 | e2e fixture + gate/workspace | G4 | **P1** | QA4-01a accepted; QA4-02 in work |
| STUB-35 | поведенческий token test | G1+G4 | **P1** | queued |
| STUB-01/24 | READY → rail серверов | G2+G1 | **P1** | queued |
| STUB-04 | enter guild UI | G2+G1 | **P1** | backend accepted, UI |
| STUB-06 | leave server | G1+G2 | **P1** | **accepted (D-89/D-92)** |
| G-05/G-04 UI | меню сервера + leave confirm на fake invoke | G2 | **P1** | **in work** |
| STUB-05 | invite join | G1+G2 | **P1** | queued: live-коды |
| STUB-02 | guild menu | G2+G3 | **P1** | **accepted (D-92)** |
| STUB-08/09 | channel tree + switch | G2+G1 | **P1** | queued |
| STUB-21/22 | read state + typing/presence | G1+G2 | **P1** | queued |
| DP-02 | 26/48/+22, member 42 | G2+G4 | **P1** | queued |
| STUB-10 | pins/threads | G2+G1 | P2 | queued |
| STUB-13 | attachments | G1+G2 | P2 | **blocked** |
| STUB-28 | notifications/DND | G5+G2 | P2 | queued |
| STUB-19 | settings sections | G2+G3 | P2 | queued |
| STUB-26 | voice session | G1+G5 | **P1** | primitives only |
| STUB-27 | video/Go Live | G5+G1 | P2 | queued |

## 4. Параллельность и правила

- ≤2 кодовых воркера; `cargo` — один на target-dir.
- Один файл — один владелец; общие файлы — PM/интеграция.
- Каждая задача: red-лог → green → targeted → evidence.
- Автор ≠ ревьюер; Codex-Review на P0/P1.
- 2 одинаковых отказа → смена подхода; 3-я попытка запрещена (`loop-guard`).

## 5. Текущий фокус

- **G6**: инвентарь 91 функций принят (D-75); P1 топ — L-01/G-01/G-05/C-02/G-04, G-03a можно кодировать (`[L]` 204).

1. **C3a** — дизайн тем light/cozy (G3, docs-only); затем C3b CSS/переключение.
2. **N1-01** — подтвердить payload join/leave и сделать backend (G1); это первый
   live-network продуктовый поток.
3. **N1-02** — дизайн меню сервера (`DESIGN/SERVER-MENU.md`), затем UI.
4. **N0-01/N0-02** — подготовить live-харнесс; окно только по согласованию.
5. **N2-01/N2-02** — после N1: RTC-сессия и UDP, затем SRTP/Opus.
6. C4/C5/C6 и D1–D5 — после N1/N2 MVP, без распыления.
