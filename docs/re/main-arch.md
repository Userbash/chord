# Discord 1.0.162 (Linux x64) — main-процесс desktop-клиента: разбор распакованного JS

Отчёт по десктопному main-процессу на основе распакованного asar (`asar-extract/`).
Дополняет (и **не дублирует**) два соседних артефакта:

* `backend-ipc.md` / `backend-ipc.json` — карта 168 IPC-каналов `DISCORD_*`, preload-whitelist, группы каналов;
* `native-re.md` / `native-re.json` — инвентарь нативных `.node`-модулей и их экспортов.

Здесь: **как main-процесс устроен** (запуск, окна, IPC-обвязка и полные цепочки, нативные биндинги,
автообновление, фичефлаги, сеть, crashpad, трей/меню/протоколы, голос и Clips в main) — с цитатами.

## 0. Входные данные, инструмент и формат цитат

| Файл (относительно `asar-extract/`) | Размер | Строк в файле | Модулей webpack |
|---|---|---|---|
| `app/bundle.js` | 3 066 301 Б | 519 | 825 |
| `core/bundle.js` | 402 296 Б | 8 | 81 |
| `core/mainScreenPreload.js` | 1 105 621 Б | 545 (489 непустых) | renderer-preload |
| `app/splashScreenPreload.js` | 9 333 Б | 30 | splash-preload |

Проверка: `md5 app/bundle.js = 23fbdabaafb7acd0d57b2b2aab60fd49`, `md5 core/bundle.js = 08edd888cca8426f0cbc6c3bb2edbf17`.

**Соглашение о цитатах.** Бандлы минифицированы: файлы состоят из гигантских строк
(до 849 148 байт — `core/bundle.js:6`), поэтому `файл:строка` различает места слабо. Каждая цитата даётся в виде

```
файл:строка  (#idМодуля, тело @byteStart..@byteEnd; цитируемый фрагмент @byteN)
```

и дополнительно везде указан diff-нейтральный «byte offset» — позиция байта в файле, по ней факт
проверяется автоматически. Сниппеты **отформатированы** (переносы/отступы добавлены для читаемости),
исходный текст минифицирован. Точки входа модулей: `app` — `__webpack_require__.s = 66321`;
`core` — `__webpack_exports__.startup` (см. §1.4).

