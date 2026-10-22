# RW-12 / P2-01 — контракт парсера READY

**Статус:** контракт **A1 заморожен** координатором 2026-10-20. Каркас `ready/` подключён,
`Ready::parse` — композиция заглушек; JSON-адаптер P-1 готов (`tests/support/ready_fixture.rs`,
2/2). `TASKS.md`/`waves/STATE.md` обновлены; `waves/db.jsonl` ждёт приёмки W12.

**Карточка:** `docs/rust-client/PLAN.md:69` (P2-01, L, зависит от P1-06).
**Оракул-фикстура:** `fixtures/gateway/by-type/002-READY.json` (579 121 Б,
`md5 a1a8fd5662be88108dd25ac619a377f2`), `fixtures/gateway/ready-stats.json`
(`md5 972065245b4f215b76314fbd7e89b573`).
**Схемы RE:** `docs/re/gateway-schema.md` §3 (строки 121–399), `docs/re/gateway-protocol.md` §5
(строки 163–213), `docs/re/gateway-schema.json` (событие `READY`).
**Тесты:** `docs/rust-client/TEST-PLAN.md:83-85` (T-DATA-01…03).

---

## 0. Главные выводы (что важно до старта)

1. **В репозитории нет `gateway/*`, `models/*`, `state`.** `discord-core` плоский:
   `gateway.rs`, `ready.rs`, `dispatch.rs`, `ws.rs`, `rest/*`. READY сейчас разбирается только в
   `ReadySummary` (`rust/crates/discord-core/src/ready.rs:9-40`) — это счётчики, а не модель.
   P2-01 создаёт первую типизированную модель READY; P2-02 — редьюсер.
2. **`serde`/`serde_json` в `Cargo.lock` нет.** Зависимости `discord-core` —
   `rust/crates/discord-core/Cargo.toml:8-20`; существующий стиль — ручной разбор `EtfValue`
   (`rust/crates/discord-codec/src/etf.rs:113-190`). Даже JSON супер-свойств пишется/читается
   вручную (`rest/super_properties.rs:81-135,261`). Новые зависимости для P2-01 **не нужны**.
3. **Фикстура и `gateway-schema.md` расходятся по ID.** В фикстуре ID — положительные строки
   (`384299970632876032`, `1014537851418980372`), это исправленный big-int. В примерах
   `gateway-schema.md` местами стоят отрицательные значения старого парсера
   (`-6147072113468506112`). Оракул ID — **фикстура**, причина расхождения —
   `gateway-protocol.md:105-117`.
4. **`member_count` крупной гильдии в фикстуре — 24 228, в доках — 24 229.** Это разные
   снапшоты, зафиксировано в `docs/rust-client/VERIFICATION.md:14-15,34-38`. Тест P2-01 обязан
   использовать **24 228** (значение `002-READY.json`).
5. **`ready-stats.json` не содержит `users` (146) и `sessions`.** T-DATA-02/03 ссылаются на него
   неточно. `users=146`, `sessions=1`, `read_state=157` считать по `002-READY.json` (и
   `gateway-protocol.md:163-187`).
6. **В READY нет `presences`, `voice_states`, `members`, `threads`.** `merged_members` — 7 групп
   по 1 участнику, а не 24 229. Полный список участников — P2-04, presence — P2-09, voice —
   P2-10 (`gateway-schema.md:397`, `gateway-protocol.md:207-209,246-253`).
7. **`user_settings_proto` — base64-текст 4400 символов**, а не сырой protobuf. P2-01 хранит
   строку; декодирование — P2-08.

---

## 1. Источники и независимые оракулы

| Оракул | Что подтверждает | Команда/ссылка |
|---|---|---|
| `fixtures/gateway/by-type/002-READY.json` | полный `d` READY: 40 ключей, все вложенные объекты | `jq`-команды ниже; `md5sum` |
| `fixtures/gateway/ready-stats.json` | 7 гильдий, 17 ЛС, 60 связей, 40 ключей, `userKeys=32` | `jq '{guilds,privateChannels,relationships,top:(.topKeys|length)}'` |
| `docs/re/gateway-schema.json` | 39 полей + `hasTrace=1` = 40; presence-строки | `jq '.events[]|select(.t=="READY")|.fields|keys|length'` → `39` |
| `docs/re/gateway-schema.md:121-399` | типы, presence, вложенные ключи, «чего нет» | таблицы §3 |
| `docs/re/gateway-protocol.md:163-213` | числа READY и `READY.guilds[]`; big-int | §5–6 |
| `docs/rust-client/VERIFICATION.md:14-15,34-38` | `member_count` 24 228 vs 24 229 | §2.1 |
| `fixtures/MANIFEST.json:105-112` | провенанс каталога `gateway/by-type` (164 файла) | `FIX-01` |

Базовые проверки фикстуры:

```sh
jq -c '{top:(.d|keys|length),guilds:(.d.guilds|length),channels:([.d.guilds[].channels|length]|add),
 roles:([.d.guilds[].roles|length]|add),emojis:([.d.guilds[]|(.emojis//[])|length]|add),
 stickers:([.d.guilds[]|(.stickers//[])|length]|add),private:(.d.private_channels|length),
 users:(.d.users|length),rels:(.d.relationships|length),rs:(.d.read_state.entries|length),
 ugs:(.d.user_guild_settings.entries|length),overrides:([.d.user_guild_settings.entries[].channel_overrides//[]|length]|add),
 sessions:(.d.sessions|length),mm:(.d.merged_members|map(length)),user_keys:(.d.user|keys|length)}' \
 fixtures/gateway/by-type/002-READY.json
```

Ожидаемо: `top=40, guilds=7, channels=272, roles=132, emojis=363, stickers=51, private=17,
users=146, rels=60, rs=157, ugs=58, overrides=1328, sessions=1, mm=[1,1,1,1,1,1,1],
user_keys=32`.

---

## 2. Контракт READY

### 2.1 Конверт и вход

`Ready::from_dispatch(frame)` принимает **полный op0-кадр**:
`op == 0` (строго `Int`), `t == "READY"` (атом или строка), `d` — map. Иначе `NotReady`
(или `Missing { path:"d" }`, если `d` нет).

`Ready::parse(d)` принимает **только payload `d`**. Это основная функция для P2-02.

### 2.2 40 ключей верхнего уровня

Все 40 присутствуют в каждом из 4 READY-инстансов (`gateway-protocol.md:163-187`). `T` — типизировано
в P2-01, `R` — сохраняется только в `Ready::raw()` (без типизации). Секреты (`analytics_token`,
`qos_token`, `auth_session_id_hash`, `static_client_session_id`) — тоже `R`, в `Debug` не попадают.

