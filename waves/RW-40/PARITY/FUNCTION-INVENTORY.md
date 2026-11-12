# FUNCTION-INVENTORY — эталонный Discord (1.0.166 / дамп 1.0.162)

Дата: 2026-10-24. Владелец: G6 Lead / REA operator. Статус: **draft для ревью G6**.
Устав: `G6-CHARTER.md`. Связанные: `NETWORK-ROADMAP.md`, `TASKS.md`, `FULL-REPORT.md`.

## 0. Что инвентаризировано

| Домен | Кол-во | Диапазон ID |
|---|---:|---|
| Auth / session | 9 | A-01…A-09 |
| Guilds / servers | 9 | G-01…G-08, G-03a |
| Channels / navigation | 6 | C-01…C-05, C-07 |
| Messages | 14 | M-01…M-14 |
| Members | 4 | MB-01…MB-04 |
| Presence / typing | 5 | P-01…P-05 |
| Voice | 11 | V-01…V-11 |
| Video / Go Live | 4 | S-01…S-04 |
| Attachments | 3 | AT-01…AT-03 |
| Settings | 8 | ST-01…ST-08 |
| Notifications / DND | 5 | N-01…N-05 |
| Platform (tray/deeplink/updater/capture) | 8 | PL-01…PL-08 |
| Live / connectivity (наши гейты) | 2 | L-01…L-02 |
| P0 live/security gates | 3 | P0-01…P0-03 |
| **Итого** | **91** | |

Level: `[L]` live-захват · `[S]` call-site в коде эталона · `[D]` только Enum/путь · `[M]` измерение (DOM/скрин/каталог) · `[A]` нет подтверждения.
Поле `confirmed` из `rest-static.json` не используется как evidence: оно `false` у всех 904 записей, включая работающие live-эндпоинты.
Ev: E0 нет источника · E1 статика · E2 E1+тест · E3 live-наблюдение (E3 ≠ уровень: снятый экран может быть `[M]`).
Тег `T` (дизайн-конвенция) в G6 не используется.

### Легенда источников

| Код | Источник | Версия |
|---|---|---|
| `S162` | `reference-oracle/rest-static.md` (static endpoint map web-бандла) | 1.0.162 |
| `D2` | `docs/re/rest-d02.md` (live REST) | 1.0.162 |
| `G162` | `docs/re/gateway-protocol.md` + `gateway-schema.md` (live gateway) | 1.0.162 |
| `V162` | `docs/re/voice-deep.md` (live RTC/UDP/движок) | 1.0.162 |
| `F162` | `docs/re/frontend-d03.md` (live UI DOM) | 1.0.162 |
| `I162` | `docs/re/i18n-deep.md` (каталоги/рантайм) | 1.0.162 |
| `M162` | `docs/re/main-arch.md` (main/preload, deep links, tray, updater) | 1.0.162 |
| `A166` | `app.asar:bundle.js` 1.0.166, смещение в байтах | 1.0.166 |
| `C166` | `core.asar:mainScreenPreload.js` 1.0.166, смещение | 1.0.166 |
| `UI2` | `reference-oracle/ui2/` (live DOM) | 1.0.162 |
| `UI6` | `reference-oracle/ui6/01-guild-channels.json` | 1.0.162 |
| `VUI` | `reference-oracle/voice/ui/01-*` (live voice panel) | 1.0.162 |
| `CAT` | `reference-oracle/ru-catalog.json` (строка i18n) | 1.0.162 |
| `B162` | `reference-oracle/out/bundles/discord.com_assets_web.4d4b2f75fdea5470.js` (web-бандл эталона) | 1.0.162 |
| `NC` | `waves/RW-40/NETWORK-CONTRACTS.md` | 1.0.162 |

`version-gap`: UI-функции подтверждены на 1.0.162, desktop/IPC — на 1.0.166. Это фиксируется,
а не сглаживается.

---

