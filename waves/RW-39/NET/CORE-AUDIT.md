# NET-B — аудит ядра: вход, сессия, сеть

Дата: 2026-10-23. Роль: NET-B (Core audit), PM DeepSeek. Режим: **read-only**
(код не менялся, `cargo` не запускался, сети/окна нет).
HEAD: `0239543` (RW-35 принят). Волна: N2 устава `waves/RW-39/NET/CHARTER.md`.

Источники: `rust/crates/discord-core/src/{rest/*,ws,gateway,identify,resume,dispatch,ready*}`,
`rust/crates/discord-core/tests/*`, `rust/crates/discord-store/src/lib.rs`,
`rust/crates/discord-platform/src/keyring.rs`, `rust/gui-spike/src/*`,
`docs/re/gateway-protocol.md`, `docs/re/rest-d02.md`, `waves/STATE.md`.
Логи аудита: `waves/RW-39/NET/PM/netb/` (`01-inventory`, `02-symbols`, `03-tests`,
`04-gaps`, `run.log`). Машиночитаемо: `waves/RW-39/NET/core-audit.json`
(72 записи `items`, 16 `gaps`, порядок вызовов `call_order`; JSON детальнее таблиц —
отдельно перечислены отсутствующие функции).

## 0. Вердикт

| Блок | Готовность | Комментарий |
|---|---:|---|
| 1. REST-транспорт, заголовки, super-properties, rate-limit, retry, TLS | **85 %** | всё проверено на стабах; нет сборки auth-заголовков из токена и разбора JSON-ответов |
| 2. Вход: login / logout / сессия | **10 %** | есть только примитивы keyring; ни одной auth-команды, нет `/auth/*`, нет logout-флоу |
| 3. Шлюз: connect / IDENTIFY / RESUME / heartbeat / READY | **80 %** | одно соединение работает end-to-end на стабах и реплее; нет URL шлюза, супервайзера и политики close-кодов |
| 4. Хранение (keyring + store) | **55 %** | keyring принят (RW-33), store — сообщения; сохранения сессии/пользователя нет, keyring ни к чему не подключён |
| 5. Интеграция UI → Tauri → core | **5 %** | BFF читает только фикстуры; сетевых команд, событий, статуса нет |

Критерий устава §5 («вход → keyring → `/users/@me` → READY») **не достигается**:
не хватает сквозного auth-слоя, URL шлюза и супервайзера, разбора ответов REST.
Ядро при этом — не «заготовка»: REST- и WS-механика покрыты 531 тестом `discord-core`
(`waves/STATE.md`), пробелы лежат на стыках (композиция) и в прикладном слое.

**Три главных пробела**

1. **Auth-слой не собран** (P0-1, P0-2): `ApiAuth` не строится из токена (дефолт — пустые
   заголовки), Tauri-команд `auth_status/login_token/logout` нет, `Keyring` не подключён
   к приложению. Вход из UI технически невозможен.
2. **Шлюз без адреса и супервайзера** (P0-3): в коде нет URL
   `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream`; `connect_and_run`
   обслуживает одно соединение, а `plan_reconnect` никем не вызывается — после первого
   обрыва приложение остаётся без связи.
3. **REST-ответы не разбираются** (P0-4): `discord-core` не имеет production-JSON-парсера
   (`discord-json` — dev-dependency), поэтому `GET /users/@me` даёт байты, а не пользователя:
   нельзя ни проверить токен, ни показать сессию, ни отличить 401 от 429 в UI.

## 1. Карта: функция/тип | файл | что делает | тесты | статус

Статусы: **готово** — реализовано и покрыто контрактными тестами; **частично** — работает,
но контракт уже́же нужного приложению; **нет** — отсутствует. В `core-audit.json` те же
значения кодируются как `ready` / `partial` / `missing`.

Числа тестов: в `discord-core` **531** объявление (528 в `tests/` + 3 inline), что совпадает
с `waves/STATE.md`; в 20 профильных файлах аудита — **255**, плюс keyring 12 и store 14.
Прогоны не выполнялись (запрет ТЗ), числа — по объявлениям `#[test]`/`#[tokio::test]`.

### 1.1 REST: транспорт и кадры

