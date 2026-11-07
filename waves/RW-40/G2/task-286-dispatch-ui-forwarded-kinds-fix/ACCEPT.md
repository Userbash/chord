# ACCEPT — task-286 (`G2 Frontend L1 / P0`, `FE-FIX-08`: `FORWARDED_KINDS` sync)

- **PR:** `PR-166` (`branch:bugfix/task-286-dispatch-ui-forwarded-kinds-fix`)
- **Статус PM:** `APPROVED` (`Ready for Technical Review` -> `TR-195A`)
- **Проверка DoD:**
  1. `rust/gui-spike/ui/features/dispatch-model.js`: `FORWARDED_KINDS` расширен с 22 до 30 событий в порядке `net.rs::FORWARDED_DISPATCHES` (включая 8 имён `ExtendedDispatchKind::NAMES`: `MESSAGE_DELETE`, `MESSAGE_DELETE_BULK`, `CHANNEL_CREATE`, `CHANNEL_UPDATE`, `CHANNEL_DELETE`, `THREAD_CREATE`, `THREAD_UPDATE`, `THREAD_DELETE`).
  2. `tests/unit/dispatch_ui.test.mjs`: `A2` синхронизирован с полным оракулом из 30 событий (`DispatchKind::as_str` + `ExtendedDispatchKind::NAMES` + `MESSAGE_REACTION_REMOVE`) и проверяет двустороннее равенство `FORWARDED_KINDS` и `FORWARDED_DISPATCHES`; добавлен `B14b` на приём 8 расширенных событий (`ok=true, ignored=0, skipped=1`).
  3. `node --test tests/unit/dispatch_ui.test.mjs` — `58/58 PASS`; `npx tsc -p tsconfig.json --noEmit` — `0 errors`; регрессионный срез — `458/458 PASS`.
