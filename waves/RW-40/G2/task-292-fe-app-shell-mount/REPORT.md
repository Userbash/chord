# REPORT — task-292 (`FE-INT-15`, G2 Frontend L1 / P1)

- **batch/карточка:** `RW-40 / task-292` (`FE-INT-15`: монтирование `createAppShellWiring` и `createComposerPopoversBridge` в `rust/gui-spike/ui/app.js`)
- **ветка:** `feature/task-292-fe-app-shell-mount`
- **рабочее дерево:** `/home/user/orca/workspaces/project/feature-task-292-fe-app-shell-mount`
- **база:** `unstable` @ `d7eff40`

## Сделано

1. `tests/unit/app-shell-wiring.test.mjs` — добавлены 3 теста (после контрактных тестов task-271), читающие исходник `rust/gui-spike/ui/app.js` через `node:fs/promises` `readFile`:
   - импорты `createAppShellWiring` из `./features/app-shell-wiring.js` и `createComposerPopoversBridge` из `./features/composer-popovers-bridge.js`;
   - создание `shellWiring` и `composerPopoversBridge` внутри тела `boot()` (тело вырезается от `function boot(` до `try { app = boot(); }`, вызов каждой фабрики — по балансу скобок) с `onDispatchIpc: (command, payload) => invoke(command, /** @type {Record<string, unknown> | undefined} */ (payload))` и наличие обоих имён в объекте, который возвращает `boot()` (ключи собираются по балансу фигурных скобок);
   - финальный `export { ... }` в конце `app.js` содержит `createUserPopoutController`, `createGuildModalsController`, `createAppShellWiring`, `createComposerPopoversBridge`.
2. `rust/gui-spike/ui/app.js` — минимальная реализация ровно в трёх точках:
   - строки 53–54: два импорта после `./features/guild-modals.js`;
   - в `boot()` перед `return {` (после триггеров `guildModals`): `shellWiring` и `composerPopoversBridge` с `onDispatchIpc`, пробрасывающим в существующий `invoke(cmd, args)`;
   - в возвращаемый объект `boot()` после `guildModals,` добавлены `shellWiring,` и `composerPopoversBridge,`;
   - финальный `export { ... }` расширен двумя фабриками.

## Red

```
$ node --test tests/unit/app-shell-wiring.test.mjs
exit=1
ℹ tests 18
ℹ pass 15
ℹ fail 3
```

Падения (полный лог — `red.log`):
- `app.js: импортирует createAppShellWiring и createComposerPopoversBridge из ./features` — нет обоих импортов;
- `app.js: создаёт shellWiring и composerPopoversBridge внутри boot() с onDispatchIpc -> invoke` — `в boot() не создан shellWiring через createAppShellWiring`;
- `app.js: реэкспортирует ...` — `actual: ['createUserPopoutController','createGuildModalsController']`.

## Green

```
$ node --test tests/unit/app-shell-wiring.test.mjs tests/unit/gui-spike-dom.test.mjs tests/unit/gui-spike-features.test.mjs
exit=0   ℹ tests 189  ℹ pass 189  ℹ fail 0  ℹ skipped 0

$ npx tsc -p tsconfig.json --noEmit
exit=0

$ npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit
exit=0
```

Дополнительно (полный unit-прогон, не обязателен по карточке): `node --test tests/unit` → `exit=0`, `1941/1941 pass`. Полные логи — `green.log`.

## Targeted

`tests/unit/app-shell-wiring.test.mjs` — 18 тестов (15 прежних task-271 + 3 новых), все зелёные; смежные `gui-spike-dom.test.mjs` и `gui-spike-features.test.mjs` — без регрессий (189 вместе).

## Oracle

- Независимый источник: исходник `rust/gui-spike/ui/app.js` (реальный файл, а не импорт модуля), публичные контракты `features/app-shell-wiring.js` (`AppShellWiringOptions.onDispatchIpc`) и `features/composer-popovers-bridge.js` (`createComposerPopoversBridge(options)`).
- Утверждения контракта: импорт из точного пути, создание внутри `boot()` (не на уровне модуля), проброс `onDispatchIpc` в `invoke(command, payload)`, наличие экземпляров в объекте `boot()`, реэкспорт из модуля.

## Генерация

Не применимо (ручных фикстур и генераторов нет).

## Мутации

Не применимо на этапе воркера: `5/5` мутаций выполняет технический ревьюер на приёмке. Тесты red-first и проверяют пять независимых свойств (два импорта, два вызова с `onDispatchIpc`, состав возврата, состав `export`), поэтому точечные мутации этих строк должны падать.

## Baseline / restore

Не применимо (мутации не запускались, продуктовый код не откатывался).

## Стабильность

Повторы не требовались: тесты синхронные/файловые, без сокетов, процессов и гонок. Направленный прогон выполнен дважды (red до реализации, green после) с одинаковым составом тестов; полный unit-прогон — один раз.

## gate:worker

Не запускался: работа идёт в batch-режиме, по `AGENTS.md` §1.3 воркер выполняет targeted-уровень (`T0`), полный `gate:worker` — один раз на batch в integration-лейне. Направленные тесты и оба `tsc` — зелёные.

## Файлы

| файл | md5 |
| --- | --- |
| `rust/gui-spike/ui/app.js` | `a97d997ba0578e184cc491fa4db4b3e9` |
| `tests/unit/app-shell-wiring.test.mjs` | `80ff1ccd1ac198ae4267cf985d5eabdd` |

Изменены только эти 2 файла из ТЗ. `git status` содержит посторонние изменения, пришедшие с базой (`D node_modules` — симлинк в индексе, `M package-lock.json`); воркер их не трогал. `git commit/push` не выполнялись.

## Риски / вопросы

1. `ComposerPopoversBridgeOptions` в `composer-popovers-bridge.js` **не объявляет** `onDispatchIpc` (есть только `sounds`, `commands`, `volume`, `onSelectSlashCommand`, `onPlaySoundboardSound`). Контракт карточки требует прокинуть `onDispatchIpc` в оба моста, поэтому объект опций приведён к `NonNullable<Parameters<typeof createComposerPopoversBridge>[0]> & { onDispatchIpc: ... }`; без приведения `tsc` даёт `TS2353`/`TS2559`. Риск: сейчас опция для моста — no-op (модуль её не читает), реальная диспетчеризация появится, когда `composer-popovers-bridge.js` начнёт её потреблять; расширять контракт моста в этой карточке нельзя (файл вне разрешённых двух).
2. Порядок свойств в возврате `boot()` не проверяется тестом (поведенчески не значим); размещены после `guildModals,` как в ТЗ.
3. `shellWiring`/`composerPopoversBridge` в `boot()` не вызывают `destroy()` при выходе из аккаунта — в карточке такого требования нет, у обоих фасадов пока нет подписок на DOM/IPC.

## Следующий шаг

Передать PM (сверка с ТЗ `FE-INT-15`) и техническому ревьюеру (проверка диффа, 5/5 мутаций по трём новым тестам, оба `tsc`). После приёмки Team Lead синхронизирует `TASKS.md`/`waves/STATE.md`/`db.jsonl`.
