# RW-40 / QA-WAVE3-TEST-INTEGRITY-AUDIT — целостность тестов

- **Роль:** воркер G4 QA, аудитор целостности тестов.
- **Дата и снимок:** 2026-10-30, 16:45–17:00 +05:00 (снимок md5 — `2026-10-30T16:58:05+05:00`).
- **HEAD:** `4e24048` (RW-40: integrate Discord GUI spike, dispatch, paging and QA evidence).
- **Режим:** read-only по коду и тестам. `cargo` не запускался, `git commit/push` не выполнялся,
  продуктовые файлы и тесты не менялись. Созданы только файлы этого аудита в `waves/RW-40/QUALITY/`.
- **Артефакты:** `TEST-INTEGRITY-WAVE3.md` (этот отчёт), `TEST-INTEGRITY-WAVE3-meta.log`
  (прогон 16:57), `TEST-INTEGRITY-WAVE3-meta-r2.log` (повтор 17:00),
  `TEST-INTEGRITY-WAVE3-tsc.log`, `TEST-INTEGRITY-WAVE3-md5.txt`.

> **Рабочее дерево двигалось во время аудита.** Параллельно работали карточки
> `STUB-16-MEMBER-PROFILE-POPOVER-UI`, `G-01b-UI-SETTINGS-FIX`, `B2a-ATTACH-TIER-FIX`.
> Три тестовых файла изменились между началом и концом аудита (отмечено ниже).
> Все md5 — на момент снимка; после завершения этих фиксов снимок нужно повторить.

## 1. Область проверки

| Группа | Файлов | Проверок (`it`/`#[test]`) |
|---|---|---|
| `tests/unit/*.test.mjs` | 36 | — |
| `tests/meta/*.test.mjs` | 12 | — |
| `tests/e2e/*.e2e.test.mjs` | 10 | — |
| `tests/smoke/*.test.mjs` (сверх ТЗ, для полноты) | 8 | — |
| всего JS-тестов | 66 | 886 литеральных `it(...)`/`test(...)` |
| `rust/**/tests/*.rs` | 113 | 1358 объявлений `#[test]`/`#[tokio::test]` |

Дополнительно проверены: `git diff` всего рабочего дерева против HEAD (`21` удалённая строка
в тестах), коммит `4e24048` целиком (тестовые файлы в нём), `scripts/gate.mjs`, `package.json`,
10 каталогов `waves/RW-40/PM/*` из ТЗ.

## 2. Команды и результаты

| Команда | Exit | Результат | Артефакт |
|---|---|---|---|
| `node --test tests/meta/*.test.mjs` | 1 | 102 теста, 101 pass, 1 fail (`plan-sync`: токены §8) | `TEST-INTEGRITY-WAVE3-meta.log` |
| `node --test tests/meta/*.test.mjs` (повтор, 17:00) | 1 | те же 102/101/1, тот же единственный красный | `TEST-INTEGRITY-WAVE3-meta-r2.log` |
| `npx tsc -p tsconfig.json` (16:57) | 1 | 2 диагностики, обе в `tests/unit/gui-spike-regressions.test.mjs:457` (STUB-16 в red-фазе) | `TEST-INTEGRITY-WAVE3-tsc.log` |
| `npx tsc -p tsconfig.json` (17:00) | 0 | чисто, без вывода (STUB-16 дописал экспорты) | повтор вручную, см. F11 |
| `git diff --stat` / `git diff -U3` по тестам | 0 | 1449 строк диффа, из них 21 удалённая — все разобраны в §3 | этот отчёт |
| `grep -E '\.(only|skip|todo)\('`, `assert.ok(true)`, `--no-verify`, `@ts-ignore` | — | `.only`/`.todo` = 0; `assert.ok(true)` = 0; `--no-verify` = 0; `@ts-ignore` = 5 | §5 |
| `md5sum` по 66 JS и 113 Rust тестовым файлам | 0 | 179 контрольных сумм | `TEST-INTEGRITY-WAVE3-md5.txt` |

`cargo test`, e2e и live-прогоны не запускались: запрет ТЗ (cargo) и отсутствие окна/браузера.

## 3. Ревизия диффа тестов: удалённые строки

В рабочем дереве удалено ровно 21 строка в тестах — три файла. Ни одна удалённая проверка
не потеряна без замены.

