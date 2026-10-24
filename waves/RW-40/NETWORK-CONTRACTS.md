# RW-40 NETWORK-CONTRACTS — guild lifecycle, voice, video, DAVE

Дата: 2026-10-24. Автор: RE-воркер G1/Network (subagent PM DeepSeek).
Задача: подтвердить сетевые контракты для `NETWORK-ROADMAP.md` §7 без кода, без сети и без окна.
Все источники — **только чтение** (`/home/sanya/re-deep/**`, `docs/re/**`). Ничего не запускалось:
ни Discord, ни cargo, ни tcpdump, ни live-окно.

## 0. Метод и система маркировки

### 0.1 Источники, на которые ссылается документ

| Код | Артефакт | Что даёт |
|---|---|---|
| `RS` | `/home/sanya/re-deep/rest-static.json` (381 655 Б, 904 записи) | статическая карта эндпоинтов из Enum клиента + call-site + HTTP-cache |
| `RS.stats` | `RS` → `.stats` | `confirmedByHttpCache: 81`, `confirmedByLiveCapture: 0`, `restEndpoints: 648` |
| `D02` | `docs/re/rest-d02.md` + `d02-snapshot/capture-*.snap.jsonl` | живой CDP-захват REST (47 эндпоинтов, 133 запроса) |
| `B` | `/home/sanya/re-deep/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` (12,4 МБ, минифицирован; строка 1 = 662 381 симв.) | **код клиента**: call-site, схемы запроса/ответа, таблицы опкодов |
| `VD` | `docs/re/voice-deep.md` | RTC/UDP/SRTP по трём живым голосовым сессиям |
| `RF` | `/home/sanya/re-deep/voice/rtc1-frames.jsonl`, `rtc2-frames.jsonl`, `rtc-summary.json` | сырые RTC-кадры (in/out, JSON + бинарные) |
| `EL1` | `/home/sanya/re-deep/voice/engine-log-session1.txt` (26 строк) | движковый лог сессии 1: подключение, кодеки, DAVE |
| `EL3` | `/home/sanya/re-deep/discord-run3.log` (95 КБ) | движковый лог: `updateVideoQuality`, MLS, disconnect |
| `GP` | `docs/re/gateway-protocol.md` | шлюз: диспатчи, READY, опкоды |
| `NR` | `docs/re/native-re.md`, `native-re/voice-features.txt` | `discord_voice.node`: MLS/DAVE, кодеки, фичи |

`B` минифицирован: номера строк для кода бесполезны (строка 1 — весь бандл), поэтому цитаты даны
**якорем** (уникальная подстрока) + фрагментом. Для JSON-дампов даны номера строк.

### 0.2 Маркировка утверждений

| Тег | Значение |
|---|---|
| **[L]** | подтверждено **живым** захватом (requestId/HTTP-cache/pcap/RTC-кадр) |
| **[S]** | подтверждено **кодом клиента** (`B`): конкретный call-site с телом/схемой |
| **[D]** | есть только в статическом дампе `RS` (Enum-запись), call-site не найден |
| **[M]** | модель/инференс по косвенным данным — не факт, в код не брать без проверки |
| **[A]** | не подтверждено, требуется окно/захват/решение |

### 0.3 Важное открытие: поле `confirmed` в дампе бесполезно

`RS.stats`: **`confirmed:false` стоит у всех 904 записей** (`jq '[.endpoints[].confirmed]|group_by(.)'`
→ `[{v:false,n:904}]`), при этом `confirmedByHttpCache: 81`. То есть `confirmed` — артефакт схемы
(«не подтверждено живым телом ответа»), а не оценка достоверности эндпоинта.

**Следствие для роадмапа:** формулировки вида «`POST /invites/{code}` — `confirmed:false`» в §7
некорректны как аргумент: тот же флаг стоит у `GET /channels/{id}/messages`, который **работает
и проверен live**. Достоверность надо брать из `cacheEvidence` (81 запись), call-site в `B` и `D02`.
В этом документе `confirmed` из `RS` не используется как признак.

---

## 1. Guild lifecycle

### 1.1 Сводная таблица

