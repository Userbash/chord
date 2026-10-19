# Нативные модули Discord Desktop 1.0.162 (Linux x64) — глубокий инвентарь

**Цель.** Описать, что представляют собой нативные модули сборки Discord 1.0.162, что они экспортируют,
что реально реализовано (по символам), и где эти модули загружаются в рантайме.

**Что анализировалось.** Только чтение:

* `/home/sanya/.config/discord/app-1.0.162/` — файлы релиза (модули, `updater.node`, `core.asar`, `app.asar`);
* `/home/sanya/orca/project/functions/pid-*-Discord.json` (+CSV) — рантайм-инвентарь двух процессов;
* `functions/static/*.csv|*.nm.txt`, `evidence/asar-inventory.json` — как независимая сверка.

**Что НЕ делалось** (запрещено условиями задачи): запуск/остановка процессов, attach (Frida/GDB/CDP),
сетевые вызовы, установка пакетов, запись в `~/.config/discord`, `logs/`, `functions/`, `evidence/`.
Все артефакты анализа лежат в `/home/sanya/re-deep/`.

**Метод.** Собственная выгрузка символов: `nm -C --defined-only`, `nm -D -C`, `readelf -lW/-SW/-dW`,
`strings`, `file`; разбор asar собственным парсером; сверка с рантайм-инвентарём. Деманглинг — `nm -C`
(Itanium + Rust v0).

**Метрика.** Числа в скобках у символов ниже — **`unique_symbols`**: сколько *уникальных имён* символов
в деманглированной таблице определённых символов (`nm -C --defined-only`) содержат этот токен
(скрипты `final-stats.py`, `verify-syms.py`, `build-json.py` — воспроизводимо). Это не «число экспортов»
и не «число вызовов», а мера того, насколько подсистема «прошита» в бинаре. Отдельно помечены
значения из строк (`strings`) и из рантайм-инвентаря.

---

## 1. Таблица модулей

Размеры — байты файла на диске; `symtab/dynsym` — число определённых (`nm --defined-only`) записей,
`функций` — из них `t/T/w/W`; `GDF` — функции с видимостью `GLOBAL DEFAULT` в dynsym (то, что реально
видно линковщику/плагину); `memsz` — сумма `PT_LOAD.memsz` (сколько образ занимает при загрузке).

| модуль | размер | sha256 (16) | symtab | функций | dynsym | GDF | stripped | memsz |
|---|---:|---|---:|---:|---:|---:|:--:|---:|
| `discord_voice.node` | 126 136 024 | `96721305d59a4fdc` | 85 577 | 69 621 | 11 726 | 11 367 | нет | 37 219 949 |
| `discord_krisp.node` | 62 078 048 | `3d70b39670d5cf02` | 43 990 | 37 001 | 28 150 | 25 825 | нет | 47 611 273 |
| `libmediapipe.so` | 23 793 696 | `4dc7da21837b507e` | 35 315 | 26 942 | 24 183 | 18 989 | нет | 20 072 872 |
| `discord_utils.node` | 19 185 488 | `3dc68ed5675318dd` | 5 212 | 3 599 | 11 | 11 | нет | 1 328 293 |
| `discord_game_utils.node` | 11 478 904 | `4fe8b2382aa28dd8` | 4 002 | 2 642 | 11 | 11 | нет | 1 073 361 |
| `discord_erlpack.node` | 7 755 832 | `6cad518145244f52` | 2 546 | 1 449 | 11 | 11 | нет | 464 973 |
| `cld.node` (spellcheck) | 6 646 552 | `9365d00f027e8cd9` | 482 | 332 | 381 | 312 | нет | 6 598 774 |
| `updater.node` | 5 812 960 | `44c55e0bc8271162` | 0 | 0 | 9 | 9 | **да** | 5 807 253 |
| `discord_zstd.node` | 873 248 | `bb7cb593f979dd77` | 1 087 | 838 | 1 | 1 | нет | 747 477 |
| `gpu_encoder_helper` (CLI) | 1 027 416 | `17c73b9039e417ab` | 0¹ | 0¹ | 0 | 0 | частично¹ | 783 498 |
| `notificationstate.node` | 16 840 | `9660a012a89479b0` | 14 | 6 | 0 | 0 | нет | 1 925 |
| `core.asar` (desktop_core) | 1 762 838 | `27aacaa590e9511b` | — | — | — | — | — | — |
| `app.asar` | 3 621 566 | `6094a8ffdd7740d9` | — | — | — | — | — | — |
| `discord_rpc` | только JS | — | — | — | — | — | — | — |

