# BACKLOG-RE-BACKEND — приоритизированные задачи по итогам RE-аудита (task-130)

Дата: 2026-10-30. Источник: `../RE-AUDIT/RE-GAP-BACKEND-PROTOCOL.md` (35 находок, 9 P1).
Задачи сформулированы так, чтобы их можно было отдать воркеру без дочитывания отчёта:
цель → что сделать → оракул → файлы (write-scope) → DoD → тест/гейт.

Общие правила (из `AGENTS.md`/`TEST-POLICY.md`):

- Код по контракту `[A]` (нет подтверждения) **не пишется** — сначала RE-задача.
  В этом бэклоге такие пункты помечены `GATE: RE`.
- Live-проверки — только в согласованное окно PM; в DoD это `E3` (окно есть) или
  `E3 нет` (offline-паритет).
- Тесты — red-first: сначала тест по оракулу (реальный файл/байты), затем код.
- Файлы `state/types.rs`, `state/reducer.rs`, `state/mod.rs`, `src/lib.rs`, `PLAN.md`,
  `STATUS.md`, `ROADMAP.md`, `TASKS.md`, `docs/map.md`, `waves/*` — только Team Lead.

---

## 0. Сводка

| Приоритет | Кол-во | Что закрывает |
|---|---:|---|
| P1 | 9 | live-протокол и заявленные функции: шлюз (zstd, исходящие опкоды, мост), история, поиск, SRTP, голосовая сессия, Select Protocol, трей |
| P2 | 15 | паритет и устойчивость: ретраи, заголовки, query-слой, идентичность, тайминги, RTCP, вложения, профиль, настройки, deep links, уведомления, DM |
| P3 | 8 | точность: тела запросов, каталог, keep-alive, поля 429, UDP-порт, enum опкодов, апдейтер, миграция токена |

Критический путь релиза: **P1-03 (zstd) → P1-02 (мост) → P1-04 (история)**, параллельно
**RE-P1-06 (SRTP) → P1-07 (voice WS) → P1-08 (Select Protocol)**. Остальные P1 не
блокируют друг друга.

---

## 1. P1 — блокеры

### BE-P1-01. Исходящие опкоды шлюза: presence, guild members, subscriptions

- **Цель:** приложение становится участником шлюза, а не только слушателем.
- **Описание:** добавить отправку op3 `{status,since,activities,afk}`, op8
  `{guild_id,query,limit,user_ids,presences}` (чанки `user_ids` по 100), op37
  `{subscriptions:{<guild_id>:<sub>}}` (чанки JSON ≤ 15 360 Б), op4
  `{guild_id,channel_id,self_mute,self_deaf,self_video,flags}` (+`tracks`).
  Op6/resume и op1/op40 уже есть. Op14 в веб-клиенте не используется — не делать.
  Форма `<sub>` (объект подписки) **не подтверждена** — вынести в RE-задачу
  `RE-P1-01a` (см. §4), до неё реализовать op3/op8/op4 (их тела подтверждены).
- **Оракул:** `RE/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js@4 996 243`
  (таблица опкодов), `@4 998 360` (`requestGuildMembers`), `@4 998 666`
  (`updateGuildSubscriptions`), `@4 954 008` (`GUILD_MEMBERS_REQUEST`),
  `@4 954 460` (`GUILD_SUBSCRIPTIONS_FLUSH`); `RE/gateway-protocol.json`
  `opcodes.dispatchTypes` (1048 `GUILD_MEMBER_LIST_UPDATE`, 11 `GUILD_MEMBERS_CHUNK`,
  107 `PRESENCE_UPDATE`).
- **Write-scope:** `rust/crates/discord-core/src/gateway.rs` (enum `Op` + билдеры
  кадров, новые `encode_presence_update`/`encode_request_members`/
  `encode_guild_subscriptions`/`encode_voice_state`), новый
  `rust/crates/discord-core/src/gateway/subscriptions.rs`,
  `rust/crates/discord-core/src/gateway/supervisor.rs` (API отправки),
  `rust/crates/discord-core/src/ws.rs` (канал/сокет для записи после handshake),
  `rust/crates/discord-core/tests/gateway_opcodes_out.rs` (новый),
  `rust/crates/discord-core/tests/gateway_supervisor.rs` (расширение — согласовать с TL).
- **DoD:**
  1. op3 уходит при смене статуса и сразу после READY (presence из IDENTIFY не
     дублируется дважды).
  2. op8 отправляется по запросу участников, `user_ids` чанкуется по 100.
  3. op37 отправляется при открытии канала/списка участников; чанк ≤ 15 360 Б.
  4. op4 отправляется при join/leave voice и при mute/deafen.
  5. Ни один кадр не уходит до WS-handshake; ошибка записи видна вызывающему.
- **Тест:** байтовые оракулы: собрать кадр и сверить с эталонной раскладкой ETF
  (ключи binary, значения по типам); для op8 — границы чанков 100/101.
- **Зависит:** BE-P1-03 (иначе ответные события не читаются). **GATE:** RE для `<sub>`.

### BE-P1-02. Мост dispatch → UI: все 21 тип, а не 6

- **Цель:** live-сообщения, правки, typing, голос, треды доходят до экранов.
- **Описание:** расширить `FORWARDED_DISPATCHES` до всех 21 типа из
  `DispatchKind::ALL` (или перейти на явную политику «тип → экран» с тестом на
  покрытие), убрать фильтр по списку либо сделать его производным от `DispatchKind`.
  Для `MESSAGE_CREATE/UPDATE/DELETE`, `MESSAGE_ACK`, `TYPING_START`,
  `VOICE_STATE_UPDATE`, `VOICE_SERVER_UPDATE`, `THREAD_LIST_SYNC`, `CHANNEL_INFO`
  проверить, что `d` сериализуется в JSON без потерь (сейчас `etf_to_json` умеет).
