# RW-17 / P2-07 — read-only research: read-state и бейджи

Статус: research завершён, дизайн ещё не заморожен. Источники только чтение:
`fixtures/`, `/home/sanya/re-deep` (локальный RE-снимок), текущий Rust-код. Живой Discord,
сеть и запись в golden не использовались.

## 1. Что уже есть в проекте

- `ready::ReadState` / `ReadStateEntry` (P2-01): 157 записей READY, версия 128949,
  поля `id`, `flags`, `last_message_id`, `last_pin_timestamp`, `mention_count`,
  `last_viewed`, `badge_count`, `last_acked_id`, `read_state_type`.
- `State.read_state` хранит снимок READY как `Vec<ReadStateEntry>` (порядок провода).
- `State::read_state_entry(id)` — единственный существующий селектор read-state.
- `Effect::AckMessage { channel_id, message_id }` уже объявлен в P2-02, но не создаётся
  reducer-ом: это декларация на будущий REST ACK.
- Общий stale-guard reducer-а отбрасывает `s <= last_seq` до разбора; `READY` сбрасывает
  последовательность.
- `MESSAGE_ACK` уже входит в `DispatchKind` (21 наблюдаемый тип), но в
  `Event::from_routed` попадает в `Unsupported`.

## 2. Фикстура `046-MESSAGE_ACK.json`

```json
{"d":{"channel_id":"1293192721237938187","flags":0,"last_viewed":4310,
      "message_id":"1561348877573947515","version":128950},"op":0,"s":38,"t":"MESSAGE_ACK"}
```

Сверка с READY-фикстурой (`002-READY.json`):

| Параметр | READY | ACK `046` |
|---|---|---|
| `read_state.version` | 128949 | 128950 |
| канал | `1293192721237938187` присутствует | тот же |
| `last_message_id` канала | `1561348877573947515` | `message_id` тот же |
| `last_viewed` | 4309 | 4310 |
| `flags` | 0 | 0 |
| `mention_count` | 0 | поля нет |
| `manual` | — | поля нет |

Вывод: ACK может подтверждать уже записанный `last_message_id` (равенство — норма, не
дубль). Он обновляет `last_viewed`, `flags`, версию read-state; `mention_count` в capture
не приходит.

READY-оракул:

- `read_state.entries|length` = **157**;
- `read_state.version` = **128949**, `partial=false`;
- положительных `mention_count` — **0**, сумма = **0** (155 нулей, 2 отсутствия);
- `badge_count` есть у **2** записей: `384299970632876032` = 16 (`read_state_type=2`,
  `last_acked_id=1294268550826950656`), `1014537851418980372` = 0 (`read_state_type=4`,
  `last_acked_id=1237379809395605509`);
- `last_acked_id` есть только у этих двух записей; у каналов ack-точка — `last_message_id`.

## 3. Capture `/home/sanya/re-deep/gw-proto/msgs-*.jsonl`

Всего **8** `MESSAGE_ACK` в union снимков; 5 каналов, 6 разных `version`:

| Файл | `s` | канал | `message_id` | `version` | `flags` |
|---|---:|---|---|---:|---:|
| run1-57992_5112 | 122 | 1176785373989445703 | 1561379697990959244 | 128949 | 1 |
| run2-95974_10029 | 17 | 1176785373989445703 | 1561379697990959244 | 128953 | 1 |
| run2-95974_10029 | 109 | 1014537851867775060 | 1561632596763353163 | 128954 | 1 |
| run2-95974_1389 | 38 | 1293192721237938187 | 1561348877573947515 | 128950 | 0 |
| run2-95974_1389 | 82 | 645271072786350111 | 1552367095730278420 | 128951 | 1 |
| run2-95974_5532 | 58 | 1176785373989445703 | 1561379697990959244 | 128951 | 1 |
| run2-95974_5532 | 79 | 1056563701764857896 | 1561624484056793151 | 128952 | 1 |
| run2-95974_8269 | 145 | 1056563701764857896 | 1561632605491830906 | 128953 | 1 |

Факты:

- обязательные поля во всех 8: `channel_id`, `message_id`, `version`; `flags` 0/1;
  `last_viewed` всегда 4310; `mention_count`/`manual` отсутствуют;
- один и тот же канал `1176785373989445703` повторно ACK-ает **тот же** `message_id`
  с version 128949/128951/128953 — это нормальная межсессионная/мультиустройственная
  синхронизация, а не duplicate-событие;
- `version` — глобальный watermark read-state, растёт 128949…128954, не per-channel;
- ACK `message_id` в capture не меньше READY `last_message_id`; монотонность по каналу
  наблюдаема, но протоколом не доказана;
- `last_viewed=4310` совпадает с официальной формулой «дней от Discord epoch»
  (`recordLastViewedTime`), не является message id.

