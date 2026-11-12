# TASKS.md — Micro-Decomposition of `discord-lab` Work

Updated: 2026-10-18 (waves W1–W4 closed and accepted: M1 and M2 completed — 176 tests,
172 pass / 0 fail / 4 e2e-skip; coverage 99.3/90.5/97.6; post-W4 audit is in the backlog below).
Updated: 2026-10-19 (waves R7/RC: specification `docs/re/`, Rust client plan `docs/rust-client/`,
fixtures `fixtures/`, skeleton `rust/`; full `npm run check` — 215 tests, 211 pass / 0 fail /
4 e2e-skip; raw artifact verification — 20/20).
Introduced production process `PROCESS.md` (roles, gates `gate:worker/release/qualify`,
spec/report templates) and `AGENTS.md` — concise rules for worker agents. Context and
rules — `TDD-PLAN.md` (§8 — machine-readable table 'test <-> module <-> exports';
meta-control — `tests/meta/plan-sync.test.mjs`).
Updated 2026-10-19: section 'R1 — Gateway Protocol Reconnaissance' (cards; approved by owner).
Summary 2026-10-19: wave R1 closed — statuses, opcode catalog, and artifacts in section 'R1'.
Updated 2026-10-19 (2): section 'R4/R5/R6/D — Feature Transfer Backlog Cards' from references
(Vesktop v1.6.7, discord-qt v0.5.1); complete descriptions and criteria — `docs/feature-backlog.md`.
Updated 2026-10-19 (3): section 'R7 — Deep Reverse Engineering: Transport, Media, Backend, UI, i18n'
(closed; normative docs — `docs/re/`). Wave R7 closed cards D-01–D-03.
Updated 2026-10-19 (4): section 'RC — Rust Client: Concept, Architecture, Plan, Tests'
(documents ready; package — `docs/rust-client/`).
Updated 2026-10-20: batch pipeline (3–6 cards per wave, single gate, review after gate) —
`docs/rust-client/TEAM.md` and `docs/rust-client/WAVE-PLAN.md`; RW-18 (P2-09) accepted,
RW-19/RW-20 (P2-10/P2-11) — active pair B01.
Updated 2026-10-21: B01 accepted (P2-10/P2-11); B02/RW-21 accepted (P2-08, P1-01,
P1-02, P0-04, P0-07); B03/RW-22 accepted (P3-15, P4-01, P7-04, P8-03); B04/RW-23 accepted
(P4-02, P8-04); B05/RW-24 accepted (P7-01, full i18n).
Updated 2026-10-22: B06/RW-25 accepted (P4-03 SRTP crypto, P7-02 perf harness,
P8-01 updater) — first headless wave, offline, without new external dependencies;
Updated 2026-10-22 (2): RW-26 accepted (P4-07 RTCP/RTP statistics, P4-11 regions and
latency) and RW-27 accepted (P7-01b ListFormat, P6-04 deep links, P6-03 autostart/badge) —
headless, without new dependencies; accepted 39/90; next batch — **B07**
(GUI/keyring/SQLite) upon owner decision.

Updated 2026-10-22 (3): RW-28 accepted (P7-01c RelativeTimeFormat — ICU parity +-2,
exact i64 >2^53) and RW-29 accepted (P4-04 RTP media against pcap oracle, P4-12 reconnect,
P7-01e `<t:...>`) — headless, without new dependencies; accepted **43/90**; next batch —
**B07** (GUI/keyring/SQLite) upon owner decision.

Updated 2026-10-22 (4): RW-30 accepted (P7-01d DurationFormat, P6-02 D-Bus notifications) —
headless, without new dependencies; accepted **42/90** (P7-01b/c/d/e tails excluded from 90,
previous counter corrected). Next batch — B07 (GUI/keyring/SQLite) upon owner decision;
headless remaining — P6-06 (power/idle) and P2-03 (SQLite via FFI, see `waves/RW-31/`).

Updated 2026-10-22 (5): RW-31 accepted (P6-06 power/idle, P2-03 SQLite store via FFI to
system libsqlite3 — owner decision A) — headless; accepted **44/90**, remaining 46.
Next batch — B07 (GUI/keyring) upon owner decision; SQLite part of B07 closed.

Updated 2026-10-22 (6): RW-32 accepted (P6-07 spellcheck via FFI libhunspell, P6-02b hints
a{sv}) — headless; accepted **45/90**, remaining 45. Next major headless block —
P1-10 keyring via FFI libsecret (libsecret 0.21 + ksecretd + secret-tool available).

Updated 2026-10-22 (7): RW-33 accepted (P1-10 keyring via FFI libsecret) — headless;
accepted **46/90**, P1 phase closed, remaining 44. B07 blocked only by GUI crates
(keyring and SQLite closed); GUI toolkit choice / live smoke window needed.

Updated 2026-10-23: RW-34 accepted (P6-02c abstract D-Bus, P6-02d hints ext) — extensions
of P6-02, PLAN counter unchanged: **46/90**, remaining 44. fd passing (SCM_RIGHTS) awaits
owner decision on `libc`; B07 — GUI only.

Updated 2026-10-23 (2): RW-35 accepted (P6-06c fd passing D-Bus + logind sleep inhibitor) —
extension of P6-06; `libc` approved; PLAN counter unchanged: **46/90**, remaining 44.
Headless contour closed; going forward — GUI / audio / UI only.

Updated 2026-10-23 (3): RW-36/B07 provisionally accepted — P0-01/P0-06 GUI spike Tauri 2
(4 screens, 100k/24 229/10/38), 60 fps p95 18 ms, cold start 102–137 ms, cold build 29.56 s;
ADR-1 — Tauri accepted, but PSS 394 MB > 300 MB -> follow-up windowed model. **48/90**,
remaining **42**. Artifacts — `waves/RW-36/`, `docs/rust-client/ADR-1.md`.

Updated 2026-10-23 (4): RW-37 accepted — windowed / paginated model `PageStore`
(<=500 rows per list, LRU, on-demand pages), RE contract for layout / pagination.
Tree PSS **393.9 -> 276.2 MB** (<=300), 60 fps p95 18 ms, cold start 142/102 ms;
model.js mutations 12 caught (2 equivalent), targeted 66/66, live smoke 1/1.
PLAN counter unchanged: **48/90**. Artifacts — `waves/RW-37/`, `ADR-1.md` §6.

Updated 2026-11-10: Wave RW-42 (GPUI migration and live parity). Closed cards TASK-701–TASK-740:
resolved VoicePanel collision on user profile (strict 52px container, single-line ellipsis, zero-overflow),
concealed fake RTC status when no voice session is active (voice_session = None),
implemented complete modal dialog 'Add Server' (+) with template picker and interactive invite
link/code entry (`discord.gg/...`, Backspace, clipboard paste, and clear input support),
community discovery screen `ExploreView` (compass), bi-directional message sending and channel history
loading via official REST API v9 (`GET /channels/{id}/messages`), dynamic guild member discovery
bypassing HTTP 403 restriction, 55/55 GPUI contracts 100% pass (7.67s). Clean release build target/release/discord-gpui (55 MB).
Artifacts — `waves/RW-42-GPUI-MIGRATION/TASKS-GPUI.md`.

## How to Read

- **ID**: `W<wave>-<group>-<number>` — infrastructure/tests/delegation;
  `M<milestone>-<MODULE>-<number>` — implementation by tests.
- **Statuses**: `[ ]` todo · `[~]` in progress · `[x]` done · `[!]` blocked · `[s]` skip.
- **Owners**: `C` — coordinator (indivisible part: plan, meta-tests, infrastructure,
  integration and acceptance), `W1a…W1d` and `W3a…W3e` — workers of corresponding waves.
- The complete list of test cases lives in test files; here are behavior clusters and references.
  When adding a test case, add a microtask line to the corresponding table.
- Repository: git (`master` branch); parallel writers work in a SINGLE folder
  `.` with non-overlapping file scopes — Level L1 process
  (`PROCESS.md` §2); orca-worktree + branch `w/<task>` — Level L2, activated
  when remote/CI is introduced. Commits are performed by the coordinator.

## Wave Map

```
W0 (C) ──► W1 (W1a…W1d) ──► W1-fix ──► W2 (C) ──► W3 (M1: W3a…W3e)
plan,      test             4 spec     acceptance      core implementation,
meta,      deepening with   defects    & checks        CLI hub, parity
infra      golden data

W3R (hardening: cxxfilt/CDP/args) ⊕ W4 (M2: frida → CLI) ──► W5 (walk engine, M3) ──► W6 (reports, M4)
                          ──► W7 (CI/mutations, M5) ──► W8 (parity, M6)

R1 (gateway recon: WS/ETF/zstd, opcodes, voice) ──► closed 2026-10-19 (artifacts /tmp/opencode/r2-net/)

R2-NET/R3 (media/cached assets/UI traces/crypto) ──► closed 2026-10-19 (artifacts /tmp/opencode/)
R7 (deep RE: gateway, voice, backend/IPC, REST, UI, i18n) ──► closed 2026-10-19 (docs — docs/re/)
RC (Rust client: concept, features, architecture, plan P0–P8, test plan) ──► documents ready 2026-10-19 (docs/rust-client/)
D (data collection: READY, REST methods, UI traces II — D-01…D-03 closed in R7) ⊕ R4 (OS shell, P0) ──► R5 (streaming/audio, P1) ──► R6 (platform, P2)
transfer backlog: docs/feature-backlog.md (41 features; cards below)
```

Delegation criterion: a task goes to a worker if it can be executed in the shared
folder with a non-overlapping file set. Everything tying waves together (plan,
meta-tests, integration, final test runs) remains with `C`.

## W0 — Plan, Meta-Control, Infrastructure (Coordinator)

| ID | Action | Artifact | Status |
| --- | --- | --- | --- |
| W0-01 | Plan audit and update: §4 methods, §7 M0.5, §8 machine readability, §11 workers | `TDD-PLAN.md` | [x] |
| W0-02 | Micro-decomposition (this file): waves, IDs, file matrix, protocol | `TASKS.md` | [x] |
| W0-03 | Meta-test "§8 <-> tests <-> modules <-> exports" | `tests/meta/plan-sync.test.mjs` | [x] |
| W0-04 | Scripts `test:meta`, `check`; coverage thresholds 90/80/90 | `package.json` | [x] |
| W0-05 | Parallel worker mode: Orca Run, dispatching, shared folder instead of copies | Orca Run `run_949f9e36ec2b` | [x] |
| W0-06 | Wave results integration, final `npm run check`, document synchronization | all | [~] |

## W1 — Test Deepening with Golden Data (4 Parallel Workers)

General acceptance for W1: `node --test <assigned files>` — all failures are only "Not implemented: ...";
`npx tsc -p tsconfig.json --noEmit` — 0 errors; report with provenance (artifact + md5 + oracle)
and commands; only assigned files modified.

| ID | Owner | Target | Owns Files | Status |
| --- | --- | --- | --- | --- |
| W1a | Worker 1 | Deepening `calls`: golden logs, CSV parity, stress cases | `tests/unit/calls.test.mjs`, `tests/smoke/summarize.smoke.test.mjs` | [x] |
| W1b | Worker 2 | Deepening `ui`: profiles top-8, blacklist matrix, walk-log | `tests/unit/{profile,blacklist,walk-log}.test.mjs` | [x] |
| W1c | Worker 3 | Deepening `report`: nm POSIX parser, domains classification, cxxfilt | `tests/unit/{nm,domains}.test.mjs`, `tests/smoke/cxxfilt.smoke.test.mjs` | [x] |
| W1d | Worker 4 | Deepening `core`: CLI args, CSV escaping, paths, inventory, processes | `tests/unit/{args,csv,paths,processes,inventory}.test.mjs` | [x] |

## W1-fix — Specification Defect Resolution Found During Acceptance

| ID | Owner | Defect | Fix | Status |
| --- | --- | --- | --- | --- |
| W1-fix-01 | W1d | `paths`: step 24 test referenced non-existent caption/file (`ui-menu-24-…-Utilities.png`); prefix tied to step numbers | real captions from golden logs (`419c869c…` / `a3683bf1…`), invariant: `_` for punctuation, slug capped at 40 chars | [x] |
| W1-fix-02 | W1b | `profile`: assert `getBoundingClientRect hits === 5` unreachable in top-8; `blacklist`: "Sign out first" -> false against substring rule; `walk-log`: vacuum shouldBail case | golden top-8 from logs (771/697 oracle); substring -> true; explicit zero error records | [x] |
| W1-fix-03 | W1c | `domains`: missing header-only fixtures dtls/ice/stun -> ENOENT after implementation | added 3 files (header md5 `97f6689a…`) | [x] |

## W2 — Acceptance of Wave W1 (Coordinator) — Closed

| ID | Action | Status |
| --- | --- | --- |
| W2-01 | Scope of edits: `find -newer` + diff against pristine copies — assigned files only | [x] |
| W2-02 | Independent verification of golden data with own oracle (jq/awk/c++filt/nm, md5, byte comparisons) | [x] |
| W2-03 | Final test run in main worktree (`npm run check`: typecheck+meta green; 153 tests — 7 pass / 142 fail / 4 skip) | [x] |
| W2-04 | Determinism (repeated runs), §8 update, counter and status updates in TASKS | [x] |

## W3 — Implementation of M1 by Workers

| ID | Worker | Implementation Files | Covers | Status |
| --- | --- | --- | --- | --- |
| W3a | W1d Terminal | `src/core/{args,csv,paths,processes,inventory}.mjs` | M1-ARGS…M1-INV | [x] |
| W3b | W1c Terminal | `src/core/calls.mjs`, `src/core/report/{nm,domains}.mjs`, `src/adapters/cxxfilt.mjs` | M1-CALLS…M1-CXXFILT | [x] |
| W3c | W1b Terminal | `src/core/ui/{profile,blacklist,walk-log}.mjs` | M1-PROFILE/BLACK/WALK | [x] |
| W3d | New Worker | `src/core/cdp/client.mjs` | M1-CDP-01…07 | [x] |
| W3e | W1c Terminal (after W3a–W3d) | `src/cli/run.mjs` (bin unchanged) | M1-CLI, M1-SUM | [x] |

Wave rule: code is written strictly to satisfy tests; tests are never modified (spec defect is
escalated to coordinator via `ask`); after every pair of microtasks, run targeted tests; W3e
starts once dependencies (`calls`, `cdp`) are implemented.

Acceptance of W3 (coordinator): scope of edits — exactly 14 `src` files (13 from W3a–W3d +
`run.mjs`, plus doc edit by coordinator in `run.mjs` after acceptance); `bin/discord-lab.mjs`
and `src/core/not-implemented.mjs` untouched; tests untouched by workers. `npm run check`:
typecheck + meta + 153 tests — 148 pass / 0 fail / 5 skip (pids — honest skip until M2
via code 2 + "Frida unavailable"; 4 e2e — skip-aware without browser/live CDP).
`npm run test:coverage`: thresholds 90/80/90 passed (actual 100 / 100 / 90.48; report
counts only modules loaded by unit tests — gate extension in M5-COV-01).

## W3-specfix — Specification Fixes Identified During Implementation

Both defects were escalated by workers via `ask` without adjusting tests to fit code — protocol functioned as designed;
corrections applied by coordinator with dual oracle verification and documented here.

| ID | Defect | Fix | Status |
| --- | --- | --- | --- |
| W3-specfix-01 | `blacklist`: test case "does not block signatures from real logs" required `false` for all 108 records, but strict union rule must block 3 real signatures | test case updated to "blocks exactly three exception signatures...": "Account Status" (settings-4), "Ignore and block" (settings-26), "Server Menu 2 -> Hide muted channels" (menus-38); oracle — union-regex across all 108 records of both fixtures; remaining 105 records must be allowed | [x] |
| W3-specfix-02 | `domains`: expected string `latency` had 13 spaces versus 12 in printf format and actual `functions/domains/summary.txt` | fixed exactly one space in `tests/unit/domains.test.mjs`; after fix 12/12 green; oracles: `printf '%-12s %8d'` + bytes of `summary.txt` | [x] |

