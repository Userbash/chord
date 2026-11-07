# RW-40 / QA-WAVE9-AUDIT — независимый аудит TEST-POLICY по 32 принятым срезам и 5 дизайн-спекам

- **Роль:** QA-инженер, отдел G4 QA.
- **Карточка:** `QA-WAVE9-AUDIT` (read-only аудит соблюдения `TEST-POLICY.md`).
- **Дата и окно:** 2026-10-30, 19:05–19:22 +05:00. Снимок md5 и счётчиков — `2026-10-30T19:13:05+05:00`.
- **HEAD:** `4e24048` («RW-40: integrate Discord GUI spike, dispatch, paging and QA evidence»).
- **Режим:** продуктовый код, тесты и golden-каталоги только чтение. `cargo` не запускался,
  `git commit/push` не выполнялся. Создан один файл — этот отчёт.
- **Что запускалось:** `node --test tests/meta/plan-sync.test.mjs tests/meta/ipc-path.test.mjs`
  (требование ТЗ) и дополнительно `node --test tests/meta/test-policy.test.mjs tests/meta/e2e-required.test.mjs`.

> **Оговорка о реестре.** Единого файла с нумерацией 32 срезов в дереве нет. Номера `#31` и `#32`
> взяты дословно из `PM/b2b-attach-ui/ACCEPT-BE-PUT.md` и `PM/stub16-profile-ui/ACCEPT.md`.
> Остальные 30 восстановлены по `MERGE-READINESS-12` (26 срезов на 18:24) и хронологии приёмки:
> `B2b-ATTACH-UI` (PASS-ревью 18:07, уже числился ACCEPTED в `ORCA-WORKERS.md`), `INT-01` (18:27),
> `INT-03` (18:45), `V-07-UI` (18:48). Совпадение с `#31`/`#32` подтверждает раскладку.

> **Дерево двигалось во время аудита.** В 19:13:37 создан
> `tests/unit/attachments-render.test.mjs` (49 КБ, red-фаза карточки `AT-03`, каталог
> `PM/at03-render-ui/` появился в 19:11). Продуктового `attachments-render.js` ещё нет.
> Первый прогон обязательных meta-тестов в 19:07 был зелёным (17/17); повторный в 19:21
> красный, потому что новый тест не описан в `TDD-PLAN.md` §8. Подробности — P1-4.

## 1. Реестр аудита

### 1.1. 32 кодовых среза

