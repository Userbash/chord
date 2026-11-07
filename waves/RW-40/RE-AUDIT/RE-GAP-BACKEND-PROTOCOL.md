# RE-GAP-BACKEND-PROTOCOL — сверка бэкенда и протокола с RE-дампом

Задача: `task-130` (RE-1, DeepSeek). Дата: 2026-10-30.
Режим: **read-only** по продуктовому коду. `cargo build/test` не запускались, продуктовая
сеть не трогалась; все утверждения — по файлам репозитория и по артефактам
`/home/sanya/re-deep/`.

Скоуп сверки: `rust/gui-spike/src/*.rs`, `rust/crates/discord-core/src/**`,
`rust/crates/discord-voice/src/**`, `rust/crates/discord-platform/src/**`
(плюс `discord-codec` — он держит ETF/zstd, без него выводы по шлюзу неполны).

---

## 0. Итог

Сверено 7 слоёв: транспорт шлюза, сессия шлюза, REST-ядро, REST-ручки, голос
(сигналинг/UDP/SRTP/RTCP), платформа и каталог REST-эндпоинтов.

Найдено **35 расхождений**: 9 P1, 15 P2, 11 P3. Из них 6 — не «мелочь полей», а
блокеры live-поведения:

1. **zstd-пайп теряет хвост соединения** — на эталонном сокете разобрано 44 терма из 113
   (`ws.rs:51-57`, `zstd.rs:16-20`). Часть диспатчей не доходит никогда.
2. **Не отправляются исходящие опкоды 3/4/8/14/37** — нет presence-апдейтов, входа в
   голос, подписок на участников. Отсюда `GUILD_MEMBER_LIST_UPDATE` у клиента 1048 шт.,
   у нас 0 (`gateway.rs:94-104`; оракул — таблица опкодов в бандле, §3.2).
3. **Мост dispatch → UI отдаёт 6 типов из 21** (`net.rs:113-120`): `MESSAGE_CREATE`,
   `MESSAGE_UPDATE`, `MESSAGE_ACK`, `TYPING_START`, `VOICE_STATE_UPDATE` и остальные
   отбрасываются. Live-чат и live-типинг на этом мосту невозможны.
4. **История канала не запрашивается по REST вообще** — `messages_page` читает фикстуры
   (`data.rs:1430`), `GET /channels/{id}/messages` есть в каталоге, но не вызывается.
5. **SRTP несовместим с проводом Discord**: ключ выводится HKDF с самодельными метками,
   nonce = seed+trailer (`srtp.rs:44-46,214-226,518-532`). С таким ключом сервер отбросит
   медиапакеты; в дампе схемы nonce/AAD нет (`voice-deep.md` §4 п.3).
6. **Голосовой WS-сессии нет** — есть только парсеры: нет клиента, нет op3
   `{t,seq_ack}`, нет op7 resume, нет op16, нет op5 speaking (`discord-voice/src/lib.rs`).

Плюс системный дефект идентичности: IDENTIFY и `X-Super-Properties` собираются из разных
источников и расходятся по `browser`, `os_arch`, `client_launch_id`, `browser_user_agent`
(`supervisor.rs:119-138` → `identify.rs:67-90` против `identity.rs:249-293`). Для сервера
это два разных клиента в одной сессии.

Полный реестр — §10; приоритизированный план для PM — `../PM/BACKLOG-RE-BACKEND.md`.

---

## 1. Оракулы

Все пути — от `/home/sanya/re-deep/` (далее `RE/`). md5 снят в этой сессии.

| Артефакт | Размер | md5 | Что берём |
|---|---:|---|---|
| `RE/README.md` | 12 КБ | `0bacbd423778140bbf647183b1a0d43a` | карта волны, покрытие |
| `RE/rest-d02.json` | 104 КБ | `9b450c34a2277b04989830b53b02f012` | 47 live-маршрутов, 133 запроса, auth-заголовки, тела ack/join |
| `RE/rest-static.json` | 376 КБ | `8e0ead46addbb2f54f143343d0de2108` | 904 записи enum, 648 REST, 81 запись с подтверждением по HTTP-кэшу и примером URL |
| `RE/gateway-protocol.json` | 33 КБ | `754fc8fa493d566bf1e46e9eebb82ff6` | ETF-теги, zstd-фрейминг, опкоды recv, heartbeat, READY, id-сверка |
| `RE/gateway-schema.json` | 92 КБ | `981063639da43d0c8d57d499aa79133e` | поля `d` по типам событий (осторожно: id отрицательные — старый парсер) |
| `RE/backend-ipc.json` | 120 КБ | `8e7057fcac57d0223174e55c3e43f2f6` | 168 каналов `DISCORD_*`, whitelists, `DiscordNative` |
| `RE/main-arch.json` | 76 КБ | `b250a52d4a0568e471b7666196cb4cd0` | main-процесс: окна, трей, меню, апдейтер, фичефлаги, deep links |
| `RE/voice-deep.json` | 8 КБ | `634400e96773fc1999a2cd83e380d7b3` | опкоды RTC, UDP-форматы, RTP/RTCP, ротация сокета |
| `RE/decoded-index.json` | 744 КБ | `70137d96454f5b71dd98d96c6b0968f2` | индекс декодированных чанков бандла |
| `RE/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` | 12.4 МБ | `934b1289b7ebf17bc0fd09569b23b438` | **исходник клиента**: таблица опкодов, тела исходящих кадров, ретраи, заголовки |
| `RE/voice/rtc1-frames.jsonl` | 3.4 КБ | `7f913c8fe659f3e20bee2cb1d5269fd8` | 22 кадра RTC (op0/1/2/3/4/6/8/15/16), полные JSON |
| `RE/voice/rtc2-frames.jsonl` | 3.2 КБ | `e8003f12f2eef3136bc31a5bba16ed5b` | вторая сессия, 20 кадров, те же формы |

Метод сверки: (1) выписать контракт из артефакта; (2) найти наш код/константу;
(3) прогнать независимую проверку по сырому захвату, если артефакт производный.
Для исходящих опкодов шлюза использован **исходник бандла** (таблица `e2=((s={})…)`
по смещению 4 996 243): это единственный в дампе оракул отправки — recv-стороны
недостаточно, исходящие кадры в захвате замаскированы (`gateway-protocol.json`,
`opcodes.sentGateway`).

Инструменты REA: `open_binary` открыл web-бандл как `javascript`
(sha256 `667a771c61ff709e2f0400d395a324f0a5b1b5aa89e11e263289144a12d084ae`).
Провайдеры REA не поддержали `search_strings` для JS-артефакта (ghidra/ida/hopper/kuna
отклоняют `target_kind=artifact`/`format=javascript`), поэтому строки и смещения сняты
grep'ом по байтам файла; смещения в отчёте — байтовые, проверяемые `find`/`rfind`.

Два ограничения оракула, которые важны для приёмки:

- `gateway-schema.json` построен на старом парсере big-int: `guild_id` там
  `-1467272931325252608` вместо `1014537851418980372`. Для id использовать только
  `gateway-protocol.json` (§2.5) или сырой захват.
- `rest-d02.json` содержит 47 маршрутов, но **не содержит** живого
  `GET /guilds/{id}/messages/search`, хотя запрос есть в `capture-run2.jsonl`
  (4 вызова, `?content=…&sort_by=timestamp&sort_order=desc&offset=0`). Каталог
  производный и неполный; счётчики «47» в шапке `routes.rs` описывают именно его.

---

## 2. Транспорт шлюза

### 2.1 Что совпадает

| Контракт | Оракул | Наш код |
|---|---|---|
| `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream` | `rest-d02.json` `ws.connections`, `gateway-protocol.md` §3 | `ws.rs:77-80` |
| один непрерывный zstd-кадр, магия только в первом WS-сообщении | `gateway-protocol.json` `zstd.perSocket` (`framesWithMagic: 1`, `magicPositions:[0]`) | `zstd.rs:3-20`, `ws.rs:8-14` |
| `0x83` перед каждым термом | `gateway-protocol.json` `etf.framing` | `etf.rs:24`, `ws.rs:265` |
| 11 фактических тегов + 6 поддержанных | `gateway-protocol.json` `etf.tags`, `supportedNotSeen` | `etf.rs:26-55` (17 тегов) |
| big-int `[n][sign][n байт LE]`, знак до цифр | `gateway-protocol.md` §2.5 | `etf.rs:479-514` (`big_int`) |
| `LIST_EXT` с хвостовым `NIL_EXT` | `gateway-protocol.json` §2 «nil — два случая» | `etf.rs:455-477` |
| атом `"nil"` ≠ `null` | там же | `etf.rs:158-159, 401` (разные ветки) |

Транспорт и кодек — самая здоровая часть. Ниже два дефекта, которые ломают приём.

