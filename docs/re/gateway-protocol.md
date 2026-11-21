# Discord Gateway Transport Protocol: WS + ETF + zstd-stream

**Cleanroom Reverse Engineering Specification**  
Empirical analysis based strictly on CDP WebSocket frame captures. No live network traffic initiated during analysis.

- **Baseline Fact Snapshot:** `gw-proto/facts-frozen.json`
- **Machine-Readable Schema:** `gateway-protocol.json`
- **Primary Inputs:** `capture-ready.jsonl` (run1), `capture-run2.jsonl` (run2), partial decodes in `gateway/messages.jsonl`, decoder `gw-decode.mjs`.

**Key Architectural Invariants:**
1. Exactly **11 ETF tags** appear in observed traffic: 70, 97, 98, 100, 104, 106, 107, 108, 109, 110, 116 — totaling **486,300** tag occurrences.
2. Snowflake IDs use **big-int** encoding `[110][n:u8][sign:u8][n bytes LE]`: sign precedes digits.
3. `zstd-stream` maintains a single contiguous compression stream per connection; magic bytes `28 b5 2f fd` appear only in frame 0.
4. Inbound Gateway opcodes: **0 (Dispatch), 10 (HELLO), 11 (ACK)**, and **7 (RECONNECT)**.
5. Standalone `GUILD_CREATE` dispatches are not emitted when `data_mode=full`: complete guild data arrives inside `READY.guilds[]`.

---

## 1. Methodology

- WebSocket frame captures recorded via CDP `webSocketFrameSent/Received`. Outbound binary frames are masked as `length + sha256` to prevent token leakage.
- Independent decoder validation:
  1. `gateway/messages.jsonl` (93 terms) aligns as an exact prefix of `.1389` (137 terms) with 0 discrepancies.
  2. Resolves big-int byte ordering: negative ID discrepancies in earlier parsers arose from evaluating sign after numeric payload.
  3. Gateway Snowflake IDs cross-referenced against REST API and CDN URLs.

## 2. ETF Wire Representation and Tag Map

Framing: Uncompressed stream emits `0x83` (ETF version byte) prior to every term. Initial stream payload: `83 74 00 00 00 04 | 64 00 01 64 …` — `MAP(4)` representing HELLO.

| Tag | Code | Type | Occurrences | Field Example |
|---:|---|---|---:|---|
| 70 | 0x46 | NEW_FLOAT_EXT (double BE) | 240 | `term.d.soundboard_sounds[0].volume` |
| 97 | 0x61 | SMALL_INTEGER_EXT (u8) | 32,920 | `term.op` |
| 98 | 0x62 | INTEGER_EXT (i32 BE) | 13,159 | `term.d.heartbeat_interval` |
| 100 | 0x64 | ATOM_EXT (u16 len, latin1) | 243,387 | Term dictionary keys `term.<key>` |
| 104 | 0x68 | SMALL_TUPLE_EXT (u8 count) | 1,113 | `apex_experiments.assignments[0]` |
| 106 | 0x6a | NIL_EXT (null / empty list) | 12,621 | `term.d._trace[nil]`, list terminators |
| 107 | 0x6b | STRING_EXT (u16 len) | 178 | `auth.authenticator_types` |
| 108 | 0x6c | LIST_EXT (u32 count + nil trailer) | 8,794 | `term.d._trace` |
| 109 | 0x6d | BINARY_EXT (u32 len, UTF-8) | 99,030 | String payloads and message content |
| 110 | 0x6e | SMALL_BIG_EXT (u8 n; sign; n bytes LE) | 34,843 | Snowflake IDs (`relationships[0].id`) |
| 116 | 0x74 | MAP_EXT (u32 count) | 40,015 | `term.d` |

### Clarifications on Wire Data:
1. **nil representations:** ATOM `"nil"` (tag 100) accounts for 42,232 instances; NIL_EXT (tag 106) accounts for 12,621 instances (8,794 being LIST_EXT terminators).
2. **Booleans:** Transmitted as ATOM strings `"true"` (6,894) and `"false"` (25,977).
3. **Snowflake Big-Int Invariant:** Format is `[110][n:u8][sign:u8][n bytes LE]`.
   - `guild.id` raw `6e 08 00 14 30 04 f5 cc 5c 14 0e` → **`1014537851418980372`** (1,503 occurrences).
   - `user.id` raw `6e 08 00 00 00 c4 84 c9 4e 55 05` → **`384299970632876032`** (58 occurrences).
   - `channel.id` raw `6e 08 00 47 20 84 11 10 c8 54 10` → **`1176785373989445703`** (31 occurrences).

