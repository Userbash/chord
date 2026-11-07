# Отчёт — task-277 (`G2 Frontend Pass 2`, `FE-FIX-02`)

Устранение 10 ошибок `tsc` в 7 файлах батча `Build-003` (блокер `npm run typecheck`, `QA-BUILD-003-REPORT.md` §5).

- **HEAD:** `355be21` (ветка `bugfix/task-277-ui-batch-typecheck-fix` от `unstable`)
- **Окружение:** node `v26.13.0`, `tsc` `7.0.2`, `tsconfig.json` c `strict` + `noUncheckedIndexedAccess` + `checkJs`
- **Коммит:** не делался (запрещено ТЗ, слияние — за координатором)

## Red

`npx tsc -p tsconfig.json --noEmit` до правок дал ровно **10 ошибок** в 7 файлах (`red.log`):

| Файл:строка | Код | Причина |
| --- | --- | --- |
| `thread-badge.js:75` | TS2322 | `THREAD_ARCHIVE_DURATIONS_RU[...]` даёт `string \| undefined` при `noUncheckedIndexedAccess` |
| `thread-badge.test.mjs:75` | TS2322 | `{ name: 42 }` — намеренно невалидный вход негативного теста |
| `thread-badge.test.mjs:100` | TS2322 | `{ messageCount: "abc" }` — намеренно невалидный вход |
| `chat-helpers-bridge.test.mjs:94` | TS2322 ×2 | `{ locked: 1, archived: "yes" }` — truthy-вход негативного теста |
| `event-banner.test.mjs:61` | TS2322 | `{ userCount: "abc" }` — намеренно невалидный вход |
| `guild-boost.test.mjs:21` | TS7053 | `for (const tier of [0,1,2,3])` даёт `number`, а `BOOST_TIER_PERKS_RU` ключуется `0 \| 1 \| 2 \| 3` |
| `poll-card.test.mjs:442` | TS2532 | `submitted[0]` — `Object is possibly 'undefined'` |
| `slowmode.test.mjs:87` | TS2345 | `undefined` в параметр `lastMessageSentAtMs: number` |
| `slowmode.test.mjs:108` | TS2345 | `undefined` в параметр `nowMs: number` |

## Green

Правки — только аннотации и вызовы, проверки тестов не ослаблены, `@ts-ignore`/`@ts-expect-error` не добавлялись.

1. `rust/gui-spike/ui/features/thread-badge.js` — к подписи срока добавлен `?? "24 часа"` как статический фолбэк для `noUncheckedIndexedAccess`. `normalizeArchiveDurationMinutes` и так возвращает только ключи справочника, поэтому наблюдаемое поведение не меняется (тесты `archiveLabel` для `60/4320/10080/777/undefined` зелёные).
2. `tests/unit/thread-badge.test.mjs:75,100` — негативные входы обёрнуты в `/** @type {any} */ (...)`.
3. `tests/unit/chat-helpers-bridge.test.mjs:94` — аргумент с `{ locked: 1, archived: "yes" }` обёрнут в `/** @type {any} */ (...)`, проверка truthy-приведения осталась.
4. `tests/unit/event-banner.test.mjs:61` — `{ userCount: "abc" }` обёрнут в `/** @type {any} */ (...)`.
5. `tests/unit/guild-boost.test.mjs:20` — добавлен `/** @type {Array<0 | 1 | 2 | 3>} */ const tiers = [0, 1, 2, 3]`. Индексация стала типизированной, проверка `Object.isFrozen` не изменилась.
6. `tests/unit/poll-card.test.mjs:441-444` — вместо `submitted[0].length` деструктуризация `const [votePayload] = submitted;` + `assert.ok(votePayload, ...)` (assert сужает тип). Проверка стала сильнее, не слабее.
7. `tests/unit/slowmode.test.mjs:87,108` — намеренно невалидный `undefined` обёрнут в `/** @type {any} */ (...)`.

### Проверки

```
$ npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E 'thread-badge|chat-helpers-bridge|slowmode\.test|event-banner\.test|guild-boost\.test|poll-card\.test' | wc -l
0

$ node --test tests/unit/thread-badge.test.mjs tests/unit/chat-helpers-bridge.test.mjs \
    tests/unit/slowmode.test.mjs tests/unit/event-banner.test.mjs \
    tests/unit/guild-boost.test.mjs tests/unit/poll-card.test.mjs
ℹ tests 68   ℹ pass 68   ℹ fail 0   ℹ skipped 0
```

Полные логи: `red.log`, `green.log`.

## Остаток вне Write-Scope

Полный `tsc` по-прежнему выдаёт 170 строк ошибок в файлах, которых нет в моём Write-Scope: `tests/unit/profile-modal.test.mjs` (29), `svg-icons.test.mjs` (27), `appearance-settings.test.mjs` (22), `channel-tree.test.mjs` (17), `locale-switcher.test.mjs` (9), `inbox.test.mjs` (6), `embed-formatter.test.mjs` (6), `voice-input-mode.test.mjs`, `poll-progress.test.mjs`, `markdown-helper.test.mjs`, `invite-parser.test.mjs`, `forum-filter.test.mjs`, `audit-log.test.mjs`, `attachment-validator.test.mjs`, а также `rust/gui-spike/ui/features/{svg-icons.js,invite-parser.js,token-map.js,inbox.js,attachment-validator.js}`. Это материал следующих карточек `FE-FIX-*`; здесь не трогал.

`git status` до начала работы уже показывал ` D node_modules` и ` M package-lock.json` — чужие/фоновые изменения worktree, мной не вносились и не менялись.

## md5sum (7 файлов)

```
8a8dbe830a80e378e3956ad614d29fc6  rust/gui-spike/ui/features/thread-badge.js
6d3d5ff74edd2275036d992c8e40c966  tests/unit/thread-badge.test.mjs
5cd90592ac62d354d0c64c1d236dccfd  tests/unit/chat-helpers-bridge.test.mjs
47f98b38846d38d7bdd898873c1090cf  tests/unit/slowmode.test.mjs
df7199b63cd808f01081639e74063cf1  tests/unit/event-banner.test.mjs
2b715cebeef80042911e35721c73e483  tests/unit/guild-boost.test.mjs
ff5b0b0f4fd7f8395b28667c3e9d19ba  tests/unit/poll-card.test.mjs
```

## Риски

- Фолбэк `"24 часа"` в `thread-badge.js` — литерал-дубликат значения `1440` из справочника. Реакция на дрейф: если справочник переименуют, `??` даст устаревшую подпись только в невозможной ветке (normalize отдаёт лишь ключи словаря). Альтернатива — сделать `normalizeArchiveDurationMinutes` возвращающей union ключей, это шире по диффу и цепляет публичный контракт модуля.
- Тесты не ослаблены: `assert.deepEqual`/`assert.equal`/`assert.ok` и все ветки негативных кейсов на месте, `@ts-expect-error` встречается только там, где был изначально (`slowmode.test.mjs:21`, freeze-проверка).