| Функция/тип | Файл | Что делает | Тесты | Статус |
|---|---|---|---|---|
| `Endpoint::{parse,target,host_header}` | `rest/transport.rs` | разбор базы (`https://discord.com/api/v9`), склейка target, `Host` с портом/IPv6 | `rest_transport.rs` (3 кейса) | готово |
| `send`, `send_with` | `rest/transport.rs` | TCP/DNS → TLS → один HTTP/1.1 запрос, `Connection: close` | `rest_transport.rs` (10; TLS-кейсы отдельно) | готово |
| `HttpClientConfig` | `rest/transport.rs` | корни webpki, connect/read/write-таймауты, лимит тела 8 МиБ | `rest_transport.rs` | готово |
| `HttpRequest`/`HttpResponse` | `rest/transport.rs` | запрос/ответ + builder `header`/`body` | `rest_transport.rs::response_shape_is_public` | частично: `Debug` печатает заголовки (утечка `Authorization` при логировании) |
| `HttpError` | `rest/transport.rs` | 10 типов: base/target/scheme/connect/tls/io/protocol/too_large/timeout | `rest_transport.rs` | готово |
| `http1::{encode_request,parse_response_head,decode_chunked,...}` | `rest/http1.rs` | чистый HTTP/1.1: инъекции, framing, chunked, лимиты | `rest_http1.rs` (16) | готово |
| `TLS` | `rest/transport.rs` | rustls + ring + webpki-roots, проверка сертификата и SAN | `rest_transport.rs` (TLS-стаб, 2) | готово |

### 1.2 REST: auth-заголовки, identity, super-properties

| Функция/тип | Файл | Что делает | Тесты | Статус |
|---|---|---|---|---|
| `headers::AUTH_HEADER_NAMES` | `rest/headers.rs` | 6 имён в порядке живого захвата | `rest_headers.rs` (5) | готово |
| `headers::auth_headers(&AuthHeaderValues)` | `rest/headers.rs` | пары имя→значение, пустые не подменяет | `rest_headers.rs` | готово (в проде не используется — вызывающий собирает `ApiAuth`) |
| `headers::classify_status` | `rest/headers.rs` | 200/204/304/404/429 + `Other(u16)` | `rest_headers.rs` | готово |
| `ApiAuth { 6 полей }` | `rest/client.rs` | owned-значения заголовков, `Debug` скрывает `Authorization` | `rest_client.rs::debug_redacts_authorization` | готово как тип, **нет конструктора из токена** |
| `ApiAuth::{from_values,pairs}` | `rest/client.rs` | копирование из ссылок / пары для отправки | `rest_client.rs` (через `client_for`) | готово |
| `identity::IdentitySources`, `SystemSources` | `rest/identity.rs` | честные источники: env, `/proc`, `/etc/os-release`, `/dev/urandom` | `rest_identity.rs` (8) | готово |
| `identity::IdentityConfig` | `rest/identity.rs` | 22 override-поля идентичности | `rest_identity.rs::config_overrides_win` | готово |
| `identity::build` | `rest/identity.rs` | собирает `SuperProperties`; 3 UUID v4 в каноническом порядке | `rest_identity.rs` + live-заголовок | готово |
| `identity::uuid_v4` | `rest/identity.rs` | RFC 4122 v4 из `random_bytes` | `rest_identity.rs::uuid_v4_shape` | готово |
| `SuperProperties` (22 поля) | `rest/super_properties.rs` | канонический JSON/base64, строгий парсер | `rest_super_properties.rs` (8, live b64) | готово |
| `SuperProperties::{to_json,from_json,to_base64,from_base64}` | `rest/super_properties.rs` | сериализация/разбор с позициями ошибок | `rest_super_properties.rs` | готово |
| `base64::{encode,decode}` | `rest/base64.rs` | base64 без внешних крейтов | `rest_base64.rs` | готово |

### 1.3 REST: клиент, лимиты, повторы

| Функция/тип | Файл | Что делает | Тесты | Статус |
|---|---|---|---|---|
| `ApiClient::new` | `rest/client.rs` | валидирует базу, хранит auth/http/retry | `rest_client.rs::invalid_base_rejected` | готово |
| `ApiClient::execute` | `rest/client.rs` | guard телеметрии → 6 заголовков → таймаут → 429/5xx повторы | `rest_client.rs` (14) | готово |
| `ApiClient::get` | `rest/client.rs` | GET без тела | `rest_client.rs` (`/users/@me` на стабе) | готово |
| `ApiClient::sleep_backoff` | `rest/client.rs` | экспонента 5xx с потолком | `rest_client.rs` | готово |
| `ApiConfig`/`ApiRetryPolicy` (default) | `rest/client.rs` | база v9, 3 попытки, backoff 250 мс, таймаут 30 с | `rest_client.rs` | **частично**: `ApiAuth::default()` = 6 пустых строк → запрос уходит без валидного `Authorization` |
| `ApiError` + `Display` | `rest/client.rs` | `Unauthorized`/`RateLimited`/`Server`/`Client`/`Timeout`/`Transport` | косвенно (matches в тестах) | готово как тип; **нет** прикладной политики на 401 |
| `rate_limit::rate_limit_info` | `rest/rate_limit.rs` | 429: `retry-after` → тело → `x-ratelimit-reset-after`, global, bucket | `rest_rate_limit.rs` (10) | готово |
| `rate_limit::classify_failure` | `rest/rate_limit.rs` | 429/401/5xx/4xx → `RestFailure` | `rest_rate_limit.rs` | готово |
| `rate_limit::{should_retry,retry_delay}` | `rest/rate_limit.rs` | 429 для всех методов, 5xx только идемпотентные | `rest_rate_limit.rs` | готово |
| `retry::{is_idempotent,attempt_once,execute_with_retries}` | `rest/retry.rs` | политика над замыканием (отдельный слой, фасадом не используется) | `rest_retry.rs` (11) | готово, но дублирует логику `ApiClient` |
| `telemetry::{guard_target,is_telemetry_path}` | `rest/telemetry.rs` | блокирует `/science`,`/beaker`,`/track` до сети | `rest_telemetry.rs` (4) | готово |
| `routes::ROUTES` (47) | `rest/routes.rs` | каталог живого захвата D-02 | `rest_routes.rs` (8) | готово; **`/auth/*` и `/gateway` в каталоге отсутствуют** |
| `routes::{find,render_path}` | `rest/routes.rs` | поиск по точной паре, подстановка `{id}`/`{num}` | `rest_routes.rs` | готово |
| `resources::{path,resource}` | `rest/resources.rs` | 8 read-only GET-ресурсов | `rest_resources.rs` | готово |
| `ack::*` | `rest/ack.rs` | тело `MESSAGE_ACK` | `rest_ack.rs` (9) | готово |

