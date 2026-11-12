# RE-GAP: бэкенд и протокол discord-lab против RE-дампа Discord 1.0.162 (часть 2)

> **Статус файла.** Это второй, независимый проход RE-1 по task-130. Канонический
> `RE-GAP-BACKEND-PROTOCOL.md` занят отчётом второго воркера той же карточки (794 строки);
> его работа не перезаписывалась. Настоящий документ и `PM/BACKLOG-RE-BACKEND-PART2.md` —
> мой результат; расхождения двух проходов полезно свести на приёмке.

**Карточка:** task-130 (RE-1), волна RW-40.
**Дата:** 2026-10-30. **Режим:** read-only по продуктовому коду; `cargo build/test` не запускались.
**Что сверялось:** `rust/gui-spike/src/*.rs`, `rust/crates/discord-core/src/**`,
`rust/crates/discord-voice/src/**`, `rust/crates/discord-platform/src/**` (≈35 тыс. строк).
**Эталон:** `reference-oracle/` — `README.md`, `rest-d02.{md,json}`, `rest-static.{md,json}`,
`gateway-protocol.{md,json}`, `gateway-schema.{md,json}`, `backend-ipc.{md,json}`,
`main-arch.{md,json}`, `voice-deep.{md,json}`, `decoded-index.json`, `decoded/`,
`out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` (веб-клиент, 12,4 МБ).

---

## 0. Метод и границы

1. Прочитан весь протокольный слой нашего кода: REST (`routes`, `headers`, `identity`,
   `super_properties`, `client`, `rate_limit`, `retry`, `telemetry`, `transport`, `http1`,
   `user`, `resources`, `ack`), шлюз (`gateway`, `ws`, `identify`, `resume`, `dispatch`,
   `ready/*`, `state/*`), голос (`lib`, `udp`, `srtp`, `media/*`, `regions`, `reconnect`,
   `rtcp`, `reception`), платформа (`notifications`, `power`, `deep_link`, `autostart`,
   `spellcheck`, `keyring`, `dbus/*`), BFF (`gui-spike/src/*`).
2. Каждое расхождение подтверждено **двумя** источниками там, где это возможно: живой захват
   (`rest-d02.json`, `gateway/*`, `voice/*.jsonl`) и статика клиента (бандл, `rest-static.md`,
   `main-arch.md`). Где источники расходятся (U-08), это указано явно — догадка не выдаётся
   за факт.
3. Живые REST-тела ответов в дампе отсутствуют (`rest-d02.md` §5, U-01), поэтому схемы ответов
   сверялись по `gateway-schema.md` (READY/MESSAGE_*) и статике, а не по телам REST.
4. Инструменты REA (`analyze_javascript_application`, `open_binary`, `trace_javascript_semantics`)
   не понадобились: все нужные факты уже лежат в дампе и в распакованном бандле; вызовы REA
   добавили бы риск без нового знания. Это ограничение метода, а не пропуск требования.

---

## 1. Сводка

| Область | Совпадение с RE | P1 | P2 | P3 |
|---|---|---:|---:|---:|
| REST v9: каталог маршрутов, тела, заголовки | высокое по 47 live-маршрутам; низкое по покрытию функций | 1 | 6 | 3 |
| REST: рейт-лимиты, ошибки, транспорт | частичное | 1 | 4 | 1 |
| Шлюз: URL, HELLO, heartbeat, IDENTIFY, RESUME | высокое, одна существенная ошибка (RESUME-окно) | 1 | 3 | 2 |
| Шлюз: zstd-пайплайн | **блокер**: теряется 60 % диспатчей в живом сокете | 1 | 0 | 0 |
| Шлюз: READY/диспатчи/состояние | схемы READY полные; payload-модели частичные | 0 | 4 | 2 |
| Голос: сигнализация | формы 0/1/2/4 разобраны; нет 5 (Speaking), 7/9 (Resume), DAVE | 2 | 2 | 2 |
| Голос: UDP/SRTP/RTP | высокое (байтовые константы совпадают) | 0 | 1 | 1 |
| Платформа/IPC | осознанно другой стек; карта возможностей неполная | 0 | 2 | 5 |
| **Итого находок** | | **6** | **22** | **16** |

Карточек в бэклоге: 6 P1, 15 P2, 9 P3 (`PM/BACKLOG-RE-BACKEND-PART2.md`).

Приоритеты: **P1** — ломает базовый сценарий (логин/шлюз/голос/чтение данных);
**P2** — расхождение с протоколом, которое сервер терпит сегодня, но оно ломает паритет и
диагностику; **P3** — расширение покрытия и полировка.

---

## 2. REST v9

### 2.1 Каталог маршрутов

| Что | Наш код | Оракул | Вердикт |
|---|---|---|---|
| 47 live-маршрутов D-02 | `rust/crates/discord-core/src/rest/routes.rs:71-161`, `ROUTES[49]` | `reference-oracle/rest-d02.json` → `endpoints[0..46]`, `rest-d02.md:63-143` | ✔ Все 47 присутствуют (сверено множествами: `LIVE − OURS = ∅`) |
| Сверх каталога | `POST /channels/{id}/messages` (`routes.rs:72-76`), `GET /guilds/{id}/messages/search` (`routes.rs:128`) | `POST` подтверждён `NETWORK-CONTRACTS.md` §1.1; `GET search` — `rest-static.md:885` (enum `SEARCH_GUILD`) + `README.md:32` (живой поиск, 3 302 результата) | ✔, но docstring `routes.rs:4-5` говорит «47 + POST» — фактически 47 + 2 |
| `[S]`-расширения | `TYPING_PATH` (`routes.rs:185`), `INVITE_PATH` (`routes.rs:194`), `EXTENSIONS[3]` (`routes.rs:209-213`) | `rest-static.md:253` (typing), `NETWORK-CONTRACTS.md` §1.2/§1.4 | ✔ |
| Query-параметры | каталог их не хранит; пагинация собирается в `history/pagination.rs:89-92` (`limit`, `before`, `after`, `around`) | `rest-d02.md:145-183` — 45 имён, пагинация `limit=12`, `before=<id>&limit=24`, `limit=30&around=<id>` | ✔ для сообщений; ✘ для остальных маршрутов (см. 2.8) |

Каталог — правильный «снимок D-02», но он не является картой функций клиента: 47 маршрутов
покрывают только то, что Discord вызвал за одну сессию. Это главная причина дефицита
эндпоинтов (2.8).

### 2.2 Заголовки запроса

Наш код отправляет ровно шесть заголовков (`rust/crates/discord-core/src/rest/headers.rs:16-23`,
`client.rs:185-194`):

`Authorization`, `X-Super-Properties`, `X-Discord-Locale`, `X-Discord-Timezone`,
`X-Debug-Options`, `Accept-Language`.

