# RW-51 atomic work items

Base: `2b7cfb52024be0453e3b9333aba292b2372e610e`. 124 leaves are planned. Three initial ECC source-audit phases completed; application migrations and next-phase acceptance remain pending. See DISPATCH-RECEIPTS.json.

Read `PLAN.md`, `LIBRARY-MATRIX.md`, `WORKER-RULES.md` and the exact leaf before implementation.

## Dependency overview

| Epic | Leaves | Mechanism delegated | Policy retained |
| --- | ---: | --- | --- |
| GOV — Baseline, contracts and truthful orchestration | 6 | Existing Git, Cargo metadata, ECC/Orca and the existing Kanban | Feature policy, integration decisions and evidence acceptance |
| RSA — Secure remote-auth RSA | 8 | Patched RustCrypto rsa plus OS CSPRNG and standard encodings | Remote-auth protocol, explicit RSA-2048, OAEP parameters, session lifecycle and redacted errors |
| CRYPTO — Standard symmetric cryptography | 8 | RustCrypto AEAD, AES, HKDF and hash crates | Discord SRTP/DAVE framing, nonce construction, replay policy and epoch state |
| COMP — Bounded HTTP decompression | 3 | flate2 with an approved pure Rust backend | Body size/deadline limits and the existing strict HTTP error contract |
| STORE — SQLite resource ownership | 4 | Already-used rusqlite with one coherent SQLite linkage policy | Database schema, queries, keyset pagination, transactions and application validation |
| HTTP — Reusable HTTP transport | 6 | Already-used reqwest/Tokio/rustls HTTP mechanisms | Discord rate limits, non-idempotent request policy, credentials, headers and verified browser-profile requirements |
| PORTAL — Typed Linux portals and D-Bus | 7 | Existing ashpd plus typed zbus proxies | Permission/source selection, restore tokens, cancellation and owned file descriptor lifetime |
| SECRET — Native secret storage | 5 | keyring-core plus explicit secure platform backend crates | Service/account identity, migration, redaction and no plaintext fallback |
| JSON — Typed bounded serialization | 5 | Serde DTOs plus a small bounded duplicate-aware visitor | Lossless IDs, duplicate rejection, depth/node budgets and error locations |
| IPC — Actual generated Rust IPC and plugin boundaries | 15 | Tauri v2, qualified exact-pinned Specta generator and framework tests | Domain methods, wire compatibility, command permissions, lossless IDs and explicit errors |
| PRIM — Standard identifiers and encodings in core | 3 | OS RNG/uuid, base64 and xxhash-rust xxh64 | Existing wire formats, strict acceptance rules and catalog hashing seed/encoding |
| RUN — Shared typed Rust services and runtime ownership | 3 | Existing Tokio tasks, channels and async services | Domain orchestration, renderer adapters, cancellation and UI responsiveness |
| URL — Standard URL grammar with explicit policy | 3 | url::Url and typed origins/query parsing | Strict raw-input policy, deep-link routing, token byte preservation and security allowlists |
| ASSET — Deterministic local UI dependency loading | 2 | A qualified small build tool and normal module resolution | Existing Tauri/browser fixture assets, CSP/offline behavior and entry points |
| SIGNAL — Real reactive primitives in the retained DOM frontend | 3 | @preact/signals-core | Domain stores/actions, lifecycle policy and existing observable UI state |
| FLOAT — Real anchored overlay positioning | 2 | @floating-ui/dom | Popover visibility, focus, keyboard, permissions and content |
| VIRT — Measured virtualization | 2 | @tanstack/virtual-core if its qualification beats the existing implementation | Page cache, loading/permissions, stable scroll anchors and actual renderer |
| MARK — Shared typed Markdown AST | 5 | pulldown-cmark for standard Markdown parsing | Discord mentions, emoji, spoilers, safe URLs and renderer policy |
| EVENT — Typed event completeness before polling removal | 3 | Tauri channels/events and Tokio state channels | Initial snapshot, generation ordering, subscription lifecycle and the proven RTC fallback |
| AUDIO — Qualified real-time and native media primitives | 3 | Existing native/audio crates and rtrb only for a proven SPSC queue | DAVE/MLS sessions, stream selection, echo/PID policy and real-time budgets |
| CLIP — Native clipboard without subprocess glue | 2 | Qualified arboard or official Tauri clipboard provider | PNG/file/URI ownership, names, deadlines and renderer-neutral provider trait |
| I18N — Qualified locale mechanisms | 2 | Targeted ICU4X components/data only after parity | Catalog message IDs, pinned locale/fallback policy and Discord formatting rules |
| DEVX — Remove duplicated control-plane and diagnostics boilerplate | 2 | Existing structured data, tracing and standard library mechanisms | Stable diagnostics, redaction, auditable task state and project workflows |
| QA — Cross-platform parity and measured acceptance | 7 | Existing test frameworks, native CI runners, Cargo tooling and browser harness | Independent oracles, release qualification and honest failure reporting |
| DEP — Serialized exact dependency integration | 15 | Existing Cargo/npm lock mechanisms and approved upstream artifacts | One approved library family per task, explicit native/features/export ownership |

## GOV: Baseline, contracts and truthful orchestration

Delegate mechanism: Existing Git, Cargo metadata, ECC/Orca and the existing Kanban.

Retain policy: Feature policy, integration decisions and evidence acceptance.

### LIB-GOV-01

Observed phase: real ECC session `8aa61e7a` completed its read-only planning audit. Report: [LIB-GOV-01](reports/LIB-GOV-01.json). Application migration: pending.

**Freeze the application feature and failure ledger** — P0, baseline, G4 QA.

Prerequisites: none. Resource lease: `gov-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `waves/STATE.md`; `TDD-PLAN.md`; `waves/RW-40/REVIEWS/CI-FAILURE-LOG.md`.

Write scope: `waves/RW-51/BASELINE-CHECKS.json`; `waves/RW-51/FEATURE-PARITY.md`.

Map every supported feature to both renderers, commands/events, native OS evidence and known failures before migration.

Independent oracle: Existing tests, clean-room docs/re records and immutable previous verification results; do not turn known reds into passes.

Targeted check: `Run affected baseline suites once with exact commands/exits; store a machine-readable expected-failure ledger.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Map every supported feature to both renderers, commands/events, native OS evidence and known failures before migration.
- Existing tests, clean-room docs/re records and immutable previous verification results; do not turn known reds into passes.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-GOV-02

Observed phase: real ECC session `051d71c3` completed its read-only planning audit. Report: [LIB-GOV-02](reports/LIB-GOV-02.json). Application migration: pending.

**Qualify dependencies and establish one integration owner** — P0, qualification, DevOps / Integration.

Prerequisites: none. Resource lease: `gov-slice`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`.

Write scope: `waves/RW-51/DEPENDENCY-DECISIONS.md`; `waves/RW-51/DEPENDENCY-GRAPH.json`.

Record existing direct/transitive versions, minimum features, MSRV, licenses, advisories and target support separately for all three Rust workspaces.

Independent oracle: Locked manifests, cargo metadata/tree, primary crate documentation and advisory database; no install based only on a package name.

Targeted check: `Serialized cargo metadata --locked for each manifest; security/license/MSRV review and duplicate links/features report.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Record existing direct/transitive versions, minimum features, MSRV, licenses, advisories and target support separately for all three Rust workspaces.
- Locked manifests, cargo metadata/tree, primary crate documentation and advisory database; no install based only on a package name.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-GOV-03

**Restore the real ECC endpoint and verify its capabilities** — P0, operations, DevOps / Integration.

Prerequisites: none. Resource lease: `gov-slice`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `scripts/ecc`; `scripts/ecc-orca-bridge.sh`; `ecc2.toml`; `docs/specs/ECC-CONTROL-PLANE-AND-DELIVERY-PIPELINE.md`.

Write scope: `waves/RW-51/DISPATCH-RECEIPTS.json`; `waves/RW-51/ECC-CAPABILITIES.json`.

Resolve an operator-supplied authentic ECC/Orca executable or endpoint, verify non-mutating help/status and available worker profiles.

Independent oracle: A real executable/version/identity/capability response; the dangling /home/sanya symlink and STATE run ID are not runtime evidence.

Targeted check: `Authenticated read-only health/profile check followed by one idempotent dry-run import; retain receipt and exact exit.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Resolve an operator-supplied authentic ECC/Orca executable or endpoint, verify non-mutating help/status and available worker profiles.
- A real executable/version/identity/capability response; the dangling /home/sanya symlink and STATE run ID are not runtime evidence.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-GOV-04

**Make detailed Kanban cards portable and lossless** — P0, implementation, DevOps / Integration.

Prerequisites: none. Resource lease: `kanban-script`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `scripts/mcp-kanban-board.mjs`.

Write scope: `scripts/mcp-kanban-board.mjs`; `tests/meta/kanban-portability.test.mjs`.

Use the actual checkout root and retain dependencies, instructions, scope and dispatch metadata through create/update/export.

Independent oracle: Process-level tests in two temporary checkouts: the decoy board remains byte-identical and rich metadata survives a round trip.

Targeted check: `node --test tests/meta/kanban-portability.test.mjs`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use the actual checkout root and retain dependencies, instructions, scope and dispatch metadata through create/update/export.
- Process-level tests in two temporary checkouts: the decoy board remains byte-identical and rich metadata survives a round trip.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-GOV-05

**Replace unknown-ID dispatch fallback with explicit mandates** — P0, implementation, DevOps / Integration.

Prerequisites: LIB-GOV-03, LIB-GOV-04. Resource lease: `dispatch-router`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `scripts/harmony-router.mjs`; `scripts/ecc-orca-bridge.sh`; `waves/RW-51/HANDOFF.json`.

Write scope: `scripts/harmony-router.mjs`; `tests/meta/modernization-dispatch-contract.test.mjs`.

Route new cards from their exact approved mandate, dependencies and file leases; reject unknown or blocked cards instead of inventing filenames.

Independent oracle: Fake control-plane fixture records zero worker starts for missing runtime, unknown ID, unmet prerequisites and duplicate dispatch key.

Targeted check: `node --test tests/meta/modernization-dispatch-contract.test.mjs`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Route new cards from their exact approved mandate, dependencies and file leases; reject unknown or blocked cards instead of inventing filenames.
- Fake control-plane fixture records zero worker starts for missing runtime, unknown ID, unmet prerequisites and duplicate dispatch key.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-GOV-06

**Establish exact code-reduction accounting and audit remaining files** — P1, baseline, G4 QA.

Prerequisites: LIB-GOV-01. Resource lease: `gov-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `waves/RW-51/INVENTORY.json`; `waves/RW-51/SOURCE-BASELINE.csv`.

Write scope: `waves/RW-51/CODE-REDUCTION.json`; `waves/RW-51/FILE-DISPOSITIONS.json`.

Classify all 589 baseline files as handwritten logic, generated data/output, tests or assets and assign retain/replace/modularize decisions by department.

Independent oracle: Immutable base Git blobs and reviewed classification; the 13,475 generated i18n data lines are not handwritten savings.

Targeted check: `Reproducible before/after inventory plus rename/move and generated-output controls.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Classify all 589 baseline files as handwritten logic, generated data/output, tests or assets and assign retain/replace/modularize decisions by department.
- Immutable base Git blobs and reviewed classification; the 13,475 generated i18n data lines are not handwritten savings.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## RSA: Secure remote-auth RSA

Delegate mechanism: Patched RustCrypto rsa plus OS CSPRNG and standard encodings.

Retain policy: Remote-auth protocol, explicit RSA-2048, OAEP parameters, session lifecycle and redacted errors.

Primary references: [source 1](https://rustsec.org/advisories/RUSTSEC-2023-0071.html), [source 2](https://docs.rs/rsa/latest/rsa/).

### LIB-RSA-01

Observed phase: real ECC session `2dc1d8ea` completed its read-only planning audit. Report: [LIB-RSA-01](reports/LIB-RSA-01.json). Application migration: pending.

**Freeze independent RSA remote-auth vectors** — P0, contract, G5 Voice/Platform.

Prerequisites: none. Resource lease: `remote-auth-crypto`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/gui-spike/src/remote_auth.rs`; `rust/gui-spike/src/biguint.rs`; `docs/re`.

Write scope: `rust/gui-spike/tests/remote_auth_crypto_contract.rs`.

Characterize explicit 2048-bit keys, OAEP SHA256/MGF1-SHA256/empty label, SPKI DER and fingerprint bytes with independent vectors.

Independent oracle: Independent RFC8017/WebCrypto/OpenSSL-produced vectors and malformed ciphertext cases; fixed keys belong only in test fixtures.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/gui-spike/Cargo.toml --test remote_auth_crypto_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Characterize explicit 2048-bit keys, OAEP SHA256/MGF1-SHA256/empty label, SPKI DER and fingerprint bytes with independent vectors.
- Independent RFC8017/WebCrypto/OpenSSL-produced vectors and malformed ciphertext cases; fixed keys belong only in test fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-RSA-02

**Qualify the patched pure-Rust RSA dependency** — P0, qualification, G5 Voice/Platform.

Prerequisites: LIB-RSA-01, LIB-GOV-02, LIB-DEP-01. Resource lease: `remote-auth-crypto`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/gui-spike/Cargo.toml`; `rust/gui-spike/src/remote_auth.rs`.

Write scope: `rust/gui-spike/tests/rsa_backend_qualification.rs`; `waves/RW-51/RSA-DEPENDENCY-DECISION.md`.

Prove a patched rsa >=0.10.0 build, supported MSRV/targets, fallible OS entropy and explicit 2048-bit configuration before production cutover.

Independent oracle: Current RustSec advisory, exact pinned crate API and the frozen vectors. New Generate defaults to 3072 bits, so defaults are insufficient.

Targeted check: `Build and execute the qualification contract on supported native targets; coordinator integrates only the approved exact dependency.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Prove a patched rsa >=0.10.0 build, supported MSRV/targets, fallible OS entropy and explicit 2048-bit configuration before production cutover.
- Current RustSec advisory, exact pinned crate API and the frozen vectors. New Generate defaults to 3072 bits, so defaults are insufficient.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-RSA-03

**Replace RSA key generation and OAEP with the qualified backend** — P0, implementation, G5 Voice/Platform.

