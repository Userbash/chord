# Discord 1.0.162 (Linux x64) — Desktop Client Main Process: Decompiled JS Analysis

Report on the desktop main process based on decompiled ASAR archives (`asar-extract/`).
Complements (without duplicating) companion artifacts:
* `backend-ipc.md` / `backend-ipc.json` — 168 IPC channels (`DISCORD_*`), preload whitelists, channel groups;
* `native-re.md` / `native-re.json` — inventory of `.node` native modules and exports.

Focus: **Main process architecture** (bootstrap lifecycle, window hierarchies, IPC dispatch patterns, native bindings, auto-updater state machine, feature flags, network/proxy/CA stack, crashpad reporting, tray/menu/protocol handlers, voice and clips integration).

## 0. Input Files and Citation Format

| File (relative to `asar-extract/`) | Size | Lines | Webpack Modules |
|---|---|---|---|
| `app/bundle.js` | 3,066,301 B | 519 | 825 |
| `core/bundle.js` | 402,296 B | 8 | 81 |
| `core/mainScreenPreload.js` | 1,105,621 B | 545 | renderer-preload |
| `app/splashScreenPreload.js` | 9,333 B | 30 | splash-preload |

Verification: `md5 app/bundle.js = 23fbdabaafb7acd0d57b2b2aab60fd49`, `md5 core/bundle.js = 08edd888cca8426f0cbc6c3bb2edbf17`.

**Citation Convention.** Bundles are minified into large lines (up to 849,148 bytes in `core/bundle.js:6`), so `file:line` is insufficient. Citations provide:

```
file:line  (#moduleId, body @byteStart..@byteEnd; cited snippet @byteN)
```

along with diff-neutral byte offsets. Code snippets are formatted for readability. Entrypoints: `app` -> `__webpack_require__.s = 66321`; `core` -> `__webpack_exports__.startup`.

---

## 1. Main Process Lifecycle (Bootstrap)

### 1.1 Dual Entrypoints: App Mode vs Overlay Host

`app/bundle.js` entry module `#66321` branches between standard application (`#23151`) and isolated overlay host (`--overlay-host` flag, with custom updater and `standalone_host.js`):

```js
// app/bundle.js:12 (#66321, body @51355..52280)
let mode = process.argv.includes("--overlay-host") ? "overlay-host" : "app";
if ("app" === mode) __webpack_require__(23151);
else if ("overlay-host" === mode) {
    __webpack_require__(43731).init();
    if (paths.updatePathsForNewUpdater(buildInfo), buildInfo.disableUpdater)
        __webpack_require__(47924).initPathsOnly(buildInfo);
    else { if (!tryInitUpdater(buildInfo, NEW_UPDATE_ENDPOINT)) throw Error("Failed to initialize modules in overlay host.");
        getUpdater()?.startCurrentVersionSync({ allowObsoleteHost: !0 }) }
    __webpack_require__(30885)("discord_overlay2/standalone_host.js"); // @52159
}
```

### 1.2 Early Bootstrap Phase `#23151` (Pre-`app.whenReady`)

```js
// app/bundle.js:10 (#23151, body @15212..25870)
"linux" === process.platform && void 0 === process.env.PULSE_LATENCY_MSEC &&
    (process.env.PULSE_LATENCY_MSEC = "30"); // @15343
...
let paths = __webpack_require__(37948); paths.init(buildInfo);
let blackbox = __webpack_require__(81598), moduleDataPath = paths.getModuleDataPath();
null != moduleDataPath && blackbox.initialize(moduleDataPath, buildInfo),
    blackbox.captureMinidumpFromCrashpadSync(); // @16369
let crashReporterSetup = __webpack_require__(62457);
crashReporterSetup.init(buildInfo, { sentry, getTransport: dsnFunc => browser.makeMultiplexedTransport(...) }); // @16488
```

Early bootstrap steps:

