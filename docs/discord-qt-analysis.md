# discord-qt ↔ RE Discord ↔ Rust-клиент: что реализовано, что нет и чего не хватает в нашем проекте

> Копия отчёта в репозитории `discord-lab` (`docs/discord-qt-analysis.md`).
> Перенос контрактов и недостающих функций — `docs/feature-backlog.md` (волны R4–R6/D).

Дата: 19.10.2026
Источник кода: клон `https://github.com/ruslang02/discord-qt` (ветка `master`) → `/tmp/opencode/discord-qt`
Наши RE-артефакты: `/tmp/opencode/r2-net/*`, `/tmp/opencode/ux-research/*`

---

## 0. Резюме

1. **discord-qt — это не форк, а самостоятельный клиент на TypeScript**: NodeGUI (Qt Widgets) + `discord.js v12.5` + 16 monkey-патчей; ≈13,4 тыс. строк (128 TS + 32 SCSS), 41 иконка, локали de/en/ru по ~165 строк.
2. **Проект фактически заморожен**: последний релиз v0.5.1 — 10.01.2021, последняя активность в master — 25.01.2023, 210★, 39 открытых issues. Лицензия GPL-3.0 (код копировать нельзя; идеи и контракты — можно).
3. **Он решает те же «узкие места», что вскрыло наше RE**: discord.js — библиотека для ботов, поэтому user-специфика дописана вручную: `READY`-поля (`user_settings`, `user_guild_settings`, `private_channels`, `notes`, `read_state`), событие `MESSAGE_ACK` и REST-ack с токеном, заметки (`users/@me/notes/:id`), настройки (`users/@me/settings`), кастомный статус (activity type 4), позиции/мьют серверов, lazy-guild-подписки (op 14).
4. **Чего в нём нет** (его «необработанные функции», часть — висящие issues): UI реакций, треды/форумы, стикеры, поиск, ответы (replies), групповые ЛС, страница друзей, видео/демонстрация экрана/стримы, DAVE/E2EE, AEAD-режимы шифрования голоса (только `xsalsa20_poly1305*`), Krisp (вместо него примитивный RMS-гейт), звуковая панель, автодополнение упоминаний, микрофон-mute/deafen вообще отсутствуют.
5. **Вывод для нашего Rust-клиента**: discord-qt полезен как **карта контрактов** «что нужно для user-опыта поверх bot-API», но его функциональное покрытие — начало 2021 года. Наши RE-артефакты уже шире: ETF+zstd, таблица тегов/опкодов, 88 REST-эндпоинтов v9, крипто-режимы 0–6, 5132 design-токена, RU-локаль (26 754 сообщения), 137 SVG, 15 звуков, IPC-инвентарь (~150 каналов).
6. **Юридика не меняется**: сценарий A (свой мессенджер с UX «по мотивам») — GO; сценарий C (клиент к реальному Discord под user-токеном) — юридический NO-GO (ToS). Сам discord-qt прямо пишет об этом в README.

---

## 1. Паспорт проекта

| Параметр | Значение |
|---|---|
| Репозиторий | `ruslang02/discord-qt`, описание: «Discord client powered by Node.JS and Qt Widgets» |
| Стек | Node.js ≥10, NodeGUI `@nodegui/nodegui ^0.26` (Qt Widgets через Node), SCSS → Qt Style Sheets, webpack 5 |
| Ядро протокола | `discord.js ^12.5.1` (+ 16 monkey-патчей) |
| Голос | discord.js voice + `prism-media`, `opusscript 0.0.7` (JS/WASM Opus), `tweetnacl` (xsalsa20), `ffmpeg-static`/`ffplay-static`, PulseAudio (`pactl`) на Linux |
| UI-ассеты | Material Design Icons (41 PNG), Source Sans Pro (Whitney — опционально вручную, README) |
| Размер | 128 TS-файлов; TS+SCSS ≈13 444 строк; 32 SCSS |
| Локализация | `i18n`, de-DE/en-US/ru-RU, по ~165–169 строк на язык |
| Статус | релиз v0.5.1 (10.01.2021); push 25.01.2023; 210★ / 10 форков / 39 открытых issues; не архивный |
| Лицензия | GPL-3.0 |
| UA | спуф официального Electron-клиента эпохи: `...discord-qt/0.5.1 Chrome/78.0.3904.130 Electron/7.3.2 Safari/537.36` |
| Авторизация | только пользовательский токен (без логина/пароля); мультиаккаунт + «auto login»; токен хранится в конфиге |
| Плагины | npm-пакеты с keywords `discord-qt`+`plugin` из `~/.config/discord-qt/plugins`; получают `{app, Qt, createLogger, Discord, module}` (пример: `mpris-dqt-plugin`) |

