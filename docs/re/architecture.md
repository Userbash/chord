# Discord Desktop Architecture: Processes, Invocations, and Graphs (R7)

Summary Mermaid diagrams based on Wave R7 research (rendered directly in GitHub markdown). Source materials: `main-arch.md` (main process & call chains), `backend-ipc.md` (channels & bridge), `native-re.md` (native modules), `gateway-protocol.md` (gateway), `voice-deep.md` (voice), `rest-d02.md` (REST). Citations and exact offsets are preserved in the respective documents.

## 1. Processes and Module Placement

```mermaid
flowchart TB
  classDef note fill:#eef2f7,stroke:#7d93ab,color:#1b2430
  classDef nok fill:#fbe3e3,stroke:#c0392b,color:#1b2430

  subgraph MAIN["main process (Electron main: app.asar + core.asar)"]
    MAPP["app.asar: bootstrap, splash, updater, tray, deep links, overlay-host"]
    MCORE["core.asar: windows, IPC handlers (152), native bindings"]
    MNAT["updater.node · discord_utils.node · discord_desktop_core"]
  end

  subgraph REND["renderer — main window (Discord web client)"]
    WEB["web client: discord.com/assets/*.js"]
    PRE["mainScreenPreload.js: native module registry + DiscordNative bridge"]
    RNAT["discord_voice.node 126 MB · discord_krisp.node 62 MB · erlpack · zstd · cld"]
  end

  subgraph OTH["other processes"]
    GPU["Chromium GPU process (iGPU renderD129)"]
    CLIP["child process discord_clips (clip rendering)"]
  end

  TOOL["CDP 9223 instrumentation: capture.mjs, ui-*.mjs, session3-run.sh"]:::note
  NOV["voice API never invoked in main — verified via grep + Frida"]:::nok

  WEB --> PRE
  PRE -->|"IPC: invoke/sendSync, channels DISCORD_*"| MCORE
  MCORE --> MAPP
  MCORE --> MNAT
  PRE -. "require(.node) via module registry" .-> RNAT
  MCORE --> GPU
  MCORE --> CLIP
  RNAT --> NOV
  WEB -. "CDP" .-> TOOL
```

Key invariant: Native voice/media (`discord_voice`, `discord_krisp`, `discord_erlpack`, `discord_zstd`, `cld`) is loaded by the **renderer preload**, not the main process; the main process contains zero voice API calls (`native-re.md` §7.1).

## 2. IPC: Channels, Prefixes, Volumes

```mermaid
flowchart LR
  R["renderer / preload"] -->|"invoke / sendSync — 128 request-response pairs"| M["main: ipcMain.handle (#152 handlers)"]
  M -->|"push — 7 channels (webContents.send / reply / sendToAllWindows)"| R
  R -. "fire-and-forget (on/send) — 40" .-> M
  M --> N["native modules and system APIs"]
```

- Channels are named with the `DISCORD_` prefix: in `core` — from enum `IPCEvents` (176 members, 168 channels), in `app` — short names resolved through runtime wrapper `getDiscordIPCEvent`.
- Functional groups (`backend-ipc.md` §2): windows 25, application 21, miscellaneous 14, processes/metrics 12, files 9, tray 9, clipboard 8, power 8, clips 7, notifications 7, splash 6, spellchecker 6, native modules 5, WebAuthn 5, and others.
- Preload whitelist — 43 channels (`RENDERER_IPC_SEND_WHITELIST` / `RENDERER_IPC_INVOKE_WHITELIST`); 47 call-sites compute channel names dynamically at runtime.

## 3. Five Full Call Chains (renderer → main → native)

