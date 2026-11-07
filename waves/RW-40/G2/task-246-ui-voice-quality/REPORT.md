# task-246 — UI voice quality (RU labels) — G2

## Сделано
- `tests/unit/voice-quality.test.mjs` — 8 тестов (константы, замороженная карта режимов видео, clamp,
  дефолты для пустого канала, пустой регион, отрицательный/переполненный `connectedCount`).
- `rust/gui-spike/ui/features/voice-quality.js` — `// @ts-check`, чистые функции без DOM/IPC:
  `MIN_VOICE_BITRATE_BPS` (8000), `DEFAULT_VOICE_BITRATE_BPS` (64000), `MAX_VOICE_BITRATE_BPS` (384000),
  `VIDEO_QUALITY_MODES_RU` (frozen), `clampVoiceBitrateBps`, `formatVoiceChannelQualityRu`.

## Red → Green
- `red.log` — `exit=1`, 1 fail: модуль отсутствует (`ERR_MODULE_NOT_FOUND`), тест не компилируется.
  Лог снят с финальной версии теста при временно убранном модуле, чтобы red соответствовал green.
- `green.log` — `exit=0`, `tests 8 / pass 8 / fail 0`.

## Отклонение от буквы ТЗ (осознанное, оракул — формула ТЗ)
`clampVoiceBitrateBps(null)`: `Number(null) === 0` — конечно, поэтому clamp даёт `8000`, а не `64000`.
ТЗ говорит «non-finite → 64000», и формула `Math.min/max(Number(bps))` даёт именно 8000. Тест закрепляет
это поведение с комментарием. `undefined`, `NaN`, `Infinity`, `"abc"` → `64000`.

## Файлы и md5
```
1482367754dd314e8a3e4f9b53129ba2  rust/gui-spike/ui/features/voice-quality.js
3bf63795695492563b9a13fc0a8e49ec  tests/unit/voice-quality.test.mjs
```

## Риски
- `formatVoiceChannelQualityRu` ожидает, что `userLimit`/`connectedCount` приходят как числа или
  числовые строки; `null` для `userLimit` даёт «Без лимита» (`Number(null) === 0`).
- Ключи `videoQualityMode` вне `{1,2}` падают в «Авто» — по контракту ТЗ.
- `gate:worker` и `git commit` не запускались (запрещено ТЗ).