¹ `gpu_encoder_helper`: `.symtab` присутствует (файл формально «not stripped»), `nm` находит 1 871
определённый символ, но **все они обезличены** LLVM-интернализацией: `anon.<hash>.N.llvm.<id>`.
`dynsym` пуст.

Сборка: `build_info.json` → version `1.0.162`, releaseChannel `stable`, sentryRelease
`discord-desktop-7e0a1fef1fc1295b33b9404e5b77f1ae39d0c6e0`, sentryDist `stable-linux-x64`.

### Пересечение `symtab` и того, что видно снаружи

Ключевой вывод таблицы: **у большинства модулей публичный API не виден в `dynsym`**. `discord_utils`,
`discord_game_utils`, `discord_erlpack` экспортируют ровно 11 динамических символов — C++-операторы
`new/delete` (`_Znwm`, `_ZdlPv`, …), `__libcpp_verbose_abort`, `napi_register_module_v1`,
`node_api_module_get_api_version_v1`. Вся полезная функциональность отдаётся **внутрь NAPI-объекта**
и в `dynsym` не отражается. Единственные исключения — `discord_zstd` (1 экспорт) и `cld.node`
(381). А вот `discord_voice` и `discord_krisp` — наоборот, «протекают» наружу тысячами символов
(см. §4), причём значительная часть этих экспортов — вообще не Discord, а влинкованные сторонние
библиотеки.

---

## 2. `discord_desktop_core` — это не нативный модуль, а asar

В `modules/` рядом с `.node` лежит `discord_desktop_core-1/discord_desktop_core/`:

```
index.js   (41 Б)   → module.exports = require('./core.asar');
package.json        → { "name": "discord_desktop_core", "main": "index.js" }
core.asar  (1 762 838 Б)
```

Внутри `core.asar`:

| файл | размер | роль |
|---|---:|---|
| `bundle.js` | 402 296 | главный процесс: окна, трей, bootstrap, обновление модулей (rspack-бандл, видны `__unused_rspack_module`) |
| `mainScreenPreload.js` | 1 105 621 | preload главного окна: собирает `globalThis.DiscordNative` и `nativeModules` |
| `data/cacert.pem`, `data/riotgames.pem` | — | встроенные CA (в т.ч. под Riot-интеграцию) |

Зависимости бандла (из `package.json`/текста): `@sentry/electron` 7.13.0, dev-зависимость `electron` 42.7.1.
Главный процесс из нативных модулей требует **только** `discord_utils` и `discord_notifications`.

### Как модули подключаются

1. `updater.node` лежит рядом с исполняемым файлом (`<execPath>/../updater.node`) и требуется напрямую —
   до появления `modules/`.
2. `requireNative("<name>")` — это обычный Node `Module` с подставленными `paths`; каталоги
   (`<host>/modules/<name>-<version>`) добавляются из `updater.queryCurrentVersionsSync().current_modules`
   (`addGlobalPath`). То есть **какая версия модуля загрузится, решает updater**.
3. `core.startup({Constants, GPUSettings, analytics, appSettings, autoStart, buildInfo, crashReporterSetup, logger, moduleUpdater, paths, splashScreen, updater})` — единая точка сборки главного процесса.
4. Preload (`mainScreenPreload.js`) отдаёт JS-стороне `DiscordNative`, в т.ч. `isRenderer: process.type === "renderer"` и `nativeModules` (модуль 7927), через `contextBridge.exposeInMainWorld`. **Это и есть загрузка `discord_voice`/`discord_krisp` в рендерер.**
5. Пути модулей приходят по IPC `NATIVE_MODULES_GET_PATHS`, установка по требованию — `NATIVE_MODULES_INSTALL`
   (`ensureModule`). Предзагрузка: `PREMAP_MODULES = ["discord_voice","discord_zstd","discord_cloudsync","discord_media"]`.
6. Имена, на которые ссылается JS, но которых нет на диске: `discord_cloudsync`, `discord_media`,
   `discord_sysimg`, `discord_hook`, `discord_notifications`.