| Файл | Удалено | Что | Замена / основание | Вердикт |
|---|---|---|---|---|
| `tests/unit/channel-paging.test.mjs` | 10 | `olderPage()` отдавала `[older, boundary]` (дубль граничной строки), два `assert.ok(...)` в хелперах | `assert.ok(older)` и `assert.ok(olderRow)` остались; `before` — exclusive (`C-02-BEFORE §1.2`), сценарий пересечения вынесен в новый тест «пересечение страниц: повтор граничной строки не даёт дубля»; сверху +2 новых теста геометрии prepend (14 `it` вместо 12) | ok, подтверждено `c02-ui-older/REVIEW-PASS2.md` (P2 закрыт) |
| `tests/unit/dispatch_ui.test.mjs` | 9 | переформатирование объекта `view` и 5 JSDoc-заголовков в `/** @param ... */` | ни одно утверждение не менялось: `it` 43 → 43, те же `assert.equal`/`assert.deepEqual` | ok, изменение чисто типовое (для `tsc`) |
| `tests/unit/gui-spike-features.test.mjs` | 2 | `disabled === null` после 429 и `callsOf("leave_guild").length === 2` как повтор | G-05-REST §3.9/D7: 429 блокирует контрол до конца отсчёта, авто-ретраев нет; вместо двух старых — пять новых assert (disabled во время отсчёта, `calls === 2` без автоповтора, разблокировка после отсчёта, ручной клик, `calls === 3`); `it` 98 → 98 | ok по существу, бумажное оформление — см. F4 |
| `rust/gui-spike/tests/{messages,net,search}_contract.rs` | 0 | 413 добавленных строк, удалений нет | — | ok |
| `rust/crates/discord-voice/tests/signaling.rs` | 0 | +2 строки | — | ok |

Тестовые файлы, где проверок стало больше (уменьшения нет ни в одном файле репозитория):

| Файл | `it` до | `it` после |
|---|---|---|
| `tests/unit/channel-paging.test.mjs` | 12 | 14 |
| `tests/unit/guild-rail.test.mjs` | 8 | 12 |
| `tests/unit/gui-spike-regressions.test.mjs` | 2 | 11 (в т.ч. STUB-16, в работе) |
| `tests/unit/read-state-ui.test.mjs` | 8 | 13 |
| `tests/unit/search-remote-real-index.test.mjs` | 2 | 8 |
| `tests/meta/rust-client-sync.test.mjs` | 10 | 14 |
| `tests/unit/cdp.test.mjs` | 23 | 36 |

Новые файлы (untracked, все аддитивные): `tests/unit/composer.test.mjs` (21),
`tests/unit/server-menu.test.mjs` (27), `rust/crates/discord-core/tests/stream_gateway_contract.rs` (11),
`rust/crates/discord-voice/tests/session_contract.rs` (16), `rust/gui-spike/tests/attachments_contract.rs` (17).

## 4. Коммит `4e24048`: изменения тестов и оракулов

| Место | Было → стало | Основание | Вердикт |
|---|---|---|---|
| `discord-platform/tests/keyring_contract.rs` | рантайм `skip("SKIP: …")` → `#[ignore = "live-keyring: …"]` + паника `KEYRING-LIVE: {not-enabled|service-unavailable|tool-missing}`; офлайн-инварианты вынесены в `keyring_offline_contract.rs` | PLAT-02: skip не должен выглядеть как pass | **усиление**, не ослабление |
| `discord-core/tests/rest_routes.rs` | `ROUTES.len() == 47` → `== 49`; вместо `captured.len() == ROUTES.len()` — `captured.len() == 47` + явные исключения двух новых маршрутов; добавлен `guild_search_route_is_catalogued` | D-02 + POST-контракт отправки; захват (47) не переписан, новые маршруты перечислены поимённо | допустимо (оракул-захват сохранён), см. F6 |
| `discord-core/tests/ws_transport.rs` | ожидание `op1` → `op40 {seq,qos}`; в стабе требуется `d.seq` (раньше допускался `None`); добавлен legacy-тест `ws_legacy_heartbeat_op1_fallback` | G-12 | **усиление** |
| `rest_client.rs`, `rest_user.rs`, `identify.rs`, `gateway_supervisor.rs`, `gui-spike/tests/*` | только добавления | — | ok |
| Удалённые/переименованные тестовые файлы | нет (`--diff-filter=DR` пусто) | — | ok |

Итого по коммиту: 2 удалённых `assert!` (оба в `rest_routes.rs`, оба заменены эквивалентными или
более строгими). Новые `#[ignore]` — 12 атрибутов по числу k01–k12, все с причиной
`live-keyring`; за пределами `keyring_contract.rs` их нет (в `keyring_offline_contract.rs` —
только упоминание в doc-комментарии). Мета-тест `keyring-live` стережёт соответствие
«12 `#[test]` ↔ 12 `#[ignore]`».

