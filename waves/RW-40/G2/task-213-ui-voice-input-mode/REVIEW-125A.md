# REVIEW-125A — T-213 UI voice input mode

Вердикт: **APPROVED**. P0 = 0, P1 = 0, мутации 5/5 killed. Целевой прогон
`node --test tests/unit/voice-input-mode.test.mjs` → 14 tests, 14 pass, 0 fail, 0 skip, ~43 мс.

## Проверено
- md5 `rust/gui-spike/ui/features/voice-input-mode.js` = `e23bf3f6f6bacc3bc14b72d873ee2685`,
  `tests/unit/voice-input-mode.test.mjs` = `3aa992af60ae8585828f1ad7d5f070d0` — оба совпадают с REPORT.md.
- Контракт кода соответствует отчёту: замороженный `VOICE_INPUT_MODES` (2 режима), клампы `−100..0` дБ и
  `0..2000` мс с округлением и фолбэками `−60`/`200`, `muted`/`deafened` глушат передачу, PTT решает по
  `pttPressed`, неизвестный режим трактуется как VOICE_ACTIVITY.

## Мутации (по одной, с восстановлением файла)
| # | Мутация | Результат |
|---|---|---|
| 1 | `MIN_VAD_THRESHOLD_DB` −100 → −90 | killed (fail 2) |
| 2 | `MAX_PTT_RELEASE_DELAY_MS` 2000 → 1500 | killed (fail 3) |
| 3 | `DEFAULT_VAD_THRESHOLD_DB` −60 → −50 | killed (fail 3) |
| 4 | guard `muted \|\| deafened` → `&&` | killed (fail 1) |
| 5 | ветка PTT `Boolean(pttPressed)` → `true` | killed (fail 1) |
После восстановления md5 `e23bf3f6f6bacc3bc14b72d873ee2685`, файл побайтово идентичен, правок не осталось.

## Замечания (не блокирующие)
- Первый прогон дал ложные «survived» из-за парсера харнесса (`# fail` вместо `ℹ fail N`); повтор с проверкой exit-кода — 5/5 killed, дефект был в моём харнессе, не в тестах.
- Модуль не подключён к панели голоса и аудио-тракту: интеграция и DOM вне объёма. Полный `gate:worker`/`tsc`/чужие тесты по режиму TOKEN-LEAN не запускались, коммита нет.
