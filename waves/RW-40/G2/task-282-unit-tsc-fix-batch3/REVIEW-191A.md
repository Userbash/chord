# REVIEW-191A — Техническое ревью PR-162 / task-282 (FE-FIX-06, batch 3)

- **Ревьюер:** `deepseek/deepseek-v4.1-flash` (`--agent opencode`), роль Technical Reviewer
- **Ветка/дерево:** `bugfix/task-282-unit-tsc-fix-batch3`, база `17dd852`
- **Объект ревью:** `TASK-TR-191A.md`, `ACCEPT.md`, `REPORT.md`, `red.log`, `green.log`
- **Вердикт:** `APPROVED`, `P0 = 0`, `P1 = 0`, мутации `5/5 (100%)`
- **git commit не делался**; продуктовый код не изменён (проверено побайтно)

## 1. Гейты

| Проверка | Команда | Результат |
| --- | --- | --- |
| `tsc` по 10 файлам | `npx tsc -p tsconfig.json --noEmit \| grep -E "<10 файлов>"` | **0 строк** (`tsc-final-191a.log`) |
| Unit-тесты | `node --test <10 файлов>` | **150 / 150 PASS**, `fail 0`, `skipped 0` |
| MD5 10 файлов | `md5sum` против `ACCEPT.md` | **10/10 совпали** |
| Мутации | 5 точечных в 3 продуктовых модулях | **5/5 убиты** |
| Восстановление | `git checkout -- rust/gui-spike/ui/features/` | `git status` по `rust/` чист, MD5 == HEAD |

Остальные ошибки `tsc` в дереве — **105 строк `error TS`**, все вне Write-Scope:
`tests/unit/profile-modal.test.mjs` 29, `tests/unit/svg-icons.test.mjs` 27,
`tests/unit/appearance-settings.test.mjs` 22, `tests/unit/channel-tree.test.mjs` 17,
плюс продуктовые `svg-icons.js` 5, `invite-parser.js` 2, `token-map.js`, `inbox.js`,
`attachment-validator.js` по 1. Ни один из этих файлов в PR не менялся — это следующие батчи,
не регрессия этой задачи.

## 2. Мутации (5/5 уничтожены)

Метод: правка продуктового модуля → `node --test` по тем же 10 файлам → `git checkout --`
и проверка чистоты каталога после каждой мутации.

| # | Файл | Мутация | Убийца | Итог |
| --- | --- | --- | --- | --- |
| M1 | `ui/features/locale-switcher.js` | убран `isSupported(stored)` при чтении из storage | `невалидное значение в storage откатывается к defaultLocale` | KILLED |
| M2 | `ui/features/locale-switcher.js` | `option.selected = locale.code === current` → `!==` | `помечает текущую локаль выбранной` | KILLED |
| M3 | `ui/features/embed-formatter.js` | `isSafeEmbedUrl`: выброшена ветка `http:` (остался только `https:`) | `isSafeEmbedUrl accepts only absolute http and https urls` | KILLED |
| M4 | `ui/features/embed-formatter.js` | `slice(0, MAX_FIELDS)` → `MAX_FIELDS - 1` (off-by-one, 24 поля) | `normalizeEmbedForRender slices fields to 25 and truncates name/value` | KILLED |
| M5 | `ui/features/poll-progress.js` | `Math.round` → `Math.floor` в проценте | `rounds percentages to the nearest integer` (67 → 66) | KILLED |

Каждый прогон: `tests 150 / pass 149 / fail 1`, падает ровно целевой тест — значит тесты
не «создают шум», а действительно привязаны к поведению. Лог: `/tmp/opencode/mutations-191a.log`,
по-мутационные логи `/tmp/opencode/mut-M{1..5}.log`.

После всех мутаций продуктовые файлы совпадают с HEAD побайтно:
`locale-switcher.js bf5bb085…`, `embed-formatter.js 8571b347…`, `poll-progress.js 4ee05978…`.

## 3. Проверка на ослабление тестов

- Число проверок `assert.*` и число тестов по каждому из 10 файлов **не изменилось**
  (`36/36`, `50/50`, `49/49`, `56/56`, `40/40`, `37/37`, `21/21`, `22/22`, `25/25`, `56/56`;
  суммарно 14/19/10/14/16/9/14/20/13/21 тестов, 150 всего).
