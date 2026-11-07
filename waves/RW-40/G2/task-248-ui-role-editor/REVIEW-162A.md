# REVIEW-162A — task-248 (Guild Role Management & Permission Matrix Editor, FE-P0-12)

**Verdict: APPROVED**

- Reviewer dispatch: `ctx_fa7dae10054c` (technical review, token-lean mode)
- Card: `task-248`, wave `RW-40`, department `G2 Frontend`
- Reviewed artifact: `rust/gui-spike/ui/features/role-editor.js`
- Test oracle: `tests/unit/role-editor.test.mjs`
- Baseline md5 (must match `REPORT.md`): `ea1895e9d69862d360cab7a9642ddf81` — **match**

## 1. Baseline (green, before mutation)

```
node --test tests/unit/role-editor.test.mjs
ℹ tests 8
ℹ suites 3
ℹ pass 8
ℹ fail 0
ℹ skipped 0
exit=0
```

## 2. Mutation testing — 5/5 killed

Each mutation was applied to a single site, the suite was run, and the original file was
restored from a byte-exact backup (`cmp` clean) before the next mutation.

| # | Mutation | Site | Result | Killed by |
|---|----------|------|--------|-----------|
| M1 | Sort direction reversed (`posB - posA` -> `posA - posB`) | `sortRolesByPositionDesc` comparator | exit=1, pass=4 / fail=4 | `sortRolesByPositionDesc`, `createRoleEditorController` |
| M2 | Tie-break on `String(id)` flipped (`return -1` -> `return 1`) | `sortRolesByPositionDesc` comparator | exit=1, pass=6 / fail=2 | «сортирует роли по убыванию position…» |
| M3 | Permission bit check inverted (`!== 0` -> `=== 0`) | perm toggle `isChecked` | exit=1, pass=4 / fail=4 | «рендерит [data-role-editor]…» (aria-checked for KICK/BAN/ADMIN/MANAGE_ROLES) |
| M4 | Name max length relaxed (`> 100` -> `> 1000`) | save handler validation | exit=1, pass=7 / fail=1 | «валидирует длину имени роли (1..100 символов после trim)» |
| M5 | Saved name emitted untrimmed (`name: trimmedName` -> raw draft name) | save payload | exit=1, pass=6 / fail=2 | «переключает биты прав и флаг hoist…» (deepEqual payload) |

Kill rate: **5/5 (100%)**. No mutation survived; no test was weakened or modified.

## 3. Restore verification

```
md5sum rust/gui-spike/ui/features/role-editor.js
ea1895e9d69862d360cab7a9642ddf81
cmp /tmp/opencode/role-editor.orig.js rust/gui-spike/ui/features/role-editor.js
byte-identical to backup
node --test tests/unit/role-editor.test.mjs  ->  tests 8 / pass 8 / fail 0
```

File is byte-for-byte the reviewed artifact; working tree carries no mutation residue.

## 4. Contract observations (non-blocking)

- `sortRolesByPositionDesc` clones each element and never mutates input — covered by both
  identity (`notEqual(sorted[0], input[3])`) and deep-equality snapshot assertions.
- Permissions matrix uses Discord bit masks exactly as shipped in
  `ROLE_PERMISSIONS_CATALOG_RU`; the catalog is frozen at both array and item level.
- Name validation is performed on `trim()` inside `1..100`; boundaries 0/101/100 are all
  exercised by the suite.
- DOM access is delegated to `container.ownerDocument.createElement`, so the controller stays
  testable without a global `document`.

## 5. Scope discipline

Reviewed only `rust/gui-spike/ui/features/role-editor.js` and `tests/unit/role-editor.test.mjs`.
`gate:worker` and `git commit` were intentionally not run (token-lean review dispatch);
no shared files (`state/*`, `TASKS.md`, `waves/*` outside this card) were touched.

**Verdict: APPROVED** — 8/8 green, 5/5 mutations killed, artifact restored byte-identically
(`ea1895e9d69862d360cab7a9642ddf81`).
