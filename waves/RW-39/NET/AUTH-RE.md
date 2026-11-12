# AUTH-RE — фактический вход и сетевое подключение Discord 1.0.162

**Волна:** RW-39, отдел NET, роль NET-A (RE auth/network). Дата: 2026-10-23.
**Статус:** N1 закрыт по доступному дампу; пробелы перечислены в §7.
**Машиночитаемая версия:** `waves/RW-39/NET/auth-re.json` (endpoints/headers/superProperties/identify/gateway/token/errors/gaps/risks).
**Логи:** `waves/RW-39/NET/PM/neta/` (`probe.txt`, `grep-auth.txt`, `md5.txt`).

Цель — восстановить, что клиент делает **фактически**, а не как «должно быть по докам», и честно
отметить, чего в дампе нет. Только чтение; сеть не использовалась, Discord не запускался, REA не
требовался (все входы — JSON/JS/HTML, читались обычным чтением).

---

## 1. Методика и провенанс

| Что | Как |
|---|---|
| Основной вход | `reference-oracle/decoded/4d2a64cb5df5fc4e_0` = `web.4d4b2f75fdea5470.js` (12 410 287 Б), декодирован из HTTP-кэша клиента; это **тот же** бандл, что в `out/bundles/` |
| Минифицированный код | один файл без переводов строк → цитирую **байтовый offset** (`@NNNN`), не строку. Все файлы ниже — ASCII-only, поэтому byte offset == позиция в UTF-8 строке (проверено `Buffer.byteLength(s.slice(0,i))`) |
| MFA-UI | `decoded/8fed1c84bcaee32d_0` (chunk `290966`, 9 759 Б) |
| Страница входа | `decoded/43644898c0897078_0` (`/login`, `window.GLOBAL_ENV`), `decoded/dbd05750a1cb3484_0` (login-чанк) |
| Main process | `asar-extract/core/bundle.js` (safeStorage IPC, чтение токена из leveldb) |
| Fast-connect | `out/bundles/discord.com_assets_fast-connect.7c1098d4887bc4b0.js` |
| Живые данные | `docs/re/rest-d02.md` (6654 запроса), `docs/re/gateway-protocol.md`, `fixtures/rest/rest-static.json`, `fixtures/gw-proto/facts-frozen.json` |

**Метки:** `[D]` — прямая цитата кода/артефакта; `[M]` — подтверждено живым захватом; `[T]` — вывод
(прямого доказательства нет). Колонка «метод»: `static` (чтение JS), `live` (захват), `static+live`.

Контрольные md5 входов — `PM/neta/md5.txt` (например `web.js` = `934b1289b7ebf17bc0fd09569b23b438`,
`core/bundle.js` = `08edd888cca8426f0cbc6c3bb2edbf17`).

---

