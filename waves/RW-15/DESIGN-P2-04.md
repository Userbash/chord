# RW-15 / P2-04 — замороженный дизайн: memory-only member list

Статус: контракт заморожен координатором до red-тестов. Файлы:
`src/state/member_list.rs`, `src/state/member_list_selectors.rs`, аддитивные правки
`src/state/{types,reducer,mod}.rs`, `src/ready/social.rs`, `src/lib.rs`.
Тесты: `tests/member_list_{types,reducer,selectors}.rs`.

## 1. Scope

**Входит:** `GUILD_MEMBER_LIST_UPDATE` (SYNC/INSERT/UPDATE/DELETE), плоский список
`group|member`, память, диагностика, селекторы, минимальный presence-снимок, root re-exports.

**Не входит:** wire-формат op37/op14 (не подтверждён capture; остаётся
`Effect::SubscribeMembers` как интерфейс), `GUILD_MEMBERS_CHUNK` (P2-05), полная presence-модель
(P2-09), `State.users` merge, SQLite, UI, сеть, live Discord, новые зависимости.

## 2. Публичные типы (`discord_core::state`, root `discord_core`)

```rust
pub struct MemberListId(String);          // raw "everyone"/"1659606647"; Ord, Display, Debug
impl MemberListId {
    pub fn new(value: impl Into<String>) -> Self;
    pub fn as_str(&self) -> &str;
}

pub struct MemberListRange { pub start: u32, pub end: u32 } // включительно

pub struct MemberListGroup { pub id: String, pub count: u32 }

pub struct MemberPresence {
    pub status: Option<String>,
    pub client_status: BTreeMap<String, String>,
}

pub enum MemberListEntry {
    Group { id: String },
    Member { member: GuildMember, user: User, presence: MemberPresence },
}

pub enum MemberListOp {
    Sync { items: Vec<MemberListEntry>, range: Option<MemberListRange> },
    Insert { index: u32, item: MemberListEntry },
    Update { index: u32, item: MemberListEntry },
    Delete { index: u32 },
}

pub struct MemberListUpdate {
    pub guild_id: u64,
    pub id: MemberListId,
    pub member_count: Option<i64>,
    pub online_count: Option<i64>,
    pub groups: Vec<MemberListGroup>,
    pub ops: Vec<MemberListOp>,
}

pub struct MemberList {
    pub guild_id: u64,
    pub id: MemberListId,
    pub member_count: Option<i64>,
    pub online_count: Option<i64>,
    pub groups: Vec<MemberListGroup>,
    pub items: Vec<MemberListEntry>,
    pub range: Option<MemberListRange>,
}
```

`State` получает `pub member_lists: BTreeMap<(u64, MemberListId), MemberList>`;
`State::Debug` печатает только число списков. `EventPayload` получает
`GuildMemberListUpdate(MemberListUpdate)`. Root re-exports всех имён из этого раздела.

## 3. Разбор (`Event::from_routed`)

Фатальные ошибки заголовка: отсутствие/не-u64 `guild_id`, отсутствие/пустой/не-текстовый `id`,
отсутствие/не-список `ops`. Пустой `id` — `WrongType` (не snowflake); отрицательный/нечисловой
`guild_id` — `InvalidId`. Состояние не меняется, `apply_dispatch` пишет `malformed`,
`last_seq` не откатывается.

Нефатально (диагностика + пропуск):
- `groups` отсутствует/nil → пусто; неверный контейнер → `ignored`;
  битая группа (`id` не текст) → `malformed` и пропуск; `count` не-u32 → `ignored`, `0`;
- op не-map, битый `index`, отсутствующий `item`, битый `item` → `malformed`, остальные ops
  применяются; неизвестный `op` → `inconsistencies` `member_list:{guild}:{list}:unknown-op:{op}`;
- битый member (нет `user.id`, отрицательный/нечисловой id, не-map `user`) → `malformed`;
- `roles: null` → пустой Vec; `presence` отсутствует/nil → дефолт; не-map → `ignored`;
- `SYNC.items` не-список → op пропускается; битые элементы пропускаются, остальные входят;
- `range` — только 2 байта (`Str`) либо `List/Tuple` из двух u32; иначе `ignored`, `None`.

`MemberPresence` берёт только `status` и `client_status` (map строк); `activities` не
разбираются в P2-04 и не считаются ошибкой.

## 4. Reducer

Для `GuildMemberListUpdate`:
1. Найти/создать `MemberList` по `(guild_id, id)`. Нет гильдии → `inconsistencies`
   `guilds:{guild_id}:member-list`; список всё равно создаётся.
2. `groups` заменяются снимком всегда; `member_count`/`online_count` обновляются только если
   пришли (`Some`), иначе сохраняются.
3. Ops применяются строго по порядку:
   - `SYNC` заменяет `items` и `range` целиком (`range=None`, если не пришёл);
   - `INSERT index`: `index <= len` → вставка со сдвигом; `index > len` → диагностика, без
     изменения;
   - `UPDATE index`: `index < len` → замена без сдвига; иначе диагностика;
   - `DELETE index`: `index < len` → удаление со сдвигом; иначе диагностика.
4. Дубли member-id применяются буквально (ops не переупорядочиваются), в
   `diagnostics.duplicates` пишется `member_list:{guild}:{list}:duplicate:{user_id}`.
   `member_list_index_of` возвращает первый индекс; полный `SYNC` лечит список.
5. Stale `s` обрабатывается общим reducer до разбора; effects для member list не создаются.

Диагностические префиксы (общий `member_list:{guild}:{list}:`):
`out-of-range:{op}:{index}` и `unknown-op:{op}` — в `inconsistencies`; `duplicate:{user_id}` — в
`duplicates`; структурные ошибки — в `malformed`, необязательные/пустые поля — в `ignored`.

## 5. Селекторы (`impl State`)

```rust
member_list(guild_id, &MemberListId) -> Option<&MemberList>
member_list_items(...) -> &[MemberListEntry]       // пустой срез, если списка нет
member_list_member_at(...) -> Option<(&GuildMember, &User, &MemberPresence)>
member_list_index_of(...) -> Option<usize>          // первый member с user_id
member_list_group(..., group_id: &str) -> Option<&MemberListGroup>
member_list_range(...) -> Option<MemberListRange>
member_list_len(...) -> usize
member_list_is_loaded(...) -> bool                  // range.is_some()
member_list_count() -> usize                        // число списков в State
```

Все — только чтение, без паник, детерминированный порядок (`items` — порядок ops).

## 6. Инварианты

1. `items` — плоский список; `index` относится к нему.
2. `SYNC` — полная замена; только он задаёт/сбрасывает `range`.
3. `INSERT/DELETE` сдвигают, `UPDATE` — нет.
4. `groups` — снимок, не накопление.
5. counts/groups — сырые независимые числа.
6. Ошибка одной op не отменяет уже применённые; событие не паникует.
7. list key — `(u64, MemberListId)`; list_id никогда не парсится в u64.
8. `State.messages`, `Guild.member_count`, `State.users` не меняются member list-событием.
