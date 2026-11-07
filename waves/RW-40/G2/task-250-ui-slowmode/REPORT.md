# task-250-ui-slowmode — slow mode helpers (G2 UI)

## Scope
Two files only, pure JS helper for the channel settings UI.

| File | Role |
| --- | --- |
| `rust/gui-spike/ui/features/slowmode.js` | implementation (`// @ts-check`) |
| `tests/unit/slowmode.test.mjs` | contract tests, `node --test` |

## Exports
- `MAX_SLOWMODE_SECONDS = 21600`
- `SLOWMODE_PRESETS_SECONDS` — frozen ladder `[0,5,10,15,30,60,120,300,600,900,1800,3600,7200,21600]`
- `clampSlowmodeSeconds(seconds)` — non-number/non-finite → `0`, clamp to `[0, 21600]`, `Math.trunc`
- `formatSlowmodeIntervalRu(seconds)` — `Медленный режим выключен` / `N сек.` / `N мин.` / `N ч.`
- `computeSlowmodeRemainingSeconds(rateLimitPerUser, lastMessageSentAtMs, nowMs, bypassSlowmode = false)`

## TDD
1. Red: `node --test tests/unit/slowmode.test.mjs` → exit 1,
   `SyntaxError: The requested module ... does not provide an export named 'SLOWMODE_PRESETS_SECONDS'`.
   Log: `red.log` (md5 `52987bed0d4e41c51da9f571b196f46c`).
2. Green: same command → exit 0, `tests 16 / pass 16 / fail 0` (~34 ms).
   Log: `green.log` (md5 `c5190712d7969b6ebbbcc986457e0f16`).

## Deviations
- `clampSlowmodeSeconds` rejects non-`number` input (so `'30'` → `0`). Matches "numeric input" in the brief; a bare `Number()` cast would have accepted numeric strings.
- Two assertions disagreed with the prescribed `computeSlowmodeRemainingSeconds` formula. With `nowMs` invalid, `(Number(nowMs) || 0)` yields `0`, `elapsed` clamps to `0`, so the full cooldown is returned, not `0`. Tests follow the formula: `NaN`/`undefined` `nowMs` → full cooldown (`60`), not `0`. Flagged for reviewer.

## Checksums
```
1c1954642ef65be4eb789c60899e3d4c  rust/gui-spike/ui/features/slowmode.js
8c6042eb74fe3f1ccf5fc67ddb6cf32c  tests/unit/slowmode.test.mjs
52987bed0d4e41c51da9f571b196f46c  red.log
c5190712d7969b6ebbbcc986457e0f16  green.log
```

## Not done (by instruction)
`gate:worker` and `git commit` were not run; `npx tsc` typecheck was not run either (token-lean mode).
