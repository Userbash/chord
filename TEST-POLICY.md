# TEST-POLICY — Prohibition of Test Weakening and Decisions Bypassing Verification

Status: **Mandatory** across all roles (Product Managers, developers, reviewers, coordinator, owner).  
Date: 2026-10-23. Baseline: Incidents RW-38/LOOP-01, RW-39 (e2e skip, UI without coverage).

---

## 1. Scope

This policy governs every change: source code, tests, fixtures, CSS, markup, automation scripts, configuration, wire contracts, and network modules. "Minor" changes are not exempt.

---

## 2. Absolute Prohibitions (Violation = P0 Blocker, Work Rejected)

1. **Weakening Tests:** Deleting or commenting out `assert` calls, lowering coverage thresholds, broadening tolerance ranges, replacing exact equality with approximate checks, disabling assertions, or increasing timeouts without justification.
2. **Fitting Tests to Code:** Modifying expected values to force a passing status without an underlying contract change.
3. **Concealing Failures:** Using `skip`, `todo`, `only`, empty test bodies, `assert.ok(true)`, wrapping assertions in empty `try/catch` blocks, or substituting assertions with `console.log`.
4. **Disabling Quality Tools:** Using `--no-verify`, bypassing linters or type checkers, using `@ts-ignore` or `eslint-disable` in test suites without written justification, or relaxing `tsconfig` flags to mask errors.
5. **Accepting Work Bypassing Verification (decisions bypassing tests):** Visual inspections without automated commands, screenshots lacking reproducible command logs, verbal assurances that code "should work", reports without execution logs, or undocumented sign-offs.
6. **Merging or Pushing Unverified Code:** Merging code with failing or unexecuted test suites.
7. **Modifying Oracles:** Changing golden references, fixtures, or clean-room dumps without independent provenance: clean-room traces, measured data, formal specifications, or logged architectural decisions in `DECISIONS.md`.
8. **Deleting or Renaming Tests to Hide Failures:** Deleting tests requires a distinct task card with explicit architectural justification and review.

---

## 3. Mandatory Development Cycle (Strict TDD)

1. **RED:** Write a test that fails for the expected reason. Record the command, exit code, error line, and log path.
2. **GREEN:** Write the minimal code necessary to satisfy the test.
3. **TARGETED:** Execute targeted tests for the modified module and adjacent integration boundaries.
4. **GATE:** Run the standard worker gate `npm run gate:worker` (or `gate:release` for release acceptance).
5. **REVIEW:** Author ≠ Reviewer. Reviewers inspect test diffs to ensure no assertions were weakened.
6. **MUTATION:** Verify that control mutations are killed (`5/5`); surviving mutants require remediation.

Production code is never accepted without red-phase verification logs. Mechanical refactoring (formatting, non-semantic renaming) must still pass targeted test suites.

---

## 4. Verification Evidence

Every verification step is documented as:

```text
command → exit code → artifact (path to log / JSON / test report)
```

- Logs must not be modified retroactively; repeated executions produce new timestamped files.
- Screenshots without accompanying reproduction commands do not constitute proof.
- Reports lacking exit codes are invalid.
- For live desktop verifications: process PID, window ID, AT-SPI dump, and entries in `window-runs.jsonl`.

---

## 5. Test Modifications (Canonical Procedure)

Test modification is permitted **only when the underlying specification or contract changes**, following this protocol:

1. **Independent Provenance:** Reference a clean-room trace, empirical measurement, formal specification, or approved ADR.
2. **Audit Table:** Include a diff table: `Old Contract → New Contract → Technical Justification`.
3. **Mutation Proof:** Demonstrate that the updated test detects regressions to the prior implementation.
4. **Independent Review:** The test diff must be reviewed and approved by an engineer other than the author.
5. **Audit Trail:** Record the modification in `DECISIONS.md` (ID, timestamp, rationale, impact).

---

## 6. Waivers and Exemptions (waiver)

- Granted strictly in writing by the Project Owner or Team Lead in `DECISIONS.md` / `WAIVERS.md`.
- Required fields: Scope, expiration date, technical risk, compensating controls, designated verifier.
- **Prohibited Waivers:** No waivers may be granted for P0 security items, new code without unit tests, failing release gates, or missing release regression matrices.
- Expired waivers are treated as policy violations.

---

## 7. Roles and Responsibilities

| Role | Responsibility |
|---|---|
| Author | Strict TDD (red-first), comprehensive test suites, logs, strictly respecting file scopes |
| Reviewer | Audits test diffs, confirms no assertions were weakened, reproduces key commands |
| Product Manager (PM) | Validates command execution, rejects work lacking evidence, maintains decision ledgers |
| Coordinator | Enforces green CI gates and QA verdicts before merging or tagging releases |
| QA Specialist | Validates edge-case matrices, AT-SPI accessibility, and reproduces defects |

---

## 8. Automated Enforcement and Guardrails

- `tests/meta/test-policy.test.mjs` — Automatically prevents `.only`, empty assertions, `todo`, and `--no-verify`, while validating mandatory testing phases.
- Independent Review: Mandatory review pass specifically examining test diffs.
- Continuous Loop Guard: A third identical failing attempt is blocked; developers must revise their approach.
- Policy Violations: Work rejection, rollback of offending test modifications, and escalation.

