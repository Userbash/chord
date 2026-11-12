# Comprehensive QA Automation Strategy & Department Protocol for `discord-gpui`

**Approval Date:** 2026-11-11  
**Document Version:** v2.0 (Native GPUI & Modern Testing Ecosystem)  
**Owning Department:** G4 QA & Automation Lead  
**Task Board Status:** 8 micro-decomposed tasks loaded in Backlog (`waves/kanban-board.json` & `waves/kanban-board.html`)  

---

## 1. QA Mission Statement: Philosophy, Methodology, and Objectives

This specification defines the testing architecture, quality assurance procedures, Definition of Done, and automation roadmap for the **`discord-gpui`** native client (WGPU, 120 FPS, pure Rust).

### Core QA Engineering Objectives:
1. **0 Regressions:** Zero commits may break established wire protocols (Gateway, REST v9, Voice opcodes 18..=22, DAVE MLS v1).
2. **Strict 120 FPS Frame Budget:** Frame render budget strictly **< 8.33 ms** (p95 < 2 ms).
3. **Memory Footprint & Leaks:** Stable Resident Set Size (**VmRSS < 60 MB**) under 24-hour continuous workload.
4. **Data Security & E2EE:** 100% protection against unmasked tokens in logs and full verification of DAVE E2EE cryptographic invariants (RFC 9420 KDF, 8-byte authentication tag, anti-replay window).
5. **Mutation Gate 5/5 (100% Mutation Kill Rate):** Tests must fail when intentional faults are injected into logic.

---

## 2. Comprehensive Testing Matrix: What, Where, and How to Apply

```text
                                      ▲
                                     / \       E2E (End-to-End User Journeys)
                                    /   \      Headless Wayland, Mock Gateway, release binary
                                   /-----\     
                                  /       \    Visual Regression & Perceptual Diff
                                 /         \   SSIM > 0.99, dynamic region masking
                                /-----------\  
                               /             \ GUI & Component (AT-SPI Driver)
                              /               \Semantic tree org.a11y.Bus, Tab navigation
                             /-----------------\
                            /                   \ Chaos & Fault Injection
                           /                     \Jitter 20-60ms, socket drops, reconnects
                          /-----------------------\
                         /                         \ Fuzzing (cargo-fuzz / libFuzzer)
                        /                           \10M iterations of parsers and RTP
                       /-----------------------------\
                      /                               \ Contract / Interface
                     /                                 \Reference cleanroom oracles docs/re/
                    /-----------------------------------\
                   /                                     \ Unit & Property-Based
                  /                                       \Pure logic, proptest, NIST KAT
                 ◄─────────────────────────────────────────►
```

| Level / Test Type | Scope | Tooling | Execution Speed | Verified Domain |
|---|---|---|---|---|
| **Static & Linter** | All Rust sources (`src/**/*.rs`) | `cargo clippy -D warnings`, `rustfmt`, `cargo-deny` | ~1–2 s | Code hygiene, zero UB, function size (<50 lines), module size (<800 lines) |
| **Unit Tests** | Isolated business logic | `cargo test --lib` | Microseconds (µs) | Snowflake parsing, hashing, bitrate computation, validators |
| **Property-Based** | Queue algorithms, parsers | `proptest`, `quickcheck` | Milliseconds (ms) | Invariants under random inputs and boundary byte sequences |
| **Contract Tests** | Network protocols, IPC, REST, Voice | Cleanroom oracles `docs/re/` | ~5–10 ms | Strict Discord wire protocol conformance, JSON Schema, ETF big-int |
| **Fuzzing** | Network parsers, RTP, DAVE E2EE | `cargo-fuzz` (libFuzzer) + ASan | Hours / Overnight | Robustness against malformed packets, overflows, panics |
| **GUI Component** | Windows, sidebars, menus, modals | `gpui-kit`, `cx.listener`, `cx.notify` | ~1–5 ms | Reactive `AppState` updates, layout calculation, filtering |
| **Accessibility (A11y)**| UI accessibility | D-Bus AT-SPI (`org.a11y.Bus`) | ~10–20 ms | Keyboard navigation (Tab), element roles, WCAG 2.2 AAA contrast |
| **Visual Regression** | Screen rendering and layout | Perceptual SSIM (>0.99), `llvmpipe` | ~100 ms | Screenshot comparison with golden references with dynamic region masking |
| **Chaos Testing** | Network stack, sockets, audio | Fault injector, packet dropper | Seconds | 5–30% UDP drop rate, jitter spikes, Opcode 6/7 reconnects |
| **Performance Benchmark**| Render pipeline, scroll | High-res clock, `/proc/self/statm` | Milliseconds | Frame latency < 8.33 ms (120 FPS), VmRSS < 60 MB |
| **Mutation Testing** | All production PRs | Mutation injector script | Seconds | Mandatory kill rate: 5 of 5 mutants per module |
| **Manual QA** | Pre-release sanity check | Checklists, Bug-bash sessions | Minutes | UX evaluation, behavior across GNOME/KDE/X11 |

