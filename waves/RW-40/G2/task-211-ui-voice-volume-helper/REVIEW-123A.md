# REVIEW-123A — task-211 UI voice volume helper

Ревьюер: Tech Review (DeepSeek), token-lean режим.
Объект: `rust/gui-spike/ui/features/voice-volume.js` (+ `tests/unit/voice-volume.test.mjs`).

## Вердикт: APPROVED — P0=0, P1=0, мутации 5/5 убиты

## Прогон
- `node --test tests/unit/voice-volume.test.mjs`: tests 15 / suites 5 / pass 15 / fail 0.
- md5 до и после мутаций: `3e62fb2946e8491741d05daf0f2ad452` — совпадает с REPORT.md, файл восстановлен побайтно.

## Мутации (каждая: правка → тест → откат)
| # | Мутация | Результат |
|---|---------|-----------|
| M1 | `MAX_USER_VOLUME 200 → 100` | убита, fail 7 |
| M2 | `DEFAULT_USER_VOLUME 100 → 50` | убита, fail 4 |
| M3 | убран `Math.max(min, …)` в `clamp` | убита, fail 3 |
| M4 | `computeAudioGainMultiplier`: `/100 → /10` | убита, fail 4 |
| M5 | `formatVolumeLabelRu`: условие `clamped === 0` выключено | убита, fail 1 |
| M4b | доп.: игнор `deafened` в gain | убита, fail 1 |

## Замечания
- P2 (не блокирует): `clampDeviceVolume` при нечисловом входе возвращает `DEFAULT_USER_VOLUME`; значение верное (100), но семантически просится отдельная константа `DEFAULT_DEVICE_VOLUME`.
- Границы и дефолты совпадают с ТЗ: 0..200 / 0..100, mute и deaf дают gain 0.
- `gate:worker` и git commit не запускались — по ТЗ token-lean. Файл в git не отслеживается (`??`), коммита нет.
