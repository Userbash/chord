# REVIEW — TR-183A / PR-154 / task-270 (`FE-INT-13`: `composer-popovers-bridge.js`)

- **Ревьюер:** Технический ревьюер (DeepSeek, TR-183A)
- **Ветка:** `feature/task-270-ui-composer-popovers-bridge` (HEAD `b35c3e1`)
- **Дата:** 2026-10-31
- **Вердикт:** **APPROVED**
- **Дефекты:** `P0=0`, `P1=0`, `P2=1`, `P3=4`
- **Мутации:** **5/5 убиты**, исходник восстановлен, MD5 совпал с эталоном

## 1. Проверяемые файлы

| Файл | MD5 (эталон ТЗ) | MD5 (факт) | Статус |
|---|---|---|---|
| `rust/gui-spike/ui/features/composer-popovers-bridge.js` | `0d5ae58d106cc500335195b82e1e2e77` | `0d5ae58d106cc500335195b82e1e2e77` | совпал |
| `tests/unit/composer-popovers-bridge.test.mjs` | `7e3c3edefd86fd58c9b4b117d4d80be1` | `7e3c3edefd86fd58c9b4b117d4d80be1` | совпал |

## 2. Scope и `git status`

`git status --short` в рабочей ветке:

```
 D node_modules
 M package-lock.json
?? rust/gui-spike/ui/features/composer-popovers-bridge.js
?? tests/unit/composer-popovers-bridge.test.mjs
```

PR добавляет ровно два файла из ТЗ, других правок кода нет. Две шумовые записи — состояние окружения, а не работа воркера:

- `D node_modules`: в индексе это симлинк (`120000`, `HEAD:node_modules` = `/home/user/orca/project/node_modules`), в рабочем дереве по этому пути лежит реальный каталог `node_modules` (149 МБ). `git` видит typechange «симлинк → каталог» и показывает удаление. Та же картина в деревьях `task-269`, `task-274`; в `qa-task-129`, `qa-task-132`, `re-task-130`, `re-task-131` симлинк на месте, но `package-lock.json` тоже изменён.
- `M package-lock.json`: к лок-файлу добавлены `bin.discord-lab` и `engines.node >=24`, которые уже есть в `package.json` (`bin`, `engines`), то есть локфайл просто догоняет манифест после `npm install`. Тот же диф `+6` строк висит в шести деревьях: `qa-task-129`, `qa-task-132`, `re-task-130`, `re-task-131`, `task-269`, `task-274`.

Вывод: посторонних изменений, внесённых PR-154, нет; `package-lock.json` и симлинк `node_modules` при merge не коммитить.

## 3. Прогоны

| Проверка | Команда | Результат |
|---|---|---|
| Контрактные тесты | `node --test tests/unit/composer-popovers-bridge.test.mjs` | `tests 11, suites 3, pass 11, fail 0`, exit `0` |
| Typecheck | `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` | 0 ошибок, exit `0` |

Повторный прогон тестов и `tsc` после всех мутаций дал те же результаты: `11/11` pass, `tsc` exit `0`.

## 4. Мутационное тестирование (5 мутаций, восстановление после каждой)

Каждая мутация применялась точечной заменой в `composer-popovers-bridge.js`, файл прогонялся unit-тестом, затем восстанавливался из исходного содержимого с проверкой MD5. Красные логи — `mutations/M<ID>-red.log`, машинный протокол — `mutations/mutations.json`.