| # | Channel | Path | Evidence |
|---|---|---|---|
| A | `DISCORD_NOTIFICATIONS_SEND_NOTIFICATION` | renderer → main `#7592` → `discord_notifications.sendNotification`; callback → `DISCORD_NOTIFICATIONS_RECEIVED_RESPONSE` to all windows | On Linux this module is not loaded: mac/win code paths are bypassed, status `unsupported` |
| B | `DISCORD_NATIVE_MODULES_INSTALL` | preload `ensureModule` → main `#216` → `updater.installModule` + `commitModules` → `addGlobalPath(app‑<ver>/modules/<name>‑<ver>)` → `GET_MODULE_PATH` (sync) → `require(<modulePath>)` | Complete chain with citations in `main-arch.md` §3.3 |
| C | `DISCORD_WINDOW_GET_NATIVE_HANDLE` | main `#3386` → `BrowserWindow.getNativeWindowHandle().readInt32LE().toString(10)` | Response is decimal window ID; Wayland 64-bit may truncate to 32-bit integer |
| D | `DISCORD_RENDER_MONTAGE_RECIPE` | main `#7635` → `spawn(discord_clips --render-recipe …)` → `verifyDiscordMP4`; protocol `disclip://` with HTTP Range (206/416) | Clips: rendered strictly via isolated binary module |
| E | `DISCORD_WEBAUTHN_REGISTER_MAC`, `DISCORD_PROCESS_UTILS_GET_SYSTEM_METRICS` | main `#5544` / `#9984` → macOS WebAuthn; system metrics via `discord_utils` | `main-arch.md` §3.6 |

Chain B sequence diagram (native module installation and loading):

```mermaid
sequenceDiagram
  participant W as renderer (web client)
  participant P as preload (mainScreenPreload.js)
  participant M as main (ipcMain)
  participant U as updater.node
  W->>P: DiscordNative.ensureModule("discord_voice")
  P->>M: invoke DISCORD_NATIVE_MODULES_INSTALL
  M->>U: installModule + commitModules → globalPaths
  M-->>P: resolve
  P->>M: sendSync DISCORD_NATIVE_MODULES_GET_MODULE_PATH
  M-->>P: path <modules>/discord_voice-<ver>
  P->>P: require(<modulePath>) → index.js → .node
```

## 4. `DiscordNative` Bridge (Renderer Side)

- Native module registry — module `7927` in `mainScreenPreload.js`: `requireModule(name)` with whitelist checks (`^discord_[a-z0-9_-]+$` or `erlpack`), `ensureModule(name)` → `DISCORD_NATIVE_MODULES_INSTALL`, `getModulePath(name)` → sync channel.
- Preload list: `PREMAP_MODULES = ["discord_voice","discord_zstd","discord_cloudsync","discord_media"]` (`discord_premap`), where `cloudsync/media/premap` are absent on disk.
- Total exposed bridge entries: 35 properties; 110 active native calls across voice and krisp (`(0,X.lE)().<method>` — 65 call sites, `native-re.md` §7.3).

## 5. Native Modules and Capabilities

| Module | Size | Loaded In | Core Capabilities |
|---|---:|---|---|
| `discord_voice` | 126.1 MB | renderer (preload) | codecs, 7 encryption modes, DAVE MLS v1, RTC worker; dynamic `dlopen`: pipewire, vulkan, va, cuda, nvenc, EGL/GLES |
| `discord_krisp` | 62.1 MB | renderer (preload) | noise cancellation / VAD; ONNX Runtime 1.14 + OpenBLAS (statically linked), models adjacent to module |
| `discord_utils` | 19.2 MB | main | X11/Wayland idle detection, PulseAudio, system metrics, input task `OnSystemInputEvent` |
| `discord_game_utils` | 11.5 MB | renderer | game integrations / RPC helpers |
| `discord_erlpack` | 7.8 MB | renderer | ETF package (erlpack) |
| `cld.node` | 6.6 MB | renderer | Compact Language Detector (CLD) |
| `updater.node` | 5.8 MB | main | Rust updater: `command`, `installModule`, `commitModules`, `create_shortcut` |
| `discord_zstd` | 0.9 MB | renderer | Zstandard decompressor (including gateway stream) |
| `notificationstate` | 16.8 KB | renderer | Linux notification state stub |

## 6. Gateway: Data Flow