## W3-review — Code Review Findings & Hardening Wave

Review 2026-10-18 (full report: `/tmp/opencode/review-w3/security-review.md`; performed
by coordinator — indivisible analytics). Graphs: SocratiCode — 55 files / 44 edges / 0 cycles;
actual call logs are flat (caller is not logged) — true call graph requires
backtrace (tracked in TDD-PLAN §10). No critical vulnerabilities (injection/eval/ReDoS) found: `spawn` without
shell, `JSON.parse` on untrusted input wrapped in try, paths sanitized; findings focus on robustness.
TDD Rule: for every item, write a red test case -> implement minimal fix.

| ID | Finding | File | Case (red) | Priority | Status |
| --- | --- | --- | --- | --- | --- |
| W3R-HARD-01 | `demangle`: no timeout (hang confirmed by test run); on early exit of `c++filt` (EPIPE, input > channel buffer), writing to `child.stdin` without 'error' handler causes uncaught exception | `src/adapters/cxxfilt.mjs` | `tests/smoke/cxxfilt.smoke.test.mjs` (fake c++filt via PATH) | P1 | [x] W4h1 |
| W3R-HARD-02 | `listTargets`: fetch without timeout; `response.json()` without array check | `src/core/cdp/client.mjs` | `tests/unit/cdp.test.mjs` | P1 | [x] W4h2 |
| W3R-HARD-03 | CDP `events`: unbounded growth + no event window per step (M3: `consoleIssues` counts full session) | `src/core/cdp/client.mjs` | `tests/unit/cdp.test.mjs` | P1 (M3 design) | [x] W4h2 |
| W3R-HARD-04 | `send()`: synchronous throw in `ws.send` after close leaves pending+timer until timeout | `src/core/cdp/client.mjs` | `tests/unit/cdp.test.mjs` | P2 | [x] W4h2 |
| W3R-HARD-05 | `parseArgs`: reserved keys (`_`, `modules`, `__proto__`, `constructor`) overwrite result structure | `src/core/args.mjs` | `tests/unit/args.test.mjs` | P2 | [x] W4h3 |
| W3R-HARD-06 | `packageVersion` without try; `parseCallLog`/`parseVisited` without corrupted line counter; missing `engines` in `package.json` | `src/cli/run.mjs`, `src/core/calls.mjs`, `src/core/ui/walk-log.mjs`, `package.json` | respective unit tests | P3 | [ ] |
| W3R-PLAN-01 | TDD-PLAN §11: workers "in project copies" — divergence from actual protocol (shared folder) | `TDD-PLAN.md` | — (docs, coordinator) | — | [x] |

Summary of wave W4 (2026-10-18, coordinator acceptance): HARD-01…05 closed — cxxfilt
(`timeoutMs=30000` + SIGKILL on timeout + stdin `'error'` handling; 6/6 smoke), CDP
(timeouts for `listTargets`/`send`, target array validation, event window `mark`/`eventsSince`,
`drainEvents` in-place flush, rejection of `send` after `close`; 23/23 unit), args (RESERVED keys;
18/18). Full acceptance: `npm run check` — 176 tests (172 pass / 0 fail / 4 e2e-skip);
coverage 99.30/90.45/97.56; tsc 0; md5 audit — modifications restricted to assigned files. HARD-06
(P3) remains in backlog. W4b inquiry regarding stub contract is valid: contracts strengthened with
`assert.doesNotMatch(/will appear in M2/)` (`tests/smoke/hook.smoke.test.mjs`, `inventory.smoke.test.mjs`).
Terminal retention: all four worker terminals retained for M3.

## M1 — Pure Domain Core: Implementation Microtasks

Format: `M1-<MODULE>-<number>` — one behavior cluster (cases reside in test files).
TDD Rule: task is closed only with a passing green test; red test run prior to code is mandatory.

### M1-ARGS — `src/core/args.mjs` (`parseArgs`)

| ID | Microtask | Cases in `tests/unit/args.test.mjs` |
| --- | --- | --- |
| M1-ARGS-01 | Base: empty argv -> `_`/`modules` empty; positionals in `_` in order | "empty argv", "positional tokens" |
| M1-ARGS-02 | Pairs `--key value` and `--key=value` | "--key value consumes...", "--key=value..." |
| M1-ARGS-03 | Flags: `--flag` -> `true`; `--key=` -> `""`; flag before flag does not consume | "--flag without value", "--key=", "flag before another flag" |
| M1-ARGS-04 | `--module` repeats and accumulates into `modules` (`module` key not present) | "--module repeats..." |
| M1-ARGS-05 | Numbers as strings; values with regex/slashes; continuation after pair; repeated key occurrence (W1d) | "numeric values...", "value may contain...", "parsing continues...", "repeat of standard key..." |

### M1-CSV — `src/core/csv.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-CSV-01 | `safeName`: alphanumeric/`.`/`-`/`_` preserved; other -> `_`; non-ascii -> `_` | 3 cases `safeName` (+W1d real names) |
| M1-CSV-02 | `csv`: all cells quoted, numbers -> strings, quotes escaped by doubling | 2 cases `csv` (+W1d inventory header) |
| M1-CSV-03 | `csv`: commas and newlines do not break cell; empty table -> `""` | 2 cases `csv` |

### M1-PATHS — `src/core/paths.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-PATHS-01 | `projectRoot` + `logsDir`/`functionsDir`/`screensDir` inside root | 2 cases |
| M1-PATHS-02 | `screenshotName`: zero-padded numbering, caption preserved, 3-digit number | 2 cases (+W1d real names) |
| M1-PATHS-03 | `screenshotName`: punctuation -> single dash, slug truncated to 40, empty caption -> `item` | 3 cases |
| M1-PATHS-04 | `relativeToRoot`: inside root -> relative; outside -> as is | 2 cases |

### M1-PROC — `src/core/processes.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-PROC-01 | `isDiscordProcess`: case-insensitive; `vesktop` excluded; foreign processes -> `false` | 3 cases (+W1d name variants) |
| M1-PROC-02 | `filterDiscordProcesses`: preserves order; empty list -> empty | 2 cases |

### M1-INV — `src/core/inventory.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-INV-01 | `inventoryCounts`: total symbols/exports; empty -> zeros | 2 cases (+W1d fragment of real JSON) |
| M1-INV-02 | `inventoryCsvRows`: header; symbols `kind=symbol/type=function`; exports; `safeName`; without modules | 3 cases (+W1d jq oracle) |
| M1-INV-03 | `inventoryJson`: `{pid, process, at, modules}`, process name as is | 1 case (+W1d round-trip of real JSON) |

### M1-PROFILE — `src/core/ui/profile.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-PROFILE-01 | `topFunctions`: sort by `hits` descending; `url` — last segment; stable on equality | 2 cases |
| M1-PROFILE-02 | Discard nodes without `hitCount` and noise `(idle)/(program)/(garbage collector)/(root)`/empty | 2 cases |
| M1-PROFILE-03 | Truncate `url` to 60; empty `url` allowed; `limit` (default 8) | 2 cases (+W1b golden top-8 of real logs) |

### M1-BLACK — `src/core/ui/blacklist.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-BLACK-01 | Action blocklist matrix: `dangerActions` (leave/delete/kick/ban/reset/disable) -> `true` | 2 cases |
| M1-BLACK-02 | Navigation and menus: `allowSubstrings` (close/back/forward/tab/next/ok/apply/cancel) -> `false` | 2 cases |
| M1-BLACK-03 | Conflict resolution: danger takes precedence over allow; case-insensitive; empty/spaces -> `false` | 3 cases |

### M1-WALK — `src/core/ui/walk-log.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-WALK-01 | `createWalkLog`: write header, steps, summary | 3 cases |
| M1-WALK-02 | `parseVisited`: extract visited item signatures | 2 cases (+W1b golden) |
| M1-WALK-03 | `logStep`: JSONL format, step counter, timestamp | 2 cases |
| M1-WALK-04 | `shouldBail`: `/timeout|closed/`; normal error — no; `null` -> `false` | 3 cases (+W1b explicit zero error records) |
| M1-WALK-05 | `pickNextItem`: visited/blocked filtering; ordering; completed -> `undefined`; custom blacklist | 3 cases (+W1b real log) |

### M1-CDP — `src/core/cdp/client.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-CDP-01 | `resolveEndpoint`/`resolveTargetHint`: default values and env overrides | 3 cases |
| M1-CDP-02 | `listTargets`: `<endpoint>/json/list`; HTTP error -> `CDP list failed: <status>` | 2 cases |
| M1-CDP-03 | `pickTarget`: by hint; "discord" in url; skip targets without `webSocketDebuggerUrl`; fallback; `undefined` | 4 cases |
| M1-CDP-04 | `createCdpClient`: dispatch `{id, method, params}` -> result; error response -> reject with method | 2 cases |
| M1-CDP-05 | Events accumulate in `events`; foreign `id` ignored; malformed JSON does not crash | 2 cases |
| M1-CDP-06 | Response timeout; `close()` rejects pending; connection error prior to open | 3 cases |
| M1-CDP-07 | `evaluate`: `returnByValue/awaitPromise` -> `value`; exception -> `Error` with `description`, fallback `text` | 3 cases |

### M1-CALLS — `src/core/calls.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-CALLS-01 | `isCallPayload` (`type === "call"`) and `callLine` (JSON+`\n`; non-call -> `null`) | 3 cases |
| M1-CALLS-02 | `parseCallLog`: fixture; empty/malformed lines and non-calls skipped | 1 case (+W1a golden) |
| M1-CALLS-03 | `summarizeCallLines`: grouping `(module,name,address)`, sorting, escaping matching shell | 1 case (+W1a golden CSV) |
| M1-CALLS-04 | `summarizeCallLines`: double quotes without outer quotes; empty list -> header only | 2 cases (+W1a stress 50k/5k) |

### M1-NM — `src/core/report/nm.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-NM-01 | `parseNmPosix`: only `T/t/W/w`; address and size; short lines skipped; empty input | 2 cases (+W1c golden) |
| M1-NM-02 | `staticFunctionsCsv`: `name,type,address,size` without trailing `\n` | 1 case (+W1c awk oracle) |

### M1-DOMAINS — `src/core/report/domains.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-DOMAINS-01 | `classifyName`: substring rules; multidomain and fixed order; clean name extraction | 5 cases (+W1c golden from domain tables) |
| M1-DOMAINS-02 | `domainCsv`: header; `demangled` quoted with double quotes; empty input | 2 cases |
| M1-DOMAINS-03 | `formatDomainSummary`: `%-12s %8d`, lines separated by `\n` | 1 case (+W1c REPORT.md figures) |

### M1-CXXFILT — `src/adapters/cxxfilt.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-CXXFILT-01 | `demangle`: order preserved; non-mangled names as is | 1 smoke case (+W1c real names) |
| M1-CXXFILT-02 | Empty list -> empty result without spawning process | 1 smoke case |

### M1-CLI — `src/cli/run.mjs` + `bin/discord-lab.mjs`

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-CLI-01 | `--help` and empty argv: help text, exit code 0, command list | 2 smoke cases |
| M1-CLI-02 | `--version`: version from `package.json` | 1 smoke case |
| M1-CLI-03 | Unknown command -> exit code 1 + error message to stderr | 1 smoke case |
| M1-CLI-04 | Unavailable external environment -> exit code 2 (CDP; later Frida) | 1 smoke case |
| M1-CLI-05 | Command routing §6 (pids/inventory/cdp/walk/summarize/domains/static) | covered by M2–M4 |

### M1-SUM — `summarize` command (on top of `calls`)

| ID | Microtask | Cases |
| --- | --- | --- |
| M1-SUM-01 | Writes `<log>.by-function.csv` and prints summary (parity with `summarize-calls.sh`) | smoke fs |
| M1-SUM-02 | Without arguments and without `logs/calls-*.jsonl` -> exit code 1, "No logs found" | smoke fs |
| M1-SUM-03 | Full real log: CSV line-by-line equal to oracle (W1a) | smoke real |

## W4-fix — Robustness Hardening Wave

Source: `/tmp/opencode/review-w3/security-review-w4.md` (§4.1–4.8, §8, §10).
Owner decision: W4-fix — approve; §10 (backtrace) — make optional (`--backtrace`).
Wave rules — `PROCESS.md` (Task spec §4.1, report §4.2, gate:worker). Spec: `/tmp/opencode/spec-w4fix-*.txt`.

| ID | Microtask | Files | Owner | Status |
| --- | --- | --- | --- | --- |
| W4f-01 | `captureMessages` (default false) — drain messages without accumulation (P2-1) | `src/adapters/frida.mjs`, `tests/unit/frida.test.mjs` | W4f-a | [x] |
| W4f-02 | `src/core/hook-io.mjs`: openCallLog (stream, error counter, close-flush), waitForStop (SIGINT/SIGTERM in both branches) (P2-2, P3-7) | `src/core/hook-io.mjs`, `tests/unit/hook-io.test.mjs`, §8 | W4f-b1 | [x] |
| W4f-03 | run.mjs hook: stream writing instead of appendFile, mkdir/open in try, error counter in summary (P2-2) | `src/cli/run.mjs`, smoke | W4f-b2 | [x] |
| W4f-04 | Numeric validation for `--pid`, `--limit`, `--max-symbols`, `--arg-count`, `--duration-ms` -> code 1 before Frida (P2-3) | `src/core/args.mjs`, `src/cli/run.mjs`, tests | W4f-b2 | [x] |
| W4f-05 | packageVersion wrapped in try -> code 2 on corrupt package.json (P3-6) | `src/cli/run.mjs`, smoke | W4f-b2 | [x] |
| W4f-06 | Validation for `--pattern` (regex + cap 512) -> code 1 (P3-8) | `src/core/args.mjs`, `src/cli/run.mjs`, smoke | W4f-b2 | [x] |
| W4f-07 | Agent: `moduleEntry` wrapped in try (P2-4); reset hooks on repeated `hook()` (P3-5) | `scripts/frida-log-calls.js`, `tests/unit/frida-agent.test.mjs` | W4f-c | [x] |
| W4f-08 | §10 backtrace: `--backtrace` -> hook option -> `stack` (up to 8 frames) in messages | agent (W4f-c) + `src/cli/run.mjs` (W4f-b2) | W4f-c + W4f-b2 | [x] |
| W4f-09 | calls contract: record with `stack` survives callLine->parseCallLog | `tests/unit/calls.test.mjs` | W4f-c | [x] |
| W4f-10 | Finisher: `tests/unit/frida-agent.test.mjs` per recipe (7 edits) + `args.mjs` types (7xTS2345 from b2 zone) | `tests/unit/frida-agent.test.mjs`, `src/core/args.mjs` | W4f-c-fin | [x] |
| W4f-11 | Branch coverage to threshold: numeric inputs `parseNumericArg`, `undefined` pattern, default `agentSource` (89.93%->91.88%) | `tests/unit/args.test.mjs`, `tests/unit/frida.test.mjs` | W4f-cov | [x] |
| W4f-12 | Live finding: `startHook` normalizes RegExp->`.source` at RPC boundary (frida RPC lost RegExp -> `new RegExp({})` -> `attach(0x0)`) | `src/adapters/frida.mjs`, `tests/unit/frida.test.mjs` | W4f-live | [x] |

