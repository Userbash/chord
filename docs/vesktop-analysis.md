# Vesktop ↔ наши RE-наработки: сравнение, пробелы, что улучшить и готовность проекта

> Копия отчёта в репозитории `discord-lab` (`docs/vesktop-analysis.md`).
> Перенос недостающих функций — `docs/feature-backlog.md` (волны R4–R6/D, карточки с критериями).

Дата: 19.10.2026. Источник: клон `https://github.com/Vencord/Vesktop.git`
(коммит `a02035b`, 05.10.2026, ветка main) + GitHub API (07.10.2026) + наши артефакты
`/tmp/opencode/r2-net/*`, `/tmp/opencode/ux-research/*`, `/tmp/opencode/discord-qt-analysis/*`,
репозиторий `discord-lab` (main `90c012a`).

---

## 0. Резюме

1. **Vesktop — не клиент Discord.** Это Electron-оболочка вокруг *официального web-клиента*
   discord.com: окно/трей/бейджи/обновления + инъекция Vencord (моды) + нативные помощники
   (venmic — захват звука PipeWire, libvesktop — D-Bus/C++, arrpc — Rich Presence для игр).
   Протокола Discord (шлюз, REST, голос, шифрование) в Vesktop нет вообще — этим занимается
   сам web-клиент, который Vesktop лишь патчит regexp-патчами.

2. **Наш RE-корпус и Vesktop почти не пересекаются по глубине.** Мы реверс-инженерили то, что
   Vesktop считает чёрным ящиком: шлюз (ETF/zstd-stream, опкоды), медиа-плоскость (RTP/RTCP,
   кодеки, SSRC), крипто медиа (режимы 0–6, AAD, nonce), UI-токены (5132 переменные),
   локализацию (26 754 строки ru), нативные модули (voice/krisp/utils), 115 158 символов функций.

3. **Пересечение — слой «десктоп-оболочка и аудио/стриминг».** Здесь Vesktop даёт ~25 конкретных
   функций/решений, которых у нас нет ни в данных, ни в спецификациях: PipeWire-loopback
   («виртуальный микрофон»), стрим-пикер с качеством, фильтры аудио-узлов, апдейтер с UI,
   автостарт/portal, deep links, спеллчек, privacy-настройки (WebRTC IP policy), hardening
   (electron fuses) и полный релизный конвейер (3 ОС, installers, winget, metainfo).

4. **Vesktop раскрывает ~15 внутренних API web-клиента**, которые уточняют/дополняют нашу карту
   (у нас есть IPC-слой main-процесса, но не web-сторона): `setSystemTrayApplications`,
   `getDefaultGoliveQuality()`, `MediaEngineStore.connections[].videoStreamParameters`,
   `STREAM_CLOSE/STREAM_UPDATE`, `STREAMER_MODE_UPDATE`, `InviteActions.resolveInvite`,
   `DEEP_LINK.handler`, детектор devtools (ложные срабатывания → сброс токена!) и др.

5. **Готовность честно делится на две оси:**
   - **Знания (RE-фаза): ~85–90%** для сценария A (свой мессенджер с UX Discord) — без изменений;
   - **Продукт (код/поставка): ~5%** — кода клиента нет, оболочки нет, релизов нет;
   - **К старту кодовой фазы — готов (блокеров нет); к «полноценному проекту» — примерно
     треть пути.** Vesktop — это планка продукта, наш RE — фундамент под собственную реализацию.

---

## 1. Паспорт Vesktop

| Параметр | Значение |
|---|---|
| Проект | Vesktop — «custom Discord desktop app» (обёртка официального web-клиента) |
| Версия / коммит | v1.6.7 / `a02035b` (05.10.2026; последний push 07.10.2026 — активен) |
| Автор/орг | Vendicated (Vencord), GPL-3.0-or-later |
| Масштаб | ~6 200 строк TS (src) + C++ хелпер (libvesktop) + скрипты сборки |
| GitHub | 8 513 ★, 471 форков, 81 открытых issue, создан 29.03.2023 |
| Стек | Electron 44.4.3, TypeScript 6.0, esbuild, React (из Discord/Vencord), pnpm 11, Node ≥ 22 |
| Зависимости рантайма | `arrpc` (fork, MIT), `electron-updater`; опционально `@vencord/venmic` (MPL-2.0) |
| Платформы | Linux (deb/tar.gz/rpm/AppImage), Windows (nsis/zip + portable), macOS (universal dmg, notarize) |
| CI/CD | test.yml (lint+tsc+build×2), release.yml (матрица 3 ОС, по тегам), meta.yml (metainfo.xml), winget-submission, update-vencord-dev |
| Ключевые фичи по README | Vencord предустановлен; легче и быстрее официального; Linux-шаринг экрана **со звуком** и Wayland; приватность («Discord не имеет доступа к системе») |
| Чего нет | Глобальные хоткеи (в роадмапе), собственный протокол/UI — по определению |

Контекст: Vesktop не борется с Discord, а *использует* его web-клиент и достраивает то, чего
лишён официальный Linux/Electron-клиент. Это противоположная стратегия нашему сценарию A
(свой клиент с нуля) и похожая на discord-qt по нише, но без user-токена и без своего UI.

---

## 2. Архитектура Vesktop (и почему сравнение несимметрично)

