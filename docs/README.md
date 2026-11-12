# Chord — Documentation Index

Welcome to the comprehensive documentation for **Chord**.

## 1. Quick Navigation

| Document | Link | Purpose |
| :--- | :--- | :--- |
| **Main Overview** | [`README.md`](../README.md) | Architecture, benchmarks, features, quick start |
| **User Guide** | [`USER-GUIDE.md`](USER-GUIDE.md) | Features, shortcuts, keyring troubleshooting, media cache, integrations |
| **Architecture Reference** | [`ARCHITECTURE.md`](ARCHITECTURE.md) | Crate inventory, REST v9, 64-bit permissions, SQLite WAL, UI catalog, i18n |
| **AI Collaboration Guide** | [`AI-AGENTS.md`](../AI-AGENTS.md) | AMCP v1.0 protocol, multi-agent rules, review gates |
| **Contributing Guide** | [`CONTRIBUTING.md`](../CONTRIBUTING.md) | Clean-Room rules, TDD, code style, PRs |
| **Developer Alpha Limitations** | [`DEVELOPER_ALPHA_LIMITATIONS.md`](DEVELOPER_ALPHA_LIMITATIONS.md) | Known gaps and roadmap in Developer Alpha Preview |
| **License** | [`LICENSE`](../LICENSE) | GNU General Public License v3.0 |
| **Beta Testing Guide** | [`BETA-TESTING-GUIDE.md`](BETA-TESTING-GUIDE.md) | How to test the v1.0.0-beta.1 native release |
| **System Requirements** | [`SYSTEM-REQUIREMENTS.md`](SYSTEM-REQUIREMENTS.md) | Linux libraries, Vulkan, PipeWire, llvmpipe |
| **Project Registry** | [`PROJECT-REGISTRY.md`](PROJECT-REGISTRY.md) | Workspace crate inventory, contracts, status |
| **Release Notes** | [`../RELEASE-NOTES-v1.0.0-beta.1.md`](../RELEASE-NOTES-v1.0.0-beta.1.md) | v1.0.0-beta.1 release summary |
| **Feature Backlog** | [`feature-backlog.md`](feature-backlog.md) | Discord feature parity checklist |
| **Vesktop Analysis** | [`vesktop-analysis.md`](vesktop-analysis.md) | Architecture comparison with Vesktop/Vencord |
| **Discord-Qt Analysis**| [`discord-qt-analysis.md`](discord-qt-analysis.md) | Architecture comparison with Discord-Qt |

---

## 2. Workspace Crates Overview

```
rust/
├── chord-gpui/         # Native GPU desktop application (GPUI/WGPU 120 FPS)
└── crates/
    ├── chord-app/          # Application bootstrap, lifecycle & integration wiring
    ├── chord-core/         # REST API v9, Gateway v9, Forums, Threads, Polls
    ├── chord-voice/        # WebRTC, RTP/RTCP, DAVE MLS v1 E2EE, Soundboard, HRTF 360°
    ├── chord-media/        # Zero-copy audio buffers, two-tier LRU media cache
    ├── chord-platform/     # XDG autostart, system tray, D-Bus, FFI Keyring, Spotify MPRIS
    ├── chord-store/        # SQLite FTS5 full-text search, WAL mode, encrypted store
    ├── chord-codec/        # ETF binary codec (zip-bomb safe) & zstd streams
    ├── chord-i18n/         # Internationalization: 36 locales, xxh64 hashed keys
    ├── chord-ui/           # Shared GPUI UI primitives & component library
    ├── chord-json/         # Zero-copy / strict JSON helpers
    ├── chord-perf/         # Frame budget & memory benchmarking utilities
    ├── chord-updater/      # Auto-update checker & release verification
    └── chord-conformance/  # Protocol conformance tests & clean-room oracle checks
```

### Key Technical Specs:
- **Renderer**: WGPU (Vulkan, Metal, DX12, software fallback to Mesa `llvmpipe`).
- **Memory Footprint**: `11.26 MB` idle VmRSS, hard budget `< 60 MB`.
- **Framerate**: Steady **120 FPS** with sub-2µs render times on GPU.
- **Audio & Voice**: WebRTC + Opus (48 kHz stereo, 20ms frames), DAVE MLS v1 (`RFC 9420`) End-to-End Encryption.
- **Screen Sharing**: PipeWire Portal (`org.freedesktop.portal.ScreenCast`) with DMA-BUF zero-copy hardware acceleration.
- **Desktop Integration**: D-Bus StatusNotifierItem system tray, AT-SPI accessibility (`org.a11y.Bus`), Linux Keyring secret storage.

---

## 3. Clean-Room Reverse Engineering & Research

Research notes and protocol specifications are documented in [`docs/re/`](re/):
- [`docs/re/gateway-protocol.md`](re/gateway-protocol.md): Discord WebSocket Gateway Opcode 0..14 specifications.
- [`docs/re/voice-deep.md`](re/voice-deep.md): Voice WebRTC signaling, UDP transport, keepalive format, and DAVE MLS frames.
- [`docs/re/rest-d02.md`](re/rest-d02.md): Discord REST API v9 route catalog, rate-limiting headers, and error structures.
- [`docs/re/i18n-deep.md`](re/i18n-deep.md): Localization key hashing algorithms (`xxh64 -> base64[0..6]`).

---

## 4. Testing & Verification Hierarchy

This repository follows **Strict TDD** with a **5/5 Mutation Gate**:
1. **T0 (Targeted)**: `node scripts/test-impact.mjs --run` (< 1 second).
2. **T1 (Worker Gate)**: `cargo clippy`, `npm run typecheck`, 147 meta-tests, 920+ unit tests.
3. **T2 (Integration & E2E)**: Headless GPUI smoke tests, 50 E2E user journeys, SSIM > 0.995 perceptual visual regression.
4. **T3 (Release Qualification)**: 5/5 mutations killed on every slice, ASan/TSan memory checks, SPDX SBOM.
5. **T4 (Live Qualification)**: Real D-Bus Keyring, PipeWire stream validation, and network jitter resilience.