| # | Операция | Метод/путь | Тело/query | Ответ | Коды | Статус | Источник |
|---|---|---|---|---|---|---|---|
| 1 | Join по инвайту | `POST /invites/{code}` | `{session_id, invite_instance_id, received_installation_id}` (+`context`, `invite_guild_scheduled_event_id`) | объект invite (`guild_id`, `target_channel_id`, `target_message_id`, `guild_scheduled_event`) | не наблюдались; клиент ловит `USER_BANNED` | **[S]** | `B` anchor `INVITE_ACCEPT_SUCCESS`; `RS:4106` |
| 2 | Join как lurker (preview) | `PUT /guilds/{id}/members/@me?lurker=true` | `{}` | **204** | 204 ×2 | **[L]** | `D02:101,138,225`; requestId `ready\|57992.6989`, `run2\|95974.7409` |
| 3 | Join обычный (та же ручка) | `PUT /guilds/{id}/members/@me` | `{}`; query `{lurker, session_id, recommendation_load_id, location, from_directory}` | `{id, join_request?, show_verification_form?, welcome_screen?, approximate_presence_count?}` | 30001 `TOO_MANY_USER_GUILDS`, 30019 `GUILD_AT_CAPACITY`, `7e5` `USER_GUILD_JOIN_LARGE_GUILD_UNDERAGE_DISALLOWED` | **[S]** | `B` anchor `Rsh.GUILD_JOIN(e)`; `RS:774` |
| 4 | Leave | `DELETE /users/@me/guilds/{id}` | `{lurking: bool}` (`isLurking(id) \|\| isCurrentUserGuest(id)`) | тело не читается (только `.then()`) | [A] | **[S]** | `B` anchor `async leaveGuild(e)`; `RS:786` |
| 5 | Delete guild (владелец) | `POST /guilds/{id}/delete` | нет | не читается | [A] | **[S]** | `B` anchor `deleteGuild:e=>`; `RS:1784` |
| 6 | Список серверов | REST-ручки нет | — | из `READY.guilds[]` + `READY_SUPPLEMENTAL` | — | **[S]+[L]** | `B` (нет `GET /users/@me/guilds`); `GP:150,196-208` |
| 7 | Один сервер (refresh/preview) | `GET /guilds/{id}` (+`PATCH`) | — | объект гильдии | [A] | **[D]** | `RS` key `GUILD`, 9 call-site |
| 8 | Инвайты сервера | `GET /guilds/{id}/invites` | — | массив инвайтов (`GUILD_SETTINGS_LOADED_INVITES`) | [A] | **[S]** | `B` anchor `Rsh.GUILD_INSTANT_INVITES`; `RS:1072` |
| 9 | Инвайты канала | `GET /channels/{id}/invites` | — | массив (`CHANNEL_SETTINGS_LOADED_INVITES`) | [A] | **[S]** | `B` anchor `CHANNEL_SETTINGS_LOADED_INVITES`; `RS:2550` |
| 10 | Создать инвайт | `POST /channels/{id}/invites` | `{max_age, max_uses, temporary, unique, target_type, target_user_id, target_application_id}` | `{code, type, channel{id,name,type}, guild{id,name,icon}, expires_at, max_age, max_uses, uses, temporary}` | [A] | **[S]** | `B` anchor `async createInvite(e)` |
| 11 | Отозвать инвайт | `DELETE /invites/{code}` | — (аналитика `uses/max_uses/max_age`) | — | [A] | **[S]** | `B` anchor `INSTANT_INVITE_REVOKE_SUCCESS` |
| 12 | Резолв инвайта | `GET /invites/{code}` | `{inputValue, with_counts, with_expiration, guild_scheduled_event_id, target_channel_id, target_message_id, with_permissions, with_games, with_guild_experiments}` | invite-объект (см. 1.4) | `USER_BANNED` → `INVITE_RESOLVE_FAILURE(banned)` | **[S]** | `B` anchor `with_counts:!0,with_expiration:!0` |
| 13 | Поиск участников | `POST /guilds/{id}/members-search` | `{or_query, and_query, sort?}` | `{guild_id, members[], page_result_count, total_result_count}`; **202** `{retry_after}` при индексации | 202 (retry ≤3) | **[S]** | `B` anchor `Rsh.GUILD_MEMBER_SEARCH(e)`; `RS:8347` |
| 14 | Typing | `POST /channels/{id}/typing` | нет | **200** `{message_send_cooldown_ms, thread_create_cooldown_ms}` | 200 → `SLOWMODE_SET_COOLDOWN` | **[S]** | `B` anchor `Rsh.TYPING(t)`; `RS:2562` |
| 15 | Свой voice-state | `PATCH /guilds/{id}/voice-states/@me` | `{suppress, channel_id, self_video, self_stream}` / `{request_to_speak_timestamp, channel_id}` / `{suppress, request_to_speak_timestamp, channel_id}` | не читается | `STAGE_CHANNEL_USER_NOT_ALLOWED_TO_SPEAK` | **[S]** | `B` anchor `UPDATE_VOICE_STATE:function(e)`; `RS:7920` |
| 16 | Voice join (шлюз) | gateway op4 | `{guild_id, channel_id, self_mute, self_deaf, self_video, flags}` + опц. `preferred_region(s)`, `tracks[{type,rid,quality}]` | `VOICE_STATE_UPDATE` + `VOICE_SERVER_UPDATE{token,guild_id,endpoint}` | [A] | **[S]** | `B` anchor `this.send(e2.VOICE_STATE_UPDATE,c)` |

### 1.2 Join по инвайту — что именно подтверждено

Call-site (`B`, anchor `dispatch({type:"INVITE_ACCEPT"`):

```js
f = B.default.getSessionId();
p = Y.A.getReceivedInstallationIdForInviteCode(A);   // может быть null
... _.Bo.post({url: ei.Rsh.INVITE(A), context: g, oldFormErrors: !0,
      body:{session_id:f, invite_instance_id:c.invite_instance_id, received_installation_id:p}})
   .then(async e => { dispatch({type:"INVITE_ACCEPT_SUCCESS", invite:e.body, code:d});
      ... e.body.guild_id, e.body.target_channel_id, e.body.target_message_id ... })
```

- **[S]** путь `POST /invites/{code}` и ровно три поля тела; `session_id` = `getSessionId()`;
  `received_installation_id` = `getReceivedInstallationIdForInviteCode(code)` и **может быть `null`**
  (клиент чистит его после успеха: `clearReceivedInstallationIdForInviteCode`).
- **[S]** `context` — analytics-объект (`INVITE_MODAL_OPEN`/`INVITE_ACCEPT`), а не поля Discord;
  в `context` подмешивается `invite_guild_scheduled_event_id`.
- **[S]** ответ — объект invite; из него берутся `guild_id`, `target_channel_id`, `target_message_id`,
  `guild_scheduled_event`.
- **Неизвестно [A]:** HTTP-коды (особенно 400/403/404/429), тело ошибки, TTL `received_installation_id`,
  обязательность `invite_instance_id`, поведение при `show_verification_form`/`join_request`
  именно для invite-flow (эти поля читаются в flow `GUILD_JOIN`, но не в accept-invite).
- **Не наблюдалось live:** в `D02` эндпоинта `POST /invites/*` нет вообще
  (`jq` по снимкам: 47 уникальных путей, invite-ручек — 0).

### 1.3 Join/leave: практический вывод для N1-01

- Для «войти по инвайту» нужен `POST /invites/{code}` — контракт тела есть **[S]**, live-подтверждения нет.
- Для «перейти в сервер из превью» уже **есть live-подтверждение** `PUT /guilds/{id}/members/@me`
  с `{}` и **204** — это самый дешёвый первый шаг N1-01 (его можно кодировать и проверять офлайн
  по фикстуре 204, а live-проверка — в окне 1).