| ID | Мутация | MD5 мутанта | Exit | Итог | Убившие тесты |
|---|---|---|---|---|---|
| M1 | `commandName: parsed.commandName` → `commandName: parsed.rawArgs` | `b9fd67ad2218a48351113ca3296c40f6` | 1 | **убита** | `resolveComposerDraftPopover` → «активирует slash-command-picker и извлекает commandName и rawArgs…»; `updateDraft` → «активирует slash-command-picker… и фильтрует команды по commandName» |
| M2 | в `updateDraft` фильтр команд игнорирует `commandName` (`filterSlashCommands(commands, "")`) | `0436d52d9a9af1de05e823b8372ac3a2` | 1 | **убита** | `updateDraft` → «активирует slash-command-picker… и фильтрует команды по commandName» |
| M3 | инверсия guard в `openPopover` (`COMPOSER_POPOVER_LAYERS.includes(...)` без отрицания) | `ece75909ef928db86f46cb1d2f4889db` | 1 | **убита** | «openPopover открывает допустимые слои…»; «openPopover бросает Error при неизвестном или невалидном идентификаторе…» |
| M4 | `volume = clampSoundboardVolume(vol)` → `volume = vol` в `setSoundboardVolume` | `2cb8bc1c8507b647e11e4b586ff166d1` | 1 | **убита** | «setSoundboardVolume нормализует громкость через clampSoundboardVolume…» |
| M5 | в `playSoundboardSound` в колбэк уходит `1` вместо текущей громкости | `af982596dcaf6d30ae7ad73ca36d6cda` | 1 | **убита** | «пробрасывает выбор слэш-команды и воспроизведение звука саундборда в колбэки options» |

Восстановление: после каждой мутации и после всего прогона MD5 файла = `0d5ae58d106cc500335195b82e1e2e77`. Финальная проверка `md5sum` совпала с эталоном ТЗ.

MD5 красных логов: `M1 327cda0bc00f36238ce644b96a7eab6b`, `M2 84a6a2f81429d14b0e51f8db446e5578`, `M3 376f8eac5559bcca472c29352e202646`, `M4 a06736ac02ffdc0ecd867d3c46e25fb2`, `M5 2a9f03c217b5c3ccdc0bc80d9135dc77`.

## 5. Качество кода

Что проверено и в порядке:

- **Границы модуля.** Нет DOM, `fetch`, IPC, `eval`, `innerHTML`; только импорт двух чистых функций (`parseSlashDraftInput`, `filterSlashCommands`, `filterSoundboardSounds`, `clampSoundboardVolume`) и возврат данных. Файл безопасен для подключения в композер.
- **Иммутабельность.** `COMPOSER_POPOVER_LAYERS`, `resolveComposerDraftPopover(...)` и `getState()` возвращают `Object.freeze`; входные `commands`/`sounds` копируются в конструкторе, поэтому данные вызывающего не разделяются с состоянием моста на верхнем уровне.
- **Ошибки не глотаются.** `openPopover` бросает `Error` с русским сообщением на невалидный или неизвестный id и не меняет состояние (проверено тестом). Колбэки вызываются только при `typeof ... === "function"`.
- **Типы.** `// @ts-check` + JSDoc на всех экспортах; `tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` чистый.
- **Контракт с соседями.** `slash-command-picker.js` (`task-261`) и `soundboard-picker.js` (`task-260`) уже приняты; мост переиспользует их чистые функции, дублирования логики парсинга/фильтрации нет. Ожидаемый порядок в тесте (`cmd-1, cmd-2, cmd-3, cmd-4`) воспроизводит правила ранжирования `filterSlashCommands` (приоритет префикса имени, затем `localeCompare("en")`, tie-break по `String(id)`).
- **Тесты.** 11 тестов покрывают позитивные, негативные, граничные случаи и защиту от мутации (`-0`, `NaN`, `-0.5`, `2.5`, нестроковые входы, `null`, `{}`, `" emoji-picker "`), а также изоляцию возвращаемых коллекций от входных данных.

## 6. Замечания

### P2-1. `scripts/test-impact.mjs` не знает о модуле — профильный тест не попадает в targeted-набор

