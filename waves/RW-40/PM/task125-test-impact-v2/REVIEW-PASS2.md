# Независимое ревью Pass 2 — task-125-test-impact-v2 (Review 44B)

Дата: **2026-10-30, 21:16 +05**. Ревьюер: `DeepSeek V4.1 Flash` (`Review 44B`, слот 4 из 4), независимая сессия.
Репозиторий: `/home/sanya/orca/workspaces/project/qa-task-125-test-impact-v2`, HEAD `4e24048`
(`RW-40: integrate Discord GUI spike, dispatch, paging and QA evidence`), ветка `qa/task-125-test-impact-v2`.
Проверялись исправления прохода 2 против `PM/task125-test-impact-v2/REVIEW.md` (Review 44A).

Артефакты (MD5 на момент проверки; любая правка инвалидирует ревью):

| Артефакт | MD5 |
|---|---|
| `scripts/test-impact.mjs` (279 строк) | `675ac3a6f7d8276be2ea4070bc1c327a` |
| `tests/unit/test-impact.test.mjs` (466 строк) | `efccfa21defac9c31228f9633a9a8770` |
| `waves/RW-40/G4/task-125-test-impact-v2/REPORT.md` (124 строки) | `c06ab5c164794d3641a151cafb71728d` |

Оракул: `TEST-STRATEGY-V2.md` §5.2.1–§5.2.3, §8.1; файловая система и `git` (`rust/gui-spike/tests/*.rs`,
`src/lib.rs`, `git ls-files`); `BOARD.md:57`; импорты профильных тестов. Продуктовый код и тесты
ревьюер не менял. Созданы этот файл и четыре лога в `evidence/`.

## Вердикт

**RETURN.** P0 = 0, **P1 = 1**, P2 = 3, P3 = 2.

Все четыре замечания 44A (P1-1, P1-2, P2-1, P2-2) и оба P3 закрыты по существу. Но новый тест
фиксирует снимок текущей ветки: проверка «существующие модули таблицы» падает, как только появляется
первый из запланированных `features/<mod>.js`. В интеграционном дереве `unstable` эти модули уже
есть, поэтому зелёный прогон не переносится на целевое дерево. Это воспроизведено, а не выведено
рассуждением (P1-1). Плюс safety net §5.2.3 реализован не полностью: неизвестный путь и `ui/app.js`
не дают полного прогона, а тест закрепляет более слабое поведение.

## 1. Запрошенные прогоны

| Команда | Результат | Лог |
|---|---|---|
| `node --test tests/unit/test-impact.test.mjs tests/meta/test-policy.test.mjs` | **exit 0**, tests 62, pass 62, fail 0 (45 + 17) | `evidence/targeted-pass2.log` |
| `node --test tests/unit/chat-reactions.test.mjs tests/unit/search-remote-real-index.test.mjs tests/unit/gui-spike-dom.test.mjs` | **exit 0**, 40/40 — профильные тесты из новых правил реально проходят | `evidence/profile-trio-pass2.log` |
| Независимый red: селектор из `HEAD` + новый тест | **exit 1**, tests 45, pass 19, fail 26 | `evidence/red-repro-head-pass2.log` |
| `npx tsc -p tsconfig.json` | **exit 1**, 16 ошибок, все в `tests/unit/dispatch_ui.test.mjs`; в файлах карточки 0 | `evidence/` (см. §4) |
| `node --test tests/meta/plan-sync.test.mjs` | **exit 1**, 4/5, один кейс «тестовые файлы без строки в §8» — внешний блокер | — |

`red2.log` автора даёт 45 / 18 pass / 27 fail, мой прогон на `HEAD`-селекторе — 19 / 26. Разница
ровно в одном кейсе («фантомный плоский путь»): в `red2.log` он падает, потому что red снимался с
селектора прохода 1, где ещё жили плоские ключи `ui/<mod>.js`. Это корректный baseline «до правки
прохода 2», но в отчёте он не назван — см. P3-2.

## 2. Находки

### P1-1. Тест зашивает снимок ветки и падает после появления модулей смежных карточек

`tests/unit/test-impact.test.mjs:245-260` сравнивает список существующих модулей с жёстким
`deepEqual`:

```js
assert.deepEqual(onDisk.sort(), [
  "rust/gui-spike/ui/features/reactions.js",
  "rust/gui-spike/ui/features/search-overlay.js",
  "rust/gui-spike/ui/features/search.js",
], ...);
```

В этой ветке трёх файлов нет, тест зелёный. В `unstable` (и в ветках task-112/113/117/124) модули
`features/notifications.js`, `features/voice-devices-settings.js`, `features/dm-friends.js` уже
лежат, а `REPORT.md` (риск №1) сам пишет, что их создают смежные карточки. После интеграции
`onDisk` содержит шесть путей, и тест падает.

Воспроизведено на копии `HEAD` + файлы прохода 2: в неё добавлены шесть модулей и шесть профильных
тестов из `unstable` (как будет после слияния), `node --test tests/unit/test-impact.test.mjs` →
**44/45, fail 1**, лог `evidence/integration-layout-sim.log`:

```
+ 'rust/gui-spike/ui/features/dm-friends.js'
+ 'rust/gui-spike/ui/features/notifications.js'
  'rust/gui-spike/ui/features/reactions.js'
  'rust/gui-spike/ui/features/search-overlay.js'
  'rust/gui-spike/ui/features/search.js'
+ 'rust/gui-spike/ui/features/voice-devices-settings.js'
```

То есть карточка не может пройти гейт в дереве, для которого написана: её зелёный держится на
отсутствии ровно тех файлов, которые она описывает.

**Исправить:** заменить `deepEqual` на инвариант, не зависящий от снимка: каждый модуль таблицы
либо существует по указанному пути, либо явно помечен как запланированный; каждый существующий по
таблице файл реально лежит на диске; ни один файл вне `features/` не получает правило. Проверку
«на диске ровно три файла» убрать или переписать как «множество существующих ⊆ таблицы».

### P2-1. Неизвестный путь не даёт полного прогона (§5.2.3), и тест это закрепляет

`TEST-STRATEGY-V2.md:215`: «…или при неизвестном пути `test-impact.mjs` автоматически переключается
в режим `all: true` (полный прогон)». Фактически:

```
$ node scripts/test-impact.mjs --changed LICENSE
tier: T0 (targeted)
  $ npm run typecheck
  $ node --test tests/meta/plan-sync.test.mjs
```

`tests/unit/test-impact.test.mjs:442-448` прямо требует `plan.commands.length === 2` («fallback не
должен разрастаться»). Это не косметика: для незнакомого файла план не запускает ни unit-, ни
Rust-прогон, хотя стратегия требует полный. Замечание 44A касалось только `package.json`/`scripts/*`,
поэтому здесь остаётся вторая половина §5.2.3.

**Исправить:** неизвестный путь → полный прогон (`all: true`), либо письменно сузить scope в
стратегии/отчёте и снять формулировку «v2 закрывает §5.2.3». Тест привести к принятому решению.

### P2-2. `ui/app.js` не входит в safety net, хотя §5.2.3 его перечисляет

Стратегия перечисляет `ui/app.js` среди общих файлов для полного прогона. Фактически:

```
$ node scripts/test-impact.mjs --changed rust/gui-spike/ui/app.js
tier: T1 (batch)
  $ npm run typecheck
  $ node --test <весь FRONTEND, 9 файлов>
  $ node waves/RW-39/PM/w39c/geometry-check.mjs
```

`isSafetyNet` (`scripts/test-impact.mjs:107-114`) `app.js` не содержит, `REPORT.md` §«Safety net»
его тоже не упоминает — отклонение не зафиксировано. Полный frontend-прогон покрывает UI хорошо, но
это не «полный прогон» из §5.2.3 (нет `npm test`, нет cargo).

**Исправить:** добавить `rust/gui-spike/ui/app.js` в safety net либо явно записать отклонение и его
причину в отчёт и стратегию. Сейчас читатель отчёта считает safety net полным.

### P2-3. Правило `features/chat.js` ссылается на отсутствующий в ветке тест

Правило добавлено по рекомендации 44A, но `tests/unit/attachments-render.test.mjs` в этой ветке нет
(в `unstable` есть, импортирует `features/chat.js:25`):

```
$ node scripts/test-impact.mjs --changed rust/gui-spike/ui/features/chat.js
  ...
  $ node --test tests/unit/attachments-render.test.mjs
$ node --test tests/unit/attachments-render.test.mjs
Could not find 'tests/unit/attachments-render.test.mjs'
```

Для существующего модуля `features/chat.js` план теперь содержит гарантированно падающую команду.
`REPORT.md` риск №1 это раскрывает, и в целевом дереве тест есть, поэтому это не P1. Но тест
`каждый существующий профильный тест лежит в tests/unit` проверяет только два имени
(`chat-reactions`, `search-remote-real-index`) и такой случай не ловит.

**Исправить:** либо проверять существование профильного теста для каждого существующего модуля и
помечать отсутствующие как запланированные (как сделано для модулей), либо не выдавать команду на
несуществующий файл. В отчёте достаточно риска, но тест должен это фиксировать, а не обходить.

### P3-1. Таблица маппинга дублируется в тесте, а whitelist не сверяется с внешним источником

`UI_MODULE_TESTS` и `RUST_MODULE_CONTRACTS` переписаны в тесте вручную. Проверки `existsSync` и
`pub mod` из `lib.rs` уменьшают риск, но «запланированные» три модуля проверяются только по
`NOT_YET_IN_BRANCH` внутри теста: опечатка вида `features/notificationz.js` в этом множестве пройдёт
зелёной. Сверить whitelist с `BOARD.md`/файлами `unstable` или вывести список модулей из
`BOARD.md` — и фантомный ключ снова не станет зелёным.

### P3-2. Baseline красного прогона в отчёте не назван