| Step | Action | Offset |
|---|---|---|
| 1 | Linux config: `PULSE_LATENCY_MSEC=30`, `app.setName(name + "-" + channel)`, `setDesktopName("discord" \| "discord-<channel>")` | @15343–@15600 |
| 2 | `errorHandler.init()` -> `paths.init(buildInfo)` -> `appSettings.init()` -> `paths.updatePathsForNewUpdater()` | @15600–@16000 |
| 3 | Blackbox crash logging init + `captureMinidumpFromCrashpadSync()` | @16369 |
| 4 | `crashReporterSetup.init(buildInfo, {...})` (Sentry + transport) | @16488 |
| 5 | `app.disableHardwareAcceleration()` if `enableHardwareAcceleration=false`; `--autoplay-policy=no-user-gesture-required` | @16000–@16300 |
| 6 | `disabled-features`: platform list + Linux `WaylandWpColorManagerV1` | @15800–@16000 |
| 7 | Linux + OpenH264: `--enable-libopenh264` + `--openh264-library-path` | @19732 |
| 8 | Windows: `setAppUserModelId`; Squirrel startup handling | @21000–@21500 |
| 9 | Single-instance lock: `app.requestSingleInstanceLock()` unless `--multi-instance` | @21961 |
| 10 | Argument parsing: `--exit-immediately`, `--url <u>` / `discord://` | @22445/@21998 |
| 11 | `app.on("ready")` -> connectivity tests and optional DNS/TCP pre-warming | @23148/@23284 |
| 12 | URL dispatch handling on launch | @23588–@24000 |
| 13 | Protocol registration (`disclip://`) -> `setGPUFlags().then(app.whenReady)` | @24154/@24188 |

`setGPUFlags()` handles hardware workarounds: probes `app.getGPUInfo("basic")` against vendor tables.

### 1.3 `whenReady` -> Splash/Updater -> `core.startup()`

```js
// app/bundle.js:10 (#23151 @24188..@25500)
setGPUFlags().then(app.whenReady).then(() => {
    paths.cleanOldVersions(buildInfo),
    startupMenu = __webpack_require__(1248), Menu.setApplicationMenu(startupMenu), // @24463
    __webpack_require__(33745).update(startMinimized, () => { // @24575
        let core = requireNative("discord_desktop_core"); // @25224
        coreModule = core, core.startup({ Constants, GPUSettings, analytics, appSettings, autoStart,
            buildInfo, crashReporterSetup, logger, moduleUpdater, paths, splashScreen, updater }); // @25283
        null != initialUrl && (core.handleOpenUrl(initialUrl), initialUrl = null);
    }, () => { coreModule.setMainWindowVisible(!startMinimized, startInactive) })
})
```

### 1.4 `core.startup()` Initialization Order

```js
// core/bundle.js:6 (entry module __webpack_exports__, starts ~@398000)
__webpack_exports__.startup = function (bootstrapModules) {
    performance.mark("coremodule-index-startup"), __webpack_require__(9895).init(bootstrapModules), ...
    let { moduleUpdater } = __webpack_require__(2979), { updater } = __webpack_require__(1623);
    Constants.init(bootstrapModules.Constants);
    let appFeatures = __webpack_require__(5976); appFeatures.init();
    bootstrapModules.GPUSettings.replace(GPUSettings), __webpack_require__(8730).init(), // cacert.pem
    let app = __webpack_require__(4943); app.injectBuildInfo(...), app.injectModuleUpdater(moduleUpdater), app.injectUpdater(updater);
    crashReporterSetup.isInitialized() || crashReporterSetup.init(buildInfo), __webpack_require__(7138),
    __webpack_require__(1353).injectFeaturesBackend(appFeatures.getFeatures()), ...
```

Sequential initialization:
1. `#8730`: Root CA certificate loading (`data/cacert.pem`);
2. `#7912`: Accessibility IPC handlers;
3. `#4943`: Core application IPC (`APP_*`);
4. `#7126`: Clipboard IPC;
5. `#6577`: Constants provider (`APP_NAME`, `API_ENDPOINT`);
6. `#7138`: Crash reporting metadata IPC;
7. `#2236`: Desktop capturer sources;
8. `#1353`: Feature flag injection;
9. `#9473`: File manager IPC;
10. `#8750`: Display metrics;
11. `#7635`: Clips protocol setup (`disclip://`);
12. `#3132` / `#1742`: Spellcheck / performance tracing;
13. `#3386`: Native window management;
14. `#5544`: WebAuthn registration.

---

## 2. Window Configurations and Security Profiles

### 2.1 Splash Window (`app/bundle.js:12`, `#53006`, @86584..101462)

