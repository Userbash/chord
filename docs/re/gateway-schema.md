# Discord Gateway Protocol Schemas (ETF + zstd-stream)

Report generated from a fresh decoded dump of the Discord gateway socket.

| Parameter | Value |
|---|---|
| Source | `/home/user/clean-room-oracle/gateway-schema-work/capture-ready.snapshot.jsonl` (live capture, snapshot) |
| Snapshot MD5 | `ff4a8e393b0e322fe393adaf6426142e` |
| Snapshot Size | 2260044 bytes |
| Capture Window | 2026-10-19T06:13:08.626Z … 2026-10-19T06:23:00.693Z (~592 c) |
| Socket | `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream` |
| requestId | `57992.5112` |
| Gateway WS Frames | 124 (opcode 2, binary), compressed 149212 bytes |
| Decompressed | 883526 bytes (ratio 0.1689) |
| Messages | 124 (102 dispatch op 0 + 1 HELLO + 21 ACK) |
| Event Types | 12 (10 dispatch + HELLO + HEARTBEAT_ACK) |
| SEQ | 1…102, no gaps (102 values) |

**Methodology.** The reference is the working decoder `/home/user/clean-room-oracle/gw-decode.mjs`; running it against the snapshot produces the same 124 messages as file `gateway/messages.jsonl` (first 99 messages matched byte-for-byte, `cmp` — identical). The independent parser in `gateway-schema-work/analyze.mjs` collects ETF tags along paths, message boundaries, and sizes; its payload sizes matched the oracle **124/124** (0 discrepancies). The frame <-> message map was verified twice (25 ms and 100 ms pauses per frame) — full match.

## 1. Stream Encoding

### 1.1 Transport and Compression

| Fact | Value |
|---|---|
| WS Frames / Messages | 124 / 124 → **1 frame = 1 message** |
| zstd magic `28 b5 2f fd` | 1 time, frame 1 @ offset 0 |
| Semantics | one continuous zstd-**stream** across all frames: header only in the first frame, subsequent are continuations |
| Compressed -> Decompressed | 149212 → 883526 bytes (0.1689) |
| Decompression Errors | None |
| Largest Frame | #2: 98 884 compressed -> 577 124 decompressed bytes (READY) |
| Smallest Frames | ACK: 43 decompressed bytes (compressed ~16) |

### 1.2 Byte 0x83 (ETF VERSION_MAGIC)

- `0x83` precedes **every** message: **124/124**. This is the standard Erlang External Term Format prefix (`term_to_binary` = version 131 + term), not a frame delimiter.
- In the decompressed stream, byte `0x83` occurs **493** times, 369 of which are within payload (lengths/values). Therefore 0x83 cannot be used as a marker: message boundaries are determined by term structure.
- Stream parsing: `0x83` + map(0x74). Example beginning: `8374000000046400016474000000026400065f7472616365` — this is HELLO.

### 1.3 Actual ETF Tags

Total tags in stream: **10**. Table — in descending frequency order, with example path.

| Tag | Byte | ETF Specification Name | Count | Example Path |
|---|---|---|---|---|
| d | `0x64` | atom(u16 latin1) | 39890 | `-` |
| m | `0x6d` | binary(u32) | 21138 | `-` |
| t | `0x74` | map(u32) | 6536 | `-` |
| a | `0x61` | uint8 | 6495 | `-` |
| n | `0x6e` | small bignum | 6415 | `-` |
| b | `0x62` | int32 | 2533 | `-` |
| j | `0x6a` | NIL | 2228 | `-` |
| l | `0x6c` | list(u32)+NIL | 1431 | `-` |
| h | `0x68` | small tuple(u8) | 279 | `-` |
| k | `0x6b` | string(u16) | 56 | `-` |

**Not observed** (important for implementations — the parser must support them per spec, but they are absent in this dump): `F double`, `c float31`, `i large tuple(u32)`, `o large bignum`, `p str32`, `s atom(u8)`, `v utf8(u16)`, `w utf8(u8)`.

### 1.4 Strings, null and bool: dual encoding

| Logical Value | How Encoded | Count | Appearance after gw-decode.mjs |
|---|---|---|---|
| string (ASCII) | 0x64 atom (u16 latin1) | 39890 | regular string; no bytes ≥ 0x80 in any atom |
| string (non-ASCII/data) | 0x6d binary (u32) | 21138 | UTF-8 string; 647 items with non-ASCII, all valid UTF-8 (0 "broken") |
| raw bytes | 0x6b string (u16) | 56 | latin1-string (example: `auth.authenticator_types` = `"\u0002"`) |
| null | 0x6a NIL | 2228 | true JSON `null` |
| null (atom) | 0x64 atom "nil" | 7779 | string `"nil"` — semantically null! |
| bool | 0x64 atom "true"/"false" | 1376 / 5605 | strings `"true"`/`"false"` — semantically boolean! |
| int | 0x61 uint8 (+ 0x62 int32) | 6495 (+ 2533) | number |
| int64 (snowflakes) | 0x6e small bignum | 6415 | BigInt (in JSON — string) |

Practical implications for data consumers:

1. The same logical field appears in **both** string tags. Verified: `MESSAGE_CREATE.content` — always `0x6d` (even empty string), and `GUILD_MEMBER_LIST_UPDATE.id` = `"everyone"` — also `0x6d`, while keys of the same objects are `0x64`. Reverse example: `user_settings_proto` — `0x6d`, `_trace[0]` — `0x6d`, but `subdivision_code` = `"HE"` — `0x64`. The exact rule for tag selection has not been established (insufficient data) — recorded as-is.
2. Atom values in the dump constitute a closed dictionary of **26** strings: `nil`×7779, `false`×5605, `true`×1376, `online`×312, `idle`×116, `dnd`×92, `desktop`×81, `GUILD_MEMBER_LIST_UPDATE`×79, `UPDATE`×57, `DELETE`×21, `INSERT`×21, `MESSAGE_CREATE`×10, `full`×7, `PRESENCE_UPDATE`×6, `linux`×2, `invisible`×2, `normal`×1, `READY`×1, `pomelo`×1, `READY_SUPPLEMENTAL`×1, `THREAD_LIST_SYNC`×1, `SYNC`×1, `GUILD_MEMBERS_CHUNK`×1, `SESSIONS_REPLACE`×1, `CHANNEL_INFO`×1, `MESSAGE_UPDATE`×1. Everything else is binary.
3. The `nil` atom and NIL tag are equivalent; strings `"true"`/`"false"` are booleans. Any typed wrapper must normalize this itself.
4. Individual complete messages: `HEARTBEAT_ACK` = map of 4 keys, where `d` is atom `nil`.

### 1.5 Lists and Tuples

- `0x6c` — list with mandatory trailing NIL: 1431 items. Empty list is `6c 00000000 6a`.
- `0x68` — small tuple (0…255 elements), 279 items; the decoder returns them as arrays (e.g., elements of `read_state.entries` / `experiments` / `guild_experiments`).

## 2. Event Catalog

Total **124** messages, **12** types. SEQ runs continuously 1…102 (dispatch events), op 10/11 have no seq (their seq is atom `nil`).

| t / op | op | Count | SEQ | Size of d (JSON) | Raw bytes (min…max) | Frames | Noise? | Note |
|---|---|---|---|---|---|---|---|---|
| `HELLO` | 10 | 1 | none | 96 | 138…138 | 1…1 |  | heartbeat_interval = 41250; s/t — atom nil |
| `READY` | 0 | 1 | 1…1 | 584149 | 577124…577124 | 1…1 |  | 40 keys (39 + _trace) |
| `READY_SUPPLEMENTAL` | 0 | 1 | 2…2 | 50696 | 48706…48706 | 1…1 |  |  |
| `THREAD_LIST_SYNC` | 0 | 1 | 3…3 | 647 | 669…669 | 1…1 |  |  |
| `CHANNEL_INFO` | 0 | 1 | 10…10 | 3011 | 2341…2341 | 1…1 |  |  |
| `GUILD_MEMBERS_CHUNK` | 0 | 1 | 7…7 | 666 | 708…708 | 1…1 |  |  |
| `GUILD_MEMBER_LIST_UPDATE` | 0 | 79 | 4…102 | 936…99061 (Σ 233615) | 969…97416 | 1…1 | partial | SYNC/INSERT/UPDATE/DELETE |
| `SESSIONS_REPLACE` | 0 | 1 | 9…9 | 219 | 260…260 | 1…1 |  | d is array, not object |
| `PRESENCE_UPDATE` | 0 | 6 | 13…47 | 184…753 (Σ 1976) | 202…783 | 1…1 |  |  |
| `MESSAGE_CREATE` | 0 | 10 | 48…74 | 1069…1846 (Σ 12271) | 1165…2009 | 1…1 |  |  |
| `MESSAGE_UPDATE` | 0 | 1 | 50…50 | 1871 | 2036…2036 | 1…1 |  |  |
| `HEARTBEAT_ACK` | 11 | 21 | none | 5…5 (Σ 105) | 43…43 | 1…1 | noise | d = atom nil, 43 bytes, all identical |

Share of each type in total payload volume (Σ = 889322 bytes JSON):

| Type | Σ bytes | Share |
|---|---|---|
| `GUILD_MEMBER_LIST_UPDATE` | 233615 | 26.3 % |
| `op_11` | 105 | 0.0 % |
| `MESSAGE_CREATE` | 12271 | 1.4 % |
| `PRESENCE_UPDATE` | 1976 | 0.2 % |
| `op_10` | 96 | 0.0 % |
| `READY` | 584149 | 65.7 % |
| `READY_SUPPLEMENTAL` | 50696 | 5.7 % |
| `THREAD_LIST_SYNC` | 647 | 0.1 % |
| `GUILD_MEMBERS_CHUNK` | 666 | 0.1 % |
| `SESSIONS_REPLACE` | 219 | 0.0 % |
| `CHANNEL_INFO` | 3011 | 0.3 % |
| `MESSAGE_UPDATE` | 1871 | 0.2 % |

## 3. READY — Full Field Schema

One event (seq=1), size 584149 (570.5 KiB) JSON / 577124 (563.6 KiB) decompressed bytes, fits in one WS frame. Top-level keys: **40** (39 significant + `_trace`, which is discarded).

The "ETF" column shows the stream tag; "Presence" shows how many objects contained the key (e.g. `272/272` — in all channels). Values of `nil` in examples are atoms, while `null` is the NIL tag (see §1.4).

