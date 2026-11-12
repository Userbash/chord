# BACKLOG RE (часть 2): бэкенд и протокол (по итогам task-130)

**Источник:** `/home/user/orca/project/waves/RW-40/RE-AUDIT/RE-GAP-BACKEND-PROTOCOL-PART2.md`
(далее — отчёт; ссылки вида «M-32», «G-10», «§3.4» — на его разделы). Канонический
`RE-GAP-BACKEND-PROTOCOL.md` содержит независимый отчёт второго воркера; этот бэклог
составлен по части 2 и не претендует на его нумерацию.
**Эталон:** `reference-oracle/` (RE-дамп Discord 1.0.162). Каждая карточка содержит
оракул — конкретный файл/чанк дампа, по которому приёмка проверяет результат.
**Базовый HEAD:** работа велась в worktree `re-task-130-re-audit-backend-protocol`;
продуктовый код не менялся (read-only аудит).

Как читать: **P1** — без этого нельзя выпускать клиент с живым аккаунтом; **P2** —
расхождение с протоколом/паритетом, сервер терпит, но поведение отличается; **P3** —
расширение покрытия. Оценка: S ≤ 1 день, M ≤ 3 дня, L > 3 дней (одна карточка = один воркер).

Правило приёмки для всех карточек: red-first тест, оракул из дампа, `gate:worker`,
файлы — строго из write-scope. Общие файлы (`state/types.rs`, `state/reducer.rs`,
`state/mod.rs`, `src/lib.rs`, `TASKS.md`, `waves/*`) правит только Team Lead.

---

## P1 — блокеры выпуска

### RE-BE-01. Шлюз: zstd-пайплайн без потери термов
**Проблема.** `ruzstd`-декодер держит окно 64 КиБ и не отдаёт хвост до конца кадра: на живом
сокете `run2:95974.1389` пайп видит 44 терма из 113 диспатчей (отчёт §3.4,
`rust/crates/discord-core/src/ws.rs:51-57`, `rust/crates/discord-codec/src/zstd.rs`).
READY приходит, дальше состояние «замирает».
**Цель.** Все термы живого потока доходят до `Session::on_value` в том же порядке, без роста
памяти на соединение.
**Описание.** Варианты: (а) потоковый декодер с flush на границе WS-кадра; (б) собственный
ридер zstd-блоков без удержания окна; (в) кольцевой буфер с принудительным сбросом окна.
Выбрать по замеру на эталонных hexl-снимках; тест — реплей всех 7 сокетов.
**Оракул.** `reference-oracle/gw-proto/zstd-frames-95974_1389.hexl` +
`reference-oracle/gateway-protocol.md:99-127` (таблица 161/137/109/648/71/374/44 кадров и
137/113/94/569/59/331/40 термов). Счётчик диспатчей на сокет — `gateway-protocol.md:150-158`.
**Write-scope.** `rust/crates/discord-codec/src/zstd.rs`, `rust/crates/discord-core/src/ws.rs`,
`rust/crates/discord-codec/tests/*` (новые), `rust/crates/discord-core/tests/ws_*.rs` (только
дополнение).
**DoD.** (1) Red-тест: реплей `.1389` даёт 113/113 диспатчей (сейчас 44) — падает до фикса.
(2) Тот же тест на всех 7 снимках: число термов = таблице. (3) Память на соединение не растёт
линейно по потоку (замер в тесте с потолком). (4) `gate:worker` зелёный.
**Зависимости.** нет. **Оценка.** L. **Риск.** смена декодера затрагивает формат ошибок
`TransportError::Zstd` — сохранить варианты.

### RE-BE-02. RESUME-окно: 3 минуты вместо 30 секунд
**Проблема.** `RESUME_MAX_DOWNTIME_MS = 30_000` (`rust/crates/discord-core/src/resume.rs:19`);
клиент резюмирует, если с последнего ACK прошло ≤ 3 минут (бандл `_doResumeOrIdentify`,
`tI=3*eb.A.Millis.MINUTE`). После 40-секундного разрыва мы делаем IDENTIFY и заново тянем READY
(~580 КБ) вместо RESUME.
**Цель.** Паритет с клиентом: RESUME до 180 с, IDENTIFY — после.
**Описание.** Поменять константу и семантику `plan_reconnect`; проверить, что супервайзер
измеряет разрыв от последнего ACK, а не от закрытия сокета. Добавить тест-кейсы 29 с / 179 с /
181 с.
**Оракул.** `reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js`
(offset 5 019 755: `_doResumeOrIdentify`, `tI=3*eb.A.Millis.MINUTE`);
`reference-oracle/gateway-protocol.md:155` (три RESUMED с продолжением `s`).
**Write-scope.** `rust/crates/discord-core/src/resume.rs`,
`rust/crates/discord-core/src/gateway/supervisor.rs`, `rust/crates/discord-core/tests/resume*.rs`.
**DoD.** (1) Red/green на границах 179/181 с. (2) `plan_reconnect` не режет окно по другим
таймерам. (3) Константа названа по смыслу (`RESUME_MAX_DOWNTIME_MS = 180_000`) и задокументирована
ссылкой на бандл. (4) `gate:worker` зелёный.
**Зависимости.** нет. **Оценка.** S.

