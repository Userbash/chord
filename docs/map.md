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
  classDef amcp fill:#f0e6ff,stroke:#8a2be2,color:#1b2430

  GOAL["Native Discord Client & Protocol Suite (Chord)"]:::neu
  A["Scenario A — Independent Messenger (Recommended)"]:::ok
  C["Scenario C — Direct Protocol Client (TOS Sensitive)"]:::no
  GOAL --> A
  GOAL --> C
  GOAL --> S8
  A --> LAYERS

  subgraph AMCP["Multi-AI Collaboration Control Plane (AMCP v2.0)"]
    bus["Agent Bus (.collab/bus/events.jsonl)"]:::amcp
    locks["Distributed File Locks (locks.json)"]:::amcp
    precheck["Sub-Second Precheck (precheck.mjs)"]:::amcp
    shield["AST Security Shield (security-shield.mjs)"]:::amcp
    contracts["Contract Drift Linter (contract-lint.mjs)"]:::amcp
    bus --- locks
    locks --- precheck
    precheck --- shield
    shield --- contracts
  end

  subgraph LAYERS["System Architecture Layers (Waves R1–R7, RW-52)"]
    direction TB
    subgraph S1["1. UI & Visual Engine"]
      u1["Tokens: 5,132 CSS variables"]:::ok
      u2["Themes: Dark / Light / Midnight"]:::ok
      u3["Views: Server Rail, Virtual Chat, Voice HUD"]:::ok
      u4["Localization: 36 Locales (chord-i18n)"]:::ok
    end

    subgraph S2["2. Protocol Core (chord-core)"]
      p1["Gateway v9: WebSocket + ETF + zstd stream"]:::ok
      p2["REST API v9: Discord endpoints"]:::ok
      p3["Permissions: 64-bit Bitmask Calculator"]:::ok
      p4["Heartbeat & Voice Gateway OP 0..43"]:::ok
    end

    subgraph S3["3. Audio & Media Engine (chord-voice / chord-platform)"]
      m1["Audio: cpal native ALSA/Pulse/PipeWire 48kHz"]:::ok
      m2["SRTP: AES-256-GCM and ChaCha20-Poly1305"]:::ok
      m3["Screen Sharing: PipeWire Portal + X11 SHM"]:::ok
      m4["DAVE MLS v1: RFC 9420 KeyRatchet E2EE"]:::ok
    end

    subgraph S6["6. Reference Implementations"]
      ref1["Vesktop 1.6.7 (Screen sharing & packaging)"]:::ok
      ref2["discord-qt 0.5.1 (Protocol contracts & schemas)"]:::ok
    end

    subgraph S8["8. Native Client Implementation (docs/rust-client/)"]
      rc1["CONCEPT.md: Zero Electron / Zero WebKit lag"]:::ok
      rc2["FEATURES.md: >= 100 features mapped"]:::ok
      rc3["ARCHITECTURE.md: Tauri v2 BFF & High-FPS WGPU"]:::ok
      rc4["PLAN.md: Waves P0–P8"]:::ok
      rc5["TEST-PLAN.md: 12 Quality Tiers (T0–T4)"]:::ok
      rc6["PROJECT-STRUCTURE.md: Workspace Crates"]:::ok
      rc7["TRACEABILITY.md: F-* to T-* mapping"]:::ok
      rc8["VERIFICATION.md: Evidence validation"]:::ok
    end
    bkl["Transfer Backlog"]:::ok
  end

  ref1 --> bkl
  ref2 --> bkl
  bkl --> S8
  AMCP --> LAYERS
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
