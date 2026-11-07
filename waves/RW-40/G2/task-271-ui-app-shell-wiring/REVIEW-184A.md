# REVIEW-184A — Техническое ревью PR-155 (`task-271` / `FE-INT-14`: `app-shell-wiring.js`)

- **Ревьюер:** Технический ревьюер (DeepSeek), TR-184A
- **Проверяемая ветка:** `feature/task-271-ui-app-shell-wiring` (база `unstable`, HEAD `b35c3e1`)
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED** — `P0 = 0`, `P1 = 0`, мутации `5/5` убиты, исходный файл восстановлен (MD5 совпадает)
- **Следующий статус:** `Ready for Merge` → слияние в `unstable` выполняет координатор

## 1. Объект ревью и целостность файлов

| Файл | MD5 в ТЗ | MD5 фактически | Строк |
| --- | --- | --- | --- |
| `rust/gui-spike/ui/features/app-shell-wiring.js` | `928cbb029bee1ae75ce4e80336a7a356` | `928cbb029bee1ae75ce4e80336a7a356` ✅ | 244 |
| `tests/unit/app-shell-wiring.test.mjs` | `c179f8be3bde983e3f058c827249f39c` | `c179f8be3bde983e3f058c827249f39c` ✅ | 226 |

MD5 сняты дважды: до начала проверки и после мутационного цикла. Оба значения совпали с ТЗ, то есть в репозитории лежит ровно тот код, который принимали PM.

## 2. Проверка состава изменений (`git status --short`)

```
 D node_modules
 M package-lock.json
?? rust/gui-spike/ui/features/app-shell-wiring.js
?? tests/unit/app-shell-wiring.test.mjs
```

- `??` — два deliverable-файла задачи, оба в scope PR-155, оба новые.
- `D node_modules` и `M package-lock.json` — **не часть PR-155**. В `unstable` `node_modules` хранится как симлинк `120000 -> /home/sanya/orca/project/node_modules`, в рабочем дереве он развёрнут в пустой каталог; `package-lock.json` рассинхронизирован с `package.json` (`bin`, `engines`), потому что lock в git устарел, а `npm install` его перегенерировал. Это описано в §6 как `P2-4`, в коммит попасть не должно.
- Изменений в чужих модулях (`shell-bridge.js`, `shell-controllers-registry.js`, `chat-helpers-bridge.js`), в `app.js`, в `docs/`, в `waves/` нет. `git diff --name-status unstable` показывает только два перечисленных шумовых пути, новых отслеживаемых правок нет.

## 3. Функциональная сверка с зависимостями

Фасад собран из трёх реально существующих модулей; API-догадок нет.

- `createShellFeatureBridge({ onDispatchIpc, onNavigate })`, `SHELL_MODAL_LAYERS`, `openLayer`, `closeTopLayer`, `closeAllLayers`, `getActiveOverlayStack`, `registerController` — все имена есть в `shell-bridge.js` (284 строки, `unstable`).
- `createGuildAdminPanelRouter({ onPanelChange, onDispatchAction })`, `openPanel`, `closeActivePanel`, `getActivePanel`, `dispatchPanelAction`, `registerPanel`, `destroy`, `GUILD_ADMIN_PANELS` — все имена есть в `shell-controllers-registry.js`.
- `buildChannelHeaderBadgesModel`, `formatChannelTopicTooltip` — есть в `chat-helpers-bridge.js`; лимит тултипа 160 символов и многоточие `…` совпадают с реализацией (`formatChannelTopicTooltip`, §130–139).
- Взаимное исключение работает в обе стороны: `openModalLayer` → `adminRouter.closeActivePanel()`, `openAdminPanel` → `shellBridge.closeAllLayers()`. Это закрывает риск «двух оверлеев одновременно», который заявлен в шапке модуля.
- Реэкспорт `GUILD_ADMIN_PANELS` и `SHELL_MODAL_LAYERS` даёт `app.js` одну точку импорта констант.