### RE-BE-03. Голос: исходящий Speaking (op5) и цикл отправки
**Проблема.** В `discord-voice` нет модели кадра Speaking; `OBSERVED_RTC_OPS` не содержит 5
(отчёт §4.1). Клиент шлёт `{op:5,d:{user_id,ssrc,speaking}}`, флаги
`NONE=0,VOICE=1,SOUNDSHARE=2,PRIORITY=4,HIDDEN=8`. Без этого передача голоса невозможна.
**Цель.** Клиент умеет начать/закончить talkspurt и отправить RTP-пакеты в медиа-сокет.
**Описание.** Типизировать op5 (parse+encode), добавить машину состояний «молчит/говорит»
(порог VAD задаёт вызывающий, не модуль), связать с `media/rtp.rs` и `media/jitter.rs`,
отправка — через `UdpMediaSocket`. DAVE не трогаем (отдельная карточка RE-BE-09).
**Оракул.** Бандл, offset 2 396 729 (`case 5: … emit("speaking", i.user_id, i.ssrc, r)`) и
≈10 768 632 (флаги Speaking); `reference-oracle/voice-deep.md:21-30` (таблица op
сигнализации), `voice/rtc1-frames.jsonl` (формат op0/op1/op2/op4).
**Write-scope.** `rust/crates/discord-voice/src/lib.rs`, `rust/crates/discord-voice/src/media/*`,
`rust/crates/discord-voice/src/udp.rs`, `rust/crates/discord-voice/tests/*` (новые).
**DoD.** (1) Red-тест: encode op5 байт-в-байт по форме клиента, обратный parse. (2) Тест
«talkspurt»: 10 кадров opus → 10 RTP-пакетов с `ts += 960`, marker на первом. (3) Отправка
в локальный UDP-стаб с проверкой байтов. (4) `gate:worker` зелёный.
**Зависимости.** нет (SRTP можно замокать). **Оценка.** M. **Риск.** ключи SRTP wire-несовместимы
(RE-BE-10) — карточка не должна обещать совместимость с живым Discord, только форму кадра.

### RE-BE-04. REST: друзья (заявка/принять/удалить)
**Проблема.** Эндпоинтов отношений нет; читаются только `READY.relationships` (60 записей).
**Цель.** Добавить/принять/удалить друга из UI без перезахода.
**Описание.** `PUT /users/@me/relationships/{id}` (заявка), `DELETE` (удалить/отклонить),
`PATCH` (изменение связи; точное назначение в статике не описано — уточнить по живому
трафику). Добавить в каталог как
`[S]`-расширения (не размывать 47 live-строк), модель ошибок — общий `ApiError`.
**Оракул.** `reference-oracle/rest-static.md:694` (`DELETE|PATCH|PUT USER_RELATIONSHIP`),
`:95`; отсутствие в живом захвате — `rest-d02.md:484-500` (маршрут ожидался, не подтверждён).
**Write-scope.** `rust/crates/discord-core/src/rest/routes.rs`,
`rust/crates/discord-core/src/rest/resources.rs`, новый
`rust/crates/discord-core/src/rest/relationships.rs`,
`rust/gui-spike/src/` — один новый модуль `friends.rs` + регистрация в `main.rs`,
`rust/crates/discord-core/tests/rest_relationships.rs`.
**DoD.** (1) Red-тест: `find(Put, "/users/@me/relationships/{id}")` + рендер пути; тело пустое,
как у клиента. (2) Типизированные ответы 200/204/403 (код `50007`/`80007`). (3) `gate:worker`.
**Зависимости.** нет. **Оценка.** M.

