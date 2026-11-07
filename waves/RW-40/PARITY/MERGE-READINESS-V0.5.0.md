# MERGE-READINESS-V0.5.0 — Комплексная матрица паритета Discord Client v0.5.0 (Wave 42 / Спринты 1–13)

- **Задача:** `TASK-632` (`Parity Matrix Wave 42 & Release Notes v0.5.0`)
- **Отдел:** `G6 Tech Writer / Parity`
- **Дата снимка:** `2026-11-07`
- **Проверяемые сборки:** `BUILD-004` (`Sprints 1–5`), `BUILD-005` (`Sprints 6–10`, `TASK-610..TASK-622`), `BUILD-006` (`Sprints 11–13`, `TASK-623..TASK-630`)
- **Эталонный RE-оракул:** `/home/sanya/re-deep/` (`rest-d02.md`, `frontend-d03.md`, `voice-deep.md`, `gateway-schema.md`, `decoded-index.json`, `backend-ipc.md`, `native-re.md`, `ru-catalog.json`)
- **Статус готовности к релизу:** **`READY FOR BETA & RELEASE v0.5.0 (P0 = 0, P1 = 0, 53/54 Implemented, 15/15 + 5/5 Mutations Killed)`**

---

## 0. Методология сверки и оракулы `/home/sanya/re-deep/`

Каждая подсистема нашего нативного клиента (`discord-lab` / `discord-gui-spike` на базе Rust + Tauri 2) сверена с артефактами обратного инжиниринга эталонного клиента Discord 1.0.162/1.0.166 (Linux x64, Electron 42):

1. **REST API v9 (`/home/sanya/re-deep/rest-d02.md`, `rest-static.json`, `decoded-index.json`):** 47 живых эндпоинтов из `d02-snapshot/capture-ready.snap.jsonl` и `capture-run2.snap.jsonl`, заголовки `X-Super-Properties` (22 поля base64 JSON), `X-Discord-Locale`, `X-Discord-Timezone`, коды HTTP `200/201/202/204/304/400/401/403/404/413/429/5xx` и JSON-ошибки Discord (`10038`, `30001`, `30005`, `40002`, `40003`, `50035`, `50050`..`50097`).
2. **Gateway ETF/zstd (`/home/sanya/re-deep/gateway-schema.md`, `gateway-protocol.md`):** 124 бинарных кадра `capture-ready.jsonl` (`0x83` ETF `VERSION_MAGIC`, 10 тегов ETF включая `0x6e SMALL_BIG_EXT` для 64-битных Snowflake, потоковый `zstd`, `READY`, `RESUMED`, 30 типов `DISPATCH`, опкоды стриминга `18..=22`).
3. **Voice, RTP/RTCP & DAVE MLS E2EE (`/home/sanya/re-deep/voice-deep.md`, `native-re.md`):** RTC WebSocket `v=8/9` (`op 0/1/2/3/4/6/8/15/16` + DAVE MLS `op 21..31`), 74-байтный UDP IP-discovery, 8-байтный keepalive (`0x1337cafe`/`0x1337f00d`), RTP `PT=120` Opus и видеокодеки `AV1 (PT=101)` / `H.264 (PT=105)` с расширением RFC 5285 `0xBEDE`, режимы `aead_aes256_gcm_rtpsize` и `aead_xchacha20_poly1305_rtpsize`, двухслойный криптоконвейер `libdave` (`MlsKeyRatchet`, `"Discord Secure Frames v0"`, магический маркер `0xFAFA`, 30-значный `voice_privacy_code`).
4. **WebView UI & Design System (`/home/sanya/re-deep/frontend-d03.md`, `ru-catalog.json`, `decoded-index.json`):** DOM-контракты поиска (7 секций фильтров, 9 опций `has:`, 3 режима сортировки), пикеров выражений и подарков Nitro, дерева каналов, контекстных меню с ползунком громкости `0..200%`, модалки `hCaptcha`, мастера создания серверов, плеера Go Live (`Stream Viewer` + `PiP`) и расширенных настроек (`Keybinds`, `Soundboard`, `Clips`).
5. **Platform & Local RPC IPC (`/home/sanya/re-deep/backend-ipc.md`, `main-arch.md`):** 168 каналов `DISCORD_*`, системное хранилище секретов `libsecret`, D-Bus уведомления и `org.freedesktop.portal.ScreenCast` (PipeWire) с X11 MIT-SHM fallback, протоколы `discord://` и `disclip://` (верификация `DISCORD_UUID` в MP4 + HTTP Range `200/206/416`), локальный сокет `discord-ipc-0..9`.

---

## 1. Voice