Инструмент разбора (собственный, вне репозитория): `/home/sanya/re-deep/main-arch-tool.mjs`
(`--list/--line/--at/--grep/--module` — печатает raw line + id модуля + byte offset),
`/home/sanya/re-deep/main-arch-find.mjs` (пакетный поиск «needle → файл:строка #модуль @byte»),
дампы в `/home/sanya/re-deep/main-arch-work/` (`m-app-*.txt`, `m-core-*.txt`, `offsets.txt`).

---

## 1. Запуск main-процесса (bootstrap)

### 1.1 Две точки входа: app-режим и overlay-host

`app/bundle.js` — entry-модуль `#66321` решает, что грузить: обычное приложение (`#23151`) или
отдельный процесс хоста оверлея (режим `--overlay-host`, свой апдейтер и `standalone_host.js`):

```js
// app/bundle.js:12  (#66321, тело @51355..52280)
let mode = process.argv.includes("--overlay-host") ? "overlay-host" : "app";
if ("app" === mode) __webpack_require__(23151);
else if ("overlay-host" === mode) {
    __webpack_require__(43731).init();
    if (paths.updatePathsForNewUpdater(buildInfo), buildInfo.disableUpdater)
        __webpack_require__(47924).initPathsOnly(buildInfo);
    else { if (!tryInitUpdater(buildInfo, NEW_UPDATE_ENDPOINT)) throw Error("Failed to initialize modules in overlay host.");
        getUpdater()?.startCurrentVersionSync({ allowObsoleteHost: !0 }) }
    __webpack_require__(30885)("discord_overlay2/standalone_host.js");   // @52159
}
```
(`--overlay-host`: @51603; `__webpack_require__(30885)("discord_overlay2/standalone_host.js")`: @52159.)

### 1.2 Ранняя фаза `#23151` (до `app.whenReady`)

```js
// app/bundle.js:10  (#23151, тело @15212..25870)
"linux" === process.platform && void 0 === process.env.PULSE_LATENCY_MSEC &&
    (process.env.PULSE_LATENCY_MSEC = "30");                                  // @15343
...
let paths = __webpack_require__(37948); paths.init(buildInfo);
let blackbox = __webpack_require__(81598), moduleDataPath = paths.getModuleDataPath();
null != moduleDataPath && blackbox.initialize(moduleDataPath, buildInfo),
    blackbox.captureMinidumpFromCrashpadSync();                               // @16369
let crashReporterSetup = __webpack_require__(62457);
crashReporterSetup.init(buildInfo, { sentry, getTransport: dsnFunc => browser.makeMultiplexedTransport(...) }); // @16488
```

Порядок и состав фазы (все в `#23151`):

| шаг | факт | offset |
|---|---|---|
| 1 | Linux-специфика: `PULSE_LATENCY_MSEC=30` (задержка PulseAudio), `app.setName(name + "-" + channel)`, `setDesktopName("discord" | "discord-<channel>")` | @15343–@15600 |
| 2 | `errorHandler.init()` → `paths.init(buildInfo)` → `appSettings.init()` → `paths.updatePathsForNewUpdater()` | @15600–@16000 |
| 3 | blackbox (лог/крэш-файлы) init + `captureMinidumpFromCrashpadSync()` | @16369 |
| 4 | `crashReporterSetup.init(buildInfo, {...})` (Sentry + транспорт) | @16488 |
| 5 | `app.disableHardwareAcceleration()` если `enableHardwareAcceleration=false`; `--autoplay-policy=no-user-gesture-required` | @16000–@16300 |
| 6 | `disabled-features` = Win+Mac-специфичный список + linux `WaylandWpColorManagerV1`; win32: `disable-background-timer-throttling`, `disable-renderer-backgrounding` | @15800–@16000 (список флагов — @15700) |
| 7 | linux+`openH264Enabled`: `--enable-libopenh264` + `--openh264-library-path=<assetCache>/openh264/libopenh264-2.5.1-linux64.7.so` | @19732 |
| 8 | win32: `setAppUserModelId`; squirrel `handleStartupEvent` → `pendingAppQuit` | @21000–@21500 |
| 9 | `allowMultipleInstances = hasArgvFlag("--multi-instance")`, `isFirstInstance = !!allowMultipleInstances || app.requestSingleInstanceLock()` | @21961 |
| 10 | `--exit-immediately` → `app.exit(0)`; `extractUrlFromArgs(argv)` (ищет `--url <u>` после `--` либо `discord://…`) | @22445/@21998 |
| 11 | `app.on("ready")` → `registerDesktopConnectivityTests(session.defaultSession, app.getSystemLocale())`; при `DESKTOP_TTI_DNSTCP_WARMUP` — `warmupDesktopTTIDnsTcp()` | @23148/@23284 |
| 12 | `will-finish-launching` → `open-url` (@23588), `continue-activity` (macOS) → `openOrQueueUrl` | @23588–@24000 |
| 13 | `beforeReadyProtocolRegistration()` (reg `disclip://`) → `setGPUFlags().then(app.whenReady)` | @24154/@24188 |

`setGPUFlags()` (там же, @22900–@24100) — единственное место, где работает GPU-workaround:
`app.getGPUInfo("basic")` → сверка `vendorId/deviceId` c таблицей NVIDIA (`[4318, 4928…6141]`) и
`app.commandLine.appendSwitch("disable_accelerated_hevc_decode","1")` только для win32.

### 1.3 `whenReady` → splash/updater → `core.startup()`

```js
// app/bundle.js:10  (#23151  @24188..@25500)
setGPUFlags().then(app.whenReady).then(() = > {
    paths.cleanOldVersions(buildInfo),
    startupMenu = __webpack_require__(1248), Menu.setApplicationMenu(startupMenu),   // @24463
    __webpack_require__(33745).update(startMinimized, () => {                        // @24575
        let core = requireNative("discord_desktop_core");                            // @25224
        coreModule = core, core.startup({ Constants, GPUSettings, analytics, appSettings, autoStart,
            buildInfo, crashReporterSetup, logger, moduleUpdater, paths, splashScreen, updater }); // @25283
        null != initialUrl && (core.handleOpenUrl(initialUrl), initialUrl = null);
    }, () => { coreModule.setMainWindowVisible(!startMinimized, startInactive) })
})
```

* второй инстанс: `app.on("second-instance", …)` → `openOrQueueUrl(url)` + `focusSplash()` (@22712);
* `pendingAppQuit` (squirrel) → «Startup prevented.»; иначе `isFirstInstance`-ветка, иначе `app.quit()` (@25600);
* `app.on("ready")` дополнительно принимает `launchInfo.userInfo.fallbackDeepLink` → `openOrQueueUrl` (@23473).

### 1.4 `core.startup()` (порядок инициализации core)

```js
// core/bundle.js:6  (entry-модуль `__webpack_exports__`, тело модуля начинается ~@398000)
__webpack_exports__.startup = function (bootstrapModules) {
    performance.mark("coremodule-index-startup"), __webpack_require__(9895).init(bootstrapModules), ...
    let { moduleUpdater } = __webpack_require__(2979), { updater } = __webpack_require__(1623);
    Constants.init(bootstrapModules.Constants);
    let appFeatures = __webpack_require__(5976); appFeatures.init();
    bootstrapModules.GPUSettings.replace(GPUSettings), __webpack_require__(8730).init(),        // cacert.pem
    let app = __webpack_require__(4943); app.injectBuildInfo(...), app.injectModuleUpdater(moduleUpdater), app.injectUpdater(updater);
    crashReporterSetup.isInitialized() || crashReporterSetup.init(buildInfo), __webpack_require__(7138),
    __webpack_require__(1353).injectFeaturesBackend(appFeatures.getFeatures()), ...
```

Продолжение цепочки (там же, @400250 ff.), порядок `require`:

| шаг | модуль | назначение |
|---|---|---|
| 1 | `#8730` | `init()` — загрузка `data/cacert.pem` (см. §7.4) |
| 2 | `#7912` | handler `ACCESSIBILITY_GET_ENABLED` |
| 3 | `#4943` | app-IPC (`APP_*`), `injectBuildInfo/injectModuleUpdater/injectUpdater` |
| 4 | `#7126` | clipboard-IPC (win32 `CF_BITMAP`/`CF_DIB`) |
| 5 | `#6577` | `CONSTANTS_GET` (отдаёт `APP_NAME/APP_ID/API_ENDPOINT/UPDATE_ENDPOINT`) |
| 6 | `#7138` | crash-metadata IPC (§8.4) |
| 7 | `#2236` | `DESKTOP_CAPTURER_GET_SOURCES` |
| 8 | `#1353` | `injectFeaturesBackend(appFeatures.getFeatures())` — фичефлаги в main (§6.1) |
| 9 | `#9473` | `FILE_MANAGER_*`-хендлеры |
| 10 | `#8750` | `HARDWARE_GET_DISPLAY_COUNT` |
| 11 | `#7635` | `setupClipsProtocol()` + clips-IPC (§10.3) |
| 12 | `#3132` / `#1742` | `SPELLCHECK_*` / `TRACING_CAPTURE_PERFORMANCE` |
| 13 | `#3386` | `windowNative` (оконные IPC, §3.4) |
| 14 | `#5544` | webauthn (§3.6) |

после чего заполняются `global.crashReporterMetadata/mainAppDirname/features/appSettings/mainWindowId/moduleUpdater`,
затем:

```js
let enableDevtoolsSetting = global.appSettings.get("DANGEROUS_ENABLE_DEVTOOLS_ONLY_ENABLE_IF_YOU_KNOW_WHAT_YOURE_DOING", !1),
    enableDevtools = "stable" !== buildInfo.releaseChannel || enableDevtoolsSetting,
    createApplicationMenu = __webpack_require__(7814);
electron_1.Menu.setApplicationMenu(createApplicationMenu(enableDevtools));
"darwin" === process.platform && (...)systemPreferences.setUserDefault("SquirrelMacEnableDirectContentsWrite","boolean",!0),
(mainScreen = __webpack_require__(5203)).init();
let { getWindow: getPopoutWindowByKey, getAllWindows: getAllPopoutWindows, setNewWindowEvent } = __webpack_require__(9698);
windowNative.injectGetWindow(key => null == key ? BrowserWindow.fromId(mainScreen.getMainWindowId()) : getPopoutWindowByKey(key), ...),
setNewWindowEvent(window => windowNative.newWindowEvent(window)), frameDebugger.injectFrameDebugger({...});
```

Итог: `core.startup()` собирает main-окружение из инжектируемых бандлов и **сам ничего не грузит из сети**.

---

## 2. Окна (BrowserWindow / webPreferences)

### 2.1 Splash-окно (`app/bundle.js:12`, `#53006`, тело @86584..101462)

```js
// app/bundle.js:12 (#53006  @91315..@91700)
useWebp = appSettings.getSettings()?.get("DESKTOP_TTI_SPLASH_USE_WEBP", !0) === !0,
windowConfig = {
    width: 300, height: LOADING_WINDOW_HEIGHT, transparent: !1, frame: !1, resizable: !1, center: !0, show: !1,
    webPreferences: {
        nodeIntegration: !1, sandbox: !1, contextIsolation: !0,
        preload: path_1.default.join(__dirname, "splashScreenPreload.js"),        // @91515
        additionalArguments: [`--splash-media-format=${useWebp ? "webp" : "webm"}`] // @91601
    }
};
splashWindow = new electron_1.BrowserWindow(windowConfig)                        // @91653
```

Каналы splash (имя канала в main — **без** префикса, префикс `DISCORD_` добавляет обёртка `#50902`,
см. §3.1; в preload-скрипте имена полные). Импорт обёртки в `#53006`:
`ipcMain_1=__importDefault(__webpack_require__(50902))` (`app/bundle.js:12 #53006 @95489`):

| канал | где | offset |
|---|---|---|
| `SPLASH_SCREEN_READY` (главный resend состояния) | `ipcMain_1.default.on("SPLASH_SCREEN_READY", …)` | @92471 |
| `SPLASH_SCREEN_QUIT` | `ipcMain_1.default.on("SPLASH_SCREEN_QUIT")` → `app.quit()` | @93327 |
| `SPLASH_UPDATE_STATE` | `webContentsSend(splashWindow, event, …)` — функция отправки всех splash-состояний | @96448 |
| `SPLASH_SCREEN_QUOTE` | отправка цитаты | @92896 |

Splash-стейт-машина (там же): события `CHECKING_FOR_UPDATES/@88338`, `DOWNLOADING_UPDATES`,
`INSTALLING_UPDATES`, `UPDATE_FAILURE`, `LAUNCHING`; `class TaskProgress` (@100500) считает
`tasks` по `package_sha256`; «ранний» режим (`DESKTOP_TTI_EARLY_UPDATE_CHECK`) шлёт состояние в
окно, даже если оно ещё не `DOMContentLoaded`. UI (диалог `update-manually`, счётчики, видео/вебп)
рисуется в `app/splashScreenPreload.js` (см. также §5.3).

### 2.2 Главное окно (`core/bundle.js:1`, `#5203`, тело @119531..156960)

```js
// core/bundle.js:1 (#5203  @141300..@142450)
let additionalArguments = ["--enable-node-leakage-in-renderers"];                 // @141476
"win32" === process.platform && settings?.get("DESKTOP_NATIVE_PREMAP", !1) === !0 &&
    additionalArguments.push("--enable-native-premap"),                          // @141637
exports.WEBAPP_ENDPOINT.startsWith("http://") && !...startsWith("http://localhost") &&
    additionalArguments.push(`--unsafely-treat-insecure-origin-as-secure=${exports.WEBAPP_ENDPOINT}`);
let mainWindowOptions = {
    title: "Discord", backgroundColor: getBackgroundColor(), width: 1280, height: 720,
    minWidth: MIN_WIDTH, minHeight: MIN_HEIGHT, transparent: !1, frame: !1, resizable: !0, show: isVisible,
    webPreferences: {
        enableBlinkFeatures: "EnumerateDevices,AudioOutputDevices", nodeIntegration: !1, sandbox: !1,
        preload: path_1.default.join(__dirname, "mainScreenPreload.js"),
        spellcheck: !0, contextIsolation: !0, additionalArguments, devTools: ENABLE_DEVTOOLS
    }
};
"linux" === process.platform && (mainWindowOptions.icon = path_1.default.join(path_1.default.dirname(app.getPath("exe")), "discord.png")); // @142388
```

* `MIN_WIDTH/MIN_HEIGHT` = настройки, по умолчанию **940×500** (`settings?.get("MIN_WIDTH", 940)`, @137043);
  сохранённые `bounds` из `getSavedWindowBounds()` восстанавливаются (@144495);
  `setMenuBarVisibility(!1)`, `IS_MAXIMIZED`→`maximize()` / `IS_MINIMIZED`→`minimize()` применяются (@147172);
* **навигационный allow-list**: `session.setPermissionRequestHandler` (@144769) и
  `setPermissionCheckHandler` (@145714): `notifications/fullscreen/pointerLock/clipboard-sanitized-write` —
  только origin == `WEBAPP_ENDPOINT`; `media` — только `AllowedMediaOrigins.K_ID / K_ID_V2 / K_ID_FACE_SCAN`
  или `WEBAPP_ENDPOINT` (@145300);
* `will-navigate` + `checkUrlOriginMatches(url, WEBAPP_ENDPOINT)` → иначе `evt.preventDefault()` (@151050);
* `setWindowOpenHandler` (@147286): `frameName.startsWith("DISCORD_") && sameOrigin && "/popout" === path`
  → `popoutWindows.openOrFocusWindow(...)`; иначе `shouldOpenExternalUrl(url) && saferShellOpenExternal(url)`
  + `{action:"deny"}` (@147448);
* `render-process-gone` (@150000): пишет `processUtilsSettings.rendererCrashReason/ExitCode`,
  `lastRunsStoredInformation = currentStoredInformation`; `"killed"` → `app.quit()`;
  повторный краш < 5 c → `app.quit()`; иначе — `webContents.reload()` (при `DISCORD_DISABLE_RESTART` — quit);
* `child-process-gone` (@121209): для `GPU/Utility` — троттлинг 30 c и `webContentsSend("CRASH_REPORTER_NEW_CRASH", details)`
  (@121602); `reason ∈ {crashed, oom}` → Sentry-событие (там же);
* `did-fail-load` (@147680): retry через `connectionBackoff` (1 c … 20 c), `loadMainPage()`;
* `select-webauthn-account` (@146647): при >1 аккаунте — `dialog.showMessageBoxSync` с выбором.

Куда грузится окно: `WEBAPP_ENDPOINT` — env `DISCORD_WEBAPP_ENDPOINT` → setting `WEBAPP_ENDPOINT` →
для `stable` — миграционная логика `discord.com`/`discordapp.com` (marker-файлы `userDataCache.json`,
`domainMigrated`), для остальных — `https://<channel>.discord.com` (см. цитату §2.2 выше, @135531 ff.);
`URL_TO_LOAD = WEBAPP_ENDPOINT + WEBAPP_PATH` (`WEBAPP_PATH` = setting либо `WEBAPP_PATHS.APP`/`CONFERENCE_MODE`
+ `?_=<Date.now()>`; @136988).

### 2.3 Popout и out-of-process overlay (`core/bundle.js:1`, `#9698`, @178984..183232)

```js
// core/bundle.js:1 (#9698; ALLOWED_FEATURES @~180100, DEFAULT_POPOUT_OPTIONS @182761, overlay-ветка @180977..)
ALLOWED_FEATURES = ["width","height","left","top","resizable","movable","alwaysOnTop","frame","transparent",
                    "hasShadow","closable","skipTaskbar","backgroundColor","menubar","toolbar","location",
                    "directories","titleBarStyle","outOfProcessOverlay","focusable"];
DEFAULT_POPOUT_OPTIONS = { title: "Discord Popout", backgroundColor: "#2f3136", minWidth: 320, minHeight: 180,
    transparent: !1, frame: !1, resizable: !0, show: !0, titleBarStyle: "darwin" === process.platform ? "hidden" : void 0,
    webPreferences: { nodeIntegration: !1, nativeWindowOpen: !0, enableRemoteModule: !1, contextIsolation: !0 } };
...
if (windowOptions.outOfProcessOverlay) {
    windowOptions.title = "Discord Overlay", windowOptions.show = !1, windowOptions.backgroundColor = "#00000000",
    windowOptions.type = "toolbar", windowOptions.alwaysOnTop = !0,          // @180977 ff.
    windowOptions.webPreferences.preload = path_1.default.join(__dirname, "mainScreenPreload.js"),
    windowOptions.frame = !1, windowOptions.transparent = !0
}
```

* `init()` объявляет фичефлаг `popout_windows` (см. §6.2), `setupPopout` навешивает `newWindowEvent`;
* `hasInit`-идемпотентность; `getWindow(key)`/`getAllWindows()` — из них строится `windowNative.injectGetWindow` (§1.4).

### 2.4 Прочие окна

* **трей-«подсказка»**: `atomTray.displayBalloon({title, content})` в `#1275` (`core/bundle.js:3` @193900, ключ настройки `trayBalloonShown`);
* **thumbar** (`#9090`, `apps`), **frame-debugger** (`#7026`, CDP через `webContents.debugger`) — не окна, но через них шлются `FRAME_DEBUGGER_*` (см. backend-ipc.md);
* `WINDOW_OPEN_DOCUMENT_PIP` (@108850) выполняет `document.getElementById('__document_pip_trigger')?.click()`
  в renderer’е (`core #3386`) — единственный «window-хайджек» через `executeJavaScript`.

---

## 3. IPC-обвязка и полные цепочки (паттерн)

### 3.1 Префиксация `DISCORD_` — три уровня

```js
// core/bundle.js:1 (#3716 @113790)
exports.getDiscordIPCEvent = function (ev) { return discordPrefixRegex.test(ev) ? ev : `DISCORD_${ev}`; };
let discordPrefixRegex = /^DISCORD_/;
```
```js
// core/bundle.js:1 (#2799 @118889..119473) — обёртка ipcMain для core
exports.default = { on: (event,cb) => ipcMain.on(getDiscordIPCEvent(event), cb),
                    reply: (event,channel,...args) => event.sender.send(getDiscordIPCEvent(channel), ...args),
                    handle: (event,cb) => ipcMain.handle(getDiscordIPCEvent(event), cb) };
```
```js
// app/bundle.js:12 (#50902 @52339..52903) — та же обёртка в app-bundle (свой экземпляр)
exports.default = { on: (event, callback) => electron_1.ipcMain.on(getDiscordIPCEvent(event), callback), ... };
```

Плюс «типизированный» фасад `DiscordIPC` (core):

```js
// core/bundle.js:1 (#1999 @112492..113550)
exports.DiscordIPC = {
    main: class { static on(channel, listener) { return electron_1.default.ipcMain.on(channel, (...args) => listener.apply(this,args)); }
                  static handle(channel, listener) { return electron_1.default.ipcMain.handle(channel, ...); } },
    renderer: class { static sendSync(...args) { return ipcRenderer.sendSync.apply(...); } static invoke(...args) { ...ipcRenderer.invoke... } }
};
```

Перечисление каналов — `core #595` (`exports.IPCEvents` @158765..171582, 168 имён с полным префиксом
`DISCORD_*`) — **не дублирую**, полная карта и preload-whitelist (`core/mainScreenPreload.js:46`,
`RENDERER_IPC_SEND_WHITELIST/RENDERER_IPC_INVOKE_WHITELIST`) в `backend-ipc.md`.

Практическое следствие: в `core`-модулях каналы берутся из enum (полные имена), в `app`-модулях —
короткие имена через `#50902`, в preload — полные строки. Поэтому grep по `DISCORD_SPLASH_*` в
`app/bundle.js` **не даёт** совпадений — имя собирается в рантайме (`DISCORD_` + `SPLASH_SCREEN_READY`).

### 3.2 Полная цепочка A — уведомления: `DISCORD_NOTIFICATIONS_SEND_NOTIFICATION`

```
renderer (backend-ipc.md §4) --invoke--> DISCORD_NOTIFICATIONS_SEND_NOTIFICATION
  -> main: core #7592 @78945  handler -> requireLoadedLib().sendNotification(options)
  -> native: discord_notifications (require(modulePath)) @80021, setDataPath/setCallbacks @80212
  -> ответ: normalizeSendResult(...) -> resolve в renderer
  -> асинхронно: native -> callback(action, identifier, userText, fallbackDeepLink)
     -> sendToAllWindows(DISCORD_NOTIFICATIONS_RECEIVED_RESPONSE) @80313 -> все окна
```

```js
// core/bundle.js:1 (#7592 @78945..79000 и @80150..80320)
DiscordIPC_1.DiscordIPC.main.handle(DiscordIPC_1.IPCEvents.NOTIFICATIONS_SEND_NOTIFICATION, async (_event, options) =>
    (0, notificationModuleStatus_1.normalizeSendResult)(await requireLoadedLib().sendNotification(options)));
...
let modulePath = (0, nativeModules_1.getModulePath)("discord_notifications") ?? path_1.default.join(moduleDataPath, "discord_notifications"),
    lib = require(modulePath);
lib.setDataPath(modulePath), lib.setCallbacks((action, identifier, userText, fallbackDeepLink) => {
    sendToAllWindows(DiscordIPC_1.IPCEvents.NOTIFICATIONS_RECEIVED_RESPONSE, action, identifier, userText, fallbackDeepLink) },
    _identifier => ["badge","banner","list","sound"], identifier => sendToAllWindows("USER_SETTINGS_OPEN", identifier, "Notifications")),
    nativeLib = lib, moduleStatus = loadedStatus(...), getFeatures().declareSupported("notifications"), ...declareSupported("notifications_provisional")
```

**Важно (Linux):** ветка загрузки — `utils_1.isOSX && kernelMajor >= 21` либо `utils_1.isWindows && Notification.isSupported()`;
на `linux` ни одна не выполняется: начальный статус `moduleStatus = unsupportedStatus(process.platform)` (@78250),
`sendNotification` всегда бросает ошибку «module unavailable». Т.е. в Linux-сборке `discord_notifications`
**не загружается вообще**, а `notifications` не объявляется в фичефлагах (§6.2).

### 3.3 Полная цепочка B — установка нативного модуля: `DISCORD_NATIVE_MODULES_INSTALL`

```
renderer/preload NativeModules.ensureModule(name)   (core/mainScreenPreload.js:46 — requireModule/ensureModule)
  --invoke--> DISCORD_NATIVE_MODULES_INSTALL
  -> main: core #216 electron.ipcMain.handle(NATIVE_MODULES_INSTALL, ...) @74005
     - новый апдейтер: newUpdaterInstall(updater, name) -> updater.installModule(name) + updater.commitModules({}) @73568/@73608
     - legacy: injectedModuleUpdater.install(name,false) + ожидание события INSTALLED_MODULE
  -> native: updater.node (app #47421) -> install/commit на хосте
  -> commitModules: core #216 commitModules @73608 / app #47421 _commitModulesInner @164666
     -> app #27453 addGlobalPath(<root>/app-<ver>/modules/<name>-<version>) @147000 (тело модуля @146799)
  --ответ--> resolve; preload вызывает DISCORD_NATIVE_MODULES_GET_PATHS (sync)
     -> core #216 getPathsResponse() = {mainAppDirname, browserModulePaths: getGlobalPaths()} @73366
     -> preload обновляет nativeModule.paths и делает require(<modulePath>)
```

```js
// core/bundle.js:1 (#216: деструктуризация @73250, getPathsResponse @73366, newUpdaterInstall @73568,
//                           NATIVE_MODULES_INSTALL @74005, GET_MODULE_PATH-хендлер @75414)
electron.ipcMain.on(NATIVE_MODULES_GET_MODULE_PATH, (event, name) => { event.returnValue = getModulePath(name) ?? null; });
...
function getModulePath(name) { let newUpdater = injectedUpdater?.getUpdater(); return null != newUpdater ? newUpdater.committedModulePaths.get(name) ?? null : null; }
async function newUpdaterInstall(updater, moduleName) {
    try { await updater.installModule(moduleName), await updater.commitModules({}) } catch (e) { throw Error(`Failed to install ${moduleName}: ${e}`) }
}
```
```js
// app/bundle.js:15 (#47421 @17700 ff., _commitModulesInner)
let { addGlobalPath, globalPathExists } = __webpack_require__(27453), hostPath = this._getHostPath(), modulesPath = path.join(hostPath, "modules");
for (let module1 in versions.current_modules) { let moduleVersion = versions.current_modules[module1],
    moduleSearchPath = path.join(modulesPath, `${module1}-${moduleVersion}`);
    this.committedModules.has(module1) || globalPathExists(moduleSearchPath) ||
    (this.committedModules.add(module1), addGlobalPath(moduleSearchPath), this.committedModulePaths.set(module1, path.join(moduleSearchPath, module1))) }
```

Механика резолва — `app #27453`/`core #7453` (одинаковый код, дублирован в оба бандла):
патч `Module._resolveLookupPaths`, добавляющий `Module.globalPaths` в конец `parent.paths`
(`core/bundle.js:6 #7453 @227925`, `app/bundle.js:15 #27453 @146799`). Именно это позволяет
`requireNative(...)` (см. §4.1) находить свежеустановленный модуль как «обычный» npm-пакет.

### 3.4 Полная цепочка C — `DISCORD_WINDOW_GET_NATIVE_HANDLE` (нативное окно)

```
renderer: ChannelActions.getNativeHandle(key)  (preload core/mainScreenPreload.js)
  --invoke--> DISCORD_WINDOW_GET_NATIVE_HANDLE
  -> main: core #3386 @111501 handler -> injectedGetWindow(key)
  -> native: BrowserWindow.getNativeWindowHandle() -> Buffer -> .readInt32LE().toString(10) @111604
  -> answer: string (десятичный id нативного окна) -> renderer (напр. для клика/записи/статистики оверлея)
```

```js
// core/bundle.js:1 (#3386 @111501..111610)
DiscordIPC_1.DiscordIPC.main.handle(DiscordIPC_1.IPCEvents.WINDOW_GET_NATIVE_HANDLE, (_, key) => {
    let win = injectedGetWindow(key);
    return null != win ? Promise.resolve(win.getNativeWindowHandle().readInt32LE().toString(10)) : Promise.resolve(null)
});
```

Замечание: чтение ровно 4 байт (`readInt32LE`) корректно для X11/Windows HWND-совместимых размеров и
является историческим выбором Discord; для 64-битных указателей Wayland это потенциально «усечённый» id
(помечено в unknowns).

### 3.5 Полная цепочка D — Clips: `DISCORD_RENDER_MONTAGE_RECIPE` (spawn EXE) и `disclip://`

```
renderer --invoke--> DISCORD_RENDER_MONTAGE_RECIPE(recipeJson, outputPath, metadata)
  -> main: core #7635 renderMontageRecipe @38211
     - modulePath = getModulePath("discord_clips") ?? <moduleDataPath>/discord_clips   (@38000)
     - exePath   = <modulePath>/discord_clips(.exe)                                   (@38060)
     - recipes: inputPath проверяется verifyDiscordMP4(inputPath) (@38300)
  -> native процесс: childProcess.spawn(exePath, ["--render-recipe", recipeJson, outputPath, metadata]) (@39418)
  -> ответ: code 0 -> verifyNewDiscordMP4Path(outputPath) -> resolve; иначе reject (частичный output удаляется)
renderer --обращение--> disclip://<file>.mp4 (protocol.handle @26431)
  -> main: Range-запросы -> 206/416 через "Content-Range" (@26800..27000); доступ только внутри clip-каталогов; verifyDiscordMP4 (@26816)
```

```js
// core/bundle.js:1 (#7635  @26431 и @39418)
electron_1.protocol.handle(constants_1.DISCORD_CLIP_PROTOCOL, async function (request) { ... verifyDiscordMP4(pathname) ... });
...
let child = childProcess.spawn(exePath, ["--render-recipe", recipeJson, outputPath, metadata], { ... });
```

`DISCORD_CLIP_PROTOCOL = "disclip"` — `app #29157` (`app/bundle.js:15 #29157 @140718`), регистрируется
как privileged **до** `app.whenReady` (`beforeReadyProtocolRegistration`, `app #46312`), а сам
`protocol.handle` ставится в `core #7635 setupClipsProtocol()` (`@26390`), вызываемой из `core.startup()`.

### 3.6 Дополнительно (короткая) — `DISCORD_WEBAUTHN_REGISTER_MAC` и `DISCORD_PROCESS_UTILS_GET_SYSTEM_METRICS`

```js
// core/bundle.js:1 (#5544 @105305..106000) — только macOS, kernel >= 21
function callNative(method, challenge) {
    if (null == moduleDataPath) return Promise.reject(Error("Module data path unset"));
    let webAuthnPath = (0, nativeModules_1.getModulePath)("discord_webauthn") ?? path_1.default.join(moduleDataPath, "discord_webauthn");
    return new Promise((resolve, _reject) => { require(webAuthnPath)[method](challenge, function (code, message) { return resolve({ code, message }) }) })
}
features.declareSupported("webauthn"),
DiscordIPC_1.DiscordIPC.main.handle(DiscordIPC_1.IPCEvents.WEBAUTHN_REGISTER_MAC, (_event, challenge) => callNative("webAuthnRegister", challenge)), ...
```
```js
// core/bundle.js:1 (#9984 @93179..93500)
DiscordIPC_1.DiscordIPC.main.handle(DiscordIPC_1.IPCEvents.PROCESS_UTILS_GET_SYSTEM_METRICS, (_, useNativeCpuTimes = !1) => {
    let { cpuTotalTick, cpuTotalIdle } = (function (useNativeCpuTimes) {
        if (useNativeCpuTimes && "win32" === process.platform) try {
            let discordUtils = __webpack_require__(4928), times = discordUtils.getSystemCpuTimes?.();
            if (null != times) return times } catch (error) { console.error("System CPU times query failed:", error) }
        /* fallback: os.cpus() */ })(!0 === useNativeCpuTimes);
    return Promise.resolve({ cpuTotalTick, cpuTotalIdle, memoryTotal: os_1.default.totalmem(), memoryFree: os_1.default.freemem() }) })
```

То есть на Linux нативный путь `discord_utils.getSystemCpuTimes` недостижим (жёсткий `win32`-гейт) —
всегда fallback на `os.cpus()`.

---

## 4. Нативные биндинги main-процесса

### 4.1 `requireNative` — как main грузит `.node` из moduleDataPath

```js
// app/bundle.js:12  (#30885, тело @84200..84330 — фабрика без стандартной сигнатуры `(module1){...}`)
30885(module1) {
  "use strict";
  let nativeModule = new (require("module"))(__filename);
  nativeModule.filename = __filename, nativeModule.paths = [], module1.exports = nativeModule.require.bind(nativeModule)
}
```

`requireNative = __webpack_require__(30885)` (использование: `app #23151 @25224`), т.е. это
`Module.prototype.require`, привязанный к **пустому** списку путей поиска. Резолв работает только
благодаря патчу `Module.globalPaths` (§3.3): `discord_desktop_core`,
`discord_overlay2/standalone_host.js` и прочие «виртуальные» модули лежат в
`<host>/modules/<name>-<version>` и попадают в `globalPaths` при `commitModules`.

### 4.2 Инвентарь нативных вызовов из main (что и где вызывается)

| Модуль | Как грузится | Где вызывается (main) | Что вызывается |
|---|---|---|---|
| `updater.node` | `require(<exe>/../updater.node \| <exe>/../../Resources/updater.node)` — `app #47421` @156352 | `Updater`-класс `#47421` (весь файл), `core #216` | `command(json)`, `command_blocking(json)` @157701, `known_folder(name)` @169575, `create_shortcut(options)` @169700, `destroy()`, `installModule`, `commitModules` |
| `discord_utils` | `require("discord_utils")` → `core #4928` (модуль-обёртка; `core/bundle.js:6 @395797`) | `core #4531` (Power/Idle), `core #9984` @88916/@93410 | `isWaylandIdleAvailable()`, `getWaylandSystemIdleTimeMs()`, `getDiscordClipsCumulativeCpuUsage()`, `getSystemCpuTimes()` (win32) |
| `discord_notifications` | `require(<modulePath>/discord_notifications)` — `core #7592` @80230; на win32 — `require("discord_notifications")` (модуль `#5385`, `core/bundle.js:6 @395720`) | `core #7592` | `setDataPath`, `setCallbacks`, `getAuthorization`, `getSettings`, `sendNotification`, `removeNotifications`, `removeAllNotifications` |
| `discord_webauthn` | `require(<modulePath>/discord_webauthn)` — `core #5544` @105305 (macOS ≥ 21) | `core #5544` | `webAuthnRegister`, `webAuthnAuthenticate`, `signalAllAcceptedCredentials`, `signalCurrentUserDetails`, `signalUnknownCredential` |
| `discord_intents` | `require(<modulePath>/discord_intents)` — `core #4355` @70300 (macOS) | `core #4355` | `setDataPath`, `indexDomains`, `clearIndex`, `deleteDomains`, `deleteItems`, `setActivity`, `resignActivity` |
| `discord_clips` | **исполняемый файл**, не `.node`: `getModulePath("discord_clips")` → `spawn(exePath, [...])` — `core #7635` @39418 | `core #7635` | `--render-recipe <recipeJson> <outputPath> <metadata>` (рендер монтажа) |
| `discord_desktop_core` | `requireNative("discord_desktop_core")` — `app #23151` @25224 | `app #23151` | `startup(bootstrapModules)`, `handleOpenUrl(url)`, `setMainWindowVisible(...)`, `WEBAPP_ENDPOINT` |
| `discord_overlay2` | `requireNative("discord_overlay2/standalone_host.js")` — `app #66321` @51981 | только `--overlay-host` | весь overlay-хост-процесс |
| `discord_sysimg` | preload-only (`core/mainScreenPreload.js`, модуль `7987`): `requireModule("discord_sysimg")`, `ensureModule(...)` | renderer | `probe`, `convertBytes` (HEIC/JXR → JPEG) |

Обёртки-«инъекции» нативных модулей в core: `core #4928` → `module.exports = require("discord_utils")`,
`core #5385` → `require("discord_notifications")` (видны как отдельные мини-модули в `core-entry` dump).
Полный инвентарь `.node` (включая renderer/preload-only: `discord_voice`, `discord_krisp`, `discord_zstd`,
`discord_media`, `discord_erlpack`, `discord_cloudsync`, `discord_premap`, `discord_notificationstate`,
`discord_cld`, `discord_game_utils`) — в `native-re.md`.

### 4.3 Чего в main нет (проверено grep’ом по обоим бандлам)

| Символ | `app/bundle.js` | `core/bundle.js` | Вывод |
|---|---|---|---|
| `discord_voice` / `discord_krisp` / `discord_zstd` / `discord_media` / `discord_cloudsync` / `discord_premap` / `discord_erlpack` | 0 | 0 | в main не загружаются, только renderer/preload (PREMAP — см. §10.2) |
| `cld`, `game_utils`, `notificationstate` | 0 | 0 | только renderer |
| `globalShortcut` | 0 | 0 | глобальных хоткеев нет (управление — через трей и меню) |
| `setProxy(` | 0 | 0 | прокси-свитча в main нет |
| `setUserAgent` | 0 | 0 | user-agent main не переопределяет |
| `certificate-error` | 2 | 0 | в app это **только** имя форматтера логгера (`app #96306 @2219777`), обработчика события нет |
| `Experiments`, `getFlag(` | 0 | 0 | локальных экспериментов/флагов в main нет (см. §6.3) |
| `discord-ipc`, `\\.\pipe`, `setAsDefaultProtocolClient`, `registerSchemeAsPrivileged`, `x-scheme-handler` | 0 | 0 | RPC-сокет и регистрация схем в asar не живут (делается установщиком/рантаймом) |

---

## 5. Автообновление

### 5.1 Выбор апдейтера (новый vs legacy)

```js
// app/bundle.js:1 (#33745  @6854..@7060)
let { getUpdater, tryInitUpdater, INCONSISTENT_INSTALLER_STATE_ERROR } = __webpack_require__(47421), settings = getSettings();
if (!buildInfo.disableUpdater && tryInitUpdater(buildInfo, bootstrapConstants.NEW_UPDATE_ENDPOINT)) {
    let updater = getUpdater(), usePinnedUpdateManifest = settings?.get("USE_PINNED_UPDATE_MANIFEST") ?? !1, ...
```

* `NEW_UPDATE_ENDPOINT` = setting либо **`https://updates.discord.com/`** (`app #19192 @3231`);
* `UPDATE_ENDPOINT` = setting либо `API_ENDPOINT` = **`https://discord.com/api`** (там же);
* `buildInfo.disableUpdater` — `app #55405 @26169`: `disableUpdater || ("development" === releaseChannel && (standaloneModules || localModulesRoot))`
  (`app/splashScreenPreload.js:1`, модуль `55405` — тот же build_info-модуль);
* `tryInitUpdater(buildInfo, repositoryUrl)` — `app #47421 @153814`: создаёт `new Updater(...)` (@154415)
  и грузит `<exe>/../updater.node` (darwin: `<exe>/../../Resources/updater.node`) (@156352);
* legacy-ветка: `moduleUpdater.init(UPDATE_ENDPOINT, …)` (`app #47924`), см. §5.4.

### 5.2 Новый апдейтер: сплэш-конечный автомат и восстановление

`app #53006` (`updateUntilCurrent`, @98275):

```js
// app/bundle.js:12 (#53006  @98275..@99200)
async function updateUntilCurrent() {
    let retryOptions = { skip_host_delta: !1, skip_module_delta: {}, skip_all_module_delta: !1, allow_optional_updates: allowOptionalUpdates };
    for (;;) { updateSplashState(CHECKING_FOR_UPDATES);
        try { let installedAnything = !1, downloads = new TaskProgress, installs = new TaskProgress;
            if (await newUpdater.updateToLatestWithOptions(retryOptions, progress => {
                let task = progress.task, downloadTask = task.HostDownload || task.ModuleDownload, installTask = task.HostInstall || task.ModuleInstall;
                installedAnything = !0, splashInstalledUpdates = !0, ...
                null != progress.state.Failed && (null != task.HostInstall ? retryOptions.skip_host_delta = !0
                    : null != task.ModuleInstall && (retryOptions.skip_module_delta[installTask.version.module.name] = !0)),
                downloads.updateSplashState(DOWNLOADING_UPDATES) || installs.updateSplashState(INSTALLING_UPDATES) }), !installedAnything) {
                performance.mark("splash-update-complete");
                await newUpdater.startCurrentVersion({}), newUpdater.setRunningInBackground(), newUpdater.collectGarbage(),
                launchMainWindow(), updateBackoff.succeed(), updateSplashState(LAUNCHING); return }
        } catch (e) { console.error("splashScreen: Update failed", e), await new Promise(resolve => { let delayMs = updateBackoff.fail(() => resolve(!1)); splashState.seconds = Math.round(delayMs / 1e3), updateSplashState(UPDATE_FAILURE) }) }
    }
}
```

Backoff (@97270): `updateBackoffMaxMs = settings?.get("DESKTOP_TTI_UPDATE_BACKOFF_MAX_MS", 3e4) ?? 3e4`,
`updateBackoff = 1 === settings?.get("DESKTOP_TTI_UPDATE_BACKOFF_ALGO", 0) ? new ScheduledBackoff([2e3,3e3,3e3,4e3,5e3,7e3,1e4,12e3,15e3], !0) : new Backoff(1e3, updateBackoffMaxMs)`;
при ретрае шлётся `splashState.seconds` (таймер в `splashScreenPreload.js`). `TaskProgress` — @97536.

### 5.3 Linux-специфика обновления

* `app #2278` (тело @47672) — «host updater» для Linux: `autoUpdater = new AutoUpdaterLinux` (@51248),
  проверка версии → **`this.emit("update-manually", latestVerStr)`** (@50475) (не скачивает и не ставит);
  `app #47924` подписан на `update-manually` (@153) и переводит splash в состояние `update-manually`;
* UI `app/splashScreenPreload.js`: `LINUX_DOWNLOAD_URL_BASE = "https://discord.com/api/download/<channel>?platform=linux&format="`,
  кнопки `Ubuntu (deb)` / `Linux (tar.gz)` / `I'll figure it out`, затем `ipcRenderer.send("DISCORD_SPLASH_SCREEN_QUIT")`;
* legacy feed: `feedURL = <endpoint>/updates/<channel>?platform=linux&version=<cur>` (`app #47924 @62552`).

### 5.4 Legacy-апдейтер `moduleUpdater` (`app #47924`, тело @55561..80430)

* `remoteBaseURL = <endpoint>/modules/<channel>` (@61321) — каталог версий модулей;
* скачивание/распаковка модулей + `installedModules`/`localModuleVersionsFilePath`, `installPendingUpdates` (@64229),
  `checkForUpdates` (@?), `events` (`INSTALLED_MODULE`, `DOWNLOADED_MODULE`, `UPDATE_MANUALLY` @?), `hostUpdater.quitAndInstall()`;
* `initPathsOnly(buildInfo)` — используется в `--overlay-host` (только пути, без сети);
* загрузка `moduleUpdater` в core: `core #2979`/`#1623` (см. `core.startup` §1.4) — инъекция в `#4943`
  (`app.injectModuleUpdater`/`injectUpdater`).

### 5.5 Прочие точки обновления

* `DISCORD_QUIT_AND_INSTALL` → `updater.quitAndInstallUpdates()` (`core #5203` @128359);
* pinned-манифест: `USE_PINNED_UPDATE_MANIFEST` → `pinned_update.json` (реализация в `app #47924`);
* `--overlay-host`: `getUpdater()?.startCurrentVersionSync({ allowObsoleteHost: !0 })` (`app #66321` @52111);
* «Startup prevented.»/`pendingAppQuit` — squirrel-события Windows, не про Linux.

---

## 6. Фичефлаги и allow-list настроек

### 6.1 Локальный реестр (никаких Experiments)

```js
// core/bundle.js:3 (#2603  @207686..208056) — FeatureFlags
exports.default = class { flags;
    constructor() { this.flags = new Set }
    getSupported() { return Array.from(this.flags) }
    supports(feature) { return this.flags.has(feature) }
    declareSupported(feature) { this.supports(feature) ? console.error("Feature redeclared; is this a duplicate flag? ", feature) : this.flags.add(feature) } }
```
```js
// core/bundle.js:1 (#1353  @45789) — выдача в renderer
DiscordIPC_1.DiscordIPC.main.on(DiscordIPC_1.IPCEvents.FEATURES_GET_BROWSER_FEATURES, event => { event.returnValue = injectedFeatures?.getSupported() ?? [] });
```

`appFeatures.init()` вызывает `core #5976` (`features = new FeatureFlags()`), сам набор — только рантайм-`declareSupported`.

### 6.2 Объявленные флаги (по месту объявления)

| Флаг | Где объявляется | Условие |
|---|---|---|
| `popout_windows` | `core #9698 @179319` | всегда при `init()` окна |
| `intents` | `core #4355 @71507` | macOS + успешная загрузка `discord_intents` |
| `notifications` + `notifications_provisional` | `core #7592 @80640` (и следом `@80660`) | macOS ≥ kernel 21 (`MIN_MACOS_KERNEL_MAJOR`, `core #1279 @75640`) **или** win32 с `Notification.isSupported()` (Linux — нет) |
| `clips_storage_size` | `core #7635 @41095` | при инициализации clips-модуля |
| `webauthn` | `core #5544 @105830` | macOS ≥ 21 |
| `staff_dev_tools` | `core #4520 @187640` | модуль стаф-девтулзов (`tryLoadStaffDevToolsModule`, см. `#5203` @~150000) |

### 6.3 Renderer-allow-list настроек

```js
// core/bundle.js:1 (#3716  @114237..114420)
exports.RENDERER_SET_ALLOWLIST = ["audioSubsystem","offloadAdmControls","debugLogging","asyncVideoInputDeviceInit",
    "ALWAYS_ALLOW_UPDATES","ALLOW_OPTIONAL_UPDATES","openH264Enabled","DESKTOP_TTI_DNSTCP_WARMUP",
    "DESKTOP_TTI_REMOVE_V8_CACHE_CLEAR","DESKTOP_TTI_REMOVE_SHADER_CACHE_CLEAR","DESKTOP_TTI_EARLY_UPDATE_CHECK",
    "DESKTOP_TTI_HTTP_CDT","DESKTOP_TTI_SPLASH_USE_WEBP","DESKTOP_TTI_LEGACY_OVERLAY_WORKER",
    "DESKTOP_TTI_UPDATE_BACKOFF_MAX_MS","DESKTOP_TTI_UPDATE_BACKOFF_ALGO","DESKTOP_NATIVE_PREMAP"];
exports.RENDERER_SET_CHROMIUM_SWITCHES_ALLOWLIST = [];
```

`SETTINGS_SET` из renderer’а фильтруется этим списком (см. `backend-ipc.md` §SETTINGS), а сам main
читает «рисковые» ключи напрямую (`chromiumSwitches` — свой `validSwitches`, @@15700+ `#23151`).

---

## 7. Сеть, прокси, сертификаты

### 7.1 Desktop connectivity tests (`app #51937`, тело @33409..38392)

```js
// app/bundle.js:10 (#51937  @~35000..@36500)
for (let [testName, test] of Object.entries(DESKTOP_TESTS)) urlFilterPatterns.push(...test.urlFilterPatterns), urlToTestName.push(...);
session.webRequest.onErrorOccurred({...}, details => { ... if (!trackedNetErrQuicProtocol && details.error.includes("net::ERR_QUIC_PROTOCOL_ERROR")) { ... } });
session.webRequest.onCompleted({...}, details => { ... })
```

Набор тестов `DESKTOP_TESTS` (`@38269`) — «webapp», «updaterdep» и др.; при ошибках — Sentry-событие
(расшифровка в `backend-ipc.md` не пересекается: это не IPC, а webRequest).

### 7.2 DNS-телеметрия (`app #65445`, @29537..33350)

```js
// app/bundle.js:10 (#65445  @31403 и @32271)
this.baseDomain = baseDomain ?? "dc-telemetry.net", this.clientLocale = clientLocale;
...
await net.fetch(`https://${dnsQuery}`)   // DNS-имя с полезной нагрузкой; ответ не читается
```
Метки кодирования: `idt`(sessionNonce), `loc`(locale), `chk`(checkId), `res`(ok/err), `err`(code-message),
`dur`(duration, base36); версия `DNS_ENCODING_VERSION="v1"`; `reportedChecks` — по одному репорту на check.

### 7.3 Прогрев DNS/TCP (`app #72100`, @38451..39001)

```js
// app/bundle.js:10 (#72100  @38557 и @38724)
for (let domain of DESKTOP_TTI_WARMUP_DOMAINS) defaultSession.resolveHost(domain).catch(() => {});
defaultSession.preconnect({ url: "https://discord.com", numSockets: 1 }),
defaultSession.preconnect({ url: "https://cdn.discordapp.com", numSockets: 1 })
```
`DESKTOP_TTI_WARMUP_DOMAINS = ["discord.com","cdn.discordapp.com","gateway.discord.gg","updates.discord.com"]`;
включается настройкой `DESKTOP_TTI_DNSTCP_WARMUP` (см. §1.2 п.11).

### 7.4 CA-бандл и webRequest-диспетчер

```js
// core/bundle.js:1 (#8730  @183290..183961)
let requestCA;
exports.init = function () { let rootCertificateAuthorities;
    try { rootCertificateAuthorities = fs_1.default.readFileSync(path_1.default.join(__dirname, "data", "cacert.pem")) }
    catch (err) { console.error("Unable to load root certificate authorities."), console.error(err) }
    requestCA = rootCertificateAuthorities ? { ca: rootCertificateAuthorities } : {} };
exports.getRequestCA = function () { return requestCA };
```
Загружается из `core.startup()` (`__webpack_require__(8730).init()`, §1.4); файл — `core/data/cacert.pem`
(см. `asar-extract/core/data`). `getRequestCA()` в main-бандле больше нигде не вызывается (единственная
точка) — т.е. это **заготовка/совместимость** для модулей, которые захотят `ca: requestCA`
(сам `net`-стек Electron использует системные/встроенные CA).

Диспетчер before-request:

```js
// core/bundle.js:3 (#6228  @205625..206707) — «один listener на session», union url-паттернов
exports.registerBeforeRequest = function (session, id, handler, urlPatterns) { ... reinstallListener(session) }
exports.unregisterBeforeRequest = function (session, id) { ... }
function reinstallListener(session) { ... session.webRequest.onBeforeRequest({ urls: Array.from(unionedPatterns) }, (details, callback) => { ... }) }
```
Единственный боевой потребитель — `DESKTOP_TTI_HTTP_CDT` в `core #5203 @144029` (редирект
`/assets/web.<hash>.js` → `?cdt=1`).

### 7.5 Что отсутствует

`setProxy`, `setUserAgent`, `certificate-error`-handler, `session.setCertificateVerifyProc`,
`app.on("login")` — не найдены. Прокси/CORS-поведение — на уровне Chromium/Electron и системных настроек,
в asar не переопределяется. `net.fetch` встречается только в DNS-репортере (§7.2).

---

## 8. Crash reporting: Sentry + crashpad + blackbox

### 8.1 Метаданные и DSN (`app #62457`, тело @140922..145800)

```js
// app/bundle.js:15 (#62457  @143136..@144000)
exports.metadata.release_channel = buildInfo.releaseChannel;
if (__webpack_require__(45468).IS_LINUX) {
    let xdgCurrentDesktop = process.env.XDG_CURRENT_DESKTOP ?? "unknown", gdmSession = process.env.GDMSESSION ?? "unknown";
    exports.metadata.wm = `${xdgCurrentDesktop},${gdmSession}`;
    let runtimeEnvironment = "native";
    null != process.env.FLATPAK_ID ? runtimeEnvironment = "flatpak" : null != process.env.SNAP || null != process.env.SNAP_NAME ? runtimeEnvironment = "snap"
        : (null != process.env.APPIMAGE || null != process.env.APPDIR) && (runtimeEnvironment = "appimage"), exports.metadata.runtime_environment = runtimeEnvironment;
    let displayServer = "unknown"; null != process.env.XDG_SESSION_TYPE && (displayServer = process.env.XDG_SESSION_TYPE), exports.metadata.display_server = displayServer;
    try { let childProcess = __webpack_require__(31421); exports.metadata.distro = childProcess.execFileSync("lsb_release", ["-ds"], { timeout: 100, maxBuffer: 512, encoding: "utf-8" }) ... }
```
Sentry-инициализация (там же): `sentry.init({dsn, transport, environment, release, dist, sampleRate, skipOpenTelemetrySetup: !0, integrations (без MainProcessSession/BrowserSession), maxValueLength: 250, beforeSend(event){ event.extra = exports.metadata; …code-integrity из minidump… __webpack_require__(81598).addSentryReport(event) }, ignoreErrors: [...], denyUrls: [/betterdiscord:\/\//] })`;
`sampleRate`: Linux/macOS **1**, Windows — по каналу (`CHANNEL_SENTRY_SAMPLE = {stable:.01, ptb:1, canary:1, development:1}`) или `.01`.
DSN-ключи main-процесса (`buildSentryDSN(key) = "https://" + key + "@o64374.ingest.sentry.io/146342"`, `#62457 @145418`):
Linux `d2558f321dfc7ab68366d8258fd256c7` (@145577), macOS `098008bd74508d75b72f31244ddb1d04`,
DEFAULT `384ce4413de74fe0be270abe03b2b35a`, stable `7a60c374cb0e99ac8a57388db6933711`,
staff (`dsnFromUser` при теге `isStaff=true`) `de156ff7a3f544cca369e77e3f1f5743`.
У renderer/preload **отдельный** DSN: ключ `a6d1a1ca54ed4a729c6949f0237969e3` встречается только в `core/mainScreenPreload.js`
(«из артефакта»; в main-бандлах его нет).

### 8.2 Electron crashReporter (@sentry/electron, `app #30269`, `app/bundle.js:505`)

```js
// app/bundle.js:505 (#30269  @1582507..@1586000)
electron.crashReporter.start({ ignoreSystemCrashHandler: !0, uploadToServer: !0, compress: !0,
    submitURL: minidumpUrlFromDsn(dsn), globalExtra: globalExtra })     // @1584418 / minidumpUrlFromDsn @1582877
```
`minidumpUrlFromDsn(dsn)` (@1582868) строит `<protocol>://<host>/api/<projectId>/minidump/?sentry_key=<publicKey>` —
т.е. для Linux submitURL = `https://o64374.ingest.sentry.io/api/146342/minidump/?sentry_key=d2558f321dfc7ab68366d8258fd256c7`.
Плюс `getLastCrashReport()` в том же модуле (@1585684) и синхронный разбор Crashpad-дампа через `crashDumps` (§8.3).

### 8.3 Blackbox: копирование minidump из crashpad (`app #81598`, тело @127756..136187; core-копия `#1598`)

```js
// app/bundle.js:12 (#81598  @128731..@130600)
let reportsDir = path_1.default.join(electron_1.default.app.getPath("crashDumps"), processUtils_1.IS_WIN ? "reports" : "completed");
let dmpFiles = dirContents.filter(f => f.endsWith(".dmp")).map(...).sort((a,b) => b.mtime - a.mtime);
if (0 === dmpFiles.length || Date.now() - dmpFiles[0].mtime > 6e4) return;
let crashlogsDir = path_1.default.join(storedModulepath, "crashlogs"); ...
if (existingDmps.length > 10) for (let i = 10; i < existingDmps.length; i++) fs_1.default.unlinkSync(existingDmps[i].fullPath)
```
`pending_minidump.txt` (`@130083`) — сигнал «в crashlogs лежит свежий дамп, забери его»;
`captureMinidumpFromCrashpadSync()` вызывается из `#23151 @16369` **до** инициализации Sentry, чтобы не
потерять дамп от предыдущего падения.

### 8.4 Main-side IPC крэш-метаданных

```js
// core/bundle.js:1 (#7138  @43633..44600)
DiscordIPC_1.DiscordIPC.main.handle(DiscordIPC_1.IPCEvents.CRASH_REPORTER_UPDATE_METADATA, (_, additionalMetadata) => {
    let metadata = crashReporterSetup_1.crashReporterSetup.metadata;
    assert(null != metadata, "Metadata imported improperly.");
    let finalMetadata = lodash_1.default.defaultsDeep(metadata, additionalMetadata ?? {});
    (0, crashReporterUtils_1.reconcileCrashReporterMetadata)(electron_1.default.crashReporter, finalMetadata);
    let sentry = crashReporterSetup_1.crashReporterSetup.getGlobalSentry();
    if (null != sentry) { let user = additionalMetadata.sentry?.user; null != user && sentry.setUser(user);
        let nativeBuildNumber = additionalMetadata.nativeBuildNumber; ...; let staff = additionalMetadata.staff; null != staff && sentry.setTag("isStaff", staff.toString()) }
    return Promise.resolve({ metadata: finalMetadata }) }),
electron_1.default.ipcMain.handle(DiscordIPC_1.IPCEvents.UNHANDLED_JS_EXCEPTION, _ => { setTimeout(() => { throw Error("UNHANDLED_EXCEPTION " + process.type) }, 50) });
```
```js
// core/bundle.js:1 (#9984  @90131..@90760)
DiscordIPC_1.DiscordIPC.main.handle(DiscordIPC_1.IPCEvents.PROCESS_UTILS_GET_LAST_CRASH, async _ => {
    let lastCrashReport = electron_1.default.crashReporter.getLastCrashReport(), pendingMinidumpPath = null, moduleDataPath1 = paths_1.paths.getModuleDataPath();
    if (null != moduleDataPath1) { let crashlogsPath = path_1.default.join(moduleDataPath1, "crashlogs"), sentinelPath = path_1.default.join(crashlogsPath, "pending_minidump.txt");
        try { let filename = (await fs_1.default.promises.readFile(sentinelPath, "utf-8")).trim();
            filename.length > 0 && (pendingMinidumpPath = path_1.default.join(crashlogsPath, filename), await fs_1.default.promises.unlink(sentinelPath)) } catch {} }
    return { date: lastCrashReport?.date ?? null, id: ..., rendererCrashReason, rendererCrashExitCode, storedInformation, lastMemoryInformation, highestMemoryInformation, pendingMinidumpPath } })
```
`rendererCrashReason`/`rendererCrashExitCode` заполняются обработчиком `render-process-gone` (§2.2),
а `storedInformation` — каналом `PROCESS_UTILS_SET_CRASH_INFORMATION` (`#9984 @92298`).
`__webpack_require__(45468).IS_WIN/IS_OSX/IS_LINUX` (`app #45468`, тело @150904..151349) — утилита
платформы, которой пользуются обе ветки.

---

## 9. Трей, меню, хоткеи, протоколы

### 9.1 Трей (`core #1275`, тело @193585..200700)

```js
// core/bundle.js:3 (#1275  @195647..@196900)
TRAY_ICON_NAMES = { DEFAULT: "tray", UNREAD: "tray-unread", CONNECTED: "tray-connected", SPEAKING: "tray-speaking",
                    MUTED: "tray-muted", DEAFENED: "tray-deafened" },
MENU_REBUILD_KEYS = ["connected","muteStates","deafStates"],
TrayGuidByChannel = { stable: "699e423d-73da-42ba-b94e-881ff8672467", ptb: "8bdf397e-37fc-440a-ac55-ec80cc585152",
                      canary: "d9c5e1da-d4ab-424f-97cf-00c2ce9bec67", development: "6603ea8b-b21d-46e9-8ea5-d87b9105613f" }
...
(atomTray = IS_WINDOWS || IS_MACOS ? new electron_1.Tray(icon, TrayGuidByChannel[buildInfo.releaseChannel]) : new electron_1.Tray(icon))
    .setToolTip(currentStrings.appName), setContextMenu(), (IS_WINDOWS || IS_LINUX) && atomTray.on("click", options.onTrayClicked)   // @200422
```
Иконки грузятся из `app/images/systemtray/<platform>` (@194230), macOS-суффикс `Template`;
меню трея: `Open Discord` (клик = `onTrayClicked`), `Mute`/`Deafen` (checked по стейтам),
`Voice / Video Settings`, `Check for Updates...` (@196610), `Acknowledgements` (@196649), `Quit Discord` (@196685) — словарь `currentStrings` (@196377).
`displayHowToCloseHint()` использует `trayBalloonShown` + `displayBalloon` (@193900) — реально
работает на Windows (Linux/Tray.dropBalloon здесь не вызывается).
Стейт-обновления приходят каналами `SYSTEM_TRAY_SET_ICON/SET_APPLICATIONS/SET_STATES/SET_STRINGS`
(см. `backend-ipc.md`); в `#5203` — `SYSTEM_TRAY_TOGGLE_MUTE` (@140085) и
`SYSTEM_TRAY_OPEN_VOICE_SETTINGS` (push в renderer, там же @140012).

### 9.2 Меню приложения

| Меню | Модуль | Содержимое |
|---|---|---|
| «начальное» (во время обновления) | linux `app #75150` (`@106163..106334`) | `&File → &Exit` (`Control+Q`) |
| «рабочее» (после `core.startup`) | `core #7814` → `require.context(1783)("./"+process.platform)` | linux — `#4380` (File→Options `Control+,`, Exit `Control+Q`; Edit; View→Reload, Toggle Full Screen; Help→Check for Updates / Discord Help), win32 — `#4933` (`Ctrl+,`, `Alt+F4`, `F11`), darwin — `#1751` (appMenu, `Command+,`, Acknowledgements, Services) |

```js
// core/bundle.js:1 (#7814  @8868..9093)
let electron_1 = __webpack_require__(1288), createMenu = __webpack_require__(1783)("./" + process.platform);
module1.exports = function (enableDevtools) { return electron_1.Menu.buildFromTemplate(createMenu(enableDevtools)) }
```
Раздел `Developer → Toggle Developer Tools` добавляется только в не-stable каналах или при
`DANGEROUS_ENABLE_DEVTOOLS_ONLY_ENABLE_IF_YOU_KNOW_WHAT_YOURE_DOING` (`core #5203 @137195`, §1.4).

### 9.3 Хоткеи

`globalShortcut` в бандлах **отсутствует** (0 совпадений) — глобальных горячих клавиш нет.
Все горячие клавиши — это `accelerator` в меню (`Control+R`, `Control+Shift+I`, `Control+Q`, `Control+,`, `F11`, `Alt+F4`)
и трей-клики. `before-input-event`-перехватов в main нет.

### 9.4 Протоколы и deep-links

```js
// app/bundle.js:10 (#23151  @21998..@22400)
function extractUrlFromArgs(args) { let urlArgIndex = args.indexOf("--url"); if (urlArgIndex >= 0) { let passThroughArgsIndex = args.indexOf("--");
        if (passThroughArgsIndex < 0 || passThroughArgsIndex < urlArgIndex) return null; let url = args[passThroughArgsIndex + 1]; return null == url ? null : url }
    let protocolPrefix = `${Constants.APP_PROTOCOL.toLowerCase()}://`;
    for (let i = 1; i < args.length; i++) if (args[i].toLowerCase().startsWith(protocolPrefix)) return args[i]; return null }
