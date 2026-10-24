# BUGS — реестр дефектов и quality-findings

Дата: 2026-10-24. PM. Правило: дефект без repro/теста не считается исправленным.
Severity: P0 security/live, P1 behaviour/parity, P2 quality/test, P3 style.

## Формат

`ID | P | Область | Симптом/repro | Evidence | Owner | Статус | Тест-регрессия`

## Открытые

| ID | P | Область | Симптом / repro | Evidence | Owner | Статус | Тест |
|---|---|---|---|---|---|---|---|
| Q-01 | P2 | Tauri | `join_guild` и `enter_guild` — два command над одним контрактом | `main.rs:244,255` | G1 | open | `guilds_contract.rs` |
| Q-02 | P1 | UI | `search-overlay.js` 1049 строк: remote/local/фильтры/пагинация в одном модуле | `search-overlay.js` | G2 | open | features tests |
| Q-03 | P2 | Tests | `gui-spike-features.test.mjs` 2303 строки, дубли setup | `tests/unit/...` | G4 | open | meta/none |
| Q-04 | P1 | Backend | `data.rs` 75 КБ / `auth.rs` 64 КБ — несколько ответственностей | file sizes | G1 | open | targeted |
| Q-05 | P2 | Core | 11 `#[allow(unused_imports)]` подряд в ready/snapshot | `snapshot.rs:393-413` | G1 | open | core test |
| Q-06 | P3 | UI | `console.log` в `app.js:228`, `metrics.js:125` | grep | G2 | open | smoke |
| Q-07 | P3 | Tauri | `#[allow(too_many_arguments)]` на command | `main.rs:375` | G1 | open | contract |
| Q-08 | P1 | FFI | 62 `unsafe` без реестра safety-инвариантов | store/platform | G4b/G5 | open | audit |
| Q-09 | P2 | E2E | 4 e2e skip, нет fixture-прогона без live | gate report | G4 | open | e2e fixture |
| DP-02 | P1 | UI parity | track 44/40 вместо D-04 26/48/42 | tokens.css | G2/G4 | **closed (D-102); legacy tokens P2** | model/features |
| DP-04 | P2 | UI parity | нет viewport/zoom probe | geometry | G4 | open | probe |
| G-04 | P1 | Guilds | leave не реализован; коды `[A]` | G6 matrix | G1 | open | `guilds_contract.rs` |

## Закрытые

— пока нет; закрывать только с green-тестом и ссылкой на diff.

## G1 audit findings (2026-10-24)

| ID | P | Область | Симптом | Evidence | Owner | Статус |
|---|---|---|---|---|---|---|
| G1-01 | **P1** | REST | 4 копии error-mapping; 403/404/5xx сводятся к `Network` | `rest_failure.rs` | G1 | **closed (D-84/D-86)** |
| G1-02 | **P1** | Tauri | чтение токена 6×; keyring→Network, missing→Unauthorized/MissingToken непоследовательно | `main.rs:150` helper | G1 | **closed (D-82)** |
| G1-03 | P2 | Auth | `Mutex<Inner>` держится через сетевой `/users/@me` | `auth.rs:1502-1528` | G1 | open |
| G1-04 | P2 | Keyring | `recv()` без таймаута → вечный spinner | `auth.rs:1075-1093` | G1 | open |
| G1-05 | P2 | UI crate | `reply_thread.rs` не в `lib.rs`; 5 `.bak*` в `src/` | `discord-ui/src` | G1/G5 | open |
| Q-01 | P2 | Tauri | `join_guild`/`enter_guild` — два command | `main.rs:244` | G1 | **closed (D-82)** |

## G2 audit findings (2026-10-24)

| ID | P | Область | Симптом | Evidence | Owner | Статус |
|---|---|---|---|---|---|---|
| G2-01 | **P1** | composer | любая клавиша при reply вызывает `clearReply` | `composer.js` | G2 | **closed (D-83)** |
| G2-02 | **P1** | errors | `.error-state` накрывает viewport; нет dismiss/auto-clear | `errors.js`, `app.js` | G2 | **closed (D-83)** |
| G2-04 | **P1** | a11y | auth-gate не в `topLayer`; Tab уходит в aria-hidden shell | `actions.js` | G2 | **closed (D-83)** |
| G2-03 | P2 | quick-switcher | empty-state инвертирован | `quick-switcher.js:100` | G2 | open |
| G2-05 | P2 | search | `applyFilter` игнорирует from/pinned/author | `search-overlay.js:1004` | G2 | open |
| G2-06…30 | P2/P3 | UI/tests | 12 P2 + 15 P3 из G2-AUDIT | `QUALITY/G2-AUDIT.md` | G2 | backlog |

| Q-11 | P2 | UI/CSS | `.content{position:relative}` выставлен inline из-за read-only CSS | `app.js:47-52` | G2 | open |

| QA4-01 | **P1** | E2E/gate | 2 fixture-теста skip из-за заглушек CLI; gate зелёный | `TEST-AUDIT.md` | G4 | open |
| QA4-02 | P1 | Gate | `cargo test --workspace` и CLI coverage не входят | `scripts/gate.mjs` | G4 | **closed (D-99)** |
| QA4-03 | P1 | Tests | source-scrape `token_or` вместо поведения keyring→Network | `guilds_contract.rs:332` | G1/G4 | open |
| G2-03 | P2 | quick-switcher | empty-state инвертирован | `quick-switcher.js` | G2 | **closed (D-87)** |
| G2-11 | P2 | toolbar | deletePending не сбрасывается | `message-toolbar.js:244` | G2 | **in work** |

