# C-02-UI-OLDER — older-history UI (`position:"latest"` + `before`/`olderCursor`)

- **Воркер:** G2 Frontend, batch RW-40/PM, карточка **C-02-UI-OLDER** (P1B).
- **PM:** DeepSeek. **Дата:** 2026-10-24.
- **Офлайн:** live/окно/сеть/commit/`gate`/cargo/`npm run *` не запускались.
  Только `node --test` по целевым файлам + `npx tsc` UI-проекта.
- **Источник контракта:** `waves/RW-40/PM/c02-before/REPORT.md` §1.1–§1.3
  (`MessagesPageRequest`: `position:"start"|"latest"`, `before`, `hasOlder`,
  `olderCursor`), `waves/RW-40/PM/c02-page/REPORT.md` (forward-ветка, окно 500,
  prepend-компенсация), `waves/RW-40/PARITY/G6-REPARITY-04.md` §4 блокер 3.
- **Blocker:** нет. Backend уже отдаёт хвост и older-страницы; правка — только UI.

## 1. Что сделано

`rust/gui-spike/ui/features/chat.js` (единственный продуктовый файл):

- **Открытие канала — на последнем сообщении.** `openChannel` шлёт
  `messages_page({channelId, position:"latest", limit:100})` — без `cursor`,
  `before` и `token`. Окно канала (`ChannelWindow`) получает **две независимые
  пары курсоров**: `nextCursor`/`hasMore` (вниз) и `olderCursor`/`hasOlder`
  (вверх). После ответа `scrollTop` ставится в низ списка (`scrollChannelToEnd`),
  т.е. пользователь видит хвост, а не начало истории.
- **Скролл вверх.** Новый триггер `CHANNEL_LOAD_TOP_EDGE_PX = 96` в том же
  обработчике `scroll`: у верхнего края вызывается `loadOlderHistory`, который
  шлёт `messages_page({channelId, before: olderCursor, limit:100})` — `before` и
  `limit` единственные аргументы, `cursor` не отправляется.
- **Prepend + позиция.** Строки приходят в возрастающем порядке; склейка —
  существующим `mergeChannelItems` (дедуп по `id`, порядок backend). При
  `prepended > 0` окно перерисовывается и `scrollTop` компенсируется на
  `scrollHeight`-delta — строка под курсором остаётся на месте. В DOM-шиме
  layout не считается (delta = 0), поэтому тест проверяет точное сохранение
  `scrollTop` и отсутствие сброса в 0.
- **Направления не смешиваются.** `loadOlderHistory` не читает и не пишет
  `nextCursor`/`hasMore`; `loadMoreHistory` не переписывает
  `olderCursor`/`hasOlder`. Это не косметика: `before`-страница тоже возвращает
  forward-метаданные своего окна (C-02-BEFORE §6.2), и принятие её `nextCursor`
  за курсор «вниз» увело бы forward-пагинацию назад. Метаданные читаются одним
  экспортируемым разбором `pageMeta` (camelCase, пустая строка = нет курсора).
- **Остановка.** `hasOlder=false` → верхняя полоса `end` («Это начало истории»),
  повторные скроллы вверх запросов не шлют. Битый ответ «тот же `olderCursor`
  без новых строк» (`stalled`) тоже останавливает направление.
- **Окно ≤ 500 строк** (`CHANNEL_WINDOW_ROWS`, `data.rs UI_WINDOW_ROWS`): guard
  стоит и в older-ветке; при заполнении окна полоса показывает `limit`, дальше
  запросов нет. Запросы двух направлений сериализованы (`loading`/`olderLoading`)
  — окно мутирует ровно один ответ.
- **Ошибка/retry.** Верхняя полоса `#chat-list .chat-history-older`:
  `data-page-state = idle|loading|error|end|limit`, `role="status"|"alert"`,
  видимый текст через `channelFailureText` (404/401/429), кнопка «Повторить»
  повторяет **тот же** `before`. Уже загруженные строки не теряются, список не
  превращается в пустой; `aria-busy` снимается.
- **Верхняя полоса — не строка.** Первый ребёнок `#chat-list` (не участвует в
  подсчёте строк/`aria-setsize`/геометрии), состояние — в `data-page-state`, а не
  в `data-state`: `#chat-list [data-state]` остаётся контрактом состояний канала
  C-02-UI (loading/empty/error). Нижний подвал `#chat-list .chat-history-more`
  не менялся по смыслу.