`resources/bootstrap/manifest.json` перечисляет лишь 6 модулей — `desktop_core`, `erlpack`, `spellcheck`,
`utils`, `voice`, `zstd`, все с версией `0`. Значит, `krisp`, `game_utils`, `rpc` в bootstrap-манифест
не входят и версионируются иначе (у них есть собственные `manifest.json` со списком файлов).

---

## 3. Что в каком модуле реально реализовано (notable-символы)

Ниже — по 5–19 репрезентативных символов на модуль с пояснением; полный машинный список (с числом
уникальных символов и источником) — в `native-modules.json`.

### 3.1 `discord_voice.node` — медиа-движок (C++ + Rust + libwebrtc)

69 621 функция в `symtab`. Это самый «честный» модуль: символы не спрятаны.

| символ | что это |
|---|---|
| `discord::media::Connection` (309) | объект голосового соединения; из JS создаётся как `VoiceEngine.VoiceConnection` |
| `discord::media::Engine` (411) | корень движка: устройства, аудиоподсистема, инициализация — самый плотный C++-класс |
| `discord::media::VideoSource` (29) | источники видео (камера/экран/файл) |
| `discord::media::RemoteUser` (85) | состояние удалённого участника |
| `ScreenshareHelper` (27) | шаринг экрана: слои качества/битрейт/симулькаст |
| `MediaPipeFilter` (56) | фильтр фона: граф MediaPipe + `selfie_segmentation*.tflite` |
| `LinuxCaptureSource` (26) / `PipewireConfig` (32) / `show_portal_picker` (7) | Wayland/PipeWire-захват и вызов xdg-desktop-portal |
| `capture_gamescope` (76) | захват Gamescope-сессий (Steam Deck) |
| `cros_codecs` (567) | кодеки Chromium: VAAPI H.264/VP8/VP9/AV1 |
| `nvidia_media_encoding` (181) | NVENC-путь (Rust) |
| `vulkan_utils` (118) | Vulkan Video: `fused_scale_to_nv12.glsl` (строка в бинаре) |
| `mlspp::hpke` (509) | MLS/HPKE — криптослой E2EE групповых звонков (DAVE) |
| `discord::uv` (1 276) | Rust-биндинги libuv (сеть, таймеры, event loop) |
| `discord::node_api` (1 378) | Rust-обёртка NAPI: превращает Rust-типы в JS-объект VoiceEngine |
| `webrtc::internal` (414) | статически влинкованный libwebrtc (аудио, RTCP, ANA) |
| `73dc9167f1cd099e525c9ade2e068d1907b78564` | коммит `rustc` (133 совпадения в строках): один и тот же Rust-тулчейн у voice/updater/zstd |
| `gpu_encoder_helper` (2 совпадения в строках) | внешний CLI-пробник GPU-энкодеров |

JS-поверхность (28 обращений `VoiceEngine.*` в `index.js`): `initialize`, `createVoiceConnectionWithOptions`,
`createOwnStreamConnectionWithOptions`, `createReplayConnection`, `setVideoOutputSink`,
`addDirectVideoOutputSink`, `getNextVideoOutputFrame`, `signalVideoOutputSinkReady`, `setKrispPath`,
`setupKrispPath`, `setMLPath`, `setupMLPath`, `setImageDataAllocator`, `setActiveSinksChangeCallback`,
`setAudioSubsystem`, `queueAudioSubsystem`, `setOffloadAdmControls`, `isVaapiEnabled`,
`isSpatialAudioEnabled`, `setDebugLogging`, `getDebugLogging`, `consoleLog`, `platform`, `_createTransport`,
`createTransport`, `removeDirectVideoOutputSink`, `VoiceConnection`, `VoiceReplayConnection`.
В `index.js` есть `RUN_MEDIA_HOST = false` — в этой сборке отдельный медиа-хост не используется.

Рядом лежат `libmediapipe.so` (23.8 МБ), `gpu_encoder_helper` (1 МБ) и две tflite-модели
(`selfie_segmentation.tflite`, `selfie_segmentation_landscape.tflite`) — все три пути встречаются
строками внутри `discord_voice.node`, то есть загружаются динамически.