| # | Заголовок | Наш код | RE | Расхождение | P |
|---|---|---|---|---|---|
| M-01 | `Accept-Language` | = локаль без q-списка (`client.rs:166`; TODO `client.rs:112-114`) | бандл, `prepareRequest` (offset 1 001 996): при `isPlatformEmbedded` собирается `navigator.languages` в q-список: первый без `q`, далее `;q=0.9`, `;q=0.8`, … | реальный клиент шлёт `ru,ru-RU;q=0.9,…`, мы — `ru`. Сервер выбирает язык ответа по q-списку | P2 |
| M-02 | `X-Fingerprint` | нет | бандл там же: `getFingerprint()` → `set("X-Fingerprint", …)`, если непустой; статика `rest-static.md:39` | живой D-02 его не видел (`rest-d02.md:400-404`, U-08) — значит, либо fingerprint пуст, либо CDP не отдал; **однозначного вывода нет**. Наш клиент не умеет его отправлять вообще | P2 |
| M-03 | `X-Installation-ID` | нет | бандл там же: `getInstallationForTracking()`; `rest-static.md:39` | то же противоречие статика↔живое (U-08); в IDENTIFY установка передаётся только при fast-connect | P2 |
| M-04 | `X-Routing-Key` | нет | бандл: `getRoutingKeyHeaderValue()` | не отправляется | P3 |
| M-05 | `x-client-trace-id` | нет | бандл: только при `isTracingRequests`, вместе с отладочной ссылкой Datadog | служебный, на протокол не влияет | P3 |
| M-06 | `X-Context-Properties` | нет | `rest-d02.md:400-402` — 8 запросов на 3 маршрутах (`quests/decision`×3, `quests/get-decisions`×3, `guilds/{id}/members/@me`×2), base64 JSON | часть контракта входа в гильдию/квестов | P3 |
| M-07 | `X-Audit-Log-Reason` | нет | бандл: один литерал; модерационные вызовы | нет модерации — не блокер | P3 |
| M-08 | `X-Failed-Requests`, `x-science-test` | нет | `rest-d02.md:402` — только на `/science` | телеметрия у нас запрещена политикой (`telemetry.rs:11`) — осознанно | — |

Ключевое: наш набор совпадает с **живым** захватом, но не со **статикой** клиента. Решение
«только 6 заголовков» верно для реплея D-02 и неверно как модель клиента. Нужен конфиг:
включать/выключать fingerprint, installation-id, routing-key, context-properties.

### 2.3 `X-Super-Properties`

| Параметр | Наш код | RE | Вердикт | P |
|---|---|---|---|---|
| Число полей | 22 (`super_properties.rs:18`, `field_names()` `:140-165`) | 22 ключа в живом заголовке (`rest-d02.md:314-372`) | ✔ | — |
| Порядок | канонический, как в захвате (`:81-109`) | порядок захвата | ✔ | — |
| `client_event_source` | `Option<String>`, `null` (`:60`, `:101`) | `null` | ✔ | — |
| Писатель JSON | свой, компактный, RFC 8259 (`:177-258`) | JSON.stringify | ✔ | — |
| Мутация во времени | **нет**: `ApiAuth` — снимок (`client.rs:22-36`, `:120-168`) | бандл, модуль `618651`: `extendSuperProperties` на каждой смене фокуса обновляет `client_app_state`, а на новую сессию — `client_heartbeat_session_id` | M-09: наша шапка «замерзает» на `focused` и одном UUID; после смены фокуса/сессии расходится с клиентом | P2 |
| `client_build_number` по умолчанию | `0` (`identity.rs:287`) | живой `641004` (`rest-d02.md:352`), бандл `parseInt("641004")` | M-10: дефолт даёт заголовок, неотличимый от «неизвестной сборки» | P2 |
| `native_build_number` | `0` (`identity.rs:288`) | `94186` (`rest-d02.md:353`) | то же | P2 |
| `os_arch` / `app_arch` | `x86_64→x64` (`identity.rs:158-166`) | `x64` | ✔ | — |
| `window_manager` | `XDG_CURRENT_DESKTOP` как есть (`identity.rs:281-283`) | `KDE,unknown` (`rest-d02.md:338`) | M-11: клиент добавляет `,unknown` (DE, WM); у нас просто `KDE` | P3 |
| `distro` | `PRETTY_NAME` дословно, с кавычками (`identity.rs:202-208`) | `"Arch Linux"` — кавычки часть значения (`rest-d02.md:339`) | ✔ | — |
| `launch_signature` | UUID v4 (`identity.rs:264`, `:227-243`) | UUID v4 | ✔ (формат) | — |

### 2.4 Авторизация и вход

| # | Что | Наш код | RE | Вердикт | P |
|---|---|---|---|---|---|
| — | Схема `Authorization` | сырой токен по умолчанию, `Bot`/`Bearer` опционально (`client.rs:61-80`) | `rest-static.md:38`, `:70` — сырой токен без `Bearer`; живое — значения вырезаны (U-06) | ✔ | — |
| M-12 | Регистр имени `Authorization` | всегда `Authorization` (`client.rs:187`) | живое: 3 запроса `GET /users/@me` ушли с `authorization` (нижний регистр), `rest-d02.md:398`; это артефакт CDP/HTTP2 (U-07) | не дефект, но зафиксировать | — |
| — | Тело `/auth/login` | `{login,password,undelete:false,login_source:null,gift_code_sku_id:null}` (`gui-spike/src/auth.rs:710-721`) | `rest-static.md:172` — те же 5 ключей | ✔ | — |
| — | Тело `/auth/mfa/{method}` | `{code,ticket,login_source,gift_code_sku_id,login_instance_id}` (`auth.rs:726-734`) | `rest-static.md:175` | ✔ | — |
| — | Повторы логина | `post_once`, без ретраев (`client.rs:475-498`) | ADR-3 | ✔ | — |
| G-01 | `/auth/logout` | только удаление токена из keyring (локально, `auth.rs:1436-1461`) | `rest-static.md:101`, `:173` — `POST /auth/logout {provider,token,voip_provider,voip_token}` | **серверная сессия не закрывается**: «выход» оставляет устройство в списке сессий | P2 |
| G-02 | `/auth/sessions` | нет | `rest-static.md:181` (в кэше 1×), `POST /auth/sessions/logout` `:182` | нет управления устройствами | P3 |
| G-03 | `/auth/location-metadata` | нет | `rest-static.md:171` (кэш 1×), `rest-d02.md:485` (в захвате не было) | нужен экран согласий/captcha до логина | P3 |

### 2.5 Рейт-лимиты, повторы, ошибки