| G-04 | P1 | Guilds | leave server backend отсутствует | G6 matrix | G1 | **backend closed (D-89); UI queued** |
| QA4-01a | P1 | E2E | cdp eval/clicktext/list/shot — заглушки | TEST-AUDIT | G4 | **in work** |

| SEC-01 | **P1** | Keyring | `ThreadedStore::call` ждёт `recv()` без timeout → вечный spinner | `auth.rs` | G1 | **closed (D-96); Drop join P2** |
| PLAT-01 | **P1** | Auth | mutex держится через keyring I/O и `/users/@me` | `auth.rs` | G1 | **closed (D-100)** |
| SEC-02 | **P1** | Auth | keyring error маскируется в `Network`, причина теряется | `main.rs:150` | G1 | open |
| PLAT-02 | **P1** | Tests | live keyring-кейсы `return` без skip → gate видит passed | `keyring_contract.rs` | G4 | **implemented; gate pending (D-119)** |
| PLAT-10 | **P1** | UI/Voice | mute/deafen только aria, нет voice-state | `actions.js` | G5+G2 | open |
| PLAT-11 | **P1** | IPC | нет инварианта «IPC не принимает путь» | attachments/capture | G4b | open |

| PLAT-11 | **P1** | Tests | нет meta-инварианта «IPC не принимает путь» | `PLAT-11-AUDIT.md` | G4 | open |
| PLAT-11a | P2 | Messages | `send_message` пропускает `..`/`?`/`\`/NUL в channelId → URL-path forgery | `messages.rs:147` | G1 | open |
| PLAT-11b | P2 | Fixtures | symlink внутри fixtures читает за root | fixtures | G1+G4 | open |

| L-01 | P1 | Live UI | нет видимого reconnect/offline | G6 matrix | G1+G2 | **closed (D-106)** |
| SEC-02 | P1 | Auth | keyring error маскируется в Network | `main.rs` | G1 | **closed (D-107)** |

| G6-M06 | **P1** | Messages/UI | реакции не рендерятся в реальном канале, `reactions.js` на bench "news" | `renderChannelMessage` → `channelReactionKey(channelId,item.id)` + pills; тест `chat-reactions.test.mjs` (9) | G2 | **closed (D-131)**; остаток live/`MESSAGE_REACTION_*` → L-02 |
| G6-P03UI | **P1** | Typing/UI | backend typing есть, UI/индикатор/троттлинг отсутствуют | `typing_contract.rs` (13) есть; `send_typing` в `ui/**` = 0 вхождений (grep 2026-10-24) | G2 | **in work** (P-03 UI) |

## G6 parity findings (2026-10-24, `G6-SYNC-01.md`)

Источник — `PARITY/G6-REPORT-RW40-02.md` §7.1. Severity сохранена как в источнике;
`G6-D-05` — P3 (дефект документации), не повышалась.

| ID | P | Область | Симптом / repro | Evidence | Owner | Статус | Тест |
|---|---|---|---|---|---|---|---|
| G6-D-01 | **P1** | Messages/live | UI слал `channelId:"news"` (не snowflake) → live 400/404; P0-02 нельзя было принять | было `chat.js:432,495,509`; сейчас реальный `channelId`/`item.id`, `BENCH_CHANNEL_ID` только в bench-ветке (`chat.js:786,1282,1321`) | G1+G2 | **closed (D-117 backend / D-120 UI)** | `channel-switch.test.mjs` (10) + `channels_contract.rs` |
| G6-D-02 | **P1** | Messages/live | `messageId = String(index)` (номер строки) → live edit/delete 404; приёмка M-03/M-04 невозможна | было `chat.js:495,509`; сейчас `channelItemAt(index).id` + `data-message-id=item.id`; bench-ветка оставлена осознанно | G2 | **closed (D-120)** | `channel-switch.test.mjs` (10), оракул `MESSAGE_*` |
| G6-D-03 | P2 | UI/BFF wiring | `ready()`, `members_page`, `search_page`, `settings`, `enter_guild` не вызываются из UI → `implemented` в матрице относится к BFF, не к экрану | grep по `rust/gui-spike/ui/**` (2026-10-24): `ready`/`members_page`/`search_page`/`settings`/`enter_guild` = 0 `invoke`; используются `channels`, `messages_page`, `read_state`, `ack_message`, `join_guild`, `leave_guild` | G2 | open | `dispatch_ui.test.mjs` (new, L-02) |
| G6-D-04 | P2 | Docs/planning | В матрице ST-01 «нет 45 подпунктов/панелей» — ложный размер задачи (XL вместо S), искажал WSJF | исправлено: 63 DOM-строки = профиль 2 + 18 разделов/«Выйти»×3 + 4 подпункта; WSJF 4.0 → 6.0 | G6 | **closed (G6-SYNC-01 §1)** | `settings-nav-63.tsv` (new; оракул из `ui8/23-settings-categories.json`) |
| G6-D-05 | P3 | Docs/evidence | В матрице G-01 был только `[M]`, хотя источник списка — `[L]` READY.guilds[] → занижало готовность | исправлено: Ref = `[L] READY.guilds[]` + `[M]` за визуал; оракул `002-READY.json` (7 гильдий) | G6 | **closed (G6-SYNC-01 §1)** | `guild-rail.test.mjs` (8) |
