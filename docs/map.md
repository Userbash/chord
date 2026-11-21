# Master Project Knowledge Graph & Component Map

> **Automated Codebase Knowledge Graph** (`scripts/sync-project-map.mjs`).  
> Active Branch: `tauri` | HEAD: `dd003713` | Roadmap Progress: **80%** (35/44 cards completed) | Tests: **643+** | Worker RAM: `2.53 GiB / 25.00 GiB`.

---

## 1. Quality & Architectural Budget Summary

| Metric | Current Value | Target Threshold | Status |
|---|---|---|---|
| **Roadmap Wave Slots** | 44 total | 44 slots | 🔄 In Progress (80%) |
| **Completed & Verified** | **35** tasks | 100% | ✅ Green |
| **Pending Tasks** | **9** tasks | 0 | 🟡 P0: 5, P1: 1, P2: 5, P3: 0 |
| **Contract & Unit Test Suites** | **643+** files | 100% pass, 0 flaky | ✅ Full Coverage (371+ suites) |
| **Thin Bridge Architectural Budget** | **≤ 600 LOC** (avg 291 LOC) | ≤ 800 LOC hard ceiling | ✅ 8/8 Bridges Compliant |
| **Worker Process RAM (OOM Guard)** | **2.53 GiB** | ≤ 25.00 GiB ceiling | ✅ Safe (< 10 GiB) |
| **Zero-Token Automation** | Git-Sync, Auto-Driver, TDD-Plan, Bridges | 100% Script-driven | ✅ >18,500 Tokens Saved |

---

## 2. System Architecture Layers (Mermaid Knowledge Graph)

```mermaid
flowchart TB
  classDef ok fill:#e2f6e7,stroke:#2f9e5f,color:#1b2430
  classDef part fill:#fdf3d8,stroke:#c99700,color:#1b2430
  classDef no fill:#fbe3e3,stroke:#c0392b,color:#1b2430
  classDef neu fill:#eef2f7,stroke:#7d93ab,color:#1b2430
  classDef amcp fill:#f0e6ff,stroke:#8a2be2,color:#1b2430

  GOAL["Native Discord Client & Protocol Suite (Chord)"]:::neu
  A["Scenario A — Independent Lightweight Desktop Client"]:::ok
  GOAL --> A
  A --> LAYERS

  subgraph AMCP["Zero-Token Automation & Control Plane"]
    sync["Multi-Remote Git Sync (scripts/git-sync.mjs)"]:::amcp
    driver["Orchestration Auto-Driver (scripts/orchestration-auto-driver.mjs)"]:::amcp
    tddsync["TDD-Plan §8 Sync (scripts/sync-tdd-plan.mjs)"]:::amcp
    detector["Worker Routine Detector (scripts/worker-routine-detector.mjs)"]:::amcp
    bridgeaudit["Thin Bridge Linter (scripts/audit-thin-bridges.mjs)"]:::amcp
    mapsync["Project Map Auto-Sync (scripts/sync-project-map.mjs)"]:::amcp
    diffutil["Report Diff Engine (scripts/report-diff.mjs)"]:::amcp
    sync --- driver
    driver --- tddsync
    tddsync --- detector
    detector --- bridgeaudit
    bridgeaudit --- mapsync
    mapsync --- diffutil
  end

  subgraph LAYERS["System Architecture Layers (Chord Core)"]
    direction TB
    subgraph S1["1. User Interface (chord-ui & Tauri 2)"]
      u1["Design Tokens: 5,132 CSS Variables"]:::ok
      u2["Themes: Dark / Light / Midnight / Custom"]:::ok
      u3["Virtualized Chat & Channel Tree (Virtualizer Bridge)"]:::ok
      u4["Localization: 36 Locales (chord-i18n & AssetManager)"]:::ok
      u5["Popovers & Modals (Floating Popover Bridge)"]:::ok
    end

    subgraph S2["2. Protocol Core (chord-core)"]
      p1["Gateway v9: WebSocket + ETF + zstd (40 top-level READY keys)"]:::ok
      p2["REST API v9: Pure reqwest + url::Url (D-02 Catalog)"]:::ok
      p3["Permissions: 64-bit Bitmask Calculator"]:::ok
      p4["Session & Streaming Management (Go-Live / Streamer Mode)"]:::ok
    end

    subgraph S3["3. Audio, Video & Media Pipeline (chord-voice)"]
      m1["Audio Engine: cpal 48kHz + crossbeam ArrayQueue (527 LOC)"]:::ok
      m2["DAVE MLS v1: RFC 9420 KeyRatchet E2EE (ring + aes-gcm, 0 unsafe)"]:::ok
      m3["Voice Activity Detection (VAD Energy Detector, 95 LOC)"]:::ok
      m4["Media Capture Constraints (D-04 Media Constraints, 4/4 tests)"]:::ok
      m5["Streaming Wire Protocol & RTCP PLI/FIR/REMB (D-05)"]:::ok
    end

    subgraph S4["4. Local Store & Cache (chord-store)"]
      d1["SQLite + FTS5 Full-Text Search (Pure rusqlite)"]:::ok
      d2["Database Worker Queue: sync_channel(1024) (559 LOC)"]:::ok
      d3["Cache Reset & WAL Checkpoint / VACUUM (R6-06)"]:::ok
    end

    subgraph S5["5. Platform Adapters (chord-platform)"]
      pl1["System Notifications & Level Cascade (D-06 Notification Defaults)"]:::ok
      pl2["Linux D-Bus + MPRIS Media Hotkeys"]:::part
      pl3["Portable Mode & Windows / macOS Adapters (R6-03, R6-04)"]:::ok
      pl4["Steam Deck & Gamepad Controller Mode (R6-01)"]:::ok
    end
  end

  AMCP --> LAYERS
```

