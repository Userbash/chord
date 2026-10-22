# RW-19 / DESIGN-P2-10 — memory-only voice states

Дата: 2026-10-21. Карточка: P2-10. Зависимость: P2-02 (принята), P2-09 (review).
Research: `waves/RW-19/RESEARCH-P2-10.md`. Контракт ниже **заморожен до кода**; менять
только через `TEST-CORRECTIONS.md` и повторный freeze Team Lead.

## 1. Scope и решения

**Входит:** `READY_SUPPLEMENTAL` снимок, `VOICE_STATE_UPDATE`,
`VOICE_CHANNEL_START_TIME_UPDATE`, `VOICE_CHANNEL_STATUS_UPDATE`; memory-only store,
вход/выход/перемещение, optional `member.user`, таймеры как чистые селекторы, диагностика,
red-first тесты.

**Не входит:** voice transport/P4, `VOICE_SERVER_UPDATE`, op4/op14, DM-звонки, SQLite/UI/сеть/
live, `Guild.members`, `session_id` в публичной модели.

Ключевые решения:

- store голосовых записей — `BTreeMap<(guild_id, user_id), VoiceState>`;
- `channel_id: None` = выход; reducer удаляет запись;
- таймер канала — отдельный `BTreeMap<(guild_id, channel_id), i64>` (секунды);
- статус канала — отдельный `BTreeMap<(guild_id, channel_id), String>`;
- `connected_at` optional; отсутствие не заменяется `now`;
- `discoverable` default `true`, `self_stream` default `false`, остальные флаги default `false`;
- `member.user` проверяется по `id == user_id`; при совпадении upsert в `State.users`;
- `READY` сбрасывает карты; `READY_SUPPLEMENTAL` добавляет снимок аддитивно;
- все timestamps — секунды; селекторы принимают `now_ms` и не заводят потоков/таймеров.

## 2. Файловая матрица

| Владелец | Файлы | Можно |
| --- | --- | --- |
| W19 (CORE-1) | `rust/crates/discord-core/src/state/voice/{mod,types,parser,reducer,selectors}.rs` | создавать/менять только эти |
| W19 (CORE-1) | `rust/crates/discord-core/tests/voice_state_{types,reducer,selectors}.rs` | только эти тесты |
| TL-2 | `state/mod.rs`, `state/types.rs`, `state/reducer.rs`, `src/lib.rs` | integration-лейн |
| TEST-1/QA-1 | `waves/RW-19/` | тест-матрица, mutation-логи |

W19 **не** правит `state/mod.rs`, `types.rs`, `reducer.rs`, `lib.rs`, чужие тесты, доки,
golden. Модуль `state/voice/` — новый каталог, поэтому W19 может добавлять в него подмодули
без shared-файлов.

## 3. Публичные типы (`state/voice/types.rs`)

```rust
pub type VoiceKey = (u64, u64);          // (guild_id, user_id)
pub type VoiceChannelKey = (u64, u64);   // (guild_id, channel_id)

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceState {
    pub guild_id: u64,
    pub user_id: u64,
    pub channel_id: Option<u64>,
    pub connected_at: Option<i64>,       // секунды
    pub mute: bool,
    pub deaf: bool,
    pub self_mute: bool,
    pub self_deaf: bool,
    pub self_video: bool,
    pub self_stream: bool,
    pub suppress: bool,
    pub discoverable: bool,
    pub request_to_speak_timestamp: Option<i64>, // секунды
}

impl VoiceState {
    pub fn is_connected(&self) -> bool;       // channel_id.is_some()
    pub fn connected_at_ms(&self) -> Option<u64>; // >=0, checked *1000
}

#[derive(Debug, Clone, PartialEq)]
pub struct VoiceStateUpdate {
    pub state: VoiceState,
    pub member_user: Option<User>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceChannelStartTime {
    pub guild_id: u64,
    pub channel_id: u64,
    pub voice_start_time: Option<i64>, // None = nil/clear
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VoiceChannelStatus {
    pub guild_id: u64,
    pub channel_id: u64,
    pub status: Option<String>,        // None = nil/clear; пустая строка тоже clear
}
```

`session_id` намеренно отсутствует. `VoiceState` не содержит `User`, поэтому `Debug` не
печатает member payload; `VoiceStateUpdate` может печатать `member_user` (как presence).

## 4. Парсеры (`state/voice/parser.rs`)

Все функции чистые, без паник; `data` — уже декодированный ETF.

