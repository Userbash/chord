# PROCESS.md — Production Process for discord-lab

Adaptation of the engineering process (task → concept → decomposition → workers → review →
build → kit → full gate → qualification → rollout → feature toggle → push) for `discord-lab`:
local CLI (Node ≥ 24, Frida 17, CDP) + native Rust GPUI client (`discord-gpui`).
The equivalent of critical currency assets is **golden baseline data** (`logs/`, `functions/`, `evidence/`) and **live Discord interactions**.

Sources of truth: this file (process); `TDD-PLAN.md` (phases M0–M6, contracts §6–§8);
`TASKS.md` (task cards, statuses, backlog); `AGENTS.md` (core agent guidelines).
In case of conflict: PROCESS.md → TDD-PLAN.md → TASKS.md.

---

## 0. Core Principles

1. **User / Owner Decision Authority.** What CLI users observe (commands, flags, terminal output, exit codes) must be confirmed by the owner ("Accept" / "Revise"). Only approved behavior reaches production code; everything else stays in the backlog.
2. **Strict TDD.** Red test → minimal implementation → green. Production code is never written without a preceding test. Meta-infrastructure (gates, docs, process scripts) is verified through automated test suites.
3. **Small Atomic Slices.** Wave → workers with disjoint file boundaries → worker gate → acceptance. Bugs are caught while isolated within an atomic unit.
4. **Golden Data Inviolability.** `logs/`, `functions/`, `evidence/` are strictly read-only. Writes outside designated logging commands are considered incidents; these artifacts are never committed to git.
5. **Consensual Live Operations.** Attaching Frida or running UI-walk operations against a running Discord process occurs only with explicit user permission. Default behavior is always fail-safe (skip / exit code 2).
6. **Dual Review for Sensitive Paths.** Modifications affecting live attachment, golden formats, or destructive operations require independent review by a secondary model (§4.4).
7. **Skip ≠ Pass.** Test skips never count as passing: they are flagged in the gate summary (`passed_with_skips`) and must be justified before release.
8. **Engineering Language Standards.** Documentation, CLI diagnostics, and commit messages are in English. Code: ESM with `// @ts-check` + JSDoc, Node ≥ 24, and modern Rust 2021.

---

## 1. Process Alignment Map

| Stage | discord-lab Equivalent |
| --- | --- |
| 1. Task / Issue Card | Card in `TASKS.md`: ID, microtask, owner, status `[ ]/[~]/[x]`; deadline = wave; audit trail = TASKS + `.gate/*.json` |
| 2. Concept (User Contract) | Observable CLI/GUI contract: commands, flags, output, exit codes (TDD-PLAN §6, spec). Owner gives sign-off |
| 3. Decomposition | Wave `W<N>`: matrix of disjoint files + specification for each worker (§4.1). Invariant checks verified against independent oracles (jq/awk/md5) |
| 4. Worker Execution | TDD in dedicated files; format compatibility checks (§6); `npm run gate:worker`; report (§4.2). Isolation via disjoint files and dedicated worktrees |
| 5. Review | Coordinator acceptance (§4.3); sensitive changes undergo dual review (§4.4) |
| 6. Release Branch Assembly | Wave acceptance into integration stream: `gate:release`, MD5 diff audits, meta-test sync |
| 7. Distribution Kit | Release tarball + deployment/rollback plan (§4.5). Kits are never edited manually: modifications trigger a new version and new `gate:qualify` |
| 8. Full Release Gate | `npm run gate:release`: typecheck → meta → unit → smoke → e2e (auto-retry on failure) → coverage → golden verification |
| 9. Qualification | `npm run gate:qualify`: tarball assembly, isolated package execution, checksum log `.gate/qualification-<version>-<ts>.json` |
| 10. Stepwise Rollout | Preflight → install (temp) → smoke check → switch → rollback fallback |
| 11. Feature Toggles | New capabilities ship disabled by default behind flags (`DISCORD_LAB_LIVE_CDP`, `--spawn`, `CDP_ENDPOINT`). Disabling requires no rebuild |
| 12. Push / Git Sync | Branch pipeline `unstable -> testing -> beta -> main` with signed tags |

---

## 2. Department Roles and Seats

Roles represent distinct seats rather than isolated developer silos. An agent may fill multiple seats across waves, but **authors never review their own code**, and live security paths are checked by an independent reviewer.