## 4. Официальный клиент (локальный снимок, read-only)

Источник: `/home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js`
и `pretty/mod-595528.pretty.js`. Это уже добытый RE-артефакт, не новый live-захват.

Gateway decoder `MESSAGE_ACK`:

```js
eH(["MESSAGE_ACK"], e => f.D.loadGuildFromChannelId(e.channel_id), e => {
  eW({type:"MESSAGE_ACK", channelId:e.channel_id, messageId:e.message_id,
      manual:e.manual, newMentionCount:e.mention_count, version:e.version})
})
```

Read-state reducer:

```js
function e1({channelId,messageId,manual,newMentionCount}) {
  const a = ew.get(channelId);
  return manual
    ? (/* ... */ a.rebuildChannelState(messageId, true, newMentionCount),
       a.clearOutgoingAck(), true)
    : messageId !== a._ackMessageId && a.ack({messageId, local:true, trackAnalytics:false});
}
```

`ack()`:

```js
unreadCount=0; mentionCount=0; isMentionLowImportance=false;
if (messageId != null || (messageId=lastMessageId) != null) ackMessageId=messageId;
```

`rebuildChannelState(messageId, true, newMentionCount)` сбрасывает `mentionCount=0`, затем
`if (newMentionCount != null) mentionCount = newMentionCount`.

Cache watermark:

```js
actions = {CONNECTION_OPEN:e=>readStateVersion=e.readState.version,
           MESSAGE_ACK:e=>readStateAction(e), ...};
handleReadStateAction(e){ if (e.version != null) readStateVersion=e.version; }
```

CONNECTION_OPEN read-state:

```js
entries.forEach(e => {
  const i = ew.get(e.id);
  i._mentionCount = e.mention_count ?? 0;
  i.flags = e.flags; i.lastViewed = e.last_viewed;
  i.ackMessageId = e.last_message_id;        // channel type
  // non-channel: id/read_state_type/badge_count/last_acked_id
});
```

Следствия для контракта:

1. `manual` и `mention_count` — подтверждённые optional-поля ACK, даже если в capture их
   нет; их отсутствие не ошибка.
2. Non-manual ACK очищает mention count (0), даже если `mention_count` пришёл; manual —
   выставляет `mention_count.unwrap_or(0)`.
3. `mention_count < 0` семантически эквивалентен 0 (в клиенте проверка `>0`, иначе 0).
4. Равенство `message_id` текущей ack-точке — не duplicate; metadata (flags/last_viewed/
   version) может обновляться.
5. `version` — глобальный watermark, не per-entry; при ACK официальный клиент присваивает
   его read-state версии без max-guard. Мы сохраняем это подтверждённое поведение, но
   stale-политика по `message_id` ниже — локальная защита, а не заявленный протокол.
6. `last_message_id` канала — ack-точка (так READY сеет `ackMessageId`); `last_acked_id`
   используется для не-channel записей (`read_state_type` 2/4 в READY).

## 5. Границы и неизвестное

**Входит в P2-07:** memory-only разбор `MESSAGE_ACK`, применение к `State.read_state`,
глобальная версия, mentions/бейджи, мультиустройственное last-wins/идемпотентность,
stale по message-id, диагностика, селекторы. Без сети, SQLite, UI, live.

**Не входит / не изобретается:**

- REST POST `/channels/{id}/messages/{id}/ack` (P1-12 уже имеет API-фасад; P2-07 только
  декларирует `Effect::AckMessage`, если понадобится UI-инициатива);
- `MESSAGE_ACKED` и токен ACK;
- unread-count: в READY/MESSAGE_ACK нет количества непрочитанных; счётчик требует
  сообщений и политики P2-11. P2-07 не выдумывает `unread_count`;
- `USER_NON_CHANNEL_ACK` / `GUILD_FEATURE_ACK` (другие типы; P2-07 их не разбирает);
- серверная семантика `version` при race; реализуем подтверждённое присваивание плюс
  локальный stale-guard по `message_id`;
- `presences`/голос/typing — P2-09…P2-11.

## 6. Оракулы для тестов

```bash
jq '[.d.read_state.entries|length, .d.read_state.version,
     ([.d.read_state.entries[].mention_count//0]|add),
     ([.d.read_state.entries[]|select(.badge_count!=null)]|length)]' \
   fixtures/gateway/by-type/002-READY.json
# [157,128949,0,2]
jq '{d:.d,s:.s}' fixtures/gateway/by-type/046-MESSAGE_ACK.json
# channel 1293192721237938187, message 1561348877573947515, version 128950, flags 0,
# last_viewed 4310, s 38
```

Синтетические кейсы (manual/mention/stale) сверяются с официальным reducer-ом §4 и
замороженным DESIGN-P2-07.md, а не с текущей реализацией.