- Новых `@ts-ignore`, `@ts-nocheck`, `.skip`, `todo`, `assert.ok(true)` в диффе нет.
- Ожидаемые значения в утверждениях не менялись; правки — только JSDoc-аннотации,
  `/** @type {any} */` на негативных входах, optional chaining на индексном доступе и
  снятие трёх неиспользуемых `// @ts-expect-error` в `markdown-helper.test.mjs`
  (сигнатура `parseDiscordTimestampTag` уже `@param {unknown}`, директива давала `TS2578`).

Разбор по файлам (все правки признаны допустимыми):

| Файл | Приём | Оценка |
| --- | --- | --- |
| `locale-switcher.test.mjs` | `map.get(k) ?? null`, `SUPPORTED_LOCALES[i]?.…`, `@param {any}` для коллбеков `children.map/filter` | OK; `??` отличается от `has()` только для явного `undefined`, в тестах недостижимо |
| `inbox.test.mjs` | `@returns {string}` на геттере `textContent`, `m[1]` → `@type {string}`, `selected[0]?.…`, `?.click()` | OK; если элемент не найден, следующий `assert` всё равно падает |
| `embed-formatter.test.mjs` | `footer?.text.length`, `fields[N]?.…` | OK; `undefined !== число` — тест краснеет, если поле исчезнет |
| `voice-input-mode.test.mjs` | `/** @type {any} */ ("‑40" / "300" / { pttPressed: "yes" })` | OK; негативные входы и ожидаемые `-60/200/true` сохранены |
| `poll-progress.test.mjs` | `items[N]?.…` | OK |
| `markdown-helper.test.mjs` | удалены 3 неиспользуемых `@ts-expect-error`, комментарии сохранены | OK; вызовы `null/undefined/1700000000` остались, runtime-проверка цела |
| `invite-parser.test.mjs` | `/** @type {any} */ ({... memberCount: null })` | OK |
| `forum-filter.test.mjs` | `input[0]?.tagIds` | OK |
| `audit-log.test.mjs` | `actionType: /** @type {number} */ (AUDIT_ACTION_TYPES.CHANNEL_CREATE)` | OK; при `undefined` в рантайме строка не совпадёт — тест упадёт |
| `attachment-validator.test.mjs` | `/** @type {any} */ (isSpoilerFilename)('SPOILER_cat.png', true)` | OK; лишний аргумент по-прежнему передаётся в рантайме |

## 4. Границы изменений

`git status`: изменены **только** 10 файлов `tests/unit/*.test.mjs`.
Продуктовый код, `tsconfig.json`, чужие тесты, `docs/`, `waves/*`, `TASKS.md`, `STATE.md` не тронуты.
`git commit` не выполнялся — соответствует правилам воркера.

Предсуществующий дрейф окружения, **не относящийся к PR**: `node_modules` (симлинк заменён
каталогом, `D` в `git status`) и `package-lock.json` (`bin`/`engines` из `package.json`).
В коммит этого PR попасть не должно — при мерже включать только 10 тестовых файлов.

## 5. Замечания

- `P0` — нет.
- `P1` — нет.
- `P2` — нет.
- `P3` (не блокируют):
  1. Optional chaining в `assert.equal`/`deepEqual` меняет вид падения: вместо `TypeError`
     будет сравнение с `undefined`. Проверка не ослаблена, но диагностика чуть менее явная.
  2. `/** @type {any} */ (isSpoilerFilename)` снимает типовую проверку арности вызова;
     если у функции появится обязательный второй параметр, `tsc` это не заметит.
  3. Снятые `@ts-expect-error` в `markdown-helper.test.mjs` убирают компайл-тайм-страховку
     на нестроковый вход; для `@param {unknown}` она избыточна, поэтому принято.
  4. `package-lock.json` / `node_modules` в рабочем дереве — шум окружения; исключить из мержа.

## 6. Итог

Вердикт: **APPROVED** (`P0 = 0`, `P1 = 0`, мутации `5/5`). PR-162 закрывает оставшиеся
34 ошибки `tsc` в 10 файлах `tests/unit/*.test.mjs` без ослабления проверок и без правок
продуктового кода; 150/150 тестов зелёные, продуктовые модули восстановлены побайтно.
Следующий шаг — `Ready for Merge` в `unstable` с коммитом только 10 тестовых файлов.
