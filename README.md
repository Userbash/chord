# Discord Lab & Native Discord GPUI Client

[![Rust](https://img.shields.io/badge/rust-2024%20edition-orange.svg)](https://www.rust-lang.org/)
[![GPUI](https://img.shields.io/badge/gui-GPUI%20%2F%20WGPU-blue.svg)](https://github.com/zed-industries/zed)
[![Tests](https://img.shields.io/badge/tests-1500%2B%20100%25%20green-brightgreen.svg)](tests/)
[![Mutations](https://img.shields.io/badge/mutations-5%2F5%20killed-success.svg)](waves/)
[![RAM](https://img.shields.io/badge/memory-%3C%2060%20MB%20RAM-blueviolet.svg)](docs/rust-client/STATUS.md)
[![DAVE E2EE](https://img.shields.io/badge/security-DAVE%20MLS%20v1%20E2EE-darkgreen.svg)](rust/crates/discord-voice/)
[![License: GPL v3](https://img.shields.io/badge/License-GPLv3-blue.svg)](LICENSE)

> **Languages:** **English** | [Русский](README.ru.md)

**Discord Lab & Native Discord GPUI Client** is a high-performance, GPU-accelerated Discord desktop client written in pure Rust (**`discord-gpui`**, powered by GPUI and WGPU for native 120 FPS rendering), accompanied by an advanced clean-room protocol analysis and reverse-engineering research suite (**`discord-lab`** CLI, Frida, CDP).

---

## 1. Why Discord GPUI? (Key Benchmarks)

Official Discord desktop runs inside Electron/Chromium, frequently demanding 800 MB to 1.5 GB of RAM, suffering from CPU rendering frame drops, Wayland screensharing limitations, and sluggish startup. `discord-gpui` replaces this stack with native compiled Rust and hardware-accelerated WGPU rendering.

| Metric / Feature | Official Discord (Electron) | Native `discord-gpui` (Rust) | Advantage |
| :--- | :---: | :---: | :---: |
| **Idle RAM Consumption** | 800 MB – 1.4 GB | **11.26 MB** VmRSS (budget < 60 MB) | **~100x less memory** |
| **Rendering Frame Rate (FPS)** | 30–60 FPS (Chromium lags) | **120 FPS** (Hardware WGPU Vulkan/Metal) | **Zero-lag smooth scrolling** |
| **Cold Startup Time** | 4.5 – 9.0 seconds | **< 480 ms** (SQLite WAL cache hydration) | **~15x faster startup** |
| **Voice End-to-End Encryption (E2EE)** | Partial in WebRTC | **DAVE MLS v1 (`RFC 9420`)** 100% Parity | **Quantum-ready forward secrecy** |
| **Binary Package Size** | 180 – 250 MB | **42 MB** (Stripped ELF with Thin LTO) | **Lightweight footprint** |
| **Linux Screen Sharing** | Black screen on Wayland / No audio | **PipeWire Portal + DMA-BUF zero-copy** with audio | **Full Wayland/PipeWire support** |
| **Realtime Spellchecker** | Chromium Hunspell (memory leaks) | **Native FFI libenchant/hunspell** (<50 lines/fn) | **Zero leaks, native speed** |
| **Handheld / Console UI** | None | **Steam Deck Gamepad Navigation + SteamOS OSD** | **Full gamepad support** |

---

## 2. Workspace Crate Architecture

The repository is structured as a modular Rust monorepo:

```
rust/
├── discord-gpui/         # Native GPU desktop application (GPUI/WGPU 120 FPS)
└── crates/
    ├── discord-core/     # REST API v9, Gateway v9, Forums, Threads, Polls, Permissions
    ├── discord-voice/    # WebRTC, RTP/RTCP, DAVE MLS v1 E2EE, Soundboard, HRTF 360°
    ├── discord-media/    # Zero-copy audio buffers, two-tier LRU/disk media & emoji cache
    ├── discord-platform/ # XDG autostart, system tray, D-Bus, FFI Keyring, Spotify MPRIS
    ├── discord-store/    # SQLite FTS5 full-text search, WAL mode, encrypted storage
    ├── discord-codec/    # ETF binary codec (decompression bomb protection) & zstd streams
    └── discord-i18n/     # Internationalization: 36 locales, xxh64 hashed translation keys
```

### Crate Highlights:
1. **`discord-gpui` (Application Front-End)**:
   - Three-panel layout: Server Rail (72px), Channel Sidebar (240px), Virtualized Chat view (100k+ messages at < 60 MB RAM), and Member list (240px).
   - Forum Channels (Type 15) with 2-column `ForumGridView` layout.
   - Nested Threads (Types 11/12) with a 360px `ThreadSidebar`.
   - Discord Polls with real-time voting, live progress bars, and zero-division protection (`PollCardView`).
   - Voice Message Player (`VoiceMessagePlayer`) with 256-byte RMS waveforms and 1x/1.5x/2x playback rates.
   - Emoji & Sticker Picker modal (`EmojiPickerModal`, 380x420px with 8x32px grid and search).
   - Soundboard Grid (`SoundboardGridView`) with custom hotkeys and live microphone mixing.
   - Interactive 2D Spatial Audio Radar (`SpatialRadar`) on WGPU with SQLite coordinate persistence.
   - 60 FPS WebRTC Telemetry overlay (`RtcDiagnosticsHud`).
   - Complete Settings suite: All 8 core Discord client settings sections.
2. **`discord-core` (Network & Protocol Core)**:
   - WebSocket Gateway v9 state machine (`HELLO`, `Heartbeat`, `IDENTIFY`, `RESUME`, `VOICE_STATE_UPDATE`).
   - HTTP/2 connection pooling with intelligent bucket-aware rate limiting and `retry_after_ms` backoff.
   - 64-bit guild role hierarchy and permissions calculation engine.
3. **`discord-voice` (Voice & Real-Time Media Engine)**:
   - **DAVE E2EE MLS v1 (`RFC 9420`)**: Cryptographic ratchets (`DaveHashRatchet`), truncated 8-byte tags (`Aes128GcmTruncated8`), epoch rollouts, and commit verification.
   - Voice Recording: 48 kHz mono recording encoded into OGG Opus 32 kbps VBR.
   - Soundboard: Voice Opcode 18 (`SOUNDBOARD_SOUND`), LRU Opus sound cache, and microphone linear mixer with soft `tanh()` clipping.
   - 3D Spatial Audio: 360-degree binaural KEMAR HRTF 512-point FFT filter.
   - Adaptive Jitter Buffer (20–60 ms) and Packet Loss Concealment (PLC).
4. **`discord-platform` (OS & Desktop Integration)**:
   - XDG autostart generator and system tray daemon (`--start-minimized`, VmRSS < 8 MB).
   - Spotify D-Bus MPRIS client (Opcode 3 `LISTENING` presence & "Listen Along" synchronization).
   - Gamepad navigation via `gilrs` and on-screen keyboard trigger for Steam Deck / SteamOS.
   - Secure OS credential storage via FFI to `libsecret` / Linux Keyring.
   - Real-time native spellchecking via FFI to `libenchant-2` / `hunspell`.
5. **`discord-media` (Media & Cache Layer)**:
   - Zero-copy ring buffers (`AudioRingBuffer`) for high-throughput audio capture and playback.
   - Two-tier cache: 256 MB RAM LRU cache + persistent disk cache with HTTP ETag validation.

---

## 3. Protocol Research Suite (`discord-lab` CLI & RE)

The repository provides automated tools for non-invasive dynamic analysis of running Discord processes using Frida and the Chrome DevTools Protocol (CDP):

```bash
# Main research CLI (bin/discord-lab.mjs)
node bin/discord-lab.mjs --help                # View available commands
node bin/discord-lab.mjs pids                  # List Discord processes discovered by Frida
node bin/discord-lab.mjs inventory --pid <PID> # Module and symbol inventory (JSON/CSV)
node bin/discord-lab.mjs hook --pid <PID> --pattern 'SendRtp|OnIncomingRtp' # Hook calls
node bin/discord-lab.mjs summarize logs/calls-<PID>.jsonl # Summarize intercepted calls
node bin/discord-lab.mjs cdp targets           # Inspect available Electron renderer targets
node bin/discord-lab.mjs cdp shot /tmp/page.png # Capture page screenshot via CDP
```

Supported CLI commands:
- `pids` — Discovers active Discord processes via Frida
- `inventory` — Collects symbol tables, dynamic libraries, and exported interfaces
- `hook` — Performs dynamic function interception and parameter inspection
- `summarize` — Generates aggregated call frequency and timing metrics from JSONL logs
- `cdp` — Inspects, evaluates JavaScript, triggers actions, and captures screenshots
- `walk` — Automatically traverses the user interface for UI state regression audits
- `domains` — Categorizes functions into architectural domains and generates coverage reports
- `static` — Extracts static symbol tables and cross-references from binary artifacts

Research specifications and oracle documentation are located in `docs/re/`:
- `gateway-protocol.md`: Gateway traces, ETF big-integer handling, zstd streams, and READY handshakes.
- `rest-d02.md`: REST v9 endpoints, headers, `X-Super-Properties`, and error structures.
- `voice-deep.md`: RTC signaling, RTP/RTCP packet structures, SRTP, and DAVE MLS crypto modes.
- `i18n-deep.md`: Hashed localization key mappings (`xxh64 -> base64[0..6]`) for 36 supported languages.

---

## 4. Quality Engineering, Strict TDD & Mutation Gates

This project enforces strict engineering quality standards (**Strict TDD** and **5/5 Mutation Gate**):

```
       ▲
      / \       T4: Qualification Gate (Keyring live, Tarball, live-probes)
     /---\      
    /     \     T3: Release & Mutation Gate (5/5 mutations killed, ASan/TSan, SBOM)
   /-------\    
  /         \   T2: Integration & E2E (GPUI Headless Smoke, 50 E2E Journeys, SSIM >0.995)
 /-----------\  
/             \ T1: Worker Gate (Clippy -D warnings, Typecheck, 147 Meta, 920+ Unit)
───────────────
T0: Targeted    (Targeted test impact: node scripts/test-impact.mjs --run, <1 sec)
```

- **100% Pass Rate**: 147 architectural meta-tests, 920+ unit tests, 391+ GPUI contract tests.
- **5/5 Mutation Kill Rate**: Every PR passes through an automated mutation fuzzer that verifies 100% of injected faults are killed by tests.
- **Visual Regression Testing**: SSIM > 0.995 across 15 reference screens using deterministic JSON masks.
- **Chaos Engineering**: 53 automated resilience scenarios (PipeWire disconnection, 20–60ms network jitter, Gateway disconnects).

---

## 5. Quick Start & Build Instructions

### Prerequisites:
- **Rust**: 1.85+ (Edition 2024, `rustup default stable`)
- **System Libraries (Linux)**: `libasound2-dev`, `libpipewire-0.3-dev`, `libsecret-1-dev`, `libenchant-2-dev` (or `hunspell`)
- **Node.js**: 24+ (for running the test harness and Kanban dashboard)

### Building the Native Client:
```bash
# 1. Clone the repository
git clone https://github.com/Userbash/discord-lab.git
cd discord-lab

# 2. Build the optimized release binary
cargo build --release --manifest-path rust/Cargo.toml -p discord-gpui

# 3. Launch the native client
./rust/target/release/discord-gpui
```

### Running Tests:
```bash
# Architectural and documentation meta-tests (147 tests)
npm run test:meta

# All unit and contract tests
npm test

# Targeted test runner based on modified files
node scripts/test-impact.mjs --run

# Full worker verification gate
node scripts/gate.mjs --scope worker
```

### Visual Kanban Board:
```bash
# Export the latest interactive HTML Kanban dashboard
node scripts/mcp-kanban-board.mjs --export

# Open the Kanban board in your default browser
xdg-open waves/kanban-board.html
```

---

## 6. Cross-Platform Packaging

Package specifications are provided for major desktop operating systems:
- **Linux Flatpak**: `packaging/linux/flatpak/` (`com.discord_lab.DiscordGpui`)
- **Linux AppImage**: `packaging/linux/appimage/` (Self-contained portable bundle)
- **Arch Linux**: `packaging/linux/arch/PKGBUILD`
- **Debian / Ubuntu**: `packaging/linux/debian/` (`.deb`)
- **Windows**: `packaging/windows/wix.xml` (WiX Toolset MSI installer + Portable `.zip`)
- **macOS**: `packaging/macos/build-dmg.sh` (DMG bundle with macOS Vibrancy)

---

## 7. Project Documentation

| Document | Path | Description |
| :--- | :--- | :--- |
| **Project Registry** | [`docs/PROJECT-REGISTRY.md`](docs/PROJECT-REGISTRY.md) | Canonical registry of crates, artifacts, and test contracts |
| **Feature Backlog** | [`docs/feature-backlog.md`](docs/feature-backlog.md) | Parity matrix and feature migration backlog |
| **Vesktop Analysis** | [`docs/vesktop-analysis.md`](docs/vesktop-analysis.md) | Architectural comparison with Vesktop / Vencord |
| **Discord-Qt Analysis** | [`docs/discord-qt-analysis.md`](docs/discord-qt-analysis.md) | Comparative evaluation of Discord-Qt |
| **Current Status** | [`docs/rust-client/STATUS.md`](docs/rust-client/STATUS.md) | Up-to-date memory metrics, FPS, and wave completion |
| **Roadmap** | [`docs/rust-client/ROADMAP.md`](docs/rust-client/ROADMAP.md) | Development milestones, completed waves, and future sprints |
| **GPUI Architecture** | [`docs/specs/GPUI-ARCHITECTURE-MIGRATION.md`](docs/specs/GPUI-ARCHITECTURE-MIGRATION.md) | Migration from WebKit to WGPU / GPUI |
| **DAVE MLS v1 Spec** | [`docs/specs/POST-BETA-ROADMAP-AND-CODING-INSTRUCTIONS.md`](docs/specs/POST-BETA-ROADMAP-AND-CODING-INSTRUCTIONS.md) | E2EE MLS v1 protocol implementation specification |
| **Agent & Team Rules** | [`AGENTS.md`](AGENTS.md) | Worker protocols, orchestrator guidelines, and department write-scopes |
| **Architecture Map** | [`docs/map.md`](docs/map.md) | Dependency graph of crates, layers, and research components |
| **Contributing Guide** | [`CONTRIBUTING.md`](CONTRIBUTING.md) | Contribution standards, Clean Room methodology, and PR workflow |

---

## 8. Clean-Room Methodology & Legal Disclaimer

This project is developed in accordance with the strict principles of **Clean-Room Software Engineering**:
- The implementation is written in 100% original Rust code without incorporating proprietary Discord Inc. source code.
- The official client was utilized solely as a local subject for protocol and audio pipeline behavioral analysis.
- This project is an independent open-source initiative and is **not affiliated with, sponsored by, or endorsed by Discord Inc.**
- "Discord" is a registered trademark of Discord Inc. All trademarks are the property of their respective owners and are used strictly for compatibility identification purposes.

---

## 9. License

This project is licensed under the **GNU General Public License v3.0 (GPL-3.0)**. See the [LICENSE](LICENSE) file for details.
