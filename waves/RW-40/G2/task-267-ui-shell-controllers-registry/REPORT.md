# task-267 / FE-INT-10 — Unified Shell Feature Controllers Registry & Action Router

**Воркер:** G2 Frontend Integration (token-lean)
**Ветка/worktree:** `feature-task-267-ui-shell-controllers-registry`
**Дата:** 2026-10-31
**HEAD на старте:** `bc9f840 feat(ui): merge PR-148 (task-263 voice-sound-effects)`

## Что сделано

1. `rust/gui-spike/ui/features/shell-controllers-registry.js` — новый ESM-модуль с `// @ts-check` и полным JSDoc:
   - `GUILD_ADMIN_PANELS` — `Object.freeze([...])` из 6 панелей в объявленном порядке;
   - `isValidGuildAdminPanel(panelId)` — чистый type-guard предикат (`typeof === "string"` + `includes`);
   - `createGuildAdminPanelRouter(options)` — фабрика с методами `registerPanel`, `openPanel`,
     `closeActivePanel`, `dispatchPanelAction`, `getActivePanel`, `getRegisteredPanels`, `destroy`;
     единственная активная панель, закрытие предыдущей контроллера через `close?.()`, все эффекты вынесены
     в колбэки `onPanelChange` / `onDispatchAction`.
2. `tests/unit/shell-controllers-registry.test.mjs` — 16 тестов в 6 сьютах (TDD red → green).

DOM, IPC и сеть не затрагиваются; внешних зависимостей у модуля нет (нулевой `import`).

## Строгий TDD

| Фаза | Команда | Лог | Итог |
| --- | --- | --- | --- |
| red | `node --test tests/unit/shell-controllers-registry.test.mjs` | `red.log` | exit 1, `ERR_MODULE_NOT_FOUND` на `shell-controllers-registry.js` |
| green | `node --test tests/unit/shell-controllers-registry.test.mjs` | `green.log` | exit 0, `tests 16 / pass 16 / fail 0` |

Red-лог снят до создания продуктового файла; падение зафиксировано на отсутствующем модуле (тот же
формат red-лога, что в принятых batch G2: task-160, task-186).

## Проверки

- `node --test tests/unit/shell-controllers-registry.test.mjs` → 16/16 pass, exit 0.
- `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → exit 0, 0 ошибок.
- `npx tsc -p tsconfig.json --noEmit` → **0 ошибок в моих двух файлах**; в репозитории остаются
  147 pre-existing ошибок в чужих тестах (`svg-icons`, `thread-badge`, `voice-input-mode` и др.),
  не связанных с задачей и не трогавшихся мной.
- `npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict --noUncheckedIndexedAccess ... shell-controllers-registry.js`
  → exit 0 (модуль проходит strict-проверку изолированно, т.к. `ui/tsconfig.json` включает только `app.js`).
- Регрессия соседнего модуля: `node --test tests/unit/shell-bridge.test.mjs` → 7/7 pass.

## Покрытие контракта тестами

- реестр: точный состав 6 панелей, `Object.isFrozen`, запрет `push`;
- предикат: true для всех панелей реестра; false для чужих id, `"role-editor "`, `"ROLE-EDITOR"`, `""`,
  `null`, `undefined`, `0`, `42`, `true`, `{}`, `[]`, `Object.create(null)`, функции;
- регистрация: сортированная копия `getRegisteredPanels()` (внешняя мутация не влияет на реестр),
  отказ на неизвестном `panelId` и на не-объектном контроллере (`TypeError`);
- открытие: payload доходит до `open()`, `onPanelChange(panelId)`, возврат `panelId`;
  предыдущая панель закрывается (`open:a → close:a → open:b`); повторное открытие той же панели
  её не закрывает; открытие без зарегистрированного контроллера работает;
- закрытие: `null` при пустом состоянии без вызова колбэка, `close()`, сброс в `null`, `onPanelChange(null)`,
  повторный вызов ничего не ломает;
- `initialPanel`: валидная панель, `null`, отказ на неизвестной;
- действия: `action.trim()` до колбэка, проброс payload и возврата `onDispatchAction`, отказ на
  неизвестной панели / `""` / `"   "` / не-строке без единого вызова колбэка;
- `destroy()`: `close()` активной, сброс, очистка реестра, повторное открытие после destroy.

## Принятые допущения (в ТЗ не специфицированы)

1. Невалидный `initialPanel` → `Error` (fail-fast), а не молчаливое игнорирование. Причина: остальные
   методы роутера на неизвестный `panelId` бросают, и тихий `null` спрятал бы опечатку в id панели.
2. При переключении панелей `onPanelChange` вызывается только для новой панели (контракт п.3: колбэк
   в `openPanel` — один вызов). Уведомление `null` бывает только из `closeActivePanel()`/`destroy()`.
3. `dispatchPanelAction` возвращает значение `onDispatchAction` (или `undefined`, если колбэк не задан).
4. Повторная `registerPanel` для того же id заменяет контроллер (Map-семантика, как в `shell-bridge.js`).

Если PM/Team Lead ждёт иное поведение в п.1 или п.2 — правка локальная, в одном методе.

## Файлы и контрольные суммы

```
5dac49c8f33ea25694f93625d7f7fd49  rust/gui-spike/ui/features/shell-controllers-registry.js
1a2f84fb41dece94e337739079fba4a1  tests/unit/shell-controllers-registry.test.mjs
```

Строк: 211 (модуль), 314 (тест).

## Риски

- Публичный API нигде не подключён: `app.js`/`dispatch-bridge.js` не входят в write-scope этой карточки,
  интеграция в реальный shell — отдельная задача. `ui/tsconfig.json` (`include: ["app.js"]`) поэтому
  не видит модуль; strict-проверка выполнена отдельным запуском tsc.
- 147 pre-existing ошибок `tsc -p tsconfig.json` в чужих тестах — не мои файлы, не исправлялись.
- `git commit` не выполнялся.

## Артефакты

- `red.log`, `green.log`, `REPORT.md` — в `waves/RW-40/G2/task-267-ui-shell-controllers-registry/`.