### 2.2 F-GW-01 (P1). Удержание окна zstd: теряется хвост каждого соединения

Наш `ZstdStream` держит `ruzstd::FrameDecoder` и после каждого push отдаёт только
`распаковано − окно`; окно шлюза 64 КиБ, кадр не завершается никогда. Это прямо
задокументировано в коде: `zstd.rs:16-20`, `ws.rs:51-57` — «пайп видит 44 полных терма
(последний `s = 36`) и 704 Б обрезанного терма в хвосте, тогда как в потоке есть
113 диспатчей».

Оракул: `gateway-protocol.md` §3 — эталонный декодер `zlib.createZstdDecompress()`
пишет кадры последовательно и **не вызывает `end()`**, но отдаёт накопленный вывод
полностью; `perSocket[].decompressedBytes` = 824 816 Б для `95974.1389`, термов 137.

Последствие: до 64 КиБ последних данных соединения не разбираются никогда; события
конца сессии (в т.ч. перед `op7`/close) теряются. Лечится либо полным буфером растущего
кадра с периодическим re-open окна, либо собственным zstd-декодером с явным flush.

### 2.3 F-GW-04 (P2). IDENTIFY отправляется после HELLO; HELLO-таймаута нет

`ws.rs:724` — `if *sent || pipe.session().hello().is_none() { return Ok(()); }`:
кадр op2 уходит только после разобранного HELLO.

Захват: в обоих свежих логинах первый **исходящий** кадр 943 Б (IDENTIFY по инференсу
длины, `gateway-protocol.md` §4) идёт **до** первого входящего 176 Б (HELLO):
`capture-ready.jsonl` ts `1792390388497` → `1792390388626` (129 мс),
`capture-run2.jsonl` ts `1792391312451` → `1792391312488` (37 мс).
Инференс по длинам, не декод — в отчёте помечаю как «наблюдение», не «подтверждено».

Второе: у клиента есть `helloTimeout` (для RTC — 20 с, бандл @2 394 981); у нас дедлайна
нет: до HELLO `Session::next_deadline_ms()` = `None`, цикл ждёт `ws.next()` бесконечно
(`ws.rs:625-636`). Соединение, которое приняло TCP и молчит, висит до внешнего таймаута.

---

## 3. Сессия шлюза и протокол опкодов

### 3.1 Совпадает

| Контракт | Оракул | Наш код |
|---|---|---|
| heartbeat 41 250 мс из `HELLO.d.heartbeat_interval` | `gateway-protocol.json` `heartbeat.gatewayIntervalMs`, 7 сокетов | `gateway.rs:110,382-393` |
| recv-опкоды 0/10/11 (+7) | `opcodes.recvBySocket` | `gateway.rs:94-104` |
| ACK закрывает удар, RTT = now − отправка | `opcodes` (ACK 196 шт.) | `gateway.rs:400-409` |
| op7 → реконнект, op9 → по `d` | `opcodes.noOpsSeen` (op9 не наблюдался) | `gateway.rs:274-275`, `ws.rs:277-281` |
| backoff 1 с … 60 с, сброс после READY | `rest-static.md` §2 (`gatewayBackoff` 1e3..6e4), бандл @5 016 157 | `supervisor.rs:32-75` |
| capabilities 1 734 653 / 1 767 421 | бандл @4 996 701 (`_doIdentify`) | `identify.rs:18-21` |
| `compress:false` при zstd-stream | бандл `_doIdentify` (`usesLegacyCompression()`) | `supervisor.rs:130` |
| 21 тип dispatch | `gateway-protocol.json` `opcodes.dispatchTypes` | `dispatch.rs:15-117` |

### 3.2 F-GW-02 (P1). Исходящих опкодов нет: 3, 4, 8, 14, 37, 18–22

Наш `Op` знает только 0/1/7/9/10/11 (`gateway.rs:94-104`), а отправляются вообще лишь
IDENTIFY (op2), RESUME (op6) и heartbeat (op1/op40). Таблица клиента (бандл @4 996 243):

```
DISPATCH=0 HEARTBEAT=1 IDENTIFY=2 PRESENCE_UPDATE=3 VOICE_STATE_UPDATE=4
VOICE_SERVER_PING=5 RESUME=6 RECONNECT=7 REQUEST_GUILD_MEMBERS=8 INVALID_SESSION=9
HELLO=10 HEARTBEAT_ACK=11 CALL_CONNECT=13 GUILD_SUBSCRIPTIONS=14
STREAM_CREATE=18 STREAM_DELETE=19 STREAM_WATCH=20 STREAM_PING=21 STREAM_SET_PAUSED=22
REQUEST_GUILD_APPLICATION_COMMANDS=24 REQUEST_FORUM_UNREADS=28 REMOTE_COMMAND=29
GET_DELETED_ENTITY_IDS_NOT_MATCHING_HASH=30 REQUEST_SOUNDBOARD_SOUNDS=31
REQUEST_LAST_MESSAGES=34 SEARCH_RECENT_MEMBERS=35 GUILD_SUBSCRIPTIONS_BULK=37
GUILD_CHANNELS_RESYNC=38 REQUEST_CHANNEL_MEMBER_COUNT=39 QOS_HEARTBEAT=40
UPDATE_TIME_SPENT_SESSION_ID=41 REQUEST_CHANNEL_INFO=43
```

Формы тел (бандл @4 998 360…4 999 200, дословно из минифицированного кода):

| op | метод клиента | тело |
|---:|---|---|
| 3 | `presenceUpdate(status,since,activities,afk)` | `{status, since, activities, afk}` |
| 4 | `voiceStateUpdate({guildId,channelId,selfMute,selfDeaf,selfVideo,flags,…})` | `{guild_id, channel_id, self_mute, self_deaf, self_video, flags}` + `preferred_region(s)`, `tracks[{type,rid,quality}]` при наличии |
| 5 | `voiceServerPing()` | `null` |
| 8 | `requestGuildMembers(guild,{query,limit,userIds,presences})` | `{guild_id, query, limit, user_ids, presences}`; клиент чанкует `user_ids` по 100 (бандл @4 954 008) |
| 14 | `GUILD_SUBSCRIPTIONS` | в веб-клиенте **не используется** (только op37) |
| 18 | `streamCreate(type,guild,channel,region)` | `{type, guild_id, channel_id, preferred_region}` |
| 19/20/21 | delete/watch/ping | `{stream_key}` |
| 22 | setPaused | `{stream_key, paused}` |
| 31 | `requestSoundboardSounds(guilds)` | `{guild_ids}` |
| 34 | `requestLastMessages(guild,channels)` | `{guild_id, channel_ids}` |
| 37 | `updateGuildSubscriptions(subs)` | `{subscriptions: {<guild_id>: <sub>}}`, чанки по 15 360 Б JSON |
| 39 | `requestChannelMemberCount` | `{guild_id, channel_id}` |
| 43 | `requestChannelInfo` | `{guild_id, fields}` |

Почему это P1, а не «фича»: без op37 подписки `GUILD_MEMBER_LIST_UPDATE` не приходит
(в захвате 1048 шт. — 78% всех op0), без op8 нет `GUILD_MEMBERS_CHUNK`, без op4 нельзя
войти в голосовой канал, без op3 другие пользователи не видят наш статус.

Форма объекта подписки в op37 всё ещё не снята: в бандле она собирается из внутреннего
`SubscriptionManager` (бандл @9 112 505, `G.get(guild)`; флаги `{typing, activities,
threads}` + `channels`). Это отдельная RE-задача (см. бэклог P1-01), но сам опкод и
обёртка `{subscriptions:{…}}` уже подтверждены.

### 3.3 F-GW-03 (P1). Мост dispatch отдаёт 6 типов из 21

`net.rs:113-120`:

```rust
pub const FORWARDED_DISPATCHES: [&str; 6] = [
    "READY", "GUILD_MEMBERS_CHUNK", "GUILD_MEMBER_LIST_UPDATE",
    "PRESENCE_UPDATE", "MESSAGE_REACTION_ADD", "MESSAGE_REACTION_REMOVE",
];
```

Всё остальное отбрасывается на `net.rs:249`. Для сравнения — частоты в захвате
(`gateway-protocol.json` `dispatchTypes`, всего 1340 op0): `MESSAGE_CREATE` 31,
`MESSAGE_UPDATE` 24, `TYPPING_START` 28, `MESSAGE_ACK` 8, `VOICE_STATE_UPDATE` 8,
`THREAD_LIST_SYNC` 7, `CHANNEL_INFO` 7, `SESSIONS_REPLACE` 8, `SOUNDBOARD_SOUNDS` 21.

Продуктовое следствие: live-сообщения, правки, удаления, typing, голосовые состояния и
треды не доходят до экранов, хотя ядро их разбирает (`state/types.rs`, `dispatch.rs`).
Это же объясняет, почему P-03 «входящий TYPING_START не маршрутизирован» и M-01
«нет live-истории» держатся, даже когда бэкенд-парсеры готовы.