```
┌────────────────────────────── Vesktop (свой код, GPL-3.0) ─────────────────────────────┐
│ main (Electron, Node)          preload (typed IPC, contextIsolation)                    │
│  • окно/трей/меню/бейджи       • VesktopNative (мост в renderer)                        │
│  • апдейтер, автостарт         ──────────────────────────────────────────────           │
│  • deep links discord://       renderer (внутри web-клиента discord.com)                 │
│  • venmic (PipeWire .node)      • инъекция Vencord (моды, темы, плагины)                 │
│  • libvesktop (D-Bus, C++)      • Vesktop-патчи regexp-заменами в модулях Discord        │
│  • arrpc worker (IPC для игр)   • ScreenSharePicker (React-модалка внутри Discord)       │
└──────────────────────────────────────────────────────────────────────────────────────────┘
                                     │ использует
                                     ▼
                       Официальный web-клиент Discord (шлюз, REST, голос, E2EE — чёрный ящик)
```

- **Что Vesktop делает сам:** оболочка ОС, захват/микс аудио, патчи web-клиента, интеграции,
  доставка/обновление собственных файлов и Vencord.
- **Что Vesktop не делает:** шлюз, ETF/zstd, REST-вызовы, медиа-стек, шифрование — всё внутри
  web-клиента; Vesktop лишь перехватывает его внутренние модули по строкам-маркерам.
- **Наш проект — зеркально наоборот:** мы разобрали нутро (протокол/медиа/крипто/UI-токены),
  но у нас нет ни строки продуктового кода оболочки.

**Вывод:** «сравнивать функции» корректно по зоне перекрытия (десктоп-интеграции, аудио/стриминг,
упаковка), а по остальным слоям Vesktop служит чек-листом продукта, а мы для него — источником
знаний, которых у него нет (и не нужно).

---

## 3. Инвентарь функций Vesktop (каждая описана)

### 3.1 Оболочка окна и ОС

| # | Функция | Файл(ы) | Что делает / детали |
|---|---|---|---|
| 1 | Frameless-окно + свой заголовок | `mainWindow.ts`, `windowsTitleBar.tsx` | Отключает нативный заголовок; патчит web-UI, чтобы рисовал Windows-стиль (`USE_OSX_NATIVE_TRAFFIC_LIGHTS`, `refresh-title-bar-small`) |
| 2 | Нативный заголовок (опция) | `nativeTitleBar` | Возвращает системный frame; влияет на `platformClass`-патч (`platform-osx`/`platform-win`/`platform-web`) |
| 3 | Сохранение геометрии окна | `initWindowBoundsListeners`, `getWindowBoundsOptions` | Позиция/размер/максимизация/минимизация в `state.json`; проверка «в границах дисплея»; дефолт 1280×720, минимум 940×500 (как у официального) |
| 4 | Отключение мин. размера | `disableMinSize` | `setMinimumSize(1,1)` |
| 5 | Прозрачность/вибрация | `transparencyOption`, `macosVibrancyStyle` | Windows `backgroundMaterial` (mica/acrylic), macOS `setVibrancy("sidebar")` |
| 6 | Splash-экран | `splash.ts`, `views/splash.html`, `themedSplash.ts` | Кастомный splash с темой Discord, пиксельный режим; замена картинки пользователем |
| 7 | Первый запуск (тур) | `firstLaunch.ts`, `views/first-launch.html` | Онбординг при отсутствии `State.firstLaunch` |
| 8 | Окно «About» | `about.ts`, `views/about.html` | Информация о версии/зависимостях |
| 9 | Статичный заголовок | `staticTitle` | Отключает `page-title-updated` |
| 10 | Single instance | `main.ts` | `requestSingleInstanceLock`; второй инстанс фокусирует окно; в dev — заменяет |
| 11 | Меню-бар | `initMenuBar` | About, Force Update Vencord, Reset, Relaunch, Quit, скрытые зум-хоткеи (обход Qwertz) |
| 12 | Меню-бар: автозакрытие | `enableMenu` | `setAutoHideMenuBar` |
| 13 | Трей | `tray.ts` | Иконки `tray`/`trayUnread` (кастомные), клик-поведение (show/hide), меню: Open/About/Repair Vencord/Reset/Restart/Quit |
| 14 | Бейджи | `appBadge.ts`, `dbus.ts` | Linux — D-Bus Unity launcher count; macOS — dock badge («•» для -1); Windows — overlay-иконки `.ico` 1–10 + «Unread» |
| 15 | Мигание таскбара | `taskBarFlash.ts` | Опция; патч `.flashFrame(!0)` Discord → `VesktopNative.win.flashFrame` |
| 16 | Автостарт | `autoStart.ts` | Windows/macOS — login items; Linux — `.desktop` в `~/.config/autostart` или **portal** `requestBackground` (libvesktop); ключ `--start-minimized` |
| 17 | Deep links `discord://` | `vesktopProtocol.ts`, `setAsDefaultProtocolClient.ts`, `loadUrl`, `darwinURL` | Регистрация схемы, маршрутизация в путь web-клиента, обработка `open-url` (macOS) |
| 18 | Выбор ветки Discord | `DiscordBranchPicker`, `loadUrl` | stable / ptb / canary (`subdomain.discord.com`) |
| 19 | Внешние ссылки | `makeLinksOpenExternally` | Открывать ссылки системным браузером (опция `openLinksWithElectron`) |
| 20 | Popout-окна | `utils/popout.ts` | Поддержка «вынуть» стрим/плеер в отдельное окно |
| 21 | Steam Deck | `utils/steamOS.ts` | Game Mode: фикс клавиатуры, всегда fullscreen/максимизация, тёмная тема, «применить раскладку» |
| 22 | Portable-режим (Win) | `constants.ts` | Данные рядом с exe, если нет «Uninstall Vesktop.exe» |
| 23 | Отладочные окна | `ipc.ts` / `VesktopNative.debug` | Запуск `chrome://gpu`, `chrome://webrtc-internals` |
| 24 | Zoom-хоткеи (хаки) | `initMenuBar` | Скрытые пункты для `CmdOrCtrl+=`, numpad |
| 25 | Devtools-события | `initDevtoolsListeners` | Проброс в renderer (для патча детектора) |

