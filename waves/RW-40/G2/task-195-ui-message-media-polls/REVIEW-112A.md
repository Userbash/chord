# REVIEW-112A — task-195 ui message media & polls

Карточка: RW-40 / G2 / task-195. Ревьюер: DeepSeek (Technical Review). Файл: `rust/gui-spike/ui/features/message-media-polls.js` (396 строк).

## Вердикт: APPROVED
P0 = 0, P1 = 0. Мутации: 5/5 killed. md5 после restore совпал с REPORT.md (`58f73ca76625e33382267d2f819d46ea`).

## Прогоны
- Targeted: `node --test tests/unit/message-media-polls.test.mjs` → tests 11, pass 11, fail 0.
- Мутации (каждая: mutate → тест → restore; исходник восстановлен байт-в-байт):
  - `m1-data-leading` (`data-leading` всегда `'false'`) → fail 1, killed.
  - `m2-spoiler-toggle` (спойлер не раскрывается) → fail 1, killed.
  - `m3-answer-id` (`data-answer-id` = текст ответа) → fail 4, killed.
  - `m4-expired` (`isExpired = false`) → fail 1, killed.
  - `m5-percentage` (`%` убран из процентов) → fail 1, killed.

## Замечания (не блокирующие)
- `data-expired`/`isExpired` завязаны на русскую строку `'Опрос завершён'` из `formatPollExpiryRu` — хрупкий контракт, лучше явный флаг из `poll-progress.js` (P2, вне scope этой карточки).
- `renderMessageAttachments` не ограничивает количество карточек; лимит 10 файлов должен применяться на уровне вызывающего кода при интеграции в `chat.js`.

## Границы
Читал только `message-media-polls.js` и `REPORT.md`. `git commit` не выполнялся, исходники не изменены.