## 3. Streaming Compression: `zstd-stream`

- Connection endpoint URI: `?encoding=etf&v=9&compress=zstd-stream`.
- **Contiguous zstd context:** The zstd magic header (`28 b5 2f fd`) appears exclusively in frame 0. Subsequent frames are raw streaming delta continuations without headers.
- The decompressor must preserve dictionary context across frames and must not call `end()` between frames.

Socket Compression Metrics:

| Socket ID | Frame Count | Compressed Bytes | Decompressed Bytes | Decoded Terms |
|---|---:|---:|---:|---:|
| run1:57992.5112 | 161 | 121,759 | 946,388 | 161 |
| run2:95974.1389 | 137 | 104,066 | 824,816 | 137 |
| run2:95974.5532 | 109 | 115,891 | 859,251 | 109 |
| run2:95974.8269 | 648 | 86,774 | 898,045 | 648 |
| run2:95974.8608 | 71 | 19,976 | 114,474 | 71 |
| run2:95974.10029 | 374 | 145,255 | 1,289,082 | 374 |
| run2:95974.13072 | 44 | 17,759 | 66,841 | 44 |

## 4. Opcodes, Heartbeat, and Resumption Lifecycle

Inbound Opcode Distribution:

| Socket | Mode | Sequence `s` Range | Opcode 0 | Opcode 10 | Opcode 11 | Opcode 7 | First Dispatch |
|---|---|---|---:|---:|---:|---:|---|
| run1:57992.5112 | Fresh Login | 1..134 | 134 | 1 | 26 | — | READY |
| run2:95974.1389 | Fresh Login | 1..113 | 113 | 1 | 23 | — | READY |
| run2:95974.5532 | Fresh Login | 1..94 | 94 | 1 | 14 | — | READY |
| run2:95974.8269 | Resume #1 | 95..663 | 569 | 1 | 78 | — | GUILD_MEMBER_LIST_UPDATE |
| run2:95974.8608 | Resume #2 | 664..722 | 59 | 1 | 11 | — | GUILD_MEMBER_LIST_UPDATE |
| run2:95974.10029 | Fresh Login | 1..331 | 331 | 1 | 41 | **1** | READY |
| run2:95974.13072 | Resume #3 | 332..371 | 40 | 1 | 3 | — | **RESUMED** |

- Heartbeat interval: `HELLO.d.heartbeat_interval = 41250` ms.
- Reconnection handling: Upon receiving `Opcode 7 (RECONNECT)`, client closes transport socket and initiates RESUME on new connection; sequence `s` transitions cleanly without packet loss.
- Total recorded Opcode 0 Dispatches across sessions: 1,340 events (READY 4, READY_SUPPLEMENTAL 4, RESUMED 3, GUILD_MEMBER_LIST_UPDATE 1048, PRESENCE_UPDATE 107, MESSAGE_CREATE 31, etc.).

## 5. `READY` Payload Schema and Metrics

4 Login Sessions analyzed:
- Top-level keys: **40** properties (`TOP_LEVEL_KEYS`). User object: 32 properties.
- Entity counts: Guilds (7), Private Channels (17, all type 1 DM), Relationships (60), Read States (157 entries), User Guild Settings (58).
- Payload size: ~579 KB JSON, ~577 KB ETF per instance. Top sections by size: `guilds` (260 KB), `user_guild_settings` (193 KB), `users` (60 KB).

### 5.1 Complete 40 Top-Level `READY` Keys (`TOP_LEVEL_KEYS`)