Acceptance (coordinator, 2026-10-18): gate:worker passed (typecheck; meta 5; unit 169; smoke 20);
gate:release passed_with_skips (4 e2e skips: 2x missing `CHROME_BIN`, 2x missing `DISCORD_LAB_LIVE_CDP` —
recorded in warnings); coverage 99.40/91.88/97.03; golden md5 192/192.
Live checklist (ptrace_scope=0 -> restored to 1): `inventory` in project copy (auto: 6 processes;
JSON/CSV keys = golden; 2 zygote skipped via `auxv-not-found`); `hook` 3 variants — targets
7/attached 7: 7732 calls (8 s), 11582 (6 s, `--backtrace`: stack in 11582/11582), 11459 (SIGINT,
code 0); 0 write errors. Byte-for-byte identical with legacy logger (`callLine` =
`JSON.stringify` + `\n`); `summarizeCallLines` reads live logs successfully.
Dispatches (2026-10-18): a `ctx_0fa64c95c67c`, b1 `ctx_68b958b71c28`, b2 `ctx_4fe8244b2c19`,
c `ctx_3492203a6dd4` (closed outside orchestration, released; previous runs failed on
`agent-trust-workspace`); finisher `ctx_bd636c98598f` (codex -> INSUFFICIENT_BALANCE 403; stopped)
-> reissue `ctx_60eb6d71f1dc` (opencode, released); cov `ctx_d7dff3391098`, live `ctx_74fa2fb51624`
(released).
Golden incident (2026-10-18): red case b2 (`inventory --pid abc` on pre-change CLI) created
`functions/pid-NaN-pid-abc.{json,csv}`; worker did not touch golden data, escalated immediately; coordinator deleted
(backup — `/tmp/opencode/golden-incident-2026-10-18/`), manifest 192/192 intact; worker isolated test
using temporary directory. Backlog: `gate:release` must detect superfluous untracked files in golden folders.

## M2–M6 — Milestone Wave Tasks

### M2 — Frida Layer (W4: Workers; Completed 2026-10-18)

Flow: W4a (adapter) -> acceptance -> W4b (CLI on terminal W4a); W3R hardening executed in parallel.

| ID | Microtask | Owner | Status |
| --- | --- | --- | --- |
| M2-FRIDA-01 | `src/adapters/frida.mjs` — `loadDevice/findProcess/spawn/enumerate` | W4a | [x] |
| M2-FRIDA-02 | `src/adapters/frida.mjs` — `compileScript/createSession/attach` | W4a | [x] |
| M2-HOOK-01 | `scripts/frida-log-calls.js` — port from legacy, error handling | W4a | [x] |
| M2-CLI-01 | `src/cli/run.mjs` — commands `pids` and `inventory` via adapter | W4b | [x] |
| M2-CLI-02 | `src/cli/run.mjs` — command `hook` with output streaming to `logs/calls-*.jsonl` | W4b | [x] |
| M2-E2E-01 | E2E test with fake process (or live Discord if available) | W4b | [x] (smoke) |

Acceptance of M2 (2026-10-18): combined with W4-fix into unified gate:worker + gate:release;
commands `pids`, `inventory`, `hook` operational via CLI; golden outputs match.

### M3 — UI-walk (W5)

| ID | Microtask | Owner | Status |
| --- | --- | --- | --- |
| M3-CDP-01 | CDP high-level actions: click, focus, input, screenshot | W5a | [x] |
| M3-WALK-01 | Walk cycle: item selection, click, screenshot, step log | W5a | [x] |
| M3-CLI-01 | Command `walk` in CLI with `--out-dir`, `--limit`, `--profile` | W5b | [x] |
| M3-CLI-02 | Command `cdp` in CLI (status, target list, evaluate) | W5b | [x] |

### M4 — Reports (W6)

| ID | Microtask | Owner | Status |
| --- | --- | --- | --- |
| M4-STATIC-01 | Command `static` — parsing `nm -C` and CSV generation | W6a | [x] |
| M4-DOM-01 | Command `domains` — classification and domain summary | W6a | [x] |
| M4-DIFF-01 | Command `diff` — comparing two inventories/reports | W6b | [ ] |

### M5 — CI and Mutations (W7)

| ID | Microtask | Owner | Status |
| --- | --- | --- | --- |
| M5-COV-01 | Coverage gate enforcement for all `src/**/*.mjs` modules | W7 | [x] |
| M5-MUT-01 | Stryker or custom mutator for critical modules | W7 | [x] |

### M6 — Parity and Cleanup (W8)

| ID | Microtask | Owner | Status |
| --- | --- | --- | --- |
| M6-PAR-01 | Output verification against legacy shell scripts | W8 | [x] |
| M6-CLEAN-01 | Deprecate and remove redundant temporary files | W8 | [x] |

## Disjoint File Boundary Matrix

All workers operate in the shared directory `.` (single check-out;
copies `project-w1…w4` removed after acceptance). File ownership during the wave:

| Worker | Owns | Forbidden |
| --- | --- | --- |
| W1a | `tests/unit/calls.test.mjs`, `tests/smoke/summarize.smoke.test.mjs` | Other tests, `src/`, docs |
| W1b | `tests/unit/{profile,blacklist,walk-log}.test.mjs` | Other tests, `src/`, docs |
| W1c | `tests/unit/{nm,domains}.test.mjs`, `tests/smoke/cxxfilt.smoke.test.mjs` | Other tests, `src/`, docs |
| W1d | `tests/unit/{args,csv,paths,processes,inventory}.test.mjs` | Other tests, `src/`, docs |
| Coordinator | `TDD-PLAN.md`, `TASKS.md`, `tests/meta/*`, `package.json`, integration | Modifying test implementations during worker phase |

Boundary rule: workers never touch another worker's files. No merge conflicts in shared workspace.

## Worker Protocol Summary (`PROCESS.md` §3.4 + `AGENTS.md`)

1. Work strictly within assigned file boundaries in the shared directory (non-overlapping sets).
2. Tests are the specification: expectations come from independent oracles (jq/awk/c++filt), with
   provenance in comments: artifact, md5, command. Fixtures <= 64 KiB.
3. Strict TDD: red test first, run test to confirm failure, then write minimal green code.
4. Gate: `gate:worker` must pass cleanly before reporting task completion.
5. On blockers or ambiguities, use `ask` immediately — do not invent contracts.

## Coordinator Acceptance Checklist (W2) — Completed

- [x] Edit scope verification: `find -newer` + diff against pristine copies — only assigned files modified (W1 and W1-fix).
- [x] Provenance: md5 hashes in comments match real artifacts (voice `f4e09451…`, walk `bf5b40e3…`/`de63e05e…`, nm `872c8da3…`/`bd7a32ea…`, inv `d671be68…`, calls `3f99a9b1…`).
- [x] Oracle correctness: jq, awk, c++filt, md5 outputs verified independently.
- [x] Deterministic test suite: no flaky assertions or unhandled timing races.

## R1 — Gateway Protocol Reconnaissance

Goal: Empirically capture and document the event gateway protocol (`gateway.discord.gg`,
WS/TCP/443) and REST surface, operating exclusively against an isolated client instance (CDP 9223); the live client
is untouched. Input findings: web bundle exposes
`wss://gateway.discord.gg/?encoding=etf&v=9&compress=zstd-stream` (or `json`).
Cleanroom approach: isolated CDP runtime, network tap, and raw capture decoders.

| ID | Direction | Result | Artifact |
| --- | --- | --- | --- |
| R1-GW-01 | Gateway URL and query parameters | Confirmed: `wss://gateway.discord.gg/?encoding=json&v=9` (etf/zstd available) | `docs/re/gateway-protocol.md` |
| R1-GW-02 | Handshake and compression | Verified zstd stream framing and dictionary initialization | `docs/re/gateway-protocol.md` |
| R1-GW-03 | Heartbeat and jitter | Heartbeat interval from HELLO (op 10), ACK (op 11), jitter math | `docs/re/gateway-protocol.md` |
| R1-GW-04 | Opcode and event dispatch catalog | HELLO(10), IDENTIFY(2), READY(0), RESUME(6), RECONNECT(7), INVALID_SESSION(9) | `docs/re/gateway-schema.md` |
| R1-REST-01 | Authentication and super-properties | Header `Authorization`, `X-Super-Properties`, `X-Context-Properties` | `docs/re/rest-d02.md` |
| R1-VOICE-01 | Voice signaling discovery | Voice server update (op 4), UDP endpoint discovery, IP discovery payload | `docs/re/voice-deep.md` |

Summary: Wave R1 closed on 2026-10-19. All captured schema artifacts, opcodes, and session flows
were structured and preserved for cleanroom implementation.

## R7 — Deep Reverse Engineering (Closed 2026-10-19)

Goal: Resolve gaps in the coverage matrix (transport/gateway, voice/crypto, REST, UI/i18n) and
comprehensively dissect the backend architecture (`main` process, IPC bridge, native modules), operating with
an isolated client instance (CDP 9223). Normative documents live in `docs/re/` (index — `docs/re/README.md`,
process/call/function graphs — `docs/re/architecture.md`); raw cleanroom evidence (pcap, strace,
decodes, screenshots, sanitized captures; 1.7 GB total) — reference cleanroom artifacts.
Secrets are sanitized, public IPs masked as `<PUB_IP>`, outbound WS frames stored as length + sha256;
golden directories (`logs/`, `functions/`, `evidence/`) remain read-only.

| ID | Direction | Result | Artifact |
| --- | --- | --- | --- |
| R7-01 | Gateway: ETF/zstd/opcodes/READY/GUILD_CREATE | **D-01 closed**: 4 full READY payloads (40 keys, 7 guilds, 17 DMs, 146 users, 157 read_state); 11 verified ETF tags; fixed big-int sign handling; op7 -> RESUME -> RESUMED without sequence breaks | `docs/re/gateway-protocol.md`, `gateway-schema.md` |
| R7-02 | REST: routes, request/response bodies, auth | **D-02 closed**: 47 routes, 133 requests v9; auth — 221 requests / 6 headers, super-properties 22 fields; independent verification exit code 0 | `docs/re/rest-d02.md` (+`.json`, `.stats.json`, `.crossref.json`, `rest-d02-verify.sh`) |
| R7-03 | UI traces II and component contracts | **D-03 closed**: search (+7 filter sections), emoji picker, member list, settings, popovers; 12 component contracts | `docs/re/frontend-d03.md` |
| R7-04 | Voice: signaling and UDP transport | ops 8/0/1/2/4/15/16/3/6; binary structures for discovery/keepalive/RTP/SRTCP; media socket rotation at +60.08 s (confirmed via strace); DTX and comfort-noise | `docs/re/voice-deep.md` |
| R7-05 | Main process and IPC execution chains | Bootstrap lifecycle, window/`webPreferences`, auto-updater; 5 complete `DISCORD_*` chains; 121/121 citations with offsets | `docs/re/main-arch.md` |
| R7-06 | IPC map and `DiscordNative` bridge | 168 channels (152 main / 129 renderer / 115 duplex), allowlist 43, bridge 35 entries, 47 call-sites | `docs/re/backend-ipc.md` |
| R7-07 | Native modules | 9 modules (`discord_voice.node` 126.1 MB, `discord_krisp.node` 62.1 MB), NAPI 94/94, Frida inventory 110 exported symbols, dlopen list | `docs/re/native-re.md`, `native-modules.md` |
| R7-08 | Internationalization (i18n) | Custom runtime; message key = xxh64 LE -> base64[0..6] (36 bits); pluralization rules + `=N`; 23 formatting markup rules; 31/36 locales; 23,836 keys | `docs/re/i18n-deep.md`, `i18n-stats.json` |
| R7-09 | Methodology and sanitization | strace -> complete UDP bytes; CDP drivers with trusted `Input.*`; snap-pcap (2.6 MB); JSONL sanitizer (zero secret leakage) | `docs/re/README.md` §4, reference cleanroom artifacts |

Constraints and unknowns: initial 43-byte talkspurt packets; RTCP payload encryption (loss/jitter
inaccessible externally); closing of old file descriptors during socket rotation not observable in strace; nested E2EE DAVE/MLS
not fully disassembled; crypto fields reconstructed observationally (`docs/re/voice-deep.md` §2.3–2.4).

## RC — Rust Client: Concept, Architecture, Plan, Tests (RC-01…RC-08)

Goal: Prepare foundational specifications for an external native Rust project — a **full-featured Discord client (native
backend, lightweight shell; without Electron and without bundled Chromium)**. Documentation package — `docs/rust-client/` (index —
`docs/rust-client/README.md`); derived from R7 (`docs/re/`) and feature transfer cards R4/R5/R6/D
(`docs/feature-backlog.md`).

| ID | Direction | Result | Artifact |
| --- | --- | --- | --- |
| RC-01 | Concept | Scope and boundaries, 10 feature groups, technical stack (tokio/tungstenite/zstd/SQLite/keyring…), user scenarios, MVP criteria, risks | `docs/rust-client/CONCEPT.md` |
| RC-02 | Feature Catalog | 14 functional groups (GW/ENC/AUTH/API/DATA/MSG/SRCH/PRES/VOICE/MEDIA/UI/I18N/SYS/SEC): purpose, rationale, cleanroom reverse-engineered behavior, origin, delivery phase | `docs/rust-client/FEATURES.md` |
| RC-03 | Architecture and Graphs | Process model (main + media worker), 10 core crates, event bus/reducer, gateway pipeline, voice stack, SQLite schema, replacement for 168 IPC channels, ADR-1, resource budgets | `docs/rust-client/ARCHITECTURE.md` (8 Mermaid graphs) |
| RC-04 | Implementation Plan | Waves P0–P8: 90 work cards with DoD, tests, dependencies, and estimations; verification gates, risk register, alignment with R4/R5/R6/D | `docs/rust-client/PLAN.md` |
| RC-05 | Test Plan | Test levels L1–L5; manifest of cleanroom fixtures; catalog of ~130 tests (T-ENC/GW/REST/DATA/MSG/SRCH/VOICE/MEDIA/UI/I18N/SYS/SEC/HARN); strict red-first TDD protocol | `docs/rust-client/TEST-PLAN.md` |
| RC-06 | Meta-Control | Cross-document synchronization: file existence, Mermaid graphs intact, unique IDs for features and tests, active references from README/TASKS/map | `tests/meta/rust-client-sync.test.mjs` |
| RC-07 | Unified Structure | Harmonization of reverse engineering and Rust client: curated fixtures `fixtures/` (12 entries, 1.59 MB, `MANIFEST.json` with md5 checksums), cargo workspace `rust/` (10 crates, offline `cargo test`), structure and traceability (141 F -> specs -> fixtures -> 133 T) | `fixtures/`, `rust/`, `docs/rust-client/PROJECT-STRUCTURE.md`, `TRACEABILITY.md` |
| RC-08 | Artifact Verification | 20 automated verifications between raw artifacts and docs: md5 of D-02 snapshots, READY statistics (7/17/146/60/157), gw-proto frames (1544/1340/21), 11 ETF tags, REST routes (47/133/221), i18n metrics (23,836/31), IPC inventory (168/43/35), native modules (9), RTC opcodes, sanitization checks, fixture integrity — 20/20 passed | `docs/rust-client/verify/verify-re.mjs`, `verify/report.json`, `VERIFICATION.md` |

Conformance test fixtures live in `fixtures/` (manifested in `TEST-PLAN.md` §2; 12 curated entries with md5);
credentials and sensitive tokens sanitized. Implementation code (Rust) follows `docs/rust-client/PROJECT-STRUCTURE.md`:
workspace scaffold established in `rust/` (crates), progressed iteratively through waves P0–P8.
Verification milestone RC-08 succeeded with 20/20 green checks (`VERIFICATION.md`).
Deliverable package RC-01…RC-08 completes foundational cleanroom planning.

## R4/R5/R6/D — Feature Transfer Backlog Cards

Goal: Port missing capabilities and features identified during comparative analysis of external reference implementations
into our architecture. Complete feature inventory — `docs/feature-backlog.md`
(41 work cards: description, source reference, destination architecture, acceptance criteria); analyses —
`docs/vesktop-analysis.md` and `docs/discord-qt-analysis.md`. Reference code is not copied (cleanroom reimplementation):
architectural ideas and contracts are adapted; open-source dependencies (venmic, arrpc) respected per their respective licenses.

