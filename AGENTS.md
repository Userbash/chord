# AGENTS.md — Agent and Coordinator Rules for discord-lab

For quick onboarding, first read `PROJECT-CONTEXT.md`; open full documents only as needed for a specific task card. Do not load entire sprint histories or massive wave plans without direct need.

You are working in the `discord-lab` repository (local CLI: Discord protocol analysis via Frida/CDP + native Rust GPUI client `discord-gpui`).
Before starting work, check HEAD and the status of your assigned files. Then review only the required excerpts:
your specification, your task card row from `TASKS.md`, the contract sections of `TDD-PLAN.md`,
relevant rules in `PROCESS.md` / `TEAM.md` / `TEST-POLICY.md`, and the current state summary in `waves/STATE.md`.
`PROCESS.md` is the source of truth for process; `TEAM.md` for role distribution, model routing, and worker parallelism; `TEST-POLICY.md` (and `waves/RW-40/QUALITY/TEST-STRATEGY-V2.md`) for testing strategy.

---

## 1. Core Development Rules (TDD, Files, Gates)

1. **Strict TDD.** Test first (red), execute and record failure with a command. Then write minimal code (green). No production code is written without a test. Tests verify the contract against an independent oracle (real artifact, jq/awk/md5), never against the current implementation.
2. **Only Assigned Files** from your task spec. Worker file scopes must never overlap. Do not touch others' tests, modules, documentation, or §8 without Team Lead approval. Shared files (`state/types.rs`, `state/reducer.rs`, `state/mod.rs`, `src/lib.rs`, `PLAN.md`, `STATUS.md`, `ROADMAP.md`, `TASKS.md`, `docs/map.md`, `waves/*`) are edited solely by the Team Lead / Tech Lead.
3. **Level-Appropriate Gate.** In batch mode, a worker runs only targeted tests for their module (`T0`: `node scripts/test-impact.mjs` / targeted `--test <mod>_contract`) and preserves red/green logs. Full `cargo fmt`/`clippy`/`test --workspace`/`npm run gate:worker` (`T1`) runs once per batch in the integration lane. When working solo (outside a batch), full `gate:worker` is required. If e2e fails, the gate retries automatically and records both attempts; flaky tests must never be silenced.
4. **Report Format** matching `PROCESS.md` §4.2: batch/card, completed work, red run, green run, targeted, gate, files+md5, risks. A task is closed upon acceptance by Team Lead + PM + Technical Reviewer, not merely by filing a report.
5. **Questions Go to Team Lead** (orchestration/ask), never guess. If a spec is ambiguous, stop and ask. If a file is locked by another worker, stop — do not attempt to resolve via concurrent edits.
6. **Wave Journal.** Batch artifacts (spec, red/green logs, report, test copies) are stored in `waves/<ID>/`; upon acceptance, Team Lead appends to `waves/db.jsonl` (append-only, v1 schema events) and updates `waves/STATE.md`. The journal is guarded by the `waves-sync` meta-test.
7. **Docs Updated Post-Acceptance.** Do not update `STATUS`/`ROADMAP`/`TASKS`/`map`/`STATE` in the red phase; Team Lead synchronizes them once after batch acceptance.

---

## 2. Team Organization, Departments, and Zero-Idle Policy

1. **Independent Specialist Departments:**
   - **G1 Backend (Rust Core & Native BFF):** `rust/crates/*`, `rust/discord-gpui/src/*.rs`, `rust/discord-gpui/tests/*_contract.rs`.
   - **G2 Frontend (UI Views & A11y):** `rust/discord-gpui/src/views/**`, `tests/unit/*.test.mjs`.
   - **G3 Design (UX/UI & Specs):** `docs/specs/*.md`, `waves/RW-*/DESIGN/*.md`, CSS tokens and breakpoints.
   - **G4 QA (Test Automation & Verification):** Centralized testing of `testing` branch builds, `scripts/test-impact.mjs`, `tests/{unit,smoke,e2e,meta}/`, reports `QA-BUILD-*.md`.
   - **G5 Voice / Platform:** `discord-voice`, `discord-platform`, audio/streaming opcodes `18..=22`.
   - **G6 Tech Writer / Parity:** `MERGE-READINESS-*.md`, `RELEASE-NOTES-*.md`, parity matrices, and documentation.
   - **G7 Reverse Engineering & Parity Discovery (`G7 RE / REA`):** Continuous discovery and comparison against clean-room oracle `docs/re/` and running Discord sessions via `rea` tools (`reverse-engineer-anything`), documenting gaps in `waves/RW-*/RE-AUDIT/RE-GAP-*.md` and filing modernization cards for the Product Manager (PM) in `waves/RW-*/PM/BACKLOG-RE-*.md`.
   - **DevOps / Integration:** `scripts/{release.sh,ci.sh,gate.mjs}`, `SHA256SUMS`, SBOM, release pipeline `unstable -> testing -> beta -> main`.
   - **Product Manager (PM) & Technical Reviewer:** Acceptance of RE findings into development backlogs, and dual-review acceptance of every PR (`ACCEPT.md` + `REVIEW.md` with 5/5 mutation gate).
