# Каталог функций Rust-клиента (по данным обратного инжиниринга)

Каждая строка: **ID** → функция → зачем нужна → как работает (механика, снятая RE) → источник →
фаза плана (`PLAN.md`). Фазы: P0 спайки → P1 ядро сети → P2 данные → P3 UI → P4 голос →
P5 видео/стрим → P6 платформа → P7 полировка.

## GW — шлюз событий

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-GW-01 | Подключение к шлюзу | Канал событий реального времени | `wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream`; сервер может вернуть `resume_gateway_url` (напр. `us-east1-c/d`) | `gateway-protocol.md` §1 | P1 |
| F-GW-02 | HELLO/Identify | Регистрация сессии | op10 HELLO `{heartbeat_interval: 41250}`; op2 IDENTIFY (~943 B) с token/properties; 4 свежих логина в захвате | `gateway-protocol.md` §4 | P1 |
| F-GW-03 | Heartbeat/ACK | Держать соединение живым, мерить RTT | op1 каждые 41,25 с (~90 B), ответ op11 ACK (196 всего) | `gateway-protocol.md` §4 | P1 |
| F-GW-04 | RESUME | Пережить разрыв без потери событий | op6 (~173/176 B) с session_id/last_seq → `RESUMED`; серия `s` продолжается (95→663, 664→722, 332→371) | `gateway-protocol.md` §4 | P1 |
| F-GW-05 | Реконнект | Восстановление по инициативе сервера/клиента | op7 RECONNECT → закрыть сокет → новый + RESUME; клиентский реконнект ~2,9 с после закрытия | `gateway-protocol.md` §4 | P1 |
| F-GW-06 | Роутер диспатчей | Типизированная доставка событий | op0 (21 тип всего): READY, READY_SUPPLEMENTAL, RESUMED, THREAD_LIST_SYNC, SESSIONS_REPLACE, CHANNEL_INFO, GUILD_MEMBER_LIST_UPDATE (1048), GUILD_MEMBERS_CHUNK, PRESENCE_UPDATE (107), MESSAGE_CREATE (31), MESSAGE_UPDATE (24), MESSAGE_ACK, VOICE_STATE_UPDATE, VOICE_CHANNEL_*, SOUNDBOARD_SOUNDS, MESSAGE_POLL_VOTE_ADD, TYPING_START, MESSAGE_REACTION_ADD, USER_SETTINGS_PROTO_UPDATE (1340 событий всего) | `gateway-protocol.md` §4 | P1 |
| F-GW-07 | Ленивые подписки | Экономия трафика для больших гильдий | op37 SUBSCRIPTIONS: диапазоны `[[0,99]]` на список участников; дозагрузка при скролле | `gateway-protocol.md` §4, `gateway-schema.md` §4 | P2 |
| F-GW-08 | QoS-телеметрия | Сервер оценивает качество канала | op40 (`ver:31`) периодически (~41 с) | `gateway-protocol.md` §4 | P1 |
| F-GW-09 | Запрос информации о канале | Догрузка каналов вне READY | op43 REQUEST_CHANNEL_INFO → dispatch CHANNEL_INFO | `gateway-protocol.md` §4 | P2 |
| F-GW-10 | Присутствие (отправка) | Управление своим статусом | op3 PRESENCE; в захвате — `invisible` | `gateway-protocol.md` §4 | P2 |
| F-GW-11 | Голосовое состояние | Вход/выход из голосового канала | op4 VOICE_STATE_UPDATE (клиент→шлюз) → VOICE_SERVER_UPDATE с RTC-эндпоинтом | `voice-deep.md` §1 | P4 |

## ENC — кодеки транспорта

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-ENC-01 | ETF-кодек | Бинарный формат шлюза | 11 фактических тегов; big-int: тег 110/111, длина, **знак до цифр**, LE-байты; snowflake — строка с суффиксом `n`; nil → «nil» | `gateway-protocol.md` §2 | P1 |
| F-ENC-02 | zstd-stream | Сжатие потока шлюза | Потоковый контекст на сокет; магия `28 B5 2F FD` только в кадре 0; инкрементальный декод без `end()` | `gateway-protocol.md` §3 | P1 |
| F-ENC-03 | Порядок/дедуп событий | Консистентность состояния | `s`-номер в каждом диспатче; разрывов не наблюдалось; при resume — продолжение | `gateway-protocol.md` §4 | P1 |

