# Unified Project Registry — discord-lab

> **Document Status:** `Canonical Project Registry (v2.1)`  
> **Updated:** November 12, 2026  
> **Repository:** `chord` (Native client line `discord-gpui` + BFF & Core Rust)  
> **Control Plane:** ECC 2.0 Control Plane (`scripts/ecc`) + Orca Engine  

---

## 1. Project Passport

| Parameter | Value |
| :--- | :--- |
| **Project Name** | `chord` |
| **Primary Product** | High-performance native Linux Discord client in pure Rust GPUI (`discord-gpui`) |
| **Graphics Stack** | `gpui-kit` / WGPU (Vulkan / OpenGL llvmpipe), 120 FPS, native compositing |
| **Language & Runtime** | Rust (Edition 2024 / MSRV 1.85+), Tokio async runtime, Node.js 24 (testing & tooling) |
| **Current Wave** | **Wave RW-47**: Sprints 1–5 Feature Parity, Soundboard, Voice Messages, Polls, Threads & Packaging |
| **Release Artifact** | [`rust/dist/discord-gpui`](rust/dist/discord-gpui) (42 MB, stripped ELF, Thin LTO, opt-level 3) |
| **SHA-256 Checksum** | `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e` |
| **Target Platforms** | Linux (Wayland: GNOME, KDE, Hyprland, Sway; X11 fallback), Windows (WiX MSI/Portable), macOS (DMG) |
| **Task Status** | **220 Completed (Done)** out of 220 kanban board tasks (**100.0%**) |
| **Test Status** | **1500+ tests** (147 meta, 920+ unit, 391+ GPUI contract/e2e/chaos) — 100% Passed (5/5 mutations killed) |

---

## 2. Workspace Crates Registry

| Crate | Path | Subsystems and Core Responsibilities | Test Contracts |
| :--- | :--- | :--- | :--- |
| **`discord-gpui`** | `rust/chord-gpui/` | **Primary native GUI application**: Server Rail, virtualized 100k-message chat, 8 settings tabs, screen share modals, interactive polls (`poll_view`), thread sidebar (`thread_sidebar`), emoji/sticker picker (`emoji_picker`), voice message player (`voice_player`), soundboard grid (`soundboard_view`), 360° spatial audio radar (`spatial_radar`), WebRTC RTCP HUD, AT-SPI accessibility. | `gpui_e2e_contract.rs`<br>`visual_regression_contract.rs`<br>`chaos_engineering_contract.rs`<br>`a11y_atspi_contract.rs`<br>`perf_benchmark_contract.rs`<br>`manual_qa_suite_contract.rs`<br>`release_smoke_contract.rs` |
| **`discord-core`** | `rust/crates/chord-core/` | **Protocol Core**: REST API v9 client, Gateway v9 (HELLO, Heartbeat, IDENTIFY, RESUME, VOICE_STATE_UPDATE), forum channels (`forum.rs`), thread lifecycle (`threads.rs`), poll engine (`polls.rs`), state reducer, 64-bit permission bitmasks. | `permissions_contract.rs`<br>`ready_read_state.rs`<br>`channels_contract.rs`<br>`threads-lifecycle-1002a`<br>`polls-core-1004a` |
| **`discord-voice`** | `rust/crates/chord-voice/` | **Voice & Media Stack**: Voice UDP streamer, IP Discovery (74B), Keepalive (8B), RTP/RTCP packets, SRTP (AES-256-GCM / ChaCha20), DAVE E2EE MLS v1 (`RFC 9420`), OGG Opus voice message recorder (`voice_message_recorder.rs`), Opcode 18 soundboard (`soundboard.rs`) with linear `tanh()` mixer, HRTF 360° spatial audio filter. | `reception_contract.rs`<br>`rtcp_contract.rs`<br>`rtp_video_contract.rs`<br>`soundboard-sfx-1012`<br>`voice-recorder-1003a`<br>`spatial-audio-filter-1017a` |
| **`discord-media`** | `rust/crates/chord-media/` | **Hardware Media Buffers & Cache**: Zero-allocation `AudioRingBuffer`, two-tier emoji and sticker cache (256 MB RAM LRU + disk ETag, `emoji_cache.rs`), X11 and Wayland Portal screen capture connectors. | `buffer_contract.rs`<br>`ring_buffer_contract.rs`<br>`capture_contract.rs`<br>`emoji-cache-1005a` |
| **`discord-platform`** | `rust/crates/chord-platform/` | **Desktop Platform Integrations**: `org.freedesktop.portal.ScreenCast`, D-Bus notifications, system tray, FFI Keyring via libsecret, XDG autostart (`autostart.rs`), Spotify D-Bus MPRIS status (`spotify_mpris.rs`), FFI spellchecking via libenchant/hunspell (`spellcheck.rs`), gilrs gamepad & SteamOS on-screen keyboard. | `screencast_contract.rs`<br>`keyring_contract.rs`<br>`autostart-xdg-1015`<br>`spotify-mpris-presence-1013`<br>`steam-deck-gamepad-1014` |
| **`discord-store`** | `rust/crates/chord-store/` | **Local Data Storage**: SQLite FTS5 full-text search, WAL mode, message and settings cache, safe schema migrations. | `store_contract.rs`<br>`fts5_contract.rs` |
| **`discord-codec`** | `rust/crates/chord-codec/` | **Binary Codecs & Decompression**: ETF binary decoder/encoder, zstd streaming, compression-bomb attack mitigation. | `zstd_bomb_contract.rs`<br>`etf_properties.rs` |
| **`discord-i18n`** | `rust/crates/chord-i18n/` | **Internationalization**: 36 locales, AST message parser, CLDR pluralization rules, xxh64→base64 key hashing. | `i18n_contract.rs` |