```
* `APP_PROTOCOL = "Discord"` (`app #19192 @~2400`) → искомый префикс `discord://`;
* `second-instance` / `open-url` / `continue-activity` / `fallbackDeepLink` → `openOrQueueUrl` → `core.handleOpenUrl`
  (`core #5203 @133092`, `handleOpenUrl = function` — парсит `discord:`-URL и мапит на навигацию окна);
* `disclip://` — регистрируется **до** `app.whenReady()`: `beforeReadyProtocolRegistration()` (`app #23151 @24154`)
  → `app #46312`; значение протокола — `DISCORD_CLIP_PROTOCOL = "disclip"` (`app #29157 @140832`);
* `setAsDefaultProtocolClient` в asar нет — регистрация схемы делает установщик (desktop-файл).

---

## 10. Голос, оверлей, PREMAP и «чего нет в main»

### 10.1 Голос

* main **не загружает** `discord_voice` (0 совпадений в обоих бандлах); вся голосовая функциональность
  живёт в renderer/preload (`native-re.md`);
* из main в голос идут только команды трея: `SYSTEM_TRAY_TOGGLE_MUTE` / `SYSTEM_TRAY_OPEN_VOICE_SETTINGS` /
  `SYSTEM_TRAY_TOGGLE_DEAFEN` (`core #5203 @140000`), а также deeplink `USER_SETTINGS_OPEN` из
  callback’а уведомлений (§3.2);