| # | Ключ | В захвате | Решение | Rust |
|---:|---|---|---|---|
| 1 | `_trace` | `list` | R | — |
| 2 | `ad_personalization_toggles_disabled` | `bool`-атом | T | `bool` |
| 3 | `analytics_token` | `str` (REDACTED) | R | — |
| 4 | `apex_experiments` | `obj` | R | — |
| 5 | `api_code_version` | `int` | T | `Option<i64>` |
| 6 | `auth` | `obj` | R | — |
| 7 | `auth_session_id_hash` | `str` (REDACTED) | R | — |
| 8 | `broadcaster_user_ids` | `null` | R | — |
| 9 | `connected_accounts` | `null` | R | — |
| 10 | `consents` | `obj` | R | — |
| 11 | `country_code` | `str` | T | `Option<String>` |
| 12 | `experiments` | `list` | R | — |
| 13 | `explicit_content_scan_version` | `int` | T | `Option<i64>` |
| 14 | `friend_suggestion_count` | `int` | T | `Option<i64>` |
| 15 | `game_relationships` | `null` | R | — |
| 16 | `geo_ordered_rtc_regions` | `list[str]` | T | `Vec<String>` |
| 17 | `guild_experiments` | `list` | R | — |
| 18 | `guild_join_requests` | `null` | R | — |
| 19 | `guilds` | `list` | T | `Vec<Guild>` |
| 20 | `linked_users` | `null` | R | — |
| 21 | `merged_members` | `list[list[obj]]` | T | `Guild.members` |
| 22 | `notification_settings` | `obj` | R | — |
| 23 | `pending_payments` | `str "nil"` | R | — |
| 24 | `private_channels` | `list` | T | `Vec<PrivateChannel>` |
| 25 | `qos_token` | `str` (REDACTED) | R | — |
| 26 | `read_state` | `obj` | T | `ReadState` |
| 27 | `regional_feature_config` | `obj` | R | — |
| 28 | `relationships` | `list` | T | `Vec<Relationship>` |
| 29 | `resume_gateway_url` | `str` (REDACTED) | T | `Option<String>` |
| 30 | `session_id` | `str` (REDACTED) | T | `String` |
| 31 | `session_type` | `str` | T | `Option<String>` |
| 32 | `sessions` | `list` | T | `Vec<Session>` |
| 33 | `static_client_session_id` | `str` (REDACTED) | R | — |
| 34 | `subdivision_code` | `str` | T | `Option<String>` |
| 35 | `tutorial` | `str "nil"` | R | — |
| 36 | `user` | `obj` (32 ключа) | T | `User` |
| 37 | `user_guild_settings` | `obj` | T | `UserGuildSettings` |
| 38 | `user_settings_proto` | `str` (4400 симв.) | T | `Option<String>` (base64, opaque) |
| 39 | `users` | `list` (146) | T | `Vec<User>` |
| 40 | `v` | `int` (9) | T | `Option<i64>` |

Итого: **21 T + 19 R = 40**. `_trace` входит в 40; в `gateway-schema.json` он учтён отдельным
`hasTrace`, поэтому там `fields=39`.

### 2.3 Обязательность, дефолты, ошибки

| Категория | Поля | Политика |
|---|---|---|
| **Fatal** (Err) | `d` — есть и map; `d` отсутствует → `Missing`; не-map `d`: `from_dispatch` → `NotReady`, `Ready::parse` → `WrongType`; `d.guilds` — есть и list; `d.user` — map; `d.user.id` — валидная снежинка; `d.session_id` — есть и текст | `Missing`/`NotReady`/`WrongType`/`InvalidId` |
| **Опциональные** (дефолт) | все остальные 35 ключей и все поля вложенных объектов, кроме явно перечисленных ниже | отсутствие → `None`/`0`/`false`/`[]`; неверный тип → дефолт + `diagnostics.ignored` |
| **Обязательные поля элемента** | `Guild.id`; `Channel.id`,`Channel.type`; `Role.id`; `Emoji.id`; `Sticker.id`; `User.id`; `PrivateChannel.id`; `Relationship.id`,`user_id`,`type`; `GuildMember.user_id`; `ReadStateEntry.id`; `ChannelOverride.channel_id`; `Session.session_id` | нет/битый id/тип → элемент **пропускается** + `diagnostics.malformed` |
| **Дубликаты** | `guilds`, `channels` (внутри гильдии), `roles`, `emojis`, `stickers`, `users`, `private_channels`, `relationships`, `read_state.entries`, `user_guild_settings.entries`, `members` (внутри группы) | **last wins**, позиция первого вхождения; `diagnostics.duplicates` |
| **Неизвестные поля** | любые ключи сверх `TOP_LEVEL_KEYS` | не ошибка; сохраняются в `raw`; top-level имя → `diagnostics.unknown_top_level` |
| **Пусто/`nil`** | опциональные списки | `Nil`, атом `"nil"`, отсутствие и `[]` эквивалентны пустому `Vec` |
| **Bool** | `true`/`false` | атом/строка `"true"`/`"false"` либо `Int 0/1`; иное → `None` |
| **Текст** | строковые поля | `Str` или `Atom`, кроме зарезервированных `nil`/`true`/`false`; иное → `None` + `ignored` (ревью P1-1) |
| **ID** | все снежинки | `u64`: `Int ≥ 0` или десятичная `Str`/`Atom` без знака/пробелов; отрицательные (старый big-int) → невалидны |
| **Время** | `joined_at`, `since`, `last_pin_timestamp`, … | строки ISO-8601 как есть, без chrono |
| **`merged_members`** | верхнеуровневый список групп | группа с сырым индексом `i` прикрепляется к гильдии из сырого `guilds[i]` (после skip/дедупликации — по карте сырых индексов); лишние/недостающие — `diagnostics.malformed`; дубликат `user_id` в группе — last wins; явный `[]` при непустых `guilds` — `malformed` (ревью P2-3/P3-1); для дубликата гильдии побеждает группа последнего сырого вхождения, группы предыдущих — `malformed` (P3-4) |

**Важно:** `read_state.entries[].id` — это не только каналы. В фикстуре есть записи с id
пользователя (`384299970632876032`) и гильдии (`1014537851418980372`). P2-01 не проверяет id по
списку каналов; интерпретация — P2-07.

### 2.4 Вложенные объекты: подтверждённые ключи

| Объект | Кол-во ключей | Обязательные | Подтверждённая опциональность (capture) |
|---|---:|---|---|
| `guilds[]` | 19 | `id` | `emojis`: list 5/7 или `null` 2/7; `stickers`: list 3/7 или `null` 4/7; `threads`, `guild_scheduled_events`, `stage_instances` — `null` 7/7 |
| `guilds[].channels[]` | 34 | `id`, `type` | см. таблицу 2.4.1 |
| `guilds[].roles[]` | 13 | `id` | все 13/13; `name`, `color`, `hoist`, `permissions`, `position` — всегда |
| `guilds[].emojis[]` | 7 | `id` | все 363/363; `roles` = `null` → `[]` |
| `guilds[].stickers[]` | 9 | `id` | все 51/51; `description` строка 24/51 или `nil` 27/51 |
| `guilds[].properties` | 45 | — | `name`, `icon`, `owner_id` — всегда; остальное raw |
| `private_channels[]` | 11 | `id` | `last_pin_timestamp` 2/17; `last_message_id` `nil` 1/17; все `type=1` |
| `users[]` | 13–14 | `id` | `bot` 105/146 (41 отсутствует), `display_name` 102/146, `clan` 60/146; остальные 13/146 |
| `user` (top-level) | 32 | `id` | все 32/32; `email` REDACTED, `phone` `nil` |
| `relationships[]` | 7 | `id`, `user_id`, `type` | все 60/60; `nickname` `nil` 60/60 |
| `merged_members[][]` | 12 | `user_id` | все 7/7; `roles` list 5/7 или `null` 2/7 |
| `read_state.entries[]` | 9 | `id` | см. таблицу 2.4.2 |
| `user_guild_settings.entries[]` | 13 | — (ключ `guild_id` может быть `nil`) | `channel_overrides` `null` 6/58; `guild_id` `nil` 1/58; `mute_config` obj 21/58, `nil` 37/58 |
| `...channel_overrides[]` | 6 | `channel_id` | `flags` 1253/1328 (75 отсутствует); `mute_config` obj 38/1328, `nil` 1290/1328 |
| `sessions[]` | 6 | `session_id` | все 1/1; `activities`, `hidden_activities` — `null` |
| **`voice_state`** | — | — | **в READY нет**; только `READY_SUPPLEMENTAL.guilds[].voice_states` (1 шт.) → P2-10 |
| **`presence`** | — | — | **ключа `presences` нет**; `READY_SUPPLEMENTAL.merged_presences` → P2-09 |

