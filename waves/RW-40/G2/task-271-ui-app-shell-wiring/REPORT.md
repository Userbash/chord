# task-271 (FE-INT-14) — Top-Level App Shell Wiring Facade

Отдел: G2 Frontend Integration. Режим: token-lean.
Ветка: `feature/task-271-ui-app-shell-wiring`, база — `unstable`.

## Что сделано

`rust/gui-spike/ui/features/app-shell-wiring.js` — фасад, сводящий три соседних
модуля в одну точку сборки:

| Импорт | Откуда |
| --- | --- |
| `createShellFeatureBridge`, `SHELL_MODAL_LAYERS` | `./shell-bridge.js` |
| `createGuildAdminPanelRouter`, `GUILD_ADMIN_PANELS` | `./shell-controllers-registry.js` |
| `buildChannelHeaderBadgesModel`, `formatChannelTopicTooltip` | `./chat-helpers-bridge.js` |

`createAppShellWiring({ onDispatchIpc, onNavigate, onAdminPanelChange })` создаёт
внутри `shellBridge` и `adminRouter` (`onDispatchAction` панели уходит в IPC как
`guild_admin:<panelId>:<action>`) и держит два уровня оверлеев во взаимном
исключении:

- `openAdminPanel()` → `shellBridge.closeAllLayers()` + `adminRouter.openPanel()`;
- `openModalLayer()` → `adminRouter.closeActivePanel()` + `shellBridge.openLayer()`;
- `closeTopLayer()` → сначала активная админ-панель, иначе верхний модальный слой.

Публичный контракт: `openModalLayer`, `closeTopLayer`, `openAdminPanel`,
`closeAdminPanel`, `registerModalController`, `registerAdminController`,
`dispatchAdminAction`, `getChannelHeaderModel`, `getState`, `destroy`.
Модуль не трогает DOM, IPC и сеть — все побочные эффекты идут в колбэки.

`tests/unit/app-shell-wiring.test.mjs` — 11 тестов на публичное поведение
(взаимное исключение в обе стороны, LIFO-закрытие, хуки контроллеров,
маршрутизация `guild_admin:*`, модель заголовка канала, валидация id,
идемпотентный `destroy`).

## Отклонения от ТЗ (осознанные, требуют подтверждения Team Lead)

1. В ТЗ указаны `shellBridge.closeAll()` и `shellBridge.getTopLayer()` — таких
   методов в `shell-bridge.js` нет. Использованы фактические:
   `closeAllLayers()` и последний элемент `getActiveOverlayStack()`.
2. Добавлен метод `dispatchAdminAction(panelId, action, payload)` — без него
   фабричный колбэк `onDispatchAction`, требуемый ТЗ, недостижим и не тестируется.
3. Добавлен ре-экспорт `SHELL_MODAL_LAYERS` и `GUILD_ADMIN_PANELS`, чтобы
   `app.js` брал константы из одного места (импорт этих имён требует ТЗ).

## Strict TDD

Red: `node --test tests/unit/app-shell-wiring.test.mjs` → `ERR_MODULE_NOT_FOUND`
для `app-shell-wiring.js`, exit 1 (`red.log`, 0 pass / 1 fail). Лог снят на
финальной версии тестов: модуль временно убирался из рабочего дерева, тесты не
подгонялись под код.

Green: тот же прогон → 11/11 pass, exit 0 (`green.log`).

Регрессия соседей: `node --test` по `app-shell-wiring`, `shell-bridge`,
`shell-controllers-registry`, `chat-helpers-bridge` → 45 тестов, 45 pass, 0 fail.

Типы: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → 0 ошибок.
Прямая проверка файлов (`--ignoreConfig ... --strict --noUncheckedIndexedAccess`)
даёт 0 ошибок в моих двух файлах; единственная ошибка — предсуществующая
`thread-badge.js(75,5) TS2322` (чужой модуль, в scope проекта `tsconfig` не входит,
`app.js` его не импортирует через проверяемый граф).

## Файлы и md5

```
928cbb029bee1ae75ce4e80336a7a356  rust/gui-spike/ui/features/app-shell-wiring.js  (244 строки)
c179f8be3bde983e3f058c827249f39c  tests/unit/app-shell-wiring.test.mjs            (226 строк)
9f3d72aca80ceb6fcf7b8957bbaf7868  waves/RW-40/G2/task-271-ui-app-shell-wiring/red.log
7b6bab0b3a56dca6d18ec96ec68c6982  waves/RW-40/G2/task-271-ui-app-shell-wiring/green.log
```

## Риски и что осталось

- Фасад пока никем не импортируется: подключение в `app.js` — отдельная задача
  (файл `app.js` вне моего write-scope).
- Mutations 5/5 не прогонялись: это уровень Technical Reviewer.
- `git commit` не делался, ветка в `unstable` не мержилась — по регламенту
  слияние за координатором.