---

## 2. Архитектура

### 2.1 Слои

```
launch.js → (qode) src/index.ts
   ├─ import './patches'          — 16 патчей discord.js (см. 2.2)
   ├─ i18n (de/en/ru) → new Application()
   └─ Application.start():
        ├─ шрифты из assets/fonts, ConfigManager (~/.config/discord-qt)
        ├─ MainWindow (QWidget) + OverlayWindow (X11-оверлей)
        ├─ Tray (аккаунты, оверлей, открыть/выйти)
        ├─ PluginManager.reload()
        └─ ClientManager.load(account) → new Client(clientOptions).login(token)
             ├─ события discord.js → UI (компоненты подписываются на app/Events)
             ├─ уведомления → node-notifier (иконка автора через PictureWorker)
             └─ голос → VoiceProvider (ffmpeg/pulse) + discord.js voice
  worker/ (worker_threads): PictureWorker — загрузка/кэш аватаров и вложений (PNG, roundify) 
```

Ключевые опции клиента (`src/utilities/ClientOptions.ts`):

| Параметр | Значение | Комментарий |
|---|---|---|
| REST | `http.version = 8` | официальный клиент 2026 — `api/v9` |
| Gateway | `ws.version = 7`, `compress: false` | официальный 2026 — `v=9`, `encoding=etf&compress=zstd-stream` |
| Properties | `browser: 'DiscordQt'`, `os`, `os_arch`, `release_channel: stable`, `client_build_number` из webpack-define (`__BUILDNUM__ || 0`) | build number подставляется вручную при сборке |

### 2.2 Патчи discord.js (16) — «чего не хватает стандартной библиотеке»

Порядок загрузки: `Constants → NodeAddon → NodeWidget → Prism → SecretBox → StreamDispatcher → TextBasedChannel → ClientUser → ClientPresence → User → Guild → GuildChannel → WebSocketShard → Handlers → RESTManager → ActionsManager`.

| Патч | Что делает | Зачем (вывод для нас) |
|---|---|---|
| `Constants` | карты user-настроек (`UserSettingsMap`), `MessageNotificationTypes`, `ExplicitContentFilterTypes`, «дружеские источники», UA-спуф | discord.js не знает user-схем настроек |
| `NodeAddon`, `NodeWidget` | Proxy-обёртка Qt-объектов: вызов метода у `destroyed` виджета → noop вместо краха | платформенный хак NodeGUI, к протоколу не относится |
| `Prism` | подмена opus-энкодера prism-media на `opusscript` (`Loader.require`) | нет нативного libopus в сборке |
| `SecretBox` | `secretbox.methods = tweetnacl.secretbox{,.open}` | **шифрование только xsalsa20_poly1305*** |
| `StreamDispatcher` | безопасная отправка UDP-пакетов (нет сокета → debug, а не краш) | устойчивость голоса |
| `TextBasedChannel` | `lastReadMessageID`, `acknowledged`, `acknowledge()` → `POST /channels/:id/messages/:lastId/ack {token}`, хранение `_ackToken` | у bot-либы нет read-state |
| `ClientUser` | `email`, `premium`, `customStatus`, `setCustomStatus()`, `acceptInvite(code)` → `POST /invite/:code` + ожидание `GUILD_CREATE` (таймаут 120 с) | user-поля профиля и инвайты |
| `ClientPresence` | `_parse` → activities: кастомный статус (type 4) + игра; emit `presenceUpdate` | у discord.js v12 «custom status» не парсился |
| `User` | `note` (get), `setNote()` → `PUT /users/@me/notes/:id` | заметки о пользователях |
| `Guild` | `acknowledged`, `position` (из `guild_positions`), `muted`, `messageNotifications`, `mobilePush`, `suppressEveryone`, `subscribeToTypingEvent()` → **op 14** `{guild_id, typing:true, activities:true}` | guild-settings + ленивые подписки |
| `GuildChannel` | `muted`/`messageNotifications` из `channel_overrides`, `can(flags)` | per-channel оверрайды |
| `WebSocketShard` | `checkReady()` → мгновенный `ALL_READY` (заглушка в READY-хендлере) | обход bot-логики «ждём все гильдии» |
| `Handlers` | кастомный `READY` (см. ниже), `MESSAGE_ACK`, `USER_GUILD_SETTINGS_UPDATE`, `USER_NOTE_UPDATE`, `USER_SETTINGS_UPDATE` | user-события шлюза |
| `RESTManager` | `getAuth()` из `client.token` | auth для user-токена |
| `ActionsManager` (+`Actions`) | actions: `MESSAGE_ACK`, `USER_NOTE_UPDATE`, `USER_SETTINGS_UPDATE` (парсинг + события) | пользовательские события |