### RE-BE-05. REST: создание ЛС и групповой ЛС
**Проблема.** `POST /users/@me/channels {recipients}` отсутствует; новый диалог начать нельзя.
**Цель.** Диалог с пользователем/группой создаётся из UI и появляется в списке.
**Описание.** Маршрут + тело `{recipients:[snowflake]}` + разбор ответа-канала (type 1/3),
дедупликация существующего канала по `READY.private_channels`.
**Оракул.** `reference-oracle/rest-static.md:616` (`USER_CHANNELS`);
схема приватного канала — `reference-oracle/gateway-schema.md:121-166` (`private_channels`,
17 шт., все type 1) и `rust/crates/discord-core/src/ready/social.rs` (уже парсит).
**Write-scope.** `rust/crates/discord-core/src/rest/routes.rs`, новый
`rust/crates/discord-core/src/rest/channels.rs`, `rust/gui-spike/src/messages.rs`
(команда «открыть ЛС»), `rust/gui-spike/src/main.rs`, тесты в `rust/crates/discord-core/tests/`.
**DoD.** (1) Red-тест рендера и тела. (2) Разбор ответа в `PrivateChannel` с проверкой полей
`id,type,recipients,flags,last_message_id`. (3) Повторный вызов не создаёт дубль (селектор).
(4) `gate:worker`.
**Зависимости.** нет. **Оценка.** M.

### RE-BE-06. REST: управление каналом (GET/PATCH/DELETE)
**Проблема.** Нет `GET/PATCH/DELETE /channels/{id}`; переименовать/закрыть/удалить канал нельзя.
**Цель.** Базовое управление каналом и ЛС из UI.
**Описание.** Три метода в каталог `[S]`; тело PATCH — только изменяемые поля; DELETE ЛС/группы
отличать по `type`; после успеха — локальное применение к состоянию.
**Оракул.** `reference-oracle/rest-static.md:200` (`CHANNEL` — PATCH|DELETE|GET);
поля канала — `gateway-schema.md` (объект канала в READY); создание канала в гильдии
(`rest-static.md:340`) — вне scope, отдельная задача PM.
**Write-scope.** `rust/crates/discord-core/src/rest/routes.rs`, новый
`rust/crates/discord-core/src/rest/channels.rs`, `rust/gui-spike/src/guilds.rs` (команды),
`rust/gui-spike/src/main.rs`, тесты.
**DoD.** (1) Red-тест трёх методов и рендера. (2) 204/200/403 классифицируются. (3) UI-команда
не отправляет `null`-поля. (4) `gate:worker`.
**Зависимости.** RE-BE-05 (общий модуль `channels.rs`). **Оценка.** M.

---

## P2 — паритет протокола

### RE-BE-07. Заголовки: `Accept-Language` q-список + опциональные fingerprint/installation/routing/context
**Проблема.** `Accept-Language` = локаль без q (`client.rs:166`); нет `X-Fingerprint`,
`X-Installation-ID`, `X-Routing-Key`, `X-Context-Properties` (M-01…M-06). Статика клиента
отправляет их условно; живой D-02 их не видел (U-08).
**Цель.** Конфигурируемая «шапка», совпадающая и с живым захватом, и с кодом клиента.
**Описание.** `Accept-Language` из списка локалей (`ru,ru-RU;q=0.9,…`, первые 10 как в бандле);
новые заголовки — `Option<String>` в `ApiAuth`, пустые не отправляются. Дефолт — как в D-02
(только 6), включение — явной настройкой и тестом на форму.
**Оракул.** Бандл `prepareRequest` offset 1 001 996 (полный текст функции);
`reference-oracle/rest-d02.md:292-312` (6 заголовков в 133/133) и `:400-404` (U-08).
**Write-scope.** `rust/crates/discord-core/src/rest/headers.rs`,
`rust/crates/discord-core/src/rest/client.rs`, `rust/crates/discord-core/src/rest/identity.rs`,
`rust/crates/discord-core/tests/rest_headers*.rs`.
**DoD.** (1) Red-тест q-списка на 3 локалях. (2) Тест: пустой fingerprint → заголовка нет.
(3) 6-заголовочный профиль остаётся дефолтом (тест на отсутствие лишних). (4) `gate:worker`.
**Оценка.** S.

### RE-BE-08. Супер-свойства: живое обновление и честные дефолты сборки
**Проблема.** `ApiAuth` — снимок; `client_app_state` навсегда `focused`; `client_build_number`
и `native_build_number` по умолчанию 0 (M-09, M-10).
**Цель.** Шапка меняется вместе с состоянием окна/сессии; дефолты — как у сборки 1.0.162.
**Описание.** Хранилище SP с `extend`-операцией (аналог `extendSuperProperties`), вызов на
событие фокуса и на новую телеметрическую сессию; дефолты 641004/94186 с возможностью
переопределить.
**Оракул.** Бандл, модуль `618651` (`extendSuperProperties` для `client_app_state` и
`client_heartbeat_session_id`); `reference-oracle/rest-d02.md:352-353` (641004, 94186).
**Write-scope.** `rust/crates/discord-core/src/rest/client.rs`,
`rust/crates/discord-core/src/rest/super_properties.rs`,
`rust/crates/discord-core/src/rest/identity.rs`, `rust/gui-spike/src/auth.rs`,
`rust/crates/discord-core/tests/rest_super_properties*.rs`.
**DoD.** (1) Red: смена фокуса меняет `client_app_state` в следующем запросе. (2) Дефолтный SP
содержит 641004/94186. (3) Тест неизменности 22 полей и порядка. (4) `gate:worker`.
**Оценка.** S.

