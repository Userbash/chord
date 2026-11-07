# task-201 — UI voice stage controller (`voice-stage-ui.js`)

Batch: RW-40 / G2. Ветка: `feature/task-201-ui-voice-stage-controller`.

## Сделано
- `tests/unit/voice-stage-ui.test.mjs` — red-first unit-тесты (7 тестов, 2 сьюта) на `createVoiceStageController` и `openScreenSharePicker`.
- `rust/gui-spike/ui/features/voice-stage-ui.js` — реализация `createVoiceStageController(containerEl, options)` с переиспользованием `./voice-grid.js` (`computeVoiceGridDimensions`, `prioritizeVoiceParticipants`, `MAX_VISIBLE_VOICE_TILES`); `// @ts-check` + JSDoc, ESM-экспорты.

## Поведение
- `createVoiceStageController(containerEl, options = { onFocusChange, onStartStream })`:
  - рендерит CSS grid-контейнер с `data-columns` и `data-rows`, вычисленными через `computeVoiceGridDimensions`;
  - рендерит плитки участников (до `MAX_VISIBLE_VOICE_TILES`, в порядке `prioritizeVoiceParticipants`) с атрибутами `data-participant-id`, `data-speaking` (`'true'|'false'`), `data-streaming` (`'true'|'false'`), `data-focused` (`'true'|'false'`) и русскими бейджами (`'В эфире'` при `streaming: true`, `'Микрофон выкл.'` при `muted: true`);
  - клик по плитке переключает фокус (`focusedId` или `null` при повторном клике), перерисовывает сетку и вызывает `onFocusChange(focusedId)`;
  - при `overflow.length > 0` рендерит плитку переполнения с `data-voice-overflow="true"` и текстом `'+' + overflow.length + ' ещё'`;
  - предоставляет `openScreenSharePicker({ sources, defaultResolution = '1080p', defaultFps = 60 })`, рендерящий русский диалог (`'Демонстрация экрана'`) с выбором источника (`[data-source-id]`), разрешения (`'720p'|'1080p'|'1440p'`), частоты кадров (`30|60`), чекбоксом `'Транслировать звук системы'` (`[data-share-audio]`) и кнопкой `'Начать трансляцию'`, вызывающей `onStartStream({ sourceId, resolution, fps, shareAudio })`.

## Прогоны
- Red: `node --test tests/unit/voice-stage-ui.test.mjs` → fail 1 (`ERR_MODULE_NOT_FOUND`) — `red.log`.
- Green: `node --test tests/unit/voice-stage-ui.test.mjs` → tests 7, pass 7, fail 0 — `green.log`.
- Typecheck: `npx tsc -p tsconfig.json` → 0 ошибок в `voice-stage-ui.js` и `voice-stage-ui.test.mjs`.

## Файлы и md5
```
8f94c7105f125417d6c2f8b243c36908  rust/gui-spike/ui/features/voice-stage-ui.js
3ea141f2ff7baad85c9827983b284386  tests/unit/voice-stage-ui.test.mjs
```

## Риски / осталось
- Подключение `createVoiceStageController` в основной экран голосовой панели `voice.js` выполняется отдельной интеграционной карточкой.
- `git commit` не выполнялся.
