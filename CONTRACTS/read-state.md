# CONTRACTS/read-state.md — ack/read-state в UI (M-07)

Дата: 2026-10-24. Владельцы: G1 (backend, `read_state`/`ack_message`), G2 (UI).
Источники: `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.5, `waves/RW-40/PM/m07/REPORT.md`,
`waves/RW-40/PM/m07-be2/REPORT.md`, `fixtures/gateway/by-type/002-READY.json`,
`fixtures/gateway/by-type/046-MESSAGE_ACK.json`, `docs/re/rest-d02.md` §4.

## 1. Данные

`READY.read_state.entries[]` → IPC `read_state()` (camelCase, `m07-be2`):

```
{ channelId, flags, lastMessageId, lastPinTimestamp, lastViewed, mentionCount }
```

- `lastViewed` — **счётчик** read-state (в фикстуре `4287`), не snowflake сообщения
  (`046-MESSAGE_ACK`: `last_viewed: 4310` при `message_id: 1561348877573947515`).
- `mentionCount` — число упоминаний; `flags`, `lastViewed`, `mentionCount` могут быть
  `null` (badge-записи `read_state_type: 2/4`). **`null` не превращается в 0**, UI не
  выдумывает счётчик.
- UI сопоставляет запись по `channelId`; порядок READY не пересортировывается.
- Записи без `lastViewed` (или badge-формы) не дают ack: курсора нет.

## 2. IPC ack (G6 §3.5)

```
ack_message { channelId, messageId, lastViewed, token }
```

- `channelId` — реальный snowflake канала; синтетический бенч `news` **не** ack'ается.
- `messageId` — `lastMessageId` из READY, иначе id последнего реально загруженного
  сообщения канала. Индекс бенч-строки не используется.
- `lastViewed` — целое неотрицательное число из READY (не строка, не id сообщения).
- `token` — всегда `null` из UI: bearer в UI не читается и в аргументы не попадает.
- Тело запроса на backend — дословно `{token, last_viewed}` (`routes.rs body_keys`).

## 3. Политика триггера — `[A]` (наше решение, не паритет)

Триггеры ack для **открытого реального канала**:

1. открытие канала (`openChannel`, включая повторный клик по кэшу);
2. фокус `#chat-list` (`focus`/`focusin`);
3. скролл в низ списка (`scrollHeight - clientHeight - scrollTop <= 8px`).

Правила:

- **Один ack на `(channelId, lastViewed)`.** Успешно подтверждённый курсор повторно не
  отправляется, даже после окна троттлинга.
- **Троттлинг** `ACK_THROTTLE_MS = 3000` мс на канал (именованная константа в
  `ui/features/chat.js`, часы — `performance.now`). Внутри окна новый курсор ждёт.
- **Ошибка не создаёт цикл.** После неуспешного ack повтор с тем же
  `lastViewed` не отправляется; новый курсор из READY — новая попытка после окна.
- Синтетическое бенч-окно (`CHAT_ROWS = 100_000`, `BENCH_CHANNEL_ID = "news"`) не
  вызывает ни `read_state`, ни `ack_message`.

## 4. UI

- Полоса `.chat-unread-divider` одна: в бенче — статичное «20+ новых» (D-13),
  в реальном канале — `data-source="read-state"` и честный `mentionCount` из READY
  (`data-mentions`). `mentionCount = 0`/`null` → полоса скрыта, число не выдумывается.
- Успешный ack: полоса скрыта (`data-cleared="true"`, `data-mentions` снят), у плитки
  канала снят `is-unread`. `Shift+Esc` **не** подменяет ack: read-state полоса гаснет
  только подтверждением сервера.
- Ошибка ack: видимая заметка `.chat-ack-note[data-state="error"]` с `role="alert"` и
  `data-kind` (`Unauthorized|RateLimited|AckFailed|ReadState`), без оверлея и без
  автоматического повтора. Текст не содержит тела ответа и токена.

## 5. Остаток / границы

- Unread **без** упоминаний из `read_state()` не выводится: в проекции нет числа
  непрочитанных; `flags` не интерпретируется как «unread» без RE-оракула.
- Живой паритет (реальный Discord, MESSAGE_ACK в UI) не проверялся: окно/live не
  использовались. Пункт 3 помечен `[A]`.
- Badge-записи (`read_state_type: 2/4`) остаются без ack и без счётчиков; отдельная
  проекция `badgeCount`/`lastAckedId` — вне контракта (G1, при необходимости).