Отклонения от первого ТЗ (`closeAllLayers` вместо несуществующего `closeAll()`, `getActiveOverlayStack()` вместо `getTopLayer()`, добавленный `dispatchAdminAction`, реэкспорт констант) подтверждены PM в `ACCEPT.md` и подтверждаются кодом зависимостей: названных в первом ТЗ методов в `shell-bridge.js` нет.

## 4. Тесты и типы

### 4.1 Контрактные тесты

```
node --test tests/unit/app-shell-wiring.test.mjs
ℹ tests 11  ℹ pass 11  ℹ fail 0  ℹ cancelled 0  ℹ skipped 0
```

Покрыты обе стороны взаимного исключения, LIFO-порядок закрытия, хуки `open/close` зарегистрированных контроллеров, маршрутизация `guild_admin:<panel>:<action>` вместе с обрезкой `action`, модель заголовка канала (`99+`, `"1 час"`, обрезка темы до 160 с `…`, `topicMaxChars`), валидация id и идемпотентный `destroy`.

### 4.2 Соседние модули (регрессия)

```
node --test tests/unit/app-shell-wiring.test.mjs tests/unit/shell-bridge.test.mjs \
  tests/unit/shell-controllers-registry.test.mjs tests/unit/chat-helpers-bridge.test.mjs
ℹ tests 45  ℹ pass 45  ℹ fail 0
```

### 4.3 Полный unit-прогон

`node --test 'tests/unit/**/*.test.mjs'` → `1916` тестов, `1914` pass, `2` fail. Падения — в `tests/unit/dispatch_ui.test.mjs` (`A2: FORWARDED_KINDS — 21 имя DispatchKind::as_str…`) и `tests/unit/test-impact.test.mjs` (`test-impact v2: точечный граф Rust BFF`, `каждый pub mod из lib.rs покрыт правилом`).

Проверил, что это не регрессия PR-155: при **временно убранных** файлах задачи (оба перенесены в `/tmp`, затем возвращены) те же два файла дают те же `2 fail / 108 pass`. Логи: `mutations/unit-full.log`. Долг предсуществующий, к task-271 отношения не имеет.

### 4.4 Типы