```js
// app/bundle.js:12 (#53006 @91315..@91700)
useWebp = appSettings.getSettings()?.get("DESKTOP_TTI_SPLASH_USE_WEBP", !0) === !0,
windowConfig = {
    width: 300, height: LOADING_WINDOW_HEIGHT, transparent: !1, frame: !1, resizable: !1, center: !0, show: !1,
    webPreferences: {
        nodeIntegration: !1, sandbox: !1, contextIsolation: !0,
        preload: path_1.default.join(__dirname, "splashScreenPreload.js"), // @91515
        additionalArguments: [`--splash-media-format=${useWebp ? "webp" : "webm"}`] // @91601
    }
};
splashWindow = new electron_1.BrowserWindow(windowConfig) // @91653
```

Splash IPC Channels:
* `SPLASH_SCREEN_READY`: Resynchronizes updater state;
* `SPLASH_SCREEN_QUIT`: Triggers application shutdown;
* `SPLASH_UPDATE_STATE`: Dispatches updater progress payloads;
* `SPLASH_SCREEN_QUOTE`: Renders splash quote string.

### 2.2 Main Window (`core/bundle.js:1`, `#5203`, @119531..156960)

```js
// core/bundle.js:1 (#5203 @141300..@142450)
let additionalArguments = ["--enable-node-leakage-in-renderers"]; // @141476
"win32" === process.platform && settings?.get("DESKTOP_NATIVE_PREMAP", !1) === !0 &&
    additionalArguments.push("--enable-native-premap"), // @141637
let mainWindowOptions = {
    title: "Discord", backgroundColor: getBackgroundColor(), width: 1280, height: 720,
    minWidth: MIN_WIDTH, minHeight: MIN_HEIGHT, transparent: !1, frame: !1, resizable: !0, show: isVisible,
    webPreferences: {
        enableBlinkFeatures: "EnumerateDevices,AudioOutputDevices", nodeIntegration: !1, sandbox: !1,
        preload: path_1.default.join(__dirname, "mainScreenPreload.js"),
        spellcheck: !0, contextIsolation: !0, additionalArguments, devTools: ENABLE_DEVTOOLS
    }
};
"linux" === process.platform && (mainWindowOptions.icon = path_1.default.join(path_1.default.dirname(app.getPath("exe")), "discord.png"));
```

Default dimensions: Minimum width/height default to **940x500** (`settings?.get("MIN_WIDTH", 940)`).

### 2.3 Popouts and Overlay Host (`core/bundle.js:1`, `#9698`, @178984..183232)

Popout windows inherit standard security properties (`nodeIntegration: false`, `contextIsolation: true`, `sandbox: false`) pointing to `mainScreenPreload.js`.
External links intercepted via `webContents.setWindowOpenHandler` and routed to the OS default browser via `shell.openExternal`.

For out-of-process overlay hosts (`--overlay-host`, executing `discord_overlay2/standalone_host.js`), window options configure transparency and focus:
```js
if (windowOptions.outOfProcessOverlay) {
    windowOptions.title = "Discord Overlay", windowOptions.show = !1, windowOptions.backgroundColor = "#00000000";
}
```

---

## 3. IPC Architecture and Complete Call Chains

### 3.1 `DISCORD_` Channel Namespacing

IPC communication employs a three-tier naming convention:
1. Internal handlers in main register unscoped event names (`SPLASH_SCREEN_READY`, `INSTALL`);
2. Wrapper `#50902` in `app` prefixes outgoing events with `DISCORD_`;
3. Preload layers enforce strict whitelists on allowed channel identifiers.

### 3.2 End-to-End Chain: Notifications

`DISCORD_NOTIFICATIONS_SEND_NOTIFICATION`:
1. Renderer invokes `DiscordNative.notifications.showNotification(...)`;
2. Preload transmits over IPC `DISCORD_NOTIFICATIONS_SEND_NOTIFICATION`;
3. Main process `#1326` unwraps notification options, checks OS capabilities via `discord_utils.shouldDisplayNotifications()`;
4. Renders native desktop notification or delegates to libnotify on Linux.

### 3.3 End-to-End Chain: Native Module Installation

