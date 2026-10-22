# RW-21 / DESIGN-P2-08 — settings-proto

Дата: 2026-10-21. Карточка: **P2-08** (M). Team Lead: TL-1. Tech Lead: TL-2.
Статус: **frozen**. Источник фактов: `waves/RW-21/RESEARCH-P2-08.md`, независимый
`waves/RW-21/oracle.py`. Зависимость: P2-02 `[x]`.

## 1. Scope

P2-08 — **чистый декодер** protobuf/base64 для настроек, без состояния, REST, UI, SQLite,
keyring, live Discord и сетевых вызовов. Задача — превратить непрозрачные base64-поля
`PreloadedUserSettings` / `FrecencyUserSettings` в структурную модель, сохранив неизвестные
поля без потери.

В scope:

- строгий разбор protobuf wire (varint, fixed64, length-delimited, fixed32);
- base64 → bytes через уже принятый `rest::base64::decode` (P1-08), без второй реализации;
- `SettingsProto::parse(proto_type, bytes)` для type 1/2 и неизвестных типов;
- `PreloadedUserSettings` (READY, type 1): `versions`, `inbox`, `voice_and_video`,
  `text_and_images`, `notifications`, `status`, `localization`, `appearance`;
- `FrecencyUserSettings` (type 2): structural `raw` + `versions` (полная map-семантика
  favorite/frecency вне P2-08; неизвестные поля сохраняются);
- неизвестные top-level/секционные поля — в `raw: ProtoMessage`, без выдуманных имён;
- `Debug` не печатает length-delimited payload/строки состояния.

Вне scope: reducer/`State`, `EventPayload`, REST PATCH, запись настроек, полная типизация
`guilds`, `guild_folders`, `audio_context_settings`, privacy/debug/ads и map-структур
frecency, UI-настройки, сеть/live/new dependencies.

## 2. Публичный контракт

```rust
pub const PRELOADED_USER_SETTINGS_TYPE: u64 = 1;
pub const FRECENCY_AND_FAVORITES_SETTINGS_TYPE: u64 = 2;

pub enum SettingsProto {
    Preloaded(Box<PreloadedUserSettings>), // Box — чтобы enum не был 352 B
    Frecency(FrecencyUserSettings),
    Unsupported { proto_type: u64, message: ProtoMessage },
}

impl SettingsProto {
    pub fn parse(proto_type: u64, bytes: &[u8]) -> Result<Self, SettingsProtoError>;
    pub fn proto_type(&self) -> u64;
    pub fn raw(&self) -> &ProtoMessage;
}

pub fn decode_settings(
    proto_type: u64,
    base64_text: &str,
) -> Result<SettingsProto, SettingsProtoError>;

pub fn decode_preloaded_user_settings(
    base64_text: &str,
) -> Result<PreloadedUserSettings, SettingsProtoError>;

pub fn decode_frecency_user_settings(
    base64_text: &str,
) -> Result<FrecencyUserSettings, SettingsProtoError>;
```

`decode_settings` использует `crate::rest::base64::decode`; ошибка base64 оборачивается в
`SettingsProtoError::Base64`. `SettingsProto::parse` — для уже декодированных bytes
(тесты/оракул), без base64.

### 2.1 Wire-модель

```rust
pub struct ProtoMessage { pub fields: Vec<ProtoField> }
pub struct ProtoField { pub number: u32, pub value: ProtoValue }
pub enum ProtoValue {
    Varint(u64),
    Fixed64(u64),
    LengthDelimited(Vec<u8>),
    Fixed32(u32),
}
```

`ProtoMessage::decode(bytes) -> Result<Self, SettingsProtoError>` разбирает один уровень.
Повторения сохраняются в порядке провода; для singular-полей типизированный парсер берёт
**последнее** значение (protobuf last-wins). `ProtoMessage` предоставляет:

- `field(number) -> Option<&ProtoField>` — последнее;
- `fields_of(number) -> impl Iterator<Item = &ProtoField>`;
- `message(number) -> Result<Option<ProtoMessage>, SettingsProtoError>`;
- `string(number) -> Result<Option<String>, SettingsProtoError>`;
- `bool_wrapper(number)`, `int32_wrapper(number)`, `int64_wrapper(number)`,
  `uint64_wrapper(number)` — `Option`, потому что отсутствие поля и пустой wrapper различаются;
- `scalar_i32(number)`, `scalar_u64(number)`, `scalar_fixed64(number)`,
  `scalar_bool(number)` — для proto3 scalar/enum; отсутствие → `None`.

### 2.2 Типизированные секции Preloaded

Все поля — `Option`, чтобы различать отсутствие и значение по умолчанию. Секции — `Option`,
если length-delimited поле отсутствует.

- `Versions`: `client_version`, `server_version`, `data_version: Option<i32>`.
- `InboxSettings`: `current_tab: Option<i32>`, `viewed_tutorial: Option<bool>`.
- `AppearanceSettings`: `theme`, `developer_mode`, `mobile_redesign_disabled`,
  `timestamp_hour_cycle`, `launch_pad_mode`, `ui_density`, `swipe_right_to_left_mode`,
  `default_guild_theme_preference`, `dark_sidebar`.
- `VoiceAndVideoSettings`: `preset_option: Option<i32>`, `always_preview_video`,
  `afk_timeout: Option<i32>`, `stream_notifications_enabled`,
  `native_phone_integration_enabled`, `disable_stream_previews`.