| # | Что | Наш код | RE | Вердикт | P |
|---|---|---|---|---|---|
| M-13 | Разбор 429 | `retry-after` → тело `retry_after` → `x-ratelimit-reset-after` (`rate_limit.rs:146-179`) | живой 429 один (`GET /users/@me/survey`, `rest-d02.md:143`), тела не захвачены (U-01) | базовый разбор верен; приоритет заголовка над телом — не подтверждён живыми данными | — |
| M-14 | Модель bucket | `bucket`, `remaining`, `reset_after`, `global` (`rate_limit.rs:17-28`) | Discord-поля `x-ratelimit-*`; в дампе не наблюдались | ✘ нет `x-ratelimit-limit`, `x-ratelimit-reset` (unix), `x-ratelimit-scope`; нет очереди по bucket (клиент ждёт до `reset`, а не повторяет вслепую) | P2 |
| M-15 | `Retry-After` | только секунды (`rate_limit.rs:66-68`) | RFC 9110 допускает HTTP-date | ✘ дата не парсится → повтор сразу | P3 |
| M-16 | Политика повторов | 429 повторяют все методы, 5xx — только идемпотентные, 3 попытки, потолок 30 с (`client.rs:210-219`, `rate_limit.rs:210-236`) | клиент: `gatewayBackoff=Backoff(1e3,6e4)` для шлюза; для REST в дампе данных нет | ✘ max_retry_after 30 с < клиентских 60 с (для REST не подтверждено) | P3 |
| M-17 | Коды ошибок Discord | разбираются точечно: `401`, `50018`, `60006/60008/60009`, `70007/20011/20013` (`auth.rs:120-140`, `:809-860`) | полный каталог кодов в дампе отсутствует; `rest-d02.md` §5 — тела не захвачены | ✘ нет общей модели `{code,message,errors{field:[...]}}` (код `50035` — ошибки валидации формы) | P2 |
| M-18 | 429 на `POST` | повтор с тем же телом | Discord идемпотентен по `nonce` только для сообщений; у нас nonce не гарантируется при повторе | ✘ повтор `POST /channels/{id}/messages` может создать дубль, если `nonce` не задан вызывающим | P2 |

### 2.6 Транспорт

| # | Что | Наш код | RE | Вердикт | P |
|---|---|---|---|---|---|
| M-20 | HTTP-версия | HTTP/1.1, один запрос на соединение, `Connection: close` (`transport.rs:1-4`, `:297-301`) | клиент — Chromium/Electron, HTTP/2 к `discord.com` (ALPN; заголовки в нижнем регистре в CDP) | ✘ нет h2 и keep-alive: больше TLS-хендшейков, другой профиль трафика; при этом протокол совместим | P2 |
| M-21 | Cookie | нет cookie-jar; `Cookie` — секретный заголовок в логах (`transport.rs:69`) | живое: `Cookie: 0` (U-05), но Cloudflare ставит `__dcfduid/__sdcfduid/__cfruid`; без них возможен challenge | ✘ нет хранения/отправки cookie | P2 |
| M-22 | Прокси | нет | `main-arch.md:707-713` — прокси на уровне Chromium/системы, в asar не переопределяется | ✘ наш клиент игнорирует системный/переменные прокси | P2 |
| M-23 | Корни TLS | `webpki-roots` (`transport.rs:44-52`) | `main-arch.md:680-706` — `cacert.pem` как заготовка, реально системные CA Chromium | ✘ корпоративный MITM/системные CA не работают | P3 |
| M-24 | Compression ответов | не запрашивается `Accept-Encoding`, тело читается как есть (`transport.rs:290-330`) | клиент принимает gzip/br | ✘ нет gzip: трафик больше, но протокол не ломается | P3 |

### 2.7 Формы тел запросов (сверка с живым захватом)

| Маршрут | Наш код | Живой D-02 | Вердикт |
|---|---|---|---|
| `POST /channels/{id}/messages` | ключи `content,nonce,tts` (`routes.rs:75`; отправка `messages.rs:159`) | не вызывался в D-02; контракт `NETWORK-CONTRACTS.md` §1.1 | ✔ (по контракту) |
| `POST .../ack` | `{token,last_viewed}` (`routes.rs:83`), `ack.rs` | `{"token":null,"last_viewed":4310}` (`rest-d02.md:207-214`) | ✔ |
| `PUT /guilds/{id}/members/@me` | тело `{}` (`routes.rs:132`, `guilds.rs:347-353`), query `?lurking=` | `{}` (`rest-d02.md:225-233`) | ✔ |
| `POST /users/@me/billing/user-offer` | ключ `allow_triggers` (`routes.rs:110`) | `{allow_triggers}` (`rest-d02.md:216-224`) | ✔ |
| `PATCH /channels/{id}/explicit-media` | `message_ids` (`routes.rs:142`) | `{message_ids:[…]}` (`rest-d02.md:243-251`) | ✔ |
| `POST /beaker` | `token,events` (`routes.rs:138`) | `{token,events}` (`rest-d02.md:234-242`) | ✔, но политика телеметрии блокирует (`telemetry.rs:48-53`) — осознанно |
| `PUT .../reactions/{emoji}/@me` | тело `{}` + `Content-Type: application/json` (`reactions.rs:126-135`) | **тела нет** (`reqBodyKeys: []`, 204, `rest-d02.json` endpoints[38]) | M-25: лишнее тело; сервер терпит, но это отклонение |
| `DELETE /users/@me/guilds/{id}` | тело `{"lurking":…}` (`guilds.rs:356-360`) | в D-02 не наблюдался; статика метода без тела (`rest-static.md:657`) | M-26: форма тела не подтверждена RE |
| `PATCH /channels/{id}/messages/{id}`, `DELETE ...` | путь склеивается строкой, каталог не участвует (`messages.rs:221`, `:240`) | `rest-static.md:200` (`CHANNEL`), `:224` (`MESSAGE`), подтверждено статикой | M-27: нет валидации маршрута/тела каталогом — расхождение может пройти незамеченным |
| `POST /channels/{id}/typing` | без тела (`routes.rs:210`, `typing.rs:213`) | `rest-static.md:253` | ✔ |

### 2.8 Чего нет в бэкенде: отсутствующие эндпоинты и функции

Ниже — только те, что нужны для паритета базового клиента (не «весь Discord»). Все строки
`rest-static.md` — записи enum `Endpoints` (904 шт.), метод определён вызовом.

