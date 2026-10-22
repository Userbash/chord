# DESIGN P2-02 — модель состояния и чистый reducer

Статус: **заморожен** 2026-10-20 до red-прогона. Основание — `RESEARCH.md`, P2-01,
`docs/rust-client/ARCHITECTURE.md` §3. Без SQLite/UI/сети/новых зависимостей.

## 1. Контракт

```rust
pub fn apply(state: State, event: Event) -> (State, Effects);
pub fn apply_dispatch(state: State, dispatch: &DispatchEvent) -> (State, Effects);
pub fn apply_frame(state: State, frame: &EtfValue) -> (State, Effects);
```

`apply` — чистая функция: не читает часы, сеть, ФС, env; не паникует на входных данных.
`apply_dispatch`/`apply_frame` — адаптеры к уже принятому `dispatch::route_dispatch`:
ошибка разбора превращается в диагностику, а не в панику. Все три возвращают владение
`State`, поэтому вызывающий может публиковать снапшот без гонок.

`State::from_ready(&Ready) -> State` — единственный конструктор начального состояния.
Он не исполняет I/O и переносит только данные READY; `raw` и токены не сохраняются.

## 2. Модель состояния

```text
State {
  user: User, session_id: String, session_type, resume_gateway_url,
  guilds: BTreeMap<u64, Guild>,
  private_channels: BTreeMap<u64, PrivateChannel>,
  users: BTreeMap<u64, User>,
  relationships: BTreeMap<u64, Relationship>,
  read_state: ReadState,
  user_guild_settings: UserGuildSettings,
  sessions: Vec<Session>,
  messages: BTreeMap<u64, Message>,
  threads: BTreeMap<u64, Thread>,
  geo_ordered_rtc_regions: Vec<String>,
  country_code, subdivision_code, user_settings_proto,
  diagnostics: StateDiagnostics,
  last_seq: Option<u64>,
  channel_guild: BTreeMap<u64, u64>,   // приватный индекс
  thread_guild: BTreeMap<u64, u64>,    // приватный индекс
}
```

Инварианты:

1. Ключ `BTreeMap` равен `id` значения; порядок обхода — по возрастанию id.
2. `channel_guild[channel_id] = guild_id`; канал хранится в `guilds[guild_id].channels`.
3. `thread_guild[thread_id] = guild_id`; тред хранится в `threads[thread_id]`.
4. `messages` индексируются по id; канал сообщения не меняется от `MESSAGE_UPDATE`.
5. `users` всегда содержит `State.user`; авторы сообщений добавляются/сливаются.
6. `read_state`/`user_guild_settings` — снимки P2-01 без переинтерпретации.
7. `last_seq` — последняя **применённая** последовательность; `READY` сбрасывает её.
8. `State::Debug` не печатает `session_id`, `user_settings_proto` и `raw` (raw не хранится).

## 3. Типы событий

```rust
pub struct Event { seq: Option<u64>, payload: EventPayload, diagnostics: StateDiagnostics }

pub enum EventPayload {
  Ready(Ready),
  ReadySupplemental(ReadySupplemental),
  Resumed,
  SessionsReplace(Vec<Session>),
  ThreadListSync(ThreadListSync),
  ChannelInfo(ChannelInfo),
  MessageCreate(Message),
  MessageUpdate(MessagePatch),
  Unsupported(DispatchKind),
  Unknown { name: String },
}

pub struct ReadySupplemental {
  guild_ids: Vec<Option<u64>>,          // сырой порядок d.guilds
  merged_members: Vec<Vec<GuildMember>>,// сырой индекс i ↔ guild_ids[i]
}

pub struct ChannelInfo { guild_id: u64, channels: Vec<Channel> }
pub struct ThreadListSync { guild_id: u64, threads: Vec<Thread> }
```