- Leave: `DELETE /users/@me/guilds/{id}` + `{lurking:boolean}` **[S]**; ответ не используется →
  в коде достаточно статуса. Коды и точная семантика `lurking` (гость/lurker) — **[A]**.
- **Опасность:** `{lurking}` вычисляется как `isLurking(guild) || isCurrentUserGuest(guild)`.
  Если клиент не различает lurker/guest так же, он может либо выйти из гильдии «как lurker»,
  либо получить 403. Нужна фикстура минимум на 204/403.
- «Список серверов» через REST — **не делать**: в бандле нет `GET /users/@me/guilds`
  (есть только `/users/@me/guilds/{id}`, `.../settings`, `.../integration-application-ids`,
  `.../join-request-guilds`). Источник списка — `READY.guilds[]` (7 гильдий в захвате) и
  `READY_SUPPLEMENTAL`; `GUILD_CREATE` в 7 шлюзовых сессиях **не наблюдался ни разу** (`GP:194`).

### 1.4 Форма invite-объекта (как её парсит клиент)

`B`, anchor `invite:function(e){let{approximate_member_count`:
`{code, state, approximate_member_count, approximate_presence_count, target_type, target_user,
target_application, type, channel, guild, is_nickname_changeable, flags, expires_at}`.

Схема ответа create-invite (`B`, anchor `max_age:e.number().optional(),max_uses`):
`{code, type?, channel?{id,name,type}, guild?{id,name,icon}, expires_at, max_age?, max_uses?, uses?, temporary?}`.
`expires_at` — `w9(e)` (nullable-схема), т.е. **может быть `null`** у бессрочного инвайта.

### 1.5 members-search — детали (P2, N1-05)

Тело собирается из фильтров (`B`, anchor `usernames:{or_query:e}`):

```
{ or_query:  {usernames:{or_query:[...]}, user_id:{or_query:[...]}},
  and_query: {source_invite_code:{or_query:[...]}, join_source_type:{or_query:[...]},
              unusual_dm_activity_until:{range:{gte}}, communication_disabled_until:{range:{gte}},
              unusual_account_activity, automod_quarantined_username},
  sort?: <enum>   // default ORDER_BY_GUILD_JOINED_AT_DESC
}
```

- **[S]** `202` означает «индекс ещё строится»: клиент читает `retry_after`, ждёт `retry_after` секунд,
  повторяет; максимум **3** повтора, иначе `Error("Unable to search guild members after max retries")`.
- **[S]** ответ: `{guild_id, members:[{member, source_invite_code, join_source_type,
  join_source_application_id, join_source_channel_id, inviter_id?}], page_result_count, total_result_count}`.
- **Gap [A]:** в найденном call-site **нет** `limit`/`page_index`/`cursor` — пагинация запроса
  не восстановлена; неизвестно, чем ограничена страница и как листать. Также неизвестны права
  (mod-view/большие гильдии) и коды 403/429.

### 1.6 Typing — детали (N1-04)

- **[S]** `POST` без тела; `200` → `{message_send_cooldown_ms?, thread_create_cooldown_ms?}` →
  событие `SLOWMODE_SET_COOLDOWN` (тип `SendMessage`/`CreateThread`).
- **[S]** клиентский троттлинг: отправка разрешена, если `now > prevSend + 0.8*E`; `TYPING_STOP`
  планируется через `E` мс; локальный guard «не более 5 каналов в состоянии typing».
- **Gap [A]:** константа `E` в минифицированном модуле не восстановлена (не нашлась в 20 КБ контекста);
  серверный TTL (`TYPING_START` истекает сам) не измерялся. Для тестов брать TTL как параметр, не хардкодить.

---

## 2. Голос: RTC-сигнализация, UDP, SRTP

### 2.1 Подключение

- **[L]** `wss://c-<region>-<hash>.discord.media:<port>/` — из трёх сессий (`VD:25,43`): порты
  `2053`, `2096`; в логе сессии 1 — `8443` (`EL1:5`). Регион в имени хоста разный
  (`c-fra19-…`, `c-arn02-…`, `c-fra10-…`).
- **[S]** версия в query **условная**: `v=10`, если включён конфиг `sfu-update` **и**
  `mediaEngine.supports(UDP_ENDPOINT_UPDATE)`, иначе `v=9` (`B`, anchor `supportsSfuUpdate:n`).
  В захвате все три сессии — `v=9` (совпадает: фича не включена).
- **[S]** `helloTimeout` → `handleClose(false, 0, "… did not receive OP_HELLO in time")`.
- **[L]** порядок (по `EL1` + `RF`): WS connect → **op8 Hello** → клиент **op0 Identify** →
  **op1 Select Protocol** → **op2 Ready** → **op4 Session Description** → медиа.
  `EL1:5-25`: `Connecting… → AUTHENTICATING → RTC_CONNECTING → Creating connection with audio ssrc
  → Connected → codecs → Sending UDP info → media-session-id → DAVE init → sink wants`.
  В `RF` порядок кадров **не причинный**: файл пишется по направлениям (сначала все `in`, потом `out`),
  поэтому op8 стоит раньше op0. Для контракта опираться на код/лог, не на порядок строк файла.

### 2.2 Опкоды RTC: полная карта из кода клиента

`B`, anchor `switch(t){case 8:this.clearHelloTimeout()` — обработчик входящих:

| op | Направление | Содержимое/поля | Статус |
|---|---|---|---|
| 8 | in | `{v, heartbeat_interval}`; сессия: `v:8, heartbeat_interval:13750` | **[L]** `RF` |
| 0 | out | Identify: `{server_id, channel_id, user_id, session_id, token, max_dave_protocol_version:1, video:true, streams:[{type:"video",rid:"100",quality:100}]}` | **[L]** `RF` rtc1/rtc2 |
| 1 | out | Select Protocol: `{protocol:"udp", data:{address,port,mode}, address, port, mode, codecs[], rtc_connection_id, experiments}` | **[L]** `RF` |
| 2 | in | Ready: `{ssrc, port, ip, modes[], experiments[], streams:[{type,ssrc,rtx_ssrc,rid,quality,active}]}` | **[L]** `RF` |
| 4 | in | Session Description: `{mode, secret_key, media_session_id, dave_protocol_version, secure_frames_version, audio_codec, video_codec}` (+ опц. `bandwidth_estimation_experiment`, `keyframe_interval`, либо `sdp`) | **[L]** `RF`, `VD:33` |
| 5 | in | `{user_id, ssrc, speaking}` (bool или число) → `speaking` | [S] |
| 6 | in | heartbeat ACK `{t}` | **[L]** `RF` |
| 7 | out | RESUME: `{token, session_id, server_id, channel_id, seq_ack}` | [S] |
| 9 | in | RESUMED | [S] |
| 11 | in | `{user_ids}` → client-connect | [S] |
| 12 | in | `{user_id, audio_ssrc, video_ssrc, streams[]}` → video | [S] |
| 13 | in | `{user_id}` → client-disconnect | [S] |
| 14 | in | `{audio_codec?, video_codec?, media_session_id?, keyframe_interval?}` → обновление кодеков | [S] |
| 15 | in | media sink wants: `{"any":100}` | **[L]** `RF`, `VD:34` |
| 16 | in/out | `{voice, rtc_worker}`; клиент шлёт `{op:16,d:{}}` | **[L]** `RF` |
| 18 | in | `{user_id, flags}` | [S] |
| 20 | in | `{user_id, platform}` | [S] |
| 21 | in | `{transition_id, protocol_version}` → secure-frames prepare-transition | [S] |
| 22 | in | `{transition_id}` → execute-transition | [S] |
| 24 | in | `{epoch, protocol_version}` → prepare-epoch | [S] |
| 25 | in | external sender package | [S] |
| 27 | in | MLS proposals | [S] |
| 29 | in (bin) | `[u16 BE transition/protocol][MLS payload]` → prepare-commit-transition | [S] |
| 30 | in (bin) | `[u16 BE][MLS payload]` → welcome | [S] |
| 33 | in | `sfu-update`; **условие [M]**: сервер шлёт только при `v=10` | [S] обработчик / [M] условие |
| 3 | out | heartbeat `{t, seq_ack}` каждые 13 750 мс | **[L]** `RF` (7 шт., дельта 13 750) |
| close | in | code **1000**, reason «Force Close» при чистом выходе | **[L]** `VD:37`, `VD:108` |
| close | out | `reconnect()` закрывает сокет кодом **4000**; неудачный resume → `disconnect(false, 4801, "Cannot resume connection.")` | [S] |
| bin | out | `sendBinary(op, payload)` = `[1 байт op][payload]` | [S] |

**Gap [A]:** бинарные MLS-кадры в `RF` сохранены без опкода (`in-bin` 100 Б, `out-bin` 394 Б +
sha256) — какой именно op (25/27/29/30) у них, из артефакта не восстановить. Опкоды 17, 19, 23, 26,
28, 31, 32 в разобранном `switch` отсутствуют → попадают в `default: "Unhandled op"`; их реальное
назначение не проверялось.

### 2.3 Select Protocol: таблица кодеков (одна для всех сессий)

**[L]** (`RF` rtc1, `RF` rtc2 — идентично, различается только `address/port/rtc_connection_id`):

| name | type | priority | payload_type | rtx_payload_type | enc/dec |
|---|---|---:|---:|---:|---|
| opus | audio | 1000 | **120** | — | — |
| AV1 | video | 1000 | **101** | 102 | +/+ |
| H265 | video | 2000 | **103** | 104 | +/+ |
| H264 | video | 3000 | **105** | 106 | +/+ |
| VP8 | video | 4000 | **107** | 108 | +/+ |

`mode` в `data`/корне — `aead_aes256_gcm_rtpsize` (совпал с выбранным на сервере).

### 2.4 Ready (op2) и Session Description (op4)

- **[L]** op2: `ssrc` (audio), `port`, `ip`, `modes:["aead_aes256_gcm_rtpsize",
  "aead_xchacha20_poly1305_rtpsize"]`, `experiments:["fixed_keyframe_interval"]`,
  `streams:[{type:"video", ssrc, rtx_ssrc, rid:"100", quality:100, active:false}]`.
  Сессия 1: ssrc 7142/7143/7144 порт 19334; сессия 2: 17843/17844/17845; сессия 3: 9208/9209/9210
  (`VD:46`) → правило **audio = N, video = N+1, rtx = N+2** подтверждено трижды и совпадает с кодом.
- **[L]** op4: `mode:"aead_aes256_gcm_rtpsize"`, `secret_key` (**32 Б**, вырезан), `media_session_id`,
  `dave_protocol_version:1`, `secure_frames_version:1`, `audio_codec:"opus"`, `video_codec:"AV1"`.
- **[S]** клиент допускает **два вида op4**: `sdp` (если пришёл) **или** `mode`+`secret_key`;
  ветка `sdp` в живых сессиях не встречалась.

### 2.5 UDP-транспорт (байтово)

