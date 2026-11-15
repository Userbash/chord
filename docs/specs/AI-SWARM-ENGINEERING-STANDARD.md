# AI Swarm Engineering Standard (ASES-1.0) & AMCP v2.0

> **Standard Authority:** Canonical Architecture Specification for Large-Scale AI Swarm Engineering (10–100 Concurrent AI Agents)  
> **Repository:** `Chord` (`discord-lab`)  
> **Protocol:** `AMCP v2.0 / ASES-1.0`  
> **Status:** `Active Engineering Standard`  

---

## 1. Overview & Problem Statement

When software development scales from a single AI assistant to an **autonomous swarm of 10 to 100 AI agents** working simultaneously on the same repository, traditional git workflows collapse:
1. **Semantic Drift & Hidden Incompatibilities:** Separate agent branches pass local unit tests, but break interfaces upon merge.
2. **Context Window Starvation & Rebase Hell:** Rapidly moving trunk branches cause agent timeout and rebase loops.
3. **Decorative Test Coverage:** Unchecked agents write "green-passing" tests that lack actual fault-detection assertions.
4. **Target Directory Corruption:** Unsynchronized compiler invocations trigger file-locking deadlocks or binary corruption.

This standard establishes the **AI Swarm Engineering Standard (ASES-1.0)** and **AI Multi-Model Collaboration Protocol v2.0 (AMCP v2.0)** to provide mathematically sound, deterministic, and scalable multi-agent software engineering.

---

## 2. Swarm Topology & Role Specialization

The swarm operates in a **hierarchically coordinated, actor-isolated topology** to prevent quadratic $O(N^2)$ communication noise:

```
[ Human Principal / Chief Architect ]
                 │
                 ▼
[ Swarm Orchestrator (Argon / Carbon) ]  ───► Manages DAG, Issue Leases & Task Queue
                 │
      ┌──────────┼──────────┬──────────┐
      ▼          ▼          ▼          ▼
  [ Worker 1 ] [ Worker 2 ] [ Worker 3 ] ... [ Worker 100 ]
  (Worktree 1) (Worktree 2) (Worktree 3)     (Worktree 100)
      │          │          │          │
      └──────────┴────┬─────┴──────────┘
                      ▼
         [ Adversarial Review Council ]
         ├── Security Reviewer (OWASP, Secrets, Memory Safety)
         ├── Performance Reviewer (O(N^2), Allocations, Concurrency)
         ├── Contract & Spec Reviewer (Verification against docs/re/)
         └── Mutation Engine (5/5 Killed Mutants Requirement)
                      │
                      ▼
         [ Arbiter / Synthesizer Agent ] ───► Two-Party Sign-off
                      │
                      ▼
      [ Smart Predictive Merge Queue ] ───► Batched E2E Harness Execution
                      │
                      ▼
                 [ Trunk ]
```

### Role Matrix & Compute Budget:

| Role | Responsibility | Allowed Models | RAM Budget | Invariants |
|---|---|---|---|---|
| **Swarm Orchestrator** | Task queue triage, DAG dependency mapping, lease granting | `gemini-pro-orchestrator-argon` / `gemini-pro-orchestrator` | $\le$ 2 GB | Never write production code |
| **Worker Coder** | Strict TDD implementation of atomic task quantums | `gemini-4-argon-medium`, `gemini-4-hydrogen-medium`, `flash-medium` | $\le$ 1.5 GB / slot | 1 task = 1 branch = 1 worktree. Only assigned scope |
| **Adversary Reviewer** | Challenge logic, inject boundary edge cases, verify security | Cross-model (Claude 3.7 Sonnet / GPT-4o / Argon-High) | $\le$ 1 GB | Two-party rule: never review own code |
| **Mutation Engine** | Inject AST mutants into modified PR functions | Deterministic AST Mutation Engine | $\le$ 512 MB | Require 100% kill rate (5/5) |
| **Merge Queue Controller**| Batch PR verification in Headless Wayland E2E Harness | Deterministic Gate Script (`scripts/gate.mjs`) | $\le$ 2.5 GB | Bisection isolation on batch failure |