### RE-BE-09. Голос: DAVE/MLS-опкоды и voice resume
**Проблема.** Нет op7/9 (resume/resumed, окно 1 мин, close 4801) и op21/22/24/25/27/29/30
(DAVE). E2EE-каналы и переподключение медиа без re-identify невозможны (отчёт 4.1).
**Цель.** Сигнализация DAVE типизирована и прогоняется через тестовый вектор; voice resume
работает по контракту клиента.
**Описание.** Модели кадров + состояние DAVE-перехода (idle→prepare→execute), без крипто
(шифрование MLS — отдельная задача); resume — `{token,session_id,server_id,channel_id,seq_ack}`,
окно 1 минута, на истечении — disconnect 4801.
**Оракул.** Бандл offset 2 396 729 (switch по op: 7,9,21,22,24,25,27,29,30 и `doResumeOrClose`,
`ew=1*MINUTE`, `disconnect(!1,4801,…)`); `reference-oracle/voice-deep.md:41-46` (движковый лог:
«DAVE protocol init», MLS external sender / key package).
**Write-scope.** `rust/crates/discord-voice/src/lib.rs`,
`rust/crates/discord-voice/src/session/*` (новый), `rust/crates/discord-voice/tests/*`.
**DoD.** (1) Red: parse/encode op7 и op21–30 по формам клиента. (2) Машина состояний
prepare→execute с запретом неверного порядка. (3) Resume-окно 60 с (граница 59/61) с тестом.
(4) Явная пометка «крипто MLS не реализовано» в API. (5) `gate:worker`.
**Оценка.** L. **Риск.** нет живого оракула DAVE — только бандл; приёмка должна это учитывать.

### RE-BE-10. Голос: SRTCP-RR при ротации + wire-совместимость ключей
**Проблема.** `rotate_if_due` отправляет 8-байтовый незашифрованный RR вместо 28-байтового
SRTCP (M-48); KDF ключей самосогласован, не wire (`srtp.rs:10-14`).
**Цель.** После ротации уходит пакет той же формы, что в захвате; ключевой график описан и
проверен вектором.
**Описание.** Переключить боевой путь на `rotate_srtcp` с тегом/счётчиком от шифра; разобрать
`secret_key` из op4 → ключ/соль → nonce (`rtpsize`), сверить с первыми 10 пакетами.
**Оракул.** `reference-oracle/voice-deep.md:64-72` (пустой RR 28 Б сразу после ротации,
раскладка `[8 Б][tag16][ctr4]`), `reference-oracle/voice/strace-media-s3.jsonl`
(10 RTP-пакетов, счётчики 0..7, SR 8..10).
**Write-scope.** `rust/crates/discord-voice/src/udp.rs`,
`rust/crates/discord-voice/src/srtp.rs`, `rust/crates/discord-voice/src/crypto/*`,
`rust/crates/discord-voice/tests/*`.
**DoD.** (1) Red: `rotate_if_due` даёт 28 байт. (2) KDF-вектор: ключ из тестового
`secret_key` → nonce → tag совпадает с оракулом (или документированный отказ с причиной).
(3) `gate:worker`.
**Оценка.** L. **Риск.** без живого `secret_key` вектор строится на синтетике — честно
обозначить статус.

### RE-BE-11. Шлюз: heartbeat fallback и expedited heartbeat
**Проблема.** Заявленный fallback op40→op1 не реализован (M-30); нет ускоренного heartbeat
после сна/смены сети (M-31).
**Цель.** При отказе op40 клиент переходит на op1; после resume/power-события шлёт удар сразу.
**Описание.** Счётчик неудачных op40 (нет ACK/close 4004-подобное) → переключение режима в
супервайзере; API `expedited_heartbeat(reason)`; подписка на power-события — через
`discord-platform::power`.
**Оракул.** Бандл: `_sendHeartbeat` (op40), `expeditedHeartbeat(5e3/15e3)` при
`powerMonitor.on("resume")` и сетевых событиях; `reference-oracle/gateway-protocol.md:158`
(исходящие 90 Б op1 и 111 Б op40 в одном захвате).
**Write-scope.** `rust/crates/discord-core/src/gateway/supervisor.rs`,
`rust/crates/discord-core/src/gateway.rs`, `rust/crates/discord-core/src/ws.rs`,
`rust/crates/discord-core/tests/gateway_*.rs`.
**DoD.** (1) Red: два подряд «нет ACK» → следующий удар op1. (2) Тест expedited: удар в течение
5 с виртуальных часов. (3) Документация совпадает с кодом. (4) `gate:worker`.
**Оценка.** M.

