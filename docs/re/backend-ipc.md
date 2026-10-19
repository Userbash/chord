# Discord Desktop 1.0.162 — карта backend/IPC (main ↔ renderer)

Версия: **1.0.162** (build_info.json: releaseChannel=stable, sentryDist=stable-linux-x64, release `7e0a1fef1fc1295b33b9404e5b77f1ae39d0c6e0`), Electron 42.11.12 / Chrome 148 (из User-Agent в CDP-захвате).
Разбор только по статике: asar-файлы + CDP-захват splash (чтение). Ничего не запускалось и не подключалось к процессам.

## 0. Что сделано и чем проверено

| Шаг | Результат |
|---|---|
| Распаковка `app.asar` | 11 файлов, 3 618 590 Б; **11/11 SHA256 совпали** с integrity из заголовка ASAR |
| Распаковка `core.asar` | 5 файлов, 1 761 514 Б; **5/5 SHA256 совпали** |
| Разбор бандлов | 4 файла JS, **1 266 модулей** (rspack/webpack-фабрики), 428 находок IPC-примитивов |
| Независимые сверки | enum `IPCEvents` (#595, 176 значений) как словарь имён; REA-граф `evidence/js-app-graph.json` (976 модулей, ipc-операций 0 — граф только про модульность); живой `capture-boot.jsonl` подтверждает splash-каналы |

Файлы-цели:

| Файл | Размер | Роль | Модулей |
|---|---|---|---|
| `app/bundle.js` | 3 066 301 Б | main (bootstrapper / splash host / module updater) | 825 |
| `core/bundle.js` | 402 296 Б | main (desktop core) | 81 |
| `core/mainScreenPreload.js` | 1 105 621 Б | preload главного окна (contextIsolation=true) | 357 |
| `app/splashScreenPreload.js` | 9 333 Б | preload splash-окна | 3 |

Ключевое правило префикса (проверено в коде обеих сторон): `getDiscordIPCEvent(ev) = /^DISCORD_/.test(ev) ? ev : "DISCORD_"+ev`. В main-бандле есть обёртка `ipcMain_1.default.{on,handle,reply,removeListener}` (core/bundle.js #2799, app/bundle.js #50902), которая пропускает имя через эту функцию — поэтому в main хендлеры часто пишутся **без** префикса (`"SYSTEM_TRAY_SET_ICON"`), а фактический канал — `DISCORD_SYSTEM_TRAY_SET_ICON`.

## 1. Статистика

| Метрика | Значение |
|---|---|
| Всего каналов (уникальных имён) | **168** |
| Каналов с обработчиком в main | **152** |
| Каналов с вызовом из renderer/preload | **129** |
| Каналов, где найдены обе стороны | **115** |
| Push main→renderer (webContents.send / reply / sendToAllWindows) | **7** (+push-хелпер с параметром-каналом) |
| Запрос-ответ (handle/invoke/sendSync) | **128** |
| Fire-and-forget (on/send/on) | **40** |
| Членов enum IPCEvents | **176** |
| Ссылок на каналы, отброшенных как шум | **85** |
| Call-sites с вычисляемым именем канала | **47** |
| Находок по файлам | preload 144, core/bundle 226, app/bundle 51, splash 7 |

Покрытие: **168** каналов; у **161** есть член в enum `IPCEvents` (#595). **7** каналов не входят в enum core-а — это собственные каналы app.asar: `DISCORD_SPLASH_SCREEN_{READY,QUIT,QUOTE}`, `DISCORD_SPLASH_UPDATE_STATE`, `DISCORD_GET_BUILD_OVERRIDE_STATUS`, `DISCORD_CLEAR_BUILD_OVERRIDE` и `DISCORD_MODULE_INSTALLED` (push из main). **15** членов enum не встретились как канал — в основном те, что отправляет веб-клиент через whitelist; их вызовы лежат вне asar (см. §6).

## 2. Каналы по группам

| Группа | N | main: файл#модули | renderer: модули | Каналы (без префикса DISCORD_) |
|---|---|---|---|---|
| окно (window) | 25 | `core/#3386,core/#5203` | `preload#5639` | WINDOW_BLUR · WINDOW_CLOSE · WINDOW_DEVTOOLS_CLOSED · WINDOW_DEVTOOLS_OPENED · WINDOW_FLASH_FRAME · WINDOW_FOCUS · WINDOW_GET_MEDIA_SOURCE_ID · WINDOW_GET_NATIVE_HANDLE · WINDOW_IS_ALWAYS_ON_TOP · WINDOW_MAXIMIZE · WINDOW_MINIMIZE · WINDOW_OPEN_DOCUMENT_PIP · WINDOW_RESTORE · WINDOW_SET_ALWAYS_ON_TOP · WINDOW_SET_BACKGROUND_THROTTLING · WINDOW_SET_CONTENT_PROTCTION · WINDOW_SET_FOCUSABLE · WINDOW_SET_FRAME_RATE · WINDOW_SET_MINIMUM_SIZE · WINDOW_SET_PROGRESS_BAR · WINDOW_SET_TRAFFIC_LIGHT_APPEARANCE · WINDOW_SET_TRAFFIC_LIGHT_POSITION · WINDOW_SET_WINDOW_CONTENT_PROTECTION · WINDOW_SHOW_INACTIVE · WINDOW_TOGGLE_FULLSCREEN |
| приложение (app) | 21 | `core/#5203,core/#6820,core/#4943,core/#4973` | `preload#2100,preload#1405` | APP_ASYNC_INDEX_TSX_LOADED · APP_BADGE_SET · APP_DOCK_BOUNCE · APP_DOCK_CANCEL_BOUNCE · APP_DOCK_SET_BADGE · APP_GET_ARCH · APP_GET_BUILD_NUMBER · APP_GET_DEFAULT_DOUBLE_CLICK_ACTION · APP_GET_HOST_VERSION_SYNC · APP_GET_MODULE_VERSIONS · APP_GET_OPEN_ON_START · APP_GET_PATH · APP_GET_PREFERRED_SYSTEM_LANGUAGES · APP_GET_RELEASE_CHANNEL_SYNC · APP_GET_SYSTEM_UI_DIRECTION_SYNC · APP_MAIN_BUNDLE_STATS · APP_PAINT_METRICS · APP_PAUSE_FRAME_EVICTOR · APP_RELAUNCH · APP_SET_BADGE_COUNT · APP_UNPAUSE_FRAME_EVICTOR |
| прочее | 14 | `app/#53006,core/#7912,core/#6577,core/#7138,core/#2236,core/#1353,core/#9419,core/#8750,core/#5203,core/#1742` | `preload#7523,preload#723,preload#8986,preload#9425,preload#7064,preload#6955,preload#2100,preload#1793` | ${event} · ACCESSIBILITY_GET_ENABLED · CONSTANTS_GET · CRASH_REPORTER_UPDATE_METADATA · DESKTOP_CAPTURER_GET_SOURCES · FEATURES_GET_BROWSER_FEATURES · GET_MOUSE_COORDINATES · HARDWARE_GET_DISPLAY_COUNT · MODULE_INSTALLED · NAVIGATION_HISTORY_CLEAR · OPEN_EXTERNAL_URL · TRACING_CAPTURE_PERFORMANCE · UNHANDLED_JS_EXCEPTION · UPDATED_QUOTES |
| процессы/метрики | 12 | `core/#9984` | `preload#9141` | PROCESS_UTILS_FLUSH_COOKIES · PROCESS_UTILS_FLUSH_DNS_CACHE · PROCESS_UTILS_FLUSH_STORAGE_DATA · PROCESS_UTILS_GET_CPU_USAGE · PROCESS_UTILS_GET_GPU_PROCESS_ID · PROCESS_UTILS_GET_LAST_CRASH · PROCESS_UTILS_GET_MAIN_ARGV_SYNC · PROCESS_UTILS_GET_SYSTEM_INFO · PROCESS_UTILS_GET_SYSTEM_METRICS · PROCESS_UTILS_SET_CRASH_INFORMATION · PROCESS_UTILS_SET_CRASH_REASON · PROCESS_UTILS_SET_MEMORY_INFORMATION |
| файлы | 9 | `core/#9473` | `preload#3466` | FILE_MANAGER_CREATE_DIRECTORY_IF_NOT_EXISTS · FILE_MANAGER_GET_ASSET_CACHE_PATH · FILE_MANAGER_GET_ASSET_CACHE_PATH_SYNC · FILE_MANAGER_GET_MODULE_DATA_PATH_SYNC · FILE_MANAGER_GET_MODULE_LOG_PATH_SYNC · FILE_MANAGER_GET_MODULE_PATH · FILE_MANAGER_SHOW_ITEM_IN_FOLDER · FILE_MANAGER_SHOW_OPEN_DIALOG · FILE_MANAGER_SHOW_SAVE_DIALOG |
| трей/панель задач/автозапуск | 9 | `core/#1275,core/#9090,core/#4973` | — | SYSTEM_TRAY_SET_APPLICATIONS · SYSTEM_TRAY_SET_ICON · SYSTEM_TRAY_SET_STATES · SYSTEM_TRAY_SET_STRINGS · THUMBAR_BUTTONS_UPDATE · TOGGLE_MINIMIZE_TO_TRAY · TOGGLE_OPEN_ON_STARTUP · TOGGLE_START_MINIMIZED · UPDATE_OPEN_ON_STARTUP |
| буфер обмена | 8 | `core/#7126` | `preload#6701` | CLIPBOARD_COPY · CLIPBOARD_CUT · CLIPBOARD_HAS_MIXED_CONTENT · CLIPBOARD_PASTE · CLIPBOARD_READ_TEXT · CLIPBOARD_WRITE_FILE · CLIPBOARD_WRITE_IMAGE · CLIPBOARD_WRITE_TEXT |
| питание | 8 | `core/#4531,core/#7420` | `preload#2818,preload#2853` | POWER_MONITOR_GET_SYSTEM_IDLE_TIME · POWER_MONITOR_LOCK_SCREEN · POWER_MONITOR_RESUME · POWER_MONITOR_SUSPEND · POWER_MONITOR_UNLOCK_SCREEN · POWER_SAVE_BLOCKER_BLOCK_DISPLAY_SLEEP · POWER_SAVE_BLOCKER_CLEANUP_DISPLAY_SLEEP · POWER_SAVE_BLOCKER_UNBLOCK_DISPLAY_SLEEP |
| клипы (clips) | 7 | `core/#7635` | `preload#6752` | DEBUG_STASH_CLIP_DECIDER_DATA · DELETE_CLIP · LOAD_CLIP · LOAD_CLIPS_DIRECTORY · MOVE_CLIP · RENDER_MONTAGE_RECIPE · UPDATE_CLIP_METADATA |
| уведомления | 7 | `core/#7592` | — | NOTIFICATIONS_GET_AUTHORIZATION · NOTIFICATIONS_GET_MODULE_STATUS · NOTIFICATIONS_GET_SETTINGS · NOTIFICATIONS_RECEIVED_RESPONSE · NOTIFICATIONS_REMOVE_ALL_NOTIFICATIONS · NOTIFICATIONS_REMOVE_NOTIFICATIONS · NOTIFICATIONS_SEND_NOTIFICATION |
| splash/бутстрап | 6 | `app/#96020` | `preload#85425` | CLEAR_BUILD_OVERRIDE · GET_BUILD_OVERRIDE_STATUS · SPLASH_SCREEN_QUIT · SPLASH_SCREEN_QUOTE · SPLASH_SCREEN_READY · SPLASH_UPDATE_STATE |
| орфография | 6 | `core/#3132` | `preload#9121` | SPELLCHECK_GET_AVAILABLE_DICTIONARIES · SPELLCHECK_REPLACE_MISSPELLING · SPELLCHECK_RESULT · SPELLCHECK_SET_LANGUAGES · SPELLCHECK_SET_LEARNED_WORDS · SPELLCHECK_SET_LOCALE |
| нативные модули | 5 | `core/#216` | — | NATIVE_MODULES_FINISH_UPDATER_BOOTSTRAP · NATIVE_MODULES_GET_HAS_NEW_UPDATER · NATIVE_MODULES_GET_MODULE_PATH · NATIVE_MODULES_GET_PATHS · NATIVE_MODULES_INSTALL |
| WebAuthn | 5 | `core/#5544` | `preload#3797` | WEBAUTHN_AUTHENTICATE_MAC · WEBAUTHN_REGISTER_MAC · WEBAUTHN_SIGNAL_ALL_ACCEPTED_CREDENTIALS · WEBAUTHN_SIGNAL_CURRENT_USER_DETAILS · WEBAUTHN_SIGNAL_UNKNOWN_CREDENTIAL |
| frame debugger | 4 | `core/#7026` | `preload#3269` | FRAME_DEBUGGER_ATTACH · FRAME_DEBUGGER_DETACH · FRAME_DEBUGGER_DETACHED · FRAME_DEBUGGER_RUN_BATCH |
| настройки | 4 | `core/#7683,core/#5203` | `preload#6434` | SETTINGS_GET · SETTINGS_GET_SYNC · SETTINGS_SET · SETTINGS_UPDATE_BACKGROUND_COLOR |
| поиск | 4 | `core/#4355` | — | SEARCH_CLEAR_INDEX · SEARCH_DELETE_DOMAINS · SEARCH_DELETE_ITEMS · SEARCH_INDEX_DOMAINS |
| GPU | 3 | `core/#1667` | `preload#4276` | GPU_SETTINGS_GET_ENABLE_HWACCEL_SYNC · GPU_SETTINGS_SET_CHROMIUM_SWITCHES · GPU_SETTINGS_SET_ENABLE_HWACCEL |
| NTP | 3 | `core/#7314` | `preload#3939` | NTP_CLOCK_GET_SYNC_INFO · NTP_CLOCK_START · NTP_CLOCK_SYNCED |
| safeStorage | 3 | `core/#5454` | `preload#5261` | SAFE_STORAGE_DECRYPT_STRING · SAFE_STORAGE_ENCRYPT_STRING · SAFE_STORAGE_IS_ENCRYPTION_AVAILABLE |
| кэш пользовательских данных | 3 | `core/#299` | `preload#5984` | USER_DATA_CACHE_DELETE · USER_DATA_CACHE_GET · USER_DATA_CACHE_SAVE |
| Rich Presence (intents) | 2 | `core/#4355` | — | INTENTS_RESIGN_ACTIVITY · INTENTS_SET_ACTIVITY |

### Детали по значимым каналам

Полный машинный список — в `backend-ipc.json` (`channels[]`: имя, enum-член, main-модуль, renderer-модуль, назначение, образцы аргументов).

| Канал | Стороны | main (модуль) | renderer (модуль) | Назначение |
|---|---|---|---|---|
| `DISCORD_FILE_MANAGER_GET_MODULE_LOG_PATH_SYNC` | M:handle + R:invoke | core/bundle.js#9473 | preload#3466 | путь к логу модуля (sendSync) |
| `DISCORD_APP_GET_MODULE_VERSIONS` | M:handle + R:invoke | core/bundle.js#4943 | preload#2100 | версии установленных модулей (через updater) |
| `DISCORD_APP_RELAUNCH` | M:handle + R:invoke | core/bundle.js#4943 | preload#2100 | перезапуск приложения (flush session → relaunch) |
| `DISCORD_CLEAR_BUILD_OVERRIDE` | M:handle + R:invoke | app/bundle.js#96020 | preload#85425 | splash: сбросить build-override |
| `DISCORD_CLIPBOARD_WRITE_IMAGE` | M:handle + R:invoke | core/bundle.js#7126 | preload#6701 | вызвать: clipboard write image |
| `DISCORD_FILE_MANAGER_SHOW_SAVE_DIALOG` | M:handle + R:invoke | core/bundle.js#9473 | preload#3466 | вызвать: file manager show save dialog |
| `DISCORD_FRAME_DEBUGGER_ATTACH` | M:handle + R:invoke | core/bundle.js#7026 | preload#3269 | подключить отладчик к iframe (CDP) |
| `DISCORD_GET_BUILD_OVERRIDE_STATUS` | M:handle + R:invoke | app/bundle.js#96020 | preload#85425 | splash: статус build-override (cookie) |
| `DISCORD_LOAD_CLIP` | M:handle + R:invoke | core/bundle.js#7635 | preload#6752 | загрузить клип (discord_clips) |
| `DISCORD_NATIVE_MODULES_GET_PATHS` | M:handle | core/bundle.js#216 | — | пути поиска нативных модулей (sendSync) |
| `DISCORD_NOTIFICATIONS_RECEIVED_RESPONSE` | P:push | core/bundle.js#7592 | — | push в renderer: пользователь нажал/ответил на уведомление |
| `DISCORD_POWER_MONITOR_SUSPEND` | R:on + P:push | core/bundle.js#4531 | preload#2818 | push: система уходит в сон |
| `DISCORD_PROCESS_UTILS_GET_SYSTEM_METRICS` | M:handle + R:invoke | core/bundle.js#9984 | preload#9141 | системные метрики (CPU/RAM процессов Electron) |
| `DISCORD_RENDER_MONTAGE_RECIPE` | M:handle + R:invoke | core/bundle.js#7635 | preload#6752 | собрать монтаж клипа |
| `DISCORD_SAFE_STORAGE_ENCRYPT_STRING` | M:on + R:sendSync | core/bundle.js#5454 | preload#5261 | шифрование строки (DPAPI/keyring) |
| `DISCORD_SETTINGS_GET_SYNC` | M:on + R:sendSync | core/bundle.js#7683 | preload#6434 | прочитать настройку синхронно |
| `DISCORD_UNHANDLED_JS_EXCEPTION` | M:handle + R:invoke | core/bundle.js#7138 | preload#8986 | вызвать: unhandled js exception |
| `DISCORD_WEBAUTHN_REGISTER_MAC` | M:handle + R:invoke | core/bundle.js#5544 | preload#3797 | WebAuthn: регистрация ключа (нативный discord_webauthn) |
| `DISCORD_WINDOW_OPEN_DOCUMENT_PIP` | M:handle + R:invoke | core/bundle.js#3386 | preload#5639 | открыть Picture-in-Picture для документа (executeJavaScript в renderer) |
| `DISCORD_WINDOW_SET_PROGRESS_BAR` | M:handle + R:invoke | core/bundle.js#3386 | preload#5639 | прогресс-бар в панели задач (по ключу окна key) |
| `DISCORD_GET_MOUSE_COORDINATES` | M:handle | core/bundle.js#9419 | — | координаты курсора на экране |
| `DISCORD_INTENTS_SET_ACTIVITY` | M:handle | core/bundle.js#4355 | — | Rich Presence: установить активность |
| `DISCORD_MODULE_INSTALLED` | R:on | — | preload#2100 | push: модуль установлен |
| `DISCORD_NATIVE_MODULES_GET_MODULE_PATH` | M:on | core/bundle.js#216 | — | путь к нативному модулю по имени (sendSync) |
| `DISCORD_NATIVE_MODULES_INSTALL` | M:handle | core/bundle.js#216 | — | установить нативный модуль (через updater) |
| `DISCORD_NAVIGATION_HISTORY_CLEAR` | M:on | core/bundle.js#5203 | — | вызвать: navigation history clear |
| `DISCORD_NOTIFICATIONS_SEND_NOTIFICATION` | M:handle | core/bundle.js#7592 | — | показать системное уведомление (нативный discord_notifications) |
| `DISCORD_NTP_CLOCK_SYNCED` | R:on | — | preload#3939 | push: NTP-синхронизация завершена |
| `DISCORD_OPEN_EXTERNAL_URL` | M:on | core/bundle.js#5203 | — | открыть внешний URL (saferShellOpenExternal) |
| `DISCORD_SEARCH_INDEX_DOMAINS` | M:handle | core/bundle.js#4355 | — | поиск: индексировать домены |
| `DISCORD_SPELLCHECK_RESULT` | R:on | — | preload#9121 | push: результат проверки орфографии (misspelled word) |
| `DISCORD_SPLASH_SCREEN_READY` | R:send | — | preload#85425 | splash готов к показу |
| `DISCORD_SPLASH_UPDATE_STATE` | R:on | — | preload#85425 | push: состояние splash (checking/launching/...) |
| `DISCORD_SYSTEM_TRAY_SET_ICON` | M:on | core/bundle.js#1275 | — | трей: установить иконку по ключу |
| `DISCORD_SYSTEM_TRAY_SET_STATES` | M:on | core/bundle.js#1275 | — | трей: состояния (чекбоксы/радио меню) |
| `DISCORD_THUMBAR_BUTTONS_UPDATE` | M:on | core/bundle.js#9090 | — | панель задач Windows: кнопки |
| `DISCORD_TOGGLE_MINIMIZE_TO_TRAY` | M:on | core/bundle.js#4973 | — | вкл/выкл «сворачивать в трей» |
| `DISCORD_UPDATED_QUOTES` | M:on | app/bundle.js#53006 | — | push: обновлённые фразы splash (quotes_copy.json) |

## 3. Мост DiscordNative (renderer-сторона)

Файл `core/mainScreenPreload.js` (preload главного окна, `contextIsolation: true`, `sandbox: false`):

- `contextBridge.exposeInMainWorld("DiscordNative", DiscordNative)`, затем `process.once("loaded", …) → global.DiscordNative`;
- мост содержит **35** записей: 33 под-API (каждое проксирует каналы либо напрямую, либо через нативные модули) + флаги `isRenderer` и `setUncaughtExceptionHandler`:

| Поле | Модуль | Поле | Модуль |
|---|---|---|---|
| `isRenderer` | флаг process.type === "renderer" | `setUncaughtExceptionHandler` | function(handler) |
| `nativeModules` | #7927 requireModule/getModulePath + PREMAP_MOD | `process` | #8434 (arch/platform) |
| `os` | #3121 (arch/платформа) | `app` | внешний объект (dock/badge/relaunch/версии) |
| `clipboard` | #6701 | `ipc` | #8981 (whitelist invoke/send/on) |
| `gpuSettings` | #4276 | `window` | #5639 (API по ключу окна) |
| `powerMonitor` | события питания (push из main) | `ntpClock` | #3939 |
| `spellCheck` | #9121 | `crashReporter` | #8986 |
| `desktopCapture` | #9425 | `fileManager` | #3466 |
| `clips` | #6752 | `processUtils` | #9141 |
| `powerSaveBlocker` | #2853 | `frameDebugger` | #3269 |
| `http` | #723 | `accessibility` | #7523 |
| `features` | #7064 | `settings` | #6434 |
| `userDataCache` | #5984 | `thumbar` | #8476 |
| `safeStorage` | #5261 | `hardware` | #6955 |
| `tracing` | #1793 | `riotGames` | #4482 |
| `cs2Gsi` | #6074 | `dotaGsi` | #3288 |
| `webAuthn` | #3797 | `gcEvents` | #8408 |
| `sysimg` | #7987 |  |  |

**Whitelists для веб-клиента** (preload #8981, `DiscordNative.ipc`):

- `RENDERER_IPC_SEND_WHITELIST` — **30** каналов, разрешённых для `send()` (`ipcRenderer.send` после префиксации);
- `RENDERER_IPC_INVOKE_WHITELIST` — **14** каналов для `invoke()`; объединение — **43** канала;
- `on(ev, cb)` префиксует имя и вызывает колбэк как `callback(null, ...args)`; попытка отправить/вызвать неразрешённый канал **бросает** `Error("cannot send this event")` / `Error("cannot invoke this event")`.
- Итого веб-клиент не может дёрнуть произвольный канал: доступен только явный список (см. таблицу ниже).

`SEND (30)`: `APP_ASYNC_INDEX_TSX_LOADED` · `APP_BADGE_SET` · `APP_FIRST_RENDER_AFTER_READY_PAYLOAD` · `APP_GET_ANALYTICS_EVENTS` · `APP_MAIN_BUNDLE_STATS` · `APP_PAINT_METRICS` · `APP_LOADED` · `APP_PUSH_ANALYTICS` · `APP_VIEWED` · `CHECK_FOR_UPDATES` · `NAVIGATION_HISTORY_CLEAR` · `NOTIFICATION_CLOSE` · `NOTIFICATION_SHOW` · `NOTIFICATIONS_CLEAR` · `OPEN_EXTERNAL_URL` · `QUIT_AND_INSTALL` · `SETTINGS_UPDATE_BACKGROUND_COLOR` · `SYSTEM_TRAY_SET_ICON` · `SYSTEM_TRAY_SET_APPLICATIONS` · `SYSTEM_TRAY_SET_STATES` · `SYSTEM_TRAY_SET_STRINGS` · `THUMBAR_BUTTONS_UPDATE` · `TOGGLE_MINIMIZE_TO_TRAY` · `TOGGLE_OPEN_ON_STARTUP` · `TOGGLE_START_MINIMIZED` · `UPDATE_OPEN_ON_STARTUP` · `UPDATER_HISTORY_QUERY_AND_TRUNCATE` · `UPDATED_QUOTES` · `WINDOW_SET_TRAFFIC_LIGHT_APPEARANCE` · `WINDOW_SET_TRAFFIC_LIGHT_POSITION`

`INVOKE (14)`: `APP_GET_ANALYTICS_EVENTS` · `GET_MOUSE_COORDINATES` · `INTENTS_SET_ACTIVITY` · `INTENTS_RESIGN_ACTIVITY` · `NOTIFICATIONS_GET_AUTHORIZATION` · `NOTIFICATIONS_GET_SETTINGS` · `NOTIFICATIONS_GET_MODULE_STATUS` · `NOTIFICATIONS_SEND_NOTIFICATION` · `NOTIFICATIONS_REMOVE_NOTIFICATIONS` · `NOTIFICATIONS_REMOVE_ALL_NOTIFICATIONS` · `SEARCH_INDEX_DOMAINS` · `SEARCH_CLEAR_INDEX` · `SEARCH_DELETE_DOMAINS` · `SEARCH_DELETE_ITEMS`

### Нативные модули и их загрузка

| Модуль | Кто грузит | Каналы/заметка |
|---|---|---|
| `discord_desktop_core` | app/bundle.js (requireNative) | не нативный .node, а точка входа core.asar; core.startup({...}) |
| `discord_asset_cache` | app/bundle.js | кэш ассетов бутстраппера |
| `discord_utils` | app/bundle.js, core/mainScreenPreload.js, core/bundle.js | `DISCORD_FILE_MANAGER_*`, `DISCORD_PROCESS_UTILS_*` |
| `discord_voice` | preload (#7927, PREMAP_MODULES) + main | голосовой движок |
| `discord_media` | preload (PREMAP_MODULES) | медиа/WebRTC |
| `discord_zstd` | preload (PREMAP_MODULES) | сжатие |
| `discord_cloudsync` | preload (PREMAP_MODULES) | синхронизация |
| `discord_premap` | preload | premap-загрузчик |
| `discord_hook` | preload | хуки (Linux) |
| `discord_sysimg` | preload #7987 (DiscordNative.sysimg) | обработка изображений |
| `discord_notifications` | core/bundle.js #7592 | `DISCORD_NOTIFICATIONS_*` |
| `discord_intents` | core/bundle.js #4355 | `DISCORD_INTENTS_*`, `DISCORD_SEARCH_*` |
| `discord_webauthn` | core/bundle.js #5544 | `DISCORD_WEBAUTHN_*` |
| `discord_clips_main_proc` | core/bundle.js #7635 | `DISCORD_LOAD_CLIP`, `DISCORD_RENDER_MONTAGE_RECIPE`, `DISCORD_*_CLIP*` |

Механика: `DiscordNative.nativeModules.requireModule(name)` разрешает только `/^discord_[a-z0-9_-]+$/` или `erlpack`, и дополнительно блокирует `PRELOAD_ONLY_MODULES`; `getModulePath(name)` уходит в main как `ipcRenderer.sendSync(DISCORD_NATIVE_MODULES_GET_MODULE_PATH, name)`. Пути поиска подтягиваются вызовом `NATIVE_MODULES_GET_PATHS`/`NATIVE_MODULES_INSTALL` (main: core/bundle.js#216). `PREMAP_MODULES = ["discord_voice","discord_zstd","discord_cloudsync","discord_media"]`.

## 4. Модель процессов и сквозные цепочки

| Процесс | Что грузит | Файлы |
|---|---|---|
| main (bootstrap) | splash-окно, апдейтер модулей, Sentry/electron-log IPC, `requireNative("discord_desktop_core")` | `resources/app.asar → app/bundle.js` |
| main (core) | окно главного экрана, окно настроек/попауты, трей, CDP-frame-debugger, нативные дискорд-модули | `modules/discord_desktop_core-1/discord_desktop_core/core.asar → core/bundle.js` |
| renderer (splash) | splash UI + `splashScreenPreload.js` | тот же app.asar |
| renderer (main) | веб-клиент discord.com + `mainScreenPreload.js` (мост `DiscordNative`) | core.asar |
| utility | учитываются в метриках `DISCORD_PROCESS_UTILS_GET_*` (типы GPU/renderer/utility); в `core/bundle.js` вызовов `utilityProcess` нет, в `app.asar` есть только Sentry-патч обёртки `utilityProcess.fork` — то есть собственных utility-процессов Discord здесь не создаёт | — |

`webPreferences` (проверено в core/bundle.js):

- splash: `nodeIntegration:false, sandbox:false, contextIsolation:true, preload: splashScreenPreload.js, frame:false, 300×N`
- главное окно: `nodeIntegration:false, sandbox:false, contextIsolation:true, spellcheck:true, preload: mainScreenPreload.js, enableBlinkFeatures:"EnumerateDevices,AudioOutputDevices", additionalArguments:[…]`
- третье окно (попауты): `nodeIntegration:false, nativeWindowOpen:true, enableRemoteModule:false, contextIsolation:true`

Связь `app.asar` ↔ `core.asar` — **не IPC**, а прямой вызов: `core = requireNative("discord_desktop_core"); core.startup({Constants, GPUSettings, analytics, appSettings, autoStart, buildInfo, crashReporterSetup, logger, moduleUpdater, paths, splashScreen, updater})`.

### Системное уведомление (renderer → main → нативный модуль → push)

- web-клиент: window.DiscordNative.ipc.invoke("NOTIFICATIONS_SEND_NOTIFICATION", opts)
- preload #8981: getDiscordIPCEvent → "DISCORD_NOTIFICATIONS_SEND_NOTIFICATION"; проверка RENDERER_IPC_INVOKE_WHITELIST; ipcRenderer.invoke
- main core/bundle.js #7592: IPCEvent handler → requireLoadedLib() = нативный discord_notifications → sendNotification
- main → renderer push: sendToAllWindows(DISCORD_NOTIFICATIONS_RECEIVED_RESPONSE, action, identifier, ...) → webContents.send
- web-клиент получает push через DiscordNative.ipc.on("NOTIFICATIONS_RECEIVED_RESPONSE") (префиксация в preload #8981)

### Уведомление о трее (renderer send → main handler через префиксующую обёртку)

- web-клиент: DiscordNative.ipc.send("SYSTEM_TRAY_SET_ICON", iconKey) (канал в RENDERER_IPC_SEND_WHITELIST)
- preload: ipcRenderer.send("DISCORD_SYSTEM_TRAY_SET_ICON", ...)
- main: ipcMain_1.default.on("SYSTEM_TRAY_SET_ICON") — обёртка вызывает ipcMain.on(getDiscordIPCEvent("SYSTEM_TRAY_SET_ICON")) → реальный канал DISCORD_SYSTEM_TRAY_SET_ICON (core/bundle.js #1275)
- реализация: Electron Tray (new electron_1.Tray) + иконки tray-*/${...}.png

### Клип (clips): файл и протокол

- renderer: DiscordNative.clips.load(path) → invoke DISCORD_LOAD_CLIP (preload #6752)
- main core/bundle.js #7635: нативный discord_clips_main_proc читает mp4, проверяет Content-Range/Content-Type
- main регистрирует кастомный протокол protocol.handle(DISCORD_CLIP_PROTOCOL="disclip") — отдача клипов по диапазонам
- ошибки: "Invalid clip requested via protocol:", "Clip not found", "Something went wrong when serving the clip:"

### Обновление/бутстрап (splash) и модули

- splash preload (#85425, app/bundle.js): invoke DISCORD_GET_BUILD_OVERRIDE_STATUS / send DISCORD_SPLASH_SCREEN_READY / on DISCORD_SPLASH_UPDATE_STATE
- app/bundle.js #96020: ipcMain_1.default.handle(IPCEvents.GET_BUILD_OVERRIDE_STATUS) → cookie build-override
- app/bundle.js #53006: splash-окно push-ит состояние: webContents.send(`DISCORD_${event}`, ...) — подтверждено capture-boot.jsonl (onStateUpdate: checking-for-updates → launching)
- установка нативных модулей идёт из main: channels DISCORD_NATIVE_MODULES_GET_PATHS / GET_MODULE_PATH / INSTALL / FINISH_UPDATER_BOOTSTRAP / GET_HAS_NEW_UPDATER (core/bundle.js #216); preload #7927 дёргает их через ipcRenderer
- загрузка ядра: app/bundle.js: core = requireNative("discord_desktop_core"); core.startup({Constants, GPUSettings, analytics, appSettings, autoStart, buildInfo, crashReporterSetup, logger, moduleUpdater, paths, splashScreen, updater}) — обмен между app.asar и core.asar идёт вызовом функции, а не IPC

### Нативный голос/медиа не ходит через IPC-каналы

- preload #7927 nativeModules: requireModule(name) разрешает только /^discord_[a-z0-9_-]+$/ или erlpack и запрещает PRELOAD_ONLY_MODULES
- PREMAP_MODULES = ["discord_voice","discord_zstd","discord_cloudsync","discord_media"] — прогреваются через premap
- getModulePath(name) → ipcRenderer.sendSync(DISCORD_NATIVE_MODULES_GET_MODULE_PATH, name) в main
- дальше вызовы идут напрямую в .node-аддон (без DISCORD_* каналов) — отсюда «дыра» в IPC-карте для голоса: его API не видно по каналам

## 5. Что осталось неизвестным

| Пробел | Детали | Чем проверять |
|---|---|---|
| 47 call-sites с вычисляемым именем канала (переменная/шаблон) | главные: generic-обёртки DiscordIPC.main.on/handle(channel) в preload #1999 и main, push-хелпер sendMainWindow(channel) в core/bundle.js #5203, Sentry ipcUtil.createKey(...) в app/bundle.js (#85575, namespace "sentry-ipc" → каналы sentry-ipc.start/scope/envelope/status/structured-log/metric), electron-log __ELECTRON_LOG__/__ELECTRON_LOG_IPC__, DISCORD_${event} в splash | нужен либо рантайм-лог ipcRenderer/main (CDP-захват), либо ручной обход конкретных хелперов |
| Отправители push-каналов (main → renderer) покрыты не полностью | каналы вида DISCORD_POWER_MONITOR_RESUME, DISCORD_NTP_CLOCK_SYNCED, DISCORD_MODULE_INSTALLED, DISCORD_SPELLCHECK_RESULT найдены как listener в preload; парные webContents.send идут через хелпер с параметром-каналом или event.sender.send | тот же CDP-захват или трассировка webContents.send |
| 15 членов enum IPCEvents не встретились ни как канал | APP_GET_ANALYTICS_EVENTS, APP_LOADED, APP_PUSH_ANALYTICS, APP_VIEWED, APP_FIRST_RENDER_AFTER_READY_PAYLOAD, APP_GET_MAIN_BUNDLE_STATS, CHECK_FOR_UPDATES, CRASH_REPORTER_NEW_CRASH, NOTIFICATION_CLOSE, NOTIFICATION_SHOW, NOTIFICATIONS_CLEAR, REQUEST_OPEN_EXTERNAL_URL, QUIT_AND_INSTALL, THUMBAR_BUTTONS_CLICKED, UPDATER_HISTORY_QUERY_AND_TRUNCATE | все 15 входят в whitelist-ы (send/invoke) или обслуживаются модульным апдейтером #6605 → их вызовы находятся в web-клиенте (Cache_Data) либо диспетчеризуются таблицей; нужен разбор web-бандлов |
| Сторона веб-клиента (discord.com bundle) не разбиралась | кто именно вызывает 43 whitelisted канала из renderer — в asar не видно; в кэше ~/.config/discord/Cache/Cache_Data лежат JS-бандлы web-клиента | rg по Cache_Data на "DiscordNative.ipc" и разбор web-бандлов |
| Нативные .node-модули не дизассемблировались | известны только точки входа (discord_voice, discord_notifications, discord_clips_main_proc, discord_webauthn, discord_intents, discord_utils), внутренние функции/экспорты — нет | REA open_binary/analyze_function по modules/…/*.node |
| События модульного апдейтера (core/bundle.js #6605) не привязаны к каналам | в #6605 есть enum с именами без префикса (CHECK_FOR_UPDATES, QUIT_AND_INSTALL, MODULE_INSTALLED, UPDATE_AVAILABLE, MODULE_INSTALL_PROGRESS, UPDATER_HISTORY_QUERY_AND_TRUNCATE…); после getDiscordIPCEvent они становятся DISCORD_<NAME>, но статического места регистрации хендлера/отправки нет — вероятно, диспетчеризация через таблицу | разобрать #6605 и места, где enum передаётся как объект (map), либо снять рантайм-лог |
| Назначение каналов выведено из имён/кода обработчиков | документация Discord недоступна; формулировки в поле purpose — интерпретация | код-ревью обработчиков в core/bundle.js (модули указаны) или рантайм-наблюдение |

## 6. Отброшено (шум)

| Что | Сколько | Почему |
|---|---|---|
| webContents-методы без канала (getTitle/isDestroyed/getAllWebContents/getFocusedWebContents/reload/toggleDevTools/insertCSS/setWindowOpenHandler/on/once/off/removeListener/fromId/clearHistory) | 85 | это не IPC-каналы, а работа с объектом окна/EventEmitter |
| плейсхолдеры в шаблонах ({`DISCORD_${event}`} в splash, `${DISCORD_NAMESPACE}${event}` в core #5203) | — | имя канала вычисляется; вынесено в unknowns |
| первая версия сканера: тело модуля #3288 в preload перекрывало следующий модуль (сдвиг границ) | — | исправлено обрезкой по следующей фабрике (clampModules); из-за этого 104 «находки» были дублями одного и того же кода |
| Sentinel/OpenTelemetry enum-константы (сотни вида gen_ai.*, db.*, package versions) | — | не IPC — фильтр оставлен только по DISCORD_*/IPCEvents |

Отдельно: `USER_SETTINGS_OPEN` — единственный найденный push-литерал **без** префикса `DISCORD_` (main #7592 → `webContents.send`), и `DISCORD_${event}` (splash) — шаблон, а не имя.

---

Артефакты разбора: `tools/asar-extract.mjs`, `tools/scan-ipc.mjs`, `tools/build-channels.mjs`, `tools/dump-module.mjs`, `tools/module-strings.mjs`; распакованные asar — `asar-extract/{app,core}/`; промежуточные сканы — `ipc/scan-*.json`, `ipc/channels-final.json`.