### 3.4 F-GW-05 (P2). `client_state` — только `guild_versions`

`identify.rs:316-327` собирает `client_state = {guild_versions: {}}`.
Клиент при `canUseGuildVersions()` отправляет восемь полей (бандл `_doIdentify`,
@4 996 300…):

```
{guild_versions, highest_last_message_id, read_state_version, user_guild_settings_version,
 user_settings_version, private_channels_version, api_code_version, initial_guild_id}
```

Без них сервер не может прислать дельту и всегда отдаёт полный READY (в захвате 577 КБ
на логин). Не блокер, но прямая причина «READY каждый раз полный» и лишнего трафика.

### 3.5 F-GW-06 (P2). Расписание heartbeat: jitter 0 и привязка к ACK

- `supervisor.rs:138` — `jitter_seed: 0`; `gateway.rs:395` — первый удар в
  `now + seed % interval` = **сразу после HELLO**. Клиент: `setTimeout(…, Math.floor(
  Math.random()*interval))`, затем `setInterval(interval)` и немедленный первый удар
  (бандл @5 016 157). То есть у клиента первый удар — случайная задержка в [0, interval),
  у нас — 0 мс при дефолтном конфиге.
- `gateway.rs:408` — следующий удар планируется от **момента ACK**. Клиент держит
  фиксированный `setInterval` от отправки; измеренные дельты в захвате — ровно 41 250 мс
  (`heartbeat.sentPeriodic`, median 41250, min 41248). Наши удары будут уезжать на RTT.

### 3.6 F-GW-07 (P2). Лимит реконнектов останавливает клиент навсегда

`supervisor.rs:333` + `max_reconnects: 8` (`:136`): после восьми неудачных попыток
супервайзер отдаёт `Stop(ReconnectLimit)` и больше не подключается.

Клиент (`_handleClose`, бандл @5 014 326) останавливается **только** на 4004
(«authentication failure»). На остальных кодах он продолжает цикл backoff, а после
`gatewayBackoff.fails > 4` вызывает `_reset`: обнуляет `sessionId`/`seq` (следующая
попытка — IDENTIFY, не RESUME) и продолжает реконнект. То есть клиент переживает
долгий offline, мы — нет: после ~8 попыток приложение остаётся мёртвым до ручного
переподключения.

Отдельно: наш Stop на 4013/4014 — осознанное решение `ADR-2-auth.md`
(`supervisor.rs:8,226-231`), у клиента эти коды специально не обрабатываются. Это
расхождение политики, его не «исправлять» без PM; но его надо держать в контракте,
а не выдавать за паритет.

### 3.7 F-GW-08 (P2). Окно RESUME: 30 с вместо 3 мин

`resume.rs:19` — `RESUME_MAX_DOWNTIME_MS = 30_000`, решение по времени с момента разрыва.
Клиент: `_doResumeOrIdentify` (бандл @5 019 509) — resume, если `sessionId != null` и
`Date.now() - lastHeartbeatAckTime <= tI`, где `tI = 3 * Millis.MINUTE`
(объявление рядом с `tf=30s, tp=1min` в модуле GatewaySocket). Окно считается от
последнего ACK, а не от разрыва. Мы делаем IDENTIFY там, где клиент ещё резюмируется.

### 3.8 F-GW-09 (P2). Две несовместимые идентичности в одной сессии

IDENTIFY собирается из `IdentifyProperties::for_rw_client()` (`supervisor.rs:126`), а
`X-Super-Properties` — из `identity::build()` (`identity.rs:249`). Значения расходятся:

| Поле | IDENTIFY (`identify.rs:67-90`) | SP (`identity.rs:250-293`) | Эталон (`rest-d02.json` `auth.superProperties`) |
|---|---|---|---|
| `browser` | `Discord Client` | `discord-lab` | `Discord Client` |
| `client_version` | версия крейта (`0.1.0`) | версия крейта | `1.0.162` |
| `browser_version` | версия крейта | версия крейта | `42.11.12` |
| `browser_user_agent` | `discord-lab-rs/0.1.0` | `discord-lab/0.1.0 (Linux; x64)` | `Mozilla/5.0 … discord/1.0.162 Chrome/148… Electron/42.11.12` |
| `os_arch` | `std::env::consts::ARCH` = `x86_64` | `map_arch` = `x64` | `x64` |
| `client_launch_id` | нулевой UUID | случайный v4 | случайный UUID |
| `client_build_number` | 641 004 | 641 004 (если задан) | 641 004 |
| `system_locale` | `en-US` | из `LC_ALL`/`LANG` | `ru` |

Политика «свои честные значения» (`identity.rs:4-6`) объясняет версию и UA, но не
объясняет (а) расхождение полей между двумя запросами одной сессии, (б) пару
`client_version=0.1.0` + `client_build_number=641004`, которой у Discord не существует
(641004 — сборка 1.0.162). Сервер и анти-абьюз видят несогласованный отпечаток;
отдельно это бьёт по капче/`/science`, где UA и версия проверяются.

### 3.9 F-GW-10 (P3). `qos_token` и ключи op1 — байтовые отличия

- `qos_token_client_provided(false)` = `0A 02 08 00` (`identify.rs:209-211`).
  Protobuf-писатель клиента пропускает `false` (`if(!1!==e.isActive)`, бандл QosToken
  writer), поэтому для `is_active=false` кадр — `0A 00`. Семантика та же, байты разные;
  если когда-нибудь понадобится сверка длины IDENTIFY (эталон 943 Б), это помешает.
- `encode_heartbeat` (`ws.rs:413-425`) кладёт ключи `op`/`d` как `EtfValue::Atom`
  (ATOM_EXT `0x64`), а `encode_qos_heartbeat` (`ws.rs:452-467`) — как
  `EtfValue::Str` (BINARY_EXT `0x6d`). Клиентский wetf для исходящих использует
  `key:"binary"` (`identify.rs:8-11` фиксирует это для IDENTIFY), то есть op1 должен
  быть binary. Сейчас два исходящих кадра одного клиента кодируются по-разному.

---

## 4. REST-ядро

### 4.1 Совпадает

| Контракт | Оракул | Наш код |
|---|---|---|
| база `https://discord.com/api/v9` | `rest-static.md` §2 | `lib.rs` `API_V9_BASE`, `client.rs:224` |
| `Authorization: <сырой токен>` без Bearer | `rest-static.md` §2/§4, `rest-d02.md` | `client.rs:60-79,120-133` |
| `X-Super-Properties` = base64 JSON 22 поля | `rest-d02.json` `auth.superProperties`, `rest-d02.md` §6.2 | `super_properties.rs:18,140-165` |
| 6 заголовков полного набора живого запроса | `rest-d02.json` `totals.apiRequestsWithFullAuthHeaderSet=133` | `headers.rs:16-23` |
| `X-Debug-Options: bugReporterEnabled` | `rest-d02.json` `auth.withDebugOptions=221` | `client.rs:53` |
| body ack `{token,last_viewed}`, `last_viewed` — счётчик | `rest-d02.json` endpoint `POST …/ack`, `reqBodySampleRef ready\|57992.7577` | `messages.rs:385-393` |
| тело accept инвайта ровно 3 nullable поля | `rest-static.json` `INVITE.bodyKeys` | `invites.rs:196-206,403-408` |
| resolve инвайта `?with_counts=true&with_expiration=true` | `rest-static.json` `INVITE` (call-site) | `invites.rs:44` |

### 4.2 F-RS-03 (P2). Ретраи: другой набор статусов и другой потолок

Клиент (бандл @10 692 340, дословно):

```js
E = new Set([502,504,507,598,599,522,523,524]);  // авто-повтор с backoff
A = new Set([429,503]);                          // очередь по URL до retry-after
```

`retry-after` берётся из заголовка как **целые секунды**, иначе из тела
`retry_after` (секунды, float) — бандл @10 692 774. Для `{429,503}` — очередь **по URL**
(`p.get(e.url)`): пауза `1000 * (retry_after ?? 5)`, backoff `new Backoff(1e3, 6e4)`,
то есть дефолт 5 с и потолок 60 с (`rest-static.md` §2).

Важно: набор `E` применяется только когда вызывающий передал `retries > 0`
(`if (null != t.retries && t.retries-- > 0 && E.has(r.status))`). То есть это
**опция на call-site**, а не безусловный ретрай; какие именно вызовы её включают,
по бандлу не восстановлено. Наш код ретраит безусловно по своей политике.

У нас: `rate_limit.rs:181-203` — 429, 401, любой 5xx, прочие 4xx; `client.rs:367-370` —
повтор 5xx только для GET/PUT/DELETE; `client.rs:210-218` — `backoff 250 мс`,
`max_retry_after 30 с`. Следствия:

- 500/501/505 мы ретраим, клиент — нет (даже с `retries`);
- 429 у нас обрабатывается общей паузой, без очереди по URL;
- 503 у нас идёт в общий 5xx-backoff и **не читает** `retry-after`, хотя клиент
  держит его в очереди 429/503;
- потолок ожидания вдвое ниже клиентского (30 с против 60 с).

### 4.3 F-RS-04 (P2). `Accept-Language` и отсутствующие заголовки

`client.rs:112-114` (TODO NET-A) и `client.rs:166`: `Accept-Language` = локаль как есть.
Клиент (бандл @1 002 368) собирает q-список из `navigator.languages`:
первый язык без q, далее `;q=0.9`, `;q=0.8`, … до `;q=0.1`
(пример из живого захвата — `ru,ru-RU;q=0.9,…`).

Также `prepareRequest` ставит `X-Fingerprint` (`getFingerprint()`),
`X-Installation-ID` (`getInstallationForTracking()`), `X-Routing-Key` (если есть),
`x-client-trace-id` (при tracing) и `X-Audit-Log-Reason` (для модерации,
`encodeURIComponent(reason)`). В live-захвате этих полей нет
(`rest-d02.json` `unknowns U-08`: «заявлены статикой, в живых запросах отсутствуют»),
поэтому для обычных чтений мы паритетны. Но `X-Fingerprint`/`X-Installation-ID`
участвуют в auth/регистрации/captcha — там без них нельзя.

### 4.4 F-RS-05 (P2). Нет `User-Agent` и `Accept-Encoding`

`transport.rs:296-305` добавляет только `Host` и `Connection: close`; `User-Agent`
не ставится нигде (проверено grep по `rest/`). Живой клиент ходит с UA Electron
(эталон `browser_user_agent` в `rest-d02.json`), а `discord.com` за Cloudflare.
Запрос без UA — либо отказ, либо повышенный challenge; `Accept-Encoding` отсутствует,
значит все ответы приходят несжатыми (READY не при чём, но `/users/@me/profile`,
`/channels/.../messages` теряют 5–10× на трафике).

### 4.5 F-RS-06 (P2). В каталоге маршрутов нет query-слоя

`RouteSpec` (`routes.rs:44-52`) — только `method/path/body_keys`. Параметров запроса
нет вообще, поэтому обязательные параметры живут в ad-hoc строках по месту:

- `GET /channels/{id}/messages` — `limit`, `before`, `after`, `around`
  (эталон: `?limit=12`, `rest-static.json` `MESSAGES.cacheEvidence`);
- `GET /channels/{id}/messages/pins` — `?limit=25` (`PINS.cacheEvidence`);
- `GET /channels/{id}/threads/search` — обязательный `archived=true` +
  `sort_by=last_message_time&sort_order=desc&limit=25&tag_setting=match_some&offset=0`
  (`THREAD_SEARCH.cacheEvidence`, дословный пример URL);
- `GET /guilds/{id}/messages/search` — `content`, `sort_by`, `sort_order`, `offset`,
  `search_session_id`, `search_query_id` (сырой захват).

Пока запросы собираются вручную, любой новый вызов легко теряет обязательный параметр —
так и происходит с pins/threads (§5.2).

### 4.6 F-RS-02 (P1). 202 не отличается от 200

`rest_failure.rs:65-76`: `200..=299 => None` — «успех». Для `GET /guilds/{id}/messages/search`
Discord отвечает `202` с `{retry_after, …}`, пока идёт индексация; клиент повторяет
запрос до 5 раз, ожидая `retry_after` (PARITY M-13, `search_contract.rs`).
У нас `search.rs:227` пытается распарсить тело `202` как `SearchResponse` и падает в
`InvalidResponse`. То есть на «холодном» поиске мы показываем ошибку вместо результата.

Оракул по статусу: в `rest-d02.json` 202 не встречался (поиск шёл по уже
проиндексированной гильдии, 133 запроса), поэтому форма тела `202` — `[A]`/
`[S]`-уровень; сама необходимость обработки 202 подтверждена требованиями M-13 и
поведением клиента. Это надо закрыть live-окном, а не выдумывать поля.

### 4.7 F-RS-12 (P3). Rate-limit поля и bucket-reuse

`rate_limit.rs:146-179` читает `retry-after`, `x-ratelimit-global`, `-remaining`,
`-reset-after`, `-bucket`, но не использует `bucket` для переиспользования очереди и не
читает `x-ratelimit-scope`/`x-ratelimit-limit`. Клиент держит очередь по URL
(`A`-множество) и `retryAfter` из тела. На один канал с двумя вкладками это даст
расхождение по числу 429.

### 4.8 F-RS-11 (P3). Один TCP+TLS на запрос

`transport.rs:1-8,301` — `Connection: close`, keep-alive выключен осознанно («появится
отдельной волной»). Клиент — superagent поверх Electron с переиспользованием сокетов.
Функционально не ломает, но на цепочке «open channel → messages → pins → threads →
ack → profile» это 5 TLS-рукопожатий подряд.

---

## 5. REST-эндпоинты и тела

### 5.1 F-RS-01 (P1). Что реально вызывается из приложения

Список получен grep'ом по `routes::find`, `format!("/…")` и `ApiClient` в
`gui-spike/src` + `discord-core/src`:

| Метод и путь | Где | Статус |
|---|---|---|
| `POST /auth/login`, `/auth/mfa/{type}`, `/auth/mfa/sms/send` | `auth.rs:430-434` | есть |
| `GET /users/@me` | `auth.rs:50` | есть |
| `GET`/`POST /invites/{code}` | `invites.rs:308-325` | есть |
| `PUT /guilds/{id}/members/@me?lurker=` | `guilds.rs:34` | есть |
| `DELETE /users/@me/guilds/{id}` | `guilds.rs:38` | есть |
| `POST /channels/{id}/messages` | `messages.rs:158-176` | есть |
| `PATCH`/`DELETE /channels/{id}/messages/{id}` | `messages.rs:221-241` | есть |
| `POST /channels/{id}/messages/{id}/ack` | `messages.rs:280` | есть |
| `PUT`/`DELETE /channels/{id}/messages/{id}/reactions/{emoji}/@me` | `reactions.rs:126-131` | есть |
| `POST /channels/{id}/typing` | `routes.rs:185` + `typing.rs` | есть |
| `GET /guilds/{id}/messages/search` | `search.rs:199` | есть |
| `GET /channels/{id}/messages` (история) | — | **не вызывается** |
| `GET /channels/{id}/messages/pins` | каталог `routes.rs:149` | **нет IPC** |
| `GET /channels/{id}/threads/search` | каталог `routes.rs:150` | **нет IPC, нет query** |
| `GET /users/{id}/profile` | — | нет |
| `POST /channels/{id}/attachments`, `PUT upload_url`, `POST /attachments/refresh-urls` | — | нет |
| `POST /read-states/ack-bulk` | — | нет |
| `GET`/`PATCH /users/@me/settings-proto/{type}` | — | нет |
| `PATCH /users/@me`, `GET /users/@me/connections`, `GET /users/@me/notes/{id}` | — | нет |
| `POST /users/@me/channels` (DM/группа), `GET/PUT/DELETE /users/@me/relationships/{id}` | — | нет |
| `GET /users/@me/mentions`, `GET /users/@me/saved-messages` | — | нет |
| `PATCH /users/@me/notification-settings` | — | нет |
| `POST /interactions`, `GET …/interaction-data` | — | нет |
| `POST /science`, `POST /metrics/v2` | — | нет (см. §5.5) |

`main.rs:730-770` регистрирует 36 Tauri-команд; `pins_list`, `threads_list`,
`create_invite`, `attachments_*` в списке отсутствуют — это подтверждает P1-находку
PARITY-SYNC-11 (фронтенд зовёт команды, которых нет).

### 5.2 F-RS-07 (P2). Поиск: нет `search_session_id`/`search_query_id`

`search.rs:204-217` собирает `content`, `channel_id`, `sort_by`, `sort_order`, `offset`.
Живой URL (сырой `capture-run2.jsonl`):

```
/guilds/1014537851418980372/messages/search?content=ProBot…&sort_by=timestamp
 &sort_order=desc&search_session_id=dae0e992-…&search_query_id=8fb1269e-…&offset=0
```

`search_session_id` и `search_query_id` — UUID на сессию/запрос; без них сервер не
связывает страницы выдачи и вкладки `search/tabs`. `include_nsfw` тоже отсутствует
(PARITY M-13). Пагинация `offset` шагом 25 в коде не проверяется — принимается любой
`offset`, включая не кратный 25.

### 5.3 F-RS-08/09 (P3). Тела реакций и ack

