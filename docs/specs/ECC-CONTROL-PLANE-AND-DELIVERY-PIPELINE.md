# ECC 2.0 Control Plane, Testing Pyramid & Delivery Pipeline Architecture

> **Document Status:** `Living Architecture & Process Specification (v2.0)`  
> **Project:** `chord` (`discord-gpui` native client + BFF & Core Rust)  
> **Orchestrator:** ECC 2.0 Control Plane (`scripts/ecc`) + Orca Orchestration Engine  
> **Updated:** November 2026  

---

## 1. Governance Architecture: ECC 2.0 Control Plane Model

The development governance and code delivery pipeline follows a hierarchical structure with a unified coordination hub — **ECC 2.0 Control Plane**:

```text
                       ┌──────────────────────────────────────────────┐
                       │          ECC 2.0 Chief Orchestrator          │
                       │           (Profile: pm_review / PM)          │
                       └──────────────────────┬───────────────────────┘
                                              │
                     ┌────────────────────────┴────────────────────────┐
                     ▼                                                 ▼
        ┌─────────────────────────┐                       ┌─────────────────────────┐
        │   Product Manager (PM)  │                       │   Technical Reviewer    │
        │ Acceptance & Scenarios  │                       │ Architecture, Sec & QA  │
        │       (ACCEPT.md)       │                       │  (REVIEW.md + 5/5 Mut)  │
        └────────────┬────────────┘                       └────────────┬────────────┘
                     │                                                 │
  ───────────────────┴─────────────────────────────────────────────────┴───────────────────
                                SPECIALIST WORKER DEPARTMENTS
  ┌───────────────┬───────────────┬───────────────┬───────────────┬───────────────────────┐
  │  G1 Backend   │  G2 Frontend  │   G3 Design   │    G4 QA      │  G5 Voice & Platform  │
  │  Rust Core &  │  WebView UI & │   UX/UI &     │  AT-SPI, E2E, │  DAVE E2EE, PipeWire  │
  │  Tauri/GPUI   │  DOM/A11y     │   Design-Kit  │  Fuzz, Chaos  │  Audio RingBuffer     │
  └───────────────┴───────────────┴───────────────┴───────────────┴───────────────────────┘
  ┌───────────────────────────────┬───────────────────────────────┬───────────────────────┐
  │     G6 Tech Writer / Parity   │   G7 Reverse Engineering REA  │  DevOps / Integration │
  │    Docs, Release Notes, SBOM  │   Oracle: docs/re             │  CI/CD, Gate, Release │
  └───────────────────────────────┴───────────────────────────────┴───────────────────────┘
```

### 1.1. Roles and Non-Overlapping Write-Scopes
Each department has an isolated write-scope to eliminate merge conflicts during parallel execution:

| Department / Role | Profile (`ecc2.toml`) | Write-Scope | T0 Verification |
| :--- | :--- | :--- | :--- |
| **G1 Backend** | `g1_backend` | `rust/crates/chord-{core,codec,media,store,i18n}`, `rust/gui-spike/src/*.rs`, `rust/chord-gpui/src/{net,state,storage,audio}` | `cargo test -p discord-core`, `--test <slice>_contract` |
| **G2 Frontend** | `g2_frontend` | `rust/gui-spike/ui/**/*.{js,css,html}`, `rust/chord-gpui/src/ui/**` | `node scripts/test-impact.mjs --run`, `npm run typecheck` |
| **G3 Design** | `g3_design` | `docs/specs/*.md`, `docs/specs/wireframes/`, design tokens | `tests/meta/design-breakpoints.test.mjs` |
| **G4 QA** | `g4_qa` | `rust/chord-gpui/tests/`, `tests/{unit,smoke,e2e,meta}/`, `scripts/test-impact.mjs` | `cargo test --test visual_regression_contract`, `gpui_e2e_contract` |
| **G5 Voice / Platform** | `g5_voice_platform` | `rust/crates/chord-voice/**`, `rust/crates/chord-platform/**`, `voice_devices.rs`, DAVE MLS v1 | `cargo test -p discord-voice -p discord-platform` |
| **G6 Tech Writer** | `g6_tech_writer` | `docs/`, `waves/RW-*/PARITY/`, `RELEASE-NOTES-*.md` | `tests/meta/docs-sync.test.mjs` |
| **G7 RE / REA** | `g7_re_rea` | Read-only `docs/re/`; writes to `waves/RW-*/RE-AUDIT/` | MCP `rea` and `understand` |
| **DevOps** | `devops` | `scripts/{ci.sh,gate.mjs,release.sh}`, `.gate/`, CI configurations | `bash scripts/ci.sh --dry-run` |
| **PM & Tech Reviewer** | `pm_review` | `ACCEPT.md`, `REVIEW.md` (5/5 mutation gate) | Comprehensive dual PR review |