## AUTH — вход и сессия

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-AUTH-01 | Вход e-mail/пароль + MFA | Первичная авторизация | REST-поток логина (наблюдались заголовки auth; MFA-ветка — по статике) | `rest-d02.md` §6 | P1 |
| F-AUTH-02 | Хранилище токена | Безопасность | keyring (Secret Service/KWallet); в файлах токена нет; редактирование в логах | политика проекта | P1 |
| F-AUTH-03 | Сессия/резюм | Непрерывность | `session_id`, `resume_gateway_url`, `auth_session_id_hash`; авто-resume при старте | `gateway-schema.md` §3 | P1 |
| F-AUTH-04 | Смена/выход | Мультиаккаунт | Смена токена, очистка кэшей, переподключение шлюза | — | P6 |
| F-AUTH-05 | WebAuthn/MFA-ключи | Безопасный вход | Наблюдались каналы и REST (`/users/@me/mfa/webauthn/credentials`); регистрация/аутентификация | `main-arch.md` §4.2 | P7 |

## API — REST v9

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-API-01 | REST-клиент | Все действия вне шлюза | База `https://discord.com/api/v9`; 6 заголовков auth-группы; коды 200/204/304/404/429 | `rest-d02.md` §2, §6 | P1 |
| F-API-02 | Супер-свойства | Параметры клиента в каждом запросе | `X-Super-Properties` — base64 JSON, 22 поля (os/browser/build 641004…); политика: свои честные значения, конфигурируемо | `rest-d02.md` §6.2 | P1 |
| F-API-03 | Отправка сообщений | Основная функция | `POST /channels/{id}/messages` (multipart с вложениями при загрузке) | `rest-d02.md` §3–4 | P3 |
| F-API-04 | Ack/read | Снятие непрочитанного | `POST /channels/{id}/messages/{id}/ack` `{token, last_viewed}` | `rest-d02.md` §4 | P2 |
| F-API-05 | Реакции | Взаимодействие | `PUT/DELETE /channels/{id}/messages/{id}/reactions/{emoji}/@me` (204) | `rest-d02.md` §2 | P3 |
| F-API-06 | Правка/удаление | Управление сообщениями | `PATCH/DELETE /channels/{id}/messages/{id}` | `rest-d02.md` §8.2 | P3 |
| F-API-07 | История и пины | Чтение канала | `GET /channels/{id}/messages` (11 запросов), `GET /channels/{id}/messages/pins` | `rest-d02.md` §2 | P2 |
| F-API-08 | Поиск | Найти сообщения | `GET /guilds/{id}/messages/search` + 45 query-параметров (author/mentions/has/link/embed/file и др.) | `rest-d02.md` §3 | P3 |
| F-API-09 | Треды | Ветки обсуждений | `GET /channels/{id}/threads/search`, `THREAD_LIST_SYNC`, THREAD_CREATE/UPDATE | `rest-d02.md` §2, `gateway-protocol.md` §4 | P3 |
| F-API-10 | Профили | Карточка пользователя | `GET /users/{id}/profile` (bio, роли, активности) | `rest-d02.md` §2 | P3 |
| F-API-11 | Настройки (proto) | Синхронизация настроек | `GET/PATCH /users/@me/settings-proto/{num}` — протобуф (`(non-json)`); декодер полей | `rest-d02.md` §4 | P2 |
| F-API-12 | Rate-limit | Устойчивость | 429 наблюдался (`/users/@me/survey`); парсить `Retry-After`/X-RateLimit-*, экспоненциальный бэкофф | `rest-d02.md` §2 | P1 |
| F-API-13 | Прочие ресурсы | Полнота | quests/promotions/entitlements/library/widgets/games/collectibles — только отображение | `rest-d02.md` §2 | P6 |
| F-API-14 | Телеметрия off | Приватность | `POST /science`, `POST /beaker` — не отправлять (по умолчанию) | `rest-d02.md` §2 | P1 |