- **Оракул:** `RE/gateway-protocol.json` `opcodes.dispatchTypes` (21 тип, 1340 op0);
  `RE/gateway-schema.json` `events[]` (поля `d` по типам; id — отрицательные, брать
  только имена полей); `RE/docs` нет — `RE/README.md` §2.
- **Write-scope:** `rust/gui-spike/src/net.rs`, `rust/gui-spike/src/lib.rs`,
  `CONTRACTS/dispatch.md` (согласовать с TL), `rust/gui-spike/tests/net_contract.rs`,
  новый `rust/gui-spike/tests/dispatch_forwarding.rs`.
- **DoD:**
  1. Для каждого из 21 `DispatchKind` есть тест: событие доходит до канала
     `gateway://dispatch` (или явно помечено «не для UI» с причиной).
  2. Секреты вырезаются как раньше (`is_secret_key`), READY-редакция не регрессирует.
  3. Медленный UI не блокирует источник (`try_send`), счётчик отказов виден.
- **Тест:** оракул — реальные `d` из `RE/gateway/by-type/*.json` (если есть) или
  `RE/gateway-schema.json` имена полей; сверять список типов с `DispatchKind::ALL`.
- **Зависит:** нет. UI-маршрутизация — отдельная карточка UI-лейна.

### BE-P1-03. zstd: не терять хвост соединения

- **Цель:** 100% термов из потока, как у эталонного декодера.
- **Описание:** текущий `ruzstd`-подход отдаёт `распаковано − окно` и держит
  последние 64 КиБ до конца кадра, которого не будет. Варианты: (а) собственный
  инкрементальный декодер блоков zstd, отдающий все байты сразу; (б) вызывать
  `FrameDecoder` с принудительным flush окна (если API позволяет); (в) буферизовать
  кадр целиком и декодировать с `skip_frame`/повторным проходом. Выбрать по замеру
  на `RE/gw-proto/zstd-frames-95974_1389.hexl` (824 816 Б / 137 термов).
- **Оракул:** `RE/gateway-protocol.md` §3 (декодер без `end()`, полный вывод),
  `RE/gateway-protocol.json` `zstd.perSocket` (по сокетам: compressed/decompressed/terms);
  `RE/gw-proto/` (декоды 7 сокетов).
- **Write-scope:** `rust/crates/discord-codec/src/zstd.rs`,
  `rust/crates/discord-codec/tests/zstd_stream.rs` (если есть/новый),
  `rust/crates/discord-core/src/ws.rs` (только если нужен API flush).
- **DoD:**
  1. На эталонном сокете `95974.1389` пайп отдаёт **137** термов и 824 816 Б.
  2. `pending_bytes()` = 0 после последнего кадра (нет вечного хвоста).
  3. Ошибка декодера по-прежнему терминальна и типизирована.
- **Тест:** hexl/бинарь из `RE/gw-proto/` как фикстура (скопировать в
  `tests/data/` с md5 в шапке теста).
- **Зависит:** нет. **Риск:** смена крейта/алгоритма — согласовать с TL.

### BE-P1-04. История канала по REST (курсоры, лимиты, IPC)

- **Цель:** live-история вместо фикстур; пагинация вверх/вниз.
- **Описание:** реализовать `GET /channels/{id}/messages` с `limit` (1..=100) и
  `before`/`after`/`around`; ответ — массив сообщений + вычисляемые
  `hasMore/olderCursor`; подключить к Tauri-команде `messages_page` (сейчас читает
  файлы, `data.rs:1430`). Кэш не вводить; окно UI ≤ 500 строк оставить инвариантом UI.
- **Оракул:** `RE/rest-static.json` `MESSAGES.cacheEvidence` (`?limit=12`, 23 вызова);
  `RE/rest-d02.json` endpoint `GET /channels/{id}/messages` (11 вызовов, 200);
  `RE/gateway-schema.json` `MESSAGE_CREATE.fields` (набор полей сообщения).
- **Write-scope:** новый `rust/crates/discord-core/src/rest/history.rs`,
  `rust/crates/discord-core/src/rest/mod.rs`, `rust/crates/discord-core/src/rest/routes.rs`
  (query-поля — см. BE-P2-04), `rust/gui-spike/src/data.rs`,
  `rust/gui-spike/src/main.rs` (регистрация команды), тесты
  `rust/crates/discord-core/tests/rest_history.rs` (новый),
  `rust/gui-spike/tests/history_before_contract.rs` (расширить).
- **DoD:**
  1. `messages_page(channelId, before|after, limit)` делает ровно один GET и
     возвращает страницу в возрастающем порядке.
  2. `limit=0`, `limit=101`, нечисловой id, отрицательный курсор — `InvalidArg` до сети.
  3. Пустая страница — валидный ответ, не ошибка.
  4. 401/403/404/429/5xx типизированы (`RestFailure`).
- **Тест:** локальный HTTP-стаб (как в `rest_client.rs`), плюс оракул URL из
  `rest-static` (сверка query-строки).
- **Зависит:** BE-P2-04 (query-слой) — можно делать вместе.

### BE-P1-05. Поиск: 202, обязательные параметры, пагинация