### 3.2 Приватность и безопасность

| # | Функция | Файл(ы) | Детали |
|---|---|---|---|
| 26 | Изоляция рендера | `mainWindow.ts` | `sandbox: true`, `contextIsolation: true`, `nodeIntegration: false`, preload-мост |
| 27 | Electron Fuses | `package.json build` | Запрет `runAsNode`, `NODE_OPTIONS`, `--inspect`, загрузка только из asar и т.д. |
| 28 | Политика WebRTC IP | `main.ts`, `WebRTCIPHandlingPolicyPicker` | `setWebRTCIPHandlingPolicy` для всех окон; лечит «DTLS connecting» под VPN/Tailscale |
| 29 | Отключение media-сервисов | `main.ts` | `HardwareMediaKeyHandling`, `MediaSessionService` выключаются (Discord не становится медиа-сервисом ОС) |
| 30 | Autoplay-фикс | `main.ts` | `autoplay-policy=no-user-gesture-required` (звуки уведомлений) |
| 31 | Wayland-фикс цветов | `main.ts` | `WaylandWpColorManagerV1` disabled (вымытые цвета Electron) |
| 32 | Настройки GPU | `hardwareAcceleration`, `hardwareVideoAcceleration` | `disableHardwareAcceleration`, фичи `AcceleratedVideoEncoder/Decoder/LinuxGL/LinuxZeroCopyGL` |
| 33 | User-Agent | `cli.ts`, `constants.ts` | Свой UA (Chrome из Electron) + `--user-agent`/`--user-agent-os` с предупреждением; UA-строка `Vesktop/x.y` только для GitHub API |
| 34 | Сброс данных | `clearData.ts` | «Reset Vesktop»: чистка данных и рестарт |
| 35 | Патч детектора devtools | `devtoolsFixes.ts` | Discord-web ложно детектит devtools и **удаляет токен** → Vesktop подменяет на честные Electron-колбэки |
| 36 | Скрытие наг-баннера | `fixes.ts` | `localStorage.hideNag = true` |
| 37 | Скрытие кнопок «скачать приложение» | `hideDownloadAppsButton.ts` | Патч web-UI |
| 38 | Уведомления по умолчанию включены | `enableNotificationsByDefault.ts` | Патч `isPlatformEmbedded||true` |
| 39 | Клик по уведомлению → фокус | `fixes.ts` | Обёртка `Notification.prototype.onclick` |

### 3.3 Аудио, стриминг, устройства

| # | Функция | Файл(ы) | Детали |
|---|---|---|---|
| 40 | Виртуальный микрофон (PipeWire) | `venmic.ts` | Нативный модуль `venmic-<arch>.node` (`PatchBay`): `link/unlink/list/unmute`; захват аудио приложений через pipewire-pulse |
| 41 | Фильтры аудио-узлов | `buildLinkData` | Исключение своего аудио-сервиса (`app.getAppMetrics()` → pid), входных потоков (`Stream/Input/Audio`), виртуальных узлов; workaround `RecordStream`; `onlySpeakers`, `onlyDefaultSpeakers`, `granularSelect`, `deviceSelect` |
| 42 | Настройки аудио (UI) | `ScreenSharePicker.tsx` | Модалка: Initial Mute, Microphone Workaround, Only Speakers, Only Default Speakers, Ignore Inputs/Virtual/Devices, Granular/Device Selection |
| 43 | Стрим-пикер | `ScreenSharePicker.tsx`, `screenShare.ts` | Сетка источников с превью, выбор одного, качество (480–2160, 15/30/60 fps), contentHint («плавность/чёткость»), звук |
| 44 | Прямой запрос большого превью | `CAPTURER_GET_LARGE_THUMBNAIL` | Thumbnail 1920×1080 по клику (в списке — мелкие) |
| 45 | Wayland-путь | `screenShare.ts` | Один источник → пикер пропускается, сразу 1920p |
| 46 | Звук при захвате (Windows) | `screenShare.ts` | `loopback`/`loopbackWithoutChrome` (Win10 19045+) |
| 47 | Подмена `getDisplayMedia` (Linux) | `screenShareFixes.ts` | После получения стрима: подключение аудио-трека с виртуального микрофона (`vencord-screen-share`, 48 кГц, стерео, 16 бит, без AGC/EC/NS), применение ограничений frameRate/разрешения, `resizeMode: none`, `contentHint` |
| 48 | Переопределение качества GoLive | `patchStreamQuality` | `getDefaultGoliveQuality()` → bitrate 0.5–8 Мбит (target 600 кбит), encode/capture width/height/framerate, `pixelCount` |
| 49 | Параметры стрим-соединения | `handleGoLive` | `MediaEngineStore.getMediaEngine().connections[]`, `videoStreamParameters[0].maxFrameRate/maxResolution`, треки `conn.input.stream` |
| 50 | События стрима | `ScreenSharePicker` | Flux `STREAM_CLOSE`/`STREAM_UPDATE` (ключ `*:userId`) → `virtmic.stop()/unmute()` |
| 51 | Фиксы `getUserMedia`/`applyConstraints` | `fixStreamConstraints.ts` | `deviceId: {exact}` если строка; `autoGainControl` из `MediaEngineStore.getAutomaticGainControl()` — лечение «устройство сбрасывается» |
| 52 | Скрытие виртуального микрофона | `hideVenmicInput.tsx`, `hideSwitchDevice.tsx` | `enumerateDevices` фильтруется от `vencord-screen-share`; модалка «подключено новое устройство» игнорируется |
| 53 | Разрешения mic/cam (macOS) | `mediaPermissions.ts` | `systemPreferences.askForMediaAccess` |
| 54 | Копирование картинок в буфер | `IPC CLIPBOARD_COPY_IMAGE` | Буфер как картинка (`buffer` + `src`) |

