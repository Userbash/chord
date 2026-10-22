# RW-17 / P2-07 — замороженный дизайн: read-state и бейджи

Статус: контракт заморожен координатором до red-тестов. Файлы:
`src/state/read_state.rs`, `src/state/read_state_reducer.rs`,
`src/state/read_state_selectors.rs`, аддитивные правки
`src/state/{types,reducer,mod}.rs`, `src/lib.rs`.
Тесты: `tests/read_state_{types,reducer,selectors}.rs`.

## 1. Scope

**Входит:** memory-only `MESSAGE_ACK` (подтверждённые capture поля `channel_id`,
`message_id`, `flags`, `last_viewed`, `version`; optional `manual`, `mention_count`),
применение к `State.read_state`, глобальная версия read-state, mention/badge селекторы,
мультиустройственная идемпотентность, stale по message-id, диагностика. Без сети, SQLite,
UI, live Discord и новых зависимостей.

**Не входит:** REST POST ACK и токен (`P1-12`), `MESSAGE_ACKED`, `USER_NON_CHANNEL_ACK`,
`GUILD_FEATURE_ACK`, unread-count (требует сообщений/политики P2-11), presence/voice/typing,
хранение на диск. `Effect::AckMessage` остаётся декларацией и P2-07 его не генерирует.

**Политика неизвестного:** `manual`/`mention_count` подтверждены официальным клиентом
(`RESEARCH-P2-07.md` §4), но в capture отсутствуют; отсутствие/`nil` → `false`/`None` без
диагностики. Неизвестные поля ACK игнорируются как неизвестные ключи события (без ошибки),
в `StateDiagnostics` не перечисляются: политика P2-02 для `MESSAGE_CREATE` такая же.

## 2. Публичные типы (`discord_core::state`, root `discord_core`)

```rust
pub struct MessageAck {
    pub channel_id: u64,
    pub message_id: u64,
    pub flags: Option<i64>,
    pub last_viewed: Option<i64>,
    pub mention_count: Option<i64>,
    pub manual: bool,
    pub version: Option<u64>,
}

impl MessageAck {
    /// Разбор `d` ACK; ошибка обязательных полей — EventError, optional — диагностика.
    pub fn parse(
        data: &discord_codec::etf::EtfValue,
    ) -> Result<(Self, StateDiagnostics), EventError>;
}
```

`EventPayload` получает `MessageAck(MessageAck)`. `Event::from_routed` маршрутизирует
`DispatchKind::MessageAck` в `MessageAck::parse`. Root re-export: `MessageAck`.

`State` не получает нового поля: канонический read-state остаётся
`ready::ReadState` (Vec записей, порядок провода). Новые записи ACK добавляются в конец.

## 3. Разбор (`MessageAck::parse` / `Event::from_routed`)

Фатальные ошибки (состояние не меняется; `apply_dispatch` пишет `malformed`, `last_seq`
не откатывается):

| Случай | Ошибка |
|---|---|
| `d` не map | `WrongType { path:"d", expected:"map", found:kind }` |
| нет/`nil`/не-u64 `channel_id` | `Missing`/`InvalidId` (`message_ack.channel_id`) |
| нет/`nil`/не-u64 `message_id` | `Missing`/`InvalidId` (`message_ack.message_id`) |

Нефатально (диагностика + дефолт):

| Поле | Отсутствие/`nil` | Неверный тип |
|---|---|---|
| `flags` | `None` | `None` + `ignored("d.flags")` |
| `last_viewed` | `None` | `None` + `ignored("d.last_viewed")` |
| `mention_count` | `None` | `None` + `ignored("d.mention_count")` |
| `manual` | `false` | `false` + `ignored("d.manual")` |
| `version` | `None` | `None` + `ignored("d.version")` |

`version` — u64 (отрицательное/дробное → `ignored`). `flags`/`last_viewed`/
`mention_count` — i64. Дубликатов внутри одного ACK нет; повторные ACK — не duplicate.

## 4. Reducer (`apply_ack`)

`pub(crate) fn apply_ack(state: &mut State, ack: MessageAck)`; вызывается из общей ветки
`EventPayload::MessageAck`.

