# task-241 — Poll Card & Live Vote Results Controller (FE-P1-27)

## Сделано
- `rust/gui-spike/ui/features/poll-card.js` (`// @ts-check`):
  - Экспортированы константы `MAX_POLL_ANSWERS = 10` и `MAX_POLL_QUESTION_CHARS = 300`.
  - Экспортирована чистая функция `computePollPercentages(answers, totalVotes)`:
    - возвращает `Array<{ answerId: string, text: string, votes: number, percentage: number, isLeading: boolean }>`;
    - вычисляет `percentage = totalVotes > 0 ? Math.round((votes * 100) / totalVotes) : 0`;
    - выставляет `isLeading: true`, когда `votes > 0` и `votes === maxVotes` среди всех вариантов;
    - не мутирует входной массив `answers` и его элементы.
  - Экспортирован контроллер `createPollCardController(options)`:
    - рендерит корень `[data-poll-card]` с `role="group"` и заголовок вопроса `[data-poll-question]`;
    - рендерит каждый вариант `[data-poll-answer-id="<answerId>"]` с `role="checkbox"` (при `allowMultiselect === true`) или `role="radio"` (при single-select), `aria-checked="true"|"false"`, `[data-poll-percent="<percentage>%"]` и `[data-leading="true"]` при `isLeading === true`;
    - при клике по варианту, когда `!isExpired`, переключает выбор (в single-select заменяет выбор или снимает при повторном клике; в multiselect переключает до `MAX_POLL_ANSWERS`);
    - рендерит кнопку `[data-poll-vote-btn]` (`"Голосовать"`, `disabled` при `isExpired` или 0 выбранных вариантов; клик вызывает `onSubmitVote(selectedAnswerIds)`) и статус `[data-poll-status]` (`"Опрос завершён"` при `isExpired === true`, иначе `"Всего голосов: N"`);
    - возвращает `{ setState(patch), getState(), destroy() }`.
- `tests/unit/poll-card.test.mjs`: 9 unit-тестов по контракту (Strict TDD: Red -> Green).

## Прогоны
- **Red**: `node --test tests/unit/poll-card.test.mjs` -> `exit=1` (`ERR_MODULE_NOT_FOUND`, `pass 0 / fail 1`) — лог сохранён в `red.log`.
- **Green**: `node --test tests/unit/poll-card.test.mjs` -> `exit=0` (`tests 9 / pass 9 / fail 0`) — лог сохранён в `green.log`.
- **Typecheck**: `npx tsc -p rust/gui-spike/ui/tsconfig.json --noEmit` -> `exit=0` (0 ошибок).

## Файлы и md5
- `rust/gui-spike/ui/features/poll-card.js` — `63b677de1f8f919aedc6316b4c044a59`
- `tests/unit/poll-card.test.mjs` — `c282ef5033555f40e85fc3d382c97e8c`

## Риски
- Контроллер не зависит от глобального `window.document`, если `options.container` предоставляет `ownerDocument.createElement` или `createElement`.
- `git commit` не выполнялся в соответствии с правилами воркера; изменения находятся в рабочем дереве ветки `feature/task-241-ui-poll-card`.