- **Порядок триггеров на вырожденной геометрии.** Если контент короче окна
  (в шиме — всегда: `scrollHeight == clientHeight`), «достигнуты» оба края.
  Сначала вызывается older-, затем forward-ветка: older без `hasOlder`/курсора
  возвращается синхронно и не блокирует forward; окно мутирует один запрос.
- **Бенч-путь не тронут:** `CHAT_ROWS=100_000`, sizer, `PageStore`,
  виртуализация, `renderChatRow`/`makeMessage` не менялись; в бенч-режиме
  `onScrollHistory` выходит по `isChannelMode()`.

Не менялись: `ui/app.js`, `ui/components/chat.css`, `ui/features/actions.js`,
`rust/gui-spike/src/**`, `tests/support/gui-dom-harness.mjs`, `logs/`,
`functions/`, `evidence/`, `waves/db.jsonl`, `STATE.md`, `TASKS.md`.

## 2. Red (red-first)

Тесты дописаны до кода: обновлено открытие канала (`position:"latest"`) и
6 новых тестов older-истории. Прогон против **старого** `chat.js`
(`position` не отправляется, ветки `before` нет):

```
node --test tests/unit/channel-paging.test.mjs tests/unit/channel-switch.test.mjs
EXIT=1
ℹ tests 24  ℹ pass 17  ℹ fail 7        (red.log)
```

Падения — по правильной причине (нет поведения, а не опечатка теста):

```
✖ открытие канала — position=latest, limit=100, без cursor/before/token
  actual: { channelId: '1163408591315812392', limit: 100 }
✖ хвост канала: position=latest, нижний подвал end, верхняя полоса idle
  AssertionError: нет #chat-list .chat-history-older
✖ before=olderCursor, prepend с дедупом… («скролл вверх не запросил более раннюю историю»)
✖ направления не смешиваются …
✖ ошибка 404 / ошибка 401: alert сверху…  («нет #chat-list .chat-history-older»)
✖ окно истории не растёт сверх 500 строк при подгрузке вверх
```

`channel-switch.test.mjs` (10 тестов) был зелёным и до, и после — регрессий
переключения каналов/ids нет.

## 3. Green

```
node --test tests/unit/channel-paging.test.mjs tests/unit/channel-switch.test.mjs
EXIT=0  ℹ tests 24  ℹ pass 24  ℹ fail 0  ℹ skipped 0        (green.log)

node --test tests/unit/{chat-reactions,read-state-ui,typing-ui,dispatch_ui,gui-spike-dom}.test.mjs
EXIT=0  ℹ tests 109 ℹ pass 109 ℹ fail 0                        (adjacent.log)

node --test tests/unit/gui-spike-dom.test.mjs                  (bench-путь)
EXIT=0  ℹ tests 29  ℹ pass 29  ℹ fail 0                        (bench.log)

npx tsc -p rust/gui-spike/ui/tsconfig.json
EXIT=0  (0 ошибок)                                             (tsc.log)
```

Было 8 тестов в `channel-paging`, стало 14 (+6 новых), ни один существующий
assert не удалён и не ослаблен.

Покрытие (`channel-paging.test.mjs`, оракул — `MESSAGE_CREATE`-кадры канала
`1163408591315812392` + контракт `data.rs`/C-02-BEFORE, не реализация):

1. Открытие: ровно один запрос, аргументы `{channelId, position:"latest",
   limit:100}`, нет `cursor`/`before`/`token`; строки — хвост оракула; нижний
   подвал `end` (hasMore=false), верхняя полоса `idle` с подсказкой.
2. Скролл вверх: `{channelId, before: olderCursor, limit:100}`, без `cursor`;
   prepend перед хвостом, дедуп повторной граничной строки, `scrollTop` = 64
   (не сброшен), `hasOlder=false` → `end` и повторные скроллы не шлют запросов.
3. Направления: older-страница возвращает свой `nextCursor` (= id граничной
   строки) — forward-запрос после неё уходит по **исходному** `nextCursor`
   хвоста, не по курсуру older-страницы; `before` в forward-запросе нет.
4. Ошибки 404/401 подгрузки вверх: полоса `role="alert"` с текстом, строки
   хвоста целы, `aria-busy=false`, retry догружает страницу и даёт `end`.
5. Окно: 6 скроллов вверх на синтетике 700 строк → ≤5 запросов, ровно 500
   строк, все id уникальны, полоса `limit`; шестой запрос не уходит.