### 1.4 Шлюз: ядро сессии и транспорт

| Функция/тип | Файл | Что делает | Тесты | Статус |
|---|---|---|---|---|
| `gateway::Session::{new,on_term,on_value,on_timer}` | `gateway.rs` | HELLO, heartbeat 41 250 мс, ACK/RTT, zombie, `s`, op7/op9 | `gateway_session.rs` (15, реплей таймлайна) | готово |
| `gateway::Effect` | `gateway.rs` | `SendHeartbeat{d}` / `Reconnect{reason}` | `gateway_session.rs` | готово |
| `gateway::{Op,Hello,ReconnectReason,GatewayError}` | `gateway.rs` | опкоды и ошибки декодирования | `gateway_session.rs` | готово |
| `ws::GatewayPipe::push_binary` | `ws.rs` | zstd-поток → ETF-термы → сессия, счётчики, десинк = ошибка | `ws_transport.rs` (13) | готово |
| `ws::GatewayPipe::{last_ready,resume_state,last_invalid_session_resumable}` | `ws.rs` | READY-сводка, `session_id`+`last_seq`, `op9 d` | `identify_ready.rs`, `resume_reconnect.rs` | готово |
| `ws::GatewayPipe::{events,take_events}` | `ws.rs` | буфер диспатчей (дренаж обязателен) | `dispatch_router.rs` | готово |
| `ws::encode_heartbeat` | `ws.rs` | `{op:1,d}` с `0x83` | `ws_transport.rs::heartbeat_encode_roundtrip` | готово |
| `ws::connect_and_run` / `_with` / `_resume` | `ws.rs` | одно соединение: HELLO → IDENTIFY/RESUME ровно раз → цикл таймеров/сообщений | `ws_transport.rs`, `identify_ready.rs`, `resume_reconnect.rs` | готово, но **single-shot**: исход отдаётся вызывающему |
| `ws::RunOutcome` | `ws.rs` | `Reconnect{reason}` / `Closed{code}` | там же | частично: код close не классифицируется (4004/4013/4014/4008) |
| `ws::RunOptions` | `ws.rs` | `identify: Option<IdentifyParams>` | `identify_ready.rs` | готово |
| `ws::tls_connector` | `ws.rs` | ring + webpki-roots для `wss://` | только сборка ветки (live запрещён) | готово |
| `identify::{build_identify,build_identify_frame}` | `identify.rs` | op2: token, capabilities, properties, presence, compress, client_state, qos_token | `identify_ready.rs` (16; frame — косвенно через WS-стаб) | готово; **пустой token не отвергается** |
| `identify::IdentifyProperties::for_rw_client` | `identify.rs` | дефолты нашего клиента (`Discord Client`, stable, 641004) | `identify_ready.rs` | готово; **не связан с `SuperProperties`** |
| `identify::qos_token_client_provided` | `identify.rs` | protobuf `QosToken` (4 Б) | `identify_ready.rs` | готово |
| `resume::{build_resume_frame,plan_reconnect}` | `resume.rs` | op6 `{token,session_id,seq}`; политика 30 с/op9 | `resume_reconnect.rs` (13) | готово |
| `resume::ResumeState`/`ResumeParams` | `resume.rs` | состояние и параметры (token в `Debug` скрыт) | `resume_reconnect.rs` | готово |

### 1.5 READY, диспатчи и состояние

