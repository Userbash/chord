# RW-40 / QA-WAVE8-AUDIT — независимый аудит TEST-POLICY по принятым срезам Wave 8

- **Роль:** QA-инженер, отдел G4 QA.
- **Карточка:** `QA-WAVE8-AUDIT` (read-only аудит соблюдения `TEST-POLICY.md`).
- **Дата и окно:** 2026-10-30, 18:26–18:37 +05:00. Снимок md5 — `2026-10-30T18:36:20+05:00`.
- **HEAD:** `4e24048` («RW-40: integrate Discord GUI spike, dispatch, paging and QA evidence»).
- **Режим:** код, тесты и golden-каталоги только чтение. `cargo` не запускался, `git commit/push`
  не выполнялся. Создан один файл — этот отчёт.
- **Срезы в области:** `INT-01-RUST-WIRING`, `INT-02-UI-CHAT-MARKDOWN-TYPING`,
  `INT-03-SAVE-QUEUE-FIX`, `STUB-10-PINS-THREADS-PANELS-UI`, `B2a-ATTACH-TIER-FIX-PASS2`,
  `STUB-16-MEMBER-PROFILE-POPOVER-FIX`.

> **Дерево двигалось во время аудита.** В 18:31–18:34 параллельно шёл мутационный прогон
> `waves/RW-40/PM/b2b-attach-ui/mutations-be-put.log`, который подменял и восстанавливал
> `rust/gui-spike/src/attachments.rs`. Наблюдённая последовательность md5: `84d8905b…` (baseline)
> → `da1a6f62…` (M1) → `788d55cc…` (M2) → … → `84d8905b…` (restore, 18:33:44). Все md5 ниже —
> на момент снимка 18:36:20, файл после restore не менялся 2.5 минуты.

## 1. Что проверено

| Срез | Артефакты приёмки | Продукт | Тесты | Число тестов |
|---|---|---|---|---|
| `INT-01-RUST-WIRING` | `PM/int01-rust-wiring/{ACCEPT,REPORT,REVIEW-PASS3}.md` | `rust/gui-spike/src/{lib,main}.rs`, `rust/crates/discord-voice/src/lib.rs` | `rust/gui-spike/tests/ipc_wiring_contract.rs` | 5 |
| `INT-02-UI-CHAT-MARKDOWN-TYPING` | `PM/int02-chat-md/{ACCEPT,REPORT,REVIEW}.md` | `ui/features/chat.js`, `ui/components/composer.css` | `tests/unit/gui-spike-model.test.mjs` | 53 |
| `INT-03-SAVE-QUEUE-FIX` | `PM/g01b-ui/{ACCEPT,REPORT-PASS4,REVIEW-PASS4}.md` | `ui/app.js` | `tests/unit/guild-rail.test.mjs` | 16 |
| `STUB-10-PINS-THREADS-PANELS-UI` | `PM/stub10-pins-threads/{ACCEPT,REPORT,REVIEW}.md` | `ui/features/pins-threads.js`, `ui/components/server-menu.css` | `tests/unit/channel-paging.test.mjs` | 19 |
| `B2a-ATTACH-TIER-FIX-PASS2` | `PM/b2a-attach/{ACCEPT,REPORT-PASS3,REVIEW-PASS3}.md` | `rust/gui-spike/src/attachments.rs` | `rust/gui-spike/tests/attachments_contract.rs` | 17 |
| `STUB-16-MEMBER-PROFILE-POPOVER-FIX` | `PM/stub16-profile-ui/{ACCEPT,REPORT-FIX,REVIEW-PASS2}.md` | `ui/features/members.js`, `ui/components/members.css` | `tests/unit/gui-spike-regressions.test.mjs` | 13 |

Дополнительно просмотрены все 179 файлов из `TEST-INTEGRITY-WAVE3-md5.txt` (сверка текущего
дерева со снимком 16:58) и дифф тестов против HEAD.

## 2. Запреты TEST-POLICY §2 — маркеры

Область grep: `rust/gui-spike/ui/` и `tests/` (включая `tests/meta`, `tests/e2e`, `tests/smoke`).