| Что | Факт | Источник |
|---|---|---|
| Discovery req | 74 Б: `[u16 type=1][u16 len=0x46][u32 ssrc][64 Б нулей]` | **[L]** `VD:56`, `voice-udp3.rtp.json` (`len:74`) |
| Discovery resp | 74 Б: `[u16 type=2][u16 len=0x46][u32 ssrc][64 Б ASCII IP + NUL][u16 порт BE @72]` | **[L]** `VD:57` |
| Кандидаты | 2 сокета шлют discovery одновременно, побеждает первый ответ | **[L]** `VD:59` |
| Keepalive | 8 Б: клиент `13 37 ca fe 01 00 00 00`, сервер `13 37 f0 0d 01 00 00 00`; интервалы 5000 мс (4999–5002) | **[L]** `VD:62`, `voice-udp3.rtp.json` → `keepalive.intervalsMs` |
| Ротация сокета | `+60.08 с` после старта: новый `socket()`, discovery с тем же ssrc, новый NAT-порт, пустой RR; старый сокет молчит | **[L]** `VD:85-89` |
| Сокет | принадлежит процессу рендерера (`0.0.0.0:55599`, fd=44) | **[L]** `VD:89` |

**Gap [A]:** причина/триггер ротации (таймер в RTC-воркере?) не восстановлена; `udp_endpoint_update`
(op33/`v=10`) в живых сессиях не встречался, поэтому «ротация через op33» — **не подтверждено**.

### 2.6 SRTP/RTP

- **[L]** RTP: `V=2,P=0,X=1,CC=0`, `PT=120`, расширение `0xBEDE` (2 слова), хвост
  `[ciphertext][GCM tag 16 Б][4 Б LE-счётчик]`, счётчик `0x80000000 | n`, сквозной для RTP+RTCP.
- **[L]** каденция 20 мс (`RTP ts` дельты 960/1050/870, сумма 3840 на 4 кадра); один talkspurt
  ~200 мс (10 пакетов) → тишина (**DTX**: пакеты не шлются).
- **[L]** RTCP: `SR` (PT200) клиент→сервер 60 Б, интервалы 3.0–6.3 с (медиана 5.2 с);
  `RR` (PT201) сервер→клиент 52 Б, ровно 1/с; пустой `RR` 28 Б сразу после ротации.
- **[L]** opus-шифротекст в talkspurt — **3 Б** (comfort-noise/DTX), речи в тестовом канале не было.
- **[A]** первые 2 пакета talkspurt — 43 Б без валидного расширения и без трейлера-счётчика;
  гипотезы (прогрев/DTX без AEAD) не подтверждены (`VD:125`). В `voice-udp3.rtp.json` это
  класс `other-len43` → **не игнорировать при парсинге**: пакет 43 Б может не иметь трейлера.
- **[A]** тела `SR`/`RR` зашифрованы → потери/джиттер снаружи не читаются; для тестов jitter-буфера
  нужен собственный оракул (ключ из op4 + AAD/nonce-схема). Документ `VOICE-CRYPTO.md` прошлой волны
  **утрачен** (`VD:131`) — схему AAD/nonce надо восстанавливать заново.
- **[A]** SRTP-ключ и `media_session_id` в артефактах вырезаны (`REDACTED`) — live-проверка шифрования
  требует окна.

### 2.7 Что реально наблюдалось, а что gap

| Наблюдалось **[L]** | Не наблюдалось **[A]** |
|---|---|
| 3 голосовые сессии, op8/0/1/2/4/16/15/6/3 | op5 speaking (никто не говорил), op11/13 client-connect/disconnect |
| UDP discovery/keepalive/ротация/RTCP | входящее медиа от другого участника (канал был пуст) |
| SRTP-конверт (tag 16, счётчик, режим) | расшифровка/шифрование своим кодом, replay-window, key rotation |
| DAVE init (лог) | MLS-сессия с двумя участниками, epoch transition |
| видео-параметры (лог) | реальный RTP-видеопоток (камера/экран не включались) |

**Вывод:** голосовой live-сертификации **нет**: все сессии — «один в канале, микрофон молчит,
видео `active:false`». Любое утверждение «голос работает» пока недопустимо.

---

## 3. Видео / Go Live

### 3.1 Streams: rid/ssrc/quality

- **[S]** правило по умолчанию (`B`, anchor `_handleReady(e,t,n,i,r,a,s)`):
  если сервер не дал streams — клиент синтезирует
  `{type:VIDEO, rid:"100", ssrc: audio_ssrc+1, rtxSsrc: audio_ssrc+2, quality:100, active:false}`.
- **[S]** `getVideoStreamParameters()`:
  - камера (`DEFAULT`): `[{rid:"100", type:VIDEO, quality:eq.Y4}]`;
  - screen share: `[{rid:"100", type:SCREEN, quality:eq.Y4}]`;
  - **simulcast**: если `isSimulcastSupported()` **и** конфиг `enableSimulcast` — добавляется
    второй слой `{rid:"50", type:VIDEO, quality:eq.Cl}`.
- **[S]** идентификатор `rid` — строка (`"100"`, `"50"`), `quality` — число; **значения enum
  `Y4`/`Cl` в бандле не инлайнятся** → [A]: в захвате всегда `quality:100`, значение для `rid 50`
  неизвестно (вероятно 50, но это догадка — не использовать).
- **[L]** `EL3:334`: `encodingVideoWidth 1280, Height 720, FrameRate 30, captureVideoFrameRate 30,
  encodingVideoBitRate 2 500 000, min 150 000, max 2 500 000, remoteSinkWantsPixelCount 921 600,
  remoteSinkWantsMaxFramerate 20, streamParameters[{type:"video", active:false, rid:"100",
  ssrc, rtxSsrc, quality:100, maxBitrate:2 500 000, maxFrameRate:20,
  maxResolution:{type:"fixed",width:1280,height:720}, minBitrate:150 000, targetBitrate:0,
  maxPixelCount:921 600}]`.
- **[L]** `EL3:333,363`: `[ConnectionEventFramerateReducer] Reduced framerate after 15000 ms`
  и повторный `updateVideoQuality {remoteSinkWantsMaxFramerate:20}` → **адаптация к sink wants**
  (20 fps при 30 fps захвата) наблюдалась. То же в сессии 3: `EL3:577-578,605`.