| Функция/тип | Файл | Что делает | Тесты | Статус |
|---|---|---|---|---|
| `ready::ReadySummary::from_dispatch` | `ready.rs` | счётчики READY + `user_id` + `session_id` + `resume_gateway_url` | `identify_ready.rs`, live `.1389` | готово (без имени/аватара пользователя) |
| `ready::Ready::from_dispatch` | `ready/snapshot.rs` | типизированная модель READY из ETF | `ready_core.rs` (14) | готово |
| `ready::{guild,social,read_state}` | `ready/*.rs` | гильдии/каналы/роли/участники/read-state | `ready_*` (48, 5 файлов) | готово |
| `dispatch::{DispatchKind,route_dispatch}` | `dispatch.rs` | 21 тип op0, raw со счётчиком | `dispatch_router.rs` (12) | готово |
| `state::{apply_dispatch,apply_frame}` | `state/reducer.rs` | чистое состояние из диспатчей | `state_*` | готово |
| `history::History` | `history/*` | окно истории канала, пагинация | `history_*` | готово (без REST-JSON) |

### 1.6 Хранение и платформа

| Функция/тип | Файл | Что делает | Тесты | Статус |
|---|---|---|---|---|
| `keyring::Keyring::{connect,store,lookup,clear}` | `discord-platform/src/keyring.rs` | Secret Service через libsecret, схема `com.discordlab.Token`, атрибут `key` | `keyring_contract.rs` (12; live по флагу) | готово (RW-33) |
| `keyring::KeyringError` + `Debug` | там же | 4 ошибки без секрета; `Debug` без значения | `keyring_contract.rs` | готово |
| `Keyring` — `!Send + !Sync` | там же | libsecret/GDBus не переносится между потоками | `keyring_contract.rs` | готово как ограничение; **влияет на Tauri-команды** |
| `Store::{open,migrate}` | `discord-store/src/lib.rs` | SQLite, WAL, схема v1, `user_version` | `store_contract.rs` (14) | готово |
| `Store::{put_channel,channel,put_message,messages,delete_message}` | там же | каналы/сообщения, keyset-пагинация | `store_contract.rs` | готово |
| `Store::{kv_set,kv_get}` | там же | key-value (единственный кандидат под сессию) | `store_contract.rs` | готово как примитив, **сессия не сохраняется** |
| схема `users`/`guilds`/`roles`/`emoji` | там же | заявлены в `CORE_TABLES`, в v1 **не созданы** | — | нет |
| `discord-app` (`discord-rs`) | `crates/discord-app/src/main.rs` | печатает 2 константы | — | нет (каркас) |

### 1.7 Прикладной слой (Tauri/BFF)

| Функция/тип | Файл | Что делает | Тесты | Статус |
|---|---|---|---|---|
| `ready/messages_page/members_page/settings/search_page` | `gui-spike/src/main.rs`, `data.rs` | BFF из **локальных фикстур**, без сети | 102 (dom/model/bench/features) | готово для фикстур |
| `core_bridge` (W39j) | `gui-spike/src/core_bridge.rs` | фикстуры → `state::Message` → `history::History` | `ipc_contract`/`data_contract` | готово как пилот, только сообщения |
| `process_metrics`, `report_*`, `spike_config` | `gui-spike/src/main.rs` | замер/маркеры | `launch.rs` | готово |
| `auth_status` / `login_token` / `logout` | — | — | — | **нет** |
| события статуса подключения в UI | — | — | — | **нет** |
| зависимость `discord-platform` в GUI | `gui-spike/Cargo.toml` | — | — | **нет** (сейчас только `discord-core`) |

## 2. Вход: что есть и чего не хватает

### 2.1 `GET /users/@me`

Есть: маршрут `rest/routes.rs:80`, `ApiClient::get`, 6 auth-заголовков, TLS, ретраи;
тест `rest_client.rs::auth_headers_added_in_order` буквально вызывает `client.get("/users/@me")`
на локальном стабе и проверяет порядок заголовков.

Нет:
1. разбора тела ответа: в `discord-core` **нет production-JSON-парсера** (`discord-json` —
   только dev-dependency, `Cargo.toml:22`), типы `ready::User` принимают только `EtfValue`;
2. типа `Me`/`CurrentUser` и маппинга `id/username/discriminator/avatar`;
3. проверки, что `200` действительно наш пользователь (сверка с ожидаемым id);
4. обёртки «токен → identity» (см. 2.2).

### 2.2 `POST /auth/*`

Нет ничего: в каталоге 47 живых маршрутов нет ни `/auth/login`, ни `/auth/logout`,
ни `/auth/mfa/*`, ни `/auth/register`, ни `/gateway`. Это согласуется с уставом §2.2
(пароль/captcha не автоматизируем) и с тем, что в захвате D-02 этих вызовов не было.
Для основного пути «импорт токена владельца» они не нужны; нужны только если ADR-2
выберет парольный/OAuth-логин — тогда это новая волна RE (NET-A) + новые маршруты.