| Wave | Scope | Cards | Priority | Status |
| --- | --- | --- | --- | --- |
| R4 | OS Shell and Desktop Integration (tray, notifications, autostart, window management) | 12 | P0 | [x] transferred to `docs/feature-backlog.md` §R4 |
| R5 | Audio and Media Subsystems (PipeWire/PulseAudio capture, WebRTC voice, volume normalization) | 14 | P1 | [x] transferred to `docs/feature-backlog.md` §R5 |
| R6 | Platform and Native Services (D-Bus, MPRIS, spellchecking, secret service keyring) | 11 | P2 | [x] transferred to `docs/feature-backlog.md` §R6 |
| D | Protocol Data Enhancements (READY, extended REST routes, interactive UI telemetry) | 4 | P0 | [x] closed in R7 (`docs/re/`) |

Cross-document synchronization: all cards in `docs/feature-backlog.md` are cross-referenced
in `docs/map.md` and traced across implementation waves.

## RW — Native Rust Client Implementation (`docs/rust-client/PLAN.md`)

Started 2026-10-19. Framework — `PROCESS.md` §3–§4: coordinator manages work cards/specs/acceptance; worker
agent writes tests and code strictly in assigned files; reviewer (independent agent)
verifies red-first TDD and oracle alignment. Effective 2026-10-20, batch pipeline active: 3–6 cards,
unified integration patch, targeted runs by workers, single gate, review post-gate (`docs/rust-client/TEAM.md`,
`docs/rust-client/WAVE-PLAN.md`). Wave journal (artifacts, red/green logs, recovery point) —
`waves/` (`db.jsonl`, `STATE.md`, `RW-01`…`RW-18`; journal in git); scratch drafts — `/tmp/opencode/rw-*` (ephemeral).

| ID | Card | Plan | DoD | Tests | Owner | Status |
|---|---|---|---|---|---|---|
| RW-01 | ETF-codec (prototype): `EtfValue`/`decode`/`encode`, fuzz-lite | P0-02 | 11 tegov + big-int «sign before digits» + 5 realnykh terms iz `capture-run2.jsonl` (length comparison with `gw-proto/msgs-*.jsonl`); round-trip byte-for-byte; errors/boundaries panic-free | T-ENC-01…03 (+fuzz-lite) | worker: subagent; acceptance: coordinator | accepted 2026-10-19 |
| RW-02 | zstd-stream decoder (prototype): `ZstdStream::push` na ruzstd 0.9.1 | P0-03 | frame 0 s magiey, framey 2+ bez (real soket `95974.1389`: 137 frameov, 104 066 → 824 816 B); continuous kontekst bez `end()`; 64 KiB window retention documented; errors `MissingMagic`/`Decode`/`TrailingData` panic-free | T-ENC-04 | worker: subagent; acceptance: coordinator | accepted 2026-10-19 |
| RW-03 | Gateway session (prototype without transport): HELLO/heartbeat 41 250/ACK/RTT/zombie v `discord-core::gateway` | P1-03 | state machine on virtual clock: first tick — `seed % 41250`, next — `ACK + 41250`; RTT po op11; zombie → `Reconnect`; `s`-afterdovatelnost; oracle — timeline `95974.1389` (23 ACK; delty ≤ 41 263; simulation: 11–12 udarov, bez zombie) | T-GW-01…03 (bez stub servera) | worker: subagent; acceptance: coordinator | accepted 2026-10-19 |
| RW-04 | Gateway transport: WS-klient (tokio-tungstenite + rustls/ring), pipe zstd->terms->session, stub server v testskh | P1-03 | connect (`ws://`/`wss://`), chtenie frameov, heartbeat per session effects, timers/zombie, outcomes `Reconnect`/`Closed`; T-GW-01/03 na lokalnom stub (`127.0.0.1`); lab-repley 137 frameov; new dependencies approved by owner 2026-10-19 | T-GW-01, T-GW-03 | worker: subagent; acceptance: coordinator | accepted 2026-10-19 |
| RW-05 | Identification: IDENTIFY (op2) per web client structure + READY summary; dispatch IDENTIFY after HELLO | P1-04 | fields op2 `{token, capabilities, properties, presence, compress, client_state, qos_token}` (oracle — bandl veb-klienta `out/bundles/discord.com_assets_web.*.js`, fakty `rest-static.md` §4); `capabilities` 1734653/1767421; READY — summary po run2-oracleu (40 klyuchey, 7/17/146/60, read_state 157/128949, user_guild_settings 58/734); T-GW-02 na stub, T-GW-04 lab-test | T-GW-02, T-GW-04 | worker: subagent; acceptance: coordinator | accepted 2026-10-19 (independent review; nits closed) |
| RW-06 | Resume/reconnect: op6 `{token,session_id,seq}`, policy resume (3 s/30 s), flag op9 `d`, continuous `s` | P1-05 | `resume.rs` (`build_resume_frame`, `ResumeState`, `plan_reconnect`, porog 30 000 ms); `ws.rs` (`connect_and_run_resume`, op6 after HELLO, `resume_state()`, `last_invalid_session_resumable()`); T-GW-05 seams 663→664 i 331→332, T-GW-06 stub scenario «op7 → RESUME → RESUMED lossless», T-GW-07 policy disconnect | T-GW-05…07 | worker: subagent; acceptance: coordinator | accepted 2026-10-19 (independent review; gaps R4/R10/R11 closedy) |
| RW-07 | Dispatch router: op0 -> typed events of 21 types; unknown -> raw (`kind: None` + counter); payload bez breakdowna | P1-06 | `dispatch.rs` (`DispatchKind` 21 tip, `DispatchEvent`, `route_dispatch`); `ws.rs` (`events`/`take_events`/`unknown_dispatches` v paype); oracley: union `msgs-*.jsonl` — 21 tip/1340 sobytiy; soket `.1389` — 44 terms/36 op0 (window ruzstd, 704 B tail) | T-GW-08 | worker: subagent; acceptance: coordinator | accepted 2026-10-19 (independent review; nits N1–N3 closedy red-first, N4 — v backlog) |
| RW-08 | REST-klient: catalog of 47 routes, 6 auth headers, status codes, retries/timeouts (3 parallel workers + refaktoring W8d + revyu W8r) | P1-07 | `routes.rs` — 47 marshrutov/`find`/`render_path` (T-REST-01); `headers.rs` — 6 zagolovkov + klassy kodov (T-REST-02); `retry.rs` — taymauty/retrai idempotentnykh, POST/PATCH bez retraev, lokalnyy stab (T-REST-03); bez novykh zavisimostey; real TLS-transport — otkrytyy vopros | T-REST-01…03 | workery: subagenty W8a–W8c; refaktoring: W8d; revyu: W8r | accepted 2026-10-20 (revyu W8r: zero blockers, 14/14 mutations; nit pokrytiya backoff closed red-first — keys N6) |
| RW-09 | X-Super-Properties: generator 22 poley (base64 JSON), konfiguriruemo, policy «chestnye znacheniya» (3 parallel workers + refaktoring W9d + revyu W9r) | P1-08 | W9a `base64.rs` — RFC 4648 vruchnuyu; W9b `super_properties.rs` — JSON-zapis/breakdown 22 poley i base64 (T-REST-04); W9c `identity.rs` — chestnye znacheniya iz sistemy (OS/yadro/lokal/distributiv/displey/okonnyy menedzher), svoi fields identichnosti, UUID v4 vruchnuyu; oracle — live zagolovok `95974.2323` (snapshot D-02) + §6.2 `rest-d02.md`; bez novykh zavisimostey | T-REST-04 | workery: subagenty W9a–W9c; refaktoring: W9d; revyu: W9r | accepted 2026-10-20 (revyu W9r: zero blockers, 20/21 mutations; nits N1/N6 closedy red-first, N4/N5 fixedy, N2 annotirovan, N3 — v backlog) |
| RW-10 | HTTP/1.1 transport REST (variant A: tokio + tokio-rustls, manual framing) — 2 parallel workers W10a/W10b | P1-07 (infrastruktura) | `http1.rs` — zapros/otvet/CL/chunked/limity/inektsii; `transport.rs` — Endpoint, TCP/TLS, SAN, taymauty, chtenie po CL/chunked/until-close; fikstury CA→leaf; zavisimosti `tokio-rustls 0.26`, `httparse 1` (oflayn) | 30 (16+14) | workery: W10a/W10b; revyu: independent agent | revyu 2026-10-20: «accepted s nitami» (povtornoe revyu — «accepted s nitami», P0/P1/P2 net); P1/P2-nits closedy red/green (`tests` 16+14; polnyy red/green `nits-red-final`/`nits-green-final`), P3 N1–N9 — queue (`waves/reviews/review-rw-10-11-round2.md`); accepted 2026-10-20 |
| RW-11 | Rate-limit/errors, telemetry-off, ack/read-state — 3 parallel workers W11a/W11b/W11c | P1-09, P1-11, P1-12 | `rate_limit.rs` — retry-after/JSON-fields/429/5xx/401/bekoff; `telemetry.rs` — blok science/beaker/track; `ack.rs` — target/body s tokenom | 23 (10+4+9) | workery: W11a–W11c; revyu: independent agent | revyu 2026-10-20: «accepted s nitami» (povtornoe revyu — «accepted s nitami», P0/P1/P2 net); P3 (JSON-skaner, normalizatsiya telemetry, ssylki, httparse, tests Host/TE) — queue; accepted 2026-10-20 |
| RW-12 | P2-01 READY parser + I1 REST facade (independent tracks) | P2-01 / infrastruktura | I1 `client.rs` — auth+transport+429/5xx+telemetriya; P2-01 — A1: 40 top-level, guilds/channels/roles/emojis/stickers, private, users, relationships, read_state, UGS, sessions, merged_members, diagnostika | I1 16; P2-01 14+13+11+7 | I1: coordinator; P2-01: W12a/W12b/W12c + review | **Accepted 2026-10-20.** I1 — independent review «Accepted» (`waves/reviews/review-i1.md`), P3-7 backlog. P2-01 — review round 1 «ne accepted» (P1/P2/P3), vse P1/P2 closedy red-first; round 3 «accepted», P0/P1/P2=0 (`waves/reviews/review-p2-01-round3.md`). Red-final 0/7, green 7/7; core **233**, workspace **275**, fmt/clippy clean; legacy `ready_data` removed (P-4); geyt passed `.gate/worker-2026-10-20T09-56-10-288Z.json`; artifacts `waves/RW-12/reports/` |
| RW-13 | P2-02 state/reducer/selectors (3 parallel workers W13a/W13b/W13c) | P2-02 | `state/types.rs` — State/Event/parsery, diagnostika, redaction; `state/reducer.rs` — `apply`/`apply_dispatch`/`apply_frame`, stale-policy, sliyanie, `SubscribeMembers`; `state/selectors.rs` — vyborki; kornevye re-exports | 42 (15+19+8) | W13a/W13b/W13c; review: independent agent | **Accepted 2026-10-20.** Review round 1 — «ne accepted»: P1 stale malformed + 5 P2 (author upsert, private DM, supplemental index, root exports, property-test); vse closedy red-first, povtornoe revyu — «accepted», P0/P1/P2=0 (`waves/reviews/review-p2-02.md`). Red fiksov: 4 runtime + compile-r19, exit 101; green 42; core **275**, workspace **317**, fmt/clippy clean; gate passed `.gate/worker-2026-10-20T10-58-32-180Z.json`; artifacts `waves/RW-13/`. |
| RW-14 | P2-06 memory history/pagination (W14a/W14b/W14c) | P2-06 | `history/types.rs` — bounded window, Live > Page, trim Latest/Before/After/Around; `pagination.rs` — Cursor/PageRequest/MessagePage/PageToken, generation/stale, atomic ChannelMismatch, dedup; `selectors.rs` — recent/range/window_around i dr. | 20 (7+5+8) | W14a/W14b/W14c; review: independent agent | **Accepted 2026-10-20.** Review round 1 — P2-1 (root re-exports), P0/P1=0; closed red-first, P3-2/cap/expect fixedy; povtornoe revyu — «accepted», P0/P1/P2=0 (`waves/reviews/review-p2-06.md`). Red: compile W14a/b + behavior W14c + root/counts, exit 101; green 20; core **295**, workspace **337**, fmt/clippy clean; gate passed `.gate/worker-2026-10-20T11-45-30-914Z.json`; scope memory-only (REST response/JSON/SQLite/perezapusk vne volny); artifacts `waves/RW-14/`. |
| RW-15 | P2-04 member list: GATEWAY upsert/delta, op37-subscriptions, range/ranges, udalenie | P2-04 | `state/member_list.rs` — tipy i parsery; `member_list_reducer.rs` — upsert/delta/range/ranges, stale/duplicate; `member_list_selectors.rs` — vyborki; `Effect::SubscribeMembers`; bez zhivogo op37 | 40 (14+17+9) | W15a/W15b/W15c; review: independent agent | **Accepted 2026-10-20.** Review round 1 — P2-1 (duplicate pri INSERT v pozitsiyu sushchestvuyushchego id), P0/P1=0; closed red-first, P3 closedy; povtornoe revyu — «accepted», P0/P1/P2=0 (`waves/reviews/review-p2-04.md`). Green targeted 40/40, core **335**, workspace **377**, fmt/clippy clean; gate passed `.gate/worker-2026-10-20T12-43-25-346Z.json`; artifacts `waves/RW-15/`. |
| RW-16 | P2-05 memory-only chunks/lazy loading: `GUILD_MEMBERS_CHUNK`, merge members, `not_found`, `Effect::RequestPresence` | P2-05 | `state/member_chunk.rs` — tipy/parser; `member_chunk_reducer.rs` — additive merge po guild, chunk_index/count, not_found; `member_chunk_selectors.rs` — diapazony/progress; `presences[]` ne razbiraetsya do P2-09 | 43 (19+13+11) | W16a/W16b/W16c; review: independent agent | **Accepted 2026-10-20.** Review round 1 — P2-1 duplicate-index, closed red-first; round 2 — «accepted», P0/P1/P2=0 (`waves/reviews/review-p2-05.md`). Green targeted 43/43, core **378**, workspace **420**, fmt/clippy clean; gate passed `.gate/worker-2026-10-20T13-21-59-680Z.json`; mutation 3/3; artifacts `waves/RW-16/`. |
| RW-17 | P2-07 memory-only read-state/badges: `MESSAGE_ACK`, mentions, badge, ack-point | P2-07 | `state/read_state.rs` — `MessageAck`/parse; `read_state_reducer.rs` — lazy upsert, non-channel/stale guard, manual/non-manual mentions, version watermark; `read_state_selectors.rs` — mentions/guild/private/badge/ack-point/is_message_read; bez unread-count, REST ACK, SQLite/UI/seti | 40 (12+17+11) | W17a/W17b/W17c; review: independent agent | **Accepted 2026-10-20.** Review round 1 — P2-1 (channel fallback v `last_acked_id` daval lozhnyy read), P0/P1=0; closed red-first s11, s06 fixed; round 2 — «accepted», P0/P1/P2=0 (`waves/reviews/review-p2-07.md`). Green targeted 40/40, core **418**, workspace **460**, fmt/clippy clean; gate passed `.gate/worker-2026-10-20T15-10-13-521Z.json`; mutation 7/7; artifacts `waves/RW-17/`. |
| RW-18 | P2-09 memory-only presence: `PRESENCE_UPDATE`, scopes, activities, member-list/chunk presences | P2-09 | `state/presence.rs` — tipy/parser; `presence_reducer.rs` — upsert/offline+hidden/supplemental/member-list/chunk; `presence_selectors.rs` — statusy/scopes/activities/online ids; bez seti/UI/SQLite/iskhodyashchikh zaprosov | 54 (18+18+18) | W18a/W18b/W18c; review: independent agent | **Accepted 2026-10-21.** Round 1 — P0/P1=0, P2=2 + P3=14, vse closedy red-first; round 2 — «accepted», P0/P1/P2=0, P3=8 backlog (`waves/reviews/review-p2-09-round2.md`); mutation 13/13, md5 15/15; gate passed `waves/RW-18/gate-worker.json`; core **473**, workspace **515**; artifacts `waves/RW-18/`. |
| RW-19 | P2-10 memory-only voice states: snapshot, `VOICE_STATE_UPDATE`, timers, channel status | P2-10 | `state/voice/{types,parser,reducer,selectors}.rs`; klyuch `(guild,user)`, timers `(guild,channel)`, `channel_id=nil` — vykhod; bez live/seti/iskhodyashchikh voice-zaprosov | 18 (types 6, reducer 8, selectors 4) | CORE-1/W19; TL-2 I0/I1; review: independent agent | **Accepted 2026-10-21.** Review round 1 — P2-1 (snapshot additivity/last-wins/remove), closed red-first v09b/v09c/v09d + mutanty clear-before-loop/first-wins; round 2 — P0/P1/P2=0, P3=11 backlog (`waves/reviews/review-p2-10-round2.md`); B01 mutations 18/18; gate passed; core 506, workspace 548. |
| RW-20 | P2-11 memory-only typing TTL: `TYPING_START`, 10 000 ms, reset/deletion | P2-11 | `state/typing/{types,parser,reducer,selectors}.rs`; determinirovannoe vremya cherez `received_at_ms`; udalenie po `MESSAGE_CREATE`/READY; bez wire `TYPING_STOP`, unread-count, UI/seti | 15 (types 5, reducer 5, selectors 5) | CORE-2/W20; TL-2 I0/I1; review: independent agent | **Accepted 2026-10-21.** Review — P0/P1/P2=0, P3=10 backlog (`waves/reviews/review-p2-11.md`); typing mutations 8/8; fixture `RW-20/tests-data/typing-run2.jsonl` (28 START); t01 — capture `idx 117`, `s=192`. |
| RW-21 | B02: P2-08 settings-proto + P1-01 ETF full + P1-02 zstd strict + P0-04 conformance + P0-07 legal | P2-08, P1-01, P1-02, P0-04, P0-07 | `settings/{mod,wire,preloaded,frecency}.rs`; checked ETF `encode_checked`/boundaries; zstd `dead`/`Poisoned`/`is_finished`; `discord-conformance` repo→lab loader; `LEGAL.md`; bez state/REST/UI/seti/novykh zavisimostey | settings 18; codec 43; conformance 6; core 524; workspace 580 | CORE-3/W21, CORE-1/2, TEST-1, OPS-1; review: otdelnye agenty | **Accepted 2026-10-21.** P2-08 review round 2 P0/P1/P2=0, P3 (MAX_FIELDS) closed t18+M13; P1-01/P1-02 accepted with P3 (doc fixed); P0-04/P0-07 round 2 accepted with P3 (RE_REQUIRE_LAB=1) fixed. Mutatsii 11/11 codec + 13/13 settings; gate `waves/RW-21/gate-worker-b02.json`; artifacts `waves/RW-21/`. |
| RW-22 | B03: P3-15 i18n runtime + P4-01 voice signaling + P7-04 read-only API + P8-03 CI/release | P3-15, P4-01, P7-04, P8-03 | `discord-json`; `discord-i18n` hash/AST/plural/markdown; `discord-voice` signaling; `rest::resources`; `scripts/{ci,release}.sh` | JSON 11; i18n 8; voice 12; REST 6; CI 4; core 530; workspace 618 | CLIENT-1, VOICE-1, CORE-1, OPS-1; review A/B/C | **Accepted 2026-10-21.** Review A/B/C: P1 fence ne vosproizvyolsya (regression dobavlen), P2 plural/link/Debug/endpoint closedy red-first; round 2 P0/P1/P2=0; mutations 18/18; gate `waves/RW-22/gate-worker-b03.json`. |
| RW-23 | B04: P4-02 UDP media transport + P8-04 reproducible SBOM/licenses | P4-02, P8-04 | `discord-voice/src/udp.rs` + `tests/udp.rs`; `scripts/sbom.mjs`; `ci.sh`/`release.sh`; `sbom_contract.rs` | UDP 15; SBOM 5; core 530; workspace 638 | VOICE-1/W24a, OPS-1/W24b; review A/B | **Accepted 2026-10-21.** Discovery/keepalive/rotation/RTCP+SRTCP, fallible encode, peer/SSRC, rotate_if_due; SBOM 85 lock packages, strict SPDX, CI/release. Review A 4 P2 + B 2 P1/3 P2 closedy red-first; mutations 21 caught + 1 inconclusive; gate `waves/RW-23/gate-worker-b04.log`; workspace 638. |
| RW-24 | B05: P7-01 polnyy i18n — locale registry/CLDR cardinal+ordinal, chisla/valyuty, daty/vremya, 23 markdown-pravila | P7-01 | `discord-i18n/src/{locale,plural,format,format_data,markdown,lib}.rs`; `tests/{locale,plural,format,i18n,markdown}_contract.rs`; `tests/data/cldr-oracle.json`; generator `waves/RW-24/gen-format-data.mjs` | i18n 33 (8 lib + 25 novykh); core 530; workspace 663 | CLIENT-1/W25a, W25b; integration TL; review A/B | **Accepted 2026-10-21.** 36 lokaley/31 vklyuchena, CLDR cardinal/ordinal, exact `=N` do offset, chisla/valyuty, 4 stilya daty/vremeni UTC, `th` +543, localized names/timezone, skeleton, 23 markdown-pravila s safe URL/bounded depth, fallback current→default→en-US. Review A/B: P0/P1/P2=0 after red-first; mutations 20/20 caught; gate `waves/RW-24/gate-worker-b05-final.log` / `.gate/worker-2026-10-21T08-58-08-129Z.json`; artifacts `waves/RW-24/`. |
| RW-25 | B06 (headless): P4-03 SRTP crypto + P7-02 perf harness + P8-01 updater | P4-03, P7-02, P8-01 | `discord-voice/src/crypto/{mod,aes,gcm,chacha,kdf}.rs` + `src/srtp.rs` + `tests/{crypto_vectors,srtp_contract}.rs`; novyy kreyt `discord-perf` (`src/workload.rs`, `tests/perf_contract.rs`); novyy kreyt `discord-updater` (`src/{manifest,plan}.rs`, `tests/updater_contract.rs`); bez novykh vneshnikh zavisimostey (krome oflayn-kesha: `ring`, `sha2`, `serde_json`) | voice 65 (38 novykh); perf 17; updater 34 + 1 doctest; core 531; workspace 785 | VOICE-1/W26a, OPS-1/W26b, OPS-1/W26c; integratsiya TL-2 (I1: O(n²) presence); review: codex + TL-1 | **Accepted 2026-10-22.** P4-03: AES-256/GCM, ChaCha20/XChaCha20-Poly1305, HKDF-SHA1 bez zavisimostey; S-box algebraicheskiy (tablitsy ubrany — side-channel cherez kesh); raskladka `[clear][ciphertext][tag 16][counter 4 LE]` po capture (`waves/RW-25/research/RE-SRTP-LAYOUT.md`) i po funktsiyam klienta (`PacketCrypter::HeaderLength`/`SpaceRequiredToEncrypt`, `RE-NATIVE-SRTP.md`); iskhodyashchiy counter `0x80000000|n`, obshchiy RTP+SRTCP; AAD = otkrytaya chast v `*_rtpsize`; sovmestimost s wire Discord **ne zayavlena** (net klyuchevogo materiala). P7-02: 24 229 uchastnikov + 100 000 soobshcheniy na realnom redyusere, kholodnyy start ~47 ms pri byudzhete 1500 ms, geyt regressii >10 %. P8-01: manifest + Ed25519 nad iskhodnymi baytami tela, SHA-256 do peremeshcheniy, staged-zamena, otkat s proverkoy backup, zapret downgrade, zashchita ot path traversal. Review: 2×P0 + 4×P1 (codex + TL-1) closedy red-first; mutations **18/18**; gate passed `.gate/worker-2026-10-22T05-52-40-267Z.json`; artifacts `waves/RW-25/`.

