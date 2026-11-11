# CONTRACTS/read-state.md — ack/read-state в UI (M-07)

Дата: 2026-10-24. Владельцы: G1 (backend, `read_state`/`ack_message`), G2 (UI).
Источники: `waves/RW-40/PARITY/G6-REPORT-RW40-02.md` §3.5, `waves/RW-40/PM/m07/REPORT.md`,
`waves/RW-40/PM/m07-be2/REPORT.md`, `fixtures/gateway/by-type/002-READY.json`,
`fixtures/gateway/by-type/046-MESSAGE_ACK.json`, `docs/re/rest-d02.md` §4.

## 1. Данные

`READY.read_state.entries[]` → IPC `read_state()` (camelCase, 9 полей, `m07-be2` + `m07-be3`):

```
{
  channelId: string,
  flags: number | null,
  lastMessageId: string | null,
  lastPinTimestamp: string | null,
  lastViewed: number | null,
  mentionCount: number | null,
  badgeCount: number | null,
  lastAckedId: string | null,
  readStateType: number | null
}
```

- `lastViewed` — **счётчик** read-state (в фикстуре `4287`), не snowflake сообщения
  (`046-MESSAGE_ACK`: `last_viewed: 4310` при `message_id: 1561348877573947515`).
- `mentionCount` — число упоминаний; `flags`, `lastViewed`, `mentionCount`, `badgeCount`,
  `lastAckedId`, `readStateType` могут быть `null`. **`null` не превращается в 0**, UI не
  выдумывает счётчик; при этом явный `badgeCount: 0` в READY сохраняется как число `0`.
- `lastAckedId` передаётся строкой (snowflake без потери точности 64-битного целого).
- `readStateType` (в фикстурах `2`, `4`) пробрасывается как `number | null`; его семантика
  на стороне клиента не домысливается без отдельного RE-оракула.
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
  в реальном канале — `data-source="read-state"` и честные счётчики из READY:
  - **приоритет упоминаний**: если `mentionCount > 0`, полоса показывает «N упоминаний»
    (`data-mentions="N"`), при этом ненулевой `badgeCount > 0` сохраняется рядом в `data-badges`
    и не подменяет/не занижает `mentionCount`;
  - **бейдж непрочитанных при отсутствии упоминаний**: если `mentionCount == 0 | null`, но
    `badgeCount > 0`, полоса показывает «N непрочитанных» (`data-badges="N"`, `data-mentions` снят);
  - **`badgeCount = 0` — значение, но не бейдж** (`CODE-STANDARD.md` §RW-17): предикат показа —
    строго `> 0`; при `mentionCount == 0 | null` и `badgeCount == 0 | null` полоса скрыта,
    `data-mentions` и `data-badges` сняты, число не выдумывается;
  - **`flags`**: числовое значение `flags` из `read_state()` записывается как есть (включая `0`)
    в `data-flags` полосы `.chat-unread-divider` и `data-read-flags` плитки канала; при `flags: null`
    оба атрибута снимаются;
  - при переключении канала `prepareReadStateBand()` сбрасывает `data-mentions`, `data-badges` и
    `data-flags`, исключая протечку состояния между каналами.
- Успешный ack: полоса скрыта (`data-cleared="true"`, `data-mentions` и `data-badges` сняты),
  у плитки канала снят `is-unread`, но серверные `flags` (`data-flags` / `data-read-flags`)
  клиентом не переписываются. `Shift+Esc` **не** подменяет ack: read-state полоса гаснет
  только подтверждением сервера.
- Ошибка ack: видимая заметка `.chat-ack-note[data-state="error"]` с `role="alert"` и
  `data-kind` (`Unauthorized|RateLimited|AckFailed|ReadState`), без оверлея и без
  автоматического повтора. Текст не содержит тела ответа и токена.

## 5. Остаток / границы

- Состояние открытого канала из READY экспонируется в `ChatScreen.readState`
  (`{flags, mentionCount, badgeCount} | null`); `flags` хранится в dataset без домысливания
  битовой маски до отдельного RE-оракула (G-08/C-03).
- Живой паритет (реальный Discord, MESSAGE_ACK в UI) не проверялся: окно/live не
  использовались. Пункт 3 помечен `[A]`.
- Non-channel badge-записи (`readStateType: 2/4`, `lastViewed: null`) остаются без ack
  (курсора канала нет; `lastAckedId` не используется как курсор канала), а для записей канала
  `badgeCount` и `flags` отображаются по правилам §4 (`m07-be3` + `m07-ui-badge`).