Препятствия для будущих `POST`: тела собираются вручную (`HttpRequest::body(Vec<u8>)`),
JSON-энкодера в проде нет; cookie-jar нет; MFA/captcha-флоу нет; `Content-Type: application/json`
проставляется автоматически (`client.rs:254`) — это плюс.

### 2.3 Заголовки

Есть: 6 имён и значения, инъекция в каждый запрос без дублей (`client.rs:249`), `Debug`
скрывает `Authorization`, тесты на порядок/дубли/`Content-Type`/`Content-Length`.
Критично: `ApiConfig::default()` → `ApiAuth::default()` → **шесть пустых строк**. То есть
«клиент по умолчанию» уходит в сеть с `Authorization: ` и пустым `X-Super-Properties`.
Конструктора `ApiAuth` из токена и `SuperProperties` в коде нет (логи подтверждают:
`ApiAuth::` встречается только в `rest/client.rs`).

Чего нет: значений `X-Discord-Locale`, `X-Discord-Timezone` (нет провайдера таймзоны/локали
для заголовка), `Accept-Language`, `X-Debug-Options`; политики «user-token без префикса» vs
`Bot <token>`; валидации токена (пустой/пробелы/префикс); запрета на логирование
`HttpRequest` (derive `Debug` печатает заголовки, `transport.rs:55`).

### 2.4 Super-properties

Есть полный честный путь: `identity::build` (ОС/ядро/дистрибутив/дисплей/локаль/UUID v4) →
`SuperProperties::to_json/to_base64`; байт-в-байт сверено с живым заголовком
(`rest-super-properties-2323.b64`), 8 тестов.

Нет: связи с `IdentifyProperties`. Это два независимых набора: 22 поля для REST-заголовка и
17 полей для IDENTIFY, с разными дефолтами (`client_launch_id` = UUID из `build` против
`00000000-0000-4000-8000-000000000000` в `IdentifyProperties::for_rw_client`,
`browser_user_agent` = `discord-lab/<ver>` против `discord-lab-rs/<ver>`).
Сервер увидит два разных «запуска клиента» — это несогласованная идентичность, а не обход.

### 2.5 Rate-limit

Есть: разбор 429 (`retry-after` из заголовка → тела → `x-ratelimit-reset-after`), `global`,
`remaining`, `bucket`, потолок ожидания 30 с, повторы 429 для всех методов, 5xx — только
идемпотентные, 401/4xx — без повторов, attempt-таймаут и повтор идемпотентных.

Нет: состояния bucket'ов между запросами (каждый запрос независим), проактивного ожидания по
`x-ratelimit-remaining/reset-after` (только реакция на 429), глобального «замка» на время
global-лимита, учёта 429 на уровне приложения (кто и когда снимает блокировку UI).

### 2.6 Retry

Есть две реализации: фасад `ApiClient` (используется) и `rest/retry.rs`
(`execute_with_retries`, только тесты/публичный API). Дублирование политики — риск
расхождения; в проде нужна одна.

### 2.7 TLS

Есть: только `https`/`wss` для реальных хостов, rustls + ring, webpki-roots, проверка
сертификата и SAN (включая IP-SAN в тестах), таймауты connect/read/write, лимит тела.
Нет (осознанно по уставу): прокси/ротации, отключения проверки сертификата, HTTP/2.
Ограничение производительности: `Connection: close`, один запрос на соединение.

### 2.8 Чего не хватает для login/logout/сессии (сводно)

1. **Композиция auth**: `ApiAuth::for_user_token(token, &SuperProperties, locale, timezone)`.
2. **Проверка токена**: `GET /users/@me` + разбор JSON (+ тип `Me`).
3. **BFF-команды**: `auth_status`, `login_token` (sync/worker-поток из-за `!Send`),
   `logout` (keyring.clear + stop gateway + reset state).
4. **Состояние сессии**: где хранить `user_id`, `session_id`, `resume_gateway_url`, `last_seq`.
5. **Восстановление**: при старте `keyring.lookup` → `GET /users/@me` → gateway; ошибки
   (401 → «токен истёк», сеть → «нет связи», 429 → «подождите N с»).
6. **Logout**: локальный (keyring + state + store) и, опционально, серверный
   (`POST /auth/logout` — отсутствует, нужен только по ADR-2).
7. **Красная линия**: токен не в `Debug`, не в ошибках, не в store, не в логах; сейчас
   `ApiAuth`/`IdentifyParams`/`ResumeParams` это соблюдают, `HttpRequest` — нет.

## 3. Шлюз: что готово и что нужно для READY

### 3.1 Готово

