# STUB-16-ATSPI-FIX — независимое ревью Pass 3

**Вердикт: PASS. P0=0, P1=0, P2=0.**

- `renderMemberRow` assigns `role="listitem"` to populated `.member` rows by default; the member screen retains at least 10 rows. The inner `.member-label` has `role="button"`, `aria-haspopup="dialog"`, `aria-controls="user-profile-popover"`, and `aria-expanded` initialized from the open state.
- `syncTriggerAria` updates the row and its nested button together when the popover opens and closes. The unit regression asserts row role, nested trigger attributes, and the false-to-true transition; the red log records the pre-fix failure (`actual: button`, expected `listitem`).
- `green-pass3.log` records `gui-window.smoke.test.mjs` PASS (1/1, no skips), including the AT-SPI probe, and the combined unit run PASS (166/166). `mutations-pass3.log` records 5/5 KILLED and matching baseline/restored MD5.
- Independently checked artifact MD5s match `REPORT-PASS3.md`: source `55356f6ee740d224c1339d70fc08ec1d`, regression test `2455ee3503f662730eb03bbbe992e6f5`, red log `7672e63963b09f5fff2e4c70fb547b79`, green log `edbbcc1fb5ce7b7f1afc75753738856b`, mutation log `cff5c073ee2be5b65fb1298d37787e0f`.

Решение ограничено контрактом AT-SPI и триггера из §4.1, а также приложенными red/green/mutation evidence. Замечаний уровней P0/P1/P2 в проверенном объёме нет.
