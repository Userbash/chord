# Chord Project Roadmap

> **Status:** Active Open-Source Strategic Roadmap  
> **Target Release:** `v1.0.0-beta.1` (Current) → `v1.0.0-stable`  

---

## 1. Completed Milestones

### Milestone 1: Clean-Room Protocol Engine & Core Crates — 100% COMPLETE
- [x] Protocol analysis and clean-room specification against independent captures (REST v9, WebSocket gateway, ETF, zstd streaming).
- [x] Crate `discord-codec`: Memory-safe ETF (erlpack) binary codec with recursion protection and zstd streams.
- [x] Crate `discord-core`: Gateway v9 client, REST v9 engine with token-bucket rate limiting, 64-bit permission calculator.
- [x] Crate `discord-voice`: WebRTC signaling, SRTP AES-256-GCM, adaptive jitter buffer (20–60ms), 48 kHz Opus.
- [x] Crate `discord-media`: Lock-free `AudioRingBuffer`, two-tier LRU memory + disk cache with HTTP ETag validation.
- [x] Crate `discord-store`: SQLite in WAL mode with FTS5 full-text message indexing and encrypted storage.
- [x] Crate `discord-platform`: System tray, XDG autostart, Keyring (`libsecret`) credential security, D-Bus notifications.
- [x] Crate `discord-i18n`: 36 localization catalogs, xxh64 translation key hashing.

### Milestone 2: DAVE MLS v1 End-to-End Encryption (`RFC 9420`) — 100% COMPLETE
- [x] Cryptographic key schedule and derivation via RFC 9420 MLS.
- [x] `Aes128GcmTruncated8` truncated 8-byte authentication tag encryption.
- [x] `DaveHashRatchet` forward secrecy ratchet with anti-replay protection.
- [x] Epoch rollout and verification for 30-digit Voice Privacy Codes.

### Milestone 3: Migration to Native Rust GPUI Client (120 FPS) — 100% COMPLETE
- [x] Transition from hybrid WebKit prototype to 100% native Rust desktop application (`discord-gpui` / `chord-gpui`).
- [x] GPU hardware acceleration via WGPU (Vulkan / OpenGL `llvmpipe`) rendering at 120 FPS.
- [x] Ultra-low memory consumption: **11.26 MB VmRSS** (budget < 60 MB, ~100x lower than Electron).
- [x] Rapid startup: Cold launch in **< 480 ms** via SQLite cache hydration.
- [x] Virtualized UI: 100,000+ message scrolling and 25,000+ member lists with zero frame drops.
- [x] PipeWire DMA-BUF zero-copy screen sharing with Wayland desktop portal integration.

### Milestone 4: Quality & Mutation Gates — 100% COMPLETE
- [x] **Strict TDD & 5/5 Mutation Gate**: 100% of injected code mutations killed by tests.
- [x] **Visual Regression Suite**: SSIM > 0.995 verification across 15 reference screens.
- [x] **Headless E2E Harness**: 50 automated user journeys against mock gateway daemon.
- [x] **Chaos Resilience**: 53 automated tests for network partitions and media restarts.

### Milestone 5: Cross-Platform Packaging — 100% COMPLETE
- [x] Linux: AppImage, Debian (`.deb`), RPM (Fedora/RHEL/openSUSE), Arch (`PKGBUILD`), Flatpak, Snapcraft.
- [x] Windows: WiX Toolset MSI installer + Portable `.zip`.
- [x] macOS: DMG bundle with native Vibrancy support.

---

## 2. Current Active Milestone: Beta Stabilization (`v1.0.0-beta.1`)

- [x] Application rebrand to **Chord** with custom harmonic squircle branding.
- [x] Establish open-source non-affiliation notices and trademark disclaimers.
- [x] Clean separation between independent app identity and protocol compatibility layers.
- [ ] Community testing across Wayland environments (GNOME, KDE Plasma 6, Hyprland, Sway).
- [ ] Soak testing on low-spec hardware and handheld devices (Steam Deck, Raspberry Pi 5).

---

## 3. Future Roadmap (`v1.1.0`+)

- [ ] **Wasm Plugin Engine**: Sandboxed extensibility layer for custom widgets and tools.
- [ ] **Custom CSS / Themes API**: User-configurable themes with dynamic token overriding.
- [ ] **Multi-Window Support**: Detachable pop-out voice panels, video grids, and stream windows.
- [ ] **Direct P2P File Sharing**: Encrypted local network file transfers.
