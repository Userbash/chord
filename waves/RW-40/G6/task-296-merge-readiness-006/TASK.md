# TASK-296 — Итоговый отчёт `MERGE-READINESS-006.md` и обновление манифеста `BUILD-003.md` (`L1 / P2`)

- **Отдел:** `G6 Tech Writer / Parity`
- **Уровень сложности / Модель:** `L1 (Микро-задача 1–2 файла)` → `DeepSeek` (`--agent opencode --model deepseek/deepseek-v4.1-flash`)
- **Приоритет:** `P2`
- **Ветка:** `docs/task-296-merge-readiness-006` (от `unstable`)

---

## 1. Строгий Write-Scope (ровно 2 файла)

1. `waves/RW-40/G6/MERGE-READINESS-006.md` (новый итоговый отчёт готовности к слиянию `testing -> beta -> main`)
2. `waves/RW-40/BUILDS/BUILD-003.md` (обновление манифеста сборки `Build-003` списком влитых PR `PR-125..PR-175`)

---

## 2. Требования к содержанию

1. В `waves/RW-40/G6/MERGE-READINESS-006.md` зафиксируй:
   - Сводку влитых PR финала волны `RW-40` (`PR-137..PR-175`), включая единый BFF-роутер `bff_domains_router.rs` (`PR-153`/`PR-164`) и команду `invoke_bff_domain` (`PR-173`), `composer-popovers-bridge.js` (`PR-154`/`PR-163`), `app-shell-wiring.js` (`PR-155`/`PR-157`/`PR-172`), `safe_storage_contract.rs` (`PR-168`), расширение `tsconfig` и `test-impact.mjs` (`PR-165`), синхронизацию `TDD-PLAN.md` (`PR-167`), `FORWARDED_KINDS` (`PR-166`) и полный стек `RE-GAP-22` (`Voice WebRTC Diagnostics`: `stats.rs` `PR-170`, `voice_diagnostics.rs` `PR-174`, `VOICE-DIAGNOSTICS-SPEC.md` `PR-171`, `voice-diagnostics.js` `PR-169` + `voice.js` `PR-175`).
   - Статус всех гейтов (`typecheck = 0 errors`, `test:meta = 102/102 PASS`, `plan-sync = 5/5 PASS`, `keyring-live = 13/13 PASS`, `dispatch_ui = 58/58 PASS`, `test-impact = 58/58 PASS`).
   - Вердикт готовности к продвижению `unstable -> testing -> beta -> main`: `READY FOR BETA & RELEASE (P0 = 0, P1 = 0)`.
2. В `waves/RW-40/BUILDS/BUILD-003.md` актуализируй таблицу задач и PR до `PR-175`.
3. Запиши краткий отчёт в `waves/RW-40/G6/task-296-merge-readiness-006/REPORT.md`.
