# RW-18 / DESIGN-P2-09 — memory-only presence

Дата: 2026-10-20. Карточка: P2-09. Зависимость: P2-02 (принята), P2-04/P2-05
(минимальный presence и chunk `presences[]`). Research: `waves/RW-18/RESEARCH-P2-09.md`.

## 1. Scope

**Входит:**

- `PRESENCE_UPDATE` (обязательный `d.user.id`, optional `guild_id`), `EventPayload::PresenceUpdate`,
  маршрут `DispatchKind::PresenceUpdate`.
- `READY_SUPPLEMENTAL.merged_presences.friends` и `.guilds` — начальный снимок.
- `GUILD_MEMBER_LIST_UPDATE.member.presence` — полные поля (activities/timestamps), аддитивное
  слияние в общий presence-store; `State.users` по-прежнему не меняется (P2-04 frozen).
- `GUILD_MEMBERS_CHUNK.presences[]` — best-effort: только элементы формы `PRESENCE_UPDATE`;
  форма в capture не подтверждена, поэтому при наличии списка ставится inconsistency
  `member_chunk:<guild>:presences:unconfirmed-shape`. Запрос presence не изобретается.
- Модель статусов, client statuses, activities (безопасное подмножество полей), hidden
  activities, processed timestamp, scope `guild|private`, селекторы, диагностика, root re-exports.
- Memory-only; без SQLite, UI, сети, live Discord, новых зависимостей.

**Не входит:** `secrets`, `session_id`, `sync_id`, `restricted_application`,
`content_classification`, `official_game_id`, `game`, TTL/лимиты, presence-запросы,
voice/typing (P2-10/P2-11), GUI, SQLite.

## 2. Публичные типы (`discord_core::state`, root `discord_core`)

```rust
pub enum PresenceStatus {
    Online, Idle, Dnd, Offline, Invisible, Unknown(String),
}
impl PresenceStatus {
    pub fn from_wire(value: &str) -> Self;
    pub fn as_str(&self) -> &str;
    pub fn is_online(&self) -> bool;   // Online | Idle | Dnd
    pub fn is_offline(&self) -> bool;  // Offline
}
impl Default for PresenceStatus { fn default() -> Self { Self::Offline } }

pub enum ActivityType {
    Playing, Streaming, Listening, Watching, Custom, Competing, Unknown(i64),
}
impl ActivityType {
    pub fn from_i64(value: i64) -> Self;
    pub fn as_i64(self) -> i64;
    pub fn priority(self) -> u8; // custom 4, competing 3, streaming 2, playing 1, иначе 0
    pub fn is_custom(self) -> bool;
    pub fn is_playing(self) -> bool;
}
impl Default for ActivityType { fn default() -> Self { Self::Unknown(-1) } }

pub struct ActivityTimestamps { pub start: Option<i64>, pub end: Option<i64> }
pub struct ActivityEmoji {
    pub name: Option<String>, pub id: Option<String>, pub animated: Option<bool>,
}
pub struct ActivityAssets {
    pub large_image: Option<String>, pub large_text: Option<String>,
    pub small_image: Option<String>, pub small_text: Option<String>,
}
pub struct ActivityParty {
    pub id: Option<String>, pub current_size: Option<u32>, pub max_size: Option<u32>,
}
pub struct Activity {
    pub activity_type: ActivityType,
    pub name: String,
    pub details: Option<String>,
    pub state: Option<String>,
    pub url: Option<String>,
    pub id: Option<String>,
    pub platform: Option<String>,
    pub created_at: Option<i64>,
    pub application_id: Option<String>,       // строкой: в capture бывает отрицательным
    pub parent_application_id: Option<String>,
    pub timestamps: Option<ActivityTimestamps>,
    pub emoji: Option<ActivityEmoji>,
    pub assets: Option<ActivityAssets>,
    pub party: Option<ActivityParty>,
    pub buttons: Vec<String>,
    pub supported_platforms: Vec<String>,
    pub flags: Option<i64>,
    pub status_display_type: Option<i64>,
}

pub enum PresenceScope { Guild(u64), Private }
pub type PresenceKey = (u64, PresenceScope);

pub struct Presence {
    pub user_id: u64,
    pub guild_id: Option<u64>,               // None = приватный scope
    pub status: PresenceStatus,
    pub client_status: BTreeMap<String, PresenceStatus>,
    pub activities: Vec<Activity>,           // нормализованы
    pub hidden_activities: Vec<Activity>,
    pub processed_at_timestamp: Option<i64>,
    pub since: Option<i64>,
    pub broadcast: Option<bool>,
}
impl Presence {
    pub fn scope(&self) -> PresenceScope;
    pub fn is_visible(&self) -> bool; // status != Offline || !hidden_activities.is_empty()
}

pub struct PresenceUpdate {
    pub presence: Presence,
    pub user: Option<User>, // live-событие; initial/chunk могут не иметь
}

pub struct MergedPresences {
    pub friends: Vec<Presence>,
    pub guilds: Vec<Vec<Presence>>, // длина = guild_ids.len(), null -> []
}
impl Default for MergedPresences { ... }
```

