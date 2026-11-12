# Feature Transfer Backlog: Missing Capabilities (References → Project)

Date: 2026-10-19. Purpose: Comprehensive inventory of features that are **not yet in our project**,
synthesized from comparisons against external reference implementations, with descriptions and acceptance criteria.
Each card represents a feature transfer into our product (Scenario A: native messenger inspired by Discord UX). Sources:

- `docs/vesktop-analysis.md` — Vesktop v1.6.7 (Electron shell of official web client; GPL-3.0 — ideas and contracts referenced, no code copied; venmic MPL-2.0, arrpc MIT);
- `docs/discord-qt-analysis.md` — discord-qt v0.5.1 (standalone native client; GPL-3.0 — architectural ideas only);
- `docs/map.md` and clean-room research lab artifacts (READINESS, REPORT, VOICE-CRYPTO, UI-TRACES).

How to read: **ID** — transfer card (`R4-*`, `R5-*`, `R6-*` — feature waves, `D-*` — data contracts);
"Target Layer" — destination layer in our architecture (Scenario A client) or research batch. Task statuses and assigned worker files are maintained in `TASKS.md`; document synchronization is validated by `tests/meta/backlog-sync.test.mjs`. Implementation architecture on Rust is detailed in [`docs/rust-client/`](rust-client/README.md): waves P0–P8 (`PLAN.md`), contract test catalog (`TEST-PLAN.md`), and feature registry (`FEATURES.md`).

| Wave | Domain | Priority | Cards |
| --- | --- | --- | --- |
| R4 | OS Shell: Window, tray, badges, autostart, deep links, notifications, updater, packaging, splash, spellcheck, settings | P0 | 12 |
| R5 | Streaming & Audio: PipeWire capture, node filters, picker, quality profiles, media API overrides, stream events | P1 | 11 |
| R6 | Platform & Extensibility: Steam Deck, D-Bus, platform integrations, debug windows, state reset, asset delivery | P2 | 10 |
| D | Data & Contracts (Clean-Room RE): READY, REST routes, UI traces, media/stream wire contracts, privacy controls | P0 | 8 |

---

## R4 — OS Shell (P0)

12 essential capabilities required for a true native desktop application rather than a web browser tab.

| ID | Feature | Description | Reference Source | Target Layer | Acceptance Criteria |
| --- | --- | --- | --- | --- | --- |
| R4-01 | Window Shell | Frameless window with native titlebar, window geometry save/restore, multi-monitor boundary protection, minimum dimensions 940×500 (default 1280×720), transparency and blur support | Vesktop: `src/main/mainWindow.ts`, `windowsTitleBar.tsx` | Native Client: Shell Layer | Window persists geometry across restarts; respects minimum bounds; remains visible when monitor topologies change |
| R4-02 | System Tray | System tray icon with unread indicator status, context menu (Open, About, Restart, Quit), toggle window visibility on click, start minimized option | Vesktop: `tray.ts` | Shell: System Tray | Clicking tray icon hides/shows main window; unread counter updates icon state; Quit terminates process cleanly |
| R4-03 | Unread Badges | Unread message counter badge: Linux Unity/Freedesktop D-Bus launcher, Windows taskbar overlay icon, macOS dock badge; numeric 1–10, overflow dot indicator, 0 clears | Vesktop: `appBadge.ts`, `dbus.ts` | Shell: OS Integration | Badge visible on desktop launcher/dock/taskbar; 0 clears badge; overflow renders dot; configurable in settings |
| R4-04 | System Autostart (Автозапуск) | Autostart on OS login: Windows/macOS login items, Linux XDG `.desktop`/portal; flag `--start-minimized` | Vesktop: `autoStart.ts` | shell: autostart | Settings toggle reliably manages autostart on all three OSes; starts minimized without window flicker |
| R4-05 | Deep Links & Single Instance | URL scheme handler `discord://` (route URLs to channels, guilds, or voice), second process invocation focuses existing window instead of duplicating | Vesktop: `vesktopProtocol.ts`, `main.ts` | Shell: Deep Link Router | Clicking `discord://...` URL navigates directly to target channel; launching binary twice activates active instance |
| R4-06 | Desktop Notifications | Native OS notifications; clicking notification raises and focuses target channel window; configurable notification sound and taskbar flashing | Vesktop: `fixes.ts`, `taskBarFlash.ts` | Shell: Notification Service | Clicking notification focuses chat view; taskbar flashing adheres to settings; persistent configuration across restarts |
| R4-07 | Application Updater | Startup update verification, download progress display, snooze/skip version options, atomic binary swap, and cryptographic signature validation | Vesktop: `updater.ts` | Updater Service | Updates download with visible progress; snooze/skip flags persist; release binary signatures verified before swap |
| R4-08 | Distribution Packaging | Cross-platform packages: Debian/Ubuntu `.deb`, RPM, AppImage, Flatpak (Linux), WiX MSI (Windows), DMG (macOS); desktop entries, icons, MIME types | Vesktop: `package.json` (builder) | Release Pipeline | Clean installation on Linux, Windows, and macOS; desktop launcher and icons register correctly |
| R4-09 | Store Metadata | Metadata manifests for package distribution repositories: Flathub `metainfo.xml`, Windows Winget manifests, release channels (stable, canary) | Vesktop: `meta.yml`, `winget/` | Release Pipeline | Manifests pass linter validation; release channel selectable at build time |
| R4-10 | Splash Screen & About Dialog | Splash screen displayed during cold boot initialization, first-time user tour, About dialog displaying component version numbers and license info | Vesktop: `splash.ts`, `firstLaunch.ts`, `about.ts` | Shell: Onboarding & About | Splash dismisses smoothly when main window paints; tour state persists; About dialog reflects exact commit and version |
| R4-11 | Spellchecker | Native spellchecker: multi-language dictionary support, inline error highlight, context menu suggestions, add-to-dictionary support | Vesktop: `mainWindow.ts`, `spellCheck.tsx` | UI: Composer Component | Misspelled words highlighted with squiggly underline; replacement and custom dictionary additions function correctly |
| R4-12 | Configuration Store | Persistent user preferences (JSON store, schema validation, default merge, reactive change listeners), comprehensive settings window | Vesktop: `settings.ts`, `components/settings/*` | Core: Settings Engine | Preferences persist across application restarts; settings UI exposes all keys; reset-to-defaults functions reliably |