## 2. Способы входа: что реально вызывает клиент

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| A-01 | База REST: `"https:" + GLOBAL_ENV.API_ENDPOINT + "/v" + API_VERSION` → `https://discord.com/api/v9` | [D] | `web.js @10697595`; `/login` HTML `@1794` (`API_VERSION:9`, `API_ENDPOINT:"//discord.com/api"`) | static+live |
| A-02 | Вход паролем: `POST /auth/login`, тело ровно `{login, password, undelete, login_source, gift_code_sku_id}`, `retries:2`, `oldFormErrors:true` | [D] | `web.js @551777` | static |
| A-03 | При мультиаккаунтном входе клиент **явно ставит пустой** заголовок `authorization:""`, чтобы не унаследовать текущий токен | [D] | `web.js @551990` (`...u?{headers:{authorization:""}}:{}`) | static |
| A-04 | Ответ логина читается по ключам `mfa, sms, webauthn, ticket, token, backup, user_id, required_actions, totp, login_instance_id` | [D] | `web.js @552070` | static |
| A-05 | Если в ответе есть `mfa` → шаг MFA: `POST /auth/mfa/{mfaType}` с `{code, ticket, login_source, gift_code_sku_id, login_instance_id}`, `retries:2` | [D] | `web.js @553107` | static |
| A-06 | `mfaType` — строка из UI, реально используются: `totp`, `sms`, `backup`, `webauthn`, `password` | [D] | MFA-chunk `@6170` (totp), `@7040` (webauthn), `@5131` (sms); слайды `@8054` | static |
| A-07 | SMS: при открытии слайда `POST /auth/mfa/sms/send` тело `{ticket}`, ответ `{phone}` (номер показывается маскированно) | [D] | MFA-chunk `@4758` | static |
| A-08 | WebAuthn-MFA: челлендж приходит в `mfa.methods[]` (`type:"webauthn"`), подпись делается нативно (`webAuthnAuthenticate`), результат уходит как `code` в `/auth/mfa/webauthn` | [D] | MFA-chunk `@7040`; `main-arch.md:494` (`discord_webauthn`) | static |
| A-09 | Passwordless/conditional UI: `POST /auth/passwordless/start` (или `/auth/conditional/start`) → `{challenge,ticket}` → `POST /auth/conditional/finish` `{credential,ticket,source,giftCodeSKUId}` → `{token,user_id,required_actions}` | [D] | `web.js @553824, @554399` | static |
| A-10 | Одноразовый вход по тикету: `POST /auth/one-time-login` `{ticket}` → `{token}` | [D] | `web.js @555416` | static |
| A-11 | Подтверждение e-mail: `POST /auth/verify` `{token}` → `{token,user_id}`; повтор — `POST /auth/verify/resend` (без тела) | [D] | `web.js @557023, @557532` | static |
| A-12 | Подтверждение по IP: `POST /auth/authorize-ip` `{token}` (ветка `LOGIN_PHONE_IP_AUTHORIZATION_REQUIRED`, код 70007) | [D] | `web.js @557413` | static |
| A-13 | Сброс пароля: `POST /auth/forgot {login}` → `{method}`; `POST /auth/reset {token,password,source}`, при MFA добавляются `{code,ticket,method}` | [D] | `web.js @558834, @558042, @558505` | static |
| A-14 | Logout: `POST /auth/logout` тело `{provider, token, voip_provider, voip_token}`. **Поле `token` здесь — push-токен устройства из localStorage, а не auth-токен**; auth-токен идёт только заголовком (и только при logout другого аккаунта) | [D] | `web.js @556041` | static |
| A-15 | Проба валидности токена: `GET /users/@me`; на 401 клиент разлогинивает (`y()` → LOGOUT + редирект) | [D]+[M] | `web.js @556871`; `rest-d02.md §2 #14` (200×3) | static+live |
| A-16 | Метаданные входа: `GET /auth/location-metadata` → `{consent_required, country_code, promotional_email_opt_in}`; если ответа нет за 5 с — консент считается обязательным | [D] | `web.js @559576` | static |
| A-17 | Сессии объявлены: `GET /auth/sessions`, `POST /auth/sessions/logout` (по статике тело `{session_id_hashes}`), `/auth/sessions/debug/notifications` | [D] | `web.js @303358`; `fixtures/rest/rest-static.json → auth.session` | static |
| A-18 | QR/remote-auth: объявлены `POST /users/@me/remote-auth`, `/remote-auth/login`, `/remote-auth/finish`, `/remote-auth/cancel`; route `/ra/:remoteAuthFingerprint`; логин-сорсы `"QR Code Login"`, `"Mobile Web Handoff"` | [D] | `web.js @275654-275802`, `@11800708`, `@377364` | static |
| A-19 | QR-транспорт: `GLOBAL_ENV.REMOTE_AUTH_ENDPOINT = wss://remote-auth-gateway.discord.gg`; сам протокол в дампе отсутствует | [D]/[T] | `/login` HTML `@2328` | static |
| A-20 | OAuth2 в клиенте — только для RPC-приложений: `GET /oauth2/authorize?client_id&response_type=token&scope=identify`; пользовательского OAuth-входа нет | [D] | `web.js @605580` | static |
| A-21 | Регистрация в desktop есть (`POST /auth/register`); поля — по статике: `fingerprint, email, username, global_name, password, invite, consent, phone_token, date_of_birth, gift_code_sku_id, guild_template_code, promotional_email_opt_in` | [T] | `rest-static.json → auth.loginFlow`; `web.js @275875` | static |
| A-22 | `login_source` — свободная строка для аналитики (в коде встречается `"account_revert"`); на поведение сервера не влияет | [T] | `web.js @551777`, login-чанк `@5861` | static |
| A-23 | Никакого «входа по bot-токену» в desktop-клиенте нет: `Authorization: Bot …` в бандле не встречается | [D] | grep `"Bot "` = 0 | static |
| A-24 | Список ключей WebAuthn: `GET /users/@me/mfa/webauthn/credentials` (настройки безопасности, не вход); живьём вызывался 3× с 200 | [D]+[M] | `web.js @275980`; `rest-d02.md §2 #23` | static+live |