## DATA — состояние и хранилище

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-DATA-01 | Парсер READY | Начальное состояние | 40 ключей; `guilds[]` (в крупной гильдии 83 канала, 34 роли, 46 эмодзи, 24 229 участников), 17 ЛС, 60 связей, 146 users, 157 read_state, сессии, `user_settings_proto` | `gateway-protocol.md` §5, `gateway-schema.md` §3 | P2 |
| F-DATA-02 | Хранилище структур | Офлайн-старт и скорость | SQLite: guilds/channels/roles/users/emoji; сортировки, папки, позиции | план | P2 |
| F-DATA-03 | Ленивые участники | Большие списки | GUILD_MEMBER_LIST_UPDATE с ops sync/insert/update/delete; GUILD_MEMBERS_CHUNK; подписки op37 | `gateway-schema.md` §4 | P2 |
| F-DATA-04 | Read-state | Бейджи непрочитанного | 157 записей из READY; обновления MESSAGE_ACK; расчёт mentions | `gateway-protocol.md` §5 | P2 |
| F-DATA-05 | Настройки пользователя | Единый профиль настроек | `settings-proto` (4400 Б в дампе) → модель (тема, плотность, уведомления, голос) | `gateway-schema.md` §3 | P2 |
| F-DATA-06 | Кэш сообщений | Быстрый скролл/офлайн | Локальная БД, пагинация вверх/вниз, лимиты, TTL, индексы по каналу/времени | план | P2 |
| F-DATA-07 | Отношения | Друзья/блокировки | READY.relationships (60), запросы, блокировки | `gateway-protocol.md` §5 | P2 |
| F-DATA-08 | Голосовые статусы в дереве | UX звонков | VOICE_STATE_UPDATE/CHANNEL_STATUS_UPDATE/START_TIME_UPDATE → участники и таймер | `gateway-protocol.md` §4 | P4 |
| F-DATA-09 | Замена сессий | Мультиустройство | SESSIONS_REPLACE (8 за сессию) → актуальный список | `gateway-protocol.md` §4 | P6 |

## MSG — сообщения

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-MSG-01 | Текст + markdown | Общение | Разметка сообщений (23 правила rich-формата), эмодзи/упоминания/ссылки; спойлеры/цитаты | `i18n-deep.md` §3 | P3 |
| F-MSG-02 | Вложения | Файлы и медиа | upload с прогрессом, download в кэш, превью изображений/видео, лимиты | `rest-d02.md` §4 | P3 |
| F-MSG-03 | Ответы | Контекст | Reply-ссылки, упоминание автора, переход к оригиналу | `frontend-d03.md` (меню) | P3 |
| F-MSG-04 | Реакции | Быстрый отклик | Пикер реакций из контекстного меню; счётчики; список поставивших | `frontend-d03.md` §2.3 | P3 |
| F-MSG-05 | Правка/удаление/меню | Управление | Контекстное меню сообщения (12+ пунктов), редактирование inline | `frontend-d03.md` §5.4 | P3 |
| F-MSG-06 | Typing | Живость | TYPING_START (28/сессию) → индикатор «печатает» | `gateway-protocol.md` §4 | P3 |
| F-MSG-07 | Опросы | Голосования | MESSAGE_POLL_VOTE_ADD (MESSAGE_POLL_VOTE_REMOVE не наблюдался); отображение результатов | `gateway-protocol.md` §4 | P6 |
| F-MSG-08 | Стикеры/GIF/звуки | Выразительность | SOUNDBOARD_SOUNDS (21), стикеры в гильдии (6), GIF-поиск | `gateway-protocol.md` §4 | P6 |
| F-MSG-09 | Треды | Ветки | THREAD_LIST_SYNC (7), создание/архив/активность | `gateway-protocol.md` §4 | P3 |
| F-MSG-10 | Системные сообщения | Полнота истории | join/boost/pin/инвайты; разделители новых сообщений | `frontend-d03.md` | P3 |
| F-MSG-11 | Черновики | UX ввода | Локальное сохранение на канал/тред, восстановление | план | P3 |
| F-MSG-12 | Saved messages/поиск-сохранение | Личные заметки | `GET /users/@me/saved-messages` (404 у тест-аккаунта; маршрут известен) | `rest-d02.md` §2 | P7 |