| ID | Модуль / Фича | Эталон (`/home/sanya/re-deep/`) | Реализация (v0.5.0) | Контракт / Тесты | Статус паритета |
|---|---|---|---|---|---|
| PM-VOICE-01 | RTC WebSocket сигнализация (`op 0/1/2/3/4/6/8/15/16`) и выбор региона | `/home/sanya/re-deep/voice-deep.md` §1 (`rtc1-frames.jsonl`, heartbeat `13750 мс`, `Select Protocol`) | `rust/crates/discord-voice/src/session.rs`, `rust/crates/discord-voice/src/regions.rs` | `rust/crates/discord-voice/tests/signaling.rs`, `rust/crates/discord-voice/tests/regions_contract.rs` | Implemented (Offline Verified) |
| PM-VOICE-02 | UDP IP-Discovery (74 Б), Keepalive (`0x1337cafe`, 5 с) и ротация сокета (+60 с) | `/home/sanya/re-deep/voice-deep.md` §2.1–§2.2 (`voice-udp3.pcap`, `strace-renderer-s3.log`) | `rust/crates/discord-voice/src/udp.rs`, `rust/crates/discord-voice/src/reconnect.rs` | `rust/crates/discord-voice/tests/udp.rs`, `rust/crates/discord-voice/tests/reconnect_contract.rs` | Implemented (Offline Verified) |
| PM-VOICE-03 | SRTP / SRTCP (`aead_aes256_gcm_rtpsize`, `aead_xchacha20_poly1305_rtpsize`, `0xBEDE`) | `/home/sanya/re-deep/voice-deep.md` §2.3–§2.4 (`PT=120`, 16B tag + 4B LE counter, `SR=200`/`RR=201`) | `rust/crates/discord-voice/src/srtp.rs`, `rust/crates/discord-voice/src/rtcp.rs` | `rust/crates/discord-voice/tests/srtp_contract.rs`, `rust/crates/discord-voice/tests/rtcp_contract.rs` | Implemented (Offline Verified) |
| PM-VOICE-04 | **TASK-623**: Криптоконвейер DAVE MLS v2 (`DAVE MLS E2EE v2`, `MlsKeyRatchet`, Proposal/Commit/Welcome, `0xFAFA`) | `/home/sanya/re-deep/voice-deep.md` §1, `native-re.md` (`mls_key_ratchet.cpp`, `"Discord Secure Frames v0"`) | `rust/crates/discord-voice/src/dave.rs`, `rust/crates/discord-voice/src/srtp.rs` | `rust/crates/discord-voice/tests/srtp_contract.rs`, `rust/crates/discord-voice/tests/crypto_vectors.rs` | Implemented (Offline Verified) |
| PM-VOICE-05 | **TASK-615**: Пакетизатор видео RTP H.264 (`Single NALU`, `STAP-A`, `FU-A`) и AV1 OBU (`Z/Y/W/N`, MTU 1200) | `/home/sanya/re-deep/voice-deep.md` §1 (`PT=101 AV1`, `PT=105 H264`, `RTX PT=102/106`, 90 кГц) | `rust/crates/discord-voice/src/rtp/video.rs`, `rust/crates/discord-voice/src/media/rtp.rs` | `rust/crates/discord-voice/tests/rtp_video_contract.rs`, `rust/crates/discord-voice/tests/media_contract.rs` | Implemented (Offline Verified) |
| PM-VOICE-06 | Джиттер-буфер аудио, DTX и сборщик видеокадров `VideoFrameAssembler` | `/home/sanya/re-deep/voice-deep.md` §2.3 (`voice-udp4.pcap`, каденция 20 мс / 960 сэмплов @48 кГц) | `rust/crates/discord-voice/src/media/jitter.rs`, `rust/crates/discord-voice/src/reception.rs` | `rust/crates/discord-voice/tests/media_contract.rs`, `rust/crates/discord-voice/tests/reception_contract.rs` | Implemented (Offline Verified) |
| PM-VOICE-07 | Аудиоустройства ввода/вывода, профили и горячая замена (`hot-plug`, fallback) | `/home/sanya/re-deep/frontend-d03.md` §4 (`ui8/23-settings-voice.json`, `backend-ipc.md`) | `rust/crates/discord-voice/src/devices.rs`, `rust/gui-spike/src/voice_devices.rs` | `rust/crates/discord-voice/tests/devices_contract.rs`, `rust/gui-spike/tests/voice_devices_contract.rs` | Implemented (Offline Verified) |
| PM-VOICE-08 | WebRTC Voice Diagnostics (`PingHistory` 199/20, `ConnectionQuality`, 30-значный Privacy Code) | `/home/sanya/re-deep/voice-deep.md` §4, `decoded-index.json` (модуль `613681` `RTCConnectionPopout`) | `rust/crates/discord-voice/src/stats.rs`, `rust/gui-spike/src/voice_diagnostics.rs` | `rust/crates/discord-voice/tests/stats_contract.rs`, `tests/unit/voice-ui.test.mjs` | Implemented (Offline Verified) |

---

## 2. REST

