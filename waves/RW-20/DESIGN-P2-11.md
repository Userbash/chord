# RW-20 / DESIGN-P2-11 — memory-only typing TTL

Дата: 2026-10-21. Карточка: P2-11. Зависимость: P2-04 (принята), P2-02 (принята).
Research: `waves/RW-20/RESEARCH-P2-11.md`. Контракт **заморожен до кода**; менять только
через `TEST-CORRECTIONS.md` и повторный freeze.

## 1. Scope и решения

**Входит:** `TYPING_START` (28 в capture), memory-only `State.typing` по
`(channel_id, user_id)`, TTL **10 000 мс**, restart продлевает, optional `guild_id`/
`timestamp`/`member.user`, удаление автора при `MESSAGE_CREATE`, очистка по `READY`,
виртуальные часы, селекторы, диагностика, red-first тесты.

**Не входит:** исходящий typing, `TYPING_START_LOCAL`, проводной `TYPING_STOP` (0/28),
`typing_indicator_style` (0/28), unread-count, SQLite/UI/сеть/live, `Guild.members`.

Ключевые решения:

- TTL — константа `TYPING_TTL_MS = 10_000`, не настройка;
- время приходит извне: `TypingStart.received_at_ms`, а не системные часы внутри reducer;
- `Event::from_frame_at(frame, now_ms)` и `apply_frame_at(state, frame, now_ms)` — новый
  детерминированный вход; `Event::from_frame`/`apply_frame` дают `received_at_ms=None`;
- `TYPING_START` без времени — `ignored "typing.received_at_ms"`, запись не создаётся;
- повторная пара `(channel,user)` перезапускает TTL, `duplicates` не пишется;
- `MESSAGE_CREATE` с автором удаляет только пару `(channel, author)`;
- `READY` создаёт пустую карту; `RESUMED` ничего не очищает;
- селекторы не мутируют; просроченные записи скрываются, `prune_expired` удаляет явно.

## 2. Файловая матрица

| Владелец | Файлы | Можно |
| --- | --- | --- |
| W20 (CORE-2) | `state/typing/{mod,types,parser,reducer,selectors}.rs` | создавать/менять только эти |
| W20 | `tests/typing_{types,reducer,selectors}.rs`, `tests/data/typing-run2.jsonl` | только эти |
| TL-2 | `state/mod.rs`, `state/types.rs`, `state/reducer.rs`, `src/lib.rs` | integration-лейн |
| TEST-1/QA-1 | `waves/RW-20/` | матрица/логи |

W20 не правит shared-файлы, чужие тесты, доки, golden, Cargo/package.

## 3. Публичные типы (`state/typing/types.rs`)

```rust
pub const TYPING_TTL_MS: u64 = 10_000;
pub type TypingKey = (u64, u64); // (channel_id, user_id)

#[derive(Debug, Clone, PartialEq)]
pub struct TypingStart {
    pub channel_id: u64,
    pub user_id: u64,
    pub guild_id: Option<u64>,
    pub wire_timestamp: Option<i64>, // секунды, только для диагностики
    pub member_user: Option<User>,
    pub received_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TypingEntry {
    pub channel_id: u64,
    pub user_id: u64,
    pub guild_id: Option<u64>,
    pub wire_timestamp: Option<i64>,
    pub started_at_ms: u64,
    pub expires_at_ms: u64,
}

impl TypingEntry {
    pub fn is_active(&self, now_ms: u64) -> bool;   // now < expires
    pub fn remaining_ms(&self, now_ms: u64) -> u64; // saturating
}
```

`TypingStart` — payload события; `TypingEntry` — store. В `TypingEntry` нет `User`, поэтому
`State::Debug` не печатает member payload.

## 4. Парсер (`state/typing/parser.rs`)

```rust
pub fn parse_typing_start(
    data: &EtfValue,
    received_at_ms: Option<u64>,
) -> Result<(TypingStart, StateDiagnostics), EventError>;
```

Путь диагностики — `typing.*`.

| Поле | Правило |
| --- | --- |
| `d` | map, иначе `WrongType { path: "typing", expected: "map" }` |
| `channel_id` | positive; missing → `Missing("typing.channel_id")`, invalid → `InvalidId(...)` |
| `user_id` | positive; `Missing/InvalidId("typing.user_id")` |
| `guild_id` | optional; nil/missing → `None`; invalid → `ignored "typing.guild_id"` |
| `timestamp` | optional int/строка-int → `Some`; nil/missing → `None`; иначе `ignored` |
| `member` | nil/missing → `None`; не map → `ignored "typing.member"`; map без `user`/не map → `malformed "typing.member.user"` |
| `member.user.id` | `parse_user`; id != `user_id` → `inconsistencies "typing:<channel>:<user>:member-user:<id>"`, `None` |
| `typing_indicator_style` | если присутствует — `ignored "typing.typing_indicator_style"`; поле не сохраняется |

`received_at_ms` не читается из провода — его передаёт вызывающий. Nil `d`/не map — ошибка;
пустой `d` без id — `Missing`, не `Unsupported`.

## 5. Reducer (`state/typing/reducer.rs`)

```rust
pub fn apply_start(state: &mut State, start: TypingStart);
pub fn remove_for_message(state: &mut State, channel_id: u64, author_id: u64) -> bool;
pub fn remove_entry(state: &mut State, channel_id: u64, user_id: u64) -> bool;
pub fn prune_expired(state: &mut State, now_ms: u64) -> usize;
```

- `apply_start`:
  - `received_at_ms=None` → `state.diagnostics.ignored.push("typing.received_at_ms")`,
    store не меняется;
  - `Some(now)` → если `member_user` есть и `id == start.user_id`, upsert в `State.users`;
    вставить/заменить `TypingEntry` с `started_at_ms=now`,
    `expires_at_ms=now.saturating_add(TYPING_TTL_MS)`;
  - повтор пары — last wins, без `duplicates`.
