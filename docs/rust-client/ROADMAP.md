# Rust Client Development Roadmap (discord-lab)

> **Status:** `Active Strategic Roadmap (v2.1)`  
> **Last Updated:** November 12, 2026  

---

## 1. Completed Milestones

### Milestone 1: Protocol Reverse Engineering & Core Crates (RW-01..RW-35) — 100% DONE
* [x] Protocol analysis and cleanroom specification against `clean-room-oracle/` (REST v9, WebSocket gateway, ETF, zstd streaming).
* [x] Crate `discord-codec`: Hardened ETF encoder/decoder with vulnerability protections, zstd decompressor.
* [x] Crate `discord-core`: Discord REST API v9 client, Gateway WebSocket client, state reducer, permission engine.
* [x] Crate `discord-media`: Lock-free audio ring buffer `AudioRingBuffer`, video pipeline.
* [x] Crate `discord-voice`: Voice UDP streamer, IP discovery, RTP/RTCP packet handling, SRTP AES-256-GCM.
* [x] Crate `discord-store`: SQLite FTS5 full-text search for messages, WAL mode.
* [x] Crate `discord-platform`: System tray, D-Bus notifications, Keyring libsecret FFI credential store.
* [x] Crate `discord-i18n`: 36 localization catalogs, xxh64 translation key hashing.

### Milestone 2: DAVE E2EE MLS v1 End-to-End Encryption (RW-40..RW-41) — 100% DONE
* [x] RFC 9420 KDF and MLS Key Schedule implementation.
* [x] `Aes128GcmTruncated8` integration with truncated 8-byte authentication tag.
* [x] `DaveHashRatchet` implementation with anti-replay protection.
* [x] Two-phase epoch transition in `DaveVoiceSession`.

### Milestone 3: Migration to Native Rust GPUI Client (RW-42..RW-43) — 100% DONE
* [x] Complete decommission of legacy WebKit/Tauri prototype.
* [x] Pure native application `rust/discord-gpui` built on `gpui-kit` (WGPU, 120 FPS).
* [x] Full graphical user interface implementation:
  * Server Rail (72px) with hover states and activity indicators.
  * Virtualized chat (100k messages at RAM < 60 MB).
  * Settings (8-tab `SettingsDialog`).
  * Screen streaming `ScreenShareModal` (PipeWire Portal, 720p/1080p, 30/60 FPS).
  * Authentication (Email, password, token, 2FA, QR code, hCaptcha bridge).
* [x] Release binary build `rust/dist/discord-gpui` — **42 MB** (stripped ELF, Thin LTO, opt-level 3).
* [x] Headless Smoke test: VmRSS **11.26 MB**, frame tick < 8.33 ms.

### Milestone 4: QA Automation & Hardening (RW-44) — 100% DONE
* [x] **`TASK-904`** — GPUI visual regression based on SSIM (>0.99) and dynamic JSON masks (10/10 passed, 5/5 mutations killed).
* [x] **`TASK-905`** — Chaos Engineering bench: network partitions, 20–60ms jitter, PipeWire restart (53/53 passed, 5/5 mutations killed).
* [x] **`TASK-908`** — Manual QA Suite protocol and test scenarios (56/56 passed).
* [x] **`TASK-902`** — Semantic AT-SPI D-Bus GUI driver and WCAG 2.2 AAA accessibility audit (34/34 passed, 5/5 mutations killed).
* [x] **`TASK-901`** — Native Headless E2E Harness (Virtual Wayland + Mock Gateway Daemon, 50 journeys).
* [x] **`TASK-903`** — Network protocol and DAVE MLS v1 fuzzing (libFuzzer + ASan, 5 targets).
* [x] **`TASK-906`** — 120 FPS performance benchmarking and jemalloc memory allocation profiling.
* [x] **`TASK-907`** — Automated code quality gate (Clippy `-D warnings`, ASan/TSan, 5/5 mutations killed).

### Milestone 5: Distribution & Packaging for Linux, Windows, macOS (RW-45..RW-46) — 100% DONE
* [x] Flatpak manifest (`com.discord_lab.DiscordGpui`).
* [x] Standalone AppImage with bundled system dependencies.
* [x] Packages for Arch Linux (PKGBUILD), Debian/Ubuntu (`.deb`), RedHat (`.rpm`), Snapcraft.
* [x] Windows WiX Toolset MSI installer + Portable `.zip`.
* [x] macOS DMG bundle with Vibrancy effect support.

---

## 2. Current Active Milestone: Wave RW-47 (Sprints 1–5 Parity & Polishing)

Goal: Achieve maximum feature parity with official Discord desktop client (>92% coverage).

* [x] **Sprint 1 (Channels & Threads)**:
  * Forum channels (type 15) and 2-column card grid `ForumGridView` (`TASK-1001A`, `TASK-1001B`).
  * Thread lifecycle and 360px `ThreadSidebar` (`TASK-1002A`, `TASK-1002B`).
* [x] **Sprint 2 (Voice Messages & Polls)**:
  * OGG Opus 32 kbps recorder and 256-byte waveform (`TASK-1003A`).
  * Interactive `VoiceMessagePlayer` with playback speed toggles (`TASK-1003B`).
  * Poll engine, voting, and percentage calculation (`TASK-1004A`).
  * `PollCardView` card with winner badge (`TASK-1004B`).
* [x] **Sprint 3 (Media, SFX & Spellchecking)**:
  * Two-tier emoji cache (256 MB LRU + ETag) and 380x420px picker (`TASK-1005A`, `TASK-1005B`).
  * Soundboard: Voice Opcode 18, Opus cache, and linear mixer with `tanh()` soft-limiting (`TASK-1012A`, `TASK-1012B`, `TASK-1012C`).
  * Native spellchecking: FFI to libenchant/hunspell, wavy underline `#f23f43` (`TASK-1006A`, `TASK-1006B`).
* [x] **Sprint 4 (Guild Administration & Platform)**:
  * 64-bit permission and guild role editor (`TASK-1007`).
  * Server audit log viewer with query filters (`TASK-1008`).
  * TOTP 2FA dialog and active sessions manager (`TASK-1009`).
  * Voice AFK monitor and automatic idle channel transfer (`TASK-1010`).
  * Spotify D-Bus MPRIS integration and Listen Along feature (`TASK-1013`).
  * Steam Deck gamepad support and SteamOS on-screen keyboard (`TASK-1014`).
  * XDG autostart generator and quiet tray startup `--start-minimized` (`TASK-1015`).
* [x] **Sprint 5 (Spatial Audio & Real-Time Diagnostics)**:
  * 360-degree binaural HRTF filter using KEMAR dataset (`TASK-1017A`).
  * WGPU 2D participant radar for Spatial Audio (`TASK-1017B`).
  * RTCP metrics collector with RingBuffer 120 (`TASK-1011A`).
  * Vector 60 FPS WebRTC diagnostics screen (`TASK-1011B`).

---

## 3. Future Roadmap

### Milestone 7: v1.0.0 Production Stable Release
* Final pre-release build `BUILD-005` in branch `testing` -> `beta` -> `main`.
* Qualification gate (`scripts/gate.mjs --scope qualify`).
* Cryptographic signing of release binary with GPG key and publication on GitHub Releases.

### Milestone 8: Post-v1.0.0 Capabilities
* Custom theme engine via GPUI styling tokens.
* High-definition video calls (4K 60 FPS with hardware NVENC/VA-API acceleration).
* Plugin system for sandboxed WebAssembly (WASM) extensions.
