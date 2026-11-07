# task-211 — UI voice volume helper

## Сделано
- `rust/gui-spike/ui/features/voice-volume.js` — константы границ и 4 чистые функции
  нормализации громкости (`// @ts-check` + JSDoc).
- `tests/unit/voice-volume.test.mjs` — 15 тестов, 5 сьютов.

## Контракт
- `MIN_USER_VOLUME=0`, `DEFAULT_USER_VOLUME=100`, `MAX_USER_VOLUME=200`, `MAX_DEVICE_VOLUME=100`.
- `clampUserVolume`: целое 0..200; нечисловое → 100.
- `clampDeviceVolume`: целое 0..100; нечисловое → 100.
- `computeAudioGainMultiplier(vol, localMuted, deafened)`: 0 при mute/deaf, иначе vol/100.
- `formatVolumeLabelRu(vol, localMuted)`: «Без звука» при mute или 0, иначе `N%`.

## Прогоны
- Red: `red.log` — `fail 1`, модуль отсутствовал (`ERR_MODULE_NOT_FOUND`).
- Green: `green.log` — `tests 15 / pass 15 / fail 0`.

## Файлы и md5
- `rust/gui-spike/ui/features/voice-volume.js` — `3e62fb2946e8491741d05daf0f2ad452`
- `tests/unit/voice-volume.test.mjs` — `f6ff26aa16dd75e2b3da40f573f7af4c`

## Примечания
- Гейт `gate:worker` по ТЗ не запускался (token-lean режим); git commit не делался.