---

## 9. Definition of Done (DoD)

- [ ] RED phase log (command + error) recorded before implementation.
- [ ] GREEN phase log + targeted tests passing.
- [ ] New behavioral invariants covered; 5/5 control mutations killed.
- [ ] `gate:worker` / `gate:release` passing (skips require explicit justification).
- [ ] Test diff audited and approved by an independent reviewer.
- [ ] No weakened assertions, lowered thresholds, or compromised coverage.
- [ ] Verification evidence attached; file SHA-256 hashes verified.

---

## 10. Prohibited Workarounds / Decisions Bypassing Tests (decisions bypassing tests)

The following practices are invalid and will result in rejected pull requests:
- "Visually verified" without a reproducible command.
- "Disabled the test because it was flaky."
- "Updated assertions to match code because the code is correct."
- "Accepted with failing E2E because E2E is skipped anyway."
- "The PM verbally said it works."
- "Network unavailable" without logging a structured `EnvFail` code.
- Deferring tests with promises to "add tests later after merge."

---

## 11. The Twelve Testing Tiers (`TEST-STRATEGY-V2`)

1. **Unit:** Pure functions, CLI parsers (`args`, `calls`, `csv`, `domains`), core reducers, isolated view helpers.
2. **Integration:** Core and GUI bridge integration, SQLite storage engines, Frida / CDP pipelines.
3. **Contract:** IPC contracts, Gateway streaming contracts (Opcodes `18..=22`), `002-READY.json` schemas, journal integrity.
4. **API (REST/HTTP Mock):** Endpoints, headers (`Authorization`, `X-Super-Properties`), query parameters, and HTTP status codes (`200..5xx`) via local mock servers.
5. **E2E:** End-to-end user journeys in Playwright and native harnesses (authentication, messaging, voice idle, search errors).
6. **UI:** Component layout and interaction states (composer, reactions, friends list, attachment shelf).
7. **Visual:** Responsive breakpoint verification, CSS design tokens, and window geometry under virtual displays (Xvfb).
8. **Accessibility (A11y):** ARIA roles (`dialog`, `tablist`, `alert`), `aria-live`, focus traps, keyboard navigation (`Escape`, `Tab`, `Arrow*`), and AT-SPI D-Bus integration.
9. **Performance:** Ready-payload parse budgets, reducer execution timings (`perf_contract`), and frame rendering budgets (`< 8.33 ms` for 120 FPS).
10. **Security:** OS Keyring storage (`keyring_contract`), XSS prevention (`textContent`, scheme validation), SRTP voice crypto, and SBOM / SHA256SUMS integrity.
11. **Regression:** Targeted regression suites preventing historical defects and race conditions.
12. **Smoke:** Cold startup verification for CLI and GUI binaries with explicit exit codes (`0`/`1`/`2`).

---

## 12. Severity Classification (`P0–P3`) and CI Stages (`T0–T4`)

- **No Priority Downgrades:** Existing test cases must never have their severity demoted; all current suites are blocking (`P0` or `P1`).
- **CI Pipeline Stages:**
  - `T0 (Worker Targeted Gate, 3–8s)`: `node scripts/test-impact.mjs --run` + targeted contract tests.
  - `T1 (PR / Batch Gate -> unstable, 18–25s)`: `npm run gate:worker` (`P0/P1` suites + 5/5 mutation gate).
  - `T2 (Centralized QA Build -> testing, 35–45s)`: Full test suite execution + coverage verification (`90/90/80`).
  - `T3 (Release Gate -> beta/main, 45–60s)`: `npm run gate:release` (`failOnSkips: true`, package checksums, SBOM).
  - `T4 (Nightly / Soak)`: Extended multi-hour soak stability tests and fuzzing targets.

---

## 13. Nine Mandatory Coverage Dimensions per Slice

1. **Negative Scenarios:** Malformed inputs, missing identifiers, graceful recovery (exit codes `1`/`2`).
2. **Boundary Conditions:** Snowflake bounds (`0`/`1`/`u64::MAX`), message character limits (`0`/`2000`/`4000`), file attachments (`10` files, `25 MB ± 1B`), volume bounds (`0..200%`).
3. **Error Handling:** HTTP error responses (`400`, `401`, `403` NSFW, `404`, `413`, `429 retry_after_ms`, `5xx`).
4. **Empty States:** Empty channel list, zero search results, unpopulated friends rosters.
5. **Loading & Processing States:** Skeletons, `aria-busy="true"`, HTTP `202 Accepted` polling loops.
6. **Permissions & Capabilities:** Channel visibility, message transmission rights, administrative privilege flags.
7. **Recovery & Reconnection:** Gateway Resume (Opcode 6) → Identify (Opcode 2), voice stream reconnection, corrupt store fallback.
8. **Backward & Forward Compatibility:** Schema migrations, parsing historical format variations.
9. **Observability & Diagnostics:** Telemetry logging, error stacks, token redacting (`mfa.[REDACTED]`).