| ID | Модуль / Фича | Эталон (`/home/sanya/re-deep/`) | Реализация (v0.5.0) | Контракт / Тесты | Статус паритета |
|---|---|---|---|---|---|
| PM-REST-01 | HTTP/1.1 транспорт, `X-Super-Properties` (22 поля), rate-limit `429` и идемпотентный retry | `/home/sanya/re-deep/rest-d02.md` §0.1, §2–§6 (`capture-run2.snap.jsonl`, `rest-static.json`) | `rust/crates/discord-core/src/rest/client.rs`, `rust/crates/discord-core/src/rest/super_properties.rs` | `rust/crates/discord-core/tests/rest_client.rs`, `rust/crates/discord-core/tests/rest_super_properties.rs` | Implemented (Offline Verified) |
| PM-REST-02 | **TASK-611**: Создание гильдии `POST /api/v9/guilds` (шаблоны, иконка base64, коды `30005`/`40002`) | `/home/sanya/re-deep/rest-d02.md` §6, `decoded-index.json` (`Endpoints.GUILDS = "/guilds"`) | `rust/crates/discord-core/src/rest/guilds.rs`, `rust/gui-spike/src/guilds.rs` | `rust/gui-spike/tests/guild_create_contract.rs`, `rust/gui-spike/tests/guilds_contract.rs` | Implemented (Offline Verified) |
| PM-REST-03 | **TASK-617**: Полнотекстовый поиск `GET /guilds/{id}/messages/search` (`202` retry, `429`, LRU-кэш) | `/home/sanya/re-deep/frontend-d03.md` §1.1–§1.4 (`20f-search-network.json`), `rest-d02.md` | `rust/crates/discord-core/src/rest/search.rs`, `rust/gui-spike/src/search.rs` | `rust/gui-spike/tests/search_contract.rs`, `tests/unit/search-remote-real-index.test.mjs` | Implemented (Offline Verified) |
| PM-REST-04 | **TASK-626**: Nitro Gifts & Entitlements (`resolve/create/revoke/redeem`, 10 планов, лимит 3 кода) | `/home/sanya/re-deep/rest-d02.md`, `decoded-index.json` (модули `869038`, `75255`, `202541`, `45938`) | `rust/gui-spike/src/gifts.rs`, `rust/gui-spike/src/main.rs` | `rust/gui-spike/tests/gifts_contract.rs`, `tests/e2e/e2e-build006.test.mjs` | Implemented (Offline Verified) |
| PM-REST-05 | Сообщения (история, отправка, reply, edit, delete) и пустое тело `PUT` реакций (`Content-Length: 0`) | `/home/sanya/re-deep/rest-d02.md` §3 (`messages`, `reactions/@me`, `ack`), `decoded-index.json` | `rust/gui-spike/src/messages.rs`, `rust/gui-spike/src/reactions.rs` | `rust/gui-spike/tests/messages_contract.rs`, `rust/gui-spike/tests/reactions_contract.rs` | Implemented (Offline Verified) |
| PM-REST-06 | Инвайты (`resolve_invite`, `accept_invite` с `captcha_key`/`rqtoken`, `create_channel_invite`, lurker `204`) | `/home/sanya/re-deep/rest-d02.md` §0.1 (`PUT guilds/{id}/members/@me`), `decoded-index.json` | `rust/gui-spike/src/invites.rs`, `rust/gui-spike/src/guilds.rs` | `rust/gui-spike/tests/invites_contract.rs`, `rust/gui-spike/tests/server_join_and_subscriptions_contract.rs` | Implemented (Offline Verified) |
| PM-REST-07 | Вложения (`POST /channels/{id}/attachments`, слоты загрузки, лимиты 20/50/100 МиБ по Nitro-тиру) | `/home/sanya/re-deep/rest-d02.md`, `decoded-index.json` (`Math.max(0x1400000, tier)`) | `rust/gui-spike/src/attachments.rs`, `rust/gui-spike/ui/features/composer.js` | `rust/gui-spike/tests/attachments_contract.rs`, `tests/unit/composer.test.mjs` | Implemented (Offline Verified) |
| PM-REST-08 | Единый BFF-роутер доменов (`slash_commands`, `onboarding`, `soundboard`, `stage_instances`, `webhooks`, `automod`) | `/home/sanya/re-deep/rest-d02.md`, `decoded-index.json` (маршруты `/api/v9/*`) | `rust/gui-spike/src/bff_domains_router.rs`, `rust/gui-spike/src/soundboard.rs` | `rust/gui-spike/tests/ipc_wiring_contract.rs`, `rust/gui-spike/tests/webhooks_contract.rs` | Implemented (Offline Verified) |

---

## 3. Gateway

