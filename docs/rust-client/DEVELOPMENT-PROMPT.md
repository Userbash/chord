# DEVELOPMENT-PROMPT — Rust Native Client Development Guide (discord-lab)

> **Status:** Accepted waves RW-01…RW-21 (25/90); Batch B02 (P2-08 settings, P1-01 ETF, P1-02 zstd, P0-04 conformance, P0-07 legal) accepted; next batch B03/RW-22; integrated batch pipeline active (`TEAM.md`, `WAVE-PLAN.md`).
> Operational record: `STATUS.md`, strategic roadmap: `ROADMAP.md` / `WAVE-PLAN.md`, audit ledger: `waves/STATE.md`.

**Purpose:** Comprehensive operational prompt for AI coordinators and worker agents developing the Rust client in this repository. In case of contractual ambiguity, authority follows: `PROCESS.md` -> `docs/rust-client/PLAN.md` / `TEST-PLAN.md` -> `TASKS.md` -> `AGENTS.md`.
**Core Invariant:** Zero invented fields, endpoints, opcodes, or UI screens. Every data model, type, and test is grounded in cleanroom RE documentation (`docs/re/`), curated fixtures (`fixtures/`), or explicitly annotated as synthetic.

---

## 0. Prerequisite Context Check

Execute prior to authoring production code:

```sh
cd <project-root>
git status --short
git diff --stat
sed -n '1,240p' PROCESS.md
sed -n '1,220p' docs/rust-client/TEAM.md
sed -n '1,220p' docs/rust-client/WAVE-PLAN.md
sed -n '1,180p' docs/rust-client/PLAN.md
sed -n '1,300p' docs/rust-client/TEST-PLAN.md
sed -n '1,220p' docs/rust-client/ARCHITECTURE.md
cat waves/STATE.md
jq -r '[.ts,.event,.wave,.card,(.summary//"")]|@tsv' waves/db.jsonl
```

Workers execute only targeted tests for their assigned module; full workspace runs and clippy checks occur in the integration/QA lane:

```sh
cd <project-root>/rust
cargo test --offline -p discord-core --test <assigned_test>
# Full workspace verification (integration lane only):
cargo test --offline --workspace
cargo fmt --check
cargo clippy --offline --workspace --all-targets -- -D warnings
```

Final batch quality gates are executed exclusively by the Integration / Team Lead:

```sh
cd <project-root>
npm run gate:worker
```

---

## 1. Product Objectives and Operational Boundaries

Deliver a high-performance native desktop Discord-compatible client in Rust:

- Zero Electron and zero bundled Chromium.
- Network I/O, Gateway WebSockets, REST, media streaming, and CPU-intensive parsing remain off the main UI rendering thread.
- State management operates via strongly typed events and pure reducers rather than direct UI mutations from network callbacks.
- All wire behaviors and data schemas are verified against cleanroom RE documentation and fixtures.
- Unverified behaviors are strictly isolated behind interfaces, mock stubs, or feature flags.

### Strict Prohibitions

- Do not issue live requests to production Discord endpoints, do not attach or spawn live Discord binaries, and do not use Frida/CDP without explicit authorization.
- Never extract, log, or persist user authentication tokens, cookies, passwords, session IDs, `analytics_token`, or `qos_token`.
- Directories `logs/`, `functions/`, `evidence/`, and `clean-room-oracle/` are strictly read-only.
- Do not install third-party packages or modify `Cargo.toml` / `Cargo.lock` / `package.json` without explicit approval.
- Do not execute git commits, pushes, resets, or destructive VCS operations from worker agents.
- Never edit peer files, shared core files, or wave audit ledgers without explicit assignment.
- Never modify test assertions to match incorrect implementation.

---

## 2. Core Crates Architecture

### 2.1 Workspace Crates

| Crate | Architectural Role | Current Status |
|---|---|---|
| `discord-codec` | ETF encoder/decoder (11 tags, big-int), streaming zstd | Production-grade, accepted in RW-01 / RW-02 |
| `discord-core` | Gateway WS, heartbeat, IDENTIFY, READY ingestion, RESUME, dispatch router, REST v9 client, rate limiting, telemetry blocking, ACK | Core foundation crate; READY parser accepted |
| `discord-conformance` | L2 fixture loader and byte-exact test harness | Active, expanding per TEST-PLAN |
| `discord-store` | Strongly typed state container and pure reducer (P2-02) | Active, SQLite migrations |
| `discord-ui` | GUI presentation layer, components, virtualization | GPUI native 120 FPS application |
| `discord-app` | Composition root, window bootstrapping | Production binary root |
| `discord-platform` | OS integrations, Keyring credential vault, system tray | Active, D-Bus and secret storage |
| `discord-voice` | Voice signaling, RTP packetizer, SRTP encryption | Active, WebRTC UDP engine |
| `discord-media` | Audio ring buffer, video pipelines | Active |
| `discord-i18n` | 36 language catalogs, xxh64 translation key hashing | Active |

### 2.2 Threading and Concurrency Architecture

