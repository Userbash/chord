# Приёмка карточки: `RW-40` / `INT-04-ATTACH-PUT-BACKEND` (Pass 2)

- **Статус**: `ACCEPTED` (Code Slice #31)
- **Дата**: 2026-10-30
- **Координатор**: Team Lead / PM (`discord-lab`)
- **Отчёт воркера**: `waves/RW-40/PM/b2b-attach-ui/REPORT-BE-PUT-PASS2.md`
- **Независимое ревью**: `waves/RW-40/PM/b2b-attach-ui/REVIEW-BE-PUT-PASS2.md` — `PASS` (`P0=0, P1=0, P2=0`)

## Доказательная база (`TEST-POLICY.md`)

1. **Red-прогон**: `waves/RW-40/PM/b2b-attach-ui/red-be-put-pass2.log` — падение до правки кода (`EXIT_CODE=101`).
2. **Green-прогон**: `waves/RW-40/PM/b2b-attach-ui/green-be-put-pass2.log` — `src/lib.rs` 8/8, `src/main.rs` 10/10, `attachments_contract` 23/23, `ipc_wiring_contract` 5/5, `tests/meta/ipc-path.test.mjs` 12/12, `clippy -D warnings` и `rustfmt --check` чистые (`EXIT_CODE=0`).
3. **Мутации**: `waves/RW-40/PM/b2b-attach-ui/mutations-be-put-pass2.log` — 5/5 мутантов (`M1`–`M5`) убиты (`EXIT_CODE=101`), базовые MD5 восстановлены:
   - `75f50c624ab5a6b21a67281235f4b289` `rust/gui-spike/src/attachments.rs`
   - `bc38ac5943b929e7a620ef911cb208a5` `rust/gui-spike/src/lib.rs`
   - `8a71f311bf0ab47ff24450eaf389b2d4` `rust/gui-spike/src/main.rs`
   - `77157aadf4267ded8c225d8891c71a9f` `rust/gui-spike/tests/attachments_contract.rs`