| ID | Модуль / Фича | Эталон (`/home/sanya/re-deep/`) | Реализация (v0.5.0) | Контракт / Тесты | Статус паритета |
|---|---|---|---|---|---|
| PM-GW-01 | Потоковый декомпрессор `zstd` и декодер Erlang ETF (`0x83`, 10 тегов, `SMALL_BIG_EXT` 64-bit Snowflake) | `/home/sanya/re-deep/gateway-schema.md` §1.1–§1.4 (124 кадра, `149212 → 883526 Б`, `0x6e`) | `rust/crates/discord-codec/src/etf.rs`, `rust/crates/discord-codec/src/zstd.rs` | `rust/crates/discord-codec/tests/etf_vectors.rs`, `rust/crates/discord-codec/tests/zstd_stream.rs` | Implemented (Offline Verified) |
| PM-GW-02 | Жизненный цикл сессии шлюза (`HELLO op:10`, `IDENTIFY op:2`, `HEARTBEAT op:1/11`, `RESUME op:6`) | `/home/sanya/re-deep/gateway-schema.md` §1.2, `gateway-protocol.md` (`002-READY.json`, `RESUMED`) | `rust/crates/discord-core/src/identify.rs`, `rust/crates/discord-core/src/resume.rs` | `rust/crates/discord-core/tests/identify_ready.rs`, `rust/crates/discord-core/tests/resume_reconnect.rs` | Implemented (Offline Verified) |
| PM-GW-03 | Парсер снимка `READY` (гильдии, каналы, `read_state`, `merged_members`, `relationships`, настройки) | `/home/sanya/re-deep/gateway-schema.md` §2 (`READY` 577 124 Б, 7 гильдий) | `rust/crates/discord-core/src/ready.rs`, `rust/crates/discord-core/src/ready/snapshot.rs` | `rust/crates/discord-core/tests/ready_core.rs`, `rust/crates/discord-core/tests/ready_read_state.rs` | Implemented (Offline Verified) |
| PM-GW-04 | Маршрутизатор `DISPATCH` (30 событий `FORWARDED_KINDS`) и реактивный мост `gateway://dispatch` | `/home/sanya/re-deep/gateway-schema.md` §3 (`MESSAGE_*`, `PRESENCE_UPDATE`, `GUILD_MEMBER_LIST_UPDATE`) | `rust/crates/discord-core/src/dispatch.rs`, `rust/gui-spike/ui/features/dispatch-bridge.js` | `rust/crates/discord-core/tests/dispatch_router.rs`, `tests/unit/dispatch_ui.test.mjs` | Implemented (Offline Verified) |
| PM-GW-05 | Опкоды видеостриминга Go Live (`18 STREAM_CREATE` .. `22 STREAM_SET_PAUSED` и серверные `STREAM_*`) | `/home/sanya/re-deep/gateway-schema.md`, `voice-deep.md` (`guild:{gid}:{cid}:{uid}`) | `rust/crates/discord-core/src/state/voice/mod.rs`, `rust/gui-spike/src/voice_stage.rs` | `rust/crates/discord-core/tests/stream_gateway_contract.rs`, `rust/gui-spike/tests/voice_stage_contract.rs` | Implemented (Offline Verified) |
| PM-GW-06 | **TASK-619**: Движок битовых прав Discord v9 (`Permissions Bitfield Engine`, 41 флаг `0..=40`, канальные overwrites) | `/home/sanya/re-deep/gateway-schema.md` (`roles`, `permission_overwrites`), `decoded-index.json` | `rust/crates/discord-core/src/permissions.rs`, `rust/gui-spike/src/permissions.rs` | `rust/crates/discord-core/tests/permissions_resolution_contract.rs`, `tests/unit/role-permissions.test.mjs` | Implemented (Offline Verified) |

---

## 4. UI