### RE-BE-12. REST: рейт-лимиты — модель bucket и очередь
**Проблема.** Нет `x-ratelimit-limit/reset/scope`, нет очереди по bucket (M-14).
**Цель.** Клиент не «долбит» 429 и уважает `reset` каждого bucket.
**Описание.** Расширить `RateLimitInfo`, ввести реестр bucket'ов (route→bucket id) и ожидание
до `reset`; `Retry-After` — секунды и HTTP-date (M-15).
**Оракул.** `reference-oracle/rest-d02.md:141-143` (единственный 429,
`GET /users/@me/survey`, `run2|95974.8271`), поля `x-ratelimit-*` — документированный контракт
Discord (в дампе не наблюдались, U-01).
**Write-scope.** `rust/crates/discord-core/src/rest/rate_limit.rs`,
`rust/crates/discord-core/src/rest/client.rs`, `rust/crates/discord-core/tests/rate_limit*.rs`.
**DoD.** (1) Red: два запроса в один bucket — второй ждёт `reset`, а не шлётся. (2) `Retry-After:
Wed, 21 Oct 2015 07:28:00 GMT` парсится. (3) `gate:worker`.
**Оценка.** M.

### RE-BE-13. REST: модель ошибок Discord и идемпотентность POST
**Проблема.** Ошибки разбираются точечно (M-17); повтор POST может создать дубль (M-18).
**Цель.** Единый `DiscordError {code,message,errors}`; POST-повтор только с `nonce` или запрещён.
**Описание.** Тип ошибки + каталог известных кодов (40xxx, 50035, 50007, 20011/20013, 70007);
в `ApiClient::execute` для POST: повтор только если тело содержит `nonce` (сообщения) — иначе
`ApiError::Client`.
**Оракул.** `reference-oracle/rest-static.md:96-106` (коды 40xxx/`suspended_user_token`),
`rest-d02.md:252-268` (тело settings-proto обрезано), тела ошибок не захвачены (U-01) —
каталог помечается как «по статике, не подтверждено живьём».
**Write-scope.** новый `rust/crates/discord-core/src/rest/error.rs`,
`rust/crates/discord-core/src/rest/client.rs`, `rust/crates/discord-core/tests/rest_error*.rs`.
**DoD.** (1) Red: `50035` с `errors.field` разбирается в структуру. (2) POST без nonce не
повторяется. (3) `gate:worker`.
**Оценка.** M.

### RE-BE-14. Транспорт: HTTP/2, keep-alive, cookie-jar, прокси
**Проблема.** HTTP/1.1 + `Connection: close` (M-20), нет cookie (M-21), нет прокси (M-22).
**Цель.** Профиль трафика ближе к клиенту; Cloudflare-сессия и корпоративные прокси работают.
**Описание.** Перейти на h2 (или h1 с keep-alive как минимум), cookie-jar с `__dcfduid/
__sdcfduid/__cfruid`, `HTTP_PROXY`/системный прокси; корни — системный store + webpki.
**Оракул.** `reference-oracle/rest-d02.md:294-312` (заголовки в CDP приходят в нижнем регистре — признак h2),
`main-arch.md:707-713` (прокси на уровне ОС), `main-arch.md:680-706` (CA).
**Write-scope.** `rust/crates/discord-core/src/rest/transport.rs`,
`rust/crates/discord-core/src/rest/http1.rs` (либо новый `http2.rs`),
`rust/crates/discord-core/tests/rest_transport*.rs`.
**DoD.** (1) Red: два запроса подряд идут по одному TCP (keep-alive). (2) Set-Cookie
сохраняется и отправляется. (3) Прокси из переменной окружения применяется. (4) `gate:worker`.
**Оценка.** L. **Риск.** тянет новые зависимости — согласовать с TL (правило «без новых
пакетов без разрешения»).

### RE-BE-15. Модель сообщения: member/embeds/attachments/reactions/reference
**Проблема.** `state::Message` — 16 полей, но вложения/embeds только счётчиками, а 3 ключа из 19 отсутствуют (M-37);
embeds/attachments/reactions не типизированы нигде.
**Цель.** Сообщение из шлюза отображается полностью без чтения фикстур.
**Описание.** Добавить `channel_type`, `member` (nick/roles/joined_at/avatar), `attachments[]`,
`embeds[]`, `reactions[]`, `referenced_message`, `nonce`, `sticker_items`, `poll` — по схеме
дапа; `MessagePatch` — те же nullable-поля.
**Оракул.** `reference-oracle/gateway-schema.md:431-500` (19 ключей, `d.author` 13 ключей,
примеры), `reference-oracle/gateway-schema.md:121-200` (объекты user/member в READY).
**Write-scope.** `rust/crates/discord-core/src/state/types.rs` — **общий файл, только через
Team Lead**; воркер готовит: `rust/crates/discord-core/src/state/message_payload.rs` (новый),
`rust/crates/discord-core/src/state/reducer.rs` — тоже общий (через TL),
`rust/crates/discord-core/tests/state_message_payload.rs` (новый).
**DoD.** (1) Red: разбор эталонного MESSAGE_CREATE из фикстуры даёт все 19 ключей.
(2) `null` в embeds не путается с пустым массивом. (3) Регресс существующих тестов состояния.
(4) `gate:worker` (integration-лейн).
**Оценка.** L. **Риск.** общие файлы — согласовать окно с TL, иначе конфликт.