`State` получает `pub presences: BTreeMap<PresenceKey, Presence>`; `State::from_ready`
инициализирует пустой map; `Debug` печатает только количество. `ReadySupplemental`
получает `pub merged_presences: MergedPresences` (derive `Default`).

## 3. Разбор

### `PRESENCE_UPDATE`

`Event::from_routed`:

```rust
Some(DispatchKind::PresenceUpdate) => {
    let (update, diagnostics) = presence::parse_presence_event(&dispatch.data)?;
    (EventPayload::PresenceUpdate(update), diagnostics)
}
```

- `d` не map → `WrongType`.
- `d.user` отсутствует → `Missing("presence.user")`; не map → `WrongType`.
  `parse_user` не смог → `InvalidId { path: "presence.user.id", ... }`.
- `d.guild_id` отсутствует/null → `None`; положительная снежинка → `Some`; иначе
  `ignored "presence.guild_id"` и `None`.
- `status` отсутствует/null/не текст → `Offline`, `ignored "presence.status"`.
  Любой непустой текст → `PresenceStatus::from_wire` (неизвестный сохраняется).
- `client_status`: map; ключ/значение должны быть непустым текстом; битые пары —
  `ignored "presence.client_status.<key>"`; null/отсутствие → пусто.
- `activities`/`hidden_activities`: list; null/отсутствие → пусто; не-list → `ignored`.
  Элемент не-map/без непустого `name`/без int `type` → `ignored <path>[i]`, пропуск.
- `processed_at_timestamp`/`since`: int или десятичная строка (включая отрицательные);
  иначе `ignored`; отсутствие/null → `None`.
- `broadcast`: bool; иначе `ignored`; отсутствие/null → `None`.
- `activities` нормализуются (§4). `offline` без hidden очищает activities; при наличии
  hidden activities сохраняются (как в официальном store; TZ r09 исправлен).
- `user` из live-события upsert-ится в `State.users` с существующей merge-политикой.

### Activity

- `type` — int; `name` — непустой текст; остальное optional.
- `created_at`, `timestamps.start/end` — int/десятичная строка.
- `application_id`, `parent_application_id` — текст (в т.ч. отрицательный) либо int → строка.
- `party.size` — подтверждённая двухбайтовая строка (`\x01\x03` → 1/3) либо list/tuple
  из двух неотрицательных чисел; иначе `ignored`.
- `buttons`/`supported_platforms` — list непустых строк; битые элементы — `ignored`.
- `assets`/`emoji`/`timestamps` — map; не-map → `ignored`.
- `secrets` не читаются; `session_id`/`sync_id` не сохраняются.

### `READY_SUPPLEMENTAL`

- `merged_presences` отсутствует/null → `MergedPresences::default()`.
- не-map → `ignored "merged_presences"`.
- `friends`: list; элемент без `user_id`/невалидный → `malformed`; иначе presence с
  `guild_id=None`.