| ID | Модуль / Фича | Эталон (`/home/sanya/re-deep/`) | Реализация (v0.5.0) | Контракт / Тесты | Статус паритета |
|---|---|---|---|---|---|
| PM-UI-01 | Каркас приложения (4 экрана, адаптивные брейкпоинты `1600/1200/960/720`, оконная модель `PageStore` ≤500 строк) | `/home/sanya/re-deep/frontend-d03.md` (`ui/tokens.json`), `gateway-schema.md` | `rust/gui-spike/ui/index.html`, `rust/gui-spike/ui/model.js` | `tests/unit/gui-spike-dom.test.mjs`, `tests/meta/design-breakpoints.test.mjs` | Implemented (Offline Verified) |
| PM-UI-02 | **TASK-610**: Интерактивная модалка решения капчи `#captcha-modal` (перехват HTTP 400 `code: 40003`, повтор `accept_invite`) | `/home/sanya/re-deep/frontend-d03.md`, `decoded-index.json` (`captcha_sitekey`, `captcha_rqtoken`, `captcha_rqdata`) | `rust/gui-spike/ui/features/captcha-modal.js`, `rust/gui-spike/ui/features/join-server.js` | `tests/unit/captcha-modal.test.mjs`, `tests/e2e/e2e-build006.test.mjs` | Implemented (Offline Verified) |
| PM-UI-03 | **TASK-612**: Пошаговый мастер создания сервера `#create-guild-dialog` (шаблоны, предпросмотр base64-иконки, A11y) | `/home/sanya/re-deep/frontend-d03.md`, `decoded-index.json` (`guild_template_code`, `CREATE_GUILD`) | `rust/gui-spike/ui/features/create-guild.js`, `rust/gui-spike/ui/styles/create-guild.css` | `tests/unit/create-guild-wizard.test.mjs`, `tests/e2e/e2e-build006.test.mjs` | Implemented (Offline Verified) |
| PM-UI-04 | **TASK-616**: Плеер стримов Go Live (`Stream Viewer`, `720p/1080p`, `30/60 FPS`, `PiP` и бейдж `DAVE MLS E2EE`) | `/home/sanya/re-deep/voice-deep.md` §3, `frontend-d03.md` (`voice/ui7/`) | `rust/gui-spike/ui/features/stream-viewer.js`, `rust/gui-spike/ui/styles/stream-viewer.css` | `tests/unit/stream-viewer-ui.test.mjs`, `tests/e2e/e2e-build006.test.mjs` | Implemented (Offline Verified) |
| PM-UI-05 | **TASK-618**: Всплывающее окно поиска `#search-popover` (7 токенов `from:`/`mentions:`/`has:`/`in:`, подсветка, пагинация) | `/home/sanya/re-deep/frontend-d03.md` §1.1–§1.3 (`20b-search-filters.json`, 9 опций `has:`) | `rust/gui-spike/ui/features/search.js`, `rust/gui-spike/ui/features/search-overlay.js` | `tests/unit/search-popover.test.mjs`, `tests/e2e/e2e-build006.test.mjs` | Implemented (Offline Verified) |
| PM-UI-06 | **TASK-620**: Унифицированное контекстное меню ПКМ (`context-menu.js`, слайдер громкости `0..200%`, кламп координат) | `/home/sanya/re-deep/frontend-d03.md`, `decoded-index.json` (`user-volume` `0..200%`, `role="menu"`) | `rust/gui-spike/ui/features/context-menu.js`, `rust/gui-spike/ui/styles/context-menu.css` | `tests/unit/context-menu.test.mjs`, `tests/e2e/e2e-build006.test.mjs` | Implemented (Offline Verified) |
| PM-UI-07 | **TASK-627**: Модальное окно выбора и кастомизации подарков `Nitro Gifts` (`#gift-modal`, 12 стилей открыток, лимит 200 симв.) | `/home/sanya/re-deep/frontend-d03.md`, `decoded-index.json` (модули `202541`, `45938`, `GiftStyle 0..=2`) | `rust/gui-spike/ui/features/gift-modal.js`, `rust/gui-spike/ui/styles/gift-modal.css` | `tests/unit/gift-modal.test.mjs`, `tests/e2e/e2e-build006.test.mjs` | Implemented (Offline Verified) |
| PM-UI-08 | **TASK-628**: Расширенные настройки (`Advanced Settings`: `Keybinds` 19 действий, `Soundboard` 0..100%, `Clips` 30/60/120с) | `/home/sanya/re-deep/frontend-d03.md` §4 (`ui8/23-settings-voice.json`), `decoded-index.json` (`994778`) | `rust/gui-spike/ui/features/advanced-settings.js`, `rust/gui-spike/ui/styles/advanced-settings.css` | `tests/unit/advanced-settings.test.mjs`, `tests/e2e/e2e-build006.test.mjs` | Implemented (Offline Verified) |
| PM-UI-09 | Рендерер сообщений: AST Markdown, спойлеры, таймстемпы `<t:UNIX:STYLE>`, пилюли реакций и полка вложений | `/home/sanya/re-deep/frontend-d03.md` §2–§3, `gateway-schema.md` | `rust/gui-spike/ui/features/markdown.js`, `rust/gui-spike/ui/features/chat.js` | `tests/unit/markdown-helper.test.mjs`, `tests/unit/chat-reactions.test.mjs` | Implemented (Offline Verified) |
| PM-UI-10 | Интеграционные мосты оболочки (`app-shell-wiring.js`, `composer-popovers-bridge.js`, пины/ветки/входящие/профиль) | `/home/sanya/re-deep/frontend-d03.md` §3 (`ui5/04` профиль, модальные слои) | `rust/gui-spike/ui/features/app-shell-wiring.js`, `rust/gui-spike/ui/features/composer-popovers-bridge.js` | `tests/unit/shell-bridge.test.mjs`, `tests/unit/pins-threads.test.mjs` | Implemented (Offline Verified) |

---

## 5. Platform