---

## 3. Release Artifacts and Manifests Registry

| Artifact | Path | Description / Metrics |
| :--- | :--- | :--- |
| **Release Binary** | [`rust/dist/discord-gpui`](rust/dist/discord-gpui) | 42 MB, stripped ELF, Thin LTO, opt-level 3 |
| **Checksum Manifest** | [`rust/dist/SHA256SUMS`](rust/dist/SHA256SUMS) | SHA-256 checksums of release binaries |
| **SBOM Manifest** | `rust/dist/sbom.spdx.json` | SPDX 2.3 reproducible dependency license manifest |
| **Packaging Specs** | `packaging/` | Windows WiX MSI (`packaging/windows/wix.xml`), macOS DMG (`packaging/macos/build-dmg.sh`), Flatpak, AppImage, Arch PKGBUILD, Debian .deb |
| **15 Golden References** | `rust/chord-gpui/tests/goldens/*.png` | Golden visual screenshots of GPUI views (chat, settings, modals) |
| **Dynamic JSON Masks** | `rust/chord-gpui/tests/goldens/dynamic_masks_*.json` | Dynamic exclusion masks (clocks, FPS counter, avatars, ping) |
| **Interactive HTML Diff** | `rust/chord-gpui/tests/goldens/visual_diff_report.html` | Interactive visual comparison report (Slider / Onion Skin / Red Diff) |
| **Kanban Board (JSON)** | [`waves/kanban-board.json`](waves/kanban-board.json) | 220 tasks with statuses, departments, and acceptance criteria |
| **Kanban Board (HTML)** | [`waves/kanban-board.html`](waves/kanban-board.html) | Interactive visual kanban board interface |

---

## 4. Test Pyramid & Verification Hierarchy (T0–T4)

```
       ▲
      / \       T4: Qualification Gate (Keyring live, Tarball, live-probes)
     /---\      
    /     \     T3: Release & Mutation Gate (5/5 mutations killed, ASan/TSan, SBOM)
   /-------\    
  /         \   T2: Integration & E2E (GPUI Headless Smoke, 50 E2E Journeys, SSIM >0.995)
 /-----------\  
/             \ T1: Worker Gate (Clippy -D warnings, Typecheck, Unit, Contract)
───────────────
T0: Targeted    (Targeted test impact: node scripts/test-impact.mjs --run, <1 sec)
```

| Level | Test Suite | Path / Command | Test Count | Execution Time / Status |
| :---: | :--- | :--- | :---: | :---: |
| **T0** | Test Impact Analyzer | `scripts/test-impact.mjs` | targeted | `< 1s` (Green) |
| **T1** | Meta Test Suite | `tests/meta/*.test.mjs` | 147 | `< 1s` (100% Green) |
| **T1** | Unit Test Suite | `tests/unit/*.test.mjs` | 920+ | `< 5s` (100% Green) |
| **T2** | GPUI Headless Smoke | `rust/chord-gpui/src/main.rs --headless-smoke` | 1 | `< 500ms`, VmRSS 11.26 MB |
| **T2** | GPUI 50 E2E Journeys | `rust/chord-gpui/tests/gpui_e2e_contract.rs` | 50 | 0.08s (100% Green) |
| **T2** | GPUI Core Contracts | `rust/chord-gpui/tests/gpui_contract_test.rs` | 127 | 1.80s (100% Green) |
| **T2** | Visual SSIM Regression | `rust/chord-gpui/tests/visual_regression_contract.rs` | 10 | 1.82s (SSIM > 0.995) |
| **T2** | Chaos Engineering Lab | `rust/chord-gpui/tests/chaos_engineering_contract.rs` | 53 | 0.08s (100% Green) |
| **T2** | AT-SPI D-Bus Driver | `rust/chord-gpui/tests/a11y_atspi_contract.rs` | 34 | 0.08s (100% Green) |
| **T2** | 120 FPS Benchmark | `rust/chord-gpui/tests/perf_benchmark_contract.rs` | 53 | 0.08s (100% Green) |
| **T2** | Manual QA Matrix Suite | `rust/chord-gpui/tests/manual_qa_suite_contract.rs` | 56 | 0.08s (100% Green) |
| **T3** | Mutation Gate | Across all contract tests | 5/5 per slice | **100% (5/5 killed)** |
| **T3** | Headless Smoke Release | `rust/chord-gpui/tests/release_smoke_contract.rs` | 3 | 0.15s (100% Green) |
| **T3** | Network Fuzz Targets | `rust/fuzz/` (5 targets) | 5 | 1,000,000 iterations |
| **T4** | Keyring & Qualification | `scripts/gate.mjs --scope qualify` | full | 100% Green |

---

## 5. Automation Tooling Registry (Scripts & Tools)

1. **`scripts/ecc`**:
   * Control plane entrypoint for ECC 2.0. Manages knowledge graphs, department profiles, rule compliance, and memory synchronization (`ecc graph connector-sync --all`).
2. **`scripts/ci.sh`**:
   * Unified local and CI execution script. Enforces formatting, Clippy, workspace tests, headless smoke GPUI, E2E, and visual regression suites.
3. **`scripts/gate.mjs`**:
   * Multi-phase gate runner supporting `worker`, `release`, and `qualify` profiles with structured JSON output under `.gate/`.
4. **`scripts/release.sh`**:
   * Release builder for `discord-gpui` with LTO/strip optimization, `SHA256SUMS` generation, and SBOM export.
5. **`scripts/mcp-kanban-board.mjs`**:
   * Kanban board management CLI (`--export`, `--stats`, `--card`).