| # | Функция | Метод и путь | Оракул | Наш код | P |
|---|---|---|---|---|---|
| G-10 | Друзья: заявка/принять/удалить | `PUT/DELETE/PATCH /users/@me/relationships/{id}` | `rest-static.md:694` | нет | P1 |
| G-11 | Друзья: список | из READY `relationships` (60 записей) | `gateway-schema.md:121-160`, `ready/social.rs` | ✔ чтение; записи — нет | P2 |
| G-12 | Друзья: игнор, bulk | `PUT/DELETE /users/@me/relationships/{id}/ignore`, `/relationships/bulk` | `rest-static.md:695`, `:772` | нет | P3 |
| G-13 | Создать ЛС/группу | `POST /users/@me/channels {recipients}` | `rest-static.md:616` | нет | P1 |
| G-14 | Покинуть группу/bulk-leave | `POST /users/@me/channels/bulk-leave`, `/group-dm-shell` | `rest-static.md:617-618` | нет | P3 |
| G-15 | Закрыть/изменить канал | `GET/PATCH/DELETE /channels/{id}` | `rest-static.md:200` | нет | P1 |
| G-16 | Создать канал в гильдии | `POST/PATCH /guilds/{id}/channels` | `rest-static.md:340` | нет | P2 |
| G-17 | Список участников | `GET /guilds/{id}/members`, `POST /members-search` | `rest-static.md:779`, `:370` | только из `GUILD_MEMBER_LIST_UPDATE`/`GUILD_MEMBERS_CHUNK` (`state/member_list.rs`, `member_chunk.rs`) | P2 |
| G-18 | Ник/роль участника | `PATCH /guilds/{id}/members/{id}/nick`, `/members/@me` | `rest-static.md:372`, `:375` | нет | P3 |
| G-19 | Кик/бан | `DELETE /guilds/{id}/members/{id}` | `rest-static.md:95` (метод `?`) | нет | P3 |
| G-20 | Список гильдий пользователя | `GET /users/@me/guilds` | в enum есть (`rest-static.md` §6, ключ `USER_GUILDS`) | только READY | P3 |
| G-21 | Ветки: создать/список/архив | `POST /channels/{id}/threads`, `GET /channels/{id}/threads/...` | `rest-static.md:251` | только `GET /threads/search` (`routes.rs:150`) и чтение `THREAD_LIST_SYNC` | P2 |
| G-22 | Закрепить/открепить | `PUT/DELETE /channels/{id}/messages/pins/{id}`, `.../messages/{id}/pins` | в enum `PINS` (`rest-static.md:226`) | только `GET .../pins` | P2 |
| G-23 | Реакции: список, снять все | `GET .../reactions/{emoji}`, `DELETE .../reactions` | `rest-static.md:232-233` | только toggle PUT/DELETE (`reactions.rs`) | P3 |
| G-24 | Bulk-delete, crosspost | `POST /channels/{id}/messages/bulk-delete`, `.../crosspost` | `rest-static.md:230` | нет | P3 |
| G-25 | Read states пачкой | `POST /read-states/ack-bulk` | `rest-static.md:506` | только одиночный ack (`routes.rs:82`) | P2 |
| G-26 | Настройки пользователя | `GET/PATCH /users/@me/settings` | `rest-static.md:843` | нет; есть только proto (`settings/wire.rs`) | P2 |
| G-27 | Proto-настройки: запись | `PATCH /users/@me/settings-proto/{type}` | `rest-d02.md:252-268` (тело обрезано на 4001 Б) | маршрут есть (`routes.rs:155`), энкодера нет — только декод | P2 |
| G-28 | Вложения: upload + refresh | `POST /channels/{id}/attachments`, `/attachments/refresh-urls` | `rest-static.md:143`; enum `ATTACHMENTS_REFRESH_URLS` | нет | P2 |
| G-29 | Эмодзи/стикеры гильдии | `GET/POST /guilds/{id}/emojis`, `GET /sticker-packs` | `rest-static.md:526-527` | нет | P3 |
| G-30 | Приглашения: создать/список/удалить | `POST /channels/{id}/invites`, `GET /guilds/{id}/invites` | в enum `INVITES` | только resolve/accept (`invites.rs`, `routes.rs:194`) | P3 |
| G-31 | Взаимодействия (кнопки/селекты) | `POST /interactions` | `rest-static.md:453` | нет | P2 |
| G-32 | Команды приложений | `GET/PUT /applications/{id}/commands`, `POST .../interactions` | `rest-static.md:139-160` | только индекс команд из READY/`application-command-index` | P3 |
| G-33 | Webhooks | `GET/POST/PATCH/DELETE /webhooks/...`, execute | enum `WEBHOOKS` | нет | P3 |
| G-34 | OAuth2 authorize/token | `GET/POST /oauth2/authorize`, `/oauth2/token` | `rest-static.md:97` | нет (deep link маршрут есть, сетевого вызова нет) | P3 |
| G-35 | Приглашение по deep link `login/one-time` | `POST /auth/one-time-login` | `rest-static.md:176` | разбор ссылки есть (`deep_link.rs`), вызова нет | P3 |
| G-36 | Профиль: заметки (запись) | `PUT /users/@me/notes/{id}` | `rest-static.md` (`NOTE`) | только `GET` (`routes.rs:137`) | P3 |
| G-37 | Эксперименты | `GET /experiments`, `/apex/experiments` | `rest-static.md:131`, `:292`; при токене приходят в READY | нет; READY-поля не типизированы (см. 3.6) | P2 |
| G-38 | Регионы голоса | нет REST-маршрута у клиента; `geo_ordered_rtc_regions` в READY | `gateway-schema.md:146`; `rest-d02.md:493` (в захвате нет) | ✔ список читается (`ready`), latency-проба — модель (`regions.rs:1-3`) | — |
| G-39 | Телеметрия `science/track` | `POST /science`, `/track` | `rest-d02.md:394-404` | заблокировано политикой (`telemetry.rs`) — осознанно | — |

**Итог по REST:** реализовано 49 маршрутов + 3 расширения; для базового паритета не хватает
≈24 функций (G-10…G-32), из них 3 — P1 (друзья, создание ЛС, управление каналом).

---

## 3. Шлюз

### 3.1 Адрес и параметры

| Что | Наш код | RE | Вердикт |
|---|---|---|---|
| База | `wss://gateway.discord.gg` (`ws.rs:77`) | `rest-static.md:48`, живой сокет `rest-d02.json` → `ws.connections` | ✔ |
| Query | `encoding=etf&v=9&compress=zstd-stream` (`ws.rs:80`) | живой URL `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream` | ✔ |
| `resume_gateway_url` | нормализуется, старый query отбрасывается (`ws.rs:162-200`) | READY `resume_gateway_url` = `wss://gateway-us-east1-c.discord.gg` (`gateway-protocol.md:163`) | ✔ |
| Сжатие | один `ZstdStream` на соединение, `end()` не вызывается (`ws.rs:202-321`) | `gateway-protocol.md:99-127` | ✔ по схеме |

### 3.2 Heartbeat и таймеры