| RW-26 | B09 (headless): P4-07 RTCP/RTP statistics + P4-11 regions/latency | P4-07, P4-11 | `discord-voice/src/{rtcp,reception,stats,regions}.rs` + `tests/{rtcp,reception,stats,regions}_contract.rs`; fikstury `fixtures/rtcp-voice-udp4-server.{bin,json}`, `fixtures/regions-rtc.json` | voice 114; workspace 869 | CORE-V1/W26a, CORE-V2/W26b; review TL-1 | **Accepted 2026-10-22.** P4-07: compound-parser RTCP (SR/RR/SDES, SRTCP-treyler 16/20, limit 32), RFC 3550 loss/jitter po otkrytym RTP-zagolovkam, `RttEstimator`/`StatsWindow`; test-only branch i `#[path]`-kopiya ustraneny. P4-11: parser `regions-rtc.json` 5×5, ranzhirovanie izmerennye→geo→leksika, `plan_probes`; wire-format prob ne izobretalsya. Mutatsii 19/19; gate `.gate/worker-2026-10-22T11-40-00-985Z.json`; artifacts `waves/RW-26/`. |
| RW-27 | B09 (headless): P7-01b ListFormat + P6-04 deep links + P6-03 autostart/badge | P7-01b, P6-04, P6-03 | `discord-i18n/src/{list,list_data}.rs` + `tests/list_contract.rs`; `discord-platform/src/{deep_link,autostart}.rs` + `tests/{deep_link,autostart}_contract.rs`; oracle `waves/RW-27/fixtures/list-oracle.json` | targeted 32; workspace 902 | CLIENT-1/W27a, PLATFORM-1/W27b; integration TL-2; review REV-2 | **Accepted 2026-10-22.** P7-01b: 31 lokal × 3 stilya × 3 tipa, 279 strok, 1674 keysa ICU. P6-04: `discord://` breakdown, RE-marshruty, `extract_deep_link`, redaction `raw`/`query`/`fragment` v `Debug`. P6-03: XDG Desktop Entry s Exec-quoting (reserved-nabor 1.5), Unity badge, `autostart_path`. Review: 2×P2 closedy red-first (Exec-quoting, dokazatelnaya baza); mutations list 6/6 + 2 ekvivalentnykh, platform 14/14; gate `.gate/worker-2026-10-22T13-58-02-813Z.json`; artifacts `waves/RW-27/`. |
| RW-28 | B10 (headless): P7-01c RelativeTimeFormat | P7-01c | `discord-i18n/src/{relative,relative_data}.rs` + `tests/relative_contract.rs`; oracle `tests/data/relative-oracle.json`; generator `waves/RW-28/gen-relative-data.mjs`; tochnye puti `plural_category_integer`/`format_integer_unsigned` | 12 (i18n 70; workspace 953) | CLIENT-1/W28a; review 2 prokhoda (nezavisimyy agent) | **Accepted 2026-10-22.** 31 lokal × 8 edinits × 3 stilya (744 string, 3392 keysa ICU), ICU-paritet `Auto` vklyuchaya lokalnye ±2 (TL-reshenie `waves/RW-28/DECISIONS.md`), tochnye tsifry/kategoriya dlya `i64` > 2^53, oracle vnutri kreyta. Review round 1 — P0 Auto ±2 + P1 tochnost, closedy; round 2 — P0/P1=0, P2 closedy pravkoy reporta. Mutatsii 8/8; gate `.gate/worker-2026-10-22T17-12-46-556Z.json`. |
| RW-29 | B10 (headless): P4-04 RTP-media + P4-12 reconnect + P7-01e `<t:…>` | P4-04, P4-12, P7-01e | `discord-voice/src/media/{mod,rtp,jitter}.rs` + `tests/media_contract.rs`; `discord-voice/src/reconnect.rs` + `tests/reconnect_contract.rs`; `discord-i18n/src/{timestamp,timestamp_data}.rs` + `tests/timestamp_contract.rs`; fikstura `fixtures/voice/rtp-pt120-oracle.json`; generatory `waves/RW-29/gen-{rtp-oracle.py,timestamp-data.mjs}` | media 13; reconnect 10+2; timestamp 14 (workspace 953) | VOICE-1/W29b, W29c; CLIENT-1/W29a; review voice — nezavisimyy agent | **Accepted 2026-10-22.** P4-04: RTP PT120/0xBEDE, marker/seq/ts+960, DTX-CN po dline shifroteksta, dzhitter-buffer mod 2¹⁶ s reorderom/dublyami/poteryami — oracle iz golden-pcap (10 paketov, 2 anomalii, opaque-profil zafiksirovan v TZ §6). P4-12: determinirovannyy bekoff 1–30 s, Resume/Identify/GiveUp, op9, sbros po READY/RESUMED. P7-01e: 7 stiley, parser `<t:…>`, UTC, 31 lokal × 6 stiley × 3 momenta (558) + R-oracle. Review voice — accepted, P0/P1/P2=0, 8 P3 v beklog. Mutatsii 8/8, 9/9, 6/6. |
| RW-30 | B11 (headless): P7-01d DurationFormat + P6-02 D-Bus notifications | P7-01d, P6-02 | `discord-i18n/src/{duration,duration_data}.rs` + `tests/duration_contract.rs`; `discord-platform/src/dbus/{mod,auth,message}.rs` + `notifications.rs` + `tests/{dbus_contract,notifications_contract}.rs`; oracley `tests/data/duration-oracle.json`, `fixtures/dbus/*`; generatory `waves/RW-30/{gen-duration-data.mjs,dbus/*.py}` | duration 13; dbus 7 + notifications 11 (workspace 984) | CLIENT-1/W30a, PLATFORM-1/W30b; review: independent reviewer | **Accepted 2026-10-22.** DurationFormat: 31 lokal × 3 stilya × 8 edinits (4464 patterna, 4092 keysa ICU), tochnye kategorii/gruppirovka. D-Bus: sobstvennyy std-klient (auth EXTERNAL, Hello, vyzovy, signaly), Notify/Close/GetCapabilities/GetServerInformation, sobytiya ActionInvoked/NotificationClosed; bez unsafe/novykh kreytov; oracle — privatnyy dbus-daemon + libdbus-mok. Review codex: P1 md5/P2 fikstura/P2 SKIP v mutationkh — closedy red-first (10/10, SKIP=0). Mutatsii 8/8 i 10/10; gate `.gate/worker-2026-10-22T17-54-26-464Z.json`. |
| RW-31 | B12 (headless): P6-06 power/idle + P2-03 SQLite-store (FFI) | P6-06, P2-03 | `discord-platform/src/power.rs` + `tests/power_contract.rs`; `discord-store/src/{lib.rs,ffi.rs}` + `tests/store_contract.rs`; mok `waves/RW-31/power/mock_power.py`; oracle `waves/RW-31/sqlite/oracle.py`; fikstura `fixtures/dbus/power.json` | power 12; store 14 (workspace 1010) | PLATFORM-1/W31a, STORE-1/W31b; review: independent reviewer | **Accepted 2026-10-22.** P6-06: ScreenSaver inhibit/uninhibit/is_active/idle_seconds (UnknownMethod→Ok(None)), PowerMonitor PrepareForSleep s obshchim dedlaynom, IdlePolicy (porogi — konfig). P2-03: SQLite cherez FFI k sistemnomu libsqlite3 3.54 bez rusqlite; migratsii user_version=1, upsert, keyset-paginatsiya, soft delete, kv; unsafe tolko v ffi.rs (26 SAFETY), unsafe_code=deny. Review: P1/P2 closedy red-first (yakorya mutations after fmt, md5, --nocapture/SKIP-detektsiya, dedlayn); mutations 7/7 i 7/7 (SKIP=0); gate `.gate/worker-2026-10-22T18-08-42-326Z.json`. |
| RW-32 | B13 (headless): P6-07 spellcheck + P6-02b hints | P6-07, P6-02b | `discord-platform/src/spellcheck.rs`, `ffi_hunspell.rs`, `notifications.rs` + tests; `fixtures/hunspell/*`, `fixtures/dbus/hints.json`; mok `waves/RW-32/dbus/mock_hints.py` | spellcheck 13; notifications 18; dbus 8 (workspace 1031) | PLATFORM-2/W32a, PLATFORM-1/W32b; review: independent reviewer | **Accepted 2026-10-22.** P6-07: FFI libhunspell 1.7.5, SpellChecker check/suggest/add/remove, tokenayzer s baytovymi smeshcheniyami (URL/e-mail/code/mentions/emoji propuskayutsya), usilennaya validatsiya .aff/.dic. P6-02b: hints `a{sv}` (urgency y, category/desktop-entry s, resident b), signatura Notify ne menyalas. Review: P1 validatsii slovarya i P3 utechki daemon closedy red-first; mutations 8/8 i 6/6 (SKIP=0); gate `.gate/worker-2026-10-22T18-33-30-356Z.json`. |
| RW-33 | B14 (headless): P1-10 keyring via libsecret FFI | P1-10 | `discord-platform/src/{keyring,ffi_secret}.rs`, `src/lib.rs` (+2 string), `tests/keyring_contract.rs`; oracle `secret-tool` + ksecretd | keyring 12 (workspace 1043) | SEC-1/W33a; review: independent reviewer | **Accepted 2026-10-22.** `Keyring::{connect,store,lookup,clear}` cherez `secret_service_*_sync` + `secret_schema_newv`; strict `xdg:schema` (bez DONT_MATCH_NAME); znachenie kopiruetsya po `length` (binarnye zapisi ne portyatsya); `GError`/`g_free` bez utechek, `put_string` osvobozhdaet key pri OOM; `!Send+!Sync`, `Debug` bez sekreta. Review: P1 obrezki na NUL, P1 kollizii chuzhikh zapisey, P2 utechki — closedy red-first; live 12 passed SKIP=0 (ksecretd+secret-tool), mutations 7/7; gate `.gate/worker-2026-10-22T18-57-30-507Z.json`. |
| RW-34 | B15 (headless): P6-02c abstract D-Bus + P6-02d hints ext | P6-02c, P6-02d | `discord-platform/src/dbus/mod.rs`, `src/notifications.rs` + tests; `waves/RW-34/dbus/mock_hints2.py`, `fixtures/dbus/hints-ext.json` | dbus 15; notifications 25 (platform 100; workspace 1057) | PLATFORM-1/W34a, PLATFORM-2/W34b; review: independent reviewer | **Accepted 2026-10-23.** Abstract: `BusAddress::{Path,Abstract}`, spiski/tcp/NUL/107B, connect cherez `from_abstract_name`+`connect_addr` bez unsafe. Hints: image-path, sound-name, transient, suppress-sound (vklyuchaya false), fiksirovannyy order; signatura Notify ne menyalas. Review: P0/P1/P2=0 (zhivaya abstract-shina bez harness-SKIP), mutations 6/6 i 6/6; gate `.gate/worker-2026-10-22T19-06-01-142Z.json`. |
| RW-35 | B16 (headless): fd-passing D-Bus + logind sleep-inhibitor | P6-06c | `discord-platform/src/{dbus/fd.rs,dbus/mod.rs,dbus/message.rs,power.rs}`, `Cargo.toml` (+libc), `tests/{dbus_contract,power_contract}.rs`, `waves/RW-35/power/mock_logind.py` | dbus 21; power 16 (platform 110; workspace 1067) | PLATFORM-1/W35a; review: independent reviewer na live shine | **Accepted 2026-10-23.** SCM_RIGHTS sendmsg/recvmsg, CMSG/CTRUNC, limity 16/253, closedie na vsekh putyakh; `Value::UnixFd`, `call_with_fds`, UNIX_FDS=9; `SleepInhibitor` Drop→close, `inhibit_sleep`; live logind zaregistriroval/snyal ingibitor. Review: P1 flak f03 (myuteks), P2 f05/utechka fd signalov, cmsg-granitsa — closedy; mutations 6/6; gate `.gate/worker-2026-10-22T19-41-26-571Z.json`. |