| ID | Модуль / Фича | Эталон (`/home/sanya/re-deep/`) | Реализация (v0.5.0) | Контракт / Тесты | Статус паритета |
|---|---|---|---|---|---|
| PM-PLAT-01 | **TASK-625**: Обработчик Deep Links `discord://` (инвайты, подарки, профили, квесты, OAuth2) и `disclip://` (MP4 UUID + HTTP Range) | `/home/sanya/re-deep/backend-ipc.md`, `decoded-index.json` (модули `901123`, `7635`, `1331` `BLOCKED_URL_PROTOCOLS`) | `rust/crates/discord-platform/src/deep_link.rs` | `rust/crates/discord-platform/tests/deep_link_contract.rs` | Implemented (Offline Verified) |
| PM-PLAT-02 | Системное хранилище секретов `libsecret` FFI и миграция зашифрованных токенов `safeStorage` | `/home/sanya/re-deep/backend-ipc.md` (`SAFE_STORAGE_*`), `main-arch.md` | `rust/crates/discord-platform/src/keyring.rs`, `rust/crates/discord-platform/src/ffi_secret.rs` | `rust/crates/discord-platform/tests/keyring_offline_contract.rs`, `tests/meta/keyring-live.test.mjs` | Implemented (Offline Verified) |
| PM-PLAT-03 | Нативные системные уведомления D-Bus (`org.freedesktop.Notifications`, `a{sv}` hints, abstract socket) | `/home/sanya/re-deep/backend-ipc.md` (`NOTIFICATIONS_SEND_NOTIFICATION`, `core/#7592`) | `rust/crates/discord-platform/src/notifications.rs`, `rust/crates/discord-platform/src/dbus/mod.rs` | `rust/crates/discord-platform/tests/notifications_contract.rs`, `rust/crates/discord-platform/tests/dbus_contract.rs` | Implemented (Offline Verified) |
| PM-PLAT-04 | Управление питанием, детектор простоя (`idle`) и блокировка сна (`logind` inhibitor через `SCM_RIGHTS` fd-passing) | `/home/sanya/re-deep/backend-ipc.md` (`POWER_MONITOR_*`, `POWER_SAVE_BLOCKER_*`, `core/#4531`) | `rust/crates/discord-platform/src/power.rs`, `rust/crates/discord-platform/src/dbus/fd.rs` | `rust/crates/discord-platform/tests/power_contract.rs`, `rust/crates/discord-platform/tests/dbus_contract.rs` | Implemented (Offline Verified) |
| PM-PLAT-05 | Системный трей (`StatusNotifierItem`), автозапуск XDG `.desktop` и проверка орфографии `hunspell` FFI | `/home/sanya/re-deep/backend-ipc.md` (`SYSTEM_TRAY_*`, `TOGGLE_OPEN_ON_STARTUP`, `SPELLCHECK_*`) | `rust/crates/discord-platform/src/tray.rs`, `rust/crates/discord-platform/src/spellcheck.rs` | `rust/crates/discord-platform/tests/tray_contract.rs`, `rust/crates/discord-platform/tests/spellcheck_contract.rs` | Implemented (Offline Verified) |
| PM-PLAT-06 | **TASK-621 / TASK-622 / TASK-630**: Headless `Xvfb` E2E раннер, 7 браузерных сьютов M2 + Build 006 и 15/15 мутационный гейт | `/home/sanya/re-deep/frontend-d03.md` (CDP E2E-сценарии) | `scripts/gate.mjs`, `tests/e2e/e2e-build006.test.mjs` | `tests/meta/mutations.test.mjs`, `tests/meta/ci-browser.test.mjs` | Implemented (Verified) |

---

## 6. RPC

| ID | Модуль / Фича | Эталон (`/home/sanya/re-deep/`) | Реализация (v0.5.0) | Контракт / Тесты | Статус паритета |
|---|---|---|---|---|---|
| PM-RPC-01 | **TASK-624**: Локальный IPC-сокет `discord-ipc-0..9` (`/run/user/1000`, права `0o600`, `listen(511)`, Windows pipes) | `/home/sanya/re-deep/voice-deep.md` (`strace-renderer-s3.log:14744`), `backend-ipc.md` | `rust/crates/discord-platform/src/rpc_ipc.rs` | `rust/crates/discord-platform/tests/rpc_ipc_contract.rs` | Implemented (Offline Verified) |
| PM-RPC-02 | Бинарный фрейминг Discord RPC (`[opcode: u32 LE][length: u32 LE]`, лимит `16 KiB`, опкоды `0=HANDSHAKE..4=PONG`) | `/home/sanya/re-deep/backend-ipc.md`, `decoded-index.json` (`RPC_PROTOCOL_VERSION = 1`) | `rust/crates/discord-platform/src/rpc_ipc.rs` | `rust/crates/discord-platform/tests/rpc_ipc_contract.rs` | Implemented (Offline Verified) |
| PM-RPC-03 | Валидация рукопожатия (`v=1`, `client_id` Snowflake) и команда `SET_ACTIVITY` (Rich Presence, до 2 кнопок, 128 симв.) | `/home/sanya/re-deep/gateway-schema.md` (`activities`), `decoded-index.json` | `rust/crates/discord-platform/src/rpc_ipc.rs`, `rust/gui-spike/ui/features/presence-activity.js` | `rust/crates/discord-platform/tests/rpc_ipc_contract.rs`, `tests/unit/presence-activity.test.mjs` | Implemented (Offline Verified) |
| PM-RPC-04 | Команды RPC `INVITE_BROWSER`, `GUILD_TEMPLATE_BROWSER`, `SUBSCRIBE`/`UNSUBSCRIBE` и пул до 8 клиентов | `/home/sanya/re-deep/backend-ipc.md`, `decoded-index.json` (`MAX_RPC_CLIENTS = 8`) | `rust/crates/discord-platform/src/rpc_ipc.rs` | `rust/crates/discord-platform/tests/rpc_ipc_contract.rs` | Implemented (Offline Verified) |
| PM-RPC-05 | Безопасность Tauri IPC моста (38+ команд, запрет передачи `PathBuf`/`File` из WebView, единый `token_or`) | `/home/sanya/re-deep/backend-ipc.md` (`securityUtils`), `main-arch.md` | `rust/gui-spike/src/main.rs`, `rust/gui-spike/src/ipc_ext.rs` | `tests/meta/ipc-path.test.mjs`, `rust/gui-spike/tests/ipc_contract.rs` | Implemented (Verified) |

---

## 7. Media