---

## R5 — Streaming and Audio (P1)

Screen sharing with system audio loopback (PipeWire on Linux, WASAPI on Windows) and quality adaptation.

| ID | Feature | Description | Reference Source | Target Layer | Acceptance Criteria |
| --- | --- | --- | --- | --- | --- |
| R5-01 | Audio Capture (PipeWire) | Capture desktop audio via PipeWire loopback and publish a virtual microphone source (2 channels, 48 kHz, s16le) for WebRTC streaming | Vesktop: `src/main/venmic.ts` (venmic, MPL-2.0) | Capture Service (`pipewire-rs`) | Desktop sound routes into WebRTC voice stream; virtual loopback device cleanly registered |
| R5-02 | Audio Node Filtering | Exclude client's own audio stream, input microphones, and virtual monitoring sinks from capture; capture only application audio to prevent echo | Vesktop: `venmic.ts` (`buildLinkData`) | Capture Service | System audio streaming excludes own microphone input; no acoustic feedback loops; filter rules configurable |
| R5-03 | Stream Audio Configuration | Modal controls: start muted, desktop sound isolation, exclude inputs, granular per-application audio source selection | Vesktop: `ScreenSharePicker` | UI: Voice & Streaming | Options persist and dynamically update audio node routing; helpful tooltips explain routing behaviors |
| R5-04 | Stream Source Selector | Source selection grid with live thumbnails, monitor vs window tabs, resolution presets (480p–1440p) and frame rates (15–60 FPS), audio toggle | Vesktop: `ScreenSharePicker.tsx` | UI: Streaming Modal | Selecting source initiates stream; target resolution, framerate, and audio flags apply to encoder pipeline |
| R5-05 | High-Res Source Thumbnails | On-demand high-resolution thumbnail generation (1920×1080) on source hover or selection while keeping the initial picker list responsive | Vesktop: `CAPTURER_GET_LARGE_THUMBNAIL` | UI: Streaming Modal | Selected thumbnail renders crisp high-res preview; source list remains smooth and responsive |
| R5-06 | Wayland ScreenCast Portal | On Wayland environments with system portal choosers, bypass redundant custom picker and delegate directly to `xdg-desktop-portal` | Vesktop: `screenShare.ts` | Capture Service: Wayland Portal | Stream initiates via native portal dialog without dual picker prompts; full native display resolution preserved |
| R5-07 | Windows Loopback Audio | Capture system desktop audio via WASAPI loopback interface without requiring third-party virtual audio cables | Vesktop: `screenShare.ts` | Capture Service: Windows WASAPI | Audio stream captures system output on Windows 10/11 out-of-the-box; clear diagnostics on legacy OS versions |
| R5-08 | Media Constraints Engine | Custom media constraint negotiation: virtual audio track injection, exact `deviceId` matching, AGC policy, resolution bounds | Vesktop: `screenShareFixes.ts`, `fixStreamConstraints.ts` | Voice Engine | Audio track remains stable through device changes; video encoder adheres strictly to selected resolution bounds |
| R5-09 | Stream Quality Profiles | Dynamic bitrate and encoder scaling: min/target/max bitrate, framerate, resolution pixel limits, connection adaptation | Vesktop: `patchStreamQuality`, `handleGoLive` | Voice Engine | Stream adheres to selected profile (e.g. 720p@60 FPS); bitrate conforms to bandwidth constraints |
| R5-10 | Stream Lifecycle & Streamer Mode | Handle `STREAM_CREATE`, `STREAM_SERVER_UPDATE`, and `STREAM_DELETE` events; automatic streamer mode toggle to hide sensitive personal data | Vesktop: `streamerMode.ts` | Core: Event Dispatcher | Stream lifecycle events update UI state; streamer mode automatically suppresses tokens, invite links, and notifications |
| R5-11 | Stream Viewer & Picture-in-Picture | In-client stream playback viewer with fullscreen toggle, audio volume slider, overlay controls, and automatic PiP floating window | Vesktop: `videoPlayer.ts` | UI: Stream Player | Active streams render with low latency; changing channels detaches player to persistent floating PiP window |