## SRCH — поиск

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-SRCH-01 | Панель фильтров | Точный поиск | 7 секций фильтров + чипы; `from/mentions/has/link/embed/file/…`; сортировка (актуальность/дата) | `frontend-d03.md` §1.2–1.3 | P3 |
| F-SRCH-02 | Результаты | Переход к контексту | `total_results`, пагинация, подсветка, прыжок к сообщению | `frontend-d03.md` §1.1 | P3 |
| F-SRCH-03 | Не повторять баг UI | Корректность | В официальном UI чип канала даёт `total_results: 0` — реализовать правильно | `frontend-d03.md` §1.4 | P3 |
| F-SRCH-04 | QuickSwitcher | Навигация | Ctrl+K → каналы/ЛС/гильдии, fuzzy | `frontend-d03.md` §5.4 | P3 |
| F-SRCH-05 | Автокомплит эмодзи | Ввод | `:fire` → 10 результатов в поповере | `frontend-d03.md` §2 | P3 |

## PRES — присутствие

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-PRES-01 | Статусы | Видимость | online/idle/dnd/invisible (op3); сохраняется в настройках; idle по бездействию | `gateway-protocol.md` §4, `backend-ipc.md` (idle) | P2 |
| F-PRES-02 | Активности | Что делает друг | PRESENCE_UPDATE (107) с activities; отображение в списках/поповере | `gateway-protocol.md` §4 | P2 |
| F-PRES-03 | Кастомный статус | Самовыражение | текст/эмодзи + срок; часть settings/профиля | `frontend-d03.md` §5 | P6 |
| F-PRES-04 | Группировка | Читаемость | Сортировка участников/друзей по статусу и роли | `frontend-d03.md` §3 | P3 |

## VOICE — голос

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-VOICE-01 | Join/Leave | Голосовая связь | op4 → VOICE_SERVER_UPDATE → RTC WS `wss://c-<region>-<hash>.discord.media:2053/2096` (v=9); выход — close 1000 «Force Close» | `voice-deep.md` §1 | P4 |
| F-VOICE-02 | Identify (RTC) | Вход в сессию | op0: server_id, channel_id, user_id, session_id, token, `max_dave_protocol_version:1`, video, streams `[{type:video, rid:"100", quality:100}]` | `voice-deep.md` §1 | P4 |
| F-VOICE-03 | Select Protocol | Выбор транспорта | op1: protocol udp, `codecs:[{opus PT 120 priority 1000},{AV1 PT 101}]` | `voice-deep.md` §1 | P4 |
| F-VOICE-04 | Ready | Параметры медиа | op2: `ssrc`, `ip`, `port`, `modes:["aead_aes256_gcm_rtpsize","aead_xchacha20_poly1305_rtpsize"]`, experiments | `voice-deep.md` §1 | P4 |
| F-VOICE-05 | Session Description | Ключ и режим | op4: `mode`, `secret_key` (32 Б), `media_session_id`, `dave_protocol_version:1`, `secure_frames_version:1` | `voice-deep.md` §1 | P4 |
| F-VOICE-06 | UDP discovery | NAT-адрес | 74 Б: type 1/2, ssrc, ASCII-адрес, порт BE @72; повтор до Ready | `voice-deep.md` §2.1 | P4 |
| F-VOICE-07 | Keepalive | Держать NAT | 8 Б: `1337cafe`/`1337f00d` каждые 5 с | `voice-deep.md` §2.2 | P4 |
| F-VOICE-08 | RTP Opus | Речь | PT 120, 20 мс, 48 кГц стерео; ext 0xBEDE (2 слова); seq/ts | `voice-deep.md` §2.3 | P4 |
| F-VOICE-09 | SRTP-крипта | Шифрование медиа | GCM-тег 16 Б + хвостовой 4-Б LE-счётчик (клиент `0x80000000|n`, сквозной RTP+RTCP); AAD = заголовок; режимы 0–6 | `voice-deep.md` §2.3–2.4 | P4 |
| F-VOICE-10 | RTCP | Качество связи | SR 60 Б ~5,2 с, RR сервера 52 Б 1/с; пустой RR 28 Б после ротации; тела шифрованы → своя статистика | `voice-deep.md` §2.4 | P4 |
| F-VOICE-11 | DTX/CN | Экономия полосы | При тишине ~10 пакетов (200 мс), затем CN; пауза не рвёт соединение | `voice-deep.md` §2.3 | P4 |
| F-VOICE-12 | Ротация сокета | Устойчивость NAT | Ровно +60,08 с: `socket()` → discovery (тот же ssrc) → новый порт → пустой RR | `voice-deep.md` §2.5 | P4 |
| F-VOICE-13 | Mute/Deafen/Speaking | Контроль | setSelfMute/Deafen; speaking callback → индикатор; эхо и жёлтая рамка | `voice-deep.md` §0/§4 | P4 |
| F-VOICE-14 | PTT | Хоткей-речь | Глобальный хоткей, режимы «по кнопке/активности», приоритет; idle-детект для активности | `backend-ipc.md`, R1 | P4 |
| F-VOICE-15 | Устройства | Гибкость | Выбор in/out, AGC/AEC/NS (38 тумблеров вкладки), hot-plug и перезапрос | `frontend-d03.md` §4.2 | P4 |
| F-VOICE-16 | Шумоподавление | Чистота звука | Аналог krisp: модели `.kef` (5 шт.), уровни; в Rust — RNNoise/свой инференс | `native-re.md` §4 | P4 |
| F-VOICE-17 | DAVE/MLS (E2EE) | Приватность голоса | `dave_protocol_version:1`, `secure_frames_version:1`; MLS-фрейминг поверх медиа (Cisco mlspp — референс); пороговые версии участников | `voice-deep.md` §1, `native-re.md` §3.5 | P4.5 |
| F-VOICE-18 | Регионы/латентность | Лучший сервер | RTCLatencyTestManager: UDP-зонды по регионам, ранжирование (fra/arn/…), выбор первого | `voice-deep.md` §1 | P4 |
| F-VOICE-19 | Статистика связи | Диагностика | RTT через op3/op6, потери/джиттер из RTCP (нужен свой парсинг после расшифровки) | `voice-deep.md` §2.4 | P4 |
| F-VOICE-20 | Soundboard | Звуки в канал | SOUNDBOARD_SOUNDS (21), воспроизведение всем участникам | `gateway-protocol.md` §4 | P6 |
| F-VOICE-21 | Видео в звонке | Камера | streams rid/quality/active; видео-ssrc + rtx_ssrc | `voice-deep.md` §1 | P5 |

