# REPORT — task-282 (`FE-FIX-06`, batch 3): 34 ошибки `tsc` в 10 файлах `tests/unit/*.test.mjs`

**Воркер:** `deepseek/deepseek-v4.1-flash` (`--agent opencode`) | **Ветка/дерево:** `bugfix-task-282-unit-tsc-fix-batch3` | **База:** `17dd852`
**Правки только в 10 тестовых файлах из Write-Scope.** Продуктовый код, чужие тесты и доки не тронуты. `git commit` не делался.

## 1. Red (до правок)

Команда из ТЗ, вывод — в `red.log`:

```
npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "locale-switcher\.test|inbox\.test|embed-formatter\.test|voice-input-mode\.test|poll-progress\.test|markdown-helper\.test|invite-parser\.test|forum-filter\.test|audit-log\.test|attachment-validator\.test"
```

Результат: **34 ошибки** (`wc -l red.log` → `34`), распределение совпало с ТЗ: locale-switcher 9, inbox 6, embed-formatter 6, voice-input-mode 3, poll-progress 3, markdown-helper 3, invite-parser 1, forum-filter 1, audit-log 1, attachment-validator 1.

## 2. Green (после правок)

Та же команда: **0 строк**. `green.log` — вывод этой команды (пусто) плюс сводка `node --test` по 10 файлам.

```
ℹ tests 150
ℹ pass 150
ℹ fail 0
ℹ skipped 0
```

Полный `tsc` по проекту показывает 121 строку ошибок, все — вне Write-Scope (`ui/features/*.js`, `tests/unit/appearance-settings.test.mjs` и др.), ни один файл из 10 в выводе не встречается.

## 3. Что именно правилось

Ни одна проверка `assert.*` не ослаблена, `@ts-ignore`/`@ts-nocheck` не добавлены, логика тестов сохранена.

| Файл | Ошибок | Приём |
| --- | --- | --- |
| `locale-switcher.test.mjs` | 9 | `makeStorage.getItem` → `map.get(k) ?? null` (совпадение с `LocaleStorage.getItem(): string \| null`); индексный доступ `SUPPORTED_LOCALES[0]?.[...]` (3); JSDoc `@param {any}` для коллбеков `children.map/filter` (2); остальные 4 — следствие первых двух |
| `inbox.test.mjs` | 6 | `/** @returns {string} */` на геттере `textContent` (снимает рекурсивный TS7023/TS7024); `const name = /** @type {string} */ (m[1])` в `matches`; `selected[0]?.getAttribute(...)`; приведение `popover.querySelector(...)` к `FakeElement` перед `querySelector("button")` |
| `embed-formatter.test.mjs` | 6 | `normalized.footer?.text.length`, `normalized.fields[N]?.…` (5) |
| `voice-input-mode.test.mjs` | 3 | негативные входы через `/** @type {any} */`: `"‑40"`, `"300"`, `{ pttPressed: "yes" }` |
| `poll-progress.test.mjs` | 3 | `result.items[0]?.percentage`, `result.items[1]?.…` |
| `markdown-helper.test.mjs` | 3 | удалены 3 неиспользуемых `// @ts-expect-error` (сигнатура `parseDiscordTimestampTag` уже `@param {unknown}`); комментарии сохранены |
| `invite-parser.test.mjs` | 1 | объект с `memberCount: null` обёрнут в `/** @type {any} */ (…)` |
| `forum-filter.test.mjs` | 1 | `input[0]?.tagIds` |
| `audit-log.test.mjs` | 1 | `actionType: /** @type {number} */ (AUDIT_ACTION_TYPES.CHANNEL_CREATE)` |
| `attachment-validator.test.mjs` | 1 | вызов с лишним аргументом через `/** @type {any} */ (isSpoilerFilename)(...)` |

## 4. md5 изменённых файлов

```
6291ec0b5c8e63384edf429d8b2abaf0  tests/unit/locale-switcher.test.mjs
2852aaeb1a7d809a722f0123021e25e7  tests/unit/inbox.test.mjs
75fc3b5982c1752e4444d80ee1d6ff67  tests/unit/embed-formatter.test.mjs
6107a504740b5b78c695ef986af3ca86  tests/unit/voice-input-mode.test.mjs
95564e7c679ce6c2afc828e20586f860  tests/unit/poll-progress.test.mjs
905f3ea4e05752fac475414df5220072  tests/unit/markdown-helper.test.mjs
f23ce4063955eb06492a8790e1b87fd0  tests/unit/invite-parser.test.mjs
a6b3b6f9d59e7bc00622f71dc56677ff  tests/unit/forum-filter.test.mjs
d3a2a6c45352787713c011c43295bb65  tests/unit/audit-log.test.mjs
db4a6f5823a22732bfbc2c3cdcb71f0a  tests/unit/attachment-validator.test.mjs
```

`git diff --stat`: 10 файлов, `31 insertions(+), 29 deletions(-)`.

## 5. Риски и замечания

- Optional chaining в `assert.equal`/`assert.deepEqual` меняет вид падения при отсутствии элемента: вместо `TypeError` тест упадёт сравнением `undefined !== expected`. Проверка не ослаблена, но сообщение об ошибке другое.
- `red.log`/`green.log` сняты на текущем дереве: `node_modules` в `git status` помечен как `D`, `package-lock.json` — `M`; эти изменения пришли с деревом, не мои, я их не трогал.
- Вне Write-Scope по-прежнему 121 строка ошибок `tsc` (в основном `rust/gui-spike/ui/features/*.js` и `tests/unit/appearance-settings.test.mjs`) — это следующие батчи, не входят в `FE-FIX-06`.
- Полный `gate:worker` (`fmt`/`clippy`/`test --workspace`/e2e) не запускался: уровень `T0` для batch-воркера — только targeted-проверки (`tsc` + `node --test` по 10 файлам).