| # | Что | Наш код | RE | Вердикт | P |
|---|---|---|---|---|---|
| — | Интервал | из `HELLO.d.heartbeat_interval` (`gateway.rs:376-397`) | 41 250 мс на всех 7 сокетах (`gateway-protocol.md:150`) | ✔ | — |
| — | Первый удар | `jitter_seed % interval` (`gateway.rs:394-395`) | `setTimeout(…, floor(random*interval))` (бандл `_startHeartbeater`) | ✔ детерминированная замена | — |
| — | Форма удара | op40 `{seq,qos}` в боевом режиме, op1 — legacy (`ws.rs:413-469`) | `_sendHeartbeat`: `send(QOS_HEARTBEAT,{seq,qos})` (бандл, offset 5 020 363); исходящие 90 Б (op1) и 111 Б (op40), `gateway-protocol.md:158` | ✔ | — |
| M-30 | Заявленный fallback op40→op1 | в комментариях (`gateway.rs:41-52`, `ws.rs:753`) | клиент всегда op40 | ✘ код `HeartbeatMode::LegacyOp1` в супервайзере **никогда** не выбирается (`gateway/supervisor.rs:295-298`); fallback не реализован, документация врёт | P2 |
| M-31 | Expedited heartbeat | нет | бандл: `powerMonitor.on("resume")` → `expeditedHeartbeat(5e3)`, сеть: offline 15 с, online 5 с | ✘ после сна/смены сети мы ждём до 41,25 с и можем словить zombie | P3 |
| — | Зомби-детектор | `sent + interval` без ACK → `Reconnect{Zombie}` (`gateway.rs:297-319`, `:371-374`) | клиент: `lastHeartbeatAckTime` + интервал | ✔ | — |

### 3.3 RESUME и реконнект

| # | Что | Наш код | RE | Вердикт | P |
|---|---|---|---|---|---|
| M-32 | Окно resume | `RESUME_MAX_DOWNTIME_MS = 30_000` (`resume.rs:19`), применяется супервайзером | бандл `_doResumeOrIdentify` (offset 5 019 755): resume, если с последнего ACK прошло `<= tI`, где `tI = 3 * Millis.MINUTE` | **расхождение в 6 раз**: после 40 с разрыва мы шлём IDENTIFY там, где клиент резюмирует сессию | P1 |
| — | Форма RESUME | `{op:6,d:{token,session_id,seq}}` (`resume.rs:107-126`) | `rest-static.md:56`, `:84`; исходящие 173/176 Б (`gateway-protocol.md:158`) | ✔ | — |
| — | `op7` | `Reconnect{ServerRequest}` (`gateway.rs:274`) | 1× в захвате, далее RESUME с продолжением `s` (`gateway-protocol.md:155`) | ✔ | — |
| — | `op9 d` | сохраняется (`ws.rs:277-281`), `plan_reconnect` учитывает `false` (`resume.rs:145-160`) | `rest-static.md:84` | ✔ | — |
| — | Close-коды | 4004 → Stop(Unauthorized), 4013/4014 → Stop (`supervisor.rs:226-231`) | контракт AUTH-RE G-18 (репозиторий); в дампе close-коды не наблюдались | ✔ по контракту | — |
| M-33 | Backoff | `Backoff(1e3,6e4)` (`supervisor.rs:73-76`), но в голосовом крейте второй набор до 30 с (`discord-voice/src/reconnect.rs:14`) | клиент: `new Backoff(1e3,6e4)` (бандл, конструктор `GatewaySocket`) | ✘ две разные политики в одном продукте; при использовании `voice::reconnect` потолок 30 с | P2 |
| M-34 | `op1` от сервера в Qos-режиме | немедленный удар + перезапуск интервала (`gateway.rs:411-423`) | `_handleHeartbeatReceive` у клиента | ✔ (по коду клиента) | — |

### 3.4 zstd-пайплайн — блокер

`rust/crates/discord-codec/src/zstd.rs` (декодер `ruzstd`) держит окно 64 КиБ и отдаёт не более
`распаковано − окно`, пока кадр не завершён. Для живого сокета `run2:95974.1389` это 759 280 из
824 816 байт: пайп видит 44 полных терма, тогда как в потоке 113 диспатчей (`ws.rs:51-57`).
Следствие: **READY (s=1) разбирается, но 60 % последующих диспатчей не доходят до состояния**,
и это не ошибка входа — это тихая потеря данных. Признано в комментарии, но не исправлено.

Приоритет P1: без этого шлюз не годится для живого аккаунта, а любые замеры UI на живых данных
недостоверны.

### 3.5 IDENTIFY (op2)

| # | Что | Наш код | RE | Вердикт | P |
|---|---|---|---|---|---|
| — | Набор полей `d` | `token, capabilities, properties, presence, compress, client_state, qos_token` (`identify.rs:310-341`) | `rest-static.md:56`, `:84`; бандл `_doIdentify` | ✔ | — |
| — | `capabilities` | 1 734 653; 1 767 421 с обфускацией (`identify.rs:18-21`) | `rest-static.md:84` | ✔ | — |
| — | `properties` | SP + `client_app_state`, `is_fast_connect`, `gateway_connect_reasons`, `installation_id?` (`identify.rs:229-276`) | бандл `handleIdentify`: `{...getSuperProperties(), client_app_state, is_fast_connect:false, gateway_connect_reasons, installation_id?}` | ✔ | — |
| — | `client_state` | `{guild_versions:{}}` (`identify.rs:316-327`) | `rest-static.md:84` (fast-connect), живой холодный старт — пусто | ✔ | — |
| — | `presence` | `{status,since,activities,afk}` (`identify.rs:279-286`) | `getInitialState()` — те же поля | ✔ | — |
| M-35 | `qos_token` | protobuf представлен как ETF-строка, не-UTF-8 → ошибка (`identify.rs:204-211`, `:314-315`) | protobuf-байты; клиентский wetf-энкодер шлёт бинарную строку (`key:"binary"`) | ✘ для произвольного токена сборка падает `QosTokenNotUtf8`, хотя бинарный ETF допустим | P2 |
| M-36 | Дефолты `for_rw_client` | `os_arch = std::env::consts::ARCH` (`"x86_64"`), `system_locale="en-US"`, `client_launch_id` нулевой UUID (`identify.rs:67-90`) | `os_arch:"x64"`, `system_locale:"ru"`, UUID v4 | ✘ если IDENTIFY собран не из SP (`supervisor.rs:155` это исправляет), уходит «не тот» клиент | P3 |

### 3.6 Диспатчи и READY