**Кастомный `READY` (важнейшее место)** — discord.js v12 это не парсит, discord-qt дописывает:
`user_settings` → `ClientUserSettings`; `user_guild_settings[]` → `ClientUserGuildSettings` (+`channel_overrides`); `private_channels[]` → в менеджер каналов; `notes` → коллекция; `read_state[]` (`{id, mention_count, last_message_id}`) → `client.read_state`; в конце — `shard.checkReady()`.

**Сопоставление с нашим захватом шлюза (R3):** в захвате есть `USER_SETTINGS_PROTO_UPDATE`, `VOICE_STATE_UPDATE`, `PRESENCE_UPDATE`, `GUILD_MEMBER_LIST_UPDATE` (51 шт.), `SESSIONS_REPLACE`, `CHANNEL_INFO`, `MESSAGE_POLL_VOTE_ADD/REMOVE` — то есть современный клиент отдаёт ещё и **proto-настройки** (`USER_SETTINGS_PROTO_UPDATE`) и события, которых в 2021 не было. Схема READY у discord-qt — хороший черновик, но для актуального клиента её надо сверять (см. §6.1, пункт «READY/GUILD_CREATE с нуля»).

---

## 3. Инвентарь функций discord-qt

### 3.1 Протокол/данные (что реально вызывается)

| Область | Реализация |
|---|---|
| Шлюз | discord.js v12: JSON, без сжатия, v7; resume/heartbeat — внутренние; кастомно: op 14 (typing/activities-подписка), парсинг user-событий (см. 2.2) |
| REST (кастомные вызовы) | `PATCH /users/@me/settings`; `PUT /users/@me/notes/:id`; `POST /channels/:id/messages/:id/ack`; `POST /invite/:code`; user guild settings — `rest.methods.patchClientUserGuildSettings()` (в discord.js 12.5.3 такого метода **нет** — путь записи серверных настроек, судя по коду, мёртв; в UI `.update()` не вызывается) |
| REST (через discord.js) | отправка/редактирование/удаление/загрузка сообщений и вложений, typing, загрузка истории, гильдии/участники/роли, presence через op 3, присоединение к голосовому каналу (voice state) |
| Сообщения | `markdown-it` (отключены `hr/blockquote/lheading/list`), эмодзи-рендер (twemoji для юникод-эмодзи), вложения, эмбеды (author/title/description/fields/image/thumbnail), инвайт-карточки, упоминания (рендер; автодополнение — нет), редактирование inline, удаление, контекстное меню: Edit / Quote / Delete / Copy text / Copy source / Copy message link / Copy ID |
| Read-state | непрочитанное для гильдий/каналов (гuild.acknowledged, `MESSAGE_ACK`), `mention_count` приходит в `read_state` |
| Профиль | `ProfilePopup` (MiniProfile, роли, presence, заметка), `UserMenu` (Profile / Mention / Send DM / Mute / Change nickname / Invite to server / Copy ID), смена ника, кастомный статус (диалог) |
| Настройки пользователя | 6 страниц: Accounts (мультиаккаунт, auto-login), Account (username/email, 2FA-заглушка «недоступно»), Appearance (тема, язык, зум, чекбоксы markdown/аватары/roundify/debug), System (minimize to tray, очистка кэша), Voice (устройства, проверка, NR-слайдер), Overlay (вкл/выкл, позиция, кейбинд) |

### 3.2 UI-экраны и компоненты

| Группа | Состав |
|---|---|
| Каркас | `MainWindow` (+ drawer), `MainTitleBar` (иконки канала/группы), `LeftPanel`, `MainView` |
| Серверы/каналы | `GuildsList` (иконки + индикатор непрочитанного), `GuildPanel` (категории, каналы, `ChannelButton` с mute-видом, `ChannelMembers` — участники голосового канала), `ChannelsListMenu` (практически только «Copy ID»), `GuildActionsMenu` |
| Личные сообщения | `DMPanel` (`DMUsersList`, `DMTitleBar`) |
| Чат | `MessagesPanel` (группировка, `MessageItem`, `MessageUtilities`: mentions/markdown/embeds/invites/attachments), `InputPanel` (ввод, цитата, эмодзи, вложения-панель, typing-индикатор), `EmojiPicker` (поиск, недавние, серверные эмодзи) |
| Участники/профили | `MembersList`, `ProfilePopup` (роли, статус), `UserPanel`/`UserButton` (ник, инвайт, шестерёнка настроек) |
| Голос | `VoicePanel` (статусы Connecting/Connected/Reconnecting/Error, disconnect, инфо-надпись); **нет** кнопок mute/deafen; «шумодав» — слайдер чувствительности RMS-гейта |
| Оверлей | `OverlayWindow` (X11: `BypassWindowManagerHint`, прозрачность, не принимает фокус, всегда сверху) + список участников голосового канала; позиция/кейбинд из настроек |
| Диалоги | AcceptInvite, ConfirmLeaveGuild, CustomStatus, NicknameChange |
| Прочее | Tray (аккаунты/switch, оверлей, open, quit), уведомления `node-notifier`, темы dark/light/amoled (SCSS), локали де/ан/ру |