#### 2.4.1 `channels[]`: поля с присутствием < 272/272

| Поле | Присутствие | В P2-01 |
|---|---:|---|
| `application_id` | 134/272 | raw |
| `available_tags` | 68/272 | raw |
| `bitrate` | 82/272 | `Option<i64>` |
| `default_auto_archive_duration` | 6/272 | `Option<i64>` |
| `default_forum_layout` | 1/272 | raw |
| `default_reaction_emoji` | 68/272 | raw |
| `default_sort_order` | 68/272 | raw |
| `default_tag_setting` | 68/272 | raw |
| `default_thread_rate_limit_per_user` | 7/272 | raw |
| `game_id` | 62/272 | raw |
| `hd_streaming_buyer_id` | 62/272 | raw |
| `hd_streaming_until` | 62/272 | raw |
| `icon_emoji` | 191/272 | raw |
| `last_message_id` | 237/272 | `Option<u64>` |
| `last_pin_timestamp` | 86/272 | `Option<String>` |
| `linked_lobby` | 62/272 | raw |
| `nsfw` | 237/272 | `bool` (дефолт `false`) |
| `rate_limit_per_user` | 229/272 | `Option<i64>` |
| `rtc_region` | 134/272 | `Option<String>` |
| `status` | 126/272 | raw |
| `template` | 9/272 | raw |
| `theme_color` | 191/272 | raw |
| `topic` | 165/272 | `Option<String>` |
| `user_limit` | 82/272 | `Option<i64>` |
| `video_quality_mode` | 3/272 | `Option<i64>` |
| `voice_background_display` | 113/272 | raw |
| `voice_hangout` | 113/272 | raw |

Всегда 272/272: `flags`, `id`, `name`, `parent_id`, `permission_overwrites`, `position`, `type`.
`name` в модели `Option<String>` — на случай partial/unavailable.

#### 2.4.2 `read_state.entries[]`: опциональность

| Поле | Присутствие | Rust |
|---|---:|---|
| `id` | 157/157 | `u64` |
| `flags` | 155/157 | `Option<i64>` |
| `last_message_id` | 155/157 | `Option<u64>` |
| `last_pin_timestamp` | 155/157 | `Option<String>` |
| `mention_count` | 155/157 | `Option<i64>` |
| `last_viewed` | 143/157 | `Option<i64>` |
| `badge_count` | 2/157 | `Option<i64>` |
| `last_acked_id` | 2/157 | `Option<u64>` |
| `read_state_type` | 2/157 | `Option<i64>` |

### 2.5 Подтверждено захватом vs не подтверждено

| Подтверждено (4 READY / фикстура) | Не подтверждено (не закладывать как факт) |
|---|---|
| 40 top-level ключей; 7 гильдий; 17 ЛС; 60 связей; 146 users; 157 read_state; 58 UGS; 1328 overrides; 272 канала; 132 роли; 363 эмодзи; 51 стикер; 7 групп `merged_members` по 1 | `guild.unavailable` в READY (в захвате нет; поддержать опционально) |
| ID — положительные; big-int без суффикса `n`; отрицательные — след старого парсера | `presences` top-level; `guild.members`/`guild.presences`/`guild.voice_states` внутри READY |
| bool — атомы `"true"`/`"false"`; null — `Nil` или атом `"nil"` | `threads` не `null`; `guild_join_requests`, `connected_accounts`, `linked_users`, `game_relationships`, `broadcaster_user_ids` не `null` |
| `guild.name`/`icon`/`owner_id` лежат в `properties` (top-level `name` в capture нет) | `private_channels` type 3 (групповая ЛС); `relationship.type` 3–5 |
| `emojis`/`stickers` = `null` при нуле; `roles` эмодзи = `null` | `read_state.partial=true`; `user_guild_settings.partial=true` |
| `users[]` — partial: `bot`/`display_name`/`clan` могут отсутствовать | семантика отсутствующего `bot` (не бот vs неизвестно); `User.bot=false` — наша договорённость |
| `merged_members` выровнен по индексу с `guilds`; в capture 7×1 | несовпадение длин `merged_members`/`guilds`; дубликаты id; неизвестные поля |
| `user_settings_proto` — base64-строка 4400 символов | `session_type` ≠ `normal`; `sessions[].status` ≠ `invisible` |
| `read_state.entries[].id` может быть каналом, пользователем или гильдией | реальные `user.email`/`phone` (в фикстуре REDACTED/`nil`) |
| `guild.version` — целое-строка (1792391252875), `permissions`/`allow`/`deny` — decimal-строки | ID > `i64::MAX` (текущий кодек — `Int(i64)`) |

---

## 3. Rust-дизайн

### 3.1 Зависимости: почему без serde

| Факт | Доказательство |
|---|---|
| `serde`/`serde_json` отсутствуют в lock | `rust/Cargo.lock`: нет записей `name = "serde"`/`"serde_json"` |
| В `discord-core` нет serde в зависимостях | `rust/crates/discord-core/Cargo.toml:8-20` |
| READY уже разбирается вручную | `ready.rs:7,44-110`; `dispatch.rs:11,138-157` |
| ETF-тип и помощники | `discord-codec/src/etf.rs:113-190` |
| JSON в проекте пишется вручную | `rest/super_properties.rs:81-135` |

Добавление serde потребовало бы новых зависимостей и сети/офлайн-кэша; выигрыша нет, потому что
ETF-значения — это `Atom`/`Str`/`Int`/`List`/`Map`, а не JSON-типы. **Решение: ручной парсер над
`EtfValue`, как в существующем коде.**

Для тестов над JSON-фикстурой добавляется **только тестовый** адаптер
`tests/support/ready_fixture.rs` (JSON → `EtfValue`), без зависимостей. Он превращает строки
`"nil"`/`"true"`/`"false"` в `Atom`, числа — в `Int`, объекты — в `Map` с ключами-атомами. Это
сохраняет семантику провода (иначе `"nil"`-строка неотличима от настоящей строки `nil`).

### 3.2 Модули и файлы