- `connect_and_run*` — реальный WS (`ws://`/`wss://`), бинарные сообщения, zstd-поток,
  ETF-термы, разрезанные между сообщениями; десинк и мусор — ошибка, не молчание.
- IDENTIFY (op2) — ровно один раз после первого HELLO; RESUME (op6) — приоритетнее,
  IDENTIFY на resume-соединении не отправляется.
- Heartbeat — из `Session` (интервал HELLO 41 250 мс, jitter от сида, `d` = последний `s`),
  ACK и RTT, зомби-детектор, op7/op9 → `Reconnect`.
- READY — `pipe.last_ready()` (счётчики, `user_id`, `session_id`, `resume_gateway_url`);
  `pipe.resume_state()`; события — `take_events()`; `dispatch` — 21 тип.
- Всё проверено на локальных WS-стабах и реплее живого потока (`95974.1389`), 531 тест.

### 3.2 Чего не хватает, чтобы получить READY на тестовом аккаунте

| # | Что | Почему блокирует | Куда |
|---|---|---|---|
| G1 | **URL шлюза** | в коде нет ни константы, ни билдера; нужен `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream` (`rest-d02.md:384`, `gateway-protocol.md:101`); `resume_gateway_url` из READY приходит без query и требует нормализации | `discord-core` (новый модуль/константа) |
| G2 | **Токен и его свойства** | `IdentifyParams.token` не валидируется; `IdentifyProperties` расходится с `SuperProperties` | `identify` + `identity` |
| G3 | **Супервайзер** | `connect_and_run` — одно соединение; после `Reconnect`/`Closed`/`Err` приложение должно само создать новый `GatewayPipe`+`Session`, решить `plan_reconnect`, добавить backoff/jitter и лимит попыток | новый слой (app/`discord-core::session`?) |
| G4 | **Политика close-кодов** | `Closed{code}` без классификации: 4004 (auth failed) нельзя ретраить, 4013 (invalid intents), 4014 (disallowed intents), 4008 (rate limited), 1000/1001 — норма | `ws`/app |
| G5 | **Прикладной статус** | нет `AuthState`/`ConnectionState` (`Disconnected/Connecting/Identifying/Ready/Reconnecting/Failed`) для UI | BFF |
| G6 | **Сессия между запусками** | `session_id`/`seq`/`resume_url` нигде не сохраняются → после рестарта всегда IDENTIFY (допустимо, но RESUME не работает) | store `kv` |
| G7 | **Окно live** | по уставу §2.5 — только согласованное окно, тестовый аккаунт, запись в `PM/window-runs.jsonl` | PM |

Порядок достижения READY (минимум для N6): G1 → G2 → (token из keyring) →
`connect_and_run_with` → READY → G3/G4 для устойчивости.

## 4. Хранение: токен и сессия

### 4.1 Keyring (RW-33) — готов, но не подключён

- Схема `com.discordlab.Token`, атрибут `key`, метка `discord-lab token`, default-коллекция.
- `store/lookup/clear`; пустой ключ и NUL → ошибка; `lookup` отсутствующего → `Ok(None)`;
  `clear` идемпотентен; `Debug` без секрета; 12 контрактных тестов (live — по флагу).
- **`!Send + !Sync`**: нельзя держать в async-задаче и нельзя передать в `tokio::spawn`.
  Следствие для NET-C: либо sync-команда Tauri (блокирует UI-поток на время prompt'а
  разблокировки коллекции), либо выделенный поток-владелец keyring с каналом команд.
- Вне крейта `discord-platform` keyring **не используется никем** (проверено `04-gaps.log`).

### 4.2 `discord-store` — готов для сообщений, не для сессии

- SQLite v1: `channels`, `messages`, `kv` (+индексы), WAL, FK, keyset-пагинация, soft delete.
- `CORE_TABLES` заявляет `guilds/roles/users/emoji`, но схема v1 их **не создаёт**.
- `kv` — единственное место под сессию; сейчас не используется.
- Токен в store класть нельзя (устав §2.3): только keyring.

### 4.3 Рекомендуемая раскладка (для NET-C, без правок сейчас)

| Данные | Где | Ключ | Примечание |
|---|---|---|---|
| user token | keyring | `token` (или `user_token`) | никогда в store/логах; `clear` при logout |
| `user_id`, `username` | store `kv` | `me.id`, `me.username` | для офлайн-старта экрана входа |
| `session_id`, `last_seq`, `resume_url` | store `kv` | `gw.session_id`, `gw.seq`, `gw.resume_url` | TTL ≤ 30 с простоя иначе IDENTIFY (`RESUME_MAX_DOWNTIME_MS`) |
| настройки/window state | store `kv` | `ui.*` | — |
| сообщения/каналы | store таблицы | — | уже есть |

Чего нет: TTL/инвалидации сессии, миграции схемы под `users`, шифрования (не требуется —
секретов в store нет), единого «session store» API.