| Маркер | Найдено | Комментарий |
|---|---|---|
| `.only(` | 0 | единственное совпадение — регэксп-запрет в `tests/meta/e2e-required.test.mjs:72` |
| `.todo(` / `it.todo` | 0 | только регэксп в `tests/meta/e2e-required.test.mjs:73` |
| `assert.ok(true)`, `console.log` вместо assert | 0 | `console.log` есть лишь в перехвате вывода `cdp.test.mjs` |
| `@ts-ignore`, `@ts-expect-error`, `@ts-nocheck` | **0** | в `ui/` и `tests/` нет ни одного; `npx tsc -p tsconfig.json` → exit 0 |
| `eslint-disable` в тестах/UI | 0 | — |
| `#[ignore]` в `rust/gui-spike/tests` | 0 | rust-тесты среза полностью исполняемые |
| `skip` в `rust/gui-spike/ui` | 0 | — |
| `skip` в `tests/` | 17 сайтов | у каждого есть причина и условие среды (см. ниже) |
| Удаление/переименование тестовых файлов | 0 | `git status --porcelain` не содержит `D`/`R` |
| Правка `tsconfig` ради «зелени» | 0 | `tsconfig*.json` вне диффа |
| Запись в golden (`logs/`, `functions/`, `evidence/`) | 0 | `git status` по этим каталогам пуст |

Причины всех 17 `skip`: `process.platform !== "linux"` и недоступный `/proc/<pid>` (4 в
`gui-spike-bench`), отсутствие `DISPLAY`/`WAYLAND_DISPLAY` (`gui-spike.smoke`, `gui-window.smoke`),
нет `CHROME_BIN` (`cdp-browser.e2e`, `walk-fixture.e2e`), нет `DISCORD_LAB_LIVE_CDP`
(`live-discord.e2e`, 2 теста с текстом причины), нет `c++filt`, нет Frida, нет `git`, нет
`logs/calls-voice-28170.jsonl`. «Тихого» пропуска без причины нет.

Отдельно проверены 5 прежних `@ts-ignore` в `tests/unit/composer.test.mjs` и 5 в
`ui/features/composer.js`, о которых писал аудит Wave 3 (F3): обе группы удалены карточкой
`B2b-ATTACH-UI-20MIB-SYNC` (Pass 3), сейчас `grep -c` даёт `0` в обоих файлах. Замечание закрыто.

## 3. Evidence: red / green / targeted / mutation

| Срез | RED | GREEN | TARGETED | Логи |
|---|---|---|---|---|
| `INT-01` | `error[E0432]` на 7 неразрешённых импортах, exit `101` (в отчёте; в самом логе строки exit нет) | `5 passed; 0 failed` | отдельного нет; модульный контракт = green, `clippy.log` без ошибок | `red.log`, `green.log`, `clippy.log`, `mutations.log` |
| `INT-02` | `tests 53 / pass 48 / fail 5` | `53 / 53 / 0` | `272 / 272`, `node_test_exit=0`, `tsc_exit=0` | `red.log`, `green.log`, `targeted.log`, `mutations.log` |
| `INT-03` (Pass 4) | `tests 16 / pass 15 / fail 1` (задержанный `settings_save` гильдии A, выбор B) | `16 / 16 / 0` | `98 / 98`, `EXIT_CODE: 0` (3 команды, включая `tsc`) | `red-pass4.log`, `green-pass4.log`, `targeted-pass4.log`, `mutations-pass4.log` |
| `STUB-10` | `19 / 14 / 5`, падения `not implemented` | `19 / 19 / 0` | отдельного нет (один модуль) | `red.log`, `green.log`, `mutations.log` |
| `B2a` | `EXIT_CODE=101`, `14 passed; 3 failed` | `17 passed`, `exit=0` | в `green-pass3.log` три прогона, все `exit=0` | `red-pass3.log`, `green-pass3.log`, `mutations-pass3.log` |
| `STUB-16` | `EXIT_CODE=1`, `13 / 3 / 10` | `EXIT_CODE=0`, `13 / 13 / 0` | targeted перечислен в отчёте (13/13, 100/100, 38/38, 43/43), отдельного лога нет | `red-fix.log`, `green-fix.log`, `mutations-fix.log` |

Порядок red→green подтверждён mtime логов для всех шести срезов: red всегда раньше green
(например, `STUB-10`: 17:13:42 → 17:14:39; `B2a`: 17:48:45 → 17:50:17). Содержимое red-логов
показывает правильную причину падения (нет модуля/импорта, `not implemented`, конкретный
`AssertionError`), а не падение по среде.