`src/ready.rs` остаётся корнем модуля (там же `ReadySummary`), подмодули — в `src/ready/`:

```text
rust/crates/discord-core/src/ready.rs            # корень: pub mod ...; re-exports (координатор)
rust/crates/discord-core/src/ready/error.rs      # ReadyError, ReadyDiagnostics (координатор)
rust/crates/discord-core/src/ready/value.rs      # общие помощники над EtfValue (координатор)
rust/crates/discord-core/src/ready/snapshot.rs   # Ready, TOP_LEVEL_KEYS, parse (W12a)
rust/crates/discord-core/src/ready/guild.rs      # Guild/Channel/Role/Emoji/Sticker (W12a)
rust/crates/discord-core/src/ready/social.rs     # User/PrivateChannel/Relationship/Member/Session (W12b)
rust/crates/discord-core/src/ready/read_state.rs # ReadState/UGS/override (W12c)
rust/crates/discord-core/tests/ready_core.rs     # W12a
rust/crates/discord-core/tests/ready_social.rs   # W12b
rust/crates/discord-core/tests/ready_read_state.rs # W12c
rust/crates/discord-core/tests/support/ready_fixture.rs # JSON→EtfValue (координатор, read-only)
```

### 3.3 Типы (минимальный набор)

```rust
// ready/error.rs
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReadyError {
    NotReady,
    Missing { path: String },
    WrongType { path: String, expected: &'static str, found: &'static str },
    InvalidId { path: String, value: String },
}
impl std::fmt::Display for ReadyError { /* ... */ }
impl std::error::Error for ReadyError {}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadyDiagnostics {
    pub unknown_top_level: Vec<String>, // новые top-level ключи (wire order)
    pub malformed: Vec<String>,         // "guilds[2]", "guilds[2].channels[7]"
    pub duplicates: Vec<String>,        // "guilds:101...", "guilds:101....channels:117..."
    pub ignored: Vec<String>,           // "d.users", "d.read_state.version", ...
}

// ready/guild.rs
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Guild {
    pub id: u64,
    pub unavailable: bool,          // default false; в capture не встречался
    pub name: Option<String>,       // top-level name -> properties.name
    pub icon: Option<String>,       // top-level icon -> properties.icon
    pub owner_id: Option<u64>,      // properties.owner_id
    pub data_mode: Option<String>,
    pub joined_at: Option<String>,
    pub large: bool,
    pub lazy: bool,
    pub member_count: Option<i64>,
    pub premium_subscription_count: Option<i64>,
    pub version: Option<u64>,
    pub channels: Vec<Channel>,
    pub roles: Vec<Role>,
    pub emojis: Vec<Emoji>,
    pub stickers: Vec<Sticker>,
    pub members: Vec<GuildMember>,  // из merged_members[i]
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Channel {
    pub id: u64,
    pub channel_type: i64,          // wire "type"
    pub name: Option<String>,
    pub parent_id: Option<u64>,
    pub position: i64,              // default 0
    pub flags: i64,                 // default 0
    pub nsfw: bool,
    pub topic: Option<String>,
    pub last_message_id: Option<u64>,
    pub last_pin_timestamp: Option<String>,
    pub permission_overwrites: Vec<PermissionOverwrite>,
    pub rate_limit_per_user: Option<i64>,
    pub bitrate: Option<i64>,
    pub user_limit: Option<i64>,
    pub rtc_region: Option<String>,
    pub video_quality_mode: Option<i64>,
    pub default_auto_archive_duration: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PermissionOverwrite {
    pub id: u64,
    pub target_type: i64,           // wire "type": 0 role, 1 member
    pub allow: u64,                 // decimal-строка "412317240385"
    pub deny: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Role {
    pub id: u64,
    pub name: Option<String>,
    pub color: i64,
    pub hoist: bool,
    pub managed: bool,
    pub mentionable: bool,
    pub permissions: u64,           // decimal-строка
    pub position: i64,
    pub flags: i64,
    pub unicode_emoji: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Emoji {
    pub id: u64,
    pub name: Option<String>,
    pub animated: bool,
    pub available: bool,
    pub managed: bool,
    pub require_colons: bool,
    pub roles: Vec<u64>,            // null -> []
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Sticker {
    pub id: u64,
    pub name: Option<String>,
    pub description: Option<String>,
    pub tags: Option<String>,
    pub asset: Option<String>,
    pub format_type: i64,
    pub available: bool,
    pub guild_id: Option<u64>,
    pub sticker_type: i64,          // wire "type"
}

// ready/social.rs
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct User {
    pub id: u64,
    pub username: Option<String>,
    pub global_name: Option<String>,
    pub display_name: Option<String>, // только users[]
    pub discriminator: Option<String>,
    pub avatar: Option<String>,
    pub bot: bool,                    // default false (в users[] может отсутствовать)
    pub public_flags: Option<i64>,    // только users[]
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PrivateChannel {
    pub id: u64,
    pub channel_type: i64,
    pub flags: i64,
    pub recipient_ids: Vec<u64>,
    pub last_message_id: Option<u64>,
    pub last_pin_timestamp: Option<String>,
    pub is_message_request: bool,
    pub is_message_request_timestamp: Option<String>,
    pub is_spam: bool,
    pub recipient_flags: i64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Relationship {
    pub id: u64,
    pub user_id: u64,
    pub relation_type: i64,          // wire "type": 1/2 в capture
    pub nickname: Option<String>,
    pub since: Option<String>,
    pub is_spam_request: bool,
    pub user_ignored: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct GuildMember {
    pub user_id: u64,
    pub nick: Option<String>,
    pub avatar: Option<String>,
    pub banner: Option<String>,
    pub joined_at: Option<String>,
    pub premium_since: Option<String>,
    pub communication_disabled_until: Option<String>,
    pub deaf: bool,
    pub mute: bool,
    pub pending: bool,
    pub flags: i64,
    pub roles: Vec<u64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Session {
    pub session_id: String,          // Debug: "<redacted>"
    pub status: Option<String>,
    pub client_info: ClientInfo,
    pub processed_at_timestamp: i64,
}
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ClientInfo { pub client: Option<String>, pub os: Option<String>, pub version: i64 }

// ready/read_state.rs
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadState { pub entries: Vec<ReadStateEntry>, pub partial: bool, pub version: u64 }

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ReadStateEntry {
    pub id: u64,
    pub flags: Option<i64>,
    pub last_message_id: Option<u64>,
    pub last_pin_timestamp: Option<String>,
    pub mention_count: Option<i64>,
    pub last_viewed: Option<i64>,
    pub badge_count: Option<i64>,
    pub last_acked_id: Option<u64>,
    pub read_state_type: Option<i64>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserGuildSettings {
    pub entries: Vec<UserGuildSettingsEntry>,
    pub partial: bool,
    pub version: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct UserGuildSettingsEntry {
    pub guild_id: Option<u64>,       // "nil" у DM-настроек
    pub channel_overrides: Vec<ChannelOverride>,
    pub flags: i64,
    pub hide_muted_channels: bool,
    pub message_notifications: i64,
    pub mobile_push: bool,
    pub mute_config: Option<MuteConfig>,
    pub mute_scheduled_events: bool,
    pub muted: bool,
    pub notify_highlights: i64,
    pub suppress_everyone: bool,
    pub suppress_roles: bool,
    pub version: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ChannelOverride {
    pub channel_id: u64,
    pub collapsed: bool,
    pub flags: Option<i64>,          // отсутствует 75/1328
    pub message_notifications: i64,
    pub mute_config: Option<MuteConfig>,
    pub muted: bool,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct MuteConfig {
    pub end_time: Option<String>,
    pub selected_time_window: Option<i64>, // -1, 86400
}

// ready/snapshot.rs
pub const TOP_LEVEL_KEYS: [&str; 40] = [/* 40 ключей из §2.2, сортировка по имени */];

#[derive(Clone, PartialEq)]
pub struct Ready {
    pub v: Option<i64>,
    pub user: User,
    pub session_id: String,
    pub session_type: Option<String>,
    pub resume_gateway_url: Option<String>,
    pub guilds: Vec<Guild>,
    pub private_channels: Vec<PrivateChannel>,
    pub users: Vec<User>,
    pub relationships: Vec<Relationship>,
    pub read_state: ReadState,
    pub user_guild_settings: UserGuildSettings,
    pub sessions: Vec<Session>,
    pub geo_ordered_rtc_regions: Vec<String>,
    pub country_code: Option<String>,
    pub subdivision_code: Option<String>,
    pub friend_suggestion_count: Option<i64>,
    pub api_code_version: Option<i64>,
    pub explicit_content_scan_version: Option<i64>,
    pub ad_personalization_toggles_disabled: bool,
    pub user_settings_proto: Option<String>,
    pub top_level_keys: usize,
    pub diagnostics: ReadyDiagnostics,
    raw: EtfValue,                    // весь d; Debug не печатает
}

impl Ready {
    pub fn from_dispatch(value: &EtfValue) -> Result<Self, ReadyError>;
    pub fn parse(d: &EtfValue) -> Result<Self, ReadyError>;
    pub fn raw(&self) -> &EtfValue;   // содержит секреты: не логировать
}
```