## 5. Gaps P0/P1

### P0 — без этого критерий §5 не выполняется

| ID | Gap | Доказательство | Что сделать |
|---|---|---|---|
| **P0-1** | Нет auth-слоя: `ApiAuth` не собирается из токена; дефолт — пустые заголовки | `rest/client.rs:116-125`, `04-gaps.log` | конструктор `ApiAuth` из `(token, SuperProperties, locale, timezone)` + тест «заголовки непустые и в порядке» |
| **P0-2** | Нет BFF-команд входа/выхода/статуса; keyring не подключён к приложению | `rg` по `gui-spike`/`discord-app` — пусто | `auth_status/login_token/logout` + поток-владелец keyring (`!Send`) + типы ошибок |
| **P0-3** | Нет URL шлюза и супервайзера; соединение одноразовое | `ws.rs:383-410`, нет `gateway.discord.gg` в коде | константа+билдер URL, supervisor: `plan_reconnect` + backoff + лимит + классификация close-кодов |
| **P0-4** | Нет разбора JSON-ответов REST (`/users/@me` нельзя превратить в пользователя) | `discord-core/Cargo.toml:22` (dev-dep), типы READY только ETF | промоут `discord-json` в prod-dep либо минимальный парсер в core + тип `Me` + тест на живом сэмпле |
| **P0-5** | Нет logout-флоу и восстановления сессии | нет кода; store `kv` не используется | `logout` = keyring.clear + stop + reset + wipe session keys; старт = lookup → `/users/@me` → gateway |
| **P0-6** | Ошибки не доходят до пользователя в пригодном виде | `ApiError`/`Closed{code}` есть, маппинга нет | `AuthError`/`ConnectionError` (401/429/сеть/4004/4014) → строки UI без утечек |

### P1 — нужно для «полноценного», но не блокирует первый READY

| ID | Gap | Что сделать |
|---|---|---|
| P1-1 | `IdentifyProperties` и `SuperProperties` расходятся (launch_id/UA/версия) | один источник: `IdentifyProperties::from_super_properties` |
| P1-2 | `HttpRequest::Debug` печатает `Authorization` | ручной `Debug` с `<redacted>` + тест; правило «не логировать запросы» |
| P1-3 | Нет проактивных rate-limit bucket'ов и глобального лока | хранить `bucket/remaining/reset_after` по маршруту, ждать до запроса |
| P1-4 | `build_identify` принимает пустой токен | `IdentifyError::EmptyToken` (симметрично `ResumeError`) |
| P1-5 | Нет keep-alive (1 запрос = 1 TCP+TLS) | пул соединений после замера выигрыша (осознанно отложено в RW-10) |
| P1-6 | `CORE_TABLES` (5) ≠ схема v1 (3) | либо миграция v2 c `users/guilds/roles/emoji`, либо честный комментарий |
| P1-7 | Две политики ретраев (`ApiClient` и `retry.rs`) | оставить одну, вторую пометить/удалить по процессу |
| P1-8 | Keyring-ошибки не отображены в UI (заблокированная коллекция = зависание) | таймаут/статус «разблокируйте связку ключей» |
| P1-9 | Нет тестов на «в логах нет токена» на границе приложения | мета-тест/лог-скан (в PROCESS уже есть практика) |
| P1-10 | `discord-app` — пустой каркас | собрать процесс/рантайм (или явно оставить GUI-spike единственным) |

### Риски, которые нельзя «зеленить»

- **ToS/бан**: пользовательский токен и сторонний клиент — риск блокировки; тестовый аккаунт
  отдельный, окно live согласуется (устав §2, §4).
- **`!Send` keyring** + async Tauri: наивное `spawn` не скомпилируется, а sync-вызов
  заблокирует UI на prompt'е — решение нужно принять в ADR-2/NET-C.
- **Зависимости**: `discord-core` без serde — добавление `discord-json` в prod и
  `discord-platform` в `gui-spike` меняет Cargo-манифесты (нужно разрешение владельца,
  см. AGENTS «никаких изменений package.json без разрешения»).

## 6. Рекомендуемый порядок вызовов (UI → Tauri → core)

Ниже — целевой поток. **Сейчас в коде есть только шаги 3–4 (сборка super-properties) и 7
(HTTP-запрос как таковой);** разбор ответа, заголовки из токена, BFF-команды и публикация
состояния — работа NET-C/NET-E.

### 6.1 Вход по токену