- **[S]** op12 (video от другого участника) мапится 1:1:
  `{type: audio|test|screen|video, rid, ssrc, rtx_ssrc, active, quality, max_bitrate, max_framerate,
  max_resolution{type,width,height}}` (`B`, anchor `function eF(e){return e?.map`).
  Т.е. **screen и camera различаются полем `type`**, а не отдельным ssrc-пространством.

### 3.2 Кодеки

- **[L]** `EL1:12-15` + `EL3:337,341`: engine codecs — `AV1(dec+enc), H264, H265, VP8`;
  experimental — `AV1, H265`; audio — `opus`; encryption modes — 7 штук (`aead_aes256_gcm_rtpsize`,
  `aead_aes256_gcm`, `aead_xchacha20_poly1305_rtpsize`, `xsalsa20_poly1305_lite_rtpsize`,
  `xsalsa20_poly1305_lite`, `xsalsa20_poly1305_suffix`, `xsalsa20_poly1305`); выбран
  `aead_aes256_gcm_rtpsize` (`VD:100`).
- **[L]** сервер вернул `video_codec:"AV1"` (op4) → согласованный видео-кодек = AV1.
- **[S]** payload types: AV1 101/102(rtx), H265 103/104, H264 105/106, VP8 107/108.
- **[A]** неизвестно: negotiated resolution/bitrate потолки для **зрителя** (viewer), поддержка
  H265 зрителями, аппаратный энкод на этой машине (`isVaapiEnabled()` не проверялся).

### 3.3 Go Live: REST и шлюз

- **[L]** формат stream key подтверждён HTTP-cache: `guild:<guild_id>:<channel_id>:<user_id>`
  (`RS:7476` → `STREAM_PREVIEW`, 28 вызовов, пример
  `/streams/guild%3A<guild>%3A<channel>%3A<user>/preview?version=...`).
- **[S]** `GET|POST /streams/{key}/preview`, тело POST — `{thumbnail}`; в HTTP-cache 28 вызовов
  (`RS:7476`).
- **[S]** `POST /streams/{key}/notify` — `{region}` (`RS:7494`, 2 call-site).
- **[S]** `PATCH /streams/{key}/stream` — `{region}` (`RS`, key `STREAM`).
- **[S]** шлюзовые диспатчи (`B`, anchor `STREAM_SERVER_UPDATE"],e=>`):
  - `STREAM_CREATE`: `{stream_key, region, viewer_ids, rtc_server_id, rtc_channel_id, paused}`;
  - `STREAM_SERVER_UPDATE`: `{stream_key, endpoint, token}`;
  - `STREAM_UPDATE`: `{stream_key, region, viewer_ids, paused}`;
  - `STREAM_DELETE`: `{stream_key, unavailable, reason}`.
- **Важно:** в `GP:150-155` (1340 op0-событий) **ни одного `STREAM_*` не было** → все четыре
  события **[S] только по коду**, live-подтверждения нет.
- **[A]** неизвестны: как выбирается `rid`/quality для зрителей, как приходит `viewer_ids` при
  подключении зрителя, что означает `paused`, как считается `rtc_server_id/rtc_channel_id`.

### 3.4 Захват экрана/камеры/системного звука

- **[S]/[L]** `native_screenshare_picker` — JS вызывает `presentNativeScreenSharePicker`
  (`NR:576-580`); на Linux — **только Wayland** (`NR:362`).
- **[A]** `screen_previews` / `window_previews` (экспорты `getScreenPreviews/getWindowPreviews` есть)
  — **только win32** (`NR:362`) → на Linux превью экранов/окон **недоступны**, нужен свой путь
  (PipeWire/X11), это решение G5, не «готово из коробки».
- **[M]** системный звук: `soundshare`, `soundshare_loopback`, `screen_soundshare` — только строки
  фич; на Linux `soundshare_loopback` помечен win32-only. Реальный захват системного звука
  **не наблюдался**.
- **[M]** `gamescope_capture` требует gamescope+vaapi+PipeWire-сокет (`NR:362`).
- **[L]** камера в живых сессиях не включалась (`active:false`, `self_video` не выставлялся),
  устройства видео (`getVideoInputDevices`) не перечислялись.

### 3.5 `voice-features.txt` — классификация (52 флага)

Файл: `/home/sanya/re-deep/native-re/voice-features.txt` (52 уникальных флага),
дублирует список `features.declareSupported` (`NR:348-357`).

**[T] — подтверждено артефактом (лог/захват/конкретный вызов или символ с параметрами):**
`fixed_keyframe_interval` (op1/op2 + лог), `media_devices` (экспорты устройств + вызовы),
`media_video` (CodecSurvey/`getCodecCapabilities`), `simulcast`+`simulcast_bugfix` (код-путь
`rid:"50"` + `enableSimulcast`), `vaapi` (`isVaapiEnabled`, строки `vaapi`×24), `udp_endpoint_update`
(условие `?v=10`), `native_screenshare_picker` (вызов в JS), `mls_pairwise_fingerprints`
(MLS-callback + `PUT /voice/public-keys`), `video_background_filter` (MediaPipe + 2 `.tflite`),
`spatial_audio` (`isSpatialAudioEnabled` + mysofa/HRTF), `sidechain_compression`
(`setSidechainCompression`), `set_audio_device_by_id`/`set_video_device_by_id`,
`async_video_input_device_init`, `krisp_native_error` (лог `VOICE_DISCONNECT krisp_nc_model`),
`port_aware_latency_testing` (RTCLatencyTestManager в логе — но «port_aware» часть **[M]**).

