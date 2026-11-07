# REVIEW-158A — task-241 UI Poll Card (FE-P1-27)

Verdict: **APPROVED**

## Объём проверки
- `rust/gui-spike/ui/features/poll-card.js` — чтение целиком (431 строка).
- `tests/unit/poll-card.test.mjs` — чтение целиком (9 тестов).
- `REPORT.md` воркера — сверка заявленных артефактов, md5 и прогонов.
- Targeted-прогон: `node --test tests/unit/poll-card.test.mjs` -> `exit=0`, `tests 9 / pass 9 / fail 0`.

## Соответствие контракту
- `MAX_POLL_ANSWERS = 10`, `MAX_POLL_QUESTION_CHARS = 300` — совпадает с ТЗ и тестами.
- `computePollPercentages(answers, totalVotes)` — чистый: вход не мутируется, `percentage = totalVotes > 0 ? Math.round(votes*100/total) : 0`, `isLeading = votes > 0 && votes === maxVotes` (несколько лидеров при равенстве, ноль голосов не лидер).
- `createPollCardController` — рендерит `[data-poll-card][role=group]`, `[data-poll-question]`, `[data-poll-answer-id]` с `role="radio" | "checkbox"`, `aria-checked`, `data-poll-percent`, `data-leading`; кнопка `[data-poll-vote-btn]` («Голосовать», `disabled` при `isExpired` или пустом выборе), статус `[data-poll-status]` («Опрос завершён» / «Всего голосов: N»); `setState/getState/destroy` присутствуют.
- Границы: single-select заменяет выбор и снимает при повторном клике; multiselect ограничен `MAX_POLL_ANSWERS`; `isExpired` блокирует и выбор, и отправку; `destroy()` очищает контейнер.

## Mutation testing 5/5 (100%)
Исходник перед началом: `63b677de1f8f919aedc6316b4c044a59` (совпал с `REPORT.md`). Базовая линия до мутаций: `exit=0`, 9/9 pass.

| # | Мутация | Результат |
|---|---------|-----------|
| M1 | `Math.round` -> `Math.ceil` в расчёте `percentage` | KILLED (`exit=1`, падает тест равенства лидеров: 44.44 -> 45 вместо 44) |
| M2 | `isLeading: item.votes > 0` -> `item.votes >= 0` | KILLED (`exit=1`) |
| M3 | `MAX_POLL_ANSWERS` 10 -> 9 | KILLED (`exit=1`) |
| M4 | single-select: повторный клик `? [] : [id]` -> `? [id] : [id]` | KILLED (`exit=1`) |
| M5 | Текст статуса «Опрос завершён» -> «Опрос завершен» | KILLED (`exit=1`) |

Восстановление: файл возвращён из исходного содержимого, итоговый md5 `63b677de1f8f919aedc6316b4c044a59` — **byte-identical**. Пост-проверка после восстановления: `exit=0`, 9/9 pass.

## Замечания (неблокирующие)
- P3: подмена `Math.round` -> `Math.floor` тестами не убивается, потому что во всех фикстурах дробная часть `<= .44` (44.44 -> 44 и при округлении, и при отбрасывании). Ветка округления вверх (дробная часть `>= .5`) не покрыта. Не влияет на текущие данные, но стоит добавить кейс вида 1/3 или 2/3 при следующем касании тестов.

## Итог
Контракт выполнен, red-first следы в `REPORT.md` подтверждены форматом логов, targeted-тесты зелёные, мутации 5/5 убиты, исходный файл восстановлен байт-в-байт. Гейт `gate:worker` и `git commit` в этом ревью не запускались по указанию координатора.
