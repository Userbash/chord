# Архитектура Rust-клиента: процессы, крейты, потоки данных

Документ описывает проектную архитектуру нативного клиента: модель процессов, разбиение на
крейты, задачно-событийную модель, восемь графов (Mermaid), хранилище, замену Electron-IPC
нативными модулями, ADR по UI-тулкиту и бюджеты производительности. Источники фактов — `../re/`.

## 1. Общая модель

- **Один основной процесс** (UI + ядро). Опционально — **медиа-процесс** для изоляции сбоев
  кодеков/энкодеров (по образцу того, что Discord держит `discord_voice.node` в рендерере, но с
  чистым разделением).
- Вместо 168 IPC-каналов Electron (`../re/backend-ipc.md`) — **внутренняя шина**
  (`tokio::broadcast` для событий, `mpsc` для команд) и **сервисы-модули** в одном адресном
  пространстве.
- Событийная модель: сервер → шлюз → диспатчи → **редьюсер состояния** → подписчики (UI, хранилище,
  звуки/уведомления, счётчики). Действия UI → REST/шлюз → **оптимистичное обновление** → подтверждение.

```mermaid
flowchart LR
    subgraph RP["Основной процесс"]
        UI["discord-ui (Tauri, web-фронтенд)"]
        CORE["discord-core<br/>gateway + rest + state"]
        STORE["discord-store (SQLite)"]
        I18N["discord-i18n"]
        PLAT["discord-platform<br/>трей, уведомления, хоткеи"]
        UI <-->|"события/команды (канal)"| CORE
        CORE <--> STORE
        UI --- I18N
        CORE --> PLAT
    end
    subgraph MP["Медиа-процесс (опционально)"]
        VOICE["discord-voice<br/>Opus + SRTP + DAVE"]
        MEDIA["discord-media<br/>видео, захват, энкод"]
        VOICE <-->|"IPC (пакеты/кадры)"| MEDIA
    end
    CORE <-->|"IPC: управление, статистика"| VOICE
    GW[("Discord Gateway<br/>wss + zstd + ETF")]
    API[("REST /api/v9")]
    RTC[("RTC WS + UDP<br/>голос/видео")]
    CORE <-->|"WS"| GW
    CORE <-->|"HTTPS"| API
    VOICE <-->|"WS + UDP"| RTC
```

## 2. Крейты (workspace)

```mermaid
flowchart TD
    APP["discord-app (бинарь)<br/>сборка зависимостей, настройки запуска"]
    UI["discord-ui<br/>экраны, виджеты, токены"]
    CORE["discord-core<br/>шлюз, REST, редьюсер, действия"]
    ENC["discord-codec<br/>ETF, zstd-stream, типы REST"]
    STORE["discord-store<br/>SQLite, миграции, кэш"]
    VOICE["discord-voice<br/>сигнализация, UDP, SRTP, Opus"]
    MEDIA["discord-media<br/>видео, захват, энкодеры"]
    PLAT["discord-platform<br/>трей, уведомления, deep links, idle"]
    I18N["discord-i18n<br/>рантайм, ключи, плюрализация"]
    CONF["discord-conformance (dev)<br/>реплей RE-фикстур"]
    APP --> UI
    APP --> CORE
    APP --> VOICE
    APP --> MEDIA
    APP --> PLAT
    UI --> I18N
    UI --> CORE
    CORE --> ENC
    CORE --> STORE
    VOICE --> CONF
    CORE --> CONF
```

Правила зависимостей: `discord-codec` — без I/O; `discord-core` не знает про UI; `discord-voice`
не зависит от состояния — получает параметры сессии из `discord-core`; UI общается только через
типизированные команды/события.

## 3. Задачно-событийная модель ядра

```mermaid
sequenceDiagram
    participant GS as gateway-task
    participant RD as reducer
    participant ST as store-task
    participant UI as ui-subscriber
    participant RT as rest-pool
    GS->>RD: Dispatch(s, op, payload)
    RD->>RD: обновление модели (гильдии, каналы, сообщения…)
    RD-->>ST: Persist(delta)
    RD-->>UI: Event(AppEvent)
    UI->>RT: Command(SendMessage{...})
    RT->>RT: POST /channels/{id}/messages
    RT-->>RD: Confirm(message)
    RD-->>UI: Event(MessageConfirmed)
```

- `gateway-task`: сокет, heartbeat (41,25 с), zstd-декодер, ETF-декодер, seq-tracking, reconnect
  (op7 → RESUME; при неудаче — новый IDENTIFY).
- `rest-pool`: ручной HTTP/1.1 на `tokio` + `tokio-rustls` (без `reqwest`), один запрос на соединение, обработка 429 (Retry-After), метрики; keep-alive — отдельная волна.
- `reducer`: единственный владелец изменяемой модели; чистая функция `(state, event) -> (state, effects)`.
- Эффекты: персист в SQLite, звук, уведомление, обновление заголовка окна/бейджа.
- UI-поток (главный): получает снапшоты/дельты, рендерит, шлёт команды.