`Ready`/`Session` реализуют `Debug` вручную: `session_id`, `user_settings_proto` и `raw` не
печатаются. Поля `email`/`phone` в `User` вообще не типизируются (остаются в `raw`).

### 3.4 Замороженные сигнатуры парсеров

```rust
// ready/guild.rs
pub(crate) struct ParsedGuilds {
    pub(crate) guilds: Vec<Guild>,
    pub(crate) raw_positions: Vec<Option<usize>>, // сырой guilds[i] → итоговый индекс
}
pub(crate) fn parse_guilds(value: &EtfValue, diags: &mut ReadyDiagnostics) -> ParsedGuilds;

// ready/social.rs
pub(crate) fn parse_user(value: &EtfValue, path: &str, diags: &mut ReadyDiagnostics) -> Option<User>;
pub(crate) fn parse_users(value: Option<&EtfValue>, diags: &mut ReadyDiagnostics) -> Vec<User>;
pub(crate) fn parse_private_channels(value: Option<&EtfValue>, diags: &mut ReadyDiagnostics) -> Vec<PrivateChannel>;
pub(crate) fn parse_relationships(value: Option<&EtfValue>, diags: &mut ReadyDiagnostics) -> Vec<Relationship>;
pub(crate) fn parse_sessions(value: Option<&EtfValue>, diags: &mut ReadyDiagnostics) -> Vec<Session>;
pub(crate) fn parse_merged_members(value: Option<&EtfValue>, diags: &mut ReadyDiagnostics) -> Vec<Vec<GuildMember>>;

// ready/read_state.rs
pub(crate) fn parse_read_state(value: Option<&EtfValue>, diags: &mut ReadyDiagnostics) -> ReadState;
pub(crate) fn parse_user_guild_settings(value: Option<&EtfValue>, diags: &mut ReadyDiagnostics) -> UserGuildSettings;

// ready/value.rs (координатор)
pub(crate) fn get<'a>(value: &'a EtfValue, key: &str) -> Option<&'a EtfValue>;
pub(crate) fn kind(value: &EtfValue) -> &'static str;
pub(crate) fn is_null(value: &EtfValue) -> bool;
pub(crate) fn text<'a>(value: &'a EtfValue) -> Option<&'a str>;
pub(crate) fn opt_text(value: Option<&EtfValue>) -> Option<String>;
pub(crate) fn integer(value: &EtfValue) -> Option<i64>;
pub(crate) fn unsigned(value: &EtfValue) -> Option<u64>;   // ID/права/версии
pub(crate) fn boolean(value: &EtfValue) -> Option<bool>;
pub(crate) fn items<'a>(value: &'a EtfValue) -> Option<&'a [EtfValue]>;
pub(crate) fn items_or_empty<'a>(value: Option<&'a EtfValue>) -> &'a [EtfValue];
pub(crate) fn object<'a>(value: &'a EtfValue) -> Option<&'a [(EtfValue, EtfValue)]>;
pub(crate) fn text_list(value: Option<&EtfValue>) -> Vec<String>;
pub(crate) fn dedup_last_by<T, K: Eq + std::hash::Hash>(
    items: Vec<T>, key: impl Fn(&T) -> K, label: impl Fn(&T) -> String,
    collection: &str, diags: &mut ReadyDiagnostics,
) -> Vec<T>;
```

### 3.5 Алгоритм `Ready::parse`

1. `Ready::parse`: `d.as_map()`; иначе `WrongType { path:"d" }`. `Ready::from_dispatch`: `d`
   отсутствует → `Missing { path:"d" }`; `d` не map → `NotReady`. `top_level_keys = pairs.len()`.
2. `raw = d.clone()` (до любых преобразований; секреты не логируются).
3. `guilds`: ключ обязателен, значение — `List`/`Tuple`; иначе `Missing`/`WrongType`.
   `parse_guilds`.
4. `user`: ключ обязателен, map; `parse_user`; нет/битый `id` → `InvalidId`.
5. `session_id`: ключ обязателен, текст; иначе `Missing`/`WrongType`.
6. Опциональные коллекции: `users`, `private_channels`, `relationships`, `sessions`,
   `merged_members` — отсутствие/`nil` → пусто; неверный тип → пусто + `diagnostics.ignored`.
7. `read_state`, `user_guild_settings` — отсутствие → `Default`; неверный тип → `Default` +
   `ignored`.
8. Скаляры из §2.2 (`v`, `session_type`, `resume_gateway_url`, `geo_ordered_rtc_regions`,
   `country_code`, `subdivision_code`, `friend_suggestion_count`, `api_code_version`,
   `explicit_content_scan_version`, `ad_personalization_toggles_disabled`,
   `user_settings_proto`) — опционально, дефолты.
9. `merged_members[i]` → гильдия по **сырому** индексу `i` (карта `raw_positions`); несовпадение
   длин → `malformed`; группа пропущенной/дедуплицированной гильдии не приклеивается к соседней;
   для дубликата гильдии побеждает группа последнего сырого вхождения, предыдущие — `malformed`;
   лишние группы игнорируются с `malformed`.