- `reactions.rs:127-131`: `PUT …/reactions/{emoji}/@me` с телом `{}`.
  Живой запрос (`rest-d02.json`, endpoint реакции) — 204, `reqBodyKeys: []`, тела нет.
  Наш `ApiClient` из-за непустого тела ещё и добавит `Content-Type: application/json`.
- `messages.rs:387-393`: ack ровно `{token,last_viewed}` — совпадает с живым телом.
  Статическая карта (`rest-static.json` `MESSAGE_ACK.bodyKeys`) знает ещё вариант
  `{token,last_viewed,flags}`; поле `flags` не отправляется и не читается. Для
  unread-бейджа (`M-07`, `C-03`) это может понадобиться.

### 5.4 F-RS-10 (P3). Дрейф каталога маршрутов

`routes.rs:71` объявляет массив на 49 элементов, шапка (`routes.rs:1-9`) говорит
«47 маршрутов живого захвата D-02 и POST отправки». Фактическая разница:

- `POST /channels/{id}/messages` — намеренное расширение (задокументировано);
- `GET /guilds/{id}/messages/search` — **нет в `rest-d02.json`**, хотя есть в живом
  захвате. То есть шапка «47 из оракула» неточна: один элемент каталога взят не из
  оракула, а из сырого захвата, и это нигде не помечено (в отличие от `TYPING_PATH`
  и `INVITE_PATH`, у которых есть явные расширения, `routes.rs:179-213`).

Плюс `body_keys` для `POST /channels/{id}/messages` = `["content","nonce","tts"]` —
в `rest-static.json` у `MESSAGES` `bodyKeys: []` (тело не восстановлено), значит набор
полей — `[S]`-уровень. Живого POST сообщения в дампе нет (`rest-d02.json`:
`apiWithRequestBody: 6`, POST messages среди них отсутствует).

### 5.5 Телеметрия: осознанный пропуск, но с последствием

`POST /science` и `POST /metrics/v2` (`rest-static.json` boot steps 10-11) у нас
заблокированы политикой (`telemetry.rs`, `client.rs:20,504`). Это решение продукта, не
дефект. Но у `/science` тело `{token, events}` и он идёт с тем же токеном; при
включении телеметрии нельзя логировать тело (сейчас `Debug` показывает только длину —
`transport.rs:78-101`, это правильно).

---

## 6. Голос

### 6.1 Совпадает

| Контракт | Оракул | Наш код |
|---|---|---|
| `wss://c-<region>.discord.media:port/?v=9` | `rest-d02.json` `ws.connections`, `voice-deep.json` `_meta` | `lib.rs:655-666` |
| op0 identify: `server_id,channel_id,user_id,session_id,token,max_dave_protocol_version,video,streams` | `voice/rtc2-frames.jsonl` (op0), `voice-deep.json` `signaling.ops.0` | `lib.rs:160-202` |
| op1 select_protocol: `protocol:udp`, `data{address,port,mode}`, дубли на верхнем уровне, `codecs` | `voice/rtc1-frames.jsonl` op1 | `lib.rs:223-255` |
| op2 ready: `ssrc,port,ip,modes,experiments,streams` | `rtc1/rtc2` op2 | `lib.rs:443-453` |
| op4 session_description: `mode,secret_key(32Б),media_session_id,dave_protocol_version,secure_frames_version,audio_codec,video_codec` | `rtc*` op4 | `lib.rs:456-473` |
| op8 hello `{v:8, heartbeat_interval:13750}` | `rtc*` op8 | `lib.rs:361-366` |
| discovery 74 Б, type 1/2, ssrc, ASCII-адрес, порт @72 | `voice-deep.json` `udp.discovery` | `udp.rs:8,110-199` |
| keepalive `1337cafe 01000000` / `1337f00d`, 5 с | `udp.keepalive` | `udp.rs:10-29` |
| RTP PT 120, 48 кГц, 20 мс/960 тиков, ext `0xBEDE` | `udp.rtp`, `media/rtp.rs` | `media/rtp.rs:14-35` |
| SRTP tag 16, хвостовой счётчик 4 Б LE, `0x80000000|n`, общий для RTP+RTCP | `udp.rtp.layout`, `udp.rtcp` | `srtp.rs:31-46,438-445` |
| ротация сокета ~60.08 с | `udp.socket_lifecycle.rotation_after_ms: 60080` | `udp.rs:11,213-245` |

### 6.2 F-VC-01 (P1). Ключ и nonce SRTP не те, что на проводе

`srtp.rs:44-46`:

```rust
const LABEL_KEY: &[u8] = b"discord-lab/srtp/aead-key";
const LABEL_NONCE: &[u8] = b"discord-lab/srtp/nonce-seed";
```

`SrtpContext::new` (`srtp.rs:214-226`) выводит сессионный ключ через HKDF-SHA1 из
`master_key`+`master_salt`, а nonce — `nonce_seed[..8] + trailer` (`srtp.rs:518-524`).
В коде честно написано: «совместимость с wire Discord не заявляется» (`srtp.rs:14-16`).

Что известно по оракулу: `secret_key` — 32 Б из op4 (`voice-deep.json`
`signaling.ops.4`, `session_description.secret_key 32Б`), режим
`aead_aes256_gcm_rtpsize`, GCM-тег 16 Б, хвост 4 Б LE. Схема nonce/AAD в дампе
**не снята** — прямо помечена как потерянная (`voice-deep.md` §4 п.3, `unknowns` п.3:
«нужен GCM-ключ из op4 + AAD/nonce-схема»). Наш HKDF-вариант — изобретение, а не RE.

Указатель для RE-задачи: строки нативного движка
`RE/native-re/raw/discord_voice/rabin2-strings.txt` — `xsalsa20_poly1305_lite_rtpsize`
(стр. 2575), `aead_aes256_gcm` (2576), `aead_aes256_gcm_rtpsize` (2577),
`aead_xchacha20_poly1305_rtpsize` (2578), `decryptInvalidNonceCount` (1536),
`Decrypt failed, cannot process nonce:` (2717), `Failed to read truncated nonce` (2734),
`base_nonce` (4212), `INVALID_NONCE` / `INVALID_NONCE_SIZE` / `UNSUPPORTED_NONCE_SIZE`
(7800-7812). Это подтверждает, что схема nonce живёт в нативном коде, а не в JS;
дальше нужен дизасм функций вокруг этих строк (REA/ghidra по самому `.node`, в дампе
его нет — только текстовые выгрузки).

Последствие: расшифровать эталонные пакеты нашим контекстом нельзя даже с ключом;
отправленные пакеты сервер отбросит. Это блокер V-04 и, через него, V-05/V-08.
Нужна отдельная RE-задача: строки/функции `discord_voice.node` (в дампе есть
`native-re.json`, модуль `discord_voice.node`, 126 МБ) + один живой пакет с известным
ключом (тестовый аккаунт, свой голос) для проверки гипотезы.

### 6.3 F-VC-02 (P1). Нет голосовой WS-сессии

В `discord-voice` есть парсеры (`parse_frame`), UDP-примитивы, reconnect-политика,
но **нет** клиента сигнализации. Следствие — не отправляются:

- op3 heartbeat: клиент для `v>=8` шлёт `{t: <ms>, seq_ack: lastRecvSeqNum ?? -1}`
  (бандл @2 400 248, `sendHeartbeat`); у нас нет ни билдера, ни расписания;
- op7 resume: `{token, session_id, server_id, channel_id, seq_ack}` (там же);
- op16 `{}` после ready (клиент: `serverVersion>=6 && this.send(16,{})`,
  `handleReady`, @2 400 248) — у нас op16 только читается как `Other`;
- op5 speaking (не наблюдался в дампе, но нужен для индикатора речи; в таблице
  опкодов RTC он есть).

`reconnect.rs` умеет только backoff; `regions.rs` — ранжирование; сессии, которая
связывает это с сокетом, нет. V-01 в PARITY помечен «принят `session.rs`», но в этом
дереве файла нет (`find rust -name '*session*'` → только `tests/gateway_session.rs`).
Это надо сверить с Team Lead: либо срез не влит в worktree, либо приёмка была по
другой ветке.

### 6.4 F-VC-03 (P1). Select Protocol теряет поля кодека и `rtc_connection_id`

Живой op1 (`voice/rtc1-frames.jsonl`) для каждого кодека содержит
`rtx_payload_type`, `encode`, `decode`, а на верхнем уровне — `rtc_connection_id`
(UUID) и `experiments:["fixed_keyframe_interval"]`.

Наш `Codec` (`lib.rs:206-211`) знает только `name/type/priority/payload_type`;
`to_json` (`lib.rs:226-253`) не пишет `rtx_payload_type/encode/decode`,
`rtc_connection_id` и `experiments`. Для аудио (opus PT120) сервер, вероятно, примет,
но видео/скриншер (`AV1 pt101 + rtx 102`) без rtx-полей не заведётся, а без
`rtc_connection_id` ломается корреляция сессии на стороне SFU (поле появилось в
клиенте 1.0.162, его наличие в живом кадре — факт).