### RE-BE-16. Диспатчи: `USER_SETTINGS_PROTO_UPDATE`, реакции, `VOICE_SERVER_UPDATE`
**Проблема.** Настройки с другого устройства, реакции и голосовой сервер из шлюза
игнорируются состоянием (M-38, M-39, M-40). `READY_SUPPLEMENTAL` при этом уже разобран
(`state/types.rs:1157`), его трогать не нужно.
**Цель.** Изменения применяются без перезапуска.
**Описание.** Типизированные payload'ы для трёх событий: настройки (proto type 1/2),
реакции (add/remove), `VOICE_SERVER_UPDATE` (переиспользовать `voice::VoiceServerUpdate`).
**Оракул.** `reference-oracle/gateway-protocol.md:155-157` (счётчики живых событий:
USER_SETTINGS_PROTO_UPDATE 3, MESSAGE_REACTION_ADD 1, VOICE_SERVER_UPDATE 3);
формы payload'ов — `gateway-schema.md` (PRESENCE_UPDATE/MESSAGE_*) и
`reference-oracle/voice/rtc1-frames.jsonl` не нужны: `VOICE_SERVER_UPDATE` уже разобран в
`discord-voice/src/lib.rs:623-692`.
**Write-scope.** `rust/crates/discord-core/src/state/types.rs` (общий — TL),
`rust/crates/discord-core/src/state/reaction.rs` (новый),
`rust/crates/discord-core/src/state/voice_server.rs` (новый), тесты в
`rust/crates/discord-core/tests/`.
**DoD.** (1) Red на каждое событие. (2) Реакция появляется/исчезает в состоянии.
(3) Настройки обновляют `settings`-снапшот. (4) `gate:worker`.
**Оценка.** M.

### RE-BE-17. READY: типизировать experiments/consents/auth/notification_settings
**Проблема.** 20 из 40 ключей READY типизированы; эксперименты (6/17/1), согласия и
`notification_settings` лежат в `raw` (M-41).
**Цель.** Фичефлаги, согласия и настройки уведомлений читаются типизированно.
**Описание.** Добавить поля в `Ready` и парсеры, сохранив `raw`; тесты на 4 живых READY.
**Оракул.** `reference-oracle/gateway-schema.md:121-166` (40 ключей, размеры секций
`guild_experiments` 7 667 Б, `apex_experiments` 5 533 Б), `reference-oracle/gateway-protocol.md:163-170`.
**Write-scope.** `rust/crates/discord-core/src/ready/snapshot.rs`,
`rust/crates/discord-core/src/ready/value.rs`, `rust/crates/discord-core/tests/ready_*.rs`.
**DoD.** (1) Red: 40/40 ключей разобраны, 0 в `raw` для известных. (2) Эксперименты доступны
селектором по имени. (3) `gate:worker`.
**Оценка.** M.

### RE-BE-18. Платформа: трей + уведомления с переходом в канал
**Проблема.** Трея нет; уведомление не несёт `identifier`/`fallbackDeepLink` (M-49, M-50).
**Цель.** Закрыть окно → трей; клик по уведомлению открывает канал/ЛС.
**Описание.** Модель трея (иконка, состояния меню, minimize-to-tray), D-Bus `Notify` с
`ActionInvoked` → маршрут deep link; поле `identifier` в `Notification`.
**Оракул.** `reference-oracle/backend-ipc.md:44` (9 каналов трея/автозапуска),
`reference-oracle/main-arch.md:325-352` (цепочка уведомления и push
`(action, identifier, userText, fallbackDeepLink)`).
**Write-scope.** `rust/crates/discord-platform/src/notifications.rs`, новый
`rust/crates/discord-platform/src/tray.rs`, `rust/gui-spike/src/main.rs`,
`rust/crates/discord-platform/tests/*`.
**DoD.** (1) Red: `NotificationEvent::Action` несёт `identifier`. (2) Тест маршрутизации
identifier → `Route::Channels`. (3) Трей: модель состояний + тест (без живого D-Bus).
(4) `gate:worker`.
**Оценка.** M.