Prerequisites: LIB-RSA-02. Resource lease: `remote-auth-crypto`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/gui-spike/src/remote_auth.rs`; `rust/gui-spike/src/biguint.rs`.

Write scope: `rust/crates/chord-core/src/lib.rs`; `rust/crates/chord-core/src/remote_auth_crypto.rs`; `rust/gui-spike/src/remote_auth.rs`; `rust/gui-spike/tests/remote_auth_crypto_contract.rs`.

Use OS-generated fresh production keys and library OAEP/encoding through the existing remote-auth API; propagate entropy failures.

Independent oracle: Existing wire vectors, independently decrypted samples, 190-byte success versus 191-byte rejection and malformed-OAEP/padding/error-redaction cases.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/gui-spike/Cargo.toml --test remote_auth_crypto_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use OS-generated fresh production keys and library OAEP/encoding through the existing remote-auth API; propagate entropy failures.
- Existing wire vectors, independently decrypted samples, 190-byte success versus 191-byte rejection and invalid-tag/error-redaction cases.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.
- Additional benefit to demonstrate (not assumed): Cryptographically fresh keys, explicit entropy errors and reduced custom cryptographic attack surface.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-RSA-04

**Review remote-auth security and remove obsolete arithmetic** — P0, review, Technical Review.

Prerequisites: LIB-RSA-03. Resource lease: `rsa-slice`.

Requested ECC profile: `tech_reviewer`; declared model `gemini-4-argon-high` / `high` (runtime not verified).

Read scope: `rust/gui-spike/src/remote_auth.rs`; `rust/gui-spike/src/biguint.rs`.

Write scope: `waves/RW-51/REVIEW-RSA.md`.

Independently reproduce vectors, production entropy isolation, zeroization and error behavior; sign off before disabling the old implementation.

Independent oracle: Reviewer is independent from the implementation author; reject known-factor production keys, secret logging and fallback to old arithmetic.

Targeted check: `Run original remote-auth contracts plus five security/parity mutations and Linux/Windows/macOS evidence.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Independently reproduce vectors, production entropy isolation, zeroization and error behavior; sign off before disabling the old implementation.
- Reviewer is independent from the implementation author; reject known-factor production keys, secret logging and fallback to old arithmetic.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-RSA-05

**Remove unused arithmetic after independent secure-backend approval** — P0, implementation, G1 Backend.

Prerequisites: LIB-RSA-04. Resource lease: `remote-auth-crypto`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/biguint.rs`; `rust/gui-spike/src/lib.rs`; `rust/gui-spike/src/remote_auth.rs`.

Write scope: `rust/gui-spike/src/biguint.rs`; `rust/gui-spike/src/lib.rs`; `rust/gui-spike/src/remote_auth.rs`.

Delete only unused arithmetic/exports after secure production key generation is independently approved; preserve test-vector provenance.

Independent oracle: Consumer/symbol inventory plus original remote-auth contracts and fresh-production-key regression.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/gui-spike/Cargo.toml --test remote_auth_crypto_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Delete only unused arithmetic/exports after secure production key generation is independently approved; preserve test-vector provenance.
- Consumer/symbol inventory plus original remote-auth contracts and fresh-production-key regression.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-RSA-06

**Resolve the two-frontend nonce and plaintext-policy contract independently** — P0, contract, G5 Voice/Platform.

Prerequisites: LIB-RSA-01. Resource lease: `remote-auth-wire`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/gui-spike/src/remote_auth.rs`; `rust/chord-gpui/src/auth/remote_auth.rs`; `rust/chord-gpui/tests/remote_auth_contract.rs`; `waves/RW-39/NET/PM/netr2/05-qr-extract.log`.

Write scope: `rust/chord-gpui/tests/remote_auth_wire_oracle_contract.rs`; `waves/RW-51/RSA-COMPATIBILITY-DECISION.md`.

Record the clean-room nonce_proof nonce-field contract, actual frontend differences, binary/text/token policies and independently approved old-to-new contract change before modifying tests.

Independent oracle: Checked-in clean-room QR extract plus authoritative WebCrypto/RFC semantics and authenticated independent vectors; current GPUI same-helper assertions are not an oracle.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/chord-gpui/Cargo.toml --test remote_auth_wire_oracle_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Record the clean-room nonce_proof nonce-field contract, actual frontend differences, binary/text/token policies and independently approved old-to-new contract change before modifying tests.
- Checked-in clean-room QR extract plus authoritative WebCrypto/RFC semantics and authenticated independent vectors; current GPUI same-helper assertions are not an oracle.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-RSA-07

**Connect GPUI to the shared secure cipher and approved nonce representation** — P0, implementation, G1 Backend.

Prerequisites: LIB-RSA-03, LIB-RSA-06. Resource lease: `remote-auth-wire`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/chord-gpui/src/auth/remote_auth.rs`; `rust/crates/chord-core/src/remote_auth_crypto.rs`.

Write scope: `rust/chord-gpui/src/auth/mod.rs`; `rust/chord-gpui/src/auth/remote_auth.rs`; `rust/chord-gpui/tests/remote_auth_wire_oracle_contract.rs`.

Implement RemoteAuthCipher with the shared validated provider and apply the independently approved raw-nonce wire contract without losing lifecycle, QR, fingerprint or error behavior.

Independent oracle: Independent cross-frontend SPKI/ciphertext/nonce fixtures and the accepted compatibility decision, never the XOR/tag mock alone.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/chord-gpui/Cargo.toml --test remote_auth_wire_oracle_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Implement RemoteAuthCipher with the shared validated provider and apply the independently approved raw-nonce wire contract without losing lifecycle, QR, fingerprint or error behavior.
- Independent cross-frontend SPKI/ciphertext/nonce fixtures and the accepted compatibility decision, never the XOR/tag mock alone.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-RSA-08

**Redact remote-auth secrets and propagate session activation failures** — P0, implementation, G5 Voice/Platform.

Prerequisites: LIB-RSA-06. Resource lease: `remote-auth-secrets`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/gui-spike/src/remote_auth.rs`; `rust/chord-gpui/src/auth/remote_auth.rs`.

Write scope: `rust/chord-gpui/src/auth/remote_auth.rs`; `rust/gui-spike/src/remote_auth.rs`; `rust/gui-spike/tests/remote_auth_secret_contract.rs`.

Remove reversible nonce/ticket/handshake-token exposure from Debug/telemetry and return explicit activation errors instead of success when the session-state lock fails.

Independent oracle: Independent sentinel secrets must never appear in formatted errors/logs; injected activation failure must be observable and preserve unchanged serialized success shapes.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/gui-spike/Cargo.toml --test remote_auth_secret_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Remove reversible nonce/ticket/handshake-token exposure from Debug/telemetry and return explicit activation errors instead of success when the session-state lock fails.
- Independent sentinel secrets must never appear in formatted errors/logs; injected activation failure must be observable and preserve unchanged serialized success shapes.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## CRYPTO: Standard symmetric cryptography

Delegate mechanism: RustCrypto AEAD, AES, HKDF and hash crates.

Retain policy: Discord SRTP/DAVE framing, nonce construction, replay policy and epoch state.