Acceptance RW-01 (2026-10-19): 24 tests `discord-codec` (13 vectors + 9 properties + 2 lib),
workspace 34/34 green (`cargo test --offline`); red-first — failures stubs (9+12) i 21/21 na
final tests (logi `/tmp/opencode/rw-01/red*.txt`); `cargo clippy -D warnings` i `cargo
fmt --check` clean; `gate:worker` passed (`.gate/worker-2026-10-19T10-49-38-957Z.json`); nezavisimoe
revyu (subagent): 95/95 automated hex/value checks against raw data i `gw-proto/msgs-*.jsonl`, dve
mutations caught by tests, vneshniy test na publichnom API 5/5; nits closedy doc comments
(encoder constraints >64 KiB atom / >4 GiB string — outside Discord corpus). Snimok priyomki:
`rust/crates/discord-codec/src/etf.rs` md5 `a636e9cd…`, `lib.rs` md5 `602968dd…`.

Acceptance RW-02 (2026-10-19): 9 tests zstd-strima (codec 33/33, workspace 43/43;
`cargo test --offline`); red-first — 9/9 failures na stub (`/tmp/opencode/rw-02/red.txt`; nit:
log snyat do finalnoy pravki tests — line numbers diverged, soderzhatelno red reproduced
reviewerom na final revision); `clippy -D warnings` / `fmt --check` clean; `gate:worker`
passed (`.gate/worker-2026-10-19T11-27-32-889Z.json`; meta 22 / unit 169 / smoke 20; reviewer
reran gate in full worktree — phases matched). Nezavisimoe revyu (subagent): oracley
independently derived using tools (137 frameov / 104 066 → 824 816 / 759 280, window 65 536 =
window descriptor `0x30`; Σbytes 824 679 + 137×`0x83`; 44 polnykh terms / smeshchenie 758 576 /
remainder 704; synthetic 67 → 139 264, md5 payload `4dbd601d…`), vneshniy test publichnogo API —
8 tests / 390 checks, 4/4 mutations caught. Snimok priyomki: `zstd.rs` md5 `0b4b9abb…`,
`lib.rs` `95e12674…`, `zstd_stream.rs` `4dd800a4…`, `tests/data/synth-zstd-frame.hexl`
`0a7742ae…`. Izvestnaya semantika: ruzstd retains tail sized to frame window (64 KiB) do ego
zaversheniya — documented; revisit (if needed) — P1-02.

Acceptance RW-03 (2026-10-19): 15 integratsionnykh tests sessii (`discord-core` — 17, workspace
59/59; `cargo test --offline`); red-first — 12/12 failures na stub (`/tmp/opencode/rw-03/red.txt`;
interrupted attempt had invalid red-log with compilation errors — saved kak
`red-prev-attempt.txt`); `clippy -D warnings` / `fmt --check` clean; `gate:worker` passed
(`.gate/worker-2026-10-19T12-29-11-179Z.json`; meta 22 / unit 169 / smoke 20). Nezavisimoe revyu
(subagent): oracley independently derived (timeline 137 / op10=1 / op11=23 / op0=113, s 1..113; first_ack
1214, last_ack 658 927, max delta 41 263, 9 delt v [41 050; 41 300]); HELLO-term (138 B) verified
mini-parserom ETF and independent decompression (`zstd` CLI + scratch `ruzstd --offline`); simulation
all 41,250 seeds — 0 zombie, ticks 11–12 (seed 1214 → 12); 11 mutations — 8 caught, 3 coverage gaps
closedy dop. testsmi (mutations snova fail targeted tests), vneshnie edge tests 6/6. Snimok
priyomki: `gateway.rs` md5 `f6fdb06b…`, `lib.rs` `21aea9a7…`, `gateway_session.rs` `c974da4d…`.
Granitsa volny: transport (soket/TLS/WS) not implemented — RW-04 (requires crates tokio/tungstenite i
razowner decision na set); T-GW-01/03 covered by logic without stub servera.

Acceptance RW-04 (2026-10-19): transport shlyuza — `discord-core::ws` (pipe zstd->terms->session,
`connect_and_run`, stub server v testskh; 13 tests, `discord-core` — 30, workspace 72/72;
`cargo test --offline`); red-first — 10/10 failures (first run `red-first.txt` s setevoy
build i reproducednyy `red.txt` na stub `ws-stub.rs`; reviewer independently reproduced
red byte-for-byte); `clippy -D warnings` / `fmt --check` clean; `gate:worker` passed
(`.gate/worker-2026-10-19T13-17-20-623Z.json`; meta 22 / unit 169 / smoke 20). Nezavisimoe revyu
(subagent): oracley independently derived (full decompression 824 816 B; verifiedo, chto payp poluchaet
44 polnykh terms = first 44 of complete stream, `last_seq == 36`, remainder 704 B — uderzhanie okna
ruzstd, not a loss; HELLO 41 250; 10/10 runs without flakes), 9 mutations — 6 caught srazu, 3 gaps
pokrytiya closedy dop. testsmi (Ping→Pong s payload, punctuality zombie ≤500 ms at interval
300 ms, desync-guard, counter Text; mutations snova fail targeted tests), lab-keys zafiksirovan
na fakticheskom kontrakte (see backlog). Snimok priyomki: `ws.rs` md5 `ca5791e3…`, `lib.rs`
`05ed2e5a…`, `ws_transport.rs` `1d091ee2…`, `Cargo.toml` `900f1e7b…`, `Cargo.lock` `d067aaa1…`.
Known relaxations/open questions: `wss://` without runtime case (nuzhen CA/live khost),
`Reconnect` does not send close-frame, branch `Text/Pong/Frame` — counter without strict shape assertions.
`s`-resume session continuity: lab window `.8608` starts with s=664 — pokrytie resume/op7/op9
vperedi (P1-05/RW-06).

Delivery RW-05 (2026-10-19): IDENTIFY (op2) + READY-summary — `discord-core::identify` (`build_identify`
= payload, `build_identify_frame` = provod `{op:2, d:…}`), `discord-core::ready` (`ReadySummary`),
surgical edits `ws.rs` (`RunOptions`, `connect_and_run_with`, IDENTIFY once after HELLO,
`GatewayPipe::last_ready`); 16 tests `identify_ready` (`discord-core` — 46, workspace 88/88;
`cargo test --offline`). Khod volny: worker-subagent prervan reyt-limitom after green-core (did not finish
workspace/clippy/geyt/report); coordinator identified and fixed spec defect — IDENTIFY ukhodil ploskim
payload vmesto `{op:2, d:{…}}` (red `red-contract-fix.txt`), completed the wave. red-first:
`red.txt` — 12 failed / 1 passed (passed was negative control for old path RW-04);
`clippy -D warnings` / `fmt --check` clean; `gate:worker` passed twice
(`.gate/worker-2026-10-19T13-58-37-508Z.json`, `…T14-12-35-509Z.json`; meta 22 / unit 169 / smoke 20).
Self-review (mutation-based; nezavisimyy reviewer-subagent 1× — reyt-limit): 11 mutations — 8 caught,
3 gaps closedy testsmi red-first against live mutants (`ready_accepts_string_user_id`,
`ready_accepts_int_partial`, `ws_identify_only_after_hello`; logi `/tmp/opencode/rw-05/mut/`).
Otchyot: `/tmp/opencode/rw-05/report.md`; independent review — first priority after snyatiya reyt-limita,
acceptance — after nego. Nit: verify wire formats against `pack({op, d})` before test lock.
Snimok: `identify.rs` md5 `17f2f310…`, `ready.rs` `c7c98489…`, `ws.rs` `659ee47e…`, `lib.rs`
`8986fa10…`, `identify_ready.rs` `ea4c7f18…`.

Delivery RW-06 (2026-10-19): RESUME/reconnect — `discord-core::resume` (`ResumeParams` s redaction
tokena, `build_resume_frame` `{op:6,d:{token,session_id,seq}}`, `ResumeState`,
`plan_reconnect`/`ReconnectAction`, porog `RESUME_MAX_DOWNTIME_MS = 30 000`), pravki `ws.rs`
(`connect_and_run_resume` — op6 vmesto op2 after pervogo HELLO, `resume_state()`,
`last_invalid_session_resumable()` iz op9 `d`, `TransportError::Resume`); 12 tests
`resume_reconnect` (`discord-core` — 58, workspace 100/100; `cargo test --offline`). Kontrakt
`RunOptions` ne rasshiryalsya — sushchestvuyushchie tests RW-05 ne pravilis. red-first na finalnoy
redaction: 11/12 failures (`red.txt`, `# EXIT=101`, md5 tests `60bf79f4…`; only passed
Debug-redaction), then green: 58/58 `discord-core`, 100/100 workspace, `resume_reconnect` ×5
stable; `clippy -D warnings`/`fmt --check` — after 3 nitov tests clean; `gate:worker` passed
(`.gate/worker-2026-10-19T14-33-22-949Z.json`). Oracles T-GW-05: run2-artifacts `.8269` (s 95..663,
569 op0), `.8608` (664..722, 59), `.10029` (1..331, op7), `.13072` (RESUMED 332..371, 40), seams
663+1=664 i 331+1=332; lab-keys skip-aware. Otchyot: `/tmp/opencode/rw-06/report.md`. Nit: porog
30 s — kontrakt plana (gaps in capture ~2.9 s); revisit on fresh capture. Nezavisimoe
revyu RW-05/RW-06 — after snyatiya reyt-limita subagentov; acceptance — after nego.

Acceptance RW-05 i RW-06 (2026-10-19): independent review (subagent;
`/tmp/opencode/review-rw-0506.md`) — **RW-05 «accepted s nitami»** (red-first podtverzhdyon logom i
sobstvennoy reproduktsiey stubs, md5 i lab-oracley matched, mutations 10/10; nits N1 summary
`mut/results.txt`, N2 string faylov, N3 promezhutochnye md5 — closedy coordinatorom) i **RW-06
«accepted s nitami»** (reproduction red 1:1, md5/oracley matched, 18/21 mutations caught; survivors
R4 threshold constant, R10 RESUME do HELLO, R11 error variants — coverage gaps, closedy red-first:
`red-gap-tests.txt` `# EXIT=101`, 10 passed/3 failed rovno po targeted testsm, md5 tests
`9852c2ca…`, copy of red version saved). Intsident: after mutations reviewera restore bez `touch`
ostavil stale rlib (tests linked s mutirovannoy build) — resolved `cargo clean` + polnyy
povtor progonov; recovery procedure now with `touch`. Finalnye chisla na cleany sborke:
`discord-core` **59** (resume_reconnect **13**), workspace **101/101**, `resume_reconnect` ×5
stable; clippy/fmt clean; `gate:worker` passed (`.gate/worker-2026-10-19T15-05-50-384Z.json`).
Snimok acceptedykh: `resume.rs` `211448f7…`, `ws.rs` `b822d6a6…`, `lib.rs` `69e299f2…`,
`resume_reconnect.rs` `9852c2ca…`; RW-05 without changes (`identify.rs` `17f2f310…`, `ready.rs`
`c7c98489…`, `identify_ready.rs` `ea4c7f18…`). Sleduyushchaya volna — RW-07 (P1-06, dispatch router,
T-GW-08).

Delivery RW-07 (2026-10-19): dispatch router — `discord-core::dispatch` (`DispatchKind`:
21 tip, `ALL`/`from_name`/`as_str`, `DispatchEvent`, `route_dispatch`), pravki `ws.rs`
(queue `events`, `events()`/`take_events()`, accumulated `unknown_dispatches()`; router
in loop `push_binary` after op9, do `drain`) i `lib.rs` (modul + re-eksporty); 11 tests
`dispatch_router` at delivery (`discord-core` — 70, workspace 112). Red-first: 3 passed /
8 failed (`# EXIT=101`, md5 `4fdfe8a8…`; passed negative controls U2/U5/P3), then green.
Oracles T-GW-08: union 7 snapshots `msgs-*.jsonl` — 21 tip / 1340 op0; soket `.1389` —
44 terms (36 op0, s = 1..36; ruzstd window retention, 704 B tail; timeline 113 — khvost ne
poteryan). Otchyot: `/tmp/opencode/rw-07/report.md`.