`Event::from_frame(&EtfValue)` использует `route_dispatch`; `Event::from_routed` —
чистый разбор уже маршрутизированного события. Неизвестный `t` — `Unknown` (не ошибка).
Известный, но не входящий в P2-02 тип — `Unsupported` (не ошибка).

Ошибки разбора (`EventError`): `NotDispatch`, `Missing{path}`, `WrongType{path,expected,found}`,
`InvalidId{path,value}`, `Ready(ReadyError)`. В `apply_dispatch` они попадают в
`diagnostics.malformed` строкой `event:<Display>`; `last_seq` обновляется, данные не меняются.

## 4. Слияние снимков

### READY

Заменяет данные состояния целиком; диагностика накапливается (история наблюдений не
теряется). `last_seq = event.seq`. Эффекты — `SubscribeMembers` для каждой lazy-гильдии с
`member_count > 0`, диапазон `[0, 99]`, порядок — по возрастанию `guild_id`.

### READY_SUPPLEMENTAL

- `merged_members[i]` заменяет `guilds[guild_ids[i]].members`; отсутствующая гильдия или
  невалидный raw-индекс — `inconsistencies`.
- Длина групп сверяется с длиной `d.guilds`; при наличии `merged_members` и расхождении —
  `malformed`. P2-01 dedup уже применён: 138 сырых записей дают 137 уникальных
  (в первой группе дубликат `user_id`), состояние хранит 137.
- `voice_states`/`merged_presences` не разбираются (P2-10/P2-09), но и не считаются ошибкой.
- Событие не меняет `guilds`, `users`, `private_channels`.

### RESUMED

Ничего не меняет; обновляет `last_seq`. `d` — отсутствует/`nil`/map.

### SESSIONS_REPLACE

Заменяет `sessions` целиком. Дубликаты `session_id` — last wins, позиция первого,
`duplicates`. Пустой список допустим.

### THREAD_LIST_SYNC

Upsert по `thread.id`: существующий тред заменяется, новый добавляется; `thread_guild`
обновляется. Отсутствующие в событии треды не удаляются (P3-07). Неизвестная гильдия —
заглушка + `inconsistencies`.

### CHANNEL_INFO

Upsert каналов по id в гильдию `guild_id`; при неизвестной гильдии — минимальная заглушка
(`unavailable=true`) + `inconsistencies`. Дубликаты id в событии — last wins. Если канал
уже принадлежал другой гильдии — переносится с `inconsistencies`.

### MESSAGE_CREATE / MESSAGE_UPDATE

- `MESSAGE_CREATE`: upsert сообщения по id; повтор id — last wins + `duplicates`.
  Автор сливается в `users` (непустые поля перекрывают, `bot` — логическое ИЛИ).
  Неизвестный канал не отменяет сообщение, но даёт `inconsistencies`.
- `MESSAGE_UPDATE`: поля `Some` перекрывают; `None` — «не менять» (включая `nil`).
  Если сообщение неизвестно и есть `channel_id` — создаётся заготовка + `inconsistencies`;
  без `channel_id` — `malformed`.
  Смена `channel_id` запрещена (сохраняется старый, `inconsistencies`); `guild_id` может
  дополнить `None`.
- `attachments`/`embeds` хранятся счётчиками; `mentions`/`mention_roles` — id.

## 5. Эффекты (только декларации)

```rust
pub enum Effect {
  FetchMessages { channel_id: u64, before: Option<u64>, limit: u16 },
  SubscribeMembers { guild_id: u64, ranges: Vec<MemberRange> },
  AckMessage { channel_id: u64, message_id: u64 },
  RequestPresence { guild_id: u64, user_ids: Vec<u64> },
}
pub struct MemberRange { start: u32, end: u32 } // включительно
pub struct Effects(Vec<Effect>);
```

P2-02 фактически порождает только `SubscribeMembers` (READY). Остальные варианты —
замороженный интерфейс для P2-06/P2-07/P2-09; их исполнение и сеть — не здесь.
Порядок эффектов детерминирован: по возрастанию `guild_id`, внутри — фиксированные поля.

