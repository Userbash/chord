# Доска задач миграции на GPUI Kit — Волна RW-42 (`waves/RW-42-GPUI-MIGRATION/TASKS-GPUI.md`)

## Обзор волны
- **Эпик:** Полный перенос Discord-клиента с гибридного стека Tauri (WebKit + JS) на нативный Rust GPU фреймворк `gpui-kit`.
- **Фокус:** Радикальное сокращение самописного кода, замена велосипедов стандартными библиотеками, стабильные 120 FPS, 0 ошибок памяти и DOM-коллизий.
- **Главный архитектор:** Chief Software Architect (Tech Lead)
- **Отделы-исполнители:** G1 Backend, G2 GPUI Frontend, G3 Design, G4 QA, G5 Voice/Platform, DevOps / PM Review.

---

## 1. Задачи приоритета P0 — Архитектурный фундамент и Базовый каркас

| ID | Название задачи | Отдел / Ответственный | Библиотеки замены | Описание и DoD | Статус |
|---|---|---|---|---|---|
| **TASK-701** | Создание крейта `discord-gpui` и интеграция `gpui-kit` | **Chief Architect** + **DevOps** | `gpui-kit = { path = "../../gpui-kit/crates/kit" }` | Инициализация рабочего крейта `rust/discord-gpui`. Настройка `Cargo.toml`, базовый цикл `gpui::App::new().run(...)`, открытие главного окна 1280x720 с GPU-ускорением. `cargo check` успешен. | `Ready for Merge` |
| **TASK-702** | Реактивная модель состояния `AppState` в GPUI | **Chief Architect** + **G1 Backend** | `gpui::Context`, `gpui::Entity`, `async-channel`, `tokio` | Создание корневого `AppState` (`CurrentUser`, `GuildTree`, `ActiveChannel`, `MessageFeed`). Интеграция связи с главным UI-потоком через `cx.listener` / `cx.notify`. Тест реактивного обновления. | `Ready for Merge` |
| **TASK-703** | Портирование темы Discord Dark в систему стилей GPUI | **G3 Design Lead** | `DISCORD_THEME`, `DiscordThemeColors`, `gpui::{rgb, rgba, Hsla}` | Создание темы `DiscordThemeColors`: токены фона (`#313338`, `#2b2d31`, `#1e1f22`), акцентный цвет (`#5865f2`), статусная палитра (`online`, `idle`, `dnd`, `offline`), типографика. Соответствие WCAG 2.2 AA. | `Ready for Merge` |
| **TASK-704** | REST v9 Клиент на базе `discord-core` / `reqwest` с автоматическим бэкоффом | **G1 Backend Lead** | `discord-core::rest`, `discord-core::state` | Интеграция проверенного клиента `discord-core` с пулом соединений и автоматической обработкой `HTTP 429 Too Many Requests` (`retry_after`). | `In Progress` |
| **TASK-705** | Headless тестовый харнесс GPUI (`gpui_contract_test`) | **G4 QA Lead** | `tests/gpui_contract_test.rs` | 11/11 пройденных контрактов: переходы гильдий, очистка unread, отправка сообщений, hCaptcha (40003), инварианты звука mute/deafen, SQLite хранилище, IP Discovery 74B, Keepalive 8B, AES-256-GCM шифрование, сайдбар участников, диалог настроек звука, жизненный цикл голосового звонка, создание сервера. 100% pass. | `Accepted` |
| **TASK-705B** | Локальная база данных SQLite вместо самописного `data.rs` | **G1 Backend Lead** | `rusqlite = "0.40.2"` | Встраиваемая база данных `LocalDatabase` (WAL-режим): таблицы гильдий, каналов, сообщений с индексами и настроек. Заменяет 10 850 строк `data.rs`. Покрыта контрактом. | `Ready for Merge` |

---

## 2. Задачи приоритета P1 — Основной пользовательский интерфейс и Чат