### 2.1 Порядок шагов (как он собран в коде)

```
POST /auth/login
  ├─ token есть, mfa нет  → LOGIN_SUCCESS(token)
  ├─ mfa есть             → LOGIN_MFA_STEP(ticket, sms, webauthn, totp, backup, login_instance_id)
  │     └─ POST /auth/mfa/{totp|sms|backup|webauthn|password} → LOGIN_SUCCESS(token)
  ├─ error.body.suspended_user_token → экран ACCOUNT_STANDING (не вход)
  ├─ code 70007 → PHONE_IP_AUTHORIZATION → POST /auth/authorize-ip {token}
  └─ code 20011/20013 → экраны «удаление запланировано»/«аккаунт отключён»
```

`login_instance_id` — сквозной идентификатор попытки: приходит в ответе логина и **обязателен** в
запросе MFA, если не задан локально (`o ?? I.default.getLoginInstanceId()`), `web.js @553328`.

---

## 3. Заголовки и identity

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| H-01 | `Authorization` = **сырой токен**, без `Bearer`. Ставится в `prepareRequest`, если заголовка ещё нет (проверяются оба написания: `Authorization` и `authorization`) | [D] | `web.js @1002375` | static |
| H-02 | `Bearer` используется только для OAuth2-токенов приложений (`/oauth2/@me`, сторонние API) — не для пользовательской сессии | [D] | `web.js @2023976`, `@7748699` | static |
| H-03 | `X-Super-Properties` = `base64(JSON.stringify(props))`, не URL-encode | [D]+[M] | `web.js @1002375, @9991807`; `rest-d02.md §6.2` | static+live |
| H-04 | Состав super-properties (desktop, Linux): `os, browser:"Discord Client", release_channel, client_version, os_version, os_arch, app_arch, system_locale, has_client_mods, client_launch_id, browser_user_agent, browser_version, window_manager, distro, runtime_environment, display_server, client_build_number, native_build_number, client_event_source, launch_signature, client_heartbeat_session_id, client_app_state` (+`install_source` опционально) | [D]+[M] | `web.js @9996765`; живой пример 22 ключей — `rest-d02.md §6.2` | static+live |
| H-05 | Часть полей кэшируется в localStorage: `deviceProperties`+`devicePropertiesVersion=1` (os/browser/device/device_model_name/system_locale/has_client_mods) и `referralProperties` (referrer/utm_*/search_engine) | [D] | `web.js @9996374-10000825` | static |
| H-06 | `X-Fingerprint` ставится **только если** в localStorage есть непустой `"fingerprint"`; значение выдаёт сервер в ответе `GET /experiments` (`{fingerprint, installation, assignments, guild_experiments}`) | [D] | `web.js @1002447`, `@8974914` | static |
| H-07 | При успешном логине и при logout клиент **удаляет** `fingerprint` из storage (и при следующем `/experiments` получает новый) | [D] | `web.js @8975704` (`et()`), `@8978963` (LOGIN_SUCCESS) | static |
| H-08 | В живом захвате `X-Fingerprint` **отсутствовал** во всех 221 запросах — на этой машине fingerprint был пуст | [M] | `rest-d02.md §6.2, U-08` | live |
| H-09 | `X-Installation-ID` = `installation_id_v3` (fallback `analytics_installation`) | [D] | `web.js @1002529`, `@8974100` | static |
| H-10 | `X-Discord-Locale` = локаль клиента; `X-Discord-Timezone` = IANA-таймзона; оба на каждом запросе | [D]+[M] | `web.js @1002751, @1002790`; 221/221 — `rest-d02.md §6.1` | static+live |
| H-11 | `Accept-Language` формируется вручную (`ru;q=0.9,en;q=0.8,…`) и **только** при `isPlatformEmbedded` (desktop) | [D] | `web.js @1002830` | static |
| H-12 | `X-Debug-Options`, `X-Routing-Key`, `x-client-trace-id` — опциональные (debug/routing/tracing); в живом трафике `X-Debug-Options` был на всех 221 | [D]+[M] | `web.js @1002895, @1003073`; `rest-d02.md §6.1` | static+live |
| H-13 | `X-Context-Properties` = base64(JSON контекста) — 8 запросов на 3 маршрутах | [D]+[M] | `web.js @10693826`; `rest-d02.md §6.2` | static+live |
| H-14 | `X-Failed-Requests` = счётчик ретраев; `X-Audit-Log-Reason` = encodeURIComponent(reason) | [D] | `web.js @10693890, @10693523` | static |
| H-15 | `User-Agent` приложение не задаёт: его подставляет Electron — `Mozilla/5.0 (X11; Linux x86_64) … discord/1.0.162 Chrome/148.0.7778.280 Electron/42.11.12 …` | [D]+[M] | grep `setUserAgent` = 0; `rest-d02.md §6.2` | static+live |
| H-16 | `Cookie` вручную не ставится; cookies (сессия/CF) живут в сетевом слое Electron, CDP их не показывает. Строк `__dcfduid/__sdcfduid/__cfruid` в бандле нет | [M]+[T] | `rest-d02.md U-05`; grep = 0 | live+static |
| H-17 | `client_build_number` = 641004 — совпадает с `GLOBAL_ENV.BUILD_NUMBER` живой страницы; `native_build_number` = 94186 | [D]+[M] | `/login` HTML `@1334`; `rest-d02.md §6.2` | static+live |
| H-18 | Заголовок `authorization` в нижнем регистре (3 запроса `GET /users/@me`) — либо реальная ветка кода, либо нормализация HTTP/2; в коде проверяются оба регистра | [T] | `rest-d02.md U-07`; `web.js @1002375` | live+static |