## 4. Конвейер шлюза

```mermaid
flowchart LR
    WS["WS-фрейм (binary)"] --> Z["zstd-stream декодер<br/>контекст на сокет"]
    Z --> E["ETF-декодер<br/>(11 тегов, big-int)"]
    E --> OP{"op?"}
    OP -->|10| HELLO["HELLO → запустить heartbeat 41250 мс"]
    OP -->|11| ACK["ACK → RTT"]
    OP -->|7| RECON["RECONNECT → close → new socket → RESUME"]
    OP -->|0| DISP["Dispatch"]
    DISP --> T["типизация по t"]
    T --> RS["редьюсер состояния"]
    RS --> SUB["UI / store / звуки / бейдж"]
    RS -->|"t=READY"| LOAD["первичная загрузка<br/>гильдии, ЛС, users, read_state"]
```

Проверки конформанса: эталонные zstd+ETF кадры из `gateway/` (см. `TEST-PLAN.md`) прогоняются
через декодер и сверяются с JSON-оракулом.

## 5. Стек голоса

```mermaid
flowchart TD
    subgraph Сигнализация
        VS["VOICE_STATE_UPDATE (шлюз)"] --> VServ["VOICE_SERVER_UPDATE<br/>endpoint + token"]
        VServ --> RTC["RTC WS: Hello(8) → Identify(0) → Ready(2) → Session Description(4)"]
    end
    subgraph Транспорт
        DISC["UDP discovery 74 Б<br/>addr@31, port BE@72"] --> KA["Keepalive 8 Б<br/>1337cafe/1337f00d, 5 с"]
        KA --> ROT["Ротация сокета +60,08 с"]
    end
    subgraph Медиа
        MIC["Микрофон (cpal)"] --> NS["NS/AEC/AGC (RNNoise-совместимый)"]
        NS --> OPUS["Opus 20 мс, 48 кГц"]
        OPUS --> SRTP["SRTP-GCM (tag16 + tail4 LE)"]
        SRTP --> RTP["RTP PT120 (+ext 0xBEDE)"]
        RTP --> UDP["UDP к SFU"]
        UDP2["UDP от SFU"] --> DEJ["Джиттер-буфер"] --> DEC["Opus декод"] --> OUT["Выход (cpal)"]
        UDP -.->|"тот же сокет"| UDP2
    end
    subgraph E2EE["DAVE / MLS (v1)"]
        MLS["openmls: Welcome/Commit/App"] <--> FRAME["secure frames поверх медиа-канала"]
    end
    RTC --> DISC
    RTC --> MLS
```

Детали режимов шифрования (0–6), структура хвостового счётчика и AAD — в `../re/voice-deep.md`
§2; при реализации обязателен независимый тест на расшифровку эталонного SRTP-пакета.

## 6. Хранилище и миграции

```mermaid
erDiagram
    GUILDS ||--o{ CHANNELS : has
    GUILDS ||--o{ ROLES : has
    GUILDS ||--o{ MEMBERS : has
    CHANNELS ||--o{ MESSAGES : has
    USERS ||--o{ MESSAGES : writes
    CHANNELS ||--o{ READ_STATE : tracks
    GUILDS ||--o{ EMOJI : has
    SESSIONS }o--|| USERS : owns
```

- Таблицы: `guilds`, `channels`, `roles`, `members` (ленивые страницы), `users`, `messages`
  (индекс `channel_id, id`), `read_state`, `emoji`, `notes`, `settings`, `drafts`, `sessions`.
- Миграции — версионированные SQL-файлы, применяются на старте; WAL; размеры полей — по READY
  (крупная гильдия: 24 229 участников → отдельные страницы member-list).
- Секреты (токен) — **не в БД**: keyring.

## 7. Замена Electron-IPC нативными модулями

По `../re/backend-ipc.md` (168 каналов, whitelist 43) и `../re/main-arch.md` (5 цепочек вызовов):

```mermaid
flowchart LR
    subgraph WAS["Было (Electron)"]
        W1["web.js (рендерер)"] --> W2["DiscordNative (preload)"] --> W3["main + нативные модули"]
    end
    subgraph NOW["Стало (нативный процесс)"]
        R1["discord-ui"] --> R2["команды и события core"] --> R3["discord-voice / media / platform"]
    end
    W2 -.->|"замена 1:1 по таблице"| R2
```

