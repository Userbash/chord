# Native Modules of Discord Desktop 1.0.162 (Linux x64) — Deep Inventory

**Objective.** Characterize native modules in the Discord 1.0.162 desktop release, identifying their exported interfaces, static symbol footprints, and runtime loading mechanics.

**Analyzed Artifacts (Read-Only):**
* `app-1.0.162/` release bundle (native modules, `updater.node`, `core.asar`, `app.asar`);
* Runtime process inventories from target PIDs;
* Static symbol extracts (`nm -C --defined-only`, `nm -D -C`, `readelf`, `strings`).

**Safety Constraints Observed:** No process execution, no debugger attachment, no network calls, strictly non-invasive inspection. Analysis artifacts stored in `docs/re/`.

**Metric Definition.** Numbers in parentheses beside symbols represent **`unique_symbols`**: the count of distinct symbol entries matching the given identifier token in the demangled defined symbol table (`nm -C --defined-only`).

---

## 1. Module Inventory Table

File sizes represent on-disk byte counts; `symtab/dynsym` count defined symbol table entries; `functions` denotes symbols marked `t/T/w/W`; `GDF` denotes `GLOBAL DEFAULT` functions in dynsym visible to external linkers; `memsz` denotes the sum of `PT_LOAD.memsz` across segments.

| Module | Size (Bytes) | SHA-256 (Prefix) | symtab | Functions | dynsym | GDF | Stripped | memsz |
|---|---:|---|---:|---:|---:|---:|:--:|---:|
| `discord_voice.node` | 126,136,024 | `96721305d59a4fdc` | 85,577 | 69,621 | 11,726 | 11,367 | No | 37,219,949 |
| `discord_krisp.node` | 62,078,048 | `3d70b39670d5cf02` | 43,990 | 37,001 | 28,150 | 25,825 | No | 47,611,273 |
| `libmediapipe.so` | 23,793,696 | `4dc7da21837b507e` | 35,315 | 26,942 | 24,183 | 18,989 | No | 20,072,872 |
| `discord_utils.node` | 19,185,488 | `3dc68ed5675318dd` | 5,212 | 3,599 | 11 | 11 | No | 1,328,293 |
| `discord_game_utils.node` | 11,478,904 | `4fe8b2382aa28dd8` | 4,002 | 2,642 | 11 | 11 | No | 1,073,361 |
| `discord_erlpack.node` | 7,755,832 | `6cad518145244f52` | 2,546 | 1,449 | 11 | 11 | No | 464,973 |
| `cld.node` (spellcheck) | 6,646,552 | `9365d00f027e8cd9` | 482 | 332 | 381 | 312 | No | 6,598,774 |
| `updater.node` | 5,812,960 | `44c55e0bc8271162` | 0 | 0 | 9 | 9 | **Yes** | 5,807,253 |
| `discord_zstd.node` | 873,248 | `bb7cb593f979dd77` | 1,087 | 838 | 1 | 1 | No | 747,477 |
| `gpu_encoder_helper` (CLI) | 1,027,416 | `17c73b9039e417ab` | 0* | 0* | 0 | 0 | Partial* | 783,498 |
| `notificationstate.node` | 16,840 | `9660a012a89479b0` | 14 | 6 | 0 | 0 | No | 1,925 |
| `core.asar` (desktop_core) | 1,762,838 | `27aacaa590e9511b` | — | — | — | — | — | — |
| `app.asar` | 3,621,566 | `6094a8ffdd7740d9` | — | — | — | — | — | — |
| `discord_rpc` | JS only | — | — | — | — | — | — | — |

* `gpu_encoder_helper`: `.symtab` is present on disk, but all defined symbols are obfuscated via LLVM internalization (`anon.<hash>.N.llvm.<id>`). Dynamic symbol table `dynsym` is empty.

Build metadata: `build_info.json` -> version `1.0.162`, releaseChannel `stable`, sentryRelease `discord-desktop-7e0a1fef1fc1295b33b9404e5b77f1ae39d0c6e0`, sentryDist `stable-linux-x64`.

### Intersection Between `symtab` and Dynamic Visibility