10. Неизвестные top-level ключи (не в `TOP_LEVEL_KEYS`) → `diagnostics.unknown_top_level`.
11. Вернуть `Ready { ..., raw }`.

**Порядок dedup:** для каждой коллекции — стабильный first-position/last-value; диагностика
пишется в `duplicates`. Это делает повторный READY/мерж идемпотентным; P2-02 может не заботиться
о дублях внутри одного снапшота.

### 3.6 Безопасность

- `Debug` для `Ready`/`Session` не печатает `session_id`, `user_settings_proto`, `raw`.
- `User` не содержит `email`/`phone`; они остаются только в `raw` (как и top-level токены).
- `raw()` документирован как «содержит секреты, не логировать»; в тестах — только явные проверки.
- Тест A13: синтетические `SECRET_*` не появляются в `format!("{ready:?}")`.
- Новые поля не логируются целиком: только имена в `unknown_top_level`.

---

## 4. Границы: P2-01 / P2-02 / P2-06 (и соседние)

| Тема | P2-01 | P2-02 | P2-06 | Другие |
|---|---|---|---|---|
| Разбор `READY.d` в типы | **да** | потребляет | — | — |
| Редьюсер `(state,event)→(state,effects)` | — | **да** | — | — |
| Слияние READY/RESUMED/READY_SUPPLEMENTAL, индексы, join users↔members | — | **да** | — | — |
| Дубли/порядок **внутри** одного READY | **да** (last wins) | повторно не нужно | — | — |
| `merged_members` как seed участников | **да** (7×1) | хранит | — | P2-04 — полные списки/ops |
| `READY_SUPPLEMENTAL.merged_members` (138), `merged_presences` (2), `voice_states` | — | частично | — | P2-04 / P2-09 / P2-10 |
| `user_settings_proto` | **да**, как opaque base64 | хранит | — | P2-08 декодирует protobuf |
| `read_state` initial | **да** | хранит | — | P2-07 — ACK/обновления |
| `presences`, `voice_states` в READY | **нет** (их нет в capture) | — | — | P2-09 / P2-10 |
| История сообщений, `GET /messages`, пагинация, кэш | — | — | **да** | — |
| `Channel.last_message_id` / `read_state.last_message_id` как курсоры | **да** (поле) | хранит | **да** (использует) | — |
| SQLite/миграции | — | — | — | P2-03 |
| UI-дерево каналов, сортировка, папки | — | — | — | P3 |

**P2-06 не получает из READY ни одного нового типа.** В READY нет массивов сообщений; есть только
id/курсоры. Всё, что делает P2-06 (лимиты, бесшовный скролл, кэш), — REST/БД, не READY.

**P2-01 не делает:** HashMaps-индексы, join пользователей, эффекты, запись в SQLite, REST,
UI, обработку op37/GUILD_MEMBER_LIST_UPDATE, декод settings-proto, presence/voice.

---

## 5. Тест-кейсы (red-first)

Все кейсы — сначала тест, прогон с `# EXIT=...`, затем минимальная реализация. Для capture-кейсов
оракул — `jq` по `002-READY.json`; для синтетики — контракт §2.3 (помечено «синтетика»).
Полный набор ожидаемых чисел:

```sh
# A2/A3
jq -c '[.d.guilds[]|select(.id=="1014537851418980372")|{name:.properties.name,
 channels:(.channels|length),roles:(.roles|length),emojis:(.emojis|length),
 stickers:(.stickers|length),member_count,premium_subscription_count,joined_at,version}]' \
 fixtures/gateway/by-type/002-READY.json
# B1/B4/B5
jq '[.d.users[]|select(has("bot")|not)]|length' fixtures/gateway/by-type/002-READY.json   # 41
jq '[.d.relationships[].type]|group_by(.)|map({type:.[0],n:length})' ...                  # 1:18, 2:42
# C1/C4
jq '.d.read_state|[.entries|length,.version,.partial]' ...                                # [157,128949,"false"]
jq '[.d.user_guild_settings.entries[].channel_overrides//[]|length]|add' ...              # 1328
```

### 5.1 W12a — каркас, гильдии/каналы/роли/эмодзи/стикеры

| ID | Кейс | Ожидание | Оракул |
|---|---|---|---|
| A1 | Минимальный READY (синтетика): `{user:{id:"1"},guilds:[],session_id:"s"}` | `Ok`; `top_level_keys=3`; `v=None`; все коллекции пусты; `diagnostics` пуст | контракт §2.3 |
| A2 | Полная фикстура | 40 ключей; 7 гильдий; 272 канала; 132 роли; 363 эмодзи; 51 стикер; `diagnostics` пуст; `user.id=384299970632876032`; `user_settings_proto.len()=4400` | jq-блок выше, `ready-stats` |
| A3 | Крупная гильдия `1014537851418980372` | 83 канала (0:47, 2:20, 4:12, 15:4), 34 роли, 46 эмодзи, 6 стикеров, `member_count=24228`, `premium=5`, `joined_at=2023-12-25T17:19:01.733000+00:00`, `version=1792391252875`, `members.len()=1` | `jq` A2/A3; VERIFICATION §2.1 |
| A4 | Guild unavailable (синтетика): `{id:"42",unavailable:"true"}` | `Ok`; `unavailable=true`; `name=None`; все коллекции пусты | контракт; не подтверждено capture |
| A5 | Новые поля: top-level `new_top`, guild `new_guild`, channel `new_channel` | `Ok`; `diagnostics.unknown_top_level=["new_top"]`; `raw().get("new_top")` сохранён; известные числа не изменились | синтетика |
| A6 | Канал только `{id,type}`; гильдия без `emojis/stickers/threads` | `Ok`; `name=None`, `position=0`, `permission_overwrites=[]`; `emojis/stickers=[]`; без диагностики | capture-поля + контракт |
| A7 | Битые якоря: `guilds:"x"`, `user:5`, `session_id:5`, нет `user` | `Err WrongType` (`d.guilds`/`d.user`/`d.session_id`), `Err Missing` для `user` | контракт §2.3 |
| A8 | Битый элемент: `guild.id="abc"`; `channel.position="x"`; `users:5` | гильдия/канал пропущены → `malformed`; `position=0` + `ignored`; `users=[]` + `ignored` | синтетика |
| A9 | Дубликаты: 2 гильдии id=1, 2 канала id=1 | остаётся 1; значение последнее; позиция первого; `duplicates` содержит `guilds:1`, `...channels:1` | синтетика |
| A10 | Пустые списки/`nil`: `channels:[]`, `roles:"nil"`, `emojis:nil`, `stickers:[]` | все `Vec` пусты; без ошибки | синтетика + capture (`null` 2/7) |
| A11 | Регрессия big-int: `guild.id=-1467272931325252608` (Int/Str) | элемент пропущен; `user.id` отрицательный → `Err InvalidId` | `gateway-protocol.md:105-117` |
| A12 | Детали канала/роли/эмодзи/стикера | канал `1275727867497680918`: parent `1167440450416824430`, pos 29, 2 overwrite (`allow=412317240385`, `deny=16`); роль `1057598992449552425`: color 16711684, pos 30, perms 41780636024647, hoist true; эмодзи `590239122757648416`: roles=[]; стикер `875058193758289990`: tags `dizzy_face`, format 1, type 2 | `jq` по id |
| A13 | Секреты в Debug (синтетика) | `format!("{ready:?}")` не содержит `SECRET_SESSION`, `SECRET_TOKEN`, `SECRET_QOS` | контракт §3.6 |
| A14 | `from_dispatch`: op10 / `t:"OTHER"` / нет `d` | `NotReady` / `NotReady` / `Missing d` | контракт §2.1 |

