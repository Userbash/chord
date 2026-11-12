# Standard Engineering Methodology (discord-lab)

Status: Active engineering standard, introduced following wave RW-15 (P2-04). It complements `AGENTS.md` and `PROCESS.md` by establishing **precisely how** we author code to ensure repeatable, verifiable, and secure deliverables. This standard is continuously refined: verified learnings from each wave are codified in §9.

## 1. Core Engineering Methodology

The standard method prioritizes **contracts, verifiable proof, and operational characteristics** over raw typing speed:

```text
research (facts) -> frozen contract -> oracle tests -> red -> minimal green
      -> adversarial/mutation -> batch gate -> independent review -> re-gate (if fixes)
      -> accepted -> journal -> lessons
```

Code is complete not merely when it compiles, but when:
1. Behavior is predicted by an independent oracle (artifact, `jq`, wire capture, mock server).
2. The initial red test verifiably failed for the exact intended behavioral reason.
3. Errors, capacity limits, and edge conditions are handled deterministically without silent fallbacks.
4. The unified batch gate passes prior to code review, and an independent reviewer confirms zero P0/P1/P2 defects.
5. All regression fixes are authored red-first and fully reproducible in the wave journal.

### 1.1 Parallel Waves Without File Contention

Development velocity stems from **disjoint file write-scopes** and pre-frozen contracts rather than uncoordinated agent concurrency:

1. The coordinator conducts research and freezes DESIGN/TZ specifications, defining exact file scopes.
2. Worker agents W<a> / W<b> / W<c> modify strictly their assigned files; shared core parsers/types are modified by a single designated agent.
3. Independent reviewers and mutation testing agents operate in read-only sandboxes on local copies without modifying active code files.
4. The coordinator unifies reports, runs targeted -> core -> workspace -> batch gates, and dispatches parallel code reviews; P0/P1/P2 findings are resolved red-first and trigger a fresh gate run.
5. Documentation and state ledgers are updated once following formal acceptance.

Rule: **Research, independent module tests, and reviews may run in parallel; concurrent edits to the same source file or contract are strictly prohibited.**

### 1.2 Batch Pipeline and Compilation Conservation

- Waves group 3–6 task cards under a single shared integration patch and batch gate.
- Individual workers run only fast targeted tests for their assigned modules; full workspace tests, `clippy`, and `fmt` run once per integrated batch.
- Concurrent cargo build processes on the same target directory are prohibited: builds serialize through workspace file locks.
- Code review occurs **after** the batch gate passes; authors never review their own code.
- Tracking documentation is updated once upon acceptance.
- Staged pipeline: while Batch B(n) is in coding, Batch B(n-1) undergoes review, and Batch B(n-2) completes acceptance.

### 1.3 Mutation-Driven Convergence

Mutation testing serves as an **iterative precision mechanism**:

1. Define fault classes for each contract: boundary operators (`>` <-> `>=`), boolean inversion (`==` <-> `!=`), fallback substitutions, guard removals, credential leakage in formatters, tie-breaking ordering, and unit conversions.
2. Inject a single fault and execute targeted tests. If the test fails, record the failure log and immediately restore the source file. If the test passes, the mutation survived: strengthen the test assertions until the mutant is killed. Never modify production code to adapt to a mutation.
3. Iterate until 100% of planned fault classes on critical paths are killed.
4. Invalidate cached compilation artifacts (`touch` or `cargo clean -p`) following file restoration.
5. Surviving mutations represent genuine test gaps or dead code rather than background noise.

## 2. Architectural Layers and Boundaries

The project enforces **"Pure Functional Core, Imperative Shell"**:

- `discord-core`: Strongly typed domain models, wire parsers, pure state reducer `(State, Event) -> (State, Effects)`, selectors; zero network, filesystem, or UI dependencies.
- `discord-net` / `ws`: Network transports, timeouts, bounded queues, cancellation tokens; isolated from UI.
- `discord-ui`: Reads state snapshots and dispatches typed intents; never blocks on I/O.
- Network I/O, disk storage, audio/video pipelines, and CPU-intensive parsing execute asynchronously on dedicated worker threads.
- Undocumented Discord behaviors are never invented: define an interface, `Effect`, or stub until cleanroom captures confirm wire payloads.

