# Master Project Knowledge Graph

Coverage: Goals and user scenarios → research layers (coverage and artifacts) → data sources → blockers.
Status colors: Green — complete, Yellow — partial / in progress, Red — blocker / risk.

Artifact documentation:
- Clean-room Reverse Engineering: `docs/re/` (Index: [`docs/re/README.md`](re/README.md), architecture diagrams: [`docs/re/architecture.md`](re/architecture.md)).
- Rust Client Specifications: [`docs/rust-client/`](rust-client/README.md) — concept, features, architecture, phases P0–P8, test plans.
- Sprint Wave Journal: [`waves/`](../waves/README.md) — event log (`db.jsonl`), recovery checkpoint ([`STATE.md`](../waves/STATE.md)), and artifacts.
- Transfer Backlog: [`docs/feature-backlog.md`](feature-backlog.md).
- Reference Project Analyses: [`docs/vesktop-analysis.md`](vesktop-analysis.md) and [`docs/discord-qt-analysis.md`](discord-qt-analysis.md).

```mermaid
flowchart TB
  classDef ok fill:#e2f6e7,stroke:#2f9e5f,color:#1b2430
  classDef part fill:#fdf3d8,stroke:#c99700,color:#1b2430
  classDef no fill:#fbe3e3,stroke:#c0392b,color:#1b2430
  classDef neu fill:#eef2f7,stroke:#7d93ab,color:#1b2430

  GOAL["Rust Client with Discord UX/UI from Scratch"]:::neu
  A["Scenario A — Independent Messenger (Recommended)"]:::ok
  C["Scenario C — Direct Discord Protocol Client (Legal Risk)"]:::no
  GOAL --> A
  GOAL --> C
  GOAL --> S8
  A --> LAYERS

  subgraph LAYERS["Research Layers (Waves R1, R2-NET/R3, R7)"]
    direction TB
    subgraph S1["1. UI System and UX"]
      u1["Tokens: 5,132 CSS variables"]:::ok
      u2["Themes: Dark / Light / Midnight"]:::ok
      u3["Densities: Cozy / Compact"]:::ok
      u4["Screens: Search, Emojis, Settings, Popovers (D-03)"]:::ok
      u5["Localization: String catalogs & i18n keys"]:::ok
      u6["Typography: 22 × WOFF2 fonts"]:::ok
    end
    subgraph S2["2. Protocol and Networking"]
      p1["Gateway: WebSocket + ETF + zstd stream"]:::ok
      p2["Opcodes 0–43, QoS op40"]:::ok
      p3["Voice Gateway: JSON, heartbeat 13.75s"]:::ok
      p4["Session Description: Key and encryption mode"]:::ok
      p5["Cadence-based socket binding"]:::ok
      p6["Clean READY: 4 logins (D-01); GUILD_CREATE full mode"]:::ok
    end
    subgraph S3["3. Media and Cryptography"]
      m1["Audio: Opus 48kHz stereo, 20ms frame"]:::ok
      m2["SRTP: AES-256-GCM and XChaCha20-Poly1305"]:::ok
      m3["Screen Sharing: PipeWire + X11 SHM fallback"]:::ok
      m4["DAVE MLS v2: MLS 1.0 KeyRatchet E2EE"]:::ok
    end
    subgraph S6["6. Reference Implementations"]
      ref1["Vesktop 1.6.7 (Screen sharing & packaging)"]:::ok
      ref2["discord-qt 0.5.1 (Protocol contracts & schemas)"]:::ok
    end
    subgraph S8["8. Native Rust Client (docs/rust-client/)"]
      rc1["CONCEPT.md: Zero Electron/Chromium"]:::ok
      rc2["FEATURES.md: >= 100 features mapped"]:::ok
      rc3["ARCHITECTURE.md: GPUI 120 FPS + WGPU"]:::ok
      rc4["PLAN.md: Waves P0–P8"]:::ok
      rc5["TEST-PLAN.md: >= 110 tests L0–L7"]:::ok
      rc6["PROJECT-STRUCTURE.md: Workspace crates"]:::ok
      rc7["TRACEABILITY.md: F-* to T-* mapping"]:::ok
      rc8["VERIFICATION.md: Evidence validation"]:::ok
    end
    bkl["Transfer Backlog"]:::ok
  end

  ref1 --> bkl
  ref2 --> bkl
  bkl --> S8
```

---

## Team and Batch Pipeline Flow

```mermaid
flowchart TB
  OWNER["Project Owner"] --> TL["Team Lead<br/>Queue, Contracts, Acceptance"]
  TL --> TECHL["Tech Lead<br/>Architecture & Shared Integrations"]
  TECHL --> CORE["G1 Backend<br/>Rust Core & Native BFF"]
  TECHL --> CLIENT["G2 Frontend<br/>UI Views & Navigation"]
  TECHL --> VOICE["G5 Voice<br/>WebRTC, Audio & DAVE MLS"]
  TL --> QUALITY["G4 QA & Reviewers<br/>Automation, Gates & 5/5 Mutations"]
  TL --> OPS["DevOps & G6 Tech Writer<br/>Packaging, Releases & Docs"]
  QUALITY -. "Review post-gate" .-> CORE
  QUALITY -. "Review post-gate" .-> CLIENT
  QUALITY -. "Review post-gate" .-> VOICE
```

Operational rule: Batches contain 3–6 cards with a frozen contract and unified gate; review executes post-gate in parallel; documentation synchronizes upon task acceptance.

---

## How to Read This Graph

- **Goals & Scenarios:** Scenario A (independent client with Discord UX) is recommended; Scenario C (direct gateway connection under user tokens) carries platform TOS risks.
- **Layers 1–8:** Systematic engineering layers detailing verified findings, clean-room dumps, and native Rust client architecture.
- **Layer 6 (References):** Architecture comparisons with external open-source projects (Vesktop and discord-qt).
- **Transfer Backlog:** Complete inventory of functional capabilities across UI shell, streaming, voice, and data contracts documented in [`docs/feature-backlog.md`](feature-backlog.md).