- **Цель:** поиск работает на «холодном» сервере и не теряет сессию выдачи.
- **Описание:** (1) трактовать 202 как «индексация идёт»: ждать `retry_after`
  (тело/заголовок), повторять ≤ 5 раз, затем ошибка `Indexing`; (2) добавить
  `search_session_id` и `search_query_id` (UUID v4 на сессию/запрос), генерировать
  один раз на запрос и переиспользовать при пагинации; (3) `include_nsfw`;
  (4) валидировать `offset` кратным 25 (или явно разрешить любой и записать это).
- **Оракул:** сырой `RE/capture-run2.jsonl` (4 живых URL с `search_session_id`/
  `search_query_id`); `RE/rest-static.json` `SEARCH_GUILD`; форма 202 — **GATE: live**
  (в D-02 не наблюдалась, `RE/rest-d02.json` `unknowns`).
- **Write-scope:** `rust/gui-spike/src/search.rs`,
  `rust/crates/discord-core/src/rest/rate_limit.rs` (если 202 классифицируется там),
  `rust/gui-spike/tests/search_contract.rs` (расширить), новый
  `rust/gui-spike/tests/search_202.rs`.
- **DoD:**
  1. 202 с `retry_after` → повтор, не `InvalidResponse`.
  2. 5 подряд 202 → типизированная ошибка с `retry_after_ms`.
  3. В каждом URL присутствуют оба UUID; при пагинации они не меняются.
  4. `include_nsfw` передаётся, если задан.
- **Тест:** стаб отдаёт 202/202/200; проверка счётчика попыток и задержек (виртуальные
  часы/`tokio::time::pause`).
- **Зависит:** BE-P2-01 (общая политика ретраев) — можно параллельно.

### RE-P1-06 + BE-P1-06. SRTP: снять схему ключа/nonce, затем переписать

- **Цель:** медиапакеты шифруются так, как их ждёт SFU Discord.
- **Описание (RE):** схема `aead_aes256_gcm_rtpsize` в дампе отсутствует
  (`RE/voice-deep.md` §4 п.3). Нужно: (а) strings/функции `discord_voice.node`
  (в дампе `RE/native-re.json`, `RE/native-re/` — модуль 126 МБ) вокруг
  `aes256_gcm`/`rtpsize`/`nonce`; (б) один собственный медиапакет с известным ключом
  (тестовый аккаунт, свой микрофон) → проверить гипотезу расшифровкой.
  Гипотезы для проверки: ключ = 32 Б `secret_key` из op4 без KDF; nonce 12 Б =
  `00 00 00 00 | ssrc BE | counter LE`; AAD = RTP-заголовок целиком (включая ext).
- **Описание (код, после RE):** заменить HKDF-метки (`srtp.rs:44-46`) на прямую схему,
  nonce — на подтверждённую; сохранить общий счётчик RTP+RTCP и хвост 4 Б LE.
- **Оракул:** `RE/voice-deep.json` `udp.rtp.layout`, `udp.rtcp`, `engine.encryption_*`;
  `RE/voice/strace-media-s3.jsonl` (полные байты UDP, но ключ REDACTED);
  `RE/native-re.md` (строки модуля). **GATE: RE** — без схемы код не менять.
- **Write-scope:** `rust/crates/discord-voice/src/srtp.rs`,
  `rust/crates/discord-voice/src/crypto/kdf.rs` (если KDF удаляется),
  `rust/crates/discord-voice/tests/srtp_contract.rs`,
  новый `rust/crates/discord-voice/tests/srtp_oracle.rs` (вектор с известным ключом).
- **DoD:**
  1. Вектор «ключ + пакет → plaintext» из живого теста проходит.
  2. Наш шифрованный пакет расшифровывается независимой реализацией (или сервером в
     live-окне).
  3. Счётчик и хвост совпадают с эталонной раскладкой побайтно.
- **Зависит:** RE-задача. **E3:** обязателен.

### BE-P1-07. Голосовая WS-сессия: heartbeat, resume, op16

- **Цель:** после join канал остаётся живым и восстанавливается.
- **Описание:** реализовать клиент сигнализации: connect `wss://<endpoint>/?v=9`,
  разбор op8 → интервал, op3 `{t: ms, seq_ack}` каждые `heartbeat_interval`,
  op6 ACK обновляет `seq_ack`, op7 resume `{token,session_id,server_id,channel_id,seq_ack}`,
  op16 `{}` после ready (для `v>=6`), таймаут HELLO 20 с, backoff RTC-сокета
  (клиент: 1 с … 5 с, после 3 неудач — disconnect; стоп на 4004/4006/4011/4015 —
  бандл @2 400 573). Наш `reconnect.rs` (1/2/4/8/16/30 с) для RTC не подходит.
- **Оракул:** `RE/voice/rtc1-frames.jsonl`, `RE/voice/rtc2-frames.jsonl`
  (полные формы op3/op6/op16); `RE/out/bundles/...@2 399 739` (`handleHello`),
  `@2 400 248` (`sendHeartbeat`, `handleReady`, op7 resume), `@2 400 573`
  (`handleClose` RTC); `RE/voice-deep.json` `signaling.ops`.
- **Write-scope:** новый `rust/crates/discord-voice/src/session.rs`,
  `rust/crates/discord-voice/src/lib.rs`, `rust/crates/discord-voice/src/reconnect.rs`,
  `rust/crates/discord-voice/tests/session_contract.rs` (новый).