| ID | Модуль / Фича | Эталон (`/home/sanya/re-deep/`) | Реализация (v0.5.0) | Контракт / Тесты | Статус паритета |
|---|---|---|---|---|---|
| PM-MEDIA-01 | **TASK-613**: Захват экрана и окон через D-Bus `org.freedesktop.portal.ScreenCast` (PipeWire/Wayland, режимы курсора и токенов) | `/home/sanya/re-deep/native-re.md` (`BaseCapturerPipeWire`, `ScreenCast`), `backend-ipc.md` | `rust/crates/discord-media/src/capture/portal.rs`, `rust/crates/discord-platform/src/screencast.rs` | `rust/crates/discord-media/tests/capture_contract.rs`, `rust/crates/discord-platform/tests/screencast_contract.rs` | Implemented (Offline Verified) |
| PM-MEDIA-02 | **TASK-613**: Резервный захват X11 `MIT-SHM` (`XShmGetImage` / `XServerPixelBuffer`) и геометрия мониторов XRandR | `/home/sanya/re-deep/native-re.md` (`ScreenCapturerX11`, `WindowCapturerX11`, `"falling back to X11"`) | `rust/crates/discord-media/src/capture/x11.rs`, `rust/crates/discord-media/src/capture/sources.rs` | `rust/crates/discord-media/tests/capture_contract.rs` | Implemented (Offline Verified) |
| PM-MEDIA-03 | **TASK-614**: Неблокирующий атомарный кольцевой буфер 60 FPS `LatestFrameSlot` (`zero-copy view`, защита от джиттера) | `/home/sanya/re-deep/voice-deep.md` §3 (`1280x720@30/60`), `native-re.md` (`getNextVideoOutputFrame`) | `rust/crates/discord-media/src/buffer.rs`, `rust/crates/discord-media/src/lib.rs` | `rust/crates/discord-media/tests/buffer_contract.rs`, `tests/meta/mutations.test.mjs` | Implemented (Offline Verified) |
| PM-MEDIA-04 | Конвертация пиксельных форматов PipeWire (`BGRA`/`RGBA`/`BGRx` → `I420`), кроппинг (`CropRect`) и контроль stride | `/home/sanya/re-deep/native-re.md` (`FrameBuffer`, `WumpusVideoSource`) | `rust/crates/discord-media/src/capture/portal.rs`, `rust/crates/discord-media/src/lib.rs` | `rust/crates/discord-media/tests/capture_contract.rs`, `rust/crates/discord-media/tests/buffer_contract.rs` | Implemented (Offline Verified) |
| PM-MEDIA-05 | Рендеринг медиа-вложений в чате (аудиосообщения `IS_VOICE_MESSAGE` с декодированием `waveform`, изображения, видео, опросы) | `/home/sanya/re-deep/frontend-d03.md`, `gateway-schema.md` (`attachments`, `waveform`) | `rust/gui-spike/ui/features/attachments.js`, `rust/gui-spike/ui/features/poll-card.js` | `tests/unit/attachments-render.test.mjs`, `tests/unit/message-media-polls.test.mjs` | Implemented (Offline Verified) |

---

## 8. Auth

| ID | Модуль / Фича | Эталон (`/home/sanya/re-deep/`) | Реализация (v0.5.0) | Контракт / Тесты | Статус паритета |
|---|---|---|---|---|---|
| PM-AUTH-01 | Вход по логину/паролю (`POST /api/v9/auth/login`) и прямому токену с сохранением исключительно в системном `keyring` | `/home/sanya/re-deep/rest-d02.md`, `decoded-index.json` (`/auth/login`, `SAFE_STORAGE`) | `rust/gui-spike/src/auth.rs`, `rust/gui-spike/ui/features/auth.js` | `rust/gui-spike/tests/auth_contract.rs`, `tests/unit/gui-spike-auth.test.mjs` | Implemented (Offline Verified) |
| PM-AUTH-02 | Многофакторная аутентификация MFA (5 методов: `TOTP`, `SMS`, `Backup`, `WebAuthn`, `Password`, `POST /auth/mfa/*`) | `/home/sanya/re-deep/rest-d02.md`, `decoded-index.json` (`mfa_ticket`, `auth.authenticator_types`) | `rust/gui-spike/src/auth.rs`, `rust/gui-spike/ui/features/auth.js` | `rust/gui-spike/tests/auth_contract.rs`, `tests/unit/gui-spike-auth.test.mjs` | Implemented (Offline Verified) |
| PM-AUTH-03 | Обработка Captcha-гейта при входе и инвайтах (`X-Captcha-Key`, `X-Captcha-Rqtoken`, `X-Captcha-Session-Id`) | `/home/sanya/re-deep/rest-d02.md`, `decoded-index.json` (`captcha_sitekey`, `captcha_rqdata`) | `rust/gui-spike/src/auth.rs`, `rust/gui-spike/ui/features/captcha-modal.js` | `rust/gui-spike/tests/auth_contract.rs`, `tests/unit/captcha-modal.test.mjs` | Implemented (Offline Verified) |
| PM-AUTH-04 | Восстановление сессии при холодном старте (`auth_status`), инвалидация токена при `401` и чистый `logout` | `/home/sanya/re-deep/rest-d02.md`, `backend-ipc.md` (`auth://state`, очистка кэша и keyring) | `rust/gui-spike/src/auth.rs`, `rust/gui-spike/src/net.rs` | `rust/gui-spike/tests/auth_contract.rs`, `rust/gui-spike/tests/net_contract.rs` | Implemented (Offline Verified) |
| PM-AUTH-05 | **TASK-629**: Вход по QR-коду (`RemoteAuth` WebSocket `wss://remote-auth-gateway.discord.gg/?v=2`, RSA-OAEP-256) | `/home/sanya/re-deep/decoded-index.json` (`REMOTE_AUTH`, `init`, `nonce_proof`, `pending_remote_init`) | `rust/gui-spike/src/auth.rs`, `rust/gui-spike/ui/features/auth.js` | `rust/gui-spike/tests/auth_contract.rs`, `tests/unit/gui-spike-auth.test.mjs` | Partial (In Progress) |
| PM-AUTH-06 | Защита секретов и редактирование токенов/паролей/ключей во всех логах (`<redacted>`, `P0-03` инвариант) | `/home/sanya/re-deep/voice-deep.md` §0, `rest-d02.md` §1 (маскирование `Authorization` и `secret_key`) | `rust/crates/discord-core/src/rest/headers.rs`, `rust/gui-spike/src/auth.rs` | `rust/gui-spike/tests/auth_contract.rs`, `tests/meta/e2e-required.test.mjs` | Implemented (Verified) |