```rust
pub fn parse_voice_state_update(
    data: &EtfValue,
) -> Result<(VoiceStateUpdate, StateDiagnostics), EventError>;

pub fn parse_voice_states(
    value: Option<&EtfValue>,
    guild_id: Option<u64>,
    path: &str,
    diagnostics: &mut StateDiagnostics,
) -> Vec<VoiceState>;

pub fn parse_voice_channel_start_time(
    data: &EtfValue,
) -> Result<(VoiceChannelStartTime, StateDiagnostics), EventError>;

pub fn parse_voice_channel_status(
    data: &EtfValue,
) -> Result<(VoiceChannelStatus, StateDiagnostics), EventError>;
```

### 4.1 `VOICE_STATE_UPDATE`

Путь диагностики — `voice.*`.

| Поле | Обязательность | Правило |
| --- | --- | --- |
| `d` | да | map, иначе `WrongType { path: "voice", expected: "map" }` |
| `guild_id` | да | положительная снежинка; missing → `Missing("voice.guild_id")`, invalid → `InvalidId("voice.guild_id")` |
| `user_id` | да | positive; `Missing/InvalidId("voice.user_id")` |
| `channel_id` | да (может быть nil) | nil → `None`; иначе positive; invalid → `InvalidId("voice.channel_id")` (не трактовать как выход) |
| `connected_at` | нет | int/строка-int → `Some`; nil/missing → `None`; иначе `ignored "voice.connected_at"` |
| `mute`, `deaf`, `self_mute`, `self_deaf`, `self_video`, `self_stream`, `suppress` | нет | bool/атом `true|false` → значение; иначе default + `ignored "voice.<field>"` |
| `discoverable` | нет | default `true`; иначе bool; иначе default + `ignored "voice.discoverable"` |
| `request_to_speak_timestamp` | нет | int/строка-int → `Some`; nil/missing → `None`; иначе `ignored "voice.request_to_speak_timestamp"` |
| `member` | нет | nil/missing → `None`; не map → `ignored "voice.member"`; map без `user`/не map → `malformed "voice.member.user"` |
| `member.user.id` | — | `parse_user`; id != `user_id` → `inconsistencies "voice:<guild>:<user>:member-user:<id>"`, `member_user=None` |
| `session_id` | нет | не читается и не сохраняется |

`value::unsigned` уже принимает строку/атом? Использовать существующие `value::unsigned`/
`value::boolean`/`value::integer`; не писать второй парсер чисел. Если helper отсутствует —
локальная функция в `parser.rs`, но без паник и с теми же диагностическими путями.

### 4.2 Снимок `READY_SUPPLEMENTAL`

`parse_voice_states(value, guild_id, path, diagnostics)`:

- `value` nil/absent → `vec![]`;
- не list → `diagnostics.malformed.push(path)` и `vec![]`;
- для каждого `i`: не map/битый `user_id` → `diagnostics.malformed.push(format!("{path}[{i}]"))`,
  пропустить;
- `guild_id` берётся из parent `guilds[i].id`; если parent `None`, а в элементе есть
  `guild_id` — использовать его; если нет ни там, ни там — `inconsistencies "{path}[{i}]:guild"`,
  пропустить;
- если в элементе `guild_id` есть и не равен parent → `inconsistencies
  "voice:<parent>:<user>:guild:<element>"`, использовать parent;
- `channel_id` nil → `inconsistencies "{path}[{i}]:channel:nil"`, не добавлять;
- поля флагов/`connected_at`/`request_to_speak_timestamp` — как в §4.1, но ошибки optional
  полей не фатальны для снимка;
- `member` в снимке `003` отсутствует; если пришёл — можно разобрать `member.user`, но
  `VoiceState` его не хранит; диагностика сохраняется;
- дубликаты `(guild,user)` в одном снимке — last wins, `duplicates` не писать (снимок
  не является «событием»); reducer просто вставляет по порядку.

### 4.3 `VOICE_CHANNEL_START_TIME_UPDATE`

Путь `voice_start_time.*`:

- `d` map, иначе `WrongType { path: "voice_start_time", expected: "map" }`;
- `guild_id` positive → `Missing/InvalidId("voice_start_time.guild_id")`;
- `id` positive → `Missing/InvalidId("voice_start_time.channel_id")`;
- `voice_start_time` отсутствует/nil → `None`; int/строка-int >= 0 → `Some`; иначе
  `WrongType { path: "voice_start_time.voice_start_time", expected: "int|null" }`.
- Нечисловое значение **не очищает** существующий таймер: событие возвращает ошибку.