### 3.2 `libmediapipe.so` — ML-бэкенд фильтра фона

35 315 символов, из них 24 183 в `dynsym` (18 989 GLOBAL DEFAULT).

| символ | что это |
|---|---|
| `mediapipe::api2::` (528) | C++ API MediaPipe (графы «калькуляторов») |
| `ImageToTensorCalculator` (158) | кадр → тензор |
| `InferenceCalculatorCpuImpl` (44) | инференс на CPU |
| `InferenceCalculatorGlAdvancedImpl` (69) | инференс на GPU (OpenGL) |
| `ImagePropertiesCalculator` (23) | геометрия кадра |
| `tflite` (7 030) | влинкованный TensorFlow Lite (интерпретатор и ядра) |

### 3.3 `discord_krisp.node` — шумоподавление (Krisp SDK 4.0.25)

37 001 функция; 28 150 записей в `dynsym` (25 825 GLOBAL DEFAULT) — но это в основном **не API Krisp**.

| символ | что это |
|---|---|
| `krispGlobalInit` (6) | глобальная инициализация SDK |
| `krispCreateNcFloat` / `krispCreateNcInt16` (по 2) | создание шумодава для float/int16-буферов |
| `KrispNCProcess` (2) / `KrispNCSetup` / `KrispNCSetModel` | основной цикл шумоподавления и настройка модели |
| `KrispVADProcess` / `KrispVADSetup` (2) | детектор голосовой активности (в рантайме есть `KrispVADSetup`) |
| `KRISP::INFER::InferenceEngine` (7) | загрузка сети и инференс |
| `OnnxExecutableNetwork` (59) | исполнение модели через ONNX Runtime |
| `CPUExecutionProvider` (19) | инференс идёт на CPU (GPU-провайдер не подключён) |
| `_initialize` (8) | JS-точка входа; также `_getNcModels`, `_getVadModels`, `_getNcModelFilename` |
| `krisp-nc-processor` / `krisp-vad-v2-processor` (строки) | версии компонентов SDK: 4.0.25 и 1.0.3 |
| `/var/lib/jenkins/.conan/...` (24 строки) | сборочные пути CI/Conan-кэша Krisp |
| `SHA256_Update`, `sgemm_small_kernel_tt_HASWELL` (рантайм-экспорты) | OpenSSL и OpenBLAS, влинкованы статически и **выведены в экспорты** |

JS-обёртка (`index.js`, 1 088 Б) делает обёртки над NAPI: `_initialize({logDirectory, logLevel})`,
а также `getNcModels/getVadModels/getNcModelFilename` как Promise-обёртки над `_get*`.
Модели — 5 отдельных `.kef`-файлов: `krisp-nc-o-med-v7` (5.9 МБ), `krisp-nc-o-lite-v1` (2.9 МБ),
`krisp-nc-o-nb-v2` (2.8 МБ), `krisp-bvc-o-pro-v3` (15.1 МБ), `krisp-vad-o-v2` (0.6 МБ).
Имена моделей **в бинаре не встречаются** (0 совпадений) — выбор приходит из JS/конфигурации.

### 3.4 `discord_utils.node` — системная интеграция (PTT, игры, окна, диагностика)

| символ | что это |
|---|---|
| `discord::input::InputEvents` (58) | системный хук ввода: PTT и горячие клавиши вне окна |
| `InputEventRegister` (12) / `InputEventUnregister` (7) | регистрация и снятие отслеживаемых клавиш |
| `InputWatchAll` (11) | подписка на все события ввода; в JS живёт как `inputWatchAll` и удаляется из экспортов |
| `SetCandidateGamesCallback` (7) / `SetObservedGamesCallback` (14) / `SetGameDetectionCallback` (7) / `SetGameCandidateOverrides` | детект игр |
| `GetDiscordProcessObserver` / `SetProcessObserverUserId` / `SetForegroundProcess` | наблюдение за процессами и фокусом |
| `SetProcessObserverCollectExecutableFingerprint` (7) | сбор отпечатка исполняемого файла |
| `GetPidFromXWindow` / `FetchWindowPropertyString` / `XGetWindowProperty` (строка) | X11: PID и свойства окна |
| `WaylandIdleManager` (19) | Wayland: idle-состояние |
| `NativePermssion*` (22) | проверки разрешений; **опечатка `Permssion` реально присутствует в бинаре** |
| `GetDiscordMemoryUsage*` (3) / `GetDiscordClips*` (2) / `GetDiscordIsElevated` | память, Clips, признак повышенных прав |
| `_generateLiveMinidump` (строка в бинаре + JS) | живой minidump (JS сам отправляет его в Sentry) |