### 3.4 Rich Presence и интеграции

| # | Функция | Файл(ы) | Детали |
|---|---|---|---|
| 55 | arRPC (RPC для игр) | `arrpc/*`, `renderer/arrpc.ts` | Воркер-сервер (IPC-сокет Discord RPC) → события `activity/invite/link` → renderer → `RichPresence (arRPC)`; кэш «detectable» в sessionData |
| 56 | Инвайт-резолв | `renderer/arrpc.ts` | `InviteActions.resolveInvite(code, "Desktop Modal")` + `INVITE_MODAL_OPEN` |
| 57 | Глубокие ссылки из RPC | `DEEP_LINK.handler({args})` | `findLazy(m => m.DEEP_LINK?.handler)` |
| 58 | Автотумбл стример-мода | `renderer/arrpc.ts` | Сокет `STREAMERMODE` от arRPC → `STREAMER_MODE_UPDATE` (auto), флаг `userDisabledStreamerMode` |
| 59 | Патч стример-мода | `streamerMode.ts` | Убирает проверку `platformEmbedded` в тумблере |
| 60 | D-Bus интеграции (Linux) | `libvesktop` (C++, gio) | `updateUnityLauncherCount`, `getAccentColor` (акцент темы ОС), `requestBackground` (portal-автостарт) |

### 3.5 Обновления, настройки, ассеты

| # | Функция | Файл(ы) | Детали |
|---|---|---|---|
| 61 | Апдейтер приложения | `updater.ts`, `views/updater` | `electron-updater`: проверка при старте, окно с прогрессом, `snooze` на сутки, `ignore` версии, `quitAndInstall`; `autoDownload=false` |
| 62 | Доставка Vencord | `vencordLoader.ts` | Скачивание 4 файлов с GitHub Releases Vencord (`vencordDesktopMain/Preload/Renderer.js+css`), проверка целостности, `--repair`, «Force Update Vencord», свой каталог Vencord |
| 63 | Настройки (свой стор) | `settings.ts`, `SettingsStore.ts` | `settings.json`/`state.json` с merge-дефолтами, глобальный листенер записи, слушатели изменений по ключу |
| 64 | Настройки UI | `components/settings/*` | Страница «Vesktop» внутри настроек Vencord: тумблеры (трей, автостарт, уведомления, прозрачность, аппаратýра, WebRTC policy, бранч, спеллчек-языки) |
| 65 | Спеллчек | `mainWindow.ts`, `VesktopNative`, `spellCheck.tsx` | Языки из Discord (`GET_LANGUAGES`), `setSpellCheckerLanguages` (до 5), проброс контекстного меню (`misspelledWord`, `dictionarySuggestions`), замена/добавление в словарь |
| 66 | Пользовательские ассеты | `userAssets.ts` | Замена splash/tray/trayUnread своими картинками (диалог выбора, URI-схема `vesktop://assets/…`) |
| 67 | Темы/quickCss | `constants.ts` | Каталоги `themes/`, `settings/quickCss.css` — паритет с Vencord |
| 68 | Бейдж уведомлений (настройка) | `NotificationBadgeToggle` | Общий тумблер бейджа |
| 69 | Предупреждение об устаревшей версии | `OutdatedVesktopWarning` | Напоминание обновиться |
| 70 | Ссылка на установку обновления | `isOutdated`/`UPDATER_OPEN` | Кнопка в настройках |

### 3.6 Сборка и выпуск (как «полноценный проект»)

| # | Функция | Файл(ы) | Детали |
|---|---|---|---|
| 71 | Сборка esbuild | `scripts/build/build.mts`, плагины | Два бандла (main/preload/renderer), `injectReact`, `sandboxFix`, `includeDir`, global externals, dev/watch |
| 72 | Упаковка installers | `package.json build`, electron-builder | Linux: deb, tar.gz, rpm, AppImage (x64+arm64), desktop-entry + MimeType; Windows: nsis, zip; macOS: universal dmg, notarize, entitlements |
| 73 | Публикация в GitHub Releases | `release.yml`, `publish: github` | По тегам `v*`, матрица 3 ОС |
| 74 | Метаданные дистрибутива | `generateMeta.mts`, `meta.yml` | `dev.vencord.Vesktop.metainfo.xml` в release (для линукс-репозиториев) |
| 75 | Winget | `winget-submission.yml` | Авто-PR в winget-community |
| 76 | CI-тесты | `test.yml` | `pnpm test` = ESLint + `tsc --noEmit` + сборка (обычная и dev) на push/PR |
| 77 | Синхронизация версии на сайте | `update-vencord-dev.yml` | При релизе пишет версию в `vencord.dev` |
| 78 | libvesktop (C++) | `packages/libvesktop` | gio/GDBus: Unity count, акцент, portal; prebuilds для x64/arm64 |

---

## 4. Сопоставление с нашими RE-наработками

### 4.1 По слоям