### 4.4 `VOICE_CHANNEL_STATUS_UPDATE`

Путь `voice_status.*`:

- `d` map, иначе `WrongType { path: "voice_status", expected: "map" }`;
- `guild_id` positive → `Missing/InvalidId("voice_status.guild_id")`;
- `id` positive → `Missing/InvalidId("voice_status.channel_id")`;
- `status` отсутствует/nil/`""` → `None`;
- строка непустая → `Some`;
- иначе `WrongType { path: "voice_status.status", expected: "str|null" }`.
- Non-nil статус в capture не подтверждён; тест synthetic `"Music"` помечен как
  минимальная симметричная модель.

## 5. Reducer (`state/voice/reducer.rs`)

```rust
pub fn apply_update(state: &mut State, update: VoiceStateUpdate);
pub fn apply_snapshot(state: &mut State, voice_states: Vec<VoiceState>);
pub fn apply_start_time(state: &mut State, update: VoiceChannelStartTime);
pub fn apply_status(state: &mut State, update: VoiceChannelStatus);
```

Правила:

- `apply_update`: если `member_user` есть и `id == state.user_id` — `state.users.insert`;
  затем `channel_id Some` → `voice_states.insert((guild,user), state)`, `None` → `remove`.
  Last wins, `Effects` пусты.
- `apply_snapshot`: для каждой записи с `channel_id Some` — upsert; `None` — remove.
  Снимок не очищает карту целиком; `READY` уже её сбросил.
- `apply_start_time`: `Some(v)` → insert; `None` → remove. `v < 0` не приходит (парсер
  отклонит); reducer не паникует на отрицательном, просто remove? Нет: `Some` с отрицательным
  — `debug_assert!(v >= 0)` + insert; парсер гарантирует. Лучше `if v >= 0 { insert } else { remove }`
  без panic; тест на границу.
- `apply_status`: `Some(s)` непустой → insert; `None`/пустой → remove.
- `READY`: `State::from_ready` создаёт пустые карты.
- `READY_SUPPLEMENTAL`: `merge_supplemental` вызывает `apply_snapshot(state, supplemental.voice_states)`.
- `MessageCreate`/другие события голос не трогают.

## 6. Селекторы (`state/voice/selectors.rs`)

Все методы — `impl State`, только чтение, без паник.

```rust
impl State {
    pub fn voice_state(&self, guild_id: u64, user_id: u64) -> Option<&VoiceState>;
    pub fn voice_channel_of(&self, guild_id: u64, user_id: u64) -> Option<u64>;
    pub fn voice_users_in_channel(&self, guild_id: u64, channel_id: u64) -> Vec<&VoiceState>;
    pub fn voice_channel_user_count(&self, guild_id: u64, channel_id: u64) -> usize;
    pub fn voice_state_count(&self) -> usize;
    pub fn voice_guild_count(&self) -> usize;
    pub fn is_user_in_voice(&self, guild_id: u64, user_id: u64) -> bool;
    pub fn self_voice_state(&self) -> Option<&VoiceState>;
    pub fn is_self_in_voice(&self) -> bool;
    pub fn voice_connected_for_ms(&self, guild_id: u64, user_id: u64, now_ms: u64) -> Option<u64>;
    pub fn voice_channel_start_time(&self, guild_id: u64, channel_id: u64) -> Option<i64>;
    pub fn voice_channel_elapsed_ms(&self, guild_id: u64, channel_id: u64, now_ms: u64) -> Option<u64>;
    pub fn voice_channel_status(&self, guild_id: u64, channel_id: u64) -> Option<&str>;
}
```

- `voice_users_in_channel` — по возрастанию `user_id` (BTreeMap уже сортирует по ключу).
- `voice_guild_count` — число уникальных guild_id в карте.
- `self_voice_state` — первая запись с `user_id == state.user.id` в порядке `(guild,user)`;
  если записей несколько, это детерминированный первый, не «любой».
- `voice_connected_for_ms`: `connected_at < 0` → `None`; `start = secs*1000`
  (checked/saturating), `Some(now_ms.saturating_sub(start))`.
- `voice_channel_elapsed_ms`: `start < 0` → `None`; иначе `Some(now_ms.saturating_sub(start*1000))`.
- `voice_channel_status`: только непустая строка; пустая всегда `None`.
- `voice_channel_start_time` — `Option<i64>`, не `Option<Option<...>>`.

## 7. Интеграция (TL-2, только его файлы)

### 7.1 Чекпоинт I0 — модуль и store