`DISCORD_NATIVE_MODULES_INSTALL`:
1. Renderer executes `DiscordNative.nativeModules.ensureModule("discord_krisp")`;
2. Dispatches `ipcRenderer.invoke(DISCORD_NATIVE_MODULES_INSTALL, name)`;
3. Main process `#8974` validates module name against `^discord_[a-z0-9_-]+$`;
4. Invokes `updater.installModule(name)` and responds with status upon disk write.

### 3.4 End-to-End Chain: Window Handle Resolution

`DISCORD_WINDOW_GET_NATIVE_HANDLE`:
1. Dispatches sync call `ipcRenderer.sendSync(DISCORD_WINDOW_GET_NATIVE_HANDLE)`;
2. Main executes `window.getNativeWindowHandle()`;
3. Returns native pointer buffer (HWND on Windows, XID / Wayland subsurface handle on Linux).

### 3.5 End-to-End Chain: Montage Recipe Rendering (`disclip://`)

1. Renderer constructs montage recipe and dispatches `DISCORD_RENDER_MONTAGE_RECIPE`;
2. Main validates executable signature and spawns video processing child process;
3. Previews served via registered custom protocol `disclip://<clipId>`.

---

## 4. Main Process Native Module Bindings

### 4.1 `requireNative` Module Resolution

`requireNative` resolves `.node` binaries from paths populated by `updater.queryCurrentVersionsSync().current_modules`.

### 4.2 Native Invocations from Main Process

Main process code invokes:
- `updater.node`: Version verification, delta updates, module installation;
- `discord_utils.node`: Power save blocks and system idle queries (`core #4531`, `isWaylandIdleAvailable()`, `getWaylandSystemIdleTimeMs()`), window PID lookups, minidump generation.
Voice and audio engines (`discord_voice`, `discord_krisp`) are **never directly invoked from the Electron main process**; they are consumed strictly in renderer contexts mediated by preload scripts.

---

## 5. Auto-Updater Architecture

### 5.1 Dual Updater Engines

- **Modern Updater (`updater.node`):** Rust engine handling manifests, LZMA/Zstandard chunks, binary diffs (`bsdiff`/`zucchini`), and SQLite version registers;
- **Legacy Module Updater (`moduleUpdater`):** Fallback mechanism downloading standalone tarballs.

### 5.2 Linux Distribution Considerations

On Linux, auto-update self-patching is disabled in package-managed environments (`.deb`, `.rpm`, Flatpak, Snap). The client transitions to the `update-manually` UI state directing users to download upstream tarballs when new builds are published.

---

## 6. Local Settings and Feature Configuration

Feature flags are held in a local encrypted key-value store. Configuration values include:
- `DESKTOP_TTI_SPLASH_USE_WEBP`: Enables WebP splash animation;
- `DANGEROUS_ENABLE_DEVTOOLS_ONLY_ENABLE_IF_YOU_KNOW_WHAT_YOURE_DOING`: Unlocks Chromium DevTools on stable builds;
- `enableHardwareAcceleration`: Toggles GPU rasterization.

---

## 7. Networking, Proxies, and Security Certificates

- **Custom Root CA:** Loads `data/cacert.pem` to prevent interception on managed enterprise environments;
- **Connectivity Probes:** Preflights endpoint availability against `status.discord.com` and telemetry collectors before full socket activation;
- **DNS / TCP Pre-warming:** Warms socket connections to gateway hosts on startup.

---

## 8. Crash Reporting Infrastructure

- **Sentry Integration:** Captures uncaught exceptions in both main and renderer processes using `@sentry/electron`;
- **Crashpad Minidumps:** Crashpad daemon writes raw minidump files on crash; `blackbox` harvests minidump files on next bootstrap and transmits to Sentry intake.

---

## 9. System Integrations: Tray, Menus, Protocols

- **System Tray:** Managed via `core/bundle.js:1` `#1275`. Listens for window minimize events and toggles visibility;
- **Protocol Schemes:** Handles `discord://` URI activations and passes command-line targets into the active window.

---

## 10. Architectural Conclusions

1. **Isolation of Media Pipeline:** Media decoding and RTC signaling run isolated from the main process, protecting desktop application stability from voice driver crashes.
2. **Deterministic Preload Whitelist:** All renderer access to native capabilities is strictly gated through validated preload bridge functions.
3. **Resilient Offline State Machine:** Splash and updater components handle degraded network connectivity gracefully before main window creation.