| Слой | Vesktop | Наши данные (RE) | Вердикт |
|---|---|---|---|
| Шлюз (WS, опкоды, ETF/zstd) | Нет (чёрный ящик) | ✅ 85%: URL, ETF-теги (16), zstd-stream, HELLO/RESUME, QoS op40, heartbeat 41.25 с | У нас глубже |
| REST API | Нет | ✅ 231 URL / 88 эндпоинтов v9 (GET, из кэша) | У нас есть, методы — добрать |
| Голосовой шлюз | Нет | ✅ heartbeat 13.75 с, session description, опкоды | У нас есть |
| Медиа RTP/RTCP | Нет | ✅ PT/SSRC/TWCC/NACK, кодеки, SFU-ротация, 152 290 пакетов | У нас есть |
| Крипто медиа | Нет (шифрует клиент) | ✅ режимы 0–6, AAD=заголовок, nonce 4Б LE, ключ 32Б | У нас есть |
| UI-токены/темы/локали | Нет (использует web-UI) | ✅ 5132 токена, dark/light/darker, cozy/compact, ru 26 754 | У нас есть |
| UI-экраны (DOM) | Патчит точечно (патчи + пикер) | 🟡 ~55%: DM/эмодзи/поповер; нет голосовой панели, меню сообщений, поиска | Добор нужен |
| Десктоп-IPC main-процесса | ✅ своя оболочка | ✅ ~150 каналов официального клиента (core.asar) | Пересекается; у нас карта, у них реализация |
| Нативные модули | venmic (свой), libvesktop (свой) | ✅ voice/krisp/utils (символы, API, input-watcher, idle) | Разные назначения |
| Ассеты | Свои (splash/tray/иконки бейджей) | ✅ 15 mp3, 137 svg, 206 png, 429 webp, 22 woff2, 6 wasm; лицензии — нельзя распространять | У нас данные, у них правовые грабли решены своими ассетами |
| Стриминг Linux со звуком | ✅ venmic + фильтры + пикер | 🟡 медиа-часть есть; loopback-практики нет (Discord использует своё) | **Пробел у нас** |
| Апдейтер/дистрибуция | ✅ полный конвейер | ❌ нет (только гейты репо) | **Пробел у нас** |
| Приватность/hardening | ✅ fuses, sandbox, WebRTC policy | ❌ не документировали | **Пробел у нас** |

### 4.2 Что Vesktop уточняет/раскрывает нового про Discord (web-сторона)

Это данные, которых у нас не было в `desktop-core-notes.md` (там был разбор IPC main-процесса):

| # | Открытие | Что это значит для нас |
|---|---|---|
| 1 | Модуль `,setSystemTrayApplications` + `window.close/minimize/maximize` в web | Точка стыка web-UI ↔ оболочка ОС; у нас IPC-каналы, здесь — вызывающая сторона |
| 2 | `getEnableHardwareAcceleration` в web-коде | Web спрашивает у оболочки про GPU-ускорение |
| 3 | Заголовок: `USE_OSX_NATIVE_TRAFFIC_LIGHTS`, `"refresh-title-bar-small"`, классы `platform-web/osx/win` | Полный контракт «web рисует под платформу» |
| 4 | `.flashFrame(!0)` + `taskbarFlash` | Механика мигания таскбара задаётся web-кодом |
| 5 | `STREAMER_MODE_UPDATE` `{key:"enabled", value, isAuto}` + `StreamerModeStore.autoToggle` | Авто-режим стримера — флаги/события; у нас его не было |
| 6 | `getDefaultGoliveQuality()` → `{capture, encode, bitrateMin/Max/Target}` | Схема параметров стрима Discord (нам пригодится для своей стрим-части) |
| 7 | `MediaEngineStore.getMediaEngine().connections[].videoStreamParameters[].maxFrameRate/maxResolution` + `conn.input.stream` | Управление качеством активного стрима |
| 8 | Flux-события `STREAM_CLOSE`/`STREAM_UPDATE`, ключ `*:userId` | Контракт подписки на свой стрим |
| 9 | `InviteActions.resolveInvite(code, "Desktop Modal")`, `INVITE_MODAL_OPEN` | Внутренний API инвайтов (у нас REST-инвентарь, здесь — flux-действия) |
| 10 | `DEEP_LINK.handler({args})` | Реестр deep links внутри клиента |
| 11 | `NotificationSettingsStore` + `isPlatformEmbedded` | Флаг «мы встроенное приложение» управляет дефолтами уведомлений |
| 12 | `lastOutputSystemDevice.justChanged`, `lastDeviceConnected` | Модалка «новое устройство»; пригодится при своей голосовой панели |
| 13 | UA-кворки: `deviceId` как `{exact}`, `autoGainControl` через `MediaEngineStore.getAutomaticGainControl()` | Реальные грабли getUserMedia, о которых мы не знали |
| 14 | Web-детектор devtools **удаляет токен** при ложном срабатывании | Важная деталь безопасности/сессии: у официального web это есть, у нас в данных — нет |
| 15 | Бранчи `stable/ptb/canary` как разные поддомены; `DISCORD_HOSTNAMES` | Совпадает с нашей картой веток — подтверждение |
| 16 | Минимальный размер окна 940×500 / 1280×720 и Android-подобные константы «как у официального» | Подтверждение дизайн-метрик (у нас есть токены, но размер окна — новая деталь) |
| 17 | `hideNag` в localStorage, кнопка `app-download-button` | Слой «нагов» официального web-клиента; для своего клиента не нужно, но для C-сценария — полезно |
| 18 | Спеллчек-контракт: `availableSpellCheckerLanguages`, `setSpellCheckerLanguages(≤5)`, `misspelledWord`, `dictionarySuggestions` | Готовый контракт для нашей будущей фичи |
| 19 | `autoplay-policy=no-user-gesture-required` нужен для звуков | Деталь для своего webview/клиента |
| 20 | Отключение `HardwareMediaKeyHandling`/`MediaSessionService` | Иначе Discord становится «медиа-сервисом» ОС — нюанс, который мы не документировали |

### 4.3 Симметрия: что есть у нас, чего нет у Vesktop

- **Протокол и транспорт**: ETF-таблица тегов, zstd-stream, компрессия с magic, RESUME/QoS,
  heartbeat-кадрирование, привязка сокетов по каденции — Vesktop не касается.
- **Крипто медиа**: режимы 0–6, AAD, nonce, ключ, оверхед — Vesktop не касается.
- **UI-система**: токены, темы, плотности, CSS-дамп 40 969 правил, локализация 26 754 строки —
  Vesktop не касается (использует чужой UI как есть).