Primary references: [source 1](https://docs.rs/aes-gcm/latest/aes_gcm/), [source 2](https://docs.rs/chacha20poly1305/latest/chacha20poly1305/), [source 3](https://docs.rs/hkdf/latest/hkdf/).

### LIB-CRYPTO-01

**Freeze independent AEAD and key-schedule contracts** — P1, contract, G5 Voice/Platform.

Prerequisites: LIB-GOV-01. Resource lease: `crypto-slice`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/crates/chord-voice/src/crypto/aes.rs`; `rust/crates/chord-voice/src/crypto/gcm.rs`; `rust/crates/chord-voice/src/crypto/chacha.rs`; `rust/crates/chord-voice/src/crypto/kdf.rs`.

Write scope: `rust/crates/chord-voice/tests/library_crypto_parity.rs`.

Cover all live modes, nonce/AAD/key sizes, 8-byte DAVE tags, bad tags and no plaintext on authentication failure.

Independent oracle: Existing crypto_vectors and fuzz corpus plus independent NIST/RFC vectors; do not derive expected ciphertext from the new backend.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-voice --test library_crypto_parity`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Cover all live modes, nonce/AAD/key sizes, 8-byte DAVE tags, bad tags and no plaintext on authentication failure.
- Existing crypto_vectors and fuzz corpus plus independent NIST/RFC vectors; do not derive expected ciphertext from the new backend.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-CRYPTO-02

**Replace HKDF primitives while preserving SRTP derivation** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-CRYPTO-01, LIB-GOV-02, LIB-DEP-02. Resource lease: `voice-crypto-tests`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/crates/chord-voice/src/crypto/kdf.rs`.

Write scope: `rust/crates/chord-voice/src/crypto/kdf.rs`; `rust/crates/chord-voice/tests/library_crypto_parity.rs`.

Delegate HKDF to compatible crate types while retaining exact SHA1-based SRTP labels, salts and output lengths.

Independent oracle: Independent SRTP derivation vectors and exact byte comparison for all label/length boundaries.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-voice --test library_crypto_parity`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Delegate HKDF to compatible crate types while retaining exact SHA1-based SRTP labels, salts and output lengths.
- Independent SRTP derivation vectors and exact byte comparison for all label/length boundaries.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-CRYPTO-03

**Replace only AES block and full-tag GCM engines** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-CRYPTO-02, LIB-DEP-02. Resource lease: `voice-crypto-tests`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/crates/chord-voice/src/crypto/aes.rs`; `rust/crates/chord-voice/src/crypto/gcm.rs`; `rust/crates/chord-voice/src/crypto/chacha.rs`; `rust/crates/chord-voice/src/crypto/kdf.rs`.

Write scope: `rust/crates/chord-voice/src/crypto/aes.rs`; `rust/crates/chord-voice/src/crypto/gcm.rs`; `rust/crates/chord-voice/tests/library_crypto_parity.rs`.

Replace AES block/full-tag GCM mechanics only; ChaCha/XChaCha has its own leaf.

Independent oracle: KAT/corpus outputs, invalid nonce/key/AAD/tag lengths, nonce-wrap behavior and no unauthenticated plaintext.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-voice --test library_crypto_parity`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Delegate standard full-tag modes through existing domain APIs without altering SRTP packet layout or key selection.
- KAT/corpus outputs, invalid nonce/key/AAD/tag lengths, nonce-wrap behavior and no unauthenticated plaintext.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-CRYPTO-04

**Qualify and migrate the exact 8-byte GCM tag mode** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-CRYPTO-03. Resource lease: `voice-crypto-tests`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/crates/chord-voice/src/crypto/gcm.rs`; `rust/fuzz/fuzz_targets/fuzz_aes128_gcm_truncated8.rs`.

Write scope: `rust/crates/chord-voice/src/crypto/gcm.rs`; `rust/crates/chord-voice/tests/library_crypto_parity.rs`.

Compile and verify an actually supported eight-byte authenticated tag configuration at the pinned crate version before replacing Aes128GcmTruncated8.

Independent oracle: Independent eight-byte-tag vectors and adversarial mutations; API/tag support must be verified from exact source, not a stale latest-doc page.

Targeted check: `Run known answers, bad-tag/counter-boundary tests and existing fuzz target; reject adoption if the crate cannot authenticate the exact mode.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Compile and verify an actually supported eight-byte authenticated tag configuration at the pinned crate version before replacing Aes128GcmTruncated8.
- Independent eight-byte-tag vectors and adversarial mutations; API/tag support must be verified from exact source, not a stale latest-doc page.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-CRYPTO-05

**Remove superseded primitives after differential review** — P1, review, Technical Review.

Prerequisites: LIB-CRYPTO-04, LIB-CRYPTO-06, LIB-CRYPTO-07, LIB-CRYPTO-08. Resource lease: `crypto-slice`.

Requested ECC profile: `tech_reviewer`; declared model `gemini-4-argon-high` / `high` (runtime not verified).

Read scope: `rust/crates/chord-voice/src/crypto/aes.rs`; `rust/crates/chord-voice/src/crypto/gcm.rs`; `rust/crates/chord-voice/src/crypto/chacha.rs`; `rust/crates/chord-voice/src/crypto/kdf.rs`.

Write scope: `waves/RW-51/REVIEW-CRYPTO.md`.

Approve deletion of unused primitive internals and report code/attack-surface reduction without removing packet/replay/DAVE policy.

Independent oracle: Independent original suite, corpus parity, reviewer-owned mutations and symbol/consumer inventory.

Targeted check: `Core voice contracts plus five mutants and native target crypto smoke.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Approve deletion of unused primitive internals and report code/attack-surface reduction without removing packet/replay/DAVE policy.
- Independent original suite, corpus parity, reviewer-owned mutations and symbol/consumer inventory.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-CRYPTO-06

**Migrate ChaCha and XChaCha independently of AES-GCM** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-CRYPTO-02, LIB-DEP-02. Resource lease: `voice-crypto-tests`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/crates/chord-voice/src/crypto/chacha.rs`.

Write scope: `rust/crates/chord-voice/src/crypto/chacha.rs`; `rust/crates/chord-voice/tests/library_crypto_parity.rs`.

Replace only ChaCha/XChaCha mechanics under the existing small adapter, preserving nonce/AAD/tag and packet semantics.

Independent oracle: Independent RFC vectors, invalid-tag/no-plaintext cases and existing voice corpus.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-voice --test library_crypto_parity`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Replace only ChaCha/XChaCha mechanics under the existing small adapter, preserving nonce/AAD/tag and packet semantics.
- Independent RFC vectors, invalid-tag/no-plaintext cases and existing voice corpus.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-CRYPTO-07

**Replace GPUI manual HKDF expansion with the already declared crate** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-CRYPTO-02. Resource lease: `gpui-audio-crypto`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/chord-gpui/src/audio/mod.rs`; `rust/chord-gpui/Cargo.toml`.

Write scope: `rust/chord-gpui/src/audio/mod.rs`; `rust/chord-gpui/tests/hkdf_library_contract.rs`.

Delegate the remaining GPUI HKDF expansion mechanics to its existing hkdf dependency, preserving exact labels, salts, hash family and output limits.

Independent oracle: Independent derivation vectors and the existing voice/DAVE framing contracts; do not change sessions, replay/epoch rules or key ownership.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/chord-gpui/Cargo.toml --test hkdf_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Delegate the remaining GPUI HKDF expansion mechanics to its existing hkdf dependency, preserving exact labels, salts, hash family and output limits.
- Independent derivation vectors and the existing voice/DAVE framing contracts; do not change sessions, replay/epoch rules or key ownership.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-CRYPTO-08

**Delegate MLS SHA256 and HMAC primitives while retaining protocol policy** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-CRYPTO-01, LIB-DEP-02. Resource lease: `mls-primitives`.

Requested ECC profile: `core_carbon`; declared model `gemini-4-carbon-high` / `high` (runtime not verified).

Read scope: `rust/crates/chord-voice/src/dave_v1/mls.rs`.

Write scope: `rust/crates/chord-voice/src/dave_v1/mls.rs`; `rust/crates/chord-voice/tests/mls_primitive_library_contract.rs`.

Replace only standard SHA256/HMAC/HKDF machinery with the qualified primitives; retain MLS labels, epochs, transcript/state transitions and wire representation.

Independent oracle: Independent hash/HMAC/MLS derivation vectors, malformed-input cases and unchanged state/wire corpus.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-voice --test mls_primitive_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Replace only standard SHA256/HMAC/HKDF machinery with the qualified primitives; retain MLS labels, epochs, transcript/state transitions and wire representation.
- Independent hash/HMAC/MLS derivation vectors, malformed-input cases and unchanged state/wire corpus.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## COMP: Bounded HTTP decompression

Delegate mechanism: flate2 with an approved pure Rust backend.

Retain policy: Body size/deadline limits and the existing strict HTTP error contract.

Primary references: [source 1](https://docs.rs/flate2/latest/flate2/).

### LIB-COMP-01

**Characterize compression acceptance and resource limits** — P1, contract, G1 Backend.

Prerequisites: LIB-GOV-01. Resource lease: `comp-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/rest/http1.rs`.

Write scope: `rust/crates/chord-core/tests/compression_library_contract.rs`.

Freeze raw deflate, zlib and gzip semantics, concatenated members, trailing bytes, corruption and exact max_body boundaries.

Independent oracle: Independent compressor-generated byte fixtures, declared existing policy and compressed-bomb cases.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-core --test compression_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Freeze raw deflate, zlib and gzip semantics, concatenated members, trailing bytes, corruption and exact max_body boundaries.
- Independent compressor-generated byte fixtures, declared existing policy and compressed-bomb cases.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-COMP-02

**Replace bit/Huffman decoders with bounded flate2** — P1, implementation, G1 Backend.

Prerequisites: LIB-COMP-01, LIB-GOV-02, LIB-DEP-03. Resource lease: `core-http1`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/rest/http1.rs`.

Write scope: `rust/crates/chord-core/src/rest/http1.rs`; `rust/crates/chord-core/tests/compression_library_contract.rs`.

Decode through the selected flate2 mode with a limit-plus-one overflow check and unchanged error categories.

Independent oracle: All frozen compressed inputs, exact-limit success versus limit-plus-one failure, and truncated header/trailer failures.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-core --test compression_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Decode through the selected flate2 mode with a limit-plus-one overflow check and unchanged error categories.
- All frozen compressed inputs, exact-limit success versus limit-plus-one failure, and truncated header/trailer failures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-COMP-03

**Delete unused decompression machinery and benchmark limits** — P1, review, G4 QA.

Prerequisites: LIB-COMP-02. Resource lease: `comp-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/rest/http1.rs`.

Write scope: `waves/RW-51/REVIEW-COMPRESSION.md`.

Verify old BitReader/Huffman/checksum code is unused and measure bounded allocation, error and throughput behavior.

Independent oracle: Independent corpus and reviewer-injected decompression-limit/corruption mutations.

Targeted check: `Run old HTTP contracts, targeted compression suite and repeated bomb/deadline probes.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Verify old BitReader/Huffman/checksum code is unused and measure bounded allocation, error and throughput behavior.
- Independent corpus and reviewer-injected decompression-limit/corruption mutations.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## STORE: SQLite resource ownership

Delegate mechanism: Already-used rusqlite with one coherent SQLite linkage policy.

Retain policy: Database schema, queries, keyset pagination, transactions and application validation.

Primary references: [source 1](https://docs.rs/rusqlite/0.40.2/rusqlite/).

### LIB-STORE-01

**Freeze on-disk database and statement contracts** — P1, contract, G1 Backend.

Prerequisites: LIB-GOV-01. Resource lease: `store-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-store/src/lib.rs`; `rust/crates/chord-store/src/ffi.rs`.

Write scope: `rust/crates/chord-store/tests/sqlite_library_contract.rs`.

Record file/schema/WAL compatibility, null/blob/text binding, busy errors, transactions and keyset pagination from real database fixtures.

Independent oracle: Independent SQLite CLI/fixture databases and existing store contracts; no production user database access.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-store --test sqlite_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Record file/schema/WAL compatibility, null/blob/text binding, busy errors, transactions and keyset pagination from real database fixtures.
- Independent SQLite CLI/fixture databases and existing store contracts; no production user database access.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-STORE-02

**Qualify a single SQLite dependency/features policy** — P1, qualification, DevOps / Integration.

Prerequisites: LIB-STORE-01, LIB-GOV-02, LIB-DEP-04. Resource lease: `store-slice`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/crates/chord-store/Cargo.toml`; `rust/chord-gpui/Cargo.lock`.

Write scope: `waves/RW-51/SQLITE-LINKAGE-DECISION.md`.

Choose already locked rusqlite 0.40.2 initially and prove one libsqlite3-sys links policy across all three workspaces.

Independent oracle: Real Cargo feature/link graph, required SQLite/SQLCipher behavior if present, and OS linker evidence.

Targeted check: `Serialized cargo tree/metadata and target compile probes; reject duplicate incompatible links=sqlite3.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Choose already locked rusqlite 0.40.2 initially and prove one libsqlite3-sys links policy across all three workspaces.
- Real Cargo feature/link graph, required SQLite/SQLCipher behavior if present, and OS linker evidence.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-STORE-03

**Replace connection, statement and transaction FFI ownership** — P1, implementation, G1 Backend.

Prerequisites: LIB-STORE-02. Resource lease: `store-ffi`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-store/src/lib.rs`; `rust/crates/chord-store/src/ffi.rs`.

Write scope: `rust/crates/chord-store/src/ffi.rs`; `rust/crates/chord-store/src/lib.rs`; `rust/crates/chord-store/tests/sqlite_library_contract.rs`.

Use rusqlite RAII while preserving the public store API and exact schema/query/error behavior.

Independent oracle: Reopen the old fixture database, compare query rows and transaction rollback outcomes independently.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-store --test sqlite_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use rusqlite RAII while preserving the public store API and exact schema/query/error behavior.
- Reopen the old fixture database, compare query rows and transaction rollback outcomes independently.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-STORE-04

**Verify crash recovery and remove the obsolete private SQLite FFI** — P1, review, G4 QA.

Prerequisites: LIB-STORE-03. Resource lease: `store-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-store/src/lib.rs`; `rust/crates/chord-store/src/ffi.rs`.

Write scope: `waves/RW-51/REVIEW-STORE.md`.

Reproduce interruption/rollback/concurrent-reader behavior, verify native builds and approve removal of unused FFI exports.

Independent oracle: Real temporary database process-kill/reopen probes and original encryption/security contracts where applicable.

Targeted check: `Store contracts, five mutations and native Linux/Windows/macOS store smoke.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Reproduce interruption/rollback/concurrent-reader behavior, verify native builds and approve removal of unused FFI exports.
- Real temporary database process-kill/reopen probes and original encryption/security contracts where applicable.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## HTTP: Reusable HTTP transport

Delegate mechanism: Already-used reqwest/Tokio/rustls HTTP mechanisms.

Retain policy: Discord rate limits, non-idempotent request policy, credentials, headers and verified browser-profile requirements.

Primary references: [source 1](https://docs.rs/reqwest/latest/reqwest/).

### LIB-HTTP-01

**Freeze HTTP, credentials and impersonation requirements** — P1, contract, G1 Backend.

Prerequisites: LIB-GOV-01. Resource lease: `http-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/rest/transport.rs`; `rust/crates/chord-core/src/rest/client.rs`; `rust/crates/chord-core/src/rest/http1.rs`.

Write scope: `rust/crates/chord-core/tests/http_transport_library_contract.rs`.

Record redirects, proxies, roots, headers/order where contractual, compression, deadlines, cancellation and browser-profile requirements.

Independent oracle: Local adversarial HTTP/TLS fixtures and the existing BrowserImpersonationProfile contracts; generic reqwest is not proof of Chrome TLS fingerprinting.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-core --test http_transport_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Record redirects, proxies, roots, headers/order where contractual, compression, deadlines, cancellation and browser-profile requirements.
- Local adversarial HTTP/TLS fixtures and the existing BrowserImpersonationProfile contracts; generic reqwest is not proof of Chrome TLS fingerprinting.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-HTTP-02

**Qualify an explicit reusable reqwest client policy** — P1, qualification, G1 Backend.

Prerequisites: LIB-HTTP-01, LIB-GOV-02, LIB-COMP-02, LIB-DEP-05. Resource lease: `http-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/rest/transport.rs`.

Write scope: `rust/crates/chord-core/tests/reqwest_policy_qualification.rs`; `waves/RW-51/HTTP-DECISION.md`.

Prove the approved transport can satisfy required profile, proxy/redirect/TLS/decompression/body limits with minimal features.

Independent oracle: Actual server-observed bytes/connections and denied redirect credential capture; fail the spike if fingerprint requirements cannot be met.

Targeted check: `Execute isolated local server transport probes and all documented policy failure paths.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Prove the approved transport can satisfy required profile, proxy/redirect/TLS/decompression/body limits with minimal features.
- Actual server-observed bytes/connections and denied redirect credential capture; fail the spike if fingerprint requirements cannot be met.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-HTTP-03

**Migrate one idempotent GET family through the shared client** — P1, implementation, G1 Backend.

Prerequisites: LIB-HTTP-02. Resource lease: `core-rest-transport`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/rest/transport.rs`; `rust/crates/chord-core/src/rest/client.rs`.

Write scope: `rust/crates/chord-core/src/rest/transport.rs`; `rust/crates/chord-core/tests/http_transport_library_contract.rs`.

Use one reusable client behind the existing transport seam for one bounded GET family with typed errors and cancellation.

Independent oracle: Accepted-connection counts and exact response/error parity against frozen fixture endpoints.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-core --test http_transport_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use one reusable client behind the existing transport seam for one bounded GET family with typed errors and cancellation.
- Accepted-connection counts and exact response/error parity against frozen fixture endpoints.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-HTTP-04

**Migrate non-idempotent requests and uploads with explicit retry ownership** — P1, implementation, G1 Backend.

Prerequisites: LIB-HTTP-03. Resource lease: `core-rest-transport`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/rest/client.rs`; `rust/crates/chord-core/src/rest/transport.rs`.

Write scope: `rust/crates/chord-core/src/rest/client.rs`; `rust/crates/chord-core/tests/http_transport_library_contract.rs`.

Preserve post_once, Discord 429 bucket semantics and bounded uploads without duplicating messages or replaying side effects.

Independent oracle: Local server records exactly one side-effecting POST under ambiguous failure, plus bounded explicit 429/upload cases.

Targeted check: `Run transport and existing REST upload/rate-limit contracts.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Preserve post_once, Discord 429 bucket semantics and bounded uploads without duplicating messages or replaying side effects.
- Local server records exactly one side-effecting POST under ambiguous failure, plus bounded explicit 429/upload cases.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-HTTP-05

**Replace GPUI pool bookkeeping with actual shared transport reuse** — P1, implementation, G1 Backend.

Prerequisites: LIB-HTTP-04. Resource lease: `gpui-net`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/chord-gpui/src/net/mod.rs`; `rust/crates/chord-core/src/rest/transport.rs`.

Write scope: `rust/chord-gpui/src/net/mod.rs`; `rust/chord-gpui/tests/http_pool_library_contract.rs`.

Route GPUI requests through the qualified shared client and report real connection reuse; remove duplicate counters-as-pool and obsolete framing only after final consumer migration.

Independent oracle: Server accepted connections, exact endpoint parity and UI cancellation/responsiveness evidence.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/chord-gpui/Cargo.toml --test http_pool_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Route GPUI requests through the qualified shared client and report real connection reuse; remove duplicate counters-as-pool and obsolete framing only after final consumer migration.
- Server accepted connections, exact endpoint parity and UI cancellation/responsiveness evidence.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-HTTP-06

**Remove unused socket/framing infrastructure after transport parity** — P1, implementation, G1 Backend.

Prerequisites: LIB-HTTP-05. Resource lease: `core-http1`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/rest/http1.rs`; `rust/crates/chord-core/src/rest/transport.rs`.

Write scope: `rust/crates/chord-core/src/rest/http1.rs`; `rust/crates/chord-core/src/rest/transport.rs`; `rust/crates/chord-core/tests/http_transport_library_contract.rs`.

Delete only unused socket/framing machinery after every transport consumer is on the qualified client; retain any still-public independently required parser contract.

Independent oracle: Complete consumer inventory, unchanged independent request/response/error corpus and no duplicate side effects.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-core --test http_transport_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Delete only unused socket/framing machinery after every transport consumer is on the qualified client; retain any still-public independently required parser contract.
- Complete consumer inventory, unchanged independent request/response/error corpus and no duplicate side effects.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## PORTAL: Typed Linux portals and D-Bus

Delegate mechanism: Existing ashpd plus typed zbus proxies.

Retain policy: Permission/source selection, restore tokens, cancellation and owned file descriptor lifetime.

Primary references: [source 1](https://docs.rs/ashpd/latest/ashpd/desktop/screencast/index.html), [source 2](https://docs.rs/zbus/latest/zbus/).

### LIB-PORTAL-01

**Freeze real portal lifecycle and descriptor contracts** — P1, contract, G5 Voice/Platform.

Prerequisites: LIB-GOV-01. Resource lease: `portal-slice`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/screencast.rs`; `rust/crates/chord-platform/src/dbus`; `rust/chord-gpui/src/pipewire_portal.rs`.

Write scope: `rust/crates/chord-platform/tests/portal_library_contract.rs`.

Record CreateSession/SelectSources/Start/OpenPipeWireRemote, cancellation, restore tokens, closed sessions and descriptor cleanup.

Independent oracle: Existing portal mocks plus actual private D-Bus and kernel FD tests; include credentials-before-rights leak regression.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-platform --test portal_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Record CreateSession/SelectSources/Start/OpenPipeWireRemote, cancellation, restore tokens, closed sessions and descriptor cleanup.
- Existing portal mocks plus actual private D-Bus and kernel FD tests; include credentials-before-rights leak regression.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-PORTAL-02

**Use the existing ashpd ScreenCast API behind the platform trait** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-PORTAL-01, LIB-GOV-02, LIB-DEP-06. Resource lease: `linux-portal`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/screencast.rs`; `rust/gui-spike/src/voice_plugin.rs`.

Write scope: `rust/crates/chord-platform/src/screencast.rs`; `rust/crates/chord-platform/tests/portal_library_contract.rs`.

Replace Linux ScreenCast request/wire handling with the qualified locked ashpd session/owned-FD API.

Independent oracle: Private-bus full lifecycle, cancellation and identical selected source/restore-token behavior.

Targeted check: `Run portal_library_contract and original screencast/portal contracts.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Replace Linux ScreenCast request/wire handling with the qualified locked ashpd session/owned-FD API.
- Private-bus full lifecycle, cancellation and identical selected source/restore-token behavior.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-PORTAL-03

**Migrate notifications to typed zbus proxies** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-PORTAL-02. Resource lease: `linux-portal`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/notifications.rs`; `rust/crates/chord-platform/src/dbus`.

Write scope: `rust/crates/chord-platform/src/notifications.rs`; `rust/crates/chord-platform/tests/zbus_3_contract.rs`.

Replace only the notifications wire consumer with a typed proxy, preserving signal subscriptions, method errors and lifecycle.

Independent oracle: Independent interface fixtures, real private-bus messages and unchanged domain behavior; track dropped subscriptions and FD ownership.

Targeted check: `Run zbus_3_contract, original notifications contracts and resource cleanup tests.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Replace only the notifications wire consumer with a typed proxy, preserving signal subscriptions, method errors and lifecycle.
- Independent interface fixtures, real private-bus messages and unchanged domain behavior; track dropped subscriptions and FD ownership.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-PORTAL-04

**Migrate tray signals to typed zbus proxies** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-PORTAL-03. Resource lease: `linux-portal`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/tray.rs`; `rust/crates/chord-platform/src/dbus`.

Write scope: `rust/crates/chord-platform/src/tray.rs`; `rust/crates/chord-platform/tests/zbus_4_contract.rs`.

Replace only the tray signals wire consumer with a typed proxy, preserving signal subscriptions, method errors and lifecycle.

Independent oracle: Independent interface fixtures, real private-bus messages and unchanged domain behavior; track dropped subscriptions and FD ownership.

Targeted check: `Run zbus_4_contract, original tray signals contracts and resource cleanup tests.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Replace only the tray signals wire consumer with a typed proxy, preserving signal subscriptions, method errors and lifecycle.
- Independent interface fixtures, real private-bus messages and unchanged domain behavior; track dropped subscriptions and FD ownership.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-PORTAL-05

**Migrate power and idle signals to typed zbus proxies** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-PORTAL-04. Resource lease: `linux-portal`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/power.rs`; `rust/crates/chord-platform/src/dbus`.

Write scope: `rust/crates/chord-platform/src/power.rs`; `rust/crates/chord-platform/tests/zbus_5_contract.rs`.

Replace only the power and idle signals wire consumer with a typed proxy, preserving signal subscriptions, method errors and lifecycle.

Independent oracle: Independent interface fixtures, real private-bus messages and unchanged domain behavior; track dropped subscriptions and FD ownership.

Targeted check: `Run zbus_5_contract, original power and idle signals contracts and resource cleanup tests.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Replace only the power and idle signals wire consumer with a typed proxy, preserving signal subscriptions, method errors and lifecycle.
- Independent interface fixtures, real private-bus messages and unchanged domain behavior; track dropped subscriptions and FD ownership.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-PORTAL-06

**Unify portal consumers and approve wire-parser removal** — P1, review, Technical Review.

Prerequisites: LIB-PORTAL-07. Resource lease: `portal-slice`.

Requested ECC profile: `tech_reviewer`; declared model `gemini-4-argon-high` / `high` (runtime not verified).

Read scope: `rust/chord-gpui/src/pipewire_portal.rs`; `rust/gui-spike/src/voice_plugin.rs`; `rust/crates/chord-platform/src/dbus`.

Write scope: `waves/RW-51/REVIEW-PORTAL.md`.

Prove both renderers use the shared provider, all descriptor/session consumers migrated and handwritten wire modules are safe to delete.

Independent oracle: Independent consumer inventory and native Linux capture/cancel/reconnect evidence; original leakage mutations remain detected.

Targeted check: `Original portal/FD/browser contracts and five mutations.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Prove both renderers use the shared provider, all descriptor/session consumers migrated and handwritten wire modules are safe to delete.
- Independent consumer inventory and native Linux capture/cancel/reconnect evidence; original leakage mutations remain detected.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-PORTAL-07

**Route both renderers to the shared portal and remove unused wire code** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-PORTAL-05. Resource lease: `linux-portal`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/chord-gpui/src/pipewire_portal.rs`; `rust/gui-spike/src/voice_plugin.rs`; `rust/crates/chord-platform/src/dbus`.

Write scope: `rust/chord-gpui/src/pipewire_portal.rs`; `rust/crates/chord-platform/src/dbus`; `rust/crates/chord-platform/src/lib.rs`; `rust/gui-spike/src/voice_plugin.rs`.

Apply the independently approved shared-provider integration and delete wire internals only after all four real consumers migrated.

Independent oracle: Exact consumer inventory, both renderer portal behavior, private-bus methods/signals and retained FD leak regressions.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-platform --test portal_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply the independently approved shared-provider integration and delete wire internals only after all four real consumers migrated.
- Exact consumer inventory, both renderer portal behavior, private-bus methods/signals and retained FD leak regressions.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## SECRET: Native secret storage

Delegate mechanism: keyring-core plus explicit secure platform backend crates.

Retain policy: Service/account identity, migration, redaction and no plaintext fallback.

Primary references: [source 1](https://docs.rs/keyring/latest/keyring/), [source 2](https://docs.rs/keyring-core/latest/keyring_core/).

### LIB-SECRET-01

**Freeze credential identities and migration behavior** — P1, contract, G5 Voice/Platform.

Prerequisites: LIB-GOV-01. Resource lease: `secret-slice`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/keyring.rs`; `rust/crates/chord-platform/src/ffi_secret.rs`.

Write scope: `rust/crates/chord-platform/tests/secret_store_library_contract.rs`.

Record existing service/account/schema lookup, locked/missing items, invalid encoding and secret-redacted failures.

Independent oracle: Ephemeral secure stores and existing credential fixtures; never query user account tokens as an oracle.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-platform --test secret_store_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Record existing service/account/schema lookup, locked/missing items, invalid encoding and secret-redacted failures.
- Ephemeral secure stores and existing credential fixtures; never query user account tokens as an oracle.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-SECRET-02

**Qualify explicit secure backend crates for each OS** — P1, qualification, G5 Voice/Platform.

Prerequisites: LIB-SECRET-01, LIB-GOV-02, LIB-DEP-07. Resource lease: `secret-slice`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/keyring.rs`.

Write scope: `waves/RW-51/KEYRING-DECISION.md`.

Choose real Linux Secret Service, Windows credential and macOS Keychain stores with compatible identity and supported targets.

Independent oracle: Current keyring docs recommend keyring-core plus selected stores for applications; mocks and sample stores are not secure production storage.

Targeted check: `Compile native providers and execute ephemeral-store round trips per OS; record unsupported locked-store conditions.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Choose real Linux Secret Service, Windows credential and macOS Keychain stores with compatible identity and supported targets.
- Current keyring docs recommend keyring-core plus selected stores for applications; mocks and sample stores are not secure production storage.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-SECRET-03

**Migrate the Linux credential provider without identity drift** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-SECRET-02. Resource lease: `platform-keyring`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/keyring.rs`; `rust/crates/chord-platform/src/ffi_secret.rs`.

Write scope: `rust/crates/chord-platform/src/ffi_secret.rs`; `rust/crates/chord-platform/src/keyring.rs`; `rust/crates/chord-platform/src/lib.rs`; `rust/crates/chord-platform/tests/secret_store_linux_contract.rs`.

Use the qualified Linux secure provider, preserve existing saved credential access and make missing/locked-store errors explicit.

Independent oracle: Real ephemeral Linux secure-store lifecycle, original identity fixtures and error-redaction checks.

Targeted check: `Run secret_store_linux_contract natively with cleanup, then the unchanged keyring suite.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use the qualified Linux secure provider, preserve existing saved credential access and make missing/locked-store errors explicit.
- Real ephemeral Linux secure-store lifecycle, original identity fixtures and error-redaction checks.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-SECRET-04

**Migrate the Windows credential provider without identity drift** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-SECRET-03. Resource lease: `platform-keyring`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/keyring.rs`; `rust/crates/chord-platform/src/ffi_secret.rs`.

Write scope: `rust/crates/chord-platform/src/ffi_secret.rs`; `rust/crates/chord-platform/src/keyring.rs`; `rust/crates/chord-platform/src/lib.rs`; `rust/crates/chord-platform/tests/secret_store_windows_contract.rs`.

Use the qualified Windows secure provider, preserve existing saved credential access and make missing/locked-store errors explicit.

Independent oracle: Real ephemeral Windows secure-store lifecycle, original identity fixtures and error-redaction checks.

Targeted check: `Run secret_store_windows_contract natively with cleanup, then the unchanged keyring suite.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use the qualified Windows secure provider, preserve existing saved credential access and make missing/locked-store errors explicit.
- Real ephemeral Windows secure-store lifecycle, original identity fixtures and error-redaction checks.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-SECRET-05

**Migrate the macOS credential provider without identity drift** — P1, implementation, G5 Voice/Platform.

Prerequisites: LIB-SECRET-04. Resource lease: `platform-keyring`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/keyring.rs`; `rust/crates/chord-platform/src/ffi_secret.rs`.

Write scope: `rust/crates/chord-platform/src/ffi_secret.rs`; `rust/crates/chord-platform/src/keyring.rs`; `rust/crates/chord-platform/src/lib.rs`; `rust/crates/chord-platform/tests/secret_store_macos_contract.rs`.

Use the qualified macOS secure provider, preserve existing saved credential access and make missing/locked-store errors explicit.

Independent oracle: Real ephemeral macOS secure-store lifecycle, original identity fixtures and error-redaction checks.

Targeted check: `Run secret_store_macos_contract natively with cleanup, then the unchanged keyring suite.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use the qualified macOS secure provider, preserve existing saved credential access and make missing/locked-store errors explicit.
- Real ephemeral macOS secure-store lifecycle, original identity fixtures and error-redaction checks.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## JSON: Typed bounded serialization

Delegate mechanism: Serde DTOs plus a small bounded duplicate-aware visitor.

Retain policy: Lossless IDs, duplicate rejection, depth/node budgets and error locations.

Primary references: [source 1](https://docs.rs/serde/latest/serde/de/trait.Visitor.html), [source 2](https://docs.rs/serde_json/latest/serde_json/).

### LIB-JSON-01

**Freeze strict parser limits and raw numeric semantics** — P1, contract, G1 Backend.

Prerequisites: LIB-GOV-01. Resource lease: `json-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-json/src/lib.rs`.

Write scope: `rust/crates/chord-json/tests/serde_parity_contract.rs`.

Characterize duplicate keys, depth 128, node budget 4,000,000, u64 limits, exact numeric lexemes, Unicode and byte offsets.

Independent oracle: Existing corpus and independently constructed duplicate/depth/Unicode boundary inputs; Value last-key-wins is not equivalent.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-json --test serde_parity_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Characterize duplicate keys, depth 128, node budget 4,000,000, u64 limits, exact numeric lexemes, Unicode and byte offsets.
- Existing corpus and independently constructed duplicate/depth/Unicode boundary inputs; Value last-key-wins is not equivalent.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-JSON-02

**Qualify a bounded duplicate-aware Serde adapter** — P1, implementation, G1 Backend.

Prerequisites: LIB-JSON-01, LIB-GOV-02, LIB-DEP-08. Resource lease: `json-parser`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-json/src/lib.rs`.

Write scope: `rust/crates/chord-json/src/lib.rs`; `rust/crates/chord-json/tests/serde_parity_contract.rs`.

Use Serde Visitor/DeserializeSeed and an exact raw/number representation while preserving all strict parser limits.

Independent oracle: Frozen duplicate/lexeme/error corpus, fuzz differential checks and measured memory/depth bounds.

Targeted check: `Run serde_parity_contract and original JSON corpus/fuzz tests.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use Serde Visitor/DeserializeSeed and an exact raw/number representation while preserving all strict parser limits.
- Frozen duplicate/lexeme/error corpus, fuzz differential checks and measured memory/depth bounds.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-JSON-03

**Promote existing messages DTOs to shared Rust definitions and migrate GPUI consumers** — P1, implementation, G1 Backend.

Prerequisites: LIB-JSON-02. Resource lease: `shared-dto-cutover`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/data.rs`; `rust/gui-spike/src/data.rs`; `rust/chord-gpui/src/net/mod.rs`.

Write scope: `rust/chord-gpui/src/net/mod.rs`; `rust/crates/chord-core/src/dto/messages.rs`; `rust/crates/chord-core/src/dto/mod.rs`; `rust/crates/chord-core/src/lib.rs`; `rust/crates/chord-core/tests/shared_dto_3_contract.rs`; `rust/gui-spike/src/data.rs`.

Reuse completed RW-50 messages DTOs in a shared core module, preserve Tauri behavior and replace remaining duplicated GPUI decoding.

Independent oracle: Completed RW-50 DTOs and independent payload fixtures; preserve missing/null/unknown policy and string snowflakes.

Targeted check: `Run shared_dto_3_contract plus unchanged endpoint and frontend fixture contracts.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Migrate one messages endpoint family to shared typed decoding and remove duplicate field extraction from its renderer adapter.
- Completed RW-50 DTOs and independent payload fixtures; preserve missing/null/unknown policy and string snowflakes.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-JSON-04

**Promote existing guilds DTOs to shared Rust definitions and migrate GPUI consumers** — P1, implementation, G1 Backend.

Prerequisites: LIB-JSON-03. Resource lease: `shared-dto-cutover`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/permissions.rs`; `rust/gui-spike/src/data.rs`; `rust/chord-gpui/src/net/mod.rs`.

Write scope: `rust/chord-gpui/src/net/mod.rs`; `rust/crates/chord-core/src/dto/guilds.rs`; `rust/crates/chord-core/src/dto/mod.rs`; `rust/crates/chord-core/src/lib.rs`; `rust/crates/chord-core/tests/shared_dto_4_contract.rs`; `rust/gui-spike/src/data.rs`; `rust/gui-spike/src/permissions.rs`.

Reuse completed RW-50 guilds DTOs in a shared core module, preserve Tauri behavior and replace remaining duplicated GPUI decoding.

Independent oracle: Completed RW-50 DTOs and independent payload fixtures; preserve missing/null/unknown policy and string snowflakes.

Targeted check: `Run shared_dto_4_contract plus unchanged endpoint and frontend fixture contracts.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Migrate one guild channels and permissions endpoint family to shared typed decoding and remove duplicate field extraction from its renderer adapter.
- Completed RW-50 DTOs and independent payload fixtures; preserve missing/null/unknown policy and string snowflakes.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-JSON-05

**Promote existing search DTOs to shared Rust definitions and migrate GPUI consumers** — P1, implementation, G1 Backend.

Prerequisites: LIB-JSON-04. Resource lease: `shared-dto-cutover`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/search.rs`; `rust/gui-spike/src/data.rs`; `rust/chord-gpui/src/net/mod.rs`.

Write scope: `rust/chord-gpui/src/net/mod.rs`; `rust/crates/chord-core/src/dto/mod.rs`; `rust/crates/chord-core/src/dto/search.rs`; `rust/crates/chord-core/src/lib.rs`; `rust/crates/chord-core/tests/shared_dto_5_contract.rs`; `rust/gui-spike/src/search.rs`.

Reuse completed RW-50 search DTOs in a shared core module, preserve Tauri behavior and replace remaining duplicated GPUI decoding.

Independent oracle: Completed RW-50 DTOs and independent payload fixtures; preserve missing/null/unknown policy and string snowflakes.

Targeted check: `Run shared_dto_5_contract plus unchanged endpoint and frontend fixture contracts.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Migrate one search results endpoint family to shared typed decoding and remove duplicate field extraction from its renderer adapter.
- Completed RW-50 DTOs and independent payload fixtures; preserve missing/null/unknown policy and string snowflakes.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## IPC: Actual generated Rust IPC and plugin boundaries

Delegate mechanism: Tauri v2, qualified exact-pinned Specta generator and framework tests.

Retain policy: Domain methods, wire compatibility, command permissions, lossless IDs and explicit errors.

Primary references: [source 1](https://specta.dev/docs/tauri-specta), [source 2](https://v2.tauri.app/develop/plugins/), [source 3](https://docs.rs/tauri/latest/tauri/test/fn.get_ipc_response.html).

### LIB-IPC-01

**Inventory the 79 production commands and actual event reachability** — P1, contract, G4 QA.

Prerequisites: LIB-GOV-01. Resource lease: `ipc-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `tests/meta/ipc-inventory-contract.test.mjs`; `waves/RW-51/IPC-INVENTORY.json`.

Record source command/consumer reachability and expected wire shapes as an inventory; runtime invocation coverage belongs to the real builder and per-domain leaves.

Independent oracle: Tauri get_ipc_response/InvokeRequest tests plus independent expected payloads; not source-string registration checks.

Targeted check: `node --test tests/meta/ipc-inventory-contract.test.mjs`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Inventory all 79 commands and actual events including argument casing, unit/null/optional values, errors and permissions.
- Tauri get_ipc_response/InvokeRequest tests plus independent expected payloads; not source-string registration checks.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-02

**Qualify pinned generated bindings without changing runtime routing** — P1, qualification, G1 Backend.

Prerequisites: LIB-IPC-01, LIB-GOV-02, LIB-DEP-09, LIB-IPC-15. Resource lease: `ipc-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/Cargo.toml`; `rust/gui-spike/src/settings_plugin.rs`.

Write scope: `rust/gui-spike/tests/specta_qualification.rs`; `waves/RW-51/IPC-GENERATOR-DECISION.md`.

Prove deterministic settings/error/snowflake binding generation with the locked Tauri release and exact candidate versions.

Independent oracle: Specta primary docs currently pin release-candidate =2.0.0-rc.27; generated wire output must match independent existing fixtures.

Targeted check: `Generate twice with no diff; compile positive and deliberately invalid type fixtures and real IPC requests.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Prove deterministic settings/error/snowflake binding generation with the locked Tauri release and exact candidate versions.
- Specta primary docs currently pin release-candidate =2.0.0-rc.27; generated wire output must match independent existing fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-03

**Generate settings client bindings from Rust DTOs** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-02, LIB-ASSET-02. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/settings_plugin.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `rust/gui-spike/src/main.rs`; `rust/gui-spike/src/settings_plugin.rs`; `rust/gui-spike/src/settings_store.rs`; `rust/gui-spike/tests/ipc_library_contract.rs`; `rust/gui-spike/ui/generated/bindings.js`.

Make Rust the source of settings argument/return types and replace only the settings handwritten client declarations.

Independent oracle: Regeneration produces no diff and runtime success/error/unit/null fixtures remain byte-shape compatible.

Targeted check: `Run IPC contract, generated-client typecheck and browser settings-save/reload fixture.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Make Rust the source of settings argument/return types and replace only the settings handwritten client declarations.
- Regeneration produces no diff and runtime success/error/unit/null fixtures remain byte-shape compatible.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-04

**Register a real settings plugin handler and permissions** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-03. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/settings_plugin.rs`; `rust/gui-spike/src/main.rs`.

Write scope: `rust/gui-spike/capabilities/default.json`; `rust/gui-spike/permissions`; `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/src/settings_plugin.rs`; `rust/gui-spike/tests/ipc_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`.

Use the actual plugin invoke_handler and explicit capabilities while coordinator removes settings forwarding wrappers from main.rs.

Independent oracle: Real allowed/denied plugin invocation and bounded old-name migration tests; Builder::build alone does not register commands.

Targeted check: `Run real IPC plus unchanged settings/UI contracts.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use the actual plugin invoke_handler and explicit capabilities while coordinator removes settings forwarding wrappers from main.rs.
- Real allowed/denied plugin invocation and bounded old-name migration tests; Builder::build alone does not register commands.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-05

**Complete typed authentication IPC registration and remove its wrappers** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-04, LIB-RSA-05. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/auth_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `rust/gui-spike/capabilities/default.json`; `rust/gui-spike/permissions`; `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/auth_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/tests/ipc_5_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`.

Generate the authentication client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.

Independent oracle: The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.

Targeted check: `Run ipc_5_library_contract, generated typecheck and the unchanged authentication contracts/browser journey.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Generate the authentication client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.
- The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-06

**Complete typed network IPC registration and remove its wrappers** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-04. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/net_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `rust/gui-spike/capabilities/default.json`; `rust/gui-spike/permissions`; `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/src/net_plugin.rs`; `rust/gui-spike/tests/ipc_6_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`.

Generate the network client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.

Independent oracle: The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.

Targeted check: `Run ipc_6_library_contract, generated typecheck and the unchanged network contracts/browser journey.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Generate the network client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.
- The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-07

**Complete typed voice IPC registration and remove its wrappers** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-04. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/voice_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `rust/gui-spike/capabilities/default.json`; `rust/gui-spike/permissions`; `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/src/voice_plugin.rs`; `rust/gui-spike/tests/ipc_7_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`.

Generate the voice client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.

Independent oracle: The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.

Targeted check: `Run ipc_7_library_contract, generated typecheck and the unchanged voice contracts/browser journey.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Generate the voice client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.
- The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-08

**Complete typed media IPC registration and remove its wrappers** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-04. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/media_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `rust/gui-spike/capabilities/default.json`; `rust/gui-spike/permissions`; `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/src/media_plugin.rs`; `rust/gui-spike/tests/ipc_8_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`.

Generate the media client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.

Independent oracle: The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.

Targeted check: `Run ipc_8_library_contract, generated typecheck and the unchanged media contracts/browser journey.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Generate the media client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.
- The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-09

**Complete typed invites IPC registration and remove its wrappers** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-04. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/invites_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `rust/gui-spike/capabilities/default.json`; `rust/gui-spike/permissions`; `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/invites_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/tests/ipc_9_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`.

Generate the invites client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.

Independent oracle: The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.

Targeted check: `Run ipc_9_library_contract, generated typecheck and the unchanged invites contracts/browser journey.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Generate the invites client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.
- The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-10

**Complete typed gifts IPC registration and remove its wrappers** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-04. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/gifts_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `rust/gui-spike/capabilities/default.json`; `rust/gui-spike/permissions`; `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/gifts_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/tests/ipc_10_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`.

Generate the gifts client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.

Independent oracle: The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.

Targeted check: `Run ipc_10_library_contract, generated typecheck and the unchanged gifts contracts/browser journey.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Generate the gifts client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.
- The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-11

**Complete typed direct messages IPC registration and remove its wrappers** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-04. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/dm_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `rust/gui-spike/capabilities/default.json`; `rust/gui-spike/permissions`; `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/dm_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/tests/ipc_11_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`.

Generate the direct messages client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.

Independent oracle: The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.

Targeted check: `Run ipc_11_library_contract, generated typecheck and the unchanged direct messages contracts/browser journey.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Generate the direct messages client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.
- The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-12

**Complete typed notifications IPC registration and remove its wrappers** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-04. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/notifications_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `rust/gui-spike/capabilities/default.json`; `rust/gui-spike/permissions`; `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/src/notifications_plugin.rs`; `rust/gui-spike/tests/ipc_12_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`.

Generate the notifications client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.

Independent oracle: The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.

Targeted check: `Run ipc_12_library_contract, generated typecheck and the unchanged notifications contracts/browser journey.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Generate the notifications client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.
- The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-13

**Complete typed search IPC registration and remove its wrappers** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-04. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/search_plugin.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/ui/features/ipc-bindings.js`.

Write scope: `rust/gui-spike/capabilities/default.json`; `rust/gui-spike/permissions`; `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/src/search_plugin.rs`; `rust/gui-spike/tests/ipc_13_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/shell-bridge.js`.

Generate the search client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.

Independent oracle: The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.

Targeted check: `Run ipc_13_library_contract, generated typecheck and the unchanged search contracts/browser journey.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Generate the search client, register the actual handler/capabilities, migrate consumers, then remove this domain forwarding code.
- The frozen 79-command inventory, independent success/error/permission payloads and existing domain fixtures.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-14

**Remove the obsolete handwritten IPC registry after consumer audit** — P1, review, Technical Review.

Prerequisites: LIB-IPC-05, LIB-IPC-06, LIB-IPC-07, LIB-IPC-08, LIB-IPC-09, LIB-IPC-10, LIB-IPC-11, LIB-IPC-12, LIB-IPC-13. Resource lease: `ipc-slice`.

Requested ECC profile: `tech_reviewer`; declared model `gemini-4-argon-high` / `high` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/ipc-bindings.js`; `rust/gui-spike/src/main.rs`.

Write scope: `waves/RW-51/REVIEW-IPC.md`.

Audit actual live/prototype consumers and remove only superseded binding declarations; retain permission/normalization helpers unless independently proved unused.

Independent oracle: Full command inventory equality, actual invoke responses and consumer import scan independent of generator output.

Targeted check: `Full IPC/runtime/browser/typecheck regression suite and five registration/type/permission mutants.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Prove all original commands/events remain mapped, old call aliases are unused and generated registration cannot silently omit a domain.
- Full command inventory equality, actual invoke responses and consumer import scan independent of generator output.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-IPC-15

**Expose the real production Tauri builder to an IPC test harness** — P1, implementation, G1 Backend.

Prerequisites: LIB-IPC-01. Resource lease: `ipc-registry`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/main.rs`; `rust/gui-spike/src/lib.rs`; `rust/gui-spike/capabilities/default.json`.

Write scope: `rust/gui-spike/src/app_builder.rs`; `rust/gui-spike/src/lib.rs`; `rust/gui-spike/src/main.rs`; `rust/gui-spike/tests/ipc_builder_contract.rs`.

Extract a narrowly reusable/mockable application builder used by production main and integration tests, without recreating a second handler registry.

Independent oracle: Real Tauri get_ipc_response/InvokeRequest paths against the production builder and actual allowed/denied command behavior.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/gui-spike/Cargo.toml --test ipc_builder_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Extract a narrowly reusable/mockable application builder used by production main and integration tests, without recreating a second handler registry.
- Real Tauri get_ipc_response/InvokeRequest paths against the production builder and actual allowed/denied command behavior.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## PRIM: Standard identifiers and encodings in core

Delegate mechanism: OS RNG/uuid, base64 and xxhash-rust xxh64.

Retain policy: Existing wire formats, strict acceptance rules and catalog hashing seed/encoding.

Primary references: [source 1](https://docs.rs/uuid/latest/uuid/), [source 2](https://docs.rs/xxhash-rust/latest/xxhash_rust/).

### LIB-PRIM-01

**Replace core entropy and UUID fallback with a fallible OS source** — P0, implementation, G1 Backend.

Prerequisites: LIB-GOV-02, LIB-DEP-11. Resource lease: `prim-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/rest/identity.rs`.

Write scope: `rust/crates/chord-core/src/rest/identity.rs`; `rust/crates/chord-core/tests/identity_rng_library_contract.rs`.

Generate UUIDs from an explicit cross-platform secure entropy source and return errors instead of repeated IDs when /dev/urandom is absent.

Independent oracle: Independent UUID version/variant/format fixtures and an injected entropy failure that must not produce a success ID.

Targeted check: `Run identity_rng_library_contract natively on Linux/Windows/macOS.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Generate UUIDs from an explicit cross-platform secure entropy source and return errors instead of repeated IDs when /dev/urandom is absent.
- Independent UUID version/variant/format fixtures and an injected entropy failure that must not produce a success ID.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-PRIM-02

**Replace the remaining core base64 implementation** — P2, implementation, G1 Backend.

Prerequisites: LIB-GOV-02, LIB-DEP-11. Resource lease: `prim-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/rest/base64.rs`.

Write scope: `rust/crates/chord-core/src/rest/base64.rs`; `rust/crates/chord-core/tests/base64_library_contract.rs`.

Use the approved base64 engine while retaining alphabet, padding, canonicalization and rejection behavior in core.

Independent oracle: RFC4648 independent vectors and existing malformed-input corpus; completed Tauri base64 work is not repeated.

Targeted check: `Run base64_library_contract and unchanged authorization/header contracts.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use the approved base64 engine while retaining alphabet, padding, canonicalization and rejection behavior in core.
- RFC4648 independent vectors and existing malformed-input corpus; completed Tauri base64 work is not repeated.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-PRIM-03

**Replace manual catalog xxh64 without changing message keys** — P2, implementation, G1 Backend.

Prerequisites: LIB-GOV-02, LIB-DEP-11. Resource lease: `prim-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-i18n/src/lib.rs`.

Write scope: `rust/crates/chord-i18n/src/lib.rs`; `rust/crates/chord-i18n/tests/xxh64_library_contract.rs`.

Use xxhash-rust xxh64 with the exact seed, byte order and six-character encoding; retain existing message identifiers.

Independent oracle: Independent catalog/key fixtures; xxh3 or another encoding must fail the tests.

Targeted check: `Run xxh64_library_contract and all existing localization key/catalog contracts.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use xxhash-rust xxh64 with the exact seed, byte order and six-character encoding; retain existing message identifiers.
- Independent catalog/key fixtures; xxh3 or another encoding must fail the tests.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## RUN: Shared typed Rust services and runtime ownership

Delegate mechanism: Existing Tokio tasks, channels and async services.

Retain policy: Domain orchestration, renderer adapters, cancellation and UI responsiveness.

Primary references: [source 1](https://docs.rs/tokio/latest/tokio/).

### LIB-RUN-01

**Inventory runtime and blocking boundaries for one service slice** — P2, qualification, G1 Backend.

Prerequisites: LIB-GOV-01. Resource lease: `run-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/data.rs`; `rust/chord-gpui/src/net/mod.rs`; `rust/crates/chord-core/src/ws.rs`.

Write scope: `waves/RW-51/RUNTIME-BOUNDARIES.md`.

Map current_thread/block_on/spawn boundaries and exact send/history call paths across both frontends before shared-service extraction.

Independent oracle: Observed current thread/runtime identities, cancellation traces and existing send/history outcomes.

Targeted check: `Run a local mock-backed send/history responsiveness and nested-runtime failure probe.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Map current_thread/block_on/spawn boundaries and exact send/history call paths across both frontends before shared-service extraction.
- Observed current thread/runtime identities, cancellation traces and existing send/history outcomes.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-RUN-02

**Implement one typed message service on the managed runtime** — P2, implementation, G1 Backend.

Prerequisites: LIB-RUN-01, LIB-JSON-03, LIB-HTTP-04. Resource lease: `message-service`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/data.rs`; `rust/chord-gpui/src/net/mod.rs`; `rust/crates/chord-core/src`.

Write scope: `rust/crates/chord-core/src/lib.rs`; `rust/crates/chord-core/src/message_service.rs`; `rust/crates/chord-core/tests/message_service_contract.rs`.

Own send/history orchestration in one typed Rust service using the existing managed runtime and explicit cancellation.

Independent oracle: Independent endpoint fixtures and the prior message ordering/error/permission contract.

Targeted check: `Run message_service_contract including cancellation, retry and UI-thread nonblocking checks.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Own send/history orchestration in one typed Rust service using the existing managed runtime and explicit cancellation.
- Independent endpoint fixtures and the prior message ordering/error/permission contract.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-RUN-03

**Route both frontend adapters to the message service** — P2, implementation, G1 Backend.

Prerequisites: LIB-RUN-02. Resource lease: `renderer-service-cutover`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/data.rs`; `rust/chord-gpui/src/net/mod.rs`.

Write scope: `rust/chord-gpui/src/net/mod.rs`; `rust/gui-spike/src/data.rs`; `rust/gui-spike/tests/shared_message_service_contract.rs`.

Remove duplicated send/history/block_on wrappers after both renderers use the shared service with equivalent results.

Independent oracle: Both renderer contracts against the same independent fixture server, including order, cleanup and lost-connection recovery.

Targeted check: `Run core service contracts plus Tauri/GPUI message and real browser fixtures.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Remove duplicated send/history/block_on wrappers after both renderers use the shared service with equivalent results.
- Both renderer contracts against the same independent fixture server, including order, cleanup and lost-connection recovery.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## URL: Standard URL grammar with explicit policy

Delegate mechanism: url::Url and typed origins/query parsing.

Retain policy: Strict raw-input policy, deep-link routing, token byte preservation and security allowlists.

Primary references: [source 1](https://docs.rs/url/latest/url/).

### LIB-URL-01

**Freeze raw-input navigation and deep-link policy** — P2, contract, G5 Voice/Platform.

Prerequisites: LIB-GOV-01. Resource lease: `url-slice`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/deep_link.rs`.

Write scope: `rust/crates/chord-platform/tests/url_library_contract.rs`.

Record custom schemes, encoded separators, credentials, IDNA, empty hosts, traversal and exact token/query bytes.

Independent oracle: Existing clean-room security corpus and independent expected allow/deny results, not Url parser acceptance.

Targeted check: `Run url_library_contract and unchanged deep-link security tests.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Record custom schemes, encoded separators, credentials, IDNA, empty hosts, traversal and exact token/query bytes.
- Existing clean-room security corpus and independent expected allow/deny results, not Url parser acceptance.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-URL-02

**Replace standards grammar while retaining explicit allowlists** — P2, implementation, G5 Voice/Platform.

Prerequisites: LIB-URL-01, LIB-GOV-02, LIB-DEP-11. Resource lease: `deep-link`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/deep_link.rs`.

Write scope: `rust/crates/chord-platform/src/deep_link.rs`; `rust/crates/chord-platform/tests/url_library_contract.rs`.

Use typed URL/origin/query operations but preserve stricter raw checks and token/query representation where contractual.

Independent oracle: Frozen raw corpus; a normalized/parseable URL may still be forbidden by navigation policy.

Targeted check: `Run url_library_contract and malformed/encoded-origin regression tests.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use typed URL/origin/query operations but preserve stricter raw checks and token/query representation where contractual.
- Frozen raw corpus; a normalized/parseable URL may still be forbidden by navigation policy.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-URL-03

**Review parser deletion and preserve unrelated sandbox checks** — P2, review, Technical Review.

Prerequisites: LIB-URL-02. Resource lease: `url-slice`.

Requested ECC profile: `tech_reviewer`; declared model `gemini-4-argon-high` / `high` (runtime not verified).

Read scope: `rust/crates/chord-platform/src/deep_link.rs`.

Write scope: `waves/RW-51/REVIEW-URL.md`.

Approve deletion of duplicated grammar only, retaining MP4/disclip sandbox and domain navigation checks sharing the module.

Independent oracle: Independent allowlist/path traversal mutations and complete old deep-link contract inventory.

Targeted check: `Original platform security suite and five mutations.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Approve deletion of duplicated grammar only, retaining MP4/disclip sandbox and domain navigation checks sharing the module.
- Independent allowlist/path traversal mutations and complete old deep-link contract inventory.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## ASSET: Deterministic local UI dependency loading

Delegate mechanism: A qualified small build tool and normal module resolution.

Retain policy: Existing Tauri/browser fixture assets, CSP/offline behavior and entry points.

Primary references: [source 1](https://v2.tauri.app/start/frontend/).

### LIB-ASSET-01

**Qualify the smallest offline asset-build pipeline** — P2, qualification, DevOps / Integration.

Prerequisites: LIB-GOV-02. Resource lease: `asset-slice`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `package.json`; `package-lock.json`; `rust/gui-spike/tauri.conf.json`; `rust/gui-spike/ui/index.html`.

Write scope: `waves/RW-51/UI-ASSET-DECISION.md`.

Compare a minimal bundler setup against current raw ESM, choose exact dependency pins and account for build/asset/startup cost.

Independent oracle: Real WebView asset resolution and existing browser fixture loading; bare npm imports cannot work in raw frontendDist ui.

Targeted check: `Build candidate twice offline after locked install, compare assets and verify CSP/no CDN.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Compare a minimal bundler setup against current raw ESM, choose exact dependency pins and account for build/asset/startup cost.
- Real WebView asset resolution and existing browser fixture loading; bare npm imports cannot work in raw frontendDist ui.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-ASSET-02

**Integrate locally bundled assets with Tauri and fixtures** — P2, implementation, DevOps / Integration.

Prerequisites: LIB-ASSET-01, LIB-DEP-10. Resource lease: `ui-assets`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/gui-spike/tauri.conf.json`; `tests/helpers/ui-fixture.mjs`.

Write scope: `rust/gui-spike/tauri.conf.json`; `scripts/build-ui.mjs`; `tests/helpers/ui-fixture.mjs`; `tests/meta/ui-assets-contract.test.mjs`.

Load deterministic local dependency assets in production and the same real HTML fixture, preserving fonts/images/CSS and source tests.

Independent oracle: Offline real browser/WebView requests and missing-asset failure fixtures; assets must not come from runtime network/CDN.

Targeted check: `Run ui-assets-contract plus all 10 non-live browser fixture files on Linux and Windows.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Load deterministic local dependency assets in production and the same real HTML fixture, preserving fonts/images/CSS and source tests.
- Offline real browser/WebView requests and missing-asset failure fixtures; assets must not come from runtime network/CDN.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## SIGNAL: Real reactive primitives in the retained DOM frontend

Delegate mechanism: @preact/signals-core.

Retain policy: Domain stores/actions, lifecycle policy and existing observable UI state.

Primary references: [source 1](https://github.com/preactjs/signals/blob/main/packages/core/README.md).

### LIB-SIGNAL-01

**Freeze store subscriptions and disposal behavior** — P2, contract, G2 Frontend.

Prerequisites: LIB-GOV-01. Resource lease: `signal-slice`.

Requested ECC profile: `g2_frontend`; declared model `gemini-3.8-flash-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/reactive-store.js`; `rust/gui-spike/ui/features/reactive-components.js`.

Write scope: `tests/unit/signals-library-contract.test.mjs`.

Record nested batch, computed dependency switching, initial subscriptions, callback errors and effect disposal before replacing the engine.

Independent oracle: Independent event/order expectations and leak counters at mount/unmount boundaries.

Targeted check: `node --test tests/unit/signals-library-contract.test.mjs`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Record nested batch, computed dependency switching, initial subscriptions, callback errors and effect disposal before replacing the engine.
- Independent event/order expectations and leak counters at mount/unmount boundaries.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-SIGNAL-02

**Replace the homemade signal engine with the real package** — P2, implementation, G2 Frontend.

Prerequisites: LIB-SIGNAL-01, LIB-ASSET-02. Resource lease: `dom-reactivity`.

Requested ECC profile: `g2_frontend`; declared model `gemini-3.8-flash-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/reactive-store.js`.

Write scope: `rust/gui-spike/ui/features/reactive-store.js`; `tests/unit/signals-library-contract.test.mjs`.

Delegate signal/computed/effect/batch to the real package and retain only application-specific store/actions.

Independent oracle: Frozen subscription/batch/error cases plus actual consumer updates using locally resolved assets.

Targeted check: `Run signals-library-contract and affected UI/browser tests.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Delegate signal/computed/effect/batch to the real package and retain only application-specific store/actions.
- Frozen subscription/batch/error cases plus actual consumer updates using locally resolved assets.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.
- Additional benefit to demonstrate (not assumed): Reliable dynamic dependency tracking and disposed effect lifecycle.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-SIGNAL-03

**Remove redundant reactive adapters after consumer migration** — P2, implementation, G2 Frontend.

Prerequisites: LIB-SIGNAL-02. Resource lease: `dom-reactivity`.

Requested ECC profile: `g2_frontend`; declared model `gemini-3.8-flash-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/reactive-components.js`; `rust/gui-spike/ui/app.js`.

Write scope: `rust/gui-spike/ui/features/reactive-components.js`; `tests/unit/signals-library-contract.test.mjs`.

Migrate one consumer group per lease and remove generic compatibility wrappers after lifecycle parity is proven.

Independent oracle: Browser mount/update/unmount scenarios, listener/effect counts and unchanged keyboard focus behavior.

Targeted check: `Run targeted store/component tests and affected real browser fixtures.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Migrate one consumer group per lease and remove generic compatibility wrappers after lifecycle parity is proven.
- Browser mount/update/unmount scenarios, listener/effect counts and unchanged keyboard focus behavior.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## FLOAT: Real anchored overlay positioning

Delegate mechanism: @floating-ui/dom.

Retain policy: Popover visibility, focus, keyboard, permissions and content.

Primary references: [source 1](https://floating-ui.com/docs/computeposition), [source 2](https://floating-ui.com/docs/autoupdate).

### LIB-FLOAT-01

**Freeze overlay geometry and lifecycle contracts** — P2, contract, G2 Frontend.

Prerequisites: LIB-GOV-01. Resource lease: `float-slice`.

Requested ECC profile: `g2_frontend`; declared model `gemini-3.8-flash-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/floating-popover.js`.

Write scope: `tests/e2e/floating-library-contract.test.mjs`.

Record viewport-edge, scrolling ancestor, zoom/resize, RTL, hidden/show, keyboard-focus and dismiss-cleanup behavior.

Independent oracle: Measured DOM rectangles and accessibility/focus outcomes in real browser, not source-string API checks.

Targeted check: `Run floating-library-contract in real Chrome on Linux and Windows.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Record viewport-edge, scrolling ancestor, zoom/resize, RTL, hidden/show, keyboard-focus and dismiss-cleanup behavior.
- Measured DOM rectangles and accessibility/focus outcomes in real browser, not source-string API checks.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-FLOAT-02

**Use async Floating UI geometry with mounted-only updates** — P2, implementation, G2 Frontend.

Prerequisites: LIB-FLOAT-01, LIB-ASSET-02. Resource lease: `overlay-consumers`.

Requested ECC profile: `g2_frontend`; declared model `gemini-3.8-flash-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/floating-popover.js`; `rust/gui-spike/ui/app.js`.

Write scope: `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/floating-popover.js`; `tests/e2e/floating-library-contract.test.mjs`.

Replace handwritten offset/flip/shift with computePosition and cleanly dispose autoUpdate on close.

Independent oracle: Frozen geometry/accessibility cases and stale async-result-after-dismiss regression.

Targeted check: `Run floating-library-contract and existing menu/dialog journeys.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Replace handwritten offset/flip/shift with computePosition and cleanly dispose autoUpdate on close.
- Frozen geometry/accessibility cases and stale async-result-after-dismiss regression.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.
- Additional benefit to demonstrate (not assumed): Resize/ancestor-scroll-aware positioning with explicit lifecycle cleanup.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## VIRT: Measured virtualization

Delegate mechanism: @tanstack/virtual-core if its qualification beats the existing implementation.

Retain policy: Page cache, loading/permissions, stable scroll anchors and actual renderer.

Primary references: [source 1](https://tanstack.com/virtual/latest/docs/introduction).

### LIB-VIRT-01

**Qualify real virtualization against existing large-list behavior** — P2, qualification, G2 Frontend.

Prerequisites: LIB-GOV-01, LIB-ASSET-02. Resource lease: `virt-slice`.

Requested ECC profile: `g2_frontend`; declared model `gemini-3.8-flash-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/virtualizer.js`; `rust/gui-spike/ui/model.js`; `rust/gui-spike/ui/features/virtual-list.js`.

Write scope: `tests/e2e/virtualizer-library-benchmark.test.mjs`; `waves/RW-51/VIRTUALIZER-DECISION.md`.

Compare variable-height behavior, prepend anchors, keyboard selection and bounded heap on 100k messages and 24,229 members.

Independent oracle: Independent list geometry/state fixtures and measured old-versus-candidate startup/frame/RSS costs; loaded window stays at most 500.

Targeted check: `Run real-browser benchmark with fixed data/hardware metadata; adopt only after reviewed parity and benefit.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Compare variable-height behavior, prepend anchors, keyboard selection and bounded heap on 100k messages and 24,229 members.
- Independent list geometry/state fixtures and measured old-versus-candidate startup/frame/RSS costs; loaded window stays at most 500.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-VIRT-02

**Replace only the qualified range and measurement engine** — P2, implementation, G2 Frontend.

Prerequisites: LIB-VIRT-01. Resource lease: `dom-virtualizer`.

Requested ECC profile: `g2_frontend`; declared model `gemini-3.8-flash-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/virtualizer.js`; `rust/gui-spike/ui/features/virtual-list.js`.

Write scope: `rust/gui-spike/ui/features/virtualizer.js`; `tests/e2e/virtualizer-library-benchmark.test.mjs`.

Delegate range/measurement mechanics while preserving the renderer, page cache, anchor and navigation contracts.

Independent oracle: Old-versus-new browser state/geometry/heap evidence including resizing and prepend/reconnect cases.

Targeted check: `Run unchanged virtual-list/message/member tests and the benchmark.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Delegate range/measurement mechanics while preserving the renderer, page cache, anchor and navigation contracts.
- Old-versus-new browser state/geometry/heap evidence including resizing and prepend/reconnect cases.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## MARK: Shared typed Markdown AST

Delegate mechanism: pulldown-cmark for standard Markdown parsing.

Retain policy: Discord mentions, emoji, spoilers, safe URLs and renderer policy.

Primary references: [source 1](https://docs.rs/pulldown-cmark/latest/pulldown_cmark/).

### LIB-MARK-01

**Freeze Discord Markdown and HTML-safety corpus** — P2, contract, G1 Backend.

Prerequisites: LIB-GOV-01. Resource lease: `mark-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/markdown.js`; `rust/gui-spike/ui/features/mentions.js`; `rust/gui-spike/ui/features/emojis.js`; `rust/crates/chord-ui/src`.

Write scope: `rust/crates/chord-ui/tests/markdown_library_contract.rs`.

Record nesting/Unicode/code-span rules, mentions/emoji/spoilers, raw HTML as text and inert unsafe URL behavior independently.

Independent oracle: Existing clean-room Markdown/mentions fixtures and independently specified safe rendering outputs.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/Cargo.toml -p chord-ui --test markdown_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Record nesting/Unicode/code-span rules, mentions/emoji/spoilers, raw HTML as text and inert unsafe URL behavior independently.
- Existing clean-room Markdown/mentions fixtures and independently specified safe rendering outputs.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-MARK-02

**Qualify a shared Rust AST and batch/cache boundary** — P2, qualification, G1 Backend.

Prerequisites: LIB-MARK-01, LIB-GOV-02, LIB-DEP-12. Resource lease: `mark-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-ui/src`.

Write scope: `rust/crates/chord-ui/src/lib.rs`; `rust/crates/chord-ui/src/markdown_ast.rs`; `rust/crates/chord-ui/tests/markdown_library_contract.rs`.

Use pulldown-cmark standard events plus explicit Discord extensions and measure batch/cache transfer cost before renderer cutover.

Independent oracle: Independent AST/render corpus; raw HTML and unsafe schemes remain non-executable, code spans suppress mention parsing.

Targeted check: `Run Markdown contract and representative large-message parse/IPC benchmarks.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use pulldown-cmark standard events plus explicit Discord extensions and measure batch/cache transfer cost before renderer cutover.
- Independent AST/render corpus; raw HTML and unsafe schemes remain non-executable, code spans suppress mention parsing.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-MARK-03

**Migrate the Tauri Markdown renderer to the shared AST** — P2, implementation, G2 Frontend.

Prerequisites: LIB-MARK-02, LIB-IPC-03, LIB-MARK-05. Resource lease: `markdown-cutover`.

Requested ECC profile: `g2_frontend`; declared model `gemini-3.8-flash-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/markdown.js`; `rust/crates/chord-ui/src/markdown_ast.rs`.

Write scope: `rust/crates/chord-ui/tests/markdown_library_contract.rs`; `rust/gui-spike/ui/app.js`; `rust/gui-spike/ui/features/chat.js`; `rust/gui-spike/ui/features/markdown.js`.

Render the typed shared AST in Tauri and remove only superseded parser mechanics after safe rendering parity.

Independent oracle: Independent corpus, actual renderer output, accessibility and safe URL/code-span behavior.

Targeted check: `Run existing Tauri Markdown/UI contracts and real render/security probes.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Render the typed shared AST in Tauri and remove only superseded parser mechanics after safe rendering parity.
- Independent corpus, actual renderer output, accessibility and safe URL/code-span behavior.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-MARK-04

**Migrate the GPUI Markdown renderer to the shared AST** — P2, implementation, G1 Backend.

Prerequisites: LIB-MARK-03. Resource lease: `markdown-cutover`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/chord-gpui/src/ui/chat_view.rs`; `rust/crates/chord-ui/src/markdown_ast.rs`.

Write scope: `rust/chord-gpui/src/ui/chat_view.rs`; `rust/crates/chord-ui/tests/markdown_library_contract.rs`.

Render the typed shared AST in GPUI and remove only superseded parser mechanics after safe rendering parity.

Independent oracle: Independent corpus, actual renderer output, accessibility and safe URL/code-span behavior.

Targeted check: `Run existing GPUI Markdown/UI contracts and real render/security probes.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Render the typed shared AST in GPUI and remove only superseded parser mechanics after safe rendering parity.
- Independent corpus, actual renderer output, accessibility and safe URL/code-span behavior.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-MARK-05

**Implement a reviewed batched AST transport without per-row async calls** — P2, implementation, G1 Backend.

Prerequisites: LIB-MARK-02, LIB-RUN-02. Resource lease: `markdown-transport`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/features/markdown.js`; `rust/gui-spike/src/data.rs`; `rust/crates/chord-core/src/message_service.rs`.

Write scope: `rust/crates/chord-core/src/message_service.rs`; `rust/gui-spike/src/data.rs`; `rust/gui-spike/tests/markdown_transport_contract.rs`.

Carry precomputed/cached typed AST in the qualified batched message service boundary while preserving synchronous renderer expectations.

Independent oracle: Existing synchronous parse/render consumers, independently frozen AST payloads, batch ordering and actual transfer/startup/heap measurements.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/gui-spike/Cargo.toml --test markdown_transport_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Carry precomputed/cached typed AST in the qualified batched message service boundary while preserving synchronous renderer expectations.
- Existing synchronous parse/render consumers, independently frozen AST payloads, batch ordering and actual transfer/startup/heap measurements.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## EVENT: Typed event completeness before polling removal

Delegate mechanism: Tauri channels/events and Tokio state channels.

Retain policy: Initial snapshot, generation ordering, subscription lifecycle and the proven RTC fallback.

Primary references: [source 1](https://v2.tauri.app/develop/calling-rust/).

### LIB-EVENT-01

**Type one RTC event stream end to end** — P2, contract, G1 Backend.

Prerequisites: LIB-IPC-03. Resource lease: `event-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/net_plugin.rs`; `rust/gui-spike/ui/features/shell-bridge.js`; `rust/gui-spike/ui/app.js`.

Write scope: `rust/gui-spike/tests/rtc_event_library_contract.rs`; `waves/RW-51/RTC-EVENT-CONTRACT.md`.

Specify snapshot-plus-generation/sequence semantics and cover every producer transition, reconnect and unsubscribe race for RTC state.

Independent oracle: The upstream f72739e3 negotiating-hang regression plus independently ordered stale/new event traces.

Targeted check: `Run rtc_event_library_contract and real browser negotiating/recovery fixture.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Specify snapshot-plus-generation/sequence semantics and cover every producer transition, reconnect and unsubscribe race for RTC state.
- The upstream f72739e3 negotiating-hang regression plus independently ordered stale/new event traces.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-EVENT-02

**Replace only proven-complete RTC polling with typed updates** — P2, implementation, G2 Frontend.

Prerequisites: LIB-EVENT-01, LIB-IPC-07, LIB-EVENT-03. Resource lease: `app-js-integration`.

Requested ECC profile: `g2_frontend`; declared model `gemini-3.8-flash-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/ui/app.js`; `rust/gui-spike/src/net_plugin.rs`.

Write scope: `rust/gui-spike/ui/app.js`; `tests/e2e/rtc-event-completeness.test.mjs`.

Adopt the qualified typed event/channel consumer and retain the intervalMs=1000 safety fallback until native completeness evidence passes.

Independent oracle: No missed initial/reconnect/mute/deaf/negotiating transitions under delayed subscription or stale events; fallback restoration remains testable.

Targeted check: `Run RTC completeness fixtures plus native voice state/lifecycle tests.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Adopt the qualified typed event/channel consumer and retain the intervalMs=1000 safety fallback until native completeness evidence passes.
- No missed initial/reconnect/mute/deaf/negotiating transitions under delayed subscription or stale events; fallback restoration remains testable.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-EVENT-03

**Implement the Rust RTC snapshot and sequenced event producer** — P2, implementation, G1 Backend.

Prerequisites: LIB-EVENT-01, LIB-IPC-15. Resource lease: `rtc-producer`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/voice_plugin.rs`; `rust/gui-spike/src/net_plugin.rs`; `rust/crates/chord-core/src/ws.rs`.

Write scope: `rust/crates/chord-core/src/ws.rs`; `rust/gui-spike/src/lib.rs`; `rust/gui-spike/src/net_plugin.rs`; `rust/gui-spike/src/rtc_events.rs`; `rust/gui-spike/src/voice_plugin.rs`; `rust/gui-spike/tests/rtc_event_library_contract.rs`.

Implement the actual typed snapshot/subscription producer for every RTC transition, including generation/sequence, reconnect and cleanup.

Independent oracle: Observed negotiation/state transitions and independent out-of-order/reconnect/subscription-race traces; no existing Rust voice-state-update producer was found.

Targeted check: `flock /tmp/chord-cargo.lock cargo test --locked --manifest-path rust/gui-spike/Cargo.toml --test rtc_event_library_contract`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Implement the actual typed snapshot/subscription producer for every RTC transition, including generation/sequence, reconnect and cleanup.
- Observed negotiation/state transitions and independent out-of-order/reconnect/subscription-race traces; no existing Rust voice-state-update producer was found.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## AUDIO: Qualified real-time and native media primitives

Delegate mechanism: Existing native/audio crates and rtrb only for a proven SPSC queue.

Retain policy: DAVE/MLS sessions, stream selection, echo/PID policy and real-time budgets.

Primary references: [source 1](https://docs.rs/rtrb/latest/rtrb/).

### LIB-AUDIO-01

**Prove queue ownership and qualify a bounded SPSC implementation** — P2, qualification, G5 Voice/Platform.

Prerequisites: LIB-GOV-01, LIB-GOV-02, LIB-DEP-13. Resource lease: `audio-slice`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/chord-gpui/src/audio/mod.rs`; `rust/crates/chord-media/src`.

Write scope: `rust/crates/chord-media/tests/audio_queue_library_contract.rs`; `waves/RW-51/AUDIO-QUEUE-DECISION.md`.

Identify exactly one SPSC PCM queue and measure allocation/locking/underflow/overflow behavior against rtrb.

Independent oracle: Actual producer/consumer ownership and callback timing; reject SPSC replacement for latest-frame/multi-reader/overwrite semantics.

Targeted check: `Run allocation/latency queue probes with deterministic overflow/underflow and native audio evidence.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Identify exactly one SPSC PCM queue and measure allocation/locking/underflow/overflow behavior against rtrb.
- Actual producer/consumer ownership and callback timing; reject SPSC replacement for latest-frame/multi-reader/overwrite semantics.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-AUDIO-02

**Migrate only the qualified PCM queue** — P2, implementation, G5 Voice/Platform.

Prerequisites: LIB-AUDIO-01. Resource lease: `audio-pcm`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/chord-gpui/src/audio/mod.rs`; `rust/crates/chord-media/src`.

Write scope: `rust/chord-gpui/src/audio/mod.rs`; `rust/crates/chord-media/tests/audio_queue_library_contract.rs`.

Use the proven bounded queue through a thin callback-safe adapter while preserving silence/drop/recovery policy.

Independent oracle: Existing audio/voice contracts and actual callback allocation/deadline measurements.

Targeted check: `Run audio_queue_library_contract and native loopback/underflow recovery checks.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Use the proven bounded queue through a thin callback-safe adapter while preserving silence/drop/recovery policy.
- Existing audio/voice contracts and actual callback allocation/deadline measurements.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-AUDIO-03

**Qualify capture/codec/RTP replacements before any large cutover** — P2, qualification, G5 Voice/Platform.

Prerequisites: LIB-GOV-01, LIB-GOV-02. Resource lease: `audio-slice`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/chord-gpui/src/video/mod.rs`; `rust/crates/chord-voice/src/session.rs`; `rust/crates/chord-voice/src/srtp.rs`.

Write scope: `waves/RW-51/MEDIA-LIBRARY-MATRIX.md`.

Map capture/codec/RTP mechanics to actual maintained crates and native SDK requirements, retaining all application/protocol policy.

Independent oracle: Existing voice/video wire and hardware evidence; codec/portal library presence is not cross-platform capture support.

Targeted check: `One scoped compile/wire/native qualification probe per candidate; create exact child cards only after a real parity winner.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Map capture/codec/RTP mechanics to actual maintained crates and native SDK requirements, retaining all application/protocol policy.
- Existing voice/video wire and hardware evidence; codec/portal library presence is not cross-platform capture support.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## CLIP: Native clipboard without subprocess glue

Delegate mechanism: Qualified arboard or official Tauri clipboard provider.

Retain policy: PNG/file/URI ownership, names, deadlines and renderer-neutral provider trait.

Primary references: [source 1](https://docs.rs/arboard/latest/arboard/struct.Clipboard.html).

### LIB-CLIP-01

**Qualify native clipboard formats and OS ownership** — P2, qualification, G5 Voice/Platform.

Prerequisites: LIB-GOV-01, LIB-GOV-02, LIB-DEP-14. Resource lease: `clip-slice`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/attachments.rs`.

Write scope: `waves/RW-51/CLIPBOARD-DECISION.md`.

Compare native providers for PNG, URI/file lists, filenames, empty/locked clipboard, Linux ownership and Wayland lifetime.

Independent oracle: Real native clipboard fixtures in disposable sessions; text/image support alone does not prove file-list parity.

Targeted check: `Execute native Linux/Windows/macOS probes and explicit unavailable-environment failure paths.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Compare native providers for PNG, URI/file lists, filenames, empty/locked clipboard, Linux ownership and Wayland lifetime.
- Real native clipboard fixtures in disposable sessions; text/image support alone does not prove file-list parity.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-CLIP-02

**Replace one qualified clipboard backend under the existing trait** — P2, implementation, G5 Voice/Platform.

Prerequisites: LIB-CLIP-01. Resource lease: `native-clipboard`.

Requested ECC profile: `g5_voice_platform`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/gui-spike/src/attachments.rs`.

Write scope: `rust/gui-spike/src/attachments.rs`; `rust/gui-spike/tests/clipboard_library_contract.rs`.

Remove only superseded subprocess mechanics after the chosen native provider proves every required format and timeout.

Independent oracle: Existing ClipboardReader contracts plus independent PNG/file-list and ownership/lifetime evidence.

Targeted check: `Run clipboard_library_contract natively and unchanged attachment paste/browser tests.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Remove only superseded subprocess mechanics after the chosen native provider proves every required format and timeout.
- Existing ClipboardReader contracts plus independent PNG/file-list and ownership/lifetime evidence.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## I18N: Qualified locale mechanisms

Delegate mechanism: Targeted ICU4X components/data only after parity.

Retain policy: Catalog message IDs, pinned locale/fallback policy and Discord formatting rules.

Primary references: [source 1](https://docs.rs/icu/latest/icu/).

### LIB-I18N-01

**Qualify ICU4X outputs and data-size cost** — P3, qualification, G1 Backend.

Prerequisites: LIB-GOV-01, LIB-GOV-02, LIB-DEP-15. Resource lease: `i18n-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-i18n/src`.

Write scope: `waves/RW-51/I18N-LIBRARY-DECISION.md`.

Compare every supported locale formatter/fallback against current independent ICU-derived fixtures and measure binary/data/startup costs.

Independent oracle: Pinned CLDR/ICU provenance, existing locale corpus and generated-table classification; relocation is not handwritten code reduction.

Targeted check: `Run locale differential corpus and component/data-size startup benchmark.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Compare every supported locale formatter/fallback against current independent ICU-derived fixtures and measure binary/data/startup costs.
- Pinned CLDR/ICU provenance, existing locale corpus and generated-table classification; relocation is not handwritten code reduction.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-I18N-02

**Migrate one approved formatter while preserving catalog keys** — P3, implementation, G1 Backend.

Prerequisites: LIB-I18N-01, LIB-PRIM-03. Resource lease: `locale-runtime`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-i18n/src`.

Write scope: `rust/crates/chord-i18n/src/format.rs`; `rust/crates/chord-i18n/src/lib.rs`; `rust/crates/chord-i18n/tests/icu_library_contract.rs`.

Migrate exactly the approved number/date formatting boundary in format.rs with pinned ICU data, unchanged locale fallback and catalog keys.

Independent oracle: Independent all-locale formatter corpus and xxh64 key fixtures, including empty/edge/timezone cases.

Targeted check: `Run icu_library_contract and unchanged localization suites.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Delegate one qualified formatter mechanism with pinned data/fallback semantics and unchanged catalog identifiers.
- Independent all-locale formatter corpus and xxh64 key fixtures, including empty/edge/timezone cases.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## DEVX: Remove duplicated control-plane and diagnostics boilerplate

Delegate mechanism: Existing structured data, tracing and standard library mechanisms.

Retain policy: Stable diagnostics, redaction, auditable task state and project workflows.

Primary references: [source 1](https://docs.rs/tracing/latest/tracing/).

### LIB-DEVX-01

**Replace the duplicated static Kanban seeder with canonical data input** — P2, implementation, DevOps / Integration.

Prerequisites: LIB-GOV-04. Resource lease: `kanban-seeder`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `scripts/populate-kanban-board.mjs`; `waves/kanban-board.json`; `scripts/mcp-kanban-board.mjs`.

Write scope: `scripts/populate-kanban-board.mjs`; `tests/meta/kanban-seeder-contract.test.mjs`.

Remove thousands of duplicated embedded card-data lines and make normal rendering/import preserve all existing/new cards.

Independent oracle: Baseline 220-card equality and unknown-rich-field round trips; rerunning export/import must never reset the board to the old seed.

Targeted check: `node --test tests/meta/kanban-seeder-contract.test.mjs`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Remove thousands of duplicated embedded card-data lines and make normal rendering/import preserve all existing/new cards.
- Baseline 220-card equality and unknown-rich-field round trips; rerunning export/import must never reset the board to the old seed.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEVX-02

**Qualify one structured tracing/error boundary** — P2, qualification, G1 Backend.

Prerequisites: LIB-GOV-01, LIB-GOV-02, LIB-DEP-15. Resource lease: `devx-slice`.

Requested ECC profile: `g1_backend`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `rust/crates/chord-core/src/ws.rs`; `rust/gui-spike/src/main.rs`.

Write scope: `rust/crates/chord-core/tests/tracing_library_contract.rs`; `waves/RW-51/TRACING-DECISION.md`.

Reuse existing tracing/thiserror for one diagnostic boundary without changing stable CLI output or leaking credentials.

Independent oracle: Independent diagnostic/redaction fixtures, disabled-level overhead and error-source preservation.

Targeted check: `Run tracing_library_contract plus unchanged CLI/error contracts.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Reuse existing tracing/thiserror for one diagnostic boundary without changing stable CLI output or leaking credentials.
- Independent diagnostic/redaction fixtures, disabled-level overhead and error-source preservation.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## QA: Cross-platform parity and measured acceptance

Delegate mechanism: Existing test frameworks, native CI runners, Cargo tooling and browser harness.

Retain policy: Independent oracles, release qualification and honest failure reporting.

Primary references: [source 1](https://doc.rust-lang.org/cargo/commands/cargo-test.html).

### LIB-QA-01

**Build the migration contract and mutation matrix** — P1, contract, G4 QA.

Prerequisites: LIB-GOV-01. Resource lease: `qa-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `waves/RW-51/TASKS.json`; `TDD-PLAN.md`.

Write scope: `waves/RW-51/QA-MATRIX.md`.

Map each leaf to applicable nine dimensions, old suites, independently defined regressions and five behavioral mutants.

Independent oracle: Existing tests and feature ledger; a rewritten test matching the new code is not a parity oracle.

Targeted check: `Validate complete feature-to-contract-to-native-target links; identify missing evidence as blocked.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Map each leaf to applicable nine dimensions, old suites, independently defined regressions and five behavioral mutants.
- Existing tests and feature ledger; a rewritten test matching the new code is not a parity oracle.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-QA-02

**Qualify accepted slices on native Linux x86_64/Wayland/X11** — P1, qualification, G4 QA.

Prerequisites: LIB-QA-07. Resource lease: `qa-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `.github/workflows/cross-platform.yml`; `waves/RW-51/QA-MATRIX.md`.

Write scope: `waves/RW-51/QA-NATIVE-2.json`.

Execute the accepted-slice matrix on real Linux x86_64/Wayland/X11, including both renderers and relevant native resource lifecycle.

Independent oracle: Actual runner/OS/architecture/process/window/media/package evidence; cross-compile or fixture IPC is not full native behavior.

Targeted check: `Run locked affected contracts, real fixture/WebView journeys, packaging smoke and relevant native integrations; preserve baseline reds.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Execute the accepted-slice matrix on real Linux x86_64/Wayland/X11, including both renderers and relevant native resource lifecycle.
- Actual runner/OS/architecture/process/window/media/package evidence; cross-compile or fixture IPC is not full native behavior.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-QA-03

**Qualify accepted slices on native Windows x86_64** — P1, qualification, G4 QA.

Prerequisites: LIB-QA-07. Resource lease: `qa-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `.github/workflows/cross-platform.yml`; `waves/RW-51/QA-MATRIX.md`.

Write scope: `waves/RW-51/QA-NATIVE-3.json`.

Execute the accepted-slice matrix on real Windows x86_64, including both renderers and relevant native resource lifecycle.

Independent oracle: Actual runner/OS/architecture/process/window/media/package evidence; cross-compile or fixture IPC is not full native behavior.

Targeted check: `Run locked affected contracts, real fixture/WebView journeys, packaging smoke and relevant native integrations; preserve baseline reds.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Execute the accepted-slice matrix on real Windows x86_64, including both renderers and relevant native resource lifecycle.
- Actual runner/OS/architecture/process/window/media/package evidence; cross-compile or fixture IPC is not full native behavior.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-QA-04

**Qualify accepted slices on native macOS Intel and Apple Silicon** — P1, qualification, G4 QA.

Prerequisites: LIB-QA-07. Resource lease: `qa-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `.github/workflows/cross-platform.yml`; `waves/RW-51/QA-MATRIX.md`.

Write scope: `waves/RW-51/QA-NATIVE-4.json`.

Execute the accepted-slice matrix on real macOS Intel and Apple Silicon, including both renderers and relevant native resource lifecycle.

Independent oracle: Actual runner/OS/architecture/process/window/media/package evidence; cross-compile or fixture IPC is not full native behavior.

Targeted check: `Run locked affected contracts, real fixture/WebView journeys, packaging smoke and relevant native integrations; preserve baseline reds.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Execute the accepted-slice matrix on real macOS Intel and Apple Silicon, including both renderers and relevant native resource lifecycle.
- Actual runner/OS/architecture/process/window/media/package evidence; cross-compile or fixture IPC is not full native behavior.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-QA-05

**Qualify accepted slices on native Linux ARM64 packages and runtime** — P1, qualification, G4 QA.

Prerequisites: LIB-QA-07. Resource lease: `qa-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `.github/workflows/cross-platform.yml`; `waves/RW-51/QA-MATRIX.md`.

Write scope: `waves/RW-51/QA-NATIVE-5.json`.

Execute the accepted-slice matrix on real Linux ARM64 packages and runtime, including both renderers and relevant native resource lifecycle.

Independent oracle: Actual runner/OS/architecture/process/window/media/package evidence; cross-compile or fixture IPC is not full native behavior.

Targeted check: `Run locked affected contracts, real fixture/WebView journeys, packaging smoke and relevant native integrations; preserve baseline reds.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Execute the accepted-slice matrix on real Linux ARM64 packages and runtime, including both renderers and relevant native resource lifecycle.
- Actual runner/OS/architecture/process/window/media/package evidence; cross-compile or fixture IPC is not full native behavior.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-QA-06

**Reconcile reduction, performance and security before release approval** — P1, review, Technical Review.

Prerequisites: LIB-QA-05, LIB-GOV-06, LIB-RSA-04, LIB-CRYPTO-05, LIB-STORE-04, LIB-PORTAL-06, LIB-IPC-14, LIB-QA-02, LIB-QA-03, LIB-QA-04, LIB-QA-07. Resource lease: `qa-slice`.

Requested ECC profile: `tech_reviewer`; declared model `gemini-4-argon-high` / `high` (runtime not verified).

Read scope: `waves/RW-51/FEATURE-PARITY.md`; `waves/RW-51/CODE-REDUCTION.json`; `waves/RW-51/QA-MATRIX.md`.

Write scope: `waves/RW-51/ACCEPT.md`; `waves/RW-51/REVIEW.md`.

Independently accept only fully evidenced slices, measured owned-code reduction and preserved or improved functionality on supported targets.

Independent oracle: Reviewer reproduces critical regressions, LOC accounting, native matrix and security/license/dependency decisions; author cannot self-approve.

Targeted check: `One integration T1/T2/T3 gate per batch; qualification T4 only after all required native/release evidence and zero blocking regressions.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Independently accept only fully evidenced slices, measured owned-code reduction and preserved or improved functionality on supported targets.
- Reviewer reproduces critical regressions, LOC accounting, native matrix and security/license/dependency decisions; author cannot self-approve.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-QA-07

**Freeze the accepted-slice revision manifest before native qualification** — P1, baseline, G4 QA.

Prerequisites: LIB-QA-01, LIB-GOV-05, LIB-GOV-06, LIB-CRYPTO-05, LIB-COMP-03, LIB-STORE-04, LIB-PORTAL-06, LIB-SECRET-05, LIB-JSON-05, LIB-IPC-14, LIB-PRIM-01, LIB-PRIM-02, LIB-RUN-03, LIB-URL-03, LIB-SIGNAL-03, LIB-FLOAT-02, LIB-VIRT-02, LIB-MARK-04, LIB-EVENT-02, LIB-AUDIO-02, LIB-AUDIO-03, LIB-CLIP-02, LIB-I18N-02, LIB-DEVX-01, LIB-DEVX-02, LIB-HTTP-06, LIB-RSA-07, LIB-RSA-08. Resource lease: `qa-slice`.

Requested ECC profile: `g4_qa`; declared model `gemini-4-argon-medium` / `medium` (runtime not verified).

Read scope: `waves/RW-51/TASKS.json`; `waves/RW-51/FEATURE-PARITY.md`.

Write scope: `waves/RW-51/ACCEPTED-SLICES.json`.

Bind each adopted or explicitly retained-by-decision slice to accepted prerequisites, exact Git revision/source hashes, dependency decision and review evidence.

Independent oracle: Actual accepted implementation/decision receipts; reject baseline-only checks and invalidate evidence whenever included source/dependency hashes change.

Targeted check: `Validate manifest hashes at native job start/end and reject any mismatch or missing included slice.`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Bind each adopted or explicitly retained-by-decision slice to accepted prerequisites, exact Git revision/source hashes, dependency decision and review evidence.
- Actual accepted implementation/decision receipts; reject baseline-only checks and invalidate evidence whenever included source/dependency hashes change.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

## DEP: Serialized exact dependency integration

Delegate mechanism: Existing Cargo/npm lock mechanisms and approved upstream artifacts.

Retain policy: One approved library family per task, explicit native/features/export ownership.

Primary references: [source 1](https://doc.rust-lang.org/cargo/reference/resolver.html).

### LIB-DEP-01

**Integrate approved minimal RSA backend dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-RSA-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-core/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved RSA backend candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved RSA backend candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-02

**Integrate approved minimal voice AEAD and HKDF primitives dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-CRYPTO-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-voice/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved voice AEAD and HKDF primitives candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved voice AEAD and HKDF primitives candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-03

**Integrate approved minimal bounded flate2 decoding dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-COMP-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-core/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved bounded flate2 decoding candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved bounded flate2 decoding candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-04

**Integrate approved minimal rusqlite linkage dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-STORE-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-store/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved rusqlite linkage candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved rusqlite linkage candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-05

**Integrate approved minimal reusable HTTP client dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-HTTP-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-core/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved reusable HTTP client candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved reusable HTTP client candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-06

**Integrate approved minimal Linux ashpd and zbus dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-PORTAL-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-platform/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved Linux ashpd and zbus candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved Linux ashpd and zbus candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-07

**Integrate approved minimal explicit secure keyring stores dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-SECRET-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-platform/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved explicit secure keyring stores candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved explicit secure keyring stores candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-08

**Integrate approved minimal strict Serde and shared DTOs dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-JSON-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-core/Cargo.toml`; `rust/crates/chord-json/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved strict Serde and shared DTOs candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved strict Serde and shared DTOs candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-09

**Integrate approved minimal exact-pinned IPC generator dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-IPC-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved exact-pinned IPC generator candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/gui-spike/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved exact-pinned IPC generator candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-10

**Integrate approved minimal local UI build and primitive packages dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-ASSET-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `package.json`; `package-lock.json`.

Write scope: `package-lock.json`; `package.json`.

Apply exactly the approved local UI build and primitive packages candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `npm ci --ignore-scripts`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved local UI build and primitive packages candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-11

**Integrate approved minimal standard IDs and encodings dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-core/Cargo.toml`; `rust/crates/chord-i18n/Cargo.toml`; `rust/crates/chord-platform/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved standard IDs and encodings candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved standard IDs and encodings candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-12

**Integrate approved minimal shared Markdown parser dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02, LIB-MARK-01. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-ui/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved shared Markdown parser candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved shared Markdown parser candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-13

**Integrate approved minimal qualified PCM queue dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-media/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved qualified PCM queue candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved qualified PCM queue candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-14

**Integrate approved minimal native clipboard candidate dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved native clipboard candidate candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/gui-spike/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved native clipboard candidate candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.

### LIB-DEP-15

**Integrate approved minimal targeted ICU and tracing candidates dependencies** — P1, integration, DevOps / Integration.

Prerequisites: LIB-GOV-02. Resource lease: `shared-dependency-integrator`.

Requested ECC profile: `devops`; declared model `gemini-4-argon-low` / `low` (runtime not verified).

Read scope: `rust/Cargo.toml`; `rust/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/gui-spike/Cargo.toml`; `rust/gui-spike/Cargo.lock`.

Write scope: `rust/Cargo.lock`; `rust/Cargo.toml`; `rust/chord-gpui/Cargo.lock`; `rust/chord-gpui/Cargo.toml`; `rust/crates/chord-core/Cargo.toml`; `rust/crates/chord-i18n/Cargo.toml`; `rust/gui-spike/Cargo.lock`; `rust/gui-spike/Cargo.toml`.

Apply exactly the approved targeted ICU and tracing candidates candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.

Independent oracle: Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.

Targeted check: `flock /tmp/chord-cargo.lock cargo metadata --locked --format-version 1 --manifest-path rust/Cargo.toml`.

Follow the shared [worker rules](WORKER-RULES.md); complete standalone instructions are embedded in the JSON/board mandate.

**Acceptance:**

- Apply exactly the approved targeted ICU and tracing candidates candidate pins/features to the qualification scope, refresh coherent locks and record the dependency delta.
- Accepted dependency decision, exact upstream artifact/license/advisory/MSRV/target records and reproducible locked resolution; no arbitrary upgrades.
- Existing observable behavior and strict assertions remain valid; all applicable nine dimensions documented.
- Independent review and targeted checks attached; native target claims require actual native evidence.
- Before/after net owned production code and performance measurements attached; rollback documented.

Dispatch/phase and admission state: see the corresponding TASKS.json card and DISPATCH-RECEIPTS.json; queued work is not automatically executable.
