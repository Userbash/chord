# Приёмка карточки B2a-ATTACH-TIER-FIX-PASS2 — Загрузка вложений и лимиты по буст-уровням (`attachments.rs`)

- **Статус:** ACCEPTED
- **Дата:** 2026-10-30
- **Независимое ревью:** `waves/RW-40/PM/b2a-attach/REVIEW-PASS3.md` — **PASS (`P0=0, P1=0, P2=0`)**
- **Отчёт:** `waves/RW-40/PM/b2a-attach/REPORT-PASS3.md`
- **Файлы:**
  - `rust/gui-spike/src/attachments.rs` (`3377d6dcd0850da6b92dbaac3a6c7dac`)
  - `rust/gui-spike/tests/attachments_contract.rs` (`45d107db41d4c1f443de8596b1196e49`, `17/17 passed`)
- **Проверки:**
  - Контракт RE (`Math.max(0x1400000, tierLimit)`): базовый пол `20 MiB` (`None`/`Tier1`), `Tier2` = `50 MiB`, `Tier3` = `100 MiB`.
  - Мутации: `4/4 KILLED` (`waves/RW-40/PM/b2a-attach/mutations-pass3.log`).