- **DoD:**
  1. На фикстуре `rtc1-frames` сессия проходит hello→identify→ready→session_description
     и не теряет ни одного кадра.
  2. op3 уходит ровно с интервалом из HELLO, `seq_ack` = последний входящий seq.
  3. Обрыв → op7 с теми же `session_id/server_id/channel_id`, затем ready/resumed.
  4. HELLO не пришёл за 20 с → типизированная ошибка, не вечное ожидание.
- **Тест:** запись/воспроизведение кадров (как `gateway_session.rs`), без сети.
- **Зависит:** BE-P1-06 (для медиа), но сигналинг можно сдавать раньше.

### BE-P1-08. Select Protocol: вернуть поля кодеков и rtc_connection_id

- **Цель:** видео/скриншер и корреляция SFU работают.
- **Описание:** расширить `Codec` полями `rtx_payload_type`, `encode`, `decode`;
  `SelectProtocol` — `rtc_connection_id` (UUID v4) и `experiments`
  (`["fixed_keyframe_interval"]` в живом кадре); сериализовать в том же порядке, что
  клиент (top-level `address/port/mode` дублируются — уже есть).
- **Оракул:** `RE/voice/rtc1-frames.jsonl` op1 (дословный JSON: 5 кодеков,
  `rtx_payload_type` 102/104/106/108, `encode/decode:true`,
  `rtc_connection_id`, `experiments`).
- **Write-scope:** `rust/crates/discord-voice/src/lib.rs`,
  `rust/crates/discord-voice/tests/signaling.rs` (или `media_contract.rs`).
- **DoD:**
  1. Собранный op1 побайтно/поле-в-поле совпадает с эталонным (кроме случайных UUID).
  2. Старый парсер принимает кадр без новых полей (обратная совместимость).
  3. `rtc_connection_id` генерируется один раз на сессию.
- **Зависит:** нет.

### BE-P1-09. Трей, бейдж, меню приложения

- **Цель:** базовое поведение десктоп-клиента на Linux.
- **Описание:** иконка трея с состояниями (обычное/непрочитанное/в звонке/mute/deafen),
  меню (Open, Mute, Deafen, Voice Settings, Check for Updates, Quit), клик — показать
  окно; бейдж непрочитанных (на Linux — через `libunity`/D-Bus `com.canonical.Unity`
  или счётчик в заголовке, если Unity нет); закрытие окна → трей, если включено.
- **Оракул:** `RE/main-arch.json` §9 (имена иконок, GUID канала, пункты меню),
  `RE/backend-ipc.json` (каналы `DISCORD_APP_SET_BADGE_COUNT`, `DISCORD_APP_DOCK_*`).
- **Write-scope:** новый `rust/crates/discord-platform/src/tray.rs`,
  `rust/crates/discord-platform/src/lib.rs`, `rust/gui-spike/src/main.rs`,
  `rust/crates/discord-platform/tests/tray_contract.rs` (новый).
- **DoD:**
  1. Трей появляется без окна, меню содержит 6 пунктов, «Quit» завершает процесс.
  2. Состояние иконки меняется по mute/deafen/входящему звонку.
  3. Нет иконки в системе → приложение работает, пишет причину в лог (не падает).
- **Тест:** headless-контракт (наличие/порядок пунктов, маппинг состояний), без GUI.
- **Зависит:** нет.

---

## 2. P2 — паритет и устойчивость

### BE-P2-01. Ретраи и 429/503 как у клиента

- **Цель:** не ловить ложные ошибки и не долбить сервер.
- **Описание:** набор авто-ретраев `{502,504,507,598,599,522,523,524}`
  (как опция `retries` на вызов, а не безусловно), `{429,503}` — очередь по URL до
  `retry-after` (заголовок — целые секунды, тело — float секунды), дефолт 5 с,
  cap 60 с, базовая пауза 1 с. Убрать безусловный ретрай 500/501/505.
- **Оракул:** бандл @10 692 340 (`E`/`A`), @10 692 774 (`f(retry-after)`),
  `RE/rest-static.md` §2.
- **Write-scope:** `rust/crates/discord-core/src/rest/rate_limit.rs`,
  `rust/crates/discord-core/src/rest/client.rs`,
  `rust/crates/discord-core/tests/rest_retry.rs`.
- **DoD:** таблица «статус × метод → повтор/нет/сколько» совпадает с оракулом;
  503 с `retry-after: 3` ждёт 3 с, а не 250 мс; 500 не повторяется.
- **Тест:** виртуальные часы, стаб со сценариями.

### BE-P2-02. Заголовки: `Accept-Language` q-список и установочные ID

- **Цель:** запросы выглядят как клиент там, где это влияет на сервер.
- **Описание:** `Accept-Language` из списка языков с q=0.9…0.1; для auth/капчи
  передавать `X-Fingerprint` и `X-Installation-ID` (появляются только при наличии
  значений); `X-Routing-Key` — если задан конфигом. `X-Context-Properties`/
  `X-Audit-Log-Reason` — только когда появятся соответствующие вызовы.
- **Оракул:** бандл @1 002 368 (`prepareRequest`); `RE/rest-static.md` §2/§4;
  `RE/rest-d02.json` `unknowns U-08` (живьём не наблюдались — не выдавать за live).
- **Write-scope:** `rust/crates/discord-core/src/rest/headers.rs`,
  `rust/crates/discord-core/src/rest/client.rs`,
  `rust/crates/discord-core/tests/rest_client.rs`.
- **DoD:** `ru,ru-RU;q=0.9,en-US;q=0.8` при трёх языках; пустые fingerprint/installation
  не отправляются; набор из 6 обязательных заголовков не изменился.