---

## 4. Токен: формат, хранение, жизненный цикл

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| T-01 | Хранилище — `localStorage`: ключи `token` (одиночный) и `tokens` (карта `{userId: token}`), плюс `__analytics__` | [D] | `web.js @11797499` (`eG="token"`, `ex="tokens"`) | static |
| T-02 | Шифрование: значение = `"dQw4w9WgXcQ:" + safeStorage.encryptString(token)`; чтение — `decryptString(...)` при наличии префикса | [D] | `web.js @11844262`; `core/bundle.js @185907` | static |
| T-03 | `safeStorage` доступен рендереру через IPC: `SAFE_STORAGE_{IS_ENCRYPTION_AVAILABLE,ENCRYPT_STRING,DECRYPT_STRING}`; main возвращает `encryptString(...).toString("base64")` | [D] | `core/bundle.js @95372`; `backend-ipc.md:67` | static |
| T-04 | Если шифрование недоступно (`isEncryptionAvailable()===false`) или значение без префикса — токен лежит **в открытом виде** в localStorage | [D] | `web.js @11844835` (`p()`/`T()`) | static |
| T-05 | Пере-шифрование существующего токена (`encryptAndStoreTokens`) вызывается на `CONNECTION_OPEN` **только** на Windows/macOS | [D]+[T] | `web.js @2222146` — вывод: на Linux первый токен остаётся в открытом виде, пока не придёт CONNECTION_OPEN на win/mac | static |
| T-06 | Формат токена — 3 сегмента через точку; regex в main-процессе: `[\w-]{24,}\.[\w-]{6,}\.[\w-]{27,}` | [D] | `core/bundle.js @186263` | static |
| T-07 | Чтение с диска (служебный модуль staff-devtools): ищет в `Local Storage/leveldb/*.ldb|*.log` сначала `"dQw4w9WgXcQ:<base64>"` → `decryptString`, иначе plaintext по regex | [D] | `core/bundle.js @185388-186400` | static |
| T-08 | Refresh-механизма нет: ни `refresh_token`, ни `/auth/refresh` в 12,4 МБ бандла (2 вхождения `refresh_token` относятся к content-inventory) | [D]+[T] | grep `refresh_token` | static |
| T-09 | Срок жизни клиентом не задаётся; отзыв — `POST /auth/logout`, смена пароля, close 4004, ручной отзыв сессий | [T] | `web.js @556041`, `@5014409` | static |
| T-10 | Logout чистит: токен(ы), fingerprint, `user_id_cache`, persisted-сторы (кроме списка исключений: InstallationManager, MultiAccount, AgeGate и др.) | [D] | `web.js @8976284` (`eo()`) | static |
| T-11 | При открытии DevTools токен **убирается** из storage (`hideToken`), при закрытии возвращается (`showToken`) | [D] | `web.js @1016683-1016969` | static |
| T-12 | Токен уходит в шлюз (IDENTIFY/RESUME) и в голосовой `op:0` — то есть это единственный credential пользовательской сессии | [D]+[M] | `web.js @5018924`; `rest-d02.md §6.5` | static+live |
| T-13 | Мультиаккаунт: `tokens`-карта; переключение = `switchAccountToken(token)` → `LOGIN_SUCCESS` → новый сокет; при логине другого аккаунта `authorization:""` | [D] | `web.js @556381, @551990` | static |