* режимы неактивности: `core #4531` (`POWER_MONITOR_*`), на Linux — `discord_utils.isWaylandIdleAvailable()/
  getWaylandSystemIdleTimeMs()` с fallback (`core #4531 @82812` ff.).

### 10.2 PREMAP (renderer-side, но включается из main)

```js
// core/mainScreenPreload.js:543 (модуль 7328) — startNativePremap
if ("win32" === process.platform && process.argv.includes("--enable-native-premap")) try { ...
    let premapDir = nativeModules.getModulePath("discord_premap"); premap.premapModules(targets),
    setBeforeRequireModuleHook(name => { let dir = nativeModules.getModulePath(name); null != dir && premap.dequeuePremap(path.join(dir, `${name}.node`)) }) }
```
`PREMAP_MODULES = ["discord_voice","discord_zstd","discord_cloudsync","discord_media"]`; флаг
`--enable-native-premap` добавляется в `additionalArguments` главного окна только на win32 при
`DESKTOP_NATIVE_PREMAP` (`core #5203 @141637`). Т.е. на Linux ветка PREMAP — мёртвый код.

### 10.3 Clips в main (сводка)

`core #7635` (тело @25173..41592; `setupClipsProtocol()` @26390) ставит `protocol.handle("disclip", ...)` (@26431)
с `Range`/`206`/`416`; `LOAD_CLIP` (@40257), `LOAD_CLIPS_DIRECTORY`, `DELETE_CLIP`, `MOVE_CLIP`,
`UPDATE_CLIP_METADATA`, `RENDER_MONTAGE_RECIPE` (spawn §3.5); лимит длины пути/`MAX_LENGTH` (@28725) для чтения файлов;
`discord_clips` — **EXE**, не `.node` (см. §4.2).