- **Тест:** юнит на сборку значения; регрессия на отсутствие пустых заголовков.

### BE-P2-03. `User-Agent` и `Accept-Encoding` в HTTP/1.1

- **Цель:** не выглядеть анонимным ботом для Cloudflare; меньше трафика.
- **Описание:** ставить `User-Agent` из идентичности (сейчас UA есть только в
  супер-свойствах), `Accept-Encoding: gzip, br` и поддержать распаковку
  (`gzip`/`brotli`) либо явно не просить сжатие, если распаковки нет. Решение по UA —
  продуктовое (свой vs эталонный), зафиксировать в контракте.
- **Оракул:** `RE/rest-d02.json` `auth.superProperties.browser_user_agent`;
  `RE/rest-static.md` §2 (superagent/Electron).
- **Write-scope:** `rust/crates/discord-core/src/rest/transport.rs`,
  `rust/crates/discord-core/src/rest/http1.rs` (если нужна распаковка),
  `rust/crates/discord-core/tests/rest_client.rs`.
- **DoD:** запрос содержит `User-Agent` непустым; при `Accept-Encoding: gzip`
  ответ распаковывается (или заголовок не отправляется); поведение описано в docs.
- **Тест:** стаб с gzip-телом.

### BE-P2-04. Query-слой маршрутов + подключение pins/threads

- **Цель:** обязательные параметры перестают теряться.
- **Описание:** добавить в `RouteSpec` описание query (`required/optional`, типы),
  рендер query с percent-encoding и валидацией; перевести поиск/историю/pins/threads
  на него. Реализовать `list_channel_pins` (limit=25) и `list_channel_threads`
  (archived, sort_by=last_message_time, sort_order=desc, limit=25, tag_setting=match_some,
  offset), зарегистрировать Tauri-команды с именами, которые уже зовёт фронтенд
  (`pins_list`, `threads_list`), либо синхронизировать имена с UI (решение PM/UI).
- **Оракул:** `RE/rest-static.json` `PINS.cacheEvidence` (`?limit=25`),
  `THREAD_SEARCH.cacheEvidence` (полный URL), `MESSAGES.cacheEvidence`;
  `RE/rest-d02.json` `queryParams` (45 имён, включая `archived`, `before`, `around`,
  `limit`, `offset`, `sort_by`, `sort_order`, `tag_setting`).
- **Write-scope:** `rust/crates/discord-core/src/rest/routes.rs`, новый
  `rust/crates/discord-core/src/rest/pins_threads.rs`,
  `rust/gui-spike/src/main.rs`, `rust/gui-spike/src/data.rs`,
  `rust/crates/discord-core/tests/rest_routes.rs`.
- **DoD:** pins отдаёт список с `limit=25`; threads без `archived` не уходит (ошибка до
  сети); query-рендер экранирует `+`, `#`, `&`, кириллицу; UI-команды существуют.
- **Тест:** сверка query-строки с эталонными URL из `rest-static`.

### BE-P2-05. Одна идентичность для IDENTIFY и X-Super-Properties

- **Цель:** сервер видит одного клиента, а не два.
- **Описание:** один источник значений (например, `SuperProperties`, собранный
  `identity::build`) для обоих запросов; выровнять `browser`, `os_arch`, `app_arch`,
  `browser_version`, `client_launch_id`, `system_locale`, `browser_user_agent`.
  Отдельно — продуктовое решение PM: своя версия/UA или маскировка под 1.0.162.
  До решения: пара `client_version` ↔ `client_build_number` должна быть согласована
  (не «0.1.0 + 641004»).
- **Оракул:** `RE/rest-d02.json` `auth.superProperties` (22 поля, значения);
  `RE/rest-static.md` §4 (UA, префикс токена); бандл `_doIdentify` (properties).
- **Write-scope:** `rust/crates/discord-core/src/identify.rs`,
  `rust/crates/discord-core/src/rest/identity.rs`,
  `rust/crates/discord-core/src/gateway/supervisor.rs`,
  `rust/crates/discord-core/tests/identify.rs`, `rest_identity.rs`.
- **DoD:** тест «IDENTIFY.properties == SP по 14 общим полям»; `os_arch` = `x64` на
  x86_64; `client_launch_id` — UUID v4 в обоих; решение PM записано в контракте.
- **Зависит:** решение PM по версии/UA.

### BE-P2-06. Расписание heartbeat и close-коды

- **Цель:** не дрейфовать по времени и не зацикливаться на «мёртвых» кодах.
- **Описание:** (а) первый удар — случайная задержка `[0, interval)` (seed из
  крипто-ГСЧ, не 0), далее фиксированный интервал от отправки, а не от ACK;
  (б) после 4 неудачных реконнектов — сброс `sessionId`/`seq` (следующая попытка
  IDENTIFY), но **не** остановка, как у клиента; наш `max_reconnects=8 → Stop`
  либо убрать, либо увеличить и оставить только как product-decision;
  (в) окно resume — 3 мин от последнего ACK.
- **Оракул:** бандл @5 016 157 (startHeartbeater), @5 014 326 (`_handleClose`,
  `_reset`), `_doResumeOrIdentify` @5 019 509; `RE/gateway-protocol.json`
  `heartbeat.sentPeriodic` (median 41 250).
- **Write-scope:** `rust/crates/discord-core/src/gateway.rs`,
  `rust/crates/discord-core/src/gateway/supervisor.rs`,
  `rust/crates/discord-core/src/resume.rs`,
  `rust/crates/discord-core/tests/gateway_session.rs`,
  `rust/crates/discord-core/tests/resume_reconnect.rs`.