### 6.5 F-VC-04 (P2). Ротация сокета шлёт незашифрованный RR

`udp.rs:418-427` `rotate_if_due` → `rotate()` → `rotate_impl(None)` → `send_empty_rr`,
то есть 8-байтный plaintext RR (`udp.rs:431-436`). Оракул: после ротации клиент шлёт
**28-байтный SRTCP-wrapped** пустой RR (`voice-deep.json`
`udp.rtcp.rr_client_empty`: «шлётся сразу после ротации», layout 8+16+4). Метод
`rotate_srtcp` есть, но в планировщик не подключён. Сервер такой RR, скорее всего,
проигнорирует; статистика и «живость» сокета поедут.

### 6.6 F-VC-05 (P2). RTCP: только парсер

`rtcp.rs` разбирает compound (`parse_compound`, константы PT/SR/RR/SDES/BYE/APP), но
не строит SR/RR и не держит тайминги. Оракул (`voice-deep.json` `udp.rtcp`):
SR 60 Б (PT200, len=6, 20 Б шифрованного sender info + tag + счётчик), интервалы
min/median/max 2998/5231/6326 мс, 22 шт. за сессию; серверный RR 52 Б каждые 1001 мс,
102 шт.; клиентских receiver reports 0. Без генерации SR мы не отдаём серверу
свою статистику — для одного участника не критично, для N — джиттер-буфер и потери
не сойдутся.

### 6.7 F-VC-06/07/08 (P3)

- `udp.rs:166-170`: в discovery-request пишется локальный порт в байты 72..73.
  Оракул описывает request как «74 Б: [u16=1][u16=0x0046][u32 ssrc][64Б нулей]» и не
  фиксирует последние 2 байта; наш выбор — не подтверждён. Нужна одна строка из
  `voice/strace-media-s3.jsonl` (там полные байты UDP).
- `lib.rs:42`: `VOICE_HEARTBEAT_INTERVAL_MS = 13_750` — константа, а HELLO
  (`lib.rs:118`) хранит `heartbeat_interval_ms`, который нигде не используется.
  Клиент берёт интервал из HELLO (и умножает на модификатор, бандл @2 399 739).
- `VoiceOp` (`lib.rs:55-79`) знает 0/1/2/3/4/6/8; op5 (speaking), op7 (resume),
  op11-13, op21-22, op24-32 (DAVE/MLS) — `Unknown`. Для парсинга это допустимо,
  для отправки — нет (см. F-VC-02).

---

## 7. Платформа и backend

Наш стек — Tauri, эталон — Electron; сверка поведенческая, не «канал в канал».
Источник: `main-arch.json` §5-§10, `backend-ipc.json` (168 каналов, 152 обработчика,
129 вызовов из renderer).

### 7.1 F-PL-01 (P1, продуктовый). Трей/бейдж/меню отсутствуют

Эталон: `Tray` с шестью иконками (`tray`, `tray-unread`, `tray-connected`,
`tray-speaking`, `tray-muted`, `tray-deafened`), стабильный GUID канала
(`699e423d-…` для stable), меню Open/Mute/Deafen/Voice Settings/Check for
Updates/Acknowledgements/Quit, клик по трею (main-arch §9).
У нас нет трея вообще (PL-01 в PARITY, `discord-platform` не содержит tray-модуля).
Для «клиента как Discord» это ежедневная базовая функция, а не P3.

### 7.2 F-PL-02 (P2). Deep link только `discord:`

`deep_link.rs:126-133` принимает схему `discord` (регистронезависимо), всё прочее —
`WrongScheme`. Эталон регистрирует ещё `disclip://` как privileged-протокол до ready
(`DISCORD_CLIP_PROTOCOL="disclip"`, main-arch §9). Плюс `main-arch` фиксирует
`extractUrlFromArgs` (`--url` после `--`), `second-instance`, `open-url` →
`openOrQueueUrl` → `handleOpenUrl` → `MAIN_WINDOW_PATH`; у нас парсер есть
(`deep_link.rs:186-228`), но очереди/wiring окна нет (PL-03).

### 7.3 F-PL-03 (P2). Уведомления не связаны с UI и не возвращают действие

`notifications.rs` реализует D-Bus `Notify` с hints/actions и слушает `ActionInvoked`
(`notifications.rs:221-300`), но из UI никто не вызывает и `gateway://dispatch` не
превращается в уведомление. Эталонная цепочка A (`DISCORD_NOTIFICATIONS_SEND_NOTIFICATION`
→ native `discord_notifications` → `DISCORD_NOTIFICATIONS_RECEIVED_RESPONSE` с
`action/identifier/userText/fallbackDeepLink`) на Linux вообще не грузит модуль
(«unsupportedStatus(process.platform)»), то есть наш D-Bus-путь — не паритет с
модулем, а замена. Это допустимо, но тогда нужен свой контракт «клик → канал».

### 7.4 F-PL-04 (P3). Апдейтер без сети

`discord-updater` содержит только `manifest.rs` (подпись 64 Б, sha256) и `plan.rs`;
URL-ов нет. Эталон: `NEW_UPDATE_ENDPOINT=https://updates.discord.com/`, legacy
`<endpoint>/modules/<channel>`, `<endpoint>/updates/<channel>?platform=linux&version=…`,
Linux-загрузка `https://discord.com/api/download/<channel>?platform=linux&format=…`
(main-arch §5). Пока апдейтер — только проверка манифеста.

### 7.5 F-PL-05 (P3). Токен at-rest: libsecret вместо safeStorage

Эталон шифрует токен через `safeStorage.encryptString` с префиксом `dQw4w9WgXcQ:` в
localStorage (`rest-static.md` §4). У нас `keyring.rs` (Secret Service) и
`ffi_secret.rs` (FFI-обёртка). Поведенчески лучше, но миграция с существующего
профиля Discord не совпадёт; для импорта токена нужен отдельный путь.

---

## 8. Отсутствующие REST-эндпоинты (каталог)

Источник — `rest-static.json` (648 REST-записей) + 81 запись, подтверждённая HTTP-кэшем.
Перечислены только те, что нужны функциям из PARITY или уже обещаны UI.

| # | Метод и путь | Зачем | Оракул | У нас |
|---|---|---|---|---|
| E-01 | `GET /channels/{id}/messages?limit&before&after&around` | история канала | `MESSAGES.cacheEvidence` (`?limit=12`), `rest-d02.json` 11 вызовов | нет |
| E-02 | `GET /channels/{id}/messages/pins?limit=25` | пины | `PINS.cacheEvidence` | маршрут есть, IPC нет |
| E-03 | `GET /channels/{id}/threads/search?archived&sort_by&sort_order&limit&tag_setting&offset` | ветки | `THREAD_SEARCH.cacheEvidence` | маршрут есть, IPC/query нет |
| E-04 | `GET /users/{id}/profile?type=popout&with_mutual_guilds&with_mutual_friends&with_mutual_friends_count` | профиль-поповер | `rest-static.json` (`calls:9`) | нет |
| E-05 | `POST /channels/{id}/attachments` → `PUT upload_url` → `POST /messages` | вложения | `ATTACH-RE.md`, PARITY AT-01 | слоты есть, PUT нет |
| E-06 | `POST /attachments/refresh-urls` | refresh подписанных URL | `rest-static.json` | нет |
| E-07 | `POST /read-states/ack-bulk` | bulk-ack при скролле | `rest-static.json` | нет |
| E-08 | `GET`/`PATCH /users/@me/settings-proto/{type}` | синхронизация настроек | live `rest-d02.json` (3 вызова) + boot step 8 | нет |
| E-09 | `PATCH /users/@me` | профиль (имя/аватар/about) | `rest-static.json` `USER` | нет |
| E-10 | `GET /users/@me/connections` | связанные аккаунты | live (`calls:1`) | нет |
| E-11 | `GET`/`PUT /users/@me/notes/{id}` | заметки | live (`calls:4`) | нет |
| E-12 | `POST /users/@me/channels` | создать DM/группу | `rest-static.json` | нет |
| E-13 | `GET/PUT/DELETE /users/@me/relationships/{id}` | друзья | `rest-static.json` | нет |
| E-14 | `GET /users/@me/mentions` | inbox/упоминания | `rest-static.json` | нет |
| E-15 | `GET /users/@me/saved-messages` | закладки | live + `rest-d02.json` (404 на пустом) | нет |
| E-16 | `PATCH /users/@me/notification-settings` | уровень уведомлений | `rest-static.json` | нет |
| E-17 | `POST /interactions`, `GET /channels/{id}/messages/{id}/interaction-data` | кнопки/команды | `rest-static.json` | нет |
| E-18 | `GET /channels/{id}/polls/{id}/…`, `POST …/expire`, `PUT …/answers/@me` | опросы | `rest-static.json` | нет |
| E-19 | `PUT /channels/{id}/voice-status`, `PATCH /guilds/{id}/voice-states/@me` | голосовой статус | `rest-static.json` | нет |
| E-20 | `GET /streams/{key}/preview`, `PATCH /streams/{id}/stream` | превью стрима | live (`calls:28`!) | нет |
| E-21 | `GET /guilds/{id}/regions`, `GET /voice/regions` | регионы голоса | `rest-static.json` `GUILD`/regions | нет (READY geo) |
| E-22 | `POST /guilds/{id}/members-search` | поиск участников | `rest-static.json` | нет |
| E-23 | `GET /guilds/{id}/roles`, `PATCH/POST /guilds/{id}/roles` | роли | `rest-static.json` | нет |
| E-24 | `GET /guilds/{id}/audit-logs` | аудит | `rest-static.json` | нет |
| E-25 | `POST /guilds` (create), `POST /guilds/{id}/delete` | создание/удаление сервера | `rest-static.json` (`GUILDS` без метода) | нет |
| E-26 | `POST /channels/{id}/invites`, `GET /channels/{id}/invites`, `DELETE /invites/{code}` | список/отзыв инвайтов | `rest-static.json` | создание есть, список/отзыв нет |
| E-27 | `GET /auth/sessions`, `POST /auth/sessions/logout` | сессии/выход других устройств | `rest-static.json` (`calls:1`) | нет |
| E-28 | `POST /science`, `POST /metrics/v2` | телеметрия | `rest-static.json` boot 10-11 | осознанно нет |