| Проверка | Результат |
| --- | --- |
| `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | exit `0`, ошибок нет |
| `npx tsc -p tsconfig.json --noEmit` (корневой, с `tests/**`) | exit `1`, `149` ошибок `error TS` |
| То же без файлов задачи (baseline) | exit `1`, `149` ошибок, **лог побайтово совпадает** (`705f8a2d…` с файлами и без) |
| Прямая проверка модуля `--ignoreConfig --strict --noUncheckedIndexedAccess` | единственная ошибка — предсуществующая `thread-badge.js(75,5) TS2322` в транзитивной зависимости |

Вывод: PR-155 не добавляет ни одной диагностики типов. Все `149` ошибок корневого `tsc` живут в чужих файлах (`profile-modal.test.mjs`, `svg-icons.test.mjs`, `appearance-settings.test.mjs` и др.) и присутствуют на базе.

**Важная оговорка по гейту (§6, `P2-1`):** `rust/gui-spike/ui/tsconfig.json` содержит `"include": ["app.js"]`, а `app.js` в этой ветке **не импортирует** ни `app-shell-wiring.js`, ни `shell-bridge.js`, ни `shell-controllers-registry.js`, ни `chat-helpers-bridge.js`. Значит предписанный в ТЗ `tsc` по этому конфигу физически не проверяет новый модуль и его `exit 0` сам по себе ничего не доказывает. Реальную проверку типов дал прогон из таблицы выше (строка 4). `tests/unit/app-shell-wiring.test.mjs` при этом попадает в корневой `tsconfig.json` (`tests/**/*.mjs`) и ошибок не даёт.

## 5. Мутации: 5/5 убиты

Методика: перед циклом исходник скопирован в `/tmp/opencode/mut271/app-shell-wiring.orig.js`; каждая мутация применялась точной заменой одной конструкции, проверялось, что MD5 мутанта отличается от исходного; затем `node --test tests/unit/app-shell-wiring.test.mjs`; после каждой мутации файл восстанавливался из бэкапа с проверкой MD5. Скрипт: `mutations/mutate-184a.py`.

| # | Мутация | MD5 мутанта | exit | Итог | Убивший тест |
| --- | --- | --- | --- | --- | --- |
| M1 | `openModalLayer` не вызывает `adminRouter.closeActivePanel()` | `4f0ccd18dcaa9426849c15967438c718` | 1 | убита | `openModalLayer: закрывает активную админ-панель и уведомляет об этом` |
| M2 | `openAdminPanel` не вызывает `shellBridge.closeAllLayers()` | `f63aca1a88b431c8288e0da72e203947` | 1 | убита | `openAdminPanel: закрывает уже открытые модальные слои…`, `closeTopLayer: при открытой админ-панели…` (`2 fail`) |
| M3 | `closeTopLayer` теряет ветку приоритета админ-панели | `b51eabf408540dadbe406c916651cc36` | 1 | убита | `closeTopLayer: при открытой админ-панели закрывает именно её…` |
| M4 | IPC-префикс искажён: `guild_admin:` → `guild_admin-` | `806a1d901837808567f6e05d5b77f728` | 1 | убита | `dispatchAdminAction: уходит в onDispatchIpc как guild_admin:<panel>:<action>…` |
| M5 | `destroy` не вызывает `shellBridge.closeAllLayers()` | `f238ba6ef0c9c0594f7e526ea802fcd1` | 1 | убита | `destroy: закрывает панель и все слои, обнуляет состояние и идемпотентен` |

**Итого: `5/5` (100%).** Мутантов, выживших среди обязательных пяти, нет.

Восстановление после цикла:

```
928cbb029bee1ae75ce4e80336a7a356  rust/gui-spike/ui/features/app-shell-wiring.js   (совпадает с исходным)
c179f8be3bde983e3f058c827249f39c  tests/unit/app-shell-wiring.test.mjs             (не трогался)
```

Контрольный прогон после восстановления: `11/11 pass`, exit `0`. `git status --short` вернулся к состоянию из §2 — лишних правок не осталось.

Логи по каждой мутации: `mutations/M1.log` … `mutations/M5.log`, сводка — `mutations/results.json`.

## 6. Замечания

Блокирующих нет. Ниже — `P2`, то есть рекомендации, а не условия слияния.

### P2-1. `tsc`-гейт не покрывает новый модуль
`rust/gui-spike/ui/tsconfig.json` → `include: ["app.js"]`, а `app.js` не импортирует фасад. Пока задача подключения в `app.js` не сделана, регрессию типов в `app-shell-wiring.js` поймает только ручной per-file прогон. **Рекомендация:** в задаче подключения фасада в `app.js` первым шагом добавить импорт, чтобы гейт начал видеть модуль; либо добавить в `ui/tsconfig.json` `include: ["features/app-shell-wiring.js", "app.js"]`. Сейчас риск не в коде PR, а в том, что зелёный `tsc` создаёт ложное чувство проверки.

### P2-2. Побочный эффект срабатывает раньше валидации id
Проверено на живом модуле (`/tmp/opencode/mut271/probe.mjs`):

```
a.openAdminPanel("role-editor");
a.openModalLayer("не-слой");   // throws: Неизвестный слой оболочки UI: не-слой
→ состояние после: {"activeModalLayer":null,"activeAdminPanel":null}   // панель уже закрыта

