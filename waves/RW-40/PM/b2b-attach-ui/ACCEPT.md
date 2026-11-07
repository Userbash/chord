# Приёмка карточки: `RW-40` / `B2b-ATTACH-UI` (Pass 4 — Attachment Composer Queue & 20 MiB Limit)

- **Статус**: `ACCEPTED` (Code Slice #27)
- **Дата**: 2026-10-30
- **Координатор**: Team Lead / PM (`discord-lab`)
- **Отчёт воркера**: `waves/RW-40/PM/b2b-attach-ui/REPORT-PASS4.md`
- **Независимое ревью**: `waves/RW-40/PM/b2b-attach-ui/REVIEW-PASS4.md` — `PASS` (`P0=0, P1=0, P2=0`)

## Доказательная база (`TEST-POLICY.md`)

1. **Red-прогон**: `waves/RW-40/PM/b2b-attach-ui/red-pass4.log` — падение до правки кода (`EXIT_CODE=1`).
2. **Green-прогон**: `waves/RW-40/PM/b2b-attach-ui/green-pass4.log` — `tests/unit/composer.test.mjs` и `ui-tsc` (`EXIT_CODE=0`).
3. **Мутации**: `waves/RW-40/PM/b2b-attach-ui/mutations-pass4.log` — 5/5 мутантов (`M1`–`M5`) убиты (`EXIT_CODE=1`), базовые MD5 восстановлены.