### 3.3 Голосовой стек

| Часть | Реализация discord-qt |
|---|---|
| Захват | `ffmpeg -f pulse -i {device} -f s16le -ar 48000 -ac 2 -` → Opus (`opusscript`) |
| Воспроизведение | Opus → PCM → `ffmpeg/ffplay -f pulse ...` |
| Устройства | Linux: `pactl list sources/sinks short`; win/darwin — свои провайдеры (ffplay/ffmpeg статически) |
| Шифрование | `tweetnacl` через патч SecretBox → discord.js v12 поддерживает только **3 режима**: `xsalsa20_poly1305`, `..._suffix`, `..._lite` (исходники 12.5.3: `StreamDispatcher.js`, строки 257–267) |
| Шумоподавление | `NoiseReductor` — RMS-гейт: тихие чанки заменяются нулями, эмитит speaking/unmuting | 
| Passthrough | `Silence` — поток Opus-«тишины» `[0xf8, 0xff, 0xfe]` |
| Организация | `VoiceProviderManager` по ОС; провайдеры: Linux (pulse), Win32, Darwin; `VoiceOptions` — шаблоны аргументов ffmpeg |
| Ограничения | Голос только Linux/ядерные платформы (issue «Voice support on Windows and macOS»), нет AEC/AGC/Krisp, нет видео/стрима, нет DAVE |

---

## 4. Чего в discord-qt НЕТ

### 4.1 По открытым issue (выборка, 39 всего)

«Search bar» · «Replying to messages» · «Group DMs support» · «@ mentions nonfunctional in 0.5.1» · «Voice support on Windows and macOS» · «No notifications on Windows» · «Notifications on new messages» · «Option to change date format» · «Smooth scrolling» · «is this no longer being maintained».

### 4.2 По коду (проверено)

| Отсутствует | Как проверено |
|---|---|
| UI реакций (ставит/показывает) | в `src/components` нет ни одного упоминания reaction; настройка `render_reactions` парсится, но не используется |
| Ответы (replies), ветки (threads), форумы | нет соответствующих компонентов/полей |
| Стикеры, GIF-пикер, звуковая панель (soundboard) | нет |
| Поиск (Quick Switcher, поиск по сообщениям) | нет; `MainTitleBar` — только иконки |
| Групповые ЛС, страница «Друзья» (заявки/список) | нет; `DMPanel` — только личные диалоги |
| Реакции/события сервера (Guild Scheduled Events), стейдж-каналы | нет |
| Видео/камера/демонстрация экрана/Go Live/просмотр стримов | нет (`join` голосового канала — максимум) |
| DAVE/E2EE (2024+) | нет (эпоха 2021) |
| AEAD-режимы шифрования (`aead_aes256_gcm*`, `aead_xchacha20*`) | нет — только xsalsa (патч SecretBox/StreamDispatcher) |
| Krisp/ML-шумодав, AEC/AGC (WebRTC APM) | нет; RMS-гейт вместо |
| Mute микрофона / Deafen (кнопки) | в коде нет `selfMute`/`setDeaf`; `microphone-settings.png` ведёт в настройки |
| Автодополнение упоминаний/каналов/эмодзи в вводе | нет (эмодзи — через пикер) |
| Папки серверов | `guildFolders` парсится, UI папок нет |
| Позиции/уведомления сервера на запись | путь `ClientUserGuildSettings.update()` не вызывается и опирается на несуществующий метод discord.js 12.5.3 |
| Proto-настройки (`USER_SETTINGS_PROTO_UPDATE`) | код 2021 г. их не знает |
| Мобильный/compact-режим | настройки «isMobile» есть, полноценного режима нет |

---

## 5. Сопоставление: официальный клиент (наши RE-данные) ↔ discord-qt

### 5.1 По слоям

