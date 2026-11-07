# T-213 (G2) — UI voice input mode

Карточка: task-213-ui-voice-input-mode. Ветка: feature/task-213-ui-voice-input-mode. HEAD: fb7af06.
Объём: чистая логика режимов ввода голоса (без DOM), Red-first, прогон только целевого файла тестов.

## Сделано
- VOICE_INPUT_MODES — замороженный набор из двух режимов: VOICE_ACTIVITY, PUSH_TO_TALK.
- clampVadThresholdDb(db) — округление до целого, зажим в −100..0, фолбэк −60 на нечислах и отсутствии значения.
- clampPttReleaseDelayMs(ms) — округление до целого, зажим в 0..2000, фолбэк 200 на нечислах.
- isVoiceTransmitting(config, state) — muted/deafened глушат любую передачу; PTT смотрит на pttPressed;
  VOICE_ACTIVITY (и любой неизвестный режим) сравнивает конечный currentLevelDb с клампленым порогом.
- formatVoiceInputSummaryRu(config) — «Режим рации (задержка N мс)» / «Активация по голосу (N дБ)».

## Red / Green
- Red: node --test tests/unit/voice-input-mode.test.mjs → падение на отсутствующем модуле, лог red.log рядом.
- Green: тот же прогон → 14 тестов, 14 pass, 0 fail, 0 skip, ~43 мс (green.log).

## Файлы + md5
- rust/gui-spike/ui/features/voice-input-mode.js — e23bf3f6f6bacc3bc14b72d873ee2685
- tests/unit/voice-input-mode.test.mjs — 3aa992af60ae8585828f1ad7d5f070d0

## Риски
- Модуль не подключён к панели голоса и аудио-тракту: сквозная интеграция и DOM не проверялись.
- По режиму TOKEN-LEAN gate:worker, tsc и чужие тесты не запускались; проверка — только целевой файл.
- Коммит не делался; отчёт лежит в waves/RW-40/G2/task-213-ui-voice-input-mode/.