---

## 5. Gateway: подключение, IDENTIFY, heartbeat, RESUME

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| G-01 | URL: `window.GLOBAL_ENV.GATEWAY_ENDPOINT` = `wss://gateway.discord.gg`; параметры добавляются кодом: `?encoding=etf&v=9&compress=zstd-stream` | [D]+[M] | `web.js @5008623`; `/login` HTML `@1794`; `rest-d02.md §6.4` | static+live |
| G-02 | `encoding` = имя паковщика (`"etf"` при наличии erlpack, иначе `"json"`); `v` = `GLOBAL_ENV.API_VERSION` (9) | [D] | `web.js @5007600-5008700`, `@4978410` | static |
| G-03 | `compress`: `zstd-stream` при модуле `discord_zstd`, иначе `zlib-stream`, иначе параметр не добавляется | [D] | `web.js @4957336`, `@4960221` | static |
| G-04 | IDENTIFY (op2) — ровно один объект: `{token, capabilities, properties, presence, compress, client_state, qos_token}` | [D] | `web.js @5018924` | static |
| G-05 | `capabilities`: `1734653` (0x1a77fd), либо `1767421` (0x1af7fd) при включённой обфускации приватных каналов (разница — бит 15) | [D] | `web.js @5019002`; fast-connect `@30209` | static |
| G-06 | `properties` для IDENTIFY = super-properties **плюс** `client_app_state`, `is_fast_connect:false`, `gateway_connect_reasons` (напр. `"COLD_START"`), `installation_id` (если есть) | [D] | `web.js @4907605` | static |
| G-07 | `presence` = `{status, since, activities, afk}`; дефолт `online` (или `idle` при простое, `invisible` при невидимке), `activities` фильтруются настройками приватности | [D] | `web.js @9332362`, `@9330823` | static |
| G-08 | `client_state` (desktop, при включённых guild-версиях): `{guild_versions, highest_last_message_id, read_state_version, user_guild_settings_version, user_settings_version, private_channels_version, api_code_version, initial_guild_id}`; иначе `{guild_versions:{}}` | [D] | `web.js @5018320` | static |
| G-09 | `intents` и `shard` в пользовательском IDENTIFY **отсутствуют** — это поля ботов, не клиента | [D] | grep `intents` = только billing-пути | static |
| G-10 | `compress:false` для всех трёх режимов (`usesLegacyCompression()` = false у zstd-stream, zlib-stream и none) | [D] | `web.js @4957358`, `@5019100` | static |
| G-11 | `qos_token` — protobuf `QosToken` (client-provided + derived от прошлой сессии), не JWT/не токен доступа | [D]+[T] | `web.js @5018238` (`to.ob(...)`); `CORE-AUDIT.md:120` | static |
| G-12 | **Heartbeat — op40 (`QOS_HEARTBEAT`) с `{seq, qos}`, а не op1.** `op1` клиент не отправляет вовсе; `op1` от сервера — триггер немедленного heartbeat | [D]+[M] | `web.js @5020412`; `facts-frozen.json → sentPeriodic` (90 Б каждые 41 250 мс) | static+live |
| G-13 | HELLO задаёт `heartbeat_interval = 41250` мс; первый heartbeat — через `random()*interval`, далее строго по интервалу; после каждого входящего кадра — проверка «пора отправить» | [D]+[M] | `web.js @5011558`, `@5016156`, `@5015814`; `gateway-protocol.md §4` | static+live |
| G-14 | Нет ACK (op11) в срок → `_handleHeartbeatTimeout` → reconnect через backoff | [D] | `web.js @5014104` | static |
| G-15 | RESUME (op6): `{token, session_id, seq}`; условие — есть `session_id` и последний ACK не старше 3 минут | [D] | `web.js @5017590`, `@5019794` | static |
| G-16 | После READY клиент берёт `resume_gateway_url` (хвостовой `/` обрезается) и использует его для всех реконнектов; при ошибке WS — сбрасывает | [D] | `web.js @5013568`, `@5014000` | static |
| G-17 | READY: `session_id` → `SESSION_ESTABLISHED`, `resume_gateway_url` сохраняется; `READY_SUPPLEMENTAL` и `RESUMED` тоже переводят в established | [D] | `web.js @5012588` | static |
| G-18 | close `4004` → `CLOSED` без ретрая (в store это триггерит logout); op7 RECONNECT → `close(4000)` + новый коннект; op9 INVALID_SESSION: `d=true` → RESUME, `d=false` → IDENTIFY | [D] | `web.js @5014409`, `@5012301`, `@5012464` | static |
| G-19 | Fast-connect: если окно получило заранее созданный `window._ws`, до загрузки приложения отправляется IDENTIFY с `is_fast_connect:true`, `properties` = super-props + `installation_id`, `client_state:{guild_versions:{}}`, **без** `presence/compress/qos_token`; затем приложение «усыновляет» сокет, только если `state.userId` совпадает | [D] | fast-connect `@30209`; `web.js @5009125` (adopt/refuse) | static |
| G-20 | Живой захват: первый исходящий кадр 943 Б = IDENTIFY, 173–176 Б = RESUME, 90 Б каждые 41,25 с = heartbeat (теперь объяснён как op40) | [M] | `gateway-protocol.md §4`; `facts-frozen.json → sentFrames/sentPeriodic` | live |