---

## 3. Code Review & Quality Checklist

### 3.1. Static Checks and Architectural Limits:
- [ ] **Zero `unwrap()` / `expect()` in production paths:** Network faults or malformed packets must return `Result<T, AppError>`. Panics strictly forbidden.
- [ ] **Functions < 50 lines:** Every function has a single, cohesive responsibility.
- [ ] **Files < 800 lines:** Soft ceiling; extract logic to submodules when exceeded.
- [ ] **Immutability:** Pure transformations preferred over in-place reference mutations.
- [ ] **Zero Allocations on Hot Paths:** 120 FPS render loops and audio packet handlers (Opus/RTP) forbid dynamic `String` and `Vec` allocations; use reusable ring buffers (`AudioRingBuffer`).

### 3.2. GUI Validation (Native GPUI):
1. **Rendering & Layout:**
   - No text collisions when compressed to minimum resolution (900x600).
   - Flexbox utilizes `min_h_0` / `min_w_0` on child scrollable containers.
   - User panel (`UserPanel`) remains pinned without being pushed out by long channel rosters.
2. **Virtualization & Scrolling:**
   - Chat with 100,000 messages allocates only the visible window (~30 items) in memory.
   - Scroll position remains anchored during upward message history fetching (`GET /channels/{id}/messages?before=...`).
3. **Interactive States:**
   - Clickable elements expose clear states: `Default`, `Hover`, `Active`, `Focus`.
   - Tooltips render with a 300 ms delay without obstructing controls.
4. **Accessibility Tree Semantics (AT-SPI):**
   - Interactive elements expose clear `Accessible Name` values.
   - Modal views trap keyboard focus and dismiss upon `Escape`.

---

## 4. Granular Task Decomposition (Tasks -> Subtasks -> Micro-steps)

All 8 tasks are registered in the project Kanban board under status `📋 Backlog` (`status: "backlog"`):

### TASK-901: Native E2E Test Harness Architecture (Virtual Wayland + Mock Gateway)
- **Subtask 901.1: Headless In-Memory Wayland Compositor**
  - *Micro-step 1.1:* Launch `cage`/`wlroots` headlessly via `scripts/run-headless-compositor.sh`.
  - *Micro-step 1.2:* Software renderer integration with `llvmpipe` (`LIBGL_ALWAYS_SOFTWARE=1`) for GPU-free CI environments.
  - *Micro-step 1.3:* Automatic allocation of isolated virtual socket `wayland-99`.
- **Subtask 901.2: Local Tokio-Based Mock Discord Gateway Daemon**
  - *Micro-step 2.1:* HTTP/2 mock server for REST v9 routes (`/users/@me`, `/guilds`, `/channels/{id}/messages`).
  - *Micro-step 2.2:* WebSocket server implementing Discord Gateway v9 (Heartbeat ACK, Opcode 2 IDENTIFY, Opcode 4 VOICE_STATE_UPDATE).
  - *Micro-step 2.3:* Fixture loader for `docs/re/capture-ready.jsonl` traces for realistic state hydration.
- **Subtask 901.3: E2E Lifecycle Runner & Process Orchestrator**
  - *Micro-step 3.1:* Rust test runner crate `tests/e2e/harness/runner.rs` with automatic startup and graceful shutdown.
  - *Micro-step 3.2:* Intercept stdout/stderr and dump backtraces on crash.
  - *Micro-step 3.3:* Confirm zero orphaned processes or lingering file descriptors upon test completion.

### TASK-902: Semantic GUI Driver and Test Automation via D-Bus AT-SPI
- **Subtask 902.1: Accessibility Tree Exposure in GPUI Components**
  - *Micro-step 1.1:* Verify Accessible Roles for buttons, inputs, and lists (`Role::PushButton`, `Role::Text`, `Role::List`).
  - *Micro-step 1.2:* Bind Accessible Name and Description from i18n catalog.
  - *Micro-step 1.3:* Expose component states (`State::Focused`, `State::Selected`, `State::Checked`, `State::Disabled`).
- **Subtask 902.2: High-Level Page-Object DSL (`AppDriver`)**
  - *Micro-step 2.1:* Method `driver.find_by_role_and_name('button', 'Log In')` with deterministic waiting.
  - *Micro-step 2.2:* Method `driver.click(element)` invoking D-Bus `do_action(0)`.
  - *Micro-step 2.3:* Method `driver.type_text(element, 'Text')` emulating keystrokes.
  - *Micro-step 2.4:* Method `driver.press_hotkey('Ctrl+K')` for keyboard navigation.
