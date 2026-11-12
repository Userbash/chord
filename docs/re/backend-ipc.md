# Discord Desktop 1.0.162 — Backend/IPC Topology (main ↔ renderer)

Version: **1.0.162** (build_info.json: releaseChannel=stable, sentryDist=stable-linux-x64, release `7e0a1fef1fc1295b33b9404e5b77f1ae39d0c6e0`), Electron 42.11.12 / Chrome 148 (User-Agent header in CDP capture). Static analysis only: ASAR unpack + CDP splash trace inspection. No live processes attached.

## 0. Methodology and Validation

| Step | Outcome |
|---|---|
| ASAR unpack `app.asar` | 11 files, 3,618,590 bytes; **11/11 SHA256 hashes matched** ASAR header integrity |
| ASAR unpack `core.asar` | 5 files, 1,761,514 bytes; **5/5 SHA256 hashes matched** |
| Bundle disassembly | 4 JS bundles, **1,266 modules** (rspack/webpack module factories), 428 IPC call-site invocations |
| Independent cross-validation | Enum `IPCEvents` (#595, 176 values) acting as channel name dictionary; live `capture-boot.jsonl` verifying splash channels |

Target Artifacts:

| File | Size | Role | Module Count |
|---|---|---|---|
| `app/bundle.js` | 3,066,301 B | main (bootstrapper / splash host / module updater) | 825 |
| `core/bundle.js` | 402,296 B | main (desktop core) | 81 |
| `core/mainScreenPreload.js` | 1,105,621 B | main window preload (`contextIsolation: true`) | 357 |
| `app/splashScreenPreload.js` | 9,333 B | splash window preload | 3 |

Prefix Invariant Rule (verified in bidirectional code): `getDiscordIPCEvent(ev) = /^DISCORD_/.test(ev) ? ev : "DISCORD_"+ev`. The main bundle employs wrapper `ipcMain_1.default.{on,handle,reply,removeListener}` (core/bundle.js #2799, app/bundle.js #50902) that filters every event name through this helper; handlers in main are frequently written without the prefix (`"SYSTEM_TRAY_SET_ICON"`), resolving to `DISCORD_SYSTEM_TRAY_SET_ICON`.

## 1. Statistics

| Metric | Quantity |
|---|---|
| Unique Channels (Canonical Names) | **168** |
| Channels Handled in Main Process | **152** |
| Channels Invoked from Renderer / Preload | **129** |
| Channels Verified Bidirectionally | **115** |
| Push Channels (main → renderer) | **7** |
| Request-Response Channels (`handle`/`invoke`/`sendSync`) | **128** |
| Fire-and-Forget Channels (`on`/`send`) | **40** |
| Members in Enum `IPCEvents` | **176** |
| Channels Filtered Out as Noise | **85** |
| Call-Sites with Dynamically Evaluated Channel Names | **47** |

Coverage: **168** channels; **161** are represented in enum `IPCEvents` (#595). **7** channels originate exclusively in app.asar: `DISCORD_SPLASH_SCREEN_{READY,QUIT,QUOTE}`, `DISCORD_SPLASH_UPDATE_STATE`, `DISCORD_GET_BUILD_OVERRIDE_STATUS`, `DISCORD_CLEAR_BUILD_OVERRIDE`, and `DISCORD_MODULE_INSTALLED`. **15** enum members appear strictly in renderer web-client whitelists.

## 2. Channel Functional Groups

| Group | Count | Main Modules | Renderer Modules | Channels (Prefix Omitted) |
|---|---|---|---|---|
| Window | 25 | `core/#3386, #5203` | `preload#5639` | WINDOW_BLUR · WINDOW_CLOSE · WINDOW_DEVTOOLS_CLOSED · WINDOW_DEVTOOLS_OPENED · WINDOW_FLASH_FRAME · WINDOW_FOCUS · WINDOW_GET_MEDIA_SOURCE_ID · WINDOW_GET_NATIVE_HANDLE · WINDOW_IS_ALWAYS_ON_TOP · WINDOW_MAXIMIZE · WINDOW_MINIMIZE · WINDOW_OPEN_DOCUMENT_PIP · WINDOW_RESTORE · WINDOW_SET_ALWAYS_ON_TOP · WINDOW_SET_BACKGROUND_THROTTLING · WINDOW_SET_CONTENT_PROTCTION · WINDOW_SET_FOCUSABLE · WINDOW_SET_FRAME_RATE · WINDOW_SET_MINIMUM_SIZE · WINDOW_SET_PROGRESS_BAR · WINDOW_SET_TRAFFIC_LIGHT_APPEARANCE · WINDOW_SET_TRAFFIC_LIGHT_POSITION · WINDOW_SET_WINDOW_CONTENT_PROTECTION · WINDOW_SHOW_INACTIVE · WINDOW_TOGGLE_FULLSCREEN |
| App | 21 | `core/#5203, #6820, #4943, #4973` | `preload#2100, #1405` | APP_ASYNC_INDEX_TSX_LOADED · APP_BADGE_SET · APP_DOCK_BOUNCE · APP_DOCK_CANCEL_BOUNCE · APP_DOCK_SET_BADGE · APP_GET_ARCH · APP_GET_BUILD_NUMBER · APP_GET_DEFAULT_DOUBLE_CLICK_ACTION · APP_GET_HOST_VERSION_SYNC · APP_GET_MODULE_VERSIONS · APP_GET_OPEN_ON_START · APP_GET_PATH · APP_GET_PREFERRED_SYSTEM_LANGUAGES · APP_GET_RELEASE_CHANNEL_SYNC · APP_GET_SYSTEM_UI_DIRECTION_SYNC · APP_MAIN_BUNDLE_STATS · APP_PAINT_METRICS · APP_PAUSE_FRAME_EVICTOR · APP_RELAUNCH · APP_SET_BADGE_COUNT · APP_UNPAUSE_FRAME_EVICTOR |
| Miscellaneous | 14 | `app/#53006, core/#7912, #6577...` | `preload#7523, #723, #8986...` | ACCESSIBILITY_GET_ENABLED · CONSTANTS_GET · CRASH_REPORTER_UPDATE_METADATA · DESKTOP_CAPTURER_GET_SOURCES · FEATURES_GET_BROWSER_FEATURES · GET_MOUSE_COORDINATES · HARDWARE_GET_DISPLAY_COUNT · MODULE_INSTALLED · NAVIGATION_HISTORY_CLEAR · OPEN_EXTERNAL_URL · TRACING_CAPTURE_PERFORMANCE · UNHANDLED_JS_EXCEPTION · UPDATED_QUOTES |
| Process & Metrics | 12 | `core/#9984` | `preload#9141` | PROCESS_UTILS_FLUSH_COOKIES · PROCESS_UTILS_FLUSH_DNS_CACHE · PROCESS_UTILS_FLUSH_STORAGE_DATA · PROCESS_UTILS_GET_CPU_USAGE · PROCESS_UTILS_GET_GPU_PROCESS_ID · PROCESS_UTILS_GET_LAST_CRASH · PROCESS_UTILS_GET_MAIN_ARGV_SYNC · PROCESS_UTILS_GET_SYSTEM_INFO · PROCESS_UTILS_GET_SYSTEM_METRICS · PROCESS_UTILS_SET_CRASH_INFORMATION · PROCESS_UTILS_SET_CRASH_REASON · PROCESS_UTILS_SET_MEMORY_INFORMATION |
| Filesystem | 9 | `core/#9473` | `preload#3466` | FILE_MANAGER_CREATE_DIRECTORY_IF_NOT_EXISTS · FILE_MANAGER_GET_ASSET_CACHE_PATH · FILE_MANAGER_GET_ASSET_CACHE_PATH_SYNC · FILE_MANAGER_GET_MODULE_DATA_PATH_SYNC · FILE_MANAGER_GET_MODULE_LOG_PATH_SYNC · FILE_MANAGER_GET_MODULE_PATH · FILE_MANAGER_SHOW_ITEM_IN_FOLDER · FILE_MANAGER_SHOW_OPEN_DIALOG · FILE_MANAGER_SHOW_SAVE_DIALOG |
| Tray & Autostart | 9 | `core/#1275, #9090, #4973` | — | SYSTEM_TRAY_SET_APPLICATIONS · SYSTEM_TRAY_SET_ICON · SYSTEM_TRAY_SET_STATES · SYSTEM_TRAY_SET_STRINGS · THUMBAR_BUTTONS_UPDATE · TOGGLE_MINIMIZE_TO_TRAY · TOGGLE_OPEN_ON_STARTUP · TOGGLE_START_MINIMIZED · UPDATE_OPEN_ON_STARTUP |
| Clipboard | 8 | `core/#7126` | `preload#6701` | CLIPBOARD_COPY · CLIPBOARD_CUT · CLIPBOARD_HAS_MIXED_CONTENT · CLIPBOARD_PASTE · CLIPBOARD_READ_TEXT · CLIPBOARD_WRITE_FILE · CLIPBOARD_WRITE_IMAGE · CLIPBOARD_WRITE_TEXT |
| Power Monitor | 8 | `core/#4531, #7420` | `preload#2818, #2853` | POWER_MONITOR_GET_SYSTEM_IDLE_TIME · POWER_MONITOR_LOCK_SCREEN · POWER_MONITOR_RESUME · POWER_MONITOR_SUSPEND · POWER_MONITOR_UNLOCK_SCREEN · POWER_SAVE_BLOCKER_BLOCK_DISPLAY_SLEEP · POWER_SAVE_BLOCKER_CLEANUP_DISPLAY_SLEEP · POWER_SAVE_BLOCKER_UNBLOCK_DISPLAY_SLEEP |
| Clips | 7 | `core/#7635` | `preload#6752` | DEBUG_STASH_CLIP_DECIDER_DATA · DELETE_CLIP · LOAD_CLIP · LOAD_CLIPS_DIRECTORY · MOVE_CLIP · RENDER_MONTAGE_RECIPE · UPDATE_CLIP_METADATA |
| Notifications | 7 | `core/#7592` | — | NOTIFICATIONS_GET_AUTHORIZATION · NOTIFICATIONS_GET_MODULE_STATUS · NOTIFICATIONS_GET_SETTINGS · NOTIFICATIONS_RECEIVED_RESPONSE · NOTIFICATIONS_REMOVE_ALL_NOTIFICATIONS · NOTIFICATIONS_REMOVE_NOTIFICATIONS · NOTIFICATIONS_SEND_NOTIFICATION |
| Splash & Bootstrap | 6 | `app/#96020` | `preload#85425` | CLEAR_BUILD_OVERRIDE · GET_BUILD_OVERRIDE_STATUS · SPLASH_SCREEN_QUIT · SPLASH_SCREEN_QUOTE · SPLASH_SCREEN_READY · SPLASH_UPDATE_STATE |
| Spellchecker | 6 | `core/#3132` | `preload#9121` | SPELLCHECK_GET_AVAILABLE_DICTIONARIES · SPELLCHECK_REPLACE_MISSPELLING · SPELLCHECK_RESULT · SPELLCHECK_SET_LANGUAGES · SPELLCHECK_SET_LEARNED_WORDS · SPELLCHECK_SET_LOCALE |
| Native Modules | 5 | `core/#216` | — | NATIVE_MODULES_FINISH_UPDATER_BOOTSTRAP · NATIVE_MODULES_GET_HAS_NEW_UPDATER · NATIVE_MODULES_GET_MODULE_PATH · NATIVE_MODULES_GET_PATHS · NATIVE_MODULES_INSTALL |
| WebAuthn | 5 | `core/#5544` | `preload#3797` | WEBAUTHN_AUTHENTICATE_MAC · WEBAUTHN_REGISTER_MAC · WEBAUTHN_SIGNAL_ALL_ACCEPTED_CREDENTIALS · WEBAUTHN_SIGNAL_CURRENT_USER_DETAILS · WEBAUTHN_SIGNAL_UNKNOWN_CREDENTIAL |
| Frame Debugger | 4 | `core/#7026` | `preload#3269` | FRAME_DEBUGGER_ATTACH · FRAME_DEBUGGER_DETACH · FRAME_DEBUGGER_DETACHED · FRAME_DEBUGGER_RUN_BATCH |
| Settings | 4 | `core/#7683, #5203` | `preload#6434` | SETTINGS_GET · SETTINGS_GET_SYNC · SETTINGS_SET · SETTINGS_UPDATE_BACKGROUND_COLOR |
| Search | 4 | `core/#4355` | — | SEARCH_CLEAR_INDEX · SEARCH_DELETE_DOMAINS · SEARCH_DELETE_ITEMS · SEARCH_INDEX_DOMAINS |
| GPU | 3 | `core/#1667` | `preload#4276` | GPU_SETTINGS_GET_ENABLE_HWACCEL_SYNC · GPU_SETTINGS_SET_CHROMIUM_SWITCHES · GPU_SETTINGS_SET_ENABLE_HWACCEL |
| NTP Clock | 3 | `core/#7314` | `preload#3939` | NTP_CLOCK_GET_SYNC_INFO · NTP_CLOCK_START · NTP_CLOCK_SYNCED |
| safeStorage | 3 | `core/#5454` | `preload#5261` | SAFE_STORAGE_DECRYPT_STRING · SAFE_STORAGE_ENCRYPT_STRING · SAFE_STORAGE_IS_ENCRYPTION_AVAILABLE |
| User Data Cache | 3 | `core/#299` | `preload#5984` | USER_DATA_CACHE_DELETE · USER_DATA_CACHE_GET · USER_DATA_CACHE_SAVE |
| Rich Presence | 2 | `core/#4355` | — | INTENTS_RESIGN_ACTIVITY · INTENTS_SET_ACTIVITY |

## 3. `DiscordNative` Bridge (Renderer Side)

File `core/mainScreenPreload.js` (main window preload: `contextIsolation: true`, `sandbox: false`):
- `contextBridge.exposeInMainWorld("DiscordNative", DiscordNative)`
- Bridge exposes **35** properties: 33 sub-APIs + flags `isRenderer` and `setUncaughtExceptionHandler`.

Preload Whitelists (`DiscordNative.ipc`, module #8981):
- `RENDERER_IPC_SEND_WHITELIST`: **30** permitted `send()` channels.
- `RENDERER_IPC_INVOKE_WHITELIST`: **14** permitted `invoke()` channels.
- Invocations to unlisted channels throw `Error("cannot send this event")` / `Error("cannot invoke this event")`.

### Native Modules Loading Mechanism
`DiscordNative.nativeModules.requireModule(name)` verifies regex `/^discord_[a-z0-9_-]+$/` or `erlpack` and rejects `PRELOAD_ONLY_MODULES`.
`getModulePath(name)` communicates with main via synchronous IPC: `ipcRenderer.sendSync(DISCORD_NATIVE_MODULES_GET_MODULE_PATH, name)`.
Search paths are populated via `NATIVE_MODULES_GET_PATHS`/`NATIVE_MODULES_INSTALL` (core/bundle.js #216).
Preloaded modules: `PREMAP_MODULES = ["discord_voice","discord_zstd","discord_cloudsync","discord_media"]`.

## 4. Process Architecture and Call Chains

| Process | Bootstrap / Responsibility | Source File |
|---|---|---|
| Main (Bootstrap) | Splash window, updater, Sentry, `requireNative("discord_desktop_core")` | `resources/app.asar → app/bundle.js` |
| Main (Core) | Main window, popouts, tray, CDP debugger, native Discord modules | `core.asar → core/bundle.js` |
| Renderer (Splash) | Splash UI + `splashScreenPreload.js` | `app.asar` |
| Renderer (Main) | Web client + `mainScreenPreload.js` (`DiscordNative` bridge) | `core.asar` |

`webPreferences` Settings:
- Splash: `nodeIntegration: false, sandbox: false, contextIsolation: true, preload: splashScreenPreload.js, frame: false`
- Main Window: `nodeIntegration: false, sandbox: false, contextIsolation: true, spellcheck: true, preload: mainScreenPreload.js`
- Popout Windows: `nodeIntegration: false, nativeWindowOpen: true, enableRemoteModule: false, contextIsolation: true`

Link between `app.asar` and `core.asar` is a direct function call (`requireNative`), not an IPC channel:
`core = requireNative("discord_desktop_core"); core.startup({...})`.

Native voice and media bypass IPC channels: calls transition directly from renderer preload to `.node` binaries via V8 N-API bindings.