| Слой | Официальный клиент (наши RE-факты) | discord-qt | Что делать Rust-клиенту |
|---|---|---|---|
| Шлюз | `wss://gateway.../?encoding=etf&v=9&compress=zstd-stream`; ETF с первого кадра; zstd-stream; QoS-пульс `op 40 {seq, qos:{ver:31}}`; `op 11`; RESUME/RESUMED; опкоды/теги — `REPORT.md §11` | JSON, без сжатия, v7; `op 40/41` не знает; resume — библиотечный | ETF+zstd (спека есть); учесть QoS-пульс и современные диспатчи (SESSIONS_REPLACE, CHANNEL_INFO, MESSAGE_POLL_VOTE_*) |
| REST | 88 эндпоинтов `api/v9` из кэша (`rest-endpoints.md`), включая `users/@me/notes/{id}`, настройки, стримы, квесты, магазин | `api/v8`, узкий срез (~10 кастомных + методы discord.js) | API v9/v10; инвентарь есть; методы/тела добирать из web-кода |
| Голос (транспорт) | RTP/RTCP, PT=101 + RTX 102 (видео), UDP/ICE/DTLS по полному стеку; heartbeat 13.75 c | discord.js voice: IP-discovery → SELECT_PROTOCOL → UDP; heartbeat 13.75 c | переиспользовать: IP-discovery, SELECT_PROTOCOL, RTP-обёртка, heartbeat |
| Голос (крипто) | `PacketCrypter`: режимы 0–6 c оверхедом; предпочтение `aead_aes256_gcm_rtpsize` (5) → `aead_xchacha20...` (6); AAD = заголовок; nonce — 4-байтный LE-счётчик в хвосте; ключ 32 Б (`VOICE-CRYPTO.md`) | только `xsalsa20_poly1305[_suffix|_lite]` (tweetnacl); nonce-счётчик в `lite` пишется **BE** | реализовать 5/6 как основные; 0–3 — для совместимости |
| Голос (E2EE) | DAVE/MLS (не разобран, ~5%) | нет | вне сценария A; для C — блокер |
| Нативные модули | `discord_erlpack` (ETF), `discord_zstd`, `discord_krisp`, `discord_voice`, `discord_utils` (глобальные хоткеи/PTT), `discord_game_utils`, RPC, spellcheck; DWARF/символы — в `functions/static/*` | замены: JSON, без zstd, RMS-гейт, opusscript, нет хоткеев/PTT | zstd/opus/aes-gcm/xchacha — готовые крейты; Krisp — ML (заменить на свой/RNNoise или отключить для A) |
| UI-система | React + CSS: 5132 токена, 737 листов, 40 969 правил, 1411 семантических имён, 58+53 PNG трасс экранов, шрифты gg sans/Ginto (лицензия проприетарная) | Qt Widgets + SCSS: 3 темы, свои компоненты (~26), Source Sans Pro | свой UI по токенам; ассеты только под личные/серые нужды — иначе редизайн |
| Локализация | 26 754 RU-сообщения, rich-формат, 24 локали в лоадере | i18n, 3 языка × ~165 строк | RU-каталог у нас есть; механизм плюрализации не разобран |
| Десктоп-обвязка | ~150 IPC-каналов (трей, бейджи, автостарт, апдейтер, уведомления, буфер, скриншот-источники, power monitor, safeStorage, spellcheck, WebAuthn, клипы, поиск) — `desktop-core-notes.md` | трей, уведомления (node-notifier), оверлей, мультиаккаунт (токены в конфиге!) | чек-лист интеграций есть; секреты хранить в keyring, не в JSON |

### 5.2 Сравнение конкретных вызовов (функция ↔ функция)

