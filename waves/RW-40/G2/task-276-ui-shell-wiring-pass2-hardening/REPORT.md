# REPORT — task-276 (`G2 Frontend Pass 2`: hardening `app-shell-wiring.js` по `REVIEW-184A`)

- **Задача:** `task-276`, dispatch `ctx_75537b9c19c9`, терминал `term_27af47f3-97cc-457e-8c4a-85270f84e0a2`
- **Роль:** `G2 Frontend`, Pass 2 (рефакторинг по замечаниям `REVIEW-184A.md` и `QA-BUILD-003-REPORT.md`)
- **Ветка / HEAD:** `bugfix/task-276-ui-shell-wiring-pass2-hardening` / `3affd78` (`Merge PR-155`)
- **Дата:** 2026-10-31
- **Итог:** `P2-2`, `P2-3`, `P2-5` закрыты кодом и тестами; целевой контракт `15/15` зелёный, регрессия соседей `49/49`, полный unit `1918/1920` (2 падения — предсуществующие, не мои). `tsc` по `ui/tsconfig.json` теперь видит фасад: `0` ошибок в `app-shell-wiring.js`, единственная ошибка — предсуществующая транзитивная `thread-badge.js(75,5)`, её закрывает `task-277`.

## 1. Что изменено

| Файл | MD5 | Строк | Δ |
| --- | --- | ---: | --- |
| `rust/gui-spike/ui/features/app-shell-wiring.js` | `a4c9cb5ea7ddca62eddbdca5ab1788fa` | 291 | было 244, `928cbb029bee1ae75ce4e80336a7a356` |
| `tests/unit/app-shell-wiring.test.mjs` | `5cca3ef80362b924786e92d9382d1baf` | 323 | было 226, `c179f8be3bde983e3f058c827249f39c` |
| `rust/gui-spike/ui/tsconfig.json` | `044b460c1852b37bee299b58ef12bc6e` | 22 | было 17, `include: ["app.js"]` |

MD5 теста одинаков в `red.log` и в `green.log`: red и green прогоны гоняли **один и тот же** файл тестов, менялся только продуктовый код.

## 2. Контракт (Strict TDD, Red → Green)

Четыре требования ТЗ §1 закрыты тестами, каждый — отдельный `test`:

1. `openAdminPanel("invalid_panel")` бросает `Неизвестная панель управления гильдией: invalid_panel`, а два уже открытых слоя (`settings-modal`, `threads-drawer`) остаются в `activeModalStack`.
2. `openModalLayer("invalid_layer")` бросает `Неизвестный слой оболочки UI: invalid_layer`, активная админ-панель `role-editor` остаётся открытой.
3. После `destroy()` `openModalLayer`, `openAdminPanel`, `dispatchAdminAction` бросают `Error`, состояние пустое, `onDispatchIpc` не вызывается ни разу.
4. `getState()` возвращает замороженный объект, `state.activeModalStack` — замороженный массив; попытки записи в объект и `push` в массив дают `TypeError` и не меняют снимок.

Код (`app-shell-wiring.js`):

- `assertAlive(method)` в начале `openModalLayer`, `openAdminPanel`, `dispatchAdminAction` — после `destroy()` фасад не открывает оверлеи (`P2-3`).
- `assertValidModalLayerId(layerId)` по `SHELL_MODAL_LAYERS` **до** `adminRouter.closeActivePanel()`, и `isValidGuildAdminPanel(panelId)` **до** `shellBridge.closeAllLayers()` — опечатка в id больше не сносит открытый оверлей (`P2-2`).
- `getState()` отдаёт `Object.freeze({ activeModalLayer, activeAdminPanel, activeModalStack: Object.freeze([...]) })` (`P2-5`). Поле `activeModalStack` новое в публичном снимке, поэтому пять существующих проверок `getState()` обновлены под расширенный контракт.
- JSDoc `destroy()` описывает новый инвариант: после `destroy()` мутаторы бросают.

## 3. Red

`red.log` — прогон до правки кода: `node --test tests/unit/app-shell-wiring.test.mjs` → `tests 15`, `pass 6`, `fail 9`, exit `1`.

