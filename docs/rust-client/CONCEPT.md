# Concept: Native Discord Client in Rust (without Electron and Chromium)

This document addresses the core design questions: "what this application is, what it does, why each feature is needed, and how it functions according to cleanroom reverse engineering evidence." Fact sources originate from `../re/` (wave R7): Gateway, REST, Voice, UI contracts, i18n, and backend architecture. Full feature catalog — `FEATURES.md`; architecture and dependency graphs — `ARCHITECTURE.md`; implementation plan — `PLAN.md`; test strategy — `TEST-PLAN.md`.

## 1. Goal and Scope

**Goal.** A full-featured desktop Discord client providing user experience and interface parity with the official client: native backend (Rust) in a **Tauri** shell (system WebView instead of bundled Chromium), **without Electron and without bundling a full Chromium browser**.
Implement the cleanroom reconstructed subsystems: event Gateway, REST v9, voice and video, UI surfaces, i18n runtime, and native desktop integrations.

**Scope:**

| In Scope | Out of Scope (for now) |
|---|---|
| Gateway (WS + zstd-stream + ETF), REST v9, session management | Third-party plugins/themes, foreign code modification |
| Chat, messages, search, reactions, threads, member lists | Bots, OAuth apps, third-party game rich presence SDK |
| Voice: signaling, UDP/SRTP, Opus codec, DAVE/MLS | Custom server backend reimplementation |
| Video and screen broadcast (Go Live) | In-game HUD overlay for arbitrary games |
| Custom i18n runtime (36 locales), design tokens, themes, audio | In-app store/Nitro billing (read-only entitlement state) |

**Legal Framework**: Cleanroom reverse engineering and client interoperability comply with fair-use and interoperability standards; usage remains user responsibility. The project bundles no proprietary third-party assets and bypasses no subscription entitlements; telemetry (science/beaker endpoints) is disabled by default.

## 2. What the Application Does

1. **Authentication and Session.** Token stored securely in native operating system keyring; login via email/password with MFA (REST flow), or direct token entry; automatic reconnect and resume without sequence loss.
2. **Guild and Channel Tree.** Guild list, channels (text, voice, threads), unread indicators, folders, direct messages, and group DMs; lazy loading of member lists (tested with 24,229 members in a large guild from READY payload).
3. **Chat Engine.** Paginated history, send/edit/delete, replies, reactions, pins, threads, file attachments (upload/download), emojis/stickers/GIFs, mentions, typing indicators, ack/read-state, polls, guild and channel search with filters and sorting.
4. **Presence and Activities.** User statuses (online, idle, dnd, invisible), rich activities, custom status messages, batched presence updates (handling bursts like PRESENCE_UPDATE x107 per session).
5. **Voice Engine.** Connect/disconnect from voice channels, mute/deafen, Push-to-Talk, audio device selection, noise suppression, speaking detection, RTCP latency and packet loss statistics, DAVE/MLS end-to-end voice encryption (v1), soundboard playback.
6. **Video and Streaming.** Webcam, Go Live (window/screen share), source selection (PipeWire/X11), adaptive bitrate control, SFU video relay.
7. **Design System & UI.** Tokenized design system, Dark/Light/Midnight themes, responsive layouts (member list hides at <1200px, navigation collapses at <720px), popovers, modals, full keyboard navigation and screen reader accessibility (WCAG 2.2).
8. **Internationalization (i18n).** Cleanroom i18n runtime, xxh64 message key hashes, 36 locale catalogs, ICU pluralization, rich Markdown and Discord formatting tags (`<t:...>`, `<@...>`, `<#...>`).
9. **Desktop Integration.** System tray with unread badge, native desktop notifications (D-Bus), global keybindings (PTT/mute), sleep/idle inhibition via logind, autostart configuration.

## 3. Rationale for Each Subsystem