| # | Что | Наш код | RE | Вердикт | P |
|---|---|---|---|---|---|
| — | 21 тип op0 | `dispatch.rs:14-117` | `gateway-protocol.md:155-157` | ✔ полное совпадение множества | — |
| — | Неизвестный тип | raw-событие + счётчик (`dispatch.rs:138-158`, `ws.rs:284-289`) | — | ✔ ошибки не глотаются | — |
| M-37 | PAYLOAD `MESSAGE_CREATE` | 19 ключей в RE; в `state::Message` 16 полей (`state/types.rs:286-319`), из них `attachment_count`/`embed_count` — только счётчики | `gateway-schema.md:431-460` | ✘ нет `channel_type`, `member` (ник/роли/joined_at), `components`; `attachments`/`embeds`/`mentions` — без объектов; нет `nonce`, `referenced_message`, стикеров, опросов, `interaction` | P2 |
| M-38 | `USER_SETTINGS_PROTO_UPDATE` | `EventPayload::Unsupported` (`state/types.rs:576`) | 3 события в захвате (`gateway-protocol.md:157`) | ✘ настройки, изменённые с другого устройства, игнорируются | P2 |
| M-39 | `MESSAGE_REACTION_ADD`, `MESSAGE_POLL_VOTE_ADD`, `SOUNDBOARD_SOUNDS` | `EventPayload::Unsupported` (`state/types.rs:576`) | все три есть в захвате (`gateway-protocol.md:157`) | ✘ реакции и голоса саундборда не появляются без перезагрузки истории | P2 |
| M-40 | `VOICE_SERVER_UPDATE` | `Unsupported` в состоянии (`state/types.rs:576`), типизирован только в голосовом крейте (`voice/lib.rs:623-692`) | 3 события в захвате (`gateway-protocol.md:157`) | ✘ голосовой сервер из шлюза не доходит до состояния: вход в канал требует отдельного разбора | P2 |
| M-41 | Поля READY | типизированы 20 из 40 (`ready/snapshot.rs:20-63`, `:70-110`) | 40 ключей (`gateway-schema.md:127-166`) | ✘ `experiments`, `guild_experiments`, `apex_experiments`, `consents`, `auth`, `notification_settings`, `regional_feature_config`, `static_client_session_id`, `sessions[].client_info` остаются в `raw` | P2 |
| — | READY `user` 32 ключа | `ready/social.rs` (User) | `gateway-schema.md:166-200` | ✔ основные; `email/phone` не парсятся — осознанно (секреты) | — |

---

## 4. Голос

### 4.1 Сигнализация: op-коды

Наш `VoiceOp` знает 0,1,2,3,4,6,8 (`discord-voice/src/lib.rs:55-94`), а `OBSERVED_RTC_OPS`
перечисляет 0,1,2,3,4,6,8,15,16 (`lib.rs:51`). Клиент (бандл, `RTCControlSocket.onmessage`,
offset 2 396 729) обрабатывает:

| № / op | Смысл | Наш код | P |
|---|---|---|---|
| 0 | Identify | ✔ `Identify` + `to_json` (`lib.rs:160-202`) | — |
| 1 | Select Protocol | ✔ (`lib.rs:204-255`) | — |
| 2 | Ready | ✔ (`lib.rs:257-266`, `:443-454`) | — |
| 3 | Heartbeat | константа интервала (`lib.rs:42`), кадра/цикла нет | P2 |
| 4 | Session Description | ✔ (`lib.rs:268-292`, `:456-474`) | — |
| M-42 | 5 — Speaking (`{user_id,ssrc,speaking}`; флаги `NONE=0,VOICE=1,SOUNDSHARE=2,PRIORITY=4,HIDDEN=8`, бандл offset 10 768 632) | **нет** — исходящий кадр не собирается | **P1** |
| 6 | Heartbeat ACK | `VoiceOp::HeartbeatAck`, payload `d.t` не моделируется | P3 |
| M-43 | 7/9 — Resume `{token,session_id,server_id,channel_id,seq_ack}` / Resumed | **нет** | P2 |
| 8 | Hello | ✔ (`lib.rs:361-366`) | — |
| 11/12/13 | client-connect, video (ssrc/streams), client-disconnect | нет | P3 |
| 14 | Session Description (обновление кодеков) | нет отдельного пути | P3 |
| 15 | Media sink wants `{any:100}` | парсится как `Other{raw}` (`lib.rs:373-376`) | P3 |
| 16 | Backend versions `{voice,rtc_worker}` | `Other{raw}` | P3 |
| 18/20 | user flags / platform | нет | P3 |
| M-44 | 21/22/24/25/27/29/30 — DAVE/MLS: prepare/execute transition, prepare epoch, external sender package, proposals, prepare commit, welcome | **нет ни одного** | P2 |
| 33 | sfu-update | нет | P3 |

Живой дамп `voice-deep.md` §1 подтверждает только 0,1,2,3,4,6,8,15,16 (в канале никого не было,
тишина, DAVE-переходов не случилось). Остальные op-коды — из кода клиента, а не из захвата;
это надо помечать при приёмке: «подтверждено статикой, не живым трафиком».

**Практические следствия:**
- Без op5 Speaking наш клиент не может передавать голос вообще (только принимать).
- Без op7/9 любой обрыв медиа-сокета = полный re-identify и потеря сессии.
- Без DAVE (21–30) E2EE-каналы и звонки с включённым E2EE работать не будут; обычные каналы —
  будут, пока сервер не потребует DAVE.

### 4.2 Жёсткие проверки парсера

| # | Что | Наш код | RE | P |
|---|---|---|---|---|
| M-45 | `op0 Identify` требует `video:true` и `max_dave_protocol_version == 1`, иначе ошибка | `lib.rs:381-399` | валидный op0 допускает `video:false` (аудио-онли) и DAVE-версию 0 | P2 |
| M-46 | `op2 Ready` требует непустые `ip`, `modes`, `experiments` | `lib.rs:443-454` | в одном из живых Ready поля есть (`voice/rtc1-frames.jsonl`, op2), но схема допускает отсутствие `experiments` | P3 |
| M-47 | `op4` требует ровно 32 байта ключа | `lib.rs:456-460` | 32 байта, `voice-deep.md` §2.4 | ✔ |

### 4.3 UDP, SRTP, RTP

