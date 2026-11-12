# RW-21 / RESEARCH-P2-08 — settings-proto

Дата: 2026-10-21. Карточка: P2-08. Зависимость: P2-02 (принята).
Режим: read-only по `reference-oracle/`, продукту и тестам; cargo не запускался.

## 1. Источники

| Источник | Что даёт | Провенанс |
| --- | --- | --- |
| `gateway/by-type/002-READY.json` | `d.user_settings_proto` — base64, 4400 символов | `reference-oracle/`, READY run2, read-only |
| сырые байты READY | 3300 B, `sha256=c79fca65e321cae5135401f22ee9041809958a34e52e959d0a1a63a04f7c5672` | `base64 -d`, Python oracle |
| `gw-proto/msgs-run2-95974_13072.jsonl` | `USER_SETTINGS_PROTO_UPDATE`: `settings.type=2`, `partial="false"`, base64 17 436 | `reference-oracle/`, read-only |
| сырые байты UPDATE | 13 077 B, `sha256=8cd02970195054bbf5bfc481486264df9027213101432548f4a1b26a8d1531e5` | `base64 -d`, Python oracle |
| `flatpak-hits/fd741e5db8758fbe_0` | минифицированный JS-бандл Discord; модуль `761821` — `Y5/Gd`, модуль `873298` — `PreloadedUserSettings`, модуль `803805` — `FrecencyUserSettings` | `reference-oracle/`, read-only |
| `docs/re/rest-d02.md` §4 | PATCH `settings-proto/2` body обрезан на 4001 символе; полного JSON/protobuf-оракула нет | уже в репозитории |

**Ограничение:** версия клиентского бандла не подтверждена отдельным манифестом. Имена полей
берутся только из сгенерированных protobuf-классов этого бандла; неизвестные поля сохраняются
структурно, без придуманных имён.

## 2. Как связаны READY и USER_SETTINGS_PROTO_UPDATE

В модуле `761821`:

```js
let E={
  [c.oD.PRELOADED_USER_SETTINGS]: o.nT,
  [c.oD.FRECENCY_AND_FAVORITES_SETTINGS]: l.aw
};
```

В модуле `355097` перечисление:

```text
PRELOADED_USER_SETTINGS=1
FRECENCY_AND_FAVORITES_SETTINGS=2
TEST_SETTINGS=3
```

Следовательно:

- READY `user_settings_proto` — `PreloadedUserSettings` (тип 1), 3300 B;
- `USER_SETTINGS_PROTO_UPDATE` в трёх захватах имеет `settings.type=2`, поэтому это
  `FrecencyUserSettings` (тип 2), 13 077 B;
- тип 2 **не** является Preloaded-настройками; это отдельная схема favorite/frecency.

## 3. Схема `PreloadedUserSettings`

Источник: `class er ... super("discord_protos.discord_users.v1.PreloadedUserSettings", [...])`
в модуле `873298`.

| № | Поле | Kind | Комментарий |
| ---: | --- | --- | --- |
| 1 | `versions` | message | `Versions` |
| 2 | `inbox` | message | `InboxSettings` |
| 3 | `guilds` | message | map/repeated `GuildSettings`; в этом карточном scope не типизируется |
| 4 | `user_content` | message | `UserContentSettings` |
| 5 | `voice_and_video` | message | `VoiceAndVideoSettings` |
| 6 | `text_and_images` | message | `TextAndImagesSettings` |
| 7 | `notifications` | message | `NotificationSettings` |
| 8 | `privacy` | message | `PrivacySettings` |
| 9 | `debug` | message | `DebugSettings` |
| 10 | `game_library` | message | `GameLibrarySettings` |
| 11 | `status` | message | `StatusSettings` |
| 12 | `localization` | message | `LocalizationSettings` |
| 13 | `appearance` | message | `AppearanceSettings` |
| 14 | `guild_folders` | message | `GuildFolders` |
| 15 | `favorites` | message | `Favorites` |
| 16 | `audio_context_settings` | message | `AudioContextSetting` |
| 17 | `communities` | message | `CommunitiesSettings` |
| 18 | `broadcast` | message | `BroadcastSettings` |
| 19 | `clips` | message | `ClipsSettings` |
| 20 | `for_later` | message | `ForLaterSettings` |
| 21 | `safety_settings` | message | `SafetySettings` |
| 22 | `icymi_settings` | message | `IcymiSettings` |
| 23 | `applications` | message | `ApplicationsSettings` |
| 24 | `ads` | message | `AdsSettings` |
| 25 | `in_app_feedback_settings` | message | `InAppFeedbackSettings` |
| 26 | `app_version_settings` | message | `AppVersionSettings` |

В READY присутствуют top-level поля:
`1,2,3,4,5,6,7,8,9,10,11,12,13,14,16,19,23,25` (18 полей).
Поле 1 `versions`: `client_version=21`, `data_version=3027`, `server_version` отсутствует.
Поле 2 `inbox`: `current_tab=2`.

### 3.1 Типизированные секции (подтверждённые поля)

`Versions` (`1/2/3` — `client_version/server_version/data_version`, scalar).

`AppearanceSettings`:

| № | Поле | Kind | READY |
| ---: | --- | --- | --- |
| 1 | `theme` | enum `Theme` | `3` = `DARKER` |
| 2 | `developer_mode` | scalar | `1` |
| 4 | `mobile_redesign_disabled` | scalar | нет |
| 9 | `timestamp_hour_cycle` | enum `TimestampHourCycle` | `2` = `H23` |
| 11 | `launch_pad_mode` | enum | нет |
| 12 | `ui_density` | enum `UIDensity` | `1` = `COMPACT` |
| 13 | `swipe_right_to_left_mode` | enum | нет |
| 14 | `default_guild_theme_preference` | enum | нет |
| 15 | `dark_sidebar` | scalar | нет |

`Theme`: `UNSET=0, DARK=1, LIGHT=2, DARKER=3, MIDNIGHT=4`.
`UIDensity`: `UNSET_UI_DENSITY=0, COMPACT=1, COZY=2, RESPONSIVE=3, DEFAULT=4`.
`TimestampHourCycle`: `AUTO=0, H12=1, H23=2`.

`VoiceAndVideoSettings` (в READY присутствуют 5,6,7,8,9,10):

| № | Поле | Kind | READY |
| ---: | --- | --- | --- |
| 5 | `always_preview_video` | BoolValue | true |
| 6 | `afk_timeout` | Int32Value | 600 |
| 7 | `stream_notifications_enabled` | BoolValue | true |
| 8 | `native_phone_integration_enabled` | BoolValue | true |
| 9 | `soundboard_settings` | message | сырые байты, не типизируется |
| 10 | `disable_stream_previews` | BoolValue | false (пустой wrapper) |
| 11 | `soundmoji_volume` | FloatValue | нет |

`TextAndImagesSettings` (READY): поля 7,9,10,11,12,13,14,15,16,17,19,20,21,22,23,27.
Ключевые значения: `view_image_descriptions=true`, `inline_attachment_media=true`,
`inline_embed_media=true`, `gif_auto_play=false`, `render_embeds=true`, `render_reactions=true`,
`animate_emoji=false`, `animate_stickers=2`, `enable_tts_command=false`,
`message_display_compact=false`, `explicit_content_filter=2`, `view_nsfw_guilds=false`,
`convert_emoticons=true`, `expression_suggestions_enabled=true`, `view_nsfw_commands=false`,
`dm_spam_filter_v2=3` (`FRIENDS_AND_NON_FRIENDS`).

`NotificationSettings` (READY): `notify_friends_on_go_live=false`,
`notification_center_acked_before_id=1041362756533751819`.

`StatusSettings` (READY): `status="invisible"`, `show_current_game=false`,
`status_created_at_ms` присутствует; `custom_status` отсутствует.
`status`/`show_current_game` — wrapper-сообщения, `status_created_at_ms` — `UInt64Value`.

`LocalizationSettings` (READY): `locale="ru"`, `timezone_offset=-300`, `timezone_name` отсутствует.

## 4. Схема `FrecencyUserSettings`

Источник: модуль `803805`, `FrecencyUserSettings`:

| № | Поле | Kind |
| ---: | --- | --- |
| 1 | `versions` | message |
| 2 | `favorite_gifs` | message `FavoriteGIFs` |
| 3 | `favorite_stickers` | message `FavoriteStickers` |
| 4 | `sticker_frecency` | message `StickerFrecency` |
| 5 | `favorite_emojis` | message `FavoriteEmojis` |
| 6 | `emoji_frecency` | message `EmojiFrecency` |
| 7 | `application_command_frecency` | message |
| 8 | `favorite_soundboard_sounds` | message |
| 9 | `application_frecency` | message |
| 10 | `heard_sound_frecency` | message |
| 11 | `played_sound_frecency` | message |
| 12 | `guild_and_channel_frecency` | message |
| 13 | `emoji_reaction_frecency` | message |

UPDATE-1 top-level поля: `1,2,4,6,7,9,11,12,13`; длины вложений:
887, 108, 2730, 964, 412, 64, 6642, 1241 B.
`versions`: `client_version=10`, `data_version=7547`, `server_version` отсутствует.
Тип 2 не типизируется в глубокие map-модели в P2-08; сохраняется структурно
(`raw` + top-level `ProtoField`), чтобы не выдумывать семантику favorite/frecency.

## 5. Wire-наблюдения

- Все length-delimited поля — protobuf messages или wrapper-сообщения; строки находятся только
  внутри wrapper поля 1 (`StringValue`).
- Wrapper с пустыми байтами — валидное значение `false/0`, а не «поле отсутствует».
- `timezone_offset` — signed `int64`, READY даёт `-300` (UTC+5).
- `notification_center_acked_before_id` и `status_created_at_ms` — `uint64`, значения
  помещаются в JavaScript safe integer, но в Rust хранятся как `u64`.
- В захвате нет wire type 3/4 (groups), нет fixed32 в типизированных секциях.
- PATCH body `settings-proto/2` в `rest-d02.md` обрезан, поэтому не используется как оракул
  полного JSON-представления.

## 6. Независимый оракул

`waves/RW-21/oracle.py` (Python, только stdlib) декодирует base64/varint/wrapper и печатает
факты из §3–§4. Rust-тесты не вызывают Python; они сверяют захардкоженные факты с
фикстурами, полученными из READY/UPDATE. Повторный запуск oracle:

```sh
python3 waves/RW-21/oracle.py
```

Ожидаемые md5/размеры фикстур фиксируются в `TEST-MATRIX-P2-08.md`.
