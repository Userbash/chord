# REPORT — task-280 (`G2`, `FE-FIX-04`, batch1)

## Карточка
- **Batch:** `task-280-unit-tsc-fix-batch1`
- **Ветка:** `bugfix/task-280-unit-tsc-fix-batch1` (от `unstable`, HEAD `8f3bc24`)
- **Приоритет / сложность:** `P0` / `L1`
- **Write-scope:** `tests/unit/profile-modal.test.mjs`, `tests/unit/appearance-settings.test.mjs`, `red.log`, `green.log`, `REPORT.md`

## Сделано
Убраны все 51 ошибка `tsc` в двух тестовых файлах одними JSDoc-приведениями и проверками на `undefined`. Логика `assert.*` не менялась, `@ts-ignore`/`@ts-expect-error` не добавлялись, продуктовый код и DOM-шим `tests/support/gui-dom-harness.mjs` не тронуты.

Разбор по файлам:
- `tests/unit/profile-modal.test.mjs` (29 ошибок → 0):
  - `matches()`: имя атрибута из regex-группы приводится к `string` (`/** @type {string} */ (m[1])`) — группа гарантирована регулярным выражением.
  - Добавлены проверки `assert.ok(...)` после `querySelectorAll` и деструктуризации вкладок — они же сужают тип для `noUncheckedIndexedAccess` (`pills[0]`, `guildItems[0..1]`, `friendItems[0]`, `aboutTab`/`guildsTab`/`friendsTab`).
- `tests/unit/appearance-settings.test.mjs` (22 ошибки → 0):
  - `makeStorage(seed)`: добавлен `@param {string|null} [seed]` (было выведено `null` из дефолта, из-за чего строки-JSON не принимались).
  - `ShimDocument` приводится к `any` на границе вызова `createAppearanceController(/** @type {any} */ (doc), ...)` — мок реализует только используемое подмножество DOM, а не полный `Document`.

## Red
- Команда: `npx tsc -p tsconfig.json --noEmit 2>&1 | grep -E "profile-modal\.test\.mjs|appearance-settings\.test\.mjs"`
- Результат: **51 строка** (29 `profile-modal`, 22 `appearance-settings`) — `red.log`.

## Green
- Та же команда: **0 строк** — `green.log`.
- `node --test tests/unit/profile-modal.test.mjs tests/unit/appearance-settings.test.mjs`: `tests 27, pass 27, fail 0, skipped 0` — `green.log`.
- Полный `tsc` по проекту: **88 ошибок остаются в других файлах**, вне write-scope этого batch (они относятся к следующим карточкам серии; в двух целевых файлах — 0).

## Файлы и контрольные суммы
| Файл | md5 |
| --- | --- |
| `tests/unit/profile-modal.test.mjs` | `33330e8cd38d2aea4f7a7bf2fa64fd9e` |
| `tests/unit/appearance-settings.test.mjs` | `0daf0811e1883af8b07c279c84b50af9` |

Diff: +27 / −19 в двух файлах. `git commit` не выполнялся.

## Риски и ограничения
- `/** @type {any} */` на DOM-моке отключает проверку типов аргумента `document` в этих вызовах; сам мок остаётся типизированным (`doc.documentElement`, `root.getAttribute` проверяются как раньше).
- Остаточные 88 ошибок `tsc` в других файлах не входят в scope и не исправлялись (это отдельные карточки batch-серии).
- В рабочем дереве на момент старта уже были изменения `D node_modules` (в репозитории это tracked-симлинк, в worktree — реальный каталог) и `M package-lock.json` (+6 строк, mtime 11:34:42, до начала правок). Я их не создавал и не трогал; вне write-scope, к приёмке не относятся.
