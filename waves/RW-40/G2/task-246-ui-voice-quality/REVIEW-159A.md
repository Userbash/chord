# task-246 — Technical Review 159A (код + мутации)

**Verdict: APPROVED**

## Объём проверки
- `rust/gui-spike/ui/features/voice-quality.js` (76 строк, `// @ts-check`, чистые функции без DOM/IPC).
- `tests/unit/voice-quality.test.mjs` (8 тестов).
- `REPORT.md` (md5-оракул).

## Targeted-прогон
`node --test tests/unit/voice-quality.test.mjs` → `tests 8 / pass 8 / fail 0`, exit=0.

## Мутационное тестирование — 5/5 убиты
Каждая мутация применялась на свежую копию оригинала, затем файл восстанавливался из бэкапа.

| # | Мутация | Результат | Поймано тестами |
|---|---------|-----------|-----------------|
| M1 | `MIN_VOICE_BITRATE_BPS = 8000` → `9000` | exit=1, fail 2 | константы + clamp нижней границы |
| M2 | `DEFAULT_VOICE_BITRATE_BPS = 64000` → `65000` | exit=1, fail 2 | константы + дефолт non-finite |
| M3 | `MAX_VOICE_BITRATE_BPS = 384000` → `385000` | exit=1, fail 2 | константы + clamp верхней границы |
| M4 | `Math.round(value)` → `Math.floor(value)` | exit=1, fail 1 | округление `63999.6 → 64000` |
| M5 | `connected >= limit` → `connected > limit` | exit=1, fail 1 | `isFull` для полного канала `10/10` |

Итог: **5/5 мутаций убиты (100%)**, мутационный счёт не ниже порога `TEST-POLICY.md`.

## Восстановление файла
```
1482367754dd314e8a3e4f9b53129ba2  rust/gui-spike/ui/features/voice-quality.js
```
md5 после восстановления совпадает с md5 в `REPORT.md` побайтово. Повторный прогон после восстановления: 8/8 pass.

## Замечания по коду (не блокирующие, P3)
1. `clampVoiceBitrateBps(null)` → `8000`, а не `64000`: `Number(null) === 0` конечно, поэтому идёт в clamp. Отклонение от буквы ТЗ осознанно задокументировано в `REPORT.md` и закреплено тестом с комментарием — оракул (формула ТЗ) соблюдён, регрессии нет.
2. `formatVoiceChannelQualityRu` полагается на числовые значения `userLimit`/`connectedCount`; `null` для `userLimit` даёт «Без лимита» (`Number(null) === 0`) — поведение приемлемо, покрыто тестом на `0`.
3. `videoQualityMode` вне `{1,2}` падает в «Авто» — по контракту ТЗ, покрыто тестом (`99`).

## Границы
`gate:worker`, полный `cargo`/`npm` гейт и `git commit` не запускались — запрещено ТЗ. Проверен только targeted-тест модуля.

## Готовность
P0 = 0, P1 = 0. Файлы не пересекаются с другими воркерами (G2, `ui/features/*` + `tests/unit/*`). Готово к merge в `unstable` после одобрения PM.