### 1.2. Model Routing Policy
1. **Strict Prohibition on `codex` and `deepseek`:** Use of `codex` and `deepseek/deepseek-v4.1-flash` is strictly banned.
2. **Gemini Line (`antigravity`):**
   - **`gemini-4-carbon-high` (`--effort high`):** Applied exclusively for P0 core tasks (engine, DAVE MLS v1 cryptography, zero-copy Ring Buffer, Tokio ↔ GPUI actors).
   - **`gemini-4-argon-high` (`--effort high`):** Standard for core development tasks, BFF REST, WebRTC, QA validation, mutation testing, and visual regression.
   - **`gemini-3.8-flash-high` / `medium`:** UI component markup, modal views, user settings, documentation, and auxiliary checklists.

### 1.3. Resource Guardrails: OOM Prevention (`WORKER_RAM_LIMIT <= 25 GB`)
* **RAM Ceiling:** Host provides 48 GB RAM with no swap space (`Swap: 0B`). Total cumulative consumption of all worker processes (`agy`, `cargo`, `rustc`, `node`) **must never exceed 25 GB RAM**, leaving at least 20–23 GB free for the host OS.
* **Zero-Idle Policy:** Upon task completion (`worker_done`), the worker submits its report immediately, releasing its terminal slot (`worker-release`) so the slot is immediately reassigned to the next queued task.

---

## 2. Testing Pyramid and Verification Pipeline (T0–T4)

The verification pipeline is structured into 5 hierarchical levels:

```text
       ▲
      / \       T4: Qualification Gate (Live probes, Keyring, Packaging)
     /---\      
    /     \     T3: Release & Mutation Gate (5/5 mutations killed, ASan/TSan, SBOM)
   /-------\    
  /         \   T2: Integration & E2E (GPUI Headless Smoke, 50 E2E Journeys, SSIM >0.99)
 /-----------\  
/             \ T1: Worker Gate (Clippy -D warnings, Typecheck, Unit, Contract)
───────────────
T0: Targeted    (Targeted test impact: node scripts/test-impact.mjs --run, <500ms)
```

### 2.1. Verification Levels
* **T0 (Targeted):** Targeted test execution restricted to the modified module (`node scripts/test-impact.mjs --run` or `cargo test --test <module>_contract`). Runtime: `< 1s`.
* **T1 (Worker Gate):** Pre-PR gate run:
  * `npm run typecheck` + `npx tsc -p rust/gui-spike/ui/tsconfig.json`.
  * `cargo fmt --all -- --check`.
  * `cargo clippy --workspace --all-targets --all-features -- -D warnings`.
  * `node scripts/gate.mjs --scope worker`. Runtime: `< 30s`.
* **T2 (Integration & E2E Gate):**
  * `cargo run --manifest-path rust/chord-gpui/Cargo.toml -- --headless-smoke` (validates cold boot, SQLite WAL, 120 FPS frame budget, VmRSS < 60 MB).
  * `cargo test --manifest-path rust/chord-gpui/Cargo.toml --test gpui_e2e_contract` (50 end-to-end user journeys offline).
  * `cargo test --manifest-path rust/chord-gpui/Cargo.toml --test visual_regression_contract test_sub_904` (SSIM > 0.995, dynamic region masking).
* **T3 (Release & Mutation Gate):**
  * Mutation testing threshold: **5 out of 5 intentional mutants must be killed by tests**.
  * Memory sanitizers: compilation with AddressSanitizer and ThreadSanitizer to detect races in async multithreaded runtime.
  * SBOM generation and verification (`node scripts/sbom.mjs --check`).
  * Checksum verification against `rust/dist/SHA256SUMS`.
* **T4 (Qualification Gate):**
  * `node scripts/gate.mjs --scope qualify`.
  * Hardware keyring integration verification (`DISCORD_LAB_KEYRING_LIVE=1`).
  * Final tarball package build and checksum generation.

---

## 3. Branching Strategy and GitHub Workflow

### 3.1. Branch Hierarchy
Development is strictly managed across protected branch tiers. Direct commits to protected branches are forbidden:

```text
 worker branch (<type>/task-<id>-<slug>)
       │  (local T0 + T1 tests, PR to unstable)
       ▼
    unstable  ◄─── Dual review (PM: ACCEPT.md + Tech: REVIEW.md)
       │  (consolidated into BUILD-00X build manifest)
       ▼
    testing   ◄─── Centralized QA verification by G4 QA
       │  (regression testing, e2e, smoke)
       ▼
      beta    ◄─── Pre-release validation (SHA256SUMS, SBOM, rollback)
       │
       ▼
      main    ◄─── Stable release, Git tag vX.Y.Z, sync back to unstable
```

#### Worker Branch Naming Conventions:
* `feature/task-<id>-<short-slug>` — new functionality (e.g. `feature/task-901-headless-harness`).
* `bugfix/task-<id>-<short-slug>` — bug fix (e.g. `bugfix/task-909-fix-render-clip`).
* `qa/task-<id>-<short-slug>` — test contracts and test automation.
* `design/task-<id>-<short-slug>` — UI specifications, tokens, wireframes.
* `devops/task-<id>-<short-slug>` — build scripts, CI/CD, release pipeline.
* `docs/task-<id>-<short-slug>` — documentation, parity matrices, release notes.

### 3.2. 13 Mandatory Task Lifecycle Statuses
Every task traverses 13 strict statuses:

1. `Backlog` — task defined with acceptance criteria and subtask breakdown.
2. `Ready for Development` — dependencies satisfied, assigned to department and engineer.
3. `In Progress` — branch created from `unstable`, RED test written.
4. `Ready for Product Review` — code implemented, T0/T1 passed, PR submitted to `unstable`.
5. `Ready for Technical Review` — PM validated business scenarios (`ACCEPT.md`), handed to Tech Reviewer.
6. `Ready for Merge` — Tech Reviewer signed off (`REVIEW.md`), 5/5 mutants killed, CI green.
7. `Merged to Unstable` — PR merged into `unstable`, worker branch deleted.
8. `In Testing` — `BUILD-00X` merged into `testing` branch, QA cycle active.
9. `QA Failed` *(on defect)* — returned with defect report, `bugfix/` branch created from `unstable`.
10. `QA Passed` — all T2/T3 tests pass with zero regressions.
11. `In Beta` — build promoted to `beta`, verified in staging environments.
12. `Ready for Release` — `SHA256SUMS`, SBOM, LTO/strip, and rollback protocol ready.
13. `Released` — merge `beta -> main`, tag `vX.Y.Z` published, binary distributed.

### 3.3. Commit Message Standard (Conventional Commits)
Commit message syntax:
```text
<type>(<scope>): <short description in imperative mood>

<detailed description: rationale, contracts modified>

Closes: #<task-id>
```
* `type`: `feat`, `fix`, `refactor`, `perf`, `test`, `docs`, `chore`, `ci`.
* `scope`: `gpui`, `voice`, `codec`, `net`, `a11y`, `visual`, `e2e`.

---

## 4. Automation Roadmap: Implemented and Planned Features

### 4.1. Currently Implemented Automation:
1. **`scripts/ci.sh`**:
   * Automatic detection of headless display server and desktop environments.
   * Formatting checks (`cargo fmt --check`) and static analysis (`clippy -D warnings`).
   * Native GPUI test suite: `headless-smoke`, 50 E2E contracts, SSIM regression.
   * License compliance and SBOM generation via `scripts/sbom.mjs`.
2. **`scripts/gate.mjs`**:
   * Phased test runner supporting 3 profiles (`worker`, `release`, `qualify`).
   * Automated storage of JSON test artifacts under `.gate/`.
   * Automatic flaky test retry mechanism for failing E2E scenarios.
   * Enforced policy: `skip` is never counted as passed.
3. **`scripts/test-impact.mjs`**:
   * Dependency graph mapping between modified source files and test suites for sub-second T0 runs.
4. **Kanban Board (`waves/kanban-board.json`, `.html`)**:
   * Automatic bi-directional synchronization with Orca orchestrator.

### 4.2. Next Automation Milestones (Roadmap):
1. **Virtual Wayland Compositor in CI:**
   * Script `scripts/run-headless-compositor.sh` launching `cage -s -- discord-gpui` on socket `wayland-99` inside CI containers without a physical display server.
2. **Failure Video Capture for E2E:**
   * Framebuffer dump via `wlr-screencopy` protocol into compressed WebM video upon `panic!` in an E2E test.
3. **Automatic SSIM Golden Update on Theme Changes:**
   * `--update-goldens` flag to re-render 15 golden reference screenshots when design tokens change.
4. **GitHub Actions CI Bot:**
   * PR bot posting coverage diffs, frame budget metrics (FPS Budget), and interactive `visual_diff_report.html`.