---

## R6 — Platform and Extensibility (P2)

Platform-specific adaptations, system integrations, and debug tools.

| ID | Feature | Description | Reference Source | Target Layer | Acceptance Criteria |
| --- | --- | --- | --- | --- | --- |
| R6-01 | Steam Deck & Controller Mode | Handheld UI mode: dynamic scaling for Steam Deck Game Mode, gamepad navigation support via D-Pad/thumbstick, virtual keyboard triggering via D-Bus | Vesktop: `steamOS.ts` | UI: Platform Navigation | Full navigation possible via gamepad buttons; input fields automatically summon SteamOS on-screen keyboard |
| R6-02 | D-Bus Desktop Integration | Linux D-Bus desktop interface implementation: MPRIS media controls, screen-saver inhibition during calls, power management integration | Vesktop: `dbus.ts`, `mpris.ts` | Platform: Linux D-Bus | Client responds to media key shortcuts; system sleep is prevented during active voice calls or streams |
| R6-03 | Windows Platform (Windows-специфика) | Taskbar badge overlays, portable-режим (isolated data alongside executable), Alt+F4/Ctrl+Q hotkeys | Vesktop: `appBadge.ts`, `constants.ts` | Windows Platform | Portable build does not touch AppData or registry; unread badge visible on taskbar icon |
| R6-04 | macOS Platform (macOS-специфика) | Vibrancy material window, native traffic lights controls, `open-url` deep links, notarization and entitlements | Vesktop: `mainWindow.ts`, `build/` | macOS Platform | Signed and notarized dmg; window with Vibrancy material; links open via custom protocol |
| R6-05 | Diagnostic Inspection Window | Built-in developer diagnostic window: WebRTC stats visualizer, gateway connection logs, frame rendering timeline, memory metrics | discord-qt: `debugWindow.cpp` | UI: Diagnostics | Diagnostic window displays real-time RTT, packet loss, frame time histograms, and memory consumption stats |
| R6-06 | Client Cache & State Reset | Settings action to purge local SQLite message cache, temporary media thumbnails, or reset local preferences without losing credentials | Vesktop: `cacheManager.ts` | Core: Store Engine | Cache purge frees disk space; local settings reset restores default values while keeping login tokens intact |
| R6-07 | Custom Themes Engine | Runtime theme loader supporting CSS variables and token overrides for custom appearance (Dark, Light, Midnight OLED, Solarized) | Vesktop: `themeLoader.ts` | UI: Theme Provider | Custom themes apply immediately without application restart; UI elements maintain WCAG 2.2 contrast compliance |
| R6-08 | Localization & Asset Downloader | Dynamic background downloader for regional font subsets, emoji packs, and audio cues with local caching and SHA-256 validation | Vesktop: `assetManager.ts` | Core: Asset Manager | Localization strings and font assets download asynchronously on first use; cached locally with verified hashes |
| R6-09 | Clipboard Formatting & Rich Pasting | Intelligent clipboard paste handler: detect file drops, image clipboard data, markdown URL wrapping, code block detection | discord-qt: `chatInput.cpp` | UI: Composer Component | Pasting images queues upload in attachment shelf; pasting links over highlighted text generates markdown links |
| R6-10 | Voice Activity & Audio HUD | Visual voice activity detection overlay (green speaking ring, input volume meter, noise gate calibration slider) | discord-qt: `voiceSettings.cpp` | UI: Voice HUD | Speaking indicator triggers with low latency; noise floor slider visually demonstrates gate cutoff threshold |