---

## 11. Сводные цепочки (для JSON)

| # | Канал | main-функция | Нативный вызов | Ответ |
|---|---|---|---|---|
| A | `DISCORD_NOTIFICATIONS_SEND_NOTIFICATION` | `core #7592` handler | `require(<modulePath>/discord_notifications).sendNotification(options)` | `normalizeSendResult(...)`; async-событие `DISCORD_NOTIFICATIONS_RECEIVED_RESPONSE` всем окнам |
| B | `DISCORD_NATIVE_MODULES_INSTALL` | `core #216` `ipcMain.handle` | `updater.installModule(name)` + `updater.commitModules({})` (updater.node) → `addGlobalPath(<...>/modules/<name>-<ver>)` | resolve + `DISCORD_NATIVE_MODULES_GET_PATHS` → `{mainAppDirname, browserModulePaths}` |
| C | `DISCORD_WINDOW_GET_NATIVE_HANDLE` | `core #3386` | `win.getNativeWindowHandle().readInt32LE().toString(10)` | строка-идентификатор окна |
| D | `DISCORD_RENDER_MONTAGE_RECIPE` (+ `disclip://`) | `core #7635` `renderMontageRecipe` / `protocol.handle` | `spawn(<modulePath>/discord_clips, ["--render-recipe", recipe, out, meta])` | code 0 + `verifyNewDiscordMP4Path(out)`; для `disclip://` — HTTP 206/416 + `verifyDiscordMP4` |
| E | `DISCORD_WEBAUTHN_REGISTER_MAC` | `core #5544` `callNative` | `require(<modulePath>/discord_webauthn).webAuthnRegister(challenge, cb)` | `{code, message}` (macOS ≥ 21) |