| # | Срез | Приёмка | Продукт / тест |
|---:|---|---|---|
| 1 | `G-01b-CONTRACT` | `PM/g01b-contract/ACCEPT.md` | `settings_store.rs`, `settings_contract.rs` |
| 2 | `QA-M2-04` | `QUALITY/QA-M2-04-ACCEPT.md` | `.github/workflows/ci.yml`, `tests/meta/ci-browser.test.mjs` |
| 3 | `L-02` (интеграция) | `PM/l02/ACCEPT.md` | `net.rs`, `net_contract.rs` |
| 4 | `L-02-CORE` | `PM/l02-core/ACCEPT.md` | `discord-core/src/ws.rs`, `gateway_supervisor.rs` |
| 5 | `L-02-UI` + `TSC-FIX` | `PM/l02-ui/ACCEPT.md` | `dispatch-model.js`, `app.js`, `dispatch_ui.test.mjs` |
| 6 | `C-02-BE` | `PM/c02-be/ACCEPT.md` | `data.rs`, `channels_contract.rs` |
| 7 | `C-02-UI-OLDER` | `PM/c02-ui-older/ACCEPT.md` | `chat.js`, `channel-paging.test.mjs` |
| 8 | `PLAT-11a` | `PM/plat-11a/ACCEPT.md` | `messages.rs`, `messages_contract.rs` |
| 9 | `M-07-BE3` | `PM/m07-be3/ACCEPT.md` | `data.rs`, `read_state_contract.rs` |
| 10 | `SEC-L02-01-FIX` | `PM/sec-l02-01/ACCEPT.md` | `net.rs`, `net_contract.rs` |
| 11 | `B7b-SEARCH-FILTERS-BACKEND` | `PM/b7b-search/ACCEPT.md` | `search.rs`, `search_contract.rs` |
| 12 | `M-07-UI-BADGE` | `PM/m07-ui-badge/ACCEPT.md` | `chat.js`, `read-state-ui.test.mjs` |
| 13 | `STUB-27-CORE-ORACLE-FIX` | `PM/stub27-core/ACCEPT.md` | `stream.rs`, `stream_gateway_contract.rs` |
| 14 | `V-01-RTC-FIX` | `PM/v01-rtc/ACCEPT.md` | `session.rs`, `session_contract.rs` |
| 15 | `STUB-05-INVITES-BACKEND` | `PM/stub05-invites-be/ACCEPT.md` | `invites.rs`, `invites_contract.rs` |
| 16 | `STUB-14-UI-SEARCH-FIX` | `PM/stub14-search-ui/ACCEPT.md` | `search-overlay.js`, `search.js`, `search-remote-real-index.test.mjs` |
| 17 | `P-03-UI-TYPING-INDICATOR` | `PM/p03-typing-ui/ACCEPT.md` | `typing-indicator.js`, `gui-spike-dom.test.mjs` |
| 18 | `M-10-M-11-PINS-THREADS-BACKEND` | `PM/m10-pins-be/ACCEPT.md` | `pins_threads.rs`, `pins_threads_contract.rs` |
| 19 | `STUB-27-CORE-DISPATCH-INTEGRATION` | `PM/stub27-dispatch/ACCEPT.md` | `dispatch.rs`, `dispatch_types.rs` |
| 20 | `STUB-05-INVITES-MODAL-UI` | `PM/stub05-invites-ui/ACCEPT.md` | `invite-modal.js`, `join-server.test.mjs` |
| 21 | `M-08-MARKDOWN-MENTIONS-UI` | `PM/m08-markdown/ACCEPT.md` | `markdown.js`, `gui-spike-model.test.mjs` |
| 22 | `V-07-VOICE-DEVICES-CONTRACT` | `PM/v07-devices/ACCEPT.md` | `devices.rs`, `devices_contract.rs` |
| 23 | `INT-02-UI-CHAT-MARKDOWN-TYPING` | `PM/int02-chat-md/ACCEPT.md` | `chat.js`, `composer.css`, `gui-spike-model.test.mjs` |
| 24 | `B2a-ATTACH-TIER-FIX-PASS2` | `PM/b2a-attach/ACCEPT.md` | `attachments.rs`, `attachments_contract.rs` |
| 25 | `STUB-16-MEMBER-PROFILE-POPOVER-FIX` | `PM/stub16-profile-ui/ACCEPT.md` (перезаписан Pass 3) | `members.js`, `members.css`, `gui-spike-regressions.test.mjs` |
| 26 | `STUB-10-PINS-THREADS-PANELS-UI` | `PM/stub10-pins-threads/ACCEPT.md` | `pins-threads.js`, `server-menu.css`, `channel-paging.test.mjs` |
| 27 | `B2b-ATTACH-UI-20MIB-SYNC` | **только** `PM/b2b-attach-ui/REVIEW-PASS4.md` | `composer.js`, `composer.test.mjs` |
| 28 | `INT-01-RUST-WIRING` | `PM/int01-rust-wiring/ACCEPT.md` | `lib.rs`, `main.rs`, `ipc_wiring_contract.rs` |
| 29 | `INT-03-SAVE-QUEUE-FIX` / `G-01b-UI` | `PM/g01b-ui/ACCEPT.md` | `app.js`, `guild-rail.test.mjs` |
| 30 | `V-07-UI-VOICE-DEVICES` | `PM/v07-ui-devices/ACCEPT.md` | `voice-devices-settings.js`, `voice-devices-settings.test.mjs` |
| 31 | `INT-04-ATTACH-PUT-BACKEND` Pass 2 | `PM/b2b-attach-ui/ACCEPT-BE-PUT.md` | `attachments.rs`, `lib.rs`, `main.rs`, `attachments_contract.rs` |
| 32 | `STUB-16 / MB-03` Pass 3 | `PM/stub16-profile-ui/ACCEPT.md` | `members.js`, `gui-spike-regressions.test.mjs` |

### 1.2. 5 принятых дизайн-спек

| # | Спека | Запись приёмки | MD5 сейчас |
|---:|---|---|---|
| 1 | `DESIGN/STREAM.md` | `STREAM-REVIEW-PASS3.md` — PASS F-02..F-04 | `580c45c65118c741f8c752ae8e525e56` |
| 2 | `DESIGN/TYPING-MARKDOWN-SPEC.md` | `TYPING-MARKDOWN-SPEC-REVIEW-PASS4.md` — PASS | `38651e2b4d740cc434b805d941703d5f` |
| 3 | `DESIGN/INVITES-SPEC.md` | `INVITES-SPEC-REVIEW-PASS5.md` + `INVITES-SPEC-ACCEPT.md` | `0007ecc4f6105c31b873d87f647dddde` |
| 4 | `DESIGN/ATTACHMENTS-RENDER-SPEC.md` | `ATTACHMENTS-RENDER-SPEC-REVIEW.md` + `ATTACHMENTS-RENDER-SPEC-ACCEPT.md` | `9e93ec4a3de64d96c7e7b7c296fc06fa` (совпал с ACCEPT) |
| 5 | `DESIGN/VOICE-DEVICES-SETTINGS-SPEC.md` | `VOICE-DEVICES-SETTINGS-SPEC-REVIEW-PASS2.md` + `VOICE-DEVICES-SETTINGS-SPEC-ACCEPT.md` | `ed91fbd3856258a7dda1288fd2de4cce` |