```text
UI Thread (GPUI / WGPU Engine)
  ├─ Reads immutable state snapshots
  ├─ Dispatches typed UiCommand intents
  └─ Zero blocking I/O, zero payload parsing, zero SQLite queries

Core Runtime (Tokio)
  ├─ GatewaySession / RestClient / Media / Voice
  ├─ Bounded MPSC channels -> state reducer
  ├─ Cancellation tokens per window and session
  └─ Explicit timeouts, reconnection backoff, rate limit handling

State Reducer / Store
  ├─ Pure (State, Event) -> (State, Effects) state machine
  ├─ Indices and member joins
  └─ Redacted UI state projection (zero secrets)

Worker Threadpool (Rayon)
  └─ Heavy ETF / JSON / Protobuf decoding, audio mixing, cryptography
```

---

## 3. Cleanroom Reverse-Engineering Sources

### 3.1 Primary Specifications

| Specification | Technical Scope |
|---|---|
| `docs/re/README.md` | Cleanroom methodology index, coverage, verification bounds |
| `docs/re/gateway-protocol.md` | Opcodes, ETF serialization, heartbeat timing, IDENTIFY, READY, RESUME |
| `docs/re/gateway-schema.md` | Dispatch event payload schemas, READY structure, entity types |
| `docs/re/rest-d02.md` | REST v9 endpoints, 47 routes, auth headers, `X-Super-Properties` |
| `docs/re/frontend-d03.md` | UI design tokens, component contracts, modal specifications |
| `docs/re/voice-deep.md` | Voice signaling gateway, RTP/SRTP wire layout, audio codecs |
| `docs/re/i18n-deep.md` | Translation key hashing, CLDR pluralization rules |
| `docs/re/backend-ipc.md` | Main process IPC channels, native platform bridges |

### 3.2 Reference Oracle Artifacts (`clean-room-oracle/`)

| Path | Purpose |
|---|---|
| `clean-room-oracle/capture-ready.jsonl` | Real READY payload captures (2.4 MB) |
| `clean-room-oracle/gw-proto/msgs-*.jsonl` | Decoded Gateway message streams across 21 dispatch types |
| `clean-room-oracle/d02-snapshot/` | REST request/response captures (`X-Super-Properties`, etc.) |
| `clean-room-oracle/i18n-stats.json` | Key hashing verification vectors |
| `clean-room-oracle/voice/` | Voice UDP captures, RTCP packets, socket rotation timings |

Curated test fixtures reside in `fixtures/` with integrity verified by `fixtures/MANIFEST.json`.

---

## 4. Strict Test-Driven Development (TDD) Protocol

### 4.1 Red Phase Execution

```sh
cd <project-root>/rust
cargo test --offline -p discord-core --test <test_file> -- --exact <test_name> 2>&1 | tee <project-root>/waves/<ID>/reports/<test_name>-red.log
```

Red Phase Invariants:
- The test must fail prior to authoring production implementation.
- The failure must occur due to the expected behavioral assertion, not syntax or typing typos.
- Command, non-zero exit code (`# EXIT=101`), and test checksum are recorded in the report.

### 4.2 Minimal Green Implementation

- Edit strictly the files assigned in the task specification.
- Author the minimal implementation required to make the test pass cleanly.
- Never swallow errors silently; provide structured error variants or explicit counters.
- Bound all inputs by size, depth, and count to prevent panics and memory exhaustion.

### 4.3 Green Phase Verification

```sh
cd <project-root>/rust
cargo test --offline -p discord-core --test <test_file>
cargo test --offline
cargo fmt --check
cargo clippy --offline --workspace --all-targets -- -D warnings
```

---

## 5. Specialist Roles for Integrated Waves

During each active wave, specialist responsibilities are assigned across:

| Role | Access Mode | Primary Responsibility |
|---|---|---|
| Protocol Researcher | Read-only | Verifies types, fields, opcodes, and schemas against cleanroom oracle |
| Test Strategist | Read-only | Formulates minimal red-first test cases, boundary assertions, and mutations |
| Code Reviewer | Read-only | Audits Rust ownership, race conditions, cancellation tokens, error bounds |
| Concurrency Auditor | Read-only | Ensures zero blocking calls on the UI thread, audits queue bounds |
| Feature Worker | Workspace-write | Implements green code within assigned non-overlapping file scopes |

---

## 6. Definition of Done (DoD) per Task Card

- [ ] Red failure log captured with exact command, non-zero exit code, and test checksums.
- [ ] Minimal implementation authored; test assertions untouched.
- [ ] Targeted module test suites pass cleanly (green).
- [ ] Workspace compiles cleanly under `cargo fmt --check` and `clippy -D warnings`.
- [ ] Mutation testing budget satisfied (100% killed on critical paths); source restored.
- [ ] Unified batch quality gate passes (`npm run gate:worker`).
- [ ] Non-overlapping file write-scopes respected; zero shared core edits.
- [ ] Independent code review completed; all P0/P1/P2 findings resolved test-first.
- [ ] Task report, test corrections, and audit ledger updated in `waves/db.jsonl`.
- [ ] Zero credentials or sensitive user data in code, logs, or test fixtures.
