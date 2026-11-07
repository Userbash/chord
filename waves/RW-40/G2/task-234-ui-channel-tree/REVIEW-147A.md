# REVIEW-147A — PR-118 / task-234 (FE-P2-26) Guild Channel Tree Sorter

**Reviewer:** Technical Reviewer (DeepSeek), token-lean mode
**Date:** 2026-10-31
**Files reviewed:** `rust/gui-spike/ui/features/channel-tree.js`, `tests/unit/channel-tree.test.mjs`, `REPORT.md`

## Verdict: APPROVED

Targeted suite green: `node --test tests/unit/channel-tree.test.mjs` -> `tests 9 / pass 9 / fail 0`.
Source restored bit-exact after mutation testing: md5 `04c1e353683249095d2c3c80e26326d0` (matches REPORT).

## Mutation testing (5/5 killed)

| # | Mutation | Location | Result |
|---|---|---|---|
| M1 | `isVoiceLikeChannel` drops `GUILD_STAGE_VOICE` (`type === 2` only) | line 56 | KILLED |
| M2 | Inverted voice-like ordering (`voiceB - voiceA`) | line 68 | KILLED |
| M3 | Removed id tiebreak in `compareCategories` (returns `0`) | line 87 | KILLED |
| M4 | Collapse filter ignored: `visibleChildren = allChildren.slice()` | line 126-127 | KILLED |
| M5 | Unknown-parent fallback removed: `bucketKey = parentId` | line 113 | KILLED |

After each mutation the suite failed; after restore the suite passed 9/9 again.

## Contract check

- `CHANNEL_TYPES` matches Discord enum (0,1,2,3,4,5,13,15) and is frozen — verified by test.
- Categories sorted by `position`, then `id` (`localeCompare`); both `allChildren` and
  `visibleChildren` populated on every node.
- Bucket sort: non-voice-like before voice-like, then `position`, then `id`; non-finite
  `position` coerced to `0`.
- Collapsed category exposes only `unread === true` children in `visibleChildren`, keeps
  `allChildren` intact; uncategorized channels never filtered.
- Orphan channels (parent missing from payload) fall into `uncategorized` instead of being dropped.
- Defensive input handling: non-array `channels` / `collapsedCategoryIds` degrade to empty.
- No mutation of the caller's array (`slice()` before `sort` in all three paths); input-immutability test passes.

## Notes / non-blocking

- Nesting of categories is not supported (single-level `parentId` buckets). REPORT lists this as a
  known limitation and Discord does not nest guild channels, so it is out of scope for FE-P2-26.
- `unread` is a client-side flag; the contract documents it as such. No ambiguity found.
- Token-lean mode: full `gate:worker` and workspace-wide suites were not run per instructions; the
  batch/integration gate remains the Team Lead's step.

## Hygiene

- No `git commit` performed by the reviewer; working tree left with the worker's changes.
- Only this review file was added under `waves/RW-40/G2/task-234-ui-channel-tree/`.