---

## 3. Thin Bridge Architectural Matrix

All technological bridges are enforced by `scripts/audit-thin-bridges.mjs` (`npm run audit:bridges`) to strictly remain under 600 LOC (soft ceiling) / 800 LOC (hard ceiling) and delegate 100% of domain processing to audited libraries:

| Bridge / Adapter | Crate / File Path | Current LOC | Budget Ceiling | Upstream Libraries | Status |
|---|---|---|---|---|---|
| **REST Transparent Bridge** | `chord-core` / `src/rest/bridge.rs` | 574 | ≤ 800 | `reqwest`, `url::Url` | ✅ 33/33 Tests Passed |
| **SQLite Store Bridge** | `chord-store` / `src/worker.rs` | 559 | ≤ 800 | `rusqlite`, `mpsc(1024)` | ✅ 22/22 Tests Passed |
| **DAVE E2EE Voice Bridge** | `dave-spike` / `src/bridge.rs` | 468 | ≤ 800 | `ring`, `aes-gcm` (0 unsafe) | ✅ 7/7 Contracts Passed |
| **Audio Lock-Free Ring Buffer** | `audio-spike` / `src/ring_buffer.rs` | 201 | ≤ 400 | `crossbeam_queue::ArrayQueue` | ✅ 7/7 Contracts Passed |
| **Audio VAD Energy Detector** | `audio-spike` / `src/vad.rs` | 95 | ≤ 400 | `cpal`, native RMS calculation | ✅ Verified |
| **Frontend Signals Bridge** | `gui-spike-ui` / `state/signals.js` | 94 | ≤ 300 | `@preact/signals-core` | ✅ Verified |
| **Frontend Virtualizer Bridge** | `gui-spike-ui` / `state/virtualizer.js` | 157 | ≤ 400 | `@tanstack/virtual-core` | ✅ Verified |
| **Frontend Popover Bridge** | `gui-spike-ui` / `state/popover.js` | 180 | ≤ 400 | `@floating-ui/dom` | ✅ Verified |

---

## 4. Documentation & Specifications Index

- **Clean-Room Reverse Engineering:** [`docs/re/`](re/README.md) (Architecture: [`docs/re/architecture.md`](re/architecture.md), Gateway: [`docs/re/gateway-protocol.md`](re/gateway-protocol.md)).
- **Client Specifications:** [`docs/specs/`](specs/) — design tokens, keyboard navigation, session parameters.
- **Sprint Wave Journal:** [`waves/`](../waves/README.md) — recovery checkpoint [`STATE.md`](../waves/STATE.md) and Kanban board [`kanban-board.json`](../waves/kanban-board.json).
- **Engineering Guidelines:** [`AGENTS.md`](../AGENTS.md), [`TEST-POLICY.md`](../TEST-POLICY.md).
