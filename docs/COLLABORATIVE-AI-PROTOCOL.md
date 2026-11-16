# Multi-Human & Multi-AI Collaboration Protocol (AMCP v2.0)
## Agent Multi-Collaborator Protocol & Engineering Standard for `chord` / `discord-lab`

> **Document Status:** Active Engineering Standard  
> **Protocol Version:** AMCP v2.0 (Agent-to-Agent Multi-Collaborator Protocol)  
> **Scope:** All human engineers, AI agents (Antigravity, Claude Code, Cursor, Windsurf, Codex, Gemini CLI), autonomous workers, and automated bots (Dependabot).

---

## 1. Introduction & Architectural Principles

The development of the high-performance native desktop client (`chord` / `discord-lab`) is conducted by a distributed team: human engineers and autonomous AI agents powered by large language models (LLMs).

When multiple AI agents and human engineers collaborate on a single repository simultaneously, five critical risks arise:
1. **Merge Conflicts and File Race Conditions:** Two agents modifying the same file concurrently, overwriting each other's code.
2. **Context and Token Depletion on Routine Ops:** Agents burning up to 80% of their context budget on massive `git status`, `cargo test`, and raw JSON dumps.
3. **Data Contract Drift:** The backend (Rust) altering a DTO structure or method signature, silently breaking the frontend (TypeScript / Tauri).
4. **Memory Leaks and Host OOM Crashes:** Multiple concurrent `cargo` and `rustc` processes causing system out-of-memory crashes on host hardware.
5. **Breaking Dependency Drift:** Unmanaged external dependency updates breaking compiled code.

The **AMCP v2.0** protocol resolves these challenges through **strict engineering standards, a local agent coordination bus, and zero-token deterministic automations**.

---

## 2. Collaboration Architecture (Multi-AI Ecosystem)

```
┌────────────────────────────────────────────────────────────────────────┐
│                   HUMAN DEVELOPERS & LLM ASSISTANTS                    │
│   (Antigravity / Claude Code / Cursor / Windsurf / Codex / Gemini CLI)  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│             UNIFIED COLLABORATION CLI (scripts/collab.sh)              │
│       npm run collab status | lock | unlock | verify | sync | send     │
└──────────────┬────────────────────┬────────────────────┬───────────────┘
               │                    │                    │
               ▼                    ▼                    ▼
┌─────────────────────────┐ ┌───────────────┐ ┌──────────────────────────┐
│  DISTRIBUTED FILE LOCKS │ │  EVENT BUS    │ │     ASYNC MAILBOX        │
│  .collab/bus/locks.json │ │  events.jsonl │ │ .collab/bus/mailbox/<id> │
└─────────────────────────┘ └───────────────┘ └──────────────────────────┘
               │                    │                    │
               └────────────────────┼────────────────────┘
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                   PRE-FLIGHT GATES & GIT HOOKS                         │
│  • .git/hooks/pre-commit (Scope Audit + Sub-Second Syntax Linter)      │
│  • scripts/precheck.mjs (Parallel TS + Node + Meta Check in 0.7s)      │
│  • scripts/scope-audit.sh (Write-Scope Boundary Enforcement)           │
│  • scripts/tdd-runner.sh (TDD Red-Phase Failure & Green-Phase Record)  │
└───────────────────────────────────┬────────────────────────────────────┘
                                    │
                                    ▼
┌────────────────────────────────────────────────────────────────────────┐
│                       PR & MERGE CONVEYOR                              │
│  • scripts/dependabot-batch.sh (SemVer Safety Filter & Auto-Rebase)    │
│  • scripts/review-scaffold.sh (Dual Review: PM ACCEPT.md + Tech 5/5)   │
│  • scripts/worker-rotate.sh (1-to-1 Zero-Idle Slot Rotation, <=25GB)   │
│  • scripts/sync-push.sh (Atomic Dual-Remote Push to origin + public)   │
└────────────────────────────────────────────────────────────────────────┘
```

---

## 3. AMCP v2.0 Protocol Core

### §3.1. Contract-First Development
- **Rule:** Any change to an inter-module boundary (Rust BFF <-> Frontend WebView / DTOs / Events) begins with freezing the contract first.
- Modifying implementations before establishing the DTO in `rust/crates/discord-core/src/dto.rs` or signatures in `shared/contracts/` is strictly forbidden.
- Contract updates must be published to the event bus:
  ```bash
  npm run collab publish CONTRACT_UPDATE "Updated VoiceStateDto to include deaf_reason field"
  ```

