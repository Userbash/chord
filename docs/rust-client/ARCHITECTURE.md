# Rust Client Architecture: Processes, Crates, and Data Flows

This document details the target architecture for the native Discord client: process isolation model, crate decomposition, event-driven task execution, Mermaid topology graphs, offline persistence, replacement of Electron IPC channels with native Rust subsystems, UI toolkit Architectural Decision Records (ADR), and performance budgets. Fact sources: `../re/`.

## 1. Target Application Process Model

- The target architecture employs a **single primary process** (UI + Core). Optionally, a dedicated **media worker process** provides crash isolation for native audio/video codecs and hardware encoders.
- Electron's legacy 168 IPC channels (`../re/backend-ipc.md`) are replaced by an **in-process async bus** (`tokio::broadcast` for global events, `tokio::sync::mpsc` for targeted commands) and modular services within a unified memory space.
- Reactive event pipeline: Remote Server -> Gateway Socket -> Event Dispatcher -> **State Reducer** -> Subscriptions (UI Model, SQLite Cache, Sound/Notification Engine). UI User Actions -> REST Client / Gateway -> **Optimistic State Mutation** -> Server Reconciliation.

```mermaid
flowchart LR
    subgraph RP["Primary Process"]
        UI["discord-ui (Tauri, WebView shell)"]
        CORE["discord-core<br/>Gateway + REST + State"]
        STORE["discord-store (SQLite)"]
        I18N["discord-i18n"]
        PLAT["discord-platform<br/>Tray, Notifications, Shortcuts"]
        UI <-->|"events / commands"| CORE
        CORE <--> STORE
        UI --- I18N
        CORE --> PLAT
    end
    subgraph MP["Media Process (Optional)"]
        VOICE["discord-voice<br/>Opus + SRTP + DAVE"]
        MEDIA["discord-media<br/>Video, Capture, Encoder"]
        VOICE <-->|"IPC (packets/frames)"| MEDIA
    end
    CORE <-->|"IPC: control & telemetry"| VOICE
    GW[("Discord Gateway<br/>wss + zstd + ETF")]
    API[("REST /api/v9")]
    RTC[("RTC WS + UDP<br/>Voice/Video SFU")]
    CORE <-->|"WS"| GW
    CORE <-->|"HTTPS"| API
    VOICE <-->|"WS + UDP"| RTC
```

## 2. Workspace Crates and Target Architecture

The following diagram illustrates the direct Cargo dependencies verified against workspace manifests:

```mermaid
flowchart TD
    APP["discord-app (binary)<br/>depends on core"]
    UI["discord-ui<br/>views, components, tokens"]
    CORE["discord-core<br/>gateway, REST, reducer, actions"]
    ENC["discord-codec<br/>ETF, zstd-stream, REST types"]
    STORE["discord-store<br/>SQLite, migrations, cache"]
    VOICE["discord-voice<br/>signaling, UDP, SRTP, Opus"]
    MEDIA["discord-media<br/>video capture, encoders"]
    PLAT["discord-platform<br/>tray, notifications, deep links, idle"]
    I18N["discord-i18n<br/>runtime, message hashes, pluralization"]
    CONF["discord-conformance (dev)<br/>replay cleanroom fixtures"]
    APP --> CORE
    CORE --> ENC
    VOICE --> JSON["discord-json"]
    I18N --> JSON
    UPDATER["discord-updater"] --> JSON
    PERF["discord-perf"] --> CORE
    PERF --> JSON
```

### Crate Responsibilities

| Crate | Responsibility | Key Dependencies |
|---|---|---|
| `discord-codec` | ETF encoder/decoder, zstd-stream frame reader, REST v9 serializable payload types | `ruzstd`, `serde` |
| `discord-core` | Gateway state machine, REST v9 client, state reducer, action dispatch, sync | `tokio`, `tokio-tungstenite`, `rustls` |
| `discord-store` | SQLite offline persistence, schema migrations, query cache | `sqlite` |
| `discord-voice` | WebRTC voice signaling (opcodes 0..16), UDP socket, SRTP encryption, Opus codec, DAVE/MLS | `tokio`, `aes-gcm`, `opus` |
| `discord-media` | PipeWire/X11 screen capture, NVENC/VA-API hardware video encoding, video player | `ashpd`, `libspa` |
| `discord-platform` | System tray, desktop notifications (D-Bus), global keybindings, idle detection | `zbus`, `libsecret` |
| `discord-i18n` | Custom i18n runtime, xxh64 string hashing, ICU plural rules | `xxhash-rust` |
| `discord-ui` | Tauri frontend assets, design tokens, virtualized message lists, popovers | Web standard DOM |
| `discord-app` | Entry point, CLI argument parsing, runtime supervisor, configuration loader | `tokio` |