| Задача | discord-qt (код) | Официальный клиент (RE) |
|---|---|---|
| Подключение к шлюзу | `WebSocketShard.connect` (discord.js; JSON, v7) | `GatewaySocket` (web): ETF+zstd-stream, `v=9` |
| Heartbeat | discord.js по `heartbeat_interval` | 41.25 с + QoS-пульс `op 40` ↔ `op 11` |
| Подписка на участников канала | `Guild.subscribeToTypingEvent()` → `op 14 {guild_id, typing, activities}` | ленивые подписки `op 14`, в захвате — 51× `GUILD_MEMBER_LIST_UPDATE` |
| Read-state (ack) | `POST /channels/:id/messages/:id/ack {token}` + `_ackToken`; `read_state` из READY | те же ack-запросы; `read_state`/`mention_count` в READY; `MESSAGE_ACK` диспатч |
| Заметки | `PUT /users/@me/notes/:id` | `users/@me/notes/{id}` присутствует в нашем REST-инвентаре (топ-таблица) |
| Настройки | `PATCH /users/@me/settings` (JSON-поля: theme, status, guild_positions, restricted_guilds…) | современный путь — **proto** (`USER_SETTINGS_PROTO_UPDATE` в захвате) + JSON-фрагменты |
| Кастомный статус | activity type 4 в presence (`op 3`), `setCustomStatus` | activity type 4 (`custom_status` в протоколе настроек) |
| Шифрование голоса | `Secretbox.methods.close` (tweetnacl, xsalsa) | `PacketCrypter::Encrypt` @0x477020, режимы 0–6; в живом сеансе — AEAD (тег 16 Б + nonce-хвост 4 Б) |
| Шумодав | `NoiseReductor` (RMS-порог) | `discord_krisp` (ML: `krisp_audio_sdk_*`, VAD, GRU-сети — есть в `functions/static/discord_krisp.nm.txt`) |
| Инвайты | `POST /invite/:code` + ожидание `GUILD_CREATE` | то же (плюс UI-инвайт-менеджер сервера) |
| Медиа-вложения | worker: HTTPS + roundify PNG, кэш в `env-paths` | кэш Chromium + CDN-параметры (`size`, `format`, `animated`) |

---

## 6. «Функции, которые не удалось обработать обратной инженерией» — карта обеих сторон

### 6.1 С нашей стороны (по `READINESS.md`, с оценкой влияния)

| Пробел RE | Статус | Влияние на Rust-клиент | Чем помогает discord-qt |
|---|---|---|---|
| Source maps (`*.js.map`) | ❌ сеть не согласована, приоритет низкий | низкое (2499 модулей уже выгружено) | — (у них свой код) |
| Свежий `READY`/`GUILD_CREATE` с нуля | ❌ нужен полный перезапуск копии | среднее: схемы READY/начальных гильдий | **полезен**: их кастомный READY показывает значимые поля (`user_settings`, `user_guild_settings`, `private_channels`, `notes`, `read_state`) — это чек-лист парсера |
| Меню сообщений / поиск / редактор тем (UI-трассы) | 🟡 50% | низкое-среднее | **полезен**: их меню сообщения = Edit/Quote/Delete/Copy×4 (минимальный набор); поиска у них нет |
| DAVE/MLS E2EE | ❌ ~5% | для сценария A не нужно; для C — блокер | нет (их эпоха) |
| Глубокий RE нативных модулей (кроме voice) | 🟡 ~60% | низкое для A | нет (у них нет ни Krisp, ни PTT) |
| Proto-настройки пользователя | 🟡 события есть (в захвате), схема нет | для A не нужно (свои настройки); для C — нужно | нет (2021, JSON-путь) |
| Симуляция медиапотока (видео) | — не требуется | — | нет |

**Вывод:** RE-пробелы **не блокируют сценарий A**; главные «дыры» для полного паритета с оригиналом — DAVE, Krisp, видео/стримы, proto-настройки, полные схемы READY/списков участников.

### 6.2 Со стороны discord-qt (что не смогли/не стали)

| Пробел discord-qt | Причина (по коду/issues) | Урок для нас |
|---|---|---|
| Реакции, ответы, треды, стикеры, поиск, группы ЛС | не реализовано (issues висят с 2021) | в Rust планировать сразу модель данных «реакции/ответы/треды» в слое сообщений |
| Видео/стримы/камера | не реализовано | вне MVP; транспорт снять (у нас есть RTP PT=101/102) |
| AEAD-крипто голоса | ограничение discord.js v12 (только xsalsa) | в Rust — сразу AEAD (5/6), крипто-спека готова |
| DAVE/E2EE | не существовало в 2021 | для A — не нужно; для C — обязательный ресёрч |
| Krisp | нет нативного модуля | для A — свой шумодав (RNNoise) или Toggle |
| Mute/Deafen микшера | в UI отсутствуют как отдельные функции | обязательные кнопки с первого среза UI |
| Папки серверов | парсинг без отрисовки | учесть в модели гильдий |
| Запись серверных настроек | несуществующий REST-метод discord.js (12.5.3) + нет вызовов из UI | свои REST-обёртки; тесты против реального API |
| Токены в открытом конфиге | `Account.token` в JSON | keyring/secret-service |
| Proto-настройки | эпоха | для C — реверс; для A — свои |

**Главный паттерн**: discord-qt строил клиент **патчами вокруг bot-библиотеки** — половина усилий ушла на «протаскивание» user-специфики (settings/read_state/notes/ack/op 14). В Rust правильнее строить собственный протокол-слой с нашими RE-знаниями, не повторяя этот долг.

---