### 5.1 Что нужно для READY (минимум)

1. TLS-соединение на `gateway.discord.gg` с `encoding=etf&v=9&compress=zstd-stream` (zstd — один
   непрерывный поток на соединение, `end()` не вызывать — `gateway-protocol.md §3`).
2. Прочитать HELLO (op10) → запомнить `heartbeat_interval`.
3. Отправить op2 IDENTIFY (см. G-04…G-11). Без `presence`/`capabilities` сервер отвечает, но
   поведение клиента отличается; `compress` можно не передавать.
4. Отвечать на heartbeat: **op40** `{seq, qos}` (не op1) и обрабатывать op11 ACK.
5. Получить op0 `READY` (в живом захвате 579 КБ JSON / 577 КБ ETF) → `session_id`,
   `resume_gateway_url`; затем `READY_SUPPLEMENTAL`.

---

## 6. Ограничения и ошибки

| id | Факт | Метка | Источник | Метод |
|---|---|---|---|---|
| E-01 | Ретраи по статусам `502, 504, 507, 598, 599, 522, 523, 524` (пока `retries > 0`) | [D] | `web.js @10692340` | static |
| E-02 | 429/503 → очередь **per-URL**: `retry-after` из заголовка (сек) или 5 с по умолчанию; backoff 1 s…60 s; запросы к этому URL ждут | [D] | `web.js @10692340`, `@10695593` | static |
| E-03 | 429 в контексте отправки сообщений дополнительно даёт `retryAfter` в ошибке (`body.code` SLOWMODE и т.п. — отдельные ветки) | [D] | `web.js @2362336` | static |
| E-04 | 50035 `INVALID_FORM_BODY` + `oldFormErrors` → плоский список ошибок полей (`errors` → `{field: message}`) | [D] | `web.js @10694717` | static |
| E-05 | Капча: `400` + `body.captcha_key` → показ капчи, повтор запроса с `X-Captcha-Key`, `X-Captcha-Rqtoken`, `X-Captcha-Session-Id`; сервисы `hcaptcha`, `recaptcha`, `recaptcha_enterprise`, `turnstile` | [D] | `web.js @1004547`, `@10697578` | static |
| E-06 | Step-up MFA: `401` + `code 60003 MFA_REQUIRED` + `body.mfa` → MFA-модалка поверх действия (не только логин) | [D] | `web.js @1004970` | static |
| E-07 | Коды ошибок входа: `50014 INVALID_TOKEN`, `40001 UNAUTHORIZED`, `50018 INVALID_PASSWORD`, `50022 INVALID_PHONE_NUMBER`, `40002 EMAIL_VERIFICATION_REQUIRED`, `70007 PHONE_VERIFICATION_REQUIRED`, `60006 MFA_INVALID_TICKET`, `60008 MFA_INVALID_CODE`, `60009 MFA_INVALID_SESSION`, `20011 ACCOUNT_SCHEDULED_FOR_DELETION`, `20013 ACCOUNT_DISABLED`, `10012 UNKNOWN_TOKEN`, `10020 UNKNOWN_SESSION` | [D] | `web.js @315000-330000` (enum `t02`) | static |
| E-08 | 401 не имеет глобального перехватчика: каждый вызов обрабатывает сам; «смерть сессии» в основном приходит как close 4004 от шлюза | [D]+[T] | grep `401===` — только локальные ветки; `web.js @5014409` | static |
| E-09 | Логин и MFA делают `retries:2`; отдельного клиентского лимитера попыток логина нет (защита — серверная) | [D]+[T] | `web.js @551777`, `@553107` | static |
| E-10 | `suspended_user_token` в теле ошибки логина → экран ACCOUNT_STANDING (не показывать как обычную ошибку) | [D] | `web.js @552070` | static |
| E-11 | Живой захват: 1×429 (`GET /users/@me/survey`), 8×404 (негативные пробы фич), 3×304 (кэш); ни одного 401/403 | [M] | `rest-d02.md §2` | live |
| E-12 | CSRF/«anti-bot» проявляется только как Cloudflare challenge-платформа (4 запроса `/cdn-cgi/challenge-platform/...`); в самом клиенте кода обхода нет | [M] | `rest-d02.md §7` | live |

