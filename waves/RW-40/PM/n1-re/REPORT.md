# N1-RE — сетевые контракты: отчёт RE-воркера (G1/Network)

Дата: 2026-10-24. Роль: RE-воркер G1/Network (subagent PM DeepSeek).
Задача: подтвердить сетевые контракты для `waves/RW-40/NETWORK-ROADMAP.md` §7.
Режим: **read-only RE**, без кода продукта, без сети, без окна, без cargo/tcpdump.
Выход: `waves/RW-40/NETWORK-CONTRACTS.md` (519 строк).

## 1. Статус

**done (RE, документально).** Собраны и размечены контракты guild lifecycle, voice (RTC/UDP/SRTP),
video/Go Live, DAVE/MLS. Каждое утверждение имеет источник и тег достоверности
**[L]** live / **[S]** код клиента / **[D]** только дамп / **[M]** инференс / **[A]** не подтверждено.
`confirmed:false` из `rest-static.json` **не использовался** как признак (см. §3.1 — он стоит у всех 904 записей).

## 2. Что подтверждено

| Область | Подтверждение | Тег |
|---|---|---|
| Join (lurker/preview) | `PUT /guilds/{id}/members/@me?lurker=true`, тело `{}`, ответ **204** ×2 | **[L]** (`D02:101,138,225`, requestId `ready\|57992.6989`, `run2\|95974.7409`) |
| Join по инвайту | `POST /invites/{code}`, тело ровно `{session_id, invite_instance_id, received_installation_id}`, ответ = invite-объект | **[S]** (`B` anchor `INVITE_ACCEPT_SUCCESS`) |
| Join (обычный) | та же ручка `PUT …/members/@me`, query `{lurker, session_id, recommendation_load_id, location, from_directory}`, тело `{}`, ответ `{id, join_request?, show_verification_form?, welcome_screen?, approximate_presence_count?}`, коды 30001/30019/`7e5` | **[S]** |
| Leave | `DELETE /users/@me/guilds/{id}`, тело `{lurking: bool}`; ответ не читается | **[S]** |
| Удаление сервера | `POST /guilds/{id}/delete`, без тела | **[S]** |
| Список серверов | REST-ручки `GET /users/@me/guilds` **нет**; список = `READY.guilds[]` + `READY_SUPPLEMENTAL`; `GUILD_CREATE` не наблюдался (0/7 сессий) | **[S]+[L]** |
| Инвайты: list/create/revoke | `GET /guilds/{id}/invites`, `GET /channels/{id}/invites`, `POST /channels/{id}/invites` (поля + схема ответа), `DELETE /invites/{code}` | **[S]** |
| Резолв инвайта | `GET /invites/{code}` + 9 query-полей (`with_counts`, `with_expiration`, `with_permissions`, …) | **[S]** |
| members-search | `POST /guilds/{id}/members-search`: тело `{or_query, and_query, sort?}` с вложенными фильтрами; `202`+`retry_after` (3 повтора); ответ `{guild_id, members[], page_result_count, total_result_count}` | **[S]** |
| typing | `POST /channels/{id}/typing` без тела; `200` → `{message_send_cooldown_ms, thread_create_cooldown_ms}`; троттлинг `0.8·E` | **[S]** |
| Voice-state | `PATCH /guilds/{id}/voice-states/@me` (4 варианта тела, вкл. `self_video/self_stream`); gateway op4 `{guild_id, channel_id, self_mute, self_deaf, self_video, flags, tracks[]}` | **[S]** |
| RTC опкоды | полная карта 0–33 из кода клиента (op5 speaking, op7 resume, op9 resumed, op11/13, op12 video, op14 codecs, op18 flags, op20 platform, op21/22/24/25/27/29/30, op33 sfu) | **[S]** |
| RTC рукопожатие | op8/0/1/2/4/15/16/6/3 c реальными полями и таблицей кодеков (opus 120, AV1 101/102, H265 103/104, H264 105/106, VP8 107/108) | **[L]** (`RF`, `VD:25-51`) |
| UDP | discovery 74 Б (req/resp), keepalive 8 Б каждые 5000 мс, ротация сокета +60.08 с, 2 кандидата | **[L]** (`VD:55-89`, `voice-udp3.rtp.json`) |
| SRTP | `aead_aes256_gcm_rtpsize`, PT 120, хвост `[ct][GCM tag 16][4 Б LE counter с 0x80…]`, 20 мс, DTX | **[L]** (`VD:66-82`) |
| Видео-параметры | rid «100»/ssrc+1/rtx+2, 1280×720@30, 2.5 Мбит/с (min 150к, max 2.5М), sink wants 20 fps, адаптация через 15 с | **[L]** (`EL3:333-334,363`) |
| Видео-потоки | `getVideoStreamParameters` (rid 100 + rid 50 при simulcast), op12-маппинг stream-объекта, `type: video|screen|audio|test` | **[S]** |
| Go Live | stream key `guild:<guild>:<channel>:<user>` (28 HTTP-cache вызовов preview); REST preview/notify/stream; 4 gateway-события `STREAM_*` | **[L]** ключ / **[S]** остальное |
| DAVE | `max_dave_protocol_version:1` → `dave_protocol_version:1`, `secure_frames_version:1`; op21/22/24/25/27/29/30; бинарный фрейм `[1 Б op][payload]`; mlspp + per-codec шифрователи (Opus/VP8/VP9/H264/H265/AV1) | **[L]+[S]** |

## 3. Что НЕ подтверждено (и почему это важно)

### 3.1 Находка для PM: `confirmed` в дампе — мёртвое поле

