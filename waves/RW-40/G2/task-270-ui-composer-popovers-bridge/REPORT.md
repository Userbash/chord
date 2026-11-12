# task-270 (FE-INT-13) — Unified Composer Popovers & Slash/Soundboard Bridge (`composer-popovers-bridge.js`)

- **Карточка / Ветка**: `task-270` (`feature/task-270-ui-composer-popovers-bridge`)
- **Отдел**: G2 Frontend Integration

## Сделано
1. `tests/unit/composer-popovers-bridge.test.mjs` (`// @ts-check`, Strict TDD: Red → Green, 11 unit-тестов в 3 suites):
   - проверка экспорта замороженного списка слоёв поповеров композера `COMPOSER_POPOVER_LAYERS = Object.freeze(['slash-command-picker', 'soundboard-picker', 'emoji-picker', 'attachment-upload'])` и защиты от мутации;
   - проверка `resolveComposerDraftPopover(draftText)`:
     - делегирует в `parseSlashDraftInput(draftText)` из `./slash-command-picker.js`;
     - возвращает замороженный `{ activePopover: 'slash-command-picker', commandName, rawArgs }` для валидного слэш-ввода;
     - возвращает замороженный `{ activePopover: null, commandName: null, rawArgs: '' }` для обычного текста, пустого слэша и нестроковых значений;
   - проверка фабрики `createComposerPopoversBridge(options)`:
     - инициализация состояния по умолчанию и замороженный снимок `getState()` (`{ activePopover, commandName, rawArgs, volume }`);
     - нормализация начальной и обновляемой громкости через `clampSoundboardVolume` (`setSoundboardVolume(vol)`);
     - обновление состояния черновика `updateDraft(draftText)` с активацией `'slash-command-picker'` и фильтрацией `getFilteredSlashCommands()` через `filterSlashCommands`;
     - открытие и закрытие поповеров `openPopover(popoverId)` / `closePopover()`, выброс `Error` при неизвестном идентификаторе слоя;
     - фильтрация звуков `getFilteredSoundboardSounds(query?)` через `filterSoundboardSounds` без мутации исходных данных;
     - проброс `selectSlashCommand(cmd)` и `playSoundboardSound(soundId)` в колбэки `onSelectSlashCommand` и `onPlaySoundboardSound`.
2. `rust/gui-spike/ui/features/composer-popovers-bridge.js` (`// @ts-check` + JSDoc, ESM):
   - реализованы и экспортированы `COMPOSER_POPOVER_LAYERS`, `resolveComposerDraftPopover`, `createComposerPopoversBridge`.

## Прогоны (Red / Green / Targeted / Typecheck)
- **Red**: `node --test tests/unit/composer-popovers-bridge.test.mjs > /home/user/orca/project/waves/RW-40/G2/task-270-ui-composer-popovers-bridge/red.log 2>&1` → `exit=1` (`ERR_MODULE_NOT_FOUND`, `tests 1, pass 0, fail 1`).
- **Green**: `node --test tests/unit/composer-popovers-bridge.test.mjs > /home/user/orca/project/waves/RW-40/G2/task-270-ui-composer-popovers-bridge/green.log 2>&1` → `exit=0` (`tests 11, suites 3, pass 11, fail 0`).
- **Targeted (соседние модули)**: `node --test tests/unit/slash-command-picker.test.mjs tests/unit/soundboard-picker.test.mjs` → `exit=0` (`tests 20, pass 20, fail 0`).
- **Typecheck**:
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `exit=0` (0 ошибок);
  - `npx tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --noUncheckedIndexedAccess --skipLibCheck --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/composer-popovers-bridge.js tests/unit/composer-popovers-bridge.test.mjs` → `exit=0` (0 ошибок).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ воркера в batch); чужие файлы не затронуты.

## Файлы и md5
```
0d5ae58d106cc500335195b82e1e2e77  rust/gui-spike/ui/features/composer-popovers-bridge.js
7e3c3edefd86fd58c9b4b117d4d80be1  tests/unit/composer-popovers-bridge.test.mjs
659c17fdf334407e10426dcd1fb0bb3a  /home/user/orca/project/waves/RW-40/G2/task-270-ui-composer-popovers-bridge/red.log
0ffdc6966854494a34806bf1bb50230a  /home/user/orca/project/waves/RW-40/G2/task-270-ui-composer-popovers-bridge/green.log
```

## Риски
- Модуль не зависит от глобального DOM/IPC и готов к подключению в композер (`composer.js` / `app.js`).
- `git commit` не выполнялся в соответствии с правилами воркера.
