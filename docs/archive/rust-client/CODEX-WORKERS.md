# Autonomous Workers: Roles, Orchestration, and Quality Gates

Status: Operational playbook. Autonomous CLI workers are **executors**, not the single source of truth: contracts, file write-scopes, and oracle assertions reside strictly in the repository. The same agent instance never authors and reviews the same card.

## 1. Roles and Responsibilities

| Seat | Executor | Permissions | Output Deliverables |
|---|---|---|---|
| TL-1 | Coordinator / Lead | Contracts, shared architecture files, acceptance, audit ledger | DESIGN specifications, architectural decisions, accepted status |
| CODE-1/2 | Worker Agent `workspace-write` | Strict non-overlapping slice of assigned task files | Feature implementation + targeted red/green verification logs |
| QA-MUT | Worker Agent `workspace-write` | Ephemeral test mutation injection, restore via sha256 | Mutation score logs, killed mutant verification |
| REV-1 | Independent Reviewer `read-only` | Strictly read-only access | P0/P1/P2/P3 defect report + formal review verdict |
| DOC-1 | Documentation Agent `workspace-write` | Dedicated documentation and release note files | Documentation updates and retrospective lessons |

Rule: **At most two active coding workers** in parallel. Code review and research tasks can proceed concurrently, but never write to production codebase files. Cargo builds are serialized through a workspace file lock:

```sh
flock /tmp/opencode/discord-cargo.lock cargo test -p <crate> --offline
```

## 2. Launching Worker Agents

Feature Worker (explicit file allowlist and strict prohibition of shared files in prompt):

```sh
worker-cli exec -C <repo-root> -s workspace-write \
  --output-last-message waves/RW-21/worker-P1-01.md \
  "You are CODE-1. Task card P1-01. Contract: waves/RW-21/DESIGN-P1-01.md.
   Files: rust/crates/discord-codec/src/etf.rs, src/lib.rs, tests/etf_full.rs.
   Strict TDD: first capture red log, then minimal green implementation.
   Do not touch shared files, do not run full workspace gate, do not commit.
   Return report following PROCESS.md §4.2 format."
```

Independent Reviewer (read-only sandbox, no file modifications):

```sh
worker-cli exec -C <repo-root> -s read-only review --ephemeral \
  -o waves/RW-21/reviews/review-P1-01.md \
  "Verify card P1-01 against DESIGN/TEST-MATRIX/REPORT, source code and logs.
   Identify real defects; classify as P0/P1/P2/P3 with file:line and reproduction steps.
   Ignore unrelated workspace changes. Do not alter test assertions to match implementation."
```

## 3. High-Velocity Task Decomposition

1. **Contract First:** Freeze API boundaries, error envelopes, edge cases, test IDs, and file write-scopes before coding.
2. **DAG Over Sequential Queues:** Independent cards with disjoint dependencies and file scopes execute concurrently.
3. **Controlled Concurrency:** Pair of workers per active batch. Adding excessive parallel agents creates thrashing on `target-dir` and shared state locks.
4. **Staged Pipeline:** While Batch B02 is in code implementation, Batch B01 undergoes gate and review; Batch B03 conducts cleanroom research.
5. **Single Batch Gate:** Individual workers run fast targeted tests; full `fmt/clippy/workspace/gate` runs once per integrated batch.
6. **Review Gate:** P0/P1/P2 defects are resolved test-first prior to acceptance; P3 non-blocking items move to backlog.

Development velocity stems from non-overlapping file scopes, frozen contracts, serialized compilation, early red tests, and independent oracle fixtures.

## 4. Worker Prompt Template

```text
Task Card: <ID>. Batch: <Batch-ID>.
Contract: <DESIGN.md>. Test Matrix: <TEST-MATRIX.md>. Oracle: <cleanroom-oracle/fixture>.
Assigned Files (STRICTLY THESE ONLY): <file-list>.
Prohibited: Shared workspace files, unrelated tests, Cargo.toml/package.json,
unauthorized network calls, live Discord instances, git commits, full gates.
Execution Sequence:
1) Targeted RED test run + capture log with non-zero exit code.
2) Minimal GREEN implementation.
3) Targeted GREEN test run + log.
4) 5–6 mutation test kills with sha256 restore.
5) Report: Summary, RED log, GREEN log, targeted tests, sha256 checksums, risks.
If contract is ambiguous: halt immediately and escalate to Lead; never invent API semantics.
```

## 5. Reviewer Prompt Template

```text
You are REV-1, independent code reviewer (not the original author). Read-only sandbox.
Verify <ID> against DESIGN, TEST-MATRIX, REPORT, source files, gate/mutation logs, and test fixtures.
Inspect for:
- Invalid boundary checks and unhandled errors.
- Credential leaks or unmasked sensitive data.
- Invented semantics diverging from cleanroom oracle.
- Relaxed test assertions or tests adapted to match faulty implementation.
- API regressions, concurrency hazards, panics, or memory unbounded growth.
- Absence of verifiable RED-first log.
Output: Verdict (ACCEPTED / CHANGES_REQUESTED); P0/P1/P2/P3 breakdown with file:line;
validation of targeted tests and mutation logs; residual operational risks.
```

## 6. Quality Feedback Loop

- **Mutation Kill Rate:** Mandatory metric: 5–10 mutations per card, target 100% killed. Any surviving mutation indicates a coverage hole or redundant code.
- **Red-First Verification:** Compilation failure is acceptable for newly declared APIs, but functional behavior requires runtime assertion failure before green. Red logs are stored in `waves/<ID>/logs/`.
- **Independent Oracle:** Golden test vectors derived from cleanroom captures, fixtures, `jq`, or cryptographic hashes rather than current code output.
- **Zero Criticals:** P0/P1/P2 = 0 before acceptance.
- **Codified Learnings:** Completed waves yield 3–5 actionable lessons recorded in `CODE-STANDARD.md`. Recurring patterns become automated rules.
- **Unified Sync:** Tracking documents (`STATE`, `STATUS`, `ROADMAP`, `TASKS`) are synchronized once per integrated batch.

## 7. Operational Boundaries

Workers must never be granted write permissions to `logs/`, `functions/`, `evidence/`, or `clean-room-oracle/`. Dangerous broad filesystem permissions (`danger-full-access`) are prohibited. Never pass credentials in prompts. Live external network calls or attaching to running production Discord instances are strictly prohibited without explicit operator approval.