`REPORT.md` §«Red»: «до правки скрипта, проход 2… tests 45, pass 18, fail 27». Независимый red на
селекторе из `HEAD` даёт 19/26 (см. §1). Расхождение объяснимо (red снимался с селектора прохода 1,
где были плоские ключи), но из отчёта это не видно. Указать baseline («селектор прохода 1, плоские
ключи») — и цифры перестанут выглядеть расхождением.

## 3. Что закрыто и совпало

- **P1-1 (пути):** все ключи переписаны на `rust/gui-spike/ui/features/<mod>.js`; плоский
  `ui/<mod>.js` больше не получает точечное правило (`tests/unit/test-impact.test.mjs:262-271`).
  `BOARD.md:57` подтверждает layout. `features/search.js`, `features/search-overlay.js`,
  `features/reactions.js` есть на ветке; остальные три — в `unstable`.
- **P1-1 (attachments/reactions):** `features/reactions.js → chat-reactions.test.mjs` (импорт
  `features/reactions.js:31` подтверждён), вложения добавлены к правилу `features/chat.js`;
  фантомных `ui/attachments-render.js` и `ui/chat-reactions.js` в таблице нет.
- **P1-2 (Rust BFF):** таблица на 12 модулей, каждый `src/<mod>.rs` → `--test <mod>_contract
  --test ipc_contract`; тест сверяет таргеты с `rust/gui-spike/tests/*.rs` и покрытие `pub mod` из
  `lib.rs`; `lib.rs`/`main.rs` → полный прогон; модуль без правила → полный прогон крейта.
  Отклонение `ipc_contract` вместо `ipc_wiring_contract` (файла с таким именем в крейте нет)
  зафиксировано в комментарии скрипта и в отчёте — это корректная привязка к реальности.
  `core_bridge.rs → core_bridge` (файл `tests/core_bridge.rs`) — тоже верно.
- **P2-1:** `gui-spike-features.test.mjs` возвращён всем шести модулям, включая реакции.
- **P2-2:** `package.json`, `package-lock.json`, `scripts/*`, оба `Cargo.toml`, `src/lib.rs`,
  `src/main.rs` → `T2`: `typecheck` + собственный контракт селектора + `npm test` + два cargo-прогона.
  `npm test` не спавнит живой Discord: `tests/e2e/live-discord.e2e.test.mjs` skip-aware по
  `DISCORD_LAB_LIVE_CDP` (проверено чтением файла, не запуском).
- **P3-1/P3-2 отчёта:** формулировка исправлена на `TDD-PLAN.md`, риск №1 переписан на «модули
  появятся из смежных карточек; до этого правило — мёртвый код».
- MD5 обоих файлов совпали с `REPORT.md`; `.only`/`.skip`/`todo` в новом тесте нет; дифф
  `scripts/test-impact.mjs` — только вставки таблиц и одна замена `tier`; существующие правила
  (`app.js`, `tokens.css`, join-server, voice, chat) не переписаны; JSDoc и `// @ts-check` на месте.

## 4. Внешние блокеры (не дефекты карточки)

| Блокер | Факт | Статус |
|---|---|---|
| `typecheck` | 16 ошибок `TS7006/TS2339/TS2345`, все в `tests/unit/dispatch_ui.test.mjs` из `HEAD`; в файлах карточки ошибок нет | чинится вне карточки |
| `plan-sync` | 1 из 5 кейсов падает: тестовые файлы без строки в `TDD-PLAN.md §8` | чинится вне карточки |

Из-за второго safety net `npm test` на этой ветке будет красным даже после исправлений карточки.
Это ограничение ветки, а не подлог отчёта; `REPORT.md` его раскрывает.

## 5. Что ревью не проверяло

- `npm run gate:worker`, `gate:release`, `gate:qualify` не перезапускались; `cargo` не запускался
  (проверено только существование таргетов и строк плана).
- Живой Discord, окно и сеть не использовались.
- Поведение после слияния проверено симуляцией: копия `HEAD` + файлы прохода 2 + модули и профильные
  тесты из `unstable`, а не реальный merge-коммит. Сами модули смежных карточек не ревьюились.

## 6. Рекомендация

1. Убрать из теста жёсткий снимок списка модулей (P1-1) и перепроверить на дереве с шестью
   `features/<mod>.js` — тест должен остаться зелёным.
2. Закрыть §5.2.3 полностью: неизвестный путь и `ui/app.js` → полный прогон, либо письменно
   зафиксировать сужение scope и убрать заявку «v2 = §5.2.3» (P2-1, P2-2).
3. Не выдавать план на несуществующий профильный тест или проверять его наличие в тесте (P2-3).
4. Сверить whitelist запланированных модулей с `BOARD.md` (P3-1), назвать baseline red-прогона
   (P3-2).
5. После правок — повторный прогон `node --test tests/unit/test-impact.test.mjs
   tests/meta/test-policy.test.mjs` и `npx tsc -p tsconfig.json`, MD5 сверить с этим ревью.