---

## 9. Сводная статистика паритета

Агрегированные показатели по всем 8 модульным доменам матрицы паритета `v0.5.0`:

| Код домена | Домен / Подсистема | Всего позиций | Implemented | Partial | Покрытие домена |
|---|---|---:|---:|---:|---:|
| `VOICE` | Голосовой тракт, RTP/RTCP, видеокодеки и DAVE MLS E2EE v2 | 8 | 8 | 0 | 100.0% |
| `REST` | HTTP/1.1 клиент, гильдии, поиск, подарки Nitro, инвайты и BFF | 8 | 8 | 0 | 100.0% |
| `GW` | Gateway ETF/zstd, READY, Dispatch (30 событий), Go Live op 18..22 и права v9 | 6 | 6 | 0 | 100.0% |
| `UI` | WebView интерфейс, модалки, плеер стримов, поиск, меню ПКМ и настройки | 10 | 10 | 0 | 100.0% |
| `PLAT` | Интеграция с ОС Linux (Deep Links, Keyring, D-Bus, Power, Tray, Xvfb CI) | 6 | 6 | 0 | 100.0% |
| `RPC` | Локальный IPC-сокет `discord-ipc-0..9`, Rich Presence и защита Tauri IPC | 5 | 5 | 0 | 100.0% |
| `MEDIA` | PipeWire ScreenCast, X11 MIT-SHM fallback, `LatestFrameSlot` 60 FPS | 5 | 5 | 0 | 100.0% |
| `AUTH` | Пароль, MFA (5 методов), Captcha, Keyring Session, QR RemoteAuth, Redaction | 6 | 5 | 1 | 83.3% |
| **Итого** | **Все 8 доменов клиента v0.5.0** | **54** | **53** | **1** | **98.1%** |

---

## 10. Готовность к релизу v0.5.0 (Merge Readiness Verdict)

1. **Полнота реализации Спринтов 1–13 (`TASK-601..TASK-630`):**
   - Все ключевые подсистемы сборок `BUILD-004`, `BUILD-005` и `BUILD-006` реализованы, интегрированы в `discord-core`, `discord-voice`, `discord-media`, `discord-platform` и `discord-gui-spike`, и сверены с эталонным RE-дампом `/home/sanya/re-deep/`.
   - Единственная позиция в статусе `Partial (In Progress)` — `PM-AUTH-05` (`TASK-629` / `TASK-634`: вход по QR-коду через `RemoteAuth` WebSocket), которая разрабатывается в изолированной ветке `feature/task-629-remote-auth-be` и не блокирует основной контур авторизации по логину/паролю/токену/MFA.
2. **Показатели контура качества:**
   - **TypeScript (`tsc`):** `0` ошибок (`tsconfig.json`, `tsconfig.agent.json`, `rust/gui-spike/ui/tsconfig.json`).
   - **Мета-тесты (`tests/meta/`):** `100% PASS` (включая `parity-matrix.test.mjs`, `mutations.test.mjs`, `plan-sync.test.mjs`, `test-policy.test.mjs`, `ipc-path.test.mjs`).
   - **Модульные тесты UI (`tests/unit/`):** `1901+` тестов (`0 fail`).
   - **Контрактные и модульные тесты Rust (`workspace` + `gui-spike`):** `1390+` тестов воркспейса + `41` контрактный сьют `gui-spike` (`0 fail`).
   - **Браузерные E2E в Headless Xvfb (`tests/e2e/`):** 7 сценариев M2 + 7 сквозных сценариев Build 005–006 в `tests/e2e/e2e-build006.test.mjs` (`100% PASS`).
   - **Мутационный гейт:** `15/15` в `tests/meta/mutations.test.mjs` + `5/5` на каждом функциональном и документационном срезе (`100% KILLED`).
3. **Итоговый вердикт G6 Tech Writer / Parity:**
   - Блокирующих замечаний нет (`P0 = 0`, `P1 = 0`, `P2 = 0`).
   - Документация и матрица паритета `v0.5.0` полностью синхронизированы с кодовой базой и тестовыми оракулами. Сборка готова к предрелизной валидации в `beta` и выпуску релиза `v0.5.0`.