Мутации (копия UI в `/tmp`, сдаваемые файлы не мутировались; `GUI_DOM_APP`
копии, лог `mutant-*.log`) — все caught:

| Мутант | Ожидание | Результат |
|---|---|---|
| `before` → `cursor` в older-запросе | тесты older падают | `pass 9 / fail 5` (mutant-a.log) |
| older-ответ перезаписывает `nextCursor`/`hasMore` | падает тест направлений | `pass 13 / fail 1` (mutant-b.log) |
| prepend без компенсации `scrollTop` (=0) | падает тест позиции | `pass 13 / fail 1` (mutant-c.log) |
| `hasOlder = true` всегда | падают тесты остановки | `pass 11 / fail 3` (mutant-d.log) |

## 4. Изменения тестов (TEST-POLICY §5)

| Тест | Было | Стало | Основание |
|---|---|---|---|
| `C-02-PAGE: первая страница` | `{channelId, limit:100}`, без `cursor` | `{channelId, position:"latest", limit:100}`, без `cursor`/`before` | контракт C-02-BEFORE §1.1/§1.3 + карточка C-02-UI-OLDER п.1 |
| `pagePayload()` helper | без older-полей | `hasOlder`/`olderCursor` (default `false`/`null`) | `MessagesPage` из `data.rs` (9 ключей, §1.2) |
| остальные 7 тестов `channel-paging` | — | не менялись (assert не ослаблен) | — |
| `channel-switch.test.mjs` | — | файл не менялся | — |

Новые тесты ловят старое поведение (см. red.log и мутантов), а не подогнаны под
реализацию: ожидания построены на оракуле кадров и на документированном
контракте backend.

## 5. Файлы и md5

| Файл | md5 (после) | md5 (до) |
|---|---|---|
| `rust/gui-spike/ui/features/chat.js` | `7098d413402c7cc6035f825bc347a286` | `2e918f3cff5dc968ca3117480e306ce4` |
| `tests/unit/channel-paging.test.mjs` (изменён) | `5feceaadeee6cab66c60d9bd45c61fe3` | `54184e41ce632d3fdb47cec9672095e0` |
| `tests/unit/channel-switch.test.mjs` (не менялся) | `2c97686949352f7b576bbdf57c13b2b4` | `2c97686949352f7b576bbdf57c13b2b4` |

Артефакты: `red.log`, `green.log`, `adjacent.log`, `bench.log`, `tsc.log`,
`mutant-a…d.log` — в этой папке.

## 6. Остаток / риски (нужно решение PM/Team Lead)

1. **CSS верхней полосы не добавлен** — карточка разрешает только
   `ui/features/chat.js` + целевые тесты, `ui/components/chat.css` вне матрицы.
   Разметка использует `.chat-history-older`/`.chat-history-older-text`/
   `.chat-history-older-retry` по образцу нижнего подвала, но без стилей;
   для паритета нужна отдельная CSS-карточка (высота/токены/состояния).
2. **`insertBefore`-ветка не покрыта шимом.** В браузере полоса вставляется
   через `insertBefore`, в шиме (метода нет) — `replaceChildren` с сохранением
   тех же узлов; тесты идут по шим-ветке. Реальный layout/scroll-anchoring не
   измерялся (окно/live запрещены).
3. **Forward-ветка в tail-режиме де-факто не срабатывает** (хвост — самая
   новая страница, `hasMore=false`), но сохранена и покрыта тестами как
   отдельный контракт (п.3 карточки). Если PM решит удалить «догрузку вниз» —
   отдельная карточка с правкой тестов.
4. **Предел 100 строк на страницу** (`MAX_PAGE_LIMIT`) не повышался; окно 500
   набирается пятью страницами вверх. Если нужен `limit` до 500 — отдельная
   карточка (как и в C-02-BEFORE §6.3).
5. **Live/RE не проверялись:** формы `MessagesPage`, `hasOlder`/`olderCursor`
   взяты из `data.rs` и отчёта C-02-BEFORE; реальный Discord REST history
   (`before=`/`after=`) в `net.rs`/`messages.rs` не реализован — BFF на фикстурах.
6. **Приоритет верха на вырожденной геометрии** (контент короче окна): один
   скролл обслуживает верх, следующий — низ. В браузере на реальных каналах
   диапазоны не пересекаются; правило зафиксировано комментарием в коде.
7. Приёмка/мутации-ревью/`waves/db.jsonl`/`STATE.md`/доки — за Team Lead/PM.
   **Blockers нет.**
