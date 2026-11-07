# task-248 — Guild Role Management & Permission Matrix Editor Controller (FE-P0-12)

## Сделано
- `rust/gui-spike/ui/features/role-editor.js` (`// @ts-check`):
  - Экспортирован замороженный каталог прав `ROLE_PERMISSIONS_CATALOG_RU = Object.freeze([...])` (6 прав: `ADMINISTRATOR` (bit `8`, `"Администратор"`), `MANAGE_GUILD` (bit `32`, `"Управление сервером"`), `MANAGE_ROLES` (bit `268435456`, `"Управление ролями"`), `MANAGE_CHANNELS` (bit `16`, `"Управление каналами"`), `KICK_MEMBERS` (bit `2`, `"Выгонять участников"`), `BAN_MEMBERS` (bit `4`, `"Банить участников"`)).
  - Экспортирована чистая функция `sortRolesByPositionDesc(roles)`: сортирует роли по убыванию `position`, а при равенстве — по возрастанию `String(id)` без мутации входного массива и его объектов.
  - Экспортирован контроллер `createRoleEditorController(options)`:
    - рендерит регион `[data-role-editor]` с `role="region"` и `aria-label="Редактор ролей"`;
    - рендерит боковой список ролей `[data-role-item-id="<id>"]`, отсортированный через `sortRolesByPositionDesc`, с выбором роли по клику;
    - для выбранной роли рендерит поля `[data-role-name-input]`, `[data-role-color-input]`, переключатель `[data-role-hoist-toggle]` (`role="switch"`, `aria-checked="true"|"false"`) и переключатели матрицы прав `[data-role-perm-toggle="<key>"]` (`role="switch"`, `aria-checked="true"|"false"`, клик инвертирует соответствующий бит в черновике `permissions`);
    - рендерит кнопку `[data-role-save-btn]` (`"Сохранить изменения"`), которая вызывает `onSaveRole({ id, name, color, hoist, permissions })` с обрезанным по краям `name`, если длина `name.trim()` находится в диапазоне `1..100`;
    - возвращает `{ selectRole(roleId), setState(patch), getState(), destroy() }`.
- `tests/unit/role-editor.test.mjs`: 8 unit-тестов по контракту (Strict TDD: Red -> Green).

## Прогоны
- **Red**: `node --test tests/unit/role-editor.test.mjs` -> `exit=1` (`ERR_MODULE_NOT_FOUND`, `pass 0 / fail 1`) — лог сохранён в `red.log`.
- **Green**: `node --test tests/unit/role-editor.test.mjs` -> `exit=0` (`tests 8 / pass 8 / fail 0`) — лог сохранён в `green.log`.
- **Typecheck**: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` -> `exit=0` (0 ошибок).

## Файлы и md5
- `rust/gui-spike/ui/features/role-editor.js` — `ea1895e9d69862d360cab7a9642ddf81`
- `tests/unit/role-editor.test.mjs` — `0ae566c525758d9e16e134bd6f81f788`
- `/home/sanya/orca/project/waves/RW-40/G2/task-248-ui-role-editor/red.log` — `21b492ce325d32f9219a2eb2907005f5`
- `/home/sanya/orca/project/waves/RW-40/G2/task-248-ui-role-editor/green.log` — `da59374a0dd2f3da6351ea06a496679c`

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера; изменения находятся в рабочем дереве ветки `feature/task-248-ui-role-editor`.