## 7. Недостающие части для «полноценного клиента как оригинал» (наш Rust-проект)

### 7.1 Данные/спецификации (добор)

| # | Что | Зачем | Как |
|---|---|---|---|
| 1 | Полный каталог опкодов/тегов шлюза актуального клиента (v9, ETF+zstd) | ядро транспорта | добить из `REPORT.md §11` + захватов; свежий READY-захват (опционально, полный перезапуск копии) |
| 2 | Схемы READY/GUILD_CREATE/GUILD_MEMBER_LIST | стартовое состояние UI | свежий захват (см. 6.1) |
| 3 | REST v9: методы/тела/параметры (не только 88 GET URL) | отправка/настройки/инвайты | статический анализ web-модулей из кэша (метод отработан) |
| 4 | UI-трассы: поиск, меню сообщений, редактор тем, голосовая панель, DM-список (расширить) | точность UI | CDP-зонды по методам `ui-traces.mjs` (окно согласования — живая копия) |
| 5 | Proto-настройки (`USER_SETTINGS_PROTO_UPDATE`) | только для сценария C | реверс схемы из web-кода |
| 6 | DAVE/MLS | только для сценария C | отдельная волна ресёрча |

### 7.2 Инженерные компоненты (Rust, сценарий A)

| Компонент | Содержание | Готовые опоры |
|---|---|---|
| `proto-gateway` | WS-клиент: ETF-кодек (16 тегов), zstd-stream, heartbeat, resume, op 14-подписки, QoS-ответы | `REPORT.md §11`, `gw-zstd.mjs` |
| `proto-rest` | HTTP-клиент: v9-роуты, rate-limit, ретраи, мультиаккаунт | `rest-endpoints.md` |
| `proto-voice` | IP-discovery, SELECT_PROTOCOL (режимы 5/6 + фоллбэк 0–3), RTP-обёртка, 4 Б nonce-хвост, AAD; Opus (крейт `audiopus`/`opus`), джиттер-буфер | `VOICE-CRYPTO.md` |
| `ui` | Дизайн-система по токенам (цвета/типографика/спейсинг), компоненты: список серверов/каналов, лента сообщений (группировка, вложения, эмбеды, упоминания), composer (эмодзи/вложения/ответы/редактирование), пикеры, поповер профиля, настройки (6+ страниц), голосовая панель (mute/deafen), оверлей | `design-tokens.json`, `tokens-light/cozy`, 58+53 PNG трасс, `UI-TRACES.md` |
| `i18n` | RU-каталог 26 754 сообщения, plural/rich-логика | `ux-research/i18n*` |
| `assets` | эмодзи (twemoji-путь, как у discord-qt — лицензионно чисто), иконки 137 SVG, звуки 15 mp3, шрифты | `assets-cache/ASSETS.md` (⚠ лицензии) |
| `desktop` | трей, уведомления, хоткеи/PTT, оверлей, автостарт, deep links | `desktop-core-notes.md` (~150 каналов) |
| `store` | кэш сообщений/аватаров (Rust: sqlite), read-state модель | — |

### 7.3 Фичи-паритет (чек-лист, приоритеты)

- **P0 (MVP, «как Discord» на вид)**: серверы/каналы/DM, лента сообщений (markdown, эмодзи, вложения, эмбеды), composer (+ответы, цитаты, редактирование), эмодзи-пикер, профиль-поповер, настройки (аккаунт/вид/система/голос), темы (dark/light/darker/amoled), трей/уведомления, RU-локализация.
- **P1**: голос (подключение, mute/deafen, devices, speaking-индикация, оверлей), реакции, read-state/бейджи, поиск, меню сообщений (полный), папки серверов, автодополнение упоминаний.
- **P2**: треды/форумы, стикеры, GIF, звуковая панель, стримы/видео, DAVE (для C), proto-настройки (для C), мультиаккаунт-полировка, плагины.

### 7.4 Что уже лежит в репозитории `discord-lab` / `/tmp/opencode`

| Артефакт | Что даёт |
|---|---|
| `/tmp/opencode/r2-net/REPORT.md` | волны R2-NET/R3: шлюз, медиа, IPC, ETF-таблица (§11) |
| `/tmp/opencode/r2-net/VOICE-CRYPTO.md` | крипто: режимы 0–6, оверхед, AAD, nonce, ключ |
| `/tmp/opencode/r2-net/rest-endpoints.md` | 231 URL / 88 эндпоинтов `api/v9` |
| `/tmp/opencode/r2-net/UI-TRACES.md`, `ui-traces/` | DOM/скриншоты: DM, эмодзи-пикер, поповер |
| `/tmp/opencode/ux-research/GAP-ANALYSIS.md` | покрытие слоёв UI/UX (токены, локали, экраны, ассеты) |
| `/tmp/opencode/r2-net/READINESS.md` | go/no-go: A — GO (85–90%), C — NO-GO |
| `functions/static/*` (golden) | символы нативных модулей: voice/krisp/zstd/erlpack/utils |
| `/tmp/opencode/r2-net/assets-cache/ASSETS.md` | ассеты из кэша (иконки/звуки/шрифты/видео) |

