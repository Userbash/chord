# task-192 — UI poll progress

Batch: RW-40 / G2. Ветка: feature/task-192-ui-poll-progress.

## Сделано
- `tests/unit/poll-progress.test.mjs` — red-first контракт на три функции (16 тестов, 3 сьюта).
- `rust/gui-spike/ui/features/poll-progress.js` — `computePollResults`, `formatPollExpiryRu`, `togglePollAnswerSelection`; `// @ts-check` + JSDoc, ESM-экспорты.

## Поведение
- `computePollResults`: `totalVotes` = сумма count; `percentage = Math.round(count*100/totalVotes)` или `0` при нуле; `isLeading` — все ответы с максимальным count при `totalVotes > 0`, никто при нуле. Отсутствующий count трактуется как `0`.
- `formatPollExpiryRu`: `null`/не-конечное/`<= now` → «Опрос завершён»; далее меньше минуты / мин. / ч. / дн. с `Math.ceil`.
- `togglePollAnswerSelection`: радио-режим pick/clear; мультивыбор снимает и добавляет до `maxChoices` (на лимите возвращается копия без изменений); входной массив не мутируется.

## Прогоны
- Red: `node --test tests/unit/poll-progress.test.mjs` → fail 1 (модуль отсутствует) — `red.log`.
- Green: tests 16, pass 16, fail 0 — `green.log`.

## Файлы и md5
```
4ee05978432a4181213ef34feefdcc1c  rust/gui-spike/ui/features/poll-progress.js
81cabc7fd271f65cd56debd52fb3440a  tests/unit/poll-progress.test.mjs
```

## Риски / осталось
- UI-рендер (полоса прогресса, попап выбора, таймер) не подключён — карточка покрывает только хелперы.
- Полный `gate:worker` и `tsc` не запускались (token-lean режим, вне scope).
- `git commit` не выполнялся.