## 4. Мутации

| Срез | Убито | Лог | Восстановление md5 |
|---|---|---|---|
| `INT-01` | **5/5** (`mut1…mut5_exit=101`, `final_green_exit=0`) | `int01-rust-wiring/mutations.log` | `restored_md5` = baseline после каждого мутанта |
| `INT-02` | **5/5** | `int02-chat-md/mutations.log` | `Restore MN MD5` = baseline, финальный блок совпадает |
| `INT-03` | **5/5 KILLED** | `g01b-ui/mutations-pass4.log` | `match: OK` после каждого |
| `STUB-10` | **5/5 CAUGHT** | `stub10-pins-threads/mutations.log` | restore проверен |
| `B2a` | **4/4 KILLED** | `b2a-attach/mutations-pass3.log` | `restore md5=3377d6dc… (совпал)`, финал = baseline |
| `STUB-16` | **5/5 KILLED** | `stub16-profile-ui/mutations-fix.log` | baseline `316bc1ac…` восстановлен |

Survivors/MISSED нет ни в одном логе. Единственное отклонение от критерия «≥5 убитых» —
`B2a`: принятая карточка (`REPORT-PASS3` + `ACCEPT.md`) опирается на 4 мутанта. В первой
итерации той же карточки было 6/6 (`b2a-attach/mutations.log`), в Pass 2 — 4/4
(`mutations-fix.log`). См. находку P2-1.

## 5. Сверка MD5 с отчётами

| Срез | Файл | В отчёте | Сейчас | Итог |
|---|---|---|---|---|
| `INT-01` | `rust/gui-spike/src/lib.rs` | `b2af030ca24749a6e4d278b5f1106a1f` | `48d01ef823c7babff52614ad95b0e60d` | ✗ |
| `INT-01` | `rust/gui-spike/src/main.rs` | `c192e36f7c3bc68637169f45cb53b203` | `dc1d395fd083020f5a7d5e038925cd1d` | ✗ |
| `INT-01` | `rust/gui-spike/tests/ipc_wiring_contract.rs` | `564bb6d670b3346a126c49d26ba1eb94` | `564bb6d670b3346a126c49d26ba1eb94` | ✔ |
| `INT-01` | `rust/crates/discord-voice/src/lib.rs` | `f365261d5c7322594bb7370a8e213b38` | `f365261d5c7322594bb7370a8e213b38` | ✔ |
| `INT-01` | 4 лога (`red/green/clippy/mutations`) | `096bf0fe…/5efd1ce6…/08003686…/ab43cff1…` | совпадают | ✔ |
| `INT-02` | `ui/features/chat.js`, `ui/components/composer.css`, `tests/unit/gui-spike-model.test.mjs` | `d2e3bb09…/d62aa5c5…/0eee4284…` | совпадают | ✔ 3/3 |
| `INT-02` | 4 лога | `2a8356b8…/d2860b21…/71d49c1d…/9c40fc8a…` | совпадают | ✔ |
| `INT-03` | `ui/app.js`, `tests/unit/guild-rail.test.mjs` | `8f5d84c1…/76a0acca…` | совпадают | ✔ 2/2 |
| `INT-03` | 4 лога Pass 4 | `3ff6b335…/8cf57dfc…/dc9b2849…/d0162e67…` | совпадают | ✔ |
| `STUB-10` | `pins-threads.js`, `channel-paging.test.mjs`, `server-menu.css` | `f3e2708a…/e6974ee4…/08c5c3cb…` | совпадают | ✔ 3/3 |
| `STUB-10` | 3 лога | `f18858e3…/f2ee4f5e…/e49f141b…` | совпадают | ✔ |
| `STUB-10` | `PM/stub10-pins-threads/REPORT.md` (сам себя) | `818d23bedcfada42897a6cde1de4b5c1` | `08ea2b99f5c3a084e8ac6730269c8a19` | ✗ (см. P2-4) |
| `STUB-16` | `members.js`, `members.css`, `gui-spike-regressions.test.mjs` | `316bc1ac…/9ff3955f…/5dbe993e…` | совпадают | ✔ 3/3 |
| `STUB-16` | 3 лога | `28ad7adc…/3a635c4a…/110c83dd…` | совпадают | ✔ |
| `B2a` | `rust/gui-spike/src/attachments.rs` | `3377d6dcd0850da6b92dbaac3a6c7dac` | `84d8905b20a67c46a7908c9ac103560a` | ✗ |
| `B2a` | `rust/gui-spike/tests/attachments_contract.rs` | `45d107db41d4c1f443de8596b1196e49` | `d1113587c21f7a91e4e0f00dc4e0c435` | ✗ |
| `B2a` | 3 лога Pass 3 | `263ad3ab…/271d99f1…/d4a6ec63…` | совпадают | ✔ |