- **Subtask 902.3: Automated WCAG 2.2 Accessibility Audit**
  - *Micro-step 3.1:* Validate color contrast against backgrounds (minimum 4.5:1 for normal text, 7:1 for AAA).
  - *Micro-step 3.2:* Verify keyboard-only navigation (`Tab`, `Shift+Tab`, `Enter`, `Space`, `Escape`, arrow keys).
  - *Micro-step 3.3:* Ensure zero focus traps in modal dialogs.

### TASK-903: Network Protocol and DAVE MLS v1 Cryptography Fuzzing (`cargo-fuzz`)
- **Subtask 903.1: Fuzz Targets for Discord Gateway and Voice Parsers**
  - *Micro-step 1.1:* Fuzz target `fuzz_gateway_json_etf` for inbound gateway messages.
  - *Micro-step 1.2:* Fuzz target `fuzz_voice_udp_packets` for 74-byte IP Discovery and 8-byte keepalives.
  - *Micro-step 1.3:* Fuzz target `fuzz_rtp_header_extensions` for RTP header parsing (`0xBEDE`, One-Byte/Two-Byte Header).
- **Subtask 903.2: Discord DAVE E2EE MLS v1 Cryptography Fuzzing**
  - *Micro-step 2.1:* Fuzz decryptor `Aes128GcmTruncated8` against random and truncated byte sequences.
  - *Micro-step 2.2:* Validate handling of corrupted MLS Commit and Proposal packets in `DaveVoiceSession`.
  - *Micro-step 2.3:* Test anti-replay sliding window against u64 overflow attacks.
- **Subtask 903.3: Automated CI Integration and Crash Corpus Aggregation**
  - *Micro-step 3.1:* Configure `cargo fuzz run` across 1,000,000 iterations in nightly CI pipeline.
  - *Micro-step 3.2:* Save and minimize interesting corpus samples in `tests/fuzz/corpus/`.
  - *Micro-step 3.3:* Automated report generation upon encountering panics or out-of-bounds reads.

### TASK-904: GPUI Visual Regression Automation via SSIM (>0.99) and Region Masking
- **Subtask 904.1: Deterministic FrameBuffer Screenshot Generator**
  - *Micro-step 1.1:* Capture uncompressed RGBA pixels directly from GPUI/WGPU virtual framebuffer.
  - *Micro-step 1.2:* Forcibly disable subpixel font antialiasing in test mode for cross-platform consistency.
  - *Micro-step 1.3:* Freeze system time (Frozen Clock) to eliminate timestamp layout shift.
- **Subtask 904.2: Perceptual SSIM Comparison with Region Masks**
  - *Micro-step 2.1:* Structural Similarity Index (SSIM) computation module with 0.995 tolerance threshold.
  - *Micro-step 2.2:* JSON region mask definitions (timestamps, avatars, FPS/ping badges).
  - *Micro-step 2.3:* Generate HTML visual difference report with highlighted pixel deviations.
- **Subtask 904.3: Golden Baseline Snapshots**
  - *Micro-step 3.1:* Capture baseline screenshots of main window in Discord Dark at 1280x720 and 1920x1080.
  - *Micro-step 3.2:* Capture baselines for all 8 tabs in `SettingsDialog`.
  - *Micro-step 3.3:* Capture baselines for modals `CaptchaModal`, `CreateGuildModal`, `ScreenShareModal`.
  - *Micro-step 3.4:* Capture baselines for context menus and profile popovers.

### TASK-905: Chaos Engineering Testbed: Network Drops, Jitter, and Fault Recovery
- **Subtask 905.1: Network Fault Injection**
  - *Micro-step 1.1:* Emulate TCP socket resets every N seconds.
  - *Micro-step 1.2:* Verify seamless transition to RESUME (Opcode 6) retaining seq and session token.
  - *Micro-step 1.3:* Inject HTTP 429 Too Many Requests with randomized `retry_after_ms` validating backoff.
  - *Micro-step 1.4:* Inject 5%, 15%, and 30% voice UDP packet loss validating Opus adaptive bitrate and PLC.
- **Subtask 905.2: Hardware and Driver Faults (Audio/Video Driver Faults)**
  - *Micro-step 2.1:* Sudden audio device disconnection during call with automatic fallback to Default.
  - *Micro-step 2.2:* Simulate PipeWire daemon restart during active screen share.
  - *Micro-step 2.3:* Out-of-memory fault injection verifying cache pruning in `MediaCache`.
- **Subtask 905.3: State Integrity Audit**
  - *Micro-step 3.1:* Verify zero duplicate messages in chat feed after reconnection storms.
  - *Micro-step 3.2:* Verify draft message preservation across unexpected modal closures.
  - *Micro-step 3.3:* Confirm proper file descriptor and socket reclamation post-failure.