- `state/mod.rs`: `pub mod voice;` и re-export типов/функций; `pub use voice::{...};`
- `state/types.rs`: `use super::voice::{VoiceChannelKey, VoiceKey, VoiceState};`
  - поля `State`: `voice_states`, `voice_channel_start_times`, `voice_channel_statuses`;
  - `State::Debug` — только длины;
  - `State::from_ready` — пустые `BTreeMap`;
  - `ReadySupplemental` — `pub voice_states: Vec<VoiceState>`;
  - `parse_ready_supplemental` — вызвать `parse_voice_states` для `d.guilds[i].voice_states`,
    используя `guild_id = guild_ids[i]`, path `ready_supplemental.guilds[i].voice_states`.
- I0 не добавляет `EventPayload`/routes; после него W19 может гонять parser/reducer/selectors
  тесты напрямую.

### 7.2 Чекпоинт I1 — event API (общий integration-патч batch)

- `EventPayload`: `VoiceStateUpdate(VoiceStateUpdate)`,
  `VoiceChannelStartTime(VoiceChannelStartTime)`, `VoiceChannelStatus(VoiceChannelStatus)`.
- `Event::from_routed`:
  - `VoiceStateUpdate` → `parse_voice_state_update`;
  - `VoiceChannelStartTime` → `parse_voice_channel_start_time`;
  - `VoiceChannelStatus` → `parse_voice_channel_status`.
- `state/reducer.rs` match:
  - `VoiceStateUpdate` → `voice::reducer::apply_update`;
  - `VoiceChannelStartTime` → `apply_start_time`;
  - `VoiceChannelStatus` → `apply_status`.
- `merge_supplemental`: `voice::reducer::apply_snapshot`.
- `src/lib.rs`: re-export `VoiceState`, `VoiceStateUpdate`, `VoiceChannelStartTime`,
  `VoiceChannelStatus`, `VoiceKey`, `VoiceChannelKey`; `state/reducer` re-export не нужен.
- Root `state/mod.rs` re-export — до I1, но после появления файлов.

## 8. Тесты (15, M)

Файлы: `tests/voice_state_types.rs` (6), `voice_state_reducer.rs` (5),
`voice_state_selectors.rs` (4). Все ID и ожидания — в `TEST-MATRIX-P2-10.md`;
ниже сводка.

| Файл | ID | Покрытие |
| --- | --- | --- |
| types | v01 | снимок `003`: 1 запись, точные id/флаги/`connected_at` |
| types | v02 | join `s=111`: member.user, `self_mute=true`, flags |
| types | v03 | leave `s=51`: `channel_id=None`, `connected_at` сохранён |
| types | v04 | timer: numeric + nil; status: nil + synthetic `"Music"` |
| types | v05 | optional/broken table: missing ids, bad channel, bad bools, unknown |
| types | v06 | member mismatch/malformed, no panic, diagnostics |
| reducer | v07 | join → entry, member upsert; leave → remove |
| reducer | v08 | move/replace, last wins, unknown leave no-op |
| reducer | v09 | snapshot `003` + READY reset |
| reducer | v10 | start/status set/clear; negative/empty boundaries |
| reducer | v11 | stale seq and malformed event diagnostic |
| selectors | v12 | self voice / empty store |
| selectors | v13 | channel users sorted, counts, guild count |
| selectors | v14 | connected/elapsed boundaries, saturating |
| selectors | v15 | status/start accessors and multiple guilds |

Бюджет: 15 тестов (M, 10–16). Таблицы вместо копий; реальный снимок + точные live-значения.

## 9. Мутации (5–8)

M1 `channel_id None` не удаляет запись; M2 `member.user` upsert без проверки id;
M3 `discoverable` default `false`; M4 `connected_at` секунды→мс без умножения;
M5 `voice_start_time nil` не очищает; M6 `status` пустая строка остаётся;
M7 `voice_users_in_channel` не сортирует/не фильтрует; M8 stale-политика не применяется.
Цель — 100 % для критических ветвей: выход, member-id, таймер, default.

## 10. Definition of Done

- W19 parser/reducer/selectors зелёные targeted; red-логи с exit 101 сохранены.
- TL-2 I0/I1 прошли; `Event::from_frame` на `003` даёт снимок, live synthetic — update.
- 15/15 targeted, batch gate 0, fmt/clippy 0.
- `cargo test --offline -p discord-core --test voice_state_types --test voice_state_reducer
  --test voice_state_selectors` — 15 passed.
- Нет сети/SQLite/UI/live/новых зависимостей; `state/voice/` не читает golden.