---

## 9. Константы и тайминги: эталон ↔ наш код

| Величина | Эталон | Источник | У нас |
|---|---|---|---|
| Gateway URL/query | `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream` | `rest-d02.json` `ws` | `ws.rs:77-80` ✅ |
| Gateway heartbeat | 41 250 мс (из HELLO) | `gateway-protocol.json` `heartbeat.gatewayIntervalMs` | из HELLO `gateway.rs:382-393` ✅ |
| Первый heartbeat | `random()*interval` | бандл @5 016 157 | `seed % interval`, seed 0 ❌ F-GW-06 |
| Каденция heartbeat | фиксированный `setInterval` | бандл @5 016 157, замеры 41 250 | от ACK ❌ F-GW-06 |
| QoS heartbeat | op40 `{seq,qos}` | `gateway-protocol.md` §4, бандл `_sendHeartbeat` | `ws.rs:432-469` ✅ |
| Gateway backoff | 1 с … 60 с; после 4 неудач — сброс сессии, реконнект продолжается | `rest-static.md` §2, бандл @5 014 326 | 1…60 с, 8 попыток → Stop ⚠️ F-GW-07 |
| Resume-окно | 3 мин от последнего ACK | бандл `_doResumeOrIdentify` @5 019 509 | 30 с от разрыва ❌ F-GW-08 |
| Capabilities | 1 734 653 / 1 767 421 | бандл @4 996 701 | `identify.rs:18-21` ✅ |
| `compress` | `false` (zstd-stream) | бандл `_doIdentify` | `supervisor.rs:130` ✅ |
| Gateway close-стоп | только 4004 | бандл @5 014 326 | 4004, 4013, 4014 ⚠️ (ADR-2) |
| Voice RTC close-стоп | 4004, 4006, 4011, 4015; после 3 неудач — disconnect | бандл @2 400 573 | нет WS ❌ F-VC-02 |
| Voice heartbeat | 13 750 мс (HELLO) | `voice-deep.json` `heartbeat_interval_ms` | константа 13 750 ⚠️ F-VC-07 |
| Voice HELLO timeout | 20 с | бандл @2 394 981 | нет WS ❌ F-VC-02 |
| UDP discovery | 74 Б, type 1/2, len 0x0046, порт @72 | `voice-deep.json` `udp.discovery` | `udp.rs:8,110-199` ✅ (порт request ⚠️) |
| UDP keepalive | 5 000 мс, `1337cafe`/`1337f00d` + seq LE | `udp.keepalive` | `udp.rs:10-29` ✅ |
| Ротация медиа-сокета | 60 080 мс | `udp.socket_lifecycle.rotation_after_ms` | `udp.rs:11` ✅ |
| RTP | PT 120, 48 кГц, 20 мс, 960 тиков, ext `0xBEDE` | `udp.rtp` | `media/rtp.rs:14-35` ✅ |
| SRTP | GCM tag 16, хвост 4 Б LE, `0x80000000|n`, общий RTP+RTCP | `udp.rtp.layout`, `udp.rtcp` | layout ✅, ключ/nonce ❌ F-VC-01 |
| RTCP SR / RR | 60 Б / 52 Б сервер, 28 Б пустой клиентский | `udp.rtcp` | только парсер ⚠️ F-VC-05 |
| Лимит сообщения | 2 000 символов | клиент (const), PARITY M-02 | `messages.rs:17` ✅ |
| Лимиты вложений | 20 МиБ пол, 50/100 МиБ Tier2/3 | `ATTACH-RE.md`, PARITY AT-01 | `attachments.rs` (вне скоупа) ⚠️ |
| REST retry-статусы | `{502,504,507,598,599,522,523,524}` (опция `retries`) + `{429,503}` очередь по URL | бандл @10 692 340 | любой 5xx ❌ F-RS-03 |
| REST retry cap | 60 с, дефолт 5 с | `rest-static.md` §2 | 30 с / 250 мс ❌ F-RS-03 |
| `Accept-Language` | q-список из `navigator.languages` | бандл @1 002 368 | локаль как есть ❌ F-RS-04 |
| Search offset | шаг 25, `total_results` | `search.rs` + PARITY M-13 | любой offset ⚠️ F-RS-07 |
| Pins limit | 25 | `PINS.cacheEvidence` | нет вызова ❌ E-02 |
| Threads archived | обязательный `archived=true` | `THREAD_SEARCH.cacheEvidence` | нет query ❌ E-03 |

---

## 10. Реестр находок

Severity: P1 — ломает live-протокол или заявленную функцию; P2 — паритет/устойчивость
с измеримым эффектом; P3 — точность/полировка.
`conf`: `[L]` — живой захват, `[S]` — код/бандл, `[A]` — не подтверждено.

