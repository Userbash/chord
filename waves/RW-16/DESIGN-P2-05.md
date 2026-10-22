# RW-16 / P2-05 — замороженный дизайн: чанки и догрузка

Статус: контракт заморожен координатором до red-тестов. Файлы:
`src/state/member_chunk.rs`, `src/state/member_chunk_selectors.rs`, аддитивные правки
`src/state/{types,reducer,mod}.rs`, `src/lib.rs`.
Тесты: `tests/member_chunk_{types,reducer,selectors}.rs`.

## 1. Scope

**Входит:** подтверждённая capture часть `GUILD_MEMBERS_CHUNK` (`guild_id`, `chunk_index`,
`chunk_count`, `members`, `not_found`), память по `(guild_id, nonce)`, сборка chunk-ов,
аддитивный merge в `Guild.members`/`State.users`, диагностика, селекторы, чистый
построитель `Effect::RequestPresence` для видимого диапазона member list P2-04.

**Не входит:** wire-запрос участников (op8/op37/op14) и `nonce`-корреляция как протокол;
`presences[]` внутри chunk (P2-09), `PRESENCE_UPDATE`, SQLite, UI, сеть, live Discord,
новые зависимости.

**Политика неизвестного:** `nonce` — optional string; в обоих фикстурах отсутствует,
поэтому при отсутствии ключ сборки `(guild_id, None)`. Поле `presences` не разбирается:
если оно есть и не `nil`, пишется `ignored` (`d.presences`), данные не теряются молча.
Это ограничение зафиксировано в `RESEARCH-P2-05.md` §3.

## 2. Публичные типы (`discord_core::state`, root `discord_core`)

```rust
pub struct MemberChunkMember {
    pub member: GuildMember,
    pub user: User,
}

pub struct MemberChunk {
    pub guild_id: u64,
    pub nonce: Option<String>,
    pub chunk_index: u32,
    pub chunk_count: u32,
    pub members: Vec<MemberChunkMember>,
    pub not_found: Vec<u64>,
}

pub struct MemberChunkAssembly {
    pub guild_id: u64,
    pub nonce: Option<String>,
    pub chunk_count: u32,
    /// Индекс chunk → последний полученный chunk с этим индексом.
    pub chunks: BTreeMap<u32, MemberChunk>,
}

impl MemberChunkAssembly {
    pub fn is_complete(&self) -> bool;
    pub fn len(&self) -> usize;             // число уникальных chunk-индексов
    pub fn is_empty(&self) -> bool;
    pub fn members(&self) -> impl Iterator<Item = &MemberChunkMember>;
}
```

`State` получает:

```rust
pub member_chunks: BTreeMap<(u64, Option<String>), MemberChunkAssembly>;
```

`State::Debug` печатает только `member_chunks` (число сборок). `EventPayload` получает
`GuildMembersChunk(MemberChunk)`. Root re-exports всех имён из этого раздела.

## 3. Разбор (`Event::from_routed`)

Фатальные ошибки заголовка (состояние не меняется, `apply_dispatch` пишет `malformed`,
`last_seq` не откатывается):

- отсутствие/не-u64/отрицательный `guild_id` → `Missing`/`InvalidId`, как P2-04;
- отсутствие/не-u32/отрицательный `chunk_index` или `chunk_count` → `Missing`/`WrongType`;
- `chunk_count == 0` → `WrongType` (`d.chunk_count`, expected `positive integer`, found `0`);
- `chunk_index >= chunk_count` → `WrongType` (`d.chunk_index`, expected `index < chunk_count`,
  found `out-of-range`).

Нефатально (диагностика + пропуск/дефолт):

- `members` отсутствует/`nil` → пусто; неверный контейнер → `ignored` (`d.members`);
  битый member (нет/отрицательный/нечисловой `user.id`, не-map `user`, не-map member) →
  `malformed` (`d.members[i]`), остальные входят;
- `not_found` отсутствует/`nil` → пусто; неверный контейнер → `ignored` (`d.not_found`);
  элемент не u64 → `ignored` (`d.not_found[i]`);
- `nonce` отсутствует/`nil` → `None`; пустая/не-текстовая строка → `ignored` (`d.nonce`),
  `None`;
- `presences` присутствует и не `nil` → `ignored` (`d.presences`), не разбирается;
- дубликат `user.id` внутри `members` → last wins, позиция первого, `duplicates`
  `member_chunk:{guild}:{nonce|none}:duplicate-member:{user_id}`.