| Role | Responsibility | Exclusions |
| --- | --- | --- |
| Owner | Priorities, contract approvals, live testing windows, release authorization | Does not write application code or specs |
| Team Lead (L1) | Queue management, batch planning, contracts, acceptance, final release gates | Never reviews own code; never edits worker files without assignment |
| Tech Lead (L2) | Architecture, shared integration patches, contract conflict resolution | Does not accept task cards unilaterally |
| Research Engineer | Clean-room RE, fixtures, provenance validation, oracle verification | Does not write production code |
| Module Worker | Module implementation + tests: red → green → targeted report; strictly assigned files | Never touches shared `types.rs`/`reducer.rs`/`mod.rs`/`lib.rs`, never touches docs |
| Test / Oracle Engineer | Independent test suites, fixtures, contract assertions, test budget | Never bends tests to fit implementation |
| Mutation / QA Engineer | Control mutations, flaky test elimination, batch gate verification | Never modifies contracts to pass mutations |
| Independent Reviewer | Code quality, file boundaries, TDD verification, security inspection | Never modifies code during review |
| Security Reviewer | Secrets management, untrusted inputs, live IPC boundaries, timeouts | Never reviews own sensitive implementations |
| Integration / Build Engineer | Batch integration, `cargo` workspace runs, `gate:worker`, build manifests | Never rewrites author business logic |
| Docs / Journal Engineer | Synchronizes `STATE`, `ROADMAP`, `TASKS`, and `db.jsonl` **after** acceptance | Never updates documentation during red phase |
| Release Engineer | `gate:release`, `gate:qualify`, package tarballs, rollback management | Never issues releases without formal acceptance |
| Golden Keeper | Read-only maintenance of `logs/`, `functions/`, `evidence/`, `docs/re/` | Never normalizes or modifies golden records |

---

## 3. Detailed Execution Stages

### 3.1. Task Initiation
The owner establishes the goal → coordinator creates a card in `TASKS.md` (ID, microtask, owner, DoD, status).

### 3.2. User Interface Contract
For any CLI or GUI changes, a contract is established: command, parameters, UI state, exit codes (0/1/2), and error diagnostics.

### 3.3. Task Decomposition
Batch wave: 3–6 microtasks with disjoint file boundaries and a formal specification (§4.1).

### 3.4. Worker TDD Execution
- **Red-first:** Test is committed and executed to prove failure before writing implementation code.
- **Green:** Minimal implementation written to satisfy the test.
- **Targeted verification:** Local test execution for the specific module.
- **Report generation:** Completion report compiled according to §4.2.

### 3.5. Code Review and Implementation Quality
Team Lead reviews the batch post-gate: file boundaries, strict TDD adherence, independent oracles, error propagation, complexity limits, and memory safety.

Review Checklist:
- **Rust Memory Safety:** `#![forbid(unsafe_code)]`, strict ownership semantics, thread safety, and explicit error handling.
- **State & Concurrency:** Isolated shared state, deterministic locking order, timeouts on socket/process I/O.
- **Maintainability:** Minimal cohesive diffs, functions `< 50 lines`, files `< 800 lines`.

### 3.6. Release Branch Assembly
Merge into integration baseline: `gate:release`, MD5 file integrity audit, and meta-test verification.

### 3.7. Distribution Packaging (Kit)
Release package `vX.Y.Z`:
- Tarball built via standard packaging scripts;
- Deployment and rollback plan (§4.5);
- Verified fallback archive.

### 3.8. Release Gate (`gate:release`)
Phases: typecheck → meta → unit → smoke → e2e (automatic retry on transient failures) → test coverage thresholds (`lines >= 90%`, `branches >= 90%`, `functions >= 80%`).

### 3.9. Qualification Gate (`gate:qualify`)
Builds standalone release packages, validates execution in a clean sandbox, and records checksums in `.gate/qualification-<version>-<ts>.json`.

### 3.10. Rollout Sequence
1. **Preflight:** Git working tree clean, golden baseline verified, version and tags aligned.
2. **Install:** Deploy archive into temporary directory; verify diagnostic flags (`--help`, `--version`).
3. **Check:** Run smoke verification on the target machine.
4. **Switch:** Atomic symlink or binary installation.
5. **Rollback:** Automated revert to fallback archive upon unexpected behavior.