## 5. Абсолютные запреты TEST-POLICY §2 — проверка маркеров

| Запрет | Найдено | Комментарий |
|---|---|---|
| `.only` / `.todo` / `it.skip` | 0 | мета-тесты `test-policy` и `e2e-required` это же и стерегут |
| `assert.ok(true)`, `assert.equal(true,true)`, пустой `deepEqual` | 0 | — |
| Тест-заглушка без проверок | 0 | ни одного тестового файла с нулём `it`/`#[test]` (хелперы `tests/support`, `tests/helpers`, `rust/**/tests/support/*` не в счёт) |
| `console.log` вместо assert | 0 | `console.log` встречается только в комментарии-оракуле (`profile.test.mjs`) и в перехвате вывода (`cdp.test.mjs`) |
| `--no-verify`, `--no-gpg-sign`, `continueOnError`, `\|\| true` в гейте | 0 | `scripts/gate.mjs` фазы падают по exit code, `failOnSkips: true` у e2e/live |
| `@ts-ignore` без обоснования | **5** | `tests/unit/composer.test.mjs:32,34,316,333,335` — см. F3 |
| Удаление/переименование теста | 0 | — |
| Правка `tsconfig` ради «зелени» | 0 | `tsconfig*.json` в диффе отсутствуют |
| Запись в golden (`logs/`, `functions/`, `evidence/`) | 0 | `git status` по этим каталогам пуст |

`skip`-аудит: 14 вызовов `t.skip(`. Существовавшие до волны (7 файлов) — про `git`, Frida,
`c++filt`, живой CDP. Новые (RW-40) — `gui-spike.smoke` и `gui-window.smoke` (нет
`DISPLAY`/`WAYLAND_DISPLAY`; на дисплее отсутствие бинарника = fail, не skip),
`gui-spike-bench` (не-linux платформа, недоступный `/proc/<pid>`). У всех — причина и условие
среды, «тихого» пропуска нет. Семь e2e-сценариев M2 (`m2-*.e2e.test.mjs`) skip-пути не имеют:
мета-тест `e2e-required` падает при появлении `t.skip(`/`skip:` в них.

## 6. Карточки `waves/RW-40/PM/*`: red/green и состояние

| Карточка | `red.log` | `green.log` | md5 теста: отчёт / сейчас | REVIEW | Состояние |
|---|---|---|---|---|---|
| `stub27-core` | compile-red `E0432 unresolved import discord_core::stream` + 5 мутантов, все caught (FAILED) | `11 passed; 0 failed` | `bfc82f9d…` = `bfc82f9d…` ✔ | нет | фикс без ревью |
| `sec-l02-01` | `23 passed; 1 failed` (assert в `net_contract`) + 2 мутанта caught | `24 passed` + `16 passed` | `573d2712…` = `573d2712…` ✔ | нет | фикс без ревью |
| `b7b-search` | 3 красных прохода (compile `E0560` → assert), 6 мутантов caught | `24 passed` + `16 passed` | `a0200648…` = `a0200648…` ✔ | нет | фикс без ревью |
| `b2b-attach-ui` | падение импорта `ATTACH_LIMITS` (`tests 1 / fail 1`) | `21 passed / 0 failed` | `03ae4054…` = `03ae4054…` ✔ | нет | фикс без ревью |
| `g05-rest` | `27 tests / 6 pass / 21 fail` | `27 passed / 0 failed`, `tsc_exit=0` | `83ed2062…` = `83ed2062…` ✔, `3c852610…` = ✔ | нет | фикс без ревью |
| `v01-rtc` | `FAILED. 0 passed; 16 failed` + 10/10 мутантов caught | 13 целей, все `ok`; один прогон `0 passed` | `1d6b0e2d…` ✔, `263184bb…` ✔ | нет | фикс без ревью |
| `b2a-attach` | `FAILED. 1 passed; 14 failed` + 6/6 мутантов killed | `15 passed` + `2 passed` | отчёт `cdc8cbd9…`, сейчас `8563afd3…` ✖ | **RETURN P1=1** | **TIER-FIX в работе**: `red-fix.log` (16:56) есть, green-fix/REPORT-FIX/повторного ревью нет |
| `m07-ui-badge` | `13 tests / 8 pass / 5 fail` | `29 passed / 0 failed` | `fe363afe…` = ✔ | **RETURN P2=1** | `CONTRACTS/read-state.md` в рабочем дереве уже описывает `data-badges`/`data-flags` (mtime 16:52), но fix-лога и повторного ревью карточки нет |
| `g01b-ui` | `14 tests / 11 pass / 3 fail` | `57 passed` + полный unit `713/713` | `939ab3ab…` → сейчас `8e44df12…` ✖ | **RETURN P2=1** | P2-фикс в работе: `red-fix.log` (16:56), green-fix нет |
| `stub14-search-ui` | `5 tests / 2 pass / 3 fail` + `red-fix.log` | `105 passed / 0 failed` + `green-fix.log` | `b2692ff…` = ✔ (REPORT-FIX) | **RETURN P1=1, P2=1** | фикс закрыт, ждёт повторного ревью |