| Что | Наш код | RE | Вердикт |
|---|---|---|---|
| Discovery 74 Б, type 1/2, len 0x0046, ssrc BE, адрес ASCII, порт BE @72 | `udp.rs:8-13`, `:110-199` | `voice-deep.md` §2.1, `voice/strace-media-s3.jsonl` (первые две строки: нули, затем `…8409`) | ✔ (включая локальный порт в запросе) |
| Keepalive 8 Б `1337cafe`/`1337f00d`, LE-счётчик | `udp.rs:12-29`, `:331-353` | `voice-deep.md` §2.2 | ✔ |
| Ротация +60,08 с | `udp.rs:11`, `RotationSchedule` `:207-245` | `voice-deep.md` §2.5 (60,08 с) | ✔ |
| Empty RR 8 Б / SRTCP 28 Б | `udp.rs:431-445` | §2.4: пустой RR 28 Б сразу после ротации | M-48: `rotate_if_due` вызывает `rotate()` → уходит **8-байтовый незашифрованный** RR вместо 28-байтового SRTCP (`udp.rs:404-427`); `rotate_srtcp` есть, но в боевом пути не используется |
| SRTP: `[clear][ct][tag16][ctr4 LE]`, счётчик `0x80000000 + n` | `srtp.rs:1-30` | `voice-deep.md` §2.3–2.4 | ✔ раскладка; ключ/KDF — самосогласованные, не wire (честно указано `srtp.rs:10-14`) |
| RTP: 12 Б, X=1, `0xBEDE`, 2 слова, PT=120, 960 тиков, 20 мс | `media/rtp.rs:14-31`, `:265-290` | §2.3; `fixtures/voice/rtp-pt120-oracle.json` | ✔ |
| Jitter-буфер | `media/jitter.rs` | в RE джиттер не измерен (тела RTCP шифрованы, §5.3) | ✔ модель, оракула нет |
| RTCP SR/RR тела | `rtcp.rs` — только структура/пустые | §2.4: тела шифрованы | ✔ с оговоркой |

### 4.4 Регионы

`geo_ordered_rtc_regions` читается из READY; `regions.rs:1-3` честно пишет, что формат
latency-пробы не подтверждён. `voice-deep.md` §1 подтверждает только сам факт
`RTCLatencyTestManager` и разные `c-<region>` для сессий; формат зонда — unknown. Оставляем P3.

---

## 5. Платформа и IPC

### 5.1 Карта каналов Discord против наших возможностей

RE: 168 каналов `DISCORD_*`, у 115 есть обе стороны, 30 в whitelist `send`, 14 в whitelist
`invoke` (`backend-ipc.md:26-43`, `backend-ipc.json` → `bridges.ipcWhitelists`).

Наш продукт — Tauri, а не Electron, поэтому копировать 168 каналов не нужно. Но **функции**
надо сопоставить:

| Группа (RE) | N | Есть у нас | Чего нет | P |
|---|---:|---|---|---|
| window | 25 | частично Tauri (окно, фокус, fullscreen) | progress bar, flash frame, always-on-top, media source id, native handle, traffic lights | P3 |
| app | 21 | нет | badge count, dock bounce, relaunch, module versions, host version, open-on-start, paint metrics | P3 |
| tray/thumbar/autostart | 9 | autostart-файл (`autostart.rs`) | трей, состояния меню, thumbar, minimize-to-tray, start-minimized | P2 |
| notifications | 7 | freedesktop `Notify`/`CloseNotification`/события (`notifications.rs`) | options-схема Discord, push-кортеж `(action, identifier, userText, fallbackDeepLink)` | P2 |
| power | 8 | inhibit/uninhibit/idle/PrepareForSleep (`power.rs`) | push-события в renderer (у нас — API), «block display sleep» отдельно | P3 |
| clipboard | 8 | нет | copy/cut/paste, write image/file, mixed content | P3 |
| spellcheck | 6 | hunspell + tokenizer (`spellcheck.rs`, `ffi_hunspell.rs`) | push `SPELLCHECK_RESULT`, словари по локали | P3 |
| files | 9 | нет | диалоги, module paths, show-in-folder | P3 |
| process utils | 12 | `process_metrics` в BFF (`main.rs:73-78`) | CPU/GPU/system metrics API, crash info | P3 |
| settings | 4 | свой JSON-стор (`settings_store.rs`) | sync `SETTINGS_GET_SYNC`, background color | P3 |
| safeStorage | 3 | keyring (`keyring.rs`, `ffi_secret.rs`) | wire-совместимость с `dQw4w9WgXcQ:`-префиксом (нужна только для импорта профиля Electron) | P3 |
| WebAuthn | 5 | нет | нативный `discord_webauthn` | P3 |
| clips | 7 | нет | `disclip://`, загрузка/монтаж | P3 |
| native modules | 5 | нет (не Electron) | — | — |
| splash/build override | 6 | нет | cookie `build-override`, состояние splash | P3 |

Ключевой вывод: **мы не обязаны** повторять IPC-каналы, но обязаны закрыть 4 пользовательские
функции, которые в Discord живут именно в main: трей, уведомления с действиями, автозапуск
с «старт свёрнутым», deep link из второго инстанса.

### 5.2 Уведомления

`notifications.rs:1-12` прямо фиксирует: на Linux Discord не грузит `discord_notifications`,
поэтому freedesktop-клиент — не «перенос», а замена. Это подтверждено `main-arch.md:325-352`
(ветка загрузки только macOS/Windows). Оракул для схемы опций у нас отсутствует: ни в
`backend-ipc.json`, ни в `main-arch.md` нет полей объекта `options` (его разбирает нативный
модуль, а `.node` в дампе не дизассемблировался — `backend-ipc.md:231-243`). Значит:
- M-49: схема `Notification` (`app_name/summary/body/actions/…`) — наша, не Discord;
  при импорте профиля/настроек из Discord она не совпадёт.
- M-50: push-событие `NOTIFICATIONS_RECEIVED_RESPONSE` несёт 4 значения
  (action, identifier, userText, fallbackDeepLink) — у нас событие из двух полей
  (`NotificationEvent::Action{id,action}` / `Closed{id,reason}`), без `identifier` и
  `fallbackDeepLink`; клик по уведомлению не может открыть нужный канал.

### 5.3 Deep link

`deep_link.rs` разбирает 6 форм: `login/one-time`, `action/oauth2/authorize`, `app/open`,
`app/settings/<section>`, `/library/<id>/launch`, `[-]/channels/...` (`deep_link.rs:58-80`).

| # | Что | Наш код | RE | P |
|---|---|---|---|---|
| M-51 | Прочие `action/*` | `Route::Unknown` | `main-arch.md:850-851`: `openOrQueueUrl` → `core.handleOpenUrl`, любые `discord://` | P2 |
| M-52 | Аргументы процесса | нет | `main-arch.md:92`: `extractUrlFromArgs(argv)` — `--url <u>` после `--` либо `discord://…` | P2 |
| M-53 | Второй инстанс | нет | `main-arch.md:117`: `second-instance` → `openOrQueueUrl` + `focusSplash` | P2 |
| M-54 | Схемы каналов | только `discord` | `APP_PROTOCOL="Discord"`; PTB/Canary — свои схемы (в дампе не перечислены) | P3 |

### 5.4 Автозапуск

`autostart.rs:21-67` генерирует XDG `.desktop` с `X-GNOME-Autostart-enabled`. В Discord это
`setLoginItemSettings` + каналы `TOGGLE_OPEN_ON_STARTUP` / `UPDATE_OPEN_ON_STARTUP` /
`TOGGLE_START_MINIMIZED` (`backend-ipc.md:44`).

