# RW-40 — TASKS-DETAIL: разбивка нереализованных функций

Дата: 2026-10-24. PM: DeepSeek. Порядок = очередь исполнения (P0 → P1 → P2),
внутри — зависимости. Каждая задача: один owner, red-first, evidence, тесты не ослаблять.

## Очередь 1 — качество auth (сейчас)

| ID | Задача | Owner | Файлы | Приёмка |
|---|---|---|---|---|
| **A5** | QA-AUTH offline: 401/429/captcha/MFA/logout/leaks | G4 QA | `QA/AUTH-REPORT.md` | матрица, 0 утечек, P0/P1=0 |
| A6-P2 | регресс-тесты: clear-error + logout-reject | G4/G2 | auth/net tests | 2 теста, не ослабить |
| A2b | UI captcha session/rqdata в retry | G2 | `auth.js`, auth tests | новые поля в args, тест |

## Очередь 2 — сообщения P1

| ID | Задача | Owner | Файлы | Зависит |
|---|---|---|---|---|
| **B3a** | backend реакций: PUT/DELETE reaction | G1 | `core/rest/client.rs` (put/delete), `gui-spike/src/reactions.rs`, `main.rs`, tests | — |
| B3b | UI реакций: клик ставит/снимает, счётчик, ошибки | G2 | `ui/features/reactions.js`, `chat.js`, tests | B3a |
| B4a | меню сообщения: правка/удаление (backend) | G1 | `messages.rs`, routes, tests | B1a |
| B4b | меню сообщения UI: действия, confirm, optimistic | G2 | `message-toolbar.js`, tests | B4a |
| B2a | вложения backend: upload (multipart) | G1 | `messages.rs`, core client, tests | B1a |
| B2b | вложения UI: drag&drop, прогресс, превью | G2 | `composer.js`, `chat.js`, CSS, tests | B2a |

## Очередь 3 — люди/навигация P1

| ID | Задача | Owner | Файлы | Зависит |
|---|---|---|---|---|
| B5a | участники: группы по ролям/статусу (backend) | G1 | `data.rs`, `members.rs`, tests | — |
| B5b | участники UI: группы, полный поиск | G2 | `members.js`, CSS, tests | B5a |
| B6 | QuickSwitcher Ctrl+K | G2 | `features/quick-switcher.js`, tests | A3 |
| B7 | серверный поиск (REST) | G1+G2 | `search.rs`, `search-overlay.js`, tests | A3 |

## Очередь 4 — настройки/система P1–P2

| ID | Задача | Owner | Файлы |
|---|---|---|---|
| B8 | сохранение настроек (store/kv) | G1+Core | `auth/store`, `settings`, tests |
| B9 | уведомления D-Bus в UI | G5+G2 | platform, UI, tests |
| C3 | темы light/cozy | G3+G2 | tokens, CSS, tests |
| C4 | i18n-рантайм en/ru | Core+G2 | i18n, UI, tests |
| C5 | голос UI: устройства/PTT/mute | G5+G2 | voice/platform, UI |
| C6 | трей/автозапуск/deep links/updater UI | G5 | platform/updater, UI |

## Очередь 5 — треды/профиль P2

| ID | Задача | Owner | Файлы |
|---|---|---|---|
| C1 | треды + пины | G2+G1 | chat, threads, backend |
| C2 | profile-popout, меню гильдии, DM-список | G2 | UI |

## Очередь 6 — релиз

| ID | Задача | Owner | Приёмка |
|---|---|---|---|
| D1 | e2e fixture включить (без live Discord) | G4 | 0 skip на fixture |
| D2 | QA-матрица `PLAN-E9` | G4 | P0/P1=0 |
| D3 | UI/Rust coverage + COV-01…05 | G4/Core | coverage, srtp/settings/voice |
| D4 | live READY + live send | G1+G4 | одно окно, тестовый аккаунт |
| D5 | merge gate: release + workspace + live | PM | passed, evidence |

## Правила очереди

1. Не больше 2 кодовых задач одновременно; `cargo` — один target-dir.
2. Следующая задача стартует после green предыдущей или явного параллельного решения PM.
3. Ни одна задача не закрывается без red/green и evidence.
4. P0 (live/auth) имеет приоритет над P1/P2.