### 5.2 W12b — users, ЛС, relationships, sessions, merged members

| ID | Кейс | Ожидание | Оракул |
|---|---|---|---|
| B1 | `users=146`; partial-поля | 41 без `bot` → `bot=false`; 44 без `display_name` → `None`; 86 без `clan` (raw) | `jq select(has("bot")|not)` и т.д. |
| B2 | Top-level `user` | 32 ключа в `raw().user`; `id=384299970632876032`, `username=pharos12510`, `global_name=ShadowByte`, `discriminator="0"`, `avatar=e234...`, `bot=false`; `email` не типизирован | `jq '.d.user'` |
| B3 | `private_channels` | 17; все `type=1`; `last_pin_timestamp` 2/17; `last_message_id="nil"` 1/17 → `None`; `recipient_ids` непусты | `jq` |
| B4 | `relationships` | 60; type 1:18, 2:42; первый id/user_id `194374444821643266`; `nickname=None`; `since=2025-06-21T06:48:57.474000+00:00`; bool false | `jq` |
| B5 | `sessions` | 1; `client_info={desktop,linux,0}`; `status=invisible`; `session_id=REDACTED`; `processed_at_timestamp=0` | `jq '.d.sessions[0]'` |
| B6 | `merged_members` | 7 групп по 1; `guilds[5].members[0]`: user_id `384299970632876032`, nick `ShadowByte`, flags 2, roles 8, joined_at `2023-12-25...` | `jq '.d.merged_members[5][0]'` |
| B7 | Несовпадение длин (синтетика): 3 гильдии, 2 группы | первые 2 прикреплены, третья пуста; `malformed` непуст; без паники | контракт §2.3 |
| B8 | Дубликат `user_id` в группе/`users` | last wins; `duplicates`; число элементов уменьшилось | синтетика |
| B9 | Partial user `{id:"1"}` в `users[]` | `Ok`; `username=None`, `bot=false`, `public_flags=None` | синтетика |
| B10 | Битый user: `users[0].id="abc"`; top-level `user.id="abc"` | элемент пропущен + `malformed`; top-level → `Err InvalidId` | синтетика |
| B11 | `users`/`private_channels` отсутствуют | `Ok`; пустые `Vec`; `diagnostics` пуст | контракт (опциональные) |

### 5.3 W12c — read_state и user_guild_settings

| ID | Кейс | Ожидание | Оракул |
|---|---|---|---|
| C1 | `read_state` | 157 записей; `partial=false`; `version=128949`; без `flags` 2; без `last_viewed` 14; `badge_count` 2; `last_acked_id` 2; `read_state_type` 2 | `jq` |
| C2 | Записи без optional | записи `384299970632876032` и `1014537851418980372` (без flags/last_message/last_pin/mention) → `None`, без ошибки | `jq` |
| C3 | Дубликат id в read_state | last wins; `duplicates`; размер 1 | синтетика |
| C4 | `user_guild_settings` | 58 записей; `partial=false`; `version=734`; overrides 1328; `guild_id="nil"` 1; `channel_overrides=null` 6; `mute_config` obj 21 | `jq` |
| C5 | ChannelOverride optional | 75/1328 без `flags` → `None`; `mute_config` obj 38; `selected_time_window=-1/86400`, `end_time` строка/`None` | `jq` |
| C6 | Дубликат UGS `guild_id` (включая `None`) | last wins; `duplicates` | синтетика |
| C7 | Битые типы: `read_state.entries=5`, `version="x"`, UGS=5 | пусто/0 + `ignored`; без Err | синтетика |
| C8 | `partial`: `"true"`/`1` → true; `"false"`/`0` → false; `"yes"`/`2` → false + `ignored` | как указано | синтетика |
| C9 | Нет `read_state`/UGS или `entries:[]` | `Default`: 0 записей, version 0, partial false | синтетика |
| C10 | Битый `id` записи read_state | запись пропущена + `malformed` | синтетика |
| C11 | id записи — пользователь/гильдия | `id=384299970632876032` и `1014537851418980372` принимаются без проверки по каналам | `jq` |

---

## 6. Матрица файлов W12a / W12b / W12c

Пересечений нет; общие файлы — только у координатора. `W12a` импортирует типы `GuildMember`
из `social.rs` и вызывает парсеры `social/read_state`; обратных зависимостей нет.

| Роль | Пишет (только эти файлы) | Читает (заморожено, не менять) |
|---|---|---|
| **W12a** (P2-01a) | `src/ready/snapshot.rs`, `src/ready/guild.rs`, `tests/ready_core.rs` | `ready/value.rs`, `ready/error.rs`, `tests/support/ready_fixture.rs`, типы `social.rs`/`read_state.rs` |
| **W12b** (P2-01b) | `src/ready/social.rs`, `tests/ready_social.rs` | `value.rs`, `error.rs`, `ready_fixture.rs`, `snapshot.rs` (только вызов `Ready::parse`) |
| **W12c** (P2-01c) | `src/ready/read_state.rs`, `tests/ready_read_state.rs` | `value.rs`, `error.rs`, `ready_fixture.rs`, `snapshot.rs` |
| **Координатор** | `src/ready.rs`, `src/ready/error.rs`, `src/ready/value.rs`, `src/lib.rs`, `tests/support/ready_fixture.rs`, стартовые заглушки всех трёх модулей | — |

Запрещено всем: `Cargo.toml`, `Cargo.lock`, `fixtures/**`, `docs/**`, `TASKS.md`,
`waves/STATE.md`, `waves/db.jsonl`, `logs/`, `functions/`, `evidence/`, чужие тесты/модули.

**Стартовые заглушки координатора обязаны компоновать все три парсера**, иначе W12b/W12c не
смогут позеленеть независимо:

```rust
// snapshot.rs (заглушка координатора; W12a наполняет, вызовы сохраняет)
pub fn parse(d: &EtfValue) -> Result<Self, ReadyError> {
    let map = d.as_map().unwrap_or(&[]);
    let guilds = guild::parse_guilds(d.get("guilds").unwrap_or(&EtfValue::Nil), &mut diags);
    let user = social::parse_user(d.get("user").unwrap_or(&EtfValue::Nil), "d.user", &mut diags)
        .unwrap_or_default();
    let users = social::parse_users(d.get("users"), &mut diags);
    let private_channels = social::parse_private_channels(d.get("private_channels"), &mut diags);
    let relationships = social::parse_relationships(d.get("relationships"), &mut diags);
    let sessions = social::parse_sessions(d.get("sessions"), &mut diags);
    let merged = social::parse_merged_members(d.get("merged_members"), &mut diags);
    let read_state = read_state::parse_read_state(d.get("read_state"), &mut diags);
    let ugs = read_state::parse_user_guild_settings(d.get("user_guild_settings"), &mut diags);
    // ... скаляры и attach merged; вернуть Ready { ..., raw: d.clone() }
}
```

