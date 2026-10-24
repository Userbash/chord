# M-07-UI — ack/read-state в UI (red-first)

Дата: 2026-10-24. Воркер: **G2 Frontend**. PM: DeepSeek.
Статус: **готово к приёмке**. Окно/live/сеть/cargo/gate/commit не использовались.

## Контракт (источник)

- `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.5: IPC
  `ack_message {channelId, messageId, lastViewed, token?}`, тело запроса — дословно
  `{token, last_viewed}`, `token` nullable и **не** bearer, `last_viewed` — счётчик
  read-state, не snowflake.
- `waves/RW-40/PM/m07-be2/REPORT.md`: `read_state()` → camelCase
  `{channelId, flags, lastMessageId, lastPinTimestamp, lastViewed, mentionCount}`;
  `null`/badge не превращаются в 0.
- `fixtures/gateway/by-type/002-READY.json`: канал `1163408591315812392` —
  `last_viewed: 4287`, `mention_count: 0`, `last_message_id: 1553370750381990038`
  (оракул теста); 157 записей, 143 с `last_viewed`, 2 badge-формы.
- `fixtures/gateway/by-type/046-MESSAGE_ACK.json`: `last_viewed: 4310` при
  `message_id: 1561348877573947515` → счётчик ≠ id сообщения.
- Политика триггера `[A]` зафиксирована в **`CONTRACTS/read-state.md`** (создан).

## Сделано

`rust/gui-spike/ui/features/chat.js`:

- `ACK_THROTTLE_MS = 3000` (политика `[A]`, экспорт) и `ACK_BOTTOM_EDGE_PX = 8`.
- `read_state()` читается при каждом триггере (локальная READY-проекция, без сети):
  запись ищется по `channelId`; `lastViewed`/`mentionCount` принимаются только как
  целые неотрицательные числа, `lastMessageId` — только строка цифр. `null`/badge →
  нет ack и нет счётчика (0 не выдумывается). Ошибка чтения — видимая заметка.
- `ackIfNeeded(id)` — единая точка ack:
  - **открытие** канала (включая повторный клик по кэшу), **фокус** `#chat-list`
    (`focus`/`focusin`), **скролл в низ** (`scrollHeight - clientHeight - scrollTop
    <= 8`);
  - один ack на `(channelId, lastViewed)`: успешный курсор повторно не уходит,
    новые курсоры — не чаще `ACK_THROTTLE_MS`; при ошибке повтор с тем же курсором
    не отправляется (нет цикла), новый курсор — новая попытка после окна;
  - `messageId` — `lastMessageId` из READY, иначе id последнего загруженного
    сообщения; индекс бенча не используется;
  - аргументы ровно `{channelId, messageId, lastViewed, token: null}`; bearer в UI
    не читается, логов ack нет;
  - синтетический бенч (`mode === "bench"`, `BENCH_CHANNEL_ID = "news"`) не вызывает
    ни `read_state`, ни `ack_message`.
- Полоса `.chat-unread-divider` одна: в бенче прежняя «20+ новых» (D-13, не тронута);
  в реальном канале `data-source="read-state"` и честный `data-mentions` из READY.
  Успешный ack: полоса скрыта (`data-cleared="true"`, `data-mentions` снят), у плитки
  канала снят `is-unread`. `mentionCount = 0/null` → полоса скрыта.
- Ошибка ack — видимая заметка `.chat-ack-note[data-state="error"][data-kind]`
  (`Unauthorized`/`RateLimited`/`AckFailed`/`ReadState`), `role="alert"`, без оверлея
  и auto-retry; текст не содержит тела ответа/токена.

`rust/gui-spike/ui/features/actions.js`:

- `Shift+Esc` больше не «сбрасывает» read-state полосу выдуманным прочитанным:
  ветка KEY-09 пропускает `data-source="read-state"`; бенч-плашка ведёт себя как
  раньше (существующий тест KEY-09 зелёный).

`rust/gui-spike/ui/components/chat.css`:

- `.chat-unread-divider[hidden] { display: none; }` (иначе `display:flex` перебивает
  UA-правило `[hidden]`), вариант `[data-source="read-state"]` на `--warning`, и
  `.chat-ack-note` на `--danger-fill`/`--text-on-danger` (токены существуют).

`rust/gui-spike/ui/app.js` — **не менялся**: `initChatScreen(onError, invoke)` уже
прокидывает единственный IPC-путь и `surface`; ack/read-state живут в `chat.js`.

