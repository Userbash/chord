# RE-GAP-TASK-768: Полная сверка кодовой базы `discord-lab` с эталонным RE-дампом (`gateway-protocol.json`, `voice-deep.json`, `frontend-d03.json`)

- **Карточка:** `TASK-768` (`RW-41-ECC-INTEGRATION` / Отдел `G7 RE / REA`)
- **Дата аудита:** `2026-11-10`
- **Ветка и коммит аудита:** `feature/sprints-1-5-build-004` (`6f60b11`)
- **Статус:** Завершено (`Read-Only` сверка дампа и кодовой базы)

---

## 0. Паспорт сверки и реестр эталонных оракулов (`/home/sanya/re-deep/`)

Все проверки выполнены строго в режиме чтения (`AGENTS.md` §6–§7) против первичных артефактов `/home/sanya/re-deep/` и актуального состояния рабочей ветки:

| Оракул / Артефакт | Путь | Размер | MD5 | Назначение |
|---|---|---:|---|---|
| **Gateway Protocol Oracle** | [gateway-protocol.json](file:///home/sanya/re-deep/gateway-protocol.json) | 33 031 B | `754fc8fa493d566bf1e46e9eebb82ff6` | Схема Gateway v10 (`zstd-stream` + `etf`), 118 кадров, 21 тип `DISPATCH`, ключи `READY` (40), `guilds[]` (19 + 45 `properties`), `READY_SUPPLEMENTAL` (7 + 4) |
| **Voice Deep Oracle** | [voice-deep.json](file:///home/sanya/re-deep/voice-deep.json) | 8 023 B | `634400e96773fc1999a2cd83e380d7b3` | Трасса RTC WebSocket (`v=8`), 30 кадров (`rtc1`/`rtc2`), `VoiceOp` (`8, 0, 2, 16, 1, 4, 12, 15, 11, 13, 5, 3, 6, 18, 20`), DAVE MLS (`21..31`), UDP discovery (74 B), SRTP (`aead_aes256_gcm_rtpsize`), ротация портов |
| **Frontend D-03 Oracle** | [frontend-d03.json](file:///home/sanya/re-deep/frontend-d03.json) | 15 498 B | `f76d7cdf866ac15279594ee3c9f6acce` | 4 экрана (`search`, `emoji`, `members`, `settings`: 19 категорий, 63 строки, 38 тумблеров `Голос и видео`) и 12 Mana-компонентов с CSS-токенами |
| **Web Bundle (Electron 42 / 1.0.162)** | [discord.com_assets_web.4d4b2f75fdea5470.js](file:///home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js) | 12 420 518 B | `934b1289b7ebf17bc0fd09569b23b438` | `GatewaySocket` (`class e3` @ `4,996,243..5,000,100`, `enum e2` @ `4,996,243`), `RTCControlSocket` (`@2,391,000..2,407,000`) |
| **RTC Signaling Capture 1** | [rtc1-frames.jsonl](file:///home/sanya/re-deep/voice/rtc1-frames.jsonl) | 15 кадров | `7f913c8fe659f3e20bee2cb1d5269fd8` | Первичная сессия RTC (`OP 8 -> OP 0 -> OP 2 -> OP 16 -> OP 1 -> OP 4 -> OP 5`) |
| **RTC Signaling Capture 2** | [rtc2-frames.jsonl](file:///home/sanya/re-deep/voice/rtc2-frames.jsonl) | 15 кадров | `e8003f12f2eef3136bc31a5bba16ed5b` | Переподключение RTC и обмен кодеками/ключами |

---

## 1. Сверка Gateway-протокола (`gateway-protocol.json` vs `discord-core` и `gui-spike`)

### 1.1. Матрица опкодов шлюза (`0..43`)

В эталонном бандле [discord.com_assets_web.4d4b2f75fdea5470.js](file:///home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js) (смещение `4,996,243..4,996,955`, перечисление `e2` и методы `class e3` на смещении `4,998,350..4,999,850`) определены **32 опкода шлюза**. В [`GatewayOpcode`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L17-L88) и [`stream.rs`](file:///home/sanya/orca/project/rust/crates/discord-core/src/stream.rs#L12-L26) реализовано **19 из 32 опкодов**, а **13 клиентских исходящих опкодов отсутствуют**:

| Опкод | Имя в `e2` (`@4,996,243`) | Направление | Метод `class e3` и формат `d` в оригинале | Статус в `discord-lab` |
|---:|---|---|---|---|
| `0` | `DISPATCH` | S → C | Входящие события `t` / `s` / `d` | Реализовано ([`GatewayOpcode::Dispatch`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L19)) |
| `1` | `HEARTBEAT` | C ↔ S | `send(1, seq)` | Реализовано ([`GatewayOpcode::Heartbeat`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L21)) |
| `2` | `IDENTIFY` | C → S | `IDENTIFY` (10 ключей `gateway-protocol.json`) | Реализовано ([`Identify`](file:///home/sanya/orca/project/rust/crates/discord-core/src/identify.rs#L31-L45)) |
| `3` | `PRESENCE_UPDATE` | C → S | `{status, since, activities, afk}` | Реализовано ([`OutboundPresenceUpdate`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L139-L152)) |
| `4` | `VOICE_STATE_UPDATE` | C → S | `{guild_id, channel_id, self_mute, self_deaf, self_video, preferred_region, preferred_regions, tracks, flags}` | **Частично** (нет `preferred_region`, `preferred_regions`, `tracks` — см. §1.2) |
| `5` | `VOICE_SERVER_PING` | C → S | `voiceServerPing(){this.send(5,null)}` (`@4,998,740`) | **Отсутствует (`Unknown(5)`)** |
| `6` | `RESUME` | C → S | `{token, session_id, seq}` | Реализовано ([`ResumePayload`](file:///home/sanya/orca/project/rust/crates/discord-core/src/ws.rs#L65-L71)) |
| `7` | `RECONNECT` | S → C | Переподключение с `RESUME` | Реализовано ([`GatewayOpcode::Reconnect`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L31)) |
| `8` | `REQUEST_GUILD_MEMBERS` | C → S | `{guild_id, query, limit, user_ids, presences, nonce}` | Реализовано ([`RequestGuildMembers`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L252-L266)) |
| `9` | `INVALID_SESSION` | S → C | `d: bool` (resumable) | Реализовано ([`GatewayOpcode::InvalidSession`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L35)) |
| `10` | `HELLO` | S → C | `{heartbeat_interval, _trace}` | Реализовано ([`Hello`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L92-L97)) |
| `11` | `HEARTBEAT_ACK` | S → C | `d: null` | Реализовано ([`GatewayOpcode::HeartbeatAck`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L39)) |
| `13` | `CALL_CONNECT` | C → S | `callConnect(e){this.send(13,{channel_id:e})}` (`@4,998,940`) | **Отсутствует (`Unknown(13)`)** — нужен для синхронизации звонков в ЛС/группах (`CALL_CREATE`) |
| `14` | `GUILD_SUBSCRIPTIONS` | C → S | Одиночная подписка на гильдию | Реализовано ([`GuildSubscriptions`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L298-L315)) |
| `18` | `STREAM_CREATE` | C → S | `{type, guild_id, channel_id, preferred_region}` | Реализовано ([`STREAM_CREATE_OP`](file:///home/sanya/orca/project/rust/crates/discord-core/src/stream.rs#L13)) |
| `19` | `STREAM_DELETE` | C → S | `{stream_key}` | Реализовано ([`STREAM_DELETE_OP`](file:///home/sanya/orca/project/rust/crates/discord-core/src/stream.rs#L16)) |
| `20` | `STREAM_WATCH` | C → S | `{stream_key}` | Реализовано ([`STREAM_WATCH_OP`](file:///home/sanya/orca/project/rust/crates/discord-core/src/stream.rs#L19)) |
| `21` | `STREAM_PING` | C → S | `{stream_key}` | Реализовано ([`STREAM_PING_OP`](file:///home/sanya/orca/project/rust/crates/discord-core/src/stream.rs#L22)) |
| `22` | `STREAM_SET_PAUSED` | C → S | `{stream_key, paused}` | Реализовано ([`STREAM_SET_PAUSED_OP`](file:///home/sanya/orca/project/rust/crates/discord-core/src/stream.rs#L25)) |
| `24` | `REQUEST_GUILD_APPLICATION_COMMANDS` | C → S | Запрос слеш-команд гильдии | **Отсутствует (`Unknown(24)`)** |
| `28` | `REQUEST_FORUM_UNREADS` | C → S | `requestForumUnreads(e,t,n){this.send(28,{guild_id:e,channel_id:t,threads:n.map(...)})}` (`@4,999,110`) | **Отсутствует (`Unknown(28)`)** |
| `29` | `REMOTE_COMMAND` | C → S | `remoteCommand(e,t){this.send(29,{target_session_id:e,payload:t})}` (`@4,999,220`) | **Отсутствует (`Unknown(29)`)** |
| `30` | `GET_DELETED_ENTITY_IDS_NOT_MATCHING_HASH` | C → S | `getDeletedEntityIdsNotMatchingHash(e){this.send(30,e)}` (`@4,999,265`) | **Отсутствует (`Unknown(30)`)** |
| `31` | `REQUEST_SOUNDBOARD_SOUNDS` | C → S | `requestSoundboardSounds(e){this.send(31,{guild_ids:e})}` (`@4,999,350`) | **Отсутствует (`Unknown(31)`)** — инициирует `SOUNDBOARD_SOUNDS` (21 событие в `gateway-protocol.json`) |
| `34` | `REQUEST_LAST_MESSAGES` | C → S | `requestLastMessages(e,t){this.send(34,{guild_id:e,channel_ids:t})}` (`@4,999,405`) | **Отсутствует (`Unknown(34)`)** |
| `35` | `SEARCH_RECENT_MEMBERS` | C → S | `searchRecentMembers(e,t){this.send(35,{guild_id:e,query:...,continuation_token:...})}` (`@4,999,455`) | **Отсутствует (`Unknown(35)`)** |
| `37` | `GUILD_SUBSCRIPTIONS_BULK` | C → S | `{subscriptions: {<guild_id>: ...}}` | Реализовано ([`GuildSubscriptionsBulk`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L323-L327)) |
| `38` | `GUILD_CHANNELS_RESYNC` | C → S | `guildChannelsResync(e,t){this.send(38,{guild_id:e,obfuscated_channel_ids:t})}` (`@4,999,560`) | **Отсутствует (`Unknown(38)`)** |
| `39` | `REQUEST_CHANNEL_MEMBER_COUNT` | C → S | `requestChannelMemberCount(e,t){this.send(39,{guild_id:e,channel_id:t})}` (`@4,999,610`) | **Отсутствует (`Unknown(39)`)** |
| `40` | `QOS_HEARTBEAT` | C → S | `{seq, qos: {ver, active, reason}}` | Реализовано ([`QosHeartbeat`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L117-L122)) |
| `41` | `UPDATE_TIME_SPENT_SESSION_ID` | C → S | `{initialization_timestamp, session_id, client_launch_id}` (`@4,999,670`) | **Отсутствует (`Unknown(41)`)** |
| `43` | `REQUEST_CHANNEL_INFO` | C → S | `requestChannelInfo(e,t){this.send(43,{guild_id:e,fields:t})}` (`@4,999,735`) | **Отсутствует (`Unknown(43)`)** — инициирует `CHANNEL_INFO` (7 событий в `gateway-protocol.json`) |

### 1.2. Недостающие поля в исходящем `OP 4 VOICE_STATE_UPDATE`

В [`OutboundVoiceStateUpdate`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L156-L169) и [`OutboundVoiceStateUpdate::to_etf`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L224-L239) кодируются только 6 полей (`guild_id`, `channel_id`, `self_mute`, `self_deaf`, `self_video`, `flags = 2`).
В эталонном методе `voiceStateUpdate(e)` ([discord.com_assets_web.4d4b2f75fdea5470.js](file:///home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js) `@4,998,425`) при непустом `channel_id` дополнительно передаются:
- `preferred_region: Option<String>` и `preferred_regions: Option<Vec<String>>` (при `shouldIncludePreferredRegion`);
- `tracks: Option<Vec<VoiceTrackDescriptor>>` (`{type, rid, quality}`, маппинг из `videoStreamParameters` при включённом видео/стриме).

### 1.3. Критический разрыв фильтрации и редактирования секретов `DISPATCH` в `gui-spike/src/net.rs`

Сверка [`FORWARDED_DISPATCHES`](file:///home/sanya/orca/project/rust/gui-spike/src/net.rs#L130-L161) и [`on_dispatch`](file:///home/sanya/orca/project/rust/gui-spike/src/net.rs#L500-L545) с [`ingest_live_dispatch`](file:///home/sanya/orca/project/rust/gui-spike/src/data.rs#L2612-L2901) выявила два критических расхождения:
1. **Потеря `GUILD_MEMBER_UPDATE`, ролей и получателей групп на входе `net.rs`:**
   - В [`data.rs::ingest_live_dispatch`](file:///home/sanya/orca/project/rust/gui-spike/src/data.rs#L2874-L2894) реализован обработчик `"GUILD_MEMBER_UPDATE"`, но в [`net.rs::FORWARDED_DISPATCHES`](file:///home/sanya/orca/project/rust/gui-spike/src/net.rs#L130-L161) и в ручном списке [`net.rs:516-527`](file:///home/sanya/orca/project/rust/gui-spike/src/net.rs#L516-L527) событие `"GUILD_MEMBER_UPDATE"` **отсутствует**. В результате `net.rs` отбрасывает все живые события `GUILD_MEMBER_UPDATE` (а также `GUILD_ROLE_CREATE`, `GUILD_ROLE_UPDATE`, `GUILD_ROLE_DELETE`, `RELATIONSHIP_UPDATE`, `CHANNEL_RECIPIENT_ADD`, `CHANNEL_RECIPIENT_REMOVE`) как `filtered` до вызова `ingest_live_dispatch` и WebView.
2. **Обход `redact_secrets` и ложное инкрементирование `state.metrics.filtered` в `net.rs:515-537`:**
   - Для 10 событий из списка [`net.rs:516-527`](file:///home/sanya/orca/project/rust/gui-spike/src/net.rs#L516-L527) (`CALL_CREATE`, `CALL_UPDATE`, `CALL_DELETE`, `GUILD_CREATE`, `GUILD_DELETE`, `RELATIONSHIP_ADD`, `RELATIONSHIP_REMOVE`, `STREAM_CREATE`, `STREAM_UPDATE`, `STREAM_DELETE`) `payload` конвертируется `etf_to_json(&event.data)` и отправляется в `channel.deliver` **без вызова `redact_secrets(&mut data)`** (тогда как в `map_dispatch` на строке [`174`](file:///home/sanya/orca/project/rust/gui-spike/src/net.rs#L174) вызывается `redact_secrets`). После этого выполнение проваливается в `map_dispatch(event)`, который возвращает `Ok(None)` и ошибочно увеличивает `state.metrics.filtered += 1` для уже доставленных событий.

### 1.4. Неподдержанные типы `DISPATCH` в `discord-core::state::reducer`

В [gateway-protocol.json](file:///home/sanya/re-deep/gateway-protocol.json#L25-L47) (`opcodes.dispatchTypes`) зафиксирован 21 тип `DISPATCH`. Все 21 вариант присутствуют в [`DispatchKind`](file:///home/sanya/orca/project/rust/crates/discord-core/src/dispatch.rs#L14-L53), однако в [`EventPayload::from_dispatch`](file:///home/sanya/orca/project/rust/crates/discord-core/src/state/types.rs#L540-L611) и [`AppState::apply`](file:///home/sanya/orca/project/rust/crates/discord-core/src/state/reducer.rs#L48-L115) **5 из 21 типов из дампа проваливаются в `EventPayload::Unsupported(kind)`** (`ignored_dispatches += 1`):
1. `MESSAGE_POLL_VOTE_ADD` (`4` кадрa в дампе) — обрабатывается в `gui-spike/src/data.rs`, но не в `discord-core::state::reducer`;
2. `MESSAGE_REACTION_ADD` (`1` кадр в дампе) — обрабатывается в `gui-spike/src/data.rs`, но не в `discord-core::state::reducer`;
3. `SOUNDBOARD_SOUNDS` (`21` кадр в дампе: `{guild_id, soundboard_sounds: [...]}`) — не изменяет `AppState`;
4. `USER_SETTINGS_PROTO_UPDATE` (`3` кадра в дампе: `{settings: {proto, type}, partial}`) — не сохраняется в `AppState`;
5. `VOICE_SERVER_UPDATE` (`3` кадра в дампе: `{token, guild_id, endpoint}`) — не сохраняется в `AppState`.

### 1.5. Недостающие поля `READY`, `READY.guilds[]` и `READY_SUPPLEMENTAL`

Сверка ключей [gateway-protocol.json](file:///home/sanya/re-deep/gateway-protocol.json#L67-L385) с парсерами `discord-core`:

1. **`READY` верхнего уровня ([gateway-protocol.json](file:///home/sanya/re-deep/gateway-protocol.json#L68-L109) — 40 ключей vs [`Ready`](file:///home/sanya/orca/project/rust/crates/discord-core/src/ready/snapshot.rs#L62-L108)):**
   - Извлекается **21 ключ** (`v`, `user`, `session_id`, `resume_gateway_url`, `session_type`, `guilds`, `private_channels`, `relationships`, `read_state`, `user_guild_settings`, `user_settings_proto`, `sessions`, `users`, `presences`, `friend_suggestion_count`, `geo_ordered_rtc_regions`, `country_code`, `api_code_version`, `Heartbeat_interval`, ` required_action`, `merged_members`).
   - Остаются нераспарсенными в `raw: EtfValue` **19 ключей верхнего уровня**:
     `_trace`, `analytics_token`, `apex_experiments`, `auth`, `auth_session_id_hash`, `broadcaster_user_ids`, `connected_accounts`, `consents`, `experiments`, `game_relationships`, `guild_experiments`, `guild_join_requests`, `linked_users`, `notification_settings`, `pending_payments`, `qos_token`, `regional_feature_config`, `static_client_session_id`, `tutorial`.

2. **Элемент `READY.guilds[]` ([gateway-protocol.json](file:///home/sanya/re-deep/gateway-protocol.json#L251-L385) — 19 ключей верхнего уровня + 45 ключей `properties` vs [`Guild`](file:///home/sanya/orca/project/rust/crates/discord-core/src/ready/guild.rs#L17-L52)):**
   - Из 19 ключей верхнего уровня `Guild` парсит 12 и **теряет 6 структурных коллекций гильдии**:
     - `threads` (активные треды гильдии на момент `READY`);
     - `stage_instances` (активные трибуны Stage);
     - `guild_scheduled_events` (запланированные события сервера);
     - `application_command_counts` (счётчики слеш-команд);
     - `channel_settings` (настройки каналов гильдии);
     - `experiments` (эксперименты гильдии).
   - Из 45 ключей вложенного словаря `guild.properties` ([gateway-protocol.json](file:///home/sanya/re-deep/gateway-protocol.json#L276-L322)) `Guild::from_etf` извлекает только **3 поля** (`name`, `icon`, `owner_id`), **отбрасывая 42 поля**, включая критичные для UI: `features`, `description`, `banner`, `splash`, `discovery_splash`, `home_header`, `premium_tier`, `premium_subscription_count`, `vanity_url_code`, `verification_level`, `nsfw_level`, `explicit_content_filter`, `default_message_notifications`, `system_channel_id`, `system_channel_flags`, `rules_channel_id`, `public_updates_channel_id`, `safety_alerts_channel_id`, `preferred_locale`, `max_members`, `max_video_channel_users`, `max_stage_video_channel_users`, `mfa_level`, `afk_channel_id`, `afk_timeout`, `premium_progress_bar_enabled`, `hub_type`, `latest_onboarding_question_id`, `incidents_data`, ` inventory_settings`, `profile`.

3. **`READY_SUPPLEMENTAL` ([gateway-protocol.json](file:///home/sanya/re-deep/gateway-protocol.json#L186-L249) vs [`ReadySupplemental`](file:///home/sanya/orca/project/rust/crates/discord-core/src/state/types.rs#L419-L428)):**
   - Верхний уровень в дампе содержит 7 ключей (`disclose`, `game_invites`, `guilds`, `lazy_private_channels`, `merged_members`, `merged_presences`, `user_activities`), тогда как `ReadySupplemental::from_etf` парсит только 3 (`guilds`, `merged_members`, `merged_presences`), **теряя 4 ключа** (`disclose`, `game_invites`, `lazy_private_channels`, `user_activities`).
   - Элемент `READY_SUPPLEMENTAL.guilds[]` в дампе содержит 4 ключа (`id`, `voice_states`, `embedded_activities`, `activity_instances`), тогда как [` SupplementalGuild`](file:///home/sanya/orca/project/rust/crates/discord-core/src/state/types.rs#L431-L435) парсит только `id` и `voice_states`, **теряя `embedded_activities` и `activity_instances`**.

---

## 2. Сверка голосового протокола (`voice-deep.json` vs `discord-voice`)

### 2.1. Матрица опкодов сигнализации RTC (`VoiceOp` `0..33`)

Сверка [voice-deep.json](file:///home/sanya/re-deep/voice-deep.json#L13-L108), трасс [rtc1-frames.jsonl](file:///home/sanya/re-deep/voice/rtc1-frames.jsonl) / [rtc2-frames.jsonl](file:///home/sanya/re-deep/voice/rtc2-frames.jsonl) и диспетчера `RTCControlSocket` ([discord.com_assets_web.4d4b2f75fdea5470.js](file:///home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js) `@2,396,100..2,405,600`) с [`VoiceOp`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L82-L180) выявила **4 отсутствующих опкода RTC (`Op 14`, `Op 18`, `Op 20`, `Op 33`)**, которые сейчас попадают в `VoiceOp::Unknown(u8)`:

| Опкод | Имя в `RTCControlSocket` | Направление | Полезная нагрузка `d` в оригинале | Статус в `discord-voice` |
|---:|---|---|---|---|
| `0..9`, `11..13`, `15` | Базовые RTC-опкоды | C ↔ S | `IDENTIFY`, `SELECT_PROTOCOL`, `READY`, `HEARTBEAT`, `SESSION_DESCRIPTION`, `SPEAKING`, `HEARTBEAT_ACK`, `RESUME`, `HELLO`, `RESUMED`, `CLIENTS_CONNECT`, `VIDEO`, `CLIENT_DISCONNECT`, `MEDIA_SINK_WANTS` | Реализованы в [`VoiceOp`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L82-L180) (с замечаниями по полям в §2.3–§2.4) |
| `14` | `SESSION_UPDATE` / `CODECS` | **C ↔ S** | S→C (`@2,396,356`): `{audio_codec, video_codec, media_session_id, keyframe_interval}`; C→S (`updateSession` `@2,405,595`): `{"op":14,"d":{"codecs":[...]}}` при fallback видеокодека | **Отсутствует (`VoiceOp::Unknown(14)`)** |
| `16` | `VOICE_BACKEND_VERSION` | **C ↔ S** | C→S (`@2,400,145`): `{"op":16,"d":{}}`; S→C (`rtc1-frames.jsonl:19`): `{"op":16,"d":{"voice":"0.22.1+f01bd52","rtc_worker":"1.6.98"}}` | Только enum-вариант `VoiceOp::VoiceBackendVersion`, **не отправляется и не парсится в `VoiceSessionDriver`** (см. §2.2) |
| `18` | `FLAGS` | S → C | `{"op":18,"d":{"user_id":"...","flags":2}}` (`voice-deep.json` `ops.18`, `@2,396,652`) | **Отсутствует (`VoiceOp::Unknown(18)`)** |
| `20` | `PLATFORM` | S → C | `{"op":20,"d":{"user_id":"...","platform":0}}` (`voice-deep.json` `ops.20`, `@2,396,712`) | **Отсутствует (`VoiceOp::Unknown(20)`)** |
| `21..32` | DAVE E2EE / SecureFrames | C ↔ S | MLS/DAVE (`PrepareTransition`, `ExecuteTransition`, `TransitionReady`, `PrepareEpoch`, `MlsExternalSender` [bin], `MlsKeyPackage` [bin], `MlsProposals` [bin], `MlsCommitWelcome` [bin], `MlsAnnounceCommitTransition` [bin], `MlsWelcome` [bin], `MlsInvalidCommitWelcome`, `Noop`) | Реализованы в [`VoiceOp`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L99-L110) и [`dave.rs`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/dave.rs) |
| `33` | `SFU_UPDATE` / `UDP_ENDPOINT_UPDATE` | S → C | Обработчик `case 33` при `serverVersion >= 10` (`@2,396,162`) | **Отсутствует (`VoiceOp::Unknown(33)`)** |

### 2.2. Пропуск рукопожатия `OP 16 VOICE_BACKEND_VERSION` в `VoiceSessionDriver`

В эталонном клиенте (`RTCControlSocket::handleReady` `@2,400,145`: `this.serverVersion>=6&&this.send(16,{})`) и в обеих живых трассах ([rtc1-frames.jsonl:14,19](file:///home/sanya/re-deep/voice/rtc1-frames.jsonl#L14) и [rtc2-frames.jsonl:13,18](file:///home/sanya/re-deep/voice/rtc2-frames.jsonl#L13)) сразу после получения `OP 2 READY` клиент отправляет `{"op":16,"d":{}}`, а сервер возвращает версии медиа-воркера:
```json
{"op":16,"d":{"voice":"0.22.1+f01bd52","rtc_worker":"1.6.98"}}
```
В [`VoiceSessionDriver::handle_ready`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L1318-L1347) кадр `{"op":16,"d":{}}` **не ставится в `outbound_ws`**, а при получении ответа `OP 16` в [`VoiceSessionDriver::handle_ws_frame`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L1283-L1288) опкод `16` попадает в ветку `_ => *self.unhandled_opcodes.entry(code).or_insert(0) += 1` вместо сохранения версий `voice` / `rtc_worker` для диагностики.

### 2.3. Потеря `rtc_connection_id` и `experiments` при отправке `OP 1 SELECT_PROTOCOL`

В [voice-deep.json](file:///home/sanya/re-deep/voice-deep.json#L28-L35) (`signaling.ops.1`) и [rtc1-frames.jsonl:15](file:///home/sanya/re-deep/voice/rtc1-frames.jsonl#L15) кадр `OP 1 SELECT_PROTOCOL` обязательно включает:
- `"rtc_connection_id": "<uuid-v4>"` (сгенерированный клиентом идентификатор RTC-соединения);
- `"experiments": ["fixed_keyframe_interval"]` (список экспериментов, полученный из `OP 2 READY.experiments`).

Хотя структура [`SelectProtocol`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L521-L529) содержит поля `rtc_connection_id: Option<String>` и `experiments: Vec<String>`, а [`VoiceSessionDriver::handle_ready`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L1341) сохраняет `self.experiments = ready.experiments.clone()`, метод [`VoiceSessionDriver::handle_discovery_response`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L1382-L1390) жёстко передаёт пустые значения:
```rust
let select = SelectProtocol {
    protocol: "udp".to_string(),
    address: discovered.address.clone(),
    port: discovered.port,
    mode,
     codecs: default_opus_codecs(),
    rtc_connection_id: None,      // <-- теряется UUID соединения
    experiments: Vec::new(),      // <-- теряется self.experiments из OP 2 READY
};
```

### 2.4. Инверсия приоритета выбора режима шифрования SRTP

В [voice-deep.json](file:///home/sanya/re-deep/voice-deep.json#L23-L39) (`signaling.ops.2.modes`, `signaling.ops.1`, `signaling.ops.4`, `engine.encryption_mode_selected`) сервер присылает в `OP 2 READY`:
```json
"modes": ["aead_aes256_gcm_rtpsize", "aead_xchacha20_poly1305_rtpsize"]
```
Эталонный клиент (`chooseEncryptionMode` в бандле `@2,391,810`) выбирает **`"aead_aes256_gcm_rtpsize"`** как основной аппаратно-ускоряемый (AES-NI) режим, а `"aead_xchacha20_poly1305_rtpsize"` использует как fallback.
В [`select_encryption_mode`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L526-L539) порядок обратный: сначала выбирается `EncryptionMode::XChaCha20Poly1305RtpSize`, и только при его отсутствии — `EncryptionMode::Aes256GcmRtpSize`.

### 2.5. Недостающие поля в `OP 4 SESSION_DESCRIPTION`, `StreamSpec` и `OP 12 VIDEO`

1. **`OP 4 SESSION_DESCRIPTION` ([`SessionDescription`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L564-L573)):**
   - В `RTCControlSocket` (`case 4` `@2,396,180`) парсятся дополнительные поля, отсутствующие в `SessionDescription`:
     - `keyframe_interval: Option<u64>`
     - `bandwidth_estimation_experiment: Option<String>`
     - `sdp: Option<String>` (используется при `protocol: "webrtc"` вместо `mode`/`secret_key`).
2. **`StreamSpec` ([`lib.rs:264-271`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L264-L271)) vs `VoiceStreamDescriptor` ([`session.rs:733-744`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L733-L744)):**
   - В `lib.rs::StreamSpec` отсутствуют поля `max_bitrate: Option<u32>`, `max_framerate: Option<u32>` и `max_resolution: Option<VideoMaxResolution>`, которые присутствуют в `IDENTIFY.streams` и `OP 12 VIDEO.streams` (`@2,391,900`).
3. **Входящий серверный `OP 12 VIDEO` ([`VoiceVideo`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L718-L895)):**
   - Входящий кадр `OP 12` от сервера (`case 12` `@2,396,420`) содержит `"user_id": "<snowflake>"` и не обязан содержать верхнеуровневый `max_resolution`, тогда как `VoiceVideo::parse` не читает `user_id` и требует обязательный верхнеуровневый объект `max_resolution`, отклоняя серверные `OP 12` уведомления о видеопотоках других участников.

### 2.6. Ограничения `VoiceServerUpdate::parse` для звонков в ЛС и смены эндпоинта

В [`VoiceServerUpdate::parse`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L983-L1024):
- `guild_id` парсится через `nonempty_string(data, "guild_id")`, что приводит к ошибке `MissingField("guild_id")` при звонках в ЛС/групповых чатах, где `guild_id: null` и передаётся `channel_id: Option<String>`;
- `endpoint` парсится через `nonempty_string(data, "endpoint")`, тогда как по контракту Discord при перевыделении голосового сервера приходит `endpoint: null` (ожидание нового сервера без фатальной ошибки парсинга).

---

## 3. Сверка фронтенда и Mana-компонентов (`frontend-d03.json` vs `rust/gui-spike/ui/`)

### 3.1. Экран 1: Поиск (`search` — [frontend-d03.json:12-56](file:///home/sanya/re-deep/frontend-d03.json#L12-L56))

Сверка с [`search.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/search.js), [`search-overlay.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/search-overlay.js) и [`search-panel.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/search-panel.js):
- **Реализовано:**
  - Контейнеры `searchResultsWrap_a98f3b`, `searchResultGroup_a7e67f`, `searchResult__80bf8`, `id="search-result-<id>"`, `menu_c1e9c4`;
  - Все 7 секций фильтров (`От`, `В`, `Содержит`, `Упоминания`, `Дата`, `Тип автора`, `Закреплено`), все 9 опций `containsOptions` (`изображение`, `видео`, `ссылку`, `файл`, `вложение`, `аудио`, `опрос`, `стикер`, `переслать`) и все 3 опции сортировки (`По новизне`, `Самое старое`, `Самое актуальное`).
- **Выявленные расхождения:**
  1. **Кнопки поповера фильтров (`frontend-d03.json:37`):** в дампе у поповера фильтров 3 кнопки `["Очистить фильтры", "Отмена", "Применить фильтры"]`. В `search-overlay.js` отсутствуют кнопки `"Очистить фильтры"` и `"Применить фильтры"`.
  2. **Плейсхолдеры и типы полей фильтров (`frontend-d03.json:27-36` vs [`SEARCH_SECTIONS`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/search-overlay.js#L67-L75)):**
     - `От`: в UI `"Имя автора"` вместо `"Например, pharos12510"`;
     - `В`: в UI `"Канал"` вместо `"Любой контент"`;
     - `Содержит`: в UI `"Тип содержимого"` вместо `"Например, «бот»"`;
     - `Дата`: в UI `<input type="date">` с пустым плейсхолдером вместо кнопки/действия `"Добавить дату"`;
     - `Закреплено`: в UI `control: "switch"` вместо комбобокса `input__75098 comboBoxInput__0edde` с плейсхолдером `"Закреплено, Например, «Да»"`.
  3. **Параметр серверной сортировки «Самое актуальное» ([`REMOTE_SORTS`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/search-overlay.js#L98-L102)):** пункт `recent` («Самое актуальное») отправляет `{ sortBy: "timestamp", sortOrder: "desc" }` вместо `sort_by=relevance&sort_order=desc`.
  4. **DOM-классы из дампа (`frontend-d03.json:22-40`):** отсутствуют классы `editor__25bec`, `container__80bf8`, `filterSection_e4772e`, `input__75098`, `comboBoxInput__0edde`, `flexible_c1e9c4`.

### 3.2. Экран 2: Пикер эмодзи и реакций (`emoji` — [frontend-d03.json:59-94](file:///home/sanya/re-deep/frontend-d03.json#L59-L94))

Сверка с [`expression-picker.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/expression-picker.js), [`emoji.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/emoji.js) и [`reactions-ui.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/reactions-ui.js):
- **Выявленные расхождения:**
  1. **Название вкладки GIF и порядок вкладок (`frontend-d03.json:68` vs [`EXPRESSION_TABS_RU`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/expression-picker.js#L18-L22), [`TAB_ORDER`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/expression-picker.js#L47-L49)):**
     - В дампе вкладки идут в порядке `["Гифки", "Стикеры", "Эмодзи"]`, а в `expression-picker.js` — `["emoji", "gif", "stickers"]` и вкладка названа `"GIF"` вместо `"Гифки"`.
  2. **Плейсхолдер поиска и кнопка `"Добавить эмодзи"` (`frontend-d03.json:69` vs [`expression-picker.js:54`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/expression-picker.js#L54)):**
     - В `expression-picker.js` используется `"Найти идеальный эмодзи..."` вместо точной строки дампа `"Найдите идеальный эмодзи"` (`emoji.js` содержит верную строку, но `expression-picker.js` — нет);
     - Отсутствует кнопка `"Добавить эмодзи"`.
  3. **Секция `"Лучшие эмодзи сервера <name>"` и `aria-label` селектора тона кожи (`frontend-d03.json:70,74,75`):**
     - Отсутствует секция `"Лучшие эмодзи сервера ..."` и категория `"top guild emoji"` в `navRail`;
     - У кнопки выбора тона кожи отсутствует `aria-label="Выбрать цвет кожи эмодзи (Пусто)"`.
  4. **Разделение полного пикера (`expressionPickerPositionLayer__74017`) и «тонкого» пикера реакций (`emojiPicker_c0e32c`, `frontend-d03.json:77-80`):**
     - В дампе пикер реакций (`message-add-reaction`) рендерится с корнем `emojiPicker_c0e32c + bodyWrapper_c0e32c` **без строки поиска и без вкладок**, тогда как в `gui-spike` эти классы (`emojiPicker_c0e32c`, `bodyWrapper_c0e32c`, `listWrapper_c656ac`, `categorySection_c656ac`, `emojiListRow_fc7141`, `emojiItem_fc7141`, `lockedEmoji_d982c2`, `listItems_affa7e`, `categoryItem_b9ee0c`, `categoryPill_b9ee0c`, `navList__08434`) не выставлены.

### 3.3. Экран 3: Список участников (`members` — [frontend-d03.json:96-107](file:///home/sanya/re-deep/frontend-d03.json#L96-L107))

Сверка с [`member-list.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/member-list.js) и [`members.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/members.js):
- **Функционально реализовано:** группировка по hoist-ролям (`NAME — COUNT`, включая `"В сети — N"` и `"Не в сети — N"`), отсутствие поля поиска (`searchField: null`), рендер статуса, короны владельца и кастомного статуса.
- **Выявленные расхождения DOM-классов (`frontend-d03.json:99-103`):**
  - В `member-list.js` и `members.js` используются собственные классы (`.member-row`, `.member-group`), тогда как эталонные селекторы дампа (`membersWrap_c8ffbb`, `hiddenMembers_c8ffbb`, `member__5d473`, `member_c8ffbb`, `container__91a9d`, `clickable__91a9d`, `memberInner__5d473`, `layout__91a9d`, `wrapper__44b0c`, `avatar__44b0c`, `avatarStack__44b0c`, `name__91a9d`, `username__5d473`, `subText__91a9d`) отсутствуют.

### 3.4. Экран 4: Настройки и вкладка «Голос и видео» (`settings` — [frontend-d03.json:109-161](file:///home/sanya/re-deep/frontend-d03.json#L109-L161))

Сверка с [`settings.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/settings.js), [`live-settings-enhancer.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/live-settings-enhancer.js) и [`voice-devices-settings.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/voice-devices-settings.js):
- **Реализовано:** все 19 категорий навигации, все 4 подразделов «Учётная запись», 4 селекта (`Микрофон`, `Динамик`, `Камера`, `Шумоподавление: Krisp`), радиогруппа `Профиль ввода` (`Изоляция голоса`, `Студия`, `Пользовательский`), 38 тумблеров, 3 именованных ползунка (`Громкость микрофона 0..100`, `Громкость динамика 0..200`, `Сила приглушения стрима 1..100`) и все 6 кнопок (`Проверка микрофона`, `Прекратить проверку`, `Проверить видео`, `Загрузить журналы`, `Показать папку`, `Сброс`).
- **Выявленные расхождения строковых меток и атрибутов (`frontend-d03.json:116,127,134,136-148` vs [`settings.js:80-150`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/settings.js#L80-L150)):**
  1. **Корневой класс настроек (`frontend-d03.json:116`):** отсутствует класс `container_abd9a8`.
  2. **Заголовок секции чувствительности (`frontend-d03.json:127` vs [`settings.js:83`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/settings.js#L83)):** в `settings.js` секция названа `"Чувствительность ввода"` вместо `"Настройки чувствительности ввода"`, а также добавлена синтетическая 10-я секция `"Эхо/рация/АРУ"` (в дампе 9 секций).
  3. **Атрибут Mana у радиогруппы «Профиль ввода» (`frontend-d03.json:134` vs [`settings.js:615-625`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/settings.js#L615-L625)):** отсутствует `data-mana-component="BaseRadioGroup"`.
  4. **Расхождение текста 7 тумблеров в [`TOGGLE_SPECS`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/settings.js#L103-L122) относительно точных строк `frontend-d03.json:136-148`:**
     - `"Предлагать устройства"` → в дампе `"Предложение аудиоустройств"`;
     - `"Предупреждать при отсутствии звука"` → в дампе `"Предупреждение об отсутствии звука"`;
     - `"Предупреждать при смене канала"` → в дампе `"Предупреждение о переключении голосового канала"`;
     - `"Приоритет QoS"` → в дампе `"Качество обслуживания"`;
     - `"Расширенная активация"` → в дампе `"Расширенная активация по голосу"`;
     - `"Предпросмотр видео"` → в дампе `"Предпросмотр видео (всегда)"`;
     - `"Предпросмотр стрима"` / `"Приглушать стрим"` → в дампе `"Показывать предпросмотр стрима"` / `"Приглушение стрима"`.
  5. **4-й ползунок порога чувствительности ввода (`frontend-d03.json:155`):** в дампе на вкладке «Голос и видео» 4 ползунка `role="slider"` (4-й — порог ручной чувствительности ввода `min: 0, max: 100, now: 26.11`), тогда как при включённом `Автоматически определять чувствительность ввода` в DOM рендерится только 3 ползунка.

### 3.5. Матрица 12 Mana-компонентов и CSS-токенов (`frontend-d03.json:163-293` vs [`tokens.css`](file:///home/sanya/orca/project/rust/gui-spike/ui/tokens.css))

1. **Атрибуты `data-mana-component`:**
   - В UI используется только `data-mana-component="layer-modal"` (в [`qr-auth-modal.js:627`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/qr-auth-modal.js#L627)).
   - Отсутствуют атрибуты `data-mana-component="button"`, `data-mana-component="text-area"`, `data-mana-component="modal"`, `data-mana-component="popover"`, `data-mana-component="mini-premium-tooltip"`, `data-mana-component="BaseRadioGroup"`.
2. **Недостающие CSS-токены Mana в [`tokens.css`](file:///home/sanya/orca/project/rust/gui-spike/ui/tokens.css) (21 токен из `frontend-d03.json`):**
   - **Button (5):** `--button-border-radius` (`2147483647px`), `--button-border-radius-lg`, `--button-danger-background-disabled`, `--button-outline-brand-border-active`, `--button-outline-primary-text`;
   - **TextInput / TextArea (4):** `--input-icon-default`, `--input-placeholder-text-default`, `--input-border-error-default`, `--input-text-error-default`;
   - **Modal (1):** `--modal-footer-background`;
   - **Switch (5):** `--switch-background-selected-default`, `--switch-border-default`, `--switch-border-selected-hover`, `--switch-thumb-background-default`, `--switch-thumb-icon-active`;
   - **Slider (1):** `--slider-track-background`;
   - **Badge (3):** `--badge-expressive-text`, `--badge-epic-border-gradient-end`, `--badge-mythic-background`;
   - **Scroller (3):** `--scrollbar-thin-track`, `--scrollbar-auto-thumb`, `--scrollbar-auto-track`.

---

## 4. Сводный приоритизированный реестр расхождений (`RE-GAP Backlog` для PM)

| ID | Приоритет | Отдел | Оракул | Описание расхождения | Целевые файлы в `discord-lab` |
|---|---|---|---|---|---|
| `GAP-768-01` | **P1** | `G1 Backend` | `gateway-protocol.json` | В `net.rs` отсутствуют `GUILD_MEMBER_UPDATE`, `GUILD_ROLE_*`, `RELATIONSHIP_UPDATE`, `CHANNEL_RECIPIENT_*` в `FORWARDED_DISPATCHES`; 10 событий в `net.rs:516-527` обходят `redact_secrets` и инкрементируют `metrics.filtered` | [`rust/gui-spike/src/net.rs`](file:///home/sanya/orca/project/rust/gui-spike/src/net.rs#L130-L161), [`net.rs:515-537`](file:///home/sanya/orca/project/rust/gui-spike/src/net.rs#L515-L537) |
| `GAP-768-02` | **P1** | `G5 Voice` | `voice-deep.json`, `rtc1-frames.jsonl` | `VoiceSessionDriver` не отправляет `{"op":16,"d":{}}` после `OP 2 READY` и считает входящий `OP 16 VOICE_BACKEND_VERSION` (`{voice, rtc_worker}`) необработанным опкодом | [`rust/crates/discord-voice/src/session.rs`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L1283-L1347) |
| `GAP-768-03` | **P1** | `G5 Voice` | `voice-deep.json`, `rtc1-frames.jsonl` | `VoiceSessionDriver::handle_discovery_response` передаёт `rtc_connection_id: None` и `experiments: Vec::new()` в `OP 1 SELECT_PROTOCOL` вместо UUID и `self.experiments` из `OP 2 READY` | [`rust/crates/discord-voice/src/session.rs`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L1382-L1390) |
| `GAP-768-04` | **P1** | `G5 Voice` | `voice-deep.json` | `select_encryption_mode` ставит `aead_xchacha20_poly1305_rtpsize` выше `aead_aes256_gcm_rtpsize`, расходясь с выбором эталонного клиента | [`rust/crates/discord-voice/src/session.rs`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L526-L539) |
| `GAP-768-05` | **P1** | `G5 Voice` | `voice-deep.json`, Web Bundle | В `VoiceOp` отсутствуют активные RTC-опкоды `Op 14` (`SESSION_UPDATE`/`CODECS`), `Op 18` (`FLAGS`), `Op 20` (`PLATFORM`), `Op 33` (`SFU_UPDATE`) | [`rust/crates/discord-voice/src/lib.rs`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L82-L180) |
| `GAP-768-06` | **P1** | `G5 Voice` | `voice-deep.json`, Web Bundle | `VoiceServerUpdate::parse` требует непустой `guild_id` и `endpoint`, ломаясь на звонках в ЛС (`guild_id: null`, `channel_id`) и перевыделении сервера (`endpoint: null`) | [`rust/crates/discord-voice/src/lib.rs`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L983-L1024) |
| `GAP-768-07` | **P2** | `G5 Voice` | `voice-deep.json`, Web Bundle | В `SessionDescription` (`OP 4`) нет `keyframe_interval`, `bandwidth_estimation_experiment`, `sdp`; в `StreamSpec` нет `max_bitrate`/`max_framerate`/`max_resolution`; в `VoiceVideo` (`OP 12`) нет входящего `user_id` | [`rust/crates/discord-voice/src/lib.rs`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L264-L271), [`lib.rs:564-573`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/lib.rs#L564-L573), [`session.rs:718-895`](file:///home/sanya/orca/project/rust/crates/discord-voice/src/session.rs#L718-L895) |
| `GAP-768-08` | **P2** | `G1 Backend` | `gateway-protocol.json`, Web Bundle | В `GatewayOpcode` отсутствуют 13 клиентских опкодов (`5, 13, 24, 28, 29, 30, 31, 34, 35, 38, 39, 41, 43`), включая `13 CALL_CONNECT`, `31 REQUEST_SOUNDBOARD_SOUNDS`, `43 REQUEST_CHANNEL_INFO` | [`rust/crates/discord-core/src/gateway.rs`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L17-L88) |
| `GAP-768-09` | **P2** | `G1 Backend` | `gateway-protocol.json`, Web Bundle | В `OutboundVoiceStateUpdate` (`OP 4`) отсутствуют опциональные поля `preferred_region`, `preferred_regions` и `tracks` | [`rust/crates/discord-core/src/gateway.rs`](file:///home/sanya/orca/project/rust/crates/discord-core/src/gateway.rs#L156-L169) |
| `GAP-768-10` | **P2** | `G1 Backend` | `gateway-protocol.json` | 5 типов `DISPATCH` из дампа (`MESSAGE_POLL_VOTE_ADD`, `MESSAGE_REACTION_ADD`, `SOUNDBOARD_SOUNDS`, `USER_SETTINGS_PROTO_UPDATE`, `VOICE_SERVER_UPDATE`) возвращают `EventPayload::Unsupported` в `discord-core::state::reducer` | [`rust/crates/discord-core/src/state/types.rs`](file:///home/sanya/orca/project/rust/crates/discord-core/src/state/types.rs#L540-L611), [`reducer.rs`](file:///home/sanya/orca/project/rust/crates/discord-core/src/state/reducer.rs#L48-L115) |
| `GAP-768-11` | **P2** | `G1 Backend` | `gateway-protocol.json` | `Guild::from_etf` теряет 6 коллекций (`threads`, `stage_instances`, `guild_scheduled_events`, `application_command_counts`, `channel_settings`, `experiments`) и 42 из 45 ключей `guild.properties` (`features`, `banner`, `premium_tier`, `vanity_url_code` и др.) | [`rust/crates/discord-core/src/ready/guild.rs`](file:///home/sanya/orca/project/rust/crates/discord-core/src/ready/guild.rs#L17-L95) |
| `GAP-768-12` | **P2** | `G1 Backend` | `gateway-protocol.json` | `ReadySupplemental::from_etf` теряет 4 ключа верхнего уровня (`disclose`, `game_invites`, `lazy_private_channels`, `user_activities`) и 2 ключа гильдии (`embedded_activities`, `activity_instances`); `Ready` не парсит 19 дополнительных ключей | [`rust/crates/discord-core/src/state/types.rs`](file:///home/sanya/orca/project/rust/crates/discord-core/src/state/types.rs#L419-L435), [`ready/snapshot.rs`](file:///home/sanya/orca/project/rust/crates/discord-core/src/ready/snapshot.rs#L62-L108) |
| `GAP-768-13` | **P2** | `G2 Frontend` | `frontend-d03.json` | В `search-overlay.js` сортировка «Самое актуальное» (`recent`) отправляет `sortBy: "timestamp"` вместо `"relevance"`, отсутствуют кнопки `"Очистить фильтры"` / `"Применить фильтры"` и эталонные плейсхолдеры 5 полей | [`rust/gui-spike/ui/features/search-overlay.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/search-overlay.js#L67-L102) |
| `GAP-768-14` | **P2** | `G2 Frontend` | `frontend-d03.json` | В `expression-picker.js` вкладка названа `"GIF"` вместо `"Гифки"`, порядок вкладок `["Эмодзи","GIF","Стикеры"]` вместо `["Гифки","Стикеры","Эмодзи"]`, нет кнопки `"Добавить эмодзи"`, секции `"Лучшие эмодзи сервера ..."` и `aria-label="Выбрать цвет кожи эмодзи (Пусто)"` | [`rust/gui-spike/ui/features/expression-picker.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/expression-picker.js#L18-L55) |
| `GAP-768-15` | **P2** | `G2 Frontend` | `frontend-d03.json` | В `settings.js` секция названа `"Чувствительность ввода"` вместо `"Настройки чувствительности ввода"`, подписи 7 тумблеров сокращены относительно дампа, у `Профиль ввода` нет `data-mana-component="BaseRadioGroup"` | [`rust/gui-spike/ui/features/settings.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/settings.js#L80-L150), [`settings.js:615-625`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/settings.js#L615-L625) |
| `GAP-768-16` | **P3** | `G3 Design` / `G2` | `frontend-d03.json` | В `tokens.css` отсутствует 21 CSS-токен Mana (`--button-border-radius`, `--input-border-error-default`, `--switch-background-selected-default`, `--slider-track-background`, `--badge-mythic-background`, `--scrollbar-auto-thumb` и др.) и атрибуты `data-mana-component` на 6 примитивах | [`rust/gui-spike/ui/tokens.css`](file:///home/sanya/orca/project/rust/gui-spike/ui/tokens.css), [`rust/gui-spike/ui/components/primitives.css`](file:///home/sanya/orca/project/rust/gui-spike/ui/components/primitives.css) |
| `GAP-768-17` | **P3** | `G2 Frontend` | `frontend-d03.json` | В `member-list.js` и `reactions-ui.js` отсутствуют эталонные CSS-классы дампа (`membersWrap_c8ffbb`, `member__5d473`, `avatarStack__44b0c`, `subText__91a9d`, `emojiPicker_c0e32c`, `bodyWrapper_c0e32c`) | [`rust/gui-spike/ui/features/member-list.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/member-list.js), [`rust/gui-spike/ui/features/reactions-ui.js`](file:///home/sanya/orca/project/rust/gui-spike/ui/features/reactions-ui.js) |
