# REVIEW-186A — Техническое ревью PR-157 (`task-276` / `FE-FIX-01`: Pass 2 hardening `app-shell-wiring.js`)

- **Ревьюер:** Технический ревьюер (DeepSeek), TR-186A
- **Проверяемая ветка:** `bugfix/task-276-ui-shell-wiring-pass2-hardening` (база `unstable`, HEAD `3affd78`)
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0` по коду; мутации `5/5`; исходник восстановлен (MD5 `a4c9cb5e…`)
- **Обязательное условие слияния:** сливать **после** PR-277 (`thread-badge.js`), иначе шаг `ui-tsc` в `gate:worker` красный — см. §6, `BLOCKER-1`
- **Следующий статус:** `Ready for Merge` (с учётом порядка слияния) → сливает координатор

## 1. Целостность файлов

| Файл | MD5 в ТЗ | MD5 до ревью | MD5 после мутаций |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/features/app-shell-wiring.js` | `a4c9cb5ea7ddca62eddbdca5ab1788fa` | ✅ совпал | ✅ `a4c9cb5ea7ddca62eddbdca5ab1788fa` |
| `tests/unit/app-shell-wiring.test.mjs` | `5cca3ef80362b924786e92d9382d1baf` | ✅ совпал | ✅ не трогался |
| `rust/gui-spike/ui/tsconfig.json` | `044b460c1852b37bee299b58ef12bc6e` | ✅ совпал | ✅ не трогался |

## 2. Состав изменений

```
 M rust/gui-spike/ui/features/app-shell-wiring.js   +57
 M tests/unit/app-shell-wiring.test.mjs            +113
 M rust/gui-spike/ui/tsconfig.json                   +7 -2
 D node_modules / M package-lock.json               шум рабочего дерева, не PR
```

Файлов вне Write-Scope задачи в диффе нет. `D node_modules` и `M package-lock.json` — состояние дерева, а не правки PR: при слиянии брать только три пути задачи.

## 3. Сверка кода с зависимостями

- `isValidGuildAdminPanel` есть в `shell-controllers-registry.js:74`, чистый предикат без сайд-эффектов — импорт корректный.
- `getActiveOverlayStack` есть в `shell-bridge.js:271`; `getState()` строит `activeModalStack` из него копией `[...]`, то есть наружу не утекает живой массив моста.
- `SHELL_MODAL_LAYERS`/`GUILD_ADMIN_PANELS` — те же замороженные реэкспорты, что и раньше; новых имён и догадок об API нет.
- Потребителей фасада пока нет (`grep "app-shell-wiring"` вне самого модуля и его теста пуст), поэтому новое поле `activeModalStack` и заморозка снимка ничего не ломают.

Три guard'а из ревью `REVIEW-184A` реализованы ровно по замечаниям:

| Замечание | Реализация |
| --- | --- |
| `P2-2` — валидация до побочных эффектов | `openModalLayer` → `assertValidModalLayerId` до `closeActivePanel()`; `openAdminPanel` → `isValidGuildAdminPanel` до `closeAllLayers()` (`:158–192`) |
| `P2-3` — после `destroy()` фасад непригоден | `assertAlive(method)` в `openModalLayer`, `openAdminPanel`, `dispatchAdminAction` (`:116–120`) |
| `P2-5` — заморозка `getState()` не покрыта тестом | `Object.freeze` объекта **и** вложенного массива (`:253–259`) + тест на `TypeError` при записи |

`P2-4` (`onNavigate` недостижим) не трогался — в ТЗ не входил, остаётся открытым.

## 4. Тесты

```
node --test tests/unit/app-shell-wiring.test.mjs
ℹ tests 15  ℹ pass 15  ℹ fail 0  ℹ skipped 0
```

Соседние модули (регрессия): `app-shell-wiring` + `shell-bridge` + `shell-controllers-registry` + `chat-helpers-bridge` → `49/49 pass`.

TDD-дисциплина подтверждена по артефактам: `red.log` снят на исходнике (`app-shell-wiring.js` MD5 `928cbb02…`, 19 падений), `green.log` — уже на финальных MD5.

## 5. Мутации: 5/5 убиты

Харнесс: `/tmp/opencode/mut186a/mutate.py`; бэкап исходника в `/tmp/opencode/mut186a/app-shell-wiring.orig.js`, восстановление после каждой мутации с проверкой MD5. Логи — `/tmp/opencode/mut186a/M1..M5.log`, сводка — `results.json`.