- **DoD:** интервалы в реплее 41 250 ± 0 без накопления RTT; после 5 неудач
  следующая попытка идёт с IDENTIFY, а не с RESUME; разрыв 31 с и 170 с от
  последнего ACK → resume/identify соответственно; поведение 4004 не изменилось.
- **Тест:** реплей таймлайна + юнит на классификацию кодов.

### BE-P2-07. Ротация сокета: SRTCP-обёрнутый пустой RR

- **Цель:** новый сокет «жив» для сервера с первого пакета.
- **Описание:** в `rotate_if_due` использовать `rotate_srtcp(tag, counter)` (28 Б),
  а не `rotate()` (8 Б plaintext); счётчик — продолжение общего RTP/RTCP-счётчика.
- **Оракул:** `RE/voice-deep.json` `udp.rtcp.rr_client_empty` (28 Б, layout),
  `udp.socket_lifecycle` (ротация 60 080 мс, «пустой RR сразу после ротации»).
- **Write-scope:** `rust/crates/discord-voice/src/udp.rs`,
  `rust/crates/discord-voice/tests/media_contract.rs`.
- **DoD:** после ротации уходит ровно 28 Б с тегом и счётчиком; старый сокет закрыт;
  порт меняется; discovery уходит одновременно с RR (как в strace).
- **Тест:** байтовый оракул 28 Б.

### BE-P2-08. RTCP: генерация SR и обработка RR

- **Цель:** сервер получает нашу статистику, мы — его.
- **Описание:** слать SRTCP SR (60 Б: `[8 hdr][ssrc][20 cipher][tag16][counter4]`)
  с интервалом ~5 с (эталон 3.0–6.3 с), разбирать серверный RR 52 Б (RC=1), учитывать
  потери/джиттер, если тела расшифруются; при отсутствии ключа — хотя бы корректно
  считать и логировать.
- **Оракул:** `RE/voice-deep.json` `udp.rtcp` (размеры, интервалы, счётчики),
  `RE/voice/voice-udp3.rtp.json`/`voice-udp4.rtp.json`.
- **Write-scope:** `rust/crates/discord-voice/src/rtcp.rs`,
  `rust/crates/discord-voice/src/stats.rs`,
  `rust/crates/discord-voice/tests/stats_contract.rs`.
- **DoD:** SR собирается побайтно по layout; интервал выдерживается; RR сервера
  парсится без падений; счётчики видны в `stats`.
- **Зависит:** BE-P1-06 (шифрование).

### BE-P2-09. Вложения: PUT загрузки и refresh URL

- **Цель:** файл реально уходит.
- **Описание:** `POST /channels/{id}/attachments` → `PUT upload_url` (тело — файл,
  без auth-заголовка Discord, с `Content-Length`), `POST /messages` с
  `attachments:[{id,filename,uploaded_filename}]`; `POST /attachments/refresh-urls`
  для истёкших; коды 40005/413/429 — типизированы; лимиты tier (20/50/100 МиБ).
  В этом дереве модуля нет — сверить с TL, где срез `attachments.rs`.
- **Оракул:** `RE/rest-static.json` `/channels/{arg}/attachments`,
  `/attachments/refresh-urls`, `/attachments/{arg}`; `ATTACH-RE.md`, PARITY AT-01
  (лимиты `Math.max(0x1400000, tier)`).
- **Write-scope:** новый `rust/gui-spike/src/attachments.rs`,
  `rust/gui-spike/src/main.rs`, `rust/gui-spike/tests/attachments_contract.rs` (новый).
- **DoD:** загрузка 1 файла → сообщение с вложением; 413 и 40005 различаются;
  прогресс/докачка `308` — если PM включит в скоуп (иначе явно вне).
- **GATE:** live-окно для кодов 40005/413 (в дампе нет).

### BE-P2-10. Профиль пользователя по REST

- **Цель:** поповер профиля показывает live-данные.
- **Описание:** `GET /users/{id}/profile?type=popout&with_mutual_guilds=true&
  with_mutual_friends=true&with_mutual_friends_count=false`, разбор
  `user/badges/guild_member/connected_accounts`; IPC-команда для UI.
- **Оракул:** `RE/rest-static.json` `/users/{arg}/profile` (`calls:9`, полный example
  URL); `RE/gateway-protocol.json` `ready.counts.users=146` (набор полей user).
- **Write-scope:** новый `rust/gui-spike/src/profile.rs`,
  `rust/gui-spike/src/main.rs`, `rust/gui-spike/tests/profile_contract.rs` (новый).
- **DoD:** профиль грузится по id; 404/403/429 типизированы; поля не выдуманы
  (только те, что в ответе).
- **GATE:** live-окно для формы ответа (в D-02 тела нет).

### BE-P2-11. settings-proto: чтение/запись настроек

- **Цель:** настройки синхронизируются с аккаунтом.
- **Описание:** `GET /users/@me/settings-proto/{type}` и
  `PATCH /users/@me/settings-proto/{type}` с protobuf-телом; типы
  (`PreloadedUserSettings`, `FrecencyUserSettings`) уже есть в
  `settings/preloaded.rs`, `settings/frecency.rs` — не хватает сети и версий.
- **Оракул:** `RE/rest-d02.json` endpoint `GET /users/@me/settings-proto/{num}` (3×200)
  и `PATCH` (1×200, тело base64-протобуф); `RE/rest-static.md` boot step 8;
  `RE/gateway-schema.json` `READY.fields.user_settings_proto` (строка).