- **Нативные модули Discord**: `discord_utils` (глобальный input-watcher PTT, Wayland idle),
  Krisp, `discord_voice` API — Vesktop замещает своими решениями (venmic), наш RE — про
  то, как это устроено у Discord.
- **Инвентарь символов** (115 158 функций, домены webrtc/rtp/audio/...) — у Vesktop этого нет
  и не требуется.

**Вывод:** наши наработки — это фундамент для *собственного* клиента (сценарий A);
Vesktop — готовый *продукт на чужом ядре*. Пересечение малое и дополняющее.

---

## 5. Чего не хватает в нашей версии (функции и детали)

### P0 — «полноценная оболочка» (у Vesktop из коробки, у нас — ноль)

| # | Функция | Зачем нам | Где черпать детали |
|---|---|---|---|
| 1 | Оконная оболочка: геометрия/multi-display guard, мин. размер, frameless+заголовок, прозрачность | базовый UX | `mainWindow.ts`, `windowsTitleBar.tsx` |
| 2 | Трей (иконка, unread-вариант, меню, клик-поведение) | фон/уведомления | `tray.ts` |
| 3 | Бейджи: Linux D-Bus (Unity), Windows overlay (1–10 + unread), macOS dock | заметность непрочитанного | `appBadge.ts`, `dbus.ts`, `libvesktop` |
| 4 | Автостарт: win/mac login items, linux `.desktop` + portal `requestBackground` | привычка пользователей | `autoStart.ts` |
| 5 | Deep links `discord://` + single instance + фокус второго инстанса | интеграция с ОС | `vesktopProtocol.ts`, `main.ts` |
| 6 | Уведомления: клик → фокус окна; дефолты; (мигание таскбара — опция) | базовый UX | `fixes.ts`, `taskBarFlash.ts` |
| 7 | Апдейтер с UI: проверка, прогресс, snooze/ignore, quitAndInstall | жизненный цикл продукта | `updater.ts` |
| 8 | Упаковка/установщики: deb/rpm/AppImage/NSIS/dmg + desktop/mime | поставка | `package.json build`, `release.yml` |
| 9 | Метаданные дистро (metainfo.xml, winget) | публикация | `meta.yml`, `winget-submission.yml` |
| 10 | Splash + первый запуск + About | первое впечатление | `splash.ts`, `firstLaunch.ts`, `about.ts` |
| 11 | Спеллчек (языки, suggestions, replace/add) | мессенджер-функция | `mainWindow.ts`, `VesktopNative`, `spellCheck.tsx` |
| 12 | Настройки продукта (свой стор + страница UI) | кастомизация | `settings.ts`, `components/settings/*` |

### P1 — стриминг/аудио и расширяемость

| # | Функция/деталь | У нас | Детали из Vesktop |
|---|---|---|---|
| 13 | Захват звука Linux (PipeWire-loopback, виртуальный микрофон) | медиа-слой RE есть, loopback-практики нет | `venmic.ts` + ScreenSharePicker: фильтры по `application.process.id`, `media.class: Stream/Input/Audio`, `node.virtual`, workaround `RecordStream`, only-speakers |
| 14 | Стрим-пикер: превью сетки, большой thumbnail, качество 480–2160/15–60fps, contentHint, «Entire System», аудио-подмодалка | нет | `ScreenSharePicker.tsx` (805 строк) |
| 15 | Переопределение качества go-live (bitrate/encode/capture/pixelCount) | нет | `patchStreamQuality` |
| 16 | Правки constraints: `deviceId:{exact}`, AGC, `resizeMode:none`, frameRate/resolution | нет | `fixStreamConstraints.ts`, `screenShareFixes.ts` |
| 17 | События стрима + автотумбл стример-мода | нет | `STREAM_CLOSE/UPDATE`, `STREAMERMODE` |
| 18 | Скрытие собственного виртуального устройства из списка устройств/модалки | нет | `hideVenmicInput`, `hideSwitchDevice` |
| 19 | Rich Presence для игр (arrpc-подобное) | нет | `arrpc/*`, `renderer/arrpc.ts` (MIT-референс) |
| 20 | Кастомные ассеты (splash/tray) + quickCss/темы из файла | нет | `userAssets.ts`, `constants.ts` |
| 21 | Копирование картинок в буфер | частично (IPC-карта) | `CLIPBOARD_COPY_IMAGE` |
| 22 | Приватность по умолчанию: WebRTC IP policy, отключение media-keys/MediaSession, fuses | нет | `main.ts`, `package.json build` |
| 23 | Защита от web-детектора devtools (у нас в данных — слепая зона) | нет | `devtoolsFixes.ts` |

### P2 — платформенная специфика

| # | Функция/деталь | Комментарий |
|---|---|---|
| 24 | Steam Deck (Game Mode, клавиатура, fullscreen, раскладка) | `steamOS.ts` |
| 25 | libvesktop (D-Bus: Unity count, акцент, portal-автостарт) | `packages/libvesktop` (C++/gio) |
| 26 | Windows: overlay-иконки бейджей, portable-режим, логика Alt+F4/Ctrl+Q | `appBadge.ts`, `constants.ts`, `mainWindow.ts` |
| 27 | macOS: vibrancy, traffic lights, `open-url`, notarize/entitlements | `mainWindow.ts`, `main.ts`, `build/` |
| 28 | Zoom-хаки (Qwertz, numpad) | `initMenuBar` |
| 29 | Отладочные окна (GPU, webrtc-internals) и «repair/reset» UX | `ipc.ts`, `clearData.ts` |
| 30 | Доставка собственных расширений/тем авто-обновлением (как Vencord-файлы) | `vencordLoader.ts` — идея канала обновления плагинов |