---

## 12. Unknowns / ограничения

1. **`Experiments`/`getFlag` в main отсутствуют** (0 совпадений): фичефлаги клиента — серверные;
   локально есть только `declareSupported`-Set (§6.1), который отдаётся в renderer синхронным каналом.
2. **Нет `globalShortcut`, `setProxy`, `setUserAgent`, `certificate-error`-handler,
   `setAsDefaultProtocolClient`, `x-scheme-handler`** в бандлах — соответствующие механизмы либо
   вне asar (установщик, Chromium, ОС), либо не используются.
3. **RPC Discord (`discord-ipc-0`) в main не найден** — ни сокета, ни `commands.json`/`net.connect`
   по этому пути; вероятно, RPC обслуживает нативный `discord_rpc` в renderer/preload или внешний процесс
   (в `native-re.md` — см. соответствующие модули; здесь подтверждение только об отсутствии в main-JS).
4. **Linux-ветки нативных уведомлений и интентов мертвы** (`isOSX`/`isWindows`/`win32`-гейты) — на Linux
   `discord_notifications`/`discord_intents`/`discord_webauthn` не грузятся; их статус в renderer —
   `unsupported`; фичефлаги не объявляются.
5. **`getNativeWindowHandle().readInt32LE()`** — 4 байта; для не-X11 (Wayland/64-бит) значение может быть
   некорректным; в коде нет проверки платформы/длины буфера.