- `remove_for_message` — удалить `(channel_id, author_id)`, вернуть `true`, если была запись.
- `remove_entry` — явное удаление, bool.
- `prune_expired` — удалить все `now >= expires_at_ms`, вернуть число; без диагностики.
- `MESSAGE_CREATE` в `state/reducer.rs` вызывает `remove_for_message` после вставки сообщения
  (author id взять до move).
- `READY` — пустая карта; `RESUMED` — без изменений.

## 6. Event API (TL-2 integration)

В `state/types.rs`:

```rust
impl Event {
    pub fn from_frame_at(
        frame: &EtfValue,
        now_ms: u64,
    ) -> Result<Self, EventError>;
}
```

- `from_routed`/`from_frame` — `received_at_ms=None`;
- `from_frame_at` — тот же route, но для `TypingStart` передаёт `Some(now_ms)`;
- `EventPayload::TypingStart(TypingStart)`;
- `state/reducer.rs`:
  ```rust
  pub fn apply_frame_at(
      state: State,
      frame: &EtfValue,
      now_ms: u64,
  ) -> (State, Effects);
  ```
  `apply_frame_at` = `Event::from_frame_at` + `apply`; ошибки — диагностика, не паника;
- `apply_dispatch`/`apply_frame` не имеют часов; typing там остаётся ignored с диагностикой;
- root re-export: `TYPING_TTL_MS`, `TypingStart`, `TypingEntry`, `TypingKey`;
- `State` field: `pub typing: BTreeMap<TypingKey, TypingEntry>`, Debug — длина,
  `from_ready` — пустая карта.

## 7. Селекторы (`state/typing/selectors.rs`)

```rust
impl State {
    pub fn typing_entry(&self, channel_id: u64, user_id: u64, now_ms: u64) -> Option<&TypingEntry>;
    pub fn is_typing(&self, channel_id: u64, user_id: u64, now_ms: u64) -> bool;
    pub fn typing_entries(&self, channel_id: u64, now_ms: u64) -> Vec<&TypingEntry>;
    pub fn typing_user_ids(&self, channel_id: u64, now_ms: u64) -> Vec<u64>;
    pub fn typing_users(&self, channel_id: u64, now_ms: u64) -> Vec<&User>;
    pub fn typing_entries_by_guild(&self, guild_id: u64, now_ms: u64) -> Vec<&TypingEntry>;
    pub fn typing_entry_count(&self, now_ms: u64) -> usize;
    pub fn typing_channel_count(&self, now_ms: u64) -> usize;
    pub fn typing_remaining_ms(&self, channel_id: u64, user_id: u64, now_ms: u64) -> Option<u64>;
}
```

- `typing_entry` возвращает `None`, если `now >= expires`; запись не удаляет;
- `typing_entries`/`typing_user_ids` — по возрастанию `user_id`;
- `typing_users` — только известные `State.users`, по возрастанию id; неизвестные не
  выдумываются;
- `typing_entries_by_guild` — только точный `Some(guild_id)`, сортировка `(channel,user)`;
- `typing_entry_count`/`typing_channel_count` — только активные; `channel_count` — уникальные
  channel_id;
- `typing_remaining_ms` — `None`, если запись отсутствует/просрочена, иначе saturating.

## 8. Тесты (15, M)

Файлы: `tests/typing_types.rs` (5), `typing_reducer.rs` (5), `typing_selectors.rs` (5).
Полная матрица — `TEST-MATRIX-P2-11.md`; fixture `tests/data/typing-run2.jsonl` (28
минимизированных строк, W20 создаёт; oracle-команда в research §2).

| Файл | ID | Покрытие |
| --- | --- | --- |
| types | t01 | реальный capture idx 117 / s=192: channel/user/guild/timestamp/member; style отсутствует |
| types | t02 | 28 строк fixture: 2 пары (27+1), счётчики, timestamps |
| types | t03 | минимальный DM-подобный `d`: guild/timestamp/member None |
| types | t04 | table: missing/bad ids, bad guild/timestamp, style; diagnostics |
| types | t05 | member mismatch/malformed; `from_frame` None vs `from_frame_at` Some |
| reducer | t06 | start → entry, TTL 10 000, member upsert, effects пуст |
| reducer | t07 | повтор через 8 000 → expiry new+10 000; active 9 999/10 000 |
| reducer | t08 | 28 реальных START → 2 активные записи; count |
| reducer | t09 | MESSAGE_CREATE автора удаляет пару, другое — нет |
| reducer | t10 | READY reset; no-clock ignored; stale/malformed; prune |
| selectors | t11 | empty/self; entry/is_typing/remaining |
| selectors | t12 | несколько пользователей: ids/users сортировка |
| selectors | t13 | guild split, entries_by_guild |
| selectors | t14 | active counts, channel count, expiry boundary |
| selectors | t15 | 28 fixture final: 2 entries, 2 channels, 1 guild |

## 9. Мутации (5–8)

M1 TTL 9 999/10 001; M2 expiry `<=` вместо `<`; M3 no-clock создаёт запись;
M4 повтор не продлевает; M5 MESSAGE_CREATE не удаляет; M6 removal по user без channel;
M7 selectors не фильтруют expired; M8 member upsert без id-check. Цель — 100 % критических
ветвей TTL/removal.

## 10. DoD

- red compile/behavior exit 101 сохранён; green targeted 15/15;
- I0/I1 интеграция; `apply_frame_at` детерминирован, `apply_frame` typing ignored;
- `cargo fmt`/`clippy` — batch gate; нет сети/SQLite/UI/live/новых зависимостей.