1. Найти запись `entry.id == ack.channel_id`. Если нет — создать
   `ReadStateEntry { id: ack.channel_id, ..Default::default() }` и добавить в конец
   `entries`. Это ленивое создание, подтверждённое `ReadStateStore.get()` официального
   клиента; `partial`/порядок существующих записей не меняются.
2. Если у существующей записи `read_state_type = Some(t)`, `t != 0` — это не-channel
   запись (READY: 2 — user, 4 — guild). ACK не применяется; `inconsistencies`
   `read_state.ack:{channel_id}:non-channel:{t}`. Запись не создаётся/не меняется;
   `read_state.version` тоже не меняется — событие отклоняется целиком (осознанное
   решение, не max-guard).
3. Stale по ack-точке: если `entry.last_message_id = Some(current)` и
   `ack.message_id < current` — данные не меняются, `inconsistencies`
   `read_state.ack:{channel_id}:stale:{message_id}`; `read_state.version` не меняется.
   Это **локальная** защита от регрессии, не заявленная семантика протокола.
4. `last_message_id`: если `None` или `ack.message_id > current` — присвоить
   `Some(ack.message_id)`. Равенство — норма (фикстура `046`); запись не дублируется и
   диагностика не пишется.
5. `flags`/`last_viewed`: при `Some` присвоить; при `None` сохранить прежнее.
6. `mention_count`:
   - `manual == true` → `Some(max(mention_count.unwrap_or(0), 0))`;
   - `manual == false` → `Some(0)` (официальный `ack()` очищает mentions; `mention_count`
     non-manual игнорируется).
   `mention_count` всегда становится `Some`, включая неизвестный канал.
7. `version`: при `Some(v)` присвоить `state.read_state.version = v` (глобальный watermark;
   официальный клиент присваивает без max-guard). При `None` версия не меняется.
   Для отклонённого non-channel ACK см. §4.2: version не меняется.
8. Не меняются: `partial`, `last_pin_timestamp`, `badge_count`, `last_acked_id`,
   `read_state_type`, `Guild`/`messages`/`member_lists`, `last_seq` (его ведёт общий
   reducer). Effects не создаются.
9. Ошибка разбора не трогает read-state; общий `apply_dispatch` оставляет `malformed` и
   монотонный `last_seq`.

Stale `s` обрабатывается общим reducer до разбора, как в P2-02/P2-05.

## 5. Селекторы (`impl State`, `read_state_selectors.rs`)

```rust
mention_count(id: u64) -> i64                         // <0 и отсутствие → 0
has_mentions(id: u64) -> bool
mention_channels() -> Vec<u64>                        // >0, возрастание id
total_mentions() -> i64
guild_mention_count(guild_id: u64) -> i64             // каналы+треды гильдии
private_mention_count() -> i64                        // private_channels
badge_count(id: u64) -> i64                           // <0 и отсутствие → 0
badge_entries() -> Vec<u64>                           // badge_count >0, возрастание id
total_badges() -> i64
acknowledged_message_id(id: u64) -> Option<u64>       // channel: last_message_id;
                                                      // иначе last_acked_id → last_message_id
is_message_read(id: u64, message_id: u64) -> bool     // ack >= message_id
read_state_version() -> u64
```

`acknowledged_message_id` для channel-записи (`read_state_type` `None`/0) использует
только `last_message_id`; fallback на `last_acked_id` для канала запрещён — это поле
принадлежит non-channel семантике. `guild_mention_count` использует `channel_guild` и
`thread_guild`; неизвестные каналы и DM не входят. `private_mention_count` — только id из `private_channels`. Порядок
`mention_channels`/`badge_entries` — по возрастанию id (детерминизм независимо от порядка
провода). Все методы только читают и не паникуют.

## 6. Инварианты

1. Read-state — снимок READY + применённые ACK; порядок существующих записей стабилен,
   новые в конце.
2. `last_message_id` канала — ack-точка; `last_acked_id` — для не-channel типов.
3. Равенство message-id не дубль; повторный ACK обновляет metadata/version.
4. Меньший message-id — stale: без изменений и без отката version.
5. Non-manual ACK всегда очищает mentions; manual — выставляет значение или 0.
6. `version` — глобальный watermark; `None` не сбрасывает.
7. `message_id`/`channel_id` обязательны; optional-поля не роняют событие.
8. Нет сети/async/UI/SQLite; ошибки не глотаются — диагностика или EventError.