---

## 6. Что можно улучшить в нашей версии (по итогам сравнения)

1. **Разделить проект на два продукта:** (а) `discord-lab` — RE-инструментарий (есть, работает);
   (б) Rust-клиент — продукт. Смешивать гейтами не нужно: у клиента свои CI/релизы.
2. **Взять из Vesktop инженерные практики, а не код** (GPL):
   - изоляция (sandbox/contextIsolation/fuses) → для Rust+webview/своего UI: capabilities,
     подпись обновлений, запрет произвольных скриптов;
   - эшелонированная защита сессии: история с devtools-детектором показывает, что клиент может
     сам себя разлогинить — нам нужен аккуратный менеджер токенов;
   - privacy-дефолты (WebRTC IP policy особенно актуален с VPN — у нас в RE этого не было);
   - UA-честность: Vesktop спуфит Chrome-UA потому что обязан (web-клиент), но даёт опцию и
     предупреждение. Своему клиенту спуф не нужен (сценарий A), для C — экономия рисков.
3. **Стриминг — пересмотреть приоритет.** Linux-звук при шаринге — самая заметная фича Vesktop
   (ради неё его чаще всего ставят). Для паритета нашего клиента нужен PoC: PipeWire-захват
   (в Rust — `pipewire-rs`), фильтры узлов как у venmic, виртуальный микрофон. venmic — MPL-2.0,
   идеи/подходы заимствовать можно, код — только с соблюдением MPL.
4. **Продуктовые детали, которые легко забыть (собраны из Vesktop):** размер окна по умолчанию,
   `--start-minimized`, место данных (portable-режим), «Reset/Repair», статичный заголовок,
   бранчи stable/ptb/canary, автозакрытие меню, zoom-хоткеи.
5. **Обновления и поставка заранее:** канал релизов (теги → артефакты), подписанные сборки,
   авто-обновление, метаданные пакетов, winget/Flatpak. У Vesktop это ~4 workflow — воспроизводимо.
6. **Расширяемость — решить сейчас:** Vesktop = Vencord-файлы из релизов (не вшиты). Для нашего
   клиента это может быть доставка тем/плагинов/Lua-скриптов; заложить каталог и манифест.
7. **Добить наши слепые зоны данных** (под стриминг/голос UI): голосовая панель, меню сообщений,
   поиск, DM-список (уже в бэклоге §7.1 discord-qt-отчёта); плюс new: контракт `getUserMedia`
   граблей, `STREAM_*` flux-события, дефолты уведомлений, devtools-детектор.
8. **Лицензионная гигиена:** Vesktop/Vencord GPL-3.0, venmic MPL-2.0, arrpc MIT. Если делать
   открытый клиент — можно совместимо; если проприетарный — только идеи/контракты
   (как мы уже решили с discord-qt).

---

## 7. Насколько готов наш проект

### 7.1 Две оси

| Ось | Содержание | Оценка |
|---|---|---|
| **Знания (RE)** | Протокол, медиа, крипто, UI-токены, локали, ассеты, IPC, нативные модули | **≈85–90%** для сценария A (READINESS.md); для паритета оболочки-стриминга добор ~10–15% деталей (см. §5) |
| **Продукт (код+поставка)** | Rust-клиент, оболочка, апдейтер, упаковка, релизы | **≈5%**: спецификаций много, кода нет; есть только репозиторий-инструментарий с гейтами |

### 7.2 Матрица «готовность под задачу Vesktop-паритета»

| Функциональный блок | Данные RE | Спецификация | Код | Готовность блока |
|---|---|---|---|---|
| Протокол (шлюз/REST/ETF) | ✅ 85% | ✅ | ❌ | 30% (код = 0) |
| Голос/медиа/крипто | ✅ 85–95% | ✅ | ❌ | 30% |
| UI-система (токены/темы/локали) | ✅ 80–95% | ✅ | ❌ | 30% |
| Экраны (DOM-трассы) | 🟡 55% | 🟡 | ❌ | 20% |
| Десктоп-оболочка | ✅ 60–75% (IPC) + Vesktop-детали | 🟡 | ❌ | 20% |
| Стриминг/аудио (Linux со звуком) | 🟡 30–40% | ❌ | ❌ | 10% |
| Обновления/дистрибуция | ➖ | 🟡 (референс Vesktop) | ❌ | 10% |
| Ассеты | ✅ 95% (⚠ лицензии) | ✅ | ❌ | 30% |
| Расширяемость (плагины/темы) | ❌ | ❌ | ❌ | 0% |

**Итог:** исследовательская фаза практически закрыта (можно начинать вертикальный срез),
продуктовая фаза не начата. Обобщённо: **к «полноценному клиенту как продукту» проект готов
примерно на треть пути — ~85–90% знаний при ~5% продукта.** Vesktop-сравнение показывает,
что оставшиеся пробелы — оболочечные и стриминговые, и они закрываются инженерией, а не RE.

### 7.3 Блокеры и риски

| Риск | Влияние | Митигация |
|---|---|---|
| Лицензии ассетов Discord (шрифты/звуки/иконки) | нельзя распространять | свои ассеты (путь Vesktop: свои splash/tray/иконки бейджей) |
| Linux-звук при стриме | главная фича-паритет | PoC `pipewire-rs` + фильтры узлов; референс venmic (MPL, идеи) |
| Слепая зона devtools-детектора/токенов | безопасность сессии | спроектировать менеджер сессии аккуратно |
| «Целевая» юридика (C-сценарий) | NO-GO | остаёмся в A; Vesktop подтверждает, что и обёртке приходится жить в серой зоне патчей |
| Объём продукта (оболочка ОС × 3 платформы) | сроки | начать с Linux (как Vesktop), потом Win/mac |

