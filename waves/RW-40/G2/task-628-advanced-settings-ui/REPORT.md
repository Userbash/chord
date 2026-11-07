# Отчёт воркера: TASK-628 (`G2 Frontend` — Advanced Settings UI: Keybinds, Soundboard, Clips)

## 1. Карточка и контекст
- **Волна / Отдел:** `RW-40` / `G2 Frontend`
- **Задача:** `TASK-628` (`task_1dfe10fc6197`, dispatch `ctx_0ccaea1f9502`)
- **Ветка:** `feature/task-628-advanced-settings-ui`
- **Назначение:** Реализация UI-модуля расширенных настроек (`Advanced Settings UI`) с тремя разделами — «Горячие клавиши» (`Keybinds`), «Звуковая панель» (`Soundboard`) и «Видеонарезки» (`Clips`) в `rust/gui-spike/ui/features/advanced-settings.js`, таблицы стилей `rust/gui-spike/ui/styles/advanced-settings.css` и полного набора unit-тестов `tests/unit/advanced-settings.test.mjs`.

## 2. Сверка с эталонным RE-дампом (`/home/sanya/re-deep/`)
Все константы, действия, диапазоны, дефолты и русские строки сверены с эталонным RE-оракулом:
1. **Вкладки и заголовки (`ru-catalog.json` / `en-catalog.json`):**
   - `T9DA2K`: `Горячие клавиши` (`Keybinds`)
   - `OWQNYr`: `Звуковая панель` (`Soundboard`)
   - `z2jK6X`: `Видеонарезки` (`Clips`), `J0dpcB`: `Настройки клипов` (`Clips Settings`)
2. **Горячие клавиши (`d8b0ffc3255e8113_0 @152000..170000`, `3eefa7dae1e2c234_0` модуль `994778`, `e2135c73a7dbde68_0 @2363`):**
   - Справочник `KEYBIND_ACTION_TYPES` из 19 действий (`UNASSIGNED`, `PUSH_TO_TALK`, `PUSH_TO_TALK_PRIORITY`, `PUSH_TO_MUTE`, `VAD_PRIORITY`, `TOGGLE_MUTE`, `TOGGLE_DEAFEN`, `TOGGLE_VOICE_MODE`, `TOGGLE_STREAMER_MODE`, `TOGGLE_VOICE_CHANNEL_CHAT`, `TOGGLE_CAMERA`, `TOGGLE_GO_LIVE_STREAMING`, `NAVIGATE_BACK`, `NAVIGATE_FORWARD`, `SWITCH_TO_VOICE_CHANNEL`, `DISCONNECT_FROM_VOICE_CHANNEL`, `SOUNDBOARD`, `SOUNDBOARD_HOLD`, `SAVE_CLIP`).
   - Запись с клавиатуры (`RECORDING` / `DEFAULT`), отмена по `Escape`, детектор конфликтов (`detectKeybindConflicts`, `7lQlw3`) со встроенными (`DEFAULT_SHORTCUTS`) и кастомными биндами, защита системных (`managed: true`) горячих клавиш от удаления и смены действия.
3. **Звуковая панель (`d8b0ffc3255e8113_0 @198479`, `ui8/23-settings-voice.json`):**
   - Ползунок громкости `0..100%` (`clampSoundboardVolumePercent`, `soundboardPercentToFloat`), выбор звуков присоединения (`Entrance Sounds`) глобально (`"all"` — «Все серверы») и по серверам с автоматическим fallback (`resolveEntranceSoundForGuild`), предпросмотр звука и валидация загрузки кастомного звука (имя `2..32` символа, `data:audio/*` до `512 КБ`).