- `guilds`: list, выровнен по `d.guilds[]`; `null`-группа → `[]`; не-list → `ignored`;
  элементов больше, чем guild ids → лишние `malformed`/игнор; меньше → пустые группы.
- Группа с неизвестным/битым guild id и непустым содержимым → `inconsistencies
  "merged_presences.guilds[i]"`, группа не применяется.
- `user_id` в элементах обязателен; иначе `malformed`.
- Длина `guilds` не совпала с `d.guilds` → `malformed
  "merged_presences.guilds:N vs guilds:M"`.

### Member list

`MemberPresence` расширяется аддитивно:

```rust
pub struct MemberPresence {
    pub status: Option<String>,
    pub client_status: BTreeMap<String, String>,
    pub activities: Vec<Activity>,
    pub hidden_activities: Vec<Activity>,
    pub processed_at_timestamp: Option<i64>,
}
```

Парсер сохраняет прежние status/client_status и добавляет полные activities; `null`/отсутствие
→ пусто. `MemberPresence` без `status` в presence-store не попадает.

### Chunk

`MemberChunk` получает `pub presences: Vec<PresenceUpdate>`. Если `d.presences` нет/null —
пусто и без диагностики. Если есть list:
- каждый элемент разбирается как `PRESENCE_UPDATE`; `guild_id` по умолчанию — гильдия chunk;
  явно другой `guild_id` → inconsistency;
- битый элемент → `malformed d.presences[i]`, остальные сохраняются;
- добавляется `inconsistencies "member_chunk:<guild>:<nonce|none>:presences:unconfirmed-shape"`.

## 4. Нормализация activities

Повторяет официальный `C`/`O` (`mod-290863`):

1. Приоритет типа: custom (4) → competing (3) → streaming (2) → playing (1) → остальные (0).
2. При равенстве: «displayable» (не custom и есть details/assets.large_image/assets.small_text/
   party/state) — выше.
3. При равенстве: `created_at` по убыванию.
4. Если playing-активностей больше одной, остаётся лучшая по тем же правилам.
5. Результат стабилен и не зависит от порядка провода для равных ключей (сортировка по
   исходному порядку — stable sort).

## 5. Reducer

`presence_reducer::apply_update(state, update)`:

- upsert user (если есть), затем presence.
- scope = `Guild(guild_id)` или `Private`.
- offline без hidden activities:
  - если у пользователя ещё нет ни одного scope — no-op;
  - иначе записать offline-scope с пустыми activities; если **все** scopes offline и hidden
    пусты — удалить все scope пользователя. Offline-scope без других scope не остаётся.
- hidden activities делают offline-scope видимым (не удаляется).
- live scope заменяет предыдущую запись этого scope (last wins), activities нормализуются.
- `PRESENCE_UPDATE` не генерирует `Effects`.

`presence_reducer::apply_supplemental(state, guild_ids, merged)`:

- friends → `Private`;
- guilds[i] → `Guild(guild_ids[i])`, null → пусто;
- presence с user_id, отсутствующим в `State.users`, сохраняется, но один раз пишется
  inconsistency `presences:user:<id>:missing`;
- после снимка offline-only пользователи не остаются.

`presence_reducer::apply_member_presence(state, guild_id, user_id, &MemberPresence)`:

- status `None` → no-op;
- activities/timestamps переносятся; `State.users` не трогается;
- scope `Guild(guild_id)`; действуют те же offline-правила.

`GUILD_MEMBER_LIST_UPDATE`: перед применением ops собрать все Member presence из SYNC items и
INSERT/UPDATE item и применить их; DELETE не удаляет presence. Применение presence не зависит
от успеха list-op (как в официальном клиенте), но только для структурно разобранных элементов.

`GUILD_MEMBERS_CHUNK`: после проверки chunk_index/count применить `presences` (до проверки
наличия гильдии), затем участников.

`READY` сбрасывает presence через `State::from_ready`.

## 6. Селекторы (`impl State`)