---

## 7. Чего нет в дампе (и что из-за этого нельзя утверждать)

| # | Пробел | Почему | Как закрывать |
|---|---|---|---|
| N-01 | Живой вызов `/auth/login` (тело запроса и тело ответа) | в обоих захватах **0** запросов к `/api/v9/auth/*` — сессия уже была (`rest-d02.md U-02`) | окно live с тестовым аккаунтом (N6) или отдельный CDP-прогон логина |
| N-02 | QR/remote-auth протокол | есть только 4 endpoint-имени, `REMOTE_AUTH_ENDPOINT` и route; самого модуля QR в декодированных чанках нет | отдельный захват login-страницы с открытием QR-вкладки |
| N-03 | Сроки жизни: тикет MFA, код TOTP/SMS, пользовательский токен | всё серверное, в клиенте нет ни expiry, ни refresh | документация/поведение сервера; для нас — хранить только токен и обрабатывать 401/4004 |
| N-04 | Значение и момент выдачи `X-Fingerprint` | в живом трафике заголовка не было; выдаётся `GET /experiments` | не нужно: клиент не должен его форджить (см. риски) |
| N-05 | Cookie `__dcfduid/__sdcfduid/__cfruid` | CDP не отдаёт Cookie-заголовки; в бандле строк нет | только через сетевой стек/`Network.getAllCookies` (новая методика) |
| N-06 | Тела REST-ответов (включая `/users/@me`) | 200 «body»-событий — все `/assets/*.js` (`rest-d02.md U-01`) | увеличить лимит и включить `responseBody` для XHR |
| N-07 | Кто создаёт `window._ws` для fast-connect | в извлечённом JS создания сокета нет (вероятно native/preload) | REA по `discord_utils`/preload, если понадобится fast-connect |
| N-08 | Использование `/auth/sessions*` в UI | в декодированном наборе только объявления эндпоинтов | догрузить чанки настроек «Устройства» |
| N-09 | Серверные лимиты логина (попытки/IP/аккаунт) | клиентских констант нет | наблюдение в согласованном окне, без «пробива» лимитов |
| N-10 | OAuth2-выдача токенов приложениям | desktop-клиент её не делает | отдельная задача, если понадобится bot/OAuth2-путь |

**Отдельно про противоречия с прежними доками** (исправления, а не мнение):

1. `fixtures/rest/rest-static.json → auth.gateway.heartbeat` и `CORE-AUDIT.md:113` говорят
   «heartbeat = op1». В коде 1.0.162 клиент отправляет **op40** (`QOS_HEARTBEAT`) с `{seq, qos}`
   (`web.js @5020412`), op1 не отправляется ни разу. Проверка совместима с живыми кадрами по 90 Б
   каждые 41,25 с.
2. `rest-d02.md U-08` считает `X-Fingerprint` расхождением «статика ↔ живое». Уточнение: заголовок
   действительно есть в `prepareRequest`, но ставится только при непустом localStorage
   `"fingerprint"`, который выдаёт `/experiments`; после логина/логаута он очищается. То есть
   расхождения нет — есть условная ветка.