## 2. Запреты TEST-POLICY §2 — маркеры

Область grep: `rust/gui-spike/ui/**` и `tests/**` (unit, meta, smoke, e2e, helpers, fixtures).

| Маркер | Найдено | Комментарий |
|---|---|---|
| `.only(` | **0** | единственное совпадение — регэксп-запрет в `tests/meta/e2e-required.test.mjs:72` |
| `.todo(` / `it.todo` | **0** | единственное совпадение — регэксп-запрет в `tests/meta/e2e-required.test.mjs:73` |
| `assert.ok(true)`, пустые assert | **0** | единственное совпадение — регэксп-запрет в `tests/meta/test-policy.test.mjs:79` |
| `@ts-ignore`, `@ts-expect-error`, `@ts-nocheck` | **0** | в `rust/gui-spike/ui`, `tests` и во всём `rust/gui-spike` |
| `eslint-disable` в UI/тестах | **0** | — |
| `#[ignore]` в `rust/gui-spike/tests` | **0** | `#[ignore]` есть только в `rust/crates/discord-platform` (live keyring), вне области |
| `skip` в `rust/gui-spike/ui` | **0** | `skipLibCheck` в `tsconfig.json` — не пропуск теста |
| `.skip` / `{ skip: … }` в `tests/**` | **17 сайтов** | у каждого причина и условие среды, «тихих» нет |
| Удаление/переименование тестовых файлов | **0** | `git status --porcelain` не содержит `D`/`R` |
| Правка `tsconfig` ради «зелени» | **0** | `tsconfig*.json` вне диффа |
| Запись в golden (`logs/`, `functions/`, `evidence/`) | **0** | `git status` по каталогам пуст |
| `--no-verify` | **0** | только регэксп-запрет в `tests/meta/test-policy.test.mjs:94` |

Разбивка 17 `skip`: 3 в `gui-spike-bench` (недоступный `/proc/<pid>`), 2 `describe.skip`
по `process.platform !== "linux"`, 2 в `live-discord.e2e` (нет `DISCORD_LAB_LIVE_CDP`),
2 smoke (`DISPLAY`/`WAYLAND_DISPLAY`), 2 e2e (нет `CHROME_BIN`), 3 `cxxfilt` (нет бинаря),
по одному `summarize` (нет `logs/calls-voice-28170.jsonl`), `pids` (нет Frida),
`fixtures-tracked` (нет `git`). У всех — текст причины в самом вызове.

## 3. Полная таблица: ACCEPT / RED / GREEN / MUTATION / MD5

`RED` — mtime и observed failure; `GREEN` — итог основного зелёного прогона; `TARGETED` — отдельный
смежный прогон; `MUT` — число убитых мутантов; `MD5` — сверка пинов ACCEPT с текущим деревом.