Все 10 карточек из ТЗ имеют и `red.log`, и `green.log`; ни один лог не пустой; зелёные логи
содержат итоговые счётчики с exit 0. Претензия к содержимому — только у `v01-rtc` (F10).

## 7. md5 тестовых файлов (снимок 16:58:05)

Полный список 179 файлов — `TEST-INTEGRITY-WAVE3-md5.txt`. Ключевые:

| Файл | md5 |
|---|---|
| `tests/unit/channel-paging.test.mjs` | `4ef50982a5de2847765bd1bc64d8dedf` |
| `tests/unit/composer.test.mjs` | `03ae40546f4f1801c553c67656de5eea` |
| `tests/unit/dispatch_ui.test.mjs` | `d30771d65b6373ecbbac6de1f9e61a3d` |
| `tests/unit/guild-rail.test.mjs` | `8e44df12e01f2fd8a831ec615e30a360` (изменён в 16:56 фиксом G-01b) |
| `tests/unit/gui-spike-features.test.mjs` | `3c85261013fe463427ef59c1ea4bf6a8` |
| `tests/unit/gui-spike-regressions.test.mjs` | `e49d9de7c72ccdaf38031a5648d4f9a0` (снимок 16:58; к 17:00 — `ca0416266f00b06208c6a2814c2aa1ca`, STUB-16 продолжал правку) |
| `tests/unit/read-state-ui.test.mjs` | `fe363afe9abcc82bc99e1e59b0af58c5` |
| `tests/unit/search-remote-real-index.test.mjs` | `b2692ff1bb6b0fb107e998ac2f5b2c2d` |
| `tests/unit/server-menu.test.mjs` | `83ed20625a33779ea173431c3d0da836` |
| `rust/crates/discord-core/tests/stream_gateway_contract.rs` | `bfc82f9d2dacc7bccb1e1475ddb5c7c0` |
| `rust/crates/discord-voice/tests/session_contract.rs` | `1d6b0e2d4e76ce3602b199eac848da0f` |
| `rust/crates/discord-voice/tests/signaling.rs` | `263184bbb3f38bcbc232b53e95351b26` |
| `rust/gui-spike/tests/attachments_contract.rs` | `8563afd362e5e3ba7f66b4f2e3704c02` (изменён в 16:57 TIER-FIX) |
| `rust/gui-spike/tests/search_contract.rs` | `a02006481e7e97ab26667367be8ce902` |
| `rust/gui-spike/tests/net_contract.rs` | `573d271204f3c1db601e2df73684f129` |
| `rust/gui-spike/tests/messages_contract.rs` | `0f92348a5e3e15563603faec0917c4b5` |

## 8. Находки

### F1 — P1 (gate): `plan-sync` красный, §8 ссылается на несуществующие токены

`node --test tests/meta/*.test.mjs` → exit 1, единственный красный:
`plan-sync.test.mjs:118`. Четыре токена из `TDD-PLAN.md §8` (строки 403–404) отсутствуют в тестах:

- `tests/unit/composer.test.mjs`: `` `initComposer` `` — такой экспорт не существует нигде,
  тест импортирует `createComposer`;
- `tests/unit/server-menu.test.mjs`: `` `DISABLED_REASON_TEXT` ``, `` `initActions` ``,
  `` `leaveFailureText` `` — тест работает через `loadApp()` и этих идентификаторов не содержит.