---

## 3. Task Slicing & Dynamic Module Lease Protocol

### 3.1. The Task Quantum
Every task submitted to the swarm must adhere to strict quantum limits:
- **Maximum Diff:** 30 to 150 lines of production code.
- **Maximum Files:** At most 3 cohesive files in a single department.
- **Execution Lifetime:** $\le$ 30 minutes wall-clock time.
- **Contract-First Decomposition:**
  1. *Step 1 (Interface):* Public trait, DTO, or protobuf definition in `.contracts/<module>.md`.
  2. *Step 2 (Failing Test):* Contract unit test written against clean-room oracle.
  3. *Step 3 (Implementation):* Minimal green implementation.
  4. *Step 4 (Plumbing):* Integration into parent controller or view.

### 3.2. Dynamic Lease Contract (Module Lease with TTL)
Concurrent file access is managed through dynamic leases stored in `.swarm/leases.json`:

```json
{
  "lease_id": "lease-mod-dave-ratchet-1042",
  "task_id": "TASK-1042",
  "scope_prefix": "rust/crates/chord-voice/src/dave_v1/",
  "worker_id": "worker-slot-04-argon",
  "granted_at": "2026-11-15T12:00:00Z",
  "expires_at": "2026-11-15T12:30:00Z",
  "status": "ACTIVE",
  "heartbeat_interval_sec": 60
}
```

- **Exclusive Write Access:** No worker may commit changes affecting paths covered by another worker's active lease.
- **Zombie Eviction:** If an agent fails to send a heartbeat within 180 seconds or crashes, the orchestrator revokes the lease, deletes the isolated worktree, and re-enqueues the task.

---

## 4. The 6-Stage Swarm Kanban Lifecycle

All tasks move sequentially through 6 deterministic states:

```
[ 1. BACKLOG ] ──► [ 2. LEASED ] ──► [ 3. TDD_IN_PROGRESS ] ──► [ 4. ADVERSARIAL_AUDIT ] ──► [ 5. MERGE_QUEUE ] ──► [ 6. LANDED ]
```

1. **`BACKLOG`:** Task has prerequisites cleared in the DAG.
2. **`LEASED`:** Worker claims task, receives lease token, provisions dedicated Git worktree (`orca worktree create`).
3. **`TDD_IN_PROGRESS`:**
   - **Phase RED:** Worker executes failing test, captures raw stdout.
   - **Phase GREEN:** Worker writes minimal code, verifies test passes.
   - **Phase T0 Gate:** Runs `cargo fmt`, `clippy -D warnings`, `node scripts/test-impact.mjs`.
4. **`ADVERSARIAL_AUDIT`:** PR opened. Triggers automated 4-way evaluation:
   - Security Audit (OWASP, UAF, memory safety, keyring).
   - Performance & Concurrency Audit (zero-copy, lock contention, non-blocking I/O).
   - Clean-room Spec Audit (conformance with `docs/re/`).
   - Mutation Engine Challenge (5 semantic mutants injected; 100% must fail the test suite).
5. **`MERGE_QUEUE`:** Approved PR enters the predictive merge queue batch.
6. **`LANDED`:** PR merged into `unstable` via fast-forward. Lease released, worktree cleaned up.

---

## 5. Adversarial Verification & Mutation Gate

### 5.1. Two-Party Rule
The agent that authored the code is strictly forbidden from participating in its review or approval.

### 5.2. Cross-Model Verification
Whenever feasible, cross-model arbitration is enforced:
- **Author:** Google Gemini (`gemini-4-argon` / `hydrogen`).
- **Reviewer:** Anthropic Claude (Claude 3.7 Sonnet) or OpenAI GPT (GPT-4o).
- **Result:** Prevents identical systematic blind spots from leaking into production.