Acceptance RW-07 (2026-10-19): independent review (subagent; `/tmp/opencode/review-rw-07.md`) —
**«accepted s nitami»**, zero blockers: boundaries (tolko 4 fayla TZ; neighboring crates RW-06 md5 intact,
`Cargo.toml/lock` untouched), red-reproduction 1:1, oracley independently recalculated (python:
union / prefiks / hexl / timeline) 1:1, 18 mutations — 16 caught. Vyzhivshie m (name case:
`route_dispatch` s imenem «ready») i q (order `take_events`) + opts. nit `s:"nil"` — coverage gaps,
closedy coordinatorom red-first (`/tmp/opencode/rw-07/red-gap.log`: 9 passed / 3 failed rovno
po targeted, `# EXIT=101`, md5 tests `fbb4e1bf…`, kopiya `dispatch_router-gap-red.rs`); produkt
pri closedii ne menyalsya (`dispatch.rs` `d929b022…`, `ws.rs` `fa6e051a…`, `lib.rs`
`fa07ceb6…`). Finalnye chisla: `discord-core` **71** (`dispatch_router` **12**), workspace
**113/113**, ×5 stable; clippy `-D warnings`/fmt clean; `gate:worker` passed
(`.gate/worker-2026-10-19T16-18-34-369Z.json`). Test `dispatch_router.rs` `fbb4e1bf…`.
T-DATA-01 (breakdown READY-krupnyaka) excluded from wave — router delivers payload without parsing
(this is P2-01). Sleduyushchaya volna — RW-08 (P1-07, REST-klient, T-REST-01…03).

Start RW-08 (2026-10-19): P1-07, REST-klient — pervyy parallelnyy zapusk (PROCESS.md §3.13,
owner preferences). Tri workera concurrently: W8a — katalog 47 marshrutov (`routes.rs`,
T-REST-01), W8b — 6 auth-zagolovkov i klassy kodov (`headers.rs`, T-REST-02), W8c — taymauty/
retrai idempotentnykh (`retry.rs`, T-REST-03, lokalnyy stab `127.0.0.1`); after green —
refaktoring-worker W8d i independent review W8r. Kontrakty i test-dannye frozen v
`waves/RW-08/TZ.md`; no new dependencies added (coordinator decision within options of
vladeltsa: manual client/stab; real TLS-transport — otkrytyy vopros). Test-dannye —
`rust/crates/discord-core/tests/data/rest-*.tsv` (excerpts from `docs/re/rest-d02.json`, md5
`87d7a05d…`).

Acceptance RW-08 (2026-10-20): parallel wave — W8a/W8b/W8c launched concurrently, zatem
refaktoring W8d i independent review W8r. Red-first: 1/7, 2/3, 0/10 (`# EXIT=101`); after fmt
tests W8a i dobavleniya keysa N6 red re-captured (sm. `waves/RW-08/red/`). Summary: `discord-core`
95, workspace 137/137, 24 novykh keysa (T-REST-01…03), clippy/fmt clean, geyt passed; revyu —
«accepted s nitami», 14/14 mutations, surviving mutation `sleep(backoff)` closeda keysom N6
(red 0/11 → green 11/11, `# EXIT=101`). Otchyot: `waves/RW-08/report.md`; revyu:
`waves/reviews/review-rw-08.md`. Sleduyushchaya volna — RW-09 (P1-08, X-Super-Properties).

Start RW-09 (2026-10-20): P1-08, X-Super-Properties — vtoraya parallel wave (PROCESS.md §3.13).
Tri workera concurrently: W9a — base64 RFC 4648 (`rest/base64.rs`), W9b — JSON-zapis/breakdown 22
poley i base64 (`rest/super_properties.rs`, T-REST-04), W9c — accurate identity i assembly
(`rest/identity.rs`); zatem refaktoring W9d i independent review W9r. Kontrakty frozen
(karkas-zaglushki v repozitorii, md5 v `waves/RW-09/TZ.md`), excerpts-oracley — iz zhivogo snapshots
`docs/re/d02-snapshot` (requestId `95974.2323`; md5 vyderzhek v TZ). Novye zavisimosti
ne dobavlyayutsya (base64/JSON/UUID — vruchnuyu); network is not used. Otdelno prorabotan vopros
HTTP-transporta: `waves/TRANSPORT.md` (recommendation — manual client na tokio+tokio-rustls;
owner decision pending).

Acceptance RW-09 (2026-10-20): parallel wave — W9a/W9b/W9c concurrently, zatem W9d i
independent review W9r. Red-first: 0/5, 0/8, 0/8 (`# EXIT=101`); after fmt tests W9a i nita N1
red re-captured (`waves/RW-09/red/`; mutation-red — `w9a-red-mutation-n1.log`). Summary: `discord-core`
**116**, workspace **158/158**, 21 novyy keys (T-REST-04), clippy/fmt clean, geyt passed; revyu —
«accepted s nitami», 20/21 mutations; nits N1/N6 closedy red-first, N4/N5 fixedy, N2 annotirovan,
N3 — v backlog. Otchyot: `waves/RW-09/report.md`; revyu: `waves/reviews/review-rw-09.md`.
Queue: transportnaya volna (reshenie A), zatem P1-09 (rate-limit/errors).

## Backlog / Open Questions

- RW-09/N3 (identity policy): `IdentityConfig` with `Some("")` returns an empty string
  verbatim (not "absence"/`unknown`) — finalize upon live generator connection to REST/gateway.

- HTTP transport: **resolved 2026-10-20 — variant A** (manual HTTP/1.1 on tokio+tokio-rustls;
  analysis — `waves/TRANSPORT.md` §6); direct `tokio-rustls` (+ optional `httparse`/`bytes`)
  permitted in `Cargo.toml` without network fetching. Implementation scheduled after RW-09 (prior to P1-09/P2-06).

- RW-07/P1-06 (nit N4 review, design): buffer `GatewayPipe::events` grows until drained — draining
  is the responsibility of the caller (contract spec §1.3, documented); in the application loop/supervisor
  (after P1-07), define drain boundary to prevent memory growth in long sessions.
- W1c finding: real `functions/static/*.nm.txt` contains no symbols of type `w`; real
  corpus covers T/t/W; synthetic case `w` remains in `tests/unit/nm.test.mjs`
  (comment in test) — will match real data when such files appear.
- W3e decision: `summarize` without argument when multiple `calls-*.jsonl` exist in cwd takes
  the lexicographically first file — verify parity with `summarize-calls.sh` in M4.
- `src/core/not-implemented.mjs` — removed 2026-10-18 (Frida layer did not require it).
- Post-W4 audit (2026-10-18; report: `/tmp/opencode/review-w3/security-review-w4.md`):
  P2 — (1) `frida.mjs attach()` accumulates `messages` without draining (memory growth on hook path);
  (2) hook writes via `appendFile` on each call (no backpressure, write errors swallowed — requires
  write-stream + dropped line counter); (3) missing numeric validation for `--pid/--limit/--max-symbols/
  --arg-count/--duration-ms` (NaN reaches agent: `limit=NaN` -> unlimited Interceptor.attach);
  (4) agent `moduleEntry`: `Process.getModuleByName` outside try -> typo in `--modules` crashes entire inventory.
  P3 — repeated `hook()` in agent without clearing `activeHooks`; `packageVersion` without try (HARD-06);
  SIGINT unhandled during `--duration-ms`; `--pattern` compiled in target (validate in CLI).
  Tested not reproducible: stdout-EPIPE on Node 24 (1 MB stream to `head` -> code 0).
- `git init` — completed 2026-10-18: `master` branch, baseline commit; private directories
  (`logs/`, `functions/`, `evidence/`) and `.gate/`, `node_modules/` in `.gitignore`.
  Wave branches `wave/<N>`, tags `v<X.Y.Z>`; remote (GitHub) push upon owner instruction.
- Migration of `/tmp/opencode/discord-ui-walkthrough/` -> `tools/`?
- `logs/ui-clicks-*.jsonl` — click event format without consumer: evaluate CLI command need.
- `screenshotName`: real names in `evidence/screens` may contain `%`/`?`
  (verify in W1d and lock behavior).
- RW-04/P1-02: lab case `lab_capture_stream_feeds_pipe` locks `last_seq == Some(36)` instead of 113 —
  ruzstd window retention (64 KiB) until frame termination; retained data not lost. Revisit decoder
  in P1-02 (streaming completion/alternative decoder) and restore strict check `s == 113`.
- RW-04 (P4): `start + Duration::from_millis(ms)` in transport loop theoretically overflows on
  adversarial `heartbeat_interval` (unreachable on Linux; safer with saturating `checked_add`) —
  address on next pass.
- Process (nit review RW-04): include `# EXIT=...` and test file md5 in wave red/green logs — proof
  that "red was taken on final test" must be established from log contents, not file mtime.

## RW-42: Complete Migration from Tauri to GPUI Kit (`gpui-kit`) and Architecture Modernization

- **Goal**: Complete migration to native GPU Rust UI framework `./gpui-kit` (75+ components, VirtualList, Markdown TextView, Dialogs, Webview). Retirement of 107 JS files, CSS, IPC bridge, and ad-hoc scripts.
- **Architectural Manifest**: [`docs/specs/GPUI-ARCHITECTURE-MIGRATION.md`](docs/specs/GPUI-ARCHITECTURE-MIGRATION.md)
- **Detailed Wave Task Board**: [`waves/RW-42-GPUI-MIGRATION/TASKS-GPUI.md`](waves/RW-42-GPUI-MIGRATION/TASKS-GPUI.md)

