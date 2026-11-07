# REVIEW-163A — task-250-ui-slowmode (G2 UI)

**Verdict: APPROVED**

Reviewer: DeepSeek (Techncial Reviewer, terminal `term_e462868c-cc72-47f1-a358-92f35a25713b`)
Scope reviewed: `rust/gui-spike/ui/features/slowmode.js`, `tests/unit/slowmode.test.mjs`
Method: token-lean — only the two files above plus `REPORT.md`, one targeted test command, 5 mutations.

## Targeted test

```
node --test tests/unit/slowmode.test.mjs
tests 16 | pass 16 | fail 0 | duration_ms 35.72   (exit 0)
```

First baseline attempt raced a parallel mutation process in the same worktree and reported a polluted failure
(`actual 20000, expected 21600`). It was re-run after restore and is green; the failing sample is ignored as harness
interference, not a code defect.

## Mutation testing — 5/5 killed

Each mutation was applied to `slowmode.js`, the targeted test was run, and the original bytes were restored from backup.

| # | Mutation | Location | Test result |
| --- | --- | --- | --- |
| M1 | `MAX_SLOWMODE_SECONDS` `21600` → `20000` | L8 | killed (exit 1) |
| M2 | `Math.trunc(seconds)` → `Math.round(seconds)` | L23 | killed (exit 1) |
| M3 | `if (clamped < 60)` → `if (clamped <= 60)` | L34 | killed (exit 1) |
| M4 | `if (clamped < 3600)` → `if (clamped < 1800)` | L35 | killed (exit 1) |
| M5 | `... / 1000)` → `... / 100)` in elapsed seconds | L52 | killed (exit 1) |

Score: **5 / 5 (100%)**. No surviving mutant, so the contract is pinned on constants, truncation, both format
boundaries and the countdown arithmetic.

## Integrity

```
1c1954642ef65be4eb789c60899e3d4c  rust/gui-spike/ui/features/slowmode.js
8c6042eb74fe3f1ccf5fc67ddb6cf32c  tests/unit/slowmode.test.mjs
```

`slowmode.js` after the mutation sweep hashes to `1c1954642ef65be4eb789c60899e3d4c`, identical to the checksum in
`REPORT.md`; `cmp` against the pre-mutation backup reports byte-identical. The test file was never modified.

## Contract notes

- `clampSlowmodeSeconds` rejects non-`number` input (`'30'` → `0`). Consistent with "numeric input" and covered by a test.
- `computeSlowmodeRemainingSeconds` with invalid `nowMs` returns the full cooldown, because `(Number(nowMs) || 0)`
  collapses to `0` and elapsed clamps to `0`. The tests assert the implemented formula, not an independent oracle;
  the behaviour is documented in `REPORT.md` §Deviations. Non-blocking: **P3** — a future card may prefer `0`
  remaining for an invalid clock, but nothing in the current contract requires it.
- Boundaries are exercised: `0`, negative, `NaN`, `Infinity`, numeric string, `0.9`, `59.99`, `60`, `3599`, `3600`,
  `21600`, `999999`, future last-message timestamp, bypass flag.

## Scope compliance

Only the two assigned files are in play; no shared files, docs or `waves/*` state touched by the worker.
`gate:worker` and `git commit` were not run, by instruction.

## Blocking findings

None (P0 = 0, P1 = 0). One P3 observation above, deferrable.