**[M] — только объявление/символ, поведение не наблюдалось:**
`audio_codec_red` (RED **не** объявлен в op1 — только opus), `loopback`, `first_frame_callback`,
`image_quality_measurement`, `bandwidth_estimation_experiments`, `experiment_config`, `debug_logging`,
`remote_locus_network_control`, `remote_user_multi_stream`, `voice_multiple_connections`,
`voice_panning`, `voice_automatic_subsystem`, `voice_experimental_subsystem`, `voice_legacy_subsystem`,
`voice_subsystem_deferred_switch`, `offload_adm_controls`, `voice_bypass_system_audio_input_processing`,
`go_live_hardware`, `direct_video`, `electron_video`, `hybrid_video`, `soundshare`, `screen_soundshare`,
`screen_previews`, `window_previews`, `clips`, `clips_recording_ready_events`, `clips_thumbnail`,
`gamescope_capture`, `elevated_hook`, `wumpus_video`, `video_effects`, `soundshare_loopback`,
`audio_debug_state`.

**[A] — подмножество [M], у которого на Linux нет пути реализации (win32/macOS-only) либо
поведение вообще не наблюдалось:**
`screen_capture_kit` (macOS), `wumpus_video`/`hybrid_video`/`soundshare_loopback`/`screen_previews`/
`window_previews`/`video_effects`/`elevated_hook`/`clips*` (win32-only → на Linux недоступны),
реальный системный звук, реальный захват экрана, **любой** просмотр чужого стрима.

Итого: 17 **[T]** + 34 **[M]** + 1 только **[A]** (`screen_capture_kit`, macOS) = 52; из 34 **[M]**
девять дополнительно помечены **[A]** как недоступные на Linux. Непокрытых флагов нет (проверено
скриптом по списку и тексту раздела).

---

## 4. DAVE / MLS

### 4.1 Что видно в сигнализации

| Факт | Значение | Статус |
|---|---|---|
| Клиент объявляет | `max_dave_protocol_version: 1` в op0 | **[L]** `RF` |
| Сервер сообщает | `dave_protocol_version: 1`, `secure_frames_version: 1` в op4 | **[L]** `RF` |
| Состояние DAVE | `{"version":1,"epochAuthenticator":""}` (epoch пустой — группы ещё нет) | **[L]** `EL3:353,597` |
| MLS в логе | `Received MLS external sender package`, `Got MLS key package, sending to RTC server`, `Sending MLS key package` | **[L]** `EL3:352-355`, `EL1:23-25` |
| Бинарные кадры | `in-bin` 100 Б; `out-bin` 394 Б (sha256 `f3a8fc9e…`) — MLS-пейлоад, **опкод не записан** | **[L]+[A]** `RF`/`rtc-summary` |
| Клиентский метод | `getSupportedSecureFramesProtocolVersion()` (нативный `SupportedSecureFramesProtocolVersion`) | **[S]** `NR:320` |

### 4.2 Опкоды/транспорт DAVE

- **[S]** op21 `{transition_id, protocol_version}` → prepare-transition; op22 `{transition_id}` →
  execute-transition; op24 `{epoch, protocol_version}` → prepare-epoch; op25 external sender package;
  op27 proposals; op29/op30 — **бинарные**: `[u16 BE][MLS payload]` (prepare-commit / welcome).
- **[S]** исходящие бинарные кадры: `sendBinary(op, payload)` = `[1 Б op][payload]` —
  то есть MLS-кадры от клиента отделимы от JSON по первому байту.
- **[A]** какие именно из 25/27/29/30 слал клиент в живых сессиях — не восстановить (опкод в
  артефакте не сохранён). На шлюзе MLS/DAVE-опкоды (24/25/27/29/30/31/32) **не встречались**
  (`GP:265`), т.е. MLS идёт **только по RTC-сокету**.

### 4.3 Крипто-слой (нативно)

- **[S]/[T]** `discord_voice.node` содержит Cisco `mlspp` (RFC 9420): `KeyPackage`×253, `Commit`×202,
  `Welcome`×178, `GroupInfo`×158, `treekem`×128, `MLSMessage`×101 и колбэки
  `MLSCommitCallback`/`MLSWelcomeCallback`/`MLSProposalsCallback`/`MLSPairwiseFingerprintCallback`
  (`NR:237-251`).
- **[T]** отдельные шифровальщики кадров **на каждый кодек**:
  `discord::dave::codec_utils::ProcessFrameOpus/ProcessFrameVp8/Vp9/H264/H265/Av1`,
  `ValidateEncryptedFrame`, `BytesCoveringH264PPS` (`NR:242-246`) → DAVE шифрует **медиа-пейлоад
  внутри SRTP**, а не только сигнализацию. Это критично: «SRTP готов» ≠ «DAVE готов».
- **[S]** REST-идентичность MLS: `PUT /voice/public-keys` `{public_key, signature, key_version}`,
  `POST /voice/{id}/match-public-key` `{public_key, key_version}` (`RS:11076`, `RS:11090`, 2 call-site
  каждый, live не наблюдались); источник ключа — `getMLSSigningKey(sessionId, ...)` (`NR:567-572`).

### 4.4 Чего нет (blockers для N3-01..03)

1. **Пороговые версии**: известно только «1». При каких условиях DAVE **обязателен** (размер
   канала, флаг гильдии, версия клиента) — **[A]**.
2. **Epoch/ratchet**: как выводится ключ эпохи из MLS (key schedule) и как он ложится на
   SRTP-ключи (двойное шифрование или замена) — **[A]**.
3. **Ротация при join/leave**: какой op/порядок (proposals → commit → welcome vs epoch) на
   практике — **[A]** (нужны 2 клиента).
4. **Формат MLS-пейлоада**: 394 Б `out-bin` — единственный образец; как он соотносится с
   `KeyPackage` — **[A]**.
5. **Pairwise fingerprint UI**: фича есть, протокол обмена — **[A]**.

---

## 5. Блокеры RE (P0/P1/P2)

### P0 — без этого нельзя писать live-код

