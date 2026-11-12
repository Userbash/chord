# Chord: session checkpoint (compact context)

Entry point for new dialogues. Read first; refer to detailed files as needed.
Updated: **2026-11-12** (Wave **RW-47** under active development; Kanban board at **85.00%** completion).

---

## 1. Current Project Status (2026-11-12)

- **Native Rust GPUI Client (`chord-gpui`)**:
  - Full migration from WebKit/Tauri prototype to direct GPU rendering via WGPU at 120 FPS (`rust/chord-gpui/`).
  - Release binary: [`rust/dist/chord`](rust/dist/chord) (and legacy `rust/dist/discord-gpui`) — **42 MB** (stripped ELF, Thin LTO, opt-level 3).
  - SHA-256 checksum in [`rust/dist/SHA256SUMS`](rust/dist/SHA256SUMS): `d5e50180d2b0a6edd8faa9c2ad908d931cecb8c5765190bff25be90c7e0e002e`.
  - Idle memory consumption: **11.26–18.00 MB VmRSS** (strict budget limit `< 60 MB`), 120 FPS frame budget maintained.
  - Deterministic E2E test harness: [`rust/chord-gpui/tests/gpui_e2e_contract.rs`](rust/chord-gpui/tests/gpui_e2e_contract.rs) — 50 end-to-end user journeys (100% pass in 0.08s).

- **Active Wave: Wave RW-47 (Sprints 1–5 & Deep Parity Integration)**:
  - Active orchestration run: `run_a9411d4ae3c4` managed via Orca Orchestration.
  - **Kanban Board:** [`waves/kanban-board.html`](waves/kanban-board.html) and [`waves/kanban-board.json`](waves/kanban-board.json):
    - Total cards: **220**
    - **Completed and pushed to GitHub (`done`):** **187 tasks (85.00%)**
    - **In active development (`in_dev`):** **10 tasks** (full worker saturation under Zero-Idle policy)
    - **In backlog (`backlog`):** **23 tasks**
  - **Recently Accepted and Verified Subsystems (100% Green + 5/5 Mutation Gate):**
    - `TASK-1004` / `TASK-1004B`: Interactive in-chat polls and voting (PollCardView with animated score bars and winner badges).
    - `TASK-1006`: Native spellchecker in composer (hunspell / libenchant-2, squiggly underline `#f23f43`, context menu suggestions).
    - `TASK-1007`: Guild role hierarchy, drag-and-drop PATCH `/guilds/{id}/roles`, and 64-bit permission bitmask editor (41 flags).
    - `TASK-1008`: Guild audit log viewer (GET `/guilds/{id}/audit-logs`, diff decoding, secret masking).
    - `TASK-1009`: TOTP 2FA modal entry and active session manager.
    - `TASK-1010`: D-Bus ScreenSaver / Wayland idle detector and auto AFK voice channel transfer.
    - `TASK-1011`: Advanced WebRTC diagnostics and RTC Internals HUD (RTCP Sender/Receiver Reports, RTT/jitter/loss 60fps curves, DAVE MLS v1 Privacy Code 6x5).
    - `TASK-1012` / `TASK-1012A`: Soundboard SFX engine (Voice Opcode 18, Opcode 19 SPEAKING, Opus 48kHz stereo LRU cache, 20ms RTP mixer with `tanh()` soft clipping).
    - `TASK-1013`: Spotify MPRIS D-Bus status integration (Listen Along, track interception, Opcode 3 `LISTENING` presence).
    - `TASK-1014`: Steam Deck & Gamepad UI mode (gilrs, 4-zone focus grid, SteamOS on-screen keyboard via D-Bus).
    - `TASK-1015`: System autostart generator (XDG Autostart `~/.config/autostart/` and silent tray launch with VmRSS < 8 MB).
    - `TASK-1016`: Cross-platform packaging specifications (Windows WiX MSI / Portable in `data/`, and macOS DMG with NSVisualEffectView Dark HUD Vibrancy).
    - `TASK-1001` / `TASK-1001A` / `TASK-1001B`: Forum channels (`GUILD_FORUM` type 15), REST pagination of active threads, and 2-column `ForumGridView` with tag filters.
    - `TASK-1002` / `TASK-1002A`: Nested discussion threads (`PUBLIC_THREAD` / `PRIVATE_THREAD`), auto-archive timers (60..10080 min), and member sync.
    - `TASK-1003` / `TASK-1003A`: Voice messages with interactive waveform (OGG Opus 48kHz mono 32kbps, 256-byte RMS waveform, flags 8192).
    - `TASK-1005` / `TASK-1005A` / `TASK-1005B`: Custom emojis and animated Lottie stickers (two-tier cache 256 MB RAM + disk, 380x420px interactive picker).

---

## 2. Architecture and Directory Structure

- **Repository Layout:**
  - `rust/crates/`: core modules (`chord-app`, `chord-core`, `chord-voice`, `chord-codec`, `chord-media`, `chord-platform`, `chord-store`, `chord-i18n`, `chord-ui`, `chord-json`, `chord-perf`, `chord-updater`, `chord-conformance`).
  - `rust/chord-gpui/`: native client UI on GPU WGPU 120 FPS.
  - `packaging/`: distribution packaging manifests (`windows/wix.xml`, `macos/build-dmg.sh`, Linux deb/rpm/AppImage).
  - `docs/articles/`: technical and public articles about the project.
  - `docs/archive/wave-artifacts/`: archive of intermediate sprint artifacts (early `CONTRACTS`, `PM`, `QA`, `QUALITY`).
  - `waves/`: sprint history, event log `db.jsonl`, state `STATE.md`, Kanban board `kanban-board.json`/`html`.
  - `tests/`: 12-tier testing matrix (`tests/unit/`, `tests/smoke/`, `tests/e2e/`, `tests/meta/`).
  - `scripts/`: automation scripts for release, quality gate, SBOM, and parity verification (`gate.mjs`, `ci.sh`, `release.sh`, `quality-gate.mjs`, `mcp-kanban-board.mjs`).

- **Quality Control & Meta-Tests:**
  - **147 / 147 meta-tests passing (100% PASS, 0 FAIL)** (`npm run test:meta`).
  - Strict prohibition against relaxing tests (`TEST-POLICY.md`).
  - Mutation testing gate: **100% Mutation Kill Rate (5/5 control mutations killed per module)**.
  - `#![forbid(unsafe_code)]` enforced across all Rust crates.
  - Code complexity limits: `< 50 lines per function`, `< 800 lines per file`.
  - Worker memory ceiling: `WORKER_RAM_LIMIT <= 25 GB` (26–34 GB free system headroom).