2. **Strict Separation of Concerns:** Each worker is responsible solely for their assigned task. Reallocation of tasks occurs only through coordinator assignment.
3. **Zero-Idle Policy:**
   - No department or worker may remain idle waiting for adjacent teams when independent tasks, code reviews, defect fixes, or contract/test definitions are available in the backlog.
   - When a worker finishes a task (`worker_done`), the coordinator immediately routes the result to review and delegates the next queue item to the freed worker in a fresh worktree.
   - Never run multiple concurrent `cargo` builds against the same `target-dir` (use shared `CARGO_TARGET_DIR=./rust/target` sequentially or in isolation).

---

## 3. Agent Routing and Model Policy (Orca Routing & Cost Policy)

1. **Worker Spawning via Orca Orchestration:**
   - Workers run as supervised terminal processes in dedicated worktrees via `orca worktree create --name <name> --base-branch unstable --branch <type>/task-<id>-<slug>` and `orca orchestration worker-start --run <run_id> --worktree branch:<type>/task-<id>-<slug> ...`.
2. **Model Selection Policy:**
   - **`gemini-4-carbon-high` (`--effort high` / `xhigh`):** Reserved for the most demanding core P0 challenges (zero-copy ring buffers, DAVE MLS v1 cryptography, AppState Tokio<->GPUI async sync).
   - **`gemini-4-argon-high` / `medium`:** Standard for validation, code reviews, mutation testing (QA / DevOps verification), and standard BFF REST services.
   - **`gemini-3.8-flash-high` / `medium` / `low`:** Used for UI layout, modal dialogues, preferences, tray integrations, and documentation sync.

---

## 4. Branch Hierarchy, Pull Requests, and 13 Task Statuses

### 4.1. Branch Hierarchy (`worker branch -> unstable -> testing -> beta -> main`)
1. **Worker Branch:** Created strictly from current `unstable` for each task. Branch naming: `<type>/task-<id>-<short-slug>`, for example:
   - `feature/task-123-reactions-extended-be`
   - `bugfix/task-109-ipc-oracle-strict`
   - `design/task-114-reactions-picker-spec`
   - `qa/task-125-test-impact-v2`
   - `devops/task-119-fix-release-sha256`
   - `docs/task-121-parity-wave11`
2. **Local Verification and PR to `unstable`:** Upon completion, the worker runs local tests, build, linter, and typecheck, compiles a report, and submits a PR to `unstable`. Workers are never permitted to merge directly into shared or protected branches.
3. **Dual Review (PM + Technical Reviewer):**
   - **Product Manager (PM)** validates results against specifications, user journeys, mockups, and DoD.
   - **Technical Reviewer** checks architecture, memory safety, code quality, test rigor, error handling, backward compatibility, and kills **5/5 mutations**.
   - Merging into `unstable` requires green CI, PM approval, Technical Reviewer sign-off, zero blocking defects (`P0=0, P1=0`), and clean conflict resolution.
4. **Branch Cleanup:** Immediately after merging into `unstable`, the worker branch is deleted.
5. **Branch `testing` (Centralized QA Integration):**
   - Individual worker changes are never merged into QA directly from task branches. Direct commits to `testing` are forbidden.
   - Integration lead compiles verified tasks from `unstable` into `testing` and issues a `BUILD-00X` manifest.
6. **Branch `beta` (Pre-Release Validation):** Following `QA Passed` status in `testing`, the build advances to `beta` for final environment validation, soak testing, performance verification (`perf_contract`), and `SHA256SUMS`/SBOM generation.
7. **Branch `main` (Stable Release) and `hotfix/*`:**
   - Direct commits to `main` are strictly forbidden. Following successful `beta` qualification, a PR merges `beta -> main`, a release version tag is created, and changes are backported to `unstable`.
   - Emergency hotfixes follow `hotfix/task-<id>-<slug>` with mandatory tests, dual review, and immediate multi-branch sync.

### 4.2. Mandatory 13 Task Statuses
Every task follows this sequential lifecycle without skipping stages:
`Backlog` → `Ready for Development` → `In Progress` → `Ready for Product Review` → `Ready for Technical Review` → `Ready for Merge` → `Merged to Unstable` → `In Testing` → (`QA Failed` | `QA Passed`) → `In Beta` → `Ready for Release` → `Released`.

---

## 5. Testing Strategy (12 Tiers, `P0–P3`, `T0–T4`, 9 Dimensions)