### 5.3. 5/5 Mutation Testing Requirement
Every contract test introduced in a PR is subjected to 5 synthetic AST mutations:
1. `ComparisonInversion`: replacing `>` with `<=`, `==` with `!=`.
2. `BooleanInversion`: flipping boolean flags.
3. `StatementDeletion`: removing critical state update calls.
4. `ReturnReplacement`: substituting `Ok(T)` with default or mock returns.
5. `BoundaryShift`: incrementing or decrementing buffer and array bounds by $\pm 1$.

If any mutant survives (test still passes), the PR is rejected with an `ASSERTION_DEFECT` error requiring edge-case assertions.

---

## 6. Smart Predictive Merge Queue

To allow 100 workers to land code without stalling the main trunk:

```
[ Inflow PRs: #101, #102, #103, #104, #105 ]
                   │
                   ▼
     [ Construct Predictive Batch ]
     Virtual Branch: trunk + PR#101 + PR#102 + PR#103 + PR#104 + PR#105
                   │
                   ▼
     [ Run Headless E2E Harness ]
     - Wayland compositor (wayland-99, /dev/shm)
     - Mock Gateway & REST daemon
     - 180 Meta-tests + Workspace unit suite
                   │
         ┌─────────┴─────────┐
         ▼                   ▼
     [ PASS ]            [ FAIL ]
         │                   │
  Fast-forward merge         ▼
  all 5 PRs at once!     [ Bisection Isolation ]
                         Split into [101, 102] & [103, 104, 105].
                         Isolate broken PR in O(log N) runs.
                         Reject culprit, merge clean subset.
```

---

## 7. Execution Sandbox & Host Safety

- **Process Sandboxing:** Linux `cgroups v2` and namespaces (`bubblewrap`).
- **Network Gating:** Workers are sandboxed from external WAN; connections are permitted solely to local mock services (`127.0.0.1:9000-9100`).
- **Global Memory Ceiling:** Total memory allocated across all active worker terminals must never exceed **25 GB RAM**.
- **Compiler Safety:** Every compilation and test run must acquire `/tmp/chord-cargo.lock` to prevent parallel `rustc` process crashes.

---

## 8. Standard Machine-Executable Worker Algorithm

Every AI worker must follow this step-by-step sequence:

```bash
# 1. Sync and branch in dedicated worktree
git fetch origin unstable
git checkout -b feature/task-<ID>-<slug> origin/unstable

# 2. Scope Validation
# Ensure changes are strictly bounded to the assigned department scope.
# Modifying shared files (Cargo.lock, TASKS.md, waves/*) is strictly prohibited.

# 3. Phase RED (Test First)
# Author contract test verifying requirement against docs/re/
flock /tmp/chord-cargo.lock cargo test --package <pkg> --test <name>_contract 2>&1 | tee /tmp/red.log
# Verify the test fails cleanly.

# 4. Phase GREEN (Minimal Implementation)
# Implement only the minimal logic required to satisfy the contract.
flock /tmp/chord-cargo.lock cargo test --package <pkg> --test <name>_contract 2>&1 | tee /tmp/green.log
# Verify the test passes cleanly.

# 5. Local Quality Gate (T0)
cargo fmt --package <pkg>
flock /tmp/chord-cargo.lock cargo clippy --package <pkg> -- -D warnings
node scripts/test-impact.mjs

# 6. Commit Following Convention
git add <only-assigned-files>
git commit -m "feat(<scope>): <description> — Task #<ID>

- Change summary: <bullet points>
- Contract Test: <test_file> (5/5 mutation verified)
- Red Log: /tmp/red.log (verified failure)
- Green Log: /tmp/green.log (verified pass)
- Task: #<ID>
- AI-Model: <model_identifier>
- Operator: @<github_handle>"

# 7. Push and Create Pull Request
git push origin feature/task-<ID>-<slug>
# Release worker slot immediately upon PR creation.
```