### §3.2. Department Isolation and Write-Scope Boundaries
Each worker operates strictly within assigned department boundaries, as defined in `AGENTS.md` §2:
- **G0 (Routine & Adapters):** Documentation, utilities, meta-sync tests, boilerplate DTOs.
- **G1 (Backend Core & BFF):** `rust/crates/discord-{core,codec,media,store,i18n}`, `rust/gui-spike/src/*.rs`.
- **G2 (Frontend & UI):** `rust/gui-spike/ui/**/*.{js,css,html}`, `tests/unit/*.test.mjs`.
- **G3 (Design & Specs):** `docs/specs/*.md`, `waves/RW-*/DESIGN/*.md`.
- **G4 (QA Automation):** `tests/{meta,unit,smoke,e2e}/`, `scripts/test-impact.mjs`.
- **G5 (Voice & Platform):** `rust/crates/discord-voice/**`, `rust/crates/discord-platform/**`, WebRTC/DAVE.
- **G7 (Clean-Room RE):** Reference oracle analysis in `docs/re/`, specification generation without touching production code.

Before committing, write-scope boundaries are automatically validated:
```bash
npm run audit:scope
```
If a frontend worker (G2) modifies a backend file (G1), the commit is rejected.

### §3.3. Distributed File Locks
Before modifying a shared or critical file, an agent MUST acquire a lock:
```bash
npm run collab lock rust/crates/discord-core/src/dto.rs TASK-123
```
- The lock is recorded in `.collab/bus/locks.json`.
- If another agent attempts to modify the locked file, the operation is blocked:
  ```
  ❌ CONFLICT BLOCKED: File rust/crates/discord-core/src/dto.rs is already locked by @g1_backend for task TASK-110!
  ```
- Upon task completion and passing tests, the agent releases the lock:
  ```bash
  npm run collab unlock rust/crates/discord-core/src/dto.rs
  ```

### §3.4. Asynchronous Messaging (Direct Mailbox)
Inter-agent and engineer communication uses the mailbox mechanism:
```bash
# Send a notification
npm run collab send g2_frontend "IPC Ready" "Command voice_connect is implemented and tested"

# Read incoming messages
npm run collab inbox g2_frontend
```

### §3.5. Strict Host Memory Ceiling (`WORKER_RAM_LIMIT <= 25 GiB`) and Zero-Idle
- Total memory consumed across all active AI agents, child processes, `cargo`, and `node` runners must never exceed **25.00 GiB** (on a 48 GiB host).
- A maximum of **5 parallel worker slots** (3–4 dev/review + 1–2 RE).
- 1-to-1 slot rotation: A worker that finishes a task immediately releases its terminal (`worker:rotate`), and the freed slot receives the next queued task without idle time.

---

## 4. Verification, Contribution, and Merge Conveyor

### 4.1. Zero-Token Local Pre-Flight Checks
The repository enforces native Git hooks (`npm run hooks:install`):
1. **Pre-commit:**
   - Validates write-scope boundaries (`scripts/scope-audit.sh`).
   - Rejects modifications to frozen directories (`rust/chord-gpui/`).
   - Verifies JS syntax and TypeScript types in parallel (`scripts/precheck.mjs`).
   - Execution time: **0.7 seconds** (0 LLM tokens burned).
2. **Commit-msg:**
   - Validates Conventional Commits format (`feat:`, `fix:`, `refactor:`, `test:`, `docs:`, `chore:`, `perf:`, `ci:`).

### 4.2. Strict TDD Lifecycle
1. **RED Phase:** The agent writes an isolated contract test and verifies failure:
   ```bash
   npm run tdd:red -- --test my_contract
   ```
2. **GREEN Phase:** The agent writes minimal production code to satisfy the test:
   ```bash
   npm run tdd:green -- --test my_contract
   ```

### 4.3. Dual Review Gate
No PR merges into `unstable` without dual sign-off:
1. **Product Review (`ACCEPT.md`):** PM validates Definition of Done (DoD), UI journeys, and zero regressions.
2. **Technical Review (`REVIEW.md`):** Tech Lead checks architecture, thread safety, memory constraints, `#![forbid(unsafe_code)]`, and **100% mutation kill rate (5/5)**.
Scaffold generation is automated:
```bash
npm run review:scaffold TASK-123
```

### 4.4. Automated SemVer Bot Processing (Dependabot Conveyor)
Incoming bot PRs (`dependabot[bot]`) and external contributors are processed via `scripts/dependabot-batch.sh`:
- **PATCH Releases (0.14.3 -> 0.14.4):** Automatically approved and squash-merged once CI checks pass.
- **MINOR Releases (0.15.0 -> 0.16.0):** Evaluated with `scripts/precheck.mjs` and merged semi-automatically.
- **MAJOR Releases (0.15 -> 0.18):** Isolated, requiring Tech Lead review and `npm run test:smoke`.
- **`Cargo.lock` Collisions:** Handled automatically by triggering `@dependabot rebase`.