`node scripts/test-impact.mjs --changed rust/gui-spike/ui/features/composer-popovers-bridge.js` выдаёт только generic `FRONTEND`-список (`gui-spike-dom`, `gui-spike-features`, …) и `geometry-check.mjs`; `tests/unit/composer-popovers-bridge.test.mjs` в наборе отсутствует, потому что записи в `UI_MODULE_TESTS` для этого модуля нет. Риск ограничен: `gate.mjs` (scope `worker`) и `npm run test:unit` гоняют `tests/unit/**/*.test.mjs`, поэтому при полном гейте тест выполняется. Рекомендация: G4 QA добавляет запись в `UI_MODULE_TESTS` отдельной карточкой (`scripts/` — не файлы PR-154, в этой ветке не правим).

### P3-1. `getFilteredSlashCommands()` защищает только верхний уровень

`filteredSlashCommands.map((cmd) => ({ ...cmd }))` копирует поля команды, но массив `options` остаётся общим с внутренним состоянием. Проверено вручную: внешний `filtered[0].options.push({ name: "injected" })` меняет последующий `getFilteredSlashCommands()` (3 опции вместо 2), при этом `filtered[0].name = "hacked"` внутреннее состояние не меняет. Тест этого не покрывает. Рекомендация на этап wiring: клонировать `options` (или переиспользовать `cloneCommandItem` из `slash-command-picker.js`).

### P3-2. `selectSlashCommand` не валидирует аргумент

`selectSlashCommand(null)` возвращает `{}` и вызывает `onSelectSlashCommand({})` — рантайм-валидация есть только у `openPopover`. Для внутреннего API это допустимо, но UI-слой может передать пустое значение из гонки рендера. Рекомендация: отбрасывать не-объекты (`if (!cmd || typeof cmd !== "object") throw new TypeError(...)`) и закрыть тестом.

### P3-3. Идентификаторы слоёв не совпадают с именами реальных модулей

`COMPOSER_POPOVER_LAYERS` содержит `emoji-picker` и `attachment-upload`, а в `rust/gui-spike/ui/features/` компоненты называются `expression-picker.js` и `attachments.js` / `attachment-helper.js`. Оракул `reference-oracle/frontend-d03.md:38-44` и `:183` фиксирует `expressionPickerPositionLayer__74017` / `layer__59d0d`; строка `attachment-upload` в дампе встречается только как CSS-переменная `--custom-channel-attachment-upload-*` (`reference-oracle/decoded/cac3dca658eee574_0`), не как id поповера. Сам контракт моста не ломается, но при подключении в `app-shell-wiring` (спецификация `task-275`, `waves/RW-40/G3/task-275-app-shell-wiring-spec/TASK.md`) нужна явная таблица «id слоя → модуль → селектор», иначе список слоёв и DOM разойдутся.

### P3-4. Шум рабочего дерева вне scope

Симлинк `node_modules` в рабочем дереве заменён реальным каталогом, `package-lock.json` рассинхронизирован с `package.json` (`bin`, `engines`). Оба эффекта воспроизводятся в несвязанных деревьях (см. §2), к PR-154 не относятся. Перед merge в `unstable` включать в коммит только два файла задачи.

## 7. Что осталось за пределами ревью

- Модуль ещё нигде не импортируется: подключение в композер и регистрацию в оболочке делает `task-275` (G3 спека `app-shell-wiring`) и следующая интеграционная карточка G2. Сам PR этого и не заявляет (REPORT.md §Риски).
- Полный `npm run gate:worker` не запускался: ТЗ TR-183A ограничивает проверки targeted-прогоном и `tsc`. Полный гейт выполняет integration-лейн на batch.
- `git commit`/`push` не выполнялись.

## 8. Итог

Код соответствует ТЗ `task-270`, контракт опирается на принятые `slash-command-picker.js` и `soundboard-picker.js`, тесты зелёные и обладают достаточной силой: все пять точечных мутаций убиты, исходник восстановлен побайтно (MD5 `0d5ae58d106cc500335195b82e1e2e77`). Блокирующих дефектов нет.

**Вердикт: APPROVED** (`P0=0`, `P1=0`, мутации `5/5`). Замечания P2-1 и P3-1…P3-4 не блокируют merge в `unstable`; P2-1 стоит завести карточкой в G4 QA.