---

## 8. Рекомендованный порядок работ

1. **Зафиксировать оболочный бэклог** (§5, P0) как задачи R4/R5 — с оценками (P0 ≈ 12 задач).
2. **Добить данные под голос/стриминг-UI**: голосовая панель, меню сообщений, поиск, DM-лист
   (CDP-трассы — окно согласования не нужно, копия жива); `getUserMedia`-грабли — из otчёта Vesktop.
3. **Мини-спецификация «оболочка»** (окно/трей/бейджи/уведомления/автостарт/deep links) — на
   основе Vesktop, но под Rust-стек (например, tao/wry/egui или своя оболочка) — решение по стеку.
4. **PoC медиа**: opus (Rust) + RTP-обёртка по `VOICE-CRYPTO.md` + PipeWire-капчер — вертикальный срез
   «голосовая комната в своём клиенте».
5. **Продуктовый каркас**: апдейтер (подписанные сборки), упаковка (deb/rpm/AppImage первыми),
   CI-релиз по тегам, metainfo; по образцу release.yml/meta.yml Vesktop (только процессы).
6. **Расширяемость** (темы/плагины) — решить, нужна ли; если да — каталог+манифест+обновление.

---

## 9. Юридика

- **Vesktop** — GPL-3.0-or-later; **Vencord** — GPL-3.0; **venmic** — MPL-2.0; **arrpc** — MIT.
  Код Vesktop/Vencord копировать в проприетарный продукт нельзя; идеи, контракты, архитектурные
  решения — можно. Код venmic (MPL) можно переиспользовать только с соблюдением MPL; мы этого
  не делаем (только подход). arrpc (MIT) — совместим с чем угодно, при сохранении копирайта.
- **Патчи web-клиента Discord** (патчи Vesktop) — серая зона ToS; для сценария A не нужны.
- **Ассеты Discord** (кэш) — интеллектуальная собственность; для публичного продукта — свои.

---

## 10. Приложение: карта файлов Vesktop (сокращённо)

```
src/
├─ main/                     — Electron main
│  ├─ index.ts, main.ts      — точка входа, сингл-инстанс, фичи Chromium, WebRTC policy
│  ├─ mainWindow.ts (518)    — окно, меню, geometry, spellcheck, splash, загрузка URL
│  ├─ cli.ts                 — CLI-флаги (repair, user-agent*, start-minimized)
│  ├─ settings.ts, events.ts — стор настроек/состояния, шина событий
│  ├─ tray.ts, appBadge.ts, dbus.ts — трей, бейджи, D-Bus
│  ├─ updater.ts             — electron-updater + окно апдейтера
│  ├─ screenShare.ts         — display media handler, loopback
│  ├─ venmic.ts              — PipeWire-захват (виртуальный микрофон)
│  ├─ userAssets.ts, splash.ts, firstLaunch.ts, about.ts — ассеты/экраны
│  ├─ autoStart.ts, vesktopProtocol.ts, mediaPermissions.ts
│  ├─ ipc.ts, ipcCommands.ts — IPC-хендлеры/команды в renderer
│  └─ utils/ (vencordLoader, clearData, steamOS, popout, http, …)
├─ preload/                  — typed IPC-мост (VesktopNative)
├─ renderer/                 — внутри web-клиента
│  ├─ index.ts, fixes.ts, arrpc.ts, settings.ts, appBadge.ts
│  ├─ components/ScreenSharePicker.tsx (805) — пикер + аудио-UI
│  ├─ components/settings/*  — страница настроек «Vesktop»
│  └─ patches/* (15)         — regexp-патчи Discord web (titlebar, стримы, devtools, …)
├─ shared/                   — IpcEvents, defaultSettings, utils
└─ packages/libvesktop/      — C++/gio хелпер (D-Bus, акцент, portal)
scripts/build/*              — esbuild-сборка, injectReact, sandboxFix, beforePack/afterPack
static/views/*               — splash/about/first-launch/updater (HTML/CSS)
.github/workflows/*          — test, release, meta, winget, update-vencord-dev
```

---

## 11. Источники и проверки

- Клон Vesktop: `a02035b` (05.10.2026), v1.6.7; GitHub API: 8513★/471 форков/81 issues,
  license GPL-3.0, push 07.10.2026; лицензии зависимостей проверены через API
  (venmic MPL-2.0, arrpc MIT).
- Прочитаны ключевые модули: `main.ts`, `mainWindow.ts`, `cli.ts`, `settings.ts`, `tray.ts`,
  `appBadge.ts`, `screenShare.ts`, `venmic.ts`, `updater.ts`, `mediaPermissions.ts`, `dbus.ts`,
  `userAssets.ts`, `vencordLoader.ts`, `autoStart.ts`, `arrpc/*`, `preload/VesktopNative.ts`,
  `renderer/fixes.ts`, `renderer/arrpc.ts`, `renderer/index.ts`, `ScreenSharePicker.tsx` (полностью),
  патчи (`windowMethods`, `windowsTitleBar`, `screenShareFixes`, `fixStreamConstraints`,
  `streamerMode`, `taskBarFlash`, `platformClass`, `devtoolsFixes`, `hideVenmicInput`,
  `hideSwitchDevice`, `hideDownloadAppsButton`, `enableNotificationsByDefault`), `constants.ts`,
  workflows и build-конфигурацию.
- Наши источники: `r2-net/READINESS.md`, `r2-net/REPORT.md` (§5–7, §10–11), `r2-net/MAP.md`,
  `ux-research/GAP-ANALYSIS.md`, `discord-qt-analysis/ANALYSIS.md` (§6–7), репозиторий
  `discord-lab` (README, CLI HELP, `desktop-core-notes.md`).