Заглушки `parse_guilds`/`parse_users`/`parse_read_state` возвращают пустые значения; каждый воркер
заменяет только свою. Это даёт red у всех трёх одновременно и green независимо.

---

## 7. Риски и открытые вопросы

| # | Вопрос | Рекомендация | Блокирует старт? |
|---:|---|---|---|
| 1 | ID: `u64` vs `String` vs `i64` | `u64`; отрицательные/подписанные — невалидны (регрессия big-int). `String` — только в `raw`/тестах | Да, зафиксировать |
| 2 | Политика «битый элемент»: skip или Err | Skip + `diagnostics.malformed`; Err только для якорей (`guilds`, `user.id`, `session_id`) | Да |
| 3 | Дубликаты: last wins или Err | Last wins + `diagnostics.duplicates`; идемпотентно для P2-02 | Да |
| 4 | `merged_members`: прикреплять к `Guild` или хранить отдельно | Прикреплять `guilds[i].members`; длины выравнивать, расхождение — `malformed` | Да |
| 5 | `Guild.name`: top-level vs `properties.name` | Сначала top-level, затем `properties.name`; в capture только properties | Да |
| 6 | `raw`: хранить весь `d` (577 КБ, секреты) или только extras | Хранить весь `d`, `Debug` redacted, `raw()` с предупреждением; P2-02 может освободить | Желательно |
| 7 | `guild.unavailable` не подтверждён | Поле `bool` default false; тест синтетический; не считать фактом capture | Нет |
| 8 | `users[].bot` отсутствует 41/146 | `bool=false`; задокументировать «не бот/неизвестно» | Нет |
| 9 | `serde` для тестов над JSON | Не добавлять; тестовый JSON→`EtfValue` адаптер | Да (если координатор не разрешит dev-dep) |
| 10 | `member_count` 24 228 vs 24 229 | Тест — 24 228 (фикстура); в комментарии — VERIFICATION §2.1 | Да |
| 11 | `ready-stats` не содержит users/sessions | Оракул users/sessions — `002-READY.json`/protocol; T-DATA-02/03 уточнить | Нет |
| 12 | `read_state.entries[].id` — не только канал | P2-01 не валидирует по каналам; P2-07 интерпретирует | Да (для P2-07) |
| 13 | `user_settings_proto` — base64 или raw | В capture — base64-строка 4400; P2-08 делает base64-decode → protobuf | Нет |
| 14 | Нужен ли `Ready` в `GatewayPipe` | Нет; P2-01 — чистая функция, интеграция в P2-02. `ws.rs` не трогаем | Нет |
| 15 | `Debug` существующего `ReadySummary` печатает `session_id` | Вне P2-01; отдельный нит/задача координатора | Нет |

---

## 8. Предлагаемое ТЗ RW-12

**Метод:** PROCESS.md §3.13 — три независимых куска одновременно; контракты заморожены
заглушками; общие файлы правит только координатор.

### 8.0 Общие правила

- Только свои файлы из §6. Чужие тесты/модули/доки/golden не трогать.
- Red-first: тест → `cargo test --offline -p discord-core --test <test>` → `# EXIT=...` → копия
  теста в `waves/RW-12/reports/<id>-test.red.rs` → минимальная реализация → green.
- Новых зависимостей и сети нет. `Cargo.toml`/`Cargo.lock` не менять.
- Публичные типы/поля/сигнатуры из §3.3 не менять; приватные сигнатуры §3.4 синхронизированы
  с ревью P2-01 (`ParsedGuilds`, `parse_user` + `path`/`diags`); при нехватке помощника —
  приватный помощник в своём файле или вопрос координатору.
- `npm run gate:worker` запускает координатор один раз после слияния (как в RW-11), чтобы три
  параллельных прогона не конкурировали за `.gate`. Воркер сдаёт `cargo test/clippy/fmt` по своим
  целям.
- Отчёт §4.2 → `waves/RW-12/reports/<id>.md`; red/green-логи туда же.

### 8.1 W12a — P2-01a: каркас READY + guild/channel/role/emoji/sticker

**Файлы:** `src/ready/snapshot.rs`, `src/ready/guild.rs`, `tests/ready_core.rs`.
**Кейсы:** A1–A14 (§5.1). **Оракулы:** `002-READY.json` (jq), `gateway-protocol.md:163-213`,
`gateway-schema.md:295-374`, `VERIFICATION.md:14-15,34-38`.
**DoD:** `TOP_LEVEL_KEYS.len()==40`; полная фикстура даёт числа A2; крупная гильдия A3;
`Ready`/`Session` Debug redacted; `from_dispatch`/`parse` ошибки A7/A14; `cargo test`, `clippy
-D warnings`, `fmt --check`; отчёт.

### 8.2 W12b — P2-01b: users/ЛС/relationships/sessions/merged members

**Файлы:** `src/ready/social.rs`, `tests/ready_social.rs`.
**Кейсы:** B1–B11 (§5.2). **Оракулы:** `002-READY.json` (jq), `gateway-schema.md:171-239,284-293`,
`gateway-protocol.md:163-187`.
**DoD:** 146 users, 17 ЛС, 60 связей, 1 сессия, 7×1 merged; partial users; дедуп; прикрепление
`members` не ломает `Guild` W12a; тесты/clippy/fmt; отчёт.

### 8.3 W12c — P2-01c: read_state + user_guild_settings

**Файлы:** `src/ready/read_state.rs`, `tests/ready_read_state.rs`.
**Кейсы:** C1–C11 (§5.3). **Оракулы:** `002-READY.json` (jq), `gateway-schema.md:241-282`.
**DoD:** 157 записей/128949/false; 58 UGS/734/false/1328 overrides; optional-поля; дедуп;
id пользователя/гильдии принимаются; тесты/clippy/fmt; отчёт.

### 8.4 Приёмка

- Все три набора зелёные; `cargo test --offline` по workspace; `cargo clippy --offline
  --workspace --all-targets -- -D warnings`; `cargo fmt --check`.
- `npm run gate:worker` — зелёный (координатор).
- Проверка red-first: тест падал до реализации; копии red-логов в `waves/RW-12/reports/`.
- Проверка границ: в диффе нет `ws.rs`, `rest/*`, `Cargo.*`, `fixtures/**`, `docs/**`,
  `TASKS.md`, `waves/STATE.md`, `db.jsonl`.
- Секреты: `grep -R "SECRET_\|analytics_token.*="` по тестам/логам не находит реальных значений;
  `Debug`-тест A13 зелёный.

---

### Приложение: что именно считается «40 ключами»

`fixtures/gateway/ready-stats.json.topKeys` — 40 имён, включая `_trace`.
`docs/re/gateway-schema.json` даёт 39 в `fields` + `hasTrace=1`; это тот же набор.
`docs/re/gateway-schema.md:123` явно: «40 (39 значимых + `_trace`)».
Поэтому `TOP_LEVEL_KEYS` в коде — **40**, включая `_trace`; `presences` в него не входит (его нет).