Dependency Invariant: **Outer layers depend on inner layers; inner layers have zero knowledge of outer layers.** Parsers never make network calls; reducers never write to disk; selectors never mutate state.

## 3. Contract Before Code

Prior to authoring production implementation, the following are formally frozen:
- Public types and function signatures in `DESIGN-*.md`.
- State invariants: ordering, offset shifts, resets, and last-wins reconciliation.
- Error taxonomy: fatal, `malformed`, `ignored`, `inconsistencies`, and `duplicates`.
- Scope boundaries and external dependency requirements.
- Verification oracles: exact cleanroom files, line numbers, commands, and expected values.

## 4. Honest Test-Driven Development (TDD)

1. **Red Phase:** The test fails with a verifiable assertion failure. Compilation failures are acceptable for newly declared interfaces but must transition to behavioral assertion failures.
2. **Green Phase:** Author the minimal implementation required to pass the test; avoid speculative refactoring.
3. **Adversarial Phase:** Validate boundary conditions: empty inputs, duplicate records, out-of-range indices, stale sequences, unknown opcodes, integer overflow, and replayed events.
4. **Mutation Phase:** Invalidate a critical check (e.g. remove duplicate detection) and verify that the test fails. If it passes, the test suite is insufficient.
5. **Review Phase:** Independent review; P0/P1/P2 defects are resolved red-first.

Tests must verify against an **independent oracle** rather than current code output. Modifying test expectations to match faulty code is strictly forbidden.

## 5. Error Handling, Diagnostics, and Security

- Zero unhandled `unwrap()`, `expect()`, or `panic!` calls on external untrusted data; zero `unsafe` blocks without formal justification and security review.
- Every error path must either influence output (return code, error counter, diagnostic event) or document why best-effort handling applies. Silent `let _ =` suppression is prohibited.
- Structured diagnostics: `malformed`, `ignored`, `inconsistencies`, `duplicates`, `stale_sequences`. Diagnostic paths remain stable (e.g. `d.ops[0].item`) for automated assertions.
- Credentials (`token`, `session_id`, `cookie`) must never appear in `Debug`, logs, or error responses; public IP addresses are masked as `<PUB_IP>`; raw binary payloads are logged as sha256 checksums.
- Tests default to offline execution. Live network connections require explicit operator approval; missing environments produce explicit skips rather than false failures.

## 6. Performance and Memory Safety

- State indices use `BTreeMap` and `Vec` to ensure deterministic iteration order; avoid non-deterministic hash maps on contractual boundaries.
- Lazy collections are bounded by active viewport windows; the client never caches unbounded server rosters in memory.
- Channel queues, ring buffers, and caches enforce strict capacity limits and timeouts; backpressure handling is built into the architecture.

## 7. Asynchronous I/O and GUI Threading

- Zero blocking I/O calls on the main UI thread.
- Events flow strictly through the reducer queue; UI components never mutate domain state directly.
- Network operations enforce timeouts; retries are restricted to idempotent GET requests; HTTP 429 triggers `Retry-After` backoff; HTTP 401 invalidates session state.
- CPU-intensive tasks (payload parsing, search indexing, cryptography) execute in background thread pools.

## 8. Definition of Done (DoD) per Task Card

- [ ] Cleanroom research documented with verified oracle facts and dependency boundaries.
- [ ] Interface contracts, data models, and file write-scopes frozen in design specifications.
- [ ] Red test failure log captured with non-zero exit code and sha256 test file checksums.
- [ ] Minimal green implementation passing targeted test suites.
- [ ] Code compiles under `cargo fmt --all -- --check` and `clippy -D warnings`.
- [ ] Mutation budget satisfied (100% killed on critical paths); sources restored to exact sha256.
- [ ] Unified batch gate passes cleanly.
- [ ] Independent code review completed with zero P0/P1/P2 defects.
- [ ] Task report and test corrections recorded in wave artifacts.
- [ ] Audit ledger updated in `waves/db.jsonl` and `waves/STATE.md`.
- [ ] Key architectural lessons codified in §9.