| # | Wire Key (`d.<key>`) | Wire / ETF Type | Strongly-Typed Mapping (`chord_core::ready`) | Notes |
|---:|---|---|---|---|
| 1 | `_trace` | `LIST_EXT` (`Vec<String>`) | Retained in `Ready::raw()` | Gateway server trace hops |
| 2 | `ad_personalization_toggles_disabled` | `ATOM_EXT` (`"true"`/`"false"`) | `Ready.ad_personalization_toggles_disabled: bool` | Normalized boolean |
| 3 | `analytics_token` | `BINARY_EXT` | Retained in `Ready::raw()` (sanitized `REDACTED`) | Telemetry token |
| 4 | `apex_experiments` | `MAP_EXT` | Retained in `Ready::raw()` | Client experiment assignments |
| 5 | `api_code_version` | `SMALL_INTEGER_EXT` | `Ready.api_code_version: Option<i64>` | Value `1` in capture |
| 6 | `auth` | `MAP_EXT` | Retained in `Ready::raw()` | Contains `authenticator_types` |
| 7 | `auth_session_id_hash` | `BINARY_EXT` | Retained in `Ready::raw()` (sanitized `REDACTED`) | Auth session hash |
| 8 | `broadcaster_user_ids` | `NIL_EXT` / `LIST_EXT` | Retained in `Ready::raw()` | Empty / null in capture |
| 9 | `connected_accounts` | `NIL_EXT` / `LIST_EXT` | Retained in `Ready::raw()` | Empty / null in capture |
| 10 | `consents` | `MAP_EXT` | Retained in `Ready::raw()` | Personalization consent map |
| 11 | `country_code` | `ATOM_EXT` / `BINARY_EXT` | `Ready.country_code: Option<String>` | ISO country code (`"DE"`) |
| 12 | `experiments` | `LIST_EXT` of `SMALL_TUPLE_EXT` | `ReadySummary.experiments: usize` | 6 experiment tuples |
| 13 | `explicit_content_scan_version` | `SMALL_INTEGER_EXT` | `Ready.explicit_content_scan_version: Option<i64>` | Value `4` in capture |
| 14 | `friend_suggestion_count` | `SMALL_INTEGER_EXT` | `Ready.friend_suggestion_count: Option<i64>` | Value `0` in capture |
| 15 | `game_relationships` | `NIL_EXT` / `LIST_EXT` | Retained in `Ready::raw()` | Empty / null in capture |
| 16 | `geo_ordered_rtc_regions` | `LIST_EXT` (`Vec<String>`) | `Ready.geo_ordered_rtc_regions: Vec<String>` | 5 ordered RTC regions |
| 17 | `guild_experiments` | `LIST_EXT` of `SMALL_TUPLE_EXT` | Retained in `Ready::raw()` | 17 guild experiment entries |
| 18 | `guild_join_requests` | `NIL_EXT` / `LIST_EXT` | Retained in `Ready::raw()` | Empty / null in capture |
| 19 | `guilds` | `LIST_EXT` (`Vec<Guild>`) | `Ready.guilds: Vec<Guild>` | Anchor (required); 7 guilds in `data_mode=full` |
| 20 | `linked_users` | `NIL_EXT` / `LIST_EXT` | Retained in `Ready::raw()` | Empty / null in capture |
| 21 | `merged_members` | `LIST_EXT` (`Vec<Vec<GuildMember>>`) | Hydrated into `Guild.members: Vec<GuildMember>` | Indexed 1-to-1 by raw `guilds[i]` position |
| 22 | `notification_settings` | `MAP_EXT` | Retained in `Ready::raw()` | Top-level notification flags |
| 23 | `pending_payments` | `ATOM_EXT` `"nil"` / `LIST_EXT` | Retained in `Ready::raw()` | Empty / null in capture |
| 24 | `private_channels` | `LIST_EXT` (`Vec<PrivateChannel>`) | `Ready.private_channels: Vec<PrivateChannel>` | 17 DM channels (`type == 1`) |
| 25 | `qos_token` | `BINARY_EXT` | Retained in `Ready::raw()` (sanitized `REDACTED`) | Quality-of-Service token |
| 26 | `read_state` | `MAP_EXT` (`ReadState`) | `Ready.read_state: ReadState` | 157 `ReadStateEntry` items, `version: 128949` |
| 27 | `regional_feature_config` | `MAP_EXT` | Retained in `Ready::raw()` | Age-gated & regional flags |
| 28 | `relationships` | `LIST_EXT` (`Vec<Relationship>`) | `Ready.relationships: Vec<Relationship>` | 60 entries (18 friends `type=1`, 42 blocked `type=2`) |
| 29 | `resume_gateway_url` | `BINARY_EXT` | `Ready.resume_gateway_url: Option<String>` | Regional resume WebSocket endpoint |
| 30 | `session_id` | `BINARY_EXT` | `Ready.session_id: String` | Anchor (required); redacted in `Debug` |
| 31 | `session_type` | `ATOM_EXT` / `BINARY_EXT` | `Ready.session_type: Option<String>` | `"normal"` |
| 32 | `sessions` | `LIST_EXT` (`Vec<Session>`) | `Ready.sessions: Vec<Session>` | Active sessions with `ClientInfo` (`session_id` redacted in `Debug`) |
| 33 | `static_client_session_id` | `BINARY_EXT` | Retained in `Ready::raw()` (sanitized `REDACTED`) | Client session UUID |
| 34 | `subdivision_code` | `ATOM_EXT` / `BINARY_EXT` | `Ready.subdivision_code: Option<String>` | Subdivision code (`"HE"`) |
| 35 | `tutorial` | `ATOM_EXT` `"nil"` / `MAP_EXT` | Retained in `Ready::raw()` | Null in capture |
| 36 | `user` | `MAP_EXT` (`User`) | `Ready.user: User` | Anchor (required `user.id: u64`); 32 wire keys |
| 37 | `user_guild_settings` | `MAP_EXT` (`UserGuildSettings`) | `Ready.user_guild_settings: UserGuildSettings` | 58 `UserGuildSettingsEntry` items, 1,328 `ChannelOverride` items |
| 38 | `user_settings_proto` | `BINARY_EXT` | `Ready.user_settings_proto: Option<String>` | Base64 protobuf (4,400 chars; redacted in `Debug`) |
| 39 | `users` | `LIST_EXT` (`Vec<User>`) | `Ready.users: Vec<User>` | 146 referenced user accounts |
| 40 | `v` | `SMALL_INTEGER_EXT` | `Ready.v: Option<i64>` | Gateway protocol version (`9`) |