6. **`requireNative` фактически не standalone**: резолв целиком зависит от патча `Module.globalPaths`
   (`#27453`/`#7453`) и от того, что `commitModules` был вызван до первого `requireNative` — в asar
   порядок соблюдён, но при любом изменении модуля `#47421` это хрупкая точка.
7. `--overlay-host` — отдельный процесс с собственным апдейтером; как он взаимодействует с основным
   приложением (общий ли `moduleDataPath`, кто запускает `standalone_host.js`), в JS-main не описано —
   передано нативному `discord_overlay2` (за границей JS).
8. Часть app-модулей использует **короткие имена** каналов (§3.1), поэтому статический поиск по
   `DISCORD_*` в `app/bundle.js` даёт ложные «отсутствия» (напр. `DISCORD_SPLASH_SCREEN_READY`).
   Это же объясняет расхождения с `backend-ipc.md`, где карта построена по preload-литералам.
9. Два места с `escaped` `pending_minidump.txt` (`app #81598`, `core #9984`) — синхронный и
   асинхронный пути; их гонка (write vs read/unlink) не разбиралась.
10. Все «секреты» в отчёте (DSN, ключи, GUID’ы трея) — статика бинарного артефакта Discord 1.0.162
    (`из артефакта`), могут отличаться в других версиях/каналах.