| # | Срез | ACCEPT.md | RED | GREEN | TARGETED | MUT | MD5 | Итог |
|---:|---|---|---|---|---|---|---|---|
| 1 | `G-01b-CONTRACT` | ✔ `PM/g01b-contract` | 16:11:32, compile 11 ошибок (`E0560`, `E0609`) | 23/23, 16:12:21 | `clippy-targeted.log` | **нет** | ✔ 2/2 | **FAIL** (P1-2) |
| 2 | `QA-M2-04` | ✔ `QUALITY/QA-M2-04-ACCEPT.md` | 15:52:07, 3/10 fail | 10/10, 15:52:26 | `QA-M2-04-meta.log` | n/a (CI/meta) | ✔ `md5.txt` | PASS |
| 3 | `L-02` | ✔ | 15:00:07, `E0432` | 21 + 9 targeted, 15:02:23 | `targeted*.log` | **нет** (условие §3.3) | ✔ 4/4 | **FAIL** (P1-3) |
| 4 | `L-02-CORE` | ✔ | 15:16:54, `E0599` | 12 + 576 core + 269 gui | `verify-*.log` | **1 мутант, до финального `ws.rs`** | 2/3 | **FAIL** (P1-2) |
| 5 | `L-02-UI` | ✔ | 15:31:28, fail | 43/43, 15:37:14 | `targeted-ui.log` | **5/5** (`MUTATION.md`) | 2/3 (см. §5) | PASS* |
| 6 | `C-02-BE` | ✔ | 13:40:18, 18 ошибок | 6 channels + 30 data | — | **нет** | ACCEPT сам фиксирует устаревший `md5.txt` | **FAIL** (P1-2) |
| 7 | `C-02-UI-OLDER` | ✔ | 16:12:11, fail | 24/24 → fix 26/26 | `adjacent.log` 109/109 | **4 мутанта** (`mutant-a…d`) | 2/2, оба пина заменены | **FAIL** (P1-2) |
| 8 | `PLAT-11a` | ✔ | red-proof 12/2, 16:12:34 | 15/15, 16:12:38 | `suite.log` | **5/5** | ✔ 2/2 | PASS* |
| 9 | `M-07-BE3` | ✔ | 15:52:38, 15+8 ошибок | 15/15, 15:53:53 | `green-targeted-final.log` | **нет** | ✔ 2/2 | **FAIL** (P1-2) |
| 10 | `SEC-L02-01` | ✔ | 16:39:41, fail | 24 passed, 16:46:04 | — | **2/2** | ✔ 2/2 | PASS* |
| 11 | `B7b-SEARCH` | ✔ | 16:44:10, fail | 16 passed (общий лог с #10) | — | **6/6** | ✔ 2/2 | PASS* |
| 12 | `M-07-UI-BADGE` | ✔ | 16:24:43, fail | 29/29, 16:25:53 | — | **нет** | 1/2 (`chat.js` → #23) | **FAIL** (P1-2) |
| 13 | `STUB-27-CORE` | ✔ | 16:41:15, fail | 11/11, 16:45:34 | `green-fix.log` | **5/5** | — | PASS |
| 14 | `V-01-RTC` | ✔ | 16:41:28; fix 17:10:22 | 147/147, 17:12:09 | `green-fix.log` | **7/7** | — | PASS |
| 15 | `STUB-05-BE` | ✔ | 17:14:49, fail | 20/20, 17:17:20 | — | **5/5** | — | PASS |
| 16 | `STUB-14-UI` | ✔ | 16:26:39; fix 16:54:54 | 108/108, 16:55:17 | `green-fix.log` | **5/5** | — | PASS |
| 17 | `P-03-UI` | ✔ | 17:26:32, fail | 38/38, 17:28:17 | `targeted.log` | **5/5** | ✔ 2/2 | PASS |
| 18 | `M-10-PINS-BE` | ✔ | 17:28:37, fail | 6/6, 17:32:55 | — | **5/5** | ✔ 2/2 | PASS |
| 19 | `STUB-27-DISPATCH` | ✔ | 16:59:27, fail | 583/583, 17:00:52 | — | **5/5** | ✔ 2/2 | PASS |
| 20 | `STUB-05-UI` | ✔ | 17:28:17, fail | 35/35, 17:32:09 | — | **5/5** | ✔ 2/2 | PASS |
| 21 | `M-08-MARKDOWN` | ✔ | 17:26:29, fail | 48/48, 17:27:35 | `targeted.log` 165/165 | **5/5** | 1/2 (тест → #23) | PASS* |
| 22 | `V-07-DEVICES` | ✔ | 17:27:30, fail | 5/5, 17:29:09 | `clippy.log` | **5/5** | ✔ 2/2 | PASS |
| 23 | `INT-02` | ✔ | 18:02:15, 48/53 | 53/53, 18:04:38 | 272/272 | **5/5** | ✔ 3/3 | PASS |
| 24 | `B2a` | ✔ | 17:48:45, 14/3 | 17/17, 17:50:17 | `green-pass3.log` | **4/4** | пины → #31 | PASS* |
| 25 | `STUB-16-FIX` | ACCEPT перезаписан #32 | 17:50:36, 13/3 | 13/13, 17:52:25 | — | **5/5** (`mutations-fix.log`) | — | CONDITIONAL |
| 26 | `STUB-10` | ✔ | 17:13:42, 19/14/5 | 19/19, 17:14:39 | — | **5/5** | ✔ 3/3 | PASS |
| 27 | `B2b-UI` | **нет ACCEPT**, только `REVIEW-PASS4` | 18:01:40, 26/3 | 29/29, 18:03:10 | — | **5/5** | ✔ 2/2 | **FAIL** (P1-1) |
| 28 | `INT-01` | ✔ | 18:04:38, `E0432`, exit 101 | 5/5, 18:08:46 | `clippy.log` exit 0 | **5/5**, baseline до #31 | — | PASS* |
| 29 | `INT-03` | ✔ | 18:17:12, 15/16 | 16/16, 18:17:31 | 98/98, `tsc` 0 | **5/5** | ✔ 1/1 + `app.js` | PASS |
| 30 | `V-07-UI` | ✔ | 18:43:17, 16 `AssertionError` | 17/17, 18:43:22 | 78/78, `tsc` 0 | **5/5** | ✔ 2/2 | PASS |
| 31 | `INT-04` | ✔ `ACCEPT-BE-PUT.md` | 18:54:38, `E0432`, exit 101 | 8/8 + 10/10 + 23/23 + 5/5 + 12/12 meta | `clippy`, `rustfmt` exit 0 | **5/5** | ✔ 4/4 | PASS |
| 32 | `STUB-16-P3` | ✔ | 18:50:34, `EXIT_UNIT=1`, `EXIT_SMOKE=1` | smoke 1/1 + unit 166/166 | `ui-tsc` exit 0 | **5/5** | ✔ 2/2 | PASS |

`PASS*` — доказательства полные, но есть замечание P2/P3 (устаревший пин, общий лог, 4 мутанта вместо 5).
`CONDITIONAL` — собственного ACCEPT у среза больше нет: файл перезаписан приёмкой Pass 3 (#32).

**Сводка по таблице:** 16 PASS, 7 PASS*, 1 CONDITIONAL, 8 FAIL.

## 4. Мутации: где доказательств нет

| # | Срез | Что есть | Чего не хватает |
|---:|---|---|---|
| 1 | `G-01b-CONTRACT` | — | Лога мутаций нет, в ACCEPT/REVIEW мутации не заявлены. Новый инвариант `lastGuildId` не проверен мутантом |
| 3 | `L-02` | — | `ACCEPT.md` §3.3 прямо требует QA-мутацию «обязательна до merge»: убрать вырезание `session_id` → тест обязан упасть. Условие не выполнено |
| 4 | `L-02-CORE` | `mutation-deferred-flush.log` — 1 мутант, `exit=101` | Один мутант на промежуточной версии `ws.rs` (`restore md5=176791cc…`), принятый файл — `8703c1d6…`. Финальная версия мутантом не покрыта |
| 6 | `C-02-BE` | — | `REPORT.md:141` перекладывает мутации на Team Lead; лога нет |
| 7 | `C-02-UI-OLDER` | 4 лога `mutant-a…d.log` | Сводки и пятого мутанта нет; порог «≥5» не выполнен |
| 9 | `M-07-BE3` | — | Лога мутаций нет, в ACCEPT не заявлены; 9-полевая проекция `read_state` — новый инвариант |
| 12 | `M-07-UI-BADGE` | — | Лога мутаций нет; поведение `badgeCount`/`flags` в UI не проверено мутантом |
| 2 | `QA-M2-04` | red/green/meta | Мутация не применима: срез меняет CI-workflow и meta-тест, продуктовой ветки нет |
| 24 | `B2a` | `mutations-pass3.log` — 4/4 KILLED | Ниже порога 5; замечание P2-1 из `QA-AUDIT-WAVE8` не закрыто |

`Survivors`/`MISSED` в логах, где мутации есть, не найдены. Во всех проверенных логах после
каждого мутанта восстановлен baseline MD5 (`match=True` / `OK` / совпадение блока restore).

## 5. Сверка MD5: устаревшие пины и их замена

| # | Файл | Пин в ACCEPT | Сейчас | Кем заменён |
|---:|---|---|---|---|
| 4 | `rust/gui-spike/tests/net_contract.rs` | `fe8aa92146e9dcf8b943335dfad652c8` | `573d271204f3c1db601e2df73684f129` | #10 `SEC-L02-01` |
| 5 | `rust/gui-spike/ui/app.js` | `1ac5c36976a064120529c132e61cfb54` | `8f5d84c14dc6e9aea749a202ecb3a96b` | #29 `INT-03` |
| 7 | `rust/gui-spike/ui/features/chat.js` | `6444e9d94a6daa9e1d7b036525295398` | `d2e3bb0958fbafb44dc14fe72cee0426` | #12/#23 |
| 7 | `tests/unit/channel-paging.test.mjs` | `4ef50982a5de2847765bd1bc64d8dedf` | `e6974ee4a62ea5eef57ac128239e5dce` | #26 `STUB-10` |
| 12 | `rust/gui-spike/ui/features/chat.js` | `1e73c60219792da3c332b8cbc73c7607` | `d2e3bb0958fbafb44dc14fe72cee0426` | #23 `INT-02` |
| 21 | `tests/unit/gui-spike-model.test.mjs` | `e975a4d45da485beebcefb0319db950f` | `0eee4284af263c3d412b0afe4910bd58` | #23 `INT-02` |
| 24 | `rust/gui-spike/src/attachments.rs` | `3377d6dcd0850da6b92dbaac3a6c7dac` | `75f50c624ab5a6b21a67281235f4b289` | #31 `INT-04` |
| 24 | `rust/gui-spike/tests/attachments_contract.rs` | `45d107db41d4c1f443de8596b1196e49` | `77157aadf4267ded8c225d8891c71a9f` | #31 `INT-04` |
| 28 | `rust/gui-spike/src/lib.rs` (в REPORT и mutation baseline) | `b2af030ca24749a6e4d278b5f1106a1f` | `bc38ac5943b929e7a620ef911cb208a5` | #31 `INT-04` |
| 28 | `rust/gui-spike/src/main.rs` (в REPORT и mutation baseline) | `c192e36f7c3bc68637169f45cb53b203` | `8a71f311bf0ab47ff24450eaf389b2d4` | #31 `INT-04` |
| 4 | `rust/crates/discord-core/src/ws.rs` (mutation baseline) | `176791cc4e9a85173e8642cab02baa9b` | `8703c1d695cd0af26458393eb3c1e3f9` | ACCEPT `L-02-CORE`; мутация устарела |

Совпадения на текущем дереве (полный прогон `md5sum`):

- #29 `guild-rail.test.mjs` `76a0acca…`, #30 `voice-devices-settings.js` `5fda00c8…`,
  тест `bf7544e0…`, #31 все 4 файла (`75f50c62…`, `bc38ac59…`, `8a71f311…`, `77157aad…`),
  #32 `members.js` `55356f6e…`, `gui-spike-regressions.test.mjs` `2455ee35…`,
  #27 `composer.js` `223e7641…`, `composer.test.mjs` `fd421139…`, #23 все три файла.
- #1, #8, #9, #10, #11, #17, #18, #19, #20, #22, #25, #26 — пины совпали без оговорок.

10 из 11 расхождений объясняются более поздними принятыми срезами; `ws.rs` — финальной версией
того же среза `L-02-CORE`. Но supersession записан
только в `m07-ui-badge/ACCEPT.md` («supersedes `chat.js` md5 pin from `c02-ui-older`»),
в `stub10/ACCEPT.md` (пин `channel-paging`), `int02/ACCEPT.md` (пин `chat.js` и `gui-spike-model`)
и `INT-04/ACCEPT-BE-PUT.md` (пин `attachments`/`lib`/`main`). Для `L-02-CORE`, `L-02-UI`, `M-08`
и `INT-01` явной записи «файл заменён следующим срезом» нет — см. P2-2.

## 6. Дизайн-спеки: приёмка и пробелы

| # | Спека | Ревью | ACCEPT.md | Итог |
|---:|---|---|---|---|
| 1 | `STREAM.md` | PASS 3: F-02..F-04 закрыты; F-01 не переоценивался | **нет** | PASS по ревью, формального ACCEPT нет (P2-1) |
| 2 | `TYPING-MARKDOWN-SPEC.md` | PASS 4: токены согласованы с `tokens.css` | **нет** | PASS по ревью, формального ACCEPT нет (P2-1) |
| 3 | `INVITES-SPEC.md` | PASS 5: P0=P1=P2=0 | ✔ | PASS |
| 4 | `ATTACHMENTS-RENDER-SPEC.md` | PASS: P0=P1=P2=0 | ✔, md5 `9e93ec4a…` совпал | PASS |
| 5 | `VOICE-DEVICES-SETTINGS-SPEC.md` | PASS 2: P0=P1=P2=0 | ✔ | PASS |

Red/green/mutation к дизайн-спекам неприменимы: это документы, их проверка — цепочка ревью
и сверка токенов/RE-оракулов. Ни у одной спеки нет отдельного «mutation»-артефакта, и это
не нарушение: `TEST-POLICY.md` §3.6 говорит о новых инвариантах в коде.

## 7. Независимое воспроизведение

| Команда | Код | Результат |
|---|---|---|
| `node --test tests/meta/plan-sync.test.mjs tests/meta/ipc-path.test.mjs` (19:07, до появления `attachments-render.test.mjs`) | **0** | `tests 17 / pass 17 / fail 0 / skipped 0`, 4 suite, 62 мс |
| та же команда (19:21, после появления файла) | **1** | `tests 17 / pass 16 / fail 1`; падает `plan-sync`: «каждый тестовый файл … описан в §8 ровно один раз», `actual: ['tests/unit/attachments-render.test.mjs']` |
| `node --test tests/meta/test-policy.test.mjs tests/meta/e2e-required.test.mjs` | **0** | `tests 32 / pass 32 / fail 0 / skipped 0` |
| `grep -rn "\.only(" rust/gui-spike/ui tests` | 1 совпадение | только регэксп-запрет в meta-тесте, реальных вызовов 0 |
| `grep -rn "@ts-ignore\|@ts-expect-error\|@ts-nocheck" rust/gui-spike/ui tests` | 0 совпадений | — |
| `grep -rn "assert\.ok(true)" tests rust/gui-spike/ui` | 1 совпадение | только регэксп-запрет в `tests/meta/test-policy.test.mjs:79` |
| `git status --porcelain -uall` | **0** | 49 `M`, 362 `??`; удалённых/переименованных тестов и записей в golden нет |

`plan-sync` подтверждает, что §8 `TDD-PLAN.md` сходится с тестовыми файлами и модулями
(5 проверок), `ipc-path` — что ни одна IPC-команда не принимает путь и файловые плагины
не подключены (12 проверок, включая мутацию `main.rs`). На 19:07 оба теста были зелёными;
на 19:21 `ipc-path` (12/12) зелёный, `plan-sync` красный из-за нового файла `AT-03`.

Rust-тесты срезов не перезапускались: `cargo` запрещён ТЗ. Их результаты приняты по логам
воркеров с указанной выше сверкой mtime и MD5.

## 8. Находки

### P1 — блокируют утверждение «все 32 среза приняты с полным evidence»

**P1-1. У среза #27 `B2b-ATTACH-UI-20MIB-SYNC` нет `ACCEPT.md`.**
Есть только `PM/b2b-attach-ui/REVIEW-PASS4.md` (PASS в объёме карточки). Сам файл ревью
фиксирует: «Полный `gate:worker` для pass 4 не представлен», runtime-передача `File`/байтов
не проверялась. `ORCA-WORKERS.md` называет карточку «ACCEPTED (PASS-ревью)», но формальной
записи приёмки нет. Требование ТЗ «у всех 32 есть `ACCEPT.md`» не выполнено: 31/32.

**P1-2. У 7 кодовых срезов нет полного мутационного evidence.**
#1, #3, #6, #7, #9, #12 — мутаций нет вообще; #4 — один мутант на промежуточной версии файла.
`TEST-POLICY.md` §3.6 требует мутанта для нового инварианта, §9 — «новые инварианты покрыты;
мутации caught». Срезы приняты без этого. Исключение — #2 (`QA-M2-04`): там мутация
неприменима, срез меняет CI-workflow и meta-тест.

**P1-3. `L-02` принят с невыполненным условием.**
`PM/l02/ACCEPT.md` §3.3: «QA-мутация (обязательна до merge): убрать вырезание `session_id` →
контрактный тест обязан упасть». Лога такой мутации нет. Приёмка формально «ACCEPTED
(с условиями)», но условие, названное обязательным до merge, не закрыто.

**P1-4. `plan-sync` красный: новый тест `AT-03` не описан в `TDD-PLAN.md` §8.**
В 19:13:37 появился `tests/unit/attachments-render.test.mjs` (red-фаза `AT-03`), продуктового
`rust/gui-spike/ui/features/attachments-render.js` ещё нет. Повторный прогон обязательной
команды даёт exit 1 и 16/17: `каждый тестовый файл (unit/meta/smoke/e2e) описан в §8 ровно
один раз`, `actual: ['tests/unit/attachments-render.test.mjs']`. Это не дефект одного из 32
срезов, а in-flight работа: `TDD-PLAN.md` §8 правит Team Lead, воркер `AT-03` его не трогает.
До добавления строки §8 гейт `gate:worker` будет красным.

### P2 — не блокируют срез по отдельности, но искажают картину приёмки

**P2-1. У 2 из 5 дизайн-спек нет `ACCEPT.md`.**
`STREAM.md` и `TYPING-MARKDOWN-SPEC.md` приняты только PASS-ревью (`STREAM-REVIEW-PASS3`,
`TYPING-MARKDOWN-SPEC-REVIEW-PASS4`). Для `STREAM` PASS 3 к тому же не переоценивает F-01:
вердикт ограничен F-02..F-04.

**P2-2. Часть устаревших MD5-пинов не имеет явной записи о замене.**
Цепочка замен в §5 объясняется последующими срезами, но в `L-02-CORE`, `L-02-UI`, `M-08`
и `INT-01` нет строки «пин заменён срезом #N». По §9 TEST-POLICY («md5 файлов совпадают»)
это делает evidence этих срезов историческим, а не текущим.

**P2-3. Один green-лог на два среза.**
`PM/sec-l02-01/green.log` и `PM/b7b-search/green.log` побайтово совпадают
(`8f076d771844f66805ac5041f3a60c28`). Внутри — два прогона: `net_contract` 24 passed и
`search_contract` 16 passed. Как комбинированный лог он валиден, но независимого green-артефакта
у #10 и #11 нет.

**P2-4. `PLAT-11a`: red-лог сконструирован позже мутаций.**
`red.log` — 16:12:34, `mutations.log` — 16:11:56, `green.log` — 16:12:38. `REPORT.md` §3
объясняет это red-proof против исходника `git show HEAD`, а не хронологическим RED.
Содержание корректно (`12 passed; 2 failed`, exit 101), но mtime-порядок red→green
для этого среза не доказывает red-first.

**P2-5. В дереве есть изменение теста от непринятой карточки.**
`tests/unit/gui-spike-features.test.mjs` переписывает проверки 429 (`leave-confirm`) со ссылкой
на «G-05-REST §3.9/D7 (согласовано PM 2026-10-30)». `G05-REST-MUTATIONS-FIX` по
`REVIEW-PASS4.md` — **RETURN** (P1: нет `gate:worker`). Изменение не покрыто ни одним
принятым срезом из 32.

**P2-6. ACCEPT `STUB-16` перезаписан, у среза #25 нет своей записи.**
`PM/stub16-profile-ui/ACCEPT.md` теперь описывает Pass 3 (#32). Приёмка Pass 1/2
сохранилась только в `REPORT-FIX.md` и `REVIEW-PASS2.md`; отдельного `ACCEPT.md` для #25 нет.

**P2-7. Форматы mutation-логов не унифицированы.**
Встречаются `verdict=KILLED`, `Status: KILLED`, `**CAUGHT**`, `exit=101`,
`M1_EXIT_CODE=101`, `ИТОГ: 5/5`. Автоматическая сверка «mutants == kills» без ручного
разбора невозможна; часть логов не содержит итоговой строки.

### P3 — наблюдения

- `B2a` по-прежнему 4/4 мутанта (порог аудита — 5); замечание P2-1 из `QA-AUDIT-WAVE8`
  не закрыто.
- У #6, #13, #14, #15, #16, #28 в ACCEPT нет MD5-секции; сверка возможна
  только по REPORT/REVIEW или `md5.txt`.
- 17 `skip` в `tests/**` корректны по причине, но gate на текущей среде всё равно даёт
  0 pass / 4 skip для e2e (нет браузера); это среда, а не ослабление тестов.

## 9. Вердикт

| Критерий | Статус | Комментарий |
|---|---|---|
| TEST-POLICY §2: `.only`, `.todo`, `@ts-ignore`, `eslint-disable`, `assert.ok(true)` | **PASS** | все 0; 17 skip с причиной; тесты не удалялись |
| 32/32 среза имеют `ACCEPT.md` | **FAIL** | 31/32; у #27 только `REVIEW-PASS4.md` |
| 32/32 среза имеют RED-лог | **PASS** | у всех; #8 — документированный red-proof, см. P2-4 |
| 32/32 среза имеют GREEN (+targeted) | **PASS** | у всех; у #10/#11 один общий лог, см. P2-3 |
| 32/32 среза имеют mutation-лог | **FAIL** | 25/32 полных (24 кодовых + #2 n/a); 7 неполных |
| 5/5 дизайн-спек имеют `ACCEPT.md` | **FAIL** | 3/5; `STREAM` и `TYPING-MARKDOWN` — только review PASS |
| MD5 в ACCEPT совпадают с деревом | **FAIL (условно)** | 11 пинов устарели; 10 объяснены поздними срезами, 1 — финальной версией того же среза, но записаны не везде |
| Тесты не ослаблены | **PASS** | 3 удалённых assert: 1 — смена типизации, 2 — заменены пятью более строгими; проверки не потеряны |
| `tsconfig`/golden не тронуты | **PASS** | диффа нет |
| `plan-sync` + `ipc-path` | **FAIL (на 19:21)** | `ipc-path` 12/12; `plan-sync` 4/5 из-за неописанного `tests/unit/attachments-render.test.mjs` (P1-4). В 19:07 было 17/17 |

**Итог: FAIL по буквальному требованию «все 32 среза и 5 спек имеют полный набор
red/green/mutation + ACCEPT».**

Блокирующее — P1-1 (#27 без ACCEPT и без gate), P1-2 (7 срезов без полного mutation evidence),
P1-3 (`L-02` с открытым обязательным условием), P1-4 (`plan-sync` красный из-за in-flight
red-фазы `AT-03`). Абсолютные запреты §2 не нарушены, ослабления тестов не найдены,
`ipc-path` и `test-policy`/`e2e-required` зелёные. Для перевода в PASS нужно:
добавить `ACCEPT.md` для #27 с `gate:worker`; закрыть мутации #1/#3/#4/#6/#7/#9/#12 или
письменно оформить waiver в `DECISIONS.md`/`WAIVERS.md`; выполнить условие §3.3 `L-02`;
добавить `ACCEPT.md` для `STREAM`/`TYPING-MARKDOWN` либо записать, что review PASS —
единственная форма их приёмки; добавить строку §8 для `attachments-render.test.mjs`.