## MEDIA — видео и трансляция

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-MEDIA-01 | Камера | Видеозвонки | Захват → энкод → RTP; превью локально | `voice-deep.md` §1 | P5 |
| F-MEDIA-02 | Go Live | Демонстрация экрана | Выбор источника, quality `rid:100`, зрители через SFU | `voice-deep.md` §1 | P5 |
| F-MEDIA-03 | Захват экрана | Linux-путь | PipeWire (Wayland) / X11; окно/монитор/регион; выбор через системный диалог | `native-re.md` §2.2 (pipewire dlopen) | P5 |
| F-MEDIA-04 | Системный звук стрима | Звук при демонстрации | Аналог venmic (PipeWire monitor); R5-бэклог | `feature-backlog.md` R5-* | P5 |
| F-MEDIA-05 | GPU-энкод | Производительность | У Discord: Nvidia cuda+vulkan (h264/h265/av1), AMD vaapi (h264), Intel — нет; в Rust — vaapi/vulkan/nvenc | README (GPU-кодировщики) | P5 |
| F-MEDIA-06 | Плеер вложений | Просмотр медиа | Видео/GIF/аудио в чате, тултипы размеров | `frontend-d03.md` §5 | P3 |
| F-MEDIA-07 | Клипы | Нарезка (необязательно) | Официально — внешний `discord_clips` exe; в Rust — опционально, низкий приоритет | `main-arch.md` §3.5 | P7 |