`CONTRACTS/read-state.md` (новый) — источники, IPC, триггеры `[A]`, правила
троттлинга/дедупа/ошибок, UI-состояния и границы (unread без упоминаний не выводится).

`tests/unit/read-state-ui.test.mjs` (новый, 8 тестов): константа троттлинга; open →
один ack с `lastViewed` из READY (число, `token: null`, `messageId` из READY);
badge/`null` → 0 ack и не 0 в полосе; троттлинг (фокус/скролл внутри окна не
дублируют, новый курсор после окна, повтор того же курсора не дублируется);
mention-полоса из READY и сброс после успешного ack (точка в дереве тоже); 401 и 429
видны и не повторяются; синтетический бенч не ack'ает. Оракулы — READY,
MESSAGE_CREATE, G6 §3.5; fake `invoke`, без сети/окна.

## RED (до продуктового кода)

`waves/RW-40/PM/m07-ui/red.log` — тесты написаны раньше реализации:

```
✖ триггер ack ... — именованная константа (0.98ms)
✖ открытие канала → ровно один ack_message ... (29.86ms)
✖ null/badge-запись read_state не даёт ack ... (10.65ms)
✖ троттлинг ... (8.53ms)
✖ unread/mention-полоса ... (8.89ms)
✖ 401 Unauthorized ... (8.20ms)
✖ 429 RateLimited ... (6.24ms)
ℹ pass 1, fail 7
```

(Зелёным в red-прогоне остался только «бенч не ack'ает» — он и не должен был падать;
`ACK_THROTTLE_MS` отсутствовал, ack не вызывался вовсе.) Ни один assert после red не
ослаблен: правки теста — только включение «медленного сервера» (deferred ack), чтобы
наблюдать полосу до ответа, без изменения ожидаемого контракта.

## GREEN

`waves/RW-40/PM/m07-ui/green-read-state.log`,
`green-frontend.log`, `tsc.log`, `geometry.log`.

- `node --test tests/unit/read-state-ui.test.mjs` — **8 passed / 0 failed** (EXIT 0).
- Список из ТЗ (10 файлов frontend) — **335 passed / 0 failed / 0 skipped**
  (EXIT 0); ослаблений нет, бенч `CHAT_ROWS=100_000` и D-13 не тронуты.
- `npx tsc -p rust/gui-spike/ui/tsconfig.json` — EXIT 0.
- `node waves/RW-39/PM/w39c/geometry-check.mjs` — EXIT 0, 31 значение в допуске.
- `cargo`/gate/окно/live/сеть/commit не запускались (запрещено ТЗ).

## Файлы + md5

| Файл | md5 |
|---|---|
| `rust/gui-spike/ui/features/chat.js` | `fc56080139437146857bbf551b61db80` |
| `rust/gui-spike/ui/features/actions.js` | `d4dcc9e886e7869d0f96224df2204687` |
| `rust/gui-spike/ui/components/chat.css` | `47f74cbedd57ab89bfde0c0228f77f7a` |
| `tests/unit/read-state-ui.test.mjs` (new) | `80f4db2ba4c8319ed30261f8026f4812` |
| `CONTRACTS/read-state.md` (new) | `fb7ed47afc3494e667ad21d873a04ed7` |

`app.js`, `src/**`, search/voice/settings, fixtures, `logs/`, `evidence/`,
`functions/`, `waves/*` — не менялись.

## Остаток / риски

- **Unread без упоминаний**: `read_state()` не отдаёт число непрочитанных, а `flags`
  не интерпретируется без RE-оракула. Полоса показывает только `mentionCount > 0`;
  unread-индикация без упоминаний — отдельное ТЗ (проекция `badgeCount`/`flags`).
- **Живой паритет не проверен**: триггеры (`[A]`) и `token: null` — офлайн-контракт;
  реальный MESSAGE_ACK/окно не использовались. Пометка `[A]` в контракте.
- **Badge-записи** (`read_state_type: 2/4`, 2 шт.) остаются без ack и счётчиков.
- `read_state()` вызывается на каждый триггер (фокус/скролл) — локальный IPC, не
  сеть; при желании PM можно кэшировать с инвалидацией по `MESSAGE_ACK` (L-02).
- `app.js` не менялся намеренно: существующего `invoke`-пути достаточно; отдельного
  wiring для ack не требуется.