```rust
presence(user_id) -> Option<&Presence>                 // выбранный видимый scope
presence_in_guild(user_id, guild_id) -> Option<&Presence>
presence_scope(user_id, scope) -> Option<&Presence>
presence_scopes(user_id) -> impl Iterator<Item=&Presence>
presence_scope_count(user_id) -> usize
presence_user_count() -> usize
presence_user_ids() -> Vec<u64>                        // возрастание
has_presence(user_id) -> bool
status(user_id) -> PresenceStatus                      // по умолчанию Offline
status_in_guild(user_id, guild_id) -> PresenceStatus
client_status(user_id) -> Option<&BTreeMap<String, PresenceStatus>>
client_status_in_guild(user_id, guild_id) -> Option<&BTreeMap<String, PresenceStatus>>
activities(user_id) -> &[Activity]
activities_in_guild(user_id, guild_id) -> &[Activity]
hidden_activities(user_id) -> &[Activity]
primary_activity(user_id) -> Option<&Activity>         // первый не-custom
custom_status(user_id) -> Option<&Activity>            // type Custom
activity_by_type(user_id, ActivityType) -> Option<&Activity>
is_online(user_id) -> bool
is_offline(user_id) -> bool                             // отсутствие = true
is_mobile_online(user_id) -> bool
is_vr_online(user_id) -> bool
online_user_ids() -> Vec<u64>                           // возрастание
guild_online_count(guild_id) -> usize                   // scope с is_online()
```

Выбор агрегата: если среди **видимых** scopes есть хотя бы один online/idle/dnd,
выбирается max по `(processed_at_timestamp, activities.len())` только среди них; иначе
(все видимые offline+hidden) — max среди всех видимых. При полном равенстве — max по
`PresenceKey` (детерминированно). Так offline+hidden не понижает живой online-агрегат
(oracle PresenceStore). `status`/`activities` без guild используют агрегат; с guild —
точный scope. `is_mobile_online` — mobile == Online и desktop/vr != Online.

## 7. Диагностика и безопасность

- `ignored` — необязательные поля неверного типа/неизвестные значения, не отменяющие событие.
- `malformed` — битые элементы списков (activity/member/merged presence) с путём.
- `inconsistencies` — неизвестная гильдия, отсутствующий user, chunk shape, конфликт guild_id.
- Никаких `session_id`/`secrets`/`sync_id` в модели; `Debug` не печатает session_id.
- Ошибки не глотаются: каждая ветка либо меняет данные, либо пишет диагностику, либо
  задокументирована как no-op (offline без записи).
- Без паник: все `get`/`expect` только для инвариантов, созданных в этом же коде.

## 8. Файлы и границы

| Файл | Содержание |
|---|---|
| `src/state/presence.rs` | типы, парсеры, normalize, MergedPresences |
| `src/state/types.rs` | `State.presences`, `ReadySupplemental.merged_presences`, EventPayload/route |
| `src/state/presence_reducer.rs` | upsert/offline/supplemental/member/chunk helpers |
| `src/state/presence_selectors.rs` | `impl State` селекторы |
| `src/state/reducer.rs` | ветка `PresenceUpdate`, supplemental merge, `upsert_user` pub(crate) |
| `src/state/member_list.rs` | поля/парсер `MemberPresence`, сбор presence из ops |
| `src/state/member_chunk.rs` | `presences`, парсер, применение |
| `src/state/mod.rs`, `src/lib.rs` | модули и root re-exports |
| `tests/presence_types.rs`, `presence_reducer.rs`, `presence_selectors.rs` | TDD |

Пересечений с voice/typing нет. P2-04/P2-05 публичные поля расширяются аддитивно; существующие
тесты не переписываются под реализацию. Старый контракт явно меняется в трёх пунктах
(`PRESENCE_UPDATE` больше не Unsupported; `ReadySupplemental` literal; `GUILD_MEMBERS_CHUNK`
`presences[]` из `ignored` становится best-effort с unconfirmed-shape), которые затрагивают
четыре тестовых файла; все они перечислены в `TEST-CORRECTIONS.md`.