## 9. Codified Learnings from Completed Waves

### Wave RW-24 / Batch B05
- **Oracles Overrule Stale Documentation:** Real runtime behavior (`Intl.PluralRules`) takes precedence over copied specification text when locale rules diverge.
- **String Formatting Consistency:** Normalize whitespace variants (U+00A0 vs U+0020) at boundary renderers to ensure bit-exact oracle matches.
- **Exact Plural Matching:** Always evaluate exact `=N` rules prior to applying variable offsets.
- **AST Depth Bounds:** Validate tree recursion limits using structural AST counters rather than relying on absence of stack overflows.

### Wave RW-23 / Batch B04
- **Accurate Packet Layer Naming:** Never label a composite transport packet (e.g. SRTCP with auth tag and counter) as a naked protocol payload without verifying wire layouts.
- **Strict Encoder Validation:** Encoders must return typed `Result` errors on invalid IP addresses, buffer lengths, or reserved ports rather than silently truncating.
- **Inbound Packet Verification:** Always validate source IP and SSRC before parsing UDP payload contents.
- **Automated Typechecking in Gates:** Full static typechecking (`tsc`, `@ts-check`) must run in pre-gate integration to catch unhandled nulls and implicit any types.

### Wave RW-22 / Batch B03
- **External Oracles Required:** Tests must assert against real cleanroom captures rather than round-tripping through internal writer/parser pairs.
- **Targeted Fault Injections:** Ensure mutations trigger the specific behavioral path claimed in test reports.
- **Distinct Fallback Catalogs:** Fallback localization tests require distinct source and target catalogs to avoid tautological passes.
- **Reproduce Before Fixing:** Code review findings must be reproduced via failing regression tests before modifying production code.
- **Masking is Part of the API:** `Debug` formatters for sensitive or unparsed network payloads must mask raw buffers.

### Wave RW-18 / P2-09
- **Presence Scope Precedence:** Real-time online/idle status indicators take precedence over stale offline state.
- **Boundary Diagnostics:** Malformed input must generate structured diagnostic events rather than being silently dropped by internal reducers.
- **State Restoration Invalidation:** Invalidate cached compilation artifacts (`touch` / `cargo clean -p`) following source code file restoration.

### Wave RW-17 / P2-07
- **Field Existence vs Predicates:** The existence of a field (e.g. `badge_count = 0`) does not imply an active badge; filter by logical predicates (`> 0`).
- **Semantic Field Isolation:** Never blend distinct entity identifiers (channel ack IDs vs user ack IDs) through indiscriminate fallbacks.
- **Watermarks vs Stale Guards:** Distinguish monotonic version watermarks from entity-level message sequence guards.

### Wave RW-16 / P2-05
- **Identical Response Keys:** Legitimate sequential responses can share matching query indices; verify correlation IDs before classifying as duplicates.
- **Metadata vs Mutations:** Response metadata (e.g. `not_found` arrays) must not be treated as imperative deletion commands.
- **Defer Unconfirmed Wire Formats:** Retain opaque buffers or typed `Effect` variants until cleanroom captures confirm actual wire schemas.

### Wave RW-15 / P2-04
- **Shift Aware Duplicate Detection:** Member list inserts shifting adjacent elements must evaluate duplicates across the entire active window.
- **Isolate Legacy Artifacts:** Corrupted or legacy captures must be rejected at boundaries rather than normalized with guesswork.
- **Surface Level Review Value:** Resolving P3 review findings eliminates edge-case regression risks across subsequent waves.

### Wave RW-21 / Batch B02
- **Universal Redaction:** Masking is an intrinsic property of all nested public structs, not just aggregate parent types.
- **Isolated Mutation Logs:** Store mutation injection logs separately from production test run logs.
- **Checked Encoding Boundaries:** Implement checked encoding methods for untrusted data to enforce wire limits safely.
- **Explicit Stream Termination:** Mark poisoned or errored stream decoders explicitly to prevent undefined re-entry.

## 10. Standards Governance

Following every accepted wave, coordinators record at most 3–5 verifiable lessons in §9. Lessons recurring across multiple waves are elevated into mandatory architectural rules in §2–§7.