### RE-BE-19. Deep link: argv, второй инстанс, прочие `action/*`
**Проблема.** Разбираются 6 форм, нет `--url` и `second-instance` (M-51…M-53).
**Цель.** Ссылка из браузера/второго запуска открывает нужный экран.
**Описание.** `extract_url_from_args`, обработчик второго инстанса → `openOrQueueUrl`
(очередь до готовности UI), маппинг остальных `action/*` в `Route::Unknown` с сохранением
пути для лога.
**Оракул.** `reference-oracle/main-arch.md:92` (`extractUrlFromArgs`),
`reference-oracle/main-arch.md:117` (`second-instance`), `main-arch.md:850-851` (префикс
`discord://`, `openOrQueueUrl`).
**Write-scope.** `rust/crates/discord-platform/src/deep_link.rs`,
`rust/gui-spike/src/main.rs`, `rust/crates/discord-platform/tests/deep_link*.rs`.
**DoD.** (1) Red: `--url discord://-/channels/1/2` разбирается. (2) Ссылка до готовности UI
не теряется. (3) Секреты (`token`) не логируются. (4) `gate:worker`.
**Оценка.** S.

### RE-BE-20. Автозапуск: «стартовать свёрнутым» и связь с настройкой
**Проблема.** `.desktop` генерируется, но не пишется; нет `--start-minimized` (M-55).
**Цель.** Настройка «запускать при входе» реально работает и уважает «свёрнутым в трей».
**Описание.** Запись/удаление файла, аргумент `--start-minimized`, чтение при старте,
синхронизация с настройкой UI.
**Оракул.** `reference-oracle/backend-ipc.md:44` (`TOGGLE_OPEN_ON_STARTUP`,
`UPDATE_OPEN_ON_STARTUP`, `TOGGLE_START_MINIMIZED`); XDG Desktop Entry 1.5 (уже в
`autostart.rs`).
**Write-scope.** `rust/crates/discord-platform/src/autostart.rs`,
`rust/gui-spike/src/main.rs`, `rust/gui-spike/src/settings_store.rs`,
`rust/crates/discord-platform/tests/autostart*.rs`.
**DoD.** (1) Red: запись файла и повторное чтение идемпотентны. (2) `--start-minimized`
в Exec. (3) Выключение удаляет/деактивирует запись. (4) `gate:worker`.
**Оценка.** S.

### RE-BE-21. Апдейтер: сверка с main-архитектурой (splash-автомат, build override)
**Проблема.** `discord-updater` не сверен с §5 `main-arch.md`: нет splash-конечного автомата и
build override cookie (M-56).
**Цель.** Контракт обновления совпадает с клиентом, состояния видны UI.
**Описание.** Сопоставить `manifest.rs`/`plan.rs` с шагами §5.1–5.5, добавить состояния
`checking-for-updates → launching` и cookie `build-override`.
**Оракул.** `reference-oracle/main-arch.md:523-596` (выбор апдейтера, splash-автомат,
Linux-специфика, legacy moduleUpdater).
**Write-scope.** `rust/crates/discord-updater/src/*`, `rust/crates/discord-updater/tests/*`.
**DoD.** (1) Таблица соответствия шагов §5 ↔ функции. (2) Red-тест на пропущенный шаг.
(3) `gate:worker`.
**Оценка.** M.

---

## P3 — покрытие и полировка

### RE-BE-22. REST: вложения (upload/refresh) и вставка изображений
**Оракул.** `rest-static.md:143` (`APPLICATION_UPLOAD_ATTACHMENT`), enum
`ATTACHMENTS_REFRESH_URLS` в `rest-static.md` §6; `rest-d02.json` — маршрут не наблюдался.
**Write-scope.** `rust/crates/discord-core/src/rest/routes.rs`, новый
`rust/crates/discord-core/src/rest/attachments.rs`, `rust/gui-spike/src/messages.rs`.
**DoD.** Red на multipart-форму и refresh; 413/415 классифицируются; `gate:worker`. **S/M.**

### RE-BE-23. REST: треды (создать/список/архив)
**Оракул.** `rest-static.md:251-252`, чтение `THREAD_LIST_SYNC` — `gateway-protocol.md:157`.
**Write-scope.** `rust/crates/discord-core/src/rest/routes.rs`, новый
`rust/crates/discord-core/src/rest/threads.rs`, `rust/crates/discord-core/src/state/types.rs`
(через TL).
**DoD.** Red на create/list; `THREAD_LIST_SYNC` применяется к состоянию; `gate:worker`. **M.**