## 1. Auth / session

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| A-01 | Вход по логину/паролю | `S162:172` | E1 | [S] (endpoint+body by call-site; live login [A]) | `POST /auth/login`; тело `{login,password,undelete,login_source,gift_code_sku_id}`; 401/429/капча | Live-прогон 1.0.166; точные коды ошибок |
| A-02 | MFA: TOTP/SMS/backup/password | `S162:174-175` | E1 | [S] | `POST /auth/mfa/{method}`, `POST /auth/mfa/sms/send`; поля `code,ticket,login_source,gift_code_sku_id,login_instance_id` | Порядок методов/таймауты на 1.0.166 |
| A-03 | Captcha-гейт | `S162:51,171` | E1 | [S] | `GET /auth/location-metadata` → `captcha_required`; `X-Captcha-*` заголовки | sitekey/flow на 1.0.166 |
| A-04 | QR / one-time login | `S162:176` | E1 | [S] (QR flow [A]) | `POST /auth/one-time-login`; UI-строка «Отсканируйте QR-код…» | Полный QR-протокол (payload/поллинг) — unknown |
| A-05 | Passkey / WebAuthn login | `S162:166-167,177` | E1 | [S] (Linux WebAuthn [A]) | `POST /auth/conditional/start\|finish`, `/auth/passwordless/start`; UI «ключ доступа» | Linux-ветка WebAuthn (IPC `WEBAUTHN_*` только mac) |
| A-06 | Хранение и восстановление сессии | `S162:65-67` | E1 | [S] (safeStorage code) | `TokenStore`: localStorage `token/tokens`, Electron `safeStorage` + префикс `dQw4w9WgXcQ:` | Наш keyring — [A], не копия механизма |
| A-07 | Logout / инвалидация сессии | `S162:101,173` | E1 | [S] | `POST /auth/logout`, тело `{provider,token,voip_provider,voip_token}` | Live-проверка остановки gateway |
| A-08 | Регистрация / сброс пароля | `S162:168,178-179` | E1 | [S] | `/auth/register`, `/auth/forgot`, `/auth/reset` | Продуктовый scope desktop — вне P1 |
| A-09 | Список сессий / выход на других устройствах | `S162:181-182` | E1 | [S] | `GET /auth/sessions`, `POST /auth/sessions/logout` (`session_id_hashes`) | UI-экран сессий не снят |

## 2. Guilds / servers

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| G-01 | Рейл серверов: список, выбор, переключение | `UI6:01, UI:00-app.png` | E3 | [M] (live screen measurement) | Гильдии в rail, выбранный сервер, каналы выбранной гильдии | Точный источник списка (READY.guilds) и восстановление выбора |
| G-02 | Добавить/создать сервер | `S162:1682 (GUILDS enum)` | E1 | [D] (call-site 0) | UI-строки «Добавить сервер», «Создать сервер» | Endpoint создания: `GET\|POST /guilds` метод **не определён** (`S162`) → E0 |
| G-03 | Вход на сервер по инвайту | `NC:1.1#1,1.2` | E1 | [S] (body; live codes/TTL [A]) | `POST /invites/{code}`; тело `{session_id,invite_instance_id,received_installation_id}`; пункт «Присоединиться» | live-коды 200/4xx и TTL — [A] |
| G-03a | Войти в сервер из preview/lurker | `D02:101,138,225` | E3 | [L] (204×2; requestId ready\|57992.6989, run2\|95974.7409) | `PUT /guilds/{id}/members/@me?lurker=true` + `{}` → **204**; тело пустое; прочие коды [A] | 403/429/lurker-семантика |
| G-04 | Выход с сервера | `NC:1.1#4` | E1/E3 | [S] (response/codes [A]) | `DELETE /users/@me/guilds/{id}`, тело `{lurking}`; пункт «Покинуть сервер» + confirm | response/коды [A]; поведение при owner |
| G-05 | Меню сервера | `UI2:D1` | E3 | [M] (live screen measurement) | 10 пунктов: read, invite, mute, notify, hide muted, privacy, profile, move, leave, copy ID | Подменю/диалоги не сняты |
| G-06 | Инвайты: создать/список/отозвать | `NC:1.1#8-11` | E1 | [S] (codes [A]) | `GET\|POST /channels/{id}/invites`, `GET /guilds/{id}/invites`, `DELETE /invites/{code}` | коды/TTL [A]; поля модалки (срок, лимит) |
| G-07 | Настройки сервера: роли, каналы, права | `S162:340,409,97` | E1 | [S] (UI [A]) | `POST\|PATCH /guilds/{id}/channels`, `/roles`, `PATCH\|DELETE /guilds/{id}/members/{id}` | UI не снят; payload/коды [A] → P3 |
| G-08 | Unread/mention-маркеры сервера | `UI6:01` | E3 | [M] | `aria="не прочитано, …"`, бейджи каналов | Счётчики на рейле/папки не сняты |