```
UI (login form)
  └─ invoke("login_token", { token })
       │  Tauri command (sync или выделенный поток — Keyring !Send)
       ├─ 1. Keyring::connect()                       discord_platform::keyring
       ├─ 2. Keyring::store("token", &token)          ← сразу в keyring, не в стейт
       ├─ 3. identity::build(&SystemSources::new(), &IdentityConfig::default())
       │        -> SuperProperties                    discord_core::rest::identity
       ├─ 4. sp.to_base64()                           X-Super-Properties
       ├─ 5. ApiAuth::for_user_token(...)             ← НЕТ, P0-1 (сейчас ApiAuth::from_values)
       ├─ 6. ApiClient::new(ApiConfig { base: API_V9_BASE, auth, .. })   rest::client
       ├─ 7. client.get("/users/@me")                 → 200 | 401 | 429 | сеть
       │        └─ parse JSON -> Me { id, username }   ← НЕТ, P0-4
       └─ 8. emit("auth_state", { state: "authenticated", user })
```

### 6.2 Подключение к шлюзу (после успешного `/users/@me`)

```
Tauri (async task; токен передаётся копией строки, не логируется)
  ├─ 1. Session::new(seed)                            discord_core::gateway
  ├─ 2. GatewayPipe::new(session)                     discord_core::ws
  ├─ 3. IdentifyProperties::from_super_properties(&sp) ← НЕТ, P1-1
  ├─ 4. IdentifyParams { token, capabilities: CAPABILITIES_ACTIVE,
  │        properties, presence: Some(online), compress: false,
  │        client_state_guild_versions: vec![],
  │        qos_token: qos_token_client_provided(false) }
  ├─ 5. gateway_url(resume_url: None)                 ← НЕТ, P0-3
  │        "wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream"
  ├─ 6. connect_and_run_with(url, &mut pipe, RunOptions { identify: Some(p) })
  │        ├─ HELLO (op10) → heartbeat 41 250 мс
  │        ├─ IDENTIFY (op2) ровно один раз
  │        └─ READY (op0 t="READY") → pipe.last_ready()
  ├─ 7. цикл: pipe.take_events() → state::apply_dispatch(state, &event)
  │        emit("discord_event", …) / emit("connection_state", "ready")
  └─ 8. RunOutcome::Reconnect{reason} | Closed{code} | Err(TransportError)
           ├─ pipe.resume_state() + plan_reconnect(state, downtime, invalid_session)
           ├─ ReconnectAction::Resume → ResumeParams + новый pipe + connect_and_run_resume
           └─ ReconnectAction::Identify → новый pipe + connect_and_run_with
              (backoff/лимит/классификация кодов — НЕТ, P0-3/G4)
```

### 6.3 Старт приложения (восстановление) и выход

```
startup:
  Keyring::connect -> lookup("token")
    ├─ None  -> emit("auth_state", { state: "unauthenticated" })  (экран входа)
    └─ Some  -> client.get("/users/@me")
                 ├─ 200 -> подключение (6.2)
                 ├─ 401 -> keyring.clear("token") + "токен истёк"
                 └─ сеть/429 -> "нет связи/подождите", кнопка «повторить»
  (кэш user_id/сессии из store.kv_get("me.id"/"gw.session_id") — НЕТ, P0-5)

logout:
  invoke("logout")
    ├─ остановить gateway-задачу (drop pipe/future)
    ├─ Keyring::clear("token")                       -> bool
    ├─ state = State::default(); store: удалить gw.* / me.*
    │     (в Store нет delete_kv — только kv_set; нужен пустой/overwrite-путь, P0-5)
    └─ emit("auth_state", { state: "unauthenticated" })
```

### 6.4 Минимальный «срез» для N6 (READY на тестовом аккаунте)

1. P0-1 (auth-заголовки) + P0-4 (разбор `/users/@me`) — без них нет проверки токена.
2. P0-3 часть 1 (URL шлюза) — без него нет соединения.
3. P0-2 (одна команда `login_token`, можно sync) — без него нет входа из UI.
4. READY фиксируется `pipe.last_ready()`; дальше — P0-3 часть 2 (супервайзер) и P0-5.

## 7. Приложение A — что проверено и чем

| Проверка | Файл лога |
|---|---|
| инвентарь файлов и строк | `PM/netb/01-inventory.log` |
| карта публичных символов (13 модулей REST + ws/gateway/identify/resume/dispatch/ready + store + keyring) | `PM/netb/02-symbols.log` |
| карта тестов (20 файлов `discord-core`, keyring, store) | `PM/netb/03-tests.log` |
| отсутствие: auth-маршрутов, URL шлюза, BFF-команд, композиции `ApiAuth`, использования keyring вне крейта, JSON в prod-зависимостях; `HttpRequest: Debug`; пустые дефолты `ApiAuth` | `PM/netb/04-gaps.log` |
| команды и границы (read-only, без cargo/сети) | `PM/netb/run.log` |

Числа тестов: `discord-core` — **531** (`528` в `tests/` + `3` inline; `waves/STATE.md` даёт
тот же итог). В аудите пересчитаны только объявления тестов по файлам, прогоны не выполнялись
(запрет ТЗ).