Тесты корректны, неверна строка §8. Правка — в `TDD-PLAN.md` (файл Team Lead), тесты трогать не надо.
Фактические токены: для composer — `createComposer`, `MAX_UPLOAD_FILE_BYTES`, `ATTACH_LIMITS`,
`ATTACH_TEXT`, `GIFT_TEXT`, `formatBytes`; для server-menu — `MENU_LABELS`, `OWNER_TEXT`,
`guild-leave`, `leave_guild`, `data-action`.
Пока строка не исправлена, фаза `meta` гейта красная, приёмка волны невозможна (§2.6).

### F2 — P1 (evidence): `b2a-attach` — файлы на диске не совпадают с отчётом и «совпавшим» паритетом

`REPORT.md` и `REVIEW.md` фиксируют `attachments_contract.rs = cdc8cbd9…` и
`src/attachments.rs = c2e106…`; на диске в 16:58 — `8563afd3…` и `e28b8cbc…`. При этом
`PARITY/MERGE-READINESS-07.md` помечает эти md5 как «совпал [V]», что уже неверно.
В каталоге есть только `red-fix.log` (16:56) — red-фаза TIER-FIX; `green-fix`, `REPORT-FIX`,
обновлённого `REVIEW` и новых md5 нет. Ревью карточки — `RETURN, P1=1` (жёсткий лимит 10 MiB
вместо tier 10/50/100). До закрытия P1 карточка не принимается; `MERGE-READINESS-07` требует
перезамера.

### F3 — P0 по букве §2.4: `@ts-ignore` в тесте без письменного обоснования

`tests/unit/composer.test.mjs:32,34,316,333,335` — пять `// @ts-ignore` (присваивание
`globalThis.document`/`window`, поля `err.status`/`err.retry_after_ms`). Это единственный
тестовый файл репозитория с `@ts-ignore`; письменного обоснования ни в отчёте карточки,
ни в `DECISIONS.md` нет. TEST-POLICY §2.4 относит такое к абсолютным запретам.
Влияние ограничено: рантайм-проверки не тронуты, tsc по этому файлу чист.
Лечение дешёвое — либо строка-обоснование рядом с директивой (и упоминание в отчёте),
либо замена на приведение типа. Решение о тяжести — за Team Lead.

### F4 — P2 (оформление §5.5): изменения ожиданий не записаны в `DECISIONS.md`

Для двух изменений тестов есть таблица `old → new → основание` в отчётах карточек
(`c02-ui-older/FIX-REPORT.md`, `g05-rest/REPORT.md` §«Ослабления нет»), но записи в
`DECISIONS.md` (ID, дата, причина, риск) нет; подтверждение PM для 429 существует только
как комментарий внутри теста («согласовано PM 2026-10-30»). Формально §5.5 не выполнен.

### F5 — P2: устаревший md5 в паритете

`PARITY/MERGE-READINESS-07.md:119` — `tests/unit/search-remote-real-index.test.mjs`
`9b099543…` с отметкой «совпал [V]»; актуальный файл после STUB-14-UI-SEARCH-FIX —
`b2692ff1…` (совпадает с `stub14-search-ui/REPORT-FIX.md`). Документ нужно перезамерить
после закрытия фиксов.

### F6 — P3: имя теста не совпадает с утверждением

`rust/crates/discord-core/tests/rest_routes.rs:80` — функция `catalog_shape_48_unique`
проверяет `assert_eq!(ROUTES.len(), 49)`. Внесено коммитом `4e24048`. Не ослабление,
но сбивает при чтении; переименовать при следующей правке файла.

### F7 — состояние, блокирует приёмку: у 6 из 10 карточек нет ревью

`stub27-core`, `sec-l02-01`, `b7b-search`, `b2b-attach-ui`, `g05-rest`, `v01-rtc` — `REVIEW.md`
отсутствует. TEST-POLICY §3.5 (§5.4) требует проверки диффа тестов не автором; без ревью
карточка не закрывается.

### F8 — состояние: четыре открытых RETURN

`b2a-attach` (P1), `m07-ui-badge` (P2, фикс не залогирован), `g01b-ui` (P2, фикс в red-фазе),
`stub14-search-ui` (P1+P2, фикс закрыт, ждёт повторного ревью). Ни одна из четырёх не должна
попасть в приёмку волны до повторного прохода.

### F9 — ограничение снимка: три файла менялись во время аудита

`tests/unit/guild-rail.test.mjs` (16:56, фикс G-01b), `tests/unit/gui-spike-regressions.test.mjs`
(16:57, STUB-16), `rust/gui-spike/tests/attachments_contract.rs` и `src/attachments.rs`
(16:57, TIER-FIX). Их md5 в §7 — состояние на момент снимка, а не «после приёмки».
Снимок md5 надо повторить, когда эти карточки закроются.