### Wave RW-42 Task Board

 | ID | Priority | Department / Owner | Task | Status | 
 | --- | --- | --- | --- | --- | 
 | **TASK-701** | `P0` | **Chief Architect** + **DevOps** | Initialize `discord-gpui` crate, 1280x720 window, launch `gpui-kit` | `[ ]` todo | 
 | **TASK-702** | `P0` | **Chief Architect** + **G1 Backend** | Reactive `AppState` model (`Model<T>`), Tokio <-> GPUI bridge | `[ ]` todo | 
 | **TASK-703** | `P0` | **G3 Design Lead** | Theme `DiscordDarkTheme` in `gpui-kit::ActiveTheme`, assets and typography | `[ ]` todo | 
 | **TASK-704** | `P0` | **G1 Backend Lead** | REST v9 client based on `reqwest` + `backoff` (replaces ad-hoc HTTP) | `[ ]` todo | 
 | **TASK-705** | `P0` | **G4 QA Lead** | Headless UI test harness on `gpui::TestAppContext` (without Chromium/Xvfb) | `[ ]` todo | 
 | **TASK-706** | `P1` | **G2 Frontend Lead** | Three-pane `AppLayout` layout based on `h_resizable` | `[ ]` todo | 
 | **TASK-707** | `P1` | **G2 Frontend Lead** | Server rail `ServerRail`, status indicators, unread badges | `[ ]` todo | 
 | **TASK-708** | `P1` | **G2 Frontend Lead** | Channel list `ChannelSidebar`, categories, current user panel | `[ ]` todo | 
 | **TASK-709** | `P1` | **G2 Frontend Lead** + **G1 Backend** | Virtual chat scrolling `ChatView` based on `virtual_list` (100k) | `[ ]` todo | 
 | **TASK-710** | `P1` | **G2 Frontend Lead** + **G3 Design** | Message rendering: `TextView` (Markdown + Syntect syntax highlighting) | `[ ]` todo | 
 | **TASK-711** | `P1` | **G2 Frontend Lead** | Message input `MessageBar` based on `gpui-component::input` | `[ ]` todo | 
 | **TASK-712** | `P2` | **G2 Frontend Lead** + **Architect** | hCaptcha solver modal dialog based on `gpui-webview::WebView` | `[ ]` todo | 
 | **TASK-713** | `P2` | **G2 Frontend Lead** | Server creation and join dialogs (`Join/Create Guild`) | `[ ]` todo | 
 | **TASK-714** | `P2` | **G2 Frontend Lead** | Virtualized member list `MemberSidebar` (25k) | `[ ]` todo | 
 | **TASK-715** | `P2` | **G2 Frontend Lead** + **G3 Design** | Settings dialog `SettingsDialog` (account, voice, sound, theme) | `[ ]` todo | 
 | **TASK-716** | `P2` | **G2 Frontend Lead** | Native right-click context menu (`menu::DropdownMenu`) | `[ ]` todo | 
 | **TASK-717** | `P3` | **G5 Voice Lead** | Native audio engine based on `cpal` + VAD + Push-to-Talk | `[x]` accepted | 
 | **TASK-718** | `P3` | **G5 Voice Lead** + **G2 Frontend** | Voice connection panel and speaking indicators | `[x]` accepted | 
 | **TASK-719** | `P3` | **G5 Platform Lead** | System tray (`tray-icon`) and native notifications (`notify-rust`) | `[ ]` todo | 
 | **TASK-720** | `P3` | **G4 QA Lead** + **PM Review** | Final release certification, 120 FPS, RAM < 60 MB, 25/25 mutations | `[ ]` todo | 
 | **TASK-721** | `P1` | **G3 Design** + **G2 Frontend** | Resolve VoicePanel collision on user profile (Arrow 2, 52px flex-none) | `[x]` accepted | 
 | **TASK-722** | `P1` | **G1 Backend** + **G2 Frontend** | Full-featured modal for server creation and joining via `+` button (Arrow 1) | `[x]` accepted | 
 | **TASK-723** | `P1` | **G1 Backend Lead** | Bi-directional message dispatch via official REST API v9 | `[x]` accepted | 
 | **TASK-724** | `P1` | **G2 Frontend** + **G3 Design** | Public community exploration screen ExploreView (Compass button) | `[x]` accepted | 
 | **TASK-725** | `P0` | **G4 QA Lead** | Expand test harness to 48/48 contracts | `[x]` accepted | 
 | **TASK-726** | `P1` | **G2 Frontend Lead** | Prevent event interception by inactive modals in AppLayout (.children) | `[x]` accepted | 
 | **TASK-727** | `P1` | **G1 Backend Lead** | Fetch and synchronize real server channels (GET /guilds/{id}/channels) for servers | `[x]` accepted | 
 | **TASK-728** | `P0` | **G4 QA Lead** | Expand test harness to 53/53 contracts with live invite verification | `[x]` accepted | 
 | **TASK-729** | `P1` | **G1 Backend Lead** | Fetch and cache guild channel messages via REST API (`GET /channels/{id}/messages`) | `[x]` accepted | 
 | **TASK-730** | `P1` | **G1 Backend Lead** | Dynamic member discovery and population from message authors (`MemberSidebar`) | `[x]` accepted | 
 | **TASK-731** | `P1` | **G2 Frontend Lead** | Synchronize guild channel selection and automatically select first text channel | `[x]` accepted | 
 | **TASK-732** | `P1` | **G2 Frontend Lead** | Interactive invite link input in `CreateGuildModal` (Arrow 1: cursor, Backspace, Paste) | `[x]` accepted | 
 | **TASK-733** | `P1` | **G5 Voice Lead** + **G2 Frontend** | Eliminate overlay and fake voice connection status in `VoicePanel` (Arrow 2) | `[x]` accepted | 
 | **TASK-734** | `P0` | **G4 QA Lead** | Expand test harness to 55/55 GPUI contracts | `[x]` accepted | 
 | **TASK-735** | `P1` | **G1 Backend Lead** | Real server connection integration via `POST /invites/{code}` | `[x]` accepted | 
 | **TASK-736** | `P2` | **G5 Voice Lead** | Voice session lifecycle management and CPAL audio device integration | `[x]` accepted | 
 | **TASK-737** | `P1` | **G3 Design Lead** | Styling and visual parity of add server modal per Discord Dark standards | `[x]` accepted | 
 | **TASK-738** | `P1` | **G2 Frontend Lead** | Isolate sidebar bottom panel containers and user profile | `[x]` accepted | 
 | **TASK-739** | `P1` | **G1 Backend** + **G2 Frontend** | Display real authenticated user profile instead of test mocks | `[x]` accepted | 
 | **TASK-740** | `P0` | **G4 QA Lead** + **DevOps** | Automated regression verification of build and release binary | `[x]` accepted | 
 | **TASK-741** | `P1` | **G1 Backend Lead** | Real REST API message actions: delete, pin, unpin (`DELETE /channels/{id}/messages/{id}`) | `[x]` accepted | 
 | **TASK-742** | `P1` | **G5 Voice Lead** | Direct Voice UDP Media Streamer (`tokio::net::UdpSocket` + Keepalive + RTP Opus AES-256-GCM) | `[x]` accepted | 
 | **TASK-743** | `P2` | **G5 Platform Lead** | Screen sharing via Linux XDG Desktop Portal and PipeWire (`ScreenCastManager` / `ashpd`) | `[x]` accepted | 
 | **TASK-744** | `P2` | **G5 Platform Lead** | Native desktop D-Bus notifications and Linux system tray (6-item menu, badges) | `[x]` accepted | 
 | **TASK-745** | `P2` | **G1 Backend Lead** | Discord Local RPC IPC protocol server (`$XDG_RUNTIME_DIR/discord-ipc-0`) per `rust-discord-activity` reference | `[x]` accepted | 
 | **TASK-746** | `P0` | **G4 QA Lead** + **DevOps** | Automated regression contract harness 61/61 (100% Pass) | `[x]` accepted | 
 | **TASK-747** | `P1` | **G3 Design** | Discord Dark Design Polish styling parity and message hover effects | `[x]` accepted | 
 | **TASK-748** | `P1` | **G5 Voice/Platform** | Hardware GPU pipeline for PipeWire frames and screen capture | `[x]` accepted | 
 | **TASK-749** | `P1` | **G2 Frontend** | Floating quick action bar above message (Floating Action Bar) | `[x]` accepted | 
 | **TASK-750** | `P0` | **G1 Backend** | Discord Gateway Opcode 4 (Voice State Update) | `[x]` accepted | 
 | **TASK-751** | `P1` | **G1 Backend** | G1 Backend + G7 REA: Robust parser for read_state.last_acked_id (Snowflake String/Integer/Null) | `[x]` accepted | 
 | **TASK-752** | `P2` | **G2 Frontend** | G2 Frontend: Message loading latency optimization for messages_page (< 300 ms) | `[x]` accepted | 
 | **TASK-769** | `P0` | **G1 Backend** | G1 Backend: Handshake Wait & Verified Cache Hydration in ready() | `[x]` accepted | 
 | **TASK-770** | `P0` | **G2 Frontend** | G2 Frontend: Non-destructive server and friend hydration in app.js and dm-friends.js | `[x]` accepted | 
 | **TASK-771** | `P0` | **G3 Design** | G3 Design & G2 UI: Precise Voice Panel and User Bar parity per official Discord | `[x]` accepted | 
 | **TASK-772** | `P0` | **G5 Voice / Platform** | G5 Voice: Eliminate delay in displaying voice channel participants | `[x]` accepted | 
 | **TASK-773** | `P0` | **G4 QA** | G4 QA: Automated tests for non-destructive hydration and UI parity | `[x]` accepted | 
 | **TASK-774** | `P0` | **DevOps / Integration** | DevOps: Release binary compilation, SHA256SUMS, and push to remote mirror | `[x]` accepted | 
 | **TASK-775** | `P0` | **G5 Voice & Platform** | G5 Voice / Core: Normalize WebRTC source (sourceId = -1) and ensure reliable encoder start | `[x]` accepted | 
 | **TASK-776** | `P0` | **G5 Voice & Platform** | G5 Voice / Core: Native Wayland capture (KWin ScreenShot2) and on-the-fly window switching | `[x]` accepted | 
 | **TASK-777** | `P0` | **G2 Frontend** | G2 Frontend: Preview synchronization (PIP), close button, and source switching | `[x]` accepted | 
 | **TASK-778** | `P0` | **G1 Backend** | G1 Backend + G5 Voice: Gateway signaling OP 18/19 and SFU codec negotiation | `[x]` accepted | 
 | **TASK-779** | `P0` | **G4 QA** | G4 QA: Comprehensive streaming regression tests and official client verification | `[x]` accepted | 
 | **TASK-780** | `P1` | **G3 Design** | G3 Design & UI: Stream quality overlay parity (720p/1080p, 30/60 FPS, H264) and settings menu | `[x]` accepted | 
 | **TASK-781** | `P1` | **G7 RE / REA** | G7 RE / REA: Audit of Go Live signaling and Video/SDP contracts (Voice Opcodes 1, 12, 16, 22) | `[x]` accepted | 
 | **TASK-783** | `P1` | **G5 Voice & Platform** | G5 Voice & G2 UI: On-the-fly dynamic stream parameters switcher (720p/1080p, 30/60 FPS) | `[x]` accepted | 
 | **TASK-785** | `P0` | **G1 Backend** | G1 Backend / Voice: Handling and generation of Voice Opcode 16 (RTC Ping / DAVE) and Opcode 12 (VIDEO SSRC Mapping) | `[x]` accepted | 
 | **TASK-787** | `P1` | **G2 Frontend** | G2 Frontend: Precise parity of RTC connection status popout (#live-rtc-popout) and ping indicator | `[x]` accepted | 
 | **TASK-788** | `P1` | **G1 Backend** | G1 Backend: Nitro status integration (premium_type) and unlocking 1080p 60 FPS stream presets | `[x]` accepted | 
 | **TASK-789** | `P0` | **DevOps** | W8-DevOps: Build and verify Build-005 release artifact and SHA256SUMS | `[x]` accepted | 
 | **TASK-792** | `P0` | **G4 QA** | W8-DevOps/QA: End-to-end verification of Build-005 artifact and release of QA-BUILD-005.md | `[x]` accepted | 
 | **TASK-794** | `P0` | **G1 Backend** | W1-G1: Stream Signaling Remediation (Broadcaster OP 18 and OP 20 isolation) | `[x]` accepted | 
 | **TASK-795** | `P0` | **G2 Frontend** | W2-G2: Stage Viewer & PiP Persistence (Screen Share UI) | `[x]` accepted | 
 | **TASK-796** | `P0` | **G5 Voice** | W3-G5: Voice Native Command Queue & Capture Target Pipeline | `[x]` accepted | 
 | **TASK-797** | `P0` | **G4 QA** | W4-G4: Stream Broadcaster vs Viewer Isolation QA Suite | `[x]` accepted | 
 | **TASK-610** | `P1` | **G2 Frontend** + **G1 Backend** | Interactive Webview Captcha solver (STUB-28 / CARD-TEL-006) | `[x]` accepted | 
 | **TASK-611** | `P1` | **G1 Backend** | W5-G1: Full support for REST POST /guilds (BFF) | `[x]` accepted | 
 | **TASK-612** | `P1` | **G2 Frontend** | W6-G2: Server creation UI templates and wizard (STUB-05) | `[x]` accepted | 
 | **TASK-613** | `P1` | **G5 Voice / Platform** | Screen and window capture via Desktop Portal / PipeWire (Task-601) | `[x]` accepted | 
 | **TASK-614** | `P1` | **G1 Backend** + **G5 Media** | Atomic buffer 60 FPS (Latest-Frame Ring Buffer) (Task-603) | `[x]` accepted | 
 | **TASK-615** | `P1` | **G5 Voice** | RTP video stream packet assembly and DAVE MLS v1 (Task-602) | `[x]` accepted | 
 | **TASK-616** | `P1` | **G2 Frontend** | Video player and Picture-in-Picture UI (Task-604) | `[x]` accepted | 
 | **TASK-617** | `P1` | **G1 Backend** | BFF for REST message search (STUB-14) | `[x]` accepted | 
 | **TASK-618** | `P1` | **G2 Frontend** | Search popup UI with autocomplete (STUB-14) | `[x]` accepted | 
 | **TASK-619** | `P1` | **G1 Backend** | Discord Permissions v9 bitwise calculator (STUB-17) | `[x]` accepted | 
 | **TASK-620** | `P2` | **G2 Frontend** | Message and user context menus (STUB-18) | `[x]` accepted | 
 | **TASK-621** | `P0` | **DevOps** + **G4 QA** | Headless Xvfb E2E Runner for CI (M2-E2E) | `[x]` accepted | 
 | **TASK-622** | `P0` | **G4 QA** + **PM Review** | Mutation test suite 15/15 (Quality Gate v2) | `[x]` accepted | 
 | **TASK-798** | `P1` | **G7 RE** | W7-G7: Gateway Stream Protocol & Codec RE Audit | `[x]` accepted | 
 | **TASK-799** | `P2` | **G6 Tech Writer** | W8-G6: Release Notes & Stream Architecture Parity Matrix | `[x]` accepted | 
 | **TASK-800** | `P0` | **G5 Voice / Platform** | AV1 video codec negotiation and configuration in WebRTC RTCControlSocket | `[x]` accepted | 
 | **TASK-801** | `P0` | **G5 Voice / Platform** | NVIDIA NVENC hardware encoder integration for streaming (RTX 5070 Ti) | `[x]` accepted | 
 | **TASK-802** | `P1` | **G5 Voice / Platform** | Proper VP8/VP9 codec support and switching in WebRTC | `[x]` accepted | 
 | **TASK-803** | `P1` | **G1 Backend Lead** | Add AV1 OBU packetizer and RTP fragmentation to discord-media | `[x]` accepted | 
 | **TASK-804** | `P1` | **G5 Voice / Platform** | PipeWire / Wayland Portal capture optimization without CPU delays | `[x]` accepted | 
 | **TASK-805** | `P0` | **G2 Frontend Lead** | Eliminate 'Connecting to video stream...' overlay for stream broadcaster | `[x]` accepted | 
 | **TASK-806** | `P1` | **G2 Frontend Lead** | Automatic stream collapse into floating PiP on channel change | `[x]` accepted | 
 | **TASK-807** | `P1` | **G2 Frontend Lead** | Eliminate audio device polling spam (voice_set_audio_device) | `[x]` accepted | 
 | **TASK-808** | `P2` | **G1 Backend Lead** | Optimistic chat message rendering and non-blocking ACK | `[x]` accepted | 
 | **TASK-809** | `P2` | **G1 Backend Lead** | Permission error handling for create_invite on voice channels | `[x]` accepted | 
 | **TASK-810** | `P0` | **G1 Backend Lead** | Integrate accept_invite_and_join_guild and auto-confirmation PUT /guilds/{id}/requests/@me | `[x]` accepted | 
 | **TASK-811** | `P0` | **G2 Frontend Lead** | Server rules screening banner and modal window | `[x]` accepted | 
 | **TASK-812** | `P0` | **G1 Backend Lead** | Store and serialize message components (ActionRow & Buttons) in data.rs | `[x]` accepted | 
 | **TASK-813** | `P0` | **G2 Frontend Lead** | Render Discord message component buttons (Message ActionRow & Buttons UI) | `[x]` accepted | 
 | **TASK-814** | `P0` | **G1 Backend Lead** | Implement POST /api/v9/interactions for button clicks (MESSAGE_COMPONENT type 3) | `[x]` accepted | 
 | **TASK-815** | `P1` | **G2 Frontend Lead** | Handle verification button click in chat and IPC binding click_message_component | `[x]` accepted | 
 | **TASK-816** | `P1` | **G2 Frontend Lead** | Interactive completion of Reaction Verification in rules channels | `[x]` accepted | 
 | **TASK-817** | `P1` | **G2 Frontend** + **G5 Platform** | Eliminate false captcha challenge and integrate WebView widget instead of manual input | `[x]` accepted | 
 | **TASK-818** | `P0` | **G2 Frontend** | Local bridge hcaptcha-bridge.html and resolution of XML AccessDenied | `[x]` accepted | 
 | **TASK-819** | `P0` | **G5 Voice / Platform** | Stabilization of SSRC, continuous 90 kHz timestamps, and RTCP PLI/FIR (Errors 2007 and 2000) | `[x]` accepted | 
 | **TASK-820** | `P0` | **G1 Backend** | Eliminate voice_connect spam and duplicate Gateway OP 4 updates | `[x]` accepted | 
 | **TASK-821** | `P1` | **G2 Frontend** | Parity of Voice Channel Stage Banner (Voice channel preview card, frame 032) | `[x]` accepted | 
 | **TASK-822** | `P1` | **G1 Backend** | HTTP/2 Keep-Alive pool and backoff for resolve_invite_info (eliminate 2.6s freeze) | `[x]` accepted | 
 | **TASK-823** | `P2` | **G5 Voice / Platform** | Eliminate AVError codes 2004, 2005, 2009, 2010, 2016, 2017 and RTC Quality Status Bar | `[x]` accepted | 
 | **TASK-824** | `P0` | **G4 QA** | Comprehensive verification of all codes 2000-2018, 15/15 mutation gate, and clean GDB logs | `[x]` accepted | 
 | **TASK-825** | `P0` | **G1 Backend** | Invites, Captcha, and seamless session authorization fallback on guild join | `[x]` accepted | 
 | ↳ **TASK-825.1** | `P0` | **G1 Backend** | Fallback in token_or to data::resolve_live_token() (eliminating missingToken) | `[x]` accepted | 
 | ↳ **TASK-825.2** | `P0` | **G1 Backend** | Public access to resolve_invite (Optional Bearer per REST v9) | `[x]` accepted | 
 | ↳ **TASK-825.3** | `P0` | **G1 Backend** | Client headers X-Context-Properties in accept_invite_with_captcha | `[x]` accepted | 
 | ↳ **TASK-825.4** | `P1` | **G2 Frontend** | Validate and preserve invite input in UI on errors | `[x]` accepted | 
 | **TASK-826** | `P1` | **G5 Voice / Platform** | Support quiet AFK channels and prevent spurious RTC server wait | `[x]` accepted | 
 | ↳ **TASK-826.1** | `P1` | **G1 Backend** | Determine is_afk_channel via guild.afk_channel_id in data.rs and channels.rs | `[x]` accepted | 
 | ↳ **TASK-826.2** | `P1` | **G2 Frontend** | Display '🔕 AFK Channel' status in UI without waiting for RTC server | `[x]` accepted | 
 | ↳ **TASK-826.3** | `P1` | **G5 Voice** | Suppress voice core reconnection timers for quiet AFK channels | `[x]` accepted | 
 | **TASK-827** | `P1` | **G2 Frontend** | Synchronize chat and banner upon switching voice channels | `[x]` accepted | 
 | ↳ **TASK-827.1** | `P1` | **G2 Frontend** | Synchronize Voice Text Channel Chat ID on voice channel click | `[x]` accepted | 
 | ↳ **TASK-827.2** | `P2` | **G2 Frontend** | Slide-out side panel via 'Open Chat' button on stage banner | `[x]` accepted | 
 | **TASK-828** | `P2` | **G3 Design** | Optimize RTC status widget layout in compact sidebar | `[x]` accepted | 
 | ↳ **TASK-828.1** | `P2` | **G3 Design** | Responsive CSS flexbox for live-voice-status-badge and voice-rtt-pill in 240px | `[x]` accepted | 
 | ↳ **TASK-828.2** | `P2` | **G2 Frontend** | Rich tooltip with RTC details, ping, and voice server region | `[x]` accepted | 
 | **TASK-829** | `P0` | **G4 QA** | Automated regression contracts for invites, AFK, and headers | `[x]` accepted | 
 | ↳ **TASK-829.1** | `P0` | **G4 QA** | Rust contract tests/invites_live_fallback_contract.rs | `[x]` accepted | 
 | ↳ **TASK-829.2** | `P0` | **G4 QA** | JS Unit tests tests/unit/afk-voice-status.test.mjs and invite-input.test.mjs | `[x]` accepted | 