Detailed specification: `TEST-POLICY.md` (§11–§15) and `waves/RW-40/QUALITY/TEST-STRATEGY-V2.md`.
- **No Severity Downgrades:** Existing tests (`147 meta`, `879+ unit`, `1142+ workspace`, `50 gpui e2e contract`, `22 smoke`) must never be demoted in priority (`P0/P1`).
- **12 Tiers:** `Unit`, `Integration`, `Contract`, `API` (REST mock `200..5xx`), `E2E`, `UI`, `Visual`, `Accessibility` (WCAG/ARIA/Keyboard), `Performance`, `Security` (Keyring, XSS, SRTP, SBOM/`SHA256SUMS`), `Regression`, `Smoke`.
- **9 Mandatory Dimensions per Slice:** Positive, negative, edge boundaries (Snowflake `0`/`u64::MAX`, 2000 chars, 10 files/25 MB, volume `0..200`), error handling (`401/403/404/413/429/5xx`), empty states, loading skeletons, capability permissions, recovery/fallback, and observable telemetry.
- **CI Quality Thresholds:** `Pass P0/P1 = 100%`, `lines >= 90%`, `branches >= 90%`, `functions >= 80%`, `mutations = 5/5 (100%)`, `flaky_rate = 0%`.

---

## 6. Prohibitions and Engineering Conventions

### Prohibitions
- `logs/`, `functions/`, `evidence/` are golden artifacts: **read-only**. No writing or renaming.
- No direct `git commit/push` from worker terminals into shared branches.
- No arbitrary network calls or package installations without prior coordination.
- Never green tests by modifying tests to fit buggy code. Weakening assertions is treated as a P0 blocker (`TEST-POLICY.md`).

### Conventions
- Modern Rust 2021 edition, `#![forbid(unsafe_code)]`, strict typing.
- CLI diagnostics and logs in English; exit codes: `0` success, `1` argument error, `2` environment unavailable.
- Tests must be skip-aware: missing external dependencies yield an explicit skip with justification (in `gate:release`, `failOnSkips: true`).
- Never swallow errors silently: return explicit Results or log appropriate contextual errors.

---

## 7. Clean-Room Oracle Verification (`docs/re/`) and REA Tools

1. **No Guesswork in Implementation or Specification:**
   - If exact behavior, payload formats, constants, limits, i18n keys, or voice opcodes are unknown, **never invent behavior**.
   - Consult the clean-room reverse-engineering references in **`docs/re/`** or use **`rea`** tools (`reverse-engineer-anything`) against clean-room artifacts.
2. **Clean-Room Oracle Map in `docs/re/`:**
   - `gateway-protocol.md`, `gateway-schema.md`: Gateway protocol traces, ETF big-int handling, and zstd stream specs.
   - `rest-d02.md`, `rest-static.md`: REST API v9 routes, headers, rate limits, and error payloads.
   - `frontend-d03.md`: UI component contracts, CSS tokens, emoji picker, settings, and voice panel behaviors.
   - `main-arch.md`, `backend-ipc.md`, `native-re.md`, `voice-deep.md`: RTC signaling, RTP/RTCP handling, and crypto modes.
   - `i18n-deep.md`: Localization key hashes and string catalogs.
3. **REA Tool Integration (`reverse-engineer-anything`):**
   - Static analysis: `analyze_javascript_application`, `trace_javascript_semantics`, `trace_application_feature`, `trace_web_module_imports`.
   - Native binaries: `open_binary`, `binary_overview`, `list_procedures`, `search_procedures`, `analyze_function`, `search_strings`.
   - Every identified function, constant, or wire contract must be documented with an exact reference to `docs/re/`.

---

## 8. Worker Memory Ceiling (`WORKER_RAM_LIMIT <= 25 GB`), OOM Prevention, and Fixed Pool

1. **Strict RAM Ceiling Across All Workers — up to 25 GB RAM (`WORKER_RAM_LIMIT = 25 GB`):**
   - The host system has 48 GB RAM (`Swap: 0B`). Total memory usage across all active workers and child processes (`cargo`, `rustc`, `node`, `rea`) must never exceed **25 GB RAM**, leaving at least 20–23 GB for the OS and base environment.
   - To guarantee safety against OOM Killer, development uses a fixed pool of **5–6 active worker slots** (`3–4` dev/review/QA slots + `1–2` read-only RE slots).
   - Zero-Idle operates by **continuous 1-to-1 slot rotation**: once a worker finishes (`worker_done`), its terminal is released (`worker-release`) and the slot immediately receives the next task from the backlog.
2. **Mandatory Terminal Release (`worker-release`) and RSS Monitoring:**
   - Before launching a new worker, always reclaim freed terminals:
     `for ctx in $(orca orchestration worker-list --run <run_id> | awk '/terminal=reclaimable/ {print $1}'); do orca orchestration worker-release --dispatch "$ctx"; done`
   - Verify aggregate RSS across worker processes (`ps -eo rss,comm | awk '/opencode|agy|cargo|rustc/ {s+=$1} END {printf "%.2f GiB\n", s/1024/1024}'` — must remain `< 25 GiB`).
   - Run at most 1 active `cargo` process at a time on the shared `CARGO_TARGET_DIR`.