Key architectural insight: **Most modules do not expose their public functional API via `dynsym`**. Modules such as `discord_utils`, `discord_game_utils`, and `discord_erlpack` export exactly 11 dynamic symbols — standard C++ allocator operators (`new/delete`), runtime abort stubs, and NAPI registration hooks (`napi_register_module_v1`). All business functionality is bound directly into the returned NAPI object.
In contrast, `discord_voice` and `discord_krisp` leak thousands of symbols into their dynamic symbol tables, primarily originating from statically linked third-party dependencies (WebRTC, OpenSSL, OpenBLAS).

---

## 2. `discord_desktop_core` — ASAR Packaging Architecture

In `modules/`, the package `discord_desktop_core-1/discord_desktop_core/` consists of:

```
index.js   (41 B)   -> module.exports = require('./core.asar');
package.json        -> { "name": "discord_desktop_core", "main": "index.js" }
core.asar  (1,762,838 B)
```

Contents of `core.asar`:

| File | Size (Bytes) | Role |
|---|---:|---|
| `bundle.js` | 402,296 | Main process bundle: window orchestration, tray, bootstrap, module updates |
| `mainScreenPreload.js` | 1,105,621 | Main window preload script: constructs `globalThis.DiscordNative` |
| `data/cacert.pem`, `data/riotgames.pem` | — | Bundled root certificate authorities |

Dependencies: `@sentry/electron` 7.13.0, dev-dependency `electron` 42.7.1.
The main process directly imports only `discord_utils` and `discord_notifications`.

### Module Loading Sequence

1. `updater.node` is co-located with the main application binary (`<execPath>/../updater.node`) and loaded early before `modules/` initialization.
2. `requireNative("<name>")` delegates to standard Node module resolution with augmented `paths`; directories are dynamically appended from `updater.queryCurrentVersionsSync().current_modules`.
3. `core.startup({Constants, GPUSettings, analytics, appSettings, autoStart, buildInfo, crashReporterSetup, logger, moduleUpdater, paths, splashScreen, updater})` serves as the centralized main process initialization gateway.
4. Preload exposes `DiscordNative` into the renderer sandbox via `contextBridge.exposeInMainWorld`, mediating access to `discord_voice` and `discord_krisp`.
5. Dynamic module discovery operates via IPC `NATIVE_MODULES_GET_PATHS`, on-demand installation via `NATIVE_MODULES_INSTALL`. Preloaded modules: `PREMAP_MODULES = ["discord_voice","discord_zstd","discord_cloudsync","discord_media"]`.

---

## 3. Module Capabilities and Representative Symbols

### 3.1 `discord_voice.node` — Media Engine (C++ + Rust + libwebrtc)

Contains 69,621 functions in `symtab`.

| Symbol | Capability Description |
|---|---|
| `discord::media::Connection` (309) | Core voice connection object; exposed to JavaScript as `VoiceEngine.VoiceConnection` |
| `discord::media::Engine` (411) | Engine entrypoint: audio devices, subsystem initialization, state machine |
| `discord::media::VideoSource` (29) | Video capture abstractions (camera, desktop, application window) |
| `discord::media::RemoteUser` (85) | Remote participant state and stream decoders |
| `ScreenshareHelper` (27) | Screen sharing pipeline: simulcast, adaptive bitrates, quality layers |
| `MediaPipeFilter` (56) | Background segmentation filter: MediaPipe graph integration with TFLite models |
| `LinuxCaptureSource` (26) / `PipewireConfig` (32) / `show_portal_picker` (7) | Wayland and PipeWire screen capture using `xdg-desktop-portal` |
| `capture_gamescope` (76) | Direct capture hook for Gamescope sessions on Steam Deck / SteamOS |
| `cros_codecs` (567) | Chromium hardware acceleration: VA-API for H.264, VP8, VP9, AV1 |
| `nvidia_media_encoding` (181) | Rust NVENC hardware encoding pipeline |
| `vulkan_utils` (118) | Vulkan Video backend: `fused_scale_to_nv12.glsl` compute shader |
| `mlspp::hpke` (509) | Messaging Layer Security / HPKE implementation for DAVE E2EE protocol |
| `discord::uv` (1,276) | Rust bindings to libuv event loop and networking primitives |
| `discord::node_api` (1,378) | Rust NAPI layer projecting engine types into JavaScript objects |
| `webrtc::internal` (414) | Statically linked libwebrtc components (audio processing, RTCP, BWE) |