- **Write-scope:** новые `rust/crates/discord-core/src/rest/settings_proto.rs`,
  `rust/gui-spike/src/settings_store.rs`,
  `rust/crates/discord-core/tests/settings_proto_http.rs`.
- **DoD:** чтение отдаёт те же поля, что локальный парсер; запись не теряет
  неизвестные поля (сохранять raw); 401/429 типизированы.
- **GATE:** live для версий/`api_code_version`.

### BE-P2-12. Read-state: bulk-ack и `flags`

- **Цель:** бейджи и «прочитано» работают на реальном состоянии.
- **Описание:** `POST /read-states/ack-bulk` при массовом скролле/открытии,
  `flags` в ack (если сервер его присылает/принимает), `mention_count`/`manual`
  из статической карты — только после live-подтверждения.
- **Оракул:** `RE/rest-static.json` `/read-states/ack-bulk`, `MESSAGE_ACK.bodyKeys`
  (`{token,last_viewed,flags}`, `{manual,mention_count}`); `RE/rest-d02.json`
  живой ack `{token,last_viewed}`.
- **Write-scope:** `rust/gui-spike/src/messages.rs`,
  новый `rust/gui-spike/src/read_states.rs`,
  `rust/gui-spike/tests/read_state_contract.rs`.
- **DoD:** bulk-ack отправляет ≤ N записей за вызов (N — из live); `flags`
  не ломает существующий ack; бейдж сбрасывается после успеха.
- **GATE:** live (формы flags/manual не подтверждены).

### BE-P2-13. Deep links: `disclip://` и очередь открытия

- **Цель:** ссылки из браузера/клипов открывают нужный экран.
- **Описание:** принимать `disclip://` (privileged), `--url` после `--`, очередь
  до готовности окна, маршрут `MAIN_WINDOW_PATH`; unknown route — не падать.
- **Оракул:** `RE/main-arch.json` §9 (`extractUrlFromArgs`, `openOrQueueUrl`,
  `handleOpenUrl`, `DISCORD_CLIP_PROTOCOL="disclip"`).
- **Write-scope:** `rust/crates/discord-platform/src/deep_link.rs`,
  `rust/crates/discord-platform/tests/deep_link_contract.rs` (если есть/новый),
  `rust/gui-spike/src/main.rs`.
- **DoD:** `discord://-/channels/1/2` и `disclip://<id>` разбираются; ссылка до старта
  окна не теряется; неизвестный путь — типизированный отказ, окно открыто.
- **Тест:** таблица URL из main-arch.

### BE-P2-14. Уведомления: dispatch → D-Bus → клик → канал

- **Цель:** упоминание в закрытом окне даёт уведомление и переход.
- **Описание:** подписка на `MESSAGE_CREATE` с упоминанием/ЛС, DND-проверка,
  `Notify` с `actions`, обработка `ActionInvoked` → deep link на канал; дедуп по
  `message.id`; не показывать для активного канала.
- **Оракул:** `RE/backend-ipc.json` цепочка A (`NOTIFICATIONS_SEND_NOTIFICATION`,
  `NOTIFICATIONS_RECEIVED_RESPONSE`), `RE/main-arch.json` §9/§10;
  `notifications.rs` уже реализует D-Bus.
- **Write-scope:** `rust/crates/discord-platform/src/notifications.rs`,
  `rust/gui-spike/src/net.rs` (хук), `rust/gui-spike/src/main.rs`,
  `rust/crates/discord-platform/tests/notifications_contract.rs`.
- **DoD:** уведомление появляется, клик открывает канал, DND подавляет, дубликаты
  не показываются, секреты в тексте не утекают.
- **Зависит:** BE-P1-02, BE-P2-13.

### BE-P2-15. DM и друзья: каналы и relationships

- **Цель:** ЛС и список друзей перестают быть статикой.
- **Описание:** `POST /users/@me/channels` (DM/группа), `GET/PUT/DELETE
  /users/@me/relationships/{id}`, разбор `RELATIONSHIP_ADD/REMOVE` (типы 1/2 из READY:
  18/42), список из READY уже есть.
- **Оракул:** `RE/rest-static.json` `/users/@me/channels`,
  `/users/@me/relationships/{arg}`; `RE/gateway-protocol.json` `ready.counts.relationships`
  (60, типы 1:18, 2:42).
- **Write-scope:** новый `rust/gui-spike/src/dms.rs`,
  `rust/gui-spike/src/main.rs`, `rust/gui-spike/tests/dm_contract.rs` (новый).
- **DoD:** создать DM по id, отправить сообщение, принять/удалить заявку; 403/404/429
  типизированы.
- **GATE:** live для кодов (в D-02 нет).

---

## 3. P3 — точность и гигиена

