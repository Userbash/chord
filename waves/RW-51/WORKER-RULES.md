# RW-51 worker instructions

These rules accompany every leaf mandate. Read `PLAN.md`, `LIBRARY-MATRIX.md`, the exact card in `TASKS.json` and applicable repository rules before editing.

## Before starting

1. Verify your real task/session/dispatch identity and the pinned base. A queue entry alone is not an execution receipt.
2. Verify accepted prerequisites and obtain exclusive leases for exact files. A broad read scope is not write permission.
3. Confirm the library/version/features/MSRV/target decision. The coordinator alone integrates Cargo/package locks, registries and shared exports.
4. Record independent expected behavior and a failing regression before implementation. Existing clean-room fixtures and goldens are read-only.
5. Split the leaf again before editing if it exceeds one clear API/domain outcome or requires overlapping live work.

## Writing code

- Prefer ordinary Rust types and the real library API. Retain only domain policy and the smallest boundary adapter.
- Use validated IDs, named DTOs/enums, `Result` errors and explicit unknown-field/variant handling. Preserve lossless IDs across JavaScript.
- Generate IPC clients/types from the accepted Rust source; generated code is not runtime input validation or a permission bypass.
- Reuse existing Tokio ownership/cancellation boundaries. Do not block UI/audio callbacks or add nested runtimes.
- Use owned resources and deterministic teardown. Exercise cleanup after early error, timeout, cancellation and repeated start/stop.
- Keep comments/Rustdoc in English. Explain invariants, wire quirks and resource constraints; avoid comments that merely repeat syntax.
- Remove the superseded mechanism after the last consumer migrates and parity passes. Do not move the same implementation into a new compatibility facade.
- Leave standard settings atomic persistence, completed DTO/library migrations and the proven RTC fallback intact unless the card explicitly proves a safe change.
- No arbitrary dependency upgrades, duplicated HTTP/SQLite stacks, plaintext secret fallback or replacement of Discord-specific state/policy with generic defaults.

## Testing and reporting

Record `command → exit code → immutable artifact` for RED, GREEN, targeted checks and review. For code behavior, demonstrate five independent behavior-changing mutants; document the applicable nine dimensions and preserve old assertions.

T0 is scoped. T1 integration runs once per batch. T2/T3/T4 and native target gates run when their required environment/artifacts exist. Missing environments, ignored tests and fixture IPC are explicit limitations, never proof of native parity.

Report source hashes, exact files, dependency decision, generated output, removed/added owned production code, performance/resource changes and rollback. Keep logs outside golden `logs/`, `functions/`, `evidence/`; copy immutable relevant records into this new wave when required.

Use isolated fixtures/private buses/temporary stores. Never dual-run live side effects. Live account/media/Frida operations need their own existing authorization.

## Coordination

The coordinator leases `main.rs`, `app.js`, shared data/module exports, all manifests/locks and Kanban/journal. Only the specified worker edits an exclusive leaf scope. Authors do not approve their own work; product and technical reviews are separate.

Maximum initial worker concurrency: three. Aggregate RSS ceiling: 25 GiB. Serialize Cargo using `/tmp/chord-cargo.lock` and a shared or isolated declared target directory; do not launch unbounded builds.

Requested repository profiles/models are declarations. Discover real runtime/provider capabilities, record actual resolved model/effort and fail explicitly if unavailable. No silent fallback to another model or harness.

ECC installation/import, worker acceptance, implementation completion and review acceptance have separate receipts/statuses. Do not invent an Orca run to represent an ECC session or mark queued tasks in progress without a real process/remote worker heartbeat.
