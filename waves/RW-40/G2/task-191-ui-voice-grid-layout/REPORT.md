# T-191 — UI voice grid layout (G2)

## Сделано
- `tests/unit/voice-grid.test.mjs` — red-first контракт: `MAX_VISIBLE_VOICE_TILES = 25`,
  диапазоны `computeVoiceGridDimensions` (0, 1, 2, 3..=4, 5..=9, 10..=16, 17..=25, >25,
  отрицательные/NaN), порядок `prioritizeVoiceParticipants` (фокус → стрим → видео →
  говорит → без mute → алфавит `displayName || id`), обрезка до 25 с overflow, отсутствие мутации.
- `rust/gui-spike/ui/features/voice-grid.js` — `// @ts-check`, JSDoc-типы
  `VoiceGridDimensions`/`VoiceParticipant`, чистые функции без DOM.

## Прогоны
- red: `red.log` — модуль отсутствовал, `fail 1`.
- green: `green.log` — `tests 16 / pass 16 / fail 0`.
- `npx tsc -p tsconfig.json --noEmit` — ошибок по `voice-grid` нет.

## Файлы и md5
- `8710a081711dab78688cc5f712e4b58e  rust/gui-spike/ui/features/voice-grid.js`
- `8187d1f40324fb7a378489bc459d4f58  tests/unit/voice-grid.test.mjs`

## Риски / осталось
- Полный `gate:worker` не запускался (token-lean режим) — нужен в integration-лейне.
- Компонент не подключён к голосовой панели: интеграция и CSS-сетка — отдельная карточка.