M-55: у нас нет контракта «стартовать свёрнутым в трей» (`--start-minimized`) и нет связи
настройки UI ↔ файл автозапуска; файл генерируется, но никем не пишется (P2, вместе с треем).

### 5.5 Обновления, crash, прочее

| # | Что | Наш код | RE | P |
|---|---|---|---|---|
| M-56 | Апдейтер | `discord-updater` (manifest/plan, 984 строки) | `main-arch.md:523-596`: новый апдейтер + splash-конечный автомат + legacy `moduleUpdater` + build override cookie | P2: нет конечного автомата splash и module-апдейтера; контракт обновления не сверен с `main-arch.md` §5 |
| M-57 | Crash reporting | нет | `main-arch.md:715+` Sentry + crashpad + blackbox | P3 |
| M-58 | NTP-синхронизация | нет | каналы `NTP_CLOCK_*` (3) | P3 |
| M-59 | `USER_SETTINGS_OPEN` push | нет | `main-arch.md:352` (единственный push без префикса) | P3 |

---

## 6. Что подтверждено, а что — нет (честные границы)

| Утверждение | Статус | Чем проверено |
|---|---|---|
| 47 REST-маршрутов D-02 присутствуют в каталоге | подтверждено | множественная сверка `rest-d02.json` ↔ `routes.rs` |
| 22 поля `X-Super-Properties` | подтверждено | живой заголовок `run2:95974.2323` |
| `X-Fingerprint`/`X-Installation-ID` отправляются клиентом | **противоречие**: статика да, живое нет | бандл `prepareRequest` vs `rest-d02.md:400-404` (U-08) |
| `Accept-Language` — q-список | подтверждено кодом клиента, не живым значением | бандл `prepareRequest`; значения в дампе не сохранены |
| RESUME-окно 3 минуты | подтверждено кодом клиента (`tI=3*MINUTE`), в захвате разрывы короче | бандл `_doResumeOrIdentify` |
| op5/7/9/DAVE в голосе | **не подтверждено живым трафиком** (канал был пуст) | бандл `RTCControlSocket.onmessage`; `voice-deep.md` §5 |
| DAVE-кадры 21–30 | не подтверждено ни живым, ни `voice-deep.md` | только бандл |
| Схема опций уведомлений | нет оракула | `backend-ipc.md:231-243` (нативный модуль не разобран) |
| Тела REST-ответов | нет оракула | `rest-d02.md` §5, U-01 |
| Формат latency-зонда RTC | нет оракула | `regions.rs:1-3`, `voice-deep.md` §1 |

### Ключевые unknowns, унаследованные из дампа

U-01 (тела REST), U-02 (`/auth/*` схемы), U-03 (`GET /gateway` не вызывается), U-05 (cookie),
U-06 (формат `Authorization`), U-08 (fingerprint), U-10 (живой файл захвата).

---

## 7. Влияние на продукт

1. **Шлюз теряет диспатчи** (3.4) — все экраны на живом аккаунте будут «замирать» после READY.
   Это первое, что надо чинить.
2. **RESUME после 30 с** (M-32) — частые полные переинициализации, мигание UI, лишние READY по
   580 КБ.
3. **Нет REST-функций P1** (друзья, создание ЛС, управление каналом) — базовые сценарии
   «написать новому человеку» и «создать канал» недоступны.
4. **Голос без Speaking** (4.1) — передача звука невозможна; без DAVE — не работают E2EE-каналы.
5. **Платформа**: трей и уведомления с переходом в канал — обязательный минимум десктоп-клиента,
   у нас есть только «показать уведомление».

---

## 8. Приоритизированный список (передаётся PM)

Полные карточки — `/home/user/orca/project/waves/RW-40/PM/BACKLOG-RE-BACKEND-PART2.md`.

**P1 (6):**
1. zstd-пайплайн шлюза без потери термов (3.4).
2. RESUME-окно 3 минуты вместо 30 с (M-32).
3. Отправка голоса: op5 Speaking + цикл медиа (4.1).
4. REST: друзья — заявка/принять/удалить (G-10).
5. REST: создание ЛС/группы (G-13).
6. REST: управление каналом (GET/PATCH/DELETE) (G-15).

**P2 (15 карточек, RE-BE-07…21):** `Accept-Language` q-список и опциональные заголовки
(fingerprint/installation/routing/context); живое обновление SP и дефолты сборки 641004/94186;
DAVE-опкоды и voice resume; SRTCP-RR при ротации + wire-ключи; heartbeat fallback/expedited;
rate-limit bucket и очередь; модель ошибок `{code,message,errors}` + `nonce`-идемпотентность;
HTTP/2, keep-alive, cookie-jar, прокси; payload MESSAGE_CREATE (member/embeds/attachments);
`USER_SETTINGS_PROTO_UPDATE`, реакции и `VOICE_SERVER_UPDATE`; experiments в READY;
трей и уведомления с action/identifier; deep link argv/second-instance; автозапуск «свёрнутым»;
апдейтер и splash-автомат.

**P3 (9 карточек, RE-BE-22…30):** вложения и refresh-urls; треды; закрепление и реакции
(список/снять все); read-states пачкой и `/users/@me/settings`; взаимодействия и команды
приложений; эмодзи/стикеры/приглашения; типизация op15/op16 и устойчивый `op2`;
`window_manager ",unknown"` и дефолты `for_rw_client`; карта остальных возможностей платформы
(clipboard, WebAuthn, clips, NTP, process utils, окно/приложение) — как решение «делаем/не делаем».

---

## 9. Артефакты сверки

| Проверка | Команда/файл |
|---|---|
| Каталог маршрутов = 47 live | множественная сверка `rest-d02.json:endpoints` ↔ `routes.rs` (LIVE−OURS=∅, OURS−LIVE=2) |
| Заголовки | `rest-d02.md:292-312`, `rest-d02.json:auth` |
| Супер-свойства | `rest-d02.md:314-372`, `rust/crates/discord-core/tests/data/rest-super-properties-2323.{json,b64}` |
| Опкоды шлюза | `gateway-protocol.md:128-162`, `gateway-protocol.json:opcodes` |
| READY/MESSAGE | `gateway-schema.md:121-200`, `:431-500` |
| Голос | `voice-deep.md:1-60`, `voice/rtc1-frames.jsonl`, `voice/strace-media-s3.jsonl` |
| IPC | `backend-ipc.md:26-43`, `backend-ipc.json:bridges` |
| Main/платформа | `main-arch.md:325-352`, `:680-713`, `:850-851` |
| Клиентский код (заголовки, RESUME, голос) | `out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` (offset 1 001 996; 5 019 755; 2 396 729) |