4. **Видеонарезки (`d8b0ffc3255e8113_0 @516500..525500`, `3eefa7dae1e2c234_0` модули `274372`, `696016`, `753070`, `fe99f9e2d77f4c0b_0` модуль `226640`):**
   - Дефолты `DEFAULT_CLIPS_SETTINGS`: `clipsEnabled: false`, `clipsLength: 30` (`30` / `60` / `120` сек. и поддержка мс `30000` / `60000` / `120000`), `resolution: 1080` (`480`, `720`, `1080`, `1440`, `0 = Источник`), `frameRate: 30` (`15`, `30`, `60` FPS), `bitratePercent: 75` (`0..100%`), `voiceAudioEnabled: true`, `remindersEnabled: true`, `clipKeybind: "Alt+C"`, `storageLocation: "Discord Clips"` (с защитой от path traversal `..` и NUL-байтов), баннер предупреждения об оборудовании (`SIxrIF`).

## 3. Red-прогон (TDD)
- До создания `rust/gui-spike/ui/features/advanced-settings.js` запущен тест `node --test tests/unit/advanced-settings.test.mjs`, зафиксировано падение (`ERR_MODULE_NOT_FOUND`).
- Лог сохранён в `waves/RW-40/G2/task-628-advanced-settings-ui/red.log`.

## 4. Green-прогон и Targeted-проверки
- `node --test tests/unit/advanced-settings.test.mjs` — **11/11 pass (`0 fail`)**, лог сохранён в `waves/RW-40/G2/task-628-advanced-settings-ui/green.log`.
- `npm run typecheck` и `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` — **0 ошибок**.
- `node --test tests/meta/*.test.mjs` — **118/118 pass (`0 fail`)**, включая `plan-sync.test.mjs` и `test-policy.test.mjs`.
- `node waves/RW-39/PM/w39c/geometry-check.mjs` — **31/31 геометрических значений в допуске**.
- Затронутые unit-тесты (`gui-spike-dom`, `gui-spike-model`, `gui-spike-bench`, `gui-spike-features`, `gui-spike-auth`, `gui-spike-regressions`, `guild-rail`, `voice-ui`, `channel-switch`, `keybinds`, `settings-modal`, `appearance-settings`) — **100% pass (`0 fail`)**.

## 5. Мутационное тестирование (`5/5 KILLED = 100%`)
Проверены 5 независимых мутаций в `rust/gui-spike/ui/features/advanced-settings.js` (лог в `waves/RW-40/G2/task-628-advanced-settings-ui/mutations.log`):
- `M1 KILLED`: `hasBuiltInConflict` принудительно выставлен в `false` (детектор встроенных конфликтов).
- `M2 KILLED`: `MAX_SOUNDBOARD_VOLUME_PERCENT` изменён с `100` на `200`.
- `M3 KILLED`: отключён глобальный fallback `"all"` в `resolveEntranceSoundForGuild`.
- `M4 KILLED`: отключена проверка `segments.includes("..")` в `validateClipsStorageLocation`.
- `M5 KILLED`: убрана защита `target.managed` в `removeKeybind`.

## 6. Затронутые файлы и контрольные суммы (`md5sum`)
| Файл | MD5 | Статус |
|---|---|---|
| `rust/gui-spike/ui/features/advanced-settings.js` | `7235afbbb097300c18d5372fb4bb67ab` | Создан |
| `rust/gui-spike/ui/styles/advanced-settings.css` | `e06a219e1faa7b303a37e0a5bf2ce9b5` | Создан |
| `tests/unit/advanced-settings.test.mjs` | `13e618a80751f9a9f5545982ace5a474` | Создан |
| `TDD-PLAN.md` | `dd2286d732f3b318333c9ca1b9095fbf` | Обновлён (§8 таблица синхронизации тестов) |

## 7. Риски и замечания
- Блокирующих рисков нет (`P0 = 0`, `P1 = 0`).
- Стили `advanced-settings.css` используют исключительно CSS-токены из `rust/gui-spike/ui/tokens.css` (0 сырых цветов, 0 `!important`, 0 `:has()`, 0 внешних ресурсов).
- Код `advanced-settings.js` не вызывает запрещённых нативных/медиа API и не пишет в `console`.