Падения: 4 новых теста плюс 5 существующих, которые сравнивают `getState()` целиком и потому не видят ещё не появившегося `activeModalStack`. Это ожидаемое red-состояние расширяемого контракта, а не поломка поведения.

## 4. Green

| Проверка | Команда | Результат |
| --- | --- | --- |
| Целевой контракт | `node --test tests/unit/app-shell-wiring.test.mjs` | `15/15`, exit `0` |
| Регрессия соседей | `node --test app-shell-wiring + shell-bridge + shell-controllers-registry + chat-helpers-bridge` | `49/49`, `8` suites, exit `0` |
| Полный unit | `node --test 'tests/unit/**/*.test.mjs'` | `1920`, pass `1918`, fail `2`, skip `0` |
| `tsc` UI-проекта | `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | `1` ошибка, обе вне `app-shell-wiring.js` |
| Корневой `tsc` | `npx tsc -p tsconfig.json --noEmit` | `149` ошибок — ровно baseline `REVIEW-184A`, совпадений с `app-shell-wiring` `0` |

Два падения полного unit-прогона предсуществующие и зафиксированы в `REVIEW-184A` §4.3: `tests/unit/dispatch_ui.test.mjs:264` (`A2: FORWARDED_KINDS`) и `tests/unit/test-impact.test.mjs:494`. Счётчик тестов вырос ровно на 4 — на число новых тестов.

Полные логи: `green.log` (§1–§6), `red.log`.

## 5. `tsconfig.json` — что сделано и почему не «0 errors»

`include` стал `["app.js", "features/app-shell-wiring.js", "features/shell-controllers-registry.js", "features/chat-helpers-bridge.js"]`. Гейт теперь реально проверяет фасад: `--listFiles` показывает `features/app-shell-wiring.js`, `shell-bridge.js`, `shell-controllers-registry.js`, `chat-helpers-bridge.js` в программе (до правки модуль не проверялся вообще — `REVIEW-184A` `P2-1`).

Требование ТЗ «`0 errors`» в этой ветке недостижимо без правки чужого файла:

- `include: ["features/**/*.js"]` → **11** предсуществующих ошибок в 6 чужих модулях (`attachment-validator.js`, `inbox.js`, `invite-parser.js`, `svg-icons.js`, `thread-badge.js`, `token-map.js`).
- `include` из ТЗ (три файла + `app.js`) → ровно **1** ошибка: `thread-badge.js(75,5) TS2322` — индекс `THREAD_ARCHIVE_DURATIONS_RU[...]` под `noUncheckedIndexedAccess`, тянется транзитивно через `chat-helpers-bridge.js`. Ошибка предсуществующая, зафиксирована в `REVIEW-184A` §4.4 и `QA-BUILD-003` `P2-1`.
- Ошибок **в самом** `app-shell-wiring.js` — `0`; это и есть прирост покрытия от задачи.

Решение координатора (запрошено через `orca orchestration ask`): чужой файл не трогать, `thread-badge.js` правится параллельно в `task-277`, транзитивную ошибку зафиксировать как известное ограничение. Дополнительно координатор предложил включить `features/composer-popovers-bridge.js`; после уточнения (файла нет на базе `3affd78`, он пришёл в `unstable` позже — `355be21` / `PR-154`) согласован вариант без него.

## 6. Риски и что осталось

- `thread-badge.js(75,5)` держит `tsc` UI-проекта красным, пока `task-277` не смёржен. До этого `gate:worker` на `tsc` по `ui/tsconfig.json` остаётся `exit 1` — но уже без новых ошибок от этой задачи.
- `P2-4` (`onNavigate` объявлен, но недостижим из фасада) не трогал: в ТЗ не входил. `P2-6` (шум `D node_modules`, `M package-lock.json`) — состояние рабочего дерева, не мои правки; в коммит не включать.
- После `destroy()` намеренно не бросают `closeTopLayer`, `closeAdminPanel`, `register*` и `getState` — существующий тест `destroy` ждёт от `closeTopLayer()` `null`, а `getState()` должен оставаться читаемым. Ограничение зафиксировано в JSDoc `destroy`.
- `git commit` не делался. Изменения лежат в рабочем дереве ветки `bugfix/task-276-ui-shell-wiring-pass2-hardening`.