| ID | P | Что | Наш код | Оракул | conf |
|---|---|---|---|---|---|
| F-GW-01 | P1 | zstd держит 64 КиБ, теряет хвост; 44/113 термов | `zstd.rs:16-20`, `ws.rs:51-57` | `gateway-protocol.md` §3 | [L] |
| F-GW-02 | P1 | нет исходящих op3/4/8/14/37/18-22 | `gateway.rs:94-104` | бандл @4 996 243, @4 998 360-4 999 300 | [S] |
| F-GW-03 | P1 | мост dispatch → UI: 6 из 21 | `net.rs:113-120,249` | `gateway-protocol.json` `dispatchTypes` | [L]+[S] |
| F-GW-04 | P2 | IDENTIFY после HELLO; нет hello-timeout | `ws.rs:724,625-636` | `capture-ready.jsonl`/`capture-run2.jsonl` ts; бандл @2 394 981 | [L] (порядок), [S] (таймаут) |
| F-GW-05 | P2 | `client_state` = только `guild_versions` | `identify.rs:316-327` | бандл `_doIdentify` | [S] |
| F-GW-06 | P2 | jitter 0; каденция от ACK | `gateway.rs:395,408`; `supervisor.rs:138` | бандл @5 016 157; `heartbeat.sentPeriodic` | [S]+[L] |
| F-GW-07 | P2 | `max_reconnects=8` → Stop; клиент после 4 неудач сбрасывает сессию и продолжает | `supervisor.rs:136,333` | бандл `_handleClose` @5 014 326, `_reset` | [S] |
| F-GW-08 | P2 | окно RESUME 30 с vs 3 мин | `resume.rs:19` | бандл `_doResumeOrIdentify` @5 019 509 | [S] |
| F-GW-09 | P2 | IDENTIFY и SP — разные значения | `identify.rs:67-90` vs `identity.rs:249-293` | `rest-d02.json` `auth.superProperties` | [L]+[S] |
| F-GW-10 | P3 | `qos_token` 0A020800 vs 0A00; op1 ключи-атомы | `identify.rs:209-211`; `ws.rs:413-425,452-467` | бандл QosToken writer; `identify.rs:8-11` | [S] |
| F-RS-01 | P1 | история только из фикстур, GET не вызывается | `data.rs:1430`; grep `routes::find(Get,"/channels/{id}/messages")` = 0 | `MESSAGES.cacheEvidence` | [L] |
| F-RS-02 | P1 | 202 не обрабатывается | `rest_failure.rs:67`; `search.rs:227` | PARITY M-13; тело 202 — [A] | [A] |
| F-RS-03 | P2 | ретраи: статусы, 503, потолок | `rate_limit.rs:181-203`; `client.rs:210-218,367-370` | бандл @10 692 340, @10 692 774; `rest-static.md` §2 | [S] |
| F-RS-04 | P2 | `Accept-Language` q-список; нет Fingerprint/Installation/Routing | `client.rs:112-114,166` | бандл @1 002 368 | [S] |
| F-RS-05 | P2 | нет `User-Agent`/`Accept-Encoding` | `transport.rs:296-305` | `rest-d02.json` `auth.superProperties` UA | [L]+[S] |
| F-RS-06 | P2 | нет query-слоя в маршрутах | `routes.rs:44-52` | `rest-static.json` cacheEvidence | [L] |
| F-RS-07 | P2 | поиск без `search_session_id`/`search_query_id`/`include_nsfw` | `search.rs:204-217` | сырой `capture-run2.jsonl` | [L] |
| F-RS-08 | P3 | PUT реакции с телом `{}` | `reactions.rs:127-131` | `rest-d02.json` endpoint реакции | [L] |
| F-RS-09 | P3 | ack без `flags` | `messages.rs:387-393` | `rest-static.json` `MESSAGE_ACK.bodyKeys` | [S] |
| F-RS-10 | P3 | 49 маршрутов vs «47 из D-02»; search не из оракула | `routes.rs:1-9,71,128` | `rest-d02.json` (47), сырой захват | [L] |
| F-RS-11 | P3 | `Connection: close`, без keep-alive | `transport.rs:4,301` | `rest-static.md` §2 (superagent) | [S] |
| F-RS-12 | P3 | нет `x-ratelimit-scope`, bucket-reuse, очереди по URL | `rate_limit.rs:146-179` | бандл `A=Set([429,503])` | [S] |
| F-VC-01 | P1 | HKDF-ключ и seed-nonce вместо схемы Discord | `srtp.rs:44-46,214-226,518-532` | `voice-deep.md` §4 п.3 (схема не снята) | [L] (факт) |
| F-VC-02 | P1 | нет voice WS: op3/op7/op16/op5 | `discord-voice/src/lib.rs` (нет клиента) | бандл @2 400 248; `voice-deep.json` `signaling.ops` | [S]+[L] |
| F-VC-03 | P1 | op1 теряет `rtx_payload_type/encode/decode/rtc_connection_id/experiments` | `lib.rs:206-253` | `voice/rtc1-frames.jsonl` op1 | [L] |
| F-VC-04 | P2 | ротация шлёт 8-байтный plaintext RR | `udp.rs:404-436` | `voice-deep.json` `udp.rtcp.rr_client_empty` | [L] |
| F-VC-05 | P2 | нет генерации RTCP SR/RR и таймингов | `rtcp.rs` (только parse) | `udp.rtcp` (60/52/28 Б, интервалы) | [L] |
| F-VC-06 | P3 | порт в discovery-request (байты 72..73) | `udp.rs:166-170` | `udp.discovery` (не фиксирует) | [A] |
| F-VC-07 | P3 | интервал голоса — константа, HELLO игнорируется | `lib.rs:42,118` | `voice-deep.json`; бандл @2 399 739 | [L]+[S] |
| F-VC-08 | P3 | `VoiceOp` не знает 5/7/11-13/21/24-32 | `lib.rs:55-79` | `voice-deep.json` `signaling.ops`, `unknowns` | [L] |
| F-PL-01 | P1 | нет трея/бейджа/меню | нет модуля | `main-arch.json` §9 | [S] |
| F-PL-02 | P2 | deep link только `discord:` | `deep_link.rs:126-133` | `main-arch.json` §9 (`disclip`) | [S] |
| F-PL-03 | P2 | уведомления не связаны с UI/действием | `notifications.rs:221-300` | `backend-ipc.json` цепочка A | [S] |
| F-PL-04 | P3 | апдейтер без URL/сети | `discord-updater/src/*` | `main-arch.json` §5 | [S] |
| F-PL-05 | P3 | токен at-rest: libsecret vs safeStorage | `keyring.rs`, `ffi_secret.rs` | `rest-static.md` §4 | [S] |

Счёт: P1 = 9 (F-GW-01/02/03, F-RS-01/02, F-VC-01/02/03, F-PL-01),
P2 = 15 (F-GW-04…09 — 6, F-RS-03…07 — 5, F-VC-04/05 — 2, F-PL-02/03 — 2),
P3 = 11 (F-GW-10, F-RS-08…12 — 5, F-VC-06/07/08 — 3, F-PL-04/05 — 2). Итого 35.

---

## 11. Что этот отчёт не доказывает

1. **Тело ответа 202 поиска** не наблюдалось (в D-02 индексация уже завершилась).
   Обработку 202 надо подтвердить live-окном, а не выдумать поля.
2. **Форма объекта подписки op37** (`{subscriptions:{guild:{channels,members,…}}}`)
   выведена из кода клиента частично: опкод и обёртка подтверждены, внутренний объект
   собирается `SubscriptionManager` (бандл @9 112 505) — нужна отдельная RE-задача.
3. **Схема nonce/AAD SRTP** в дампе отсутствует (`voice-deep.md` §4 п.3); наш HKDF —
   гипотеза кода, не паритет. Проверять только расшифровкой живого пакета с известным
   ключом.
4. **Порядок IDENTIFY/HELLO** установлен по меткам CDP и длинам кадров (инференс
   `gateway-protocol.md` §4), а не по декоду исходящих (они замаскированы).
5. **Часть полей `rest-static` не подтверждена живьём** (`X-Fingerprint`,
   `X-Installation-ID`, `X-Routing-Key`) — `rest-d02.json` `unknowns U-08`; в отчёте
   они помечены как static-only.
6. **`client_state` из 8 полей** прочитан из минифицированного бандла; точные имена
   (`highest_last_message_id`, `read_state_version`, …) взяты дословно из объекта,
   но семантика каждого поля не проверялась.
7. Цифры частот диспатчей — union 7 сокетов на снимке 2026-10-19; живой `capture-run2`
   продолжал расти (`gateway-protocol.json` `meta.snapshot`).

---

*Отчёт подготовлен RE-воркером (слот RE-1). Приоритизированный план —
`../PM/BACKLOG-RE-BACKEND.md`. Код не менялся.*

---

## 12. Как перепроверить (без сборки продукта)

```bash
# 1. Целостность оракулов
cd /home/sanya/re-deep && md5sum README.md rest-d02.json rest-static.json \
  gateway-protocol.json gateway-schema.json backend-ipc.json main-arch.json \
  voice-deep.json decoded-index.json out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js

# 2. Таблица опкодов и тела исходящих кадров клиента
python3 - <<'PY'
b=open('/home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js','rb').read()
i=b.find(b'DISPATCH=0'); print(i); print(b[i-40:i+1500].decode('utf-8','replace'))
j=b.find(b'requestGuildMembers(e,t){'); print(b[j:j+600].decode('utf-8','replace'))
PY

# 3. Ретраи и retry-after
python3 - <<'PY'
b=open('/home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js','rb').read()
i=b.find(b'E=new Set([502,504,507,598,599,522,523,524])'); print(b[i-40:i+2000].decode('utf-8','replace'))
PY

# 4. Живые голосовые кадры (формы op0/1/2/3/4/6/8/16)
cat /home/sanya/re-deep/voice/rtc1-frames.jsonl

# 5. Сколько термов теряет наш zstd
#    эталон: 137 термов / 824 816 Б для 95974.1389
python3 - <<'PY'
import json
d=json.load(open('/home/sanya/re-deep/gateway-protocol.json'))
print([ (s['key'], s['terms'], s['decompressedBytes']) for s in d['zstd']['perSocket'] ])
PY

# 6. Частоты диспатчей (для приоритета моста)
python3 - <<'PY'
import json
d=json.load(open('/home/sanya/re-deep/gateway-protocol.json'))
print(d['opcodes']['dispatchTypes'])
PY
```

Проверка наших утверждений — grep'ом по репозиторию (номера строк в отчёте):

```bash
grep -n 'FORWARDED_DISPATCHES' rust/gui-spike/src/net.rs
grep -n 'jitter_seed: 0\|max_reconnects: 8' rust/crates/discord-core/src/gateway/supervisor.rs
grep -n 'LABEL_KEY\|nonce_seed' rust/crates/discord-voice/src/srtp.rs
grep -n 'empty_receiver_report' rust/crates/discord-voice/src/udp.rs
grep -n 'pub const ROUTES' rust/crates/discord-core/src/rest/routes.rs
grep -n 'etf_to_json\|FORWARDED' rust/gui-spike/src/net.rs
```