## 3. Gateway Event Pipeline

```mermaid
sequenceDiagram
    participant S as Discord Gateway
    participant T as Transport (tokio-tungstenite)
    participant Z as ZstdStream (ruzstd)
    participant E as EtfDecoder
    participant D as Dispatcher
    participant R as State Reducer
    participant U as UI Event Stream

    S->>T: Compressed WS Binary Frame
    T->>Z: Push chunk bytes
    Z->>E: Decompressed ETF stream
    E->>D: Parse EtfValue -> GatewayEvent
    D->>R: Dispatch Event (e.g. MESSAGE_CREATE)
    R->>U: Publish StateDelta to subscribers
```

## 4. Voice Subsystem Architecture

```mermaid
flowchart TD
    subgraph Signaling["Voice Signaling (WS)"]
        VGW["Voice Gateway Client"]
        VHELLO["Opcode 8: HELLO"]
        VID["Opcode 0: IDENTIFY"]
        VRDY["Opcode 2: READY"]
        VSEL["Opcode 1: SELECT_PROTOCOL"]
        VSSRC["Opcode 5: SPEAKING"]
    end
    subgraph Media["Voice Media (UDP)"]
        UDP["Async UdpSocket"]
        DISC["IP Discovery (74 bytes)"]
        RTP["RTP Header (RFC 3550)"]
        SRTP["SRTP / AES-256-GCM Cryptor"]
        OPUS["Opus Audio Codec (48 kHz, 20 ms)"]
        DAVE["DAVE / MLS E2EE Layer (v1)"]
    end
    VGW --> VHELLO --> VID --> VRDY --> VSEL --> VSSRC
    VRDY -.-> DISC
    DISC --> VSEL
    VSEL --> UDP
    UDP <--> SRTP <--> DAVE <--> OPUS
```

## 5. Replacement of Legacy Electron IPC Channels

| Legacy Electron Channel | Original Function | Native Rust Replacement |
|---|---|---|
| `DISCORD_NATIVE_MODULES_*` | dlopen native libraries | In-tree Rust crates (media, voice, platform) |
| `DISCORD_PROCESS_UTILS_*` | Process supervision | `tokio::process`, `sysinfo` |
| `DISCORD_POWER_SAVE_*` | Sleep prevention | D-Bus inhibitors (`org.freedesktop.ScreenSaver`, `login1`) |
| `DISCORD_CLIPBOARD_*` | System clipboard | `arboard` |
| `DISCORD_UTILITY_*` | Tray, badge, deep links | Native D-Bus (`zbus`), `xdg-open` |
| `DISCORD_VOICE_ENGINE_*` | Voice audio processing | `discord-voice` crate |
| `DISCORD_KRISP_*` | Noise cancellation | RNNoise / cleanroom ML inference |

## 6. ADR-1: UI Shell Selection — Tauri 2

Tauri 2 was selected as the desktop UI shell: Rust backend services coupled with system WebView (WebKitGTK on Linux, WebView2 on Windows, WKWebView on macOS). No bundled Chromium binary is shipped.

```mermaid
flowchart TD
    S0["P0-01: Tauri Spike Evaluation"] --> Q1{"60 FPS achieved on 100k messages?"}
    Q1 -->|No| ALT["Fallback: Native GUI Toolkit (iced/Slint)"]
    Q1 -->|Yes| Q2{"Design tokens, IME, and 38 settings toggles viable?"}
    Q2 -->|No| ALT
    Q2 -->|Yes| OK["ADR-1: Tauri 2 Accepted"]
```

## 7. Performance Budgets

| Metric | Budget Target |
|---|---|
| Cold start to READY (cache available) | <= 1.5 s |
| Channel open from local cache | <= 100 ms |
| Scroll render time for 25k member list | 60 FPS, <= 16 ms per frame |
| Idle memory consumption (RSS) | <= 300 MB |
| Voice round-trip latency | <= 150 ms (LAN), jitter buffer 40–60 ms |
| Gateway network serialization overhead | <= 37 bytes per message payload |

## 8. Architectural Risks and Mitigation

| Risk | Mitigation |
|---|---|
| WebView virtualization performance limits | Evaluate native Rust GUI toolkits (iced/Slint) if frame drops occur |
| Media process IPC overhead | Maintain unified in-process pipeline; extract to separate worker only on instability |
| DAVE MLS specification evolution | Encapsulate cryptographic envelope behind abstract traits (`openmls`) |
| High memory footprint in massive guilds | Strict member paging limits, LRU channel caching, lazy role resolution |