| ID | Задача | Оракул | Write-scope | DoD |
|---|---|---|---|---|
| BE-P3-01 | PUT реакции без тела | `RE/rest-d02.json` endpoint реакции (`reqBodyKeys: []`) | `rust/gui-spike/src/reactions.rs`, тест | 204 без `Content-Type`; обратная совместимость |
| BE-P3-02 | Каталог маршрутов: 49 vs 47, пометить `[S]`-расширения | `RE/rest-d02.json` (47), сырой `capture-run2.jsonl` (search) | `rust/crates/discord-core/src/rest/routes.rs`, `tests/data/rest-routes-47.tsv` | шапка и массив сходятся; search помечен как расширение |
| BE-P3-03 | Keep-alive пул соединений | `RE/rest-static.md` §2 (superagent) | `rust/crates/discord-core/src/rest/transport.rs` | 2 запроса подряд = 1 TLS; замер в тесте |
| BE-P3-04 | Поля rate-limit: `scope`, `limit`, bucket-reuse | бандл @10 692 340; `x-ratelimit-*` | `rust/crates/discord-core/src/rest/rate_limit.rs` | поля парсятся, bucket переиспользуется |
| BE-P3-05 | Discovery request: последние 2 байта | `RE/voice/strace-media-s3.jsonl` (полные байты) | `rust/crates/discord-voice/src/udp.rs` | байт-в-байт с записью strace |
| BE-P3-06 | `VoiceOp`: 5/7/11-13/21/24-32 + DAVE | `RE/voice-deep.json` `signaling.ops`, `unknowns` | `rust/crates/discord-voice/src/lib.rs` | неизвестный op не теряется, известный типизирован |
| BE-P3-07 | Апдейтер: URL и загрузка | `RE/main-arch.json` §5 | `rust/crates/discord-updater/src/*` | манифест с боевого endpoint, подпись, отказ при несовпадении |
| BE-P3-08 | Миграция токена из safeStorage | `RE/rest-static.md` §4 (`dQw4w9WgXcQ:`) | `rust/crates/discord-platform/src/keyring.rs` | импорт существующего токена один раз, без логов |

Отдельно, без кода: **BE-P3-09** — синхронизировать `docs/re/` и `routes.rs`
(в `docs/re/rest-d02.json` уже лежит копия оракула; если она разошлась с
`reference-oracle/rest-d02.json` — обновить и зафиксировать md5).

---

## 4. RE-задачи (до кода)

| ID | Что снять | Зачем | Как | Выход |
|---|---|---|---|---|
| RE-P1-01a | Объект подписки op37 (`SubscriptionManager.get(guild)`) | без него нельзя подписаться на участников | бандл @9 112 505 (`G.get`), @4 954 460 (`GUILD_SUBSCRIPTIONS_FLUSH`); выписать поля `channels/members/threads/typing/activities` и формат `channels[<id>]` | `RE/…`-фрагмент + `CONTRACTS/gateway-subscriptions.md` |
| RE-P1-06 | Схема nonce/AAD/key для `aead_aes256_gcm_rtpsize` | без неё SRTP несовместим | строки `RE/native-re/raw/discord_voice/rabin2-strings.txt`: 2575-2578 (режимы), 2717/2734 (nonce), 4212 (`base_nonce`), 1536 (`decryptInvalidNonceCount`); дизасм `.node` + живой пакет с известным ключом | `VOICE-CRYPTO.md` (восстановить) + вектор |
| RE-P2-01a | Форма тела 202 поиска | чтобы не выдумывать поля | live-окно: поиск по большому серверу сразу после старта | запись `retry_after` и полей |
| RE-P2-02a | Коды загрузки 40005/413 и `refresh-urls` | типизация AT-01 | live-окно: своя загрузка | таблица кодов |
| RE-P2-03a | Формы `flags`/`manual`/`mention_count` в ack | бейджи | live-окно + бандл call-site | контракт read-state |

---

## 5. Порядок исполнения (предложение)

**Волна A (backend-протокол, 3 воркера, файлы не пересекаются):**

1. BE-P1-03 (zstd) — `discord-codec`.
2. BE-P1-04 + BE-P2-04 (история + query-слой) — `discord-core/rest` + `gui-spike/data.rs`.
3. BE-P1-05 (поиск 202) — `gui-spike/search.rs`.

**Волна B (шлюз):**

4. BE-P1-01 (op3/op8/op4, op37 после RE-P1-01a) — `discord-core/gateway`.
5. BE-P1-02 (мост) — `gui-spike/net.rs` (после A1, чтобы не тестировать на потерянном
   хвосте).
6. BE-P2-06 (тайминги/коды).

**Волна C (голос, после RE-P1-06):**

7. BE-P1-06 (SRTP) → BE-P1-07 (WS-сессия) → BE-P1-08 (Select Protocol) → BE-P2-07/08.

**Волна D (продукт):**

8. BE-P2-09/10/11/12/13/14/15, BE-P1-09 (трей), P3-пакет.

Ограничения по правилам репозитория: не более 2 фоновых `cargo` одновременно;
`state/types.rs`, `state/reducer.rs`, `state/mod.rs`, `src/lib.rs` правит только TL;
live-окна — по одному на приёмку, с тестовым аккаунтом.

---

## 6. Что PM нужно решить

1. **Идентичность клиента:** остаёмся «discord-lab 0.1.0» или маскируемся под 1.0.162?
   От этого зависят BE-P2-05, BE-P2-03, риск капчи и `/science`.
2. **Телеметрия:** `/science` и `/metrics/v2` не отправляем — подтвердить как политику
   (сейчас это код-блокировка, а не решение PM).
3. **Приоритет трея** (BE-P1-09): он в P1 по продуктовой важности, но не блокирует
   протокол. Если релиз протокольный — можно опустить в P2 и начать с волны A.
4. **Вложения:** включать ли докачку `308`/`Content-Range` в первый срез (AT-01)
   или ограничиться одной PUT-загрузкой.
5. **Судьба `session.rs`:** в PARITY V-01 помечен принятым, в этом дереве файла нет.
   Нужно подтвердить, какой срез считается источником истины, иначе BE-P1-07
   продублирует работу.