## 3. Channels / navigation

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| C-01 | Дерево каналов: категории, text/voice/forum | `UI6:01` | E3 | [M] | `href=/channels/<g>/<c>`, aria-типы «текстовый канал», «канал форума», voice-каналы | Полный список типов (announcement/stage) |
| C-02 | Переключение канала + загрузка истории | `D2:70,180` | E3 | [L] (history live; UI switch [M]) | GET `/channels/{id}/messages`; канал в URL | Восстановление канала после reload |
| C-03 | Unread/mention в канале | `UI6:01` | E3 | [M] | `не прочитано` в доступном имени | Точная модель read_state/mention_count |
| C-04 | Шапка канала: пины, ветки, участники, поиск | `D2:109-110` | E3 | [L] (UI panels [M]) | pins и threads/search наблюдались live; поиск — D-03 | Inbox/уведомления в шапке не сняты |
| C-05 | Создание/редактирование канала и прав | `S162:200,340` | E1 | [S] (UI [A]) | `POST\|PATCH /guilds/{id}/channels`, `PATCH\|DELETE /channels/{id}`; тело `{name,type,topic}` | UI/диалоги не сняты → P3 |
| C-07 | QuickSwitcher `Ctrl+K` | `F162:118` | E3 | [S] (code registry; screen [M]) | Модуль 675704 (`mod+k`), список каналов/гильдий | Точный алгоритм ранжирования |

## 4. Messages

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| M-01 | История: пагинация, окно | `D2:70,180` | E3 | [L] | `GET /channels/{id}/messages` (limit/cursor), 200×11 | Точный `before/after` и лимиты на 1.0.166 |
| M-02 | Отправка сообщения | `S162:223` | E1 | [S] (body [A]) | `POST /channels/{id}/messages`; тело в статике не раскрыто | Тело (`content/nonce/tts`) и optimistic-поведение live не сняты |
| M-03 | Редактирование своего | `S162:228` | E1 | [S] (body [A]) | `PATCH /channels/{id}/messages/{id}`; статик body `{flags},{attachments}` | Тело edit (`content`) live не снято |
| M-04 | Удаление своего | `S162:228` | E1 | [S] | `DELETE /channels/{id}/messages/{id}` | Confirm/tombstone поведение |
| M-05 | Ответ (reply) | `B162:message_reference` | E1 | [S] (call-site reply; live field [A]) | `reference` в MESSAGE_CREATE не наблюдался (19 ключей) | Точное тело reply не снято |
| M-06 | Реакции: поставить/снять | `D2:108` | E3 | [L] | `PUT .../reactions/{emoji}/@me` → 204 (live); `DELETE` — static | Лимиты/кастомные эмодзи |
| M-07 | Read state / ack | `D2:73,207` | E3 | [L] | `POST .../ack`, тело `{token,last_viewed}` → 200×5 | `mention_count/manual` не наблюдались |
| M-08 | Markdown, embeds, mentions | `G162:433-438` | E3 | [L] (markdown rules [S]) | `MESSAGE_CREATE` 19 ключей: `content/embeds/mentions/...` | Полный набор markdown-правил — `I162` |
| M-09 | Контекстное меню сообщения | `UI:01` | E3 | [M] | 15 пунктов: reaction, forward, copy, bookmark, reminder, apps, unread, link, TTS, report, copy ID | Подменю/права не сняты |
| M-10 | Пины | `D2:109` | E3 | [L] | `GET /channels/{id}/messages/pins` → 200 | UI пинов не снят |
| M-11 | Ветки (threads) | `D2:110` | E3 | [L] | `GET /channels/{id}/threads/search` → 200 | Создание/архив веток |
| M-12 | Forward/bookmark/reminder/TTS/report | `UI:01` | E3 | [M] (endpoints [A]) | Пункты меню подтверждены DOM | Endpoint'ы не сняты → P3 |
| M-13 | Поиск по серверу: 7 фильтров, 3 сортировки | `F162:1, 20f` | E3 | [L] (live network capture) | `GET /guilds/{id}/messages/search`; подтверждены `content,channel_id,sort_by,sort_order,offset`; 7 секций UI | Остальные 6 фильтров и лимиты — `ATTACH-RE` gap |
| M-14 | Эмодзи-пикер | `F162:2` | E3 | [M] | 3 вкладки, 10 категорий, поиск, 50 items, locked | Кастомные эмодзи/стикеры |