## UI — интерфейс

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-UI-01 | Каркас окна | Основной экран | Тема dark/light, плотности cozy/compact, min-size, frameless/тайтлбар по платформе | `frontend-d03.md` §6 | P3 |
| F-UI-02 | Гильд-рейл | Навигация | Иконки/папки/бейджи, порядок, drag-сортировка | `frontend-d03.md` §3 | P3 |
| F-UI-03 | Дерево каналов | Навигация | Категории, коллапс, непрочитанные, голосовые участники, треды | `frontend-d03.md` §3 | P3 |
| F-UI-04 | Чат-вью | Основная работа | Виртуализированный список, группировка по автору, день-разделители, embeds/реакции/hover-меню | `frontend-d03.md` §5.4–5.12 | P3 |
| F-UI-05 | Композер | Ввод | Текст, эмодзи-автокомплит, mentions, upload, кнопки вложений | `frontend-d03.md` §2 | P3 |
| F-UI-06 | Список участников | Соц. функции | Виртуализация до 24k, группы роли/статусы, поиск по участникам | `gateway-protocol.md` §5 | P3 |
| F-UI-07 | ЛС/друзья | Личные чаты | Список, активность, запросы, группы | `frontend-d03.md` §3 | P3 |
| F-UI-08 | Оверлей поиска | Найти всё | Фильтры, результаты, сортировка | `frontend-d03.md` §1 | P3 |
| F-UI-09 | Эмодзи-пикер | Выразительность | 3 вкладки, 10 категорий, 50 видимых, «часто используемые», поиск | `frontend-d03.md` §2 | P3 |
| F-UI-10 | Настройки | Управление | Категории + «Голос и видео» (38 тумблеров), язык, клавиши, уведомления | `frontend-d03.md` §4 | P3 |
| F-UI-11 | Поповер пользователя | Профиль | Аватар/баннер, bio, роли, активности, заметка | `frontend-d03.md` (popover) | P3 |
| F-UI-12 | Меню Mana | Действия | Пункты colorDefault/danger, чекбоксы, радио, субменю; Escape/стрелки | `frontend-d03.md` §5.3 | P3 |
| F-UI-13 | Модалки | Диалоги | Фокус-ловушка, aria, анимации, закрытие | `frontend-d03.md` §5.5 | P3 |
| F-UI-14 | Тултипы | Подсказки | Обычные + mini-premium | `frontend-d03.md` §5.7 | P3 |
| F-UI-15 | Тумблеры/слайдеры | Настройки | switch (react-aria-совместимый), слайдеры с marks | `frontend-d03.md` §5.8–5.9 | P3 |
| F-UI-16 | Аватары/статус-рамки | Соц. идентичность | Стеки, ring, grayscale для offline | `frontend-d03.md` §5.10 | P3 |
| F-UI-17 | Бейджи | Уведомления | Number/icon/premium, badge сервера | `frontend-d03.md` §5.11 | P3 |
| F-UI-18 | Скроллеры | Большие списки | Thin/auto/none, gutter, виртуализация | `frontend-d03.md` §5.12 | P3 |
| F-UI-19 | Дизайн-токены | Стиль | 5132 переменные → токены; маппинг 12 контрактов; темы/плотности | `frontend-d03.md` §6 | P3 |
| F-UI-20 | Звуки интерфейса | Обратная связь | 15 mp3 (join/leave/mute/message/ring) | R1 (ассеты кэша) | P3 |
| F-UI-21 | Шорткаты | Скорость | Ctrl+K (QuickSwitcher), Ctrl+F (поиск), Ctrl+E (эмодзи), mod+f | `frontend-d03.md` §5.4 | P3 |
| F-UI-22 | Слои/поповеры | Правильное наложение | positionLayer, клик-аутсайд, scroll-lock, фокус | `frontend-d03.md` §5.6 | P3 |
| F-UI-23 | Доступность | Инклюзивность | role=dialog/menu, aria-label(s), фокус-порядок | `frontend-d03.md` §5.5 | P3 |
| F-UI-24 | Анимации | Живость | Переходы модалок/меню, loading spinner, fade | `frontend-d03.md` §5 | P3 |