### 7.5 Юридика и лицензии

- **ToS Discord**: третьи клиенты не запрещены буквально, но self-bot-подобный доступ (user-токен) — риск блокировки аккаунта. discord-qt прямо предупреждает об этом. Наш сценарий A (свой бэкенд) снимает вопрос; сценарий C — только с осознанным риском и не как продукт (текущий вывод: NO-GO).
- **GPL-3.0 discord-qt**: код (включая патчи) не копировать в проприетарный/другой продукт; перенимать можно идеи/контракты (список полей, состав патчей, подход к ffmpeg).
- **Ассеты Discord** (кэш): шрифты (gg sans, Whitney) — проприетарные, распространять нельзя; звуки/иконки — интеллектуальная собственность Discord. Для публичного продукта — свои ассеты; для прототипа — локально.

---

## 8. Приложение A: карта файлов discord-qt (сокращённо)

```
src/
├─ index.ts, Application.ts, Tray.ts, ApplicationEventEmitter.ts
├─ components/            — 26 UI-компонентов (см. §3.2); крупнейшие:
│    MessagesPanel/{MessageItem,MessageUtilities,MessagesPanel}, InputPanel/{InputPanel,AttachmentsPanel},
│    EmojiPicker/, UserButton/, VoicePanel/{VoicePanel,NoiseReductor,Silence}, GuildPanel/*, DMPanel/*, ProfilePopup/*
├─ patches/               — 16 патчей discord.js (§2.2) + Patcher.ts (patchClass/After/Before)
├─ utilities/             — ClientManager, ClientOptions, Account, ConfigManager, PluginManager, PictureWorker,
│    MarkdownStyles, ResolveEmoji, Paths, Events, PresenceStatusColor, StringProvider, voice/ (провайдеры)
├─ views/                 — MainView, SettingsView/{Accounts,MyAccount,Appearance,Voice,System,Overlay}
├─ windows/               — MainWindow, OverlayWindow
├─ dialogs/               — AcceptInvite, ConfirmLeaveGuild, CustomStatus, NicknameChange
├─ themes/                — dark/light/amoled.theme.scss
├─ locales/               — de-DE/en-US/ru-RU.json
└─ types/                 — discordjs.d.ts (расширения типов)
worker/                   — index.ts (HTTPS-get + PNG roundify в worker_threads), ProcessPng, HttpsGet
assets/icons/             — 41 PNG (Material Design Icons)
deploy/, build-deb.js, downloadNodeGui.js, launch.js, package.json
```

## 9. Приложение B: источники и проверки

- Клон и метрики: `/tmp/opencode/discord-qt` (git clone --depth 1; 128 TS, 44 PNG, 32 SCSS; README/LICENSE/package.json).
- Патчи и обработчики: `src/patches/*` прочитаны целиком (16 шт.), `src/utilities/ClientOptions.ts`, `ClientManager.ts`.
- Голос: `src/utilities/voice/*`, `VoicePanel/{NoiseReductor,Silence}.ts`, патчи `SecretBox.ts`, `StreamDispatcher.ts`, `Prism.ts`; проверка возможностей по исходникам `discord.js` 12.5.3 (`StreamDispatcher.js` 257–267; `Secretbox.js`; `RESTManager.js` — метода `patchClientUserGuildSettings` нет).
- UI: `src/components/*` (грепы фич), `MessageItem` контекстное меню, `ChannelsList`, `UserMenu`, настройки, оверлей.
- GitHub API: `archived=false`, `stargazers=210`, `forks=10`, `open_issues=39`, `pushed_at=2023-01-25`, последний релиз `v0.5.1` (10.01.2021); открытые issues — список проблемных функций.
- Наши артефакты: `r2-net/{READINESS,REPORT,VOICE-CRYPTO,UI-TRACES,rest-endpoints}.md`, `ux-research/GAP-ANALYSIS.md`, `functions/static/*.nm.txt`.

> **Статус документа**: черновик после кодового разбора; при старте Rust-разработки рекомендуется свести §7.1 в задачи волны (R4) и §7.3 — в бэклог милестоунов.