В `index.js` экспорты **правятся на месте**: `clearCandidateGamesCallback = setCandidateGamesCallback`,
`delete nativeUtils.inputWatchAll`, при `isElectronRenderer` добавляется `inputCaptureRegisterElement`
(из `input_capture.js`), а `inputEventRegister/Unregister` оборачиваются (`input_event.js`).
Плюс JS добавляет `getGPUDriverVersions` (через `nvidia-smi`, только win32) и `submitLiveCrashReport`
(эндпоинт Sentry зашит прямо в JS). Итог: набор API зависит от того, где загружен модуль — в main или в renderer.

### 3.5 `discord_game_utils.node` — детект игр

`IdentifyGame` (9 символов), `PlatIdentifyGame`, `FindLaunchable` (13), `SetRecentGames`,
`IsProtocolSchemeRegistered`, `ReadIcon` — идентификация игры, поиск запускаемого таргета, иконки,
проверка схемы протокола. Отдельно: влинкованный **libsodium** (`argon2_*` — 10 символов,
`blake2b_*` — 22, `crypto_scalarmult_curve25519` — 34, `crypto_onetimeauth_poly1305` — 22).
Где именно он применяется — по символам не видно (пробел).

### 3.6 `discord_erlpack.node` — распаковка ETF

`Decoder::decode{Array, Atom, Big, BinaryAsString, Compressed, Export, Float, Integer, List, Map,
NewFloat, NewReference, Nil, PID, Port, Reference, SmallAtom, SmallBig, SmallInteger, SmallTuple,
StringAsList, Tuple}` — 22 точки входа декодера, включая `Compressed` (сжатый терм) и
`BinaryAsString` (совместимость). JS-API — одна функция `unpack`. **Пакера нет**: модуль только
распаковывает термы Discord Gateway в JS-объекты (что согласуется с наличием отдельного `zstd`
для сжатых полезных нагрузок).

### 3.7 `discord_zstd.node` — распаковка zstd (Rust, napi-rs)

Класс `discord_zstd::bindings::DecompressStream` с `decompress`, `get_last_error`, `reset`
плюс `initialize_logging`. Внутри — декодер zstd: `ZSTD_*`, `FSE_decompress*`, `HUF_decodeStreamX4`,
`BIT_reloadDStream_internal`. JS: `createContext() → {id, decompress(data), getLastError()}` —
**один активный контекст на процесс**, повторное использование через `reset()`; обращение к
устаревшему контексту бросает Error. Компрессора нет вообще.
В рантайме сохранён Rust v0 mangling (`_RINvNt..._12discord_zstd`) — символы не деманглировались при сборке.

### 3.8 `cld.node` (модуль `discord_spellcheck`) — определение языка

`NodeCld::Detect`, `NodeCld::DetectAsync` + `DetectAsyncWorker` (через `Napi::AsyncWorker`, не блокирует
event loop), `NodeCld::Constants::{getLanguages,getEncodings,getDetected,init}`, `UnpackInputFromJSArgs`,
`UnpackOutputToJS`. Ядро — статически влинкованный **CLD2** (`CLD2::DetectLanguageSummaryV2`,
`CLD2::UTF8GenericScan`, `CLD2::ScriptScanner`, таблицы `kQuad0122`, `kCLDLangTagsHintTable*`, …) —
332 функции CLD2, выведенные в `dynsym` (312 GLOBAL DEFAULT). JS: `cld.detect(text)`.
Название модуля (`spellcheck`) не соответствует содержимому: это языковой детектор, а не проверка орфографии.

### 3.9 `notificationstate.node` — пустая заглушка на Linux