### F10 — замечание: `v01-rtc/green.log` содержит цель с `0 passed`

Последняя цель в логе — `test result: ok. 0 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out`.
Это результат фильтра по имени, а не skip, но в отчёте стоит указать, какая цель так
отработала, чтобы «0 passed» не читался как отсутствие проверок.

### F11 — gate: `typecheck` был красным из-за карточки вне аудита, к 17:00 позеленел

На 16:57 `npx tsc -p tsconfig.json` → exit 1, ровно 2 диагностики, обе в
`tests/unit/gui-spike-regressions.test.mjs:457` (`ensureMemberProfilePopover`,
`renderMemberProfileState` отсутствовали в `ui/features/members.js`) — это была red-фаза
`STUB-16-MEMBER-PROFILE-POPOVER-UI` (каталог `waves/RW-40/PM/stub16-profile-ui/` содержал
только `red.log` от 16:57). Повтор в 17:00 — exit 0, вывод пустой: экспорты появились,
фаза `typecheck` зелёная. На тесты из ТЗ это не влияло; блокером остаётся только `meta` (F1).

## 9. Вердикт по TEST-POLICY

**Ослаблений тестов не найдено: P0 по §2.1–§2.3, §2.7, §2.8 — чисто. Единственный формальный
P0 — F3 (§2.4, `@ts-ignore` без обоснования); по влиянию он ограничен и лечится одной строкой.**
Ни один тест не удалён, ни одна проверка не снята без контрактного основания, пороги и допуски
не понижены, `.only`/`todo`/пустых assert нет, новых необоснованных skip нет, тестовые
файлы не переименовывались, golden не тронут, оракулы (READY/MESSAGE_CREATE-кадры, захват
47 маршрутов, RE-дампы) не переписаны под код.

Формальные нарушения и блокеры:

| Пункт TEST-POLICY | Статус |
|---|---|
| §2.1 ослабление тестов | чисто |
| §2.2 подгонка теста под код | чисто (два изменения ожиданий — контрактные, с основанием, см. §3) |
| §2.3 сокрытие падений (`skip`/`todo`/`only`/пустые assert) | чисто |
| §2.4 отключение инструментов | **F3**: 5 × `@ts-ignore` без обоснования в `composer.test.mjs` |
| §2.5 приёмка мимо проверок | см. F7: 6 карточек без ревью |
| §2.6 merge с красным статусом | **F1**: фаза `meta` красная (`plan-sync`); `typecheck` к 17:00 зелёный (F11) |
| §3 RED → GREEN → TARGETED → GATE | у всех 10 карточек red и green есть; gate — за integration-лейном |
| §3.6 мутации | приложены: stub27 5/5, sec-l02 2/2, b7b 6/6, b2a 6/6, v01 10/10 — все caught |
| §4 evidence (команда → exit → артефакт) | у `stub27-core`, `sec-l02-01`, `v01-rtc` в `red.log` нет строки команды (только вывод cargo; команда есть в REPORT), у `b2a-attach` — упомянута в конце лога; не блокер, но формат §4 лучше соблюдать |
| §5.2 таблица old → new → основание | есть у `c02-ui-older`, `g05-rest` |
| §5.5 запись в `DECISIONS.md` | **F4**: отсутствует |

**Итог:** целостность тестов волны 3 в порядке — тесты не «зеленились» правкой тестов под код.
Волна к приёмке не готова по трём причинам: красная фаза `meta` (F1), незакрытые RETURN и
отсутствующие ревью (F7, F8), расхождение evidence в `b2a-attach` (F2). Плюс два бумажных
пункта к устранению: обоснование `@ts-ignore` (F3) и записи в `DECISIONS.md` (F4).

## 10. Что не проверялось

- `cargo test`/`clippy`/`fmt` и Rust-мутации — запрет ТЗ; выводы по Rust-тестам сделаны по диффу,
  логам карточек и статическому чтению.
- Живые окна, e2e в браузере, live-CDP — нет окна и браузерной среды в этом прогоне;
  e2e-логи волны не пересматривались на исполнение, только на отсутствие skip-путей.
- Юнит-тесты внутри `rust/**/src/**` (модульные `#[cfg(test)]`) — вне объявленной области;
  выборочная проверка диффа показала 0 удалённых assert в `src`.
- Поведение шима `tests/support/gui-dom-harness.mjs` (1412 строк) — не аудировалось на
  «снисходительность»: это отдельная задача, если понадобится.
