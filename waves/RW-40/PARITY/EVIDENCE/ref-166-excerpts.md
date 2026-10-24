# E1 — выдержки из установленного эталона 1.0.166

Источник: `/home/sanya/.config/discord/app-1.0.166/resources/app.asar` (bundle.js) и
`.../modules/discord_desktop_core-1/discord_desktop_core/core.asar` (mainScreenPreload.js).
Смещения — байты от начала распакованного файла. Артефакты распакованы в `/tmp/opencode/ref166`
только для чтения/цитирования; исходные asar не изменялись.

SHA-256: см. `hashes.txt`. Редакция: секретов/токенов в цитатах нет.

## 1. Deep links / single instance (app.asar:bundle.js)

- `APP_PROTOCOL` определён в константах: `bundle.js@2394`.
- `extractUrlFromArgs` + `openOrQueueUrl`: `bundle.js@22606`
  ```
  initialUrl=extractUrlFromArgs(process.argv);function openOrQueueUrl(url){null!=coreModule?coreModule.handleOpenUrl(url):initialUrl=url}
  allowMultipleInstances||app.on("second-instance",...)
  ```
- `app.on("open-url")` (macOS/Electron) и `launchInfo.userInfo.fallbackDeepLink`: `bundle.js@23596`.
  ```
  app.on("will-finish-launching",()=>{app.on("open-url",(event,url)=>{event.preventDefault(),openOrQueueUrl(url)})
  ```
- `setAsDefaultProtocolClient` в asar **отсутствует** (схему регистрирует установщик) — согласуется
  с `docs/re/main-arch.md:841-860`.

## 2. Автозапуск (app.asar:bundle.js)

- `autoStart.update(...)` после обновления хоста: `bundle.js@7201`; модуль autostart пишет
  `.desktop`-файл с `X-GNOME-Autostart-enabled=true` (Linux), `bundle.js@11083`.

## 3. IPC-события платформы (core.asar:mainScreenPreload.js)

Перечисление `IPCEvents` (модуль 595) содержит **203** константы `DISCORD_*`.
Ключевые (смещение в файле):

| Смещение | Константа | Что подтверждает |
|---|---|---|
| 11203 | `ACTIVITY_CAPTURE_STREAM_START/STOP/UPDATE` | захват потока активности (screen share/Go Live-класс) |
| 13624 | `APP_SET_BADGE_COUNT` | бейдж приложения |
| 23840 | `getDesktopCaptureSources()` | список источников экрана/окон |
| 23953 | `DESKTOP_CAPTURER_GET_SOURCES` | IPC к desktopCapturer |
| 54787 | `SETTINGS_GET/SET` | настройки через main |
| 62339 | `SYSTEM_TRAY_SET_STATES` | состояния трея |
| 62475 | `TOGGLE_MINIMIZE_TO_TRAY` | закрытие в трей |
| 63242 | `NOTIFICATIONS_SEND_NOTIFICATION` | системные уведомления |
| 90009 | `POWER_MONITOR_LOCK_SCREEN/UNLOCK` | блокировка экрана (idle/DND-сигнал) |
| 108699 | `SPELLCHECK_GET_AVAILABLE_DICTIONARIES` | spellcheck |
| 117717 | `WEBAUTHN_REGISTER_MAC` | WebAuthn (macOS-ветка) |
| 120721 | `WINDOW_SET_PROGRESS_BAR` | прогресс в таскбаре |

Дополнительно: `captureStream/captureImage`, `getMediaSourceId` (`WINDOW_GET_MEDIA_SOURCE_ID`),
`GET_MOUSE_COORDINATES`, `OPEN_EXTERNAL_URL`, `QUIT_AND_INSTALL`, `CHECK_FOR_UPDATES`,
`WINDOW_FLASH_FRAME`, `WINDOW_SET_ALWAYS_ON_TOP`, `WINDOW_OPEN_DOCUMENT_PIP`.

## 4. Голосовые/медийные модули 1.0.166 (файлы)

| Модуль | Размер | Комментарий |
|---|---:|---|
| `modules/discord_voice-1/discord_voice/discord_voice.node` | 127 791 736 | нативный голосовой движок |
| `.../discord_voice/libmediapipe.so` | 23 793 696 | mediapipe (видео/обработка) |
| `modules/discord_krisp-1/.../discord_krisp.node` | 62 107 672 | шумоподавление Krisp |
| `modules/discord_utils-1/.../discord_utils.node` | 23 679 480 | idle/power/utils |

`docs/re/main-arch.md:861-869`: main **не загружает** `discord_voice`; голос живёт в
renderer/preload; main даёт трею команды `SYSTEM_TRAY_TOGGLE_MUTE`, `SYSTEM_TRAY_TOGGLE_DEAFEN`,
`SYSTEM_TRAY_OPEN_VOICE_SETTINGS`.

## 5. Что этим НЕ подтверждается

- Renderer (React-приложение Discord) в asar не входит; UI-функции подтверждаются дампом
  1.0.162 (`UI2`, `UI6`, `VUI`, `F162`), а не 1.0.166.
- Наличие IPC-константы не доказывает её runtime-поведение; это E1, не E3.
- Go Live/stream-сигнализация (gateway `STREAM_*`) в дампе не наблюдалась.
