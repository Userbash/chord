# REVIEW-148A — task-235 (PR-119): Message Rich Embed Limits & Safe URL Validator

Reviewer: Technical Review (DeepSeek), token-lean mode.
Branch: `feature/task-235-ui-embed-formatter` @ worktree `feature-task-235-ui-embed-formatter`.
Scope reviewed: `rust/gui-spike/ui/features/embed-formatter.js`, `tests/unit/embed-formatter.test.mjs`, `REPORT.md`.

## Verdict: APPROVED

Targeted test `node --test tests/unit/embed-formatter.test.mjs`: exit 0, `tests 10 / pass 10 / fail 0 / skipped 0`.

## Contract check
- `EMBED_LIMITS` frozen: 10 embeds, title 256, description 4096, fields 25, field name 256, field value 1024, footer 2048 — matches Discord rich embed limits.
- `isSafeEmbedUrl` parses via `new URL()` and accepts only `http:`/`https:`; rejects `javascript:`, `data:`, `file:`, `mailto:`, protocol-relative `//host`, relative paths, empty/whitespace and non-strings. No regex-based bypass surface.
- `normalizeEmbedForRender` truncates by code units, drops unsafe URLs to `null`, maps integer `0..0xFFFFFF` to `#rrggbb` with `#2b2d31` fallback, caps fields at 25, coerces `inline` strictly via `=== true`, never mutates input (`slice` + new objects), and degrades malformed input to an empty embed.
- Tests assert against independent literal expectations (Discord limit numbers, hex literals), not against implementation internals.

## Mutations — 5/5 killed

| # | Mutation | File diff | Test run | Result |
|---|---|---|---|---|
| M1 | `MAX_TITLE_CHARS: 256` → `255` | `embed-formatter.js:17` | exit 1, fail 3 | KILLED |
| M2 | drop `\|\| parsed.protocol === 'http:'` | `:48` | exit 1, fail 1 | KILLED |
| M3 | `color > MAX_COLOR_INT` → `color >= MAX_COLOR_INT` | `:69` | exit 1, fail 1 | KILLED |
| M4 | `raw.fields.slice(0, MAX_FIELDS)` → `slice(0, 30)` | `:106` | exit 1, fail 1 | KILLED |
| M5 | `return value.length > max ? value.slice(0, max) : value;` → `return value;` | `:59` | exit 1, fail 3 | KILLED |

Restore verified: `md5sum rust/gui-spike/ui/features/embed-formatter.js` = `8571b347913aa320e2affdaac4b44ec3`, identical to REPORT.md and to the pre-mutation baseline; post-restore test run is green again (10/10).

File hashes at review time:
- `rust/gui-spike/ui/features/embed-formatter.js` — `8571b347913aa320e2affdaac4b44ec3` (matches REPORT)
- `tests/unit/embed-formatter.test.mjs` — `ea130729e0557bd8d7dd0e869e0bf5c3` (matches REPORT)

## Notes (non-blocking)
- `gate:worker`, `cargo fmt/clippy/test --workspace` and `npx tsc` were not run: token-lean review scope limits verification to the targeted module test. Full gate belongs to the integration lane.
- `MAX_EMBEDS_PER_MESSAGE` is exported and asserted but not consumed by `normalizeEmbedForRender` (per-message cap is the caller's job); acceptable for this task's scope, worth confirming when the renderer lands.
- Text clamping is by UTF-16 code units, so an emoji can be split at the boundary; Discord counts grapheme-ish length. Not a defect for this PR, but flag it if the renderer displays broken surrogate pairs.

No commit was made by this review.