`rest-static.json`: `confirmed:false` — у **всех 904** записей, при `stats.confirmedByHttpCache=81`
и `confirmedByLiveCapture=0`. Значит формулировки роадмапа «endpoint X — `confirmed:false`»
(§7 п.1, п.5) не являются оценкой: тот же флаг стоит у работающего `GET /channels/{id}/messages`.
**Рекомендация:** в `NETWORK-ROADMAP.md` §7 заменить «`confirmed:false`» на теги из
`NETWORK-CONTRACTS.md` **[L]/[S]/[D]/[A]**.

### 3.2 Гэпы (полный список с приоритетами — `NETWORK-CONTRACTS.md` §5)

| Приоритет | Гэпы | Итог для планирования |
|---|---|---|
| **P0** | B-01 коды `POST /invites`; B-02 коды leave + семантика `{lurking}`; B-03 live READY/send; B-04 voice live (нет входящего медиа, нет расшифровки SRTP) | N1-01 и N2-01..03 нельзя «закрыть» без окна |
| **P1** | B-05 пагинация members-search (`limit`/`cursor` не найдены); B-06 op5/11/13/12 входящие; B-07 константа typing `E`; B-08 AAD/nonce SRTP (документ утерян); B-09 триггер ротации UDP; B-10 enum `quality` (rid 50) | фичи возможны, но тесты — на синтетике/конфиге |
| **P2** | B-11 `STREAM_*` live; B-12 refresh списка серверов; B-13 DAVE-пороги/epoch/ротация; B-14 43-Б пакеты talkspurt; B-15 Linux-видео (win32-only превью); B-16 системный звук; B-17 `POST /guilds` без call-site | не кодировать по догадке; зафиксировать в дизайне |

### 3.3 Отдельно про видео на Linux

`screen_previews`/`window_previews`/`clips*`/`soundshare_loopback`/`video_effects`/`hybrid_video`
в `voice-features.txt` — **win32-only** (`NR:362`), `native_screenshare_picker` — только Wayland.
То есть «выбор окна с превью» на Linux **недоступен штатно**; это решение G5 (PipeWire/X11),
а не готовая фича. В роадмапе N3-04 стоит планировать как «свой захват», не как «включить фичу».

## 4. Артефакты и контрольные суммы

| Файл | Строк | md5 |
|---|---:|---|
| `waves/RW-40/NETWORK-CONTRACTS.md` | 519 | `79ed101289ac9630fbf5687b2238915b` |
| `waves/RW-40/PM/n1-re/REPORT.md` | — | см. `md5.txt` (считается после записи) |

Источники (read-only, не изменялись):

| Файл | md5/размер | Использование |
|---|---|---|
| `reference-oracle/rest-static.json` | 381 655 Б, 904 endpoints | пути/тела/`cacheEvidence` |
| `reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` | 12 410 287 Б | call-site, схемы, опкоды |
| `reference-oracle/d02-snapshot/capture-run2.snap.jsonl` | `abfe14b76cec1e559c3b45522bc44b7a` (`D02:46`) | requestId join |
| `reference-oracle/d02-snapshot/capture-ready.snap.jsonl` | `b43636da09a4a2c0f08d36b1ce66a46a` (`D02:48`) | requestId join |
| `reference-oracle/voice/rtc1-frames.jsonl` / `rtc2-frames.jsonl` | 3 370 / 3 217 Б | op0/1/2/4/16 |
| `reference-oracle/voice/engine-log-session1.txt` | 3 620 Б, 26 строк | кодек/DAVE |
| `reference-oracle/discord-run3.log` | 95 280 Б | video quality, MLS |
| `reference-oracle/native-re/voice-features.txt` | 1 029 Б, 52 флага | классификация [T]/[M]/[A] |

## 5. Следующие RE-шаги (по возрастанию стоимости)

1. **Без окна, сейчас (0.5–1 ч):** добить константы из бандла, которые не восстановились:
   typing `E`, enum `quality` (`Y4`/`Cl`), значения `USER_GUILD_JOIN_LARGE_GUILD_UNDERAGE_DISALLOWED`
   (в минифицированном виде `7e5`). Это чисто статический поиск, окно не нужно.
2. **Без окна:** восстановить SRTP AAD/nonce по схеме `aead_aes256_gcm_rtpsize` + 43-Б пакетам
   из `voice/strace-media-s3.jsonl` (полные байты уже есть) — закрывает B-08/B-14 частично.
3. **Окно 1 (уже в плане как N0-01/02):** вместе с READY/send снять `POST /invites/{code}`
   (успех + expired + 429) и `DELETE /users/@me/guilds/{id}` (lurker и обычный) → B-01/B-02.
4. **Окно 2 (голос, 2 клиента, consent):** входящий op12/op5, реальный SRTP-обмен, DAVE epoch →
   B-04/B-06/B-13. Запись только своего микрофона/экрана.
5. **Окно 3 (Go Live):** один стрим + один зритель → B-11/B-10 (quality/rid 50, viewer flow).
6. **Дизайн-вход:** передать G3 факт «Linux: screen_previews/window_previews недоступны» до
   `DESIGN/STREAM.md`, иначе дизайн будет опираться на несуществующий пикер.

## 6. Риски и ограничения

- Порядок кадров в `voice/rtc*-frames.jsonl` **не причинный** (группировка по направлению) —
  последовательность рукопожатия восстановлена по `EL1` и коду; не строить на файле таймлайн.
- Все три голосовые сессии — «один в канале, микрофон молчит, видео `active:false`».
  Утверждение «голос/видео работают» по-прежнему **недопустимо**.
- IP/ключи/`media_session_id`/session id в документах вырезаны (`<MEDIA_IP>`, `REDACTED`).
- В read-only деревьях (`reference-oracle/`, `docs/re`, `logs`, `functions`, `evidence`)
  ничего не изменялось; коммитов нет.