---

## D — Data and Contracts (P0)

Reverse engineering and protocol specifications required to power all client layers.

| ID | Feature | Description | Reference Source | Target Layer | Acceptance Criteria |
| --- | --- | --- | --- | --- | --- |
| D-01 | Gateway Initial State (`READY`) | Complete schema and capture of initial gateway connection state: user accounts, guild lists, channel structures, member roles | Clean-Room Lab: Gateway Traces | Protocol Core | Full `READY` payload schema parsed into strongly-typed structs; documented in `docs/re/gateway-protocol.md` |
| D-02 | REST API Routes & Payloads | Inventory of REST API v9 endpoints: HTTP method, URL path, request body schema, authentication headers, rate limits | Clean-Room Lab: REST Traces | Network BFF | Route catalog covers authentication, messaging, search, channels, and guilds; documented in `docs/re/rest-d02.md` |
| D-03 | UI Component Interaction Traces | Empirical UI contracts: channel sidebar, chat scroll virtualization, emoji picker, user popouts, settings navigation | Clean-Room Lab: UI Traces | UI Architecture | Component specs define DOM hierarchy, CSS tokens, keyboard bindings, and ARIA roles; documented in `docs/re/frontend-d03.md` |
| D-04 | Media Capture Constraints Protocol | Detailed contract of Discord's media device selection, AGC algorithms, noise suppression flags, and stream renegotiation | Reference Code: Media Constraints | Voice Engine | Complete specification of audio capture parameters, echo cancellation options, and sample rate requirements |
| D-05 | Streaming Protocol Wire Contracts | Resolution parameters, video codec negotiation (H.264, AV1), RTP packetization rules, and voice opcodes `18..=22` | Reference Code: Stream Quality | Media Engine | Full wire contract specification for WebRTC screen share signaling, RTP framing, and keyframe request opcodes |
| D-06 | Notification Defaults Protocol | Client decision heuristics for default notification states (`isPlatformEmbedded`, channel notification settings overrides) | Reference Code: Notification Stores | Notification Engine | Specification of server-level vs channel-level notification cascades, mention sound rules, and mute inheritance |
| D-07 | Session Resilience & Security | Session token protection, secure keyring storage, recovery from abrupt connection dropouts, rate limit backoff | Clean-Room Lab: Auth Traces | Auth & Storage | Session recovery protocol handles Opcode 6 Resume; secure token storage in OS keyring with automatic masking |
| D-08 | Platform Privacy Guardrails | WebRTC IP leakage prevention policy, MediaSession key bindings isolation, auto-play media audio suppression | Reference Code: Privacy Config | Platform Shell | Comprehensive privacy checklist for native client implementation ensuring user IP and telemetry protection |

---

## Out of Scope (What We Do Not Adopt)

- **Regex Web Client Patching:** Webpack chunk rewriting and regex DOM injection are specific to web wrappers; our native Rust architecture has full control over rendering.
- **User-Agent Spoofing:** Unnecessary in a native independent architecture.
- **Electron Fuses / Sandbox:** Specific to the Chromium runtime. Our native client achieves security through OS process isolation, `#![forbid(unsafe_code)]`, and safe memory management.
- **Third-Party Code Copying:** All implementations are developed cleanly from protocol specifications; zero lines of reference code are copied.
- **Proprietary Media Assets:** Discord logos, proprietary sound effects, and default stickers are excluded; the project uses open-source sound assets and custom SVG iconography.