JavaScript interface exposes 28 top-level methods on `VoiceEngine`: `initialize`, `createVoiceConnectionWithOptions`, `createOwnStreamConnectionWithOptions`, `createReplayConnection`, `setVideoOutputSink`, `addDirectVideoOutputSink`, `getNextVideoOutputFrame`, `setKrispPath`, `setMLPath`, `setAudioSubsystem`, `isVaapiEnabled`, `isSpatialAudioEnabled`, `VoiceConnection`, `VoiceReplayConnection`.

### 3.2 `libmediapipe.so` — ML Background Segmentation

35,315 symbols, with 24,183 entries in `dynsym`.

| Symbol | Capability Description |
|---|---|
| `mediapipe::api2::` (528) | MediaPipe C++ calculator graph API |
| `ImageToTensorCalculator` (158) | Frame normalization and tensor transformation pipeline |
| `InferenceCalculatorCpuImpl` (44) | CPU-based inference runtime |
| `InferenceCalculatorGlAdvancedImpl` (69) | GPU-accelerated OpenGL inference runtime |
| `tflite` (7,030) | Embedded TensorFlow Lite interpreter and optimized kernels |

### 3.3 `discord_krisp.node` — Audio Noise Suppression (Krisp SDK 4.0.25)

37,001 functions, with 28,150 entries in `dynsym`.

| Symbol | Capability Description |
|---|---|
| `krispGlobalInit` (6) | Global SDK initialization |
| `krispCreateNcFloat` / `krispCreateNcInt16` (2 each) | Noise suppression instances for float and int16 PCM streams |
| `KrispNCProcess` (2) / `KrispNCSetup` / `KrispNCSetModel` | Real-time audio processing loop and model assignment |
| `KrispVADProcess` / `KrispVADSetup` (2) | Voice Activity Detection (VAD) processing |
| `KRISP::INFER::InferenceEngine` (7) | Model loading and inference controller |
| `OnnxExecutableNetwork` (59) | Embedded ONNX Runtime execution engine |
| `CPUExecutionProvider` (19) | CPU inference provider (GPU acceleration disabled) |
| `_initialize` (8) | JavaScript NAPI initialization interface |

Models are supplied as 5 external `.kef` binaries: `krisp-nc-o-med-v7`, `krisp-nc-o-lite-v1`, `krisp-nc-o-nb-v2`, `krisp-bvc-o-pro-v3`, `krisp-vad-o-v2`.

### 3.4 `discord_utils.node` — OS Integration and Input Monitoring

| Symbol | Capability Description |
|---|---|
| `discord::input::InputEvents` (58) | Global OS input hooks: Push-to-Talk (PTT) and global keybinds |
| `InputEventRegister` / `InputEventUnregister` | Registration and lifecycle tracking for global shortcuts |
| `InputWatchAll` (11) | Raw input event monitoring stream |
| `SetCandidateGamesCallback` / `SetGameDetectionCallback` | Running process game detection hooks |
| `GetPidFromXWindow` / `XGetWindowProperty` | X11 window hierarchy and PID resolution |
| `WaylandIdleManager` (19) | Wayland desktop idle state monitoring |
| `NativePermssion*` (22) | Permission query interfaces (includes literal symbol typo `Permssion`) |
| `GetDiscordMemoryUsage*` (3) / `GetDiscordClips*` (2) | System metrics collection and clips capability check |

### 3.5 `discord_game_utils.node` — Game Detection Subsystem

Exposes `IdentifyGame`, `PlatIdentifyGame`, `FindLaunchable`, `SetRecentGames`, `IsProtocolSchemeRegistered`, `ReadIcon`.
Includes embedded `libsodium` cryptography primitives (`argon2`, `blake2b`, `curve25519`, `poly1305`) used for binary signature and identity validation.

### 3.6 `discord_erlpack.node` — ETF Decompression