3. `rest-d02.md §6.4` перечисляет `/auth/*` как «не подтверждено». Теперь подтверждено **статикой**
   (какие пути/тела), но по-прежнему **не живьём** — статус N-01 сохраняется.

---

## 8. Риски ToS / блокировки (честно, без обхода)

1. **Сторонние клиенты для пользовательских аккаунтов запрещены ToS Discord.** Любой наш вход
   owner-токеном — риск ограничения/блокировки аккаунта. Это фиксируется письменно (устав NET §2.2),
   тестовый аккаунт — отдельный.
2. **Токен = полный доступ к аккаунту.** Хранить только в keyring (libsecret), не писать в логи,
   не отправлять в телеметрию, не показывать в UI. В эталоне Discord токен на Linux может лежать в
   localStorage открытым текстом (T-04/T-05) — это **не** пример для копирования.
3. **Автоматизация логина паролем, капчи и MFA-кодов — вне scope.** Капча (hcaptcha/recaptcha/
   turnstile) — штатный серверный барьер; клиент лишь показывает её человеку (E-05). Никаких
   «решений капчи», ферм аккаунтов, подбора кодов.
4. **Не подделывать identity.** `X-Fingerprint` выдаётся сервером, `has_client_mods` отражает
   реальность; копирование официальных super-properties «как у Discord Client» — мимикрия под
   официальный клиент, запрещена уставом. Решение о том, что именно мы отправляем в
   `X-Super-Properties`, принимает ADR-2 (вариант «честный свой клиент» vs «протокольно необходимый
   минимум»), а не этот отчёт.
5. **Лимиты.** Уважать 429/503 и `retry-after`; параллельные логины и ретраи «в лоб» ведут к
   IP/аккаунт-блокировкам. Backoff 1…60 с и очередь per-URL — обязательный минимум (E-01/E-02).
6. **Мультиаккаунт и переключение токенов** — повышенный риск (массовость/автоматизация).
   В нашем приложении не реализуем без отдельного решения владельца.
7. **Прокси-ротация, смена UA, «анти-детект»** — запрещены; обнаружение таких практик ведёт к бану
   и отзыву токенов. Единственный транспорт — TLS к официальным хостам Discord (устав NET §2.4).
8. **QR/remote-auth** — передача учётных данных между устройствами; для нас это чужой флоу, не
   реализуем.

---

## 9. Итог для следующих этапов (N2/N3)

- Минимальный жизнеспособный вход по уставу: **импорт токена владельца** → `GET /users/@me`
  (проверка) → gateway READY. Всё, что нужно для READY, перечислено в §5.1.
- Точные контракты, которые нельзя «додумывать»: `Authorization` без `Bearer` (H-01),
  `X-Super-Properties` = base64 JSON (H-03), heartbeat **op40** (G-12), `compress:false` (G-10),
  отсутствие `intents` (G-09).
- Обработка ошибок для UI: 401 (неверный токен), 4004 (сессия отозвана), 429/503 (лимит, показать
  «повторить через N»), 70007/60003 (нужен шаг подтверждения — в нашей модели просто «нужен вход в
  официальном клиенте»).
- Чего не делаем: пароль/MFA/капча-флоу, QR, мультиаккаунт-автоматизацию, любые обходы.

**Ответы на открытые вопросы NET-C1 (`BRIEF-NETC1.md`):**
- P0-1 «схема Authorization неясна» → **ясна**: сырой токен без `Bearer` (H-01, `web.js @1002375`),
  отдельного `scheme` не нужно — параметризация только для OAuth2-токенов приложений (H-02).
- P1-1 «`IdentifyProperties::from_super_properties`» → **не полное равенство**: IDENTIFY-properties =
  super-properties **плюс** `client_app_state`, `is_fast_connect:false`, `gateway_connect_reasons`,
  `installation_id?` (G-06). Простое копирование super-properties даст расхождение.
- Для будущего NET-E: heartbeat — **op40** `{seq,qos}`, не op1 (G-12); `compress:false` (G-10);
  `intents`/`shard` в IDENTIFY отсутствуют (G-09).

---

*Отчёт построен на чтении артефактов; каждое утверждение имеет offset/строку. Живые числа — из
`rest-d02.md` и `fixtures/gw-proto/facts-frozen.json`; новые живые прогоны не выполнялись.*
