# Changelog

All notable changes to **Chord** are documented in this file.
The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

---

## [v1.0.0-beta.1] - 2026-11-11

### Added
- **Native GPU Client (`discord-gpui` / `chord-gpui`)**: Complete architectural transition to a 100% native Rust desktop application powered by GPUI and WGPU for hardware-accelerated 120 FPS rendering.
- **DAVE MLS v1 E2EE Voice Encryption (`RFC 9420`)**: Full cryptographic parity with Discord Voice End-to-End Encryption, including `DaveHashRatchet`, `Aes128GcmTruncated8`, epoch rotation, and 30-digit Voice Privacy Verification Codes.
- **PipeWire Screen Sharing & Wayland Support**: Native DMA-BUF NV12 zero-copy screen and window streaming via `org.freedesktop.portal.ScreenCast` with automatic X11 MIT-SHM fallback.
- **Interactive Spatial Audio Radar (`SpatialRadar`)**: 360-degree binaural KEMAR HRTF 512-point FFT sound positioning with 2D avatar dragging and SQLite coordinate persistence.
- **Native Spellchecking**: Real-time spellchecking through direct FFI to `libenchant-2` / `hunspell` with zero memory leaks.
- **Steam Deck & Handheld Support**: Full gamepad navigation via `gilrs` and SteamOS On-Screen Keyboard trigger.
- **Spotify MPRIS Synchronization**: Real-time D-Bus integration for "Listening to Spotify" status (Opcode 3) and "Listen Along" session synchronization.
- **Custom Emojis & Stickers Picker**: 380x420px picker modal with search, frequency ranking, and two-tier LRU memory + disk cache.
- **Soundboard Engine**: Voice Opcode 18 protocol support, Opus sound cache, and microphone linear mixer with soft `tanh()` saturation.
- **Forums & Threads Lifecycle**: Full support for Channel Type 15 (Forums) with 2-column card layout and Types 11/12 nested Threads with a 360px sidebar.
- **Linux Packaging**: Production packaging specifications for AppImage, Debian (`.deb`), RPM (Fedora/RHEL/openSUSE), Arch Linux (`PKGBUILD`), Flatpak, and Snapcraft.

### Changed
- Rebranded project identity to **Chord** with custom harmonic squircle branding and clear open-source non-affiliation notices.
- Memory consumption reduced by ~90% (from 260–390 MB WebView footprint to **11.26–18.00 MB VmRSS**).
- Cold startup time reduced from 4.5s to **< 480 ms** via SQLite WAL cache hydration.

### Security
- Linux Keyring credential encryption (`libsecret` / `org.freedesktop.secrets`).
- Decompression bomb and recursion depth protection in binary ETF codec.
- 5/5 mutation gate verification across all code changes.

---

## [v0.5.0] - 2026-10-31

### Added
- **Core Rust Protocol Workspace**: Crate boundaries for `discord-core`, `discord-codec`, `discord-voice`, `discord-media`, `discord-store`, and `discord-platform`.
- **WebSocket Gateway v9**: State machine supporting `HELLO`, `Heartbeat`, `IDENTIFY`, `RESUME`, and `VOICE_STATE_UPDATE`.
- **Binary ETF Parser**: Zero-copy Erlang External Term Format decoding with 64-bit Snowflake preservation.
- **REST API v9 Engine**: HTTP/2 connection pooling with token bucket rate limiting and automatic exponential backoff.
- **Research CLI Suite (`bin/chord.mjs` / `bin/discord-lab.mjs`)**: Dynamic process discovery, Frida function hooking, and Chrome DevTools Protocol (CDP) DOM inspection.
- **Tauri / WebKit Desktop Prototype**: Functional hybrid prototype featuring virtualized chat, channel switching, and local SQLite cache.

---

## [v0.1.0] - 2026-10-18

### Added
- Initial project repository and clean-room protocol analysis suite.
- Gateway packet capture fixtures and protocol schema specifications (`docs/re/`).
- Architectural foundation for non-invasive dynamic analysis.