Implements `Decoder::decode{Array, Atom, Big, BinaryAsString, Compressed, Export, Float, Integer, List, Map, Nil, SmallAtom, SmallBig, SmallInteger, Tuple}`.
Exposes a single JS entrypoint: `unpack`. Purely a decoder; term serialization is not supported in this module.

### 3.7 `discord_zstd.node` — Zstandard Stream Decompressor

Rust implementation using `napi-rs`. Provides `discord_zstd::bindings::DecompressStream` with methods `decompress`, `get_last_error`, `reset`.
Employs a single active decompressor context per process with session reuse via `reset()`.

### 3.8 `cld.node` (`discord_spellcheck`) — Language Detection

Despite the module name, this component is the Compact Language Detector (CLD2), not a spellchecking engine. Implements `NodeCld::Detect`, `NodeCld::DetectAsync` running over libuv worker threads via `Napi::AsyncWorker`.

### 3.9 `notificationstate.node` — Platform Stub

Contains 14 basic runtime CRT stubs (`_init`, `frame_dummy`). Dynamic symbol table is empty. Under non-Windows environments, the JavaScript wrapper raises `windows-notification-state only works on windows`.

### 3.10 `updater.node` — Module Delivery and Lifecycle Engine

Fully stripped binary (0 symbols in `.symtab`, 9 in `dynsym`). Compiled via WebAssembly bridge (`wasm2c` / `wasm-bindgen`).
Embeds Rust dependencies: `reqwest`, `rusqlite`, `tokio`, `rustls`, `brotli`, `bsdiff`, `zucchini` for delta patching.

### 3.11 `gpu_encoder_helper` — GPU Hardware Probe CLI

Standalone 64-bit ELF PIE binary. Executes vendor detection via dynamic loading:
- `libva.so.2`, `libva-drm.so.2` for VA-API on AMD and Intel hardware;
- `libnvidia-encode.so.1` for NVENC on NVIDIA hardware.
Invoked as a subprocess by `discord_voice.node` to probe GPU capabilities safely without crashing the main media pipeline.

### 3.12 `discord_rpc` — Pure JavaScript Service

Contains no compiled native code. Implements `RPCIPC` and `RPCWebSocket` leveraging the `ws` library.

---

## 4. Runtime Process Allocation

Inspection of running process states shows two distinct native footprints:

**Process Profile A — Updater and Bootstrap:**
- `updater.node`
- `discord_utils.node`

**Process Profile B — Media and Networking Engine:**
- `discord_voice.node`
- `discord_utils.node`
- `discord_erlpack.node`
- `discord_zstd.node`
- `discord_game_utils.node`
- `discord_krisp.node`

Dynamic loading occurs on demand: `cld.node` and optional components are deferred until relevant features are exercised.

---

## 5. Architectural Findings

1. **Dual Build Systems:** C++ modules utilize `napi_register_module_v1` and `node-addon-api`. Rust modules utilize `napi-rs` or `wasm2c`.
2. **Static Vendoring:** Third-party frameworks (libwebrtc, Krisp, MediaPipe, CLD2, libsodium) are linked statically into `.node` binaries rather than consumed as shared objects.
3. **Hardware Acceleration Separation:** Linux video encoding integrates VA-API (`cros_codecs`), NVENC, and Vulkan Video, delegating stability-critical capability checks to `gpu_encoder_helper`.
4. **Wayland Screen Capture:** Screen capture on Linux uses PipeWire and `xdg-desktop-portal` with native Gamescope support for handheld targets.
5. **DAVE Protocol Architecture:** Group call E2EE is implemented natively in Rust via `mlspp::hpke`, with the JavaScript layer handling transport signaling.

---

## 6. Artifact Registry

| File | Description |
|---|---|
| `docs/re/native-modules.json` | Structured inventory: 14 modules, symbol counts, hashes, exported methods |
| `docs/re/native-stats.json` | Raw metric tables (sizes, checksums, symbol counts, memsz) |
| `docs/re/nm/*.defined.C.txt` | Demangled symbol tables for each native module |
| `docs/re/core-asar/`, `app-asar/` | Decompressed application packages used for loading hierarchy verification |