## 5. Members

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| MB-01 | Список участников: роли + статусы | `F162:55-62` | E3 | [M] | Группы «Server Booster», «В сети — 735»; строки 35; member 42px | Точная сортировка ролей |
| MB-02 | Роли, цвета, права | `G162:397` | E3 | [L] (roles in READY; UI [M]) | Роли в READY; группы участников | UI управления ролями не снят |
| MB-03 | Профиль-поповер участника | `UI5:04` | E3 | [M] | Поповер открывается; `06-profile-popout.png` | Поля профиля (текст извлёкся `null`) |
| MB-04 | Полный поиск участников | `NC:1.5` | E1 | [S] (202 retry; pagination [A]) | `POST /guilds/{id}/members-search` | 202 retry [S]; пагинация/коды [A] |

## 6. Presence / typing

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| P-01 | Статусы online/idle/dnd/offline/invisible | `G162:76,293` | E3 | [L] | 26-словный словарь: `online/idle/dnd/invisible`; `status: invisible` в READY | Приоритет статусов при нескольких сессиях |
| P-02 | Обновления присутствия | `G162:152,397` | E3 | [L] | `PRESENCE_UPDATE` 107; `READY_SUPPLEMENTAL.merged_presences` | `activities`-схема частично |
| P-03 | Индикатор набора + `POST /typing` | `S162:253, NC:1.6` | E3/E1 | [S] (TYPING_START live [L]; TTL [A]) | `TYPING_START` 28 событий; `POST /channels/{id}/typing`; строка «Печатает…» | Точный payload/таймаут — [M] (в core 10 с) |
| P-04 | Кастомный статус/активность | `G162:250` | E3 | [L] | activities type 0/4, `subText` участника | Редактор статуса не снят |
| P-05 | Смена своего статуса | `G162:293, UI3:B-panels` | E3 | [L] (invisible in READY) | Панель «ShadowByte … Невидимый», меню статуса | Точный `presence` в IDENTIFY — [M] |

## 7. Voice

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| V-01 | Вход/выход из голосового канала | `V162:23-52` | E3 | [L] | 3 live-сессии; кнопка «Отключиться», чистый close 1000 | Live 1.0.166 нет |
| V-02 | RTC-сигнализация (WS v8) | `V162:23-52` | E3 | [L] | op0/1/2/4/8/15/16/3/6, endpoint из VOICE_SERVER_UPDATE | Reconnect-сценарии |
| V-03 | UDP discovery/select/keepalive/rotation | `V162:53-90` | E3 | [L] | discovery 74 Б, keepalive 8 Б/5 с, ротация +60,08 с | IPv6/двойной стек |
| V-04 | SRTP-крипто | `V162:65-90` | E3 | [L] (decrypt [A]) | `aead_aes256_gcm_rtpsize`, tag16, tail4, KDF | DAVE-ключи — отдельно |
| V-05 | Opus/джиттер/DTX/CN | `V162:91-118` | E3 | [L] (jitter model [M]) | engine log: audio codec `opus`; DTX/CN | Точные параметры буфера — [M] |
| V-06 | Статистика/reconnect/регионы | `V162:119-123` | E1 | [M] (ping UI; reconnect code [S]) | ping в UI; RTCP тела шифрованы | Пороги reconnect — [M]/[A] |
| V-07 | Устройства, mute/deafen, PTT | `F162:63-83` | E3 | [M] (settings screen) | Микрофон/динамик/камера, 38 тумблеров, PTT, mute/deafen | Hot-plug и точные 38 id |
| V-08 | Голосовой UI: панель, участники, speaking, ping | `VUI:01` | E3 | [M] | Панель, ping 93 ms, участники, «микрофон отключён» | Speaking-анимация не измерена |
| V-09 | Шумоподавление Krisp | `V162:91-118` | E3 | [L] (engine log) | `krisp-nc-o-med-v7.kef`, селект «Krisp» | Linux-модель и лицензия |
| V-10 | DAVE/MLS E2EE | `V162:91-118` | E3 | [L] (init; 2-client [A]) | «DAVE protocol init v1», MLS external sender/key package | Полный MLS-обмен и интероп |
| V-11 | Голосовые сообщения/клипы/soundboard | `C166@11203` | E1/E3 | [D] (clips enum; soundboard live [L]) | IPC capture/clips; soundboard 21 | Контракт клипов/записи |