## 6. Детерминизм и последовательности

- Все индексы — `BTreeMap`; `Vec` сохраняет порядок входа/первого вхождения.
- Одинаковые `(state, event)` → структурно равные `(state, effects)`.
- `seq` монотонен: событие с `seq <= last_seq` игнорируется (`stale_sequences`), кроме
  `READY`, который всегда открывает новую последовательность.
- `seq = None` не отбрасывается и не сбрасывает `last_seq`.
- Повторное применение одного и того же события при неизменившемся `state` даёт тот же
  результат; повтор после применения отсекается как stale (кроме READY).
- Ошибки/неизвестные события не роняют reducer и не меняют данные.
- Stale-проверка выполняется и для неразобранного dispatch: ошибка разбора не откатывает
  `last_seq`; `READY` остаётся исключением. Битый старый кадр попадает в `stale_sequences`,
  а не в `malformed`.

## 7. Диагностика

```text
StateDiagnostics {
  unknown_top_level, malformed, duplicates, ignored, unknown_events,
  unsupported_events, stale_sequences, inconsistencies
}
```

- `unknown_top_level`/`malformed`/`duplicates`/`ignored` наследуются из READY.
- `unknown_events` — точные имена `t`; `unsupported_events` — имена известных 21 типа.
- `stale_sequences` — значения `s`.
- `inconsistencies` — ссылочные несоответствия (неизвестная гильдия/канал, смена канала).
- Ошибки не глотаются: либо данные, либо диагностика.

## 8. Селекторы

`State` предоставляет: `guild`, `channel` + `channel_guild_id`, `channels_in_guild`
(сортировка `position`, затем `id`), `private_channel`, `user`, `relationship`,
`message`, `messages_in_channel` (по возрастанию id), `thread`, `threads_in_guild`,
`guild_member`, `read_state_entry`, `user_guild_settings_entry`, счётчики.
Отсутствие — `None`/пустой вектор, без паники.

## 9. Границы файлов

| Владелец | Файлы |
|---|---|
| W13a | `rust/crates/discord-core/src/state/types.rs`, `tests/state_types.rs` |
| W13b | `rust/crates/discord-core/src/state/reducer.rs`, `tests/state_reducer.rs` |
| W13c | `rust/crates/discord-core/src/state/selectors.rs`, `tests/state_selectors.rs` |
| координатор | `src/state/mod.rs`, `src/lib.rs`, видимость `ready/guild.rs`/`ready/social.rs`, `tests/support/state_fixture.rs`, `tests/support/mod.rs`, `waves/RW-13/*` |

Запрещено: чужие тесты, `logs/`, `functions/`, `evidence/`, `/home/sanya/re-deep`,
`Cargo.toml`, сеть, SQLite/UI/новые зависимости.

## 10. Оракулы и приёмка

- READY-числа: `jq` из `RESEARCH.md` §2.
- Supplemental: сырые `per_group=[17,14,7,2,10,87,1]`, 138 записей; после P2-01 dedup —
  `[16,14,7,2,10,87,1]`, 137 уникальных. Оракул состояния — 137.
- MESSAGE_CREATE: content 208, author bot=true; MESSAGE_UPDATE: content 208 (не пустой),
  embeds=1.
- SESSIONS_REPLACE: 1 сессия, status `invisible`, client desktop/linux.
- THREAD/CHANNEL: реальные фикстуры с отрицательными id — диагностика/отказ; валидный
  путь — синтетика с теми же значениями полей.
- T-DATA-04: два одинаковых прогона → `assert_eq!(state_a, state_b)` и
  `assert_eq!(effects_a, effects_b)`; stale/unknown/malformed не меняют данные.
- Red-first: тесты фиксируются до кода (md5), затем green; тест-спека после red не
  меняется. После green — `cargo fmt`, clippy `-D warnings`, workspace-тесты, gate.