| Field | Type (after decoder) | ETF | Example | Comment |
|---|---|---|---|---|
| `game_relationships` | null | nil | null |  |
| `explicit_content_scan_version` | int | int | 4 |  |
| `user_settings_proto` | string | str | REDACTED…<len=4400> | bin, 4400 bytes base64-protobuf (truncated in example) |
| `subdivision_code` | string | str | HE |  |
| `v` | int | int | 9 |  |
| `linked_users` | null | nil | null |  |
| `regional_feature_config` | object | map | {"age_gated_features":687,"should_collect_app_store_signal":"fal… |  |
| `relationships` | array | list | [{"id":"-12866369726921572864","is_spam_request":"false","nickna… |  |
| `guild_experiments` | array | list | [[944104242,"nil"],["-3262271744","nil"]] |  |
| `api_code_version` | int | int | 1 |  |
| `resume_gateway_url` | string | str | wss://gateway-us-east1-c.discord.gg |  |
| `session_type` | string | str | normal |  |
| `read_state` | object | map | {"entries":[{"flags":0,"id":"-14827589263259861248","last_messag… |  |
| `apex_experiments` | object | map | {"assignments":{"1":{"384299970632876032":{"assignments":[["-320… |  |
| `country_code` | string | str | DE |  |
| `notification_settings` | object | map | {"flags":0} |  |
| `geo_ordered_rtc_regions` | array | list | ["frankfurt","brussels"] |  |
| `tutorial` | null | str | nil |  |
| `user_guild_settings` | object | map | {"entries":"…array(len=2)","partial":"false","version":734} |  |
| `merged_members` | array | list | [[{"avatar":"nil","banner":"nil","communication_disabled_until":… |  |
| `user` | object | map | {"accent_color":"nil","age_verification_status":4,"avatar":"e234… |  |
| `session_id` | string | str | REDACTED |  |
| `consents` | object | map | {"personalization":{"consented":"true"}} |  |
| `guilds` | array | list | {"…first-of":"array(len=2)","first":{"application_command_counts… |  |
| `auth_session_id_hash` | string | str | REDACTED |  |
| `auth` | object | map | {"authenticator_types":"0x02"} |  |
| `friend_suggestion_count` | int | int | 0 |  |
| `users` | array | list | {"…first-of":"array(len=2)","first":{"avatar":"a_f4e1e95983b96c2… |  |
| `broadcaster_user_ids` | null | nil | null |  |
| `guild_join_requests` | null | nil | null |  |
| `qos_token` | string | str | REDACTED |  |
| `analytics_token` | string | str | REDACTED |  |
| `private_channels` | array | list | [{"flags":0,"id":"-12344321117467458816","is_message_request":"f… |  |
| `ad_personalization_toggles_disabled` | bool | str | false |  |
| `static_client_session_id` | string | str | REDACTED |  |
| `experiments` | array | list | [["-2754442240",5],[1438986391,0]] |  |
| `pending_payments` | null | str | nil |  |
| `sessions` | array | list | [{"activities":null,"client_info":{"client":"desktop","os":"linu… |  |
| `connected_accounts` | null | nil | null |  |

Nested structures (depth ≥ 1 level).

**READY.d.user** (32 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `accent_color` | null | str | 1/1 | nil |
| `age_verification_status` | int | int | 1/1 | 4 |
| `avatar` | string | str | 1/1 | e234430751874dcfca60cdd3a53a2673 |
| `avatar_decoration_data` | null | str | 1/1 | nil |
| `banner` | null | str | 1/1 | nil |
| `banner_color` | null | str | 1/1 | nil |
| `bio` | string | str | 1/1 |  |
| `bot` | bool | str | 1/1 | false |
| `clan` | null | str | 1/1 | nil |
| `collectibles` | null | str | 1/1 | nil |
| `desktop` | bool | str | 1/1 | true |
| `discriminator` | string | str | 1/1 | 0 |
| `display_name_styles` | null | str | 1/1 | nil |
| `email` | string | str | 1/1 | REDACTED |
| `flags` | int | int | 1/1 | 32 |
| `global_name` | string | str | 1/1 | ShadowByte |
| `id` | int64 | bignum | 1/1 | -6147072113468506112 |
| `mfa_enabled` | bool | str | 1/1 | true |
| `mobile` | bool | str | 1/1 | true |
| `nsfw_allowed` | bool | str | 1/1 | true |
| `perks` | object | map | 1/1 | {"active_perks_bitmask":null,"config_by_perk":{}} |
| `phone` | null | str | 1/1 | REDACTED |
| `premium` | bool | str | 1/1 | false |
| `premium_type` | int | int | 1/1 | 0 |
| `primary_guild` | null | str | 1/1 | nil |
| `pronouns` | string | str | 1/1 |  |
| `purchased_flags` | int | int | 1/1 | 6 |
| `store_country` | object | map | 1/1 | {"country":"DE","is_locked":"false","set_at":"nil"} |
| `typing_indicator_style` | null | str | 1/1 | nil |
| `username` | string | str | 1/1 | pharos12510 |
| `vad_colors` | null | str | 1/1 | nil |
| `verified` | bool | str | 1/1 | true |

**READY.d.private_channels[] (17)** (11 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `flags` | int | int | 17/17 | 0 |
| `id` | int64 | bignum | 17/17 | -12344321117467458816 |
| `is_message_request` | bool | str | 17/17 | false |
| `is_message_request_timestamp` | null | str | 17/17 | nil |
| `is_spam` | bool | str | 17/17 | false |
| `last_message_id` | int64\|null | bignum | 17/17 | -11501585636411842304 |
| `recipient_flags` | int | int | 17/17 | 0 |
| `recipient_ids` | array | list | 17/17 | ["-6974117715061506304"] |
| `safety_warnings` | null | nil | 17/17 | null |
| `type` | int | int | 17/17 | 1 |
| `last_pin_timestamp` | string | str | 2/17 | 2021-03-17T13:34:40+00:00 |

**READY.d.merged_members[][]. (member; 12 fields)** (12 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `avatar` | null | str | 7/7 | nil |
| `banner` | null | str | 7/7 | nil |
| `communication_disabled_until` | null | str | 7/7 | nil |
| `deaf` | bool | str | 7/7 | false |
| `flags` | int | int | 7/7 | 0 |
| `joined_at` | string | str | 7/7 | 2026-09-11T12:29:43.951000+00:00 |
| `mute` | bool | str | 7/7 | false |
| `nick` | string | str | 7/7 | nil |
| `pending` | bool | str | 7/7 | false |
| `premium_since` | null | str | 7/7 | nil |
| `roles` | array\|null | list/nil | 7/7 | ["-15040210351226355712"] |
| `user_id` | int64 | bignum | 7/7 | -6147072113468506112 |

**READY.d.read_state.entries[] (157)** (9 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `flags` | int | int | 155/157 | 0 |
| `id` | int64 | bignum | 157/157 | -14827589263259861248 |
| `last_message_id` | int64 | bignum | 155/157 | -8892870717163443456 |
| `last_pin_timestamp` | string | str | 155/157 | 1970-01-01T00:00:00+00:00 |
| `mention_count` | int | int | 155/157 | 0 |
| `last_viewed` | int | int | 143/157 | 4274 |
| `badge_count` | int | int | 2/157 | 16 |
| `last_acked_id` | int64 | bignum | 2/157 | -17738099758636990464 |
| `read_state_type` | int | int | 2/157 | 2 |

**READY.d.user_guild_settings.entries[] (58)** (13 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `channel_overrides` | array\|null | list/nil | 58/58 | [{"channel_id":"-5385433394157977600","collapsed":"false… |
| `flags` | int | int | 58/58 | 16384 |
| `guild_id` | int64\|null | bignum | 58/58 | -1467272931325252608 |
| `hide_muted_channels` | bool | str | 58/58 | false |
| `message_notifications` | int | int | 58/58 | 1 |
| `mobile_push` | bool | str | 58/58 | false |
| `mute_config` | object\|null | map/str | 58/58 | {"end_time":"nil","selected_time_window":-1} |
| `mute_scheduled_events` | bool | str | 58/58 | true |
| `muted` | bool | str | 58/58 | true |
| `notify_highlights` | int | int | 58/58 | 1 |
| `suppress_everyone` | bool | str | 58/58 | false |
| `suppress_roles` | bool | str | 58/58 | true |
| `version` | int | int | 58/58 | 734 |

**READY.d.user_guild_settings.entries[].channel_overrides[] (1328)** (6 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `channel_id` | int64 | bignum | 1328/1328 | -5385433394157977600 |
| `collapsed` | bool | str | 1328/1328 | false |
| `flags` | int | int | 1253/1328 | 4096 |
| `message_notifications` | int | int | 1328/1328 | 3 |
| `mute_config` | object\|null | str/map | 1328/1328 | nil |
| `muted` | bool | str | 1328/1328 | false |

**READY.d.sessions[] (1)** (6 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `activities` | null | nil | 1/1 | null |
| `client_info` | object | map | 1/1 | {"client":"desktop","os":"linux","version":0} |
| `hidden_activities` | null | nil | 1/1 | null |
| `processed_at_timestamp` | int | int | 1/1 | 0 |
| `session_id` | string | str | 1/1 | REDACTED |
| `status` | string | str | 1/1 | invisible |

**READY.d.guilds[] (7 guilds)** (19 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `application_command_counts` | object | map | 7/7 | {} |
| `channel_settings` | object | map | 7/7 | {} |
| `channels` | array | list | 7/7 | {"…first-of":"array(len=2)","first":{"application_id":"n… |
| `data_mode` | string | str | 7/7 | full |
| `emojis` | array\|null | list/nil | 7/7 | [{"animated":"false","available":"true","id":"-352726283… |
| `experiments` | object | map | 7/7 | {"assignments":null,"evaluation_id":"nil"} |
| `guild_scheduled_events` | null | nil | 7/7 | null |
| `id` | int64 | bignum | 7/7 | -14827589263259861248 |
| `joined_at` | string | str | 7/7 | 2026-09-11T12:29:43.951000+00:00 |
| `large` | bool | str | 7/7 | false |
| `lazy` | bool | str | 7/7 | true |
| `member_count` | int | int | 7/7 | 157 |
| `premium_subscription_count` | int | int | 7/7 | 0 |
| `properties` | object | map | 7/7 | {"hub_type":"nil","moderator_reporting":"nil","vanity_ur… |
| `roles` | array | list | 7/7 | [{"color":0,"colors":{"primary_color":0,"secondary_color… |
| `stage_instances` | null | nil | 7/7 | null |
| `stickers` | array\|null | list/nil | 7/7 | [{"asset":"","available":"true","description":"[sticker text]… |
| `threads` | null | nil | 7/7 | null |
| `version` | int64 | bignum | 7/7 | -177376947577600 |

**READY.d.guilds[].channels[] (272 channels)** (34 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `application_id` | null | str | 134/272 | nil |
| `available_tags` | array\|null | nil/list | 68/272 | null |
| `default_reaction_emoji` | object\|null | str/map | 68/272 | nil |
| `default_sort_order` | null | str | 68/272 | nil |
| `default_tag_setting` | string | str | 68/272 | nil |
| `flags` | int | int | 272/272 | 131072 |
| `game_id` | null | str | 62/272 | nil |
| `hd_streaming_buyer_id` | null | str | 62/272 | nil |
| `hd_streaming_until` | null | str | 62/272 | nil |
| `icon_emoji` | object\|null | str/map | 191/272 | nil |
| `id` | int64 | bignum | 272/272 | -7897473852211469824 |
| `last_message_id` | int64\|null | bignum | 237/272 | nil |
| `last_pin_timestamp` | string | str | 86/272 | nil |
| `linked_lobby` | null | str | 62/272 | nil |
| `name` | string | str | 272/272 | ___hidden___ |
| `nsfw` | bool | str | 237/272 | false |
| `parent_id` | int64\|null | bignum | 272/272 | nil |
| `permission_overwrites` | array\|null | list/nil | 272/272 | [{"allow":"0","deny":"1024","id":"-14827589263259861248"… |
| `position` | int | int | 272/272 | 24 |
| `rate_limit_per_user` | int | int | 229/272 | 0 |
| `rtc_region` | string | str | 134/272 | nil |
| `status` | null | str | 126/272 | nil |
| `theme_color` | null | str | 191/272 | nil |
| `topic` | string | str | 165/272 | nil |
| `type` | int | int | 272/272 | 0 |
| `voice_background_display` | object\|null | str/map | 113/272 | nil |
| `voice_hangout` | null | str | 113/272 | nil |
| `bitrate` | int | int | 82/272 | 96000 |
| `user_limit` | int | int | 82/272 | 0 |
| `video_quality_mode` | int | int | 3/272 | 2 |
| `default_auto_archive_duration` | int | int | 6/272 | 10080 |
| `template` | string | str | 9/272 |  |
| `default_forum_layout` | int | int | 1/272 | 2 |
| `default_thread_rate_limit_per_user` | int | int | 7/272 | 0 |

**READY.d.guilds[].roles[] (132 roles)** (13 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `color` | int | int | 132/132 | 0 |
| `colors` | object | map | 132/132 | {"primary_color":0,"secondary_color":"nil","tertiary_col… |
| `flags` | int | int | 132/132 | 0 |
| `hoist` | bool | str | 132/132 | false |
| `icon` | string | str | 132/132 | nil |
| `id` | int64 | bignum | 132/132 | -14827589263259861248 |
| `managed` | bool | str | 132/132 | false |
| `mentionable` | bool | str | 132/132 | false |
| `name` | string | str | 132/132 | @everyone |
| `permissions` | string | str | 132/132 | 2221977711673344 |
| `position` | int | int | 132/132 | 0 |
| `tags` | object | map | 132/132 | {} |
| `unicode_emoji` | string | str | 132/132 | nil |

**Other nested READY collections**:

| Structure | Size | Element Keys | Note |
|---|---|---|---|
| d.private_channels[] | 17 (type=1) | flags, id, is_message_request, is_message_request_timestamp, is_spam, last_message_id, recipient_flags, recipient_ids, safety_warnings, type, last_pin_timestamp | DM/group channels; recipient_ids are snowflakes |
| d.users[] | 146 | avatar, avatar_decoration_data, bot, collectibles, discriminator, display_name, display_name_styles, global_name, id, primary_guild, public_flags, username, vad_colors, clan | simplified user (14 fields) |
| d.relationships[] | 60 (type=1,2) | id, is_spam_request, nickname, since, type, user_id, user_ignored | friends/requests |
| d.merged_members[][] | 1,1,1,1,1,1,1 | avatar, banner, communication_disabled_until, deaf, flags, joined_at, mute, nick, pending, premium_since, roles, user_id | one array per guild: guild → [member] |
| d.read_state.entries[] | 157 | flags, id, last_message_id, last_pin_timestamp, mention_count, last_viewed, badge_count, last_acked_id, read_state_type | version=128949, partial |
| d.user_guild_settings.entries[] | 58 | channel_overrides, flags, guild_id, hide_muted_channels, message_notifications, mobile_push, mute_config, mute_scheduled_events, muted, notify_highlights, suppress_everyone, suppress_roles, version | version=734 |
| d.sessions[] | 1 | activities, client_info, hidden_activities, processed_at_timestamp, session_id, status | session_id redacted, client_info={client,os,version} |
| d.experiments[] | 6 | tuple elements of form [id, 5, 11, -1, 1, 3219, 0, 0, "nil", "nil", "nil"] | tuples (0x68), not objects |
| d.guild_experiments[] | 17 | nested tuples/lists, many nil atoms | the most "irregular" READY structure |
| d.geo_ordered_rtc_regions[] | 5 | frankfurt, brussels, rotterdam, paris, milan | RTC region preference order |
| d.consents | 1 | personalization.consented | values are atoms true/false |
| d.apex_experiments | 1 | assignments → <id> → {assignments, evaluation_id} | keys are numeric strings |
| d.notification_settings | 1 | flags | int |
| d.regional_feature_config | 3 | age_gated_features, should_collect_app_store_signal, teen_by_default_settings | numbers + atoms |
| d.auth | 1 | authenticator_types (0x6b string, raw bytes) | not a string |
| d.pending_payments, d.tutorial | — | atom nil | not NIL tag, but specifically atom-string |

**What is NOT in READY (important):** no `presences` key at all; inside `guilds[]` there are no `members`, `presences`, or `threads` (threads = null, members moved to `merged_members`, statuses in `READY_SUPPLEMENTAL.merged_presences` and subsequent `PRESENCE_UPDATE`).

**READY_SUPPLEMENTAL** (seq=2, 50696 bytes): `disclose`, `game_invites`, `guilds`, `lazy_private_channels`, `merged_members`, `merged_presences`, `user_activities`; guild keys: `activity_instances`, `embedded_activities`, `id`, `voice_states`; `merged_presences.friends` = 2, `merged_presences.guilds` = 7, `disclose` = ["pomelo"].

## 4. GUILD_MEMBER_LIST_UPDATE

All 79 events have the same set of keys: `groups`, `guild_id`, `id`, `member_count`, `online_count`, `ops`. This is the most frequent type (64% of messages).

| Field | Type | ETF | Value/Range | Semantics |
|---|---|---|---|---|
| `guild_id` | int64 | bignum | -1467272931325252608 | guild of the list |
| `id` | string | bin (79/79) | everyone | member list id (lobby/channel/"everyone") |
| `member_count` | int | uint8/int32 | 24227…24228 | total members (changes during capture) |
| `online_count` | int | uint8/int32 | 657…679 | online |
| `groups` | array | list | always 2 elements; elements: `count`, `id` | group headers: {id: "online"\|"<role_id>", count} |
| `ops` | array | list | SYNC×1, UPDATE×57, DELETE×21, INSERT×21 | list of change operations |

### 4.1 Operations in ops[]

| op | Count | Operation Keys | items (min…max) | Semantics |
|---|---|---|---|---|
| `SYNC` | 1 | items, op, range | 100…100 | full rebuild of the visible window: items = sequence of groups and members |
| `INSERT` | 21 | index, item, op | — (single item) | insert a single element by index |
| `UPDATE` | 57 | index, item, op | — (single item) | update element by index (status/role/nick) |
| `DELETE` | 21 | index, op | — (no item) | delete element by index |

Data observations:

- `index` appears only for INSERT/UPDATE/DELETE (1…99 in this dump), `range` — only for SYNC.
- `range` is **not an array**, but a 2-byte string/binary: observed `0063` = bytes [0, 99] (start and end of window). gw-decode.mjs returns it as string `\u0000c` — the consumer must unpack the bytes manually.
- In a SYNC event there are 100 items elements: 2 groups and 98 members; in INSERT/UPDATE, item is a single wrapper object `item.member` (78 items) or `item.group`.
- An `item.member` element is a guild member **with an embedded presence** (`member.presence.user`, `client_status`, `activities`), i.e. the member list carries statuses.
- `groups` is always present (2 per event, 158 total), but the `members` list never appears in this dump (0) — replaced by `items[].member`.

## 5. MESSAGE_CREATE / MESSAGE_UPDATE

MESSAGE_CREATE: **10** events (seq 48…74), sizes 1069…1846 bytes JSON. All events have the same set of **19** keys: `attachments`, `author`, `channel_id`, `channel_type`, `components`, `content`, `edited_timestamp`, `embeds`, `flags`, `guild_id`, `id`, `member`, `mention_everyone`, `mention_roles`, `mentions`, `pinned`, `timestamp`, `tts`, `type`.

| Field | Type | ETF | Presence | Example/Value |
|---|---|---|---|---|
| `guild_id` | int64 | bignum | 10/10 | -1467272931325252608 |
| `attachments` | null | nil | 10/10 | null |
| `author` | object | map | 10/10 | {"avatar":"156a0d2872579f1ffcaa5d2127239bfd","avatar_decorat… |
| `channel_id` | int64 | bignum | 10/10 | -1467273046216692736 |
| `channel_type` | int | int | 10/10 | 0 |
| `components` | null | nil | 10/10 | null |
| `content` | string | str | 10/10 | > Hello <@!1560490000590700634> glad to…<len=299> |
| `edited_timestamp` | null | str | 10/10 | nil |
| `embeds` | array\|null | nil/list | 10/10 | null |
| `flags` | int | int | 10/10 | 0 |
| `id` | int64 | bignum | 10/10 | -12394240098567211520 |
| `member` | object | map | 10/10 | {"avatar":"nil","banner":"nil","communication_disabled_until… |
| `mention_everyone` | bool | str | 10/10 | false |
| `mention_roles` | null | nil | 10/10 | null |
| `mentions` | array\|null | list/nil | 10/10 | [{"avatar":"nil","avatar_decoration_data":"nil","clan":"nil"… |
| `pinned` | bool | str | 10/10 | false |
| `timestamp` | string | str | 10/10 | 2026-10-19T06:18:08.463000+00:00 |
| `tts` | bool | str | 10/10 | false |
| `type` | int | int | 10/10 | 0 |

**MESSAGE_CREATE.d.author** (13 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `avatar` | string | str | 10/10 | 156a0d2872579f1ffcaa5d2127239bfd |
| `avatar_decoration_data` | null | str | 10/10 | nil |
| `bot` | bool | str | 10/10 | true |
| `clan` | null | str | 10/10 | nil |
| `collectibles` | null | str | 10/10 | nil |
| `discriminator` | string | str | 10/10 | 5803 |
| `display_name_styles` | null | str | 10/10 | nil |
| `global_name` | null | str | 10/10 | nil |
| `id` | int64 | bignum | 10/10 | -17071683194832551936 |
| `primary_guild` | null | str | 10/10 | nil |
| `public_flags` | int | int | 10/10 | 65536 |
| `username` | string | str | 10/10 | ProBot ✨ |
| `vad_colors` | null | str | 10/10 | nil |

**MESSAGE_CREATE.d.member** (11 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `avatar` | null | str | 10/10 | nil |
| `banner` | null | str | 10/10 | nil |
| `communication_disabled_until` | null | str | 10/10 | nil |
| `deaf` | bool | str | 10/10 | false |
| `flags` | int | int | 10/10 | 0 |
| `joined_at` | string | str | 10/10 | 2025-10-29T11:51:15.562000+00:00 |
| `mute` | bool | str | 10/10 | false |
| `nick` | string | str | 10/10 | nil |
| `pending` | bool | str | 10/10 | false |
| `premium_since` | null | str | 10/10 | nil |
| `roles` | array | list | 10/10 | ["-16375379901382926848"] |

**MESSAGE_CREATE.d.mentions[] (in 1 event)** (13 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `avatar` | null | str | 1/1 | nil |
| `avatar_decoration_data` | null | str | 1/1 | nil |
| `clan` | null | str | 1/1 | nil |
| `collectibles` | null | str | 1/1 | nil |
| `discriminator` | string | str | 1/1 | 0 |
| `display_name_styles` | null | str | 1/1 | nil |
| `global_name` | string | str | 1/1 | [user display name] |
| `id` | int64 | bignum | 1/1 | -12103814603318778368 |
| `member` | object | map | 1/1 | {"avatar":"nil","banner":"nil","communication_disabled_u… |
| `primary_guild` | null | str | 1/1 | nil |
| `public_flags` | int | int | 1/1 | 0 |
| `username` | string | str | 1/1 | stanislav_52989 |
| `vad_colors` | null | str | 1/1 | nil |

**MESSAGE_UPDATE.d.embeds[] (1 event)** (10 keys)

| Field | Type | ETF | Presence | Example |
|---|---|---|---|---|
| `author` | object | map | 1/1 | {"icon_url":"https://cdn.discordapp.com/embed/avatars/0.… |
| `color` | int | int | 1/1 | 3092790 |
| `content_scan_version` | int | int | 1/1 | 4 |
| `description` | string | str | 1/1 | :writing_hand: **<@!1560490000590700634> was updated.** |
| `fields` | array | list | 1/1 | [{"inline":"false","name":"**Roles:**","value":":white_c… |
| `footer` | object | map | 1/1 | {"text":"ITSKIDON PC BOOST"} |
| `id` | int64 | bignum | 1/1 | -12394242370638462464 |
| `thumbnail` | object | map | 1/1 | {"content_type":"image/png","flags":0,"height":256,"plac… |
| `timestamp` | string | str | 1/1 | 2026-10-19T06:18:10.499000+00:00 |
| `type` | string | str | 1/1 | rich |

Observations:

- `content` — always `0x6d` binary (UTF-8), lengths 299, 0, 143, 137, 127, 167, 132, 137, 152, 156; non-ASCII text arrives as binary (see §1.4).
- `type` ∈ {0}, `flags` ∈ {0}, `pinned` — atom `false`.
- `mentions`: 1, null, null, null, null, null, null, null, null, null — in 9 out of 10 events this is **null**, not an empty array; when present — array of objects with keys `avatar`, `avatar_decoration_data`, `clan`, `collectibles`, `discriminator`, `display_name_styles`, `global_name`, `id`, `member`, `primary_guild`, `public_flags`, `username`, `vad_colors`.
- `attachments` = null in all 10 events; `embeds`: null, 1, null, null, null, null, null, null, null, null.
- `author` (13 fields: `avatar`, `avatar_decoration_data`, `bot`, `clan`, `collectibles`, `discriminator`, `display_name_styles`, `global_name`, `id`, `primary_guild`, `public_flags`, `username`, `vad_colors`), `member` (11 fields: `avatar`, `banner`, `communication_disabled_until`, `deaf`, `flags`, `joined_at`, `mute`, `nick`, `pending`, `premium_since`, `roles`).
- **Never observed** (commonly expected fields): `attachments (always null in this dump)`, `embeds (9/10 null, 1 event with array)`, `components`, `mention_roles`, `reactions`, `reference`, `sticker_items`, `nonce`, `webhook_id`, `application_id`, `message_snapshots` — absent in dump, schema cannot be built from them.

MESSAGE_UPDATE (1 event, seq 50) has the same set of keys, but `embeds` is an array with 1 element, `content` = empty string, `mentions` = null.

## 6. HELLO (op 10) and HEARTBEAT_ACK (op 11)

### 6.1 HELLO

Full bytes (138 decompressed bytes):

```
83 74 00000004 64000164 7400000002 6400065f7472616365 6c00000001 6d00000032 5b22676174657761792d7072642d61726d2d75732d65617374312d632d31386666222c7b226d6963726f73223a302e307d5d 6a 6400126865617274626561745f696e74657276616c 620000a122 6400026f70 610a 64000173 6400036e696c 64000174 6400036e696c
```

| Key | ETF | Value |
|---|---|---|
| `d.heartbeat_interval` | 0x62 int32 | **41250** ms (41.25 s) |
| `d._trace` | 0x6c list + 0x6d binary | ["gateway-prd-arm-us-east1-c-18ff",{"micros":0.0}] |
| `op` | 0x61 uint8 | 10 |
| `s` | 0x64 atom | `nil` |
| `t` | 0x64 atom | `nil` |

Format: `0x83` + map(4): `d`, `op`, `s`, `t`. No other fields (no jitter, no shard info) — the stated interval is exactly `41250`.

### 6.2 HEARTBEAT_ACK

- 21 items, **43 bytes** each, all **byte-for-byte identical** (one md5 for all 21 events).
- Full contents: `0x83` + map(4) {d: atom `nil`, op: 11, s: atom `nil`, t: atom `nil`) → in JSON `{"d":"nil","op":11,"s":"nil","t":"nil"}`.
- **Why empty:** ETF has no separate "void"; the server places atom `nil` in `d`. No payload, therefore this is pure service noise (see §7).
- ACK indices: 21 items — 7, 15, 18, 22, 27, 32, 36, 38, 41, 50, 51, 53, 57, 67, 77, 81, 84, 99, 105, 114, 122.

### 6.3 Heartbeat Rhythm

- Declared in HELLO: `heartbeat_interval = 41250 ms`.
- Client heartbeat frames in capture: **9** frames of 90 bytes length (the only repeating length in the outgoing stream), intervals between them: 123 s, 41 s, 165 s, 41 s, 83 s, 41 s, 41 s, 41 s — multiples of 41.25 (123 = 3×41, 165 = 4×41), confirming the interval.
- **Discrepancy (observed fact):** 21 ACK frames but only 9 recorded heartbeats. This means `wssent-bin` in the CDP capture does not record some outgoing frames, or the client sends heartbeats more frequently than the stated interval. Additionally: deltas between ACK timestamps are uneven (27 s, 13 s, 17 s, 24 s, 41 s, 41 s, 8 s, 33 s…), meaning `wsrecv` timestamps in CDP are batched and unsuitable for rhythm measurement.

## 7. Noise, Duplicates, Discarded

| Category | Count | Bytes (each / total) | Why noise |
|---|---|---|---|
| noise/service: HEARTBEAT_ACK (op 11) | 21 | 43 / 903 | no payload: d = atom 'nil', all 21 frames byte-for-byte identical (43 bytes) |
| low per-event information: GUILD_MEMBER_LIST_UPDATE: ops[].op=UPDATE/DELETE | 78 | — | a single event carries little data (index + one member), but the stream of 78 events = 63% of all traffic |
| discarded: _trace | 2 | 1943 / 3886 | routing telemetry (JSON string in ETF-binary); excluded from schema |
| noise: duplicate frames | 21 | 903 / 18963 | 21 identical HEARTBEAT_ACK; no other complete duplicates among dispatch events |

Discarded from schema (not included in `gateway-schema.json`):

| Path | Where | Action | Detail |
|---|---|---|---|
| `d._trace` | op_10 (HELLO), READY | `drop` | telemetry: ["gateway-prd-arm-us-east1-c-18ff",{"micros":0.0}] (HELLO) and 1893-byte string (READY) |
| `HEARTBEAT_ACK.d` | op_11 | `mark-empty` | atom 'nil' — zero payload |
| `secrets` | READY.session_id, READY.auth_session_id_hash, READY.qos_token, READY.analytics_token, READY.static_client_session_id, READY.user.email, READY.user.phone, any *token/*secret/*password | `redact-to-REDACTED` | values replaced with "REDACTED"; session_type/sessions not hidden (not secrets) |

Duplicates: full md5 analysis of `d` found **1** group with repeats — only the 21 identical HEARTBEAT_ACK. Among dispatch events there are no complete duplicates (79 GUILD_MEMBER_LIST_UPDATE events differ at least in counters/indices).

## 8. Limitations and Open Questions

- live capture snapshot from a single client session (one guild in focus + 7 guilds in READY); not all Discord event types are represented
- not observed: MESSAGE_DELETE, MESSAGE_REACTION_*, TYPING_START, GUILD_CREATE/UPDATE/DELETE, VOICE_*, PRESENCE_UPDATE with activities, RESUME/dedup scenarios, READY.presences (key absent in dump)
- client→server frames recorded incompletely (9 heartbeat frames len=90 vs 21 ACK) — the CDP capture reverse channel loses some frames
- wsrecv timestamps in CDP are grouped/batched: cannot recover exact ACK rhythm from them (deltas 2..41 s)
- dual string encoding (atom vs binary) was observed but the exact selection rule has not been established

Additional notes:

- The data type after the decoder does not match the "logical" type: `"nil"`/`"true"`/`"false"` are strings, int64 is BigInt/string. All tables above give the "logical" type and a separate ETF column.
- The atom vs binary selection rule for strings has not been established (§1.4) — only observations recorded.
- Schemas are built from **one** session, so the field set may differ from other accounts/guilds (e.g. `emojis` = null for 2 of 7 guilds, `threads` = null for all 7).

## 9. Artifacts and Reproducibility

| File | Contents |
|---|---|
| `/home/user/clean-room-oracle/gateway-schema.md` | this report |
| `/home/user/clean-room-oracle/gateway-schema.json` | machine-readable schemas, statistics, noise/discarded |
| `gateway-schema-work/capture-ready.snapshot.jsonl` | live capture snapshot (md5 ff4a8e393b0e322fe393adaf6426142e) |
| `gateway-schema-work/analyze.mjs` | independent analyzer (tags, sizes, schemas, extras) |
| `gateway-schema-work/render.mjs` | renders this report and JSON |
| `gateway-schema-work/analysis.json` | numerical analysis results |
| `gateway-schema-work/schemas.json` | merge trees by type (with field presence) |
| `gateway-schema-work/oracle-out/*` | reference gw-decode.mjs run on snapshot (for verification) |

Sanitization: values of keys `session_id`, `auth_session_id_hash`, `qos_token`, `analytics_token`, `static_client_session_id`, `email`, `phone`, and any `*token*`/`*secret*`/`*password*` are replaced with `REDACTED`; `_trace` is not included; long strings in examples are truncated (`…<len=N>`). Identifiers (snowflake ids), names, and message texts remain as in the dump.