Несовпадения в `INT-01` и `B2a` объясняются одним источником: карточка `B2b-ATTACH-UI`
(backend-PUT `attachments_upload_put`). Её лог `PM/b2b-attach-ui/mutations-be-put.log` содержит
ровно текущие значения как baseline и как restore: `lib.rs` `48d01ef8…`, `main.rs` `dc1d395f…`,
`attachments.rs` `84d8905b…`, `attachments_contract.rs` `d1113587…`. Дифф против HEAD тоже
показывает добавления `attachments_upload_put` в `lib.rs`/`main.rs` и рост контракта
`attachments_contract.rs` с 17 до 22 тестов. При этом у `PM/b2b-attach-ui/` **нет** `ACCEPT.md`
(есть только `REVIEW-PASS4.md` — PASS), то есть поверх двух принятых срезов лежит
непринятая работа.

## 6. Изменения тестов после снимка Wave 3

Со снимка `16:58` изменились 13 из 179 тестовых файлов; все — результатом карточек, принятых
позже (`INT-01/02/03`, `STUB-10/16`, `B2a`, `B2b`, `SEC-L02-01`, `B7b`). Ослаблений не найдено:

| Файл | Удалено строк | Что именно | Вердикт |
|---|---|---|---|
| `tests/unit/dispatch_ui.test.mjs` | 9 | JSDoc-заголовки, `view`-литерал, каст в `expected.map((/** @type {any} */ guild) => …)` | assert сохранён, изменена только типизация |
| `tests/unit/gui-spike-features.test.mjs` | 2 | 2 старых assert по 429 заменены 5 более строгими (`disabled` во время отсчёта, `calls === 2` без автоповтора, разблокировка, ручной клик, `calls === 3`) | усиление |
| `tests/unit/guild-rail.test.mjs` | 1 | `return node` → `return /** @type {ShimElement} */ (node)` в хелпере | проверки не тронуты, `it` 8 → 16 |
| `rust/gui-spike/tests/invites_contract.rs` | 4 | список импортов и два поля `headers: Vec::new()` в литералах | проверки не тронуты |
| `tests/unit/channel-paging.test.mjs` | 0 | дифф против HEAD — только добавления (`it` 12 → 19) | удалений нет |

Расхождение с отчётом Wave 3: там у `channel-paging.test.mjs` было 10 удалённых строк. Сейчас
дифф против HEAD даёт `324 / 0`, а набор `it` — надмножество HEAD (12 → 19, ни один заголовок
не пропал). Файл переписан карточкой `STUB-10` между 16:58 и 17:13, исходное состояние
Wave 3 больше не воспроизводится. Тест «пересечение страниц» из описания Wave 3 в текущем файле
отсутствует; при этом ни одна проверка HEAD не потеряна. Это расхождение документации, а не
ослабление тестов (P2-5).

## 7. Воспроизведение на текущем дереве

| Команда | Exit | Результат |
|---|---|---|
| `node --test tests/unit/channel-paging.test.mjs` | 0 | 19 tests, 19 pass, 0 fail, 0 skip |
| `node --test tests/unit/guild-rail.test.mjs tests/unit/gui-spike-regressions.test.mjs tests/unit/gui-spike-model.test.mjs` | 0 | 82 tests, 82 pass, 0 fail, 0 skip (16 + 13 + 53) |
| `node --test tests/meta/test-policy.test.mjs tests/meta/e2e-required.test.mjs tests/meta/ipc-path.test.mjs` | 0 | 44 tests, 44 pass |
| `npx tsc -p tsconfig.json` | 0 | без вывода (`noEmit: true`) |

