# Приёмка карточки: `RW-40` / `INT-05-IPC-ALIASES` (Pass 2 — 100% UI ↔ Tauri `main.rs` IPC Parity)

- **Статус**: `ACCEPTED` (Code Slice #34)
- **Дата**: 2026-10-30
- **Координатор**: Team Lead / PM (`discord-lab`)
- **Отчёт воркера**: `waves/RW-40/PM/int05-ipc-aliases/REPORT.md`
- **Независимое ревью (DeepSeek Pass 2)**: `waves/RW-40/PM/int05-ipc-aliases/REVIEW-PASS2.md` — `PASS` (`P0=0, P1=0`)
- **Интеграционный гейт**: `.gate/worker-2026-10-30T14-36-10-163Z.json` (`verdict: passed`, `meta 102/102`, `unit 876/876`, `spike-rust PASS`, `workspace-rust 1142/1142`, `smoke 22/22`)

## Доказательная база (`TEST-POLICY.md`)

1. **Red-прогон**: `waves/RW-40/PM/int05-ipc-aliases/red.log` — падение до реализации (`EXIT_CODE=101`).
2. **Green-прогон**: `waves/RW-40/PM/int05-ipc-aliases/green.log` — `26/26 Rust tests`, `clippy -D warnings`, `rustfmt --check`, `ipc-path.test.mjs 12/12` (`EXIT_CODE=0`).
3. **Мутации**: `waves/RW-40/PM/int05-ipc-aliases/mutations.log` — 5/5 мутантов (`MUT-1`–`MUT-5`) убиты (`EXIT_CODE=101`), базовые MD5 восстановлены:
   - `a9b2c9164e121deb3497e5cfac195ad6` `rust/gui-spike/src/main.rs`
   - `2c6efc012048d839b26b95cffdc18edc` `rust/gui-spike/tests/ipc_wiring_contract.rs`