| ID | Гэп | Почему P0 | Как закрыть |
|---|---|---|---|
| **B-01** | `POST /invites/{code}`: коды ответов (200/204?), тело ошибки, обязательность `invite_instance_id`, TTL `received_installation_id` | N1-01 нельзя типизировать ошибки; сейчас известен только успешный shape | окно 1: join по одноразовому инвайту + 404/expired/429; либо фикстуры из поддержки |
| **B-02** | `DELETE /users/@me/guilds/{id}`: коды, ответ, семантика `{lurking}` | N1-01: неверный `lurking` → 403 или выход «не того» типа | окно 1: leave на тестовой гильдии (lurker и обычный) |
| **B-03** | Live READY/send (N0-01/02) не подтверждены | всё остальное опирается на живой шлюз | окно 1 (уже в плане) |
| **B-04** | Voice live: нет ни одного входящего медиа-пакета; нет расшифровки SRTP | «голос готов» объявлять нельзя; N2-01..03 без этого — парсеры | окно 2: два клиента, запись только своего микрофона (consent) |

### P1 — блокирует продуктовую часть N1/N2

| ID | Гэп | Что нужно |
|---|---|---|
| **B-05** | Кодовая пагинация `members-search` (`limit`/`page_index`/`cursor` не найдены) | N1-05: либо снять с живого запроса (devtools-захват запроса при поиске), либо отложить фичу |
| **B-06** | RTC `op5 speaking` + `op11/13` + входящий op12 не наблюдались | N2-06 (индикатор речи) и N3-10 (зритель) — только по коду, тесты на синтетике |
| **B-07** | Константа typing `E` (TTL/троттлинг) не восстановлена | N1-04: TTL вынести в конфиг, не хардкодить |
| **B-08** | SRTP AAD/nonce-схема (утерян `VOICE-CRYPTO.md`) | N2-03: без документа ключ из op4 не применить; восстановить по GCM-тест-векторам |
| **B-09** | Ротация UDP-сокета: триггер неизвестен (таймер? op33?) | N2-02: иначе после +60 с медиа «умирает» в проде |
| **B-10** | `quality` enum (`Y4`/`Cl`) и правила simulcast не инлайнятся | N3-06: rid 50/quality — только по захвату зрителя |

### P2 — можно отложить, но зафиксировать

| ID | Гэп |
|---|---|
| **B-11** | `STREAM_CREATE/SERVER_UPDATE/UPDATE/DELETE` не наблюдались live → Go Live (N3-07) только по коду |
| **B-12** | `GET /users/@me/guilds` отсутствует в клиенте: «refresh списка серверов» — только через READY/resume; `GUILD_CREATE` не наблюдался → сценарий «список обновился после join» непроверяем без окна |
| **B-13** | DAVE: пороги обязательности, epoch-вывод, ротация при join/leave, формат 394-Б кадра |
| **B-14** | 43-байтные первые 2 пакета talkspurt (нет трейлера) — парсер должен терпеть |
| **B-15** | Linux-видео: `screen_previews`/`window_previews`/`clips*` win32-only; путь захвата на Linux (PipeWire/X11) — решение G5, не RE |
| **B-16** | Системный звук: только строки фич (`soundshare*`), реального захвата нет |
| **B-17** | `POST /guilds` (создание сервера): Enum-запись есть (`GUILDS:"/guilds"`, `RS:1682`), **REST-call-site = 0**. Проверено по всем 3 628 бандлам: `Rsh.GUILDS`/`.GUILDS(` — 0 сетевых ссылок; 9 совпадений `.GUILDS` — это UI-перечисления (`sectionIndex`, оверлей), не сеть → не кодировать |

---

## 6. Итог в одну таблицу

| Область | Подтверждено | Не подтверждено |
|---|---|---|
| Join по инвайту | путь+тело+форма ответа **[S]** | коды, live, TTL installation id **[A]** |
| Join (lurker) | `PUT …/members/@me?lurker=true`, `{}`, **204** **[L]** | семантика lurker/guest, прочие коды |
| Leave | путь+тело `{lurking}` **[S]** | коды, live **[A]** |
| Список серверов | READY.guilds[] **[L]** | REST-ручки нет; `GUILD_CREATE` не наблюдался |
| Инвайты list/create/revoke | тела/схемы **[S]** | коды, TTL, live **[A]** |
| members-search | тело/ответ/202-retry **[S]** | пагинация, права, коды **[A]** |
| typing | путь, `200`+slowmode, троттлинг **[S]** | константа TTL, TTL события **[A]** |
| RTC сигнализация | op8/0/1/2/4/15/16/6/3 **[L]**, карта op0–33 **[S]** | op5/11/13, bin-опкоды MLS **[A]** |
| UDP/SRTP | discovery 74 Б, keepalive 5 с, ротация +60 с, PT120, GCM tag, счётчик **[L]** | расшифровка, AAD/nonce, триггер ротации **[A]** |
| Видео | rid 100/50, ssrc+1/+2, payload types 101–108, 720p30/2.5 Мбит, AV1 **[L]** | quality enum, зритель, реальный поток **[A]** |
| Go Live | stream key `guild:g:c:u` **[L]**, 4 gateway-события **[S]** | live-события, viewer flow **[A]** |
| DAVE | v1, op21/22/24/25/27/29/30, mlspp, per-codec шифрование **[L]+[S]** | пороги, epoch, 2-клиентный тест **[A]** |

---

## 7. Ограничения этого документа

1. Ничего не запускалось: ни окно, ни сеть, ни cargo; все выводы — из артефактов 2026-10-18/19.
2. `B` минифицирован: часть констант (typing `E`, enum `quality`) не восстановлена — это отмечено как gap,
   а не заполнено догадкой.
3. Порядок кадров в `voice/rtc*-frames.jsonl` не причинный (группировка по направлению), поэтому
   последовательность рукопожатия взята из `EL` и кода.
4. Токены, `secret_key`, `media_session_id`, session id и IP-адреса в этом документе вырезаны
   (`REDACTED`, `<MEDIA_IP>`, `<PUB_IP>`, `<LOCAL_IP>`).
5. Документ не заменяет live-сертификацию: всё, что помечено **[A]**, требует окна/захвата.