14 определённых символов, и все — CRT-обвязка (`_init`, `frame_dummy`, `register_tm_clones`,
`completed.7381`, `__dso_handle`, …). **Нет ни `napi_register_module_v1`, ни экспортов**, `dynsym` пуст.
JS-обёртка (`windows-notification-state/lib/index.js`) на не-win32 возвращает `{}` и бросает
`Error('windows-notification-state only works on windows')`. То есть на Linux модуль неработоспособен
by design, а `discord_utils/index.js` вызывает `getNotificationState()` только под `process.platform === 'win32'`.
Фактическая функциональность (Windows `QueryUserNotificationState`) в этой сборке отсутствует.

### 3.10 `updater.node` — загрузчик и установщик модулей

Полностью stripped: **0 символов в `.symtab`**, в `dynsym` ровно 9 имён:

```
napi_register_module_v1
__externref_table_alloc      __externref_table_dealloc   __externref_drop_slice
__externref_heap_live_count  __wbindgen_malloc           __wbindgen_free
__wbindgen_realloc           __wbindgen_exn_store
```

Это каркас **wasm-bindgen + wasm2c**: модуль скомпилирован из WebAssembly (`__externref_*` — таблица
внешних ссылок JS-объектов), а не написан сразу на Rust-C-ABI. Строки показывают зависимости:
`reqwest 0.11.9`, `rusqlite`, `tokio`, `rustls`, `brotli`, `bsdiff` + `zucchini` (бинарные диффы),
а пути исходников — `/ci/build/discord/discord/discord_updater/updater_client/src/{api,db,install,lib,query,repair}.rs`.
Итог: HTTP-клиент + SQLite-база версий + установка/починка модулей + загрузка диффов.

### 3.11 `gpu_encoder_helper` — CLI-пробник аппаратных энкодеров

ELF 64-bit PIE **исполняемый файл** (не библиотека), линкуется только с
`libdl/libgcc_s/libpthread/libc/ld-linux`, а вендорные библиотеки подгружает в рантайме:

| символ/строка | что это |
|---|---|
| `libva.so.2`, `libva-drm.so.2` | dlopen VA-API (AMD/Intel), модуль `vaapi_media_encoding::stubs`; рядом строки `libva loaded successfully`, `VA library not initialized`, `VA library not available` |
| `libnvidia-encode.so.1` (+ `libnvidia-encode.so`) | dlopen NVENC; символ `NvEncodeAPICreateInstance` |
| `clap` (12 строк с упоминанием, 470 совпадений) | CLI на Rust clap — это утилита, а не библиотека |
| `anon.<hash>.N.llvm.<id>` (1 871) | все имена символов обезличены LTO-интернализацией |
| `vulkan`/`vaapi`/`nvidia`/`amd`/`intel` (2/4/5/4/3 строки) | строки проб вендоров |

В `discord_voice.node` встречается строка `gpu_encoder_helper` (рядом — `Failed to create native GPU
encoder in Rust encoder` и путь `discord_media/components/video/interop/webrtc-interop/cpp/webrtc-interop/encoding/rust_encoder.cpp`)
и `GPU probe result for …`. Значит, voice запускает этот helper как отдельный процесс для проверки
энкодеров; сам вызов из JS-обёртки `index.js` не виден.

### 3.12 `discord_rpc` — вообще без нативного кода

`index.js` → `{ RPCIPC, RPCWebSocket }`; файлы `RPCIPC.js` (2 972 Б), `RPCWebSocket.js` (3 358 Б),
`safeEmitter.js` (1 301 Б). Единственная зависимость — `node_modules/ws`. В `modules/` есть,
в bootstrap-манифесте нет.

---

## 4. Рантайм: где модули действительно загружаются

Источник — два снимка инвентаря (только чтение):

**pid 23548 — updater/bootstrap процесс** (2 модуля):

| модуль | base | размер | экспортов | символов |
|---|---|---:|---:|---:|
| `updater.node` | `0x7f0815000000` | 5 816 392 | 9 | 187 |
| `discord_utils.node` | `0x7f0812356000` | 1 335 680 | 11 | 3 807 |

**pid 23877 — процесс с медиа-модулями** (6 модулей):