Числа совпадают с заявленными в отчётах и `ACCEPT.md` шести срезов. Rust-тесты
(`ipc_wiring_contract`, `attachments_contract`) не перезапускались: `cargo` запрещён ТЗ, их
результаты приняты только по логам.

## 8. Находки

**P1-1. MD5 `INT-01` не воспроизводятся.** `lib.rs` и `main.rs` в дереве отличаются от
`REPORT.md`/`ACCEPT.md`. Следствие: 5 мутантов и green-прогон `INT-01` относятся к содержимому,
которого в дереве уже нет. Причина — правки `B2b`, а не ошибка автора, но evidence по
TEST-POLICY §9 («md5 файлов совпадают») на текущем состоянии не выполняется.

**P1-2. MD5 `B2a` не воспроизводятся.** То же для `attachments.rs` и `attachments_contract.rs`
(17 тестов в приёмке против 22 в дереве). Следствие: mutation evidence 4/4 относится к
предыдущей версии контракта.

Требование по P1-1/P1-2: после формальной приёмки (или возврата) `B2b-ATTACH-UI` перефризить
md5 в `ACCEPT.md` обоих срезов и либо перезапустить их мутации на итоговом содержимом, либо
явно записать в `ACCEPT.md`/`DECISIONS.md`, что файл заменён следующей карточкой. До этого
слияние считать неподтверждённым по критерию md5.

**P2-1. `B2a`: 4 мутанта вместо ≥5.** Принятая карточка опирается на `4/4 KILLED`
(`REPORT-PASS3`, `ACCEPT.md`). Требование аудита — не меньше 5. Покрытие инвариантов: базовый
пол 20 MiB, Tier2 50 MiB, Tier3 100 MiB и граница `>` — то есть логика покрыта, но до порога
не хватает одного мутанта (например, подмена единиц `MiB` → `MB` или снятие проверки
`validate_files` по пустому списку).

**P2-2. Evidence-строки `INT-01` неполные.** В `red.log`/`green.log` нет строки с exit code:
он указан только в `REPORT.md` (`101`). TEST-POLICY §4 требует в артефакте связку
«команда → exit code → лог». Отдельного targeted-лога у среза тоже нет (только контрактный
green и `clippy.log`).

**P2-3. Путь теста в приёмке `INT-03`.** `PM/g01b-ui/ACCEPT.md` указывает
`tests/gui-spike/guild-rail.test.mjs`; такого каталога нет, файл лежит в
`tests/unit/guild-rail.test.mjs` (md5 в приёмке при этом верный). Опечатка в пути, не в
контрольной сумме.

**P2-4. Самоописание md5 в `STUB-10`.** `REPORT.md` содержит собственную контрольную сумму
`818d23be…`, которая не совпадает с фактической (`08ea2b99…`). Сумма, записанная в тот же файл,
который она описывает, не может быть верной; строку надо убрать.

**P2-5. Дрейф описания в `TEST-INTEGRITY-WAVE3.md`.** Утверждение о 10 удалённых строках в
`channel-paging.test.mjs` и о тесте «пересечение страниц» на текущем дереве не
воспроизводится (см. §6). Проверок не потеряно, но отчёт Wave 3 нужно пометить как снимок,
относящийся к промежуточной версии файла.

## 9. Вердикт

Абсолютные запреты TEST-POLICY §2 по шести срезам **не нарушены**: `.only`/`todo`/`assert.ok(true)`
нет, `@ts-ignore`/`@ts-expect-error` нет ни одного, все 17 `skip` имеют причину, тесты не
удалялись, `tsconfig` и golden не тронуты, red→green порядок и содержание логов правдоподобны,
мутации убиты без survivors, текущие тесты воспроизводятся (19/19, 82/82, 44/44, `tsc` 0).

Блокирующее для мержа — не тесты, а целостность evidence: у 2 из 6 срезов md5 в отчётах и
`ACCEPT.md` не совпадают с деревом из-за непринятой работы `B2b-ATTACH-UI` (P1-1, P1-2).
Пока md5 не перефрижены, приёмка `INT-01` и `B2a` считается условной и слияние Wave 8
не подтверждено. Остальные замечания — P2, исправляются правкой документации и одного
дополнительного мутанта.