### 5.2 Strongly-Typed Rust Protocol Core Schema (`chord_core::ready`)

The `READY` dispatch (`op == 0, t == "READY"`) is parsed into strongly-typed Rust structs in `chord_core::ready` (`Ready::from_dispatch` / `Ready::parse` and compact `ReadySummary::from_dispatch`) with non-fatal wire anomaly telemetry in `ReadyDiagnostics` (`unknown_top_level`, `malformed`, `duplicates`, `ignored`) and fatal anchor validation via `ReadyError` (`NotReady`, `Missing`, `WrongType`, `InvalidId`):

- **User Accounts & Social Graph (`chord_core::ready::social`):**
  - `User`: `id: u64`, `username: Option<String>`, `global_name: Option<String>`, `display_name: Option<String>`, `discriminator: Option<String>`, `avatar: Option<String>`, `bot: bool` (defaults to `false` when omitted in 41/146 `users[]` entries), `public_flags: Option<i64>`.
  - `PrivateChannel`: `id: u64`, `channel_type: i64` (`1` = DM), `flags: i64`, `recipient_ids: Vec<u64>`, `last_message_id: Option<u64>`, `last_pin_timestamp: Option<String>`, `is_message_request: bool`, `is_message_request_timestamp: Option<String>`, `is_spam: bool`, `recipient_flags: i64`.
  - `Relationship`: `id: u64`, `user_id: u64`, `relation_type: i64` (`1` = friend, `2` = blocked), `nickname: Option<String>`, `since: Option<String>`, `is_spam_request: bool`, `user_ignored: bool`.
  - `Session` & `ClientInfo`: `session_id: String` (redacted in `Debug`), `status: Option<String>`, `client_info: ClientInfo` (`client`, `os`, `version`), `processed_at_timestamp: i64`.