| модуль | base | размер | экспортов | символов | обрезано |
|---|---|---:|---:|---:|---:|
| `discord_voice.node` | `0x7f0a77600000` | 37 231 944 | 11 699 | 50 000 | **57 065** |
| `discord_utils.node` | `0x7f0a87db7000` | 1 335 680 | 11 | 3 807 | — |
| `discord_erlpack.node` | `0x7f0a88e85000` | 472 640 | 11 | 1 554 | — |
| `discord_zstd.node` | `0x7f0a86444000` | 756 424 | 1 | 886 | — |
| `discord_game_utils.node` | `0x7f0a83ef6000` | 1 080 528 | 11 | 2 818 | — |
| `discord_krisp.node` | `0x7f09e3c90000` | 47 621 928 | 28 091 | 37 368 | — |

Наблюдения:

* **Все записи рантайма — `type=function`**, то есть это динамические экспорты (dynsym), а не отладочные
  таблицы; рантайм-числа совпадают с измеренными `dynsym` (voice 11 699 ≈ 11 726; krisp 28 091 ≈ 28 150).
* **Размер в рантайме ≈ сумма `PT_LOAD.memsz`**: voice 126 МБ файла → 37.2 МБ отображения (BSS/выравнивание),
  krisp 62 МБ → 47.6 МБ, utils 19.2 МБ → 1.33 МБ. То есть «модуль занимает столько памяти» — неверная
  интерпретация: значительная часть — пустое место в отображаемых сегментах.
* `discord_voice` в рантайме отдаёт наружу **11 699 экспортов**, и это в основном влинкованные
  библиотеки: libspa-rs (`spa_pod_builder_*_libspa_rs`), VA-API (`vaDestroyConfig`, `vaMaxNumEntrypoints`),
  Chromium-кодеки (`cc_video_frame_get_data`). У `krisp` аналогично видны OpenSSL/OpenBLAS
  (28 091 экспорт при том, что API Krisp — это десятки функций).
* Символы `zstd` сохранили Rust v0 mangling.
* Загрузка по требованию: `PREMAP_MODULES` (voice, zstd, cloudsync, media) объясняет, почему
  в снимке нет `cld.node`/`notificationstate.node` — они подгружаются только при обращении.
* **Ограничение: тип процесса (main/utility/renderer) по артефактам не определяется** — нет cmdline,
  все PID уже мертвы. Косвенно: pid 23548 — это bootstrap (только updater+utils), pid 23877 —
  процесс, где живут voice/krisp/erlpack/zstd/game_utils. При этом локальный JS показывает, что
  поверхность загрузки `discord_voice`/`discord_krisp` — **preload главного окна**
  (`mainScreenPreload.js`), а README репозитория называет его voice-utility. Расхождение не разрешено.

---

## 5. Как это реализовано «под капотом» — 5 наблюдений

1. **Два разных стиля сборки.** C++-модули (`voice`, `utils`, `game_utils`, `erlpack`) — это
   `napi_register_module_v1` + `node-addon-api`, с непустым `symtab` и почти пустым `dynsym`
   (11 экспортов). Rust-модули (`zstd`, части `voice`, `updater`) — napi-rs или wasm-bindgen.
   `updater.node` вообще скомпилирован из WASM (`__externref_*`, `__wbindgen_*`) и stripped до 9 экспортов.
2. **Сторонние библиотеки влинкованы статически и «протекают» в экспорты.** krisp = Krisp SDK
   + ONNX Runtime + OpenSSL + OpenBLAS; libmediapipe = MediaPipe + TensorFlow Lite;
   cld = CLD2; voice = libwebrtc + libspa-rs + VA-API; game_utils = libsodium.
   Из-за этого «число экспортов модуля» ничего не говорит о его API.
3. **Аппаратное кодирование вынесено в отдельные сущности.** В voice — Rust-подсистемы
   `cros_codecs` (VAAPI), `nvidia_media_encoding` (NVENC), `vulkan_utils` (Vulkan Video,
   GLSL-шейдер `fused_scale_to_nv12.glsl`), плюс отдельный процесс-пробник `gpu_encoder_helper`,
   который сам решает, какая вендорная библиотека доступна (dlopen `libva.so.2`, `libnvidia-encode.so.1`).
   В сборке **нет** `nvcuvid`/`nvdec` (ни в `Discord`, ни в `libffmpeg.so`, ни в `discord_voice.node`),
   а в `libffmpeg.so` из hwaccel-строк есть только `d3d11va`, `av_d3d11va_alloc_context`, `vaapi`, `vulkan`
   (для `nvenc` — 0 совпадений), то есть это перечисление имён ускорителей, а не подтверждённая
   Linux-реализация. Аппаратное декодирование на Linux не подтверждено.
