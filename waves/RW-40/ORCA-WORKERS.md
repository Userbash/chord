# ORCA-WORKERS — один воркер на задачу

Дата: 2026-10-24. PM. Решение владельца: **не использовать native subagents** для новых
задач; запускать отдельного Orca-воркера на каждую задачу.

## 1. Правила

| Правило | Значение |
|---|---|
| Один task | один Orca worker (terminal/chat) |
| Read-only | может работать в текущем checkout |
| Writer | только один writer на checkout; иначе отдельный Orca worktree |
| Worktree | создаётся от **закоммиченного** baseline, иначе не видит dirty-изменения |
| Handoff | отдельный handle; prompt доставлен (`handed-to-terminal`/`accepted`) |
| Ownership | PM проверяет/мержит; Orca worker не коммитит |
| Не дублировать | «unfinished native workers» не дублируются Orca-воркерами |

## 2. Текущие Orca-воркеры

OpenCode: **`deepseek/deepseek-flash` (DeepSeek V4.1 Flash)**.
Codex — только code review. 4 writer с непересекающимися файлами; `cargo` один (G1).

| Task | Handle | Режим | Файлы/отчёт |
|---|---|---|---|
| C-02-UI-OLDER | `term_58e9f3a2-ebfa-42f2-b17f-c93701623e54` | **writer** | `ui/features/chat.js` |
| G-01b-CONTRACT | `term_78500e18-ecb9-4a28-a168-1f743d7483ae` | **writer** | `settings_store.rs` |
| QA-M2-02-VERIFY | `term_c8cf13b1-3035-4559-ae15-c17e7a3094b3` | **writer** | `tests/e2e/**`, `tests/helpers/**` |
| QA-M2-04-VERIFY | `term_bc68781a-2148-428d-a7e8-783be87124cd` | **writer** | `.github/workflows/**`, `scripts/ci.sh` |
| SEC-L02-FINAL | `term_e9528cd2-d977-4e63-a289-c7fac0d39fe2` | read-only | `QUALITY/SEC-L02.md` |
| MERGE-READINESS-05 | `term_93810767-043d-4463-823a-a754f0264b1e` | read-only | `PARITY/MERGE-READINESS-05.md` |
| G3-STREAM-REVIEW | `term_50483361-8d32-4eb6-8175-48a6874d1c40` | read-only | `DESIGN/STREAM-REVIEW.md` |
| REVIEW-L02-UI-FIX | `term_5da6aedb-84e7-4444-b9bc-cd45db5030dd` | Codex review-only | `PM/l02-ui/REVIEW-FIX.md` |

## 3. Почему code-задачи пока в текущем checkout

Рабочее дерево **dirty**, HEAD = `0239543`; отдельный Orca worktree от HEAD не увидит
RW-39/RW-40 изменения. Поэтому:
1. координатор делает integration commit;
2. после этого `orca worktree create --name <task> --agent codex --prompt ...`
   для каждой writer-задачи;
3. read-only tasks могут идти в текущем checkout.

## 4. Следующие worktree-воркеры (после baseline commit)

| Task | Worktree | Owner |
|---|---|---|
| L-02 dispatch | `l02-dispatch` | G1+G2 |
| C-02 paging | `c02-paging` | G2 |
| QA-E2E | `qa-e2e` | G4 |
| PLAT-11a | `plat-11a` | G1 |
| G5 portal | `g5-portal` | G5 |

## 5. Stop conditions

- worker не доставил prompt `accepted:true` — не считать запущенным;
- writer без worktree при dirty/чужом writer — не запускать;
- два worker на один checkout с записью — запрещено;
- live/commit — только PM/координатор.