b.openModalLayer("settings-modal");
b.openAdminPanel("не-панель"); // throws: Неизвестная панель управления гильдией: не-панель
→ состояние после: {"activeModalLayer":null,"activeAdminPanel":null}   // слой уже снят
```

То есть опечатка в id сначала сносит текущий оверлей, и только потом летит исключение. Тест `валидация идентификаторов делегируется реестрам…` проверяет только факт `throws`, но не сохранность состояния. **Рекомендация:** валидировать id до мутаций (`SHELL_MODAL_LAYERS.includes(layerId)` / `isValidGuildAdminPanel(panelId)` в начале метода) и добавить тест «неверный id не меняет состояние». Сейчас не блокирует: все id берутся из реэкспортированных замороженных списков, а не из пользовательского ввода.

### P2-3. После `destroy()` фасад снова открывает оверлеи
Флаг `destroyed` защищает только повторный `destroy`. Проверено:

```
c.destroy(); c.openAdminPanel("role-editor"); c.openModalLayer("search-panel");
→ {"activeModalLayer":"search-panel","activeAdminPanel":null}
```

Для фасада, который `app.js` вызывает на teardown, ожидаемо, что после `destroy` методы-мутаторы либо бросают, либо ничего не делают. **Рекомендация:** либо документировать «после `destroy` фасад непригоден» в JSDoc `destroy`, либо добавить guard `if (destroyed) return null;` во все мутаторы и тест на это.

### P2-4. `onNavigate` объявлен, но недостижим из фасада
`createAppShellWiring({ onNavigate, … })` прокидывает колбэк в `shellBridge`, но фасад не отдаёт наружу ни `navigate`, ни `dispatchIpc` (`grep "\.navigate\|dispatchIpc"` по фасаду пуст). Проверено: при `onNavigate` ни один вызов фасада колбэк не вызывает. Потребитель, который передаст `onNavigate` и будет ждать навигации через фасад, получит тишину. **Рекомендация:** до подключения в `app.js` либо экспонировать `navigate(target)`, либо убрать `onNavigate` из `AppShellWiringOptions`, чтобы контракт не обещал несуществующего.

### P2-5. Заморозка снимка `getState()` не покрыта тестом
Дополнительная исследовательская мутация (вне обязательных пяти): убрал `Object.freeze` в `getState()` — тесты остались зелёными (`11/11 pass`), то есть мутант выжил. Сама заморозка в коде есть (`Object.isFrozen(getState()) === true`), но контракт не защищён тестом. **Рекомендация:** одна строка `assert.equal(Object.isFrozen(wiring.getState()), true)` в существующем тесте `getState`.

### P2-6. Шум в рабочем дереве, не входящий в PR
`D node_modules`, `M package-lock.json` — см. §2. **Рекомендация:** координатору при слиянии брать только два файла задачи (`git add` по путям), не `git add -A`. Отдельно стоит починить lock: расхождение `package.json` ↔ `package-lock.json` по `bin`/`engines` существует на `unstable` и всплывёт на CI с `npm ci`.

## 7. Соответствие DoD

| Критерий | Статус |
| --- | --- |
| Файлы соответствуют заявленным MD5 | ✅ |
| Изменения не выходят за scope задачи | ✅ (шум `node_modules`/`package-lock` — не из PR, см. `P2-6`) |
| Контрактные тесты зелёные | ✅ `11/11` |
| Регрессия соседей | ✅ `45/45` |
| Типы | ✅ новых ошибок `0` (149 предсуществующих, baseline идентичен) |
| Мутации `5/5` убиты | ✅ |
| Исходник восстановлен, MD5 совпадает | ✅ `928cbb029bee1ae75ce4e80336a7a356` |
| `git commit` не делался | ✅ |
| `P0 = 0`, `P1 = 0` | ✅ |

## 8. Артефакты

```
waves/RW-40/G2/task-271-ui-app-shell-wiring/REVIEW-184A.md          — этот отчёт
waves/RW-40/G2/task-271-ui-app-shell-wiring/mutations/M1.log … M5.log
waves/RW-40/G2/task-271-ui-app-shell-wiring/mutations/results.json  — сводка мутаций и MD5
waves/RW-40/G2/task-271-ui-app-shell-wiring/mutations/mutate-184a.py — харнесс с восстановлением
waves/RW-40/G2/task-271-ui-app-shell-wiring/mutations/tsc-root-with-files.log
waves/RW-40/G2/task-271-ui-app-shell-wiring/mutations/tsc-root-baseline.log
waves/RW-40/G2/task-271-ui-app-shell-wiring/mutations/unit-full.log
```

**Вердикт: APPROVED.** Замечания `P2-1…P2-6` — рекомендации для задач подключения в `app.js` и гигиены ветки; слияние в `unstable` ими не блокируется.
