# Codex Review 27A: INT-05-IPC-ALIASES

**Verdict: RETURN (P1)**

## Findings

- **P1 — Batch gate is not green.** The applicable saved artifact `.gate/worker-2026-10-30T13-37-49-761Z.json` is for HEAD `4e240481e59ff4d49732ac8bd5ba08a006363748` and has `verdict: failed`: `meta` is 101/102 because `tests/unit/voice-devices-settings.test.mjs` has no §8 entry, and `smoke` is 21/22 because `tests/smoke/gui-window.smoke.test.mjs` exits 1. Although these failures are outside INT-05, TEST-POLICY.md §3 and §9 require a green gate before acceptance; rerun the batch gate after those failures are resolved.

## Reviewed Contract

- `rust/gui-spike/src/main.rs:1091-1100` registers each requested alias exactly once in `tauri::generate_handler![...]`: `create_invite`, `attachments_upload_slots`, `pins_list`, and `threads_list`.
- Each alias reads `backend.token()` once inside `token_or`, takes no token/bearer argument, and does not log. The wrappers retain their domain error variants and delegate to the corresponding existing crate functions.
- `attachments_upload_slots` accepts `channelId` and `files`, with each item accepting the UI's `filename`, `file_size`, `id`, and `is_clip`; it returns `{ attachments }`, as consumed by `composer.js:725-737`.
- `create_invite` accepts the flat `channelId`, `maxAge`, `maxUses`, `temporary`, and nullable `flags` payload from `invite-modal.js:370-376`; it also supports the documented nested `params` form.
- `pins_list` and `threads_list` accept `channelId` from `app.js:1372` and `app.js:1378`. `archived` is optional and defaults to `false`.

## Evidence Review

- `red.log`: saved command exits 101; failures identify the missing alias registrations and command declarations before implementation.
- `green.log`: targeted Rust suites total 26/26 (8 library + 11 binary + 7 integration), Clippy and rustfmt exit 0, and `ipc-path.test.mjs` passes 12/12. No Cargo command was run for this review.
- `mutations.log`: all five mutations exit 101 on relevant assertions; after each restore, `main.rs` returns to MD5 `a9b2c9164e121deb3497e5cfac195ad6`, and the final targeted run passes. The test sources are not weakened by the reviewed change.
- Current MD5s match the report for `main.rs`, `ipc_wiring_contract.rs`, and all three saved logs.

Resolve the batch-gate failures and provide a passing gate artifact before acceptance. No source or test files were changed during this review.