### 4.5. Defective PR Quarantine Protocol
If an external PR causes breaking regressions:
- The PR is moved to **Changes Requested** with detailed regression logs (`npm run pr:request-changes`).
- The branch is quarantined and does **not** block the main development pipeline.
- Useful sub-components can be salvaged cleanly via cherry-pick (`npm run pr:salvage`).

---

## 5. Complete Automation & CLI Registry

| Command | Script | Purpose | Token Savings / Advantage |
|---|---|---|---|
| `npm run collab status` | `scripts/collab.sh status` | Unified status: locks, events, workers, RAM | Replaces 6 manual commands in one click |
| `npm run collab lock` | `scripts/collab.sh lock` | Acquire exclusive file lock | Prevents concurrent overwrite conflicts |
| `npm run collab unlock` | `scripts/collab.sh unlock` | Release file lock | Keeps lock registry clean |
| `npm run collab verify` | `scripts/collab.sh verify` | Comprehensive pre-flight verification | 100% protection against broken commits |
| `npm run collab sync` | `scripts/sync-push.sh` | Atomic dual-push to origin & public remotes | Eliminates remote mirror desync |
| `npm run precheck` | `scripts/precheck.mjs` | Parallel TS + Node + Meta checker | 0.7 s instead of 12 s `npm test` |
| `npm run audit:scope` | `scripts/scope-audit.sh` | AGENTS.md §2 department boundary linter | Protects against accidental cross-dept edits |
| `npm run report:worker` | `scripts/worker-report.sh` | PROCESS.md §4.2 report generator with SHA256 | -1500 tokens on manual markdown crafting |
| `npm run hooks:install` | `scripts/install-git-hooks.sh` | Installs Git pre-commit & commit-msg hooks | Automated enforcement at Git level |
| `npm run wave:accept` | `scripts/wave-accept.sh` | Records completed task in waves/db.jsonl | Enforces v1 wave journal schema |
| `npm run review:scaffold`| `scripts/review-scaffold.sh` | Scaffolds ACCEPT.md and REVIEW.md (5/5) | Standardizes dual review process |
| `npm run tdd:red` | `scripts/tdd-runner.sh --red` | Records red-phase test failure | Enforces strict test-first development |
| `npm run tdd:green` | `scripts/tdd-runner.sh --green`| Verifies green-phase pass and mutation gate | Automates TDD green phase |
| `npm run orca:status` | `scripts/orca-slim.sh` | Compact Orca worker status | Compresses 8000 tokens to 30 (-99.5%) |
| `npm run gc` | `scripts/worker-gc.mjs` | Cleans target cache & stale worktrees | Frees up to 105 GB disk in 1 s |
| `npm run dependabot:batch`| `scripts/dependabot-batch.sh` | SemVer filter & auto-merge for bot PRs | Full autonomy for dependency updates |
| `npm run rust:slim` | `scripts/rust-slim.sh` | Compresses Rust compiler error output | Compresses 400 lines to 3 lines (-95%) |
| `npm run contract:lint`| `scripts/contract-lint.mjs` | Rust DTO ↔ UI contract drift detector | Prevents silent UI mismatches |
| `npm run kanban:sync` | `scripts/kanban-sync.sh` | Zero-touch Kanban board synchronization | Auto-moves cards from git history |
| `npm run pr:quarantine`| `scripts/pr-quarantine.sh` | Quarantines defective external PRs | Isolates breaking branches automatically |
| `npm run test:visual` | `scripts/visual-diff.mjs` | Headless visual regression comparison | Evaluates UI shift (<0.2%) for PM sign-off |
| `npm run security:shield`| `scripts/security-shield.mjs` | AST scanner for keys, tokens & unsafe code | Pre-commit protection against secret leaks |

---

## 6. Developer & AI Quick Reference

1. **Before Starting Work:**
   ```bash
   npm run collab status
   npm run collab lock <file_path> <task_id>
   ```
2. **During Development:**
   - Write tests first (`npm run tdd:red`).
   - Adhere strictly to department scopes (`AGENTS.md`).
   - Use verified contracts and clean-room references (`docs/re/`).
3. **Before Committing:**
   ```bash
   npm run collab verify
   npm run collab unlock <file_path>
   npm run report:worker
   ```
4. **Synchronizing with the Team:**
   ```bash
   npm run collab publish TASK_DONE "Completed TASK-XXX: Description"
   npm run collab sync
   ```

By following this protocol, any combination of human engineers and autonomous AI agents can advance the repository concurrently without risking architectural drift or system instability.
