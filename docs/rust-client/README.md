# Discord Rust Client: Concept, Architecture, Plan, Tests

This section prepares the development of a **full-featured Discord client in Rust** — a high-performance native backend wrapped in a modern GUI framework (without Electron and without bundled Chromium). Everything is built on cleanroom reverse-engineering data from wave R7 (`../re/` — gateway, REST, voice, UI, i18n, backend) and prior waves (`../feature-backlog.md`, R4/R5/R6/D).

## Package Contents

| Document | Description |
|---|---|
| `STATUS.md` | **Actual State:** 25/90 accepted, 580 tests, crates, blockers, evidence |
| `ROADMAP.md` | **Strategic Plan:** batch waves, milestones, and architectural decisions |
| `TEAM.md` | **Engineering Department:** seats, RACI, pipeline, WIP, worker playbook, KPI |
| `../archive/rust-client/WAVE-PLAN.md` | **Batch Plan (Archived):** historical 90-card wave schedule |
| `CONCEPT.md` | Complete concept: product scope, principles, stack, user scenarios, success metrics |
| `FEATURES.md` | Feature catalog (ID, purpose, RE behavior, source, phase) — 14 groups |
| `ARCHITECTURE.md` | Target architecture, actual Cargo dependency graph, storage, Electron IPC, UI ADR |
| `PLAN.md` | Waves P0–P8: 90 task cards (historical wave plan, retained for `rust-client-sync` meta-test) |
| `TEST-PLAN.md` | Levels L0–L7, RE fixture manifest, ~130 test catalog, red-first protocol |
| `MUTATION-LEDGER.md` | Mutation testing ledger: wave summary, mutant classes, survivor triage |
| `SECURITY.md` | Threat model, trust boundaries, secrets, FFI, supply chain, security tests |
| `ADR-1.md` | GUI toolkit decision: Tauri spike, FPS/RAM/build/startup metrics, preconditions |
| `CODE-STANDARD.md` | **Engineering Standard:** research -> frozen contract -> red-first -> mutation/review -> gate; security, async/UI, Definition of Done |
| `LEGAL.md` | **Legal Framework:** ToS, telemetry disabled, honest user agent, secrets/assets, live actions require approval |
| `../archive/rust-client/CODEX-WORKERS.md` | **Worker Playbook (Archived):** historical worker/reviewer prompts and DAG |
| `PROJECT-STRUCTURE.md` | Unified project layout: laboratory <-> repository <-> `fixtures/` <-> `rust/`, file organization rules |
| `TRACEABILITY.md` | Features -> specifications -> fixtures -> tests: 14 group traceability matrix (141 F / 133 T) |
| `VERIFICATION.md` | Raw evidence verification log against documentation: 20/20 checks, discrepancy analysis |
| `../archive/rust-client/DEVELOPMENT-PROMPT.md` | **Coordinator Prompt (Archived):** historical coordinator/worker prompt |

## Reading Guide

0. **Current status** -> `STATUS.md`; **what to do next** -> `ROADMAP.md`.
1. **Product scope** -> `CONCEPT.md` (§2 "Product Capabilities", §3 Scenarios).
2. **Features** -> `FEATURES.md` (filter by phase: F-* with "Phase" column).
3. **Architecture** -> `ARCHITECTURE.md` (graphs 1–7).
4. **Planning** -> `PLAN.md` (wave map and cards P0-01...P8-05).
5. **Test-first implementation** -> `TEST-PLAN.md` §3 (test IDs for each card).
6. **Starting a wave (Archived)** -> `TEAM.md` and `../archive/rust-client/WAVE-PLAN.md`; `../archive/rust-client/DEVELOPMENT-PROMPT.md` for historical context.
7. **File organization** -> `PROJECT-STRUCTURE.md`; **verify feature to test** -> `TRACEABILITY.md`; **data validation** -> `VERIFICATION.md` + `node verify/verify-re.mjs`.

## Sources (in this repository)

- `../re/README.md` — Wave R7 index: methodology, coverage, fidelity boundaries.
- `../re/gateway-protocol.md`, `../re/rest-d02.md`, `../re/voice-deep.md` — protocols.
- `../re/frontend-d03.md` — UI contracts; `../re/i18n-deep.md` — localization.
- `../re/main-arch.md`, `../re/backend-ipc.md`, `../re/native-re.md` — backend and native modules.
- `../re/architecture.md` — Discord internal architecture graphs.

## Raw Artifacts

Reference cleanroom oracle: `clean-room-oracle/` (1.7 GB, 22,072 files): gateway/REST/UI captures, voice pcap and strace, i18n catalogs, extracted asar packages. **Curated conformance subset** for automated tests — `../../fixtures/` (12 entries, 1.59 MB, `MANIFEST.json` with sha256 checksums; full manifest in `TEST-PLAN.md` §2). All credentials and tokens in documentation and fixtures are replaced with `REDACTED` placeholders.

## Harness and Verification

- `../../rust/` — Cargo workspace (13 crates, offline): `cargo test --offline --workspace` (all suites green). Waves: RW-01 — ETF codec (`discord-codec`), RW-02 — zstd stream decoder (`ruzstd`, 64 KiB window), RW-03 — gateway session (`discord-core::gateway`: HELLO/heartbeat 41,250/ACK/RTT/zombie detection), RW-04 — WS transport (`ws`: ws:// + wss://, zstd -> term -> session pipeline), RW-05 — IDENTIFY/READY summary, RW-06 — RESUME/reconnect (op6, 3s/30s, op9 `d`), RW-07 — 21 dispatch router, RW-08 — REST catalog/headers/retries, RW-09 — X-Super-Properties, RW-10 — HTTP/1.1/TLS transport, RW-11 — rate limiting/telemetry/ack, RW-12 — I1 REST facade and P2-01 READY (A1); RW-13 — P2-02 state/reducer; RW-14 — P2-06 history; RW-15 — P2-04 member list; RW-16 — P2-05 chunks; RW-17 — P2-07 read-state; RW-18 — P2-09 presence; RW-19/RW-20 — P2-10/P2-11; RW-21 — P2-08/P1-01/P1-02/P0-04/P0-07. Status tracked in `STATUS.md`, `TASKS.md`, `waves/STATE.md`.
- `verify/verify-re.mjs` — Raw evidence vs documentation oracle (20 checks; exit codes 0/1/2), report in `verify/report.json`.
- `../../fixtures/MANIFEST.json` — Fixture provenance (source, md5, T-ID).

## Status

- Documentation: **Complete**; `rust/` harness and `fixtures/` assembled; raw evidence verification **20/20** (`verify/report.json`).
- Input risks and RE gaps are documented in `CONCEPT.md` §7; open questions in `../feature-backlog.md`.
- Document synchronization is guarded by meta-test `../../tests/meta/rust-client-sync.test.mjs`.