| Subsystem | Why Needed | Consequence if Missing |
|---|---|---|
| **ETF Codec** | Official Gateway transport format; 30-40% smaller payload than JSON; preserves large 64-bit snowflakes without precision truncation | Fallback to JSON increases memory, CPU parsing overhead, and network bandwidth |
| **zstd Stream** | Mandatory compression format for Gateway v9; compresses state payloads by 5-10x | Inability to connect to Gateway or massive memory allocations |
| **Offline Cache** | Instant cold start without waiting for network; offline access to chat history | Blank screen on launch, heavy rate-limiting on app startup |
| **Native Audio Engine** | Low-latency audio (<=20 ms), direct hardware access, low CPU footprint | Audio stutter, high power consumption, microphone desynchronization |
| **Native Keyring** | Secure credentials storage using system Secret Service / libsecret | Plaintext token storage exposes account to credential theft |

## 4. Technology Stack Selection

- **Backend**: Rust 2021 edition.
- **Async Runtime**: `tokio` (multi-threaded work-stealing reactor).
- **WebSockets & Networking**: `tokio-tungstenite` + `rustls` (ring crypto provider).
- **HTTP Client**: `reqwest` / cleanroom HTTP/1.1 implementation.
- **Database & Storage**: `sqlite` (WAL mode, parameterized queries).
- **UI Shell**: **Tauri 2** (lightweight native shell hosting system WebView, zero Electron overhead).
- **Audio & Media**: `cpal`, `opus`, `webrtc`, PipeWire DMA-BUF.
- **Security**: OS Keyring (`libsecret` on Linux, DPAPI on Windows, Keychain on macOS).

## 5. Architectural Model Summary

- **Single Application Process** (with optional isolated media worker process for codec fault isolation). In place of Electron's 168 IPC channels, internal communication uses `tokio::broadcast` event bus and direct service handles.
- **Core Crates**: `gateway`, `rest`, `state` (reducer), `store` (SQLite), `voice`, `media`, `i18n`, `ui`.
- **Data Flow**: Gateway event -> Dispatcher -> State Reducer -> UI Model -> View Subscription; User Actions -> Command Handler -> REST/Gateway -> Optimistic UI -> Server Reconciliation.

## 6. Verification and Sufficiency of Data

Cleanroom artifacts provide complete operational coverage:
- **Gateway**: Validated ETF binary grammar (11 actual tags, exact big-int representation), zstd dictionary framing, HELLO heartbeat timing, IDENTIFY/RESUME state machine, lossless resume verification.
- **REST**: 47 active v9 routes, headers, super-properties fingerprinting, rate-limiting backoff contracts.
- **Voice**: Full signaling (opcodes 0..16), UDP discovery packets, RTP/SRTP packet headers, comfort-noise, socket rotation timers.
- **UI & i18n**: 12 complete component contracts, design token definitions, 23,836 translated string entries across 36 locales.

## 7. Risk Management

| Risk | Impact | Mitigation Strategy |
|---|---|---|
| Anti-abuse client fingerprinting | High | Configurable client properties; strict RFC-compliant headers; regression testing |
| Encrypted RTCP payload formats | Medium | Observationally reconstructed stats contracts; test oracles against real traces |
| DAVE MLS v1 cryptography | Medium | Modular MLS crate architecture (`openmls` integration); automated test vectors |
| High member count UI rendering | Medium | Virtualized list rendering; benchmarked with 25,000+ members in CI |
| Proprietary asset licensing | High | 100% cleanroom code; custom SVG icons and open-source system fonts |

## 8. Success Criteria

| Milestone | Target Criteria |
|---|---|
| MVP (P3) | Login, guild/channel navigation, basic messaging, search, tokenized UI |
| Voice (P4) | Peer-to-peer and guild voice, Opus encoding/decoding, low latency, mute/PTT |
| Complete (P7) | Video/screen share, full 36-locale support, system integration, auto-updater |
| Quality Standard | >=80% core test coverage, 100% mutation kill rate, zero memory leaks |
