# task-263 — Voice Session Sound Cues & Notification Audio Mute Helper (G5/G2)

Batch: RW-40 / G5–G2. Ветка: `feature/task-263-ui-voice-sound-effects` (без `git commit`).

## Сделано
- `tests/unit/voice-sound-effects.test.mjs` — red-first контракт, 12 тестов: заморозка справочника и точные русские подписи, неизвестные/нестроковые ключи, пустой `state`, приоритет `disableAllSounds`, точечный `mutedEvents`, `selfDeaf` (слышны только `deafen`/`undeafen`), `streamerMode` (глушит только `user_join`/`user_leave`), комбинации фильтров, границы и округление громкости, битый вход.
- `rust/gui-spike/ui/features/voice-sound-effects.js` (`// @ts-check` + JSDoc, ESM):
  - `VOICE_SOUND_EVENTS_RU`: `Object.freeze` на 8 событий (`mute`, `unmute`, `deafen`, `undeafen`, `user_join`, `user_leave`, `stream_start`, `stream_end`) с русскими подписями из ТЗ.
  - `shouldPlayVoiceCue(eventKey, state)`: неизвестный ключ → `false` (проверка через `hasOwnProperty`, `typeof === "string"`); затем отсечки `disableAllSounds`, `mutedEvents.includes`, `selfDeaf` с исключением `deafen`/`undeafen`, `streamerMode` для `user_join`/`user_leave`; иначе `true`.
  - `normalizeCueVolume(volumePercent)`: `[0..200]` зажимается, результат `Math.round(clamped) / 100` (0..2, шаг 0.01); не число / `NaN` / бесконечность → `1`.

Отклонение от буквы ТЗ (осознанное, покрыто тестом): `Infinity` не число-процент, поэтому даёт дефолт `1`, а не максимум `2`; битый вход не должен включать максимальную громкость.

## Прогоны
- **Red:** `node --test tests/unit/voice-sound-effects.test.mjs` → `tests 1, pass 0, fail 1`, `ERR_MODULE_NOT_FOUND` на ещё не созданном модуле — `red.log`. Лог снят на финальной версии теста (модуль временно убран, затем возвращён).
- **Green (targeted):** `node --test tests/unit/voice-sound-effects.test.mjs` → `tests 12, pass 12, fail 0, skipped 0` — `green.log`.
- **Typecheck (контракт ТЗ):** `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` → exit `0` (0 ошибок).
- **Typecheck (targeted, строже include проекта):** `npx tsc --ignoreConfig --noEmit --allowJs --checkJs --strict --noUncheckedIndexedAccess --target ES2023 --module NodeNext --moduleResolution NodeNext --types node rust/gui-spike/ui/features/voice-sound-effects.js tests/unit/voice-sound-effects.test.mjs` → exit `0`.
- Полный `gate:worker` в воркере не запускался: по регламенту `T1` выполняется один раз на batch в integration-лейне.

## Файлы и md5
```
99026507e8cdb8a17e2b345ebed814a4  rust/gui-spike/ui/features/voice-sound-effects.js
1799f8674142d207e6ca134bacb10cec  tests/unit/voice-sound-effects.test.mjs
3c484f6c0ca410c1de3d553ded7130b2  waves/RW-40/G2/task-263-ui-voice-sound-effects/red.log
5179f2823e7cce71156199140d295712  waves/RW-40/G2/task-263-ui-voice-sound-effects/green.log
```

## Риски / осталось
- Модуль пока никем не импортируется: воспроизведение сигналов (Web Audio, `Audio`), подписка на события голосовой сессии и UI-переключатели «Отключить все звуки» / «Режим стримера» подключаются отдельной интеграционной карточкой.
- `shouldPlayVoiceCue` опирается на `mutedEvents.includes` — сравнение по строковому ключу события; если настройки придут из localStorage строками иного вида, потребуется миграция значений.
- `git commit` не выполнялся по правилам пайплайна; `package-lock.json` был изменён до начала работы и не трогался.
