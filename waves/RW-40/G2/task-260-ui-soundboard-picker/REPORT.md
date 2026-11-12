# task-260 — Voice Soundboard Picker & Custom Sound Upload Controller (`soundboard-picker.js`, FE-P0-17)

- **Карточка / Ветка**: `task-260` (`feature/task-260-ui-soundboard-picker`)
- **Отдел**: G2 Frontend (WebView UI & DOM/A11y)
- **Оракулы**: `reference-oracle/rest-static.json` (`GUILD_SOUNDBOARD_SOUNDS POST /guilds/{arg}/soundboard-sounds`, `SEND_SOUNDBOARD_SOUND POST /channels/{arg}/send-soundboard-sound`, `web.4d4b2f75fdea5470.js`, лимиты имени звука `2..=32` символов, размер файла до `512 КБ`, громкость `0..1`).

## Сделано
1. `tests/unit/soundboard-picker.test.mjs` (`// @ts-check`, Strict TDD: Red → Green, 10 unit-тестов в 5 suites):
   - проверка экспорта констант `MIN_SOUNDBOARD_NAME_CHARS = 2`, `MAX_SOUNDBOARD_NAME_CHARS = 32`, `MAX_SOUNDBOARD_BYTES = 524288`;
   - проверка `isValidSoundboardName(name)`:
     - возвращает `true`, если `typeof name === "string"` и длина `name.trim()` в диапазоне `2..=32` символов (включая границы `2` и `32`);
     - возвращает `false` для пустых/пробельных строк, длины `1` или `33` символов после `trim()`, а также нестроковых значений (`null`, `undefined`, `42`, `{}`);
   - проверка `clampSoundboardVolume(volume)`:
     - ограничивает числовые значения диапазоном `0..1` (`< 0 -> 0`, `> 1 -> 1`);
     - возвращает `1` при нечисловом или `NaN` значении;
   - проверка `filterSoundboardSounds(sounds, query)`:
     - фильтрация доступных элементов (`available !== false`) по регистронезависимой подстроке `name` (при пустом/пробельном запросе возвращает все доступные элементы);
     - сортировка по `name.localeCompare(..., "ru")` по возрастанию и tie-break по `String(soundId)` по возрастанию;
     - сохранение неизменности входного массива и его объектов;
   - проверка `createSoundboardPickerController(options)`:
     - рендер корневого диалога `[data-soundboard-picker]` с `role="dialog"` и `aria-label="Звуковая панель"`;
     - рендер поля поиска `[data-soundboard-search]` (фильтрует список при `input`), ползунка громкости `[data-soundboard-volume]` и кнопок доступных звуков `[data-soundboard-play="<soundId>"]` (клик вызывает `onPlaySound(soundId, currentVolume)`);
     - рендер формы загрузки кастомного звука (`[data-soundboard-name-input]`, `[data-soundboard-datauri-input]`, `[data-soundboard-upload-btn]`, `[data-soundboard-error]`): валидация `isValidSoundboardName(name)` и префикса `data:audio/` (и лимита `MAX_SOUNDBOARD_BYTES`), вывод текста ошибки в `[data-soundboard-error]` при ошибке, очистка ошибки и вызов `onUploadSound({ name: trimmedName, soundDataUri: trimmedUri, volume: currentVolume })` при успехе;
     - проверка методов `{ setState(patch), getState(), destroy() }` и защиты возвращаемого состояния от внешней мутации.
2. `rust/gui-spike/ui/features/soundboard-picker.js` (`// @ts-check` + JSDoc, ESM):
   - реализованы и экспортированы `MIN_SOUNDBOARD_NAME_CHARS`, `MAX_SOUNDBOARD_NAME_CHARS`, `MAX_SOUNDBOARD_BYTES`, `isValidSoundboardName`, `clampSoundboardVolume`, `filterSoundboardSounds` и `createSoundboardPickerController`.

## Прогоны (Red / Green / Targeted / Gate)
- **Red**: `node --test tests/unit/soundboard-picker.test.mjs > /home/user/orca/project/waves/RW-40/G2/task-260-ui-soundboard-picker/red.log 2>&1` → `exit=1` (`ERR_MODULE_NOT_FOUND`, `tests 1, pass 0, fail 1`).
- **Green**: `node --test tests/unit/soundboard-picker.test.mjs > /home/user/orca/project/waves/RW-40/G2/task-260-ui-soundboard-picker/green.log 2>&1` → `exit=0` (`tests 10, suites 5, pass 10, fail 0`).
- **Typecheck**:
  - `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → `exit=0` (0 ошибок);
  - `tsc --ignoreConfig --noEmit --strict --allowJs --checkJs --noUncheckedIndexedAccess --skipLibCheck --target es2022 --module node20 --moduleResolution node16 --types node rust/gui-spike/ui/features/soundboard-picker.js tests/unit/soundboard-picker.test.mjs` → `exit=0` (0 ошибок).
- **Gate**: полный `gate:worker` не запускался (режим ТЗ воркера в batch); чужие файлы не затронуты.

## Файлы и md5
```
6e41082ccdde7d3a6b5c0227114c1271  rust/gui-spike/ui/features/soundboard-picker.js
5fb863f07ecbf1bc7aad453db4e0fee4  tests/unit/soundboard-picker.test.mjs
61761707b4b78f359f51862bc54a65f5  /home/user/orca/project/waves/RW-40/G2/task-260-ui-soundboard-picker/red.log
3705dc1499c18593c176ece91c6429b3  /home/user/orca/project/waves/RW-40/G2/task-260-ui-soundboard-picker/green.log
```

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера.