## 8. Video / Go Live

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| S-01 | Камера: preview/enable | `V162:29-32` | E3 | [L] (video capability; camera [A]) | RTC Identify `video:true`, Ready `streams[{type:video,ssrc,rtx,rid:100,quality:100}]`; UI «Камера недоступна» | Захват камеры live не выполнялся |
| S-02 | Демонстрация экрана / Go Live start-stop | `VUI:01, C166@23840` | E1 | [D] (capture IPC/UI; STREAM [A]) | UI «Продемонстрируйте свой экран»; `DESKTOP_CAPTURER_GET_SOURCES`, `ACTIVITY_CAPTURE_STREAM_*` | Gateway `STREAM_*`, stream_key, разрешения — **unknown** |
| S-03 | Зрители, качество, simulcast | `NC:3.3` | E1 | [A] (viewer flow; STREAM_* code is [S]) | Ready: rid 100, quality 100, rtx; remote sink wants `{any:100}` | Декод/отрисовка у зрителя — не снято |
| S-04 | Activities (встроенные приложения) | `VUI:01` | E3 | [M] | «Начать активность»; activities type 0/4 | Контракт запуска активности |

## 9. Attachments

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| AT-01 | Загрузка файла: multipart, прогресс | `S162:817` | E1 | [D] (path only; method/fields [A]) | Путь `/channels/{id}/attachments` (`MESSAGE_CREATE_ATTACHMENT_UPLOAD`), метод **не определён**, call-site 0 | Метод, поля, лимиты, 413/429 — **RE-blocked** |
| AT-02 | Drag&drop / paste файла | `CAT:IJyOUf` | E1 | [M] | Строки «Перетащите…», «Прикрепить вложения» | Триггеры/лимиты |
| AT-03 | Рендер вложений в сообщении | `G162:433-438` | E1 | [D] (key null; schema [A]) | Ключ `attachments` в MESSAGE_CREATE (в дампе `null`); `POST /attachments/refresh-urls` | Схема attachment-объекта и CDN-подписи |

## 10. Settings

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| ST-01 | Навигация настроек (63 строки) | `F162:63-70` | E3 | [M] | 18 верхнеуровневых + подпункты, всего 63 строки | Состав вложенных экранов |
| ST-02 | Голос/видео: 38 тумблеров, устройства | `F162:71-83` | E3 | [M] | 38 `role=switch`, mic/spk/cam, Krisp, PTT, 4 слайдера | Application/effects на Linux |
| ST-03 | Внешний вид: тема, плотность, масштаб | `CAT:K2sFfo, F162:145-158` | E1/E3 | [S] (tokens/code) | Светлая/тёмная, компактный, zoom | Точные токены light |
| ST-04 | Язык/время + i18n-рантайм | `I162:14,205` | E1 | [S] (runtime; counts [M]) | 23 836 ключей, 31/36 локалей, плурализация, rich-разметка | 1.0.166 каталоги не сняты |
| ST-05 | Уведомления: настройки | `S162:680` | E1/E3 | [S] (UI [M]) | `PATCH /users/@me/notification-settings`; раздел «Уведомления» | Точные уровни/поля |
| ST-06 | Горячие клавиши | `F162:118` | E1 | [S] | Реестр шорткатов: `mod+f`, `mod+k`, `mod+e` | Редактор биндингов не снят |
| ST-07 | Аккаунт/профиль: имя, аватар, about | `F162:63-70` | E3 | [M] (fields [A]) | Разделы «Учётная запись», поповер профиля | Поля/эндпоинты смены профиля не сняты |
| ST-08 | Персистентность/синхронизация настроек | `D2:95,115` | E3 | [L] | `GET/PATCH /users/@me/settings-proto/{num}` (live GET; PATCH non-json) | Формат protobuf — `settings_proto` в core |

## 11. Notifications / DND

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| N-01 | Системные уведомления | `C166@63242, M162:892-900` | E1 | [S] (IPC+native chain) | `DISCORD_NOTIFICATIONS_SEND_NOTIFICATION` → native `discord_notifications.sendNotification` | Linux-реализация (наша D-Bus — [A]) |
| N-02 | DND / режим концентрации | `CAT:gH3Frd,gYEF6m` | E1 | [S] (DND string; suppression rules [A]) | Статус DND, «режим концентрации», lock-screen IPC | Точные правила подавления/таймеры |
| N-03 | Inbox / упоминания | `CAT` | E1 | [M] (strings only) | Раздел существует | UI/счётчики не сняты |
| N-04 | Бейдж/прогресс приложения | `C166@13624` | E1 | [S] | `APP_SET_BADGE_COUNT`, `WINDOW_SET_PROGRESS_BAR`, `APP_DOCK_*` | Linux Unity Launcher |
| N-05 | Переопределения на сервер/канал | `UI2:D1` | E3 | [M] | «Параметры уведомлений Только @упоминания», «Заглушить сервер» | Точные уровни и меню канала |