### 3.11. Feature Toggles
New features ship inactive behind configuration flags or command-line switches. Toggling features requires no binary rebuilds.

### 3.12. Git Synchronization
All feature work flows through `unstable` → `testing` → `beta` → `main` with annotated semantic tags.

---

## 4. Operational Templates

### 4.1. Worker Specification Template

```markdown
### Spec <batch>/<ID> — <Title>
Role / Seat: <G1-Backend / G2-Frontend / etc.>
Batch: <RW-NN>; Dependencies: <Accepted prerequisite cards>
Goal: <1–3 lines; user-observable contract>
Files (Strict Scope): <Explicit file paths; shared files labeled as Tech Lead only>
Forbidden: <All other files, shared state, docs, or golden directories>
Test Cases (Red-First): <Test IDs, expected assertions, boundaries>
Oracle / Provenance: <jq / awk / md5 / reference artifact>
Implementation Invariants: <Error handling, ownership, limits, timeouts, concurrency>
Verification Command: <Targeted test command>
DoD: Red run with failure exit code; green targeted run; report §4.2; file checksums.
```

### 4.2. Worker Completion Report Template

```markdown
- Batch / Card: <RW-NN / TASK-XXX>;
- Summary: <What was implemented and verified>;
- Red Phase: <Command> → <Failure output, exit code>;
- Green Phase: <Command> → <Success output, exit code>;
- Targeted Tests: <Log file / number of passing assertions>;
- Independent Oracle: <Provenance reference, paths, contract claims>;
- Control Mutations: <KILLED / total; 5/5 killed>;
- Baseline Restore: <Verification of clean working tree>;
- Gate Status: <Local gate execution result>;
- Files and Checksums: <List of modified files + SHA-256>;
- Residual Risks: <Identified edge cases or follow-up items>;
- Hand-off Target: <Assigned reviewer>
```

### 4.3. Review Checklist (Coordinator)

- [ ] Batch gate completed successfully prior to review.
- [ ] File boundaries respected (diff audit against spec).
- [ ] Strict TDD confirmed via red/green logs.
- [ ] Assertions validated against independent clean-room oracles.
- [ ] Mutation testing verified (5/5 control mutations killed).
- [ ] Concurrent execution safety verified for shared resources.
- [ ] Explicit error handling (no unhandled errors or unlogged silent fallbacks).
- [ ] Memory safety verified: `#![forbid(unsafe_code)]` intact.
- [ ] Complexity ceilings respected (`< 50 lines/fn`, `< 800 lines/file`).
- [ ] Meta-tests passing 100%.

---

## 5. Summary of Quality Gates

| Gate | Contents | Timing | Target Artifact |
| --- | --- | --- | --- |
| Targeted Worker | Module-specific unit and contract tests | After green implementation | `waves/<batch>/workers/<id>/` |
| Batch Gate | Formatting, linter, workspace tests | Once per batch in integration lane | `.gate/worker-<ts>.json` |
| `npm run gate:release` | Full unit, smoke, E2E, and coverage suite | Release assembly | `.gate/release-<ts>.json` |
| `npm run gate:qualify` | Sandboxed archive build and verification | Pre-deployment qualification | `.gate/qualification-<ts>.json` |
| `npm run golden:freeze` | Integrity manifest of golden baselines | Prior to major refactoring | `.gate/golden.manifest` |

---

## 6. Format Compatibility and Schema Migrations

Data formats (`functions/pid-*.json`, `logs/*.jsonl`, SQLite stores):
- Forward Compatibility: New reader implementations must parse older historical data formats.
- Backward Compatibility: Older client versions should gracefully handle or ignore unknown new fields.
- Migration verification is always tested against real populated datasets.

---

## 7. Process Glossary

- **Wave (Batch):** A cohesive collection of 3–6 task cards sharing a frozen contract and integration gate.
- **Lane:** An execution track (backend core, UI frontend, voice/platform, QA integration).
- **Seat:** A defined operational role within the development department.
- **Golden Baseline:** Immutable reference data gathered from clean-room experiments.
- **Zero-Idle:** Engineering operating model ensuring continuous developer and review throughput.
- **DoD (Definition of Done):** Mandatory criteria required for a task card to transition to accepted status.