| Электронный канал (исходно) | Цепочка RE | Замена в Rust |
|---|---|---|
| `DISCORD_WINDOW_*` | рендерер → preload → main → BrowserWindow | окно Tauri (`tao`/`wry`): min-size, фуллскрин, закрытие в трей |
| `DISCORD_NOTIFICATIONS_*` | → main → Notification | `notify-rust`/D-Bus, действия open/reply |
| `DISCORD_NATIVE_MODULES_*` | → main → `require()` | собственные крейты (медиа, NS, spellcheck) |
| `DISCORD_PROCESS_UTILS_*` | → main → child_process | `tokio::process` (клипы), sysinfo |
| `DISCORD_POWER_SAVE_*` | → main → powerSaveBlocker | D-Bus inhibitors (ScreenSaver/Login1) |
| `DISCORD_CLIPBOARD_*` | → main → clipboard | `arboard` |
| `DISCORD_UTILITY_*` | трей, бейдж, deep links | `ksni`, Unity launcher API, `xdg-open`/`zbus` |
| `DISCORD_VOICE_ENGINE_*` (голос) | рендерер ↔ `discord_voice.node` | `discord-voice` (внутри процесса или медиа-IPC) |
| `DISCORD_KRISP_*` (шумодав) | `discord_krisp.node` | RNNoise-совместимый крейт + свой инференс моделей |
| Акселерация GPU | `@discordapp/gpu-helper` + dlopen vulkan/va | `wgpu` (vulkan) + `va-api`/`nvenc` для видео |
| Idle/wayland | `discord-utils`/idle | `ext-idle-notify`/X11 XScreenSaver |
| Автозапуск | main | XDG autostart файл |

## 8. ADR-1: UI-оболочка — `Tauri` (решение владельца, 2026-10-19)

Выбрана оболочка **`Tauri`**: Rust-бэкенд-процесс + фронтенд в системном WebView (Linux —
WebKitGTK, Windows — WebView2, macOS — WKWebView); встроенный Chromium не поставляется.
Спайк P0-01 измеряет метрики трёх экранов на Tauri; для запасного пути сохранены критерии
нативных тулкитов:

| Критерий | Tauri (выбран) | iced | Slint |
|---|---|---|---|
| Виртуализация 100k сообщений | виртуальный скролл (проверить в спайке) | есть (widget::List) | есть (ListView) |
| Кастомные токены/темы (5132 переменные) | CSS-переменные 1:1 | стилизация свойств | дизайн-язык, переменные |
| Анимации 60 fps | композитор WebView (проверить в спайке) | да (wgpu) | да |
| Текстовый ввод/IME/вставка медиа | WebView (проверить IME/Wayland) | хороший | хороший |
| Скорость итераций | высокая (web-стек) | средняя | высокая |
| Размер/зависимости | системный WebView + нативные крейты | wgpu (тяжелее) | мал |
| Роль | **✔ основа** | резерв №1 | резерв №2 |

Спайк: три экрана (чат с 100k сообщений, эмодзи-пикер с 10 категориями, настройки с 38
тумблерами) на Tauri; ADR-1 фиксирует бенчмарки (FPS, RAM, время сборки, старт) и замер
резервного нативного тулкита.

```mermaid
flowchart TD
    S0["P0-01: спайк Tauri — три экрана (чат 100k, эмодзи, настройки)"] --> Q1{"60 fps на 100k сообщений в WebView?"}
    Q1 -->|нет| ALT["Запасной путь: нативный тулкит (iced/Slint), повторный спайк"]
    Q1 -->|да| Q2{"Токены, IME и 38 тумблеров выразимы?"}
    Q2 -->|нет| ALT
    Q2 -->|да| OK["ADR-1: метрики Tauri зафиксированы"]
```

## 9. Бюджеты производительности

| Метрика | Бюджет |
|---|---|
| Холодный старт до READY (кэш есть) | ≤ 1,5 с |
| Открытие канала из кэша | ≤ 100 мс |
| Отрисовка скролла 24k-списка участников | 60 fps, ≤ 16 мс кадр |
| Память в простое | ≤ 300 МБ |
| Голос: сквозная задержка | ≤ 150 мс (LAN), джиттер-буфер 40–60 мс |
| Сетевой оверхед шлюза (idx) | ≤ 37 байт на 1000 сообщений (сеть ×37 от идеала) |

## 10. Риски архитектуры и запасные пути

| Риск | План Б |
|---|---|
| UI-слой (Tauri/WebView) не тянет виртуальный чат | переход на нативный тулкит (iced/Slint) после P0-спайка |
| Медиа-процесс усложняет отладку | начать в одном процессе, вынести в P5 по мере надобности |
| DAVE/MLS задерживается | голос без E2EE там, где канал не требует; E2EE — обязательный гейт для «полного» релиза |
| Пиковый RSS на гигантских гильдиях | агрессивные лимиты member-страниц, LRU, ленивая загрузка эмодзи/ролей |