| ID | Название задачи | Отдел / Ответственный | Библиотеки замены | Описание и DoD | Статус |
|---|---|---|---|---|---|
| **TASK-706** | Трехпанельный макет приложения `AppLayout` | **G2 Frontend Lead** | `gpui-kit`, `AppLayout::render` | Замена самописных Flexbox-колонок на нативный трехпанельный макет GPUI: левый `ServerRail` (72px), средний `ChannelSidebar` (240px), центральный `ChatView` (flex-1). | `Ready for Merge` |
| **TASK-707** | Колонка серверов (`ServerRail`) и папки | **G2 Frontend Lead** | `ServerRail`, `DISCORD_THEME` | Отображение иконок серверов с индикатором активного сервера (белая полоска слева), бейджи непрочитанных сообщений, кнопка Добавить сервер (+) и тест hCaptcha (40003). | `Ready for Merge` |
| **TASK-708** | Список каналов гильдии (`ChannelSidebar`) | **G2 Frontend Lead** | `ChannelSidebar`, `DISCORD_THEME` | Категории каналов с иконками (#, 🔊, 📢), различие типов каналов, нижняя плашка профиля текущего пользователя с микрофоном/наушниками/шестерёнкой. | `Ready for Merge` |
| **TASK-709** | Виртуальный скроллинг чата (`ChatView`) на 100k сообщений | **G2 Frontend Lead** + **G1 Backend** | `ChatView`, `DISCORD_THEME` | Полная замена самописного JS-виртуализатора на нативный GPUI рендеринг ленты сообщений. Мгновенная отрисовка, плавная прокрутка, O(1) потребление памяти. | `Ready for Merge` |
| **TASK-710** | Рендеринг текста сообщений с Markdown и реакциями | **G2 Frontend Lead** + **G3 Design** | `ChatView`, `MessageReaction` | Нативный рендеринг заголовков сообщений, аватарок авторов, временных меток и интерактивных чипов реакций (🔥, 🚀, 🦀, 🎉). | `Ready for Merge` |
| **TASK-711** | Поле ввода сообщений (`MessageBar`) и быстрый ввод | **G2 Frontend Lead** | `ChatView::render`, `btn-quick-send` | Поле ввода текста с плейсхолдером под текущий канал, кнопка прикрепления (+), смайлики (🎁, 👾, 😊) и мгновенная отправка по кнопке / Enter. | `Ready for Merge` |

---

## 3. Задачи приоритета P2 — Модальные окна, Капча, Настройки и Участники

| ID | Название задачи | Отдел / Ответственный | Библиотеки замены | Описание и DoD | Статус |
|---|---|---|---|---|---|
| **TASK-712** | Нативное модальное окно решения hCaptcha (`CaptchaModal`) | **G2 Frontend Lead** + **Chief Architect** | `CaptchaModal`, `gpui::rgba(0x000000b3)` | При получении ошибки `40003` от Discord открывается нативное модальное окно GPUI поверх интерфейса. Никаких утечек фокуса и сбоев стилей! Кнопка теста 🛡️ на серверной панели позволяет проверить модалку в один клик. | `Ready for Merge` |
| **TASK-713** | Диалог вступления и создания сервера (`CreateGuildModal`) | **G2 Frontend Lead** | `CreateGuildModal` | Модальное окно создания сервера с выбором шаблонов (Игры, Друзья), добавлением сервера в реактивное состояние `AppState` и переключением на него. | `Ready for Merge` |
| **TASK-714** | Список участников гильдии (`MemberSidebar`) | **G2 Frontend Lead** | `MemberSidebar` | Виртуализированный сайдбар участников, группировка «В СЕТИ» и «НЕ В СЕТИ», статус-точки на аватарках, кнопка переключения видимости (👥) в чате. | `Ready for Merge` |
| **TASK-715** | Окно настроек пользователя («Голос и видео») | **G2 Frontend Lead** + **G3 Design** | `SettingsDialog` | Экран настроек: перечисление устройств ввода/вывода через `cpal`, ползунки громкости 0–100% и 0–200%, переключатели шумоподавления (Krisp/RNNoise) и эхоподавления. | `Ready for Merge` |
| **TASK-716** | Нативное контекстное меню (ПКМ) | **G2 Frontend Lead** | `gpui-kit::component::menu` | Вызов контекстного меню по правому клику мыши на сообщении (Копировать текст, Закрепить, Ответить, Удалить) и на пользователе. | `Ready for Development` |

---

## 4. Задачи приоритета P3 — Медиа, Голос, Системная интеграция и Финальная полировка

| ID | Название задачи | Отдел / Ответственный | Библиотеки замены | Описание и DoD | Статус |
|---|---|---|---|---|---|
| **TASK-717** | Нативный аудио-движок ввода/вывода на `cpal` и крипто-протокол звонков | **G5 Voice Lead** | `cpal = "0.15.3"`, `aes-gcm = "0.11.1"` | Прямой захват микрофона и воспроизведение звука через `cpal`. IP Discovery (74B), Keepalive (8B 0x1337cafe) и AEAD AES-256-GCM шифрование RTP-пакетов по спецификации `re-deep/voice-deep.md`. Протестировано 100%. | `Ready for Merge` |
| **TASK-718** | Голосовая панель подключения (`VoicePanel`) и индикатор звонка | **G5 Voice Lead** + **G2 Frontend** | `VoicePanel` | Плашка «Голос подключён» над профилем в `ChannelSidebar`: пинг 18 мс, RTC эндпоинт, кнопка отключения от звонка (📞❌), реактивная сессия в `AppState`. | `Ready for Merge` |
| **TASK-719** | Системный трей и нативные уведомления | **G5 Platform Lead** | `gpui-kit::tray-icon`, `notify-rust` / `ashpd` | Иконка в системном трее с меню (Показать/Скрыть, Статус, Выход), отправка нативных уведомлений рабочего стола при личных сообщениях и упоминаниях. | `Ready for Development` |
| **TASK-720** | Финальная сертификация сборки и мутационный гейт 25/25 | **G4 QA Lead** + **PM Review** | `scripts/ci.sh`, `gpui-test-runner` | Полный цикл верификации: 100% покрытие контрактов, 25/25 мутаций уничтожено, замер памяти < 60 MB RAM, замер FPS = 120, сборка `release` бинарника и SHA256SUMS. | `Ready for Development` |
| **TASK-721** | Устранение наложения и коллизии `VoicePanel` на профиль пользователя (Стрелка 2) | **G3 Design** + **G2 Frontend** | `VoicePanel`, `ChannelSidebar` | Жёсткая фиксация высоты 52px, однострочный вывод «🔊 Канал / Сервер» с `text_ellipsis()`, запрет многострочного переноса, `overflow_hidden()` на всех контейнерах, скрытие голосовой панели при открытом окне прямого звонка. | `Accepted` |
| **TASK-722** | Полнофункциональное модальное окно создания и вступления на сервер по кнопке `+` (Стрелка 1) | **G1 Backend** + **G2 Frontend** | `CreateGuildModal`, `CreateServerStep` | Трёхэтапный визард: 1) Шаблоны («Свой шаблон», «Для меня и друзей», «Игры и стримы»), 2) Персонализация названия, 3) Вступление по коду/ссылке (`discord.gg/...`). Обработчики `create_guild` и `join_guild_by_invite`. | `Accepted` |
| **TASK-723** | Двусторонняя отправка сообщений через официальный REST API v9 | **G1 Backend Lead** | `send_live_message_blocking`, `dm_channels` | Таблица маппинга `dm_channels` (`recipient_id -> snowflake_channel_id`), отправка реальных `POST /channels/{id}/messages` в живой Discord. Доставка сообщений собеседнику `ShadowByte`. | `Accepted` |
| **TASK-724** | Экран обзора публичных сообществ `ExploreView` (Кнопка 🧭) | **G2 Frontend** + **G3 Design** | `ExploreView`, `ServerRail` | Discover-экран по эталону официального клиента: баннер Hero, табы категорий («Главная», «Игры», «Музыка»), карточки популярных сообществ (Marvel Rivals, Midjourney, Genshin) с кнопкой вступления. | `Accepted` |
| **TASK-725** | Расширение тестового харнесса до 48/48 контрактов | **G4 QA Lead** | `gpui_contract_test.rs` | 48/48 контрактов (100% pass): шаги модалки сервера, вступление по ссылке, обзор сообществ, супрессия панели звонка, 120 FPS бенчмарк без блокирующих сетевых тайм-аутов в мок-режиме. | `Accepted` |
| **TASK-726** | Устранение перехвата событий неактивными модальными окнами в `AppLayout` | **G2 Frontend Lead** | `layout.rs`, `.children(if open { Some(...) } else { None })` | Замена безусловного добавления 13 пустых элементов модалок на условный рендеринг `.children(...)`. Добавление клик-ту-клоуз на оверлей `modal-create-guild-overlay` и поглощение событий внутри окна. | `Accepted` |
| **TASK-727** | Загрузка и синхронизация реальных каналов серверов (`GET /guilds/{id}/channels`) | **G1 Backend Lead** | `net::fetch_live_discord_account`, `state::join_guild_by_invite` | Получение каналов и категорий для гильдий аккаунта, автоматическая привязка `SISI 18+ PROJECT` (24 канала) в `ServerRail` и `ChannelSidebar`. Устранение дубликатов гильдий при повторном вступлении. | `Accepted` |
| **TASK-728** | Расширение тестового харнесса до 53/53 контрактов с верификацией живого инвайта | **G4 QA Lead** | `gpui_contract_test.rs` | 53/53 контрактов (100% pass): проверка сквозного присоединения к серверу через живой токен и инвайт-код, валидация закрытия модалки и обновления дерева каналов. | `Accepted` |
| **TASK-729** | Загрузка и кэширование сообщений гильдийных каналов через REST API (`GET /channels/{id}/messages`) | **G1 Backend Lead** | `net::fetch_channel_messages`, `AppState::select_channel` | Устранение пустоты чата при переключении на серверные каналы. Подгрузка истории сообщений (limit=50) с парсингом авторов, аватарок и временных меток. | `Accepted` |
| **TASK-730** | Динамическое обнаружение и наполнение участников сервера из авторов сообщений (`MemberSidebar`) | **G1 Backend Lead** | `AppState::select_channel`, `MemberSidebar` | Обход ограничения HTTP 403 при запросе `/guilds/{id}/members` для пользовательских токенов. Авто-обнаружение уникальных авторов сообщений и вывод реальных участников в сайдбаре. | `Accepted` |
| **TASK-731** | Синхронизация выбора каналов гильдии и автоматический выбор первого текстового канала | **G2 Frontend Lead** | `AppState::select_guild` | Устранение зависания `selected_channel_id` в "dm-friends" при клике на сервер в `ServerRail`. Авто-переход на первый доступный канал сервера с немедленной подгрузкой чата. | `Accepted` |
| **TASK-732** | Полнофункциональный интерактивный ввод ссылки приглашения в `CreateGuildModal` (Стрелка 1) | **G2 Frontend Lead** | `CreateGuildModal::render_join_step`, `handle_key_input`, `paste_text` | Устранение проблемы «нельзя вбить сысслку приглос». Реальное редактируемое поле ввода с курсором `|`, стиранием через Backspace, кнопкой быстрой вставки из буфера 📋 и очистки ✕. | `Accepted` |
| **TASK-733** | Устранение наложения и фейкового статуса голосового подключения `VoicePanel` (Стрелка 2) | **G5 Voice Lead** + **G2 Frontend** | `VoicePanel`, `channel_sidebar.rs` | Скрытие плашки `VoicePanel` при отсутствии активной сессии (`voice_session = None`), устранение коллизии на профиль пользователя, мгновенное отключение по кнопке 📞 или повторному клику на канал. | `Accepted` |
| **TASK-734** | Расширение тестового харнесса до 55/55 контрактов GPUI | **G4 QA Lead** | `gpui_contract_test.rs` | 55/55 контрактов (100% pass): валидация циклического подключения/отключения голоса, посимвольного ввода ссылки приглашения, бэкспейса, вставки из буфера и очистки поля. | `Accepted` |
| **TASK-735** | Интеграция реального подключения к серверу через `POST /invites/{code}` | **G1 Backend Lead** | `net::join_live_discord_invite` | Сквозной вызов официального REST API Discord для присоединения к серверу по ссылке или коду, получение объекта `guild` и выбор нового сервера. | `Accepted` |
| **TASK-736** | Управление жизненным циклом голосовой сессии и интеграция аудио-устройств CPAL | **G5 Voice Lead** | `audio::mod.rs`, `VoiceSessionState` | Прямое управление устройствами захвата/воспроизведения звука, переключение mute/deafen без сбоев интерфейса. | `Ready for Merge` |
| **TASK-737** | Стилизация и паритет модального окна добавления сервера по стандартам Discord Dark | **G3 Design Lead** | `CreateGuildModal`, `DISCORD_THEME` | Точные отступы, брейкпоинты, цветовые токены фона `#2b2d31`, акцент `#5865f2` и типографика для всех шагов создания и вступления на сервер. | `Accepted` |
| **TASK-738** | Изоляция контейнеров нижней панели сайдбара и профиля пользователя | **G2 Frontend Lead** | `channel_sidebar.rs`, `render_user_profile_bar` | Строгий `overflow_hidden()`, разделение `VoicePanel` (52px) и `UserProfileBar` (52px), гарантия непересекающихся слоёв и корректный z-index. | `Accepted` |
| **TASK-739** | Отображение реального профиля аутентифицированного пользователя вместо тестовых моков | **G1 Backend** + **G2 Frontend** | `AppState::new_live`, `channel_sidebar.rs` | Отображение реального Discord-юзера из сохранённого токена (`~/.config/discord-gpui/token.json`) с кнопкой «Выход» (🚪). | `Accepted` |
| **TASK-740** | Автоматизированная регрессионная верификация сборки и релиз бинарника | **G4 QA Lead** + **DevOps** | `target/release/discord-gpui` | Полный прогон тестовой сюиты (55/55 pass за 7.67с), компиляция релизного исполняемого файла без предупреждений о сбоях компиляции. | `Accepted` |
| **TASK-741** | Реальные REST API действия с сообщениями (Удаление, Закрепление, Открепление) | **G1 Backend Lead** | `net::{delete,pin,unpin}_live_message`, `discord-core::rest` | Вызовы `DELETE /channels/{id}/messages/{id}`, `PUT /channels/{id}/pins/{id}`, `DELETE /channels/{id}/pins/{id}`. Оптимистичное обновление UI и неблокирующая отправка на сервер. Контекстное меню ПКМ переключает «Закрепить/Открепить». | `Accepted` |
| **TASK-742** | Прямой Voice UDP Media Streamer (`tokio::net::UdpSocket` + Opus + AES-256-GCM) | **G5 Voice Lead** | `VoiceUdpStreamer`, `Serein::transport.rs`, `discord-voice::udp` | Привязка UDP-сокета к голосовому шлюзу Discord. Выполнение IP Discovery (74B), периодические Keepalive (8B 0x1337cafe) и упаковка 20мс Opus-пакетов в RTP с AEAD шифрованием. | `Accepted` |
| **TASK-743** | Демонстрация экрана через Linux XDG Desktop Portal и PipeWire (`ashpd` / D-Bus) | **G5 Platform Lead** | `ScreenCastManager`, `discord-platform::screencast` | Интеграция `org.freedesktop.portal.ScreenCast`: создание сессии, выбор источников (монитор / окно), согласование курсора (Embedded/Metadata) и получение PipeWire remote fd. | `Accepted` |
| **TASK-744** | Нативные D-Bus уведомления рабочего стола и системный трей Linux | **G5 Platform Lead** | `DesktopNotificationManager`, `SystemTrayManager`, `notify-rust` / `discord-platform::tray` | Отправка уведомлений `org.freedesktop.Notifications` при входящих сообщениях и упоминаниях. Системный трей с 6-пунктовым меню Discord и бейджем количества непрочитанных сообщений. | `Accepted` |
| **TASK-745** | Сервер локального бинарного протокола Discord Local RPC IPC (`discord-ipc-0..9`) | **G1 Backend Lead** | `LocalRpcIpcServer`, `rust-discord-activity`, `discord-platform::rpc_ipc` | Поддержка `HANDSHAKE` (Opcode 0), `FRAME` (Opcode 1) с полезной нагрузкой `SET_ACTIVITY` для игр и внешних приложений. Обновление статуса активности пользователя. | `Accepted` |
| **TASK-746** | Автоматизированный регрессионный контрактный харнесс 61/61 (100% Pass) | **G4 QA Lead** + **DevOps** | `tests/gpui_contract_test.rs` | 61/61 контрактных тестов GPUI: REST удаление и закрепление сообщений, UDP голосовой сокет и Keepalive, Screencast портал опции, D-Bus уведомления, меню трея и RPC фреймы. | `Accepted` |

---

## 5. График выполнения и Zero-Idle ротация

```text
Спринт 1 (Фундамент):
  [TASK-701: Crate GPUI] ──► [TASK-702: AppState] ──► [TASK-703: Theme] ──► [TASK-704: REST reqwest] ──► [TASK-705: QA Test Harness]

Спринт 2 (Основной UI):
  [TASK-706: AppLayout] ──► [TASK-707: ServerRail] ──► [TASK-708: ChannelSidebar] ──► [TASK-709: VirtualList Chat] ──► [TASK-710: Markdown TextView]

Спринт 3 (Интерактив и Модалки):
  [TASK-711: MessageBar] ──► [TASK-712: Captcha WebView] ──► [TASK-713: Server Dialogs] ──► [TASK-714: MemberSidebar] ──► [TASK-715: Settings]

Спринт 4 (Медиа и Релиз):
  [TASK-716: ContextMenu] ──► [TASK-717: CPAL Audio] ──► [TASK-718: Voice Panel] ──► [TASK-719: Tray & Notifs] ──► [TASK-720: Final Certification]
```