4. **Захват на Linux — это Rust, а не C++.** `discord::media::capture_linux::{LinuxCaptureSource,
   PipewireConfig, picker::show_portal_picker}` и `capture_gamescope` — то есть Wayland/PipeWire через
   xdg-desktop-portal и отдельная поддержка Gamescope (Steam Deck). Рядом — краты `wayland`, `pipewire`,
   `zbus` и `nix`/`rustix` для системных вызовов.
5. **Безопасные звонки (E2EE/DAVE) — на Rust через MLS, шумодав — локальный CPU-only.**
   В voice присутствует `mlspp::hpke` (509 символов) — библиотека MLS/HPKE; JS лишь передаёт фреймы,
   криптография — на нативной стороне. Шумоподавление Krisp считается на CPU
   (ONNX Runtime `CPUExecutionProvider`), фильтр фона — MediaPipe + tflite selfie-segmentation.

---

## 6. Пробелы и ограничения

1. **Нет декомпиляции.** REA-провайдеры для глубокого разбора (`inspect_binary_layout` с
   pwntools 4.15.0/pyelftools 0.33, Ghidra-класс) в этом окружении недоступны. Все выводы — по символам,
   строкам, импортам и JS-обёрткам; «как устроено внутри функции» не восстановлено.
2. **Роль процессов не подтверждена.** Нет cmdline/типа процесса, PID мертвы; разделение
   main/utility/renderer опирается только на косвенные признаки.
3. **Символы voice обрезаны:** `symbolsTruncated=57065` — больше половины строк отброшено лимитом 50 000.
4. **libsodium в `game_utils`** — по символам не видно применения (вероятно, целостность/подпись
   данных детекта), не доказано.
5. **Запуск `gpu_encoder_helper`** — строка в voice.node есть, JS-склейки нет; фактическая точка
   `spawn` не найдена.
6. **Выбор `.kef`-моделей Krisp** — имена моделей в бинаре отсутствуют (0 совпадений); логика выбора
   не восстановлена.
7. **`notificationstate.node`** — на Linux заглушка (0 dynsym, 14 CRT-символов); реальный код только в Windows-сборке.
8. **`updater.node`** — stripped + wasm2c: без трассировки рантайма (запрещена) связь строк и функций
   не восстанавливается.
9. **Аппаратное декодирование** (`libffmpeg.so`) — из ускорителей присутствуют только строки
   `d3d11va`/`vaapi`/`vulkan` (перечисление), `nvenc`/`nvcuvid`/`nvdec` нет; путь не подтверждён.
10. **Расхождение с README репозитория.** README утверждает «9–20 динамических экспортов» и «скрытые
    символы voice/krisp». Измерено: voice — 11 726 определённых dynsym-записей (11 367 GLOBAL DEFAULT
    функций), krisp — 28 150 (25 825). Наблюдение README верно только для `utils`/`erlpack`/`game_utils`
    (там ровно 11) и для `zstd` (1). Для voice/krisp символы не «скрыты» — они как раз выставлены наружу,
    но по большей части принадлежат влинкованным сторонним библиотекам.

---

## 7. Артефакты

| файл | что внутри |
|---|---|
| `/home/sanya/re-deep/native-modules.json` | результат: 14 модулей (`name/path/size/sha256/symtab/dynsym/stripped/notable/js_api/children`), блок `runtime` (2 снимка, факты, оговорки), 10 `gaps`, поле `metric` с описанием метрики |
| `/home/sanya/re-deep/native-stats.json` | «сырая» таблица измерений (размеры, sha256, счётчики символов, memsz) |
| `/home/sanya/re-deep/build-json.py`, `final-stats.py`, `verify-syms.py`, `notable.py`, `families.py`, `runtime.py`, `dynsym.py`, `asar*.py` | воспроизводимые скрипты анализа (можно перезапустить) |
| `/home/sanya/re-deep/nm/*.defined.C.txt`, `*.dyn.C.txt` | деманглированные таблицы символов по каждому модулю |
| `/home/sanya/re-deep/core-asar/`, `app-asar/` | распакованные asar (копии для анализа загрузки, оригиналы не изменялись) |