- **Guild Lists (`chord_core::ready::guild::Guild`):**
  - `Guild`: `id: u64`, `unavailable: bool`, `name: Option<String>` (resolved from top-level `name` or `properties.name`), `icon: Option<String>` (`properties.icon`), `owner_id: Option<u64>` (`properties.owner_id`), `data_mode: Option<String>` (`"full"`), `joined_at: Option<String>`, `large: bool`, `lazy: bool`, `member_count: Option<i64>`, `premium_subscription_count: Option<i64>`, `version: Option<u64>`, `channels: Vec<Channel>`, `roles: Vec<Role>`, `emojis: Vec<Emoji>`, `stickers: Vec<Sticker>`, `members: Vec<GuildMember>`.
  - Observed 7 guilds in capture (`272` channels, `132` roles, `363` emojis, `51` stickers total):
    1. `274093052673392641` (`Игровое сообщество ⚛MONOLIT⚛`): 55 channels, 31 roles, 71 emojis, 29 stickers, 157 members.
    2. `294064825540280335` (`Generations of Players`): 24 channels, 16 roles, 150 emojis, 0 stickers, 112 members.
    3. `338001249918255106` (`True Job Project`): 45 channels, 16 roles, 25 emojis, 0 stickers, 59 members.
    4. `700482297027231765` (`SISI 18+ PROJECT`): 24 channels, 6 roles, 0 emojis, 0 stickers, 73 members.
    5. `942355007271997450` (`XOS`): 35 channels, 28 roles, 71 emojis, 16 stickers, 44,749–44,752 members.
    6. `1014537851418980372` (`ITSKIDON PC BOOST`): 83 channels, 34 roles, 46 emojis, 6 stickers, 24,228–24,229 members.
    7. `1340572775974305803` (`Game Room`): 6 channels, 1 role, 0 emojis, 0 stickers, 3 members.
- **Channel Structures (`Channel` & `PermissionOverwrite`):**
  - `Channel`: `id: u64`, `channel_type: i64` (`0` = text [137], `2` = voice [82], `4` = category [43], `5` = announcement [1], `15` = forum [9]), `name: Option<String>`, `parent_id: Option<u64>`, `position: i64`, `flags: i64`, `nsfw: bool`, `topic: Option<String>`, `last_message_id: Option<u64>`, `last_pin_timestamp: Option<String>`, `permission_overwrites: Vec<PermissionOverwrite>`, `rate_limit_per_user: Option<i64>`, `bitrate: Option<i64>`, `user_limit: Option<i64>`, `rtc_region: Option<String>`, `video_quality_mode: Option<i64>`, `default_auto_archive_duration: Option<i64>`.
  - `PermissionOverwrite`: `id: u64` (role or user Snowflake), `target_type: i64` (`0` = role, `1` = member), `allow: u64` (64-bit decimal string bitmask), `deny: u64` (64-bit decimal string bitmask).
- **Member Roles & Guild Assets (`Role`, `GuildMember`, `Emoji`, `Sticker`):**
  - `Role`: `id: u64` (`@everyone` role has `role.id == guild.id` at `position == 0`), `name: Option<String>`, `color: i64`, `hoist: bool`, `managed: bool`, `mentionable: bool`, `permissions: u64` (64-bit decimal string bitmask), `position: i64`, `flags: i64`, `unicode_emoji: Option<String>`.
  - `GuildMember`: hydrated from `d.merged_members[raw_guild_idx]` into `Guild.members` (`user_id: u64`, `nick: Option<String>`, `avatar: Option<String>`, `banner: Option<String>`, `joined_at: Option<String>`, `premium_since: Option<String>`, `communication_disabled_until: Option<String>`, `deaf: bool`, `mute: bool`, `pending: bool`, `flags: i64`, `roles: Vec<u64>`).
  - `Emoji` (`id`, `name`, `animated`, `available`, `managed`, `require_colons`, `roles: Vec<u64>`) & `Sticker` (`id`, `name`, `description`, `tags`, `asset`, `format_type`, `available`, `guild_id`, `sticker_type`).
- **Read State & User Guild Settings (`chord_core::ready::read_state`):**
  - `ReadState` (`entries: Vec<ReadStateEntry>`, `partial: bool`, `version: u64`) & `ReadStateEntry` (`id: u64`, `flags`, `last_message_id`, `last_pin_timestamp`, `mention_count`, `last_viewed`, `badge_count`, `last_acked_id`, `read_state_type`).
  - `UserGuildSettings` (`entries: Vec<UserGuildSettingsEntry>`, `partial: bool`, `version: u64`), `UserGuildSettingsEntry` (`guild_id: Option<u64>` where `None` is DM settings, `channel_overrides: Vec<ChannelOverride>`, notification/mute flags, `mute_config: Option<MuteConfig>`, `version: u64`), `ChannelOverride`, and `MuteConfig`.

## 6. Guild Invariant (`data_mode=full`)

Under `data_mode: full`, guild structures arrive pre-populated in `READY.guilds[]`, obviating subsequent individual `GUILD_CREATE` dispatches during initial handshake. Reference guild `1014537851418980372` includes 83 channels (47 text, 20 voice, 12 categories, 4 forums), 34 roles, 46 emoji, and 24,229 members.
