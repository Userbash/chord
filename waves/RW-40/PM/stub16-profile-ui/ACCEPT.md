# Приёмка карточки: `RW-40` / `STUB-16 / MB-03` (Pass 3 — Member Profile Popover + AT-SPI `list item`)

- **Статус**: `ACCEPTED` (Code Slice #32)
- **Дата**: 2026-10-30
- **Координатор**: Team Lead / PM (`discord-lab`)
- **Отчёт воркера**: `waves/RW-40/PM/stub16-profile-ui/REPORT-PASS3.md`
- **Независимое ревью**: `waves/RW-40/PM/stub16-profile-ui/REVIEW-PASS3.md` — `PASS` (`P0=0, P1=0, P2=0`)

## Доказательная база (`TEST-POLICY.md`)

1. **Red-прогон**: `waves/RW-40/PM/stub16-profile-ui/red-pass3.log` — падение до правки кода (`EXIT_UNIT=1`, `EXIT_SMOKE=1`).
2. **Green-прогон**: `waves/RW-40/PM/stub16-profile-ui/green-pass3.log` — `gui-window.smoke.test.mjs` 1/1 pass (AT-SPI `list item >= 10` на экране участников и в чате), `gui-spike-regressions.test.mjs` 13/13, `gui-spike-features.test.mjs` 124/124, `gui-spike-model.test.mjs` 29/29, `ui-tsc` (`EXIT_CODE=0`).
3. **Мутации**: `waves/RW-40/PM/stub16-profile-ui/mutations-pass3.log` — 5/5 мутантов (`M1`–`M5`) убиты (`EXIT_CODE=1`), базовые MD5 восстановлены:
   - `55356f6ee740d224c1339d70fc08ec1d` `rust/gui-spike/ui/features/members.js`
   - `2455ee3503f662730eb03bbbe992e6f5` `tests/unit/gui-spike-regressions.test.mjs`