## I18N — локализация

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-I18N-01 | Рантайм сообщений | Локализация | Собственный AST (не IntlMessageFormat); выбор по локали | `i18n-deep.md` §1 | P3 |
| F-I18N-02 | Ключ-функция | Адресация строк | `xxh64(LE-байты строки) → base64 → 6 символов` (36 бит) | `i18n-deep.md` §0 | P3 |
| F-I18N-03 | Плурализация | Грамматика | CLDR (one/few/many/other) + точные `=N`; демо ru: 1/2/5/11/21/22/101 | `i18n-deep.md` §2 | P3 |
| F-I18N-04 | Rich-разметка | Форматирование | 23 правила (bold/italic/underline/strike/code/link/emoji/user/channel/…), экранирование | `i18n-deep.md` §3 | P3 |
| F-I18N-05 | Форматы данных | Даты/числа | Разделители, даты/время, «N участников» | `i18n-deep.md` §5–6 | P3 |
| F-I18N-06 | Локали и fallback | 36 языков | 31 из 36 файлов в сборке; fallback en-US; системная локаль; смена на лету | `i18n-deep.md` §7 | P7 |
| F-I18N-07 | Свой каталог строк | Наши тексты | Отдельные строки приложения (не копировать чужие ассеты) | план | P3 |

## SYS — платформенные функции

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-SYS-01 | Трей | Фоновый режим | Иконка/меню/состояния, закрытие в трей, мигание непрочитанного | `backend-ipc.md` (SYSTEM_TRAY_*) | P6 |
| F-SYS-02 | Бейдж лончера | Счётчик | `APP_SET_BADGE_COUNT`/dock-bounce (Linux: Unity launcher API) | `backend-ipc.md` | P6 |
| F-SYS-03 | Уведомления | Внимание | D-Bus notify с действиями (открыть/ответить); на Linux официальный клиент не поддерживает — делаем своё | `main-arch.md` §3.2 | P6 |
| F-SYS-04 | Deep links | Интеграция | `discord://` → открытие канала/сообщения; single-instance | `main-arch.md` | P6 |
| F-SYS-05 | Автозапуск | Старт с системой | XDG autostart, флаг «открывать свёрнутым» | `backend-ipc.md` (OPEN_ON_STARTUP) | P6 |
| F-SYS-06 | Обновление | Доставка версий | Свой updater: манифест, подпись, staged-замена; без Electron-updater | `main-arch.md` §5 | P8 |
| F-SYS-07 | Буфер обмена | Удобство | Копировать текст/картинки; вставка изображения → загрузка | `backend-ipc.md` (CLIPBOARD_*) | P6 |
| F-SYS-08 | Блокировка сна | Звонок не рвётся | ScreenSaver/Login1 inhibitor во время звонка/стрима | `backend-ipc.md` (POWER_SAVE_*) | P6 |
| F-SYS-09 | Idle-детект | Авто-статус | Wayland idle (ext-idle-notify) / X11; idle → статус | `backend-ipc.md` (POWER_MONITOR_*) | P6 |
| F-SYS-10 | Spellcheck | Грамотность | Hunspell-словари системы (аналог `cld.node`); подчёркивание/замена | `native-re.md` §6 | P6 |
| F-SYS-11 | Клипы (файлы) | Просмотр клипов | В официальном клиенте — внешний exe; в Rust — опционально | `main-arch.md` §3.5 | P7 |
| F-SYS-12 | Мультиокна | Попауты | Отдельные окна для каналов/видео (popout) | `main-arch.md` §2.3 | P7 |
| F-SYS-13 | Win/macOS | Кроссплатформа | После Linux: WASAPI/CoreAudio, трей, хоткеи | план | P7 |

## SEC — безопасность и приватность

| ID | Функция | Зачем | Как работает (RE-факты) | Источник | Фаза |
|---|---|---|---|---|---|
| F-SEC-01 | Секреты только в keyring | Защита аккаунта | Никаких токенов в файлах; очистка при выходе | политика | P1 |
| F-SEC-02 | Редакция логов | Безопасные баг-репорты | Фильтр по ключам (token/session/secret/email) + паттернам | `sanitize-jsonl.mjs` (лаборатория) | P1 |
| F-SEC-03 | Телеметрия off | Приватность | science/beaker не отправляются | `rest-d02.md` §2 | P1 |
| F-SEC-04 | Отпечаток клиента | Управляемая идентичность | Супер-свойства конфигурируемы; честные значения по умолчанию | `rest-d02.md` §6.2 | P1 |
| F-SEC-05 | Подпись обновлений | Целостность | Ed25519-подпись манифеста и бинаря | план | P8 |
| F-SEC-06 | Без DRM/обхода | Правовая чистота | Видео/аудио — только официальные CDN-потоки | README (правовая рамка) | — |