| # | Мутация | MD5 мутанта | exit | Итог | Убивший тест |
| --- | --- | --- | --- | --- | --- |
| M1 | `openAdminPanel`: `closeAllLayers()` **до** валидации панели | `b5779f0724faf7c531f565a853aaf445` | 1 | убита | `openAdminPanel: невалидный id не снимает уже открытые модальные слои` |
| M2 | `openModalLayer`: `closeActivePanel()` **до** валидации слоя | `018187ef97f14b1dca7d7b2494cf6f45` | 1 | убита | `openModalLayer: невалидный id не закрывает активную админ-панель` |
| M3 | `destroyed` guard отключён (`if (destroyed)` → `if (false)`) | `0210896dae90f31354637dc0fe092e39` | 1 | убита | `после destroy() мутаторы бросают Error и оверлеи не открываются` |
| M4 | `getState()` возвращает незамороженный объект | `30e3d00bcf60be238e8bacd26a305697` | 1 | убита | `getState: снимок и вложенный activeModalStack заморожены` |
| M5 | `getState()`: вложенный `activeModalStack` без `Object.freeze` | `15c46dc966793a0a63320acf5e8f07c2` | 1 | убита | тот же тест (падение на `push`) |

**Итого `5/5`.** Все три новых guard'а защищены тестами: мутант `M5` (заморозка только объекта) тоже убит, то есть тест проверяет именно вложенный массив, а не только внешний `Object.isFrozen`.

Контроль после цикла: `md5 = a4c9cb5ea7ddca62eddbdca5ab1788fa`, целевой прогон `15/15 pass`, exit `0`, `git status` вернулся к состоянию §2.

## 6. Замечания

### BLOCKER-1 (порядок слияния, не дефект кода PR). `ui-tsc` красный до слияния `task-277`

Расширение `include` в `ui/tsconfig.json` заставило `tsc` проверять фасад, а через `chat-helpers-bridge.js` он тянет транзитивный `thread-badge.js`, где живёт предсуществующая ошибка `thread-badge.js(75,5) TS2322`. Замеры:

| Состояние | `npx tsc -p rust/gui-spike/ui/tsconfig.json` |
| --- | --- |
| `HEAD` (`include: ["app.js"]`) | exit `0` |
| PR-157 как есть | exit `1` — 1 ошибка, `thread-badge.js(75,5)` |
| PR-157 + правка `thread-badge.js` из ветки `task-277` | exit `0` |

`gate:worker` выполняет ровно эту команду (`scripts/gate.mjs:240`, шаг `ui-tsc`), поэтому при слиянии PR-157 **раньше** PR-277 гейт на `unstable` станет красным для всех. Ошибка не в коде PR-157: из `app-shell-wiring.js` диагностик ноль, а `thread-badge.js` вне его Write-Scope и уже закрыт карточкой `task-277` (`P0`, правка готова в её рабочем дереве, но не закоммичена). **Условие:** сливать PR-157 после PR-277; отдельно чинить `thread-badge.js` внутри PR-157 не нужно и нельзя — файл чужой.

Это же объясняет отклонение от буквы ТЗ §2 (`tsc` должен быть с `0 errors`): воркер честно зафиксировал `exit 1` и зависимость в `REPORT.md` §5, а не подогнал `include` под зелёный результат.

### P2-1. Корневой `tsc` остаётся красным по baseline
`npx tsc -p tsconfig.json` даёт `149` ошибок — столько же, сколько на `unstable` (`REVIEW-184A` §4.4). Новых диагностик от файлов задачи нет.

### P2-2. `P2-4` из `REVIEW-184A` не закрыт
`onNavigate` по-прежнему прокидывается в мост, но наружу не экспонируется. В ТЗ не входило — оставить на задачу подключения в `app.js`.

### P2-3. Шум рабочего дерева
`D node_modules`, `M package-lock.json` — не часть PR (§2). При слиянии `git add` только по трём путям.

## 7. Соответствие DoD

| Критерий | Статус |
| --- | --- |
| MD5 файлов совпадают с ТЗ | ✅ |
| Изменения в пределах Write-Scope | ✅ |
| Целевой контракт | ✅ `15/15` |
| Регрессия соседей | ✅ `49/49` |
| Новые ошибки типов от кода задачи | ✅ `0` |
| Мутации `5/5` | ✅ |
| Исходник восстановлен | ✅ `a4c9cb5ea7ddca62eddbdca5ab1788fa` |
| `git commit` не делался | ✅ |
| `P0 = 0`, `P1 = 0` по коду | ✅ |
| `gate:worker` (`ui-tsc`) зелёный как есть | ⚠️ требует PR-277 первым (`BLOCKER-1`) |

## 8. Артефакты

```
waves/RW-40/G2/task-276-ui-shell-wiring-pass2-hardening/REVIEW-186A.md   — этот отчёт
/tmp/opencode/mut186a/mutate.py          — харнесс мутаций с восстановлением
/tmp/opencode/mut186a/M1.log … M5.log    — логи мутаций
/tmp/opencode/mut186a/results.json       — сводка: MD5, exit, убившие тесты
/tmp/opencode/mut186a/post-restore.log   — контрольный прогон после восстановления
```

**Вердикт: APPROVED.** Код и тесты PR-157 замечаний `P0`/`P1` не имеют; слияние в `unstable` — только после PR-277, иначе `gate:worker` падает на `ui-tsc`.