`MemberChunkMember.user` разбирается тем же `parse_user`, что READY/P2-04; `member` —
`parse_member_fields` с `user.id`. `roles: null` → пустой Vec; отсутствующие поля member —
дефолты без паники.

## 4. Reducer (`apply_chunk`)

1. Guard: `chunk_count == 0` или `chunk_index >= chunk_count` → `inconsistencies`
   `member_chunk:{guild}:{nonce|none}:invalid-index:{index}/{count}`; событие не меняет
   состояние (на практике это уже отсекает парсер).
2. Найти/создать `MemberChunkAssembly` по `(guild_id, nonce)`.
3. Если у существующей сборки другой `chunk_count` → `inconsistencies`
   `member_chunk:{guild}:{nonce|none}:chunk-count:{old}->{new}`; новый chunk не вставляется.
4. Вставить chunk по `chunk_index`; повторный индекс — last wins. `duplicates`
   `member_chunk:{guild}:{nonce|none}:duplicate-index:{index}` пишется для correlated
   (`nonce: Some`) и для `nonce: None` при `chunk_count > 1` (batch без корреляции —
   потенциальный дубль внутри ответа). Единственное исключение — `nonce: None`,
   `chunk_count=1`: повтор `0/1` является **новым независимым ответом** (capture: s=78 и
   s=84 одной гильдии), без duplicate-диагностики. Члены/`not_found` нового ответа
   заменяют снимок chunk-а; ранее слитые `Guild.members`/`State.users` не удаляются.
5. Аддитивный merge каждого member:
   - `State.users.insert(user.id, user.clone())` — last wins без диагностики (нормальный
     повтор при догрузке);
   - `Guild.members`: найти по `user_id`, заменить; иначе push; порядок — порядок прихода
     chunk-ов и членов внутри chunk;
   - гильдии нет → `inconsistencies` `guilds:{guild}:member-chunk`; сборка всё равно
     сохраняется.
6. `not_found` сохраняется в chunk; участники не удаляются, `Guild.member_count` не меняется.
7. `MemberChunkAssembly::is_complete()` — `chunk_count > 0`, есть ровно все индексы
   `0..chunk_count`; пропуски/out-of-order не блокируют приём, полнота вычисляется
   селектором.

Stale `s` обрабатывается общим reducer до разбора. Effects для chunk не создаются.

## 5. Селекторы (`impl State`, `member_chunk_selectors.rs`)

```rust
member_chunks(guild_id) -> impl Iterator<Item = &MemberChunkAssembly> + '_
member_chunk(guild_id, nonce: Option<&str>) -> Option<&MemberChunkAssembly>
member_chunk_is_complete(guild_id, nonce: Option<&str>) -> bool
member_chunk_len(guild_id, nonce) -> usize
member_chunk_member_count(guild_id, nonce) -> usize
member_chunk_members(guild_id, nonce) -> impl Iterator<Item = &MemberChunkMember> + '_
member_chunk_member(guild_id, nonce, user_id) -> Option<&MemberChunkMember>
member_chunk_not_found(guild_id, nonce) -> Vec<u64>  // flattened, порядок chunk-ов
member_chunk_count() -> usize
```

`member_chunk_members` идёт по возрастанию `chunk_index`, затем по порядку `members`;
`member_chunk_member` возвращает первый найденный (стабильно). Все — только чтение.

### Видимый диапазон и presence

```rust
member_list_presence_effect(
    guild_id: u64,
    list_id: &MemberListId,
    range: MemberListRange,
) -> Option<Effect>
```

Собирает member-ids из плоского `items` в позициях `range` (включительно), пропускает
group-заголовки, дедуплицирует с сохранением первого порядка, возвращает
`Effect::RequestPresence { guild_id, user_ids }`. Пустой список/диапазон/только группы →
`None`. Это **декларация**, без сети и без wire; исполнитель — P2-09/transport.

## 6. Инварианты

1. `MemberChunk` — снимок одного chunk; `Assembly` — набор по nonce.
2. Ключ сборки — `(u64, Option<String>)`; `None` — nonce отсутствовал.
3. `chunk_count` фиксируется первым chunk-ом; конфликт — диагностика, не перезапись.
4. Дубликат индекса — last wins + `duplicates`, кроме независимого `nonce=None/count=1`;
   полнота — только при всех индексах.
5. `Guild.members`/`State.users` merge аддитивный, без удаления; `not_found` не удаляет.
6. `Guild.member_count`, `MemberList` и `State.messages` chunk-событием не меняются.
7. Ошибка одного chunk-а не отменяет уже применённые и не паникует.
8. Presence effect — чистая функция чтения; никаких network/async/UI.