- `TextAndImagesSettings`: `view_image_descriptions`, `inline_attachment_media`,
  `inline_embed_media`, `gif_auto_play`, `render_embeds`, `render_reactions`,
  `animate_emoji`, `animate_stickers: Option<i32>`, `enable_tts_command`,
  `message_display_compact`, `explicit_content_filter: Option<i32>`, `view_nsfw_guilds`,
  `convert_emoticons`, `expression_suggestions_enabled`, `view_nsfw_commands`,
  `dm_spam_filter_v2: Option<i32>`.
- `NotificationSettings`: `show_in_app_notifications`, `notify_friends_on_go_live`,
  `notification_center_acked_before_id: Option<u64>` (fixed64),
  `enable_burst_reaction_notifications`, `quiet_mode`, `reaction_notifications: Option<i32>`,
  `game_activity_notifications: Option<i32>`, `custom_status_push_notifications: Option<i32>`.
- `StatusSettings`: `status: Option<String>`, `show_current_game: Option<bool>`,
  `status_expires_at_ms: Option<u64>`, `status_created_at_ms: Option<u64>`.
  `custom_status` не типизируется в P2-08 (остаётся в `raw` секции).
- `LocalizationSettings`: `locale: Option<String>`, `timezone_offset: Option<i64>`,
  `timezone_name: Option<String>`.
- `PreloadedUserSettings`: `raw: ProtoMessage` + восемь `Option`-секций выше.

`FrecencyUserSettings`: `raw: ProtoMessage`, `versions: Option<Versions>`.

Все структуры — `Clone, PartialEq, Eq`; `Debug`:
- `ProtoMessage` — только число полей;
- `ProtoValue::LengthDelimited` — длина, не bytes;
- `PreloadedUserSettings`/`FrecencyUserSettings` — raw count + presence секций;
- `StatusSettings` — `status` как длина строки, без текста.

## 3. Wire-правила и ошибки

Константы: `MAX_PROTO_BYTES = 1_048_576`, `MAX_FIELDS = 262_144`.

| Условие | Ошибка |
|---|---|
| `bytes.len() > MAX_PROTO_BYTES` | `TooLarge { size, limit }` |
| varint > 10 байт / переполнение u64 | `VarintOverflow { offset }` |
| обрыв varint/fixed/length | `UnexpectedEnd { offset }` |
| tag field number = 0 или > `0x1FFF_FFFF` | `InvalidFieldNumber { number, offset }` |
| wire type 3/4/6/7 | `UnsupportedWireType { field, wire_type, offset }` |
| length > остатка bytes | `LengthOutOfBounds { field, length, remaining }` |
| > `MAX_FIELDS` | `TooManyFields { limit }` |
| base64 | `Base64(Base64Error)` |
| известное поле с чужим wire type | `WrongWireType { field, expected, actual }` |
| StringValue не UTF-8 | `InvalidUtf8 { field }` |

Группы (3/4) не поддерживаются — в захватах их нет; unknown wire type не «угадывается».
`sint`/`zigzag` в подтверждённых полях не используются; `int32/int64` читаются как
two's-complement из varint. `fixed64` — little-endian.

## 4. Файловая матрица (B02/RW-21)

| Роль | Файлы |
|---|---|
| CORE-3 | `rust/crates/discord-core/src/settings/mod.rs`, `settings/wire.rs`, `settings/preloaded.rs`, `settings/frecency.rs` |
| CORE-3 tests | `rust/crates/discord-core/tests/settings_proto.rs`; `tests/data/settings-preloaded-ready-4400.b64`, `tests/data/settings-frecency-update-{8269,8608,13072}.b64` |
| TL-2 | `rust/crates/discord-core/src/lib.rs` — только `pub mod settings;` и re-export, после красного |
| Oracle/docs | `waves/RW-21/{RESEARCH,oracle.py,TEST-MATRIX}-P2-08.md`; этот файл |

Запрещено: `state/*`, `ready/*`, `reducer`, `Cargo.toml`, `package.json`, golden, live, сеть,
чужие тесты/фикстуры. Новых зависимостей нет; `rest::base64` — существующий модуль.

## 5. Интеграция

1. Red: тесты + `pub mod settings;` → compile-red (модуль объявлен, файлов нет).
2. Green: `wire.rs` → `preloaded.rs`/`frecency.rs` → `mod.rs` → re-export.
3. Targeted: `cargo test --offline -p discord-core --test settings_proto` (16 тестов).
4. Full: core/workspace, fmt, clippy; gate `npm run gate:worker`.
5. Независимое ревью; P0/P1/P2 — red-first, P3 — backlog.
6. `accepted` только после review; docs/журнал — один раз после accepted.

## 6. Критерии приёмки

- READY 4400 b64 / 3300 B и три UPDATE type 2 / 13 077 B декодируются без паники;
- все заявленные значения §3 RESEARCH совпадают с oracle;
- неизвестные top-level поля (READY: 3,4,8,9,10,14,16,19,23,25) сохранены в `raw`;
- type 3 → `Unsupported`, не ошибка; invalid base64/wire → точная ошибка;
- `Debug` не содержит `invisible`, `ru`, `1041362756533751819` и payload;
- no state/network/new deps; targeted/full/gate зелёные; ревью P0/P1/P2=0.