### TASK-906: 120 FPS Benchmark and Real-Time Memory Profiling (Heap/RSS < 60 MB)
- **Subtask 906.1: 120 FPS Frame Budget Monitoring (<8.33 ms)**
  - *Micro-step 1.1:* High-resolution timestamps at every step of layout and rendering.
  - *Micro-step 1.2:* Aggregate p50, p95, p99 frame duration percentiles while scrolling 100,000 messages.
  - *Micro-step 1.3:* Fail test if frame render duration exceeds 8.33 ms in >0.1% of frames.
- **Subtask 906.2: Memory Leak Profiling**
  - *Micro-step 2.1:* Heap allocation profiling via `jemalloc` / `heaptrack` across 24-hour soak tests.
  - *Micro-step 2.2:* Enforce VmRSS ceiling: strictly < 60 MB across the application.
  - *Micro-step 2.3:* Test garbage collection and GPU VRAM texture release for avatars and attachments.
- **Subtask 906.3: Cold Start Benchmark**
  - *Micro-step 3.1:* Measure duration from process launch to First Meaningful Paint.
  - *Micro-step 3.2:* Target: < 150 ms with warm SQLite cache, < 300 ms cold boot.
  - *Micro-step 3.3:* Publish performance trends in CI build artifacts.

### TASK-907: Automated Code Quality Gate: Clippy, ASan/TSan, and 5/5 Mutation Gate
- **Subtask 907.1: Static Analysis and Rust Linting**
  - *Micro-step 1.1:* Enforce `cargo clippy --all-targets -- -D warnings`.
  - *Micro-step 1.2:* Validate formatting via `rustfmt --check --edition 2024`.
  - *Micro-step 1.3:* Audit dependencies via `cargo audit` and licenses via `cargo-deny`.
  - *Micro-step 1.4:* File size limits: functions < 50 lines, modules < 800 lines.
- **Subtask 907.2: LLVM Sanitizers (ASan & TSan)**
  - *Micro-step 2.1:* Run contract tests with `RUSTFLAGS="-Zsanitizer=address"` to detect use-after-free and buffer overflows.
  - *Micro-step 2.2:* Run with `RUSTFLAGS="-Zsanitizer=thread"` to surface data races in concurrent ring buffers.
  - *Micro-step 2.3:* Confirm zero deadlocks between Tokio runtime and GPUI Event Loop.
- **Subtask 907.3: Mutation Testing Gate**
  - *Micro-step 3.1:* Generate 5 mutation variants per modified module.
  - *Micro-step 3.2:* Execute targeted test suite against mutated binaries.
  - *Micro-step 3.3:* Mandatory pass criteria: strictly 5 out of 5 mutants killed (100% kill rate).

### TASK-908: Manual Exploratory Testing Suite & Checklists
- **Subtask 908.1: Smoke & Sanity Checklists**
  - *Micro-step 1.1:* Authentication checklist (password login, QR code, captcha 40003, token storage, logout).
  - *Micro-step 1.2:* Chat checklist (text, emoji, 25 MB attachments, pinning, editing, deletion, quotes).
  - *Micro-step 1.3:* Voice checklist (connection, mute/deafen, audio devices, Push-to-Talk, volume).
  - *Micro-step 1.4:* Screen share checklist (screen/window selection, 720p/1080p, on-the-fly switching).
- **Subtask 908.2: Linux Desktop Cross-Platform Matrix**
  - *Micro-step 2.1:* GNOME 46/47 Wayland (AppIndicator tray and native notifications).
  - *Micro-step 2.2:* KDE Plasma 6 Wayland (PipeWire portal and fractional scaling at 125%/150%).
  - *Micro-step 2.3:* Hyprland / Sway (tiling window managers and window resizing).
  - *Micro-step 2.4:* X11 session (Xfce / i3) with X11 SHM fallback verification.
- **Subtask 908.3: Exploratory Testing Charters**
  - *Micro-step 3.1:* Stress input charter: 2000 character boundaries, Unicode, RTL text, malformed Markdown.
  - *Micro-step 3.2:* Rapid navigation monkey test (10 clicks per second).
  - *Micro-step 3.3:* Network disruption scenario: 30-second disconnect recovery.
  - *Micro-step 3.4:* Defect logging in issue tracker with reproduction steps, screenshots, GDB traces, and P0-P3 ranking.

---

## 5. Interactive Kanban Board Access

- **Board Data File:** [`waves/kanban-board.json`](waves/kanban-board.json) (128 cards: 120 Done, 8 Backlog).
- **Interactive HTML Interface:** [`waves/kanban-board.html`](waves/kanban-board.html) (filtering by department, priority P0–P3, and full-text search).