### RE-BE-24. REST: закрепление сообщений и реакции (список/снять все)
**Оракул.** `rest-static.md:226` (`PINS`), `:232-233` (снятие реакций).
**Write-scope.** `rust/crates/discord-core/src/rest/routes.rs`, `rust/gui-spike/src/messages.rs`,
`rust/gui-spike/src/reactions.rs`.
**DoD.** Red на PUT/DELETE pin и DELETE reactions; `gate:worker`. **S.**

### RE-BE-25. REST: read-states пачкой и настройки пользователя
**Оракул.** `rest-static.md:506` (`ack-bulk`), `:843` (`/users/@me/settings`).
**Write-scope.** `rust/crates/discord-core/src/rest/routes.rs`, новый
`rust/crates/discord-core/src/rest/settings_api.rs`, `rust/gui-spike/src/typing.rs`.
**DoD.** Red на bulk-ack (тело `{read_states:[…]}`) и GET/PATCH settings; `gate:worker`. **M.**

### RE-BE-26. REST: взаимодействия и команды приложений
**Оракул.** `rest-static.md:453` (`/interactions`), `:139-160` (applications/commands).
**Write-scope.** `rust/crates/discord-core/src/rest/routes.rs`, новый
`rust/crates/discord-core/src/rest/interactions.rs`, `rust/gui-spike/src/messages.rs`.
**DoD.** Red на POST interactions (кнопка → ответ 200/204); `gate:worker`. **M.**

### RE-BE-27. REST: эмодзи/стикеры/приглашения (управление)
**Оракул.** `rest-static.md:526-527` (sticker-packs), `INVITES`/`GUILD_EMOJIS` в §6.
**Write-scope.** `rust/crates/discord-core/src/rest/routes.rs`, `rust/gui-spike/src/invites.rs`,
новый `rust/crates/discord-core/src/rest/emojis.rs`.
**DoD.** Red на list/create/delete; `gate:worker`. **M.**

### RE-BE-28. Голос: окно `op15/op16` типизировано, `op2` устойчив к отсутствию `experiments`
**Оракул.** `voice/rtc1-frames.jsonl` (op2 с `ip/experiments`, op15 `{any:100}`,
op16 `{voice,rtc_worker}`), `voice-deep.md:21-30`.
**Write-scope.** `rust/crates/discord-voice/src/lib.rs`, `rust/crates/discord-voice/tests/*`.
**DoD.** Red на parse op15/16; Ready без `experiments` не падает; `gate:worker`. **S.**

### RE-BE-29. Профиль: `window_manager ",unknown"`, `for_rw_client` дефолты
**Оракул.** `rest-d02.md:338-339` (`KDE,unknown`), `:352-353` (сборки), `identify.rs:67-90`.
**Write-scope.** `rust/crates/discord-core/src/rest/identity.rs`,
`rust/crates/discord-core/src/identify.rs`.
**DoD.** Red на `KDE,unknown`; дефолты `x64`/UUID v4; `gate:worker`. **S.**

### RE-BE-30. Платформа: clipboard, WebAuthn, clips, NTP, process utils — только карта, без реализации
**Оракул.** `backend-ipc.md:44` (группы каналов), `backend-ipc.json:bridges`.
**Write-scope.** `waves/RW-40/RE-AUDIT/` (документ) + `docs/rust-client/PROJECT-STRUCTURE.md`
(через TL).
**DoD.** Таблица «канал → нужная функция → приоритет» с решением «делаем/не делаем»;
`gate:worker` не требуется (docs-only). **S.**

---

## Сводка для PM

| Приоритет | Карточки | Суммарная оценка |
|---|---|---|
| P1 | RE-BE-01…06 | 1×L, 4×M, 1×S |
| P2 | RE-BE-07…21 | 4×S, 8×M, 3×L |
| P3 | RE-BE-22…30 | 3×S, 5×M, 1×S/M, 1×docs |

Порядок запуска (зависимости):
1. Параллельно: RE-BE-01 (шлюз), RE-BE-02, RE-BE-04/05/06 (REST-функции, разные файлы, но
   общий `routes.rs` — развести по одному воркеру или сериализовать).
2. После RE-BE-01: RE-BE-15/16/17 (модель данных поверх живого потока).
3. Голос: RE-BE-03 → RE-BE-09 → RE-BE-10.
4. Платформа: RE-BE-18 → RE-BE-19/20.

Ограничения, которые надо явно принять:
- DAVE/MLS (RE-BE-09) и wire-крипто (RE-BE-10) не имеют живого оракула — только код клиента;
  приёмка «по форме кадра», не «по расшифровке».
- Тела REST-ответов в дампе отсутствуют (U-01) — схемы ответов берутся из шлюза/статики,
  это надо указывать в DoD каждой REST-карточки.
- Схема опций уведомлений не восстановлена (нативный модуль не дизассемблирован) —
  RE-BE-18 закрывает поведение, не wire-совместимость с Discord.