```mermaid
flowchart LR
  WS["WS wss://gateway*.discord.gg <br/> ?encoding=etf&v=9&compress=zstd-stream"] --> Z["zstd stream <br/> (magic 28 B5 2F FD on frame 0 only)"]
  Z --> ETF["ETF terms <br/> (11 factual tags)"]
  ETF --> OP{"op dispatcher"}
  OP -->|"op 0"| D["dispatches: READY 4, GUILD_MEMBER_LIST_UPDATE 1048, <br/> MESSAGE_CREATE 31, PRESENCE_UPDATE 107, …"]
  OP -->|"op 10 / op 11"| H["HELLO (heartbeat 41,250 ms) / ACK"]
  H --> HB["op 1 heartbeat every 41.25 s"]
  OP -->|"op 7"| RC["RECONNECT → RESUME (op 6) → RESUMED <br/> sequence s preserved"]
```

- 7 sockets captured: 4 fresh logins + 3 resumes; `op 7` observed once — server closed socket, client cleanly resumed sequence via `RESUMED s=332` with zero loss.
- Outbound frames binary; lengths: IDENTIFY ≈ 943 B, RESUME 173/176 B, heartbeat 90 B.
- READY: 4 instances (~577 KB ETF terms), `resume_gateway_url` pointing to `us-east1-c/d`.

## 7. Voice: Stack and Transport

```mermaid
flowchart TB
  UIP["Voice Panel (renderer, Ctrl+K → channel)"] --> RTC["RTCControlSocket: wss://c-<region>-<hash>.discord.media <br/> ops 8/0/1/2/4/15/16/3/6, heartbeat 13,750 ms"]
  RTC --> MED["Media: UDP to SFU (port from op 2 Ready)"]
  MED --> RTP["RTP: opus PT 120, 20 ms; DTX ~10 packets ≈200 ms, then comfort-noise"]
  MED --> RTCP["SRTCP: SR 60 B / RR 52 B; trailing 4 B LE counter"]
  RTC --> ENG["discord_voice.node: codecs, DAVE/MLS v1, mode aead_aes256_gcm_rtpsize"]
  RTC --> KRS["discord_krisp.node: NC/VAD (ONNX 1.14)"]
  MED -. "exactly +60.08 s" .-> ROT["Socket rotation: socket() → discovery (same ssrc) <br/> → new NAT port → empty RR"]
```

- Discovery 74 B (type 1/2, ssrc, ASCII address, port BE), keepalive 8 B (`1337cafe`/`1337f00d`) every 5 s; media socket owned by renderer process.
- Region selection via `RTCLatencyTestManager` (UDP probes): Session 2 — Frankfurt, Session 3 — Stockholm.

## 8. REST: Request Pipeline

```mermaid
flowchart LR
  A["web client: fetch / API modules"] --> B["Chromium network service"]
  B --> C["https://discord.com/api/v9/<route>"]
  C --> D["response: JSON"]
  H["Auth: Authorization, X-Super-Properties (22 fields, build 641004), <br/> X-Discord-Locale, X-Debug-Options, …"] -.-> B
```

- Live capture covers 47 routes and 133 v9 requests, including message delivery, acknowledgments, settings, search (`/messages/search`), billing, and profile updates.
- Voice gateway maintains separate authentication flow (token issued via op 0 Identify), documented in `rest-d02.md` §6.5.

## 9. Document Cross-Reference Map

| Domain | Specification Document |
|---|---|
| Main process, windows, IPC chains, updater | `main-arch.md` (+ `.json`) |
| IPC channels and bridge | `backend-ipc.md` (+ `.json`) |
| Native modules and exports | `native-re.md` (+ `.json`), `native-modules.md` |
| Gateway: ETF/zstd/opcodes/READY | `gateway-protocol.md`, `gateway-schema.md` |
| Voice: signaling and UDP transport | `voice-deep.md` (+ `.json`) |
| REST: routes, payloads, auth | `rest-d02.md` (+ `.json`, `.stats.json`, `.crossref.json`) |
| UI: screens and component contracts | `frontend-d03.md` (+ `.json`) |
| i18n: runtime, keys, pluralization | `i18n-deep.md` (+ `.json`, `i18n-stats.json`) |