## 12. Platform: tray, deep links, updater, capture

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| PL-01 | Трей и фоновый режим | `C166@62339, M162:861-869` | E1 | [S] | `SYSTEM_TRAY_*`, `TOGGLE_MINIMIZE_TO_TRAY`; tray mute/deafen/voice settings | Linux-меню/иконки |
| PL-02 | Автозапуск / старт свёрнутым | `A166@7201, C166@62475` | E1 | [S] | XDG `.desktop` с `X-GNOME-Autostart-enabled`; `TOGGLE_OPEN_ON_STARTUP`, `TOGGLE_START_MINIMIZED` | — |
| PL-03 | Deep links `discord://` + single instance | `A166@23596` | E1 | [S] | `extractUrlFromArgs`, `open-url`, `second-instance`, `openOrQueueUrl`; `discord://[-]/channels/...` | Точная маршрутизация окон |
| PL-04 | Апдейтер | `A166@7201, M162:579-591` | E1 | [S] | `NEW_UPDATE_ENDPOINT`, pinned manifest, `host-updated`; `QUIT_AND_INSTALL` | Подпись/канал обновлений |
| PL-05 | Spellcheck | `C166@108699` | E1 | [S] | `SPELLCHECK_*`: словари, языки, learned words, replace | Linux-hunspell словари |
| PL-06 | Питание/idle | `C166@90009` | E1 | [S] (thresholds [A]) | `POWER_MONITOR_SUSPEND/RESUME/LOCK_SCREEN`, idle time | Порог idle→status — **unknown**, у нас [A] |
| PL-07 | Разрешения/consent на захват | `C166@23840` | E1 | [D] (capture IPC; consent flow [A]) | `getDesktopCaptureSources` есть; consent-диалог в дампе не снят | Consent-flow — RE/design |
| PL-08 | Окно: badge/progress/flash/PiP | `C166@120721` | E1 | [S] | `WINDOW_SET_PROGRESS_BAR`, `WINDOW_FLASH_FRAME`, `WINDOW_OPEN_DOCUMENT_PIP` | — |

## 13. Live / connectivity (наши гейты)

| ID | Функция | Источник | Ev | Level | Подтверждено / контракт | Unknown / RE-gate |
|---|---|---|---|---|---|---|---|
| L-01 | Reconnect/offline state в UI | `G162:152` | E3 | [M] (UI state; gateway resume [L]) | Эталон показывает reconnect/resume; у нас `GatewayStatus` есть, UI нет | Точные тексты/тайминги 1.0.166 |
| L-02 | Dispatch → UI (guilds/channels/members) | `G162:152,397` | E3 | [L] (UI mapping [M]) | READY/CHUNK/PRESENCE несут гильдии, каналы, участников | Полный набор событий и порядок применения |

## 14. P0 live/security gates (не функции эталона, а условия приёмки)

| ID | Гейт | Источник | Ev | Level | Условие |
|---|---|---|---|---|---|---|
| P0-01 | Live READY на тестовом аккаунте | `G162` | E3* | [L] (reference READY; ours [A]) | READY + guild/channel в логе, токен не в логе, окно 1 |
| P0-02 | Live send | `S162:223` | E1* | [S] (body [A]) | сообщение видно в канале; 401/429 обработаны; окно 1 |
| P0-03 | Секреты/ключи не в логах | `S162:65-67` | E1 | [S] (mechanism; logs [A]) | grep логов на token/password/key = 0; redaction-тест |

`E3*`/`E1*` — evidence эталона; у нас live-прогона нет. Гейт закрывается только окном PM.

---

## 15. Ограничения инвентаризации

1. Renderer 1.0.166 не входит в asar; UI подтверждён на 1.0.162 (`version-gap`).
2. `STREAM_*`/Go Live не наблюдались; S-02 — `[D]`, не `[L]`; viewer — `[A]`.
3. Тела POST/PATCH сообщений не наблюдались live: это `[A]`. Тело invite — `[S]`; live-коды/TTL — `[A]`. Поле `confirmed` из `RS` не используется.
4. Схема `attachments` не построена: ключ есть, значения `null`.
5. Полный список 904 endpoint'ов (`S162`) шире таблицы; в инвентарь взяты только домены G6.
6. Функции DM/друзей, магазина, Nitro, квестов — вне scope (NETWORK-ROADMAP §11).
